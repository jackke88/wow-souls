//! Blood: a landed blow, either way, shows the spurt WoW 1.12 itself plays. The chain is the
//! client's (`0x624530` -> `0x625010`, benilla's `creature_anim/blood.rs` @ 2e82d34, which is
//! crate-private): the victim's `CreatureDisplayInfo.BloodLevel`, else its model's `BloodID`, else the
//! first `UnitBloodLevels` row; that row's `UnitBlood` id at the violence level; its front or back,
//! small or large `SpellVisualEffectName`; the model path. The spurt is a particle-only M2 hung on the
//! victim's chest (attachment 15) or back (16) by which side the attacker stands on.
//!
//! The effect lane of benilla-app is crate-private and heavy (parts, materials, a rig). A spurt model has
//! only emitters, and the engine's `spawn_emitter` is public, so each emitter rides a small root entity
//! under the victim's attachment anchor and the root is despawned when the burst is over.
//! `WOWSOULS_BLOOD=0` turns it off; any other number scales how much blood there is (default 1).
//!
//! WoW's own spurt is thin: one droplet emitter (alpha blend, 100 a second for a third of a second,
//! half-extent 0.03 yd). It is turned up here: the droplet emitters are spawned [`DROPLET_COPIES`]
//! times (each with its own random seed) at [`DROPLET_SIZE`] times the size; the additive flash
//! emitters are left as they are.

use bevy::prelude::*;

use benilla_assets::{m2_url, LockRecover, M2Model};
use benilla_formats::{
    load_blood_catalog, load_spell_visual_catalog, BloodCatalog, CreatureCatalog, ParticleBlend,
    SpellVisualCatalog,
};
use benilla_world::particles::{
    spawn_emitter, EmitClock, EmitterFrames, OwnerLoss, ParticleEmitter,
};
use benilla_world::rig_anim::RigPose;

use crate::actor::{ActorModel, Catalogs};
use crate::avatar::Avatar;
use crate::hits::Hit;
use crate::npc::PlayerHurt;
use crate::settle::Phase;

/// The client's default gore (`violenceLevel`), 2 for enUS: true colours.
const VIOLENCE: usize = 2;
/// The victim attachments the spurt hangs on: front and back of the chest.
const ATTACH_FRONT: u16 = 15;
const ATTACH_BACK: u16 = 16;
/// How many times the droplet emitters are spawned at `WOWSOULS_BLOOD=1`, and how much bigger each
/// droplet streak is than WoW's. Tuning values (taste, not data).
pub const DROPLET_COPIES: f32 = 3.0;
pub const DROPLET_SIZE: f32 = 1.6;
/// How long a built spurt lives (s): its longest rate track (0.67 s) and its longest particle (0.5 s),
/// rounded up. The live trace shows the particles gone before it.
pub const SPURT_LIFE: f32 = 1.6;
/// How long a spurt may wait for a model that never loads (s).
const LOAD_TIMEOUT: f32 = 10.0;

/// The two tables the spurt reads; absent, there is no blood and the gizmo cross stays.
#[derive(Resource)]
pub struct BloodTables {
    blood: BloodCatalog,
    visuals: SpellVisualCatalog,
    /// `WOWSOULS_BLOOD`: how much blood, 1 by default.
    amount: f32,
}

impl BloodTables {
    pub fn load(chain: &mut benilla_formats::Chain, amount: f32) -> anyhow::Result<Self> {
        Ok(Self {
            blood: load_blood_catalog(chain)?,
            visuals: load_spell_visual_catalog(chain)?,
            amount,
        })
    }

    /// How many copies of each droplet emitter one spurt gets.
    pub fn copies(&self) -> usize {
        (DROPLET_COPIES * self.amount).round().max(1.0) as usize
    }

