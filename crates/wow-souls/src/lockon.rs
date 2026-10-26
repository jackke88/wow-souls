//! Lock-on, Souls-style. One button (Q, middle mouse, right-stick click) locks the best
//! [`Lockable`] in front of the camera, or with none in reach recentres the camera behind the
//! body; pressed while locked it lets go. Locked, the camera holds the target in view, the body
//! faces it and strafes, and the lock breaks past [`LOCK_BREAK`].
//!
//! The best target is the one nearest the view centre, distance breaking near-ties; a target
//! behind the camera's [`LOCK_CONE`] is never picked.
//!
//! Locked, a deliberate horizontal swipe, right-stick flick or look key moves
//! the lock to the nearest target on that side of the current one, without letting go first
//! ([`Flick`], [`next_target`]).

mod check;
mod input;
use input::{Flick, InputScript};

use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::camera::{OrbitCam, Rig};
use crate::controller::{wrap_pi, yaw_of, PlaySet};
use crate::settings::binds::Action;
#[cfg(test)]
use crate::settings::binds::Binds;
use crate::settings::Controls;
use crate::settle::Phase;
use bevy::input::gamepad::Gamepad;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

/// How far a target may be to lock on (yd).
pub const LOCK_RANGE: f32 = 20.0;
/// How far a locked target may go before the lock breaks (yd).
pub const LOCK_BREAK: f32 = 30.0;
/// The most a target may be above or below the player (yd) to be locked: one under the ground (a mine's
/// tunnel under a meadow) or on a floor far overhead is not in the fight.
pub const LOCK_RISE: f32 = 6.0;
/// Seconds a locked target may stay out of sight (behind a wall, round a corner) before the lock breaks.
const LOST_SIGHT_SECS: f32 = 2.0;
/// The player's eye over the feet (yd) for the sight checks.
const EYE: f32 = 1.5;

/// Whether `to` can be seen from `eye`: the camera's own probe meets nothing solid on the way (terrain,
/// a building, a mine's rock; bodies are not on its layer).
pub fn in_sight(world: &benilla_world::collision::WorldCollision, eye: Vec3, to: Vec3) -> bool {
    let d = to - eye;
    let len = d.length();
    len < 0.5
        || world
            .cast_camera(eye, d, false)
            .is_none_or(|hit| hit >= len - 0.4)
}

/// [`in_sight`] where the world may not be there (a test app): without it, everything is in sight.
fn seen(world: Option<&benilla_world::collision::WorldCollision>, eye: Vec3, to: Vec3) -> bool {
    world.is_none_or(|w| in_sight(w, eye, to))
}

/// Whether a target with its feet at `at` and its aim point `aim_height` above them can be locked from a
/// player whose feet are at `feet`.
fn lockable_from(
    world: Option<&benilla_world::collision::WorldCollision>,
    feet: Vec3,
    at: Vec3,
    aim_height: f32,
) -> bool {
    (at.y - feet.y).abs() <= LOCK_RISE
        && seen(world, feet + Vec3::Y * EYE, at + Vec3::Y * aim_height)
}
/// Half-angle of the cone ahead of the camera a target must be in (rad), 75°.
pub const LOCK_CONE: f32 = 1.309;
/// How much a yard of distance weighs against a radian off-centre when picking.
const DISTANCE_WEIGHT: f32 = 0.5 / LOCK_RANGE;

/// Something the player can lock onto; `aim_height` is the point above its feet the camera and
/// the marker hold (yd), `height` its body's height: the camera rig is picked by it
/// ([`crate::camera::lock_rig`]).
#[derive(Component, Debug, Clone, Copy)]
pub struct Lockable {
    pub aim_height: f32,
    pub height: f32,
}

/// The current lock.
#[derive(Resource, Default, Debug)]
pub struct LockOn {
    pub target: Option<Entity>,
}

/// `WOWSOULS_LOCK=<s>`: press the lock button once at `s` seconds. An unattended-run instrument.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ScriptedLock(pub f32);

/// `WOWSOULS_LOCK_SWITCH=<s>:<left|right>,...`: flick the lock to the next target on that side at
/// `s` seconds. An unattended-run instrument.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ScriptedSwitches(pub Vec<(f32, Side)>);

/// Parses `WOWSOULS_LOCK_SWITCH`.
pub fn parse_switches(v: &str) -> Option<ScriptedSwitches> {
    let list: Vec<(f32, Side)> = v
        .split(',')
        .map(|part| {
            let (at, side) = part.split_once(':')?;
            let at = at.trim().parse::<f32>().ok().filter(|t| t.is_finite())?;
            let side = match side.trim() {
                "left" | "l" => Side::Left,
                "right" | "r" => Side::Right,
                _ => return None,
            };
            Some((at, side))
        })
        .collect::<Option<_>>()?;
    (!list.is_empty()).then_some(ScriptedSwitches(list))
}

