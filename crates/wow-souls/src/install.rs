//! Where the player's WoW 1.12.1 client is. Picked once in a folder dialog, kept in
//! `%LOCALAPPDATA%\WoWSouls\wow-folder.txt`, and handed to the engine as `WOW_DATA` before the app
//! is built (benilla reads `WOW_DATA` first). A `WOW_DATA` set by hand still wins; `--choose-wow`
//! asks again. Unattended runs never open a dialog.
//!
//! Then the souls animations ([`animations`]): built once on this machine from the player's own Dark
//! Souls Remastered by the separate `wow-souls-dsr setup`, into `%LOCALAPPDATA%\WoWSouls\anim`.

use std::path::{Path, PathBuf};

/// The archives every 1.12 client's `Data` folder has. `dbc.MPQ` is gone from 2.0 on, so it tells a
/// vanilla client from a later one.
const VANILLA_ARCHIVES: [&str; 3] = ["dbc.mpq", "model.mpq", "terrain.mpq"];

/// The saved choice: the client's `Data` folder, one line.
fn saved_path() -> PathBuf {
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "saves".into()))
        .join("WoWSouls/wow-folder.txt")
}

/// Settles `WOW_DATA` for this process: the hand-set value, else what benilla finds on its own (a
/// dev build's `WoW` junction, a client beside the exe), else the saved folder, else the player's
/// pick. Returns whether a 1.12.1 client was found.
pub fn choose(unattended: bool) -> bool {
    if std::env::var_os("WOW_DATA").is_some() {
        return found();
    }
    let ask_again = std::env::args().any(|a| a == "--choose-wow");
    if !ask_again {
        if found() {
            return true;
        }
        if let Some(data) = saved().and_then(|d| data_dir(&d)) {
            set(&data);
            return true;
        }
    }
    if unattended {
        return found();
    }
    loop {
        let Some(picked) = rfd::FileDialog::new()
            .set_title("WoW Souls: choose your World of Warcraft 1.12.1 folder")
            .pick_folder()
        else {
            return found();
        };
        if let Some(data) = data_dir(&picked) {
            save(&data);
            set(&data);
            return true;
        }
        let again = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Not a WoW 1.12.1 folder")
            .set_description(format!(
                "{}\n\nhas no 1.12.1 game data (Data\\dbc.MPQ, model.MPQ, terrain.MPQ).\n\
                 Choose the folder that holds WoW.exe and the Data folder.",
                picked.display()
            ))
            .set_buttons(rfd::MessageButtons::OkCancel)
            .show();
        if again != rfd::MessageDialogResult::Ok {
            return found();
        }
    }
}

/// Whether benilla's own lookup lands on a 1.12.1 `Data` folder. Its last guess is `Data` beside
/// the exe, which on Windows also matches a packaged game's own `data` folder, so it is checked.
fn found() -> bool {
    benilla_formats::wow_data()
        .and_then(|d| data_dir(&d))
        .is_some()
}

/// Tells the player the game cannot start without a client.
pub fn report_missing() {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("WoW Souls needs World of Warcraft 1.12.1")
        .set_description(
            "No WoW 1.12.1 client was chosen. The game reads its world, models and sounds from \
             your own installation.\n\nStart the game again and choose the folder that holds \
             WoW.exe, or set WOW_DATA to its Data folder.",
        )
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// The client's `Data` folder for a picked folder: the folder itself or its `Data` child, if it
/// holds the vanilla archives. Names compare without case (a client copied from a Mac or Linux
/// box may say `Data` or `data`, `dbc.MPQ` or `DBC.MPQ`).
pub fn data_dir(picked: &Path) -> Option<PathBuf> {
    if has_vanilla_archives(picked) {
        return Some(picked.to_path_buf());
    }
    std::fs::read_dir(picked)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.is_dir()
                && p.file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("data"))
        })
        .filter(|data| has_vanilla_archives(data))
}

fn has_vanilla_archives(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_ascii_lowercase())
        .collect();
    VANILLA_ARCHIVES
        .iter()
        .all(|a| names.iter().any(|n| n == a))
}

