# Guard, parry, riposte: what the Dark Souls Remastered data says (read-only survey)

Date 2026-10-09. Nothing in the game or the installs was changed; no build was run. This is the report the
handover asked for ("Parry / block: go and check"). It answers four questions with numbers from the files:
the player's guard clips (section 2), the params (3), the four creatures (4), and what Northshire gives (5);
section 6 lists what could NOT be found, 7 corrects the older docs, 8 is the recommended minimal design, 9 the work steps.

Evidence tags used below: **[V]** read directly from the files and exact; **[I]** inferred from patterns in
the files (stated reason); **[?]** not found / not decidable from the files. Frames are 30 fps (frame count / 30 = the
project's `survey` seconds). "m" is a DS metre (1 m = 1.0936 yd, the project's conversion).

## 0. The answer in ten lines

1. Everything needed exists in the data: a guard stance with a flagged guard-active window, guard-hit reactions
   (S / L / LL / min, plus guard-break), a parry (a 7-frame hit volume), a riposte and a backstab with their position rules and damage.
2. The straight sword can block: `EquipParamWeapon` 200000 has `enableGuard` 1, `physGuardCutRate` 50, stability
   (`staminaGuardDef`) 32, `guardLevel` 2; it cannot parry (`enableParry` 0). Shields and daggers parry; **bare fists parry too** (row 900000, set `a42`).
3. The guard-hit reactions `a00_2060-2077` are **additive** animations (Havok `blendHint` 1), not poses; the project's retarget cannot play them as is.
4. The parry is not a "window in a clip" you test for: it is an attack event (TAE type 1, first int 64, judge 450) whose
   `AtkParam` has no damage and a 0.55 m radius, active frames 0-7 of a 36-frame clip. Creatures have the same thing (judge 900, radius 0.8 m, clip `a00_3100`).
5. Riposte and backstab are `ThrowParam` rows: the player plays `a<class>_3201` / `a<class>_3400` and the victim `a00_2510` / `a00_2500`; damage x2.5 / x2.0 (judge 510 / 500).
6. Of the four creatures: the kobold (c2060) has backstab, riposte and a "parried" clip but cannot guard or parry; the black dog (c3340) has none of the three (no `ThrowParam` row, no `a00_2100`);
   the thug (c2520) and Garrick (c2400) guard (`NpcParam` cut 75, level 4), parry (`a00_3100`), and riposte the player (`ThrowParam` 252001 / 240001).
7. Northshire does have shields (section 5): the armorer Godric Rothgar sells a Small Shield and a Large Round Shield. (A Pikeman Shield is the reward of "Report to Goldshire", which is handed in in Goldshire, and Mine Spiders drop bucklers at 0.04-0.14 %: neither counts.)
8. Shield-less, the data's own answer is: block with the weapon two-handed (`a10` stance clips carry the guard flag), parry with the bare left hand (`a42_5100`).
9. What the files do not hold: the engine's block rule (angle, stamina and damage formulas), the riposte-able state, the guard-walk clip choice (section 6).
10. Recommended: a two-step build, block + guard break first, parry + riposte second, enemy guard and parry last (sections 8-9).

## 1. How this was read

`wow-souls-dsr survey --chr cNNNN` reads only `chr/<chr>.anibnd.dcx`; for `c0000` it prints "61 bones, 0 animation files"
because the player's clips live in `chr/c0000_a00_*.anibnd.dcx` and `c0000_a0x`-`a9x`. There is no CLI command for player clips, and
building one was out of bounds (RAM-tight machine). So the survey was done with a **read-only Python port** of the project's own readers
(`dsr-formats`: DCX, BND3, TAE, PARAMDEF + PARAM, Havok tagfile, spline decode; plus the FMG for item names and a forward-kinematics pose of the 61-bone skeleton).
The scripts are throwaway files in the session scratchpad, not part of the repo.
Validation, so the numbers can be trusted: the port reproduces the project's own test values (`a00_0700` 51 frames, root z -3.94; row 200000 category 23 / variation 2300;
`BehaviorParam_PC` 102300010 stamina 20) and the built CLI's lines for the thug are identical to the port's
(`c2520` `a00_3100` 51 f 1.70 s "hit judge 900 0.00-0.23s radius 0.80", `a00_2100` 79 f 2.63 s, `a00_2500` 178 f, `a00_2510` 196 f, `a00_9900`-`9930`).
Poses were read from the decoded clips: the left-hand item dummy `L_Shield` measured forward of / above `Spine1` ("fwd / h" below, DS forward is -z).
The installs were only opened for reading.

## 2. The player's clips (c0000)

### 2.1 What the TAE type-0 flags mean here

Type-0 events are `ChrActionFlag` windows (the first int is the flag). The project already reads 8 (invincible), 4 (chain), 26 (evade), 11 (move).
New, from this survey:

