//! What worn gear does in a fight (spec `docs/superpowers/specs/2026-10-12-gear-effects-design.md`).
//! Pure rules over [`Item`] rows and [`Gear`]: armor lowers the damage taken by WoW 1.12's formula, a
//! weapon's WoW damage per second scales the damage the Dark Souls moveset deals (against the
//! starting sword, damped), a shield's block value shrinks a blocked blow.

use crate::equipment::{totals, weapon_dps, Gear, Slot};
use crate::model::Item;

/// WoW 1.12's armor formula: `armor / (armor + 400 + 85 x attacker level)`.
pub const ARMOR_BASE: f32 = 400.0;
pub const ARMOR_PER_LEVEL: f32 = 85.0;
/// The most damage armor can take off.
pub const ARMOR_CAP: f32 = 0.75;
/// The Worn Shortsword's damage per second ((1 + 3) / 2 over 1.9 s): the starting kit is exactly x1.
pub const REF_DPS: f32 = 2.0 / 1.9;
/// How damped the step between weapons is: factor = (dps / REF_DPS) ^ this.
pub const WEAPON_EXPONENT: f32 = 0.5;
pub const WEAPON_FACTOR_MIN: f32 = 0.75;
pub const WEAPON_FACTOR_MAX: f32 = 2.5;
/// Each point of a shield's block value takes this share off a blocked blow's chip damage ...
pub const BLOCK_REDUCTION_PER_POINT: f32 = 0.05;
/// ... up to this share.
pub const BLOCK_REDUCTION_MAX: f32 = 0.5;
/// The attacker level the equipment page quotes its armor figure at: the valley's typical creature.
pub const DISPLAY_LEVEL: u8 = 3;

/// The share of a blow that armor takes off, from a creature of `attacker_level`.
pub fn armor_reduction(armor: u32, attacker_level: u8) -> f32 {
    let armor = armor as f32;
    let against = armor + ARMOR_BASE + ARMOR_PER_LEVEL * f32::from(attacker_level);
    (armor / against).min(ARMOR_CAP)
}

/// How much harder (or softer) this weapon's blows are than the starting sword's: its WoW damage per
/// second against [`REF_DPS`], damped. A row without a swing time or damage, or that is not a weapon,
/// is 1.
pub fn weapon_factor(item: &Item) -> f32 {
    if item.class != 2 {
        return 1.0;
    }
    let dps = weapon_dps(item);
    if dps <= 0.0 {
        return 1.0;
    }
    (dps / REF_DPS)
        .powf(WEAPON_EXPONENT)
        .clamp(WEAPON_FACTOR_MIN, WEAPON_FACTOR_MAX)
}

/// What is left of a blocked blow's chip after the shield's block value.
pub fn block_chip_factor(block: u32) -> f32 {
    1.0 - (BLOCK_REDUCTION_PER_POINT * block as f32).min(BLOCK_REDUCTION_MAX)
}

/// What the worn gear does in a fight, summed once whenever the gear changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GearEffect {
    /// Every worn piece's armor (the shield's too).
    pub armor: u32,
    /// The shield's block value.
    pub block: u32,
    /// The main hand's [`weapon_factor`] (1 with empty hands).
    pub melee_factor: f32,
    /// The ranged slot's [`weapon_factor`] (1 with nothing there).
    pub ranged_factor: f32,
}

impl Default for GearEffect {
    /// Nothing worn: no change to any blow.
    fn default() -> Self {
        Self {
            armor: 0,
            block: 0,
            melee_factor: 1.0,
            ranged_factor: 1.0,
        }
    }
}

impl GearEffect {
    /// What `gear` does in a fight. `off_hand_in_use` is whether the off hand is actually held: a
    /// shield put away (a two-handed grip, a drawn bow) gives neither armor nor block.
    pub fn from_gear(gear: &Gear, items: &[Item], off_hand_in_use: bool) -> Self {
        let mut held = gear.clone();
        if !off_hand_in_use {
            held.set(Slot::OffHand, None);
        }
        let t = totals(&held, items);
        let factor = |slot: Slot| {
            gear.get(slot)
                .and_then(|entry| items.iter().find(|i| i.entry == entry))
                .map_or(1.0, weapon_factor)
        };
        Self {
            armor: t.armor,
            block: t.block,
            melee_factor: factor(Slot::MainHand),
            ranged_factor: factor(Slot::Ranged),
        }
    }

