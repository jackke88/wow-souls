//! The valley's friendly NPCs: the Abbey's trainers and vendors, Marshal McBride, Deputy Willem,
//! Eagan, Milly, the guards, the peasants and the White Stallion, each at its database spot, dressed
//! as WoW dresses it (a character display's `CreatureDisplayInfoExtra` row, built by
//! [`crate::actor`]) and holding its main-hand weapon. Posts stand and keep their database facing;
//! the walkers (three guards, five peasants) walk their paths ([`crate::walkers`]). Nothing here is
//! hostile, struck or lockable: the player's sword passes through them, the player's body does not
//! (a body collider on the walk layer, as the hostiles have).

use bevy::prelude::*;
use wow_world::model::World;

use benilla_world::collision::{ColliderEpoch, WorldCollision};

use crate::actor::held::HeldItem;
use crate::actor::{ActorModel, Catalogs, Look};
use crate::npc::{BodyCollider, Placed, Roam, Unplaced};
use crate::walkers::Origin;
use crate::world::{Spot, Template};

/// The Spirit Healer: in WoW only a ghost sees it. Never spawned.
pub const SPIRIT_HEALER: u32 = 6491;

/// Entries the game leaves out of the valley.
pub fn skipped(entry: u32) -> bool {
    entry == SPIRIT_HEALER
}

/// A friendly NPC: its template and what the quest phase will need of it.
#[derive(Component, Debug, Clone)]
pub struct Npc {
    pub entry: u32,
    /// `creature_template.npc_flags` (gossip, quest giver, vendor, trainer ...).
    pub flags: u32,
    /// The line under its name (`Weaponsmith`), empty for most.
    pub subname: String,
    /// Its walk (yd/s): WoW's 2.5 times the template's `speed_walk`.
    pub walk: f32,
}

/// The item display an NPC of `equipment_id` holds in its right hand: its equipment row's main-hand
/// item when that item is a weapon (`class` 2). Shields, the off hand and ranged weapons are not
/// held (the actor builder carries one item, on attachment 1).
pub fn held_weapon(world: &World, equipment_id: u32) -> Option<u32> {
    if equipment_id == 0 {
        return None;
    }
    let row = world.equipment.iter().find(|e| e.entry == equipment_id)?;
    let main = row.items[0];
    let item = world.items.iter().find(|i| i.entry == main)?;
    (item.class == 2 && item.display != 0).then_some(item.display)
}

/// Which of a template's displays a spawn wears: one of the non-zero ones, picked by its guid (the
/// database's display probabilities are not in the manifest).
pub fn pick_display(displays: &[u32], guid: u32) -> Option<u32> {
    let shown: Vec<u32> = displays.iter().copied().filter(|d| *d != 0).collect();
    (!shown.is_empty()).then(|| shown[guid as usize % shown.len()])
}

/// Its walk speed (yd/s): WoW's walk times the template's multiplier, never a crawl.
pub fn walk_speed(speed_walk: f32) -> f32 {
    (crate::motor::WALK_SPEED * speed_walk).max(0.8)
}

/// The body collider of a display: a character's is the player's capsule, a creature's (the
/// stallion) stands as tall as its model's collision height says.
pub fn body_size(character: bool, collision_height: Option<f32>, scale: f32) -> (f32, f32) {
    if character {
        return (
            crate::motor::CAPSULE_RADIUS * scale,
            crate::motor::CAPSULE_HEIGHT * scale,
        );
    }
    let height = collision_height
        .filter(|h| h.is_finite() && *h > 0.0)
        .map_or(crate::motor::CAPSULE_HEIGHT, |h| h * scale)
        .clamp(0.5, 3.0);
    ((0.3 * height).clamp(0.25, 0.8), height)
}

