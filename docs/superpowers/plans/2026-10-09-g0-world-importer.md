# G0 World Importer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read Northshire Valley's creatures, spawns, NPCs, quests, quest items, loot, vendors and objects out of the vMaNGOS world database on disk and write one manifest the game can load (`cache/world/northshire.txt`, `wsworld 1`).

**Architecture:** A PowerShell script copies the repack's MyISAM database, runs a throwaway `mysqld` on 127.0.0.1:3399 and exports tables as tab-separated files into `cache/mangos/tsv/`. A new library crate `wow-world` (plus bin `wow-souls-world`) parses those files, decides what is inside the valley from the WoW client's own terrain data (ADT chunk `area_id` + `AreaTable.dbc` parents, never a coordinate box), selects the connected set (spawns, NPC templates, quests and the NPCs / items / loot / objects they touch) and writes / reads the manifest. The game later reads the manifest only (G1+).

**Tech Stack:** Rust 2021 (workspace), `anyhow`, `benilla-formats` (`Chain`, `load_tile_mesh`, `area_id_at`, `load_area_table_catalog`), PowerShell 5.1, MySQL 5.5 (`mysqld.exe` / `mysql.exe` from `C:\Spel\MaNGOS\mysql5`).

Spec: `docs/superpowers/specs/2026-10-09-northshire-quests-design.md`. Facts and findings: `docs/HANDOVER.md` "Quests: data import". Rules that bind every task:

- The repack's own `mysqld` is never started and its data dir is only read (`tools/mangos-db.ps1` copies it).
- Never write into `C:\Spel\World of Warcraft 1.12` or `C:\Spel\MaNGOS` (check by file listing before / after at the end).
- `patch_min <= 10 AND patch_max >= 10` on `creature`, `gameobject`, relation and loot tables; for tables with a `patch` column (`creature_template`, `quest_template`, `item_template`, `gameobject_template`) keep the row with the highest `patch <= 10` per entry.
- Build with `-j 4` (RAM-tight machine): `cargo test -j 4 -p wow-world`, `cargo clippy -j 4 -p wow-world --all-targets -- -D warnings`, `cargo fmt --all -- --check`. No commits unless the user asks (nothing in this repo is committed yet).
- Install tests need `WOW_DATA="C:/Spel/World of Warcraft 1.12/Data"` and the export in `cache/mangos/tsv/` in the same shell command; without them they print "skipped" and pass.

## File structure

| File | Responsibility |
|---|---|
| `tools/mangos-db.ps1` | **already written** (start / stop / query the throwaway server over a copy) |
| `tools/export-mangos.ps1` | one `SELECT ... INTO OUTFILE` per table -> `cache/mangos/tsv/<name>.tsv` |
| `crates/wow-world/Cargo.toml` | crate manifest (lib `wow_world`, bin `wow-souls-world`) |
| `crates/wow-world/src/lib.rs` | module list, `pub use` of the public types |
| `crates/wow-world/src/tsv.rs` | parse MySQL `INTO OUTFILE` text (escapes, `\N`) -> `Table` with named columns |
| `crates/wow-world/src/model.rs` | plain data types: `Spawn`, `Creature`, `Quest`, `Item`, `ObjectSpawn`, `ObjectTemplate`, `LootRow`, `Relation`, `Vendor`, `World` |
| `crates/wow-world/src/db.rs` | `Db::load(dir)`: the TSV files -> typed vectors (patch rule applied) |
| `crates/wow-world/src/valley.rs` | `Valley`: ADT area ids + `AreaTable` ancestors -> `contains(x, y)` |
| `crates/wow-world/src/select.rs` | `select(&Db, &Valley) -> World`: the connected set |
| `crates/wow-world/src/manifest.rs` | `write(&World) -> String`, `parse(&str) -> World` |
| `crates/wow-world/src/main.rs` | CLI: `import`, `areas`, `summary` |

---

### Task 1: Crate skeleton and the TSV parser

**Files:**
- Create: `crates/wow-world/Cargo.toml`, `crates/wow-world/src/lib.rs`, `crates/wow-world/src/tsv.rs`, `crates/wow-world/src/main.rs` (stub)
- Modify: nothing (workspace `members = ["crates/*"]` picks the crate up)

- [ ] **Step 1: Create the manifest and stubs**

`crates/wow-world/Cargo.toml`:

```toml
[package]
name = "wow-world"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Northshire's creatures, spawns, NPCs and quests from the vMaNGOS world database, as one manifest for the game"

[lints]
workspace = true

[lib]
name = "wow_world"
path = "src/lib.rs"

[[bin]]
name = "wow-souls-world"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
benilla-formats.workspace = true
```

`crates/wow-world/src/lib.rs`:

```rust
//! Northshire Valley as WoW Classic has it: creatures, spawns, NPCs, quests, items and objects,
//! read from the vMaNGOS world database (exported by `tools/export-mangos.ps1`) and written as one
//! manifest (`wsworld 1`) that the game loads.

pub mod tsv;
```

`crates/wow-world/src/main.rs`:

```rust
fn main() {
    eprintln!("wow-souls-world: not built yet");
}
```

- [ ] **Step 2: Write the failing tests in `crates/wow-world/src/tsv.rs`**

