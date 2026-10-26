# wow-souls — Handover

**Bootstrap:** this file + `CLAUDE.md`. Look up other docs on demand:
- archive: `docs/archive/HANDOVER-full-2026-10-13.md`; search headings in `docs/log/INDEX.txt`, then read a bounded section
- modules: `docs/ARCHITECTURE.md` or module `//!` docs; specs/plans: `docs/superpowers/specs/`, `docs/superpowers/plans/`

**Keep under 8 KB:** update State/Next in place; history goes to `docs/log/<date>-<topic>.md`.

## What this is

Single-player WoW 1.12.1 (Northshire, Elwynn, Westfall), Souls combat, Rust/Bevy 0.18.1 on the benilla fork.
Offline. Swedish, short. "continue" = next small step, live verification, handover.

## Standing orders (the user, do not ask again)

- **Model per task is Claude's call** (haiku/sonnet down for mechanical work, opus/fable up for design, new extractor work, hard debugging, review). Whoever does the work verifies it.
- **Commit only when the user asks.** Never `cargo fmt --all` (it rewrites the benilla fork): gate = `cargo fmt -p wow-souls -p wow-world -p wscl -- --check`. Build with `-j 4` (RAM-tight).
- **Judge animation in motion**: a dense fight frame series from the game's camera + front views, never stills (a stretched Hogger was once called fine from stills).
- **Spawn/NPC/quest data** = vMaNGOS DB at `C:\Spel\MaNGOS`; never start its server, query a COPY (details: archive, "Quests: data import").
- **Monsters** wander/stand/path exactly as the DB says; density 0.5 with per-quest floors; respawn minutes, never near the player.
- **Source workflow**: DSR/ER → test → own (`docs/SOURCE-TO-ORIGINAL.md`). 1,203 renamed reference clips + project kits, no source install/cache (`anim/README.md`). **User keeps DSR**; ER reference only. Sprint fix: `2026-10-16-dsr-sprint-upright`.
- **Release target**: own website; WoW required, **zero DSR/ER dependency** after clean-environment gate. Raw archives outside repo/package; env/future `paths`, **never new hardcoded machine paths**.
- **Parallel chats** also edit files (weapons / animation / combat / projectiles / defense, the graphics fork): re-read a file right before editing it; keep edits small.
- **Production tests** use tracked project animations/world data without DSR/cache settings. WoW-only format checks still need `WOW_DATA`. DSR tools are standalone, excluded from the production workspace.
- **Python hangs the Bash tool here**: use perl/awk/sed or the Write/Edit tools for scripted edits.

## Paths

