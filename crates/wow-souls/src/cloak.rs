//! Cloaks as cloth. 1.12 draws a cloak as a cape geoset (group 15, styles 1502-1506) skinned
//! rigidly to the spine, so on a moving body it hugs the back like a painting and never feels the
//! wind. Here the main cape mesh is replaced by a cloth simulated every frame:
//!
//! - the mesh is the cape's own (its vertices, its mirrored UVs, so the texture lands exactly as
//!   authored), subdivided twice so it can bend; its *rest* shape is the skinned one, so the cape
//!   still follows the shoulders and the spine the way the animation drives them;
//! - each node is a verlet particle with gravity, an air term from the wind and the cloak's own
//!   motion (`a = k (n . (wind - v)) n`), a spring back to its skinned position that is stiff at the
//!   shoulders and loose at the hem, a limit on how far it may leave that position, edge
//!   constraints, a **backstop** (a node never ends up inside the body: it may not move behind the
//!   skinned surface) and the ground;
//! - the wind is the engine's own field (`benilla_world::wind::WindField`, the one the grass and the
//!   trees sway to), so the cloak blows with them.
//!
//! The pieces: [`Topology`] (the subdivided mesh, built once per cape), [`Cloth`] (the particles and
//! [`Cloth::step`], pure and unit-tested), and the systems that skin the targets from the rig palette
//! and write the mesh. A cloak on an actor says what it wears: its geoset style, its cape texture
//! (`Item/ObjectComponents/Cape/<ModelTextureLeft>.blp`) and the cloth are built by `actor::build`.
//! The player's comes from the item in the back slot of its equipment ([`crate::equip`], the display
//! lands in `Dress::cloak`); `Cloak(<ItemDisplayInfo id>)` on any actor is the direct hook, and
//! `WOWSOULS_CLOAK=<ItemDisplayInfo id>` puts one on the player over what the equipment says.
//! `WOWSOULS_CLOTH=0` keeps the rigid 1.12 cape.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::math::{DMat4, DVec3};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use benilla_assets::coords::wow_to_bevy;
use benilla_formats::RenderSubmesh;
use benilla_world::rig_palette::{RigPalettes, RigSkin};
use benilla_world::wind::{FoliageWind, FoliageWindStrength, WindField};

use crate::avatar::Avatar;

/// A cape mesh with fewer vertices than this is a clasp or a collar, not the cloak: it stays rigid.
pub const MIN_CLOTH_VERTS: usize = 20;
/// Midpoint subdivisions of the authored cape (39 vertices become ~150, then ~590).
pub const LEVELS: usize = 2;
/// The body's axis in cape model space (WoW axes, +x forward), for orienting the outward normals.
const BODY_AXIS_X: f32 = -0.02;

/// What an actor wears on its back: an `ItemDisplayInfo` id.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cloak(pub u32);

/// Whether the cape is cloth (`WOWSOULS_CLOTH`, default on) or the rigid 1.12 geoset.
pub fn cloth_enabled() -> bool {
    !std::env::var("WOWSOULS_CLOTH").is_ok_and(|v| v.trim() == "0")
}

// ---------------------------------------------------------------------------------------------
// The mesh
// ---------------------------------------------------------------------------------------------

/// The subdivided cape: shared by every instance of one cape mesh.
#[derive(Debug)]
pub struct Topology {
    /// The authored vertices (they come first among the nodes).
    pub n_orig: usize,
    /// Per node, the two nodes it is the midpoint of (an authored vertex: itself twice).
    pub parents: Vec<(u32, u32)>,
    pub uvs: Vec<[f32; 2]>,
    /// Rest positions and outward normals, Bevy model space.
    pub rest: Vec<Vec3>,
    pub rest_normal: Vec<Vec3>,
    /// 0 at the shoulders, 1 at the hem.
    pub hang: Vec<f32>,
    pub indices: Vec<u32>,
    pub edges: Vec<[u32; 2]>,
}

