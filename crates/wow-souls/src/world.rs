//! The valley as a book of spawn spots read from `cache/world/northshire.txt` (the manifest of
//! `wow-world`), the rules that turn a creature entry into a kind of creature, and the systems that
//! keep the hostile ones alive in a ring around the player ([`crate::ring`]): `ring_tick` spawns and
//! despawns, `note_deaths` starts a killed creature's respawn time, a player respawn clears them.
//!
//! Switches: `WOWSOULS_WORLD=0|1` (0: the slice's hand-placed groups; 1: the manifest even in an
//! unattended run; unset: the manifest for a person playing, the groups for a scripted run, whose
//! checks assume one lone kobold), `WOWSOULS_WORLD_FILE=<path>`, `WOWSOULS_RING=<spawn>,<despawn>`
//! (yd), `WOWSOULS_SIM=<yd>`, `WOWSOULS_RESPAWN=<floor s>`, `WOWSOULS_RESPAWN_X=<multiplier>`.
//! `WOWSOULS_TRACE` or `WOWSOULS_FRAMES` print a frame-time line every five seconds.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use bevy::prelude::*;
use bevy::time::Real;
use wow_world::model::{Creature, Waypoint, World};

use benilla_world::collision::{ColliderEpoch, WorldCollision};

use crate::actor::Catalogs;
use crate::avatar::Avatar;
use crate::monster::Roster;
use crate::npc::{BodyCollider, Brain, Hostile, Mind, Reaction, Slain, Unplaced};
use crate::ring::{Rec, Respawn, Ring, State};
use crate::roam::{Area, Node};
use crate::settle::Phase;

/// The manifest the game reads unless `WOWSOULS_WORLD_FILE` names another.
pub const MANIFEST: &str = "cache/world/northshire.txt";
/// How often the ring is looked at (real s).
const TICK: f32 = 0.25;
/// At most this many creatures are put down per tick (nearest first).
const MAX_SPAWN: usize = 3;
/// A spawn that found no ground is tried again after this long (real s).
const RETRY: f32 = 0.5;

/// Whether the valley comes from the manifest: `WOWSOULS_WORLD=0` no, `=1` yes, otherwise yes for a
/// person playing and no for an unattended (scripted) run.
pub fn decide_world(var: Option<&str>, unattended: bool) -> bool {
    match var.map(str::trim) {
        Some("0") => false,
        Some("1") => true,
        _ => !unattended,
    }
}

/// [`decide_world`] from the environment, decided once.
pub fn world_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        decide_world(
            std::env::var("WOWSOULS_WORLD").ok().as_deref(),
            crate::run_mode::unattended(),
        )
    })
}

/// Links a creature to its spawn record (an index into [`WorldBook`]'s book).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnId(pub usize);

/// The book and the ring's own bookkeeping.
#[derive(Resource)]
pub struct WorldBook {
    pub book: Book,
    pub ring: Ring,
    pub respawn: Respawn,
    /// The live entity of each record.
    entity: Vec<Option<Entity>>,
    /// When a record that found no ground may be tried again (real s).
    retry_at: Vec<f32>,
    /// The record's "no ground yet" line has been written.
    warned: Vec<bool>,
}

/// Whether a spot never spawns: the hostiles the roster lacks (Mine Spider, Kobold Miner) and the
/// friendly entries the game leaves out (the Spirit Healer); they must not take the ring's slots.
pub fn starts_off(spot: &Spot) -> bool {
    match spot.kind {
        Kind::Unmapped => true,
        Kind::Friendly => crate::friendly::skipped(spot.entry),
        Kind::Foe(_) | Kind::Critter => false,
    }
}

/// The default share of the hostile and critter spawns that stay: the user found the database's
/// density "a bit too much" (2026-10-09) and asked for about half. `WOWSOULS_DENSITY=<0..1>`.
pub const DENSITY: f32 = 0.5;

/// What the quests need without waiting for a respawn (18 minutes), as (creature entry, how many
/// spots must stay): Skirmish at Echo Ridge wants 12 Kobold Laborers and the valley has only 17, so
/// halving them (9) would make it impossible. The other kill and drop counts are far below half
/// their spawns. G3 can compute this from the real quest requirements.
pub const QUEST_FLOORS: [(u32, usize); 1] = [(80, 15)];

/// The spawns dropped by the density `density`: `true` at index `i` means spot `i` is thinned out.
/// Only hostiles and critters are thinned (the friendly NPCs and a lone boss stay). Inside one
/// creature entry the kept spots are spread evenly along the database's guid order, and the
/// database allocates guids camp by camp, so every camp keeps about its share. An entry keeps at
/// least `ceil(n * density)` and at least its [`QUEST_FLOORS`] entry.
pub fn thin(spots: &[Spot], density: f32) -> Vec<bool> {
    let density = if density.is_finite() {
        density.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let mut thinned = vec![false; spots.len()];
    let mut by_entry: std::collections::BTreeMap<u32, Vec<usize>> = Default::default();
    for (i, s) in spots.iter().enumerate() {
        if matches!(s.kind, Kind::Foe(_) | Kind::Critter) {
            by_entry.entry(s.entry).or_default().push(i);
        }
    }
    for (entry, mut idx) in by_entry {
        let n = idx.len();
        let floor = QUEST_FLOORS
            .iter()
            .find(|(e, _)| *e == entry)
            .map_or(0, |(_, f)| *f);
        let keep = ((n as f32 * density).ceil() as usize).max(floor).min(n);
        idx.sort_by_key(|&i| spots[i].guid);
        for (k, &i) in idx.iter().enumerate() {
            // Spot k stays when the running share (k+1) * keep / n crosses a whole number.
            let before = k * keep / n;
            let after = (k + 1) * keep / n;
            thinned[i] = after == before;
        }
    }
    thinned
}

impl WorldBook {
    /// Every spot that [`starts_off`], and every spot that `density` thins out, is [`State::Off`].
    pub fn new(mut book: Book, ring: Ring, respawn: Respawn, density: f32) -> WorldBook {
        let thinned = thin(&book.spots, density);
        for ((spot, rec), gone) in book.spots.iter().zip(book.recs.iter_mut()).zip(thinned) {
            if starts_off(spot) || gone {
                rec.state = State::Off;
            }
        }
        let n = book.recs.len();
        WorldBook {
            book,
            ring,
            respawn,
            entity: vec![None; n],
            retry_at: vec![0.0; n],
            warned: vec![false; n],
        }
    }
}

/// How many spots of each kind the book holds.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Census {
    pub foe: usize,
    pub critter: usize,
    pub friendly: usize,
    pub unmapped: usize,
}

