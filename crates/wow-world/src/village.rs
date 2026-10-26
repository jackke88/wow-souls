//! Local road traffic, sampled from the saved clock while absent and physically
//! walked near the player. Every connection follows an authored native road edge.
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
pub type Point = [f32; 3];
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoadGraph {
    pub nodes: Vec<Point>,
    /// Undirected, explicitly validated walking connections.
    pub edges: Vec<[usize; 2]>,
}

#[derive(Clone, Copy, PartialEq)]
struct RoadQueue {
    cost: f64,
    node: usize,
}

impl Eq for RoadQueue {}

impl Ord for RoadQueue {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max heap. Reverse both keys to visit the cheapest node
        // first, with a stable node-index tie break for equal-length road routes.
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.node.cmp(&self.node))
    }
}

impl PartialOrd for RoadQueue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl RoadGraph {
    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.iter().any(|p| !finite(*p)) {
            return Err("road graph contains a non-finite point".into());
        }
        if self
            .edges
            .iter()
            .any(|edge| edge.iter().any(|i| *i >= self.nodes.len()))
        {
            return Err("road graph edge references a missing node".into());
        }
        Ok(())
    }

    /// Shortest polyline along graph edges. Attachments must already be graph nodes
    /// (within 5 cm for text round-tripping); disconnected roads are an error.
    pub fn shortest_path(&self, from: Point, to: Point) -> Result<Vec<Point>, String> {
        self.validate()?;
        let start = self.node(from)?;
        let end = self.node(to)?;
        let mut costs = vec![f64::INFINITY; self.nodes.len()];
        let mut previous = vec![None; self.nodes.len()];
        let mut adjacent = vec![Vec::new(); self.nodes.len()];
        for &[a, b] in &self.edges {
            let length = distance(self.nodes[a], self.nodes[b]);
            adjacent[a].push((b, length));
            adjacent[b].push((a, length));
        }
        costs[start] = 0.0;
        let mut pending = BinaryHeap::from([RoadQueue {
            cost: 0.0,
            node: start,
        }]);
        loop {
            let Some(RoadQueue {
                node: current,
                cost,
            }) = pending.pop()
            else {
                return Err(format!("no road connection from {from:?} to {to:?}"));
            };
            if cost > costs[current] {
                continue; // A shorter connection replaced this queued candidate.
            }
            if current == end {
                break;
            }
            for &(neighbor, length) in &adjacent[current] {
                let cost = costs[current] + length;
                if cost < costs[neighbor] {
                    costs[neighbor] = cost;
                    previous[neighbor] = Some(current);
                    pending.push(RoadQueue {
                        cost,
                        node: neighbor,
                    });
                }
            }
        }
        let mut indices = vec![end];
        while *indices.last().expect("the end node exists") != start {
            indices.push(
                previous[*indices.last().expect("path is nonempty")]
                    .ok_or_else(|| "road path predecessor is missing".to_string())?,
            );
        }
        indices.reverse();
        let mut points = vec![from];
        for index in indices {
            push_point(&mut points, self.nodes[index]);
        }
        push_point(&mut points, to);
        Ok(points)
    }

    fn node(&self, point: Point) -> Result<usize, String> {
        if !finite(point) {
            return Err("road attachment is not finite".into());
        }
        self.nodes
            .iter()
            .position(|p| distance(*p, point) <= 0.05)
            .ok_or_else(|| format!("road attachment {point:?} is not a graph node"))
    }
}

#[derive(Debug, Clone)]
pub struct Journey {
    pub guid: u32,
    /// Ordered stops; the last stop connects back to the first along the graph.
    pub stops: Vec<Point>,
    pub pause_seconds: f64,
    /// Individual offset in real seconds, so people do not march together.
    pub phase: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub position: Point,
    pub target: Point,
    pub heading: f32,
    pub moving: bool,
    pub leg: usize,
    pub waypoint: usize,
}

