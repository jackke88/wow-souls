//! Souls ranged actions: release at the clip event, finite flight, swept world/body collision.
//! Speeds, gravity and damage are explicit prototype estimates (see weapon provenance).
//!
//! A spell's missile ([`crate::spells`], the Mage) flies the same way: the same sweep against the world
//! and the posed bodies, the same [`Hit`]; it adds a target to steer at, a fatter body and a [`SpellHit`]
//! on contact (what the hit leaves behind is the spells').
use crate::{
    actor::held::HeldBuilt,
    avatar::Avatar,
    combat::{Action, ActionState, Health},
    hitbody::{posed_body, BodyHits},
    hits::{segment_distance, Hit, Hurtbox},
    lockon::{LockOn, Lockable},
    motion_clip::WeaponClips,
    spells::cast::Impact,
};
use benilla_world::{
    collision::WorldCollision,
    rig_palette::{RigPalettes, RigSkin},
};
use bevy::prelude::*;

/// The margin an arrow's contact test adds to a body's radius (yd).
const ARROW_MARGIN: f32 = 0.06;
/// An arrow's blow: its flinch level and poise damage (a spell without Dark Souls numbers hits alike).
const ARROW_LEVEL: u8 = 1;
const ARROW_POISE: f32 = 8.0;

#[derive(Component)]
pub(crate) struct Projectile {
    at: Vec3,
    velocity: Vec3,
    gravity: f32,
    damage: f32,
    from: Vec3,
    age: f32,
    bullet: bool,
    /// A spell's flight: what it is, how long it may fly and what it steers at; `None` for an arrow.
    spell: Option<SpellFlight>,
}

/// What makes a [`Projectile`] a spell's missile.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpellFlight {
    pub spell: u32,
    /// Seconds it may fly (range over speed).
    pub life: f32,
    /// The enemy it steers at (it flies straight when that is dead or gone).
    pub target: Option<Entity>,
    /// How fast it may turn toward the target (rad/s).
    pub turn: f32,
    /// Its body's radius (yd), the margin of the contact test.
    pub radius: f32,
    /// What it does where it lands (Dark Souls' attack row of the spell); `None`: an arrow's blow.
    pub impact: Option<Impact>,
}

/// A spell's missile struck `target` at `point`: its extras (a slow, a burn, the impact) are the
/// spells' to apply ([`crate::spells`]).
#[derive(Message, Debug, Clone, Copy)]
pub struct SpellHit {
    pub target: Entity,
    pub spell: u32,
    pub point: Vec3,
}

impl Projectile {
    /// A spell's missile leaving `at` along `direction` at `speed` (yd/s); nothing pulls it down.
    pub(crate) fn spell(
        at: Vec3,
        direction: Vec3,
        speed: f32,
        damage: f32,
        from: Vec3,
        flight: SpellFlight,
    ) -> Projectile {
        Projectile {
            at,
            velocity: direction * speed,
            gravity: 0.0,
            damage,
            from,
            age: 0.0,
            bullet: false,
            spell: Some(flight),
        }
    }
}

/// The flinch level, the poise damage and the push (Dark Souls metres) a blow carries: the spell's own
/// attack row, or an arrow's.
pub(crate) fn blow_of(impact: Option<Impact>) -> (u8, f32, f32) {
    impact.map_or((ARROW_LEVEL, ARROW_POISE, 0.0), |i| {
        (i.level, i.poise, i.knock)
    })
}

/// Who a blast `radius` (yd) wide centred on `centre` takes in: every body (its capsule parts: axis ends
/// and radius) within the radius of the point, except `skip`, the one the missile struck. No radius, no
/// blast.
pub(crate) fn blast_victims(
    centre: Vec3,
    radius: f32,
    bodies: &[(Entity, Vec<(Vec3, Vec3, f32)>)],
    skip: Option<Entity>,
) -> Vec<Entity> {
    if radius <= 0.0 {
        return Vec::new();
    }
    bodies
        .iter()
        .filter(|(e, _)| Some(*e) != skip)
        .filter(|(_, parts)| {
            parts
                .iter()
                .any(|&(a, b, r)| point_segment_distance(centre, a, b) <= radius + r)
        })
        .map(|(e, _)| *e)
        .collect()
}