| What | Where |
|---|---|
| Workspace (own git repo) | `C:\Spel\wow-souls` |
| WoW 1.12.1 install (read-only) | `C:\Spel\World of Warcraft 1.12` (junction `wow-souls\WoW`) |
| DSR install (read-only, read by `wow-souls-dsr`) | `C:\Spel\Dark Souls Remastered` |
| Engine source | `vendor/benilla/` (licences and exact revision included) |
| World and voices | `data/world/`, `data/voices/` |
| Evidence pictures (gitignored) | `C:\Users\jacob\Documents\GitHub\wow darksouls\docs\evidence\` |
| Saves | `%LOCALAPPDATA%\WoWSouls\characters\<n>\` |
| Showcase films, edit tools, logs | `C:\Users\jacob\Documents\GitHub\wow darksouls\videos\`; see `tools\README.md`; never upload without the user's yes |

## Build, run, verify

```bash
cd /c/Spel/wow-souls && cargo run -j 4 -p wow-souls
# production gates (WoW only):
cargo fmt -p wow-souls -p wow-world -p wscl -- --check
cargo clippy -j 4 --workspace --all-targets -- -D warnings
WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 --workspace --offline
# DSR cache (extractor): cargo run -j 4 --manifest-path crates/dsr-cache/Cargo.toml -- extract   (also: hud, locomotion, survey, probe)
```
Live: `WOWSOULS_CHECK=8 WOWSOULS_SHOT=<png> WOWSOULS_SHOT_AT=5 target/debug/wow-souls.exe` (unattended; isolated saves).
`WOWSOULS_*` switches: table in `README.md`. Flat test ground: `WOWSOULS_AT=-8906,-136`.

## State

- **Warrior (2026-10-19):** Talents, signatures and perfect defense live-verified; user playtest pending. `docs/log/2026-10-19-warrior-talents.md`; philosophy: `docs/CLASS-DESIGN.md`.

- **Roads/remains (2026-10-18):** six civilians, extra houses removed; corpse compass. `docs/log/2026-10-18-road-life-and-remains.md`.

- **WoW-only:** local code/data, readable motion names, unchanged curves. Provenance: `anim/provenance/motion-names.tsv`; gates: `tools/Test-MotionNames.ps1`, `tools/Test-WowOnly.ps1`.
- **Westfall (2026-10-17):** 1,245 sites / 100 local types / 48 quests; combined 3,194 sites / 152 quests. Native motion for pending bodies. Roster and animation/event work: `docs/WESTFALL.md`.
- **Treasures**: 18 permanent Elwynn caches (6 supplies, 4 white, 8 green), all classes. `docs/ELWYNN-TREASURES.md`.
- **Soul binding**: one saved graveyard; grounded respawn. Tutorial 9000101: overhead '?' and compass; ordinary runes within 120 yd. `docs/log/2026-10-17-wow-names-memorial-westfall.md`.
- **Combat**: roll/backstep, sword R1/R2/rolling attack, guard/parry/block, Militia hammer grip, hit-stop/knock-back/shake, WoW blood, reactions/poise, walk/run/sprint (tilt fixed), inertia/lean/head look.
- **Enemies (2026-10-19):** 47 types / 15 roles; 140° sight + movement/action hearing. 1,360 tests + 5 live checks pass: `docs/log/2026-10-19-enemy-perception.md`. Family AI: `docs/log/2026-10-17-enemy-family-ai.md`. Separate Hogger; bosses/rares best first loot.
- **Player**: 19 slots + ammo, WoW armor/block, DPS-scaled moves, poise/attributes; Warrior Charge/Shout/Clap; Mage Fireball/Frost Armor/Frostbolt/Fire Blast, mana; souls shops, Abbey bank/tithe, level-up FX.
- **Warrior balance:** current scaling/live results in `docs/NEXT-AI.md`; original pass: `docs/log/2026-10-17-warrior-balance.md`.
- **Playtest fixes (2026-10-17)**: boar/spider/shout, sight and decor live-verified, not user-tested. Logs: `docs/log/2026-10-17-boar-spider-shout.md`, `docs/log/2026-10-17-boar-sight-decor.md`.
- **Presentation:** WoW UI/audio, `WOWSOULS_GFX=classic..ultra`, day/night/weather. Cloak/legs trial REVERTED at user's request; keep original behavior. `docs/log/2026-10-20-cloth-legs.md`.
- **UI/camera**: menus/3D roster, map/compass, saved area titles, lock-on swipes/follow, single death banner. `docs/log/2026-10-16-lock-on-stability.md`.

## Next / open

**Open tasks: `docs/NEXT-AI.md`.** Batch 3: `docs/log/2026-10-18-playtest-batch-3.md`. White rain fog remains unreproduced.

**Motion remix:** opt-in `WOWSOULS_MOTION=1`; `WOWSOULS_WOLF_MOTION=1` wolves only. Player timing kept; `docs/log/2026-10-15-motion-remix.md`.

**Elwynn:** S1–S3 accepted (25 species, 679 foe sites); S6 mechanics built, uncommitted; quest events open. Default `data/world/elwynn-westfall.txt`. Kits/status: `docs/log/2026-10-16-handover-spiders-latch.md`; implementation: `docs/log/2026-10-15-elwynn-implementation.md`.

1. **Mage:** S1–S6 built, live verified/reviewed, uncommitted. Playtest 2.5 s Fireball, mana/trickle and Frost Armor; S7 water/balance after that. Sources, timings and rulings: `docs/log/2026-10-14-mage-ledger.md`.
2. **Hogger / 14 Elwynn specials:** uncommitted; user judges feel. `docs/log/2026-10-16-hogger-rework.md`, `docs/log/2026-10-17-elwynn-specials.md`.
3. **Combined user playtest** after implementation: DS sprint/run (7/4.2 yd/s), footwork/numbers, audio, Hogger difficulty. Continue without per-family approval requests.
4. **Open items**: turn-in-place clips (`a00_0602`-`0605`), sheathing (none in DS/ER), Charge trail / shout glow draw nothing, pad L3 / D-pad-down untested, more classes after the Mage (Rogue first pick), release plan (`paths` module, first-run setup), commit question.

## Gotchas

- WoW coords: +X north, +Y west, +Z up; `wow_to_bevy([x,y,z]) = (-y, z, -x)`. Observer offsets are in Bevy axes.
- `cache/dsr` holds five stale `weapons-<stamp>` dirs (~310 MB); nothing reads them (do not delete without asking).
- Retarget root cause class: `wow_hip_root` must be the common ancestor of spine AND legs; guard with `Retargeted.body_height`.
- Unattended runs are silent unless `WOWSOULS_AUDIO=1|null`; scripted presses count from process start (a cold load shifts them).
- A trial copy and the real repo must not share `CARGO_TARGET_DIR` (stale wscl rlib).
