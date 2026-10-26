//! Read-only Goldshire terrain/placement survey. Source archives are never modified.
//! Run with WOW_DATA and an optional output prefix (default docs/research/2026-10-17-goldshire-survey).
use anyhow::{Context, Result};
use benilla_formats::{
    accumulate_wmo_group_collision, load_m2_bounds, load_m2_mesh, load_tile_mesh, load_wmo,
    parse_wmo_root, terrain_height_at, Chain, ChunkMesh, MapTiles, TileMesh, WmoInstance,
};
use glam::{Quat, Vec2, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::PathBuf;

const CENTER: Vec2 = Vec2::new(-9460.0, 50.0);
const RADIUS: f32 = 420.0;

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: Vec3,
    max: Vec3,
}
impl Bounds {
    fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }
    fn add(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    fn transformed(self, p: Vec3, q: Quat, scale: f32) -> Self {
        let mut b = Self::empty();
        for x in [self.min.x, self.max.x] {
            for y in [self.min.y, self.max.y] {
                for z in [self.min.z, self.max.z] {
                    b.add(p + q * Vec3::new(x, y, z) * scale);
                }
            }
        }
        b
    }
    fn xy_distance(self, other: Self) -> f32 {
        let delta = (self.min.truncate() - other.max.truncate())
            .max(other.min.truncate() - self.max.truncate())
            .max(Vec2::ZERO);
        delta.length()
    }
}

/// Same raw-WoW quaternion as benilla_assets::coords::placement_rotation, before conjugation.
fn rotation(r: [f32; 3]) -> Quat {
    use std::f32::consts::{FRAC_PI_2, PI};
    Quat::from_rotation_x(FRAC_PI_2)
        * Quat::from_rotation_y(r[1].to_radians() - PI)
        * Quat::from_rotation_z(-r[0].to_radians())
        * Quat::from_rotation_x(r[2].to_radians() - FRAC_PI_2)
}

struct Obstacle {
    kind: &'static str,
    id: u32,
    model: String,
    position: Vec3,
    rotation: [f32; 3],
    scale: f32,
    bounds: Bounds,
    triangles: Vec<[Vec3; 3]>,
}
struct WmoModel {
    bounds: Bounds,
    triangles: Vec<[Vec3; 3]>,
    portals: Vec<Vec3>,
    floor_z: f32,
    profile: Vec<Bounds>,
}
fn wmo_model(chain: &mut Chain, path: &str) -> Result<WmoModel> {
    let bytes = chain.read_file(path)?;
    let root = parse_wmo_root(&bytes)?;
    let mut bounds = Bounds::empty();
    for g in root.group_infos() {
        bounds.add(Vec3::from(g.bbox_min));
        bounds.add(Vec3::from(g.bbox_max));
    }
    let mut triangles = Vec::new();
    let mut levels = BTreeMap::<i32, f32>::new();
    for i in 0..root.group_count() {
        let stem = path.trim_end_matches(".wmo").trim_end_matches(".WMO");
        let bytes = chain.read_file(&format!("{stem}_{i:03}.wmo"))?;
        let (mut pos, mut idx) = (Vec::new(), Vec::new());
        accumulate_wmo_group_collision(&bytes, &mut pos, &mut idx);
        for t in idx.as_chunks::<3>().0.iter() {
            let tri = [
                Vec3::from(pos[t[0] as usize]),
                Vec3::from(pos[t[1] as usize]),
                Vec3::from(pos[t[2] as usize]),
            ];
            let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
            if n.z > 0.1 && n.normalize().z > 0.9 {
                let z = (tri[0].z + tri[1].z + tri[2].z) / 3.0;
                *levels.entry((z * 4.0).round() as i32).or_default() += n.length() * 0.5;
            }
            triangles.push(tri);
        }
    }
    let floor_z = levels
        .iter()
        .filter(|(z, _)| **z < 32)
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0.0, |(z, _)| *z as f32 / 4.0);
    let portals = root
        .portals()
        .infos
        .iter()
        .filter_map(|i| {
            let points = root
                .portals()
                .vertices
                .get(i.start_vertex as usize..i.start_vertex as usize + i.count as usize)?;
            (!points.is_empty())
                .then(|| points.iter().copied().map(Vec3::from).sum::<Vec3>() / points.len() as f32)
        })
        .collect();
    let render_tris = if path.contains("human_farm") {
        load_wmo(chain, path)?
            .into_iter()
            .flat_map(|s| {
                s.indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|t| {
                        [
                            s.positions[t[0] as usize].into(),
                            s.positions[t[1] as usize].into(),
                            s.positions[t[2] as usize].into(),
                        ]
                    })
                    .collect::<Vec<[Vec3; 3]>>()
            })
            .collect::<Vec<_>>()
    } else {
        triangles.clone()
    };
    let mut profile = Vec::new();
    for i in 0..=55 {
        let low = i as f32 * 0.5;
        let high = low + 0.5;
        let mut b = Bounds::empty();
        for t in &render_tris {
            let mut poly = t.to_vec();
            for (plane, sign) in [(low, 1.0), (high, -1.0)] {
                let mut out = Vec::new();
                for j in 0..poly.len() {
                    let a = poly[j];
                    let v = poly[(j + 1) % poly.len()];
                    let inside = (a.z - plane) * sign >= 0.0;
                    if inside {
                        out.push(a);
                    }
                    if inside != ((v.z - plane) * sign >= 0.0) {
                        out.push(a.lerp(v, (plane - a.z) / (v.z - a.z)));
                    }
                }
                poly = out;
            }
            for p in poly {
                b.add(p);
            }
        }
        if b.min.x.is_finite() {
            b.min.z = low;
            b.max.z = high;
            profile.push(b);
        }
    }
    Ok(WmoModel {
        bounds,
        triangles,
        portals,
        floor_z,
        profile,
    })
}

fn ground(tiles: &[TileMesh], p: Vec2) -> Option<f32> {
    tiles
        .iter()
        .find_map(|t| terrain_height_at(&t.chunks, [p.x, p.y, 0.0]))
}

