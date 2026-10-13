# G3a The Quest Engine (pure, in `wow-world`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** WoW Classic 1.12's quest rules for Northshire as a pure, fully tested library: which quests a player can see and take, what accepting, killing, looting, completing and turning in does, the marks over NPCs, the quest texts with their placeholders filled, the XP table and levels, and a plain-text save format. No Bevy, no UI: G3b (interaction and UI) and G3c (hooks into kills, loot, objects, the inventory and the HUD) use it.

**Architecture:** New modules inside the library crate `crates/wow-world` (it already holds the data model and compiles independently of the game): `quest/book.rs` (an index over `World`: quests by id, the prerequisite graph built the way vMaNGOS's loader builds it), `quest/rules.rs` (`can_take`, marks), `quest/log.rs` (`QuestLog` and its transitions), `quest/text.rs` (placeholder expansion), `quest/xp.rs` (XP table, levels), `quest/save.rs` (text save / load). The spec for every rule is the research report `docs/superpowers/specs/2026-10-09-quest-semantics.md` (read ALL of it first, especially sections 1, 1.12, 2.3, 3, 5, 6): where this plan and the report differ, the report's vMaNGOS-derived rule wins and the deviation goes in your report.

**Tech Stack:** Rust 2021, `anyhow`, no new dependencies. Data: the manifest `cache/world/northshire.txt` (`wow_world::manifest::parse`) and the TSV exports in `cache/mangos/tsv/`.

Decisions already taken (the user's "kör på" and the planner, 2026-10-09; do not ask again): the player is a Human **Warrior** (class letter 3100 only; the other class letters, trainer quests of other classes and the orphans 5805 / 7961 / 7962 are data only and never offered); quest 6 waits for 33 too (vMaNGOS's own rule, replicate as the data says); the Stormwind chain 344-347 is data only; quests 54 and 2158 can be accepted but are parked in the log (their enders stand in Goldshire, outside the valley); XP from quest rewards is real and the level gate (`MinLevel`) is enforced, because five quests need level 2 (WoW's level 2 is 400 XP, the level-1 quests give 1205); kill XP, health scaling and souls are G4. The player's name defaults to `Adventurer`, male.

Rules that bind every task: tests first (the report gives test vectors: use them); `cargo fmt --all -- --check`, `cargo clippy -j 4 -p wow-world --all-targets -- -D warnings`, `WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 -p wow-world`; build with `-j 4` only; no commits; edit only `crates/wow-world/**` and `tools/export-mangos.ps1`; never write into the WoW, Dark Souls or MaNGOS installs or benilla; do not start MySQL except through `tools\mangos-db.ps1 start` for Task 1 (and never pipe it into `Out-Null` / `Out-String`; stop it when done: `tools\mangos-db.ps1 stop`). Do not edit `docs/HANDOVER.md`.

## File structure

| File | Responsibility |
|---|---|
| `tools/export-mangos.ps1` | the extra columns and tables (Task 1) |
| `crates/wow-world/src/model.rs`, `db.rs`, `select.rs`, `manifest.rs` | the extra fields (Task 1) |
| `crates/wow-world/src/quest/mod.rs` | `pub mod` list, shared small types (`Class`, `Race`, `Player`, `Status`) |
| `crates/wow-world/src/quest/book.rs` | `QuestBook::new(&World)` |
| `crates/wow-world/src/quest/rules.rs` | `can_take`, `reasons`, `marks` |
| `crates/wow-world/src/quest/log.rs` | `QuestLog`, `Event`, accept / kill / items / turn in / abandon |
| `crates/wow-world/src/quest/text.rs` | `expand` |
| `crates/wow-world/src/quest/xp.rs` | `XpTable`, `level_for`, `xp_to_next` |
| `crates/wow-world/src/quest/save.rs` | `QuestLog::to_text` / `from_text` |

---

### Task 1: Export and model the columns the rules use

- [ ] **Step 1: Re-export.** `tools/export-mangos.ps1` (the `-Only` parameter exists): add to `quest_template` the columns `MaxLevel, RequiredCondition, RepObjectiveFaction, RepObjectiveValue, RequiredMinRepFaction, RequiredMinRepValue, RequiredMaxRepFaction, RequiredMaxRepValue, RewRepFaction1..5, RewRepValue1..5, RewMailTemplateId, RewMailDelaySecs, RewMailMoney` (names exactly as the database has them: check with `tools\mangos-db.ps1 query "show columns from mangos.quest_template"`); to `item_template` the columns `start_quest, bonding, page_text`; to `gameobject_template` `data6..data23, script_name` is NOT needed, add only `data6, data7, data8, data9` (chest and goober fields); new tables `quest_start_scripts` and `quest_end_scripts` (all columns, all rows) and `player_xp_for_level` (all columns, all rows; find its real name with `show tables like '%xp%'`). Start the server with `powershell -ExecutionPolicy Bypass -File tools\mangos-db.ps1 start` (not piped), run `powershell -ExecutionPolicy Bypass -File tools\export-mangos.ps1 -Only quest_template,item_template,gameobject_template,quest_start_scripts,quest_end_scripts,player_xp_for_level`, report the line counts, then stop the server.
- [ ] **Step 2: Model.** Add to `model.rs`: `Quest { max_level: u32, required_condition: u32, rep_objective: (u32, i32), rep_min: (u32, i32), rep_max: (u32, i32), reward_rep: Vec<(u32, i32)>, reward_mail: (u32, u32, u32) }`, `Item { start_quest: u32, bonding: u32 }`, `World.scripts: Vec<QuestScript>` with `QuestScript { quest: u32, start: bool, delay: u32, command: u32, data: [i64; 4], text: String }` (read the table's real columns: keep the numeric ones the table has, name them from the table; the game will ignore scripts and log one line saying so), and `World.xp_for_level: Vec<u32>` (index = level - 1: XP needed to go from that level to the next; use the table's real column). Loader functions in `db.rs` with fixture tests, selection in `select.rs` (scripts of selected quests only; the whole XP table), manifest records `qscript` and `xp` with the round-trip tests extended (the manifest stays `wsworld 1` only if the layout of the existing records is unchanged: new fields append to the END of a record; bump to `wsworld 2` and update the parser to accept both if any existing record's field order changes). Re-run `./target/debug/wow-souls-world.exe import` after `cargo build -j 4 -p wow-world` and report the counts.

### Task 2: `quest/mod.rs`, `book.rs`

- [ ] **Step 1: Types** in `quest/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class { Warrior = 1, Paladin = 2, Hunter = 3, Rogue = 4, Priest = 5, Shaman = 7, Mage = 8, Warlock = 9, Druid = 11 }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Race { Human = 1, Orc = 2, Dwarf = 3, NightElf = 4, Undead = 5, Tauren = 6, Gnome = 7, Troll = 8 }
impl Class { pub fn mask(self) -> u32 { 1 << (self as u32 - 1) } }
impl Race { pub fn mask(self) -> u32 { 1 << (self as u32 - 1) } }
#[derive(Debug, Clone, PartialEq)]
pub struct Player { pub name: String, pub male: bool, pub class: Class, pub race: Race, pub level: u32 }
impl Player { pub fn warrior(name: &str, level: u32) -> Player { /* Human male Warrior */ } }
```

- [ ] **Step 2: `QuestBook::new(&World) -> QuestBook`** indexes the quests by id and builds the prerequisite lists exactly as the report's section 1.1 says vMaNGOS's `LoadQuests` does (a quest's `PrevQuestId`; and the quests whose `NextQuestId` names it, with the exclusive-group handling of section 1.5), `by_giver(npc, object)`, `by_ender(npc, object)` from `World.starts` / `World.ends`, `quest(id)`, `playable(id) -> bool` (false for the quests the decisions above exclude: other classes' letters, 5623, 1598, the Stormwind chain 344-347 (the quests whose giver is outside the valley and level >= 20), the orphans 5805, 7961, 7962). Tests: the prerequisite lists of 3903 (needs 18 and 33), 6, 21, 7, 15 against the report's table 1.11; `by_giver(197)` gives 7, 15, 21, 54, 3100-3105; `playable` is true for exactly the 14 quests the report lists plus none of the others (14 minus the two parked ones is still "playable": 54 and 2158 are playable, just not finishable).

### Task 3: `rules.rs`

- [ ] **Step 1: Tests from the report's section 1.12** (all of them, as written): at level 1 with rewarded {783} the takeable set is {7, 5261, 2158}, 3100 not yet (its prerequisite is 7), at level 2 {7, 18, 5261, 2158}; rewarded {783, 7} adds {15, 3100}; 15 current (not rewarded) -> 21 not takeable; rewarded {18} -> 3903 and 6 not takeable; rewarded {18, 33} -> both takeable; 18 or 33 in the log -> 3903 not takeable; a quest already in the log or rewarded is not takeable; a Warrior never sees 3101-3105, 5623, 1598; wrong class / race bit masks; `max_level` 0 means no upper bound. Plus the full "waves" simulation of section 1.12 for level 1 and level 2 (take everything takeable whose giver is spawned in the valley, then turn in everything whose ender is spawned in the valley and whose objectives are met): the waves equal the report's.
- [ ] **Step 2: Implement**

```rust
pub fn can_take(book: &QuestBook, log: &QuestLog, p: &Player, quest: u32) -> bool
pub fn why_not(book: &QuestBook, log: &QuestLog, p: &Player, quest: u32) -> Option<Refusal>   // None = can take
pub enum Refusal { Unknown, NotPlayable, AlreadyHave, AlreadyDone, Level { need: u32 }, TooHigh, Class, Race, Prerequisite, Chain, Exclusive, Skill, Reputation, Condition }
/// What shows over an NPC for the player, the 1.12 marks: the best of its quests.
pub fn marks(book: &QuestBook, log: &QuestLog, p: &Player, npc: Npc) -> Mark
pub enum Npc { Creature(u32), Object(u32) }
pub enum Mark { None, Available, AvailableLow /* grey: quest level 4+ below the player */, InProgress /* ? grey */, Ready /* ? yellow */ }
```

`marks`: a quest this NPC ends and the log has COMPLETE gives `Ready`; one it ends and the log has incomplete gives `InProgress`; a quest it gives and the player can take gives `Available` (or `AvailableLow` when `player.level >= quest.level + 4`, vMaNGOS default `Quests.LowLevelHideDiff` 4; high-level hide diff 7 means a quest 7 levels above the player is hidden); the order is Ready > Available > AvailableLow > InProgress > None. A parked quest (its ender outside the valley) shows nothing at any valley NPC.
- [ ] **Step 3: Run, gates.**

### Task 4: `log.rs`, the state of a player's quests

- [ ] **Step 1: Types and API** (signatures are the contract; implement them with tests):

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Status { Incomplete, Complete, Rewarded }
#[derive(Debug, Clone, PartialEq)]
pub struct Entry { pub status: Status, pub kills: [u32; 4], pub items: [u32; 4] }
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuestLog { /* BTreeMap<u32, Entry> */ }
/// What changed, for the UI's toasts and the HUD tracker.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Accepted { quest: u32, source_item: Option<(u32, u32)> },   // the source item is given by the caller to the inventory
    Progress { quest: u32, slot: usize, now: u32, need: u32, what: Need },
    ReadyToTurnIn { quest: u32 },
    TurnedIn { quest: u32, rewards: Rewards },
    Abandoned { quest: u32 },
}
pub enum Need { Kill { entry: i32 }, Item { item: u32 } }
pub struct Rewards { pub items: Vec<(u32, u32)>, pub money: i32, pub xp: u32, pub next_in_chain: Option<u32> }

impl QuestLog {
    pub fn status(&self, quest: u32) -> Option<&Status>;
    pub fn entry(&self, quest: u32) -> Option<&Entry>;
    pub fn active(&self) -> Vec<u32>;                       // Incomplete or Complete, in id order
    pub fn is_rewarded(&self, quest: u32) -> bool;
    pub fn accept(&mut self, book: &QuestBook, p: &Player, quest: u32, inventory: &dyn Fn(u32) -> u32) -> Result<Vec<Event>, Refusal>;
    /// A creature of `entry` died by the player's hand. Credit by exact entry.
    pub fn kill(&mut self, book: &QuestBook, entry: u32) -> Vec<Event>;
    /// An object of `entry` was used (a kill-credit object, `req_kills` below zero).
    pub fn use_object(&mut self, book: &QuestBook, entry: u32) -> Vec<Event>;
    /// The inventory's count of `item` changed to `count`: re-evaluates every active quest that needs it.
    pub fn item_count(&mut self, book: &QuestBook, item: u32, count: u32) -> Vec<Event>;
    /// Should a kill drop this quest item for this player: an active quest still needs it (not complete, counter below the requirement).
    pub fn wants_item(&self, book: &QuestBook, item: u32, held: u32) -> bool;
    /// Turn in at `npc` (the ender), choosing reward `choice` (an index into the quest's reward choices, required when it has any).
    pub fn turn_in(&mut self, book: &QuestBook, p: &Player, quest: u32, npc: Npc, choice: Option<usize>, inventory: &dyn Fn(u32) -> u32) -> Result<Vec<Event>, TurnInError>;
    pub fn abandon(&mut self, quest: u32) -> Vec<Event>;
}
pub enum TurnInError { NotInLog, NotComplete, WrongNpc, OutsideTheValley /* the ender has no spawn: parked */, ChoiceRequired, BadChoice }
```

Rules to implement and test (each from the report; cite the section in the test name or comment): accept checks `can_take`; the source item is reported in the event (the caller gives it) and counts toward the quest's required items (quests 54, 346, 3100-3105, 3905 complete the moment they are accepted: `accept` re-checks completion after crediting `inventory`); a quest with no objectives completes at accept; kill credit is by exact entry and capped at the requirement; item objectives are the inventory's count (capped for display), so `item_count` can also move a quest back from Complete to Incomplete when the item is lost; `wants_item` (section 5.2); `turn_in` removes nothing itself (the caller removes the required items and pays) but returns exactly what to give: the fixed items, the chosen item, money (`reward_money`; a negative `RewOrReqMoney` means the player pays: return it negative and refuse with an error variant if you add one), XP, and `next_in_chain` (the quest the same NPC offers at once, `NextQuestInChain` of section 1.6); a parked quest (its ender has no spawn: `book.ender_spawned(quest) == false`) refuses with `OutsideTheValley` and stays in the log; quests are never repeatable here (flags are data only).
- [ ] **Step 2: The play-through test:** drive a Human Warrior through the report's section 6.2 order with the real manifest (skipped without `cache/world/northshire.txt`): 783 -> 7 (kill 10 x entry 6) -> 3100 -> 5261 -> 33 (items 750 x 8 through `item_count`) -> 15 (10 x 257) -> 21 (12 x 80) -> 54, granting each turn-in's XP through `XpTable`, taking 18 / 6 / 3903 / 3904 / 3905 only once level 2 is reached (assert they were refused before and allowed after), ending with 54 and 2158 parked and every other playable quest rewarded. Assert the XP total and the rewards of each quest against the manifest.
- [ ] **Step 3: Gates.**

### Task 5: `text.rs`, `xp.rs`, `save.rs`

- [ ] **Step 1: `text::expand(text: &str, p: &Player) -> String`**: the placeholders in the report's section 3 (`$B $b` line breaks, `$N $n` the name, `$c $C` the class name, `$g male:female;` and the three `$g...;` switches that occur, and the literal line feed as is); an unknown `$x` is left untouched and counted in a returned `Vec<String>` through a second function `expand_checked` so the install test can assert that no quest text of the 28 contains an unknown placeholder. Tests: the report's examples, a name with a `$` in it, `$g` with a missing `;`.
- [ ] **Step 2: `xp::XpTable { for_level: Vec<u32> }`** built from `World.xp_for_level`: `level_for(total_xp) -> u32` (cap at the table's end, level 60), `xp_into_level(total_xp) -> (u32 level, u32 into, u32 need)`; tests: 0 XP is level 1, 400 is level 2 (check the real table's first value and use it), the sum of the level-1 quests' XP (1205) reaches level 2 and stops below level 3; a table of one row.
- [ ] **Step 3: `QuestLog::to_text(&self) -> String` and `from_text(&str) -> anyhow::Result<QuestLog>`**: one line per quest `quest <id> <status> <kills x4> <items x4>`, a header `wsquests 1`, plus `xp <total>`, `money <copper>`, `name <text>` lines kept by a small wrapper type `SaveGame { log, xp, money, name, items: Vec<(u32, u32)> }` with `to_text` / `from_text`; round-trip tests and a corrupt-file error test.
- [ ] **Step 4: Gates (whole workspace is untouched but run clippy for it once), counts, report.**

## Self-review against the spec

- G3 "state per quest (unavailable / available / active / complete / turned in), counters, prerequisites and chains, exclusive groups, source items, quest items at their own chance, objects, turn-in with a reward choice, XP stored (here: paid, for the level gate)": Tasks 2-5. "persistence": `save.rs`. "A DS-style dialogue and log, the marks, the interaction": G3b; "kills, loot, objects and the inventory feeding the engine": G3c.
- The engine returns events and never touches the world; every effect (items, money, XP, toasts) is the caller's job: this keeps G3b and G3c thin.
- Names used across tasks: `Class`, `Race`, `Player`, `QuestBook`, `QuestLog`, `Status`, `Entry`, `Event`, `Need`, `Rewards`, `Refusal`, `TurnInError`, `Npc`, `Mark`, `can_take`, `why_not`, `marks`, `expand`, `expand_checked`, `XpTable`, `level_for`, `SaveGame` are defined above.
