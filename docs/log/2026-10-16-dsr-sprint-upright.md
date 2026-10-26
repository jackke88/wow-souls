# DSR sprint: remaining sideways lean fixed — 2026-10-16

**User decision:** keep DSR's animations. Fix its sideways sprint lean; do not adopt ER movement or
roll timing/distance. The earlier ER comparison/candidate pack remains reference material only.

## Root cause and correction

The old correction averaged the chest bone's up-axis, then split the correction 35/65% across spine
and chest. A straight chest orientation did not center the actual anatomical torso: the real
a00_0535 spine-to-neck line retained about **4.485°** of sideways bias. An independent numeric probe
also checked all six sprint sets. Reading stale parent matrices alone explained only about 0.1° of
the common sprint's orientation error, so that was not the primary cause.

`dsloco.rs` now measures sprint's actual mean spine-to-neck model-space line. Its bias is carried
separately with the sprint blend weight and applied once at the spine base in `lively.rs`. The other
gait roles keep their existing measurement and split correction. This centers the mean torso without
flattening the cyclic sway or forward hunch. Original `.wsanim` files, frame rate, stride/root motion,
movement speeds, stamina, action/cancel/iframe windows and selected DSR animation families are intact.

The additive edit also now derives its parent rotation from current local transforms. `PosePost`
runs before model composition, so the old `RigPose.model` lookup described a previous pose, including
old additive edits. Fresh parent axes are necessary when combining the base correction, acceleration
lean and head look. No engine/fork edits were required.

## Verification

- New project-sprint geometry regression and stale-parent regression both failed before the fix and
  passed afterward. The geometry check uses the actual project clip, not an installed cache.
- A third regression runs the real `lively::drive` post-pass in an ECS app for every frame of the
  actual sprint: mean lateral torso offset below 0.00001 yd; foot positions and mean forward hunch
  preserved within 0.00001 yd. The fixture initializes a settled sprint so startup acceleration lean
  does not contaminate that invariant; a diagnostic run caught this fixture issue before correction.
- Full workspace tests with `DSR_DIR`, `WOW_DATA` and absolute legacy-cache override: **1,358 passed**
  (91 extractor, 52 formats, 963 game, 241 world library, 4 world CLI, 7 wscl). Strict workspace Clippy
  and scoped fmt passed. Build uses `-j 4`; no production source changed after that build, only tests.
- Two before and eight after live captures: front/game sprint, stop, attack, roll, turning, normal
  run, and a Militia Warhammer (5579) selecting `great_hammer`. All exited 0 with `WOWSOULS_CHECK ok`
  and 180 frames/6 s. Dense sequences were inspected from both camera directions. The corrected
  sprint keeps its forward hunch and step cycle; stop/attack/roll transitions return normally.
- Runtime roll remains **a00_0710: 2.033 s / 4.47 yd, iframe 0–0.367 s, cancel from 1.100 s**. Source
  playback still loads the project library (66 gaits, 100 player moves, 19 weapon families).

Isolated saves and source-free playback checks use `cache/sprint-tilt/run.ps1`; original logs,
numeric probe and captures remain there. Final gates: `tests-final.log`, `clippy-final.log`, `fmt-final.log`.
The uncommitted source changes are confined to `dsloco.rs` and `lively.rs`.

Local evidence (unuploaded):
`C:/Users/jacob/Documents/GitHub/wow darksouls/docs/evidence/dsr-sprint-upright/`.
`comparison-front.mp4` and `comparison-game.mp4` show before/after side by side. The original DSR
animation files and ER reference candidates were not edited. No commit was made.
