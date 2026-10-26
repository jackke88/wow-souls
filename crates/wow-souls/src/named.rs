//! Elwynn's named mechanics (vMaNGOS `creature_spells` and EventAI, read on a copy of the database), made to
//! play as Souls fights rather than WoW auras:
//!
//! - **Thrash** (Gruff Swiftbite, Riverpaw Outrunner; 3391, first after 8 s, then every 15-20 s): its next swing
//!   chains straight into the other one, an extra blow with no pause ([`crate::npc::Brain::forced`]).
//! - **Rushing Charge** (the boars; 6268 on aggro, 10-20 %): +59 % speed for 3 s as it notices the player, with
//!   WoW's sprint burst at its feet and sound 3339.
//! - **Backstab** (Cutpurse, Thuros, Morgaine, Morgan, Murloc Lurker, Bodyguard): a blow that lands from behind
//!   the player deals half again as much ([`on_blow`]).
//! - **Gouge** (Morgan 1776 every 11-13 s, Morgaine 12540 every 10 s): the next blow that lands is a level 5
//!   blow, the short knockdown, instead of a flinch.
//! - **Poison** (Morgaine 11918 every 30 s, Mother Fang 16400 every 17-19 s): the next blow that lands poisons:
//!   a few points every second and a half for nine seconds, no flinch ([`PlayerPoison`]).
//! - **Net** (Ruklar, 12024: first after 19 s, then every 21-31 s) and **Web** (Mother Fang, 12023, every 22 s):
//!   from three to fourteen yards it throws, with a move of its own kit, a missile that holds the player's feet
//!   for [`ROOT_SECS`] ([`Rooted`]): WoW's own net and web models in flight and wrapped round the player. A roll
//!   through it lets it pass ([`crate::foebolt`]).
//! - **Summon** (the Defias Dockmaster, script 684602): 1.5 s after it notices the player, three Defias
//!   Bodyguards (6866) step out beside it; they go 30 s after the fight ends ([`Summoned`]).
use crate::{
    abilities::fx::{self, Fx, Kind},
    avatar::Avatar,
    monster::{Bullet, Roster},
    npc::{Brain, Hostile, Mind, PlayerHurt, Reaction},
    species::Species,
};
use bevy::prelude::*;

/// What a thrown move holds the player with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Throw {
    Net,
    Web,
}

impl Throw {
    /// The missile: Net flies at WoW's 40 yd/s, Web at its 12 (slow enough to read and roll through).
    pub fn bullet(self) -> Bullet {
        match self {
            Throw::Net => Bullet {
                speed: 30.0,
                max: 30.0,
                accel: 0.0,
                radius: 0.5,
                homing: 10.0,
                life: 0.9,
            },
            Throw::Web => Bullet {
                speed: 11.0,
                max: 11.0,
                accel: 0.0,
                radius: 0.6,
                homing: 10.0,
                life: 1.8,
            },
        }
    }

    pub fn missile(self) -> Kind {
        match self {
            Throw::Net => Kind::NetMissile,
            Throw::Web => Kind::WebMissile,
        }
    }

    pub fn state(self) -> Kind {
        match self {
            Throw::Net => Kind::NetState,
            Throw::Web => Kind::WebState,
        }
    }
}

