//! Hits: the blade against enemy hurtboxes, and what a hit does.
//!
//! During a swing's active window the weapon (from the blade's `$WTT` tip marker, see
//! [`crate::actor::held`], down to the hand holding it) is swept from its last frame's place to this frame's: the old and new
//! blade and the paths of its tip, middle and base are tested against each target's vertical
//! capsule [`Hurtbox`], so a fast swing cannot pass through a target between frames. A target is
//! hit at most once per swing.
//!
//! A hit costs health, freezes the world for a moment (hit-stop: virtual time slowed, so the
//! swing, the motors and the animations all hold together), and makes the target flinch; at zero
//! health it dies: it plays its death, stays down, and stops being a target.

use bevy::prelude::*;

use crate::actor::held::HeldBuilt;
use crate::avatar::Avatar;
use crate::combat::{Action, ActionState, Health, SwingPhase, SwingSpec};
use crate::controller::PlaySet;
use crate::hitbody::{posed_body, BodyHits};
use benilla_world::rig_palette::{RigPalettes, RigSkin};

/// The base hit; a move's damage multiplier (`SwingSpec::mult`) scales it.
pub const LIGHT_DAMAGE: f32 = 20.0;

/// The damage of one hit of a move: the base hit times the move's multiplier.
fn hit_damage(spec: &SwingSpec) -> f32 {
    LIGHT_DAMAGE * spec.mult
}
/// The blade's hit thickness for the sweep (yd): forgiving, as Souls weapon hitboxes are. A
/// human's one-handed slash crosses a 1.3 yd kobold about half a yard above its head's centre
/// line (measured), so a thin blade whiffs over small enemies. Tuning value.
pub const BLADE_RADIUS: f32 = 0.18;
/// The poise damage the player's blow does when the data gives it none: `AtkParam_Pc.atkSuperArmor`
/// is 0 on every straight-sword move, so a creature with poise (Garrick's 20) could never be broken
/// by it. A tuning value: a light blow (level 1) takes 8, a heavy one (level 2) 20, more 30.
pub fn light_poise(level: u8) -> f32 {
    match level {
        0 => 0.0,
        1 => 8.0,
        2 => 20.0,
        _ => 30.0,
    }
}
/// How long a hit freezes the world (real seconds) and how slow it runs meanwhile.
pub const HIT_STOP: f32 = 0.07;
pub const HIT_STOP_SPEED: f32 = 0.05;

/// A creature too small for a human's swing: the player's blade treats each of its body capsules
/// as at least this thick (yd). Dark Souls' own answer is a fat hit volume (the sword's `hit0_Radius`
/// is 0.3-0.4 m, in [`SwingSpec::radius`]); a creature as small as a young wolf still slips under it.
#[derive(Component, Debug, Clone, Copy)]
pub struct HitFloor(pub f32);

/// A body that can be struck: a vertical capsule from the feet, `height` tall.
#[derive(Component, Debug, Clone, Copy)]
pub struct Hurtbox {
    pub radius: f32,
    pub height: f32,
}

impl Hurtbox {
    /// The capsule's axis ends (the hemisphere centres) for feet at `feet`.
    pub fn axis(&self, feet: Vec3) -> (Vec3, Vec3) {
        let r = self.radius.min(self.height * 0.5);
        (feet + Vec3::Y * r, feet + Vec3::Y * (self.height - r))
    }
}

/// One landed blow.
#[derive(Message, Debug, Clone, Copy)]
pub struct Hit {
    pub target: Entity,
    pub damage: f32,
    pub point: Vec3,
    /// The target's health went to zero with this hit.
    pub lethal: bool,
    /// The blow's flinch level and poise damage (the move's), and where it came from.
    pub level: u8,
    pub poise: f32,
    pub from: Vec3,
    /// How far the blow pushes its target (Dark Souls metres, `knockbackDist`).
    pub knock: f32,
}

/// The blade's last frame, and who this swing has already struck.
#[derive(Component, Debug, Default)]
pub struct BladeTrail {
    prev: Option<[Vec3; 2]>,
    /// The serial of the swing that `struck` belongs to.
    swing: Option<u32>,
    hit_index: usize,
    struck: Vec<Entity>,
}

/// The world is frozen for a hit until this real time.
#[derive(Resource, Debug, Default)]
pub struct HitStop {
    until: Option<f32>,
}

