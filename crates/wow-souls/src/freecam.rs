//! The free-fly camera: spawns the world camera, and is the debug eye behind F1 once play starts
//! (the orbit rig, [`crate::camera`], drives it otherwise). Spawned
//! with the component set benilla's world camera needs (`WorldCamera`, HDR target without
//! tonemapping, the latched MSAA level, FFXGlow), as `benilla_world::worldview` spawns its own.
//!
//! Controls: hold the right mouse button to look, WASD to move, Space/C for up/down, Shift to
//! boost, the wheel to change speed.

use bevy::camera::{PerspectiveProjection, Projection};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use benilla_assets::coords::wow_to_bevy;
use benilla_world::schedule::WorldStage;
use benilla_world::view::{MsaaSetting, WorldCamera, CAM_FAR, NEARCLIP_DEFAULT};

use crate::camera::CamMode;
use crate::slice;

/// Where the eye opens relative to the slice anchor, in Bevy space (yards): up and back, looking
/// at the anchor.
const OPEN_OFFSET: Vec3 = Vec3::new(0.0, 18.0, 38.0);
/// Vertical field of view, radians (60°).
const FOVY: f32 = std::f32::consts::FRAC_PI_3;
/// Radians of turn per pixel of mouse motion.
const LOOK_SENSITIVITY: f32 = 0.003;
/// Pitch limit, just short of straight up/down so the basis never degenerates.
const PITCH_LIMIT: f32 = 1.54;
const SPEED_DEFAULT: f32 = 30.0;
const SPEED_RANGE: std::ops::RangeInclusive<f32> = 1.0..=1000.0;
const BOOST: f32 = 5.0;

#[derive(Component)]
pub struct FreeCam {
    yaw: f32,
    pitch: f32,
    speed: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn.after(benilla_assets::AssetSet::Open))
        .add_systems(
            Update,
            fly.in_set(WorldStage::Input)
                .run_if(resource_equals(CamMode::Free)),
        );
}

fn spawn(mut commands: Commands, msaa: Res<MsaaSetting>) {
    let target = wow_to_bevy(slice::anchor());
    let transform = Transform::from_translation(target + OPEN_OFFSET).looking_at(target, Vec3::Y);
    let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
    commands.spawn((
        Camera3d::default(),
        WorldCamera,
        msaa.level(),
        Projection::from(PerspectiveProjection {
            far: CAM_FAR,
            near: NEARCLIP_DEFAULT,
            fov: FOVY,
            ..default()
        }),
        // The engine lights in clamped gamma space into a linear float target.
        Hdr,
        Tonemapping::None,
        benilla_world::ffx_glow::FfxGlow::WORLD,
        transform,
        FreeCam {
            yaw,
            pitch,
            speed: SPEED_DEFAULT,
        },
    ));
}

fn fly(
    mode: Res<CamMode>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut cam: Query<(&mut Transform, &mut FreeCam)>,
) {
    let Ok((mut xf, mut cam)) = cam.single_mut() else {
        return;
    };
    // Taking over from the orbit rig: start from where it left the eye.
    if mode.is_changed() {
        let (yaw, pitch, _) = xf.rotation.to_euler(EulerRot::YXZ);
        cam.yaw = yaw;
        cam.pitch = pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    let looking = buttons.pressed(MouseButton::Right);
    if let Ok(mut cursor) = cursor.single_mut() {
        let want = if looking {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        };
        if cursor.grab_mode != want {
            cursor.grab_mode = want;
            cursor.visible = !looking;
        }
    }
    if looking {
        cam.yaw -= motion.delta.x * LOOK_SENSITIVITY;
        cam.pitch =
            (cam.pitch - motion.delta.y * LOOK_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }
    xf.rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);

    for ev in wheel.read() {
        cam.speed =
            (cam.speed * (1.0 + ev.y * 0.1)).clamp(*SPEED_RANGE.start(), *SPEED_RANGE.end());
    }

    let axis = |pos: KeyCode, neg: KeyCode| {
        f32::from(u8::from(keys.pressed(pos))) - f32::from(u8::from(keys.pressed(neg)))
    };
    let dir = *xf.forward() * axis(KeyCode::KeyW, KeyCode::KeyS)
        + *xf.right() * axis(KeyCode::KeyD, KeyCode::KeyA)
        + Vec3::Y * axis(KeyCode::Space, KeyCode::KeyC);
    if let Some(dir) = dir.try_normalize() {
        let boost = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
            BOOST
        } else {
            1.0
        };
        xf.translation += dir * cam.speed * boost * time.delta_secs();
    }
}