/// Exact extrema of piecewise-linear ADT triangles clipped to the rotated house footprint.
/// A terrain peak can lie between grid samples; all clipped triangle vertices are checked too.
fn terrain_range(
    tiles: &[TileMesh],
    p: Vec2,
    q: Quat,
    bounds: Bounds,
) -> Option<(f32, f32, usize)> {
    let broad = bounds.transformed(p.extend(0.0), q, 1.0);
    let inv = q.inverse();
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut area = 0.0;
    let mut count = 0;
    for tile in tiles {
        for chunk in &tile.chunks {
            let nw = *chunk.positions.first()?;
            let cb = Bounds {
                min: Vec3::new(
                    nw[0] - benilla_formats::CHUNK_SIZE,
                    nw[1] - benilla_formats::CHUNK_SIZE,
                    -1000.0,
                ),
                max: Vec3::new(nw[0], nw[1], 1000.0),
            };
            if cb.xy_distance(broad) > 0.0 {
                continue;
            }
            for t in chunk.indices.as_chunks::<3>().0.iter() {
                let mut poly = t
                    .iter()
                    .map(|i| inv * (Vec3::from(chunk.positions[*i as usize]) - p.extend(0.0)))
                    .collect::<Vec<_>>();
                for (axis, plane, sign) in [
                    (0, bounds.min.x, 1.0),
                    (0, bounds.max.x, -1.0),
                    (1, bounds.min.y, 1.0),
                    (1, bounds.max.y, -1.0),
                ] {
                    let mut clipped = Vec::new();
                    for j in 0..poly.len() {
                        let a = poly[j];
                        let b = poly[(j + 1) % poly.len()];
                        let inside = (a[axis] - plane) * sign >= 0.0;
                        if inside {
                            clipped.push(a);
                        }
                        if inside != ((b[axis] - plane) * sign >= 0.0) {
                            clipped.push(a.lerp(b, (plane - a[axis]) / (b[axis] - a[axis])));
                        }
                    }
                    poly = clipped;
                }
                if poly.len() < 3 {
                    continue;
                }
                for v in &poly {
                    min = min.min(v.z);
                    max = max.max(v.z);
                }
                for j in 1..poly.len() - 1 {
                    area += (poly[j] - poly[0])
                        .truncate()
                        .perp_dot((poly[j + 1] - poly[0]).truncate())
                        .abs()
                        * 0.5;
                }
                count += 1;
            }
        }
    }
    let required = (bounds.max.x - bounds.min.x) * (bounds.max.y - bounds.min.y);
    (area >= required * 0.999 && min.is_finite() && max.is_finite()).then_some((min, max, count))
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}
fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--inspect-point") {
        let a = std::env::args()
            .skip(2)
            .map(|v| v.parse::<f32>())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        anyhow::ensure!(a.len() == 3, "--inspect-point x y z");
        return inspect_point(Vec3::new(a[0], a[1], a[2]));
    }
    let prefix = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "docs/research/2026-10-17-goldshire-survey".into()),
    );
    std::fs::create_dir_all(prefix.parent().context("output parent")?)?;
    let data = benilla_formats::wow_data().context("set WOW_DATA to vanilla Data directory")?;
    let mut chain = benilla_formats::open_chain(&data)?;
    let catalog = MapTiles::load(&mut chain, "Azeroth")?;
    let mut tiles = Vec::new();
    for (x, y) in catalog.existing_in_radius(CENTER.x, CENTER.y, 1) {
        tiles.push(load_tile_mesh(&mut chain, "Azeroth", x, y)?);
    }
    let mut models = BTreeMap::<String, WmoModel>::new();
    let mut wmos = BTreeMap::<u32, WmoInstance>::new();
    let mut obstacles = Vec::new();
    let mut seen = BTreeSet::new();
    let mut m2_cache = BTreeMap::new();
    let mut m2_tris = BTreeMap::<String, Vec<[Vec3; 3]>>::new();
    let mut failures = Vec::new();
    for t in &tiles {
        for w in &t.wmos {
            if wmos.contains_key(&w.unique_id) {
                continue;
            }
            if !models.contains_key(&w.model) {
                models.insert(w.model.clone(), wmo_model(&mut chain, &w.model)?);
            }
            let b = models[&w.model].bounds.transformed(
                Vec3::from(w.position),
                rotation(w.rotation),
                1.0,
            );
            if b.xy_distance(Bounds {
                min: (CENTER - Vec2::splat(RADIUS + 50.0)).extend(-1000.0),
                max: (CENTER + Vec2::splat(RADIUS + 50.0)).extend(1000.0),
            }) > 0.0
            {
                continue;
            }
            let wp = Vec3::from(w.position);
            let wq = rotation(w.rotation);
            let triangles = models[&w.model]
                .triangles
                .iter()
                .map(|t| t.map(|v| wp + wq * v))
                .collect();
            obstacles.push(Obstacle {
                kind: "wmo",
                id: w.unique_id,
                model: w.model.clone(),
                position: w.position.into(),
                rotation: w.rotation,
                scale: 1.0,
                bounds: b,
                triangles,
            });
            wmos.insert(w.unique_id, w.clone());
        }
        for d in &t.doodads {
            if (Vec3::from(d.position).truncate() - CENTER).length() > RADIUS + 100.0
                || !seen.insert(d.unique_id)
            {
                continue;
            }
            let bound = if let Some(b) = m2_cache.get(&d.model) {
                *b
            } else {
                match load_m2_bounds(&mut chain, &d.model) {
                    Ok(b) => {
                        let b = Bounds {
                            min: b.bbox_min.into(),
                            max: b.bbox_max.into(),
                        };
                        m2_cache.insert(d.model.clone(), b);
                        b
                    }
                    Err(e) => {
                        failures.push(format!("{}\t{}\t{e}", d.unique_id, d.model));
                        continue;
                    }
                }
            };
            if !m2_tris.contains_key(&d.model) && !d.model.contains("\\critter\\") {
                let mut triangles = Vec::new();
                for s in load_m2_mesh(&mut chain, &d.model)? {
                    for t in s.indices.as_chunks::<3>().0.iter() {
                        triangles.push([
                            Vec3::from(s.positions[t[0] as usize]),
                            Vec3::from(s.positions[t[1] as usize]),
                            Vec3::from(s.positions[t[2] as usize]),
                        ]);
                    }
                }
                m2_tris.insert(d.model.clone(), triangles);
            }
            let p = Vec3::from(d.position);
            let q = rotation(d.rotation);
            let triangles = m2_tris
                .get(&d.model)
                .map(|ts| ts.iter().map(|t| t.map(|v| p + q * v * d.scale)).collect())
                .unwrap_or_default();
            obstacles.push(Obstacle {
                kind: "m2",
                id: d.unique_id,
                model: d.model.clone(),
                position: d.position.into(),
                rotation: d.rotation,
                scale: d.scale,
                bounds: bound.transformed(d.position.into(), rotation(d.rotation), d.scale),
                triangles,
            });
        }
    }
    let mut out = String::from(
        "kind\tid\tmodel\tx\ty\tz\trx\try\trz\tscale\tmin_x\tmin_y\tmin_z\tmax_x\tmax_y\tmax_z\n",
    );
    for o in &obstacles {
        writeln!(out,"{}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}",o.kind,o.id,o.model,o.position.x,o.position.y,o.position.z,o.rotation[0],o.rotation[1],o.rotation[2],o.scale,o.bounds.min.x,o.bounds.min.y,o.bounds.min.z,o.bounds.max.x,o.bounds.max.y,o.bounds.max.z)?;
    }
    std::fs::write(prefix.with_extension("placements.tsv"), out)?;
    std::fs::write(prefix.with_extension("missing.tsv"), failures.join("\n"))?;
    let mut model_report =
        String::from("model\tmin\tmax\tfloor_z_estimate\tcollision_triangles\tportal_centers\n");
    for (path, m) in &models {
        writeln!(
            model_report,
            "{}\t{:?}\t{:?}\t{:.2}\t{}\t{:?}",
            path,
            m.bounds.min,
            m.bounds.max,
            m.floor_z,
            m.triangles.len(),
            m.portals
        )?;
    }
    let listing = chain.list()?;
    for e in listing {
        let n = e.name.to_ascii_lowercase();
        if n.ends_with(".wmo")
            && !n
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or("")
                .trim_end_matches(".wmo")
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_digit())
            && (n.contains("human") || n.contains("farmhouse") || n.contains("cottage"))
        {
            writeln!(model_report, "asset\t{}", e.name)?;
        }
    }
    std::fs::write(prefix.with_extension("models.tsv"), model_report)?;
    let mut road_report = String::from("x\ty\tz\ttexture\tweight\n");
    let mut svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 1000\"><rect width=\"1000\" height=\"1000\" fill=\"#dbe2b6\"/><g transform=\"translate(500 500) scale({})\">",1000.0/(RADIUS*2.0));
    let mut textures = BTreeSet::new();
    let mut roads = Vec::new();
    for t in &tiles {
        for c in &t.chunks {
            textures.extend(c.layer_textures.iter().cloned());
            draw_chunk(c, &mut svg, &mut road_report, &mut roads)?;
        }
    }
    for o in &obstacles {
        let b = o.bounds;
        if b.min.x > CENTER.x + RADIUS
            || b.max.x < CENTER.x - RADIUS
            || b.min.y > CENTER.y + RADIUS
            || b.max.y < CENTER.y - RADIUS
        {
            continue;
        }
        let color = if o.kind == "wmo" {
            "#774835"
        } else if o.model.to_ascii_lowercase().contains("tree") {
            "#3b722f"
        } else {
            "#6a8055"
        };
        writeln!(svg,"<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\" fill-opacity=\"0.07\" stroke=\"{color}\" stroke-width=\"0.3\"><title>{} #{}</title></rect>",CENTER.y-b.max.y,CENTER.x-b.max.x,b.max.y-b.min.y,b.max.x-b.min.x,xml(&o.model),o.id)?;
    }
    for o in obstacles
        .iter()
        .filter(|o| o.model == "world\\wmo\\azeroth\\buildings\\human_farm\\farm.wmo")
    {
        let door = o.position + rotation(o.rotation) * Vec3::new(-1.5399, -6.0261, 4.1093);
        println!(
            "native farm {}: originZ {:.3}, terrain origin {:?}, doorZ {:.3}, terrain door {:?}",
            o.id,
            o.position.z,
            ground(&tiles, o.position.truncate()),
            door.z,
            ground(&tiles, door.truncate())
        );
    }
    let (roads, edges) = road_graph(&roads, &tiles, &obstacles);
    candidates(
        &prefix, &tiles, &obstacles, &models, &roads, &edges, &mut svg,
    )?;
    svg.push_str("</g></svg>");
    std::fs::write(prefix.with_extension("svg"), svg)?;
    std::fs::write(prefix.with_extension("roads.tsv"), road_report)?;
    std::fs::write(
        prefix.with_extension("textures.txt"),
        textures.into_iter().collect::<Vec<_>>().join("\n"),
    )?;
    println!("{} tiles, {} WMO instances / {} types, {} M2 obstacles / {} types, {} unreadable M2; center terrain {:?}; outputs {}.*",tiles.len(),wmos.len(),models.len(),seen.len(),m2_cache.len(),failures.len(),ground(&tiles,CENTER),prefix.display());
    Ok(())
}