impl Topology {
    /// Subdivide `positions` / `uvs` / `indices` (WoW model space) `levels` times.
    pub fn build(positions: &[[f32; 3]], uvs: &[[f32; 2]], indices: &[u32], levels: usize) -> Self {
        let n_orig = positions.len();
        let mut pos: Vec<Vec3> = positions.iter().map(|p| Vec3::from(*p)).collect();
        let mut uv: Vec<[f32; 2]> = uvs.to_vec();
        let mut parents: Vec<(u32, u32)> = (0..n_orig as u32).map(|i| (i, i)).collect();
        let mut tris: Vec<u32> = indices.to_vec();
        for _ in 0..levels {
            let mut mids: HashMap<(u32, u32), u32> = HashMap::new();
            let mut next: Vec<u32> = Vec::with_capacity(tris.len() * 4);
            let mut mid = |a: u32,
                           b: u32,
                           pos: &mut Vec<Vec3>,
                           uv: &mut Vec<[f32; 2]>,
                           parents: &mut Vec<(u32, u32)>| {
                let key = (a.min(b), a.max(b));
                *mids.entry(key).or_insert_with(|| {
                    let id = pos.len() as u32;
                    pos.push((pos[a as usize] + pos[b as usize]) * 0.5);
                    let (ua, ub) = (uv[a as usize], uv[b as usize]);
                    uv.push([(ua[0] + ub[0]) * 0.5, (ua[1] + ub[1]) * 0.5]);
                    parents.push(key);
                    id
                })
            };
            for t in tris.as_chunks::<3>().0 {
                let (a, b, c) = (t[0], t[1], t[2]);
                let ab = mid(a, b, &mut pos, &mut uv, &mut parents);
                let bc = mid(b, c, &mut pos, &mut uv, &mut parents);
                let ca = mid(c, a, &mut pos, &mut uv, &mut parents);
                next.extend_from_slice(&[a, ab, ca, b, bc, ab, c, ca, bc, ab, bc, ca]);
            }
            tris = next;
        }
        // Outward normals (WoW axes): the area-weighted face normals, turned away from the body.
        let mut normal = vec![Vec3::ZERO; pos.len()];
        for t in tris.as_chunks::<3>().0 {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let n = (pos[b] - pos[a]).cross(pos[c] - pos[a]);
            normal[a] += n;
            normal[b] += n;
            normal[c] += n;
        }
        let (zmin, zmax) = pos.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p.z), hi.max(p.z))
        });
        let span = (zmax - zmin).max(1.0e-4);
        let mut hang = Vec::with_capacity(pos.len());
        for (p, n) in pos.iter().zip(normal.iter_mut()) {
            let axis = Vec3::new(BODY_AXIS_X, 0.0, p.z.clamp(0.75, 1.7));
            let away = *p - axis;
            let mut dir = n.normalize_or_zero();
            if dir == Vec3::ZERO || dir.dot(away) < 0.0 {
                dir = if dir == Vec3::ZERO {
                    away.normalize_or_zero()
                } else {
                    -dir
                };
            }
            *n = dir;
            hang.push(((zmax - p.z) / span).clamp(0.0, 1.0));
        }
        let mut edge_set: HashMap<(u32, u32), ()> = HashMap::new();
        for t in tris.as_chunks::<3>().0 {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                edge_set.insert((a.min(b), a.max(b)), ());
            }
        }
        let mut edges: Vec<[u32; 2]> = edge_set.into_keys().map(|(a, b)| [a, b]).collect();
        edges.sort_unstable();
        Self {
            n_orig,
            parents,
            uvs: uv,
            rest: pos.iter().map(|p| wow_to_bevy([p.x, p.y, p.z])).collect(),
            rest_normal: normal
                .iter()
                .map(|n| wow_to_bevy([n.x, n.y, n.z]))
                .collect(),
            hang,
            indices: tris,
            edges,
        }
    }

    pub fn nodes(&self) -> usize {
        self.rest.len()
    }

    /// The render mesh at the rest pose; `colors` are the authored vertices' tints, if any.
    pub fn mesh(&self, colors: &[[f32; 4]]) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            self.rest.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            self.rest_normal
                .iter()
                .map(|n| n.to_array())
                .collect::<Vec<_>>(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs.clone());
        if colors.len() == self.n_orig {
            // A subdivided node takes the tint of its first parent (the authored one is constant).
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_COLOR,
                self.parents
                    .iter()
                    .map(|(a, _)| colors[*a as usize])
                    .collect::<Vec<_>>(),
            );
        }
        mesh.insert_indices(Indices::U32(self.indices.clone()));
        mesh
    }
}

// ---------------------------------------------------------------------------------------------
// The cloth
// ---------------------------------------------------------------------------------------------

