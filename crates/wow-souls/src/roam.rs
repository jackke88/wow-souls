//! Where a creature walks while nothing is after it: nothing stands still unless the database says
//! so (`creature.movement_type`): type 0 stands at its post, type 1 wanders at random inside
//! `wander_distance` yards of its spawn point, type 2 walks the waypoints of its path and loops.

use crate::pattern::Rng;
use std::sync::Arc;

/// How close counts as arrived (yd).
pub const ARRIVE: f32 = 0.6;
/// A wanderer pauses this long (s) at each stop: `PAUSE.0 ..= PAUSE.1`.
pub const PAUSE: (f32, f32) = (3.0, 9.0);
/// A new wander target is at least this far (yd) from where the creature stands, if the area allows.
const MIN_LEG: f32 = 2.0;
/// How far a path walker may stray from its line before it counts as lost (yd).
pub const PATH_SLACK: f32 = 2.5;
/// A wanderer that has not reached its target in this long (s) gives it up and pauses: a point behind
/// a tree or a rock would otherwise hold it walking on the spot for ever.
pub const GIVE_UP: f32 = 20.0;

/// One waypoint: where (Bevy ground `[x, z]`) and how long to wait there (s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    pub at: [f32; 2],
    pub wait: f32,
}

/// The area a creature belongs to.
#[derive(Debug, Clone, PartialEq)]
pub enum Area {
    Stand,
    /// Random wander inside this radius (yd) of its home.
    Wander(f32),
    /// Walk these nodes in order, and from the last back to the first.
    Path(Arc<[Node]>),
}

/// The walker's own bookkeeping.
#[derive(Debug, Clone, Default)]
pub struct Walk {
    target: Option<[f32; 2]>,
    /// How long it has walked toward `target` (s).
    leg: f32,
    pause: f32,
    next: usize,
}

impl Walk {
    /// A fresh walker that first stands a random part of a pause, so creatures spawned together do not
    /// all set off on the same frame.
    pub fn staggered(rng: &mut Rng) -> Walk {
        Walk {
            pause: PAUSE.1 * unit(rng),
            ..Walk::default()
        }
    }
}

/// Whether the area is a post: the creature stands at its home and turns back to the way it was
/// placed (a zero-radius wander and an empty path are posts too).
pub fn is_post(area: &Area) -> bool {
    match area {
        Area::Stand => true,
        Area::Wander(radius) => *radius <= 0.0,
        Area::Path(nodes) => nodes.is_empty(),
    }
}