pub fn census(book: &Book) -> Census {
    let mut c = Census::default();
    for s in &book.spots {
        match s.kind {
            Kind::Foe(_) => c.foe += 1,
            Kind::Critter => c.critter += 1,
            Kind::Friendly => c.friendly += 1,
            Kind::Unmapped => c.unmapped += 1,
        }
    }
    c
}

/// A short name of an area for the log lines.
fn area_label(area: &Area) -> String {
    match area {
        Area::Stand => "stands".into(),
        Area::Wander(r) => format!("wanders {r} yd"),
        Area::Path(nodes) => format!("walks {} nodes", nodes.len()),
    }
}

fn ground_dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

pub fn plugin(app: &mut App) {
    if frames_on() {
        app.init_resource::<MainCost>()
            .add_systems(First, main_cost_start)
            .add_systems(Last, main_cost_end)
            .add_systems(Update, frame_report.run_if(in_state(Phase::Playing)));
    }
    if !world_on() {
        return;
    }
    app.add_systems(Startup, load)
        // In `PreUpdate`: its spawns and despawns land before any `Update` system dresses a creature
        // (`actor::build`, `held`, `hitbody`) or inserts into one, so a despawn never races an insert.
        .add_systems(
            PreUpdate,
            ring_tick
                .run_if(in_state(Phase::Playing))
                .run_if(resource_exists::<WorldBook>),
        )
        .add_systems(
            Update,
            (note_deaths, on_respawned)
                .run_if(in_state(Phase::Playing))
                .run_if(resource_exists::<WorldBook>),
        );
}

/// Reads the manifest into the [`WorldBook`]; without it the valley stays empty (one warning).
fn load(mut commands: Commands, state: Option<Res<crate::world_state::WorldStateFile>>) {
    let path = std::env::var("WOWSOULS_WORLD_FILE").unwrap_or_else(|_| MANIFEST.to_string());
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            warn!("world: cannot read {path} ({e}); no creatures (run `wow-souls-world import`)");
            return;
        }
    };
    let world = match wow_world::manifest::parse(&text) {
        Ok(w) => w,
        Err(e) => {
            warn!("world: {path} does not parse ({e:#}); no creatures");
            return;
        }
    };
    let book = Book::from_world(&world);
    let c = census(&book);
    let ring = std::env::var("WOWSOULS_RING")
        .ok()
        .and_then(|v| crate::ring::parse_ring(&v))
        .unwrap_or(crate::ring::RING);
    let respawn = crate::ring::parse_respawn(
        std::env::var("WOWSOULS_RESPAWN").ok().as_deref(),
        std::env::var("WOWSOULS_RESPAWN_X").ok().as_deref(),
    );
    info!(
        "world: {} spots (Foe {}, Critter {}, Friendly {}, Unmapped {}) from {path}; ring {}/{} yd, sim {} yd, respawn x{} floor {} s",
        book.spots.len(),
        c.foe,
        c.critter,
        c.friendly,
        c.unmapped,
        ring.spawn,
        ring.despawn,
        crate::ring::sim(),
        respawn.mult,
        respawn.floor
    );
    let mut foes: Vec<(&str, usize)> = Vec::new();
    for s in &book.spots {
        if let Kind::Foe(name) = s.kind {
            match foes.iter_mut().find(|(n, _)| *n == name) {
                Some(f) => f.1 += 1,
                None => foes.push((name, 1)),
            }
        }
    }
    let off: Vec<(u32, Kind)> = book
        .spots
        .iter()
        .filter(|s| starts_off(s))
        .map(|s| (s.entry, s.kind))
        .collect();
    info!(
        "world: foes {foes:?}; live too: {} critters, {} friendly; off (never spawned): {} spots {off:?}",
        c.critter,
        c.friendly - off.iter().filter(|(_, k)| *k == Kind::Friendly).count(),
        off.len()
    );
    let density = std::env::var("WOWSOULS_DENSITY")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|d| d.is_finite())
        .unwrap_or(DENSITY);
    let gone = thin(&book.spots, density).iter().filter(|g| **g).count();
    info!(
        "world: density {density}: {gone} hostile / critter spots thinned out (kept at least {QUEST_FLOORS:?}), the rest live"
    );
    let mut wb = WorldBook::new(book, ring, respawn, density);
    // The creatures that never come back and died in an earlier run stay dead.
    if let Some(state) = state.as_deref() {
        let mut n = 0;
        for (spot, rec) in wb.book.spots.iter().zip(wb.book.recs.iter_mut()) {
            if state.state.slain.contains(&spot.guid) {
                crate::ring::note_gone_for_good(rec);
                n += 1;
            }
        }
        if n > 0 {
            info!("ring: {n} creature(s) stay dead from an earlier run");
        }
    }
    commands.insert_resource(wb);
}

