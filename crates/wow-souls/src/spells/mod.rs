//! The Mage's spells (spec `docs/superpowers/specs/2026-10-14-mage-class-design.md`): checks a cast
//! before it is paid for ([`plan`]), the release (a missile, an instant hit or a buff), the missile's hits
//! and what they leave behind, the buffs. The cast clips are [`cast`]; the body's commitment to a cast
//! is the action machine's (`combat::ActionState::Cast`); the numbers are `wow_world::spells`.

pub mod cast;

use bevy::prelude::*;

use wow_world::spells::{self as defs, Delivery, SpellDef};

use crate::abilities::fx::{self, Kind};
use crate::abilities::{Buffs, Refusal};
use crate::actor::held::HeldBuilt;
use crate::audio::Audio;
use crate::avatar::Avatar;
use crate::combat::{Action, ActionState, CastSpec, Health};
use crate::hits::{Hit, Hurtbox};
use crate::lockon::{LockOn, Lockable};
use crate::motor::MoveIntent;
use crate::npc::Hostile;
use crate::projectiles::{blow_of, release_due, Projectile, SpellFlight, SpellHit};
use crate::settle::Phase;

/// An enemy this far off the way ahead (cosine of the angle: 60 degrees) is not the one an instant spell
/// picks when none is locked.
const AIM_CONE: f32 = 0.5;
/// How fast a missile may turn per degree of the Dark Souls bullet's homing angle (rad/s per degree):
/// Soul Arrow's 40 degrees turns it at about 4 rad/s, Fireball's 10 at about 1. A first guess.
const TURN_PER_HOMING_DEG: f32 = 0.105;

/// What a checked cast will do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastPlan {
    pub spec: CastSpec,
    pub target: Option<Entity>,
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// The enemy an instant spell hits: the locked one (any direction) or the nearest ahead, within `range`.
pub fn aim_instant(
    player: Vec3,
    facing: Vec3,
    locked: Option<Entity>,
    foes: &[(Entity, Vec3)],
    range: f32,
) -> Result<Entity, Refusal> {
    let reach = |p: Vec3| flat(p - player).length();
    let judge = |d: f32| {
        if d > range {
            Err(Refusal::TooFar)
        } else {
            Ok(())
        }
    };
    if let Some((e, p)) = locked.and_then(|l| foes.iter().find(|(e, _)| *e == l)) {
        return judge(reach(*p)).map(|()| *e);
    }
    let nose = flat(facing).normalize_or_zero();
    let mut ahead: Vec<(Entity, f32)> = foes
        .iter()
        .filter(|(_, p)| flat(*p - player).normalize_or_zero().dot(nose) >= AIM_CONE)
        .map(|(e, p)| (*e, reach(*p)))
        .collect();
    ahead.sort_by(|a, b| a.1.total_cmp(&b.1));
    match ahead.first() {
        Some((e, d)) => judge(*d).map(|()| *e),
        None => Err(Refusal::NoTarget),
    }
}

/// Checks a cast before anything is paid: the cast clips are loaded, the body can start one (`can_cast`,
/// `grounded`), and an instant spell has its target in range.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    def: &SpellDef,
    can_cast: bool,
    grounded: bool,
    timing: Option<cast::Timing>,
    player: Vec3,
    facing: Vec3,
    locked: Option<Entity>,
    foes: &[(Entity, Vec3)],
) -> Result<CastPlan, Refusal> {
    let t = timing.ok_or(Refusal::NotYet)?;
    if !can_cast || !grounded {
        return Err(Refusal::Busy);
    }
    let target = match def.delivery {
        Delivery::Instant => Some(aim_instant(player, facing, locked, foes, def.range)?),
        Delivery::Missile { .. } => locked,
        Delivery::SelfBuff => None,
    };
    Ok(CastPlan {
        spec: CastSpec {
            duration: t.duration,
            release: t.release,
            recover: t.recover,
        },
        target,
    })
}

/// Uniform numbers for the damage rolls (xorshift; seeded from the clock).
#[derive(Resource)]
pub struct SpellRng(u32);

impl Default for SpellRng {
    fn default() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9E37_79B9, |d| d.subsec_nanos() ^ 0x9E37_79B9);
        SpellRng(seed.max(1))
    }
}

