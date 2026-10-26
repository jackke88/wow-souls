//! Lays a wide, flat creature along the ground it stands on (2026-10-18, the user: the hairy spiders
//! "går igenom världen"). A creature's body stands level on its feet point, which is right for a man
//! and wrong for a tarantula two yards long: on a hillside its front legs went into the slope and its
//! back legs hung in the air. Here the ground is sampled ahead, behind and to both sides of the body,
//! and the model (its [`crate::actor::ModelFrame`], never the body's own `Transform`, which the motor,
//! the AI and the hit tests read) is turned to that plane about the feet, eased so a step over a root
//! does not snap it.

use benilla_world::collision::WorldCollision;
use bevy::prelude::*;

use crate::actor::ModelFrame;
use crate::settle::{snap_to_ground, Phase};

/// The steepest tilt shown (radians): past it the hill is a wall and the body stays upright on it.
const MAX_TILT: f32 = 0.55;
/// How fast the tilt follows the ground (1/s).
const EASE: f32 = 9.0;
/// The share of the model's half-length the ground is sampled at (the leg tips, not the leg ends).
const REACH: f32 = 0.75;
/// Beyond this distance from the player (yd) nobody looks closely enough to pay for the rays.
const NEAR: f32 = 70.0;
/// A body this far over the ground under it (a leap, a latch) leans back to level instead.
const AIRBORNE: f32 = 0.45;

/// The kits laid along the ground: the many-legged ones.
pub fn wants_slope(kit: &str) -> bool {
    crate::legfloor::wants_floor(kit)
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        lay.before(TransformSystems::Propagate)
            .run_if(in_state(Phase::Playing)),
    );
}

/// The tilt that turns local up onto the plane through four ground heights: `front`/`back` at
/// `length` ahead/behind (local -Z/+Z) and `left`/`right` at `width` to the sides, clamped to
/// [`MAX_TILT`].
pub fn tilt(front: f32, back: f32, left: f32, right: f32, length: f32, width: f32) -> Quat {
    if length <= 0.0 || width <= 0.0 {
        return Quat::IDENTITY;
    }
    let dz = (back - front) / (2.0 * length);
    let dx = (right - left) / (2.0 * width);
    let normal = Vec3::new(-dx, 1.0, -dz).normalize();
    let turn = Quat::from_rotation_arc(Vec3::Y, normal);
    let (axis, angle) = turn.to_axis_angle();
    if angle > MAX_TILT {
        Quat::from_axis_angle(axis, MAX_TILT)
    } else {
        turn
    }
}

#[allow(clippy::type_complexity)]
fn lay(
    time: Res<Time>,
    world: WorldCollision,
    player: Query<&Transform, (With<crate::avatar::Avatar>, Without<ModelFrame>)>,
    bodies: Query<(
        &Transform,
        &crate::species::Species,
        &ModelFrame,
        Option<&benilla_world::world_unit::WorldUnit>,
    )>,
    mut frames: Query<
        &mut Transform,
        (
            Without<crate::species::Species>,
            Without<crate::avatar::Avatar>,
        ),
    >,
) {
    let me = player.single().map(|t| t.translation).ok();
    let ease = (time.delta_secs() * EASE).min(1.0);
    for (xf, species, frame, unit) in &bodies {
        if !wants_slope(species.kit()) {
            continue;
        }
        let Ok(mut model) = frames.get_mut(frame.0) else {
            continue;
        };
        let near = me.is_none_or(|p| p.distance_squared(xf.translation) < NEAR * NEAR);
        let want = if near {
            ground_tilt(&world, xf, unit)
        } else {
            Quat::IDENTITY
        };
        let next = model.rotation.slerp(want, ease);
        if model.rotation.angle_between(next) > 1.0e-4 || model.translation != Vec3::ZERO {
            model.rotation = next;
            model.translation = Vec3::ZERO;
        }
    }
}

/// The plane under the body, as a turn of its model; level when the body is off the ground.
fn ground_tilt(
    world: &WorldCollision,
    xf: &Transform,
    unit: Option<&benilla_world::world_unit::WorldUnit>,
) -> Quat {
    // The footprint: the model's idle box (unscaled) times the body's scale; a yard when unknown.
    let half = unit
        .and_then(|u| u.bound)
        .map(|b| Vec3::from(b.half_extents))
        .filter(|h| h.max_element() > 0.0)
        .map_or(1.0, |h| h.x.max(h.z) * xf.scale.x);
    let reach = (half * REACH).clamp(0.3, 3.0);
    let feet = xf.translation;
    let Some(under) = snap_to_ground(world, feet) else {
        return Quat::IDENTITY;
    };
    if feet.y - under > AIRBORNE {
        return Quat::IDENTITY;
    }
    let at = |local: Vec3| {
        let p = feet + xf.rotation * local;
        snap_to_ground(world, p).unwrap_or(under)
    };
    let front = at(Vec3::NEG_Z * reach);
    let back = at(Vec3::Z * reach);
    let left = at(Vec3::NEG_X * reach);
    let right = at(Vec3::X * reach);
    tilt(front, back, left, right, reach, reach)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_ground_is_no_tilt() {
        assert!(tilt(0.0, 0.0, 0.0, 0.0, 1.0, 1.0).angle_between(Quat::IDENTITY) < 1.0e-5);
    }

    #[test]
    fn an_uphill_ahead_raises_the_nose() {
        // Ground one yard higher a yard ahead than a yard behind: 26.6 degrees nose up.
        let q = tilt(0.5, -0.5, 0.0, 0.0, 1.0, 1.0);
        let nose = q * Vec3::NEG_Z;
        assert!(nose.y > 0.4, "{nose:?}");
        assert!((q.angle_between(Quat::IDENTITY) - 0.5_f32.atan()).abs() < 1.0e-3);
        // Higher on the right: the right side rises.
        let q = tilt(0.0, 0.0, -0.3, 0.3, 1.0, 1.0);
        assert!((q * Vec3::X).y > 0.2);
    }

    #[test]
    fn a_cliff_is_clamped() {
        let q = tilt(5.0, -5.0, 0.0, 0.0, 1.0, 1.0);
        assert!((q.angle_between(Quat::IDENTITY) - MAX_TILT).abs() < 1.0e-4);
    }
}
