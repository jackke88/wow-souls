# Attributes, Item Stats, Quest Items Before the Quest, and Boss / Rare Rules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The character has WoW's five attributes (database base per level + worn gear) that set health, attack power and armor; quest items drop before the quest is accepted; bosses and minibosses never respawn; bosses and rares drop the best item of their table the first time.

**Architecture:** The importer reads item stats and the level / class tables into manifest `wsworld 3`. A pure `wow-world/src/attributes.rs` turns tables + gear into a `Character` (attributes, max health, attack-power factors) that the existing `GearEffect` (armor, damage factors) and `level.rs` (max HP) consume. `SpeciesDef.rank` drives a pure `world_state` save (slain, first-kill, held loot), a `ring::NEVER` respawn sentinel, and a `best_drop` rule in the corpse roll. Quest items use a new `QuestLog::droppable_items`.

**Tech Stack:** Rust, Bevy 0.18.1 (`wow-souls`), pure `wow-world`, PowerShell exporter against the throwaway vMaNGOS copy.

**Spec:** `docs/superpowers/specs/2026-10-12-attributes-and-boss-rules-design.md` (approved 2026-10-12: "Kör på"). Implementation choices that differ from the spec's text (the spec is updated to match in Task 10): (1) item stats are a field on `Item` (`stats: Vec<(u32, i32)>`, written as a trailing list of the `item` record), not in `itemx`; (2) the level and class tables are two new record kinds `levelstat` / `classstat`; (3) a dead never-respawn creature's corpse is never despawned by the ring, and its unlooted `held` items are delivered to the pack at the next start; (4) a debug switch `WOWSOULS_RANK=<species>:<rank>[,..]` lets a live run make any species a boss, miniboss or rare.

## Global Constraints

- **No git commits.** The repo has no commits and the user has not asked for one; every "commit" step is replaced by running the task's gates. Do not run `git add` / `git commit`.
- Build and test with `-j 4`. Never run bare `cargo fmt --all`; the fmt gate is `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check` (fix with the same command without `--check`).
- Gates at the end of every Rust task: that fmt check, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`, and the task's tests. The final task runs the whole workspace tests with `DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data"` set in the same command.
- `hits.rs`, `projectiles.rs`, `defense.rs`, `weapons.rs`, `combat/` are co-owned with another chat: this plan does not edit them. `npc.rs`, `world.rs`, `ring.rs`, `species.rs`, `level.rs`, `loot/mod.rs`, `quests/mod.rs`, `shop/mod.rs`, `equip/*` are ours: still re-read the region right before editing.
- Python hangs the Bash tool here: use the Edit tool, `perl`, `sed`, `awk`.
- Bevy trap: a system with more than 16 parameters does not compile as a system; group parameters in a tuple.
- Unattended live runs always use scratch saves: `WOWSOULS_SETTINGS`, `_CHARACTER`, `_ITEMS_SAVE`, `_QUESTS_SAVE`, `_SOULS_SAVE`, and from Task 8 `WOWSOULS_WORLD_STATE`. An unattended run with no `WOWSOULS_WORLD_STATE` neither reads nor writes the world-state file (so a test can never kill the user's boss).
- Constants (spec "Tuning knobs"): `HEALTH_OFFSET` 0; HP = base HP (class level table) + `min(sta, 20) + 10 x max(sta - 20, 0)`; melee AP = `level x 3 + Str x 2 - 20`; ranged AP = `level + Agi - 10`; `ap_factor = (REF_DPS + AP / 14) / (REF_DPS + AP_REF / 14)` with `AP_REF` the class's level-1 value; armor `+ 2 x Agility`; Intellect and Spirit inert. Stat type ids: 0 Mana, 1 Health, 3 Agility, 4 Strength, 5 Intellect, 6 Spirit, 7 Stamina.

## Review Focus

Inputs the spec implies but does not spell out, most likely first. Each has a test in the task named.

1. **An old manifest (`wsworld 1` / `2`) or a class with no table row:** attributes are `None`, and the game keeps today's fixed vitals (100 + 10 per level) and logs why; nothing panics (Tasks 1, 2, 3).
2. **Stat rows that are empty, zero, unknown or negative** (`stat_type` 0 with value 0 is an empty slot; a type outside 0-7; a negative value): empty / unknown skipped, totals never below 0 (Tasks 1, 2).
3. **A level outside the table (0, 61+):** clamped to the table's first and last row, not a panic or a zero (Task 2).
4. **A never-respawn creature must stay dead** through the ring, the player's death, F6 (`Respawned`) and a restart; a missing, corrupt or half-written state file must play on with nothing slain and must not erase what was readable (Tasks 8, 9).
5. **The first-kill drop is given once per creature entry, never for quest rows, and a table of only food and pouches still yields its best gear or nothing; an empty / reference-only table yields nothing without error** (Tasks 7, 9).

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `tools/export-mangos.ps1` | modify | export stat columns, `player_levelstats` (race 1), `player_classlevelstats` |
| `crates/wow-world/src/model.rs` | modify | `Item.stats`, `LevelStat`, `ClassStat`, `World.level_stats` / `class_stats` |
| `crates/wow-world/src/db.rs` | modify | read the stat columns and the two tables, `Db` fields |
| `crates/wow-world/src/select.rs` | modify | carry the tables into the `World` |
| `crates/wow-world/src/manifest.rs` | modify | `wsworld 3`: `item` stats list, `levelstat`, `classstat` |
| `crates/wow-world/src/main.rs` | modify | summary line |
| `crates/wow-world/src/attributes.rs` | create | pure: `Attributes`, tables lookup, health, attack power, `Character` |
| `crates/wow-world/src/gear_effect.rs` | modify | `GearEffect::with_character` |
| `crates/wow-world/src/quest/rules.rs`, `quest/log.rs` | modify | `takeable_later`, `droppable_items`, `pre_need` |
| `crates/wow-world/src/loot.rs` | modify | `best_drop` |
| `crates/wow-world/src/world_state.rs` | create | pure `wsstate 1` text save |
| `crates/wow-souls/src/gear_effect.rs` | modify | `character()` helper, refresh with attributes |
| `crates/wow-souls/src/level.rs` | modify | max health from attributes, follows gear |
| `crates/wow-souls/src/equip/mod.rs`, `equip/ui.rs` | modify | stat rows in tooltip / compare, the sheet |
| `crates/wow-souls/src/quests/mod.rs`, `shop/mod.rs` | modify | droppable items, shop refusal |
| `crates/wow-souls/src/species.rs` | modify | `Rank`, `rank` replaces `boss`, `rank_of`, override |
| `crates/wow-souls/src/ring.rs` | modify | `NEVER`, `clear_timers` keeps it |
| `crates/wow-souls/src/world_state.rs` | create | the resource, load / save, delivery of held items |
| `crates/wow-souls/src/world.rs`, `npc.rs`, `loot/mod.rs` | modify | slain bookkeeping, no revive, keep corpse, first-kill drop, held sync |
| `README.md`, `docs/HANDOVER.md`, the spec | modify | switches, entry, spec alignment |

---

### Task 1: The importer carries item stats and the level tables (`wsworld 3`)

**Files:**
- Modify: `tools/export-mangos.ps1`, `crates/wow-world/src/model.rs`, `crates/wow-world/src/db.rs`, `crates/wow-world/src/select.rs`, `crates/wow-world/src/manifest.rs`, `crates/wow-world/src/main.rs`
- Modify (literals): every `Item { .. }` construction needs `stats: Vec::new(),` after `armor`: `crates/wow-world/src/equipment.rs` (test helper `item`, ~line 543, and ~763), `crates/wow-world/src/shop.rs` (~223), `crates/wow-world/src/select.rs` (~472), `crates/wow-world/src/manifest.rs` (~960), `crates/wow-souls/src/equip/mod.rs` (~1040 and the `shown` helper ~1045), `crates/wow-souls/src/friendly.rs` (~163), `crates/wow-souls/src/shop/mod.rs` (~605), `crates/wow-souls/src/world.rs` (~1103)
- Test: in-file tests of `db.rs`, `manifest.rs`, `select.rs`

**Interfaces:**
- Produces (Tasks 2-9 rely on these exact names):
  - `model::Item.stats: Vec<(u32, i32)>` (stat type, value; empty slots and zero values dropped)
  - `model::LevelStat { class: u32, level: u32, strength: u32, agility: u32, stamina: u32, intellect: u32, spirit: u32 }` (Human, race 1 only)
  - `model::ClassStat { class: u32, level: u32, base_hp: u32, base_mana: u32 }`
  - `World.level_stats: Vec<LevelStat>`, `World.class_stats: Vec<ClassStat>` (sorted by (class, level); empty from a `wsworld` 1 / 2 file)
  - `db::read_level_stats(t: &Table) -> Result<Vec<LevelStat>>`, `db::read_class_stats(t: &Table) -> Result<Vec<ClassStat>>`
  - manifest records: `item` gains a trailing stats list (count, then (type, value) pairs, since version 3); `levelstat\t<class>\t<level>\t<str>\t<agi>\t<sta>\t<int>\t<spi>`; `classstat\t<class>\t<level>\t<base_hp>\t<base_mana>`; `VERSION = "3"`, `READS = ["1","2","3"]`.

- [ ] **Step 1: Write the failing tests**

In `crates/wow-world/src/db.rs` `mod tests`, replace the header of `items_read_damage_and_signed_restrictions` so it carries the stat columns, and add the new tests. The header becomes (one string; the stat columns are appended after `spelltrigger_1`):

```rust
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
        let mut cols = vec![
            ("entry", "3288".to_string()),
            ("class", "4".to_string()),
            ("subclass", "2".to_string()),
            ("name", "Tribal Vest".to_string()),
            ("display_id", "1".to_string()),
            ("quality", "2".to_string()),
            ("flags", "0".to_string()),
            ("buy_price", "0".to_string()),
            ("sell_price", "0".to_string()),
            ("inventory_type", "5".to_string()),
            ("allowable_class", "-1".to_string()),
            ("allowable_race", "-1".to_string()),
            ("item_level", "10".to_string()),
            ("required_level", "8".to_string()),
            ("max_count", "0".to_string()),
            ("stackable", "1".to_string()),
            ("delay", "0".to_string()),
            ("dmg_min1", "0".to_string()),
            ("dmg_max1", "0".to_string()),
            ("dmg_type1", "0".to_string()),
            ("block", "0".to_string()),
            ("armor", "65".to_string()),
        ];
        let stats = [(3, 2), (7, 1), (0, 5), (4, -2)];
        for i in 1..=10 {
            let (t, v) = stats.get(i - 1).copied().unwrap_or((0, 0));
            cols.push((Box::leak(format!("stat_type{i}").into_boxed_str()), t.to_string()));
            cols.push((Box::leak(format!("stat_value{i}").into_boxed_str()), v.to_string()));
        }
        let pairs: Vec<(&str, &str)> = cols.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let items = read_items(&one_row(&pairs)).unwrap();
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
                LevelStat { class: 1, level: 1, strength: 23, agility: 20, stamina: 22, intellect: 20, spirit: 20 },
                LevelStat { class: 1, level: 2, strength: 24, agility: 21, stamina: 23, intellect: 20, spirit: 20 },
            ],
            "sorted by class then level whatever the row order"
        );
        let c = Table::parse("class\tlevel\tbasehp\tbasemana\n1\t1\t20\t0\n1\t2\t29\t0\n").unwrap();
        assert_eq!(
            read_class_stats(&c).unwrap(),
            vec![
                ClassStat { class: 1, level: 1, base_hp: 20, base_mana: 0 },
                ClassStat { class: 1, level: 2, base_hp: 29, base_mana: 0 },
            ]
        );
    }
```

In `crates/wow-world/src/manifest.rs` `mod tests`, extend the `full()` world builder (where `w.xp_for_level = vec![400, 900, 1400];` is set) with:

```rust
        w.level_stats = vec![
            LevelStat { class: 1, level: 1, strength: 23, agility: 20, stamina: 22, intellect: 20, spirit: 20 },
            LevelStat { class: 1, level: 2, strength: 24, agility: 21, stamina: 23, intellect: 20, spirit: 20 },
        ];
        w.class_stats = vec![
            ClassStat { class: 1, level: 1, base_hp: 20, base_mana: 0 },
            ClassStat { class: 1, level: 2, base_hp: 29, base_mana: 0 },
        ];
```

and give the first item in `full()` (the one built around `armor: 8`) `stats: vec![(7, 4), (6, 4)],`. Add tests:

```rust
    #[test]
    fn stats_and_the_level_tables_round_trip_and_are_written_as_wsworld_3() {
        let w = full();
        let text = write(&w);
        assert!(text.starts_with("wsworld 3\n"), "{text}");
        assert!(text.contains("\nlevelstat\t1\t2\t24\t21\t23\t20\t20\n"), "{text}");
        assert!(text.contains("\nclassstat\t1\t2\t29\t0\n"), "{text}");
        assert_eq!(parse(&text).unwrap(), w);
    }

    #[test]
    fn a_wsworld_2_file_still_loads_with_no_stats_and_no_tables() {
        let mut w = full();
        w.level_stats.clear();
        w.class_stats.clear();
        for i in &mut w.items {
            i.stats.clear();
        }
        // The version-2 text: same records, header 2, no stats list on the item, no new kinds.
        let text = write(&w).replacen("wsworld 3", "wsworld 2", 1);
        let text: String = text
            .lines()
            .map(|l| {
                if l.starts_with("item\t") {
                    // drop the trailing "0" count the version 3 writer appends
                    l.rsplit_once('\t').map(|(head, _)| head).unwrap().to_string()
                } else {
                    l.to_string()
                }
            })
            .map(|l| l + "\n")
            .collect();
        let back = parse(&text).unwrap();
        assert!(back.level_stats.is_empty() && back.class_stats.is_empty());
        assert!(back.items.iter().all(|i| i.stats.is_empty()));
        assert_eq!(back, w);
    }

    #[test]
    fn the_new_kinds_are_refused_in_an_older_file() {
        for line in ["levelstat\t1\t1\t23\t20\t22\t20\t20", "classstat\t1\t1\t20\t0"] {
            let text = format!("wsworld 2\n{line}\n");
            let e = parse(&text).unwrap_err().to_string();
            assert!(e.contains("wsworld 3"), "{e}");
        }
    }
```

In `crates/wow-world/src/select.rs` `mod tests`, add to the `db()` fixture's `Db { .. }` literal `level_stats: vec![LevelStat { class: 1, level: 1, strength: 23, agility: 20, stamina: 22, intellect: 20, spirit: 20 }], class_stats: vec![ClassStat { class: 1, level: 1, base_hp: 20, base_mana: 0 }],` (and add `LevelStat, ClassStat` to its `use crate::model::...`), and add:

```rust
    #[test]
    fn the_level_and_class_tables_are_taken_whole() {
        let w = select(&db(), &Fake);
        assert_eq!(w.level_stats.len(), 1);
        assert_eq!(w.class_stats.len(), 1);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world 2>&1 | tail -30`
Expected: compile errors (`no field stats`, `cannot find LevelStat`, `read_level_stats` not found).

- [ ] **Step 3: Implement the model and importer**

`model.rs`: add to `Item` (after `armor`):

```rust
    /// `stat_type1..10` / `stat_value1..10` of the 1.12 item table, the empty slots left out: (3 Agility,
    /// 4 Strength, 5 Intellect, 6 Spirit, 7 Stamina, 0 Mana, 1 Health, value).
    pub stats: Vec<(u32, i32)>,
```

and, before `World`:

```rust
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
```

and in `World` after `item_extras`:

```rust
    /// Human base attributes per class and level (`player_levelstats`, race 1), sorted by (class,
    /// level). Empty in a manifest written before `wsworld 3`.
    pub level_stats: Vec<LevelStat>,
    /// Base health and mana per class and level (`player_classlevelstats`), sorted likewise.
    pub class_stats: Vec<ClassStat>,
```

`db.rs`: in `Db` add `pub level_stats: Vec<LevelStat>, pub class_stats: Vec<ClassStat>,`; in `load` after `item_extras`:

```rust
        let level_stats = read_level_stats(&table("player_levelstats")?)
            .with_context(|| in_file("player_levelstats"))?;
        let class_stats = read_class_stats(&table("player_classlevelstats")?)
            .with_context(|| in_file("player_classlevelstats"))?;
```

and the two fields in the returned `Db { .. }`. In `read_items` add after `armor:`:

```rust
            stats: read_stats(r)?,
```

and below `read_items`:

```rust
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
```

`select.rs`: in the final `World { .. }` literal after `item_extras,` add:

```rust
        // The whole tables: a player's levels do not depend on the valley.
        level_stats: db.level_stats.clone(),
        class_stats: db.class_stats.clone(),
```

`manifest.rs`: change header docs' version text, `const VERSION: &str = "3";`, `const READS: [&str; 3] = ["1", "2", "3"];`; in `write`, after `.put(i.armor)` of the `item` record chain end the record with the stats list: replace `.put(i.armor));` (the item record's last line, ~line 247) with:

