//! Camera-relative control, Souls-style: the stick (or WASD) pushes the body along the camera's
//! ground-plane basis, and the body turns to face where it goes. Turning is a fast slerp, not
//! WoW's facing-relative tank steering. The motor ([`crate::motor`]) then moves the capsule.
//!
//! Keys: WASD move, Ctrl walk, Shift+Space jump (Space alone is the evade,
//! [`crate::combat`]). Gamepad: left stick move (analog), South jump, East evade.

use bevy::prelude::*;

use benilla_world::collision::WorldCollision;
use benilla_world::schedule::WorldStage;

use crate::avatar::{Avatar, BodyShape};
use crate::camera::{CamMode, OrbitCam};
use crate::motor::{self, Motor, MoveIntent, RUN_SPEED, WALK_SPEED};
use crate::settings::binds::{Action, Binds, Held};
use crate::settings::Controls;
use crate::settle::Phase;

/// How fast the body turns toward its move direction (rad/s): about two full turns a second.
pub const TURN_RATE: f32 = 12.0;
/// The locked-on strafe speed (yd/s), the stock `MOVE_RUN_BACK` 4.5.
pub const LOCKED_SPEED: f32 = 4.5;
/// A stick push under this length is no push.
const STICK_DEADZONE: f32 = 0.15;
/// A body this far below its spawn has left the world: it is put back at the spawn.
const VOID_DEPTH: f32 = 150.0;

/// The per-frame order inside [`WorldStage::Input`]: read input, move the body, seat the camera.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaySet {
    Intent,
    Motor,
    Camera,
}

/// Where the hold released the body, on the ground: the void-out return point.
#[derive(Resource, Clone, Copy, Debug)]
pub struct SpawnPoint(pub Vec3);

pub fn plugin(app: &mut App) {
    app.configure_sets(
        Update,
        (PlaySet::Intent, PlaySet::Motor, PlaySet::Camera)
            .chain()
            .in_set(WorldStage::Input)
            .run_if(in_state(Phase::Playing)),
    )
    .add_systems(Update, read_intent.in_set(PlaySet::Intent))
    .add_systems(
        Update,
        (drive_motor, face_motion).chain().in_set(PlaySet::Motor),
    );
    // Instruments for unattended verification runs.
    if let Some(drive) = std::env::var("WOWSOULS_DRIVE")
        .ok()
        .and_then(|v| parse_drive(&v))
    {
        app.insert_resource(drive);
    }
    if let Some(window) = std::env::var("WOWSOULS_DRIVE_FROM")
        .ok()
        .and_then(|v| parse_drive_window(&v))
    {
        app.insert_resource(window);
    }
    if std::env::var_os("WOWSOULS_TRACE").is_some() {
        app.add_systems(
            Update,
            trace_body
                .after(PlaySet::Motor)
                .run_if(in_state(Phase::Playing)),
        );
    }
}

/// `WOWSOULS_DRIVE=<x>,<y>[,<jump every s>]`: hold a constant stick push (x right, y forward) for
/// the whole run, jumping on a period if asked. An unattended-run instrument.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct ScriptedDrive {
    pub push: Vec2,
    pub jump_every: Option<f32>,
}

pub fn parse_drive(v: &str) -> Option<ScriptedDrive> {
    let parts: Vec<f32> = v
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect::<Option<_>>()?;
    let (push, jump_every) = match parts[..] {
        [x, y] => (Vec2::new(x, y), None),
        [x, y, j] if j > 0.0 => (Vec2::new(x, y), Some(j)),
        _ => return None,
    };
    Some(ScriptedDrive {
        push: push.clamp_length_max(1.0),
        jump_every,
    })
}

/// `WOWSOULS_DRIVE_FROM=<s>[,<until s>]`: the scripted drive pushes only from `s` (and until the
/// second number) seconds of real time, the clock `WOWSOULS_ATTACK` runs on: stand and fight, then walk
/// off; or walk off, then stand. An unattended-run instrument.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct DriveWindow {
    pub from: f32,
    pub until: f32,
}

impl DriveWindow {
    pub fn contains(&self, now: f32) -> bool {
        now >= self.from && now < self.until
    }
}

pub fn parse_drive_window(v: &str) -> Option<DriveWindow> {
    let parts: Vec<f32> = v
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect::<Option<_>>()?;
    match parts[..] {
        [from] => Some(DriveWindow {
            from,
            until: f32::INFINITY,
        }),
        [from, until] if until > from => Some(DriveWindow { from, until }),
        _ => None,
    }
}