/// How long a net or a web holds the player's feet (s).
pub const ROOT_SECS: f32 = 2.5;
/// The band a throw is made from (yd).
const THROW_BAND: (f32, f32) = (3.0, 14.0);
/// Backstab's damage from behind, and how far behind the blow must come from (the cosine between the
/// player's facing and the way to the creature).
pub const BACKSTAB: f32 = 1.5;
const BEHIND: f32 = -0.25;
/// The flinch level of a gouging blow: the short knockdown.
pub const GOUGE_LEVEL: u8 = 5;
/// Rushing Charge: the speed and how long it lasts.
const RUSH_FACTOR: f32 = 1.59;
const RUSH_SECS: f32 = 3.0;
const SOUND_RUSH: u32 = 3339;
/// The summons' lingering after the fight (s).
const SUMMON_LINGER: f32 = 30.0;
/// Pierce Armor (6016: -51 % armor for 20 s) on the player's Pierce Armor stacks ([`crate::boss::ArmorShred`],
/// Hogger's +8 % damage taken per stack): two stacks.
pub const PIERCE_STACKS: u8 = 2;
/// Disarm (6713, the Bodyguard's): the weapon goes for this long (s) and the fists strike for [`DISARM_FACTOR`].
pub const DISARM_SECS: f32 = 5.0;
pub const DISARM_FACTOR: f32 = 0.3;
/// Clara Charles (5917): the player within this distance (yd) springs her ambush (EventAI 591701, out-of-combat
/// line of sight, 15 yd), once per game; the Defias Ambusher (583) steps out at its database spot and goes 180 s
/// after the fight.
const CLARA: u32 = 5917;
const AMBUSH_RANGE: f32 = 15.0;
const AMBUSH_AT: [f32; 3] = [-9232.11, 342.473, 74.4399];
const AMBUSH_FACING: f32 = 4.31658;
const AMBUSH_LINGER: f32 = 180.0;

/// A timed mechanic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    Thrash,
    Gouge,
    Poison {
        per_tick: f32,
        ticks: u32,
    },
    Throw {
        kind: Throw,
        mv: &'static str,
    },
    /// Pierce Armor: the next blow that lands adds [`PIERCE_STACKS`] stacks of [`crate::boss::ArmorShred`].
    Pierce,
    /// Disarm: the next blow that lands takes the player's weapon for [`DISARM_SECS`].
    Disarm,
}

/// A mechanic that comes round on a timer: first after `first` seconds of the fight, then every `every`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timed {
    pub act: Act,
    pub first: f32,
    pub every: (f32, f32),
}

const THRASH: Timed = Timed {
    act: Act::Thrash,
    first: 8.0,
    every: (15.0, 20.0),
};

/// The timed mechanics of a species.
pub fn timed(species: &str) -> &'static [Timed] {
    match species {
        "gruff" | "outrunner" => &[THRASH],
        "morgan" => &[Timed {
            act: Act::Gouge,
            first: 1.0,
            every: (11.0, 13.0),
        }],
        "morgaine" => &[
            Timed {
                act: Act::Gouge,
                first: 1.0,
                every: (10.0, 10.0),
            },
            Timed {
                act: Act::Poison {
                    per_tick: 4.0,
                    ticks: 6,
                },
                first: 6.0,
                every: (30.0, 30.0),
            },
        ],
        "ruklar" => &[
            Timed {
                act: Act::Throw {
                    kind: Throw::Net,
                    mv: "big",
                },
                first: 6.0,
                every: (21.0, 31.0),
            },
            Timed {
                act: Act::Pierce,
                first: 3.0,
                every: (45.0, 45.0),
            },
        ],
        "bodyguard" => &[Timed {
            act: Act::Disarm,
            first: 2.0,
            every: (34.0, 38.0),
        }],
        "mother_fang" => &[
            Timed {
                act: Act::Throw {
                    kind: Throw::Web,
                    mv: "bite",
                },
                first: 5.0,
                every: (22.0, 22.0),
            },
            Timed {
                act: Act::Poison {
                    per_tick: 4.0,
                    ticks: 6,
                },
                first: 1.0,
                every: (17.0, 19.0),
            },
        ],
        _ => &[],
    }
}

/// Whether the species backstabs (a blow from behind the player counts for [`BACKSTAB`]).
pub fn backstabs(species: &str) -> bool {
    matches!(
        species,
        "cutpurse" | "thuros" | "morgaine" | "morgan" | "lurker" | "bodyguard"
    )
}