```rust
            .put(i.armor)
            .pairs(&i.stats));
```

and after the `xp` loop add:

```rust
    for s in &w.level_stats {
        add(Rec::new("levelstat")
            .put(s.class)
            .put(s.level)
            .put(s.strength)
            .put(s.agility)
            .put(s.stamina)
            .put(s.intellect)
            .put(s.spirit));
    }
    for s in &w.class_stats {
        add(Rec::new("classstat")
            .put(s.class)
            .put(s.level)
            .put(s.base_hp)
            .put(s.base_mana));
    }
```

In `read_record`: change the guard to `if matches!(kind, "itemx" | "qscript" | "xp") && c.version < 2 { .. }` (unchanged) and add directly after it:

```rust
    if matches!(kind, "levelstat" | "classstat") && c.version < 3 {
        bail!("unknown record kind `{kind}` (it needs wsworld 3)");
    }
```

In the `"item"` reader add after `armor: c.num()?,`:

```rust
            stats: c.since(3, |c| c.pairs())?,
```

and add a `pairs` reader on `Cur` next to `arr`/`since` (read the existing `arr` to match its style; a pair list is a count then both halves):

```rust
    /// A list of (type, value) pairs: a count, then both halves of each entry.
    fn pairs(&mut self) -> Result<Vec<(u32, i32)>> {
        let n: usize = self.num()?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push((self.num()?, self.num()?));
        }
        Ok(v)
    }
```

and the two new record readers next to `"xp"`:

```rust
        "levelstat" => w.level_stats.push(LevelStat {
            class: c.num()?,
            level: c.num()?,
            strength: c.num()?,
            agility: c.num()?,
            stamina: c.num()?,
            intellect: c.num()?,
            spirit: c.num()?,
        }),
        "classstat" => w.class_stats.push(ClassStat {
            class: c.num()?,
            level: c.num()?,
            base_hp: c.num()?,
            base_mana: c.num()?,
        }),
```

Add `"levelstat", "classstat"` to the kinds list in the `the_kinds_...` test array near line 1050 if that test enumerates every kind (read it; keep it passing). `main.rs`: extend `quest_data_line` with ` {} level rows, {} class rows,` using `w.level_stats.len()` and `w.class_stats.len()`.

Every `Item { .. }` literal listed under Files gets `stats: Vec::new(),` (or `vec![]`) after its `armor` field.

`tools/export-mangos.ps1`: change the `item_template` column list by appending `,stat_type1,stat_value1,stat_type2,stat_value2,stat_type3,stat_value3,stat_type4,stat_value4,stat_type5,stat_value5,stat_type6,stat_value6,stat_type7,stat_value7,stat_type8,stat_value8,stat_type9,stat_value9,stat_type10,stat_value10` after `page_text`, and add after `player_xp_for_level`:

```powershell
    # Human base attributes per class and level (the game plays one race for now) and each class's base
    # health and mana per level. No patch columns used.
    player_levelstats = @('race,class,level,str,agi,sta,inte,spi', 'mangos.player_levelstats t WHERE t.race = 1')
    player_classlevelstats = @('class,level,basehp,basemana', 'mangos.player_classlevelstats t')
```

