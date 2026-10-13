//! The creation screen's rules, pure and tested without a window: which classes a race can be, how many
//! steps each of the five dials has, and what a click on a race, sex, class or dial does to the
//! selection (the reference's `CharacterCreate.lua`: a race switch resets the class to the race's first
//! and the facing to -15 degrees, a sex switch only re-clamps, the dials wrap).
//!
//! The ids are the client's own: `ChrRaces` 1..=8 and `ChrClasses` (1 warrior, 2 paladin, 3 hunter, 4 rogue,
//! 5 priest, 7 shaman, 8 mage, 9 warlock, 11 druid); sex 0 male, 1 female.

use std::collections::{HashMap, HashSet};

use benilla_formats::CharCreateCatalog;

/// The Alliance column, top to bottom: Human, Dwarf, Night Elf, Gnome. `CharacterCreate.xml` chains buttons
/// 1-4 down the first column and 5-8 down the second, in `GetAvailableRaces()` order (ascending id).
pub const ALLIANCE: [u8; 4] = [1, 3, 4, 7];
/// The Horde column: Orc, Undead, Tauren, Troll.
pub const HORDE: [u8; 4] = [2, 5, 6, 8];
/// `SetCharacterCreateFacing(-15)`, in radians; reset on every race switch.
pub const INITIAL_FACING: f32 = -15.0 * std::f32::consts::PI / 180.0;

/// Every class id, ascending: the fallback when a race has no catalog row.
pub const ALL_CLASSES: [u8; 9] = [1, 2, 3, 4, 5, 7, 8, 9, 11];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Faction {
    Alliance,
    Horde,
}

pub fn faction_of(race: u8) -> Faction {
    if ALLIANCE.contains(&race) {
        Faction::Alliance
    } else {
        Faction::Horde
    }
}

/// The client's race names (`GetNameForRace`; Undead is "Undead" in 1.12's strings).
pub fn race_name(race: u8) -> &'static str {
    match race {
        1 => "Human",
        2 => "Orc",
        3 => "Dwarf",
        4 => "Night Elf",
        5 => "Undead",
        6 => "Tauren",
        7 => "Gnome",
        8 => "Troll",
        _ => "Unknown",
    }
}

pub fn class_name(class: u8) -> &'static str {
    match class {
        1 => "Warrior",
        2 => "Paladin",
        3 => "Hunter",
        4 => "Rogue",
        5 => "Priest",
        7 => "Shaman",
        8 => "Mage",
        9 => "Warlock",
        11 => "Druid",
        _ => "Unknown",
    }
}

/// The class's file string: the key of `CLASS_ICON_TCOORDS` and the `CLASS_<FILE>` glue string.
pub fn class_file(class: u8) -> &'static str {
    match class {
        1 => "WARRIOR",
        2 => "PALADIN",
        3 => "HUNTER",
        4 => "ROGUE",
        5 => "PRIEST",
        7 => "SHAMAN",
        8 => "MAGE",
        9 => "WARLOCK",
        11 => "DRUID",
        _ => "WARRIOR",
    }
}

/// What the client's tables allow, as plain data (a test builds one by hand).
#[derive(Debug, Clone, Default)]
pub struct Rules {
    classes: HashMap<u8, Vec<u8>>,
    ranges: HashMap<(u8, u8), [u8; 5]>,
    /// The (race, sex) pairs whose facial-hair dial is not shown (the client's `NONE` token).
    no_facial: HashSet<(u8, u8)>,
    /// The races the screen lets the player choose; the others are drawn grey. `None` is every race.
    choosable_races: Option<HashSet<u8>>,
}

impl Rules {
    pub fn from_catalog(c: &CharCreateCatalog) -> Rules {
        let mut rules = Rules::default();
        for race in 1..=8u8 {
            rules.classes.insert(race, c.classes_for_race(race));
            for sex in 0..=1u8 {
                if let Some(r) = c.ranges(race, sex) {
                    rules.ranges.insert(
                        (race, sex),
                        [r.skin, r.face, r.hair_style, r.hair_color, r.facial_hair],
                    );
                }
                if c.facial_hair_customization(race, sex) == Some("NONE") {
                    rules.no_facial.insert((race, sex));
                }
            }
        }
        rules
    }

