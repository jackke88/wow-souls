# Motion remix: the wolf method for every animation, 2026-10-15

User, after the wolf trial: "läs rapporten angående animationerna och gör det för alla animationer vi har. Rör inget med UI."
Nothing in the UI was touched (no `hud/`, `menu`, `loot/ui` files). Nothing is committed.

## Switch

`WOWSOULS_MOTION=1` remixes everything below. Default off: the DSR originals stay the standard. `WOWSOULS_WOLF_MOTION=1` alone still gives only the wolves' authored bite/lunge (unchanged). Flipping the default is one line (`remix::enabled`).

## Code

`crates/wow-souls/src/remix.rs` (child of `monster`, like `wolf_motion.rs`), about 1300 lines of which 500 are tests.

- Enemy kits: `monster::load` calls `remix::apply_loaded` per kit. Atomic: a kit whose rig does not match keeps every original clip and logs one warning.
- Player: `dsclip::read_clip` calls `remix::dress_player` for the `human_male` folder, which every player loader goes through (moves, roll/backstep, reactions, deaths, guards, interactions, casts). One hook, no loader touched.
- `wolf_motion.rs` now uses the shared `Clock` (the 5-knot copy is gone) and exposes `authored(kit, attack)` so the general pass leaves the hand-authored wolf curves alone.

## What a clip gets

| Layer | Applies to | What |
|---|---|---|
| Timing | enemy attacks | held, slower first half of the wind-up; shorter hit windows; longer recovery. One monotone clock carries pose, root motion, flags and hit windows; `AttackSpec` is rebuilt from the new clip |
| Coil | every attack (enemy and player) | trunk winds up away from the swing, holds, releases past neutral (`FOLLOW` 0.55) and settles. Direction read from where the striking hand really goes between the first hit start and the last hit end; head keeps half its gaze on the target; neck dips in the wind-up |
| Springs | every non-looping clip | neck, head, jaw, tail, ears are springs in model space: they lag a quick trunk, overshoot, settle, and fade to the source over the clip's last 0.05-0.25 s |

Weights (`LIGHT`/`HEAVY`/`BOSS`, from stagger level and poise damage for enemies, from clip length for the player) set the timing multipliers and coil degrees. Kinds: Attack, Hurt (flinch, death, parried), Evade (roll, backstep: small caps), Misc (guard, cast, interact), Loop.

Rigs: HumanMale (player, thug, Erlan, bandit, Garrick), HumanFemale (Morgaine, female bandit), Kobold, Gnoll (Hogger: jaw and 3 tail bones), Wolf (jaw, 5 tail, ears), DireWolf (jaw, 4 tail). Each is checked against the clip's parents before anything is written.

## Not touched, and why

- **Gaits and the stance**: first left alone (`lively.rs` dresses them); dressed in the follow-up below with head and neck springs only, so `lively`'s lean and look-at are not doubled.
- **The player's timing**: moves keep DS timing to the frame. Their hit windows, cancel windows and i-frames are manifest and DS data (`hit` lines, `preact` chain/evade/move times, guard clips), so a retime would need every one remapped. The player gets coil and springs only. Retiming them is possible (remap those manifest numbers through the same clock) but changes how the player's own combat feels; not done unasked.
- **Enemy flinches, deaths, parried**: springs only, same length (stagger length and the death hold are balance).
- **Wolf bite/lunge**: kept as the authored recipes.
- Legs, arms, translations, scales, root motion: the source's in every clip.

## Enemy attack timing (source → remix, hit window)

kobold small_1 2.87→3.13 s (0.77-0.93 → 0.86-1.01), small_2 2.17→2.35, big 3.17→3.72, run_strike 3.17→3.70, body_check 1.40→1.60, pounce 3.33→3.68. thug slash_1 2.33→2.57 (0.87-0.97 → 0.97-1.06), slash_2 2.10→2.28 (female the same). Erlan axe_1/2 2.17→2.42. bandit sword 2.50→2.73, kick 1.67→1.82 (female the same). Garrick double 2.73→2.87, triple 2.67→2.78, lunge 2.27→2.67, dash 1.63→1.92, quick 1.33→1.45. Hogger overhead 4.67→5.20, heavy_swing 6.13→6.88, step_smash 4.53→5.03, sweep 5.07→5.62, lunge_smash 5.33→5.98, quick 4.00→4.45, backhand 4.53→5.08, feint_plunge 4.00→4.50, charge 2.23→2.58; hop_back keeps its timing (its hit window lies outside its clip in the DS data; springs only). Wolf bite 1.667→1.500, lunge 1.500→1.600, Prowler bite 1.667→1.500 (authored, as before).

Damage, poise, radii, flags' order and root-motion distance are unchanged. Balance does change: the telegraph is 12-22 % longer, each hit window 10-18 % shorter, the recovery 10-18 % longer.

## Verification

- 10 remix unit tests (clock monotone and invertible, phases, double-hit gaps, endpoints/translations/distance of a retime, coil direction, spring lag and cap, loops and unlike rigs untouched, name classification) pass.
- Two install tests on the real cache (skip without it):
  - every kit (10) remixes without fallback; first/last frames bit-equal; every bone outside the rig's head/trunk/tail set has the source's rotation, translation and scale at the clock's time; feet FK positions drift < 1e-3 model units; damage/poise/radius equal; hit windows ordered and inside the clip; reference clips untouched except where the weapon grip samples them (then retimed with the same clock).
  - all 115 player `human_male` clips: 48 dressed, 67 loops left alone, 0 kept; fps, frame count, root motion, flags and hits identical to the DS clip; largest trunk/head turn 25.2° (a00_2007 flinch).
