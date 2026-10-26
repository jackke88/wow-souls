//! Life on top of the Dark Souls gait: the small things a body does that no clip authored for a
//! different skeleton carries. Three layers, all additive rotations on the upper body's bones
//! (never the legs, so no foot slides), applied after every clip layer:
//!
//! - **Lean**: the body leans into what it does. Speeding up pitches the spine forward, braking
//!   back (so a stop from a run does not snap from a hunched lean to upright in one frame), and a
//!   turn at speed banks it toward the inside. Read from the achieved velocity's acceleration,
//!   which [`crate::glide`] keeps bounded.
//! - **Head**: the head leads. Locked on, it looks at the target (the body strafes facing it
//!   anyway; the pitch is what shows); free, it looks where the body is going; standing, it looks
//!   where the camera does, within what a neck can do. Eased, so it arrives a beat before the body.
//! - **Straightening**: a gait clip that carries the trunk to one side the whole loop (the Dark
//!   Souls sprint) is given its sideways share back at the gait's weight. Sprint corrects the
//!   actual spine-to-neck line at the spine base; other gaits share their correction across the
//!   spine/chest. This correction also applies when `WOWSOULS_LIVELY=0`.
//! - Nothing else: no idle sway (the stance clip breathes) and no step bob (the gait clips carry
//!   theirs), measured: the clips' own torso swings 6 to 22 degrees a cycle.
//!
//! An action that poses the body itself (a swing, a roll, a flinch, a cast) takes all of it away,
//! so the blade's measured path is the clip's. `WOWSOULS_LIVELY=0` switches it off, a number
//! scales the amounts.

use benilla_world::rig_anim::{PosePost, RigPose};
use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::camera::OrbitCam;
use crate::combat::{Action, ActionState};
use crate::lockon::{LockOn, Lockable};
use crate::motor::Motor;

/// HumanMale bones (`dsr-cache` `bonemap::HUMAN_MALE`): the spine, the chest, the neck, the head.
const SPINE: usize = 20;
const CHEST: usize = 22;
const NECK: usize = 48;
const HEAD: usize = 64;

/// The lean's per-bone share of the whole: the chest carries most of it.
const LEAN_SHARES: [(usize, f32); 2] = [(SPINE, 0.35), (CHEST, 0.65)];
/// The look's per-bone share of the whole: the head turns more than the neck.
const LOOK_SHARES: [(usize, f32); 2] = [(NECK, 0.4), (HEAD, 0.6)];

/// Radians of pitch per yd/s² of acceleration along the facing, and the most it gives (forward,
/// back). A start at 40 yd/s² is 6 degrees, a stop at 60 is the 8 degree ceiling.
const PITCH_PER_ACCEL: f32 = 0.0026;
const PITCH_FORWARD_MAX: f32 = 0.105;
const PITCH_BACK_MAX: f32 = 0.14;
/// The same for the sideways acceleration (a turn at speed): 8 degrees at most toward the inside.
const ROLL_PER_ACCEL: f32 = 0.0035;
const ROLL_MAX: f32 = 0.14;
/// How quickly the lean follows its target (1/s): about 100 ms.
const LEAN_RATE: f32 = 10.0;
/// The achieved velocity's average, before the acceleration is read off it (1/s).
const VELOCITY_RATE: f32 = 18.0;

/// The most the head turns either way (rad) and tips (rad), and the camera angle past which the
/// head gives up and looks ahead.
const LOOK_YAW_MAX: f32 = 1.22;
const LOOK_GIVE_UP: f32 = 1.92;
const LOOK_PITCH_MAX: f32 = 0.45;
/// A target's pitch is followed this much.
const LOOK_PITCH_SHARE: f32 = 0.7;
/// How quickly the head follows (1/s): about 150 ms.
const LOOK_RATE: f32 = 6.5;
/// The speed (yd/s) above which the head looks along the travel rather than where the camera does.
const TRAVEL_LOOK: f32 = 1.0;
/// The head's height over the feet for the pitch to a target (yd).
const HEAD_HEIGHT: f32 = 1.6;

