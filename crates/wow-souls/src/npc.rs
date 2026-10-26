//! The slice's hostiles: creatures placed by hand in Northshire, as a server would spawn them
//! (each one's [`crate::species::SpeciesDef`] says what it is; the text below says kobold where it
//! was written for the first one).
//!
//! Its [`Brain`] is a small state machine ([`think_step`], pure): it stands at home until the
//! player is seen or heard ([`perception`]), chases on the ground through the same motor the player
//! walks with, and in reach winds up a telegraphed swing (its Attack1H clip, slowed so the wind-up
//! reads). It tracks the player through the wind-up and commits from the strike on, so a roll
//! timed to the strike passes through it on the roll's invulnerability. Past [`LEASH_RANGE`], or
//! with the player dead, it walks home. A hit makes it flinch, which breaks its swing.
//!
//! The player can lock onto it ([`crate::lockon::Lockable`]) and cannot walk through it: its body
//! collider (kinematic, following it) sits on the engine's *walk* layer, which the player's sweeps
//! hit and the camera's do not, so the camera never snaps in on a passing enemy. Its own motor
//! sweeps skip that collider ([`MoverTraceExclusions`]).
//!
//! `creature_template` (which creature wears which display) is server data, not in the client:
//! the display is picked by hand from `CreatureDisplayInfo` (see [`crate::species`]).

use avian3d::prelude::*;
use bevy::animation::transition::AnimationTransitions;
use bevy::animation::RepeatAnimation;
use bevy::prelude::*;

use benilla_assets::coords::wow_to_bevy;
use benilla_assets::ModelAnimations;
use benilla_world::collision::{ColliderEpoch, MoverTraceExclusions, WorldCollision};

use crate::actor::held::{HeldBuilt, HeldItem, OffHandItem};
use crate::actor::{ActorModel, Catalogs};
use crate::avatar::{Avatar, BodyShape};
use crate::combat::{Action, Health};
use crate::controller::{turn_toward, yaw_of};
use crate::hits::Hurtbox;
use crate::lockon::Lockable;
use crate::monster::{
    choose_death, choose_react, AttackSpec, HandTrail, MonsterClip, MonsterClips, MonsterFade,
    MonsterPlay, Poise, Roster,
};
use crate::motor::{self, Motor, MoveIntent};
use crate::pattern::Rng;
use crate::roam::{Area, Walk};
use crate::settle::{snap_to_ground, Phase};
use crate::slice;
use crate::species::{Blow, Species, SpeciesDef};

mod perception;

/// A fight is over once the player is this far above or below (yd): no chasing from under the ground.
pub const LEASH_RISE: f32 = 8.0;
/// The player's chest over the feet (yd): what a creature looks at to notice the player.
const PLAYER_CHEST: f32 = 1.2;
/// Past this distance (yd) a chasing kobold gives up and walks home.
pub const LEASH_RANGE: f32 = 30.0;
/// Home is reached within this distance (yd).
const HOME_SLACK: f32 = 0.6;
/// How fast it turns to face (rad/s).
const TURN_RATE: f32 = 6.0;
/// The walk-home speed (yd/s); the chase speed is the species' ([`crate::species::SpeciesDef`]).
pub const RETURN_SPEED: f32 = crate::motor::WALK_SPEED;
/// The chase stops this far (yd, centre to centre) from the player.
pub const STOP_DISTANCE: f32 = 1.3;
/// A swing starts within this distance (yd, centre to centre) with the player this far inside
/// its facing (the cosine of the angle off its nose).
pub const ATTACK_DISTANCE: f32 = 1.8;
const ATTACK_FACING: f32 = 0.85;
/// Attack1H, its swing (AnimationData.dbc), and the slowed rate that makes the wind-up readable.
pub const KOBOLD_ATTACK: u16 = 17;
pub const KOBOLD_ATTACK_RATE: f32 = 0.7;
/// The swing's strike window as fractions of its length: it tracks the player before
/// [`STRIKE_FROM`] and can hit until [`STRIKE_TO`]. Measured on the clip (`WOWSOULS_TRACE` swing
/// lines): the right hand rises behind the head to 0.28, sweeps forward from 0.34 (0.35 yd ahead)
/// through its reach at 0.42 (0.85 yd ahead, crossing right to left) and is back beside the
/// body by 0.48.
pub const STRIKE_FROM: f32 = 0.34;
pub const STRIKE_TO: f32 = 0.48;
/// The swing reaches a body whose edge is within this distance (yd) of the kobold's centre and
/// inside this cone off its nose (cosine of the half angle).
pub const STRIKE_REACH: f32 = 1.6;
const STRIKE_CONE: f32 = 0.5;
/// What a strike does to the player.
pub const KOBOLD_DAMAGE: f32 = 25.0;
/// The pause after a swing before the next chase (s).
pub const COOLDOWN: f32 = 1.2;
/// The same with Dark Souls clips, which carry their own recovery.
const CLIP_COOLDOWN: f32 = 0.3;
/// The pause after a cast ([`crate::foebolt`]).
const CAST_COOLDOWN: f32 = 1.8;
/// A kit move's lunge stops this far (yd, edge to edge) short of a player ahead of it: the bodies
/// do not collide, so a run-and-strike would carry the kobold through the player.
const LUNGE_STOP_GAP: f32 = 0.3;
/// The engine's walk-only collision layer: bit 1 of `benilla_world::collision::CollisionLayer`
/// (`Default`, `Walk`, `Camera`, `Liquid`), which is crate-private, so the bit is restated here.
/// The body sweep (`Default | Walk`) hits it; the camera sweep (`Default | Camera`) does not.
const WALK_LAYER_BIT: u32 = 1 << 1;

/// CombatWound and Death (AnimationData.dbc).
pub const COMBAT_WOUND: u16 = 9;
pub const DEATH: u16 = 1;

/// A hostile creature.
#[derive(Component, Debug, Default)]
pub struct Hostile {
    /// Engaged with the player (chasing, swinging or between swings).
    pub alert: bool,
}

/// The creature's body collider, a separate kinematic entity that follows it; gone while dead.
#[derive(Component, Debug, Clone, Copy)]
pub struct BodyCollider(pub Entity);

/// Where a hostile belongs and what it is made of, for the walk home and the respawn.
#[derive(Component, Debug, Clone, Copy)]
pub struct Home {
    pub feet: Vec3,
    pub facing: f32,
    pub lockable: Lockable,
    pub hurtbox: Hurtbox,
    /// The body collider's radius and height (yd).
    pub body: (f32, f32),
}

/// Where a creature walks while nothing is after it ([`crate::roam`]): its area, its walker's
/// bookkeeping and its own dice (apart from the fight's [`crate::monster::FoeRng`], so a roaming
/// valley does not change a seeded fight).
#[derive(Component, Debug, Clone)]
pub struct Roam {
    pub area: Area,
    pub walk: Walk,
    pub rng: Rng,
}

impl Roam {
    /// A walker for `area` whose dice are seeded from `seed` (the spawn's guid): it first stands a
    /// random part of a pause, so creatures spawned together do not set off together.
    pub fn new(area: Area, seed: u64) -> Roam {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03);
        let walk = Walk::staggered(&mut rng);
        Roam { area, walk, rng }
    }
}

/// A spawn's ground: the snapped height, or under `strict` (a ring spawn) nothing when no ground
/// was found within [`GROUND_TOLERANCE`] of the database's height. The database's height is the
/// server's own and right to a yard; a ray that only found ground far from it hit a hill over a cave
/// or a roof, and a spawn there would stand in the wrong place, so the ring waits for the colliders
/// under the real spot. The old hand-placed groups keep their seed height when nothing is found.
pub fn pick_ground(seed: f32, found: Option<f32>, strict: bool) -> Option<f32> {
    match (found, strict) {
        (Some(y), false) => Some(y),
        (None, false) => Some(seed),
        (Some(y), true) if (y - seed).abs() <= GROUND_TOLERANCE => Some(y),
        (_, true) => None,
    }
}

/// How far (yd) a ring spawn's ground may lie from its database height.
pub const GROUND_TOLERANCE: f32 = 4.0;

/// What a hit did to the creature.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub enum Reaction {
    #[default]
    None,
    /// Playing its wound clip for this many more seconds.
    Flinch(f32),
    /// Dead: its death played, it stays down.
    Dead,
}

/// What the creature is minded to do.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Mind {
    #[default]
    Idle,
    Chase,
    /// Swinging move `mv` (an index into the kit's attacks; 0 for the procedural swing), `t`
    /// seconds in.
    Attack {
        t: f32,
        mv: u8,
    },
    /// Catching its breath after a swing.
    Cooldown {
        left: f32,
    },
    /// Walking home.
    Return,
}

impl Mind {
    pub fn engaged(self) -> bool {
        matches!(
            self,
            Mind::Chase | Mind::Attack { .. } | Mind::Cooldown { .. }
        )
    }
}

/// A hostile's mind and its current swing's bookkeeping.
#[derive(Component, Debug, Default)]
pub struct Brain {
    pub mind: Mind,
    /// The hit windows of this swing that have landed (bit `i` = window `i`): each strikes once.
    pub(crate) struck: u32,
    /// This swing has passed through a roll's invulnerability.
    pub(crate) dodged: bool,
    /// The move it wishes for next and the one it made last (kit indices).
    pub wish: Option<u8>,
    pub last: Option<u8>,
    /// Where it puts its feet between swings ([`crate::footwork`]).
    pub foot: crate::footwork::Foot,
    /// Family movement phases and fair attack admission, separate from the clip clock.
    pub tactic: crate::behavior::State,
    /// The follow-up this swing planned as it began (a kit index), and the hold it takes at its
    /// tell: `(clip second, seconds left)` ([`crate::pattern::Style`]).
    pub(crate) next: Option<u8>,
    pub(crate) hold: Option<(f32, f32)>,
    /// A leap that landed holds on to the player ([`crate::latch`]); the move it makes once it lets
    /// go (the hop back), a kit index.
    pub(crate) latch: Option<crate::latch::Latch>,
    pub(crate) forced: Option<u8>,
    /// A move to start now whatever the distance (a kit index), and the throw it makes instead of a blow
    /// ([`crate::named`]: Ruklar's Net, Mother Fang's Web).
    pub(crate) start: Option<u8>,
    pub(crate) throw: Option<crate::named::Throw>,
    /// A boar's charge run under way ([`crate::boar_rush`]).
    pub(crate) rush: Option<crate::boar_rush::Rush>,
}

impl Brain {
    /// The swing owns the creature's animation.
    pub fn owns_body(&self) -> bool {
        matches!(self.mind, Mind::Attack { .. })
    }
}

/// What the creature perceives this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sense {
    /// Ground distance to the player (yd, centre to centre).
    pub dist: f32,
    /// How square the player is to its nose (cosine).
    pub facing: f32,
    pub player_alive: bool,
    /// Ground distance to its home (yd).
    pub home_dist: f32,
    /// The move it would start now, if any (a kit index; 0 for the procedural swing).
    pub attack: Option<u8>,
    /// The pause after a swing (s).
    pub cooldown: f32,
    /// Nothing solid between them (sight and hearing share world occlusion). An engaged creature
    /// keeps this true: acquisition is separate from combat memory.
    pub seen: bool,
    /// How far the player is above or below it (yd).
    pub rise: f32,
    /// How far the player's current movement or action is audible (yd).
    pub noise: f32,
}

impl Sense {
    fn detection(self) -> Option<perception::Detection> {
        perception::detect(self.dist, self.facing, self.rise, self.seen, self.noise)
    }
}

/// What [`think_step`] started or finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    None,
    AttackStarted,
    AttackEnded,
}

/// One frame of the creature's mind; `attack_len` is the current swing's length (s).
pub fn think_step(mind: &mut Mind, s: Sense, attack_len: f32, dt: f32) -> Cue {
    // A swing, once begun, is seen through.
    if let Mind::Attack { t, .. } = mind {
        *t += dt;
        if *t >= attack_len {
            *mind = Mind::Cooldown { left: s.cooldown };
            return Cue::AttackEnded;
        }
        return Cue::None;
    }
    let stand_down = if s.home_dist > HOME_SLACK {
        Mind::Return
    } else {
        Mind::Idle
    };
    let engaged = s.player_alive && s.dist <= LEASH_RANGE && s.rise <= LEASH_RISE;
    *mind = match (*mind, s.attack) {
        (Mind::Idle | Mind::Return, _) if s.player_alive && s.detection().is_some() => Mind::Chase,
        (Mind::Idle | Mind::Return, _) => stand_down,
        (_, _) if !engaged => stand_down,
        (Mind::Chase, Some(mv)) if s.facing >= ATTACK_FACING => {
            *mind = Mind::Attack { t: 0.0, mv };
            return Cue::AttackStarted;
        }
        (Mind::Cooldown { left }, _) if left - dt > 0.0 => Mind::Cooldown { left: left - dt },
        _ => Mind::Chase,
    };
    Cue::None
}

