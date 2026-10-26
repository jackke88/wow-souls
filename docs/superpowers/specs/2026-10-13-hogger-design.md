# Hogger, the Elwynn Forest zone boss: design

Date: 2026-10-13. Status: written for the user's review; nothing built. Decided by the user in chat
2026-10-13: Hogger stands **at his real place** (Riverpark), and we make **our own test character,
level 10-12**, so the fight can really be played ("a hard boss you need about level 10-12 and gear
for; it is skill based, so someone lower could do it earlier").

## 1. What the user wants

A real boss, as hard as Dark Souls bosses are: the Elwynn Forest zone boss, WoW's Hogger (level 11
elite), built the way the game builds its enemies (a Dark Souls kit retargeted onto the WoW model),
with his own look, weapon and abilities, at his real position, plus a level 10-12 character with fitting
gear to fight him with.

Not in this spec: the Riverpaw camp around him (Riverpaw gnolls, mystics), the Stormwind quest
"Wanted: Hogger", other Elwynn zones, new player classes.

## 2. Source facts (vMaNGOS 1.12 DB copy, read 2026-10-13; client DBCs)

| What | Value |
|---|---|
| Entry / name | 448 Hogger, level 11, rank 1 (elite), type humanoid, faction 20 |
| Health, damage | `health_multiplier` 3, `damage_multiplier` 1.7, base attack 2000 ms |
| Speed | walk 1.2, run 1.14286 |
| Model | display 384 = `Creature\GnollMelee\GnollMelee.m2` (model id 123), skin `RiverPawGnollFighterSkin`, scale 1.15 (a Riverpaw Gnoll Fighter, display 175, is 1.0) |
| Collision | 0.83 wide, 1.56 high (before the 1.15) |
| Weapon | `creature_equip_template` 448 = item 1905 "Monster - Axe, Metal Basic" (one-handed axe, display 7428, 2000 ms) |
| Spells | Rushing Charge 6268 (cast once, on aggro: AI script 44802), Head Butt 6730 (first 1-3 s, then every 20-29 s), Pierce Armor 6016 (first 4-6 s, then every 46-48 s) |
| Speech | a "say text" on aggro (script 44801) |
| Loot | Huge Gnoll Claw (quest item, 100 %), then junk (Linen Cloth 23 %, Tough Jerky 6 %, greys and greens at under 3 %); no unique drops |
| Spawn | five rows (guid 80531 on a path, 80731 / 81027 / 81028 / 81029 wandering 15 yd), all within about 160 yd of WoW (-9946, 604, 38); respawn 180 s. About 1.2 km from our Northshire anchor (-8913, -136, 82) |

The gnoll model has only: Stand, Walk (2.5 yd/s), Run (8.33), Attack Unarmed / 1H / 2H, wounds, Stun,
Death. No ShuffleL / ShuffleR / WalkBackwards: footwork uses the kobold's solution (tangent walking,
reversed walk). Attachment 1 (right hand) exists for the axe.

## 3. Decision: base the fight on a Dark Souls boss, not from scratch

Candidates measured with `wow-souls-dsr survey` (NpcParam + skeleton + animation table). **Names are
read from the game's own `NPC_name_.fmg`** (`menu`/`item.msgbnd`; the name id equals the chr number),
through the new `names.rs` of the extractor, not guessed:

| chr | The game's name; what it is in the data | Fit |
|---|---|---|
| **c2250** | **Taurus Demon**; 86 bones (a biped with a tail, `L_weapon` on the left hand), hp 1215, poise 120, 8 attacks `a00_3000`-`3007` (4.0-6.2 s) plus three big ones (`3008`, `3009`, `3013`) and a fast 8.5 m leap (`3007`, 2.27 s) | **Chosen**: an axe brute, the first real boss of the Undead Burg (a zone boss), the leap is Hogger's Rushing Charge |
| c2360 | Executioner Smough; two-handed hammer, 10 attacks, an 18 m leap | Fallback (a human executioner, more arena than gnoll) |
| c2232 | Asylum Demon; the 142-bone winged and tailed skeleton of c2230 / c2231 | A different body plan; a harder retarget |
| c2240 | Capra Demon; two weapons, quick side hops | Wrong weapon and tempo |
| c2300, c2860, c2870 | ordinary enemies with a boss-like build (not in the boss name list) | Rejected: not bosses |
| from scratch | the gnoll has no attack / charge / telegraph clips | Would borrow DS clips anyway, with more work |

