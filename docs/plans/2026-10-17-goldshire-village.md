> Superseded 2026-10-18 at the user's request: remove extra houses and vegetation omissions; keep sparse road traffic only. Current behavior: docs/GOLDSHIRE.md. The house survey is historical research and now writes only its research output, never production road data.

# Goldshire Village Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make Goldshire a living village with approximately 12–16 family homes, coherent civilian routines and native WoW ambience.

**Architecture:** Project-owned village data augments native streamed WMO/M2 placements. A local clock-driven civilian simulation supplies current positions to the existing spawn ring and uses physical movement near the player. Reuse original client assets and existing animation/ambience systems.

**Tech Stack:** Rust, Bevy 0.18.1, benilla, WoW 1.12 client data, existing wow-world manifests.

User approval supersedes the skill's commit and execution-choice boilerplate:
execute in this session, preserve parallel changes, do not commit.

### Task 1: Survey original village geometry and scenery

Files: `crates/wow-world/examples/goldshire_survey.rs`, `docs/research/2026-10-17-goldshire-survey.*`.
Read native ADT placements, WMO meshes, tree bounds, terrain and animated wildlife.
Produce measured model/placement inventory and map. No guessed house dimensions.
Run `cargo run -j 4 -p wow-world --example goldshire_survey --offline` with `WOW_DATA`.

### Task 2: Place and validate family homes

Files: project-owned village data/module; small additive integration in
`vendor/benilla/crates/benilla-world/src/terrain_stream.rs` if needed.
Choose 12–16 measured, road-adjacent sites with usable doorways. Reject terrain,
tree/object/building/water overlaps. Reuse native WMO streaming and props.
Test rejection cases and unique placement IDs. Render the actual layout and walk entrances.

### Task 3: Families, daily routines and travel

Files: `crates/wow-world/src/village.rs`, `crates/wow-souls/src/village.rs`,
bounded hooks in `world.rs`, `walkers.rs`, `actor/anim.rs`, `boot.rs`, `main.rs`.
Map current residents to homes; keep quest/service access. Build continuous routes,
day/night activities and sparse travel; preserve current location when streaming.
Test midnight, time jumps, re-entry and protected NPCs through production integration.

### Task 4: Native ambience and live acceptance

Inventory before adding native wildlife. Verify existing birds move; fill real
gaps with original M2 resources and modest density. Preserve original small animals.
Capture day/evening/night motion, validate house access and background continuity.
Run `cargo fmt -p wow-souls -p wow-world -p wscl -- --check`,
`cargo clippy -j 4 --workspace --all-targets -- -D warnings`,
`cargo test -j 4 --workspace --offline` with `WOW_DATA`.
Write exact outcomes and limitations in `docs/log/2026-10-17-goldshire-village.md`
and update HANDOVER state without overwriting other work.