pub fn plugin(app: &mut App) {
    check::plugin(app);
    app.init_resource::<LockOn>()
        .add_systems(
            Update,
            (toggle, switch, keep).chain().in_set(PlaySet::Intent),
        )
        .add_systems(
            Update,
            draw_marker
                .after(PlaySet::Camera)
                .run_if(in_state(Phase::Playing))
                // The Dark Souls reticle (`hud::lock`) replaces the dot when its art is there.
                .run_if(not(resource_exists::<crate::hud::HudArt>))
                .run_if(not(crate::hud::theme::enabled)),
        );
    if let Some(at) = std::env::var("WOWSOULS_LOCK")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
    {
        app.insert_resource(ScriptedLock(at));
    }
    if let Some(list) = std::env::var("WOWSOULS_LOCK_SWITCH")
        .ok()
        .and_then(|v| parse_switches(&v))
    {
        app.insert_resource(list);
    }
    if crate::run_mode::unattended() {
        if let Some(script) = std::env::var("WOWSOULS_LOCK_INPUT")
            .ok()
            .and_then(|s| InputScript::parse(&s))
        {
            app.insert_resource(script);
        }
    }
}

/// Which way a target switch goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// How far round (rad) from the current target's bearing a switch may reach: 100 degrees.
pub const SWITCH_CONE: f32 = 1.75;
/// A target closer than this to the current one's bearing (rad) is the same side of nothing.
const SWITCH_MIN: f32 = 0.05;

/// The target to move the lock to: of `candidates` within [`LOCK_RANGE`] of `from`, the one nearest
/// the current target's bearing on the `side` flicked (the angle round the player, as seen from
/// above, from the current target to it; distance breaks a near-tie), up to [`SWITCH_CONE`]
/// away. `None` when nothing stands on that side: the lock stays where it is.
pub fn next_target(
    from: Vec3,
    current: (Entity, Vec3),
    candidates: impl IntoIterator<Item = (Entity, Vec3)>,
    side: Side,
) -> Option<Entity> {
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let facing = flat(current.1 - from).normalize_or_zero();
    if facing == Vec3::ZERO {
        return None;
    }
    // Seen from above with -Z ahead, the right of a bearing is `bearing x up`.
    let right = facing.cross(Vec3::Y);
    candidates
        .into_iter()
        .filter(|(e, _)| *e != current.0)
        .filter_map(|(e, at)| {
            let d = flat(at - from);
            let dist = d.length();
            if dist > LOCK_RANGE || dist < 1.0e-3 {
                return None;
            }
            let turn = d.dot(right).atan2(d.dot(facing));
            let turn = if side == Side::Right { turn } else { -turn };
            (SWITCH_MIN..=SWITCH_CONE)
                .contains(&turn)
                .then_some((turn + dist * DISTANCE_WEIGHT, e))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, e)| e)
}

/// How well a target at `to` suits a lock from `from` under a camera looking along `cam_fwd`:
/// lower is better, `None` out of range or outside the cone.
pub fn score(cam_fwd: Vec3, from: Vec3, to: Vec3) -> Option<f32> {
    let d = to - from;
    let flat = Vec3::new(d.x, 0.0, d.z);
    let dist = flat.length();
    if dist > LOCK_RANGE {
        return None;
    }
    let fwd = Vec3::new(cam_fwd.x, 0.0, cam_fwd.z).normalize_or_zero();
    let off = if dist < 1.0e-3 {
        0.0
    } else {
        fwd.angle_between(flat)
    };
    (off <= LOCK_CONE).then_some(off + dist * DISTANCE_WEIGHT)
}

/// The best of `candidates` for a lock from `from`.
pub fn pick(
    cam_fwd: Vec3,
    from: Vec3,
    candidates: impl IntoIterator<Item = (Entity, Vec3)>,
) -> Option<Entity> {
    candidates
        .into_iter()
        .filter_map(|(e, at)| score(cam_fwd, from, at).map(|s| (s, e)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, e)| e)
}

/// The steepest the locked camera looks down (rad): -55 degrees, the lowest `rotRangeMinX` in
/// DSR's `LockCamParam` (row 1103). The rig's own floor (-40 for a human-sized target) is the
/// natural look; this is how far it may go to keep a close, small target in sight.
pub const LOCK_PITCH_FLOOR: f32 = -55.0 * std::f32::consts::PI / 180.0;
/// Extra room (yd) between the line of sight and the player's body.
const SIGHT_MARGIN: f32 = 0.15;
/// The look-down search's step (rad): one degree.
const PITCH_STEP: f32 = std::f32::consts::PI / 180.0;

