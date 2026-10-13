# Attributes, item stats, quest items before the quest, and boss / rare rules (design)

2026-10-12. The user: "Gå vidare till nästa steg" (= stats: Str / Stam etc.) "- i samma steg kan du lägga till att
bossar och rares ... ska droppa det bästa som dom har på sin loot table första gången man dödar dem. Bossar och
minibossar ska inte respawna?"; then the quest-item idea ("skulle man inte kunna loota det innan man får
questen"), then "B", then "ren B" with enemy damage left as it is.
Status: **approved by the user 2026-10-12 ("Kör på"); built the same day by `docs/superpowers/plans/2026-10-12-attributes-and-boss-rules.md`** (measured live: see HANDOVER, "ATTRIBUTES, ITEM STATS ... BUILT"). Nothing committed.

Implementation choices that differ from the text below: item stats are a field on `Item` (a trailing list of the `item` record), not in `itemx`; the tables are the manifest kinds `levelstat` / `classstat`; a dead never-respawning creature's corpse is kept by the ring and its unlooted `held` items go to the pack at the next start; `WOWSOULS_RANK=<species>:<rank>` is a debug switch; the Wandering Boots are +4 Stamina and +4 Intellect (the "Spirit" in the examples was a misreading).

## What the user wants

1. **The character has WoW's five attributes** (Strength, Agility, Stamina, Intellect, Spirit): base values per level
   from the database plus what the worn gear adds, with WoW's formulas for health, attack power and armor. "Vill ju
   ha mycket av original WoW kvar." (option **B, pure**: WoW's health numbers, level 1 = 60 HP.)
2. **Enemy damage is not scaled down.** "Är inte det en del av souls spelen att det ska vara farligt? Man blir ju
   starkare." The player outgrows fixed creatures; the creatures do not change.
3. **Quest items drop before the quest is accepted**, so they are already carried when the quest is taken (the
   user's own idea; it removes every "killed the boss too early" soft lock).
4. **Bosses and minibosses never respawn.** **Bosses and rares drop the best item of their loot table the first
   time they are killed.**
5. A rare in Northshire: the database has none (see below); the mechanism is built, none is placed unless the user
   names one.

Assumed by Claude (to be judged in play): every constant in "Tuning knobs", what "best" means, the file name of the
new save, that a Miniboss gets no first-kill guarantee (only "no respawn"), that a Rare gets no "no respawn".

## What exists (read 2026-10-12)

- HP and stamina come from the level only: `level.rs` `vitals(level)` = 100 + 10 per level and 100 + 5 per level.
  There are no attributes anywhere.
- Items: `wow_world::model::Item` has armor, block, damage, delay but **no stats**; the exporter
  (`tools/export-mangos.ps1`) does not read `stat_type1..10` / `stat_value1..10`. In the valley's 331 items, 17 have
  fixed stats (+1 to +4, all green, required level 0-8) and 30 have a random suffix property (rolled per drop:
  out of scope here). Stat type ids in 1.12: 3 Agility, 4 Strength, 5 Intellect, 6 Spirit, 7 Stamina.
- Level tables are in the database: `player_levelstats` (race, class, level, str, agi, sta, inte, spi) and
  `player_classlevelstats` (class, level, basehp, basemana). Human Warrior level 1 / 5 / 8: Str 23 / 28 / 31,
  Agi 20 / 23 / 25, Sta 22 / 26 / 29, Int 20 / 20 / 21, Spi 20 / 22 / 23; base HP 20 / 56 / 83.
- Creatures: **every creature in the valley has `rank` 0** in the database, Garrick included (`rank` is populated
  elsewhere: Hogger 1, Thuros Lightfingers 4), and no valley spawn has a respawn time of 600 s or more. The nearest
  rares are Thuros Lightfingers (level 11, 416 yd from the Abbey) and Mother Fang (level 10, 505 yd), both in
  Elwynn. `SpeciesDef` has `boss: bool` (Garrick only).
- Respawn: `ring.rs` brings a creature back after `max(600 s, 4 x its database time)` while the player is 60+ yd
  away, and **every creature is due again at once when the player respawns or presses F6**.
- Loot: `loot/mod.rs` `corpse_loot` rolls a corpse's table with `QuestLootEligibility` (quest items only for
  accepted, unfinished quests). `quest::log::accept` already counts what is carried
  (`entry.items[i] = inventory(item) + pending`).