/// The old procedural swing of length `len`: the strike window measured on its clip, one damage.
fn procedural_attack(len: f32) -> AttackSpec {
    AttackSpec {
        name: "swing".into(),
        length: len,
        hit: (STRIKE_FROM * len, STRIKE_TO * len),
        hits: vec![(STRIKE_FROM * len, STRIKE_TO * len)],
        damage: KOBOLD_DAMAGE,
        poise: 0.0,
        level: 1,
        radius: 0.0,
        radius2: 0.0,
        stop: crate::hurt::HURT_STOP,
        knock: 0.0,
    }
}

/// What a swing is: the kit's move, or the old procedural swing (its clip's length at the slowed
/// rate).
fn attack_spec(mv: u8, kit: Option<&MonsterClips>, anims: &ModelAnimations) -> AttackSpec {
    match kit.and_then(|k| k.attacks.get(usize::from(mv))) {
        Some(a) => a.spec.clone(),
        None => procedural_attack(
            anims
                .find(native_attack(anims))
                .map_or(1.0, |c| c.duration / KOBOLD_ATTACK_RATE),
        ),
    }
}

/// Beasts and mechanical bodies may expose only an unarmed or two-hand attack in their M2.
fn native_attack(anims: &ModelAnimations) -> u16 {
    [KOBOLD_ATTACK, 16, 18, 19]
        .into_iter()
        .find(|id| anims.find(*id).is_some())
        .unwrap_or(KOBOLD_ATTACK)
}

/// How long a move lasts (clip s): the clip, or, for a creature that cuts its recoveries
/// ([`crate::species::SpeciesDef::recovery`]), `recovery` after the last hit window: the long tail of
/// a Dark Souls boss move (the player's punish window there) is not waited out.
pub fn swing_length(spec: &AttackSpec, recovery: Option<f32>) -> f32 {
    let last = spec
        .hits
        .iter()
        .map(|h| h.1)
        .fold(f32::NEG_INFINITY, f32::max);
    match recovery {
        Some(r) if last.is_finite() => spec.length.min(last + r),
        _ => spec.length,
    }
}

/// Whether a swing from a body at `from` facing `fwd` reaches a target body at `to` of radius
/// `radius`.
pub fn strike_reaches(from: Vec3, fwd: Vec3, to: Vec3, radius: f32) -> bool {
    let d = to - from;
    let flat = Vec3::new(d.x, 0.0, d.z);
    let nose = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero();
    flat.length() - radius <= STRIKE_REACH && flat.normalize_or_zero().dot(nose) >= STRIKE_CONE
}

/// A creature's blow landing on the player; [`crate::hurt`] applies it.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlayerHurt {
    pub from: Entity,
    pub damage: f32,
    /// The blow's flinch level, its hit-stop on the player (s) and its push (DS metres).
    pub level: u8,
    pub stop: f32,
    pub knock: f32,
    /// Its poise damage (`atkSuperArmor`).
    pub poise: f32,
    /// Guard resolved at the collision: chip damage, no flesh impact or flinch.
    pub blocked: bool,
}

/// A creature started a swing: its wind-up is showing.
#[derive(Message, Debug, Clone, Copy)]
pub struct Telegraph {
    pub from: Entity,
}

/// A creature died of a blow; its kill is worth `souls` (the Dark Souls kit's `getSoul`, 0
/// without the cache).
#[derive(Message, Debug, Clone, Copy)]
pub struct Slain {
    pub who: Entity,
    pub souls: u32,
}

pub fn plugin(app: &mut App) {
    crate::latch::plugin(app);
    crate::slope::plugin(app);
    crate::foebolt::plugin(app);
    crate::named::plugin(app);
    crate::shock::plugin(app);
    app.add_message::<PlayerHurt>()
        .add_message::<Telegraph>()
        .add_message::<Slain>();
    if let Some(at) = std::env::var("WOWSOULS_RESET")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|s| s.is_finite())
    {
        app.insert_resource(ScriptedReset(at));
    }
    if crate::actor::anim::trace_on() {
        app.add_systems(
            Update,
            (trace_dead, trace_swing, trace_body).run_if(in_state(Phase::Playing)),
        );
    }
    // The valley from the manifest ([`crate::world`]) or the slice's hand-placed groups.
    if !crate::world::world_on() {
        app.add_systems(OnEnter(Phase::Playing), spawn);
    }
    app.add_systems(
        Update,
        (
            reset_key,
            reset,
            poise_tick,
            react,
            crate::abilities::stun,
            crate::boss::phases,
            think,
            crate::abilities::apply_slow,
            crate::shove::push_hostiles,
            exclude_bodies,
            move_hostiles,
            clear_exclusions,
            follow_bodies,
        )
            .chain()
            .after(crate::controller::PlaySet::Motor)
            .run_if(in_state(Phase::Playing)),
    )
    // The kit's blow, as this frame draws the hand.
    .add_systems(
        PostUpdate,
        strike
            .after(benilla_world::billboard::BillboardPlace)
            .run_if(in_state(Phase::Playing)),
    );
}

/// The body collider for a creature with feet at `feet`: solid to the player's sweeps only.
pub(crate) fn spawn_body(
    commands: &mut Commands,
    feet: Vec3,
    (radius, height): (f32, f32),
) -> Entity {
    commands
        .spawn((
            Name::new("Creature body"),
            Transform::from_translation(feet + Vec3::Y * (height * 0.5)),
            RigidBody::Kinematic,
            Collider::capsule(radius, (height - 2.0 * radius).max(0.0)),
            CollisionLayers::new(LayerMask(WALK_LAYER_BIT), LayerMask::ALL),
        ))
        .id()
}

/// Places the slice's hand-placed groups on the ground at their spots (the old scripted world,
/// `WOWSOULS_WORLD=0`); the actor builder dresses them.
fn spawn(
    mut commands: Commands,
    roster: Option<Res<Roster>>,
    catalogs: Option<Res<Catalogs>>,
    asset_server: Res<AssetServer>,
    collide: WorldCollision,
    mut epoch: ResMut<ColliderEpoch>,
) {
    let Some(c) = catalogs else {
        warn!("npc: no catalogs; no creatures");
        return;
    };
    // Prototype groups only; database creatures retain their authored orientation.
    let facing = std::env::var("WOWSOULS_NPC_FACING")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| v.is_finite());
    for (def, spot) in slice::spawn_plan() {
        // A missing display skips that creature, not the rest of the plan.
        let Ok(placed) = place(
            &mut commands,
            def,
            None,
            spot,
            facing,
            Area::Stand,
            0,
            false,
            roster.as_deref(),
            &c,
            &asset_server,
            &collide,
            &mut epoch,
        ) else {
            continue;
        };
        info!(
            "npc: {} (display {}, scale {}) at wow {:?}",
            def.name,
            placed.display,
            placed.scale,
            benilla_assets::coords::bevy_to_wow(placed.feet)
        );
    }
}

/// A creature [`place`] put down.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Placed {
    pub entity: Entity,
    pub feet: Vec3,
    pub display: u32,
    pub scale: f32,
}

/// Why [`place`] put nothing down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unplaced {
    /// The display has no model (warned once here).
    NoModel(u32),
    /// This display uses a body the species' kit has not been retargeted for.
    UnsupportedRig(u32),
    /// A strict spawn found no ground near its height (the colliders may still be coming).
    NoGround,
}

/// One creature of `def` standing at `spot` (WoW coords; the height is a seed the ground snaps),
/// facing `facing` (Bevy yaw; `None` faces the slice anchor, as the hand-placed groups do), roaming
/// `area` with dice seeded from `seed`. `strict_ground` (the ring) puts nothing down when no ground
/// is found near the spot's height ([`pick_ground`]), so the caller can try again later.
#[allow(clippy::too_many_arguments)]
pub(crate) fn place(
    commands: &mut Commands,
    def: &'static SpeciesDef,
    display: Option<u32>,
    spot: [f32; 3],
    facing: Option<f32>,
    area: Area,
    seed: u64,
    strict_ground: bool,
    roster: Option<&Roster>,
    catalogs: &Catalogs,
    asset_server: &AssetServer,
    collide: &WorldCollision,
    epoch: &mut ColliderEpoch,
) -> Result<Placed, Unplaced> {
    // `WOWSOULS_HOGGER=taurus` swaps Hogger back to the Taurus Demon's kit ([`crate::species::resolve`]).
    let def = crate::species::resolve(def);
    let shown = display_for(def, display.unwrap_or(def.display));
    let species = if let (Some(actual), Some(target)) = (
        catalogs.creatures.model(shown),
        catalogs.creatures.model(def.display),
    ) {
        Species::for_model(def, &target.model_path, &actual.model_path)
            .ok_or(Unplaced::UnsupportedRig(shown))?
    } else {
        Species::new(def)
    };
    if (species.kit() != def.kit
        || matches!(
            species.kit(),
            "thug_female"
                | "human_axe_2h"
                | "human_bandit"
                | "human_bandit_female"
                | "gnoll"
                | "caster"
                | "caster_female"
                | "geomancer"
                | "ambusher"
                | "murloc"
                | "boar"
                | "bear"
                | "goblin"
                | "tarantula"
                | "minespider"
        ))
        && roster.is_none_or(|r| r.get(species.kit()).is_none())
    {
        return Err(Unplaced::UnsupportedRig(shown));
    }
    let Some(mut model) =
        ActorModel::for_display(&catalogs.creatures, shown, def.look, asset_server)
    else {
        warn!("npc: display {shown} has no model");
        return Err(Unplaced::NoModel(shown));
    };
    let mut feet = wow_to_bevy(spot);
    let found = snap_to_ground(collide, feet);
    if found.is_none() && !strict_ground {
        warn!("npc: no ground under the {} at {feet:?}", def.name);
    }
    feet.y = pick_ground(feet.y, found, strict_ground).ok_or(Unplaced::NoGround)?;
    {
        // A boss is drawn larger than the client's model; every size below follows.
        model.scale *= def.size;
        let scale = model.scale;
        let (height, radius) = (def.collision.0 * scale, def.collision.1 * scale);
        // Facing the Abbey path, back toward the player's spawn, unless the spawn says otherwise.
        let facing = facing.unwrap_or_else(|| yaw_of(wow_to_bevy(slice::anchor()) - feet));
        let home = Home {
            feet,
            facing,
            lockable: Lockable {
                aim_height: height * 0.6,
                height,
            },
            // A little fuller than the body: arms and the candle hat take blows too.
            hurtbox: Hurtbox {
                radius: radius * def.hurt_grow.0,
                height: height * def.hurt_grow.1,
            },
            body: (radius, height),
        };
        let body = spawn_body(commands, feet, home.body);
        epoch.bump();
        let mut creature = commands.spawn((
            (Name::new(def.title), species),
            Transform::from_translation(feet).with_rotation(Quat::from_rotation_y(facing)),
            Visibility::default(),
            model,
            Hostile::default(),
            Brain::default(),
            Health {
                cur: def.max_health()
                    * if def.rank == crate::species::Rank::Boss {
                        crate::boss::start_fraction(
                            std::env::var("WOWSOULS_BOSS_HP").ok().as_deref(),
                        )
                    } else {
                        1.0
                    },
                max: def.max_health(),
            },
            Motor::default(),
            MoveIntent::default(),
            home.lockable,
            home.hurtbox,
            home,
            BodyCollider(body),
            Reaction::default(),
            (
                MonsterPlay::default(),
                MonsterFade::default(),
                HandTrail::default(),
                Poise::new(
                    roster
                        .and_then(|r| r.get(species.kit()))
                        .map_or(0.0, |k| k.poise_max)
                        .max(def.poise()),
                ),
                crate::audio::VoiceOf(shown),
                Roam::new(area, seed),
            ),
        ));
        if let Some(weapon) = def.weapon {
            creature.insert(if def.left_handed {
                HeldItem::left_hand(weapon)
            } else {
                HeldItem::right_hand(weapon)
            });
        }
        if let Some(weapon) = def.offhand_weapon {
            creature.insert(OffHandItem::new(weapon, false));
        }
        if def.hit_floor > 0.0 {
            creature.insert(crate::hits::HitFloor(def.hit_floor));
        }
        if crate::species::rank_of(def) == crate::species::Rank::Boss {
            creature.insert(crate::species::Boss);
        }
        if crate::species::shows_boss_bar(def) {
            creature.insert(crate::species::BossBar);
        }
        creature.insert((
            crate::boss::BossPhase::default(),
            crate::boss::Tempo(def.tempo),
        ));
        Ok(Placed {
            entity: creature.id(),
            feet,
            display: shown,
            scale,
        })
    }
}

/// The display a species wears: its own, or `WOWSOULS_<NAME>_DISPLAY=<id>` for looking at another
/// (the client has no creature templates, so the right display of a creature is picked by eye).
fn display_for(def: &crate::species::SpeciesDef, default: u32) -> u32 {
    std::env::var(format!(
        "WOWSOULS_{}_DISPLAY",
        def.name.to_ascii_uppercase()
    ))
    .ok()
    .and_then(|v| v.trim().parse().ok())
    .unwrap_or(default)
}