impl HitStop {
    /// Freezes the world for `secs` real seconds from `now`.
    pub fn freeze(&mut self, virt: &mut Time<Virtual>, now: f32, secs: f32) {
        virt.set_relative_speed(HIT_STOP_SPEED);
        self.until = Some(self.until.map_or(now + secs, |u| u.max(now + secs)));
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins((
        crate::hitbody::plugin,
        crate::motion_clip::plugin,
        crate::weapons::plugin,
        crate::rig_view::plugin,
        crate::monster::plugin,
    ))
    .add_message::<Hit>()
    .init_resource::<HitStop>()
    .init_resource::<Impacts>()
    .add_systems(Update, release_hit_stop.after(PlaySet::Motor))
    // The blade and the bodies as this frame draws them: after the pose is final (the rig
    // world pass), not the previous frame's, which `Update` would read.
    .add_systems(
        PostUpdate,
        (strike, draw_hurtboxes).after(benilla_world::billboard::BillboardPlace),
    )
    .add_systems(Update, flash_impacts);
    if crate::actor::anim::trace_on() {
        app.add_systems(
            PostUpdate,
            trace_blade
                .after(benilla_world::billboard::BillboardPlace)
                .after(strike),
        );
    }
}

/// `WOWSOULS_TRACE`: every frame of a swing, as drawn this frame: the window phase, the ground
/// distance between the two bodies (`d`), the blade's clearance past its reach (yd, negative =
/// inside) to the target's posed body capsules (`parts`, what [`strike`] tests) and to its
/// fallback capsule at the feet (`cap`), and the blade tip's and hand's heights over its feet.
#[allow(clippy::type_complexity)]
fn trace_blade(
    avatar: Query<(&Action, &HeldBuilt, &GlobalTransform), With<Avatar>>,
    roots: Query<&GlobalTransform>,
    targets: Query<
        (
            Entity,
            &GlobalTransform,
            &Hurtbox,
            Option<&BodyHits>,
            Option<&RigSkin>,
        ),
        Without<Avatar>,
    >,
    palettes: Res<RigPalettes>,
) {
    let Ok((action, held, body)) = avatar.single() else {
        return;
    };
    let ActionState::Attack(swing) = action.state else {
        return;
    };
    let (Some([top, _]), Ok(root)) = (held.blade, roots.get(held.root)) else {
        return;
    };
    let (tip, hand) = (root.transform_point(top), root.translation());
    let apart = |at: Vec3| (at - body.translation()).xz().length();
    for (e, at, hurt, body, skin) in &targets {
        let parts = posed_body(body, skin, &palettes)
            .into_iter()
            .map(|(a, b, r)| segment_distance(tip, hand, a, b).0 - r - swing.spec.radius)
            .min_by(f32::total_cmp);
        let feet = at.translation();
        let (axis_a, axis_b) = hurt.axis(feet);
        let cap = segment_distance(tip, hand, axis_a, axis_b).0 - hurt.radius - swing.spec.radius;
        info!(
            "blade {e} f={:.2} {:?} d={:.2} parts={} cap={cap:+.2} tip_h={:.2} hand_h={:.2}",
            swing.t / swing.spec.duration,
            swing.phase(),
            apart(feet),
            parts.map_or("-".into(), |p| format!("{p:+.2}")),
            tip.y - feet.y,
            hand.y - feet.y
        );
    }
}

/// F2: each enemy's hurtbox, the capsule the blade is tested against.
#[allow(clippy::type_complexity)]
fn draw_hurtboxes(
    show: Res<crate::avatar::ShowCapsule>,
    targets: Query<
        (
            &GlobalTransform,
            &Hurtbox,
            Option<&BodyHits>,
            Option<&RigSkin>,
        ),
        Without<Avatar>,
    >,
    palettes: Res<RigPalettes>,
    mut gizmos: Gizmos,
) {
    if !show.0 {
        return;
    }
    for (at, hurt, body, skin) in &targets {
        let parts = posed_body(body, skin, &palettes);
        for (a, b, r) in &parts {
            let axis = *b - *a;
            let rot = Quat::from_rotation_arc(Vec3::Y, axis.normalize_or(Vec3::Y));
            gizmos.primitive_3d(
                &Capsule3d::new(*r, axis.length()),
                Isometry3d::new((*a + *b) * 0.5, rot),
                Color::srgb(1.0, 0.55, 0.1),
            );
        }
        if !parts.is_empty() {
            continue;
        }
        let r = hurt.radius.min(hurt.height * 0.5);
        gizmos.primitive_3d(
            &Capsule3d::new(r, hurt.height - 2.0 * r),
            Isometry3d::from_translation(at.translation() + Vec3::Y * (hurt.height * 0.5)),
            Color::srgb(1.0, 0.85, 0.2),
        );
    }
}

/// Recent impact points and the real time each fades.
#[derive(Resource, Debug, Default)]
pub struct Impacts(Vec<(Vec3, f32)>);

/// How long an impact flash shows (real seconds).
const FLASH: f32 = 0.15;

/// A short burst at each blade contact: the gizmo cross stands in until the blood plays
/// ([`crate::blood`]), and shows only where there is none.
fn flash_impacts(
    real: Res<Time<bevy::time::Real>>,
    mut hits: MessageReader<Hit>,
    mut impacts: ResMut<Impacts>,
    blood: Option<Res<crate::blood::BloodTables>>,
    mut gizmos: Gizmos,
) {
    let now = real.elapsed_secs();
    for h in hits.read() {
        if blood.is_none() {
            impacts.0.push((h.point, now + FLASH));
        }
    }
    impacts.0.retain(|&(_, until)| until > now);
    for &(p, until) in &impacts.0 {
        let k = ((until - now) / FLASH).clamp(0.0, 1.0);
        let color = Color::srgba(1.0, 0.85, 0.5, k);
        for d in [
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            Vec3::new(1.0, 1.0, 0.0).normalize(),
        ] {
            gizmos.line(p - d * 0.25 * k, p + d * 0.25 * k, color);
        }
    }
}

/// The closest points of segments `p1q1` and `p2q2`, and their distance (Ericson, Real-Time
/// Collision Detection, 5.1.9).
pub fn segment_distance(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (f32, Vec3, Vec3) {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.length_squared();
    let e = d2.length_squared();
    let f = d2.dot(r);
    const EPS: f32 = 1.0e-8;
    let (s, t) = if a <= EPS && e <= EPS {
        (0.0, 0.0)
    } else if a <= EPS {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= EPS {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s = if denom > EPS {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    let c1 = p1 + d1 * s;
    let c2 = p2 + d2 * t;
    (c1.distance(c2), c1, c2)
}

/// Whether a blade sweeping from `prev` to `now` (each `[tip, base]`) touches the capsule with
/// axis `cap` and radius `cap_r`; the touch point on the blade when it does.
pub fn blade_hits(
    now: [Vec3; 2],
    prev: Option<[Vec3; 2]>,
    cap: (Vec3, Vec3),
    cap_r: f32,
    blade_r: f32,
) -> Option<Vec3> {
    let reach = cap_r + blade_r;
    let mut segments = vec![(now[0], now[1])];
    if let Some(p) = prev {
        segments.push((p[0], p[1]));
        // The paths of the tip, the middle and the base between the frames.
        segments.push((p[0], now[0]));
        segments.push((p[0].lerp(p[1], 0.5), now[0].lerp(now[1], 0.5)));
        segments.push((p[1], now[1]));
    }
    segments
        .into_iter()
        .map(|(a, b)| segment_distance(a, b, cap.0, cap.1))
        .filter(|(d, _, _)| *d <= reach)
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(_, on_blade, _)| on_blade)
}

/// Sweeps the avatar's blade through its active window and lands hits.
#[allow(clippy::too_many_arguments)]
fn strike(
    moves: Option<Res<crate::motion_clip::WeaponClips>>,
    real: Res<Time<bevy::time::Real>>,
    mut virt: ResMut<Time<Virtual>>,
    mut stop: ResMut<HitStop>,
    mut avatar: Query<
        (
            &Action,
            &HeldBuilt,
            &mut BladeTrail,
            &GlobalTransform,
            &mut crate::combat::Stamina,
            &mut Health,
        ),
        With<Avatar>,
    >,
    roots: Query<&GlobalTransform>,
    mut targets: Query<
        (
            Entity,
            &GlobalTransform,
            &Hurtbox,
            &mut Health,
            Option<&BodyHits>,
            Option<&RigSkin>,
            Option<&HitFloor>,
        ),
        Without<Avatar>,
    >,
    palettes: Res<RigPalettes>,
    mut hits: MessageWriter<Hit>,
    buffs: Res<crate::abilities::Buffs>,
    gear: Res<crate::gear_effect::GearFx>,
    disarmed: Option<Res<crate::named::Disarmed>>,
    (talents, mut warrior, mut rage, time, mut commands): (
        Res<crate::talents::Talents>,
        ResMut<crate::warrior::Warrior>,
        ResMut<crate::abilities::Rage>,
        Res<Time>,
        Commands,
    ),
) {
    if moves.as_ref().is_some_and(|m| m.projectile.is_some()) {
        return;
    }
    let Ok((action, held, mut trail, avatar_at, mut stamina, mut hp)) = avatar.single_mut() else {
        return;
    };
    let (Some([top, _]), Ok(root)) = (held.blade, roots.get(held.root)) else {
        return;
    };
    // The whole weapon strikes, from the blade's tip down to the hand that holds it (the item
    // root sits at the hand's attachment point): point-blank, the hilt end passes through the
    // target while the tip is already past it.
    let now = [root.transform_point(top), root.translation()];
    let prev = trail.prev.replace(now);
    let ActionState::Attack(mut swing) = action.state else {
        trail.swing = None;
        return;
    };
    let window = if swing.reference {
        moves
            .as_deref()
            .and_then(|m| m.clip(swing.kind))
            .and_then(|c| {
                c.hits
                    .iter()
                    .enumerate()
                    .find(|(_, s)| swing.t >= s.hit.0 && swing.t < s.hit.1)
            })
            .map(|(i, s)| (i, *s))
    } else {
        (swing.phase() == Some(SwingPhase::Active)).then_some((0, swing.spec))
    };
    let Some((hit_index, spec)) = window else {
        return;
    };
    swing.spec = spec;
    // Each authored hit window may strike once, including a heavy attack's second contact.
    if trail.swing != Some(swing.serial) || trail.hit_index != hit_index || swing.t <= 0.0 {
        trail.swing = Some(swing.serial);
        trail.hit_index = hit_index;
        trail.struck.clear();
    }
    for (target, at, hurt, mut health, body, skin, floor) in &mut targets {
        if health.cur <= 0.0 || trail.struck.contains(&target) {
            continue;
        }
        let parts = posed_body(body, skin, &palettes);
        if crate::actor::anim::trace_on() {
            let cap = hurt.axis(at.translation());
            let d = segment_distance(now[0], now[1], cap.0, cap.1).0;
            info!(
                "blade t={:.2} tip={:.2} base={:.2} target={:.2} dist={d:.2} reach={:.2}",
                swing.t,
                now[0],
                now[1],
                at.translation(),
                hurt.radius + swing.spec.radius
            );
        }
        // The posed body where it has one; the capsule at the feet otherwise.
        let contact = if parts.is_empty() {
            blade_hits(
                now,
                prev,
                hurt.axis(at.translation()),
                hurt.radius,
                swing.spec.radius,
            )
        } else {
            parts.iter().find_map(|&(a, b, r)| {
                blade_hits(
                    now,
                    prev,
                    (a, b),
                    r.max(floor.map_or(0.0, |f| f.0)),
                    swing.spec.radius,
                )
            })
        };
        let Some(point) = contact else {
            continue;
        };
        trail.struck.push(target);
        let contact = warrior.contact(
            &talents,
            swing,
            target,
            time.elapsed_secs(),
            rage.cur,
            hp.cur / hp.max.max(1.0),
        );
        // Battle Shout (`crate::abilities`) and the weapon's own worth (`crate::gear_effect`) scale it.
        let damage = if swing.reference {
            moves.as_ref().map_or(LIGHT_DAMAGE, |m| m.base_damage) * swing.spec.mult
        } else {
            hit_damage(&swing.spec)
        } * buffs.damage_factor()
            * gear.0.melee_factor
            // Disarmed ([`crate::named`]), the fists strike for a third.
            * disarmed.as_deref().map_or(1.0, crate::named::disarm_factor)
            * contact.damage;
        health.cur = (health.cur - damage).max(0.0);
        let lethal = health.cur <= 0.0;
        warrior.landed(&talents, swing.serial, lethal, &mut hp, &mut stamina);
        if contact.signature {
            rage.ability_damage += damage;
        }
        if !lethal && contact.bleed > 0.0 {
            commands.entity(target).insert(crate::warrior::Bleed {
                left: 4.0,
                damage_left: damage * contact.bleed,
                tick: 0.0,
                from: avatar_at.translation(),
            });
        }
        hits.write(Hit {
            target,
            damage,
            point,
            lethal,
            level: swing.spec.level,
            poise: (if swing.reference {
                swing.spec.poise
            } else {
                swing.spec.poise.max(light_poise(swing.spec.level))
            }) * contact.poise,
            from: avatar_at.translation(),
            knock: swing.spec.knock,
        });
        info!(
            "hit: {target} for {damage:.0} at {point:.2}, health {:.0}/{:.0}{} (stop {:.2} s, knock {:.2} m)",
            health.cur,
            health.max,
            if lethal { " (killed)" } else { "" },
            swing.spec.stop,
            swing.spec.knock
        );
        // The move's own hit-stop; none, no freeze.
        if swing.spec.stop > 0.0 {
            stop.freeze(&mut virt, real.elapsed_secs(), swing.spec.stop);
        }
    }
}

/// Lets the world run again when the hit-stop is over.
fn release_hit_stop(
    real: Res<Time<bevy::time::Real>>,
    mut virt: ResMut<Time<Virtual>>,
    mut stop: ResMut<HitStop>,
) {
    if stop.until.is_some_and(|t| real.elapsed_secs() >= t) {
        stop.until = None;
        virt.set_relative_speed(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1.0e-4
    }

    #[test]
    fn a_moves_damage_is_the_base_hit_times_its_multiplier() {
        let hit = |mult| {
            hit_damage(&SwingSpec {
                mult,
                ..SwingSpec::procedural(0.8)
            })
        };
        // The spec's table: R1 x1.00, R2 x1.35, R2 second x1.25, rolling attack x0.90.
        for (mult, want) in [(1.0, 20.0), (1.35, 27.0), (1.25, 25.0), (0.9, 18.0)] {
            assert!(close(hit(mult), want), "x{mult}: {}", hit(mult));
        }
        assert!(close(hit(SwingSpec::procedural(0.8).mult), LIGHT_DAMAGE));
    }

    #[test]
    fn crossing_segments_touch() {
        let (d, _, _) = segment_distance(
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        assert!(close(d, 0.0));
    }

    #[test]
    fn parallel_segments_keep_their_gap() {
        let (d, _, _) = segment_distance(
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(1.0, 2.0, 0.0),
        );
        assert!(close(d, 2.0));
    }

    #[test]
    fn skew_segments_measure_between_their_closest_points() {
        // One along X at z=0, one along Y at z=3: the gap is 3.
        let (d, a, b) = segment_distance(
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, -1.0, 3.0),
            Vec3::new(0.0, 1.0, 3.0),
        );
        assert!(close(d, 3.0));
        assert!(a.distance(Vec3::ZERO) < 1.0e-4 && b.distance(Vec3::new(0.0, 0.0, 3.0)) < 1.0e-4);
    }

    #[test]
    fn segment_ends_clamp() {
        // Past the end of the first segment, the end point is the closest.
        let (d, _, _) = segment_distance(
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(3.0, -1.0, 0.0),
            Vec3::new(3.0, 1.0, 0.0),
        );
        assert!(close(d, 2.0));
    }

    /// A capsule standing at the origin, 1.2 tall, radius 0.4.
    fn target() -> ((Vec3, Vec3), f32) {
        let h = Hurtbox {
            radius: 0.4,
            height: 1.2,
        };
        (h.axis(Vec3::ZERO), h.radius)
    }

    #[test]
    fn a_blade_through_the_body_hits() {
        let (cap, r) = target();
        let blade = [Vec3::new(0.3, 0.8, 0.0), Vec3::new(1.2, 0.8, 0.0)];
        assert!(blade_hits(blade, None, cap, r, BLADE_RADIUS).is_some());
    }

    #[test]
    fn a_blade_out_of_reach_misses() {
        let (cap, r) = target();
        let blade = [Vec3::new(1.5, 0.8, 0.0), Vec3::new(2.5, 0.8, 0.0)];
        assert!(blade_hits(blade, None, cap, r, BLADE_RADIUS).is_none());
    }

    #[test]
    fn a_fast_swing_that_jumps_over_the_body_between_frames_still_hits() {
        // Last frame the blade was left of the body, this frame right of it: neither frame
        // touches, the sweep between them does.
        let (cap, r) = target();
        // The tip path passes 0.2 yd from the axis, inside the 0.48 yd reach.
        let prev = [Vec3::new(-1.5, 0.8, 0.2), Vec3::new(-1.5, 0.8, 1.2)];
        let now = [Vec3::new(1.5, 0.8, 0.2), Vec3::new(1.5, 0.8, 1.2)];
        assert!(blade_hits(now, None, cap, r, BLADE_RADIUS).is_none());
        assert!(blade_hits(prev, None, cap, r, BLADE_RADIUS).is_none());
        assert!(blade_hits(now, Some(prev), cap, r, BLADE_RADIUS).is_some());
    }

    #[test]
    fn the_capsule_axis_stays_inside_a_short_body() {
        let h = Hurtbox {
            radius: 1.0,
            height: 1.0,
        };
        let (a, b) = h.axis(Vec3::ZERO);
        assert!(close(a.y, 0.5) && close(b.y, 0.5));
    }
}
