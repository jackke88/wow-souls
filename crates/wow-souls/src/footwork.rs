//! Footwork: where a fighting creature stands between its swings. Before this a creature that had
//! noticed the player ran straight at it, swung, stood still for a breath and ran straight at it
//! again. A Souls enemy sizes the player up: it closes to just outside the reach of the move it
//! has in mind, circles or waits there for a beat, then commits and comes in; after a swing or a
//! blow it may step back first. Pure, like [`crate::pattern`]: the caller owns the creature, the
//! clock and the dice (a [`Foot`] keeps its own).
//!
//! What this decides is only *where the creature moves while it is engaged and not swinging*: the
//! attack table, the damage and the swings' own timing are untouched, and a swing still starts the
//! moment the player is inside its move's band and square to its nose, whatever the creature was
//! doing. So a player who walks into a circling creature is struck; one who stands off is circled.

use bevy::prelude::*;

use crate::pattern::Rng;

/// How many creatures may have committed to the player at once (closing in or swinging) within
/// [`CROWD_RANGE`] of it; the rest keep circling until a place frees. One creature never waits on
/// itself, and a lone boss is never held back.
pub const MAX_COMMITTED: usize = 2;
/// Creatures further than this from the player (yd) neither count nor wait.
pub const CROWD_RANGE: f32 = 10.0;

/// `WOWSOULS_FOOTWORK=0` gives back the straight chase.
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("WOWSOULS_FOOTWORK").map_or(true, |v| v != "0"))
}

/// A species' temperament: how it holds itself at a distance. Yards are the species' own (the
/// attack bands of its table are in them too).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Temper {
    /// How far outside the reach of the move it has in mind it stops closing and sizes up.
    pub ring: f32,
    /// How long it sizes the player up before it commits (s, shortest and longest).
    pub hesitate: (f32, f32),
    /// Of the time it sizes up, the share spent circling rather than standing.
    pub circle: f32,
    /// The chance it steps back after a swing of its own, and after a blow it took.
    pub back_after_swing: f32,
    pub back_after_hit: f32,
    /// How far it steps back (yd, least and most).
    pub back: (f32, f32),
    /// Its circling and stepping back as a share of its chase speed.
    pub pace: f32,
}

/// `Temper` with the live-check switches applied: `WOWSOULS_FOOTWORK_BACK=<0..1>` sets both chances
/// to step back, `WOWSOULS_FOOTWORK_CIRCLE=<0..1>` the share of the pause spent circling. For
/// watching a step in a scripted run; nothing else reads them.
pub fn tuned(t: &Temper) -> Temper {
    static BACK: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    static CIRCLE: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    let read = |name: &str| {
        std::env::var(name)
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .map(|v| v.clamp(0.0, 1.0))
    };
    let back = *BACK.get_or_init(|| read("WOWSOULS_FOOTWORK_BACK"));
    let circle = *CIRCLE.get_or_init(|| read("WOWSOULS_FOOTWORK_CIRCLE"));
    Temper {
        back_after_swing: back.unwrap_or(t.back_after_swing),
        back_after_hit: back.unwrap_or(t.back_after_hit),
        circle: circle.unwrap_or(t.circle),
        ..*t
    }
}

