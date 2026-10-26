//! Dark Souls locomotion: the body's walk, run, sprint and locked-on steps, from
//! `anim/source/locomotion.txt` (tracked editable project data).
//!
//! Every weapon motion set of the install (`a00` unarmed and one-handed, `a10` two-handed small
//! weapons, `a12` great weapons over the shoulder, `a13` polearms, `a14` bow, `a16` crossbow)
//! holds the same gait clips posed for its weapon, so a body carries its weapon the way Dark
//! Souls does while it travels. A [`Kit`] is one set's eleven clips; the set is the one the held
//! weapon's idle clip comes from (`WeaponClips::idle`, `great_weapon_idle`).
//!
//! What plays is a blend space over the body's achieved speed. Free, the body turns to its
//! travel, so only forward clips are needed: walk, run and sprint blend by speed. Locked on, the
//! body keeps facing its target, so the walk and the run each come in four directions, and the
//! two nearest directions of the travel blend. One stride cycle is shared by the clips of a blend
//! (they all loop, with a foot down at about a quarter and three quarters of the way round), and
//! it advances by the ground covered over the blend's stride, so the feet never slide whatever
//! the speed. The gait owns the whole pose of the driven bones (legs, spine, arms, hands) at a
//! weight that eases in and out ([`GaitState::weight`]); the stance and the actions of
//! [`crate::motion_clip`] blend over it.
//!
//! Footsteps come from the gait's own foot-downs, not from the WoW clip playing underneath, so a
//! step sounds when the visible foot lands ([`crate::foot`] stays quiet while this owns the legs).
//! Without a cache, or for a set the cache lacks, nothing here runs and the WoW gait plays.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use benilla_world::rig_anim::{PosePost, RigPose};
use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::combat::{Action, ActionState};
use crate::foot::AnimSound;
use crate::motion_clip::{self, sample, WeaponClips};
use crate::motor::Motor;

/// A locked-on direction, in the body's frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dir {
    Forward,
    Back,
    Left,
    Right,
}

/// One clip of a kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Free: the body faces its travel, so each is forward only.
    Walk,
    Run,
    Sprint,
    /// Locked on: the body faces its target, whatever way it steps.
    LockWalk(Dir),
    LockRun(Dir),
}

impl Role {
    pub const ALL: [Role; 11] = [
        Role::Walk,
        Role::Run,
        Role::Sprint,
        Role::LockWalk(Dir::Forward),
        Role::LockWalk(Dir::Back),
        Role::LockWalk(Dir::Left),
        Role::LockWalk(Dir::Right),
        Role::LockRun(Dir::Forward),
        Role::LockRun(Dir::Back),
        Role::LockRun(Dir::Left),
        Role::LockRun(Dir::Right),
    ];

    /// The name in `locomotion.txt` (`dsr-cache`'s `loco::ROLES`).
    pub fn name(self) -> &'static str {
        match self {
            Role::Walk => "walk",
            Role::Run => "run",
            Role::Sprint => "sprint",
            Role::LockWalk(Dir::Forward) => "lock_walk_f",
            Role::LockWalk(Dir::Back) => "lock_walk_b",
            Role::LockWalk(Dir::Left) => "lock_walk_l",
            Role::LockWalk(Dir::Right) => "lock_walk_r",
            Role::LockRun(Dir::Forward) => "lock_run_f",
            Role::LockRun(Dir::Back) => "lock_run_b",
            Role::LockRun(Dir::Left) => "lock_run_l",
            Role::LockRun(Dir::Right) => "lock_run_r",
        }
    }

    pub fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == name)
    }

    /// The way the clip travels, in the body's frame.
    fn dir(self) -> Dir {
        match self {
            Role::LockWalk(d) | Role::LockRun(d) => d,
            _ => Dir::Forward,
        }
    }
}

/// The WoW HumanMale ankles, left and right (`dsr-cache` `HUMAN_MALE_CHECKS.wow_soles`).
const ANKLES: [usize; 2] = [68, 70];

/// A foot is planted while it moves over the ground slower than this share of the clip's pace.
const PLANTED_SHARE: f32 = 0.3;

/// One gait clip: both of its files and what the blend needs to know about it.
#[derive(Debug)]
pub struct Clip {
    pub role: Role,
    /// On the WoW HumanMale skeleton, bone-local.
    pub wow: wscl::Clip,
    /// On the DS skeleton, model space (for the side-by-side).
    pub reference: wscl::Clip,
    /// Yards one loop covers along the clip's own way of travel (always positive).
    pub stride: f32,
    /// Where in the loop (0..1) a foot comes down.
    pub plants: Vec<f32>,
    /// The roll (radians, about the body's forward axis, positive toward its left) that stands
    /// the clip's torso upright on average: a source clip that carries its trunk to one side the
    /// whole loop reads as a crooked body on a different skeleton. Sprint measures the actual
    /// spine-to-neck line and corrects it at the spine base; other roles use the chest orientation.
    pub straighten: f32,
    /// Per bone, what carries the last frame onto the first (a translation, and a rotation
    /// applied after the bone's own), of which [`blended`] adds the loop's phase: a source loop
    /// that does not quite close (the polearm's run: 12 degrees at one shoulder) would jump once
    /// a cycle without it.
    closure: Vec<(Vec3, Quat)>,
}

/// What carries each bone of `clip`'s last frame onto its first.
fn closure_of(clip: &wscl::Clip) -> Vec<(Vec3, Quat)> {
    let (Some(first), Some(last)) = (clip.frames.first(), clip.frames.last()) else {
        return Vec::new();
    };
    first
        .iter()
        .zip(last)
        .map(|(f, l)| {
            let qf = Quat::from_array(f.r).normalize();
            let ql = Quat::from_array(l.r).normalize();
            (
                Vec3::from_array(f.t) - Vec3::from_array(l.t),
                ql.inverse() * qf,
            )
        })
        .collect()
}

/// The WoW HumanMale chest (`dsr-cache` `bonemap::HUMAN_MALE`): the bone whose average sideways
/// lean the gait is judged by.
const TORSO: usize = 22;

/// The roll about the forward axis (-Z) that brings the chest's loop-average up direction onto
/// the body's median plane: positive leans the top toward the body's left (-X), as
/// [`crate::lively::lean_rotation`] has it. Zero for a clip that is not on the human skeleton or
/// that has no frames. Only the sideways share is taken: the forward hunch is the clip's own.
pub fn straighten_of(clip: &wscl::Clip) -> f32 {
    if TORSO >= clip.bones() || clip.frames.is_empty() {
        return 0.0;
    }
    let mut chain = vec![TORSO];
    while let Ok(p) = usize::try_from(clip.parents[*chain.last().expect("never empty")]) {
        if chain.len() > clip.bones() {
            return 0.0;
        }
        chain.push(p);
    }
    let mut up = Vec3::ZERO;
    for frame in &clip.frames {
        let rotation = chain.iter().rev().fold(Quat::IDENTITY, |q, &b| {
            q * Quat::from_array(frame[b].r).normalize()
        });
        up += rotation * Vec3::Y;
    }
    // Rz(theta) takes (x, y) to (x cos - y sin, ..): zero x is `tan theta = x / y`. A torso
    // that lies more than a quarter turn over has no sideways to speak of.
    if up.y <= 1.0e-3 {
        return 0.0;
    }
    up.x.atan2(up.y)
}