```rust
//! MySQL `SELECT ... INTO OUTFILE ... FIELDS TERMINATED BY '\t' ESCAPED BY '\\' LINES TERMINATED BY '\n'`
//! text. The first line is a header of column names (the export unions one in). A field is
//! backslash-escaped: `\\` is a backslash, `\t` is NOT used (a tab inside a value is written as
//! backslash followed by a literal tab), a backslash followed by a literal newline is a newline
//! inside the value, `\0` is NUL, and the two characters `\N` as a whole field are SQL NULL.

use anyhow::{bail, Result};
use std::collections::HashMap;

/// A parsed export: column names and rows of optional strings (`None` is SQL NULL).
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    index: HashMap<String, usize>,
}

/// One row with access by column name.
pub struct Row<'a> {
    table: &'a Table,
    cells: &'a [Option<String>],
}

impl Table {
    pub fn parse(text: &str) -> Result<Table> {
        let mut records: Vec<Vec<Option<String>>> = Vec::new();
        let mut row: Vec<Option<String>> = Vec::new();
        let mut field = String::new();
        let mut is_null = false;
        let mut chars = text.chars().peekable();
        let mut field_started = false;
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    field_started = true;
                    match chars.next() {
                        Some('N') if field.is_empty() && !is_null => is_null = true,
                        Some('0') => field.push('\0'),
                        Some(other) => field.push(other),
                        None => bail!("tsv: text ends inside an escape"),
                    }
                }
                '\t' => {
                    row.push(if is_null { None } else { Some(std::mem::take(&mut field)) });
                    field.clear();
                    is_null = false;
                    field_started = false;
                }
                '\n' => {
                    row.push(if is_null { None } else { Some(std::mem::take(&mut field)) });
                    field.clear();
                    is_null = false;
                    field_started = false;
                    records.push(std::mem::take(&mut row));
                }
                other => {
                    field_started = true;
                    field.push(other);
                }
            }
        }
        if field_started || !row.is_empty() {
            row.push(if is_null { None } else { Some(field) });
            records.push(row);
        }
        let mut it = records.into_iter();
        let Some(header) = it.next() else {
            bail!("tsv: empty text, no header line");
        };
        let columns: Vec<String> = header.into_iter().map(|c| c.unwrap_or_default()).collect();
        let index = columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
        let rows: Vec<_> = it.collect();
        for (n, r) in rows.iter().enumerate() {
            if r.len() != columns.len() {
                bail!("tsv: row {} has {} fields, header has {}", n + 1, r.len(), columns.len());
            }
        }
        Ok(Table { columns, rows, index })
    }

    pub fn load(path: &std::path::Path) -> Result<Table> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        Table::parse(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
    }

    pub fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        self.rows.iter().map(|cells| Row { table: self, cells })
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl<'a> Row<'a> {
    fn cell(&self, col: &str) -> Result<&'a Option<String>> {
        let i = *self
            .table
            .index
            .get(col)
            .ok_or_else(|| anyhow::anyhow!("tsv: no column `{col}` (have {:?})", self.table.columns))?;
        Ok(&self.cells[i])
    }

    /// The text of a column, NULL read as the empty string.
    pub fn text(&self, col: &str) -> Result<String> {
        Ok(self.cell(col)?.clone().unwrap_or_default())
    }

    pub fn u32(&self, col: &str) -> Result<u32> {
        let t = self.text(col)?;
        if t.is_empty() {
            return Ok(0);
        }
        t.parse().map_err(|e| anyhow::anyhow!("column `{col}` = `{t}`: {e}"))
    }

    pub fn i32(&self, col: &str) -> Result<i32> {
        let t = self.text(col)?;
        if t.is_empty() {
            return Ok(0);
        }
        t.parse().map_err(|e| anyhow::anyhow!("column `{col}` = `{t}`: {e}"))
    }

    pub fn f32(&self, col: &str) -> Result<f32> {
        let t = self.text(col)?;
        if t.is_empty() {
            return Ok(0.0);
        }
        t.parse().map_err(|e| anyhow::anyhow!("column `{col}` = `{t}`: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_header_rows_and_typed_columns() {
        let t = Table::parse("entry\tname\tx\n38\tDefias Thug\t-8913.5\n6\tKobold Vermin\t\\N\n").unwrap();
        assert_eq!(t.columns, vec!["entry", "name", "x"]);
        assert_eq!(t.len(), 2);
        let rows: Vec<_> = t.rows().collect();
        assert_eq!(rows[0].u32("entry").unwrap(), 38);
        assert_eq!(rows[0].text("name").unwrap(), "Defias Thug");
        assert!((rows[0].f32("x").unwrap() + 8913.5).abs() < 1e-3);
        // NULL reads as empty / zero.
        assert_eq!(rows[1].f32("x").unwrap(), 0.0);
    }

    #[test]
    fn escapes_keep_tabs_newlines_and_backslashes_inside_a_value() {
        // A value `a<TAB>b<NL>c\d`: MySQL writes backslash+tab, backslash+newline, double backslash.
        let text = "id\ttext\n1\ta\\\tb\\\nc\\\\d\n2\tplain\n";
        let t = Table::parse(text).unwrap();
        assert_eq!(t.len(), 2);
        let rows: Vec<_> = t.rows().collect();
        assert_eq!(rows[0].text("text").unwrap(), "a\tb\nc\\d");
        assert_eq!(rows[1].text("text").unwrap(), "plain");
    }

    #[test]
    fn backslash_n_is_null_only_as_a_whole_field() {
        let t = Table::parse("a\tb\nx\\N\t\\N\n").unwrap();
        let rows: Vec<_> = t.rows().collect();
        // `x\N` is the text `xN`; `\N` alone is NULL.
        assert_eq!(rows[0].text("a").unwrap(), "xN");
        assert_eq!(t.rows[0][1], None);
    }

    #[test]
    fn a_row_with_the_wrong_field_count_is_an_error() {
        let e = Table::parse("a\tb\n1\n").unwrap_err().to_string();
        assert!(e.contains("row 1 has 1 fields, header has 2"), "{e}");
    }

    #[test]
    fn an_unknown_column_names_the_columns_that_exist() {
        let t = Table::parse("a\n1\n").unwrap();
        let e = t.rows().next().unwrap().u32("zzz").unwrap_err().to_string();
        assert!(e.contains("no column `zzz`") && e.contains("\"a\""), "{e}");
    }
}
```