/// What the layers add, per frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Amounts {
    /// Tip forward (negative: back), banking toward the left (negative: right), in radians.
    pub pitch: f32,
    pub roll: f32,
    /// Head turn toward the left (negative: right) and tip up (negative: down).
    pub yaw: f32,
    pub tip: f32,
}

/// The avatar's eased amounts, and what it wrote last frame (to put back before writing again).
#[derive(Component, Debug, Default)]
pub struct Lively {
    pub amounts: Amounts,
    velocity: Vec3,
    seen: bool,
    /// Seconds since the trace line.
    traced: f32,
    /// Per bone written: the bone, its rotation before the edit and the one it was given.
    written: Vec<(usize, Quat, Quat)>,
}

/// The scale of every amount (`WOWSOULS_LIVELY`, 1 by default, 0 off).
pub fn scale() -> f32 {
    static SCALE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| {
        std::env::var("WOWSOULS_LIVELY")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .map_or(1.0, |v| v.min(3.0))
    })
}

/// The lean a body's acceleration asks for: `forward` along its facing, `left` across it (yd/s²).
/// Returns `(pitch, roll)`: forward and toward the left are positive.
pub fn lean_for(forward: f32, left: f32) -> (f32, f32) {
    let pitch = (forward * PITCH_PER_ACCEL).clamp(-PITCH_BACK_MAX, PITCH_FORWARD_MAX);
    let roll = (left * ROLL_PER_ACCEL).clamp(-ROLL_MAX, ROLL_MAX);
    (pitch, roll)
}

/// `current` moved toward `target` at `rate` (1/s), whatever the frame length.
pub fn ease(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    if !(dt > 0.0 && dt.is_finite()) {
        return current;
    }
    current + (target - current) * (1.0 - (-rate * dt).exp())
}

/// `a` wrapped into -pi..pi.
fn wrap(a: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    let a = a.rem_euclid(two_pi);
    if a > std::f32::consts::PI {
        a - two_pi
    } else {
        a
    }
}

/// The head's turn for a wish `rel` radians to the left of the body's facing: what a neck can do,
/// and nothing for a wish behind the shoulder (the head looks ahead rather than flip sides).
pub fn look_yaw(rel: f32) -> f32 {
    let rel = wrap(rel);
    if rel.abs() >= LOOK_GIVE_UP {
        0.0
    } else {
        rel.clamp(-LOOK_YAW_MAX, LOOK_YAW_MAX)
    }
}

/// The tip for a target `rise` yd above the head and `ahead` yd off in the ground plane.
pub fn look_tip(rise: f32, ahead: f32) -> f32 {
    (rise.atan2(ahead.max(0.1)) * LOOK_PITCH_SHARE).clamp(-LOOK_PITCH_MAX, LOOK_PITCH_MAX)
}

/// A rotation `delta`, given in the body's frame, as the bone's own local frame has it: the
/// bone's parent carries `parent` (model space), so the bone turns by `parent^-1 delta parent`.
pub fn in_parent(parent: Quat, delta: Quat) -> Quat {
    parent.inverse() * delta * parent
}

/// The body-frame rotation of a lean: `pitch` forward (about the body's side axis, +X turns the
/// top of the body backward in the rig's axes, so forward is negative), `roll` toward the left
/// (about the forward axis, +Z leans the top toward -X, the body's left).
pub fn lean_rotation(pitch: f32, roll: f32) -> Quat {
    Quat::from_rotation_z(roll) * Quat::from_rotation_x(-pitch)
}

/// The body-frame rotation of a look: `yaw` toward the left (about up), `tip` up (about the side
/// axis, +X lifts the face).
pub fn look_rotation(yaw: f32, tip: f32) -> Quat {
    Quat::from_rotation_y(yaw) * Quat::from_rotation_x(tip)
}

