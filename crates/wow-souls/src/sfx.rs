//! Sound data and the arithmetic of one play: decoding a WoW `.wav` to stereo at the output
//! device's rate, picking a kit's variation by weight, a source's gain and pan, the impact slot.
//! Pure: no device, no ECS. The rules follow benilla-app's `sound/combat.rs` and `kit.rs`.

use std::io::Cursor;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use benilla_formats::impact_slot;
use bevy::math::Vec3;
use symphonia::core::audio::conv::{FromSample, IntoSample};
use symphonia::core::audio::sample::Sample;
use symphonia::core::audio::{Audio, AudioBuffer, GenericAudioBufferRef};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::TrackType;
use symphonia::core::io::MediaSourceStream;

/// A decoded sound: stereo frames at the output device's sample rate.
#[derive(Clone)]
pub struct Sound {
    pub frames: Arc<[[f32; 2]]>,
}

impl Sound {
    pub fn len(&self) -> usize {
        self.frames.len()
    }
}

fn frames_of<S: Sample>(buffer: &AudioBuffer<S>) -> Result<Vec<[f32; 2]>>
where
    f32: FromSample<S>,
{
    match buffer.num_planes() {
        1 => Ok(buffer
            .plane(0)
            .context("plane 0")?
            .iter()
            .map(|s| {
                let m: f32 = (*s).into_sample();
                [m, m]
            })
            .collect()),
        2 => Ok(buffer
            .plane(0)
            .context("plane 0")?
            .iter()
            .zip(buffer.plane(1).context("plane 1")?.iter())
            .map(|(l, r)| [(*l).into_sample(), (*r).into_sample()])
            .collect()),
        n => bail!("{n} channels: only mono and stereo are played"),
    }
}

pub(crate) fn frames_of_ref(buffer: &GenericAudioBufferRef) -> Result<Vec<[f32; 2]>> {
    match buffer {
        GenericAudioBufferRef::U8(b) => frames_of(b),
        GenericAudioBufferRef::U16(b) => frames_of(b),
        GenericAudioBufferRef::U24(b) => frames_of(b),
        GenericAudioBufferRef::U32(b) => frames_of(b),
        GenericAudioBufferRef::S8(b) => frames_of(b),
        GenericAudioBufferRef::S16(b) => frames_of(b),
        GenericAudioBufferRef::S24(b) => frames_of(b),
        GenericAudioBufferRef::S32(b) => frames_of(b),
        GenericAudioBufferRef::F32(b) => frames_of(b),
        GenericAudioBufferRef::F64(b) => frames_of(b),
    }
}

/// Decodes a `.wav` file's bytes to stereo at `rate` (mono is both channels; the sample rate is
/// converted by linear interpolation).
pub fn decode(bytes: Vec<u8>, rate: u32) -> Result<Sound> {
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
    let mut frames = Vec::new();
    while let Some(packet) = reader.next_packet()? {
        if packet.track_id == id {
            frames.append(&mut frames_of_ref(&decoder.decode(&packet)?)?);
        }
    }
    if frames.is_empty() {
        bail!("no samples");
    }
    Ok(Sound {
        frames: resample(&frames, from, rate).into(),
    })
}

/// `frames` at `from` Hz as frames at `to` Hz, linear.
pub fn resample(frames: &[[f32; 2]], from: u32, to: u32) -> Vec<[f32; 2]> {
    if from == to || frames.is_empty() || from == 0 || to == 0 {
        return frames.to_vec();
    }
    let n = ((frames.len() as u64 * u64::from(to)) / u64::from(from)).max(1) as usize;
    let step = f64::from(from) / f64::from(to);
    (0..n)
        .map(|i| {
            let p = i as f64 * step;
            let k = (p as usize).min(frames.len() - 1);
            let a = (p - k as f64) as f32;
            let (x, y) = (frames[k], frames[(k + 1).min(frames.len() - 1)]);
            [x[0] + (y[0] - x[0]) * a, x[1] + (y[1] - x[1]) * a]
        })
        .collect()
}