**H0 is done** (2026-10-13): the first guess, `c2860`, was wrong (an ordinary enemy); the name table
gives the Taurus Demon as `c2250`. The weapon is in the **left** hand (`L_weapon`, parent `L Hand`): the
gnoll's axe goes on its left-hand attachment (id 2; the right hand is id 1), so Hogger is left-handed in
the game; mirroring a retarget is not supported and would be a pipeline of its own.

## 4. The boss

### 4.1 Kit (extractor, `crates/dsr-cache`)

- `wow::GNOLL_M2 = "Creature\\GnollMelee\\GnollMelee.m2"`; `bonemap::GNOLL` pair table and `GNOLL_RIG`
  (hips, legs, soles, ground contact height) in the style of `KOBOLD`: the DS bone names are the ghoul's
  kind with spaces (`L Thigh`, `Spine1` ...), the WoW indices come from `benilla-extract m2bones` (60 bones).
  The gnoll is a hunched biped like the kobold, so the kobold map is the template.
- `monster::HOGGER: Kit` with `chr` `c2250`, `npc` 225 000, `variation` 22 500, `Attack`s (names, clips,
  TAE judge ids), `React`s (`a00_2000` is a 2.9 s one; the wounds are read from the TAE data), `deaths`
  (`a00_2200`, `a00_2521` candidates), `Lying` (biped with a tail: the quadruped limits are not needed).
- A new `Tier::Boss` (more than five attacks, flinch levels up to 4, repeating hit windows allowed) with the
  same `check_attack` discipline; the rule R1 (an ordinary creature borrows 1-2 attacks) stays untouched
  for everything else.
- Every retarget gate stays: direction drift, cross drift, foot gap on contact frames, loop and death
  end-pose checks.

### 4.2 Species and pattern (game, `crates/wow-souls`)

- `species::HOGGER`: name `hogger`, kit `hogger`, title "Hogger", level 11, display 384 (look from the
  model, scale 1.15), weapon item display 7428 (attachment 1), `Rank::Boss` (the Dark Souls boss bar, no
  respawn, drops once: the rules Garrick has), `chase_speed` ~3.4 (slower than the player), `Temper::HOGGER`
  (duellist: a large ring, long sizing-up, rare step back).
- `pattern::HOGGER` table: about 7 moves of the kit with bands, weights and follow-ups, hit windows from
  the TAE data like the existing kits.
- Fight numbers (first guesses, tuned live): health ~900 (30-40 light blows of the trial character),
  poise ~40 (a light blow takes 8, a heavy one 20), damage from the kit's `atkPhys x 25/138` scaled to
  3-4 blows on a level 11 player (200 HP) before armour.

### 4.3 Hogger's WoW identity, as Souls mechanics

| WoW | Here |
|---|---|
| Rushing Charge (on aggro) | the kit's leap / charge: his opening move when the player is far; also used from range later in the fight (cooldown); knocks back, high flinch level |
| Head Butt | a short, fast close-range move that breaks a raised guard (guard break) and staggers |
| Pierce Armor | his axe blows may apply an armour reduction on the player for 30 s (stacks to 3), through the existing `gear_effect` armour path; shown on the HUD as a debuff line |
| Say text on aggro | one voice line / log line as he notices the player (the existing bark path) |
| Enrage | phase 2 at 50 % health: a roar (a stagger-immune clip), then +15 % speed, 20 % shorter recoveries, one more chain in his pattern, charge on a shorter cooldown |

Phases are new game code (`Phase` on the boss, read by the pattern and the clip speed), small and
tested in the pure layer like `pattern.rs`.

### 4.4 Presentation

Boss bar (exists), no adds in v1, his name shown. He keeps his place on the hill; the leash is the usual
30 yd plus "return to full health" (a Souls boss does not give up its health at a leash: it resets).

## 5. In the world

- **Placement:** a named out-of-valley spawn list in the world manifest (`wsworld` section `boss`):
  Hogger at WoW (-9947.88, 594.77, 39.61) (guid 80731, a wanderer of 15 yd; the other four rows are not
  used), imported from the DB by `wow-souls-world import` for a short hand list (`[448]`), not by the valley
  area filter. `world.rs` maps entry 448 to `Kind::Foe("hogger")` like Garrick (103).
