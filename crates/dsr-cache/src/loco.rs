//! The player's locomotion: which animation file is the walk, the run, the sprint and the
//! locked-on steps of each weapon motion set, and `wow-souls-dsr loco [--set a00]`, the measuring
//! that told them apart (the files carry no names).
//!
//! The common `a00` set holds the unarmed / one-handed gait; every other weapon motion set that
//! carries the body differently (`a10` two-handed small weapons, `a12` great weapons over the
//! shoulder, `a13` polearms, `a14` bow, `a16` crossbow) holds its own copy of the same clips with
//! the same timing and root travel, posed for its weapon: `<set>_0200` walk 1.50 m/s, `0300-0303`
//! locked-on walk (forward, back, left, right) 1.62 m/s, `0500` run 3.67 m/s, `0550-0553` locked-on
//! run 3.38 m/s, `0510` sprint 5.03 m/s (`a00_0535` is a faster one, 6.33 m/s, in the common set
//! only). All of them loop (the first and last frames' worst bone differs by under 1.5 degrees).
//!
//! `loco` (read-only) prints, for every animation file of the set (all sets without `--set`) that
//! carries no hit window and no invulnerability window: its length, the root's travel along the
//! body's facing and across it, the pace that travel makes, whether the loop closes and the
//! `ChrActionFlag` windows it carries; `--clips a00_0500,...` prints every TAE event instead.

use std::path::Path;

use anyhow::Result;
use dsr_formats::hkx::skeleton::Qs;
use glam::{Quat, Vec3};

use crate::dsr::{self, Player};
use crate::retarget::ds_forward;

/// A locomotion role and the animation ids that fill it in a weapon motion set (the first the
/// set has; a set that has none of them uses the common `a00` set's).
pub struct Role {
    /// The name in `locomotion.txt` and the game.
    pub name: &'static str,
    pub ids: &'static [u32],
}

const fn role(name: &'static str, ids: &'static [u32]) -> Role {
    Role { name, ids }
}

/// The eleven roles of one weapon motion set. The free ones are forward only (the body turns to
/// its travel); the locked-on ones run in four directions (the body keeps facing its target).
pub const ROLES: [Role; 11] = [
    role("walk", &[200]),
    role("run", &[500]),
    role("sprint", &[535, 510]),
    role("lock_walk_f", &[300]),
    role("lock_walk_b", &[301]),
    role("lock_walk_l", &[302]),
    role("lock_walk_r", &[303]),
    role("lock_run_f", &[550]),
    role("lock_run_b", &[551]),
    role("lock_run_l", &[552]),
    role("lock_run_r", &[553]),
];

/// The clip that fills `role` for motion set `set`: the set's own first, else the common set's.
pub fn clip_for(player: &Player, set: i64, role: &Role) -> Option<String> {
    for set in [set, 0] {
        for id in role.ids {
            let name = format!("a{set:02}_{id:04}");
            let known = player
                .resolve(&name, false)
                .is_ok_and(|resolved| player.hkx.contains_key(&resolved));
            if known {
                return Some(name);
            }
        }
    }
    None
}

/// One locomotion candidate, measured.
#[derive(Debug, Clone)]
pub struct Measured {
    pub name: String,
    pub frames: usize,
    /// Seconds from the first frame to the last.
    pub duration: f32,
    /// Root travel along the skeleton's facing / across it (DS metres): `forward` is positive
    /// ahead, `side` positive to the body's left.
    pub forward: f32,
    pub side: f32,
    pub up: f32,
    pub yaw_deg: f32,
    /// The worst bone rotation difference (deg) between the first and the last frame.
    pub seam_deg: f32,
}

impl Measured {
    /// Metres per second along the facing (negative when the clip walks backwards).
    pub fn pace(&self) -> f32 {
        if self.duration > 0.0 {
            self.forward / self.duration
        } else {
            0.0
        }
    }
}

fn angle_between(a: &Qs, b: &Qs) -> f32 {
    let qa = Quat::from_array(a.r).normalize();
    let qb = Quat::from_array(b.r).normalize();
    qa.angle_between(qb).to_degrees()
}

/// Measures one clip of the player.
pub fn measure(player: &Player, name: &str, fwd: Vec3, left: Vec3) -> Result<Measured> {
    let clip = dsr::decode(player, name)?;
    let first = clip.root.first().copied().unwrap_or([0.0; 4]);
    let last = clip.root.last().copied().unwrap_or([0.0; 4]);
    let d = Vec3::new(last[0] - first[0], last[1] - first[1], last[2] - first[2]);
    let up = Vec3::from_array(clip.up).normalize_or_zero();
    let flat = d - up * d.dot(up);
    let seam = match (clip.frames.first(), clip.frames.last()) {
        (Some(a), Some(b)) => a
            .iter()
            .zip(b)
            .map(|(x, y)| angle_between(x, y))
            .fold(0.0, f32::max),
        _ => 0.0,
    };
    Ok(Measured {
        name: name.to_string(),
        frames: clip.frames.len(),
        duration: clip.frames.len().saturating_sub(1) as f32 / clip.fps,
        forward: flat.dot(fwd),
        side: flat.dot(left),
        up: d.dot(up),
        yaw_deg: (last[3] - first[3]).to_degrees(),
        seam_deg: seam,
    })
}

