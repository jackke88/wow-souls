//! Zone music and ambience, WoW's own, streamed from the install. The area under the player (the MCNK area
//! id the world already tracks) names a `ZoneMusic` row and a `SoundAmbience` row through `AreaTable`
//! (a subzone without its own takes its zone's); each holds a sound kit for day and one for night.
//!
//! Music: a zone change starts its track at once and fades the old one out over 4 s. WoW's option "Loop
//! Music" (`SoundZoneMusicNoDelay`, "Enable to continuously play background music") is on here: a track
//! that ends is followed by the next of the zone's tracks at once (a different one when the zone has
//! another), so the music never stops. With the option off (`WOWSOULS_MUSIC_LOOP=0`) a track is followed
//! by a silence, uniform in the row's range for the phase, then the next track (WoW's default: the music
//! comes and goes, it is not a wall of sound). Ambience: the bed loops and an area or day / night change
//! crossfades over 5 s. Day is 05:30 to 21:00 on the game clock.
//!
//! `WOWSOULS_MUSIC=<0..1>` and `WOWSOULS_AMBIENCE=<0..1>` set their loudness under the master (`0` turns
//! it off). The rules follow benilla-app's `sound/zone.rs` (docs/PROVENANCE.md), without the intro
//! fanfares, interiors, weather, ghost or underwater beds.

use bevy::prelude::*;
use bevy::time::Real;

use benilla_world::lighting::GameClock;
use benilla_world::wmo_portal::CameraInteriorClaim;
use benilla_world::world_point::WorldPoint;

use crate::audio::{Audio, Playing, ZoneChoice};
use crate::settle::Phase;
use crate::weather::WeatherAmbience;

/// How long the outgoing music takes to fade (s), and the ambience crossfade (both legs).
const MUSIC_FADE_OUT: f64 = 4.0;
const AMBIENCE_FADE: f64 = 5.0;
/// After a track or bed that could not start, how long before the next try (s).
const RETRY: f64 = 5.0;
/// Draws for a track other than the one that just played, when the zone has more than one.
const FRESH_TRACK_TRIES: usize = 4;
/// A track that is over sooner than this after it started did not play (s).
const MIN_TRACK_SECS: f64 = 1.0;

/// `[day, night]` slot of a game-clock minute: day is 05:30 up to 21:00.
pub fn phase(minute: u32) -> usize {
    if (330..1260).contains(&minute) {
        0
    } else {
        1
    }
}

/// The silence after a track, in seconds: `roll` draws uniformly in `[min_ms, max_ms]` (a reversed range
/// reads as `min_ms`).
pub fn silence_secs(min_ms: u32, max_ms: u32, roll: u32) -> f64 {
    let span = u64::from(max_ms.saturating_sub(min_ms)) + 1;
    (u64::from(min_ms) + u64::from(roll) % span) as f64 / 1000.0
}

/// Seconds until the next track after one that has played for `played` seconds: none when the music
/// loops, `silence` (the row's draw) when it does not, and [`RETRY`] for a track that was over at once
/// (the mixer refused it, the file would not decode), which is no loop to spin on every frame.
pub fn gap_after(looped: bool, played: f64, silence: f64) -> f64 {
    if played < MIN_TRACK_SECS {
        RETRY
    } else if looped {
        0.0
    } else {
        silence
    }
}

/// The level a fade started at `start` has reached at `now` going down (1 -> 0 over `secs`).
pub fn fade_out(now: f64, start: f64, secs: f64) -> f32 {
    (1.0 - (now - start) / secs).clamp(0.0, 1.0) as f32
}

/// The level a fade started at `start` has reached at `now` going up (0 -> 1 over `secs`).
pub fn fade_in(now: f64, start: f64, secs: f64) -> f32 {
    ((now - start) / secs).clamp(0.0, 1.0) as f32
}

/// A sound on its way out.
struct Fading {
    playing: Playing,
    start: f64,
    secs: f64,
    /// The loudness it fades from (the player's music or ambience level when it began to go).
    from: f32,
}