- [ ] **Step 3: Run the tests; expect the parser tests to pass or show real defects**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world`
Expected: 5 passed. If `backslash_n_is_null_only_as_a_whole_field` fails because `x\N` was read as `x` + NULL, fix `Some('N') if field.is_empty() && !is_null` so `\N` only counts when nothing precedes it in the field (it already does; keep the guard).

- [ ] **Step 4: Gates**

Run: `cargo fmt --all -- --check && cargo clippy -j 4 -p wow-world --all-targets -- -D warnings`
Expected: clean.

---

### Task 2: The export script

**Files:**
- Create: `tools/export-mangos.ps1`

- [ ] **Step 1: Write the script**

```powershell
# Exports the tables the world importer reads from the throwaway vMaNGOS server into
# cache\mangos\tsv\<name>.tsv (tab separated, backslash escapes, a header line, NULL as \N).
# Starts the server if it is not up and leaves it up (tools\mangos-db.ps1 stop stops it).

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$db = Join-Path $PSScriptRoot 'mangos-db.ps1'
$out = Join-Path $repo 'cache\mangos\tsv'
New-Item -ItemType Directory -Force $out | Out-Null
& powershell -ExecutionPolicy Bypass -File $db start
if ($LASTEXITCODE -ne 0) { throw 'could not start the database copy' }

$outFwd = $out -replace '\\', '/'
$live = 'patch_min <= 10 AND patch_max >= 10'
# Newest row of a (entry, patch) table that 1.12 sees.
function Latest($table, $key) {
    "$table t WHERE t.patch = (SELECT MAX(x.patch) FROM mangos.$table x WHERE x.$key = t.$key AND x.patch <= 10)"
}

# name -> (column list, FROM ... WHERE ... clause). The header line is unioned in by the loop below.
$tables = [ordered]@{
    creature = @(
        'guid,id,id2,id3,id4,id5,map,position_x,position_y,position_z,orientation,spawntimesecsmin,spawntimesecsmax,wander_distance,movement_type,spawn_flags',
        "mangos.creature WHERE map = 0 AND $live")
    creature_template = @(
        'entry,name,subname,level_min,level_max,faction,npc_flags,display_id1,display_id2,display_id3,display_id4,display_scale1,display_scale2,display_scale3,display_scale4,display_probability1,display_probability2,display_probability3,display_probability4,type,rank,detection_range,call_for_help_range,leash_range,speed_walk,speed_run,loot_id,equipment_id,vendor_id,trainer_id,trainer_type,trainer_class,movement_type,inhabit_type,civilian,flags_extra,health_multiplier,damage_multiplier,unit_class',
        (Latest 'creature_template' 'entry'))
    creature_questrelation = @('id,quest', "mangos.creature_questrelation WHERE $live")
    creature_involvedrelation = @('id,quest', "mangos.creature_involvedrelation WHERE $live")
    gameobject_questrelation = @('id,quest', "mangos.gameobject_questrelation WHERE $live")
    gameobject_involvedrelation = @('id,quest', "mangos.gameobject_involvedrelation WHERE $live")
    quest_template = @(
        'entry,patch,Method,ZoneOrSort,MinLevel,QuestLevel,Type,RequiredClasses,RequiredRaces,RequiredSkill,RequiredSkillValue,SuggestedPlayers,LimitTime,QuestFlags,SpecialFlags,PrevQuestId,NextQuestId,ExclusiveGroup,BreadcrumbForQuestId,NextQuestInChain,SrcItemId,SrcItemCount,SrcSpell,Title,Details,Objectives,OfferRewardText,RequestItemsText,EndText,ObjectiveText1,ObjectiveText2,ObjectiveText3,ObjectiveText4,ReqItemId1,ReqItemId2,ReqItemId3,ReqItemId4,ReqItemCount1,ReqItemCount2,ReqItemCount3,ReqItemCount4,ReqSourceId1,ReqSourceId2,ReqSourceId3,ReqSourceId4,ReqSourceCount1,ReqSourceCount2,ReqSourceCount3,ReqSourceCount4,ReqCreatureOrGOId1,ReqCreatureOrGOId2,ReqCreatureOrGOId3,ReqCreatureOrGOId4,ReqCreatureOrGOCount1,ReqCreatureOrGOCount2,ReqCreatureOrGOCount3,ReqCreatureOrGOCount4,ReqSpellCast1,ReqSpellCast2,ReqSpellCast3,ReqSpellCast4,RewChoiceItemId1,RewChoiceItemId2,RewChoiceItemId3,RewChoiceItemId4,RewChoiceItemId5,RewChoiceItemId6,RewChoiceItemCount1,RewChoiceItemCount2,RewChoiceItemCount3,RewChoiceItemCount4,RewChoiceItemCount5,RewChoiceItemCount6,RewItemId1,RewItemId2,RewItemId3,RewItemId4,RewItemCount1,RewItemCount2,RewItemCount3,RewItemCount4,RewXP,RewOrReqMoney,RewMoneyMaxLevel,RewSpell,RewSpellCast,PointMapId,PointX,PointY,PointOpt,StartScript,CompleteScript',
        (Latest 'quest_template' 'entry'))
    item_template = @(
        'entry,class,subclass,name,display_id,quality,flags,buy_price,sell_price,inventory_type,allowable_class,allowable_race,item_level,required_level,max_count,stackable,delay,dmg_min1,dmg_max1,dmg_type1,block,armor,spellid_1,spelltrigger_1',
        (Latest 'item_template' 'entry'))
    gameobject_template = @(
        'entry,type,displayId,name,faction,flags,size,data0,data1,data2,data3,data4,data5',
        (Latest 'gameobject_template' 'entry'))
    gameobject = @(
        'guid,id,map,position_x,position_y,position_z,orientation,spawntimesecsmin,spawntimesecsmax,state,spawn_flags',
        "mangos.gameobject WHERE map = 0 AND $live")
    creature_loot_template = @('entry,item,ChanceOrQuestChance,groupid,mincountOrRef,maxcount', "mangos.creature_loot_template WHERE $live")
    gameobject_loot_template = @('entry,item,ChanceOrQuestChance,groupid,mincountOrRef,maxcount', "mangos.gameobject_loot_template WHERE $live")
    reference_loot_template = @('entry,item,ChanceOrQuestChance,groupid,mincountOrRef,maxcount', "mangos.reference_loot_template WHERE $live")
    npc_vendor = @('entry,slot,item,maxcount,incrtime', 'mangos.npc_vendor')
    creature_equip_template = @('entry,probability,item1,item2,item3', "mangos.creature_equip_template WHERE $live")
    game_event_creature = @('guid,event', 'mangos.game_event_creature')
    game_event_gameobject = @('guid,event', 'mangos.game_event_gameobject')
    game_event_quest = @('quest,event', 'mangos.game_event_quest')
    game_event = @('entry,description,holiday', 'mangos.game_event')
}

