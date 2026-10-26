//! Where every character's progress lives. Each character has a folder of its own,
//! `%LOCALAPPDATA%\WoWSouls\characters\<n>\`, holding the files that are theirs (the character,
//! the quest log, the pack and the gear, the souls, the day clock, the valley's history, soul binding). The
//! settings, the key bindings and the audio log stay beside them: they belong to the player.
//!
//! A process plays one character, found once at the first use: `WOWSOULS_SLOT=<n>` (a check, or
//! the relaunch of a character switch), else the slot last played (`active-v1.txt`), else the first
//! one there is, else the next free number. Switching to another character relaunches the game (every
//! save is read at start); deleting one moves its folder to `backup-<stamp>-character-<n>`, nothing
//! is erased.
//!
//! The loose files of the single-character game (`WoWSouls\character-v1.txt` ...) move into slot 1
//! the first time any of them is looked for.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use bevy::prelude::{info, warn};

/// The most characters the list holds (WoW's per realm).
pub const MAX_CHARACTERS: usize = 10;

/// The marker of a slot: a folder without it is not a character.
pub const CHARACTER_FILE: &str = "character-v1.txt";

/// The files that belong to one character.
pub const PER_CHARACTER: [&str; 9] = [
    CHARACTER_FILE,
    "quests-v1.txt",
    "items-v1.txt",
    "souls-v1.txt",
    "clock-v1.txt",
    crate::world_state::FILE_NAME,
    crate::soulbind::FILE_NAME,
    crate::talents::FILE_NAME,
    crate::torch::FILE_NAME,
];

/// Which slot was played last.
const ACTIVE_FILE: &str = "active-v1.txt";
const ACTIVE_HEADER: &str = "wsactive 1";

pub fn characters_dir(root: &Path) -> PathBuf {
    root.join("characters")
}

/// A slot's folder; it may not exist yet.
pub fn slot_dir(root: &Path, id: u32) -> PathBuf {
    characters_dir(root).join(id.to_string())
}

/// Every numbered folder under `characters`, sorted, characters or not.
fn folder_ids(root: &Path) -> Vec<u32> {
    let mut ids: Vec<u32> = std::fs::read_dir(characters_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str()?.parse().ok())
        .collect();
    ids.sort_unstable();
    ids
}

/// The slots that hold a character, in order.
pub fn slots(root: &Path) -> Vec<u32> {
    folder_ids(root)
        .into_iter()
        .filter(|&id| slot_dir(root, id).join(CHARACTER_FILE).is_file())
        .collect()
}

/// The number a new character gets: one past the highest folder (a number is not reused while
/// its folder is there), `None` when the list is full.
pub fn next_id(root: &Path) -> Option<u32> {
    if slots(root).len() >= MAX_CHARACTERS {
        return None;
    }
    Some(folder_ids(root).last().map_or(1, |&last| last + 1))
}

/// Moves the loose saves of the single-character game into slot 1; whether it did. The character
/// file goes last, so a move that was cut short is finished at the next start, and a slot 1 that
/// already has a character is never touched.
pub fn migrate_legacy(root: &Path) -> io::Result<bool> {
    let loose: Vec<&str> = PER_CHARACTER
        .iter()
        .copied()
        .filter(|name| root.join(name).is_file())
        .collect();
    let dest = slot_dir(root, 1);
    if loose.is_empty() || dest.join(CHARACTER_FILE).is_file() {
        return Ok(false);
    }
    std::fs::create_dir_all(&dest)?;
    let ordered = loose
        .iter()
        .copied()
        .filter(|&n| n != CHARACTER_FILE)
        .chain(loose.iter().copied().filter(|&n| n == CHARACTER_FILE));
    for name in ordered {
        if !dest.join(name).exists() {
            std::fs::rename(root.join(name), dest.join(name))?;
        }
    }
    set_active(root, 1)?;
    Ok(true)
}

/// The slot the pointer file names, if it reads.
pub fn read_active(root: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(root.join(ACTIVE_FILE)).ok()?;
    let mut lines = text.lines();
    (lines.next()?.trim() == ACTIVE_HEADER)
        .then(|| lines.next()?.trim().parse().ok())
        .flatten()
}

