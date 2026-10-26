//! The player hurt: a creature's blow ([`PlayerHurt`]) costs health and staggers the body (its
//! CombatWound clip, whatever it was doing stops); at zero health the body dies (Death), "YOU
//! DIED" fades in, and after [`RESPAWN_AFTER`] seconds the player stands again at the spawn
//! point, whole, and every creature is reset ([`Respawned`]).

use std::time::Duration;

use bevy::animation::transition::AnimationTransitions;
use bevy::animation::RepeatAnimation;
use bevy::prelude::*;

use benilla_assets::ModelAnimations;

use crate::avatar::Avatar;
use crate::combat::{Action, Health, Stamina};
use crate::controller::SpawnPoint;
use crate::hits::HitStop;
use crate::monster::Poise;
use crate::motor::{Motor, MoveIntent};
use crate::npc::{PlayerHurt, COMBAT_WOUND, DEATH};
use crate::reaction::PlayerReacts;
use crate::settle::Phase;

/// How long a blow staggers the player (s).
pub const STAGGER: f32 = 0.5;
/// From death to the respawn (real s).
pub const RESPAWN_AFTER: f32 = 4.5;
/// The freeze when the player is struck (real s): shorter than a landed blow's.
pub const HURT_STOP: f32 = 0.05;
/// "YOU DIED" fades in over this long (real s).
const DIED_FADE: f32 = 1.2;

/// The player stands again; creatures reset.
#[derive(Message, Debug, Clone, Copy)]
pub struct Respawned;

/// When the player died (real s), while dead.
#[derive(Resource, Debug, Default)]
pub struct Death {
    pub at: Option<f32>,
}

#[derive(Component)]
struct DiedText;

pub fn plugin(app: &mut App) {
    app.add_message::<Respawned>()
        .init_resource::<Death>()
        // Plain text only when neither the native nor the legacy banner is available.
        .add_systems(
            OnEnter(Phase::Playing),
            spawn_text.run_if(not(
                resource_exists::<crate::hud::HudArt>.or(crate::hud::theme::enabled)
            )),
        )
        .add_systems(
            Update,
            (suffer, respawn, fade_text)
                .chain()
                .after(crate::controller::PlaySet::Motor)
                .run_if(in_state(Phase::Playing)),
        );
}

/// Plays a clip once over `len` seconds; a model without it holds its pose.
fn play_over(
    anims: &ModelAnimations,
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    id: u16,
    len: Option<f32>,
) {
    let Some(clip) = anims.find(id) else {
        return;
    };
    let rate = len.map_or(1.0, |l| (clip.duration / l).clamp(0.5, 3.0));
    transitions
        .play(player, clip.node, Duration::from_secs_f32(0.05))
        .set_repeat(RepeatAnimation::Never)
        .set_speed(rate);
}

