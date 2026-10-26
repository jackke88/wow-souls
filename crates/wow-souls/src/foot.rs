//! Model sound events and footsteps. An M2 clip carries event keys (`$FSD` at each foot-plant of a walk
//! or run, `$SND` ...); [`fire`] scans the keys each playing gait clip crossed since the last frame and
//! sends them as [`AnimSound`]s, and [`footsteps`] answers a `$FSD` with WoW's own step: the ground under
//! the foot picks a `TerrainType`, the creature's `CreatureSoundData.FootstepID` the class, and
//! `FootstepTerrainLookup` the `SoundEntries` kit (wading takes the splash kit, deep water is silent).
//!
//! The rules follow benilla-app's `creature_anim/events.rs` and `sound/footsteps.rs` (docs/PROVENANCE.md),
//! cut to what a single-player world needs: no armor foley, mounts, stealth or ghost gates yet.

use std::collections::HashMap;

use bevy::animation::graph::AnimationNodeIndex;
use bevy::prelude::*;

use benilla_assets::coords::bevy_to_wow;
use benilla_assets::{ClipEvent, ModelAnimations};
use benilla_world::view::WorldCamera;
use benilla_world::world_point::{Subject, WorldPoint};

use crate::actor::anim::Locomotion;
use crate::actor::ActorModel;
use crate::audio::{listener, Audio};
use crate::avatar::Avatar;
use crate::combat::Action;
use crate::settle::Phase;

/// How loud a step is next to the other sounds (on top of the kit's own volume and the master).
const SCALE_STEP: f32 = 0.8;
/// A foot-plant farther than this from the listener is not even looked up (yd).
const HEARD_WITHIN: f32 = 45.0;
/// At most this many steps start in one frame (the reference's footstep bus holds six voices).
const STEPS_PER_FRAME: usize = 4;
/// Deeper than this the body swims and its steps are silent (yd; the reference's `0.75 x height`).
const WADE_MAX: f32 = 1.5;

/// A model event key crossed while a clip played: the forward-stored four-character tag (`*b"$FSD"`),
/// its payload (a `SoundEntries` id for `$SND`) and the key's world point.
#[derive(Message, Debug, Clone, Copy)]
pub struct AnimSound {
    pub entity: Entity,
    pub ident: [u8; 4],
    pub at: Vec3,
}

/// The keys of `events` crossed on `(prev, cur]`; a loop wrap (`cur < prev`) takes the tail of the clip
/// and then its head.
pub fn crossed(events: &[ClipEvent], duration: f32, prev: f32, cur: f32) -> Vec<ClipEvent> {
    if events.is_empty() || cur == prev {
        return Vec::new();
    }
    let within = |lo: f32, hi: f32| {
        events
            .iter()
            .filter(move |e| e.time > lo && e.time <= hi)
            .copied()
    };
    if cur >= prev {
        within(prev, cur).collect()
    } else {
        within(prev, duration + 1.0)
            .chain(within(-1.0, cur))
            .collect()
    }
}

/// Fires the event keys each actor's playing gait clip crossed since last frame.
#[allow(clippy::type_complexity)]
fn fire(
    actors: Query<(
        Entity,
        &ModelAnimations,
        &AnimationPlayer,
        &Locomotion,
        &GlobalTransform,
        Option<&crate::locomotion::GaitState>,
    )>,
    mut last: Local<HashMap<Entity, (AnimationNodeIndex, f32)>>,
    mut out: MessageWriter<AnimSound>,
) {
    for (entity, anims, player, loco, world, project_gait) in &actors {
        // The Dark Souls gait posing the legs sounds its own foot-downs ([`crate::locomotion`]); the
        // WoW clip underneath runs at another stride and would step out of time with them.
        if project_gait.is_some_and(crate::locomotion::GaitState::owns_steps) {
            last.remove(&entity);
            continue;
        }
        let Some(node) = loco.playing() else {
            continue;
        };
        let Some(clip) = anims.clips.iter().find(|c| c.node == node) else {
            continue;
        };
        let Some(cur) = player.animation(node).map(|a| a.seek_time()) else {
            continue;
        };
        // A new clip only starts the memory: nothing fires on the frame it arms.
        let prev = last
            .insert(entity, (node, cur))
            .filter(|(n, _)| *n == node)
            .map(|(_, t)| t);
        let Some(prev) = prev else {
            continue;
        };
        for e in crossed(&clip.events, clip.duration, prev, cur) {
            out.write(AnimSound {
                entity,
                ident: e.ident,
                at: world.transform_point(e.point),
            });
        }
    }
    // Forget the despawned (the map would only grow).
    if last.len() > 512 {
        last.retain(|e, _| actors.contains(*e));
    }
}

/// `$FSD`: the foot-plant sound of a gait.
pub fn is_footstep(ident: &[u8; 4]) -> bool {
    ident == b"$FSD"
}

