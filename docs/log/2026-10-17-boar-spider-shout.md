# Boars, spider legs, Battle Shout on the run (2026-10-17)

User: "Boar = attackerar inte tillräckligt och dom bör chargea direkt in mot fighten. Den håriga spindeln går igenom marken när den attackerar. Battle shout ska kunna göras medans man springer och vi måste visa buffen och hur länge den är på."

## Boars (`behavior.rs`, `species.rs`)

- Out of reach a `BoarCharge` creature sprints in: pace `BOAR_RUSH` 1.35 (Stonetusk 4.8 -> 6.5 yd/s).
- Aim 0.65 -> 0.25 s, recover after a move 0.65 -> 0.3 s.
- `STONETUSK.recovery = Some(0.6)` (all boars inherit): the gore clip is 3.67 s with its blow done at 1.67 s; the move now ends 0.6 s after its last hit window.
- Live (capture `boar-rush` vs `boar-rush2`, entry 113 from 14 yd, trace): Approach -> Aim -> Engage at once; attack interval about 3.3-4 s before, 1.6-2.7 s after (wall clock under recording); 4 blows landed in 14 s against 2 in 10 s.

## Spider legs (`legfloor.rs`, hooked in `monster::load`)

- Cause: the Crag Spider moves on the tarantula (pose-relative retarget, no ground lock). Probe of `body_attack_01` (bite): front-leg tips (bones 58/62) about 0.9 model yd under their rest height; head (34) about 0.3. The body (2/5) never goes below 0.
- Fix: per frame, a joint below `min(first-frame height, 0)` has its ancestors' turns slerped back toward the first frame, binary search for the largest share that keeps it up. Kits `tarantula`, `minespider`, `mother_fang`; attacks and reacts. Log line `monster: tarantula legs kept out of the ground (2147 joint-frames)`.
- Live before/after strip (capture `spider-sink` / `spider-floor`, forced bite): legs vanish into the turf before, stay on it after. Hit windows: the reach bones (head 34, abdomen 5) are above the floor in the windows, so reach is unchanged.

## Battle Shout and the buff row (`abilities/`)

- `moves::shouts_on_the_move`: above 1 yd/s ground speed the shout inserts no `Casting` (no roar holding the body); standing still keeps the roar.
- `abilities::ui` buff row under the status meters (left 96, top 74): icon per running buff (Battle Shout 6673, Frost Armor 168), time left under it (`2m`, `59s`), blinking in the last 5 s.
- Live (scratch run, drive forward, shout at 3 s): kept running every frame; icon + `2m` under the meters.

Tests: wow-souls 1024 pass; fmt + clippy clean. Not playtested by the user.

## Upper-body roar on the run (same day, user: "borde väll kunna göra animationen medans warriorn springer som i wow")

- benilla already builds an upper-body node for every WoW clip (`AnimClip::upper_node`, mask group 2 = legs and pelvis out, root KeyBone 4 = HumanMale bone 20); nothing used it before.
- `abilities::moves::UpperCast`: a shout above 1 yd/s plays the roar's `upper_node` over the run at weight 8 (as the hand grip), easing in 0.12 s and out 0.2 s; a blow, a roll or death ends it. Standing still keeps the whole-body `Casting` roar.
- `locomotion`: `GaitState.upper` (the cast's share) and `upper_body()` (bone 20 and its subtree): the DS gait keeps the legs and gives the torso, arms and head up by that share. `GaitState::torso_weight()` scales the torso straightening (`lively.rs`) and the blade/support grip (`motion_clip.rs`) the same way.
- Live (side observer, grass off): legs keep the running stride every frame; the upper body goes into WoW's BattleRoar (chest out, arms flexed, blade upright in the hand) and eases back into the run carry. Tests 1026 pass, fmt + clippy clean.
