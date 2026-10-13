//! Knock-back: a blow shoves the body it strikes along the way it came, `knockbackDist` metres
//! (Dark Souls params), as a slide of [`SHOVE_TIME`] that is fastest first. The slide goes through the
//! body's `MoveIntent`, so the motor does the move and walls stop it. The flinch clips carry no
//! root motion (measured), so this is the only push a flinching body gets.

use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::controller::PlaySet;
use crate::hurt::Respawned;
use crate::motor::MoveIntent;
use crate::npc::Hostile;
use crate::settle::Phase;

/// How long the slide lasts (s). A tuning value: the data gives a distance, not a time.
pub const SHOVE_TIME: f32 = 0.30;
/// Dark Souls params are in metres, the world in yards.
pub const METRES_TO_YARDS: f32 = 1.0936;

/// A slide in progress on a body.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Shove {
    /// Horizontal, unit.
    dir: Vec3,
    /// Total yards.
    dist: f32,
    /// Seconds in.
    t: f32,
}

/// The fraction of the slide done after `u` (0..=1) of its time: `1 - (1-u)^2`.
pub fn covered(u: f32) -> f32 {
    let rest = 1.0 - u.clamp(0.0, 1.0);
    1.0 - rest * rest
}

impl Shove {
    /// A slide of `metres` along `dir` (flattened); `None` for no distance or no direction.
    pub fn new(dir: Vec3, metres: f32) -> Option<Shove> {
        let dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        (metres > 0.0 && dir != Vec3::ZERO).then_some(Shove {
            dir,
            dist: metres * METRES_TO_YARDS,
            t: 0.0,
        })
    }

    /// Runs the slide on by `dt` seconds; the yards it covers in that step.
    pub fn advance(&mut self, dt: f32) -> f32 {
        let before = covered(self.t / SHOVE_TIME);
        self.t = (self.t + dt.max(0.0)).min(SHOVE_TIME);
        (covered(self.t / SHOVE_TIME) - before) * self.dist
    }

    pub fn done(&self) -> bool {
        self.t >= SHOVE_TIME
    }
}

/// Starts a slide on `e` (a log line says how far); nothing for no distance or no direction.
pub fn start(commands: &mut Commands, e: Entity, who: &str, dir: Vec3, metres: f32) {
    if let Some(s) = Shove::new(dir, metres) {
        info!("shove: {who} {:.2} yd over {SHOVE_TIME:.2} s", s.dist);
        commands.entity(e).insert(s);
    }
}

/// One frame of a slide: the body's intent for this step; whether the slide is over. The frame
/// after its last step is the one that ends it, with an idle intent: a last tiny step must not
/// outlive the `Shove` that keeps `face_motion` from turning the body toward it.
fn drive(shove: &mut Shove, intent: &mut MoveIntent, dt: f32) -> bool {
    if shove.done() {
        *intent = MoveIntent::default();
        return true;
    }
    let step = shove.advance(dt);
    *intent = crate::combat::root_step(shove.dir, step, dt);
    false
}

/// The player's slide, between reading input and moving the body.
fn push_avatar(
    mut commands: Commands,
    time: Res<Time>,
    mut bodies: Query<(Entity, &mut Shove, &mut MoveIntent), With<Avatar>>,
) {
    for (e, mut shove, mut intent) in &mut bodies {
        if drive(&mut shove, &mut intent, time.delta_secs()) {
            commands.entity(e).remove::<Shove>();
        }
    }
}

/// A creature's slide, between its thinking and its motor step.
pub fn push_hostiles(
    mut commands: Commands,
    time: Res<Time>,
    mut bodies: Query<(Entity, &mut Shove, &mut MoveIntent), With<Hostile>>,
) {
    for (e, mut shove, mut intent) in &mut bodies {
        if drive(&mut shove, &mut intent, time.delta_secs()) {
            commands.entity(e).remove::<Shove>();
        }
    }
}

/// A respawn ends every slide.
fn clear(
    mut commands: Commands,
    mut respawned: MessageReader<Respawned>,
    shoved: Query<Entity, With<Shove>>,
) {
    if respawned.read().count() == 0 {
        return;
    }
    for e in &shoved {
        commands.entity(e).remove::<Shove>();
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        push_avatar
            .after(PlaySet::Intent)
            .before(PlaySet::Motor)
            .run_if(in_state(Phase::Playing)),
    )
    .add_systems(Update, clear.run_if(in_state(Phase::Playing)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slide_covers_its_whole_distance_at_any_frame_rate() {
        for dt in [1.0 / 144.0, 1.0 / 60.0, 1.0 / 30.0, 0.11] {
            let mut s = Shove::new(Vec3::X, 0.72).unwrap();
            let mut total = 0.0;
            let mut frames = 0;
            while !s.done() {
                total += s.advance(dt);
                frames += 1;
                assert!(frames < 1000);
            }
            assert!(
                (total - 0.72 * METRES_TO_YARDS).abs() < 1.0e-4,
                "dt {dt}: {total}"
            );
        }
    }

    #[test]
    fn the_push_is_fastest_first() {
        let mut s = Shove::new(Vec3::X, 1.0).unwrap();
        let first = s.advance(0.05);
        s.advance(0.15);
        let last = s.advance(0.05);
        assert!(first > 3.0 * last, "{first} vs {last}");
        assert!(covered(0.0) == 0.0 && (covered(1.0) - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn a_shove_needs_a_distance_and_a_way_and_stays_flat() {
        assert!(Shove::new(Vec3::X, 0.0).is_none());
        assert!(Shove::new(Vec3::ZERO, 1.0).is_none());
        assert!(Shove::new(Vec3::Y, 1.0).is_none(), "straight up is no way");
        let s = Shove::new(Vec3::new(3.0, 5.0, 0.0), 1.0).unwrap();
        assert_eq!(s.dir, Vec3::X);
    }

    #[test]
    fn driving_a_slide_asks_the_motor_for_each_step_and_ends_it() {
        let mut s = Shove::new(Vec3::NEG_Z, 0.45).unwrap();
        let mut intent = MoveIntent::default();
        let dt = 1.0 / 60.0;
        let mut moved = 0.0;
        let mut ended = false;
        for _ in 0..40 {
            ended = drive(&mut s, &mut intent, dt);
            if intent.speed != 0.0 {
                assert_eq!(intent.dir, Vec3::NEG_Z);
            }
            moved += intent.speed * dt;
            if ended {
                break;
            }
        }
        assert!(ended);
        assert_eq!(intent.speed, 0.0, "the ending frame is idle");
        assert!((moved - 0.45 * METRES_TO_YARDS).abs() < 1.0e-3, "{moved}");
    }
}
