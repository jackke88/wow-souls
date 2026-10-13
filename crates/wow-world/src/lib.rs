//! Northshire Valley as WoW Classic has it: creatures, spawns, NPCs, quests, items and objects,
//! read from the vMaNGOS world database (exported by `tools/export-mangos.ps1`) and written as one
//! manifest (`wsworld 2`, which the reader also takes as `wsworld 1`) that the game loads. The quest
//! rules (`quest`) are a pure engine over that data.

pub mod abilities;
pub mod attributes;
pub mod db;
pub mod equipment;
pub mod gear_effect;
pub mod inventory;
pub mod loot;
pub mod manifest;
pub mod model;
pub mod quest;
pub mod select;
pub mod shop;
pub mod tsv;
pub mod valley;
pub mod world_state;

use std::path::PathBuf;

/// The manifest to read: `cache/world/northshire.next.txt` when it exists (the re-import with the quest
/// engine's columns, written beside the live file until the game is rebuilt on it), else
/// `cache/world/northshire.txt`.
pub fn default_manifest_path() -> PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cache/world");
    let next = dir.join("northshire.next.txt");
    if next.exists() {
        next
    } else {
        dir.join("northshire.txt")
    }
}