/// The chance (%) a boar rushes as it notices the player (EventAI: Stonetusk 15, Princess and the Porcine
/// Entourage 20, Rockhide 10).
pub fn rush_chance(species: &str) -> Option<u32> {
    match species {
        "stonetusk" => Some(15),
        "princess" | "porcine" => Some(20),
        "rockhide" => Some(10),
        _ => None,
    }
}

/// Who the species calls into the fight, how many, and how long after it notices the player.
pub fn summons(species: &str) -> Option<(&'static crate::species::SpeciesDef, usize, f32)> {
    match species {
        "dockmaster" => Some((&crate::species::BODYGUARD, 3, 1.5)),
        _ => None,
    }
}

fn has_mechanics(species: &str) -> bool {
    !timed(species).is_empty()
        || backstabs(species)
        || rush_chance(species).is_some()
        || summons(species).is_some()
}

/// A named creature's mechanics in the fight.
#[derive(Component, Debug, Default)]
pub struct Named {
    /// Seconds until each of [`timed`]'s mechanics is ready.
    timers: Vec<f32>,
    /// A Gouge or a Poison waiting for the next blow that lands.
    pub primed: Option<Act>,
    alerted: bool,
    engaged: f32,
    summoned: bool,
}

/// A creature called into the fight: it goes [`SUMMON_LINGER`] seconds after the fight ends or it dies.
#[derive(Component, Debug)]
pub struct Summoned {
    pub left: f32,
    /// How long it stays once out of the fight (s).
    pub linger: f32,
}

/// The player's feet are held for this long (s).
#[derive(Resource, Debug, Default)]
pub struct Rooted(pub f32);

/// A poison on the player.
#[derive(Resource, Debug, Default)]
pub struct PlayerPoison {
    pub from: Option<Entity>,
    pub per_tick: f32,
    pub ticks: u32,
    pub timer: f32,
}

/// Seconds between poison ticks.
const POISON_EVERY: f32 = 1.5;

/// A blow of `species` standing at `from` landing on the player at `player`: its damage (a backstab from
/// behind counts for more) and its flinch level (a primed Gouge knocks down); a primed Poison is left for
/// [`poison_on_hurt`].
pub fn on_blow(
    named: Option<&mut Named>,
    species: &str,
    from: Vec3,
    player: &Transform,
    damage: f32,
    level: u8,
) -> (f32, u8) {
    let mut out = (damage, level);
    if backstabs(species) {
        let to = Vec3::new(
            from.x - player.translation.x,
            0.0,
            from.z - player.translation.z,
        )
        .normalize_or_zero();
        let facing = Vec3::new(player.forward().x, 0.0, player.forward().z).normalize_or_zero();
        if to != Vec3::ZERO && facing.dot(to) < BEHIND {
            out.0 = (damage * BACKSTAB).round();
            info!("named: {species} backstabs ({damage:.0} -> {:.0})", out.0);
        }
    }
    if let Some(n) = named {
        if n.primed == Some(Act::Gouge) {
            n.primed = None;
            out.1 = GOUGE_LEVEL;
            info!("named: {species} gouges the player");
        }
    }
    out
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Rooted>()
        .init_resource::<PlayerPoison>()
        .init_resource::<Disarmed>()
        .add_systems(
            Update,
            (
                attach,
                tick,
                poison_on_hurt,
                poison_tick,
                root_tick,
                linger,
                disarm_tick,
                ambush,
            )
                .run_if(in_state(crate::settle::Phase::Playing)),
        )
        .add_systems(
            Update,
            summon
                .after(tick)
                .run_if(in_state(crate::settle::Phase::Playing)),
        );
}

fn attach(
    mut commands: Commands,
    foes: Query<(Entity, &Species), (With<Hostile>, Without<Named>)>,
) {
    for (e, s) in &foes {
        if has_mechanics(s.0.name) {
            commands.entity(e).insert(Named {
                timers: vec![0.0; timed(s.0.name).len()],
                ..Named::default()
            });
        }
    }
}