    /// Whether the facial-hair dial is left off the screen for a (race, sex): the client hides it
    /// when `GetFacialHairCustomization()` is `"NONE"`.
    pub fn facial_hair_hidden(&self, race: u8, sex: u8) -> bool {
        self.no_facial.contains(&(race, sex))
    }

    /// Only `races` can be chosen; the screen draws the others grey (the game has the Northshire
    /// Valley and the Human start only, each race's own start zone is yet to come).
    pub fn only_races(mut self, races: &[u8]) -> Rules {
        self.choosable_races = Some(races.iter().copied().collect());
        self
    }

    /// Whether a race can be chosen on the screen.
    pub fn race_choosable(&self, race: u8) -> bool {
        self.choosable_races
            .as_ref()
            .is_none_or(|set| set.contains(&race))
    }

    /// The classes a race can be, ascending id (`CharBaseInfo` order).
    pub fn classes(&self, race: u8) -> &[u8] {
        self.classes
            .get(&race)
            .map_or(&ALL_CLASSES[..], Vec::as_slice)
    }

    /// The five dials' step counts for a (race, sex): skin, face, hair style, hair colour, facial hair.
    /// Never a zero (a dial with nothing to choose has one step).
    pub fn ranges(&self, race: u8, sex: u8) -> [u8; 5] {
        self.ranges
            .get(&(race, sex))
            .map_or([1; 5], |r| r.map(|n| n.max(1)))
    }

    /// A small fixed rule set for tests: the 1.12 class lists, ranges that differ by race and sex.
    #[cfg(test)]
    pub fn standard() -> Rules {
        let mut rules = Rules::default();
        let lists: [(u8, &[u8]); 8] = [
            (1, &[1, 2, 4, 5, 8, 9]),
            (2, &[1, 3, 4, 7, 9]),
            (3, &[1, 2, 3, 4, 5]),
            (4, &[1, 3, 4, 5, 11]),
            (5, &[1, 4, 5, 8, 9]),
            (6, &[1, 3, 7, 11]),
            (7, &[1, 4, 8, 9]),
            (8, &[1, 3, 4, 5, 7, 8]),
        ];
        for (race, classes) in lists {
            rules.classes.insert(race, classes.to_vec());
            for sex in 0..=1u8 {
                let n = race + sex * 2;
                rules
                    .ranges
                    .insert((race, sex), [9, 10, 5 + n, 8 + sex, 1 + n % 3]);
            }
        }
        rules
    }
}

/// What the screen has selected.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub race: u8,
    pub sex: u8,
    pub class: u8,
    /// `[skin, face, hair style, hair colour, facial hair]`.
    pub dials: [u8; 5],
    pub name: String,
    /// The preview's facing, radians (drag and the rotate buttons turn it).
    pub yaw: f32,
}

impl Selection {
    /// Human, male, the race's first class, every dial 0 (`ResetCharCustomize` picks at random in the
    /// client; the game opens on the plain one, so the first picture is the same every time).
    pub fn new(rules: &Rules) -> Selection {
        Selection {
            race: 1,
            sex: 0,
            class: rules.classes(1).first().copied().unwrap_or(1),
            dials: [0; 5],
            name: String::new(),
            yaw: INITIAL_FACING,
        }
    }

    fn clamp(&mut self, rules: &Rules) {
        let counts = rules.ranges(self.race, self.sex);
        for (d, n) in self.dials.iter_mut().zip(counts) {
            *d = (*d).min(n - 1);
        }
        if !rules.classes(self.race).contains(&self.class) {
            self.class = rules.classes(self.race).first().copied().unwrap_or(1);
        }
    }

    /// A click on a race button: only a real change does anything.
    pub fn pick_race(&mut self, race: u8, rules: &Rules) {
        if race == self.race || !(1..=8).contains(&race) || !rules.race_choosable(race) {
            return;
        }
        self.race = race;
        self.class = rules.classes(race).first().copied().unwrap_or(1);
        self.clamp(rules);
        self.yaw = INITIAL_FACING;
    }

    pub fn pick_sex(&mut self, sex: u8, rules: &Rules) {
        if sex == self.sex || sex > 1 {
            return;
        }
        self.sex = sex;
        self.clamp(rules);
    }

