//! Critters: the valley's rabbits, deer and fawns (`creature_template.type` 8). They graze their area
//! as any wanderer does ([`crate::roam`]), run from a player who comes too close ([`mood_step`],
//! [`flee_dir`]) and die to one blow. No Dark Souls kit, no lock-on, no body collider: the player
//! walks through them, as in WoW. They take blows like any creature ([`crate::hits::strike`] needs
//! only a `Hurtbox` and `Health`), and [`react_critter`] plays the death; their motors step in
//! [`crate::walkers`].

use bevy::animation::transition::AnimationTransitions;
use bevy::prelude::*;

use benilla_assets::ModelAnimations;
use benilla_world::collision::WorldCollision;

use crate::actor::{ActorModel, Catalogs};
use crate::avatar::Avatar;
use crate::combat::{Action, Health};
use crate::hits::{Hit, HitFloor, Hurtbox};
use crate::motor::{Motor, MoveIntent};
use crate::npc::{Placed, Reaction, Roam, Slain, Unplaced};
use crate::roam::{Area, Walk};
use crate::walkers::Origin;
use crate::world::{SpawnId, Spot, Template};

/// What a critter does this frame, from its distance to the player (ground yd) and its state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mood {
    /// Grazing: walks its area like any wanderer.
    Graze,
    /// Running from the player until it is [`SAFE`] yards off or `left` seconds have passed.
    Flee { left: f32 },
}

/// A person closer than this (yd) startles a critter that has no distance of its own
/// ([`startle_for`]).
pub const FLEE_NEAR: f32 = 9.0;
/// A fleeing critter calms down this far (yd) from the player.
pub const SAFE: f32 = 22.0;
/// It runs at most this long (s) once the player is no longer near.
pub const FLEE_MAX: f32 = 6.0;
/// A flight bends home once the critter is this many times its wander radius from home.
pub const BEND_AT: f32 = 2.0;
/// The least radius (yd) the bend counts with: a critter at a post (radius 0) still runs a few
/// yards straight before it turns home.
pub const BEND_FLOOR: f32 = 5.0;

/// The next mood: a grazing critter with the player within `near` yards starts to run; a running
/// one is startled again (its time reset) while the player stays that near, and calms down when it
/// is [`SAFE`] or its time is up.
pub fn mood_step(mood: Mood, dist: f32, near: f32, dt: f32) -> Mood {
    if dist <= near {
        return Mood::Flee { left: FLEE_MAX };
    }
    match mood {
        Mood::Graze => Mood::Graze,
        Mood::Flee { left } => {
            let left = left - dt;
            if left <= 0.0 || dist >= SAFE {
                Mood::Graze
            } else {
                Mood::Flee { left }
            }
        }
    }
}

/// How close (yd) the player may come before a critter of `entry` runs: a rabbit lets the player
/// nearest, a deer runs early.
pub fn startle_for(entry: u32) -> f32 {
    match entry {
        721 => 8.0,  // Rabbit
        890 => 10.0, // Fawn
        883 => 14.0, // Deer
        _ => FLEE_NEAR,
    }
}

/// The startle distance a critter gets: its own ([`startle_for`]), or under `calm` one no player can
/// come within, so it only grazes (and still runs when a blow does not kill it).
pub fn startle_near(entry: u32, calm: bool) -> f32 {
    if calm {
        -1.0
    } else {
        startle_for(entry)
    }
}

/// The critters never run from an approaching player (WoW Classic's critters graze and wander; they
/// only run when a blow does not kill them), so the player can walk up to one and strike it. Decided
/// after the first build, where rabbits that bolted at 8 yd could not be hit in a scripted run;
/// `WOWSOULS_STARTLE=1` brings the startle distances of [`startle_for`] back.
fn calm_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| !std::env::var("WOWSOULS_STARTLE").is_ok_and(|v| v.trim() == "1"))
}