/// The distance between two 2D segments (0 when they cross).
fn segment_distance(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> f32 {
    fn side(p: Vec2, q: Vec2, r: Vec2) -> f32 {
        (q - p).perp_dot(r - p)
    }
    if side(a0, a1, b0) * side(a0, a1, b1) < 0.0 && side(b0, b1, a0) * side(b0, b1, a1) < 0.0 {
        return 0.0;
    }
    fn point_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
        let ab = b - a;
        let t = if ab.length_squared() > 0.0 {
            ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        p.distance(a + ab * t)
    }
    point_segment(a0, b0, b1)
        .min(point_segment(a1, b0, b1))
        .min(point_segment(b0, a0, a1))
        .min(point_segment(b1, a0, a1))
}

/// Whether a camera looking down `pitch` (negative is down) at the rig's boom length sees the aim
/// point, `flat` yd across and `rise` yd above the pivot, past the player's own body: the
/// capsule from the feet to the head, seen from the side along the line to the target.
pub fn sightline_clear(rig: &Rig, flat: f32, rise: f32, pitch: f32) -> bool {
    let r = crate::motor::CAPSULE_RADIUS;
    let eye = Vec2::new(-rig.distance * pitch.cos(), -rig.distance * pitch.sin());
    let aim = Vec2::new(flat, rise);
    let feet = Vec2::new(0.0, r - rig.pivot_height);
    let head = Vec2::new(0.0, crate::avatar::HEIGHT - r - rig.pivot_height);
    segment_distance(eye, aim, feet, head) > r + SIGHT_MARGIN
}

/// The first pitch from `start` downwards (one degree at a time) at which the aim point is in
/// sight, or the floor [`LOCK_PITCH_FLOOR`] when none is: a close target needs the camera high
/// above the player, a far one just over the head.
pub fn clear_pitch(rig: &Rig, flat: f32, rise: f32, start: f32) -> f32 {
    let mut pitch = start.min(crate::camera::PITCH_MAX);
    while pitch > LOCK_PITCH_FLOOR {
        if sightline_clear(rig, flat, rise, pitch) {
            return pitch;
        }
        pitch -= PITCH_STEP;
    }
    LOCK_PITCH_FLOOR
}

/// The camera yaw and pitch that hold `aim` in view from `pivot`: the yaw faces it. The pitch
/// starts at the rig's `lockRotXShiftRatio` of the target's angle (floor: the rig's
/// `rotRangeMinX`) and goes down until the target is not behind the player's body.
pub fn lock_angles(pivot: Vec3, aim: Vec3, rig: &Rig) -> (f32, f32) {
    let d = aim - pivot;
    let flat = Vec3::new(d.x, 0.0, d.z);
    let across = flat.length();
    let yaw = yaw_of(flat.normalize_or(Vec3::NEG_Z));
    let natural = (d.y.atan2(across.max(0.5)) * rig.shift_ratio)
        .clamp(rig.pitch_min, crate::camera::PITCH_MAX);
    let pitch = clear_pitch(rig, across, d.y, natural);
    (yaw, pitch.clamp(LOCK_PITCH_FLOOR, crate::camera::PITCH_MAX))
}

/// The lock button: let go, lock the best target, or recentre.
#[allow(clippy::too_many_arguments)]
fn toggle(
    time: Res<Time<bevy::time::Real>>,
    controls: Controls,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    scripted: Option<Res<ScriptedLock>>,
    mut fired: Local<bool>,
    avatar: Query<&Transform, With<Avatar>>,
    targets: Query<(Entity, &GlobalTransform, &Lockable)>,
    mut cam: Query<(&Transform, &mut OrbitCam), Without<Avatar>>,
    mut lock: ResMut<LockOn>,
    screen: Res<crate::loot::Screen>,
    menu: Res<crate::settings::MenuUp>,
    mode: Res<crate::camera::CamMode>,
    world: Option<benilla_world::collision::WorldCollision>,
) {
    if screen.blocks_actions(time.elapsed_secs())
        || menu.0
        || *mode != crate::camera::CamMode::Orbit
    {
        return;
    }
    // The Lock On binding: Q, the middle mouse button or a right-stick click by default.
    let mut pressed = unattended.is_none() && controls.just_pressed(Action::LockOn);
    if let Some(at) = scripted.as_deref() {
        if !*fired && time.elapsed_secs() >= at.0 {
            *fired = true;
            pressed = true;
        }
    }
    if !pressed {
        return;
    }
    if lock.target.take().is_some() {
        info!("lock-on: released");
        return;
    }
    let (Ok(body), Ok((cam_xf, mut orbit))) = (avatar.single(), cam.single_mut()) else {
        return;
    };
    // Only what the player can see, on about the player's level.
    let candidates = targets
        .iter()
        .filter(|(_, g, l)| {
            lockable_from(
                world.as_ref(),
                body.translation,
                g.translation(),
                l.aim_height,
            )
        })
        .map(|(e, g, _)| (e, g.translation()));
    match pick(*cam_xf.forward(), body.translation, candidates) {
        Some(e) => {
            info!("lock-on: locked {e}");
            lock.target = Some(e);
            orbit.recenter = None;
        }
        None => orbit.recenter = Some(body.rotation.to_euler(EulerRot::YXZ).0),
    }
}

/// Moves the lock to the next target on the side the look input is flicked.
#[allow(clippy::too_many_arguments)]
fn switch(
    time: Res<Time<bevy::time::Real>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    controls: Controls,
    gamepads: Query<&Gamepad>,
    window: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    scripted: Option<Res<ScriptedSwitches>>,
    input_script: Option<Res<InputScript>>,
    mut done: Local<usize>,
    mut flick: Local<Flick>,
    avatar: Query<&Transform, With<Avatar>>,
    targets: Query<(Entity, &GlobalTransform, &Lockable)>,
    mut lock: ResMut<LockOn>,
    (screen, menu, mode): (
        Res<crate::loot::Screen>,
        Res<crate::settings::MenuUp>,
        Res<crate::camera::CamMode>,
    ),
    world: Option<benilla_world::collision::WorldCollision>,
) {
    let Some(current) = lock.target else {
        flick.rest();
        return;
    };
    if screen.blocks_actions(time.elapsed_secs())
        || menu.0
        || *mode != crate::camera::CamMode::Orbit
    {
        flick.rest();
        return;
    }
    let dt = time.delta_secs();
    let mut side = None;
    if let Some(script) = input_script.as_deref() {
        let (stick, mouse) = script.sample(time.elapsed_secs(), dt);
        side = flick.step(stick, mouse, dt);
    } else if unattended.is_none() {
        let stick = controls
            .read(|binds, held| crate::camera::look_stick(binds, held, &gamepads))
            .x;
        let captured = window
            .single()
            .is_ok_and(|(w, c)| w.focused && c.grab_mode == CursorGrabMode::Locked);
        if captured {
            side = flick.step(stick, motion.delta, dt);
        } else {
            flick.rest();
        }
    }
    if let Some(list) = scripted.as_deref() {
        if let Some(&(at, s)) = list.0.get(*done) {
            if time.elapsed_secs() >= at {
                *done += 1;
                side = Some(s);
            }
        }
    }
    let Some(side) = side else { return };
    let (Ok(body), Ok((_, here, _))) = (avatar.single(), targets.get(current)) else {
        return;
    };
    let candidates = targets
        .iter()
        .filter(|(_, g, l)| {
            lockable_from(
                world.as_ref(),
                body.translation,
                g.translation(),
                l.aim_height,
            )
        })
        .map(|(e, g, _)| (e, g.translation()));
    match next_target(
        body.translation,
        (current, here.translation()),
        candidates,
        side,
    ) {
        Some(e) => {
            info!("lock-on: switched {current} -> {e} ({side:?})");
            lock.target = Some(e);
        }
        None => info!("lock-on: nothing to the {side:?} of {current}"),
    }
}

/// Breaks a lock whose target is gone, out of reach, off the player's level or out of sight for
/// [`LOST_SIGHT_SECS`].
fn keep(
    time: Res<Time>,
    avatar: Query<&Transform, With<Avatar>>,
    targets: Query<(&GlobalTransform, &Lockable)>,
    mut lock: ResMut<LockOn>,
    world: Option<benilla_world::collision::WorldCollision>,
    mut unseen: Local<f32>,
) {
    let Some(e) = lock.target else {
        *unseen = 0.0;
        return;
    };
    let (Ok(body), Ok((t, l))) = (avatar.single(), targets.get(e)) else {
        lock.target = None;
        info!("lock-on: target gone");
        return;
    };
    let at = t.translation();
    if body.translation.distance(at) > LOCK_BREAK {
        lock.target = None;
        info!("lock-on: target out of reach");
        return;
    }
    if (at.y - body.translation.y).abs() > LOCK_RISE {
        lock.target = None;
        info!("lock-on: target off the player's level");
        return;
    }
    if seen(
        world.as_ref(),
        body.translation + Vec3::Y * EYE,
        at + Vec3::Y * l.aim_height,
    ) {
        *unseen = 0.0;
    } else {
        *unseen += time.delta_secs();
        if *unseen > LOST_SIGHT_SECS {
            lock.target = None;
            *unseen = 0.0;
            info!("lock-on: target out of sight");
        }
    }
}

/// The aim point of the locked target, Bevy space.
pub fn locked_aim(lock: &LockOn, targets: &Query<(&GlobalTransform, &Lockable)>) -> Option<Vec3> {
    let (g, l) = targets.get(lock.target?).ok()?;
    Some(g.translation() + Vec3::Y * l.aim_height)
}

/// The body height of the locked target (yd), which picks the camera rig.
pub fn locked_height(lock: &LockOn, targets: &Query<(&GlobalTransform, &Lockable)>) -> Option<f32> {
    let (_, l) = targets.get(lock.target?).ok()?;
    Some(l.height)
}

/// The Souls lock dot: a small marker on the target's aim point.
fn draw_marker(
    lock: Res<LockOn>,
    targets: Query<(&GlobalTransform, &Lockable)>,
    mut gizmos: Gizmos,
) {
    if let Some(aim) = locked_aim(&lock, &targets) {
        gizmos.sphere(
            Isometry3d::from_translation(aim),
            0.14,
            Color::srgb(1.0, 0.95, 0.8),
        );
    }
}

/// The yaw step that turns a locked view toward its target, frame-rate independent.
pub fn track(current: f32, wanted: f32, rate: f32, dt: f32) -> f32 {
    wrap_pi(current + wrap_pi(wanted - current) * (1.0 - (-rate * dt).exp()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::FREE_RIG;

    const FWD: Vec3 = Vec3::NEG_Z;

    fn e(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn out_of_range_or_behind_is_never_picked() {
        assert_eq!(score(FWD, Vec3::ZERO, Vec3::new(0.0, 0.0, -25.0)), None);
        assert_eq!(score(FWD, Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0)), None);
        assert!(score(FWD, Vec3::ZERO, Vec3::new(0.0, 0.0, -5.0)).is_some());
    }

    #[test]
    fn the_target_nearest_the_view_centre_wins() {
        let ahead = (e(1), Vec3::new(0.0, 0.0, -15.0));
        let aside = (e(2), Vec3::new(-6.0, 0.0, -6.0));
        assert_eq!(pick(FWD, Vec3::ZERO, [aside, ahead]), Some(e(1)));
    }

    #[test]
    fn distance_breaks_a_near_tie() {
        let near = (e(1), Vec3::new(0.1, 0.0, -4.0));
        let far = (e(2), Vec3::new(0.1, 0.0, -18.0));
        assert_eq!(pick(FWD, Vec3::ZERO, [far, near]), Some(e(1)));
    }

    #[test]
    fn nothing_in_reach_picks_nothing() {
        assert_eq!(pick(FWD, Vec3::ZERO, []), None);
    }

    #[test]
    fn the_locked_view_faces_the_target() {
        let rig = FREE_RIG;
        let (yaw, _) = lock_angles(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), &rig);
        // Facing +X is a quarter turn right of -Z.
        assert!(
            (wrap_pi(yaw + std::f32::consts::FRAC_PI_2)).abs() < 1.0e-5,
            "{yaw}"
        );
    }

    /// A target `flat` yd away whose aim point is `aim` yd above the feet, as the camera's
    /// `(flat, rise above the pivot)`.
    fn target(flat: f32, aim: f32) -> (f32, f32) {
        (flat, aim - FREE_RIG.pivot_height)
    }

    #[test]
    fn a_level_camera_hides_a_far_target_behind_the_player() {
        // The bug the user hit: the camera at the pivot looks through the player's body.
        let (flat, rise) = target(10.0, 0.8);
        assert!(!sightline_clear(&FREE_RIG, flat, rise, 0.0));
    }

    #[test]
    fn a_far_target_is_seen_over_the_players_head() {
        let (flat, rise) = target(10.0, 0.8);
        let pitch = clear_pitch(&FREE_RIG, flat, rise, 0.0);
        assert!(sightline_clear(&FREE_RIG, flat, rise, pitch), "{pitch}");
        assert!(
            (-0.6..-0.1).contains(&pitch),
            "a modest look-down, {pitch} rad"
        );
    }

    #[test]
    fn a_close_small_target_needs_a_steep_look_down() {
        // A kobold at melee range: its chest is 0.7 yd up, 1.8 yd away.
        let (flat, rise) = target(1.8, 0.7);
        let pitch = clear_pitch(&FREE_RIG, flat, rise, 0.0);
        assert!(pitch <= -0.7, "{pitch}");
        assert!(sightline_clear(&FREE_RIG, flat, rise, pitch), "{pitch}");
    }

    #[test]
    fn the_closer_the_target_the_steeper_the_view() {
        let pitch = |flat: f32| {
            let (f, r) = target(flat, 0.7);
            clear_pitch(&FREE_RIG, f, r, 0.0)
        };
        let seen: Vec<f32> = [14.0, 8.0, 4.0, 2.5, 1.8].map(pitch).to_vec();
        assert!(seen.windows(2).all(|w| w[1] <= w[0] + 1.0e-6), "{seen:?}");
    }

    #[test]
    fn every_target_is_in_sight_unless_the_floor_binds() {
        for flat in [1.2_f32, 1.8, 2.5, 4.0, 7.0, 12.0, 19.0] {
            for aim in [0.4_f32, 0.7, 1.0, 1.4, 2.4] {
                let (f, r) = target(flat, aim);
                let pitch = clear_pitch(&FREE_RIG, f, r, 0.0);
                assert!(
                    sightline_clear(&FREE_RIG, f, r, pitch)
                        || (pitch - LOCK_PITCH_FLOOR).abs() < 1.0e-6,
                    "flat {flat} aim {aim}: pitch {pitch}"
                );
                assert!(pitch >= LOCK_PITCH_FLOOR - 1.0e-6);
            }
        }
    }

    #[test]
    fn the_locked_pitch_stays_inside_the_cameras_limits() {
        let rig = FREE_RIG;
        // Far below: the look-down stops at the floor.
        let (_, down) = lock_angles(Vec3::ZERO, Vec3::new(1.0, -30.0, 0.0), &rig);
        assert!(down >= LOCK_PITCH_FLOOR - 1.0e-6, "{down}");
        // Far above: raised only to the camera's limit.
        let (_, high) = lock_angles(Vec3::ZERO, Vec3::new(0.0, 50.0, -1.0), &rig);
        assert!(high <= crate::camera::PITCH_MAX + 1.0e-6);
        // A same-height target 10 yd off is seen from above the head, not level.
        let (_, level) = lock_angles(
            Vec3::new(0.0, rig.pivot_height, 0.0),
            Vec3::new(10.0, 0.8, 0.0),
            &rig,
        );
        assert!(level < -0.1, "{level}");
    }

    #[test]
    fn tracking_converges_the_short_way() {
        let mut yaw = 3.0;
        for _ in 0..60 {
            yaw = track(yaw, -3.0, 10.0, 1.0 / 60.0);
        }
        assert!(wrap_pi(yaw + 3.0).abs() < 0.01, "{yaw}");
        // The first step goes positive, across π.
        assert!(wrap_pi(track(3.0, -3.0, 10.0, 1.0 / 60.0) - 3.0) > 0.0);
    }

    #[test]
    fn a_switch_goes_to_the_nearest_target_on_the_flicked_side() {
        // The player at the origin, the lock on a target straight ahead (-Z).
        let cur = (e(1), Vec3::new(0.0, 0.0, -8.0));
        let right_near = (e(2), Vec3::new(4.0, 0.0, -8.0));
        let right_far = (e(3), Vec3::new(9.0, 0.0, -2.0));
        let left = (e(4), Vec3::new(-3.0, 0.0, -8.0));
        let all = [cur, right_near, right_far, left];
        assert_eq!(next_target(Vec3::ZERO, cur, all, Side::Right), Some(e(2)));
        assert_eq!(next_target(Vec3::ZERO, cur, all, Side::Left), Some(e(4)));
        // From the one on the right, left goes back to where it came from (the lock itself is
        // never its own next), right goes on.
        assert_eq!(
            next_target(Vec3::ZERO, right_near, all, Side::Left),
            Some(e(1))
        );
        assert_eq!(
            next_target(Vec3::ZERO, right_near, all, Side::Right),
            Some(e(3))
        );
    }

    #[test]
    fn a_switch_with_nothing_on_that_side_stays_put() {
        let cur = (e(1), Vec3::new(0.0, 0.0, -8.0));
        let only_left = (e(2), Vec3::new(-4.0, 0.0, -8.0));
        assert_eq!(
            next_target(Vec3::ZERO, cur, [cur, only_left], Side::Right),
            None
        );
        // Out of range, and past the cone, are not candidates.
        let far = (e(3), Vec3::new(30.0, 0.0, -8.0));
        let behind = (e(4), Vec3::new(1.0, 0.0, 8.0));
        assert_eq!(
            next_target(Vec3::ZERO, cur, [cur, far, behind], Side::Right),
            None
        );
        assert_eq!(next_target(Vec3::ZERO, cur, [cur], Side::Left), None);
    }

    #[test]
    fn a_near_tie_on_the_same_bearing_prefers_the_nearer() {
        let cur = (e(1), Vec3::new(0.0, 0.0, -10.0));
        let near = (e(2), Vec3::new(3.0, 0.0, -9.0));
        let far = (e(3), Vec3::new(4.0, 0.0, -12.0));
        assert_eq!(
            next_target(Vec3::ZERO, cur, [far, near, cur], Side::Right),
            Some(e(2))
        );
    }

    #[test]
    fn a_stick_flick_fires_once_per_push_and_rearms_at_rest() {
        let mut f = Flick::default();
        let dt = 1.0 / 60.0;
        // The stick is at rest after a lock: nothing fires; the first frames arm it.
        for _ in 0..30 {
            assert_eq!(f.step(0.0, Vec2::ZERO, dt), None);
        }
        // A push right fires once, however long it is held.
        assert_eq!(f.step(0.9, Vec2::ZERO, dt), Some(Side::Right));
        for _ in 0..60 {
            assert_eq!(f.step(0.9, Vec2::ZERO, dt), None);
        }
        // Back to rest, then left.
        for _ in 0..30 {
            f.step(0.0, Vec2::ZERO, dt);
        }
        assert_eq!(f.step(-0.8, Vec2::ZERO, dt), Some(Side::Left));
        // Half a push never fires.
        for _ in 0..30 {
            f.step(0.0, Vec2::ZERO, dt);
        }
        for _ in 0..30 {
            assert_eq!(f.step(0.4, Vec2::ZERO, dt), None);
        }
    }

    #[test]
    fn a_stick_held_over_when_the_lock_is_made_waits_for_rest() {
        let mut f = Flick::default();
        let dt = 1.0 / 60.0;
        for _ in 0..30 {
            assert_eq!(
                f.step(0.9, Vec2::ZERO, dt),
                None,
                "a deflected stick at the lock"
            );
        }
        for _ in 0..20 {
            f.step(0.0, Vec2::ZERO, dt);
        }
        assert_eq!(f.step(0.9, Vec2::ZERO, dt), Some(Side::Right));
        // Lock let go: the next lock starts over.
        f.rest();
        assert_eq!(f.step(0.9, Vec2::ZERO, dt), None);
    }

    #[test]
    fn a_quick_mouse_flick_fires_and_a_slow_drift_does_not() {
        let dt = 1.0 / 60.0;
        let mut quick = Flick::default();
        for _ in 0..30 {
            quick.step(0.0, Vec2::ZERO, dt);
        }
        // 160 raw pixels over 8 frames: a clear horizontal swipe.
        let mut fired = None;
        for _ in 0..8 {
            fired = fired.or(quick.step(0.0, Vec2::new(20.0, 0.0), dt));
        }
        assert_eq!(fired, Some(Side::Right));
        let mut slow = Flick::default();
        for _ in 0..30 {
            slow.step(0.0, Vec2::ZERO, dt);
        }
        // 90 px a second, steadily: well under a flick.
        for _ in 0..240 {
            assert_eq!(slow.step(0.0, Vec2::new(-1.5, 0.0), dt), None);
        }
    }

    #[test]
    fn ordinary_mouse_motion_does_not_switch_a_locked_target() {
        let dt = 1.0 / 60.0;
        let mut flick = Flick::default();
        for _ in 0..30 {
            flick.step(0.0, Vec2::ZERO, dt);
        }
        // An ordinary 40 px adjustment over 100 ms is not a deliberate target swipe.
        for _ in 0..6 {
            assert_eq!(flick.step(0.0, Vec2::new(40.0 / 6.0, 0.0), dt), None);
        }
    }

    fn input_app(sensitivity: f32) -> (App, [Entity; 3]) {
        let mut app = App::new();
        let mut settings = crate::settings::Settings::default();
        settings.game.mouse = sensitivity;
        app.insert_resource(settings)
            .init_resource::<Time<bevy::time::Real>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .init_resource::<crate::settings::MenuUp>()
            .init_resource::<crate::loot::Screen>()
            .init_resource::<crate::camera::CamMode>()
            .add_systems(Update, switch);
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            CursorOptions {
                grab_mode: CursorGrabMode::Locked,
                ..default()
            },
            PrimaryWindow,
        ));
        app.world_mut().spawn((Avatar, Transform::default()));
        let targets = [
            Vec3::new(0.0, 0.0, -8.0),
            Vec3::new(4.0, 0.0, -8.0),
            Vec3::new(9.0, 0.0, -4.0),
        ]
        .map(|at| {
            app.world_mut()
                .spawn((
                    GlobalTransform::from_translation(at),
                    Lockable {
                        aim_height: 1.0,
                        height: 1.8,
                    },
                ))
                .id()
        });
        app.insert_resource(LockOn {
            target: Some(targets[0]),
        });
        (app, targets)
    }
    fn motion(app: &mut App, delta: Vec2, frames: usize) {
        for _ in 0..frames {
            app.world_mut()
                .resource_mut::<Time<bevy::time::Real>>()
                .advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            app.world_mut()
                .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
                .delta = delta;
            app.update();
        }
    }

    #[test]
    fn actual_mouse_input_is_independent_of_camera_sensitivity_and_only_switches_once() {
        for sensitivity in [0.5, 1.0, 3.0] {
            let (mut app, targets) = input_app(sensitivity);
            motion(&mut app, Vec2::ZERO, 30);
            motion(&mut app, Vec2::new(40.0 / 6.0, 0.0), 6);
            assert_eq!(app.world().resource::<LockOn>().target, Some(targets[0]));
            motion(&mut app, Vec2::ZERO, 30);
            motion(&mut app, Vec2::new(20.0, 0.0), 90);
            assert_eq!(app.world().resource::<LockOn>().target, Some(targets[1]));
        }
    }

    #[test]
    fn menu_dialogue_cursor_and_focus_loss_discard_gestures_before_resuming() {
        for blocked in ["menu", "dialogue", "cursor", "focus", "freecam"] {
            let (mut app, targets) = input_app(1.0);
            motion(&mut app, Vec2::ZERO, 30);
            match blocked {
                "menu" => app.world_mut().resource_mut::<crate::settings::MenuUp>().0 = true,
                "dialogue" => app.world_mut().resource_mut::<crate::loot::Screen>().modal = true,
                "freecam" => {
                    *app.world_mut().resource_mut::<crate::camera::CamMode>() =
                        crate::camera::CamMode::Free
                }
                "cursor" => {
                    let mut q = app.world_mut().query::<&mut CursorOptions>();
                    q.single_mut(app.world_mut()).unwrap().grab_mode = CursorGrabMode::None;
                }
                "focus" => {
                    let mut q = app.world_mut().query::<&mut Window>();
                    q.single_mut(app.world_mut()).unwrap().focused = false;
                }
                _ => unreachable!(),
            }
            motion(&mut app, Vec2::new(40.0, 0.0), 15);
            assert_eq!(
                app.world().resource::<LockOn>().target,
                Some(targets[0]),
                "{blocked}"
            );
            app.world_mut().resource_mut::<crate::settings::MenuUp>().0 = false;
            app.world_mut().resource_mut::<crate::loot::Screen>().modal = false;
            *app.world_mut().resource_mut::<crate::camera::CamMode>() =
                crate::camera::CamMode::Orbit;
            let mut q = app.world_mut().query::<(&mut Window, &mut CursorOptions)>();
            let (mut window, mut cursor) = q.single_mut(app.world_mut()).unwrap();
            window.focused = true;
            cursor.grab_mode = CursorGrabMode::Locked;
            motion(&mut app, Vec2::new(200.0, 0.0), 1);
            assert_eq!(
                app.world().resource::<LockOn>().target,
                Some(targets[0]),
                "stale {blocked}"
            );
            motion(&mut app, Vec2::ZERO, 30);
            motion(&mut app, Vec2::new(20.0, 0.0), 8);
            assert_eq!(
                app.world().resource::<LockOn>().target,
                Some(targets[1]),
                "fresh {blocked}"
            );
        }
    }

    #[test]
    fn the_scripted_switches_parse() {
        assert_eq!(
            parse_switches("6:right, 8.5:left"),
            Some(ScriptedSwitches(vec![
                (6.0, Side::Right),
                (8.5, Side::Left)
            ]))
        );
        assert_eq!(parse_switches("6:up"), None);
        assert_eq!(parse_switches(""), None);
        assert_eq!(parse_switches("x:right"), None);
    }

    fn lock_for(
        key: Option<KeyCode>,
        mouse: Option<MouseButton>,
        pad: Option<GamepadButton>,
    ) -> bool {
        use crate::pad::testkit::{read, world_with_pad};
        let mut keys = ButtonInput::<KeyCode>::default();
        if let Some(k) = key {
            keys.press(k);
        }
        let mut buttons = ButtonInput::<MouseButton>::default();
        if let Some(b) = mouse {
            buttons.press(b);
        }
        let mut world = match pad {
            Some(b) => world_with_pad(move |p| {
                p.digital_mut().press(b);
            }),
            None => World::new(),
        };
        read(&mut world, move |pads| {
            Binds::default().just_pressed_in(Action::LockOn, &keys, &buttons, pads, true)
        })
    }

    #[test]
    fn the_lock_button_is_q_the_middle_mouse_or_a_right_stick_click() {
        assert!(lock_for(Some(KeyCode::KeyQ), None, None));
        assert!(lock_for(None, Some(MouseButton::Middle), None));
        assert!(lock_for(None, None, Some(GamepadButton::RightThumb)));
        assert!(!lock_for(None, None, Some(GamepadButton::LeftThumb)));
        assert!(!lock_for(
            Some(KeyCode::KeyW),
            Some(MouseButton::Left),
            None
        ));
        assert!(!lock_for(None, None, None));
    }
}
