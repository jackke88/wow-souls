//! Northshire and Elwynn area selection from the client's terrain and WMO interiors. Legacy XY
//! queries still describe Northshire terrain; positional queries use the selected zone and the
//! nearest collision floor, so a mine and the grass above it can have different area identities.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use benilla_formats::{
    area_id_at, load_area_table_catalog, load_faction_catalog, load_tile_mesh, open_chain,
    terrain_height_at, world_to_tile, AreaTableCatalog, ChunkMesh, FactionCatalog, Reaction,
    CHUNK_SIZE, TILE_SIZE,
};

#[path = "area.rs"]
mod interior;

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
    fn area_at(&self, pos: [f32; 3]) -> u32 {
        Valley::area_at(self, pos)
    }
    fn contains_at(&self, pos: [f32; 3]) -> bool {
        Valley::contains_at(self, pos)
    }
    fn quest_in_zone(&self, zone_or_sort: i32) -> bool {
        Valley::quest_in_zone(self, zone_or_sort)
    }
    fn quest_level_ok(&self, level: i32, min_level: u32) -> bool {
        match self.zone {
            Zone::Northshire | Zone::Westfall => true,
            Zone::Elwynn => (0..=13).contains(&level) && min_level <= 13,
        }
    }
    fn named_spawns(&self) -> &[u32] {
        match self.zone {
            Zone::Northshire => &crate::select::NAMED_SPAWNS,
            Zone::Elwynn | Zone::Westfall => &[],
        }
    }
}

/// `AreaTable.dbc` id of Northshire Valley (parent: Elwynn Forest, 12).
pub const NORTHSHIRE_VALLEY: u32 = 9;

/// `AreaTable.dbc` id of Stormwind City (inside Elwynn's terrain bounds).
pub const STORMWIND_CITY: u32 = 1519;

/// Sub-areas that lie inside the valley but hang under Elwynn Forest (12) in `AreaTable.dbc`, found
/// by `wow-souls-world areas` on 2026-10-09: 34 "Echo Ridge Mine" holds the Echo Ridge kobolds
/// (40 spawns) and 59 "Northshire Vineyards" holds Milly's vines, 46 Defias Thugs and Garrick
/// Padfoot. The other Elwynn areas near the valley are not Northshire: 18 Crystal Lake (murlocs,
/// Mangy Wolves), 54 Jasperlode Mine (Kobold Miners, Geomancers), 87 Goldshire, 91 Tower of Azora.
pub const EXTRA_VALLEY_AREAS: [u32; 2] = [34, 59];

/// Import scope; both zones use map 0, and Elwynn includes Northshire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Northshire,
    Elwynn,
    Westfall,
}

impl Zone {
    pub const fn root(self) -> u32 {
        match self {
            Self::Northshire => NORTHSHIRE_VALLEY,
            Self::Elwynn => 12,
            Self::Westfall => 40,
        }
    }

    /// Terrain load bounds with a margin, not a coordinate-based spawn selection rule.
    pub const fn bounds(self) -> ([f32; 2], [f32; 2]) {
        match self {
            Self::Northshire => ([-9600.0, -600.0], [-8500.0, 200.0]),
            Self::Elwynn => ([-10400.0, -1900.0], [-7800.0, 1250.0]),
            Self::Westfall => ([-11600.0, -300.0], [-9400.0, 2600.0]),
        }
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Northshire => "northshire",
            Self::Elwynn => "elwynn",
            Self::Westfall => "westfall",
        }
    }
}

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
    interiors: interior::Interiors,
    factions: FactionCatalog,
    zone: Zone,
}

impl Valley {
    /// Open the client's `Data` directory and load every ADT tile that the box `bounds`
    /// (`[x_min, y_min]`, `[x_max, y_max]`, WoW coordinates) touches.
    pub fn load(wow_data: &Path, bounds: ([f32; 2], [f32; 2])) -> Result<Valley> {
        Self::load_zone(wow_data, bounds, Zone::Northshire)
    }