/// Its walk (grazing) and run (fleeing) speeds, yd/s. A run must be at least the gait's Run line
/// (4.9 yd/s, [`crate::actor::anim`]) or the body plays its walk clip sped up; the rabbit's Run clip
/// is authored at 6.94 yd/s, the deer's too (the fawn is the deer model at 0.6).
pub fn speeds_for(entry: u32) -> (f32, f32) {
    match entry {
        721 => (1.2, 5.0), // Rabbit
        890 => (1.3, 5.0), // Fawn
        883 => (1.5, 7.0), // Deer
        _ => (1.2, 5.0),
    }
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn unit(v: [f32; 2]) -> Option<[f32; 2]> {
    let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
    (l > 1.0e-5).then(|| [v[0] / l, v[1] / l])
}

/// The way a critter at `pos` runs from the player at `player` (Bevy ground `[x, z]`, a unit vector):
/// straight away while it is near its `home`, bent toward home once it is more than [`BEND_AT`] times
/// its wander `radius` (at least [`BEND_FLOOR`]) from it, never toward the player.
pub fn flee_dir(player: [f32; 2], pos: [f32; 2], home: [f32; 2], radius: f32) -> [f32; 2] {
    let away = unit([pos[0] - player[0], pos[1] - player[1]]).unwrap_or([1.0, 0.0]);
    if dist(pos, home) <= BEND_AT * radius.max(BEND_FLOOR) {
        return away;
    }
    let Some(back) = unit([home[0] - pos[0], home[1] - pos[1]]) else {
        return away;
    };
    match unit([away[0] + back[0], away[1] + back[1]]) {
        Some(bent) => bent,
        // Home lies straight through the player: run across, to the side home is on.
        None => [-away[1], away[0]],
    }
}

/// Where a grazing critter walks this frame: back into its area when something pulled it out (a
/// flight), else wherever its wander takes it; `None` to stand.
pub fn graze(
    walk: &mut Walk,
    area: &Area,
    home: [f32; 2],
    pos: [f32; 2],
    dt: f32,
    rng: &mut crate::pattern::Rng,
) -> Option<[f32; 2]> {
    let (gap, back) = crate::roam::home_distance(area, home, pos);
    let out = if crate::roam::is_post(area) {
        gap > crate::roam::ARRIVE
    } else {
        gap > 0.0
    };
    if out {
        *walk = Walk::default();
        return Some(back);
    }
    crate::roam::step(walk, area, home, pos, dt, rng)
}

/// What a blow does to a critter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// Already dead: nothing.
    Ignore,
    /// It dies (one blow kills it: it has a single point of health).
    Dies,
    /// It lives (a blow too weak to kill): it runs.
    Runs,
}

pub fn fate(dead: bool, lethal: bool) -> Fate {
    match (dead, lethal) {
        (true, _) => Fate::Ignore,
        (false, true) => Fate::Dies,
        (false, false) => Fate::Runs,
    }
}

/// How small a critter is to the player's blade: each of its body capsules counts at least this
/// thick (yd), as the Young Wolf's do ([`HitFloor`]).
pub const HIT_FLOOR: f32 = 0.35;
/// How fast a critter turns (rad/s): quicker than a hostile, a startled animal spins round.
const TURN_RATE: f32 = 10.0;

/// A critter: its entry, its mood, how near it lets the player come, its two speeds and the
/// hurtbox it gets back when the player respawns.
#[derive(Component, Debug, Clone)]
pub struct Critter {
    pub entry: u32,
    pub mood: Mood,
    pub near: f32,
    pub walk: f32,
    pub run: f32,
    pub hurtbox: Hurtbox,
}