/// `WOWSOULS_TRACE=1`: the body's WoW position, ground state and speed, four times a second.
fn trace_body(
    time: Res<Time<bevy::time::Real>>,
    avatar: Query<
        (
            &Transform,
            &Motor,
            Option<&crate::actor::anim::Locomotion>,
            Option<&AnimationPlayer>,
        ),
        With<Avatar>,
    >,
    mut next_at: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now < *next_at {
        return;
    }
    *next_at = now + 0.25;
    if let Ok((xf, m, loco, player)) = avatar.single() {
        let seek = loco
            .and_then(|l| l.playing())
            .zip(player)
            .and_then(|(n, p)| p.animation(n))
            .map(|a| (a.seek_time(), a.is_finished()));
        let wow = benilla_assets::coords::bevy_to_wow(xf.translation);
        info!(
            "trace t={now:6.2} wow=({:9.2},{:8.2},{:7.2}) yaw={:+.2} {} asked={:.2} moved={:.2} vy={:+.2} gait={:?} seek={:?}",
            wow[0],
            wow[1],
            wow[2],
            xf.rotation.to_euler(EulerRot::YXZ).0,
            if m.grounded { "ground" } else { "air   " },
            m.horiz_vel.length(),
            m.velocity.xz().length(),
            m.vel_y,
            loco.map(|l| l.gait),
            seek
        );
    }
}

/// The raw move input this frame: a 2D push (x right, y forward, length `0..=1`), walk, jump.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PadInput {
    pub push: Vec2,
    pub walk: bool,
    pub jump: bool,
}

/// The keys (the player's bindings: move, walk, jump) and the first connected gamepad's left stick,
/// merged: the larger push wins. A pad button is a binding like a key (jump is South by default).
fn read_pad(binds: &Binds, held: &Held, gamepads: &Query<&Gamepad>) -> PadInput {
    let keyboard = Vec2::new(
        binds.axis(Action::MoveLeft, Action::MoveRight, held),
        binds.axis(Action::MoveBack, Action::MoveForward, held),
    )
    .clamp_length_max(1.0);
    let mut input = PadInput {
        push: keyboard,
        walk: binds.pressed(Action::Walk, held),
        jump: binds.just_pressed(Action::Jump, held),
    };
    if let Some(pad) = gamepads.iter().next() {
        let stick = deadzone(pad.left_stick(), STICK_DEADZONE);
        if stick.length() > input.push.length() {
            input.push = stick;
        }
    }
    input
}

/// A radial deadzone, rescaled so the push still reaches 1 at the rim.
pub fn deadzone(v: Vec2, dz: f32) -> Vec2 {
    let len = v.length();
    if len <= dz {
        return Vec2::ZERO;
    }
    v / len * ((len - dz) / (1.0 - dz)).min(1.0)
}

/// The world move direction for a push under a camera at `yaw`: forward is the camera's look
/// direction flattened onto the ground, right is its right.
pub fn camera_relative(push: Vec2, yaw: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    let forward = Vec3::new(-s, 0.0, -c);
    let right = Vec3::new(c, 0.0, -s);
    (right * push.x + forward * push.y).clamp_length_max(1.0)
}

pub fn read_intent(
    time: Res<Time>,
    controls: Controls,
    gamepads: Query<&Gamepad>,
    drive: Option<Res<ScriptedDrive>>,
    (window, real): (Option<Res<DriveWindow>>, Res<Time<bevy::time::Real>>),
    unattended: Option<Res<crate::run_mode::Unattended>>,
    mut since_jump: Local<f32>,
    mode: Res<CamMode>,
    loot_screen: Res<crate::loot::Screen>,
    cam: Query<&OrbitCam>,
    lock: Res<crate::lockon::LockOn>,
    mut avatar: Query<&mut MoveIntent, With<Avatar>>,
) {
    let Ok(mut intent) = avatar.single_mut() else {
        return;
    };
    if loot_screen.locked() {
        *intent = MoveIntent::default();
        return;
    }
    // The free camera owns WASD; the body stands.
    let (CamMode::Orbit, Ok(cam)) = (*mode, cam.single()) else {
        *intent = MoveIntent::default();
        return;
    };
    let mut pad = if unattended.is_some() {
        PadInput::default()
    } else {
        controls.read(|binds, held| read_pad(binds, held, &gamepads))
    };
    let driving = window
        .as_deref()
        .is_none_or(|w| w.contains(real.elapsed_secs()));
    if let Some(drive) = drive.as_deref().filter(|_| driving) {
        pad.push = drive.push;
        *since_jump += time.delta_secs();
        if drive.jump_every.is_some_and(|every| *since_jump >= every) {
            *since_jump = 0.0;
            pad.jump = true;
        }
    }
    *intent = MoveIntent {
        dir: camera_relative(pad.push, cam.yaw),
        speed: match (pad.walk, lock.target.is_some()) {
            (true, _) => WALK_SPEED,
            // Locked on, the body strafes at the stock backpedal speed.
            (false, true) => LOCKED_SPEED,
            (false, false) => RUN_SPEED,
        },
        // A jump press that confirmed a dialogue is the dialogue's, not the body's.
        jump: pad.jump && !loot_screen.blocks_actions(real.elapsed_secs()),
    };
}

