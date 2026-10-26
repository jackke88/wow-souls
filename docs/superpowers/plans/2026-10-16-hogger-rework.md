# Hogger rework (Artorias kit + impact) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hogger fights as Knight Artorias (`c4100`) on the gnoll body, aggressive with readable tells, with ground shocks, a 50 % Frenzy power-up and a 25 % pack call.

**Architecture:** A new extractor kit `hogger_knight` sits beside the Taurus kit `hogger`. The game species `HOGGER` switches to it; `HOGGER_TAURUS` (same species name, not in `ROSTER`) is chosen with `WOWSOULS_HOGGER=taurus`. Pattern tables and styles are keyed by **kit** name. New manifest lines carry the ground-shock and blast numbers from the source rows. The game reads them in a new `shock.rs` (pure hit rules + systems). Frenzy and the pack call extend `boss.rs`.

**Tech Stack:** Rust 1.98.1, Bevy 0.18.1, avian3d 0.6; `dsr-cache` extractor (DSR read-only); `.wsanim` project library in `anim/source`.

**Spec:** `docs/superpowers/specs/2026-10-16-hogger-rework-design.md` (§ numbers below refer to it).

## Global Constraints

- Commit only when the user asks (HANDOVER standing order; overrides the skill's "commit" steps: each task ends with the gates instead).
- Never `cargo fmt --all`. Gate: `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check`; `cargo clippy -j 4 --workspace --all-targets -- -D warnings`; install tests: `DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 --workspace`.
- Build with `-j 4`. Python hangs the Bash tool: use perl/awk/sed.
- Parallel chats edit files: re-read a file right before editing it; keep edits small.
- Tier `Boss`: at most 12 attacks. Blow bands from §2: medium ≈ 12-15 % of the level 11 trial player's health, a big slam ≈ 28 % at no Pierce Armor stacks (≤ 35 % at 3 stacks), the Frenzy blast ≈ 20 %.
- `tempo` 1.0 for the knight kit (the clips are fast already); recovery multiplier as found live.
- Effects are ground-quad decals or WoW spell models, never an `ActorModel` (memory: effect models).
- Judge animation in motion: dense frame series from the game camera + a front view, never stills.
- No new hardcoded machine paths in code.

## Review Focus

1. **A blow or shock that lands above the ground** (vertical root travel of the flips): the shock's centre must be at ground height under the impact point. Test in Task 6 `shock_centre_sits_on_the_ground`.
2. **The phase change during a swing**: Frenzy must wait for the current move to end, not cut it. Test in Task 7 `frenzy_waits_for_the_move_to_end`.
3. **A fight reset after the pack was called**: the adds go and the once-flag clears. Test in Task 8 `a_reset_clears_the_pack_call`.
4. **A rolling player inside the shock/blast radius**: i-frames avoid it, the frames after do not. Test in Task 6 `a_roll_in_its_iframes_passes_a_shock`.
5. **The Taurus switch**: with `WOWSOULS_HOGGER=taurus`, the old kit, table and styles all come back together. Test in Task 3 `the_taurus_switch_brings_back_the_whole_old_hogger`.

---

### Task 1: Survey prints the attack rows and bullets of a creature

**Files:**
- Modify: `crates/dsr-cache/src/survey.rs`

**Interfaces:**
- Consumes: `params::Params::npc_attack(variation, judge)` (the existing `BehaviorParam → AtkParam_Npc` lookup in `params.rs` ~line 379; use whatever the function there is called).
- Produces: survey output lines `    atk judge J phys P level L poise S radius R knock K` under each `hit judge` line, and `    bullet behavior B -> bullet N atk A radius R phys P level L knock K` under each bullet event, when `--npc` is given.

- [ ] Step 1: Add the two print forms; an unknown row prints `atk judge J: no row` (do not fail the survey).
- [ ] Step 2: Run `target/debug/wow-souls-dsr survey --chr c4100 --npc 410000` (after `cargo build -j 4 -p dsr-cache`). Expected: every `hit judge` line of `a00_3000`-`3023` has an `atk` line; `a00_3012`/`3013` bullets resolve or print `no row`.
- [ ] Step 3: Save the output to `docs/log/2026-10-16-hogger-artorias-survey.txt`. Mark which judges are the **shock** windows (the second judge sharing a window with the blade, e.g. 141 in `3003`/`3009`: radius and level tell). Gates.

### Task 2: Probe Artorias on the gnoll; pick the rig and the clips

**Files:**
- Modify: `crates/dsr-cache/src/bonemap.rs` (only if `GNOLL_SOLDIER_RIG` does not fit: add `GNOLL_KNIGHT` pairs + `GNOLL_KNIGHT_RIG`)
- Log: `docs/log/2026-10-16-hogger-rework.md` (create)

- [ ] Step 1: `PROBE_CHR=c4100 PROBE_BONES="L Hand,R Hand,R_weapon" target/debug/wow-souls-dsr probe ...` on the clips of the spec's §4 table. Record which hand strikes, and the vertical root/pelvis travel of `3003`, `3005`, `3009`, `3010` at their windows.
- [ ] Step 2: Decide in the log: `left_handed` true/false, the blow volume's attachment, which clips drop (blow above the ground = drop, §11.3), the rig (soldier reuse unless the drift/foot-gap numbers fail the existing gates).

### Task 3: Kit `hogger_knight` extracted into the project library

**Files:**
- Modify: `crates/dsr-cache/src/monster.rs` (new `pub const HOGGER_KNIGHT: Kit`, add to `KITS`; tests)
- Modify: `crates/dsr-cache/src/cache.rs` only if a new hook is needed
- Output: `anim/source/hogger_knight/`, `anim/source/hogger_knight_ds/` (if `reference_dir` set), lines merged into `anim/source/manifest.txt`

**Interfaces:**
- Produces: kit name `"hogger_knight"`, `chr "c4100"`, `npc 410_000`, `variation 41_000`, `tier Tier::Boss`, `wow_model GNOLL_M2`, `lying` = a `LYING_*` that the deaths pass. Attack names (fixed; later tasks and the pattern use them): `slash`, `quick_slash`, `step_slash`, `close_two`, `pounce` (gap closer), `lunge` (gap closer, opener), `thrust`, `spin`, `backflip`, `head_butt` (guard break, the fast jab), `flip_slam` (big), `overhead` (big). Non-table kit moves for Task 7: `frenzy` (`a00_3013`) and `blast` (`a00_3012`), with windows that never strike as blows (`Some((9.0, 9.1))`-style past the end, as `hop_back`).
  Tier cap: 12 table attacks + 2 non-table = 14 → the two non-table moves must not count, or a table move is dropped. **Decision: drop `quick_slash` if the cap counts all 14** (`slash` covers it).
- Name collision rule: an attack named `leap` triggers `bake_leap` in `cache.rs`; hence `pounce`.

- [ ] Step 1: Failing tests in `monster.rs` tests: `hogger_knight_wears_artorias` (chr, tier, attack count ≤ `Tier::Boss.max_attacks()`, the names above present, no attack named `leap`), `the_install_calls_the_knights_chr_artorias` (install-gated like `the_install_calls_hoggers_chr_the_taurus_demon`, expects `"Knight Artorias"`).
- [ ] Step 2: Write `HOGGER_KNIGHT` from the Task 2 decisions; reacts from Artorias' `a00_2000`-range flinch clips found by survey; deaths from his `a00_22xx` clips.
- [ ] Step 3: `cargo test -j 4 -p dsr-cache hogger_knight`: pass.
- [ ] Step 4: Extract to a fresh dir: `target/debug/wow-souls-dsr extract --out cache/dsr-next`. Back up `cache/dsr/manifest.txt` to `manifest.txt.bak-before-hogger-knight`. Copy `hogger_knight*` dirs + manifest into `cache/dsr`. Diff the manifest: only `hogger_knight` lines may be new.
- [ ] Step 5: Bake: `cargo run -j 4 -p dsr-cache -- bake-project --from cache/dsr --out cache/anim-next`, copy `cache/anim-next/hogger_knight*` to `anim/source/`. Insert the new `monster hogger_knight` block and `defattack hogger_knight` lines into `anim/source/manifest.txt` (perl, at the place the fresh export has them). `cargo test -j 4 -p wow-souls ownanim::`: pass.

### Task 4: Game species, switch, tables and styles for the knight

**Files:**
- Modify: `crates/wow-souls/src/species.rs` (`HOGGER` → kit `hogger_knight`; new `HOGGER_TAURUS` = the old def, not in `ROSTER`; `pub fn resolve(def: &'static SpeciesDef) -> &'static SpeciesDef`)
- Modify: `crates/wow-souls/src/npc.rs:522` (call `crate::species::resolve(def)` first in the placing fn), `npc.rs:1021` (`styles(species.kit(), …)`)
- Modify: `crates/wow-souls/src/pattern.rs` (`HOGGER_KNIGHT`, `HOGGER_KNIGHT_TWO` rows; `HOGGER_KNIGHT_STYLE`, `HOGGER_KNIGHT_TWO_STYLE`; `styles(kit, second)` keyed by kit: `"hogger"` = Taurus styles, `"hogger_knight"` = new)
- Modify: `crates/wow-souls/src/remix.rs:276` and `:1512`, `crates/wow-souls/src/monster.rs:847` (`torso_for`): add `"hogger_knight"` where the gnoll body needs it (torso constraint only if Task 5's frame series shows the chest roll problem)
- Modify: `crates/wow-souls/src/footwork.rs` (`Temper::HOGGER_KNIGHT`: `hesitate` short (0.2, 0.6), `back_after_swing` low; the aggressive temper)

**Interfaces:**
- Produces: `species::resolve`, `species::HOGGER_TAURUS`, `pattern::HOGGER_KNIGHT{,_TWO}{,_STYLE}`.
- Pattern rules (§5.2): phase 1 chains 2-3 (`then` sums 50-70 %), `lunge`/`pounce` as opener (`opening: true` on `lunge`) and whenever the player is in their far band, `head_butt` band (0.0, ~1.6), big slams weight 1, `backflip` follow-up only (weight 0) after a chain end. Phase 2 (§5.3): chains 3-4 (sums 70-85 %), gap closer weights up. `frenzy` and `blast` are **not** rows.
- Bands: Artorias think near 3 / mid 6 / far 20 DS m × the kit's manifest `scale` × Hogger's `size`, then live.

- [ ] Step 1: Failing tests: `the_taurus_switch_brings_back_the_whole_old_hogger` (`resolve(&HOGGER)` with the env unset is `HOGGER` (kit `hogger_knight`); a pure helper `resolve_with(def, Some("taurus"))` returns `HOGGER_TAURUS` with kit `hogger`, table `pattern::HOGGER`, and `styles("hogger", false)` is the Taurus styles); `every_knight_row_names_a_kit_attack` (against the Task 3 names); `the_knight_never_lists_frenzy_or_blast_as_a_row`; `every_knight_chain_ends_in_a_move_without_follow_up_sometimes` (each style's `then` sum < 100).
- [ ] Step 2: Implement; `HOGGER.tempo` 1.0, `recovery` `Some(1.0)`, `damage_scale` set so the medium blows land in the §2 band (phys from the manifest × `DAMAGE_K` × scale vs the trial player's 290 HP); keep `health 900`, `poise` 40 (pool comes from the row's 130).
- [ ] Step 3: `cargo test -j 4 -p wow-souls species:: pattern:: boss::`: pass. Gates.

### Task 5: GATE — judge Hogger the knight in motion (§9)

**Files:** log `docs/log/2026-10-16-hogger-rework.md`; evidence `C:\Users\jacob\Documents\GitHub\wow darksouls\docs\evidence\hogger-knight-*`

- [ ] Step 1: Live unattended trial runs at Hogger (`--trial-character`, isolated saves, `WOWSOULS_WORLD=1`, `WOWSOULS_START=1`, `WOWSOULS_TRACE=1`): a passive player, and one with `WOWSOULS_ROLL`. Dense frame series (game camera + front, `cache/motion-remix/capture-at.ps1` style) of each move.
- [ ] Step 2: Judge against §8: no stretched torso, no floating blow, flips read, tells readable, blows land. Fix rig/clip picks (drop clips) and repeat.
- [ ] Step 3: **If it does not read after fixes: STOP, write the log, report to the user with the frame series; Tasks 6-9 wait.** If it reads: continue.

### Task 6: Ground shock and blast as hit spheres (`shock.rs`)

**Files:**
- Modify: `crates/dsr-cache/src/cache.rs` + `monster.rs` (for attacks listed in a new `pub const SHOCKS: &[(&str, &str, i32)]` = (kit, attack, shock judge), keep that judge's window too and write `attack-shock <attack> judge J at <s> radius <m> phys P level L knock K`; for `blast`/`frenzy`, `attack-blast <attack> at <s> radius <m> phys P level L knock K` from the bullet's row)
- Modify: `crates/wow-souls/src/monster.rs` (parse both into `AttackSpec.shock: Option<Shock>` / `blast: Option<Shock>`; `pub struct Shock { at: f32, radius: f32, damage: f32, level: u8, knock: f32 }`)
- Create: `crates/wow-souls/src/shock.rs`
- Modify: `crates/wow-souls/src/main.rs` (plugin)

**Interfaces:**
- Produces: `shock::hits(centre: Vec3, radius_yd: f32, player_feet: Vec3, player_height: f32, intangible: bool) -> bool`; `shock::centre(blade_tip: Vec3, ground_y: f32) -> Vec3` (x/z of the tip, y = ground); message `shock::Landed { at: Vec3, radius: f32, big: bool }` (read by Task 7 for decal, sound and shake).
- Radius in yd = DS m × kit scale × `size` (same as reach). Damage = phys × `DAMAGE_K` × `damage_scale`. A shock that hits applies its level/knock through the existing player hurt path (as `foebolt` does).
- Gap fallback: a bullet whose row does not resolve gets `radius 3.0 phys 200 level 4 knock 3.0` DS units, logged as `attack-blast … fallback`.

- [ ] Step 1: Failing pure tests in `shock.rs`: `shock_centre_sits_on_the_ground`; `a_shock_hits_inside_its_radius_and_not_outside`; `a_roll_in_its_iframes_passes_a_shock`; `a_shock_reaches_a_player_level_with_it_but_not_one_far_above` (height check: the player's feet within 1.0 yd of the centre's y). Extractor test `shocks_name_kit_attacks_and_judges_their_clips_carry`.
- [ ] Step 2: Implement extractor lines; re-extract and re-bake `hogger_knight` (Task 3 steps 4-5).
- [ ] Step 3: Implement `shock.rs` systems: at a kit move's `shock.at` (clip time, tempo-scaled) fire once; `blast.at` likewise. `cargo test -j 4 -p wow-souls shock::` pass; gates.

### Task 7: Impact: super armor, heavier landings, sounds, Frenzy

**Files:**
- Modify: `crates/wow-souls/src/npc.rs` (super armor: a boss in a swing's wind-up/hit window takes damage but no flinch unless its poise breaks)
- Modify: `crates/wow-souls/src/boss.rs` (Frenzy: `pub struct Frenzy { state: FrenzyState }`, `enum FrenzyState { Waiting, Rising(f32), Done }`; at the phase change it waits for the current move to end, then forces the kit move `frenzy`; super armor through it; the `blast` at the clip's burst via Task 6; then phase 2 tables; red glow)
- Modify: `crates/wow-souls/src/audio.rs`, `sfx.rs` (swing whoosh, landed impact, ground thud on `shock::Landed` hit or miss, a real roar for Frenzy)
- Decal: ground dust ring on `shock::Landed` (ground quads, as Battle Shout / Thunder Clap decals in `abilities/fx.rs`)

**Interfaces:**
- Consumes: `shock::Landed`, `boss::Roar` (now sent when Frenzy rises, not at the threshold).
- Produces: `boss::frenzy_step(state: FrenzyState, phase_two: bool, swinging: bool, clip_t: f32, burst_at: f32, len: f32) -> (FrenzyState, Option<FrenzyEvent>)`, `enum FrenzyEvent { Start, Burst, End }` (pure).

- [ ] Step 1: Failing pure tests: `frenzy_waits_for_the_move_to_end`; `frenzy_bursts_once_at_its_time`; `frenzy_happens_once_per_fight`; `a_boss_mid_swing_does_not_flinch_from_a_light_blow` (pure rule `boss::flinches(swinging: bool, poise_broken: bool) -> bool`).
- [ ] Step 2: Implement. Sounds: pick WoW sound files by name in the existing sfx tables (heavy weapon impact, `GnollAggro` pitched down for the roar if no better roar exists); log the picks.
- [ ] Step 3: Live: Frenzy plays after the current move, blast knocks back a standing player and misses a rolling one (`WOWSOULS_BOSS_HP=0.55` to start near the threshold). Gates.

### Task 8: "Yipe! Help Hogger!" pack call at 25 %

**Files:**
- Modify: `crates/wow-souls/src/boss.rs` (`PackCall` resource/component; system spawns two Riverpaw gnolls via the summon path `named` uses; `named::Summoned` with linger 30 s)
- Modify: `crates/wow-souls/src/named.rs` only to expose the spawn helper it already uses for the Dockmaster

**Interfaces:**
- Produces: `boss::pack_call_due(called: bool, health_fraction: f32) -> bool` (true once at ≤ 0.25), `boss::pack_spot(boss: Vec2, player: Vec2, dist: f32) -> Vec2` (15 yd from the boss, opposite the player).
- The species of the adds: the Riverpaw gnoll species the zone already spawns (`species::by_name` of the Riverpaw Outrunner / Runt def; pick the level 9-10 one).

- [ ] Step 1: Failing tests: `the_pack_is_called_once_at_a_quarter`; `the_pack_comes_from_the_far_side`; `a_reset_clears_the_pack_call` (a boss back at full health clears `called`).
- [ ] Step 2: Implement; the yell as a log line + the gnoll bark sound.
- [ ] Step 3: Live with `WOWSOULS_BOSS_HP=0.3`: two gnolls arrive once; after a player death/reset they go. Gates.

### Task 9: Tune, scripted gates, hand over

**Files:** `species.rs`, `pattern.rs` (numbers only), `docs/log/2026-10-16-hogger-rework.md`, `docs/HANDOVER.md`, memory `wow-souls-hogger.md`

- [ ] Step 1: Scripted runs (§10): passive level 11 trial dies in < ~15 s; Frenzy and pack fire once each; every shock lands at ground height (trace line); blast misses a rolling player. Adjust `damage_scale`, bands, chances.
- [ ] Step 2: A short fight clip + frame series for the user (`videos/`, no upload).
- [ ] Step 3: Full gates (fmt, clippy, install tests). HANDOVER State/Next updated in place (< 8 KB), log written, memory updated.
