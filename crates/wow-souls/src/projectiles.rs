//! Souls ranged actions: release at the clip event, finite flight, swept world/body collision.
//! Speeds, gravity and damage are explicit prototype estimates (see weapon provenance).
use crate::{
    actor::held::HeldBuilt,
    avatar::Avatar,
    combat::{Action, ActionState, Health},
    dsclip::DsMoves,
    hitbody::{posed_body, BodyHits},
    hits::{segment_distance, Hit, Hurtbox},
    lockon::{LockOn, Lockable},
};
use benilla_world::{
    collision::WorldCollision,
    rig_palette::{RigPalettes, RigSkin},
};
use bevy::prelude::*;

#[derive(Component)]
struct Projectile {
    at: Vec3,
    velocity: Vec3,
    gravity: f32,
    damage: f32,
    from: Vec3,
    age: f32,
    bullet: bool,
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (release, fly)
            .chain()
            .after(benilla_world::billboard::BillboardPlace),
    );
}

#[allow(clippy::too_many_arguments)]
fn release(
    mut commands: Commands,
    moves: Option<Res<DsMoves>>,
    lock: Res<LockOn>,
    gear: Res<crate::gear_effect::GearFx>,
    avatar: Query<(Entity, &Action, &GlobalTransform, Option<&HeldBuilt>), With<Avatar>>,
    roots: Query<&GlobalTransform>,
    targets: Query<(&GlobalTransform, &Lockable, &Health), Without<Avatar>>,
    mut fired: Local<Option<(Entity, u32)>>,
) {
    let Some(moves) = moves else { return };
    let Some(spec) = moves.projectile else { return };
    let Ok((entity, action, body, held)) = avatar.single() else {
        return;
    };
    let ActionState::Attack(swing) = action.state else {
        return;
    };
    if swing.t < spec.release || *fired == Some((entity, swing.serial)) {
        return;
    }
    *fired = Some((entity, swing.serial));
    let at = held.and_then(|h| roots.get(h.root).ok()).map_or(
        body.translation() + Vec3::Y * 1.4,
        GlobalTransform::translation,
    );
    let direction = lock
        .target
        .and_then(|e| targets.get(e).ok())
        .filter(|(_, _, h)| h.cur > 0.0)
        .map(|(t, l, _)| {
            (t.translation() + Vec3::Y * l.aim_height - at).normalize_or(*body.forward())
        })
        .unwrap_or(*body.forward());
    commands.spawn(Projectile {
        at,
        velocity: direction * spec.speed,
        gravity: spec.gravity,
        damage: spec.damage * gear.0.ranged_factor,
        from: body.translation(),
        age: 0.0,
        bullet: moves.family == "gun",
    });
    info!(
        "projectile: {} released at {:.3}s (source {:.3}s)",
        moves.family, swing.t, spec.release
    );
}

/// Contact distance along a swept segment, including targets narrower than one frame's travel.
fn contact(from: Vec3, to: Vec3, a: Vec3, b: Vec3, r: f32) -> Option<f32> {
    let (distance, on_ray, _) = segment_distance(from, to, a, b);
    (distance <= r + 0.06).then(|| on_ray.distance(from))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    world: WorldCollision,
    mut shots: Query<(Entity, &mut Projectile)>,
    mut targets: Query<
        (
            Entity,
            &GlobalTransform,
            &Hurtbox,
            &mut Health,
            Option<&BodyHits>,
            Option<&RigSkin>,
        ),
        Without<Avatar>,
    >,
    palettes: Res<RigPalettes>,
    mut hits: MessageWriter<Hit>,
    mut gizmos: Gizmos,
) {
    let dt = time.delta_secs();
    for (entity, mut p) in &mut shots {
        let from = p.at;
        let to = from + p.velocity * dt - Vec3::Y * (0.5 * p.gravity * dt * dt);
        let delta = to - from;
        let distance = delta.length();
        let direction = delta.normalize_or_zero();
        let wall = Dir3::new(direction)
            .ok()
            .and_then(|dir| world.ray_body(from, dir, distance))
            .map(|h| h.distance);
        let mut nearest = None;
        let mut limit = wall.unwrap_or(distance + 0.001);
        for (target, at, hurt, health, body, skin) in &targets {
            if health.cur <= 0.0 {
                continue;
            }
            let mut parts = posed_body(body, skin, &palettes);
            if parts.is_empty() {
                let (a, b) = hurt.axis(at.translation());
                parts.push((a, b, hurt.radius));
            }
            for (a, b, r) in parts {
                if let Some(d) = contact(from, to, a, b, r).filter(|d| *d < limit) {
                    limit = d;
                    nearest = Some(target);
                }
            }
        }
        gizmos.line(
            from,
            to,
            if p.bullet {
                Color::srgb(1.0, 0.8, 0.3)
            } else {
                Color::srgb(0.8, 0.7, 0.5)
            },
        );
        if let Some(target) = nearest {
            if let Ok((_, _, _, mut health, _, _)) = targets.get_mut(target) {
                health.cur = (health.cur - p.damage).max(0.0);
                hits.write(Hit {
                    target,
                    damage: p.damage,
                    point: from + direction * limit,
                    lethal: health.cur <= 0.0,
                    level: 1,
                    poise: 8.0,
                    from: p.from,
                    knock: 0.0,
                });
                info!("projectile: hit {target} for {}", p.damage);
            }
        }
        p.at = to;
        let gravity = p.gravity;
        p.velocity -= Vec3::Y * gravity * dt;
        p.age += dt;
        if nearest.is_some() || wall.is_some() || p.age > 4.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fast_projectile_hits_thin_capsule_between_frames() {
        assert!(contact(
            Vec3::ZERO,
            Vec3::X * 10.0,
            Vec3::new(5.0, -0.1, 0.0),
            Vec3::new(5.0, 0.1, 0.0),
            0.1
        )
        .is_some());
        assert!(contact(
            Vec3::ZERO,
            Vec3::X * 10.0,
            Vec3::new(5.0, -0.1, 1.0),
            Vec3::new(5.0, 0.1, 1.0),
            0.1
        )
        .is_none());
    }
}