fn drive_motor(
    time: Res<Time>,
    collide: WorldCollision,
    shape: Res<BodyShape>,
    spawn: Option<Res<SpawnPoint>>,
    mut avatar: Query<(&mut Transform, &mut Motor, &MoveIntent), With<Avatar>>,
) {
    let Ok((mut xf, mut m, intent)) = avatar.single_mut() else {
        return;
    };
    let mut feet = xf.translation;
    motor::step(&mut m, &mut feet, time.delta(), &collide, &shape.0, *intent);
    if let Some(spawn) = spawn.as_deref() {
        if feet.y < spawn.0.y - VOID_DEPTH {
            warn!("motor: the body fell out of the world at {feet:?}; back to the spawn");
            feet = spawn.0;
            *m = Motor::default();
        }
    }
    xf.translation = feet;
}

/// Turns a grounded, moving body toward its move direction at [`TURN_RATE`]. An evade holds the
/// facing it set out with.
fn face_motion(
    time: Res<Time>,
    lock: Res<crate::lockon::LockOn>,
    interaction: Res<crate::loot::interaction::Active>,
    targets: Query<(&GlobalTransform, &crate::lockon::Lockable)>,
    mut avatar: Query<
        (
            &mut Transform,
            &Motor,
            &MoveIntent,
            Option<&crate::combat::Action>,
            Option<&crate::shove::Shove>,
        ),
        With<Avatar>,
    >,
) {
    let Ok((mut xf, m, intent, action, shoved)) = avatar.single_mut() else {
        return;
    };
    if interaction.0.is_some() {
        return;
    }
    // A knock-back pushes the body; it does not turn it.
    let reeling = action.is_some_and(|a| {
        matches!(
            a.state,
            crate::combat::ActionState::Reel(_)
                | crate::combat::ActionState::Defend(crate::defense::Playing {
                    kind: crate::defense::Kind::Parry | crate::defense::Kind::Break(_),
                    ..
                })
        )
    });
    if !m.grounded || shoved.is_some() || reeling || action.is_some_and(|a| a.evading().is_some()) {
        return;
    }
    // Locked, the body faces the target whatever it does; otherwise where it goes.
    let wanted = match crate::lockon::locked_aim(&lock, &targets) {
        Some(aim) => {
            let to = aim - xf.translation;
            Vec3::new(to.x, 0.0, to.z)
        }
        None => intent.dir,
    };
    if wanted.length_squared() < 1.0e-6 {
        return;
    }
    let current = xf.rotation.to_euler(EulerRot::YXZ).0;
    let next = turn_toward(current, yaw_of(wanted), TURN_RATE * time.delta_secs());
    xf.rotation = Quat::from_rotation_y(next);
}

/// The yaw whose forward (`-Z` rotated) is `dir`.
pub fn yaw_of(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}

/// `from` turned toward `to` by at most `max_step` radians, the short way round.
pub fn turn_toward(from: f32, to: f32, max_step: f32) -> f32 {
    let gap = wrap_pi(to - from);
    from + gap.clamp(-max_step, max_step)
}