#[derive(Default)]
struct ZoneState {
    area: Option<u32>,
    choice: Option<ZoneChoice>,
    phase: usize,
    music_row: u32,
    track: Option<Playing>,
    /// When the next track may start (real s).
    next_track: f64,
    /// The track that played last, so a loop moves on to another when there is one.
    last_track: Option<String>,
    /// When the current track started (real s).
    track_since: f64,
    out: Vec<Fading>,
    bed_kit: u32,
    bed: Option<(Playing, f64)>,
    /// When a bed that failed to start is tried again (real s).
    next_bed: f64,
}

/// The player's loudness of the zone's sounds, under the master volume: the settings (the options menu
/// moves them while a track plays), `WOWSOULS_MUSIC` / `_AMBIENCE` / `_MUSIC_LOOP` over the file.
#[derive(Resource, Clone, Copy)]
struct Loudness {
    music: f32,
    ambience: f32,
    /// "Loop Music": no silence between tracks.
    looped: bool,
}

fn sync_loudness(settings: Res<crate::settings::Settings>, mut loud: ResMut<Loudness>) {
    let s = &settings.sound;
    let want = Loudness {
        music: s.music,
        ambience: s.ambience,
        looped: s.loop_music,
    };
    if loud.music != want.music || loud.ambience != want.ambience || loud.looped != want.looped {
        *loud = want;
    }
}

