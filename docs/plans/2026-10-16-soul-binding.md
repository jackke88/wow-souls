# Soul Binding Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Persist one explicit graveyard binding per character and teach it through Falkhaan.

**Architecture:** A soulbind resource owns DB-derived sites and an atomic GUID save. The existing interaction reader sends a bind request; the binding system validates it, writes the save and publishes SoulBound. The existing death sequence holds the destination until streamed ground is ready. A separate tutorial overlay and adapter feed successful binding into the existing quest engine.

**Tech Stack:** Rust, Bevy 0.18.1, the existing wow-world manifest and quest engine.

Work in the shared checkout to preserve concurrent changes. Re-read shared files before each small edit. User standing order: no commits.

1. Create `crates/wow-souls/src/soulbind.rs`: sites from Spirit Healer spawns, strict `wssoulbind 1\nsite <guid>\n` parsing, per-character save with unreadable-file protection, range/height/combat validation, request and success messages. Test malformed/unknown saves, switching a site and failed persistence.
2. Create `crates/wow-souls/src/soulbind/art.rs`: resident-ground memorial stones with stone pedestal, rounded cap, inset soul rune and active glow. Reuse the quest prompt for the bound/current location.
3. Wire `main.rs`, `boot.rs`, `saves.rs`, `avatar.rs`, `quests/mod.rs` and `quests/prompt.rs` narrowly: register plugin/save ownership, use saved initial position, route interact to a nearby stone before NPCs/objects and display its action.
4. Adapt `hurt.rs`: resolve the active site's safe position, hold the dead body and black death cover while destination terrain/collision loads, then perform the existing full restoration and Respawned message. Test holding without ground and completing with ground.
5. Create `quests/soul_tutorial.rs`: clone the inn quest's structural template into a separate authored ID, add Falkhaan start/end relations and a memorial objective; use existing object credit only after successful binding. Test acceptance stays incomplete, a binding completes it, prebound acceptance, turn-in, and the original inn quest stays unchanged.
6. Run `cargo test -j 4 -p wow-souls --bin wow-souls soul` and targeted respawn tests, then required formatting, clippy and workspace tests with both install variables. Run an isolated live interaction/death and saved-graveyard restart, inspect a screenshot, update HANDOVER state and `docs/log/2026-10-16-soul-binding.md` with results.

Execute in this chat under the user's approved scope; no separate execution-choice approval is needed.