/// A number in `lo..=hi` from the creature's own clock (no shared random stream needed for timers).
fn between((lo, hi): (f32, f32), seed: f32) -> f32 {
    lo + (hi - lo) * (seed.sin() * 43_758.547).fract().abs()
}

#[allow(clippy::type_complexity)]
fn tick(
    mut commands: Commands,
    time: Res<Time>,
    roster: Option<Res<Roster>>,
    fx: Option<Res<Fx>>,
    mut audio: Option<ResMut<crate::audio::Audio>>,
    avatar: Query<&Transform, With<Avatar>>,
    mut foes: Query<(
        Entity,
        &Species,
        &mut Brain,
        &Hostile,
        &Transform,
        &Reaction,
        &mut Named,
    )>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let player = avatar.single().ok();
    for (e, species, mut brain, hostile, xf, reaction, mut named) in &mut foes {
        let name = species.0.name;
        if *reaction == Reaction::Dead {
            continue;
        }
        if hostile.alert && !named.alerted {
            named.alerted = true;
            named.engaged = 0.0;
            for (t, m) in named.timers.iter_mut().zip(timed(name)) {
                *t = m.first;
            }
            if let Some(chance) = rush_chance(name) {
                // `WOWSOULS_RUSH=1`: every boar rushes (an unattended-run instrument).
                let always = std::env::var_os("WOWSOULS_RUSH").is_some();
                if always
                    || ((now * 7.31 + e.index_u32() as f32).sin() * 43_758.547)
                        .fract()
                        .abs()
                        * 100.0
                        < chance as f32
                {
                    commands.entity(e).insert(crate::abilities::Slowed {
                        left: RUSH_SECS,
                        factor: RUSH_FACTOR,
                    });
                    if let Some(fx) = fx.as_deref() {
                        fx::spawn(&mut commands, fx, Kind::SprintCast, e);
                    }
                    fx::sound(audio.as_deref_mut(), SOUND_RUSH, "rushing charge");
                    info!("named: {name} rushes (Rushing Charge)");
                }
            }
        } else if !hostile.alert && named.alerted {
            named.alerted = false;
            named.primed = None;
            brain.throw = None;
            brain.start = None;
            continue;
        }
        if !named.alerted {
            continue;
        }
        named.engaged += dt;
        let kit = roster.as_deref().and_then(|r| r.get(species.kit()));
        let dist = player.map_or(f32::MAX, |p| {
            Vec2::new(
                p.translation.x - xf.translation.x,
                p.translation.z - xf.translation.z,
            )
            .length()
        });
        for i in 0..named.timers.len() {
            named.timers[i] -= dt;
            if named.timers[i] > 0.0 {
                continue;
            }
            let m = timed(name)[i];
            let done = match m.act {
                Act::Thrash => match (brain.mind, kit) {
                    (Mind::Attack { mv, .. }, Some(k)) if brain.forced.is_none() => {
                        // The other swing, straight after this one.
                        let other = (0..k.attacks.len())
                            .find(|&j| j != usize::from(mv))
                            .and_then(|j| u8::try_from(j).ok());
                        brain.forced = other;
                        if other.is_some() {
                            info!("named: {name} thrashes");
                        }
                        other.is_some()
                    }
                    _ => false,
                },
                Act::Gouge | Act::Poison { .. } | Act::Pierce | Act::Disarm => {
                    if named.primed.is_none() {
                        named.primed = Some(m.act);
                        true
                    } else {
                        false
                    }
                }
                Act::Throw { kind, mv } => {
                    let index = kit
                        .and_then(|k| k.attack_index(mv))
                        .and_then(|j| u8::try_from(j).ok());
                    match index {
                        Some(j)
                            if matches!(brain.mind, Mind::Chase)
                                && brain.start.is_none()
                                && (THROW_BAND.0..=THROW_BAND.1).contains(&dist) =>
                        {
                            brain.start = Some(j);
                            brain.throw = Some(kind);
                            info!("named: {name} throws its {kind:?} ({dist:.1} yd)");
                            true
                        }
                        _ => false,
                    }
                }
            };
            if done {
                named.timers[i] = between(m.every, now + i as f32 + e.index_u32() as f32);
            }
        }
    }
}

