//! One explicit graveyard binding per character. Graveyards come from the DB's Spirit Healer
//! spawns; their GUIDs survive imports. The healer stays hidden and a memorial marks its place.
//! Save before publishing a binding: a failed write must never complete the tutorial.

mod art;
pub(crate) use art::rune_texture;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use benilla_assets::coords::wow_to_bevy;
use bevy::prelude::*;
use wow_world::model::World;

use crate::avatar::Avatar;
use crate::combat::{Action, Health};
use crate::motor::Motor;
use crate::npc::{Hostile, Reaction};
use crate::quests::Toasts;
use crate::settle::Phase;

pub const FILE_NAME: &str = "soulbind-v1.txt";
pub const RANGE: f32 = 3.5;
const HEIGHT: f32 = 3.0;

/// The grounded stone root, shared with the tutorial's native WoW overhead sign.
#[derive(Component)]
pub(crate) struct Memorial(pub u32);

#[derive(Debug, Clone)]
pub struct Site {
    pub id: u32,
    pub name: String,
    pub at: Vec3,
    pub yaw: f32,
}

impl Site {
    /// Stand in front of the memorial rather than inside its stonework.
    pub fn arrival(&self) -> Vec3 {
        self.at + Quat::from_rotation_y(self.yaw) * Vec3::Z * 2.2
    }

    pub fn facing(&self) -> Quat {
        Quat::from_rotation_y(self.yaw)
    }
}

pub fn sites(world: &World) -> Vec<Site> {
    world
        .spawns
        .iter()
        .filter(|s| s.entry == crate::friendly::SPIRIT_HEALER && !s.event)
        .filter(|s| s.pos.iter().all(|v| v.is_finite()) && s.orientation.is_finite())
        .map(|s| Site {
            id: s.guid,
            name: if s.area == 12 {
                "Goldshire".into()
            } else {
                world
                    .areas
                    .iter()
                    .find(|(id, _)| *id == s.area)
                    .map_or("Graveyard", |(_, name)| name.as_str())
                    .to_string()
            },
            at: wow_to_bevy(s.pos),
            // WoW +X faces Bevy -Z, with positive orientation turning west.
            yaw: s.orientation,
        })
        .collect()
}

pub fn nearest(sites: &[Site], feet: Vec3) -> Option<&Site> {
    sites
        .iter()
        .filter_map(|s| {
            let distance = Vec2::new(feet.x - s.at.x, feet.z - s.at.z).length();
            (distance <= RANGE && (feet.y - s.at.y).abs() <= HEIGHT).then_some((distance, s))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, s)| s)
}

/// The save: the active graveyard (where a new session opens) and every graveyard the character has
/// bound at (`wssoulbind 2`); death returns to the nearest of those. `wssoulbind 1` (one site) still reads.
pub fn to_text(site: u32, known: &[u32]) -> String {
    let known: Vec<String> = known.iter().map(u32::to_string).collect();
    format!("wssoulbind 2\nsite {site}\nknown {}\n", known.join(" "))
}

fn guid(value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| "invalid graveyard GUID".into())
}

/// Reads [`to_text`]: the active site and the bound ones (the active one always among them).
pub fn from_text(text: &str) -> Result<(u32, Vec<u32>), String> {
    let mut lines = text.lines();
    let version = match lines.next() {
        Some("wssoulbind 1") => 1,
        Some("wssoulbind 2") => 2,
        _ => return Err("expected wssoulbind 1 or 2".into()),
    };
    let words: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
    let ["site", value] = words.as_slice() else {
        return Err("expected site <graveyard GUID>".into());
    };
    let id = guid(value)?;
    let mut known = vec![id];
    if version == 2 {
        let words: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
        let Some((&"known", rest)) = words.split_first() else {
            return Err("expected known <graveyard GUIDs>".into());
        };
        for value in rest {
            let other = guid(value)?;
            if !known.contains(&other) {
                known.push(other);
            }
        }
    }
    if lines.next().is_some() {
        return Err("unexpected lines after the binding".into());
    }
    Ok((id, known))
}

#[derive(Resource, Default)]
pub struct Binding {
    pub sites: Vec<Site>,
    pub bound: Option<u32>,
    /// Every graveyard bound at (the active one too): death returns to the nearest of them.
    pub known: Vec<u32>,
    path: PathBuf,
    blocked: Option<String>,
}

impl Binding {
    pub fn site(&self) -> Option<&Site> {
        self.sites.iter().find(|s| Some(s.id) == self.bound)
    }

    /// Where death at `feet` returns the player (2026-10-18, the user: "ifall man dör närmre en så
    /// spawnas man där"): the nearest graveyard they have bound at, else the active one.
    pub fn respawn_site(&self, feet: Vec3) -> Option<&Site> {
        self.sites
            .iter()
            .filter(|s| self.known.contains(&s.id) || Some(s.id) == self.bound)
            .min_by(|a, b| {
                a.at.distance_squared(feet)
                    .total_cmp(&b.at.distance_squared(feet))
            })
            .or_else(|| self.site())
    }

