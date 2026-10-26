//! Locomotion animation: which sequence a body plays from what its motor achieved. A small state
//! machine over the 1.12 animation ids (AnimationData.dbc): Stand, Walk, Run, the three-part
//! jump and Fall. Walk and Run play at `speed / clip.move_speed`, the reference's gait rate
//! (benilla `creature_anim::select::playback_rate`), so the feet match the ground.
//!
//! The speed is the motor's *achieved* velocity averaged as a vector, so a body jittering in a
//! wall corner (back and forth, net zero) reads as standing, not walking.

use std::time::Duration;

use bevy::animation::transition::AnimationTransitions;
use bevy::animation::RepeatAnimation;
use bevy::prelude::*;

use benilla_assets::ModelAnimations;

use crate::motor::Motor;

/// AnimationData.dbc ids.
pub mod ids {
    pub const STAND: u16 = 0;
    pub const WALK: u16 = 4;
    pub const RUN: u16 = 5;
    pub const SHUFFLE_LEFT: u16 = 11;
    pub const SHUFFLE_RIGHT: u16 = 12;
    pub const WALK_BACKWARDS: u16 = 13;
    pub const JUMP_START: u16 = 37;
    pub const JUMP: u16 = 38;
    pub const JUMP_END: u16 = 39;
    pub const FALL: u16 = 40;
    pub const SWIM_IDLE: u16 = 41;
    pub const SWIM: u16 = 42;
    pub const SWIM_BACKWARDS: u16 = 45;
}

/// Stand → Walk above this planar speed (yd/s), Walk → Stand below the lower one.
const WALK_ON: f32 = 0.6;
const WALK_OFF: f32 = 0.3;
/// Walk → Run above this, Run → Walk below the lower one: either side of halfway between the
/// stock walk (2.5) and run (7.0) speeds.
const RUN_ON: f32 = 4.9;
const RUN_OFF: f32 = 4.3;
/// A grounded body that loses the ground keeps its gait this long before Fall: stepping off a
/// kerb is not a fall.
const FALL_DELAY: f32 = 0.25;
/// A locked-on direction gait holds while its axis keeps this share of the larger axis.
const DIRECTION_KEEP: f32 = 0.7;
/// The velocity average's rate (1/s): about 80 ms to follow a real change.
const SMOOTH_RATE: f32 = 12.0;

/// The locomotion state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Gait {
    #[default]
    Stand,
    Walk,
    Run,
    JumpStart,
    Jump,
    JumpEnd,
    Fall,
    /// Locked on: stepping back, left or right while facing the target.
    Backpedal,
    StrafeLeft,
    StrafeRight,
    /// Afloat ([`crate::motor::swim`]): treading water, stroking, backing.
    SwimIdle,
    Swim,
    SwimBack,
}

impl Gait {
    pub fn anim_id(self) -> u16 {
        match self {
            Gait::Stand => ids::STAND,
            Gait::Walk => ids::WALK,
            Gait::Run => ids::RUN,
            Gait::JumpStart => ids::JUMP_START,
            Gait::Jump => ids::JUMP,
            Gait::JumpEnd => ids::JUMP_END,
            Gait::Fall => ids::FALL,
            Gait::Backpedal => ids::WALK_BACKWARDS,
            Gait::StrafeLeft => ids::SHUFFLE_LEFT,
            Gait::StrafeRight => ids::SHUFFLE_RIGHT,
            Gait::SwimIdle => ids::SWIM_IDLE,
            Gait::Swim => ids::SWIM,
            Gait::SwimBack => ids::SWIM_BACKWARDS,
        }
    }

    /// Plays once and hands over when done.
    pub fn one_shot(self) -> bool {
        matches!(self, Gait::JumpStart | Gait::JumpEnd)
    }

    /// Played at the gait rate.
    pub fn rate_scaled(self) -> bool {
        matches!(
            self,
            Gait::Walk
                | Gait::Run
                | Gait::Backpedal
                | Gait::StrafeLeft
                | Gait::StrafeRight
                | Gait::Swim
                | Gait::SwimBack
        )
    }

    /// A ground gait with the body moving.
    fn moving(self) -> bool {
        self.rate_scaled()
    }
}

/// What the state machine reads each frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocoInput {
    pub grounded: bool,
    pub jumped: bool,
    pub landed: bool,
    /// The averaged planar speed (yd/s).
    pub speed: f32,
    /// Seconds since the ground was lost (0 while grounded).
    pub airborne_for: f32,
    /// The playing one-shot has finished.
    pub clip_done: bool,
    /// Locked on: the averaged velocity in the body's frame, `(right, forward)`; the ground gait
    /// then follows the direction of travel instead of the speed alone.
    pub strafe: Option<Vec2>,
}

