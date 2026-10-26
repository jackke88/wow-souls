//! Warrior mastery is earned at actual contacts. No whiff healing, automatic counters or proc loops.
use bevy::prelude::*;
use wow_world::talents::TalentId;

use crate::avatar::Avatar;
use crate::combat::{Action, ActionState, Health, MoveKind, Stamina, Swing, SwingSpec};
use crate::defense::{PerfectDefense, PerfectKind, Training};
use crate::talents::Talents;

pub const MORTAL_STRIKE: u32 = 12294;
pub const BLOODTHIRST: u32 = 23881;
pub const SHIELD_WALL: u32 = 871;

#[derive(Debug, Clone, Copy)]
pub struct Counter {
    target: Entity,
    until: f32,
    damage: f32,
    poise: f32,
}

#[derive(Debug, Default, Resource)]
pub struct Warrior {
    pub wall: f32,
    pub signature: Option<(u32, u32)>, // (swing serial, spell)
    counter: Option<Counter>,
    refunded: Option<u32>,
    healed: Option<u32>,
    signature_healed: Option<u32>,
    build: String,
    class: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct Contact {
    pub damage: f32,
    pub poise: f32,
    pub bleed: f32,
    pub signature: bool,
}

impl Warrior {
    /// A weaker defense never erases a stronger counter that the player has already earned.
    fn offer_counter(&mut self, counter: Counter, now: f32) {
        if counter.damage <= 0.0 && counter.poise <= 0.0 {
            return;
        }
        if self
            .counter
            .is_some_and(|old| now < old.until && old.damage > counter.damage)
        {
            return;
        }
        self.counter = Some(counter);
    }

    /// Consume the counter only on its attacker; an unrelated target cannot waste or steal it.
    pub fn contact(
        &mut self,
        talents: &Talents,
        swing: Swing,
        target: Entity,
        now: f32,
        rage: f32,
        health_fraction: f32,
    ) -> Contact {
        let rank = |id| f32::from(talents.rank(id));
        let heavy = matches!(swing.kind, MoveKind::R2(_));
        let chain = matches!(swing.kind, MoveKind::R1(n) | MoveKind::R2(n) if n > 0);
        let mut damage = 1.0;
        let mut poise = 1.0;
        if heavy {
            damage += 0.03 * rank(TalentId::Impale);
            poise += 0.08 * rank(TalentId::HeavyHand);
        }
        if chain {
            damage += 0.02 * rank(TalentId::Cruelty);
        }
        if rage >= 50.0 {
            damage += 0.02 * rank(TalentId::BattleTrance);
        }
        if health_fraction <= 0.5 {
            damage += 0.02 * rank(TalentId::Berserker);
        }
        if let Some(c) = self.counter.filter(|c| c.target == target && now < c.until) {
            damage += c.damage;
            poise += c.poise;
            self.counter = None;
        }
        let signature = self.signature.filter(|&(serial, _)| serial == swing.serial);
        if let Some((_, spell)) = signature {
            damage += if spell == MORTAL_STRIKE { 0.5 } else { 0.2 };
            if spell == MORTAL_STRIKE {
                poise += 0.5;
            }
        }
        Contact {
            damage: damage.min(2.25),
            poise: poise.min(2.5),
            bleed: if heavy {
                0.05 * rank(TalentId::Bloodletting)
            } else {
                0.0
            },
            signature: signature.is_some(),
        }
    }