- Garrick's table (entry 103): quest item Garrick's Head (-100 %), Pitted Defias Shortsword (q1, ilvl 4, 0.83 %),
  food, water and five pouches (q1, ilvl 5), and reference group 30017 (56 poor-quality q0 pieces, item levels 3-10).

## Part 1: attributes and item stats

**Data.** The exporter gains `stat_type1..10, stat_value1..10` on `item_template`, and two tables:
`player_levelstats` for race 1 (all classes) and `player_classlevelstats` (all classes), levels 1-60. The manifest
becomes `wsworld 3`: item stat pairs are carried in the `itemx` extras row (beside `start_quest` and `bonding`, so
`Item` keeps its shape for the crates and tests that build one by hand), plus `levelstat <class> <level> <str> <agi>
<sta> <int> <spi>` and `classstat <class> <level> <basehp> <basemana>` rows. The reader still takes `wsworld 1` and
`2` (no stats, no tables: attributes then read as the old fixed vitals and the game logs that it did).

**Attributes.** `Attributes { strength, agility, stamina, intellect, spirit }` = the level table's base + the sum of
the worn items' stats of that type. Stats count while an item is worn, whatever the grip; only armor and block
follow `GearEffect`'s stowed-shield rule.

**Rules (WoW 1.12, normalised where noted):**