    /// Load the area resolver for a selected zone, including placed WMO collision and faction data.
    /// An unreadable WMO root/group is an import error: guessing terrain could select another zone.
    pub fn load_zone(wow_data: &Path, bounds: ([f32; 2], [f32; 2]), zone: Zone) -> Result<Valley> {
        let ([x0, y0], [x1, y1]) = bounds;
        if ![x0, y0, x1, y1].into_iter().all(f32::is_finite) || x0 > x1 || y0 > y1 {
            bail!("invalid zone terrain bounds {bounds:?}");
        }
        let mut chain = open_chain(wow_data)
            .with_context(|| format!("opening WoW data {}", wow_data.display()))?;
        let catalog = load_area_table_catalog(&mut chain)?;
        let factions = load_faction_catalog(&mut chain)?;
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
        let mut placements = Vec::new();
        for (tx, ty) in wanted {
            // A tile that does not exist (sea) or does not parse simply has no ground.
            if let Ok(mesh) = load_tile_mesh(&mut chain, MAP, tx, ty) {
                placements.extend(mesh.wmos);
                let chunks = mesh.chunks.into_iter().map(area_chunk).collect();
                tiles.insert((tx, ty), chunks);
            }
        }
        let interiors = interior::Interiors::load(&mut chain, placements)?;
        Ok(Valley {
            tiles,
            catalog,
            interiors,
            factions,
            zone,
        })
    }

    /// The `AreaTable` id under a position, 0 when no loaded tile covers it.
    pub fn area_of(&self, x: f32, y: f32) -> u32 {
        self.tiles
            .get(&tile_of(x, y))
            .and_then(|chunks| area_id_at(chunks, [x, y, 0.0]))
            .unwrap_or(0)
    }

