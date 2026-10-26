//! The kinematic motor: one [`step`] a frame moves the avatar's capsule through the world. It
//! classifies the ground, jumps and falls on the reference's closed-form arc, and walks through
//! [`resolve::grounded_step`] (step-up, slide, step-down snap) or slides airborne through
//! [`resolve::airborne_step`].
//!
//! Ported from benilla @ 2e82d34 `crates/benilla-app/src/player/mover.rs` (`step`) and
//! `player/state.rs` (the constants), MIT OR Apache-2.0. Cut: the settle hold (ours is
//! [`crate::settle`]), root, hover, water walking, knockback and every wire/trace concern. What
//! moves the body is a [`MoveIntent`], written by [`crate::controller`] from camera-relative input.

use avian3d::prelude::*;
use bevy::prelude::*;

pub(crate) mod resolve;
pub mod swim;
#[cfg(test)]
pub(crate) mod testkit;

use resolve::{airborne_step, fall_step, grounded_step, Support};

// ── Constants (benilla `player/state.rs`, the 1.12.1 reference's values) ─────────────────────

/// Capsule radius (yd), the `CMovement` ctor's 1/3.
pub const CAPSULE_RADIUS: f32 = 1.0 / 3.0;
/// Capsule total height (yd).
pub const CAPSULE_HEIGHT: f32 = 2.027_777_7;
/// Gravity (yd/s²), the reference's `Movement::gravity`.
pub const GRAVITY: f32 = 19.291_105;
/// Jump take-off speed (yd/s); the apex is `v²/2g` = 1.64 yd.
pub const JUMP_SPEED: f32 = 7.955_547;
/// Terminal fall speed (yd/s).
pub const TERMINAL_VELOCITY: f32 = 60.148_003;
/// Run speed (yd/s), the stock `MOVE_RUN`.
pub const RUN_SPEED: f32 = 7.0;
/// Walk speed (yd/s), the stock `MOVE_WALK`.
pub const WALK_SPEED: f32 = 2.5;
/// Walkable iff the surface normal is within 50° of up.
pub(crate) const GROUND_COS: f32 = 0.642_788;
/// Downward probe distance (yd) deciding whether a grounded body still stands.
pub(crate) const GROUND_PROBE: f32 = 0.2;
/// The snap's slope ratio (atan ≈ 61.6°), also the foot cone's.
pub(crate) const STEP_SLOPE_RATIO: f32 = 1.849_399;
/// The snap reach's fixed slack (yd), 1/36.
pub(crate) const STEP_SNAP_SLACK: f32 = 0.027_777_8;
/// The step-up rise ceiling (yd) for a player body.
pub(crate) const STEP_UP_HEIGHT: f32 = 1.0;
/// The step-up certify advance (yd): `max(H·tan50°, radius + 1/720)` at `H` = 1.0.
pub(crate) const STEP_UP_ADVANCE: f32 = 1.191_753_6;
/// The certify reach per yard of rise budget, `tan 50°`.
pub(crate) const STEP_UP_ADVANCE_PER_YARD: f32 = STEP_UP_ADVANCE / STEP_UP_HEIGHT;
/// The foot cone's height (yd): an edge below it is ridden up the 61.6° skirt, one above pops.
pub(crate) const FOOT_CONE_HEIGHT: f32 = CAPSULE_RADIUS * STEP_SLOPE_RATIO;
/// Airborne, walk resumes only this close to the floor (yd).
pub(crate) const LAND_PROBE: f32 = 0.05;
/// Stalled airborne frames that count as resting wedged between two steep faces.
pub(crate) const WEDGE_STILL_FRAMES: u8 = 3;
/// A frame stalls when its descent is under this fraction of gravity's intent.
pub(crate) const WEDGE_STALL_RATIO: f32 = 0.15;
/// Fall speed (yd/s) before stalled frames count.
pub(crate) const WEDGE_MIN_FALL: f32 = 1.0;
/// The one-shot air nudge (yd/s) a standstill jump may steer by.
pub(crate) const AIR_NUDGE_SPEED: f32 = 2.5;
/// Skin width (yd) kept between the capsule and geometry on casts.
pub(crate) const SKIN_WIDTH: f32 = 0.02;

/// The longest frame the motor integrates in one step (s). A hitch longer than this is played
/// slower rather than swept as one huge move. Deviation: benilla integrates the raw frame time.
pub const MAX_STEP: f32 = 1.0 / 15.0;

/// What the body wants this frame, written by the controller and read by [`step`].
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct MoveIntent {
    /// Horizontal world direction scaled by how hard it is pushed, length `0..=1`.
    pub dir: Vec3,
    /// Speed at full push (yd/s); a negative one backs the body along `dir` (a clip's recoil).
    pub speed: f32,
    /// A jump was asked for this frame.
    pub jump: bool,
}

