//! Walk the six road routes through original WoW collision in both directions.
//! This deliberately uses the real capsule/motor and the runtime cursor's arrival
//! tolerances: a clear-looking doorway is insufficient if its steps block bodies.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use avian3d::prelude::*;
use benilla_assets::coords::{placement_rotation, wmo_doodad_local, wow_to_bevy};
use benilla_formats::{Chain, CollisionMesh, TileMesh, WmoInstance};
use benilla_world::collision::{MoverTraceExclusions, WorldCollision};
use benilla_world::terrain_stream::placement_collider_data;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use crate::motor::{self, Motor, MoveIntent};

const DT: f32 = 1.0 / 60.0;
const ARRIVE_XZ: f32 = 0.16;
const ARRIVE_Y: f32 = 0.35;

#[derive(Clone, Copy)]
struct Footprint {
    min: Vec2,
    max: Vec2,
}

impl Footprint {
    fn around(points: &[[f32; 3]], margin: f32) -> Self {
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for point in points {
            let xy = Vec2::new(point[0], point[1]);
            min = min.min(xy);
            max = max.max(xy);
        }
        Self {
            min: min - Vec2::splat(margin),
            max: max + Vec2::splat(margin),
        }
    }

    fn contains(self, point: [f32; 3], margin: f32) -> bool {
        let point = Vec2::new(point[0], point[1]);
        point.cmpge(self.min - Vec2::splat(margin)).all()
            && point.cmple(self.max + Vec2::splat(margin)).all()
    }

    fn intersects(self, points: &[[f32; 3]]) -> bool {
        let other = Self::around(points, 0.0);
        self.min.cmple(other.max).all() && other.min.cmple(self.max).all()
    }
}

fn collision_world() -> App {
    let mut app = App::new();
    app.init_resource::<MoverTraceExclusions>();
    app.add_plugins((
        MinimalPlugins,
        bevy::transform::TransformPlugin,
        bevy::asset::AssetPlugin::default(),
        bevy::scene::ScenePlugin,
        PhysicsPlugins::new(bevy::app::PostUpdate),
    ));
    app.init_asset::<Mesh>();
    app
}

fn add_hull(app: &mut App, hull: &CollisionMesh, transform: Transform, label: String) {
    if let Some((positions, triangles)) = placement_collider_data(Some(hull), &transform) {
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::trimesh(positions, triangles),
            Transform::IDENTITY,
            Name::new(label),
        ));
    }
}

fn placement(wmo: &WmoInstance) -> Transform {
    Transform {
        translation: wow_to_bevy(wmo.position),
        rotation: placement_rotation(wmo.rotation),
        scale: Vec3::ONE,
    }
}

fn m2_hull<'a>(
    chain: &mut Chain,
    cache: &'a mut BTreeMap<String, CollisionMesh>,
    model: &str,
) -> &'a CollisionMesh {
    cache.entry(model.to_ascii_lowercase()).or_insert_with(|| {
        // The native asset streamer retires unavailable props; reproduce that
        // behavior rather than adding collision for an object the game cannot load.
        benilla_formats::load_m2_collision_hull(chain, model).unwrap_or_else(|error| {
            eprintln!("native road fixture: unavailable prop {model}: {error:#}");
            CollisionMesh::default()
        })
    })
}

