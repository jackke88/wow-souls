//! The paper doll: WoW 1.12's equipment slots, which item goes in which slot and who may wear what.
//! Pure rules over [`Item`] rows; `inventory::Progress` keeps the worn items beside the carried ones
//! so a single save (and a single atomic publish) covers both.
//!
//! The slots are the character pane's. The server numbers nineteen of them (vMaNGOS
//! `EQUIPMENT_SLOT_*`, `Player::EquipItem`): head, neck, shoulders, shirt, chest, waist, legs, feet,
//! wrists, hands, two fingers, two trinkets, back, main hand, off hand, ranged and tabard. The pane
//! also shows the ammo slot under the weapons, so it is the twentieth here. The four bag slots are
//! left out: carrying has no capacity in this game.
//!
//! What it does not model: weapon skills learned from a trainer (a warrior may use every weapon type
//! his class can ever learn), bind-on-equip, durability and reputation-gated items.

use anyhow::{bail, ensure, Result};

use crate::model::Item;

/// Equipment slots counting the ammo slot.
pub const SLOT_COUNT: usize = 21;

/// One place on the paper doll. The discriminant is the server's equipment slot number (ammo, which
/// the server keeps outside the numbered slots, takes the next free one).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Slot {
    #[default]
    Head,
    Neck,
    Shoulders,
    Shirt,
    Chest,
    Waist,
    Legs,
    Feet,
    Wrists,
    Hands,
    Finger1,
    Finger2,
    Trinket1,
    Trinket2,
    Back,
    MainHand,
    OffHand,
    Ranged,
    Tabard,
    Ammo,
    /// The torch at the belt (2026-10-18, ours, not WoW's: the user wants one for the dark nights).
    Torch,
}

impl Slot {
    /// Every slot in the server's order.
    pub const ALL: [Slot; SLOT_COUNT] = [
        Slot::Head,
        Slot::Neck,
        Slot::Shoulders,
        Slot::Shirt,
        Slot::Chest,
        Slot::Waist,
        Slot::Legs,
        Slot::Feet,
        Slot::Wrists,
        Slot::Hands,
        Slot::Finger1,
        Slot::Finger2,
        Slot::Trinket1,
        Slot::Trinket2,
        Slot::Back,
        Slot::MainHand,
        Slot::OffHand,
        Slot::Ranged,
        Slot::Tabard,
        Slot::Ammo,
        Slot::Torch,
    ];

    /// The server's slot number (0 head ... 18 tabard, 19 ammo here).
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(index: usize) -> Option<Slot> {
        Self::ALL.get(index).copied()
    }

    /// The name the character pane gives the slot.
    pub fn name(self) -> &'static str {
        match self {
            Slot::Head => "Head",
            Slot::Neck => "Neck",
            Slot::Shoulders => "Shoulders",
            Slot::Shirt => "Shirt",
            Slot::Chest => "Chest",
            Slot::Waist => "Waist",
            Slot::Legs => "Legs",
            Slot::Feet => "Feet",
            Slot::Wrists => "Wrist",
            Slot::Hands => "Hands",
            Slot::Finger1 | Slot::Finger2 => "Finger",
            Slot::Trinket1 | Slot::Trinket2 => "Trinket",
            Slot::Back => "Back",
            Slot::MainHand => "Main Hand",
            Slot::OffHand => "Off Hand",
            Slot::Ranged => "Ranged",
            Slot::Tabard => "Tabard",
            Slot::Ammo => "Ammo",
            Slot::Torch => "Torch",
        }
    }

    /// The word the save file uses.
    pub fn key(self) -> &'static str {
        match self {
            Slot::Head => "head",
            Slot::Neck => "neck",
            Slot::Shoulders => "shoulders",
            Slot::Shirt => "shirt",
            Slot::Chest => "chest",
            Slot::Waist => "waist",
            Slot::Legs => "legs",
            Slot::Feet => "feet",
            Slot::Wrists => "wrists",
            Slot::Hands => "hands",
            Slot::Finger1 => "finger1",
            Slot::Finger2 => "finger2",
            Slot::Trinket1 => "trinket1",
            Slot::Trinket2 => "trinket2",
            Slot::Back => "back",
            Slot::MainHand => "mainhand",
            Slot::OffHand => "offhand",
            Slot::Ranged => "ranged",
            Slot::Tabard => "tabard",
            Slot::Ammo => "ammo",
            Slot::Torch => "torch",
        }
    }

    pub fn from_key(key: &str) -> Option<Slot> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }
}

