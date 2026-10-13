//! The mixer: a handful of decoded sounds added into the output buffer, and the thread that owns
//! the audio device. [`Mixer`] is pure (tested without a device); [`Output`] opens the default
//! output device through cpal on a thread of its own (a `cpal::Stream` is not `Send` everywhere)
//! and takes new voices over a channel.
//!
//! Three kinds of voice: a decoded one-shot ([`Voice`]), the same looped (an ambience bed), and a
//! streamed one ([`StreamVoice`], WoW's music: a decoder thread feeds it chunks, so a four-minute track
//! is never held in memory whole). Both carry a live `level` the owner moves (fades, crossfades), applied
//! as a ramp across each buffer so it does not click.
//!
//! The sound must never end for good. A full mixer retires the voice nearest its end for a new one (a
//! blow is never refused for want of room); a voice with a gain that is not a number is refused and a
//! sample that is not a number is zeroed before it reaches the device (a NaN in the stream can leave a
//! driver's filters silent for the rest of the run); a panic in the mix empties the mixer instead of
//! killing the stream; and a stream the device reports as failed (a headset pulled out, the default
//! device changed) is built again, on whatever device is the default then. Each of these is counted
//! ([`Stats`]) and written to the log, so the next silence says what happened.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

use crate::stream::Resampler;

/// The most one-shot / looped voices at once; past it a new voice takes the place of the one nearest
/// its end.
pub const MAX_VOICES: usize = 48;
/// The most streamed voices at once (a music track and the one fading out, a spare).
pub const MAX_STREAMS: usize = 4;
/// How often a failed stream is tried again (s).
const REBUILD_EVERY: Duration = Duration::from_secs(1);
/// How often the watcher looks at the counters and the stream.
const WATCH_EVERY: Duration = Duration::from_millis(250);

/// What went wrong in the mix, counted by the audio thread and read by its watcher; and the master
/// volume the player sets while it runs.
pub struct Stats {
    /// Voices that took the place of one still playing because the mixer was full.
    pub stolen: AtomicU32,
    /// Voices that could not be started at all (every place held a loop, or the gain was not a number).
    pub refused: AtomicU32,
    /// Output samples that were not numbers and were zeroed.
    pub non_finite: AtomicU32,
    /// Panics in the mix (the mixer was emptied each time).
    pub panics: AtomicU32,
    /// Errors the device reported on the stream.
    pub errors: AtomicU32,
    /// Times the stream was built again.
    pub restarts: AtomicU32,
    /// The master gain (an `f32`'s bits), applied to the whole mix: 1.0 leaves it as made.
    master: AtomicU32,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            stolen: AtomicU32::new(0),
            refused: AtomicU32::new(0),
            non_finite: AtomicU32::new(0),
            panics: AtomicU32::new(0),
            errors: AtomicU32::new(0),
            restarts: AtomicU32::new(0),
            master: AtomicU32::new(1.0f32.to_bits()),
        }
    }
}

impl Stats {
    /// Sets the master gain, 0 to 1 and a little over.
    pub fn set_master(&self, gain: f32) {
        let gain = if gain.is_finite() {
            gain.clamp(0.0, 4.0)
        } else {
            1.0
        };
        self.master.store(gain.to_bits(), Ordering::Relaxed);
    }

    pub fn master(&self) -> f32 {
        f32::from_bits(self.master.load(Ordering::Relaxed))
    }

    fn snapshot(&self) -> [u32; 6] {
        [
            self.stolen.load(Ordering::Relaxed),
            self.refused.load(Ordering::Relaxed),
            self.non_finite.load(Ordering::Relaxed),
            self.panics.load(Ordering::Relaxed),
            self.errors.load(Ordering::Relaxed),
            self.restarts.load(Ordering::Relaxed),
        ]
    }
}

/// One line in the log, on stderr and in `%LOCALAPPDATA%\WoWSouls\audio.log` (best effort): the audio
/// thread has no logger of the engine's.
fn note(line: &str) {
    eprintln!("audio: {line}");
    let Some(dir) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let dir = std::path::Path::new(&dir).join("WoWSouls");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("audio.log"))
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let _ = writeln!(f, "{secs} {line}");
    }
}

/// A loudness the owner changes while the sound plays: an `f32` in an atomic.
#[derive(Clone)]
pub struct Level(Arc<AtomicU32>);

impl Level {
    pub fn new(v: f32) -> Level {
        Level(Arc::new(AtomicU32::new(v.to_bits())))
    }

