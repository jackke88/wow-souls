//! The movers that are not hostile: critters ([`crate::critter`]) and friendly NPCs
//! ([`crate::friendly`]). The same rules as the hostiles' ([`crate::npc`]): a motor stepped with the
//! player's capsule shape through [`motor::step`], only inside the sim radius ([`crate::ring::sim`]);
//! outside it, or standing still on the ground, the motor is parked so the body shows Stand. An NPC
//! at a post never steps; a path walker walks its waypoints ([`npc_think`], [`crate::roam`]); an NPC's
//! body collider follows it ([`follow_walker_bodies`]).
//!
//! Switch: with `WOWSOULS_TRACE`, a `walker` line per critter and walking NPC near the player twice a
//! second, a `post` line per standing NPC within 40 yd every five seconds.

use bevy::prelude::*;

use benilla_assets::coords::wow_to_bevy;
use benilla_world::collision::{MoverTraceExclusions, WorldCollision};

use crate::avatar::{Avatar, BodyShape};
use crate::critter::{Critter, Mood};
use crate::friendly::Npc;
use crate::motor::{self, Motor, MoveIntent};
use crate::npc::{BodyCollider, Reaction, Roam};
use crate::settle::Phase;
use crate::world::SpawnId;

/// How fast an NPC turns (rad/s), as a hostile does.
const TURN_RATE: f32 = 6.0;
/// How far past the sim radius (yd) the walkers' sweeps still skip a creature's body.
const EXCLUDE_MARGIN: f32 = 15.0;

/// Where a critter or an NPC was put down: its feet on the snapped ground, its database facing, and
/// its body collider's radius and height (0 for a critter, which has none).
#[derive(Component, Debug, Clone, Copy)]
pub struct Origin {
    pub feet: Vec3,
    pub facing: f32,
    pub body: (f32, f32),
}

/// A spawn's feet at a WoW point: the ground found near the database's height, or `None` while the
/// colliders under it are not in yet (the ring tries again, as for a foe: [`crate::npc::pick_ground`]).
pub(crate) fn feet_at(collide: &WorldCollision, wow: [f32; 3]) -> Option<Vec3> {
    let mut feet = wow_to_bevy(wow);
    let found = crate::settle::snap_to_ground(collide, feet);
    feet.y = crate::npc::pick_ground(feet.y, found, true)?;
    Some(feet)
}

pub fn plugin(app: &mut App) {
    if !crate::world::world_on() {
        return;
    }
    app.add_systems(
        Update,
        (
            crate::critter::react_critter,
            crate::critter::reset_critters,
            crate::critter::critter_think,
            npc_think,
            exclude_walker_bodies,
            move_walkers,
            clear_walker_exclusions,
            follow_walker_bodies,
        )
            .chain()
            // After the hostiles' own motor pass, whose exclusion list is empty again by then.
            .after(crate::npc::clear_exclusions)
            .run_if(in_state(Phase::Playing)),
    );
    if crate::actor::anim::trace_on() {
        app.add_systems(Update, trace_walkers.run_if(in_state(Phase::Playing)));
    }
}

fn ground(v: Vec3) -> [f32; 2] {
    [v.x, v.z]
}

