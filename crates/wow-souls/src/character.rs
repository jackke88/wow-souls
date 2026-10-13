//! The player's character: race, class, sex and name, chosen in the creation menu ([`crate::menu`]) and
//! saved in `%LOCALAPPDATA%\WoWSouls\character-v1.txt` (`wschar 1`). Only a Human Warrior (male) can be
//! made for now: everything else is listed and greyed, so the menu shows where the game is going.

use std::path::PathBuf;

use bevy::prelude::*;

/// A name is this many letters at most, and at least two (WoW's rule is two to twelve).
pub const NAME_MIN: usize = 2;
pub const NAME_MAX: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Race {
    Human,
    Dwarf,
    Gnome,
    NightElf,
    Orc,
    Troll,
    Undead,
    Tauren,
}

impl Race {
    pub const ALL: [Race; 8] = [
        Race::Human,
        Race::Dwarf,
        Race::Gnome,
        Race::NightElf,
        Race::Orc,
        Race::Troll,
        Race::Undead,
        Race::Tauren,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Race::Human => "Human",
            Race::Dwarf => "Dwarf",
            Race::Gnome => "Gnome",
            Race::NightElf => "Night Elf",
            Race::Orc => "Orc",
            Race::Troll => "Troll",
            Race::Undead => "Undead",
            Race::Tauren => "Tauren",
        }
    }

    /// Only the human can be made yet.
    pub fn enabled(self) -> bool {
        self == Race::Human
    }

    /// The `ChrRaces` id the item rules and the databases use.
    pub fn id(self) -> u8 {
        match self {
            Race::Human => 1,
            Race::Orc => 2,
            Race::Dwarf => 3,
            Race::NightElf => 4,
            Race::Undead => 5,
            Race::Tauren => 6,
            Race::Gnome => 7,
            Race::Troll => 8,
        }
    }

    /// The race with this `ChrRaces` id.
    pub fn from_id(id: u8) -> Option<Race> {
        Race::ALL.into_iter().find(|r| r.id() == id)
    }

    fn key(self) -> &'static str {
        match self {
            Race::NightElf => "nightelf",
            other => other.name(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Warrior,
    Paladin,
    Hunter,
    Rogue,
    Priest,
    Mage,
    Warlock,
    Druid,
    Shaman,
}

impl Class {
    pub const ALL: [Class; 9] = [
        Class::Warrior,
        Class::Paladin,
        Class::Hunter,
        Class::Rogue,
        Class::Priest,
        Class::Mage,
        Class::Warlock,
        Class::Druid,
        Class::Shaman,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Class::Warrior => "Warrior",
            Class::Paladin => "Paladin",
            Class::Hunter => "Hunter",
            Class::Rogue => "Rogue",
            Class::Priest => "Priest",
            Class::Mage => "Mage",
            Class::Warlock => "Warlock",
            Class::Druid => "Druid",
            Class::Shaman => "Shaman",
        }
    }

    /// Only the warrior can be made yet.
    pub fn enabled(self) -> bool {
        self == Class::Warrior
    }

    /// The `ChrClasses` id the item rules and the databases use.
    pub fn id(self) -> u8 {
        match self {
            Class::Warrior => 1,
            Class::Paladin => 2,
            Class::Hunter => 3,
            Class::Rogue => 4,
            Class::Priest => 5,
            Class::Shaman => 7,
            Class::Mage => 8,
            Class::Warlock => 9,
            Class::Druid => 11,
        }
    }

    /// The class with this `ChrClasses` id.
    pub fn from_id(id: u8) -> Option<Class> {
        Class::ALL.into_iter().find(|c| c.id() == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sex {
    Male,
    Female,
}

impl Sex {
    pub const ALL: [Sex; 2] = [Sex::Male, Sex::Female];

    pub fn name(self) -> &'static str {
        match self {
            Sex::Male => "Male",
            Sex::Female => "Female",
        }
    }

    /// Only the male body is built yet.
    pub fn enabled(self) -> bool {
        self == Sex::Male
    }

    /// The sex of the client's index (0 male, 1 female).
    pub fn from_index(i: u8) -> Option<Sex> {
        Sex::ALL.get(usize::from(i)).copied()
    }
}

/// The five customization dials of WoW's creation screen, as indices into the (race, sex)'s ranges:
/// skin colour, face, hair style, hair colour, facial hair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dials {
    pub skin: u8,
    pub face: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub facial_hair: u8,
}

impl Dials {
    /// What the player's body wore before the dials were saved (the avatar's old default look), so a
    /// character saved without them looks as it always did.
    pub const DEFAULT: Dials = Dials {
        skin: 2,
        face: 1,
        hair_style: 3,
        hair_color: 1,
        facial_hair: 1,
    };

    /// The dials from the screen's array, `[skin, face, hair style, hair colour, facial hair]`.
    pub fn from_array(a: [u8; 5]) -> Dials {
        Dials {
            skin: a[0],
            face: a[1],
            hair_style: a[2],
            hair_color: a[3],
            facial_hair: a[4],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Character {
    pub name: String,
    pub race: Race,
    pub class: Class,
    pub sex: Sex,
    pub dials: Dials,
}

impl Character {
    /// The only character the game can make today.
    #[cfg(test)]
    pub fn human_warrior(name: &str) -> Character {
        Character {
            name: name.to_string(),
            race: Race::Human,
            class: Class::Warrior,
            sex: Sex::Male,
            dials: Dials::DEFAULT,
        }
    }

    /// Whether the game can play this character yet.
    pub fn playable(&self) -> bool {
        self.race.enabled() && self.class.enabled() && self.sex.enabled()
    }

    pub fn to_text(&self) -> String {
        format!(
            "wschar 2\nname {}\nrace {}\nclass {}\nsex {}\nskin {}\nface {}\nhair_style {}\n\
             hair_color {}\nfacial_hair {}\n",
            self.name,
            self.race.key().to_ascii_lowercase(),
            self.class.name().to_ascii_lowercase(),
            self.sex.name().to_ascii_lowercase(),
            self.dials.skin,
            self.dials.face,
            self.dials.hair_style,
            self.dials.hair_color,
            self.dials.facial_hair,
        )
    }

    /// Reads [`Character::to_text`] (`wschar 2`) or the older `wschar 1`, which has no dials and takes
    /// [`Dials::DEFAULT`]; anything else is an error naming the problem.
    pub fn from_text(text: &str) -> Result<Character, String> {
        let mut lines = text.lines();
        let version = match lines.next() {
            Some("wschar 1") => 1,
            Some("wschar 2") => 2,
            _ => return Err("not a wschar 1 or 2 file".into()),
        };
        let (mut name, mut race, mut class, mut sex) = (None, None, None, None);
        let mut dials: [Option<u8>; 5] = [None; 5];
        for (n, line) in lines.enumerate() {
            let (key, value) = line
                .split_once(' ')
                .ok_or_else(|| format!("line {}: `key value` was expected", n + 2))?;
            let dial = |value: &str| {
                value
                    .trim()
                    .parse::<u8>()
                    .map_err(|_| format!("line {}: {key} wants a number 0..255", n + 2))
            };
            match key {
                "name" => name = Some(check_name(value)?),
                "race" => {
                    race = Race::ALL
                        .into_iter()
                        .find(|r| r.key().eq_ignore_ascii_case(value))
                }
                "class" => {
                    class = Class::ALL
                        .into_iter()
                        .find(|c| c.name().eq_ignore_ascii_case(value))
                }
                "sex" => {
                    sex = Sex::ALL
                        .into_iter()
                        .find(|s| s.name().eq_ignore_ascii_case(value))
                }
                "skin" | "face" | "hair_style" | "hair_color" | "facial_hair" if version == 2 => {
                    let slot = match key {
                        "skin" => 0,
                        "face" => 1,
                        "hair_style" => 2,
                        "hair_color" => 3,
                        _ => 4,
                    };
                    dials[slot] = Some(dial(value)?);
                }
                other => return Err(format!("line {}: unknown key {other:?}", n + 2)),
            }
        }
        let dials = if version == 1 {
            Dials::DEFAULT
        } else {
            let missing = ["skin", "face", "hair_style", "hair_color", "facial_hair"]
                .into_iter()
                .zip(dials)
                .find(|(_, d)| d.is_none());
            if let Some((key, _)) = missing {
                return Err(format!("no {key}"));
            }
            Dials::from_array(dials.map(|d| d.unwrap_or_default()))
        };
        Ok(Character {
            name: name.ok_or("no name")?,
            race: race.ok_or("no race")?,
            class: class.ok_or("no class")?,
            sex: sex.ok_or("no sex")?,
            dials,
        })
    }
}

/// A name as WoW takes it: two to twelve letters, written with a capital first and the rest small.
/// Returns the normalised name or what is wrong with it.
pub fn check_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    let count = name.chars().count();
    if count < NAME_MIN {
        return Err(format!("A name needs at least {NAME_MIN} letters."));
    }
    if count > NAME_MAX {
        return Err(format!("A name has at most {NAME_MAX} letters."));
    }
    if !name.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("A name has letters only.".into());
    }
    let mut chars = name.chars();
    let first = chars.next().map(|c| c.to_ascii_uppercase());
    Ok(first
        .into_iter()
        .chain(chars.map(|c| c.to_ascii_lowercase()))
        .collect())
}

/// Where the character is saved.
pub fn save_path() -> PathBuf {
    std::env::var("WOWSOULS_CHARACTER")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "saves".into()))
                .join("WoWSouls/character-v1.txt")
        })
}