/// The variation of a kit to play: `files` are `(path, Freq weight)`, `roll` any 32-bit draw. A kit
/// whose weights are all 0 picks evenly.
pub fn pick_variation(files: &[(String, u32)], roll: u32) -> Option<usize> {
    if files.is_empty() {
        return None;
    }
    let total: u64 = files.iter().map(|f| u64::from(f.1)).sum();
    if total == 0 {
        return Some((u64::from(roll) % files.len() as u64) as usize);
    }
    let mut at = u64::from(roll) % total;
    for (i, f) in files.iter().enumerate() {
        if at < u64::from(f.1) {
            return Some(i);
        }
        at -= u64::from(f.1);
    }
    None
}

/// The class chance roll (reference `0x623520`): `r = 101 · roll >> 32` in 0..=100, a pass when
/// `threshold >= r`, so P = (threshold + 1) / 101.
pub fn chance(threshold: u32, roll: u32) -> bool {
    ((101u64 * u64::from(roll)) >> 32) as u32 <= threshold
}

/// The gains `(left, right)` of a sound at `dist` yd from the listener: the kit's `volume` times
/// 1 inside `min`, falling linearly to 0 at `cutoff` (beyond it, `None`: inaudible; a cutoff of 0
/// is non-positional, no fall-off), spread by `pan` (-1 left, 1 right) at constant power.
///
/// A source at a position that is not a number (a transform gone to NaN) is inaudible, not a NaN
/// in the mix.
pub fn gains(volume: f32, dist: f32, min: f32, cutoff: f32, pan: f32) -> Option<(f32, f32)> {
    if ![volume, dist, min, cutoff, pan]
        .iter()
        .all(|v| v.is_finite())
    {
        return None;
    }
    let fall = if cutoff <= 0.0 {
        1.0
    } else if dist >= cutoff {
        return None;
    } else if dist <= min {
        1.0
    } else {
        1.0 - (dist - min) / (cutoff - min).max(1.0e-3)
    };
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    let g = volume * fall;
    Some((g * a.cos(), g * a.sin()))
}

/// The pan of a source in direction `dir` from a listener whose right hand is `right`.
pub fn pan_of(right: Vec3, dir: Vec3) -> f32 {
    let d = dir.normalize_or_zero();
    right.normalize_or_zero().dot(d).clamp(-1.0, 1.0)
}