- **Streaming:** the benilla engine streams terrain around the viewer, so walking there works; to test
  directly, `WOWSOULS_AT=-9930,590` puts the player at his hill. The ring spawner (100 yd in, 130 out)
  spawns him when the player arrives; collision of the hill must be there before he stands on it (the
  ring waits for ground within 4 yd of the DB height, as it does for every spawn).
- **Respawn / loot:** `Rank::Boss` rules; Huge Gnoll Claw as the first-kill drop, a few of the junk and
  green rows, souls (a boss gives a lot: the value is a tuning number).
- **Nothing else outside the valley is spawned** (the rest of Elwynn is the next roadmap step).

## 6. The test character, level 10-12

- A character slot `characters/<n>/` (the slot system built 2026-10-13) holding: the usual
  `character-v1` (human male warrior, "Hogger trial" as its name), `quests-v1` with the XP of level 11
  (35,200: the table is 27,600 for 10, 35,200 for 11, 44,000 for 12) and the valley quests rewarded,
  `items-v1` with the three abilities learned (Charge, Battle Shout, Thunder Clap), a fitting kit and a
  pack of healing salves, `souls-v1` with some souls, `world-state` placing the player on the road
  to Riverpark.
- **Gear:** chosen from the DB (`item_template`, `RequiredLevel` 8-12, quality green or better, the
  armour classes and weapon families the game already supports: sword, axe, mace, shield, a two-hander,
  plus the helmet / chest / legs / boots / gloves / bracers slots); the exact list is a plan task, taken
  from Elwynn / Westfall quest rewards and vendor stock so it is what a real level 11 would carry.
- **How it is made:** a small command (`wow-souls-world trial-character --level 11 --slot <n>`, or a
  one-shot game switch) that writes the slot with the existing save writers, so the formats cannot drift;
  never overwrites a slot that has a character.
- **Also scalable:** the same command with `--level 6` etc. for a skill-based run with less (the user: "someone
  lower could do it earlier").

## 7. How hard is "hard", and how it is checked

- Targets: a passive level 11 player dies in under 15 s; a player who only blocks loses stamina and dies;
  a player who rolls through his tells and punishes recoveries wins in about 3-5 minutes with 6-10 openings
  per phase; one mistake is a quarter of the health bar.
- Scripted gates (unattended, existing instruments `WOWSOULS_DODGE`, `_ATTACK`, `_ROLL`, `_PARRY`): the passive
  player's time to death; a scripted rolling player survives the first phase's chains; every move's
  damage window is inside its clip; the phase change fires once. The feel is the user's to judge (as with
  every earlier boss, Garrick included).

## 8. Order of work (the plan will cut each into tasks)

1. **H0** (done) identify the boss: `c2250` Taurus Demon, by the game's name table; then list its moves with hit windows (the `survey` output for `a00_3000`-`3013`).
2. **H1** extractor: gnoll model + bone map + rig + `Tier::Boss` + `HOGGER` kit; run the retarget gates;
   contact sheets of every move, flinch and death on the gnoll.
3. **H2** game: `species::HOGGER`, `pattern::HOGGER`, the kit in the roster, a live fight with the default
   character (a placed test spawn near the anchor, `WOWSOULS_HOGGER_AT`), the five new fight numbers.
4. **H3** abilities and phases: Rushing Charge / Head Butt / Pierce Armor as game effects, the phase 2 rule
   and roar.
5. **H4** the real place: boss list in the importer and manifest, ring spawn at Riverpark, loot, respawn
   rule, streaming checked on the hill.
6. **H5** the trial character command, the gear list, the first full fight on the real hill; tuning.

## 9. Risks and open points

- The Taurus Demon is about 5 m tall in DS metres and the gnoll about 1.8 yd: the retarget scales by leg
  length (as for the ghoul / kobold), so an 8.5 m leap becomes about 2-3 yd; the leap needs its own distance
  rule so the charge still crosses the gap (a pattern parameter, not a clip edit).
- The demon's tail (`Tail`..`TailNub`, 7 bones) has no gnoll counterpart (the gnoll is tailless): left
  unmapped, as the kobold's extras are.
- The axe is in the demon's left hand: the held item needs the gnoll's left-hand attachment (id 2) instead
  of the right (id 1) that the thug's dagger uses; a small `HeldItem` option.
- WoW Hogger is a 3-minute respawn zone boss; here he stays dead after his first kill unless the user
  wants the 18-minute respawn the valley uses (decide at H4).
- The terrain at Riverpark (colliders of the hill, the road) has not been walked by the engine in this
  project yet; the first check of H4 is a screenshot and a walk there.