impl SpellRng {
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// How long the release's flash lasts in a hand (s).
const HAND_FLASH: f32 = 0.4;

/// What a cast's wind-up put on the body: the flames in the hands and the precast sound, until the
/// release (or whatever replaces the cast) puts them out.
#[derive(Component)]
pub struct CastFx {
    pub effects: Vec<Entity>,
    pub sound: Option<crate::mixer::Level>,
}

/// The wind-up is over: the cast was released (`t` has reached `release`), or there is no cast any more
/// (a stagger, a roll or a death replaced it). `cast` is `(t, release)` of the cast in progress.
pub fn precast_over(cast: Option<(f32, f32)>) -> bool {
    cast.is_none_or(|(t, release)| t >= release)
}

/// Puts the wind-up's flames out and its sound down when it is over.
fn end_precast(mut commands: Commands, avatar: Query<(Entity, &Action, &CastFx), With<Avatar>>) {
    for (me, action, fx) in &avatar {
        let cast = match action.state {
            ActionState::Cast(c) => Some((c.t, c.spec.release)),
            _ => None,
        };
        if !precast_over(cast) {
            continue;
        }
        for &e in &fx.effects {
            commands.entity(e).try_despawn();
        }
        if let Some(level) = &fx.sound {
            level.set(0.0);
        }
        info!(
            "spells: the wind-up ends ({} flames out, sound {}) at cast time {}",
            fx.effects.len(),
            if fx.sound.is_some() { "cut" } else { "none" },
            cast.map_or("-".to_string(), |(t, _)| format!("{t:.2} s"))
        );
        commands.entity(me).remove::<CastFx>();
    }
}

/// Where a spell leaves: the held staff's tip, else its root (the grip), else the chest.
fn leaves_from(tip: Option<Vec3>, root: Option<Vec3>, body: Vec3) -> Vec3 {
    tip.or(root).unwrap_or(body + Vec3::Y * 1.4)
}

/// How far (yd) a cast carries the body forward over `dt` seconds ending at `t`: this frame's share of its
/// clip's root motion, as a swing's (the clip's travel is the lunge a staff cast is given).
fn cast_step(clip: &wscl::Clip, t: f32, dt: f32, scale: f32) -> f32 {
    let prev = (t - dt).max(0.0);
    (crate::motion_clip::progress(clip, t) - crate::motion_clip::progress(clip, prev)) * scale
}

/// A cast steps along its clip's root motion (the lunge of a swing or a thrust), except a buff, which
/// stays where it stands. After the body's own actions, as the abilities' rush is.
fn cast_travel(
    time: Res<Time>,
    kit: Option<Res<cast::Kit>>,
    mut avatar: Query<(&Transform, &Action, &mut MoveIntent), With<Avatar>>,
) {
    let (Some(kit), Ok((xf, action, mut intent))) = (kit, avatar.single_mut()) else {
        return;
    };
    let ActionState::Cast(c) = action.state else {
        return;
    };
    if defs::def(c.spell).is_none_or(|d| d.delivery == Delivery::SelfBuff) {
        return;
    }
    let Some(clip) = cast::Role::of_spell(c.spell).and_then(|r| kit.clip(r)) else {
        return;
    };
    let dt = time.delta_secs();
    let step = cast_step(&clip.wow, c.t, dt, xf.scale.y);
    *intent = crate::combat::root_step(*xf.forward(), step, dt);
}

/// The spell leaves the hand: a missile, an instant hit or a buff. Runs in `PostUpdate` after the
/// body's bones are placed, as the arrow's release does, so the missile starts at the hand.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn release(
    mut commands: Commands,
    mut buffs: ResMut<Buffs>,
    mut rng: ResMut<SpellRng>,
    kit: Option<Res<cast::Kit>>,
    fx: Option<Res<fx::Fx>>,
    audio: Option<ResMut<Audio>>,
    mut hits: MessageWriter<Hit>,
    mut spell_hits: MessageWriter<SpellHit>,
    lock: Res<LockOn>,
    avatar: Query<(Entity, &Action, &GlobalTransform, Option<&HeldBuilt>), With<Avatar>>,
    roots: Query<&GlobalTransform>,
    mut foes: Query<
        (&GlobalTransform, Option<&Lockable>, &Hurtbox, &mut Health),
        (With<Hostile>, Without<Avatar>),
    >,
    mut fired: Local<Option<(Entity, u32)>>,
) {
    let Ok((me, action, body, held)) = avatar.single() else {
        return;
    };
    let cast = match action.state {
        ActionState::Cast(cast) => Some(cast),
        _ => None,
    };
    let due = release_due(&mut fired, me, cast.map(|c| (c.serial, c.released())));
    let (true, Some(cast)) = (due, cast) else {
        return;
    };
    let (Some(def), Some(fx)) = (defs::def(cast.spell), fx) else {
        return;
    };
    let mut audio = audio.map(|a| a.into_inner());
    // The spell leaves the staff where it connects: its tip when the model authors one, else the grip.
    let root = held.and_then(|h| roots.get(h.root).ok());
    let tip = held
        .and_then(|h| h.blade)
        .zip(root)
        .map(|([top, _], r)| r.transform_point(top));
    let at = leaves_from(
        tip,
        root.map(GlobalTransform::translation),
        body.translation(),
    );
    let fire = def.school == defs::School::Fire;
    for hand in [0x15, 0x16] {
        let kind = if fire {
            Kind::FireHand(hand)
        } else {
            Kind::IceHand(hand)
        };
        // A flash as the spell leaves the hand, not the model's own length.
        fx::spawn_for(&mut commands, &fx, kind, me, HAND_FLASH);
    }
    fx::sound(
        audio.as_deref_mut(),
        if fire {
            fx::SOUND_FIRE_CAST
        } else {
            fx::SOUND_ICE_CAST
        },
        "spell cast",
    );
    let damage = defs::roll(def, rng.next());
    match def.delivery {
        Delivery::SelfBuff => {
            buffs.frost_armor = def.secs;
            fx::spawn(&mut commands, &fx, Kind::FrostArmor, me);
            fx::sound(audio, fx::SOUND_FROST_ARMOR, "frost armor");
        }
        Delivery::Instant => {
            let Some(target) = cast.target else {
                return;
            };
            let Ok((tf, _, hurt, mut health)) = foes.get_mut(target) else {
                return;
            };
            if health.cur <= 0.0 {
                return;
            }
            health.cur = (health.cur - damage).max(0.0);
            let (a, b) = hurt.axis(tf.translation());
            let point = (a + b) * 0.5;
            // The blow is the spell's own Dark Souls attack row (its flinch, poise and push).
            let (level, poise, knock) = blow_of(kit.as_deref().and_then(|k| k.impact(cast.spell)));
            hits.write(Hit {
                target,
                damage,
                point,
                lethal: health.cur <= 0.0,
                level,
                poise,
                from: body.translation(),
                knock,
            });
            spell_hits.write(SpellHit {
                target,
                spell: cast.spell,
                point,
            });
        }
        Delivery::Missile { speed } => {
            let bullet = cast::Role::of_spell(cast.spell)
                .and_then(|r| kit.as_deref().and_then(|k| k.clip(r)))
                .and_then(|c| c.bullet);
            let homing = bullet.map_or(0.0, |b| b.homing_deg);
            let radius = bullet.map_or(0.2, |b| b.radius * crate::shove::METRES_TO_YARDS);
            // Who it was aimed at when the button was pressed, else who is locked now, if alive.
            let target = cast
                .target
                .or(lock.target)
                .filter(|t| foes.get(*t).is_ok_and(|(_, _, _, h)| h.cur > 0.0));
            let direction = target
                .and_then(|t| foes.get(t).ok())
                .map(|(tf, l, _, _)| {
                    let aim = l.map_or(1.0, |l| l.aim_height);
                    (tf.translation() + Vec3::Y * aim - at).normalize_or(*body.forward())
                })
                .unwrap_or(*body.forward());
            let flight = SpellFlight {
                spell: cast.spell,
                life: def.range / speed,
                target,
                turn: homing * TURN_PER_HOMING_DEG,
                radius,
                impact: kit.as_deref().and_then(|k| k.impact(cast.spell)),
            };
            let missile = commands
                .spawn((
                    Name::new("spell missile"),
                    Projectile::spell(at, direction, speed, damage, body.translation(), flight),
                    Transform::from_translation(at),
                    Visibility::default(),
                ))
                .id();
            let model = if fire {
                Kind::FireballMissile
            } else {
                Kind::FrostboltMissile
            };
            fx::spawn(&mut commands, &fx, model, missile);
            info!(
                "spells: {} released, {damage:.0} damage, homing {homing:.0} deg",
                cast.spell
            );
        }
    }
}

