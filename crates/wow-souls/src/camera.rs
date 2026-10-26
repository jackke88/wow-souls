//! The third-person camera, Souls-style: mouse or right stick orbit a smoothed pivot at the
//! avatar's shoulders. The boom is swept against the world, so walls, ceilings and the ground
//! pull the camera in, and it eases back out once clear. A recentre swings it behind the body.
//!
//! Collision is the engine's camera query ([`WorldCollision::cast_camera`]): a 0.3 yd sphere
//! against terrain, doodads and each WMO's *camera* collider (which keeps DETAIL faces and drops
//! NOCAMCOLLIDE ones), swept from the head to the seat as benilla's `seat_camera` does
//! (benilla @ 2e82d34, `player/camera.rs`). Snap in on a hit, ease out at [`RETURN_RATE`].
//!
//! Keys: mouse orbit; gamepad: right stick orbit. The lock button ([`crate::lockon`]) recentres
//! when nothing is in reach, and while locked the camera holds the target instead.
//! F1 swaps to the free-fly debug camera ([`crate::freecam`]). Esc frees the cursor and a left
//! click captures it again.

use bevy::camera::Projection;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use benilla_world::collision::WorldCollision;
use benilla_world::model_fade::{self_model_fade_alpha, SELF_FADE_WINDOW};
use benilla_world::view::NEARCLIP_DEFAULT;

use crate::avatar::{Avatar, HEIGHT};
use crate::controller::{deadzone, turn_toward, wrap_pi, PlaySet};
use crate::motor::CAPSULE_RADIUS;
use crate::settings::binds::{Action, Binds, Held};
use crate::settings::Controls;
use crate::settle::Phase;

/// WoW yards per Dark Souls metre at the body's scale: the retarget's, so the camera frames our
/// 2.03 yd human as Dark Souls frames its 1.8 m one.
pub const METRES_TO_YARDS: f32 = 1.135;
/// Vertical field of view (rad): DSR's 43 degrees (`LockCamParam` `camFovY`, 41.5-48 in every row).
pub const FOV_Y: f32 = 43.0 * std::f32::consts::PI / 180.0;

/// One camera rig as Dark Souls Remastered's `LockCamParam` rows define it, in yards and radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rig {
    /// Boom length (yd): `camDistTarget`.
    pub distance: f32,
    /// Pivot height above the feet (yd): `chrOrgOffset_Y`.
    pub pivot_height: f32,
    /// The lowest the camera looks, locked (rad, negative is down): `rotRangeMinX`.
    pub pitch_min: f32,
    /// How much of the angle to the target the locked view follows: `lockRotXShiftRatio`.
    pub shift_ratio: f32,
}

/// A `LockCamParam` row: metres and degrees in, yards and radians out.
const fn row(distance_m: f32, pivot_m: f32, rot_min_deg: f32, shift_ratio: f32) -> Rig {
    Rig {
        distance: distance_m * METRES_TO_YARDS,
        pivot_height: pivot_m * METRES_TO_YARDS,
        pitch_min: rot_min_deg * std::f32::consts::PI / 180.0,
        shift_ratio,
    }
}

/// Row 0, the default: 4.0 m back, 1.42 m up, floor -40 degrees, ratio 0.6. The free camera's rig
/// too: DSR keeps those numbers in its exe, this is the nearest data it ships.
pub const FREE_RIG: Rig = row(4.0, 1.42, -40.0, 0.6);

/// Pivot height above the feet (yd): `chrOrgOffset_Y` 1.42 m.
pub const PIVOT_HEIGHT: f32 = FREE_RIG.pivot_height;
/// Boom length (yd): `camDistTarget` 4.0 m.
pub const DISTANCE: f32 = FREE_RIG.distance;

/// The lock-on rigs by the target's height (DS metres): rows 101 and 102 (small), 0 (human, 1.8 m),
/// 103 and 104 (large and huge). The rows' ids are the file's; which creature uses which is the map's
/// (camera events), so the ladder is a reading by size. Played, the small rows' closer, lower
/// camera hid the enemy behind the player's body ([`lock_rig`] never goes under the free rig).
const LADDER: [(f32, Rig); 5] = [
    (0.6, row(2.2, 1.22, -40.0, 0.6)),
    (1.2, row(3.2, 1.32, -40.0, 0.6)),
    (1.8, FREE_RIG),
    (3.5, row(5.0, 1.60, -20.0, 0.4)),
    (6.0, row(6.0, 1.10, -20.0, 0.4)),
];

/// How fast the rig eases to another target's size (1/s): a size change settles in about a second.
const RIG_RATE: f32 = 3.0;

/// The rig for a locked target `height` yd tall: the ladder, interpolated, held at its ends, and
/// never closer or lower than the free rig: a big target pushes the camera out, nothing pulls it in.
pub fn lock_rig(height: f32) -> Rig {
    let h = height / METRES_TO_YARDS;
    let [first, .., last] = LADDER;
    let rig = if h <= first.0 {
        first.1
    } else {
        LADDER
            .windows(2)
            .find(|pair| h <= pair[1].0)
            .map_or(last.1, |pair| {
                let ((h0, a), (h1, b)) = (pair[0], pair[1]);
                let t = (h - h0) / (h1 - h0);
                Rig {
                    distance: a.distance + (b.distance - a.distance) * t,
                    pivot_height: a.pivot_height + (b.pivot_height - a.pivot_height) * t,
                    pitch_min: a.pitch_min + (b.pitch_min - a.pitch_min) * t,
                    shift_ratio: a.shift_ratio + (b.shift_ratio - a.shift_ratio) * t,
                }
            })
    };
    Rig {
        distance: rig.distance.max(DISTANCE),
        pivot_height: rig.pivot_height.max(PIVOT_HEIGHT),
        ..rig
    }
}

