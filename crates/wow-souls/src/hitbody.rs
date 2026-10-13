//! Hit capsules that ride the bones: the body a blade is tested against, fitted once per model
//! to its own skinned mesh and posed every frame by the same skinning matrices the GPU draws it
//! with, so a hunched, lunging, flinching or falling creature is struck where it is drawn.
//!
//! Each skinning bone gets one capsule, fitted (in the mesh's bind space) to the vertices it
//! carries most: its axis along their longest spread, its radius the distance that covers
//! [`COVER`] of them. A vertical capsule at the feet ([`crate::hits::Hurtbox`]) stays the
//! fallback for a model without skin data.

use bevy::prelude::*;

/// The share of a bone's vertices its capsule must contain.
pub const COVER: f32 = 0.85;
/// Bones carrying fewer vertices than this get no capsule (an eyelid, a candle wick).
pub const MIN_POINTS: usize = 3;
/// The smallest capsule radius (yd, bind space).
pub const MIN_RADIUS: f32 = 0.03;

/// One capsule riding a bone, in the model's bind space (Bevy axes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitPart {
    pub bone: u16,
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

impl HitPart {
    /// The capsule posed by `palette` (world-from-bind skinning matrices): its axis ends and
    /// radius in the world, the radius scaled with the bone's scale.
    pub fn posed(&self, palette: &[Mat4]) -> Option<(Vec3, Vec3, f32)> {
        let m = palette.get(self.bone as usize)?;
        let scale = m.x_axis.truncate().length();
        Some((
            m.transform_point3(self.a),
            m.transform_point3(self.b),
            self.radius * scale,
        ))
    }
}

/// A body's hit capsules.
#[derive(Component, Debug, Clone, Default)]
pub struct BodyHits {
    pub parts: Vec<HitPart>,
}

impl BodyHits {
    /// Every capsule posed by `palette`: `(axis end, axis end, radius)` in the world.
    pub fn posed(&self, palette: &[Mat4]) -> Vec<(Vec3, Vec3, f32)> {
        self.parts.iter().filter_map(|p| p.posed(palette)).collect()
    }
}

/// The posed capsules of a body this frame, from its rig's palette; empty without them.
pub fn posed_body(
    hits: Option<&BodyHits>,
    skin: Option<&benilla_world::rig_palette::RigSkin>,
    palettes: &benilla_world::rig_palette::RigPalettes,
) -> Vec<(Vec3, Vec3, f32)> {
    let (Some(hits), Some(skin)) = (hits, skin) else {
        return Vec::new();
    };
    palettes
        .world_palette(skin.slot, skin.bones() as usize)
        .map_or_else(Vec::new, |palette| hits.posed(&palette))
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, build);
}

/// Fits the capsules of each struck-at creature once its model is on.
#[allow(clippy::type_complexity)]
fn build(
    mut commands: Commands,
    pending: Query<
        (Entity, &crate::actor::ActorModel),
        (
            With<crate::hits::Hurtbox>,
            With<crate::actor::ActorBuilt>,
            Without<BodyHits>,
            Without<crate::avatar::Avatar>,
        ),
    >,
    m2s: Res<Assets<benilla_assets::M2Model>>,
) {
    for (e, actor) in &pending {
        // A character body hides geosets this fit would count; only creatures are struck at.
        if !matches!(actor.look, crate::actor::Look::Creature { .. }) {
            commands.entity(e).insert(BodyHits::default());
            continue;
        }
        let Some(model) = m2s.get(&actor.m2) else {
            continue;
        };
        let (mut positions, mut joints, mut weights) = (Vec::new(), Vec::new(), Vec::new());
        for sub in model.submeshes.iter().filter(|s| s.billboard.is_none()) {
            let g = &sub.geometry;
            if g.joints.len() != g.positions.len() || g.weights.len() != g.positions.len() {
                continue;
            }
            positions.extend(
                g.positions
                    .iter()
                    .map(|p| benilla_assets::coords::wow_to_bevy(*p)),
            );
            joints.extend_from_slice(&g.joints);
            weights.extend_from_slice(&g.weights);
        }
        let parts = body_parts(&positions, &joints, &weights);
        info!(
            "hitbody: {e} {} capsules from {} skinned vertices",
            parts.len(),
            positions.len()
        );
        commands.entity(e).insert(BodyHits { parts });
    }
}