/// The simulation's constants (yards, seconds).
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub gravity: f64,
    /// Velocity lost per second.
    pub damping: f64,
    /// The air term's gain: `a = air * (n . (wind - v)) * n`.
    pub air: f64,
    /// The spring back to the skinned position, per second, at the shoulders and at the hem.
    pub follow_top: f64,
    pub follow_hem: f64,
    /// How far a node may leave its skinned position, at the hem (yd); 0 at the shoulders.
    pub slack: f64,
    /// A node closer to the shoulders than this share of the hang is pinned to its skinned position.
    pub pin: f32,
    pub iterations: usize,
}

impl Params {
    /// The defaults with `WOWSOULS_CLOAK_WIND=<gain>` (0 to 4, default 1) on the air term.
    pub fn from_env() -> Self {
        let gain = std::env::var("WOWSOULS_CLOAK_WIND")
            .ok()
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .map_or(1.0, |v| v.clamp(0.0, 4.0));
        let base = Self::default();
        Self {
            air: base.air * gain,
            ..base
        }
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            gravity: 9.0,
            damping: 1.4,
            air: 0.8,
            follow_top: 90.0,
            follow_hem: 5.0,
            slack: 0.55,
            pin: 0.06,
            iterations: 4,
        }
    }
}

/// The particles of one cape.
#[derive(Debug)]
pub struct Cloth {
    topo: Arc<Topology>,
    x: Vec<DVec3>,
    xp: Vec<DVec3>,
    rest_len: Vec<f64>,
    live: bool,
    /// Seconds simulated, for the flutter.
    t: f64,
}

impl Cloth {
    pub fn new(topo: Arc<Topology>) -> Self {
        let n = topo.nodes();
        Self {
            x: vec![DVec3::ZERO; n],
            xp: vec![DVec3::ZERO; n],
            rest_len: vec![0.0; topo.edges.len()],
            live: false,
            t: 0.0,
            topo,
        }
    }

    pub fn positions(&self) -> &[DVec3] {
        &self.x
    }

    /// Lay the cape on its skinned shape (a spawn, a teleport): rest lengths are measured from it.
    pub fn reset(&mut self, targets: &[DVec3]) {
        self.x.clone_from_slice(targets);
        self.xp.clone_from_slice(targets);
        for (len, e) in self.rest_len.iter_mut().zip(&self.topo.edges) {
            *len = targets[e[0] as usize].distance(targets[e[1] as usize]);
        }
        self.live = true;
    }

