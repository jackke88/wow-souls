//! Target acquisition: a forward sight cone and omnidirectional, action-dependent hearing.
//! These gameplay rules do not read audio output, volume settings or the requested movement.

use bevy::math::Vec3Swizzles;

use crate::abilities::moves::{ANIM_CLAP, ANIM_ROAR};
use crate::combat::ActionState;
use crate::motor::{Motor, WALK_SPEED};

/// Sight reaches 20 yd inside a 140 degree cone (70 degrees either side of the nose).
pub const SIGHT_RANGE: f32 = 20.0;
const SIGHT_COS: f32 = 0.342_020_15;
/// No detection across floors or the meadow above a mine.
pub const MAX_RISE: f32 = 5.0;
/// A body right against the creature cannot hide by standing still behind it.
const CONTACT_RANGE: f32 = 1.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detection {
    Sight,
    Hearing,
    Contact,
}

/// Returns the channel that would detect the player, with clear world geometry supplied by
/// the caller. Also called with `clear = true` to skip raycasts for impossible candidates.
pub fn detect(dist: f32, facing: f32, rise: f32, clear: bool, noise: f32) -> Option<Detection> {
    if !clear || rise > MAX_RISE {
        None
    } else if dist <= SIGHT_RANGE && facing >= SIGHT_COS {
        Some(Detection::Sight)
    } else if noise > 0.0 && dist <= noise {
        Some(Detection::Hearing)
    } else if dist <= CONTACT_RANGE {
        Some(Detection::Contact)
    } else {
        None
    }
}

/// Audible radius in yd, sampled after the player's motor. The strongest current source wins.
/// Airborne travel and pushing into a wall produce no footsteps; landing and actions still do.
pub fn noise(motor: Option<&Motor>, action: &ActionState, warrior_clip: Option<u16>) -> f32 {
    if matches!(action, ActionState::Dead) {
        return 0.0;
    }
    let movement: f32 = motor.map_or(0.0, |m| {
        if m.last.landed {
            8.0
        } else if !m.grounded {
            0.0
        } else {
            let speed = m.velocity.xz().length();
            if speed <= 0.15 {
                0.0
            } else if speed <= WALK_SPEED + 0.25 {
                2.5
            } else if speed <= 5.0 {
                6.0
            } else {
                10.0
            }
        }
    });
    let action: f32 = match action {
        ActionState::Attack(_) | ActionState::Evade(_) => 8.0,
        ActionState::Cast(_) => 12.0,
        _ => 0.0,
    };
    let warrior: f32 = match warrior_clip {
        Some(ANIM_ROAR) => 16.0,
        Some(ANIM_CLAP) => 14.0,
        _ => 0.0,
    };
    movement.max(action).max(warrior)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{Cast, CastSpec, Evade, EvadeKind, MoveKind, Swing, SwingSpec};
    use bevy::prelude::*;

    fn moving(speed: f32) -> Motor {
        let mut motor = Motor::default();
        motor.grounded = true;
        motor.velocity = Vec3::X * speed;
        motor
    }

    #[test]
    fn sight_has_a_forward_cone_and_an_inclusive_range() {
        assert_eq!(detect(20.0, 1.0, 0.0, true, 0.0), Some(Detection::Sight));
        assert_eq!(detect(20.01, 1.0, 0.0, true, 0.0), None);
        assert_eq!(
            detect(10.0, SIGHT_COS, 0.0, true, 0.0),
            Some(Detection::Sight)
        );
        assert_eq!(detect(10.0, SIGHT_COS - 0.001, 0.0, true, 0.0), None);
        assert_eq!(detect(3.0, -1.0, 0.0, true, 0.0), None);
    }

    #[test]
    fn hearing_is_omnidirectional_but_standing_is_silent() {
        for facing in [-1.0, 0.0, 1.0] {
            assert!(detect(6.0, facing, 0.0, true, 6.0).is_some());
        }
        assert_eq!(detect(6.01, -1.0, 0.0, true, 6.0), None);
        assert_eq!(detect(2.0, -1.0, 0.0, true, 0.0), None);
        assert_eq!(
            detect(CONTACT_RANGE, -1.0, 0.0, true, 0.0),
            Some(Detection::Contact)
        );
    }

    #[test]
    fn walls_and_height_separate_sight_hearing_and_contact() {
        for (dist, facing, noise) in [(12.0, 1.0, 0.0), (4.0, -1.0, 16.0), (1.0, -1.0, 0.0)] {
            assert_eq!(detect(dist, facing, 0.0, false, noise), None);
            assert_eq!(detect(dist, facing, MAX_RISE + 0.01, true, noise), None);
            assert!(detect(dist, facing, MAX_RISE, true, noise).is_some());
        }
    }

    #[test]
    fn actual_grounded_speed_controls_footstep_noise() {
        for (speed, radius) in [
            (0.0, 0.0),
            (0.1, 0.0),
            (2.5, 2.5),
            (4.2, 6.0),
            (4.5, 6.0),
            (7.0, 10.0),
        ] {
            assert_eq!(
                noise(Some(&moving(speed)), &ActionState::Idle, None),
                radius
            );
        }
        let mut motor = moving(0.0);
        motor.horiz_vel = Vec3::X * 7.0;
        assert_eq!(
            noise(Some(&motor), &ActionState::Idle, None),
            0.0,
            "wall push is silent"
        );
        motor.velocity = Vec3::new(0.0, -10.0, 0.0);
        assert_eq!(
            noise(Some(&motor), &ActionState::Idle, None),
            0.0,
            "vertical motion is not footsteps"
        );
        motor.velocity = Vec3::X * 7.0;
        motor.grounded = false;
        assert_eq!(noise(Some(&motor), &ActionState::Idle, None), 0.0);
        motor.last.landed = true;
        assert_eq!(noise(Some(&motor), &ActionState::Idle, None), 8.0);
    }

    #[test]
    fn actions_are_audible_without_movement_or_an_audio_device() {
        let attack = ActionState::Attack(Swing {
            t: 0.2,
            kind: MoveKind::R1(0),
            spec: SwingSpec::procedural(1.0),
            reference: false,
            serial: 1,
        });
        let roll = ActionState::Evade(Evade {
            t: 0.2,
            kind: EvadeKind::Roll,
            dir: Vec3::X,
            spec: *EvadeKind::Roll.spec(),
            reference: false,
        });
        let cast = ActionState::Cast(Cast {
            t: 0.2,
            spell: 133,
            spec: CastSpec {
                duration: 2.5,
                release: 2.0,
                recover: 2.3,
            },
            serial: 1,
            target: None,
        });
        assert_eq!(noise(None, &attack, None), 8.0);
        assert_eq!(noise(None, &roll, None), 8.0);
        assert_eq!(noise(None, &cast, None), 12.0);
        assert_eq!(noise(None, &ActionState::Idle, Some(ANIM_ROAR)), 16.0);
        assert_eq!(noise(Some(&moving(7.0)), &attack, Some(ANIM_CLAP)), 14.0);
        assert_eq!(
            noise(Some(&moving(7.0)), &ActionState::Dead, Some(ANIM_ROAR)),
            0.0
        );
    }
}