fn add_wmo(
    app: &mut App,
    chain: &mut Chain,
    wmo_hulls: &mut BTreeMap<String, CollisionMesh>,
    m2_hulls: &mut BTreeMap<String, CollisionMesh>,
    wmo: &WmoInstance,
) {
    let transform = placement(wmo);
    let hull = wmo_hulls
        .entry(wmo.model.to_ascii_lowercase())
        .or_insert_with(|| {
            let hull = benilla_formats::load_wmo_collision_tris(chain, &wmo.model).unwrap_or_else(
                |error| panic!("native {} walking collision: {error:#}", wmo.model),
            );
            assert!(
                !hull.is_empty(),
                "native {} has no walking faces",
                wmo.model
            );
            hull
        });
    add_hull(
        app,
        hull,
        transform,
        format!("WMO {} {}", wmo.unique_id, wmo.model),
    );

    let bytes = chain.read_file(&wmo.model).expect("native WMO root");
    let root = benilla_formats::parse_wmo_root(&bytes).expect("parse native WMO root");
    let mut indices = BTreeSet::new();
    // Match native spawning: always-on set zero plus the MODF-selected set.
    for set in [0, usize::from(wmo.doodad_set)] {
        if let Some(range) = root.doodad_sets().get(set) {
            indices.extend(range.start..range.start + range.count);
        }
    }
    for index in indices {
        let Some(doodad) = root.doodads().get(index as usize) else {
            continue;
        };
        if doodad.model.is_empty() {
            continue;
        }
        let local = wmo_doodad_local(doodad.position, doodad.orientation, doodad.scale);
        let hull = m2_hull(chain, m2_hulls, &doodad.model);
        add_hull(
            app,
            hull,
            transform.mul_transform(local),
            format!("WMO {} prop {index} {}", wmo.unique_id, doodad.model),
        );
    }
}

fn add_terrain(app: &mut App, tile: &TileMesh, footprints: &[Footprint]) {
    let mut hull = CollisionMesh::default();
    for chunk in &tile.chunks {
        if !footprints
            .iter()
            .any(|bounds| bounds.intersects(&chunk.positions))
        {
            continue;
        }
        let base = hull.positions.len() as u32;
        hull.positions.extend_from_slice(&chunk.positions);
        hull.indices
            .extend(chunk.indices.iter().map(|index| base + index));
    }
    // Hole-masked, source terrain only. There is deliberately no synthetic floor.
    let origin = tile
        .chunks
        .first()
        .and_then(|chunk| chunk.positions.first());
    add_hull(
        app,
        &hull,
        Transform::IDENTITY,
        format!("ADT terrain {origin:?}"),
    );
}

fn arrived(feet: Vec3, target: Vec3) -> bool {
    let delta = target - feet;
    Vec2::new(delta.x, delta.z).length() <= ARRIVE_XZ && delta.y.abs() <= ARRIVE_Y
}

fn source_support(world: &WorldCollision<'_, '_>, capsule: &Collider, feet: Vec3) -> bool {
    world
        .cast_mover(
            capsule,
            feet + Vec3::Y * (motor::CAPSULE_HEIGHT * 0.5),
            Vec3::NEG_Y,
            0.02,
        )
        .is_some()
}

