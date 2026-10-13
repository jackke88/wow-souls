# Straight-sword moveset: design

Date: 2026-10-07. Sub-project C of the Dark Souls integration. A (data reader) and B (roll retarget)
are done: `2026-10-07-dsr-data-and-roll-retarget-design.md`. D impact, E enemy movesets, F DS HUD,
G playable Northshire, H WoW abilities keep their own specs.

## Goal

Replace the procedural light attack with Dark Souls Remastered's own straight-sword moves, played on
the WoW human from the install's animations, with the animations' own timing and the game's own
params: the one-handed R1 chain, R2, and the rolling attack. The moves chain, cancel and cost
stamina the way their data says. Also make the Xbox controller work, and give every combat action a
keyboard key, so the slice can be tested from the keyboard.

## Success criteria

1. Per move (table below), what the game plays equals the data: clip length, hit window, the three
   cancel times (f4, f26, f11) and lunge distance. Asserted by tests against the extracted clips
   (`DSR_DIR` set) and by live trace (`WOWSOULS_TRACE`): the swing's `Active` phase opens and closes
   at the hit window to one 30 fps frame; the body travels the clip's root motion scaled to the model
   (±2 %).
2. Stamina per move equals `BehaviorParam_PC.stamina` and damage per hit equals the base hit (20)
   times `AtkParam_Pc.atkPhysCorrection / 100` (table below). The extractor fails, naming the row, if a
   lookup is missing.
3. Chain rules (below) hold under scripted presses.
4. The look, side by side: `WOWSOULS_DSVIEW=<clip>` plays the DS skeleton and the retargeted human
   in lockstep for any cached clip. Numbers: the extractor's retarget checks (direction drift,
   cross drift, foot gap) pass on every move, and the WoW blade stays within 15° of the DS weapon's
   axis on every frame of every move after one constant grip correction per weapon class. The user
   judges the look.
5. With no cache the game plays exactly as today (procedural swing and roll), logging one warning.
   Today's tests stay green.
6. Controller: with a pad connected the log says `gamepad: connected <name>` and the pad drives
   move, camera, lock, roll, jump, R1 and R2 (the user verifies: Windows gives pad input to a
   focused window only). With none connected nothing changes. Done for everything but R2.

## Non-goals here (parked, own specs)

Running attack (needs a sprint button), two-handing and the 4000-series, jump and plunge attacks,
backstab and riposte, left-hand attacks, guard and parry, DS idle/walk/run (locomotion stays WoW),
other weapon classes (the move table is per class, so they are data-only additions later), DS impact
(hit stop, knock-back, stagger levels; D), poise. No change to benilla.

## Facts (measured 2026-10-07 on the user's install)

- **Correction.** `a20` is the dagger set, not straight swords. `EquipParamWeapon.wepmotionCategory`
  20 holds ids 100000-104000 (variations 2000-2004). Straight swords are category 23: ids 200000
  Shortsword (2.0 kg, model `parts/WP_A_0207`), 201000 Longsword, 202000 Broadsword, …, variation
  2300 (Broadsword 2301); animation set `a23`. The plan-A+B text and the LLM-written
  `docs/animations.md` / `attacks-swords.md` that call `a20_3000` the sword R1 are wrong.
- Animation ids: `a<category>_<hand + action>`; hand 3000 = one-handed, 4000 = two-handed, 5000 =
  left; action 000-002 R1 chain, 300/301/310 R2, 400-403 backstab, 500 running, 600/640 jump,
  800/801/810 plunge, 201-203/980 riposte, 900/940 post-evade (Souls Modding wiki, "Attack
  Animation Schema"; consistent with the data: `a23_3500` lunges 3.75 m, `a23_3900` is a short
  post-roll swing, `a23_3201` a 5.4 s riposte).
- TAE: a type-1 event is an attack's hit window; its third parameter int is the behavior judge id
  (3000/3001/3002 → 0/10/20, 3300 → 100, 3310 → 110, 3900 → 540). Type-0 events are `ChrActionFlag`
  windows. On attacks the cancel flags open staggered (f4 ≤ f26 ≤ f11), on rolls all at once;
  read as attack input, evade, movement (inferred, not documented; see Risks).