impl Temper {
    /// A creature that never sizes up: the old straight chase.
    #[cfg(test)]
    pub const STRAIGHT: Temper = Temper {
        ring: 0.0,
        hesitate: (0.0, 0.0),
        circle: 0.0,
        back_after_swing: 0.0,
        back_after_hit: 0.0,
        back: (0.0, 0.0),
        pace: 0.5,
    };
    /// The Kobold Laborer: wary, backs off after a swing, circles about half the time.
    pub const KOBOLD: Temper = Temper {
        ring: 1.5,
        hesitate: (0.5, 1.6),
        circle: 0.5,
        back_after_swing: 0.45,
        back_after_hit: 0.35,
        back: (1.5, 3.0),
        pace: 0.6,
    };
    /// The Kobold Vermin: skittish, quick to step away from a blow.
    pub const VERMIN: Temper = Temper {
        ring: 1.0,
        hesitate: (0.3, 1.0),
        circle: 0.6,
        back_after_swing: 0.3,
        back_after_hit: 0.5,
        back: (1.0, 2.0),
        pace: 0.7,
    };
    /// The Kobold Worker: it digs, it does not duel; it plods in.
    pub const WORKER: Temper = Temper {
        ring: 1.0,
        hesitate: (0.2, 0.8),
        circle: 0.25,
        back_after_swing: 0.15,
        back_after_hit: 0.15,
        back: (1.0, 2.0),
        pace: 0.5,
    };
    /// The Timber Wolf: a circler, long pauses at a few yards, then the lunge.
    pub const WOLF: Temper = Temper {
        ring: 1.4,
        hesitate: (1.0, 2.4),
        circle: 0.8,
        back_after_swing: 0.5,
        back_after_hit: 0.3,
        back: (1.5, 3.0),
        pace: 0.75,
    };
    /// The Young Wolf: the same, shorter of breath.
    pub const YOUNG_WOLF: Temper = Temper {
        ring: 1.0,
        hesitate: (0.6, 1.6),
        circle: 0.7,
        back_after_swing: 0.4,
        back_after_hit: 0.5,
        back: (1.0, 2.2),
        pace: 0.8,
    };
    /// The Defias Thug: steady, a man with a blade; a sidestep now and then.
    pub const THUG: Temper = Temper {
        ring: 1.2,
        hesitate: (0.4, 1.2),
        circle: 0.4,
        back_after_swing: 0.25,
        back_after_hit: 0.15,
        back: (1.0, 2.0),
        pace: 0.55,
    };
    /// Garrick Padfoot: a duellist, keeps his distance, steps away after his combos.
    pub const GARRICK: Temper = Temper {
        ring: 1.8,
        hesitate: (0.8, 1.8),
        circle: 0.7,
        back_after_swing: 0.6,
        back_after_hit: 0.0,
        back: (2.0, 4.0),
        pace: 0.7,
    };
    /// Hogger: a brute that comes at the player: a short look from a little way off, hardly any
    /// circling, and no step back after a swing but now and then (he is there to be fought, not
    /// to be waited out).
    pub const HOGGER: Temper = Temper {
        ring: 1.35,
        hesitate: (0.25, 0.9),
        circle: 0.3,
        back_after_swing: 0.1,
        back_after_hit: 0.0,
        back: (0.8, 1.5),
        pace: 0.75,
    };

    /// Hogger the knight presses: a short look, little circling, hardly ever a step back after his own swing.
    pub const HOGGER_KNIGHT: Temper = Temper {
        ring: 1.0,
        hesitate: (0.15, 0.5),
        circle: 0.25,
        back_after_swing: 0.05,
        back_after_hit: 0.0,
        back: (0.8, 1.5),
        pace: 0.85,
    };
}

/// What the feet are doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Beat {
    /// Nothing chosen yet.
    #[default]
    Fresh,
    /// Coming in.
    Close,
    /// Going round the player, facing it.
    Circle,
    /// Stepping away.
    Back,
    /// Standing, facing it.
    Hold,
}

/// What the creature has just been through, which its next pause answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum After {
    #[default]
    Nothing,
    Swing,
    Hit,
}

/// One creature's feet: the beat, how long it has left, and whether it has made up its mind.
#[derive(Debug, Clone, Default)]
pub struct Foot {
    pub beat: Beat,
    /// Seconds (Circle, Hold) or yards (Back) left in the beat.
    left: f32,
    /// Seconds of sizing-up left before it commits; `None` before it has reached its ring.
    hesitate: Option<f32>,
    /// It has made up its mind: it closes in until the swing.
    pub committed: bool,
    /// Which way round it circles: 1 its right (facing the player), -1 its left.
    side: f32,
    after: After,
    dice: Rng,
}

