# Enemy movesets: design (the kobold, from Dark Souls' c2060)

Date: 2026-10-08. Sub-project E of the Dark Souls integration. A (data reader), B (roll retarget) and C
(straight-sword moveset) are done: `2026-10-07-dsr-data-and-roll-retarget-design.md`,
`2026-10-07-straight-sword-moveset-design.md`. D (impact), F (DS HUD), G (playable Northshire) and H (WoW
abilities) keep their own specs. This spec takes D's enemy half: the flinch and death clips and poise as a
number.

## Goal

Replace the kobold's stand-in fight (WoW's `Attack1H` clip at 0.7x, one attack, a strike window measured by
hand, WoW's wound and death clips) with Dark Souls' own: the Blighttown ghoul's (`c2060`) melee moves
retargeted onto the kobold's rig and timed by their TAE windows, damage and poise from the install's params,
the ghoul's flinch and death clips as the kobold's reactions, and a hand-written pattern table that picks the
move. The duel then plays the way the player's side does since plan C.

## Success criteria

1. Per move (table below), what the game plays equals the data: clip length, hit window (to one 30 fps
   frame), lunge distance (the clip's root motion times the extractor's scale, ±2 %), damage
   (`atkPhys × K`) and poise damage. Asserted by tests against the extracted clips (`DSR_DIR` set) and by the
   live trace (`WOWSOULS_TRACE`).
2. The look, side by side: `WOWSOULS_DSVIEW=kobold:<clip>` plays the DS ghoul skeleton and the retargeted
   kobold in lockstep for any cached clip. Numbers: the extractor's retarget gates (`MAX_ERROR_DEG` 15,
   `MAX_CROSS_DEG` 15, `MAX_FOOT_GAP` 0.08) pass on every clip. The user judges the look; it is shown before
   any game wiring (step order below).
3. Patterns: with the kobold chasing, the trace shows moves picked from the table by distance band, never two
   at once, one `Telegraph` at each move's start, a cooldown between moves; a forced move
   (`WOWSOULS_MOVE=<name>`) plays that move on its next attack.
4. Reactions: the player's R1 (`dmgLevel` 1) plays a small flinch, R2 (`dmgLevel` 2) a medium one, the variant
   follows the hit direction; a kill plays the death clip and the pose holds; respawn and `npc::reset` work as
   today. Poise is a number (`superArmorDurability` 0, damage `atkSuperArmor` 0) and the rule that uses it is
   unit-tested, so a kobold with poise would not flinch from a weak hit.
5. `think_step`, `Telegraph`, `PlayerHurt` and `WOWSOULS_DODGE` keep working; a strike still passes through a
   roll's i-frames.
6. With no cache the kobold fights exactly as today (WoW clips, hand-measured window), with one warning.
   Today's tests stay green. The DSR and WoW installs are untouched (before/after listings).

## Non-goals here (parked, own specs)

The Defias thief and the wolf (a quadruped: a different retarget), a visible weapon in the kobold's hand, the
ghoul's head throw, plunge attack and feign-death attack, heavier flinch levels (`dmgLevel` 3 and up, launched
and knock-down clips: the player's moves only deal level 1 and 2), group tactics, DS idle/walk/run (the kobold
keeps WoW's stand, walk and run, as the player does), hit-stop tuning, sparks, camera shake, player poise (D).
Decompiling the Lua AI: a separate decision for the user; the pattern table stands in until then. No change
to benilla.

## Facts (measured 2026-10-08 on the user's install)

- **Which rig.** `NpcParam` rows carry Shift-JIS debug names, and each map's MSB (`map/MapStudio/*.msb`)
  binds a chr model to an `NpcParam` id (Enemy part type data, the fourth int). `msg/ENGLISH` names only
  bosses. So: c2060 = the Blighttown ghoul (NpcParam 206000 sword, 206001 spear, 206002 bare, 206003
  black-soul variant; HP 203, poise 0, hit height 1.5 m, radius 0.55 m). `docs/enemy-kobold.md` names `c2100`
  and `c2210` (which is a placeholder rig with no attacks): wrong. 165 enemy rigs were surveyed; the others
  that are small (c2940 baby skeleton 0.7 m with 1 attack, c2280 mushroom child 1.0 m with 3) or upright (c2500
  Hollow, c2540) fit worse. The ghoul's idle is hunched with bent legs, like the kobold's bind pose
  (`docs/evidence/enemy-survey-posture.png`, side-view stick figures).