    pub fn set(&self, v: f32) {
        self.0.store(v.max(0.0).to_bits(), Ordering::Relaxed);
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

/// One playing sound: its frames at the device rate, a fractional position, the step per output
/// frame (the pitch) and a gain per channel.
pub struct Voice {
    pub frames: Arc<[[f32; 2]]>,
    pub pos: f64,
    pub step: f64,
    pub gain: [f32; 2],
    /// Set by the owner to cut the voice short (a quest voice when its dialogue closes).
    pub stop: Option<Arc<AtomicBool>>,
    /// Starts again at the top when it reaches the end (an ambience bed) until stopped.
    pub looped: bool,
    /// A loudness the owner moves while it plays, on top of `gain`.
    pub level: Option<Level>,
    /// The level the last buffer ended on (the ramp's start); `NaN` before the first buffer.
    prev_level: f32,
    finished: bool,
}

impl Voice {
    pub fn new(frames: Arc<[[f32; 2]]>, gain: (f32, f32)) -> Voice {
        Voice {
            frames,
            pos: 0.0,
            step: 1.0,
            gain: [gain.0, gain.1],
            stop: None,
            looped: false,
            level: None,
            prev_level: f32::NAN,
            finished: false,
        }
    }

    /// Frames still to play; a loop never runs out.
    fn remaining(&self) -> f64 {
        if self.finished
            || self
                .stop
                .as_ref()
                .is_some_and(|s| s.load(Ordering::Relaxed))
        {
            0.0
        } else if self.looped {
            f64::INFINITY
        } else {
            (self.frames.len() as f64 - self.pos).max(0.0)
        }
    }
}

/// A sound fed in chunks by a decoder thread: it plays what has arrived and is silent while it waits.
pub struct StreamVoice {
    rx: Receiver<Vec<[f32; 2]>>,
    cur: Vec<[f32; 2]>,
    at: usize,
    pub gain: f32,
    pub level: Level,
    pub stop: Arc<AtomicBool>,
    /// Set by the mixer when the stream ended (its decoder finished and everything was played) or was
    /// stopped.
    pub done: Arc<AtomicBool>,
    prev_level: f32,
    finished: bool,
    /// Brings the decoder's chunks to the device's rate when the device changed after they were made
    /// (`from`, `to`, the resampler).
    resampler: Option<(u32, u32, Resampler)>,
}

impl StreamVoice {
    pub fn new(
        rx: Receiver<Vec<[f32; 2]>>,
        gain: f32,
        level: Level,
        stop: Arc<AtomicBool>,
        done: Arc<AtomicBool>,
    ) -> StreamVoice {
        StreamVoice {
            rx,
            cur: Vec::new(),
            at: 0,
            gain,
            level,
            stop,
            done,
            prev_level: f32::NAN,
            finished: false,
            resampler: None,
        }
    }
}

/// What the audio thread is told.
pub enum Command {
    Voice(Voice),
    Stream(StreamVoice),
}

/// The level ramp across one buffer: from the last buffer's end to this one's.
fn ramp(prev: &mut f32, now: f32, frames: usize) -> (f32, f32) {
    let from = if prev.is_nan() { now } else { *prev };
    *prev = now;
    let step = if frames > 0 {
        (now - from) / frames as f32
    } else {
        0.0
    };
    (from, step)
}

pub struct Mixer {
    voices: Vec<Voice>,
    streams: Vec<StreamVoice>,
    stats: Arc<Stats>,
    /// The rate the sounds were made at, and the rate of the device now. They differ only after the
    /// device was changed for one with another rate.
    source_rate: u32,
    device_rate: u32,
    /// The master gain of the last buffer (NaN before the first), for the ramp.
    prev_master: f32,
}

impl Default for Mixer {
    fn default() -> Mixer {
        Mixer::new()
    }
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer::with_stats(Arc::new(Stats::default()), 48_000)
    }

    /// A mixer for a device of `rate` Hz (the rate every sound is made at) that counts into `stats`.
    pub fn with_stats(stats: Arc<Stats>, rate: u32) -> Mixer {
        Mixer {
            voices: Vec::with_capacity(MAX_VOICES),
            streams: Vec::with_capacity(MAX_STREAMS),
            stats,
            source_rate: rate,
            device_rate: rate,
            prev_master: f32::NAN,
        }
    }

    /// The device now runs at `rate`: sounds made for another rate are played faster or slower by the
    /// difference and the streams are resampled, so the pitch holds.
    pub fn set_device_rate(&mut self, rate: u32) {
        self.device_rate = rate;
        for s in &mut self.streams {
            s.resampler = None;
        }
    }

    /// Drops everything that plays (after a panic in the mix: whatever broke it goes with it). The
    /// streams are told they ended, so their owners start fresh ones.
    pub fn clear(&mut self) {
        self.voices.clear();
        for s in self.streams.drain(..) {
            s.done.store(true, Ordering::Relaxed);
        }
    }

    /// Starts a voice; `false` when it could not be started. A full mixer retires the voice nearest
    /// its end (one already stopped first) for the new one: a loop is never taken.
    pub fn add(&mut self, v: Voice) -> bool {
        if !v.gain.iter().all(|g| g.is_finite()) || !v.step.is_finite() || v.step <= 0.0 {
            self.stats.refused.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        if self.voices.len() >= MAX_VOICES {
            let victim = self
                .voices
                .iter()
                .enumerate()
                .map(|(i, o)| (i, o.remaining()))
                .filter(|(_, left)| left.is_finite())
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i);
            let Some(victim) = victim else {
                self.stats.refused.fetch_add(1, Ordering::Relaxed);
                return false;
            };
            self.voices.swap_remove(victim);
            self.stats.stolen.fetch_add(1, Ordering::Relaxed);
        }
        self.voices.push(v);
        true
    }

    /// Starts a streamed voice; `false` when the mixer is full or the gain is not a number (its `done`
    /// flag is then set so the owner does not wait for it).
    pub fn add_stream(&mut self, s: StreamVoice) -> bool {
        if self.streams.len() >= MAX_STREAMS || !s.gain.is_finite() {
            s.done.store(true, Ordering::Relaxed);
            self.stats.refused.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        self.streams.push(s);
        true
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.voices.len() + self.streams.len()
    }

    /// Mixes into `out` (interleaved, `channels` per frame), replacing its content: stereo as is,
    /// mono as the mean, more channels with the rest silent. Finished voices are retired. The sum
    /// is clamped to [-1, 1].
    pub fn fill(&mut self, out: &mut [f32], channels: usize) {
        out.fill(0.0);
        if channels == 0 {
            return;
        }
        let frames = out.len() / channels;
        let (source, device) = (self.source_rate, self.device_rate.max(1));
        let ratio = f64::from(source) / f64::from(device);
        for v in &mut self.voices {
            if v.stop.as_ref().is_some_and(|s| s.load(Ordering::Relaxed)) {
                v.finished = true;
                continue;
            }
            let len = v.frames.len();
            if len == 0 {
                v.finished = true;
                continue;
            }
            let (mut lvl, lvl_step) = match &v.level {
                Some(l) => ramp(&mut v.prev_level, l.get(), frames),
                None => (1.0, 0.0),
            };
            for f in 0..frames {
                if v.pos as usize >= len {
                    if v.looped {
                        v.pos %= len as f64;
                    } else {
                        break;
                    }
                }
                let k = v.pos as usize;
                let a = (v.pos - k as f64) as f32;
                let next = if v.looped {
                    (k + 1) % len
                } else {
                    (k + 1).min(len - 1)
                };
                let (x, y) = (v.frames[k], v.frames[next]);
                let l = (x[0] + (y[0] - x[0]) * a) * v.gain[0] * lvl;
                let r = (x[1] + (y[1] - x[1]) * a) * v.gain[1] * lvl;
                add_frame(&mut out[f * channels..(f + 1) * channels], l, r);
                v.pos += v.step * ratio;
                lvl += lvl_step;
            }
        }
        self.voices
            .retain(|v| !v.finished && (v.looped || (v.pos as usize) < v.frames.len()));
        for s in &mut self.streams {
            if s.stop.load(Ordering::Relaxed) {
                s.finished = true;
                continue;
            }
            let (mut lvl, lvl_step) = ramp(&mut s.prev_level, s.level.get(), frames);
            for f in 0..frames {
                if s.at >= s.cur.len() {
                    match s.rx.try_recv() {
                        Ok(chunk) => {
                            s.cur = if source == device {
                                chunk
                            } else {
                                let stale = s
                                    .resampler
                                    .as_ref()
                                    .is_none_or(|(from, to, _)| (*from, *to) != (source, device));
                                if stale {
                                    s.resampler =
                                        Some((source, device, Resampler::new(source, device)));
                                }
                                let mut converted = Vec::with_capacity(chunk.len() + 8);
                                if let Some((_, _, r)) = s.resampler.as_mut() {
                                    r.push(&chunk, &mut converted);
                                }
                                converted
                            };
                            s.at = 0;
                        }
                        // The decoder has not caught up: silence until it has.
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            s.finished = true;
                            break;
                        }
                    }
                    if s.cur.is_empty() {
                        continue;
                    }
                }
                let x = s.cur[s.at];
                s.at += 1;
                add_frame(
                    &mut out[f * channels..(f + 1) * channels],
                    x[0] * s.gain * lvl,
                    x[1] * s.gain * lvl,
                );
                lvl += lvl_step;
            }
        }
        for s in &self.streams {
            if s.finished {
                s.done.store(true, Ordering::Relaxed);
            }
        }
        self.streams.retain(|s| !s.finished);
        // The player's master volume, live: ramped across the buffer so a slider never clicks.
        let (mut gain, gain_step) = ramp(&mut self.prev_master, self.stats.master(), frames);
        for f in 0..frames {
            for sample in &mut out[f * channels..(f + 1) * channels] {
                *sample *= gain;
            }
            gain += gain_step;
        }
        let mut bad = 0;
        for s in out.iter_mut() {
            if s.is_finite() {
                *s = s.clamp(-1.0, 1.0);
            } else {
                *s = 0.0;
                bad += 1;
            }
        }
        if bad > 0 {
            self.stats.non_finite.fetch_add(bad, Ordering::Relaxed);
        }
    }
}

fn add_frame(o: &mut [f32], l: f32, r: f32) {
    match o.len() {
        1 => o[0] += 0.5 * (l + r),
        _ => {
            o[0] += l;
            o[1] += r;
        }
    }
}

/// What the audio callback works on: the mixer and the channel new voices arrive on. Behind a mutex so a
/// new stream can take over from a failed one; only one stream runs at a time, so it is never contended.
struct Shared {
    mixer: Mixer,
    rx: Receiver<Command>,
}

fn locked(shared: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|p| p.into_inner())
}

