//! The spawn hold: the world streams around the avatar's body, and the body stays frozen until
//! the ground under it is resident *with its colliders*. Colliders build on the async compute
//! pool and attach under a per-frame budget, so a body released on first sight of the terrain
//! falls through it. The release keys on the engine's published residency
//! ([`WorldLoadProgress::presentable`]), never on ground contact, as benilla's
//! `player::world_focus::release_post_snap_hold` does (benilla @ 2e82d34), with the same stall
//! backstop: only a stream that makes no progress for [`SETTLE_TIMEOUT`] times out.

use bevy::prelude::*;

use benilla_assets::coords::bevy_to_wow;
use benilla_world::collision::WorldCollision;
use benilla_world::schedule::WorldStage;
use benilla_world::terrain_stream::{ViewFocus, WorldLoadProgress};
use benilla_world::view::Viewer;

use crate::avatar::{Avatar, HEIGHT};

/// Stall budget in seconds: a hold with no stream progress for this long releases anyway
/// (benilla's `SETTLE_TIMEOUT`).
pub const SETTLE_TIMEOUT: f32 = 6.0;

/// Ground-snap ray starts above the feet, nearest first: a spawn height that is slightly under
/// the real ground still finds it from the higher start. One-sided faces mean a ray that starts
/// below the ground sees nothing.
const SNAP_STARTS: [f32; 2] = [2.0, 60.0];
/// How far down a ground-snap ray reaches, yards.
const SNAP_REACH: f32 = 200.0;

/// The game's phase. `Streaming` holds the body under the loading cover; `Menu` is the title and
/// character creation over the streamed valley ([`crate::menu`]); `Playing` is live.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    #[default]
    Streaming,
    Menu,
    Playing,
}

pub fn plugin(app: &mut App) {
    app.init_state::<Phase>()
        .init_resource::<SettleClock>()
        // After the motor, so the stream follows this frame's body.
        .add_systems(
            Update,
            publish_focus
                .in_set(WorldStage::Input)
                .after(crate::controller::PlaySet::Motor),
        )
        // After the stream stage, so the progress read is this frame's.
        .add_systems(
            Update,
            release_hold
                .after(WorldStage::Stream)
                .before(WorldStage::Present)
                .run_if(in_state(Phase::Streaming)),
        );
}

/// Why the hold ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseWhy {
    /// The body's tile and its neighbours are up, colliders included.
    Resident,
    /// The stream stalled for [`SETTLE_TIMEOUT`].
    Timeout,
}

/// What the hold reads from the world each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SettleFacts {
    /// [`WorldLoadProgress::presentable`]: scene up, no collider/merge/retained-pass backlog.
    pub presentable: bool,
    /// The progress is about the tile the body stands on.
    pub on_body_tile: bool,
    /// Every counter the streamer publishes; any change is progress.
    pub counters: [usize; 6],
}

/// The hold's clock: the stall deadline and the last counters seen.
#[derive(Resource, Debug)]
pub struct SettleClock {
    deadline: f32,
    last: Option<[usize; 6]>,
}

impl Default for SettleClock {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl SettleClock {
    pub fn new(now: f32) -> Self {
        Self {
            deadline: now + SETTLE_TIMEOUT,
            last: None,
        }
    }