    /// The spurt model's path for a body of `display`, as the tables name it (`.mdl` or `.mdx`;
    /// [`m2_url`] maps either to the archive's `.m2`).
    pub fn spurt(
        &self,
        creatures: &CreatureCatalog,
        display: u32,
        front: bool,
        large: bool,
    ) -> Option<String> {
        let m = creatures.model(display)?;
        let key = self.blood.level_key(m.blood_display, m.blood_model)?;
        let id = self.blood.effect_id(key as i32, VIOLENCE, front, large)?;
        self.visuals.effect_path(id).map(str::to_owned)
    }
}

/// Whether the attacker stands in front of the victim: the sign of the flat dot of the victim's
/// forward and the way to the attacker, `>= 0` front. An unknown attacker counts as front, as the
/// client's does.
pub fn is_front(victim_forward: Vec3, victim: Vec3, attacker: Option<Vec3>) -> bool {
    let Some(a) = attacker else {
        return true;
    };
    let to = Vec3::new(a.x - victim.x, 0.0, a.z - victim.z);
    let fwd = Vec3::new(victim_forward.x, 0.0, victim_forward.z);
    fwd.dot(to) >= 0.0
}

/// A flinch level 2 or more picks the large spurt: the stand-in for the client's crushing bit, which our
/// blows do not carry (R2, the kobold's `big`, `run_strike` and `body_check`).
pub fn is_large(level: u8) -> bool {
    level >= 2
}

/// One spurt: its root entity is a child of the victim's attachment anchor.
#[derive(Component, Debug)]
struct Spurt {
    victim: Entity,
    front: bool,
    model: Handle<M2Model>,
    /// Seconds since it was asked, or since it was built.
    age: f32,
    built: bool,
}

/// One blow to show: who bled, where the attacker stood, how hard.
struct Blow {
    victim: Entity,
    from: Option<Vec3>,
    level: u8,
}

/// `WOWSOULS_BLOOD`: unset is 1; a number is how much blood; 0 (or less) is none.
pub fn amount_from(var: Option<&str>) -> f32 {
    var.and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|a| a.is_finite())
        .unwrap_or(1.0)
        .min(10.0)
}

fn load_tables(mut commands: Commands, catalogs: Option<Res<Catalogs>>) {
    let amount = amount_from(std::env::var("WOWSOULS_BLOOD").ok().as_deref());
    if amount <= 0.0 {
        info!("blood: off (WOWSOULS_BLOOD=0)");
        return;
    }
    let Some(catalogs) = catalogs else {
        warn!("blood: no catalogs; no spurts");
        return;
    };
    let loaded = BloodTables::load(&mut catalogs.chain.lock_recover(), amount);
    match loaded {
        Ok(t) => {
            info!("blood: tables loaded, amount {amount}");
            commands.insert_resource(t);
        }
        Err(e) => warn!("blood: the tables did not load ({e:#}); no spurts"),
    }
}