/// The audio device, on its own thread: new voices go in through [`Output::play`].
pub struct Output {
    tx: Sender<Command>,
    /// The rate of the device first opened: sounds are decoded to it.
    pub rate: u32,
    stats: Arc<Stats>,
    _stop: Sender<()>,
}

impl Output {
    /// Opens the default output device and starts its stream. The thread that owns it also watches it:
    /// a stream the device reports as failed is built again (see the module notes).
    pub fn open() -> Result<Output> {
        let (tx, rx) = channel::<Command>();
        let (stop_tx, stop_rx) = channel::<()>();
        let (ready_tx, ready_rx) = channel::<Result<u32, String>>();
        let stats = Arc::new(Stats::default());
        let watched = stats.clone();
        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || watch(rx, stop_rx, ready_tx, watched))
            .context("starting the audio thread")?;
        let rate = ready_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| anyhow!("the audio device did not answer in 5 s"))?
            .map_err(|e| anyhow!(e))?;
        Ok(Output {
            tx,
            rate,
            stats,
            _stop: stop_tx,
        })
    }

    /// The master volume of everything that plays, now.
    pub fn set_master(&self, gain: f32) {
        self.stats.set_master(gain);
    }

    pub fn play(&self, v: Voice) {
        let _ = self.tx.send(Command::Voice(v));
    }

    pub fn play_stream(&self, s: StreamVoice) {
        let _ = self.tx.send(Command::Stream(s));
    }
}