/// The saved character: `Ok(None)` when there is none yet, an error when the file cannot be read (the
/// menu then refuses to write over it).
pub fn load(path: &std::path::Path) -> Result<Option<Character>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Character::from_text(&text).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

pub fn save(path: &std::path::Path, c: &Character) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, c.to_text())?;
    std::fs::rename(&tmp, path)
}

/// The character the player plays this session: the saved or created one, or the default of an
/// unattended run.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct Chosen(pub Option<Character>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_human_male_warrior_can_be_made_yet() {
        let c = Character::human_warrior("Jacob");
        assert!(c.playable());
        for race in Race::ALL.into_iter().filter(|r| *r != Race::Human) {
            assert!(!race.enabled(), "{race:?}");
        }
        for class in Class::ALL.into_iter().filter(|c| *c != Class::Warrior) {
            assert!(!class.enabled(), "{class:?}");
        }
        assert!(!Sex::Female.enabled());
        let elf = Character {
            race: Race::NightElf,
            ..c
        };
        assert!(!elf.playable());
    }

    #[test]
    fn a_name_is_two_to_twelve_letters_with_a_capital_first() {
        assert_eq!(check_name("jacob"), Ok("Jacob".to_string()));
        assert_eq!(check_name("  gARRICK "), Ok("Garrick".to_string()));
        assert!(check_name("J").is_err());
        assert!(check_name("Abcdefghijklm").is_err(), "thirteen letters");
        assert!(check_name("Abcdefghijkl").is_ok(), "twelve");
        for bad in ["Jac0b", "Ja cob", "Jacob!", "Jäger", ""] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_save_text_round_trips_every_race_and_class() {
        for race in Race::ALL {
            for class in Class::ALL {
                for sex in Sex::ALL {
                    let c = Character {
                        name: "Test".into(),
                        race,
                        class,
                        sex,
                        dials: Dials {
                            skin: 4,
                            face: 0,
                            hair_style: 9,
                            hair_color: 7,
                            facial_hair: 2,
                        },
                    };
                    assert_eq!(Character::from_text(&c.to_text()), Ok(c));
                }
            }
        }
    }

    #[test]
    fn a_broken_save_is_an_error() {
        let v2 = "name Jacob\nrace human\nclass warrior\nsex male\n";
        let dials = "skin 1\nface 1\nhair_style 1\nhair_color 1\nfacial_hair 1\n";
        for bad in [
            String::new(),
            "wschar 3\n".to_string(),
            "wschar 2\n".to_string(),
            "wschar 1\nname Jacob\n".to_string(),
            "wschar 1\nname J\nrace human\nclass warrior\nsex male\n".to_string(),
            "wschar 1\nname Jacob\nrace elf\nclass warrior\nsex male\n".to_string(),
            "wschar 1\nname Jacob\nrace human\nclass warrior\nsex male\nbonfire x\n".to_string(),
            "wschar 1\nnamejacob\n".to_string(),
            // A v2 file needs every dial, each a number that fits a byte; v1 knows no dials.
            format!("wschar 2\n{v2}"),
            format!("wschar 2\n{v2}skin 1\nface 1\nhair_style 1\nhair_color 1\n"),
            format!("wschar 2\n{v2}skin 300\nface 1\nhair_style 1\nhair_color 1\nfacial_hair 1\n"),
            format!("wschar 2\n{v2}skin x\nface 1\nhair_style 1\nhair_color 1\nfacial_hair 1\n"),
            format!("wschar 1\n{v2}{dials}"),
        ] {
            assert!(Character::from_text(&bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn an_older_wschar_1_save_takes_the_default_look() {
        let c = Character::from_text("wschar 1\nname Jacob\nrace human\nclass warrior\nsex male\n")
            .unwrap();
        assert_eq!(c, Character::human_warrior("Jacob"));
        assert_eq!(c.dials, Dials::DEFAULT);
    }

    #[test]
    fn the_save_writes_version_2_with_its_dials() {
        let text = Character::human_warrior("Jacob").to_text();
        assert!(text.starts_with("wschar 2\n"), "{text}");
        assert!(text.contains("\nskin 2\n") && text.contains("\nfacial_hair 1\n"));
        assert_eq!(Dials::from_array([2, 1, 3, 1, 1]), Dials::DEFAULT);
    }

    #[test]
    fn a_missing_file_is_no_character_and_a_saved_one_is_read_back() {
        let dir = std::env::temp_dir().join(format!("wowsouls-char-{}", std::process::id()));
        let path = dir.join("character-v1.txt");
        assert_eq!(load(&path), Ok(None));
        let c = Character::human_warrior("Jacob");
        save(&path, &c).unwrap();
        assert_eq!(load(&path), Ok(Some(c)));
        std::fs::write(&path, "junk").unwrap();
        assert!(
            load(&path).is_err(),
            "a broken file is never taken for none"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
