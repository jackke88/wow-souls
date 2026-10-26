//! A class is a data row (spec `docs/superpowers/specs/2026-10-14-mage-class-design.md`, section 1):
//! what it spends to use its abilities, which abilities it has and in what order they are learned, the
//! trainer that teaches them and what a new human of the class wears. The abilities themselves are in
//! [`crate::abilities`]; the rest of the game asks this table which of them are a class's.

use crate::equipment::{Slot, CLASS_MAGE, CLASS_WARRIOR, HUMAN_MAGE_KIT, HUMAN_WARRIOR_KIT};

/// What a class spends to use an ability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    /// Builds from blows dealt and taken, fades away out of the fight.
    Rage,
    /// A pool from the class tables and Intellect that trickles back with Spirit.
    Mana,
}

impl Power {
    /// The word the refusals and the cards use.
    pub fn word(self) -> &'static str {
        match self {
            Power::Rage => "rage",
            Power::Mana => "mana",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ClassDef {
    /// The `ChrClasses` id.
    pub id: u8,
    pub name: &'static str,
    pub power: Power,
    /// The class trainer's creature entry.
    pub trainer: u32,
    /// The spell ids of the class's abilities, in the order they are learned.
    pub spells: &'static [u32],
    /// What a new human of the class wears.
    pub kit: &'static [(Slot, u32)],
}

pub const WARRIOR: ClassDef = ClassDef {
    id: CLASS_WARRIOR,
    name: "Warrior",
    power: Power::Rage,
    // Llane Beshere.
    trainer: 911,
    // Charge, Battle Shout, Thunder Clap.
    spells: &[100, 6673, 6343],
    kit: &HUMAN_WARRIOR_KIT,
};

pub const MAGE: ClassDef = ClassDef {
    id: CLASS_MAGE,
    name: "Mage",
    power: Power::Mana,
    // Khelden Bremen, `creature_template` 198, `trainer_id` 5.
    trainer: 198,
    // Fireball, Frost Armor (known from the start), Frostbolt (lvl 4), Fire Blast (lvl 6).
    spells: &[133, 168, 116, 2136],
    kit: &HUMAN_MAGE_KIT,
};

/// The classes the game can play.
pub static CLASSES: [ClassDef; 2] = [WARRIOR, MAGE];

/// The row of a class id; `None` for a class the game cannot play yet.
pub fn def(class: u8) -> Option<&'static ClassDef> {
    CLASSES.iter().find(|c| c.id == class)
}

/// The class a trainer (a creature entry) teaches.
pub fn trainer_of(npc: u32) -> Option<&'static ClassDef> {
    CLASSES.iter().find(|c| c.trainer == npc)
}

/// The class an ability (a spell id) belongs to.
pub fn of_spell(spell: u32) -> Option<&'static ClassDef> {
    CLASSES.iter().find(|c| c.spells.contains(&spell))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::Slot;

    #[test]
    fn warrior_is_row_one_and_mage_row_two() {
        let w = def(1).unwrap();
        assert_eq!((w.name, w.power, w.trainer), ("Warrior", Power::Rage, 911));
        assert_eq!(
            w.spells,
            &[100, 6673, 6343],
            "Charge, Battle Shout, Thunder Clap"
        );
        assert_eq!(w.kit.len(), 5);
        let m = def(8).unwrap();
        assert_eq!((m.name, m.power, m.trainer), ("Mage", Power::Mana, 198));
        assert_eq!(
            m.spells,
            &[133, 168, 116, 2136],
            "Fireball, Frost Armor, Frostbolt, Fire Blast"
        );
        assert_eq!(m.kit.len(), 5);
        assert!(
            def(2).is_none() && def(0).is_none(),
            "paladins are not playable yet"
        );
    }

    #[test]
    fn a_trainer_and_a_spell_know_their_class() {
        assert_eq!(trainer_of(911).map(|c| c.id), Some(1));
        assert_eq!(trainer_of(198).map(|c| c.id), Some(8));
        assert!(trainer_of(1).is_none());
        assert_eq!(of_spell(6673).map(|c| c.id), Some(1));
        assert_eq!(of_spell(116).map(|c| c.id), Some(8));
        assert!(of_spell(78).is_none(), "no Heroic Strike");
    }

    #[test]
    fn the_kits_are_the_vanilla_ones() {
        assert!(MAGE.kit.contains(&(Slot::MainHand, 35)), "Bent Staff");
        assert!(MAGE.kit.contains(&(Slot::Chest, 56)), "Apprentice's Robe");
        assert!(MAGE.kit.contains(&(Slot::Legs, 1395)), "Apprentice's Pants");
        assert!(MAGE.kit.contains(&(Slot::Feet, 55)), "Apprentice's Boots");
        assert!(
            MAGE.kit.contains(&(Slot::Shirt, 6096)),
            "Apprentice's Shirt"
        );
        assert!(
            WARRIOR.kit.contains(&(Slot::MainHand, 25)),
            "Worn Shortsword"
        );
    }
}