/// `WOWSOULS_TRACE`: a dead creature's place, whether it renders, its death clip and how its body
/// lies (the height of its bone capsules over its feet), twice a second.
fn trace_dead(
    time: Res<Time<bevy::time::Real>>,
    npcs: Query<(
        Entity,
        &GlobalTransform,
        &Reaction,
        &ViewVisibility,
        Option<&AnimationPlayer>,
        Option<&ModelAnimations>,
    )>,
    parts: Query<(&ChildOf, &ViewVisibility), With<benilla_world::model_render::ModelPart>>,
    bodies: Query<(
        Option<&crate::hitbody::BodyHits>,
        Option<&benilla_world::rig_palette::RigSkin>,
    )>,
    palettes: Res<benilla_world::rig_palette::RigPalettes>,
    mut next_at: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now < *next_at {
        return;
    }
    *next_at = now + 0.5;
    for (e, (entity, at, r, vis, player, anims)) in npcs.iter().enumerate() {
        if *r != Reaction::Dead {
            continue;
        }
        let seek = player
            .zip(anims.and_then(|a| a.find(DEATH)))
            .and_then(|(p, c)| {
                p.animation(c.node)
                    .map(|a| (a.seek_time(), a.is_finished(), a.weight()))
            });
        let shown_parts = parts.iter().filter(|(_, v)| v.get()).count();
        // How the body lies: the capsule centres' heights over the feet.
        let lie = bodies.get(entity).ok().map(|(hits, skin)| {
            let centres: Vec<Vec3> = crate::hitbody::posed_body(hits, skin, &palettes)
                .iter()
                .map(|(a, b, _)| (*a + *b) * 0.5 - at.translation())
                .collect();
            let (lo, hi) = centres.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| {
                (lo.min(c.y), hi.max(c.y))
            });
            (centres.len(), lo, hi)
        });
        info!(
            "dead[{e}] at {:.2} root visible={} parts visible (any npc)={shown_parts} death={seek:?} bone centres (n, lowest, highest) over the feet={lie:.2?}",
            at.translation(),
            vis.get()
        );
    }
}

/// `WOWSOULS_TRACE`: each creature's feet and its body collider's physics position (WoW coords),
/// four times a second.
fn trace_body(
    time: Res<Time<bevy::time::Real>>,
    npcs: Query<(
        &Species,
        &Transform,
        Option<&BodyCollider>,
        &Brain,
        Option<&crate::world::SpawnId>,
    )>,
    bodies: Query<&Position>,
    mut next_at: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now < *next_at {
        return;
    }
    *next_at = now + 0.25;
    for (species, xf, body, brain, id) in &npcs {
        let wow = |v: Vec3| {
            let w = benilla_assets::coords::bevy_to_wow(v);
            format!("({:9.2},{:8.2},{:6.2})", w[0], w[1], w[2])
        };
        let collider = body.and_then(|b| bodies.get(b.0).ok()).map(|p| wow(p.0));
        // A spawn of the valley says which record it is (`ring:` lines name it the same way).
        let id = id.map_or(String::new(), |i| format!(" #{}", i.0));
        info!(
            "{}{id} t={now:6.2} feet={} body={collider:?} mind={:?}",
            species.0.name,
            wow(xf.translation),
            brain.mind
        );
    }
}

/// `WOWSOULS_TRACE`: through a swing, every frame, its right hand (attachment 1) in the
/// creature's frame, `(right, up, forward)`, to place the strike window on the clip.
fn trace_swing(
    m2s: Res<Assets<benilla_assets::M2Model>>,
    npcs: Query<(
        &Brain,
        &GlobalTransform,
        &ActorModel,
        &benilla_world::rig_anim::RigPose,
    )>,
) {
    for (brain, at, model, pose) in &npcs {
        let Mind::Attack { t, .. } = brain.mind else {
            continue;
        };
        let Some(hand) = m2s
            .get(&model.m2)
            .and_then(|m| m.attachments.iter().find(|a| a.id == 1).copied())
        else {
            continue;
        };
        let Some(p) = pose.posed_point(at, hand.bone, hand.offset) else {
            continue;
        };
        let local = at.rotation().inverse() * (p - at.translation());
        info!(
            "swing t={t:.2} hand right={:+.2} up={:+.2} fwd={:+.2}",
            local.x, local.y, -local.z
        );
    }
}

/// Plays a clip once on a creature at `rate`; its length at that rate, `None` when the model
/// lacks it.
pub(crate) fn play_once(
    anims: &ModelAnimations,
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    id: u16,
    rate: f32,
) -> Option<f32> {
    let clip = anims.find(id)?;
    transitions
        .play(player, clip.node, std::time::Duration::from_secs_f32(0.05))
        .set_repeat(RepeatAnimation::Never)
        .set_speed(rate);
    Some(clip.duration / rate)
}

/// Turns hits into flinches and deaths. A flinch breaks a swing. A dead creature stops being a
/// target: its lock, hurtbox and body collider go, and the lock-on lets go on its own when the
/// target loses `Lockable`. With the Dark Souls kit the clip comes from the blow's level and
/// the way it pushes the creature, and a creature with poise flinches only when its poise breaks.
#[allow(clippy::type_complexity)]
fn react(
    mut commands: Commands,
    roster: Option<Res<Roster>>,
    mut hits: MessageReader<crate::hits::Hit>,
    mut npcs: Query<(
        &Species,
        &Transform,
        &mut Reaction,
        &mut Hostile,
        &mut Brain,
        &mut Poise,
        &mut MonsterPlay,
        Option<&BodyCollider>,
        Option<&ModelAnimations>,
        Option<&mut AnimationPlayer>,
        Option<&mut AnimationTransitions>,
        Option<&crate::boss::Frenzy>,
    )>,
    mut epoch: ResMut<ColliderEpoch>,
    mut slain: MessageWriter<Slain>,
) {
    for hit in hits.read() {
        let Ok((
            species,
            xf,
            mut reaction,
            mut hostile,
            mut brain,
            mut poise,
            mut play,
            body,
            anims,
            player,
            transitions,
            frenzy,
        )) = npcs.get_mut(hit.target)
        else {
            continue;
        };
        if *reaction == Reaction::Dead {
            continue;
        }
        let kit = roster.as_deref().and_then(|r| r.get(species.kit()));
        let name = species.0.name;
        // A damaging hit reveals the attacker even from behind or outside hearing range.
        // Acquisition must not depend on breaking poise, nor interrupt an armored swing.
        if !hit.lethal {
            if !brain.mind.engaged() {
                brain.mind = Mind::Chase;
                brain.wish = None;
                brain.tactic.reset();
                info!("npc: {name} notices the player after a hit");
            }
            hostile.alert = true;
        }
        let (Some(anims), Some(mut p), Some(mut tr)) = (anims, player, transitions) else {
            continue;
        };
        // Which way the blow pushes the creature, in its own frame: (forward, left).
        let away = Vec3::new(
            xf.translation.x - hit.from.x,
            0.0,
            xf.translation.z - hit.from.z,
        )
        .normalize_or_zero();
        let local = xf.rotation.inverse() * away;
        let push = Vec2::new(-local.z, -local.x);
        if hit.lethal {
            // The WoW death underneath; the kit's over it where it has one.
            play_once(anims, &mut p, &mut tr, DEATH, 1.0);
            play.clip = kit
                .and_then(|k| choose_death(k, push))
                .map(|i| (MonsterClip::Death(i), 0.0));
            *reaction = Reaction::Dead;
            hostile.alert = false;
            brain.mind = Mind::Idle;
            brain.tactic.reset();
            commands
                .entity(hit.target)
                .remove::<(Lockable, Hurtbox, BodyCollider)>();
            if let Some(b) = body {
                commands.entity(b.0).despawn();
                epoch.bump();
            }
            slain.write(Slain {
                who: hit.target,
                souls: kit.map_or(0, |k| k.souls),
            });
            info!("npc: {name} dies (push {push:.2})");
        } else if (hit.level > 0 || hit.poise > 0.0)
            && !frenzy.is_some_and(|f| f.0.armored())
            && (kit.is_none() || poise.hit_chained(hit.poise))
        {
            let chosen = kit.and_then(|k| choose_react(k, hit.level, push).map(|i| (k, i)));
            let len = match chosen {
                Some((k, i)) => {
                    play_once(anims, &mut p, &mut tr, COMBAT_WOUND, 1.0);
                    k.reacts[usize::from(i)].wow.duration()
                }
                None => play_once(anims, &mut p, &mut tr, COMBAT_WOUND, 1.0).unwrap_or(0.4),
            };
            play.clip = chosen.map(|(_, i)| (MonsterClip::React(i), 0.0));
            *reaction = Reaction::Flinch(len);
            crate::shove::start(&mut commands, hit.target, name, away, hit.knock);
            if matches!(brain.mind, Mind::Attack { .. }) {
                info!("npc: {name}'s swing broken by the hit");
            }
            brain.mind = Mind::Chase;
            brain.wish = None;
            brain.foot.hit();
            brain.tactic.reset();
            info!(
                "npc: {name} flinches (level {}, push {push:.2}, {len:.2} s)",
                hit.level
            );
        } else {
            info!(
                "npc: {name} holds its ground (poise {:.0}/{:.0})",
                poise.left, poise.max
            );
        }
    }
}

/// Poise comes back to the creature that is left alone.
fn poise_tick(time: Res<Time>, mut creatures: Query<&mut Poise>) {
    for mut p in &mut creatures {
        p.tick(time.delta_secs());
    }
}

