# Warrior weapons implementation plan

**Goal:** Equipped warrior weapons select appropriate moves, using locally measured DSR timing, stamina and attack parameters wherever a counterpart exists.

**Architecture:** Extend the existing DSR extractor with a reproducible weapon survey and independently published weapon cache. Select a cached family from WoW item subclass; preserve the existing roll, reactions, defense and loot caches. All derived assets remain in ignored cache directories. User requested implementation; no commits.

**Tech Stack:** Rust, Bevy 0.18.1, wscl, local DSR PARAM/TAE/HKX and WoW 1.12 data.

1. Survey `EquipParamWeapon`, `BehaviorParam_PC`, TAE and HKX for representative families; record provenance and distinguish adaptations from exact source facts.
2. Add family extraction in `dsr-cache/src/moveset.rs` and `cache.rs`, publish complete manifests only after validation. Check retarget and weapon-axis gates for each move.
3. Add runtime family selection in `wow-souls/src/dsclip.rs` and equipment integration. Cover dagger, sword, axe, hammer, greatsword, greataxe, greathammer, polearm, staff, fist/unarmed, bow, crossbow, gun and thrown. The user confirmed Souls combat for all, with research-based approximations where the only installed game (DSR) lacks a counterpart.
4. Verify switch semantics, unavailable cache behavior, multi-window attacks, two-hand grip, matching hit geometry and source parameters. Preserve inventory and action state during equipment changes.
5. Run scoped fmt, clippy and tests with installs enabled, then unattended runtime traces and screenshots, especially Militia Warhammer. Update HANDOVER and README with actual results and remaining limits.