/// Every [`TICK`]: despawns what left the ring (or a corpse that is due, away from the player) and
/// puts down up to [`MAX_SPAWN`] of the creatures that entered it, nearest first. A spawn only counts
/// when its ground was found; otherwise the record is tried again after [`RETRY`].
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn ring_tick(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut next_at: Local<f32>,
    mut no_catalogs: Local<bool>,
    mut wb: ResMut<WorldBook>,
    avatar: Query<&Transform, With<Avatar>>,
    // Every spawn of the book: a hostile has a brain, a critter a reaction, an NPC a body collider.
    creatures: Query<(
        &Transform,
        Option<&Brain>,
        Option<&Reaction>,
        Option<&BodyCollider>,
    )>,
    roster: Option<Res<Roster>>,
    catalogs: Option<Res<Catalogs>>,
    asset_server: Res<AssetServer>,
    collide: WorldCollision,
    mut epoch: ResMut<ColliderEpoch>,
) {
    let now = time.elapsed_secs();
    if now < *next_at {
        return;
    }
    *next_at = now + TICK;
    let Ok(p) = avatar.single() else {
        return;
    };
    let Some(cat) = catalogs.as_deref() else {
        if !*no_catalogs {
            *no_catalogs = true;
            warn!("ring: no catalogs; no creatures");
        }
        return;
    };
    let player = [p.translation.x, p.translation.z];
    let wb = &mut *wb;
    let plan = {
        let entity = &wb.entity;
        // Not despawned: anything awake (chasing, fighting, walking home, flinching), and a corpse the
        // player stands near (it may lie far from its spawn spot, where the distances are measured).
        let keep = |i: usize| -> bool {
            // A creature that is gone for good (a boss, a miniboss) is left lying: its unlooted spoils
            // are on it, and nothing replaces it.
            if wb.book.recs[i].until == Some(crate::ring::NEVER) {
                return entity[i].is_some();
            }
            let Some((xf, brain, reaction, _)) = entity[i].and_then(|e| creatures.get(e).ok())
            else {
                return false;
            };
            brain.is_some_and(|b| b.mind != Mind::Idle)
                || matches!(reaction, Some(Reaction::Flinch(_)))
                || (reaction == Some(&Reaction::Dead)
                    && ground_dist(player, [xf.translation.x, xf.translation.z])
                        < crate::ring::CLEAR)
        };
        crate::ring::plan(&wb.book.recs, player, now, wb.ring, usize::MAX, &keep)
    };
    let mut gone = 0;
    for &i in &plan.despawn {
        let spot = &wb.book.spots[i];
        let rec = &mut wb.book.recs[i];
        let why = if rec.until.is_some() {
            "corpse due"
        } else {
            "out of the ring"
        };
        if let Some(e) = wb.entity[i].take() {
            if let Ok((_, _, _, Some(body))) = creatures.get(e) {
                commands.entity(body.0).try_despawn();
                epoch.bump();
            }
            commands.entity(e).try_despawn();
        }
        crate::ring::despawned(rec);
        gone += 1;
        info!(
            "ring: - {} #{i} guid {} ({why}, {:.0} yd)",
            kind_name(spot.kind),
            spot.guid,
            ground_dist(player, rec.at)
        );
    }
    let mut placed = 0;
    for &i in &plan.spawn {
        if placed >= MAX_SPAWN {
            break;
        }
        if now < wb.retry_at[i] {
            continue;
        }
        let spot = &wb.book.spots[i];
        // A foe is built from its species row; a critter or an NPC from its creature template.
        let (label, result) = match spot.kind {
            Kind::Foe(name) => {
                let Some(def) = crate::species::by_name(name) else {
                    wb.book.recs[i].state = State::Off;
                    continue;
                };
                let r = crate::npc::place(
                    &mut commands,
                    def,
                    spot.wow,
                    Some(spot.facing),
                    spot.area.clone(),
                    u64::from(spot.guid),
                    true,
                    roster.as_deref(),
                    cat,
                    &asset_server,
                    &collide,
                    &mut epoch,
                );
                (def.name.to_string(), r)
            }
            Kind::Critter | Kind::Friendly => {
                let Some(t) = wb.book.templates.get(&spot.entry) else {
                    wb.book.recs[i].state = State::Off;
                    warn!(
                        "ring: entry {} (guid {}) has no template; left out",
                        spot.entry, spot.guid
                    );
                    continue;
                };
                let r = if spot.kind == Kind::Critter {
                    crate::critter::place(&mut commands, spot, t, cat, &asset_server, &collide)
                } else {
                    crate::friendly::place(
                        &mut commands,
                        spot,
                        t,
                        cat,
                        &asset_server,
                        &collide,
                        &mut epoch,
                    )
                };
                (t.name.clone(), r)
            }
            Kind::Unmapped => {
                wb.book.recs[i].state = State::Off;
                continue;
            }
        };
        match result {
            Ok(at) => {
                commands.entity(at.entity).insert(SpawnId(i));
                wb.entity[i] = Some(at.entity);
                crate::ring::spawned(&mut wb.book.recs[i]);
                placed += 1;
                let w = benilla_assets::coords::bevy_to_wow(at.feet);
                info!(
                    "ring: + {label} #{i} guid {} entry {} at wow ({:.2},{:.2},{:.2}) (database ({:.2},{:.2},{:.2})) {}, facing {:.2}, {:.0} yd, display {}",
                    spot.guid,
                    spot.entry,
                    w[0],
                    w[1],
                    w[2],
                    spot.wow[0],
                    spot.wow[1],
                    spot.wow[2],
                    area_label(&spot.area),
                    spot.facing,
                    ground_dist(player, wb.book.recs[i].at),
                    at.display
                );
            }
            Err(Unplaced::NoGround) => {
                wb.retry_at[i] = now + RETRY;
                if !wb.warned[i] {
                    wb.warned[i] = true;
                    info!(
                        "ring: no ground yet within {} yd of {label} #{i} guid {} at wow {:?}; trying again",
                        crate::npc::GROUND_TOLERANCE,
                        spot.guid,
                        spot.wow
                    );
                }
            }
            Err(Unplaced::NoModel(shown)) => {
                wb.book.recs[i].state = State::Off;
                warn!(
                    "ring: {label} #{i} guid {} has no model (display {shown}); left out",
                    spot.guid
                );
            }
        }
    }
    if placed + gone > 0 {
        let live = wb
            .book
            .recs
            .iter()
            .filter(|r| r.state == State::Live)
            .count();
        let w = benilla_assets::coords::bevy_to_wow(p.translation);
        info!(
            "ring: spawned {placed}, despawned {gone} (live {live}) player wow ({:.1},{:.1}) t={now:.2}",
            w[0], w[1]
        );
    }
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Foe(name) => name,
        Kind::Critter => "critter",
        Kind::Friendly => "friendly",
        Kind::Unmapped => "unmapped",
    }
}

