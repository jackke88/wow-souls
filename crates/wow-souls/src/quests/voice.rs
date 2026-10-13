//! Quest voices: the Spoken Player addon's AI voices for WoW Classic's quest texts
//! (https://www.curseforge.com/wow/addons/spoken-player, by rustykey, MIT; the `SpokenQuestsAudio` packs
//! from the addon's GitHub releases). The packs name their quest files by quest id:
//! `<id>-accept.ogg` is read to the player when a quest is offered, `<id>-complete.ogg` when it is handed
//! in. They live outside the repo (1.4 GB of someone else's audio) in one root folder holding the four
//! pack folders (`SpokenQuestsAudioAlliance`, `...Horde`, `...Shared`, `...Gossip`), each with
//! `generated\sounds\quests\`: `WOWSOULS_SPOKEN=<that root>` (`0` turns the voices off), by default
//! `C:\Spel\research\spoken\x`. The first pack that has a file wins, the player's faction first (the
//! player is a Human). A voice plays while its page is open and is cut when the page changes or the
//! dialogue closes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bevy::prelude::*;

use super::dialogue::Page;
use super::Talk;
use crate::audio::Audio;

/// The root the four packs are expected under.
const DEFAULT_ROOT: &str = r"C:\Spel\research\spoken\x";
/// The packs' quest folders, in the order they are searched: the player's faction (Alliance) before the
/// quests both sides share.
const PACKS: [&str; 3] = [
    "SpokenQuestsAudioAlliance",
    "SpokenQuestsAudioShared",
    "SpokenQuestsAudioHorde",
];
/// How loud a voice is next to the sound effects (the effects are mixed under the master volume).
const VOICE_SCALE: f32 = 1.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    Accept,
    Complete,
}

impl Line {
    fn word(self) -> &'static str {
        match self {
            Line::Accept => "accept",
            Line::Complete => "complete",
        }
    }
}

/// The voice a dialogue page asks for: the offer page reads the quest, the reward page the thanks.
pub fn wanted(page: Page) -> Option<(u32, Line)> {
    match page {
        Page::Detail(q) => Some((q, Line::Accept)),
        Page::Reward(q) => Some((q, Line::Complete)),
        _ => None,
    }
}

/// The quest folders of the packs installed under `root`, in search order.
pub fn pack_dirs(root: &Path) -> Vec<PathBuf> {
    PACKS
        .iter()
        .map(|p| root.join(p).join("generated").join("sounds").join("quests"))
        .filter(|d| d.is_dir())
        .collect()
}

/// The first pack's file for a quest line, or `None` when no pack has one.
pub fn file_for(dirs: &[PathBuf], quest: u32, line: Line) -> Option<PathBuf> {
    dirs.iter()
        .map(|d| d.join(format!("{quest}-{}.ogg", line.word())))
        .find(|f| f.is_file())
}

#[derive(Resource)]
struct Voices {
    dirs: Vec<PathBuf>,
    now: Option<(u32, Line, Arc<AtomicBool>)>,
}

pub fn plugin(app: &mut App) {
    let root = match std::env::var("WOWSOULS_SPOKEN") {
        Ok(v) if v.trim() == "0" => None,
        Ok(v) => Some(PathBuf::from(v)),
        Err(_) => Some(PathBuf::from(DEFAULT_ROOT)),
    };
    let dirs = root.as_deref().map(pack_dirs).unwrap_or_default();
    if dirs.is_empty() {
        info!("voices: off (no Spoken Player packs; see WOWSOULS_SPOKEN)");
    } else {
        info!(
            "voices: Spoken Player quest voices from {} pack(s) under {}",
            dirs.len(),
            root.as_deref()
                .map_or(String::new(), |r| r.display().to_string())
        );
    }
    app.insert_resource(Voices { dirs, now: None })
        .add_systems(Update, follow.after(super::drive));
}