- [ ] **Step 4: Run the importer tests**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world 2>&1 | grep -E "^error|test result|FAILED|failed" | head`
Expected: `test result: ok.` for the lib tests (the whole-load install test is skipped or fails only until Step 5; if it fails on a missing `player_levelstats.tsv`, that is Step 5's cause).

- [ ] **Step 5: Re-export, re-import, check**

```bash
cd /c/Spel/wow-souls
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/export-mangos.ps1 -Only item_template,player_levelstats,player_classlevelstats
cp cache/world/northshire.txt cache/world/northshire.pre-stats.bak
cargo run -j 4 -p wow-world --bin wow-souls-world -- import --out cache/world/northshire.txt
cargo run -j 4 -p wow-world --bin wow-souls-world -- summary 2>&1 | tail -6
head -c 12 cache/world/northshire.txt; echo
awk -F'\t' '$1=="levelstat"' cache/world/northshire.txt | head -3
```

Expected: the file starts `wsworld 3`; `levelstat` rows begin `levelstat 1 1 23 20 22 20 20`; the summary line reports 60 x classes of level rows. Then `DSR_DIR=... WOW_DATA=... cargo test -j 4 -p wow-world` passes including the whole-load test (add the assertion `assert!(db.level_stats.iter().any(|s| s.class == 1 && s.level == 1 && s.stamina == 22));` and `assert!(db.items.iter().any(|i| i.entry == 6095 && i.stats == vec![(7, 4), (6, 4)]));` to the install test next to the `xp_for_level` assertions: the Wandering Boots carry +4 Stamina and +4 Spirit).

- [ ] **Step 6: Gates**

Run: `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check && cargo clippy -j 4 --workspace --all-targets -- -D warnings`
Expected: exit 0 (fix fmt with the same command without `--check`). No commit.

---

### Task 2: The pure attribute rules (`wow-world/src/attributes.rs`)

**Files:**
- Create: `crates/wow-world/src/attributes.rs`
- Modify: `crates/wow-world/src/lib.rs` (add `pub mod attributes;` after `pub mod abilities;`), `crates/wow-world/src/gear_effect.rs` (`with_character`)
- Test: in-file

**Interfaces:**
- Consumes: `model::{Item, LevelStat, ClassStat}`, `equipment::Gear`, `gear_effect::REF_DPS`.
- Produces:
  - `pub mod stat { pub const MANA: u32 = 0; HEALTH = 1; AGILITY = 3; STRENGTH = 4; INTELLECT = 5; SPIRIT = 6; STAMINA = 7 }`
  - `pub struct Attributes { pub strength: u32, pub agility: u32, pub stamina: u32, pub intellect: u32, pub spirit: u32 }` (`Default`, `Clone, Copy, PartialEq, Eq, Debug`)
  - `pub fn gear_attributes(gear: &Gear, items: &[Item]) -> [i32; 8]`? **No:** `pub fn gear_bonus(gear: &Gear, items: &[Item]) -> Bonus` where `pub struct Bonus { pub strength: i32, pub agility: i32, pub stamina: i32, pub intellect: i32, pub spirit: i32 }`
  - `pub fn stamina_hp(stamina: u32) -> f32`, `pub fn attack_power(level: u32, strength: u32) -> f32`, `pub fn ranged_attack_power(level: u32, agility: u32) -> f32`, `pub fn ap_factor(ap: f32, ap_ref: f32) -> f32`
  - `pub struct Character { pub base: Attributes, pub total: Attributes, pub max_health: f32, pub melee_ap_factor: f32, pub ranged_ap_factor: f32 }`
  - `pub fn character(level_stats: &[LevelStat], class_stats: &[ClassStat], class: u32, level: u32, gear: &Gear, items: &[Item]) -> Option<Character>`
  - `pub fn sheet(c: &Character) -> String` (the equipment page's attribute lines)
  - `GearEffect::with_character(self, c: &Character) -> GearEffect`
  - `pub const HEALTH_OFFSET: f32 = 0.0`

- [ ] **Step 1: Write the failing tests**

Create `crates/wow-world/src/attributes.rs`:

```rust
//! A character's WoW attributes (spec `docs/superpowers/specs/2026-10-12-attributes-and-boss-rules-design.md`):
//! the database's base values for the class and level plus what the worn gear adds, and what they do:
//! health from Stamina, attack power from Strength (melee) and Agility (ranged), armor from Agility.
//! Intellect and Spirit are counted and shown but do nothing yet (no mana, no regeneration).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::{inv, tests::item, Gear, Slot};
    use crate::model::{ClassStat, Item, LevelStat};

    fn close(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    /// Human Warrior (class 1), levels 1-8, from the vMaNGOS tables.
    fn tables() -> (Vec<LevelStat>, Vec<ClassStat>) {
        let rows = [
            (1, 23, 20, 22, 20, 20, 20),
            (2, 24, 21, 23, 20, 20, 29),
            (3, 25, 21, 24, 20, 21, 38),
            (4, 26, 22, 25, 20, 22, 47),
            (5, 28, 23, 26, 20, 22, 56),
            (6, 29, 24, 27, 21, 22, 65),
            (7, 30, 24, 28, 21, 23, 74),
            (8, 31, 25, 29, 21, 23, 83),
        ];
        let levels = rows
            .iter()
            .map(|&(level, s, a, st, i, sp, _)| LevelStat {
                class: 1,
                level,
                strength: s,
                agility: a,
                stamina: st,
                intellect: i,
                spirit: sp,
            })
            .collect();
        let classes = rows
            .iter()
            .map(|&(level, _, _, _, _, _, hp)| ClassStat {
                class: 1,
                level,
                base_hp: hp,
                base_mana: 0,
            })
            .collect();
        (levels, classes)
    }

    fn with_stats(entry: u32, inventory_type: u32, stats: Vec<(u32, i32)>) -> Item {
        let mut i = item(entry, inventory_type, 4, 2);
        i.stats = stats;
        i
    }

    #[test]
    fn stamina_gives_a_point_each_to_twenty_and_ten_above() {
        assert_eq!(stamina_hp(0), 0.0);
        assert_eq!(stamina_hp(20), 20.0);
        assert_eq!(stamina_hp(21), 30.0);
        assert_eq!(stamina_hp(22), 40.0);
        assert_eq!(stamina_hp(26), 80.0);
    }

    #[test]
    fn a_human_warriors_health_follows_the_class_table_and_stamina() {
        let (l, c) = tables();
        let at = |level| {
            character(&l, &c, 1, level, &Gear::default(), &[])
                .unwrap()
                .max_health
        };
        assert_eq!(at(1), 60.0, "20 base + 40 from 22 stamina");
        assert_eq!(at(3), 98.0);
        assert_eq!(at(5), 136.0);
        assert_eq!(at(8), 193.0);
    }

    #[test]
    fn gear_stats_add_to_the_base_and_four_stamina_is_forty_health() {
        let (l, c) = tables();
        let boots = with_stats(6095, inv::FEET, vec![(stat::STAMINA, 4), (stat::SPIRIT, 4)]);
        let mut gear = Gear::default();
        gear.set(Slot::Feet, Some(6095));
        let bare = character(&l, &c, 1, 1, &Gear::default(), &[]).unwrap();
        let shod = character(&l, &c, 1, 1, &gear, &[boots]).unwrap();
        assert_eq!(shod.total.stamina, bare.total.stamina + 4);
        assert_eq!(shod.total.spirit, bare.total.spirit + 4);
        assert_eq!(shod.base, bare.base, "the base does not move");
        assert_eq!(shod.max_health - bare.max_health, 40.0);
    }

    #[test]
    fn attack_power_and_its_factor_start_at_one_for_a_level_one_warrior() {
        assert_eq!(attack_power(1, 23), 29.0);
        assert_eq!(attack_power(5, 28), 51.0);
        assert_eq!(attack_power(1, 0), 0.0, "never negative");
        let (l, c) = tables();
        let one = character(&l, &c, 1, 1, &Gear::default(), &[]).unwrap();
        assert!(close(one.melee_ap_factor, 1.0, 1.0e-6));
        assert!(close(one.ranged_ap_factor, 1.0, 1.0e-6));
        let five = character(&l, &c, 1, 5, &Gear::default(), &[]).unwrap();
        assert!(close(five.melee_ap_factor, 1.503, 2.0e-3));
        let eight = character(&l, &c, 1, 8, &Gear::default(), &[]).unwrap();
        assert!(close(eight.melee_ap_factor, 1.846, 2.0e-3));
    }

    #[test]
    fn strength_from_gear_raises_the_factor() {
        let (l, c) = tables();
        let belt = with_stats(9, inv::WAIST, vec![(stat::STRENGTH, 3)]);
        let mut gear = Gear::default();
        gear.set(Slot::Waist, Some(9));
        let bare = character(&l, &c, 1, 5, &Gear::default(), &[]).unwrap();
        let belted = character(&l, &c, 1, 5, &gear, &[belt]).unwrap();
        assert!(belted.melee_ap_factor > bare.melee_ap_factor + 0.1);
    }

    #[test]
    fn a_level_outside_the_table_uses_its_first_or_last_row() {
        let (l, c) = tables();
        let zero = character(&l, &c, 1, 0, &Gear::default(), &[]).unwrap();
        let one = character(&l, &c, 1, 1, &Gear::default(), &[]).unwrap();
        assert_eq!(zero.max_health, one.max_health);
        let high = character(&l, &c, 1, 61, &Gear::default(), &[]).unwrap();
        let top = character(&l, &c, 1, 8, &Gear::default(), &[]).unwrap();
        assert_eq!(high.total, top.total);
    }

    #[test]
    fn no_table_row_for_the_class_means_no_character() {
        let (l, c) = tables();
        assert!(character(&l, &c, 4, 1, &Gear::default(), &[]).is_none());
        assert!(character(&[], &[], 1, 1, &Gear::default(), &[]).is_none());
    }

    #[test]
    fn unknown_stat_types_are_ignored_and_a_total_never_goes_below_zero() {
        let (l, c) = tables();
        let odd = with_stats(
            3,
            inv::HEAD,
            vec![(99, 5), (stat::STRENGTH, -1000), (stat::HEALTH, 7)],
        );
        let mut gear = Gear::default();
        gear.set(Slot::Head, Some(3));
        let ch = character(&l, &c, 1, 1, &gear, &[odd]).unwrap();
        assert_eq!(ch.total.strength, 0);
        assert_eq!(ch.total.agility, 20);
    }

    #[test]
    fn the_effect_gains_two_armor_per_agility_and_the_attack_power_factors() {
        use crate::gear_effect::GearEffect;
        let (l, c) = tables();
        let ch = character(&l, &c, 1, 5, &Gear::default(), &[]).unwrap();
        let fx = GearEffect::default().with_character(&ch);
        assert_eq!(fx.armor, 2 * 23);
        assert!(close(fx.melee_factor, ch.melee_ap_factor, 1.0e-6));
        assert!(close(fx.ranged_factor, ch.ranged_ap_factor, 1.0e-6));
    }

    #[test]
    fn the_sheet_shows_base_plus_gear_and_health() {
        let (l, c) = tables();
        let belt = with_stats(9, inv::WAIST, vec![(stat::STRENGTH, 3), (stat::STAMINA, 4)]);
        let mut gear = Gear::default();
        gear.set(Slot::Waist, Some(9));
        let ch = character(&l, &c, 1, 5, &gear, &[belt]).unwrap();
        assert_eq!(
            sheet(&ch),
            "Strength 31 (+3)   Agility 23   Stamina 30 (+4)\nIntellect 20   Spirit 22   Health 176"
        );
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world attributes 2>&1 | grep -E "^error|cannot find" | head -5`
Expected: compile errors (`character`, `stamina_hp`, `stat` not found).

- [ ] **Step 3: Implement**

Add `pub mod attributes;` to `lib.rs`. In `attributes.rs` above the tests:

```rust
use crate::equipment::Gear;
use crate::gear_effect::REF_DPS;
use crate::model::{ClassStat, Item, LevelStat};

/// `item_template.stat_type` ids in the 1.12 data.
pub mod stat {
    pub const MANA: u32 = 0;
    pub const HEALTH: u32 = 1;
    pub const AGILITY: u32 = 3;
    pub const STRENGTH: u32 = 4;
    pub const INTELLECT: u32 = 5;
    pub const SPIRIT: u32 = 6;
    pub const STAMINA: u32 = 7;
}

/// Added to every max health (0: WoW's own numbers, the user's choice; +40 puts level 1 back at 100).
pub const HEALTH_OFFSET: f32 = 0.0;
/// The first this many Stamina points give a health point each; every point above gives [`HP_PER_STAMINA`].
pub const STAMINA_FREE: u32 = 20;
pub const HP_PER_STAMINA: f32 = 10.0;
/// Warrior melee attack power: `level x 3 + Strength x 2 - 20`.
pub const AP_PER_LEVEL: f32 = 3.0;
pub const AP_PER_STRENGTH: f32 = 2.0;
pub const AP_BASE: f32 = -20.0;
/// Warrior ranged attack power (as MaNGOS has it): `level + Agility - 10`.
pub const RAP_PER_LEVEL: f32 = 1.0;
pub const RAP_BASE: f32 = -10.0;
/// One damage per second per this much attack power.
pub const AP_PER_DPS: f32 = 14.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attributes {
    pub strength: u32,
    pub agility: u32,
    pub stamina: u32,
    pub intellect: u32,
    pub spirit: u32,
}

/// What the worn gear adds to each attribute (can be negative in the data; the total is floored at 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bonus {
    pub strength: i32,
    pub agility: i32,
    pub stamina: i32,
    pub intellect: i32,
    pub spirit: i32,
}

/// Every worn piece's stats summed per attribute. Unknown stat types (and mana / health) are ignored.
pub fn gear_bonus(gear: &Gear, items: &[Item]) -> Bonus {
    let mut b = Bonus::default();
    for (_, entry) in gear.iter() {
        let Some(item) = items.iter().find(|i| i.entry == entry) else {
            continue;
        };
        for &(kind, value) in &item.stats {
            match kind {
                stat::STRENGTH => b.strength += value,
                stat::AGILITY => b.agility += value,
                stat::STAMINA => b.stamina += value,
                stat::INTELLECT => b.intellect += value,
                stat::SPIRIT => b.spirit += value,
                _ => {}
            }
        }
    }
    b
}

/// The health Stamina gives.
pub fn stamina_hp(stamina: u32) -> f32 {
    let free = stamina.min(STAMINA_FREE) as f32;
    let above = stamina.saturating_sub(STAMINA_FREE) as f32;
    free + above * HP_PER_STAMINA
}

pub fn attack_power(level: u32, strength: u32) -> f32 {
    (level as f32 * AP_PER_LEVEL + strength as f32 * AP_PER_STRENGTH + AP_BASE).max(0.0)
}

pub fn ranged_attack_power(level: u32, agility: u32) -> f32 {
    (level as f32 * RAP_PER_LEVEL + agility as f32 + RAP_BASE).max(0.0)
}

/// How much harder a blow lands at `ap` than at `ap_ref`, for the starting weapon: WoW adds `ap / 14`
/// damage per second to the weapon's own, normalised so `ap_ref` is exactly 1.
pub fn ap_factor(ap: f32, ap_ref: f32) -> f32 {
    (REF_DPS + ap / AP_PER_DPS) / (REF_DPS + ap_ref / AP_PER_DPS)
}

/// The row of `class` at `level`, the level clamped to the class's first and last row.
fn row_at<T: Copy>(rows: &[T], class: u32, level: u32, key: impl Fn(&T) -> (u32, u32)) -> Option<T> {
    let of_class: Vec<&T> = rows.iter().filter(|r| key(r).0 == class).collect();
    let first = of_class.iter().map(|r| key(r).1).min()?;
    let last = of_class.iter().map(|r| key(r).1).max()?;
    let want = level.clamp(first, last);
    of_class.into_iter().find(|r| key(r).1 == want).copied()
}

/// The lowest and highest level the table has for `class`.
fn level_bounds(rows: &[LevelStat], class: u32) -> Option<(u32, u32)> {
    let mut levels = rows.iter().filter(|r| r.class == class).map(|r| r.level);
    let first = levels.next()?;
    Some(levels.fold((first, first), |(lo, hi), l| (lo.min(l), hi.max(l))))
}

/// What a character is and does from its class tables and gear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Character {
    /// The database's base attributes at the level.
    pub base: Attributes,
    /// Base plus gear.
    pub total: Attributes,
    pub max_health: f32,
    pub melee_ap_factor: f32,
    pub ranged_ap_factor: f32,
}

fn add(base: u32, bonus: i32) -> u32 {
    (i64::from(base) + i64::from(bonus)).max(0) as u32
}

/// `None` when the tables have no row for the class (a manifest older than `wsworld 3`, or a class the
/// game does not know): the caller keeps the old fixed vitals.
pub fn character(
    level_stats: &[LevelStat],
    class_stats: &[ClassStat],
    class: u32,
    level: u32,
    gear: &Gear,
    items: &[Item],
) -> Option<Character> {
    let l = row_at(level_stats, class, level, |r| (r.class, r.level))?;
    let c = row_at(class_stats, class, level, |r| (r.class, r.level))?;
    let first = row_at(level_stats, class, 1, |r| (r.class, r.level))?;
    let base = Attributes {
        strength: l.strength,
        agility: l.agility,
        stamina: l.stamina,
        intellect: l.intellect,
        spirit: l.spirit,
    };
    let b = gear_bonus(gear, items);
    let total = Attributes {
        strength: add(base.strength, b.strength),
        agility: add(base.agility, b.agility),
        stamina: add(base.stamina, b.stamina),
        intellect: add(base.intellect, b.intellect),
        spirit: add(base.spirit, b.spirit),
    };
    // The level the tables stand for: a level above the table reads its last row, below its first.
    let (lo, hi) = level_bounds(level_stats, class)?;
    let lvl = level.clamp(lo, hi);
    let max_health = c.base_hp as f32 + stamina_hp(total.stamina) + HEALTH_OFFSET;
    let ap_ref = attack_power(1, first.strength);
    let rap_ref = ranged_attack_power(1, first.agility);
    Some(Character {
        base,
        total,
        max_health,
        melee_ap_factor: ap_factor(attack_power(lvl, total.strength), ap_ref),
        ranged_ap_factor: ap_factor(ranged_attack_power(lvl, total.agility), rap_ref),
    })
}