/// The skeleton's facing and its left, for `measure`.
pub fn axes(player: &Player) -> Result<(Vec3, Vec3)> {
    let up = Vec3::Y;
    let fwd = ds_forward(&player.skeleton, up, &crate::bonemap::C0000.facing)?;
    // Left from the shoulders, as the retarget's basis does.
    let bind = crate::retarget::model_space(&player.skeleton.parents, &player.skeleton.reference)?;
    let names = &crate::bonemap::C0000;
    let l = player.skeleton.bone(names.left).map(|i| bind[i].0);
    let r = player.skeleton.bone(names.right).map(|i| bind[i].0);
    let left = match (l, r) {
        (Some(l), Some(r)) => {
            let d = l - r;
            (d - up * d.dot(up)).normalize_or_zero()
        }
        _ => up.cross(fwd),
    };
    Ok((fwd, left))
}

/// Every TAE event of the named clips: kind, window and the first parameters.
pub fn events(player: &Player, clips: &[String]) {
    for name in clips {
        println!("{name}:");
        for e in player.events(name) {
            let p: Vec<String> = (0..6)
                .filter_map(|i| e.param_i32(i).map(|v| v.to_string()))
                .collect();
            println!(
                "    kind {:>3} {:.2}-{:.2}s params [{}]",
                e.kind,
                e.start,
                e.end,
                p.join(" ")
            );
        }
    }
}

pub fn run(dsr_dir: &Path, set: Option<&str>, clips: &[String]) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    if !clips.is_empty() {
        events(&player, clips);
        return Ok(());
    }
    let (fwd, left) = axes(&player)?;
    println!(
        "facing {:+.2} {:+.2} {:+.2}, left {:+.2} {:+.2} {:+.2}",
        fwd.x, fwd.y, fwd.z, left.x, left.y, left.z
    );
    println!(
        "{:<10} {:>5} {:>6} {:>7} {:>7} {:>6} {:>7} {:>6} {:>6}  flags",
        "clip", "frm", "dur s", "fwd m", "side m", "up m", "m/s", "yaw", "seam"
    );
    for name in player.hkx.keys() {
        if set.is_some_and(|s| !name.starts_with(&format!("{s}_"))) {
            continue;
        }
        if !player.hits(name).is_empty() {
            continue;
        }
        let flags = player.flags(name);
        if flags.iter().any(|w| w.flag == wscl::FLAG_INVINCIBLE) {
            continue;
        }
        let m = match measure(&player, name, fwd, left) {
            Ok(m) => m,
            Err(e) => {
                println!("{name}: cannot decode: {e:#}");
                continue;
            }
        };
        let flags: Vec<String> = flags
            .iter()
            .map(|w| format!("{}@{:.2}-{:.2}", w.flag, w.start, w.end))
            .collect();
        println!(
            "{:<10} {:>5} {:>6.2} {:>+7.2} {:>+7.2} {:>+6.2} {:>+7.2} {:>+6.0} {:>6.1}  {}",
            m.name,
            m.frames,
            m.duration,
            m.forward,
            m.side,
            m.up,
            m.pace(),
            m.yaw_deg,
            m.seam_deg,
            flags.join(" ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_roles_have_distinct_names_and_ids() {
        let mut names: Vec<_> = ROLES.iter().map(|r| r.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ROLES.len());
        let mut ids: Vec<u32> = ROLES.iter().flat_map(|r| r.ids.iter().copied()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(
            ids.len(),
            ROLES.iter().map(|r| r.ids.len()).sum::<usize>(),
            "an animation id fills one role"
        );
    }

    #[test]
    fn install_every_weapon_set_fills_every_role_and_the_clips_are_gaits() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let player = dsr::load(Path::new(&dir)).unwrap();
        let (fwd, left) = axes(&player).unwrap();
        for set in [0, 10, 12, 13, 14, 16] {
            for role in &ROLES {
                let name = clip_for(&player, set, role)
                    .unwrap_or_else(|| panic!("a{set:02} has no {}", role.name));
                // The set's own clip where it has one: the common set's only when it has none.
                assert!(
                    name.starts_with(&format!("a{set:02}_"))
                        || set == 0
                        || name.starts_with("a00_"),
                    "{name}"
                );
                let m = measure(&player, &name, fwd, left).unwrap();
                assert!(
                    m.duration > 0.5 && m.duration < 2.0,
                    "{name}: {}",
                    m.duration
                );
                // Each role travels the way its name says, at the pace of its kind.
                let (along, across) = (m.forward / m.duration, m.side / m.duration);
                match role.name {
                    "walk" | "lock_walk_f" => {
                        assert!((1.3..1.8).contains(&along), "{name}: {along}")
                    }
                    "run" | "lock_run_f" => assert!((3.2..4.0).contains(&along), "{name}: {along}"),
                    "sprint" => assert!((4.5..6.8).contains(&along), "{name}: {along}"),
                    "lock_walk_b" | "lock_run_b" => assert!(along < -1.3, "{name}: {along}"),
                    n if n.ends_with("_l") => assert!(across > 1.3, "{name}: {across}"),
                    n if n.ends_with("_r") => assert!(across < -1.3, "{name}: {across}"),
                    _ => unreachable!("{}", role.name),
                }
                // None of them is an attack, a roll or a flinch: no hit, no invulnerability.
                assert!(player.hits(&name).is_empty(), "{name}");
            }
        }
        // The sprint of the common set is its own, faster clip.
        let sprint = ROLES.iter().find(|r| r.name == "sprint").unwrap();
        assert_eq!(clip_for(&player, 0, sprint).as_deref(), Some("a00_0535"));
        assert_eq!(clip_for(&player, 12, sprint).as_deref(), Some("a12_0510"));
        // A set the install does not have borrows the common one.
        let walk = ROLES.iter().find(|r| r.name == "walk").unwrap();
        assert_eq!(clip_for(&player, 99, walk).as_deref(), Some("a00_0200"));
    }
}
