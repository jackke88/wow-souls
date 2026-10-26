//! Offline area claims from WMO collision faces. Keep the importer's position ray independent of
//! the renderer: portal crossings and the camera's DETAIL-face fallback do not name a spawn's area.

use std::collections::{BTreeMap, HashMap};

use anyhow::{bail, Context, Result};
use benilla_formats::{
    accumulate_wmo_group_collision, load_wmo_area_catalog, parse_wmo_root, triangle_z_at,
    wmo_group_header, wmo_root_id, Chain, WmoInstance,
};
use glam::{Quat, Vec3};

type Triangle = [[f32; 3]; 3];
type Bounds = ([f32; 3], [f32; 3]);

const EXTERIOR: u32 = 0x8;
const PROBE_LIFT: f32 = 0.1;
const RAY_LENGTH: f32 = 1000.0;

struct ModelGroup {
    flags: u32,
    /// MOGP uniqueID, the WMOAreaTable.WMOGroupID join key, not an AreaTable id.
    key: u32,
    triangles: Vec<Triangle>,
}

struct Model {
    id: u32,
    groups: Vec<ModelGroup>,
}

struct Group {
    flags: u32,
    area: u32,
    triangles: Vec<Triangle>,
    bounds: Option<Bounds>,
}

struct Placement {
    groups: Vec<Group>,
    bounds: Option<Bounds>,
}

#[derive(Default)]
pub(super) struct Interiors {
    placements: Vec<Placement>,
}

impl Interiors {
    pub(super) fn load(chain: &mut Chain, placements: Vec<WmoInstance>) -> Result<Self> {
        let areas = load_wmo_area_catalog(chain)?;
        // MODF lists a building in every tile it intersects. Stable uid order also fixes ray ties.
        let placements: BTreeMap<_, _> = placements.into_iter().map(|p| (p.unique_id, p)).collect();
        let mut models = HashMap::new();
        let mut placed = Vec::with_capacity(placements.len());
        for p in placements.into_values() {
            let path = p.model.to_ascii_lowercase();
            let model = match models.entry(path) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::hash_map::Entry::Vacant(e) => {
                    let model = load_model(chain, e.key())?;
                    e.insert(model)
                }
            };
            let rotation = placement_rotation(p.rotation);
            let origin = Vec3::from_array(p.position);
            let groups: Vec<Group> = model
                .groups
                .iter()
                .map(|g| {
                    // World-space faces keep the ray vertical for pitched and rolled placements too.
                    let triangles: Vec<Triangle> = g
                        .triangles
                        .iter()
                        .map(|t| t.map(|v| (origin + rotation * Vec3::from_array(v)).to_array()))
                        .collect();
                    Group {
                        flags: g.flags,
                        area: areas
                            .resolve(model.id, u32::from(p.name_set), g.key)
                            .map_or(0, |row| row.area_table_id),
                        bounds: triangle_bounds(&triangles),
                        triangles,
                    }
                })
                .collect();
            let bounds = union_bounds(groups.iter().filter_map(|g| g.bounds));
            placed.push(Placement { groups, bounds });
        }
        // Local geometry is only a load-time cache; retain the world-space collision once loaded.
        Ok(Self { placements: placed })
    }

    /// Nonzero WMO area when its face owns this column; terrain is the caller's fallback.
    pub(super) fn area_at(&self, pos: [f32; 3], terrain_z: Option<f32>) -> Option<u32> {
        let eye = [pos[0], pos[1], pos[2] + PROBE_LIFT];
        let mut best: Option<(f32, &Group)> = None;
        for p in &self.placements {
            if !column_in_bounds(p.bounds, eye) {
                continue;
            }
            for group in &p.groups {
                if !column_in_bounds(group.bounds, eye) {
                    continue;
                }
                if let Some(z) = floor_at(&group.triangles, eye) {
                    // Exterior faces compete too: never let a more distant interior behind a deck win.
                    if best.as_ref().is_none_or(|(best_z, _)| z > *best_z) {
                        best = Some((z, group));
                    }
                }
            }
        }
        let (z, group) = best?;
        if !wmo_wins(eye[2], z, terrain_z) || group.flags & EXTERIOR != 0 {
            return None;
        }
        (group.area != 0).then_some(group.area)
    }
}