/// The equipment page's attribute lines: each attribute with the gear's share, then health.
pub fn sheet(c: &Character) -> String {
    let one = |name: &str, base: u32, total: u32| {
        if total > base {
            format!("{name} {total} (+{})", total - base)
        } else {
            format!("{name} {total}")
        }
    };
    format!(
        "{}   {}   {}\n{}   {}   Health {}",
        one("Strength", c.base.strength, c.total.strength),
        one("Agility", c.base.agility, c.total.agility),
        one("Stamina", c.base.stamina, c.total.stamina),
        one("Intellect", c.base.intellect, c.total.intellect),
        one("Spirit", c.base.spirit, c.total.spirit),
        c.max_health.round() as i64,
    )
}
```

In `gear_effect.rs` add to `impl GearEffect`:

```rust
    /// The attributes' share of the effect: armor from Agility, the attack-power factors on the damage
    /// factors.
    pub fn with_character(mut self, c: &crate::attributes::Character) -> Self {
        self.armor += crate::attributes::ARMOR_PER_AGILITY * c.total.agility;
        self.melee_factor *= c.melee_ap_factor;
        self.ranged_factor *= c.ranged_ap_factor;
        self
    }
```

and in `attributes.rs` add `pub const ARMOR_PER_AGILITY: u32 = 2;`.

- [ ] **Step 4: Run to verify it passes**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world attributes 2>&1 | grep -E "test |test result"`
Expected: 10 tests pass. The `the_sheet_...` expectation: base 28/23/26/20/22 + belt (+3 Str, +4 Sta) at level 5; health 56 + (20 + 10 x 10 = 120) + 0 = 176 ✓.

- [ ] **Step 5: Gates**

Run: `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check && cargo clippy -j 4 -p wow-world --all-targets -- -D warnings`
Expected: exit 0. No commit.

---

### Task 3: The game takes its health and its factors from the attributes

**Files:**
- Modify: `crates/wow-souls/src/level.rs`, `crates/wow-souls/src/gear_effect.rs`
- Test: in-file (`level.rs` tests adjusted, `gear_effect.rs` one new)

**Interfaces:**
- Consumes: `wow_world::attributes::{character, Character}`, `QuestGame.player.{class, level}`, `Owned.progress.gear`, `Data.0.{items, level_stats, class_stats}`.
- Produces: `crate::gear_effect::character(game: &QuestGame, owned: &Owned, data: &Data) -> Option<Character>`; `level::follow(hp_max: f32, st_max: f32, first: bool, hp: &mut Health, st: &mut Stamina)`; `level::max_stamina(level: u32) -> f32`.

- [ ] **Step 1: Adjust and add the failing tests (`level.rs`)**

Replace the tests that call `follow(level, applied, ..)` so they pass maxima. In `crates/wow-souls/src/level.rs` `mod tests`, replace `the_first_application_fills_the_player` ... `a_dead_player_is_not_healed_by_a_level` with:

```rust
    #[test]
    fn the_first_application_fills_the_player() {
        let mut hp = Health::full(100.0);
        let mut st = Stamina::full(100.0);
        hp.cur = 40.0;
        st.cur = 10.0;
        follow(120.0, 110.0, true, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (120.0, 120.0));
        assert_eq!((st.cur, st.max), (110.0, 110.0));
    }

    #[test]
    fn a_change_of_maxima_moves_the_current_values_by_the_same_difference() {
        let mut hp = Health::full(100.0);
        let mut st = Stamina::full(100.0);
        hp.cur = 40.0;
        st.cur = 30.0;
        follow(120.0, 110.0, false, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (60.0, 120.0), "40 + 20, no full heal");
        assert_eq!((st.cur, st.max), (40.0, 110.0), "30 + 10");
    }

    #[test]
    fn a_fall_in_the_maximum_never_kills_and_a_full_player_stays_full() {
        let mut hp = Health::full(100.0);
        let mut st = Stamina::full(100.0);
        hp.cur = 5.0;
        follow(60.0, 100.0, false, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (1.0, 60.0), "floored at 1 while alive");
        let mut full = Health::full(100.0);
        follow(140.0, 100.0, false, &mut full, &mut st);
        assert_eq!((full.cur, full.max), (140.0, 140.0));
    }

    #[test]
    fn a_dead_player_is_not_healed_by_a_new_maximum() {
        let mut hp = Health::full(100.0);
        let mut st = Stamina::full(100.0);
        hp.cur = 0.0;
        follow(110.0, 105.0, false, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (0.0, 110.0));
    }

    #[test]
    fn the_stamina_bar_still_grows_five_a_level() {
        assert_eq!(max_stamina(1), 100.0);
        assert_eq!(max_stamina(5), 120.0);
        assert_eq!(max_stamina(0), 100.0);
    }
```

Keep `only_a_rise_over_an_applied_level_dings` and `a_level_adds_ten_health_and_five_stamina` (the fallback `vitals` stays). In `crates/wow-souls/src/gear_effect.rs` tests add:

```rust
    #[test]
    fn without_tables_there_is_no_character_and_the_effect_stays_the_gears_own() {
        let none = wow_world::attributes::character(
            &[],
            &[],
            1,
            3,
            &wow_world::equipment::Gear::default(),
            &[],
        );
        assert!(none.is_none());
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls level 2>&1 | grep -E "^error|cannot find|expected" | head`
Expected: `follow` takes 4 args (found 5), `max_stamina` not found.

- [ ] **Step 3: Implement `level.rs`**

Replace `follow` and `follow_level` and add `max_stamina`:

```rust
/// The stamina bar of a level (a Dark Souls bar, WoW has none): 100 + 5 per level.
pub fn max_stamina(level: u32) -> f32 {
    STAMINA_BASE + STAMINA_PER_LEVEL * (level.max(1) - 1) as f32
}

/// Brings the maxima to `hp_max` / `st_max`. The first time (`first`, the avatar was just built from a
/// save or a fresh start) the player is full; after that the current values move by the same difference
/// as the maxima, so a level-up or a new piece of gear is a longer bar, not a full heal. A dead player
/// (no health) stays dead; a living one is never taken below 1 by a smaller maximum.
pub fn follow(hp_max: f32, st_max: f32, first: bool, hp: &mut Health, st: &mut Stamina) {
    if first {
        *hp = Health::full(hp_max);
        *st = Stamina::full(st_max);
        return;
    }
    if hp.cur > 0.0 {
        hp.cur = (hp.cur + hp_max - hp.max).clamp(1.0, hp_max);
    }
    hp.max = hp_max;
    st.cur = (st.cur + st_max - st.max).clamp(0.0, st_max);
    st.max = st_max;
}
```

`follow_level` becomes (keep the level-up flash code unchanged; only the head and the computation change):

```rust
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn follow_level(
    mut commands: Commands,
    game: Option<Res<QuestGame>>,
    owned: Option<Res<crate::loot::Owned>>,
    data: Option<Res<crate::loot::Data>>,
    mut applied: Local<Option<(u32, f32)>>,
    art: Option<Res<crate::loot::vfx::Art>>,
    audio: Option<ResMut<crate::audio::Audio>>,
    mut avatar: Query<(Entity, &mut Health, &mut Stamina), With<Avatar>>,
) {
    let Some(game) = game else {
        return;
    };
    let level = game.player.level;
    // From the class tables and the worn gear; without them (an older manifest) today's fixed vitals.
    let character = match (owned.as_deref(), data.as_deref()) {
        (Some(o), Some(d)) => crate::gear_effect::character(&game, o, d),
        _ => None,
    };
    let (hp_max, st_max) = match &character {
        Some(c) => (c.max_health, max_stamina(level)),
        None => vitals(level),
    };
    if *applied == Some((level, hp_max)) {
        return;
    }
    let Ok((me, mut hp, mut st)) = avatar.single_mut() else {
        return;
    };
    let rise = is_rise(applied.map(|a| a.0), level);
    follow(hp_max, st_max, applied.is_none(), &mut hp, &mut st);
    info!(
        "level: {level} -> health {hp_max}, stamina {st_max}{}",
        if character.is_some() { "" } else { " (no class tables: fixed vitals)" }
    );
    *applied = Some((level, hp_max));
    if rise {
        // ... the existing flash and chime block, unchanged ...
    }
}
```

(Keep the existing `if rise { ... }` body verbatim.) `vitals(level)` stays above as the fallback.

`gear_effect.rs`: add the helper and use it in `refresh`:

```rust
use crate::quests::QuestGame;

/// The character the tables and the worn gear make (`None` without class tables).
pub fn character(
    game: &QuestGame,
    owned: &Owned,
    data: &Data,
) -> Option<wow_world::attributes::Character> {
    wow_world::attributes::character(
        &data.0.level_stats,
        &data.0.class_stats,
        game.player.class as u32,
        game.player.level,
        &owned.progress.gear,
        &data.0.items,
    )
}
```

and change `refresh`: add parameter `game: Option<Res<QuestGame>>`, include `&& !game.as_ref().is_some_and(|g| g.is_changed())` in the early-return condition, and compute

```rust
    let held = off_hand_held(moves.as_deref());
    let mut now = if enabled.0 {
        GearEffect::from_gear(&owned.progress.gear, &data.0.items, held)
    } else {
        GearEffect::default()
    };
    if enabled.0 {
        if let Some(c) = game.as_deref().and_then(|g| character(g, &owned, &data)) {
            now = now.with_character(&c);
        }
    }
```

(Replace the old `let now = ...` block; keep the logging unchanged.)

- [ ] **Step 4: Run to verify it passes**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls level gear_effect 2>&1 | grep -E "^error|test result|FAILED"`
Expected: all pass; `cargo build -j 4 -p wow-souls` compiles.

- [ ] **Step 5: Gates**

Run: `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check && cargo clippy -j 4 --workspace --all-targets -- -D warnings`
Expected: exit 0. No commit.

---

### Task 4: Stats show in tooltips, comparisons and on the equipment page

**Files:**
- Modify: `crates/wow-souls/src/equip/mod.rs` (`tooltip`, `compare`), `crates/wow-souls/src/equip/ui.rs` (the `Words::Totals` arm)
- Test: in-file `mod tests` of `equip/mod.rs`

**Interfaces:**
- Consumes: `Item.stats`, `wow_world::attributes::{stat, sheet}`, `crate::gear_effect::character`.
- Produces: tooltip rows `+4 Stamina`; compare rows `+4 Stamina (+4)`; the Totals text = sheet + effect summary.

- [ ] **Step 1: Write the failing tests** (append to `equip/mod.rs` `mod tests`, using its local `item(entry, inventory_type, class, subclass)` helper)

```rust
    #[test]
    fn a_tooltip_lists_the_stats_after_armor_in_wows_words() {
        let mut vest = item(3288, inv::CHEST, 4, 2);
        vest.armor = 65;
        vest.required_level = 8;
        vest.stats = vec![(3, 2), (7, 1)];
        assert_eq!(
            tooltip(&vest),
            "Chest   Leather\n65 Armor\n+2 Agility\n+1 Stamina\nRequires Level 8"
        );
    }

    #[test]
    fn a_comparison_holds_each_stat_against_the_worn_piece_and_ignores_the_inert_ones() {
        let mut worn = item(1, inv::CHEST, 4, 2);
        worn.stats = vec![(7, 1)];
        let mut new = item(2, inv::CHEST, 4, 2);
        new.stats = vec![(7, 4), (6, 2)];
        let lines = compare(&new, Some(&worn), None);
        let stamina = lines.iter().find(|l| l.text.contains("Stamina")).unwrap();
        assert_eq!(stamina.text, "+4 Stamina (+3)");
        assert_eq!(stamina.trend, Trend::Better);
        let spirit = lines.iter().find(|l| l.text.contains("Spirit")).unwrap();
        assert_eq!(spirit.text, "+2 Spirit (+2)");
        assert_eq!(spirit.trend, Trend::Same, "spirit does nothing yet: shown, not weighed");
    }

    #[test]
    fn losing_a_stat_is_a_worse_row() {
        let mut worn = item(1, inv::CHEST, 4, 2);
        worn.stats = vec![(4, 3)];
        let new = item(2, inv::CHEST, 4, 2);
        let lines = compare(&new, Some(&worn), None);
        let strength = lines.iter().find(|l| l.text.contains("Strength")).unwrap();
        assert_eq!(strength.text, "+0 Strength (-3)");
        assert_eq!(strength.trend, Trend::Worse);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls equip::tests 2>&1 | grep -E "^error|FAILED|panicked" | head`
Expected: the three new tests fail (no stat rows yet).

- [ ] **Step 3: Implement**

In `equip/mod.rs` add a helper above `tooltip`:

```rust
/// A stat type's word in a tooltip (1.12's names); `None` for the types the game does not show.
fn stat_word(kind: u32) -> Option<&'static str> {
    use wow_world::attributes::stat;
    Some(match kind {
        stat::AGILITY => "Agility",
        stat::STRENGTH => "Strength",
        stat::INTELLECT => "Intellect",
        stat::SPIRIT => "Spirit",
        stat::STAMINA => "Stamina",
        stat::MANA => "Mana",
        stat::HEALTH => "Health",
        _ => return None,
    })
}

