# D2a — impact feel from the data (design)

Date: 2026-10-08. Sub-project D2a of the A-H decomposition (D1 sound is built). Scope chosen by the
user: hit-stop, knock-back and camera shake now; blood and sparks are D2b, after the user has felt
this. Nothing here is committed (the user commits).

## Goal

A landed blow feels like Dark Souls': the world holds for the time the move's params say, the struck
body is pushed back by the distance the params say, and the camera jolts when something heavy lands.
The numbers come from the install's params, like the rest of A-E. Where the data has nothing (camera
shake), the value is ours and named as a tuning value.

## What the data says (read from `AtkParam_Pc` / `AtkParam_Npc`, checked 2026-10-08)

| blow | `hitStopTime` s | `knockbackDist` m | `dmgLevel` |
|---|---|---|---|
| player R1-1 / R1-2 / R1-3 (judge 0 / 10 / 20) | 0.08 | 0.72 / 0.84 / 0.72 | 1 |
| player R2 / R2 second (judge 100 / 110) | 0.08 | 0.95 / 0.90 | 2 |
| player rolling attack (judge 540) | 0.08 | 0.80 | 1 |
| kobold `small_1` / `small_2` (judge 100 / 110) | 0.00 | 0.45 / 0.45 | 1 |
| kobold `big` / `run_strike` (120 / 130) | 0.00 | 1.00 / 1.00 | 2 |
| kobold `body_check` (140) | 0.00 | 1.37 | 5 |
| kobold `pounce` (160) | 0.00 | 1.00 | 1 |

- The sword's hit-stop is the same 0.08 s on every move: there is no "by weight" scale in the data, so
  none is invented. The kobold's blows have none: being hit does not freeze the world.
- The kobold's flinch clips (`a00_2000`-`2007`) carry zero root motion (measured), so `knockbackDist`
  is the only thing that moves a flinching body. No double count. The two death clips do carry root
  motion (0.70 and 0.42 yd) and are left alone.
- benilla's `camera_shake.rs` is crate-private (`benilla-app`) and plays only WoW's footstep and spell
  presets. The blow params have no shake value, so the shake below is our own and a taste choice.

## Design

### 1. Extractor: two more numbers per blow

- `params.rs`: `Behavior` and `NpcAttack` gain `stop: f32` (`hitStopTime`) and `knock: f32`
  (`knockbackDist`). Install tests pin the table above.
- `cache.rs`: the `move` and `attack` manifest lines end with `stop <s> knock <m>`
  (`move r1_1 ... level 1 poise 0 stop 0.08 knock 0.72`). `MANIFEST_VERSION` 3 -> 4. The clips do not
  change. The game reads the new fields as required; an old manifest is "stale" (the existing one-line
  warning, procedural fallback), as for every earlier bump. Rebuild:
  `cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- extract`.

### 2. Carrying the numbers

- `SwingSpec` (`combat/action.rs`) gains `stop` and `knock`; `SwingSpec::procedural` keeps today's
  feel: `stop = HIT_STOP` (0.07 s), `knock = 0.0`. `AttackSpec` (`monster.rs`) gains the same two.
- `Hit` (`hits.rs`) gains `knock` (its `stop` is used inside `strike`, so it does not travel).
  `PlayerHurt` (`npc.rs`) gains `level`, `stop`, `knock`; `suffer` finds the direction from the `from`
  entity's `Transform`, so no position travels. The kobold without a cache keeps today's `HURT_STOP`
  (0.05 s) and no knock-back, so the old fallback fight is unchanged.

### 3. Hit-stop from the data

`strike` freezes the world for `swing.spec.stop` real seconds; `suffer` (`hurt.rs`) for
`hurt.stop`. A value of 0 means no freeze at all (no call to `HitStop::freeze`). The speed stays
`HIT_STOP_SPEED` 0.05 and `HitStop` keeps the longest pending end. The fixed `HIT_STOP` constant
remains only as the procedural default.

### 4. Knock-back: a `Shove`

- New module `shove.rs`. `Shove { dir: Vec3, dist: f32, t: f32 }` is a component on the struck body
  (the kobold, or the avatar). `dir` is horizontal and unit: from the blow's origin to the body. `dist`
  is `knock * METRES_TO_YARDS` (1.0936). The slide takes `SHOVE_TIME` 0.30 s and eases out: the
  fraction covered is `p(u) = 1 - (1-u)^2`, `u = t / SHOVE_TIME`, so the push starts fast and settles.
  Pure `shove_step(t, dt) -> f32` returns the fraction advanced in a step; the steps of any frame
  rate sum to the same distance (unit test).