- **The rig.** `c2060.anibnd.dcx`: `skeleton.hkx` (53 bones: `master`, `center`, `Pelvis`, `Spine`,
  `L Thigh`/`L Calf`/`L Foot`/`L Toe0`, `Spine1`, `Neck`, `Head`, `L Clavicle`/`L UpperArm`/`L Forearm`/`L Hand`,
  the `R` side, `R_Weapon` under `R Hand`), `taeNew/x64/c2060.tae`, 135 animation files `aXX_YYYY.hkx` where the
  TAE animation id is `XX × 10000 + YYYY`. All 135 decode (53-bone skeleton, unit rotations, track bones in
  range). The names have spaces (`L Thigh`), unlike the player's `L_Thigh`: the bone table is per DS skeleton.
  DS stands in a T-pose with -Z forward, Y up; root motion z is negative forward (`a00_3003` -3.17).
- **The moves** (sword kit, `animIdOffset` 0; the spear kit is +10000, the bare kit +20000, not used). The
  clip files are `a00_<id>`; judge → `BehaviorParam` row (variation 20600, judge) → `AtkParam_Npc` row:

  | move | anim | judge | length s | hit window s | lunge m | `atkPhys` | `atkSuperArmor` | `dmgLevel` | hit radius m |
  |---|---|---|---|---|---|---|---|---|---|
  | small 1 | 3000 | 100 | 2.867 | 0.767-0.933 | 1.59 | 138 | 10 | 1 | 0.30 |
  | small 2 | 3001 | 110 | 2.167 | 0.833-1.033 | 0.49 | 142 | 10 | 1 | 0.30 |
  | big | 3002 | 120 | 3.167 | 1.433-1.667 | 1.35 | 195 | 20 | 2 | 0.30 |
  | run and strike | 3003 | 130 | 3.167 | 0.767-0.967 | 3.17 | 186 | 20 | 2 | 0.40 |
  | body check | 3004 | 140 | 1.400 | 0.600-0.833 | 1.43 | 106 | 30 | 5 | 0.20 + 0.20 |
  | pounce and bite | 3006 | 160 | 3.333 | 2.067-2.167 | 3.37 | 124 | 20 | 1 | 0.60 + 0.60 |

  The row names say what they are (`小攻撃` small attack, `走って鉄槌` run and strike, `体当たり` body
  check, `かみつくために襲う` pounce to bite). The sword hit volume is a capsule between the weapon dummies 1
  and 11 (the hand and the blade tip); the body attacks use spheres at dummy 1 and 2 (the hands). The dummies
  live in the chr's FLVER, which is not read: see Hit volume. Left out: `a00_3401` (plunge, judge 170),
  `a00_9061` (feign death, judge 240), the head throw (judge 150, a bullet).
- **Flags.** Events of type 0 are `ChrActionFlag` windows: on the sword moves flag 7 opens 0.33-0.73 s in
  (1.13 s on the pounce) and closes at 2.0-2.7 s, flags 78/79/86 open late (62-95 % of the clip). As with the player's f4/f26/f11,
  their meaning is inferred (7 = the AI may chain, 78/79/86 = movement and evade allowed), not documented.
