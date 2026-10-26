//! Swimming (2026-10-18, the user: "Vi har ingen simfunktion! ... Precis som i wow"). Deep enough, the
//! body leaves the floor and swims: no gravity, the stroke along the camera's pitch, the climb capped
//! with the head out of the water.
//!
//! The numbers and the shape are the 1.12.1 reference's as benilla reads them (`benilla-app`
//! `player/swim.rs` @ 2e82d34, MIT OR Apache-2.0): the latch on the depth over the feet against 0.75 of
//! the body's height with a 1/36-yd band (`0x6030c0`), the swim speeds (`MOVE_SWIM` 4.72, back 2.5),
//! the floating resolver's slide with the rise capped at the rest line (`0x632ba0`) and a rising
//! stroke turned level along it at full speed. Ours, not the reference's: Jump held swims up (1.12's
//! Space), and the dive follows the camera's pitch past a dead zone, since the Souls camera always
//! looks a little down.

use avian3d::character_controller::move_and_slide::{MoveAndSlideConfig, MoveAndSlideHitResponse};
use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Motor, Outcome, CAPSULE_HEIGHT};

/// Swim speed (yd/s), the stock `MOVE_SWIM`.
pub const SWIM_SPEED: f32 = 4.722_222;
/// Backward swim speed (yd/s), the stock `MOVE_SWIM_BACK`.
pub const SWIM_BACK_SPEED: f32 = 2.5;
/// The share of the body's height the water must cover to swim.
const SWIM_DEPTH_FRAC: f32 = 0.75;
/// The enter/leave band (yd): enter above `0.75·h`, leave below `0.75·h - 1/36`.
const SWIM_HYSTERESIS: f32 = 1.0 / 36.0;
/// The camera pitch (radians) inside which the stroke stays level.
const DIVE_DEAD_ZONE: f32 = 0.3;
/// How steeply the stroke follows the camera past the dead zone.
const DIVE_GAIN: f32 = 1.6;
/// The steepest dive or climb (radians).
const DIVE_MAX: f32 = 1.2;

/// Depth over the feet (yd) past which a body swims.
pub fn enter_depth() -> f32 {
    SWIM_DEPTH_FRAC * CAPSULE_HEIGHT
}

/// The swim latch: enter on `depth > 0.75·h`, leave on `depth < 0.75·h - 1/36` or no water.
pub fn latch(was: bool, surface: Option<f32>, feet_y: f32) -> bool {
    let Some(surface) = surface else {
        return false;
    };
    let depth = surface - feet_y;
    if was {
        depth >= enter_depth() - SWIM_HYSTERESIS
    } else {
        depth > enter_depth()
    }
}

/// The stroke's pitch for a camera at `cam_pitch` (negative looks down): level inside the dead zone.
pub fn dive_pitch(cam_pitch: f32) -> f32 {
    let past = cam_pitch.abs() - DIVE_DEAD_ZONE;
    if past <= 0.0 {
        0.0
    } else {
        (cam_pitch.signum() * past * DIVE_GAIN).clamp(-DIVE_MAX, DIVE_MAX)
    }
}

/// The swim velocity: `push` is the camera-relative level push (length `0..=1`), `forward` the
/// camera's level forward, `pitch` the stroke's ([`dive_pitch`]), `up` Jump held.
pub fn stroke(push: Vec3, forward: Vec3, pitch: f32, up: bool) -> Vec3 {
    let push = Vec3::new(push.x, 0.0, push.z).clamp_length_max(1.0);
    let forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let fwd = push.dot(forward);
    let side = push - forward * fwd;
    let speed = if fwd < 0.0 {
        SWIM_BACK_SPEED
    } else {
        SWIM_SPEED
    };
    // Only a forward stroke dives or climbs; backing and sidling stay level, as in the reference.
    let (sp, cp) = pitch.sin_cos();
    let along = if fwd > 0.0 {
        forward * cp + Vec3::Y * sp
    } else {
        forward
    };
    let mut v = (along * fwd + side) * speed;
    if up {
        v.y = v.y.max(SWIM_SPEED);
    }
    v
}

/// Clamps a rising stroke's upward speed to `cap` and turns the rest level at the same speed.
fn cap_redirect(v: Vec3, cap: f32) -> Vec3 {
    if v.y <= 0.0 || v.y <= cap {
        return v;
    }
    let speed = v.length();
    let level = Vec3::new(v.x, 0.0, v.z).normalize_or_zero();
    let level_speed = (speed * speed - cap * cap).max(0.0).sqrt();
    level * level_speed + Vec3::Y * cap
}