#[allow(clippy::too_many_arguments)]
fn zone_audio(
    mut st: Local<ZoneState>,
    loud: Res<Loudness>,
    real: Res<Time<Real>>,
    clock: Option<Res<GameClock>>,
    weather: Res<WeatherAmbience>,
    interior: Option<Res<CameraInteriorClaim>>,
    world: WorldPoint,
    audio: Option<ResMut<Audio>>,
) {
    let Some(mut audio) = audio else {
        return;
    };
    let now = real.elapsed_secs_f64();
    let phase = clock.map_or(0, |c| phase(c.minute));

    // Fades first: they run whatever the area says.
    st.out.retain(|f| {
        let level = fade_out(now, f.start, f.secs);
        f.playing.set_level(level * f.from);
        if level <= 0.0 {
            f.playing.stop();
        }
        level > 0.0
    });
    if let Some((bed, since)) = &st.bed {
        bed.set_level(fade_in(now, *since, AMBIENCE_FADE) * loud.ambience);
    }
    // A track plays at the player's music level, moved while it plays.
    if let Some(track) = &st.track {
        track.set_level(loud.music);
    }

    // Where the player is: until the ground is resident the area is unknown and nothing changes.
    let area = world.area();
    if area != st.area || phase != st.phase {
        if area != st.area {
            st.choice = area.and_then(|a| audio.zone_choice(a));
            if let Some(c) = &st.choice {
                info!(
                    "zone: {} (music row {}, ambience row {}, {})",
                    c.name,
                    c.music_row,
                    c.ambience_row,
                    if phase == 0 { "day" } else { "night" }
                );
            }
        }
        st.area = area;
        st.phase = phase;
    }
    let Some(choice) = st.choice.clone() else {
        return;
    };

    // Music.
    if loud.music > 0.0 {
        if choice.music_row != st.music_row {
            if let Some(old) = st.track.take() {
                st.out.push(Fading {
                    playing: old,
                    start: now,
                    secs: MUSIC_FADE_OUT,
                    from: loud.music,
                });
            }
            st.music_row = choice.music_row;
            st.next_track = now; // the new zone's track opens at once
        }
        if st.track.as_ref().is_some_and(Playing::is_done) {
            st.track = None;
            let played = now - st.track_since;
            let roll = audio.roll();
            let silence = silence_secs(
                choice.silence_min_ms[phase],
                choice.silence_max_ms[phase],
                roll,
            );
            let gap = gap_after(loud.looped, played, silence);
            st.next_track = now + gap;
            if played < MIN_TRACK_SECS {
                info!("zone music: the track ended at once, trying again in {RETRY:.0} s");
            } else if loud.looped {
                info!("zone music: the track ended, the next one follows");
            } else {
                info!("zone music: the track ended, {gap:.0} s of silence");
            }
        }
        if st.track.is_none() && choice.music_row != 0 && now >= st.next_track {
            // A loop moves on to another track when the zone has one.
            let mut pick = audio.pick_file(choice.music_kits[phase]);
            for _ in 1..FRESH_TRACK_TRIES {
                if pick.as_ref().map(|p| &p.0) != st.last_track.as_ref() {
                    break;
                }
                pick = audio.pick_file(choice.music_kits[phase]);
            }
            let started = pick.and_then(|(path, volume)| {
                audio
                    .stream_path(&path, volume, loud.music, "zone music")
                    .map(|p| (path, p))
            });
            match started {
                Some((path, playing)) => {
                    info!("zone music: {} plays {path}", choice.name);
                    st.track = Some(playing);
                    st.track_since = now;
                    st.last_track = Some(path);
                }
                None => st.next_track = now + RETRY,
            }
        }
    }

    // Ambience.
    // The weather's loop replaces the bed while the player is outdoors, as the server's selector
    // does (`0x460bd0`); indoors the area's own bed returns.
    let indoors = interior.is_some_and(|claim| claim.0.is_some());
    let kit = if weather.0 != 0 && !indoors {
        weather.0
    } else {
        choice.ambience_kits[phase]
    };
    if loud.ambience > 0.0 {
        if kit != st.bed_kit {
            if let Some((old, _)) = st.bed.take() {
                st.out.push(Fading {
                    playing: old,
                    start: now,
                    secs: AMBIENCE_FADE,
                    from: loud.ambience,
                });
            }
            st.bed_kit = kit;
            st.next_bed = now;
        }
        if st.bed.is_none() && kit != 0 && now >= st.next_bed {
            let started = audio.pick_file(kit).and_then(|(path, volume)| {
                audio
                    .loop_path(&path, volume, 0.0, "ambience")
                    .map(|p| (path, p))
            });
            match started {
                Some((path, playing)) => {
                    info!("zone ambience: {} loops {path}", choice.name);
                    st.bed = Some((playing, now));
                }
                None => st.next_bed = now + RETRY,
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    let s = &crate::settings::initial().sound;
    let loud = Loudness {
        music: s.music,
        ambience: s.ambience,
        looped: s.loop_music,
    };
    app.insert_resource(loud)
        .add_systems(Update, sync_loudness)
        // The valley's music and ambience play behind the menu too.
        .add_systems(
            Update,
            zone_audio
                .after(sync_loudness)
                .run_if(in_state(Phase::Playing).or(in_state(Phase::Menu))),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_is_half_past_five_to_nine_in_the_evening() {
        assert_eq!(phase(329), 1);
        assert_eq!(phase(330), 0, "05:30 is day");
        assert_eq!(phase(720), 0);
        assert_eq!(phase(1259), 0);
        assert_eq!(phase(1260), 1, "21:00 is night");
        assert_eq!(phase(0), 1);
    }

    #[test]
    fn a_looping_track_is_followed_at_once_and_a_plain_one_by_the_rows_silence() {
        assert_eq!(gap_after(true, 180.0, 60.0), 0.0, "Loop Music: no silence");
        assert_eq!(gap_after(false, 180.0, 60.0), 60.0, "WoW's default");
        // A track that never played is not retried every frame, loop or not.
        assert_eq!(gap_after(true, 0.1, 60.0), RETRY);
        assert_eq!(gap_after(false, 0.1, 60.0), RETRY);
    }

    #[test]
    fn the_silence_between_tracks_is_uniform_in_the_rows_range() {
        assert_eq!(silence_secs(60_000, 60_000, 12345), 60.0);
        for roll in [0, 1, 9_999, 123_456_789, u32::MAX] {
            let s = silence_secs(30_000, 90_000, roll);
            assert!((30.0..=90.0).contains(&s), "{roll} gave {s}");
        }
        assert_eq!(silence_secs(30_000, 90_000, 0), 30.0);
        assert_eq!(
            silence_secs(90_000, 30_000, 7),
            90.0,
            "a reversed range reads as its minimum"
        );
    }

    #[test]
    fn fades_run_linearly_and_hold_at_their_ends() {
        assert_eq!(fade_out(10.0, 10.0, 4.0), 1.0);
        assert_eq!(fade_out(12.0, 10.0, 4.0), 0.5);
        assert_eq!(fade_out(20.0, 10.0, 4.0), 0.0);
        assert_eq!(fade_in(10.0, 10.0, 5.0), 0.0);
        assert!((fade_in(12.5, 10.0, 5.0) - 0.5).abs() < 1.0e-6);
        assert_eq!(fade_in(99.0, 10.0, 5.0), 1.0);
    }
}
