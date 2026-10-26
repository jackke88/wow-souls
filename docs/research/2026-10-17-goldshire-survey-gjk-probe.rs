//! Standalone research probe: exact native home-1 failure plane, far/world vs rebased GJK.
use parry3d::{
    math::{Pose3, Vec3},
    query::{cast_shapes, contact, PointQuery, ShapeCastOptions},
    shape::{Capsule, Triangle},
};
fn main() {
    let mut tri = [
        Vec3::new(-52.082684, 56.70405, 9502.083),
        Vec3::new(-54.166668, 57.06188, 9504.167),
        Vec3::new(-50.0, 56.845947, 9504.167),
    ];
    let mut feet = Vec3::new(-53.180923, 56.899284, 9503.236);
    let mut target = Vec3::new(-53.303, 56.935, 9503.48);
    if std::env::args().nth(1).as_deref() == Some("--edwin") {
        tri = [
            Vec3::new(-143.74934, 56.5394, 9481.249),
            Vec3::new(-145.83333, 56.484364, 9479.167),
            Vec3::new(-145.83333, 56.567905, 9483.333),
        ];
        feet = Vec3::new(-145.27151, 56.550716, 9482.298);
        target = Vec3::new(-144.792, 56.554, 9482.292);
    }
    let center = feet + Vec3::Y * (2.0277777 / 2.0);
    let cap = Capsule::new_y((2.0277777 - 2.0 / 3.0) / 2.0, 1.0 / 3.0);
    let horizontal = Vec3::new(target.x - feet.x, 0.0, target.z - feet.z).normalize() * 2.5;
    let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize();
    let tangent = Vec3::new(
        horizontal.x,
        -(horizontal.x * n.x + horizontal.z * n.z) / n.y,
        horizontal.z,
    );
    for (label, origin) in [
        ("world", Vec3::ZERO),
        ("triangle-origin", tri[0]),
        ("capsule-origin", center),
    ] {
        let t = Triangle::new(tri[0] - origin, tri[1] - origin, tri[2] - origin);
        let pose = Pose3::from_translation(center - origin);
        let contact_hit = contact(&Pose3::IDENTITY, &t, &pose, &cap, 0.04);
        println!("{label} contact {contact_hit:?}");
        for (motion, v) in [("horizontal", horizontal), ("tangent", tangent)] {
            let d = v.normalize();
            let hit = cast_shapes(
                &Pose3::IDENTITY,
                Vec3::ZERO,
                &t,
                &pose,
                d,
                &cap,
                ShapeCastOptions {
                    max_time_of_impact: if std::env::args().nth(2).as_deref() == Some("--long") {
                        0.62
                    } else {
                        v.length() / 60.0 + 0.02
                    },
                    target_distance: 0.0,
                    stop_at_penetration: false,
                    compute_impact_geometry_on_penetration: true,
                },
            );
            println!(
                "{label} {motion}: native_n={n:?} native_dot={:.9} hit={hit:?}",
                n.dot(d)
            );
            if motion == "horizontal" {
                if let Ok(Some(h)) = hit {
                    let feature = t.project_local_point_and_get_feature(h.witness1);
                    println!("  witness trianglefeature={feature:?}");
                    let r = h.normal1;
                    let reported_tangent = Vec3::new(
                        horizontal.x,
                        -(horizontal.x * r.x + horizontal.z * r.z) / r.y,
                        horizontal.z,
                    )
                    .normalize();
                    println!(
                        "  ridefromreportednormal native_dot={:.9} would_native_gate_recast={}",
                        n.dot(reported_tangent),
                        n.dot(reported_tangent) <= -1e-5
                    );
                }
            }
        }
    }
}
