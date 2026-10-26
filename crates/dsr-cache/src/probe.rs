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
    // `PROBE_CHR=cNNNN PROBE_BONES=a,b`: DS bone paths (model space, minus the pelvis, every third frame) of any
    // chr's clips, no kit needed: which bone strikes in a candidate source (a paw, a jaw) and where it goes.
    if let (Ok(chr), Ok(list)) = (std::env::var("PROBE_CHR"), std::env::var("PROBE_BONES")) {
        // The player (`c0000`) has its own archives.
        let source = if chr == "c0000" {
            dsr::load(dsr_dir)?
        } else {
            dsr::load_chr(dsr_dir, &chr)?
        };
        let pelvis = source.skeleton.bone("Pelvis").context("no Pelvis")?;
        for name in clips {
            let clip = dsr::decode(&source, name)?;
            for (i, locals) in clip.frames.iter().enumerate().step_by(3) {
                let ms = retarget::model_space(&source.skeleton.parents, locals)?;
                let s: Vec<String> = list
                    .split(',')
                    .filter_map(|n| source.skeleton.bone(n))
                    .map(|b| {
                        let v = ms[b].0 - ms[pelvis].0;
                        format!("({:+.2} {:+.2} {:+.2})", v.x, v.y, v.z)
                    })
                    .collect();
                println!("  {name} {:.2}s {}", i as f32 / clip.fps, s.join(" "));
            }
        }
        return Ok(());
    }
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
    // `PROBE_PIVOTS=1`: the WoW model's bones (parent, bind pivot in Bevy axes) for writing a pair table.
    if std::env::var("PROBE_PIVOTS").is_ok() {
        for (i, (p, par)) in skel.pivots.iter().zip(&skel.parents).enumerate() {
            println!(
                "{i:3} parent {par:3} pivot {:+.2} {:+.2} {:+.2}",
                p.x, p.y, p.z
            );
        }
    }
    for name in clips {
        let clip = dsr::decode(&source, name)?;
        let r = retarget::retarget(&source.skeleton, &clip, &skel, rig, source.flags(name))?;
        // `PROBE_BONES=a,b`: the same paths through the kit's retarget.
        if let Ok(list) = std::env::var("PROBE_BONES") {
            for (i, f) in r.ds.frames.iter().enumerate().step_by(3) {
                let p0 = glam::Vec3::from_array(f[pelvis].t);
                let s: Vec<String> = list
                    .split(',')
                    .filter_map(|n| source.skeleton.bone(n))
                    .map(|b| {
                        let v = glam::Vec3::from_array(f[b].t) - p0;
                        format!("({:+.2} {:+.2} {:+.2})", v.x, v.y, v.z)
                    })
                    .collect();
                println!("  {name} {:.2}s {}", i as f32 / clip.fps, s.join(" "));
            }
        }
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
