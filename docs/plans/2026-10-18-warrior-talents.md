# Warrior talents and perfect defense Implementation Plan

> Execute in this session with subagent-driven-development. Preserve existing working changes; commit only if the user requests it.

**Goal:** Deliver a playable, saved Warrior talent/progression flow with skill-based perfect block/parry and meaningful Arms, Fury and Protection effects.

**Architecture:** Pure talent definitions/allocation/save parsing live in wow-world. A game talent resource and menu own per-character persistence and selection. Combat effects operate at actual defense/weapon contacts; existing hit messages, action machines, animations, rewards and four-slot hotbar remain authoritative.

**Tech Stack:** Rust, Bevy 0.18.1, existing wow-world and wow-souls crates, local project animation library.

---

### Task 1: Talent rules, persistence and player menu

Create `crates/wow-world/src/talents.rs`, export it from `lib.rs`; create `crates/wow-souls/src/talents/mod.rs` and UI as needed; register in `main.rs`; add Menu > Talents through `sysmenu/mod.rs` and `sysmenu/ui.rs`. Register `talents-v1.txt` in `saves.rs`.

Implement the exact 21-node design, gates, per-level 51-point budget, class validation, stable string IDs and descriptions. Expose `talents::Talents` resource with `pub build: wow_world::talents::Build` and `pub fn rank(&self, id: TalentId) -> u8`; inactive classes return zero. `Build` exposes ranks, points/spent by tree, available(level), spend(id, level), reset, text/parse. Expose pure `signature()` returning spell id 12294/23881/871 for Mortal Strike/Bloodthirst/Shield Wall. Missing saves remain unspent. Menu gives requirements/ranks/feedback, points and basic defense instructions; block allocation/respec during combat/death, make reset deliberate, and save only successful changes. Existing saves must retain all unrelated data.

Write meaningful pure rule/parser tests first, run `cargo test -j 4 -p wow-world talents --offline`, then implement and rerun. Test menu transitions and persistence without live player save writes. Add unattended talent script/save overrides for reproducible UI checks. Coordinate shared files with root; do not edit defense/combat/hits/abilities files.

### Task 2: Perfect defense and combat rewards

Modify `defense.rs` and action state with per-guard perfect eligibility/rearm fields, explicit perfect event and tested timing/arithmetic helpers. Preserve `intercept` caller API if practical. Add target-bound counter reward and transient feedback using existing sound, HitStop and Toasts. Tests before implementation: boundary/rearm/multi-hit/ordinary fallback; run `cargo test -j 4 -p wow-souls defense --offline`. Audit attack eligibility and preserve authored animations and ordinary guard/parry behavior.

### Task 3: Wire talent effects and signatures

Modify `hits.rs`, `abilities/mod.rs`, `abilities/moves.rs`, world ability/class tables, and new `warrior.rs` runtime. Apply rank effects only to Warrior and actual weapon contacts. Keep all damage multipliers bounded; one stamina/rage/healing reward per attack rather than per target. Bleed uses normal death/reward messages and cannot recursively proc. Signatures derive from allocations, occupy fourth hotbar slot, and disappear on reset. Validate stamina/body/weapon/grounding before payment. Execute signature via weapon action, no guaranteed auto-hit; heal Bloodthirst only on contact. Clear temporary combat rewards on death/reset. Add meaningful integration tests for denial/resource/proc limits and class restrictions.

### Task 4: Spec review and quality review

Independently inspect code against the design, repair any omissions, then run a separate code quality review. New reviews must inspect current working files, because the repository has substantial earlier uncommitted work.

### Task 5: Production checks and live evidence

Run `cargo fmt -p wow-souls -p wow-world -p wscl -- --check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`, `$env:WOW_DATA='C:/Spel/World of Warcraft 1.12/Data'; cargo test -j 4 --workspace --offline`, then `cargo build -j 4 -p wow-souls --offline`. Use isolated unattended save overrides and scripted inputs to capture talent view, allocations/reload and real defense encounters. Inspect image sequences in motion order; distinguish machine verification from the user's judgement of feel. Update `docs/HANDOVER.md` and `docs/log/2026-10-18-warrior-talents.md` with controls, decisions, evidence and limits. Do not commit or stage.