/// Sprint's anatomical torso line, not the chest bone's local up. The two are not parallel:
/// splitting a chest-orientation correction across spine/chest leaves the trunk visibly leaning.
/// Applying this constant bias at the spine base preserves forward hunch and cyclic sway.
fn sprint_straighten_of(clip: &wscl::Clip) -> f32 {
    const SPINE: usize = 20;
    const NECK: usize = 48;
    if clip.bones() <= NECK {
        return 0.0;
    }
    let mut up = Vec3::ZERO;
    for frame in &clip.frames {
        let mut model = Vec::<Mat4>::with_capacity(frame.len());
        for (bone, local) in frame.iter().enumerate() {
            let local = Mat4::from_scale_rotation_translation(
                Vec3::from_array(local.s),
                Quat::from_array(local.r).normalize(),
                Vec3::from_array(local.t),
            );
            let parent = usize::try_from(clip.parents[bone]).ok();
            if parent.is_some_and(|p| p >= bone) {
                return 0.0;
            }
            model.push(parent.map_or(local, |p| model[p] * local));
        }
        up += model[NECK].transform_point3(Vec3::ZERO) - model[SPINE].transform_point3(Vec3::ZERO);
    }
    if up.y > 1.0e-3 {
        up.x.atan2(up.y)
    } else {
        0.0
    }
}

impl Clip {
    /// Measures a clip pair: its stride from the root motion, its foot-downs from the legs.
    pub fn new(role: Role, wow: wscl::Clip, reference: wscl::Clip) -> Result<Clip, String> {
        let duration = wow.duration();
        if duration <= 0.0 || !duration.is_finite() {
            return Err(format!("{}: no duration", wow.name));
        }
        let stride = match role.dir() {
            Dir::Forward | Dir::Back => motion_clip::progress(&wow, duration).abs(),
            Dir::Left | Dir::Right => motion_clip::root_offset(&wow, duration).x.abs(),
        };
        if stride < 0.25 {
            return Err(format!(
                "{} ({}): covers {stride:.2} yd a loop, that is no gait",
                wow.name,
                role.name()
            ));
        }
        let plants = foot_downs(&wow);
        let closure = closure_of(&wow);
        let straighten = if role == Role::Sprint {
            sprint_straighten_of(&wow)
        } else {
            straighten_of(&wow)
        };
        Ok(Clip {
            role,
            wow,
            reference,
            stride,
            plants,
            straighten,
            closure,
        })
    }

    /// The worst rotation (degrees) the loop's closure turns a bone by.
    pub fn seam_degrees(&self) -> f32 {
        self.closure
            .iter()
            .map(|(_, q)| q.angle_between(Quat::IDENTITY).to_degrees())
            .fold(0.0, f32::max)
    }

    /// Yards per second the clip covers at its own rate.
    pub fn pace(&self) -> f32 {
        self.stride / self.wow.duration()
    }
}

/// Where each ankle is on the ground plane on every frame of `clip` (yards, with the root's
/// travel added, so a planted foot holds still in it), left then right; empty for a clip that is
/// not on the human skeleton.
fn ankle_tracks(clip: &wscl::Clip) -> [Vec<Vec2>; 2] {
    if ANKLES.iter().any(|&b| b >= clip.bones()) || clip.root_motion.len() != clip.frames.len() {
        return [Vec::new(), Vec::new()];
    }
    let model_position = |frame: &[wscl::Trs], bone: usize| {
        let mut chain = vec![bone];
        while let Ok(p) = usize::try_from(clip.parents[*chain.last().expect("never empty")]) {
            if chain.len() > clip.bones() {
                break;
            }
            chain.push(p);
        }
        chain
            .iter()
            .rev()
            .fold(Mat4::IDENTITY, |m, &b| {
                let t = frame[b];
                m * Mat4::from_scale_rotation_translation(
                    Vec3::from_array(t.s),
                    Quat::from_array(t.r).normalize(),
                    Vec3::from_array(t.t),
                )
            })
            .transform_point3(Vec3::ZERO)
    };
    ANKLES.map(|bone| {
        clip.frames
            .iter()
            .zip(&clip.root_motion)
            .map(|(frame, root)| {
                let p = model_position(frame, bone);
                Vec2::new(p.x + root[0], p.z + root[2])
            })
            .collect()
    })
}

/// The phases (0..1) at which a foot comes down in one loop, from its track over the ground
/// (`ankle_tracks`: the first and the last point are one instant, `fps` points a second): the
/// first interval of each stretch the foot spends holding still, slower than a share of `pace`.
pub fn plants_in(track: &[Vec2], fps: f32, pace: f32) -> Vec<f32> {
    let Some(n) = track.len().checked_sub(1).filter(|n| *n >= 3) else {
        return Vec::new();
    };
    if !(fps > 0.0 && pace > 0.0) {
        return Vec::new();
    }
    let still = |i: usize| {
        let (a, b) = (track[i % n], track[i % n + 1]);
        (b - a).length() * fps < PLANTED_SHARE * pace
    };
    (0..n)
        .filter(|&i| still(i) && still(i + 1) && !still(i + n - 1))
        .map(|i| i as f32 / n as f32)
        .collect()
}

/// Both feet's foot-downs in one loop of `clip`, in phase order.
fn foot_downs(clip: &wscl::Clip) -> Vec<f32> {
    let pace = if clip.duration() > 0.0 {
        (clip.root_motion.last().zip(clip.root_motion.first()))
            .map_or(0.0, |(l, f)| Vec2::new(l[0] - f[0], l[2] - f[2]).length())
            / clip.duration()
    } else {
        0.0
    };
    let mut out: Vec<f32> = ankle_tracks(clip)
        .iter()
        .flat_map(|track| plants_in(track, clip.fps, pace))
        .collect();
    out.sort_by(f32::total_cmp);
    out
}

/// One weapon motion set's gait.
#[derive(Debug)]
pub struct Kit {
    clips: Vec<Arc<Clip>>,
}

impl Kit {
    pub fn get(&self, role: Role) -> Option<&Clip> {
        self.clips
            .iter()
            .find(|c| c.role == role)
            .map(|c| c.as_ref())
    }

    /// The bone count the kit's clips share.
    pub fn bones(&self) -> usize {
        self.clips.first().map_or(0, |c| c.wow.bones())
    }

    /// The fastest pace in the kit (yd/s).
    fn top_pace(&self) -> f32 {
        self.clips.iter().map(|c| c.pace()).fold(0.0, f32::max)
    }
}

