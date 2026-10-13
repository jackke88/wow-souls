//! The quest engine, pure and tested: which quests a player can see and take (`rules`), what accepting,
//! killing, looting, completing and turning in does to the player's log (`log`), the marks over NPCs, the
//! quest texts with their placeholders filled (`text`), the XP table and levels (`xp`) and a plain-text
//! save (`save`). The rules are vMaNGOS's for the 1.12 data; the report behind them is
//! `docs/superpowers/specs/2026-10-09-quest-semantics.md` (its section numbers are quoted in comments).
//!
//! The engine never touches the world: every method returns events (`log::Event`) and every effect (items,
//! money, XP, toasts) is the caller's job. Item ownership is the caller's too (`wow_world::inventory`):
//! the log takes the inventory as a count closure `&dyn Fn(u32) -> u32`.

pub mod book;
pub mod log;
#[cfg(test)]
mod playthrough;
pub mod rules;
pub mod save;
pub mod text;
pub mod xp;

pub use book::QuestBook;
pub use log::{Entry, Event, Need, QuestLog, Rewards, Status, TurnInError};
pub use rules::{can_take, marks, why_not, Mark, Refusal};

/// The playable classes, with their ids in the 1.12 client (the bit of a class mask is `1 << (id - 1)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Warrior = 1,
    Paladin = 2,
    Hunter = 3,
    Rogue = 4,
    Priest = 5,
    Shaman = 7,
    Mage = 8,
    Warlock = 9,
    Druid = 11,
}

/// The playable races, with their ids in the 1.12 client (the bit of a race mask is `1 << (id - 1)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Race {
    Human = 1,
    Orc = 2,
    Dwarf = 3,
    NightElf = 4,
    Undead = 5,
    Tauren = 6,
    Gnome = 7,
    Troll = 8,
}

impl Class {
    pub fn mask(self) -> u32 {
        1 << (self as u32 - 1)
    }

    /// The class word of the quest texts (`$C`; `$c` is the same in lower case).
    pub fn name(self) -> &'static str {
        match self {
            Class::Warrior => "Warrior",
            Class::Paladin => "Paladin",
            Class::Hunter => "Hunter",
            Class::Rogue => "Rogue",
            Class::Priest => "Priest",
            Class::Shaman => "Shaman",
            Class::Mage => "Mage",
            Class::Warlock => "Warlock",
            Class::Druid => "Druid",
        }
    }
}

impl Race {
    pub fn mask(self) -> u32 {
        1 << (self as u32 - 1)
    }

    /// The race word of the quest texts (`$R`; `$r` is the same in lower case).
    pub fn name(self) -> &'static str {
        match self {
            Race::Human => "Human",
            Race::Orc => "Orc",
            Race::Dwarf => "Dwarf",
            Race::NightElf => "Night Elf",
            Race::Undead => "Undead",
            Race::Tauren => "Tauren",
            Race::Gnome => "Gnome",
            Race::Troll => "Troll",
        }
    }
}

/// The player as the quest rules see them.
#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub name: String,
    pub male: bool,
    pub class: Class,
    pub race: Race,
    pub level: u32,
}

impl Player {
    /// A Human male Warrior: the character the game has (the name is the player's, `Adventurer` by default).
    pub fn warrior(name: &str, level: u32) -> Player {
        Player {
            name: name.to_string(),
            male: true,
            class: Class::Warrior,
            race: Race::Human,
            level,
        }
    }
}

/// A quest giver or ender: a creature (by entry) or a game object (by entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Npc {
    Creature(u32),
    Object(u32),
}

impl Npc {
    pub(crate) fn new(entry: u32, object: bool) -> Npc {
        if object {
            Npc::Object(entry)
        } else {
            Npc::Creature(entry)
        }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! Fixtures for the tests: a plain quest, and the real manifest for the install tests (skipped, with a
    //! note, when there is none).
    use crate::model::{Quest, World};

    /// A quest with nothing in it: level 1, no gates, no objectives, no rewards.
    pub fn quest(id: u32) -> Quest {
        Quest {
            id,
            patch: 0,
            title: format!("Quest {id}"),
            details: String::new(),
            objectives: String::new(),
            offer_reward: String::new(),
            request_items: String::new(),
            end_text: String::new(),
            objective_text: Default::default(),
            zone_or_sort: 9,
            min_level: 1,
            level: 1,
            required_classes: 0,
            required_races: 0,
            prev: 0,
            next: 0,
            exclusive_group: 0,
            next_in_chain: 0,
            flags: 0,
            special_flags: 0,
            limit_time: 0,
            src_item: 0,
            src_item_count: 0,
            req_items: vec![],
            req_kills: vec![],
            req_spell_cast: vec![],
            reward_choice: vec![],
            reward_items: vec![],
            reward_money: 0,
            reward_xp: 0,
            reward_spell: 0,
            point: (0, 0.0, 0.0),
            start_script: 0,
            complete_script: 0,
            event: false,
            max_level: 0,
            required_condition: 0,
            rep_objective: (0, 0),
            rep_min: (0, 0),
            rep_max: (0, 0),
            reward_rep: vec![],
            reward_mail: (0, 0, 0),
        }
    }

    pub fn real_world() -> Option<World> {
        let path = crate::default_manifest_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("skipped: no manifest at {}", path.display());
            return None;
        };
        Some(crate::manifest::parse(&text).unwrap())
    }
}
