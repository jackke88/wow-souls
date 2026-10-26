//! The remix's live half (`WOWSOULS_MOTION=1`): the enemies that move on WoW's own animations
//! (standing, walking, running, turning: the model's clips, not the DS kit's) get the same
//! secondary motion the baked clips have. Neck, head, jaw, tail and ears are springs in world
//! space, so a wolf's tail swings out when it turns and trails when it runs, a kobold's head lags
//! its shoulders, and a bite's jaw is not a rigid hinge.
//!
//! A creature that plays a kit clip is left to the clip (which was dressed when it loaded); this
//! only runs while the rig is its own. The edits are put back before they are made again
//! (as [`crate::lively`] does) so they never pile up on a bone the evaluator did not rewrite.
//! Rotations only: no bone moves, no length changes.

use benilla_world::rig_anim::{PosePost, RigPose};
use bevy::prelude::*;

use super::remix::{rig_for_bones, rig_of_kit, Chase, Rig, Spring, LIVE};
use super::{MonsterFade, MonsterPlay};
use crate::friendly::Npc;
use crate::species::Species;

/// A creature's springs: one per bone of its rig that trails, in the rig's order.
#[derive(Component, Default)]
pub(super) struct Live {
    chases: Vec<Chase>,
    /// `(bone, rotation before, rotation after)` of last frame's edits, to put back.
    written: Vec<(usize, Quat, Quat)>,
    /// The springs sit on their targets (they have seen a frame); false after a kit clip.
    primed: bool,
    /// The rig's parents were checked: `Some(true)` it is the rig, `Some(false)` it is not.
    fits: Option<bool>,
}

/// The bones that trail, parents before children, with their springs.
fn followers(rig: &Rig) -> Vec<(usize, Spring)> {
    let mut out: Vec<(usize, Spring)> = rig.neck.map(|n| (n, LIVE.neck)).into_iter().collect();
    out.push((rig.head, LIVE.head));
    out.extend(rig.jaw.map(|j| (j, LIVE.jaw)));
    for &(bone, speed) in rig.tail {
        let mut spring = LIVE.tail;
        spring.omega *= speed;
        out.push((bone, spring));
    }
    out.extend(rig.ears.iter().map(|&e| (e, LIVE.ear)));
    out
}

/// The model-space rotation of everything above `bone`.
fn parent_rot(locals: &[Transform], parents: &[i16], bone: usize) -> Quat {
    let mut q = Quat::IDENTITY;
    let mut cur = parents[bone];
    while let Ok(i) = usize::try_from(cur) {
        q = locals[i].rotation.normalize() * q;
        cur = parents[i];
    }
    q
}

/// Put back what last frame's edits changed, where the pose has not been rewritten since.
fn undo(locals: &mut [Transform], live: &mut Live) {
    for (bone, before, after) in live.written.drain(..) {
        if let Some(local) = locals.get_mut(bone) {
            if local.rotation == after {
                local.rotation = before;
            }
        }
    }
}

/// One frame: the springs chase what the pose says (`body` is the creature's own turn, so the
/// head and tail trail it too) and the bones show their share of the way there. Returns whether
/// the pose changed.
fn dress_pose(
    live: &mut Live,
    locals: &mut [Transform],
    parents: &[i16],
    body: Quat,
    rig: &Rig,
    dt: f32,
) -> bool {
    undo(locals, live);
    let followers = followers(rig);
    if live.chases.len() != followers.len() {
        live.chases = vec![Chase::at(Quat::IDENTITY); followers.len()];
        live.primed = false;
    }
    let mut changed = false;
    for (k, &(bone, spring)) in followers.iter().enumerate() {
        let above = body * parent_rot(locals, parents, bone);
        let before = locals[bone].rotation;
        let target = above * before.normalize();
        if !live.primed {
            live.chases[k] = Chase::at(target);
            continue;
        }
        let q = live.chases[k].step(target, spring, dt);
        let out = target.slerp(q, spring.mix);
        let after = (above.inverse() * out).normalize();
        locals[bone].rotation = after;
        live.written.push((bone, before, after));
        changed = true;
    }
    live.primed = true;
    changed
}

