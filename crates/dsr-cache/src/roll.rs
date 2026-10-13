//! Choosing the medium forward roll from data: forward root motion, under 3 s, an
//! invulnerability window of 11 frames at 30 fps. DSR's three roll weight classes
//! (`a00_070x` fast, `a00_071x` medium, `a00_072x` fat) all have 11 invulnerable frames and
//! differ by when their anim-cancel windows open (0.8 s, 1.1 s, 1.6 s). The measured medium
//! clip is selected explicitly; incomplete data must not silently select a different weight class.

use anyhow::Result;
use dsr_formats::hkx::tagfile::Tagfile;
use glam::Vec3;

use crate::dsr::Player;
use crate::retarget::ds_forward;

pub const MEDIUM_ROLL_IFRAMES: f32 = 11.0 / 30.0;
pub const MEDIUM_ROLL: &str = "a00_0710";

/// `ChrActionFlag` values that let an animation be cancelled into another.
const CANCEL_FLAGS: [i32; 3] = [4, 11, 26];

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub name: String,
    pub duration: f32,
    /// Earliest start of an anim-cancel window (s), or `duration` if there is none: the clip
    /// keeps a standing-up tail that play never reaches.
    pub cancel: f32,
    /// Root motion along the forward axis and across it (DS units, metres).
    pub forward: f32,
    pub side: f32,
    /// Total invulnerable time (s).
    pub iframes: f32,
}

/// The measured medium forward roll, with its motion and windows still validated.
pub fn pick(c: &[Candidate]) -> Option<&Candidate> {
    // Distance from the medium roll's window, in whole 30 fps frames.
    let off = |c: &Candidate| ((c.iframes - MEDIUM_ROLL_IFRAMES).abs() * 30.0).round() as i32;
    // The installed a00 family has been measured. A missing medium clip must not silently
    // turn into the fat roll (the old median rule did exactly that with two candidates).
    c.iter().find(|c| {
        c.name == MEDIUM_ROLL
            && off(c) == 0
            && c.duration < 3.0
            && c.forward >= 2.5
            && c.side.abs() < 0.15 * c.forward
            && (c.cancel - 1.1).abs() < 1.0 / 30.0
    })
}

/// The medium backstep: the same weight class as [`MEDIUM_ROLL`] (`a00_0680` fast, `0690` medium,
/// `0695` fat: cancel windows at 0.6, 0.87 and 1.03 s), four invulnerable frames, 2 m backwards.
pub const MEDIUM_BACKSTEP: &str = "a00_0690";
pub const MEDIUM_BACKSTEP_IFRAMES: f32 = 4.0 / 30.0;

/// The measured medium backstep, with its motion and windows still validated: it must be the named
/// clip with four invulnerable frames, backwards travel and no sideways drift.
pub fn pick_backstep(c: &[Candidate]) -> Option<&Candidate> {
    c.iter().find(|c| {
        c.name == MEDIUM_BACKSTEP
            && ((c.iframes - MEDIUM_BACKSTEP_IFRAMES).abs() * 30.0).round() as i32 == 0
            && c.duration < 3.0
            && c.forward <= -1.5
            && c.side.abs() < 0.15 * c.forward.abs()
            && (c.cancel - 0.867).abs() < 1.0 / 30.0
    })
}

/// Every animation with a motion file and an invulnerability window, measured. Forward is the
/// skeleton's facing (`ds_forward`), not the animation file's stored `forward`, which points
/// backwards for c0000.
pub fn candidates(player: &Player) -> Result<Vec<Candidate>> {
    let fwd = ds_forward(&player.skeleton, Vec3::Y, &crate::bonemap::C0000.facing)?;
    let mut out = Vec::new();
    for (name, bytes) in &player.hkx {
        let flags = player.flags(name);
        let iframes: f32 = flags
            .iter()
            .filter(|w| w.flag == wscl::FLAG_INVINCIBLE)
            .map(|w| (w.end - w.start).max(0.0))
            .sum();
        if iframes <= 0.0 {
            continue;
        }
        let Ok(tf) = Tagfile::parse(bytes.clone()) else {
            continue;
        };
        let Ok(file) = dsr_formats::hkx::animation::read(&tf) else {
            continue;
        };
        let Some(rm) = file.root else { continue };
        // Every DSR clip is Y-up; the forward axis above assumes it.
        if (Vec3::from_array(rm.up).normalize() - Vec3::Y).length() > 1.0e-3 {
            continue;
        }
        let (Some(first), Some(last)) = (rm.samples.first(), rm.samples.last()) else {
            continue;
        };
        let d = Vec3::new(last[0] - first[0], last[1] - first[1], last[2] - first[2]);
        let flat = d - Vec3::Y * d.dot(Vec3::Y);
        let duration = file.anim.duration;
        let cancel = flags
            .iter()
            .filter(|w| CANCEL_FLAGS.contains(&w.flag))
            .map(|w| w.start)
            .fold(duration, f32::min);
        out.push(Candidate {
            name: name.clone(),
            duration,
            cancel,
            forward: flat.dot(fwd),
            side: (flat - fwd * flat.dot(fwd)).length(),
            iframes,
        });
    }
    Ok(out)
}

