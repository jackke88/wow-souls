//! A level 6-12 trial character: a save slot written from the same text formats the game reads, so
//! Hogger (level 11) can be fought at the level the zone boss is made for without a play-through.
//!
//! `wow-souls --trial-character <level> [slot]` writes one into a new slot (or the named slot if it
//! holds no character yet) and exits; the character list then shows it like any other.

use std::path::Path;

use wow_world::equipment::TRIAL_GEAR;
use wow_world::inventory::Progress;
use wow_world::quest::save::SaveGame;

use crate::character::{check_name, Character, Class, Dials, Race, Sex};

/// The name a trial character carries.
pub const TRIAL_NAME: &str = "Trialist";

/// The pack's Salves (the Estus of this game) and how many the trial character carries.
pub const TRIAL_FOOD: u32 = 9_000_001;
pub const TRIAL_FOOD_COUNT: u32 = 8;

/// The souls the trial character carries: enough for a few things at the vendors, not for the world.
pub const TRIAL_SOULS: u32 = 2_000;

/// The abilities it has learned: Charge, Battle Shout, Thunder Clap (the set the game offers).
const TRIAL_ABILITIES: [u32; 3] = [100, 6673, 6343];

/// Where a trial character starts (and where it stands again after a death): on the slope west of
/// Hogger's camp at Riverpark, about 24 yd from him (he notices at about 18), looking at him, so
/// the fight is one short walk away. His spawn is WoW (-9947.88, 594.77, 39.6).
pub const TRIAL_START: crate::saves::Start = crate::saves::Start {
    at: [-9924.6, 588.6, 45.0],
    face: Some([-9947.88, 594.77]),
};

/// The levels a trial character can be made at.
const LEVELS: std::ops::RangeInclusive<u32> = 6..=12;

/// The 1.12 XP needed in all to reach `level` (the `player_xp_for_level` table, cumulative).
pub fn xp_for_level(level: u32) -> u32 {
    const STEP: [u32; 11] = [
        400, 900, 1_400, 2_100, 2_800, 3_600, 4_500, 5_400, 6_500, 7_600, 8_800,
    ];
    STEP.iter().take(level.saturating_sub(1) as usize).sum()
}

/// Writes the slot's files into `dir`; the character file goes last, so a write that is cut short
/// leaves a folder that is not a character. Refuses a directory that already holds a character, a
/// level outside 6..=12 and a name the game would refuse, before touching anything.
pub fn write(dir: &Path, level: u32, name: &str) -> Result<(), String> {
    if !LEVELS.contains(&level) {
        return Err(format!(
            "level {level}: the trial characters are {} to {}",
            LEVELS.start(),
            LEVELS.end()
        ));
    }
    let name = check_name(name)?;
    if dir.join(crate::saves::CHARACTER_FILE).exists() {
        return Err(format!("{} already holds a character", dir.display()));
    }
    let character = Character {
        name: name.clone(),
        race: Race::Human,
        class: Class::Warrior,
        sex: Sex::Male,
        dials: Dials::DEFAULT,
    };
    let mut progress = Progress {
        kit_given: true,
        ..Progress::default()
    };
    for (slot, item) in TRIAL_GEAR {
        progress.gear.set(slot, Some(item));
    }
    progress.abilities.extend(TRIAL_ABILITIES);
    progress
        .inventory
        .add(&[(TRIAL_FOOD, TRIAL_FOOD_COUNT)])
        .map_err(|e| e.to_string())?;
    let quests = SaveGame {
        xp: xp_for_level(level),
        name,
        ..SaveGame::default()
    };
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let put = |file: &str, text: String| {
        std::fs::write(dir.join(file), text).map_err(|e| format!("{file}: {e}"))
    };
    put("quests-v1.txt", quests.to_text())?;
    put("items-v1.txt", progress.text())?;
    put(
        "souls-v1.txt",
        crate::bloodstain::to_text(TRIAL_SOULS, 0, None),
    )?;
    put(
        crate::saves::START_FILE,
        crate::saves::start_text(&TRIAL_START),
    )?;
    put(crate::saves::CHARACTER_FILE, character.to_text())
}

/// The command line of the trial command: `Ok(None)` when it is not that command (the game starts),
/// else the level and the slot asked for.
pub fn parse_args(args: &[String]) -> Result<Option<(u32, Option<u32>)>, String> {
    if args.first().map(String::as_str) != Some("--trial-character") {
        return Ok(None);
    }
    let level = args
        .get(1)
        .ok_or("--trial-character wants a level (6 to 12)")?
        .parse::<u32>()
        .map_err(|_| "--trial-character: the level is a number (6 to 12)".to_string())?;
    let slot = match args.get(2) {
        None => None,
        Some(s) => Some(
            s.parse::<u32>()
                .ok()
                .filter(|&n| n >= 1)
                .ok_or("--trial-character: the slot is a number from 1")?,
        ),
    };
    Ok(Some((level, slot)))
}