fn walk_route(app: &mut App, label: String, path: &[[f32; 3]]) {
    eprintln!("native route {label}: checking");
    assert!(!path.is_empty(), "{label} has no measured path");
    let points: Vec<_> = path.iter().copied().map(wow_to_bevy).collect();
    app.world_mut()
        .run_system_once(move |world: WorldCollision, names: Query<&Name>| {
            let capsule = crate::motor::testkit::player_capsule();
            let mut motor = Motor::default();
            let mut feet = points[0];
            let dt = Duration::from_secs_f32(DT);
            // Start once on the authored road. All later positions come from the
            // motor; the return walk continues from the actual interior arrival.
            for _ in 0..60 {
                motor::step(&mut motor, &mut feet, dt, &world, &capsule, MoveIntent::default());
            }
            assert!(arrived(feet, points[0]), "{label} road is not supported: {feet:?}");
            assert!(motor.grounded, "{label} road has no source ground");

            for (direction, targets) in [
                ("enter", points.clone()),
                ("leave", points.iter().rev().copied().collect()),
            ] {
                let length: f32 = targets.windows(2).map(|p| p[0].distance(p[1])).sum();
                let max_frames = ((length / motor::WALK_SPEED * 3.0 + 30.0) / DT) as usize;
                let mut cursor = 0;
                let mut stalled = 0;
                let mut unsupported = 0;
                let mut frames = 0;
                while cursor < targets.len() && frames < max_frames {
                    while cursor < targets.len() && arrived(feet, targets[cursor]) {
                        cursor += 1;
                    }
                    if cursor == targets.len() {
                        break;
                    }
                    let target = targets[cursor];
                    let delta = Vec3::new(target.x - feet.x, 0.0, target.z - feet.z);
                    let intent = MoveIntent {
                        dir: delta.normalize_or_zero(),
                        speed: motor::WALK_SPEED.min(delta.length() / DT),
                        jump: false,
                    };
                    let previous = feet;
                    let outcome = motor::step(&mut motor, &mut feet, dt, &world, &capsule, intent);
                    assert!(!outcome.jumped, "{label} {direction} used a jump");
                    assert!(feet.is_finite(), "{label} {direction} produced nonfinite feet");
                    let movement = feet - previous;
                    assert!(
                        Vec2::new(movement.x, movement.z).length() <= 0.25,
                        "{label} {direction} jumped horizontally: {previous:?} -> {feet:?}"
                    );
                    assert!(
                        movement.y.abs() <= 1.1,
                        "{label} {direction} jumped vertically: {previous:?} -> {feet:?}"
                    );
                    stalled = if movement.length() < 0.001 { stalled + 1 } else { 0 };
                    if stalled == 120 {
                        let center = feet + Vec3::Y * (motor::CAPSULE_HEIGHT * 0.5);
                        for (probe, from, travel) in [
                            ("forward", center, delta.normalize_or_zero() * 0.6),
                            ("raised", center + Vec3::Y * 0.5, delta.normalize_or_zero() * 0.6),
                            ("ground", center, Vec3::NEG_Y),
                        ] {
                            if let Some(hit) = world.cast_mover(&capsule, from, travel, 0.02) {
                                eprintln!("{label} {direction} waypoint {cursor} {probe}: {} distance={} raw={} normal={:?} point={:?} feet={feet:?} target={target:?}",
                                    names.get(hit.entity).map_or("unnamed collider", Name::as_str),
                                    hit.distance, hit.collision_distance, hit.normal1, hit.point1);
                            } else {
                                eprintln!("{label} {direction} waypoint {cursor} {probe}: clear");
                            }
                        }
                    }
                    unsupported = if outcome.grounded || source_support(&world, &capsule, feet) {
                        0
                    } else {
                        unsupported + 1
                    };
                    assert!(
                        stalled < 480,
                        "{label} {direction} blocked at waypoint {cursor}/{}: actual {feet:?}, target {target:?}",
                        targets.len()
                    );
                    assert!(
                        unsupported < 30,
                        "{label} {direction} left source ground at waypoint {cursor}: {feet:?}"
                    );
                    frames += 1;
                }
                assert_eq!(
                    cursor, targets.len(),
                    "{label} {direction} timed out: actual {feet:?}, target {:?}",
                    targets.get(cursor)
                );
                assert!(arrived(feet, *targets.last().unwrap()), "{label} {direction} missed arrival");
                assert!(source_support(&world, &capsule, feet), "{label} {direction} ended unsupported");
                eprintln!("native route {label}: {direction} {} corners, {frames} frames", targets.len());
            }
        })
        .unwrap();
}