/// Asks a spurt for each landed blow: the player's on a creature ([`Hit`]), a creature's on the player
/// ([`PlayerHurt`]). A new spurt on the same body and side replaces the old one, so a busy fight does not
/// stack them.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn on_blows(
    mut commands: Commands,
    tables: Res<BloodTables>,
    catalogs: Option<Res<Catalogs>>,
    asset_server: Res<AssetServer>,
    m2s: Res<Assets<M2Model>>,
    mut dealt: MessageReader<Hit>,
    mut taken: MessageReader<PlayerHurt>,
    avatar: Query<Entity, With<Avatar>>,
    places: Query<&Transform>,
    mut bodies: Query<(&Transform, &ActorModel, &mut RigPose)>,
    spurts: Query<(Entity, &Spurt)>,
) {
    let me = avatar.single().ok();
    let mut blows: Vec<Blow> = dealt
        .read()
        .map(|h| Blow {
            victim: h.target,
            from: Some(h.from),
            level: h.level,
        })
        .collect();
    for p in taken.read() {
        if p.blocked {
            continue;
        }
        if let Some(me) = me {
            blows.push(Blow {
                victim: me,
                from: places.get(p.from).ok().map(|t| t.translation),
                level: p.level,
            });
        }
    }
    let Some(catalogs) = catalogs else {
        return;
    };
    for blow in blows {
        let Ok((xf, model, mut pose)) = bodies.get_mut(blow.victim) else {
            continue;
        };
        let front = is_front(*xf.forward(), xf.translation, blow.from);
        let large = is_large(blow.level);
        let who = if Some(blow.victim) == me {
            "player"
        } else {
            "creature"
        };
        let Some(path) = tables.spurt(&catalogs.creatures, model.display, front, large) else {
            info!(
                "blood: {who}: no spurt for display {} (front {front}, large {large})",
                model.display
            );
            continue;
        };
        let attach = if front { ATTACH_FRONT } else { ATTACH_BACK };
        let Some(point) = m2s
            .get(&model.m2)
            .and_then(|m| m.attachments.iter().find(|a| a.id == attach).copied())
        else {
            info!("blood: {who}: the body has no attachment {attach}");
            continue;
        };
        let Some(joint) = pose.anchor_for(&mut commands, blow.victim, point.bone) else {
            continue;
        };
        for (old, s) in &spurts {
            if s.victim == blow.victim && s.front == front {
                commands.entity(old).despawn();
            }
        }
        let root = commands
            .spawn((
                Name::new("Blood spurt"),
                Transform::from_translation(point.offset),
                Visibility::default(),
                Spurt {
                    victim: blow.victim,
                    front,
                    model: asset_server.load(m2_url(&path)),
                    age: 0.0,
                    built: false,
                },
            ))
            .id();
        commands.entity(joint).add_child(root);
        info!(
            "blood: {who} {} {} {path}",
            if front { "front" } else { "back" },
            if large { "large" } else { "small" }
        );
    }
}

/// When a spurt's model has loaded, one emitter per `ModelEmitter` rides its root.
fn build_spurts(
    mut commands: Commands,
    tables: Res<BloodTables>,
    m2s: Res<Assets<M2Model>>,
    mut spurts: Query<(Entity, &mut Spurt)>,
) {
    for (root, mut s) in &mut spurts {
        if s.built {
            continue;
        }
        let Some(model) = m2s.get(&s.model) else {
            continue;
        };
        s.built = true;
        s.age = 0.0;
        let mut made = 0;
        for em in &model.emitters {
            // The droplets (alpha blend) come in several copies and bigger; the flash (additive) once.
            let droplets = em.def.blend == ParticleBlend::Alpha;
            for copy in 0..if droplets { tables.copies() } else { 1 } {
                let frames = EmitterFrames {
                    owner: Some((root, [0.0; 3])),
                    anchor: Some(root),
                    alpha: None,
                    light_node: None,
                    on_owner_loss: OwnerLoss::Free,
                };
                // The placement seeds the emitter's random numbers: a copy needs its own.
                let seed = Transform::from_translation(Vec3::splat(copy as f32 * 0.0137));
                let Some(e) =
                    spawn_emitter(&mut commands, em, seed, frames, EmitClock::Effect(None))
                else {
                    continue;
                };
                made += 1;
                if droplets {
                    commands.entity(e).queue(|mut e: EntityWorldMut| {
                        if let Some(mut p) = e.get_mut::<ParticleEmitter>() {
                            p.set_size_scale(DROPLET_SIZE);
                        }
                    });
                }
            }
        }
        info!(
            "blood: spurt built, {made} emitters from the model's {}",
            model.emitters.len()
        );
    }
}

/// A spurt is gone [`SPURT_LIFE`] after it was built (its emitters with it), or [`LOAD_TIMEOUT`] after it
/// was asked if its model never came.
fn reap_spurts(mut commands: Commands, time: Res<Time>, mut spurts: Query<(Entity, &mut Spurt)>) {
    for (root, mut s) in &mut spurts {
        s.age += time.delta_secs();
        let limit = if s.built { SPURT_LIFE } else { LOAD_TIMEOUT };
        if s.age > limit {
            commands.entity(root).despawn();
        }
    }
}

