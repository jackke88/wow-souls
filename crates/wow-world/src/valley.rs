//! Which ground is Northshire Valley: the client's own terrain data decides, never a coordinate
//! box (Elwynn Forest's rabbits and deer fit in any box around the valley).
//!
//! Every ADT chunk (MCNK) carries an `AreaTable.dbc` id; a position is in the valley when the chunk
//! under it names area 9 (Northshire Valley) or a sub-area that has 9 up its parent chain.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use benilla_formats::{
    area_id_at, load_area_table_catalog, load_tile_mesh, open_chain, world_to_tile,
    AreaTableCatalog, ChunkMesh, CHUNK_SIZE, TILE_SIZE,
};

impl crate::select::InValley for Valley {
    fn area_of(&self, x: f32, y: f32) -> u32 {
        Valley::area_of(self, x, y)
    }
    fn contains(&self, x: f32, y: f32) -> bool {
        Valley::contains(self, x, y)
    }
    fn name(&self, area: u32) -> String {
        Valley::name(self, area)
    }
}

/// `AreaTable.dbc` id of Northshire Valley (parent: Elwynn Forest, 12).
pub const NORTHSHIRE_VALLEY: u32 = 9;

/// Sub-areas that lie inside the valley but hang under Elwynn Forest (12) in `AreaTable.dbc`, found
/// by `wow-souls-world areas` on 2026-10-09: 34 "Echo Ridge Mine" holds the Echo Ridge kobolds
/// (40 spawns) and 59 "Northshire Vineyards" holds Milly's vines, 46 Defias Thugs and Garrick
/// Padfoot. The other Elwynn areas near the valley are not Northshire: 18 Crystal Lake (murlocs,
/// Mangy Wolves), 54 Jasperlode Mine (Kobold Miners, Geomancers), 87 Goldshire, 91 Tower of Azora.
pub const EXTRA_VALLEY_AREAS: [u32; 2] = [34, 59];

/// The ADT file prefix of map 0, the Eastern Kingdoms (`World\Maps\Azeroth\Azeroth_<x>_<y>.adt`).
const MAP: &str = "Azeroth";

/// Whether `area` is `root` or has `root` somewhere up its parent chain. `parent(a)` is the parent
/// area id (`Some(0)` for a top-level zone), `None` for an unknown area. Chains deeper than eight
/// are corrupt data and count as "no".
pub fn has_ancestor(area: u32, root: u32, parent: &dyn Fn(u32) -> Option<u32>) -> bool {
    let mut id = area;
    for _ in 0..8 {
        if id == root {
            return true;
        }
        match parent(id) {
            Some(0) | None => return false,
            Some(p) => id = p,
        }
    }
    false
}

/// The ADT tile `(column, row)` that holds a WoW position (the engine's own axis swap).
pub fn tile_of(x: f32, y: f32) -> (u32, u32) {
    world_to_tile(x, y)
}

/// The terrain areas around the valley plus `AreaTable`'s parents: answers "which area is under
/// (x, y)" and "is that the valley".
pub struct Valley {
    tiles: HashMap<(u32, u32), Vec<ChunkMesh>>,
    catalog: AreaTableCatalog,
}

impl Valley {
    /// Open the client's `Data` directory and load every ADT tile that the box `bounds`
    /// (`[x_min, y_min]`, `[x_max, y_max]`, WoW coordinates) touches.
    pub fn load(wow_data: &Path, bounds: ([f32; 2], [f32; 2])) -> Result<Valley> {
        let mut chain = open_chain(wow_data)
            .with_context(|| format!("opening WoW data {}", wow_data.display()))?;
        let catalog = load_area_table_catalog(&mut chain)?;
        let ([x0, y0], [x1, y1]) = bounds;
        let mut wanted: HashSet<(u32, u32)> = HashSet::new();
        let step = TILE_SIZE / 2.0;
        let mut x = x0;
        while x <= x1 + step {
            let mut y = y0;
            while y <= y1 + step {
                wanted.insert(tile_of(x.min(x1), y.min(y1)));
                y += step;
            }
            x += step;
        }
        let mut tiles = HashMap::new();
        for (tx, ty) in wanted {
            // A tile that does not exist (sea) or does not parse simply has no ground.
            if let Ok(mesh) = load_tile_mesh(&mut chain, MAP, tx, ty) {
                tiles.insert((tx, ty), mesh.chunks);
            }
        }
        Ok(Valley { tiles, catalog })
    }

    /// The `AreaTable` id under a position, 0 when no loaded tile covers it.
    pub fn area_of(&self, x: f32, y: f32) -> u32 {
        self.tiles
            .get(&tile_of(x, y))
            .and_then(|chunks| area_id_at(chunks, [x, y, 0.0]))
            .unwrap_or(0)
    }