/// The motor's state on a body: its velocities and the airborne bookkeeping.
#[derive(Component, Default, Clone, Debug)]
pub struct Motor {
    /// Vertical speed (yd/s), up positive.
    pub vel_y: f32,
    /// Horizontal velocity (yd/s): the input while grounded, the locked momentum while airborne.
    pub horiz_vel: Vec3,
    /// What the body actually did last step (yd/s), walls and slopes included: a run into a wall
    /// asks for 7 yd/s and achieves about 0.
    pub velocity: Vec3,
    /// Supported at the end of the last step.
    pub grounded: bool,
    /// The walkable collider under the feet when the last grounded step settled onto one.
    pub ground: Option<Entity>,
    /// What the last step did: its take-off and touchdown edges, for the animation.
    pub last: Outcome,
    /// Airborne on the last step, a take-off frame included.
    airborne_prev: bool,
    /// The airborne arc's direction is set: a moving take-off is locked to its momentum, a
    /// standing one may steer once ([`AIR_NUDGE_SPEED`]).
    arc_dirs_set: bool,
    /// Resting pinched between steep faces (a tree trunk's funnel).
    wedged: bool,
    wedge_still: u8,
    /// On a certified steep contact (riding a step, following a surface down).
    steep_support: bool,
}

/// What one [`step`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outcome {
    /// Supported this frame.
    pub grounded: bool,
    /// Took off this frame.
    pub jumped: bool,
    /// Touched down this frame after being airborne.
    pub landed: bool,
}

/// Advances the body one frame: `feet` is the capsule's bottom in Bevy space.
pub fn step(
    m: &mut Motor,
    feet: &mut Vec3,
    dt: std::time::Duration,
    world: &benilla_world::collision::WorldCollision<'_, '_>,
    capsule: &Collider,
    intent: MoveIntent,
) -> Outcome {
    let dt = dt.min(std::time::Duration::from_secs_f32(MAX_STEP));
    let secs = dt.as_secs_f32();
    let push = Vec3::new(intent.dir.x, 0.0, intent.dir.z).clamp_length_max(1.0);
    let moving = push.length_squared() > 1.0e-6;
    let input_horiz = push * intent.speed;
    let half_h = Vec3::Y * (CAPSULE_HEIGHT * 0.5);
    let mut center = *feet + half_h;
    let probe_down =
        |c: Vec3, dist: f32| world.cast_mover(capsule, c, Vec3::NEG_Y * dist, SKIN_WIDTH);

    // Airborne, the ground reach tightens to [`LAND_PROBE`] so the arc ends where the slide meets
    // the floor.
    let reach = if m.airborne_prev {
        LAND_PROBE
    } else {
        GROUND_PROBE
    };
    let on_walkable = probe_down(center, reach).is_some_and(|h| h.normal1.y >= GROUND_COS);
    // A body part-way up a foot cone stands: the straight-down probe sees only the riser it rides.
    let on_floor = (on_walkable || m.steep_support) && m.vel_y <= 0.0;
    // The wedge rest holds until real ground takes over or its support goes.
    if m.wedged && (on_floor || probe_down(center, LAND_PROBE).is_none()) {
        m.wedged = false;
    }
    let grounded = on_floor || m.wedged;

    let mut jumped = false;
    if grounded {
        m.vel_y = 0.0;
        if intent.jump {
            m.vel_y = JUMP_SPEED;
            m.wedged = false;
            m.steep_support = false;
            jumped = true;
            // A moving jump is locked to its momentum; a standing one may steer once.
            m.arc_dirs_set = moving;
        }
    } else if !m.airborne_prev {
        // A step-off: the arc's direction is seeded from the input.
        m.arc_dirs_set = moving;
    }

    // The fall runs on every airborne frame, the take-off's included, in closed form, so the arc
    // is the same at any frame rate.
    let mut fall_mean_vy = m.vel_y;
    if jumped || !grounded {
        let (end_vy, mean_vy) = fall_step(m.vel_y, secs, TERMINAL_VELOCITY);
        m.vel_y = end_vy;
        fall_mean_vy = mean_vy;
    }
    if grounded {
        m.horiz_vel = input_horiz;
    } else if moving && !m.arc_dirs_set {
        // Air control: one nudge for a jump taken standing.
        m.horiz_vel = push.normalize_or_zero() * AIR_NUDGE_SPEED;
        m.arc_dirs_set = true;
    }

    let pre_move = center;
    if grounded && !jumped {
        let g = grounded_step(
            world,
            capsule,
            center,
            m.horiz_vel,
            dt,
            Support {
                steep: m.steep_support,
                ..Support::default()
            },
        );
        // Only a mover with a fall to elect spends the no-floor drop, so a ledge's first fall
        // frame starts lower.
        center = g.center - Vec3::Y * g.unsupported.unwrap_or(0.0);
        m.steep_support = g.steep_support;
        m.ground = g.ground;
    } else {
        center = airborne_step(
            world,
            capsule,
            center,
            m.horiz_vel + Vec3::Y * fall_mean_vy,
            dt,
        );
        m.steep_support = false;
        m.ground = None;
    }
    // The wedge rest: falling, yet achieving under [`WEDGE_STALL_RATIO`] of gravity's intent for
    // [`WEDGE_STILL_FRAMES`] frames, is a capsule pinched with no way down, so it lands there.
    if !grounded
        && !jumped
        && m.vel_y < -WEDGE_MIN_FALL
        && (pre_move.y - center.y) < -m.vel_y * secs * WEDGE_STALL_RATIO
    {
        m.wedge_still += 1;
        if m.wedge_still >= WEDGE_STILL_FRAMES {
            m.wedged = true;
            m.wedge_still = 0;
            m.vel_y = 0.0;
        }
    } else {
        m.wedge_still = 0;
    }
    let grounded = grounded || m.wedged;

    let new_feet = center - half_h;
    if secs > 0.0 {
        m.velocity = (new_feet - *feet) / secs;
    }
    *feet = new_feet;
    let was_airborne = m.airborne_prev;
    m.airborne_prev = jumped || !grounded;
    m.grounded = grounded && !jumped;
    m.last = Outcome {
        grounded: m.grounded,
        jumped,
        landed: was_airborne && grounded && !jumped,
    };
    m.last
}