/// A capsule around `points`: axis ends and radius; `None` for no points.
pub fn fit_capsule(points: &[Vec3]) -> Option<(Vec3, Vec3, f32)> {
    if points.is_empty() {
        return None;
    }
    let centre = points.iter().copied().sum::<Vec3>() / points.len() as f32;
    let farthest = |from: Vec3| {
        points
            .iter()
            .copied()
            .max_by(|p, q| {
                p.distance_squared(from)
                    .total_cmp(&q.distance_squared(from))
            })
            .unwrap_or(from)
    };
    // The longest spread: the points' principal axis, by power iteration on their covariance,
    // seeded from the point farthest out to the point farthest from that one (two rim corners of
    // a tube, alone, tilt the axis across it).
    let p = farthest(centre);
    let mut dir = (farthest(p) - p).normalize_or_zero();
    let cov = points.iter().fold(Mat3::ZERO, |m, q| {
        let d = *q - centre;
        m + Mat3::from_cols(d * d.x, d * d.y, d * d.z)
    });
    for _ in 0..16 {
        let next = (cov * dir).normalize_or_zero();
        if next == Vec3::ZERO {
            break;
        }
        dir = next;
    }
    // The axis through the centre along it, spanning the points' extent.
    let (lo, hi) = points.iter().fold((0.0f32, 0.0f32), |(lo, hi), q| {
        let t = (*q - centre).dot(dir);
        (lo.min(t), hi.max(t))
    });
    let (a0, b0) = (centre + dir * lo, centre + dir * hi);
    // Drawn in by the radius so the caps end where the points do, then the radius that covers
    // [`COVER`] of them from that axis.
    let r0 = cover_radius(points, a0, b0);
    let pull = r0.min((hi - lo) * 0.5);
    let (a, b) = (a0 + dir * pull, b0 - dir * pull);
    Some((a, b, cover_radius(points, a, b)))
}

/// The distance from segment `a`-`b` within which [`COVER`] of `points` lie.
fn cover_radius(points: &[Vec3], a: Vec3, b: Vec3) -> f32 {
    let mut d: Vec<f32> = points
        .iter()
        .map(|p| crate::hits::segment_distance(*p, *p, a, b).0)
        .collect();
    d.sort_by(f32::total_cmp);
    let i = ((COVER * d.len() as f32).ceil() as usize).clamp(1, d.len()) - 1;
    d[i].max(MIN_RADIUS)
}