/// The order a tooltip lists stats in (WoW: Strength, Agility, Stamina, Intellect, Spirit).
const STAT_ORDER: [u32; 5] = [4, 3, 7, 5, 6];
```

In `tooltip`, after the `if item.block > 0 { .. }` block and before `required_level`:

```rust
    for kind in STAT_ORDER.into_iter().chain([0, 1]) {
        let value: i32 = item.stats.iter().filter(|s| s.0 == kind).map(|s| s.1).sum();
        if value != 0 {
            if let Some(word) = stat_word(kind) {
                lines.push(format!("{value:+} {word}"));
            }
        }
    }
```

In `compare`, before the `if let Some(also)` block:

```rust
    // Stats: each of the five attributes held against the worn piece. Intellect and Spirit do nothing
    // yet (no mana, no regeneration): shown, not weighed.
    let sum = |i: &Item, kind: u32| -> i32 { i.stats.iter().filter(|s| s.0 == kind).map(|s| s.1).sum() };
    for kind in STAT_ORDER {
        let (now, before) = (sum(new, kind), old.map_or(0, |o| sum(o, kind)));
        if now == 0 && before == 0 {
            continue;
        }
        let inert = matches!(kind, 5 | 6);
        let word = stat_word(kind).unwrap_or("");
        out.push(CompareLine {
            text: format!("{now:+} {word} ({:+})", now - before),
            trend: if inert { Trend::Same } else { trend_of((now - before) as f32) },
        });
    }
```

(The expected texts use `{now:+}`: `+0 Strength (-3)` and `+4 Stamina (+3)` match Rust's `{:+}` of 0 and 4.)

In `equip/ui.rs`, change the `Words::Totals` arm to compose the sheet and the effect line (keep `fx` computed as in the gear step, including the stowed-shield stance), using the `game`, `owned` and `data` parameters `refresh` already has:

```rust
            Words::Totals => {
                let fx = wow_world::gear_effect::GearEffect::from_gear(
                    gear,
                    items,
                    crate::gear_effect::off_hand_held(moves.as_deref()),
                );
                let character = game
                    .as_deref()
                    .and_then(|g| crate::gear_effect::character(g, &owned, &data));
                let (fx, head) = match character {
                    Some(c) => (fx.with_character(&c), wow_world::attributes::sheet(&c) + "\n"),
                    None => (fx, String::new()),
                };
                (format!("{head}{}", fx.summary()), IVORY)
            }
```

- [ ] **Step 4: Run to verify they pass**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls equip 2>&1 | grep -E "^error|test result|FAILED"`
Expected: all `equip` tests pass, including the existing tooltip / compare tests (items without stats print as before).

- [ ] **Step 5: Gates**

Run: `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check && cargo clippy -j 4 --workspace --all-targets -- -D warnings`
Expected: exit 0. No commit.

---

### Task 5: Quest items drop before the quest

**Files:**
- Modify: `crates/wow-world/src/quest/rules.rs`, `crates/wow-world/src/quest/log.rs`, `crates/wow-souls/src/quests/mod.rs` (`sync_items`), `crates/wow-souls/src/shop/mod.rs` (`needed_by`)
- Test: in-file, `quest/log.rs` `mod tests` (read its book fixture first) and `quest/rules.rs`

**Interfaces:**
- Produces:
  - `rules::takeable_later(book: &QuestBook, log: &QuestLog, p: &Player, quest: u32) -> bool`
  - `QuestLog::droppable_items(&self, book: &QuestBook, p: &Player, inventory: &dyn Fn(u32) -> u32) -> BTreeSet<u32>`
  - `QuestLog::pre_need(&self, book: &QuestBook, p: &Player, item: u32) -> u32` (the most of an item a not-yet-accepted, takeable quest asks for)

- [ ] **Step 1: Write the failing tests**

Both test modules run against the real valley data (`real_world()`, skipped when `cache/world/northshire.txt` is absent). The real quests used: 33 (needs 8 x item 750 Tough Wolf Meat, follows 5261), 18 (12 x item 752 Red Burlap Bandana, level 2), 6 (1 x item 182 Garrick's Head, level 2, needs 18 and 33 handed in), 3101 (a Mage's class letter).

In `crates/wow-world/src/quest/rules.rs` `mod tests` (its `log_of(rewarded, current)` and `warrior(level)` helpers exist) add:

```rust
    #[test]
    fn a_quest_that_is_not_ours_yet_is_still_takeable_later() {
        let Some(w) = real_world() else { return };
        let book = QuestBook::new(&w);
        let fresh = log_of(&[], &[]);
        // 33 wants its prerequisite, 18 and 6 want a level: all of them come later.
        for q in [33, 18, 6] {
            assert!(takeable_later(&book, &fresh, &warrior(1), q), "{q}");
        }
        // Another class's: never.
        assert!(!takeable_later(&book, &fresh, &warrior(1), 3101));
        // Handed in: never again.
        assert!(!takeable_later(&book, &log_of(&[18], &[]), &warrior(2), 18));
        // In the log: the accepted rule's business, not an early pickup.
        assert!(!takeable_later(
            &book,
            &log_of(&[], &[(18, Status::Incomplete)]),
            &warrior(2),
            18
        ));
    }
```

In `crates/wow-world/src/quest/log.rs` `mod tests` (helpers `book()`, `warrior(level)`, `nothing`) add:

```rust
    #[test]
    fn droppable_items_cover_untaken_quests_and_stop_when_carried_or_handed_in() {
        let Some(book) = book() else { return };
        let p = warrior(1);
        let log = QuestLog::default();
        // Before any quest: the wolf meat of 33, the bandanas of 18 and Garrick's head of 6 are worth
        // picking up.
        let fresh = log.droppable_items(&book, &p, &nothing);
        for item in [750, 752, 182] {
            assert!(fresh.contains(&item), "{item}");
        }
        // Carrying what a quest needs: no more of it.
        let carried = log.droppable_items(&book, &p, &|i| if i == 182 { 1 } else { 0 });
        assert!(!carried.contains(&182));
        // Quest 6 handed in: its head is wanted no more.
        let mut done = QuestLog::default();
        done.insert(6, Entry::new(Status::Rewarded));
        assert!(!done.droppable_items(&book, &p, &nothing).contains(&182));
        // The shop does not sell what a quest the player has not taken yet will ask for.
        assert_eq!(log.pre_need(&book, &p, 750), 8);
        assert_eq!(log.pre_need(&book, &p, 182), 1);
        assert_eq!(done.pre_need(&book, &p, 182), 0);
    }

    #[test]
    fn an_accepted_quest_is_the_accepted_rules_business_not_an_early_pickup() {
        let Some(book) = book() else { return };
        let p = warrior(1);
        let mut log = QuestLog::default();
        log.accept(&book, &p, 783, &nothing).unwrap();
        log.turn_in(&book, &p, 783, Npc::Creature(197), None, &nothing)
            .unwrap();
        log.accept(&book, &p, 5261, &nothing).unwrap();
        log.turn_in(&book, &p, 5261, Npc::Creature(196), None, &nothing)
            .unwrap();
        log.accept(&book, &p, 33, &nothing).unwrap();
        assert!(log.droppable_items(&book, &p, &nothing).contains(&750), "while incomplete");
        assert_eq!(log.pre_need(&book, &p, 750), 0, "33 is in the log");
    }
```

(Add `use crate::quest::log::Entry;` to `log.rs` tests if it is not in scope.)

- [ ] **Step 2: Run to verify they fail**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world quest 2>&1 | grep -E "^error|cannot find" | head`
Expected: `takeable_later` / `droppable_items` / `pre_need` not found.

- [ ] **Step 3: Implement**

`rules.rs`, after `can_take`:

```rust
/// Whether the player could still take `quest` some day: it is not handed in, not another class's or
/// race's, not shut by a taken exclusive-group sibling or a chain, and the game offers it. A level or a
/// prerequisite that is not met yet does not count (the player will meet it): this is what lets a quest
/// item be picked up before the quest is accepted.
pub fn takeable_later(book: &QuestBook, log: &QuestLog, p: &Player, quest: u32) -> bool {
    matches!(
        why_not(book, log, p, quest),
        None | Some(Refusal::Level { .. } | Refusal::Prerequisite)
    )
}
```

(`AlreadyHave` is excluded on purpose: a quest in the log is handled by `wanted_items`; `Chain` means a successor / predecessor is current, so no early pickup.)

`log.rs`, after `wanted_items`:

```rust
    /// What a corpse may drop for quests: everything `wanted_items` holds, and the required items of
    /// every quest the player has not taken yet but could (see [`rules::takeable_later`]), up to the
    /// count already carried.
    pub fn droppable_items(
        &self,
        book: &QuestBook,
        p: &Player,
        inventory: &dyn Fn(u32) -> u32,
    ) -> BTreeSet<u32> {
        let mut out = self.wanted_items(book, inventory);
        for id in book.ids() {
            if self.entry(id).is_some() || !rules::takeable_later(book, self, p, id) {
                continue;
            }
            let Some(q) = book.quest(id) else { continue };
            for &(item, need) in &q.req_items {
                if inventory(item) < need {
                    out.insert(item);
                }
            }
        }
        out
    }

    /// The most of `item` a quest the player has not taken yet (but could) will ask for: the shop does
    /// not sell that many.
    pub fn pre_need(&self, book: &QuestBook, p: &Player, item: u32) -> u32 {
        book.ids()
            .filter(|&id| self.entry(id).is_none() && rules::takeable_later(book, self, p, id))
            .filter_map(|id| book.quest(id))
            .flat_map(|q| q.req_items.iter())
            .filter(|&&(it, _)| it == item)
            .map(|&(_, n)| n)
            .max()
            .unwrap_or(0)
    }
```

(import `super::rules`, `super::Player` in `log.rs` if not already: check the file's `use` lines.)

`quests/mod.rs` `sync_items`: replace `let wanted = g.log.wanted_items(&g.book, &|i| inv.count(i));` with:

```rust
    let wanted = g
        .log
        .droppable_items(&g.book, &g.player, &|i| inv.count(i));
```

`shop/mod.rs` `needed_by`: replace the final expression so it also counts quests not yet taken:

```rust
pub fn needed_by(game: Option<&QuestGame>, entry: u32) -> u32 {
    let Some(g) = game else {
        return 0;
    };
    let accepted = g
        .log
        .active()
        .into_iter()
        .filter_map(|q| g.book.quest(q))
        .flat_map(|q| q.req_items.iter())
        .filter(|&&(item, _)| item == entry)
        .map(|&(_, n)| n)
        .max()
        .unwrap_or(0);
    accepted.max(g.log.pre_need(&g.book, &g.player, entry))
}
```

(`Refusal::Needed`'s message "A quest you have accepted still needs that item." becomes "A quest still needs that item." in `wow-world/src/shop.rs`; update its one test string if one pins it: `grep -rn "you have accepted still needs"`.)

- [ ] **Step 4: Run to verify they pass**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world quest 2>&1 | grep -E "test result|FAILED"; cargo test -j 4 -p wow-souls quests shop 2>&1 | grep -E "test result|FAILED"`
Expected: pass.

- [ ] **Step 5: Gates**

Run: `cargo fmt ... --check && cargo clippy -j 4 --workspace --all-targets -- -D warnings`
Expected: exit 0. No commit.

---

### Task 6: A rank on every species

**Files:**
- Modify: `crates/wow-souls/src/species.rs` (`SpeciesDef.boss` becomes `rank`), `crates/wow-souls/src/npc.rs` (the `def.boss` insert, ~line 550)
- Test: in-file `species.rs` tests