/// The view's rotation: the orbit's own `boom` rotation when free; locked, the camera at `seat`
/// looks `focus` (0..1) of the way from the pivot to the locked `aim`, so the player drops to the
/// lower part of the screen and the enemy comes to its middle, as in Dark Souls.
pub fn view_rotation(seat: Vec3, pivot: Vec3, aim: Vec3, focus: f32, boom: Quat) -> Quat {
    if focus <= 1.0e-3 {
        return boom;
    }
    let at = pivot.lerp(aim, focus.min(1.0));
    Transform::from_translation(seat)
        .looking_at(at, Vec3::Y)
        .rotation
}

/// One frame of the focus easing toward the rig's ratio (locked) or 0 (free), at the rig's rate.
pub fn ease_focus(current: f32, locked_ratio: Option<f32>, dt: f32) -> f32 {
    let want = locked_ratio.unwrap_or(0.0);
    current + (want - current) * (1.0 - (-RIG_RATE * dt).exp())
}

/// One frame of easing `from` toward `to`, frame-rate independent.
pub fn ease_rig(from: Rig, to: Rig, dt: f32) -> Rig {
    let k = 1.0 - (-RIG_RATE * dt).exp();
    let lerp = |a: f32, b: f32| a + (b - a) * k;
    Rig {
        distance: lerp(from.distance, to.distance),
        pivot_height: lerp(from.pivot_height, to.pivot_height),
        pitch_min: lerp(from.pitch_min, to.pitch_min),
        shift_ratio: lerp(from.shift_ratio, to.shift_ratio),
    }
}

/// Sets a perspective projection to [`FOV_Y`]; whether it changed.
pub fn set_fov(projection: &mut Projection) -> bool {
    match projection {
        Projection::Perspective(p) if p.fov != FOV_Y => {
            p.fov = FOV_Y;
            true
        }
        _ => false,
    }
}

/// Pitch limits (rad): down to ~70°, up to ~30°.
pub const PITCH_MIN: f32 = -1.22;
pub const PITCH_MAX: f32 = 0.52;
/// The pitch the camera opens at: a little above the shoulders.
const PITCH_OPEN: f32 = -0.25;
/// Radians per pixel of mouse motion.
pub(crate) const MOUSE_SENSITIVITY: f32 = 0.0025;
/// Right-stick orbit speed at full deflection (rad/s).
const STICK_RATE: f32 = 3.0;
/// How fast the boom grows back after geometry stops cutting it (1/s), benilla's
/// `CAM_RETURN_RATE`.
pub const RETURN_RATE: f32 = 6.0;
/// Pivot follow rates (1/s): near-rigid across the ground, softer in height, so steps, slopes
/// and jumps do not jerk the view and a body jittering in a corner does not shake it.
const FOLLOW_RATE_XZ: f32 = 25.0;
const FOLLOW_RATE_Y: f32 = 9.0;
/// A pivot further than this from the body (yd) snaps instead of easing: a void-out, a respawn.
const FOLLOW_SNAP: f32 = 10.0;
/// The recentre's swing rate (1/s, exponential): a half turn settles in about 0.36 s.
const RECENTER_RATE: f32 = 18.0;
/// The recentre is done inside this yaw gap (rad).
const RECENTER_DONE: f32 = 0.005;
/// Orbit input over this (rad, per frame) cancels a recentre in flight.
const RECENTER_CANCEL: f32 = 0.002;
/// How fast a locked camera swings to hold its target (1/s).
const LOCK_TRACK_RATE: f32 = 10.0;
/// Bound large target changes instead of allowing the exponential's first step to whip the boom.
const LOCK_YAW_SPEED: f32 = std::f32::consts::PI;
const LOCK_PITCH_SPEED: f32 = std::f32::consts::FRAC_PI_2;
const LOCK_VIEW_SPEED: f32 = 240.0_f32.to_radians();
const LOCK_AIM_RATE: f32 = 12.0;

/// Follow a target around the pivot without snapping the focus point across the screen.
pub fn follow_lock_aim(pivot: Vec3, current: Vec3, wanted: Vec3, dt: f32) -> Vec3 {
    let dt = dt.clamp(0.0, 0.1);
    if dt == 0.0 {
        return current;
    }
    let (from, to) = (current - pivot, wanted - pivot);
    let (a, b) = (from.length(), to.length());
    if a < 1.0e-3 || b < 1.0e-3 {
        return current.lerp(wanted, 1.0 - (-LOCK_AIM_RATE * dt).exp());
    }
    let (from, to) = (from / a, to / b);
    let angle = from.angle_between(to);
    let k = 1.0 - (-LOCK_AIM_RATE * dt).exp();
    // Rotation-arc construction rounds tiny angles to identity; finish positionally so the
    // focus does not stop a few millimetres short of a stationary target.
    if angle < 0.005 {
        return current.lerp(wanted, k);
    }
    let turn = k.min(LOCK_VIEW_SPEED * dt / angle);
    let direction = Quat::IDENTITY.slerp(Quat::from_rotation_arc(from, to), turn) * from;
    pivot + direction * (a + (b - a) * k)
}