fn inspect_point(p: Vec3) -> Result<()> {
    let data = benilla_formats::wow_data().context("WOW_DATA")?;
    let mut chain = benilla_formats::open_chain(&data)?;
    let catalog = MapTiles::load(&mut chain, "Azeroth")?;
    let mut tiles = Vec::new();
    for (x, y) in catalog.existing_in_radius(p.x, p.y, 1) {
        tiles.push(load_tile_mesh(&mut chain, "Azeroth", x, y)?);
    }
    let body = Bounds {
        min: p + Vec3::new(-0.4, -0.4, 0.10),
        max: p + Vec3::new(0.4, 0.4, 2.1),
    };
    let foot = Bounds {
        min: p + Vec3::new(-0.4, -0.4, -0.05),
        max: p + Vec3::new(0.4, 0.4, 0.15),
    };
    let mut seen = BTreeSet::new();
    let mut cache = BTreeMap::new();
    for tile in &tiles {
        for d in &tile.doodads {
            if !seen.insert(d.unique_id)
                || (Vec3::from(d.position).truncate() - p.truncate()).length() > 120.0
            {
                continue;
            }
            if !cache.contains_key(&d.model) {
                cache.insert(
                    d.model.clone(),
                    benilla_formats::load_m2_collision_hull(&mut chain, &d.model)?,
                );
            }
            let hull = &cache[&d.model];
            let q = rotation(d.rotation);
            let pos = Vec3::from(d.position);
            let vertices: Vec<_> = hull
                .positions
                .iter()
                .map(|v| pos + q * Vec3::from(*v) * d.scale)
                .collect();
            let mut bounds = Bounds::empty();
            for v in &vertices {
                bounds.add(*v);
            }
            if bounds.xy_distance(body) > 2.0 && d.unique_id != 65538 && d.unique_id != 12580 {
                continue;
            }
            let body_hits = hull
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .filter(|t| tri_box(t.map(|i| vertices[i as usize]), body))
                .count();
            let foot_hits = hull
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .filter(|t| tri_box(t.map(|i| vertices[i as usize]), foot))
                .count();
            println!("M2 {} {} collision bounds {:?}..{:?}, {} faces, bodyhits{body_hits}, foothits{foot_hits}",d.unique_id,d.model,bounds.min,bounds.max,hull.triangle_count());
        }
        for c in &tile.chunks {
            if c.height_at(p.to_array()).is_none() {
                continue;
            }
            for t in c.indices.as_chunks::<3>().0 {
                let tri = t.map(|i| c.positions[i as usize]);
                if let Some(z) = benilla_formats::triangle_z_at(&tri, p.x, p.y) {
                    let t = tri.map(Vec3::from);
                    let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize();
                    println!("TERRAIN z{z} normal{n:?} vertices{t:?}");
                }
            }
        }
    }
    for tile in &tiles {
        for w in &tile.wmos {
            if !seen.insert(w.unique_id)
                || (Vec3::from(w.position).truncate() - p.truncate()).length() > 150.0
            {
                continue;
            }
            let m = wmo_model(&mut chain, &w.model)?;
            let q = rotation(w.rotation);
            let pos = Vec3::from(w.position);
            let b = m.bounds.transformed(pos, q, 1.0);
            if b.xy_distance(body) > 3.0 {
                continue;
            }
            let hit: Vec<_> = m
                .triangles
                .iter()
                .enumerate()
                .filter(|(_, t)| tri_box(t.map(|v| pos + q * v), body))
                .map(|(i, t)| (i, t.map(|v| pos + q * v)))
                .collect();
            println!(
                "WMO {} {} bounds {:?}..{:?} bodyfaces{:?}",
                w.unique_id, w.model, b.min, b.max, hit
            );
        }
    }
    Ok(())
}

fn draw_chunk(
    c: &ChunkMesh,
    svg: &mut String,
    report: &mut String,
    roads: &mut Vec<Vec3>,
) -> Result<()> {
    let Some(nw) = c.positions.first() else {
        return Ok(());
    };
    let road_indices: Vec<_> = c
        .layer_textures
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            let t = t.to_ascii_lowercase();
            t.contains("road") || t.contains("path") || t.contains("cobble")
        })
        .collect();
    if road_indices.is_empty() {
        return Ok(());
    }
    for row in 0..16 {
        for col in 0..16 {
            let x = nw[0] - (row as f32 + 0.5) * benilla_formats::CHUNK_SIZE / 16.0;
            let y = nw[1] - (col as f32 + 0.5) * benilla_formats::CHUNK_SIZE / 16.0;
            if (x - CENTER.x).abs() > RADIUS || (y - CENTER.y).abs() > RADIUS {
                continue;
            }
            let (layer, tex) = road_indices[0];
            let weight = layer_weight(c, layer, row * 4 + 2, col * 4 + 2);
            if weight < 0.3 {
                continue;
            }
            let z = c.height_at([x, y, 0.0]).unwrap_or(f32::NAN);
            writeln!(report, "{x:.3}\t{y:.3}\t{z:.3}\t{tex}\t{weight:.3}")?;
            if weight > 0.3 && z.is_finite() {
                roads.push(Vec3::new(x, y, z));
            }
            writeln!(svg,"<rect x=\"{}\" y=\"{}\" width=\"2.1\" height=\"2.1\" fill=\"#c6a676\" fill-opacity=\"{weight}\"/>",CENTER.y-y-1.05,CENTER.x-x-1.05)?;
        }
    }
    Ok(())
}
fn layer_weight(c: &ChunkMesh, layer: usize, row: usize, col: usize) -> f32 {
    let Some(alpha) = &c.alpha_map else {
        return if layer == 0 { 1.0 } else { 0.0 };
    };
    let i = (row * 64 + col) * 4;
    let a = |k: usize| alpha.get(i + k - 1).copied().unwrap_or(0) as f32 / 255.0;
    let base = if layer == 0 { 1.0 } else { a(layer) };
    (layer + 1..c.layer_textures.len()).fold(base, |weight, k| weight * (1.0 - a(k)))
}