    /// The attributes' share of the effect: armor from Agility, the attack-power factors on the damage
    /// factors.
    pub fn with_character(mut self, c: &crate::attributes::Character) -> Self {
        self.armor += crate::attributes::ARMOR_PER_AGILITY * c.total.agility;
        self.melee_factor *= c.melee_ap_factor;
        self.ranged_factor *= c.ranged_ap_factor;
        self
    }

    /// The share armor takes off a blow of `attacker_level`.
    pub fn reduction(&self, attacker_level: u8) -> f32 {
        armor_reduction(self.armor, attacker_level)
    }

    /// The damage that gets through: armor always, the shield's block only when the blow was blocked
    /// (`damage` is then already the guard's chip).
    pub fn taken(&self, damage: f32, attacker_level: u8, blocked: bool) -> f32 {
        let mut taken = damage * (1.0 - self.reduction(attacker_level));
        if blocked {
            taken *= block_chip_factor(self.block);
        }
        taken
    }

    /// The equipment page's totals line.
    pub fn summary(&self) -> String {
        let mut line = format!("Armor {}", self.armor);
        let off = (self.reduction(DISPLAY_LEVEL) * 100.0).round();
        if off >= 1.0 {
            line.push_str(&format!(" (-{off:.0} %)"));
        }
        if self.block > 0 {
            line.push_str(&format!("     Block {}", self.block));
        }
        if (self.melee_factor - 1.0).abs() >= 0.005 {
            line.push_str(&format!("     Hits x{:.2}", self.melee_factor));
        }
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::{inv, tests::item, Gear, Slot};

    fn close(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    fn weapon(entry: u32, min: f32, max: f32, delay: u32) -> Item {
        let mut i = item(entry, inv::WEAPON, 2, 7);
        i.damage = (min, max, 0);
        i.delay = delay;
        i
    }

    #[test]
    fn armor_follows_wows_formula_and_caps_at_three_quarters() {
        assert_eq!(armor_reduction(0, 3), 0.0);
        assert!(
            close(armor_reduction(655, 3), 0.5, 1.0e-6),
            "655 / (655 + 400 + 255)"
        );
        assert!(close(armor_reduction(275, 3), 275.0 / 930.0, 1.0e-6));
        assert!(
            close(armor_reduction(100, 0), 0.2, 1.0e-6),
            "level 0 divides safely"
        );
        assert_eq!(armor_reduction(u32::MAX, 3), ARMOR_CAP);
        assert_eq!(armor_reduction(u32::MAX, 0), ARMOR_CAP);
    }

    #[test]
    fn the_starting_sword_is_exactly_one_and_better_weapons_climb_damped() {
        assert!(close(
            weapon_factor(&weapon(25, 1.0, 3.0, 1900)),
            1.0,
            1.0e-5
        ));
        // Militia Dagger 2-4 over 1.5 s = 2.0 dps; Hunting Bow 8-16 over 2.6 s = 4.62 dps.
        assert!(close(
            weapon_factor(&weapon(2224, 2.0, 4.0, 1500)),
            1.3784,
            1.0e-3
        ));
        assert!(close(
            weapon_factor(&weapon(8180, 8.0, 16.0, 2600)),
            2.0940,
            1.0e-3
        ));
        // Spiked Club 22-34 over 3.1 s = 9.03 dps: capped.
        assert_eq!(
            weapon_factor(&weapon(4564, 22.0, 34.0, 3100)),
            WEAPON_FACTOR_MAX
        );
        // A hopeless weapon is floored, not zeroed.
        assert_eq!(weapon_factor(&weapon(9, 0.1, 0.1, 3000)), WEAPON_FACTOR_MIN);
    }

    #[test]
    fn a_row_with_no_swing_time_or_no_damage_changes_nothing() {
        assert_eq!(weapon_factor(&weapon(1, 5.0, 9.0, 0)), 1.0);
        assert_eq!(weapon_factor(&weapon(2, 0.0, 0.0, 2000)), 1.0);
        let vest = item(3, inv::CHEST, 4, 2);
        assert_eq!(weapon_factor(&vest), 1.0);
    }

    #[test]
    fn block_takes_five_percent_a_point_off_the_chip_up_to_half() {
        assert_eq!(block_chip_factor(0), 1.0);
        assert!(close(block_chip_factor(1), 0.95, 1.0e-6));
        assert!(close(block_chip_factor(5), 0.75, 1.0e-6));
        assert_eq!(block_chip_factor(100), 0.5);
    }

    #[test]
    fn taken_applies_armor_always_and_block_only_to_a_blocked_blow() {
        let none = GearEffect::default();
        assert_eq!(none.taken(25.0, 4, false), 25.0);
        assert_eq!(none.taken(25.0, 4, true), 25.0);

        let worn = GearEffect {
            armor: 195,
            block: 5,
            melee_factor: 1.0,
            ranged_factor: 1.0,
        };
        // 195 / (195 + 400 + 340) = 0.2086 off a level 4 blow.
        assert!(close(
            worn.taken(25.0, 4, false),
            25.0 * (1.0 - 195.0 / 935.0),
            1.0e-3
        ));
        // A blocked chip of 12.5 also loses a quarter to the shield.
        let armored = worn.taken(12.5, 4, false);
        assert!(close(worn.taken(12.5, 4, true), armored * 0.75, 1.0e-4));
    }

    #[test]
    fn the_start_kit_is_armor_seven_and_no_change_to_damage() {
        let sword = weapon(25, 1.0, 3.0, 1900);
        let mut shield = item(2362, inv::SHIELD, 4, 6);
        shield.armor = 5;
        shield.block = 1;
        let mut pants = item(39, inv::LEGS, 4, 1);
        pants.armor = 2;
        let items = vec![sword, shield, pants];
        let mut gear = Gear::default();
        gear.set(Slot::MainHand, Some(25));
        gear.set(Slot::OffHand, Some(2362));
        gear.set(Slot::Legs, Some(39));
        let fx = GearEffect::from_gear(&gear, &items, true);
        assert_eq!((fx.armor, fx.block), (7, 1));
        assert!(close(fx.melee_factor, 1.0, 1.0e-5));
        assert_eq!(fx.ranged_factor, 1.0);
        assert_eq!(fx.summary(), "Armor 7 (-1 %)     Block 1");
    }

    #[test]
    fn a_stowed_shield_gives_neither_armor_nor_block() {
        // Two-handing a weapon or drawing a bow puts the shield away: it is not held, so it does
        // not guard or protect (final review, Important: a free defence otherwise).
        let sword = weapon(25, 1.0, 3.0, 1900);
        let mut shield = item(2362, inv::SHIELD, 4, 6);
        shield.armor = 5;
        shield.block = 1;
        let mut pants = item(39, inv::LEGS, 4, 1);
        pants.armor = 2;
        let items = vec![sword, shield, pants];
        let mut gear = Gear::default();
        gear.set(Slot::MainHand, Some(25));
        gear.set(Slot::OffHand, Some(2362));
        gear.set(Slot::Legs, Some(39));
        let held = GearEffect::from_gear(&gear, &items, true);
        let stowed = GearEffect::from_gear(&gear, &items, false);
        assert_eq!((held.armor, held.block), (7, 1));
        assert_eq!((stowed.armor, stowed.block), (2, 0), "the pants only");
        assert_eq!(stowed.melee_factor, held.melee_factor);
        // The worn gear itself is untouched.
        assert_eq!(gear.get(Slot::OffHand), Some(2362));
    }

    #[test]
    fn a_ranged_weapon_scales_only_the_ranged_factor() {
        let items = vec![weapon(25, 1.0, 3.0, 1900), {
            let mut bow = weapon(8180, 8.0, 16.0, 2600);
            bow.inventory_type = inv::RANGED;
            bow.subclass = 2;
            bow
        }];
        let mut gear = Gear::default();
        gear.set(Slot::MainHand, Some(25));
        gear.set(Slot::Ranged, Some(8180));
        let fx = GearEffect::from_gear(&gear, &items, true);
        assert!(close(fx.melee_factor, 1.0, 1.0e-5));
        assert!(close(fx.ranged_factor, 2.0940, 1.0e-3));
    }

    #[test]
    fn empty_gear_and_unknown_items_are_neutral() {
        assert_eq!(
            GearEffect::from_gear(&Gear::default(), &[], true),
            GearEffect::default()
        );
        let mut gear = Gear::default();
        gear.set(Slot::MainHand, Some(999_999));
        gear.set(Slot::Chest, Some(888_888));
        assert_eq!(
            GearEffect::from_gear(&gear, &[], true),
            GearEffect::default()
        );
        assert_eq!(GearEffect::default().summary(), "Armor 0");
    }

    #[test]
    fn the_summary_quotes_the_level_three_figure_and_the_weapon() {
        let fx = GearEffect {
            armor: 275,
            block: 3,
            melee_factor: 1.378,
            ranged_factor: 1.0,
        };
        assert_eq!(fx.summary(), "Armor 275 (-30 %)     Block 3     Hits x1.38");
    }
}