/// Remembers the slot played last.
pub fn set_active(root: &Path, id: u32) -> io::Result<()> {
    std::fs::create_dir_all(root)?;
    let tmp = root.join(ACTIVE_FILE).with_extension("tmp");
    std::fs::write(&tmp, format!("{ACTIVE_HEADER}\n{id}\n"))?;
    std::fs::rename(&tmp, root.join(ACTIVE_FILE))
}

/// The slot to play: the switch, else the one played last if it is still there, else the first
/// character, else the number the first character will get.
pub fn resolve_active(root: &Path, switch: Option<u32>) -> u32 {
    if let Some(id) = switch.filter(|&id| id >= 1) {
        return id;
    }
    let have = slots(root);
    if let Some(id) = read_active(root).filter(|id| have.contains(id)) {
        return id;
    }
    have.first().copied().unwrap_or(1)
}

/// Moves a slot's folder to a backup folder beside the saves (nothing is deleted) and returns it.
pub fn archive_slot(root: &Path, id: u32, stamp: u64) -> io::Result<PathBuf> {
    let to = root.join(format!("backup-{stamp}-character-{id}"));
    std::fs::rename(slot_dir(root, id), &to)?;
    Ok(to)
}

// ---------------------------------------------------------------------------------------------
// This process's character.
// ---------------------------------------------------------------------------------------------

/// The folder the saves live in: `%LOCALAPPDATA%\WoWSouls`.
pub fn root() -> PathBuf {
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "saves".into())).join("WoWSouls")
}

/// The slot this process plays, decided once (after the loose saves have moved in).
pub fn active() -> u32 {
    static ACTIVE: OnceLock<u32> = OnceLock::new();
    *ACTIVE.get_or_init(|| {
        let root = root();
        match migrate_legacy(&root) {
            Ok(true) => info!(
                "saves: the loose saves of the single-character game moved into {}",
                slot_dir(&root, 1).display()
            ),
            Ok(false) => {}
            Err(e) => warn!("saves: the loose saves could not be moved ({e})"),
        }
        let switch = std::env::var("WOWSOULS_SLOT")
            .ok()
            .and_then(|v| v.trim().parse().ok());
        let id = resolve_active(&root, switch);
        info!("saves: character slot {id}");
        id
    })
}

/// A save of this process's character: `<root>\characters\<n>\<name>`.
pub fn file(name: &str) -> PathBuf {
    slot_dir(&root(), active()).join(name)
}

// ---------------------------------------------------------------------------------------------
// Where a character starts.
// ---------------------------------------------------------------------------------------------

/// A character's own start: `<slot>\start-v1.txt`. A character without one starts where the game
/// does (the Abbey's doors); a trial character starts by the boss it is made to fight.
pub const START_FILE: &str = "start-v1.txt";
const START_HEADER: &str = "wsstart 1";

/// Where a character stands when the world opens, and where it stands again after a death (the
/// settled body is the spawn point): a point in WoW coordinates (x north, y west, z up; the height
/// is a seed the ground snap corrects) and, optionally, a point to face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Start {
    pub at: [f32; 3],
    pub face: Option<[f32; 2]>,
}

/// The text of a start file.
pub fn start_text(start: &Start) -> String {
    let mut text = format!(
        "{START_HEADER}\nat {} {} {}\n",
        start.at[0], start.at[1], start.at[2]
    );
    if let Some([x, y]) = start.face {
        text.push_str(&format!("face {x} {y}\n"));
    }
    text
}

/// A start file's text; an error names the line.
pub fn parse_start(text: &str) -> Result<Start, String> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next() != Some(START_HEADER) {
        return Err(format!(
            "not a start file (the first line is {START_HEADER:?})"
        ));
    }
    let numbers = |line: &str, rest: &[&str], n: usize| -> Result<Vec<f32>, String> {
        if rest.len() != n {
            return Err(format!("{line:?}: {n} numbers wanted"));
        }
        rest.iter()
            .map(|w| {
                w.parse::<f32>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("{line:?}: {w:?} is not a number"))
            })
            .collect()
    };
    let (mut at, mut face) = (None, None);
    for line in lines {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.first().copied() {
            Some("at") => {
                let v = numbers(line, &words[1..], 3)?;
                at = Some([v[0], v[1], v[2]]);
            }
            Some("face") => {
                let v = numbers(line, &words[1..], 2)?;
                face = Some([v[0], v[1]]);
            }
            _ => return Err(format!("{line:?}: not a start line")),
        }
    }
    Ok(Start {
        at: at.ok_or("no `at` line")?,
        face,
    })
}