struct Candidate {
    position: Vec3,
    rotation: [f32; 3],
    bounds: Bounds,
    road: Vec3,
    relief: f32,
    clearance: f32,
    omit: BTreeSet<u32>,
}
struct ProtectedSite {
    p: Vec3,
    radius: f32,
}
struct ProtectedGameplay {
    sites: Vec<ProtectedSite>,
    segments: Vec<(Vec3, Vec3)>,
}
fn protected_gameplay() -> Result<ProtectedGameplay> {
    let manifest = std::fs::read_to_string("data/world/elwynn-westfall.txt")?;
    let mut sites = Vec::new();
    let mut paths = BTreeMap::<(u32, u32), Vec3>::new();
    let nearby = |p: Vec3| {
        (p.x - CENTER.x).abs() < RADIUS + 100.0 && (p.y - CENTER.y).abs() < RADIUS + 100.0
    };
    for line in manifest.lines() {
        let f: Vec<_> = line.split('\t').collect();
        let offset = match f.first().copied() {
            Some("spawn") => 7,
            Some("objspawn" | "waypoint") => 3,
            _ => continue,
        };
        let read =
            |i: usize| -> Result<f32> { Ok(f.get(i).context("short gameplay row")?.parse()?) };
        let p = Vec3::new(read(offset)?, read(offset + 1)?, read(offset + 2)?);
        if f[0] == "waypoint" {
            paths.insert((f[1].parse()?, f[2].parse()?), p);
        }
        if nearby(p) {
            sites.push(ProtectedSite {
                p,
                radius: if f[0] == "objspawn" { 2.0 } else { 1.5 },
            });
        }
    }
    let mut segments = Vec::new();
    let mut before = None;
    for ((guid, point), p) in paths {
        if let Some((last_guid, last_point, last)) = before {
            if guid == last_guid && point > last_point && (nearby(p) || nearby(last)) {
                segments.push((last, p));
            }
        }
        before = Some((guid, point, p));
    }
    // Source literals, read rather than duplicated so new fixed treasures get protection too.
    for line in std::fs::read_to_string("crates/wow-souls/src/loot/treasures.rs")?.lines() {
        let Some(s) = line.trim().strip_prefix("wow: [") else {
            continue;
        };
        let Some(s) = s.split(']').next() else {
            continue;
        };
        let values = s
            .split(',')
            .map(|v| v.trim().parse::<f32>())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if values.len() == 3 {
            let p = Vec3::new(values[0], values[1], values[2]);
            if nearby(p) {
                sites.push(ProtectedSite { p, radius: 2.5 });
            }
        }
    }
    println!("protected gameplay: {} nearby creature/object/patrol/treasure points, {} original patrol segments",sites.len(),segments.len());
    Ok(ProtectedGameplay { sites, segments })
}
fn gameplay_conflict(protected: &ProtectedGameplay, p: Vec3, q: Quat, bounds: Bounds) -> bool {
    let inv = q.inverse();
    for site in &protected.sites {
        let local = inv * (site.p - p);
        if local.z + 2.4 < bounds.min.z || local.z > bounds.max.z {
            continue;
        }
        if local.x >= bounds.min.x - site.radius
            && local.x <= bounds.max.x + site.radius
            && local.y >= bounds.min.y - site.radius
            && local.y <= bounds.max.y + site.radius
        {
            return true;
        }
    }
    let b = Bounds {
        min: bounds.min - Vec3::new(1.5, 1.5, 2.4),
        max: bounds.max + Vec3::new(1.5, 1.5, 0.0),
    };
    protected
        .segments
        .iter()
        .any(|(a, v)| segment_box(inv * (*a - p), inv * (*v - p), b))
}
fn segment_box(a: Vec3, b: Vec3, bounds: Bounds) -> bool {
    let d = b - a;
    let mut low: f32 = 0.0;
    let mut high: f32 = 1.0;
    for axis in 0..3 {
        if d[axis].abs() < 1e-8 {
            if a[axis] < bounds.min[axis] || a[axis] > bounds.max[axis] {
                return false;
            }
            continue;
        }
        let t = (bounds.min[axis] - a[axis]) / d[axis];
        let u = (bounds.max[axis] - a[axis]) / d[axis];
        low = low.max(t.min(u));
        high = high.min(t.max(u));
        if low > high {
            return false;
        }
    }
    true
}
fn candidates(
    prefix: &std::path::Path,
    tiles: &[TileMesh],
    obstacles: &[Obstacle],
    models: &BTreeMap<String, WmoModel>,
    roads: &[Vec3],
    edges: &[(usize, usize)],
    svg: &mut String,
) -> Result<()> {
    let model_path = "world\\wmo\\azeroth\\buildings\\human_farm\\farm.wmo";
    let model = &models[model_path];
    let protected = protected_gameplay()?;
    let mut valid = Vec::new();
    let mut count = [0; 4];
    for ix in -92..=92 {
        for iy in -92..=92 {
            let p = CENTER + Vec2::new(ix as f32 * 4.0, iy as f32 * 4.0);
            if p.distance(CENTER) > 370.0 {
                continue;
            }
            let Some(road) = roads.iter().min_by(|a, b| {
                a.truncate()
                    .distance_squared(p)
                    .total_cmp(&b.truncate().distance_squared(p))
            }) else {
                continue;
            };
            let dist = road.truncate().distance(p);
            if !(17.0..=38.0).contains(&dist) {
                continue;
            }
            count[0] += 1;
            let toward = road.truncate() - p;
            let heading = (toward.y.atan2(toward.x).to_degrees() + 270.0).rem_euclid(360.0);
            let rot = [0.0, heading, 0.0];
            let q = rotation(rot);
            let Some(center_z) = ground(tiles, p) else {
                continue;
            };
            if gameplay_conflict(&protected, p.extend(center_z - 3.75), q, model.bounds) {
                continue;
            }
            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            let mut missing = false;
            // A full metre/yard grid across the rotated 19x18 yd original footprint.
            for x in 0..=20 {
                for y in 0..=20 {
                    let local = Vec3::new(
                        model.bounds.min.x
                            + (model.bounds.max.x - model.bounds.min.x) * x as f32 / 20.0,
                        model.bounds.min.y
                            + (model.bounds.max.y - model.bounds.min.y) * y as f32 / 20.0,
                        0.0,
                    );
                    let world = p + (q * local).truncate();
                    if let Some(z) = ground(tiles, world) {
                        min = min.min(z);
                        max = max.max(z);
                    } else {
                        missing = true;
                    }
                }
            }
            let Some((exact_min, exact_max, _)) = terrain_range(tiles, p, q, model.bounds) else {
                continue;
            };
            min = min.min(exact_min);
            max = max.max(exact_max);
            if missing || max - min > 1.5 {
                continue;
            }
            count[2] += 1;
            // Native farms sink the 4-yard stone foundation into terrain. Keep the living floor
            // above every sampled terrain column; this leaves just the native top doorstep exposed.
            let position = p.extend(max - 3.75);
            if gameplay_conflict(&protected, position, q, model.bounds) {
                continue;
            }
            let bounds = model.bounds.transformed(position, q, 1.0);
            let mut omit = BTreeSet::new();
            let mut invalid = false;
            let mut clearance = f32::INFINITY;
            for o in obstacles {
                let d = bounds.xy_distance(o.bounds);
                if d > 1.0 || o.bounds.max.z < bounds.min.z || o.bounds.min.z > bounds.max.z {
                    continue;
                }
                if !obstacle_intersects(o, position, q, &model.profile) {
                    clearance = clearance.min(d);
                    continue;
                }
                let plant = o.model.contains("\\trees\\")
                    || o.model.contains("\\bush\\")
                    || o.model.contains("\\bushes\\")
                    || o.model.contains("\\plants\\");
                let central_tree =
                    o.model.contains("\\trees\\") && o.position.truncate().distance(CENTER) < 65.0;
                if plant && !central_tree {
                    omit.insert(o.id);
                } else {
                    invalid = true;
                    break;
                }
            }
            if invalid || omit.len() > 7 {
                continue;
            }
            count[1] += 1;
            let porch = (position + q * Vec3::new(-1.5399, -10.0, 0.0)).truncate();
            let mut blocked = false;
            let mut last = None;
            let steps = (porch.distance(road.truncate()) / 0.5).ceil() as usize;
            for i in 0..=steps {
                let point = porch.lerp(road.truncate(), i as f32 / steps.max(1) as f32);
                let Some(z) = ground(tiles, point) else {
                    blocked = true;
                    break;
                };
                let capsule = Bounds {
                    min: (point - Vec2::splat(0.7)).extend(z + 0.15),
                    max: (point + Vec2::splat(0.7)).extend(z + 2.0),
                };
                for o in obstacles {
                    if omit.contains(&o.id)
                        || o.bounds.min.z >= capsule.max.z
                        || o.bounds.max.z <= capsule.min.z
                        || o.bounds.xy_distance(capsule) > 0.01
                    {
                        continue;
                    }
                    if !triangle_obstacle_box(o, capsule) {
                        continue;
                    }
                    let plant = o.model.contains("\\trees\\")
                        || o.model.contains("\\bush\\")
                        || o.model.contains("\\bushes\\")
                        || o.model.contains("\\plants\\");
                    if plant
                        && !(o.model.contains("\\trees\\")
                            && o.position.truncate().distance(CENTER) < 65.0)
                    {
                        omit.insert(o.id);
                    } else {
                        blocked = true;
                        break;
                    }
                }
                if blocked {
                    break;
                }
                if last.is_some_and(|h: f32| (h - z).abs() > 0.55) {
                    blocked = true;
                    break;
                }
                last = Some(z);
            }
            if blocked {
                continue;
            }
            count[3] += 1;
            let c = Candidate {
                position,
                rotation: rot,
                bounds,
                road: *road,
                relief: max - min,
                clearance: clearance.max(0.65),
                omit,
            };
            let access = entry_path(&c, model, tiles);
            if access.windows(2).any(|p| (p[0].z - p[1].z).abs() > 0.55) {
                continue;
            }
            valid.push(c);
        }
    }
    valid.sort_by(|a, b| {
        (a.position.truncate().distance(CENTER) + a.relief * 40.0 + a.omit.len() as f32 * 12.0)
            .total_cmp(
                &(b.position.truncate().distance(CENTER)
                    + b.relief * 40.0
                    + b.omit.len() as f32 * 12.0),
            )
    });
    let mut selected: Vec<Candidate> = Vec::new();
    for c in valid {
        if selected
            .iter()
            .any(|v| v.bounds.xy_distance(c.bounds) < 8.0)
        {
            continue;
        }
        let access = entry_path(&c, model, tiles);
        if selected.iter().any(|v| {
            access.iter().any(|p| {
                v.bounds.xy_distance(Bounds {
                    min: *p - Vec3::splat(0.8),
                    max: *p + Vec3::splat(0.8),
                }) < 0.001
            })
        }) {
            continue;
        }
        if selected.iter().any(|v| {
            entry_path(v, model, tiles).iter().any(|p| {
                c.bounds.xy_distance(Bounds {
                    min: *p - Vec3::splat(0.8),
                    max: *p + Vec3::splat(0.8),
                }) < 0.001
            })
        }) {
            continue;
        }
        selected.push(c);
        if selected.len() >= 14 {
            break;
        }
    }
    anyhow::ensure!(
        selected.len() == 14,
        "only {} validated homes",
        selected.len()
    );
    let mut checks = String::from("home_id\tterrain_min\tterrain_max\tclipped_terrain_triangles\tfoundation_embed_min\tliving_floor_clearance_min\tmaximum_entry_z_step\tentry_points\tomitted_native_ids\n");
    for (i, c) in selected.iter().enumerate() {
        let (min, max, n) = terrain_range(
            tiles,
            c.position.truncate(),
            rotation(c.rotation),
            model.bounds,
        )
        .context("selected terrain coverage")?;
        let embed = min - c.bounds.min.z;
        let floor_clearance = c.position.z + 4.0525 - max;
        let access = entry_path(c, model, tiles);
        let step = access
            .windows(2)
            .map(|p| (p[0].z - p[1].z).abs())
            .fold(0.0, f32::max);
        anyhow::ensure!(
            embed > 2.2 && floor_clearance > 0.29 && step <= 0.55,
            "home{} failed final ground/entry check",
            i + 1
        );
        writeln!(
            checks,
            "{}\t{min:.4}\t{max:.4}\t{n}\t{embed:.4}\t{floor_clearance:.4}\t{step:.4}\t{}\t{:?}",
            i + 1,
            access.len(),
            c.omit
        )?;
    }
    std::fs::write(prefix.with_extension("checks.tsv"), checks)?;
    let mut out=String::from("home_id\tmodel\tx\ty\tz\trx\try\trz\tdoodad_set\tmin_x\tmin_y\tmin_z\tmax_x\tmax_y\tmax_z\tbed_x\tbed_y\tbed_z\tdoor_x\tdoor_y\tdoor_z\tporch_x\tporch_y\tporch_z\troad_x\troad_y\troad_z\trelief\tclearance\n");
    let (runtime_roads, runtime_edges) =
        compact_road_graph(roads, edges, &selected, tiles, obstacles);
    let roads = &runtime_roads;
    let edges = &runtime_edges;
    let mut village=String::from("wsvillage 1\n# Generated by goldshire_survey: native terrain samples, cottage source collision, foliage triangles.\n# All coordinates are raw WoW yards; rotation is native MODF Euler degrees.\n");
    for (i, p) in roads.iter().enumerate() {
        writeln!(
            village,
            "node\t{}\t{:.3}\t{:.3}\t{:.3}",
            i + 1,
            p.x,
            p.y,
            p.z
        )?;
    }
    for (a, b) in edges {
        writeln!(village, "edge\t{}\t{}", a + 1, b + 1)?;
        let p = roads[*a];
        let q = roads[*b];
        writeln!(svg,"<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#627c94\" stroke-width=\"0.7\"/>",CENTER.y-p.y,CENTER.x-p.x,CENTER.y-q.y,CENTER.x-q.x)?;
    }
    for (name, p) in [
        ("work", Vec2::new(-9450.0, 90.0)),
        ("social", Vec2::new(-9440.0, 50.0)),
        ("travel", Vec2::new(-9075.0, 430.0)),
    ] {
        if let Some((i, _)) = roads.iter().enumerate().min_by(|(_, a), (_, b)| {
            a.truncate()
                .distance_squared(p)
                .total_cmp(&b.truncate().distance_squared(p))
        }) {
            writeln!(village, "anchor\t{name}\t{}", i + 1)?;
        }
    }
    for (i, c) in selected.iter().enumerate() {
        let q = rotation(c.rotation);
        let bed = c.position + q * Vec3::new(2.0, 2.0, 4.11);
        let door = c.position + q * Vec3::new(-1.5399, -6.0261, 4.1093);
        let porch_xy = (c.position + q * Vec3::new(-1.5399, -10.0, 0.0)).truncate();
        let porch = porch_xy.extend(ground(tiles, porch_xy).unwrap_or(c.position.z));
        let b = c.bounds;
        let line=format!("{}\t{model_path}\t{:.3}\t{:.3}\t{:.3}\t0\t{:.3}\t0\t0\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}",i+1,c.position.x,c.position.y,c.position.z,c.rotation[1],b.min.x,b.min.y,b.min.z,b.max.x,b.max.y,b.max.z,bed.x,bed.y,bed.z,door.x,door.y,door.z,porch.x,porch.y,porch.z,c.road.x,c.road.y,c.road.z,c.relief,c.clearance);
        writeln!(out, "{line}")?;
        writeln!(village, "home\t{line}")?;
        for (slot, local) in [
            Vec3::new(2.0, 2.0, 4.1093),
            Vec3::new(0.0, 2.0, 4.1093),
            Vec3::new(-2.0, 2.0, 4.1093),
        ]
        .into_iter()
        .enumerate()
        {
            let start = Vec3::new(2.0, 2.0, 4.1093);
            for step in 0..=20 {
                let p = start.lerp(local, step as f32 / 20.0);
                let capsule = Bounds {
                    min: p - Vec3::new(0.55, 0.55, -0.15),
                    max: p + Vec3::new(0.55, 0.55, 2.0),
                };
                anyhow::ensure!(
                    !model.triangles.iter().any(|t| tri_box(*t, capsule)),
                    "rest slot {slot}: blocked native room route at {p:?}"
                );
                let floor = model
                    .triangles
                    .iter()
                    .filter_map(|t| {
                        benilla_formats::triangle_z_at(&t.map(|v| v.to_array()), p.x, p.y)
                    })
                    .filter(|z| *z <= 4.2)
                    .fold(f32::NEG_INFINITY, f32::max);
                anyhow::ensure!(
                    (floor - 4.1093).abs() < 0.05,
                    "rest slot {slot}: missing source floor {floor}"
                );
            }
            let p = c.position + q * local;
            writeln!(
                village,
                "rest\t{}\t{slot}\t{:.3}\t{:.3}\t{:.3}",
                i + 1,
                p.x,
                p.y,
                p.z
            )?;
        }
        let access = entry_path(c, model, tiles);
        for (j, p) in access.iter().enumerate() {
            writeln!(
                village,
                "entry\t{}\t{j}\t{:.3}\t{:.3}\t{:.3}",
                i + 1,
                p.x,
                p.y,
                p.z
            )?;
        }
        writeln!(svg,"<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#edfaff\" stroke=\"#135ec5\" stroke-width=\"1\"/><text x=\"{}\" y=\"{}\" font-size=\"7\" fill=\"#051c37\">{}</text><line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#135ec5\" stroke-width=\"1\"/>",CENTER.y-b.max.y,CENTER.x-b.max.x,b.max.y-b.min.y,b.max.x-b.min.x,CENTER.y-c.position.y,CENTER.x-c.position.x,i+1,CENTER.y-porch.y,CENTER.x-porch.x,CENTER.y-c.road.y,CENTER.x-c.road.x)?;
    }
    std::fs::write(prefix.with_extension("candidates.tsv"), out)?;
    let omits: BTreeSet<_> = selected
        .iter()
        .flat_map(|c| c.omit.iter().copied())
        .collect();
    let mut omit_report = String::from("uid\tmodel\tx\ty\tz\n");
    for id in &omits {
        if let Some(o) = obstacles.iter().find(|o| o.id == *id) {
            writeln!(
                omit_report,
                "{}\t{}\t{}\t{}\t{}",
                o.id, o.model, o.position.x, o.position.y, o.position.z
            )?;
        }
    }
    for id in &omits {
        writeln!(village, "omit\t{id}")?;
        if let Some(o) = obstacles.iter().find(|o| o.id == *id) {
            let x = CENTER.y - o.position.y;
            let y = CENTER.x - o.position.x;
            writeln!(
                svg,
                "<path d=\"M{},{}l4,4m-4,0l4,-4\" stroke=\"#ab2c23\" stroke-width=\"1\"/>",
                x - 2.0,
                y - 2.0
            )?;
        }
    }
    std::fs::write(prefix.with_extension("superseded-housing.tsv"), village)?;
    std::fs::write(prefix.with_extension("omit.tsv"), omit_report)?;
    let mut stairs = String::from("local_x\tlocal_y\tupward_collision_z_below_4_5\n");
    for i in -48..=-16 {
        let y = i as f32 / 4.0;
        let heights: Vec<_> = model
            .triangles
            .iter()
            .filter(|t| (t[1] - t[0]).cross(t[2] - t[0]).z > 0.0)
            .filter_map(|t| benilla_formats::triangle_z_at(&t.map(|p| p.to_array()), -1.5399, y))
            .filter(|z| *z <= 4.5)
            .collect();
        writeln!(stairs, "-1.5399\t{y}\t{:?}", heights)?;
    }
    std::fs::write(prefix.with_extension("stairs.tsv"), stairs)?;
    println!(
        "omits {} native props, {} trees",
        omits.len(),
        obstacles
            .iter()
            .filter(|o| omits.contains(&o.id) && o.model.contains("\\trees\\"))
            .count()
    );
    println!("candidate scan {:?}: road-proximate, no-bbox-conflict, terrain-relief<=1.5, approach-clear; selected {}",count,selected.len());
    Ok(())
}

