//! Plain data. WoW coordinates throughout: +X north, +Y west, +Z up (yards).

#[derive(Debug, Clone, PartialEq)]
pub struct Spawn {
    pub guid: u32,
    /// The creature entry (`creature.id`); `alt` are the extra entries a spawn picks between (0 = none).
    pub entry: u32,
    pub alt: [u32; 4],
    pub pos: [f32; 3],
    pub orientation: f32,
    pub respawn_min: u32,
    pub respawn_max: u32,
    pub wander: f32,
    /// 0 stays, 1 wanders (random), 2 follows a waypoint path.
    pub movement: u8,
    /// The `AreaTable` id under the spawn (0 = none found).
    pub area: u32,
    /// The spawn belongs to a seasonal game event (not part of the base valley).
    pub event: bool,
}

/// One point of a creature's walking path (`creature_movement`, keyed by the creature spawn's guid).
/// A spawn with `movement == 2` walks its points in `point` order and starts over.
#[derive(Debug, Clone, PartialEq)]
pub struct Waypoint {
    /// The `Spawn::guid` the path belongs to.
    pub guid: u32,
    pub point: u32,
    pub pos: [f32; 3],
    /// 0 in the database can mean "keep the walking direction"; the number is kept as it is.
    pub orientation: f32,
    /// How long the creature stands at this point, in milliseconds.
    pub wait_ms: u32,
    pub script_id: u32,
    pub path_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Creature {
    pub entry: u32,
    pub name: String,
    pub subname: String,
    pub level_min: u32,
    pub level_max: u32,
    pub faction: u32,
    pub npc_flags: u32,
    pub display: [u32; 4],
    pub scale: [f32; 4],
    /// `creature_template.type`: 1 beast, 5 humanoid ... 8 critter.
    pub kind: u32,
    pub rank: u32,
    pub detection_range: f32,
    pub call_for_help_range: f32,
    pub leash_range: f32,
    pub speed_walk: f32,
    pub speed_run: f32,
    pub loot_id: u32,
    pub equipment_id: u32,
    pub vendor_id: u32,
    pub trainer_type: u32,
    pub trainer_class: u32,
    pub movement_type: u32,
    pub civilian: bool,
    pub health_multiplier: f32,
    pub damage_multiplier: f32,
    pub unit_class: u32,
}

/// An item and a count: a required item, a reward.
pub type ItemCount = (u32, u32);

#[derive(Debug, Clone, PartialEq)]
pub struct Quest {
    pub id: u32,
    pub patch: u32,
    pub title: String,
    pub details: String,
    pub objectives: String,
    pub offer_reward: String,
    pub request_items: String,
    pub end_text: String,
    pub objective_text: [String; 4],
    pub zone_or_sort: i32,
    pub min_level: u32,
    pub level: i32,
    pub required_classes: u32,
    pub required_races: u32,
    pub prev: i32,
    pub next: i32,
    pub exclusive_group: i32,
    pub next_in_chain: u32,
    pub flags: u32,
    pub special_flags: u32,
    pub limit_time: u32,
    pub src_item: u32,
    pub src_item_count: u32,
    pub req_items: Vec<ItemCount>,
    /// (creature entry, or object id as a negative number, count); never zero.
    pub req_kills: Vec<(i32, u32)>,
    pub req_spell_cast: Vec<u32>,
    pub reward_choice: Vec<ItemCount>,
    pub reward_items: Vec<ItemCount>,
    pub reward_money: i32,
    pub reward_xp: u32,
    pub reward_spell: u32,
    pub point: (u32, f32, f32),
    pub start_script: u32,
    pub complete_script: u32,
    /// In a `game_event_quest` row: seasonal.
    pub event: bool,
    /// `MaxLevel`: a player above this level cannot take the quest (0 = no upper bound).
    pub max_level: u32,
    /// `RequiredCondition`: a row of the conditions table (0 = none; none of the valley's quests has one).
    pub required_condition: u32,
    /// (faction, value) of `RepObjectiveFaction` / `RepObjectiveValue`; faction 0 = none.
    pub rep_objective: (u32, i32),
    /// (faction, value) of `RequiredMinRepFaction` / `RequiredMinRepValue`; faction 0 = none.
    pub rep_min: (u32, i32),
    /// (faction, value) of `RequiredMaxRepFaction` / `RequiredMaxRepValue`; faction 0 = none.
    pub rep_max: (u32, i32),
    /// The reputation the quest pays: (faction, value) for each of `RewRepFaction1..5` that is set.
    pub reward_rep: Vec<(u32, i32)>,
    /// `RewMailTemplateId`, `RewMailDelaySecs`, `RewMailMoney`.
    pub reward_mail: (u32, u32, u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub entry: u32,
    pub name: String,
    pub class: u32,
    pub subclass: u32,
    pub display: u32,
    pub quality: u32,
    pub flags: u32,
    pub buy_price: u32,
    pub sell_price: u32,
    pub inventory_type: u32,
    pub allowable_class: i32,
    pub allowable_race: i32,
    pub item_level: u32,
    pub required_level: u32,
    pub max_count: u32,
    pub stackable: u32,
    pub delay: u32,
    pub damage: (f32, f32, u32),
    pub block: u32,
    pub armor: u32,
    /// `stat_type1..10` / `stat_value1..10` of the 1.12 item table, the empty slots left out: (3 Agility,
    /// 4 Strength, 5 Intellect, 6 Spirit, 7 Stamina, 0 Mana, 1 Health, value).
    pub stats: Vec<(u32, i32)>,
}

/// The `item_template` columns the quest rules use, kept beside [`Item`] (one row per selected item) so
/// that `Item` keeps its shape for the crates that build one by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemExtra {
    pub entry: u32,
    /// The quest that using the item starts (`start_quest`; 0 = none). The Northshire Gift Voucher starts 5805.
    pub start_quest: u32,
    /// 0 none, 1 when picked up, 2 when equipped, 3 when used, 4 quest item.
    pub bonding: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectTemplate {
    pub entry: u32,
    pub name: String,
    pub kind: u32,
    pub display: u32,
    pub faction: u32,
    pub flags: u32,
    pub size: f32,
    /// `data0..data9`. A chest (type 3) reads lock, loot table, restock time, consumable, min / max
    /// opens, event, trap, quest (`data8`) and level (`data9`); a goober (type 10) its own set.
    pub data: [i32; 10],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectSpawn {
    pub guid: u32,
    pub entry: u32,
    pub pos: [f32; 3],
    pub orientation: f32,
    pub respawn_min: u32,
    pub respawn_max: u32,
    pub area: u32,
    pub event: bool,
}

/// Which loot table family a row belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootKind {
    Creature,
    Object,
    Reference,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LootRow {
    pub kind: LootKind,
    /// The loot table id (`creature_template.loot_id`, an object's `data1`, or a reference id).
    pub table: u32,
    pub item: u32,
    /// Negative: a quest drop at that percentage; positive: a normal drop chance.
    pub chance: f32,
    pub group: u32,
    /// `mincountOrRef`: positive = minimum count, negative = `-reference table`.
    pub min_or_ref: i32,
    pub max_count: u32,
}

/// A quest relation row: `npc` is a creature entry or an object entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Relation {
    pub npc: u32,
    pub quest: u32,
    pub object: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vendor {
    pub npc: u32,
    pub item: u32,
    pub max_count: u32,
    pub incr_time: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Equipment {
    pub entry: u32,
    pub probability: u32,
    pub items: [u32; 3],
}

/// One row of `quest_start_scripts` (`start`: run when the quest is accepted, `StartScript`) or
/// `quest_end_scripts` (run when it is handed in, `CompleteScript`). The columns keep the table's names.
/// The game does not run them; it logs that it skips them.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestScript {
    /// The quest whose `StartScript` / `CompleteScript` names this script's `id`.
    pub quest: u32,
    pub start: bool,
    pub delay: u32,
    pub priority: u32,
    pub command: u32,
    pub datalong: [u32; 4],
    pub target_param: [u32; 2],
    pub target_type: u32,
    pub data_flags: u32,
    pub dataint: [i32; 4],
    /// `x`, `y`, `z`, `o`.
    pub pos: [f32; 4],
    pub condition_id: u32,
    pub comments: String,
}

/// One row of `player_levelstats` for the Human race (race 1): a class's base attributes at a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelStat {
    pub class: u32,
    pub level: u32,
    pub strength: u32,
    pub agility: u32,
    pub stamina: u32,
    pub intellect: u32,
    pub spirit: u32,
}

/// One row of `player_classlevelstats`: a class's base health and mana at a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassStat {
    pub class: u32,
    pub level: u32,
    pub base_hp: u32,
    pub base_mana: u32,
}

/// Everything the valley needs, selected and connected.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct World {
    /// Area ids counted as the valley, with their names, for the manifest's record of what was used.
    pub areas: Vec<(u32, String)>,
    pub creatures: Vec<Creature>,
    pub spawns: Vec<Spawn>,
    /// The paths of the selected spawns, sorted by (guid, path_id, point).
    pub waypoints: Vec<Waypoint>,
    pub quests: Vec<Quest>,
    pub starts: Vec<Relation>,
    pub ends: Vec<Relation>,
    pub items: Vec<Item>,
    pub objects: Vec<ObjectTemplate>,
    pub object_spawns: Vec<ObjectSpawn>,
    pub loot: Vec<LootRow>,
    pub vendors: Vec<Vendor>,
    pub equipment: Vec<Equipment>,
    /// The scripts of the selected quests (start scripts, then end scripts; each in the table's order).
    pub scripts: Vec<QuestScript>,
    /// `player_xp_for_level`: index = level - 1, the XP needed to go from that level to the next
    /// (60 rows: the last one, level 60, is never reached). Empty in a manifest written before `wsworld 2`.
    pub xp_for_level: Vec<u32>,
    /// One row per selected item that has one (see [`ItemExtra`]), sorted by entry.
    pub item_extras: Vec<ItemExtra>,
    /// Human base attributes per class and level (`player_levelstats`, race 1), sorted by (class,
    /// level). Empty in a manifest written before `wsworld 3`.
    pub level_stats: Vec<LevelStat>,
    /// Base health and mana per class and level (`player_classlevelstats`), sorted likewise.
    pub class_stats: Vec<ClassStat>,
}
