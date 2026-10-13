# D3 — the player's Dark Souls reactions and poise (design)

Date: 2026-10-08. Sub-project D3 of the A-H decomposition (D1 sound, D2a hit-stop / knock-back / shake and
D2b blood are built). The user's wish: "Dark Souls reactions and poise, and the Dark Souls HUD". The HUD is
sub-project F with its own spec afterwards. Scope chosen by the user: flinch + knockdown + death (not the
armor-style poise system). Nothing here is committed (the user commits).

## Goal

When a creature's blow lands, the player does what a Dark Souls character does: a small, medium or large
flinch by the blow's level and direction, or a knockdown with the get-up, held for as long as the clip's own
control flags say; at zero health the Dark Souls death clip plays and holds. Poise is a number: a blow that
does not break it hurts but does not flinch. Today the player staggers 0.5 s in WoW's `CombatWound` clip for
every blow and dies in WoW's `Death`.

## What the install says (surveyed 2026-10-08, player `c0000`, the `a00` set)

The player's reactions are in the same archives as the roll and the sword moves (`c0000_a00_lo/md/hi`), on the
same skeleton, so the existing retarget (`dsr-cache` `extract_clip` onto `HumanMale`) applies unchanged.

| family | clips | length s | root travel | what it is |
|---|---|---|---|---|
| S | `a00_2000`-`2003` | 0.83 | 0 | small flinch, four directions |
| M | `a00_2004`-`2007` | 1.9 | 0 | medium flinch |
| L | `a00_2010`-`2013` | 1.97 | 0 | large flinch |
| min | `a00_2020`-`2022` | 0.5 | 0 | the lightest |
| Down | `a00_2014`-`2017` | 4.27 / 4.0 | 5.6 | thrown down, lies, gets up |
| Down short | `a00_2024`-`2027` | 3.5 | 2.8 | knocked down |
| Down | `a00_2080`-`2083`, `2085`-`2088` | 2.3 / 4.5 | 1.1-1.4 / 3.4-3.9 | knocked down |

The four clips of a family are the four directions (front, back, left, right); which is which is measured from
the head's push, as for the kobold (`monster::push`), not guessed from the ids.

- **Control returns from the clip's own TAE flags**, the same three the player's moves use: `f4` (attack),
  `f26` (evade), `f11` (movement), first-open times: S `a00_2000` 0.60 / 0.60 / 0.77 of 0.83 s; M `a00_2004`
  1.03 / 1.07 / 1.07 of 1.9; L `a00_2010` 1.43 / 1.50 / 1.50 of 1.97; Down `a00_2014` 3.57 / 3.47 / 3.70 of 4.27;
  min `a00_2020` 0.43 / 0.43 / 0.47 of 0.5. The old fixed stagger is 0.5 s.
- **The knockdowns carry an invulnerability window**, `f8`: `a00_2014` 0.00-3.60 s, `a00_2024` 0.00-3.10 s,
  `a00_2085` 0.00-3.83 s, closed by `f24` (the get-up), so a body on the ground cannot be hit.
- **The deaths** are the flag-less clips `a00_2200`-`2213`, which mirror the damage families (+200, the same
  lengths 0.83 / 1.9 / 1.97, no flags, no root motion). `a00_9000` (5.9 s, with get-up flags) is a bonfire-style
  rise, not a death. Whether a `22xx` clip differs from its `20xx` twin is checked in the contact sheets; if it
  does not, the death is the large flinch clip held on its last frame.
- **The blows** (`AtkParam_Npc`, already in the kobold kit): `dmgLevel` 1 (`small_1`, `small_2`, `pounce`), 2
  (`big`, `run_strike`), 5 (`body_check`); `atkSuperArmor` (poise damage) 10 / 10 / 20 / 20 / 30 / 20.

## Design

### 1. Extractor: the player's reaction kit

- `dsr-cache/src/reacts.rs` (new): the hand-made class table: level -> family (1: S, 2: M, 3-4: L, 5+: Down)
  and each family's four clip ids; `class_for(level)`. (A tuning table: which family answers which `dmgLevel`
  is not in the params; the contact sheets and the user's eyes decide. The `Down` family is the first in the
  table above and may become a shorter one.)