/// A killed creature of the valley comes back after its respawn time ([`Respawn`]).
fn note_deaths(
    time: Res<Time<Real>>,
    mut slain: MessageReader<Slain>,
    ids: Query<&SpawnId>,
    mut wb: ResMut<WorldBook>,
    mut state: ResMut<crate::world_state::WorldStateFile>,
) {
    let now = time.elapsed_secs();
    for s in slain.read() {
        let Ok(&SpawnId(i)) = ids.get(s.who) else {
            continue;
        };
        let wb = &mut *wb;
        let spot = &wb.book.spots[i];
        // A boss or a miniboss never comes back: remembered across runs.
        let never = match spot.kind {
            Kind::Foe(name) => crate::species::by_name(name)
                .is_some_and(|d| crate::species::rank_of(d).never_respawns()),
            _ => false,
        };
        if never {
            crate::ring::note_gone_for_good(&mut wb.book.recs[i]);
            state.state.slain.insert(spot.guid);
            state.mark();
            info!(
                "ring: {} #{i} guid {} died and stays dead (never respawns)",
                kind_name(spot.kind),
                spot.guid
            );
            continue;
        }
        let delay = wb.respawn.delay(spot.respawn_secs);
        crate::ring::note_death(&mut wb.book.recs[i], now, delay);
        info!(
            "ring: {} #{i} guid {} died at t={now:.2}; due again at t={:.2} ({delay:.0} s: x{} of {} s, floor {} s), once the player is {} yd from its spot",
            kind_name(spot.kind),
            spot.guid,
            wb.book.recs[i].until.unwrap_or(now),
            wb.respawn.mult,
            spot.respawn_secs,
            wb.respawn.floor,
            crate::ring::CLEAR
        );
    }
}

/// The player respawned (death or F6): every creature is due again at once. The corpses still
/// standing rise at home with `npc::reset`; the despawned dead come back as the ring meets them.
fn on_respawned(mut respawned: MessageReader<crate::hurt::Respawned>, mut wb: ResMut<WorldBook>) {
    if respawned.read().count() == 0 {
        return;
    }
    let n = wb.book.recs.iter().filter(|r| r.until.is_some()).count();
    crate::ring::clear_timers(&mut wb.book.recs);
    info!("ring: the player respawned: {n} respawn timers cleared");
}

/// `WOWSOULS_TRACE` or `WOWSOULS_FRAMES`: the frame-time line.
fn frames_on() -> bool {
    crate::actor::anim::trace_on() || std::env::var_os("WOWSOULS_FRAMES").is_some()
}

/// Frame times over a window: count, sum and the worst.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FrameStats {
    pub start: Option<f32>,
    pub frames: u32,
    pub sum: f32,
    pub worst: f32,
}

impl FrameStats {
    /// Adds one frame of `dt` seconds at real time `now`; after `window` seconds returns (frames,
    /// average ms, worst ms) and starts a new window.
    pub fn add(&mut self, dt: f32, now: f32, window: f32) -> Option<(u32, f32, f32)> {
        let start = *self.start.get_or_insert(now);
        self.frames += 1;
        self.sum += dt;
        self.worst = self.worst.max(dt);
        if now - start < window {
            return None;
        }
        let out = (
            self.frames,
            self.sum / self.frames as f32 * 1000.0,
            self.worst * 1000.0,
        );
        *self = FrameStats {
            start: Some(now),
            ..FrameStats::default()
        };
        Some(out)
    }
}

/// The main schedule's own time per frame (from `First` to `Last`, the game's and the engine's
/// systems on the main thread; rendering runs beside it): what a frame costs, whatever the display
/// paces the frames to.
#[derive(Resource, Default)]
struct MainCost {
    start: Option<std::time::Instant>,
    frames: u32,
    sum: f32,
    worst: f32,
}

fn main_cost_start(mut c: ResMut<MainCost>) {
    c.start = Some(std::time::Instant::now());
}

fn main_cost_end(mut c: ResMut<MainCost>) {
    if let Some(t) = c.start.take() {
        let dt = t.elapsed().as_secs_f32();
        c.frames += 1;
        c.sum += dt;
        c.worst = c.worst.max(dt);
    }
}

