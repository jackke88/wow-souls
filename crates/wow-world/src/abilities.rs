//! The abilities of the classes the game plays, and what their trainers ask for them. Pure data and
//! rules; the game's rage and mana, hotbar and the trainer's screen draw them.
//!
//! **The Warrior's** set is the one the user chose: **Charge, Battle Shout and Thunder Clap**, learned from
//! Llane Beshere (`npc_trainer_template` 22, build 5875), Charge first. Heroic Strike (it does nothing by
//! itself, it only makes the next blow harder) and Rend are not in the game. The vMaNGOS trainer asks
//! level 1 for Battle Shout, 4 for Charge and 6 for Thunder Clap; here Charge is the first thing a
//! warrior learns (level 1) and Thunder Clap asks 4, because level 6 is out of reach in the valley. The
//! prices are the database's (10 copper for the shout, 100 for the others), paid in souls
//! ([`crate::shop::SOULS_PER_COPPER`]). Rage costs are the spells' own (`manaCost` / 10 of a rage spell).
//!
//! **The Mage's** set is the user's too (2026-10-14): **Fireball, Frost Armor, Frostbolt and Fire Blast**,
//! rank 1. Fireball and Frost Armor are known from the start (`playercreateinfo_spell`), Frostbolt (lvl 4)
//! and Fire Blast (lvl 6) are taught by Khelden Bremen (`npc_trainer_template` 5) for 100 copper each.
//! Mana costs are the spells' own `manaCost`.
//!
//! `usable` says whether the game does something when the ability is used; an ability that is not usable
//! cannot be learned (it would cost souls and do nothing). Which abilities are whose, and in what order
//! they are learned, is [`crate::class`].

use std::collections::BTreeSet;

use crate::class::Power;
use crate::shop;

/// The highest rage a warrior holds.
pub const RAGE_MAX: u32 = 100;

/// What using the ability does; the game maps each to its effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    MortalStrike,
    Bloodthirst,
    ShieldWall,
    /// Every blow hits harder for a while.
    BattleShout,
    /// A rush at the enemy that stuns it.
    Charge,
    /// Blasts what stands near.
    ThunderClap,
    /// A fire missile that burns.
    Fireball,
    /// A frost missile that slows.
    Frostbolt,
    /// An instant blast at a target in range.
    FireBlast,
    /// More armor for half an hour.
    FrostArmor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ability {
    /// The spell id of the ability (rank 1).
    pub spell: u32,
    pub name: &'static str,
    pub rank: u8,
    /// The level the trainer asks.
    pub level: u32,
    /// The trainer's price in copper (0: known from the start).
    pub copper: u32,
    /// What it is paid in, and how much.
    pub power: Power,
    pub cost: u32,
    /// The client's `SpellIcon` id and the icon it names (`Interface\Icons\<icon>`).
    pub icon: &'static str,
    pub effect: Effect,
    pub usable: bool,
    /// What the card says (WoW's own sense, in the game's numbers).
    pub text: &'static str,
}

impl Ability {
    /// The price in souls.
    pub fn souls(&self) -> u32 {
        shop::souls_for_copper(self.copper)
    }
}