impl Foot {
    /// A foot whose dice are seeded from `seed` (the creature's id).
    pub fn seeded(seed: u64) -> Foot {
        Foot {
            dice: Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA076_1D64_78BD_642F),
            ..Foot::default()
        }
    }

    /// Whether the dice have not been seeded yet.
    pub fn is_unseeded(&self) -> bool {
        self.dice.0 == 0
    }

    /// The creature lost interest, went home or died: the next fight starts fresh.
    pub fn reset(&mut self) {
        *self = Foot {
            dice: self.dice,
            ..Foot::default()
        };
    }

    /// The creature finished a swing: it has not made up its mind again yet.
    pub fn swung(&mut self) {
        self.committed = false;
        self.beat = Beat::Fresh;
        self.hesitate = None;
        self.after = After::Swing;
    }

    /// The creature took a blow.
    pub fn hit(&mut self) {
        self.committed = false;
        self.beat = Beat::Fresh;
        self.hesitate = None;
        self.after = After::Hit;
    }

    /// A number in `lo..=hi`.
    fn between(&mut self, lo: f32, hi: f32) -> f32 {
        if hi <= lo {
            return lo;
        }
        lo + (hi - lo) * (self.dice.below(1000) as f32 / 999.0)
    }

    fn chance(&mut self, p: f32) -> bool {
        p > 0.0 && (self.dice.below(1000) as f32) < p * 1000.0
    }
}

/// Whether the player's body gives a creature an opening: it is swinging, casting or reeling from a
/// blow. A committed cast is as much a commitment as a swing, and as open to a counter.
pub fn is_open(state: &crate::combat::ActionState) -> bool {
    use crate::combat::ActionState;
    matches!(
        state,
        ActionState::Attack(_)
            | ActionState::Cast(_)
            | ActionState::Reel(_)
            | ActionState::Stagger { .. }
    )
}

/// What the creature sees this frame.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// Ground distance to the player (yd, centre to centre).
    pub dist: f32,
    /// The reach of the move it has in mind: the move's band (its own yards).
    pub band: (f32, f32),
    /// Its chase speed (yd/s).
    pub speed: f32,
    /// The player is in the middle of a swing, a roll or a stagger: open.
    pub player_open: bool,
    /// Others already hold the committed places.
    pub crowded: bool,
}

/// What the feet do this frame, in the frame of the creature facing the player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    /// Toward the player (1) or away (-1), as a share of the speed.
    pub ahead: f32,
    /// To its right (1) or left (-1) as it faces the player.
    pub across: f32,
    /// How fast, as a share of its chase speed.
    pub share: f32,
}

impl Plan {
    #[cfg(test)]
    pub const STAND: Plan = Plan {
        ahead: 0.0,
        across: 0.0,
        share: 0.0,
    };
    const CLOSE: Plan = Plan {
        ahead: 1.0,
        across: 0.0,
        share: 1.0,
    };
}

/// How much of its pace a creature that goes round may spend going in or out, as a share of the
/// sideways one: 0.25 is 14 degrees off straight sideways.
const SIDE_DRIFT: f32 = 0.25;
/// Within this of the reach of its move (yd) it does not go round the player.
pub const REACH_SLACK: f32 = 0.4;

/// The ring: the distance at which a creature with `band` in mind and `temper` sizes up.
pub fn ring(temper: &Temper, band: (f32, f32)) -> f32 {
    band.1 + temper.ring
}