    fn read(path: PathBuf, sites: Vec<Site>) -> Self {
        let mut state = Self {
            path,
            sites,
            ..default()
        };
        match std::fs::read_to_string(&state.path) {
            Ok(text) => match from_text(&text) {
                Ok((id, known)) if state.sites.iter().any(|s| s.id == id) => {
                    state.bound = Some(id);
                    // A graveyard gone from the world is dropped from the list, not an error.
                    state.known = known
                        .into_iter()
                        .filter(|k| state.sites.iter().any(|s| s.id == *k))
                        .collect();
                }
                Ok((id, _)) => {
                    state.blocked = Some(format!("graveyard {id} is absent from the world"))
                }
                Err(e) => state.blocked = Some(e),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => state.blocked = Some(e.to_string()),
        }
        state
    }

    fn bind(&mut self, id: u32) -> Result<(), String> {
        if self.blocked.is_some() {
            return Err("The saved soul binding could not be read. It has been preserved.".into());
        }
        if !self.sites.iter().any(|s| s.id == id) {
            return Err("This graveyard is unavailable.".into());
        }
        if self.bound == Some(id) {
            return Ok(());
        }
        let mut known = self.known.clone();
        if !known.contains(&id) {
            known.push(id);
        }
        write(&self.path, &to_text(id, &known))
            .map_err(|e| format!("Could not save your soul binding: {e}"))?;
        self.bound = Some(id);
        self.known = known;
        Ok(())
    }
}

fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

/// The interaction reader requests a real nearby binding, never a teleport or quest credit.
#[derive(Message)]
pub struct BindRequest(pub u32);

/// Published only after the new active site has been saved successfully.
#[derive(Message, Debug, Clone, Copy)]
pub struct SoulBound {
    pub site: u32,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Apply;

pub fn plugin(app: &mut App) {
    app.init_resource::<Binding>()
        .add_message::<BindRequest>()
        .add_message::<SoulBound>()
        .add_systems(Startup, load)
        .add_systems(
            Update,
            (apply, crate::quests::soul_tutorial::sync)
                .chain()
                .in_set(Apply)
                .run_if(in_state(Phase::Playing)),
        )
        .add_plugins(art::plugin);
}

pub(crate) fn load(mut binding: ResMut<Binding>) {
    let sites = std::fs::read_to_string(crate::world::manifest_path())
        .ok()
        .and_then(|text| wow_world::manifest::parse(&text).ok())
        .map(|w| sites(&w))
        .unwrap_or_default();
    let path = std::env::var_os("WOWSOULS_BIND_SAVE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if crate::run_mode::unattended() {
                PathBuf::from(format!(
                    "cache/loot-check/soulbind-{}.txt",
                    std::process::id()
                ))
            } else {
                crate::saves::file(FILE_NAME)
            }
        });
    *binding = Binding::read(path, sites);
    if let Some(why) = &binding.blocked {
        warn!(
            "soulbind: cannot read {} ({why}); preserving the file",
            binding.path.display()
        );
    }
    info!(
        "soulbind: {} graveyards, active {:?}, save {}",
        binding.sites.len(),
        binding.bound,
        binding.path.display()
    );
}

fn apply(
    mut requests: MessageReader<BindRequest>,
    mut successes: MessageWriter<SoulBound>,
    mut binding: ResMut<Binding>,
    real: Res<Time<Real>>,
    mut toasts: ResMut<Toasts>,
    avatar: Query<(&Transform, &Action, &Health, &Motor), With<Avatar>>,
    foes: Query<(&Hostile, &Reaction)>,
) {
    for request in requests.read() {
        let Ok((me, action, hp, motor)) = avatar.single() else {
            continue;
        };
        if hp.cur <= 0.0 || action.owns_body() || !motor.grounded {
            continue;
        }
        if nearest(&binding.sites, me.translation).is_none_or(|s| s.id != request.0) {
            continue;
        }
        let now = real.elapsed_secs();
        if foes
            .iter()
            .any(|(foe, reaction)| foe.alert && *reaction != Reaction::Dead)
        {
            toasts.push(now, "You cannot bind your soul while in combat.");
            continue;
        }
        match binding.bind(request.0) {
            Ok(()) => {
                let name = &binding.site().unwrap().name;
                toasts.push(
                    now,
                    format!("Bound to {name}. Death returns you to the nearest graveyard you have bound."),
                );
                successes.write(SoulBound { site: request.0 });
                info!("soulbind: bound to {name} ({})", request.0);
            }
            Err(e) => {
                warn!("soulbind: {e}");
                toasts.push(now, e);
            }
        }
    }
}
