//! Sparse background traffic on native Elwynn roads. Original residents, houses,
//! vegetation and wildlife are unchanged. Absent pedestrians follow the saved
//! clock; visible bodies walk the same corners with the normal physical motor.
use crate::avatar::Avatar;
use crate::daycycle::DayClock;
use crate::friendly::Npc;
use crate::motor::MoveIntent;
use crate::settle::Phase;
use crate::world::{SpawnId, WorldBook};
use anyhow::{anyhow, bail, Context, Result};
use benilla_assets::coords::{bevy_to_wow, wow_to_bevy};
use benilla_assets::ModelAnimations;
use bevy::animation::transition::AnimationTransitions;
use bevy::app::AnimationSystems;
use bevy::prelude::*;
use std::time::Duration;
use wow_world::model::{CreatureMeta, Disposition, Spawn, World as WorldData};
use wow_world::village::{Sample, Traffic};
#[cfg(test)]
mod road_tests;
mod scene;
const SCENE: &str = include_str!("../../../data/world/goldshire-roads.tsv");
const ADULT_GUID: u32 = 9_100_000;
const ADULT_ENTRY: u32 = 9_200_000;
const TURN_RATE: f32 = 6.0;
const WALK_SPEED: f32 = 2.5;

#[derive(Resource)]
pub struct VillageScene {
    lives: Traffic,
    day_seconds: f64,
}
#[derive(Component, Debug)]
pub struct Citizen {
    pub guid: u32,
    leg: usize,
    waypoint: usize,
    animation: Option<u16>,
    armed: Option<u16>,
    last_at: Vec3,
    stalled: f32,
    next_trace: f32,
}
impl Citizen {
    pub fn owns_animation(&self) -> bool {
        self.animation.is_some()
    }
    fn at(guid: u32, sample: Sample, at: Vec3) -> Self {
        Self {
            guid,
            leg: sample.leg,
            waypoint: sample.waypoint,
            animation: None,
            armed: None,
            last_at: at,
            stalled: 0.0,
            next_trace: 0.0,
        }
    }
}
pub fn configure(world: &mut WorldData) -> Result<Option<VillageScene>> {
    let setting = std::env::var("WOWSOULS_VILLAGE").ok();
    if setting.as_deref().map(str::trim) == Some("0") {
        return Ok(None);
    }
    let explicit = setting.as_deref().map(str::trim) == Some("1");
    if !explicit && std::env::var_os("WOWSOULS_WORLD_FILE").is_some() {
        return Ok(None);
    }
    if !matches_goldshire(world) {
        if explicit {
            bail!("road traffic requires the original Goldshire world");
        }
        return Ok(None);
    }
    let data = scene::parse(SCENE).context("loading native Goldshire roads")?;
    build_scene(world, &data).map(Some)
}
fn matches_goldshire(world: &WorldData) -> bool {
    [(79647, 465), (80327, 66), (80339, 2046), (81093, 1632)]
        .into_iter()
        .all(|(guid, entry)| {
            world
                .spawns
                .iter()
                .any(|s| s.guid == guid && s.entry == entry && s.area == 87)
                && world.creatures.iter().any(|c| c.entry == entry)
        })
}
fn build_scene(world: &mut WorldData, data: &scene::SceneData) -> Result<VillageScene> {
    let lives =
        Traffic::new(&data.roads, &scene::journeys(data), WALK_SPEED).map_err(|e| anyhow!(e))?;
    let mut staged = world.clone();
    for (i, t) in scene::TRAVELLERS.iter().enumerate() {
        let guid = ADULT_GUID + i as u32;
        let entry = ADULT_ENTRY + i as u32;
        if staged.spawns.iter().any(|s| s.guid == guid)
            || staged.creatures.iter().any(|c| c.entry == entry)
        {
            bail!("road traveller id already used");
        }
        let mut template = staged
            .creatures
            .iter()
            .find(|c| c.entry == t.source)
            .cloned()
            .ok_or_else(|| anyhow!("missing native traveller template {}", t.source))?;
        let weights = staged
            .creature_meta
            .iter()
            .find(|m| m.entry == t.source)
            .map_or([0; 4], |m| m.display_weights);
        template.entry = entry;
        template.name = t.name.into();
        template.subname = t.occupation.into();
        template.npc_flags = 0;
        template.vendor_id = 0;
        template.trainer_type = 0;
        template.trainer_class = 0;
        template.equipment_id = 0;
        template.loot_id = 0;
        template.civilian = true;
        template.speed_walk = 1.0;
        staged.creatures.push(template);
        staged.creature_meta.push(CreatureMeta {
            entry,
            display_weights: weights,
            reaction: Some(Disposition::Friendly),
        });
        staged.spawns.push(Spawn {
            guid,
            entry,
            alt: [0; 4],
            pos: data.anchors[t.stops[0]],
            orientation: 0.0,
            respawn_min: 300,
            respawn_max: 300,
            wander: 0.0,
            movement: 0,
            area: 87,
            event: false,
        });
    }
    let day_seconds = std::env::var("WOWSOULS_DAY_MINUTES")
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .map_or(1440.0, |v| v * 60.0);
    *world = staged;
    info!("village: 6 road travellers, original residents and scenery preserved");
    Ok(VillageScene { lives, day_seconds })
}
fn clock_seconds(scene: &VillageScene, clock: &DayClock) -> f64 {
    (clock.days as f64 + clock.minute / 1440.0) * scene.day_seconds
}
pub fn plugin(app: &mut App) {
    app.add_systems(
        PreUpdate,
        (
            prepare.before(crate::world::ring_tick),
            attach.after(crate::world::ring_tick),
        )
            .run_if(in_state(Phase::Playing))
            .run_if(resource_exists::<VillageScene>)
            .run_if(resource_exists::<WorldBook>),
    );
    app.add_systems(
        Update,
        drive
            .before(crate::walkers::move_walkers)
            .run_if(in_state(Phase::Playing))
            .run_if(resource_exists::<VillageScene>),
    );
    app.add_systems(
        PostUpdate,
        emote
            .after(crate::actor::anim::drive)
            .before(AnimationSystems)
            .run_if(in_state(Phase::Playing))
            .run_if(resource_exists::<VillageScene>),
    );
}
fn prepare(
    scene: Res<VillageScene>,
    clock: Res<DayClock>,
    mut wb: ResMut<WorldBook>,
    live: Query<&Transform, With<Npc>>,
) {
    let now = clock_seconds(&scene, &clock);
    let wb = &mut *wb;
    for (index, spot) in wb.book.spots.iter_mut().enumerate() {
        if !scene.lives.contains(spot.guid) {
            continue;
        }
        if let Some(xf) = wb.entity[index].and_then(|entity| live.get(entity).ok()) {
            wb.book.recs[index].at = [xf.translation.x, xf.translation.z];
        } else if let Some(sample) = scene.lives.sample(spot.guid, now) {
            spot.wow = sample.position;
            spot.facing = sample.heading;
            let at = wow_to_bevy(sample.position);
            wb.book.recs[index].at = [at.x, at.z];
        }
    }
}
fn attach(
    mut commands: Commands,
    scene: Res<VillageScene>,
    clock: Res<DayClock>,
    wb: Res<WorldBook>,
    newcomers: Query<(Entity, &SpawnId, &Transform), (With<Npc>, Without<Citizen>)>,
) {
    let now = clock_seconds(&scene, &clock);
    for (entity, spawn, xf) in &newcomers {
        let Some(spot) = wb.book.spots.get(spawn.0) else {
            continue;
        };
        if let Some(sample) = scene.lives.sample(spot.guid, now) {
            commands
                .entity(entity)
                .insert(Citizen::at(spot.guid, sample, xf.translation));
            if trace_on() {
                info!(
                    "village attach: guid {} moving {} actual {:?} expected {:?}",
                    spot.guid,
                    sample.moving,
                    bevy_to_wow(xf.translation),
                    sample.position
                );
            }
        }
    }
}
fn trace_on() -> bool {
    crate::actor::anim::trace_on() || std::env::var_os("WOWSOULS_VILLAGE_TRACE").is_some()
}
/// Visible bodies finish every pending corner, even if the schedule is ahead.
fn advance_cursor(
    scene: &VillageScene,
    citizen: &mut Citizen,
    desired: Sample,
    at: Vec3,
) -> Option<Vec3> {
    for _ in 0..32 {
        let points = scene.lives.waypoints(citizen.guid, citizen.leg)?;
        while citizen.waypoint < points.len() {
            let target = wow_to_bevy(points[citizen.waypoint]);
            let delta = target - at;
            if Vec2::new(delta.x, delta.z).length() <= 0.16 && delta.y.abs() <= 0.35 {
                citizen.waypoint += 1;
            } else {
                if citizen.leg == desired.leg && citizen.waypoint > desired.waypoint {
                    return None;
                }
                return Some(target);
            }
        }
        if citizen.leg == desired.leg {
            return None;
        }
        let next = citizen.leg + 1;
        citizen.leg = if scene.lives.waypoints(citizen.guid, next).is_some() {
            next
        } else {
            0
        };
        citizen.waypoint = 0;
    }
    None
}
fn drive(
    time: Res<Time>,
    scene: Res<VillageScene>,
    clock: Res<DayClock>,
    world: Option<Res<WorldBook>>,
    avatar: Query<&Transform, (With<Avatar>, Without<Citizen>)>,
    mut citizens: Query<(
        &mut Transform,
        &Npc,
        &mut MoveIntent,
        &mut Citizen,
        Option<&ModelAnimations>,
    )>,
    mut previous: Local<Option<DayClock>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let elapsed = clock_seconds(&scene, &clock);
    let running = previous.is_some_and(|old| old != *clock);
    *previous = Some(*clock);
    let Some(player) = avatar.iter().next().map(|xf| xf.translation) else {
        return;
    };
    let now = time.elapsed_secs();
    for (mut xf, npc, mut intent, mut citizen, anims) in &mut citizens {
        *intent = MoveIntent::default();
        citizen.animation = None;
        let Some(sample) = scene.lives.sample(citizen.guid, elapsed) else {
            continue;
        };
        let distance = Vec2::new(player.x - xf.translation.x, player.z - xf.translation.z).length();
        let reach = world
            .as_ref()
            .map_or(crate::ring::RING.despawn, |w| w.ring.despawn);
        if distance > reach {
            continue;
        }
        let target = advance_cursor(&scene, &mut citizen, sample, xf.translation);
        if running {
            if let Some(target) = target {
                let delta = Vec3::new(
                    target.x - xf.translation.x,
                    0.0,
                    target.z - xf.translation.z,
                );
                if delta.length_squared() > 1e-6 {
                    *intent = MoveIntent {
                        dir: delta.normalize_or_zero(),
                        speed: npc.walk.min(delta.length() / dt.max(0.001)),
                        jump: false,
                    };
                    let current = xf.rotation.to_euler(EulerRot::YXZ).0;
                    xf.rotation = Quat::from_rotation_y(crate::controller::turn_toward(
                        current,
                        crate::controller::yaw_of(delta),
                        TURN_RATE * dt,
                    ));
                }
            }
        }
        if let (None, Some(anims)) = (target, anims) {
            citizen.animation = (!sample.moving
                && (now + (citizen.guid % 11) as f32) % 12.0 < 3.0
                && anims.find(60).is_some())
            .then_some(60);
        }
        let progress = Vec2::new(
            xf.translation.x - citizen.last_at.x,
            xf.translation.z - citizen.last_at.z,
        )
        .length();
        if running && target.is_some() && progress < dt * 0.08 {
            citizen.stalled += dt;
        } else {
            citizen.stalled = 0.0;
        }
        citizen.last_at = xf.translation;
        if citizen.stalled >= 8.0 && now >= citizen.next_trace {
            warn!("village stuck: guid {} leg {} waypoint {} actual {:?} target {:?}, stalled {:.1} s",citizen.guid,citizen.leg,citizen.waypoint,bevy_to_wow(xf.translation),target.map(bevy_to_wow),citizen.stalled);
            citizen.next_trace = now + 10.0;
        } else if trace_on() && now >= citizen.next_trace {
            info!("village: guid {} moving {} leg {} waypoint {} actual {:?} expected {:?} walking {} progress {:.3} stalled {:.1}",citizen.guid,sample.moving,citizen.leg,citizen.waypoint,bevy_to_wow(xf.translation),sample.position,intent.speed>0.0,progress,citizen.stalled);
            citizen.next_trace = now + 5.0;
        }
    }
}
fn emote(
    mut citizens: Query<(
        &mut Citizen,
        &ModelAnimations,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
) {
    for (mut citizen, anims, mut player, mut transitions) in &mut citizens {
        if let Some(id) = citizen.animation {
            if let Some(clip) = anims.find(id).filter(|_| citizen.armed != Some(id)) {
                transitions
                    .play(&mut player, clip.node, Duration::from_secs_f32(0.2))
                    .repeat();
            }
        }
        citizen.armed = citizen.animation;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::village::{Journey, RoadGraph};
    #[test]
    fn road_addition_preserves_every_original_record_and_adds_six_service_free_civilians() {
        let mut world =
            wow_world::manifest::parse(include_str!("../../../data/world/elwynn-westfall.txt"))
                .unwrap();
        let original_spawns = world.spawns.clone();
        let original_creatures = world.creatures.clone();
        let data = scene::parse(SCENE).unwrap();
        let scene = build_scene(&mut world, &data).unwrap();
        assert_eq!(world.spawns.len(), original_spawns.len() + 6);
        assert_eq!(
            format!("{:?}", &world.spawns[..original_spawns.len()]),
            format!("{original_spawns:?}")
        );
        assert_eq!(
            format!("{:?}", &world.creatures[..original_creatures.len()]),
            format!("{original_creatures:?}")
        );
        assert_eq!(scene.lives.guids().count(), 6);
        assert!(world
            .creatures
            .iter()
            .filter(|c| c.entry >= ADULT_ENTRY)
            .all(|c| c.npc_flags == 0 && c.vendor_id == 0 && c.loot_id == 0));
    }
    #[test]
    fn visible_cursor_follows_corners_and_does_not_teleport_when_late() {
        let roads = RoadGraph {
            nodes: vec![[0.0; 3], [20.0, 0.0, 0.0], [20.0, 20.0, 0.0]],
            edges: vec![[0, 1], [1, 2]],
        };
        let lives = Traffic::new(
            &roads,
            &[Journey {
                guid: 1,
                stops: vec![roads.nodes[0], roads.nodes[2]],
                pause_seconds: 5.0,
                phase: 0.0,
            }],
            2.5,
        )
        .unwrap();
        let scene = VillageScene {
            lives,
            day_seconds: 1440.0,
        };
        let sample = scene.lives.sample(1, 15.0).unwrap();
        let mut body = Citizen::at(1, sample, Vec3::ZERO);
        body.waypoint = 0;
        assert_eq!(
            advance_cursor(&scene, &mut body, sample, Vec3::ZERO),
            Some(wow_to_bevy(roads.nodes[1]))
        );
        assert_eq!(
            advance_cursor(&scene, &mut body, sample, wow_to_bevy(roads.nodes[1])),
            Some(wow_to_bevy(roads.nodes[2]))
        );
    }
    #[test]
    fn midnight_keeps_absolute_route_time() {
        let data = scene::parse(SCENE).unwrap();
        let scene = VillageScene {
            lives: Traffic::new(&data.roads, &scene::journeys(&data), WALK_SPEED).unwrap(),
            day_seconds: 1440.0,
        };
        let a = DayClock {
            minute: 1439.99,
            days: 3,
        };
        let b = DayClock {
            minute: 0.01,
            days: 4,
        };
        for guid in scene.lives.guids() {
            let a = scene.lives.sample(guid, clock_seconds(&scene, &a)).unwrap();
            let b = scene.lives.sample(guid, clock_seconds(&scene, &b)).unwrap();
            assert!(Vec3::from_array(a.position).distance(Vec3::from_array(b.position)) < 0.06);
        }
    }
}