/// The ground gait for `speed`, with hysteresis around `current`.
pub fn ground_gait(current: Gait, speed: f32) -> Gait {
    match current {
        Gait::Run if speed >= RUN_OFF => Gait::Run,
        _ if speed >= RUN_ON => Gait::Run,
        Gait::Walk | Gait::Run if speed >= WALK_OFF => Gait::Walk,
        _ if speed >= WALK_ON => Gait::Walk,
        _ => Gait::Stand,
    }
}

/// The locked-on ground gait: still, or a step in the dominant direction of travel. Forward
/// keeps the speed gaits; the deadband is [`ground_gait`]'s.
pub fn strafe_gait(current: Gait, local: Vec2, speed: f32) -> Gait {
    let still = if current.moving() {
        speed < WALK_OFF
    } else {
        speed < WALK_ON
    };
    if still {
        return Gait::Stand;
    }
    // Direction hysteresis: a direction gait holds while its own axis keeps a share of the larger,
    // so travel on the diagonal (a body sliding along a wall) does not flap between two clips.
    let held_axis = match current {
        Gait::Walk | Gait::Run => Some(local.y),
        Gait::Backpedal => Some(-local.y),
        Gait::StrafeRight => Some(local.x),
        Gait::StrafeLeft => Some(-local.x),
        _ => None,
    };
    let dominant = local.x.abs().max(local.y.abs());
    if held_axis.is_some_and(|a| a > 0.0 && a >= DIRECTION_KEEP * dominant) {
        return match current {
            Gait::Walk | Gait::Run => ground_gait(current, speed),
            g => g,
        };
    }
    if local.y.abs() >= local.x.abs() {
        if local.y >= 0.0 {
            // Forward stays a walk or a run by speed.
            let held = match current {
                Gait::Walk | Gait::Run => current,
                _ => Gait::Walk,
            };
            ground_gait(held, speed)
        } else {
            Gait::Backpedal
        }
    } else if local.x > 0.0 {
        Gait::StrafeRight
    } else {
        Gait::StrafeLeft
    }
}

/// The next gait.
/// Afloat: treading water when still, else the stroke, or the backstroke when going backwards
/// (`local` is the velocity in the body's own frame, -Z forward).
pub fn swim_gait(speed: f32, local: Vec3) -> Gait {
    if speed < WALK_ON {
        Gait::SwimIdle
    } else if local.z > 0.3 * speed {
        Gait::SwimBack
    } else {
        Gait::Swim
    }
}

pub fn next_gait(current: Gait, i: LocoInput) -> Gait {
    if i.jumped {
        return Gait::JumpStart;
    }
    if !i.grounded {
        return match current {
            Gait::JumpStart if !i.clip_done => Gait::JumpStart,
            Gait::JumpStart | Gait::Jump => Gait::Jump,
            Gait::Fall => Gait::Fall,
            // Off a ledge: keep the stride a moment, then fall.
            _ if i.airborne_for >= FALL_DELAY => Gait::Fall,
            other => other,
        };
    }
    if i.landed && matches!(current, Gait::JumpStart | Gait::Jump | Gait::Fall) {
        // Landing at a run goes straight back to the run, as the stride hides the landing.
        return match ground_gait(Gait::Stand, i.speed) {
            Gait::Stand => Gait::JumpEnd,
            moving => moving,
        };
    }
    if current == Gait::JumpEnd && !i.clip_done && i.speed < WALK_ON {
        return Gait::JumpEnd;
    }
    match i.strafe {
        Some(local) => strafe_gait(current, local, i.speed),
        None => match ground_gait(current, i.speed) {
            // Unlocked, the body faces its travel: a strafe gait carried over walks.
            Gait::Backpedal | Gait::StrafeLeft | Gait::StrafeRight => Gait::Walk,
            g => g,
        },
    }
}

/// A body's locomotion animation state.
#[derive(Component, Debug, Default)]
pub struct Locomotion {
    pub gait: Gait,
    /// The averaged planar velocity.
    smoothed: Vec3,
    airborne_for: f32,
    /// The clip node playing for the gait, `None` until the first arm.
    playing: Option<AnimationNodeIndex>,
}