- `apply_shove` writes the body's `MoveIntent { dir, speed: step / dt }` while a `Shove` runs and
  removes it at `t >= SHOVE_TIME`, so the motor does the move and walls stop it. Ordering: the
  avatar's after `PlaySet::Intent` and before `PlaySet::Motor`; the kobold's after `think` and before
  `move_hostiles`. The shove advances in virtual time, so hit-stop holds it too.
- A shove is added when a non-lethal hit lands: in `react` (`npc.rs`) for a flinching kobold, in
  `suffer` for a staggered player. Lethal blows do not shove (deaths have their own motion). A new
  shove replaces a running one. A blow the player's i-frames let through never reaches `suffer`, so
  no shove either (already the case). A poise-holding kobold (`poise.hit` false) does not shove.
- `face_motion` turns a moving grounded body toward its move direction; it must skip a body that
  carries a `Shove`, or the knock-back would spin the player round.
- The direction for a kobold hit is `hit.from` (the avatar) to the kobold; for a player hit, the
  kobold's position to the avatar. Both flat.
- A `Respawned` and a kobold `reset` remove any `Shove`.

### 5. Camera shake (our own)

- New module `shake.rs`: `Shake { trauma: f32 }` resource, decaying in real time at `DECAY` 2.5 per
  second (about 0.4 s from a full kick; real time so hit-stop does not stall it). `kick(level)` adds
  the blow's trauma, clamped to 1.0. Pure `shake_offset(trauma, t) -> (Vec3, Quat)` is a sum of
  incommensurate sines scaled by `trauma^2`: at full trauma 1.2 deg of rotation and 0.04 yd of
  translation, deterministic (no RNG) so tests and runs repeat.
- Trauma per blow (tuning values, in `shake.rs`): hit on the player: level 1 = 0.35, level 2 = 0.55,
  level 5 and up = 0.9. A level-2+ blow the player lands: 0.25 (level 5+: 0.4). Level-1 blows the
  player lands: no kick.
- `camera::seat` adds the offset after it has placed the camera (the same point where the body fade is
  published), so the sweep and the follow smoothing are not disturbed.
- `WOWSOULS_SHAKE=0` turns it off; the unattended runs keep it on and log `shake: kick 0.35 (level 1)`.

### 6. Instruments and logs

- The `hit:` line gains `stop 0.08 knock 0.72`; the player's `struck` line gains the same.
- New line `shove: <who> 0.79 yd over 0.30 s` when one starts, and `WOWSOULS_TRACE=1` adds the body's
  position per frame while a shove runs (for the numeric check).

## Not in this sub-project

Blood and sparks (D2b), DS hit sounds (FMOD, separate extractor job), the player's own DS reactions
and poise (the player still plays WoW's CombatWound), shoves on death, shove tied to `weight`
(NpcParam `weight` 50 for the ghoul) or to the target's size, footsteps.

## Verification

- Unit tests: `shove_step` sums to 1 over any frame split and is fastest first; a shove ends and
  removes itself; the manifest parser takes `stop`/`knock` and rejects a missing one; `SwingSpec`
  and `AttackSpec` carry them; `shake_offset` is zero at trauma 0 and bounded at 1; the decay reaches
  0; the kick table; a zero `stop` does not call `freeze`. Install tests (with `DSR_DIR`) pin the table
  above.
- Live, unattended: a seeded fight logs every `hit:` with `stop 0.08`; a kobold struck by R1-1 at 2 yd
  from a still player travels 0.79 yd (0.72 m) over 0.30 s on flat ground
  (`WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8960,-136` is the flat spot; for a kobold shove the
  kobold is placed 2 yd ahead); the player hit by `small_1` is pushed 0.49 yd and `body_check` 1.50
  yd; the player is not turned by the shove (facing unchanged in the trace); `WOWSOULS_SHAKE=0` logs
  no kick; with no cache the old fight runs unchanged (`WOWSOULS_DSR_CACHE=/nonexistent`).
- Gates: `cargo fmt --all -- --check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`,
  `cargo test -j 4 --workspace` (with `DSR_DIR`/`WOW_DATA` exported in the same shell command), both
  installs identical before and after.

## Open for the user (their hands and eyes)

How far and how quickly a blow pushes (`SHOVE_TIME`, `METRES_TO_YARDS`), whether being hit without a
freeze reads right, and the shake: strength, or off. The values are named constants and tune in one
place.

## Built (2026-10-08)

Plan `docs/superpowers/plans/2026-10-08-impact-feel.md`, executed inline. Two findings while building:
- A shove must end one frame after its last step, with an idle intent: ending in the same frame left
  the last tiny step in `MoveIntent` after the `Shove` component was gone (an automatic sync point
  applies the removal before `face_motion`), and the body turned 0.20 rad toward the push. Fixed in
  `drive`; the trace line `controller: trace` now also prints `yaw=`.
- `WOWSOULS_SHAKE=0` stops the kicks, not the shove or the hit-stop.