/// The audio thread: opens the device, then looks at the stream and the counters until the [`Output`] is
/// dropped. A stream that failed is dropped and a new one built on the default device, once a second,
/// until one runs.
fn watch(
    rx: Receiver<Command>,
    stop_rx: Receiver<()>,
    ready_tx: Sender<Result<u32, String>>,
    stats: Arc<Stats>,
) {
    let broken = Arc::new(AtomicBool::new(false));
    let shared = Arc::new(Mutex::new(Shared {
        mixer: Mixer::with_stats(stats.clone(), 48_000),
        rx,
    }));
    let first = default_device().and_then(|(device, supported, rate)| {
        locked(&shared).mixer = Mixer::with_stats(stats.clone(), rate);
        start(&device, &supported, &shared, &broken, &stats).map(|stream| (stream, rate))
    });
    let (stream, rate) = match first {
        Ok(opened) => opened,
        Err(e) => {
            let _ = ready_tx.send(Err(format!("{e:#}")));
            return;
        }
    };
    let _ = ready_tx.send(Ok(rate));
    let mut stream = Some(stream);
    let mut seen = stats.snapshot();
    let mut last_try = Instant::now();
    let mut failing = false;
    // A timeout is a tick; anything else means the `Output` was dropped.
    while let Err(RecvTimeoutError::Timeout) = stop_rx.recv_timeout(WATCH_EVERY) {
        let now = stats.snapshot();
        if now != seen {
            note(&format!(
                "stolen {} refused {} non-finite {} panics {} errors {} restarts {} (voices taken over / refused / bad samples zeroed / mix panics / device errors / stream rebuilds)",
                now[0], now[1], now[2], now[3], now[4], now[5]
            ));
            seen = now;
        }
        if stream.is_some() && !broken.load(Ordering::Relaxed) {
            continue;
        }
        stream = None; // the failed stream goes before the next is tried
        if last_try.elapsed() < REBUILD_EVERY {
            continue;
        }
        last_try = Instant::now();
        broken.store(false, Ordering::Relaxed);
        let rebuilt = default_device().and_then(|(device, supported, rate)| {
            locked(&shared).mixer.set_device_rate(rate);
            start(&device, &supported, &shared, &broken, &stats).map(|stream| (stream, rate))
        });
        match rebuilt {
            Ok((s, rate)) => {
                stats.restarts.fetch_add(1, Ordering::Relaxed);
                note(&format!("the output was built again ({rate} Hz)"));
                stream = Some(s);
                failing = false;
            }
            Err(e) => {
                if !failing {
                    note(&format!("no output yet ({e:#}); trying again every second"));
                }
                failing = true;
            }
        }
    }
}