/// Enemies, and the friendly NPCs (the valley's trainers, guards, peasants), on their own rigs.
#[allow(clippy::type_complexity)]
pub(super) fn secondary(
    time: Res<Time>,
    mut commands: Commands,
    mut creatures: Query<
        (
            Entity,
            Option<&Species>,
            Option<(&MonsterPlay, &MonsterFade)>,
            &GlobalTransform,
            &mut RigPose,
            Option<&mut Live>,
        ),
        Or<(With<Species>, With<Npc>)>,
    >,
) {
    // A paused or stopped clock leaves the pose as the last frame made it.
    let dt = time.delta_secs().min(0.05);
    if dt <= 0.0 {
        return;
    }
    for (entity, species, kit, at, mut pose, live) in &mut creatures {
        let Some(mut live) = live else {
            commands.entity(entity).insert(Live::default());
            continue;
        };
        // A foe's rig is its kit's; a friendly NPC's is found by its bone count.
        let rig = match species {
            Some(species) => rig_of_kit(species.kit()),
            None => rig_for_bones(pose.locals.len()),
        };
        let Some(rig) = rig else {
            continue;
        };
        let RigPose {
            locals, parents, ..
        } = &mut *pose;
        let fits = *live.fits.get_or_insert_with(|| rig.fits(parents));
        if !fits {
            continue;
        }
        if kit.is_some_and(|(play, fade)| play.clip.is_some() || !fade.0.is_free()) {
            // The kit owns the bones: its own writes cover ours; the springs start again at rest.
            undo(locals, &mut live);
            live.primed = false;
            continue;
        }
        let body = at.to_scale_rotation_translation().1;
        if dress_pose(&mut live, locals, parents, body, rig, dt) {
            pose.pose_dirty = true;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, secondary.in_set(PosePost).after(super::pose));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wolf-like skeleton of 64 bones: a chain 0 -> 21 -> 25 (neck) -> 29 (head) -> 47 (jaw),
    /// and the five tail bones hanging from the root.
    fn parents() -> Vec<i16> {
        let mut p = vec![0i16; 64];
        p[0] = -1;
        p[25] = 0;
        p[29] = 25;
        p[47] = 29;
        p[45] = 29;
        p[46] = 29;
        p
    }

    fn wolf() -> &'static Rig {
        rig_of_kit("wolf").unwrap()
    }

    #[test]
    fn the_wolf_rig_fits_a_wolf_shaped_skeleton_and_nothing_else() {
        assert!(wolf().fits(&parents()));
        let mut odd = parents();
        odd[29] = 0;
        assert!(!wolf().fits(&odd));
        assert!(!wolf().fits(&parents()[..40]));
    }

    #[test]
    fn a_creature_at_rest_is_not_touched_and_a_turn_makes_the_tail_trail() {
        let (parents, rig) = (parents(), wolf());
        let mut locals = vec![Transform::IDENTITY; 64];
        let mut live = Live::default();
        let dt = 1.0 / 60.0;
        // The first frame primes the springs; nothing is written.
        assert!(!dress_pose(
            &mut live,
            &mut locals,
            &parents,
            Quat::IDENTITY,
            rig,
            dt
        ));
        // At rest, the springs sit on their targets: the pose is not changed (to rounding).
        for _ in 0..30 {
            dress_pose(&mut live, &mut locals, &parents, Quat::IDENTITY, rig, dt);
        }
        for local in &locals {
            assert!(local.rotation.angle_between(Quat::IDENTITY) < 1e-4);
        }
        // The body turns 40 degrees in a few frames: the tail bones lag it (their local rotation
        // turns back against the body), then settle.
        let mut worst = 0.0_f32;
        for i in 1..=12 {
            let body = Quat::from_rotation_y((40.0_f32 * i as f32 / 12.0).to_radians());
            undo(&mut locals, &mut live);
            dress_pose(&mut live, &mut locals, &parents, body, rig, dt);
            worst = worst.max(
                locals[38]
                    .rotation
                    .angle_between(Quat::IDENTITY)
                    .to_degrees(),
            );
        }
        assert!(worst > 2.0, "the tail trails the turn: {worst}");
        assert!(worst <= LIVE.tail.max + 0.5, "within its cap: {worst}");
        for _ in 0..120 {
            dress_pose(
                &mut live,
                &mut locals,
                &parents,
                Quat::from_rotation_y(40.0_f32.to_radians()),
                rig,
                dt,
            );
        }
        assert!(
            locals[38]
                .rotation
                .angle_between(Quat::IDENTITY)
                .to_degrees()
                < 0.2,
            "and settles"
        );
    }

    #[test]
    fn the_edits_are_put_back_before_they_are_made_again() {
        let (parents, rig) = (parents(), wolf());
        let mut locals = vec![Transform::IDENTITY; 64];
        let mut live = Live::default();
        for i in 0..20 {
            let body = Quat::from_rotation_y((i as f32 * 3.0).to_radians());
            dress_pose(&mut live, &mut locals, &parents, body, rig, 1.0 / 60.0);
        }
        // Edits piled up would grow past the cap; a bone the evaluator does not rewrite comes back.
        undo(&mut locals, &mut live);
        for local in &locals {
            assert!(local.rotation.angle_between(Quat::IDENTITY) < 1e-5);
        }
    }
}