/// Every ability of every class, a class's own in the order of its row ([`crate::class::ClassDef::spells`]).
pub const ALL: [Ability; 10] = [
    Ability { spell: 12294, name: "Mortal Strike", rank: 1, level: 35, copper: 0,
        power: Power::Rage, cost: 30, icon: "Ability_Warrior_SavageBlow", effect: Effect::MortalStrike,
        usable: true, text: "Commit to your weapon's heavy attack: +50% damage and +50% poise damage on contact. Costs the attack's stamina. 8 second cooldown. Requires the Arms talent." },
    Ability { spell: 23881, name: "Bloodthirst", rank: 1, level: 35, copper: 0,
        power: Power::Rage, cost: 30, icon: "Spell_Nature_BloodLust", effect: Effect::Bloodthirst,
        usable: true, text: "Strike with your weapon's light attack for +20% damage. A landed strike heals 5% maximum health, once per swing. Costs attack stamina. 10 second cooldown. Requires the Fury talent." },
    Ability { spell: 871, name: "Shield Wall", rank: 1, level: 35, copper: 0,
        power: Power::Rage, cost: 40, icon: "Ability_Warrior_ShieldWall", effect: Effect::ShieldWall,
        usable: true, text: "Raise a held shield: take 40% less damage for 8 seconds. 45 second cooldown. Requires a shield and the Protection talent." },
    Ability {
        spell: 100,
        name: "Charge",
        rank: 1,
        level: 1,
        copper: 100,
        power: Power::Rage,
        cost: 0,
        icon: "Ability_Warrior_Charge",
        effect: Effect::Charge,
        usable: true,
        text: "Charge an enemy 5 to 25 yards away: you rush to it, gain 9 rage and it is stunned for 1.5 seconds. 12 second cooldown.",
    },
    Ability {
        spell: 6673,
        name: "Battle Shout",
        rank: 1,
        level: 1,
        copper: 10,
        power: Power::Rage,
        cost: 20,
        icon: "Ability_Warrior_BattleShout",
        effect: Effect::BattleShout,
        usable: true,
        text: "You shout: your blows do 15% more damage for 2 minutes.",
    },
    Ability {
        spell: 6343,
        name: "Thunder Clap",
        rank: 1,
        level: 4,
        copper: 100,
        power: Power::Rage,
        cost: 20,
        icon: "Spell_Nature_ThunderClap",
        effect: Effect::ThunderClap,
        usable: true,
        text: "Blasts every enemy within 5 yards with thunder: 30 damage, and they move and recover 40% slower for 10 seconds. 5 second cooldown.",
    },
    Ability {
        spell: 133,
        name: "Fireball",
        rank: 1,
        level: 1,
        copper: 0,
        power: Power::Mana,
        cost: 30,
        icon: "Spell_Fire_FlameBolt",
        effect: Effect::Fireball,
        usable: true,
        text: "Hurls a fiery ball that causes 14 to 22 fire damage and an additional 2 fire damage over 4 seconds.",
    },
    Ability {
        spell: 168,
        name: "Frost Armor",
        rank: 1,
        level: 1,
        copper: 0,
        power: Power::Mana,
        cost: 60,
        icon: "Spell_Frost_FrostArmor02",
        effect: Effect::FrostArmor,
        usable: true,
        text: "Covers you in a layer of frost: your armor is increased by 30 for 30 minutes.",
    },
    Ability {
        spell: 116,
        name: "Frostbolt",
        rank: 1,
        level: 4,
        copper: 100,
        power: Power::Mana,
        cost: 25,
        icon: "Spell_Frost_FrostBolt02",
        effect: Effect::Frostbolt,
        usable: true,
        text: "Launches a bolt of frost that causes 18 to 20 frost damage and slows the enemy's movement by 40% for 5 seconds.",
    },
    Ability {
        spell: 2136,
        name: "Fire Blast",
        rank: 1,
        level: 6,
        copper: 100,
        power: Power::Mana,
        cost: 40,
        icon: "Spell_Fire_Fireball",
        effect: Effect::FireBlast,
        usable: true,
        text: "Blasts the enemy within 20 yards for 24 to 32 fire damage. 8 second cooldown.",
    },
];

/// The warrior's trainer: Llane Beshere (the same number as [`crate::class::WARRIOR`]'s).
pub const WARRIOR_TRAINER: u32 = 911;

pub fn find(spell: u32) -> Option<&'static Ability> {
    ALL.iter().find(|a| a.spell == spell)
}

/// What a trainer sells: the abilities of the class it teaches that have a price.
pub fn trainer_list(npc: u32) -> Vec<&'static Ability> {
    let Some(class) = crate::class::trainer_of(npc) else {
        return Vec::new();
    };
    class
        .spells
        .iter()
        .filter_map(|&s| find(s))
        .filter(|a| a.copper > 0)
        .collect()
}

