# WoW-only runtime and local source Implementation Plan

**Goal:** Build and run the existing game from this repository with WoW 1.12.1 as the only required game installation.

**Architecture:** Runtime animation loaders use the tracked editable library exclusively. DSR extractors become optional standalone developer workspaces; the production workspace and HUD never read DSR files or binary caches. Copy the exact engine dependency closure, including current local fixes and licences, into `vendor/benilla`; Cargo paths must remain inside this repository. Preserve existing source animation curves and truthful provenance.

**Tech Stack:** Rust 2021, Cargo, Bevy 0.18.1, vendored benilla engine, PowerShell, static HTML/JavaScript.

## 1. Audit and preserve

- Inspect runtime loaders, workspace dependency graph, project asset tracking and current changes.
- Keep all existing edits, staged animations and source provenance. Do not commit.
- Record imported animations as source-derived; independent animation replacement is separate work.

## 2. Remove runtime DSR selection

- `crates/wow-souls/src/dsclip.rs`: ignore the retired DSR cache variable, reject extraction manifests, load `.wsanim` exclusively.
- `crates/wow-souls/src/hud/mod.rs`, `hud/art.rs`: remove loading from the DSR HUD cache; legacy/plain styles remain self-contained.
- Update loader diagnostics to point to project assets, never an extractor.
- Convert binary test fixtures and optional cache tests to editable project files and mandatory checks.
- Regression: obsolete DSR settings, missing project text, extraction source declarations and old HUD modes must never re-enable external reads.

## 3. Keep source dependencies in the repository

- `vendor/benilla`: copy only the 12 Cargo engine dependencies and their required support files; preserve both licences, third-party notices, revision and local-change hashes.
- `Cargo.toml`: explicit production members and local vendor paths; exclude standalone DSR tools.
- `crates/dsr-cache/Cargo.toml`, `crates/dsr-formats/Cargo.toml`: optional standalone tooling, explicit dependencies.
- Include the active world manifests as project data; remove default paths into developer research folders.
- Verify Cargo metadata has no local package outside the repository and no DSR package in the production graph.

## 4. Documentation and packaging

- Update current README/handover, animation guide and dependency report; keep historical records and licences.
- Add a reproducible isolated package check with only the executable, project animation/data files and WoW configured. Keep saves isolated.
- Update website requirements/install instructions to require only WoW; keep the disclosure that current animation curves are DSR-derived.

## 5. Verification

- Scoped formatting (never `cargo fmt --all`), production workspace tests, strict Clippy, and player build with `-j 4`.
- Run isolated gameplay checks with absent DSR/ER paths, no extraction caches and the tracked project assets.
- Static site checks and dependency audit. Record exact passing checks and any limitations in the work log.