/// The distance from `p` to the segment `a`-`b`.
fn point_segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 <= f32::EPSILON {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// `velocity` turned toward the direction `to` by at most `max_angle` radians, its speed kept.
pub(crate) fn steer(velocity: Vec3, to: Vec3, max_angle: f32) -> Vec3 {
    let speed = velocity.length();
    let (cur, want) = (velocity.normalize_or_zero(), to.normalize_or_zero());
    if speed == 0.0 || cur == Vec3::ZERO || want == Vec3::ZERO {
        return velocity;
    }
    let angle = cur.angle_between(want);
    if angle <= max_angle {
        return want * speed;
    }
    let axis = cur.cross(want);
    if axis.length_squared() < 1.0e-8 {
        return velocity;
    }
    Quat::from_axis_angle(axis.normalize(), max_angle) * cur * speed
}

/// A launch fires once per swing or cast: `true` the first time the body's `(serial, released)` is seen
/// with `released` set. `None` (the body is not swinging or casting) forgets the last one, so the first
/// swing of a new life, whose counter starts again, is not taken for the old one.
pub(crate) fn release_due(
    fired: &mut Option<(Entity, u32)>,
    who: Entity,
    swing: Option<(u32, bool)>,
) -> bool {
    let Some((serial, released)) = swing else {
        *fired = None;
        return false;
    };
    if !released || *fired == Some((who, serial)) {
        return false;
    }
    *fired = Some((who, serial));
    true
}

pub fn plugin(app: &mut App) {
    app.add_message::<SpellHit>().add_systems(
        PostUpdate,
        (release, fly)
            .chain()
            .after(benilla_world::billboard::BillboardPlace),
    );
}

#[allow(clippy::too_many_arguments)]
fn release(
    mut commands: Commands,
    moves: Option<Res<WeaponClips>>,
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
    let swing = match action.state {
        ActionState::Attack(swing) => Some(swing),
        _ => None,
    };
    let due = release_due(
        &mut fired,
        entity,
        swing.map(|s| (s.serial, s.t >= spec.release)),
    );
    let (true, Some(swing)) = (due, swing) else {
        return;
    };
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
        spell: None,
    });
    info!(
        "projectile: {} released at {:.3}s (source {:.3}s)",
        moves.family, swing.t, spec.release
    );
}

