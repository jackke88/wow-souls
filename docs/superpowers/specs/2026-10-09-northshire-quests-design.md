# Northshire as WoW Classic has it: world data, spawns, respawn, quests (design)

Status: **DRAFT for the user's go** (2026-10-09). Nothing here is built. Order the user chose: quests first,
XP after, items after that. Sub-project names G0-G4 continue the roadmap in `docs/HANDOVER.md`.

## Goal

Northshire Valley populated and playable exactly as WoW Classic 1.12 has it: every quest, every NPC (quest givers,
trainers, vendors, guards, peasants), every monster and critter at its real spawn, a long respawn, and a quest
system with Dark Souls look and feel. The monsters keep their Dark Souls moves (E / E2); the quests and NPCs are WoW's.

## The data source (found today)

`C:\Spel\MaNGOS` is a vMaNGOS repack with a MyISAM world database (`mangos`, 145 MB). It holds, for patch 10 (1.12):
`creature` (guid, entry, map, x/y/z, orientation, spawntimesecsmin/max, wander_distance, movement_type),
`creature_template` (name, subname, level, display ids, npc_flags, faction), `quest_template` (everything: texts,
objectives, rewards, chains), `creature_questrelation` / `creature_involvedrelation` (who gives / takes), the
`gameobject*` twins (Milly's Harvest grape crates), `creature_loot_template` (the quest-item drops: Garrick's Head 182,
Red Burlap Bandana 752, Tough Wolf Meat 750), `item_template`, `npc_vendor`, `creature_movement`. The WoW client has none
of it (its `WDB` caches are empty). Recipe, rules and findings: HANDOVER "Quests: data import".

## Approaches considered

