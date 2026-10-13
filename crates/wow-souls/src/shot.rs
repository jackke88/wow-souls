//! `WOWSOULS_SHOT=<png>`: write one frame of the primary window at `WOWSOULS_SHOT_AT` seconds
//! (default 25) and exit two seconds later, since the write is asynchronous. A verification
//! instrument; write the PNG outside the WoW install, which is read-only. With several times
//! (`WOWSOULS_SHOT_AT=6,6.1,6.2`) a frame is written at each, named `<name>-<s>.png`: a gait
//! cycle in one run.
//!
//! `WOWSOULS_SHOT_DEATH=<s>[,<s>...]` instead writes a frame that many seconds after the
//! player's first death (with several, `<name>-<s>.png` for each), for the YOU DIED banner.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

const DEFAULT_AT_SECS: f32 = 25.0;
/// Grace for the async PNG write before exiting.
const WRITE_GRACE_SECS: f32 = 2.0;

pub fn plugin(app: &mut App) {
    if let Ok(path) = std::env::var("WOWSOULS_SHOT") {
        let mut times = std::env::var("WOWSOULS_SHOT_AT")
            .ok()
            .map(|v| parse_offsets(&v))
            .unwrap_or_default();
        if times.is_empty() {
            times.push(DEFAULT_AT_SECS);
        }
        let after_death = std::env::var("WOWSOULS_SHOT_DEATH")
            .ok()
            .map(|v| parse_offsets(&v))
            .unwrap_or_default();
        app.insert_resource(ShotRequest {
            path,
            times,
            after_death,
        })
        .add_systems(Update, shoot_and_exit);
    }
}

#[derive(Resource)]
struct ShotRequest {
    path: String,
    /// Seconds of real time, ascending: one frame at each (several get `<name>-<s>.png`).
    times: Vec<f32>,
    /// Seconds after the first death, ascending; empty for the plain clock.
    after_death: Vec<f32>,
}

/// `0.6, 1.5,3` as sorted seconds; junk and negatives are dropped.
fn parse_offsets(v: &str) -> Vec<f32> {
    let mut out: Vec<f32> = v
        .split(',')
        .filter_map(|p| p.trim().parse::<f32>().ok())
        .filter(|s| s.is_finite() && *s >= 0.0)
        .collect();
    out.sort_by(f32::total_cmp);
    out
}

/// `<stem>-<s>.png` beside `path`.
fn numbered(path: &str, secs: f32) -> String {
    match path.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{secs}.{ext}"),
        None => format!("{path}-{secs}"),
    }
}

/// Where the death-relative shots stand.
#[derive(Default)]
struct DeathShots {
    died: Option<f32>,
    next: usize,
    last: Option<f32>,
}

fn shoot_and_exit(
    time: Res<Time<bevy::time::Real>>,
    req: Res<ShotRequest>,
    death: Res<crate::hurt::Death>,
    mut commands: Commands,
    mut shots: Local<DeathShots>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if !req.after_death.is_empty() {
        shots.died = shots.died.or(death.at);
        let Some(died) = shots.died else {
            return;
        };
        while let Some(&s) = req.after_death.get(shots.next) {
            if now < died + s {
                break;
            }
            let path = if req.after_death.len() > 1 {
                numbered(&req.path, s)
            } else {
                req.path.clone()
            };
            info!("wow-souls: writing {path} ({s} s after the death)");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
            shots.next += 1;
            shots.last = Some(now);
        }
        if shots.next >= req.after_death.len()
            && shots.last.is_some_and(|t| now >= t + WRITE_GRACE_SECS)
        {
            exit.write(AppExit::Success);
        }
        return;
    }
    while let Some(&s) = req.times.get(shots.next) {
        if now < s {
            break;
        }
        let path = if req.times.len() > 1 {
            numbered(&req.path, s)
        } else {
            req.path.clone()
        };
        info!("wow-souls: writing {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        shots.next += 1;
        shots.last = Some(now);
    }
    if shots.next >= req.times.len() && shots.last.is_some_and(|t| now >= t + WRITE_GRACE_SECS) {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_are_sorted_and_junk_is_dropped() {
        assert_eq!(parse_offsets("1.5, 0.6,3"), vec![0.6, 1.5, 3.0]);
        assert_eq!(parse_offsets("x,-1,2"), vec![2.0]);
        assert!(parse_offsets("").is_empty());
    }

    #[test]
    fn several_shots_get_numbered_names() {
        assert_eq!(numbered(r"C:\died.png", 1.5), r"C:\died-1.5.png");
        assert_eq!(numbered("died", 3.0), "died-3");
    }
}
