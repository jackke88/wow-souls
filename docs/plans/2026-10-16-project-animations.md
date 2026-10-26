# Project Animation Library Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Every imported runtime animation is editable text in the real project, with no ignored-file or DSR-install requirement.

**Architecture:** Extend the existing `wscl::text` and bake pipeline to export a complete namespaced
project library with manifests. Make the shared clip reader read project text for all rigs, preserve
the existing flat player overrides, and switch the default animation root to the project library.
Keep original cache verification strict for explicit cache-based development workflows.

**Tech Stack:** Rust, wscl, Bevy 0.18.1, existing DSR extractor and WoW retargeted clips.

---

Execute in the shared checkout, rereading before edits. User has approved this concrete migration;
do not commit unless asked. Preserve other chats' changes and existing manually edited animations.

1. Inventory current `anim/source`, all runtime readers, root metadata, active `weapons.txt` paths
   and creature/reference folders. Record existing vs missing project coverage.
2. Extend `crates/wscl/src/text.rs` only where needed to preserve every transform, including scale;
   test finite/shape validation and exact orientation/root/event round trips.
3. Add complete-project export to `crates/dsr-cache/src/` and CLI wiring in `main.rs`. Namespaced
   clips, stable weapon-family paths, source provenance without runtime source-file reads, validated
   round trips, and no overwriting pre-existing editable clips.
4. Update `ownanim.rs`, shared `dsclip.rs` reader/default root, and locomotion/player integration.
   Verify reaction/defense/interaction/monster loaders automatically consume the shared text path.
5. Export the library; remove `/anim/source` from `.gitignore`; verify `git check-ignore` and project
   inventory. Update stale instructions in source docs, provenance, handover and an animation README.
6. Add mandatory library completeness/loader checks and missing/corrupt clip regressions, retaining
   synthetic-cache tests' isolation. Run tests without source installs or legacy binary cache access.
7. Build and compare representative gait/attack/reaction/interaction/creature motion sequences to
   the existing cache path using isolated saves. Run scoped fmt, strict workspace Clippy and workspace
   tests with the required install variables. Record counts, residual non-animation dependencies
   and evidence in `docs/log/2026-10-16-project-animations.md`.