/// Lands the creatures' blows on the player. With the Dark Souls kit ([`crate::reaction`]) the player's
/// poise decides whether a blow flinches at all, the blow's level and the way it pushes the body choose
/// the clip, and the clip's own control flags say how long the body reels; a killing blow picks the
/// death. Without the kit the body staggers 0.5 s in WoW's wound clip and dies in WoW's death.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn suffer(
    mut commands: Commands,
    real: Res<Time<bevy::time::Real>>,
    mut virt: ResMut<Time<Virtual>>,
    mut stop: ResMut<HitStop>,
    mut hurts: MessageReader<PlayerHurt>,
    mut death: ResMut<Death>,
    mut lock: ResMut<crate::lockon::LockOn>,
    kit: Option<Res<PlayerReacts>>,
    foes: Query<&Transform, Without<Avatar>>,
    mut avatar: Query<
        (
            Entity,
            &Transform,
            &mut Health,
            &mut Action,
            &mut Poise,
            Option<&ModelAnimations>,
            Option<&mut AnimationPlayer>,
            Option<&mut AnimationTransitions>,
        ),
        With<Avatar>,
    >,
) {
    let Ok((me, body, mut hp, mut action, mut poise, anims, mut player, mut transitions)) =
        avatar.single_mut()
    else {
        return;
    };
    let kit = kit.as_deref();
    let now = real.elapsed_secs();
    for hurt in hurts.read() {
        if action.is_dead() {
            continue;
        }
        let hurt = PlayerHurt {
            damage: hurt.damage * action.training.damage_taken,
            ..*hurt
        };
        hp.cur = (hp.cur - hurt.damage).max(0.0);
        // Which way the blow pushes the body, in its own frame: (forward, left).
        let away = foes
            .get(hurt.from)
            .map(|foe| {
                Vec3::new(
                    body.translation.x - foe.translation.x,
                    0.0,
                    body.translation.z - foe.translation.z,
                )
                .normalize_or_zero()
            })
            .unwrap_or(Vec3::ZERO);
        let local = body.rotation.inverse() * away;
        let push = Vec2::new(-local.z, -local.x);
        // The WoW clip to play when no Dark Souls clip does.
        let mut wow_clip = None;
        if hp.cur <= 0.0 {
            let fall = kit.and_then(|k| k.choose_death(push));
            action.die_with(fall);
            death.at = Some(now);
            lock.target = None;
            info!(
                "player: struck for {:.0} by {}, dies{}",
                hurt.damage,
                hurt.from,
                fall.and_then(|d| kit.and_then(|k| k.death_clip(d))).map_or(
                    String::new(),
                    |(c, hold)| format!(
                        " ({}, push {:+.2}/{:+.2}, holds from {hold:.2} s)",
                        c.wow.name, push.x, push.y
                    )
                )
            );
            if fall.is_none() {
                wow_clip = Some((DEATH, None));
            }
        } else if hurt.blocked {
            info!(
                "player: block chip {:.1}, health {:.1}/{:.0}",
                hurt.damage, hp.cur, hp.max
            );
            if let Ok(foe) = foes.get(hurt.from) {
                crate::shove::start(
                    &mut commands,
                    me,
                    "guard",
                    body.translation - foe.translation,
                    hurt.knock * 0.15,
                );
            }
        } else if hurt.level == 0 || !poise.hit(hurt.poise) {
            // Level 0 is a bite that holds on (a latched spider chews): it hurts and never flinches.
            info!(
                "player: struck for {:.0} by {}, health {:.0}/{:.0}, poise holds ({:.0}/{:.0}): no flinch",
                hurt.damage, hurt.from, hp.cur, hp.max, poise.left, poise.max
            );
        } else {
            let chosen = kit.and_then(|k| k.choose(hurt.level, push).map(|i| (k, i)));
            let shoves = match chosen {
                Some((k, i)) => {
                    let r = &k.reacts[i];
                    action.reel(i, r.spec);
                    info!(
                        "player: struck for {:.0} by {}, health {:.0}/{:.0}, reaction {} ({}, level {}, push {:+.2}/{:+.2}), control in {:.2} s, poise {:.0}/{:.0} (stop {:.2} s, knock {:.2} m)",
                        hurt.damage,
                        hurt.from,
                        hp.cur,
                        hp.max,
                        r.wow.name,
                        r.class,
                        hurt.level,
                        push.x,
                        push.y,
                        r.spec.chain_from.min(r.spec.evade_from),
                        poise.left,
                        poise.max,
                        hurt.stop,
                        hurt.knock
                    );
                    // A knockdown is thrown by its own clip; a flinch is shoved.
                    !r.travels()
                }
                None => {
                    action.stagger(STAGGER);
                    info!(
                        "player: struck for {:.0} by {}, health {:.0}/{:.0}, staggered (stop {:.2} s, knock {:.2} m)",
                        hurt.damage, hurt.from, hp.cur, hp.max, hurt.stop, hurt.knock
                    );
                    wow_clip = Some((COMBAT_WOUND, Some(STAGGER)));
                    true
                }
            };
            if shoves {
                if let Ok(foe) = foes.get(hurt.from) {
                    crate::shove::start(
                        &mut commands,
                        me,
                        "player",
                        body.translation - foe.translation,
                        hurt.knock,
                    );
                }
            }
        }
        if let (Some((clip, len)), Some(a), Some(p), Some(tr)) = (
            wow_clip,
            anims,
            player.as_deref_mut(),
            transitions.as_deref_mut(),
        ) {
            play_over(a, p, tr, clip, len);
        }
        if hurt.stop > 0.0 {
            stop.freeze(&mut virt, now, hurt.stop);
        }
    }
}