- **Reactions.** Damage clips by file: `a00_2000-2002` (0.6 s), `2004-2007` (1.0 s, four variants), `2010-2013`
  (1.333 s, four variants), `2030-2032` (0.5 s), `2014`/`2020` (launched, 4.0 / 3.0 s with 4.34 / 2.36 m of
  root motion), `2040` (knock-down). Which belongs to which `dmgLevel`, and the variant order (front, back,
  left, right is the Souls convention), is not verified. Death: `a00_9000`-`9004` START lying (pelvis 0.40 m up from frame 0: the feigned death, no death);
  the deaths are `a00_2200`-`2209`, which start standing (pelvis 0.91 m) and end lying (0.11-0.37 m): `2202`
  (2.8 s, ends on its back, head 0.75 m behind the pelvis, 1.14 m of travel backward) and `2205`, `2207`
  fall backward, `2200`, `2203`, `2204` (2.7 s) and `2209` fall on their face. `a00_2500`, `2510` and `9061`
  end in the idle pose (the feigned death's rise).
- **Params.** The player's moves deal `dmgLevel` 1 (R1 chain, rolling attack) and 2 (R2 and its follow-up) with
  `atkSuperArmor` 0 (`AtkParam_Pc`, variation 2300). `NpcParam` 206000: `hp` 203, `superArmorDurability` 0,
  `stamina` 100, `animIdOffset` 0, `aiThinkId` 206000. `NpcThinkParam` 206000: `logicId` 10000 (Lua),
  `nearDist` 2, `midDist` 5, `farDist` 10, `outDist` 20, `eye_dist` 20, `BattleStartDist` 20.
- **The kobold.** `Creature\Kobold\Kobold.m2` (display 365, scale 0.85): 72 bones in a WoW biped hierarchy
  (root 13, spine 14, waist 15, head 35 by key-bone ids 26, 4, 5, 6), 21 sequences (stand 0, walk 4, run 5,
  attacks 16/17, wounds 9/10, death 1, ...), no held item (`npc.rs` builds none). The attack today is clip 17.

## Architecture

```
crates/
  wscl/            clip format: unchanged (hit windows and flag windows exist since v2)
  dsr-formats/     (GPL) + param.rs: unchanged; reads NpcParam, BehaviorParam, AtkParam_Npc, NpcThinkParam
  dsr-cache/       (GPL) dsr.rs:      the loader takes a chr id (c0000 today; c2060 too)
                         bonemap.rs:  + the ghoul → kobold pair table, hips, legs, checks
                         wow.rs:      + the kobold M2's skeleton (generalised from `human_male`)
                         monster.rs:  the hand-made kit of a chr (moves, reaction families, death)
                         params.rs:   + the NPC lookups
                         cache.rs:    extracts the kit into cache/dsr/kobold/, manifest `monster` lines
  wow-souls/       the game
    monster.rs     (new) loads the monster half of the manifest; the kobold's clips and numbers
    pattern.rs     (new) the pure pattern table: `pick(table, range, last, rng) -> Move`
    npc.rs         attack from a clip, hit volume, reactions, poise; the old path stays as the fallback
    dsclip.rs      the shared pose/blend/layer code is lifted out so the kobold's rig uses it too
    dsview.rs      `WOWSOULS_DSVIEW=kobold:<clip>`
```

`dsr-formats` knows no WoW and no Bevy; `dsr-cache` is the only reader of the DSR install; the game reads only
`cache/dsr/` (the GPL stays in the extractor, as in A to C).

### Extractor

- `dsr.rs`: `load(dsr, chr)` reads `chr/<chr>.anibnd.dcx` (skeleton, TAE, animations; names compared without
  case: `skeleton.hkx` and `Skeleton.HKX` both occur). The player's extra archives (`c0000_a*`) stay the
  player's. The decode, root motion and events code is shared.
- `bonemap.rs`: `GHOUL_KOBOLD_RIG`, a `Rig` like `HUMAN_MALE_RIG`: the pairs (DS bone, aim bone, kobold bone
  index, aim index, check, align) for pelvis, spine, neck, head, both arms and legs, `hips` (the hip root and
  the two hip joints), `legs` (hip, knee, ankle: the scale), `checks` (cross axes, soles). The kobold's indices
  come from `benilla-extract m2bones Creature\Kobold\Kobold.m2` and the bone hierarchy, then the retarget's own
  gates (a wrong pair shows as direction or cross drift). Posture: the retarget keeps the DS pose's rotations
  as deltas from bind (as for the player); the ghoul's hunch comes with the clips, the kobold's rest hunch
  stays in its bind pose. The side-by-side decides whether that reads right.
- `monster.rs`: a kit record: `kobold`, chr `c2060`, `NpcParam` 206000, the moves (`small_1 small_2 big
  run_strike body_check pounce` with clips and judges), the flinch clips with their level, the death clips. At
  start the extractor checks the `NpcParam` row (`behaviorVariationId` 20600) and that each judge resolves to
  an attack row (`BehaviorParam` then `AtkParam_Npc`, an attack and not a bullet), so a wrong kit cannot pass
  silently.
- `cache.rs`: for each clip, decode, retarget (the existing pipeline), apply the existing gates, write
  `cache/dsr/kobold/<clip>.wscl` and `cache/dsr/c2060/<clip>.wscl`. Manifest lines, written last (one atomic
  switch):
  - `monster kobold chr c2060 npc 206000 hp <n> poise <n> scale <leg ratio>`
  - `attack <name> <clip> judge <n> phys <n> poise <n> level <n> radius <m>[ radius2 <m>]`
  - `react <level> <clip> push <forward> <left>` and `death <clip> push <forward> <left>`: where each
    clip throws the head, measured (below)
  - `think near <m> mid <m> far <m> out <m>`
  - `reach <hand bone> <forearm bone>`: the kobold bones the hit volume runs along (Hit volume).
  The player's `move` lines gain `level <n> poise <n>` (from `AtkParam_Pc`, for the kobold's reactions).
  `EXTRACTOR_VERSION` 4, manifest version 3: an old cache is ignored with one warning.