impl Locomotion {
    /// The clip node playing for the gait.
    pub fn playing(&self) -> Option<AnimationNodeIndex> {
        self.playing
    }
}

/// The clip a gait plays and whether it plays backwards. A model without the clip goes on as well as
/// it can: a creature without Jump stands through it, one without a backward step walks its walk
/// backwards (a reversed cycle), one without a sidestep walks.
pub fn gait_clip(anims: &ModelAnimations, gait: Gait) -> Option<(&benilla_assets::AnimClip, bool)> {
    if let Some(c) = anims.find(gait.anim_id()) {
        return Some((c, false));
    }
    match gait {
        Gait::Backpedal => anims.find(ids::WALK).map(|c| (c, true)),
        Gait::StrafeLeft | Gait::StrafeRight => anims.find(ids::WALK).map(|c| (c, false)),
        Gait::Swim | Gait::SwimBack => anims.find(ids::SWIM_IDLE).map(|c| (c, false)),
        _ => anims.find(ids::STAND).map(|c| (c, false)),
    }
}

/// Plays a gait's clip on the body; returns the node, or `None` when the model has no such clip.
fn arm(
    anims: &ModelAnimations,
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    gait: Gait,
    speed: f32,
) -> Option<AnimationNodeIndex> {
    let (clip, reversed) = gait_clip(anims, gait)?;
    let active = transitions.play(
        player,
        clip.node,
        Duration::from_secs_f32(clip.blend_time.max(0.0)),
    );
    active.set_repeat(if gait.one_shot() || !clip.looping {
        RepeatAnimation::Never
    } else {
        RepeatAnimation::Forever
    });
    active.set_speed(rate(gait, clip.move_speed, speed) * if reversed { -1.0 } else { 1.0 });
    Some(clip.node)
}

/// The playback rate: the gait rate for the travelling gaits, else 1. A backwards or sideways
/// clip is authored with a negative or zero move speed (the reference plays those at 1x); here
/// they scale by its magnitude, or by the stock walk speed, so a 4.5 yd/s strafe does not slide.
/// Deviation from the reference, for the locked-on strafe 1.12 does not have.
pub fn rate(gait: Gait, move_speed: f32, speed: f32) -> f32 {
    if !gait.rate_scaled() {
        return 1.0;
    }
    let divisor = match gait {
        Gait::Walk | Gait::Run | Gait::Swim => move_speed,
        _ if move_speed.abs() > 0.0 => move_speed.abs(),
        _ => crate::motor::WALK_SPEED,
    };
    if divisor > 0.0 {
        (speed / divisor).max(0.1)
    } else {
        1.0
    }
}

pub fn trace_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("WOWSOULS_TRACE").is_some())
}

