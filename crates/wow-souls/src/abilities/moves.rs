//! What the abilities do in the world.
//!
//! **Charge**: the warrior rushes at an enemy (the locked one, else the nearest ahead), 5 to 25 yd away, at
//! 24 yd/s through the same motor he walks with, stops at arm's length, gains 9 rage and the enemy is stunned
//! for 1.5 s (it stands dazed, nothing it was doing goes on). An attack, a roll, a blow taken or an open screen
//! ends the rush. **Thunder Clap**: every enemy within 5 yd takes 30 damage (with the shout's bonus), is
//! knocked a little and moves 40 % slower for 10 s (and rests 40 % longer between swings). **Battle Shout** is
//! the buff of [`super::Buffs`]. The shout and the clap play their WoW cast animation (`Casting`): the body is
//! the animation's for its length, a blow or a roll cuts it short.
//!
//! The pure parts ([`pick_target`], [`rush_step`], [`clap_victims`]) are tested; the systems drive them.

use std::time::Duration;

use benilla_assets::ModelAnimations;
use bevy::animation::transition::AnimationTransitions;
use bevy::animation::RepeatAnimation;
use bevy::prelude::*;
use bevy::time::Real;

use wow_world::abilities::Effect;

use super::{fx, Buffs, Rage, Refusal};
use crate::audio::Audio;
use crate::avatar::Avatar;
use crate::combat::{Action, Health};
use crate::controller::yaw_of;
use crate::hits::{Hit, Hurtbox};
use crate::loot::Screen;
use crate::monster::MonsterPlay;
use crate::motor::MoveIntent;
use crate::npc::{Brain, Hostile, Mind, Reaction, COMBAT_WOUND};
use crate::quests::Toasts;
use crate::shake::Shake;

/// A charge needs its enemy at least this far and at most this far (yd, ground distance).
pub const CHARGE_MIN: f32 = 5.0;
pub const CHARGE_MAX: f32 = 25.0;
/// An enemy this far off the way ahead (cosine of the angle: 60 degrees) is not the one charged.
const CHARGE_CONE: f32 = 0.5;
/// How fast the rush goes (yd/s), and how far the warrior stops from the enemy's body (yd).
pub const CHARGE_SPEED: f32 = 24.0;
const CHARGE_STOP: f32 = 1.0;
/// The enemy stands stunned this long (s); the warrior gains this much rage.
pub const CHARGE_STUN: f32 = 1.5;
pub const CHARGE_RAGE: f32 = 9.0;
/// A rush that has not got closer by a quarter of its speed for this long (s) is stuck behind something.
const STALL_AFTER: f32 = 0.3;

/// Thunder Clap: how far it blasts (yd, to the enemy's body), what it does, how long it slows.
pub const CLAP_RADIUS: f32 = 5.0;
pub const CLAP_DAMAGE: f32 = 30.0;
pub const CLAP_SLOW_SECS: f32 = 10.0;
/// A slowed enemy moves at this share of its speed and rests this much longer between swings.
pub const CLAP_SLOW: f32 = 0.6;
/// The blast's flinch (level 1, a poise hit) and push (Dark Souls metres), and the camera's trauma.
const CLAP_POISE: f32 = 12.0;
const CLAP_KNOCK: f32 = 0.5;
const CLAP_SHAKE: f32 = 0.4;

/// The cooldown of an ability (s): Charge 12, Thunder Clap 5, the shout none.
pub fn cooldown(effect: Effect) -> f32 {
    match effect {
        Effect::Charge => 12.0,
        Effect::ThunderClap => 5.0,
        Effect::BattleShout => 0.0,
    }
}

/// The cast animations of the human model (`AnimationData`): BattleRoar and the clap's own, and the
/// Stun a creature plays while stunned.
pub const ANIM_ROAR: u16 = 55;
pub const ANIM_CLAP: u16 = 181;
const ANIM_STUN: u16 = 14;
/// The animations' lengths on the human male (s).
pub const ROAR_SECS: f32 = 1.0;
pub const CLAP_SECS: f32 = 0.5;

/// A charge under way: the enemy, and how long it has not got closer.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct Charging(pub Option<Rush>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rush {
    pub target: Entity,
    stalled: f32,
    last_gap: f32,
}

impl Rush {
    pub fn at(target: Entity) -> Rush {
        Rush {
            target,
            stalled: 0.0,
            last_gap: f32::INFINITY,
        }
    }
}

/// The warrior is playing a cast animation: movement and the gait wait for it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Casting {
    pub clip: u16,
    pub left: f32,
}