/// One capsule per bone over the skinned vertices (`positions` in bind space, Bevy axes), each
/// vertex counted for the bone that weighs on it most.
pub fn body_parts(positions: &[Vec3], joints: &[[u16; 4]], weights: &[[f32; 4]]) -> Vec<HitPart> {
    let mut by_bone: std::collections::BTreeMap<u16, Vec<Vec3>> = Default::default();
    for ((p, j), w) in positions.iter().zip(joints).zip(weights) {
        let k = (0..4).fold(0, |best, k| if w[k] > w[best] { k } else { best });
        if w[k] > 0.0 {
            by_bone.entry(j[k]).or_default().push(*p);
        }
    }
    by_bone
        .into_iter()
        .filter(|(_, pts)| pts.len() >= MIN_POINTS)
        .filter_map(|(bone, pts)| {
            let (a, b, radius) = fit_capsule(&pts)?;
            Some(HitPart { bone, a, b, radius })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    /// Rings of radius `r` around the X axis from `x0` to `x1`.
    fn tube(x0: f32, x1: f32, r: f32) -> Vec<Vec3> {
        let mut out = Vec::new();
        for i in 0..=20 {
            let x = x0 + (x1 - x0) * i as f32 / 20.0;
            for k in 0..12 {
                let a = TAU * k as f32 / 12.0;
                out.push(Vec3::new(x, r * a.cos(), r * a.sin()));
            }
        }
        out
    }

    #[test]
    fn a_tube_fits_a_capsule_along_it() {
        let (a, b, r) = fit_capsule(&tube(0.0, 1.0, 0.1)).unwrap();
        assert!((r - 0.1).abs() < 0.02, "radius {r}");
        let (lo, hi) = if a.x < b.x { (a, b) } else { (b, a) };
        // The axis runs along X, its ends drawn in by the radius so the caps end at the tube's.
        assert!(
            (lo.x - 0.1).abs() < 0.03 && (hi.x - 0.9).abs() < 0.03,
            "{lo} {hi}"
        );
        assert!(
            lo.y.abs() < 0.02 && lo.z.abs() < 0.02 && hi.y.abs() < 0.02,
            "{lo} {hi}"
        );
    }

    #[test]
    fn a_ball_fits_a_short_capsule_that_holds_it() {
        let mut pts = Vec::new();
        for i in 0..10 {
            for k in 0..12 {
                let (t, p) = (TAU * k as f32 / 12.0, std::f32::consts::PI * i as f32 / 9.0);
                pts.push(Vec3::new(p.sin() * t.cos(), p.cos(), p.sin() * t.sin()) * 0.2);
            }
        }
        let (a, b, r) = fit_capsule(&pts).unwrap();
        let inside = pts
            .iter()
            .filter(|p| crate::hits::segment_distance(**p, **p, a, b).0 <= r + 1.0e-4)
            .count();
        assert!(
            inside as f32 >= COVER * pts.len() as f32,
            "{inside}/{}",
            pts.len()
        );
        assert!(r <= 0.21 && (a - b).length() <= 0.2, "{a} {b} {r}");
    }

    #[test]
    fn no_points_no_capsule() {
        assert_eq!(fit_capsule(&[]), None);
    }

    #[test]
    fn each_bone_gets_the_vertices_it_weighs_on_most() {
        // Bone 2 carries a tube along +X, bone 5 one along +Y up at x=3; a few stray vertices on
        // bone 9 are too few for a capsule.
        let mut positions = Vec::new();
        let mut joints = Vec::new();
        let mut weights = Vec::new();
        for p in tube(0.0, 1.0, 0.1) {
            positions.push(p);
            joints.push([5, 2, 0, 0]);
            weights.push([0.3, 0.7, 0.0, 0.0]);
        }
        for p in tube(0.0, 1.0, 0.1) {
            positions.push(Vec3::new(3.0 + p.y, p.x, p.z));
            joints.push([5, 0, 0, 0]);
            weights.push([1.0, 0.0, 0.0, 0.0]);
        }
        for p in [Vec3::splat(9.0), Vec3::splat(9.1)] {
            positions.push(p);
            joints.push([9, 0, 0, 0]);
            weights.push([1.0, 0.0, 0.0, 0.0]);
        }
        let mut parts = body_parts(&positions, &joints, &weights);
        parts.sort_by_key(|p| p.bone);
        assert_eq!(parts.iter().map(|p| p.bone).collect::<Vec<_>>(), [2, 5]);
        assert!(parts[0].a.x.min(parts[0].b.x) < 0.2, "{:?}", parts[0]);
        assert!((parts[1].a.x - 3.0).abs() < 0.05, "{:?}", parts[1]);
        assert!(parts[1].a.y.max(parts[1].b.y) > 0.8, "{:?}", parts[1]);
    }

    #[test]
    fn a_part_is_posed_by_its_bones_palette_row() {
        let part = HitPart {
            bone: 1,
            a: Vec3::ZERO,
            b: Vec3::X,
            radius: 0.1,
        };
        // Row 1 turns a quarter about Y, scales by 2 and lifts by 5.
        let m = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.0),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::Y * 5.0,
        );
        let (a, b, r) = part.posed(&[Mat4::IDENTITY, m]).unwrap();
        assert!((a - Vec3::Y * 5.0).length() < 1.0e-5, "{a}");
        assert!((b - Vec3::new(0.0, 5.0, -2.0)).length() < 1.0e-5, "{b}");
        assert!((r - 0.2).abs() < 1.0e-5);
        assert_eq!(part.posed(&[Mat4::IDENTITY]), None, "no row for the bone");
    }
}
