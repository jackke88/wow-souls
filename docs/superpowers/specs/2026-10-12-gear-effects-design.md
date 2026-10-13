# Gear effects: armor, weapon damage and block do something (design)

2026-10-12. The user: "Gear är nästa då - detta måste fixas så gear gör något", then "Kör hybrid".
Status: **approved by the user 2026-10-12; built the same day by `docs/superpowers/plans/2026-10-12-gear-effects.md`** (measured live: see HANDOVER, "GEAR EFFECTS BUILT"). Nothing committed.

## What the user wants

What a character wears must change how a fight goes. Today the paper doll, the pack's Better / Worse
marks and every tooltip show numbers that nothing in combat reads. Success = a better piece of gear
visibly changes the damage taken or dealt, in the same units the tooltip prints, without turning
the Dark Souls feel into a WoW one.

Said by the user: hybrid (armor and block as WoW has them, the weapon's WoW damage scales the Dark
Souls moveset, poise stays Dark Souls, stats come later).
Assumed by Claude (to be judged in play): the constants in "Tuning knobs".

## What exists (read 2026-10-12)

- Nothing in combat reads armor or block: `equipment::totals` (armor, block) is used only by the
  equipment page, the pack's comparison and the tooltips.
- Damage dealt: `hits.rs` `strike`: `moves.base_damage * swing.spec.mult * buffs.damage_factor()`
  (`base_damage` = the DS weapon family's ratio normalised to the Worn Shortsword = 20). The item's own
  WoW damage / dps is ignored. Ranged: `projectiles.rs` `spec.damage` (explicit estimates per
  family). Unarmed has a fist profile.
- Damage taken: `hurt.rs` `suffer`: `hp.cur = hp.cur - hurt.damage` (raw). A guarded blow arrives as
  `PlayerHurt { blocked: true, damage: chip }`, the chip computed in `defense.rs`
  `block(cut, stability, damage, ..)` from the kit's single `cut` (a family value, 50 % for the sword).
- Poise: `reaction.rs` `player_poise()` is a constant.
- Items: `wow_world::model::Item { armor, block, damage: (min, max, type), delay, .. }`; **no stat
  columns** (`stat_type` / `stat_value` are not imported). 331 items in the manifest.
- Creatures have no level in `SpeciesDef` ("the client has no creature levels"); `PlayerHurt.level` is
  the blow's flinch level, not the attacker's.
- Files `hits.rs`, `hurt.rs`, `defense.rs`, `projectiles.rs` are co-owned with the other chat
  (weapons / animation): re-read before each edit, keep edits to a few lines.

## The rules

| What | Rule | Source of the numbers |
|---|---|---|
| Damage taken | `taken = damage * (1 - armor / (armor + 400 + 85 * attacker_level))`, reduction capped at 75 % | WoW 1.12's armor formula; `armor` = `totals().armor` (every worn piece incl. the shield) |
| Damage dealt (melee) | `damage * weapon_factor(main hand)` | `weapon_factor` below |
| Damage dealt (ranged) | the projectile's damage `* weapon_factor(ranged slot item)` | same function |
| Guard | a blocked blow's chip `* (1 - min(BLOCK_REDUCTION_PER_POINT * block, BLOCK_REDUCTION_MAX))` | the shield's `block` (`totals().block`); changed from "raise the kit's cut" while planning so `defense.rs` needs no edit |
| Stowed shield | a shield put away (a two-handed grip, a drawn bow: `equip::apply` hides it) gives **neither armor nor block** | added after the final review: it was a free defence otherwise |
| Poise, levels, reactions | unchanged (Dark Souls) | |
| Fists | `weapon_factor` = 1.0 | unchanged feel |

**`weapon_factor(item) = clamp((dps / REF_DPS)^0.5, MIN, MAX)`** with `dps` = `weapon_dps(item)`
(mean of the damage range over the swing time, as the tooltip prints it) and `REF_DPS` = the Worn
Shortsword's own dps (1-3 over 1.9 s = 1.053), so **the starting kit is exactly 1.0 and today's
balance does not move**. The family's DS ratio (dagger vs hammer) stays: the factor only says how
much better or worse this piece is than the starter blade. The square root keeps a level-7 mace from
being 6 times the starting sword (a Dark Souls +10 is about 2-3 times, not 6).

Examples (factor): Worn Shortsword 1.00, Militia Dagger / Militia Shortsword 1.38, Militia Hammer 1.36,
Militia Warhammer 1.57, Militia Quarterstaff 1.60, Flanged Mace 1.69, Billy Club 1.91, Hunting Bow 2.10,
Priest's Mace 2.48, Spiked Club 2.93 -> capped 2.5.

**Attacker level** for the armor formula comes from a new `level: u8` on `SpeciesDef`, taken from the
vMaNGOS `creature_template.level_max` rows already in `cache/world/northshire.txt`: Kobold Laborer
(`kobold`) 4, Vermin 2, Worker 3, Timber Wolf 2, Young Wolf 1, Defias Thug 4, Garrick 5.

Reduction examples at attacker level 3 / 5 (armor / (armor + 655 | 825)):

| Worn | Armor | Level 3 | Level 5 |
|---|---|---|---|
| New character (shirt, pants, boots, Worn Wooden Shield) | 7 | 1.1 % | 0.8 % |
| Cracked Leather set + Pioneer Buckler | ~275 | 30 % | 25 % |
| Best leather of the valley + Cadet Shield (274) | ~520 | 44 % | 39 % |

A shield is most of the armor (274 of 520). That is WoW's own table, kept; the user may want it
scaled down (tuning knobs).

## Where it lives

- **Pure, in `wow-world`** (no Bevy): a new `gear_effect.rs`: `armor_reduction(armor, attacker_level)`,
  `weapon_factor(item)`, `block_cut(base_cut, block)`, the constants, tests.
- **Game, `src/gear_effect.rs`** (new): `Res<GearEffect { armor, weapon_factor, ranged_factor, block }>`
  recomputed where the avatar's `Appearance` is applied (`equip::apply`, so it follows every equip,
  unequip, save load and start kit), and the log line
  `gear: armor 275 (30 % at level 3), melee x1.38, ranged x1.00, block 3`.