/// One friendly NPC at its spot: its display dressed by the actor builder, its weapon in hand, a
/// body collider, a parked motor (posts never step it; walkers do, in [`crate::walkers`]).
#[allow(clippy::too_many_arguments)]
pub(crate) fn place(
    commands: &mut Commands,
    spot: &Spot,
    t: &Template,
    catalogs: &Catalogs,
    asset_server: &AssetServer,
    collide: &WorldCollision,
    epoch: &mut ColliderEpoch,
) -> Result<Placed, Unplaced> {
    let shown = pick_display(&t.displays, spot.guid).ok_or(Unplaced::NoModel(0))?;
    let Some(model) = ActorModel::for_display(&catalogs.creatures, shown, None, asset_server)
    else {
        warn!("friendly: display {shown} has no model");
        return Err(Unplaced::NoModel(shown));
    };
    let feet = crate::walkers::feet_at(collide, spot.wow).ok_or(Unplaced::NoGround)?;
    let scale = model.scale;
    let character = matches!(model.look, Look::Character(_));
    let body = body_size(character, catalogs.creatures.collision_height(shown), scale);
    let collider = crate::npc::spawn_body(commands, feet, body);
    epoch.bump();
    let mut npc = commands.spawn((
        Name::new(t.name.clone()),
        Transform::from_translation(feet).with_rotation(Quat::from_rotation_y(spot.facing)),
        Visibility::default(),
        model,
        Npc {
            entry: spot.entry,
            flags: t.flags,
            subname: t.subname.clone(),
            walk: walk_speed(t.speed_walk),
        },
        crate::npc::parked(),
        crate::motor::MoveIntent::default(),
        Roam::new(spot.area.clone(), u64::from(spot.guid)),
        Origin {
            feet,
            facing: spot.facing,
            body,
        },
        BodyCollider(collider),
    ));
    if let Some(weapon) = t.weapon {
        npc.insert(HeldItem::right_hand(weapon));
    }
    Ok(Placed {
        entity: npc.id(),
        feet,
        display: shown,
        scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::model::{Equipment, Item};

    fn item(entry: u32, class: u32, display: u32) -> Item {
        Item {
            entry,
            name: format!("i{entry}"),
            class,
            subclass: 0,
            display,
            quality: 0,
            flags: 0,
            buy_price: 0,
            sell_price: 0,
            inventory_type: 13,
            allowable_class: -1,
            allowable_race: -1,
            item_level: 1,
            required_level: 0,
            max_count: 0,
            stackable: 1,
            delay: 2000,
            damage: (1.0, 2.0, 0),
            block: 0,
            armor: 0,
            stats: Vec::new(),
        }
    }

    #[test]
    fn the_spirit_healer_is_left_out() {
        assert!(skipped(SPIRIT_HEALER));
        assert!(!skipped(197), "McBride");
        assert!(!skipped(1642), "a guard");
    }

    #[test]
    fn an_npc_holds_its_main_hand_weapon_and_nothing_else() {
        let world = World {
            // The guard: a long sword, the Stormwind shield, a crossbow; a peasant: an axe; a shield
            // alone; a row whose item is missing.
            equipment: vec![
                Equipment {
                    entry: 1642,
                    probability: 1,
                    items: [1899, 143, 2551],
                },
                Equipment {
                    entry: 11260,
                    probability: 1,
                    items: [1905, 0, 0],
                },
                Equipment {
                    entry: 7,
                    probability: 1,
                    items: [143, 0, 0],
                },
                Equipment {
                    entry: 8,
                    probability: 1,
                    items: [4242, 0, 0],
                },
            ],
            items: vec![
                item(1899, 2, 7483),
                item(143, 4, 2080),
                item(2551, 2, 10671),
                item(1905, 2, 7428),
            ],
            ..World::default()
        };
        assert_eq!(held_weapon(&world, 1642), Some(7483));
        assert_eq!(held_weapon(&world, 11260), Some(7428));
        assert_eq!(held_weapon(&world, 7), None, "a shield is not a weapon");
        assert_eq!(held_weapon(&world, 8), None, "no such item");
        assert_eq!(held_weapon(&world, 0), None, "no equipment");
        assert_eq!(held_weapon(&world, 999), None, "no such row");
    }

    #[test]
    fn a_spawn_wears_one_of_its_templates_displays_by_its_guid() {
        // The rabbit has two displays, the guard two, McBride one.
        assert_eq!(pick_display(&[328, 4626, 0, 0], 79944), Some(328));
        assert_eq!(pick_display(&[328, 4626, 0, 0], 79947), Some(4626));
        assert_eq!(pick_display(&[1859, 0, 0, 0], 79970), Some(1859));
        assert_eq!(pick_display(&[0, 0, 0, 0], 1), None);
    }

    #[test]
    fn walk_speeds_follow_the_template() {
        assert!((walk_speed(1.0) - 2.5).abs() < 1e-6, "a guard");
        assert!((walk_speed(0.666_668) - 1.666_67).abs() < 1e-3, "a peasant");
        assert_eq!(walk_speed(0.0), 0.8);
    }

    /// The real manifest and the client's display tables (skipped without either): each of the 31
    /// friendly entries and the three critters wears a display the client resolves; every friendly
    /// NPC but the White Stallion and the Northshire Peasant is a dressed character model (with its
    /// baked atlas); the critters, the stallion (`RidingHorse`) and the peasant (WoW's own
    /// `HumanMalePeasant` model, display 11354) are plain creature models.
    #[test]
    fn install_every_friendly_and_critter_display_resolves() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../cache/world/northshire.txt");
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("skipped: {} not found", path.display());
            return;
        };
        let Some(data) = benilla_formats::wow_data() else {
            eprintln!("skipped: no WoW install");
            return;
        };
        let world = wow_world::manifest::parse(&text).expect("the manifest parses");
        let book = crate::world::Book::from_world(&world);
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let creatures = benilla_formats::load_creature_catalog(&mut chain).unwrap();
        let mut friendly = std::collections::BTreeSet::new();
        let mut critters = std::collections::BTreeSet::new();
        for s in &book.spots {
            let critter = match s.kind {
                crate::world::Kind::Critter => true,
                crate::world::Kind::Friendly if !skipped(s.entry) => false,
                _ => continue,
            };
            let t = &book.templates[&s.entry];
            // Every display the template has, not only the one this spawn picks.
            for &d in &t.displays {
                let m = creatures
                    .model(d)
                    .unwrap_or_else(|| panic!("{} (entry {}): display {d}", t.name, s.entry));
                let character = m.npc_appearance.is_some();
                let plain = critter || s.entry == 5403 || s.entry == 11260;
                assert_eq!(
                    character, !plain,
                    "{} display {d}: character {character}",
                    t.name
                );
                if character {
                    assert!(
                        m.npc_appearance
                            .as_ref()
                            .is_some_and(|a| a.bake_name.is_some()),
                        "{} display {d} has a baked atlas",
                        t.name
                    );
                }
            }
            if critter {
                critters.insert(s.entry);
            } else {
                friendly.insert(s.entry);
            }
        }
        assert_eq!(
            critters.into_iter().collect::<Vec<_>>(),
            vec![721, 883, 890]
        );
        // 22 entries: six guards and five peasants share theirs.
        assert_eq!(friendly.len(), 22, "{friendly:?}");
        let live_friendly = book
            .spots
            .iter()
            .filter(|s| s.kind == crate::world::Kind::Friendly && !skipped(s.entry))
            .count();
        assert_eq!(live_friendly, 31);
    }

    #[test]
    fn bodies_are_the_players_capsule_or_the_models_height() {
        let (r, h) = body_size(true, Some(9.0), 1.0);
        assert_eq!(
            (r, h),
            (crate::motor::CAPSULE_RADIUS, crate::motor::CAPSULE_HEIGHT)
        );
        let (r, h) = body_size(false, Some(2.0), 0.9);
        assert!((h - 1.8).abs() < 1e-5 && (r - 0.54).abs() < 1e-5, "{r} {h}");
        let (r, h) = body_size(false, None, 1.0);
        assert!((h - crate::motor::CAPSULE_HEIGHT).abs() < 1e-5 && r > 0.25);
        assert_eq!(body_size(false, Some(40.0), 1.0).1, 3.0, "clamped");
    }
}