**Interfaces:**
- Produces: `species::Rank { Normal, Rare, Miniboss, Boss }` with `never_respawns()`, `first_kill_best()`, `parse(&str)`; `SpeciesDef.rank: Rank` (replaces `boss: bool`); `species::rank_of(def: &SpeciesDef) -> Rank` (honours `WOWSOULS_RANK=<species>:<rank>[,..]`, read once).

- [ ] **Step 1: Write the failing tests** (in `species.rs` `mod tests`)

```rust
    #[test]
    fn rank_rules_follow_the_spec_table() {
        assert!(!Rank::Normal.never_respawns() && !Rank::Normal.first_kill_best());
        assert!(!Rank::Rare.never_respawns() && Rank::Rare.first_kill_best());
        assert!(Rank::Miniboss.never_respawns() && !Rank::Miniboss.first_kill_best());
        assert!(Rank::Boss.never_respawns() && Rank::Boss.first_kill_best());
    }

    #[test]
    fn garrick_is_the_valleys_one_boss() {
        let bosses: Vec<_> = ROSTER.iter().filter(|s| s.rank == Rank::Boss).map(|s| s.name).collect();
        assert_eq!(bosses, vec!["garrick"]);
        assert!(ROSTER.iter().all(|s| s.rank == Rank::Boss || s.rank == Rank::Normal));
    }

    #[test]
    fn the_rank_switch_parses_names_and_ignores_garbage() {
        assert_eq!(Rank::parse("boss"), Some(Rank::Boss));
        assert_eq!(Rank::parse(" Rare "), Some(Rank::Rare));
        assert_eq!(Rank::parse("miniboss"), Some(Rank::Miniboss));
        assert_eq!(Rank::parse("normal"), Some(Rank::Normal));
        assert_eq!(Rank::parse("elite"), None);
        let map = parse_rank_overrides("kobold:boss, wolf:rare ,bad,thug:nope");
        assert_eq!(map.get("kobold"), Some(&Rank::Boss));
        assert_eq!(map.get("wolf"), Some(&Rank::Rare));
        assert_eq!(map.len(), 2);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls species 2>&1 | grep -E "^error|cannot find" | head -4`
Expected: `Rank` not found.

- [ ] **Step 3: Implement**

In `species.rs`, replace the `boss` field and its doc with `pub rank: Rank,` and doc "How special it is: a Boss gets the screen's bar and neither respawns nor drops twice (see [`Rank`])"; replace each `boss: false,` with `rank: Rank::Normal,` and `garrick`'s `boss: true,` with `rank: Rank::Boss,`. Add (before `SpeciesDef`):

```rust
/// How special a creature is. The database cannot say (every valley creature, Garrick included, is
/// rank 0 there), so the species row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    Normal,
    /// Respawns like anything else (4 x its database time), but drops the best of its table the first
    /// time it is killed.
    Rare,
    /// Never respawns.
    Miniboss,
    /// Never respawns, and drops the best of its table the first time it is killed.
    Boss,
}

impl Rank {
    pub fn never_respawns(self) -> bool {
        matches!(self, Rank::Miniboss | Rank::Boss)
    }

    pub fn first_kill_best(self) -> bool {
        matches!(self, Rank::Rare | Rank::Boss)
    }

    pub fn parse(s: &str) -> Option<Rank> {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Rank::Normal),
            "rare" => Some(Rank::Rare),
            "miniboss" => Some(Rank::Miniboss),
            "boss" => Some(Rank::Boss),
            _ => None,
        }
    }
}

/// `WOWSOULS_RANK=<species>:<rank>[,...]` as a map (a pair that does not read is dropped).
pub fn parse_rank_overrides(v: &str) -> std::collections::HashMap<String, Rank> {
    v.split(',')
        .filter_map(|p| p.split_once(':'))
        .filter_map(|(name, rank)| Some((name.trim().to_string(), Rank::parse(rank)?)))
        .collect()
}

/// The rank a species plays at: its row, unless `WOWSOULS_RANK` names it (a live check can make a kobold a
/// boss to watch the first-kill drop and the stay-dead rule).
pub fn rank_of(def: &SpeciesDef) -> Rank {
    static OVERRIDES: std::sync::OnceLock<std::collections::HashMap<String, Rank>> =
        std::sync::OnceLock::new();
    OVERRIDES
        .get_or_init(|| {
            std::env::var("WOWSOULS_RANK")
                .map(|v| parse_rank_overrides(&v))
                .unwrap_or_default()
        })
        .get(def.name)
        .copied()
        .unwrap_or(def.rank)
}
```

In `npc.rs` replace `if def.boss {` with `if crate::species::rank_of(def) == crate::species::Rank::Boss {`.

- [ ] **Step 4: Run to verify they pass**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls species 2>&1 | grep -E "test result|FAILED|^error"; grep -rn "\.boss\b" crates/wow-souls/src --include=*.rs | grep -v "follow.boss\|hud/boss"`
Expected: pass; the grep shows no remaining `def.boss` use.

- [ ] **Step 5: Gates**

As before (fmt check, workspace clippy). No commit.

---

### Task 7: The best item of a table (`best_drop`)

**Files:**
- Modify: `crates/wow-world/src/loot.rs`
- Test: in-file

**Interfaces:**
- Consumes: `model::{Item, LootKind, LootRow}`.
- Produces: `pub fn best_drop(rows: &[LootRow], items: &[Item], kind: LootKind, table: u32) -> Option<u32>`.

- [ ] **Step 1: Write the failing tests** (append to `loot.rs` `mod tests`; needs `use crate::model::Item;` and a helper)

```rust
    fn gear(entry: u32, class: u32, quality: u32, ilvl: u32, inv_type: u32) -> crate::model::Item {
        crate::model::Item {
            entry,
            name: format!("Item {entry}"),
            class,
            subclass: 0,
            display: 0,
            quality,
            flags: 0,
            buy_price: 0,
            sell_price: 0,
            inventory_type: inv_type,
            allowable_class: -1,
            allowable_race: -1,
            item_level: ilvl,
            required_level: 1,
            max_count: 0,
            stackable: 1,
            delay: 0,
            damage: (0.0, 0.0, 0),
            block: 0,
            armor: 0,
            stats: vec![],
        }
    }

    #[test]
    fn the_best_drop_prefers_gear_then_quality_then_item_level_then_the_rarer_chance() {
        let rows = [row(1, 12.0, 0), row(2, 0.8, 0), row(3, 5.0, 0), row(4, 2.0, 0), row(5, 9.0, 0)];
        let items = [
            gear(1, 0, 1, 5, 0),  // a consumable, ilvl 5
            gear(2, 2, 1, 4, 21), // a q1 weapon, ilvl 4, 0.8 %
            gear(3, 4, 1, 4, 5),  // a q1 chest piece, ilvl 4, 5 %
            gear(4, 4, 2, 3, 7),  // a q2 legs piece, ilvl 3
            gear(5, 4, 2, 3, 8),  // a q2 boots piece, ilvl 3, commoner than 4
        ];
        assert_eq!(best_drop(&rows, &items, LootKind::Creature, 1), Some(4), "q2 gear, then the rarer");
        let no_green = [items[0].clone(), items[1].clone(), items[2].clone()];
        assert_eq!(best_drop(&rows[..3], &no_green, LootKind::Creature, 1), Some(2), "q1 gear of ilvl 4: the rarer chance wins");
    }

    #[test]
    fn quest_rows_and_unknown_items_are_never_the_best() {
        let rows = [row(9, -100.0, 0), row(8, 50.0, 0)];
        let items = [gear(9, 2, 3, 30, 21)]; // 8 has no row in the table
        assert_eq!(best_drop(&rows, &items, LootKind::Creature, 1), None);
    }

    #[test]
    fn references_are_expanded_and_a_loop_ends() {
        let mut to_ref = row(0, 2.5, 0);
        to_ref.min_or_ref = -2;
        to_ref.max_count = 1;
        let mut inside = row(7, 0.0, 1);
        inside.kind = LootKind::Reference;
        inside.table = 2;
        let items = [gear(7, 4, 0, 9, 5)];
        assert_eq!(best_drop(&[to_ref.clone(), inside], &items, LootKind::Creature, 1), Some(7));
        let mut cycle = to_ref.clone();
        cycle.kind = LootKind::Reference;
        cycle.table = 2;
        assert_eq!(best_drop(&[to_ref, cycle], &items, LootKind::Creature, 1), None, "a loop yields nothing, no panic");
    }

    #[test]
    fn an_empty_table_has_no_best_drop() {
        assert_eq!(best_drop(&[], &[], LootKind::Creature, 1), None);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world best_drop 2>&1 | grep -E "^error|cannot find" | head -3`
Expected: `best_drop` not found.

- [ ] **Step 3: Implement** (in `loot.rs`, after `roll`)

```rust
/// The item a boss or a rare is guaranteed the first time it dies: of the table's non-quest rows
/// (reference groups expanded), gear (a weapon or armor that goes on a slot) before anything else, then
/// the higher quality, the higher item level, and the lower drop chance. `None` for a table with no
/// known item. Quest rows (a negative chance) and items the item table lacks are left out.
pub fn best_drop(
    rows: &[LootRow],
    items: &[crate::model::Item],
    kind: LootKind,
    table: u32,
) -> Option<u32> {
    let mut found: Vec<(u32, f32)> = Vec::new();
    collect(rows, kind, table, 0, &mut found);
    found
        .into_iter()
        .filter_map(|(entry, chance)| {
            let item = items.iter().find(|i| i.entry == entry)?;
            let is_gear = matches!(item.class, 2 | 4) && item.inventory_type != 0;
            // A chance of 0 is "an equal share of its group": treated as common.
            let chance = if chance > 0.0 { chance } else { 100.0 };
            Some((entry, (is_gear, item.quality, item.item_level), chance))
        })
        .max_by(|a, b| {
            a.1.cmp(&b.1)
                .then(b.2.total_cmp(&a.2)) // the lower chance is the better
                .then(b.0.cmp(&a.0)) // a stable tie: the lower entry
        })
        .map(|(entry, _, _)| entry)
}

fn collect(rows: &[LootRow], kind: LootKind, table: u32, depth: usize, out: &mut Vec<(u32, f32)>) {
    if depth >= 12 {
        return;
    }
    for row in rows.iter().filter(|r| r.kind == kind && r.table == table) {
        if row.chance < 0.0 {
            continue;
        }
        if row.min_or_ref < 0 {
            collect(rows, LootKind::Reference, row.min_or_ref.unsigned_abs(), depth + 1, out);
        } else if row.item > 0 {
            out.push((row.item, row.chance));
        }
    }
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world loot 2>&1 | grep -E "test |test result"`
Expected: pass (4 new + the 3 old).

- [ ] **Step 5: Gates**

As before. No commit.

---

### Task 8: The world state (slain, first kills, held loot) and never-respawn

**Files:**
- Create: `crates/wow-world/src/world_state.rs`, `crates/wow-souls/src/world_state.rs`
- Modify: `crates/wow-world/src/lib.rs` (`pub mod world_state;`), `crates/wow-souls/src/main.rs` (`mod world_state;`), `crates/wow-souls/src/boot.rs` (plugin), `crates/wow-souls/src/ring.rs`, `crates/wow-souls/src/world.rs`, `crates/wow-souls/src/npc.rs` (`reset`)
- Test: in-file (pure state, ring)

**Interfaces:**
- Produces:
  - `wow_world::world_state::WorldState { slain: BTreeSet<u32>, first: BTreeSet<u32>, held: BTreeMap<u32, Vec<(u32, u32)>> }` with `parse(text: &str) -> (WorldState, Vec<String>)` (the rows that could not be read, never a failure), `write(&self) -> String`.
  - `ring::NEVER: f32`; `ring::note_gone_for_good(r: &mut Rec)`; `clear_timers` keeps `until == Some(NEVER)`.
  - game: `Resource WorldStateFile { pub state: WorldState, path: Option<PathBuf>, dirty: bool }` with `mark()`; plugin `world_state::plugin` (load at build, save in `Last`, deliver held items at Startup).

- [ ] **Step 1: Write the failing tests**

`crates/wow-world/src/world_state.rs` (tests only first):

```rust
//! The world's memory between runs, `wsstate 1`: which never-respawning creatures are dead (`slain`, by
//! spawn guid), which creature entries have given their first-kill drop (`first`), and the loot a dead
//! never-respawning creature still carries (`held`). Plain text beside the other saves; a line that
//! cannot be read is skipped (and reported), the rest still loads.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_round_trips_in_a_stable_order() {
        let mut s = WorldState::default();
        s.slain.insert(80247);
        s.slain.insert(12);
        s.first.insert(103);
        s.held.insert(80247, vec![(2057, 1), (182, 1)]);
        let text = s.write();
        assert_eq!(
            text,
            "wsstate 1\nslain 12\nslain 80247\nfirst 103\nheld 80247 2057 1 182 1\n"
        );
        let (back, skipped) = WorldState::parse(&text);
        assert_eq!(back, s);
        assert!(skipped.is_empty());
    }

    #[test]
    fn a_bad_line_is_skipped_and_reported_and_the_rest_loads() {
        let text = "wsstate 1\nslain 5\nslain notanumber\nheld 7 9\nfirst 3\nwhat is this\nheld 8 100 2\n";
        let (s, skipped) = WorldState::parse(text);
        assert_eq!(s.slain.iter().copied().collect::<Vec<_>>(), vec![5]);
        assert_eq!(s.first.iter().copied().collect::<Vec<_>>(), vec![3]);
        assert_eq!(s.held.get(&8), Some(&vec![(100, 2)]));
        assert!(s.held.get(&7).is_none(), "an odd item / count list is not read");
        assert_eq!(skipped.len(), 3, "{skipped:?}");
    }

    #[test]
    fn an_empty_or_foreign_text_is_an_empty_state() {
        assert_eq!(WorldState::parse("").0, WorldState::default());
        let (s, skipped) = WorldState::parse("not a state file\nslain 3\n");
        assert!(s.slain.is_empty(), "the header decides: another file is not ours");
        assert!(!skipped.is_empty());
    }
}
```

`crates/wow-souls/src/ring.rs` tests (append):

```rust
    #[test]
    fn a_creature_gone_for_good_is_never_due_and_a_reset_leaves_it() {
        let mut dead = rec(5.0, State::Absent, None);
        note_gone_for_good(&mut dead);
        assert_eq!(dead.until, Some(NEVER));
        let p = plan(&[dead], [0.0, 0.0], 1.0e9, RING, 9, &nobody);
        assert!(p.spawn.is_empty(), "never spawns, however late");
        let mut recs = [dead, rec(5.0, State::Absent, Some(100.0))];
        clear_timers(&mut recs);
        assert_eq!(recs[0].until, Some(NEVER), "the reset does not revive it");
        assert_eq!(recs[1].until, None, "an ordinary timer is cleared");
    }

    #[test]
    fn a_corpse_that_is_gone_for_good_is_not_replaced() {
        let mut live = rec(5.0, State::Live, None);
        note_gone_for_good(&mut live);
        let p = plan(&[live], [0.0, 0.0], 1.0e9, RING, 9, &nobody);
        assert!(p.despawn.is_empty() && p.spawn.is_empty());
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world world_state 2>&1 | grep -E "^error" | head -3; cargo test -j 4 -p wow-souls ring 2>&1 | grep -E "^error" | head -3`
Expected: `WorldState` not found; `note_gone_for_good` / `NEVER` not found.

- [ ] **Step 3: Implement**

`wow-world/src/world_state.rs` (above the tests):

```rust
use std::collections::{BTreeMap, BTreeSet};

pub const MAGIC: &str = "wsstate";
pub const VERSION: &str = "1";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorldState {
    /// Spawn guids of never-respawning creatures that are dead for good.
    pub slain: BTreeSet<u32>,
    /// Creature entries whose first-kill best drop has been given.
    pub first: BTreeSet<u32>,
    /// A dead never-respawning creature's unlooted items (spawn guid -> (item, count) pairs).
    pub held: BTreeMap<u32, Vec<(u32, u32)>>,
}

impl WorldState {
    /// The text, in a stable order: header, slain, first, held.
    pub fn write(&self) -> String {
        let mut out = format!("{MAGIC} {VERSION}\n");
        for g in &self.slain {
            out.push_str(&format!("slain {g}\n"));
        }
        for e in &self.first {
            out.push_str(&format!("first {e}\n"));
        }
        for (g, items) in &self.held {
            out.push_str(&format!("held {g}"));
            for (item, count) in items {
                out.push_str(&format!(" {item} {count}"));
            }
            out.push('\n');
        }
        out
    }

    /// Reads a text. A file that does not start with the header is not ours (empty state, every line
    /// reported); a line that cannot be read is skipped and reported, the rest loads.
    pub fn parse(text: &str) -> (WorldState, Vec<String>) {
        let mut s = WorldState::default();
        let mut skipped = Vec::new();
        let mut lines = text.lines();
        let Some(head) = lines.next() else {
            return (s, skipped);
        };
        if head.trim() != format!("{MAGIC} {VERSION}") {
            skipped.push(format!("not a {MAGIC} {VERSION} file: {head:?}"));
            return (s, skipped);
        }
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let mut w = line.split_whitespace();
            let ok = match w.next() {
                Some("slain") => w
                    .next()
                    .and_then(|g| g.parse().ok())
                    .map(|g| s.slain.insert(g))
                    .is_some(),
                Some("first") => w
                    .next()
                    .and_then(|e| e.parse().ok())
                    .map(|e| s.first.insert(e))
                    .is_some(),
                Some("held") => {
                    let guid: Option<u32> = w.next().and_then(|g| g.parse().ok());
                    let nums: Option<Vec<u32>> = w.map(|n| n.parse().ok()).collect();
                    match (guid, nums) {
                        (Some(g), Some(n)) if !n.is_empty() && n.len() % 2 == 0 => {
                            s.held
                                .insert(g, n.chunks(2).map(|c| (c[0], c[1])).collect());
                            true
                        }
                        _ => false,
                    }
                }
                _ => false,
            };
            if !ok {
                skipped.push(line.to_string());
            }
        }
        (s, skipped)
    }
}
```

and `pub mod world_state;` in `wow-world/src/lib.rs`.

`ring.rs`: add after the `SIM` constant block:

```rust
/// The `until` of a creature that never comes back: no clock reaches it and a reset keeps it.
pub const NEVER: f32 = f32::INFINITY;