| Flag | Where it appears | Reading |
|---|---|---|
| **85** | `a00_0005`, `0104`, `0108`, `0132`, `0138`, `0140`-`0152`, `0214`, `0519`, `0524`, the same ids in `a02`/`a03` (which also flag `0001`, `0003`), and in the two-hand set `a10` (`0001`, `0100`, `0106`, `0130`, `0136`, `0140`-`0142`, `0210`, `0515`, `0520`) | **guard state / guard-capable stance** [I]: pose-checked, every clip with the left item held out in front (fwd >= 0.5 m, h >= 0) carries it, and no raised-arm clip lacks it; the `0140`-`0147` families carry it with the arm lower or at the chest. Its window is the guard-active time |
| 3 and 18 | the parry clips (`5100`: 3 over 0.00-0.23, 18 over 0.00-0.27), the kick `3100` (3 over 0.20-0.53), creature `a00_3100` | "special attack running" [I] |
| 37, 15, 83 | idle / stance clips | stance bookkeeping [?] |
| 41 | stagger-like and committed clips (rolls, `2050`-`2053`, `2100`/`2101` 0.00-1.83 s on creatures) | "cannot act yet" [I] |
| 71 | every damage reaction `2000`-`2057`, `2080`-`2088` | "in a damage reaction" [I] |
| 67, 69, 50, 51 | the victims of throws (`2500`, `2510`, `9000`...) | "being thrown" [I] |
| 5 | one frame just before every creature hit window (e.g. thug `a00_3003`: flag 5 at 0.83-0.87, hit at 0.867) | the AI's "attack starts" notification, **not** a parry flag [I] |

TAE type-1 (`InvokeAttackBehavior`) events: the third int is the judge id, as the project reads it. The **first int is an attack-type code**
[V as a pattern over every event of the c0000 TAE]: 0 for ordinary attacks, 2 for the kicks (judge 410 / 420, 7 events), 62 for the plunging attacks (judge 40 / 50 / 240 / 250, 82 events),
**64 for the parry** (judge 450, 34 events, all in `a00_0110` and the `5100` / `5102` / `5103` / `5130` clips of the parry-capable classes).
The creatures use the same code: `c2520` and `c2400` `a00_3100` carry it with judge 900 (c2060 and c3340 have no event with 64).
Event type 304 in the throw clips is the throw's damage event (below). Event types 112 / 128 are on nearly every clip (sound / effects, not read).

### 2.2 The clips