/// Every kit the cache holds, by motion set number (`a12` is 12).
#[derive(Resource, Debug, Default)]
pub struct LocomotionClips {
    sets: BTreeMap<u8, Arc<Kit>>,
}

impl LocomotionClips {
    /// The kit of a motion set, else the common one.
    pub fn kit_for(&self, set: u8) -> Option<&Arc<Kit>> {
        self.sets.get(&set).or_else(|| self.sets.get(&0))
    }

    /// The worst rotation (degrees) any clip's loop closure turns a bone by.
    pub fn worst_seam(&self) -> f32 {
        self.sets
            .values()
            .flat_map(|k| k.clips.iter())
            .map(|c| c.seam_degrees())
            .fold(0.0, f32::max)
    }
}

/// The motion set of an idle clip's name (`great_weapon_idle` is 12); the common set without one.
pub fn motion_set(idle: Option<&str>) -> u8 {
    match idle {
        Some("shield_idle") => 10,
        Some("great_weapon_idle") => 12,
        Some("staff_idle") => 13,
        Some("polearm_idle") => 14,
        Some("bow_idle") => 16,
        _ => 0,
    }
}

/// The kits `locomotion.txt` in `dir` names, each with all eleven roles on a rig of `bones`.
pub fn load(dir: &Path, bones: usize) -> Result<LocomotionClips, String> {
    let text = std::fs::read_to_string(dir.join("locomotion.txt")).map_err(|e| e.to_string())?;
    if text.lines().next() != Some("locomotion-version 1") {
        return Err("wrong locomotion cache version".into());
    }
    // Validate the same project format versions as the combat library.
    motion_clip::checked_manifest(dir)?;
    let mut wanted: BTreeMap<u8, Vec<(Role, String)>> = BTreeMap::new();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let ["loco", set, role, clip] = w[..] else {
            continue;
        };
        let set: u8 = set.parse().map_err(|_| format!("bad loco line {line:?}"))?;
        let role = Role::from_name(role).ok_or_else(|| format!("unknown gait role in {line:?}"))?;
        wanted
            .entry(set)
            .or_default()
            .push((role, clip.to_string()));
    }
    let mut shared: HashMap<String, Arc<Clip>> = HashMap::new();
    let mut sets = BTreeMap::new();
    let mut editable = 0;
    for (set, roles) in wanted {
        let mut clips = Vec::new();
        for role in Role::ALL {
            let name = roles
                .iter()
                .find(|(r, _)| *r == role)
                .map(|(_, n)| n)
                .ok_or_else(|| format!("set a{set:02} lacks the {} clip", role.name()))?;
            let key = format!("{name}/{}", role.name());
            if let Some(c) = shared.get(&key) {
                clips.push(c.clone());
                continue;
            }
            // An editable clip of the same name (`anim/source`, [`crate::animation_library`]) is played in place of the
            // cache's.
            let wow = match crate::animation_library::load(name) {
                Some(Ok(clip)) => {
                    editable += 1;
                    clip
                }
                Some(Err(why)) => return Err(why),
                None => motion_clip::read_clip(dir, "human_male", name)?,
            };
            let reference = motion_clip::read_clip(dir, "human_reference", name)
                .unwrap_or_else(|_| wow.clone());
            if wow.bones() != bones {
                return Err(format!(
                    "{name}: {} bones, the rig has {bones}",
                    wow.bones()
                ));
            }
            let clip = Arc::new(Clip::new(role, wow, reference)?);
            shared.insert(key, clip.clone());
            clips.push(clip);
        }
        sets.insert(set, Arc::new(Kit { clips }));
    }
    if editable > 0 {
        info!(
            "locomotion: {editable} of the gait clips from their editable text ({})",
            crate::animation_library::anim_dir().display()
        );
    }
    if sets.is_empty() {
        return Err("locomotion.txt names no set".into());
    }
    Ok(LocomotionClips { sets })
}

// ---- the blend --------------------------------------------------------------------------------

/// The most clips one blend holds: two speeds of two directions.
const MAX_SHARES: usize = 4;

/// One clip's share of a blend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Share {
    pub role: Role,
    pub weight: f32,
}

/// What the stride is made of: weights that sum to 1, and the yards one cycle of the blend covers.
#[derive(Debug, Clone, PartialEq)]
pub struct Mix {
    shares: [Option<Share>; MAX_SHARES],
    pub stride: f32,
}

impl Mix {
    pub fn shares(&self) -> impl Iterator<Item = Share> + '_ {
        self.shares.iter().flatten().copied()
    }

    /// The clip with the largest share.
    pub fn dominant(&self) -> Option<Role> {
        self.shares()
            .max_by(|a, b| a.weight.total_cmp(&b.weight))
            .map(|s| s.role)
    }
}

/// Adds the blend of `roles` (slowest first) at `speed` to `out`, scaled by `scale`: the slower
/// clip below its pace, the faster above its own, the two between.
fn speed_blend(kit: &Kit, roles: &[Role], speed: f32, scale: f32, out: &mut Vec<(Role, f32)>) {
    let mut paced: Vec<(f32, Role)> = roles
        .iter()
        .filter_map(|&r| kit.get(r).map(|c| (c.pace(), r)))
        .collect();
    paced.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (Some(&(slow, slow_role)), Some(&(fast, fast_role))) = (paced.first(), paced.last()) else {
        return;
    };
    if speed <= slow || paced.len() == 1 {
        out.push((slow_role, scale));
    } else if speed >= fast {
        out.push((fast_role, scale));
    } else {
        // The pair of neighbours the speed lies between.
        let hi = paced.iter().position(|(p, _)| *p >= speed).unwrap_or(0);
        let ((a, ra), (b, rb)) = (paced[hi - 1], paced[hi]);
        let t = (speed - a) / (b - a);
        out.push((ra, scale * (1.0 - t)));
        out.push((rb, scale * t));
    }
}

/// The two nearest of the four directions for a step `(right, forward)`, as weights (cosine and
/// sine of the angle to them, normalized): straight ahead is one clip, a diagonal two halves.
pub fn direction_weights(local: Vec2) -> [(Dir, f32); 2] {
    let n = local.try_normalize().unwrap_or(Vec2::Y);
    let (ahead, side) = (n.y, n.x);
    let long = if ahead >= 0.0 {
        Dir::Forward
    } else {
        Dir::Back
    };
    let wide = if side >= 0.0 { Dir::Right } else { Dir::Left };
    let sum = ahead.abs() + side.abs();
    [(long, ahead.abs() / sum), (wide, side.abs() / sum)]
}