/// One frame of the feet: what to do while engaged and not swinging.
pub fn step(foot: &mut Foot, temper: &Temper, look: Look, dt: f32) -> Plan {
    let outer = ring(temper, look.band);
    // A creature that never sizes up is the straight chase.
    if temper.hesitate.1 <= 0.0 && temper.back_after_swing <= 0.0 && temper.back_after_hit <= 0.0 {
        foot.committed = true;
    }
    if foot.committed {
        foot.beat = Beat::Close;
        return Plan::CLOSE;
    }
    // What it has just been through is answered first: a swing or a blow may make it want room,
    // whatever the player is doing (an open player is the reason to want it, not to forgo it).
    if foot.beat == Beat::Fresh && foot.after != After::Nothing {
        let back = match std::mem::take(&mut foot.after) {
            After::Swing => foot.chance(temper.back_after_swing),
            After::Hit => foot.chance(temper.back_after_hit),
            After::Nothing => false,
        };
        if back && temper.back.1 > 0.0 {
            foot.left = foot.between(temper.back.0, temper.back.1);
            foot.beat = Beat::Back;
        }
    }
    // The player is open: now, if it is near enough to be had (and it is not stepping away).
    if foot.beat != Beat::Back && look.player_open && look.dist <= outer + 1.5 && !look.crowded {
        foot.committed = true;
        foot.beat = Beat::Close;
        return Plan::CLOSE;
    }
    let pace = temper.pace.clamp(0.1, 1.0);
    loop {
        match foot.beat {
            Beat::Fresh => {
                if look.dist > outer + 0.25 {
                    foot.beat = Beat::Close;
                } else {
                    // At the ring: size the player up for a while.
                    let left = match foot.hesitate {
                        Some(h) => h,
                        None => {
                            let h = foot.between(temper.hesitate.0, temper.hesitate.1);
                            foot.hesitate = Some(h);
                            h
                        }
                    };
                    if left <= 0.0 {
                        if look.crowded {
                            foot.hesitate = Some(0.6);
                            continue;
                        }
                        foot.committed = true;
                        foot.beat = Beat::Close;
                        return Plan::CLOSE;
                    }
                    let length = foot.between(0.5, 1.2).min(left).max(0.15);
                    if foot.chance(temper.circle) {
                        if foot.side == 0.0 || foot.chance(0.3) {
                            foot.side = if foot.chance(0.5) { 1.0 } else { -1.0 };
                        }
                        foot.beat = Beat::Circle;
                    } else {
                        foot.beat = Beat::Hold;
                    }
                    foot.left = length;
                }
            }
            Beat::Close => {
                if look.dist <= outer {
                    // Arrived: the pause begins (the Fresh arm sets it).
                    foot.beat = Beat::Fresh;
                    continue;
                }
                return Plan::CLOSE;
            }
            Beat::Circle | Beat::Hold => {
                foot.left -= dt;
                if let Some(h) = foot.hesitate.as_mut() {
                    *h -= dt;
                }
                if foot.left <= 0.0 {
                    foot.beat = Beat::Fresh;
                    continue;
                }
                // It keeps its distance while it goes round, but only by a gentle drift: a sidestep
                // clip played on a body that travels diagonally looks crooked, so the way it goes
                // stays within [`SIDE_DRIFT`] of straight sideways. Inside the reach of its move it
                // does not go round at all: it stands and the swing comes.
                let in_reach = look.dist <= look.band.1 + REACH_SLACK;
                let error = outer - 0.3 - look.dist;
                let ahead = if in_reach {
                    0.0
                } else {
                    (-error * 0.5).clamp(-SIDE_DRIFT, SIDE_DRIFT)
                };
                return if foot.beat == Beat::Circle && !in_reach {
                    Plan {
                        ahead,
                        across: foot.side,
                        share: pace,
                    }
                } else {
                    Plan {
                        ahead,
                        across: 0.0,
                        share: if ahead.abs() > 0.05 { pace } else { 0.0 },
                    }
                };
            }
            Beat::Back => {
                let gone = pace * 1.1 * look.speed * dt;
                foot.left -= gone;
                if foot.left <= 0.0 || look.dist > outer + 1.0 {
                    foot.beat = Beat::Fresh;
                    // It sizes the player up again from where it stands.
                    foot.hesitate = None;
                    continue;
                }
                return Plan {
                    ahead: -1.0,
                    across: 0.0,
                    share: pace * 1.1,
                };
            }
        }
    }
}

