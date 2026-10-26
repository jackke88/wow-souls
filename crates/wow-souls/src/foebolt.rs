//! A caster's bolt ([`crate::monster::Bullet`]): [`crate::npc::strike`] releases it from the caster's hand at its
//! cast's blow, aimed at the player's chest. It speeds up to the row's top speed, turns toward the player as the
//! row's homing allows and flies until it meets the player, a wall or the end of its life. A roll's
//! invulnerability lets it through (it flies on, past); a raised guard in front blocks it like a blow
//! ([`crate::defense::intercept`]). It wears WoW's own missile model, frost or fire by the caster
//! ([`look_of`]).
use crate::{
    abilities::fx::{self, Fx, Kind},
    avatar::Avatar,
    combat::{Action, Stamina},
    hits::{segment_distance, Hurtbox},
    monster::Bullet,
    npc::PlayerHurt,
    shove::METRES_TO_YARDS,
};
use benilla_world::collision::WorldCollision;
use bevy::prelude::*;

/// How fast a bolt turns per degree of its row's homing (rad/s), an eighth of the player's spells' (0.105). Measured
/// live: Soul Arrow's 40 at 0.105 (4.2 rad/s at 33 yd/s) and at 0.03 (1.2 rad/s, a 2.9 yd bend over its 8 yd
/// flight) both followed a sideways roll that took the player 1.8 yd aside after the release; at 0.5 rad/s (1.2
/// yd) the roll takes the player out of its path while it still bends toward a walking one.
const TURN_PER_HOMING_DEG: f32 = 0.0125;

/// A bolt in flight.
#[derive(Component, Debug, Clone)]
pub struct FoeBolt {
    /// The caster and its name (for the log).
    pub from: Entity,
    pub caster: &'static str,
    /// Where it left the hand: the side a guard must face.
    pub origin: Vec3,
    pub at: Vec3,
    pub velocity: Vec3,
    pub bullet: Bullet,
    pub age: f32,
    /// What it does to the player: the caster's damage and the blow's flinch level, hit-stop, push and poise.
    pub damage: f32,
    pub foe_level: u8,
    pub level: u8,
    pub stop: f32,
    pub knock: f32,
    pub poise: f32,
    /// How a guard takes it (`None`: as any unlisted blow).
    pub guard: Option<crate::defense::Attack>,
    /// A roll let it through once: it says so once.
    pub passed: bool,
    /// A named throw's hold instead of a blow ([`crate::named`]): it holds the player's feet.
    pub effect: Option<crate::named::Throw>,
}

/// The missile a caster's bolt wears: the casters of fire (WoW: Surena Caledon and the Kobold Geomancer cast
/// Fireball, `creature_spells` 8810 / 4760) and of frost (the Defias Rogue Wizard's Frostbolt, 4740).
pub fn look_of(species: &str) -> Kind {
    if matches!(species, "surena" | "geomancer") {
        Kind::FireballMissile
    } else if matches!(species, "forest_spider" | "mother_fang") {
        // The hairy spiders spit venom (WoW's Venom Spit, 16552).
        Kind::VenomMissile
    } else {
        Kind::FrostboltMissile
    }
}

/// The bolt's speed (yd/s) `age` seconds after release: the row's start speed rising to its top speed over its
/// acceleration time.
pub fn speed_at(b: &Bullet, age: f32) -> f32 {
    let t = if b.accel > 0.0 {
        (age / b.accel).clamp(0.0, 1.0)
    } else {
        1.0
    };
    (b.speed + (b.max.max(b.speed) - b.speed) * t) * METRES_TO_YARDS
}

/// Releases `bolt` at its `at`, toward `aim`, wearing `look`.
pub fn release(commands: &mut Commands, fx: Option<&Fx>, mut bolt: FoeBolt, aim: Vec3, look: Kind) {
    let dir = (aim - bolt.at).normalize_or(Vec3::NEG_Z);
    bolt.velocity = dir * speed_at(&bolt.bullet, 0.0);
    let at = bolt.at;
    let e = commands
        .spawn((
            Name::new("foe bolt"),
            Transform::from_translation(at).looking_to(dir, Vec3::Y),
            Visibility::default(),
            bolt,
        ))
        .id();
    if let Some(fx) = fx {
        fx::spawn(commands, fx, look, e);
    }
}