/// The default output device, its format and its sample rate.
fn default_device() -> Result<(cpal::Device, cpal::SupportedStreamConfig, u32)> {
    let device = cpal::default_host()
        .default_output_device()
        .context("no audio output device")?;
    let supported = device
        .default_output_config()
        .context("the device's default output format")?;
    let rate = supported.sample_rate();
    if !(8000..=384_000).contains(&rate) {
        bail!("the device reports a sample rate of {rate}");
    }
    Ok((device, supported, rate))
}

/// Builds the stream on `device` and starts it.
fn start(
    device: &cpal::Device,
    supported: &cpal::SupportedStreamConfig,
    shared: &Arc<Mutex<Shared>>,
    broken: &Arc<AtomicBool>,
    stats: &Arc<Stats>,
) -> Result<cpal::Stream> {
    let config = supported.config();
    let channels = usize::from(config.channels);
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => stream::<f32>(device, &config, channels, shared, broken, stats)?,
        cpal::SampleFormat::I16 => stream::<i16>(device, &config, channels, shared, broken, stats)?,
        cpal::SampleFormat::U16 => stream::<u16>(device, &config, channels, shared, broken, stats)?,
        other => bail!("the device wants {other:?} samples, which are not handled"),
    };
    stream.play().context("starting the output stream")?;
    Ok(stream)
}

