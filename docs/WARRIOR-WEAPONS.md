# Warrior weapons — source and implementation

The warrior's weapon subclass selects Souls combat. The installed DSR files are read-only;
derived HKX/TAE/PARAM data lives under ignored `cache/dsr/weapons-*/`. No game assets are checked in.

`cargo run -j 4 -p dsr-cache -- weapons` rebuilds only these families. The regular `extract`
command now also builds them. A generation is indexed only after all families pass extraction.
`weapons-survey` prints the original rows, animation imports and events for inspection.

## Controls and coverage

Equip through the existing WoW equipment page. F/LMB/RB is R1, R/RMB/RT is R2, with the
source's chain and cancel windows. H toggles a second hand on a dagger, sword, axe or mace.
WoW two-handed weapon subclasses select their large two-handed counterparts automatically.
Y / D-pad left draws or puts away the equipped ranged weapon; it uses the same attack inputs.
Switches wait until the current action finishes. A two-handed grip hides the off-hand model
without changing ownership or the saved equipment. Bows attach to the left hand.

### Resting and carrying weapons

Outside attacks, guard, rolls and interactions, the equipped weapon uses its DSR passive
stance. This also applies without a locked target. Greatswords/greataxes/great hammers use
`a12_0000` (over the shoulder); small weapons held in two hands use `a10_0000` (in front);
polearms/staff use `a13_0000`. One-handed melee uses `a00_0000`. Bow uses `a14_0000`,
crossbow and the adapted gun use `a16_0000`, and thrown uses `a00_0000`.

The two-second breathing loop has a continuous ownership clock. Only the sampling phase
wraps, so a loop boundary never reintroduces the underlying WoW pose. Attacks and interactions
still crossfade normally. While walking (up to 3 yd/s), the source carry pose remains on the upper body;
the existing WoW locomotion supplies the legs, easing between standing and walking over 0.2 s. From 3 to 4.4 yd/s the carry pose fades out (0.2 s), so a locked-on strafe and a run use WoW's own clips on the whole body: a frozen upper body over running legs looked absurd (2026-10-11, `dsclip.rs` `carry_share`).
This was a carry-pose adaptation (the part above), not extracted DSR locomotion or automatic sheathing.

**Update 2026-10-12: the gait is the real Dark Souls one.** With `cache/dsr/locomotion.txt` the
walk, run, sprint and the locked-on steps are the weapon motion set's own DSR clips
(`<set>_0200` walk, `0500` run, `0510` / `a00_0535` sprint, `0300-0303` and `0550-0553` locked
walk and run in four directions; `a00` one-handed, `a10` small two-handed, `a12` great weapons,
`a13` polearms, `a14` bow, `a16` crossbow), blended by speed and played at the pace of the
ground (`src/dsloco.rs`), so each weapon is carried the way DSR carries it while moving. The
carry-pose fade above remains only as the fallback without that cache. Nothing sheathes
automatically: DS1 / DSR has no timed sheathing (the weapon stays in the hand in its passive
stance, which is the breathing loop above).

