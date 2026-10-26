//! Quest objects in the world: gatherable quest chests (`Milly's Harvest`, `Bundle of Wood`,
//! `Dead-tooth's Strongbox`, `Stolen Books`), quest-giver/ender objects (`A half-eaten body`,
//! `Rolf's corpse`, `Wanted Poster`), and the Mirror Lake waterfall spell focus (`174794`).
//! They stand where the database puts them, appear in a ring around the player as the creatures do,
//! and are gathered or spoken with via the talk/interact key (`X` / pad `Y` / `T`) within [`REACH`].

use std::collections::{BTreeMap, BTreeSet};

use benilla_assets::LockRecover;
use benilla_world::collision::WorldCollision;
use bevy::prelude::*;
use wow_world::model::LootKind;
use wow_world::quest::rules::{marks, Mark};
use wow_world::quest::Npc as QuestNpc;

use super::QuestGame;
use crate::actor::{ActorModel, Catalogs, Look};
use crate::avatar::Avatar;
use crate::loot::Data;
use crate::settle::{snap_to_ground, Phase};

/// The grape crate's object entry (type 3, loot 10119 -> item 11119).
pub const CRATE_ENTRY: u32 = 161_557;
/// Mirror Lake waterfall spell-focus object for Mage quest 1861 (`Mirror Lake` -> item 7206).
pub const MIRROR_LAKE_WATERWALL: u32 = 174_794;
pub const MIRROR_LAKE_SAMPLE: u32 = 7206;
/// `GameObjectDisplayInfo` 3012: `GrapeBucket02` (the client names it `.mdx`).
const CRATE_MODEL: &str = "World/Azeroth/Elwynn/PassiveDoodads/GrapeBuckets/GrapeBucket02.m2";
const CRATE_DIR: &str = "World/Azeroth/Elwynn/PassiveDoodads/GrapeBuckets";
/// A quest object appears inside this ring (yd) and goes beyond the next.
const SPAWN: f32 = 100.0;
const DESPAWN: f32 = 130.0;
/// How near the player must be to gather or inspect (yd, ground distance).
pub const REACH: f32 = 3.5;

#[derive(Debug, Clone)]
pub struct Spot {
    pub entry: u32,
    pub name: String,
    pub at: Vec3,
    pub yaw: f32,
    pub scale: f32,
    pub model: Option<(String, String)>,
    /// The database's respawn time (s).
    pub respawn: f32,
    /// `gameobject_template.data1`: the loot table (`0` for quest-giver objects and spell foci).
    pub loot: u32,
    /// Quest items this object can yield when gathered.
    pub quest_items: Vec<u32>,
    /// True when this object starts or ends quests (`Npc::Object(entry)`).
    pub giver_or_ender: bool,
}

#[derive(Resource, Default)]
pub struct Objects {
    pub spots: Vec<Spot>,
    live: Vec<Option<Entity>>,
    /// Real second a gathered object returns at.
    until: Vec<f32>,
    ready: bool,
}

/// One quest object in the world: its place in [`Objects::spots`].
#[derive(Component, Debug, Clone, Copy)]
pub struct QuestObject(pub usize);