/// A plan as the world's motion: the way to go (a unit vector or zero), the speed (yd/s) and the
/// way to face. `to_player` is the flat vector from the creature to the player; `strafes` is
/// whether the model has the clips to go round facing the player (else it faces the way it goes).
pub fn world_motion(plan: Plan, to_player: Vec3, chase: f32, strafes: bool) -> (Vec3, f32, Vec3) {
    let ahead = to_player.normalize_or_zero();
    // Its right as it faces the player.
    let right = ahead.cross(Vec3::Y);
    let go = (ahead * plan.ahead + right * plan.across).clamp_length_max(1.0);
    let speed = chase * plan.share.clamp(0.0, 1.0);
    let way = go.normalize_or_zero();
    let look = if plan.across != 0.0 && !strafes && plan.ahead >= 0.0 {
        way
    } else {
        ahead
    };
    (way, speed, look)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn a_committed_cast_gives_a_creature_the_opening_a_swing_does() {
        use crate::combat::{Action, CastSpec};
        let mut a = Action::default();
        assert!(!is_open(&a.state), "standing still is no opening");
        let spec = CastSpec {
            duration: 1.5,
            release: 0.7,
            recover: 1.2,
        };
        assert!(a.begin_cast(133, spec, None));
        assert!(is_open(&a.state), "a cast commits the body like a swing");
        a.stagger(0.5);
        assert!(is_open(&a.state), "a stagger is open too");
    }

    fn look(dist: f32) -> Look {
        Look {
            dist,
            band: (0.0, 2.0),
            speed: 4.0,
            player_open: false,
            crowded: false,
        }
    }

    /// Runs `secs` of frames with the player at `dist`, returns each frame's plan.
    fn run(foot: &mut Foot, t: &Temper, l: Look, secs: f32) -> Vec<Plan> {
        (0..(secs / DT) as u32)
            .map(|_| step(foot, t, l, DT))
            .collect()
    }

    #[test]
    fn far_off_it_closes_in_at_full_pace() {
        let mut f = Foot::seeded(1);
        let p = step(&mut f, &Temper::KOBOLD, look(12.0), DT);
        assert_eq!(p, Plan::CLOSE);
        assert_eq!(f.beat, Beat::Close);
    }

    #[test]
    fn at_its_ring_it_sizes_the_player_up_then_commits() {
        let t = Temper::KOBOLD;
        let mut f = Foot::seeded(7);
        let at_ring = ring(&t, (0.0, 2.0));
        // The first frames at the ring: no straight run in.
        let early = run(&mut f, &t, look(at_ring - 0.1), 0.3);
        assert!(early
            .iter()
            .all(|p| p.ahead < 0.7 && p.share <= t.pace + 1e-6));
        assert!(!f.committed);
        // Within the longest hesitation (and a beat) it has made up its mind and closes in.
        let later = run(&mut f, &t, look(at_ring - 0.1), t.hesitate.1 + 1.5);
        assert!(f.committed);
        assert_eq!(*later.last().unwrap(), Plan::CLOSE);
    }

    #[test]
    fn it_circles_with_a_side_and_keeps_its_distance_while_it_does() {
        let t = Temper {
            circle: 1.0,
            hesitate: (2.0, 2.0),
            ..Temper::WOLF
        };
        let mut f = Foot::seeded(3);
        let at_ring = ring(&t, (0.0, 2.0)) - 0.3;
        let plans = run(&mut f, &t, look(at_ring), 1.5);
        assert!(plans.iter().any(|p| p.across.abs() == 1.0), "{plans:?}");
        // On its ring there is nothing to correct: it goes round, not in or out.
        let round: Vec<_> = plans.iter().filter(|p| p.across != 0.0).collect();
        assert!(round.iter().all(|p| p.ahead.abs() < 0.2), "{round:?}");
        // Pushed too near it gives a little ground, too far it comes back, both gently.
        let near = step(&mut f, &t, look(at_ring - 0.5), DT);
        assert!(near.ahead < 0.0 || f.beat == Beat::Fresh);
        assert!(near.ahead.abs() <= SIDE_DRIFT + 1.0e-6);
    }

    #[test]
    fn going_round_is_always_nearly_straight_sideways_and_never_inside_reach() {
        // The sidestep clip is a pure sideways step: the body may not travel crooked on it.
        for t in [Temper::WOLF, Temper::THUG, Temper::GARRICK, Temper::KOBOLD] {
            let band = (0.0, 2.0);
            let outer = ring(&t, band);
            for seed in 1..8 {
                let mut f = Foot::seeded(seed);
                f.swung();
                // From well inside the ring out to past it, over a few seconds.
                for i in 0..420 {
                    let d = 0.8 + (i as f32 / 420.0) * (outer + 1.0);
                    let l = Look {
                        dist: d,
                        band,
                        speed: 4.0,
                        player_open: false,
                        crowded: false,
                    };
                    let p = step(&mut f, &t, l, DT);
                    if p.across != 0.0 {
                        assert!(
                            p.ahead.abs() <= SIDE_DRIFT * p.across.abs() + 1.0e-6,
                            "{t:?} d={d}: {p:?}"
                        );
                        assert!(d > band.1 + REACH_SLACK, "circling inside its reach at {d}");
                    }
                }
            }
        }
    }

    #[test]
    fn an_open_player_is_punished_at_once() {
        let t = Temper::WOLF;
        let mut f = Foot::seeded(9);
        let l = Look {
            player_open: true,
            ..look(ring(&t, (0.0, 2.0)) + 0.5)
        };
        assert_eq!(step(&mut f, &t, l, DT), Plan::CLOSE);
        assert!(f.committed);
        // Not when it is too far to have, nor when the places are taken.
        let mut g = Foot::seeded(9);
        let far = Look {
            player_open: true,
            ..look(30.0)
        };
        step(&mut g, &t, far, DT);
        assert!(g.beat == Beat::Close && !g.committed);
        let mut h = Foot::seeded(9);
        let crowded = Look {
            player_open: true,
            crowded: true,
            ..look(ring(&t, (0.0, 2.0)) - 0.2)
        };
        step(&mut h, &t, crowded, DT);
        assert!(!h.committed);
    }

    #[test]
    fn a_crowded_creature_keeps_circling_until_a_place_frees() {
        let t = Temper::KOBOLD;
        let mut f = Foot::seeded(11);
        let at = ring(&t, (0.0, 2.0)) - 0.2;
        let held = Look {
            crowded: true,
            ..look(at)
        };
        run(&mut f, &t, held, 6.0);
        assert!(!f.committed, "a crowded creature never commits");
        // The place frees: it commits within a beat.
        run(&mut f, &t, look(at), 2.0);
        assert!(f.committed);
    }

    #[test]
    fn after_a_swing_it_may_step_back_then_sizes_up_again() {
        let t = Temper {
            back_after_swing: 1.0,
            back: (2.0, 2.0),
            ..Temper::KOBOLD
        };
        let mut f = Foot::seeded(5);
        f.committed = true;
        f.swung();
        assert!(!f.committed);
        let first = step(&mut f, &t, look(1.4), DT);
        assert!(first.ahead < 0.0, "{first:?}");
        assert_eq!(f.beat, Beat::Back);
        // It covers its two yards at its backing pace and is done.
        let pace = t.pace * 1.1 * 4.0;
        let frames = (2.0 / pace / DT).ceil() as u32 + 3;
        for _ in 0..frames {
            step(&mut f, &t, look(1.4), DT);
        }
        assert_ne!(f.beat, Beat::Back);
    }

    #[test]
    fn every_temperament_stops_at_its_ring_on_the_approach() {
        // A creature walking in from far away, whatever its reach, sizes the player up at its
        // ring first: no species runs straight through it to its swing's band.
        for (t, band) in [
            (Temper::GARRICK, (3.0, 4.8)),
            (Temper::KOBOLD, (0.0, 3.4)),
            (Temper::WOLF, (1.6, 2.4)),
            (Temper::THUG, (0.9, 2.6)),
        ] {
            for seed in 1..6 {
                let mut f = Foot::seeded(seed);
                let outer = ring(&t, band);
                let mut dist = 15.0f32;
                let mut stopped = None;
                for i in 0..900 {
                    let l = Look {
                        dist,
                        band,
                        speed: 5.0,
                        player_open: false,
                        crowded: false,
                    };
                    let p = step(&mut f, &t, l, DT);
                    if f.beat != Beat::Close && stopped.is_none() {
                        stopped = Some((i, dist));
                    }
                    dist = (dist - p.ahead * p.share * 5.0 * DT).max(0.5);
                    if f.committed || dist <= band.1 {
                        break;
                    }
                }
                let (_, at) = stopped.expect("it sized the player up before its band");
                assert!(
                    (at - outer).abs() < 0.2,
                    "{t:?} seed {seed}: stopped at {at}, ring {outer}"
                );
            }
        }
    }

    #[test]
    fn a_step_back_is_not_cancelled_by_an_open_player() {
        let t = Temper {
            back_after_swing: 1.0,
            back: (2.0, 2.0),
            ..Temper::KOBOLD
        };
        let mut f = Foot::seeded(5);
        f.swung();
        let open = Look {
            player_open: true,
            ..look(1.4)
        };
        // Swung, and the player is swinging too: the step back comes first and holds.
        assert!(step(&mut f, &t, open, DT).ahead < 0.0);
        assert_eq!(f.beat, Beat::Back);
        assert!(step(&mut f, &t, open, DT).ahead < 0.0);
        assert!(!f.committed);
    }

    #[test]
    fn a_blow_taken_is_answered_by_the_hit_chance() {
        let always = Temper {
            back_after_hit: 1.0,
            ..Temper::KOBOLD
        };
        let never = Temper {
            back_after_hit: 0.0,
            ..Temper::KOBOLD
        };
        let mut a = Foot::seeded(2);
        a.hit();
        step(&mut a, &always, look(1.4), DT);
        assert_eq!(a.beat, Beat::Back);
        let mut b = Foot::seeded(2);
        b.hit();
        step(&mut b, &never, look(1.4), DT);
        assert_ne!(b.beat, Beat::Back);
    }

    #[test]
    fn the_same_seed_makes_the_same_dance() {
        let t = Temper::WOLF;
        let play = |seed| {
            let mut f = Foot::seeded(seed);
            f.swung();
            (0..600)
                .map(|i| {
                    let d = 1.6 + (i as f32 / 40.0).sin();
                    step(&mut f, &t, look(d + 2.0), DT)
                })
                .map(|p| (p.ahead.to_bits(), p.across.to_bits(), p.share.to_bits()))
                .collect::<Vec<_>>()
        };
        assert_eq!(play(42), play(42));
        assert_ne!(play(42), play(43));
    }

    #[test]
    fn a_straight_temper_never_sizes_up() {
        let mut f = Foot::seeded(1);
        for d in [1.0, 3.0, 6.0, 15.0] {
            assert_eq!(step(&mut f, &Temper::STRAIGHT, look(d), DT), Plan::CLOSE);
        }
        f.swung();
        assert_eq!(step(&mut f, &Temper::STRAIGHT, look(2.0), DT), Plan::CLOSE);
    }

    #[test]
    fn a_plan_becomes_a_world_move_and_a_facing() {
        // The player is north (-Z); its right as it faces north is east (+X).
        let to = Vec3::new(0.0, 0.0, -5.0);
        let circle = Plan {
            ahead: 0.0,
            across: 1.0,
            share: 0.5,
        };
        let (go, speed, face) = world_motion(circle, to, 4.0, true);
        assert!((go - Vec3::X).length() < 1e-5, "{go:?}");
        assert!((speed - 2.0).abs() < 1e-6);
        assert!(
            (face - Vec3::NEG_Z).length() < 1e-5,
            "strafing faces the player"
        );
        // A model without the shuffle clips faces the way it walks.
        let (_, _, face) = world_motion(circle, to, 4.0, false);
        assert!((face - Vec3::X).length() < 1e-5, "{face:?}");
        // Backing faces the player either way.
        let back = Plan {
            ahead: -1.0,
            across: 0.0,
            share: 0.6,
        };
        let (go, _, face) = world_motion(back, to, 4.0, false);
        assert!((go - Vec3::Z).length() < 1e-5);
        assert!((face - Vec3::NEG_Z).length() < 1e-5);
        // A stand is no motion.
        let (go, speed, _) = world_motion(Plan::STAND, to, 4.0, true);
        assert_eq!((go, speed), (Vec3::ZERO, 0.0));
    }

    #[test]
    fn the_temperaments_are_sane() {
        for t in [
            Temper::KOBOLD,
            Temper::VERMIN,
            Temper::WORKER,
            Temper::WOLF,
            Temper::YOUNG_WOLF,
            Temper::THUG,
            Temper::GARRICK,
            Temper::HOGGER,
            Temper::HOGGER_KNIGHT,
        ] {
            assert!(t.ring > 0.0 && t.hesitate.0 <= t.hesitate.1 && t.hesitate.0 > 0.0);
            assert!((0.0..=1.0).contains(&t.circle));
            assert!((0.0..=1.0).contains(&t.back_after_swing));
            assert!((0.0..=1.0).contains(&t.back_after_hit));
            assert!(t.back.0 <= t.back.1 && t.back.0 > 0.0);
            assert!(t.pace > 0.0 && t.pace <= 1.0);
        }
    }
}