/// The 1.12 character pane, left column top to bottom (`PaperDollFrame.xml`).
pub const LEFT_COLUMN: [Slot; 8] = [
    Slot::Head,
    Slot::Neck,
    Slot::Shoulders,
    Slot::Back,
    Slot::Chest,
    Slot::Shirt,
    Slot::Tabard,
    Slot::Wrists,
];
/// The right column.
pub const RIGHT_COLUMN: [Slot; 8] = [
    Slot::Hands,
    Slot::Waist,
    Slot::Legs,
    Slot::Feet,
    Slot::Finger1,
    Slot::Finger2,
    Slot::Trinket1,
    Slot::Trinket2,
];
/// The row under the doll.
pub const WEAPON_ROW: [Slot; 4] = [Slot::MainHand, Slot::OffHand, Slot::Ranged, Slot::Ammo];
/// Under the weapons: the belt torch (ours).
pub const BELT_ROW: [Slot; 1] = [Slot::Torch];

/// `item_template.inventory_type`.
pub mod inv {
    pub const HEAD: u32 = 1;
    pub const NECK: u32 = 2;
    pub const SHOULDERS: u32 = 3;
    pub const BODY: u32 = 4;
    pub const CHEST: u32 = 5;
    pub const WAIST: u32 = 6;
    pub const LEGS: u32 = 7;
    pub const FEET: u32 = 8;
    pub const WRISTS: u32 = 9;
    pub const HANDS: u32 = 10;
    pub const FINGER: u32 = 11;
    pub const TRINKET: u32 = 12;
    /// A one-handed weapon that goes in either hand.
    pub const WEAPON: u32 = 13;
    pub const SHIELD: u32 = 14;
    /// A bow.
    pub const RANGED: u32 = 15;
    pub const CLOAK: u32 = 16;
    pub const TWO_HAND: u32 = 17;
    pub const BAG: u32 = 18;
    pub const TABARD: u32 = 19;
    /// A robe: a chest piece that also covers the legs.
    pub const ROBE: u32 = 20;
    pub const MAIN_HAND: u32 = 21;
    pub const OFF_HAND: u32 = 22;
    /// A held-in-off-hand item (a torch, a tome).
    pub const HOLDABLE: u32 = 23;
    pub const AMMO: u32 = 24;
    pub const THROWN: u32 = 25;
    /// A gun, crossbow or wand.
    pub const RANGED_RIGHT: u32 = 26;
    pub const QUIVER: u32 = 27;
    /// Ours, past WoW's types: a torch for the belt ([`super::Slot::Torch`]).
    pub const TORCH: u32 = 900;
}

/// The slots an item of this `inventory_type` can go in, in the order a quick equip tries them
/// (`Player::FindEquipSlot`). Empty for a bag, a quiver or anything that is not worn.
pub fn slots_for(inventory_type: u32) -> &'static [Slot] {
    match inventory_type {
        inv::HEAD => &[Slot::Head],
        inv::NECK => &[Slot::Neck],
        inv::SHOULDERS => &[Slot::Shoulders],
        inv::BODY => &[Slot::Shirt],
        inv::CHEST | inv::ROBE => &[Slot::Chest],
        inv::WAIST => &[Slot::Waist],
        inv::LEGS => &[Slot::Legs],
        inv::FEET => &[Slot::Feet],
        inv::WRISTS => &[Slot::Wrists],
        inv::HANDS => &[Slot::Hands],
        inv::FINGER => &[Slot::Finger1, Slot::Finger2],
        inv::TRINKET => &[Slot::Trinket1, Slot::Trinket2],
        inv::WEAPON => &[Slot::MainHand, Slot::OffHand],
        inv::SHIELD | inv::OFF_HAND | inv::HOLDABLE => &[Slot::OffHand],
        inv::RANGED | inv::THROWN | inv::RANGED_RIGHT => &[Slot::Ranged],
        inv::CLOAK => &[Slot::Back],
        inv::TWO_HAND | inv::MAIN_HAND => &[Slot::MainHand],
        inv::TABARD => &[Slot::Tabard],
        inv::AMMO => &[Slot::Ammo],
        inv::TORCH => &[Slot::Torch],
        _ => &[],
    }
}

