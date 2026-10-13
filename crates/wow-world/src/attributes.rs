//! A character's WoW attributes (spec `docs/superpowers/specs/2026-10-12-attributes-and-boss-rules-design.md`):
//! the database's base values for the class and level plus what the worn gear adds, and what they do:
//! health from Stamina, attack power from Strength (melee) and Agility (ranged), armor from Agility.
//! Intellect and Spirit are counted and shown but do nothing yet (no mana, no regeneration).

use crate::equipment::Gear;
use crate::gear_effect::REF_DPS;
use crate::model::{ClassStat, Item, LevelStat};

/// `item_template.stat_type` ids in the 1.12 data.
pub mod stat {
    pub const MANA: u32 = 0;
    pub const HEALTH: u32 = 1;
    pub const AGILITY: u32 = 3;
    pub const STRENGTH: u32 = 4;
    pub const INTELLECT: u32 = 5;
    pub const SPIRIT: u32 = 6;
    pub const STAMINA: u32 = 7;
}

/// Added to every max health (0: WoW's own numbers, the user's choice; +40 puts level 1 back at 100).
pub const HEALTH_OFFSET: f32 = 0.0;
/// The first this many Stamina points give a health point each; every point above gives [`HP_PER_STAMINA`].
pub const STAMINA_FREE: u32 = 20;
pub const HP_PER_STAMINA: f32 = 10.0;
/// Warrior melee attack power: `level x 3 + Strength x 2 - 20`.
pub const AP_PER_LEVEL: f32 = 3.0;
pub const AP_PER_STRENGTH: f32 = 2.0;
pub const AP_BASE: f32 = -20.0;
/// Warrior ranged attack power (as MaNGOS has it): `level + Agility - 10`.
pub const RAP_PER_LEVEL: f32 = 1.0;
pub const RAP_BASE: f32 = -10.0;
/// One damage per second per this much attack power.
pub const AP_PER_DPS: f32 = 14.0;
/// Armor each point of Agility gives.
pub const ARMOR_PER_AGILITY: u32 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attributes {
    pub strength: u32,
    pub agility: u32,
    pub stamina: u32,
    pub intellect: u32,
    pub spirit: u32,
}

/// What the worn gear adds to each attribute (can be negative in the data; the total is floored at 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bonus {
    pub strength: i32,
    pub agility: i32,
    pub stamina: i32,
    pub intellect: i32,
    pub spirit: i32,
}

/// Every worn piece's stats summed per attribute. Unknown stat types (and mana / health) are ignored.
pub fn gear_bonus(gear: &Gear, items: &[Item]) -> Bonus {
    let mut b = Bonus::default();
    for (_, entry) in gear.iter() {
        let Some(item) = items.iter().find(|i| i.entry == entry) else {
            continue;
        };
        for &(kind, value) in &item.stats {
            match kind {
                stat::STRENGTH => b.strength += value,
                stat::AGILITY => b.agility += value,
                stat::STAMINA => b.stamina += value,
                stat::INTELLECT => b.intellect += value,
                stat::SPIRIT => b.spirit += value,
                _ => {}
            }
        }
    }
    b
}

/// The health Stamina gives.
pub fn stamina_hp(stamina: u32) -> f32 {
    let free = stamina.min(STAMINA_FREE) as f32;
    let above = stamina.saturating_sub(STAMINA_FREE) as f32;
    free + above * HP_PER_STAMINA
}

pub fn attack_power(level: u32, strength: u32) -> f32 {
    (level as f32 * AP_PER_LEVEL + strength as f32 * AP_PER_STRENGTH + AP_BASE).max(0.0)
}

pub fn ranged_attack_power(level: u32, agility: u32) -> f32 {
    (level as f32 * RAP_PER_LEVEL + agility as f32 + RAP_BASE).max(0.0)
}

/// How much harder a blow lands at `ap` than at `ap_ref`, for the starting weapon: WoW adds `ap / 14`
/// damage per second to the weapon's own, normalised so `ap_ref` is exactly 1.
pub fn ap_factor(ap: f32, ap_ref: f32) -> f32 {
    (REF_DPS + ap / AP_PER_DPS) / (REF_DPS + ap_ref / AP_PER_DPS)
}

/// The row of `class` at `level`, the level clamped to the class's first and last row.
fn row_at<T: Copy>(
    rows: &[T],
    class: u32,
    level: u32,
    key: impl Fn(&T) -> (u32, u32),
) -> Option<T> {
    let of_class: Vec<&T> = rows.iter().filter(|r| key(r).0 == class).collect();
    let first = of_class.iter().map(|r| key(r).1).min()?;
    let last = of_class.iter().map(|r| key(r).1).max()?;
    let want = level.clamp(first, last);
    of_class.into_iter().find(|r| key(r).1 == want).copied()
}

/// The lowest and highest level the table has for `class`.
fn level_bounds(rows: &[LevelStat], class: u32) -> Option<(u32, u32)> {
    let mut levels = rows.iter().filter(|r| r.class == class).map(|r| r.level);
    let first = levels.next()?;
    Some(levels.fold((first, first), |(lo, hi), l| (lo.min(l), hi.max(l))))
}