/// The blend for a body going `speed` yd/s: free (`local` is `None`: forward clips by speed) or
/// locked on (`local`: the step's direction `(right, forward)` in the body's frame).
pub fn mix(kit: &Kit, speed: f32, local: Option<Vec2>) -> Option<Mix> {
    let mut want: Vec<(Role, f32)> = Vec::with_capacity(MAX_SHARES);
    match local {
        None => speed_blend(
            kit,
            &[Role::Walk, Role::Run, Role::Sprint],
            speed,
            1.0,
            &mut want,
        ),
        Some(step) => {
            for (dir, w) in direction_weights(step) {
                if w > 1.0e-4 {
                    speed_blend(
                        kit,
                        &[Role::LockWalk(dir), Role::LockRun(dir)],
                        speed,
                        w,
                        &mut want,
                    );
                }
            }
        }
    }
    let total: f32 = want.iter().map(|(_, w)| w).sum();
    if want.is_empty() || total <= 0.0 {
        return None;
    }
    let mut shares = [None; MAX_SHARES];
    let mut stride = 0.0;
    for (slot, (role, w)) in shares.iter_mut().zip(want.iter().filter(|(_, w)| *w > 0.0)) {
        let weight = w / total;
        stride += weight * kit.get(*role)?.stride;
        *slot = Some(Share {
            role: *role,
            weight,
        });
    }
    Some(Mix { shares, stride })
}

/// The stride phase after `dt` seconds at `speed` yd/s over a blend of `stride` yd a cycle.
pub fn advance(phase: f32, speed: f32, stride: f32, dt: f32) -> f32 {
    if !(stride > 0.0 && speed.is_finite() && dt.is_finite()) {
        return phase;
    }
    (phase + dt * speed.max(0.0) / stride).rem_euclid(1.0)
}

/// Whether the phase `at` was passed going from `before` to `after` (a wrap included).
pub fn crossed(before: f32, after: f32, at: f32) -> bool {
    if after >= before {
        at > before && at <= after
    } else {
        at > before || at <= after
    }
}

/// Moves `weight` toward `target` by `dt` over `over` seconds.
pub fn ease(weight: f32, target: f32, dt: f32, over: f32) -> f32 {
    let step = dt / over;
    weight + (target - weight).clamp(-step, step)
}

/// The blend's pose of `bone` at `phase`: translation, rotation, scale.
pub fn blended(kit: &Kit, mix: &Mix, phase: f32, bone: usize) -> (Vec3, Quat, Vec3) {
    let (mut t, mut s) = (Vec3::ZERO, Vec3::ZERO);
    let mut r = Vec4::ZERO;
    let mut first: Option<Vec4> = None;
    for share in mix.shares() {
        let Some(clip) = kit.get(share.role) else {
            continue;
        };
        let at = phase * clip.wow.duration();
        let (mut ti, mut ri, si) = sample(&clip.wow, at, bone);
        if let Some(&(dt, dq)) = clip.closure.get(bone) {
            ti += dt * phase;
            ri *= Quat::IDENTITY.slerp(dq, phase);
        }
        t += ti * share.weight;
        s += si * share.weight;
        // One hemisphere, so opposite signs of one rotation do not cancel.
        let mut q = Vec4::from(ri.to_array());
        match first {
            None => first = Some(q),
            Some(f) if f.dot(q) < 0.0 => q = -q,
            Some(_) => {}
        }
        r += q * share.weight;
    }
    let rotation = if r.length_squared() > 1.0e-12 {
        Quat::from_vec4(r).normalize()
    } else {
        Quat::IDENTITY
    };
    (t, rotation, s)
}

// ---- the system -------------------------------------------------------------------------------

/// The HumanMale spine base (`KeyBone` 4, SpineLow): benilla's upper-body animation nodes animate it
/// and everything below it in the tree, the masks that [`upper_body`] mirrors.
const UPPER_ROOT: usize = 20;

/// Which of `bones` bones hang from [`UPPER_ROOT`] (itself included). A rig without it has none.
fn upper_body(parents: &[i16], bones: usize) -> Vec<bool> {
    let mut upper = vec![false; bones];
    for (i, flag) in upper.iter_mut().enumerate() {
        let mut b = i;
        // A parent always comes before its child; the walk stops at a root or a bad index.
        for _ in 0..bones {
            if b == UPPER_ROOT {
                *flag = true;
                break;
            }
            match parents
                .get(b)
                .copied()
                .filter(|&p| p >= 0 && (p as usize) < bones)
            {
                Some(p) => b = p as usize,
                None => break,
            }
        }
    }
    upper
}

/// The avatar's gait: where it is in its stride and how much of the pose it owns.
#[derive(Component, Debug, Default)]
pub struct GaitState {
    /// Where in the stride cycle the body is, 0..1.
    pub phase: f32,
    /// How much of the driven bones' pose the gait owns (eased): 0 standing, in the air, or in an
    /// action that poses the body itself.
    pub weight: f32,
    /// A Dark Souls kit is in use for this body, even at weight 0: the WoW gait's steps stay quiet
    /// and its legs are not borrowed by the stance.
    pub kit: bool,
    /// The roll (radians, positive toward the left) that stands the blend's torso upright, from
    /// Non-sprint [`Clip::straighten`] by share, applied across spine/chest at the gait's weight.
    pub straighten: f32,
    /// Sprint's anatomical correction, applied once at the spine base at its blend/gait weights.
    pub sprint_straighten: f32,
    /// How much of the upper body (the spine base and all above it) an upper-body cast has taken over
    /// ([`crate::abilities::moves::UpperCast`]): the gait keeps the legs and gives that much of the rest up.
    pub upper: f32,
    /// Which bones are the upper body's, worked out once from the rig.
    upper_bones: Vec<bool>,
    /// The achieved velocity, averaged (yd/s).
    smoothed: Vec3,
    moving: bool,
    /// Seconds off the ground, and whether the last time off it was a jump.
    airborne: f32,
    jumping: bool,
}

impl GaitState {
    /// Whether the WoW gait clip's own foot-downs should still sound.
    pub fn owns_steps(&self) -> bool {
        self.kit && self.weight > 0.01
    }

    /// The gait's weight over the torso: its weight less what an upper-body cast has taken.
    pub fn torso_weight(&self) -> f32 {
        self.weight.clamp(0.0, 1.0) * (1.0 - self.upper.clamp(0.0, 1.0))
    }
}

/// Planar speed over which a body counts as moving, with the lower bound it keeps moving down to
/// (`actor::anim`'s walk thresholds).
const MOVING_ON: f32 = 0.6;
const MOVING_OFF: f32 = 0.3;
/// The average of the achieved velocity follows a real change in about 80 ms.
const SMOOTH_RATE: f32 = 12.0;
/// A gait never cycles faster than this many times the fastest clip's own pace (a charge at
/// 24 yd/s is a sprint with feet that cannot keep up, not a flicker).
const CADENCE_CAP: f32 = 1.35;
/// A body that loses the ground keeps its stride this long before the jump clips take over.
const AIR_GRACE: f32 = 0.25;