    /// Finest area at a spawn's feet: WMO interior first, terrain otherwise. Missing tiles return 0.
    pub fn area_at(&self, pos: [f32; 3]) -> u32 {
        if !pos.into_iter().all(f32::is_finite) {
            return 0;
        }
        let Some(chunks) = self.tiles.get(&tile_of(pos[0], pos[1])) else {
            return 0;
        };
        self.interiors
            .area_at(pos, terrain_height_at(chunks, pos))
            .or_else(|| area_id_at(chunks, pos))
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

    /// Whether an area's parent chain belongs to the requested import scope.
    pub fn is_zone_area(&self, area: u32) -> bool {
        match self.zone {
            Zone::Northshire => self.is_valley_area(area),
            Zone::Westfall => {
                area != 0 && has_ancestor(area, self.zone.root(), &|a| self.parent_of(a))
            }
            Zone::Elwynn => {
                area != 0
                    && (has_ancestor(area, self.zone.root(), &|a| self.parent_of(a))
                        || has_ancestor(area, STORMWIND_CITY, &|a| self.parent_of(a)))
            }
        }
    }

    pub fn contains_at(&self, pos: [f32; 3]) -> bool {
        self.is_zone_area(self.area_at(pos))
    }

    /// Positive ZoneOrSort values name areas; negative values are class/profession quest sorts.
    pub fn quest_in_zone(&self, zone_or_sort: i32) -> bool {
        zone_or_sort > 0 && self.is_zone_area(zone_or_sort as u32)
    }

    /// Resolve selected creatures' base reaction toward a human from the install's FactionTemplate.
    /// Reputation changes remain a runtime concern; this import records the initial template law.
    pub fn annotate_world(&self, world: &mut crate::model::World) -> Result<()> {
        use crate::model::Disposition;

        let human = self
            .factions
            .race_template(1)
            .context("missing human faction template")?;
        let creatures: HashMap<_, _> = world.creatures.iter().map(|c| (c.entry, c)).collect();
        let metadata_entries: HashSet<_> = world.creature_meta.iter().map(|m| m.entry).collect();
        if creatures.len() != world.creatures.len()
            || creatures.len() != world.creature_meta.len()
            || metadata_entries.len() != world.creature_meta.len()
        {
            bail!("selected creatures require one metadata row each");
        }
        for meta in &mut world.creature_meta {
            let creature = creatures
                .get(&meta.entry)
                .with_context(|| format!("metadata has no creature template {}", meta.entry))?;
            let faction = self.factions.template(creature.faction).with_context(|| {
                format!(
                    "creature {} has unknown faction template {}",
                    creature.entry, creature.faction
                )
            })?;
            meta.reaction = Some(match faction.reaction_toward(human) {
                Reaction::Friendly => Disposition::Friendly,
                Reaction::Neutral => Disposition::Neutral,
                Reaction::Hostile => Disposition::Hostile,
            });
        }
        Ok(())
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

/// Keep area/height data, release rendering allocations as each tile loads. Full Elwynn needs
/// many terrain tiles, but its importer never samples colour textures, lighting or liquid meshes.
fn area_chunk(mut chunk: ChunkMesh) -> ChunkMesh {
    chunk.normals = Vec::new();
    chunk.uvs = Vec::new();
    chunk.base_texture = None;
    chunk.layer_textures = Vec::new();
    chunk.layer_effect_ids = Vec::new();
    chunk.alpha_map = None;
    chunk.shadow = None;
    chunk.liquids = Vec::new();
    chunk
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

    fn scope(zone: Zone) -> Valley {
        use benilla_formats::AreaTableRow;

        let catalog = AreaTableCatalog::from_rows(
            [
                (9, 12),
                (12, 0),
                (34, 12),
                (54, 12),
                (59, 12),
                (1519, 0),
                (40, 0),
                (108, 40),
                (111, 40),
                (10, 0),
            ]
            .into_iter()
            .map(|(id, zone_id)| {
                (
                    id,
                    AreaTableRow {
                        map_id: 0,
                        zone_id,
                        explore_flag: 0,
                        flags: 0,
                        faction_group_mask: 0,
                        exploration_level: 0,
                        name: format!("area {id}"),
                    },
                )
            })
            .collect(),
        );
        Valley {
            tiles: HashMap::new(),
            catalog,
            interiors: Default::default(),
            factions: FactionCatalog::from_rows(HashMap::new(), HashMap::new()),
            zone,
        }
    }

    #[test]
    fn elwynn_includes_northshire_while_legacy_valley_queries_keep_their_scope() {
        let elwynn = scope(Zone::Elwynn);
        for area in [9, 12, 34, 54, 59, 1519] {
            assert!(elwynn.is_zone_area(area));
            assert!(elwynn.quest_in_zone(area as i32));
        }
        for area in [0, 9999] {
            assert!(!elwynn.is_zone_area(area));
        }
        assert!(!elwynn.quest_in_zone(-161));
        assert!(!elwynn.is_valley_area(12));
        assert!(!elwynn.is_valley_area(54));
        assert!(!elwynn.is_valley_area(1519));
        let northshire = scope(Zone::Northshire);
        assert!(northshire.is_zone_area(9));
        assert!(northshire.is_zone_area(34));
        assert!(!northshire.is_zone_area(12));
        assert!(!northshire.is_zone_area(1519));
        assert!(!northshire.quest_in_zone(12));
        assert_eq!(elwynn.area_at([f32::NAN, 0.0, 0.0]), 0);
    }

    #[test]
    fn westfall_includes_its_mines_and_high_level_quests_only_in_its_zone() {
        use crate::select::InValley;
        let westfall = scope(Zone::Westfall);
        for area in [40, 108, 111] {
            assert!(westfall.is_zone_area(area));
        }
        for area in [0, 9, 12, 10, 1519] {
            assert!(!westfall.is_zone_area(area));
        }
        assert!(westfall.quest_level_ok(30, 20));
        assert!(!scope(Zone::Elwynn).quest_level_ok(30, 20));
        assert_eq!(Zone::Westfall.slug(), "westfall");
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

    #[test]
    fn install_elwynn_resolves_stormwind_and_mines_by_the_spawn_floor() {
        let Ok(data) = std::env::var("WOW_DATA") else {
            eprintln!("skipped: WOW_DATA not set");
            return;
        };
        let v = Valley::load_zone(Path::new(&data), Zone::Elwynn.bounds(), Zone::Elwynn).unwrap();
        let jasper = [-8807.56, 328.195, 102.336];
        let city = v.area_at(jasper);
        println!("Jasper Fel: area {city} {}", v.name(city));
        assert_eq!(v.area_of(jasper[0], jasper[1]), 12);
        assert!(has_ancestor(city, 1519, &|a| v.parent_of(a)));
        assert!(v.contains_at(jasper));

        for (label, pos, expected) in [
            ("Mine Spider", [-9066.4, -548.124, 58.3119], 54),
            ("Mother Fang", [-9051.83, -621.321, 53.5771], 54),
            ("Kobold Miner", [-9759.31, 148.983, 22.3136], 57),
        ] {
            let area = v.area_at(pos);
            println!("{label}: area {area} {}", v.name(area));
            assert_eq!(area, expected, "{label}: {}", v.name(area));
            assert!(v.contains_at(pos));
            let chunks = &v.tiles[&tile_of(pos[0], pos[1])];
            if let Some(z) = terrain_height_at(chunks, pos) {
                let above = [pos[0], pos[1], z + 0.1];
                assert_eq!(
                    v.area_at(above),
                    v.area_of(pos[0], pos[1]),
                    "grass above {label}"
                );
            }
        }
    }
}