/// What a character is and does from its class tables and gear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Character {
    /// The database's base attributes at the level.
    pub base: Attributes,
    /// Base plus gear.
    pub total: Attributes,
    pub max_health: f32,
    pub melee_ap_factor: f32,
    pub ranged_ap_factor: f32,
}

fn add(base: u32, bonus: i32) -> u32 {
    (i64::from(base) + i64::from(bonus)).max(0) as u32
}

/// `None` when the tables have no row for the class (a manifest older than `wsworld 3`, or a class the
/// game does not know): the caller keeps the old fixed vitals.
pub fn character(
    level_stats: &[LevelStat],
    class_stats: &[ClassStat],
    class: u32,
    level: u32,
    gear: &Gear,
    items: &[Item],
) -> Option<Character> {
    let l = row_at(level_stats, class, level, |r| (r.class, r.level))?;
    let c = row_at(class_stats, class, level, |r| (r.class, r.level))?;
    let first = row_at(level_stats, class, 1, |r| (r.class, r.level))?;
    let base = Attributes {
        strength: l.strength,
        agility: l.agility,
        stamina: l.stamina,
        intellect: l.intellect,
        spirit: l.spirit,
    };
    let b = gear_bonus(gear, items);
    let total = Attributes {
        strength: add(base.strength, b.strength),
        agility: add(base.agility, b.agility),
        stamina: add(base.stamina, b.stamina),
        intellect: add(base.intellect, b.intellect),
        spirit: add(base.spirit, b.spirit),
    };
    // The level the tables stand for: a level above the table reads its last row, below its first.
    let (lo, hi) = level_bounds(level_stats, class)?;
    let lvl = level.clamp(lo, hi);
    let max_health = c.base_hp as f32 + stamina_hp(total.stamina) + HEALTH_OFFSET;
    let ap_ref = attack_power(1, first.strength);
    let rap_ref = ranged_attack_power(1, first.agility);
    Some(Character {
        base,
        total,
        max_health,
        melee_ap_factor: ap_factor(attack_power(lvl, total.strength), ap_ref),
        ranged_ap_factor: ap_factor(ranged_attack_power(lvl, total.agility), rap_ref),
    })
}