fn obstacle_intersects(o: &Obstacle, position: Vec3, q: Quat, profile: &[Bounds]) -> bool {
    if o.triangles.is_empty() {
        return true;
    }
    let inv = q.inverse();
    let mut broad = Bounds::empty();
    for b in profile {
        broad.add(b.min);
        broad.add(b.max);
    }
    broad.min -= Vec3::splat(0.65);
    broad.max += Vec3::splat(0.65);
    let broad = broad.transformed(position, q, 1.0);
    o.triangles.iter().any(|tri| {
        let min = tri[0].min(tri[1]).min(tri[2]);
        let max = tri[0].max(tri[1]).max(tri[2]);
        if min.cmpgt(broad.max).any() || max.cmplt(broad.min).any() {
            return false;
        }
        let t = tri.map(|v| inv * (v - position));
        profile.iter().any(|b| {
            tri_box(
                t,
                Bounds {
                    min: b.min - Vec3::splat(0.65),
                    max: b.max + Vec3::splat(0.65),
                },
            )
        })
    })
}
fn triangle_obstacle_box(o: &Obstacle, b: Bounds) -> bool {
    o.triangles.is_empty() || o.triangles.iter().any(|t| tri_box(*t, b))
}
/// Triangle/AABB separating-axis test; includes foliage triangles, not just collision trunks.
fn tri_box(tri: [Vec3; 3], b: Bounds) -> bool {
    let center = (b.min + b.max) * 0.5;
    let ext = (b.max - b.min) * 0.5;
    let t = tri.map(|v| v - center);
    let edges = [t[1] - t[0], t[2] - t[1], t[0] - t[2]];
    let separated = |axis: Vec3| {
        if axis.length_squared() < 1e-12 {
            return false;
        }
        let d = t.map(|v| v.dot(axis));
        let r = ext.dot(axis.abs());
        d.into_iter().fold(f32::INFINITY, f32::min) > r
            || d.into_iter().fold(f32::NEG_INFINITY, f32::max) < -r
    };
    for axis in [Vec3::X, Vec3::Y, Vec3::Z, edges[0].cross(edges[1])] {
        if separated(axis) {
            return false;
        }
    }
    for e in edges {
        for a in [Vec3::X, Vec3::Y, Vec3::Z] {
            if separated(e.cross(a)) {
                return false;
            }
        }
    }
    true
}