fn unit(rng: &mut Rng) -> f32 {
    rng.next_u32() as f32 / 4_294_967_296.0
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// One frame of roaming: the ground point to walk toward, or `None` to stand (a pause, a post).
pub fn step(
    w: &mut Walk,
    area: &Area,
    home: [f32; 2],
    pos: [f32; 2],
    dt: f32,
    rng: &mut Rng,
) -> Option<[f32; 2]> {
    match area {
        Area::Stand => None,
        Area::Wander(radius) if *radius <= 0.0 => None,
        Area::Wander(radius) => {
            if w.pause > 0.0 {
                w.pause -= dt;
                return None;
            }
            if let Some(t) = w.target {
                w.leg += dt;
                if dist(pos, t) > ARRIVE && w.leg < GIVE_UP {
                    return Some(t);
                }
                w.target = None;
                w.leg = 0.0;
                w.pause = PAUSE.0 + (PAUSE.1 - PAUSE.0) * unit(rng);
                return None;
            }
            // Pick a point inside the area, preferably a real walk away.
            let mut pick = home;
            for _ in 0..6 {
                let angle = unit(rng) * std::f32::consts::TAU;
                let r = unit(rng).sqrt() * radius;
                pick = [home[0] + r * angle.cos(), home[1] + r * angle.sin()];
                if dist(pos, pick) >= MIN_LEG.min(*radius) {
                    break;
                }
            }
            w.target = Some(pick);
            Some(pick)
        }
        Area::Path(nodes) => {
            if nodes.is_empty() {
                return None;
            }
            if w.pause > 0.0 {
                w.pause -= dt;
                return None;
            }
            let i = w.next % nodes.len();
            let node = nodes[i];
            if dist(pos, node.at) > ARRIVE {
                return Some(node.at);
            }
            w.pause = node.wait;
            w.next = (i + 1) % nodes.len();
            if w.pause > 0.0 {
                None
            } else {
                Some(nodes[w.next].at)
            }
        }
    }
}

/// How far a creature at `pos` is from where it belongs (yd) and the ground point to walk to when it
/// has to go back: 0 inside its area. A wanderer is at home anywhere in its radius, a path walker
/// anywhere within [`PATH_SLACK`] of the polyline through its nodes (closed from the last node to the
/// first); a creature at a post stands at `home`.
pub fn home_distance(area: &Area, home: [f32; 2], pos: [f32; 2]) -> (f32, [f32; 2]) {
    match area {
        Area::Stand => (dist(pos, home), home),
        Area::Wander(radius) => ((dist(pos, home) - radius).max(0.0), home),
        Area::Path(nodes) if nodes.is_empty() => (dist(pos, home), home),
        Area::Path(nodes) => {
            let mut best = (f32::MAX, nodes[0].at);
            for i in 0..nodes.len() {
                let a = nodes[i].at;
                let b = nodes[(i + 1) % nodes.len()].at;
                let p = nearest_on_segment(a, b, pos);
                let d = dist(pos, p);
                if d < best.0 {
                    best = (d, p);
                }
            }
            ((best.0 - PATH_SLACK).max(0.0), best.1)
        }
    }
}

fn nearest_on_segment(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> [f32; 2] {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 < 1.0e-6 {
        return a;
    }
    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0);
    [a[0] + ab[0] * t, a[1] + ab[1] * t]
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 30.0;

    /// Walk a creature for `secs` seconds at `speed` yd/s toward whatever `step` asks.
    fn simulate(area: &Area, home: [f32; 2], secs: f32, speed: f32) -> (Vec<[f32; 2]>, Walk) {
        let mut rng = Rng(7);
        let mut walk = Walk::default();
        let mut pos = home;
        let mut trail = vec![pos];
        for _ in 0..(secs / DT) as usize {
            if let Some(t) = step(&mut walk, area, home, pos, DT, &mut rng) {
                let d = dist(pos, t);
                if d > 1.0e-6 {
                    let stride = (speed * DT).min(d);
                    pos = [
                        pos[0] + (t[0] - pos[0]) / d * stride,
                        pos[1] + (t[1] - pos[1]) / d * stride,
                    ];
                }
            }
            trail.push(pos);
        }
        (trail, walk)
    }

    #[test]
    fn a_post_never_moves() {
        let (trail, _) = simulate(&Area::Stand, [10.0, 20.0], 60.0, 2.5);
        assert!(trail.iter().all(|p| *p == [10.0, 20.0]));
        let (trail, _) = simulate(&Area::Wander(0.0), [10.0, 20.0], 60.0, 2.5);
        assert!(
            trail.iter().all(|p| *p == [10.0, 20.0]),
            "radius 0 is a post"
        );
    }

    #[test]
    fn a_wanderer_roams_inside_its_radius_and_pauses() {
        let home = [100.0, -50.0];
        let (trail, _) = simulate(&Area::Wander(7.0), home, 600.0, 2.5);
        let farthest = trail.iter().map(|p| dist(*p, home)).fold(0.0, f32::max);
        assert!(farthest <= 7.0 + 0.01, "strayed {farthest} yd");
        assert!(farthest > 3.0, "it did walk (farthest {farthest})");
        // It stands for stretches: some consecutive frames are identical.
        let still = trail.windows(2).filter(|w| w[0] == w[1]).count();
        assert!(
            still > trail.len() / 4,
            "pauses: {still} of {}",
            trail.len()
        );
    }

    #[test]
    fn a_path_walker_visits_its_nodes_in_order_waits_and_loops() {
        let nodes: Arc<[Node]> = Arc::from(vec![
            Node {
                at: [0.0, 0.0],
                wait: 2.0,
            },
            Node {
                at: [10.0, 0.0],
                wait: 0.0,
            },
            Node {
                at: [10.0, 10.0],
                wait: 1.0,
            },
        ]);
        let area = Area::Path(nodes.clone());
        let (trail, _) = simulate(&area, [0.0, 0.0], 40.0, 2.5);
        // Every node is reached.
        for n in nodes.iter() {
            assert!(
                trail.iter().any(|p| dist(*p, n.at) <= ARRIVE + 0.01),
                "never reached {:?}",
                n.at
            );
        }
        // It loops: after the last node it is back near the first.
        let last = trail
            .iter()
            .rposition(|p| dist(*p, nodes[2].at) <= ARRIVE + 0.01)
            .unwrap();
        assert!(
            trail[last..]
                .iter()
                .any(|p| dist(*p, nodes[0].at) <= ARRIVE + 0.01),
            "it did not walk back to the first node"
        );
        // It waits 2 s at the first node: at least 60 identical frames in a row somewhere.
        let mut run = 0;
        let mut longest = 0;
        for w in trail.windows(2) {
            run = if w[0] == w[1] { run + 1 } else { 0 };
            longest = longest.max(run);
        }
        assert!(longest >= 55, "longest pause {longest} frames");
    }

    #[test]
    fn an_empty_path_stands() {
        let (trail, _) = simulate(&Area::Path(Arc::from(Vec::new())), [1.0, 1.0], 10.0, 2.5);
        assert!(trail.iter().all(|p| *p == [1.0, 1.0]));
    }

    #[test]
    fn posts_are_the_areas_that_never_walk() {
        assert!(is_post(&Area::Stand));
        assert!(is_post(&Area::Wander(0.0)));
        assert!(is_post(&Area::Path(Arc::from(Vec::new()))));
        assert!(!is_post(&Area::Wander(5.0)));
        let one: Arc<[Node]> = Arc::from(vec![Node {
            at: [0.0, 0.0],
            wait: 0.0,
        }]);
        assert!(!is_post(&Area::Path(one)));
    }

    #[test]
    fn a_wanderer_that_cannot_reach_its_target_gives_it_up() {
        // Pinned in place (a rock in the way): it asks for its target until GIVE_UP, then pauses.
        let mut rng = Rng(3);
        let mut walk = Walk::default();
        let home = [0.0, 0.0];
        let first =
            step(&mut walk, &Area::Wander(8.0), home, home, DT, &mut rng).expect("it sets off");
        let mut asked = 0.0;
        while let Some(t) = step(&mut walk, &Area::Wander(8.0), home, home, DT, &mut rng) {
            assert_eq!(t, first, "the same target while it tries");
            asked += DT;
            assert!(asked < GIVE_UP + 1.0, "it never gave up");
        }
        assert!(
            (asked - GIVE_UP).abs() < 2.0 * DT,
            "gave up after {asked} s"
        );
        // Then it pauses before it picks again.
        assert_eq!(
            step(&mut walk, &Area::Wander(8.0), home, home, DT, &mut rng),
            None
        );
    }

    #[test]
    fn staggered_walkers_set_off_at_different_times() {
        let mut rng = Rng(11);
        let starts: Vec<usize> = (0..5)
            .map(|_| {
                let mut w = Walk::staggered(&mut rng);
                let mut frames = 0;
                let mut r2 = Rng(5);
                while step(&mut w, &Area::Wander(5.0), [0.0; 2], [0.0; 2], DT, &mut r2).is_none() {
                    frames += 1;
                    assert!(frames < 1000);
                }
                frames
            })
            .collect();
        assert!(starts.iter().all(|&f| (f as f32) * DT <= PAUSE.1 + DT));
        let mut distinct = starts.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert!(distinct.len() >= 4, "{starts:?}");
    }

    #[test]
    fn home_distance_is_zero_inside_the_area_and_points_back_outside_it() {
        let home = [0.0, 0.0];
        assert_eq!(home_distance(&Area::Stand, home, [3.0, 4.0]).0, 5.0);
        let (d, back) = home_distance(&Area::Wander(7.0), home, [3.0, 4.0]);
        assert_eq!((d, back), (0.0, home));
        let (d, _) = home_distance(&Area::Wander(7.0), home, [9.0, 12.0]);
        assert!((d - 8.0).abs() < 1e-4, "{d}");
        let nodes: Arc<[Node]> = Arc::from(vec![
            Node {
                at: [0.0, 0.0],
                wait: 0.0,
            },
            Node {
                at: [20.0, 0.0],
                wait: 0.0,
            },
        ]);
        // Beside the middle of the line, 1 yd off: inside the slack.
        assert_eq!(
            home_distance(&Area::Path(nodes.clone()), home, [10.0, 1.0]).0,
            0.0
        );
        // 10 yd off the line: it must walk back to the point on the line below it.
        let (d, back) = home_distance(&Area::Path(nodes), home, [10.0, 10.0]);
        assert!((d - (10.0 - PATH_SLACK)).abs() < 1e-3, "{d}");
        assert!(
            (back[0] - 10.0).abs() < 1e-3 && back[1].abs() < 1e-3,
            "{back:?}"
        );
    }
}
