//! Northshire Valley as WoW Classic has it: creatures, spawns, NPCs, quests, items and objects,
//! read from the vMaNGOS world database (exported by `tools/export-mangos.ps1`) and written as one
//! manifest (`wsworld 4`, with backwards-compatible reading of versions 1–3) that the game loads. The quest
//! rules (`quest`) are a pure engine over that data.

pub mod abilities;
pub mod attributes;
pub mod balance;
pub mod class;
pub mod db;
pub mod equipment;
pub mod gear_effect;
pub mod inventory;
pub mod loot;
pub mod manifest;
pub mod merge;
pub mod model;
pub mod pool;
pub mod quest;
pub mod select;
pub mod shop;
pub mod spells;
pub mod talents;
pub mod torch;
pub mod tsv;
pub mod valley;
pub mod village;
pub mod world_state;

use std::path::PathBuf;

/// Locate packaged project data independently of the launcher's working directory.
/// An explicit data root wins, then the executable's own data, then a development
/// checkout ancestor. The player build embeds no developer-machine source path.
pub fn project_path(relative: &str) -> PathBuf {
    if let Some(root) = std::env::var_os("WOWSOULS_DATA_DIR") {
        return PathBuf::from(root).join(relative);
    }
    let executable = std::env::current_exe().ok();
    let beside = executable.as_deref().and_then(std::path::Path::parent);
    if let Some(root) = beside.filter(|root| root.join(relative).exists()) {
        return root.join(relative);
    }
    let current = std::env::current_dir().ok();
    if let Some(root) = current
        .as_deref()
        .and_then(|cwd| cwd.ancestors().find(|root| root.join(relative).exists()))
    {
        return root.join(relative);
    }
    beside
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(relative)
}

/// The same packaged world for spawns, quests and loot. New imports need an explicit override.
pub fn default_manifest_path() -> PathBuf {
    if let Some(path) = std::env::var_os("WOWSOULS_WORLD_FILE") {
        return path.into();
    }
    project_path("data/world/elwynn-westfall.txt")
}