/// The equipment page's attribute lines: each attribute with the gear's share, then health.
pub fn sheet(c: &Character) -> String {
    let one = |name: &str, base: u32, total: u32| {
        if total > base {
            format!("{name} {total} (+{})", total - base)
        } else {
            format!("{name} {total}")
        }
    };
    format!(
        "{}   {}   {}\n{}   {}   Health {}",
        one("Strength", c.base.strength, c.total.strength),
        one("Agility", c.base.agility, c.total.agility),
        one("Stamina", c.base.stamina, c.total.stamina),
        one("Intellect", c.base.intellect, c.total.intellect),
        one("Spirit", c.base.spirit, c.total.spirit),
        c.max_health.round() as i64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::{inv, tests::item, Gear, Slot};
    use crate::model::{ClassStat, Item, LevelStat};

    fn close(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    /// Human Warrior (class 1), levels 1-8, from the vMaNGOS tables.
    fn tables() -> (Vec<LevelStat>, Vec<ClassStat>) {
        let rows = [
            (1, 23, 20, 22, 20, 20, 20),
            (2, 24, 21, 23, 20, 20, 29),
            (3, 25, 21, 24, 20, 21, 38),
            (4, 26, 22, 25, 20, 22, 47),
            (5, 28, 23, 26, 20, 22, 56),
            (6, 29, 24, 27, 21, 22, 65),
            (7, 30, 24, 28, 21, 23, 74),
            (8, 31, 25, 29, 21, 23, 83),
        ];
        let levels = rows
            .iter()
            .map(|&(level, s, a, st, i, sp, _)| LevelStat {
                class: 1,
                level,
                strength: s,
                agility: a,
                stamina: st,
                intellect: i,
                spirit: sp,
            })
            .collect();
        let classes = rows
            .iter()
            .map(|&(level, _, _, _, _, _, hp)| ClassStat {
                class: 1,
                level,
                base_hp: hp,
                base_mana: 0,
            })
            .collect();
        (levels, classes)
    }

    fn with_stats(entry: u32, inventory_type: u32, stats: Vec<(u32, i32)>) -> Item {
        let mut i = item(entry, inventory_type, 4, 2);
        i.stats = stats;
        i
    }

    #[test]
    fn stamina_gives_a_point_each_to_twenty_and_ten_above() {
        assert_eq!(stamina_hp(0), 0.0);
        assert_eq!(stamina_hp(20), 20.0);
        assert_eq!(stamina_hp(21), 30.0);
        assert_eq!(stamina_hp(22), 40.0);
        assert_eq!(stamina_hp(26), 80.0);
    }

    #[test]
    fn a_human_warriors_health_follows_the_class_table_and_stamina() {
        let (l, c) = tables();
        let at = |level| {
            character(&l, &c, 1, level, &Gear::default(), &[])
                .unwrap()
                .max_health
        };
        assert_eq!(at(1), 60.0, "20 base + 40 from 22 stamina");
        assert_eq!(at(3), 98.0);
        assert_eq!(at(5), 136.0);
        assert_eq!(at(8), 193.0);
    }

    #[test]
    fn gear_stats_add_to_the_base_and_four_stamina_is_forty_health() {
        let (l, c) = tables();
        let boots = with_stats(6095, inv::FEET, vec![(stat::STAMINA, 4), (stat::SPIRIT, 4)]);
        let mut gear = Gear::default();
        gear.set(Slot::Feet, Some(6095));
        let bare = character(&l, &c, 1, 1, &Gear::default(), &[]).unwrap();
        let shod = character(&l, &c, 1, 1, &gear, &[boots]).unwrap();
        assert_eq!(shod.total.stamina, bare.total.stamina + 4);
        assert_eq!(shod.total.spirit, bare.total.spirit + 4);
        assert_eq!(shod.base, bare.base, "the base does not move");
        assert_eq!(shod.max_health - bare.max_health, 40.0);
    }

    #[test]
    fn attack_power_and_its_factor_start_at_one_for_a_level_one_warrior() {
        assert_eq!(attack_power(1, 23), 29.0);
        assert_eq!(attack_power(5, 28), 51.0);
        assert_eq!(attack_power(1, 0), 0.0, "never negative");
        let (l, c) = tables();
        let one = character(&l, &c, 1, 1, &Gear::default(), &[]).unwrap();
        assert!(close(one.melee_ap_factor, 1.0, 1.0e-6));
        assert!(close(one.ranged_ap_factor, 1.0, 1.0e-6));
        let five = character(&l, &c, 1, 5, &Gear::default(), &[]).unwrap();
        assert!(close(five.melee_ap_factor, 1.503, 2.0e-3));
        let eight = character(&l, &c, 1, 8, &Gear::default(), &[]).unwrap();
        assert!(close(eight.melee_ap_factor, 1.846, 2.0e-3));
    }

    #[test]
    fn strength_from_gear_raises_the_factor() {
        let (l, c) = tables();
        let belt = with_stats(9, inv::WAIST, vec![(stat::STRENGTH, 3)]);
        let mut gear = Gear::default();
        gear.set(Slot::Waist, Some(9));
        let bare = character(&l, &c, 1, 5, &Gear::default(), &[]).unwrap();
        let belted = character(&l, &c, 1, 5, &gear, &[belt]).unwrap();
        assert!(belted.melee_ap_factor > bare.melee_ap_factor + 0.1);
    }

    #[test]
    fn a_level_outside_the_table_uses_its_first_or_last_row() {
        let (l, c) = tables();
        let zero = character(&l, &c, 1, 0, &Gear::default(), &[]).unwrap();
        let one = character(&l, &c, 1, 1, &Gear::default(), &[]).unwrap();
        assert_eq!(zero.max_health, one.max_health);
        let high = character(&l, &c, 1, 61, &Gear::default(), &[]).unwrap();
        let top = character(&l, &c, 1, 8, &Gear::default(), &[]).unwrap();
        assert_eq!(high.total, top.total);
    }

    #[test]
    fn no_table_row_for_the_class_means_no_character() {
        let (l, c) = tables();
        assert!(character(&l, &c, 4, 1, &Gear::default(), &[]).is_none());
        assert!(character(&[], &[], 1, 1, &Gear::default(), &[]).is_none());
    }

    #[test]
    fn unknown_stat_types_are_ignored_and_a_total_never_goes_below_zero() {
        let (l, c) = tables();
        let odd = with_stats(
            3,
            inv::HEAD,
            vec![(99, 5), (stat::STRENGTH, -1000), (stat::HEALTH, 7)],
        );
        let mut gear = Gear::default();
        gear.set(Slot::Head, Some(3));
        let ch = character(&l, &c, 1, 1, &gear, &[odd]).unwrap();
        assert_eq!(ch.total.strength, 0);
        assert_eq!(ch.total.agility, 20);
    }

    #[test]
    fn the_effect_gains_two_armor_per_agility_and_the_attack_power_factors() {
        use crate::gear_effect::GearEffect;
        let (l, c) = tables();
        let ch = character(&l, &c, 1, 5, &Gear::default(), &[]).unwrap();
        let fx = GearEffect::default().with_character(&ch);
        assert_eq!(fx.armor, 2 * 23);
        assert!(close(fx.melee_factor, ch.melee_ap_factor, 1.0e-6));
        assert!(close(fx.ranged_factor, ch.ranged_ap_factor, 1.0e-6));
    }

    #[test]
    fn the_sheet_shows_base_plus_gear_and_health() {
        let (l, c) = tables();
        let belt = with_stats(9, inv::WAIST, vec![(stat::STRENGTH, 3), (stat::STAMINA, 4)]);
        let mut gear = Gear::default();
        gear.set(Slot::Waist, Some(9));
        let ch = character(&l, &c, 1, 5, &gear, &[belt]).unwrap();
        assert_eq!(
            sheet(&ch),
            "Strength 31 (+3)   Agility 23   Stamina 30 (+4)\nIntellect 20   Spirit 22   Health 176"
        );
    }
}