fn stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    shared: &Arc<Mutex<Shared>>,
    broken: &Arc<AtomicBool>,
    stats: &Arc<Stats>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let (shared, flag, errors) = (shared.clone(), broken.clone(), stats.clone());
    let mut scratch: Vec<f32> = Vec::with_capacity(8192 * channels.max(1));
    device
        .build_output_stream::<T, _, _>(
            config,
            move |data: &mut [T], _| {
                scratch.resize(data.len(), 0.0);
                let mixed = {
                    let mut guard = locked(&shared);
                    let Shared { mixer, rx } = &mut *guard;
                    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        while let Ok(c) = rx.try_recv() {
                            match c {
                                Command::Voice(v) => {
                                    mixer.add(v);
                                }
                                Command::Stream(s) => {
                                    mixer.add_stream(s);
                                }
                            }
                        }
                        mixer.fill(&mut scratch, channels);
                    }));
                    if ran.is_err() {
                        // Whatever broke the mix goes with the voices.
                        mixer.clear();
                        mixer.stats.panics.fetch_add(1, Ordering::Relaxed);
                    }
                    ran.is_ok()
                };
                if !mixed {
                    scratch.fill(0.0);
                }
                for (o, s) in data.iter_mut().zip(scratch.iter()) {
                    *o = T::from_sample_(*s);
                }
            },
            move |e| {
                errors.errors.fetch_add(1, Ordering::Relaxed);
                note(&format!("stream error: {e}"));
                flag.store(true, Ordering::Relaxed);
            },
            None,
        )
        .context("opening the output stream")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;

    #[test]
    fn a_stopped_voice_is_cut_and_retired() {
        let mut m = Mixer::new();
        let stop = Arc::new(AtomicBool::new(false));
        let mut v = Voice::new(sound(&[[0.5, 0.5]; 100]), (1.0, 1.0));
        v.stop = Some(stop.clone());
        assert!(m.add(v));
        let mut out = vec![0.0; 20];
        m.fill(&mut out, 2);
        assert!(out.iter().any(|s| *s > 0.0), "it plays until stopped");
        stop.store(true, Ordering::Relaxed);
        m.fill(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0), "then it is silent");
        assert_eq!(m.len(), 0, "and gone");
    }

    fn sound(frames: &[[f32; 2]]) -> Arc<[[f32; 2]]> {
        frames.to_vec().into()
    }

    #[test]
    fn voices_add_up_and_a_finished_one_is_retired() {
        let mut m = Mixer::new();
        m.add(Voice::new(sound(&[[0.25, 0.25], [0.25, 0.25]]), (1.0, 1.0)));
        m.add(Voice::new(sound(&[[0.5, 0.0]; 4]), (1.0, 1.0)));
        let mut out = [0.0; 12];
        m.fill(&mut out, 2);
        assert_eq!(&out[..2], &[0.75, 0.25]);
        assert_eq!(&out[2..4], &[0.75, 0.25]);
        assert_eq!(&out[4..6], &[0.5, 0.0], "the short one has ended");
        assert_eq!(m.len(), 0, "both ended within the buffer's six frames");
        // Silence once nothing plays.
        m.fill(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn the_master_volume_scales_the_whole_mix_and_ramps_to_a_new_value() {
        let stats = Arc::new(Stats::default());
        assert_eq!(stats.master(), 1.0, "a mixer starts at full");
        let mut m = Mixer::with_stats(stats.clone(), 48_000);
        m.add(Voice::new(sound(&[[0.5, 0.5]; 400]), (1.0, 1.0)));
        let mut out = [0.0; 20];
        m.fill(&mut out, 2);
        assert!((out[0] - 0.5).abs() < 1.0e-6, "{out:?}");
        // Half the master: this buffer ramps from 1 to 0.5 and the next holds 0.25.
        stats.set_master(0.5);
        m.fill(&mut out, 2);
        assert!(out[0] > 0.45 && out[19] < 0.3, "ramping: {out:?}");
        m.fill(&mut out, 2);
        assert!((out[0] - 0.25).abs() < 1.0e-6 && (out[19] - 0.25).abs() < 1.0e-6);
        // A master that is not a number is full volume, one far too large is capped.
        stats.set_master(f32::NAN);
        assert_eq!(stats.master(), 1.0);
        stats.set_master(100.0);
        assert_eq!(stats.master(), 4.0);
        stats.set_master(-1.0);
        assert_eq!(stats.master(), 0.0);
    }

    #[test]
    fn the_sum_is_clamped() {
        let mut m = Mixer::new();
        for _ in 0..4 {
            m.add(Voice::new(sound(&[[0.9, -0.9]; 3]), (1.0, 1.0)));
        }
        let mut out = [0.0; 6];
        m.fill(&mut out, 2);
        assert_eq!(&out[..2], &[1.0, -1.0]);
    }

    #[test]
    fn a_voice_plays_across_buffers_and_pitch_changes_its_length() {
        let ramp: Vec<[f32; 2]> = (0..8).map(|i| [i as f32 * 0.1, 0.0]).collect();
        let mut m = Mixer::new();
        m.add(Voice::new(sound(&ramp), (1.0, 1.0)));
        let mut a = [0.0; 6];
        m.fill(&mut a, 2);
        assert!((a[4] - 0.2).abs() < 1.0e-6);
        let mut b = [0.0; 6];
        m.fill(&mut b, 2);
        assert!((b[0] - 0.3).abs() < 1.0e-6, "the second buffer carries on");
        // Double pitch: the eight frames take four.
        let mut m = Mixer::new();
        let mut v = Voice::new(sound(&ramp), (1.0, 1.0));
        v.step = 2.0;
        m.add(v);
        let mut out = [0.0; 16];
        m.fill(&mut out, 2);
        assert!((out[2] - 0.2).abs() < 1.0e-6 && (out[6] - 0.6).abs() < 1.0e-6);
        assert_eq!(out[8], 0.0, "ended after four frames");
    }

    #[test]
    fn gains_pan_and_the_channel_layout() {
        let mut m = Mixer::new();
        m.add(Voice::new(sound(&[[1.0, 1.0]; 2]), (0.5, 0.0)));
        let mut out = [0.0; 8];
        m.fill(&mut out, 4);
        assert_eq!(
            &out[..4],
            &[0.5, 0.0, 0.0, 0.0],
            "left only, the other channels silent"
        );
        let mut m = Mixer::new();
        m.add(Voice::new(sound(&[[1.0, 0.5]; 2]), (1.0, 1.0)));
        let mut out = [0.0; 2];
        m.fill(&mut out, 1);
        assert_eq!(out[0], 0.75, "mono is the mean");
    }

    #[test]
    fn a_full_mixer_retires_the_voice_nearest_its_end_for_a_new_one() {
        let mut m = Mixer::new();
        // One short voice among long ones: it is the one that goes.
        assert!(m.add(Voice::new(sound(&[[0.25, 0.25]; 4]), (1.0, 1.0))));
        for _ in 1..MAX_VOICES {
            assert!(m.add(Voice::new(sound(&[[0.0, 0.0]; 4000]), (1.0, 1.0))));
        }
        assert_eq!(m.len(), MAX_VOICES);
        assert!(
            m.add(Voice::new(sound(&[[0.5, 0.5]; 400]), (1.0, 1.0))),
            "a blow is not refused for want of room"
        );
        assert_eq!(m.len(), MAX_VOICES);
        assert_eq!(m.stats.stolen.load(Ordering::Relaxed), 1);
        let mut out = [0.0; 2];
        m.fill(&mut out, 2);
        assert_eq!(out, [0.5, 0.5], "the short one is gone, the new one plays");
    }

    #[test]
    fn a_mixer_full_of_loops_refuses_and_counts() {
        let mut m = Mixer::new();
        for _ in 0..MAX_VOICES {
            let mut v = Voice::new(sound(&[[0.0, 0.0]; 2]), (1.0, 1.0));
            v.looped = true;
            assert!(m.add(v));
        }
        assert!(!m.add(Voice::new(sound(&[[0.0, 0.0]; 2]), (1.0, 1.0))));
        assert_eq!(m.len(), MAX_VOICES);
        assert_eq!(m.stats.refused.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn a_voice_whose_gain_is_not_a_number_is_refused() {
        let mut m = Mixer::new();
        assert!(!m.add(Voice::new(sound(&[[1.0, 1.0]; 4]), (f32::NAN, 1.0))));
        assert!(!m.add(Voice::new(sound(&[[1.0, 1.0]; 4]), (1.0, f32::INFINITY))));
        assert_eq!(m.len(), 0);
        assert_eq!(m.stats.refused.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn a_sample_that_is_not_a_number_never_reaches_the_device() {
        let mut m = Mixer::new();
        // The sound itself holds a NaN (a decoder fault): the buffer is zeroed there, not poisoned. The
        // two frames the NaN touches (it is interpolated into its neighbour) go, the rest plays.
        m.add(Voice::new(
            sound(&[
                [0.5, 0.5],
                [0.5, 0.5],
                [f32::NAN, f32::NAN],
                [0.25, 0.25],
                [0.25, 0.25],
                [0.25, 0.25],
            ]),
            (1.0, 1.0),
        ));
        let mut out = [0.0; 12];
        m.fill(&mut out, 2);
        assert!(out.iter().all(|s| s.is_finite()), "{out:?}");
        assert_eq!(&out[..2], &[0.5, 0.5]);
        assert_eq!(
            &out[2..6],
            &[0.0; 4],
            "the frame before the NaN and the NaN"
        );
        assert_eq!(&out[6..8], &[0.25, 0.25]);
        assert_eq!(m.stats.non_finite.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn a_device_at_another_rate_plays_at_the_same_pitch() {
        let ramp: Vec<[f32; 2]> = (0..8).map(|i| [i as f32 * 0.1, 0.0]).collect();
        // Sounds made for 48 kHz on a 96 kHz device: each frame lasts twice as long.
        let mut m = Mixer::with_stats(Arc::new(Stats::default()), 48_000);
        m.set_device_rate(96_000);
        m.add(Voice::new(sound(&ramp), (1.0, 1.0)));
        let mut out = [0.0; 8]; // four stereo frames cover two source frames
        m.fill(&mut out, 2);
        assert!((out[2] - 0.05).abs() < 1.0e-5 && (out[4] - 0.1).abs() < 1.0e-5);
        // A stream is resampled by the same ratio: 64 chunk frames become about 126, so a hundred
        // output frames are all sound (not 64 and a gap).
        let (tx, rx) = sync_channel(2);
        tx.send(vec![[0.2, 0.2]; 64]).unwrap();
        let mut s = StreamVoice::new(
            rx,
            1.0,
            Level::new(1.0),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        s.gain = 1.0;
        let mut m = Mixer::with_stats(Arc::new(Stats::default()), 48_000);
        m.set_device_rate(96_000);
        m.add_stream(s);
        let mut out = vec![0.0; 2 * 100];
        m.fill(&mut out, 2);
        let sounding = out.chunks(2).filter(|f| f[0] > 0.0).count();
        assert_eq!(sounding, 100);
        drop(tx);
    }

    #[test]
    fn clearing_ends_the_streams_so_their_owners_start_fresh_ones() {
        let (tx, rx) = sync_channel::<Vec<[f32; 2]>>(2);
        tx.send(vec![[0.5, 0.5]; 100]).unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let s = StreamVoice::new(
            rx,
            1.0,
            Level::new(1.0),
            Arc::new(AtomicBool::new(false)),
            done.clone(),
        );
        let mut m = Mixer::new();
        m.add_stream(s);
        m.add(Voice::new(sound(&[[0.5, 0.5]; 100]), (1.0, 1.0)));
        m.clear();
        assert_eq!(m.len(), 0);
        assert!(done.load(Ordering::Relaxed));
        drop(tx);
    }

    #[test]
    fn a_looped_voice_wraps_until_it_is_stopped() {
        let mut m = Mixer::new();
        let stop = Arc::new(AtomicBool::new(false));
        let mut v = Voice::new(sound(&[[0.1, 0.1], [0.2, 0.2], [0.3, 0.3]]), (1.0, 1.0));
        v.looped = true;
        v.stop = Some(stop.clone());
        m.add(v);
        let mut out = [0.0; 14]; // seven stereo frames over a three-frame loop
        m.fill(&mut out, 2);
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        for (i, l) in left.iter().enumerate() {
            assert!(
                (l - [0.1, 0.2, 0.3][i % 3]).abs() < 1.0e-6,
                "frame {i}: {l}"
            );
        }
        assert_eq!(m.len(), 1, "a loop does not end by itself");
        stop.store(true, Ordering::Relaxed);
        m.fill(&mut out, 2);
        assert_eq!(m.len(), 0, "but it ends when stopped");
    }

    #[test]
    fn a_level_ramps_across_the_buffer_instead_of_jumping() {
        let level = Level::new(0.0);
        let mut v = Voice::new(sound(&[[1.0, 1.0]; 64]), (1.0, 1.0));
        v.level = Some(level.clone());
        let mut m = Mixer::new();
        m.add(v);
        let mut out = [0.0; 16]; // eight frames at level 0: the first buffer only arms the ramp
        m.fill(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0), "it starts at its level");
        level.set(1.0);
        m.fill(&mut out, 2);
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        assert!(
            left[0] < 0.01,
            "the ramp opens from the old level: {left:?}"
        );
        assert!(left.windows(2).all(|w| w[1] > w[0]), "and rises steadily");
        assert!(left[7] > 0.8 && left[7] <= 1.0, "to the new one: {left:?}");
    }

    fn stream_of(chunks: Vec<Vec<[f32; 2]>>) -> (StreamVoice, Arc<AtomicBool>, Level) {
        let (tx, rx) = sync_channel(chunks.len().max(1));
        for c in chunks {
            tx.send(c).unwrap();
        }
        drop(tx); // the decoder is finished: the stream ends after its chunks
        let done = Arc::new(AtomicBool::new(false));
        let level = Level::new(1.0);
        let s = StreamVoice::new(
            rx,
            1.0,
            level.clone(),
            Arc::new(AtomicBool::new(false)),
            done.clone(),
        );
        (s, done, level)
    }

    #[test]
    fn a_stream_plays_its_chunks_in_order_and_reports_when_it_is_over() {
        let (s, done, _) = stream_of(vec![vec![[0.1, 0.1], [0.2, 0.2]], vec![[0.3, 0.3]]]);
        let mut m = Mixer::new();
        assert!(m.add_stream(s));
        let mut out = [0.0; 10]; // five frames, three of them sound
        m.fill(&mut out, 2);
        assert_eq!(&out[..6], &[0.1, 0.1, 0.2, 0.2, 0.3, 0.3]);
        assert_eq!(&out[6..], &[0.0; 4]);
        assert!(
            done.load(Ordering::Relaxed),
            "drained and disconnected: done"
        );
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn a_stream_that_waits_for_its_decoder_is_silent_not_finished() {
        let (tx, rx) = sync_channel::<Vec<[f32; 2]>>(2);
        let done = Arc::new(AtomicBool::new(false));
        let s = StreamVoice::new(
            rx,
            1.0,
            Level::new(1.0),
            Arc::new(AtomicBool::new(false)),
            done.clone(),
        );
        let mut m = Mixer::new();
        m.add_stream(s);
        let mut out = [0.0; 8];
        m.fill(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0));
        assert!(
            !done.load(Ordering::Relaxed),
            "the decoder may still deliver"
        );
        tx.send(vec![[0.5, 0.5]; 4]).unwrap();
        m.fill(&mut out, 2);
        assert_eq!(out[0], 0.5, "it plays once the chunk arrives");
    }

    #[test]
    fn a_stopped_stream_is_cut_and_marked_done() {
        let (tx, rx) = sync_channel::<Vec<[f32; 2]>>(2);
        tx.send(vec![[0.5, 0.5]; 100]).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let s = StreamVoice::new(rx, 1.0, Level::new(1.0), stop.clone(), done.clone());
        let mut m = Mixer::new();
        m.add_stream(s);
        let mut out = [0.0; 8];
        m.fill(&mut out, 2);
        assert!(out[0] > 0.0);
        stop.store(true, Ordering::Relaxed);
        m.fill(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0));
        assert!(done.load(Ordering::Relaxed));
        drop(tx);
    }

    #[test]
    fn the_stream_cap_marks_the_extra_one_done() {
        let mut m = Mixer::new();
        for _ in 0..MAX_STREAMS {
            let (s, _, _) = stream_of(vec![vec![[0.0, 0.0]; 4]]);
            assert!(m.add_stream(s));
        }
        let (extra, done, _) = stream_of(vec![vec![[0.0, 0.0]; 4]]);
        assert!(!m.add_stream(extra));
        assert!(
            done.load(Ordering::Relaxed),
            "the owner is not left waiting"
        );
    }
}
