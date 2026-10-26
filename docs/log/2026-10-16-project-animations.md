# Complete editable animation library — 2026-10-16

The user requested the same editable-text treatment for every imported animation missing from the
project, and explicitly requested that the files be real Git project files rather than ignored exports.
The normal runtime now reads `anim/source/`. The project animation library needs neither a DSR install
nor the legacy binary animation cache. This is still DSR-derived reference content (`SOURCE_TESTED`),
not the independently authored replacement phase. The WoW install remains required for WoW models,
world data, audio and native M2 animations. No Elden Ring clips had entered this animation pipeline.

## Inventory and preservation

Before: 166 ignored flat player `.wsanim` files (59,151,011 bytes). Audited the active root cache and
the 19 families selected by its `weapons.txt`, excluding obsolete `weapons-<stamp>` snapshots and HUD
art. These contain 1,076 current binary clips (188,797,457 bytes), including gameplay and DS skeleton
comparison clips.

After: **1,161 editable `.wsanim` files** (323,033,001 bytes):

- 200 flat player clips: the existing 166 retained, plus 34 missing reaction/defense/interaction and
  related clips. Gaits, attacks, roll/backstep, guard, casts, hit reactions and deaths are covered.
- 961 namespaced creature, comparison-rig and weapon-family clips. Skeleton namespaces prevent
  different creatures' identically named clips from colliding.
- 25 tables: five root manifests/indexes, 19 stable family manifests and `inventory.tsv`.

That is **995 added animation files**. The inventory maps all 1,076 active binary clips to project
text; 85 additional preserved flat player overrides share names with weapon-family clips. All 200
flat player files were independently compared against their source clips: worst normalized quaternion
component error **0.00000011920929**. No prior editable file was overwritten. Every exported clip
passed numeric round-trip validation of transforms, scale, root motion, flags and hit windows.

`/anim/source` was removed from `.gitignore`. All **1,186 files under `anim/source/`** plus its parent
README and `.gitignore` were added to the Git index, with no ignore exceptions/force-add needed.
No commit was made, in accordance with the standing order.

## Implementation

- `wscl::text` writes version 2 and reads versions 1/2. Bone rotations remain editable degrees in
  YXZ order; non-unit scales are retained. Near-vertical rotations no longer lose small turns.
  Nonfinite values and invalid clip shapes fail parsing.
- `wow-souls-dsr bake-project --from cache/dsr --out anim/source` exports the complete active library,
  preserves editable files, validates every generated round trip and uses stable family paths.
  Original source-file checks become provenance comments in project manifests. The explicitly selected
  legacy extraction cache still checks its sources.
- The shared runtime reader loads text for player/creature/reference rigs. `anim/source` is the default
  root; `WOWSOULS_ANIM_DIR` overrides it. `WOWSOULS_DSR_CACHE` explicitly selects a legacy cache.
  Malformed text fails visibly rather than silently playing a binary substitute.
- Mandatory project tests cover inventory completeness, all gameplay loaders, 19 weapon families,
  rig namespace separation and malformed text. They do not depend on installed source games.
- Legacy HUD comparison still uses its own optional cache path; animation loading no longer selects it.

## Verification

Scoped format gate and strict workspace Clippy passed. Full workspace tests passed with both install
variables and an absolute legacy cache path: **1,355 tests** (91 extractor, 52 formats, 960 game,
241 world library, 4 world CLI, 7 wscl). The full run exposed one stale defense test: it expected no
block during a guard-raise interval whose imported guard flag is already active. Only that test's
expectation/stamina setup changed; production combat behavior did not.

The final executable also ran from an isolated working directory with no `.wscl` files or `cache/dsr`,
an absent `DSR_DIR`, and the project text directory selected. Startup loaded 66 gaits, 100 moves,
16 reaction clips/two deaths, all 19 weapon families and the creature kits. Gait, attacks, defense,
enemy/reaction and chest interaction checks exited successfully. Saves were isolated from the user's.

Paired six-second/180-frame project-text and legacy-cache recordings were made. Whole-frame PSNR:
walk 48.029, attack 51.498, defense 57.673, interaction 54.879 dB. Enemy AI attack timing diverged
between runs (24.136 dB), so that pair is only a loading/motion check, not an equality measurement.
Numeric comparison of every source clip establishes curve fidelity independently of AI timing.
Additional whole-body front and gameplay-camera sequences were inspected. Initial close/fence-obscured
recordings are retained as diagnostics rather than treated as sufficient animation evidence.
Final enemy checks on open ground show both kobold attacks and the player's small/medium hit reactions
from the gameplay camera and a clear front view (`project-enemy-field[-front].mp4`). Both completed
180 frames and `WOWSOULS_CHECK ok`; the player took 21/32 damage and recovered through the imported poses.

Evidence is local and unuploaded under
`C:\Users\jacob\Documents\GitHub\wow darksouls\docs\evidence\project-animations\`.
Reproduction harness, original logs, videos and comparison metrics remain in
`cache/project-animations/`; final gate logs are `cache/project-animations-*-final.log` and
`cache/project-animations-clippy-corrected.log`.

See [editing/export guide](../../anim/README.md),
[source-to-original policy](../SOURCE-TO-ORIGINAL.md) and
[implementation plan](../plans/2026-10-16-project-animations.md).