/// This process's character's start, if it has a file and the run is one a person plays: an
/// instrumented run starts at the anchor the scripted checks assume (`WOWSOULS_START=1` honours the
/// file there too, `WOWSOULS_START=0` ignores it anywhere). Decided once.
pub fn start() -> Option<Start> {
    static START: OnceLock<Option<Start>> = OnceLock::new();
    *START.get_or_init(|| {
        let switch = std::env::var("WOWSOULS_START").ok();
        let wanted = match switch.as_deref().map(str::trim) {
            Some("0") => false,
            Some("1") => true,
            _ => !crate::run_mode::unattended(),
        };
        if !wanted {
            return None;
        }
        let path = file(START_FILE);
        let text = std::fs::read_to_string(&path).ok()?;
        match parse_start(&text) {
            Ok(start) => {
                info!(
                    "saves: this character starts at wow ({:.1}, {:.1}, {:.1})",
                    start.at[0], start.at[1], start.at[2]
                );
                Some(start)
            }
            Err(e) => {
                warn!(
                    "saves: {}: {e}; the character starts at the Abbey",
                    path.display()
                );
                None
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wowsouls-saves-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn character(root: &Path, id: u32) {
        let dir = slot_dir(root, id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(CHARACTER_FILE), format!("who {id}")).unwrap();
    }

    #[test]
    fn a_start_file_round_trips_and_a_broken_one_names_its_line() {
        let with_face = Start {
            at: [-9924.5, 588.25, 45.0],
            face: Some([-9947.88, 594.77]),
        };
        assert_eq!(parse_start(&start_text(&with_face)), Ok(with_face));
        let bare = Start {
            at: [1.0, -2.0, 3.0],
            face: None,
        };
        assert_eq!(parse_start(&start_text(&bare)), Ok(bare));
        assert!(parse_start("at 1 2 3\n")
            .unwrap_err()
            .contains("not a start file"));
        assert!(parse_start("wsstart 1\nat 1 2\n")
            .unwrap_err()
            .contains("3 numbers"));
        assert!(parse_start("wsstart 1\nat 1 two 3\n")
            .unwrap_err()
            .contains("not a number"));
        assert!(parse_start("wsstart 1\nat 1 nan 3\n").is_err());
        assert!(parse_start("wsstart 1\nface 1 2\n")
            .unwrap_err()
            .contains("no `at`"));
        assert!(parse_start("wsstart 1\nat 1 2 3\nwhere 4\n")
            .unwrap_err()
            .contains("not a start line"));
    }

    #[test]
    fn slots_are_the_numbered_folders_that_hold_a_character() {
        let root = scratch("slots");
        assert!(slots(&root).is_empty());
        character(&root, 3);
        character(&root, 1);
        // A folder with no character file, and one that is not a number, are not slots.
        std::fs::create_dir_all(slot_dir(&root, 2)).unwrap();
        std::fs::create_dir_all(characters_dir(&root).join("junk")).unwrap();
        assert_eq!(slots(&root), [1, 3]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_next_number_is_one_past_the_highest_folder_and_the_list_has_a_cap() {
        let root = scratch("next");
        assert_eq!(next_id(&root), Some(1));
        character(&root, 1);
        character(&root, 4);
        assert_eq!(next_id(&root), Some(5));
        // A folder without a character still holds its number.
        std::fs::create_dir_all(slot_dir(&root, 6)).unwrap();
        assert_eq!(next_id(&root), Some(7));
        for id in 10..10 + MAX_CHARACTERS as u32 {
            character(&root, id);
        }
        assert_eq!(next_id(&root), None, "a full list takes no more");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_loose_saves_of_the_old_game_move_into_slot_one_once() {
        let root = scratch("legacy");
        for name in PER_CHARACTER {
            std::fs::write(root.join(name), name).unwrap();
        }
        std::fs::write(root.join("settings-v1.txt"), "keep").unwrap();
        assert!(migrate_legacy(&root).unwrap());
        for name in PER_CHARACTER {
            assert!(!root.join(name).exists(), "{name} moved out");
            assert_eq!(
                std::fs::read_to_string(slot_dir(&root, 1).join(name)).unwrap(),
                name
            );
        }
        assert!(root.join("settings-v1.txt").exists(), "the settings stay");
        assert_eq!(read_active(&root), Some(1));
        assert_eq!(slots(&root), [1]);
        // Nothing left to move, and a second run does nothing.
        assert!(!migrate_legacy(&root).unwrap());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_slot_one_that_has_a_character_is_never_overwritten_by_loose_files() {
        let root = scratch("legacy-keep");
        character(&root, 1);
        std::fs::write(root.join(CHARACTER_FILE), "old").unwrap();
        assert!(!migrate_legacy(&root).unwrap());
        assert_eq!(
            std::fs::read_to_string(slot_dir(&root, 1).join(CHARACTER_FILE)).unwrap(),
            "who 1"
        );
        assert!(root.join(CHARACTER_FILE).exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_move_cut_short_is_finished_at_the_next_start() {
        let root = scratch("legacy-cut");
        // The quest log made it across; the character file and the pack did not.
        std::fs::create_dir_all(slot_dir(&root, 1)).unwrap();
        std::fs::write(slot_dir(&root, 1).join("quests-v1.txt"), "moved").unwrap();
        std::fs::write(root.join(CHARACTER_FILE), "c").unwrap();
        std::fs::write(root.join("items-v1.txt"), "i").unwrap();
        assert!(migrate_legacy(&root).unwrap());
        assert_eq!(slots(&root), [1]);
        assert!(slot_dir(&root, 1).join("items-v1.txt").is_file());
        assert!(!root.join("items-v1.txt").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_slot_to_play_is_the_switch_then_the_last_played_then_the_first() {
        let root = scratch("active");
        assert_eq!(
            resolve_active(&root, None),
            1,
            "nothing yet: the first number"
        );
        character(&root, 2);
        character(&root, 5);
        assert_eq!(
            resolve_active(&root, None),
            2,
            "no pointer: the first character"
        );
        set_active(&root, 5).unwrap();
        assert_eq!(resolve_active(&root, None), 5);
        assert_eq!(resolve_active(&root, Some(2)), 2, "the switch wins");
        assert_eq!(
            resolve_active(&root, Some(9)),
            9,
            "a slot that is about to be made"
        );
        // A pointer at a slot that is gone falls back to the first character.
        set_active(&root, 7).unwrap();
        assert_eq!(resolve_active(&root, None), 2);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_bad_pointer_file_is_ignored() {
        let root = scratch("pointer");
        std::fs::write(root.join(ACTIVE_FILE), "wsactive 2\n3\n").unwrap();
        assert_eq!(read_active(&root), None);
        std::fs::write(root.join(ACTIVE_FILE), "wsactive 1\nthree\n").unwrap();
        assert_eq!(read_active(&root), None);
        set_active(&root, 4).unwrap();
        assert_eq!(read_active(&root), Some(4));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A new character must not inherit the old one's dead bosses (the final review of the first
    /// replace flow, Critical): the valley's history is a character's file like the rest.
    #[test]
    fn a_characters_folder_holds_every_save_including_the_valleys_history() {
        for name in [
            "character-v1.txt",
            "quests-v1.txt",
            "items-v1.txt",
            "souls-v1.txt",
            "clock-v1.txt",
            crate::world_state::FILE_NAME,
            crate::soulbind::FILE_NAME,
            crate::talents::FILE_NAME,
            crate::torch::FILE_NAME,
        ] {
            assert!(PER_CHARACTER.contains(&name), "{name}");
        }
        assert_eq!(PER_CHARACTER.len(), 9);
    }

    #[test]
    fn archiving_a_slot_moves_its_whole_folder_and_keeps_the_others() {
        let root = scratch("archive");
        character(&root, 1);
        character(&root, 2);
        std::fs::write(slot_dir(&root, 2).join("items-v1.txt"), "pack").unwrap();
        let backup = archive_slot(&root, 2, 77).unwrap();
        assert!(backup.ends_with("backup-77-character-2"));
        assert_eq!(
            std::fs::read_to_string(backup.join("items-v1.txt")).unwrap(),
            "pack"
        );
        assert_eq!(slots(&root), [1]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
