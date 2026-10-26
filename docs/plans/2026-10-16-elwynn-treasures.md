# Elwynn Hidden Treasures Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add eighteen discoverable Elwynn chests with exact persistent loot and useful gear for every Vanilla class.

**Architecture:** Extend the small authored table in `crates/wow-souls/src/loot/treasures.rs` using
stable IDs and existing `ItemCount` bundles. Keep the current open/take/save transaction and native
chest animations. Restrict chest ground snapping near its authored floor so streamed cave roofs or
missing floors cannot permanently place a chest on a different level.

**Tech Stack:** Rust, Bevy 0.18.1, benilla streamed WoW world, vMaNGOS item and placement data.

---

Execution is in the shared checkout; user standing orders prohibit commits without a request.
The user has authorized implementation with expanded chest count and class coverage. Execute here.

### Task 1: Survey and choose fixed loot

- Read `loot/treasures.rs`, `loot/mod.rs`, imported `cache/world/elwynn.txt` item/object rows.
- Inspect native scenery with the existing isolated `cache/treasure-work/survey.ps1` harness.
- Select six supplies, four white gear and eight green gear locations. Preserve Northshire IDs.
- Save final human-readable content in `docs/ELWYNN-TREASURES.md` with class coverage.

### Task 2: Place the chests safely

- Modify `loot/treasures.rs`: append eighteen independent permanent caches using existing items.
- Modify `loot/mod.rs` only where needed for safe authored floor alignment.
- Validate imported metadata before exposing a reward on a smaller/custom manifest.
- Add meaningful regression tests for fixed imported gear, Warrior/Mage equip eligibility,
  all nine planned class categories, unique independent claims and the cave-floor guard.
- Run `cargo test -j 4 -p wow-souls loot::` and the scoped fmt gate; expect success.

### Task 3: Verify the complete flow

- Build `cargo build -j 4 -p wow-souls`; copy the binary into the isolated verification directory.
- Use child `LOCALAPPDATA` and explicit items save; never change a real character's progress.
- Check each site's floor, native concealment, accessible front and animated lid. Open/take all
  eighteen through the real interaction system, verify exact inventory and permanent saved claims.
- Test reopening/restarting, approach with the real motor, and representative complete-world NPCs.
- Adjust failing coordinates and recheck only affected locations.

### Task 4: Required gates and handover

- `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check`
- `cargo clippy -j 4 --workspace --all-targets -- -D warnings`
- Set `DSR_DIR` and `WOW_DATA`; `cargo test -j 4 --workspace`.
- Report unrelated parallel-file failures accurately, without claiming a green gate.
- Update `docs/log/2026-10-16-elwynn-treasures.md`, README pointer and the slim HANDOVER state.