/// An enemy the clap slowed.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Slowed {
    pub left: f32,
}

/// Thunder Clap was cast: the blast happens in [`thunder_clap`].
#[derive(Message, Debug, Clone, Copy)]
pub struct ThunderClap;

/// A creature is stunned for `secs`.
#[derive(Message, Debug, Clone, Copy)]
pub struct Stun {
    pub target: Entity,
    pub secs: f32,
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// What a charge runs at: the locked enemy, else the nearest ahead of `facing` that is in range. Why not,
/// when none: nobody ahead, or the nearest one is too close or too far.
pub fn pick_target(
    player: Vec3,
    facing: Vec3,
    locked: Option<Entity>,
    foes: &[(Entity, Vec3)],
) -> Result<Entity, Refusal> {
    let reach = |p: Vec3| flat(p - player).length();
    let judge = |d: f32| {
        if d < CHARGE_MIN {
            Err(Refusal::TooClose)
        } else if d > CHARGE_MAX {
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
        .filter(|(_, p)| flat(*p - player).normalize_or_zero().dot(nose) >= CHARGE_CONE)
        .map(|(e, p)| (*e, reach(*p)))
        .collect();
    ahead.sort_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((e, _)) = ahead.iter().find(|(_, d)| judge(*d).is_ok()) {
        return Ok(*e);
    }
    match ahead.first() {
        Some((_, d)) => Err(judge(*d).err().unwrap_or(Refusal::TooFar)),
        None => Err(Refusal::NoTarget),
    }
}

/// The rage a warrior holds after a charge lands: nine more than before, up to the most there is.
pub fn rage_after_charge(cur: f32) -> f32 {
    (cur + CHARGE_RAGE).min(wow_world::abilities::RAGE_MAX as f32)
}

/// One frame of the rush: the speed that closes `gap` (yd still to go to the stopping point) without
/// overshooting it in `dt`, and whether the warrior is there.
pub fn rush_step(gap: f32, dt: f32) -> (f32, bool) {
    if gap <= 0.0 {
        return (0.0, true);
    }
    let speed = CHARGE_SPEED.min(gap / dt.max(1.0e-3));
    (speed, speed * dt >= gap - 1.0e-3)
}

/// The enemies a clap at `player` reaches: those whose body (`radius`) is within [`CLAP_RADIUS`] along the
/// ground and no more than 3 yd above or below.
pub fn clap_victims(player: Vec3, foes: &[(Entity, Vec3, f32)]) -> Vec<Entity> {
    foes.iter()
        .filter(|(_, p, radius)| {
            flat(*p - player).length() - radius <= CLAP_RADIUS && (p.y - player.y).abs() <= 3.0
        })
        .map(|(e, _, _)| *e)
        .collect()
}

/// The rush: steers the body at the enemy, and where it arrives, stuns it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn rush(
    mut commands: Commands,
    time: Res<Time>,
    real: Res<Time<Real>>,
    mut charging: ResMut<Charging>,
    mut rage: ResMut<Rage>,
    screen: Res<Screen>,
    fx: Res<fx::Fx>,
    audio: Option<ResMut<Audio>>,
    mut stuns: MessageWriter<Stun>,
    mut toasts: ResMut<Toasts>,
    mut avatar: Query<(Entity, &mut Transform, &Action, &Health, &mut MoveIntent), With<Avatar>>,
    foes: Query<(&GlobalTransform, &Hurtbox, &Reaction), (With<Hostile>, Without<Avatar>)>,
) {
    let Some(mut run) = charging.0 else {
        return;
    };
    let Ok((me, mut xf, action, health, mut intent)) = avatar.single_mut() else {
        charging.0 = None;
        return;
    };
    let Ok((at, hurt, reaction)) = foes.get(run.target) else {
        charging.0 = None;
        return;
    };
    // Anything that takes the body ends the rush where it is.
    if screen.blocks_actions(real.elapsed_secs())
        || action.owns_body()
        || action.is_dead()
        || health.cur <= 0.0
        || *reaction == Reaction::Dead
    {
        info!("abilities: the charge is cut short");
        charging.0 = None;
        return;
    }
    let dt = time.delta_secs();
    let to = flat(at.translation() - xf.translation);
    let gap = to.length() - hurt.radius - CHARGE_STOP;
    let (speed, arrived) = rush_step(gap, dt);
    xf.rotation = Quat::from_rotation_y(yaw_of(to));
    // Stuck behind a wall, a fence or another body: it does not run in place.
    if run.last_gap - gap < CHARGE_SPEED * dt * 0.25 {
        run.stalled += dt;
    } else {
        run.stalled = 0.0;
    }
    run.last_gap = gap;
    if run.stalled > STALL_AFTER && !arrived {
        info!("abilities: the charge is stopped, something is in the way");
        charging.0 = None;
        return;
    }
    if arrived {
        *intent = MoveIntent::default();
        rage.cur = rage_after_charge(rage.cur);
        rage.quiet = 0.0;
        toasts.push(real.elapsed_secs(), format!("+{CHARGE_RAGE:.0} rage"));
        stuns.write(Stun {
            target: run.target,
            secs: CHARGE_STUN,
        });
        fx::spawn(&mut commands, &fx, fx::Kind::ChargeDust, me);
        fx::sound(
            audio.map(|a| a.into_inner()),
            fx::SOUND_CLAP_HIT,
            "charge hit",
        );
        info!("abilities: the charge lands (rage {:.0})", rage.cur);
        charging.0 = None;
        return;
    }
    *intent = MoveIntent {
        dir: to.normalize_or_zero(),
        speed,
        jump: false,
    };
    charging.0 = Some(run);
}

/// A cast animation holds the body still for its length; a blow, a roll or a death ends it.
pub fn casting(
    mut commands: Commands,
    time: Res<Time>,
    mut avatar: Query<(Entity, &Action, &Health, &mut MoveIntent, &mut Casting), With<Avatar>>,
) {
    for (me, action, health, mut intent, mut cast) in &mut avatar {
        cast.left -= time.delta_secs();
        if cast.left <= 0.0 || action.owns_body() || health.cur <= 0.0 {
            commands.entity(me).remove::<Casting>();
            continue;
        }
        *intent = MoveIntent::default();
    }
}

/// Starts the cast animation's clip on the body (the gait yields while [`Casting`] is on it).
pub fn cast_play(
    mut casters: Query<
        (
            &Casting,
            &ModelAnimations,
            &mut AnimationPlayer,
            &mut AnimationTransitions,
        ),
        Added<Casting>,
    >,
) {
    for (cast, anims, mut player, mut transitions) in &mut casters {
        if let Some(clip) = anims.find(cast.clip) {
            transitions
                .play(&mut player, clip.node, Duration::from_secs_f32(0.1))
                .set_repeat(RepeatAnimation::Never);
        }
    }
}

/// The clap: the blast around the warrior, damage through the same [`Hit`] a sword blow makes (so the
/// flinch, the blood, the sound, the kill and its souls all follow), the slow, the lightning.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn thunder_clap(
    mut commands: Commands,
    mut casts: MessageReader<ThunderClap>,
    buffs: Res<Buffs>,
    mut rage: ResMut<Rage>,
    mut shake: ResMut<Shake>,
    fx: Res<fx::Fx>,
    audio: Option<ResMut<Audio>>,
    mut hits: MessageWriter<Hit>,
    avatar: Query<(Entity, &Transform), With<Avatar>>,
    mut foes: Query<
        (Entity, &GlobalTransform, &Hurtbox, &mut Health, &Reaction),
        (With<Hostile>, Without<Avatar>),
    >,
) {
    let mut audio = audio.map(|a| a.into_inner());
    for _ in casts.read() {
        let Ok((me, xf)) = avatar.single() else {
            continue;
        };
        for kind in [
            fx::Kind::ClapBase,
            fx::Kind::ClapHand(0x15),
            fx::Kind::ClapHand(0x16),
        ] {
            fx::spawn(&mut commands, &fx, kind, me);
        }
        fx::sound(audio.as_deref_mut(), fx::SOUND_CLAP, "thunder clap");
        shake.kick(CLAP_SHAKE);
        let candidates: Vec<(Entity, Vec3, f32)> = foes
            .iter()
            .filter(|(_, _, _, health, reaction)| health.cur > 0.0 && **reaction != Reaction::Dead)
            .map(|(e, at, hurt, _, _)| (e, at.translation(), hurt.radius))
            .collect();
        let victims = clap_victims(xf.translation, &candidates);
        let damage = CLAP_DAMAGE * buffs.damage_factor();
        for &victim in &victims {
            let Ok((_, at, _, mut health, _)) = foes.get_mut(victim) else {
                continue;
            };
            health.cur = (health.cur - damage).max(0.0);
            let lethal = health.cur <= 0.0;
            hits.write(Hit {
                target: victim,
                damage,
                point: at.translation() + Vec3::Y * 0.9,
                lethal,
                level: 1,
                poise: CLAP_POISE,
                from: xf.translation,
                knock: CLAP_KNOCK,
            });
            // What an ability deals does not build rage: the blast would pay for itself.
            rage.ability_damage += damage;
            fx::spawn(&mut commands, &fx, fx::Kind::ClapImpact, victim);
            if !lethal {
                commands.entity(victim).insert(Slowed {
                    left: CLAP_SLOW_SECS,
                });
                fx::spawn(&mut commands, &fx, fx::Kind::Slow, victim);
            }
        }
        if !victims.is_empty() {
            fx::sound(audio.as_deref_mut(), fx::SOUND_CLAP_HIT, "thunder clap hit");
        }
        info!(
            "abilities: thunder clap reaches {} of {} enemies, {damage:.0} each",
            victims.len(),
            candidates.len()
        );
    }
}

/// Stuns what a message names: the creature stands dazed for its length, its swing and its plans gone.
#[allow(clippy::type_complexity)]
pub fn stun(
    mut stuns: MessageReader<Stun>,
    mut foes: Query<(
        &mut Reaction,
        &mut Hostile,
        &mut Brain,
        &mut MonsterPlay,
        &mut MoveIntent,
        Option<&ModelAnimations>,
        Option<&mut AnimationPlayer>,
        Option<&mut AnimationTransitions>,
    )>,
) {
    for s in stuns.read() {
        let Ok((mut reaction, mut hostile, mut brain, mut play, mut intent, anims, player, tr)) =
            foes.get_mut(s.target)
        else {
            continue;
        };
        if *reaction == Reaction::Dead {
            continue;
        }
        if let (Some(anims), Some(mut p), Some(mut tr)) = (anims, player, tr) {
            // WoW's Stun clip while it lasts; a model without one holds its wound clip.
            let id = if anims.find(ANIM_STUN).is_some() {
                ANIM_STUN
            } else {
                COMBAT_WOUND
            };
            if let Some(clip) = anims.find(id) {
                tr.play(&mut p, clip.node, Duration::from_secs_f32(0.05))
                    .set_repeat(if id == ANIM_STUN {
                        RepeatAnimation::Forever
                    } else {
                        RepeatAnimation::Never
                    });
            }
        }
        *reaction = Reaction::Flinch(s.secs);
        hostile.alert = true;
        brain.mind = Mind::Cooldown { left: 0.3 };
        brain.wish = None;
        brain.last = None;
        play.clip = None;
        *intent = MoveIntent::default();
        info!("abilities: a creature is stunned for {:.1} s", s.secs);
    }
}

/// A slowed creature walks and runs at [`CLAP_SLOW`] of its speed and rests longer between swings. Runs
/// after the creature's mind has set its intent for the frame.
pub fn apply_slow(
    mut commands: Commands,
    time: Res<Time>,
    mut foes: Query<(Entity, &mut Slowed, &mut MoveIntent, &mut Brain)>,
) {
    let dt = time.delta_secs();
    for (e, mut slowed, mut intent, mut brain) in &mut foes {
        slowed.left -= dt;
        if slowed.left <= 0.0 {
            commands.entity(e).remove::<Slowed>();
            continue;
        }
        intent.speed *= CLAP_SLOW;
        // The pause between swings counts down slower: it lasts 1 / CLAP_SLOW as long.
        if let Mind::Cooldown { left } = &mut brain.mind {
            *left += dt * (1.0 - CLAP_SLOW);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn foe(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn the_locked_enemy_is_charged_when_it_is_in_range() {
        let foes = [
            (foe(1), Vec3::new(0.0, 0.0, -10.0)),
            (foe(2), Vec3::new(2.0, 0.0, -6.0)),
        ];
        let at = |locked| pick_target(Vec3::ZERO, Vec3::NEG_Z, locked, &foes);
        assert_eq!(
            at(Some(foe(1))),
            Ok(foe(1)),
            "the lock, though the other is nearer"
        );
        // A lock out of range says so; it does not pick another.
        let far = [(foe(1), Vec3::new(0.0, 0.0, -30.0))];
        assert_eq!(
            pick_target(Vec3::ZERO, Vec3::NEG_Z, Some(foe(1)), &far),
            Err(Refusal::TooFar)
        );
        let near = [(foe(1), Vec3::new(0.0, 0.0, -3.0))];
        assert_eq!(
            pick_target(Vec3::ZERO, Vec3::NEG_Z, Some(foe(1)), &near),
            Err(Refusal::TooClose)
        );
    }

    #[test]
    fn without_a_lock_the_nearest_enemy_ahead_in_range_is_charged() {
        let foes = [
            (foe(1), Vec3::new(0.0, 0.0, -3.0)),  // too close
            (foe(2), Vec3::new(1.0, 0.0, -12.0)), // ahead, in range
            (foe(3), Vec3::new(0.0, 0.0, -20.0)), // ahead, farther
            (foe(4), Vec3::new(0.0, 0.0, 8.0)),   // behind
        ];
        assert_eq!(
            pick_target(Vec3::ZERO, Vec3::NEG_Z, None, &foes),
            Ok(foe(2)),
            "not the one at arm's length, not the one behind"
        );
        assert_eq!(
            pick_target(Vec3::ZERO, Vec3::Z, None, &foes[3..]),
            Ok(foe(4)),
            "ahead is where he faces"
        );
    }

    #[test]
    fn a_refused_charge_says_why() {
        let behind = [(foe(1), Vec3::new(0.0, 0.0, 12.0))];
        assert_eq!(
            pick_target(Vec3::ZERO, Vec3::NEG_Z, None, &behind),
            Err(Refusal::NoTarget),
            "nobody ahead"
        );
        assert_eq!(
            pick_target(Vec3::ZERO, Vec3::NEG_Z, None, &[]),
            Err(Refusal::NoTarget)
        );
        let close = [(foe(1), Vec3::new(0.0, 0.0, -2.0))];
        assert_eq!(
            pick_target(Vec3::ZERO, Vec3::NEG_Z, None, &close),
            Err(Refusal::TooClose)
        );
        let far = [(foe(1), Vec3::new(0.0, 0.0, -40.0))];
        assert_eq!(
            pick_target(Vec3::ZERO, Vec3::NEG_Z, None, &far),
            Err(Refusal::TooFar)
        );
        // Height is not distance: the ground between is what counts.
        let up = [(foe(1), Vec3::new(0.0, 9.0, -8.0))];
        assert_eq!(pick_target(Vec3::ZERO, Vec3::NEG_Z, None, &up), Ok(foe(1)));
    }

    #[test]
    fn the_rush_runs_at_full_speed_and_brakes_into_its_stop() {
        let dt = 1.0 / 60.0;
        let (v, arrived) = rush_step(20.0, dt);
        assert_eq!((v, arrived), (CHARGE_SPEED, false));
        // Closer than one frame's run: just far enough, and there.
        let (v, arrived) = rush_step(0.2, dt);
        assert!(arrived && (v * dt - 0.2).abs() < 1.0e-5, "{v}");
        assert_eq!(rush_step(0.0, dt), (0.0, true));
        assert_eq!(rush_step(-1.0, dt), (0.0, true), "already past the stop");
        // Simulated: from 24 yd it arrives in about a second, never overshooting.
        let (mut gap, mut t) = (24.0_f32, 0.0_f32);
        loop {
            let (v, arrived) = rush_step(gap, dt);
            gap -= v * dt;
            t += dt;
            assert!(gap >= -1.0e-3, "no overshoot: {gap}");
            if arrived {
                break;
            }
            assert!(t < 3.0, "it gets there");
        }
        assert!((0.95..1.1).contains(&t), "24 yd at 24 yd/s: {t} s");
    }

    #[test]
    fn the_clap_reaches_what_stands_near_and_not_what_is_far_or_on_another_floor() {
        let foes = [
            (foe(1), Vec3::new(3.0, 0.0, 0.0), 0.4),
            (foe(2), Vec3::new(5.3, 0.0, 0.0), 0.4), // its edge is within 5 yd
            (foe(3), Vec3::new(6.0, 0.0, 0.0), 0.4), // out
            (foe(4), Vec3::new(1.0, 5.0, 0.0), 0.4), // above
            (foe(5), Vec3::new(0.0, 0.0, -4.0), 0.4),
        ];
        assert_eq!(
            clap_victims(Vec3::ZERO, &foes),
            vec![foe(1), foe(2), foe(5)]
        );
        assert!(clap_victims(Vec3::ZERO, &[]).is_empty());
    }

    #[test]
    fn a_charge_that_lands_generates_nine_rage_up_to_the_cap() {
        assert_eq!(rage_after_charge(0.0), 9.0, "it costs none and gives nine");
        assert_eq!(rage_after_charge(40.0), 49.0);
        assert_eq!(rage_after_charge(95.0), 100.0);
    }

    #[test]
    fn the_cooldowns_are_charge_twelve_clap_five_and_the_shout_none() {
        assert_eq!(cooldown(Effect::Charge), 12.0);
        assert_eq!(cooldown(Effect::ThunderClap), 5.0);
        assert_eq!(cooldown(Effect::BattleShout), 0.0);
    }
}