- Params (PARAM + PARAMDEF, format 104): `BehaviorParam_PC` row = (`variationId`,
  `behaviorJudgeId`) → `refId` (row in `AtkParam_Pc`) and `stamina`; `AtkParam_Pc.atkPhys` is 0
  throughout, the multiplier is `atkPhysCorrection` (percent). In practice the row id is
  `100_000_000 + variation × 1000 + judge` (variation 2300, judge 10 → 102300010); the lookup
  matches on the two fields, not on that formula.
- The existing extractor already retargets all six moves: direction drift 0.06°, cross drift
  2.65-3.15°, foot gap ≤ 0.059 yd (`--roll a23_3000` probe, scratch output).

The moves (clip length, hit window, f4 / f26 / f11 in seconds into the clip; lunge in yards at the
model's scale):

| move | clip | judge | length | hit window | f4 / f26 / f11 | lunge | stamina | damage × |
|---|---|---|---|---|---|---|---|---|
| R1-1 | a23_3000 | 0 | 2.500 | 0.533-0.667 | 0.767 / 0.900 / 1.033 | 1.98 | 20 | 1.00 |
| R1-2 | a23_3001 | 10 | 2.233 | 0.467-0.567 | 0.733 / 0.900 / 1.000 | 1.42 | 20 | 1.00 |
| R1-3 | a23_3002 | 20 | 2.500 | 0.467-0.600 | 0.767 / 0.900 / 1.033 | 1.98 | 20 | 1.00 |
| R2 | a23_3300 | 100 | 2.533 | 0.733-0.867 | 1.300 / 1.300 / 1.367 | 1.98 | 45 | 1.35 |
| R2 second | a23_3310 | 110 | 1.900 | 0.333-0.467 | 0.933 / 0.933 / 1.200 | 0.94 | 35 | 1.25 |
| rolling attack | a23_3900 | 540 | 2.100 | 0.233-0.333 | 0.700 / 0.767 / 0.867 | 1.42 | 20 | 0.90 |

`a23_3301` has the same judge and hit window as `a23_3300` and is not used.

## Architecture

```
crates/
  wscl/            clip format v2: + hit windows (judge, start, end)
  dsr-formats/     (GPL) + param.rs: PARAMDEF v104 and PARAM → typed rows; tae.rs: + attack judge
  dsr-cache/       (GPL) + moveset.rs: the hand-made move table of a weapon class
                          + extraction of every move, stamina and multiplier from params
  wow-souls/       the game
    combat/          combat.rs split into mod.rs (plugin, systems, instruments) and action.rs (the
                     pure action machine and its tests), moved first with no change in behaviour;
                     the moveset work then lands in action.rs
    dsclip.rs        loads roll + moves; plays any cached clip on the rig
    dsview.rs        side-by-side for any clip
```

`dsr-formats` knows no WoW and no Bevy; `dsr-cache` is the only reader of the DSR install; the game
reads only `cache/dsr/` (GPL stays in the extractor, as in A+B).

### Extractor

- `param.rs`: reads a PARAMDEF (header 0x30; fields 0xB0 bytes: names, types, byte count; bit
  fields and enums by byte count) and a PARAM (row table of id, data offset, name offset; rows laid
  out by the paramdef). Format 104 only; anything else is an error naming the file.
- `moveset.rs`: a class record: name `straight_sword`, weapon row 200000, motion category 23,
  variation 2300, and the moves `r1_1 r1_2 r1_3 r2_1 r2_2 roll_attack` with their clip names. At
  start the extractor checks the weapon row's `wepmotionCategory` and `behaviorVariationId` against
  the record, so a wrong class cannot pass silently.
- `cache.rs`: for each move, decode, retarget (the existing pipeline), apply the existing quality
  gates, write `human_male/<clip>.wscl` and `c0000/<clip>.wscl`; read the TAE type-1 judge; look up
  stamina and multiplier. The manifest (one file, written last, so one atomic switch) gets a
  `class <name>` line, one `move <name> <clip> judge <n> stamina <n> mult <x>` line per move and
  one `grip <x> <y> <z> <x> <y> <z>` line (see Weapon grip). The roll is extracted as before.
  `EXTRACTOR_VERSION` 3, `wscl-cache` 2: an old cache is ignored with one warning.

### Game

- **Timing as data.** A swing carries `SwingSpec { duration, hit: (start, end), chain_from,
  evade_from, move_from, cost, mult }` in seconds. The procedural swing is the same struct built from
  today's fractions (`ACTIVE_FROM/TO`, `CANCEL_FROM`, `ATTACK_COST`), so the action machine has one
  code path and a test pins the fallback to today's phase boundaries.
- **Presses.** `Press::{Attack, Heavy, Evade}`. Attack = R1, Heavy = R2.
- **Transitions** (a press waits in the 0.35 s buffer until the state accepts it):
  - idle: R1 starts R1-1; R2 starts R2.
  - in a swing, from its f4: R1 starts the next R1 of the chain (R1-1 → R1-2 → R1-3 → R1-1; from
    an R2 swing it starts R1-1); R2 starts R2 (from R1-n), R2-second (from R2) or R2 (from
    R2-second).
  - in a swing from its f26: evade → roll or backstep as now.
  - in a swing from its f11: a pushed stick ends it (the movement cancel).
  - in a roll (DS) or backstep once its cancel time has passed: R1 → rolling attack; R2 → R2.
  - the swing ends at the clip's length.
- **Playing.** The clip plays at 1.0× (the 1.25× WoW rate goes). The rig is posed in `PosePost` like
  the roll; the weight blends in over 0.1 s and out over 0.2 s; a chained move fades in over the last
  one's end pose. `anim::drive` already yields while an action owns the body.
- **Lunge.** Each frame's share of the clip's root motion goes into the motor as the roll's does, along
  the body's facing, recoil included (a step back is a negative speed along the facing, so the body
  does not turn: R1-2 steps back 0.16 yd after its thrust and ends 1.42 yd on, as the clip does). The
  motor stops at bodies. The old stop-gap rule (`enemy_in_reach`) stays for the procedural swing only.
