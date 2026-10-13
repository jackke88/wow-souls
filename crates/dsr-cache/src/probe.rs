//! `wow-souls-dsr probe --kit <name> --clips a00_2209,a00_2207`: retargets clips of a kit's
//! Dark Souls creature onto its WoW model with the kit's rig and prints what the gates would see
//! (direction and cross drift, foot gap, how tall the body stands at the start and at the end,
//! where the head is pushed), without refusing anything and without writing anything. For choosing a
//! kit's clips and tuning its bone table.

use std::path::Path;

use anyhow::{Context, Result};

use crate::{dsr, monster, retarget, wow};

/// The kit name that stands for the player's own skeleton (`c0000`, every weapon set) on the
/// WoW HumanMale: `probe --kit player --clips a00_0200,a00_0500`.
pub const PLAYER: &str = "player";

pub fn run(dsr_dir: &Path, wow_dir: &Path, kit_name: &str, clips: &[String]) -> Result<()> {
    let (source, skel, rig) = if kit_name == PLAYER {
        (
            dsr::load(dsr_dir)?,
            wow::human_male(wow_dir)?,
            &crate::bonemap::HUMAN_MALE_RIG,
        )
    } else {
        let kit = monster::KITS
            .iter()
            .find(|k| k.name == kit_name)
            .with_context(|| format!("no kit called {kit_name}"))?;
        (
            dsr::load_chr(dsr_dir, kit.chr)?,
            wow::skeleton(wow_dir, kit.wow_model)?,
            kit.rig,
        )
    };
    let (head, pelvis) = (
        source
            .skeleton
            .bone(rig.names.head)
            .context("the DS head bone is missing")?,
        source
            .skeleton
            .bone(rig.names.pelvis)
            .context("the DS pelvis bone is missing")?,
    );
    for name in clips {
        let clip = dsr::decode(&source, name)?;
        let r = retarget::retarget(&source.skeleton, &clip, &skel, rig, source.flags(name))?;
        let (fwd, left) = monster::push(&r, head, pelvis)?;
        let f = &r.ds.frames;
        let pelvis_y = |i: usize| f[i][pelvis].t[1];
        let last = f.len() - 1;
        let at = |i: usize| glam::Vec3::from_array(f[i][pelvis].t);
        let settle = (last.saturating_sub(10)..=last)
            .map(|i| (at(i) - at(last)).length())
            .fold(0.0_f32, f32::max);
        println!(
            "{name}: {} frames ({:.2} s), direction drift {:.2} deg, cross drift {:.2} deg, foot gap {:.3} yd over {} contact frames ({:.3} all), body {:.2} -> {:.2} yd, pelvis {:.2} -> {:.2} yd (moves {settle:.3} yd over the last 10 frames), head push ({fwd:+.2} forward, {left:+.2} left)",
            f.len(),
            f.len() as f32 / clip.fps,
            r.max_error_deg,
            r.max_cross_deg,
            r.max_foot_gap,
            r.contact_frames,
            r.max_foot_gap_all,
            r.body_height.0,
            r.body_height.1,
            pelvis_y(0),
            pelvis_y(last),
        );
    }
    Ok(())
}
