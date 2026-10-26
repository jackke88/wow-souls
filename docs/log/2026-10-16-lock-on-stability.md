# Lock-on: deliberate gestures and continuous camera following

The user reported violent camera changes and repeated target switching from ordinary mouse
motion, and selected **clear horizontal mouse swipes, with small movements ignored**.
Claude's concurrent animation/combat work is outside this change. No commits were made.

## Causes and regression

- The old leaky mouse accumulator mixed camera sensitivity with target-switch intent. The
  threshold was about 29 pixels at sensitivity 1. A 40 px/100 ms movement switched at 100 ms
  at sensitivity 1, 50 ms at 2 and 33 ms at 3. The original implementation fails the new
  `ordinary_mouse_motion_does_not_switch_a_locked_target` regression (`cache/lock-on/red.log`).
- Switching assigned `cam.look = aim` immediately even while boom yaw eased. At an existing
  focus of 0.6, the old unobstructed left/right target example produced roughly 51 degrees of
  view-yaw change in one 60 Hz frame.

## Behavior and tuning

`lockon/input.rs` separates raw mouse pixels from stick/key input and camera sensitivity:

| Parameter | Value |
|---|---|
| Horizontal distance | 120 px |
| Stroke window | 200 ms |
| Minimum horizontal average speed | 800 px/s |
| Horizontal dominance | horizontal distance >= 1.7 × accumulated absolute vertical distance |
| Rearm | 120 ms neutral input; mouse speed <= 50 px/s |
| Shared cooldown | 450 ms |
| Stick/key thresholds | engage at 0.75; neutral below 0.25 |

One continuous mouse movement or held stick produces one request. Reversals discard the
partial stroke; cooldown input is consumed rather than queued. Menus, dialogue, freecam,
unlock, focus/cursor loss and >100 ms input hitches discard gestures. After returning, a
neutral dwell is required. The existing target selection, range and eligibility rules stay
the same.

`camera.rs` eases the focus point around the pivot (rate 12/s), follows boom yaw/pitch with
the existing exponential plus 180/90 degree/s limits, and caps the final unshaken view at
240 degree/s while locked. The previous base pose excludes hit shake. Free orbit responds
directly, including immediately after unlocking. Existing collision sweeps, body fade and
target-size rigs remain in use. Teleports can reset the view with the pivot.

Tiny focus angles finish with positional interpolation: Glam's rotation-arc identity
rounding otherwise stopped the focus a few millimetres short of its destination.

## Verification

The 29 lock-on/input and 23 camera tests pass. Coverage includes 30/60/144 Hz gestures,
small/slow/vertical movements, reversal jitter, continuous input, cooldown, stale input,
sensitivity 0.5/1/3 through the actual ECS switch system, UI/focus/cursor/freecam guards,
shortest-arc following, near-target view changes and settling.

Native replay uses `cache/lock-on/capture.ps1`, isolated character/quest/item/map saves and
`LOCALAPPDATA`, a quiet road fixture, and the real gesture detector. `WOWSOULS_LOCK_SCENE`
and `WOWSOULS_LOCK_INPUT` only create fixtures/replay in unattended mode. The `actors`
scene makes four stationary NPCs in its isolated manifest lockable for visual checking;
it does not change normal friendly NPC eligibility.

**Measurement finding:** the engine's `frame_pace.rs` deliberately gives `Time`/virtual
time a presentation cadence while `Time<Real>` measures the CPU/render handoff. A diagnostic
sample was 9.8758 ms Real versus 16.4167 ms camera time. Dividing the camera step by Real
reported a false overspeed. The final trace uses the camera clock and relative-quaternion
atan2 for small angular steps. Gameplay camera timing was not changed to compensate for
that measurement error.

All **13 native runs passed** the ECS check, expected switch counts and angular limits.
Measured maxima below are degrees/s against the camera clock (1 degree/s rounding allowance
in the harness). Free-orbit maxima are deliberately not sampled by the lock-only trace.

| Case | Switches | Max view | Max yaw | Max pitch |
|---|---:|---:|---:|---:|
| Small/vertical mouse motion | 0 | 79.24 | 0.00 | 33.06 |
| Two deliberate swipes | 2 | 175.09 | 126.78 | 33.12 |
| Continuous two-second movement | 1 | 175.73 | 126.80 | 33.10 |
| Held stick, rest, opposite stick | 2 | 176.05 | 126.78 | 33.10 |
| Cooldown discards middle stroke | 2 | 175.49 | 126.73 | 33.13 |
| Wide-angle changes | 3 | 219.16 | 180.00 | 45.73 |
| Near target circling player | 0 | 91.36 | 91.05 | 90.00 |
| Moving target | 0 | 79.46 | 31.46 | 33.12 |
| Menu input blocked, fresh stroke after close | 1 | 175.67 | 126.72 | 33.09 |
| Map input blocked, fresh stroke after close | 1 | 175.17 | 126.68 | 33.11 |
| Free orbit | 0 | — | — | — |
| Real stationary NPC models | 2 | 175.93 | 139.69 | 33.06 |
| Fixed-clock real-model video | 3 | 175.46 | 126.73 | 33.11 |

Evidence: **63 PNGs**, all over 1 MB, in `docs/evidence/lock-on/`; native summary
`cache/lock-on/native-results.log`. The [7-second preview](../evidence/lock-on/lock-on-preview.mp4)
is H.264, 1280×720, 30 fps, 210 frames. A 12-frame contact sheet around the first swipe
(`cache/lock-on/preview-switch.png`) confirms continuous reframing without a view snap.
The white reticle moves immediately to the selected target. Diagnostic capsules appear
black under the world's lighting; the final video and `actors-*.png` use real WoW models.
Gamepad gestures were verified through vectors/ECS/native replay; physical hardware was
not available for this check.

Gate snapshots (shared checkout is being edited concurrently):

- Scoped fmt and workspace all-target Clippy passed before the final QA-only clock change
  (`cache/lock-on/fmt-final.log`, `clippy-final.log`). The final instrumented game built
  successfully (`build-measure.log`); final owned-source rustfmt check also passes
  (`fmt-owned-final.log`).
- Full install-enabled tests: all 937 game tests, 90 dsr-cache and 52 dsr-formats tests pass.
  wow-world has 239 passing and one failure:
  `quest::rules::tests::class_and_race_masks_let_zero_and_listed_bits_through`, at
  `quest/rules.rs:381`: Dwarf eligibility returns `None` where the test expects
  `Some(NotPlayable)`. This is outside the camera/input change (`tests-final.log`).
- A subsequent whole-workspace check encountered newer concurrent quest-object edits:
  `quests/mod.rs:749` calls missing `objects::nearest`, and `quests/objects.rs:192` calls
  missing `GameObjectCatalog::get` (`clippy-verified.log`). Those features were not changed
  to make this camera task pass. Format-only fixes were applied to the narrow dirty lines
  reported in `npc.rs` and `quests/objects.rs`.

## Prior UI work

- [First-visit titles and free font](2026-10-16-area-title.md), with earlier UI links.
- [Hand-in compass](2026-10-16-compass.md).
- [World map and map-only quest seals](2026-10-16-world-map.md).
- [Character roster](2026-10-16-character-roster.md).

This work adds no dependency, generated art, extraction step or external API charge. Exact
model token usage/billing is not exposed in this session.