The [DS1 passive motion categories](https://soulsmodding.com/doku.php?id=ds1-refmat:wepmotioncategory-list)
document these categories; the actual clips and per-weapon motion IDs come from the local install.

| WoW weapon | DSR reference row | Source light / heavy family | Hand |
|---|---:|---|---|
| Dagger | 100000 | a20 | 1H and 2H |
| Sword | 200000, Shortsword | a23 | 1H and 2H |
| Two-handed sword | 300000 | a25 | 2H |
| Axe | 701000 | a30 | 1H and 2H |
| Two-handed axe | 750000 | a32 | 2H |
| Mace | 801000, Mace | **a33 light / a59 heavy** | 1H and 2H |
| Two-handed mace | 852000, Demon's Great Hammer | a35 | 2H |
| Polearm / historical spear subclass | 1103000, Gargoyle's Halberd | a38 | 2H |
| Staff | 1103000, Gargoyle's Halberd | a38; deliberate melee adaptation | 2H |
| Fist weapon | 901000, Caestus | a42 | right hand |
| Unarmed | 900000, Fists | a42; different behavior values from Caestus | right hand |
| Bow | 1201000 | a14_0100 draw + a44_4000 shot | left hand holds bow |
| Crossbow | 1250000 | a46_4000 | 2H shot |
| Gun | crossbow motion adapted | a46_4000 with estimated 0.3 s preparation | 2H shot |
| Thrown | DSR item throw | a00_7582, release at goods event 65 | right hand |

There are 19 runtime profiles, counting alternative grips and unarmed separately.
One representative moveset per weapon family is intentional; it does not reproduce every
unique weapon in Dark Souls. In particular a large WoW two-handed mace and the smaller DSR
Mace held with both hands are different profiles.

## What is read from DSR

Melee skeleton motion, 30 fps timing, root motion, hit windows, chaining/roll/movement cancels,
stamina, attack multipliers, hit-stop, knockback and hit radii come from HKX/TAE and PARAM.
TAE imports are resolved separately for HKX-only aliases and full event imports, with cycle
rejection. The two-contact great-hammer heavy retains both judges and parameter rows.
Fists use PCBehavior type 16. Poise damage includes `saWeaponDamage * atkSuperArmorCorrection`
(percent), plus `atkSuperArmor`; the old generic fallback is no longer used for these moves.

Melee base physical attack also comes from `attackBasePhysics`. It is normalized to the
prototype's existing Shortsword = 20 damage scale. This preserves weapon damage ratios without
pretending that prototype enemy health/armor is the complete DSR damage formula. Item upgrades,
stat scaling, elemental damage and WoW item-specific damage are not applied by this layer.

Examples from the local install:

| Profile | R1 stamina / multiplier / poise | R2 stamina / multiplier / poise |
|---|---|---|
| Mace 1H | 25 / 1.00 / 35 | 50 / 1.25 / 42 |
| Mace 2H | 38 / 1.20 / 52.5 | 68 / 1.50 / 52.5 |
| Great hammer 2H | 48 / 1.20 / 75 | 85 / 1.50 / 75 (first contact) |
| Shortsword 1H | 20 / 1.00 / 20 | 45 / 1.35 / 24 |
| Dagger 1H | 14 / 1.00 / 5 | 30 / 1.35 / 6 |

Great-hammer R1 lasts 2.900 s, active 0.833333–1.000000 s. Its first heavy lasts 3.333333 s,
with contacts at 1.266667–1.400000 and 1.433333–1.500000 s. Earlier cancel flags still control
when the player may act again; clip duration is not an unconditional input lock.

Idle and weapon-block clips come from the reference weapon's motion family. Guard cut and
stability are read per weapon. **Guard uses TAE flag 3.** The old implementation confused
flag 85 (Light Lantern Weapon) with Guarding; that is corrected, including the old sword kit.
The existing bare-left-hand parry is disabled while using two hands or a ranged weapon.

The original skeletal motion is retargeted to WoW HumanMale proportions, not binary-identical
joint coordinates on a different skeleton. Every extracted melee and ranged segment passes
the existing direction, cross-body and grounded-foot gates. Runtime weapon orientation follows
the source weapon bone's full rotation, including twist and dagger grip changes. Blade length
and collision extent follow the equipped WoW model.
Crossbow/gun models lack blade markers: their barrel is mounted separately onto the source
weapon's +X axis, leaving the source bone rotations unchanged and creating no melee hitbox.

## Deliberate adaptations and remaining limits

Only DSR is installed. Staff uses DSR polearm motion as a temporary melee interpretation informed
by DS2's Santier's Spear. Gun uses DSR crossbow motion as a temporary Souls firearm informed by
Bloodborne's Rifle Spear. **Neither is an original DS2/Bloodborne animation.**

Bow draw/release is composed from two unchanged DSR segments. Crossbow uses its source shot.
Thrown uses the fast DSR goods animation; association of `a00_7582` with throwing knives is
inferred from the event and observed motion, not verified against the player's ESD state machine.
Ranged R1/R2/roll-follow-up currently all fire the same committed shot; there are no alternate
ammo attacks or a separate reload state. Projectiles release once per action at the event,
travel without homing, sweep against terrain and posed bodies, and stop at the first obstruction.
They never deal a simultaneous melee hit. Ranged guard/parry is disabled.
The projectile visual is currently a tracer; bowstring deformation and ammunition meshes are
not reproduced. Free aim uses character facing, while lock-on aims at the target at release.

Projectile speeds (45/55/90/28 yd/s for bow/crossbow/gun/thrown), gravity (1.5/0.6/0/3 yd/s²),
damage (24/30/32/16), poise (8) and thrown stamina (10) are estimates. Bow/crossbow stamina is
the original behavior value, 20. Gun borrows that value. Ammunition is not consumed yet.
The local prototype still lacks whiff branches, sprint/jump/plunge attacks, off-hand attacks,
shield-specific defense, critical attacks and a complete DSR armor/stat calculation. These are
not represented as exact implementations by this work.

## References

- Installed `chr/c0000*.anibnd.dcx`, `param/GameParam/GameParam.parambnd.dcx`, and PARAMDEFs
  are the numeric/animation source of truth; each published family fingerprints these sources.
- [DS1 attack animation schema](https://soulsmodding.wikidot.com/reference:ds1-attack-animation-schema)
  explains motion/action numbering and whiff variants.
- [EquipParamWeapon documentation](https://soulsmodding.com/doku.php?id=ds1-refmat:param:equipparamweapon)
  documents base attack, poise, special motion and hand-motion fields.
- [DSAnimStudio DS1 TAE schema](https://github.com/Meowmaritus/DSAnimStudio)
  (`TAE.Template.DS1.xml`, local research copy) documents flag 3, flag 85, bullet and PCBehavior events.
- [Santier's Spear](https://darksouls2.wikidot.com/santier-s-spear) and
  [Rifle Spear](https://www.bloodborne-wiki.com/2015/03/rifle-spear.html) are adaptation references,
  not sources of extracted files or claimed exact frame data.

Runtime checks use isolated unattended saves. `WOWSOULS_WEAPON=<profile>` forces a family for
inspection; `WOWSOULS_DSVIEW=r1_1@0.83` freezes it beside its source skeleton. `WOWSOULS_WEAR`
changes only the visible display. Set `WOWSOULS_TEST_WEAPONS` to the absolute DSR cache path to
run the test that loads and validates every published family.

## Verification, 2026-10-11

- Workspace tests with both installs enabled: 832 passed (game 560, extractor 54, formats 52,
  world 158 + importer 4, clip format 4). Scoped formatting and workspace clippy passed.
- Every published family loads, with source Mace heavy category, multi-contact hammer heavy,
  source stamina/poise checks and forward-pointing crossbow/gun barrel at release.
- Unattended source-skeleton comparisons: mace 2H, bow, crossbow and gun; all exit successfully.
  Mace 2H heavy's worst rendered blade-axis error over a complete loop was 3.1 degrees.
- Equipping actual item 2055 selects mace from the starting sword without a forced profile.
  A released gun projectile hits a kobold for 32 and triggers its hit reaction; the values
  remain the declared prototype estimates. Bow/crossbow/thrown also release at their events.
- Screenshots and logs are under `docs/evidence/warrior-weapons/`; detailed build and test
  logs are under ignored `cache/warrior-*.log`. These checks do not constitute a full manual
  playthrough of every chain, stance transition or terrain-obstruction case.
