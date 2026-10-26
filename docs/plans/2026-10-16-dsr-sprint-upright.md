# DSR Sprint Upright Implementation Plan

**Goal:** Fix the remaining sideways sprint lean while retaining DSR animations and all movement/dodge balance.

**Architecture:** The existing chest-orientation measurement plus split spine/chest correction leaves the
actual spine-to-neck line about 4.5° off-center in a00_0535. Measure the sprint's anatomical torso line,
apply that constant bias at the spine base, and blend it only with the sprint's contribution. Other
gaits retain their current correction. Read current local parent rotations during additive edits:
PosePost runs before model composition, so `RigPose.model` contains a previous pose.

**Tech Stack:** Existing Rust/Bevy RigPose post-pass, DSR `.wsanim` library, existing motion capture tools.

The user explicitly chose to keep DSR and fix sprint (2026-10-16). No approval/commit requested.
Source scope is `dsloco.rs` and `lively.rs`; preserve shared edits and all animation files.

1. Confirm numerical bias against the real project sprint and retain before motion captures.
2. Add regressions for anatomical sprint alignment and a post-pass edit with a stale model buffer;
   demonstrate both fail before the correction.
3. Measure the mean spine-to-neck model-space vector for sprint, carry a separate weighted correction,
   and rotate at the spine base. Preserve cyclic sway, forward pitch, feet, root motion and gameplay.
4. Derive additive edit axes from current locals rather than stale composed matrices.
5. Run focused regressions, full install-backed workspace tests, scoped format and strict Clippy.
6. Compare front/game-camera sprint sequences, other weapon carry sets and roll/attack transitions;
   record evidence and update handover/ER comparison decision. No commit.