    /// A click on the `slot`th class button of the race's list; a slot past the list does nothing.
    pub fn pick_class_slot(&mut self, slot: usize, rules: &Rules) {
        if let Some(&class) = rules.classes(self.race).get(slot) {
            self.class = class;
        }
    }

    /// The `slot` of the selected class in the race's list.
    pub fn class_slot(&self, rules: &Rules) -> Option<usize> {
        rules
            .classes(self.race)
            .iter()
            .position(|&c| c == self.class)
    }

    /// An arrow on a dial: one step, wrapping within the dial's count.
    pub fn cycle_dial(&mut self, dial: usize, dir: i8, rules: &Rules) {
        let Some(slot) = self.dials.get_mut(dial) else {
            return;
        };
        let n = i32::from(rules.ranges(self.race, self.sex)[dial]);
        *slot = (i32::from(*slot) + i32::from(dir)).rem_euclid(n) as u8;
    }

    /// Randomize: every dial to a random step of its range (xorshift with a golden-ratio add, the fork's).
    pub fn randomize(&mut self, rng: &mut u64, rules: &Rules) {
        let counts = rules.ranges(self.race, self.sex);
        for (d, n) in self.dials.iter_mut().zip(counts) {
            *rng ^= *rng << 13;
            *rng ^= *rng >> 7;
            *rng ^= *rng << 17;
            *rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
            *d = (*rng % u64::from(n)) as u8;
        }
    }