/// Points the kit of the held weapon's motion set at the loaded moves.
fn attach(loco: Option<Res<LocomotionClips>>, moves: Option<ResMut<WeaponClips>>) {
    let (Some(loco), Some(mut moves)) = (loco, moves) else {
        return;
    };
    let set = motion_set(moves.idle.as_ref().map(|(wow, _)| wow.name.as_str()));
    let want = loco.kit_for(set);
    let same = match (moves.loco.as_ref(), want) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same {
        // Not a change of the moves: the stance and the clip layers need not start over.
        moves.bypass_change_detection().loco = want.cloned();
    }
}

fn startup(
    mut commands: Commands,
    roll: Option<Res<motion_clip::EvadeClips>>,
    moves: Option<ResMut<WeaponClips>>,
) {
    let Some(roll) = roll else { return };
    match load(&motion_clip::cache_dir(), roll.wow.bones()) {
        Ok(loco) => {
            // The base moves have no idle clip, so the common set's gait: the family chosen
            // later takes its own through `attach`.
            if let Some(mut moves) = moves {
                moves.loco = loco.kit_for(0).cloned();
            }
            info!(
                "locomotion: Dark Souls gait for {} weapon motion sets ({}), worst loop seam {:.1} deg",
                loco.sets.len(),
                loco.sets
                    .keys()
                    .map(|s| format!("a{s:02}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                loco.worst_seam()
            );
            commands.insert_resource(loco);
        }
        Err(why) => {
            info!("locomotion: no Dark Souls gait ({why}); WoW's walk and run stay");
        }
    }
}

/// The base layer of the pose: the gait's blend over the rig's own pose, at the weight the body
/// has earned (grounded, moving, not in an action that poses it).
#[allow(clippy::type_complexity)]
fn drive(
    mut commands: Commands,
    time: Res<Time>,
    lock: Res<crate::lockon::LockOn>,
    moves: Option<Res<WeaponClips>>,
    view: Option<Res<crate::rig_view::RigView>>,
    mut sounds: MessageWriter<AnimSound>,
    mut avatar: Query<
        (
            Entity,
            &Action,
            &mut RigPose,
            &Motor,
            &Transform,
            &GlobalTransform,
            Option<&mut GaitState>,
            Has<crate::abilities::moves::Casting>,
            Option<&crate::abilities::moves::UpperCast>,
        ),
        With<Avatar>,
    >,
) {
    let dt = time.delta_secs();
    for (entity, action, mut pose, motor, xf, world, gait, casting, upper_cast) in &mut avatar {
        let Some(mut gait) = gait else {
            commands.entity(entity).insert(GaitState::default());
            continue;
        };
        let kit = moves
            .as_deref()
            .and_then(|m| m.loco.as_deref())
            .filter(|k| k.bones() == pose.locals.len() && view.is_none());
        let Some(kit) = kit else {
            gait.kit = false;
            gait.weight = 0.0;
            gait.straighten = 0.0;
            gait.sprint_straighten = 0.0;
            continue;
        };
        gait.kit = true;
        gait.upper = upper_cast.map_or(0.0, crate::abilities::moves::UpperCast::share);
        if gait.upper_bones.len() != pose.locals.len() {
            gait.upper_bones = upper_body(&pose.parents, pose.locals.len());
        }
        let planar = Vec3::new(motor.velocity.x, 0.0, motor.velocity.z);
        let k = 1.0 - (-SMOOTH_RATE * dt).exp();
        gait.smoothed = gait.smoothed.lerp(planar, k);
        let speed = gait.smoothed.length();
        gait.moving = speed >= if gait.moving { MOVING_OFF } else { MOVING_ON };
        // Over a bump the feet leave the ground for a few frames and the stride goes on (the WoW
        // gait's `FALL_DELAY`); a jump, or a fall that lasts, hands the body to the jump clips.
        if motor.grounded {
            gait.airborne = 0.0;
            gait.jumping = false;
        } else {
            gait.airborne += dt;
        }
        gait.jumping |= motor.last.jumped;
        let aloft = gait.jumping || gait.airborne >= AIR_GRACE;
        // An action poses the body itself (a swing, a roll, a flinch, a cast), except a guard,
        // which is raised over walking legs.
        let free =
            !casting && !aloft && (matches!(action.state, ActionState::Idle) || action.guarding());
        let target = if free && gait.moving { 1.0 } else { 0.0 };
        gait.weight = ease(gait.weight, target, dt, motion_clip::BLEND_OUT);
        // Locked on, the steps follow where the body goes in its own frame.
        let local = lock.target.is_some().then(|| {
            let l = xf.rotation.inverse() * gait.smoothed;
            Vec2::new(l.x, -l.z)
        });
        let Some(blend) = mix(kit, speed, local) else {
            continue;
        };
        (gait.straighten, gait.sprint_straighten) =
            blend.shares().fold((0.0, 0.0), |(other, sprint), share| {
                let correction = kit.get(share.role).map_or(0.0, |c| c.straighten) * share.weight;
                if share.role == Role::Sprint {
                    (other, sprint + correction)
                } else {
                    (other + correction, sprint)
                }
            });
        let before = gait.phase;
        let cadence = speed.min(CADENCE_CAP * kit.top_pace());
        if gait.moving {
            gait.phase = advance(before, cadence, blend.stride, dt);
        }
        if gait.weight > 0.5 && gait.moving {
            if let Some(clip) = blend.dominant().and_then(|r| kit.get(r)) {
                for &down in &clip.plants {
                    if crossed(before, gait.phase, down) {
                        sounds.write(AnimSound {
                            entity,
                            ident: *b"$FSD",
                            at: world.translation(),
                        });
                    }
                }
            }
        }
        if gait.weight <= 0.0 {
            continue;
        }
        let Some(driven) = blend
            .dominant()
            .and_then(|r| kit.get(r))
            .map(|c| c.wow.driven.as_slice())
        else {
            continue;
        };
        let w = gait.weight.min(1.0);
        for (i, local) in pose.locals.iter_mut().enumerate() {
            if !driven.get(i).copied().unwrap_or(false) {
                continue;
            }
            // An upper-body cast has the torso, arms and head by its share; the legs stay the gait's.
            let w = if gait.upper_bones.get(i).copied().unwrap_or(false) {
                w * (1.0 - gait.upper)
            } else {
                w
            };
            if w <= 0.0 {
                continue;
            }
            let (t, r, s) = blended(kit, &blend, gait.phase, i);
            *local = if w >= 1.0 {
                Transform {
                    translation: t,
                    rotation: r,
                    scale: s,
                }
            } else {
                Transform {
                    translation: local.translation.lerp(t, w),
                    rotation: local.rotation.slerp(r, w),
                    scale: local.scale.lerp(s, w),
                }
            };
        }
        pose.pose_dirty = true;
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, startup.after(motion_clip::startup))
        .add_systems(
            PostUpdate,
            (attach, drive)
                .chain()
                .in_set(PosePost)
                .before(motion_clip::ClipLayers),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_upper_body_is_the_spine_base_and_all_that_hangs_from_it() {
        // 0 the root, 1 a hip, 2 a knee under it, 20 the spine base on the root, 21 a chest on it, 22 an
        // arm on the chest; the rest hang from the root.
        let mut parents = vec![0i16; 23];
        parents[0] = -1;
        parents[2] = 1;
        parents[20] = 0;
        parents[21] = 20;
        parents[22] = 21;
        let upper = upper_body(&parents, 23);
        assert!(upper[20] && upper[21] && upper[22]);
        assert!(
            !upper[0] && !upper[1] && !upper[2],
            "the root and the legs stay the gait's"
        );
        assert!(
            upper_body(&[-1, 0, 1], 3).iter().all(|u| !u),
            "a small rig has none"
        );
        let g = GaitState {
            weight: 1.0,
            upper: 0.75,
            ..GaitState::default()
        };
        assert!((g.torso_weight() - 0.25).abs() < 1.0e-6);
    }

    /// A clip of `frames` frames on a 3-bone chain that travels `stride` yd forward (-Z) over
    /// `secs`, at 30 fps.
    fn gait_clip(name: &str, secs: f32, stride: f32, side: f32) -> wscl::Clip {
        let n = (secs * 30.0).round() as usize + 1;
        wscl::Clip {
            name: name.into(),
            fps: 30.0,
            model_space: false,
            parents: vec![-1, 0, 1],
            driven: vec![false, true, true],
            frames: vec![vec![wscl::Trs::IDENTITY; 3]; n],
            root_motion: (0..n)
                .map(|i| {
                    let u = i as f32 / (n - 1) as f32;
                    [side * u, 0.0, -stride * u, 0.0]
                })
                .collect(),
            flags: Vec::new(),
            hits: Vec::new(),
        }
    }

    /// A kit whose walk, run and sprint pace 2, 4 and 7 yd/s, and whose locked walks and runs go
    /// 2 and 4 in every direction.
    fn kit() -> Kit {
        let mut clips = Vec::new();
        for role in Role::ALL {
            let (secs, pace) = match role {
                Role::Walk | Role::LockWalk(_) => (1.0, 2.0),
                Role::Run | Role::LockRun(_) => (0.8, 4.0),
                Role::Sprint => (0.6, 7.0),
            };
            let (stride, side) = match role.dir() {
                Dir::Forward => (pace * secs, 0.0),
                Dir::Back => (-pace * secs, 0.0),
                Dir::Left => (0.0, -pace * secs),
                Dir::Right => (0.0, pace * secs),
            };
            let wow = gait_clip(role.name(), secs, stride, side);
            clips.push(Arc::new(
                Clip::new(role, wow.clone(), wow).expect("a gait clip"),
            ));
        }
        Kit { clips }
    }

    fn weights(m: &Mix) -> Vec<(Role, f32)> {
        m.shares().map(|s| (s.role, s.weight)).collect()
    }

    /// A clip on the human's 23 chest bones in a chain whose chest (22) carries `torso` every
    /// frame, and the bones above the chest none.
    fn chest_clip(torso: Quat) -> wscl::Clip {
        let mut clip = gait_clip("sprint", 0.5, 3.0, 0.0);
        clip.parents = (0..23).map(|i| i - 1).collect();
        clip.driven = vec![true; 23];
        let mut frame = vec![wscl::Trs::IDENTITY; 23];
        frame[22].r = torso.to_array();
        clip.frames = vec![frame; 16];
        clip
    }

    #[test]
    fn a_torso_carried_to_one_side_is_straightened_by_the_sideways_share_only() {
        // Hunched 55 degrees forward and 13 to the left, the sprint's pose: the correction is a
        // roll to the right that puts the chest back over the median plane, whatever the hunch.
        let pitch = Quat::from_rotation_x(-55.0f32.to_radians());
        let lean = Quat::from_rotation_z(13.0f32.to_radians());
        let torso = lean * pitch;
        let fix = straighten_of(&chest_clip(torso));
        assert!(fix < 0.0, "{fix}");
        let after = Quat::from_rotation_z(fix) * torso * Vec3::Y;
        assert!(after.x.abs() < 1.0e-4, "{after:?}");
        // The forward hunch is the clip's own: up's forward share does not change.
        let before = torso * Vec3::Y;
        assert!((after.z - before.z).abs() < 1.0e-4, "{after:?} {before:?}");
        // Mirror: a lean to the right is given back to the left.
        let mirror = straighten_of(&chest_clip(lean.inverse() * pitch));
        assert!(
            mirror > 0.0 && (mirror + fix).abs() < 0.02,
            "{mirror} {fix}"
        );
    }

    #[test]
    fn a_torso_that_is_square_is_left_alone_and_a_stub_skeleton_has_nothing_to_straighten() {
        let pitch = Quat::from_rotation_x(-0.9);
        assert!(straighten_of(&chest_clip(pitch)).abs() < 1.0e-5);
        assert_eq!(straighten_of(&gait_clip("walk", 1.0, 2.0, 0.0)), 0.0);
        // A torso lying on its back (up pointing down) has no sideways to give back.
        assert_eq!(straighten_of(&chest_clip(Quat::from_rotation_x(3.0))), 0.0);
    }

    #[test]
    fn the_project_sprint_centers_the_actual_torso_without_changing_its_forward_hunch() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source/body_sprint.wsanim");
        let clip = crate::animation_library::read(&path).unwrap();
        let gait = Clip::new(Role::Sprint, clip.clone(), clip).unwrap();
        let mut before = Vec3::ZERO;
        for frame in &gait.wow.frames {
            let mut model = Vec::<Mat4>::new();
            for (bone, local) in frame.iter().enumerate() {
                let local = Mat4::from_scale_rotation_translation(
                    Vec3::from_array(local.s),
                    Quat::from_array(local.r),
                    Vec3::from_array(local.t),
                );
                model.push(
                    usize::try_from(gait.wow.parents[bone])
                        .map_or(local, |parent| model[parent] * local),
                );
            }
            before +=
                model[48].transform_point3(Vec3::ZERO) - model[20].transform_point3(Vec3::ZERO);
        }
        let after = Quat::from_rotation_z(gait.straighten) * before;
        assert!(
            after.x.abs() < 1.0e-5,
            "torso remains off-center: {after:?}"
        );
        assert!((after.z - before.z).abs() < 1.0e-5, "forward hunch changed");
    }

    #[test]
    fn the_roles_have_the_extractors_names() {
        for role in Role::ALL {
            assert_eq!(Role::from_name(role.name()), Some(role));
        }
        // The manifest's names, as `dsr-cache` `loco::ROLES` writes them.
        for name in [
            "walk",
            "run",
            "sprint",
            "lock_walk_f",
            "lock_walk_b",
            "lock_walk_l",
            "lock_walk_r",
            "lock_run_f",
            "lock_run_b",
            "lock_run_l",
            "lock_run_r",
        ] {
            assert!(Role::from_name(name).is_some(), "{name}");
        }
        assert_eq!(Role::from_name("crawl"), None);
    }

    #[test]
    fn a_clips_stride_and_pace_come_from_its_root_motion() {
        let k = kit();
        let walk = k.get(Role::Walk).unwrap();
        assert!((walk.stride - 2.0).abs() < 1.0e-4);
        assert!((walk.pace() - 2.0).abs() < 1.0e-4);
        // Going backwards or sideways is a stride all the same, a positive one.
        assert!((k.get(Role::LockRun(Dir::Back)).unwrap().stride - 3.2).abs() < 1.0e-4);
        assert!((k.get(Role::LockWalk(Dir::Left)).unwrap().stride - 2.0).abs() < 1.0e-4);
        // A clip that does not go anywhere is no gait.
        let still = gait_clip("still", 1.0, 0.0, 0.0);
        assert!(Clip::new(Role::Walk, still.clone(), still).is_err());
    }

    #[test]
    fn free_the_speed_blends_walk_run_and_sprint() {
        let k = kit();
        let one = |speed| weights(&mix(&k, speed, None).unwrap());
        // Slower than the walk: the walk, played slower.
        assert_eq!(one(0.5), vec![(Role::Walk, 1.0)]);
        assert_eq!(one(2.0), vec![(Role::Walk, 1.0)]);
        // Between two paces: both, by how far along.
        let m = one(3.0);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].0, Role::Walk);
        assert!(
            (m[0].1 - 0.5).abs() < 1.0e-5 && (m[1].1 - 0.5).abs() < 1.0e-5,
            "{m:?}"
        );
        assert_eq!(m[1].0, Role::Run);
        let m = one(5.5);
        assert_eq!((m[0].0, m[1].0), (Role::Run, Role::Sprint));
        assert!((m[1].1 - 0.5).abs() < 1.0e-5, "{m:?}");
        // At and past the sprint: the sprint.
        assert_eq!(one(7.0), vec![(Role::Sprint, 1.0)]);
        assert_eq!(one(24.0), vec![(Role::Sprint, 1.0)]);
    }

    #[test]
    fn locked_on_the_step_blends_its_two_nearest_directions() {
        let k = kit();
        let one = |step: Vec2, speed| weights(&mix(&k, speed, Some(step)).unwrap());
        // Straight ahead at a walk's pace: the forward locked walk alone.
        assert_eq!(one(Vec2::Y, 2.0), vec![(Role::LockWalk(Dir::Forward), 1.0)]);
        // Backing off at a run's pace: the backward locked run.
        assert_eq!(one(-Vec2::Y, 4.0), vec![(Role::LockRun(Dir::Back), 1.0)]);
        // Between the two speeds, both sets of one direction.
        let m = one(Vec2::X, 3.0);
        assert_eq!(
            m.iter().map(|(r, _)| *r).collect::<Vec<_>>(),
            vec![Role::LockWalk(Dir::Right), Role::LockRun(Dir::Right)]
        );
        // A diagonal is two halves.
        let m = one(Vec2::new(1.0, -1.0), 4.0);
        assert_eq!(m.len(), 2);
        assert!(m.iter().all(|(_, w)| (w - 0.5).abs() < 1.0e-5), "{m:?}");
        // Weights always sum to one.
        for step in [
            Vec2::new(0.3, 0.9),
            Vec2::new(-0.8, -0.2),
            Vec2::new(0.1, -0.05),
        ] {
            for speed in [0.4, 2.5, 3.9, 9.0] {
                let sum: f32 = weights(&mix(&k, speed, Some(step)).unwrap())
                    .iter()
                    .map(|(_, w)| w)
                    .sum();
                assert!((sum - 1.0).abs() < 1.0e-5, "{step} at {speed}: {sum}");
            }
        }
    }

    #[test]
    fn the_stride_of_a_blend_is_the_weighted_stride() {
        let k = kit();
        // Walk 2.0 yd per 1.0 s and run 3.2 per 0.8 s: at 3.0 yd/s, half each.
        let m = mix(&k, 3.0, None).unwrap();
        assert!((m.stride - 2.6).abs() < 1.0e-4, "{}", m.stride);
        // Nearer one pace, that clip leads.
        assert_eq!(mix(&k, 2.4, None).unwrap().dominant(), Some(Role::Walk));
        assert_eq!(mix(&k, 3.6, None).unwrap().dominant(), Some(Role::Run));
    }

    #[test]
    fn the_feet_cover_the_ground_they_are_given() {
        // One stride per cycle, advanced by the ground covered over it: a cycle of a 2.6 yd
        // stride takes 2.6 / speed seconds whatever the blend.
        let mut phase = 0.0;
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        let mut cycles = 0;
        while t < 2.0 {
            let next = advance(phase, 3.0, 2.6, dt);
            if next < phase {
                cycles += 1;
            }
            phase = next;
            t += dt;
        }
        // 2 s at 3 yd/s is 6 yd of ground, 2.31 strides: two whole cycles done.
        assert_eq!(cycles, 2);
        // No stride or no speed leaves the phase where it was.
        assert_eq!(advance(0.4, 3.0, 0.0, dt), 0.4);
        assert_eq!(advance(0.4, 0.0, 2.6, dt), 0.4);
        assert_eq!(advance(0.4, f32::NAN, 2.6, dt), 0.4);
        // A phase never leaves 0..1.
        assert!((0.0..1.0).contains(&advance(0.99, 50.0, 2.0, 0.1)));
    }

    #[test]
    fn a_foot_down_is_heard_once_per_pass_and_across_the_wrap() {
        assert!(crossed(0.2, 0.3, 0.25));
        assert!(!crossed(0.2, 0.3, 0.35));
        assert!(
            !crossed(0.25, 0.3, 0.25),
            "the instant it stood on was heard"
        );
        assert!(crossed(0.2, 0.25, 0.25), "(before, after]");
        // Over the wrap: 0.9 -> 0.1 passes 0.95 and 0.05, not 0.5.
        assert!(crossed(0.9, 0.1, 0.95));
        assert!(crossed(0.9, 0.1, 0.05));
        assert!(!crossed(0.9, 0.1, 0.5));
    }

    #[test]
    fn the_weight_eases_in_and_out() {
        let dt = 1.0 / 60.0;
        let mut w = 0.0;
        for _ in 0..6 {
            w = ease(w, 1.0, dt, 0.2);
        }
        assert!(w > 0.4 && w < 0.6, "{w}");
        for _ in 0..30 {
            w = ease(w, 1.0, dt, 0.2);
        }
        assert_eq!(w, 1.0);
        for _ in 0..30 {
            w = ease(w, 0.0, dt, 0.2);
        }
        assert_eq!(w, 0.0);
    }

    #[test]
    fn a_blend_of_one_clip_is_that_clip_and_opposite_signs_do_not_cancel() {
        let mut k = kit();
        // Give the walk a turn about Y on bone 1, and the run the same turn written with the
        // other sign of its quaternion.
        let q = Quat::from_rotation_y(0.8);
        for (role, sign) in [(Role::Walk, 1.0), (Role::Run, -1.0)] {
            let c = Arc::get_mut(k.clips.iter_mut().find(|c| c.role == role).unwrap()).unwrap();
            for f in &mut c.wow.frames {
                f[1].r = (q * sign).to_array();
            }
        }
        let one = mix(&k, 1.0, None).unwrap();
        let (_, r, _) = blended(&k, &one, 0.3, 1);
        assert!(r.angle_between(q) < 1.0e-4);
        let both = mix(&k, 3.0, None).unwrap();
        assert_eq!(both.shares().count(), 2);
        let (_, r, _) = blended(&k, &both, 0.3, 1);
        assert!(
            r.angle_between(q) < 1.0e-4,
            "the halves of one rotation: {r:?}"
        );
    }

    #[test]
    fn a_loop_that_does_not_close_is_closed_over_the_cycle() {
        // The walk's last frame is turned 20 degrees about Z on bone 1 and 5 mm up on bone 2.
        let mut k = kit();
        let c = Arc::get_mut(k.clips.iter_mut().find(|c| c.role == Role::Walk).unwrap()).unwrap();
        let mut wow = c.wow.clone();
        let last = wow.frames.len() - 1;
        wow.frames[last][1].r = Quat::from_rotation_z(20f32.to_radians()).to_array();
        wow.frames[last][2].t = [0.0, 0.005, 0.0];
        *c = Clip::new(Role::Walk, wow, c.reference.clone()).unwrap();
        assert!(
            (c.seam_degrees() - 20.0).abs() < 0.01,
            "{}",
            c.seam_degrees()
        );
        let m = mix(&k, 1.0, None).unwrap();
        // No jump across the wrap: just before the end it is the first frame's pose again.
        let at = |phase| blended(&k, &m, phase, 1);
        assert!(at(0.9999).1.angle_between(at(0.0).1).to_degrees() < 0.1);
        assert!((at(0.9999).0 - at(0.0).0).length() < 1.0e-3);
        // Half way round, half the seam is spread in (the frames themselves are unturned).
        let half = at(0.5).1.angle_between(Quat::IDENTITY).to_degrees();
        assert!((half - 10.0).abs() < 0.1, "{half}");
        // A loop that closes is left as it is.
        let again = kit();
        assert!(again.get(Role::Run).unwrap().seam_degrees() < 1.0e-3);
    }

    /// A foot's track over the ground: still while `down` says so, else moving `2 * pace` ahead
    /// (it covers one stride of `2 * pace * 0.5 s` in its half of the loop).
    fn track(n: usize, fps: f32, pace: f32, down: impl Fn(f32) -> bool) -> Vec<Vec2> {
        let mut x = 0.0;
        let mut out = vec![Vec2::ZERO];
        for i in 0..n {
            let u = i as f32 / n as f32;
            if !down(u) {
                x += 2.0 * pace / fps;
            }
            out.push(Vec2::new(x, 0.0));
        }
        out
    }

    #[test]
    fn a_foot_comes_down_where_it_stops_moving_over_the_ground() {
        let (n, fps, pace) = (30, 30.0, 2.0);
        // Still from 0.25 to 0.75 of the loop: down at 0.25, one foot-down in the loop.
        let t = track(n, fps, pace, |u| (0.25..0.75).contains(&u));
        let p = plants_in(&t, fps, pace);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!((p[0] - 0.25).abs() < 1.5 / n as f32, "{p:?}");
        // A stance that wraps over the end of the loop is found at its start.
        let t = track(n, fps, pace, |u| !(0.2..0.7).contains(&u));
        let p = plants_in(&t, fps, pace);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!((p[0] - 0.7).abs() < 1.5 / n as f32, "{p:?}");
        // A foot that never holds still, or holds still all along, makes no step.
        assert!(plants_in(&track(n, fps, pace, |_| false), fps, pace).is_empty());
        assert!(plants_in(&track(n, fps, pace, |_| true), fps, pace).is_empty());
        // A single still interval is a pass of the swing, not a stance.
        let t = track(n, fps, pace, |u| (0.5..0.52).contains(&u));
        assert!(plants_in(&t, fps, pace).is_empty());
        // Nothing to measure.
        assert!(plants_in(&[], fps, pace).is_empty());
        assert!(plants_in(&t, fps, 0.0).is_empty());
    }

    #[test]
    fn the_motion_set_is_the_idle_clips() {
        assert_eq!(motion_set(Some("great_weapon_idle")), 12);
        assert_eq!(motion_set(Some("body_idle")), 0);
        assert_eq!(motion_set(Some("bow_idle")), 16);
        assert_eq!(motion_set(None), 0);
        assert_eq!(motion_set(Some("roll")), 0);
    }

    #[test]
    fn a_set_the_cache_lacks_borrows_the_common_one() {
        let common = Arc::new(kit());
        let mut loco = LocomotionClips::default();
        loco.sets.insert(0, common.clone());
        assert!(Arc::ptr_eq(loco.kit_for(14).unwrap(), &common));
        assert!(Arc::ptr_eq(loco.kit_for(0).unwrap(), &common));
        assert!(LocomotionClips::default().kit_for(0).is_none());
    }

    /// The tracked project library, checked on every test run.
    #[test]
    fn the_built_cache_loads_every_set_and_its_gaits_make_sense() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source");
        let dir = root.as_path();
        let roll = motion_clip::read_clip(dir, "human_male", "body_roll").unwrap();
        let loco = load(dir, roll.bones()).unwrap_or_else(|e| panic!("{e}"));
        for set in [0, 10, 12, 13, 14, 16] {
            let k = loco
                .sets
                .get(&set)
                .unwrap_or_else(|| panic!("set a{set:02}"));
            for role in Role::ALL {
                let c = k
                    .get(role)
                    .unwrap_or_else(|| panic!("a{set:02} {}", role.name()));
                // A loop: the source's first and last frame are nearly one pose (the closure
                // spreads what is left over the cycle).
                assert!(
                    c.seam_degrees() < 15.0,
                    "{}: seam {:.1} deg",
                    c.wow.name,
                    c.seam_degrees()
                );
                // A foot comes down twice a cycle.
                assert_eq!(c.plants.len(), 2, "{}: {:?}", c.wow.name, c.plants);
            }
            // Walking is slower than running, which is slower than sprinting.
            let (w, r, s) = (
                k.get(Role::Walk).unwrap().pace(),
                k.get(Role::Run).unwrap().pace(),
                k.get(Role::Sprint).unwrap().pace(),
            );
            assert!(w < r && r < s, "a{set:02}: {w} {r} {s}");
            assert!(
                (1.0..2.5).contains(&w) && (3.0..5.5).contains(&r),
                "a{set:02}"
            );
        }
    }
}
