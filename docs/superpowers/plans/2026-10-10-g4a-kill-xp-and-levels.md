# G4a: kill XP and the level's effect: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Steps use `- [ ]`.
> Executed inline by the planner on the user's "Kör på" (standing orders: no commits, `-j 4`, gates every step).

**Status:** all four tasks done 2026-10-10 (G4b, the bloodstain, was then built inline from the spec without its own plan file;
the record of both is `docs/HANDOVER.md` "Next action"). The level-up sound of Task 2 was left out.

**Goal:** a kill pays WoW XP (the vMaNGOS formula) into the one XP total, and the level raises max HP and stamina.

**Architecture:** the formula is pure in `wow_world::quest::xp` (`gray_level`, `zero_difference`, `kill_xp`, `MobXp`);
the game side adds `level.rs` (`vitals`, a follower system) and extends `quests::credit_kills`. Spec:
`docs/superpowers/specs/2026-10-10-xp-levels-souls-design.md`.

**Tech Stack:** Rust 2021, Bevy 0.18.1. Gates: `cargo fmt --all -- --check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`,
`DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 --workspace`.
Baseline before G4a: wow-souls 377, wow-world 117 + 4, dsr-cache 53, dsr-formats 51, wscl 4.

---

### Task 1: the formula (`crates/wow-world/src/quest/xp.rs`)

- [x] Tests first (in `xp.rs` `mod tests`): `gray_level` (1 -> 0, 5 -> 0, 6 -> 1, 10 -> 4, 39 -> 31, 40 -> 31, 60 -> 47),
  `zero_difference` (every band edge), `kill_xp(1, 1, false) == 50`, `kill_xp(1, 3, false) == 55`, `kill_xp(1, 9, false)`
  is the +4 cap (60), `kill_xp(3, 3, true) == 120`, `kill_xp(6, 1, false) == 0` (gray 1), `kill_xp(10, 4, false) == 0` (gray 4),
  `kill_xp(10, 6, false) == 41` (95 x 3 / 7), ties to even (`kill_xp(1, 2) == 52`, `kill_xp(1, 4) == 58`),
  a critter's `MobXp` pays 0 whatever the level, `MobXp::level_for(guid)` spreads `level_min..=level_max` by guid.
  (Done 2026-10-10; the first draft of these numbers in this plan had the gray levels of 6 and 39 wrong, the tests have the right ones.)
- [ ] Implement `gray_level(u32) -> u32`, `zero_difference(u32) -> u32`, `kill_xp(player: u32, mob: u32, elite: bool) -> u32`
  (round half to even), `MobXp { level_min, level_max, elite, pays }` with `MobXp::of(&Creature)` (`pays = kind != 8`,
  `elite = (1..=3).contains(&rank)`) and `level_for(guid) -> u32`, `payout(&self, player: u32, guid: Option<u32>) -> u32`.
- [ ] Run: `cargo test -j 4 -p wow-world xp` -> pass. Update the module doc (kill XP is in).

### Task 2: the level's effect (`crates/wow-souls/src/level.rs`, `lib`/`main` module list, `boot.rs` plugin)

- [ ] Tests first: `vitals(1) == (100.0, 100.0)`, `vitals(5) == (140.0, 120.0)`, `vitals(0)` = level 1's, `apply` (a pure
  function `fn follow(level: u32, applied: Option<u32>, hp: &mut Health, st: &mut Stamina) -> bool`): first application sets the
  maxima and fills; a rise by 2 raises `max` and `cur` by the same difference; no change when the level is the applied one;
  a lower `cur` keeps its difference to the new `max`.
- [ ] Implement `vitals`, `follow`, and the system `follow_level` (Update, after the quest systems; `Option<Res<QuestGame>>`, the
  avatar's `Health` / `Stamina`, a `Local<Option<u32>>`), registered by a `plugin`. The level-up sound is a later step if the file
  is found (see Task 4).
- [ ] Run the new tests.

### Task 3: kills pay XP (`crates/wow-souls/src/quests/mod.rs`)

- [ ] `QuestGame` gains `mobs: HashMap<u32, MobXp>` (built in `load` from `world.creatures`); `entry_of` also returns the spawn
  record's guid (`Option<u32>`); `credit_kills` computes `payout(player.level, guid)` per `Slain`, `set_xp(xp + gain)`, a toast
  `N experience gained` and the level-up toast, one log line `quests: kill xp +N (entry E, level L, you M)`, saves.
- [ ] `xp:<n>` script cheat (adds XP as a reward does). Test: `parse_script` keeps it; a pure helper test for the payout wiring
  where it can be done without an `App`.
- [ ] Gates: fmt, clippy workspace, tests.

### Task 4: live check, picture, HANDOVER

- [ ] Unattended fight: `WOWSOULS_WORLD=0 WOWSOULS_KOBOLD_AT=-8911,-136 WOWSOULS_ATTACK=3.0,0.9 WOWSOULS_CHECK=...` (kobold dies,
  `quests: kill xp +N` appears; with `WOWSOULS_QUESTS_SCRIPT=0.5:xp:395` the first kill crosses 400: a level-up toast and the
  HP/stamina maxima 110 / 105 in the log; the bar is longer in a screenshot). Install untouched check as always.
- [ ] Update `docs/HANDOVER.md` (new "G4a BUILT" entry first in "Next action", test baselines, the module table, the switches).
