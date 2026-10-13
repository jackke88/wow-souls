# Hidden Northshire caches — built, 2026-10-09

The user asked to remove the chest obstructing the chapel and spread treasures into logical
places that require searching. The three authored supply caches now sit on optional detours,
next to native supplies. The five imported WoW Battered Chest positions, loot tables and
restock timers remain intact: eight chests in total. Nothing is committed.

## Placement and purpose

| Persistent ID | Place / environmental clue | WoW position after ground snap | Facing | Fixed reward |
|---|---|---|---|---|
| 9000001 | Storage behind the Abbey, beside existing crate, barrel and fence | [-8896, -235, 80.259] | North | 3 Abbey Salves |
| 9000002 | Woods side of the canvas shelter, behind its boxes and barrels | [-8768, -170, 83.125] | West | 2 Abbey Salves |
| 9000003 | Outer edge of a separate forest camp, beyond its supplies | [-8786, -254, 82.477] | East | 2 Abbey Salves |

The first cache previously stood at [-8906,-151] in the chapel entrance. Its ID and reward are
unchanged, so previously opened or claimed state survives the move. The two new IDs have their
own pending and permanent claim states. No reset of the player's progress is needed or performed.

Existing shelter/supply props explain why someone stored a chest at each place. The Abbey itself
screens the rear cache from the entrance; reaching either camp requires leaving the main route.
The woodland cache is on the back of the shelter, and the other camp's cache is beyond its crates.
There are no new distant markers, chest sparkles, mandatory jumps or changes to terrain/quest props.
The chest fronts face clear walkable ground and the animated lids clear the adjacent scenery.

`crates/wow-souls/src/loot/treasures.rs` owns the small placement/reward table. `loot/mod.rs` now
carries each authored reward on its chest instead of hard-coding three salves for every fixed chest.
Ground alignment, approach, native lid clips, DS player clips and save-first acquisition are reused.

## Live verification

The rebuilt executable passed all unattended ECS fault checks. Each cache was approached using
the real walking motor, then opened and collected with the native Open/Take animation loop:

| Check log in cache/treasure-work | Approach / result |
|---|---|
| abbey-walk-loot.log | Walked from [-8890,-235] to the north-facing front; opened, took 3 |
| woodland-walk-loot.log | Walked from [-8768,-163.5] to the west-facing front; opened, took 2 |
| camp-walk-loot.log | Walked from [-8786,-260.5] to the east-facing front; opened, took 2 |
| chapel-passage.log | Full world with NPCs: walked from [-8906,-145] through the old chest location to [-8905.81,-166.06] without obstruction; all 8 chests spawned |
| old-claim-relocated.log | Copied the earlier doorway-check save to an isolated file; the relocated chest loaded Claimed, repeated interaction gave nothing, SHA-256 remained identical |

The first three runs shared an isolated save. Its salve count advanced 3 → 5 → 7, with all three
distinct chest IDs permanently claimed. Restarted runs loaded the earlier claims correctly.
The normal player save was never used. Actual screenshots were inspected for ground contact,
prop overlap and lid clearance. The chapel observer camera ended inside a wall; that frame is
not visual proof of the passage. The motor trace above is the traversal proof.

Evidence screenshots are in the session's `docs/evidence/`: `treasure-abbey.png`,
`treasure-woodland.png`, `treasure-camp.png`, and `treasure-woodland-preview.png` (normal orbit camera,
developer overlays hidden). These are game renders, not mockups. The pickup/preview runs disabled
creatures to inspect scenery and interactions; the separate full-world woodland check kept the
native camp workers, which aggroed normally. The cache does not replace their spawn records.
The pickup frames show the native art from the previous loot slice.

## Gates

`cargo fmt --all -- --check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`, and
`cargo test -j 4 --workspace` passed with DSR_DIR and WOW_DATA set. **565 tests passed** across
dsr-cache 53, dsr-formats 51, game 356, wow-world 97, world CLI 4, wscl 4. The count includes the
parallel quest foundation work. Logs: `cache/treasure-work/{build,fmt,clippy,tests}.log`.

The installed games and pinned benilla were not edited. The placement work is complete;
connecting loot to the real gameplay quest log remains the separate G3 integration step.