/// Exponential following with a physical angular-speed bound; yaw takes the shortest arc.
pub fn lock_turn(current: f32, wanted: f32, speed: f32, dt: f32) -> f32 {
    let dt = dt.clamp(0.0, 0.1);
    let eased = crate::lockon::track(current, wanted, LOCK_TRACK_RATE, dt);
    wrap_pi(turn_toward(current, eased, speed * dt))
}

/// The final view also stays continuous near/under a target, where look-at geometry is sensitive.
/// Free orbit is direct. Call with the unshaken previous pose to avoid accumulating hit shake.
pub fn stable_lock_view(current: Quat, wanted: Quat, tracking: bool, dt: f32) -> Quat {
    if !tracking {
        return wanted;
    }
    let angle = current.angle_between(wanted);
    if angle < 1.0e-5 {
        return wanted;
    }
    current.slerp(
        wanted,
        (LOCK_VIEW_SPEED * dt.clamp(0.0, 0.1) / angle).min(1.0),
    )
}

/// Which camera drives the view.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CamMode {
    #[default]
    Orbit,
    Free,
}

/// The orbit rig's state, on the world camera entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct OrbitCam {
    pub yaw: f32,
    pub pitch: f32,
    /// The rig in force (boom length, pivot height, locked pitch floor and ratio), eased.
    pub rig: Rig,
    /// The locked target's aim point (Bevy space), last seen.
    pub look: Vec3,
    /// How far the view aims from the pivot toward `look`: 0 free, the rig's ratio locked, eased.
    pub focus: f32,
    /// Last unshaken view orientation; shake never feeds back into camera following.
    pub view: Quat,
    /// The smoothed pivot (Bevy space).
    pub pivot: Vec3,
    /// The boom length geometry allows, eased (yd from the head).
    pub open: f32,
    /// A recentre in flight: the yaw it swings to.
    pub recenter: Option<f32>,
    /// This frame's sweep hit (yd from the head), `None` when the boom is clear.
    pub hit: Option<f32>,
}

/// The body's opacity as the camera closes on it: 1 normally, 0 with the camera inside the
/// head. The model reads it (step 5); benilla's `self_model_fade_alpha` curve.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct BodyFade(pub f32);

impl Default for BodyFade {
    fn default() -> Self {
        Self(1.0)
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<CamMode>()
        .init_resource::<BodyFade>()
        .add_systems(OnEnter(Phase::Playing), attach_orbit)
        .add_systems(
            Update,
            (toggle_mode, manage_cursor)
                .chain()
                .before(PlaySet::Intent)
                .run_if(in_state(Phase::Playing)),
        )
        .add_systems(
            Update,
            (orbit_input, seat)
                .chain()
                .in_set(PlaySet::Camera)
                .run_if(resource_equals(CamMode::Orbit)),
        );
    if std::env::var_os("WOWSOULS_TRACE").is_some() {
        app.add_systems(
            Update,
            trace_camera
                .after(PlaySet::Camera)
                .run_if(in_state(Phase::Playing)),
        );
    }
}

/// Puts the orbit rig on the world camera behind the body, the moment play starts.
fn attach_orbit(
    mut commands: Commands,
    cam: Query<Entity, With<benilla_world::view::WorldCamera>>,
    avatar: Query<&Transform, With<Avatar>>,
) {
    let (Ok(cam), Ok(body)) = (cam.single(), avatar.single()) else {
        return;
    };
    let yaw = body.rotation.to_euler(EulerRot::YXZ).0;
    commands.entity(cam).insert(OrbitCam {
        yaw,
        pitch: PITCH_OPEN,
        rig: FREE_RIG,
        look: body.translation,
        focus: 0.0,
        view: Quat::from_euler(EulerRot::YXZ, yaw, PITCH_OPEN, 0.0),
        pivot: body.translation + Vec3::Y * PIVOT_HEIGHT,
        open: DISTANCE,
        recenter: None,
        hit: None,
    });
}

fn toggle_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<CamMode>,
    screen: Res<crate::loot::Screen>,
) {
    if screen.locked() {
        return;
    }
    if keys.just_pressed(KeyCode::F1) {
        *mode = match *mode {
            CamMode::Orbit => CamMode::Free,
            CamMode::Free => CamMode::Orbit,
        };
        info!("camera: {:?}", *mode);
    }
}