/// A creature victim's `WeaponImpactSounds` slot from its `CreatureSoundData.impact_type`: flesh,
/// stone, wood, ethereal.
pub fn creature_impact_slot(impact_type: u32) -> usize {
    match impact_type {
        1 => impact_slot::STONE,
        2 => impact_slot::WOOD,
        3 => impact_slot::ETHEREAL,
        _ => impact_slot::FLESH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PCM16 `.wav` of `channels` interleaved channels.
    fn wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&channels.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        b.extend_from_slice(&(channels * 2).to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&data);
        b
    }

    #[test]
    fn mono_is_both_channels_and_stereo_stays_two() {
        let mono = decode(wav(8000, 1, &[16384, -16384, 0, 8192]), 8000).unwrap();
        assert_eq!(mono.len(), 4);
        assert!((mono.frames[0][0] - 0.5).abs() < 1.0e-3 && mono.frames[0][0] == mono.frames[0][1]);
        assert!((mono.frames[1][1] + 0.5).abs() < 1.0e-3);
        let stereo = decode(wav(8000, 2, &[16384, -16384, 0, 32767]), 8000).unwrap();
        assert_eq!(stereo.len(), 2);
        assert!(
            (stereo.frames[0][0] - 0.5).abs() < 1.0e-3
                && (stereo.frames[0][1] + 0.5).abs() < 1.0e-3
        );
    }

    #[test]
    fn decoding_converts_the_rate_and_refuses_junk() {
        // 100 frames at 8 kHz are 600 at 48 kHz.
        let s = decode(wav(8000, 1, &[1000; 100]), 48_000).unwrap();
        assert_eq!(s.len(), 600);
        assert!(decode(vec![1, 2, 3, 4, 5, 6, 7, 8], 48_000).is_err());
        let mut cut = wav(8000, 1, &[1000; 100]);
        cut.truncate(30);
        assert!(
            decode(cut, 48_000).is_err(),
            "a header without samples is no sound"
        );
    }

    #[test]
    fn resampling_interpolates() {
        let r = resample(&[[0.0, 0.0], [1.0, -1.0]], 1, 2);
        assert_eq!(r.len(), 4);
        assert!((r[1][0] - 0.5).abs() < 1.0e-6 && (r[1][1] + 0.5).abs() < 1.0e-6);
        assert_eq!(resample(&[[0.3, 0.3]], 48_000, 48_000), vec![[0.3, 0.3]]);
    }

    #[test]
    fn a_variation_is_picked_by_weight() {
        let files = vec![("a".to_string(), 1), ("b".to_string(), 3)];
        let n = |k: usize| {
            (0..4000u32)
                .filter(|&r| pick_variation(&files, r.wrapping_mul(1_000_003)) == Some(k))
                .count()
        };
        assert!(n(1) > 2 * n(0), "{} vs {}", n(1), n(0));
        assert_eq!(pick_variation(&[], 5), None);
        let flat = vec![("a".to_string(), 0), ("b".to_string(), 0)];
        assert_eq!(pick_variation(&flat, 3), Some(1));
        assert_eq!(pick_variation(&flat, 4), Some(0));
    }

    #[test]
    fn a_class_chance_is_its_threshold_over_101() {
        let passes = (0..=1000u32)
            .filter(|&i| chance(70, (u64::from(i) * 4_294_967u64) as u32))
            .count();
        assert!((680..=740).contains(&passes), "{passes} of 1001 at 70 %");
        assert!(chance(100, u32::MAX) && !chance(0, u32::MAX));
    }

    #[test]
    fn gain_falls_from_the_min_distance_to_the_cutoff() {
        let g = |d| gains(1.0, d, 8.0, 45.0, 0.0).map(|(l, r)| l.hypot(r));
        assert!((g(4.0).unwrap() - 1.0).abs() < 1.0e-5);
        assert!((g(26.5).unwrap() - 0.5).abs() < 1.0e-4);
        assert_eq!(g(45.0), None);
        // Non-positional kit: no fall-off at any distance.
        assert!(
            (gains(0.8, 999.0, 0.0, 0.0, 0.0)
                .map(|(l, r)| l.hypot(r))
                .unwrap()
                - 0.8)
                .abs()
                < 1.0e-5
        );
        // A distance that is not a number would have made the gain one (every comparison with NaN is
        // false, so it fell through to the fall-off formula).
        assert_eq!(gains(1.0, f32::NAN, 8.0, 45.0, 0.0), None);
        assert_eq!(gains(f32::NAN, 1.0, 8.0, 45.0, 0.0), None);
        assert_eq!(gains(1.0, 1.0, 8.0, 45.0, f32::INFINITY), None);
        // Pan: hard left puts it all in the left channel, centre splits evenly at constant power.
        let (l, r) = gains(1.0, 0.0, 1.0, 10.0, -1.0).unwrap();
        assert!(l > 0.999 && r < 1.0e-3);
        let (l, r) = gains(1.0, 0.0, 1.0, 10.0, 0.0).unwrap();
        assert!((l - r).abs() < 1.0e-6 && (l.hypot(r) - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn pan_is_the_direction_against_the_listeners_right() {
        assert!((pan_of(Vec3::X, Vec3::new(3.0, 0.0, 0.0)) - 1.0).abs() < 1.0e-6);
        assert!((pan_of(Vec3::X, Vec3::NEG_X) + 1.0).abs() < 1.0e-6);
        assert!(pan_of(Vec3::X, Vec3::Z).abs() < 1.0e-6);
        assert_eq!(pan_of(Vec3::X, Vec3::ZERO), 0.0);
    }

    #[test]
    fn creature_materials_map_to_the_four_creature_slots() {
        assert_eq!(creature_impact_slot(0), impact_slot::FLESH);
        assert_eq!(creature_impact_slot(1), impact_slot::STONE);
        assert_eq!(creature_impact_slot(2), impact_slot::WOOD);
        assert_eq!(creature_impact_slot(3), impact_slot::ETHEREAL);
        assert_eq!(creature_impact_slot(9), impact_slot::FLESH);
    }
}