/// Runs each creature's mind: senses the player, picks a move from the pattern, steps
/// [`think_step`], plays its swing's clip, turns it and asks its motor to move. Without the Dark
/// Souls kit the old swing and its distance-and-cone strike run here.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn think(
    time: Res<Time>,
    world: WorldCollision,
    roster: Option<Res<Roster>>,
    mut rng: ResMut<crate::monster::FoeRng>,
    gear: Res<crate::gear_effect::GearFx>,
    avatar: Query<
        (
            &Transform,
            &Action,
            &Health,
            &Hurtbox,
            Option<&Motor>,
            Option<&crate::abilities::moves::Casting>,
            Option<&crate::abilities::moves::UpperCast>,
        ),
        (With<Avatar>, Without<Hostile>),
    >,
    mut npcs: Query<(
        Entity,
        &Species,
        &mut Transform,
        &mut Hostile,
        &mut Brain,
        &mut Reaction,
        &Home,
        &mut MoveIntent,
        &mut MonsterPlay,
        Option<&ModelAnimations>,
        Option<&mut AnimationPlayer>,
        Option<&mut AnimationTransitions>,
        &mut Roam,
        (&crate::boss::BossPhase, &crate::boss::Tempo),
    )>,
    mut hurts: MessageWriter<PlayerHurt>,
    mut telegraphs: MessageWriter<Telegraph>,
    mut tells: MessageWriter<crate::foebolt::CastTell>,
) {
    let Ok((player, action, health, hurt, motor, casting, upper_cast)) = avatar.single() else {
        return;
    };
    let dt = time.delta_secs();
    let player_alive = !action.is_dead() && health.cur > 0.0;
    let noise = perception::noise(
        motor,
        &action.state,
        casting
            .map(|c| c.clip)
            .or_else(|| upper_cast.map(|c| c.clip)),
    );
    let sim = crate::ring::sim();
    // The player is open to a counter while it swings, casts, staggers or reels.
    let player_open = crate::footwork::is_open(&action.state);
    // How many creatures near the player have made up their minds (closing in or swinging):
    // the others wait their turn at their rings ([`crate::footwork::MAX_COMMITTED`]).
    let near_player = |t: &Transform| {
        Vec2::new(
            player.translation.x - t.translation.x,
            player.translation.z - t.translation.z,
        )
        .length()
            <= crate::footwork::CROWD_RANGE
    };
    let mut busy = npcs
        .iter()
        .filter(|(_, _, t, _, b, ..)| {
            (matches!(b.mind, Mind::Attack { .. }) || (b.mind == Mind::Chase && b.foot.committed))
                && near_player(t)
        })
        .count();
    let actors: Vec<_> = npcs
        .iter()
        .filter(|(_, _, _, _, _, reaction, ..)| **reaction != Reaction::Dead)
        .map(|(e, species, xf, _, brain, ..)| crate::behavior::Actor {
            id: e.to_bits(),
            role: species.0.behavior,
            at: xf.translation,
            attacking: matches!(brain.mind, Mind::Attack { .. }),
            ready: brain.mind == Mind::Chase
                && brain.tactic.ready()
                && (player.translation - xf.translation).xz().length()
                    <= roster
                        .as_deref()
                        .and_then(|r| r.get(species.kit()))
                        .map_or(ATTACK_DISTANCE, |_| {
                            crate::pattern::reach(species.0.table).1
                        })
                        + 0.1,
            wait: brain.tactic.wait,
        })
        .collect();
    let admitted = crate::behavior::admissions(&actors, player.translation);
    for (
        e,
        species,
        mut xf,
        mut hostile,
        mut brain,
        mut reaction,
        home,
        mut intent,
        mut play,
        anims,
        p,
        tr,
        mut roam,
        (phase, tempo),
    ) in &mut npcs
    {
        *intent = MoveIntent::default();
        // The side-by-side shows a clip, not a fight.
        if crate::rig_view::view_species() == Some(species.0.name) {
            continue;
        }
        // Far from the player an idle creature rests (its intent stays cleared above).
        let gap = Vec2::new(
            player.translation.x - xf.translation.x,
            player.translation.z - xf.translation.z,
        )
        .length();
        if !crate::ring::simulated(gap, brain.mind == Mind::Idle, sim) {
            continue;
        }
        let name = species.0.name;
        let kit = roster.as_deref().and_then(|r| r.get(species.kit()));
        // A boss's second phase: another table, quicker feet and swings, shorter pauses.
        let mods = phase.0.mods(species.0);
        let chase = species.0.chase_speed * mods.chase;
        let dts = dt * tempo.0;
        let table = phase.0.table(species.0);
        // A boss's rhythm: what follows what, how a combo links, where a swing is held back.
        let styles = crate::pattern::styles(species.kit(), phase.0 == crate::boss::Phase::Two);
        // The pattern's rows are named, the kit's moves indexed.
        let mv_of = |row: usize| {
            kit.and_then(|k| k.attack_index(table[row].name))
                .map(|i| i as u8)
        };
        let row_of = |mv: u8| (0..table.len()).find(|&r| mv_of(r) == Some(mv));
        // Not before its model is on: a swing needs its clip.
        let Some(anims) = anims else {
            continue;
        };
        match *reaction {
            Reaction::Dead => continue,
            Reaction::Flinch(left) => {
                let left = left - dt;
                *reaction = if left > 0.0 {
                    Reaction::Flinch(left)
                } else {
                    play.clip = None;
                    Reaction::None
                };
                if left > 0.0 {
                    continue;
                }
            }
            Reaction::None => {}
        }
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
        let ground = |v: Vec3| [v.x, v.z];
        let to_player = flat(player.translation - xf.translation);
        // Home is the whole area: anywhere inside a wanderer's radius or along a walker's path; out
        // of it, the walk home goes to the nearest point of it.
        let (home_gap, back_to) =
            crate::roam::home_distance(&roam.area, ground(home.feet), ground(xf.translation));
        let to_home = Vec3::new(
            back_to[0] - xf.translation.x,
            0.0,
            back_to[1] - xf.translation.z,
        );
        let nose = flat(*xf.forward()).normalize_or_zero();
        let rise = (player.translation.y - xf.translation.y).abs();
        let dist = to_player.length();
        let facing = to_player.normalize_or_zero().dot(nose);
        // Reject range/cone/height first. Both channels need a clear ray, so hearing cannot pull
        // a creature through a mine wall or the ground. Combat retains its existing memory.
        let candidate = perception::detect(dist, facing, rise, true, noise).is_some();
        let seen = brain.mind.engaged()
            || (player_alive
                && candidate
                && crate::lockon::in_sight(
                    &world,
                    xf.translation + Vec3::Y * home.lockable.aim_height,
                    player.translation + Vec3::Y * PLAYER_CHEST,
                ));
        let mut sense = Sense {
            dist,
            facing,
            player_alive,
            home_dist: home_gap,
            attack: None,
            cooldown: COOLDOWN,
            seen,
            rise,
            noise,
        };
        match kit {
            Some(k) => {
                sense.cooldown = CLIP_COOLDOWN * mods.cooldown;
                // After a cast it gathers itself: a bolt every three and a half seconds or so, as WoW's 3 s casts.
                if let Mind::Attack { mv, .. } = brain.mind {
                    if k.attacks
                        .get(usize::from(mv))
                        .is_some_and(|a| a.bullet.is_some())
                    {
                        sense.cooldown = CAST_COOLDOWN * mods.cooldown;
                    }
                }
                if brain.mind == Mind::Chase {
                    // It wishes for a move while closing in and starts it once the player is
                    // inside that move's band; already inside another band, it takes that.
                    let last = brain.last.and_then(row_of);
                    let wish = match brain.wish.and_then(row_of) {
                        Some(w) => Some(w),
                        None => {
                            // A boss opens a fight with its charge, once.
                            let opening = styles
                                .iter()
                                .find(|s| s.opening)
                                .and_then(|s| table.iter().position(|r| r.name == s.name))
                                .filter(|_| brain.last.is_none());
                            let w =
                                opening.or_else(|| crate::pattern::wish(table, last, &mut rng.0));
                            brain.wish = w.and_then(mv_of);
                            w
                        }
                    };
                    sense.attack =
                        crate::pattern::choose(table, sense.dist, wish, last, &mut rng.0)
                            .and_then(mv_of);
                    if let Some(mv) = brain.start.take() {
                        sense.attack = Some(mv);
                    }
                    if sense.attack.is_some() {
                        if let Some(forced) = crate::monster::forced_move(k) {
                            sense.attack = Some(forced);
                        }
                    }
                }
            }
            None => sense.attack = (sense.dist <= ATTACK_DISTANCE).then_some(0),
        }
        // Its footwork: the temperament, and the ring of the move it has in mind (the reach of
        // every move it can open with before it has one).
        let temper = crate::footwork::tuned(&species.0.temper);
        let band = brain
            .wish
            .and_then(row_of)
            .map_or_else(|| crate::pattern::reach(table), |r| table[r].band);
        let family_ai = crate::footwork::enabled()
            && species.0.behavior != crate::behavior::Role::BossCommitted;
        let mut tactical = None;
        let mut tactical_stop = species.0.stop_distance;
        if family_ai && matches!(brain.mind, Mind::Chase | Mind::Cooldown { .. }) {
            let (allies, center, leader) =
                crate::behavior::group(species.0.behavior, e.to_bits(), xf.translation, &actors);
            let mut context = crate::behavior::Context {
                me: xf.translation,
                target: player.translation,
                forward: *player.forward(),
                facing: sense.facing,
                reach: if kit.is_some() {
                    crate::pattern::reach(table).1
                } else {
                    ATTACK_DISTANCE
                },
                near: if kit.is_none() {
                    ATTACK_DISTANCE
                } else {
                    table
                        .iter()
                        .filter(|r| r.weight > 0)
                        .map(|r| r.band.1)
                        .fold(f32::MAX, f32::min)
                },
                speed: chase,
                admitted: admitted.contains(&e.to_bits()),
                open: player_open && player_alive,
                guarding: matches!(action.state, crate::combat::ActionState::Defend(_)),
                sight: true,
                can_attack: if let Some(k) = kit {
                    table.iter().any(|r| {
                        r.weight > 0
                            && sense.dist >= r.band.0
                            && sense.dist <= r.band.1
                            && k.attack_index(r.name).is_some()
                    })
                } else {
                    sense.dist <= ATTACK_DISTANCE
                },
                allies,
                center,
                leader,
            };
            if species.0.behavior == crate::behavior::Role::CasterKite {
                context.sight = crate::behavior::sight(&world, context, home.body.0, hurt.radius);
            }
            // A caster's preferred casting distance must not prevent it crossing a gap
            // between its bolt and melee bands, or seeking a clear line of sight.
            tactical_stop = tactical_stop.min((context.near - 0.1).max(0.4));
            let was = brain.tactic.phase;
            brain.tactic.seed(e.to_bits());
            let decision =
                crate::behavior::step(species.0.behavior, name, &mut brain.tactic, context, dt);
            // A named Net/Web launch already selected by named.rs keeps its existing control path.
            if brain.throw.is_none() {
                if !decision.attack {
                    sense.attack = None;
                } else if let Some(row) =
                    crate::behavior::preferred(species.0.behavior, &brain.tactic, context, table)
                {
                    if kit.and_then(crate::monster::forced_move).is_none() {
                        sense.attack = mv_of(row).or(sense.attack);
                    }
                }
            }
            if was != brain.tactic.phase && crate::actor::anim::trace_on() {
                info!(
                    "ai: {name} {} {was:?} -> {:?} dist={:.2} slot={} allies={allies}",
                    species.0.behavior.id(),
                    brain.tactic.phase,
                    sense.dist,
                    context.admitted
                );
            }
            tactical = Some(decision);
        } else if crate::footwork::enabled() && brain.mind == Mind::Chase {
            let stepping_away = brain.foot.beat == crate::footwork::Beat::Back;
            // Not made up its mind and the player out of reach of the move it has in mind: it
            // comes in to its ring and sizes the player up first; a move whose band it happens to
            // pass through on the way (or that reaches out to the ring itself) does not start.
            // Inside that reach it swings at once, as a player who walks into it learns.
            let approaching =
                !brain.foot.committed && sense.dist > band.1 + crate::footwork::REACH_SLACK;
            if stepping_away || approaching {
                sense.attack = None;
            }
        }
        let now_mv = match brain.mind {
            Mind::Attack { mv, .. } => mv,
            _ => 0,
        };
        let now_spec = attack_spec(now_mv, kit, anims);
        let mut attack_len = swing_length(&now_spec, species.0.recovery);
        if kit.is_some() {
            if let Some(cut) = crate::pattern::style_of(styles, &now_spec.name).and_then(|s| s.cut)
            {
                attack_len = attack_len.min(cut);
            }
            // A swing that links into the next gives its pause short.
            if brain.next.is_some() {
                let last_hit = now_spec
                    .hits
                    .iter()
                    .map(|h| h.1)
                    .fold(f32::NEG_INFINITY, f32::max);
                if last_hit.is_finite() {
                    attack_len = attack_len.min(last_hit + crate::pattern::CHAIN_RECOVERY);
                }
            }
        }
        // A swing held back at its tell does not run its clock.
        let mut dts_step = dts;
        // A cast winds up at its caster's pace until the bolt leaves ([`crate::foebolt::windup_rate`]).
        if let Mind::Attack { t, mv } = brain.mind {
            if kit
                .and_then(|k| k.attacks.get(usize::from(mv)))
                .is_some_and(|a| a.bullet.is_some() && t < a.spec.hit.0)
            {
                dts_step *= kit
                    .and_then(|k| k.attacks.get(usize::from(mv)))
                    .map_or(1.0, |a| {
                        crate::foebolt::windup_rate(species.0.name, &a.spec.name)
                    });
            }
        }
        if let (Mind::Attack { t, .. }, Some((at, left))) = (brain.mind, brain.hold) {
            if t >= at && left > 0.0 {
                dts_step = 0.0;
                brain.hold = Some((at, left - dt));
            }
        }
        // A spider hanging on ([`crate::latch`]): the swing's clock stands still while it chews. The time
        // up, a roll or the player's death lets go, and the hop back follows; a flinch (the swing
        // broken) just drops it.
        if brain.latch.is_some() && !matches!(brain.mind, Mind::Attack { .. }) {
            brain.latch = None;
        }
        let mut let_go = false;
        if let Some(latch) = brain.latch.as_mut() {
            dts_step = 0.0;
            for _ in 0..latch.step(dt) {
                let damage =
                    (now_spec.damage * species.0.blow_scale() * crate::latch::CHEW_SHARE).round();
                hurts.write(PlayerHurt {
                    from: e,
                    damage: gear.0.taken(damage, species.0.level, false),
                    level: 0,
                    stop: 0.0,
                    knock: 0.0,
                    poise: 0.0,
                    blocked: false,
                });
                info!("npc: {name} chews on the player for {damage:.0}");
            }
            let_go = latch.done() || !player_alive || action.invulnerable();
        }
        if let_go {
            info!("npc: {name} lets go");
            brain.latch = None;
            brain.forced = kit
                .and_then(|k| k.attack_index("hop_back"))
                .and_then(|i| u8::try_from(i).ok());
            if let Mind::Attack { t, .. } = &mut brain.mind {
                *t = attack_len;
            }
        }
        // A boar's charge runs through the player ([`crate::boar_rush`]): the clip's burst is spread
        // over the run, so the swing's clock follows the yards covered, not the seconds.
        let mut rush_step = None;
        if let Mind::Attack { t, mv } = brain.mind {
            // The boar kit's (Princess keeps her own short charge and long charge).
            let charging = matches!(species.kit(), "boar" | "princess")
                && kit.and_then(|k| k.attack_index(crate::boar_rush::MOVE))
                    == Some(usize::from(mv));
            if charging && brain.rush.is_none() && crate::boar_rush::begins(t, t + dts_step) {
                brain.rush = crate::boar_rush::Rush::start(*xf.forward(), sense.dist);
                if let Some(r) = brain.rush {
                    info!("npc: {name} rushes {:.1} yd through the player", r.length);
                }
            }
            if let Some(mut rush) = brain.rush.filter(|_| charging) {
                let (step, at) = rush.step(dts);
                rush_step = Some((rush.dir, step));
                brain.mind = Mind::Attack { t: at, mv };
                dts_step = 0.0;
                brain.rush = (!rush.done()).then_some(rush);
            }
        } else {
            brain.rush = None;
        }
        let before = brain.mind;
        let mut cue = think_step(&mut brain.mind, sense, attack_len, dts_step);
        if cue == Cue::AttackEnded {
            brain.last = Some(now_mv);
            brain.wish = None;
            brain.throw = None;
            play.clip = None;
            brain.foot.swung();
            // A chained move: the player still inside the follow-up's band. A boss planned it as
            // the swing began; the others roll their row's `then` now. A spider that let go hops back.
            let planned = brain.next.take();
            let forced = brain
                .forced
                .take()
                .filter(|_| kit.is_some() && player_alive);
            brain.hold = None;
            let rolled = if styles.is_empty() {
                row_of(now_mv)
                    .filter(|_| kit.is_some() && player_alive)
                    .and_then(|r| crate::pattern::follow_up(table, r, sense.dist, &mut rng.0))
                    .and_then(mv_of)
            } else {
                planned
                    .filter(|_| kit.is_some() && player_alive)
                    .filter(|&mv| {
                        row_of(mv).is_some_and(|r| {
                            let band = table[r].band;
                            sense.dist >= band.0 - crate::footwork::REACH_SLACK
                                && sense.dist <= band.1 + crate::footwork::REACH_SLACK
                        })
                    })
            };
            let next = forced.or(rolled);
            if let Some(mv) = next {
                // A link skips part of the wind-up: a combo's blows come closer.
                let link = kit
                    .and_then(|k| k.attacks.get(usize::from(mv)))
                    .and_then(|a| crate::pattern::style_of(styles, &a.spec.name))
                    .map_or(0.0, |s| s.link);
                brain.mind = Mind::Attack { t: link, mv };
                cue = Cue::AttackStarted;
            } else if family_ai {
                brain.tactic.finished(species.0.behavior, name);
            }
        }
        if std::mem::discriminant(&before) != std::mem::discriminant(&brain.mind)
            && crate::actor::anim::trace_on()
        {
            info!(
                "npc: {before:?} -> {:?} dist={:.2} facing={:.2}",
                brain.mind, sense.dist, sense.facing
            );
        }
        if brain.mind.engaged() != hostile.alert {
            hostile.alert = brain.mind.engaged();
            if hostile.alert && !before.engaged() {
                info!(
                    "npc perception: {name} acquired via {:?} dist={:.2} facing={:.2} noise={:.1}",
                    sense.detection(),
                    sense.dist,
                    sense.facing,
                    sense.noise
                );
            }
            if !hostile.alert {
                brain.last = None;
            }
            info!(
                "npc: {name} {}",
                if hostile.alert {
                    "notices the player"
                } else {
                    "loses interest"
                }
            );
        }
        let mv = match brain.mind {
            Mind::Attack { mv, .. } => mv,
            _ => 0,
        };
        let spec = attack_spec(mv, kit, anims);
        if cue == Cue::AttackStarted {
            brain.tactic.started();
            if family_ai {
                busy += 1;
            }
            brain.struck = 0;
            brain.dodged = false;
            if let (Some(mut p), Some(mut tr)) = (p, tr) {
                match kit {
                    // The rig idles underneath; the kit's clip owns its driven bones.
                    Some(_) => {
                        play_once(anims, &mut p, &mut tr, 0, 1.0);
                    }
                    None => {
                        play_once(
                            anims,
                            &mut p,
                            &mut tr,
                            native_attack(anims),
                            KOBOLD_ATTACK_RATE,
                        );
                    }
                }
            }
            let start = match brain.mind {
                Mind::Attack { t, .. } => t,
                _ => 0.0,
            };
            play.clip = kit.map(|_| (MonsterClip::Attack(mv), start));
            brain.next = None;
            brain.hold = None;
            if let (Some(k), Some(style)) = (kit, crate::pattern::style_of(styles, &spec.name)) {
                brain.next = crate::pattern::plan_next(style, &mut rng.0)
                    .and_then(|n| k.attack_index(n))
                    .and_then(|i| u8::try_from(i).ok());
                brain.hold = crate::pattern::plan_hold(style, &mut rng.0);
            }
            telegraphs.write(Telegraph { from: e });
            // A cast shows its wind-up ([`crate::foebolt::CastTell`]).
            if kit
                .and_then(|k| k.attacks.get(usize::from(mv)))
                .is_some_and(|a| a.bullet.is_some())
            {
                tells.write(crate::foebolt::CastTell {
                    from: e,
                    species: species.0.name,
                    secs: spec.hit.0 / crate::foebolt::windup_rate(species.0.name, &spec.name),
                });
            }
            info!(
                "npc: {name} winds up with {} ({:.2} s, strike {:.2}-{:.2} s, damage {:.0}){}{}",
                spec.name,
                spec.length,
                spec.hit.0,
                spec.hit.1,
                (spec.damage * species.0.blow_scale()).round(),
                brain
                    .next
                    .and_then(|n| kit.and_then(|k| k.attacks.get(usize::from(n))))
                    .map_or(String::new(), |a| format!(", then {}", a.spec.name)),
                brain.hold.map_or(String::new(), |(at, s)| format!(
                    ", held {s:.2} s at {at:.2}"
                )),
            );
        }
        // The feet while it is engaged and not swinging: it sizes the player up, circles, steps
        // back ([`crate::footwork`]); the straight chase when that is off.
        if !brain.mind.engaged() {
            brain.foot.reset();
            brain.tactic.reset();
        } else if brain.foot.is_unseeded() {
            brain.foot = crate::footwork::Foot::seeded(e.to_bits());
        }
        let strafes = anims.find(crate::actor::anim::ids::SHUFFLE_LEFT).is_some()
            && anims.find(crate::actor::anim::ids::SHUFFLE_RIGHT).is_some();
        let mut footing = None;
        if brain.mind == Mind::Chase && crate::footwork::enabled() && !family_ai {
            let crowded = !brain.foot.committed && busy >= crate::footwork::MAX_COMMITTED;
            let was = (brain.foot.committed, brain.foot.beat);
            let plan = crate::footwork::step(
                &mut brain.foot,
                &temper,
                crate::footwork::Look {
                    dist: sense.dist,
                    band,
                    speed: chase,
                    player_open: player_open && player_alive,
                    crowded,
                },
                dt,
            );
            if !was.0 && brain.foot.committed {
                busy += 1;
                if crate::actor::anim::trace_on() {
                    info!(
                        "npc: {name} commits at dist={:.2} (ring {:.2}, player open {}, was {:?})",
                        sense.dist,
                        crate::footwork::ring(&temper, band),
                        player_open && player_alive,
                        was.1
                    );
                }
            }
            if was.1 != brain.foot.beat && crate::actor::anim::trace_on() {
                info!(
                    "npc: {name} footwork {:?} -> {:?} dist={:.2} ring={:.2}{}",
                    was.1,
                    brain.foot.beat,
                    sense.dist,
                    crate::footwork::ring(&temper, band),
                    if brain.foot.committed {
                        " committed"
                    } else {
                        ""
                    }
                );
            }
            footing = Some(plan);
        }
        // Where it goes and where it looks.
        let (go, speed, look) = match brain.mind {
            Mind::Chase | Mind::Cooldown { .. } if family_ai => {
                let decision = tactical.unwrap_or(crate::behavior::Decision {
                    goal: xf.translation,
                    pace: 0.0,
                    attack: false,
                });
                let mut delta = flat(decision.goal - xf.translation);
                // A charging/pressing creature still respects the player's physical edge.
                if delta.dot(to_player) > 0.0 && sense.dist <= tactical_stop {
                    delta = Vec3::ZERO;
                }
                let dir =
                    crate::behavior::safe_direction(&world, xf.translation, delta, home.body.0);
                let speed = if delta.length() > 0.15 && dir != Vec3::ZERO {
                    chase * decision.pace
                } else {
                    0.0
                };
                let facing = if strafes || speed == 0.0 {
                    to_player
                } else {
                    dir
                };
                (dir, speed, facing)
            }
            Mind::Chase => match footing {
                Some(plan) => {
                    // It does not walk into the player past its stop distance.
                    let plan = if plan.ahead > 0.0 && sense.dist <= species.0.stop_distance {
                        crate::footwork::Plan { ahead: 0.0, ..plan }
                    } else {
                        plan
                    };
                    crate::footwork::world_motion(plan, to_player, chase, strafes)
                }
                None if sense.dist > species.0.stop_distance => (to_player, chase, to_player),
                None => (Vec3::ZERO, 0.0, to_player),
            },
            Mind::Cooldown { .. } => (Vec3::ZERO, 0.0, to_player),
            // Tracking through the wind-up, committed from the strike on.
            Mind::Attack { t, .. } if t < spec.hit.0 => (Vec3::ZERO, 0.0, to_player),
            Mind::Return => (to_home, RETURN_SPEED, to_home),
            // Home, it roams its area at its walk; a creature at a post turns back to the way it
            // was placed, a paused wanderer keeps its facing.
            Mind::Idle => {
                let Roam { area, walk, rng } = &mut *roam;
                let here = ground(xf.translation);
                match crate::roam::step(walk, area, ground(home.feet), here, dt, rng) {
                    Some(t) => {
                        let d = Vec3::new(t[0] - here[0], 0.0, t[1] - here[1]);
                        (d, species.0.walk_speed(), d)
                    }
                    None if crate::roam::is_post(area) => (
                        Vec3::ZERO,
                        0.0,
                        Quat::from_rotation_y(home.facing) * Vec3::NEG_Z,
                    ),
                    None => (Vec3::ZERO, 0.0, Vec3::ZERO),
                }
            }
            Mind::Attack { .. } => (Vec3::ZERO, 0.0, Vec3::ZERO),
        };
        // Roused from its roaming, it starts its walk afresh once it is home again.
        if brain.mind != Mind::Idle {
            roam.walk = Walk::default();
        }
        *intent = MoveIntent {
            dir: go.normalize_or_zero(),
            speed,
            jump: false,
        };
        if look.length_squared() > 1.0e-4 {
            let current = xf.rotation.to_euler(EulerRot::YXZ).0;
            let next = turn_toward(current, yaw_of(look), TURN_RATE * dt);
            xf.rotation = Quat::from_rotation_y(next);
        }
        // The clip is where the mind is in the swing (a held swing stands at its tell).
        if let (Some(_), Mind::Attack { t, mv }) = (kit, brain.mind) {
            if let Some((MonsterClip::Attack(playing), at)) = play.clip.as_mut() {
                if *playing == mv {
                    *at = t;
                }
            }
        }
        // A kit move carries the body by its clip's root motion, recoil included.
        if let (Some(k), Mind::Attack { t, mv }) = (kit, brain.mind) {
            if let Some((wow, _)) = k.clips(MonsterClip::Attack(mv)) {
                let mut step = (crate::motion_clip::progress(wow, t)
                    - crate::motion_clip::progress(wow, (t - dts).max(0.0)))
                    * xf.scale.y;
                // Not into a player ahead of it: it stops at its edge (recoil still backs it).
                let edge = sense.dist - hurt.radius - home.body.0;
                if step > 0.0 && sense.facing > 0.5 && edge <= LUNGE_STOP_GAP {
                    step = 0.0;
                }
                *intent = crate::combat::root_step(*xf.forward(), step, dt);
            }
        }
        // A charge's run holds its line through the player and past ([`crate::boar_rush`]).
        if let Some((dir, step)) = rush_step {
            *intent = crate::combat::root_step(dir, step, dt);
            xf.rotation = Quat::from_rotation_y(yaw_of(dir));
        }
        // Hanging on: it chews on the spot, the bite's strike over and over, carried along with the
        // player by the offset it landed at.
        if let (Some(latch), Some(k)) = (brain.latch, kit) {
            if let Some(bite) = k.attack_index("bite").and_then(|i| u8::try_from(i).ok()) {
                play.clip = Some((MonsterClip::Attack(bite), latch.chew_pose()));
            }
            *intent = MoveIntent::default();
            // It closes the last bit as it lands (a quick ease, not a jump). Never into the world: the reach
            // is cut where a wall or a hill stands between the player's chest and the hold, and its feet
            // stay on the ground there (2026-10-17, the user: the spiders "går igenom världen").
            let mut target = latch.hold(player.translation);
            let side = flat(target - player.translation);
            let reach = side.length();
            if reach > 1.0e-3 {
                let chest = player.translation + Vec3::Y * 1.0;
                if let Some(hit) = world.cast_camera(chest, side, false) {
                    let keep = (hit - 0.35).max(0.0);
                    if keep < reach {
                        target -= side * (1.0 - keep / reach);
                    }
                }
            }
            if let Some(ground) = snap_to_ground(&world, target) {
                target.y = target.y.max(ground);
            }
            xf.translation = xf.translation.lerp(target, (dt * 14.0).min(1.0));
            let to = flat(player.translation - xf.translation);
            if to.length_squared() > 1.0e-4 {
                xf.rotation = Quat::from_rotation_y(yaw_of(to));
            }
        }
        // The old strike: a distance and a cone (the kit's is [`strike`], from the posed hand).
        if kit.is_some() {
            continue;
        }
        let Mind::Attack { t, .. } = brain.mind else {
            continue;
        };
        if brain.struck != 0 || !player_alive || t < spec.hit.0 || t >= spec.hit.1 {
            continue;
        }
        if !strike_reaches(
            xf.translation,
            *xf.forward(),
            player.translation,
            hurt.radius,
        ) {
            continue;
        }
        if action.invulnerable() {
            if !brain.dodged {
                brain.dodged = true;
                info!("npc: {name}'s strike passes through the roll (t={t:.2})");
            }
            continue;
        }
        brain.struck = 1;
        hurts.write(PlayerHurt {
            from: e,
            damage: gear.0.taken(spec.damage, species.0.level, false),
            level: spec.level,
            stop: spec.stop,
            knock: spec.knock,
            poise: spec.poise,
            blocked: false,
        });
        info!("npc: {name} strikes the player (t={t:.2})");
    }
}

