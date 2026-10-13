//! Headless collision fixtures for the motor tests: a static trimesh world built from a 2D
//! profile, and the player capsule. From benilla @ 2e82d34 `player/mover.rs` tests.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{CAPSULE_HEIGHT, CAPSULE_RADIUS};

/// A headless world from a `(x, y)` polyline extruded across `z`, wound so every face's
/// authored normal points up and back at the approaching body: faces are one-sided.
pub(crate) fn world_from_profile(profile: &[(f32, f32)]) -> App {
    const W: f32 = 3.0;
    let mut app = App::new();
    // `WorldCollision` needs the mover's trace exclusions, which the world plugins insert.
    app.init_resource::<benilla_world::collision::MoverTraceExclusions>();
    // avian's collider backend reads `Assets<Mesh>` and `SceneSpawner` even without meshes.
    app.add_plugins((
        MinimalPlugins,
        bevy::transform::TransformPlugin,
        bevy::asset::AssetPlugin::default(),
        bevy::scene::ScenePlugin,
        PhysicsPlugins::new(bevy::app::PostUpdate),
    ));
    app.init_asset::<Mesh>();
    let (mut verts, mut tris) = (Vec::new(), Vec::new());
    for w in profile.windows(2) {
        let (&(x0, y0), &(x1, y1)) = (&w[0], &w[1]);
        let b = verts.len() as u32;
        verts.extend([
            Vec3::new(x0, y0, -W),
            Vec3::new(x1, y1, -W),
            Vec3::new(x1, y1, W),
            Vec3::new(x0, y0, W),
        ]);
        // (a, c, b) / (a, d, c): normal = (-dy, dx, 0), up for the flats and up-and-back for
        // the riser; the reverse winding would block nothing.
        tris.extend([[b, b + 2, b + 1], [b, b + 3, b + 2]]);
    }
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::trimesh(verts, tris),
        Transform::default(),
    ));
    app.update(); // one frame builds Position/Rotation and the spatial-query trees
    app
}

pub(crate) fn player_capsule() -> Collider {
    Collider::capsule(CAPSULE_RADIUS, CAPSULE_HEIGHT - 2.0 * CAPSULE_RADIUS)
}