    /// Advance `dt` seconds. `targets` / `outward` are the skinned positions and outward normals
    /// of every node this frame, `wind` the air's velocity, `ground` the floor's height.
    pub fn step(
        &mut self,
        targets: &[DVec3],
        outward: &[DVec3],
        wind: DVec3,
        ground: Option<f64>,
        dt: f64,
        p: &Params,
    ) {
        if !self.live {
            self.reset(targets);
            return;
        }
        // A cape that lost its body (a teleport, a respawn) is laid back on it.
        if self
            .x
            .iter()
            .zip(targets)
            .any(|(x, t)| x.distance(*t) > 4.0 || !x.is_finite())
        {
            self.reset(targets);
            return;
        }
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 1.0 / 15.0)
        } else {
            0.0
        };
        if dt <= 0.0 {
            return;
        }
        let sub = ((dt * 120.0).ceil() as usize).clamp(1, 4);
        let h = dt / sub as f64;
        for _ in 0..sub {
            self.t += h;
            let normals = self.normals(outward);
            for i in 0..self.x.len() {
                let hang = f64::from(self.topo.hang[i]);
                if self.topo.hang[i] <= p.pin {
                    self.x[i] = targets[i];
                    self.xp[i] = targets[i];
                    continue;
                }
                let v = (self.x[i] - self.xp[i]) / h;
                // The wind flutters: a slow swell and a quick ripple, out of step along the cape.
                let uv = self.topo.uvs[i];
                let phase = f64::from(uv[0]) * 5.0 + f64::from(uv[1]) * 7.0;
                let gust = 1.0
                    + 0.35 * (self.t * 2.3 + phase).sin()
                    + 0.2 * (self.t * 6.1 + phase * 1.7).sin();
                let rel = wind * gust - v;
                let n = normals[i];
                let follow = p.follow_top + (p.follow_hem - p.follow_top) * smooth(hang);
                let mut a = DVec3::new(0.0, -p.gravity, 0.0);
                a += p.air * (0.4 + 0.6 * hang) * n.dot(rel) * n;
                a += follow * (targets[i] - self.x[i]);
                let next = self.x[i] + v * (1.0 - p.damping * h).max(0.0) * h + a * h * h;
                self.xp[i] = self.x[i];
                self.x[i] = next;
            }
            for _ in 0..p.iterations {
                for (e, len) in self.topo.edges.iter().zip(&self.rest_len) {
                    let (a, b) = (e[0] as usize, e[1] as usize);
                    let d = self.x[b] - self.x[a];
                    let dist = d.length();
                    if dist < 1.0e-9 {
                        continue;
                    }
                    let (wa, wb) = (self.free(a, p), self.free(b, p));
                    let total = wa + wb;
                    if total == 0.0 {
                        continue;
                    }
                    let corr = d * ((dist - len) / dist / total);
                    self.x[a] += corr * wa;
                    self.x[b] -= corr * wb;
                }
                for i in 0..self.x.len() {
                    if self.topo.hang[i] <= p.pin {
                        self.x[i] = targets[i];
                        continue;
                    }
                    // Within reach of where the skeleton puts it.
                    let reach = p.slack * smooth(f64::from(self.topo.hang[i]));
                    let off = self.x[i] - targets[i];
                    let len = off.length();
                    if len > reach {
                        self.x[i] = targets[i] + off * (reach / len);
                    }
                    // Never behind the skinned surface: that is inside the body.
                    let along = (self.x[i] - targets[i]).dot(outward[i]);
                    if along < 0.0 {
                        self.x[i] -= outward[i] * along;
                    }
                    if let Some(g) = ground {
                        self.x[i].y = self.x[i].y.max(g);
                    }
                }
            }
        }
    }

    fn free(&self, i: usize, p: &Params) -> f64 {
        if self.topo.hang[i] <= p.pin {
            0.0
        } else {
            1.0
        }
    }

    /// Smooth vertex normals of the current shape, turned outward.
    pub fn normals(&self, outward: &[DVec3]) -> Vec<DVec3> {
        let mut n = vec![DVec3::ZERO; self.x.len()];
        for t in self.topo.indices.as_chunks::<3>().0 {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let f = (self.x[b] - self.x[a]).cross(self.x[c] - self.x[a]);
            n[a] += f;
            n[b] += f;
            n[c] += f;
        }
        for (v, out) in n.iter_mut().zip(outward) {
            let mut d = v.normalize_or_zero();
            if d == DVec3::ZERO {
                d = *out;
            } else if d.dot(*out) < 0.0 {
                d = -d;
            }
            *v = d;
        }
        n
    }
}

fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

// ---------------------------------------------------------------------------------------------
// The skinning of the targets
// ---------------------------------------------------------------------------------------------

/// The skinned position and outward normal of every node: the authored vertices through the rig
/// palette (rows relative to the rig, `origin` added in f64: a body ~9 000 yd from the world's origin
/// keeps its millimetres), the subdivided ones as midpoints.
pub fn skin_targets(
    topo: &Topology,
    positions: &[[f32; 3]],
    joints: &[[u16; 4]],
    weights: &[[f32; 4]],
    rows: &[Mat4],
    origin: DVec3,
) -> (Vec<DVec3>, Vec<DVec3>) {
    let mut pos = Vec::with_capacity(topo.nodes());
    let mut nor = Vec::with_capacity(topo.nodes());
    for i in 0..topo.nodes() {
        if i < topo.n_orig {
            let rest = wow_to_bevy(positions[i]);
            let mut p = DVec3::ZERO;
            let mut n = DVec3::ZERO;
            let mut total = 0.0;
            for k in 0..4 {
                let w = weights[i][k];
                if w <= 0.0 {
                    continue;
                }
                let Some(m) = rows.get(joints[i][k] as usize) else {
                    continue;
                };
                p += m.transform_point3(rest).as_dvec3() * f64::from(w);
                n += m.transform_vector3(topo.rest_normal[i]).as_dvec3() * f64::from(w);
                total += f64::from(w);
            }
            if total <= 0.0 {
                p = rest.as_dvec3();
                n = topo.rest_normal[i].as_dvec3();
                total = 1.0;
            }
            pos.push(origin + p / total);
            nor.push((n / total).normalize_or_zero());
        } else {
            let (a, b) = topo.parents[i];
            pos.push((pos[a as usize] + pos[b as usize]) * 0.5);
            nor.push(((nor[a as usize] + nor[b as usize]) * 0.5).normalize_or_zero());
        }
    }
    (pos, nor)
}