/// The hit window of a move that is open at `t` and has not struck yet (`struck` has bit `i` set for
/// every window `i` that has).
pub(crate) fn open_window(hits: &[(f32, f32)], t: f32, struck: u32) -> Option<usize> {
    hits.iter()
        .position(|w| t >= w.0 && t < w.1)
        .filter(|i| struck & (1 << i) == 0)
}

/// The kit's blow: through a move's hit window the kobold's hand volume (a capsule from its right
/// hand along its forearm, the move's radius thick) is swept against the player's hurtbox, once
/// per move; a roll's invulnerability lets it through, as it always did.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn strike(
    mut commands: Commands,
    fx: Option<Res<crate::abilities::fx::Fx>>,
    roster: Option<Res<Roster>>,
    defense: Option<Res<crate::defense::Kit>>,
    gear: Res<crate::gear_effect::GearFx>,
    mut shred: ResMut<crate::boss::ArmorShred>,
    m2s: Res<Assets<benilla_assets::M2Model>>,
    mut avatar: Query<
        (
            &Transform,
            &mut Action,
            &mut crate::combat::Stamina,
            &Hurtbox,
        ),
        (With<Avatar>, Without<Hostile>),
    >,
    mut npcs: Query<
        (
            Entity,
            &Species,
            &mut Brain,
            &GlobalTransform,
            &ActorModel,
            &benilla_world::rig_anim::RigPose,
            &mut HandTrail,
            Option<&HeldBuilt>,
            Option<&mut crate::named::Named>,
        ),
        With<Hostile>,
    >,
    roots: Query<&GlobalTransform>,
    mut hurts: MessageWriter<PlayerHurt>,
    (mut shocks, mut perfects): (
        MessageWriter<crate::shock::Landed>,
        MessageWriter<crate::defense::PerfectDefense>,
    ),
    collide: benilla_world::collision::WorldCollision,
) {
    let (Some(roster), Ok((player, mut action, mut stamina, hurt))) = (roster, avatar.single_mut())
    else {
        return;
    };
    for (e, species, mut brain, at, model, pose, mut trail, held, mut named) in &mut npcs {
        let Mind::Attack { t, mv } = brain.mind else {
            trail.0 = None;
            continue;
        };
        let Some(kit) = roster.get(species.kit()) else {
            continue;
        };
        let Some(a) = kit.attacks.get(usize::from(mv)) else {
            continue;
        };
        let at_bone = |bone: u16| pose.posed_point(at, bone, Vec3::ZERO);
        let (Some(wrist), Some(elbow)) = (at_bone(kit.reach.hand), at_bone(kit.reach.forearm))
        else {
            continue;
        };
        let scale = at.scale().y;
        // A hand blow starts at the hand's attachment point, a bone blow at the bone itself and a
        // blade blow runs along the held weapon's markers (a weapon without them swings the hand).
        // The hand that holds the weapon: the right's attachment point, the left's for a left-hander.
        let hand_id = if species.0.left_handed {
            crate::actor::held::ATTACH_HAND_LEFT
        } else {
            crate::actor::held::ATTACH_HAND_RIGHT
        };
        let hand = || {
            m2s.get(&model.m2)
                .and_then(|m| m.attachments.iter().find(|x| x.id == hand_id).copied())
                .and_then(|hand| pose.posed_point(at, hand.bone, hand.offset))
        };
        let along = |base: Vec3| {
            [
                base,
                base + (wrist - elbow).normalize_or_zero() * species.0.reach_length * scale,
            ]
        };
        // A cast throws its bolt from the hand as its blow opens, once, instead of sweeping a volume; so does a
        // named throw (a net, a web) made with a move of the kit.
        let throw = brain.throw;
        if let Some(bullet) = a.bullet.or(throw.map(crate::named::Throw::bullet)) {
            if let Some(window) = open_window(&a.spec.hits, t, brain.struck) {
                brain.struck |= 1 << window;
                if !action.is_dead() {
                    let from = hand().unwrap_or(wrist);
                    // A low mouth (a spider's spit) leaves a little over the turf, not in it.
                    let feet = at.translation().y;
                    let from = Vec3::new(from.x, from.y.max(feet + 0.6), from.z);
                    let (lo, hi) = hurt.axis(player.translation);
                    let bolt = crate::foebolt::FoeBolt {
                        from: e,
                        caster: species.0.name,
                        origin: at.translation(),
                        at: from,
                        velocity: Vec3::ZERO,
                        bullet,
                        age: 0.0,
                        damage: if throw.is_some() {
                            0.0
                        } else {
                            (a.spec.damage
                                * species.0.blow_scale()
                                * crate::foebolt::bolt_weight(&a.spec.name))
                            .round()
                        },
                        foe_level: species.0.level,
                        level: a.spec.level,
                        stop: a.spec.stop,
                        knock: a.spec.knock,
                        poise: a.spec.poise,
                        guard: defense
                            .as_deref()
                            .and_then(|k| k.attack(species.kit(), &a.spec.name)),
                        passed: false,
                        effect: throw,
                    };
                    crate::foebolt::release(
                        &mut commands,
                        fx.as_deref(),
                        bolt,
                        (lo + hi) * 0.5,
                        throw.map_or_else(
                            || crate::foebolt::look_of(species.0.name),
                            crate::named::Throw::missile,
                        ),
                    );
                    info!(
                        "npc: {} releases its {} (t={t:.2})",
                        species.0.name, a.spec.name
                    );
                }
            }
            trail.0 = None;
            continue;
        }
        let ends = if let Some(reach) = a.reach {
            at_bone(reach.hand)
                .zip(at_bone(reach.forearm))
                .map(|(a, b)| [a, b])
        } else {
            match species.0.blow {
                Blow::Hand => hand().map(along),
                Blow::Bone => Some(along(wrist)),
                Blow::Blade => held
                    .and_then(|h| h.blade.zip(roots.get(h.root).ok()))
                    .map(|([top, bottom], root)| {
                        [root.transform_point(bottom), root.transform_point(top)]
                    })
                    .or_else(|| hand().map(along)),
            }
        };
        let Some(ends) = ends else {
            continue;
        };
        let [base, tip] = ends;
        let now = crate::foehit::Volume {
            ends: [base, tip],
            radius: a.spec.radius.max(a.spec.radius2)
                * a.source_scale.unwrap_or(kit.scale)
                * scale
                * species.0.hit_radius_scale,
        };
        let prev = trail.0.replace([base, tip]);
        // A slam's ground shock and a power-up's blast ([`crate::shock`]): once a swing, at the clip's moment, hit
        // or miss; a player the blade already struck in this swing is not struck again by its shock.
        // The ground under the blade's tip (a slam on a hillside lands on the slope, not at the boss's feet).
        let ground = || crate::settle::snap_to_ground(&collide, tip).unwrap_or(at.translation().y);
        let kit_scale = a.source_scale.unwrap_or(kit.scale);
        for (shock, bit, blast) in [
            (a.shock, crate::shock::SHOCK_BIT, false),
            (a.blast, crate::shock::BLAST_BIT, true),
        ] {
            let Some(shock) = shock else { continue };
            if !crate::shock::due(shock.at, t, brain.struck & bit != 0) {
                continue;
            }
            let bladed = brain.struck & !(crate::shock::SHOCK_BIT | crate::shock::BLAST_BIT) != 0;
            brain.struck |= bit;
            let centre = if blast {
                at.translation()
            } else {
                crate::shock::centre(tip, ground())
            };
            let radius = crate::shock::radius_yd(shock.radius, kit_scale, scale);
            shocks.write(crate::shock::Landed {
                from: e,
                at: centre,
                radius,
                blast,
            });
            let caught = crate::shock::hits(
                centre,
                radius,
                player.translation,
                hurt.radius,
                action.invulnerable(),
            );
            info!(
                "npc: {}'s {} {} lands at {:?} radius {radius:.2}: {}",
                species.0.name,
                a.spec.name,
                if blast { "blast" } else { "shock" },
                centre.to_array().map(|v| (v * 100.0).round() / 100.0),
                if action.is_dead() || (bladed && !blast) {
                    "no second hit"
                } else if caught {
                    "the player is in it"
                } else {
                    "the player is clear"
                }
            );
            if !caught || action.is_dead() || (bladed && !blast) {
                continue;
            }
            let damage = (shock.damage * species.0.blow_scale()).round();
            let (damage, blocked) = crate::defense::intercept(
                defense.as_deref(),
                defense
                    .as_deref()
                    .and_then(|k| k.attack(species.kit(), &a.spec.name)),
                player,
                centre,
                &mut action,
                &mut stamina,
                damage,
            );
            let damage = gear
                .0
                .taken(damage * shred.factor(), species.0.level, blocked);
            hurts.write(PlayerHurt {
                from: e,
                damage,
                level: if blocked {
                    a.spec.level.min(shock.level)
                } else {
                    shock.level
                },
                stop: crate::hurt::HURT_STOP,
                knock: shock.knock,
                poise: shock.poise,
                blocked,
            });
            info!(
                "npc: {}'s {} {} strikes the player for {damage:.0}",
                species.0.name,
                a.spec.name,
                if blast { "blast" } else { "shock" }
            );
        }
        let Some(window) = open_window(&a.spec.hits, t, brain.struck) else {
            continue;
        };
        if action.is_dead() {
            continue;
        }
        let cap = hurt.axis(player.translation);
        if crate::actor::anim::trace_on() {
            // `WOWSOULS_TRACE=1`: where the volume is against the player's feet while a blow can land.
            let rel = |p: Vec3| {
                (p - player.translation)
                    .to_array()
                    .map(|v| (v * 100.0).round() / 100.0)
            };
            let local = at.rotation().inverse() * (player.translation - at.translation());
            // The volume's ends in the creature's own frame: right, ahead, up of its feet.
            let own = |p: Vec3| {
                let l = at.rotation().inverse() * (p - at.translation());
                [l.x, -l.z, l.y].map(|v| (v * 100.0).round() / 100.0)
            };
            info!(
                "strike {} {} t={t:.2} base {:?} tip {:?} radius {:.2}+{:.2} (to the player's feet); the player is {:+.2} right, {:+.2} ahead of the creature; own base {:?} tip {:?}",
                species.0.name,
                a.spec.name,
                rel(base),
                rel(tip),
                now.radius,
                hurt.radius,
                local.x,
                -local.z,
                own(base),
                own(tip)
            );
        }
        if !crate::foehit::touches(now, prev, cap, hurt.radius) {
            continue;
        }
        if action.invulnerable() {
            if !brain.dodged {
                brain.dodged = true;
                info!(
                    "npc: {}'s {} passes through the roll (t={t:.2})",
                    species.0.name, a.spec.name
                );
            }
            // A leap rolled through flies past: its flight outlasts the roll's i-frames, so the window
            // left open would catch the player as they stand up.
            if crate::latch::latches(&a.spec.name) {
                brain.struck |= 1 << window;
            }
            continue;
        }
        brain.struck |= 1 << window;
        // A boar's charge that connects is its big blow ([`crate::boar_rush::DAMAGE`]).
        let rush = species.0.behavior == crate::behavior::Role::BoarCharge
            && crate::boar_rush::is_charge(&a.spec.name);
        let (rush_damage, knock, poise) = if rush {
            (
                crate::boar_rush::DAMAGE,
                a.spec.knock.max(crate::boar_rush::KNOCK),
                a.spec.poise.max(crate::boar_rush::POISE),
            )
        } else {
            (1.0, a.spec.knock, a.spec.poise)
        };
        let damage = (a.spec.damage * species.0.blow_scale() * rush_damage).round();
        let (damage, blocked, perfect) = crate::defense::intercept_contact(
            defense.as_deref(),
            defense
                .as_deref()
                .and_then(|k| k.attack(species.kit(), &a.spec.name)),
            player,
            at.translation(),
            &mut action,
            &mut stamina,
            damage,
            a.reach.is_none(),
        );
        if perfect {
            perfects.write(crate::defense::PerfectDefense {
                attacker: e,
                point: player.translation + Vec3::Y * 1.1 + *player.forward() * 0.5,
                kind: crate::defense::PerfectKind::Block,
            });
        }
        // Armor always, a shield's block value only when the guard caught the blow; Pierce Armor
        // makes every blow count for more while it lasts.
        let damage = gear
            .0
            .taken(damage * shred.factor(), species.0.level, blocked);
        // A backstab from behind counts for more; a primed Gouge knocks down ([`crate::named`]).
        let (damage, level) = if blocked {
            (damage, a.spec.level)
        } else {
            crate::named::on_blow(
                named.as_deref_mut(),
                species.0.name,
                at.translation(),
                player,
                damage,
                a.spec.level,
            )
        };
        // A leap that lands holds on instead of knocking the player away ([`crate::latch`]).
        let latches = !blocked && crate::latch::latches(&a.spec.name);
        hurts.write(PlayerHurt {
            from: e,
            damage,
            level,
            stop: if blocked {
                crate::hurt::HURT_STOP
            } else {
                a.spec.stop
            },
            knock: if latches { 0.0 } else { knock },
            poise,
            blocked,
        });
        info!(
            "npc: {}'s {} strikes the player for {damage:.0} (t={t:.2})",
            species.0.name, a.spec.name
        );
        if latches {
            brain.latch = Some(crate::latch::Latch::new(
                at.translation() - player.translation,
            ));
            info!("npc: {} latches on", species.0.name);
        }
        // A landed blow of Hogger's adds a stack of Pierce Armor.
        if !blocked && crate::boss::pierces(species.0.name) {
            shred.add();
            info!("boss: Pierce Armor x{}", shred.stacks());
        }
    }
}

