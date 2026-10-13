//! Streaming decode: a track (WoW's zone music is `.mp3`, minutes long) is read and decoded on a thread of
//! its own, resampled to the device rate on the way, and handed to the mixer in chunks through a bounded
//! channel, so the whole track is never held in memory and the frame that starts it decodes nothing.

use std::io::Cursor;
use std::sync::mpsc::{sync_channel, Receiver};

use anyhow::{bail, Context, Result};
use bevy::log::{debug, warn};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::TrackType;
use symphonia::core::io::MediaSourceStream;

use crate::sfx::frames_of_ref;

/// Frames in one chunk handed to the mixer (about 85 ms at 48 kHz).
const CHUNK: usize = 4096;
/// How many chunks the decoder may run ahead of the mixer before it waits (about 0.7 s).
const CHUNKS_AHEAD: usize = 8;
/// Bad packets in a row after which the track is given up.
const MAX_BAD_PACKETS: usize = 64;

/// A stateful linear resampler for a stream: it carries the fractional position and the unread tail
/// across chunks, so the seams between packets are as smooth as the signal itself.
pub struct Resampler {
    /// Input frames consumed per output frame (`from / to`).
    ratio: f64,
    /// The next output frame's position in `pending`.
    pos: f64,
    pending: Vec<[f32; 2]>,
}

impl Resampler {
    pub fn new(from: u32, to: u32) -> Resampler {
        Resampler {
            ratio: f64::from(from.max(1)) / f64::from(to.max(1)),
            pos: 0.0,
            pending: Vec::new(),
        }
    }

    /// Takes the next `input` and appends every output frame that is now fully determined to `out`.
    pub fn push(&mut self, input: &[[f32; 2]], out: &mut Vec<[f32; 2]>) {
        if (self.ratio - 1.0).abs() < 1.0e-12 {
            out.extend_from_slice(input);
            return;
        }
        self.pending.extend_from_slice(input);
        while (self.pos as usize) + 1 < self.pending.len() {
            let k = self.pos as usize;
            let a = (self.pos - k as f64) as f32;
            let (x, y) = (self.pending[k], self.pending[k + 1]);
            out.push([x[0] + (y[0] - x[0]) * a, x[1] + (y[1] - x[1]) * a]);
            self.pos += self.ratio;
        }
        let used = (self.pos as usize).min(self.pending.len());
        self.pending.drain(..used);
        self.pos -= used as f64;
    }
}

/// Starts decoding on a new thread and returns the receiving end: `load` produces the file's bytes (it
/// runs on the thread, so reading the archive does not stall the frame) and the sound arrives resampled
/// to `rate`. The receiver is closed when the track is over, the decoder failed (one warning), or the
/// receiver was dropped.
pub fn spawn<F>(name: String, rate: u32, load: F) -> Receiver<Vec<[f32; 2]>>
where
    F: FnOnce() -> Result<Vec<u8>> + Send + 'static,
{
    let (tx, rx) = sync_channel(CHUNKS_AHEAD);
    let spawned = std::thread::Builder::new()
        .name("decode".into())
        .spawn(move || {
            if let Err(e) = run(&name, rate, load, &tx) {
                warn!("audio: stream {name}: {e:#}");
            }
        });
    if let Err(e) = spawned {
        warn!("audio: could not start a decoder thread: {e}");
    }
    rx
}