/// Puts back what this system wrote last frame where the pose has not been rewritten since (the
/// evaluator leaves a bone with no key in the playing clip as it was), so the edits never pile up.
fn undo(pose: &mut RigPose, lively: &mut Lively) {
    for (bone, before, after) in lively.written.drain(..) {
        if let Some(local) = pose.locals.get_mut(bone) {
            if local.rotation == after {
                local.rotation = before;
            }
        }
    }
}

/// `delta` (body frame) applied to `bone`, at `share` of its angle, remembering the change.
fn turn(pose: &mut RigPose, lively: &mut Lively, bone: usize, delta: Quat, share: f32) {
    if bone >= pose.locals.len() {
        return;
    }
    // PosePost precedes model composition: model still describes the previous pose, including
    // last frame's additive edits. Read the locals that the clip layers just wrote instead.
    let mut parent = Quat::IDENTITY;
    let mut ancestor = pose.parents[bone];
    for _ in 0..pose.locals.len() {
        let Ok(index) = usize::try_from(ancestor) else {
            break;
        };
        let Some(local) = pose.locals.get(index) else {
            return;
        };
        parent = local.rotation * parent;
        ancestor = pose.parents[index];
    }
    let part = Quat::IDENTITY.slerp(delta, share);
    let before = pose.locals[bone].rotation;
    let after = (in_parent(parent, part) * before).normalize();
    pose.locals[bone].rotation = after;
    lively.written.push((bone, before, after));
}

