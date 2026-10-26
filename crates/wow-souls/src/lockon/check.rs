//! Opt-in unattended camera fixtures and unshaken view measurements. No gameplay NPC changes.
use super::{LockOn, Lockable};
use crate::avatar::Avatar;
use crate::camera::OrbitCam;
use crate::controller::PlaySet;
use crate::settle::Phase;
use bevy::prelude::*;

#[derive(Resource)]
struct Scene(String);
#[derive(Component)]
struct Anchor {
    index: usize,
    home: Vec3,
}

pub(super) fn plugin(app: &mut App) {
    if crate::run_mode::unattended() {
        if let Ok(scene) = std::env::var("WOWSOULS_LOCK_SCENE") {
            app.insert_resource(Scene(scene))
                .add_systems(OnEnter(Phase::Playing), spawn)
                .add_systems(
                    Update,
                    (bind_actors, move_anchor)
                        .chain()
                        .before(PlaySet::Intent)
                        .run_if(in_state(Phase::Playing)),
                );
        }
    }
    if std::env::var_os("WOWSOULS_LOCK_TRACE").is_some() {
        app.add_systems(
            Update,
            trace
                .after(PlaySet::Camera)
                .run_if(in_state(Phase::Playing)),
        );
    }
}

fn spawn(
    mut commands: Commands,
    scene: Res<Scene>,
    avatar: Query<&Transform, With<Avatar>>,
    ground: benilla_world::world_point::WorldPoint,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if scene.0 == "actors" {
        return;
    }
    let Ok(body) = avatar.single() else {
        return;
    };
    let positions = match scene.0.as_str() {
        "wide" => [
            Vec3::new(0.0, 0.0, -7.0),
            Vec3::new(10.0, 0.0, -1.5),
            Vec3::new(-10.0, 0.0, -1.5),
            Vec3::new(12.0, 0.0, 4.0),
        ],
        "near" => [
            Vec3::new(0.0, 0.0, -1.6),
            Vec3::new(5.0, 0.0, -8.0),
            Vec3::new(-5.0, 0.0, -8.0),
            Vec3::new(10.0, 0.0, -4.0),
        ],
        _ => [
            Vec3::new(0.0, 0.0, -9.0),
            Vec3::new(5.0, 0.0, -8.0),
            Vec3::new(-5.0, 0.0, -8.0),
            Vec3::new(10.0, 0.0, -4.0),
        ],
    };
    let colors = [
        Color::srgb(0.85, 0.67, 0.25),
        Color::srgb(0.25, 0.65, 0.80),
        Color::srgb(0.75, 0.30, 0.27),
        Color::srgb(0.55, 0.40, 0.75),
    ];
    for (index, offset) in positions.into_iter().enumerate() {
        let mut at = body.translation + offset;
        at.y = ground.terrain_height_under(at).unwrap_or(at.y);
        let parent = commands
            .spawn((
                Name::new(format!("lock fixture {index}")),
                Anchor { index, home: at },
                Transform::from_translation(at),
                Visibility::Inherited,
                Lockable {
                    aim_height: 1.2,
                    height: 1.8,
                },
            ))
            .id();
        commands.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.28, 1.05))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: colors[index],
                perceptual_roughness: 0.7,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.82, 0.0),
            ChildOf(parent),
        ));
        info!("lock-check: fixture {index} = {parent} at {at:?}");
    }
}

/// The visual check may reuse four real, stationary NPCs from an isolated manifest. Only this
/// unattended fixture makes them lockable; the ordinary game's friendly NPCs remain untouched.
fn bind_actors(
    mut commands: Commands,
    scene: Res<Scene>,
    actors: Query<(Entity, &crate::friendly::Npc), Without<Anchor>>,
) {
    if scene.0 != "actors" {
        return;
    }
    for (entity, npc) in &actors {
        if let Some(index) = [197, 823, 196, 152]
            .iter()
            .position(|&entry| entry == npc.entry)
        {
            commands.entity(entity).insert((
                Lockable {
                    aim_height: 1.2,
                    height: 1.8,
                },
                Anchor {
                    index,
                    home: Vec3::ZERO,
                },
            ));
            info!(
                "lock-check: actor fixture {index} = {entity} (entry {})",
                npc.entry
            );
        }
    }
}

fn move_anchor(
    time: Res<Time<bevy::time::Real>>,
    scene: Res<Scene>,
    mut anchors: Query<(&Anchor, &mut Transform)>,
) {
    let t = (time.elapsed_secs() - 25.0).max(0.0);
    for (anchor, mut xf) in &mut anchors {
        if anchor.index != 0 {
            continue;
        }
        match scene.0.as_str() {
            "moving" => xf.translation.x = anchor.home.x + (t * 0.8).sin() * 6.0,
            "near" => {
                xf.translation.x = anchor.home.x + (t * 1.5).sin() * 1.6;
                xf.translation.z = anchor.home.z + 1.6 - (t * 1.5).cos() * 1.6;
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct Trace {
    last: Option<(Quat, f32, f32)>,
    next: f32,
    max_view: f32,
    max_yaw: f32,
    max_pitch: f32,
}
fn trace(
    time: Res<Time<bevy::time::Real>>,
    camera_time: Res<Time>,
    lock: Res<LockOn>,
    camera: Query<&OrbitCam>,
    mut state: Local<Trace>,
) {
    let Ok(cam) = camera.single() else {
        return;
    };
    // The engine paces virtual time to presentation cadence. CPU-loop Real deltas can be
    // 26 ms then 8 ms for two frames displayed 16.7 ms apart; compare against the camera's clock.
    let dt = camera_time.delta_secs();
    if let Some((view, yaw, pitch)) = state.last {
        if lock.target.is_some() && dt > 0.0 {
            let yaw_step = crate::controller::wrap_pi(cam.yaw - yaw).abs().to_degrees();
            // Relative-rotation atan2 resolves tiny steps that acos(dot) rounds to zero.
            let delta = (view.conjugate() * cam.view).normalize();
            let view_step = 2.0 * delta.xyz().length().atan2(delta.w.abs()).to_degrees();
            state.max_view = state.max_view.max(view_step / dt);
            state.max_yaw = state.max_yaw.max(yaw_step / dt);
            state.max_pitch = state
                .max_pitch
                .max((cam.pitch - pitch).abs().to_degrees() / dt);
        }
    }
    state.last = Some((cam.view, cam.yaw, cam.pitch));
    let now = time.elapsed_secs();
    if now >= state.next {
        state.next = now + 0.2;
        info!(
            "lock-check t={now:.3} target={:?} max_view={:.2} max_yaw={:.2} max_pitch={:.2}",
            lock.target, state.max_view, state.max_yaw, state.max_pitch
        );
    }
}