/// How far past the sim radius (yd) the creatures' sweeps still skip a resting creature's body: a
/// creature walking at the edge passes through its neighbours as everywhere else.
const EXCLUDE_MARGIN: f32 = 15.0;

/// The creatures' motor sweeps skip the creatures' body colliders (theirs and each other's): those
/// near the player and those of creatures still simulated far off. Every sweep copies the list, so
/// listing every body of a crowded valley would cost (motors x bodies) per frame.
#[allow(clippy::type_complexity)]
fn exclude_bodies(
    mut ex: ResMut<MoverTraceExclusions>,
    avatar: Query<&Transform, (With<Avatar>, Without<Hostile>)>,
    bodies: Query<(&BodyCollider, &Transform, &Brain), With<Hostile>>,
) {
    let Ok(player) = avatar.single() else {
        ex.0.extend(bodies.iter().map(|(b, ..)| b.0));
        return;
    };
    let reach = crate::ring::sim() + EXCLUDE_MARGIN;
    ex.0.extend(
        bodies
            .iter()
            .filter(|(_, xf, brain)| {
                let gap = Vec2::new(
                    player.translation.x - xf.translation.x,
                    player.translation.z - xf.translation.z,
                )
                .length();
                crate::ring::simulated(gap, brain.mind == Mind::Idle, reach)
            })
            .map(|(b, ..)| b.0),
    );
}