fn saved() -> Option<PathBuf> {
    let text = std::fs::read_to_string(saved_path()).ok()?;
    let line = text.lines().next()?.trim();
    (!line.is_empty()).then(|| PathBuf::from(line))
}

fn save(data: &Path) {
    let path = saved_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&path, format!("{}\n", data.display())) {
        eprintln!(
            "wow-souls: could not save the WoW folder to {} ({e})",
            path.display()
        );
    }
}

/// Before the app exists, so no other thread reads the environment yet.
fn set(data: &Path) {
    std::env::set_var("WOW_DATA", data);
}

fn saves_dir() -> PathBuf {
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "saves".into())).join("WoWSouls")
}

/// The animation library built on this machine (`wow-souls-dsr setup`), used when the project's
/// `anim/source` holds only the tables.
pub fn local_anim_dir() -> PathBuf {
    saves_dir().join("anim")
}

/// The last file `wow-souls-dsr setup` writes into the library (its `setup::COMPLETE`).
const ANIM_BUILT: &str = "built.txt";

/// Builds the souls animations from the player's own Dark Souls Remastered, once: when the project has
/// no clips of its own and none were built here yet. The animations are what the game is built
/// around, so there is nothing to decline: the dialog explains and asks for the folder, and a first
/// start without them ends in [`report_animations_required`]. `--setup-animations` builds them again
/// (a failed rebuild keeps the old ones). The builder is a separate program, `wow-souls-dsr.exe`
/// (beside the game, or built in `crates/dsr-cache`), so its GPL code stays out of the game; it
/// checks every clip against the developer's hash before the game uses the folder. Returns whether
/// the animations are in place.
pub fn animations(unattended: bool) -> bool {
    let ask_again = std::env::args().any(|a| a == "--setup-animations");
    if unattended || std::env::var_os("WOWSOULS_ANIM_DIR").is_some() {
        return true;
    }
    let project = wow_world::project_path("anim/source");
    let local = local_anim_dir();
    if crate::animation_library::has_clips(&project) {
        return true;
    }
    let built = local.join(ANIM_BUILT).is_file();
    if !ask_again && built {
        return true;
    }
    // An older version let the player decline; that mark no longer counts.
    let _ = std::fs::remove_file(saves_dir().join("anim-skip.txt"));
    message(
        rfd::MessageLevel::Info,
        "WoW Souls: combat animations",
        "WoW Souls is built around Dark Souls' combat. It builds its animations (the roll, the          attacks, the guard, every creature's moves) from your own copy of Dark Souls Remastered,          which you need to play. Nothing is downloaded and your game folder is only read; it takes          about half a minute.

Next, choose your Dark Souls Remastered folder (in Steam:          right-click the game, Manage, Browse local files).",
    );
    let Some(dsr) = pick_dsr() else {
        return built;
    };
    let Some(builder) = builder() else {
        message(
            rfd::MessageLevel::Error,
            "Animation builder missing",
            "wow-souls-dsr.exe was not found beside the game. Build it with

             cargo build --release --manifest-path crates/dsr-cache/Cargo.toml

             (Play.bat does this), then start the game again.",
        );
        return built;
    };
    let (Some(wow), Some(root)) = (
        benilla_formats::wow_data(),
        wow_world::project_path("anim/provenance/clip-sources.tsv")
            .ancestors()
            .nth(3)
            .map(Path::to_path_buf),
    ) else {
        return built;
    };
    let log = saves_dir().join("anim-setup.log");
    let ran = std::fs::File::create(&log).and_then(|out| {
        let err = out.try_clone()?;
        let mut cmd = std::process::Command::new(&builder);
        cmd.arg("setup")
            .arg("--dsr")
            .arg(&dsr)
            .arg("--wow")
            .arg(&wow)
            .arg("--project")
            .arg(&root)
            .arg("--out")
            .arg(&local)
            .stdout(out)
            .stderr(err);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: no console flashes up behind the game.
            cmd.creation_flags(0x0800_0000);
        }
        cmd.status()
    });
    match ran {
        Ok(status) if status.success() && local.join(ANIM_BUILT).is_file() => true,
        other => {
            let why = match other {
                Ok(status) => format!("the builder stopped ({status})"),
                Err(e) => format!("the builder could not start ({e})"),
            };
            let tail: String = std::fs::read_to_string(&log)
                .unwrap_or_default()
                .lines()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            message(
                rfd::MessageLevel::Error,
                "Animations not built",
                &format!(
                    "{why}.\n\n{tail}\n\nFull log: {}\n\nStart the game again to retry.",
                    log.display()
                ),
            );
            built
        }
    }
}