fn run(
    name: &str,
    rate: u32,
    load: impl FnOnce() -> Result<Vec<u8>>,
    tx: &std::sync::mpsc::SyncSender<Vec<[f32; 2]>>,
) -> Result<()> {
    let bytes = load().context("reading the file")?;
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut reader = symphonia::default::get_probe().probe(
        &Default::default(),
        mss,
        Default::default(),
        Default::default(),
    )?;
    let track = reader
        .default_track(TrackType::Audio)
        .context("no audio track")?;
    let id = track.id;
    let Some(CodecParameters::Audio(params)) = track.codec_params.as_ref() else {
        bail!("no audio parameters");
    };
    let from = params.sample_rate.context("unknown sample rate")?;
    let mut decoder =
        symphonia::default::get_codecs().make_audio_decoder(params, &Default::default())?;
    let mut resampler = Resampler::new(from, rate);
    let mut chunk: Vec<[f32; 2]> = Vec::with_capacity(CHUNK * 2);
    let mut bad = 0;
    while let Some(packet) = reader.next_packet()? {
        if packet.track_id != id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(b) => b,
            Err(e) => {
                bad += 1;
                debug!("audio: stream {name}: a packet did not decode ({e})");
                if bad > MAX_BAD_PACKETS {
                    bail!("too many packets did not decode");
                }
                continue;
            }
        };
        bad = 0;
        resampler.push(&frames_of_ref(&buffer)?, &mut chunk);
        while chunk.len() >= CHUNK {
            let rest = chunk.split_off(CHUNK);
            let full = std::mem::replace(&mut chunk, rest);
            if tx.send(full).is_err() {
                return Ok(()); // the listener stopped
            }
        }
    }
    if !chunk.is_empty() {
        let _ = tx.send(chunk);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(n: usize) -> Vec<[f32; 2]> {
        (0..n).map(|i| [i as f32, -(i as f32)]).collect()
    }

    #[test]
    fn the_same_rate_passes_through_untouched() {
        let mut r = Resampler::new(48_000, 48_000);
        let mut out = Vec::new();
        r.push(&ramp(5), &mut out);
        assert_eq!(out, ramp(5));
    }

    #[test]
    fn chunked_resampling_equals_resampling_the_whole_at_once() {
        let input = ramp(1000);
        let mut whole = Vec::new();
        Resampler::new(44_100, 48_000).push(&input, &mut whole);
        let mut chunked = Vec::new();
        let mut r = Resampler::new(44_100, 48_000);
        for piece in input.chunks(37) {
            r.push(piece, &mut chunked);
        }
        assert_eq!(
            whole.len(),
            chunked.len(),
            "no frame lost or doubled at a seam"
        );
        for (a, b) in whole.iter().zip(&chunked) {
            assert!((a[0] - b[0]).abs() < 1.0e-3, "{a:?} {b:?}");
        }
    }

    #[test]
    fn a_ramp_stays_a_ramp_through_the_resampler() {
        // 44.1 -> 48 kHz: output frame n sits at input position n * 0.91875, the value there.
        let mut out = Vec::new();
        let mut r = Resampler::new(44_100, 48_000);
        for piece in ramp(300).chunks(64) {
            r.push(piece, &mut out);
        }
        for (n, f) in out.iter().enumerate() {
            let want = n as f64 * 44_100.0 / 48_000.0;
            assert!(
                (f64::from(f[0]) - want).abs() < 1.0e-2,
                "frame {n}: {f:?} vs {want}"
            );
        }
        assert!(
            out.len() >= 270,
            "about 300 x 48/44.1 frames came out: {}",
            out.len()
        );
    }

    #[test]
    fn downsampling_keeps_the_length_in_proportion() {
        let mut out = Vec::new();
        Resampler::new(96_000, 48_000).push(&ramp(2000), &mut out);
        assert!((out.len() as i64 - 1000).abs() <= 1, "{}", out.len());
    }

    /// A mono 16-bit PCM `.wav` of `frames` samples of a ramp.
    fn wav(rate: u32, frames: u32) -> Vec<u8> {
        let data_len = frames * 2;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes()); // PCM
        b.extend_from_slice(&1u16.to_le_bytes()); // mono
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..frames {
            b.extend_from_slice(&(((i % 2000) as i16 - 1000) * 10).to_le_bytes());
        }
        b
    }

    #[test]
    fn a_wav_streams_in_chunks_resampled_to_the_device_rate() {
        // One second at 24 kHz, played on a 48 kHz device: about 48 000 frames, in chunks of 4096.
        let rx = spawn("test".into(), 48_000, || Ok(wav(24_000, 24_000)));
        let mut total = 0;
        let mut chunks = 0;
        while let Ok(c) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
            assert!(c.len() <= CHUNK, "a chunk is at most {CHUNK} frames");
            total += c.len();
            chunks += 1;
        }
        assert!(chunks >= 11, "{chunks} chunks");
        assert!((total as i64 - 48_000).abs() <= 2, "{total} frames");
    }

    #[test]
    fn install_a_zone_music_mp3_streams_and_decodes_at_the_device_rate() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let rx = spawn("DayForest02".into(), 48_000, move || {
            chain
                .read_file("Sound\\Music\\ZoneMusic\\Forest\\DayForest02.mp3")
                .map_err(|e| anyhow::anyhow!("{e:#}"))
        });
        let (mut frames, mut peak) = (0usize, 0f32);
        while frames < 48_000 * 3 {
            match rx.recv_timeout(std::time::Duration::from_secs(10)) {
                Ok(c) => {
                    frames += c.len();
                    peak = c.iter().flatten().fold(peak, |p, s| p.max(s.abs()));
                }
                Err(_) => break,
            }
        }
        assert!(
            frames >= 48_000 * 3,
            "three seconds of music arrived: {frames}"
        );
        assert!(
            peak > 0.01 && peak < 1.5,
            "it is music, not silence or noise: peak {peak}"
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_ends_the_stream_without_a_panic() {
        let rx = spawn("missing".into(), 48_000, || bail!("no such file"));
        assert!(rx.recv_timeout(std::time::Duration::from_secs(5)).is_err());
        let rx = spawn("junk".into(), 48_000, || Ok(vec![1, 2, 3, 4]));
        assert!(rx.recv_timeout(std::time::Duration::from_secs(5)).is_err());
    }
}