/// One critter at its spot: its template's display (no geoset filtering: a plain creature model draws
/// every submesh but its billboards), a single point of health, a small hurtbox, a parked motor.
pub(crate) fn place(
    commands: &mut Commands,
    spot: &Spot,
    t: &Template,
    catalogs: &Catalogs,
    asset_server: &AssetServer,
    collide: &WorldCollision,
) -> Result<Placed, Unplaced> {
    let shown =
        crate::friendly::pick_display(&t.displays, spot.guid).ok_or(Unplaced::NoModel(0))?;
    let Some(model) = ActorModel::for_display(&catalogs.creatures, shown, None, asset_server)
    else {
        warn!("critter: display {shown} has no model");
        return Err(Unplaced::NoModel(shown));
    };
    let feet = crate::walkers::feet_at(collide, spot.wow).ok_or(Unplaced::NoGround)?;
    let scale = model.scale;
    // `CreatureModelData.collisionHeight` is in model units, before the display's scale.
    let height = catalogs
        .creatures
        .collision_height(shown)
        .filter(|h| h.is_finite() && *h > 0.0)
        .map_or(0.6, |h| h * scale)
        .clamp(0.2, 2.5);
    let hurtbox = Hurtbox {
        radius: 0.3 * height,
        height,
    };
    let (walk, run) = speeds_for(spot.entry);
    let entity = commands
        .spawn((
            Name::new(t.name.clone()),
            Transform::from_translation(feet).with_rotation(Quat::from_rotation_y(spot.facing)),
            Visibility::default(),
            model,
            Critter {
                entry: spot.entry,
                mood: Mood::Graze,
                near: startle_near(spot.entry, calm_on()),
                walk,
                run,
                hurtbox,
            },
            Health::full(1.0),
            hurtbox,
            HitFloor(HIT_FLOOR),
            Reaction::default(),
            crate::npc::parked(),
            MoveIntent::default(),
            Roam::new(spot.area.clone(), u64::from(spot.guid)),
            Origin {
                feet,
                facing: spot.facing,
                body: (0.0, 0.0),
            },
            crate::audio::VoiceOf(shown),
        ))
        .id();
    Ok(Placed {
        entity,
        feet,
        display: shown,
        scale,
    })
}

fn turn(xf: &mut Transform, toward: Vec3, rate: f32) {
    if toward.length_squared() > 1.0e-4 {
        let current = xf.rotation.to_euler(EulerRot::YXZ).0;
        let next = crate::controller::turn_toward(current, crate::controller::yaw_of(toward), rate);
        xf.rotation = Quat::from_rotation_y(next);
    }
}

/// Each living critter near the player: its mood from the player's distance, then a flight along
/// [`flee_dir`] at its run, or its grazing walk ([`graze`]). Out of the sim radius a grazing critter
/// rests (its motor parks in [`crate::walkers`]).
#[allow(clippy::type_complexity)]
pub(crate) fn critter_think(
    time: Res<Time>,
    avatar: Query<(&Transform, &Action, &Health), (With<Avatar>, Without<Critter>)>,
    mut critters: Query<(
        &Name,
        Option<&SpawnId>,
        &mut Transform,
        &mut Critter,
        &Origin,
        &mut Roam,
        &mut MoveIntent,
        &Reaction,
    )>,
) {
    let Ok((player, action, health)) = avatar.single() else {
        return;
    };
    let dt = time.delta_secs();
    let alive = !action.is_dead() && health.cur > 0.0;
    let sim = crate::ring::sim();
    let p = [player.translation.x, player.translation.z];
    for (name, id, mut xf, mut c, origin, mut roam, mut intent, reaction) in &mut critters {
        *intent = MoveIntent::default();
        if *reaction == Reaction::Dead {
            continue;
        }
        let here = [xf.translation.x, xf.translation.z];
        let gap = dist(p, here);
        if !crate::ring::simulated(gap, c.mood == Mood::Graze, sim) {
            continue;
        }
        // A dead player startles nothing.
        let seen = if alive { gap } else { f32::INFINITY };
        let before = c.mood;
        c.mood = mood_step(c.mood, seen, c.near, dt);
        let id = id.map_or(String::new(), |i| format!(" #{}", i.0));
        match (before, c.mood) {
            (Mood::Graze, Mood::Flee { .. }) => {
                info!("critter: {name}{id} runs from the player ({gap:.1} yd)")
            }
            (Mood::Flee { left }, Mood::Graze) => info!(
                "critter: {name}{id} grazes again ({gap:.1} yd from the player, {:.1} s left)",
                left.max(0.0)
            ),
            _ => {}
        }
        let home = [origin.feet.x, origin.feet.z];
        let (go, speed) = match c.mood {
            Mood::Flee { .. } => {
                roam.walk = Walk::default();
                let radius = match roam.area {
                    Area::Wander(r) => r,
                    _ => 0.0,
                };
                let d = flee_dir(p, here, home, radius);
                (Vec3::new(d[0], 0.0, d[1]), c.run)
            }
            Mood::Graze => {
                let Roam { area, walk, rng } = &mut *roam;
                match graze(walk, area, home, here, dt, rng) {
                    Some(t) => (Vec3::new(t[0] - here[0], 0.0, t[1] - here[1]), c.walk),
                    None => {
                        if crate::roam::is_post(area) {
                            let back = Quat::from_rotation_y(origin.facing) * Vec3::NEG_Z;
                            turn(&mut xf, back, TURN_RATE * dt);
                        }
                        (Vec3::ZERO, 0.0)
                    }
                }
            }
        };
        *intent = MoveIntent {
            dir: go.normalize_or_zero(),
            speed,
            jump: false,
        };
        turn(&mut xf, go, TURN_RATE * dt);
    }
}

