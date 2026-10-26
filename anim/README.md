# Editable project animations

This is the normal runtime animation library. `source/` is part of the project and is **not
gitignored** (Jacob's instruction, 2026-10-16). It contains the exact imported DSR reference
animations as editable text, plus the tables needed to play them. A DSR install and the old binary
`cache/dsr` are not required for the animation library. Source-derived curves are still reference
material; making independently authored replacements remains a later step in
[`SOURCE-TO-ORIGINAL.md`](../docs/SOURCE-TO-ORIGINAL.md).

Elden Ring run/sprint/roll candidates and their comparison notes live separately in
[`candidates/elden-ring/`](candidates/elden-ring/README.md); they do not change the active defaults.

Current audit (2026-10-17): **1,203 project clips are indexed**, including the 42 newer Hogger-knight clips. The original export inventory/counts below describe the earlier complete-library migration; the current gameplay loader tests cover the active kits.

## Readable names (2026-10-17)

The 1,203 active clips use project names: body_roll, body_sprint, straight_sword_light_1,
wolf/body_attack_01 and kobold/body_reaction_01. Reference skeletons use folders such as
human_reference and kobold_reference. Runtime modules are motion_clip, locomotion, rig_view
and animation_library.

Attribution remains in Inspiration comments and in
[provenance/motion-names.tsv](provenance/motion-names.tsv), which maps every previous path to
its current path and records a SHA-256 of the unchanged numeric motion payload.
Run tools/Test-MotionNames.ps1 to verify this migration. Intentional future motion edits need
their own recorded revision. Renaming does not make source-derived curves independently authored.

## Original migration inventory

- 200 flat player `.wsanim` files: gait, attacks, rolls/backsteps, idle/guard, casts, hit reactions,
  death, parry/guard responses, chest opening and pickup. Existing 166 editable files were retained.
- 961 namespaced `.wsanim` files: creature kits, DS comparison skeletons and active weapon families.
- `manifest.txt`, `locomotion.txt`, `casts.txt`, `interaction.txt`, `weapons.txt` and 19 stable
  `weapons/<family>/manifest.txt` tables. Runtime source-file checks are replaced with provenance
  comments in this project copy; original extraction caches keep their strict checks.
- `inventory.tsv` records 1,076 current binary-cache clips and their project equivalents. The 85
  additional existing flat player overrides serve clip names from the weapon families. Total:
  **1,161 editable clips**, about 323 MB (largest file about 1.4 MB).

The directories separate skeletons: `wolf/body_attack_01.wsanim`, `kobold/body_attack_01.wsanim` and
`human_reference/body_attack_01.wsanim` are different animations/rigs despite sharing a name. The flat player file
is the editable override shared by weapon-family loaders. Keep file names and skeletons intact.

## Edit

Open a `.wsanim` file, change it, then restart the game. For example:

```text
wsanim 2
name body_sprint
fps 30
model_space 0
bones 119
parents -1 0 ...
driven 0 18 ...
frames 19
frame 0 root 0.000000 0.000000 0.000000 0.000000
b 18 0.00000 0.00000 0.00000 0.000000 0.000000 0.000000
```

Every bone line is `b INDEX PITCH YAW ROLL X Y Z`: rotations in degrees, bone-local translation in
yards. Rotation order is `Ry * Rx * Rz` (`EulerRot::YXZ`). Non-unit scale adds `SX SY SZ` at the end.
The frame's root line stores displacement X/Y/Z in yards and yaw in radians. `flag`/`hit` lines
retain the actual gameplay event windows in seconds. `model_space 1` identifies model-space rigs.
Versions 1 and 2 are readable; the exporter writes version 2. Do not remove frames/bones or replace
values with NaN/Infinity. A malformed project file is reported, with no silent binary replacement.

`WOWSOULS_ANIM_DIR` selects another editable library. The retired `WOWSOULS_DSR_CACHE` setting is ignored. Missing or malformed text never falls back to binary files. Native WoW M2 animations remain in the separately required WoW data. Package anim/source beside the executable, or configure WOWSOULS_DATA_DIR.

## Export / verify

```powershell
cargo run -j 4 --manifest-path crates/dsr-cache/Cargo.toml -- bake-project --from cache/dsr --out anim/source
cargo test -j 4 -p wow-souls animation_library::
cargo test -j 4 -p wscl
git check-ignore --no-index anim/source/body_sprint.wsanim
```

The last command should report no ignore rule (exit 1). Export validates each binary-to-text round
trip, including scale, root motion and event windows, and preserves existing editable files. An
existing manifest with different content stops export instead of overwriting an edited table.
`bake --clips ...` remains the explicit single-player-clip re-export tool and overwrites named files.

History, counts, source-free loader checks and motion evidence:
[`2026-10-16-project-animations.md`](../docs/log/2026-10-16-project-animations.md).
