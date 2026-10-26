# Warrior balance pass (2026-10-17)

User: "lvl 5 one shottar jag alla fiender och två shottar garrick med 2 handed mace ... man tar lite mer skada ... skadan man gör är väldigt för hög". Warrior only.

## Cause

Creature tables (`species.rs`) were tuned against a level-1 player: 20 per light sword blow, 100 health. The player then grew on three WoW axes while creatures stood still:

- `weapon_factor` (Militia Warhammer 5579, 6-9 / 2.9 s): x1.57 on top of the 2H mace moveset (base 91/78, mult 1.2).
- `melee_ap_factor`: x1.50 at level 5, x1.62 at 6, x1.85 at 8.
- Health from the class table: 60 at level 1, 155 at level 6, while kit blows stayed 20-25.

The user's save (character 1, 10,972 XP = level 6): light 71, heavy 89, heavy + Battle Shout 102 against Garrick's 180 and a kobold's 60.

## Change

- `wow_world::attributes::AP_EXPONENT = 0.5`: both AP factors square-rooted (x1.23 at 5, x1.27 at 6).
- `wow_world::gear_effect::WEAPON_EXPONENT` 0.5 -> 0.35, `WEAPON_FACTOR_MAX` 2.5 -> 2.0 (Warhammer x1.37).
- New `wow_world::balance`: `creature_health_factor(level) = sqrt(1 + 0.12 (level-1)) x TOUGHNESS 1.25` (tracks the damped warrior AP curve, tested against the class table) and `creature_damage_factor(level) = max(1, (60 + 19 (level-1)) / 100)` (an on-level naked warrior's health over the kits' 100, exact to the table).
- `SpeciesDef::max_health()` / `blow_scale()` apply them; `npc.rs` spawn health and all five blow sites use them. Species tables stay in level-1 units.
- Hogger re-expressed in level-1 units so his effective numbers hold: health 900 -> 500 (927 at 11), damage 1.5 -> 0.6 (taurus), 2.2 -> 0.88 (knight).
- DB-built creatures (`creature_motion.rs`) moved to level-1 units: health `45 + 6 x level`, damage 0.9 (both x the DB multipliers).

## Numbers (user's level 6, Warhammer)

| | before | after |
|---|---|---|
| light / heavy | 71 / 89 | 49 / 61 |
| + Battle Shout | 82 / 102 | 56 / 70 |
| Kobold / thug (lvl 4) HP, blow | 60, 25 | 87, 29 |
| Garrick (lvl 5) HP, blow | 180, 20 | 274, 27 |
| Stonetusk (lvl 6) | 80, 20 | 126, 31 |
| Bear (lvl 9) | 160, 25 | 280, 53 |

Kobold: 1 hit -> 2. Garrick: 2 shouted heavies -> 4 (6 lights). On-level fights now take the species' stated blow count x1.25 at every level; level 1-3 creature damage is unchanged.

## Open

- Mage not rebalanced: creatures are tougher for spells too (spells do not use the AP factor).
- Knobs: `TOUGHNESS`, `AP_EXPONENT`, `WEAPON_EXPONENT`, `HEALTH_GROWTH_PER_LEVEL`. Not playtested live yet.
- Tests: wow-world 247, wow-souls 1016 pass.