    /// One frame of the hold: `Some` when it ends.
    pub fn step(&mut self, facts: SettleFacts, now: f32) -> Option<ReleaseWhy> {
        let progressed = self.last.replace(facts.counters) != Some(facts.counters);
        if facts.presentable && facts.on_body_tile {
            return Some(ReleaseWhy::Resident);
        }
        if progressed {
            self.deadline = now + SETTLE_TIMEOUT;
            return None;
        }
        (now >= self.deadline).then_some(ReleaseWhy::Timeout)
    }
}

/// Tells the engine where to stream from (the body) and who the viewer is. Spawning is uncapped
/// while held under the cover and paced once live, as the client paces only a settled world.
fn publish_focus(
    phase: Res<State<Phase>>,
    avatar: Query<&Transform, With<Avatar>>,
    mut focus: ResMut<ViewFocus>,
    mut viewer: ResMut<Viewer>,
) {
    let Ok(body) = avatar.single() else {
        *focus = ViewFocus::camera();
        *viewer = Viewer::default();
        return;
    };
    let paced = *phase.get() == Phase::Playing;
    *focus = ViewFocus::body(bevy_to_wow(body.translation), paced);
    *viewer = Viewer {
        at: Some(body.translation),
        height: HEIGHT,
        ..Viewer::default()
    };
}

/// Releases the hold on residency (or the stall backstop), snapping the body onto the ground.
fn release_hold(
    time: Res<Time<bevy::time::Real>>,
    progress: Option<Res<WorldLoadProgress>>,
    collide: WorldCollision,
    mut clock: ResMut<SettleClock>,
    mut avatar: Query<&mut Transform, With<Avatar>>,
    mut next: ResMut<NextState<Phase>>,
    mut commands: Commands,
) {
    let (Some(p), Ok(mut body)) = (progress, avatar.single_mut()) else {
        return;
    };
    let wow = bevy_to_wow(body.translation);
    let (tx, ty) = benilla_formats::world_to_tile(wow[0], wow[1]);
    let facts = SettleFacts {
        presentable: p.presentable(),
        on_body_tile: p.focus_tile == Some((tx as i32, ty as i32)),
        counters: [
            p.ready,
            p.total,
            p.colliders_pending,
            p.merge_pending,
            p.placements_pending,
            p.gx_pending,
        ],
    };
    let now = time.elapsed_secs();
    let Some(why) = clock.step(facts, now) else {
        return;
    };
    match snap_to_ground(&collide, body.translation) {
        Some(y) => {
            info!(
                "settle: released ({why:?}) at {now:.1} s; feet snapped {:+.2} yd to wow z {:.2}",
                y - body.translation.y,
                bevy_to_wow(Vec3::new(body.translation.x, y, body.translation.z))[2]
            );
            body.translation.y = y;
        }
        None => warn!(
            "settle: released ({why:?}) at {now:.1} s but found no ground under wow {wow:?}; \
             the body keeps its spawn height"
        ),
    }
    commands.insert_resource(crate::controller::SpawnPoint(body.translation));
    next.set(if crate::menu::wanted() {
        Phase::Menu
    } else {
        Phase::Playing
    });
}

/// The ground height under `feet` (Bevy Y), from the nearest snap start that hits.
pub(crate) fn snap_to_ground(collide: &WorldCollision, feet: Vec3) -> Option<f32> {
    SNAP_STARTS.iter().find_map(|&lift| {
        let origin = feet + Vec3::Y * lift;
        collide
            .ray_body(origin, Dir3::NEG_Y, SNAP_REACH)
            .map(|hit| origin.y - hit.distance)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(presentable: bool, on_body_tile: bool, counters: [usize; 6]) -> SettleFacts {
        SettleFacts {
            presentable,
            on_body_tile,
            counters,
        }
    }

    #[test]
    fn holds_while_the_world_is_arriving() {
        let mut clock = SettleClock::new(0.0);
        assert_eq!(
            clock.step(facts(false, true, [0, 9, 4, 0, 0, 0]), 0.1),
            None
        );
        assert_eq!(
            clock.step(facts(false, true, [3, 9, 2, 0, 0, 0]), 0.2),
            None
        );
    }

    #[test]
    fn releases_when_presentable_on_the_body_tile() {
        let mut clock = SettleClock::new(0.0);
        assert_eq!(
            clock.step(facts(true, true, [9, 9, 0, 0, 0, 0]), 1.0),
            Some(ReleaseWhy::Resident)
        );
    }

    #[test]
    fn residency_of_another_tile_never_releases() {
        let mut clock = SettleClock::new(0.0);
        for i in 0..10 {
            // Counters move every frame, so the backstop never fires either.
            let f = facts(true, false, [i, 9, 0, 0, 0, 0]);
            assert_eq!(clock.step(f, i as f32), None);
        }
    }

    #[test]
    fn progress_pushes_the_deadline() {
        let mut clock = SettleClock::new(0.0);
        // Progress at t = 5 moves the deadline to 11, so t = 10 still holds.
        assert_eq!(
            clock.step(facts(false, true, [1, 9, 0, 0, 0, 0]), 5.0),
            None
        );
        assert_eq!(
            clock.step(facts(false, true, [1, 9, 0, 0, 0, 0]), 10.0),
            None
        );
        assert_eq!(
            clock.step(facts(false, true, [1, 9, 0, 0, 0, 0]), 11.0),
            Some(ReleaseWhy::Timeout)
        );
    }

    #[test]
    fn a_stalled_stream_times_out() {
        let mut clock = SettleClock::new(0.0);
        let stuck = facts(false, true, [2, 9, 1, 0, 0, 0]);
        assert_eq!(clock.step(stuck, 0.0), None);
        assert_eq!(clock.step(stuck, SETTLE_TIMEOUT - 0.01), None);
        assert_eq!(clock.step(stuck, SETTLE_TIMEOUT), Some(ReleaseWhy::Timeout));
    }
}