    /// One stamina refund and one kill heal per swing, regardless of crowd size or hit windows.
    pub fn landed(
        &mut self,
        talents: &Talents,
        serial: u32,
        killed: bool,
        hp: &mut Health,
        stamina: &mut Stamina,
    ) {
        if self.refunded != Some(serial) {
            stamina.cur =
                (stamina.cur + f32::from(talents.rank(TalentId::Flurry))).min(stamina.max);
            self.refunded = Some(serial);
        }
        if killed && self.healed != Some(serial) {
            hp.cur = (hp.cur + hp.max * 0.005 * f32::from(talents.rank(TalentId::BloodCraze)))
                .min(hp.max);
            self.healed = Some(serial);
        }
        if self.signature == Some((serial, BLOODTHIRST)) && self.signature_healed != Some(serial) {
            hp.cur = (hp.cur + hp.max * 0.05).min(hp.max);
            self.signature_healed = Some(serial);
        }
    }
}

/// Plan first, pay later: signatures require a grounded, free body and a supported melee motion.
pub fn signature_plan(
    spell: u32,
    action: &Action,
    grounded: bool,
    moves: Option<&crate::motion_clip::WeaponClips>,
    stamina: &Stamina,
    shield: bool,
) -> Result<Option<(MoveKind, SwingSpec)>, crate::abilities::Refusal> {
    use crate::abilities::Refusal;
    if !grounded || !action.can_cast() {
        return Err(Refusal::Busy);
    }
    if spell == SHIELD_WALL {
        return if shield {
            Ok(None)
        } else {
            Err(Refusal::ShieldRequired)
        };
    }
    let moves = moves
        .filter(|m| m.projectile.is_none())
        .ok_or(Refusal::NotYet)?;
    let kind = match spell {
        MORTAL_STRIKE => MoveKind::R2(0),
        BLOODTHIRST => MoveKind::R1(0),
        _ => return Err(Refusal::NotYet),
    };
    let spec = moves.moveset.spec(kind).ok_or(Refusal::NotYet)?;
    if stamina.cur < spec.cost {
        return Err(Refusal::NotEnoughStamina);
    }
    Ok(Some((kind, spec)))
}

#[derive(Debug, Component)]
pub struct Bleed {
    pub left: f32,
    pub damage_left: f32,
    pub tick: f32,
    pub from: Vec3,
}

/// Refreshing replaces the old bleed. Its pulses use normal reward/death messages, with no flinch or rage.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn bleed(
    mut commands: Commands,
    time: Res<Time>,
    mut rage: ResMut<crate::abilities::Rage>,
    mut hits: MessageWriter<crate::hits::Hit>,
    talents: Res<Talents>,
    avatar: Query<&Health, With<Avatar>>,
    mut foes: Query<(Entity, &GlobalTransform, &mut Health, &mut Bleed), Without<Avatar>>,
) {
    for (e, at, mut hp, mut b) in &mut foes {
        if hp.cur <= 0.0
            || talents.rank(TalentId::Bloodletting) == 0
            || avatar.single().is_ok_and(|h| h.cur <= 0.0)
        {
            commands.entity(e).remove::<Bleed>();
            continue;
        }
        let dt = time.delta_secs().min(b.left);
        b.left = (b.left - dt).max(0.0);
        b.tick += dt;
        if b.tick >= 1.0 || b.left <= 0.0 {
            let damage = if b.left <= 0.0 {
                b.damage_left
            } else {
                b.damage_left * b.tick / (b.left + b.tick)
            }
            .max(0.0);
            b.damage_left = (b.damage_left - damage).max(0.0);
            b.tick = 0.0;
            hp.cur = (hp.cur - damage).max(0.0);
            rage.ability_damage += damage;
            hits.write(crate::hits::Hit {
                target: e,
                damage,
                point: at.translation() + Vec3::Y,
                lethal: hp.cur <= 0.0,
                level: 0,
                poise: 0.0,
                from: b.from,
                knock: 0.0,
            });
        }
        if b.left <= 0.0 {
            commands.entity(e).remove::<Bleed>();
        }
    }
}

/// Clear rewards on death/class/build changes, and provide defense with bounded current modifiers.
fn prepare(
    time: Res<Time>,
    talents: Res<Talents>,
    active: Res<crate::abilities::ActiveClass>,
    mut warrior: ResMut<Warrior>,
    mut avatar: Query<(&mut Action, &Health), With<Avatar>>,
) {
    let Ok((mut action, hp)) = avatar.single_mut() else {
        return;
    };
    let build = talents.build.text();
    if hp.cur <= 0.0 || warrior.class != active.id || warrior.build != build {
        *warrior = Warrior {
            build,
            class: active.id,
            ..default()
        };
    }
    warrior.wall = (warrior.wall - time.delta_secs()).max(0.0);
    if warrior
        .counter
        .is_some_and(|c| time.elapsed_secs() >= c.until)
    {
        warrior.counter = None;
    }
    if warrior.signature.is_some_and(
        |(serial, _)| !matches!(action.state, ActionState::Attack(s) if s.serial == serial),
    ) {
        warrior.signature = None;
    }
    action.training = Training {
        block_cost: 1.0 - 0.04 * f32::from(talents.rank(TalentId::ShieldSpecialization)),
        block_chip: 1.0 - 0.08 * f32::from(talents.rank(TalentId::Bulwark)),
        parry_discount: 0.8 * f32::from(talents.rank(TalentId::Deflection)),
        damage_taken: if warrior.wall > 0.0 { 0.6 } else { 1.0 },
    };
    if hp.cur <= 0.0 {
        action.guard_timing = default();
    }
}

