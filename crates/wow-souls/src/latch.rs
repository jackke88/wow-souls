//! The spiders' latch: a leap that lands holds on, chews, and lets go with a hop back.
//!
//! A kit move named `leap` that connects does not knock the player away. The spider's [`crate::npc::Brain`]
//! takes a [`Latch`]: the swing's clock stops, the creature stays against the player where its blow landed
//! (the offset it had then) and is carried along with them, chewing (the claw bite's strike, looped) every
//! [`CHEW_EVERY`] s for a [`CHEW_SHARE`] of the leap's damage, with no flinch, while the player is slowed
//! ([`Snare`]). It lets go when [`LATCH_TIME`] is up (then it hops back, `hop_back`, before it comes again),
//! when the player rolls or dies, or when a blow knocks it off (any flinch of the creature ends its swing).
//! A leap that misses just lands: the creature is wide open.

use bevy::prelude::*;

/// How long a spider hangs on (s).
pub const LATCH_TIME: f32 = 2.0;
/// The time between its bites while it holds (s); the leap's own blow is the first.
pub const CHEW_EVERY: f32 = 0.55;
/// A chew's share of the leap's damage.
pub const CHEW_SHARE: f32 = 0.5;
/// The player's speed while a spider hangs on, as a share of the usual.
pub const SNARE_SPEED: f32 = 0.45;
/// How far from the player's feet (yd) the spider sits while it holds on, and how far above them: at the
/// chest, not on the ground beside them.
pub const HOLD_REACH: f32 = 0.8;
pub const HOLD_RISE: f32 = 0.7;
/// The part of the bite clip that is chewed: it starts here (s) and runs for the span.
const CHEW_FROM: f32 = 0.55;
const CHEW_SPAN: f32 = 0.40;

/// Whether a kit move that lands holds on.
pub fn latches(move_name: &str) -> bool {
    move_name == "leap"
}

/// A spider hanging on to the player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Latch {
    /// Where the creature stands against the player (world axes): it keeps that as they move.
    pub offset: Vec3,
    /// How long it has held (s).
    pub elapsed: f32,
    chews: u32,
}

impl Latch {
    pub fn new(offset: Vec3) -> Self {
        Latch {
            offset,
            elapsed: 0.0,
            chews: 0,
        }
    }

    /// Holds on for `dt` more; how many chews fall due in it.
    pub fn step(&mut self, dt: f32) -> u32 {
        self.elapsed = (self.elapsed + dt).min(LATCH_TIME);
        let due = (self.elapsed / CHEW_EVERY).floor() as u32;
        let new = due.saturating_sub(self.chews);
        self.chews += new;
        new
    }

    /// Where the creature's feet are while it holds on to a player standing at `player`: on the side it
    /// landed on, at the chest.
    pub fn hold(&self, player: Vec3) -> Vec3 {
        let side = Vec3::new(self.offset.x, 0.0, self.offset.z).normalize_or_zero();
        player + side * HOLD_REACH + Vec3::Y * HOLD_RISE
    }

    /// The time is up.
    pub fn done(&self) -> bool {
        self.elapsed >= LATCH_TIME
    }

    /// The second of the bite clip it shows now: the chew's wind-up and strike over and over, the strike
    /// at the moment a chew lands.
    pub fn chew_pose(&self) -> f32 {
        CHEW_FROM + CHEW_SPAN * (self.elapsed / CHEW_EVERY).fract()
    }
}

/// The player's speed while spiders hang on: 1 free, [`SNARE_SPEED`] held.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Snare(pub f32);

impl Default for Snare {
    fn default() -> Self {
        Snare(1.0)
    }
}

fn snare(
    brains: Query<(&crate::npc::Brain, &crate::npc::Reaction)>,
    rooted: Option<Res<crate::named::Rooted>>,
    mut snare: ResMut<Snare>,
) {
    // A dead spider's latch is gone with it.
    let held = brains
        .iter()
        .any(|(b, r)| b.latch.is_some() && !matches!(r, crate::npc::Reaction::Dead));
    // A net or a web holds the feet still ([`crate::named`]).
    let want = if rooted.is_some_and(|r| r.0 > 0.0) {
        0.0
    } else if held {
        SNARE_SPEED
    } else {
        1.0
    };
    if snare.0 != want {
        snare.0 = want;
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Snare>().add_systems(
        Update,
        snare.run_if(in_state(crate::settle::Phase::Playing)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_leap_holds_on() {
        assert!(latches("leap"));
        for other in ["bite", "lunge", "hop_back", "charge"] {
            assert!(!latches(other), "{other}");
        }
    }

    #[test]
    fn it_chews_every_beat_and_lets_go_when_the_time_is_up() {
        let mut l = Latch::new(Vec3::new(0.0, 0.0, 1.0));
        let mut chews = 0;
        let mut steps = 0;
        while !l.done() {
            chews += l.step(1.0 / 60.0);
            steps += 1;
            assert!(steps < 1000, "never lets go");
        }
        // 2.0 s of 0.55 s beats: chews at 0.55, 1.10 and 1.65.
        assert_eq!(chews, 3);
        assert!((l.elapsed - LATCH_TIME).abs() < 1.0e-4);
        // Once done it chews no more.
        assert_eq!(l.step(1.0), 0);
    }

    #[test]
    fn a_long_frame_still_counts_every_chew_once() {
        let mut l = Latch::new(Vec3::ZERO);
        assert_eq!(l.step(1.2), 2);
        assert_eq!(l.step(0.1), 0);
        assert_eq!(l.step(5.0), 1);
    }

    #[test]
    fn the_chew_pose_stays_inside_the_bite_and_strikes_on_the_beat() {
        let mut l = Latch::new(Vec3::ZERO);
        for _ in 0..200 {
            l.step(0.01);
            let pose = l.chew_pose();
            assert!(
                (CHEW_FROM..CHEW_FROM + CHEW_SPAN).contains(&pose),
                "pose {pose} at {}",
                l.elapsed
            );
        }
        // Just before a beat the pose is at the end of the strike.
        let mut l = Latch::new(Vec3::ZERO);
        l.step(CHEW_EVERY - 0.001);
        assert!(l.chew_pose() > CHEW_FROM + CHEW_SPAN * 0.99);
    }

    #[test]
    fn it_hangs_at_the_players_chest_on_the_side_it_landed() {
        let l = Latch::new(Vec3::new(-3.0, 0.4, 0.0));
        let player = Vec3::new(10.0, 2.0, 5.0);
        let at = l.hold(player);
        assert!((at.x - (10.0 - HOLD_REACH)).abs() < 1.0e-5 && (at.z - 5.0).abs() < 1.0e-5);
        assert!((at.y - (2.0 + HOLD_RISE)).abs() < 1.0e-5);
        // Landed right on top of the player: it still has a place.
        assert!(Latch::new(Vec3::ZERO).hold(player).is_finite());
    }

    // The snare slows but never roots: 0 < SNARE_SPEED < 1, checked when the crate compiles.
    const _: () = assert!(SNARE_SPEED > 0.0 && SNARE_SPEED < 1.0);

    #[test]
    fn the_snare_starts_free() {
        assert_eq!(Snare::default().0, 1.0);
    }
}
