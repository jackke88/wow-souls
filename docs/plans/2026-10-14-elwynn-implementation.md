# Elwynn Forest Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** implement Elwynn in the approved roster order, with database placement and Souls attacks that fit each WoW body.

**Architecture:** extend the existing world importer from Northshire to a selectable zone and preserve faction, display and pool metadata. Build creature families as shared source kits plus target-specific rig maps; each stage is tested and checked live before the following family. Existing changes stay in the current checkout; no commits without the user's request.

**Tech Stack:** Rust, Bevy 0.18.1, benilla-formats, WoW 1.12.1 MPQ/DBC, vMaNGOS patch-10 exports, DSR HKX/TAE and existing wscl extractor.

## S1 — zone and spawn foundation

1. Add backwards-compatible world metadata records for faction reaction, display weights and creature pools. Optional pool exports keep old TSV folders usable. Files: `crates/wow-world/src/{model,db,manifest}.rs`, new `pool.rs`; meaningful parser/round-trip/pool tests.
2. Generalize selection to caller-supplied zone roots and three-dimensional positions. Keep the existing Northshire API/default usable; add Elwynn area 12, bounds with a coverage margin and WMO interior overrides. Files: `select.rs`, `valley.rs`, importer `main.rs`. Tests cover Northshire inclusion, neighboring-zone rejection, Stormwind WMO rejection and mine interiors.
3. Select one alternative creature per GUID, preserve pool limits before density thinning, use imported reactions for flagless friendly NPCs, and use weighted displays. Files: `crates/wow-souls/src/{world,friendly,critter}.rs`. Existing manifest versions retain their old fallback behavior. Unsupported enemy rigs remain explicitly unmapped.
4. Import to `cache/world/elwynn.next.txt`, inspect data invariants, then make the verified Elwynn manifest the default for world/quests/loot/shop while retaining explicit `WOWSOULS_WORLD_FILE` override. Do not overwrite Northshire.
5. Run scoped formatting, workspace clippy and install-enabled tests, then unattended checks in Northshire, Goldshire, Eastvale and Hogger's area. Update handover state and log with evidence.

## S2 — existing kit families, in roster order

1. Add Kobold Miner, Tunneler, Goldtooth, Ruklar and Narg species rows using the existing kobold rig/kit; retain named/rare rank, DB equipment and their distinct tuning/pattern. Make ordinary melee use at most two selected moves rather than inherit all Legacy attacks.
2. Add Mangy Wolf and Gray Forest Wolf on the existing wolf rig; add Prowler only after its distinct DireWolf retarget is verified. Files: `species.rs`, `world.rs`, `pattern.rs`, extractor target registry as needed.
3. Verify source timing/data, all required equipment and kill-credit entries, and dense fight sequences in motion. Fix faults before continuing.

## S3 — Defias bodies and enemy casts

1. Generalize target kits to human female as well as male, with actual display and gear; add Cutpurse, Bandit and named melee rows. Choose a real 2H swing for Erlan and an actual kick for Snap Kick roles.
2. Add Kobold Geomancer (`c4160`) and human caster source (`c2650` or existing Mage source clips); connect enemy cast release, swept projectiles, interruption and Frost Armor. Keep Common at two attack roles and avoid the sorcerer's barrage.
3. Add Dockmaster's goblin rig with an unarmed source, then the database-driven Bodyguard/Clara summons. Validate female/goblin motion, weapon contact and cast release.

## S4 — gnolls, then murlocs

1. Survey/select single-hit ordinary gnoll attacks (`c2550`; `c2810` alternative). Map Gnoll and GnollMelee separately; Hogger's parked boss kit is not the ordinary gnoll moveset.
2. Build Murloc target rig and weapon grips for sword/club/scimitar/dagger with `c2550`/`c2520`. Add Lurker's positional attack and Forager's potion.
3. Check directions, hip root, ground contacts, hit volumes, death settlement and live fights before accepting.

## S5 — boars, spiders, bear

1. Boar: try `c3460` head/charge motions; preserve source data and explicitly separate ordinary gameplay reaction if adaptation is needed. Do not silently loosen `Tier::Common` or grant boss knockdowns. `c1200`/native WoW attack is an alternative if the source cannot fit.
2. Spider: independently map Tarantula and MineSpider to selected `c3400` melee clips; leave fire spray out. Add poison and a readable Mother Fang web. Fall back to actual WoW motion with measured timing if DS topology fails.
3. Bear: try suitable short bite/swipe references; no direct DSR bear exists in the survey. Keep native WoW bear motion as the baseline until a new target passes motion review.

## S6 — named mechanics and zone completion

Add named effects, escort/summon events, quest-linked objects/counters/rewards and loot. Preserve one-active Hogger/Thuros pool semantics, quest floors, path/wander rules and player-safe respawn. Return to Hogger's parked motion quality with fight-series evidence. Seasonal events and the remote cliff scenario are separate, explicit later implementations; do not turn event rows into permanent base-zone spawns.

## Validation commands and completion ledger

- Formatting: `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check`.
- Checks/builds: `cargo clippy -j 4 --workspace --all-targets -- -D warnings`; `cargo test -j 4 --workspace`, with `DSR_DIR` and `WOW_DATA` set in that shell.
- Import: `wow-souls-world import --zone elwynn --wow <Data> --tsv <exports> --out cache/world/elwynn.next.txt`.
- Evidence and per-stage results: `docs/log/2026-10-15-elwynn-implementation.md`.
- Status is updated only after tests and the live check; source candidates are not accepted from a still image.

Current: S1 and S2 accepted. S3's HumanFemale target, six initial melee entries, Erlan's actual 2H attacks and Bandit/Dockworker sword + kick roles are implemented and verified in motion: 25 species, 679 foe sites before density. The kick has an explicit small-flinch gameplay adaptation with original source data retained. Northshire starters retain sight aggression; James retains neutral retaliation. Next Geomancer/Rogue Wizard/Surena casts, then goblin/summons; S4–S6 pending. User will judge everything together in a later large playtest; keep internal gates without per-family approval requests.