/// What the character knows: the starting abilities and what was learned.
pub fn knows(learned: &BTreeSet<u32>, ability: &Ability) -> bool {
    !matches!(
        ability.effect,
        Effect::MortalStrike | Effect::Bloodthirst | Effect::ShieldWall
    ) && (ability.copper == 0 || learned.contains(&ability.spell))
}

/// The abilities a character of `class` knows, in the class's learning order.
pub fn known(learned: &BTreeSet<u32>, class: u8) -> Vec<&'static Ability> {
    let Some(class) = crate::class::def(class) else {
        return Vec::new();
    };
    class
        .spells
        .iter()
        .filter_map(|&s| find(s))
        .filter(|a| knows(learned, a))
        .collect()
}

/// Why the trainer will not teach it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Known,
    Level(u32),
    NotEnoughSouls {
        need: u32,
        have: u32,
    },
    /// The game does not do anything with it yet.
    NotYet,
}

impl Refusal {
    pub fn message(self) -> String {
        match self {
            Refusal::Known => "You already know that.".into(),
            Refusal::Level(n) => format!("You must reach level {n} to learn that."),
            Refusal::NotEnoughSouls { need, have } => {
                format!("Not enough souls: it costs {need}, you carry {have}.")
            }
            Refusal::NotYet => "The trainer cannot teach that yet.".into(),
        }
    }
}

/// The trainer's verdict on teaching `ability` to a character of `level` carrying `souls`.
pub fn can_learn(
    learned: &BTreeSet<u32>,
    ability: &Ability,
    level: u32,
    souls: u32,
) -> Result<(), Refusal> {
    if matches!(
        ability.effect,
        Effect::MortalStrike | Effect::Bloodthirst | Effect::ShieldWall
    ) {
        return Err(Refusal::NotYet);
    }
    if knows(learned, ability) {
        return Err(Refusal::Known);
    }
    if !ability.usable {
        return Err(Refusal::NotYet);
    }
    if level < ability.level {
        return Err(Refusal::Level(ability.level));
    }
    let need = ability.souls();
    if souls < need {
        return Err(Refusal::NotEnoughSouls { need, have: souls });
    }
    Ok(())
}