#[allow(clippy::too_many_arguments)]
fn perfect_rewards(
    time: Res<Time>,
    real: Res<Time<Real>>,
    mut events: MessageReader<PerfectDefense>,
    talents: Res<Talents>,
    active: Res<crate::abilities::ActiveClass>,
    mut warrior: ResMut<Warrior>,
    mut rage: ResMut<crate::abilities::Rage>,
    mut toasts: ResMut<crate::quests::Toasts>,
    mut stop: ResMut<crate::hits::HitStop>,
    mut virt: ResMut<Time<Virtual>>,
    mut avatar: Query<(&Health, &mut Stamina), With<Avatar>>,
    mut points: Local<Vec<(Vec3, f32, PerfectKind)>>,
    mut gizmos: Gizmos,
) {
    let now = time.elapsed_secs();
    let real_now = real.elapsed_secs();
    let Ok((hp, mut stamina)) = avatar.single_mut() else {
        events.clear();
        return;
    };
    for event in events.read() {
        if hp.cur <= 0.0 {
            continue;
        }
        let parry = event.kind == PerfectKind::Parry;
        toasts.push(
            real_now,
            if parry {
                "Perfect parry — counter!"
            } else {
                "Perfect block"
            },
        );
        stop.freeze(&mut virt, real_now, if parry { 0.065 } else { 0.045 });
        points.push((event.point, real_now, event.kind));
        let rank = |id| f32::from(talents.rank(id));
        let trained = active.id == 1;
        if trained {
            rage.cur = (rage.cur
                + if parry {
                    8.0
                } else {
                    4.0 + rank(TalentId::ShieldDiscipline)
                })
            .min(100.0);
            rage.quiet = 0.0;
            if parry {
                stamina.cur = (stamina.cur + 2.0 * rank(TalentId::SecondWind)).min(stamina.max);
            }
        }
        warrior.offer_counter(
            Counter {
                target: event.attacker,
                until: now + 3.0,
                damage: if parry { 0.25 } else { 0.0 }
                    + if trained {
                        0.04 * rank(TalentId::Revenge)
                    } else {
                        0.0
                    },
                poise: if trained {
                    0.08 * rank(TalentId::Concussion)
                } else {
                    0.0
                },
            },
            now,
        );
        info!(
            "defense: PERFECT {:?} attacker={} rage={:.0}",
            event.kind, event.attacker, rage.cur
        );
    }
    points.retain(|(_, at, _)| real_now - at < 0.22);
    for &(p, at, kind) in points.iter() {
        let age = (real_now - at) / 0.22;
        let color = if kind == PerfectKind::Parry {
            Color::srgba(0.65, 0.85, 1.0, 1.0 - age)
        } else {
            Color::srgba(1.0, 0.85, 0.3, 1.0 - age)
        };
        for i in 0..12 {
            let a = i as f32 * std::f32::consts::TAU / 12.0;
            let dir = Vec3::new(a.cos(), a.sin(), 0.3).normalize();
            gizmos.line(p + dir * age * 0.2, p + dir * (0.1 + age * 0.6), color);
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Warrior>()
        .add_systems(
            Update,
            prepare
                .before(crate::combat::act)
                .run_if(in_state(crate::settle::Phase::Playing)),
        )
        .add_systems(
            Update,
            (perfect_rewards, bleed)
                .after(crate::combat::act)
                .run_if(in_state(crate::settle::Phase::Playing)),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_weaker_block_reward_does_not_erase_a_live_perfect_parry_counter() {
        let attacker = Entity::from_bits(42);
        let other = Entity::from_bits(43);
        let mut state = Warrior::default();
        state.offer_counter(
            Counter {
                target: attacker,
                until: 3.0,
                damage: 0.25,
                poise: 0.0,
            },
            0.0,
        );
        state.offer_counter(
            Counter {
                target: other,
                until: 4.0,
                damage: 0.0,
                poise: 0.0,
            },
            1.0,
        );
        assert_eq!(state.counter.unwrap().target, attacker);
        state.offer_counter(
            Counter {
                target: other,
                until: 4.0,
                damage: 0.20,
                poise: 0.4,
            },
            1.0,
        );
        assert_eq!(state.counter.unwrap().target, attacker);
        state.offer_counter(
            Counter {
                target: other,
                until: 7.0,
                damage: 0.20,
                poise: 0.4,
            },
            4.0,
        );
        assert_eq!(
            state.counter.unwrap().target,
            other,
            "an expired counter can be replaced"
        );
    }

    #[test]
    fn fury_cruelty_rewards_light_and_heavy_followups() {
        let talents = trained(&[TalentId::UnbridledWrath, TalentId::Cruelty]);
        let mut state = Warrior::default();
        for kind in [MoveKind::R1(1), MoveKind::R2(1)] {
            let swing = Swing {
                t: 0.4,
                kind,
                spec: SwingSpec::procedural(1.0),
                reference: false,
                serial: 1,
            };
            assert!(
                (state
                    .contact(&talents, swing, Entity::from_bits(42), 1.0, 0.0, 1.0)
                    .damage
                    - 1.1)
                    .abs()
                    < 0.001
            );
        }
    }

    fn trained(ids: &[TalentId]) -> Talents {
        let mut build = wow_world::talents::Build::default();
        for &id in ids {
            for _ in 0..5 {
                build.spend(id, 60).unwrap();
            }
        }
        Talents::for_build(build)
    }

    #[test]
    fn arms_heavies_gain_damage_poise_and_bleed_without_changing_light_attacks() {
        let talents = trained(&[
            TalentId::TacticalMastery,
            TalentId::ImprovedCharge,
            TalentId::HeavyHand,
            TalentId::Impale,
            TalentId::SecondWind,
            TalentId::Bloodletting,
        ]);
        let mut state = Warrior::default();
        let mut swing = Swing {
            t: 0.4,
            kind: MoveKind::R2(0),
            spec: SwingSpec::procedural(1.0),
            reference: false,
            serial: 1,
        };
        let contact = state.contact(&talents, swing, Entity::from_bits(42), 1.0, 0.0, 1.0);
        assert!((contact.damage - 1.15).abs() < 0.001);
        assert!((contact.poise - 1.4).abs() < 0.001);
        assert_eq!(contact.bleed, 0.25);
        swing.kind = MoveKind::R1(0);
        let contact = state.contact(&talents, swing, Entity::from_bits(42), 1.0, 0.0, 1.0);
        assert_eq!(contact.damage, 1.0);
        assert_eq!(contact.poise, 1.0);
        assert_eq!(contact.bleed, 0.0);
    }

    #[test]
    fn fury_refunds_once_per_swing_and_melee_kill_heals_once_across_a_crowd() {
        let talents = trained(&[
            TalentId::UnbridledWrath,
            TalentId::Flurry,
            TalentId::Cruelty,
            TalentId::BloodCraze,
        ]);
        let mut state = Warrior::default();
        let mut hp = Health::full(100.0);
        hp.cur = 50.0;
        let mut stamina = Stamina::full(100.0);
        stamina.cur = 40.0;
        state.landed(&talents, 1, false, &mut hp, &mut stamina);
        assert_eq!(stamina.cur, 45.0);
        assert_eq!(hp.cur, 50.0);
        state.landed(&talents, 1, true, &mut hp, &mut stamina);
        state.landed(&talents, 1, true, &mut hp, &mut stamina);
        assert_eq!(stamina.cur, 45.0);
        assert_eq!(hp.cur, 52.5);
        state.landed(&talents, 2, true, &mut hp, &mut stamina);
        assert_eq!(stamina.cur, 50.0);
        assert_eq!(hp.cur, 55.0);
    }

    #[test]
    fn signature_refusals_leave_resources_and_actions_untouched() {
        let action = Action::default();
        let stamina = Stamina::full(100.0);
        assert_eq!(
            signature_plan(SHIELD_WALL, &action, true, None, &stamina, false),
            Err(crate::abilities::Refusal::ShieldRequired)
        );
        assert!(
            signature_plan(SHIELD_WALL, &action, true, None, &stamina, true)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            signature_plan(MORTAL_STRIKE, &action, false, None, &stamina, true),
            Err(crate::abilities::Refusal::Busy)
        );
        assert_eq!(
            signature_plan(MORTAL_STRIKE, &action, true, None, &stamina, true),
            Err(crate::abilities::Refusal::NotYet)
        );
        assert_eq!(stamina.cur, 100.0);
        assert_eq!(action.state, ActionState::Idle);
    }

    #[test]
    fn signature_requires_the_full_stamina_price_before_any_payment() {
        let dir = crate::motion_clip::cache_dir();
        let manifest = crate::motion_clip::checked_manifest(&dir).unwrap();
        let moves = crate::motion_clip::load_moves(&dir, &manifest, 119).unwrap();
        let action = Action::default();
        let mut stamina = Stamina::full(100.0);
        let plan = signature_plan(MORTAL_STRIKE, &action, true, Some(&moves), &stamina, true)
            .unwrap()
            .unwrap();
        assert!(plan.1.cost > 0.0);
        stamina.cur = plan.1.cost - 0.01;
        assert_eq!(
            signature_plan(MORTAL_STRIKE, &action, true, Some(&moves), &stamina, true),
            Err(crate::abilities::Refusal::NotEnoughStamina)
        );
        assert_eq!(stamina.cur, plan.1.cost - 0.01);
    }
    #[test]
    fn counter_is_target_bound_expires_and_is_spent_only_on_contact() {
        let target = Entity::from_bits(42);
        let other = Entity::from_bits(43);
        let mut warrior = Warrior {
            counter: Some(Counter {
                target,
                until: 3.0,
                damage: 0.25,
                poise: 0.0,
            }),
            ..default()
        };
        let talents = Talents::default();
        let swing = Swing {
            t: 0.4,
            kind: MoveKind::R1(0),
            spec: SwingSpec::procedural(1.0),
            reference: false,
            serial: 1,
        };
        assert_eq!(
            warrior
                .contact(&talents, swing, other, 1.0, 0.0, 1.0)
                .damage,
            1.0
        );
        assert_eq!(
            warrior
                .contact(&talents, swing, target, 1.0, 0.0, 1.0)
                .damage,
            1.25
        );
        assert_eq!(
            warrior
                .contact(&talents, swing, target, 1.0, 0.0, 1.0)
                .damage,
            1.0
        );
        warrior.counter = Some(Counter {
            target,
            until: 3.0,
            damage: 0.25,
            poise: 0.0,
        });
        assert_eq!(
            warrior
                .contact(&talents, swing, target, 3.0, 0.0, 1.0)
                .damage,
            1.0
        );
    }

    #[test]
    fn bloodthirst_heals_only_once_per_landed_swing_and_never_above_max() {
        let talents = Talents::default();
        let mut warrior = Warrior {
            signature: Some((7, BLOODTHIRST)),
            ..default()
        };
        let mut hp = Health::full(100.0);
        hp.cur = 50.0;
        let mut stamina = Stamina::full(100.0);
        warrior.landed(&talents, 7, false, &mut hp, &mut stamina);
        assert_eq!(hp.cur, 55.0);
        warrior.landed(&talents, 7, true, &mut hp, &mut stamina);
        assert_eq!(hp.cur, 55.0);
        warrior.signature = Some((8, BLOODTHIRST));
        hp.cur = 99.0;
        warrior.landed(&talents, 8, false, &mut hp, &mut stamina);
        assert_eq!(hp.cur, 100.0);
    }
}