- **Hits.** `strike` sweeps the blade while `swing.t` is inside the hit window; damage is
  `LIGHT_DAMAGE × mult`; each swing gets a serial so a chained or looped move can strike a target
  again.
- **Stamina.** Spent at the move's start by its `cost`. Maximum and regeneration stay as they are.
- **Inputs.** The pad fix and the keyboard keys were done first, on the user's request (2026-10-07):
  - Done: the pad works in principle (`boot.rs` re-enables benilla's disabled `GilrsPlugin`; the
    bottom-left help line shows `pad: <name> (last RB)`), F = light attack, arrow keys = camera.
    Space = roll, Q = lock, Shift+Space = jump, as before.
  - With this spec's R2 move: RMB = R2 (only with the cursor captured, as LMB), R = R2 on the
    keyboard, RT = R2 on the pad.
  - Pad map: RB = R1, RT = R2, B = roll, A = jump, RS click = lock, left stick move, right stick
    camera.
- **Instruments.** `WOWSOULS_HEAVY=<s>[,every]` presses R2 on a schedule. `WOWSOULS_DSVIEW=<clip>`
  plays any cached clip (the value `1` keeps meaning the roll) and logs, once per complete loop with
  the body standing, `blade error: worst <w> deg ..., mean <m> deg` (the drawn WoW blade against the
  DS blade, both in the body's frame); it also draws the DS blade (pale) and its axis laid over the WoW
  sword (magenta). `WOWSOULS_TRACE` logs `combat: move <name> (<length>, hit <window>), stamina <n>`
  at each start and `combat: <name> moved <yd> yd in <s> s` at each end.

### Weapon grip

The retarget carries each hand's rotation but not the sword's pose in the hand: the WoW attachment
frame differs from DS's weapon dummy. Two halves fix it:

- The extractor writes, per weapon class, the DS blade axis expressed in the retargeted WoW
  right-hand bone's frame (a constant), and the same axis in the DS weapon bone's own axes. The
  `R_Weapon` bone sits unrotated in `R_Hand` (its local rotation is the identity in every frame of
  every move, measured) and the retarget carries the hand's rotation, so the axis is the same in
  every frame; the extractor checks that to a degree. The blade is the weapon bone's **+Y**,
  chosen from measurements, not from the weapon model (whose blade runs along -Y, flipped by an
  attach transform we do not read): the grip runs across the palm (the thumb is on +Y, the
  fingers split along Y, the weapon bone sits on the palm side), and over the hit windows of all
  six moves +Y reaches furthest in front of the hand (the thrust a23_3300: 1.59 m, against 0.20 m
  for -Y). The user's eye on the side-by-side is the last check; `WOWSOULS_GRIP` retunes it.
- The game, when it builds the held item, takes the item's own blade axis (the `$WTB` → `$WTT`
  markers in the attachment frame) and applies the minimal rotation onto that cached axis to the
  item's root. The twist about the blade stays as WoW authored it.