/// Drives every motor-moved body's locomotion animation.
pub fn drive(
    time: Res<Time>,
    lock: Res<crate::lockon::LockOn>,
    mut bodies: Query<(
        &Motor,
        &Transform,
        Has<crate::avatar::Avatar>,
        Option<&crate::combat::Action>,
        Option<&crate::npc::Brain>,
        Option<&crate::npc::Reaction>,
        Has<crate::abilities::moves::Casting>,
        Option<&crate::village::Citizen>,
        Option<&crate::motor::swim::Swimming>,
        &ModelAnimations,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut Locomotion,
    )>,
) {
    let dt = time.delta_secs();
    for (
        m,
        xf,
        is_avatar,
        action,
        brain,
        reaction,
        casting,
        citizen,
        swimming,
        anims,
        mut player,
        mut transitions,
        mut loco,
    ) in &mut bodies
    {
        // An action (a swing, a flinch, a death, a cast) plays its own clip; the gait re-arms when it
        // lets go of the body.
        if citizen.is_some_and(crate::village::Citizen::owns_animation)
            || casting
            || action.is_some_and(|a| a.owns_body() && !a.guarding())
            || brain.is_some_and(crate::npc::Brain::owns_body)
            || reaction.is_some_and(|r| *r != crate::npc::Reaction::None)
        {
            loco.playing = None;
            loco.gait = Gait::Stand;
            continue;
        }
        let k = 1.0 - (-SMOOTH_RATE * dt).exp();
        let planar = Vec3::new(m.velocity.x, 0.0, m.velocity.z);
        loco.smoothed = loco.smoothed.lerp(planar, k);
        loco.airborne_for = if m.grounded {
            0.0
        } else {
            loco.airborne_for + dt
        };
        let clip_done = loco
            .playing
            .and_then(|n| player.animation(n))
            .is_none_or(|a| a.is_finished());
        let speed = loco.smoothed.length();
        let input = LocoInput {
            grounded: m.grounded,
            jumped: m.last.jumped,
            landed: m.last.landed,
            speed,
            airborne_for: loco.airborne_for,
            clip_done,
            // Locked on, the avatar steps by direction; a creature that is after the player does
            // too (it keeps its face to it while it circles and steps back).
            strafe: ((is_avatar && lock.target.is_some())
                || brain.is_some_and(|b| b.mind.engaged()))
            .then(|| {
                let local = xf.rotation.inverse() * loco.smoothed;
                Vec2::new(local.x, -local.z)
            }),
        };
        let next = match swimming.filter(|s| s.on) {
            Some(_) => swim_gait(speed, xf.rotation.inverse() * loco.smoothed),
            None => next_gait(loco.gait, input),
        };
        if next != loco.gait || loco.playing.is_none() || (m.last.jumped && next.one_shot()) {
            if trace_on() {
                let clip = anims.find(next.anim_id());
                info!(
                    "gait {:?} -> {next:?} speed={speed:.2} clip={:?}",
                    loco.gait,
                    clip.map(|c| (c.anim_id, c.duration, c.looping, c.move_speed, c.blend_time))
                );
            }
            loco.gait = next;
            loco.playing = arm(anims, &mut player, &mut transitions, next, speed);
        } else if next.rate_scaled() {
            // Keep the stride matched to the ground as the speed changes.
            if let Some((clip, reversed)) = gait_clip(anims, next) {
                if let Some(active) = loco.playing.and_then(|n| player.animation_mut(n)) {
                    active.set_speed(
                        rate(next, clip.move_speed, speed) * if reversed { -1.0 } else { 1.0 },
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ground(speed: f32) -> LocoInput {
        LocoInput {
            grounded: true,
            speed,
            clip_done: true,
            ..LocoInput::default()
        }
    }

    #[test]
    fn speed_picks_the_ground_gait() {
        assert_eq!(next_gait(Gait::Stand, ground(0.0)), Gait::Stand);
        assert_eq!(next_gait(Gait::Stand, ground(2.5)), Gait::Walk);
        assert_eq!(next_gait(Gait::Stand, ground(7.0)), Gait::Run);
    }

    #[test]
    fn the_thresholds_have_hysteresis() {
        // Between the on and off lines the gait holds.
        assert_eq!(next_gait(Gait::Walk, ground(0.45)), Gait::Walk);
        assert_eq!(next_gait(Gait::Stand, ground(0.45)), Gait::Stand);
        assert_eq!(next_gait(Gait::Run, ground(4.6)), Gait::Run);
        assert_eq!(next_gait(Gait::Walk, ground(4.6)), Gait::Walk);
        // Past them it changes.
        assert_eq!(next_gait(Gait::Walk, ground(0.2)), Gait::Stand);
        assert_eq!(next_gait(Gait::Run, ground(4.0)), Gait::Walk);
    }

    #[test]
    fn a_jump_runs_start_then_loop_then_end() {
        let jumped = LocoInput {
            grounded: false,
            jumped: true,
            ..LocoInput::default()
        };
        assert_eq!(next_gait(Gait::Stand, jumped), Gait::JumpStart);
        let rising = LocoInput {
            grounded: false,
            clip_done: false,
            ..LocoInput::default()
        };
        assert_eq!(next_gait(Gait::JumpStart, rising), Gait::JumpStart);
        let start_done = LocoInput {
            clip_done: true,
            ..rising
        };
        assert_eq!(next_gait(Gait::JumpStart, start_done), Gait::Jump);
        assert_eq!(next_gait(Gait::Jump, rising), Gait::Jump);
        let landed = LocoInput {
            grounded: true,
            landed: true,
            clip_done: false,
            ..LocoInput::default()
        };
        assert_eq!(next_gait(Gait::Jump, landed), Gait::JumpEnd);
        // JumpEnd plays out standing…
        assert_eq!(
            next_gait(
                Gait::JumpEnd,
                LocoInput {
                    clip_done: false,
                    ..ground(0.0)
                }
            ),
            Gait::JumpEnd
        );
        // …then stands.
        assert_eq!(next_gait(Gait::JumpEnd, ground(0.0)), Gait::Stand);
    }

    #[test]
    fn landing_at_a_run_skips_the_landing_clip() {
        let landed_running = LocoInput {
            grounded: true,
            landed: true,
            speed: 7.0,
            ..LocoInput::default()
        };
        assert_eq!(next_gait(Gait::Jump, landed_running), Gait::Run);
    }

    #[test]
    fn a_step_off_keeps_the_stride_then_falls() {
        let air = |t: f32| LocoInput {
            grounded: false,
            airborne_for: t,
            speed: 7.0,
            ..LocoInput::default()
        };
        assert_eq!(next_gait(Gait::Run, air(0.1)), Gait::Run);
        assert_eq!(next_gait(Gait::Run, air(0.3)), Gait::Fall);
        assert_eq!(next_gait(Gait::Fall, air(0.0)), Gait::Fall);
    }

    fn locked(local: Vec2, speed: f32) -> LocoInput {
        LocoInput {
            strafe: Some(local),
            ..ground(speed)
        }
    }

    #[test]
    fn locked_on_the_gait_follows_the_direction_of_travel() {
        assert_eq!(next_gait(Gait::Stand, locked(Vec2::Y, 4.5)), Gait::Walk);
        assert_eq!(
            next_gait(Gait::Stand, locked(Vec2::NEG_Y, 4.5)),
            Gait::Backpedal
        );
        assert_eq!(
            next_gait(Gait::Stand, locked(Vec2::X, 4.5)),
            Gait::StrafeRight
        );
        assert_eq!(
            next_gait(Gait::Stand, locked(Vec2::NEG_X, 4.5)),
            Gait::StrafeLeft
        );
        // A diagonal goes to its larger axis.
        assert_eq!(
            next_gait(Gait::Stand, locked(Vec2::new(0.9, -0.4), 4.5)),
            Gait::StrafeRight
        );
        assert_eq!(
            next_gait(Gait::StrafeLeft, locked(Vec2::ZERO, 0.0)),
            Gait::Stand
        );
    }

    #[test]
    fn locked_strafing_keeps_the_deadband() {
        // A moving gait holds down to the off line...
        assert_eq!(
            next_gait(Gait::StrafeLeft, locked(Vec2::NEG_X, 0.45)),
            Gait::StrafeLeft
        );
        // ...a still one waits for the on line.
        assert_eq!(
            next_gait(Gait::Stand, locked(Vec2::NEG_X, 0.45)),
            Gait::Stand
        );
    }

    #[test]
    fn a_diagonal_slide_keeps_its_strafe_gait() {
        // Sliding back-right along a wall, just either side of 45 degrees.
        let a = Vec2::new(0.70, -0.72);
        let b = Vec2::new(0.72, -0.70);
        assert_eq!(
            next_gait(Gait::StrafeRight, locked(a, 2.8)),
            Gait::StrafeRight
        );
        assert_eq!(
            next_gait(Gait::StrafeRight, locked(b, 2.8)),
            Gait::StrafeRight
        );
        assert_eq!(next_gait(Gait::Backpedal, locked(b, 2.8)), Gait::Backpedal);
        // A clear turn of direction still changes the gait.
        assert_eq!(
            next_gait(Gait::StrafeRight, locked(Vec2::NEG_Y, 2.8)),
            Gait::Backpedal
        );
        assert_eq!(
            next_gait(Gait::StrafeRight, locked(Vec2::NEG_X, 2.8)),
            Gait::StrafeLeft
        );
    }

    #[test]
    fn letting_go_of_the_lock_drops_the_strafe_gaits() {
        assert_eq!(next_gait(Gait::StrafeRight, ground(2.5)), Gait::Walk);
        assert_eq!(next_gait(Gait::Backpedal, ground(7.0)), Gait::Run);
    }

    #[test]
    fn strafes_play_at_their_magnitude_or_the_walk_speed() {
        assert!((rate(Gait::Backpedal, -2.5, 4.5) - 1.8).abs() < 1.0e-5);
        assert!((rate(Gait::StrafeLeft, 0.0, 2.5) - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn walk_and_run_play_at_the_ground_speed() {
        assert!((rate(Gait::Run, 7.0, 7.0) - 1.0).abs() < 1.0e-6);
        assert!((rate(Gait::Run, 7.0, 5.25) - 0.75).abs() < 1.0e-6);
        assert_eq!(rate(Gait::Stand, 0.0, 3.0), 1.0);
        assert_eq!(rate(Gait::Jump, 7.0, 7.0), 1.0);
        // An unauthored move speed plays at 1.
        assert_eq!(rate(Gait::Walk, 0.0, 2.5), 1.0);
    }
}