- **Four small hook edits** (each a multiply or one more parameter, like `Buffs` in `strike`):
  `hits.rs` `strike` (`* gear.weapon_factor`), `projectiles.rs` (damage at spawn), and the two
  places in `npc.rs` that write `PlayerHurt` (the armor reduction by the attacker's `Species.level`,
  and the shield's chip factor when `blocked`). Applying it where the damage is made, not in
  `hurt.rs` `suffer`, means shake, blood and voice read the reduced number too; `hurt.rs` and
  `defense.rs` are not edited. (Changed from the first draft while writing the plan.)
- **Switch:** `WOWSOULS_GEAR_EFFECT=0` makes every factor 1.0 / reduction 0 (scripted checks that
  assume exact damage, such as the old kobold 25-damage runs).
- **Display** (small): the equipment page's totals line becomes `Armor 275 (-30 %)     Block 3
  Hits x1.38` (the percentage is against a level 3 attacker, the factor is the main hand's). WoW's
  tooltip text is left as it is. The pack's Better / Worse verdict already weighs dps, armor and
  block; it now tells the truth.
- **Save format:** no change (everything is derived from what is worn).

## Not in this step

- **Stats** (+Strength / Agility / Stamina / ...): needs the importer to read `stat_type1..10` /
  `stat_value1..10` and a manifest version bump; then Stamina -> HP and Strength -> damage rules.
  Next step, own spec.
- Armor -> poise (the Dark Souls link; a weight-to-poise table), durability, weapon skills, class
  armor proficiencies (the class framework), resistances, off-hand weapon damage (dual wield stays
  visual), NPC gear.

## Tests and verification

- Pure: the formula at its edges (0 armor, cap 75 %, level 0 and 60), `weapon_factor` for the
  starter (exactly 1.0), a no-delay item (1.0), the cap and the floor, `block_cut` cap, the
  Hunting Bow and Militia Dagger values above.
- Game: `GearEffect` from the start kit; changes on equip / unequip; `WOWSOULS_GEAR_EFFECT=0`.
- Live (unattended, as the handover's recipe): a kobold's 25-damage blow with the kit vs with a
  Cracked Leather vest + Pioneer Buckler equipped (`WOWSOULS_EQUIP=...`), the log shows the reduced
  number; a Militia Dagger vs the Worn Shortsword on the same kobold (`WOWSOULS_TRACE` damage lines);
  a blocked blow with and without the shield.
- Gates: scoped fmt `--check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`,
  `cargo test -j 4 --workspace` with `DSR_DIR` / `WOW_DATA` set.

## Tuning knobs (Claude's choices, for the user's hands)

| Constant | Value | Why |
|---|---|---|
| armor formula | WoW 1.12 (400 + 85 x level, cap 75 %) | the tooltip's numbers mean what WoW means |
| `WEAPON_EXPONENT` | 0.5 | a damped, not linear, step between pieces |
| `WEAPON_FACTOR_MAX` / `_MIN` | 2.5 / 0.75 | a level-8 club must not one-shot a 60 HP kobold |
| `BLOCK_REDUCTION_PER_POINT` | 0.05 | block 1..5 -> the chip 5..25 % smaller |
| `BLOCK_REDUCTION_MAX` | 0.5 | a shield never makes a hit free |
| `DISPLAY_LEVEL` | 3 | the attacker level the equipment page quotes armor at |

If the valley turns too easy with gear, the first lever is the armor curve (add a divisor on the
reduction), the second the weapon exponent.