/// A caster starts a bolt: its wind-up is shown and heard (2026-10-18, the user on the Kobold Geomancer: "lite
/// eld ljud när dom börjar så man får en indikation och lite längre animation"). `secs` is the wind-up in
/// real seconds, to the release.
#[derive(Message, Debug, Clone, Copy)]
pub struct CastTell {
    pub from: Entity,
    pub species: &'static str,
    pub secs: f32,
}

/// How fast a caster's wind-up clock runs before the bolt leaves: the geomancer's cast was too quick to
/// read (0.6 makes its 0.53 s wind-up 0.9 s); everything else as cut.
pub fn windup_rate(species: &str, move_name: &str) -> f32 {
    match (species, move_name) {
        ("geomancer", _) => 0.6,
        // Surena's heavy bolt (2026-10-18): the same cast as her bolt, so it is told apart by a long,
        // glowing wind-up and a harder hit ([`bolt_weight`]).
        (_, "heavy_bolt") => 0.45,
        _ => 1.0,
    }
}

/// How much harder than its kit's number a bolt hits: a heavy bolt 1.8 times.
pub fn bolt_weight(move_name: &str) -> f32 {
    if move_name == "heavy_bolt" {
        1.8
    } else {
        1.0
    }
}

pub fn plugin(app: &mut App) {
    app.add_message::<CastTell>().add_systems(
        Update,
        (fly, tell).run_if(in_state(crate::settle::Phase::Playing)),
    );
}