/// One line every five seconds: the frames, their average and worst wall time, the main schedule's
/// average and worst time, and the creatures (all, alive, simulated).
#[allow(clippy::type_complexity)]
fn frame_report(
    time: Res<Time<Real>>,
    mut stats: Local<FrameStats>,
    mut cost: ResMut<MainCost>,
    avatar: Query<&Transform, With<Avatar>>,
    creatures: Query<(&Transform, &Brain, &Reaction), (With<Hostile>, Without<Avatar>)>,
    others: Query<
        (&Transform, Has<crate::critter::Critter>),
        (
            Or<(With<crate::critter::Critter>, With<crate::friendly::Npc>)>,
            Without<Avatar>,
        ),
    >,
) {
    let Some((frames, avg, worst)) = stats.add(time.delta_secs(), time.elapsed_secs(), 5.0) else {
        return;
    };
    let main = format!(
        "main {:.1} ms avg, {:.1} worst",
        cost.sum / cost.frames.max(1) as f32 * 1000.0,
        cost.worst * 1000.0
    );
    *cost = MainCost {
        start: cost.start,
        ..MainCost::default()
    };
    let player = avatar
        .single()
        .ok()
        .map(|p| [p.translation.x, p.translation.z]);
    let sim = crate::ring::sim();
    let (mut all, mut alive, mut simulated) = (0, 0, 0);
    for (xf, brain, reaction) in &creatures {
        all += 1;
        if *reaction != Reaction::Dead {
            alive += 1;
            let gap = player.map_or(0.0, |p| {
                ground_dist(p, [xf.translation.x, xf.translation.z])
            });
            if crate::ring::simulated(gap, brain.mind == Mind::Idle, sim) {
                simulated += 1;
            }
        }
    }
    let at = player.map_or(String::new(), |p| {
        let w = benilla_assets::coords::bevy_to_wow(Vec3::new(p[0], 0.0, p[1]));
        format!(" player wow ({:.0},{:.0})", w[0], w[1])
    });
    // The critters and the friendly NPCs, and how many of them stand inside the sim radius.
    let (mut critters, mut npcs, mut near) = (0, 0, 0);
    for (xf, is_critter) in &others {
        if is_critter {
            critters += 1;
        } else {
            npcs += 1;
        }
        if player.is_some_and(|p| ground_dist(p, [xf.translation.x, xf.translation.z]) <= sim) {
            near += 1;
        }
    }
    info!(
        "frames: {frames} in 5 s, avg {avg:.1} ms, worst {worst:.1} ms ({main}); creatures {all} ({alive} alive, {simulated} simulated); critters {critters}, npcs {npcs} ({near} within sim){at}"
    );
}

/// What a spawn becomes in the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A hostile of this species (`species::by_name`).
    Foe(&'static str),
    /// A critter (G1b).
    Critter,
    /// A friendly NPC, guard, peasant or horse (G1c).
    Friendly,
    /// A hostile the roster does not have yet (logged, not spawned): Mine Spider, Kobold Miner.
    Unmapped,
}

/// Creature entry -> species (the kit rows built in E / E2; displays and numbers stay the species').
pub fn foe_of(entry: u32) -> Option<&'static str> {
    Some(match entry {
        80 => "kobold",
        6 => "vermin",
        257 => "worker",
        69 => "wolf",
        299 => "young_wolf",
        38 => "thug",
        103 => "garrick",
        _ => return None,
    })
}

/// The creature entry of a species ([`foe_of`] the other way): what a kill of an old hand-placed
/// creature (no spawn record) counts for in the quest log.
pub fn foe_entry(species: &str) -> Option<u32> {
    [80, 6, 257, 69, 299, 38, 103]
        .into_iter()
        .find(|&e| foe_of(e) == Some(species))
}

/// Entries with no `npc_flags` that are still friendly: the valley's guards, its peasants and the
/// White Stallion.
const FRIENDLY_EXTRA: [u32; 3] = [1642, 11260, 5403];

pub fn kind_of(c: &Creature) -> Kind {
    if let Some(name) = foe_of(c.entry) {
        Kind::Foe(name)
    } else if c.kind == 8 {
        Kind::Critter
    } else if c.npc_flags != 0 || FRIENDLY_EXTRA.contains(&c.entry) {
        Kind::Friendly
    } else {
        Kind::Unmapped
    }
}

/// One spawn spot, in the game's own terms.
#[derive(Debug, Clone)]
pub struct Spot {
    pub guid: u32,
    pub entry: u32,
    pub kind: Kind,
    /// WoW coordinates (x north, y west, z up): the ground height is a seed, the spawn snaps it.
    pub wow: [f32; 3],
    /// Bevy yaw (the database's orientation is the same angle).
    pub facing: f32,
    pub area: Area,
    /// The database's longest respawn time (s).
    pub respawn_secs: u32,
}

/// Bevy ground `[x, z]` of a WoW point.
pub fn flat(wow: [f32; 3]) -> [f32; 2] {
    [-wow[1], -wow[0]]
}

/// What the game reads of a creature template to build a critter or a friendly NPC (a foe is built
/// from its species row instead).
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub name: String,
    pub subname: String,
    /// `npc_flags`.
    pub flags: u32,
    /// Its display ids, the zeros left out; a spawn wears one ([`crate::friendly::pick_display`]).
    pub displays: Vec<u32>,
    /// WoW's walk multiplier.
    pub speed_walk: f32,
    /// The weapon's item display it holds ([`crate::friendly::held_weapon`]).
    pub weapon: Option<u32>,
}

impl Template {
    pub fn of(c: &Creature, w: &World) -> Template {
        Template {
            name: c.name.clone(),
            subname: c.subname.clone(),
            flags: c.npc_flags,
            displays: c.display.iter().copied().filter(|d| *d != 0).collect(),
            speed_walk: c.speed_walk,
            weapon: crate::friendly::held_weapon(w, c.equipment_id),
        }
    }
}

pub struct Book {
    pub spots: Vec<Spot>,
    pub recs: Vec<Rec>,
    /// Every creature template of the manifest, by entry.
    pub templates: HashMap<u32, Template>,
}

