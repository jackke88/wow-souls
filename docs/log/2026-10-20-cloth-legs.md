# Mantel och ben från sidan — 2026-10-20

User: the cloak looks see-through; legs look strange while running from the side. Reference screenshot uses the player's short pointed cloak (item 3070) and two-handed hammer (2480).

**REVERTED AT THE USER'S REQUEST.** User rejected the changes and explicitly asked to restore the behavior from before this task. All cloak/body collision, tether, diagnostics and run/sprint leg fitting code introduced here has been removed. The original cloak file is restored; the existing locomotion system is retained without the added leg fit. No source animation files were changed. Do not resume this task without a new request. The investigation below is historical, not the current implementation.

## Investigation and changes

The live material probe reports cape geoset 1505, 28 authored vertices, `Opaque`, `two_sided=true`, no vertex alpha. The cloth is already opaque on both faces. Its old collision constraint was only the skinned cape surface: the body or a limb could cross it during a pose. Added posed torso, thigh, shin and arm capsules for the current HumanMale avatar, with 0.012 yd clearance. Torso volumes are elliptical (1.6 times as wide as deep, following the posed chest); the first round-volume capture still missed the broad back at the cape edge, caught in visual QA despite low capsule penetration. Contact is solved with the cloth constraints, including reset/paused frames; shoulder pins stay attached. Collision projection translates the previous Verlet point too, avoiding an artificial launch. The existing wind, authored texture and two-sided material remain.

Leg measurements match the DSR reference's lengths and bend angles; this is a fit/readability issue rather than inverted joints. Source sprint recovery folds the knees as far as 153.7 degrees (common set), 137 degrees with the hammer, producing the high heel kick on bulky WoW boots. Free run/sprint clips now fit the airborne recovery to a 110-degree bend ceiling with two-bone IK. The ankle lowers only when over-folded; its horizontal track and model-space orientation remain. The pelvis, upper body, bone lengths, clip/root timing and planted frames remain. Walk, locked steps and action clips are not fitted. Source files are unchanged; the fit happens on the loaded clip in memory.

Code: `crates/wow-souls/src/cloak.rs`, `cloak/body.rs`, `locomotion.rs`, `locomotion/legs.rs`. A temporary material trace in `actor/mod.rs` was removed after diagnosis. `WOWSOULS_CLOAK_TRACE=1` logs fitted cloth body count and residual penetration for live verification.

## Verification

- Regression before collision: `the_body_and_moving_legs_stay_inside_the_cape_even_when_the_skinned_target_does_not` failed at frame 0/node 5. With contact solving: all 15 cloak-filtered tests passed.
- New real-clip regression checks all six weapon locomotion families, run and sprint: bend, bone lengths, ankle ground track/orientation, planted frames, root timing and unchanged upper body. Walk and reference skeleton bypass are covered.
- Final format gate and workspace clippy (`-j 4`, all targets, `-D warnings`, offline): pass.
- Full workspace tests with WoW data, offline and `-j 4`: **1,365 pass** (1,094 game, 259 world, 4 integration, 1 wscl, 7 doc tests). Log: ignored `cache/cloth-legs-check/tests-verified.log`.
- The broad-back regression covers the edge missed by a round torso without inflating back depth. A UV-split hypothesis was ruled out against the actual HumanMale cape geometry; no seam/stitch change is retained.
- Final binary built with `cargo build -j 4 -p wow-souls --offline`; final format/clippy gates pass.
- Six initial `WOWSOULS_CHECK ok` runs, 150 recorded frames each at 30 fps: hammer attack back/front, sprint side/front, run side, turning in the game camera. `final-*` videos/screenshots are the first candidate, subsequently rejected by the user; preliminary `after-*` captures used the rejected round torso volume.
- All final traces use 325 cloth nodes and 10 posed body volumes. Maximum residual volume penetration: 0.00070 yd during the hammer attack (under 0.7 mm), zero in running/sprinting/turning samples. This measures collision-volume contact; visual inspection remains necessary for actual mesh coverage.
- Inspected dense back-attack, side-sprint and front-sprint/attack sequences. The broad back no longer cuts the cape into the jagged edge visible in the first collision pass; heel recovery is lower while weapon carry and planted strides remain. Cloth still billows and folds with wind; user judgement of the fit remains open.

Baseline and complete-library 110/120-degree candidate previews recorded using isolated copies of slot 5. Equipment/character/quest overrides are explicit because unattended mode otherwise uses separate defaults. An initial bare-body fixture and a partial-library candidate were invalid controls and excluded; complete-library previews loaded `great_hammer` correctly. No original character files were written.

Evidence: `C:/Users/jacob/Documents/GitHub/wow darksouls/docs/evidence/cloth-legs/`. Reproduction script/snapshot: ignored `cache/cloth-legs-check/`. The old binary is kept as `before.exe`. Temporary candidate animation trees/binary copies were removed after filming to recover disk space. A full test link hit LNK1201 writing its PDB; the failed generated PDB was removed and the small Bevy wrapper cache refreshed after an E0463 retry. Source/dependency code was not changed for those build failures.

User playtest and subjective movement approval remain pending. No staging/commits.

## Correction after rejection (same day)

Reproduced the cloth collapse with the same equipment in **High**, the user's saved preset (the original QA was Classic). An isolated live dump at 2.5 s measured the authored hem target at y=0.926 yd but simulated hem at y=1.375 yd: about 0.45 yd of unintended lift. Nearest-surface capsule projection moved nodes sideways/up over the shoulder; the spherical 0.55 yd tether also allowed the hem to gather there. Low penetration against those volumes did not prove a correct cloak.

Changed contact to exit the convex body volume along each node's posed outward direction, preserving its position along the cloth instead of choosing the nearest radial exit. The tether permits outward billow but limits tangential slide to 20% of its former reach. Shoulder pins and the existing opaque/two-sided material remain. Added regressions for directed contact and a hanging hem under body contact plus wind. The directed-contact test was compiled in isolation against both the old and corrected capsule calculations: old fails (101), corrected passes (0).

Format, full workspace tests (**1,367 pass**) and all-target workspace clippy pass; `target/debug/wow-souls.exe` rebuilt. Temporary CSV diagnostics were removed from production source. Source scope this follow-up: `cloak.rs`, `cloak/body.rs`; leg fit unchanged.

Live correction captures use `correction-*` in the same evidence folder, all High. Six checks/captures passed and dense rear/front attack and side-running sequences were inspected: the hem no longer gathered at the shoulders. The user's clarified world-through-whole-figure symptom was not reproduced. Before reporting this candidate, the user requested full rollback of this cloak/legs task; the candidate was removed along with the initial attempt.

Rollback verified: cloak file hash exactly matches its pre-task Git version (`c019fef9501413e14600f2a04d4d841d62929a90`); added body/leg modules and all hooks are absent. Format gate, 13 original cloak tests and 18 original locomotion tests pass. The restored game was rebuilt and a High-preset isolated-save launch returned `WOWSOULS_CHECK ok` (`rollback-idle-back-cloth1-motion0`). Earlier Warrior/perception changes and user saves were retained. No staging/commits.