fn road_graph(
    input: &[Vec3],
    tiles: &[TileMesh],
    obstacles: &[Obstacle],
) -> (Vec<Vec3>, Vec<(usize, usize)>) {
    let step = benilla_formats::CHUNK_SIZE / 16.0;
    let mut lookup = BTreeMap::new();
    let mut positions = Vec::new();
    for p in input {
        let cube = Bounds {
            min: *p - Vec3::new(0.45, 0.45, -0.2),
            max: *p + Vec3::new(0.45, 0.45, 2.0),
        };
        if obstacles.iter().any(|o| {
            o.bounds.min.z < cube.max.z
                && o.bounds.max.z > cube.min.z
                && o.bounds.xy_distance(cube) < 0.001
                && triangle_obstacle_box(o, cube)
        }) {
            continue;
        }
        let key = (
            (p.x / step - 0.5).round() as i32,
            (p.y / step - 0.5).round() as i32,
        );
        lookup.insert(key, positions.len());
        positions.push(*p);
    }
    let mut neighbours = vec![Vec::new(); positions.len()];
    for (&(x, y), &i) in &lookup {
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let Some(&j) = lookup.get(&(x + dx, y + dy)) else {
                    continue;
                };
                let a = positions[i];
                let b = positions[j];
                if a.distance(b) > 3.2 || (a.z - b.z).abs() > 1.0 {
                    continue;
                }
                let mid = (a + b) * 0.5;
                let Some(z) = ground(tiles, mid.truncate()) else {
                    continue;
                };
                let cube = Bounds {
                    min: mid.truncate().extend(z) - Vec3::new(0.45, 0.45, -0.2),
                    max: mid.truncate().extend(z) + Vec3::new(0.45, 0.45, 2.0),
                };
                if obstacles.iter().any(|o| {
                    o.bounds.min.z < cube.max.z
                        && o.bounds.max.z > cube.min.z
                        && o.bounds.xy_distance(cube) < 0.001
                        && triangle_obstacle_box(o, cube)
                }) {
                    continue;
                }
                neighbours[i].push(j);
            }
        }
    }
    // Native alpha feathers and isolated transparent-card bounds can leave a few raster
    // texels absent. Bridge only <=10yd between paved components and sample every .35yd.
    let mut components = vec![usize::MAX; positions.len()];
    let mut component_count = 0;
    for start in 0..positions.len() {
        if components[start] != usize::MAX {
            continue;
        }
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(i) = queue.pop_front() {
            if components[i] != usize::MAX {
                continue;
            }
            components[i] = component_count;
            queue.extend(
                neighbours[i]
                    .iter()
                    .copied()
                    .filter(|i| components[*i] == usize::MAX),
            );
        }
        component_count += 1;
    }
    let mut bridges = BTreeMap::<(usize, usize), Vec<(f32, usize, usize)>>::new();
    for (&(x, y), &i) in &lookup {
        for dx in -5..=5 {
            for dy in -5..=5 {
                let Some(&j) = lookup.get(&(x + dx, y + dy)) else {
                    continue;
                };
                if j <= i || components[i] == components[j] {
                    continue;
                }
                let distance = positions[i].distance(positions[j]);
                if distance > 10.1 {
                    continue;
                }
                let key = (
                    components[i].min(components[j]),
                    components[i].max(components[j]),
                );
                let v = bridges.entry(key).or_default();
                v.push((distance, i, j));
                v.sort_by(|a, b| a.0.total_cmp(&b.0));
                v.truncate(8);
            }
        }
    }
    let mut bridge_count = 0;
    for choices in bridges.values() {
        for &(_, i, j) in choices {
            if walk_segment_clear(positions[i], positions[j], tiles, obstacles) {
                neighbours[i].push(j);
                neighbours[j].push(i);
                bridge_count += 1;
                break;
            }
        }
    }
    let Some((start, _)) = positions.iter().enumerate().min_by(|(_, a), (_, b)| {
        a.truncate()
            .distance_squared(CENTER)
            .total_cmp(&b.truncate().distance_squared(CENTER))
    }) else {
        return (Vec::new(), Vec::new());
    };
    let mut connected = BTreeSet::new();
    let mut queue = std::collections::VecDeque::from([start]);
    while let Some(i) = queue.pop_front() {
        if !connected.insert(i) {
            continue;
        }
        queue.extend(
            neighbours[i]
                .iter()
                .copied()
                .filter(|i| !connected.contains(i)),
        );
    }
    let mut out = Vec::new();
    let mut remap = BTreeMap::new();
    for &i in &connected {
        remap.insert(i, out.len());
        out.push(positions[i]);
    }
    let mut edges = Vec::new();
    for &i in &connected {
        for j in &neighbours[i] {
            if *j > i && connected.contains(j) {
                edges.push((remap[&i], remap[j]));
            }
        }
    }
    let travel = Vec2::new(-9075.0, 430.0);
    let travel_distance = out
        .iter()
        .map(|p| p.truncate().distance(travel))
        .fold(f32::INFINITY, f32::min);
    let closest = out.iter().min_by(|a, b| {
        a.truncate()
            .distance_squared(travel)
            .total_cmp(&b.truncate().distance_squared(travel))
    });
    println!("roads {} raw / {} clear / {} connected central; {} edges ({bridge_count} safe raster bridges); travel target distance {travel_distance:.1} {closest:?}",input.len(),positions.len(),out.len(),edges.len());
    (out, edges)
}

