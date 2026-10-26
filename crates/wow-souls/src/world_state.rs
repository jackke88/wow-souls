//! The world's memory between runs: `WorldState` (the pure text save in `wow_world::world_state`) as a
//! resource, loaded when the plugin is built and saved when it changes. The file is
//! `world-state-v1.txt` beside the other saves (`WOWSOULS_WORLD_STATE=<file>` names another); an
//! unattended run without that switch keeps the state in memory only, so a test never kills the
//! player's boss.

use std::path::PathBuf;

use bevy::prelude::*;
use wow_world::world_state::WorldState;

#[derive(Resource)]
pub struct WorldStateFile {
    pub state: WorldState,
    path: Option<PathBuf>,
    dirty: bool,
}

impl WorldStateFile {
    /// Read `path` (a missing file is an empty state; a file that cannot be read keeps its rows that
    /// could be and is not written over until something changes).
    pub fn load(path: Option<PathBuf>) -> WorldStateFile {
        let state = match path.as_ref().map(std::fs::read_to_string) {
            Some(Ok(text)) => {
                let (s, skipped) = WorldState::parse(&text);
                for line in skipped {
                    warn!("world state: skipped {line:?}");
                }
                s
            }
            _ => WorldState::default(),
        };
        WorldStateFile {
            state,
            path,
            dirty: false,
        }
    }

    #[cfg(test)]
    pub fn in_memory() -> WorldStateFile {
        WorldStateFile::load(None)
    }

    /// Something changed: write at the end of the frame.
    pub fn mark(&mut self) {
        self.dirty = true;
    }
}

/// The file's name beside the other saves (a character's folder holds it with them).
pub const FILE_NAME: &str = "world-state-v1.txt";

/// `WOWSOULS_WORLD_STATE`, else beside the other saves; `None` for an unattended run with no switch.
pub fn save_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("WOWSOULS_WORLD_STATE") {
        return Some(PathBuf::from(p));
    }
    if crate::run_mode::unattended() {
        return None;
    }
    Some(crate::saves::file(FILE_NAME))
}

fn save(mut file: ResMut<WorldStateFile>) {
    if !file.dirty {
        return;
    }
    file.dirty = false;
    let Some(path) = file.path.clone() else {
        return;
    };
    let text = file.state.write();
    let tmp = path.with_extension("tmp");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, &path)) {
        warn!("world state: could not save {}: {e}", path.display());
    }
}

/// Puts every held item into the pack and forgets the held list. An item list that does not fit stays
/// held. Returns what was given.
pub fn deliver_held(
    progress: &mut wow_world::inventory::Progress,
    state: &mut WorldState,
) -> Vec<(u32, u32)> {
    let mut given = Vec::new();
    let guids: Vec<u32> = state.held.keys().copied().collect();
    for guid in guids {
        let items = state.held.get(&guid).cloned().unwrap_or_default();
        let mut next = progress.clone();
        if next.inventory.add(&items).is_ok() {
            *progress = next;
            given.extend(items);
            state.held.remove(&guid);
        }
    }
    given
}

/// At start: spoils a boss still carried when its corpse was lost (a restart) are in the pack now.
fn deliver_at_start(mut file: ResMut<WorldStateFile>, owned: Option<ResMut<crate::loot::Owned>>) {
    let Some(mut owned) = owned else { return };
    if file.state.held.is_empty() {
        return;
    }
    let mut next = owned.progress.clone();
    let given = deliver_held(&mut next, &mut file.state);
    if given.is_empty() {
        return;
    }
    match owned.publish(next) {
        Ok(()) => {
            info!(
                "world state: {} item(s) from an earlier kill are in your pack: {given:?}",
                given.len()
            );
            file.mark();
        }
        Err(e) => warn!("world state: could not deliver held loot: {e}"),
    }
}

pub fn plugin(app: &mut App) {
    let file = WorldStateFile::load(save_path());
    info!(
        "world state: {} slain, {} first kills, {} held loots",
        file.state.slain.len(),
        file.state.first.len(),
        file.state.held.len()
    );
    app.insert_resource(file)
        .add_systems(Startup, deliver_at_start.after(crate::loot::load))
        .add_systems(Last, save);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_file_written_by_one_run_is_read_by_the_next() {
        let dir = std::env::temp_dir().join(format!("wssave-{}", std::process::id()));
        let path = dir.join("world-state-v1.txt");
        let mut a = WorldStateFile::load(Some(path.clone()));
        a.state.slain.insert(80247);
        a.state.first.insert(103);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, a.state.write()).unwrap();
        let b = WorldStateFile::load(Some(path));
        assert!(b.state.slain.contains(&80247) && b.state.first.contains(&103));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_or_foreign_file_is_an_empty_state() {
        let none = WorldStateFile::load(Some(PathBuf::from("Z:/no/such/file.txt")));
        assert!(none.state.slain.is_empty());
        assert!(WorldStateFile::in_memory().state.first.is_empty());
    }

    #[test]
    fn held_loot_from_an_earlier_run_goes_to_the_pack_and_is_forgotten() {
        use wow_world::inventory::Progress;
        let mut progress = Progress::default();
        let mut state = wow_world::world_state::WorldState::default();
        state.held.insert(80247, vec![(2057, 1), (182, 1)]);
        state.held.insert(9, vec![(159, 3)]);
        let given = deliver_held(&mut progress, &mut state);
        assert_eq!(given.len(), 3);
        assert_eq!(progress.inventory.count(2057), 1);
        assert_eq!(progress.inventory.count(159), 3);
        assert!(state.held.is_empty(), "delivered, so forgotten");
    }
}