/// `WOWSOULS_TRACE`: the live particles of the spurts, every 0.1 s while there are any.
fn trace_spurts(
    real: Res<Time<bevy::time::Real>>,
    spurts: Query<(), With<Spurt>>,
    emitters: Query<&ParticleEmitter>,
    mut next_at: Local<f32>,
) {
    let now = real.elapsed_secs();
    if now < *next_at {
        return;
    }
    *next_at = now + 0.1;
    let live: usize = emitters
        .iter()
        .filter(|e| e.anchor().is_some_and(|a| spurts.contains(a)))
        .map(ParticleEmitter::live)
        .sum();
    if live > 0 || !spurts.is_empty() {
        info!(
            "blood: {} spurt(s), {live} live particles",
            spurts.iter().count()
        );
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Phase::Playing), load_tables)
        .add_systems(
            Update,
            (on_blows, build_spurts, reap_spurts)
                .chain()
                .run_if(in_state(Phase::Playing))
                .run_if(resource_exists::<BloodTables>),
        );
    if crate::actor::anim::trace_on() {
        app.add_systems(Update, trace_spurts.run_if(in_state(Phase::Playing)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_attacker_in_front_or_behind() {
        // A body at the origin facing -Z (Bevy forward).
        let fwd = Vec3::NEG_Z;
        let at = |x, z| Some(Vec3::new(x, 0.0, z));
        assert!(is_front(fwd, Vec3::ZERO, at(0.0, -2.0)));
        assert!(!is_front(fwd, Vec3::ZERO, at(0.0, 2.0)));
        assert!(
            is_front(fwd, Vec3::ZERO, at(2.0, 0.0)),
            "abeam counts as front"
        );
        assert!(
            is_front(fwd, Vec3::ZERO, None),
            "an unknown attacker counts as front"
        );
        // Height does not matter.
        assert!(!is_front(fwd, Vec3::ZERO, Some(Vec3::new(0.0, 5.0, 1.0))));
    }

    #[test]
    fn the_amount_scales_the_droplet_copies() {
        assert_eq!(amount_from(None), 1.0);
        assert_eq!(amount_from(Some("2")), 2.0);
        assert_eq!(amount_from(Some("0")), 0.0);
        assert_eq!(amount_from(Some("junk")), 1.0);
        assert_eq!(amount_from(Some("99")), 10.0, "capped");
    }

    #[test]
    fn heavier_blows_pick_the_large_spurt() {
        assert!(!is_large(0) && !is_large(1));
        assert!(is_large(2) && is_large(5));
    }

    /// The kobold and the human male both bleed red: the table chain reaches the spurt models.
    #[test]
    fn install_the_two_bodies_resolve_to_the_red_spurt() {
        let Some(data) = benilla_formats::wow_data() else {
            eprintln!("skipped: no WoW install");
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let tables = BloodTables::load(&mut chain, 1.0).unwrap();
        let creatures = benilla_formats::load_creature_catalog(&mut chain).unwrap();
        for display in [365, 49] {
            for (front, large, want) in [
                (true, false, "mpq://particles/bloodspurts/bloodspurt.m2"),
                (true, true, "mpq://particles/bloodspurts/bloodspurtlarge.m2"),
                (false, false, "mpq://particles/bloodspurts/bloodspurt.m2"),
                (
                    false,
                    true,
                    "mpq://particles/bloodspurts/bloodspurtlarge.m2",
                ),
            ] {
                let path = tables
                    .spurt(&creatures, display, front, large)
                    .unwrap_or_else(|| panic!("display {display}"));
                assert_eq!(
                    m2_url(&path),
                    want,
                    "display {display} front {front} large {large}"
                );
                assert!(
                    chain
                        .read_file(&want.trim_start_matches("mpq://").replace('/', "\\"))
                        .is_ok(),
                    "{want} is in the chain"
                );
            }
        }
        assert!(tables.spurt(&creatures, 9_999_999, true, false).is_none());
    }
}