impl Objects {
    /// A quest object was gathered: it returns after its respawn time.
    pub fn gathered(&mut self, index: usize, now: f32) {
        if let Some(live) = self.live.get_mut(index) {
            *live = None;
        }
        if let (Some(until), Some(spot)) = (self.until.get_mut(index), self.spots.get(index)) {
            *until = now + spot.respawn.max(1.0);
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Objects>()
        .add_systems(Update, ring.run_if(in_state(Phase::Playing)));
}

/// The gatherable quest object nearest to `me` within [`REACH`] whose quest item is in `eligibility`.
pub fn nearest_gatherable(
    crates: &Query<(Entity, &Transform, &QuestObject), Without<Avatar>>,
    objects: &Objects,
    me: Vec3,
    eligibility: &BTreeSet<u32>,
) -> Option<(Entity, usize)> {
    if eligibility.is_empty() {
        return None;
    }
    crates
        .iter()
        .filter_map(|(e, xf, o)| {
            let spot = objects.spots.get(o.0)?;
            if !spot.quest_items.iter().any(|i| eligibility.contains(i)) {
                return None;
            }
            let d = Vec2::new(me.x - xf.translation.x, me.z - xf.translation.z).length();
            (d <= REACH).then_some((d, e, o.0))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, e, i)| (e, i))
}

/// The quest-giver/ender object nearest to `me` within [`REACH`] that has a non-`None` quest mark.
pub fn nearest_talkable(
    crates: &Query<(Entity, &Transform, &QuestObject), Without<Avatar>>,
    objects: &Objects,
    game: &QuestGame,
    me: Vec3,
) -> Option<(f32, usize, Mark)> {
    crates
        .iter()
        .filter_map(|(_, xf, o)| {
            let spot = objects.spots.get(o.0)?;
            if !spot.giver_or_ender {
                return None;
            }
            let d = Vec2::new(me.x - xf.translation.x, me.z - xf.translation.z).length();
            if d > REACH {
                return None;
            }
            let mark = marks(
                &game.book,
                &game.log,
                &game.player,
                QuestNpc::Object(spot.entry),
            );
            (mark != Mark::None).then_some((d, o.0, mark))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

#[allow(clippy::too_many_arguments)]
fn ring(
    mut commands: Commands,
    data: Option<Res<Data>>,
    catalogs: Option<Res<Catalogs>>,
    real: Res<Time<Real>>,
    assets: Res<AssetServer>,
    collide: WorldCollision,
    avatar: Query<&Transform, With<Avatar>>,
    mut objects: ResMut<Objects>,
) {
    if !crate::world::world_on() {
        return;
    }
    let Some(data) = data else { return };
    if !objects.ready {
        objects.ready = true;
        let go_catalog = catalogs.as_ref().and_then(|c| {
            let mut chain = c.chain.lock_recover();
            benilla_formats::load_gameobject_catalog(&mut chain).ok()
        });
        let giver_objects: BTreeSet<u32> = data
            .0
            .starts
            .iter()
            .chain(&data.0.ends)
            .filter(|r| r.object)
            .map(|r| r.npc)
            .collect();
        let mut templates: BTreeMap<u32, Spot> = BTreeMap::new();
        for o in &data.0.objects {
            let giver_or_ender = giver_objects.contains(&o.entry);
            let loot = if o.kind == 3 {
                o.data[1].max(0) as u32
            } else {
                0
            };
            let mut quest_items: Vec<u32> = if loot != 0 {
                data.0
                    .loot
                    .iter()
                    .filter(|l| l.kind == LootKind::Object && l.table == loot && l.chance < 0.0)
                    .map(|l| l.item)
                    .collect()
            } else {
                Vec::new()
            };
            if o.entry == MIRROR_LAKE_WATERWALL {
                quest_items.push(MIRROR_LAKE_SAMPLE);
            }
            if !giver_or_ender && quest_items.is_empty() {
                continue;
            }
            let model = if o.display == 0 {
                None
            } else if let Some(raw) = go_catalog.as_ref().and_then(|c| c.model_path(o.display)) {
                let norm = raw.replace('\\', "/");
                let dir = norm.rsplit_once('/').map_or("", |(d, _)| d).to_string();
                Some((raw.to_string(), dir))
            } else if o.entry == CRATE_ENTRY {
                Some((CRATE_MODEL.to_string(), CRATE_DIR.to_string()))
            } else {
                None
            };
            templates.insert(
                o.entry,
                Spot {
                    entry: o.entry,
                    name: o.name.clone(),
                    at: Vec3::ZERO,
                    yaw: 0.0,
                    scale: if o.size > 0.0 { o.size } else { 1.0 },
                    model,
                    respawn: 180.0,
                    loot,
                    quest_items,
                    giver_or_ender,
                },
            );
        }
        let spots: Vec<Spot> = data
            .0
            .object_spawns
            .iter()
            .filter(|s| !s.event)
            .filter_map(|s| {
                let tpl = templates.get(&s.entry)?;
                Some(Spot {
                    at: benilla_assets::coords::wow_to_bevy(s.pos),
                    yaw: s.orientation,
                    respawn: s.respawn_max.max(s.respawn_min) as f32,
                    ..tpl.clone()
                })
            })
            .collect();
        info!(
            "quests: {} quest object spawns across {} templates",
            spots.len(),
            templates.len()
        );
        objects.live = vec![None; spots.len()];
        objects.until = vec![0.0; spots.len()];
        objects.spots = spots;
    }
    let Ok(me) = avatar.single() else { return };
    let now = real.elapsed_secs();
    for i in 0..objects.spots.len() {
        let spot = objects.spots[i].clone();
        let d = Vec2::new(me.translation.x - spot.at.x, me.translation.z - spot.at.z).length();
        match objects.live[i] {
            None if d <= SPAWN && now >= objects.until[i] => {
                let mut at = spot.at;
                let Some(y) = snap_to_ground(&collide, at) else {
                    // The ground is not there yet: try again in a moment.
                    objects.until[i] = now + 0.5;
                    continue;
                };
                at.y = y;
                let mut ent = commands.spawn((
                    Name::new(spot.name.clone()),
                    Transform::from_translation(at).with_rotation(Quat::from_rotation_y(spot.yaw)),
                    Visibility::default(),
                    QuestObject(i),
                ));
                if let Some((model_path, dir)) = spot.model {
                    ent.insert(ActorModel {
                        m2: assets.load(benilla_assets::m2_url(&model_path)),
                        dir,
                        look: Look::Creature {
                            skins: [None, None, None],
                        },
                        scale: spot.scale,
                        display: 0,
                        worn: None,
                    });
                }
                objects.live[i] = Some(ent.id());
            }
            Some(e) if d > DESPAWN => {
                commands.entity(e).despawn();
                objects.live[i] = None;
            }
            _ => {}
        }
    }
}