pub fn list(player: &Player) -> Result<()> {
    let c = candidates(player)?;
    println!(
        "{:<14} {:>7} {:>8} {:>8} {:>6} {:>8}",
        "animation", "dur s", "cancel s", "fwd m", "side m", "inv s"
    );
    for x in &c {
        println!(
            "{:<14} {:>7.3} {:>8.3} {:>8.2} {:>6.2} {:>8.3}",
            x.name, x.duration, x.cancel, x.forward, x.side, x.iframes
        );
    }
    match pick(&c) {
        Some(r) => println!("medium forward roll: {}", r.name),
        None => println!("no roll candidate"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(name: &str, cancel: f32, forward: f32, side: f32, iframes: f32) -> Candidate {
        Candidate {
            name: name.into(),
            duration: cancel + 0.9,
            cancel,
            forward,
            side,
            iframes,
        }
    }

    #[test]
    fn a_missing_medium_roll_never_falls_back_to_a_fat_roll() {
        let f11 = 11.0 / 30.0;
        let fast = c("fast", 0.8, 3.94, 0.0, f11);
        let medium = c(MEDIUM_ROLL, 1.1, 3.94, 0.0, f11);
        let fat = c("fat", 1.6, 3.77, 0.0, f11);
        let all = [
            fat.clone(),
            c("thirteen_frames", 0.733, 4.21, 0.0, 13.0 / 30.0),
            c("backstep", 0.6, -2.0, 0.0, 0.133),
            medium,
            c("side", 0.8, 0.0, 4.03, f11),
            fast.clone(),
        ];
        assert_eq!(pick(&all).unwrap().name, MEDIUM_ROLL);
        assert_eq!(pick(&[fast, fat]), None);
        assert_eq!(pick(&[]), None);
    }

    #[test]
    fn the_medium_backstep_is_named_and_never_replaced_by_a_lighter_or_heavier_one() {
        let f4 = 4.0 / 30.0;
        let light = c("a00_0680", 0.6, -2.06, 0.0, f4);
        let medium = c(MEDIUM_BACKSTEP, 0.867, -2.0, 0.0, f4);
        let heavy = c("a00_0695", 1.033, -2.0, 0.0, f4);
        let all = [
            c(MEDIUM_ROLL, 1.1, 3.94, 0.0, 11.0 / 30.0),
            light.clone(),
            heavy.clone(),
            medium,
        ];
        assert_eq!(pick_backstep(&all).unwrap().name, MEDIUM_BACKSTEP);
        assert_eq!(pick_backstep(&[light, heavy]), None);
        // The right name with the wrong body: forwards, sideways, more invulnerability.
        for bad in [
            c(MEDIUM_BACKSTEP, 0.867, 2.0, 0.0, f4),
            c(MEDIUM_BACKSTEP, 0.867, -2.0, 1.5, f4),
            c(MEDIUM_BACKSTEP, 0.867, -2.0, 0.0, 11.0 / 30.0),
            c(MEDIUM_BACKSTEP, 0.5, -2.0, 0.0, f4),
        ] {
            assert_eq!(pick_backstep(&[bad]), None);
        }
    }

    #[test]
    fn install_the_medium_backstep_reads_as_measured() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            return;
        };
        let player = crate::dsr::load(std::path::Path::new(&dir)).unwrap();
        let all = candidates(&player).unwrap();
        let back = pick_backstep(&all).expect("the installed medium backstep");
        assert_eq!(back.name, MEDIUM_BACKSTEP);
        assert!((back.duration - 1.833).abs() < 0.01, "{}", back.duration);
        assert!((back.forward + 2.0).abs() < 0.05, "{}", back.forward);
        // Lighter classes cancel sooner, heavier ones later, like the rolls.
        let cancel = |n: &str| all.iter().find(|c| c.name == n).unwrap().cancel;
        assert!(cancel("a00_0680") < back.cancel && back.cancel < cancel("a00_0695"));
    }

    #[test]
    fn install_medium_and_fat_rolls_have_different_recoveries() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            return;
        };
        let player = crate::dsr::load(std::path::Path::new(&dir)).unwrap();
        let all = candidates(&player).unwrap();
        let medium = pick(&all).unwrap();
        let fat = all.iter().find(|c| c.name == "a00_0720").unwrap();
        assert_eq!(medium.name, MEDIUM_ROLL);
        assert!((medium.cancel - 1.1).abs() < 0.001);
        assert!((fat.cancel - 1.6).abs() < 0.001);
        assert!(medium.duration < fat.duration);
    }
}