/// Teaches it: takes the souls and adds the ability; returns the price. Nothing changes on a refusal.
pub fn learn(
    learned: &mut BTreeSet<u32>,
    souls: &mut u32,
    ability: &Ability,
    level: u32,
) -> Result<u32, Refusal> {
    can_learn(learned, ability, level, *souls)?;
    let cost = ability.souls();
    *souls -= cost;
    learned.insert(ability.spell);
    Ok(cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warrior_signatures_are_talent_rewards_and_cannot_be_bought_or_known_for_free() {
        for spell in [12294, 23881, 871] {
            let ability = find(spell).unwrap();
            assert!(!knows(&BTreeSet::new(), ability));
            assert_eq!(
                can_learn(&BTreeSet::new(), ability, 60, 100_000),
                Err(Refusal::NotYet)
            );
            assert!(!trainer_list(WARRIOR_TRAINER)
                .iter()
                .any(|a| a.spell == spell));
            assert!(!known(&BTreeSet::new(), 1).iter().any(|a| a.spell == spell));
        }
    }

    fn shout() -> &'static Ability {
        find(6673).unwrap()
    }
    fn charge() -> &'static Ability {
        find(100).unwrap()
    }

    #[test]
    fn nothing_is_known_at_the_start_and_the_trainer_sells_charge_first() {
        let none = BTreeSet::new();
        assert!(
            known(&none, 1).is_empty(),
            "no Heroic Strike, nothing for free"
        );
        assert!(!knows(&none, charge()));
        let list = trainer_list(WARRIOR_TRAINER);
        assert_eq!(
            list.iter().map(|a| a.name).collect::<Vec<_>>(),
            vec!["Charge", "Battle Shout", "Thunder Clap"],
            "the set the user chose, in the order it is learned"
        );
        assert_eq!(
            list.iter().map(|a| a.souls()).collect::<Vec<_>>(),
            vec![400, 40, 400]
        );
        assert!(
            ALL.iter()
                .filter(|a| a.power == Power::Rage)
                .all(|a| a.usable),
            "every warrior ability works"
        );
        assert!(trainer_list(1).is_empty());
    }

    #[test]
    fn the_mage_knows_fireball_and_frost_armor_and_learns_the_other_two() {
        let none = BTreeSet::new();
        assert_eq!(
            known(&none, 8).iter().map(|a| a.name).collect::<Vec<_>>(),
            vec!["Fireball", "Frost Armor"],
            "known from the start, in the class's order"
        );
        let list = trainer_list(198);
        assert_eq!(
            list.iter().map(|a| a.name).collect::<Vec<_>>(),
            vec!["Frostbolt", "Fire Blast"]
        );
        assert_eq!(
            list.iter()
                .map(|a| (a.level, a.souls()))
                .collect::<Vec<_>>(),
            vec![(4, 400), (6, 400)]
        );
        // All four do something in the game (2026-10-14): Fireball, Frostbolt, Fire Blast, Frost Armor.
        assert!(ALL
            .iter()
            .filter(|a| a.power == Power::Mana)
            .all(|a| a.usable));
        // A class never lists another's abilities.
        assert!(known(&none, 1).iter().all(|a| a.power == Power::Rage));
        assert!(known(&none, 8).iter().all(|a| a.power == Power::Mana));
    }

    #[test]
    fn charge_is_the_first_thing_a_new_warrior_can_learn() {
        let learned = BTreeSet::new();
        assert_eq!(can_learn(&learned, charge(), 1, 400), Ok(()));
        assert_eq!(
            can_learn(&learned, charge(), 1, 399),
            Err(Refusal::NotEnoughSouls {
                need: 400,
                have: 399
            })
        );
        // Thunder Clap waits for level 4, which the valley can reach.
        let clap = find(6343).unwrap();
        assert_eq!(can_learn(&learned, clap, 3, 999), Err(Refusal::Level(4)));
        assert_eq!(can_learn(&learned, clap, 4, 999), Ok(()));
    }

    #[test]
    fn learning_needs_the_level_the_souls_and_a_usable_ability() {
        let mut learned = BTreeSet::new();
        let mut souls = 30;
        assert_eq!(
            learn(&mut learned, &mut souls, shout(), 1),
            Err(Refusal::NotEnoughSouls { need: 40, have: 30 })
        );
        souls = 100;
        assert_eq!(learn(&mut learned, &mut souls, shout(), 1), Ok(40));
        assert_eq!((souls, learned.contains(&6673)), (60, true));
        assert_eq!(
            learn(&mut learned, &mut souls, shout(), 1),
            Err(Refusal::Known)
        );
        // An ability the game cannot do is refused and costs nothing.
        let mut broken = *charge();
        broken.usable = false;
        assert_eq!(
            learn(&mut learned, &mut souls, &broken, 9),
            Err(Refusal::NotYet)
        );
        assert_eq!((souls, learned.len()), (60, 1), "a refusal changes nothing");
    }

    #[test]
    fn the_level_gate_speaks_in_wows_words() {
        let mut ability = *shout();
        ability.level = 4;
        let learned = BTreeSet::new();
        assert_eq!(
            can_learn(&learned, &ability, 3, 999),
            Err(Refusal::Level(4))
        );
        assert_eq!(
            Refusal::Level(4).message(),
            "You must reach level 4 to learn that."
        );
        assert_eq!(can_learn(&learned, &ability, 4, 999), Ok(()));
    }

    #[test]
    fn the_costs_are_the_spells_own() {
        let cost = |s: u32| find(s).unwrap().cost;
        assert_eq!((cost(100), cost(6673), cost(6343)), (0, 20, 20));
        assert_eq!(
            (cost(133), cost(168), cost(116), cost(2136)),
            (30, 60, 25, 40),
            "Fireball, Frost Armor, Frostbolt, Fire Blast mana"
        );
        assert!(ALL
            .iter()
            .filter(|a| a.power == Power::Rage)
            .all(|a| a.cost <= RAGE_MAX));
        assert!(
            find(78).is_none() && find(772).is_none(),
            "Heroic Strike and Rend are not in the game"
        );
    }
}