/// A blow on a critter: one kills it (its Death clip, once; it lies where it fell as a corpse until
/// the ring replaces it, [`crate::world`]), and the kill is told as a [`Slain`] with no souls (the
/// quest phase counts kills from it). A blow on a corpse does nothing.
#[allow(clippy::type_complexity)]
pub(crate) fn react_critter(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    mut critters: Query<(
        &Name,
        Option<&SpawnId>,
        &mut Reaction,
        &mut Critter,
        Option<&ModelAnimations>,
        Option<&mut AnimationPlayer>,
        Option<&mut AnimationTransitions>,
    )>,
    mut slain: MessageWriter<Slain>,
) {
    for hit in hits.read() {
        let Ok((name, id, mut reaction, mut critter, anims, player, transitions)) =
            critters.get_mut(hit.target)
        else {
            continue;
        };
        match fate(*reaction == Reaction::Dead, hit.lethal) {
            Fate::Ignore => {}
            Fate::Runs => critter.mood = Mood::Flee { left: FLEE_MAX },
            Fate::Dies => {
                let clip = match (anims, player, transitions) {
                    (Some(a), Some(mut p), Some(mut tr)) => {
                        crate::npc::play_once(a, &mut p, &mut tr, crate::npc::DEATH, 1.0)
                    }
                    _ => None,
                };
                *reaction = Reaction::Dead;
                critter.mood = Mood::Graze;
                commands.entity(hit.target).remove::<Hurtbox>();
                slain.write(Slain {
                    who: hit.target,
                    souls: 0,
                });
                let id = id.map_or(String::new(), |i| format!(" #{}", i.0));
                info!("critter: {name}{id} dies (Death clip {clip:.2?} s)");
            }
        }
    }
}