impl Book {
    pub fn from_world(w: &World) -> Book {
        let templates: HashMap<u32, &Creature> = w.creatures.iter().map(|c| (c.entry, c)).collect();
        let mut paths: HashMap<u32, Vec<&Waypoint>> = HashMap::new();
        for p in &w.waypoints {
            paths.entry(p.guid).or_default().push(p);
        }
        let mut spots = Vec::new();
        for s in &w.spawns {
            let Some(t) = templates.get(&s.entry) else {
                continue;
            };
            let area = match s.movement {
                2 => match paths.get(&s.guid) {
                    Some(points) if !points.is_empty() => {
                        let mut nodes: Vec<&Waypoint> = points.clone();
                        nodes.sort_by_key(|p| (p.path_id, p.point));
                        Area::Path(Arc::from(
                            nodes
                                .into_iter()
                                .map(|p| Node {
                                    at: flat(p.pos),
                                    wait: p.wait_ms as f32 / 1000.0,
                                })
                                .collect::<Vec<_>>(),
                        ))
                    }
                    _ => Area::Stand,
                },
                1 if s.wander > 0.0 => Area::Wander(s.wander),
                _ => Area::Stand,
            };
            spots.push(Spot {
                guid: s.guid,
                entry: s.entry,
                kind: kind_of(t),
                wow: s.pos,
                facing: s.orientation,
                area,
                respawn_secs: s.respawn_max.max(s.respawn_min),
            });
        }
        let recs = spots
            .iter()
            .map(|s| Rec {
                at: flat(s.wow),
                state: State::Absent,
                until: None,
            })
            .collect();
        let templates = w
            .creatures
            .iter()
            .map(|c| (c.entry, Template::of(c, w)))
            .collect();
        Book {
            spots,
            recs,
            templates,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::model::{Creature, Spawn, Waypoint, World};

    fn creature(entry: u32, kind: u32, flags: u32) -> Creature {
        Creature {
            entry,
            name: format!("c{entry}"),
            subname: String::new(),
            level_min: 1,
            level_max: 1,
            faction: 0,
            npc_flags: flags,
            display: [0; 4],
            scale: [0.0; 4],
            kind,
            rank: 0,
            detection_range: 0.0,
            call_for_help_range: 0.0,
            leash_range: 0.0,
            speed_walk: 0.0,
            speed_run: 0.0,
            loot_id: 0,
            equipment_id: 0,
            vendor_id: 0,
            trainer_type: 0,
            trainer_class: 0,
            movement_type: 0,
            civilian: false,
            health_multiplier: 1.0,
            damage_multiplier: 1.0,
            unit_class: 0,
        }
    }

    fn spawn(guid: u32, entry: u32, x: f32, y: f32, movement: u8, wander: f32) -> Spawn {
        Spawn {
            guid,
            entry,
            alt: [0; 4],
            pos: [x, y, 80.0],
            orientation: 1.5,
            respawn_min: 120,
            respawn_max: 270,
            wander,
            movement,
            area: 9,
            event: false,
        }
    }

    #[test]
    fn the_world_switch_table() {
        // (WOWSOULS_WORLD, unattended) -> the manifest?
        let table = [
            (None, false, true),
            (None, true, false),
            (Some("0"), false, false),
            (Some("0"), true, false),
            (Some("1"), false, true),
            (Some("1"), true, true),
            (Some(" 1 "), true, true),
            (Some("yes"), true, false),
            (Some(""), false, true),
        ];
        for (var, unattended, want) in table {
            assert_eq!(
                decide_world(var, unattended),
                want,
                "WOWSOULS_WORLD={var:?} unattended={unattended}"
            );
        }
    }

    #[test]
    fn critters_and_npcs_are_on_and_the_spirit_healer_and_the_unmapped_are_off() {
        let w = World {
            creatures: vec![
                creature(38, 7, 0),
                creature(721, 8, 0),
                creature(197, 7, 3),
                creature(6491, 7, 33),
                creature(43, 1, 0),
            ],
            spawns: vec![
                spawn(1, 38, -9056.0, -460.0, 0, 0.0),
                spawn(2, 721, -8905.0, -150.0, 1, 10.0),
                spawn(3, 197, -8900.0, -150.0, 0, 0.0),
                spawn(4, 6491, -8901.0, -150.0, 0, 0.0),
                spawn(5, 43, -8902.0, -150.0, 0, 0.0),
            ],
            ..World::default()
        };
        let book = Book::from_world(&w);
        assert_eq!(
            census(&book),
            Census {
                foe: 1,
                critter: 1,
                friendly: 2,
                unmapped: 1
            }
        );
        let wb = WorldBook::new(book, crate::ring::RING, Respawn::DEFAULT, 1.0);
        let states: Vec<State> = wb.book.recs.iter().map(|r| r.state).collect();
        assert_eq!(
            states,
            vec![
                State::Absent,
                State::Absent,
                State::Absent,
                State::Off,
                State::Off
            ]
        );
        // Beside the Abbey the rabbit and McBride spawn, nearest first; the healer and the spider
        // never take a slot, the thug is too far.
        let player = flat([-8900.0, -150.0, 0.0]);
        let p = crate::ring::plan(&wb.book.recs, player, 0.0, wb.ring, 9, &|_| false);
        assert_eq!(p.spawn, vec![2, 1]);
        let near_thug = flat([-9050.0, -450.0, 0.0]);
        let p = crate::ring::plan(&wb.book.recs, near_thug, 0.0, wb.ring, 3, &|_| false);
        assert_eq!(p.spawn, vec![0]);
    }

    fn spot(guid: u32, entry: u32, kind: Kind) -> Spot {
        Spot {
            guid,
            entry,
            kind,
            wow: [0.0; 3],
            facing: 0.0,
            area: Area::Stand,
            respawn_secs: 270,
        }
    }

    fn many(entry: u32, kind: Kind, n: u32) -> Vec<Spot> {
        (1..=n)
            .map(|g| spot(entry * 1000 + g, entry, kind))
            .collect()
    }

    fn kept(spots: &[Spot], thinned: &[bool], entry: u32) -> Vec<u32> {
        spots
            .iter()
            .zip(thinned)
            .filter(|(s, gone)| s.entry == entry && !**gone)
            .map(|(s, _)| s.guid)
            .collect()
    }

    #[test]
    fn half_the_hostiles_and_critters_stay_spread_along_the_guids() {
        let mut spots = many(38, Kind::Foe("thug"), 10);
        spots.extend(many(721, Kind::Critter, 7));
        let t = thin(&spots, 0.5);
        // 10 thugs keep 5, every second one in guid order; 7 rabbits keep ceil(3.5) = 4.
        assert_eq!(
            kept(&spots, &t, 38),
            vec![38_002, 38_004, 38_006, 38_008, 38_010]
        );
        assert_eq!(kept(&spots, &t, 721).len(), 4);
    }

    #[test]
    fn the_quest_floor_keeps_the_laborers_the_skirmish_needs() {
        let spots = many(80, Kind::Foe("kobold"), 17);
        let t = thin(&spots, 0.5);
        assert_eq!(
            kept(&spots, &t, 80).len(),
            15,
            "halving would leave 9 of the 12 needed"
        );
        // The floor never keeps more than there are.
        let few = many(80, Kind::Foe("kobold"), 4);
        assert_eq!(kept(&few, &thin(&few, 0.0), 80).len(), 4);
    }

    #[test]
    fn friendly_npcs_and_a_lone_boss_are_never_thinned() {
        let mut spots = many(197, Kind::Friendly, 1);
        spots.extend(many(1642, Kind::Friendly, 6));
        spots.extend(many(103, Kind::Foe("garrick"), 1));
        for density in [0.0, 0.5] {
            let t = thin(&spots, density);
            assert!(
                !t[..7].iter().any(|g| *g),
                "no friendly NPC is thinned at {density}"
            );
        }
        assert!(!thin(&spots, 0.5)[7], "a lone spot keeps ceil(0.5) = 1");
    }

    #[test]
    fn density_one_or_junk_thins_nothing_and_zero_keeps_only_the_floors() {
        let spots = many(38, Kind::Foe("thug"), 6);
        assert!(thin(&spots, 1.0).iter().all(|g| !g));
        assert!(thin(&spots, f32::NAN).iter().all(|g| !g));
        assert!(thin(&spots, 7.0).iter().all(|g| !g), "clamped to 1");
        assert!(thin(&spots, 0.0).iter().all(|g| *g), "no floor for thugs");
    }

    #[test]
    fn thinned_spots_never_take_a_ring_slot() {
        let mut w = World {
            creatures: vec![creature(38, 7, 0)],
            ..World::default()
        };
        w.spawns = (1..=4)
            .map(|g| spawn(g, 38, -9050.0 - g as f32, -450.0, 0, 0.0))
            .collect();
        let wb = WorldBook::new(
            Book::from_world(&w),
            crate::ring::RING,
            Respawn::DEFAULT,
            0.5,
        );
        let off = wb
            .book
            .recs
            .iter()
            .filter(|r| r.state == State::Off)
            .count();
        assert_eq!(off, 2, "four thugs keep two");
    }

    #[test]
    fn a_template_carries_its_name_displays_walk_and_weapon() {
        let mut guard = creature(1642, 7, 0);
        guard.name = "Northshire Guard".into();
        guard.display = [3167, 3257, 0, 0];
        guard.speed_walk = 1.0;
        guard.equipment_id = 1642;
        let w = World {
            creatures: vec![guard.clone()],
            equipment: vec![wow_world::model::Equipment {
                entry: 1642,
                probability: 1,
                items: [1899, 143, 2551],
            }],
            items: vec![wow_world::model::Item {
                entry: 1899,
                name: "Monster - Sword, Long Basic".into(),
                class: 2,
                subclass: 7,
                display: 7483,
                quality: 0,
                flags: 0,
                buy_price: 0,
                sell_price: 0,
                inventory_type: 13,
                allowable_class: -1,
                allowable_race: -1,
                item_level: 1,
                required_level: 0,
                max_count: 0,
                stackable: 1,
                delay: 2000,
                damage: (1.0, 2.0, 0),
                block: 0,
                armor: 0,
                stats: Vec::new(),
            }],
            ..World::default()
        };
        let t = Template::of(&guard, &w);
        assert_eq!(t.name, "Northshire Guard");
        assert_eq!(t.displays, vec![3167, 3257]);
        assert_eq!(t.weapon, Some(7483));
        assert_eq!(t.speed_walk, 1.0);
        assert_eq!(Book::from_world(&w).templates.get(&1642), Some(&t));
    }

    #[test]
    fn the_frame_line_averages_a_window() {
        let mut s = FrameStats::default();
        assert_eq!(s.add(0.010, 0.0, 5.0), None);
        assert_eq!(s.add(0.030, 2.0, 5.0), None);
        let (n, avg, worst) = s.add(0.020, 5.0, 5.0).expect("the window closed");
        assert_eq!(n, 3);
        assert!((avg - 20.0).abs() < 1e-3 && (worst - 30.0).abs() < 1e-3);
        // The next window starts empty.
        assert_eq!(s.frames, 0);
        assert_eq!(s.add(0.050, 6.0, 5.0), None);
        assert_eq!(s.frames, 1);
    }

    #[test]
    fn entries_map_to_kinds() {
        assert_eq!(kind_of(&creature(38, 7, 0)), Kind::Foe("thug"));
        assert_eq!(kind_of(&creature(103, 7, 0)), Kind::Foe("garrick"));
        assert_eq!(kind_of(&creature(721, 8, 0)), Kind::Critter);
        assert_eq!(
            kind_of(&creature(197, 7, 3)),
            Kind::Friendly,
            "McBride has npc flags"
        );
        assert_eq!(
            kind_of(&creature(1642, 7, 0)),
            Kind::Friendly,
            "a guard has none"
        );
        assert_eq!(
            kind_of(&creature(43, 1, 0)),
            Kind::Unmapped,
            "the Mine Spider"
        );
    }

    #[test]
    fn every_foe_entry_names_a_species_of_the_roster() {
        for entry in [80, 6, 257, 69, 299, 38, 103] {
            let name = foe_of(entry).unwrap();
            assert!(crate::species::by_name(name).is_some(), "{name}");
        }
    }

    #[test]
    fn the_book_reads_areas_facing_and_respawn_from_the_world() {
        let w = World {
            creatures: vec![creature(38, 7, 0), creature(721, 8, 0)],
            spawns: vec![
                spawn(1, 38, -9056.0, -460.0, 0, 0.0),
                spawn(2, 721, -9000.0, -300.0, 1, 10.0),
                spawn(3, 38, -9010.0, -250.0, 2, 0.0),
                spawn(4, 999, 0.0, 0.0, 0, 0.0),
            ],
            waypoints: vec![
                Waypoint {
                    guid: 3,
                    point: 2,
                    pos: [-9010.0, -240.0, 80.0],
                    orientation: 0.0,
                    wait_ms: 0,
                    script_id: 0,
                    path_id: 0,
                },
                Waypoint {
                    guid: 3,
                    point: 1,
                    pos: [-9010.0, -250.0, 80.0],
                    orientation: 0.0,
                    wait_ms: 4000,
                    script_id: 0,
                    path_id: 0,
                },
            ],
            ..World::default()
        };
        let b = Book::from_world(&w);
        assert_eq!(b.spots.len(), 3, "spawn 4 has no template and is dropped");
        assert_eq!(b.spots[0].area, Area::Stand);
        assert_eq!(b.spots[1].area, Area::Wander(10.0));
        let Area::Path(nodes) = &b.spots[2].area else {
            panic!("a path")
        };
        assert_eq!(nodes.len(), 2);
        assert_eq!(
            nodes[0],
            Node {
                at: [250.0, 9010.0],
                wait: 4.0
            },
            "sorted by point; WoW (x, y) is Bevy [-y, -x]"
        );
        assert_eq!(b.spots[0].respawn_secs, 270);
        assert_eq!(b.recs.len(), 3);
        assert_eq!(b.recs[0].at, [460.0, 9056.0]);
        assert!((b.spots[0].facing - 1.5).abs() < 1e-6);
    }

    /// The real manifest (`cache/world/northshire.txt`, skipped when the file is missing): every
    /// spawn lands in exactly one kind, the seven foe entries cover the hostiles the roster has, and
    /// the 18 database walkers keep their paths.
    #[test]
    fn the_installed_manifest_maps_every_spawn() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../cache/world/northshire.txt");
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("skipped: {} not found", path.display());
            return;
        };
        let world = wow_world::manifest::parse(&text).expect("the manifest parses");
        let book = Book::from_world(&world);
        assert_eq!(book.spots.len(), 334);
        assert_eq!(book.recs.len(), 334);

        let count =
            |pick: &dyn Fn(Kind) -> bool| book.spots.iter().filter(|s| pick(s.kind)).count();
        let foe = |name: &'static str| count(&|k| k == Kind::Foe(name));
        assert_eq!(foe("thug"), 77);
        assert_eq!(foe("worker"), 43);
        assert_eq!(foe("young_wolf"), 37);
        assert_eq!(foe("wolf"), 36);
        assert_eq!(foe("vermin"), 33);
        assert_eq!(foe("kobold"), 17);
        assert_eq!(foe("garrick"), 1);
        assert_eq!(
            count(&|k| matches!(k, Kind::Foe(_))),
            244,
            "the seven foe entries together"
        );
        assert_eq!(count(&|k| k == Kind::Critter), 53);
        assert_eq!(count(&|k| k == Kind::Friendly), 32);
        assert_eq!(count(&|k| k == Kind::Unmapped), 5);
        let mut unmapped: Vec<u32> = book
            .spots
            .iter()
            .filter(|s| s.kind == Kind::Unmapped)
            .map(|s| s.entry)
            .collect();
        unmapped.sort_unstable();
        assert_eq!(
            unmapped,
            vec![40, 43, 43, 43, 43],
            "one Kobold Miner and four Mine Spiders"
        );
        // Off: the five unmapped and the Spirit Healer; every critter and the other 31 friendly
        // spots are live, each with a template that has a display to wear.
        let off: Vec<u32> = book
            .spots
            .iter()
            .filter(|s| starts_off(s))
            .map(|s| s.entry)
            .collect();
        assert_eq!(off.len(), 6, "{off:?}");
        assert!(off.contains(&crate::friendly::SPIRIT_HEALER));
        let live_non_foes: Vec<&Spot> = book
            .spots
            .iter()
            .filter(|s| matches!(s.kind, Kind::Critter | Kind::Friendly) && !starts_off(s))
            .collect();
        assert_eq!(live_non_foes.len(), 53 + 31);
        for s in live_non_foes {
            let t = book.templates.get(&s.entry).expect("a template");
            assert!(
                crate::friendly::pick_display(&t.displays, s.guid).is_some(),
                "{} (entry {}) has no display",
                t.name,
                s.entry
            );
        }

        let walkers = world.spawns.iter().filter(|s| s.movement == 2).count();
        let paths = book
            .spots
            .iter()
            .filter(|s| matches!(s.area, Area::Path(_)))
            .count();
        assert_eq!(walkers, 18);
        assert_eq!(paths, 18, "every walker of the database has its waypoints");
        assert!(
            book.spots.iter().all(|s| match &s.area {
                Area::Wander(r) => *r > 0.0,
                Area::Path(nodes) => !nodes.is_empty(),
                Area::Stand => true,
            }),
            "no empty path and no zero-radius wander"
        );
        assert!(
            book.spots.iter().any(|s| matches!(s.area, Area::Wander(_))),
            "the valley has wanderers"
        );
    }
}
