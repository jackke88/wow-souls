//! The WoW HumanMale skeleton (parents, pivots in Bevy axes) from the user's WoW install.

use std::io::Cursor;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use glam::Vec3;

pub const HUMAN_MALE_M2: &str = "Character\\Human\\Male\\HumanMale.m2";

#[derive(Debug, Clone)]
pub struct WowSkel {
    pub parents: Vec<i16>,
    /// Bone pivots, model space, Bevy axes.
    pub pivots: Vec<Vec3>,
}

/// WoW (x north, y west, z up) → Bevy (x east, y up, -z north).
pub fn wow_to_bevy(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(-y, z, -x)
}

pub const KOBOLD_M2: &str = "Creature\\Kobold\\Kobold.m2";
pub const WOLF_M2: &str = "Creature\\Wolf\\Wolf.m2";

/// The bones of the M2 `model` (parents, pivots in Bevy axes) from the install's data directory.
pub fn skeleton(data_dir: &Path, model: &str) -> Result<WowSkel> {
    let mut chain = benilla_formats::Chain::open(data_dir)
        .with_context(|| format!("opening WoW data {}", data_dir.display()))?;
    let bytes = chain.read_file(model)?;
    let fmt = benilla_m2::parse_m2(&mut Cursor::new(&bytes[..]))
        .map_err(|e| anyhow!("parsing {model}: {e:?}"))?;
    let bones = &fmt.model().bones;
    Ok(WowSkel {
        parents: bones.iter().map(|b| b.parent).collect(),
        pivots: bones
            .iter()
            .map(|b| wow_to_bevy(b.pivot.x, b.pivot.y, b.pivot.z))
            .collect(),
    })
}

pub fn human_male(data_dir: &Path) -> Result<WowSkel> {
    skeleton(data_dir, HUMAN_MALE_M2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_human_male_has_its_measured_bones() {
        let Ok(data) = std::env::var("WOW_DATA") else {
            eprintln!("skipped: WOW_DATA not set");
            return;
        };
        let skel = human_male(Path::new(&data)).unwrap();
        assert_eq!(skel.parents.len(), 119);
        assert_eq!(skel.pivots.len(), 119);
        assert_eq!(skel.parents[21], 18);
        assert_eq!(skel.parents[65], 49);
        assert_eq!(skel.parents[95], 78);
        let pelvis = skel.pivots[21];
        assert!((pelvis.y - 1.16).abs() < 0.02, "pelvis {pelvis}");
        // WoW +Y is left; `wow_to_bevy` maps y to -x, so the left arm has negative Bevy x.
        let arm_l = skel.pivots[65];
        assert!(arm_l.x < 0.0, "ArmL {arm_l}");
    }

    #[test]
    fn install_the_kobold_has_its_measured_bones() {
        let Ok(data) = std::env::var("WOW_DATA") else {
            eprintln!("skipped: WOW_DATA not set");
            return;
        };
        let k = skeleton(Path::new(&data), KOBOLD_M2).unwrap();
        assert_eq!(k.parents.len(), 72);
        // Key bones 26 root, 4 spine, 6 head; the thighs hang off the hip root 18.
        assert_eq!((k.parents[14], k.parents[27], k.parents[28]), (13, 18, 18));
        assert_eq!((k.parents[35], k.parents[39], k.parents[47]), (29, 27, 39));
        // Left is +Y in WoW, so Bevy -X.
        assert!(
            k.pivots[27].x < 0.0 && k.pivots[28].x > 0.0,
            "{} {}",
            k.pivots[27],
            k.pivots[28]
        );
        // Feet stand near the ground, the hips at about 0.74.
        assert!(k.pivots[55].y < 0.05 && (k.pivots[27].y - 0.743).abs() < 0.02);
    }
}
