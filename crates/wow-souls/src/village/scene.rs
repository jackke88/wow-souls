//! Measured native-road graph and six independently phased background travellers.
use anyhow::{anyhow, bail, Result};
use std::collections::BTreeMap;
use wow_world::village::{Journey, Point, RoadGraph};

pub(super) struct SceneData {
    pub roads: RoadGraph,
    pub anchors: BTreeMap<String, Point>,
}

pub(super) fn parse(text: &str) -> Result<SceneData> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("wsroads 1") {
        bail!("expected wsroads 1");
    }
    let mut nodes = BTreeMap::new();
    let mut edges = Vec::new();
    let mut anchors = BTreeMap::new();
    for line in lines {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<_> = line.split('\t').collect();
        let id = |i: usize| -> Result<u32> {
            f.get(i)
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| anyhow!("invalid road node id"))
        };
        match f[0] {
            "node" if f.len() == 5 => {
                let point: Point = [f[2].parse()?, f[3].parse()?, f[4].parse()?];
                if !point.into_iter().all(f32::is_finite) || nodes.insert(id(1)?, point).is_some() {
                    bail!("invalid or duplicate road node");
                }
            }
            "edge" if f.len() == 3 => edges.push([id(1)?, id(2)?]),
            "anchor" if f.len() == 3 => {
                if anchors.insert(f[1].to_string(), id(2)?).is_some() {
                    bail!("duplicate road anchor");
                }
            }
            _ => bail!("unknown road record: {}", f[0]),
        }
    }
    let order: BTreeMap<_, _> = nodes.keys().enumerate().map(|(i, id)| (*id, i)).collect();
    let index = |id: u32| {
        order
            .get(&id)
            .copied()
            .ok_or_else(|| anyhow!("missing road node {id}"))
    };
    let roads = RoadGraph {
        nodes: nodes.values().copied().collect(),
        edges: edges
            .into_iter()
            .map(|[a, b]| Ok([index(a)?, index(b)?]))
            .collect::<Result<_>>()?,
    };
    roads.validate().map_err(|e| anyhow!(e))?;
    let anchors: BTreeMap<_, _> = anchors
        .into_iter()
        .map(|(name, id)| {
            Ok((
                name,
                *nodes
                    .get(&id)
                    .ok_or_else(|| anyhow!("missing anchor node"))?,
            ))
        })
        .collect::<Result<_>>()?;
    for traveller in TRAVELLERS {
        for stop in traveller.stops {
            if !anchors.contains_key(stop) {
                bail!("missing road stop {stop}");
            }
        }
    }
    Ok(SceneData { roads, anchors })
}

pub(super) struct Traveller {
    pub name: &'static str,
    pub occupation: &'static str,
    pub source: u32,
    pub stops: [&'static str; 2],
    pub pause: f64,
    pub phase: f64,
}
pub(super) const TRAVELLERS: [Traveller; 6] = [
    Traveller {
        name: "Edwin Brook",
        occupation: "Woodworker",
        source: 66,
        stops: ["west", "crossroads"],
        pause: 18.0,
        phase: 0.0,
    },
    Traveller {
        name: "Martha Brook",
        occupation: "Gardener",
        source: 1632,
        stops: ["crossroads", "north"],
        pause: 24.0,
        phase: 49.0,
    },
    Traveller {
        name: "Harlan Reed",
        occupation: "Carpenter",
        source: 2046,
        stops: ["east", "inn"],
        pause: 22.0,
        phase: 91.0,
    },
    Traveller {
        name: "Alice Reed",
        occupation: "Weaver",
        source: 54,
        stops: ["inn", "west"],
        pause: 30.0,
        phase: 137.0,
    },
    Traveller {
        name: "Tobias Wood",
        occupation: "Traveller",
        source: 465,
        stops: ["crossroads", "stormwind"],
        pause: 45.0,
        phase: 210.0,
    },
    Traveller {
        name: "Helen Wood",
        occupation: "Traveller",
        source: 1632,
        stops: ["stormwind", "inn"],
        pause: 36.0,
        phase: 17.0,
    },
];
pub(super) fn journeys(scene: &SceneData) -> Vec<Journey> {
    TRAVELLERS
        .iter()
        .enumerate()
        .map(|(i, t)| Journey {
            guid: super::ADULT_GUID + i as u32,
            stops: t.stops.iter().map(|s| scene.anchors[*s]).collect(),
            pause_seconds: t.pause,
            phase: t.phase,
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_and_vegetation_records_are_not_accepted() {
        assert!(parse("wsroads 1\nhome\t1\n").is_err());
        assert!(parse("wsroads 1\nomit\t123\n").is_err());
        assert!(parse("wsroads 1\nnode\t1\tNaN\t0\t0\n").is_err());
    }
}