pub(crate) fn clear_exclusions(mut ex: ResMut<MoverTraceExclusions>) {
    ex.0.clear();
}

/// A resting creature's motor: standing on the ground it was placed on, so its gait reads Stand
/// (not a walk frozen mid-stride, nor a fall).
pub(crate) fn park(m: &mut Motor) {
    m.velocity = Vec3::ZERO;
    m.horiz_vel = Vec3::ZERO;
    m.vel_y = 0.0;
    m.grounded = true;
}

/// A motor standing on the ground: a body placed on snapped ground that has not stepped yet (a
/// never-stepped `Motor::default()` reads airborne, and its gait turns to Fall).
pub(crate) fn parked() -> Motor {
    let mut m = Motor::default();
    park(&mut m);
    m
}

/// A creature that keeps walking into a wall (2026-10-18, the user: "Buggat att dom fastnar på väggar"): how
/// long it has asked to move and got nowhere, and the detour it takes round the obstacle.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct Unstick {
    stuck: f32,
    detour: f32,
    turn: f32,
    /// A detour began this frame.
    fresh: bool,
}

/// Seconds of asking and getting nowhere before a detour, how long a detour lasts, how far it turns.
const STUCK_AFTER: f32 = 0.4;
const DETOUR_SECS: f32 = 0.7;
const DETOUR_TURN: f32 = 1.2;

impl Unstick {
    /// One frame: `asked` and `got` the planar speeds (yd/s). Returns the turn to put on the intent.
    pub fn step(&mut self, asked: f32, got: f32, dt: f32, open_side: impl FnOnce() -> f32) -> f32 {
        self.fresh = false;
        if self.detour > 0.0 {
            self.detour -= dt;
            return self.turn;
        }
        if asked > 1.0 && got < 0.25 * asked.min(2.0) {
            self.stuck += dt;
        } else {
            self.stuck = 0.0;
        }
        if self.stuck >= STUCK_AFTER {
            self.stuck = 0.0;
            self.detour = DETOUR_SECS;
            self.fresh = true;
            self.turn = open_side() * DETOUR_TURN;
            return self.turn;
        }
        0.0
    }
}

/// Steps each living, simulated creature's motor; it walks the player's capsule shape.
#[allow(clippy::type_complexity)]
fn move_hostiles(
    mut commands: Commands,
    time: Res<Time>,
    collide: WorldCollision,
    shape: Res<BodyShape>,
    avatar: Query<&Transform, (With<Avatar>, Without<Hostile>)>,
    mut npcs: Query<
        (
            Entity,
            &mut Transform,
            &mut Motor,
            &MoveIntent,
            &Reaction,
            &Brain,
            Option<&mut Unstick>,
        ),
        With<Hostile>,
    >,
) {
    let player = avatar.single().ok().map(|p| p.translation);
    let sim = crate::ring::sim();
    for (e, mut xf, mut m, intent, reaction, brain, unstick) in &mut npcs {
        if *reaction == Reaction::Dead {
            continue;
        }
        if let Some(p) = player {
            let gap = Vec2::new(p.x - xf.translation.x, p.z - xf.translation.z).length();
            if !crate::ring::simulated(gap, brain.mind == Mind::Idle, sim) {
                if !m.grounded || m.velocity != Vec3::ZERO {
                    park(&mut m);
                }
                continue;
            }
        }
        let mut feet = xf.translation;
        let mut intent = *intent;
        match unstick {
            Some(mut u) => {
                // Only travel detours: a swing's own steps press into the player by design.
                let travelling = !matches!(brain.mind, Mind::Attack { .. });
                let asked = if travelling {
                    intent.dir.length() * intent.speed
                } else {
                    0.0
                };
                let got = m.velocity.xz().length();
                let dir = Vec3::new(intent.dir.x, 0.0, intent.dir.z);
                let chest = feet + Vec3::Y * 1.0;
                let open = |turn: f32| {
                    let d = (Quat::from_rotation_y(turn) * dir).normalize_or_zero() * 2.0;
                    collide.cast_camera(chest, d, false).unwrap_or(2.0)
                };
                let turn = u.step(asked, got, time.delta_secs(), || {
                    if open(DETOUR_TURN) >= open(-DETOUR_TURN) {
                        1.0
                    } else {
                        -1.0
                    }
                });
                if turn != 0.0 && asked > 0.0 {
                    if u.fresh {
                        debug!("npc: {e} detours round an obstacle (turn {turn:+.1})");
                    }
                    intent.dir = Quat::from_rotation_y(turn) * intent.dir;
                }
            }
            None => {
                commands.entity(e).insert(Unstick::default());
            }
        }
        motor::step(&mut m, &mut feet, time.delta(), &collide, &shape.0, intent);
        if feet != xf.translation {
            xf.translation = feet;
        }
    }
}

/// Keeps each body collider on its creature.
fn follow_bodies(
    npcs: Query<(&Transform, &BodyCollider, &Home), (With<Hostile>, Changed<Transform>)>,
    mut bodies: Query<&mut Transform, Without<Hostile>>,
) {
    for (xf, body, home) in &npcs {
        if let Ok(mut b) = bodies.get_mut(body.0) {
            b.translation = xf.translation + Vec3::Y * (home.body.1 * 0.5);
        }
    }
}

/// `WOWSOULS_RESET=<s>`: press the F6 reset once at `s` seconds. An unattended-run instrument.
#[derive(Resource, Debug, Clone, Copy)]
struct ScriptedReset(f32);

/// F6: every creature home again, whole and calm, as when the player respawns (the player is
/// untouched): to fight them again without dying first.
fn reset_key(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time<bevy::time::Real>>,
    scripted: Option<Res<ScriptedReset>>,
    mut done: Local<bool>,
    mut respawned: MessageWriter<crate::hurt::Respawned>,
) {
    let scripted_now = scripted.is_some_and(|s| !*done && time.elapsed_secs() >= s.0);
    if scripted_now {
        *done = true;
    }
    if keys.just_pressed(KeyCode::F6) || scripted_now {
        respawned.write(crate::hurt::Respawned);
    }
}

/// The player respawned: every creature is home again, whole and calm.
#[allow(clippy::type_complexity)]
fn reset(
    mut commands: Commands,
    mut respawned: MessageReader<crate::hurt::Respawned>,
    mut npcs: Query<(
        Entity,
        &mut Transform,
        &Home,
        &mut Health,
        &mut Reaction,
        &mut Brain,
        &mut Hostile,
        &mut Motor,
        &Species,
        (&mut MonsterPlay, &mut MonsterFade, &mut Poise, &mut Roam),
        Option<&BodyCollider>,
    )>,
    mut epoch: ResMut<ColliderEpoch>,
) {
    if respawned.read().count() == 0 {
        return;
    }
    for (
        e,
        mut xf,
        home,
        mut hp,
        mut reaction,
        mut brain,
        mut hostile,
        mut m,
        species,
        kit,
        body,
    ) in &mut npcs
    {
        // A boss or a miniboss that is dead stays dead: neither the player's death nor F6 raises it.
        if crate::species::rank_of(species.0).never_respawns() && *reaction == Reaction::Dead {
            continue;
        }
        let (mut play, mut fade, mut poise, mut roam) = kit;
        roam.walk = Walk::default();
        *xf = Transform::from_translation(home.feet)
            .with_rotation(Quat::from_rotation_y(home.facing))
            .with_scale(xf.scale);
        *hp = Health::full(hp.max);
        *reaction = Reaction::None;
        *brain = Brain::default();
        hostile.alert = false;
        *m = Motor::default();
        play.clip = None;
        *fade = MonsterFade::default();
        *poise = Poise::new(poise.max);
        commands.entity(e).insert((home.lockable, home.hurtbox));
        if body.is_none() {
            let b = spawn_body(&mut commands, home.feet, home.body);
            commands.entity(e).insert(BodyCollider(b));
            epoch.bump();
        }
    }
    info!("npc: creatures reset at home");
}

#[cfg(test)]
mod tests {
    use super::perception::{MAX_RISE, SIGHT_RANGE};
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    const LEN: f32 = 1.0;

    fn sense(dist: f32) -> Sense {
        Sense {
            dist,
            facing: 1.0,
            player_alive: true,
            home_dist: 0.0,
            attack: (dist <= ATTACK_DISTANCE).then_some(0),
            cooldown: COOLDOWN,
            seen: true,
            rise: 0.0,
            noise: 0.0,
        }
    }

    #[test]
    fn a_hit_reveals_the_player_without_flinching_or_interrupting_an_attack() {
        use bevy::ecs::system::RunSystemOnce;
        for initial in [Mind::Idle, Mind::Return, Mind::Attack { t: 0.3, mv: 0 }] {
            let mut app = App::new();
            app.init_resource::<ColliderEpoch>()
                .add_message::<crate::hits::Hit>()
                .add_message::<Slain>();
            let target = app
                .world_mut()
                .spawn((
                    Species::new(&crate::species::KOBOLD),
                    Transform::default(),
                    Reaction::None,
                    Hostile::default(),
                    Brain {
                        mind: initial,
                        ..Default::default()
                    },
                    Poise::new(100.0),
                    MonsterPlay::default(),
                ))
                .id();
            app.world_mut().write_message(crate::hits::Hit {
                target,
                damage: 1.0,
                point: Vec3::ZERO,
                lethal: false,
                level: 0,
                poise: 0.0,
                from: Vec3::Z * 25.0,
                knock: 0.0,
            });
            app.world_mut().run_system_once(react).unwrap();
            assert!(app.world().get::<Hostile>(target).unwrap().alert);
            let expected = if initial.engaged() {
                initial
            } else {
                Mind::Chase
            };
            assert_eq!(app.world().get::<Brain>(target).unwrap().mind, expected);
            assert_eq!(
                *app.world().get::<Reaction>(target).unwrap(),
                Reaction::None
            );
        }
    }