/// The trial command: `Some(exit code)` when `args` is it (the game must not start), else `None`.
pub fn cli(args: Vec<String>) -> Option<i32> {
    let (level, slot) = match parse_args(&args) {
        Ok(None) => return None,
        Ok(Some(ask)) => ask,
        Err(e) => {
            eprintln!("{e}");
            return Some(2);
        }
    };
    let root = crate::saves::root();
    let Some(id) = slot.or_else(|| crate::saves::next_id(&root)) else {
        eprintln!(
            "the character list is full ({} characters)",
            crate::saves::MAX_CHARACTERS
        );
        return Some(2);
    };
    let dir = crate::saves::slot_dir(&root, id);
    match write(&dir, level, TRIAL_NAME) {
        Ok(()) => {
            println!(
                "trial character level {level} written to slot {id} ({}); pick {TRIAL_NAME} in the \
                 character list, or start with WOWSOULS_SLOT={id}",
                dir.display()
            );
            Some(0)
        }
        Err(e) => {
            eprintln!("{e}");
            Some(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::equipment::Slot;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wowsouls-trial-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_xp_table_is_the_1_12_one() {
        assert_eq!(xp_for_level(6), 7_600);
        assert_eq!(xp_for_level(10), 27_600);
        assert_eq!(xp_for_level(11), 35_200);
        assert_eq!(xp_for_level(12), 44_000);
        assert_eq!(xp_for_level(1), 0);
    }

    #[test]
    fn a_trial_slot_reads_back_as_a_level_11_warrior_with_gear_and_abilities() {
        let dir = scratch("read");
        write(&dir, 11, "Trialist").unwrap();
        let c = crate::character::Character::from_text(
            &std::fs::read_to_string(dir.join("character-v1.txt")).unwrap(),
        )
        .unwrap();
        assert_eq!(c.name, "Trialist");
        assert!(c.playable());

        let q = wow_world::quest::save::SaveGame::from_text(
            &std::fs::read_to_string(dir.join("quests-v1.txt")).unwrap(),
        )
        .unwrap();
        assert_eq!((q.xp, q.name.as_str()), (35_200, "Trialist"));

        let p = wow_world::inventory::Progress::parse(
            &std::fs::read_to_string(dir.join("items-v1.txt")).unwrap(),
        )
        .unwrap();
        for (slot, item) in wow_world::equipment::TRIAL_GEAR {
            assert_eq!(p.gear.get(slot), Some(item), "{}", slot.key());
        }
        assert!(p.kit_given, "the starting kit is not handed out again");
        assert_eq!(p.gear.get(Slot::Shirt), None);
        for spell in [100, 6673, 6343] {
            assert!(p.abilities.contains(&spell), "{spell}");
        }
        assert_eq!(p.inventory.count(TRIAL_FOOD), TRIAL_FOOD_COUNT);

        let (carried, banked, stain) = crate::bloodstain::from_text(
            &std::fs::read_to_string(dir.join("souls-v1.txt")).unwrap(),
        )
        .unwrap();
        assert_eq!((carried, banked, stain), (TRIAL_SOULS, 0, None));
        // It starts by Hogger, facing him, and the start file reads back as written.
        let start = crate::saves::parse_start(
            &std::fs::read_to_string(dir.join(crate::saves::START_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(start, TRIAL_START);
        let hogger = [-9947.88_f32, 594.77];
        let from = TRIAL_START.at;
        let away = ((from[0] - hogger[0]).powi(2) + (from[1] - hogger[1]).powi(2)).sqrt();
        assert!((18.0..40.0).contains(&away), "{away} yd from Hogger");
        assert_eq!(TRIAL_START.face, Some(hogger));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_slot_with_a_character_is_never_overwritten() {
        let dir = scratch("keep");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("character-v1.txt"), "wschar 2\nname Mine\n").unwrap();
        assert!(write(&dir, 11, "Trialist").is_err());
        assert_eq!(
            std::fs::read_to_string(dir.join("character-v1.txt")).unwrap(),
            "wschar 2\nname Mine\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn levels_outside_the_trial_range_and_names_the_game_refuses_are_refused() {
        let dir = scratch("range");
        assert!(write(&dir, 0, "Trialist").is_err());
        assert!(write(&dir, 5, "Trialist").is_err());
        assert!(write(&dir, 13, "Trialist").is_err());
        assert!(write(&dir, 11, "Hogger trial").is_err());
        assert!(!dir.exists(), "a refused write leaves nothing behind");
    }

    #[test]
    fn the_command_line_names_a_level_and_perhaps_a_slot() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse_args(&args(&["--trial-character", "11"])),
            Ok(Some((11, None)))
        );
        assert_eq!(
            parse_args(&args(&["--trial-character", "11", "4"])),
            Ok(Some((11, Some(4))))
        );
        assert!(parse_args(&args(&["--trial-character", "x"])).is_err());
        assert!(parse_args(&args(&["--trial-character"])).is_err());
        assert!(parse_args(&args(&["--trial-character", "11", "0"])).is_err());
        // Any other command line is the game's.
        assert_eq!(parse_args(&args(&[])), Ok(None));
        assert_eq!(parse_args(&args(&["--something"])), Ok(None));
    }
}
