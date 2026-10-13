# Northshire hidden chest placement plan

The user explicitly asked to remove the chest blocking the chapel and spread chests into logical
places that require searching. This continues the approved optional-detour / fixed-reward design.
Execute in the existing checkout, without commits or another approval step.

**Goal:** Three authored supply caches on optional detours, with a clear chapel entrance and
working ground alignment, approach, lid animation and persistent independent claims.

**Architecture:** A small authored placement table in `src/loot/treasures.rs`; retain the old
Abbey chest ID and reward, add two unique IDs, and keep baseline WoW chest tables/spawns intact.
The native Northshire world supplies the scenery; no new terrain, mandatory jumps or quest props.

**Tech stack:** Rust / Bevy, benilla collision and native WoW TreasureChest01 animation.

1. Survey native scenery in actual game screenshots. Completed: chapel/rear supplies, the wooded
   tent camp and the northeastern camp clearing; both camp sites contain native crates/barrels.
2. Move `9000001` from `[-8906,-151]` to the storage behind the Abbey, initially `[-8896,-235]`.
   Keep three Salves and all previously saved open/claimed state; do not reset player progress.
3. Place `9000002` behind the wooded camp's canvas/boxes, initially `[-8768,-170]`, west-facing.
   Place `9000003` outside the clearing behind its supplies, initially `[-8786,-254]`, east-facing.
   Both have two usable Salves and a one-time claim. Adjust survey coordinates if live checks fail.
4. Validate every spot with the real motor and native Open/Take clips, not just a spawn screenshot.
   Check floor contact and lid clearance; walk in from the adjacent route. Verify independent saves
   and no repeat grants. Check the old doorway is traversable in the complete world with NPCs.
5. Run fmt, workspace clippy and tests with both install paths set; build, inspect final evidence,
   and update HANDOVER/build report. No edits to benilla, installed games or imported quest records.

The five original Battered Chest positions already cover Echo Ridge, the vineyard and the Defias
camp. They remain the baseline; the three authored caches bring the total to eight. Clues are the
existing camp supplies and the Abbey perimeter; no distant marker or chest sparkle reveals them.

**Completed:** all five steps, with live walk/open/take at each cache, full-world chapel traversal,
prior-claim preservation and 565 workspace tests. Final placements and evidence are recorded in
`docs/superpowers/specs/2026-10-09-treasure-placements-built.md`.