/// Armor `subclass` values (`item_template.class` 4).
mod armor {
    pub const MISC: u32 = 0;
    pub const CLOTH: u32 = 1;
    pub const LEATHER: u32 = 2;
    pub const MAIL: u32 = 3;
    pub const PLATE: u32 = 4;
    pub const SHIELD: u32 = 6;
}

/// Who is wearing: the three things the rules read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wearer {
    /// `ChrRaces` id: 1 human, 2 orc, 3 dwarf, 4 night elf, 5 undead, 6 tauren, 7 gnome, 8 troll.
    pub race: u8,
    /// `ChrClasses` id: 1 warrior, 2 paladin, 3 hunter, 4 rogue, 5 priest, 7 shaman, 8 mage,
    /// 9 warlock, 11 druid.
    pub class: u8,
    pub level: u32,
}

pub const RACE_HUMAN: u8 = 1;
pub const CLASS_WARRIOR: u8 = 1;
pub const CLASS_MAGE: u8 = 8;
/// The level a warrior can plate himself (the Plate Mail proficiency).
pub const WARRIOR_PLATE_LEVEL: u32 = 40;
/// The level a warrior learns Dual Wield, and so can put a one-handed weapon in the off hand.
pub const WARRIOR_DUAL_WIELD_LEVEL: u32 = 20;

impl Wearer {
    pub fn new(race: u8, class: u8, level: u32) -> Self {
        Self { race, class, level }
    }

    fn class_bit(self) -> i32 {
        1i32.checked_shl(u32::from(self.class).saturating_sub(1))
            .unwrap_or(0)
    }

    fn race_bit(self) -> i32 {
        1i32.checked_shl(u32::from(self.race).saturating_sub(1))
            .unwrap_or(0)
    }

    /// Whether the class wears this armor type at this level. Only the warrior's table is written
    /// (the one class the game can make); another class is let through.
    fn wears_armor(self, subclass: u32) -> bool {
        if self.class != CLASS_WARRIOR {
            return true;
        }
        match subclass {
            armor::MISC | armor::CLOTH | armor::LEATHER | armor::MAIL | armor::SHIELD => true,
            armor::PLATE => self.level >= WARRIOR_PLATE_LEVEL,
            _ => false,
        }
    }

    /// Whether the class can ever learn this weapon type (`item_template.subclass` of class 2).
    fn wields_weapon(self, subclass: u32) -> bool {
        if self.class != CLASS_WARRIOR {
            return true;
        }
        matches!(
            subclass,
            0 | 1 // axes
            | 2 // bows
            | 3 // guns
            | 4 | 5 // maces
            | 6 // polearms
            | 7 | 8 // swords
            | 10 // staves
            | 13 // fist weapons
            | 15 // daggers
            | 16 // thrown
            | 18 // crossbows
            | 20 // fishing poles
        )
    }

    /// Whether a second one-handed weapon may go in the off hand.
    pub fn dual_wields(self) -> bool {
        self.class == CLASS_WARRIOR && self.level >= WARRIOR_DUAL_WIELD_LEVEL
    }
}

/// Why an item could not be equipped. [`Refusal::message`] is WoW's wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The item is not carried.
    NotOwned,
    /// A bag, a potion, a quest item: nothing to put on.
    NotWearable,
    /// The item does not go in that slot.
    WrongSlot,
    /// The required level.
    Level(u32),
    /// The class or race can never use it.
    Never,
    /// The armor or weapon type is not the class's (yet).
    Proficiency,
    /// A one-handed weapon in the off hand without Dual Wield.
    DualWield,
    /// A two-handed weapon in the main hand leaves no hand for it.
    TwoHanded,
    /// Nothing is worn there.
    Empty,
}

impl Refusal {
    pub fn message(self) -> String {
        match self {
            Refusal::NotOwned => "You do not have that item.".into(),
            Refusal::NotWearable => "That item can't be equipped.".into(),
            Refusal::WrongSlot => "That item does not go in that slot.".into(),
            Refusal::Level(n) => format!("You must reach level {n} to use that item."),
            Refusal::Never => "You can never use that item.".into(),
            Refusal::Proficiency => "You don't have the required proficiency for that item.".into(),
            Refusal::DualWield => "You cannot dual-wield.".into(),
            Refusal::TwoHanded => "Cannot equip that with a two-handed weapon.".into(),
            Refusal::Empty => "There is nothing to take off.".into(),
        }
    }
}

