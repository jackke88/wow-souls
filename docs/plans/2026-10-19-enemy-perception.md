# Enemy Perception Implementation Plan

> **For Claude:** Execute task by task in this session. User authorized implementation; no commits or staging.

**Goal:** Replace omnidirectional distance aggro with sight and action-dependent hearing.

**Architecture:** A small pure perception module computes a detection cause and player noise. `npc::think` supplies facing, actual movement, action and existing world occlusion. `think_step` uses the result only to acquire targets; engagement/leashing stays intact.

**Tech Stack:** Rust, Bevy 0.18, existing motor and WorldCollision.

---

1. Add a regression test in `crates/wow-souls/src/behavior/tests.rs` exercising the actual NPC system at front/back positions and walking/running/sprinting speeds. Run `cargo test -j 4 -p wow-souls --offline the_real_npc_loop_requires_sight_or_audible_movement`; confirm the quiet approach behind currently fails.
2. Create `crates/wow-souls/src/npc/perception.rs`: sight cone/range, hearing by actual motor/action, shared height/occlusion gates, detection cause. Add boundary, contact, wall/height, silence, movement and action tests.
3. Integrate the module in `crates/wow-souls/src/npc.rs`: extend Sense with hearing radius, replace acquisition guard, compute noise once per player/frame, raycast only a candidate, log acquisition cause. Add an optional prototype-spawn facing instrument for repeatable live checks. Test return acquisition, combat memory and dead player behavior.
4. Add a real NPC-loop occlusion test using a static solid collider. Run targeted tests, then format check, workspace clippy and workspace tests with `WOW_DATA`, all builds limited to `-j 4`. Format only changed source files if necessary.
5. Build the binary and run isolated saves on flat Northshire ground. Compare front/behind at the same distance, walking safely outside hearing range, running into hearing range and Battle Shout. Inspect evidence and logs. Update `docs/log/2026-10-19-enemy-perception.md` and the compact state in `docs/HANDOVER.md`.