/// In orbit mode the cursor is captured while the window has focus; a screen (the pack, a dialogue)
/// or the system menu frees it while it is up (Esc opens the menu, [`crate::sysmenu`]). The free camera
/// manages its own (right-drag look).
fn manage_cursor(
    mode: Res<CamMode>,
    mut window: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    screen: Res<crate::loot::Screen>,
    menu: Res<crate::settings::MenuUp>,
) {
    let Ok((window, mut cursor)) = window.single_mut() else {
        return;
    };
    if mode.is_changed() && *mode == CamMode::Free {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        return;
    }
    if *mode != CamMode::Orbit {
        return;
    }
    if screen.locked() || menu.0 {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        return;
    }
    let want = window.focused && unattended.is_none();
    let grab = if want {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    if cursor.grab_mode != grab {
        cursor.grab_mode = grab;
        cursor.visible = !want;
    }
}

/// The look input as a stick (x right, y up, length `0..=1`): the look keys (the arrows by default),
/// or the first pad's right stick past its deadzone, whichever pushes harder.
pub fn look_stick(binds: &Binds, held: &Held, gamepads: &Query<&Gamepad>) -> Vec2 {
    let keyboard = Vec2::new(
        binds.axis(Action::LookLeft, Action::LookRight, held),
        binds.axis(Action::LookDown, Action::LookUp, held),
    )
    .clamp_length_max(1.0);
    let stick = gamepads
        .iter()
        .next()
        .map_or(Vec2::ZERO, |pad| deadzone(pad.right_stick(), 0.15));
    if stick.length() > keyboard.length() {
        stick
    } else {
        keyboard
    }
}

fn orbit_input(
    time: Res<Time>,
    motion: Res<AccumulatedMouseMotion>,
    controls: Controls,
    settings: Res<crate::settings::Settings>,
    gamepads: Query<&Gamepad>,
    window: Query<&CursorOptions, With<PrimaryWindow>>,
    spin: Option<Res<ScriptedSpin>>,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    screen: Res<crate::loot::Screen>,
    lock: Res<crate::lockon::LockOn>,
    targets: Query<(&GlobalTransform, &crate::lockon::Lockable)>,
    mut cam: Query<&mut OrbitCam>,
) {
    if screen.locked() {
        return;
    }
    let Ok(mut cam) = cam.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    // The rig follows the locked target's size, and eases back to the free one when it lets go.
    let wanted = crate::lockon::locked_height(&lock, &targets).map_or(FREE_RIG, lock_rig);
    cam.rig = ease_rig(cam.rig, wanted, dt);
    // The view aims part of the way to the locked target (the rig's shift ratio), and back.
    let locked = crate::lockon::locked_aim(&lock, &targets);
    if let Some(aim) = locked {
        cam.look = if cam.focus <= 1.0e-3 {
            aim
        } else {
            follow_lock_aim(cam.pivot, cam.look, aim, dt)
        };
    }
    cam.focus = ease_focus(cam.focus, locked.map(|_| cam.rig.shift_ratio), dt);
    let captured = window
        .single()
        .is_ok_and(|c| c.grab_mode == CursorGrabMode::Locked);
    // The player's look settings: how fast the mouse and the stick turn the view, and whether up is
    // down (a flipped vertical applies to both, the mouse and the stick).
    let prefs = &settings.game;
    let flip = Vec2::new(1.0, if prefs.invert_y { -1.0 } else { 1.0 });
    let mut d = Vec2::ZERO;
    if captured {
        d += motion.delta * MOUSE_SENSITIVITY * prefs.mouse * flip;
    }
    if unattended.is_some() {
        // Nobody is at the desk: live input is someone else's.
        d = Vec2::ZERO;
    } else {
        // Stick (or look key) up looks up, as the mouse moving up does.
        let s = controls.read(|binds, held| look_stick(binds, held, &gamepads));
        d += Vec2::new(s.x, -s.y) * STICK_RATE * prefs.stick * dt * flip;
    }
    if let Some(spin) = spin.as_deref() {
        d.x -= spin.0 * dt;
    }
    // Locked, the target owns the view: the orbit swings to hold it, and look input is ignored.
    if locked.is_some() {
        let (yaw, pitch) = crate::lockon::lock_angles(cam.pivot, cam.look, &cam.rig);
        cam.yaw = lock_turn(cam.yaw, yaw, LOCK_YAW_SPEED, dt);
        cam.pitch = lock_turn(cam.pitch, pitch, LOCK_PITCH_SPEED, dt);
        cam.recenter = None;
        return;
    }
    if d.length() > RECENTER_CANCEL {
        // The player took the camera back.
        cam.recenter = None;
    }
    cam.yaw = wrap_pi(cam.yaw - d.x);
    cam.pitch = (cam.pitch - d.y).clamp(PITCH_MIN, PITCH_MAX);
    if let Some(target) = cam.recenter {
        let (yaw, done) = recenter_step(cam.yaw, target, dt);
        cam.yaw = yaw;
        if done {
            cam.recenter = None;
        }
    }
}

/// Seats the camera: smooth the pivot, sweep the boom from the head, ease the open length, and
/// publish the body fade.
#[allow(clippy::too_many_arguments)]
fn seat(
    time: Res<Time>,
    collide: WorldCollision,
    lock: Res<crate::lockon::LockOn>,
    avatar: Query<&Transform, (With<Avatar>, Without<OrbitCam>)>,
    mut cam: Query<(&mut Transform, &mut OrbitCam, &mut Projection)>,
    mut fade: ResMut<BodyFade>,
    shake: Res<crate::shake::Shake>,
    observe: Option<Res<Observe>>,
    mut gizmos: Gizmos,
) {
    let (Ok(body), Ok((mut xf, mut orbit, mut projection))) = (avatar.single(), cam.single_mut())
    else {
        return;
    };
    // The view is Dark Souls' own 43 degrees; the engine's spawn FOV is the title's and the free camera's.
    if matches!(&*projection, Projection::Perspective(p) if p.fov != FOV_Y) {
        set_fov(&mut projection);
    }
    let dt = time.delta_secs();
    let wanted_pivot = body.translation + Vec3::Y * orbit.rig.pivot_height;
    let snapped = orbit.pivot.distance(wanted_pivot) > FOLLOW_SNAP;
    orbit.pivot = follow_pivot(orbit.pivot, wanted_pivot, dt);

    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    let seat = boom_seat(orbit.pivot, rotation, orbit.rig.distance);
    // The sweep starts at the head, the capsule's top hemisphere centre: body collision keeps
    // it inside whatever room the body is in, so the camera never ends past a wall.
    let head = body.translation + Vec3::Y * (HEIGHT - CAPSULE_RADIUS);
    let boom = seat - head;
    let boom_len = boom.length().max(1.0e-3);
    let hit = collide.cast_camera(head, boom, false);
    orbit.hit = hit;
    orbit.open = ease_open(orbit.open, hit.unwrap_or(boom_len), dt);
    let seated = head + boom * (orbit.open / boom_len).clamp(0.0, 1.0);

    if let Some(obs) = observe.as_deref() {
        // Dev: draw the rig and look at it from outside.
        gizmos.sphere(
            Isometry3d::from_translation(head),
            0.12,
            Color::srgb(1.0, 1.0, 0.0),
        );
        gizmos.line(head, seat, Color::WHITE);
        gizmos.sphere(
            Isometry3d::from_translation(seat),
            0.3,
            Color::srgb(1.0, 0.2, 0.2),
        );
        gizmos.sphere(
            Isometry3d::from_translation(seated),
            0.3,
            Color::srgb(0.2, 1.0, 1.0),
        );
        let eye = body.translation + obs.offset;
        *xf = Transform::from_translation(eye)
            .looking_at(body.translation + Vec3::Y * obs.target_height, Vec3::Y);
    } else {
        // A blow's jolt on top of the seat, from the same base pose each frame (nothing builds up).
        let (shift, tilt) = shake.offset();
        let wanted = view_rotation(seated, orbit.pivot, orbit.look, orbit.focus, rotation);
        let view = stable_lock_view(orbit.view, wanted, lock.target.is_some() && !snapped, dt);
        orbit.view = view;
        xf.rotation = view * tilt;
        xf.translation = seated + view * shift;
    }
    let alpha = self_model_fade_alpha(
        (seated - orbit.pivot).length(),
        NEARCLIP_DEFAULT,
        SELF_FADE_WINDOW,
    );
    if fade.0 != alpha {
        fade.0 = alpha;
    }
}

/// The camera's seat `distance` behind `pivot` along the view.
pub fn boom_seat(pivot: Vec3, rotation: Quat, distance: f32) -> Vec3 {
    pivot + rotation * Vec3::Z * distance
}

/// The eased boom length: geometry that cuts in snaps it in at once; once clear it grows back
/// at [`RETURN_RATE`], frame-rate independent.
pub fn ease_open(current: f32, open: f32, dt: f32) -> f32 {
    if open < current {
        open
    } else {
        current + (open - current) * (1.0 - (-RETURN_RATE * dt).exp())
    }
}

/// The smoothed pivot: tight across the ground, softer in height; snaps across a teleport.
pub fn follow_pivot(current: Vec3, wanted: Vec3, dt: f32) -> Vec3 {
    if current.distance(wanted) > FOLLOW_SNAP {
        return wanted;
    }
    let k = |rate: f32| 1.0 - (-rate * dt).exp();
    let xz = current.xz().lerp(wanted.xz(), k(FOLLOW_RATE_XZ));
    let y = current.y + (wanted.y - current.y) * k(FOLLOW_RATE_Y);
    Vec3::new(xz.x, y, xz.y)
}

/// One frame of the recentre swing: the new yaw, and whether it has arrived.
pub fn recenter_step(yaw: f32, target: f32, dt: f32) -> (f32, bool) {
    let gap = wrap_pi(target - yaw);
    if gap.abs() <= RECENTER_DONE {
        return (target, true);
    }
    let step = gap.abs() * (1.0 - (-RECENTER_RATE * dt).exp());
    // Never slower than a crawl, so the exponential tail finishes.
    let yaw = turn_toward(yaw, target, step.max(RECENTER_DONE));
    (wrap_pi(yaw), false)
}

/// `WOWSOULS_OBSERVE=<x>,<y>,<z>` (a Bevy-space offset from the feet; bare is overhead): render
/// from an observer while the rig still runs, with the head (yellow), wanted seat (red), boom
/// (white) and seated eye (cyan) drawn. A camera-debugging instrument.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Observe {
    pub offset: Vec3,
    /// Architectural inspection can frame a roof without moving the grounded avatar.
    pub target_height: f32,
}

