//! The exported tables, typed. `Db::load` reads `cache/mangos/tsv/<name>.tsv` for every table
//! `tools/export-mangos.ps1` writes; the `read_*` functions turn one parsed table into model rows
//! and are public so each can be tested on a fixture without files.

use crate::model::*;
use crate::tsv::{Row, Table};
use anyhow::{anyhow, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Every exported table, typed. Quests and items are the newest patch <= 10 row (the export already
/// applied the rule); the loaders do not filter again.
pub struct Db {
    pub spawns: Vec<Spawn>,
    pub creatures: Vec<Creature>,
    pub creature_meta: Vec<CreatureMeta>,
    pub spawn_pools: Vec<SpawnPool>,
    pub quests: Vec<Quest>,
    pub creature_starts: Vec<Relation>,
    pub creature_ends: Vec<Relation>,
    pub object_starts: Vec<Relation>,
    pub object_ends: Vec<Relation>,
    pub items: Vec<Item>,
    pub objects: Vec<ObjectTemplate>,
    pub object_spawns: Vec<ObjectSpawn>,
    pub loot: Vec<LootRow>,
    pub vendors: Vec<Vendor>,
    pub equipment: Vec<Equipment>,
    /// GUID paths from `creature_movement`, then entry-template paths expanded for walkers
    /// without a GUID path. The selected world sorts these by GUID, path and point.
    pub waypoints: Vec<Waypoint>,
    /// `quest_start_scripts` then `quest_end_scripts`, each row with the script `id` it belongs to
    /// (`QuestScript::quest` is 0 here: `select` fills it in from the quests' `StartScript` / `CompleteScript`).
    pub scripts: Vec<(u32, QuestScript)>,
    /// `player_xp_for_level`, index = level - 1.
    pub xp_for_level: Vec<u32>,
    /// The `item_template` columns of [`ItemExtra`], every item.
    pub item_extras: Vec<ItemExtra>,
    /// `player_levelstats` (race 1), sorted by (class, level).
    pub level_stats: Vec<LevelStat>,
    /// `player_classlevelstats`, sorted by (class, level).
    pub class_stats: Vec<ClassStat>,
}

impl Db {
    /// Read `dir/<name>.tsv` for every table the export writes.
    pub fn load(dir: &Path) -> Result<Db> {
        let table = |name: &str| Table::load(&dir.join(format!("{name}.tsv")));
        let in_file = |name: &str| format!("reading {name}.tsv");

        let event_creature = table("game_event_creature")?;
        let event_object = table("game_event_gameobject")?;
        let event_quest = table("game_event_quest")?;

        let spawns = read_spawns(&table("creature")?, &event_creature)
            .with_context(|| in_file("creature"))?;
        let creature_template = table("creature_template")?;
        let creatures =
            read_creatures(&creature_template).with_context(|| in_file("creature_template"))?;
        let creature_meta =
            read_creature_meta(&creature_template).with_context(|| in_file("creature_template"))?;
        let spawn_pools = crate::pool::load_spawn_pools(dir, &spawns)?;
        let quests = read_quests(&table("quest_template")?, &event_quest)
            .with_context(|| in_file("quest_template"))?;
        let relations = |name: &str, object: bool| -> Result<Vec<Relation>> {
            read_relations(&table(name)?, object).with_context(|| in_file(name))
        };
        let creature_starts = relations("creature_questrelation", false)?;
        let creature_ends = relations("creature_involvedrelation", false)?;
        let object_starts = relations("gameobject_questrelation", true)?;
        let object_ends = relations("gameobject_involvedrelation", true)?;
        let items =
            read_items(&table("item_template")?).with_context(|| in_file("item_template"))?;
        let objects = read_objects(&table("gameobject_template")?)
            .with_context(|| in_file("gameobject_template"))?;
        let object_spawns = read_object_spawns(&table("gameobject")?, &event_object)
            .with_context(|| in_file("gameobject"))?;
        let mut loot = Vec::new();
        for (name, kind) in [
            ("creature_loot_template", LootKind::Creature),
            ("gameobject_loot_template", LootKind::Object),
            ("reference_loot_template", LootKind::Reference),
        ] {
            loot.extend(read_loot(&table(name)?, kind).with_context(|| in_file(name))?);
        }
        let vendors = read_vendors(&table("npc_vendor")?).with_context(|| in_file("npc_vendor"))?;
        let equipment = read_equipment(&table("creature_equip_template")?)
            .with_context(|| in_file("creature_equip_template"))?;
        let template_path = dir.join("creature_movement_template.tsv");
        let movement_templates = if template_path
            .try_exists()
            .with_context(|| format!("checking {}", template_path.display()))?
        {
            Some(Table::load(&template_path)?)
        } else {
            None
        };
        let waypoints = read_waypoints_with_templates(
            &table("creature_movement")?,
            movement_templates.as_ref(),
            &spawns,
        )
        .with_context(|| in_file("creature_movement"))?;
        let mut scripts = read_scripts(&table("quest_start_scripts")?, true)
            .with_context(|| in_file("quest_start_scripts"))?;
        scripts.extend(
            read_scripts(&table("quest_end_scripts")?, false)
                .with_context(|| in_file("quest_end_scripts"))?,
        );
        let xp_for_level = read_xp_for_level(&table("player_xp_for_level")?)
            .with_context(|| in_file("player_xp_for_level"))?;
        let item_extras =
            read_item_extras(&table("item_template")?).with_context(|| in_file("item_template"))?;
        let level_stats = read_level_stats(&table("player_levelstats")?)
            .with_context(|| in_file("player_levelstats"))?;
        let class_stats = read_class_stats(&table("player_classlevelstats")?)
            .with_context(|| in_file("player_classlevelstats"))?;
        Ok(Db {
            spawns,
            creatures,
            creature_meta,
            spawn_pools,
            quests,
            creature_starts,
            creature_ends,
            object_starts,
            object_ends,
            items,
            objects,
            object_spawns,
            loot,
            vendors,
            equipment,
            waypoints,
            scripts,
            xp_for_level,
            item_extras,
            level_stats,
            class_stats,
        })
    }
}

/// Map every row, naming the row number when one fails.
fn each<T>(t: &Table, f: impl Fn(&Row) -> Result<T>) -> Result<Vec<T>> {
    t.rows()
        .enumerate()
        .map(|(n, r)| f(&r).with_context(|| format!("row {}", n + 1)))
        .collect()
}

/// The set of ids in one column (the game-event tables).
fn id_set(t: &Table, col: &str) -> Result<HashSet<u32>> {
    t.rows().map(|r| r.u32(col)).collect()
}

/// The columns `<prefix>1..=<prefix>N`, read one by one in slot order.
fn numbered<T>(
    r: &Row,
    prefix: &str,
    n: usize,
    get: impl Fn(&Row, &str) -> Result<T>,
) -> Result<Vec<T>> {
    (1..=n).map(|i| get(r, &format!("{prefix}{i}"))).collect()
}

/// Slots `(<ids>i, <counts>i)` for i in 1..=n, dropping the ones whose id is 0.
fn item_counts(r: &Row, ids: &str, counts: &str, n: usize) -> Result<Vec<ItemCount>> {
    let mut v = Vec::new();
    for i in 1..=n {
        let id = r.u32(&format!("{ids}{i}"))?;
        if id != 0 {
            v.push((id, r.u32(&format!("{counts}{i}"))?));
        }
    }
    Ok(v)
}

/// An `int unsigned` column that holds `-1` as 4294967295 (a gameobject's `data5`): the same 32 bits, signed.
fn i32_bits(r: &Row, col: &str) -> Result<i32> {
    let t = r.text(col)?;
    if t.is_empty() {
        return Ok(0);
    }
    let v: i64 = t
        .parse()
        .map_err(|e| anyhow!("column `{col}` = `{t}`: {e}"))?;
    i32::try_from(v)
        .or_else(|_| u32::try_from(v).map(|u| u as i32))
        .map_err(|_| anyhow!("column `{col}` = `{t}` does not fit 32 bits"))
}

fn array4<T: Copy>(v: Vec<T>) -> [T; 4] {
    [v[0], v[1], v[2], v[3]]
}

pub fn read_spawns(spawns: &Table, events: &Table) -> Result<Vec<Spawn>> {
    let in_event = id_set(events, "guid")?;
    each(spawns, |r| {
        let guid = r.u32("guid")?;
        let movement = r.u32("movement_type")?;
        Ok(Spawn {
            guid,
            entry: r.u32("id")?,
            alt: [r.u32("id2")?, r.u32("id3")?, r.u32("id4")?, r.u32("id5")?],
            pos: [
                r.f32("position_x")?,
                r.f32("position_y")?,
                r.f32("position_z")?,
            ],
            orientation: r.f32("orientation")?,
            respawn_min: r.u32("spawntimesecsmin")?,
            respawn_max: r.u32("spawntimesecsmax")?,
            wander: r.f32("wander_distance")?,
            movement: u8::try_from(movement)
                .map_err(|_| anyhow!("movement_type {movement} does not fit a byte"))?,
            area: 0,
            event: in_event.contains(&guid),
        })
    })
}

pub fn read_creatures(t: &Table) -> Result<Vec<Creature>> {
    each(t, |r| {
        Ok(Creature {
            entry: r.u32("entry")?,
            name: r.text("name")?,
            subname: r.text("subname")?,
            level_min: r.u32("level_min")?,
            level_max: r.u32("level_max")?,
            faction: r.u32("faction")?,
            npc_flags: r.u32("npc_flags")?,
            display: array4(numbered(r, "display_id", 4, |r, c| r.u32(c))?),
            scale: array4(numbered(r, "display_scale", 4, |r, c| r.f32(c))?),
            kind: r.u32("type")?,
            rank: r.u32("rank")?,
            detection_range: r.f32("detection_range")?,
            call_for_help_range: r.f32("call_for_help_range")?,
            leash_range: r.f32("leash_range")?,
            speed_walk: r.f32("speed_walk")?,
            speed_run: r.f32("speed_run")?,
            loot_id: r.u32("loot_id")?,
            equipment_id: r.u32("equipment_id")?,
            vendor_id: r.u32("vendor_id")?,
            trainer_type: r.u32("trainer_type")?,
            trainer_class: r.u32("trainer_class")?,
            movement_type: r.u32("movement_type")?,
            civilian: r.u32("civilian")? != 0,
            health_multiplier: r.f32("health_multiplier")?,
            damage_multiplier: r.f32("damage_multiplier")?,
            unit_class: r.u32("unit_class")?,
        })
    })
}

/// Read display probabilities from new exports; older exports keep zero weights.
/// Faction reactions require the WoW faction DBC and are filled by the importer.
pub fn read_creature_meta(t: &Table) -> Result<Vec<CreatureMeta>> {
    each(t, |r| {
        let mut display_weights = [0; 4];
        for (i, weight) in display_weights.iter_mut().enumerate() {
            let column = format!("display_probability{}", i + 1);
            if t.columns.contains(&column) {
                *weight = r.u32(&column)?;
            }
        }
        Ok(CreatureMeta {
            entry: r.u32("entry")?,
            display_weights,
            reaction: None,
        })
    })
}

pub fn read_quests(t: &Table, events: &Table) -> Result<Vec<Quest>> {
    let in_event = id_set(events, "quest")?;
    each(t, |r| {
        let id = r.u32("entry")?;
        let mut req_kills = Vec::new();
        for i in 1..=4 {
            let target = r.i32(&format!("ReqCreatureOrGOId{i}"))?;
            if target != 0 {
                req_kills.push((target, r.u32(&format!("ReqCreatureOrGOCount{i}"))?));
            }
        }
        Ok(Quest {
            id,
            patch: r.u32("patch")?,
            title: r.text("Title")?,
            details: r.text("Details")?,
            objectives: r.text("Objectives")?,
            offer_reward: r.text("OfferRewardText")?,
            request_items: r.text("RequestItemsText")?,
            end_text: r.text("EndText")?,
            objective_text: [
                r.text("ObjectiveText1")?,
                r.text("ObjectiveText2")?,
                r.text("ObjectiveText3")?,
                r.text("ObjectiveText4")?,
            ],
            zone_or_sort: r.i32("ZoneOrSort")?,
            min_level: r.u32("MinLevel")?,
            level: r.i32("QuestLevel")?,
            required_classes: r.u32("RequiredClasses")?,
            required_races: r.u32("RequiredRaces")?,
            prev: r.i32("PrevQuestId")?,
            next: r.i32("NextQuestId")?,
            exclusive_group: r.i32("ExclusiveGroup")?,
            next_in_chain: r.u32("NextQuestInChain")?,
            flags: r.u32("QuestFlags")?,
            special_flags: r.u32("SpecialFlags")?,
            limit_time: r.u32("LimitTime")?,
            src_item: r.u32("SrcItemId")?,
            src_item_count: r.u32("SrcItemCount")?,
            req_items: item_counts(r, "ReqItemId", "ReqItemCount", 4)?,
            req_kills,
            req_spell_cast: numbered(r, "ReqSpellCast", 4, |r, c| r.u32(c))?
                .into_iter()
                .filter(|&s| s != 0)
                .collect(),
            reward_choice: item_counts(r, "RewChoiceItemId", "RewChoiceItemCount", 6)?,
            reward_items: item_counts(r, "RewItemId", "RewItemCount", 4)?,
            reward_money: r.i32("RewOrReqMoney")?,
            reward_xp: r.u32("RewXP")?,
            reward_spell: r.u32("RewSpell")?,
            point: (r.u32("PointMapId")?, r.f32("PointX")?, r.f32("PointY")?),
            start_script: r.u32("StartScript")?,
            complete_script: r.u32("CompleteScript")?,
            event: in_event.contains(&id),
            max_level: r.u32("MaxLevel")?,
            required_condition: r.u32("RequiredCondition")?,
            rep_objective: (r.u32("RepObjectiveFaction")?, r.i32("RepObjectiveValue")?),
            rep_min: (
                r.u32("RequiredMinRepFaction")?,
                r.i32("RequiredMinRepValue")?,
            ),
            rep_max: (
                r.u32("RequiredMaxRepFaction")?,
                r.i32("RequiredMaxRepValue")?,
            ),
            reward_rep: reward_rep(r)?,
            reward_mail: (
                r.u32("RewMailTemplateId")?,
                r.u32("RewMailDelaySecs")?,
                r.u32("RewMailMoney")?,
            ),
        })
    })
}

/// `(RewRepFaction<i>, RewRepValue<i>)` for i in 1..=5, dropping the slots whose faction is 0.
fn reward_rep(r: &Row) -> Result<Vec<(u32, i32)>> {
    let mut v = Vec::new();
    for i in 1..=5 {
        let faction = r.u32(&format!("RewRepFaction{i}"))?;
        if faction != 0 {
            v.push((faction, r.i32(&format!("RewRepValue{i}"))?));
        }
    }
    Ok(v)
}

pub fn read_relations(t: &Table, object: bool) -> Result<Vec<Relation>> {
    each(t, |r| {
        Ok(Relation {
            npc: r.u32("id")?,
            quest: r.u32("quest")?,
            object,
        })
    })
}

pub fn read_items(t: &Table) -> Result<Vec<Item>> {
    each(t, |r| {
        Ok(Item {
            entry: r.u32("entry")?,
            name: r.text("name")?,
            class: r.u32("class")?,
            subclass: r.u32("subclass")?,
            display: r.u32("display_id")?,
            quality: r.u32("quality")?,
            flags: r.u32("flags")?,
            buy_price: r.u32("buy_price")?,
            sell_price: r.u32("sell_price")?,
            inventory_type: r.u32("inventory_type")?,
            allowable_class: r.i32("allowable_class")?,
            allowable_race: r.i32("allowable_race")?,
            item_level: r.u32("item_level")?,
            required_level: r.u32("required_level")?,
            max_count: r.u32("max_count")?,
            stackable: r.u32("stackable")?,
            delay: r.u32("delay")?,
            damage: (r.f32("dmg_min1")?, r.f32("dmg_max1")?, r.u32("dmg_type1")?),
            block: r.u32("block")?,
            // One test item ("Resist Test Item") carries -50 armor; the model has no negative armor.
            armor: u32::try_from(r.i32("armor")?).unwrap_or(0),
            stats: read_stats(r)?,
        })
    })
}

/// `stat_type1..10` / `stat_value1..10`: the pairs with a non-zero value (a zero value is an empty slot,
/// including the all-zero default whose type 0 is "mana").
fn read_stats(r: &Row) -> Result<Vec<(u32, i32)>> {
    let mut v = Vec::new();
    for i in 1..=10 {
        let value = r.i32(&format!("stat_value{i}"))?;
        if value != 0 {
            v.push((r.u32(&format!("stat_type{i}"))?, value));
        }
    }
    Ok(v)
}

/// `player_levelstats` for race 1: one row per (class, level), sorted.
pub fn read_level_stats(t: &Table) -> Result<Vec<LevelStat>> {
    let mut rows = each(t, |r| {
        Ok(LevelStat {
            class: r.u32("class")?,
            level: r.u32("level")?,
            strength: r.u32("str")?,
            agility: r.u32("agi")?,
            stamina: r.u32("sta")?,
            intellect: r.u32("inte")?,
            spirit: r.u32("spi")?,
        })
    })?;
    rows.sort_by_key(|r| (r.class, r.level));
    Ok(rows)
}

/// `player_classlevelstats`: one row per (class, level), sorted.
pub fn read_class_stats(t: &Table) -> Result<Vec<ClassStat>> {
    let mut rows = each(t, |r| {
        Ok(ClassStat {
            class: r.u32("class")?,
            level: r.u32("level")?,
            base_hp: r.u32("basehp")?,
            base_mana: r.u32("basemana")?,
        })
    })?;
    rows.sort_by_key(|r| (r.class, r.level));
    Ok(rows)
}

pub fn read_objects(t: &Table) -> Result<Vec<ObjectTemplate>> {
    each(t, |r| {
        Ok(ObjectTemplate {
            entry: r.u32("entry")?,
            name: r.text("name")?,
            kind: r.u32("type")?,
            display: r.u32("displayId")?,
            faction: r.u32("faction")?,
            flags: r.u32("flags")?,
            size: r.f32("size")?,
            data: [
                i32_bits(r, "data0")?,
                i32_bits(r, "data1")?,
                i32_bits(r, "data2")?,
                i32_bits(r, "data3")?,
                i32_bits(r, "data4")?,
                i32_bits(r, "data5")?,
                i32_bits(r, "data6")?,
                i32_bits(r, "data7")?,
                i32_bits(r, "data8")?,
                i32_bits(r, "data9")?,
            ],
        })
    })
}

/// `item_template.start_quest` and `bonding` of every row.
pub fn read_item_extras(t: &Table) -> Result<Vec<ItemExtra>> {
    each(t, |r| {
        Ok(ItemExtra {
            entry: r.u32("entry")?,
            start_quest: r.u32("start_quest")?,
            bonding: r.u32("bonding")?,
        })
    })
}

/// `quest_start_scripts` (`start`) or `quest_end_scripts`: every row with its script `id`; the
/// `quest` of each [`QuestScript`] is 0 until `select` knows which quest names the script.
pub fn read_scripts(t: &Table, start: bool) -> Result<Vec<(u32, QuestScript)>> {
    each(t, |r| {
        Ok((
            r.u32("id")?,
            QuestScript {
                quest: 0,
                start,
                delay: r.u32("delay")?,
                priority: r.u32("priority")?,
                command: r.u32("command")?,
                datalong: [
                    r.u32("datalong")?,
                    r.u32("datalong2")?,
                    r.u32("datalong3")?,
                    r.u32("datalong4")?,
                ],
                target_param: [r.u32("target_param1")?, r.u32("target_param2")?],
                target_type: r.u32("target_type")?,
                data_flags: r.u32("data_flags")?,
                dataint: [
                    r.i32("dataint")?,
                    r.i32("dataint2")?,
                    r.i32("dataint3")?,
                    r.i32("dataint4")?,
                ],
                pos: [r.f32("x")?, r.f32("y")?, r.f32("z")?, r.f32("o")?],
                condition_id: r.u32("condition_id")?,
                comments: r.text("comments")?,
            },
        ))
    })
}

/// `player_xp_for_level`: `lvl` 1, 2, 3 ... (in any row order) with the XP to the next level. The
/// levels must be consecutive from 1, so the index is the level minus one.
pub fn read_xp_for_level(t: &Table) -> Result<Vec<u32>> {
    let mut rows = each(t, |r| Ok((r.u32("lvl")?, r.u32("xp_for_next_level")?)))?;
    rows.sort_unstable();
    for (i, &(lvl, _)) in rows.iter().enumerate() {
        if lvl as usize != i + 1 {
            return Err(anyhow!(
                "player_xp_for_level: expected level {} after {} row(s), found {lvl}",
                i + 1,
                i
            ));
        }
    }
    Ok(rows.into_iter().map(|(_, xp)| xp).collect())
}

pub fn read_object_spawns(spawns: &Table, events: &Table) -> Result<Vec<ObjectSpawn>> {
    let in_event = id_set(events, "guid")?;
    each(spawns, |r| {
        let guid = r.u32("guid")?;
        Ok(ObjectSpawn {
            guid,
            entry: r.u32("id")?,
            pos: [
                r.f32("position_x")?,
                r.f32("position_y")?,
                r.f32("position_z")?,
            ],
            orientation: r.f32("orientation")?,
            // A negative object respawn time marks an object that is not spawned by default (MaNGOS
            // negates it back); the delay is kept, none of those rows lies in Northshire.
            respawn_min: r.i32("spawntimesecsmin")?.unsigned_abs(),
            respawn_max: r.i32("spawntimesecsmax")?.unsigned_abs(),
            area: 0,
            event: in_event.contains(&guid),
        })
    })
}

pub fn read_loot(t: &Table, kind: LootKind) -> Result<Vec<LootRow>> {
    each(t, |r| {
        Ok(LootRow {
            kind,
            table: r.u32("entry")?,
            item: r.u32("item")?,
            chance: r.f32("ChanceOrQuestChance")?,
            group: r.u32("groupid")?,
            min_or_ref: r.i32("mincountOrRef")?,
            max_count: r.u32("maxcount")?,
        })
    })
}

pub fn read_vendors(t: &Table) -> Result<Vec<Vendor>> {
    each(t, |r| {
        Ok(Vendor {
            npc: r.u32("entry")?,
            item: r.u32("item")?,
            max_count: r.u32("maxcount")?,
            incr_time: r.u32("incrtime")?,
        })
    })
}

/// `creature_movement`: `id` is the creature spawn's guid, `waittime` is milliseconds. The table's
/// own `wander_distance` per point is not modelled.
pub fn read_waypoints(t: &Table) -> Result<Vec<Waypoint>> {
    each(t, |r| read_waypoint(r, r.u32("id")?))
}

fn read_waypoint(r: &Row, guid: u32) -> Result<Waypoint> {
    Ok(Waypoint {
        guid,
        point: r.u32("point")?,
        pos: [
            r.f32("position_x")?,
            r.f32("position_y")?,
            r.f32("position_z")?,
        ],
        orientation: r.f32("orientation")?,
        wait_ms: r.u32("waittime")?,
        script_id: r.u32("script_id")?,
        path_id: r.u32("path_id")?,
    })
}

/// GUID paths take precedence over the optional `creature_movement_template` entry paths.
/// Only movement-type 2 spawns without any GUID path receive their primary entry's template;
/// alternative creature entries do not change the database's path owner. Point order and
/// `path_id` are retained, and per-point `wander_distance` is unmodelled as in GUID paths.
pub fn read_waypoints_with_templates(
    guid_paths: &Table,
    templates: Option<&Table>,
    spawns: &[Spawn],
) -> Result<Vec<Waypoint>> {
    let mut waypoints = read_waypoints(guid_paths)?;
    let Some(templates) = templates else {
        return Ok(waypoints);
    };
    let rows = each(templates, |r| Ok((r.u32("entry")?, read_waypoint(r, 0)?)))
        .context("creature_movement_template")?;
    let mut by_entry: HashMap<u32, Vec<Waypoint>> = HashMap::new();
    for (entry, point) in rows {
        by_entry.entry(entry).or_default().push(point);
    }
    let mut has_path: HashSet<_> = waypoints.iter().map(|point| point.guid).collect();
    for spawn in spawns {
        if spawn.movement != 2 || has_path.contains(&spawn.guid) {
            continue;
        }
        let Some(points) = by_entry.get(&spawn.entry) else {
            continue;
        };
        waypoints.extend(points.iter().map(|point| Waypoint {
            guid: spawn.guid,
            ..point.clone()
        }));
        has_path.insert(spawn.guid);
    }
    Ok(waypoints)
}

pub fn read_equipment(t: &Table) -> Result<Vec<Equipment>> {
    each(t, |r| {
        Ok(Equipment {
            entry: r.u32("entry")?,
            probability: r.u32("probability")?,
            items: [r.u32("item1")?, r.u32("item2")?, r.u32("item3")?],
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tsv::Table;

    #[test]
    fn creature_metadata_preserves_display_weights_without_guessing_reaction() {
        let table = Table::parse(
            "entry\tdisplay_probability1\tdisplay_probability2\tdisplay_probability3\tdisplay_probability4\n\
             38\t10\t25\t0\t65\n\
             721\t0\t0\t0\t0\n",
        )
        .unwrap();
        let metadata = read_creature_meta(&table).unwrap();
        assert_eq!(metadata[0].entry, 38);
        assert_eq!(metadata[0].display_weights, [10, 25, 0, 65]);
        assert_eq!(metadata[1].display_weights, [0; 4]);
        assert!(metadata.iter().all(|meta| meta.reaction.is_none()));
    }

    #[test]
    fn old_and_partial_exports_keep_missing_display_weights_zero() {
        for (text, expected) in [
            ("entry\n38\n", [0; 4]),
            ("entry\tdisplay_probability2\n38\t40\n", [0, 40, 0, 0]),
        ] {
            let metadata = read_creature_meta(&Table::parse(text).unwrap()).unwrap();
            assert_eq!(metadata[0].display_weights, expected);
            assert_eq!(metadata[0].reaction, None);
        }
        let table = Table::parse("entry\tdisplay_probability1\n38\t-1\n").unwrap();
        assert!(read_creature_meta(&table).is_err());
    }

    #[test]
    fn spawns_read_every_column_and_flag_events() {
        let spawns = Table::parse(
            "guid\tid\tid2\tid3\tid4\tid5\tmap\tposition_x\tposition_y\tposition_z\torientation\tspawntimesecsmin\tspawntimesecsmax\twander_distance\tmovement_type\tspawn_flags\n\
             80247\t103\t0\t0\t0\t0\t0\t-9056.48\t-460.903\t72.8318\t1.5\t270\t270\t0\t0\t0\n\
             5\t721\t0\t0\t0\t0\t0\t-9000\t-300\t70\t0\t120\t180\t5\t1\t0\n",
        )
        .unwrap();
        let events = Table::parse("guid\tevent\n5\t12\n").unwrap();
        let v = read_spawns(&spawns, &events).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].entry, 103);
        assert_eq!(v[0].pos, [-9056.48, -460.903, 72.8318]);
        assert_eq!(
            (v[0].respawn_min, v[0].respawn_max, v[0].movement),
            (270, 270, 0)
        );
        assert!(!v[0].event);
        assert!(v[1].event, "guid 5 is in a game event");
        assert_eq!((v[1].wander, v[1].movement, v[1].area), (5.0, 1, 0));
    }

    #[test]
    fn spawns_keep_the_alternative_entries() {
        let spawns = Table::parse(
            "guid\tid\tid2\tid3\tid4\tid5\tmap\tposition_x\tposition_y\tposition_z\torientation\tspawntimesecsmin\tspawntimesecsmax\twander_distance\tmovement_type\tspawn_flags\n\
             9\t10\t11\t12\t0\t14\t0\t1\t2\t3\t0\t60\t60\t0\t0\t0\n",
        )
        .unwrap();
        let events = Table::parse("guid\tevent\n").unwrap();
        let v = read_spawns(&spawns, &events).unwrap();
        assert_eq!((v[0].entry, v[0].alt), (10, [11, 12, 0, 14]));
    }

    #[test]
    fn quest_reads_objectives_rewards_and_signed_kill_ids() {
        let q = read_quests(&quest_table(), &no_events("quest")).unwrap();
        assert_eq!(q.len(), 1);
        let q = &q[0];
        assert_eq!(
            (q.id, q.title.as_str(), q.exclusive_group, q.reward_xp),
            (18, "Brotherhood of Thieves", -18, 355)
        );
        assert_eq!(q.details, "Hello $N,\nthis is a test");
        assert_eq!(q.req_items, vec![(752, 12)]);
        assert_eq!(
            q.req_kills,
            vec![(-6, 3)],
            "a negative id is an object, kept signed"
        );
        assert_eq!(
            q.reward_choice,
            vec![(2224, 1), (5580, 1), (1161, 1), (5579, 1), (1159, 1)]
        );
        assert!(q.reward_items.is_empty());
        assert_eq!(q.prev, 783);
    }

    #[test]
    fn quest_reads_the_remaining_scalars_and_text_nulls() {
        let q = &read_quests(&quest_table(), &no_events("quest")).unwrap()[0];
        assert_eq!(
            (q.patch, q.zone_or_sort, q.min_level, q.level),
            (0, 9, 2, 4)
        );
        assert_eq!((q.next, q.next_in_chain), (3903, 0));
        assert_eq!(q.objectives, "Bring me 12 bandanas");
        assert_eq!(q.offer_reward, "Thanks");
        assert_eq!(q.request_items, "Not yet");
        assert_eq!(q.end_text, "", "NULL text reads as empty");
        assert_eq!(q.objective_text, <[String; 4]>::default());
        assert!(q.req_spell_cast.is_empty());
        assert_eq!(
            (q.reward_money, q.reward_spell, q.point),
            (0, 0, (0, 0.0, 0.0))
        );
        assert!(!q.event);
    }

    #[test]
    fn quest_reads_the_level_condition_reputation_and_mail_columns() {
        let q = &read_quests(&quest_table(), &no_events("quest")).unwrap()[0];
        assert_eq!((q.max_level, q.required_condition), (9, 8));
        assert_eq!(q.rep_objective, (7, 70));
        assert_eq!(q.rep_min, (6, -60), "a reputation value can be negative");
        assert_eq!(q.rep_max, (5, 50));
        assert_eq!(
            q.reward_rep,
            vec![(72, -25)],
            "slot 1 has no faction (its value is ignored), slot 2 pays -25 with faction 72"
        );
        assert_eq!(q.reward_mail, (11, 12, 13));
    }

    #[test]
    fn quest_in_a_game_event_is_flagged_and_zero_slots_are_dropped() {
        // Slot 2 filled, slot 1 empty: slots are collected by id, not by position.
        let row = QUEST_ROW.replacen("752\t0\t0\t0\t12\t0\t0\t0", "0\t753\t0\t0\t0\t7\t0\t0", 1);
        let t = Table::parse(&format!("{QUEST_HEAD}\n{row}\n")).unwrap();
        let events = Table::parse("quest\tevent\n18\t3\n").unwrap();
        let q = &read_quests(&t, &events).unwrap()[0];
        assert!(q.event, "quest 18 is in game_event_quest");
        assert_eq!(q.req_items, vec![(753, 7)]);
    }

    #[test]
    fn relations_carry_the_object_flag() {
        let t = Table::parse("id\tquest\n197\t7\n823\t6\n").unwrap();
        let creature = read_relations(&t, false).unwrap();
        assert_eq!(
            creature,
            vec![
                Relation {
                    npc: 197,
                    quest: 7,
                    object: false
                },
                Relation {
                    npc: 823,
                    quest: 6,
                    object: false
                }
            ]
        );
        let object = read_relations(&t, true).unwrap();
        assert!(object.iter().all(|r| r.object));
    }

    #[test]
    fn creatures_read_displays_flags_and_multipliers() {
        let thug = one_row(&[
            ("entry", "38"),
            ("name", "Defias Thug"),
            ("subname", "\\N"),
            ("level_min", "2"),
            ("level_max", "3"),
            ("faction", "12"),
            ("npc_flags", "0"),
            ("display_id1", "5035"),
            ("display_id2", "0"),
            ("display_id3", "0"),
            ("display_id4", "0"),
            ("display_scale1", "1"),
            ("display_scale2", "0"),
            ("display_scale3", "0"),
            ("display_scale4", "0"),
            ("type", "7"),
            ("rank", "0"),
            ("detection_range", "20"),
            ("call_for_help_range", "5"),
            ("leash_range", "0"),
            ("speed_walk", "1"),
            ("speed_run", "1.14286"),
            ("loot_id", "38"),
            ("equipment_id", "4"),
            ("vendor_id", "0"),
            ("trainer_id", "0"),
            ("trainer_type", "0"),
            ("trainer_class", "0"),
            ("movement_type", "1"),
            ("civilian", "0"),
            ("health_multiplier", "1.5"),
            ("damage_multiplier", "1.2"),
            ("unit_class", "1"),
        ]);
        let c = &read_creatures(&thug).unwrap()[0];
        assert_eq!(
            (c.entry, c.name.as_str(), c.subname.as_str()),
            (38, "Defias Thug", "")
        );
        assert_eq!((c.level_min, c.level_max, c.faction), (2, 3, 12));
        assert_eq!(c.display, [5035, 0, 0, 0]);
        assert_eq!(c.scale, [1.0, 0.0, 0.0, 0.0]);
        assert_eq!((c.kind, c.rank, c.loot_id, c.equipment_id), (7, 0, 38, 4));
        assert_eq!((c.detection_range, c.call_for_help_range), (20.0, 5.0));
        assert_eq!((c.speed_walk, c.speed_run), (1.0, 1.14286));
        assert_eq!((c.health_multiplier, c.damage_multiplier), (1.5, 1.2));
        assert_eq!((c.movement_type, c.unit_class), (1, 1));
        assert!(!c.civilian);

        // A civilian critter with a second display and a scale.
        let rabbit = one_row(&[
            ("entry", "721"),
            ("name", "Rabbit"),
            ("subname", "Critter"),
            ("level_min", "1"),
            ("level_max", "1"),
            ("faction", "31"),
            ("npc_flags", "0"),
            ("display_id1", "328"),
            ("display_id2", "329"),
            ("display_id3", "0"),
            ("display_id4", "0"),
            ("display_scale1", "0.7"),
            ("display_scale2", "0.8"),
            ("display_scale3", "0"),
            ("display_scale4", "0"),
            ("type", "8"),
            ("rank", "0"),
            ("detection_range", "10"),
            ("call_for_help_range", "0"),
            ("leash_range", "0"),
            ("speed_walk", "1"),
            ("speed_run", "1"),
            ("loot_id", "0"),
            ("equipment_id", "0"),
            ("vendor_id", "0"),
            ("trainer_id", "0"),
            ("trainer_type", "0"),
            ("trainer_class", "0"),
            ("movement_type", "1"),
            ("civilian", "1"),
            ("health_multiplier", "1"),
            ("damage_multiplier", "1"),
            ("unit_class", "1"),
        ]);
        let r = &read_creatures(&rabbit).unwrap()[0];
        assert_eq!((r.subname.as_str(), r.kind), ("Critter", 8));
        assert_eq!(r.display, [328, 329, 0, 0]);
        assert_eq!(r.scale, [0.7, 0.8, 0.0, 0.0]);
        assert!(r.civilian);
    }

    #[test]
    fn items_read_damage_and_signed_restrictions() {
        let stats_head: String = (1..=10)
            .map(|i| format!("\tstat_type{i}\tstat_value{i}"))
            .collect();
        let zeros = "\t0".repeat(20);
        let t = Table::parse(&format!(
            "entry\tclass\tsubclass\tname\tdisplay_id\tquality\tflags\tbuy_price\tsell_price\tinventory_type\tallowable_class\tallowable_race\titem_level\trequired_level\tmax_count\tstackable\tdelay\tdmg_min1\tdmg_max1\tdmg_type1\tblock\tarmor\tspellid_1\tspelltrigger_1{stats_head}\n\
             2092\t2\t15\tWorn Dagger\t6544\t1\t0\t35\t7\t13\t-1\t-1\t1\t0\t0\t1\t1600\t1\t3\t0\t0\t0\t0\t0{zeros}\n"
        ))
        .unwrap();
        let i = &read_items(&t).unwrap()[0];
        assert_eq!((i.entry, i.name.as_str()), (2092, "Worn Dagger"));
        assert_eq!(
            (i.class, i.subclass, i.display, i.quality),
            (2, 15, 6544, 1)
        );
        assert_eq!((i.allowable_class, i.allowable_race), (-1, -1));
        assert_eq!(i.damage, (1.0, 3.0, 0));
        assert_eq!((i.delay, i.buy_price, i.sell_price), (1600, 35, 7));
        assert!(i.stats.is_empty(), "stat_value 0 is an empty slot");
    }

    #[test]
    fn item_stats_keep_real_pairs_and_drop_empty_slots() {
        // Tribal Vest: +2 Agility (type 3), +1 Stamina (type 7); a mana stat (type 0) with a value is
        // real, type 0 with value 0 is an empty slot; a negative value is kept.
        let mut head: Vec<String> = [
            "entry",
            "class",
            "subclass",
            "name",
            "display_id",
            "quality",
            "flags",
            "buy_price",
            "sell_price",
            "inventory_type",
            "allowable_class",
            "allowable_race",
            "item_level",
            "required_level",
            "max_count",
            "stackable",
            "delay",
            "dmg_min1",
            "dmg_max1",
            "dmg_type1",
            "block",
            "armor",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let mut row: Vec<String> = [
            "3288",
            "4",
            "2",
            "Tribal Vest",
            "1",
            "2",
            "0",
            "0",
            "0",
            "5",
            "-1",
            "-1",
            "10",
            "8",
            "0",
            "1",
            "0",
            "0",
            "0",
            "0",
            "0",
            "65",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let stats = [(3, 2), (7, 1), (0, 5), (4, -2)];
        for i in 1..=10 {
            let (t, v) = stats.get(i - 1).copied().unwrap_or((0, 0));
            head.push(format!("stat_type{i}"));
            row.push(t.to_string());
            head.push(format!("stat_value{i}"));
            row.push(v.to_string());
        }
        let t = Table::parse(&format!("{}\n{}\n", head.join("\t"), row.join("\t"))).unwrap();
        let items = read_items(&t).unwrap();
        assert_eq!(items[0].stats, vec![(3, 2), (7, 1), (0, 5), (4, -2)]);
    }

    #[test]
    fn level_and_class_tables_read_by_class_and_level() {
        let t = Table::parse(
            "race\tclass\tlevel\tstr\tagi\tsta\tinte\tspi\n1\t1\t2\t24\t21\t23\t20\t20\n1\t1\t1\t23\t20\t22\t20\t20\n",
        )
        .unwrap();
        assert_eq!(
            read_level_stats(&t).unwrap(),
            vec![
                LevelStat {
                    class: 1,
                    level: 1,
                    strength: 23,
                    agility: 20,
                    stamina: 22,
                    intellect: 20,
                    spirit: 20
                },
                LevelStat {
                    class: 1,
                    level: 2,
                    strength: 24,
                    agility: 21,
                    stamina: 23,
                    intellect: 20,
                    spirit: 20
                },
            ],
            "sorted by class then level whatever the row order"
        );
        let c = Table::parse("class\tlevel\tbasehp\tbasemana\n1\t1\t20\t0\n1\t2\t29\t0\n").unwrap();
        assert_eq!(
            read_class_stats(&c).unwrap(),
            vec![
                ClassStat {
                    class: 1,
                    level: 1,
                    base_hp: 20,
                    base_mana: 0
                },
                ClassStat {
                    class: 1,
                    level: 2,
                    base_hp: 29,
                    base_mana: 0
                },
            ]
        );
    }

    #[test]
    fn objects_and_their_spawns() {
        let t = Table::parse(
            "entry\ttype\tdisplayId\tname\tfaction\tflags\tsize\tdata0\tdata1\tdata2\tdata3\tdata4\tdata5\tdata6\tdata7\tdata8\tdata9\n\
             61\t3\t259\tCrate of Candles\t0\t4\t1.5\t57\t61\t0\t-1\t0\t7\t8\t9\t10\t11\n",
        )
        .unwrap();
        let o = &read_objects(&t).unwrap()[0];
        assert_eq!((o.entry, o.kind, o.display, o.flags), (61, 3, 259, 4));
        assert_eq!(o.name, "Crate of Candles");
        assert_eq!(o.size, 1.5);
        assert_eq!(o.data, [57, 61, 0, -1, 0, 7, 8, 9, 10, 11]);

        let s = Table::parse(
            "guid\tid\tmap\tposition_x\tposition_y\tposition_z\torientation\tspawntimesecsmin\tspawntimesecsmax\tstate\tspawn_flags\n\
             100\t61\t0\t-8900.5\t-120.25\t80\t0.5\t300\t360\t1\t0\n\
             101\t61\t0\t-8901\t-121\t81\t0\t300\t300\t1\t0\n",
        )
        .unwrap();
        let ev = Table::parse("guid\tevent\n101\t9\n").unwrap();
        let v = read_object_spawns(&s, &ev).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].guid, v[0].entry), (100, 61));
        assert_eq!(v[0].pos, [-8900.5, -120.25, 80.0]);
        assert_eq!(
            (v[0].respawn_min, v[0].respawn_max, v[0].area),
            (300, 360, 0)
        );
        assert!(!v[0].event && v[1].event);
    }

    #[test]
    fn loot_vendor_and_equipment_rows() {
        let loot = Table::parse(
            "entry\titem\tChanceOrQuestChance\tgroupid\tmincountOrRef\tmaxcount\n\
             38\t752\t-80\t0\t1\t1\n\
             38\t0\t100\t1\t-100500\t1\n",
        )
        .unwrap();
        let v = read_loot(&loot, LootKind::Creature).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].kind, LootKind::Creature);
        assert_eq!((v[0].table, v[0].item, v[0].chance), (38, 752, -80.0));
        assert_eq!(
            (v[1].group, v[1].min_or_ref, v[1].max_count),
            (1, -100500, 1)
        );
        assert_eq!(
            read_loot(&loot, LootKind::Reference).unwrap()[0].kind,
            LootKind::Reference
        );

        let vend = Table::parse(
            "entry\tslot\titem\tmaxcount\tincrtime\n\
             152\t0\t2070\t0\t0\n\
             152\t1\t159\t5\t3600\n",
        )
        .unwrap();
        let v = read_vendors(&vend).unwrap();
        assert_eq!(
            v[1],
            Vendor {
                npc: 152,
                item: 159,
                max_count: 5,
                incr_time: 3600
            }
        );

        let eq =
            Table::parse("entry\tprobability\titem1\titem2\titem3\n4\t100\t1\t2\t3\n").unwrap();
        assert_eq!(
            read_equipment(&eq).unwrap(),
            vec![Equipment {
                entry: 4,
                probability: 100,
                items: [1, 2, 3]
            }]
        );
    }

    #[test]
    fn waypoints_read_every_column_in_file_order() {
        let t = Table::parse(
            "id\tpoint\tposition_x\tposition_y\tposition_z\torientation\twaittime\twander_distance\tscript_id\tpath_id\n\
             80247\t2\t-8900.5\t-120.25\t80\t1.5\t5000\t0\t7\t1\n\
             80247\t1\t-8901\t-121\t81\t0\t0\t0\t0\t0\n\
             80248\t1\t1\t2\t3\t\\N\t250\t2.5\t0\t0\n",
        )
        .unwrap();
        let v = read_waypoints(&t).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(
            v[0],
            Waypoint {
                guid: 80247,
                point: 2,
                pos: [-8900.5, -120.25, 80.0],
                orientation: 1.5,
                wait_ms: 5000,
                script_id: 7,
                path_id: 1
            }
        );
        // The loader keeps the file's order (select sorts) and a 0 orientation as the number 0.
        assert_eq!((v[1].point, v[1].orientation, v[1].wait_ms), (1, 0.0, 0));
        assert_eq!(
            (v[2].guid, v[2].orientation, v[2].wait_ms),
            (80248, 0.0, 250)
        );
    }

    fn path_spawn(guid: u32, entry: u32, movement: u8) -> Spawn {
        Spawn {
            guid,
            entry,
            alt: [0; 4],
            pos: [0.0; 3],
            orientation: 0.0,
            respawn_min: 300,
            respawn_max: 300,
            wander: 0.0,
            movement,
            area: 12,
            event: false,
        }
    }

    const PATH_FIELDS: &str = "point\tposition_x\tposition_y\tposition_z\torientation\twaittime\twander_distance\tscript_id\tpath_id";

    fn template_paths() -> Table {
        Table::parse(&format!(
            "entry\t{PATH_FIELDS}\n\
             448\t2\t-9850.5\t-950.25\t36\t1.5\t3000\t0\t7\t9\n\
             448\t1\t-9840\t-940\t37\t0\t250\t0\t8\t0\n"
        ))
        .unwrap()
    }

    #[test]
    fn template_waypoints_expand_to_each_walker_guid_and_keep_path_ids() {
        let empty = Table::parse(&format!("id\t{PATH_FIELDS}\n")).unwrap();
        let mut unrelated = path_spawn(5, 900, 2);
        unrelated.alt[0] = 448;
        let spawns = [
            path_spawn(80531, 448, 2),
            path_spawn(80532, 448, 2),
            path_spawn(3, 448, 0),
            path_spawn(4, 448, 1),
            unrelated,
        ];
        let paths =
            read_waypoints_with_templates(&empty, Some(&template_paths()), &spawns).unwrap();
        assert_eq!(paths.len(), 4);
        assert_eq!(
            paths[0],
            Waypoint {
                guid: 80531,
                point: 2,
                pos: [-9850.5, -950.25, 36.0],
                orientation: 1.5,
                wait_ms: 3000,
                script_id: 7,
                path_id: 9,
            }
        );
        assert_eq!(
            (paths[1].point, paths[1].path_id, paths[1].wait_ms),
            (1, 0, 250)
        );
        assert_eq!(
            paths[2],
            Waypoint {
                guid: 80532,
                ..paths[0].clone()
            }
        );
        assert_eq!(
            paths[3],
            Waypoint {
                guid: 80532,
                ..paths[1].clone()
            }
        );
    }

    #[test]
    fn a_guid_path_suppresses_all_entry_template_paths_for_that_spawn() {
        let direct = Table::parse(&format!(
            "id\t{PATH_FIELDS}\n80531\t4\t1\t2\t3\t0\t500\t0\t0\t5\n"
        ))
        .unwrap();
        let spawns = [path_spawn(80531, 448, 2), path_spawn(80532, 448, 2)];
        let paths =
            read_waypoints_with_templates(&direct, Some(&template_paths()), &spawns).unwrap();
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], read_waypoints(&direct).unwrap()[0]);
        assert_eq!(paths.iter().filter(|p| p.guid == 80531).count(), 1);
        assert!(paths[1..].iter().all(|p| p.guid == 80532));
    }

    #[test]
    fn missing_template_exports_keep_guid_paths_and_do_not_invent_paths() {
        let direct = Table::parse(&format!(
            "id\t{PATH_FIELDS}\n80531\t4\t1\t2\t3\t0\t500\t0\t0\t5\n"
        ))
        .unwrap();
        let spawns = [path_spawn(80531, 448, 2), path_spawn(80532, 448, 2)];
        assert_eq!(
            read_waypoints_with_templates(&direct, None, &spawns).unwrap(),
            read_waypoints(&direct).unwrap()
        );
    }

    const SCRIPT_HEAD: &str = "id\tdelay\tpriority\tcommand\tdatalong\tdatalong2\tdatalong3\tdatalong4\ttarget_param1\ttarget_param2\ttarget_type\tdata_flags\tdataint\tdataint2\tdataint3\tdataint4\tx\ty\tz\to\tcondition_id\tcomments";

    #[test]
    fn quest_scripts_keep_every_column_and_the_side_they_come_from() {
        let t = Table::parse(&format!(
            "{SCRIPT_HEAD}\n\
             54\t1\t0\t15\t6245\t1\t2\t3\t4\t5\t6\t7\t-8\t0\t0\t-9\t1.5\t-2.5\t3\t0.25\t99\tReport to Goldshire\\\n- Cast\n\
             54\t2\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t1242\t0\t0\t0\t0\t0\t0\t0\t0\t\\N\n"
        ))
        .unwrap();
        let v = read_scripts(&t, true).unwrap();
        assert_eq!(v.len(), 2);
        let (id, s) = &v[0];
        assert_eq!(*id, 54);
        assert!(s.start && s.quest == 0, "the quest is filled in by select");
        assert_eq!((s.delay, s.priority, s.command), (1, 0, 15));
        assert_eq!(s.datalong, [6245, 1, 2, 3]);
        assert_eq!(s.target_param, [4, 5]);
        assert_eq!((s.target_type, s.data_flags, s.condition_id), (6, 7, 99));
        assert_eq!(s.dataint, [-8, 0, 0, -9]);
        assert_eq!(s.pos, [1.5, -2.5, 3.0, 0.25]);
        assert_eq!(s.comments, "Report to Goldshire\n- Cast");
        assert_eq!(v[1].1.dataint[0], 1242);
        assert_eq!(v[1].1.comments, "", "NULL reads as empty");
        assert!(!read_scripts(&t, false).unwrap()[0].1.start);
    }

    #[test]
    fn the_xp_table_is_indexed_by_level_minus_one_whatever_the_row_order() {
        let t = Table::parse("lvl\txp_for_next_level\n3\t1400\n1\t400\n2\t900\n").unwrap();
        assert_eq!(read_xp_for_level(&t).unwrap(), vec![400, 900, 1400]);
        let gap = Table::parse("lvl\txp_for_next_level\n1\t400\n3\t1400\n").unwrap();
        let e = read_xp_for_level(&gap).unwrap_err().to_string();
        assert!(e.contains("expected level 2"), "{e}");
        let dup = Table::parse("lvl\txp_for_next_level\n1\t400\n1\t400\n").unwrap();
        assert!(read_xp_for_level(&dup).is_err());
    }

    #[test]
    fn item_extras_read_the_start_quest_and_the_bonding() {
        let t = Table::parse(
            "entry\tstart_quest\tbonding\tpage_text\n14646\t5805\t4\t0\n2092\t0\t0\t7\n",
        )
        .unwrap();
        assert_eq!(
            read_item_extras(&t).unwrap(),
            vec![
                ItemExtra {
                    entry: 14646,
                    start_quest: 5805,
                    bonding: 4
                },
                ItemExtra {
                    entry: 2092,
                    start_quest: 0,
                    bonding: 0
                }
            ]
        );
    }

    #[test]
    fn a_missing_column_is_named() {
        let t = Table::parse("id\n1\n").unwrap();
        // `{:#}` prints the whole chain: the row number first, the missing column after it.
        let e = format!("{:#}", read_relations(&t, false).unwrap_err());
        assert!(
            e.contains("row 1") && e.contains("no column `quest`"),
            "{e}"
        );
    }

    #[test]
    fn export_oddities_are_read_without_failing() {
        // item 10555 "Resist Test Item" has armor -50; the model has no negative armor.
        let item = one_row(&[
            ("entry", "10555"),
            ("class", "4"),
            ("subclass", "0"),
            ("name", "Resist Test Item"),
            ("display_id", "1"),
            ("quality", "1"),
            ("flags", "0"),
            ("buy_price", "0"),
            ("sell_price", "0"),
            ("inventory_type", "0"),
            ("allowable_class", "-1"),
            ("allowable_race", "-1"),
            ("item_level", "1"),
            ("required_level", "0"),
            ("max_count", "0"),
            ("stackable", "1"),
            ("delay", "0"),
            ("dmg_min1", "0"),
            ("dmg_max1", "0"),
            ("dmg_type1", "0"),
            ("block", "0"),
            ("armor", "-50"),
            ("stat_type1", "0"),
            ("stat_value1", "0"),
            ("stat_type2", "0"),
            ("stat_value2", "0"),
            ("stat_type3", "0"),
            ("stat_value3", "0"),
            ("stat_type4", "0"),
            ("stat_value4", "0"),
            ("stat_type5", "0"),
            ("stat_value5", "0"),
            ("stat_type6", "0"),
            ("stat_value6", "0"),
            ("stat_type7", "0"),
            ("stat_value7", "0"),
            ("stat_type8", "0"),
            ("stat_value8", "0"),
            ("stat_type9", "0"),
            ("stat_value9", "0"),
            ("stat_type10", "0"),
            ("stat_value10", "0"),
        ]);
        assert_eq!(read_items(&item).unwrap()[0].armor, 0);

        // gameobject_template.data5 = 4294967295 is -1 in the same 32 bits.
        let obj = one_row(&[
            ("entry", "176084"),
            ("type", "11"),
            ("displayId", "1"),
            ("name", "Subway"),
            ("faction", "0"),
            ("flags", "0"),
            ("size", "1"),
            ("data0", "0"),
            ("data1", "2147483647"),
            ("data2", "0"),
            ("data3", "0"),
            ("data4", "0"),
            ("data5", "4294967295"),
            ("data6", "-7"),
            ("data7", "0"),
            ("data8", "4294967294"),
            ("data9", "0"),
        ]);
        let o = &read_objects(&obj).unwrap()[0];
        assert_eq!((o.data[1], o.data[5]), (i32::MAX, -1));
        assert_eq!((o.data[6], o.data[8]), (-7, -2));

        // A negative object respawn time (not spawned by default) keeps its delay.
        let spawn = one_row(&[
            ("guid", "30031"),
            ("id", "1729"),
            ("map", "0"),
            ("position_x", "0"),
            ("position_y", "0"),
            ("position_z", "0"),
            ("orientation", "0"),
            ("spawntimesecsmin", "-21600"),
            ("spawntimesecsmax", "-600"),
            ("state", "1"),
            ("spawn_flags", "0"),
        ]);
        let none = Table::parse(
            "guid	event
",
        )
        .unwrap();
        let s = &read_object_spawns(&spawn, &none).unwrap()[0];
        assert_eq!((s.respawn_min, s.respawn_max), (21600, 600));
    }

    #[test]
    fn install_export_has_the_known_rows() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cache/mangos/tsv");
        if !dir.join("creature.tsv").exists() {
            eprintln!("skipped: no export in {}", dir.display());
            return;
        }
        let db = Db::load(&dir).unwrap();
        let garrick: Vec<_> = db.spawns.iter().filter(|s| s.entry == 103).collect();
        assert_eq!(garrick.len(), 1);
        assert!(
            (garrick[0].pos[0] + 9056.48).abs() < 0.01 && (garrick[0].pos[1] + 460.90).abs() < 0.01
        );
        let q18 = db.quests.iter().find(|q| q.id == 18).unwrap();
        assert_eq!(q18.req_items, vec![(752, 12)]);
        assert_eq!(q18.reward_xp, 355);
        assert_eq!(q18.title, "Brotherhood of Thieves");
        assert!(
            db.quests.iter().any(|q| q.details.contains('\n')),
            "escaped newlines inside quest text come out as newlines"
        );
        assert!(db
            .creatures
            .iter()
            .any(|c| c.entry == 38 && c.name == "Defias Thug"));
        assert!(db
            .creature_starts
            .iter()
            .any(|r| r.npc == 197 && r.quest == 7));
        assert!(db
            .loot
            .iter()
            .any(|l| l.kind == LootKind::Creature && l.item == 752 && l.table == 38));
        // creature_movement.tsv: every row of the table, keyed by the spawn's guid.
        assert!(
            db.waypoints.len() > 60_000,
            "{} waypoints",
            db.waypoints.len()
        );
        assert!(db.waypoints.iter().all(|w| w.point > 0 && w.guid > 0));
        // The quest-engine columns and tables (re-export of 2026-10-09).
        let q7 = db.quests.iter().find(|q| q.id == 7).unwrap();
        assert_eq!(q7.reward_rep, vec![(72, 100)], "Stormwind reputation");
        assert_eq!((q7.max_level, q7.required_condition), (0, 0));
        assert_eq!(q7.reward_mail, (0, 0, 0));
        let q54 = db.quests.iter().find(|q| q.id == 54).unwrap();
        assert_eq!(q54.start_script, 54);
        assert!(
            db.scripts.iter().any(|(id, s)| *id == 54 && s.start),
            "quest 54 has a start script"
        );
        assert!(
            db.scripts.iter().any(|(id, s)| *id == 21 && !s.start),
            "quest 21 has an end script"
        );
        assert_eq!(db.xp_for_level.len(), 60);
        assert_eq!(db.xp_for_level[..3], [400, 900, 1400]);
        assert!(
            db.level_stats
                .iter()
                .any(|s| s.class == 1 && s.level == 1 && s.stamina == 22),
            "Human Warrior level 1 has 22 Stamina"
        );
        assert!(
            db.items
                .iter()
                .any(|i| i.entry == 6095 && i.stats == vec![(7, 4), (5, 4)]),
            "the Wandering Boots carry +4 Stamina and +4 Intellect"
        );
        assert!(
            db.item_extras
                .iter()
                .any(|i| i.entry == 14646 && i.start_quest == 5805 && i.bonding == 4),
            "the Northshire Gift Voucher starts quest 5805"
        );
        let crate161557 = db.objects.iter().find(|o| o.entry == 161557).unwrap();
        assert_eq!(crate161557.data[1], 10119, "Milly's Harvest: loot table");
    }

    /// Only the columns `read_quests` needs; the loader must not require the others.
    const QUEST_HEAD: &str = "entry\tpatch\tZoneOrSort\tMinLevel\tQuestLevel\tRequiredClasses\tRequiredRaces\tPrevQuestId\tNextQuestId\tExclusiveGroup\tNextQuestInChain\tQuestFlags\tSpecialFlags\tLimitTime\tSrcItemId\tSrcItemCount\tTitle\tDetails\tObjectives\tOfferRewardText\tRequestItemsText\tEndText\tObjectiveText1\tObjectiveText2\tObjectiveText3\tObjectiveText4\tReqItemId1\tReqItemId2\tReqItemId3\tReqItemId4\tReqItemCount1\tReqItemCount2\tReqItemCount3\tReqItemCount4\tReqCreatureOrGOId1\tReqCreatureOrGOId2\tReqCreatureOrGOId3\tReqCreatureOrGOId4\tReqCreatureOrGOCount1\tReqCreatureOrGOCount2\tReqCreatureOrGOCount3\tReqCreatureOrGOCount4\tReqSpellCast1\tReqSpellCast2\tReqSpellCast3\tReqSpellCast4\tRewChoiceItemId1\tRewChoiceItemId2\tRewChoiceItemId3\tRewChoiceItemId4\tRewChoiceItemId5\tRewChoiceItemId6\tRewChoiceItemCount1\tRewChoiceItemCount2\tRewChoiceItemCount3\tRewChoiceItemCount4\tRewChoiceItemCount5\tRewChoiceItemCount6\tRewItemId1\tRewItemId2\tRewItemId3\tRewItemId4\tRewItemCount1\tRewItemCount2\tRewItemCount3\tRewItemCount4\tRewXP\tRewOrReqMoney\tRewSpell\tPointMapId\tPointX\tPointY\tStartScript\tCompleteScript\tMaxLevel\tRequiredCondition\tRepObjectiveFaction\tRepObjectiveValue\tRequiredMinRepFaction\tRequiredMinRepValue\tRequiredMaxRepFaction\tRequiredMaxRepValue\tRewRepFaction1\tRewRepFaction2\tRewRepFaction3\tRewRepFaction4\tRewRepFaction5\tRewRepValue1\tRewRepValue2\tRewRepValue3\tRewRepValue4\tRewRepValue5\tRewMailTemplateId\tRewMailDelaySecs\tRewMailMoney";

    /// The `Details` cell is `Hello $N,<backslash><LF>this is a test`: MySQL writes a newline inside a
    /// value as a backslash followed by a real newline (the plan's `\\n` would be backslash + `n`).
    const QUEST_ROW: &str = "18\t0\t9\t2\t4\t0\t0\t783\t3903\t-18\t0\t0\t0\t0\t0\t0\tBrotherhood of Thieves\tHello $N,\\\nthis is a test\tBring me 12 bandanas\tThanks\tNot yet\t\\N\t\t\t\t\t752\t0\t0\t0\t12\t0\t0\t0\t-6\t0\t0\t0\t3\t0\t0\t0\t0\t0\t0\t0\t2224\t5580\t1161\t5579\t1159\t0\t1\t1\t1\t1\t1\t0\t0\t0\t0\t0\t0\t0\t0\t0\t355\t0\t0\t0\t0\t0\t0\t0\t9\t8\t7\t70\t6\t-60\t5\t50\t0\t72\t0\t0\t0\t9\t-25\t0\t0\t0\t11\t12\t13";

    /// A one-row table from `(column, value)` pairs (a value is written as the export would).
    fn one_row(pairs: &[(&str, &str)]) -> Table {
        let head: Vec<&str> = pairs.iter().map(|p| p.0).collect();
        let row: Vec<&str> = pairs.iter().map(|p| p.1).collect();
        Table::parse(&format!(
            "{}
{}
",
            head.join("	"),
            row.join("	")
        ))
        .unwrap()
    }

    fn quest_table() -> Table {
        Table::parse(&format!("{QUEST_HEAD}\n{QUEST_ROW}\n")).unwrap()
    }

    fn no_events(col: &str) -> Table {
        Table::parse(&format!("{col}\tevent\n")).unwrap()
    }
}