/// A fire's burn on an enemy: `per_tick` damage `ticks_left` more times, one every `every` seconds.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Burning {
    pub per_tick: f32,
    pub ticks_left: u32,
    pub every: f32,
    pub timer: f32,
}

/// `dt` seconds pass: the damage of a tick that fell due (0 when none did).
pub fn burn_tick(b: &mut Burning, dt: f32) -> f32 {
    if b.ticks_left == 0 {
        return 0.0;
    }
    b.timer -= dt;
    if b.timer > 0.0 {
        return 0.0;
    }
    b.timer += b.every;
    b.ticks_left -= 1;
    b.per_tick
}

/// Burns tick. A tick that does not kill is silent (no flinch, no number); the one that does carries a
/// [`Hit`] so the kill pays out (XP, souls, loot) like any other.
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    avatar: Query<&GlobalTransform, With<Avatar>>,
    mut burning: Query<(Entity, &mut Burning, &mut Health, &GlobalTransform), With<Hostile>>,
) {
    let from = avatar
        .single()
        .map_or(Vec3::ZERO, GlobalTransform::translation);
    for (e, mut b, mut health, at) in &mut burning {
        let damage = burn_tick(&mut b, time.delta_secs());
        if health.cur > 0.0 && damage > 0.0 {
            health.cur = (health.cur - damage).max(0.0);
            if health.cur <= 0.0 {
                hits.write(Hit {
                    target: e,
                    damage,
                    point: at.translation() + Vec3::Y * 0.9,
                    lethal: true,
                    level: 0,
                    poise: 0.0,
                    from,
                    knock: 0.0,
                });
            }
        }
        if b.ticks_left == 0 || health.cur <= 0.0 {
            commands.entity(e).remove::<Burning>();
        }
    }
}