- `cache.rs` `extract_player_reacts`: each clip is retargeted onto `HumanMale` like the roll (written under
  `cache/dsr/human_male/` and `c0000/`), its head push is measured (human head and pelvis bones), its TAE
  flags are read; manifest lines `preact <class> <clip> push <fwd> <left> chain <s> evade <s> move <s> down
  <from> <to>` (`down` the `f8` window, `0 0` for none) and `pdeath <clip> push <fwd> <left>`.
  `MANIFEST_VERSION` 5. Without the lines the game keeps today's stagger and death.

### 2. The kit in the game

- `reaction.rs` (new, `PlayerReacts` resource, parsed like `monster::MonsterClips`): per class the clips with
  their `ReelSpec { duration, chain_from, evade_from, move_from, invuln: Option<(f32, f32)> }` (from the flags
  and the clip's length), `choose(level, push)` (the class by level, the clip whose measured push points most
  like the blow's, as `monster::choose_react`) and `choose_death(push)`.
- `ActionState::Reel(Reel)` replaces the fixed `Stagger` when the kit is loaded: `Reel { t, spec, clip }`.
  `step_action` treats it like a swing's windows: a buffered attack is taken from `chain_from`, an evade from
  `evade_from`, a pushed stick ends it from `move_from`, else it ends at `duration`; `Action::invulnerable()`
  is also true inside `spec.invuln`. The old `Stagger` stays for the no-cache fallback.
- `dsclip.rs`: `ClipId::React(u8)` and `ClipId::Death(u8)` join `Roll` and `Move`; `playing()` returns the
  reaction's clip and time (a death clip holds its last frame); the same blend and fade layers as the moves.
- Root motion: a `Down` clip's travel drives the body like a lunge (`root_step`, recoil included) and replaces
  the D2a shove for that blow; flinch classes keep the shove (they carry no root motion).

### 3. Hurting the player

`hurt.rs` `suffer`: `PlayerHurt.poise` is new (the blow's `atkSuperArmor`). The player's `Poise` (the
`monster::Poise`: damage soaks, a break flinches and refills, rest recovers) starts at `PLAYER_POISE` 0 (naked,
so every blow flinches, as in Dark Souls; `WOWSOULS_POISE=<n>` to try a higher one). A blow that does not break
poise takes health and shows blood and shake but no flinch, no shove and no hit-stop change. Otherwise: choose
the clip from the blow's level and the direction it comes from (in the player's frame), `Action::reel(spec)`,
the clip posed by `dsclip`, health and blood as before. At zero health: `Action::die()` with the death clip. The
`HURT_STOP` of the no-cache fallback and everything else of D2a stay.

### 4. Switches, logs, fallback

`WOWSOULS_POISE=<n>`. Log line per reaction: `player: struck for N by <who>, reaction a00_20xx (<class>,
level L, push P), control in X s` and `poise a/b`. Without the cache lines: the old stagger and Death, one
warning, as for every earlier bump.

## Not in this sub-project

Armor-style poise values, hyper-armor frames in the player's own moves, poise recovery timing from the params,
guard and parry reactions (`a00_2050`-`2053`, `2100`), the player's rolled-into-a-wall reactions, ragdoll,
backstab / riposte reactions on the kobold, bonfire rise (`a00_9000`). The HUD (F) is its own spec.

## Verification

- Unit tests: the class table by level; `choose` picks the front / back / side clip by push; a `ReelSpec` from
  flags; `step_action` takes a buffered attack and evade at their times, ends at `move_from` on a pushed stick and at
  `duration`; `Action::invulnerable()` inside `invuln` only; a death holds its last frame; poise absorb / break; the
  manifest parse and a missing-line refusal. Install tests (`DSR_DIR` / `WOW_DATA`): the kit extracts with the table's
  clip lengths and flag times.
- Contact sheets (`WOWSOULS_DSVIEW=<clip>` side by side, the kobold's method): one per class and direction, sent to the
  user.
- Live, unattended: forced kobold moves on a still player: `small_1` -> S, `big` -> M, `body_check` -> Down with
  the invulnerable window, the travel, the get-up and control returning at the flag time; the measured stagger times
  against the table; a body check while down does not hurt; death plays and holds, respawn resets;
  `WOWSOULS_POISE=100` makes `small_1` leave the player standing; `WOWSOULS_DSR_CACHE=/nonexistent` runs the old
  stagger. Gates: fmt, clippy `-D warnings`, `cargo test -j 4 --workspace` with the install env, installs identical.

## Open for the user (their eyes and hands)

The look of each class, which family answers which blow level (the table is a tuning choice), and the feel:
the player is now helpless for 0.6 s (small), about 1 s (medium), 1.5 s (large) and about 3.5 s (knocked down)
where it was 0.5 s, as in Dark Souls. A shorter knockdown family is one table row away.

## Built (2026-10-08)

Plan `docs/superpowers/plans/2026-10-08-player-reactions.md`, executed inline. What the survey and the build changed
in the design above:
- **Deaths.** The flag-less `a00_2200`-`2213` do not retarget (every one fails the bone-direction check by 90-180
  degrees: they are no ordinary pose), so the death is a knockdown clip held on the frame where it lies: `a00_2026`
  (thrown back, a blow from the front) or `a00_2027` (thrown forward), held from 1.13 s / 1.40 s (the slide ends at 99 %
  of the travel, plus 0.3 s). The extractor writes `pdeath <clip> push <f> <l> hold <s>`; `lies` is gone.
- **Knockdown family.** The sideways knockdowns (`2014`/`2015`, `2024`/`2025`) turn the body 90 degrees (root yaw) and are
  left out until the game applies root yaw; the kit has `down` (`2016`/`2017`, 6.4 yd) and `down2` (`2026`/`2027`, 3.2-3.7
  yd). The level table uses `down2` for level 5 and up. A knockdown clip is chosen by the way it throws the body, a flinch
  by its head's measured push.
- **Flinch directions measured** (head push forward / left, a blow from the front pushes the head back = negative
  forward): S `2002` -0.43, `2003` -0.21, `2000` +0.19 / -0.14, `2001` +0.18 / +0.16; M `2007` -0.39, `2006` +0.28, `2004`
  +0.22 / +0.42, `2005` +0.22 / -0.43; L all four push the head back (-0.33 to -0.59, +-0.2 left).
- **Manifest** `wscl-cache 5`; `preact` lines carry the class, push, the first-open times of flags 4 / 26 / 11 and the
  `f8` window.
- **Game.** `reaction.rs` (kit, `class_for`, `choose`, `choose_death`, `player_poise`), `ActionState::Reel` +
  `ReelSpec`, `Action.death` / `dead_t`, `ClipId::React` / `Death`, `hurt::suffer` (poise, clip, shove only for flinches),
  `combat::reel_step` (a knockdown's travel through the motor), `face_motion` never turns a reeling body.
  `WOWSOULS_DSVIEW=<clip>@<s>` holds a player clip at a time for stills.
- **Live** (forced kobold moves, a still player): `small_1` -> `a00_2002` (control in 0.60 s), `big` -> `a00_2007` (1.03 s),
  `body_check` -> `a00_2026` (3.00 s, thrown 3.37 yd back, up again, blood trailing the flight); three `big` blows kill:
  `dies (a00_2026, holds from 1.13 s)`, respawn resets; `WOWSOULS_POISE=100` -> `poise holds (90/100): no flinch`;
  `WOWSOULS_DSR_CACHE=/nonexistent` -> the old stagger (one warning).
- Pictures in `docs/evidence/`: `react-down2-sheet.png` (the knockdown at +0.28, +0.63, +1.5, +3.1 s),
  `react-sheet-a00_2002.png` / `a00_2007` / `a00_2013` (small, medium, large flinch, the DS skeleton beside the human),
  `react-death-a00_2026.png`.