/// `WOWSOULS_SPIN=<rad/s>`: orbit the camera at a constant rate. An unattended-run instrument.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ScriptedSpin(pub f32);

/// `WOWSOULS_TRACE=1`: the camera's wanted and allowed boom and the body fade, twice a second.
fn trace_camera(
    time: Res<Time<bevy::time::Real>>,
    cam: Query<(&OrbitCam, &Transform)>,
    fade: Res<BodyFade>,
    mut next_at: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now < *next_at {
        return;
    }
    *next_at = now + 0.5;
    if let Ok((c, xf)) = cam.single() {
        let eye = benilla_assets::coords::bevy_to_wow(xf.translation);
        info!(
            "cam   t={now:6.2} yaw={:+.2} pitch={:+.2} boom {:.2}/{:.2} fade={:.2} eye=({:.2},{:.2},{:.2}) hit={:?}",
            c.yaw,
            c.pitch,
            c.open,
            c.rig.distance,
            fade.0,
            eye[0],
            eye[1],
            eye[2],
            c.hit
        );
    }
}

/// Reads `WOWSOULS_SPIN` into the app.
pub fn instruments(app: &mut App) {
    if let Ok(v) = std::env::var("WOWSOULS_OBSERVE") {
        let offset = crate::slice::parse_wow_point(&v, 0.0)
            .map(Vec3::from_array)
            .unwrap_or(Vec3::new(0.0, 18.0, 10.0));
        let target_height = std::env::var("WOWSOULS_OBSERVE_HEIGHT")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(0.0);
        app.insert_resource(Observe {
            offset,
            target_height,
        })
        // The rig's gizmos draw through the world.
        .add_systems(Startup, |mut store: ResMut<GizmoConfigStore>| {
            store.config_mut::<DefaultGizmoConfigGroup>().0.depth_bias = -1.0;
            store.config_mut::<DefaultGizmoConfigGroup>().0.enabled =
                std::env::var_os("WOWSOULS_OBSERVE_CLEAN").is_none();
        });
    }
    if let Some(rate) = std::env::var("WOWSOULS_SPIN")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|r| r.is_finite())
    {
        app.insert_resource(ScriptedSpin(rate));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_switches_have_bounded_motion_and_settle_at_all_frame_rates() {
        let pivot = Vec3::Y * PIVOT_HEIGHT;
        let a = Vec3::new(-6.0, 0.8, -7.0);
        let b = Vec3::new(6.0, 0.8, -7.0);
        for fps in [30, 60, 144] {
            let dt = 1.0 / fps as f32;
            let (mut yaw, mut pitch) = crate::lockon::lock_angles(pivot, a, &FREE_RIG);
            let mut look = a;
            let boom = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
            let mut view = view_rotation(boom_seat(pivot, boom, DISTANCE), pivot, look, 0.6, boom);
            for _ in 0..fps * 2 {
                look = follow_lock_aim(pivot, look, b, dt);
                let (want_yaw, want_pitch) = crate::lockon::lock_angles(pivot, look, &FREE_RIG);
                let (before_yaw, before_pitch) = (yaw, pitch);
                yaw = lock_turn(yaw, want_yaw, LOCK_YAW_SPEED, dt);
                pitch = lock_turn(pitch, want_pitch, LOCK_PITCH_SPEED, dt);
                assert!(wrap_pi(yaw - before_yaw).abs() <= LOCK_YAW_SPEED * dt + 1.0e-5);
                assert!((pitch - before_pitch).abs() <= LOCK_PITCH_SPEED * dt + 1.0e-5);
                let boom = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
                let wanted =
                    view_rotation(boom_seat(pivot, boom, DISTANCE), pivot, look, 0.6, boom);
                let next = stable_lock_view(view, wanted, true, dt);
                assert!(
                    view.angle_between(next) <= LOCK_VIEW_SPEED * dt + 1.0e-4,
                    "{fps}"
                );
                view = next;
            }
            assert!(look.distance(b) < 0.001, "{fps}: {look:?}");
            let (want_yaw, _) = crate::lockon::lock_angles(pivot, b, &FREE_RIG);
            assert!(wrap_pi(want_yaw - yaw).abs() < 0.005, "{fps}: {yaw}");
        }
    }

    #[test]
    fn near_target_crossings_and_yaw_wrap_do_not_snap_or_feed_back_shake() {
        let dt = 1.0 / 60.0;
        let mut base = Quat::from_rotation_y(3.1);
        for wanted in [
            Quat::from_rotation_y(-3.1),
            Quat::from_rotation_x(-1.2),
            Quat::from_rotation_y(1.5),
        ] {
            let next = stable_lock_view(base, wanted, true, dt);
            assert!(base.angle_between(next) <= LOCK_VIEW_SPEED * dt + 1.0e-4);
            base = next;
        }
        assert!(wrap_pi(lock_turn(3.0, -3.0, LOCK_YAW_SPEED, dt) - 3.0) > 0.0);
        assert_eq!(
            stable_lock_view(base, Quat::IDENTITY, false, dt),
            Quat::IDENTITY
        );
        assert_eq!(stable_lock_view(base, Quat::IDENTITY, true, 0.0), base);
    }

    #[test]
    fn a_level_camera_sits_straight_behind_the_pivot() {
        let seat = boom_seat(Vec3::ZERO, Quat::IDENTITY, 5.0);
        assert!((seat - Vec3::new(0.0, 0.0, 5.0)).length() < 1.0e-5);
    }

    #[test]
    fn looking_down_raises_the_seat() {
        let rot = Quat::from_euler(EulerRot::YXZ, 0.0, PITCH_OPEN, 0.0);
        let seat = boom_seat(Vec3::ZERO, rot, DISTANCE);
        assert!(
            seat.y > 0.0,
            "a downward look sits above the pivot: {seat:?}"
        );
        assert!(((seat.length()) - DISTANCE).abs() < 1.0e-4);
    }

    #[test]
    fn the_camera_always_looks_at_the_pivot() {
        for (yaw, pitch) in [(0.0, -0.3), (1.2, 0.2), (-2.5, -1.0)] {
            let rot = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
            let pivot = Vec3::new(3.0, 1.0, -2.0);
            let seat = boom_seat(pivot, rot, DISTANCE);
            let to_pivot = (pivot - seat).normalize();
            assert!((to_pivot - rot * Vec3::NEG_Z).length() < 1.0e-5);
        }
    }

    #[test]
    fn geometry_snaps_the_boom_in_at_once() {
        assert_eq!(ease_open(5.5, 1.2, 1.0 / 60.0), 1.2);
    }

    #[test]
    fn a_cleared_boom_eases_back_out_at_the_return_rate() {
        let dt = 1.0 / 60.0;
        let mut open = 1.0;
        let mut frames = 0;
        while open < 5.0 {
            open = ease_open(open, 5.5, dt);
            frames += 1;
            assert!(open <= 5.5);
        }
        // 1 - e^(-6 t) reaches (5 - 1)/(5.5 - 1) at t ≈ 0.36 s.
        assert!((18..=26).contains(&frames), "{frames} frames");
    }

    #[test]
    fn easing_is_frame_rate_independent() {
        // A quarter second at each rate.
        let run = |fps: u32| {
            let dt = 1.0 / fps as f32;
            let mut open = 1.0;
            for _ in 0..fps / 4 {
                open = ease_open(open, 5.5, dt);
            }
            open
        };
        assert!((run(40) - run(240)).abs() < 1.0e-3);
    }

    #[test]
    fn the_pivot_follows_tight_across_and_soft_in_height() {
        let dt = 1.0 / 60.0;
        let p = follow_pivot(Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0), dt);
        assert!(
            p.x > p.y,
            "horizontal catches up faster than vertical: {p:?}"
        );
        // After half a second both have arrived.
        let mut p = Vec3::ZERO;
        for _ in 0..30 {
            p = follow_pivot(p, Vec3::new(1.0, 1.0, 0.0), dt);
        }
        assert!((p - Vec3::new(1.0, 1.0, 0.0)).length() < 0.02, "{p:?}");
    }

    #[test]
    fn a_teleport_snaps_the_pivot() {
        let far = Vec3::new(0.0, 0.0, 50.0);
        assert_eq!(follow_pivot(Vec3::ZERO, far, 1.0 / 60.0), far);
    }

    #[test]
    fn the_recentre_arrives_from_either_side_within_0_4_s() {
        for start in [2.5_f32, -2.5, 0.3, -3.1] {
            let (mut yaw, mut frames) = (start, 0);
            loop {
                let (y, done) = recenter_step(yaw, 0.0, 1.0 / 60.0);
                yaw = y;
                frames += 1;
                if done {
                    break;
                }
                assert!(frames < 60, "start {start}: still swinging at {yaw}");
            }
            assert!(yaw.abs() < 1.0e-6);
            assert!(frames <= 24, "start {start}: {frames} frames");
        }
    }

    #[test]
    fn the_recentre_takes_the_short_way_round() {
        // From +3.0 to -3.0 is 0.28 rad across the seam, not 6 rad back.
        let (yaw, _) = recenter_step(3.0, -3.0, 1.0 / 60.0);
        assert!(
            wrap_pi(yaw - 3.0) > 0.0,
            "it swings positive, across π: {yaw}"
        );
    }

    #[test]
    fn the_free_rig_is_dark_souls_default_row() {
        // `LockCamParam` row 0 (DSR's GameParam): 4.0 m back, 1.42 m up, 43 degrees.
        assert!((DISTANCE - 4.0 * METRES_TO_YARDS).abs() < 1.0e-4);
        assert!((PIVOT_HEIGHT - 1.42 * METRES_TO_YARDS).abs() < 1.0e-4);
        assert!((FOV_Y.to_degrees() - 43.0).abs() < 1.0e-3);
        assert_eq!(FREE_RIG.distance, DISTANCE);
        assert_eq!(FREE_RIG.pivot_height, PIVOT_HEIGHT);
        assert!((FREE_RIG.pitch_min.to_degrees() + 40.0).abs() < 1.0e-3);
        assert_eq!(FREE_RIG.shift_ratio, 0.6);
    }

    #[test]
    fn a_human_sized_target_gets_the_default_rig() {
        // Garrick and the thugs are the capsule's height: DS's 1.8 m human.
        let rig = lock_rig(crate::avatar::HEIGHT);
        assert!((rig.distance - DISTANCE).abs() < 0.1, "{rig:?}");
        assert!((rig.pivot_height - PIVOT_HEIGHT).abs() < 0.05, "{rig:?}");
    }

    #[test]
    fn no_target_draws_the_camera_closer_than_the_default_and_big_ones_push_it_out() {
        let d = |h: f32| lock_rig(h).distance;
        // A rat, a kobold, a human: rows 101, 102, 0 would come in to 2.2 m / 3.2 m, which hides the
        // enemy behind the player's body: the lock never sits closer than the free camera.
        for h in [0.0, 0.5, 1.4, 2.03] {
            assert!((d(h) - DISTANCE).abs() < 0.1, "height {h}: {}", d(h));
        }
        // A giant and a dragon: rows 103 and 104, 5.0 m and 6.0 m, held beyond the end.
        let big = [d(2.03), d(4.0), d(7.0)];
        assert!(big.windows(2).all(|w| w[0] < w[1]), "{big:?}");
        assert!((d(100.0) - 6.0 * METRES_TO_YARDS).abs() < 1.0e-4);
    }

    #[test]
    fn no_target_lowers_the_pivot_below_the_default() {
        for h in [0.0, 0.5, 1.4, 2.03, 4.0, 7.0, 100.0] {
            assert!(
                lock_rig(h).pivot_height >= PIVOT_HEIGHT - 1.0e-4,
                "height {h}: {}",
                lock_rig(h).pivot_height
            );
        }
    }

    #[test]
    fn the_view_aims_part_of_the_way_to_the_locked_target() {
        let (pivot, aim) = (Vec3::new(0.0, 1.6, 0.0), Vec3::new(0.0, 0.8, -10.0));
        let seat = Vec3::new(0.0, 3.0, 4.3);
        let boom = Quat::from_euler(EulerRot::YXZ, 0.0, -0.3, 0.0);
        // Free: exactly the orbit's own rotation.
        assert_eq!(view_rotation(seat, pivot, aim, 0.0, boom), boom);
        // Locked at 0.6: the view points at the point 60 % of the way to the target.
        let view = view_rotation(seat, pivot, aim, 0.6, boom);
        let want = (pivot.lerp(aim, 0.6) - seat).normalize();
        assert!((view * Vec3::NEG_Z - want).length() < 1.0e-4);
    }

    #[test]
    fn the_focus_eases_to_the_rigs_ratio_and_back() {
        let dt = 1.0 / 60.0;
        let mut focus = 0.0_f32;
        for _ in 0..180 {
            focus = ease_focus(focus, Some(0.6), dt);
        }
        assert!((focus - 0.6).abs() < 0.01, "{focus}");
        for _ in 0..180 {
            focus = ease_focus(focus, None, dt);
        }
        assert!(focus < 0.01, "{focus}");
    }

    #[test]
    fn big_targets_keep_the_camera_low() {
        // Rows 103 and 104 stop the look-down at 20 degrees; a human's row at 40.
        assert!((lock_rig(4.0).pitch_min.to_degrees() + 20.0).abs() < 1.0);
        assert!((lock_rig(crate::avatar::HEIGHT).pitch_min.to_degrees() + 40.0).abs() < 2.0);
        assert!(lock_rig(4.0).shift_ratio < lock_rig(1.4).shift_ratio);
    }

    #[test]
    fn the_rig_eases_to_a_new_target_size_and_arrives() {
        // From a big target's rig back to the free one.
        let (from, to) = (lock_rig(4.0), FREE_RIG);
        let one = ease_rig(from, to, 1.0 / 60.0);
        assert!(one.distance < from.distance && one.distance > to.distance);
        let mut rig = from;
        for _ in 0..120 {
            rig = ease_rig(rig, to, 1.0 / 60.0);
        }
        assert!((rig.distance - to.distance).abs() < 0.01, "{rig:?}");
        assert!((rig.pivot_height - to.pivot_height).abs() < 0.01, "{rig:?}");
    }

    #[test]
    fn the_rig_puts_the_world_camera_at_dark_souls_fov() {
        use bevy::camera::PerspectiveProjection;
        let mut projection = Projection::from(PerspectiveProjection {
            fov: std::f32::consts::FRAC_PI_3,
            ..default()
        });
        assert!(set_fov(&mut projection), "60 degrees is changed");
        let Projection::Perspective(p) = &projection else {
            panic!("still perspective");
        };
        assert_eq!(p.fov, FOV_Y);
        assert!(
            !set_fov(&mut projection),
            "already there: no change flagged"
        );
    }

    fn look(keys: &[KeyCode], pad: Option<(GamepadAxis, f32)>) -> Vec2 {
        use crate::pad::testkit::{read, world_with_pad};
        let mut held = ButtonInput::<KeyCode>::default();
        for &k in keys {
            held.press(k);
        }
        let mut world = match pad {
            Some((axis, v)) => world_with_pad(move |p| {
                p.analog_mut().set(axis, v);
            }),
            None => World::new(),
        };
        let mouse = ButtonInput::<MouseButton>::default();
        read(&mut world, move |pads| {
            let all: Vec<&Gamepad> = pads.iter().collect();
            let h = Held {
                keys: &held,
                mouse: &mouse,
                pads: &all,
                captured: false,
            };
            look_stick(&Binds::default(), &h, pads)
        })
    }

    #[test]
    fn the_arrow_keys_look_like_a_stick() {
        assert_eq!(look(&[KeyCode::ArrowUp], None), Vec2::Y);
        assert_eq!(look(&[KeyCode::ArrowLeft], None), Vec2::NEG_X);
        assert_eq!(
            look(&[KeyCode::ArrowUp, KeyCode::ArrowDown], None),
            Vec2::ZERO
        );
        // A diagonal is no faster than a straight push.
        let diagonal = look(&[KeyCode::ArrowUp, KeyCode::ArrowRight], None);
        assert!((diagonal.length() - 1.0).abs() < 1.0e-5, "{diagonal:?}");
        assert_eq!(look(&[], None), Vec2::ZERO);
    }

    #[test]
    fn the_right_stick_looks_past_its_deadzone_and_the_harder_push_wins() {
        // Inside the deadzone: still. Past it: a push on the stick's side.
        assert_eq!(look(&[], Some((GamepadAxis::RightStickX, 0.1))), Vec2::ZERO);
        let right = look(&[], Some((GamepadAxis::RightStickX, 1.0)));
        assert!((right - Vec2::X).length() < 1.0e-5, "{right:?}");
        // A full arrow push beats a half stick.
        let both = look(&[KeyCode::ArrowDown], Some((GamepadAxis::RightStickX, 0.5)));
        assert_eq!(both, Vec2::NEG_Y);
    }
}
