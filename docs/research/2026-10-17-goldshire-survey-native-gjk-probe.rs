//! Read-only source probe for the Edwin Brook native-road motor stall.
use parry3d::{
    math::{Pose3, Vec3},
    query::{cast_shapes, contact, PointQuery, ShapeCastOptions},
    shape::{Capsule, FeatureId, SupportMap, Triangle},
};
fn main() -> anyhow::Result<()> {
    let data = benilla_formats::wow_data().expect("WOW_DATA");
    let mut chain = benilla_formats::open_chain(&data)?;
    let harlan = std::env::args().nth(1).as_deref() == Some("--harlan");
    let wow = if harlan {
        [-9446.782, 61.558693, 56.069046]
    } else {
        [-9482.298, 145.27151, 56.550716]
    };
    let (tx, ty) = benilla_formats::world_to_tile(wow[0], wow[1]);
    let tile = benilla_formats::load_tile_mesh(&mut chain, "Azeroth", tx, ty)?;
    let feet = Vec3::new(-wow[1], wow[2], -wow[0]);
    let center = feet + Vec3::Y * (2.0277777 / 2.0);
    let target = if harlan {
        Vec3::new(-63.542, 56.132, 9446.875)
    } else {
        Vec3::new(-144.792, 56.554, 9482.292)
    };
    let horizontal = Vec3::new(target.x - feet.x, 0.0, target.z - feet.z).normalize();
    let cap = Capsule::new_y((2.0277777 - 2.0 / 3.0) / 2.0, 1.0 / 3.0);
    let mut rows = Vec::new();
    for (ci, c) in tile.chunks.iter().enumerate() {
        for (ti, indices) in c.indices.as_chunks::<3>().0.iter().enumerate() {
            let p = indices.map(|i| {
                let v = c.positions[i as usize];
                Vec3::new(-v[1], v[2], -v[0])
            });
            let min = p[0].min(p[1]).min(p[2]);
            let max = p[0].max(p[1]).max(p[2]);
            if min.x > feet.x + 1.0
                || max.x < feet.x - 1.0
                || min.z > feet.z + 1.0
                || max.z < feet.z - 1.0
                || min.y > feet.y + 1.0
                || max.y < feet.y - 1.0
            {
                continue;
            }
            let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize();
            if n.dot(horizontal) > -1e-5 {
                continue;
            }
            let origin = p[0];
            let tri = Triangle::new(Vec3::ZERO, p[1] - origin, p[2] - origin);
            let pose = Pose3::from_translation(center - origin);
            let c = contact(&Pose3::IDENTITY, &tri, &pose, &cap, 0.04)?;
            for (label, dir, len) in [("forward", horizontal, 0.62), ("ground", Vec3::NEG_Y, 0.62)]
            {
                let h = cast_shapes(
                    &Pose3::IDENTITY,
                    Vec3::ZERO,
                    &tri,
                    &pose,
                    dir,
                    &cap,
                    ShapeCastOptions {
                        max_time_of_impact: len,
                        target_distance: 0.0,
                        stop_at_penetration: false,
                        compute_impact_geometry_on_penetration: true,
                    },
                )?;
                if let Some(h) = h {
                    let leading = cap.support_point(&pose, dir).dot(dir);
                    let ahead = [tri.a, tri.b, tri.c]
                        .into_iter()
                        .map(|v| v.dot(dir) - leading)
                        .fold(f32::MIN, f32::max);
                    let behind = ahead < -1.0 / 36.0;
                    let feature = tri.project_local_point_and_get_feature(h.witness1).1;
                    let fixed = if matches!(feature, FeatureId::Face(_)) && h.normal1.dot(n) > 0.0 {
                        n
                    } else {
                        h.normal1
                    };
                    let tangent = Vec3::new(
                        horizontal.x,
                        -(horizontal.x * fixed.x + horizontal.z * fixed.z) / fixed.y,
                        horizontal.z,
                    )
                    .normalize();
                    rows.push((h.time_of_impact,format!("chunk{ci} tri{ti} {label} toi={} behind{behind} feature{feature:?} normal{:?} authored{n:?} correctedRideDot={} vertices{p:?} contact{c:?}",h.time_of_impact,h.normal1,n.dot(tangent))));
                }
            }
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, row) in rows {
        println!("{row}");
    }
    Ok(())
}