fn load_model(chain: &mut Chain, path: &str) -> Result<Model> {
    let bytes = chain
        .read_file(path)
        .with_context(|| format!("reading area WMO {path}"))?;
    let root = parse_wmo_root(&bytes).with_context(|| format!("parsing area WMO {path}"))?;
    let stem = path.strip_suffix(".wmo").unwrap_or(path);
    let mut groups = Vec::with_capacity(root.group_count() as usize);
    for i in 0..root.group_count() {
        let group_path = format!("{stem}_{i:03}.wmo");
        let bytes = chain
            .read_file(&group_path)
            .with_context(|| format!("reading area WMO group {group_path}"))?;
        let header = wmo_group_header(&bytes)
            .with_context(|| format!("missing area WMO group header in {group_path}"))?;
        let expected_indices = collision_index_count(&bytes)
            .with_context(|| format!("validating area WMO collision in {group_path}"))?;
        let (mut positions, mut indices) = (Vec::new(), Vec::new());
        accumulate_wmo_group_collision(&bytes, &mut positions, &mut indices);
        if indices.len() != expected_indices {
            bail!("area WMO collision failed to decode in {group_path}");
        }
        let mut triangles = Vec::with_capacity(indices.len() / 3);
        for face in indices.as_chunks::<3>().0 {
            let vertex = |i: u32| {
                positions.get(i as usize).copied().with_context(|| {
                    format!("invalid area WMO collision index {i} in {group_path}")
                })
            };
            triangles.push([vertex(face[0])?, vertex(face[1])?, vertex(face[2])?]);
        }
        groups.push(ModelGroup {
            flags: header.flags,
            key: header.area_table_id,
            triangles,
        });
    }
    Ok(Model {
        id: wmo_root_id(&bytes),
        groups,
    })
}

/// The public collision adapter is infallible and discards parse errors. Validate its source
/// geometry and expected face count so a damaged group cannot quietly become an outdoor miss.
/// Chunk tails clamp to EOF, matching benilla's decoder (one shipped MOGP declares one extra byte).
fn collision_index_count(bytes: &[u8]) -> Result<usize> {
    let (_, mogp) = raw_chunks(bytes)
        .find(|(magic, _)| *magic == b"PGOM")
        .context("missing MOGP")?;
    let payload = mogp.get(68..).context("short MOGP header")?;
    let (mut vertices, mut materials, mut expected) = (0usize, 0usize, 0usize);
    let mut indices = Vec::new();
    for (magic, data) in raw_chunks(payload) {
        match magic {
            b"TVOM" => {
                if !data.len().is_multiple_of(12) {
                    bail!("truncated MOVT vertices");
                }
                for value in data.as_chunks::<4>().0 {
                    if !f32::from_le_bytes(*value).is_finite() {
                        bail!("non-finite MOVT coordinate");
                    }
                }
                vertices += data.len() / 12;
            }
            b"IVOM" => {
                if !data.len().is_multiple_of(2) {
                    bail!("truncated MOVI indices");
                }
                indices.extend(
                    data.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|v| u16::from_le_bytes(*v)),
                );
            }
            b"YPOM" => {
                if !data.len().is_multiple_of(2) {
                    bail!("truncated MOPY materials");
                }
                materials += data.len() / 2;
                expected += data
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .filter(|m| m[0] & 0x4 == 0)
                    .count()
                    * 3;
            }
            _ => {}
        }
    }
    if !indices.len().is_multiple_of(3) || materials != indices.len() / 3 {
        bail!("MOVI triangle count and MOPY materials disagree");
    }
    if indices.iter().any(|&i| usize::from(i) >= vertices) {
        bail!("MOVI index outside MOVT vertices");
    }
    Ok(expected)
}

fn raw_chunks(bytes: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    let mut offset = 0usize;
    std::iter::from_fn(move || {
        let header = bytes.get(offset..offset + 8)?;
        let size = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
        let start = offset + 8;
        let end = start.saturating_add(size).min(bytes.len());
        offset = end;
        Some((&header[..4], &bytes[start..end]))
    })
}

/// Raw-WoW half of benilla-assets::coords::placement_rotation; no renderer basis conversion.
fn placement_rotation(degrees: [f32; 3]) -> Quat {
    use std::f32::consts::{FRAC_PI_2, PI};
    let [rx, ry, rz] = degrees.map(f32::to_radians);
    Quat::from_rotation_x(FRAC_PI_2)
        * Quat::from_rotation_y(ry - PI)
        * Quat::from_rotation_z(-rx)
        * Quat::from_rotation_x(rz - FRAC_PI_2)
}

fn triangle_bounds(triangles: &[Triangle]) -> Option<Bounds> {
    let mut vertices = triangles.iter().flatten();
    let first = *vertices.next()?;
    let (mut min, mut max) = (first, first);
    for vertex in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex[axis]);
            max[axis] = max[axis].max(vertex[axis]);
        }
    }
    Some((min, max))
}

fn union_bounds(mut bounds: impl Iterator<Item = Bounds>) -> Option<Bounds> {
    let (mut min, mut max) = bounds.next()?;
    for (low, high) in bounds {
        for axis in 0..3 {
            min[axis] = min[axis].min(low[axis]);
            max[axis] = max[axis].max(high[axis]);
        }
    }
    Some((min, max))
}

fn column_in_bounds(bounds: Option<Bounds>, eye: [f32; 3]) -> bool {
    bounds.is_some_and(|(min, max)| {
        eye[0] >= min[0]
            && eye[0] <= max[0]
            && eye[1] >= min[1]
            && eye[1] <= max[1]
            && min[2] <= eye[2]
    })
}

fn floor_at(triangles: &[Triangle], eye: [f32; 3]) -> Option<f32> {
    triangles
        .iter()
        .filter_map(|t| triangle_z_at(t, eye[0], eye[1]))
        .filter(|&z| z <= eye[2])
        .max_by(f32::total_cmp)
}

