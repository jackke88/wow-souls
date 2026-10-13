# Player block, parry and roll audit Implementation Plan

> Execute inline in this chat, task by task. The user asked for this work alongside the world/quest work. Do not commit; the user commits.

**Goal:** A held sword block, stamina guard break and timed empty-left-hand parry, using the installed Dark Souls clips and params; verify that the existing dodge is the medium roll.

**Architecture:** Extend the pure action machine with defense clips and cancel times. Extract optional defense manifest records without invalidating the existing player/monster cache. Resolve guard at the actual enemy hit; successful blocks retain guard, reduce health damage, spend stamina and emit a metal impact instead of flesh/blood. A swept left-hand parry cancels an eligible creature's swing and plays its own parried clip. The world/quest modules remain under their current work.

**Tech Stack:** Rust, Bevy 0.18.1, existing dsr-cache / wscl readers and retargeter.

## Design decisions

- Block: a10_0100 raise, a10_0001 hold, a10_0101 lower, active only in flag 85; sword row 200000 (cut 50%, stability 32). Hold E / pad LB. Slow movement, no jumping, regeneration held while guarding. Attack, evade and parry obey clip cancel flags.
- Parry: a42_5100, left hand radius from variation 4201 / judge 450; C / pad LT. No invulnerability. Eligible attacks: melee wind-up through the active hit window, in front, within the swept hand volume; dogs and unblockable throws are excluded. This timing rule and front half-plane are explicit prototype choices, not decoded engine rules.
- Guard break: a00_2080..2083 chosen by measured push, no i-frames. Unblockable attacks bypass guard. Damage and stamina remain fractional internally.
- Parry reaction: a00_2100 for kobold, thug and Garrick kits; interrupt movement and attack for its flag-41 control window. Riposte, enemy guard/parry and item-equipment changes are separate follow-up work.
- Roll: retain a00_0710 and its source timings; fail automatic selection when the medium roll is missing, instead of silently selecting a fat roll.

## Tasks and checks

- [x] Audit roll list and current cache; regression test incomplete selection and installed clip timings.
- [x] Read guard and attack params through PARAMDEF; install tests pin sword, fist-parry and enemy stamina/unblockable values.
- [x] Extract defense clips, measured sword grip, break pushes and creature parry reactions; keep retarget gates unchanged and write source provenance.
- [x] Load optional defense records with strict finite/range/bone checks. Add pure action tests for raise/hold/release, cancels, no-stamina, no-cache, break and failed parry.
- [x] Wire E/LB and C/LT, scripted guard/parry schedules, guard interception, swept parry, metal feedback and no blood on blocks.
- [x] Run fmt, clippy -j 4 --workspace --all-targets -D warnings, tests -j 4 --workspace with DSR_DIR and WOW_DATA. Build game/extractor; install-untouched comparison.
- [x] Live unattended checks: front/back block, stamina break, unblockable bypass, timed/late parry, wolf exclusion, guard release/attack/roll, medium roll root motion; screenshots and HANDOVER update.

Built report: [2026-10-09-player-defense-built.md](../superpowers/specs/2026-10-09-player-defense-built.md).