foreach ($name in $tables.Keys) {
    $cols, $from = $tables[$name]
    $list = $cols -split ','
    $header = ($list | ForEach-Object { "'$_'" }) -join ','
    $select = ($list | ForEach-Object { "CAST(t.``$_`` AS CHAR)" }) -join ','
    # `FROM <table> t`: the WHERE clauses above are written against the bare table or alias `t`.
    $fromSql = if ($from -match '^mangos\.') { $from -replace '^(mangos\.\S+)', '$1 t' } else { "mangos.$from" }
    $file = "$outFwd/$name.tsv"
    if (Test-Path ($file -replace '/', '\')) { Remove-Item ($file -replace '/', '\') }
    $sql = "SELECT $header UNION ALL SELECT $select FROM $fromSql INTO OUTFILE '$file' FIELDS TERMINATED BY '\t' ESCAPED BY '\\\\' LINES TERMINATED BY '\n'"
    & powershell -ExecutionPolicy Bypass -File $db query $sql
    if ($LASTEXITCODE -ne 0) { throw "export of $name failed" }
    $lines = (Get-Content ($file -replace '/', '\') -ReadCount 0 | Measure-Object).Count
    Write-Host ("{0,-28} {1,8} lines" -f $name, $lines)
}
```

(The script is the contract; the implementer fixes quoting if MySQL rejects a clause, and reports the line counts.)

- [ ] **Step 2: Run it and read the counts**

Run: `powershell -ExecutionPolicy Bypass -File tools\export-mangos.ps1`
Expected: one line per table with a line count (header included). `creature` roughly 60 000+ lines, `quest_template` about 4 000+, `creature_template` several thousand. If a clause fails (alias `t` used on a table name that is not a bare `mangos.<table> WHERE` string, `\\` escaping of PowerShell vs MySQL), fix the script, rerun, and write the final counts into the task report.

- [ ] **Step 3: Check one file by eye**

Run (Git Bash): `head -c 600 cache/mangos/tsv/creature.tsv; grep -c "" cache/mangos/tsv/creature.tsv; grep -P "^38\t|\tDefias Thug\t" cache/mangos/tsv/creature_template.tsv | head -3`
Expected: header line with `guid\tid\t...`, then rows; Defias Thug (entry 38) present exactly once in `creature_template` (the newest patch row).

---

### Task 3: Typed model and database loader

**Files:**
- Create: `crates/wow-world/src/model.rs`, `crates/wow-world/src/db.rs`
- Modify: `crates/wow-world/src/lib.rs` (add `pub mod model; pub mod db;`)

- [ ] **Step 1: `model.rs` (the contract every later task and the game use)**

```rust
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
    pub data: [i32; 6],
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

/// Everything the valley needs, selected and connected.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct World {
    /// Area ids counted as the valley, with their names, for the manifest's record of what was used.
    pub areas: Vec<(u32, String)>,
    pub creatures: Vec<Creature>,
    pub spawns: Vec<Spawn>,
    pub quests: Vec<Quest>,
    pub starts: Vec<Relation>,
    pub ends: Vec<Relation>,
    pub items: Vec<Item>,
    pub objects: Vec<ObjectTemplate>,
    pub object_spawns: Vec<ObjectSpawn>,
    pub loot: Vec<LootRow>,
    pub vendors: Vec<Vendor>,
    pub equipment: Vec<Equipment>,
}
```

- [ ] **Step 2: Write `db.rs` tests first (fixtures in the test module, no files needed)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tsv::Table;

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
        assert_eq!((v[0].respawn_min, v[0].respawn_max, v[0].movement), (270, 270, 0));
        assert!(!v[0].event);
        assert!(v[1].event, "guid 5 is in a game event");
    }

    #[test]
    fn quest_reads_objectives_rewards_and_signed_kill_ids() {
        // Only the columns read_quests needs; the loader must not require the others.
        let head = "entry\tpatch\tZoneOrSort\tMinLevel\tQuestLevel\tRequiredClasses\tRequiredRaces\tPrevQuestId\tNextQuestId\tExclusiveGroup\tNextQuestInChain\tQuestFlags\tSpecialFlags\tLimitTime\tSrcItemId\tSrcItemCount\tTitle\tDetails\tObjectives\tOfferRewardText\tRequestItemsText\tEndText\tObjectiveText1\tObjectiveText2\tObjectiveText3\tObjectiveText4\tReqItemId1\tReqItemId2\tReqItemId3\tReqItemId4\tReqItemCount1\tReqItemCount2\tReqItemCount3\tReqItemCount4\tReqCreatureOrGOId1\tReqCreatureOrGOId2\tReqCreatureOrGOId3\tReqCreatureOrGOId4\tReqCreatureOrGOCount1\tReqCreatureOrGOCount2\tReqCreatureOrGOCount3\tReqCreatureOrGOCount4\tReqSpellCast1\tReqSpellCast2\tReqSpellCast3\tReqSpellCast4\tRewChoiceItemId1\tRewChoiceItemId2\tRewChoiceItemId3\tRewChoiceItemId4\tRewChoiceItemId5\tRewChoiceItemId6\tRewChoiceItemCount1\tRewChoiceItemCount2\tRewChoiceItemCount3\tRewChoiceItemCount4\tRewChoiceItemCount5\tRewChoiceItemCount6\tRewItemId1\tRewItemId2\tRewItemId3\tRewItemId4\tRewItemCount1\tRewItemCount2\tRewItemCount3\tRewItemCount4\tRewXP\tRewOrReqMoney\tRewSpell\tPointMapId\tPointX\tPointY\tStartScript\tCompleteScript";
        let row = "18\t0\t9\t2\t4\t0\t0\t783\t3903\t-18\t0\t0\t0\t0\t0\t0\tBrotherhood of Thieves\tHello $N,\\nthis is a test\tBring me 12 bandanas\tThanks\tNot yet\t\\N\t\t\t\t\t752\t0\t0\t0\t12\t0\t0\t0\t-6\t0\t0\t0\t3\t0\t0\t0\t0\t0\t0\t0\t2224\t5580\t1161\t5579\t1159\t0\t1\t1\t1\t1\t1\t0\t0\t0\t0\t0\t0\t0\t0\t0\t355\t0\t0\t0\t0\t0\t0\t0";
        let t = Table::parse(&format!("{head}\n{row}\n")).unwrap();
        let events = Table::parse("quest\tevent\n").unwrap();
        let q = read_quests(&t, &events).unwrap();
        assert_eq!(q.len(), 1);
        let q = &q[0];
        assert_eq!((q.id, q.title.as_str(), q.exclusive_group, q.reward_xp), (18, "Brotherhood of Thieves", -18, 355));
        assert_eq!(q.details, "Hello $N,\nthis is a test");
        assert_eq!(q.req_items, vec![(752, 12)]);
        assert_eq!(q.req_kills, vec![(-6, 3)], "a negative id is an object, kept signed");
        assert_eq!(q.reward_choice, vec![(2224, 1), (5580, 1), (1161, 1), (5579, 1), (1159, 1)]);
        assert!(q.reward_items.is_empty());
        assert_eq!(q.prev, 783);
    }
}
```

Note the fixture's `\\n` is a backslash followed by `n` inside the Rust string, i.e. text `\n` (backslash, n): the TSV parser maps backslash + `n` to the single character `n`, NOT a newline. A real export writes a literal newline after a backslash. Use a real escaped newline in the fixture instead: write the `Details` cell as `Hello $N,\<LF>this is a test` (in Rust: `"Hello $N,\\\nthis is a test"`). Fix the fixture to that before running; the assertion `q.details == "Hello $N,\nthis is a test"` then holds.

- [ ] **Step 3: Implement `db.rs`**

Public API (the later tasks depend on exactly these names):

```rust
use crate::model::*;
use crate::tsv::Table;
use anyhow::Result;
use std::path::Path;

/// Every exported table, typed. Quests and items are the newest patch <= 10 row (the export already
/// applied the rule); the loaders do not filter again.
pub struct Db {
    pub spawns: Vec<Spawn>,
    pub creatures: Vec<Creature>,
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
}

impl Db {
    /// Read `dir/<name>.tsv` for every table the export writes.
    pub fn load(dir: &Path) -> Result<Db>;
}

pub fn read_spawns(spawns: &Table, events: &Table) -> Result<Vec<Spawn>>;
pub fn read_creatures(t: &Table) -> Result<Vec<Creature>>;
pub fn read_quests(t: &Table, events: &Table) -> Result<Vec<Quest>>;
pub fn read_relations(t: &Table, object: bool) -> Result<Vec<Relation>>;
pub fn read_items(t: &Table) -> Result<Vec<Item>>;
pub fn read_objects(t: &Table) -> Result<Vec<ObjectTemplate>>;
pub fn read_object_spawns(spawns: &Table, events: &Table) -> Result<Vec<ObjectSpawn>>;
pub fn read_loot(t: &Table, kind: LootKind) -> Result<Vec<LootRow>>;
pub fn read_vendors(t: &Table) -> Result<Vec<Vendor>>;
pub fn read_equipment(t: &Table) -> Result<Vec<Equipment>>;
```

Rules the implementation must follow (each is a `Row` accessor call): required objective ids/counts are collected in index order 1..N and a slot is dropped when its id is 0 (items, spell casts) or its id is 0 (kills); reward lists drop slots whose item id is 0; `req_kills` keeps the signed `ReqCreatureOrGOId`; the event flag of a spawn is `guid in game_event_creature.guid` (object spawns: `game_event_gameobject`), of a quest `entry in game_event_quest.quest`. `area` is left 0 here (task 4 and 5 fill it). The game-event files are `game_event_creature.tsv`, `game_event_gameobject.tsv`, `game_event_quest.tsv`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -j 4 -p wow-world`
Expected: all pass (the TSV tests and the two loader tests).

- [ ] **Step 5: Install test (skipped without the export)**

Add to `db.rs` tests:

```rust
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
        assert!((garrick[0].pos[0] + 9056.48).abs() < 0.01 && (garrick[0].pos[1] + 460.90).abs() < 0.01);
        let q18 = db.quests.iter().find(|q| q.id == 18).unwrap();
        assert_eq!(q18.req_items, vec![(752, 12)]);
        assert_eq!(q18.reward_xp, 355);
        assert!(db.creatures.iter().any(|c| c.entry == 38 && c.name == "Defias Thug"));
        assert!(db.creature_starts.iter().any(|r| r.npc == 197 && r.quest == 7));
        assert!(db.loot.iter().any(|l| l.kind == LootKind::Creature && l.item == 752 && l.table == 38));
    }
```

Run: `cargo test -j 4 -p wow-world install_export`
Expected: PASS (after Task 2's export), printing nothing about "skipped".

- [ ] **Step 6: Gates** (`cargo fmt --all -- --check`, clippy `-D warnings`).

---

### Task 4: The valley from the client's own terrain data

**Files:**
- Create: `crates/wow-world/src/valley.rs`
- Modify: `crates/wow-world/src/lib.rs` (`pub mod valley;`)

Facts (verified in the benilla source): `benilla_formats::open_chain(path: &Path) -> Result<Chain>` takes the WoW `Data` directory; `benilla_formats::terrain::load_tile_mesh(chain, "Azeroth", tile_x, tile_y) -> Result<TileMesh>` (`TileMesh.chunks: Vec<ChunkMesh>`); `benilla_formats::terrain::area_id_at(&chunks, [x, y, z]) -> Option<u32>` returns the chunk's `AreaTable` id when the position is over it; `benilla_wdt::world_to_tile(x, y) -> (tx, ty)` gives the tile; `benilla_formats::area_table::load_area_table_catalog(&mut chain) -> Result<AreaTableCatalog>` with `get(id) -> Option<&AreaTableRow>` (`zone_id` is the parent id, 0 for a top-level zone, `name`). Check how these are re-exported (`grep -n "pub use\|pub mod" C:\Spel\benilla\crates\benilla-formats\src\lib.rs`) and import accordingly; `benilla-wdt` is not a direct dependency of this crate, so compute the tile index with the same arithmetic instead of adding a dependency: `tile = 32 - floor(coord / 533.3333)` for both axes (x -> tx? verify with the anchor (-8913, -136) against `world_to_tile` once by a throwaway test and write the verified formula into a doc comment).

- [ ] **Step 1: Write the pure test first** (no install needed): the ancestor rule.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subzone_counts_when_the_root_is_in_its_parent_chain() {
        // area -> parent: 9 -> 12 (Elwynn), 86 -> 9, 87 -> 12, 12 -> 0
        let parents = |a: u32| match a {
            9 => Some(12),
            86 => Some(9),
            87 => Some(12),
            12 => Some(0),
            _ => None,
        };
        assert!(has_ancestor(9, 9, &parents), "the root itself");
        assert!(has_ancestor(86, 9, &parents), "a child of the root");
        assert!(!has_ancestor(87, 9, &parents), "a sibling under Elwynn");
        assert!(!has_ancestor(12, 9, &parents), "the parent zone is not inside its child");
        assert!(!has_ancestor(999, 9, &parents), "an unknown area");
    }

    #[test]
    fn tile_of_a_world_position() {
        // Northshire's anchor is inside tile (tx, ty) of Azeroth; fix the numbers from benilla_wdt::world_to_tile.
        let (tx, ty) = tile_of(-8913.0, -136.0);
        assert_eq!((tx, ty), (EXPECT_TX, EXPECT_TY));
    }
}
```

Replace `EXPECT_TX` / `EXPECT_TY` by the values `benilla_wdt::world_to_tile(-8913.0, -136.0)` returns (find them with a one-off `cargo test` in the benilla workspace is NOT allowed (benilla is never modified): compute by the formula above and cross-check with `grep -n "fn world_to_tile" -A 12 C:\Spel\benilla\crates\benilla-wdt\src\lib.rs`).

- [ ] **Step 2: Implement `valley.rs`**

```rust
/// Whether `area` is `root` or has `root` somewhere up its parent chain. `parent(a)` is the parent
/// area id (`Some(0)` for a top-level zone), `None` for an unknown area. Chains deeper than 8 are corrupt.
pub fn has_ancestor(area: u32, root: u32, parent: &dyn Fn(u32) -> Option<u32>) -> bool {
    let mut id = area;
    for _ in 0..8 {
        if id == root {
            return true;
        }
        match parent(id) {
            Some(0) | None => return false,
            Some(p) => id = p,
        }
    }
    false
}

pub const NORTHSHIRE_VALLEY: u32 = 9;

pub fn tile_of(x: f32, y: f32) -> (u32, u32) { /* verified formula from step 1 */ }

/// Terrain areas of the Eastern Kingdoms (map 0, files `Azeroth`) around the valley and the
/// `AreaTable` parents; answers "which area is under (x, y)" and "is it Northshire Valley".
pub struct Valley {
    tiles: std::collections::HashMap<(u32, u32), Vec<benilla_formats::terrain::ChunkMesh>>,
    catalog: benilla_formats::area_table::AreaTableCatalog,
}

impl Valley {
    /// Open the client's `Data` directory and load every ADT tile that the spawns' bounding box touches.
    pub fn load(wow_data: &std::path::Path, bounds: ([f32; 2], [f32; 2])) -> anyhow::Result<Valley>;
    /// The `AreaTable` id under a position, 0 when no loaded tile covers it.
    pub fn area_of(&self, x: f32, y: f32) -> u32;
    /// True when the area under (x, y) is Northshire Valley or one of its sub-areas.
    pub fn contains(&self, x: f32, y: f32) -> bool;
    pub fn name(&self, area: u32) -> String;
    /// `(area id, name)` of every area id that has `NORTHSHIRE_VALLEY` in its ancestry and appears under `points`.
    pub fn areas_under(&self, points: &[[f32; 2]]) -> Vec<(u32, String)>;
}
```

`load` collects the tiles that cover the box (`bounds` = min and max corner in WoW x, y; use `x_min..=x_max`, `y_min..=y_max` stepped by `TILE_SIZE / 2` so every tile is hit), calls `load_tile_mesh(&mut chain, "Azeroth", tx, ty)` once per distinct tile and keeps only the `chunks`.

- [ ] **Step 3: Install test (skipped without WOW_DATA)**

```rust
    #[test]
    fn install_the_abbey_is_in_northshire_and_goldshire_is_not() {
        let Ok(data) = std::env::var("WOW_DATA") else {
            eprintln!("skipped: WOW_DATA not set");
            return;
        };
        let v = Valley::load(std::path::Path::new(&data), ([-9600.0, -600.0], [-8500.0, 200.0])).unwrap();
        // The Abbey's doors, Milly's vineyard, Garrick's spot (all in the valley).
        for (x, y) in [(-8913.0, -136.0), (-9070.0, -340.0), (-9056.48, -460.9), (-8850.29, -224.03)] {
            assert!(v.contains(x, y), "({x}, {y}) is area {} {}", v.area_of(x, y), v.name(v.area_of(x, y)));
        }
        // Goldshire (-9459, 64) is Elwynn, not the valley.
        assert!(!v.contains(-9459.0, 64.0), "Goldshire {}", v.name(v.area_of(-9459.0, 64.0)));
        assert_eq!(v.area_of(-8913.0, -136.0) != 0, true);
    }
```

If Garrick's spot or the vineyard is NOT inside the valley by area id (it can sit in a sub-area whose parent is Elwynn), do not weaken the test silently: print the area names, report them, and ask the planner (the right answer may be an explicit extra list of area ids).

- [ ] **Step 4: Run, gates.** `WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 -p wow-world valley` then fmt and clippy.

---

### Task 5: Select the connected set

**Files:**
- Create: `crates/wow-world/src/select.rs`
- Modify: `crates/wow-world/src/lib.rs` (`pub mod select;`)

- [ ] **Step 1: Interface and rules**

```rust
use crate::{db::Db, model::World};

/// How the valley is told apart from its surroundings: given by the caller (the real one asks the
/// terrain; tests give a closure).
pub trait InValley {
    fn area_of(&self, x: f32, y: f32) -> u32;
    fn contains(&self, x: f32, y: f32) -> bool;
    fn name(&self, area: u32) -> String;
}

pub fn select(db: &Db, valley: &dyn InValley) -> World;
```

Rules (each one gets a test):
1. `spawns` = every creature spawn with `valley.contains(x, y)` and not `event`; each gets `area = valley.area_of(x, y)`. Same for `object_spawns` (objects: all types).
2. `creatures` = the templates of every entry used by those spawns (`entry` and the non-zero `alt` entries) plus every creature entry that starts or ends a selected quest.
3. `quests` = the quests with `zone_or_sort == 9` (area id of the valley, it is the quest's zone), plus every quest that a creature in `creatures` (spawned in the valley) starts or ends, plus every quest an object in `object_spawns` starts or ends; minus `event` quests. Sorted by id.
4. `starts` / `ends` = the relation rows (creature and object, `object` flag set) whose quest is in `quests` and whose NPC or object is spawned in the valley, i.e. in `creatures` (spawned) or `objects`.
5. `items` = every item named by a selected quest (required items, source item, `req_source` items are not exported; reward choices and rewards), by the loot of selected creatures and objects (following one level of reference tables), by selected vendors' stock, and by the equipment rows of selected creatures. `objects` = templates of the selected object spawns plus the objects named in a quest's `req_kills` with a negative id.
6. `loot` = all `LootRow`s of tables `creature.loot_id` (use `entry` when `loot_id` is 0) for the selected creatures (`LootKind::Creature`), of tables `data1` for selected chests / gatherable objects of type 3 (`LootKind::Object`), and the `Reference` rows those rows point to (`min_or_ref < 0`, table `-min_or_ref`).
7. `vendors` = `npc_vendor` rows of the selected creatures (`npc == entry`); `equipment` = `creature_equip_template` rows of `equipment_id`s of selected creatures.
8. `areas` = distinct `(area, name)` of all selected spawns, sorted by id.
9. Output vectors are sorted by their natural key (spawns by guid, creatures by entry, ...), so a manifest is deterministic.

- [ ] **Step 2: Tests first** with a small fake `Db` built in the test (a helper `fn db() -> Db` adds: creature 38 spawn inside, creature 721 spawn outside, creature 197 (McBride) spawn inside starting quest 7, quest 7 `zone_or_sort 9`, quest 999 `zone_or_sort 12` not related, an event spawn, loot row (table 38, item 752, chance -80), item 752, a reference loot row, a vendor row, an object spawn type 3 with a loot table). Assert each rule above: e.g. `world.spawns.iter().map(|s| s.guid)` equals the inside, non-event guids; `world.quests` ids equal `[7]`; `world.items` contains 752; the outside rabbit is absent; quest 999 absent; output sorted.

- [ ] **Step 3: Implement `select`**, make the tests pass, gates.

- [ ] **Step 4: `impl InValley for crate::valley::Valley`** (a thin delegation in `valley.rs`).

---

### Task 6: The manifest

**Files:**
- Create: `crates/wow-world/src/manifest.rs`
- Modify: `crates/wow-world/src/lib.rs` (`pub mod manifest;`)

Format `wsworld 1`: the first line is `wsworld 1`; every other line is `kind<TAB>field<TAB>...` with the same escaping as `tsv` (backslash before a backslash, tab or newline; empty string is an empty field). Kinds and field order are exactly the struct field order in `model.rs` (flatten arrays to consecutive fields; `Vec<ItemCount>` as a count then pairs; `Vec<(i32,u32)>` likewise; floats with `{}` Rust formatting, which round-trips). Kinds: `area`, `creature`, `spawn`, `quest`, `start`, `end`, `item`, `object`, `objspawn`, `loot`, `vendor`, `equip`.

- [ ] **Step 1: Test first: write -> parse is the identity**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn sample() -> World {
        World {
            areas: vec![(9, "Northshire Valley".into())],
            spawns: vec![Spawn { guid: 80247, entry: 103, alt: [0; 4], pos: [-9056.48, -460.903, 72.8318], orientation: 1.5, respawn_min: 270, respawn_max: 270, wander: 0.0, movement: 0, area: 9, event: false }],
            quests: vec![Quest {
                id: 18, patch: 0, title: "Brotherhood of Thieves".into(), details: "Hello $N,\nit\ttabs\\slash".into(),
                objectives: String::new(), offer_reward: "Thanks".into(), request_items: "Not yet".into(), end_text: String::new(),
                objective_text: Default::default(), zone_or_sort: 9, min_level: 2, level: 4, required_classes: 0, required_races: 0,
                prev: 783, next: 3903, exclusive_group: -18, next_in_chain: 0, flags: 0, special_flags: 0, limit_time: 0,
                src_item: 0, src_item_count: 0, req_items: vec![(752, 12)], req_kills: vec![(-6, 3)], req_spell_cast: vec![],
                reward_choice: vec![(2224, 1), (5580, 1)], reward_items: vec![], reward_money: 25, reward_xp: 355, reward_spell: 0,
                point: (0, 0.0, 0.0), start_script: 0, complete_script: 0, event: false,
            }],
            ..World::default()
        }
    }

    #[test]
    fn write_then_parse_is_the_identity() {
        let w = sample();
        let text = write(&w);
        assert!(text.starts_with("wsworld 1\n"));
        assert_eq!(parse(&text).unwrap(), w);
    }

    #[test]
    fn writing_twice_gives_the_same_text() {
        assert_eq!(write(&sample()), write(&sample()));
    }

    #[test]
    fn an_unknown_version_or_kind_is_an_error() {
        assert!(parse("wsworld 2\n").unwrap_err().to_string().contains("version"));
        assert!(parse("wsworld 1\nbogus\t1\n").unwrap_err().to_string().contains("bogus"));
    }
}
```

- [ ] **Step 2: Implement `write(&World) -> String` and `parse(&str) -> Result<World>`**, reusing the escape rules (add `pub fn escape(&str) -> String` to `tsv.rs` and test it against `Table::parse`). Extend `sample()` in a second test to cover every record kind at least once so no kind is untested (add one value of each of Creature, Start, End, Item, ObjectTemplate, ObjectSpawn, LootRow, Vendor, Equipment).

- [ ] **Step 3: Gates.**

---

### Task 7: The command line and the real run

**Files:**
- Modify: `crates/wow-world/src/main.rs`

- [ ] **Step 1: Implement three subcommands**

```
wow-souls-world import [--tsv <dir>] [--wow <Data dir>] [--out <file>]
   defaults: cache/mangos/tsv, C:\Spel\World of Warcraft 1.12\Data (env WOW_DATA wins), cache/world/northshire.txt
wow-souls-world areas [--tsv <dir>] [--wow <Data dir>]
   prints, for every area id found under the creature and object spawns inside the box
   x -9600..-8500, y -600..200: id, name, parent id and name, spawn count, and whether it counts as the valley
wow-souls-world summary [--out <file>]
   reads the manifest back and prints counts (spawns by creature name, quests by title, items, objects)
```

`import` writes the manifest and prints the summary; it refuses to overwrite when the selection is empty (zero spawns or zero quests) and says why.

- [ ] **Step 2: Run on the real data**

```
cd /c/Spel/wow-souls && cargo build -j 4 -p wow-world
./target/debug/wow-souls-world.exe areas
./target/debug/wow-souls-world.exe import
./target/debug/wow-souls-world.exe summary
```

Expected, to be verified against the database by hand (`powershell -ExecutionPolicy Bypass -File tools\mangos-db.ps1 query "..."`): Defias Thug, Kobold Worker / Vermin / Laborer, Timber Wolf, Young Wolf, Garrick (one spawn at -9056.48, -460.90), the critters (Rabbit, Deer, Fawn, cats, Cow, White Stallion), McBride, Willem, Eagan, Milly, the trainers, Brother Paxton are all present; the 12 quests with `zone_or_sort 9`, the class letters, 344-347, 1598, 2158, 5623, 5805 are present; the Elwynn rabbits north of the valley (y > ~15) are NOT. Write the real counts into the report and into `docs/HANDOVER.md`.

- [ ] **Step 3: A picture of the spawns**

Optional but wanted: write `cache/world/northshire-spawns.svg` (a 1000 px wide scatter of spawn positions coloured by entry class: hostile / critter / friendly, plus the valley's area boundary as dots for each chunk centre whose area counts) from `import`, so the user can see the valley at a glance. Add `--svg` to `import`. Look at it (Read the file) and compare with the Abbey, Echo Ridge Mine, Milly's vineyard, Garrick's spot.

- [ ] **Step 4: Gates and the untouched check**

```
cargo fmt --all -- --check
cargo clippy -j 4 --workspace --all-targets -- -D warnings
WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" DSR_DIR="C:/Spel/Dark Souls Remastered" cargo test -j 4 --workspace
find "/c/Spel/World of Warcraft 1.12" -type f -newer docs/superpowers/plans/2026-10-09-g0-world-importer.md | head
find "/c/Spel/MaNGOS" -type f -newer docs/superpowers/plans/2026-10-09-g0-world-importer.md | head
```

Expected: clean, all tests pass (wow-souls 290, dsr-cache 51, dsr-formats 51, wscl 4 plus the new `wow-world` tests), and the two `find` listings empty (neither install was written).

- [ ] **Step 5: Stop the server and update `docs/HANDOVER.md`**

`powershell -ExecutionPolicy Bypass -File tools\mangos-db.ps1 stop`. In HANDOVER: the "Next action" entry for G0 (what was built, the counts, the commands: export script, importer, where the manifest is, tests), the architecture table row for `wow-world`, and the test counts.

---

## Self-review against the spec

- G0 "script copies the DB ... exports TSV": Task 2. "new importer crate decides what is in the valley from the client's ADT area ids": Task 4. "writes one manifest": Task 6, Task 7. "spawns, NPC roster, quest set, objects, items the quests touch, loot rules": Tasks 3 and 5. "Check: counts per entry printed and compared with the DB; a sample of spawns drawn": Task 7 steps 2 and 3. "install test asserting per-entry counts": Task 3 step 5 and Task 4 step 3.
- G1-G4 are not in this plan: each gets its own plan after G0's manifest exists (G1 reads `World` through `wow_world::manifest::parse`).
- Types used across tasks: `Table`, `Row`, `Db`, `World`, `Spawn`, `Creature`, `Quest`, `Item`, `ObjectTemplate`, `ObjectSpawn`, `LootRow`, `LootKind`, `Relation`, `Vendor`, `Equipment`, `InValley`, `Valley`, `has_ancestor`, `tile_of`, `select`, `write`, `parse`, `escape` are all defined in a task above.
- Known soft spot: whether Garrick's spot and the vineyard lie in an area whose chain contains 9 (Task 4 step 3 stops and asks rather than weakening the test).