/// Whether `wearer` may use `item` at all: wearable, class, race, level, proficiency.
pub fn can_wear(item: &Item, wearer: Wearer) -> Result<(), Refusal> {
    if slots_for(item.inventory_type).is_empty() {
        return Err(Refusal::NotWearable);
    }
    // `-1` is every class or race; 0 is a data hole and means the same.
    let allows = |mask: i32, bit: i32| mask <= 0 || mask & bit != 0;
    if !allows(item.allowable_class, wearer.class_bit())
        || !allows(item.allowable_race, wearer.race_bit())
    {
        return Err(Refusal::Never);
    }
    if item.required_level > wearer.level {
        return Err(Refusal::Level(item.required_level));
    }
    match item.class {
        4 if !wearer.wears_armor(item.subclass) => Err(Refusal::Proficiency),
        2 if !wearer.wields_weapon(item.subclass) => Err(Refusal::Proficiency),
        _ => Ok(()),
    }
}

/// What is worn, by slot. Item entries; the carried count lives in the inventory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Gear {
    worn: [Option<u32>; SLOT_COUNT],
}

impl Gear {
    pub fn get(&self, slot: Slot) -> Option<u32> {
        self.worn[slot.index()]
    }

    pub fn set(&mut self, slot: Slot, item: Option<u32>) {
        self.worn[slot.index()] = item;
    }

    /// The worn items in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Slot, u32)> + '_ {
        Slot::ALL
            .into_iter()
            .filter_map(|s| self.get(s).map(|item| (s, item)))
    }

    pub fn is_empty(&self) -> bool {
        self.worn.iter().all(Option::is_none)
    }

    /// Whether `item` is worn anywhere.
    pub fn wears(&self, item: u32) -> bool {
        self.worn.contains(&Some(item))
    }

    /// Where a quick equip puts `item`: the first legal slot that is empty, else the first legal
    /// one (`Player::FindEquipSlot`; a ring swaps with the first finger).
    pub fn best_slot(&self, items: &[Item], item: &Item, wearer: Wearer) -> Option<Slot> {
        let legal: Vec<Slot> = slots_for(item.inventory_type)
            .iter()
            .copied()
            .filter(|&s| self.plan(items, item, s, wearer).is_ok())
            .collect();
        legal
            .iter()
            .copied()
            .find(|&s| self.get(s).is_none())
            .or_else(|| legal.first().copied())
    }

    /// Where `item` goes without moving anything worn: the first legal slot, in a quick equip's
    /// order, that is empty and takes the item without taking anything else off (a two-hander
    /// beside a shield, a shield beside a two-hander, a level the wearer lacks: no slot).
    pub fn free_slot(&self, items: &[Item], item: &Item, wearer: Wearer) -> Option<Slot> {
        slots_for(item.inventory_type).iter().copied().find(|&s| {
            self.get(s).is_none()
                && self
                    .plan(items, item, s, wearer)
                    .is_ok_and(|taken| taken.is_empty())
        })
    }

    /// Puts `item` in `slot`: what the move takes off, in slot order (the slot's own occupant, and
    /// the off hand when a two-handed weapon needs both). The gear is not touched.
    pub fn plan(
        &self,
        items: &[Item],
        item: &Item,
        slot: Slot,
        wearer: Wearer,
    ) -> Result<Vec<(Slot, u32)>, Refusal> {
        can_wear(item, wearer)?;
        if !slots_for(item.inventory_type).contains(&slot) {
            return Err(Refusal::WrongSlot);
        }
        let two_hander = |entry: u32| {
            items
                .iter()
                .find(|i| i.entry == entry)
                .is_some_and(|i| i.inventory_type == inv::TWO_HAND)
        };
        let mut off = Vec::new();
        match slot {
            Slot::OffHand => {
                if item.inventory_type == inv::WEAPON && !wearer.dual_wields() {
                    return Err(Refusal::DualWield);
                }
                if self.get(Slot::MainHand).is_some_and(two_hander) {
                    return Err(Refusal::TwoHanded);
                }
            }
            Slot::MainHand if item.inventory_type == inv::TWO_HAND => {
                off.extend(self.get(Slot::OffHand).map(|o| (Slot::OffHand, o)));
            }
            _ => {}
        }
        let mut taken: Vec<(Slot, u32)> = self.get(slot).map(|o| (slot, o)).into_iter().collect();
        taken.extend(off);
        taken.sort_by_key(|&(s, _)| s);
        Ok(taken)
    }
}

