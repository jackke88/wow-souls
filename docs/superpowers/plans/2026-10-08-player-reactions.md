# D3 — player reactions and poise Implementation Plan

> **For agentic workers:** Execute inline (the user's usual choice), task by task, checkboxes as you go.
> Spec: `docs/superpowers/specs/2026-10-08-player-reactions-design.md`. Always `-j 4`. Install tests need
> `DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data"` exported in the same
> shell command. Nothing is committed (the user commits).

**Goal:** A creature's blow on the player plays the player's own Dark Souls flinch / knockdown / death clip,
held for the clip's TAE control windows; poise decides whether a blow flinches at all.

**Architecture:** `dsr-cache` retargets the candidate `a00` clips onto `HumanMale` (the roll's pipeline), measures each
head push and reads the f4 / f26 / f11 / f8 windows into `preact` / `pdeath` manifest lines (`wscl-cache 5`). The game
reads them into `PlayerReacts`; `ActionState::Reel` carries a clip index and a `ReelSpec` (windows like a swing's);
`dsclip::pose` poses `ClipId::React` / `ClipId::Death`; `hurt::suffer` picks the clip by blow level and direction and
applies the player's `Poise` (the kobold's `monster::Poise`). Without the lines everything falls back to today.

**Tech Stack:** Rust 1.98, Bevy 0.18.1, `dsr-cache` (GPL extractor), `wscl`, `wow-souls`.

**Looking before locking:** the level -> family table (`reaction::LEVEL_CLASSES`) and the death clips are tuning choices
the contact sheets decide (Task 5); the first build uses S / M / L / Down2 and the 22xx deaths.

---

## Task 1: Extractor — the reaction kit

**Files:** create `crates/dsr-cache/src/reacts.rs`; modify `main.rs` (mod), `monster.rs` (`best_by_push` is in the game, not here),
`cache.rs`.

- [ ] `reacts.rs`: `pub struct React { pub class: &'static str, pub clip: &'static str }`;
  `REACTS`: class `s` = `a00_2000`-`2003`, `m` = `2004`-`2007`, `l` = `2010`-`2013`, `down` = `2014`-`2017`, `down2` =
  `2024`-`2027`; `DEATHS`: `a00_2200`-`2207`, `2210`-`2213`; `pub fn clips() -> Vec<&'static str>` (every clip once);
  test: 20 reacts + 12 deaths, distinct clips, all start `a00_2`.
- [ ] `cache.rs`: `MANIFEST_VERSION` 4 -> 5; `fn extract_player_reacts(player, skel, out, worst) -> Result<String>`:
  for each clip `extract_clip(player, skel, &HUMAN_MALE_RIG, clip)`, `write_pair(out, "human_male", "c0000", ..)`,
  `monster::push(&e.r, head, pelvis)` with the DS bones `Head` and `Pelvis`; the first open time of flags 4, 26, 11
  (`wscl::FLAG_CHAIN`, `FLAG_EVADE`, `FLAG_MOVE`; an `ensure!` that all three exist) and the first `FLAG_INVINCIBLE`
  window (`0 0` when none). Manifest lines
  `preact <class> <clip> push <fwd> <left> chain <s> evade <s> move <s> down <from> <to>` and
  `pdeath <clip> push <fwd> <left> lies <yes|no>` (`lies` = `monster::check_death` passes: a death that stays standing is
  a flinch twin). Called from `extract` after the monster kit; its lines go into the manifest.
- [ ] Run `cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- extract`; read the printed summaries and the new manifest
  lines: which clips pass the retarget checks, their pushes (the four of a family should differ), the flag times, which
  `22xx` lie. A clip that fails a retarget check is reported with its reason; relax the check for that class only when
  the cause is the pose itself (lying down), not a wrong bone map, and say so in the spec.
- [ ] Install test: the table's lengths and flag times (`a00_2000` 0.833 s, chain 0.60, evade 0.60, move 0.77; `a00_2004`
  1.90 / 1.03 / 1.07 / 1.07; `a00_2014` `down` 0.00-3.60) within one frame.

## Task 2: The game's kit

**Files:** create `crates/wow-souls/src/reaction.rs`; modify `main.rs`, `boot.rs` (`reaction::plugin`),
`combat/action.rs` (`ReelSpec`), `monster.rs` (`best_by_push` -> `pub(crate)`), `dsclip.rs` (`CACHE_VERSION` "5",
`read_clip` already `pub`).

- [ ] `ReelSpec { duration, chain_from, evade_from, move_from, invuln: Option<(f32, f32)> }` in `combat/action.rs`.
- [ ] `reaction.rs`: `ReactClip { class: String, wow, ds, push: Vec2, spec: ReelSpec, travels: bool }` (`travels` = the clip's
  root motion is over 0.3 yd), `DeathClip { wow, ds, push, lies }`, `PlayerReacts { reacts, deaths }`;
  `parse(text, dir) -> Result<PlayerReacts, String>` (the `preact` / `pdeath` lines, clips from `human_male/` and `c0000/`,
  equal bone counts, `ReelSpec` from the flags and `wow.duration()`); `LEVEL_CLASSES: [(u8, &str)]` and
  `class_for(level)`; `choose(level, push) -> Option<usize>` (the class's clips, the one whose push points most like the
  blow's, via `monster::best_by_push`); `choose_death(push)`; `startup` loads it like `monster::startup` (a stale or missing
  manifest -> one warning, no resource).
- [ ] Unit tests with a temp-dir cache (like `monster`'s): a whole kit loads with its specs; a missing `preact` set is an
  error; `class_for` by level; `choose` picks the front / back clip by push; `choose_death` likewise.

## Task 3: The action machine

**Files:** `combat/action.rs` (+ its tests), `combat/mod.rs`, `hud.rs` / others the compiler names.

- [ ] `ActionState::Reel(Reel)`, `Reel { t, clip: usize, spec: ReelSpec }`; `Action::reel(clip, spec)`; `Action.death:
  Option<usize>` and `Action.dead_t: f32`, `Action::die_with(clip: Option<usize>)`; `step_action` advances `Reel` and the
  death clock; `accepts`: a reel takes a buffered attack from `chain_from` and an evade from `evade_from`; the pushed-stick
  `free` test from `move_from`; `over` at `duration`; `Action::invulnerable()` also inside `spec.invuln`.
- [ ] Tests: a buffered attack waits for `chain_from` and then starts; an evade waits for `evade_from`; a pushed stick
  ends it at `move_from`; it ends at `duration`; `invulnerable()` only inside `invuln`; a stagger started during a swing
  breaks it (existing behaviour for `Stagger`, repeated for `Reel`); the death clock runs only when dead.
- [ ] `combat::act`: a `Reel` carries the body by its clip's root motion (the clip's own `root_motion` x / z delta rotated by
  the body, `root_step`), only for `travels` clips; elsewhere the intent is idle.

## Task 4: Posing and hurting

**Files:** `dsclip.rs`, `dsview.rs`, `hurt.rs`, `npc.rs` (`PlayerHurt.poise`), `controller.rs` (`face_motion`), `avatar.rs`
(`Poise` on the avatar), `monster.rs` (`Poise` as a `Component` if it is not one).

- [ ] `ClipId::React(usize)` and `ClipId::Death(usize)`; `playing()` maps `Reel` and a dead `Action` with `death`;
  `clips_of(roll, moves, reacts, id)`; `pose` takes `Option<Res<PlayerReacts>>` and clamps a death to its last frame (the
  existing `sample` does). `dsview::resolve` knows a reaction's clip name (`WOWSOULS_DSVIEW=a00_2000`).
- [ ] `PlayerHurt` gains `poise: f32` (the blow's `atkSuperArmor`; the old procedural kobold 0).
- [ ] `hurt::suffer` with the kit: `Poise::hit(hurt.poise)` false -> health and nothing else (`poise holds` log);
  else `choose(level, push)` (push from the blow's direction in the player's frame, as `npc::react`), `Action::reel`,
  the shove only when `!travels`; lethal: `choose_death(push)` -> `die_with`; no WoW `play_over` when a DS clip plays;
  the old path (WoW wound, `Action::stagger`) without the kit. `PLAYER_POISE` 0 and `WOWSOULS_POISE=<n>`; a respawn
  refills it and resets `Action`.
- [ ] `face_motion` does not turn a body in a `Reel`.
- [ ] Log line: `player: struck for N by <e>, reaction <clip> (<class>, level L, push P), control in X s, poise a/b`.

## Task 5: Look, verify, document

- [ ] Contact sheets of every class and direction with `WOWSOULS_DSVIEW=<clip>` and the evidence shot method
  (`docs/evidence/react-<clip>-<t>.png`), a montage per class; read them, fix the table (`LEVEL_CLASSES`, which `22xx` is a
  death) and re-extract; send them to the user.
- [ ] Live, unattended, forced kobold moves (`WOWSOULS_MOVE=small_1|big|body_check`, still player, kobold 2-3 yd ahead /
  behind): reaction clip by level and side; the measured time until a scripted `_ATTACK` / `_ROLL` is accepted against
  the table; the knockdown's travel and invulnerable window; death plays and holds, `Respawned` resets;
  `WOWSOULS_POISE=100` leaves the player standing; `WOWSOULS_DSR_CACHE=/nonexistent` runs the old stagger.
- [ ] Gates: fmt, clippy `--workspace -D warnings`, `cargo test -j 4 --workspace` with the install env; installs identical
  before and after.
- [ ] Docs: spec "Built" note, README (`WOWSOULS_POISE`), HANDOVER (module rows, "Latest" block, counts), memory.
