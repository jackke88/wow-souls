//! Inertia for the player's own walking. The motor takes the stick at face value (0 to 7 yd/s in
//! one frame, and back), which a body of flesh and a shield does not do and which makes every Dark
//! Souls clip on top of it look stiff: a clip is authored with a start and a stop. The controller
//! feeds the motor a velocity that follows the stick at a bounded acceleration instead.
//!
//! Pure: the controller owns the state and the stick; actions that carry the body themselves (a
//! swing's root motion, a roll, a knock-back, a charge) overwrite the intent as ever and are not
//! softened.

use bevy::prelude::*;

/// How fast the walking velocity builds toward a faster or turned stick (yd/s²): a run (7 yd/s)
/// takes 0.175 s to reach, a full reversal of one 0.35 s.
pub const ACCEL: f32 = 40.0;
/// How fast it falls toward a slower or released stick (yd/s²): a stop from a run takes 0.12 s,
/// quicker than the start so that the body does not slide past where the player let go.
pub const BRAKE: f32 = 60.0;

/// `WOWSOULS_GLIDE=0` gives the motor the raw stick again.
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("WOWSOULS_GLIDE").map_or(true, |v| v != "0"))
}

/// The walking velocity after `dt` seconds of following `target` from `vel` (both planar,
/// yd/s). Speeding up, or turning at the same speed, is limited by [`ACCEL`]; slowing down by
/// [`BRAKE`]. Never overshoots the target.
pub fn follow(vel: Vec3, target: Vec3, dt: f32) -> Vec3 {
    if !(dt > 0.0 && dt.is_finite()) {
        return vel;
    }
    let gap = target - vel;
    let left = gap.length();
    if left <= 1.0e-6 {
        return target;
    }
    let rate = if target.length() + 1.0e-4 >= vel.length() {
        ACCEL
    } else {
        BRAKE
    };
    let step = rate * dt;
    if left <= step {
        target
    } else {
        vel + gap / left * step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN: Vec3 = Vec3::new(0.0, 0.0, -7.0);

    fn run_for(mut vel: Vec3, target: Vec3, secs: f32) -> Vec3 {
        let dt = 1.0 / 60.0;
        for _ in 0..(secs / dt).round() as u32 {
            vel = follow(vel, target, dt);
        }
        vel
    }

    #[test]
    fn a_start_builds_over_a_fraction_of_a_second_not_a_frame() {
        let one_frame = follow(Vec3::ZERO, RUN, 1.0 / 60.0);
        assert!(one_frame.length() > 0.0 && one_frame.length() < 1.0);
        assert!((one_frame.length() - ACCEL / 60.0).abs() < 1.0e-4);
        // Not there at 0.1 s, there by 0.2 s.
        assert!(run_for(Vec3::ZERO, RUN, 0.1).length() < 7.0 - 1.0);
        assert!((run_for(Vec3::ZERO, RUN, 0.2) - RUN).length() < 1.0e-4);
    }

    #[test]
    fn a_stop_is_quicker_than_a_start() {
        let up = (0..600)
            .scan(Vec3::ZERO, |v, i| {
                *v = follow(*v, RUN, 1.0 / 600.0);
                Some((i, *v))
            })
            .find(|(_, v)| (*v - RUN).length() < 1.0e-3)
            .map(|(i, _)| i)
            .expect("reaches the run");
        let down = (0..600)
            .scan(RUN, |v, i| {
                *v = follow(*v, Vec3::ZERO, 1.0 / 600.0);
                Some((i, *v))
            })
            .find(|(_, v)| v.length() < 1.0e-3)
            .map(|(i, _)| i)
            .expect("reaches standstill");
        assert!(down < up, "stop {down} vs start {up}");
    }

    #[test]
    fn a_reversal_passes_through_a_standstill_without_a_jump() {
        let mut v = RUN;
        let mut slowest = f32::MAX;
        let mut worst_step = 0.0f32;
        for _ in 0..60 {
            let next = follow(v, -RUN, 1.0 / 60.0);
            worst_step = worst_step.max((next - v).length());
            slowest = slowest.min(next.length());
            v = next;
        }
        assert!(slowest < 0.5, "slowest {slowest}");
        // Each frame moves the velocity by at most the brake or the acceleration allows.
        assert!(
            worst_step <= BRAKE / 60.0 + 1.0e-4,
            "worst step {worst_step}"
        );
        assert!((v + RUN).length() < 1.0e-3);
    }

    #[test]
    fn it_never_overshoots_and_holds_a_reached_target() {
        let v = follow(Vec3::new(0.0, 0.0, -6.99), RUN, 1.0);
        assert_eq!(v, RUN);
        assert_eq!(follow(RUN, RUN, 1.0 / 60.0), RUN);
        // A pause (no time) changes nothing; a bad delta is no delta.
        assert_eq!(follow(Vec3::ZERO, RUN, 0.0), Vec3::ZERO);
        assert_eq!(follow(Vec3::ZERO, RUN, f32::NAN), Vec3::ZERO);
    }

    #[test]
    fn a_turn_at_speed_is_a_curve_not_a_snap() {
        // Running north, the stick goes east: the velocity swings round through the corner.
        let east = Vec3::new(7.0, 0.0, 0.0);
        let after = follow(RUN, east, 1.0 / 60.0);
        assert!(after.z < 0.0 && after.x > 0.0, "{after:?}");
        assert!((after - RUN).length() <= ACCEL / 60.0 + 1.0e-4);
        // Mid-turn the speed dips a little, as a body that changes course does.
        let mid = run_for(RUN, east, 0.2);
        assert!(mid.length() < 7.0 && mid.length() > 3.0, "{mid:?}");
    }
}