/// An angle wrapped into `(-π, π]`.
pub fn wrap_pi(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let r = (a + PI).rem_euclid(TAU) - PI;
    if r <= -PI {
        r + TAU
    } else {
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    /// `read_pad` with the default bindings over a keyboard and the pad query.
    fn read_with(keys: &ButtonInput<KeyCode>, pads: &Query<&Gamepad>) -> PadInput {
        let mouse = ButtonInput::<MouseButton>::default();
        let all: Vec<&Gamepad> = pads.iter().collect();
        let held = Held {
            keys,
            mouse: &mouse,
            pads: &all,
            captured: false,
        };
        read_pad(&Binds::default(), &held, pads)
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1.0e-5
    }

    #[test]
    fn forward_is_where_the_camera_looks() {
        // A camera at yaw 0 looks down -Z.
        assert!(close(camera_relative(Vec2::Y, 0.0), Vec3::NEG_Z));
        assert!(close(camera_relative(Vec2::X, 0.0), Vec3::X));
        // Turned a quarter left, forward is -X.
        assert!(close(camera_relative(Vec2::Y, FRAC_PI_2), Vec3::NEG_X));
    }

    #[test]
    fn the_basis_matches_a_bevy_transform() {
        for yaw in [0.0, 0.7, -2.1, 3.0] {
            let xf = Transform::from_rotation(Quat::from_rotation_y(yaw));
            assert!(close(camera_relative(Vec2::Y, yaw), *xf.forward()));
            assert!(close(camera_relative(Vec2::X, yaw), *xf.right()));
        }
    }

    #[test]
    fn a_diagonal_push_is_no_faster_than_a_straight_one() {
        let d = camera_relative(Vec2::new(1.0, 1.0), 0.3);
        assert!((d.length() - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn yaw_of_inverts_the_forward_basis() {
        for yaw in [0.0, 1.0, -1.0, 2.5, -3.0] {
            let fwd = camera_relative(Vec2::Y, yaw);
            assert!(wrap_pi(yaw_of(fwd) - yaw).abs() < 1.0e-5);
        }
    }

    #[test]
    fn turning_takes_the_short_way_and_stops_on_target() {
        // From just under +π to just over -π is a small step across the seam.
        let t = turn_toward(PI - 0.1, -PI + 0.1, 1.0);
        assert!(wrap_pi(t - (-PI + 0.1)).abs() < 1.0e-5);
        // A big gap is clamped to the step.
        assert!((turn_toward(0.0, 2.0, 0.5) - 0.5).abs() < 1.0e-6);
        assert!((turn_toward(0.0, -2.0, 0.5) + 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn deadzone_kills_drift_and_keeps_the_rim() {
        assert_eq!(deadzone(Vec2::new(0.1, 0.05), 0.15), Vec2::ZERO);
        assert!((deadzone(Vec2::new(1.0, 0.0), 0.15).length() - 1.0).abs() < 1.0e-6);
        let half = deadzone(Vec2::new(0.575, 0.0), 0.15).length();
        assert!((half - 0.5).abs() < 1.0e-4, "{half}");
    }

    #[test]
    fn drive_parses_a_push_and_an_optional_jump_period() {
        assert_eq!(
            parse_drive("0,1"),
            Some(ScriptedDrive {
                push: Vec2::Y,
                jump_every: None
            })
        );
        let d = parse_drive("1, 1, 2").unwrap();
        assert!((d.push.length() - 1.0).abs() < 1.0e-6, "clamped to the rim");
        assert_eq!(d.jump_every, Some(2.0));
        assert_eq!(parse_drive("1"), None);
        assert_eq!(parse_drive("0,1,-2"), None);
        assert_eq!(parse_drive("a,b"), None);
    }

    #[test]
    fn the_drive_window_parses_a_start_and_an_optional_end() {
        let w = parse_drive_window("12").unwrap();
        assert!(!w.contains(11.9) && w.contains(12.0) && w.contains(1.0e6));
        let w = parse_drive_window("12, 20").unwrap();
        assert!(w.contains(12.0) && w.contains(19.9) && !w.contains(20.0));
        assert_eq!(parse_drive_window("20,12"), None);
        assert_eq!(parse_drive_window("x"), None);
        assert_eq!(parse_drive_window("1,2,3"), None);
    }

    #[test]
    fn wrap_pi_stays_in_range() {
        for a in [-10.0, -PI, -1.0, 0.0, 1.0, PI, 7.0, 100.0] {
            let w = wrap_pi(a);
            assert!(w > -PI - 1.0e-6 && w <= PI + 1.0e-6, "{a} -> {w}");
            // Wrapping only removes whole turns.
            let turns = (a - w) / std::f32::consts::TAU;
            assert!((turns - turns.round()).abs() < 1.0e-4, "{a} -> {w}");
        }
    }

    #[test]
    fn the_pad_stick_moves_the_body_and_a_jumps() {
        use crate::pad::testkit::{read, world_with_pad};
        let mut world = world_with_pad(|pad| {
            pad.analog_mut().set(GamepadAxis::LeftStickX, 1.0);
            pad.digital_mut().press(GamepadButton::South);
        });
        let keys = ButtonInput::<KeyCode>::default();
        let input = read(&mut world, move |pads| read_with(&keys, pads));
        assert!((input.push - Vec2::X).length() < 1.0e-5, "{:?}", input.push);
        assert!(input.jump);
    }

    #[test]
    fn a_stick_inside_the_deadzone_does_not_move_the_body() {
        use crate::pad::testkit::{read, world_with_pad};
        let mut world = world_with_pad(|pad| {
            pad.analog_mut().set(GamepadAxis::LeftStickX, 0.1);
        });
        let keys = ButtonInput::<KeyCode>::default();
        let input = read(&mut world, move |pads| read_with(&keys, pads));
        assert_eq!(input.push, Vec2::ZERO);
        assert!(!input.jump);
    }

    #[test]
    fn keyboard_and_stick_merge_and_the_larger_push_wins() {
        use crate::pad::testkit::{read, world_with_pad};
        let mut world = world_with_pad(|pad| {
            pad.analog_mut().set(GamepadAxis::LeftStickX, 0.5);
        });
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyW);
        let input = read(&mut world, move |pads| read_with(&keys, pads));
        assert_eq!(
            input.push,
            Vec2::Y,
            "the full key push beats the half stick"
        );
    }
}
