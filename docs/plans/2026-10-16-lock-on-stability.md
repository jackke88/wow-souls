# Lock-on stability implementation plan

**Goal:** Preserve the user's chosen mouse-swipe target switching while stopping accidental switches and abrupt camera jumps.

**Architecture:** Keep target selection/gameplay immediate. Separate raw-pixel mouse gestures from free-camera sensitivity and from stick/key input; require a deliberate horizontal stroke, neutral dwell and cooldown. Smooth the visual focus and bound camera rotation without feeding camera shake back into the base pose. Touch lock-on/camera only; animation, combat and target eligibility remain separate parallel work.

**Tech stack:** Existing Rust/Bevy input, camera math and unattended fixtures; no dependencies.

## Root cause evidence

Existing `Flick` multiplies mouse pixels by `settings.game.mouse`, then thresholds the leaky sum at about 29 px at sensitivity 1. Replaying the same 40 px/100 ms motion fires at 100 ms with sensitivity 1, 50 ms with sensitivity 2 and 33 ms with sensitivity 3. Camera `orbit_input` assigns `cam.look = aim` on a switch while focus is already 0.6, so `seat` immediately looks toward the new point even while boom yaw eases. An unobstructed left/right target pair gives about 51 degrees of view-yaw change in a single 60 Hz frame using the old equations.

The user explicitly chose: keep switching with a clear mouse swipe; ignore small mouse movements. This authorizes that direction. No design approval is pending.

## Steps

1. Add regression coverage for small/slow/vertical motion, deliberate left/right swipes, one switch per stroke, rest/cooldown, frame rates and camera continuity. First demonstrate the original behavior fails the regression.
2. Replace sensitivity-dependent leaky mouse accumulation in `src/lockon.rs` with a bounded raw-pixel gesture window, horizontal dominance, neutral re-arm and common cooldown. Keep stick/key thresholding and scripted direct switches. Reset gestures behind menus/dialogue, on cursor/focus loss, freecam and unlock.
3. In `src/camera.rs`, ease the focus point, bound boom yaw/pitch and the unshaken view rotation. Normal free-camera mouse input retains its direct response. Preserve collision sweeps, body fade, shake and existing size rigs. Use short-arc, time-based math and handle initialization/teleport safely.
4. Run headless input/system regressions and real dense switch frame sequences with isolated saves. Verify small mouse motion causes no switch, strong horizontal strokes produce exactly one, held inputs/reacquisition do not repeat, and direct changes preserve immediate target state while camera rotation stays bounded. Keep strong visual evidence for front/nearby/moving targets and free orbit.
5. Run scoped fmt, workspace Clippy and full install-enabled tests with `-j4`; build and document thresholds, evidence, controls and findings in `docs/log/2026-10-16-lock-on-stability.md`, update slim handover. No commits.