The correction applies only while a Dark Souls clip poses the rig, weighted by the clips' blend
weight, so WoW's own idle and gaits keep their authored grip. The check measures, per frame, the
angle between the WoW blade (markers through the held item root) and the DS weapon axis
(`R_Weapon` rotation in the DS clip × its blade axis). `WOWSOULS_GRIP=rx,ry,rz` (degrees about the
hand's axes) replaces the fit by a rotation tuned by eye in the side-by-side.

## Error handling

- Extractor: a missing install, an unreadable param, a missing row, a class record that disagrees
  with the weapon row, or a retarget outside the gates is a clear error naming file and row;
  moves already written stay valid, the manifest is written last.
- Game: a missing or stale cache, or a clip whose bone count differs from the rig's, is skipped with
  one warning; the procedural swing and roll remain.

## Testing

- `dsr-formats`: `param.rs` on hand-built PARAMDEF and PARAM buffers (a bit field, an enum, a
  `dummy8`, two rows); the TAE judge on the existing hand-built buffer; install test (`DSR_DIR`):
  the Shortsword row says category 23 / variation 2300 and the six judges resolve to the table's
  stamina and multipliers.
- `wscl`: v2 round trip with hit windows; a v1 file is refused.
- `dsr-cache`: install test asserting the table above (windows to 1 ms, lunge ±2 %).
- Game: `step_action` tests for every transition and gate (before f4, at f4, evade at f26, movement
  at f11, loop after R1-3, R2 chain, rolling attack, buffer expiry, stagger interrupts); the
  procedural spec reproduces today's `swing_phase` boundaries; the manifest tests of `dsclip`.
- Live (unattended, as in A+B): kobold 2 yd ahead with `WOWSOULS_ATTACK=3,0.9` (chain), `…HEAVY`,
  `WOWSOULS_ROLL` then an attack press (rolling attack); trace lines against the table; screenshots
  at hit frames; install before/after diff.

## Locality rule

As in A+B: everything read from or derived from the DSR install (cache, dumps, screenshots) stays on
this machine and is gitignored (`/cache`). Only code and hand-made tables (bone map, move table) are
source.

## Risks and fallback

- **Grip.** The sword may sit twisted in the hand. The blade check measures it; the fix is one
  constant per class, else a hand-tuned `WOWSOULS_GRIP`.
- **Reach and feel.** The R1 lunges 2 yd and commits for 0.77-1.03 s; evade opens 0.9 s into R1-1
  (today 0.44 s). This is Dark Souls' own timing, slower than the current prototype. The WoW short
  sword matches DS's Shortsword (row 200000); a longer model is a one-line swap.
- **Flag meaning and move order** (f4 attack, f26 evade, f11 movement; R1 loops after three; R2 then
  `3310`) are inferred from the data pattern and the Souls Modding docs, not from the player ESD. A
  wrong guess moves a cancel by at most 0.27 s (the f4 to f11 gap of the R1s); the user judges by
  feel.
- **Pose seams.** Moves start from and end in DS's stance; the 0.1 s and 0.2 s blends cover the jump
  to WoW's idle and gait. A visible pop would argue for DS locomotion (a later spec), not for
  changes here.
- **Param decoding** is ported from the format knowledge in SoulsFormats; every value the slice uses
  is cross-checked (judge → row → plausible stamina and multipliers).
- **Pad.** Button reads need a focused window and a person at the pad, so only the user can prove
  them; tests cover the mapping on mocked pads and the bottom-left help line shows what the game
  sees. The keyboard path works regardless.