1. **Export the database to files, import in Rust, game reads a manifest (recommended).** A script copies the DB
   and exports the needed tables as TSV into `cache/mangos/` (gitignored like `cache/dsr/`); a new importer crate decides
   what is in the valley (area ids from the client's own ADT data) and writes one manifest `cache/world/northshire.txt`
   (`wsworld 1`) that the game loads, as it loads `cache/dsr/manifest.txt` today. No MySQL client in the game or in Rust;
   everything after the export is plain text and unit-testable.
2. Game links a MySQL/SQL client and queries live. Rejected: a server dependency for a single-player game, heavy
   build, and the repack's data must not be touched.
3. Hand-transcribe the quests from classicdb.ch. Rejected: slow, error-prone, no positions.

## Decomposition (each is its own plan and verification)

- **G0 Importer.** `tools/export-mangos.ps1` (copy DB, throwaway `mysqld` on port 3399, `mysql --batch` per query,
  stop it), crate `wow-world` (importer + parsers + manifest writer/reader, shared by the game), bin `wow-souls-world`.
  The valley test: `AreaTable.dbc` parent chain contains area 9 (Northshire Valley) at the spawn's `area_id_at`
  (benilla-formats), not a coordinate box (Elwynn leaks into any box). Output: spawns (entry, guid, position,
  facing, respawn min/max, wander, movement type) split into hostile / critter / friendly NPC, NPC roster
  (display, level, flags, equipment), quest set (see G3), objects, items the quests touch, loot rules for quest items.
  Check: counts per entry printed and compared with the DB; a sample of spawns drawn on the ground screenshot.
- **G1 Spawns replace `slice.rs`'s hand-made groups.** Every hostile spawn from the DB becomes a species instance by
  entry (38 thug, 80 / 6 / 257 kobolds, 69 / 299 wolves, 103 Garrick at his real spot WoW -9056, -461 instead of our
  candidate), with its wander distance and (later) its patrol. Friendly NPCs (the built NPC dressing pipeline: baked atlas,
  helm / shoulder models): McBride, Willem, Eagan, Milly, the trainers, vendors, guards, peasants, standing on their
  spawn, with their idle animation; they are not hostile and not lockable-hostile. Critters (rabbit 721, deer 883,
  fawn 890, cats, cow 2442, horse 5403...) are new `Critter` species: a wander / flee brain, the WoW model,
  die in one blow, no Dark Souls kit (they have none to borrow; WoW clips for walk, run, stand, death). Lazy
  streaming: creatures spawn near the player and sleep far away (hundreds of spawns).
  **Roaming (the user's order): no mob only stands.** Each spawn keeps its own area: movement type 1 wanders at random inside
  its `wander_distance` around the spawn point (pick a point, walk there at walk speed with the walk clip, pause, repeat),
  type 2 walks its `creature_movement` path (waypoints, `waittime` ms at each, back and forth or in a loop as the
  path ends), type 0 stands (NPC posts). Aggro interrupts it, the leash walks the creature home to its area; critters
  flee instead of fighting. The wander clock keeps running only for creatures inside the streaming ring.
- **G2 Respawn.** Per spawn: the creature, once dead, comes back after `max(10 min, 4 x DB respawn)` of game time, only
  when the player is at least 60 yd away and cannot see the spot, so a cave round trip never meets a respawn behind
  the player. The values are tuning knobs (`species` / one table), `F6` still resets everything. Souls and quest
  progress are not undone by a respawn.
- **G3 Quests.**
  - Data: the valley's quests as the DB has them for patch <= 10 (highest `patch` row per entry): 783, 5261, 33, 18,
    6, 7, 15, 21, 54, 3903, 3904, 3905; Brother Paxton's chain 344-347; 1598, 2158, 5623, 5805; the class letters
    3100-3105 (per class, see Open question). Fields kept: title, details / objectives / progress / completion texts,
    level, required class / race, prev / next / exclusive group, source item, required items and creature / object
    counts, reward choice items, fixed items, money, XP (stored, not granted until XP exists), end texts.
  - Runtime: a quest state per quest (unavailable / available / active / complete / turned in), counters (kills by
    entry, items held, objects used), prerequisites and chains, exclusive groups (Brotherhood of Thieves vs Wolves
    Across the Border share group -18), source items (Marshal McBride's Documents), quest items dropped at their
    own chance by the creatures in `creature_loot_template` (-80 % for Red Burlap Bandana, Tough Wolf Meat; -100 %
    for Garrick's Head), objects (Milly's Harvest crates give 11119 when used), turn-in with a reward choice.
  - Interaction and look: press the interact key near an NPC; a DS-style dialogue box (text on the layout's own dark
    plate, the DS font, choices as DS menu lines) with the WoW texts (placeholders `$N`, `$R`, `$C` filled); an exclamation /
    question mark over NPCs with a quest to give / take; a quest log screen in the HUD style (title, objectives with
    counters, rewards); a short toast when an objective changes and when a quest completes (the DS "banner" art
    from `menu.drb`, like YOU DIED). All DS UI art comes from the same cache the HUD already reads.
  - Persistence: a save file for quests, items and kills so the valley survives a restart (small step in G3; DS has
    bonfires, WoW has none: for now saves are automatic).
- **G4 XP and levels** (after G3, its own spec): the user's open question of WoW levels against DS souls.

## Non-goals now

Vendors that sell (needs the item phase), trainers that teach (needs abilities), class quests other than the one class,
the Stormwind-side of Paxton's chain, game events (Noblegarden eggs), pets, NPC patrol paths from `creature_movement`
(a later polish, listed under G1), the Cataclysm Northshire.

## Data-correctness rules

1. Patch filter `patch_min <= 10 AND patch_max >= 10` on creatures; highest `patch <= 10` row per quest / item entry.
2. The valley is decided by area id, never by a coordinate box.
3. Texts are WoW's, verbatim, in English; only the placeholders are filled.
4. Anything the DB lists but the game cannot do yet (spells, money, vendors) is imported and kept in the manifest, and
   the game says plainly it ignores it (one log line), so nothing is silently dropped.

## Testing

Importer: unit tests on the parsers with small TSV fixtures; an install test (skipped without the export) asserting
per-entry counts against the DB (e.g. quest 18 requires 12 x item 752; Garrick at -9056.48, -460.90, 72.83) and that
Eagan Peltskinner and Marshal McBride are inside area 9. Game: pure quest-state tests (chains, exclusive groups, counters,
loot chance with a seeded RNG), a scripted run that takes a quest, kills the required creatures and turns it in
(`WOWSOULS_*` instruments, unattended), pictures of the dialogue box and the log. Gates as everywhere: fmt, clippy
`-D warnings`, tests, installs untouched.

## Open question (the user's)

The player has no class. WoW's start gives one letter by class (3100 Simple Letter for Warriors, 3101 Paladin, 3102
Rogue, 3103 Priest, 3104 Mage, 3105 Warlock; Hunter / Shaman / Druid start elsewhere). Proposal: **Warrior**, since the
player already fights with a sword (the Llane Beshere letter). The other class quests, trainers' dialogue and letters
stay out of scope for now.

## Order of work

G0 -> G1 (hostiles first, then friendly NPCs, then critters) -> G2 -> G3 -> G4. Each ends with the three gates,
a live unattended check, pictures where something is drawn, the HANDOVER updated.