#[allow(clippy::type_complexity)]
fn drive(
    mut commands: Commands,
    time: Res<Time>,
    lock: Res<LockOn>,
    targets: Query<(&GlobalTransform, &Lockable)>,
    cam: Query<&OrbitCam>,
    mut avatar: Query<
        (
            Entity,
            &Action,
            &mut RigPose,
            &Motor,
            &Transform,
            Option<&crate::locomotion::GaitState>,
            Option<&mut Lively>,
            Has<crate::abilities::moves::Casting>,
        ),
        With<Avatar>,
    >,
) {
    let dt = time.delta_secs();
    let amount = scale();
    for (entity, action, mut pose, motor, xf, gait, lively, casting) in &mut avatar {
        let Some(mut lively) = lively else {
            commands.entity(entity).insert(Lively::default());
            continue;
        };
        undo(&mut pose, &mut lively);
        if dt <= 0.0 {
            continue;
        }
        let body_yaw = xf.rotation.to_euler(EulerRot::YXZ).0;
        let forward = *xf.forward();
        let left = *xf.left();
        // The achieved velocity's average, and the acceleration off it.
        let planar = Vec3::new(motor.velocity.x, 0.0, motor.velocity.z);
        let before = lively.velocity;
        let average = before.lerp(planar, 1.0 - (-VELOCITY_RATE * dt).exp());
        let accel = if lively.seen {
            (average - before) / dt
        } else {
            Vec3::ZERO
        };
        lively.seen = true;
        lively.velocity = average;
        let free = !casting
            && (matches!(action.state, ActionState::Idle) || action.guarding())
            && motor.grounded;
        // The lean is the gait's to give: it goes where the gait's weight does.
        let weight = gait.map_or(0.0, |g| g.weight.clamp(0.0, 1.0));
        // What the gait's clips carry to one side is given back whatever the flourish scale: a
        // crooked body is no style.
        let straighten = gait.map_or(0.0, |g| g.straighten * g.torso_weight());
        let sprint_straighten = gait.map_or(0.0, |g| g.sprint_straighten * g.torso_weight());
        let (pitch, roll) = lean_for(accel.dot(forward), accel.dot(left));
        let (pitch, roll) = if free {
            (pitch * weight, roll * weight)
        } else {
            (0.0, 0.0)
        };
        // Where the head wants to look.
        let (yaw, tip) = if !free {
            (0.0, 0.0)
        } else if let Some((at, l)) = lock
            .target
            .and_then(|t| targets.get(t).ok())
            .map(|(g, l)| (g.translation(), l))
        {
            let aim = at + Vec3::Y * l.aim_height;
            let to = aim - (xf.translation + Vec3::Y * HEAD_HEIGHT);
            let flat = Vec2::new(to.x, to.z);
            let want = (-to.x).atan2(-to.z);
            (look_yaw(want - body_yaw), look_tip(to.y, flat.length()))
        } else if planar.length() > TRAVEL_LOOK {
            (look_yaw((-planar.x).atan2(-planar.z) - body_yaw), 0.0)
        } else if let Ok(cam) = cam.single() {
            (look_yaw(cam.yaw - body_yaw), 0.0)
        } else {
            (0.0, 0.0)
        };
        let a = &mut lively.amounts;
        a.pitch = ease(a.pitch, pitch, LEAN_RATE, dt);
        a.roll = ease(a.roll, roll, LEAN_RATE, dt);
        a.yaw = ease(a.yaw, yaw, LOOK_RATE, dt);
        a.tip = ease(a.tip, tip, LOOK_RATE, dt);
        let now = *a;
        if crate::actor::anim::trace_on() {
            lively.traced += dt;
            if lively.traced >= 0.25 {
                lively.traced = 0.0;
                info!(
                    "lively: lean pitch {:+.1} roll {:+.1} deg (straighten {:+.1}), head yaw {:+.1} tip {:+.1} deg, free={free}",
                    now.pitch.to_degrees(),
                    now.roll.to_degrees(),
                    (straighten + sprint_straighten).to_degrees(),
                    now.yaw.to_degrees(),
                    now.tip.to_degrees()
                );
            }
        }
        let lean = lean_rotation(now.pitch * amount, now.roll * amount + straighten);
        let look = look_rotation(now.yaw * amount.min(1.0), now.tip * amount.min(1.0));
        if sprint_straighten != 0.0 {
            turn(
                &mut pose,
                &mut lively,
                SPINE,
                Quat::from_rotation_z(sprint_straighten),
                1.0,
            );
        }
        if lean != Quat::IDENTITY {
            for (bone, share) in LEAN_SHARES {
                turn(&mut pose, &mut lively, bone, lean, share);
            }
        }
        if look != Quat::IDENTITY {
            for (bone, share) in LOOK_SHARES {
                turn(&mut pose, &mut lively, bone, look, share);
            }
        }
        if !lively.written.is_empty() {
            pose.pose_dirty = true;
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        drive.in_set(PosePost).after(crate::motion_clip::ClipLayers),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceleration_leans_the_body_into_it_and_braking_leans_it_back() {
        let (start, _) = lean_for(40.0, 0.0);
        let (stop, _) = lean_for(-60.0, 0.0);
        assert!(start > 0.0 && stop < 0.0);
        assert!((start.to_degrees() - 6.0).abs() < 0.2, "{start}");
        // Capped: nothing the motor can do tips it past the ceilings.
        let (hard, _) = lean_for(1.0e4, 0.0);
        let (back, _) = lean_for(-1.0e4, 0.0);
        assert!((hard - PITCH_FORWARD_MAX).abs() < 1.0e-6);
        assert!((back + PITCH_BACK_MAX).abs() < 1.0e-6);
    }

    #[test]
    fn a_turn_banks_toward_the_inside_and_is_capped() {
        let (_, left) = lean_for(0.0, 30.0);
        let (_, right) = lean_for(0.0, -30.0);
        assert!(left > 0.0 && right < 0.0);
        assert!((left + right).abs() < 1.0e-6);
        assert!((lean_for(0.0, 1.0e4).1 - ROLL_MAX).abs() < 1.0e-6);
        assert_eq!(lean_for(0.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn the_head_turns_what_a_neck_can_and_gives_up_behind_the_shoulder() {
        assert!((look_yaw(0.5) - 0.5).abs() < 1.0e-6);
        assert!((look_yaw(-0.5) + 0.5).abs() < 1.0e-6);
        assert!((look_yaw(1.5) - LOOK_YAW_MAX).abs() < 1.0e-6);
        assert!((look_yaw(-1.5) + LOOK_YAW_MAX).abs() < 1.0e-6);
        // Behind the shoulder, and a camera spun a full circle, it looks ahead; no side flip.
        assert_eq!(look_yaw(3.0), 0.0);
        assert_eq!(look_yaw(-3.0), 0.0);
        assert!((look_yaw(0.3 + std::f32::consts::TAU) - 0.3).abs() < 1.0e-4);
    }

    #[test]
    fn a_target_above_tips_the_head_up_and_one_below_down() {
        assert!(look_tip(0.5, 4.0) > 0.0);
        assert!(look_tip(-0.5, 4.0) < 0.0);
        assert!((look_tip(100.0, 1.0) - LOOK_PITCH_MAX).abs() < 1.0e-6);
        // A target at the feet (no ground distance) does not divide by zero.
        assert!(look_tip(1.0, 0.0).is_finite());
    }

    #[test]
    fn easing_reaches_its_target_at_any_frame_rate() {
        let at = |frames: u32| {
            let mut v = 0.0;
            for _ in 0..frames {
                v = ease(v, 1.0, 8.0, 1.0 / frames as f32);
            }
            v
        };
        assert!((at(30) - at(144)).abs() < 1.0e-4);
        assert!(at(30) > 0.99);
        // A paused frame changes nothing.
        assert_eq!(ease(0.25, 1.0, 8.0, 0.0), 0.25);
    }

    #[test]
    fn a_lean_in_the_parents_frame_is_the_same_turn_of_the_world() {
        // A parent already tipped 25 degrees forward: the bone's local turn, taken through the
        // parent, is still a yaw about the world's up.
        let parent = Quat::from_rotation_x(-0.44);
        let world = look_rotation(0.5, 0.0);
        let local = in_parent(parent, world);
        let back = parent * local * parent.inverse();
        assert!((back.dot(world).abs() - 1.0).abs() < 1.0e-5);
        // Identity parent: unchanged.
        assert!((in_parent(Quat::IDENTITY, world).dot(world).abs() - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn additive_edits_use_the_current_parent_after_clip_layers_change_the_pose() {
        use benilla_assets::{ModelJoint, ModelSkeleton};
        let skeleton = ModelSkeleton {
            joints: [-1, 0, 1]
                .map(|parent| ModelJoint {
                    parent,
                    local_translation: Vec3::Y,
                    billboard: None,
                    parent_arm: None,
                })
                .to_vec(),
            ..Default::default()
        };
        let mut pose = RigPose::new(Entity::PLACEHOLDER, &skeleton);
        // PosePost edits locals before the engine composes model matrices. Model is still bind.
        pose.locals[0].rotation = Quat::from_rotation_y(0.6);
        pose.locals[1].rotation = Quat::from_rotation_x(-0.9);
        pose.pose_dirty = true;
        let parent = pose.locals[0].rotation * pose.locals[1].rotation;
        let delta = Quat::from_rotation_z(-0.3);
        turn(&mut pose, &mut Lively::default(), 2, delta, 1.0);
        let after = parent * pose.locals[2].rotation;
        let wanted = delta * parent;
        assert!(
            after.angle_between(wanted) < 1.0e-4,
            "post-pass used the stale model instead of current locals"
        );
    }

    #[test]
    fn sprint_post_pass_centers_the_torso_and_preserves_feet_and_forward_motion() {
        use benilla_assets::{ModelJoint, ModelSkeleton};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../anim/source/body_sprint.wsanim");
        let clip = crate::animation_library::read(&path).unwrap();
        let gait_clip = crate::locomotion::Clip::new(
            crate::locomotion::Role::Sprint,
            clip.clone(),
            clip.clone(),
        )
        .unwrap();
        let skeleton = ModelSkeleton {
            joints: clip
                .parents
                .iter()
                .zip(&clip.frames[0])
                .map(|(&parent, bone)| ModelJoint {
                    parent,
                    local_translation: Vec3::from_array(bone.t),
                    billboard: None,
                    parent_arm: None,
                })
                .collect(),
            ..Default::default()
        };
        let mut gait = crate::locomotion::GaitState::default();
        gait.kit = true;
        gait.weight = 1.0;
        gait.sprint_straighten = gait_clip.straighten;
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .init_resource::<LockOn>()
            .add_systems(Update, drive);
        let mut motor = Motor::default();
        motor.grounded = true;
        motor.velocity = Vec3::NEG_Z * 7.0;
        // Steady sprint, after startup acceleration has settled; acceleration lean is separate.
        let lively = Lively {
            velocity: motor.velocity,
            seen: true,
            ..Default::default()
        };
        let entity = app
            .world_mut()
            .spawn((
                Avatar,
                Action::default(),
                RigPose::new(Entity::PLACEHOLDER, &skeleton),
                motor,
                Transform::IDENTITY,
                gait,
                lively,
            ))
            .id();
        let positions = |pose: &RigPose| {
            let mut model = Vec::<Mat4>::new();
            for (bone, local) in pose.locals.iter().enumerate() {
                let local = local.to_matrix();
                model.push(
                    usize::try_from(pose.parents[bone])
                        .map_or(local, |parent| model[parent] * local),
                );
            }
            model
                .into_iter()
                .map(|m| m.transform_point3(Vec3::ZERO))
                .collect::<Vec<_>>()
        };
        let (mut before_trunk, mut after_trunk) = (Vec3::ZERO, Vec3::ZERO);
        for frame in &clip.frames {
            let mut pose = app.world_mut().get_mut::<RigPose>(entity).unwrap();
            for (local, bone) in pose.locals.iter_mut().zip(frame) {
                *local = Transform {
                    translation: Vec3::from_array(bone.t),
                    rotation: Quat::from_array(bone.r).normalize(),
                    scale: Vec3::from_array(bone.s),
                };
            }
            let before = positions(&pose);
            before_trunk += before[48] - before[20];
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(1.0 / 30.0));
            app.update();
            let after = positions(app.world().get::<RigPose>(entity).unwrap());
            after_trunk += after[48] - after[20];
            for foot in [68, 70, 80, 81] {
                assert!(
                    before[foot].distance(after[foot]) < 1.0e-5,
                    "foot {foot} moved"
                );
            }
        }
        // Measure mean offsets in yards, independent of how many frames the clip contains.
        before_trunk /= clip.frames.len() as f32;
        after_trunk /= clip.frames.len() as f32;
        assert!(
            after_trunk.x.abs() < 1.0e-5,
            "sprint torso remains tilted: {after_trunk:?}"
        );
        assert!(
            (before_trunk.z - after_trunk.z).abs() < 1.0e-5,
            "forward hunch changed: {before_trunk:?} -> {after_trunk:?}"
        );
    }

    #[test]
    fn lean_rotation_pitches_the_top_forward_and_banks_it_left() {
        // The rig's axes: forward -Z, up +Y, left -X.
        let up = Vec3::Y;
        let forward = lean_rotation(0.2, 0.0) * up;
        assert!(forward.z < 0.0 && forward.x.abs() < 1.0e-6, "{forward:?}");
        let left = lean_rotation(0.0, 0.2) * up;
        assert!(left.x < 0.0 && left.z.abs() < 1.0e-6, "{left:?}");
        // A look to the left carries the face (forward -Z) toward -X; up tips it upward.
        let face = look_rotation(0.3, 0.0) * Vec3::NEG_Z;
        assert!(face.x < 0.0);
        let tipped = look_rotation(0.0, 0.3) * Vec3::NEG_Z;
        assert!(tipped.y > 0.0);
    }
}