#[derive(Debug)]
struct Leg {
    start: f64,
    duration: f64,
    points: Vec<Point>,
    cumulative: Vec<f64>,
}
impl Leg {
    fn new(start: f64, duration: f64, points: Vec<Point>) -> Self {
        let mut cumulative = vec![0.0];
        for pair in points.windows(2) {
            cumulative.push(cumulative.last().copied().unwrap() + distance(pair[0], pair[1]));
        }
        Self {
            start,
            duration,
            points,
            cumulative,
        }
    }
    fn sample(&self, seconds: f64, leg: usize) -> Sample {
        let length = *self.cumulative.last().unwrap();
        let moving = length > 1e-6;
        let along = if moving {
            (seconds / self.duration).clamp(0.0, 1.0) * length
        } else {
            0.0
        };
        let waypoint = if moving {
            self.cumulative
                .iter()
                .position(|d| *d > along)
                .unwrap_or(self.points.len() - 1)
        } else {
            0
        };
        let (position, heading) = if waypoint > 0 {
            let a = self.points[waypoint - 1];
            let b = self.points[waypoint];
            let edge = self.cumulative[waypoint] - self.cumulative[waypoint - 1];
            let t = ((along - self.cumulative[waypoint - 1]) / edge).clamp(0.0, 1.0);
            (
                std::array::from_fn(|i| (f64::from(a[i]) + f64::from(b[i] - a[i]) * t) as f32),
                (b[1] - a[1]).atan2(b[0] - a[0]),
            )
        } else {
            (self.points[0], 0.0)
        };
        Sample {
            position,
            target: self.points[waypoint],
            heading,
            moving,
            leg,
            waypoint,
        }
    }
}
#[derive(Debug)]
struct Circuit {
    phase: f64,
    period: f64,
    legs: Vec<Leg>,
}

