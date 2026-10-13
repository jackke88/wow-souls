//! Bind-aligned delta transfer (spec "Retargeting method"): for each mapped bone, the DS
//! rotation's delta from its bind, applied after a bind alignment of the two bones' directions.
//! Produces the WoW clip (bone-local), the DS clip (model space, Bevy axes, WoW scale) for the
//! side-by-side, the worst direction error over checked bones and frames, and two checks the
//! direction error cannot make (it is ~0 by construction for aligned bones): directions across
//! the body, and the feet's height. Angles count as errors only as far as they drift from the
//! same angle at bind (unaligned bones keep their anatomical difference by design).

use anyhow::{bail, Context, Result};
use glam::{EulerRot, Quat, Vec3};

use crate::basis::Basis;
use crate::bonemap::{Facing, Pair, Rig};
use crate::dsr::DsClip;
use crate::wow::WowSkel;
use dsr_formats::hkx::skeleton::{Qs, Skeleton};

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

fn q4(a: [f32; 4]) -> Quat {
    Quat::from_xyzw(a[0], a[1], a[2], a[3]).normalize()
}

/// The angle (deg) between two directions.
fn angle_deg(a: Vec3, b: Vec3) -> f32 {
    a.normalize()
        .dot(b.normalize())
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

/// Model-space positions and rotations of a parent-sorted skeleton.
pub fn model_space(parents: &[i16], locals: &[Qs]) -> Result<Vec<(Vec3, Quat)>> {
    let mut out: Vec<(Vec3, Quat)> = Vec::with_capacity(locals.len());
    for (i, l) in locals.iter().enumerate() {
        let (t, r) = (v3(l.t), q4(l.r));
        out.push(match usize::try_from(parents[i]) {
            Ok(p) if p < i => (out[p].0 + out[p].1 * t, out[p].1 * r),
            Ok(p) => bail!("bone {i}'s parent {p} is not before it"),
            Err(_) => (t, r),
        });
    }
    Ok(out)
}

struct Mapped {
    ds: usize,
    ds_aim: usize,
    wow: usize,
    wow_aim: usize,
    check: bool,
    align: bool,
}

fn resolve(map: &[Pair], ds: &Skeleton, wow: &WowSkel) -> Result<Vec<Mapped>> {
    map.iter()
        .map(|p| {
            let ds_i = ds
                .bone(p.ds)
                .with_context(|| format!("DS bone {} missing", p.ds))?;
            let ds_aim = ds
                .bone(p.ds_aim)
                .with_context(|| format!("DS bone {} missing", p.ds_aim))?;
            if p.wow >= wow.parents.len() || p.wow_aim >= wow.parents.len() {
                bail!("WoW bone {} or {} past the skeleton", p.wow, p.wow_aim);
            }
            Ok(Mapped {
                ds: ds_i,
                ds_aim,
                wow: p.wow,
                wow_aim: p.wow_aim,
                check: p.check,
                align: p.align,
            })
        })
        .collect()
}

pub struct Retargeted {
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    /// The worst drift (deg) of the angle between a checked WoW bone's direction and its DS
    /// counterpart's from that angle at bind.
    pub max_error_deg: f32,
    /// The same for the directions across the body (`Checks::cross`).
    pub max_cross_deg: f32,
    /// The worst difference (yd) between the WoW soles' lowest height and the DS soles' on
    /// contact frames (DS soles below `CONTACT_HEIGHT`); 0 without contact frames.
    pub max_foot_gap: f32,
    /// The same over all frames (for information: airborne feet differ by foot anatomy).
    pub max_foot_gap_all: f32,
    /// How many frames are contact frames.
    pub contact_frames: usize,
    /// The height (yd) of the driven WoW bones (the lowest to the highest) on the first frame and
    /// on the last: a body that lies has shrunk to a fraction of what it was standing.
    pub body_height: (f32, f32),
}

/// A frame is a contact frame when the DS soles are below this height (yd).
pub const CONTACT_HEIGHT: f32 = 0.02;

/// The way `ds` faces at bind: for a biped the feet → toes directions, for a quadruped the body's
/// own axis (pelvis → head), perpendicular to `up` (the reference frame's up), summed and
/// normalized. Read from the skeleton rather than the reference frame (DSR's says +Z while c0000
/// faces -Z) or the root motion (backsteps travel backwards), so every clip of a skeleton shares
/// one basis.
pub fn ds_forward(ds: &Skeleton, up: Vec3, facing: &Facing) -> Result<Vec3> {
    let bind = model_space(&ds.parents, &ds.reference)?;
    let up = up.normalize();
    let at = |n: &str| {
        ds.bone(n)
            .map(|i| bind[i].0)
            .with_context(|| format!("DS bone {n} missing (needed for the facing)"))
    };
    let flat = |d: Vec3| d - up * d.dot(up);
    let sum = match facing {
        Facing::FeetToToes(feet) => {
            let mut sum = Vec3::ZERO;
            for &(foot, toe) in feet {
                sum += flat(at(toe)? - at(foot)?);
            }
            sum
        }
        Facing::Along(from, to) => flat(at(to)? - at(from)?),
    };
    if !sum.is_finite() || sum.length() < 1.0e-4 {
        bail!("DS bones {sum} give no horizontal facing");
    }
    Ok(sum.normalize())
}

/// The basis for `clip` on `ds`: DS up from the reference frame, forward from the feet
/// (`ds_forward`), left from the shoulders, scale from the leg lengths (hip → knee → ankle at
/// bind, both sides averaged).
pub fn basis_for(ds: &Skeleton, clip: &DsClip, wow: &WowSkel, rig: &Rig) -> Result<Basis> {
    let bind = model_space(&ds.parents, &ds.reference)?;
    let bone = |n: &str| ds.bone(n).with_context(|| format!("DS bone {n} missing"));
    let left = bind[bone(rig.names.left)?].0 - bind[bone(rig.names.right)?].0;
    let unit = Basis::new(
        v3(clip.up),
        ds_forward(ds, v3(clip.up), &rig.names.facing)?,
        left,
        1.0,
    );
    let mut ds_leg = 0.0;
    for &[hip, knee, ankle] in rig.legs.ds {
        let (h, k, a) = (
            bind[bone(hip)?].0,
            bind[bone(knee)?].0,
            bind[bone(ankle)?].0,
        );
        ds_leg += (k - h).length() + (a - k).length();
    }
    let mut wow_leg = 0.0;
    for &[hip, knee, ankle] in rig.legs.wow {
        let at = |i: usize| {
            wow.pivots
                .get(i)
                .copied()
                .with_context(|| format!("WoW leg bone {i} past the skeleton"))
        };
        let (h, k, a) = (at(hip)?, at(knee)?, at(ankle)?);
        wow_leg += (k - h).length() + (a - k).length();
    }
    if !(ds_leg > 1.0e-4 && wow_leg > 1.0e-4) {
        bail!("degenerate legs: DS {ds_leg}, WoW {wow_leg}");
    }
    Ok(Basis {
        scale: wow_leg / ds_leg,
        ..unit
    })
}

pub fn retarget(
    ds: &Skeleton,
    clip: &DsClip,
    wow: &WowSkel,
    rig: &Rig,
    flags: Vec<wscl::FlagWindow>,
) -> Result<Retargeted> {
    let (hips, checks) = (rig.hips, &rig.checks);
    let pairs = resolve(rig.pairs, ds, wow)?;
    let ds_bone = |n: &str| ds.bone(n).with_context(|| format!("DS bone {n} missing"));
    let wow_bone = |i: usize| {
        if i < wow.parents.len() {
            Ok(i)
        } else {
            bail!("WoW bone {i} past the skeleton")
        }
    };
    let cross = checks
        .cross
        .iter()
        .map(|&(a, b, c, d)| Ok((ds_bone(a)?, ds_bone(b)?, wow_bone(c)?, wow_bone(d)?)))
        .collect::<Result<Vec<_>>>()?;
    let wow_soles = checks
        .wow_soles
        .iter()
        .map(|&i| wow_bone(i))
        .collect::<Result<Vec<_>>>()?;
    let ds_soles = if wow_soles.is_empty() {
        Vec::new()
    } else {
        rig.names
            .soles
            .iter()
            .map(|n| ds_bone(n))
            .collect::<Result<Vec<_>>>()?
    };
    let hip_root = wow_bone(hips.wow_hip_root)?;
    let wow_hips = [
        wow_bone(hips.wow_hip_joints[0])?,
        wow_bone(hips.wow_hip_joints[1])?,
    ];
    let ds_hips = [
        ds_bone(hips.ds_hip_joints[0])?,
        ds_bone(hips.ds_hip_joints[1])?,
    ];
    let basis = basis_for(ds, clip, wow, rig)?;
    let bind = model_space(&ds.parents, &ds.reference)?;
    let n = wow.parents.len();
    let parent = |i: usize| usize::try_from(wow.parents[i]).ok().filter(|&p| p < i);
    // Bind alignment and DS bind rotation per pair.
    let align: Vec<(Quat, Quat)> = pairs
        .iter()
        .map(|m| {
            let dir_w = (wow.pivots[m.wow_aim] - wow.pivots[m.wow]).normalize();
            let dir_d = basis.dir(bind[m.ds_aim].0 - bind[m.ds].0).normalize();
            let a = if m.align {
                Quat::from_rotation_arc(dir_w, dir_d)
            } else {
                Quat::IDENTITY
            };
            (a, basis.rot(bind[m.ds].1))
        })
        .collect();
    // The angles at bind that the checks measure drift from (0 for an aligned pair: the
    // alignment removes its bind difference).
    let pair_bind: Vec<f32> = pairs
        .iter()
        .map(|m| {
            if m.align {
                0.0
            } else {
                angle_deg(
                    wow.pivots[m.wow_aim] - wow.pivots[m.wow],
                    basis.dir(bind[m.ds_aim].0 - bind[m.ds].0),
                )
            }
        })
        .collect();
    let cross_bind: Vec<f32> = cross
        .iter()
        .map(|&(a, b, c, d)| {
            angle_deg(
                wow.pivots[d] - wow.pivots[c],
                basis.dir(bind[b].0 - bind[a].0),
            )
        })
        .collect();
    let mut driven = vec![false; n];
    for m in &pairs {
        let mut i = Some(m.wow);
        while let Some(b) = i {
            driven[b] = true;
            i = parent(b);
        }
    }
    // The offset between the WoW soles' heights at bind and the DS soles' heights, which a
    // relative check subtracts. The DS soles are read at bind, or, for a ground lock, at the
    // clip's first frame: its clips start standing, and a stance is not the bind pose.
    let foot_bind = if checks.relative_soles && !wow_soles.is_empty() {
        let low_w = wow_soles
            .iter()
            .map(|&i| wow.pivots[i].y)
            .fold(f32::INFINITY, f32::min);
        let reference = match clip.frames.first() {
            Some(first) if checks.ground_lock => model_space(&ds.parents, first)?,
            _ => bind.clone(),
        };
        let low_d = ds_soles
            .iter()
            .map(|&i| basis.pos(reference[i].0).y)
            .fold(f32::INFINITY, f32::min);
        low_w - low_d
    } else {
        0.0
    };
    let ds_pelvis = ds.bone(rig.names.pelvis).context("DS pelvis missing")?;
    let bind_local_t = |i: usize| match parent(i) {
        Some(p) => wow.pivots[i] - wow.pivots[p],
        None => wow.pivots[i],
    };
    let mut wow_frames = Vec::with_capacity(clip.frames.len());
    let mut ds_frames = Vec::with_capacity(clip.frames.len());
    let mut max_error = 0.0f32;
    let mut max_cross = 0.0f32;
    let mut max_foot_gap = 0.0f32;
    let mut max_foot_gap_all = 0.0f32;
    let mut contact_frames = 0;
    let mut heights: Vec<f32> = Vec::with_capacity(clip.frames.len());
    // The hip root turns with the DS pelvis, so the spine base and the waist move as one, and is
    // placed by the hip joints (see `Hips`): their bind midpoint relative to the hip root.
    let hip_arm = (wow.pivots[wow_hips[0]] + wow.pivots[wow_hips[1]]) * 0.5 - wow.pivots[hip_root];
    let pelvis_bind = basis.rot(bind[ds_pelvis].1);
    for locals in &clip.frames {
        let ms = model_space(&ds.parents, locals)?;
        let d_pelvis = (basis.rot(ms[ds_pelvis].1) * pelvis_bind.inverse()).normalize();
        let low_d = ds_soles
            .iter()
            .map(|&i| basis.pos(ms[i].0).y)
            .fold(f32::INFINITY, f32::min);
        // How far the body was raised (negative: lowered) to put its soles where the DS soles are
        // (`Checks::ground_lock`); found by posing the frame once without, once with the lift.
        let mut lift = 0.0f32;
        let mut locked = !(checks.ground_lock && !wow_soles.is_empty());
        let (out, pos) = loop {
            let mut world = vec![Quat::IDENTITY; n];
            let mut pos = vec![Vec3::ZERO; n];
            let mut out = Vec::with_capacity(n);
            let hip_target =
                basis.pos((ms[ds_hips[0]].0 + ms[ds_hips[1]].0) * 0.5) + Vec3::Y * lift;
            for i in 0..n {
                let parent_world = parent(i).map_or(Quat::IDENTITY, |p| world[p]);
                world[i] = match pairs.iter().position(|m| m.wow == i) {
                    Some(k) => {
                        let delta = basis.rot(ms[pairs[k].ds].1) * align[k].1.inverse();
                        (delta * align[k].0).normalize()
                    }
                    None if i == hip_root => d_pelvis,
                    None => parent_world,
                };
                let mut t = bind_local_t(i);
                if i == hip_root {
                    let parent_pos = parent(i).map_or(Vec3::ZERO, |p| pos[p]);
                    t = parent_world.inverse() * (hip_target - world[i] * hip_arm - parent_pos);
                }
                pos[i] = match parent(i) {
                    Some(p) => pos[p] + world[p] * t,
                    None => t,
                };
                let local = (parent_world.inverse() * world[i]).normalize();
                out.push(wscl::Trs {
                    t: t.to_array(),
                    r: local.to_array(),
                    s: [1.0; 3],
                });
            }
            if !locked {
                locked = true;
                let low_w = wow_soles
                    .iter()
                    .map(|&i| pos[i].y)
                    .fold(f32::INFINITY, f32::min);
                lift = low_d + foot_bind - low_w;
                continue;
            }
            break (out, pos);
        };
        for (m, at_bind) in pairs.iter().zip(&pair_bind).filter(|(m, _)| m.check) {
            let dw = pos[m.wow_aim] - pos[m.wow];
            let dd = basis.dir(ms[m.ds_aim].0 - ms[m.ds].0);
            max_error = max_error.max((angle_deg(dw, dd) - at_bind).abs());
        }
        for (&(a, b, c, d), at_bind) in cross.iter().zip(&cross_bind) {
            let dd = basis.dir(ms[b].0 - ms[a].0);
            max_cross = max_cross.max((angle_deg(pos[d] - pos[c], dd) - at_bind).abs());
        }
        if !wow_soles.is_empty() {
            let low_w = wow_soles
                .iter()
                .map(|&i| pos[i].y)
                .fold(f32::INFINITY, f32::min);
            // Locked, the soles agree by construction: the lift is what was moved.
            let gap = if checks.ground_lock {
                lift.abs()
            } else {
                (low_w - low_d - foot_bind).abs()
            };
            max_foot_gap_all = max_foot_gap_all.max(gap);
            if low_d < rig.contact_height {
                contact_frames += 1;
                max_foot_gap = max_foot_gap.max(gap);
            }
        }
        let (low, high) = (0..n)
            .filter(|&i| driven[i])
            .fold((f32::MAX, f32::MIN), |(lo, hi), i| {
                (lo.min(pos[i].y), hi.max(pos[i].y))
            });
        heights.push(high - low);
        wow_frames.push(out);
        ds_frames.push(
            ms.iter()
                .map(|(p, r)| wscl::Trs {
                    t: basis.pos(*p).to_array(),
                    r: basis.rot(*r).to_array(),
                    s: [1.0; 3],
                })
                .collect::<Vec<_>>(),
        );
    }
    let up = v3(clip.up);
    let root_motion: Vec<[f32; 4]> = clip
        .root
        .iter()
        .map(|r| {
            let p = basis.pos(Vec3::new(r[0], r[1], r[2]));
            let yaw = basis
                .rot(Quat::from_axis_angle(up.normalize(), r[3]))
                .to_euler(EulerRot::YXZ)
                .0;
            [p.x, p.y, p.z, yaw]
        })
        .collect();
    let wow_clip = wscl::Clip {
        name: clip.name.clone(),
        fps: clip.fps,
        model_space: false,
        parents: wow.parents.clone(),
        driven,
        frames: wow_frames,
        root_motion: root_motion.clone(),
        flags: flags.clone(),
        hits: Vec::new(),
    };
    let ds_clip = wscl::Clip {
        name: clip.name.clone(),
        fps: clip.fps,
        model_space: true,
        parents: ds.parents.clone(),
        driven: vec![true; ds.parents.len()],
        frames: ds_frames,
        root_motion,
        flags,
        hits: Vec::new(),
    };
    Ok(Retargeted {
        wow: wow_clip,
        ds: ds_clip,
        max_error_deg: max_error,
        max_cross_deg: max_cross,
        max_foot_gap,
        max_foot_gap_all,
        contact_frames,
        body_height: (
            heights.first().copied().unwrap_or(0.0),
            heights.last().copied().unwrap_or(0.0),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bonemap::{self, Checks, Hips, Legs};

    /// DS: a pelvis, a left upper arm along -X (T-pose) with its forearm, a right upper arm and
    /// straight legs (0.45 + 0.45) with feet facing -Z. WoW: the same left arm bound 45°
    /// down-left (A-pose), a marker across it, and legs of the same length (scale 1).
    fn arm_rig() -> (Skeleton, WowSkel, Vec<Pair>) {
        let qs = |t: [f32; 3]| Qs { t, ..Qs::IDENTITY };
        let ds = Skeleton {
            names: [
                "Pelvis",
                "L_UpperArm",
                "L_Forearm",
                "R_UpperArm",
                "L_Thigh",
                "L_Calf",
                "L_Foot",
                "L_Toe0",
                "R_Thigh",
                "R_Calf",
                "R_Foot",
                "R_Toe0",
            ]
            .map(String::from)
            .to_vec(),
            parents: vec![-1, 0, 1, 0, 0, 4, 5, 6, 0, 8, 9, 10],
            reference: vec![
                qs([0.0, 1.0, 0.0]),
                qs([-0.2, 0.5, 0.0]),
                qs([-0.3, 0.0, 0.0]),
                qs([0.2, 0.5, 0.0]),
                qs([-0.1, 0.0, 0.0]),
                qs([0.0, -0.45, 0.0]),
                qs([0.0, -0.45, 0.0]),
                qs([0.0, 0.0, -0.1]),
                qs([0.1, 0.0, 0.0]),
                qs([0.0, -0.45, 0.0]),
                qs([0.0, -0.45, 0.0]),
                qs([0.0, 0.0, -0.1]),
            ],
        };
        // WoW: 0 root, 1 hips (pelvis), 2 shoulder → 3 elbow pointing 45° down-left, 4 a marker
        // off the shoulder across the arm (+Z) that shows its twist, 5-7 / 8-10 hip, knee, ankle.
        let wow = WowSkel {
            parents: vec![-1, 0, 1, 2, 2, 1, 5, 6, 1, 8, 9],
            pivots: vec![
                Vec3::ZERO,
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(-0.2, 1.5, 0.0),
                Vec3::new(-0.4, 1.3, 0.0),
                Vec3::new(-0.2, 1.5, 0.1),
                Vec3::new(-0.1, 1.0, 0.0),
                Vec3::new(-0.1, 0.55, 0.0),
                Vec3::new(-0.1, 0.1, 0.0),
                Vec3::new(0.1, 1.0, 0.0),
                Vec3::new(0.1, 0.55, 0.0),
                Vec3::new(0.1, 0.1, 0.0),
            ],
        };
        let map = vec![
            Pair {
                ds: "Pelvis",
                ds_aim: "L_UpperArm",
                wow: 1,
                wow_aim: 2,
                check: false,
                align: false,
            },
            Pair {
                ds: "L_UpperArm",
                ds_aim: "L_Forearm",
                wow: 2,
                wow_aim: 3,
                check: true,
                align: true,
            },
        ];
        (ds, wow, map)
    }

    fn test_rig(pairs: &[Pair]) -> Rig<'_> {
        Rig {
            pairs,
            hips: Hips {
                wow_hip_root: 1,
                wow_hip_joints: [5, 8],
                ds_hip_joints: ["L_Thigh", "R_Thigh"],
            },
            legs: Legs {
                ds: &[
                    ["L_Thigh", "L_Calf", "L_Foot"],
                    ["R_Thigh", "R_Calf", "R_Foot"],
                ],
                wow: &[[5, 6, 7], [8, 9, 10]],
            },
            checks: Checks {
                cross: &[],
                wow_soles: &[],
                relative_soles: false,
                ground_lock: false,
                max_foot_gap: crate::bonemap::DEFAULT_FOOT_GAP,
            },
            names: &bonemap::C0000,
            contact_height: CONTACT_HEIGHT,
        }
    }

    fn clip_of(frames: Vec<Vec<Qs>>) -> DsClip {
        DsClip {
            name: "test".into(),
            fps: 30.0,
            root: vec![[0.0; 4]; frames.len()],
            frames,
            up: [0.0, 1.0, 0.0],
        }
    }

    /// DS: the upper arm turns 90° about Z to point down by frame 1. WoW: bound 45° down-left.
    /// The WoW bone must follow the DS bone's direction in every frame.
    #[test]
    fn a_mapped_bone_follows_its_ds_direction_despite_a_different_bind() {
        let (ds, wow, map) = arm_rig();
        let down = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let mut f1 = ds.reference.clone();
        f1[1].r = down.to_array();
        let clip = clip_of(vec![ds.reference.clone(), f1]);
        let r = retarget(&ds, &clip, &wow, &test_rig(&map), Vec::new()).unwrap();
        assert!(r.max_error_deg < 1.0, "error {}", r.max_error_deg);
        assert_eq!(r.wow.frames.len(), 2);
        assert_eq!(r.wow.parents, wow.parents);
        assert!(
            r.wow.driven[2] && r.wow.driven[0],
            "mapped bones and their ancestors are driven"
        );
        assert!(!r.wow.driven[3], "an unmapped leaf is left alone");
        assert!(r.ds.model_space);
    }

    /// A DS bone twisting 90° about its own aim axis gives the WoW bone the same twist about
    /// its direction: a marker across the WoW arm turns 90° around it, the arm stays put.
    #[test]
    fn a_twist_about_the_bone_carries_over() {
        let (ds, wow, map) = arm_rig();
        let twist = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let mut f1 = ds.reference.clone();
        f1[1].r = twist.to_array();
        let clip = clip_of(vec![ds.reference.clone(), f1]);
        let r = retarget(&ds, &clip, &wow, &test_rig(&map), Vec::new()).unwrap();
        let fk = |f: usize| {
            let ms = wow_fk(&r, f);
            (ms[3].0 - ms[2].0, ms[4].0 - ms[2].0)
        };
        let ((arm0, marker0), (arm1, marker1)) = (fk(0), fk(1));
        assert!(
            (arm1 - arm0).length() < 1.0e-5,
            "the arm stays: {arm0} → {arm1}"
        );
        assert!((arm0.normalize() - Vec3::NEG_X).length() < 1.0e-5, "{arm0}");
        let want = twist * marker0;
        assert!((marker1 - want).length() < 1.0e-5, "{marker1} vs {want}");
        assert!((angle_deg(marker0, marker1) - 90.0).abs() < 1.0e-3);
    }

    #[test]
    fn the_facing_comes_from_the_feet() {
        let feet = Skeleton {
            names: ["Pelvis", "L_Foot", "L_Toe0", "R_Foot", "R_Toe0"]
                .map(String::from)
                .to_vec(),
            parents: vec![-1, 0, 1, 0, 3],
            reference: [
                [0.0, 1.0, 0.0],
                [0.1, -0.9, 0.0],
                [0.0, -0.05, -0.1],
                [-0.1, -0.9, 0.0],
                [0.0, -0.05, -0.1],
            ]
            .map(|t| Qs { t, ..Qs::IDENTITY })
            .to_vec(),
        };
        let f = ds_forward(&feet, Vec3::Y, &bonemap::C0000.facing).unwrap();
        assert!((f - Vec3::NEG_Z).length() < 1.0e-6, "{f}");
        let no_toes = Skeleton {
            names: feet.names[..2].to_vec(),
            parents: feet.parents[..2].to_vec(),
            reference: feet.reference[..2].to_vec(),
        };
        assert!(ds_forward(&no_toes, Vec3::Y, &bonemap::C0000.facing).is_err());
    }

    #[test]
    fn model_space_composes_parents_first() {
        let locals = [
            Qs {
                t: [0.0, 1.0, 0.0],
                r: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array(),
                s: [1.0; 3],
            },
            Qs {
                t: [0.0, 0.0, -1.0],
                ..Qs::IDENTITY
            },
        ];
        let ms = model_space(&[-1, 0], &locals).unwrap();
        assert!(
            (ms[1].0 - Vec3::new(-1.0, 1.0, 0.0)).length() < 1.0e-5,
            "{}",
            ms[1].0
        );
        assert!(model_space(&[1, -1], &locals).is_err());
    }

    /// FK of a retargeted WoW frame.
    fn wow_fk(r: &Retargeted, f: usize) -> Vec<(Vec3, Quat)> {
        let locals: Vec<Qs> = r.wow.frames[f]
            .iter()
            .map(|t| Qs {
                t: t.t,
                r: t.r,
                s: t.s,
            })
            .collect();
        model_space(&r.wow.parents, &locals).unwrap()
    }

    #[test]
    fn install_the_roll_retargets_onto_the_human() {
        let (Ok(dsr_dir), Ok(wow_data)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("skipped: DSR_DIR and WOW_DATA not both set");
            return;
        };
        let player = crate::dsr::load(std::path::Path::new(&dsr_dir)).unwrap();
        let wow = crate::wow::human_male(std::path::Path::new(&wow_data)).unwrap();
        let rig = &bonemap::HUMAN_MALE_RIG;
        let ds_bind = model_space(&player.skeleton.parents, &player.skeleton.reference).unwrap();
        let bone = |n: &str| player.skeleton.bone(n).unwrap();
        // a00_0700 is the plan's reference roll; a00_0710 the one `roll::pick` chooses.
        for stem in ["a00_0700", "a00_0710"] {
            let clip = crate::dsr::decode(&player, stem).unwrap();
            let basis = basis_for(&player.skeleton, &clip, &wow, rig).unwrap();
            let r = retarget(&player.skeleton, &clip, &wow, rig, player.flags(stem)).unwrap();
            let bind_dir = |a: &str, b: &str| basis.dir(ds_bind[bone(b)].0 - ds_bind[bone(a)].0);
            let pair_bind = |p: &Pair| {
                if p.align {
                    0.0
                } else {
                    angle_deg(
                        wow.pivots[p.wow_aim] - wow.pivots[p.wow],
                        bind_dir(p.ds, p.ds_aim),
                    )
                }
            };
            let last = *r.wow.root_motion.last().unwrap();
            println!("== {stem}: {} frames", r.wow.frames.len());
            println!(
                "scale {:.4}, ds_forward {}",
                basis.scale,
                ds_forward(&player.skeleton, v3(clip.up), &rig.names.facing).unwrap()
            );
            println!(
                "max_error_deg {:.3}, max_cross_deg {:.3}, max_foot_gap {:.4} yd over {} contact \
                 frames, max_foot_gap_all {:.4} yd",
                r.max_error_deg,
                r.max_cross_deg,
                r.max_foot_gap,
                r.contact_frames,
                r.max_foot_gap_all
            );
            println!("last root motion {last:?}");

            // Per-pair and per-frame breakdown from the output clips (WoW by FK, DS straight
            // from the model-space clip, already in Bevy axes at WoW scale).
            let mut worst = vec![(0.0f32, 0usize); bonemap::HUMAN_MALE.len()];
            let mut cross = vec![(0.0f32, 0usize); bonemap::HUMAN_MALE_CHECKS.cross.len()];
            let mut gap = (0.0f32, 0usize);
            let mut contact_gap = (0.0f32, 0usize);
            let mut gaps = Vec::new();
            for f in 0..r.wow.frames.len() {
                let wm = wow_fk(&r, f);
                let df = &r.ds.frames[f];
                let dp = |n: &str| v3(df[bone(n)].t);
                for (k, p) in bonemap::HUMAN_MALE.iter().enumerate() {
                    let at_bind = pair_bind(p);
                    let e = (angle_deg(wm[p.wow_aim].0 - wm[p.wow].0, dp(p.ds_aim) - dp(p.ds))
                        - at_bind)
                        .abs();
                    if p.check && e > worst[k].0 {
                        worst[k] = (e, f);
                    }
                }
                for (k, &(a, b, c, d)) in bonemap::HUMAN_MALE_CHECKS.cross.iter().enumerate() {
                    let at_bind = angle_deg(wow.pivots[d] - wow.pivots[c], bind_dir(a, b));
                    let e = (angle_deg(wm[d].0 - wm[c].0, dp(b) - dp(a)) - at_bind).abs();
                    if e > cross[k].0 {
                        cross[k] = (e, f);
                    }
                }
                let low_w = bonemap::HUMAN_MALE_CHECKS
                    .wow_soles
                    .iter()
                    .map(|&i| wm[i].0.y)
                    .fold(f32::INFINITY, f32::min);
                let low_d = bonemap::C0000
                    .soles
                    .iter()
                    .map(|n| dp(n).y)
                    .fold(f32::INFINITY, f32::min);
                let g = (low_w - low_d).abs();
                if g > gap.0 {
                    gap = (g, f);
                }
                if low_d < CONTACT_HEIGHT && g > contact_gap.0 {
                    contact_gap = (g, f);
                }
                gaps.push((low_w, low_d, g));
            }
            for (p, (e, f)) in bonemap::HUMAN_MALE
                .iter()
                .zip(&worst)
                .filter(|(p, _)| p.check)
            {
                let at_bind = pair_bind(p);
                println!(
                    "  dir   {:<12} bind {at_bind:7.3} deg, drift {e:7.3} deg (frame {f})",
                    p.ds
                );
            }
            for ((a, b, c, d), (e, f)) in bonemap::HUMAN_MALE_CHECKS.cross.iter().zip(&cross) {
                let at_bind = angle_deg(wow.pivots[*d] - wow.pivots[*c], bind_dir(a, b));
                println!(
                    "  cross {a}->{b} / {c}->{d} bind {at_bind:7.3} deg, drift {e:7.3} deg (frame {f})"
                );
            }
            println!(
                "  foot gap {:.4} yd (frame {}); on contact frames {:.4} yd (frame {})",
                gap.0, gap.1, contact_gap.0, contact_gap.1
            );
            for (f, (lw, ld, g)) in gaps.iter().enumerate().skip(12).take(9) {
                println!("    frame {f}: soles WoW {lw:.3} DS {ld:.3} gap {g:.3}");
            }
            let lf = r.wow.frames.len() - 1;
            let (wm, df) = (wow_fk(&r, lf), &r.ds.frames[lf]);
            let dy = |n: &str| df[bone(n)].t[1];
            println!(
                "  last frame hips y WoW {:.3}/{:.3} vs DS {:.3}/{:.3}",
                wm[25].0.y,
                wm[26].0.y,
                dy("L_Thigh"),
                dy("R_Thigh")
            );
            println!(
                "  last frame ankles/toes y WoW {:.3}/{:.3}/{:.3}/{:.3} vs DS {:.3}/{:.3}/{:.3}/{:.3}",
                wm[68].0.y,
                wm[70].0.y,
                wm[80].0.y,
                wm[81].0.y,
                dy("L_Foot"),
                dy("R_Foot"),
                dy("L_Toe0"),
                dy("R_Toe0")
            );
            let bind_waist = (wow.pivots[20] - wow.pivots[21]).length();
            for f in [0, 12] {
                let wm = wow_fk(&r, f);
                println!(
                    "  frame {f}: waist 20-21 separation {:.4} yd (bind {bind_waist:.4})",
                    (wm[20].0 - wm[21].0).length()
                );
            }

            // The direction error cannot see a body turned around (every bone aligns to the
            // turned DS bone); the converted DS pose can: in the first frame its toes point
            // forward (Bevy -Z) and its left shoulder sits on Bevy -X.
            let at0 = |n: &str| v3(r.ds.frames[0][bone(n)].t);
            let fwd = at0("L_Toe0") - at0("L_Foot");
            println!(
                "  DS frame 0: foot->toe {fwd}, L_UpperArm {}",
                at0("L_UpperArm")
            );
            assert!(fwd.z < 0.0 && fwd.z.abs() > fwd.x.abs(), "DS toes {fwd}");
            assert!(at0("L_UpperArm").x < 0.0, "DS left arm on the left");

            assert_eq!(r.wow.frames.len(), clip.frames.len());
            assert!(r.wow.frames.iter().all(|f| f.len() == 119));
            assert!(r.max_error_deg < 15.0, "error {}", r.max_error_deg);
            assert!(
                r.max_cross_deg <= crate::cache::MAX_CROSS_DEG,
                "cross {}",
                r.max_cross_deg
            );
            assert!(
                r.max_foot_gap <= crate::bonemap::DEFAULT_FOOT_GAP,
                "foot gap {}",
                r.max_foot_gap
            );
            assert!((0.8..1.6).contains(&basis.scale), "scale {}", basis.scale);
            assert!(last[2] < 0.0, "forward is Bevy -Z: {last:?}");
            assert!((3.0..6.0).contains(&last[2].abs()), "distance {last:?}");
            for (f, frame) in r.wow.frames.iter().enumerate() {
                for (b, t) in frame.iter().enumerate() {
                    let len = Quat::from_array(t.r).length();
                    assert!(
                        (len - 1.0).abs() < 1.0e-4,
                        "frame {f} bone {b}: |r| = {len}"
                    );
                }
            }
            assert!(r.contact_frames > 0, "the roll lands");
            assert!((r.max_foot_gap - contact_gap.0).abs() < 1.0e-4);
            assert!((r.max_foot_gap_all - gap.0).abs() < 1.0e-4);
            // The waist does not tear: the spine base keeps its bind distance from the waist; and
            // the hip joints sit on the DS hip joints.
            for f in 0..r.wow.frames.len() {
                let wm = wow_fk(&r, f);
                let df = &r.ds.frames[f];
                let wow_mid = (wm[25].0 + wm[26].0) * 0.5;
                let ds_mid = (v3(df[bone("L_Thigh")].t) + v3(df[bone("R_Thigh")].t)) * 0.5;
                assert!(
                    (wow_mid - ds_mid).length() < 1.0e-3,
                    "frame {f}: hips {wow_mid} vs {ds_mid}"
                );
                let sep = (wm[20].0 - wm[21].0).length();
                assert!((sep - bind_waist).abs() < 1.0e-3, "frame {f}: waist {sep}");
            }
        }
        let clip = crate::dsr::decode(&player, "a00_0700").unwrap();
        assert_eq!(clip.frames.len(), 51);
    }

    /// The ghoul's moves, flinches, death and idle retarget onto the kobold inside the same
    /// gates the player's clips pass; prints the numbers.
    #[test]
    fn install_the_ghoul_retargets_onto_the_kobold() {
        let (Ok(dsr_dir), Ok(wow_data)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("skipped: DSR_DIR and WOW_DATA not both set");
            return;
        };
        let ghoul = crate::dsr::load_chr(std::path::Path::new(&dsr_dir), "c2060").unwrap();
        let kobold =
            crate::wow::skeleton(std::path::Path::new(&wow_data), crate::wow::KOBOLD_M2).unwrap();
        let rig = &bonemap::KOBOLD_RIG;
        for stem in [
            "a00_0000", "a00_3000", "a00_3001", "a00_3002", "a00_3003", "a00_3004", "a00_3006",
            "a00_2000", "a00_2001", "a00_2004", "a00_2005", "a00_2006", "a00_2007", "a00_9000",
        ] {
            let clip = crate::dsr::decode(&ghoul, stem).unwrap();
            let basis = basis_for(&ghoul.skeleton, &clip, &kobold, rig).unwrap();
            let r = retarget(&ghoul.skeleton, &clip, &kobold, rig, ghoul.flags(stem)).unwrap();
            println!(
                "{stem}: scale {:.4}, direction drift {:.2} deg, cross drift {:.2} deg,                  foot gap {:.3} yd on {} contact frames ({:.3} over all)",
                basis.scale,
                r.max_error_deg,
                r.max_cross_deg,
                r.max_foot_gap,
                r.contact_frames,
                r.max_foot_gap_all
            );
            assert!(
                r.max_error_deg <= crate::cache::MAX_ERROR_DEG,
                "{stem}: {}",
                r.max_error_deg
            );
            assert!(
                r.max_cross_deg <= crate::cache::MAX_CROSS_DEG,
                "{stem}: {}",
                r.max_cross_deg
            );
            assert!(
                r.max_foot_gap <= crate::bonemap::DEFAULT_FOOT_GAP,
                "{stem}: {}",
                r.max_foot_gap
            );
            assert!(
                (0.4..0.9).contains(&basis.scale),
                "{stem}: scale {}",
                basis.scale
            );
            assert!(r.wow.frames.iter().all(|f| f.len() == 72));
        }
    }
}
