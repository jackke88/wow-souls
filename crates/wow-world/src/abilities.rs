//! The warrior's abilities for the valley's levels, and what the trainer asks for them. Pure data and
//! rules; the game's rage, hotbar and the trainer's screen draw them.
//!
//! The set is the one the user chose: **Charge, Battle Shout and Thunder Clap**, learned from Llane Beshere
//! (`npc_trainer_template` 22, build 5875), Charge first. Heroic Strike (it does nothing by itself, it only
//! makes the next blow harder) and Rend are not in the game. The vMaNGOS trainer asks level 1 for Battle
//! Shout, 4 for Charge and 6 for Thunder Clap; here Charge is the first thing a warrior learns (level 1) and
//! Thunder Clap asks 4, because level 6 is out of reach in the valley. The prices are the database's
//! (10 copper for the shout, 100 for the others), paid in souls ([`crate::shop::SOULS_PER_COPPER`]). Rage
//! costs are the spells' own (`manaCost` / 10 of a rage spell).
//!
//! `usable` says whether the game does something when the ability is used; an ability that is not usable
//! cannot be learned (it would cost souls and do nothing).

use std::collections::BTreeSet;

use crate::shop;

/// The highest rage a warrior holds.
pub const RAGE_MAX: u32 = 100;

/// What using the ability does; the game maps each to its effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Every blow hits harder for a while.
    BattleShout,
    /// A rush at the enemy that stuns it.
    Charge,
    /// Blasts what stands near.
    ThunderClap,
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
    pub rage: u32,
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

/// Every ability of the warrior's list, in the order they are learned.
pub const ALL: [Ability; 3] = [
    Ability {
        spell: 100,
        name: "Charge",
        rank: 1,
        level: 1,
        copper: 100,
        rage: 0,
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
        rage: 20,
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
        rage: 20,
        icon: "Spell_Nature_ThunderClap",
        effect: Effect::ThunderClap,
        usable: true,
        text: "Blasts every enemy within 5 yards with thunder: 30 damage, and they move and recover 40% slower for 10 seconds. 5 second cooldown.",
    },
];

/// The abilities a trainer teaches: the warrior trainer's.
pub const WARRIOR_TRAINER: u32 = 911;

pub fn find(spell: u32) -> Option<&'static Ability> {
    ALL.iter().find(|a| a.spell == spell)
}

/// What a trainer sells: the abilities with a price.
pub fn trainer_list(npc: u32) -> Vec<&'static Ability> {
    if npc == WARRIOR_TRAINER {
        ALL.iter().filter(|a| a.copper > 0).collect()
    } else {
        Vec::new()
    }
}

/// What the character knows: the starting abilities and what was learned.
pub fn knows(learned: &BTreeSet<u32>, ability: &Ability) -> bool {
    ability.copper == 0 || learned.contains(&ability.spell)
}

/// The known abilities in list order.
pub fn known(learned: &BTreeSet<u32>) -> Vec<&'static Ability> {
    ALL.iter().filter(|a| knows(learned, a)).collect()
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
            known(&none).is_empty(),
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
            ALL.iter().all(|a| a.usable),
            "every ability on the list works"
        );
        assert!(trainer_list(1).is_empty());
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
    fn the_rage_costs_are_the_spells_own() {
        let cost = |s: u32| find(s).unwrap().rage;
        assert_eq!((cost(100), cost(6673), cost(6343)), (0, 20, 20));
        assert!(ALL.iter().all(|a| a.rage <= RAGE_MAX));
        assert!(
            find(78).is_none() && find(772).is_none(),
            "Heroic Strike and Rend are not in the game"
        );
    }
}