/// One swimming frame: no gravity, a slide against the lakebed and the banks, the rise capped so the
/// feet stop at `surface - 0.75·h` (head and shoulders out).
pub fn step(
    m: &mut Motor,
    feet: &mut Vec3,
    dt: std::time::Duration,
    world: &benilla_world::collision::WorldCollision<'_, '_>,
    capsule: &Collider,
    velocity: Vec3,
    surface: f32,
) -> Outcome {
    let dt = dt.min(std::time::Duration::from_secs_f32(super::MAX_STEP));
    let secs = dt.as_secs_f32();
    let half_h = Vec3::Y * (CAPSULE_HEIGHT * 0.5);
    let rest_feet = surface - enter_depth();
    let cap = if secs > 0.0 {
        ((rest_feet - feet.y) / secs).max(0.0)
    } else {
        0.0
    };
    let vel = cap_redirect(velocity, cap);
    let out = world.slide_body(
        capsule,
        *feet + half_h,
        vel,
        dt,
        &MoveAndSlideConfig::default(),
        |_hit| MoveAndSlideHitResponse::Accept,
    );
    let c = out.position;
    let before = *feet;
    *feet = c - half_h;
    m.vel_y = 0.0;
    m.horiz_vel = Vec3::new(vel.x, 0.0, vel.z);
    m.velocity = if secs > 0.0 {
        (*feet - before) / secs
    } else {
        Vec3::ZERO
    };
    // Afloat is never "grounded": the Souls gait and stance (which need ground) give way to the swim
    // clips, and the jump is no land jump.
    m.grounded = false;
    m.ground = None;
    m.airborne_prev = false;
    m.arc_dirs_set = false;
    m.wedged = false;
    m.wedge_still = 0;
    m.steep_support = false;
    let outcome = Outcome {
        grounded: false,
        jumped: false,
        landed: false,
    };
    m.last = outcome;
    outcome
}

/// The swim state on the avatar: latched by [`latch`], its waterline and the stroke's pitch (for the
/// model's lean).
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct Swimming {
    pub on: bool,
    pub surface: Option<f32>,
    pub pitch: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latch_has_the_reference_band() {
        let enter = enter_depth();
        assert!(!latch(false, Some(enter - 0.01), 0.0));
        assert!(latch(false, Some(enter + 0.01), 0.0));
        // Held inside the band, let go below it.
        assert!(latch(true, Some(enter - 0.01), 0.0));
        assert!(!latch(true, Some(enter - 0.05), 0.0));
        assert!(!latch(true, None, 0.0), "no water, no swim");
    }

    #[test]
    fn the_camera_dives_only_past_its_dead_zone() {
        assert_eq!(
            dive_pitch(-0.25),
            0.0,
            "the Souls camera's resting look stays level"
        );
        assert!(dive_pitch(-0.9) < -0.5);
        assert!(dive_pitch(0.5) > 0.0);
        assert!(dive_pitch(-3.0) >= -DIVE_MAX);
    }

    #[test]
    fn a_forward_stroke_follows_the_pitch_and_backing_stays_level() {
        let f = Vec3::NEG_Z;
        let v = stroke(f, f, -0.6, false);
        assert!((v.length() - SWIM_SPEED).abs() < 1.0e-4);
        assert!(v.y < -2.0, "{v:?}");
        let back = stroke(-f, f, -0.6, false);
        assert!((back.length() - SWIM_BACK_SPEED).abs() < 1.0e-4);
        assert_eq!(back.y, 0.0);
        assert!(
            stroke(Vec3::ZERO, f, 0.0, true).y >= SWIM_SPEED,
            "Jump swims up"
        );
    }

    #[test]
    fn a_climb_into_the_cap_turns_level_at_full_speed() {
        let v = cap_redirect(Vec3::new(3.0, 3.6, 0.0), 0.0);
        assert_eq!(v.y, 0.0);
        assert!((v.length() - Vec3::new(3.0, 3.6, 0.0).length()).abs() < 1.0e-4);
        assert_eq!(cap_redirect(Vec3::new(0.0, -1.0, 0.0), 0.0).y, -1.0);
    }
}