/// The creature of `r` is dead for good (a boss, a miniboss).
pub fn note_gone_for_good(r: &mut Rec) {
    r.until = Some(NEVER);
}
```

and change `clear_timers`:

```rust
pub fn clear_timers(recs: &mut [Rec]) {
    for r in recs {
        if r.until != Some(NEVER) {
            r.until = None;
        }
    }
}
```

`crates/wow-souls/src/world_state.rs` (game side):

```rust
//! The world's memory between runs: `WorldState` (the pure text save in `wow_world::world_state`) as a
//! resource, loaded when the plugin is built and saved when it changes. The file is
//! `world-state-v1.txt` beside the other saves (`WOWSOULS_WORLD_STATE=<file>` names another); an
//! unattended run without that switch keeps the state in memory only, so a test never kills the
//! player's boss.

use std::path::PathBuf;

use bevy::prelude::*;
use wow_world::world_state::WorldState;

#[derive(Resource)]
pub struct WorldStateFile {
    pub state: WorldState,
    path: Option<PathBuf>,
    dirty: bool,
}

impl WorldStateFile {
    /// Read `path` (a missing file is an empty state; a file that cannot be read keeps its rows that
    /// could be and is never written over until something changes).
    pub fn load(path: Option<PathBuf>) -> WorldStateFile {
        let state = match path.as_ref().map(std::fs::read_to_string) {
            Some(Ok(text)) => {
                let (s, skipped) = WorldState::parse(&text);
                for line in skipped {
                    warn!("world state: skipped {line:?}");
                }
                s
            }
            _ => WorldState::default(),
        };
        WorldStateFile {
            state,
            path,
            dirty: false,
        }
    }

    pub fn in_memory() -> WorldStateFile {
        WorldStateFile::load(None)
    }

    /// Something changed: write at the end of the frame.
    pub fn mark(&mut self) {
        self.dirty = true;
    }
}

/// `WOWSOULS_WORLD_STATE`, else beside the other saves; `None` for an unattended run with no switch.
pub fn save_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("WOWSOULS_WORLD_STATE") {
        return Some(PathBuf::from(p));
    }
    if crate::run_mode::unattended() {
        return None;
    }
    let dir = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "saves".into());
    Some(PathBuf::from(dir).join("WoWSouls").join("world-state-v1.txt"))
}

fn save(mut file: ResMut<WorldStateFile>) {
    if !file.dirty {
        return;
    }
    file.dirty = false;
    let Some(path) = file.path.clone() else {
        return;
    };
    let text = file.state.write();
    let tmp = path.with_extension("tmp");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, &path)) {
        warn!("world state: could not save {}: {e}", path.display());
    }
}

pub fn plugin(app: &mut App) {
    let file = WorldStateFile::load(save_path());
    info!(
        "world state: {} slain, {} first kills, {} held loots",
        file.state.slain.len(),
        file.state.first.len(),
        file.state.held.len()
    );
    app.insert_resource(file).add_systems(Last, save);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_written_by_one_run_is_read_by_the_next() {
        let dir = std::env::temp_dir().join(format!("wssave-{}", std::process::id()));
        let path = dir.join("world-state-v1.txt");
        let mut a = WorldStateFile::load(Some(path.clone()));
        a.state.slain.insert(80247);
        a.state.first.insert(103);
        a.mark();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, a.state.write()).unwrap();
        let b = WorldStateFile::load(Some(path));
        assert!(b.state.slain.contains(&80247) && b.state.first.contains(&103));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_or_foreign_file_is_an_empty_state() {
        let none = WorldStateFile::load(Some(PathBuf::from("Z:/no/such/file.txt")));
        assert!(none.state.slain.is_empty());
        assert!(WorldStateFile::in_memory().state.first.is_empty());
    }
}
```

Register: `mod world_state;` in `main.rs` (alphabetical, after `mod world;`), `.add_plugins(crate::world_state::plugin)` in `boot.rs` after `crate::bloodstain::plugin` (before `world::plugin` is registered: the resource must exist when `world::load` runs at Startup, and plugin build inserts it immediately, so order within `boot.rs` does not matter).

`world.rs`:
1. In `load` (the Startup system that ends with `commands.insert_resource(WorldBook::new(book, ring, respawn, density));`), take `state: Option<Res<crate::world_state::WorldStateFile>>` and after building the book mark the dead for good:

```rust
    let mut wb = WorldBook::new(book, ring, respawn, density);
    if let Some(state) = state.as_deref() {
        let mut n = 0;
        for (spot, rec) in wb.book.spots.iter().zip(wb.book.recs.iter_mut()) {
            if state.state.slain.contains(&spot.guid) {
                crate::ring::note_gone_for_good(rec);
                n += 1;
            }
        }
        if n > 0 {
            info!("ring: {n} creature(s) stay dead from an earlier run");
        }
    }
    commands.insert_resource(wb);
```

2. `note_deaths`: add `mut state: ResMut<crate::world_state::WorldStateFile>` and replace the body after `let spot = ...` with a rank branch:

```rust
        let wb = &mut *wb;
        let spot = &wb.book.spots[i];
        let never = match spot.kind {
            Kind::Foe(name) => crate::species::by_name(name)
                .is_some_and(|d| crate::species::rank_of(d).never_respawns()),
            _ => false,
        };
        if never {
            crate::ring::note_gone_for_good(&mut wb.book.recs[i]);
            state.state.slain.insert(spot.guid);
            state.mark();
            info!(
                "ring: {} #{i} guid {} died and stays dead (never respawns)",
                kind_name(spot.kind),
                spot.guid
            );
            continue;
        }
        let delay = wb.respawn.delay(spot.respawn_secs);
        // ... the existing note_death call and info! line, unchanged ...
```

3. The `keep` closure in `ring_tick`: add a clause so a dead never-respawn corpse is never despawned: after `let Some((xf, brain, reaction, _)) = ...` block's `else { return false; }` — the closure first looks up the entity; a record whose entity is None returns false. Add at the top of the closure:

```rust
            if wb.book.recs[i].until == Some(crate::ring::NEVER) {
                return entity[i].is_some();
            }
```

(`wb` is borrowed immutably there already via `wb.book.recs` in `crate::ring::plan(&wb.book.recs, ...)`; the closure only reads. If the borrow checker objects, copy `let never: Vec<bool> = wb.book.recs.iter().map(|r| r.until == Some(crate::ring::NEVER)).collect();` before the closure and read `never[i]`.)

`npc.rs` `reset`: add `&crate::species::Species` to the query (before the `(&mut MonsterPlay, ...)` group) and skip the dead never-respawn ones at the top of the loop body:

```rust
        if crate::species::rank_of(species.0).never_respawns() && *reaction == Reaction::Dead {
            continue;
        }
```

(with `species` bound in the `for` pattern: `for (e, mut xf, home, mut hp, mut reaction, mut brain, mut hostile, mut m, species, kit, body) in &mut npcs`).

- [ ] **Step 4: Run to verify they pass**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world world_state 2>&1 | grep -E "test result|FAILED"; cargo test -j 4 -p wow-souls ring world_state 2>&1 | grep -E "test result|FAILED"; cargo build -j 4 -p wow-souls 2>&1 | tail -2`
Expected: pass and the game builds.