- Reaction numbers (not looks): for each flinch and death clip the extractor measures where the head goes
  (its largest deviation from its first frame relative to the pelvis, as forward and left in the
  creature's frame) and writes it as the clip's `push`; the game picks the clip of the blow's level whose
  push points most nearly the way the blow pushes the creature, so no variant order is assumed. For a
  death clip the extractor checks that the pelvis ends below 0.6 of its first height and moves less than
  0.02 yd over the last ten frames.
- Feet: the kobold's feet are twice as long as the ghoul's scaled ones (ankle to toe 0.24 against 0.12 model
  units), so toes cannot be compared; the ghoul's soles are its ankles (`bonemap::GHOUL.soles`), the contact
  height is the ankle height at rest (`Rig.contact_height`, 0.09), and the foot gate (0.08 yd) holds on the
  ankles. The kobold's toes sink a little in the lunges: anatomy, not retarget.

### Hit volume

The kobold holds nothing, so the sword capsule cannot come from a weapon, and the ghoul's weapon axis cannot
be measured (its sword sweeps sideways: every `R_Weapon` axis scores about 0 for "ahead", measured). The
volume is a capsule from the kobold's right-hand attachment (attachment 1) along its forearm (the right
hand bone minus the right forearm bone, posed) for `REACH_LENGTH` (0.55 kobold yd, a club's), radius
`max(hit0_Radius, hit1_Radius) × scale` (the move's, in kobold units). The length is tuned so that a move
connects at the player's centre distances the old kobold did (1.1-1.6 yd) and by eye. Hit test:
`foehit::touches` (the capsule swept from its last pose to this one, against the player's `Hurtbox` axis),
every frame of the hit window, once per move.

### Game

- **Timing as data.** `Mind::Attack` carries an `AttackSpec { name, length, hit: (start, end), progress,
  phys, poise, level, radius }` read from the cache. The procedural attack is the same struct built from
  today's constants (`KOBOLD_ATTACK`, `STRIKE_FROM/TO`), so the AI has one path and a test pins the fallback
  to today's boundaries. `think_step` takes the chosen attack's length.
- **Playing.** The clip plays at 1.0x in `PosePost` on the kobold's rig, blended in over 0.1 s and out over
  0.2 s, through the same pose and layer code as the player's roll and moves (lifted out of `dsclip.rs`). The
  kobold's gait clips yield while an attack owns the body (`Brain::owns_body`, as today).
- **Lunge.** Each frame's share of the clip's root motion, times the extractor's scale, goes into the kobold's
  `motor::step` along its facing, recoil included (negative steps), as the player's `root_step`. The kobold
  tracks the player until the hit window opens, then holds its heading (as `STRIKE_FROM` does today).
- **Damage.** `damage = round(atkPhys × K)` with `K = 25 / 138` so small 1 stays at today's 25 (small 2 26,
  big 35, run and strike 34, body check 19, pounce 22). The player's HP and the kobold's 60 HP stay.
- **Pattern.** `pattern.rs`: a table of rows (move, band, weight, cooldown) and a pure `pick` over the player's
  distance; bands start from `NpcThinkParam`'s 2 / 5 / 10 m and are tuned in kobold yards (`pattern::KOBOLD`). Starting table, from the move names: near: small 1 (weight 3, then small 2 and big as a chain from
  the move's flag-7 time), big (2), body check (1); mid: run and strike (3), pounce (1); far: close in. A
  small seeded RNG (`WOWSOULS_SEED`) keeps tests and unattended runs repeatable. The table is hand-made data
  at the top of `pattern.rs`; the Lua AI replaces it later without touching `npc.rs`.
- **Reactions.** `Reaction::Flinch` gains `{ level, variant }` and `Dead { clip }`. The `Hit` message carries
  the move's `level`, `poise` and the hit direction (player-to-kobold in the kobold's frame); `react` picks
  the family by `level` (1 small, 2 medium) and the variant by direction, plays it through the same pose
  code, and one of the two death clips (`a00_2202` on its back for a blow from the front, `a00_2204` on its
  face for a blow from behind, chosen like the flinch) on a kill. WoW clips 9, 10 and 1 stay as the fallback.
- **Poise.** `Poise { max, left, since_hit }` on the kobold. A hit subtracts the move's poise damage; the
  kobold flinches when `left` falls to 0 or below, then `left` returns to `max`; it also returns to `max`
  after 5 s without a hit. With `max` 0 (the ghoul's) every hit flinches, as today. Pure `poise_step`,
  unit-tested; the number comes from the manifest.
- **Instruments.** `WOWSOULS_DSVIEW=kobold:<clip>[@<s>]`: the DS ghoul skeleton (gold gizmo bones, 1.2 yd to
  the side of the kobold) and the retargeted kobold in lockstep, looping, or held at second `<s>` for stills. `WOWSOULS_MOVE=<name>` forces the next kobold attack, `WOWSOULS_SEED=<n>` seeds the
  pattern, `WOWSOULS_TRACE` logs `npc: move <name> (<length>, hit <window>), damage <n>` at each start and
  `npc: <name> moved <yd> yd in <s> s` at each end, plus `npc: flinch level <n> variant <v>` and `npc: poise
  <left>/<max>`.

## Order of work (the look first)

1. Survey notes into the repo and the extractor on c2060: loader with a `chr` argument, the pair table, the six
   moves and the reaction clips retargeted, the gates passing. Show the user `WOWSOULS_DSVIEW=kobold:<clip>`
   pictures of every move; the user judges the look before any gameplay wiring.
2. The attacks in the game (timing, lunge, hit volume, damage), one forced move at a time.
3. Reactions and poise.
4. The pattern table.
5. Gates, installs before/after, HANDOVER.

## Error handling

- Extractor: a missing install, an unreadable param, a missing row, a kit record that disagrees with the
  `NpcParam` row, a retarget outside the gates or a reaction check that fails is a clear error naming file
  and row; clips already written stay valid, the manifest is written last.
- Game: a missing or stale cache, or a clip whose bone count differs from the kobold rig's, is skipped with
  one warning; the procedural attack and WoW's wound and death clips remain.

## Testing

- `dsr-cache` (install, `DSR_DIR` and `WOW_DATA`): the six moves match the table (windows to 1 ms, lunge ±2 %
  of the DS root motion times the scale), the retarget gates pass, the NPC lookups return the table's
  `atkPhys`/`atkSuperArmor`/`dmgLevel`, the reaction and death checks pass.
- `wow-souls`: the manifest parser (`monster` lines, refusal of old versions), `AttackSpec` from a clip, the
  procedural spec pinned to today's boundaries, `pattern::pick` (bands, weights, cooldown, seeded repeat,
  no repeat of the last move when others fit), `poise_step` (max 0 always flinches; a poised kobold absorbs a
  weak hit and breaks on the second), `react` family and variant choice by level and direction.
- Live (unattended): `WOWSOULS_MOVE=<name>` for each move against the still player, damage and window from
  the trace; the player rolls at the telegraph (`WOWSOULS_DODGE`) and takes no damage; R1 and R2 flinches;
  a kill; respawn; a seeded fight; pictures at the hit frames. Install listings before/after.

## Locality rule

As before: everything read from or derived from the DSR install (cache, dumps, survey tables, pictures) stays
on this machine and is gitignored (`/cache`, the evidence folder). Only code and the hand-made tables (bone
pairs, the kit, the pattern) are source.

## Risks and fallback

- **Retarget onto a smaller, differently built rig.** The kobold is shorter and its limbs proportioned
  differently; the hunch of the ghoul's clips and the kobold's bind pose may add up wrong. The gates measure
  direction, cross and foot drift; the look is the user's call on the side-by-side. Fallback: a per-bone
  correction in the pair table, else a partial kit (arms and torso only).
- **Hit volume without a weapon.** The reach and the large DS radius (0.30 m on a 1.5 m body, scaled) may hit
  too early or too late; `REACH_LENGTH` and the radius scale are constants in `monster.rs` / `npc::strike`, tuned against the trace (every forced move connects from 3.4 yd (run_strike) and 3.2 yd (pounce) or closer).
- **Variant order and level mapping.** Front/back/left/right and "level 1 = small" are conventions plus the
  extractor's lean check, not documented; a wrong guess shows as a flinch away from the hit, fixed in the
  table.
- **Flags and chains.** Which flag lets the AI chain small 1 into small 2 is inferred; a wrong guess moves a
  chain by under a second.
- **Damage scale.** `K` makes small 1 equal today's hit; the others follow the DS ratios and the duel may feel
  harder or easier than today. One constant.
- **Lua.** The real choice logic is not read. The table is the stand-in and is replaced, not extended.