// ---------------------------------------------------------------------------------------------
// The systems
// ---------------------------------------------------------------------------------------------

/// The cape part of an actor: a child of its model frame carrying the cloth mesh.
#[derive(Component)]
pub struct ClothPart {
    pub owner: Entity,
    mesh: Handle<Mesh>,
    /// The cape's shadow caster (a child sharing the mesh, on the shadow rig's layer), while the rig is up.
    caster: Option<Entity>,
    cloth: Cloth,
    /// The authored vertices, for skinning.
    sub: Arc<RenderSubmesh>,
}

impl ClothPart {
    pub fn new(
        owner: Entity,
        mesh: Handle<Mesh>,
        topo: Arc<Topology>,
        sub: Arc<RenderSubmesh>,
    ) -> Self {
        Self {
            owner,
            mesh,
            caster: None,
            cloth: Cloth::new(topo),
            sub,
        }
    }
}

/// The air's velocity at the cloak, from the engine's wind (yd/s, Bevy axes); none with the wind off.
pub fn wind_vector(field: &WindField, quality: u8, strength: f32) -> DVec3 {
    if quality == 0 {
        return DVec3::ZERO;
    }
    let s = field.sample;
    let speed = f64::from(s.speed) * (0.55 + 0.9 * f64::from(s.gust)) * f64::from(strength);
    DVec3::new(f64::from(s.dir.x), 0.0, f64::from(s.dir.y)) * speed
}

/// The cloth's constants, read once.
#[derive(Resource, Clone, Copy)]
struct ClothParams(Params);

impl std::ops::Deref for ClothParams {
    type Target = Params;
    fn deref(&self) -> &Params {
        &self.0
    }
}

/// How far above the feet the floor of the cloak is (yd).
const GROUND_CLEARANCE: f64 = 0.04;