- [ ] **Step 5: Gates**

As before. No commit.

---

### Task 9: First-kill best drop, and held loot

**Files:**
- Modify: `crates/wow-souls/src/loot/mod.rs` (`corpse_loot`, a release system), `crates/wow-souls/src/world_state.rs` (delivery at start)
- Test: in-file (`world_state.rs` delivery helper)

**Interfaces:**
- Consumes: `wow_world::loot::best_drop`, `species::rank_of`, `WorldStateFile`, `Rank::first_kill_best / never_respawns`.
- Produces: the corpse of a Rare / Boss holds its table's best item the first time; a never-respawn corpse's items are recorded in `held` and released when taken; held items left from an earlier run are given at start (`deliver_held`).

- [ ] **Step 1: Write the failing test** (`world_state.rs` tests; the pure delivery step is a function over a `Progress` and a `WorldState`)

```rust
    #[test]
    fn held_loot_from_an_earlier_run_goes_to_the_pack_and_is_forgotten() {
        use wow_world::inventory::Progress;
        let mut progress = Progress::default();
        let mut state = WorldState::default();
        state.held.insert(80247, vec![(2057, 1), (182, 1)]);
        state.held.insert(9, vec![(159, 3)]);
        let given = deliver_held(&mut progress, &mut state);
        assert_eq!(given.len(), 3);
        assert_eq!(progress.inventory.count(2057), 1);
        assert_eq!(progress.inventory.count(159), 3);
        assert!(state.held.is_empty(), "delivered, so forgotten");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls world_state 2>&1 | grep -E "^error" | head -3`
Expected: `deliver_held` not found.

- [ ] **Step 3: Implement**

`world_state.rs`:

```rust
/// Puts every held item into the pack and forgets the held list. An item that does not fit stays held.
/// Returns what was given.
pub fn deliver_held(
    progress: &mut wow_world::inventory::Progress,
    state: &mut WorldState,
) -> Vec<(u32, u32)> {
    let mut given = Vec::new();
    let guids: Vec<u32> = state.held.keys().copied().collect();
    for guid in guids {
        let items = state.held.get(&guid).cloned().unwrap_or_default();
        let mut next = progress.clone();
        if next.inventory.add(&items).is_ok() {
            *progress = next;
            given.extend(items);
            state.held.remove(&guid);
        }
    }
    given
}

/// At start: spoils a boss still carried when its corpse was lost (a restart) are in the pack now.
fn deliver_at_start(
    mut file: ResMut<WorldStateFile>,
    owned: Option<ResMut<crate::loot::Owned>>,
) {
    let Some(mut owned) = owned else { return };
    if file.state.held.is_empty() {
        return;
    }
    let mut next = owned.progress.clone();
    let given = deliver_held(&mut next, &mut file.state);
    if given.is_empty() {
        return;
    }
    match owned.publish(next) {
        Ok(()) => {
            info!("world state: {} item(s) from an earlier kill are in your pack: {given:?}", given.len());
            file.mark();
        }
        Err(e) => warn!("world state: could not deliver held loot: {e}"),
    }
}
```

and register in `plugin`: `.add_systems(Startup, deliver_at_start.after(crate::loot::load))` (`loot::load` is the system the equip plugin orders after: `.after(crate::loot::load)`; match that path).

`loot/mod.rs` `corpse_loot`: add parameter `mut state: ResMut<crate::world_state::WorldStateFile>`. After the `table` is resolved and `items` rolled (the `let items = match wow_world::loot::roll(..)` block) and before `if items.is_empty() { continue; }`, make `items` mutable and add:

```rust
        let mut items = items;
        let rank = crate::species::rank_of(species.0);
        if rank.first_kill_best() {
            if let Some(entry) = entry.filter(|e| !state.state.first.contains(e)) {
                if let Some(best) = wow_world::loot::best_drop(
                    &data.0.loot,
                    &data.0.items,
                    LootKind::Creature,
                    table,
                ) {
                    match items.iter_mut().find(|(i, _)| *i == best) {
                        Some(slot) => slot.1 = slot.1.max(1),
                        None => items.push((best, 1)),
                    }
                    info!("loot: first kill of entry {entry}: the best of its table is item {best}");
                }
                state.state.first.insert(entry);
                state.mark();
            }
        }
```

(`LootKind` and `entry` are in scope in that function; `entry` is the `Option<u32>` resolved above, then re-bound by `table`'s shadowing: bind `let entry_for_state = entry;` before `entry` is consumed if the compiler complains.)

and after `info!("loot: rolled corpse {table} {:?}", items);` and before `commands.entity(dead.who).insert(Corpse { items, glow });`:

```rust
        if rank.never_respawns() {
            if let Some(guid) = spot
                .zip(book.as_deref())
                .and_then(|(s, b)| b.book.spots.get(s.0))
                .map(|s| s.guid)
            {
                state.state.held.insert(guid, items.clone());
                state.mark();
            }
        }
```

Release when taken: add a system in `loot/mod.rs` registered with the module's other systems (read how `corpse_loot` is registered, near `.add_systems(` for `corpse_loot`, and add this next to it):

```rust
/// A never-respawning creature's corpse was looted (its `Corpse` is gone but the body stays): what it
/// held is the player's, so the world state forgets it.
fn release_held(
    mut gone: RemovedComponents<Corpse>,
    ids: Query<&crate::world::SpawnId>,
    book: Option<Res<crate::world::WorldBook>>,
    mut state: ResMut<crate::world_state::WorldStateFile>,
) {
    let Some(book) = book else { return };
    for e in gone.read() {
        // A despawned body has no SpawnId any more: its loot stays held for the next start.
        let Ok(&crate::world::SpawnId(i)) = ids.get(e) else {
            continue;
        };
        if let Some(spot) = book.book.spots.get(i) {
            if state.state.held.remove(&spot.guid).is_some() {
                state.mark();
            }
        }
    }
}
```

(Registration: `.add_systems(Update, release_held)` beside `corpse_loot`'s. `WorldBook.book` is `pub`; `spots` is `pub` on `Book`.)

- [ ] **Step 4: Run to verify it passes**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls world_state loot 2>&1 | grep -E "test result|FAILED|^error"; cargo build -j 4 -p wow-souls 2>&1 | tail -2`
Expected: pass; the build succeeds. The existing `loot` tests build a minimal `World` (`init_resource::<QuestLootEligibility>` near line ~1095): add `w.insert_resource(crate::world_state::WorldStateFile::in_memory());` there if a test app runs `corpse_loot` and panics on the missing resource.

- [ ] **Step 5: Gates**

As before. No commit.

---

### Task 10: Measure it live, then document

**Files:**
- Modify: `README.md` (switch rows), `docs/HANDOVER.md` (the entry), the spec (the four implementation choices), memory file `wow-souls-gear-and-classes.md` (one line)
- Evidence: log lines in the HANDOVER entry

Unattended runs use scratch saves (Global Constraints). After `cargo build -j 4 -p wow-souls`, from `/c/Spel/wow-souls`:

- [ ] **Step 1: Scratch environment and level 1 health**

```bash
B="C:/Users/jacob/AppData/Local/Temp/claude/C--Users-jacob-Documents-GitHub-wow-darksouls/31841a9f-78ae-41a7-bbe1-8944448122ba/scratchpad"
run() { S="$B/live-$1"; rm -rf "$S"; mkdir -p "$S"; shift
  env WOWSOULS_SETTINGS="$S/settings.txt" WOWSOULS_CHARACTER="$S/character.txt" WOWSOULS_ITEMS_SAVE="$S/items.txt" \
      WOWSOULS_QUESTS_SAVE="$S/quests.txt" WOWSOULS_SOULS_SAVE="$S/souls.txt" WOWSOULS_WORLD_STATE="$S/state.txt" \
      WOWSOULS_MENU=0 WOWSOULS_CHECK=24 WOWSOULS_KOBOLDS=1 WOWSOULS_KOBOLD_AT=-8911,-136 WOWSOULS_SPIN=0 "$@" \
      timeout 150 ./target/debug/wow-souls.exe > "$S/run.log" 2>&1
  echo "exit $?"; sed 's/\x1b\[[0-9;]*m//g' "$S/run.log" | grep -E "$GREP" | cut -c1-210 | head -14; }
GREP="level:|gear:|strikes the player|WOWSOULS_CHECK|panicked"
run hp
```

Expected: `level: 1 -> health 60, stamina 100` and `gear: armor 47 (...)` (7 gear + 2 x 20 Agility = 47), kobold blows of 25-35 against 60 HP.

- [ ] **Step 2: Gear stamina is health**

`run boots WOWSOULS_EQUIP=6095` (Wandering Boots, +4 Stamina +4 Spirit, required level 0). Expected: `level: 1 -> health 100` after the equip (60 + 40), then the gear line.

- [ ] **Step 3: Quest item before the quest, a boss that stays dead, the first-kill drop**

A kobold is made a boss with the debug switch and killed with the scripted attack; its table's best gear must be in the corpse, and a second run on the same state file must find it dead.

```bash
GREP="ring:|loot:|first kill|world state|hit: |dies|WOWSOULS_CHECK"
run boss WOWSOULS_RANK=kobold:boss WOWSOULS_WORLD=1 WOWSOULS_LOOT_SEED=1 WOWSOULS_ATTACK=8.0,0.9
cat "$B/live-boss/state.txt"
```

Expected: `ring: ... died and stays dead (never respawns)`, `loot: first kill of entry 80: the best of its table is item N`, and `state.txt` containing `wsstate 1`, `slain <guid>`, `first 80`, `held <guid> <item> <count> ...`. Then run the same environment again without a scratch wipe (a variant of `run` that does not `rm -rf`): `ring: 1 creature(s) stay dead from an earlier run`, `world state: ... item(s) from an earlier kill are in your pack`, and the kobold of that guid does not appear (`ring: + kobold #... guid <guid>` absent).

- [ ] **Step 4: The quest item before the quest**

Kill any creature whose table has a quest row of an untaken quest (a Defias Thug carries Red Burlap Bandana, quest 18) with the quest not accepted: `loot: rolled corpse 38 [..(752, ..)..]` must now list item 752 (it did not before). Verify with `WOWSOULS_LOOT_SEED=<n>` over a few seeds, then accept the quest in a following run and check the log completes the objective from the carried count (`WOWSOULS_QUESTS_SCRIPT` verbs as in the HANDOVER's quest checks).

- [ ] **Step 5: Full gates and documentation**

```bash
cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check
cargo clippy -j 4 --workspace --all-targets -- -D warnings
DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 --workspace
```

Expected: all exit 0. Then add to `README.md` after the `WOWSOULS_GEAR_EFFECT=0` row:

```
| `WOWSOULS_RANK=<species>:<rank>[,...]` | make a species `normal`, `rare`, `miniboss` or `boss` for this run (a boss or miniboss never respawns and stays dead in `world-state-v1.txt`; a boss or rare drops the best item of its table the first time) |
| `WOWSOULS_WORLD_STATE=<file>` | the never-respawn / first-kill save (`world-state-v1.txt` by default; an unattended run without this switch keeps it in memory only) |
```

Update the spec's four implementation choices (listed at the top of this plan), the HANDOVER entry ("NEXT STEP SPECIFIED" becomes "BUILT" with the measured numbers and the open items: the 30 random-suffix items, the valley's no-rare finding, Int / Spi inert, `HEALTH_OFFSET`), and the memory file. Nothing committed.

---

## Self-review (done when writing)

- **Spec coverage:** attributes + tables import (Tasks 1-2), HP / AP / armor rules and `HEALTH_OFFSET` (2-3), stat display in tooltip / compare / page (4), quest items before the quest + shop refusal (5), `rank` (6), best item (7), world state, never respawn, F6 / death / ring (8), first-kill drop + held loot (9), tests / live / docs (10). Spec items not built: random-suffix items, resistances, crits (listed "Not in this step").
- **Placeholders:** none; Task 5's fixture helpers are named by contract ("use the log's own way ... in this file's tests") because those tests' helpers must be read first, and the plan says to build the `Quest` values directly if no helper exists.
- **Type consistency:** `Item.stats: Vec<(u32, i32)>`, `LevelStat` / `ClassStat`, `Character { base, total, max_health, melee_ap_factor, ranged_ap_factor }`, `GearEffect::with_character`, `character(game, owned, data)`, `Rank` with `never_respawns` / `first_kill_best`, `rank_of`, `NEVER`, `note_gone_for_good`, `WorldState { slain, first, held }`, `WorldStateFile::{load, in_memory, mark}`, `deliver_held`, `best_drop` are used with the same names in every task.