#[test]
fn all_six_road_routes_are_physically_walkable_both_ways() {
    let Some(data) = benilla_formats::wow_data() else {
        eprintln!("skipped native road traversal: no WoW install (set WOW_DATA)");
        return;
    };
    let scene = super::scene::parse(super::SCENE).expect("packaged native roads");
    let road_routes: Vec<_> = super::scene::TRAVELLERS
        .iter()
        .map(|t| {
            (
                t.name.to_string(),
                scene
                    .roads
                    .shortest_path(scene.anchors[t.stops[0]], scene.anchors[t.stops[1]])
                    .unwrap(),
            )
        })
        .collect();
    let footprints: Vec<_> = road_routes
        .iter()
        .flat_map(|(_, path)| path.windows(2).map(|edge| Footprint::around(edge, 5.0)))
        .collect();
    let mut tiles = BTreeSet::new();
    for bounds in &footprints {
        let a = benilla_formats::world_to_tile(bounds.min.x, bounds.min.y);
        let b = benilla_formats::world_to_tile(bounds.max.x, bounds.max.y);
        for x in a.0.min(b.0)..=a.0.max(b.0) {
            for y in a.1.min(b.1)..=a.1.max(b.1) {
                tiles.insert((x, y));
            }
        }
    }
    let mut chain = benilla_formats::open_chain(&data).unwrap();
    let mut app = collision_world();
    let mut wmo_hulls = BTreeMap::new();
    let mut m2_hulls = BTreeMap::new();
    let mut seen_wmos = BTreeSet::new();
    let mut seen_doodads = BTreeSet::new();
    for (x, y) in tiles {
        let tile = benilla_formats::load_tile_mesh(&mut chain, "Azeroth", x, y).unwrap();
        add_terrain(&mut app, &tile, &footprints);
        // Include complete native WMOs: a city wall may cross the route even
        // though its placement origin is hundreds of yards away.
        for wmo in &tile.wmos {
            if seen_wmos.insert(wmo.unique_id) {
                add_wmo(&mut app, &mut chain, &mut wmo_hulls, &mut m2_hulls, wmo);
            }
        }
        for doodad in &tile.doodads {
            if !seen_doodads.insert(doodad.unique_id)
                || !footprints.iter().any(|b| b.contains(doodad.position, 60.0))
            {
                continue;
            }
            let hull = m2_hull(&mut chain, &mut m2_hulls, &doodad.model);
            add_hull(
                &mut app,
                hull,
                Transform {
                    translation: wow_to_bevy(doodad.position),
                    rotation: placement_rotation(doodad.rotation),
                    scale: Vec3::splat(doodad.scale),
                },
                format!("native doodad {} {}", doodad.unique_id, doodad.model),
            );
        }
    }
    app.update();
    for (label, path) in road_routes {
        walk_route(&mut app, label, &path);
    }
}

#[test]
fn a_native_goldshire_slope_does_not_lock_the_motor_at_large_coordinates() {
    assert_native_slope(
        [
            Vec3::new(-52.082684, 56.70405, 9502.083),
            Vec3::new(-54.166668, 57.06188, 9504.167),
            Vec3::new(-50.0, 56.845947, 9504.167),
        ],
        Vec3::new(-53.180923, 56.899284, 9503.236),
        Vec3::new(-0.122077, 0.0, 0.243164).normalize(),
    );
    // GJK can also approximate an interior face normal after query rebasing.
    assert_native_slope(
        [
            Vec3::new(-143.74934, 56.5394, 9481.249),
            Vec3::new(-145.83333, 56.484364, 9479.167),
            Vec3::new(-145.83333, 56.567905, 9483.333),
        ],
        Vec3::new(-145.27151, 56.550716, 9482.298),
        Vec3::new(0.47951, 0.0, -0.006).normalize(),
    );
}

fn assert_native_slope(vertices: [Vec3; 3], feet: Vec3, direction: Vec3) {
    let mut displacements = Vec::new();
    for origin in [Vec3::ZERO, vertices[0]] {
        let mut app = collision_world();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::trimesh(
                vertices.iter().map(|v| *v - origin).collect(),
                vec![[0, 1, 2]],
            ),
            Transform::IDENTITY,
        ));
        app.update();
        let displacement = app
            .world_mut()
            .run_system_once(move |world: WorldCollision| {
                let mut at = feet - origin;
                let mut motor = Motor::default();
                let capsule = crate::motor::testkit::player_capsule();
                let center = at + Vec3::Y * (motor::CAPSULE_HEIGHT * 0.5);
                let hit = world
                    .cast_mover(&capsule, center, Vec3::NEG_Y, 0.02)
                    .unwrap();
                assert!(
                    hit.point1.distance(hit.point2) < 0.05,
                    "witnesses must share the world frame: {hit:?}"
                );
                for _ in 0..10 {
                    motor::step(
                        &mut motor,
                        &mut at,
                        Duration::from_secs_f32(DT),
                        &world,
                        &capsule,
                        MoveIntent {
                            dir: direction,
                            speed: 2.5,
                            jump: false,
                        },
                    );
                }
                at - (feet - origin)
            })
            .unwrap();
        assert!(
            displacement.dot(direction) > 0.38,
            "shallow native floor blocked movement: {displacement:?}"
        );
        displacements.push(displacement);
    }
    assert!(
        displacements[0].distance(displacements[1]) < 0.01,
        "translation changed floor movement: {displacements:?}"
    );
}