#[cfg(test)]
mod tests {
    use super::testkit::{player_capsule, world_from_profile};
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    const DT: f32 = 1.0 / 60.0;

    /// A flat floor at y = 0 from x = -20 to 20, with a sheer drop to y = -3 past x = 2 when
    /// `ledge` is set.
    fn floor(ledge: bool) -> App {
        if ledge {
            world_from_profile(&[(-20.0, 0.0), (2.0, 0.0), (2.01, -3.0), (20.0, -3.0)])
        } else {
            world_from_profile(&[(-20.0, 0.0), (20.0, 0.0)])
        }
    }

    /// Runs `frames` motor steps from `feet`, with `intent(frame)`; returns each frame's feet and
    /// outcome.
    fn run(
        mut app: App,
        feet: Vec3,
        frames: usize,
        intent: fn(usize) -> MoveIntent,
    ) -> Vec<(Vec3, Outcome, Vec3)> {
        app.world_mut()
            .run_system_once(move |world: benilla_world::collision::WorldCollision| {
                let capsule = player_capsule();
                let mut m = Motor::default();
                let mut feet = feet;
                (0..frames)
                    .map(|i| {
                        let o = step(
                            &mut m,
                            &mut feet,
                            std::time::Duration::from_secs_f32(DT),
                            &world,
                            &capsule,
                            intent(i),
                        );
                        (feet, o, m.horiz_vel)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap()
    }

    fn idle(_: usize) -> MoveIntent {
        MoveIntent::default()
    }

    fn run_x(_: usize) -> MoveIntent {
        MoveIntent {
            dir: Vec3::X,
            speed: RUN_SPEED,
            jump: false,
        }
    }

    #[test]
    fn a_body_standing_on_flat_ground_stays_put_and_grounded() {
        let frames = run(floor(false), Vec3::ZERO, 60, idle);
        for (i, (feet, o, _)) in frames.iter().enumerate() {
            assert!(o.grounded, "frame {i} lost the ground");
            assert!(feet.length() < 1.0e-3, "frame {i} drifted to {feet:?}");
        }
    }

    #[test]
    fn running_covers_the_run_speed_on_flat_ground() {
        let frames = run(floor(false), Vec3::new(-10.0, 0.0, 0.0), 60, run_x);
        let (feet, o, _) = frames.last().unwrap();
        assert!(o.grounded);
        assert!(
            (feet.x - (-10.0 + RUN_SPEED)).abs() < 0.05,
            "one second at a run should cover {RUN_SPEED} yd, ended at {feet:?}"
        );
        assert!(
            feet.y.abs() < 1.0e-3,
            "no height change on the flat: {feet:?}"
        );
    }

    #[test]
    fn a_negative_speed_backs_the_body_along_its_direction() {
        fn back_x(_: usize) -> MoveIntent {
            MoveIntent {
                dir: Vec3::X,
                speed: -RUN_SPEED,
                jump: false,
            }
        }
        let frames = run(floor(false), Vec3::new(-10.0, 0.0, 0.0), 60, back_x);
        let (feet, o, _) = frames.last().unwrap();
        assert!(o.grounded);
        assert!(
            (feet.x - (-10.0 - RUN_SPEED)).abs() < 0.05,
            "one second backing at a run should cover {RUN_SPEED} yd the other way, ended at {feet:?}"
        );
    }

    #[test]
    fn a_standing_jump_reaches_the_reference_apex_and_lands() {
        fn jump_once(i: usize) -> MoveIntent {
            MoveIntent {
                jump: i == 0,
                ..MoveIntent::default()
            }
        }
        let frames = run(floor(false), Vec3::ZERO, 90, jump_once);
        assert!(frames[0].1.jumped, "the first frame takes off");
        let apex = frames.iter().map(|f| f.0.y).fold(f32::MIN, f32::max);
        let want = JUMP_SPEED * JUMP_SPEED / (2.0 * GRAVITY);
        assert!(
            (apex - want).abs() < 0.05,
            "apex should be ~{want:.3} yd, got {apex:.3}"
        );
        let landed = frames.iter().position(|f| f.1.landed).expect("it lands");
        assert!(frames[landed..].iter().all(|f| f.1.grounded));
        // The arc ends where the slide meets the floor: within the landing probe plus the skin.
        let rest = frames.last().unwrap().0.y;
        assert!(
            (0.0..=LAND_PROBE + SKIN_WIDTH).contains(&rest),
            "back on the floor: {rest:.4}"
        );
    }

    #[test]
    fn a_moving_jump_keeps_its_momentum_whatever_the_input() {
        fn jump_then_reverse(i: usize) -> MoveIntent {
            MoveIntent {
                dir: if i == 0 { Vec3::X } else { Vec3::NEG_X },
                speed: RUN_SPEED,
                jump: i == 0,
            }
        }
        let frames = run(
            floor(false),
            Vec3::new(-10.0, 0.0, 0.0),
            30,
            jump_then_reverse,
        );
        for (i, (_, o, vel)) in frames.iter().enumerate().take(20) {
            if !o.grounded {
                assert!(
                    (vel.x - RUN_SPEED).abs() < 1.0e-4,
                    "airborne frame {i} changed its momentum: {vel:?}"
                );
            }
        }
    }

    #[test]
    fn running_off_a_ledge_falls_and_lands_below() {
        let frames = run(floor(true), Vec3::ZERO, 120, run_x);
        let first_air = frames
            .iter()
            .position(|f| !f.1.grounded)
            .expect("the body leaves the ledge");
        assert!(
            frames[first_air].0.x > 1.5,
            "it leaves at the ledge, not before"
        );
        let (feet, o, _) = frames.last().unwrap();
        assert!(o.grounded, "it lands");
        assert!(
            (0.0..=LAND_PROBE + SKIN_WIDTH).contains(&(feet.y + 3.0)),
            "and stands on the lower floor: {feet:?}"
        );
        assert!(
            frames.iter().any(|f| f.1.landed),
            "the touchdown is reported"
        );
    }

    #[test]
    fn a_run_into_a_wall_reports_what_it_achieved_not_what_it_asked() {
        // A 5 yd sheer wall at x = 1, facing the approaching body.
        let wall = world_from_profile(&[(-20.0, 0.0), (1.0, 0.0), (1.001, 5.0), (20.0, 5.0)]);
        let mut app = wall;
        let (asked, achieved, feet) = app
            .world_mut()
            .run_system_once(|world: benilla_world::collision::WorldCollision| {
                let capsule = player_capsule();
                let mut m = Motor::default();
                let mut feet = Vec3::ZERO;
                for _ in 0..60 {
                    step(
                        &mut m,
                        &mut feet,
                        std::time::Duration::from_secs_f32(DT),
                        &world,
                        &capsule,
                        run_x(0),
                    );
                }
                (m.horiz_vel.length(), m.velocity.length(), feet)
            })
            .unwrap();
        assert!((asked - RUN_SPEED).abs() < 1.0e-4);
        assert!(feet.x < 1.0, "the wall stops the body: {feet:?}");
        assert!(
            achieved < 0.1,
            "pressed against the wall it goes nowhere: {achieved}"
        );
    }

    #[test]
    fn a_long_hitch_is_capped_to_one_max_step() {
        let mut app = floor(false);
        let feet = app
            .world_mut()
            .run_system_once(|world: benilla_world::collision::WorldCollision| {
                let capsule = player_capsule();
                let mut m = Motor::default();
                let mut feet = Vec3::new(-10.0, 0.0, 0.0);
                step(
                    &mut m,
                    &mut feet,
                    std::time::Duration::from_secs(2),
                    &world,
                    &capsule,
                    run_x(0),
                );
                feet
            })
            .unwrap();
        assert!(
            (feet.x - (-10.0 + RUN_SPEED * MAX_STEP)).abs() < 1.0e-3,
            "a 2 s frame moves one MAX_STEP's worth: {feet:?}"
        );
    }
}