/// The Dockmaster calls its Bodyguards.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn summon(
    mut commands: Commands,
    roster: Option<Res<Roster>>,
    catalogs: Option<Res<crate::actor::Catalogs>>,
    asset_server: Res<AssetServer>,
    collide: benilla_world::collision::WorldCollision,
    mut epoch: ResMut<benilla_world::collision::ColliderEpoch>,
    mut foes: Query<(Entity, &Species, &Transform, &mut Named)>,
) {
    let Some(catalogs) = catalogs else {
        return;
    };
    for (e, species, xf, mut named) in &mut foes {
        let Some((def, count, after)) = summons(species.0.name) else {
            continue;
        };
        if !named.alerted || named.summoned || named.engaged < after {
            continue;
        }
        named.summoned = true;
        for k in 0..count {
            // Beside it: behind and to either side.
            let angle = std::f32::consts::TAU * (k as f32 + 0.5) / count as f32;
            let at = xf.translation + Vec3::new(angle.cos(), 0.0, angle.sin()) * 2.5;
            let wow = benilla_assets::coords::bevy_to_wow(at);
            if call(
                &mut commands,
                def,
                [wow[0], wow[1], wow[2] + 1.0],
                None,
                e.to_bits() ^ k as u64,
                SUMMON_LINGER,
                (
                    roster.as_deref(),
                    &catalogs,
                    &asset_server,
                    &collide,
                    &mut epoch,
                ),
            )
            .is_some()
            {
                info!("named: {} calls a {}", species.0.name, def.title);
            }
        }
    }
}

pub(crate) type Placing<'a, 'w, 's> = (
    Option<&'a Roster>,
    &'a crate::actor::Catalogs,
    &'a AssetServer,
    &'a benilla_world::collision::WorldCollision<'w, 's>,
    &'a mut benilla_world::collision::ColliderEpoch,
);

/// Puts `def` down at `wow` (WoW coordinates), engaged with the player, going `linger` seconds after its fight. Its
/// entity, or `None` when it could not be placed.
pub(crate) fn call(
    commands: &mut Commands,
    def: &'static crate::species::SpeciesDef,
    wow: [f32; 3],
    facing: Option<f32>,
    seed: u64,
    linger: f32,
    (roster, catalogs, asset_server, collide, epoch): Placing,
) -> Option<Entity> {
    match crate::npc::place(
        commands,
        def,
        None,
        wow,
        facing,
        crate::roam::Area::Stand,
        seed,
        false,
        roster,
        catalogs,
        asset_server,
        collide,
        epoch,
    ) {
        Ok(placed) => {
            commands.entity(placed.entity).insert((
                Summoned {
                    left: linger,
                    linger,
                },
                Brain {
                    mind: Mind::Chase,
                    ..Brain::default()
                },
            ));
            Some(placed.entity)
        }
        Err(why) => {
            warn!("named: could not call a {}: {why:?}", def.title);
            None
        }
    }
}

/// Clara Charles has sprung her ambush.
#[derive(Component)]
pub struct Ambushed;