fn walk_segment_clear(a: Vec3, b: Vec3, tiles: &[TileMesh], obstacles: &[Obstacle]) -> bool {
    let broad = Bounds {
        min: a.min(b) - Vec3::new(0.5, 0.5, 0.0),
        max: a.max(b) + Vec3::new(0.5, 0.5, 2.0),
    };
    let local: Vec<_> = obstacles
        .iter()
        .filter(|o| {
            o.bounds.min.z < broad.max.z
                && o.bounds.max.z > broad.min.z
                && o.bounds.xy_distance(broad) < 0.001
        })
        .collect();
    let n = (a.distance(b) / 0.35).ceil().max(1.0) as usize;
    let mut last = a.z;
    for i in 0..=n {
        let p = a.truncate().lerp(b.truncate(), i as f32 / n as f32);
        let Some(z) = ground(tiles, p) else {
            return false;
        };
        if (last - z).abs() > 0.4 {
            return false;
        }
        last = z;
        let cube = Bounds {
            min: p.extend(z) - Vec3::new(0.45, 0.45, -0.2),
            max: p.extend(z) + Vec3::new(0.45, 0.45, 2.0),
        };
        if local.iter().any(|o| triangle_obstacle_box(o, cube)) {
            return false;
        }
    }
    true
}

fn entry_path(c: &Candidate, model: &WmoModel, tiles: &[TileMesh]) -> Vec<Vec3> {
    let q = rotation(c.rotation);
    let local = [
        Vec3::new(-1.5399, -10.0, 0.0),
        Vec3::new(-1.5399, -6.0261, 4.1093),
        Vec3::new(-1.5399, -4.0, 4.1093),
        Vec3::new(0.0, -1.8, 4.1093),
        Vec3::new(0.0, 1.8, 4.1093),
        Vec3::new(2.0, 2.0, 4.1093),
    ];
    let mut path = Vec::new();
    let porch = (c.position + q * local[0]).truncate();
    let count = (c.road.truncate().distance(porch) / 0.4).ceil().max(1.0) as usize;
    for i in 0..=count {
        let p = c.road.truncate().lerp(porch, i as f32 / count as f32);
        if let Some(z) = ground(tiles, p) {
            path.push(p.extend(z));
        }
    }
    for pair in local.windows(2) {
        let count = (pair[0].truncate().distance(pair[1].truncate()) / 0.1)
            .ceil()
            .max(1.0) as usize;
        for i in 1..=count {
            let p = pair[0].lerp(pair[1], i as f32 / count as f32);
            let world = c.position + q * p;
            let terrain = ground(tiles, world.truncate()).unwrap_or(c.position.z);
            let house = model
                .triangles
                .iter()
                .filter(|t| (t[1] - t[0]).cross(t[2] - t[0]).z > 0.0)
                .filter_map(|t| benilla_formats::triangle_z_at(&t.map(|p| p.to_array()), p.x, p.y))
                .filter(|z| *z < 4.5)
                .fold(f32::NEG_INFINITY, f32::max)
                + c.position.z;
            path.push(world.truncate().extend(terrain.max(house)));
        }
    }
    path
}