/// Contact distance along a swept segment, including targets narrower than one frame's travel;
/// `margin` is what the missile's own body adds to the target's radius `r`.
fn contact(from: Vec3, to: Vec3, a: Vec3, b: Vec3, r: f32, margin: f32) -> Option<f32> {
    let (distance, on_ray, _) = segment_distance(from, to, a, b);
    (distance <= r + margin).then(|| on_ray.distance(from))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    world: WorldCollision,
    mut shots: Query<(Entity, &mut Projectile, Option<&mut Transform>)>,
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
    mut spell_hits: MessageWriter<SpellHit>,
    mut gizmos: Gizmos,
) {
    let dt = time.delta_secs();
    for (entity, mut p, mut model) in &mut shots {
        // A spell's missile steers at its enemy while the enemy lives; a dead or gone one is let go.
        if let Some(SpellFlight {
            target: Some(t),
            turn,
            ..
        }) = p.spell
        {
            if let Ok((_, at, hurt, health, _, _)) = targets.get(t) {
                if health.cur > 0.0 {
                    let (a, b) = hurt.axis(at.translation());
                    p.velocity = steer(p.velocity, (a + b) * 0.5 - p.at, turn * dt);
                }
            }
        }
        let margin = p.spell.map_or(ARROW_MARGIN, |s| s.radius);
        let (level, poise, knock) = blow_of(p.spell.and_then(|s| s.impact));
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
                if let Some(d) = contact(from, to, a, b, r, margin).filter(|d| *d < limit) {
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
                    level,
                    poise,
                    from: p.from,
                    knock,
                });
                info!("projectile: hit {target} for {}", p.damage);
                if let Some(s) = p.spell {
                    spell_hits.write(SpellHit {
                        target,
                        spell: s.spell,
                        point: from + direction * limit,
                    });
                }
            }
        }
        // An explosion (a fireball's) takes what stands around where it burst: on a body or on the world.
        let blast = p
            .spell
            .and_then(|s| s.impact)
            .map_or(0.0, |i| i.blast * crate::shove::METRES_TO_YARDS);
        if blast > 0.0 && (nearest.is_some() || wall.is_some()) {
            let centre = from + direction * limit;
            let bodies: Vec<(Entity, Vec<(Vec3, Vec3, f32)>)> = targets
                .iter()
                .filter(|(_, _, _, health, _, _)| health.cur > 0.0)
                .map(|(e, at, hurt, _, body, skin)| {
                    let mut parts = posed_body(body, skin, &palettes);
                    if parts.is_empty() {
                        let (a, b) = hurt.axis(at.translation());
                        parts.push((a, b, hurt.radius));
                    }
                    (e, parts)
                })
                .collect();
            for victim in blast_victims(centre, blast, &bodies, nearest) {
                if let Ok((_, _, _, mut health, _, _)) = targets.get_mut(victim) {
                    health.cur = (health.cur - p.damage).max(0.0);
                    hits.write(Hit {
                        target: victim,
                        damage: p.damage,
                        point: centre,
                        lethal: health.cur <= 0.0,
                        level,
                        poise,
                        from: p.from,
                        knock,
                    });
                    info!("projectile: the blast takes {victim} for {}", p.damage);
                }
            }
        }
        p.at = to;
        let gravity = p.gravity;
        p.velocity -= Vec3::Y * gravity * dt;
        p.age += dt;
        // A spell's missile carries its model: the entity follows the flight.
        if let Some(t) = model.as_deref_mut() {
            t.translation = to;
            if direction != Vec3::ZERO {
                t.look_to(direction, Vec3::Y);
            }
        }
        let end = p.spell.map_or(4.0, |s| s.life);
        if nearest.is_some() || wall.is_some() || p.age > end {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_happens_once_per_swing_and_a_new_life_starts_clean() {
        let me = Entity::from_raw_u32(1).unwrap();
        let mut fired = None;
        assert!(release_due(&mut fired, me, Some((5, true))));
        assert!(
            !release_due(&mut fired, me, Some((5, true))),
            "same swing, same release"
        );
        assert!(
            release_due(&mut fired, me, Some((6, true))),
            "the next swing releases again"
        );
        assert!(
            !release_due(&mut fired, me, Some((7, false))),
            "not yet released"
        );
        // A death makes a new `Action`: its swing counter starts again, so the first swing of the new
        // life can carry the serial the last released one had, and must still release.
        assert!(release_due(&mut fired, me, Some((3, true))));
        assert!(
            !release_due(&mut fired, me, None),
            "no swing, nothing is due"
        );
        assert!(
            release_due(&mut fired, me, Some((3, true))),
            "serial 3 of the new life is not serial 3 of the old one"
        );
    }

    #[test]
    fn fast_projectile_hits_thin_capsule_between_frames() {
        assert!(contact(
            Vec3::ZERO,
            Vec3::X * 10.0,
            Vec3::new(5.0, -0.1, 0.0),
            Vec3::new(5.0, 0.1, 0.0),
            0.1,
            ARROW_MARGIN
        )
        .is_some());
        assert!(contact(
            Vec3::ZERO,
            Vec3::X * 10.0,
            Vec3::new(5.0, -0.1, 1.0),
            Vec3::new(5.0, 0.1, 1.0),
            0.1,
            ARROW_MARGIN
        )
        .is_none());
    }

    #[test]
    fn a_fat_missile_reaches_what_an_arrow_misses() {
        let (a, b) = (Vec3::new(5.0, -0.1, 0.4), Vec3::new(5.0, 0.1, 0.4));
        assert!(contact(Vec3::ZERO, Vec3::X * 10.0, a, b, 0.1, ARROW_MARGIN).is_none());
        assert!(contact(Vec3::ZERO, Vec3::X * 10.0, a, b, 0.1, 0.35).is_some());
    }

    #[test]
    fn steering_turns_toward_the_target_by_at_most_the_allowed_angle_and_keeps_the_speed() {
        let v = Vec3::new(0.0, 0.0, -10.0);
        let to = Vec3::new(10.0, 0.0, 0.0);
        let turned = steer(v, to, 0.2);
        assert!((turned.length() - 10.0).abs() < 1.0e-3, "speed kept");
        assert!(
            (turned.angle_between(v) - 0.2).abs() < 1.0e-3,
            "turned by the limit"
        );
        assert!(turned.x > 0.0, "toward the target");
        // Within the limit it points straight at the target.
        let want = Vec3::new(1.0, 0.0, -10.0);
        let near = steer(v, want, 0.5);
        assert!(near.normalize().angle_between(want.normalize()) < 1.0e-3);
        // No direction or no speed: untouched.
        assert_eq!(steer(Vec3::ZERO, to, 0.2), Vec3::ZERO);
        assert_eq!(steer(v, Vec3::ZERO, 0.2), v);
    }

    #[test]
    fn a_spell_missile_leaves_along_its_direction_and_falls_under_nothing() {
        let flight = SpellFlight {
            spell: 133,
            life: 1.5,
            target: None,
            turn: 2.0,
            radius: 0.25,
            impact: None,
        };
        let p = Projectile::spell(Vec3::ZERO, Vec3::NEG_Z, 24.0, 18.0, Vec3::ZERO, flight);
        assert_eq!(p.velocity, Vec3::new(0.0, 0.0, -24.0));
        assert_eq!(p.gravity, 0.0);
        assert_eq!(p.damage, 18.0);
        assert!(p.spell.is_some() && !p.bullet);
    }

    fn ent(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn a_spells_blow_carries_the_dark_souls_numbers_and_a_bare_missile_the_arrows() {
        let fireball = Impact {
            blast: 1.0,
            knock: 1.2,
            level: 3,
            poise: 40.0,
        };
        assert_eq!(blow_of(Some(fireball)), (3, 40.0, 1.2));
        assert_eq!(blow_of(None), (1, 8.0, 0.0), "an arrow's");
    }

    #[test]
    fn a_blast_takes_what_stands_within_its_radius_of_the_body_and_spares_the_one_struck() {
        let point = Vec3::new(10.0, 0.0, 0.0);
        let upright = |x: f32, z: f32| vec![(Vec3::new(x, 0.3, z), Vec3::new(x, 1.0, z), 0.3)];
        let bodies = [
            (ent(1), upright(10.0, 0.0)),
            (ent(2), upright(10.9, 0.0)),
            (ent(3), upright(12.5, 0.0)),
        ];
        // 1.1 yd wide: the second stands 0.9 yd off with a 0.3 yd body; the third 2.5 yd off.
        assert_eq!(
            blast_victims(point, 1.1, &bodies, Some(ent(1))),
            vec![ent(2)]
        );
        assert_eq!(
            blast_victims(point, 1.1, &bodies, None),
            vec![ent(1), ent(2)]
        );
        assert!(
            blast_victims(point, 0.0, &bodies, None).is_empty(),
            "a plain hit has no blast"
        );
    }

    #[test]
    fn a_blast_measures_to_the_whole_body_not_to_the_feet() {
        let point = Vec3::new(0.0, 1.0, 0.0);
        let tall = vec![(Vec3::new(1.0, 0.5, 0.0), Vec3::new(1.0, 2.5, 0.0), 0.4)];
        assert_eq!(
            blast_victims(point, 0.7, &[(ent(1), tall)], None),
            vec![ent(1)]
        );
    }
}
