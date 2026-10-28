//! `WOWSOULS_TOUR=1`: a teleporter for showing the creatures off (a film, a screenshot session).
//! A list of stops, one per hostile species of the world book: the rares and named quest mobs first
//! (Garrick to Hogger, the order of the bestiary film), then the other quest creatures by level.
//!
//! PageDown: the next stop. PageUp: the previous. Home: the first. End: the same stop again.
//!
//! A jump puts the creature of the stop back to life where it stands (a slain rare, a spot the
//! density thinned out, a hurt creature all start fresh), stands the player [`STAND_OFF`] yd in front of
//! it facing it, heals the body, waits for the ground there to stream in and then hands the body
//! back. The stop also becomes the spawn point, so a death returns to the same creature. A toast names
//! the stop. Only a person playing can press the keys (never an unattended run); a check or a recording
//! drives the same jumps with `WOWSOULS_TOUR_KEYS=<real s>:<next|prev|first|again>,...`.

use bevy::prelude::*;
use bevy::time::Real;

use benilla_assets::coords::wow_to_bevy;
use benilla_world::terrain_stream::WorldLoadProgress;

use crate::avatar::Avatar;
use crate::combat::{Action, Health, Stamina};
use crate::controller::{PlaySet, SpawnPoint};
use crate::hurt::Death;
use crate::monster::Poise;
use crate::motor::{Motor, MoveIntent};
use crate::npc::BodyCollider;
use crate::settle::Phase;
use crate::world::{Kind, WorldBook};

/// The rares and named quest mobs in the order of the bestiary film; Hogger, the zone boss, last.
pub const RARES: [&str; 16] = [
    "garrick",
    "goldtooth",
    "ruklar",
    "erlan",
    "princess",
    "narg",
    "morgaine",
    "morgan",
    "mother_fang",
    "surena",
    "dead_tooth_jack",
    "thuros",
    "james_clark",
    "gruff",
    "fedfennel",
    "hogger",
];

/// How far in front of the creature (yd, the way it faces) the player stands.
pub const STAND_OFF: f32 = 7.0;
/// Stops whose creature stands in a small room get a shorter stand-off.
const CLOSE: [(&str, f32); 1] = [("james_clark", 3.5)];
/// A body held for ground that never streams is released after this long (real s).
const GIVE_UP: f32 = 10.0;

/// One stop: the species, the spot of the book it uses and how far in front of it the player stands.
#[derive(Debug, Clone, PartialEq)]
pub struct Stop {
    pub name: &'static str,
    pub spot: usize,
    pub rare: bool,
    pub stand_off: f32,
}

/// The stops, where the tour is and the jump that waits for ground.
#[derive(Resource, Default)]
pub struct Tour {
    pub stops: Vec<Stop>,
    pub at: Option<usize>,
    wait: Option<Wait>,
}

#[derive(Debug, Clone, Copy)]
struct Wait {
    dest: Vec3,
    yaw: Quat,
    since: f32,
}

/// What a key press asks for: the stop to go to, from the one the tour is at (`None` at the start).
pub fn target(
    at: Option<usize>,
    n: usize,
    next: bool,
    prev: bool,
    first: bool,
    again: bool,
) -> Option<usize> {
    if n == 0 {
        return None;
    }
    if first {
        return Some(0);
    }
    match (at, next, prev, again) {
        (None, true, _, _) | (None, _, _, true) => Some(0),
        (None, _, true, _) => Some(n - 1),
        (Some(i), true, _, _) => Some((i + 1) % n),
        (Some(i), _, true, _) => Some((i + n - 1) % n),
        (Some(i), _, _, true) => Some(i),
        _ => None,
    }
}