    /// The only combination the game can play yet: Human, Warrior, male.
    pub fn playable(&self) -> bool {
        self.race == 1 && self.class == 1 && self.sex == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_columns_partition_the_eight_races() {
        let mut all: Vec<u8> = ALLIANCE.iter().chain(&HORDE).copied().collect();
        all.sort_unstable();
        assert_eq!(all, (1..=8).collect::<Vec<u8>>());
        assert_eq!(faction_of(7), Faction::Alliance, "gnome");
        assert_eq!(faction_of(5), Faction::Horde, "undead");
    }

    #[test]
    fn the_classes_of_a_race_come_from_the_rules_in_ascending_order() {
        let r = Rules::standard();
        assert_eq!(
            r.classes(1),
            [1, 2, 4, 5, 8, 9],
            "human has no hunter in 1.12"
        );
        assert_eq!(r.classes(6), [1, 3, 7, 11], "tauren");
        assert_eq!(
            Rules::default().classes(1),
            ALL_CLASSES,
            "no data: every class"
        );
    }

    #[test]
    fn a_new_selection_is_the_plain_human_warrior_facing_minus_fifteen_degrees() {
        let r = Rules::standard();
        let s = Selection::new(&r);
        assert_eq!((s.race, s.sex, s.class, s.dials), (1, 0, 1, [0; 5]));
        assert!((s.yaw - INITIAL_FACING).abs() < 1e-6);
        assert!(s.playable());
    }

    #[test]
    fn a_race_switch_resets_class_and_facing_and_clamps_the_dials() {
        let r = Rules::standard();
        let mut s = Selection::new(&r);
        s.pick_class_slot(3, &r); // priest
        s.dials = [8, 9, 9, 7, 2];
        s.yaw = 1.0;
        s.pick_race(6, &r); // tauren
        assert_eq!(s.race, 6);
        assert_eq!(s.class, 1, "the race's first class");
        assert!(
            (s.yaw - INITIAL_FACING).abs() < 1e-6,
            "facing back to -15 degrees"
        );
        let counts = r.ranges(6, 0);
        for (d, n) in s.dials.iter().zip(counts) {
            assert!(*d < n, "{d} within {n}");
        }
    }

    #[test]
    fn a_grey_race_cannot_be_chosen_and_the_rest_still_can() {
        let r = Rules::standard().only_races(&[1]);
        assert!(r.race_choosable(1));
        for race in 2..=8 {
            assert!(!r.race_choosable(race), "{race}");
        }
        let mut s = Selection::new(&r);
        s.yaw = 0.7;
        s.pick_race(2, &r);
        assert_eq!(
            (s.race, s.class),
            (1, 1),
            "the orc is grey: nothing happens"
        );
        assert!((s.yaw - 0.7).abs() < 1e-6, "not even the facing resets");
        // Every race is choosable without a restriction (what the tests and the dev switch use).
        let all = Rules::standard();
        assert!((1..=8).all(|race| all.race_choosable(race)));
        let mut s = Selection::new(&all);
        s.pick_race(2, &all);
        assert_eq!(s.race, 2);
    }

    #[test]
    fn choosing_the_same_race_or_sex_changes_nothing() {
        let r = Rules::standard();
        let mut s = Selection::new(&r);
        s.pick_class_slot(2, &r);
        s.yaw = 0.7;
        let before = s.clone();
        s.pick_race(1, &r);
        s.pick_sex(0, &r);
        assert_eq!(s, before);
    }

    #[test]
    fn a_sex_switch_keeps_the_class_and_clamps_the_dials() {
        let r = Rules::standard();
        let mut s = Selection::new(&r);
        s.pick_class_slot(1, &r);
        let class = s.class;
        s.dials = [8, 9, 9, 7, 2];
        s.yaw = 0.5;
        s.pick_sex(1, &r);
        assert_eq!(s.sex, 1);
        assert_eq!(s.class, class);
        assert!((s.yaw - 0.5).abs() < 1e-6, "the facing is kept");
        let counts = r.ranges(1, 1);
        for (d, n) in s.dials.iter().zip(counts) {
            assert!(*d < n);
        }
    }

    #[test]
    fn a_class_slot_past_the_list_is_ignored() {
        let r = Rules::standard();
        let mut s = Selection::new(&r);
        s.pick_race(6, &r);
        s.pick_class_slot(7, &r);
        assert_eq!(s.class, 1);
        s.pick_class_slot(3, &r);
        assert_eq!(s.class, 11, "tauren's fourth class is druid");
        assert_eq!(s.class_slot(&r), Some(3));
    }

    #[test]
    fn a_dial_wraps_both_ways() {
        let r = Rules::standard();
        let mut s = Selection::new(&r);
        let n = r.ranges(1, 0)[0];
        s.cycle_dial(0, -1, &r);
        assert_eq!(s.dials[0], n - 1, "left from 0 lands on the last");
        s.cycle_dial(0, 1, &r);
        assert_eq!(s.dials[0], 0);
        s.cycle_dial(9, 1, &r); // not a dial: nothing
        assert_eq!(s.dials, [0, 0, 0, 0, 0]);
    }

    #[test]
    fn randomize_stays_in_range_for_every_race_and_sex_and_follows_its_seed() {
        let r = Rules::standard();
        for race in 1..=8u8 {
            for sex in 0..=1u8 {
                let mut s = Selection::new(&r);
                s.pick_race(race, &r);
                s.pick_sex(sex, &r);
                let counts = r.ranges(race, sex);
                let mut rng = 0x1234_5678_u64 + u64::from(race);
                for _ in 0..200 {
                    s.randomize(&mut rng, &r);
                    for (d, n) in s.dials.iter().zip(counts) {
                        assert!(*d < n, "race {race} sex {sex}: {d} within {n}");
                    }
                }
            }
        }
        let (mut a, mut b) = (Selection::new(&r), Selection::new(&r));
        let (mut ra, mut rb) = (42_u64, 42_u64);
        a.randomize(&mut ra, &r);
        b.randomize(&mut rb, &r);
        assert_eq!(a.dials, b.dials, "the same seed gives the same look");
    }

    #[test]
    fn only_a_human_male_warrior_is_playable() {
        let r = Rules::standard();
        let mut s = Selection::new(&r);
        assert!(s.playable());
        s.pick_class_slot(1, &r);
        assert!(!s.playable(), "paladin");
        s.pick_class_slot(0, &r);
        s.pick_sex(1, &r);
        assert!(!s.playable(), "female");
        s.pick_sex(0, &r);
        s.pick_race(2, &r);
        assert!(!s.playable(), "orc");
    }

    #[test]
    fn names_and_files_cover_every_race_and_class() {
        for race in 1..=8u8 {
            assert_ne!(race_name(race), "Unknown");
        }
        for class in ALL_CLASSES {
            assert_ne!(class_name(class), "Unknown");
            assert_eq!(class_file(class), class_name(class).to_ascii_uppercase());
        }
    }
}