/// The player comes near Clara Charles: a Defias Ambusher steps out.
#[allow(clippy::type_complexity)]
fn ambush(
    mut commands: Commands,
    roster: Option<Res<Roster>>,
    catalogs: Option<Res<crate::actor::Catalogs>>,
    asset_server: Res<AssetServer>,
    collide: benilla_world::collision::WorldCollision,
    mut epoch: ResMut<benilla_world::collision::ColliderEpoch>,
    avatar: Query<&Transform, With<Avatar>>,
    clara: Query<(Entity, &crate::friendly::Npc, &Transform), Without<Ambushed>>,
) {
    let (Some(catalogs), Ok(player)) = (catalogs, avatar.single()) else {
        return;
    };
    for (e, npc, xf) in &clara {
        if npc.entry != CLARA || xf.translation.distance(player.translation) > AMBUSH_RANGE {
            continue;
        }
        commands.entity(e).insert(Ambushed);
        if call(
            &mut commands,
            &crate::species::AMBUSHER,
            AMBUSH_AT,
            Some(AMBUSH_FACING),
            u64::from(CLARA),
            AMBUSH_LINGER,
            (
                roster.as_deref(),
                &catalogs,
                &asset_server,
                &collide,
                &mut epoch,
            ),
        )
        .is_some()
        {
            info!("named: Clara Charles is ambushed: a Defias Ambusher steps out");
        }
    }
}

/// The player's weapon is taken for this long (s).
#[derive(Resource, Debug, Default)]
pub struct Disarmed(pub f32);

/// The fists' share of a blow while disarmed.
pub fn disarm_factor(d: &Disarmed) -> f32 {
    if d.0 > 0.0 {
        DISARM_FACTOR
    } else {
        1.0
    }
}