/// Return to the bound memorial, or the initial spawn. Keep the death cover and the dead body
/// held while the destination streams; health and the Respawned event are restored only on ground.
#[allow(clippy::type_complexity)]
fn respawn(
    real: Res<Time<bevy::time::Real>>,
    mut death: ResMut<Death>,
    mut spawn: Option<ResMut<SpawnPoint>>,
    binding: Res<crate::soulbind::Binding>,
    progress: Option<Res<benilla_world::terrain_stream::WorldLoadProgress>>,
    collide: benilla_world::collision::WorldCollision,
    mut returning: Local<bool>,
    mut respawned: MessageWriter<Respawned>,
    mut avatar: Query<
        (
            &mut Transform,
            &mut Motor,
            &mut MoveIntent,
            &mut Health,
            &mut Stamina,
            &mut Action,
            &mut Poise,
        ),
        With<Avatar>,
    >,
) {
    let (Some(at), Some(spawn)) = (death.at, spawn.as_deref_mut()) else {
        return;
    };
    if real.elapsed_secs() < at + RESPAWN_AFTER {
        return;
    }
    let Ok((mut xf, mut m, mut intent, mut hp, mut st, mut action, mut poise)) =
        avatar.single_mut()
    else {
        return;
    };
    // The nearest graveyard bound at, from where the body fell.
    let site = binding.respawn_site(xf.translation);
    let destination = site.map_or(spawn.0, |s| s.arrival());
    xf.translation = destination;
    xf.rotation = site.map_or(Quat::IDENTITY, |s| s.facing());
    *m = Motor::default();
    *intent = MoveIntent::default();
    if site.is_some() {
        if !*returning {
            info!(
                "soulbind: returning to {:?}; waiting for resident ground",
                binding.bound
            );
            *returning = true;
        }
        let wow = benilla_assets::coords::bevy_to_wow(destination);
        let tile = benilla_formats::world_to_tile(wow[0], wow[1]);
        let resident = progress.as_ref().is_some_and(|p| {
            p.presentable() && p.focus_tile == Some((tile.0 as i32, tile.1 as i32))
        });
        if !resident {
            return;
        }
        let Some(y) = crate::settle::snap_to_ground(&collide, destination)
            .filter(|y| (*y - destination.y).abs() <= crate::npc::GROUND_TOLERANCE)
        else {
            return;
        };
        xf.translation.y = y;
        spawn.0 = xf.translation;
    }
    *returning = false;
    death.at = None;
    *hp = Health::full(hp.max);
    *st = Stamina::full(st.max);
    *action = Action::default();
    *poise = Poise::new(poise.max);
    respawned.write(Respawned);
    info!(
        "player: respawns at {} ({:?})",
        site.map_or("the spawn point", |s| s.name.as_str()),
        xf.translation
    );
}

fn spawn_text(mut commands: Commands) {
    let row = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                top: percent(40),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            DiedText,
        ))
        .id();
    commands.spawn((
        Text::new("YOU DIED"),
        TextFont::from_font_size(84.0),
        TextColor(Color::srgba(0.62, 0.06, 0.04, 0.0)),
        ChildOf(row),
    ));
}

/// Fades "YOU DIED" in while the player is dead.
fn fade_text(
    real: Res<Time<bevy::time::Real>>,
    death: Res<Death>,
    mut row: Query<(&mut Visibility, &Children), With<DiedText>>,
    mut colors: Query<&mut TextColor>,
) {
    let Ok((mut vis, children)) = row.single_mut() else {
        return;
    };
    let Some(at) = death.at else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    };
    *vis = Visibility::Inherited;
    let k = ((real.elapsed_secs() - at) / DIED_FADE).clamp(0.0, 1.0);
    for c in children.iter() {
        if let Ok(mut color) = colors.get_mut(c) {
            color.0.set_alpha(k);
        }
    }
}