/// Tells the player the game cannot start without the animations.
pub fn report_animations_required() {
    message(
        rfd::MessageLevel::Error,
        "WoW Souls needs Dark Souls Remastered",
        "The combat animations were not built. They come from your own copy of Dark Souls \
         Remastered, which the game needs.\n\nStart the game again and choose the folder that \
         holds DarkSoulsRemastered.exe (in Steam: right-click the game, Manage, Browse local files).",
    );
}

/// The player's DSR folder: the one holding `DarkSoulsRemastered.exe` and `chr\c0000.anibnd.dcx`.
fn pick_dsr() -> Option<PathBuf> {
    loop {
        let picked = rfd::FileDialog::new()
            .set_title("WoW Souls: choose your Dark Souls Remastered folder")
            .pick_folder()?;
        if picked.join("chr").join("c0000.anibnd.dcx").is_file() {
            return Some(picked);
        }
        let again = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Not a Dark Souls Remastered folder")
            .set_description(format!(
                "{}\n\nhas no chr\\c0000.anibnd.dcx. Choose the folder that holds \
                 DarkSoulsRemastered.exe (in Steam: right-click the game, Manage, Browse local files).",
                picked.display()
            ))
            .set_buttons(rfd::MessageButtons::OkCancel)
            .show();
        if again != rfd::MessageDialogResult::Ok {
            return None;
        }
    }
}

/// `wow-souls-dsr.exe` beside the game, else where a checkout builds it.
fn builder() -> Option<PathBuf> {
    let name = format!("wow-souls-dsr{}", std::env::consts::EXE_SUFFIX);
    let beside = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join(&name)));
    beside
        .into_iter()
        .chain(
            ["release", "debug"]
                .map(|p| wow_world::project_path(&format!("crates/dsr-cache/target/{p}/{name}"))),
        )
        .find(|p| p.is_file())
}

fn message(level: rfd::MessageLevel, title: &str, text: &str) {
    rfd::MessageDialog::new()
        .set_level(level)
        .set_title(title)
        .set_description(text)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wowsouls-install-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(dir: &Path, names: &[&str]) {
        std::fs::create_dir_all(dir).unwrap();
        for n in names {
            std::fs::write(dir.join(n), b"").unwrap();
        }
    }

    #[test]
    fn the_client_folder_and_its_data_folder_both_resolve_to_data() {
        let root = scratch("client");
        let data = root.join("Data");
        touch(&data, &["dbc.MPQ", "model.MPQ", "terrain.MPQ", "patch.MPQ"]);
        assert_eq!(data_dir(&root), Some(data.clone()));
        assert_eq!(data_dir(&data), Some(data));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn names_compare_without_case() {
        let root = scratch("case");
        touch(&root.join("data"), &["DBC.MPQ", "Model.mpq", "TERRAIN.MPQ"]);
        assert_eq!(data_dir(&root), Some(root.join("data")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_later_client_without_dbc_mpq_is_refused() {
        let root = scratch("tbc");
        touch(
            &root.join("Data"),
            &["common.MPQ", "expansion.MPQ", "model.MPQ", "terrain.MPQ"],
        );
        assert_eq!(data_dir(&root), None);
        assert_eq!(data_dir(&root.join("missing")), None);
        let _ = std::fs::remove_dir_all(root);
    }
}
