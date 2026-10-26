//! Pick Northshire's connected set out of the whole database.
//!
//! The valley is told apart from its surroundings by the caller ([`InValley`]: the real one asks the
//! client's terrain data, the tests give a closure). Everything else follows from that and the
//! database's own links, never from a coordinate box:
//!
//! 1. **Spawns** are the creature and object spawns the valley contains, minus the ones that belong
//!    to a seasonal game event; each gets the area id under it. The named spawns outside the valley
//!    ([`NAMED_SPAWNS`], the zone boss) come with them, their area 0.
//! 2. **Quests** are those whose zone is Northshire Valley, plus every quest a creature spawned
//!    in the valley (or one of its `alt` entries) or an object spawned in the valley starts or ends,
//!    minus the seasonal ones.
//! 3. **Creatures** (templates) are the entries the spawns use and the creatures that start or end a
//!    selected quest, so a quest handed in outside the valley (Marshal Dughan in Goldshire) still
//!    has its NPC. **Objects** are the spawned templates, the objects a quest's kill list names
//!    (`req_kills` below zero) and the objects that start or end a selected quest.
//! 4. **Relations** are all start / end rows of the selected quests; their creature or object is in
//!    the selection by (3), so no row points at nothing.
//! 5. **Loot** is every row of the loot table of each selected creature (`loot_id`, or its entry when
//!    that is 0) and of each selected chest (`data1` of a type 3 object), plus the reference tables
//!    those rows name (one level). **Vendors** and **equipment** are the rows of the selected creatures.
//! 6. **Items** are what the selected quests require, hand out or reward, what the loot, the vendors'
//!    stock and the equipment name, and the pieces a new character is dressed in
//!    (every class's kit, [`crate::class::CLASSES`]) and the trial character's
//!    ([`crate::equipment::TRIAL_GEAR`]). A name with no row in `item_template` is skipped.
//! 7. **Areas** are the distinct areas under the selected spawns.
//! 8. **Waypoints** are the `creature_movement` rows whose guid is a selected creature spawn's guid, so
//!    a walking spawn (`movement == 2`) keeps its path and a path of a spawn outside the valley is
//!    dropped. [`spawns_without_path`] lists the walkers that ended up with no rows.
//! 9. **Scripts** are the rows of the scripts a selected quest names (`StartScript` in
//!    `quest_start_scripts`, `CompleteScript` in `quest_end_scripts`), each tagged with that quest;
//!    **item extras** (`start_quest`, `bonding`) are those of the selected items; the **XP table** is
//!    taken whole.
//!
//! Every vector is sorted by its natural key, so the same database gives the same manifest. The shop
//! lists keep the database's order inside one vendor (the model has no slot number to sort by).

use crate::db::Db;
use crate::model::*;
use crate::valley::NORTHSHIRE_VALLEY;
use std::collections::{BTreeMap, BTreeSet};

/// How the valley is told apart from its surroundings: given by the caller (the real one asks the
/// terrain; tests give a closure).
pub trait InValley {
    fn area_of(&self, x: f32, y: f32) -> u32;
    fn contains(&self, x: f32, y: f32) -> bool;
    fn name(&self, area: u32) -> String;

    /// Full position: interiors can override the terrain above a spawn.
    fn area_at(&self, pos: [f32; 3]) -> u32 {
        self.area_of(pos[0], pos[1])
    }

    fn contains_at(&self, pos: [f32; 3]) -> bool {
        self.contains(pos[0], pos[1])
    }

    fn quest_in_zone(&self, zone_or_sort: i32) -> bool {
        zone_or_sort == NORTHSHIRE_VALLEY as i32
    }

    fn quest_level_ok(&self, _level: i32, _min_level: u32) -> bool {
        true
    }

    /// Northshire's historical extra boss; a whole-zone selector needs no extras.
    fn named_spawns(&self) -> &[u32] {
        &NAMED_SPAWNS
    }
}

/// The `gameobject_template.type` of a chest (herb and ore nodes are chests too).
const OBJECT_CHEST: u32 = 3;

/// The guids of the selected spawns that walk a path (`movement == 2`) but have no waypoint rows,
/// sorted. A walker without a path would stand still, so the importer refuses such a selection.
pub fn spawns_without_path(w: &World) -> Vec<u32> {
    let with_path: BTreeSet<u32> = w.waypoints.iter().map(|p| p.guid).collect();
    let mut guids: Vec<u32> = w
        .spawns
        .iter()
        .filter(|s| s.movement == 2 && !with_path.contains(&s.guid))
        .map(|s| s.guid)
        .collect();
    guids.sort_unstable();
    guids
}

/// Database guids of spawns outside the valley that the game wants anyway: the zone boss. Hogger
/// (entry 448) has five rows around WoW (-9946, 604, 38); 80731 is the one that wanders 15 yd and
/// the one the game uses.
pub const NAMED_SPAWNS: [u32; 1] = [80_731];