fn wmo_wins(eye_z: f32, floor_z: f32, terrain_z: Option<f32>) -> bool {
    eye_z - floor_z <= RAY_LENGTH
        && !terrain_z.is_some_and(|terrain| terrain <= eye_z && terrain > floor_z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor(z: f32) -> Vec<Triangle> {
        vec![
            [[-2.0, -2.0, z], [2.0, -2.0, z], [2.0, 2.0, z]],
            [[-2.0, -2.0, z], [2.0, 2.0, z], [-2.0, 2.0, z]],
        ]
    }

    fn interiors(spec: &[(f32, u32, u32)]) -> Interiors {
        let groups: Vec<_> = spec
            .iter()
            .map(|&(z, flags, area)| {
                let triangles = floor(z);
                Group {
                    bounds: triangle_bounds(&triangles),
                    triangles,
                    flags,
                    area,
                }
            })
            .collect();
        let bounds = union_bounds(groups.iter().filter_map(|g| g.bounds));
        Interiors {
            placements: vec![Placement { groups, bounds }],
        }
    }

    #[test]
    fn terrain_above_a_tunnel_is_not_on_the_ray_but_grass_below_the_feet_is() {
        let mine = interiors(&[(0.0, 0, 54)]);
        assert_eq!(mine.area_at([0.0, 0.0, 0.0], Some(20.0)), Some(54));
        assert_eq!(mine.area_at([0.0, 0.0, 20.0], Some(20.0)), None);
        assert_eq!(mine.area_at([0.0, 0.0, 0.0], Some(0.0)), Some(54));
    }

    #[test]
    fn nearest_floor_wins_and_exterior_decks_block_the_room_below() {
        let rooms = interiors(&[(0.0, 0, 54), (5.0, 0, 57)]);
        assert_eq!(rooms.area_at([0.0, 0.0, 6.0], None), Some(57));
        assert_eq!(rooms.area_at([0.0, 0.0, 1.0], None), Some(54));
        let deck = interiors(&[(0.0, 0, 54), (5.0, EXTERIOR, 57)]);
        assert_eq!(deck.area_at([0.0, 0.0, 6.0], None), None);
    }

    #[test]
    fn exterior_lit_stormwind_streets_still_name_the_city() {
        let streets = interiors(&[(0.0, 0x40, 1519)]);
        assert_eq!(streets.area_at([0.0, 0.0, 0.0], None), Some(1519));
    }

    #[test]
    fn probe_has_a_finite_reach_and_no_upward_retry() {
        let room = interiors(&[(0.0, 0, 54)]);
        assert_eq!(room.area_at([0.0, 0.0, -1.0], None), None);
        assert_eq!(room.area_at([0.0, 0.0, 1001.0], None), None);
        assert_eq!(room.area_at([3.0, 0.0, 0.0], None), None);
    }

    #[test]
    fn placement_rotation_maps_heading_and_keeps_tilted_faces_in_world_space() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert!((placement_rotation([0.0, 180.0, 0.0]) * v - v).length() < 1.0e-5);
        let yaw = placement_rotation([0.0, 270.0, 0.0]) * Vec3::X;
        assert!((yaw - Vec3::Y).length() < 1.0e-5);
        let rotation = placement_rotation([15.0, 195.0, 10.0]);
        let triangles: Vec<_> = floor(0.0)
            .iter()
            .map(|t| t.map(|v| (rotation * Vec3::from_array(v)).to_array()))
            .collect();
        let p = rotation * Vec3::new(0.3, 0.4, 0.0);
        let z = floor_at(&triangles, [p.x, p.y, p.z + 0.1]).unwrap();
        assert!((z - p.z).abs() < 1.0e-5);
    }

    #[test]
    fn damaged_group_geometry_is_an_error_even_when_the_adapter_would_return_empty() {
        let chunk = |magic: &[u8; 4], payload: &[u8]| {
            [
                magic.as_slice(),
                &(payload.len() as u32).to_le_bytes(),
                payload,
            ]
            .concat()
        };
        let group = |indices: &[u8], material: &[u8]| {
            let payload = [
                vec![0; 68],
                chunk(b"TVOM", &[0; 36]),
                chunk(b"IVOM", indices),
                chunk(b"YPOM", material),
            ]
            .concat();
            chunk(b"PGOM", &payload)
        };
        let triangle = [0, 0, 1, 0, 2, 0];
        assert_eq!(
            collision_index_count(&group(&triangle, &[0, 0])).unwrap(),
            3
        );
        assert_eq!(
            collision_index_count(&group(&triangle, &[4, 0])).unwrap(),
            0
        );
        assert!(collision_index_count(&group(&triangle, &[])).is_err());
        assert!(collision_index_count(&group(&[0, 0, 1, 0, 3, 0], &[0, 0])).is_err());
        assert!(collision_index_count(&group(&[0, 0, 1], &[0, 0])).is_err());
    }
}