/// The weapon is hidden while it is taken and comes back with the time.
fn disarm_tick(
    time: Res<Time>,
    mut disarmed: ResMut<Disarmed>,
    mut hidden: Local<bool>,
    avatar: Query<&crate::actor::held::HeldBuilt, With<Avatar>>,
    mut vis: Query<&mut Visibility>,
) {
    if disarmed.0 > 0.0 {
        disarmed.0 = (disarmed.0 - time.delta_secs()).max(0.0);
    }
    let want = disarmed.0 > 0.0;
    if want == *hidden {
        return;
    }
    if let Ok(held) = avatar.single() {
        if let Ok(mut v) = vis.get_mut(held.root) {
            *v = if want {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            *hidden = want;
            if !want {
                info!("named: the player takes up the weapon again");
            }
        }
    }
}

/// A summon goes when its time is up after the fight (or its death).
fn linger(
    mut commands: Commands,
    time: Res<Time>,
    mut summoned: Query<(
        Entity,
        &mut Summoned,
        &Hostile,
        &Reaction,
        Option<&crate::npc::BodyCollider>,
    )>,
) {
    for (e, mut s, hostile, reaction, body) in &mut summoned {
        if hostile.alert && *reaction != Reaction::Dead {
            s.left = s.linger;
            continue;
        }
        s.left -= time.delta_secs();
        if s.left <= 0.0 {
            if let Some(b) = body {
                commands.entity(b.0).despawn();
            }
            commands.entity(e).despawn();
        }
    }
}

/// A landed blow of a creature with a primed Poison poisons the player.
fn poison_on_hurt(
    mut hurts: MessageReader<PlayerHurt>,
    mut foes: Query<(&Species, &mut Named)>,
    mut poison: ResMut<PlayerPoison>,
    mut shred: ResMut<crate::boss::ArmorShred>,
    mut disarmed: ResMut<Disarmed>,
) {
    for h in hurts.read() {
        if h.blocked || h.level == 0 {
            continue;
        }
        let Ok((species, mut named)) = foes.get_mut(h.from) else {
            continue;
        };
        if let Some(Act::Poison { per_tick, ticks }) = named.primed {
            named.primed = None;
            *poison = PlayerPoison {
                from: Some(h.from),
                per_tick,
                ticks,
                timer: POISON_EVERY,
            };
            info!("named: {} poisons the player", species.0.name);
        }
        if named.primed == Some(Act::Pierce) {
            named.primed = None;
            for _ in 0..PIERCE_STACKS {
                shred.add();
            }
            info!(
                "named: {} pierces the player's armor (x{})",
                species.0.name,
                shred.stacks()
            );
        }
        if named.primed == Some(Act::Disarm) {
            named.primed = None;
            disarmed.0 = DISARM_SECS;
            info!("named: {} disarms the player", species.0.name);
        }
    }
}

fn poison_tick(
    time: Res<Time>,
    mut poison: ResMut<PlayerPoison>,
    avatar: Query<&crate::combat::Action, With<Avatar>>,
    mut hurts: MessageWriter<PlayerHurt>,
) {
    let (Some(from), true) = (poison.from, poison.ticks > 0) else {
        return;
    };
    if avatar.single().is_ok_and(|a| a.is_dead()) {
        *poison = PlayerPoison::default();
        return;
    }
    poison.timer -= time.delta_secs();
    if poison.timer > 0.0 {
        return;
    }
    poison.timer += POISON_EVERY;
    poison.ticks -= 1;
    hurts.write(PlayerHurt {
        from,
        damage: poison.per_tick,
        level: 0,
        stop: 0.0,
        knock: 0.0,
        poise: 0.0,
        blocked: false,
    });
    info!("named: the poison stings for {:.0}", poison.per_tick);
    if poison.ticks == 0 {
        poison.from = None;
    }
}

fn root_tick(time: Res<Time>, mut rooted: ResMut<Rooted>) {
    if rooted.0 > 0.0 {
        rooted.0 = (rooted.0 - time.delta_secs()).max(0.0);
    }
}

/// A net or a web caught the player: their feet are held ([`ROOT_SECS`]) and the throw's model wraps them.
pub fn catch(
    commands: &mut Commands,
    fx: Option<&Fx>,
    rooted: &mut Rooted,
    player: Entity,
    kind: Throw,
) {
    rooted.0 = ROOT_SECS;
    if let Some(fx) = fx {
        fx::spawn_for(commands, fx, kind.state(), player, ROOT_SECS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blow_from_behind_is_a_backstab_and_from_the_front_is_not() {
        // The player at the origin facing -Z (Bevy forward).
        let player = Transform::default();
        let (behind, _) = on_blow(None, "cutpurse", Vec3::new(0.0, 0.0, 1.5), &player, 20.0, 1);
        let (front, _) = on_blow(
            None,
            "cutpurse",
            Vec3::new(0.0, 0.0, -1.5),
            &player,
            20.0,
            1,
        );
        let (plain, _) = on_blow(None, "thug", Vec3::new(0.0, 0.0, 1.5), &player, 20.0, 1);
        assert_eq!((behind, front, plain), (30.0, 20.0, 20.0));
    }

    #[test]
    fn a_primed_gouge_turns_the_next_blow_into_a_knockdown_once() {
        let player = Transform::default();
        let mut n = Named {
            primed: Some(Act::Gouge),
            ..Named::default()
        };
        let from = Vec3::new(0.0, 0.0, -1.5);
        assert_eq!(
            on_blow(Some(&mut n), "morgan", from, &player, 20.0, 1).1,
            GOUGE_LEVEL
        );
        assert_eq!(on_blow(Some(&mut n), "morgan", from, &player, 20.0, 1).1, 1);
    }

    #[test]
    fn the_named_mechanics_belong_to_their_creatures() {
        assert_eq!(timed("outrunner"), &[THRASH]);
        assert!(timed("thug").is_empty());
        assert_eq!(rush_chance("stonetusk"), Some(15));
        assert!(summons("dockmaster").is_some_and(|(_, n, _)| n == 3));
        assert!(timed("ruklar").iter().any(|t| t.act == Act::Pierce));
        assert!(timed("bodyguard").iter().any(|t| t.act == Act::Disarm));
        assert_eq!(disarm_factor(&Disarmed(0.0)), 1.0);
        assert_eq!(disarm_factor(&Disarmed(2.0)), DISARM_FACTOR);
        assert!(has_mechanics("mother_fang") && has_mechanics("lurker") && !has_mechanics("wolf"));
        for (lo, hi) in [(15.0, 20.0), (21.0, 31.0)] {
            for s in 0..20 {
                let v = between((lo, hi), s as f32);
                assert!((lo..=hi).contains(&v), "{v}");
            }
        }
    }
}
