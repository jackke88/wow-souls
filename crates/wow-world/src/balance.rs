//! How hard a creature is against the player who meets it on level (2026-10-17). The creature tables
//! (`wow-souls` `species.rs`) give health and damage in level-1 units: health in light blows of a
//! level-1 warrior's starting sword (20 each), damage against the 100 health the first prototype had.
//! The player grows on WoW's tables (attack power, weapon damage per second, class health) while those
//! numbers stood still, so by level 5 every creature fell to one blow and took seven to kill. These
//! curves carry a creature's numbers to its level, so an on-level fight takes about as many blows each
//! way at level 10 as at level 1; a player above the creature's level still wins faster.

use crate::attributes::AP_EXPONENT;

/// A warrior's raw attack-power factor grows about this much per level over level 1 (the class table:
/// x1.50 at 5, x2.07 at 10). Health follows the damped factor the player's blows get.
pub const HEALTH_GROWTH_PER_LEVEL: f32 = 0.12;
/// Every creature is this much tougher than its blow count says: the first tuning was against a sword
/// with no gear, and the game was too easy with any.
/// 1.25 until 2026-10-18, when the user still found on-level mobs too easy ("kolla över scalingen").
pub const TOUGHNESS: f32 = 1.5;
/// A naked warrior's health at level 1 and per level after (the class table: 60, 79, ... 231 at 10).
pub const WARRIOR_HEALTH_1: f32 = 60.0;
pub const WARRIOR_HEALTH_PER_LEVEL: f32 = 19.0;
/// The player health the creature kits' damage was tuned against.
pub const KIT_HEALTH: f32 = 100.0;

/// What a creature of `level` multiplies its table health by.
pub fn creature_health_factor(level: u8) -> f32 {
    let up = f32::from(level.max(1) - 1);
    (1.0 + HEALTH_GROWTH_PER_LEVEL * up).powf(AP_EXPONENT) * TOUGHNESS
}

/// What a creature of `level` multiplies its blows by: an on-level warrior's health over the kits' 100,
/// never below 1 (the first levels keep the kits' own numbers).
pub fn creature_damage_factor(level: u8) -> f32 {
    let up = f32::from(level.max(1) - 1);
    ((WARRIOR_HEALTH_1 + WARRIOR_HEALTH_PER_LEVEL * up) / KIT_HEALTH).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attributes::character;
    use crate::equipment::Gear;
    use crate::model::{ClassStat, LevelStat};

    /// The warrior rows of `player_levelstats` / `player_classlevelstats`, levels 1-10.
    fn warrior() -> (Vec<LevelStat>, Vec<ClassStat>) {
        let rows = [
            (1, 23, 20, 22, 20),
            (2, 24, 21, 23, 29),
            (3, 25, 21, 24, 38),
            (4, 26, 22, 25, 47),
            (5, 28, 23, 26, 56),
            (6, 29, 24, 27, 65),
            (7, 30, 24, 28, 74),
            (8, 31, 25, 29, 83),
            (9, 32, 26, 30, 92),
            (10, 33, 26, 31, 101),
        ];
        let levels = rows
            .iter()
            .map(|&(level, strength, agility, stamina, _)| LevelStat {
                class: 1,
                level,
                strength,
                agility,
                stamina,
                intellect: 20,
                spirit: 20,
            })
            .collect();
        let classes = rows
            .iter()
            .map(|&(level, _, _, _, base_hp)| ClassStat {
                class: 1,
                level,
                base_hp,
                base_mana: 0,
            })
            .collect();
        (levels, classes)
    }

    #[test]
    fn health_grows_with_an_on_level_warriors_blows() {
        let (l, c) = warrior();
        for level in 1..=10u8 {
            let ch = character(&l, &c, 1, level.into(), &Gear::default(), &[]).unwrap();
            let want = ch.melee_ap_factor * TOUGHNESS;
            let got = creature_health_factor(level);
            assert!(
                (got / want - 1.0).abs() < 0.02,
                "level {level}: {got:.3} against {want:.3}"
            );
        }
    }

    #[test]
    fn damage_grows_with_an_on_level_warriors_health() {
        let (l, c) = warrior();
        for level in 1..=10u8 {
            let ch = character(&l, &c, 1, level.into(), &Gear::default(), &[]).unwrap();
            let want = (ch.max_health / KIT_HEALTH).max(1.0);
            let got = creature_damage_factor(level);
            assert!(
                (got - want).abs() < 1.0e-4,
                "level {level}: {got:.3} against {want:.3}"
            );
        }
    }

    #[test]
    fn the_first_levels_keep_the_kits_damage_and_level_zero_reads_as_one() {
        assert_eq!(creature_damage_factor(1), 1.0);
        assert_eq!(creature_damage_factor(3), 1.0);
        assert!(creature_damage_factor(4) > 1.0);
        assert_eq!(creature_health_factor(0), creature_health_factor(1));
        assert_eq!(creature_health_factor(1), TOUGHNESS);
    }
}