/// What the worn armor adds up to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub armor: u32,
    /// The shield's block value.
    pub block: u32,
}

pub fn totals(gear: &Gear, items: &[Item]) -> Totals {
    let mut t = Totals::default();
    for (_, entry) in gear.iter() {
        if let Some(i) = items.iter().find(|i| i.entry == entry) {
            t.armor += i.armor;
            t.block += i.block;
        }
    }
    t
}

/// Damage per second of a weapon row, as the tooltip prints it: the mean of the range over the
/// swing time. 0 for an item without a swing time.
pub fn weapon_dps(item: &Item) -> f32 {
    if item.delay == 0 {
        return 0.0;
    }
    (item.damage.0 + item.damage.1) * 0.5 / (item.delay as f32 / 1000.0)
}

/// What a new character wears: vMaNGOS `playercreateinfo_item` for race 1, class 1 (a human warrior),
/// the pieces that go on a slot (the jerky and the hearthstone go in a bag).
pub const HUMAN_WARRIOR_KIT: [(Slot, u32); 5] = [
    (Slot::MainHand, 25),  // Worn Shortsword, display 1542
    (Slot::OffHand, 2362), // Worn Wooden Shield
    (Slot::Shirt, 38),     // Recruit's Shirt
    (Slot::Legs, 39),      // Recruit's Pants
    (Slot::Feet, 40),      // Recruit's Boots
];

/// What a new human mage wears: vMaNGOS `playercreateinfo_item` for race 1, class 8, the pieces that go
/// on a slot (two Refreshing Spring Water, four Darnassian Bleu and the hearthstone go in a bag).
pub const HUMAN_MAGE_KIT: [(Slot, u32); 5] = [
    (Slot::MainHand, 35), // Bent Staff, two-handed staff 3-5, 2.9 s
    (Slot::Shirt, 6096),  // Apprentice's Shirt
    (Slot::Chest, 56),    // Apprentice's Robe
    (Slot::Legs, 1395),   // Apprentice's Pants
    (Slot::Feet, 55),     // Apprentice's Boots
];

/// What a level 11 trial character wears (`wow-souls --trial-character`, the Hogger fight's intended
/// level): real 1.12 items, all green, required level 9-12, picked from `item_template`. Part of the
/// item selection like the starting kit, so a save that names them finds their rows.
pub const TRIAL_GEAR: [(Slot, u32); 8] = [
    (Slot::MainHand, 1219), // Redridge Machete, sword 13-25, 2.3 s
    (Slot::OffHand, 6560),  // Soldier's Shield, 361 armor, block 6
    (Slot::Chest, 2977),    // Veteran Armor, 151
    (Slot::Legs, 6546),     // Soldier's Leggings, 139
    (Slot::Feet, 6551),     // Soldier's Boots, 109
    (Slot::Hands, 6547),    // Soldier's Gauntlets, 99
    (Slot::Wrists, 9785),   // Raider's Bracers, 69
    (Slot::Waist, 6548),    // Soldier's Girdle, 85
];

/// The kit a new character of this race and class starts in; empty for the ones the game cannot make.
pub fn start_kit(race: u8, class: u8) -> &'static [(Slot, u32)] {
    if race != RACE_HUMAN {
        return &[][..];
    }
    crate::class::def(class).map_or(&[][..], |c| c.kit)
}

/// The text of the worn items: `kit 1` once the starting kit was given, then one `equip <slot> <item>`
/// line per worn slot (read back by [`parse_line`]).
pub fn write_lines(gear: &Gear, kit_given: bool, out: &mut String) {
    use std::fmt::Write as _;
    if kit_given {
        out.push_str("kit 1\n");
    }
    for (slot, item) in gear.iter() {
        let _ = writeln!(out, "equip {} {item}", slot.key());
    }
}