All `a00_*` ids are in `c0000_a00_lo/md/hi.anibnd.dcx` (the base set); `a02` / `a03` hold copies of the guard clips below (same size, as the project's loader requires, and the same poses where checked: `0005`, `0104`, `0108`, `0150`, `0151`); `a10` is the two-hand stance set (`wepmotionBothHandId` 10, which the Shortsword uses).
"fwd / h" = the left item dummy relative to `Spine1`, metres, at the start / middle / end where it moves.

**Guard (shield, or a weapon in the left hand, `guardmotionCategory` 2 pose)** [V flags and lengths, I roles]

| Role | Clip | Frames (s) | Root | TAE and pose |
|---|---|---|---|---|
| raise | `a00_0104` | 17 (0.57) | none | flag 85 over the whole clip 0.00-0.70; arm from fwd 0.39 / h -0.04 to 0.65 / 0.13 by the middle; cancel flags (4, 26, 29, 31, 32) open at 0.07 |
| hold (stand) | `a00_0005` | 41 (1.37) | none | flag 85 0.00-2.33 (3, 37 too); pose fwd 0.64 / h 0.12, constant: a loop |
| lower | `a00_0105` | 12 (0.40) | none | no 85; pose from the guard pose (0.64 / 0.12) to rest (0.12 / -0.34); cancel flags from 0.10 |
| raise, variant | `a00_0108` | 17 (0.57) | none | flag 83 0.00-0.10 then 85 0.00-0.73 |
| slow raise | `a00_0132` / `0138` | 31 (1.03) | none | 85 only 0.87-1.67; the arm rises from fwd 0.24 / h -0.26 to 0.64 / 0.12 |
| stand, moving set | `a00_0150` / `0151` / `0152` | 25 (0.83) / 51 (1.70) / 65 (2.17) | z 0 / **+2.04** / **+5.09** (backwards, DS forward is -z) | 85 over the whole clip; `0152` has flag 8 (invincible) 0.00-0.83 and 51: a back-step with i-frames [I] |
| stand, alt | `a00_0214` | 41 (1.37) | none | 85 0.00-1.93; fwd 0.55 / h 0.15 |
| step variants | `a00_0519` (17 f, none), `a00_0524` (23 f, z -2.69 = 3.5 m/s forward) | | | 85 0.00-1.33 |

The same ids come in two more pose families that belong together: a chest-high forearm (fwd 0.34 / h 0.03, `0001`, `0100`, `0140`-`0142`, `0210`, `0515`, `0520`) and a low, wide one (fwd 0.42 / h -0.21, `0003`, `0102`, `0145`-`0147`, `0212`, `0517`, `0522`).
`guardmotionCategory` has three values in `EquipParamWeapon` (0: medium shields, 1: greatshields, 2: small shields and every other weapon) and there are three arm poses, so each category very likely selects one family [I];
the data does not say which clip the engine plays for guard-walking. The plain walks and runs (`0200`-`0207`, `0300`-`0313`, `0500`-`0563`) keep the left item low (fwd 0.0-0.3, h -0.2 to -0.35): they are not guard clips.

**Two-hand stance (`a10`, what a Shortsword uses two-handed)** [V flags and lengths, I roles]: the clips with flag 85 are the idle `a10_0001` (61 f, flag over 0.00-2.73), the raise `a10_0100` / `0106` (17 f; flag 85 from **0.20**, so the first 6 frames do not guard),
`0130` (51 f) / `0136` (37 f) (85 from 1.17 / 1.07), the set `0140` (25 f) / `0141` (51 f, walk back, z +2.04) / `0142` (65 f, back-step, z +5.09, flag 8 over 0.00-0.83), and `0210`, `0515`, `0520`; the lower `a10_0101` (21 f, 0.70 s) has no 85.
In all of them both hands sit together in front (left hand fwd 0.34 / h 0.05, right hand fwd 0.35 / h 0.16): a two-hand ready pose, and the data flags it as the guard stance.

**Guard-hit reactions** [V ids, lengths and blend; I on which class is which]. `a00_2060`-`2077` have `blendHint` 1 (additive: deltas applied on top of the pose underneath, near-zero translations, hand and head excursion about 0.15 m and 0.05 m)
and **no TAE events at all**. Their classes follow the order of the player ESD's table (see 6), which lists each damage reaction beside its guard twin: `2000 <-> 2060`, `2004 <-> 2064`, `2010 <-> 2070`, `2020 <-> 2075`,
and the `KnockBackParam` fields `guard_S` / `guard_L` / `guard_LL` / `guardBrake`:

| Class | Clips | Frames (s) |
|---|---|---|
| guard S (blow level 1) | `a00_2060`-`2063` | 26 (0.87) each |
| guard L (level 2) | `a00_2064`, `2065`, `2066`, `2067` | 58, 58, 56, 51 (1.7-1.93) |
| guard LL (level 3 and up) | `a00_2070`-`2073` | 60, 60, 61, 60 (2.0) |
| guard min | `a00_2075`-`2077` | 16 (0.53) |
| **guard broken** (full-body, not additive) | `a00_2080`-`2083` | 71 (2.37); root x -1.07 / x +0.80 / z +1.17 / z -1.35; no i-frames; cancel flags 4 / 26 / 29 open about 1.4-1.5 s [I that this is the stamina-out reaction] |
| guard broken, heavy (knockdown) | `a00_2085`-`2088` | 136 (4.53); root 3.40 m (x -3.40 yaw -1.57 / x +3.40 yaw +1.57 / z +3.40 / z -3.94); flag 8 0.00-3.70 or 3.83, flag 44 [I] |

Four clips per class = four push directions (as `2000`-`2003` are); the project's `choose` by head push applies. The additive ones cannot go through `retarget` as absolute poses
(the older note in `reacts.rs` that `2200`-`2213` "do not retarget" is the same cause: they are additive too; the full list of additive clips in c0000 is exactly `2060`-`2067`, `2070`-`2073`, `2075`-`2077`, `2200`-`2207`, `2210`-`2213`).

**Parry** [V] (the left hand: dagger `a20`, shields `a48`, **fists `a42`**, rapier `a27`, curved swords `a28`, whip `a43`; also `a00_0110`, `a71`, `a91`, DLC `a130`)

| Set | Clips | Frames (s) | Parry volume (TAE type 1, first int 64, judge 450) |
|---|---|---|---|
| `a48` shields | `5100`, `5102`, `5103` | 36 (1.20) each | **0.000-0.233 s = frames 0-7** |
| | `5130` | 81 (2.70) | 0.833-0.867 (one frame) |
| `a20` dagger | `5100`, `5102`, `5103` / `5130` | 36 / 71 (2.37) | 0.000-0.233 / 0.667-0.733 |
| **`a42` fists** | `5100`, `5102`, `5103` / `5130` | 36 / 71 | 0.000-0.233 / 0.867-0.900 |
| `a27` rapier | `5100`-`5103` | 26 (0.87) | 0.000-0.200 (6 frames) |
| `a28` curved | `5100`-`5103` / `5130` | 41 / 81 | 0.000-0.233 / 0.833-0.867 |
| `a00_0110` | | 36 | 0.000-0.233 |

The i-frames: none (no flag 8 in any parry clip). Cancel flags (`a20_5100`): evade 26 from 0.67, chain 4 from 0.73, move 11 from 0.83. Greatshields (`a47`) have **no** `5100` clip, consistent with `enableParry` 0 on those rows; the Shortsword set `a23` has none either.
`5100` / `5102` / `5103` share timing; why three (hand state or stance variants) is not decided [?]. A forum measurement of the original game gives 7 parry frames at 30 fps for ordinary shields
([source](https://forum.norestforthewicked.com/t/parrying-frame-data/26049), secondary): it agrees.

**Kick (guard break)** [V]: `a23_3100` (right hand, 51 f 1.70 s, root z -0.47; its judge-410 hit is an event of type 307 at 0.43-0.57), `a48_3100` / `a47_3100` (z -0.66; a type-1 event with first int 2, judge 410, window 0.40-0.53 and 0.37-0.50); two-hand is 4100 / judge 420.
`AtkParam_Pc` 2300410: `atkStamCorrection` 280 (dagger 2000410: 700, shield 4800410: 360), `guardAtkRateCorrection` 500, `dmgLevel` 5, `knockbackDist` 1.47 m, no damage; costs stamina 10 (sword), 14 (shield).

**Riposte and backstab (the player side)** [V ids, lengths, damage rows; I on the order of events]

| | Clip (straight sword `a23`; the dagger `a20` has the same ids) | Frames (s) | TAE |
|---|---|---|---|
| backstab | `a23_3400` | 129 (4.30), root 0 | type 304 at 0.833-0.933 with ints 240, **500**; type 116 0.83-3.03; cancel flags from 3.33-3.43 |
| riposte | `a23_3201` | 162 (5.40), root 0 | type 304 at 0.833-0.933 with ints 240, **510**; cancel flags from 4.60-4.70 |
| other backstab / riposte | `a23_3401` (111 f), `a23_3202` (211 f, 7.03 s) | | used against player victims (rows 100101 / 100102 -> victim `a00_9420` / `9430`) |

Damage: `BehaviorParam_PC` variation 2300 judge 500 -> `AtkParam_Pc` 2300500 `atkPhysCorrection` **200** (x2.0); judge 510 -> 2300510 **250** (x2.5); stamina 20 each.
Dagger (variation 2000): 250 / 300. Neither clip carries flag 8: no i-frames of their own. Both are two-body animations (the victim's clip is 5.9 / 6.5 s, section 4).
Not every class has them: `a20`, `a23`, `a25`, `a26`, `a33`, `a36`, `a42` do (`3400`, `3201`); `a27`, `a28`, `a47`, `a48` and others do not.

## 3. The params

The PARAMDEFs carry every field named here (read by name; `GameParam.parambnd` has 40 params). **There is no `ParryParam`** and nothing named parry beyond:
`EquipParamWeapon.attackBaseParry` / `defenseBaseParry` (0 on every weapon), `parryDamageLife` (10 everywhere) and `NpcParam.parryAttack` / `parryDefence` (0 on all four creatures) —
the DS2-style parry power is unused in this game. **`guardAngle` is 0 on all 1245 weapon rows and 555 of 556 `NpcParam` rows**: the guard cone is not data, it is in the engine [V / ?].

### 3.1 Weapon rows (`EquipParamWeapon`, base rows) [V]

| Row | Name | Class (`wepmotionCategory` / variation) | `enableGuard` / `enableParry` | `physGuardCutRate` | stability `staminaGuardDef` | `guardLevel` | `guardmotionCategory` | `guardBaseRepel` / `attackBaseRepel` |
|---|---|---|---|---|---|---|---|---|
| 200000 | **Shortsword** | 23 / 2300 | **1 / 0** | **50** (mag 10, fire 35, thun 35) | **32** | **2** | 2 | 10 / 20 |
| 201000-211000 | the other straight swords | 23 | 1 / 0 | 50 (Broken Straight Sword 20, stab 16, level 1) | 32 | 2 | 2 | |
| 100000 | Dagger | 20 / 2000 | 0 / **1** | 45 | 26 | 1 | 2 | 10 / 20 |
| 900000 | **Fists** | 42 / 4201 | 0 / **1** | 0 | 0 | 1 | 2 | 10 / 20 |
| 1405000 | Buckler | 48 / 4801 | 1 / 1 | 76 | 40 | 4 | 2 | 30 / 20 |
| 1403000 | Small Leather Shield | 48 / 4801 | 1 / 1 | 80 | 40 | 4 | 2 | 30 / 20 |
| 1404000 | Target Shield | 48 / 4801 | 1 / 1 | 78 | 45 | 4 | 2 | 30 / 20 |
| 1400000 / 1450000 | East-West / Heater Shield | 48 / 4800 | 1 / 1 | 88 / 100 | 44 / 55 | 4 | 0 | 50 / 20 |
| 1500000 | Eagle Shield (greatshield) | 47 / 4701 | 1 / **0** | 95 | 70 | 4 | 1 | 65 / 60 |

All of these are equippable in left, right and both hands (`leftHandEquipable` / `rightHandEquipable` / `bothHandEquipable` 1). Weapon-guarding classes: straight swords (phys 50, stab 32), greatswords 60 / 38, ultra 70 / 44.
Reinforcement scales `physicsGuardCutRate` / `staminaGuardDefRate` (`ReinforceParamWeapon`; type 0 is 1.0 at +0).

### 3.2 What a blocked blow costs: the attacker's rows [V fields, I on the formula]

`AtkParam_Npc` carries, per attack: `atkPhys` (the damage the project already uses x 25/138), `atkStam` (stamina damage to an unguarded target), **`guardAtkRate`** (the stamina cost to a guard; 100 for most attacks in the game, 20 / 40 / 60 for light ones),
`guardBreakRate` (17 rows, 10-70), `disableGuard` (246 rows: unblockable), `throwFlag`. `guardStaminaCutRate`, `guardRate` (Npc) and `guardCutCancelRate` are zero in every row of both tables.
The engine formula is not in the files; the usual community reading is stamina lost = `guardAtkRate` x (1 - stability / 100), damage taken = `atkPhys` x (1 - `physGuardCutRate` / 100) [I, unverified].

| Creature attack (project name) | judge | `atkPhys` -> game damage | `atkStam` | `guardAtkRate` | notes |
|---|---|---|---|---|---|
| kobold `small_1` / `small_2` | 100 / 110 | 138 / 142 -> 25 / 26 | 30 | 20 | |
| kobold `big` | 120 | 195 -> 35 | 60 | 20 | level 2 |
| kobold `run_strike` | 130 | 186 -> 34 | 100 | 40 | level 2 (TAE first int 1 on this event) |
| kobold `body_check` | 140 | 106 -> 19 | 140 | 60 | `dmgLevel` 5, `atkType` 1, `disableStaminaAttack` 1 |
| kobold `pounce` | 160 | 124 -> 22 | 60 | 100 | **`disableGuard` 1, `throwFlag` 1, `throwTypeId` 2060**: unblockable, a grab (`ThrowParam` 206000) |
| wolf `bite` / `lunge` | 100 / 130 | 104 / 100 -> 19 / 18 | 20 | 20 | |
| thug `slash_1` / `slash_2` | 130 / 140 | 150 / 153 -> 27 / 28 | 40 | 40 | |
| Garrick `double` / `triple` | 120 / 130 | 140 / 155 -> 25 / 28 | 60 | 60 | 2 / 3 windows |
| Garrick `lunge` / `dash` | 150 / 160 | 179 / 173 -> 32 / 31 | 120 | 100 | level 2 |
| Garrick `quick` | 180 | 105 -> 19 | 20 | 20 | |

The game damage column is `atkPhys` x 25/138 rounded, without a species factor (Garrick's `x0.8` in `species::GARRICK` is not applied).
With the formula above, a Shortsword block (stability 32) of a kobold `small_1` costs 20 x 0.68 = 13.6 stamina and 12.5 damage instead of 25; a Garrick lunge costs 68 of the 100-point bar, two of them break the guard.
(The project's stamina is 100 max, 45/s regen after 0.6 s: `combat/mod.rs`.)

### 3.3 What the player's guard-related moves cost [V]

`BehaviorParam_PC` stamina: **parry 14 (shield / fists, judge 450 in variations 4801 / 4201) or 15 (dagger, rapier, curved: 2000 / 2700 / 2800)**; kick 10 (sword) / 14 (shield); backstab and riposte 20 (judges 500 / 510).
The parry's `AtkParam_Pc` (e.g. 2300450, 2000450, 4800450): `hit0_Radius` **0.55 m**, dummy poly **131**, `atkPhys` 0, `atkType` 0, `atkAttribute` 4, `spAttribute` 1, `hitSourceType` 1, no damage, no knock-back.
The guard-state rows 430 / 440 / 600 exist with no volume (`guardRate` -50 on 22 `AtkParam_Pc` rows, e.g. 2000600 / 4800600) [?: engine hooks, meaning unknown].

### 3.4 Knock-back on a guard (`KnockBackParam` row 0, the id the four creatures name in `knockbackParamId`; the player's row is not named in the files read; rows 0, 1, 10 exist) [V]

`guard_S` 0.05 s + 0.20 s decay, `guard_L` 0.02 s + 0.70 s, `guard_LL` 0 + 0, `guardBrake` 0.07 s + 0.20 s (damage classes for comparison: S 0.01 + 0.15, M 0.03 + 0.69, L 0.05 + 1.0). Three classes: S / L / LL plus brake, matching the clip classes in 2.2.

### 3.5 Creatures' guard stats (`NpcParam`) [V]

| Creature | row | `physGuardCutRate` | `staminaGuardDef` | `guardLevel` | `stamina` (guard pool) | `parryAttack` / `parryDefence` |
|---|---|---|---|---|---|---|
| kobold (ghoul) c2060 | 206000 | 0 | 0 | 0 | 100 | 0 / 0 |
| black dog c3340 | 334000 | 0 | 0 | 0 | 100 | 0 / 0 |
| undead thief c2520 | 252000 | **75** | 0 | **4** | 50 | 0 / 0 |
| assassin c2400 (Garrick) | 240000 | **75** | 0 | **4** | 100 | 0 / 0 |

### 3.6 `ThrowParam`: backstab, riposte, and the creatures' throws [V]

Fields per row: `AtkChrId` / `DefChrId` (0 = the player), `Dist`, `DiffAngMin` / `DiffAngMax`, `upperYRange` / `lowerYRange`, `diffAngMyToDef`, `atkAnimId`, `defAnimId`, `throwType`, `PadType`, `atkSorbDmyId` (the dummy poly the victim is attached to), `isTurnAtker`.

| Row | Meaning | Values |
|---|---|---|
| 12060 / 12400 / 12520 | **backstab** on c2060 / c2400 / c2520 | `Dist` 1.5 m, angle 0-60 deg (behind), +0.5 / -0.65 m vertical, `diffAngMyToDef` 90, player clip `a<class>_3400`, victim `a00_2500`, `throwType` 0, `atkSorbDmyId` 230, `isTurnAtker` 1, `PadType` 1 (R1) |
| 22060 / 22400 / 22520 | **riposte** on the same three | same `Dist` and vertical, angle **90-180** (face to face), player `3201`, victim `a00_2510`, `throwType` 8, `atkSorbDmyId` 231 |
| 112xxx / 122xxx | variants `throwType` 11 / 12 | atk ids 3402 / 3203: **no animation file for them exists in any c0000 set** (3401 / 3202 do); unplayable [V] |
| **none for c3340** | the dog | no row with `DefChrId` 3340: it cannot be backstabbed or riposted [V] |
| 1-4, 100101-100104 | the player as victim (`DefChrId` 0) | `defAnimId` 9000 / 9010 / 9190 / 9420 / 9430 / 9440 |
| 206000 | **c2060 grabs the player** (the `pounce`) | `AtkChrId` 2060, `Dist` **10 m**, angle 0-180, `atkAnimId` 3300, victim `a00_9220` (in `c0000_c2060.anibnd`), `escHp` 25, `PadType` 3 (mash) |
| 240001 / 252001 | **c2400 / c2520 riposte the player** | `Dist` 4.0 m, angle 90-180, `atkAnimId` 3101, victim `a00_9010` (Garrick) / `a00_9350` (thug, in `c0000_c2520.anibnd`), `throwType` 8 |
| 240000 / 252000 | c2400 / c2520 backstab the player | `Dist` 2.0 m, angle 0-60, `atkAnimId` 3300, victim `a00_9000` / `a00_9360` |

The creatures' throw damage is the `AtkParam_Npc` judge 600 / 610 row (thug 252022 / 252023: `atkPhys` 235; Garrick 240011 / 240012: 275; `disableGuard` 1, `throwFlag` 2): at the project's factor that is **about 43 and 50 damage, half of the player's 100**.

## 4. The four creatures [V unless tagged]

| | kobold c2060 | black dog c3340 | undead thief c2520 | assassin c2400 (Garrick) |
|---|---|---|---|---|
| Guards (stats) | no | no | yes: cut 75, level 4, pool 50 | yes: cut 75, level 4, pool 100 |
| Guard clips (own, absolute) | none | none | `a00_9910` raise (19 f 0.63 s, flag 3 0.17-1.63), `9900` hold (61 f 2.03 s, flag 3 0.00-2.33, shield at chest), `9920` lower (36 f 1.20 s), `9930` (71 f, 2.37 s, root z +1.21, the right arm raised mid-clip: role unknown [?]); roles of `9900`-`9920` [I from pose and from the thug AI using action 9910 for `GOAL_COMMON_Guard`]. Also `1510` / `1610` / `1620` (3.0-3.4 s, shield out front, role unknown) | same ids (`9900`-`9930`) |
| Guard-hit overlays (additive) | `a00_2300`-`2302`, `2330`-`2333`, `2360`-`2363`, `2400`, `2430`-`2433` (19-41 f) [I: the enemy ESD pairs `2002 <-> 2302`, `2004 <-> 2330`, `2010 <-> 2360`] | same ids, 21-36 f | same ids, 36-41 f | same ids |
| Parries (clip) | no | no | **`a00_3100`**: 51 f (1.70 s), hit judge **900** at **0.00-0.23 s**, radius **0.80 m** (`AtkParam_Npc` 252024, dummy poly 8, no damage), flags 3 and 18 as the player's parry; first int 64 | `a00_3100`: 36 f (1.20 s), same event (`AtkParam_Npc` 240013) |
| "Parried" clip | `a00_2100`: 81 f (2.70 s), flag 41 0.00-1.83, type-67 event 0.00-1.83 | **none** | `a00_2100`: 79 f (2.63 s), flag 41 0.00-1.83, type-67 0.00-1.83 | `a00_2100`: 79 f (2.63 s) |
| Backstabbed / riposted clips | `a00_2500` 178 f (5.93 s, root z -2.53, yaw -2.87) / `2510` 196 f (6.53 s); follow-ups `2501` 70 f, `2511` 81 f | **none** | `a00_2500` 178 f (z -2.12, yaw -2.94) / `2510` 196 f; `2501`, `2511` | same (`2500` yaw -3.14) |
| Unblockable moves | `pounce` (judge 160) | none | judge 600 / 610 throws | judge 600 / 610 throws |
| AI script functions (compiled Lua strings) | `GuardBreak_Act` (breaks the player's guard), `Odds_Guard`, `RebByOpGuard_Step` (steps back when its blow is blocked) | `RebByOpGuard_Step`, `MissSwing_Int` | adds `SuccessGuard_Act`, `Parry_Act` (`ParryAng` / `ParryDist` / `ParryThrowAng` / `ParryThrowDist`), `Backstab_Act`, `ObserveAreaForBackstab`; `GOAL_COMMON_Guard` with guard action **9910** | same plus `FindGuardBreak_Act`, a guard-break counter |

Which creature attacks can be parried: **no TAE parry window or "parry-able" flag exists on any of their attack clips** [V: all attack clips of the four were read; the only per-attack marker, flag 5, is the AI's attack-start notice].
The decision is the engine's, with these data-side facts: the player's parry volume (judge 450, 0.55 m) must overlap the creature; a creature that has no `a00_2100` (the dog, all large demons: 38 of the 168 creature archives have it) has no clip to react with;
and the flinch / super-armor fields are zero for these four (`superArmorDurability` 0). Whether a given attack is parried by timing (before its hit window) is **not decidable from the files** [?].
**What a riposte target needs** [V on the data side]: a `ThrowParam` row with its chr id and `throwType` 8 (c2060, c2400, c2520 have it; c3340 does not), the victim clips `2510` / `2511` in its archive, the player within 1.5 m (1.64 yd), 90-180 deg facing it, within +0.5 / -0.65 m height;
and to be in a riposte-able state: the only clip family that carries a "stunned, thrown-at" marker is the parried one: **event type 67 occurs in `a00_2100` / `2101` (also `a10_2100`) and in each creature's `a00_2100`** and, unrelated to guarding, in `a00_6613` and the attack clips of the set `a117`; the window is 0.00-1.83 s (creature) / 0.00-1.57 s (player), first int 30 [V occurrence, I that it is the riposte-able window].

## 5. What Northshire gives the player

The handover says "no shield known" for the valley. The exported world tables (`cache/mangos/tsv`: the 41 creature entries of `cache/world/northshire.txt`, their vendor lists, loot ids and quests; direct `item_template` lookups, class 4 subclass 6 = shield) say a shield is on sale [V]:

| Source | Item (entry) | WoW block / armor | Counts? |
|---|---|---|---|
| vendor **Godric Rothgar** (1213, "Armorer & Shieldcrafter", level 5) | Small Shield (17184), Large Round Shield (2129) | block 1 | **yes**, in stock |
| quest "Report to Goldshire" (54), fixed reward | Pikeman Shield (6078): block 1, armor 55 | | no: given by Marshal McBride in the valley but **handed in to Marshal Dughan in Goldshire** |
| loot Mine Spider (43, level 8-9) | Burnt Buckler (15895) 0.143 %, Warrior's Shield (1438) 0.1 %, Pioneer Buckler (7109) 0.04 % | block 2-3 | in practice no (drop chances) |

Weapons from "Brotherhood of Thieves": Militia Dagger (2224, a dagger: `a20` parries), Hammer, Shortsword (1161), Warhammer, Quarterstaff. So the left hand can hold a bought shield, the dagger, or nothing.
Mapping WoW items to DS rows is a design choice; the closest rows by size are Buckler 1405000 / Small Leather Shield 1403000 (small shields, set `a48`, guard category 2, parry). The WoW human model has a shield attachment (the project's `HeldItem` uses attachment 1, the right hand; the shield goes on the left-forearm one) [I from the M2 attachment convention; not checked in this survey].

**A shield-less guard by the DS data** has two forms, both real rows:
1. **Weapon block**: the Shortsword itself guards (`enableGuard` 1, cut 50, stability 32, level 2). Two-handed it uses the `a10` stance, where every clip is flag-85 guard-capable (2.2).
   One-handed in the left hand it would use the small-shield pose (guard category 2). A right-hand sword with an empty left hand has no one-hand guard in the data.
2. **Fist parry**: with the left hand empty the slot is Fists (row 900000: `enableParry` 1, `enableGuard` 0), whose parry is `a42_5100` (36 f, volume frames 0-7, stamina 14) [I that the empty slot is Fists in DSR; the row and clips exist].
So the cheapest faithful shield-less pair is: **block two-handed with the sword, parry with the bare hand**; a shield adds the better numbers (76-80% cut, stability 40) and the one-hand guard clips.

## 6. What could not be found or decided

- The block rule itself: the angle (`guardAngle` is 0 everywhere), the damage and stamina formulas (3.2 is the community reading), when a block becomes a guard break, how `guardLevel` and `dmgLevel` choose S / L / LL (the order in the player ESD suggests level 1 -> S, 2 -> L, higher -> LL).
- Which clip the engine plays for a guard held while walking (candidates: `a00_0151`-family backwards, `0524` forward at 3.5 m/s; a forward guard-walk at walking speed is not among the flagged clips), and the exact `guardmotionCategory` -> pose family mapping.
- The riposte-able state and whether a creature attack is parried by timing (section 4); the dummy poly geometry (130 / 131 for the volumes, 230 / 231 for the throws, 8 for the creature parry): the model files were not read.
- The player ESD (`chr/c0000.esd.dcx`, 406 KB, format `fsSL`) was only scanned for the animation ids it uses: it holds the damage table `(2000, 2060, 2030, 2200)`, `(2004, 2064, 2034, 2204)`, `(2010, 2070, 2054, 2210)`, `(2020, 2075)`, and `2080`, `2085`, `2100`; the enemy ESD (`enemyCommon.esd.dcx`) the pairs `(2002, 2302, 2312)`, `(2004, 2330, 2340)`, `(2010, 2360, 2370)`. It was not parsed, so the state logic is unread.
- The creatures' Lua AI (`script/*.luabnd.dcx`) was read for function names only; the distances and odds are numeric constants not decoded.
- Which of `5100` / `5102` / `5103` the engine picks; what `5130` is (a second, one-frame volume late in a 2.4-2.7 s clip: a parry follow-up, not decided).
- The additive composition convention (Havok `ADDITIVE_DEPRECATED`: result = base combined with delta; the order of the quaternion product must be tested with `probe`).
- Sound and effect events (112 / 128 / 144 / 145 / 300 / 301 / 307 / 401) were not decoded.

## 7. Wrong ids in the older docs

`docs/animations.md`, `docs/attacks-swords.md`, `docs/enemy-*.md` and `docs/stamina-and-poise.md` are LLM-written (the straight-sword spec already says so) and their guard ids do not exist or are something else:
`a00_8050` "Guard Break" and `a00_8060` "Parried" have no hkx and no TAE entry; `a00_3030` "Riposte" does not exist (a00 has no 3xxx clip; `a23_3030` is a 4.33 s one-handed move with a judge-0 hit);
`a00_0500` "Parry Stance" is a 23-frame forward locomotion loop (root z -2.69 in 0.77 s = 3.5 m/s, no guard flag, left item low); `a20_3400` "Kick / Guard Break" is the dagger **backstab** (129 f, 4.27 s); the parry window "frames 12-24" is wrong (the data: frames 0-7 at 30 fps).
Use this report's tables.

## 8. Recommended minimal design

Goal: Dark Souls feel with the least new machinery, reusing what exists (`Action` state machine, `PlayerHurt` -> `hurt::suffer`, `Reel`, `Shove`, `HitStop`, the stamina bar, the creature `Brain`, the extractor's retarget and gates).

**Step A (block, no shield):** a held button (LB / L1 / Shift) puts the player in `ActionState::Guard`: two-hand stance clips `a10_0100` (raise 17 f; guard counts from 0.20 s), `a10_0001` (hold, 61 f loop), `a10_0101` (lower 21 f);
movement slowed to walking, no sprint, stamina regeneration paused while up. In `suffer`: a blow from the front half (the cone is the port's choice, `guardAngle` is 0 in the data) is blocked when the guard is up: damage x (1 - 0.50), stamina - `guardAtkRate` x 0.68, a `Shove` by the blow's knock-back and a short hit-stop, a spark and the weapon-on-weapon sound;
`disableGuard` attacks (the kobold `pounce`) ignore it; stamina to 0 plays the guard break (`a00_2080`-`2083`, a full-body clip the existing `Reel` already knows how to run) and the poise/stagger rules apply.
The guard-hit overlays (`2060`-`2077`) are additive: for the first version skip them and let shove + hit-stop + spark carry the hit (their amplitude is 0.15 m); bake them later by composing each over the guard-hold frame in the extractor.
Why the two-hand set and not the shield set: it is the only form of "block with the sword and no shield" the data has, and it needs three absolute clips.

**Step B (parry, no shield):** a button (LT / L2 / RMB-hold alternative) plays `a42_5100` (36 f), extractor-retargeted as any absolute clip; a sphere of radius 0.55 m on the left hand (the geometry of dummy poly 131 was not read) is active for frames 0-7 (0.233 s) and costs 14 stamina.
A creature inside it that is in its wind-up (after `Telegraph`, before its hit window) and whose species carries a parried clip (kobold-kit species, thug, Garrick; not the wolves) goes into `a00_2100` (79-81 f, locked for about 1.83 s) and its swing is cancelled.
The bare-hand parry is a design choice from the data (row 900000 + clips exist), the species flag and the timing rule are the port's own, because the files do not decide them (section 6).

**Step C (riposte / backstab):** R1 within the `ThrowParam` box (1.5 m = 1.64 yd, angle 90-180 deg for a riposte on a creature in its parried window; 0-60 deg behind for a backstab, vertical +0.5 / -0.65 m) plays `a23_3201` (162 f) / `a23_3400` (129 f) on the player and `a00_2510` / `a00_2500` on the victim (kit clips of c2060 / c2520 / c2400),
damage at the player's clip time 0.83 s: `LIGHT_DAMAGE` x 2.5 (riposte, 50) or x 2.0 (backstab, 40), stamina 20. The player is locked until the clip's cancel flags open, 4.60 s (riposte) / 3.33 s (backstab), not for the whole 5.4 / 4.3 s. The pair needs aligning (the victim turns 165-180 deg and travels 2.1-2.5 m in `2500`; the engine glues it to dummy 230 / 231, which we lack): start with the attacker placed 1.0 yd in front of / behind the victim and the victim's root motion applied as measured.

**Step D (enemies):** the thug and Garrick get the guard (`9910` / `9900` / `9920`, damage cut 75) and the parry (`a00_3100`, volume 0.8 m, frames 0-7) and, after a successful parry of the player, the riposte throw (`3101`, player victim `a00_9350` / `a00_9010`, about 43 / 50 damage): the heaviest part, deliberately last.

**Later (items):** a shield bought from Godric Rothgar maps to Buckler / Small Leather Shield (cut 76-80, stability 40) and adds the one-hand guard (`a00_0104` / `0005` / `0105`, shield on the WoW shield attachment).

## 9. Work steps (rough; S about half a day, M about a day or two, L several days)

1. **Extractor params (S):** `params.rs` accessors: `weapon_guard(id)` (cut rates, `staminaGuardDef`, `guardLevel`, `guardmotionCategory`, `enableGuard`, `enableParry`), `npc_guard(id)`, `atk_guard(variation, judge)` for both Atk tables (`guardAtkRate`, `atkStam`, `disableGuard`, `throwFlag`), `throw(def_chr, throw_type)`, `knockback(guard class)`; tests like the existing install tests (values in section 3).
2. **Player survey command (S):** a `survey-player --set aXX --ids ...` (or let `survey --chr c0000` open the `c0000_a*.anibnd` archives), printing frames, root, flags and type-1 events with first int; this report's tables are its expected output. `moveset.rs` gains a guard record (raise / hold / lower / break clips), a parry class (fists) and the throw pair (riposte, backstab), each with a `check` against the params as `Class::check` does.
3. **Block clips through the retarget (M):** `a10_0100`, `a10_0001`, `a10_0101`, `a00_2080`-`2083` with `probe` (head push, foot gap), cache manifest lines `guard ...` / `breaks ...`, `EXTRACTOR_VERSION` bump, new gates; the held sword's grip (`grip` line) needs its own fit for the two-hand pose, the existing one is fitted to the one-hand `a23` moves.
4. **Game: guard state (M):** `Press::Guard` (held, not buffered), `ActionState::Guard`, the intercept in `hurt::suffer` (front arc, damage cut, stamina cost, shove, break -> `Reel`), HUD (the stamina bar already drains), sounds, `WOWSOULS_GUARD` scripted press and a live check against a still creature.
5. **Parry (M):** extractor `a42_5100` (+ `ParryVolume` in the manifest: radius, frames, stamina); game `Press::Parry`, a swept sphere against the `Hurtbox`/`BodyHits`, `Reaction::Parried` with the kobold / thug / Garrick `a00_2100` clips (extractor: three more kit clips, `Tier` rules), species flag `parryable`, cancel of the swing (`Brain`), tests for the window.
6. **Additive guard-hit overlays (M, optional):** compose `a00_2060`-`2077` over the guard-hold frame in the extractor (needs the product order verified), or keep the procedural hit.
7. **Riposte and backstab (L):** the pair alignment, the victim clips as kit extraction for each creature, `Brain` states `Parried` / `Thrown`, damage events at the clip time, tests for the box.
8. **Enemy guard and parry (L):** `pattern.rs` entries (when a thug guards, when it parries: from `Odds_Guard`, `Parry_Act` constants if decoded, otherwise tuned live), the guard clips and `a00_3100` per kit, the player-as-victim clips `a00_9350` / `a00_9010` (from `c0000_c2520.anibnd.dcx` and `c0000_a00_*`), the half-bar riposte.
9. **Shield item (M, with the items phase):** Buckler row mapping, shield on the WoW attachment, one-hand guard clips.

Verification for each step stays the project's: the three gates, the install-untouched check, and unattended live runs with the numbers above as expectations.