/// Keeps the playing voice in step with the open dialogue page.
fn follow(talk: Res<Talk>, mut voices: ResMut<Voices>, audio: Option<ResMut<Audio>>) {
    let want = talk.0.as_ref().and_then(|d| wanted(d.page));
    let playing = voices.now.as_ref().map(|(q, l, _)| (*q, *l));
    if want == playing {
        return;
    }
    if let Some((_, _, stop)) = voices.now.take() {
        stop.store(true, Ordering::Relaxed);
    }
    let (Some((quest, line)), Some(mut audio)) = (want, audio) else {
        return;
    };
    let Some(file) = file_for(&voices.dirs, quest, line) else {
        return;
    };
    if let Some(stop) = audio.play_file(
        &file,
        VOICE_SCALE,
        &format!("voice {quest} {}", line.word()),
    ) {
        voices.now = Some((quest, line, stop));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offer_and_the_reward_page_ask_for_a_voice_the_others_do_not() {
        assert_eq!(wanted(Page::Detail(7)), Some((7, Line::Accept)));
        assert_eq!(wanted(Page::Reward(7)), Some((7, Line::Complete)));
        for page in [Page::List, Page::Progress(7), Page::Nothing] {
            assert_eq!(wanted(page), None);
        }
    }

    fn pack(root: &Path, name: &str, files: &[&str]) {
        let dir = root
            .join(name)
            .join("generated")
            .join("sounds")
            .join("quests");
        std::fs::create_dir_all(&dir).unwrap();
        for f in files {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
    }

    #[test]
    fn a_file_is_found_by_quest_and_line_in_the_first_pack_that_has_it() {
        let root = std::env::temp_dir().join(format!("wowsouls-spoken-{}", std::process::id()));
        pack(
            &root,
            "SpokenQuestsAudioAlliance",
            &["7-accept.ogg", "9-accept.ogg"],
        );
        pack(
            &root,
            "SpokenQuestsAudioShared",
            &["7-accept.ogg", "7-complete.ogg"],
        );
        pack(&root, "SpokenQuestsAudioGossip", &["8-accept.ogg"]);
        let dirs = pack_dirs(&root);
        assert_eq!(
            dirs.len(),
            2,
            "Gossip holds no quest lines and is not searched"
        );
        let alliance = &dirs[0];
        assert_eq!(
            file_for(&dirs, 7, Line::Accept),
            Some(alliance.join("7-accept.ogg")),
            "the player's own faction first"
        );
        assert_eq!(
            file_for(&dirs, 7, Line::Complete),
            Some(dirs[1].join("7-complete.ogg")),
            "then what both sides share"
        );
        assert_eq!(file_for(&dirs, 8, Line::Accept), None);
        assert_eq!(pack_dirs(&root.join("nowhere")), Vec::<PathBuf>::new());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn install_a_spoken_player_ogg_decodes_to_seconds_of_speech() {
        let dirs = pack_dirs(Path::new(DEFAULT_ROOT));
        let Some(file) = file_for(&dirs, 783, Line::Accept) else {
            eprintln!("skipped: no Spoken Player packs at {DEFAULT_ROOT}");
            return;
        };
        let sound = crate::sfx::decode(std::fs::read(file).unwrap(), 48_000).unwrap();
        let seconds = sound.len() as f32 / 48_000.0;
        assert!(
            (5.0..120.0).contains(&seconds),
            "the quest text is read aloud, not a blip: {seconds:.1} s"
        );
    }

    #[test]
    fn install_the_valley_quests_have_their_voices_when_the_packs_are_there() {
        let dirs = pack_dirs(Path::new(DEFAULT_ROOT));
        if dirs.is_empty() {
            eprintln!("skipped: no Spoken Player packs at {DEFAULT_ROOT}");
            return;
        }
        for q in [783, 7, 15, 21, 33, 5261, 3100, 18, 6] {
            assert!(file_for(&dirs, q, Line::Accept).is_some(), "{q} accept");
            assert!(file_for(&dirs, q, Line::Complete).is_some(), "{q} complete");
        }
    }
}