/// The player respawned (death or F6): every critter is whole, calm and home again, as the
/// hostiles are ([`crate::npc`]'s reset).
#[allow(clippy::type_complexity)]
pub(crate) fn reset_critters(
    mut commands: Commands,
    mut respawned: MessageReader<crate::hurt::Respawned>,
    mut critters: Query<(
        Entity,
        &mut Transform,
        &Origin,
        &mut Critter,
        &mut Health,
        &mut Reaction,
        &mut Motor,
        &mut Roam,
    )>,
) {
    if respawned.read().count() == 0 {
        return;
    }
    let mut n = 0;
    for (e, mut xf, origin, mut c, mut hp, mut reaction, mut m, mut roam) in &mut critters {
        *xf = Transform::from_translation(origin.feet)
            .with_rotation(Quat::from_rotation_y(origin.facing))
            .with_scale(xf.scale);
        *hp = Health::full(hp.max);
        *reaction = Reaction::None;
        c.mood = Mood::Graze;
        *m = crate::npc::parked();
        roam.walk = Walk::default();
        commands.entity(e).insert(c.hurtbox);
        n += 1;
    }
    info!("critter: {n} critters reset at home");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::Rng;

    const DT: f32 = 1.0 / 30.0;

    #[test]
    fn a_grazing_critter_runs_when_the_player_comes_near() {
        assert_eq!(mood_step(Mood::Graze, 9.0, 8.0, DT), Mood::Graze);
        assert_eq!(
            mood_step(Mood::Graze, 8.0, 8.0, DT),
            Mood::Flee { left: FLEE_MAX }
        );
        assert_eq!(
            mood_step(Mood::Graze, 2.0, 14.0, DT),
            Mood::Flee { left: FLEE_MAX }
        );
    }

    #[test]
    fn a_flight_counts_down_and_ends_when_safe_or_timed_out() {
        // Between near and safe: it keeps running, its time running down.
        let Mood::Flee { left } = mood_step(Mood::Flee { left: 1.0 }, 15.0, 8.0, DT) else {
            panic!("still fleeing")
        };
        assert!((left - (1.0 - DT)).abs() < 1.0e-6, "{left}");
        // Safe: grazing again at once.
        assert_eq!(
            mood_step(Mood::Flee { left: 5.0 }, SAFE, 8.0, DT),
            Mood::Graze
        );
        // Out of time: grazing again.
        assert_eq!(
            mood_step(Mood::Flee { left: DT * 0.5 }, 15.0, 8.0, DT),
            Mood::Graze
        );
    }

    #[test]
    fn a_fleeing_critter_is_startled_again_while_the_player_stays_near() {
        assert_eq!(
            mood_step(Mood::Flee { left: 0.2 }, 5.0, 8.0, DT),
            Mood::Flee { left: FLEE_MAX }
        );
    }

    #[test]
    fn each_critter_has_its_own_startle_distance_and_speeds() {
        assert_eq!(startle_for(721), 8.0, "rabbit");
        assert_eq!(startle_for(890), 10.0, "fawn");
        assert_eq!(startle_for(883), 14.0, "deer");
        assert_eq!(startle_for(4075), FLEE_NEAR, "anything else");
        for entry in [721, 890, 883, 1] {
            let (walk, run) = speeds_for(entry);
            assert!((0.9..=1.6).contains(&walk), "{entry} walks {walk}");
            assert!(
                run > 3.0 * walk && run <= crate::motor::RUN_SPEED,
                "{entry} runs {run}"
            );
            // At its run the body plays its Run clip, not its walk sped up (the gait's Run line).
            let gait = crate::actor::anim::next_gait(
                crate::actor::anim::Gait::Walk,
                crate::actor::anim::LocoInput {
                    grounded: true,
                    speed: run,
                    clip_done: true,
                    ..Default::default()
                },
            );
            assert_eq!(gait, crate::actor::anim::Gait::Run, "{entry} at {run}");
            // At its walk it walks.
            let gait = crate::actor::anim::next_gait(
                crate::actor::anim::Gait::Stand,
                crate::actor::anim::LocoInput {
                    grounded: true,
                    speed: walk,
                    clip_done: true,
                    ..Default::default()
                },
            );
            assert_eq!(gait, crate::actor::anim::Gait::Walk, "{entry} at {walk}");
        }
    }

    fn len(v: [f32; 2]) -> f32 {
        (v[0] * v[0] + v[1] * v[1]).sqrt()
    }
    fn dot(a: [f32; 2], b: [f32; 2]) -> f32 {
        a[0] * b[0] + a[1] * b[1]
    }

    #[test]
    fn near_home_it_runs_straight_away_from_the_player() {
        let d = flee_dir([0.0, 0.0], [3.0, 4.0], [3.0, 4.0], 10.0);
        assert!(
            (d[0] - 0.6).abs() < 1e-5 && (d[1] - 0.8).abs() < 1e-5,
            "{d:?}"
        );
        // Inside twice the radius still straight.
        let d = flee_dir([0.0, 0.0], [10.0, 0.0], [-9.0, 0.0], 10.0);
        assert_eq!(d, [1.0, 0.0]);
    }

    #[test]
    fn far_from_home_the_flight_bends_home_but_never_toward_the_player() {
        let (player, pos, home) = ([0.0, 0.0], [30.0, 0.0], [30.0, 40.0]);
        let d = flee_dir(player, pos, home, 10.0);
        assert!((len(d) - 1.0).abs() < 1e-5);
        assert!(dot(d, [1.0, 0.0]) > 0.0, "still away: {d:?}");
        assert!(dot(d, [0.0, 1.0]) > 0.5, "bent home: {d:?}");
        // Home straight through the player: it runs across, not back at them.
        let d = flee_dir([0.0, 0.0], [30.0, 0.0], [-30.0, 0.0], 10.0);
        assert!((len(d) - 1.0).abs() < 1e-5);
        assert!(dot(d, [1.0, 0.0]).abs() < 1e-5, "{d:?}");
        // A critter at a post (radius 0) bends after the floor, not at once.
        assert_eq!(
            flee_dir([0.0, 0.0], [5.0, 0.0], [0.5, 0.0], 0.0),
            [1.0, 0.0]
        );
    }

    #[test]
    fn a_critter_run_from_its_area_walks_home_and_grazes_again() {
        let mut rng = Rng(9);
        // A wanderer 30 yd from its spawn point with a 10 yd area: back to the centre until inside.
        let area = Area::Wander(10.0);
        let mut walk = Walk::default();
        let home = [0.0, 0.0];
        let mut pos = [30.0, 0.0];
        assert_eq!(graze(&mut walk, &area, home, pos, DT, &mut rng), Some(home));
        let mut steps = 0;
        while crate::roam::home_distance(&area, home, pos).0 > 0.0 {
            let t = graze(&mut walk, &area, home, pos, DT, &mut rng).expect("walking home");
            let (dx, dz) = (t[0] - pos[0], t[1] - pos[1]);
            let l = (dx * dx + dz * dz).sqrt();
            pos = [pos[0] + dx / l * 1.2 * DT, pos[1] + dz / l * 1.2 * DT];
            steps += 1;
            assert!(steps < 2000);
        }
        assert!(dist(pos, home) <= 10.0 + 0.05);
        // Inside, it wanders: the next ask is a point of its area or a pause.
        for _ in 0..600 {
            if let Some(t) = graze(&mut walk, &area, home, pos, DT, &mut rng) {
                assert!(dist(t, home) <= 10.0 + 1e-3, "{t:?}");
            }
        }
        // A critter at a post walks back to the post and stands there.
        let mut walk = Walk::default();
        let post = Area::Stand;
        assert_eq!(
            graze(&mut walk, &post, home, [6.0, 0.0], DT, &mut rng),
            Some(home)
        );
        assert_eq!(
            graze(&mut walk, &post, home, [0.3, 0.0], DT, &mut rng),
            None
        );
    }

    #[test]
    fn a_chased_off_critter_runs_clear_then_grazes_again() {
        // The player stands still 4 yd from a rabbit; it runs, gets clear and calms down.
        let player = [0.0, 0.0];
        let home = [4.0, 0.0];
        let (walk_speed, run_speed) = speeds_for(721);
        let mut pos = home;
        let mut mood = Mood::Graze;
        let mut rng = Rng(2);
        let mut walk = Walk::default();
        let mut fled = 0.0;
        let mut calmed = None;
        for i in 0..(20.0 / DT) as usize {
            mood = mood_step(mood, dist(pos, player), startle_for(721), DT);
            let (to, speed) = match mood {
                Mood::Flee { .. } => {
                    fled += DT;
                    let d = flee_dir(player, pos, home, 10.0);
                    (Some([pos[0] + d[0], pos[1] + d[1]]), run_speed)
                }
                Mood::Graze => {
                    calmed.get_or_insert(i);
                    (
                        graze(&mut walk, &Area::Wander(10.0), home, pos, DT, &mut rng),
                        walk_speed,
                    )
                }
            };
            if let Some(t) = to {
                let (dx, dz) = (t[0] - pos[0], t[1] - pos[1]);
                let l = (dx * dx + dz * dz).sqrt();
                if l > 1e-6 {
                    let s = (speed * DT).min(l);
                    pos = [pos[0] + dx / l * s, pos[1] + dz / l * s];
                }
            }
            if let (Some(c), Mood::Flee { .. }) = (calmed, mood) {
                // Startled again by wandering back near the player: allowed, but not at once.
                assert!(i > c + 30);
            }
        }
        assert!(fled > 0.5, "it ran ({fled} s)");
        assert!(calmed.is_some(), "it calmed down");
        assert!(
            dist(pos, home) <= 2.0 * 10.0 + 1.0,
            "it stayed near home: {pos:?}"
        );
    }

    #[test]
    fn a_calm_critter_is_never_startled() {
        assert_eq!(startle_near(883, false), 14.0);
        let near = startle_near(883, true);
        assert_eq!(mood_step(Mood::Graze, 0.0, near, DT), Mood::Graze);
        // A blow that does not kill still makes it run, and it calms down as before.
        let Mood::Flee { left } = mood_step(Mood::Flee { left: FLEE_MAX }, 0.5, near, DT) else {
            panic!("still running")
        };
        assert!(left < FLEE_MAX);
    }

    #[test]
    fn one_blow_kills_and_a_corpse_feels_nothing() {
        assert_eq!(fate(false, true), Fate::Dies);
        assert_eq!(fate(true, true), Fate::Ignore);
        assert_eq!(fate(true, false), Fate::Ignore);
        assert_eq!(fate(false, false), Fate::Runs);
    }
}