fn gap(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// Each walking NPC near the player follows its path (or wanders its area) at its walk; a post
/// stands and keeps its facing.
#[allow(clippy::type_complexity)]
fn npc_think(
    time: Res<Time>,
    avatar: Query<&Transform, (With<Avatar>, Without<Npc>)>,
    mut npcs: Query<
        (&mut Transform, &Npc, &Origin, &mut Roam, &mut MoveIntent),
        Without<crate::village::Citizen>,
    >,
) {
    let Ok(player) = avatar.single() else {
        return;
    };
    let dt = time.delta_secs();
    let sim = crate::ring::sim();
    for (mut xf, npc, origin, mut roam, mut intent) in &mut npcs {
        *intent = MoveIntent::default();
        if crate::roam::is_post(&roam.area) || gap(player.translation, xf.translation) > sim {
            continue;
        }
        let here = ground(xf.translation);
        let Roam { area, walk, rng } = &mut *roam;
        let Some(t) = crate::roam::step(walk, area, ground(origin.feet), here, dt, rng) else {
            continue;
        };
        let d = Vec3::new(t[0] - here[0], 0.0, t[1] - here[1]);
        *intent = MoveIntent {
            dir: d.normalize_or_zero(),
            speed: npc.walk,
            jump: false,
        };
        if d.length_squared() > 1.0e-4 {
            let current = xf.rotation.to_euler(EulerRot::YXZ).0;
            let next = crate::controller::turn_toward(
                current,
                crate::controller::yaw_of(d),
                TURN_RATE * dt,
            );
            xf.rotation = Quat::from_rotation_y(next);
        }
    }
}

/// The walkers' sweeps skip every creature body collider near the player (a guard's own, its
/// neighbours', a hostile's: units pass through each other, as in WoW); the hostiles' list is empty
/// again when this runs.
#[allow(clippy::type_complexity)]
fn exclude_walker_bodies(
    mut ex: ResMut<MoverTraceExclusions>,
    avatar: Query<&Transform, With<Avatar>>,
    bodies: Query<(&BodyCollider, &Transform), Without<Avatar>>,
    world: Option<Res<crate::world::WorldBook>>,
) {
    let Ok(player) = avatar.single() else {
        return;
    };
    let reach =
        crate::ring::sim().max(world.as_ref().map_or(0.0, |w| w.ring.despawn)) + EXCLUDE_MARGIN;
    ex.0.extend(
        bodies
            .iter()
            .filter(|(_, xf)| gap(player.translation, xf.translation) <= reach)
            .map(|(b, _)| b.0),
    );
}

fn clear_walker_exclusions(mut ex: ResMut<MoverTraceExclusions>) {
    ex.0.clear();
}

/// Steps the motors of the critters and the NPCs that move: alive, inside the sim radius (or a
/// critter in flight), asked to go somewhere or not yet on the ground. The rest are parked.
#[allow(clippy::type_complexity)]
pub(crate) fn move_walkers(
    time: Res<Time>,
    collide: WorldCollision,
    shape: Res<BodyShape>,
    world: Option<Res<crate::world::WorldBook>>,
    avatar: Query<&Transform, (With<Avatar>, Without<Critter>, Without<Npc>)>,
    mut movers: Query<
        (
            &mut Transform,
            &mut Motor,
            &MoveIntent,
            Option<&Reaction>,
            Option<&Critter>,
            Has<crate::village::Citizen>,
        ),
        (Or<(With<Critter>, With<Npc>)>, Without<Avatar>),
    >,
) {
    let Ok(player) = avatar.single() else {
        return;
    };
    let player = player.translation;
    let sim = crate::ring::sim();
    for (mut xf, mut m, intent, reaction, critter, citizen) in &mut movers {
        if reaction == Some(&Reaction::Dead) {
            continue;
        }
        let idle = critter.is_none_or(|c| c.mood == Mood::Graze);
        let still = intent.speed == 0.0 || intent.dir.length_squared() < 1.0e-6;
        let reach = if citizen {
            sim.max(
                world
                    .as_ref()
                    .map_or(crate::ring::RING.despawn, |w| w.ring.despawn),
            )
        } else {
            sim
        };
        if !crate::ring::simulated(gap(player, xf.translation), idle, reach)
            || (still && m.grounded)
        {
            if !m.grounded || m.velocity != Vec3::ZERO {
                crate::npc::park(&mut m);
            }
            continue;
        }
        let mut feet = xf.translation;
        motor::step(&mut m, &mut feet, time.delta(), &collide, &shape.0, *intent);
        if feet != xf.translation {
            xf.translation = feet;
        }
    }
}

/// Keeps each NPC's body collider on it.
fn follow_walker_bodies(
    npcs: Query<(&Transform, &BodyCollider, &Origin), (With<Npc>, Changed<Transform>)>,
    mut bodies: Query<&mut Transform, Without<Npc>>,
) {
    for (xf, body, origin) in &npcs {
        if let Ok(mut b) = bodies.get_mut(body.0) {
            b.translation = xf.translation + Vec3::Y * (origin.body.1 * 0.5);
        }
    }
}

/// `WOWSOULS_TRACE`: the critters and walking NPCs near the player twice a second (place, mood or
/// path, distance to the player, speed, gait), the standing NPCs within 40 yd every five seconds.
#[allow(clippy::type_complexity)]
fn trace_walkers(
    time: Res<Time<bevy::time::Real>>,
    avatar: Query<&Transform, With<Avatar>>,
    movers: Query<
        (
            &Name,
            Option<&SpawnId>,
            &Transform,
            &Motor,
            &Roam,
            Option<&Critter>,
            Option<&Npc>,
            Option<&Reaction>,
            Option<&crate::actor::anim::Locomotion>,
        ),
        (Or<(With<Critter>, With<Npc>)>, Without<Avatar>),
    >,
    mut next_at: Local<f32>,
    mut posts_at: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now < *next_at {
        return;
    }
    *next_at = now + 0.5;
    let posts = now >= *posts_at;
    if posts {
        *posts_at = now + 5.0;
    }
    let Ok(player) = avatar.single() else {
        return;
    };
    let sim = crate::ring::sim();
    for (name, id, xf, m, roam, critter, npc, reaction, loco) in &movers {
        let d = gap(player.translation, xf.translation);
        let post = critter.is_none() && crate::roam::is_post(&roam.area);
        if (post && (!posts || d > 40.0)) || (!post && d > sim) {
            continue;
        }
        let w = benilla_assets::coords::bevy_to_wow(xf.translation);
        let id = id.map_or(String::new(), |i| format!(" #{}", i.0));
        let state = match (critter, npc, reaction) {
            (_, _, Some(Reaction::Dead)) => "dead".to_string(),
            (Some(c), _, _) => format!("entry {} {:?}", c.entry, c.mood),
            (None, Some(n), _) => format!(
                "entry {} '{}' flags {} {}",
                n.entry,
                n.subname,
                n.flags,
                if post { "post" } else { "path" }
            ),
            (None, None, _) => String::new(),
        };
        info!(
            "{} {name}{id} t={now:6.2} feet=({:9.2},{:8.2},{:6.2}) yaw={:+.2} {state} dist={d:.1} speed={:.2} gait={:?}",
            if post { "post" } else { "walker" },
            w[0],
            w[1],
            w[2],
            xf.rotation.to_euler(EulerRot::YXZ).0,
            m.velocity.xz().length(),
            loco.map(|l| l.gait)
        );
    }
}