| What | Rule |
|---|---|
| Max health | `basehp(class, level) + stamina_hp(sta) + HEALTH_OFFSET`, where `stamina_hp(s) = min(s, 20) + 10 x max(s - 20, 0)`; `HEALTH_OFFSET` is 0 (pure WoW). Human Warrior: 60 / 79 / 98 / 117 / 136 at level 1-5. |
| Health on a gear change | current health moves by the same difference as the maximum (as in a level-up), never below 1 while alive |
| Stamina bar | unchanged: 100 + 5 per level (a Dark Souls bar; WoW has none) |
| Attack power (melee) | `level x 3 + Strength x 2 - 20` (Warrior row; other classes get their own row with the class framework) |
| Attack power (ranged) | `level + Agility - 10` (Warrior, as MaNGOS computes it) |
| Damage dealt | `melee_factor = weapon_factor x ap_factor`, `ap_factor = (REF_DPS + AP / 14) / (REF_DPS + AP_REF / 14)`, `AP_REF` = a level 1 Human Warrior's (29 melee, 11 ranged): **the starting kit at level 1 is exactly x1.0 as today**; level 5 about x1.5, level 8 about x1.85 |
| Armor | `+ 2 x Agility` on top of the worn armor (so level 1 is 40 + 7) |
| Block value | unchanged (shield's own) |
| Intellect, Spirit | counted and shown, no effect until mana and regeneration exist |
| Poise, guard, reactions, **enemy damage** | unchanged (Dark Souls) |

**What it does to the game (numbers):**

| Level | HP today | HP pure WoW | Strength | Damage x |
|---|---|---|---|---|
| 1 | 100 | 60 | 23 | 1.00 |
| 2 | 110 | 79 | 24 | 1.11 |
| 3 | 120 | 98 | 25 | 1.23 |
| 4 | 130 | 117 | 26 | 1.34 |
| 5 | 140 | 136 | 28 | 1.50 |
| 8 | 170 | 193 | 31 | 1.85 |

The valley carries a player to level 4-5 (400 / 900 / 1400 / 2100 XP to the next), so **the whole valley is 10-40 %
lower in HP than today, while hits land harder as you level**. A kobold's 25-35 damage is 2-3 blows at level 1
(4 today). This is the user's choice ("farligt"); the one lever if it is too much in play is `HEALTH_OFFSET`
(+40 puts level 1 back at 100).

**Display.** An item tooltip lists `+4 Stamina` rows after armor (WoW's wording); the comparison rows show
`+4 Stamina (+4)` and the pack's Better / Worse verdict weighs them; the equipment page gains an attribute block
(`Strength 28 (+3)`, base with the gear's share, then Health, Armor, Block, Hits).

## Part 2: quest items before the quest

`QuestLootEligibility` becomes "items required by a quest the player may still take or has in the log, that is not
yet handed in": may-take = class and race allow it and no other quest of its exclusive group is done; level and
prerequisite chains are ignored (the item is picked up early, used later). Unchanged: a quest item still drops only
by its own chance row, and accepting counts what is carried. Added: the shop refuses to buy / sell an item such a
quest wants (today only accepted quests refuse), so it cannot be sold by accident. A quest handed in stops its items
dropping.

## Part 3: rank rules (boss, miniboss, rare)

`SpeciesDef` gains `rank: Rank { Normal, Rare, Miniboss, Boss }` (the database cannot say: rank 0 everywhere in the
valley). Garrick is `Boss`; every other species is `Normal`; `boss: bool` is replaced by `rank == Boss`.

| Rank | Respawns | First kill drops the best item |
|---|---|---|
| Normal | yes (today's rule) | no |
| Rare | yes, today's rule (4 x database time: hours) | **yes** |
| Miniboss | **never** | no |
| Boss | **never** | **yes** |

**Persistence.** A new save `world-state-v1.txt` (`wsstate 1`) beside the others: `slain <spawn guid>` for a
creature that stays dead, `first <creature entry>` once its first-kill drop has been given, and
`held <spawn guid> <item>x<count> ...` for a never-respawning kill's rolled loot until it is taken (so a corpse
that despawns at 130 yd, or a restart, does not lose it). A never-respawning creature is not spawned at all while its
`slain` line exists. **A player death and F6 do not revive it** (the existing "everyone due at once" reset skips
it). A save with no such file means nothing is slain.

**Best item.** For the creature's loot table (reference groups expanded, quest-item rows left out) choose by
(1) equippable gear before anything else (weapon, armor, shield), (2) quality, (3) item level, (4) the lower drop
chance. It is added to the corpse's normally rolled loot with 100 % chance, once per creature entry. Garrick's is the
Pitted Defias Shortsword (q1, ilvl 4); the rest of his table is white food and poor gear, so a better first-kill
reward for him would be an authored fixed reward (the user's "own secrets and fixed rewards" rule), a later choice.

**Rares.** None is placed in the valley (the database has none). The rule is data: a species row with
`rank: Rank::Rare` and a spawn is all a future zone needs.

## Not in this step

Random-suffix items ("of the Monkey": 30 valley items; needs per-instance item data in the inventory), resistances,
critical strike, dodge and parry chances from Agility, Spirit regeneration and Intellect mana (no resource yet),
Strength's block bonus, class rows other than Warrior, enchants and buffs that change attributes, the
character-creation preview of attributes, NPC attributes.

## Tests and verification

- Pure (`wow-world`): stat parsing and the stat-type map, the level tables lookup (level 0, 61, unknown class),
  `stamina_hp` at 20 / 21 / 0, `max_health`, `attack_power`, `ap_factor` (the starter at level 1 is exactly 1.0),
  `Attributes::of` with and without gear, the manifest round trip and `wsworld 2` still loading, the best-item
  choice (gear before food, quality, item level, lower chance, reference expansion, quest rows ignored, empty
  table), the quest-item eligibility (accepted, takeable, handed in, wrong class, exclusive group), the world-state
  file round trip.
- Game: vitals follow attributes and gear changes (current moves with max), the corpse of a Boss holds the best
  item once, a slain boss is not spawned, a Rare drops its best item once then rolls normally, the world-state
  file survives a restart.
- Live (unattended, scratch saves, as the gear step): level 1 HP 60 and a kobold's blow count; Wandering Boots
  +40 HP; Tribal Vest tooltip; a Garrick kill gives the Pitted Defias Shortsword and Garrick's Head with the quest
  not yet accepted, then the quest is accepted complete; a restart leaves him dead.
- Gates: scoped fmt `--check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`,
  `cargo test -j 4 --workspace` with `DSR_DIR` / `WOW_DATA` set. The manifest must be re-imported
  (`tools/export-mangos.ps1`, then `wow-souls-world import`); the old file is kept as `northshire.pre-stats.bak`.

## Tuning knobs (Claude's choices, for the user's hands)

| Constant | Value | Why |
|---|---|---|
| `HEALTH_OFFSET` | 0 | pure WoW, the user's choice; +40 restores level 1 = 100 |
| `HP_PER_STAMINA` | 10 (1 per point to 20) | WoW 1.12 |
| `AP_PER_STRENGTH` / `AP_PER_LEVEL` / `AP_BASE` | 2 / 3 / -20 | WoW 1.12 Warrior, from MaNGOS's formula (not checked against the client) |
| ranged AP | level + Agility - 10 | MaNGOS, same caveat |
| `AP_PER_DPS` | 14 | WoW 1.12 |
| `ARMOR_PER_AGILITY` | 2 | WoW 1.12 |
| first-kill tie-breaks | gear, quality, item level, lower chance | "det bästa som dom har" |