/// The stops of a book's spots: one per hostile species, the rares first in [`RARES`] order, the rest
/// by level and name. A species takes its lowest guid among the spots the density keeps, else
/// its lowest guid.
pub fn stops(spots: &[crate::world::Spot], kept: impl Fn(usize) -> bool) -> Vec<Stop> {
    let mut by_species: std::collections::BTreeMap<&'static str, usize> = Default::default();
    for (i, s) in spots.iter().enumerate() {
        let Kind::Foe(name) = s.kind else { continue };
        let better = by_species.get(name).is_none_or(|&j| {
            let (a, b) = (kept(i), kept(j));
            (a && !b) || (a == b && s.guid < spots[j].guid)
        });
        if better {
            by_species.insert(name, i);
        }
    }
    let stop = |name: &'static str, spot: usize| Stop {
        name,
        spot,
        rare: RARES.contains(&name),
        stand_off: CLOSE
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(STAND_OFF, |(_, d)| *d),
    };
    let mut out: Vec<Stop> = RARES
        .iter()
        .filter_map(|&n| by_species.get(n).map(|&i| stop(n, i)))
        .collect();
    let mut rest: Vec<(u8, &'static str, usize)> = by_species
        .iter()
        .filter(|(n, _)| !RARES.contains(*n))
        .map(|(&n, &i)| (crate::species::by_name(n).map_or(0, |d| d.level), n, i))
        .collect();
    rest.sort();
    out.extend(rest.into_iter().map(|(_, n, i)| stop(n, i)));
    out
}

pub fn plugin(app: &mut App) {
    let script = std::env::var("WOWSOULS_TOUR_KEYS")
        .ok()
        .map(|v| parse_script(&v))
        .unwrap_or_default();
    if std::env::var("WOWSOULS_TOUR").map_or(script.is_empty(), |v| v.trim() == "0") {
        return;
    }
    app.insert_resource(TourScript(script))
        .init_resource::<Tour>()
        .add_systems(
            Update,
            (build, jump)
                .chain()
                .after(PlaySet::Motor)
                .run_if(in_state(Phase::Playing)),
        );
}

/// `<real s>:<next|prev|first|again>,...`: the presses a check makes, in time order.
#[derive(Resource, Default)]
struct TourScript(Vec<(f32, String)>);

fn parse_script(v: &str) -> Vec<(f32, String)> {
    let mut steps: Vec<(f32, String)> = v
        .split(',')
        .filter_map(|p| {
            let (t, k) = p.trim().split_once(':')?;
            Some((t.trim().parse::<f32>().ok()?, k.trim().to_string()))
        })
        .collect();
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    steps
}