/// Select the connected set; see the module documentation for the rules.
pub fn select(db: &Db, valley: &dyn InValley) -> World {
    // 1. Spawns inside the valley, with the area under each.
    let mut spawns: Vec<Spawn> = db
        .spawns
        .iter()
        .filter(|s| !s.event && valley.contains_at(s.pos))
        .map(|s| Spawn {
            area: valley.area_at(s.pos),
            ..s.clone()
        })
        .collect();
    // The named ones outside the valley (no valley area: 0).
    let named: Vec<Spawn> = db
        .spawns
        .iter()
        .filter(|s| valley.named_spawns().contains(&s.guid) && !s.event)
        .filter(|s| !spawns.iter().any(|v| v.guid == s.guid))
        .map(|s| Spawn {
            area: 0,
            ..s.clone()
        })
        .collect();
    spawns.extend(named);
    spawns.sort_by_key(|s| s.guid);
    let mut object_spawns: Vec<ObjectSpawn> = db
        .object_spawns
        .iter()
        .filter(|s| !s.event && valley.contains_at(s.pos))
        .map(|s| ObjectSpawn {
            area: valley.area_at(s.pos),
            ..s.clone()
        })
        .collect();
    object_spawns.sort_by_key(|s| s.guid);

    let spawned: BTreeSet<u32> = spawns
        .iter()
        .flat_map(|s| std::iter::once(s.entry).chain(s.alt))
        .filter(|&e| e != 0)
        .collect();
    let spawned_objects: BTreeSet<u32> = object_spawns.iter().map(|s| s.entry).collect();

    // 2. Quests: the valley's zone, and whatever a valley creature or object starts or ends.
    let mut quest_ids: BTreeSet<u32> = db
        .quests
        .iter()
        .filter(|q| valley.quest_in_zone(q.zone_or_sort))
        .map(|q| q.id)
        .collect();
    let creature_rows = || db.creature_starts.iter().chain(&db.creature_ends);
    let object_rows = || db.object_starts.iter().chain(&db.object_ends);
    quest_ids.extend(
        creature_rows()
            .filter(|r| spawned.contains(&r.npc))
            .map(|r| r.quest),
    );
    quest_ids.extend(
        object_rows()
            .filter(|r| spawned_objects.contains(&r.npc))
            .map(|r| r.quest),
    );
    let mut quests: Vec<Quest> = db
        .quests
        .iter()
        .filter(|q| {
            !q.event
                && quest_ids.contains(&q.id)
                && valley.quest_level_ok(q.level, q.min_level)
                && !q.title.contains("<NYI>")
        })
        .cloned()
        .collect();
    quests.sort_by_key(|q| q.id);
    let quest_ids: BTreeSet<u32> = quests.iter().map(|q| q.id).collect();

    // 4. The relation rows of the selected quests (before 3: they name the extra NPCs and objects).
    let relations = |a: &[Relation], b: &[Relation]| {
        let mut v: Vec<Relation> = a
            .iter()
            .chain(b)
            .filter(|r| quest_ids.contains(&r.quest))
            .copied()
            .collect();
        v.sort_by_key(|r| (r.quest, r.npc, r.object));
        v.dedup();
        v
    };
    let starts = relations(&db.creature_starts, &db.object_starts);
    let ends = relations(&db.creature_ends, &db.object_ends);

    // 3. Creature and object templates.
    let mut npc_ids = spawned;
    let mut object_ids = spawned_objects;
    for r in starts.iter().chain(&ends) {
        if r.object {
            object_ids.insert(r.npc);
        } else {
            npc_ids.insert(r.npc);
        }
    }
    for q in &quests {
        for &(target, _) in &q.req_kills {
            if target < 0 {
                object_ids.insert(target.unsigned_abs());
            }
        }
    }
    let mut creatures: Vec<Creature> = db
        .creatures
        .iter()
        .filter(|c| npc_ids.contains(&c.entry))
        .cloned()
        .collect();
    creatures.sort_by_key(|c| c.entry);
    let mut objects: Vec<ObjectTemplate> = db
        .objects
        .iter()
        .filter(|o| object_ids.contains(&o.entry))
        .cloned()
        .collect();
    objects.sort_by_key(|o| o.entry);

    // 5. Loot: the creatures' and chests' tables, then the reference tables they point to.
    let creature_tables: BTreeSet<u32> = creatures
        .iter()
        .map(|c| if c.loot_id != 0 { c.loot_id } else { c.entry })
        .collect();
    let object_tables: BTreeSet<u32> = objects
        .iter()
        .filter(|o| o.kind == OBJECT_CHEST && o.data[1] > 0)
        .map(|o| o.data[1].unsigned_abs())
        .collect();
    let mut loot: Vec<LootRow> = db
        .loot
        .iter()
        .filter(|l| match l.kind {
            LootKind::Creature => creature_tables.contains(&l.table),
            LootKind::Object => object_tables.contains(&l.table),
            LootKind::Reference => false,
        })
        .cloned()
        .collect();
    let reference_tables: BTreeSet<u32> = loot
        .iter()
        .filter(|l| l.min_or_ref < 0)
        .map(|l| l.min_or_ref.unsigned_abs())
        .collect();
    loot.extend(
        db.loot
            .iter()
            .filter(|l| l.kind == LootKind::Reference && reference_tables.contains(&l.table))
            .cloned(),
    );
    loot.sort_by_key(|l| (l.kind as u8, l.table, l.group, l.item, l.min_or_ref));

    // Vendors and equipment of the selected creatures.
    let mut vendors: Vec<Vendor> = db
        .vendors
        .iter()
        .filter(|v| npc_ids.contains(&v.npc))
        .cloned()
        .collect();
    vendors.sort_by_key(|v| v.npc); // stable: a shop keeps the database's order
    let equipment_ids: BTreeSet<u32> = creatures
        .iter()
        .map(|c| c.equipment_id)
        .filter(|&e| e != 0)
        .collect();
    let mut equipment: Vec<Equipment> = db
        .equipment
        .iter()
        .filter(|e| equipment_ids.contains(&e.entry))
        .cloned()
        .collect();
    equipment.sort_by_key(|e| e.entry);

    // 6. Items.
    let mut item_ids: BTreeSet<u32> = BTreeSet::new();
    for q in &quests {
        item_ids.insert(q.src_item);
        for &(id, _) in q
            .req_items
            .iter()
            .chain(&q.reward_choice)
            .chain(&q.reward_items)
        {
            item_ids.insert(id);
        }
    }
    // A negative `min_or_ref` is a reference row; its `item` column is not an item.
    item_ids.extend(loot.iter().filter(|l| l.min_or_ref >= 0).map(|l| l.item));
    item_ids.extend(vendors.iter().map(|v| v.item));
    item_ids.extend(equipment.iter().flat_map(|e| e.items));
    item_ids.extend(
        crate::class::CLASSES
            .iter()
            .flat_map(|c| c.kit.iter())
            .chain(&crate::equipment::TRIAL_GEAR)
            .map(|&(_, id)| id),
    );
    item_ids.remove(&0);
    let mut items: Vec<Item> = db
        .items
        .iter()
        .filter(|i| item_ids.contains(&i.entry))
        .cloned()
        .collect();
    items.sort_by_key(|i| i.entry);
    let mut item_extras: Vec<ItemExtra> = db
        .item_extras
        .iter()
        .filter(|i| item_ids.contains(&i.entry))
        .copied()
        .collect();
    item_extras.sort_by_key(|i| i.entry);

    // 9. The scripts the selected quests name, once per quest that names them; the rows of one script
    // keep the table's order (the executor sorts by delay).
    let mut scripts: Vec<QuestScript> = Vec::new();
    for q in &quests {
        for (start, id) in [(true, q.start_script), (false, q.complete_script)] {
            if id == 0 {
                continue;
            }
            scripts.extend(
                db.scripts
                    .iter()
                    .filter(|(sid, s)| *sid == id && s.start == start)
                    .map(|(_, s)| QuestScript {
                        quest: q.id,
                        ..s.clone()
                    }),
            );
        }
    }

    // 8. The paths of the selected spawns, sorted by (guid, path id, point).
    let spawn_guids: BTreeSet<u32> = spawns.iter().map(|s| s.guid).collect();
    let mut waypoints: Vec<Waypoint> = db
        .waypoints
        .iter()
        .filter(|w| spawn_guids.contains(&w.guid))
        .cloned()
        .collect();
    waypoints.sort_by_key(|w| (w.guid, w.path_id, w.point));

    // 7. The areas under the selected spawns (0 = none found, not an area).
    let mut areas: BTreeMap<u32, String> = BTreeMap::new();
    for area in spawns
        .iter()
        .map(|s| s.area)
        .chain(object_spawns.iter().map(|s| s.area))
        .filter(|&a| a != 0)
    {
        areas.entry(area).or_insert_with(|| valley.name(area));
    }

    World {
        areas: areas.into_iter().collect(),
        creature_meta: db
            .creature_meta
            .iter()
            .filter(|m| npc_ids.contains(&m.entry))
            .cloned()
            .collect(),
        spawn_pools: db
            .spawn_pools
            .iter()
            .filter_map(|p| {
                let members: Vec<_> = p
                    .members
                    .iter()
                    .filter(|m| spawn_guids.contains(&m.guid))
                    .cloned()
                    .collect();
                (!members.is_empty()).then(|| SpawnPool {
                    members,
                    ..p.clone()
                })
            })
            .collect(),
        creatures,
        spawns,
        waypoints,
        quests,
        starts,
        ends,
        items,
        objects,
        object_spawns,
        loot,
        vendors,
        equipment,
        scripts,
        // The whole table: the player's levels do not depend on the valley.
        xp_for_level: db.xp_for_level.clone(),
        item_extras,
        // The whole tables: a player's levels do not depend on the valley.
        level_stats: db.level_stats.clone(),
        class_stats: db.class_stats.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valley that is "every position with y < 0": area 9 east of x = -9000, area 86 west of it,
    /// Elwynn (12) everywhere else.
    struct Fake;

    impl InValley for Fake {
        fn area_of(&self, x: f32, y: f32) -> u32 {
            if !self.contains(x, y) {
                12
            } else if x > -9000.0 {
                9
            } else {
                86
            }
        }
        fn contains(&self, _x: f32, y: f32) -> bool {
            y < 0.0
        }
        fn name(&self, area: u32) -> String {
            match area {
                9 => "Northshire Valley".into(),
                86 => "Echo Ridge Mine".into(),
                12 => "Elwynn Forest".into(),
                _ => String::new(),
            }
        }
    }

    fn spawn(guid: u32, entry: u32, x: f32, y: f32) -> Spawn {
        Spawn {
            guid,
            entry,
            alt: [0; 4],
            pos: [x, y, 70.0],
            orientation: 0.0,
            respawn_min: 120,
            respawn_max: 120,
            wander: 0.0,
            movement: 0,
            area: 0,
            event: false,
        }
    }

    fn creature(entry: u32, name: &str, loot_id: u32, equipment_id: u32) -> Creature {
        Creature {
            entry,
            name: name.into(),
            subname: String::new(),
            level_min: 1,
            level_max: 1,
            faction: 12,
            npc_flags: 0,
            display: [1, 0, 0, 0],
            scale: [1.0, 0.0, 0.0, 0.0],
            kind: 5,
            rank: 0,
            detection_range: 20.0,
            call_for_help_range: 0.0,
            leash_range: 0.0,
            speed_walk: 1.0,
            speed_run: 1.14,
            loot_id,
            equipment_id,
            vendor_id: 0,
            trainer_type: 0,
            trainer_class: 0,
            movement_type: 0,
            civilian: false,
            health_multiplier: 1.0,
            damage_multiplier: 1.0,
            unit_class: 1,
        }
    }

    fn quest(id: u32, zone: i32) -> Quest {
        Quest {
            id,
            patch: 0,
            title: format!("Quest {id}"),
            details: String::new(),
            objectives: String::new(),
            offer_reward: String::new(),
            request_items: String::new(),
            end_text: String::new(),
            objective_text: Default::default(),
            zone_or_sort: zone,
            min_level: 1,
            level: 1,
            required_classes: 0,
            required_races: 0,
            prev: 0,
            next: 0,
            exclusive_group: 0,
            next_in_chain: 0,
            flags: 0,
            special_flags: 0,
            limit_time: 0,
            src_item: 0,
            src_item_count: 0,
            req_items: vec![],
            req_kills: vec![],
            req_spell_cast: vec![],
            reward_choice: vec![],
            reward_items: vec![],
            reward_money: 0,
            reward_xp: 0,
            reward_spell: 0,
            point: (0, 0.0, 0.0),
            start_script: 0,
            complete_script: 0,
            event: false,
            max_level: 0,
            required_condition: 0,
            rep_objective: (0, 0),
            rep_min: (0, 0),
            rep_max: (0, 0),
            reward_rep: vec![],
            reward_mail: (0, 0, 0),
        }
    }

    fn item(entry: u32) -> Item {
        Item {
            entry,
            name: format!("Item {entry}"),
            class: 0,
            subclass: 0,
            display: 0,
            quality: 1,
            flags: 0,
            buy_price: 0,
            sell_price: 0,
            inventory_type: 0,
            allowable_class: -1,
            allowable_race: -1,
            item_level: 1,
            required_level: 0,
            max_count: 0,
            stackable: 1,
            delay: 0,
            damage: (0.0, 0.0, 0),
            block: 0,
            armor: 0,
            stats: Vec::new(),
        }
    }

    fn object(entry: u32, kind: u32, data1: i32) -> ObjectTemplate {
        ObjectTemplate {
            entry,
            name: format!("Object {entry}"),
            kind,
            display: 1,
            faction: 0,
            flags: 0,
            size: 1.0,
            data: [0, data1, 0, 0, 0, 0, 0, 0, 0, 0],
        }
    }

    fn object_spawn(guid: u32, entry: u32, x: f32, y: f32) -> ObjectSpawn {
        ObjectSpawn {
            guid,
            entry,
            pos: [x, y, 70.0],
            orientation: 0.0,
            respawn_min: 300,
            respawn_max: 300,
            area: 0,
            event: false,
        }
    }

    fn rel(npc: u32, quest: u32, object: bool) -> Relation {
        Relation { npc, quest, object }
    }

    fn loot(kind: LootKind, table: u32, item: u32, chance: f32, min_or_ref: i32) -> LootRow {
        LootRow {
            kind,
            table,
            item,
            chance,
            group: 0,
            min_or_ref,
            max_count: 1,
        }
    }

    /// A small world with every case the rules name:
    /// * inside: thug 38 (loot table 38, equipment 4), McBride 197, kobold 80 (+ alt 257, loot via its
    ///   own entry), vendor Godric 1213; chest 61 (loot 61), board 62 (starts quest 200);
    /// * outside: rabbit 721, chest 65, a thug in a game event (guid 4);
    /// * quest 7 (zone 9, McBride, kills 80 and object 63), quest 54 (zone 9, ends at Dughan 240 who
    ///   is not spawned), 100 (zone 12, started by the vendor), 200 (started by object 62), and the
    ///   ones that must stay out: 300 (event), 400 (event, started by the thug), 500 (only the outside
    ///   rabbit), 600 (only Dughan), 999 (unrelated).
    fn db() -> Db {
        let mut q7 = quest(7, 9);
        q7.req_kills = vec![(80, 10), (-63, 4)];
        q7.reward_choice = vec![(1, 1)];
        q7.reward_items = vec![(2, 1), (8888, 1)];
        q7.src_item = 3;
        q7.complete_script = 7;
        let mut q54 = quest(54, 9);
        q54.req_items = vec![(4, 1)];
        q54.start_script = 54;
        let mut q300 = quest(300, 9);
        q300.event = true;
        let mut q400 = quest(400, 12);
        q400.event = true;

        let mut event_spawn = spawn(4, 38, -8900.0, -101.0);
        event_spawn.event = true;
        let mut kobold = spawn(5, 80, -9100.0, -400.0);
        kobold.alt = [257, 0, 0, 0];

        Db {
            creature_meta: Vec::new(),
            spawn_pools: Vec::new(),
            spawns: vec![
                spawn(1, 38, -8900.0, -100.0),
                spawn(2, 721, -9000.0, 100.0),
                spawn(3, 197, -8913.0, -136.0),
                event_spawn,
                kobold,
                spawn(6, 1213, -8890.0, -150.0),
            ],
            creatures: vec![
                creature(38, "Defias Thug", 38, 4),
                creature(80, "Kobold Worker", 0, 0),
                creature(197, "Marshal McBride", 0, 0),
                creature(240, "Marshal Dughan", 0, 0),
                creature(257, "Kobold Vermin", 0, 0),
                creature(721, "Rabbit", 721, 0),
                creature(1213, "Godric Rothgar", 0, 0),
                creature(69, "Timber Wolf", 69, 0),
            ],
            quests: vec![
                q7,
                q54,
                quest(100, 12),
                quest(200, 12),
                q300,
                q400,
                quest(500, 12),
                quest(600, 12),
                quest(999, 12),
            ],
            creature_starts: vec![
                rel(197, 7, false),
                rel(197, 54, false),
                rel(1213, 100, false),
                rel(38, 400, false),
                rel(721, 500, false),
                rel(240, 600, false),
            ],
            creature_ends: vec![
                rel(197, 7, false),
                rel(240, 54, false),
                rel(721, 999, false),
            ],
            object_starts: vec![rel(62, 200, true)],
            object_ends: vec![rel(64, 7, true)],
            items: [
                1, 2, 3, 4, 5, 6, 9, 10, 11, 25, 26, 159, 752, 753, 754, 755, 999, 2070,
            ]
            .into_iter()
            .map(item)
            .collect(),
            objects: vec![
                object(61, 3, 61),
                object(62, 5, 0),
                object(63, 5, 0),
                object(64, 5, 0),
                object(65, 3, 65),
            ],
            object_spawns: vec![
                object_spawn(10, 61, -8950.0, -90.0),
                object_spawn(11, 62, -9050.0, -95.0),
                object_spawn(12, 65, -9000.0, 50.0),
            ],
            loot: vec![
                loot(LootKind::Creature, 38, 752, -80.0, 1),
                loot(LootKind::Creature, 38, 0, 100.0, -900),
                loot(LootKind::Creature, 38, 753, 5.0, 1),
                loot(LootKind::Creature, 80, 754, 30.0, 1),
                loot(LootKind::Creature, 721, 755, 50.0, 1),
                loot(LootKind::Object, 61, 5, 25.0, 1),
                loot(LootKind::Object, 5000, 6, 25.0, 1),
                loot(LootKind::Reference, 900, 9, 10.0, 1),
                loot(LootKind::Reference, 900, 10, 10.0, 1),
                loot(LootKind::Reference, 901, 11, 10.0, 1),
            ],
            vendors: vec![
                Vendor {
                    npc: 1213,
                    item: 2070,
                    max_count: 0,
                    incr_time: 0,
                },
                Vendor {
                    npc: 721,
                    item: 2070,
                    max_count: 0,
                    incr_time: 0,
                },
                Vendor {
                    npc: 1213,
                    item: 159,
                    max_count: 0,
                    incr_time: 0,
                },
            ],
            equipment: vec![
                Equipment {
                    entry: 4,
                    probability: 100,
                    items: [25, 26, 0],
                },
                Equipment {
                    entry: 5,
                    probability: 100,
                    items: [999, 0, 0],
                },
            ],
            waypoints: vec![],
            scripts: vec![
                // Quest 54 (zone 9) names start script 54 and quest 7 end script 7; quest 100 (selected
                // through the vendor) names no script; script 999 belongs to nobody selected.
                script(54, true, 2, 1242),
                script(54, true, 1, 6245),
                script(7, false, 0, 11),
                script(7, true, 0, 12),
                script(999, true, 0, 13),
            ],
            xp_for_level: vec![400, 900, 1400],
            level_stats: vec![LevelStat {
                class: 1,
                level: 1,
                strength: 23,
                agility: 20,
                stamina: 22,
                intellect: 20,
                spirit: 20,
            }],
            class_stats: vec![ClassStat {
                class: 1,
                level: 1,
                base_hp: 20,
                base_mana: 0,
            }],
            item_extras: [159, 752, 999, 5000]
                .into_iter()
                .map(|entry| ItemExtra {
                    entry,
                    start_quest: if entry == 159 { 5805 } else { 0 },
                    bonding: 4,
                })
                .collect(),
        }
    }

    /// A script row of script `id` (`QuestScript::quest` is 0 as `Db` has it).
    fn script(id: u32, start: bool, delay: u32, dataint: i32) -> (u32, QuestScript) {
        (
            id,
            QuestScript {
                quest: 0,
                start,
                delay,
                priority: 0,
                command: 0,
                datalong: [0; 4],
                target_param: [0; 2],
                target_type: 0,
                data_flags: 0,
                dataint: [dataint, 0, 0, 0],
                pos: [0.0; 4],
                condition_id: 0,
                comments: String::new(),
            },
        )
    }

    fn ids<T, K: Ord>(v: &[T], key: impl Fn(&T) -> K) -> Vec<K> {
        v.iter().map(key).collect()
    }

    #[test]
    fn spawns_are_the_inside_non_event_ones_and_carry_their_area() {
        let w = select(&db(), &Fake);
        assert_eq!(ids(&w.spawns, |s| s.guid), vec![1, 3, 5, 6]);
        let areas = ids(&w.spawns, |s| s.area);
        assert_eq!(areas, vec![9, 9, 86, 9]);
        assert!(w.spawns.iter().all(|s| !s.event));
        // The rest of the row is untouched.
        assert_eq!(w.spawns[2].alt, [257, 0, 0, 0]);
        assert_eq!(w.spawns[0].pos, [-8900.0, -100.0, 70.0]);
    }

    #[test]
    fn object_spawns_are_the_inside_ones_and_carry_their_area() {
        let w = select(&db(), &Fake);
        assert_eq!(ids(&w.object_spawns, |s| s.guid), vec![10, 11]);
        assert_eq!(ids(&w.object_spawns, |s| s.area), vec![9, 86]);
    }

    #[test]
    fn creatures_are_the_spawned_entries_their_alts_and_the_quest_npcs() {
        let w = select(&db(), &Fake);
        // 38, 197, 80, 1213 and 257 (alt) are spawned inside; 240 ends quest 54. Not the rabbit (only
        // spawned outside), not the wolf (not spawned), though both are in the database.
        assert_eq!(
            ids(&w.creatures, |c| c.entry),
            vec![38, 80, 197, 240, 257, 1213]
        );
    }

    #[test]
    fn quests_are_zone_nine_plus_those_the_valley_npcs_and_objects_touch_minus_events() {
        let w = select(&db(), &Fake);
        // 7 and 54 by zone, 100 by the vendor, 200 by the board; not the event quests 300 / 400,
        // not 500 (only an outside spawn), 600 (only Dughan), 999 (unrelated).
        assert_eq!(ids(&w.quests, |q| q.id), vec![7, 54, 100, 200]);
    }

    #[test]
    fn relations_are_those_of_the_selected_quests_with_their_npcs_included() {
        let w = select(&db(), &Fake);
        let starts: Vec<_> = w
            .starts
            .iter()
            .map(|r| (r.quest, r.npc, r.object))
            .collect();
        assert_eq!(
            starts,
            vec![
                (7, 197, false),
                (54, 197, false),
                (100, 1213, false),
                (200, 62, true)
            ]
        );
        let ends: Vec<_> = w.ends.iter().map(|r| (r.quest, r.npc, r.object)).collect();
        assert_eq!(
            ends,
            vec![(7, 64, true), (7, 197, false), (54, 240, false)],
            "Dughan is not spawned but ends quest 54; the object 64 ends quest 7"
        );
        // Every relation points at an NPC or object that is in the world.
        for r in w.starts.iter().chain(&w.ends) {
            if r.object {
                assert!(w.objects.iter().any(|o| o.entry == r.npc), "{r:?}");
            } else {
                assert!(w.creatures.iter().any(|c| c.entry == r.npc), "{r:?}");
            }
        }
    }

    #[test]
    fn objects_are_the_spawned_ones_the_kill_targets_and_the_quest_objects() {
        let w = select(&db(), &Fake);
        // 61 and 62 are spawned inside, 63 is quest 7's `-63` kill target, 64 ends quest 7; 65 is
        // spawned outside.
        assert_eq!(ids(&w.objects, |o| o.entry), vec![61, 62, 63, 64]);
    }

    #[test]
    fn items_are_everything_quests_loot_vendors_and_equipment_name() {
        let w = select(&db(), &Fake);
        // quests: reward choice 1, rewards 2 (and 8888, which has no template and is skipped), source
        // item 3, required item 4; loot: 752, 753 (not the reference row's placeholder 0), 754, the
        // chest's 5, the reference table's 9 and 10; vendor: 2070, 159; equipment of the thug: 25, 26.
        // Not: 6 (an unrelated chest), 11 (an unreferenced table), 755 (the rabbit), 999 (unrelated
        // equipment).
        assert_eq!(
            ids(&w.items, |i| i.entry),
            vec![1, 2, 3, 4, 5, 9, 10, 25, 26, 159, 752, 753, 754, 2070]
        );
    }

    #[test]
    fn loot_follows_one_level_of_references_and_falls_back_to_the_entry() {
        let w = select(&db(), &Fake);
        let rows: Vec<_> = w.loot.iter().map(|l| (l.kind, l.table, l.item)).collect();
        assert_eq!(
            rows,
            vec![
                // Table 38 is the thug's `loot_id`.
                (LootKind::Creature, 38, 0),
                (LootKind::Creature, 38, 752),
                (LootKind::Creature, 38, 753),
                // Kobold Worker 80 has loot_id 0: its own entry is the table.
                (LootKind::Creature, 80, 754),
                // The chest's `data1`.
                (LootKind::Object, 61, 5),
                // The reference table named by the thug's row (-900), both rows of it.
                (LootKind::Reference, 900, 9),
                (LootKind::Reference, 900, 10),
            ]
        );
    }

    #[test]
    fn vendors_and_equipment_belong_to_the_selected_creatures() {
        let w = select(&db(), &Fake);
        let stock: Vec<_> = w.vendors.iter().map(|v| (v.npc, v.item)).collect();
        assert_eq!(
            stock,
            vec![(1213, 2070), (1213, 159)],
            "the rabbit's row is out; the shop keeps the database's order"
        );
        assert_eq!(ids(&w.equipment, |e| e.entry), vec![4]);
    }

    #[test]
    fn areas_are_the_distinct_areas_under_the_selected_spawns_by_id() {
        let w = select(&db(), &Fake);
        assert_eq!(
            w.areas,
            vec![
                (9, "Northshire Valley".to_string()),
                (86, "Echo Ridge Mine".to_string())
            ]
        );
    }

    #[test]
    fn scripts_are_those_the_selected_quests_name_tagged_with_the_quest() {
        let w = select(&db(), &Fake);
        let rows: Vec<_> = w
            .scripts
            .iter()
            .map(|s| (s.quest, s.start, s.delay, s.dataint[0]))
            .collect();
        assert_eq!(
            rows,
            vec![
                // Quest 7's end script 7 (the start script 7 is another table's row and is not named).
                (7, false, 0, 11),
                // Quest 54's start script, in the table's order (delay 2 before delay 1).
                (54, true, 2, 1242),
                (54, true, 1, 6245),
            ],
            "script 999 is nobody's; start and end scripts of the same id are told apart"
        );
    }

    #[test]
    fn two_quests_naming_one_script_each_get_its_rows() {
        let mut d = db();
        d.quests
            .iter_mut()
            .find(|q| q.id == 200)
            .unwrap()
            .start_script = 54;
        let w = select(&d, &Fake);
        let of_54: Vec<_> = w
            .scripts
            .iter()
            .filter(|s| s.start)
            .map(|s| s.quest)
            .collect();
        assert_eq!(of_54, vec![54, 54, 200, 200]);
    }

    #[test]
    fn item_extras_follow_the_selected_items_and_the_xp_table_is_taken_whole() {
        let w = select(&db(), &Fake);
        // 159 and 752 are selected items; 999 and 5000 are not (999 is unrelated equipment, 5000 has no item).
        assert_eq!(
            w.item_extras,
            vec![
                ItemExtra {
                    entry: 159,
                    start_quest: 5805,
                    bonding: 4
                },
                ItemExtra {
                    entry: 752,
                    start_quest: 0,
                    bonding: 4
                }
            ]
        );
        assert_eq!(w.xp_for_level, vec![400, 900, 1400]);
    }

    #[test]
    fn the_level_and_class_tables_are_taken_whole() {
        let w = select(&db(), &Fake);
        assert_eq!(w.level_stats.len(), 1);
        assert_eq!(w.class_stats.len(), 1);
    }

    #[test]
    fn the_result_does_not_depend_on_the_order_of_the_database_rows() {
        let a = select(&db(), &Fake);
        let mut d = db();
        d.spawns.reverse();
        d.creatures.reverse();
        d.quests.reverse();
        d.creature_starts.reverse();
        d.creature_ends.reverse();
        d.object_starts.reverse();
        d.object_ends.reverse();
        d.items.reverse();
        d.objects.reverse();
        d.object_spawns.reverse();
        d.loot.reverse();
        d.equipment.reverse();
        d.item_extras.reverse();
        // (The vendors and the rows of a script keep the database's order on purpose, so they are left
        // as they are.)
        let b = select(&d, &Fake);
        assert_eq!(a, b);
    }

    #[test]
    fn selecting_twice_gives_the_same_world() {
        assert_eq!(select(&db(), &Fake), select(&db(), &Fake));
    }

    #[test]
    fn a_valley_with_nothing_in_it_keeps_only_the_zone_quests_and_their_npcs() {
        struct Nothing;
        impl InValley for Nothing {
            fn area_of(&self, _: f32, _: f32) -> u32 {
                0
            }
            fn contains(&self, _: f32, _: f32) -> bool {
                false
            }
            fn name(&self, _: u32) -> String {
                String::new()
            }
        }
        let w = select(&db(), &Nothing);
        assert!(w.spawns.is_empty() && w.object_spawns.is_empty() && w.areas.is_empty());
        assert_eq!(ids(&w.quests, |q| q.id), vec![7, 54]);
        assert_eq!(ids(&w.creatures, |c| c.entry), vec![197, 240]);
        assert_eq!(ids(&w.objects, |o| o.entry), vec![63, 64]);
    }

    #[test]
    fn a_duplicated_relation_row_is_written_once() {
        let mut d = db();
        d.creature_starts.push(rel(197, 7, false));
        let w = select(&d, &Fake);
        assert_eq!(w.starts.iter().filter(|r| r.quest == 7).count(), 1);
    }

    fn waypoint(guid: u32, path_id: u32, point: u32, x: f32, y: f32) -> Waypoint {
        Waypoint {
            guid,
            point,
            pos: [x, y, 70.0],
            orientation: 0.0,
            wait_ms: 0,
            script_id: 0,
            path_id,
        }
    }

    /// `db()` plus: a walking thug (guid 7, movement 2) inside with a three-point path stored out of
    /// order and a stop of four seconds; the thug of guid 1 with two paths (path ids 0 and 1) stored
    /// backwards; a walking rabbit (guid 8, movement 2) outside; paths for the event spawn 4 and for
    /// a guid nobody has.
    fn db_with_paths() -> Db {
        let mut d = db();
        let mut walker = spawn(7, 38, -8920.0, -60.0);
        walker.movement = 2;
        let mut outside = spawn(8, 721, -9000.0, 60.0);
        outside.movement = 2;
        d.spawns.push(walker);
        d.spawns.push(outside);
        let mut stop = waypoint(7, 0, 2, -8921.0, -61.0);
        stop.wait_ms = 4000;
        d.waypoints = vec![
            waypoint(7, 0, 3, -8922.0, -62.0),
            waypoint(7, 0, 1, -8920.0, -60.0),
            stop,
            waypoint(1, 1, 5, -8901.0, -101.0),
            waypoint(1, 1, 2, -8902.0, -102.0),
            waypoint(1, 0, 9, -8903.0, -103.0),
            waypoint(8, 0, 1, -9000.0, 60.0),
            waypoint(8, 0, 2, -9001.0, 61.0),
            waypoint(4, 0, 1, -8900.0, -101.0),
            waypoint(999, 0, 1, 0.0, 0.0),
        ];
        d
    }

    #[test]
    fn a_movement_2_spawn_keeps_its_path_and_the_outside_ones_are_dropped() {
        let w = select(&db_with_paths(), &Fake);
        assert!(w.spawns.iter().any(|s| s.guid == 7 && s.movement == 2));
        let rows: Vec<_> = w
            .waypoints
            .iter()
            .map(|p| (p.guid, p.path_id, p.point))
            .collect();
        // Sorted by (guid, path id, point); the rabbit 8 (outside), the event spawn 4 and the
        // unknown guid 999 are gone.
        assert_eq!(
            rows,
            vec![
                (1, 0, 9),
                (1, 1, 2),
                (1, 1, 5),
                (7, 0, 1),
                (7, 0, 2),
                (7, 0, 3)
            ]
        );
        // The rows themselves are untouched.
        let stop = &w.waypoints[4];
        assert_eq!((stop.wait_ms, stop.pos), (4000, [-8921.0, -61.0, 70.0]));
    }

    #[test]
    fn the_paths_do_not_depend_on_the_order_of_the_database_rows() {
        let a = select(&db_with_paths(), &Fake);
        let mut d = db_with_paths();
        d.waypoints.reverse();
        d.spawns.reverse();
        assert_eq!(a, select(&d, &Fake));
    }

    #[test]
    fn a_world_without_movement_rows_has_no_waypoints() {
        assert!(select(&db(), &Fake).waypoints.is_empty());
    }

    #[test]
    fn spawns_without_path_names_the_walkers_that_have_no_rows() {
        let w = select(&db_with_paths(), &Fake);
        assert!(spawns_without_path(&w).is_empty());

        // Take the walker's path away: it is reported (its guid), nothing else is.
        let mut bare = w.clone();
        bare.waypoints.retain(|p| p.guid != 7);
        assert_eq!(spawns_without_path(&bare), vec![7]);

        // A second walker without rows; the answer is sorted by guid.
        let mut two = bare.clone();
        let mut other = two.spawns[0].clone();
        other.guid = 2;
        other.movement = 2;
        two.spawns.push(other);
        assert_eq!(spawns_without_path(&two), vec![2, 7]);

        // A spawn that only stands (0) or wanders (1) needs no path.
        let mut wanders = bare;
        wanders.spawns.iter_mut().for_each(|s| s.movement = 1);
        assert!(spawns_without_path(&wanders).is_empty());
    }
    #[test]
    fn a_named_spawn_outside_the_valley_is_selected_with_its_template() {
        let mut d = db();
        let mut named = spawn(NAMED_SPAWNS[0], 448, -9947.88, 594.77);
        named.pos[2] = 39.61;
        d.spawns.push(named);
        d.creatures.push(creature(448, "Hogger", 0, 0));
        let w = select(&d, &Fake);
        let hogger = w
            .spawns
            .iter()
            .find(|s| s.guid == NAMED_SPAWNS[0])
            .expect("the named spawn");
        assert_eq!(hogger.area, 0);
        assert_eq!(hogger.pos, [-9947.88, 594.77, 39.61]);
        assert!(w.creatures.iter().any(|c| c.entry == 448));
        // The valley's own selection is unchanged, and a named spawn that is an event one stays out.
        assert_eq!(
            ids(&w.spawns, |s| s.guid),
            vec![1, 3, 5, 6, NAMED_SPAWNS[0]]
        );
        let mut seasonal = db();
        let mut event_named = spawn(NAMED_SPAWNS[0], 448, -9947.88, 594.77);
        event_named.event = true;
        seasonal.spawns.push(event_named);
        seasonal.creatures.push(creature(448, "Hogger", 0, 0));
        let w = select(&seasonal, &Fake);
        assert!(w.spawns.iter().all(|s| s.guid != NAMED_SPAWNS[0]));
    }
    #[test]
    fn the_trial_gear_is_selected_with_the_items_even_when_nothing_else_names_it() {
        let mut d = db();
        for (_, id) in crate::equipment::TRIAL_GEAR {
            d.items.push(item(id));
        }
        let w = select(&d, &Fake);
        for (_, id) in crate::equipment::TRIAL_GEAR {
            assert!(w.items.iter().any(|i| i.entry == id), "item {id}");
        }
        // Only those: another item the database has stays out.
        d = db();
        d.items.push(item(1220));
        assert!(select(&d, &Fake).items.iter().all(|i| i.entry != 1220));
    }
}