/// The armor the buffs give now: Frost Armor's while it lasts.
pub fn armor_buff(buffs: &Buffs) -> u32 {
    if buffs.frost_armor > 0.0 {
        defs::def(168).map_or(0, |d| d.armor)
    } else {
        0
    }
}

/// The armor buff follows its seconds into the gear effect the damage hooks read.
fn frost_armor(buffs: Res<Buffs>, mut gear: ResMut<crate::gear_effect::GearFx>) {
    let want = armor_buff(&buffs);
    if gear.0.buff_armor != want {
        gear.0.buff_armor = want;
        info!(
            "spells: frost armor {} ({} armor from the buff)",
            if want > 0 { "on" } else { "off" },
            want
        );
    }
}

/// What a missile's hit leaves: the impact model and sound on the target, the slow that holds it back
/// and the burn that eats at it.
#[allow(clippy::too_many_arguments)]
fn spell_hit(
    mut commands: Commands,
    mut hits: MessageReader<SpellHit>,
    fx: Option<Res<fx::Fx>>,
    audio: Option<ResMut<Audio>>,
    foes: Query<&Health, With<Hostile>>,
) {
    let Some(fx) = fx else {
        hits.clear();
        return;
    };
    let mut audio = audio.map(|a| a.into_inner());
    for h in hits.read() {
        let Some(def) = defs::def(h.spell) else {
            continue;
        };
        info!("spells: {} struck {} at {:.1?}", h.spell, h.target, h.point);
        let fire = def.school == defs::School::Fire;
        let kind = match (def.spell, fire) {
            (2136, _) => Kind::FireBlastImpact,
            (_, true) => Kind::FireImpact,
            (_, false) => Kind::IceImpact,
        };
        fx::spawn(&mut commands, &fx, kind, h.target);
        fx::sound(
            audio.as_deref_mut(),
            if fire {
                fx::SOUND_FIRE_IMPACT
            } else {
                fx::SOUND_ICE_IMPACT
            },
            "spell impact",
        );
        let alive = foes.get(h.target).is_ok_and(|hp| hp.cur > 0.0);
        if !alive {
            continue;
        }
        if let Some(slow) = def.slow {
            commands.entity(h.target).insert(crate::abilities::Slowed {
                left: slow.secs,
                factor: 1.0 - slow.share,
            });
            fx::spawn(&mut commands, &fx, Kind::IceMark, h.target);
            info!(
                "spells: {} slowed {:.0} % for {:.0} s",
                h.target,
                slow.share * 100.0,
                slow.secs
            );
        }
        if let Some(dot) = def.dot {
            commands.entity(h.target).insert(Burning {
                per_tick: dot.per_tick,
                ticks_left: dot.ticks,
                every: dot.every,
                timer: dot.every,
            });
            info!(
                "spells: {} burns for {} x {:.0} every {:.0} s",
                h.target, dot.ticks, dot.per_tick, dot.every
            );
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SpellRng>()
        .add_systems(Startup, cast::load_kit)
        .add_systems(
            PostUpdate,
            release.after(benilla_world::billboard::BillboardPlace),
        )
        .add_systems(
            Update,
            (spell_hit, burn, frost_armor, end_precast).run_if(in_state(Phase::Playing)),
        )
        .add_systems(
            Update,
            cast_travel
                .in_set(crate::controller::PlaySet::Intent)
                .after(crate::combat::act)
                .run_if(in_state(Phase::Playing)),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::spells;

    fn timing() -> cast::Timing {
        cast::Timing {
            duration: 1.5,
            release: 0.767,
            recover: 1.233,
        }
    }
    fn foe(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn the_precast_ends_with_the_release_or_with_whatever_replaces_the_cast() {
        assert!(!precast_over(Some((0.2, 0.6))), "still winding up");
        assert!(precast_over(Some((0.6, 0.6))), "released");
        assert!(precast_over(Some((0.9, 0.6))), "released a while ago");
        assert!(precast_over(None), "no cast: a stagger, a roll, a death");
    }

    #[test]
    fn a_missile_leaves_the_staffs_tip_before_its_root_before_the_chest() {
        let (tip, root, body) = (
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(-1.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
        );
        assert_eq!(leaves_from(Some(tip), Some(root), body), tip);
        assert_eq!(leaves_from(None, Some(root), body), root);
        assert_eq!(leaves_from(None, None, body), body + Vec3::Y * 1.4);
    }

    #[test]
    fn a_cast_steps_forward_by_its_clips_root_motion() {
        let clip = wscl::Clip {
            name: "c".into(),
            fps: 2.0,
            model_space: false,
            parents: vec![],
            driven: vec![],
            frames: vec![],
            root_motion: vec![[0.0; 4], [0.0, 0.0, -1.0, 0.0], [0.0, 0.0, -3.0, 0.0]],
            flags: vec![],
            hits: vec![],
        };
        // 0.5 s in, 0.5 s per frame: the second frame's distance, 1 yd.
        assert_eq!(cast_step(&clip, 0.5, 0.5, 1.0), 1.0);
        // The next half second covers 2 yd; a body scaled by 1.5 covers 1.5 times that.
        assert_eq!(cast_step(&clip, 1.0, 0.5, 1.0), 2.0);
        assert_eq!(cast_step(&clip, 1.0, 0.5, 1.5), 3.0);
        assert_eq!(cast_step(&clip, 0.0, 0.016, 1.0), 0.0, "not yet moving");
    }

    #[test]
    fn a_missile_goes_at_the_locked_enemy_or_at_nothing() {
        let f = spells::def(133).unwrap();
        let p = plan(
            f,
            true,
            true,
            Some(timing()),
            Vec3::ZERO,
            Vec3::NEG_Z,
            Some(foe(3)),
            &[],
        )
        .unwrap();
        assert_eq!(p.target, Some(foe(3)));
        assert_eq!(
            p.spec,
            CastSpec {
                duration: 1.5,
                release: 0.767,
                recover: 1.233
            }
        );
        let p = plan(
            f,
            true,
            true,
            Some(timing()),
            Vec3::ZERO,
            Vec3::NEG_Z,
            None,
            &[],
        )
        .unwrap();
        assert_eq!(p.target, None, "it flies ahead");
    }

    #[test]
    fn without_cast_clips_or_a_free_body_the_cast_is_refused() {
        let f = spells::def(133).unwrap();
        assert_eq!(
            plan(f, true, true, None, Vec3::ZERO, Vec3::NEG_Z, None, &[]),
            Err(Refusal::NotYet),
            "no Dark Souls cache"
        );
        assert_eq!(
            plan(
                f,
                false,
                true,
                Some(timing()),
                Vec3::ZERO,
                Vec3::NEG_Z,
                None,
                &[]
            ),
            Err(Refusal::Busy)
        );
        assert_eq!(
            plan(
                f,
                true,
                false,
                Some(timing()),
                Vec3::ZERO,
                Vec3::NEG_Z,
                None,
                &[]
            ),
            Err(Refusal::Busy),
            "not in the air"
        );
    }

    #[test]
    fn an_instant_spell_needs_a_target_in_range_before_it_is_paid_for() {
        let f = spells::def(2136).unwrap();
        let t = Some(timing());
        let near = [(foe(1), Vec3::new(0.0, 0.0, -10.0))];
        let far = [(foe(2), Vec3::new(0.0, 0.0, -30.0))];
        let ok = plan(f, true, true, t, Vec3::ZERO, Vec3::NEG_Z, None, &near).unwrap();
        assert_eq!(ok.target, Some(foe(1)), "the nearest enemy ahead");
        assert_eq!(
            plan(f, true, true, t, Vec3::ZERO, Vec3::NEG_Z, None, &far),
            Err(Refusal::TooFar)
        );
        assert_eq!(
            plan(f, true, true, t, Vec3::ZERO, Vec3::NEG_Z, None, &[]),
            Err(Refusal::NoTarget)
        );
        // Behind the caster is not ahead.
        let behind = [(foe(4), Vec3::new(0.0, 0.0, 5.0))];
        assert_eq!(
            plan(f, true, true, t, Vec3::ZERO, Vec3::NEG_Z, None, &behind),
            Err(Refusal::NoTarget)
        );
        // The locked enemy counts wherever it is, if it is in range.
        let ok = plan(
            f,
            true,
            true,
            t,
            Vec3::ZERO,
            Vec3::NEG_Z,
            Some(foe(4)),
            &behind,
        )
        .unwrap();
        assert_eq!(ok.target, Some(foe(4)));
        // ...and out of range it is refused even when locked.
        assert_eq!(
            plan(
                f,
                true,
                true,
                t,
                Vec3::ZERO,
                Vec3::NEG_Z,
                Some(foe(2)),
                &far
            ),
            Err(Refusal::TooFar)
        );
    }

    #[test]
    fn a_buff_needs_no_target() {
        let f = spells::def(168).unwrap();
        let p = plan(
            f,
            true,
            true,
            Some(timing()),
            Vec3::ZERO,
            Vec3::NEG_Z,
            None,
            &[],
        )
        .unwrap();
        assert_eq!(p.target, None);
    }

    #[test]
    fn a_burn_ticks_every_two_seconds_twice_and_ends() {
        let mut b = Burning {
            per_tick: 1.0,
            ticks_left: 2,
            every: 2.0,
            timer: 2.0,
        };
        assert_eq!(burn_tick(&mut b, 1.9), 0.0);
        assert_eq!(burn_tick(&mut b, 0.2), 1.0, "the first tick at 2 s");
        assert_eq!(burn_tick(&mut b, 2.0), 1.0, "the second at 4 s");
        assert_eq!(b.ticks_left, 0);
        assert_eq!(burn_tick(&mut b, 5.0), 0.0, "it is over");
    }

    #[test]
    fn frost_armor_gives_its_armor_only_while_it_lasts() {
        assert_eq!(armor_buff(&Buffs::default()), 0);
        let on = Buffs {
            frost_armor: 1799.0,
            ..Buffs::default()
        };
        assert_eq!(armor_buff(&on), 30);
        let shout_only = Buffs {
            shout: 50.0,
            ..Buffs::default()
        };
        assert_eq!(armor_buff(&shout_only), 0, "a shout is no armor");
    }

    #[test]
    fn the_damage_roll_stream_stays_in_the_unit_interval() {
        let mut rng = SpellRng::default();
        for _ in 0..1000 {
            let u = rng.next();
            assert!((0.0..1.0).contains(&u), "{u}");
        }
    }
}