/// The wind-up of a cast: WoW's precast in both spell hands (fire or frost, as the bolt) and its precast
/// sound at the caster, for as long as the wind-up lasts.
fn tell(
    mut commands: Commands,
    time: Res<Time>,
    mut playing: Local<Vec<(crate::mixer::Level, f32)>>,
    mut tells: MessageReader<CastTell>,
    fx: Option<Res<Fx>>,
    mut audio: Option<ResMut<crate::audio::Audio>>,
    cams: Query<&GlobalTransform, With<benilla_world::view::WorldCamera>>,
    bodies: Query<&GlobalTransform>,
) {
    // The precast loop is cut when the bolt leaves (WoW's file runs over five seconds).
    for (level, left) in playing.iter_mut() {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            level.set(0.0);
        }
    }
    playing.retain(|(_, left)| *left > 0.0);
    let heard = crate::audio::listener(&cams);
    for t in tells.read() {
        // A spider's spit has no spell hands to glow.
        if look_of(t.species) == Kind::VenomMissile {
            continue;
        }
        let fire = look_of(t.species) == Kind::FireballMissile;
        if let Some(fx) = fx.as_deref() {
            for hand in [0x15, 0x16] {
                let kind = if fire {
                    Kind::FirePrecastHand(hand)
                } else {
                    Kind::IceHand(hand)
                };
                fx::spawn_for(&mut commands, fx, kind, t.from, t.secs.max(0.3));
            }
        }
        if let (Some(audio), Ok(at)) = (audio.as_deref_mut(), bodies.get(t.from)) {
            let kit = if fire {
                fx::SOUND_FIRE_PRECAST
            } else {
                fx::SOUND_ICE_PRECAST
            };
            if let Some(level) =
                audio.play_kit_leveled(kit, heard.map(|l| (at.translation(), l)), 1.0, "cast tell")
            {
                playing.push((level, t.secs));
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    world: WorldCollision,
    defense: Option<Res<crate::defense::Kit>>,
    gear: Res<crate::gear_effect::GearFx>,
    mut bolts: Query<(Entity, &mut FoeBolt, &mut Transform), Without<Avatar>>,
    mut avatar: Query<(Entity, &Transform, &mut Action, &mut Stamina, &Hurtbox), With<Avatar>>,
    mut hurts: MessageWriter<PlayerHurt>,
    fx: Option<Res<Fx>>,
    mut rooted: ResMut<crate::named::Rooted>,
) {
    let dt = time.delta_secs();
    let mut player = avatar.single_mut().ok();
    for (e, mut b, mut xf) in &mut bolts {
        b.age += dt;
        if b.age > b.bullet.life {
            debug!("foebolt: {}'s bolt fades at {:?}", b.caster, b.at);
            commands.entity(e).despawn();
            continue;
        }
        let speed = speed_at(&b.bullet, b.age);
        let chest = player.as_ref().map(|(_, p, _, _, hurt)| {
            let (lo, hi) = hurt.axis(p.translation);
            (lo + hi) * 0.5
        });
        let mut v = b.velocity.normalize_or(Vec3::NEG_Z) * speed;
        // A bolt that has passed through a roll flies on straight: it does not come back for the player.
        if let Some(chest) = chest.filter(|_| !b.passed) {
            v = crate::projectiles::steer(
                v,
                chest - b.at,
                b.bullet.homing * TURN_PER_HOMING_DEG * dt,
            );
        }
        b.velocity = v;
        let from = b.at;
        let to = from + v * dt;
        let dir = (to - from).normalize_or_zero();
        // The camera's sweep sees the solid world only, not the creatures' bodies (the caster's own among them).
        let wall = world.cast_camera(from, to - from, false);
        if let Some((me, body, action, stamina, hurt)) = player.as_mut() {
            let (lo, hi) = hurt.axis(body.translation);
            let (gap, on_path, _) = segment_distance(from, to, lo, hi);
            let reach = hurt.radius + b.bullet.radius * METRES_TO_YARDS;
            let before_wall = wall.is_none_or(|w| on_path.distance(from) <= w);
            if gap <= reach && before_wall && !action.is_dead() {
                if action.invulnerable() {
                    if !b.passed {
                        b.passed = true;
                        info!("foebolt: {}'s bolt passes through the roll", b.caster);
                    }
                } else {
                    if let Some(kind) = b.effect {
                        crate::named::catch(&mut commands, fx.as_deref(), &mut rooted, *me, kind);
                        info!("foebolt: {}'s {kind:?} holds the player", b.caster);
                        commands.entity(e).despawn();
                        continue;
                    }
                    let (damage, blocked) = crate::defense::intercept(
                        defense.as_deref(),
                        b.guard,
                        body,
                        b.origin,
                        action,
                        stamina,
                        b.damage,
                    );
                    let damage = gear.0.taken(damage, b.foe_level, blocked);
                    hurts.write(PlayerHurt {
                        from: b.from,
                        damage,
                        level: b.level,
                        stop: if blocked {
                            crate::hurt::HURT_STOP
                        } else {
                            b.stop
                        },
                        knock: b.knock,
                        poise: b.poise,
                        blocked,
                    });
                    info!(
                        "foebolt: {}'s bolt strikes the player for {damage:.0}{}",
                        b.caster,
                        if blocked { " (blocked)" } else { "" }
                    );
                    commands.entity(e).despawn();
                    continue;
                }
            }
        }
        if wall.is_some() {
            debug!(
                "foebolt: {}'s bolt meets a wall at {:?} (age {:.2})",
                b.caster, b.at, b.age
            );
            commands.entity(e).despawn();
            continue;
        }
        b.at = to;
        *xf = Transform::from_translation(to).looking_to(dir.normalize_or(Vec3::NEG_Z), Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARROW: Bullet = Bullet {
        speed: 4.0,
        max: 30.0,
        accel: 0.32,
        radius: 0.1,
        homing: 40.0,
        life: 1.1,
    };

    #[test]
    fn a_bolt_speeds_up_to_its_top_speed_over_its_acceleration() {
        assert!((speed_at(&ARROW, 0.0) - 4.0 * METRES_TO_YARDS).abs() < 1.0e-4);
        assert!((speed_at(&ARROW, 0.16) - 17.0 * METRES_TO_YARDS).abs() < 1.0e-3);
        assert!((speed_at(&ARROW, 0.32) - 30.0 * METRES_TO_YARDS).abs() < 1.0e-4);
        assert!((speed_at(&ARROW, 0.9) - 30.0 * METRES_TO_YARDS).abs() < 1.0e-4);
    }

    #[test]
    fn fire_casters_throw_fire_and_the_rest_frost() {
        assert_eq!(look_of("surena"), Kind::FireballMissile);
        assert_eq!(look_of("geomancer"), Kind::FireballMissile);
        assert_eq!(look_of("rogue_wizard"), Kind::FrostboltMissile);
    }
}