#[derive(Debug)]
pub struct Traffic {
    circuits: BTreeMap<u32, Circuit>,
}
impl Traffic {
    pub fn new(roads: &RoadGraph, journeys: &[Journey], speed: f32) -> Result<Self, String> {
        roads.validate()?;
        if !speed.is_finite() || speed <= 0.0 {
            return Err("traffic speed must be positive".into());
        }
        let mut circuits = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for journey in journeys {
            if !ids.insert(journey.guid)
                || journey.stops.len() < 2
                || !journey.phase.is_finite()
                || !journey.pause_seconds.is_finite()
                || journey.pause_seconds <= 0.0
            {
                return Err("invalid or duplicate road traveller".into());
            }
            let mut legs = Vec::new();
            let mut time = 0.0;
            for (index, &stop) in journey.stops.iter().enumerate() {
                roads.node(stop)?;
                legs.push(Leg::new(time, journey.pause_seconds, vec![stop]));
                time += journey.pause_seconds;
                let next = journey.stops[(index + 1) % journey.stops.len()];
                let path = roads.shortest_path(stop, next)?;
                let duration =
                    path.windows(2).map(|p| distance(p[0], p[1])).sum::<f64>() / f64::from(speed);
                if duration <= 1e-6 {
                    return Err("traffic stops must be distinct".into());
                }
                legs.push(Leg::new(time, duration, path));
                time += duration;
            }
            circuits.insert(
                journey.guid,
                Circuit {
                    phase: journey.phase,
                    period: time,
                    legs,
                },
            );
        }
        Ok(Self { circuits })
    }
    pub fn guids(&self) -> impl Iterator<Item = u32> + '_ {
        self.circuits.keys().copied()
    }
    pub fn contains(&self, guid: u32) -> bool {
        self.circuits.contains_key(&guid)
    }
    pub fn waypoints(&self, guid: u32, leg: usize) -> Option<&[Point]> {
        self.circuits
            .get(&guid)?
            .legs
            .get(leg)
            .map(|leg| leg.points.as_slice())
    }
    /// Absolute elapsed seconds, including complete saved days. Individual circuits
    /// need not divide a day; crossing midnight must not reset a pedestrian's route.
    pub fn sample(&self, guid: u32, elapsed: f64) -> Option<Sample> {
        if !elapsed.is_finite() {
            return None;
        }
        let circuit = self.circuits.get(&guid)?;
        let phase = (elapsed + circuit.phase).rem_euclid(circuit.period);
        let (index, leg) = circuit
            .legs
            .iter()
            .enumerate()
            .find(|(_, leg)| phase < leg.start + leg.duration)?;
        Some(leg.sample(phase - leg.start, index))
    }
}
fn finite(point: Point) -> bool {
    point.into_iter().all(f32::is_finite)
}
fn distance(a: Point, b: Point) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(a, b)| (f64::from(b) - f64::from(a)).powi(2))
        .sum::<f64>()
        .sqrt()
}
fn push_point(points: &mut Vec<Point>, point: Point) {
    if points.last().is_none_or(|p| distance(*p, point) > 1e-6) {
        points.push(point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn graph() -> RoadGraph {
        RoadGraph {
            nodes: vec![[0.0, 0.0, 0.0], [30.0, 0.0, 0.0], [30.0, 40.0, 0.0]],
            edges: vec![[0, 1], [1, 2]],
        }
    }
    fn traffic() -> Traffic {
        let g = graph();
        Traffic::new(
            &g,
            &[Journey {
                guid: 7,
                stops: vec![g.nodes[0], g.nodes[2]],
                pause_seconds: 5.0,
                phase: 0.0,
            }],
            2.5,
        )
        .unwrap()
    }
    #[test]
    fn routes_preserve_native_corners_and_never_cut_across_fields() {
        let t = traffic();
        assert_eq!(t.waypoints(7, 1).unwrap(), graph().nodes);
        assert_eq!(
            t.waypoints(7, 3).unwrap(),
            graph().nodes.into_iter().rev().collect::<Vec<_>>()
        );
        let s = t.sample(7, 19.0).unwrap();
        assert_eq!(s.position, [30.0, 5.0, 0.0]);
        assert_eq!(s.target, [30.0, 40.0, 0.0]);
    }
    #[test]
    fn walking_speed_pauses_and_cycle_seams_are_continuous() {
        let t = traffic();
        assert!(!t.sample(7, 4.99).unwrap().moving);
        assert!(t.sample(7, 5.01).unwrap().moving);
        for n in 1..6600 {
            let a = t.sample(7, f64::from(n - 1) * 0.01).unwrap();
            let b = t.sample(7, f64::from(n) * 0.01).unwrap();
            assert!(distance(a.position, b.position) <= 0.0251);
        }
        assert_eq!(t.sample(7, 0.0), t.sample(7, 66.0));
    }
    #[test]
    fn clock_sampling_is_deterministic_including_absent_days() {
        let t = traffic();
        assert_eq!(t.sample(7, 1234567.0), t.sample(7, 1234567.0));
        assert!(
            distance(
                t.sample(7, 1439.99).unwrap().position,
                t.sample(7, 1440.01).unwrap().position
            ) < 0.051
        );
        assert!(t.sample(7, f64::NAN).is_none());
    }
    #[test]
    fn invalid_and_disconnected_routes_are_rejected() {
        let mut g = graph();
        g.edges.pop();
        let j = Journey {
            guid: 1,
            stops: vec![g.nodes[0], g.nodes[2]],
            pause_seconds: 5.0,
            phase: 0.0,
        };
        assert!(Traffic::new(&g, std::slice::from_ref(&j), 2.5).is_err());
        assert!(Traffic::new(&graph(), &[j.clone(), j], 2.5).is_err());
        assert!(Traffic::new(&graph(), &[], f32::NAN).is_err());
        assert!(graph()
            .shortest_path([1.0, 1.0, 0.0], [30.0, 40.0, 0.0])
            .is_err());
    }
}