/// The step of each foot-plant: the kit for this creature class on the ground under the foot.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn footsteps(
    mut commands: Commands,
    mut events: MessageReader<AnimSound>,
    audio: Option<ResMut<Audio>>,
    world: WorldPoint,
    collide: benilla_world::collision::WorldCollision,
    (wetness, shelter, spray): (
        Option<Res<benilla_world::weather::Wetness>>,
        Option<Res<benilla_world::weather::shelter::ShelterGrid>>,
        Option<Res<crate::puddle::Spray>>,
    ),
    cams: Query<&GlobalTransform, With<WorldCamera>>,
    bodies: Query<(
        &ActorModel,
        &GlobalTransform,
        Option<&Avatar>,
        Option<&Action>,
        Option<&crate::motor::Motor>,
    )>,
) {
    let Some(mut audio) = audio else {
        events.clear();
        return;
    };
    let heard = listener(&cams);
    let mut played = 0;
    for ev in events.read() {
        if !is_footstep(&ev.ident) || played >= STEPS_PER_FRAME {
            continue;
        }
        let Ok((model, xf, avatar, action, motor)) = bodies.get(ev.entity) else {
            continue;
        };
        // A roll or a swing poses the body itself; the gait clip under it makes no steps.
        if action.is_some_and(Action::owns_body) {
            continue;
        }
        if heard.is_some_and(|l| (ev.at - l.pos).length() > HEARD_WITHIN) {
            continue;
        }
        let class = audio.footstep_class(model.display);
        if class == 0 {
            continue; // no voice row, or a class that makes no sound
        }
        let who = if avatar.is_some() {
            Subject::Player
        } else {
            Subject::Unit(ev.entity)
        };
        let feet = xf.translation();
        let depth = world
            .water_surface_at(who, bevy_to_wow(feet))
            .map(|s| s - bevy_to_wow(feet)[2])
            .filter(|d| *d > 0.0);
        if depth.is_some_and(|d| d > WADE_MAX) {
            continue;
        }
        let Some(terrain) = world.terrain_type(audio.footsteps(), who, feet) else {
            continue;
        };
        let Some((dry, splash)) = audio.footsteps().resolve_terrain(class, terrain) else {
            continue;
        };
        // A rain puddle under the foot ([`crate::puddle`]) splashes as shallow water does.
        let wetted = crate::puddle::under(&collide, wetness.as_deref(), shelter.as_deref(), feet);
        if avatar.is_some() {
            trace!(
                "foot: puddle {wetted:.2} (wetness {:?})",
                wetness.as_deref().map(|w| w.wetness)
            );
        }
        let puddle = depth.is_none() && wetted > crate::puddle::SPLASH_FROM;
        let wet = depth.is_some() || puddle;
        let kit = if wet && splash != 0 { splash } else { dry };
        // WoW's footstep spray where the foot meets the water: the surface when wading, the ground in a
        // puddle.
        if let (true, Some(spray)) = (wet, spray.as_deref()) {
            let at = Vec3::new(ev.at.x, feet.y + depth.unwrap_or(0.0), ev.at.z);
            let speed = motor.map_or(7.0, |m| m.velocity.xz().length());
            crate::puddle::spray(&mut commands, spray, at, speed, xf.rotation());
            debug!("foot: splash at {at:?} (puddle {puddle}, kit {kit})");
        }
        if kit == 0 {
            continue;
        }
        // The player's own feet sound at the player, not from the camera six yards behind.
        let at = if avatar.is_some() {
            None
        } else {
            heard.map(|l| (ev.at, l))
        };
        audio.play_kit(kit, at, SCALE_STEP, "footstep");
        played += 1;
    }
}

pub fn plugin(app: &mut App) {
    app.add_message::<AnimSound>().add_systems(
        Update,
        (fire, footsteps)
            .chain()
            .after(crate::actor::anim::drive)
            .run_if(in_state(Phase::Playing)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(time: f32) -> ClipEvent {
        ClipEvent {
            time,
            ident: *b"$FSD",
            data: 0,
            bone: 0,
            offset: Vec3::ZERO,
            point: Vec3::ZERO,
        }
    }

    fn times(v: Vec<ClipEvent>) -> Vec<f32> {
        v.into_iter().map(|e| e.time).collect()
    }

    #[test]
    fn a_key_fires_once_when_the_clip_clock_passes_it() {
        let keys = [key(0.2), key(0.7)];
        assert_eq!(times(crossed(&keys, 1.0, 0.1, 0.3)), vec![0.2]);
        assert_eq!(times(crossed(&keys, 1.0, 0.3, 0.6)), Vec::<f32>::new());
        assert_eq!(times(crossed(&keys, 1.0, 0.6, 0.9)), vec![0.7]);
        assert_eq!(
            times(crossed(&keys, 1.0, 0.2, 0.7)),
            vec![0.7],
            "(prev, cur]"
        );
    }

    #[test]
    fn a_loop_wrap_takes_the_tail_then_the_head() {
        let keys = [key(0.1), key(0.9)];
        assert_eq!(times(crossed(&keys, 1.0, 0.8, 0.2)), vec![0.9, 0.1]);
        assert_eq!(times(crossed(&keys, 1.0, 0.95, 0.05)), Vec::<f32>::new());
    }

    #[test]
    fn a_standing_clock_fires_nothing() {
        let keys = [key(0.5)];
        assert!(crossed(&keys, 1.0, 0.5, 0.5).is_empty());
        assert!(crossed(&[], 1.0, 0.0, 1.0).is_empty());
    }

    #[test]
    fn only_the_foot_plant_tag_makes_a_step() {
        assert!(is_footstep(b"$FSD"));
        for other in [b"$FL0", b"$FR0", b"$SND", b"$DSO"] {
            assert!(!is_footstep(other));
        }
    }
}
