# Militia Warhammer: grip and striking-face fit

User feedback: the player grips the hammer poorly and lands with the head's side. Scope: item 5579 (Militia Warhammer), display 19544, HumanMale, `great_hammer`; no enemy or source-animation changes.

## Investigation

The equipped save identifies 5579. Its original model is `hammer_2h_maul_a_02.m2`. In the game's coordinates its shaft points along -Z, while the two striking faces lie along +/-Y. Trail markers describe the shaft and cannot determine the head's clock around it. The old shortest-arc mount puts that face along source +Z, across the hammer's swing plane. The calibrated quarter turn retains the shaft axis and maps the face onto source -X. At the first heavy hit's end (a35_4300, 1.4 s), its downward alignment improves from about 0.03 to 0.976.

The source also deliberately releases L_Hand during portions of the swing. Measured c0000 bone 24's radial distance from R_Weapon 56's shaft is about 0.064 yd closed, 0.436 during R1's open-hand wind-up, and 0.363 in the heavy wind-up. A full-time support-hand constraint incorrectly erases those releases. The runtime contact share is full below 0.09 yd and fades out by 0.18; roll, hurt, death and unrelated actions release the constraint.

Independent retargeting leaves the support palm offset even when the source is gripping. Fixed-length arm solving exposed an additional physical constraint: keeping the right palm at the model origin puts parts of the handle beyond the left arm's reach. The actual native handle spans about -0.34 to +0.55 along its shaft. The new upper-hand grip is 0.16 toward the head, with the support palm chosen on real wood (-0.30 to +0.48) inside the arm's reachable interval. A two-bone support solve rotates the upper arm/forearm and preserves local translations, scales and wrist pose; it never lengthens the arm. Across closed-grip frames of all six attack roles, the worst palm gap improves from 0.243 to 0.056 yd. Stance and ordinary slashes fit to numerical precision; the remaining maxima occur in the rolling attack and heavy wind-up.

## Implementation and gates

`actor/held.rs` retains the measured right/left attachment points only for this display/body profile. `dsclip.rs` applies the clock and upper-hand offset, then fits the support hand after the source layers have blended. The weapon and existing hit volume share the same transform. Source clips, TAE windows, stamina, damage and hit radii are unchanged; other displays retain their existing mount. `WOWSOULS_GRIP` still bypasses the calibration.

Three regressions cover shaft/face orientation, rigid arm lengths and local wrist pose, source hand release/regrip, and actual installed attack clips. Targeted tests (23) and workspace Clippy with warnings denied pass. Evidence and research helpers are gitignored under `cache/hammer-grip/` (`probe`, `source-palms`, `ik-probe`, logs and recordings).

## Final verification

The final build, scoped formatting, workspace Clippy (warnings denied), and install-enabled workspace tests pass: wow-souls 878, dsr-cache 88, dsr-formats 52, wow-world 240+4 importer tests, wscl 4. Whitespace checks pass.

Eight final motion recordings cover idle, R1 and R2 from the front, R1 and R2 from the side, rolling attack, guard and walking. Dense frame series were reviewed across the whole recordings; the support grip is improved without stretching the arm, flipping the elbow or snapping the weapon, while the source's deliberate hand releases remain. Early front takes with a HUD-obscured hammer were replaced with fully framed, plain-HUD recordings (`clear-*`). Normal game-camera combat was also reviewed. All unattended checks report `WOWSOULS_CHECK ok`.

Separate game-camera contact checks use a stationary, initially neutral James Clark fixture with isolated saves: R1 deals 149 damage (31/180 HP remaining), and R2 deals 186 damage and kills him. The heavy contact, enemy death and recovery were reviewed in a dense frame series. Hit windows, radii and combat parameters are unchanged. Live fixtures and saves are gitignored; the user's saves were not written. No commits.
