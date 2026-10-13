//! Quest objects in the world: Milly's Harvest, the forty grape crates of the Northshire Vineyards that
//! "Milly's Harvest" (quest 3904) sends the player to gather eight of. They stand where the database
//! puts them, appear in a ring around the player as the creatures do, and are gathered with the talk
//! key (`T`) within [`REACH`]: the crate's own loot table (`gameobject_loot_template`) is rolled with
//! the quest log's eligibility (a quest item drops only while a quest still wants it), the crate
//! vanishes and comes back after its database respawn time (180 s).

use benilla_world::collision::WorldCollision;
use bevy::prelude::*;

use crate::actor::{ActorModel, Look};
use crate::avatar::Avatar;
use crate::loot::Data;
use crate::settle::{snap_to_ground, Phase};

/// The grape crate's object entry (type 3, loot 10119 -> item 11119).
pub const CRATE_ENTRY: u32 = 161_557;
/// `GameObjectDisplayInfo` 3012: `GrapeBucket02` (the client names it `.mdx`).
const CRATE_MODEL: &str = "World/Azeroth/Elwynn/PassiveDoodads/GrapeBuckets/GrapeBucket02.m2";
const CRATE_DIR: &str = "World/Azeroth/Elwynn/PassiveDoodads/GrapeBuckets";
/// A crate appears inside this ring (yd) and goes beyond the next.
const SPAWN: f32 = 100.0;
const DESPAWN: f32 = 130.0;
/// How near the player must be to gather (yd, ground distance).
pub const REACH: f32 = 3.0;

#[derive(Debug, Clone)]
pub struct Spot {
    pub at: Vec3,
    pub yaw: f32,
    /// The database's respawn time (s).
    pub respawn: f32,
    /// `gameobject_template.data1`: the loot table.
    pub loot: u32,
}

#[derive(Resource, Default)]
pub struct Objects {
    pub spots: Vec<Spot>,
    live: Vec<Option<Entity>>,
    /// Real second a gathered crate returns at.
    until: Vec<f32>,
    ready: bool,
}

/// One crate in the world: its place in [`Objects::spots`].
#[derive(Component, Debug, Clone, Copy)]
pub struct QuestObject(pub usize);

impl Objects {
    /// A crate was gathered: it returns after its respawn time.
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

/// The crate nearest to `me` within [`REACH`], as (entity, spot index).
pub fn nearest(
    crates: &Query<(Entity, &Transform, &QuestObject), Without<Avatar>>,
    me: Vec3,
) -> Option<(Entity, usize)> {
    crates
        .iter()
        .map(|(e, xf, o)| {
            (
                Vec2::new(me.x - xf.translation.x, me.z - xf.translation.z).length(),
                e,
                o.0,
            )
        })
        .filter(|(d, _, _)| *d <= REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, e, i)| (e, i))
}

#[allow(clippy::too_many_arguments)]
fn ring(
    mut commands: Commands,
    data: Option<Res<Data>>,
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
        let loot = data
            .0
            .objects
            .iter()
            .find(|o| o.entry == CRATE_ENTRY)
            .map_or(0, |o| o.data[1].max(0) as u32);
        let spots: Vec<Spot> = data
            .0
            .object_spawns
            .iter()
            .filter(|s| s.entry == CRATE_ENTRY && !s.event)
            .map(|s| Spot {
                at: benilla_assets::coords::wow_to_bevy(s.pos),
                yaw: s.orientation,
                respawn: s.respawn_max.max(s.respawn_min) as f32,
                loot,
            })
            .collect();
        info!("quests: {} grape crates (loot table {loot})", spots.len());
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
                let e = commands
                    .spawn((
                        Name::new("Milly's Harvest"),
                        Transform::from_translation(at)
                            .with_rotation(Quat::from_rotation_y(spot.yaw)),
                        Visibility::default(),
                        QuestObject(i),
                        ActorModel {
                            m2: assets.load(benilla_assets::m2_url(CRATE_MODEL)),
                            dir: CRATE_DIR.into(),
                            look: Look::Creature {
                                skins: [None, None, None],
                            },
                            scale: 1.0,
                            display: 0,
                            worn: None,
                        },
                    ))
                    .id();
                objects.live[i] = Some(e);
            }
            Some(e) if d > DESPAWN => {
                commands.entity(e).despawn();
                objects.live[i] = None;
            }
            _ => {}
        }
    }
}
