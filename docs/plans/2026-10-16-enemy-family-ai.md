# Enemy Family AI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.
> Execution here is in the current shared session. Jacob approved the complete mapping; no commits requested.

**Goal:** Give all 47 roster entries their approved 15 combat roles while retaining working DSR clips and combat values.

**Status:** Implemented and verified 2026-10-17. Coverage, repairs and limits: `../log/2026-10-17-enemy-family-ai.md`.

**Architecture:** Pure per-role decisions own movement phases and attack permission. The existing NPC brain continues to own animation clocks, damage, reactions, special abilities, leash and respawn. Shared steering and fair local encounter admission remain separate from each family's tactics; Hogger retains his existing boss path.

**Tech Stack:** Rust, Bevy 0.18.1, existing benilla world collision, project text animation library.

---

### 1. Freeze a comparison and add policy tests

- Copy the current executable to `cache/family-ai/before.exe` and preserve source snapshots for relevant files there.
- Add `crates/wow-souls/src/behavior.rs` and `behavior/tests.rs`. Test role assignments against `docs/ENEMY-AI-MAPPING.tsv` and whole decision sequences: boar aim/charge/recover, bear approach/pressure, spider aim/leap/hop, kobold short sortie, wolf flank, murloc gather/rush, caster finite escape, escort interception.
- Include checks for close-range attack gating, valid available moves, delay of player reactions, cancellation/reset, fairness, and blocked terrain/line of sight.

### 2. Implement role decisions and roster bindings

- Add a per-species role binding in `species.rs` and explicit overrides for mixed-role families.
- Implement state and contextual attack preference in `behavior.rs` using current species attack bands. No imported Lua runtime or installation access.
- Share only movement conversion and safe ground checks; pressure/charge/ambush roles do not orbit.

### 3. Integrate with the actual NPC loop

- `main.rs`: register module. `npc.rs`: store per-creature tactical state, collect local group facts, choose tactical plan before attack admission, and apply it to movement.
- Preserve existing chain/forced special attacks, latch hop, stagger, clip root motion and boss path. Reset plans on hit, disengagement and respawn.
- Use deterministic encounter admission with wait priority; idle group members receive their role's task.
- Add bounded diagnostics through the existing animation trace switch for live proof.

### 4. Verify actual fights and finish documentation

- Focused role/regression tests, scoped workspace format, strict workspace all-target Clippy and workspace tests (`-j 4`, installed source env in same shell).
- Build current game. Use isolated saves/manifests and project animations, with nonexistent DSR/ER install paths, to record before/after fights. First kobold/boar/bear; then wolf, spider, murloc, gnoll, Defias roles, casters, leader/escort and Garrick. Include group, movement/attack/dodge and terrain cases.
- Inspect game-camera and front motion sheets and actual traces; repair failures before accepting them.
- Update the approved design status, mapping statuses and `docs/HANDOVER.md` (<8 KB); write results/limits in a dated log. No commit.