fn compact_road_graph(
    positions: &[Vec3],
    edges: &[(usize, usize)],
    homes: &[Candidate],
    tiles: &[TileMesh],
    obstacles: &[Obstacle],
) -> (Vec<Vec3>, Vec<(usize, usize)>) {
    let nearest = |p: Vec2| {
        positions
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.truncate()
                    .distance_squared(p)
                    .total_cmp(&b.truncate().distance_squared(p))
            })
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    let root = nearest(Vec2::new(-9450.0, 90.0));
    let mut terminals = BTreeSet::from([
        root,
        nearest(Vec2::new(-9440.0, 50.0)),
        nearest(Vec2::new(-9075.0, 430.0)),
    ]);
    for h in homes {
        terminals.insert(nearest(h.road.truncate()));
    }
    let mut adjacency = vec![Vec::new(); positions.len()];
    for &(a, b) in edges {
        let w = positions[a].distance(positions[b]);
        adjacency[a].push((b, w));
        adjacency[b].push((a, w));
    }
    let mut distance = vec![f32::INFINITY; positions.len()];
    let mut parent = vec![usize::MAX; positions.len()];
    let mut heap = std::collections::BinaryHeap::new();
    distance[root] = 0.0;
    heap.push(std::cmp::Reverse((0u32, root)));
    while let Some(std::cmp::Reverse((bits, i))) = heap.pop() {
        let d = f32::from_bits(bits);
        if d > distance[i] {
            continue;
        }
        for &(j, w) in &adjacency[i] {
            let next = d + w;
            if next < distance[j] {
                distance[j] = next;
                parent[j] = i;
                heap.push(std::cmp::Reverse((next.to_bits(), j)));
            }
        }
    }
    let mut used = BTreeSet::new();
    let mut tree_edges = BTreeSet::new();
    for &t in &terminals {
        let mut at = t;
        used.insert(at);
        while at != root && parent[at] != usize::MAX {
            let next = parent[at];
            tree_edges.insert((at.min(next), at.max(next)));
            used.insert(next);
            at = next;
        }
    }
    let mut adjacency = vec![Vec::new(); positions.len()];
    for &(a, b) in &tree_edges {
        adjacency[a].push(b);
        adjacency[b].push(a);
    }
    let retained: BTreeSet<_> = used
        .iter()
        .copied()
        .filter(|i| terminals.contains(i) || adjacency[*i].len() != 2)
        .collect();
    let mut simplified = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for &start in &retained {
        for &next in &adjacency[start] {
            if visited.contains(&(start.min(next), start.max(next))) {
                continue;
            }
            let mut chain = vec![start];
            let mut before = start;
            let mut at = next;
            loop {
                visited.insert((before.min(at), before.max(at)));
                chain.push(at);
                if retained.contains(&at) {
                    break;
                }
                let next = adjacency[at]
                    .iter()
                    .copied()
                    .find(|j| *j != before)
                    .unwrap_or(before);
                before = at;
                at = next;
            }
            let mut a = 0;
            while a + 1 < chain.len() {
                let mut chosen = a + 1;
                for b in (a + 2..chain.len()).rev() {
                    let p = positions[chain[a]].truncate();
                    let q = positions[chain[b]].truncate();
                    let delta = q - p;
                    if delta.length() > 25.0 {
                        continue;
                    }
                    if chain[a + 1..b].iter().any(|i| {
                        let t = ((positions[*i].truncate() - p).dot(delta)
                            / delta.length_squared())
                        .clamp(0.0, 1.0);
                        positions[*i].truncate().distance(p + delta * t) > 0.9
                    }) {
                        continue;
                    }
                    if walk_segment_clear(
                        positions[chain[a]],
                        positions[chain[b]],
                        tiles,
                        obstacles,
                    ) {
                        chosen = b;
                        break;
                    }
                }
                let i = chain[a];
                let j = chain[chosen];
                simplified.insert((i.min(j), i.max(j)));
                a = chosen;
            }
        }
    }
    let nodes: BTreeSet<_> = simplified.iter().flat_map(|(a, b)| [*a, *b]).collect();
    let mut remap = BTreeMap::new();
    let mut out = Vec::new();
    for &i in &nodes {
        remap.insert(i, out.len());
        out.push(positions[i]);
    }
    let out_edges: Vec<_> = simplified
        .into_iter()
        .map(|(a, b)| (remap[&a], remap[&b]))
        .collect();
    println!(
        "runtime roads: {} path-union nodes => {} vertices / {} edges",
        used.len(),
        out.len(),
        out_edges.len()
    );
    (out, out_edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_placement_yaw_uses_modf_heading_offset() {
        assert!((rotation([0.0, 180.0, 0.0]) * Vec3::X - Vec3::X).length() < 1e-5);
        assert!((rotation([0.0, 90.0, 0.0]) * Vec3::X - Vec3::NEG_Y).length() < 1e-5);
    }

    #[test]
    fn triangle_bounds_overlap_does_not_imply_contact() {
        let b = Bounds {
            min: Vec3::splat(-1.0),
            max: Vec3::splat(1.0),
        };
        assert!(!tri_box(
            [
                Vec3::new(3.0, 0.0, 0.0),
                Vec3::new(0.0, 3.0, 0.0),
                Vec3::new(3.0, 3.0, 0.0)
            ],
            b
        ));
        assert!(tri_box(
            [
                Vec3::new(0.0, -2.0, 0.0),
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::new(0.0, 0.0, 2.0)
            ],
            b
        ));
    }

    #[test]
    fn terrain_extrema_include_peak_between_grid_samples() {
        let mut c = ChunkMesh {
            positions: vec![
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [5.123, 5.123, 3.0],
            ],
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: vec![0, 1, 4, 1, 3, 4, 3, 2, 4, 2, 0, 4],
            holes: 0,
            base_texture: None,
            layer_textures: Vec::new(),
            layer_effect_ids: Vec::new(),
            alpha_map: None,
            shadow: None,
            pred_tex: [0; 64],
            no_effect_doodad: [false; 64],
            index_x: 0,
            index_y: 0,
            area_id: 0,
            impassable: false,
            liquids: Vec::new(),
        };
        let t = TileMesh {
            chunks: vec![c.clone()],
            doodads: Vec::new(),
            wmos: Vec::new(),
        };
        let b = Bounds {
            min: Vec3::new(0.0, 0.0, -1.0),
            max: Vec3::new(10.0, 10.0, 5.0),
        };
        let (_, max, _) = terrain_range(&[t], Vec2::ZERO, Quat::IDENTITY, b).unwrap();
        assert!((max - 3.0).abs() < 1e-5);
        c.indices.clear();
        let t = TileMesh {
            chunks: vec![c],
            doodads: Vec::new(),
            wmos: Vec::new(),
        };
        assert!(terrain_range(&[t], Vec2::ZERO, Quat::IDENTITY, b).is_none());
    }
}