fn build(
    mut tour: ResMut<Tour>,
    wb: Option<Res<WorldBook>>,
    real: Res<Time<Real>>,
    mut toasts: Option<ResMut<crate::quests::Toasts>>,
) {
    let Some(wb) = wb else { return };
    if !tour.stops.is_empty() {
        return;
    }
    let kept = |i: usize| {
        wb.book.recs[i].state != crate::ring::State::Off
            && wb.book.recs[i].until != Some(crate::ring::NEVER)
    };
    tour.stops = stops(&wb.book.spots, kept);
    info!(
        "tour: {} stops, PageDown / PageUp / Home / End",
        tour.stops.len()
    );
    if let Some(t) = toasts.as_deref_mut() {
        t.push(
            real.elapsed_secs(),
            "Tour: PageDown next, PageUp previous, Home first, End again",
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn jump(
    mut commands: Commands,
    real: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    menu: Res<crate::settings::MenuUp>,
    mut tour: ResMut<Tour>,
    mut wb: Option<ResMut<WorldBook>>,
    (mut death, mut spawn, mut toasts): (
        ResMut<Death>,
        Option<ResMut<SpawnPoint>>,
        Option<ResMut<crate::quests::Toasts>>,
    ),
    (script, mut scripted): (Res<TourScript>, Local<usize>),
    (progress, collide, mut epoch): (
        Option<Res<WorldLoadProgress>>,
        benilla_world::collision::WorldCollision,
        ResMut<benilla_world::collision::ColliderEpoch>,
    ),
    creatures: Query<Option<&BodyCollider>, Without<Avatar>>,
    mut avatar: Query<
        (
            &mut Transform,
            &mut Motor,
            &mut MoveIntent,
            &mut Health,
            &mut Stamina,
            &mut Action,
            &mut Poise,
        ),
        With<Avatar>,
    >,
) {
    let Ok((mut xf, mut m, mut intent, mut hp, mut st, mut action, mut poise)) =
        avatar.single_mut()
    else {
        return;
    };
    let now = real.elapsed_secs();

    // A jump waits for the ground: the body is held at its seed height until the tile is resident.
    if let Some(w) = tour.wait {
        xf.translation = w.dest;
        xf.rotation = w.yaw;
        *m = Motor::default();
        *intent = MoveIntent::default();
        let wow = benilla_assets::coords::bevy_to_wow(w.dest);
        let tile = benilla_formats::world_to_tile(wow[0], wow[1]);
        let resident = progress.as_ref().is_some_and(|p| {
            p.presentable() && p.focus_tile == Some((tile.0 as i32, tile.1 as i32))
        });
        let ground = crate::settle::snap_to_ground(&collide, w.dest)
            .filter(|y| (*y - w.dest.y).abs() <= crate::npc::GROUND_TOLERANCE);
        if resident && ground.is_some() || now > w.since + GIVE_UP {
            if let Some(y) = ground {
                xf.translation.y = y;
            }
            if let Some(s) = spawn.as_deref_mut() {
                s.0 = xf.translation;
            }
            tour.wait = None;
        }
        return;
    }

    if tour.stops.is_empty() {
        return;
    }
    let due = script
        .0
        .get(*scripted)
        .filter(|(t, _)| now >= *t)
        .map(|(_, k)| k.as_str());
    if due.is_some() {
        *scripted += 1;
    }
    let live = unattended.is_none() && !menu.0;
    let key = |k: KeyCode| live && keys.just_pressed(k);
    let n = tour.stops.len();
    let Some(i) = target(
        tour.at,
        n,
        key(KeyCode::PageDown) || due == Some("next"),
        key(KeyCode::PageUp) || due == Some("prev"),
        key(KeyCode::Home) || due == Some("first"),
        key(KeyCode::End) || due == Some("again"),
    ) else {
        return;
    };
    let Some(wb) = wb.as_deref_mut() else { return };
    let stop = tour.stops[i].clone();
    let spot = wb.book.spots[stop.spot].clone();

    // The creature starts fresh where it stands: whatever is there goes, a slain one comes back.
    if let Some(e) = wb.entity[stop.spot].take() {
        if let Ok(Some(body)) = creatures.get(e) {
            commands.entity(body.0).try_despawn();
            epoch.bump();
        }
        commands.entity(e).try_despawn();
    }
    let rec = &mut wb.book.recs[stop.spot];
    rec.until = None;
    rec.state = crate::ring::State::Absent;

    // The player in front of it (the way it faces), looking at it, whole.
    let at = wow_to_bevy(spot.wow);
    let creature = Quat::from_rotation_y(spot.facing);
    let front = creature * Vec3::NEG_Z;
    let dest = at + front * stop.stand_off + Vec3::Y * 1.0;
    let yaw = creature * Quat::from_rotation_y(std::f32::consts::PI);
    xf.translation = dest;
    xf.rotation = yaw;
    *m = Motor::default();
    *intent = MoveIntent::default();
    *action = Action::default();
    *hp = Health::full(hp.max);
    *st = Stamina::full(st.max);
    *poise = Poise::new(poise.max);
    death.at = None;
    tour.wait = Some(Wait {
        dest,
        yaw,
        since: now,
    });
    tour.at = Some(i);

    let def = crate::species::by_name(stop.name);
    let title = def.map_or(stop.name, |d| d.title);
    let level = def.map_or(0, |d| d.level);
    let text = format!(
        "{}/{}  {title}  ({}, level {level})",
        i + 1,
        n,
        if stop.rare { "rare" } else { "quest creature" }
    );
    info!("tour: {text}");
    if let Some(t) = toasts.as_deref_mut() {
        t.push(now, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_walk_the_list_and_wrap() {
        assert_eq!(target(None, 5, true, false, false, false), Some(0));
        assert_eq!(target(None, 5, false, true, false, false), Some(4));
        assert_eq!(target(Some(4), 5, true, false, false, false), Some(0));
        assert_eq!(target(Some(0), 5, false, true, false, false), Some(4));
        assert_eq!(target(Some(2), 5, false, false, false, true), Some(2));
        assert_eq!(target(Some(3), 5, false, false, true, false), Some(0));
        assert_eq!(target(Some(3), 5, false, false, false, false), None);
        assert_eq!(target(None, 0, true, false, false, false), None);
    }
}