    #[test]
    fn a_creature_walking_into_a_wall_detours_and_a_free_one_never_does() {
        let mut u = Unstick::default();
        // Walking freely: no turn, however long.
        for _ in 0..100 {
            assert_eq!(u.step(4.0, 4.0, 0.05, || 1.0), 0.0);
        }
        // Against a wall: after STUCK_AFTER it turns to the open side for DETOUR_SECS.
        let mut turned = 0;
        for _ in 0..40 {
            if u.step(4.0, 0.0, 0.05, || -1.0) != 0.0 {
                turned += 1;
            }
        }
        assert!(turned >= 10, "{turned}");
        assert_eq!(
            Unstick::default().step(0.0, 0.0, 1.0, || 1.0),
            0.0,
            "standing still is not stuck"
        );
    }

    #[test]
    fn it_does_not_notice_a_player_it_cannot_see_or_far_above_it() {
        let mut m = Mind::Idle;
        think_step(
            &mut m,
            Sense {
                seen: false,
                ..sense(5.0)
            },
            LEN,
            DT,
        );
        assert_eq!(m, Mind::Idle, "a wall or the ground between them");
        think_step(
            &mut m,
            Sense {
                rise: MAX_RISE + 1.0,
                ..sense(5.0)
            },
            LEN,
            DT,
        );
        assert_eq!(m, Mind::Idle, "on the meadow over its mine");
        think_step(&mut m, sense(5.0), LEN, DT);
        assert_eq!(m, Mind::Chase);
        // Up a cliff out of its reach, the fight is over.
        think_step(
            &mut m,
            Sense {
                rise: LEASH_RISE + 1.0,
                ..sense(5.0)
            },
            LEN,
            DT,
        );
        assert_eq!(m, Mind::Idle);
    }

    #[test]
    fn it_notices_the_player_in_range_only() {
        let mut m = Mind::Idle;
        think_step(&mut m, sense(SIGHT_RANGE + 1.0), LEN, DT);
        assert_eq!(m, Mind::Idle);
        think_step(&mut m, sense(SIGHT_RANGE - 1.0), LEN, DT);
        assert_eq!(m, Mind::Chase);
        // Once chasing, it holds on out to the leash.
        think_step(&mut m, sense(SIGHT_RANGE + 5.0), LEN, DT);
        assert_eq!(m, Mind::Chase);
    }

    #[test]
    fn a_quiet_approach_behind_is_hidden_until_heard_and_combat_keeps_its_memory() {
        let behind = Sense {
            facing: -1.0,
            home_dist: 8.0,
            ..sense(5.0)
        };
        for initial in [Mind::Idle, Mind::Return] {
            let mut m = initial;
            think_step(&mut m, behind, LEN, DT);
            assert!(!m.engaged());
            think_step(
                &mut m,
                Sense {
                    noise: 6.0,
                    ..behind
                },
                LEN,
                DT,
            );
            assert_eq!(m, Mind::Chase);
            think_step(
                &mut m,
                Sense {
                    seen: false,
                    ..behind
                },
                LEN,
                DT,
            );
            assert_eq!(
                m,
                Mind::Chase,
                "looking away or stopping does not erase a fight"
            );
        }
        let mut m = Mind::Idle;
        think_step(
            &mut m,
            Sense {
                player_alive: false,
                noise: 16.0,
                ..behind
            },
            LEN,
            DT,
        );
        assert!(!m.engaged(), "a dead player cannot wake an enemy");
    }

    #[test]
    fn past_the_leash_or_with_the_player_dead_it_goes_home() {
        let away = |s: Sense| Sense {
            home_dist: 8.0,
            ..s
        };
        let mut m = Mind::Chase;
        think_step(&mut m, away(sense(LEASH_RANGE + 1.0)), LEN, DT);
        assert_eq!(m, Mind::Return);
        let mut m = Mind::Cooldown { left: 1.0 };
        let dead = Sense {
            player_alive: false,
            ..away(sense(2.0))
        };
        think_step(&mut m, dead, LEN, DT);
        assert_eq!(m, Mind::Return);
        // A dead player is not noticed on the way home either.
        think_step(&mut m, dead, LEN, DT);
        assert_eq!(m, Mind::Return);
        // Home, it stands.
        think_step(
            &mut m,
            Sense {
                home_dist: 0.1,
                ..dead
            },
            LEN,
            DT,
        );
        assert_eq!(m, Mind::Idle);
    }

    #[test]
    fn in_reach_and_square_it_swings_then_cools_down_then_chases() {
        let mut m = Mind::Chase;
        // In reach but not facing: it keeps turning.
        let askew = Sense {
            facing: 0.5,
            ..sense(1.5)
        };
        assert_eq!(think_step(&mut m, askew, LEN, DT), Cue::None);
        assert_eq!(m, Mind::Chase);
        assert_eq!(think_step(&mut m, sense(1.5), LEN, DT), Cue::AttackStarted);
        // The swing runs its length whatever the player does.
        let mut t = 0.0;
        let mut cue = Cue::None;
        while cue == Cue::None {
            cue = think_step(&mut m, sense(25.0), LEN, DT);
            t += DT;
            assert!(t < 2.0);
        }
        assert_eq!(cue, Cue::AttackEnded);
        assert!((t - LEN).abs() < 2.0 * DT, "{t}");
        assert!(matches!(m, Mind::Cooldown { .. }));
        let mut t = 0.0;
        while m != Mind::Chase {
            think_step(&mut m, sense(1.5), LEN, DT);
            t += DT;
            assert!(t < 3.0);
        }
        assert!((t - COOLDOWN).abs() < 2.0 * DT, "{t}");
    }

    #[test]
    fn a_creature_with_a_recovery_cuts_the_tail_of_its_clip() {
        let spec = AttackSpec {
            length: 4.7,
            hits: vec![(1.37, 1.70), (2.0, 2.2)],
            ..procedural_attack(4.7)
        };
        // No recovery set: the whole clip, as before.
        assert_eq!(swing_length(&spec, None), 4.7);
        // The move ends a recovery after its last window.
        assert!((swing_length(&spec, Some(1.0)) - 3.2).abs() < 1.0e-6);
        // A recovery past the clip's end changes nothing, and a move with no window is whole.
        assert_eq!(swing_length(&spec, Some(9.0)), 4.7);
        let none = AttackSpec {
            hits: vec![],
            ..spec
        };
        assert_eq!(swing_length(&none, Some(1.0)), 4.7);
    }

    #[test]
    fn the_procedural_swing_keeps_the_old_kobolds_numbers() {
        // Attack1H at the slowed rate: 1.43 s, strike 0.34-0.48 of it (0.49-0.69 s), 25 damage.
        let a = procedural_attack(1.43);
        assert_eq!(a.hit, (STRIKE_FROM * 1.43, STRIKE_TO * 1.43));
        assert!((a.hit.0 - 0.486).abs() < 0.001 && (a.hit.1 - 0.686).abs() < 0.001);
        assert_eq!((a.damage, a.poise, a.level), (KOBOLD_DAMAGE, 0.0, 1));
        assert_eq!(a.length, 1.43);
    }

    #[test]
    fn it_starts_the_move_it_was_given_and_waits_without_one() {
        let mut m = Mind::Chase;
        let none = Sense {
            attack: None,
            ..sense(1.5)
        };
        assert_eq!(think_step(&mut m, none, LEN, DT), Cue::None);
        assert_eq!(m, Mind::Chase);
        let go = Sense {
            attack: Some(4),
            ..sense(3.0)
        };
        assert_eq!(think_step(&mut m, go, LEN, DT), Cue::AttackStarted);
        assert_eq!(m, Mind::Attack { t: 0.0, mv: 4 });
        // The cooldown after the swing is the one the sense carries.
        let slow = Sense {
            cooldown: 0.3,
            ..sense(3.0)
        };
        while matches!(m, Mind::Attack { .. }) {
            think_step(&mut m, slow, LEN, DT);
        }
        assert_eq!(m, Mind::Cooldown { left: 0.3 });
    }

    #[test]
    fn the_strike_reaches_a_body_ahead_not_behind_or_far() {
        let fwd = Vec3::NEG_Z;
        assert!(strike_reaches(
            Vec3::ZERO,
            fwd,
            Vec3::new(0.0, 0.0, -1.7),
            0.33
        ));
        assert!(!strike_reaches(
            Vec3::ZERO,
            fwd,
            Vec3::new(0.0, 0.0, -2.5),
            0.33
        ));
        assert!(!strike_reaches(
            Vec3::ZERO,
            fwd,
            Vec3::new(0.0, 0.0, 1.0),
            0.33
        ));
        // Off to the side past the cone.
        assert!(!strike_reaches(
            Vec3::ZERO,
            fwd,
            Vec3::new(1.2, 0.0, -0.3),
            0.33
        ));
    }

    #[test]
    fn a_roaming_creature_inside_its_area_stays_idle() {
        // A wanderer 6 yd from its spawn point inside a 10 yd radius is home: `home_distance` gives
        // 0, and the mind stays Idle (the old 0.6 yd slack would have sent it back every frame).
        let area = crate::roam::Area::Wander(10.0);
        let (gap, _) = crate::roam::home_distance(&area, [0.0, 0.0], [6.0, 0.0]);
        let mut m = Mind::Idle;
        for _ in 0..120 {
            think_step(
                &mut m,
                Sense {
                    home_dist: gap,
                    ..sense(SIGHT_RANGE + 10.0)
                },
                LEN,
                DT,
            );
            assert_eq!(m, Mind::Idle);
        }
        // Dragged 5 yd past its edge it walks back, and is home again at the edge.
        let (out, back) = crate::roam::home_distance(&area, [0.0, 0.0], [15.0, 0.0]);
        assert!((out - 5.0).abs() < 1e-4);
        assert_eq!(back, [0.0, 0.0]);
        think_step(
            &mut m,
            Sense {
                home_dist: out,
                ..sense(SIGHT_RANGE + 10.0)
            },
            LEN,
            DT,
        );
        assert_eq!(m, Mind::Return);
        let (edge, _) = crate::roam::home_distance(&area, [0.0, 0.0], [9.9, 0.0]);
        think_step(
            &mut m,
            Sense {
                home_dist: edge,
                ..sense(SIGHT_RANGE + 10.0)
            },
            LEN,
            DT,
        );
        assert_eq!(m, Mind::Idle);
    }

    #[test]
    fn a_ring_spawn_takes_only_ground_near_its_height() {
        // The hand-placed groups take what they find, or keep their seed.
        assert_eq!(pick_ground(80.0, Some(81.2), false), Some(81.2));
        assert_eq!(pick_ground(80.0, None, false), Some(80.0));
        assert_eq!(pick_ground(80.0, Some(120.0), false), Some(120.0));
        // A ring spawn waits for ground near the database's height.
        assert_eq!(pick_ground(80.0, Some(81.2), true), Some(81.2));
        assert_eq!(pick_ground(80.0, Some(79.0), true), Some(79.0));
        assert_eq!(pick_ground(80.0, None, true), None);
        assert_eq!(
            pick_ground(80.0, Some(80.0 + GROUND_TOLERANCE + 0.5), true),
            None,
            "a hill over a cave"
        );
    }

    #[test]
    fn a_roam_has_its_own_dice_and_a_staggered_start() {
        let a = Roam::new(crate::roam::Area::Wander(5.0), 80020);
        let b = Roam::new(crate::roam::Area::Wander(5.0), 80021);
        assert_ne!(a.rng.0, b.rng.0, "neighbouring guids roll apart");
        // The same guid rolls the same (a repeatable valley).
        assert_eq!(
            Roam::new(crate::roam::Area::Stand, 7).rng.0,
            Roam::new(crate::roam::Area::Stand, 7).rng.0
        );
    }

    #[test]
    fn each_hit_window_of_a_move_strikes_once() {
        let windows = [(0.53, 0.77), (1.17, 1.40)];
        assert_eq!(open_window(&windows, 0.3, 0), None, "before the first");
        assert_eq!(open_window(&windows, 0.6, 0), Some(0));
        assert_eq!(open_window(&windows, 0.6, 0b01), None, "it has struck");
        assert_eq!(open_window(&windows, 0.9, 0b01), None, "between the two");
        assert_eq!(
            open_window(&windows, 1.2, 0b01),
            Some(1),
            "the second still can"
        );
        assert_eq!(open_window(&windows, 1.2, 0b11), None);
        assert_eq!(
            open_window(&windows, 0.77, 0),
            None,
            "a window is half open"
        );
        assert_eq!(open_window(&[], 0.5, 0), None);
    }
}