- Live, `WOWSOULS_DSVIEW` side-by-side recordings (1080p, original over remix, 5-6 frames/s): kobold small_1, thug slash_1, thug death a00_2204 and flinch a00_2007, player r1_1, Hogger overhead. No stretched limbs, detached heads or abrupt skeleton changes seen; the player's blade still follows the DS blade; the remix is subtle from the game camera (as the wolf trial found), clearer in close-up.
- Live fights (kobold small_1, thug slash_1): contact lands inside the new window, thug 0.90→1.02 s, kobold 0.80→0.90 s, same damage (25/26 and 23).
- Gates: see the end of this file.

Re-run: `cargo test -j 4 --offline -p wow-souls remix -- --nocapture` (with `DSR_DIR`, `WOW_DATA`), recordings `cache/motion-remix/capture.ps1 -Species kobold -Entry 80 -Clip small_1` then `sheet.ps1 -Tag kobold-small_1 -Crop <w:h:x:y>` (gitignored cache).

## Tuning

Everything editable is a constant at the top of `remix.rs`: `LIGHT`/`HEAVY`/`BOSS` (timing multipliers, coil degrees), `FOLLOW`, the `ATTACK`/`HURT`/`MISC`/`EVADE` spring sets (omega, damping, mix, max lag in degrees), and the rig tables.

## Open

- Judge by eye in play (the user's call): is the telegraph long enough, are the flinches too loose? Flinch head lag reaches 25-30° locally (cap 28° + neck 16° in model space).
- Retiming the player's moves (see above), a hand-authored curve set per enemy (the wolf recipe style), and sync of the DS reference skeleton for non-grip attacks remain.

## Gates (2026-10-15, after the last edit)

`cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check` clean; `cargo clippy -j 4 --workspace --all-targets -- -D warnings` clean; `cargo test -j 4 --workspace` with `DSR_DIR`/`WOW_DATA`: wow-souls 892, wow-world 240+4, dsr-cache 88, dsr-formats 52, wscl 4, all pass. Another chat was editing the UI in the same tree at the same time; its files were neither read nor changed here, and its work is in those counts.

## Follow-up 2026-10-16: R2 missed, loops, WoW's own animations

User: the remixes "blev perfekta, fortsätt"; but with the one-handed sword the right-click attack (R2) almost never hit; "fortsätt att rigga alla animationer".

**The R2 bug (mine).** The coil was at its strongest *inside* the hit windows: at the first frame of a window the trunk was fully wound up (BOSS weight for R2, 13 deg yaw, 9 deg pitch) and released through it. The blade is swept from the hand bone (`hits.rs`, `BLADE_RADIUS` 0.18 yd, and its own comment says a one-handed slash only just reaches a 1.3 yd kobold), so the hand moving decided hit or miss. Measured with the new tests on the real clips, hand off its DS position during the hit windows: R1 a23_3000 0.20 yd, a23_3002 0.17, R2 a23_3300 0.06, a23_3310 0.08, roll attack 0.11, other weapons up to 0.16; enemies up to 0.20 (Erlan's axe). Live trace on a 0.4-scale young wolf: blade clearance past reach in the active frames 0.18/0.12 original, 0.19/0.14/0.30 with the old coil, 0.18/0.12 now. Fix: the coil is wound up fast, held and let go *before* the first window, is exactly zero through every window, and the follow-through starts after the last one. Hand deviation in every hit window is now 0.0000 yd for all 10 player blows and all 30 enemy blows (two install tests keep it that way; threshold 1e-3). Live: heavy attacks on a held goldtooth hit 3/3 at the same points as the original. The visual wind-up and follow-through stay; the hit geometry is the DS clip's, bit for bit.

**Loops.** Gaits, lock-on walks, sprint, idles and the guard hold (67 player clips) now get neck/head springs only (`LOOP`: lag at most 6 + 9 deg, mix 0.35/0.5). Three laps are simulated and the last kept, so the springs start in steady state and the seam closes: a clip whose rig bones repeat at the end keeps first == last for them; install test: every loop's seam within 0.5 deg of the DS clip's. Recorded sprint and run from the game camera: no artifacts, subtle.

**WoW's own animations (`remix_live.rs`).** Foes on the model's own stand/walk/run/turn clips get the same springs at runtime: neck, head, jaw, tail, ears in world space (the creature's own turn included, so a tail swings out on a turn). Only while no kit clip shows (`Fade::is_free`, `MonsterPlay::clip` none); the edits are undone before they are made again (as `lively` does); rotations only. `LIVE` springs: head 10 deg cap, tail 24. Wolf, Prowler, kobolds, humans, Garrick and Hogger; the player is not touched (his gaits are the dressed DS clips). 3 unit tests (rig fits, rest untouched, a turn trails then settles, edits put back). Seen live on a wolf closing in and circling: body intact, no detached tail.

Tests now: 16 remix tests; whole workspace green: wow-souls 898, wow-world 240+4, dsr-cache 88, dsr-formats 52, wscl 4; clippy `-D warnings` and the fmt gate clean.

Still open: retiming the player's own moves (manifest times), per-enemy authored curves, judging by feel (the user).
