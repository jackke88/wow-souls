//! Authored memorials at the database's graveyard positions. Ordinary stone, an inset rune,
//! and a quiet soul light; no animation assets or game-install changes.

use std::collections::BTreeMap;

use avian3d::prelude::{Collider, RigidBody};
use benilla_world::collision::WorldCollision;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::Binding;
use crate::avatar::Avatar;
use crate::settle::{snap_to_ground, Phase};

#[derive(Resource, Default)]
struct Live(BTreeMap<u32, Entity>);

#[derive(Component)]
struct Rune(u32);

const RUNE_IMAGE: &[u8] = include_bytes!("../../assets/soulbind/memorial-rune-v1.png");

pub(crate) fn rune_texture() -> Image {
    let mut reader = png::Decoder::new(std::io::Cursor::new(RUNE_IMAGE))
        .read_info()
        .expect("embedded memorial PNG");
    let mut pixels = vec![0; reader.output_buffer_size().expect("memorial PNG size")];
    let info = reader.next_frame(&mut pixels).expect("memorial PNG pixels");
    assert_eq!(info.color_type, png::ColorType::Rgba);
    pixels.truncate(info.buffer_size());
    Image::new(
        Extent3d {
            width: info.width,
            height: info.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn rune_face(back: bool) -> Transform {
    Transform::from_xyz(0.0, 0.98, if back { -0.176 } else { 0.176 }).with_rotation(
        Quat::from_rotation_y(if back { std::f32::consts::PI } else { 0.0 }),
    )
}

/// The world uses its own sunlight shader. These ordinary Bevy props use painted shading,
/// as WoW's graveyard textures do, so they remain readable in that renderer.
fn stone_texture() -> Image {
    const SIZE: u32 = 128;
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let hash = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263);
            let grain = ((hash ^ (hash >> 13)).wrapping_mul(1274126177) >> 24) as f32 / 255.0;
            let cloud = ((x as f32 * 0.12).sin() * (y as f32 * 0.08).cos()) * 0.08;
            let border = x.min(SIZE - 1 - x).min(y.min(SIZE - 1 - y));
            let edge = if border < 3 {
                0.65
            } else if border < 7 {
                1.07
            } else {
                0.91
            };
            let shade = (0.80 + grain * 0.19 + cloud) * edge;
            pixels.extend([
                (178.0 * shade) as u8,
                (191.0 * shade) as u8,
                (194.0 * shade) as u8,
                255,
            ]);
        }
    }
    Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Live>().add_systems(
        Update,
        (stones, glow).chain().run_if(in_state(Phase::Playing)),
    );
}

fn piece(
    commands: &mut Commands,
    root: Entity,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    transform: Transform,
) -> Entity {
    commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            transform,
            ChildOf(root),
        ))
        .id()
}

fn stones(
    mut commands: Commands,
    binding: Res<Binding>,
    mut live: ResMut<Live>,
    avatar: Query<&Transform, With<Avatar>>,
    collide: WorldCollision,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok(me) = avatar.single() else { return };
    for site in &binding.sites {
        if live.0.contains_key(&site.id) || me.translation.distance(site.at) > 100.0 {
            continue;
        }
        let Some(y) = snap_to_ground(&collide, site.at)
            .filter(|y| (*y - site.at.y).abs() <= crate::npc::GROUND_TOLERANCE)
        else {
            continue;
        };
        let stone = materials.add(StandardMaterial {
            base_color: Color::srgb(0.75, 0.78, 0.78),
            base_color_texture: Some(images.add(stone_texture())),
            perceptual_roughness: 1.0,
            unlit: true,
            ..default()
        });
        let dark = materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.16, 0.17),
            perceptual_roughness: 1.0,
            unlit: true,
            ..default()
        });
        let rune = images.add(rune_texture());
        let light = materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.85, 0.82),
            base_color_texture: Some(rune.clone()),
            emissive_texture: Some(rune),
            alpha_mode: AlphaMode::Mask(0.15),
            perceptual_roughness: 1.0,
            unlit: true,
            ..default()
        });
        let root = commands
            .spawn((
                Name::new(format!("{} Memorial Stone", site.name)),
                super::Memorial(site.id),
                Transform::from_translation(Vec3::new(site.at.x, y, site.at.z))
                    .with_rotation(site.facing()),
                Visibility::default(),
            ))
            .id();
        live.0.insert(site.id, root);
        commands.spawn((
            RigidBody::Static,
            Collider::cuboid(0.94, 1.32, 0.34),
            Transform::from_xyz(0.0, 0.96, 0.0),
            ChildOf(root),
        ));
        let block = meshes.add(Cuboid::new(1.30, 0.20, 0.76));
        piece(
            &mut commands,
            root,
            block,
            stone.clone(),
            Transform::from_xyz(0.0, 0.1, 0.0),
        );
        let block = meshes.add(Cuboid::new(1.10, 0.12, 0.57));
        piece(
            &mut commands,
            root,
            block,
            dark.clone(),
            Transform::from_xyz(0.0, 0.26, 0.0),
        );
        let block = meshes.add(Cuboid::new(0.94, 0.90, 0.34));
        piece(
            &mut commands,
            root,
            block,
            stone.clone(),
            Transform::from_xyz(0.0, 0.74, 0.0),
        );
        let cap = meshes.add(Cylinder::new(0.47, 0.34));
        piece(
            &mut commands,
            root,
            cap,
            stone.clone(),
            Transform::from_xyz(0.0, 1.17, 0.0)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        );
        // Separate outward-facing decals keep the lettering-free sigil upright on both faces.
        // Alpha cutout reveals the actual stone; there is no black backing rectangle.
        let decal = meshes.add(Rectangle::new(0.78, 1.17));
        for back in [false, true] {
            let entity = piece(
                &mut commands,
                root,
                decal.clone(),
                light.clone(),
                rune_face(back),
            );
            commands.entity(entity).insert(Rune(site.id));
        }
        // A narrow weathered ledge catches the world's light across the stone's face.
        let ledge = meshes.add(Cuboid::new(1.0, 0.055, 0.40));
        piece(
            &mut commands,
            root,
            ledge,
            stone,
            Transform::from_xyz(0.0, 0.39, 0.0),
        );
        info!(
            "soulbind: memorial {} at {:?}",
            site.id,
            [site.at.x, y, site.at.z]
        );
    }
}

fn glow(
    binding: Res<Binding>,
    real: Res<Time<Real>>,
    runes: Query<(&Rune, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let pulse = 1.0 + 0.08 * (real.elapsed_secs() * 1.4).sin();
    for (rune, handle) in &runes {
        if let Some(material) = materials.get_mut(&handle.0) {
            let strength = if binding.bound == Some(rune.0) {
                0.12
            } else {
                0.025
            } * pulse;
            material.emissive = LinearRgba::new(0.55 * strength, strength, 0.85 * strength, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_rune_is_rgba_with_open_stone_around_its_silhouette() {
        let image = rune_texture();
        assert_eq!((image.width(), image.height()), (1024, 1536));
        let pixels = image.data.as_ref().unwrap();
        let alpha: Vec<_> = pixels.as_chunks::<4>().0.iter().map(|p| p[3]).collect();
        assert_eq!(alpha[0], 0, "no opaque black backing");
        assert_eq!(alpha[1023], 0);
        assert!(alpha.iter().filter(|a| **a == 0).count() > alpha.len() / 3);
        assert!(alpha.iter().any(|a| *a > 250), "solid painted symbol");
    }
}