#[allow(clippy::too_many_arguments)]
fn simulate(
    time: Res<Time>,
    params: Res<ClothParams>,
    palettes: Res<RigPalettes>,
    rigs: Query<&RigSkin>,
    roots: Query<&GlobalTransform, Without<ClothPart>>,
    mut parts: Query<(&mut ClothPart, &GlobalTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    field: Res<WindField>,
    quality: Res<FoliageWind>,
    strength: Res<FoliageWindStrength>,
) {
    let params = *params;
    let wind = wind_vector(&field, quality.0, strength.0);
    let dt = f64::from(time.delta_secs());
    for (mut part, global) in &mut parts {
        let Ok(rig) = rigs.get(part.owner) else {
            continue;
        };
        let (Some(rows), Some(origin)) = (
            palettes.rig_rows(rig.slot, rig.bones() as usize),
            palettes.slot_origin(rig.slot),
        ) else {
            continue;
        };
        let ground = roots
            .get(part.owner)
            .ok()
            .map(|g| f64::from(g.translation().y) + GROUND_CLEARANCE);
        let ClothPart {
            cloth, sub, mesh, ..
        } = &mut *part;
        let (targets, outward) = skin_targets(
            &cloth.topo,
            &sub.positions,
            &sub.joints,
            &sub.weights,
            &rows,
            origin.as_dvec3(),
        );
        cloth.step(&targets, &outward, wind, ground, dt, &params);
        // World to the part's own space, in f64: the mesh is drawn through the part's transform.
        let inverse =
            DMat4::from_cols_array(&global.to_matrix().to_cols_array().map(f64::from)).inverse();
        let normals = cloth.normals(&outward);
        let positions: Vec<[f32; 3]> = cloth
            .positions()
            .iter()
            .map(|x| inverse.transform_point3(*x).as_vec3().to_array())
            .collect();
        let normals: Vec<[f32; 3]> = normals
            .iter()
            .map(|n| {
                inverse
                    .transform_vector3(*n)
                    .normalize_or_zero()
                    .as_vec3()
                    .to_array()
            })
            .collect();
        if let Some(mesh) = meshes.get_mut(&*mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        }
    }
}

/// The cape casts its shadow: while the sun's shadow rig is up, a child of the cape part shares its
/// mesh on the shadow layer with the rig's invisible caster material (the rigid parts reach the map
/// through `PickMesh`, which a mesh that moves every frame cannot use).
fn cast_shadow(
    mut commands: Commands,
    frame: Res<crate::gfx::shadow_core::ShadowFrame>,
    mut parts: Query<(Entity, &mut ClothPart)>,
) {
    for (entity, mut part) in &mut parts {
        match (frame.active, part.caster) {
            (true, None) => {
                let Some(material) = frame.material.clone() else {
                    continue;
                };
                let caster = crate::gfx::shadow_core::spawn_solid_caster(
                    &mut commands,
                    part.mesh.clone(),
                    material,
                );
                commands.entity(caster).insert(ChildOf(entity));
                part.caster = Some(caster);
            }
            (false, Some(caster)) => {
                commands.entity(caster).despawn();
                part.caster = None;
            }
            _ => {}
        }
    }
}

/// `WOWSOULS_CLOAK=<ItemDisplayInfo id>`: the player wears it.
#[derive(Resource)]
struct PlayerCloak(u32);

fn dress_player(
    wanted: Res<PlayerCloak>,
    mut commands: Commands,
    avatar: Query<Entity, (With<Avatar>, Without<Cloak>)>,
) {
    for entity in &avatar {
        commands.entity(entity).insert(Cloak(wanted.0));
        info!("cloak: the player wears ItemDisplayInfo {}", wanted.0);
    }
}

pub fn plugin(app: &mut App) {
    if let Some(id) = std::env::var("WOWSOULS_CLOAK")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|id| *id != 0)
    {
        app.insert_resource(PlayerCloak(id))
            .add_systems(Update, dress_player);
    }
    app.insert_resource(ClothParams(Params::from_env()))
        .add_systems(Update, cast_shadow)
        .add_systems(
            PostUpdate,
            simulate
                .after(bevy::transform::TransformSystems::Propagate)
                .after(benilla_world::billboard::BillboardPlace),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat cape: `cols` x `rows` vertices, 0.5 yd wide and 1 yd long, hanging behind a body
    /// whose axis is x = 0, in WoW axes (x forward, y left, z up), the top at z = 1.8.
    fn grid(cols: usize, rows: usize) -> (Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>) {
        let mut pos = Vec::new();
        let mut uv = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let (u, v) = (c as f32 / (cols - 1) as f32, r as f32 / (rows - 1) as f32);
                pos.push([-0.1, (u - 0.5) * 0.5, 1.8 - v]);
                uv.push([u, v]);
            }
        }
        let mut idx = Vec::new();
        for r in 0..rows - 1 {
            for c in 0..cols - 1 {
                let a = (r * cols + c) as u32;
                let (b, d) = (a + 1, a + cols as u32);
                idx.extend_from_slice(&[a, b, d, b, d + 1, d]);
            }
        }
        (pos, uv, idx)
    }

    fn topo(levels: usize) -> Arc<Topology> {
        let (p, u, i) = grid(4, 6);
        Arc::new(Topology::build(&p, &u, &i, levels))
    }

    /// The rest shape as the skinned targets: the world is the model space.
    fn rest_targets(t: &Topology) -> (Vec<DVec3>, Vec<DVec3>) {
        (
            t.rest.iter().map(|p| p.as_dvec3()).collect(),
            t.rest_normal.iter().map(|n| n.as_dvec3()).collect(),
        )
    }

    #[test]
    fn subdivision_quadruples_the_triangles_and_keeps_the_originals_first() {
        let (p, u, i) = grid(4, 6);
        let t0 = Topology::build(&p, &u, &i, 0);
        let t2 = Topology::build(&p, &u, &i, 2);
        assert_eq!(t0.nodes(), p.len());
        assert_eq!(
            t2.indices.len(),
            i.len() * 16,
            "two levels: 4 x 4 triangles each"
        );
        assert_eq!(t2.n_orig, p.len());
        for (k, uv) in u.iter().enumerate() {
            assert_eq!(t2.parents[k], (k as u32, k as u32));
            assert_eq!(t2.uvs[k], *uv);
        }
        for (k, (a, b)) in t2.parents.iter().enumerate().skip(p.len()) {
            assert!(
                (*a as usize) < k && (*b as usize) < k,
                "a midpoint comes after its parents"
            );
        }
        let used: std::collections::HashSet<u32> = t2.indices.iter().copied().collect();
        assert_eq!(used.len(), t2.nodes(), "every node is in a triangle");
    }

    #[test]
    fn the_hang_runs_from_the_shoulders_to_the_hem() {
        let t = topo(1);
        let top = t.hang.iter().cloned().fold(f32::MAX, f32::min);
        let hem = t.hang.iter().cloned().fold(f32::MIN, f32::max);
        assert_eq!((top, hem), (0.0, 1.0));
        for (e, w) in t.edges.iter().zip(t.edges.iter().skip(1)) {
            assert!(e < w, "edges are unique and sorted");
        }
    }

    #[test]
    fn the_normals_point_away_from_the_body() {
        let t = topo(1);
        // Behind the body (x < 0 in WoW axes) is +z in Bevy axes (z = -x).
        for n in &t.rest_normal {
            assert!(n.z > 0.9, "outward is backward: {n:?}");
        }
    }

    #[test]
    fn a_hanging_cape_in_still_air_stays_on_its_skinned_shape() {
        let t = topo(2);
        let (targets, outward) = rest_targets(&t);
        let mut cloth = Cloth::new(t.clone());
        let params = Params::default();
        for _ in 0..240 {
            cloth.step(&targets, &outward, DVec3::ZERO, None, 1.0 / 60.0, &params);
        }
        let worst = cloth
            .positions()
            .iter()
            .zip(&targets)
            .map(|(x, t)| x.distance(*t))
            .fold(0.0, f64::max);
        assert!(
            worst < 0.12,
            "still air: the hem sags {worst:.3} yd at most"
        );
        for (i, x) in cloth.positions().iter().enumerate() {
            if t.hang[i] <= params.pin {
                assert_eq!(*x, targets[i], "the shoulders do not move");
            }
        }
    }

    #[test]
    fn a_wind_from_behind_billows_the_hem_outward_but_never_into_the_body() {
        let t = topo(2);
        let (targets, outward) = rest_targets(&t);
        let mut cloth = Cloth::new(t.clone());
        let params = Params::default();
        // The cape's outward is +z (Bevy): a wind blowing along +z pushes it away from the body.
        let wind = DVec3::new(0.0, 0.0, 9.0);
        for _ in 0..300 {
            cloth.step(&targets, &outward, wind, None, 1.0 / 60.0, &params);
        }
        let hem: Vec<usize> = (0..t.nodes()).filter(|i| t.hang[*i] > 0.9).collect();
        let pushed: f64 = hem
            .iter()
            .map(|i| (cloth.positions()[*i] - targets[*i]).dot(outward[*i]))
            .sum::<f64>()
            / hem.len() as f64;
        assert!(
            pushed > 0.15,
            "the hem billows outward by {pushed:.3} yd on average"
        );
    }

    #[test]
    fn a_wind_toward_the_body_cannot_push_the_cape_into_it() {
        let t = topo(2);
        let (targets, outward) = rest_targets(&t);
        let mut cloth = Cloth::new(t.clone());
        let params = Params::default();
        let against = DVec3::new(0.0, 0.0, -14.0);
        for _ in 0..300 {
            cloth.step(&targets, &outward, against, None, 1.0 / 60.0, &params);
            for ((x, tg), n) in cloth.positions().iter().zip(&targets).zip(&outward) {
                assert!((*x - *tg).dot(*n) >= -1.0e-9, "inside the body");
            }
        }
    }

    #[test]
    fn the_edges_hold_their_lengths_under_a_hard_gust() {
        let t = topo(2);
        let (targets, outward) = rest_targets(&t);
        let mut cloth = Cloth::new(t.clone());
        let params = Params::default();
        let gust = DVec3::new(6.0, 0.0, 12.0);
        for _ in 0..200 {
            cloth.step(&targets, &outward, gust, None, 1.0 / 60.0, &params);
        }
        let worst = t
            .edges
            .iter()
            .zip(&cloth.rest_len)
            .map(|(e, len)| {
                let d = cloth.positions()[e[0] as usize].distance(cloth.positions()[e[1] as usize]);
                (d - len).abs() / len.max(1.0e-6)
            })
            .fold(0.0, f64::max);
        assert!(worst < 0.35, "an edge stretched by {worst:.2}");
    }

    #[test]
    fn the_cape_follows_a_moving_body_and_lags_behind_it() {
        let t = topo(2);
        let (mut targets, outward) = rest_targets(&t);
        let rest = targets.clone();
        let mut cloth = Cloth::new(t.clone());
        let params = Params::default();
        cloth.step(&targets, &outward, DVec3::ZERO, None, 1.0 / 60.0, &params);
        // The body runs along +x at 7 yd/s.
        let mut lag = 0.0f64;
        for k in 1..=60 {
            for (p, r) in targets.iter_mut().zip(&rest) {
                p.x = r.x + 7.0 * f64::from(k) / 60.0;
            }
            cloth.step(&targets, &outward, DVec3::ZERO, None, 1.0 / 60.0, &params);
            for i in (0..t.nodes()).filter(|i| t.hang[*i] > 0.9) {
                lag = lag.max(targets[i].x - cloth.positions()[i].x);
            }
        }
        assert!(lag > 0.02, "the hem trails the body by {lag:.3}");
        assert!(
            lag <= params.slack + 1.0e-9,
            "but never farther than its slack"
        );
    }

    #[test]
    fn a_teleport_lays_the_cape_back_on_the_body() {
        let t = topo(1);
        let (mut targets, outward) = rest_targets(&t);
        let mut cloth = Cloth::new(t.clone());
        let params = Params::default();
        cloth.step(&targets, &outward, DVec3::ZERO, None, 1.0 / 60.0, &params);
        for p in &mut targets {
            p.x += 500.0;
        }
        cloth.step(&targets, &outward, DVec3::ZERO, None, 1.0 / 60.0, &params);
        let worst = cloth
            .positions()
            .iter()
            .zip(&targets)
            .map(|(x, t)| x.distance(*t))
            .fold(0.0, f64::max);
        assert!(worst < 1.0e-9, "{worst}");
    }

    #[test]
    fn the_ground_holds_the_hem() {
        let t = topo(1);
        let (targets, outward) = rest_targets(&t);
        let mut cloth = Cloth::new(t.clone());
        let params = Params::default();
        let floor = 1.5; // above the hem's rest height (0.8)
        for _ in 0..30 {
            cloth.step(
                &targets,
                &outward,
                DVec3::ZERO,
                Some(floor),
                1.0 / 60.0,
                &params,
            );
        }
        for (i, x) in cloth.positions().iter().enumerate() {
            if t.hang[i] > params.pin {
                assert!(x.y >= floor - 1.0e-9, "node {i} fell through at {}", x.y);
            }
        }
    }

    #[test]
    fn a_huge_or_zero_step_does_not_break_the_cloth() {
        let t = topo(1);
        let (targets, outward) = rest_targets(&t);
        let mut cloth = Cloth::new(t.clone());
        let params = Params::default();
        let wind = DVec3::new(5.0, 0.0, 5.0);
        cloth.step(&targets, &outward, wind, None, 0.0, &params);
        cloth.step(&targets, &outward, wind, None, 10.0, &params);
        cloth.step(&targets, &outward, wind, None, f64::NAN, &params);
        assert!(cloth.positions().iter().all(|x| x.is_finite()));
    }

    #[test]
    fn skinning_through_a_translated_palette_moves_every_node() {
        let (p, u, i) = grid(4, 6);
        let t = Topology::build(&p, &u, &i, 1);
        let joints = vec![[0u16, 0, 0, 0]; p.len()];
        let weights = vec![[1.0f32, 0.0, 0.0, 0.0]; p.len()];
        let rows = [Mat4::from_translation(Vec3::new(0.0, 5.0, 0.0))];
        let origin = DVec3::new(100.0, 0.0, 9000.0);
        let (pos, nor) = skin_targets(&t, &p, &joints, &weights, &rows, origin);
        assert_eq!(pos.len(), t.nodes());
        for (k, x) in pos.iter().enumerate() {
            let rest = t.rest[k].as_dvec3();
            assert!(
                (*x - (rest + origin + DVec3::new(0.0, 5.0, 0.0))).length() < 1.0e-5,
                "node {k}"
            );
        }
        assert!(nor.iter().all(|n| (n.length() - 1.0).abs() < 1.0e-5));
    }

    #[test]
    fn the_wind_vector_follows_the_engine_wind_and_its_switch() {
        let mut field = WindField::default();
        field.sample.dir = Vec2::new(1.0, 0.0);
        field.sample.speed = 10.0;
        field.sample.gust = 0.0;
        let v = wind_vector(&field, 2, 1.0);
        assert!(v.x > 5.0 && v.y == 0.0 && v.z == 0.0, "{v:?}");
        assert_eq!(
            wind_vector(&field, 0, 1.0),
            DVec3::ZERO,
            "wind off, no wind"
        );
        assert!(
            wind_vector(&field, 1, 2.0).x > v.x,
            "the player's strength scales it"
        );
    }
}
