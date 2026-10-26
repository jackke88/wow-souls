//! The world's decorative game objects (2026-10-17, "Eldarna i eldstäderna är inte där"): in WoW the
//! fires in hearths and camps ("Cozy Fire", "Campfire", "Warm Fire", "Bonfire"), forges and stoves are
//! spell-focus game objects (type 8), and much of the furniture and dressing (benches, braziers, crates:
//! type 5 generic and type 7 chairs) and the mailboxes (type 19) are game objects too, not the
//! building's or the map's doodads. The game placed only the quest objects and chests
//! ([`crate::quests::objects`], [`crate::loot`]), so every one of these was missing.
//!
//! Each is the client's own model from `GameObjectDisplayInfo`, built as a native effect instance
//! ([`crate::loot::vfx::effect`]): its geometry, its first sequence looping, its emitters (the
//! flames and the smoke) and its ribbons. They come and go in a ring around the player like the quest
//! objects, at their database place (a game object's height is exact; the snap would put a hearth's
//! fire on the chimney), turned and sized as the database has them. They do not collide.

use std::collections::BTreeSet;

use benilla_assets::{m2_url, LockRecover};
use bevy::prelude::*;

use crate::actor::Catalogs;
use crate::avatar::Avatar;
use crate::loot::Data;
use crate::settle::Phase;

/// Game object types placed as decoration: generic, chair, spell focus, mailbox.
pub const KINDS: [u32; 4] = [5, 7, 8, 19];
/// Within this distance (yd, on the ground) an object is placed, past this one it goes.
const SPAWN: f32 = 110.0;
const DESPAWN: f32 = 140.0;

/// One decorative object's place and model.
#[derive(Debug, Clone)]
struct Spot {
    name: String,
    at: Vec3,
    yaw: f32,
    scale: f32,
    model: String,
}

#[derive(Resource, Default)]
struct Decor {
    ready: bool,
    spots: Vec<Spot>,
    live: Vec<Option<Entity>>,
}

/// Whether an object template is placed as decoration: one of [`KINDS`], with a model, and not a quest
/// object (those are [`crate::quests::objects`]'s).
pub fn decorative(kind: u32, display: u32, entry: u32, quest_objects: &BTreeSet<u32>) -> bool {
    KINDS.contains(&kind)
        && display != 0
        && !quest_objects.contains(&entry)
        && entry != crate::quests::objects::MIRROR_LAKE_WATERWALL
}

fn ring(
    mut commands: Commands,
    data: Option<Res<Data>>,
    catalogs: Option<Res<Catalogs>>,
    assets: Res<AssetServer>,
    avatar: Query<&Transform, With<Avatar>>,
    mut decor: ResMut<Decor>,
) {
    if !crate::world::world_on() {
        return;
    }
    let Some(data) = data else { return };
    if !decor.ready {
        let Some(catalogs) = catalogs else { return };
        decor.ready = true;
        let go_catalog = {
            let mut chain = catalogs.chain.lock_recover();
            benilla_formats::load_gameobject_catalog(&mut chain).ok()
        };
        let Some(go_catalog) = go_catalog else {
            warn!("decor: no GameObjectDisplayInfo; the world's objects stay unplaced");
            return;
        };
        let quest_objects: BTreeSet<u32> = data
            .0
            .starts
            .iter()
            .chain(&data.0.ends)
            .filter(|r| r.object)
            .map(|r| r.npc)
            .collect();
        let mut kinds = std::collections::BTreeMap::<u32, usize>::new();
        let spots: Vec<Spot> = data
            .0
            .object_spawns
            .iter()
            .filter(|s| !s.event)
            .filter_map(|s| {
                let t = data.0.objects.iter().find(|t| t.entry == s.entry)?;
                if !decorative(t.kind, t.display, t.entry, &quest_objects) {
                    return None;
                }
                let model = go_catalog.model_path(t.display)?.to_string();
                *kinds.entry(t.kind).or_default() += 1;
                Some(Spot {
                    name: t.name.clone(),
                    at: benilla_assets::coords::wow_to_bevy(s.pos),
                    yaw: s.orientation,
                    scale: if t.size > 0.0 { t.size } else { 1.0 },
                    model,
                })
            })
            .collect();
        info!("decor: {} world objects by type {kinds:?}", spots.len());
        decor.live = vec![None; spots.len()];
        decor.spots = spots;
    }
    let Ok(me) = avatar.single() else { return };
    let Decor { spots, live, .. } = &mut *decor;
    for (spot, live) in spots.iter().zip(live.iter_mut()) {
        let d = Vec2::new(me.translation.x - spot.at.x, me.translation.z - spot.at.z).length();
        match *live {
            None if d <= SPAWN => {
                let owner = commands
                    .spawn((
                        Name::new(spot.name.clone()),
                        Transform::from_translation(spot.at)
                            .with_rotation(Quat::from_rotation_y(spot.yaw))
                            .with_scale(Vec3::splat(spot.scale)),
                        Visibility::default(),
                    ))
                    .id();
                crate::loot::vfx::effect(
                    &mut commands,
                    "WoW world object",
                    owner,
                    assets.load(m2_url(&spot.model)),
                    None,
                    false,
                );
                *live = Some(owner);
            }
            Some(e) if d > DESPAWN => {
                commands.entity(e).despawn();
                *live = None;
            }
            _ => {}
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Decor>()
        .add_systems(Update, ring.run_if(in_state(Phase::Playing)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_furniture_and_mailboxes_are_decor_quest_objects_and_chests_are_not() {
        let none = BTreeSet::new();
        assert!(decorative(8, 192, 1833, &none), "a campfire");
        assert!(decorative(5, 10, 1, &none));
        assert!(decorative(7, 10, 2, &none), "a bench");
        assert!(decorative(19, 10, 3, &none), "a mailbox");
        assert!(!decorative(3, 10, 4, &none), "a chest is the loot's");
        assert!(!decorative(8, 0, 5, &none), "nothing to draw");
        let quest: BTreeSet<u32> = [6].into();
        assert!(
            !decorative(5, 10, 6, &quest),
            "a quest giver object is the quests'"
        );
        assert!(!decorative(
            8,
            10,
            crate::quests::objects::MIRROR_LAKE_WATERWALL,
            &none
        ));
    }
}