    fn parent_of(&self, area: u32) -> Option<u32> {
        self.catalog.get(area).map(|r| r.zone_id)
    }

    /// Whether `area` is the valley or one of its sub-areas.
    pub fn is_valley_area(&self, area: u32) -> bool {
        area != 0
            && (EXTRA_VALLEY_AREAS.contains(&area)
                || has_ancestor(area, NORTHSHIRE_VALLEY, &|a| self.parent_of(a)))
    }

    /// True when the area under (x, y) is Northshire Valley or one of its sub-areas.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.is_valley_area(self.area_of(x, y))
    }

    /// The display name of an area ("Northshire Abbey"), or `area <id>` when the table lacks it.
    pub fn name(&self, area: u32) -> String {
        self.catalog
            .name(area)
            .map_or_else(|| format!("area {area}"), str::to_string)
    }

    /// The parent area's id (0 = top level) and name.
    pub fn parent(&self, area: u32) -> (u32, String) {
        let p = self.parent_of(area).unwrap_or(0);
        (p, if p == 0 { String::new() } else { self.name(p) })
    }

    /// Every loaded terrain chunk as `(centre [x, y], area id)`, for drawing the valley's outline.
    pub fn cells(&self) -> Vec<([f32; 2], u32)> {
        let half = CHUNK_SIZE / 2.0;
        let mut out = Vec::new();
        for chunks in self.tiles.values() {
            for c in chunks {
                if let Some(nw) = c.positions.first() {
                    // The first vertex is the chunk's north-west corner; x falls to the south, y to the east.
                    out.push(([nw[0] - half, nw[1] - half], c.area_id));
                }
            }
        }
        out
    }

    /// `(area id, name)` of every valley area found under `points`, sorted by id.
    pub fn areas_under(&self, points: &[[f32; 2]]) -> Vec<(u32, String)> {
        let found: BTreeMap<u32, String> = points
            .iter()
            .map(|p| self.area_of(p[0], p[1]))
            .filter(|&a| self.is_valley_area(a))
            .map(|a| (a, self.name(a)))
            .collect();
        found.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subzone_counts_when_the_root_is_in_its_parent_chain() {
        // area -> parent: 9 -> 12 (Elwynn), 86 -> 9, 87 -> 12, 12 -> 0
        let parents = |a: u32| match a {
            9 => Some(12),
            86 => Some(9),
            87 => Some(12),
            12 => Some(0),
            _ => None,
        };
        assert!(has_ancestor(9, 9, &parents), "the root itself");
        assert!(has_ancestor(86, 9, &parents), "a child of the root");
        assert!(!has_ancestor(87, 9, &parents), "a sibling under Elwynn");
        assert!(
            !has_ancestor(12, 9, &parents),
            "the parent zone is not inside its child"
        );
        assert!(!has_ancestor(999, 9, &parents), "an unknown area");
    }

    #[test]
    fn a_parent_cycle_is_not_followed_forever() {
        let cyc = |a: u32| match a {
            1 => Some(2),
            2 => Some(1),
            _ => None,
        };
        assert!(!has_ancestor(1, 9, &cyc));
    }

    #[test]
    fn the_abbey_anchor_is_on_tile_32_48() {
        // benilla's world_to_tile: tile_x = (32 * 533.33 - y) / 533.33, tile_y = (32 * 533.33 - x) / 533.33.
        // y = -136 -> 32.25 -> 32; x = -8913 -> 48.71 -> 48 (Azeroth_32_48.adt).
        assert_eq!(tile_of(-8913.0, -136.0), (32, 48));
    }

    fn install() -> Option<Valley> {
        let Ok(data) = std::env::var("WOW_DATA") else {
            eprintln!("skipped: WOW_DATA not set");
            return None;
        };
        Some(Valley::load(Path::new(&data), ([-9600.0, -600.0], [-8500.0, 200.0])).unwrap())
    }

    #[test]
    fn install_the_abbey_is_in_northshire_and_goldshire_is_not() {
        let Some(v) = install() else { return };
        // The Abbey's doors, Milly's vineyard, Garrick's spot, Milly Osworth herself.
        for (x, y) in [
            (-8913.0, -136.0),
            (-9070.0, -340.0),
            (-9056.48, -460.9),
            (-8850.29, -224.03),
        ] {
            let a = v.area_of(x, y);
            assert!(v.contains(x, y), "({x}, {y}) is area {a} {}", v.name(a));
        }
        // Goldshire (-9459, 64) is Elwynn proper.
        let g = v.area_of(-9459.0, 64.0);
        assert!(
            !v.contains(-9459.0, 64.0),
            "Goldshire is area {g} {}",
            v.name(g)
        );
        assert_ne!(v.area_of(-8913.0, -136.0), 0);
    }
}