#[cfg(test)]
mod soul_respawn_tests {
    use super::*;
    use benilla_world::terrain_stream::WorldLoadProgress;
    use bevy::ecs::system::RunSystemOnce;

    fn fixture(ground: bool) -> (App, Entity, Vec3) {
        let mut app = crate::motor::testkit::world_from_profile(if ground {
            &[(-10.0, 0.0), (10.0, 0.0)]
        } else {
            &[(100.0, 0.0), (110.0, 0.0)]
        });
        let site = crate::soulbind::Site {
            id: 1,
            name: "Test Graveyard".into(),
            at: Vec3::ZERO,
            yaw: 0.0,
        };
        let destination = site.arrival();
        let mut binding = crate::soulbind::Binding::default();
        binding.sites = vec![site];
        binding.bound = Some(1);
        app.insert_resource(binding)
            .insert_resource(Death { at: Some(0.0) })
            .insert_resource(SpawnPoint(Vec3::new(100.0, 0.0, 100.0)))
            .insert_resource(WorldLoadProgress::default())
            .add_message::<Respawned>();
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs(10));
        let mut action = Action::default();
        action.die();
        let mut hp = Health::full(150.0);
        hp.cur = 0.0;
        let body = app
            .world_mut()
            .spawn((
                Avatar,
                Transform::from_xyz(500.0, 0.0, 500.0),
                Motor::default(),
                MoveIntent::default(),
                hp,
                Stamina::full(200.0),
                action,
                Poise::new(20.0),
            ))
            .id();
        (app, body, destination)
    }

    fn resident(app: &mut App, destination: Vec3) {
        let wow = benilla_assets::coords::bevy_to_wow(destination);
        let tile = benilla_formats::world_to_tile(wow[0], wow[1]);
        *app.world_mut().resource_mut::<WorldLoadProgress>() = WorldLoadProgress {
            scene_ready: true,
            focus_tile: Some((tile.0 as i32, tile.1 as i32)),
            ..default()
        };
    }

    #[test]
    fn soul_respawn_holds_death_until_destination_terrain_and_ground_exist() {
        let (mut app, body, destination) = fixture(false);
        app.world_mut().run_system_once(respawn).unwrap();
        assert_eq!(
            app.world().get::<Transform>(body).unwrap().translation,
            destination
        );
        assert!(app.world().resource::<Death>().at.is_some());
        assert_eq!(app.world().get::<Health>(body).unwrap().cur, 0.0);
        assert!(app.world().resource::<Messages<Respawned>>().is_empty());
        resident(&mut app, destination);
        app.world_mut().run_system_once(respawn).unwrap();
        assert!(
            app.world().resource::<Death>().at.is_some(),
            "residency without a collider is unsafe"
        );
        assert!(app.world().resource::<Messages<Respawned>>().is_empty());
    }

    #[test]
    fn soul_respawn_restores_health_at_the_bound_graveyard_once_ready() {
        let (mut app, body, destination) = fixture(true);
        resident(&mut app, destination);
        app.world_mut().run_system_once(respawn).unwrap();
        assert!(app.world().resource::<Death>().at.is_none());
        let hp = app.world().get::<Health>(body).unwrap();
        assert_eq!(hp.cur, hp.max);
        assert!(!app.world().get::<Action>(body).unwrap().is_dead());
        assert_eq!(
            app.world().get::<Transform>(body).unwrap().translation,
            destination
        );
        assert_eq!(app.world().resource::<SpawnPoint>().0, destination);
        assert_eq!(app.world().resource::<Messages<Respawned>>().len(), 1);
        app.world_mut().run_system_once(respawn).unwrap();
        assert_eq!(app.world().resource::<Messages<Respawned>>().len(), 1);
    }
}
