//! Per-character Warrior talents and their native menu. Invalid saves remain untouched.
//! `WOWSOULS_TALENTS_SAVE` overrides the file; unattended runs default to an isolated cache file.
//! `WOWSOULS_TALENTS_SCRIPT=2:open,3:spend:tactical-mastery,4:reset,5:confirm` uses real time.

mod ui;

use std::path::PathBuf;

use bevy::prelude::*;
pub use wow_world::talents::{Build, TalentId, Tree};

pub const FILE_NAME: &str = "talents-v1.txt";

#[derive(Resource, Debug, Default)]
pub struct Talents {
    pub build: Build,
    class: u8,
    loaded: bool,
    path: PathBuf,
    blocked: Option<String>,
    budget_seen: u8,
}

impl Talents {
    #[cfg(test)]
    pub(crate) fn for_build(build: Build) -> Self {
        Self {
            build,
            class: wow_world::talents::WARRIOR,
            loaded: true,
            ..default()
        }
    }

    pub fn rank(&self, id: TalentId) -> u8 {
        if self.class == wow_world::talents::WARRIOR {
            self.build.rank(id)
        } else {
            0
        }
    }

    pub fn signature(&self) -> Option<u32> {
        (self.class == wow_world::talents::WARRIOR)
            .then(|| self.build.signature())
            .flatten()
    }

    fn read(path: PathBuf, class: u8, level: u8) -> Self {
        let mut talents = Self {
            path,
            class,
            loaded: true,
            budget_seen: Build::budget(level),
            ..default()
        };
        match std::fs::read_to_string(&talents.path) {
            Ok(text) => match Build::parse(&text, level).and_then(|build| {
                build.validate(class, level)?;
                Ok(build)
            }) {
                Ok(build) => talents.build = build,
                Err(error) => talents.blocked = Some(error),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => talents.blocked = Some(error.to_string()),
        }
        talents
    }

    fn advance_budget(&mut self, level: u8) -> u8 {
        let budget = Build::budget(level);
        let gained = budget.saturating_sub(self.budget_seen);
        self.budget_seen = budget;
        gained
    }

    /// Persist first. A refused or failed write never grants a talent in memory.
    fn replace(&mut self, next: Build, level: u8) -> Result<(), String> {
        if self.blocked.is_some() {
            return Err(
                "Saved talents could not be read. The original file has been preserved.".into(),
            );
        }
        next.validate(self.class, level)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Could not save talents: {e}"))?;
        }
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, next.text())
            .and_then(|()| std::fs::rename(&tmp, &self.path))
            .map_err(|e| format!("Could not save talents: {e}"))?;
        self.build = next;
        Ok(())
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Talents>()
        // QuestGame is inserted by Startup; PreUpdate reads the actual loaded class/level.
        .add_systems(PreUpdate, follow)
        .add_plugins(ui::plugin);
}

fn save_path() -> PathBuf {
    std::env::var_os("WOWSOULS_TALENTS_SAVE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if crate::run_mode::unattended() {
                PathBuf::from(format!(
                    "cache/talents-check/talents-{}.txt",
                    std::process::id()
                ))
            } else {
                crate::saves::file(FILE_NAME)
            }
        })
}

fn follow(
    game: Option<Res<crate::quests::QuestGame>>,
    mut talents: ResMut<Talents>,
    mut toasts: Option<ResMut<crate::quests::Toasts>>,
    real: Res<Time<bevy::time::Real>>,
) {
    let Some(game) = game else {
        return;
    };
    let class = game.player.class as u8;
    let level = game.player.level as u8;
    if !talents.loaded {
        *talents = Talents::read(save_path(), class, level);
        if let Some(why) = &talents.blocked {
            warn!(
                "talents: {}: {why}; preserving unreadable save",
                talents.path.display()
            );
        } else {
            info!(
                "talents: {} points spent, {} available at level {level}",
                talents.build.spent(),
                talents.build.available(level)
            );
        }
    }
    let gained = talents.advance_budget(level);
    if class == wow_world::talents::WARRIOR && gained > 0 {
        if let Some(toasts) = toasts.as_deref_mut() {
            toasts.push(
                real.elapsed_secs(),
                format!(
                    "{} talent point{} earned — Menu > Talents ({} available)",
                    gained,
                    if gained == 1 { "" } else { "s" },
                    talents.build.available(level)
                ),
            );
        }
    }
    talents.class = class;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_discovery_is_for_leveling_and_skips_initial_load() {
        let path = std::env::temp_dir().join(format!(
            "wowsouls-talents-levels-{}.txt",
            std::process::id()
        ));
        let mut state = Talents::read(path, 1, 9);
        assert_eq!(state.advance_budget(9), 0);
        assert_eq!(state.advance_budget(10), 1);
        assert_eq!(state.advance_budget(10), 0);
        assert_eq!(state.advance_budget(60), 50);
        assert_eq!(state.advance_budget(61), 0);
        let path = std::env::temp_dir().join(format!(
            "wowsouls-talents-levels-high-{}.txt",
            std::process::id()
        ));
        assert_eq!(Talents::read(path, 1, 60).advance_budget(60), 0);
    }

    #[test]
    fn corrupt_save_is_preserved_and_has_no_effects() {
        let path = std::env::temp_dir().join(format!(
            "wowsouls-talents-corrupt-{}.txt",
            std::process::id()
        ));
        let source = "wstalents 1\nclass 1\nshield-wall 5\n";
        std::fs::write(&path, source).unwrap();
        let mut state = Talents::read(path.clone(), 1, 60);
        assert_eq!(state.rank(TalentId::ShieldWall), 0);
        assert!(state.replace(Build::default(), 60).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn save_load_and_reset_use_the_same_character_file() {
        let path = std::env::temp_dir().join(format!(
            "wowsouls-talents-roundtrip-{}.txt",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut state = Talents::read(path.clone(), 1, 10);
        let mut build = state.build.clone();
        build.spend(TalentId::TacticalMastery, 10).unwrap();
        state.replace(build, 10).unwrap();
        assert_eq!(
            Talents::read(path.clone(), 1, 10).rank(TalentId::TacticalMastery),
            1
        );
        assert_eq!(
            Talents::read(path.clone(), 8, 10).rank(TalentId::TacticalMastery),
            0
        );
        assert_eq!(
            Talents::read(path.clone(), 1, 9).rank(TalentId::TacticalMastery),
            0
        );
        state.replace(Build::default(), 10).unwrap();
        assert_eq!(Talents::read(path.clone(), 1, 10).build.spent(), 0);
        std::fs::remove_file(path).unwrap();
    }
}
