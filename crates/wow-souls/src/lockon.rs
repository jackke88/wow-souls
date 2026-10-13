//! Lock-on, Souls-style. One button (Q, middle mouse, right-stick click) locks the best
//! [`Lockable`] in front of the camera, or with none in reach recentres the camera behind the
//! body; pressed while locked it lets go. Locked, the camera holds the target in view, the body
//! faces it and strafes, and the lock breaks past [`LOCK_BREAK`].
//!
//! The best target is the one nearest the view centre, distance breaking near-ties; a target
//! behind the camera's [`LOCK_CONE`] is never picked.

use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::camera::{OrbitCam, Rig};
use crate::controller::{wrap_pi, yaw_of, PlaySet};
use crate::settings::binds::Action;
#[cfg(test)]
use crate::settings::binds::Binds;
use crate::settings::Controls;
use crate::settle::Phase;

/// How far a target may be to lock on (yd).
pub const LOCK_RANGE: f32 = 20.0;
/// How far a locked target may go before the lock breaks (yd).
pub const LOCK_BREAK: f32 = 30.0;
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

pub fn plugin(app: &mut App) {
    app.init_resource::<LockOn>()
        .add_systems(Update, (toggle, keep).chain().in_set(PlaySet::Intent))
        .add_systems(
            Update,
            draw_marker
                .after(PlaySet::Camera)
                .run_if(in_state(Phase::Playing))
                // The Dark Souls reticle (`hud::lock`) replaces the dot when its art is there.
                .run_if(not(resource_exists::<crate::hud::HudArt>)),
        );
    if let Some(at) = std::env::var("WOWSOULS_LOCK")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
    {
        app.insert_resource(ScriptedLock(at));
    }
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
) {
    if screen.locked() {
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
    let candidates = targets.iter().map(|(e, g, _)| (e, g.translation()));
    match pick(*cam_xf.forward(), body.translation, candidates) {
        Some(e) => {
            info!("lock-on: locked {e}");
            lock.target = Some(e);
            orbit.recenter = None;
        }
        None => orbit.recenter = Some(body.rotation.to_euler(EulerRot::YXZ).0),
    }
}

/// Breaks a lock whose target is gone or out of reach.
fn keep(
    avatar: Query<&Transform, With<Avatar>>,
    targets: Query<&GlobalTransform, With<Lockable>>,
    mut lock: ResMut<LockOn>,
) {
    let Some(e) = lock.target else { return };
    let (Ok(body), Ok(t)) = (avatar.single(), targets.get(e)) else {
        lock.target = None;
        info!("lock-on: target gone");
        return;
    };
    if body.translation.distance(t.translation()) > LOCK_BREAK {
        lock.target = None;
        info!("lock-on: target out of reach");
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
