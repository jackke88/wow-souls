# Elden Ring run, sprint and dodge investigation — 2026-10-16

User request: inspect ER running and dodge rolls as potentially more dynamic replacements for DSR.
Result: working local selective extraction, five decoded/retargeted candidates in ordinary Git project
files, and nine successful in-game recording checks. No production source or active default changed.
Common forward locomotion was compared; the other weapon carry sets remain unassessed.

## Assessment

My visual judgment after dense gameplay-camera and front sequences:

- **Sprint is the strongest replacement candidate.** ER carries the torso more upright and avoids
  the strong persistent sideways lean of the current DSR sprint. Both reach the same physical game
  speed in the comparison; their own stride curves/cadence differ.
- **Run is viable, with a subtler difference.** Its body/leg movement retargets cleanly. There is less
  reason to claim a large improvement from the rear camera alone; front footage is provided too.
- **Normal roll is a promising visual candidate**, with a more compact recovery. It also changes
  timing and root distance, so a permanent swap needs an explicit balance decision. The preview is
  useful for motion judgment, not proof of identical dodge mechanics.

Recommended implementation approach: adopt the ER run/sprint poses first while preserving physical
movement speeds, and separately decide whether the roll should retain our current duration/distance/
iframes/cancels or follow ER's. Alternative: keep DSR movement and replace sprint only (smallest scope),
or migrate the full ER movement/roll timing together (larger gameplay change). Do not copy the same
one-handed arm carry onto every weapon family. User preference was requested during investigation.

## Exact clips and measurements

| Source clip | Role | Frames | Full duration | Retargeted distance |
|---|---|---:|---:|---:|
| a000_020000 | walk | 41 | 1.333 s | 2.2695 yd |
| a000_020100 | run | 23 | 0.733 s | 3.3384 yd |
| a000_020200 | sprint | 18 | 0.567 s | 3.8808 yd |
| a000_027110 | medium forward roll | 51 | 1.667 s | 4.1382 yd |
| a000_027100 | light forward roll reference | 51 | 1.667 s | 4.9170 yd |

Current DSR medium roll a00_0710 is 2.033 s / 4.47 yd, with iframe window 0–0.367 s and earliest
cancel 1.100 s. The normal ER roll is identified from the original behavior graph, not guessed from
filenames: RollingMediumFront_CMSG → a000_027110. Light uses 027100, heavy 027120. Back/left/right
siblings end in 1/2/3. ER source run/sprint root speeds are 4.012 / 6.035 m/s before WoW scaling.

WoW retarget diagnostics: maximum checked direction drift run 3.619°, sprint 1.713°, roll 7.833°;
contact sole gap 0.0402 / 0.0360 / 0.0378 yd. These quantify retarget geometry, not visual quality.
There are 150 ER source bones and 119 WoW bones; original track→bone bindings were preserved.

## Roll gameplay distinction

Original a00.tae 27110 event 0, independently labeled by the
[DSAnimStudio ER template](https://raw.githubusercontent.com/Meowmaritus/DSAnimStudio/master/DSAnimStudioNETCore/Res/TAE.Template.ER.xml):

- Dodging flag 8: 0–0.433333 s baseline; 0.433333–0.533333 s has **StateInfo 290**, so is conditional.
- RH/LH attack cancel flags 4/16 start 0.666667 s; dodge input 25 starts 0.333333 s.
- Dodge cancel 26 starts 0.700 s; movement 11 starts 0.733333 s; guard 22 starts 0.833333 s.
- Event 67 adds a SpEffect; it is not itself an iframe marker.

The installed regulation's SpEffect rows 340700 and 20382404 have stateInfo 290, matching the
[Paramdex names](https://raw.githubusercontent.com/soulsmods/Paramdex/master/ER/Names/SpEffectParam.txt)
for roll-extension talisman effects. This supports interpreting that extra tail as conditional on an
extension effect. Do not grant it to the base roll. Full clip length also differs from the earliest
allowed action transitions; a full 1.667 s mandatory lock would not recreate ER's cancel behavior.

Prototype videos deliberately copy the current DSR action flag windows onto the ER visual roll.
They retain ER's source duration/root travel; thus balance is not fully preserved even in that prototype.
Candidate project files retain raw ER event rows in a separate TSV and leave runtime flags unset until
the chosen integration mapping is implemented/tested. The current `roll_spec` uses one earliest cancel
for several actions; faithfully separate ER cancel times would require further combat work.

## Extraction and source records

Read-only install: `C:/Spel/Elden.Ring.REPACK-KaOs`. No loose chr folder; selective BHD/BDT reads found
the required player binders in Data3. Only five binders (about21 MB) were extracted, not the whole game.
No install files were changed. The Oo2core6 library was loaded from the original install for extraction.
Neither source install is used when playing the resulting text candidates.

Scratch tools: `cache/er-motion-review/` (ignored). SoulsFormatsNEXT at
`ee1dd61958f60bdc51ce3da548e9a90a8ab39905`, SoulsAssetPipeline at
`d11caf989c917c7a43e2c7559915b1c5af218153`, plus
[UXM selective-unpack source](https://github.com/Nordgaren/UXM-Selective-Unpack).
SF was compiled into scratch under SDK 8 from its existing source with a net8 project; no upstream
source was edited. Havoc reads Havok 2018 external compendiums. The existing Rust spline decoder was
copied only into scratch and allowed up to 31 **zero** padding bytes (ER clips carry 16 trailing zeros
where the DSR reader allows 15). The checked run has zero float tracks. Production decoder is unchanged.

The existing human retarget was reused with original ER bindings and sampled root motion. Each output
was written/read as `.wsanim`; all five retargets completed. Ordinary project candidates:
[`anim/candidates/elden-ring/`](../../anim/candidates/elden-ring/README.md). Five WoW clips + five
comparison-rig clips, source event TSV, diagnostics and README. No raw game archives/keys/libraries are
added to the project. These remain source-derived reference material under SOURCE-TO-ORIGINAL.md.

## Live checks and reproduction

`cache/er-motion-review/preview.ps1 -Profile dsr|er -Case run|sprint|roll -View game|front`.
The ER preview library has hardlinks/junctions to the unchanged DSR library with copies of the two
edited tables; ER clips have distinct names. Original files were not modified through those links.
All saves are isolated. An absent DSR_DIR and ER_DIR and an isolated CWD are used during playback.
Drive 0.6/1.0 compares identical 4.2/7.0 yd/s movement inputs; rolls are requested every 2.5 s.

Nine captures: DSR/ER run, sprint, roll from gameplay camera; ER's three front views.
Each exited 0 with `WOWSOULS_CHECK ok` and 180 frames/6 s. Contact sheets were inspected at 4–5 fps,
including the full tuck and recovery. Videos are the review artifact. No Rust production code changed,
so the prior full workspace gates were not rerun for this investigation.

All 10 candidate project files also passed an independent parser/shape/finite-transform/normalized-rotation
check against the 119/150-bone rigs. All 13 pack files are in the Git index, none ignored; staged whitespace
check passes. No commit. The source-event TSV uses `-` for empty parameter-byte sequences.

Local evidence and interactive comparison (not uploaded):
`C:/Users/jacob/Documents/GitHub/wow darksouls/docs/evidence/er-motion-review/review.html`.
The active game still selects DSR by default. A permanent ER profile/default swap remains the next
implementation decision; this investigation does not claim to have replaced all weapon-set gaits.