/// Reads one save line of the worn items; `Ok(false)` when the line is not theirs.
pub fn parse_line(words: &[&str], gear: &mut Gear, kit_given: &mut bool) -> Result<bool> {
    match *words {
        ["kit", "1"] => {
            ensure!(!*kit_given, "duplicate kit line");
            *kit_given = true;
            Ok(true)
        }
        ["equip", key, item] => {
            let Some(slot) = Slot::from_key(key) else {
                bail!("unknown equipment slot {key}");
            };
            let item: u32 = item.parse()?;
            ensure!(item > 0, "zero equipped item");
            ensure!(gear.get(slot).is_none(), "duplicate equipment slot {key}");
            gear.set(slot, Some(item));
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn item(entry: u32, inventory_type: u32, class: u32, subclass: u32) -> Item {
        Item {
            entry,
            name: format!("Item {entry}"),
            class,
            subclass,
            display: 0,
            quality: 1,
            flags: 0,
            buy_price: 0,
            sell_price: 0,
            inventory_type,
            allowable_class: -1,
            allowable_race: -1,
            item_level: 1,
            required_level: 1,
            max_count: 1,
            stackable: 1,
            delay: 0,
            damage: (0.0, 0.0, 0),
            block: 0,
            armor: 0,
            stats: Vec::new(),
        }
    }

    const WARRIOR: Wearer = Wearer {
        race: RACE_HUMAN,
        class: CLASS_WARRIOR,
        level: 5,
    };

    #[test]
    fn there_are_nineteen_server_slots_ammo_and_the_belt_torch() {
        assert_eq!(Slot::ALL.len(), 21);
        for (i, slot) in Slot::ALL.into_iter().enumerate() {
            assert_eq!(slot.index(), i);
            assert_eq!(Slot::from_index(i), Some(slot));
            assert_eq!(Slot::from_key(slot.key()), Some(slot));
        }
        // The numbers the server uses (vMaNGOS `EquipmentSlots`).
        assert_eq!(Slot::Head.index(), 0);
        assert_eq!(Slot::Shirt.index(), 3);
        assert_eq!(Slot::Chest.index(), 4);
        assert_eq!(Slot::Back.index(), 14);
        assert_eq!(Slot::MainHand.index(), 15);
        assert_eq!(Slot::Ranged.index(), 17);
        assert_eq!(Slot::Tabard.index(), 18);
        assert_eq!(Slot::Ammo.index(), 19);
        assert_eq!(Slot::Torch.index(), 20);
        assert_eq!(Slot::from_index(21), None);
    }

    #[test]
    fn the_pane_lays_every_slot_out_once() {
        let mut seen: Vec<Slot> = LEFT_COLUMN.into_iter().chain(RIGHT_COLUMN).collect();
        seen.extend(WEAPON_ROW);
        seen.extend(BELT_ROW);
        seen.sort();
        assert_eq!(seen, Slot::ALL.to_vec());
    }

    #[test]
    fn every_wearable_type_has_a_slot_and_the_rest_have_none() {
        for t in 1..=26 {
            let wearable = t != inv::BAG;
            assert_eq!(!slots_for(t).is_empty(), wearable, "inventory type {t}");
        }
        assert!(slots_for(0).is_empty());
        assert!(slots_for(inv::QUIVER).is_empty());
        assert_eq!(slots_for(inv::ROBE), &[Slot::Chest]);
        assert_eq!(slots_for(inv::FINGER), &[Slot::Finger1, Slot::Finger2]);
        assert_eq!(slots_for(inv::SHIELD), &[Slot::OffHand]);
        assert_eq!(slots_for(inv::WEAPON), &[Slot::MainHand, Slot::OffHand]);
        assert_eq!(slots_for(inv::MAIN_HAND), &[Slot::MainHand]);
    }

    #[test]
    fn level_class_race_and_armor_type_decide_who_may_wear() {
        let mut chest = item(1, inv::CHEST, 4, armor::CLOTH);
        assert_eq!(can_wear(&chest, WARRIOR), Ok(()));
        chest.required_level = 8;
        assert_eq!(can_wear(&chest, WARRIOR), Err(Refusal::Level(8)));
        chest.required_level = 1;
        chest.allowable_class = 1 << 7; // mages only
        assert_eq!(can_wear(&chest, WARRIOR), Err(Refusal::Never));
        chest.allowable_class = 1 << 0;
        assert_eq!(can_wear(&chest, WARRIOR), Ok(()));
        chest.allowable_race = 1 << 1; // orcs only
        assert_eq!(can_wear(&chest, WARRIOR), Err(Refusal::Never));

        let plate = item(2, inv::CHEST, 4, armor::PLATE);
        assert_eq!(can_wear(&plate, WARRIOR), Err(Refusal::Proficiency));
        let at_forty = Wearer {
            level: 40,
            ..WARRIOR
        };
        assert_eq!(can_wear(&plate, at_forty), Ok(()));
        let mail = item(3, inv::CHEST, 4, armor::MAIL);
        assert_eq!(can_wear(&mail, WARRIOR), Ok(()));
        let shield = item(4, inv::SHIELD, 4, armor::SHIELD);
        assert_eq!(can_wear(&shield, WARRIOR), Ok(()));
        let wand = item(5, inv::RANGED_RIGHT, 2, 19);
        assert_eq!(can_wear(&wand, WARRIOR), Err(Refusal::Proficiency));
        assert_eq!(
            can_wear(&item(6, inv::BAG, 1, 0), WARRIOR),
            Err(Refusal::NotWearable)
        );
        assert_eq!(
            can_wear(&item(7, 0, 0, 0), WARRIOR),
            Err(Refusal::NotWearable)
        );
    }

    #[test]
    fn an_all_classes_mask_of_minus_one_or_zero_lets_everyone_in() {
        let mut ring = item(1, inv::FINGER, 4, armor::MISC);
        for mask in [-1, 0, 32767] {
            ring.allowable_class = mask;
            assert_eq!(can_wear(&ring, WARRIOR), Ok(()), "mask {mask}");
        }
    }

    #[test]
    fn a_quick_equip_fills_the_empty_slot_and_swaps_the_first_when_full() {
        let items = vec![
            item(10, inv::FINGER, 4, armor::MISC),
            item(11, inv::FINGER, 4, armor::MISC),
            item(12, inv::FINGER, 4, armor::MISC),
        ];
        let mut gear = Gear::default();
        assert_eq!(
            gear.best_slot(&items, &items[0], WARRIOR),
            Some(Slot::Finger1)
        );
        gear.set(Slot::Finger1, Some(10));
        assert_eq!(
            gear.best_slot(&items, &items[1], WARRIOR),
            Some(Slot::Finger2)
        );
        gear.set(Slot::Finger2, Some(11));
        assert_eq!(
            gear.best_slot(&items, &items[2], WARRIOR),
            Some(Slot::Finger1)
        );
    }

    #[test]
    fn a_one_hander_takes_the_off_hand_only_with_dual_wield() {
        let sword = item(20, inv::WEAPON, 2, 7);
        let items = vec![sword.clone()];
        let gear = Gear::default();
        assert_eq!(
            gear.plan(&items, &sword, Slot::OffHand, WARRIOR),
            Err(Refusal::DualWield)
        );
        let veteran = Wearer {
            level: WARRIOR_DUAL_WIELD_LEVEL,
            ..WARRIOR
        };
        assert_eq!(
            gear.plan(&items, &sword, Slot::OffHand, veteran),
            Ok(vec![])
        );
        // A main-hand-only weapon never goes in the off hand, a shield never in the main hand.
        let main_only = item(21, inv::MAIN_HAND, 2, 4);
        assert_eq!(
            gear.plan(&items, &main_only, Slot::OffHand, veteran),
            Err(Refusal::WrongSlot)
        );
        let shield = item(22, inv::SHIELD, 4, armor::SHIELD);
        assert_eq!(
            gear.plan(&items, &shield, Slot::MainHand, WARRIOR),
            Err(Refusal::WrongSlot)
        );
        // A quick equip of a one-hander picks the main hand when the off hand is not allowed.
        assert_eq!(
            gear.best_slot(&items, &sword, WARRIOR),
            Some(Slot::MainHand)
        );
    }

    #[test]
    fn a_two_hander_sends_the_shield_home_and_shuts_the_off_hand() {
        let staff = item(30, inv::TWO_HAND, 2, 10);
        let shield = item(31, inv::SHIELD, 4, armor::SHIELD);
        let sword = item(32, inv::MAIN_HAND, 2, 7);
        let items = vec![staff.clone(), shield.clone(), sword.clone()];
        let mut gear = Gear::default();
        gear.set(Slot::MainHand, Some(32));
        gear.set(Slot::OffHand, Some(31));
        // Both come off.
        assert_eq!(
            gear.plan(&items, &staff, Slot::MainHand, WARRIOR),
            Ok(vec![(Slot::MainHand, 32), (Slot::OffHand, 31)])
        );
        // With the staff in the main hand a shield will not go on.
        gear.set(Slot::MainHand, Some(30));
        gear.set(Slot::OffHand, None);
        assert_eq!(
            gear.plan(&items, &shield, Slot::OffHand, WARRIOR),
            Err(Refusal::TwoHanded)
        );
        // A one-hander in the main hand takes the staff off and leaves the off hand alone.
        assert_eq!(
            gear.plan(&items, &sword, Slot::MainHand, WARRIOR),
            Ok(vec![(Slot::MainHand, 30)])
        );
    }

    #[test]
    fn the_armor_and_the_block_of_what_is_worn_add_up() {
        let mut vest = item(40, inv::CHEST, 4, armor::LEATHER);
        vest.armor = 33;
        let mut buckler = item(41, inv::SHIELD, 4, armor::SHIELD);
        buckler.armor = 5;
        buckler.block = 1;
        let items = vec![vest, buckler];
        let mut gear = Gear::default();
        gear.set(Slot::Chest, Some(40));
        gear.set(Slot::OffHand, Some(41));
        gear.set(Slot::Head, Some(999)); // not in the table: counts for nothing
        assert_eq!(
            totals(&gear, &items),
            Totals {
                armor: 38,
                block: 1
            }
        );
        assert_eq!(totals(&Gear::default(), &items), Totals::default());
    }

    #[test]
    fn a_weapons_dps_is_the_mean_damage_over_the_swing_time() {
        let mut sword = item(50, inv::MAIN_HAND, 2, 7);
        sword.damage = (1.0, 3.0, 0);
        sword.delay = 1900;
        assert!((weapon_dps(&sword) - 2.0 / 1.9).abs() < 1e-5);
        sword.delay = 0;
        assert_eq!(weapon_dps(&sword), 0.0);
    }

    #[test]
    fn a_human_warrior_starts_in_the_vanilla_kit() {
        let kit = start_kit(RACE_HUMAN, CLASS_WARRIOR);
        assert_eq!(kit.len(), 5);
        assert!(kit.contains(&(Slot::MainHand, 25)));
        assert!(start_kit(2, CLASS_WARRIOR).is_empty());
        assert_eq!(
            start_kit(RACE_HUMAN, CLASS_MAGE).len(),
            5,
            "the mage has a kit"
        );
        assert!(
            start_kit(RACE_HUMAN, 5).is_empty(),
            "the priest has none yet"
        );
    }

    #[test]
    fn the_worn_items_round_trip_through_their_lines() {
        let mut gear = Gear::default();
        gear.set(Slot::Back, Some(1372));
        gear.set(Slot::MainHand, Some(25));
        let mut text = String::new();
        write_lines(&gear, true, &mut text);
        assert_eq!(text, "kit 1\nequip back 1372\nequip mainhand 25\n");
        let (mut back, mut kit) = (Gear::default(), false);
        for line in text.lines() {
            let words: Vec<_> = line.split_whitespace().collect();
            assert!(parse_line(&words, &mut back, &mut kit).unwrap());
        }
        assert_eq!((back, kit), (gear, true));
    }

    #[test]
    fn bad_equipment_lines_are_refused_and_other_lines_are_not_theirs() {
        let (mut gear, mut kit) = (Gear::default(), false);
        for line in [
            "equip belt 5",
            "equip head x",
            "equip head 0",
            "kit 2x",
            "equip head",
        ] {
            let words: Vec<_> = line.split_whitespace().collect();
            let r = parse_line(&words, &mut gear, &mut kit);
            assert!(r.is_err() || matches!(r, Ok(false)), "{line}");
        }
        let words = ["item", "1", "2"];
        assert!(!parse_line(&words, &mut gear, &mut kit).unwrap());
        let words = ["equip", "head", "5"];
        assert!(parse_line(&words, &mut gear, &mut kit).unwrap());
        assert!(parse_line(&words, &mut gear, &mut kit).is_err());
    }
}
