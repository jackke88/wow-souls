# G4: kill XP, levels with effect, souls lost at death (design)

Approved by the user 2026-10-10 ("Kör på", after the choices "Hybrid" and "Ja, DS-regeln"). Talent trees and abilities
are a later discussion the user asked for after G4; nothing here pre-empts them (the level is the only hook they need).
Nothing committed (the repo has no commits; commit only when the user asks).

## Decision

- **WoW XP is the level currency.** Quest XP (G3, already in) and kill XP (new) add to one total, `QuestGame.xp`; the level
  is `XpTable::level_for(xp)` as today. The quest gates (`MinLevel`) keep working unchanged.
- **Souls stay the Dark Souls currency.** A kill gives its `getSoul` souls as today; **dying loses them to a bloodstain**
  at the death spot (G4b). Souls are spent in the item phase (vendors, upgrades): no sink in G4.
- **A level has an effect:** max HP and max stamina grow (G4a).

## G4a: kill XP and the level's effect

### Kill XP (pure, `wow_world::quest::xp`)

The vMaNGOS formula (`MaNGOS::XP`, read from `src/game/Formulas.h` of github.com/vmangos/core, 2026-10-10):

```
base   = 5 * player_level + 45
factor = mob_level >= player_level : 1 + 0.05 * min(mob_level - player_level, 4)
         mob_level  < player_level : 0 if mob_level <= gray(player_level)
                                     else (ZD + mob_level - player_level) / ZD
xp     = round_half_even(base * factor * (2 if elite else 1))
gray(l)= 0 for l <= 5 ; l - 5 - l/10 for 6..=39 ; l - 1 - l/5 for l >= 40   (integer division)
ZD(l)  = 5 (1-7), 6 (8-9), 7 (10-11), 8 (12-15), 9 (16-19), 11 (20-29), 12 (30-39), 13 (40-44), 14 (45-49),
         15 (50-54), 16 (55-59), 17 (60+)
```

- A creature of type critter (`creature_template.type` 8: rabbit, deer, fawn) gives no XP (the vMaNGOS `Gain` returns 0
  for it), as it gives no souls today.
- Elite = `rank` 1-3 (elite, rare elite, world boss). The valley has none: Garrick is rank 0 (level 5, the database), so he
  pays like any level-5 creature. The ×2 is implemented and tested, unused here.
- The server's own rates (`xp_multiplier`, rate configs) are not modelled: 1.0.
- A spawn's level is `level_min + guid % (level_max - level_min + 1)` (deterministic, so seeded fights repeat); a creature
  with no spawn record (the old hand-placed groups) uses `level_min` of its entry.
- Level-1 ground truth for the numbers: 50 XP for an equal-level kill at level 1, 400 XP to level 2, so about eight
  kills; a Level-3 Defias Thug at level 1 pays 50 * 1.10 = 55.

### The level's effect (`crates/wow-souls/src/level.rs`)

- `vitals(level) -> (health, stamina)`: HP `100 + 10 * (level - 1)`, stamina `100 + 5 * (level - 1)` (level 5: 140 / 120).
  Constants next to the function, tuned live. Enemy damage is not scaled.
- A system follows `QuestGame.player.level` and applies the vitals to the avatar's `Health` / `Stamina`: on the first
  frame the avatar exists (a save at level 3 starts with level 3's maxima, current = max); on a later rise the maxima
  grow and **the current value grows by the same difference** (no full heal). The respawn already restores `max`.
- The HUD bars take their length from the maxima (3 layout px per point), so a level makes the bars longer on its own.
- Feedback: the toast `N experience gained` for a kill, `You have reached level N` on a rise (the toast exists for
  quests); a rise also plays WoW's own level-up sound when the file is in the install (else silent, one log line).

### Instruments

`WOWSOULS_QUESTS_SCRIPT` gains `xp:<n>` (add XP as a quest reward would) next to `kill:` and `give:`; a real fight proves
the kill path (a log line `quests: kill xp +N (creature entry, level L, you M)`).

## G4b: souls are lost at death (BUILT 2026-10-10, see HANDOVER; the look is a dark disc plus WoW's loot sparkles)

- On death the player's carried souls move to a **bloodstain** at the death position (ground-snapped) and the carried count
  goes to 0. Walking into it (1.5 yd) returns the souls with a toast.
- Dying again before recovering: the new stain replaces the old one, but only if the player carries at least 1 soul; with 0
  the old stain stays (the Dark Souls rule). XP, level, quests, items are untouched by death.
- The look: a WoW particle emitter (the loot module's `LootFX` is the candidate); the plan chooses after a contact look.
- Persistence: `%LOCALAPPDATA%\WoWSouls\souls-v1.txt` (`wssouls 1`: carried, optional stain `x y z souls`), written first,
  like the quest and item saves; unattended runs get their own isolated file under `cache/loot-check/`. The quest save
  format stays `wsquests 1`.
- F6 (dev reset of creatures) leaves souls and the stain alone.

## Non-goals

Souls as a level currency; a bonfire; spending souls (item phase); scaling enemy damage or HP with the player's level;
group XP, rested XP, kill-streak reductions; WoW stats (strength, agility...) and talents (the user's next discussion).
