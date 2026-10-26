# Mage class: the second class, a cast pipeline and mana (design)

2026-10-14. The user: "Läs handover och sedan börja med att göra mage-klassen".
Status: **design approved by the user 2026-10-14 (in chat, section by section); this written spec waits for the user's read, then the plan.** Nothing built, nothing committed.

## What the user wants

A human male Mage that can be made in the creation menu and played from level 1 in the valley and Elwynn with the
game's Dark Souls feel: a mana bar, four spells (two known from the start, two from the trainer in the Abbey),
cast with Dark Souls' own sorcery animations, WoW's spell models and sounds, a missile that hits what it was aimed
at and gives XP, souls and loot like a sword blow does. Success = the user plays the Mage through a pack of kobolds
and judges the cast "in motion" as a Souls cast, not a WoW cast bar.

Said by the user (2026-10-14, answers to Claude's questions):

- **The class is the Mage**, not Claude's earlier Rogue pick. It comes through a class framework (a class is a data
  row, Warrior is row 1: decided 2026-10-12, see `wow-souls-gear-and-classes`).
- **The set is Fireball, Frostbolt, Fire Blast, Frost Armor**, rank 1 of each (Claude's recommended set, accepted).
  The user names a class's abilities themselves; nothing else is added.
- **The cast model is Dark Souls sorcery clips** (option C). Claude recommended A (a WoW cast clip with a release
  event); B was a WoW cast bar. The user chose C: casts play DSR's own cast animations through the same retarget
  pipeline as the weapons.
- "Souls ska vara farligt" (2026-10-12): enemy damage is not scaled for the new class either.

Assumed by Claude (to be judged in play): the constants under "Tuning knobs", the clip picked for each spell, that
a cast cannot be moved in, and that the WoW cast time is not used (the DS clip's own wind-up is the cast time).

## What exists (read 2026-10-14)

- `Class::Mage` is in `character.rs` (id 8) but `Class::enabled()` lets only `Warrior` through; the creation menu
  lists the others greyed. `equipment::start_kit(race, class)` returns the kit for a human warrior only.
- The warrior's abilities are hard-coded: `wow_world::abilities` (`Effect` has three variants, `ALL` three rows,
  `WARRIOR_TRAINER` 911, `rage` per ability), the game's `abilities/mod.rs` (`Rage` resource, `Buffs`,
  `Cooldowns`, `Hotbar` of four slots, `activate`), `moves.rs`, `fx.rs` (WoW spell models + sound kits through
  `loot::vfx::effect`), `ui.rs` (a thin red rage bar under the stamina bar, the four icons, the trainer's card).
- Attributes are built (`wow-world/src/attributes.rs`): Intellect and Spirit are counted and shown "but do
  nothing yet (no mana, no regeneration)". `player_classlevelstats.basemana` is imported (`ClassStat.base_mana`).
- `projectiles.rs` flies an arrow: spawned from the *weapon's* `DsMoves.projectile` at a clip event
  (`ActionState::Attack(swing)`), ballistic, swept against `posed_body` and the world, writes `Hit`. It is
  co-owned with another chat.
- `combat/action.rs` is the pure action machine: `ActionState::{Idle, Attack, Evade, Defend, Stagger, Reel, Dead}`.
- `Casting { clip, left }` (abilities/moves.rs) holds the body while a WoW cast clip plays (Battle Shout, Thunder
  Clap).
- The DS HUD cache has HP and stamina bars only (`F20-00/bar_*_hp`, `bar_*_sp`): DS1 has no mana.

## What Dark Souls says (read-only from the user's install, 2026-10-14; throwaway tools in the session scratchpad)

- **`Magic` param** (`MAGIC_PARAM_ST`, 141 rows): 3000-3xxx sorceries (`ezStateBehaviorType` 0), 4000-4xxx
  pyromancies (2), 5000-5xxx miracles (1). `refCategory` 1 points `refId` at a `Bullet` row, 2 at a behavior.
  **`refType` is the cast type**: 22 distinct values (3000 Soul Arrow = 0, 4000 Fireball = 5, 4300 Iron Flesh = 6,
  3100 Magic Weapon = 1, 3300 Magic Shield = 17 ...). Spells have `maxQuantity` casts (Soul Arrow 30), no mana.
- **`Bullet` param**: Soul Arrow (row 3000): `life` 1.1 s, `dist` 20 m, speed 4 rising to 30 m/s over 0.32 s,
  `homingAngle` 40, `hitRadius` 0.1, `atkId_Bullet` 10000. Fireball (row 6000): 15 m/s, gravity 9.8, `life` 0.5,
  `hitRadius` 0.25, `HitBulletID` 6010 (the burst on impact), `homingAngle` 10.
- **Cast clips** in `c0000.anibnd.dcx` / `c0000_a00_md.anibnd.dcx`: the blocks `a00_6200-6226`, `6300-6326`,
  `6400-6426`, `6500-6526` hold one clip per cast type (27 each; right hand / left hand / two further variants,
  hypothesis), 1.5 to 3.1 s by the TAE events' last end (the clip lengths are measured in step S3). Examples by
  that measure: `a00_6200` 1.90 s, `6205` 1.73 s, `6206` 2.33 s, `6217` 1.47 s.
- **Timing**: the clips carry `ChrActionFlag` windows 91 then 90 then 89 (`a00_6201`: 0.33-0.73, 0.73-1.20,
  1.20-1.73). Hypothesis: wind-up, release, recovery. No TAE bullet event (kind 2) exists in `a00.tae`, so the
  missile is not made by the clip: presumably by the player's ESD (`c0000.esd`), which the project does not read.
  The release instant is therefore taken as the start of flag 90 and verified in S3 / S4.
- **Catalysts**: `EquipParamWeapon` weaponCategory 8, `wepmotionCategory` 41 (a41: R1 imports a23's sword swing).
  The project maps the WoW staff to the polearm family a38 (a documented adaptation); the Mage keeps that.

## What WoW says (vMaNGOS copy through `tools\mangos-db.ps1`, build <= 5875)

| Spell | id | mana | cast | what it does (rank 1) |
|---|---:|---:|---|---|
| Fireball | 133 | 30 | `castingTimeIndex` 16, missile 24 yd/s | 14-22 fire + 2 over 4 s |
| Frostbolt | 116 | 25 | index 16, missile 28 yd/s | 18-20 frost, 40 % slower for the aura's duration (index 28; base -41 + the 1..1 die) |
| Fire Blast | 2136 | 40 | instant, 8 s cooldown | 24-32 fire |
| Frost Armor | 168 | 60 | instant | +30 armor, 30 min (it also chills melee attackers: later) |

- Known from the start (`playercreateinfo_spell`, human mage): Fireball R1, Frost Armor R1 (and Shoot / Wands,
  Staves, Cloth).
- Trainer **Khelden Bremen** (creature 198, `trainer_id` 5, Abbey): in `npc_trainer_template` 5, level <= 12:
  Arcane Intellect 10 cp (1), Frostbolt 100 cp (4), Conjure Water 100 cp (4), Fireball R2 / Fire Blast /
  Conjure Food 100 cp each (6). Of those only Frostbolt (lvl 4) and Fire Blast (lvl 6) are in the set.
- Start kit (`playercreateinfo_item`, race 1 class 8): Bent Staff 35 (two-hand staff 3-5, 2.9 s), Apprentice's
  Boots 55, Robe 56, Pants 1395, Shirt 6096, 2 Refreshing Spring Water 159, 4 Darnassian Bleu 2070, Hearthstone.
- Base stats, human mage level 1: Str 20, Agi 20, Sta 20, Int 23, Spi 22; `basehp` 32, `basemana` 100.

## Design

### 1. A class is a data row

`wow_world::class`: `ClassDef { id, power: Power::{Rage, Mana}, trainer, abilities, kit }`, `def(class_id)`.
Warrior is row 1 and is the current constants moved in (`HUMAN_WARRIOR_KIT`, `WARRIOR_TRAINER`, the three
abilities): **no warrior behaviour changes** and every warrior test stays as it is. Mage is row 2.
`Class::enabled()` lets Warrior and Mage through (race and sex stay Human and male). `start_kit` and the
trainer lists read the row. Constants come from the queries above, with the query in a comment and a test that
pins the numbers, as the warrior's did (no exporter work).

### 2. Mana

`Power` replaces the hard-coded `Rage` resource where the class decides: a `Mana { cur, max, quiet }` next to
`Rage`, only the class's own is live. Max mana is WoW's: `basemana` of the level plus the Intellect bonus
(`min(int, 20) + 15 * (int - 20)` above 20): a human mage at level 1 has 100 + 65 = **165** (and 32 + 20 = 52
health: squishier than the warrior's 60). Casting spends mana at the cast's **start** (DS spends the cast
at the start); a refused cast says "Not enough mana." Regeneration follows WoW's shape: nothing for a few seconds
after a cast, then a Spirit-based trickle; the numbers are Claude's first guesses (like the rage constants).
A death refills it (a new life starts with full mana: a mage at 0 would be stuck); rage resets to 0 as now.
The bar is the thin one the rage bar uses, blue, under the stamina bar.

### 3. The cast (C)

- **Extractor** (`dsr-cache`): a `casts` family: the cast clips (`a00_62xx` and the variant blocks) retargeted
  onto the WoW human with the existing gates (they are standing clips: drift gates are easy), written to
  `cache/dsr/casts/` with manifest lines: `cast <type> <clip> <len> wind <t> release <t> recover <t>`, where
  `release` is the start of flag 90 and `recover` the start of 89. Same staleness rule as the other caches.
- **Action machine**: `ActionState::Cast(Cast { spell, t, spec })`: the body is committed (no walking, like a
  swing); a hit that reels the body ends the cast (the mana stays spent, no missile); a roll press is accepted
  from `recover` on, as a swing accepts a follow-up from `CANCEL_FROM`. Presses come from the existing hotbar
  keys (1-4, Z / D-pad down).
- **Release**: at `release` a missile spawns at the right hand (the held weapon's root, as the arrow does),
  aimed at the lock-on target's `aim_height`, else along the body. **As built (2026-10-14):** the flight is
  WoW's, the steering is Dark Souls': speed (24 / 28 yd/s) and range (35 / 30 yd) are `spell_template.speed` and
  `SpellRange`; the `Bullet` row gives the homing angle (Soul Arrow 40, Fireball 10 degrees: the turn rate is
  the angle x 0.105 rad/s per degree, a first guess) and the hit radius; there is no gravity (DS's Fireball is
  a 10-m lob, 15 m/s with gravity 9.8, that cannot reach 35 yd). The sweep is `projectiles.rs`'s (generalized
  with `SpellFlight`: a target to steer at, a life, a radius; and a `SpellHit` message), and it writes the same
  `Hit` message (`damage`, `poise`, `knock`) so XP, souls, loot, blood and reactions work unchanged. An enemy
  that dies in flight is let go: the missile flies straight on and ends at its range.
- **Damage** is WoW's rank 1 (table above), rolled once at release. It is not scaled by a weapon (no spell power
  in 1.12); the DoT and the slow ride on the target as `Slowed` already does for Thunder Clap.
- **Instants** (Fire Blast, Frost Armor) are the shortest suitable DS cast clips with the earliest release:
  there is no instant cast in Dark Souls, so "instant" means "no extra wind-up".
- **Frost Armor** is a buff of `Buffs` (+30 armor for 30 min) that `gear_effect` reads in the armor total; the
  chill on attackers is a later step (it needs an on-hit hook in `hurt.rs`, co-owned).

### 4. Which clip each spell wears (to be judged in motion)

| Spell | DS cast type tried first | Why |
|---|---|---|
| Fireball | 5 (Fireball, `a00_6205`) | lobbed fire, gravity, burst |
| Frostbolt | 0 (Soul Arrow, `a00_6200`) | straight homing bolt |
| Fire Blast | a short one (1.5 s: `6202` / `6204` / `6211` / `6216`) | quick release |
| Frost Armor | 6 (Iron Flesh, `a00_6206`) or 17 (Magic Shield, `a00_6217`) | self buff |

**Second revision (2026-10-14, after the second play: "ingen kraft ... animationerna är dåliga"; the user's
reference: Elden Ring's sorcery showcase, youtube B3CkG1ylo_0).** The pyromancy and sorcery casts below were
judged to have no power. Elden Ring's staff casts are one shape: the staff raised overhead, then a hard swing or
thrust with the whole body and a step forward (Magma Shot, Great Glintstone Shard; the buff, Thops's Barrier, is
the staff held straight up). Only Dark Souls' data is installed, so the match is made from its two-handed weapon
moves, which have that weight, used as casts: **Fireball** `a25_4000` (greatsword R1), **Frostbolt** `a38_4000`
(the staff's R1), **Fire Blast** `a59_4300` (two-handed mace R2); **Frost Armor** keeps `a00_6506` (the staff
raised overhead). Release = the move's first hit window (the weapon connects), free = its chain / evade flag; the
missile leaves the staff's tip (the item's blade marker, else the grip); a cast steps along its clip's root
motion like a swing (a buff stands still). The impact numbers below are unchanged.

**As built, first revision (2026-10-14, after the first play: "inte särskilt darksouls"; superseded as to the clips).** The table above was a guess from the
flag windows and is replaced by Dark Souls' own mapping (memory `wow-souls-ds-magic`): `Magic.refType` is the cast
motion and the clip is `a00_6400 + refType` (right hand) or `a00_6500 + refType` (left hand) for a pyromancy,
`a00_6200 + refType` for a staff. Picks: **Fireball** `a00_6505` (motion 5, the left-hand throw; the staff stays in
the right hand), **Fire Blast** `a00_6508` (Combustion, motion 8, the palm slam), **Frost Armor** `a00_6506`
(Iron Flesh / Power Within, motion 6, the buff gesture), **Frostbolt** stays `a00_6209` (a staff cast). The
pyromancy clips carry their own bullet event (a TAE type-64 event; so the note above that no bullet event exists
holds only for the staff block): **release is that event** (Fireball 0.50 s), the body is free where the chain /
evade flags open (1.50 s of the 2.0 s clip), a staff clip keeps flag 90 / 89. `casts.txt` is version 2: each role
may add `impact <role> <blast m> <knock m> <flinch level> <poise>`, read from the attack row of the spell's last
bullet (Fireball: Bullet 6000 -> explosion 6010, 1.0 m wide, AtkParam_Pc 13000: 1.2 m, level 3, poise 40;
Combustion 0.6 m / 0.8 m / 2 / 40; Soul Arrow 0 / 0.7 m / 1 / 20). The game gives a missile's hit that flinch,
poise and push, takes in everything within the blast of where it burst (splash, the primary target spared), and
lets a pyromancy throw leave the left hand's attachment point with the fire only in that hand. WoW's range, speed,
damage, models and sounds are kept (the user's choice: DS explosion on WoW's missile).

If a DS clip looks wrong in the hand of a WoW human with a two-handed staff, the spell falls back to a WoW cast
clip (`SpellCastDirected` family, the way Battle Shout does) with the same release timing: A stays as a
per-spell reserve. The user judges each from a frame series, never stills.

### 5. Look and sound

WoW's own: `benilla-extract spellvis 133 / 116 / 2136 / 168` (Fireball visual 67, Frostbolt 13, Fire Blast 143,
Frost Armor 124), built with `vfx::effect` (an effect model is never an `ActorModel`: `wow-souls-effect-models`).
The missile is one such effect riding the projectile entity; impacts and sound kits as the warrior's abilities.

### 6. Trainer, hotbar, creation

Khelden Bremen opens the existing trainer screen with Frostbolt (lvl 4) and Fire Blast (lvl 6, the DB's level:
Elwynn is in the game now), souls prices as the warrior's (`shop::souls_for_copper`: 100 cp = 400 souls).
Fireball and Frost Armor are known from the start (`copper` 0), so the bar has two spells on day one.
`ui.rs` shows mana where it shows rage; icons from the spells' own `SpellIcon`.

## Not in v1

Drinking Refreshing Spring Water (S7), Conjure Water / Food, Arcane Intellect, the wand's Shoot, spell ranks
(Fireball R2), Frost Armor's chill, Arcane Missiles / Polymorph, mage quests, other races and sexes, the
spellbook UI.

## Steps (each verified live, then HANDOVER updated)

| | Step | Proof |
|---|---|---|
| S1 | Class row; Warrior moved in | all existing tests green, warrior unchanged in a live run |
| S2 | Mage character: creation enabled, kit, mana + bar, save / load | `WOWSOULS_*` run with a mage: HUD shot, 165 mana, 52 HP |
| S3 | Cast clips: extractor family + a read-only measure of lengths and flags | table in the log, frame series of 4 candidate clips |
| S4 | `ActionState::Cast` + Fireball end to end | frame series: cast, release, flight, hit, XP / loot |
| S5 | Frostbolt (slow), Fire Blast, Frost Armor | one frame series each, the warrior unchanged |
| S6 | Trainer, hotbar, regeneration | scripted learn, mana math in tests |
| S7 | Drinking water, balance pass | the user plays a pack of kobolds |

## Tests

Pure parts unit-tested: the class rows (Warrior's numbers unchanged), the mana formula (165 at level 1) and
regeneration, spell damage ranges, the cast state's interrupt / cancel rules, missile homing and gravity, the
trainer's refusals ("You must reach level 4"). Install tests (skip without `DSR_DIR` / `WOW_DATA`): the cast
clips exist and retarget inside the gates, `Magic` 3000 / 4000 and `Bullet` 3000 / 6000 hold the numbers the code
quotes. Live: unattended frame series from the game's camera plus a front view, with a warrior regression run.

## Tuning knobs (first guesses, in code constants)

Mana regeneration numbers and the quiet delay, the yard conversion of DS's metres for the missiles (the arrow's
`spec.speed` convention), missile damage `poise` / `knock`, which clip each spell wears, `MANA_OFFSET` if the
level-1 pool of 165 feels wrong (the same lever as `HEALTH_OFFSET`).

## Risks and what the plan must measure

1. **Release instant**: flag 90's start is a hypothesis from the event windows; check it against the effect timing
   in motion before building on it.
2. **A DS cast with a WoW staff**: the catalyst / flame is in the DS hand, ours is a two-hand staff; the pose may
   read oddly (hence the per-spell WoW-clip reserve).
3. **Lengths**: 1.5 to 3.1 s casts in a game where kobolds hit fast; Souls is meant to be dangerous, but S4's
   playtest may ask for shorter clips or a faster playback rate.
4. **Co-owned files**: `projectiles.rs`, `hits.rs`, `hurt.rs`, `defense.rs`, `gear_effect.rs` change by a few
   lines each; re-read before every edit.
5. **Quests**: `RequiredClasses` masks must show the Mage's quests and hide the warrior's (`quest/rules.rs` has
   class handling: check, do not assume).
6. **Item selection**: the kit's items join `select.rs`'s item selection, so the manifest is rebuilt and the
   `wsworld` version may need a bump.
