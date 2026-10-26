//! Rage, mana and the abilities in play ([`wow_world::abilities`]: what exists, what the trainer asks;
//! [`wow_world::class`]: whose they are). A warrior has no mana: **rage** builds from the blows he deals
//! and takes and fades away when the fighting stops, and an ability spends it. A mage has **mana**: a pool
//! from the class tables and Intellect that comes back with Spirit once five seconds have passed without
//! a cast. The abilities known (what the trainer taught) sit on a four-slot bar; the keys 1 to 4 use a
//! slot, Z / the left-stick click cycles the selected one and the D-pad down uses it (the D-pad left is
//! the ranged stance).
//!
//! The warrior's set is the user's: **Charge** (a rush at an enemy that stuns it and gives rage),
//! **Battle Shout** (blows do 15 % more damage for two minutes) and **Thunder Clap** (the enemies around
//! take damage and are slowed); the mage's: **Fireball**, **Frost Armor**, **Frostbolt** and **Fire
//! Blast** (`crate::spells`). What each does in the world is in [`moves`]; how it looks and sounds is in
//! [`fx`] (WoW's own spell models and sounds); the bar and the rage / mana bar are in [`ui`].
//!
//! The damage bonus is read by the strike ([`Buffs::damage_factor`] in `hits.rs`); the rest is here.
//!
//! Switches: `WOWSOULS_RAGE=<n>` starts with that much rage; `WOWSOULS_MANA=<n>` starts a mage with that
//! much mana (at most its maximum); `WOWSOULS_ABILITY_SCRIPT=<s>:<1-4|cycle|use>,..` presses the ability
//! keys of an unattended run.

use std::collections::BTreeMap;

use bevy::prelude::*;

use wow_world::abilities::{self, Ability, Effect, RAGE_MAX};
use wow_world::class::{self, Power};

use crate::audio::Audio;
use crate::avatar::Avatar;
use crate::combat::{Action, Health};
use crate::controller::PlaySet;
use crate::equip::Card;
use crate::hits::Hit;
use crate::hurt::Respawned;
use crate::lockon::LockOn;
use crate::loot::{Owned, Screen};
use crate::npc::{Hostile, PlayerHurt, Reaction};
use crate::quests::{QuestGame, Toasts};
use crate::settings::binds::Action as BindAction;
use crate::settings::Controls;
use crate::settle::Phase;

pub mod fx;
pub mod moves;
pub mod ui;

pub use moves::{apply_slow, stun, Slowed};
use moves::{Casting, Charging, Rush, ThunderClap};

/// What Battle Shout adds to the damage, and how long a shout lasts (s).
pub const SHOUT_BONUS: f32 = 1.15;
pub const SHOUT_SECS: f32 = 120.0;
/// Rage per point of damage dealt and taken (WoW's own is near 1 and 2.5 per point of its smaller numbers).
const DEALT: f32 = 0.5;
const TAKEN: f32 = 0.25;
/// Rage starts to fade this many seconds after the last blow, by this much per second.
const QUIET_AFTER: f32 = 5.0;
const FADE: f32 = 2.0;
/// Seconds after a cast before mana comes back (WoW's five-second rule).
pub const MANA_QUIET: f32 = 5.0;
/// Mana a second per point of Spirit once it comes back: a level-1 human mage (Spirit 22) regains about
/// 1.5 a second. Claude's first guess, tuned in play.
pub const REGEN_PER_SPIRIT: f32 = 0.07;

/// The abilities' icons (`Interface\Icons\<icon>`), by spell.
#[derive(Resource, Default)]
pub struct AbilityArt(pub BTreeMap<u32, Handle<Image>>);

/// The warrior's rage and how long since a blow was dealt or taken.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct Rage {
    pub cur: f32,
    pub quiet: f32,
    /// Damage an ability dealt that the rage has not yet been told to ignore: what an ability does does
    /// not build rage (Thunder Clap would pay for itself).
    pub ability_damage: f32,
}

impl Rage {
    /// Rage from a blow dealt (`dealt`) or taken.
    pub fn gain(&mut self, damage: f32, dealt: bool) {
        let k = if dealt { DEALT } else { TAKEN };
        self.cur = (self.cur + damage.max(0.0) * k).min(RAGE_MAX as f32);
        self.quiet = 0.0;
    }

    /// Rage from a blow dealt of `damage` points, less what an ability dealt in it.
    pub fn gain_from_blow(&mut self, damage: f32) {
        let ability = self.ability_damage.min(damage);
        self.ability_damage -= ability;
        self.gain(damage - ability, true);
    }

    /// `dt` seconds pass: out of the fight for a while, rage drains.
    pub fn tick(&mut self, dt: f32) {
        self.quiet += dt;
        if self.quiet > QUIET_AFTER {
            self.cur = (self.cur - FADE * dt).max(0.0);
        }
    }
}

/// A caster's mana pool and what refills it.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct Mana {
    pub cur: f32,
    pub max: f32,
    /// Seconds since the last cast.
    pub quiet: f32,
    /// The character's Spirit, which sets the trickle.
    pub spirit: f32,
    /// What a shorter bar took below zero, paid back when the bar grows again (as `level::follow`'s
    /// debt does for health): taking Intellect gear off and putting it back is no free refill.
    debt: f32,
    /// The pool was sized at least once: the first time it starts full.
    set: bool,
}

impl Mana {
    /// The tables and the gear say `max` and `spirit` now. The first call fills the pool (or keeps a
    /// positive `cur` a debug switch put there, clamped); later ones move the current value by the same
    /// difference as the maximum, so a longer bar is not a refill, and never leave it outside `0..=max`.
    pub fn sync(&mut self, max: f32, spirit: f32) {
        self.spirit = spirit;
        if !self.set {
            self.set = true;
            self.cur = if self.cur > 0.0 {
                self.cur.min(max)
            } else {
                max
            };
            self.max = max;
            return;
        }
        if self.cur >= self.max {
            self.debt = 0.0;
        }
        let raw = self.cur + max - self.max - self.debt;
        if raw < 0.0 {
            self.cur = 0.0;
            self.debt = -raw;
        } else {
            self.cur = raw.min(max);
            self.debt = 0.0;
        }
        self.max = max;
    }

    pub fn spend(&mut self, cost: f32) {
        self.cur = (self.cur - cost).max(0.0);
        self.quiet = 0.0;
    }

    /// `dt` seconds pass: after the quiet, Spirit gives mana back.
    pub fn tick(&mut self, dt: f32) {
        self.quiet += dt;
        if self.quiet >= MANA_QUIET && self.max > 0.0 {
            self.cur = (self.cur + REGEN_PER_SPIRIT * self.spirit * dt).min(self.max);
        }
    }

    /// A new life starts with a full pool.
    pub fn refill(&mut self) {
        self.cur = self.max;
        self.quiet = MANA_QUIET;
        self.debt = 0.0;
    }
}

/// Which class the character is and what it spends, from the quest game's player.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveClass {
    pub id: u8,
    pub power: Power,
}

impl Default for ActiveClass {
    fn default() -> Self {
        ActiveClass {
            id: class::WARRIOR.id,
            power: Power::Rage,
        }
    }
}

/// Rage builds from blows for a class that has rage; a mage's fight builds none.
pub fn builds_rage(power: Power) -> bool {
    power == Power::Rage
}

fn follow_class(game: Option<Res<QuestGame>>, mut active: ResMut<ActiveClass>) {
    let Some(game) = game else {
        return;
    };
    let id = game.player.class as u8;
    if active.id != id {
        if let Some(def) = class::def(id) {
            *active = ActiveClass {
                id,
                power: def.power,
            };
        }
    }
}

/// The effects that are running.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct Buffs {
    /// Seconds of Battle Shout left.
    pub shout: f32,
    /// Seconds of Frost Armor left.
    pub frost_armor: f32,
}

impl Buffs {
    /// `dt` seconds pass.
    pub fn tick(&mut self, dt: f32) {
        self.shout = (self.shout - dt).max(0.0);
        self.frost_armor = (self.frost_armor - dt).max(0.0);
    }

    /// What the damage of a blow is multiplied by right now.
    pub fn damage_factor(&self) -> f32 {
        if self.shout > 0.0 {
            SHOUT_BONUS
        } else {
            1.0
        }
    }
}

/// Seconds left before each ability (by spell) can be used again.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Cooldowns(BTreeMap<u32, f32>);

impl Cooldowns {
    pub fn left(&self, spell: u32) -> f32 {
        self.0.get(&spell).copied().unwrap_or(0.0)
    }

    pub fn start(&mut self, spell: u32, secs: f32) {
        self.0.insert(spell, secs);
    }

    pub fn tick(&mut self, dt: f32) {
        for left in self.0.values_mut() {
            *left = (*left - dt).max(0.0);
        }
        self.0.retain(|_, left| *left > 0.0);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// The four slots: spells of the known, usable abilities, and the one selected.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct Hotbar {
    pub slots: Vec<u32>,
    pub selected: usize,
}

/// The slots for what is learned: the known and usable abilities of `class` in list order, four at most.
pub fn slots_for(learned: &std::collections::BTreeSet<u32>, class: u8) -> Vec<u32> {
    abilities::known(learned, class)
        .into_iter()
        .filter(|a| a.usable)
        .map(|a| a.spell)
        .take(4)
        .collect()
}

/// Why an ability could not be used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refusal {
    NotEnoughStamina,
    ShieldRequired,
    /// The power (rage or mana) the ability is paid in is short.
    NotEnough(Power),
    /// Not ready: this many seconds are left of its cooldown.
    NotReady(f32),
    /// Nothing to do with it yet.
    NotYet,
    /// Charge: no enemy ahead, or one too close or too far.
    NoTarget,
    TooClose,
    TooFar,
    /// The body is busy (a swing, a roll, a charge under way).
    Busy,
}

impl Refusal {
    pub fn message(self) -> String {
        match self {
            Refusal::NotEnoughStamina => "Not enough stamina.".into(),
            Refusal::ShieldRequired => "You must hold a shield.".into(),
            Refusal::NotEnough(p) => format!("Not enough {}.", p.word()),
            Refusal::NotReady(s) => format!("Not ready yet ({:.0} s).", s.ceil()),
            Refusal::NotYet => "You cannot do that yet.".into(),
            Refusal::NoTarget => "You have no target.".into(),
            Refusal::TooClose => "Target too close.".into(),
            Refusal::TooFar => "Out of range.".into(),
            Refusal::Busy => "You cannot do that now.".into(),
        }
    }
}

/// What a used ability asks of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cast {
    Melee(u32),
    Wall,
    Shout,
    Charge(Entity),
    Clap,
    /// A mage's spell: the body is asked to start its cast (`crate::spells`, the release is its).
    Spell(u32),
}

/// Uses an ability: checks it (usable, ready, its power, for a charge its target `aim`), then spends
/// the rage or the mana, starts the cooldown and the buff, and says what the world is to do. Nothing
/// changes on a refusal.
pub fn activate(
    rage: &mut Rage,
    mana: &mut Mana,
    buffs: &mut Buffs,
    cooldowns: &mut Cooldowns,
    ability: &Ability,
    aim: Option<Result<Entity, Refusal>>,
) -> Result<Cast, Refusal> {
    if !ability.usable {
        return Err(Refusal::NotYet);
    }
    let wait = cooldowns.left(ability.spell);
    if wait > 0.0 {
        return Err(Refusal::NotReady(wait));
    }
    let have = match ability.power {
        Power::Rage => rage.cur,
        Power::Mana => mana.cur,
    };
    if have < ability.cost as f32 {
        return Err(Refusal::NotEnough(ability.power));
    }
    let cast = match ability.effect {
        Effect::MortalStrike | Effect::Bloodthirst => Cast::Melee(ability.spell),
        Effect::ShieldWall => Cast::Wall,
        Effect::BattleShout => Cast::Shout,
        Effect::Charge => Cast::Charge(aim.unwrap_or(Err(Refusal::NoTarget))?),
        Effect::ThunderClap => Cast::Clap,
        Effect::Fireball | Effect::Frostbolt | Effect::FireBlast | Effect::FrostArmor => {
            Cast::Spell(ability.spell)
        }
    };
    match ability.power {
        Power::Rage => rage.cur -= ability.cost as f32,
        Power::Mana => mana.spend(ability.cost as f32),
    }
    if cast == Cast::Shout {
        buffs.shout = SHOUT_SECS;
    }
    let wait = moves::cooldown(ability.effect);
    if wait > 0.0 {
        cooldowns.start(ability.spell, wait);
    }
    Ok(cast)
}

/// The card the shop and trainer screens show for an ability.
pub fn card(spell: u32) -> Option<Card> {
    let a = abilities::find(spell)?;
    let cost = if a.cost > 0 {
        let word = match a.power {
            wow_world::class::Power::Rage => "Rage",
            wow_world::class::Power::Mana => "Mana",
        };
        format!("{} {word}", a.cost)
    } else {
        format!("No {}", a.power.word())
    };
    Some(Card {
        name: a.name.to_string(),
        quality: 1,
        tooltip: format!("Rank {}   {cost}\n{}", a.rank, a.text),
        warning: (!a.usable).then(|| "The game cannot use it yet.".to_string()),
        versus: if a.copper > 0 {
            format!("Requires level {}", a.level)
        } else {
            String::new()
        },
        compare: Vec::new(),
        worn: None,
    })
}

fn load_art(mut commands: Commands, assets: Res<AssetServer>) {
    let icons = abilities::ALL
        .iter()
        .map(|a| {
            (
                a.spell,
                crate::loot::ui::load_icon(&assets, &format!("Interface/Icons/{}", a.icon)),
            )
        })
        .collect();
    commands.insert_resource(AbilityArt(icons));
}

/// Rage from the blows of the fight (for a class that has rage), mana's trickle, the fading, the timers,
/// and a death that ends it all.
#[allow(clippy::too_many_arguments)]
fn upkeep(
    time: Res<Time>,
    active: Res<ActiveClass>,
    mut rage: ResMut<Rage>,
    mut mana: ResMut<Mana>,
    mut buffs: ResMut<Buffs>,
    mut cooldowns: ResMut<Cooldowns>,
    mut hits: MessageReader<Hit>,
    mut hurts: MessageReader<PlayerHurt>,
    mut respawned: MessageReader<Respawned>,
    talents: Res<crate::talents::Talents>,
) {
    let rages = builds_rage(active.power);
    for h in hits.read() {
        if rages {
            let before = rage.cur;
            rage.gain_from_blow(h.damage);
            let bonus = (rage.cur - before)
                * 0.08
                * f32::from(talents.rank(wow_world::talents::TalentId::UnbridledWrath));
            rage.cur = (rage.cur + bonus).min(RAGE_MAX as f32);
        }
    }
    for h in hurts.read() {
        // A death blow's thousands are not a fight.
        if rages {
            rage.gain(h.damage.min(100.0), false);
        }
    }
    let dt = time.delta_secs();
    let before_decay = rage.cur;
    rage.tick(dt);
    rage.cur += (before_decay - rage.cur)
        * 0.10
        * f32::from(talents.rank(wow_world::talents::TalentId::TacticalMastery));
    mana.tick(dt);
    cooldowns.tick(dt);
    buffs.tick(dt);
    if respawned.read().count() > 0 {
        *rage = Rage::default();
        // A new life starts with a full pool: a mage at nothing would be stuck.
        mana.refill();
        *buffs = Buffs::default();
        cooldowns.clear();
    }
}

/// The bar follows what is learned.
fn refit(
    owned: Res<Owned>,
    game: Option<Res<QuestGame>>,
    mut bar: ResMut<Hotbar>,
    talents: Res<crate::talents::Talents>,
) {
    let class = game.map_or(1, |g| g.player.class as u8);
    let mut slots = slots_for(&owned.progress.abilities, class);
    if let Some(spell) = talents.signature() {
        slots.push(spell);
    }
    if bar.slots != slots {
        bar.selected = bar.selected.min(slots.len().saturating_sub(1));
        bar.slots = slots;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Press {
    Slot(usize),
    Cycle,
    UseSelected,
}

fn parse_script(text: &str) -> Vec<(f32, Press)> {
    text.split(',')
        .filter_map(|step| {
            let (at, what) = step.trim().split_once(':')?;
            let press = match what.trim() {
                "cycle" => Press::Cycle,
                "use" => Press::UseSelected,
                n => Press::Slot(n.parse::<usize>().ok()?.checked_sub(1)?),
            };
            Some((at.trim().parse().ok()?, press))
        })
        .collect()
}

/// The keys, the buttons and the script.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn input(
    mut commands: Commands,
    (controls, real, unattended): (
        Controls,
        Res<Time<Real>>,
        Option<Res<crate::run_mode::Unattended>>,
    ),
    (screen, lock, fx): (Res<Screen>, Res<LockOn>, Res<fx::Fx>),
    (mut bar, mut rage, mut mana, mut buffs, mut cooldowns, mut charging): (
        ResMut<Hotbar>,
        ResMut<Rage>,
        ResMut<Mana>,
        ResMut<Buffs>,
        ResMut<Cooldowns>,
        ResMut<Charging>,
    ),
    (mut toasts, mut claps, audio, casts): (
        ResMut<Toasts>,
        MessageWriter<ThunderClap>,
        Option<ResMut<Audio>>,
        Option<Res<crate::spells::cast::Kit>>,
    ),
    mut avatar: Query<
        (
            Entity,
            &Transform,
            &mut Action,
            &Health,
            &mut crate::combat::Stamina,
            &crate::motor::Motor,
        ),
        With<Avatar>,
    >,
    foes: Query<(Entity, &GlobalTransform, &Reaction), (With<Hostile>, Without<Avatar>)>,
    mut script: Local<Option<Vec<(f32, Press)>>>,
    mut next_step: Local<usize>,
    (talents, mut warrior, weapon, gear, menu): (
        Res<crate::talents::Talents>,
        ResMut<crate::warrior::Warrior>,
        Option<Res<crate::motion_clip::WeaponClips>>,
        Res<crate::gear_effect::GearFx>,
        Res<crate::settings::MenuUp>,
    ),
) {
    let Ok((me, xf, mut action, health, mut stamina, motor)) = avatar.single_mut() else {
        return;
    };
    let now = real.elapsed_secs();
    let mut presses: Vec<Press> = Vec::new();
    if unattended.is_none() {
        // The slots, the cycle and the use button are the player's bindings (1-4, Z, D-pad down).
        for (i, action) in [
            BindAction::Ability1,
            BindAction::Ability2,
            BindAction::Ability3,
            BindAction::Ability4,
        ]
        .into_iter()
        .enumerate()
        {
            if controls.just_pressed(action) {
                presses.push(Press::Slot(i));
            }
        }
        if controls.just_pressed(BindAction::AbilityCycle) {
            presses.push(Press::Cycle);
        }
        if controls.just_pressed(BindAction::AbilityUse) {
            presses.push(Press::UseSelected);
        }
    }
    let steps = script.get_or_insert_with(|| {
        std::env::var("WOWSOULS_ABILITY_SCRIPT")
            .map(|t| parse_script(&t))
            .unwrap_or_default()
    });
    while let Some(&(at, press)) = steps.get(*next_step) {
        if at > now {
            break;
        }
        *next_step += 1;
        presses.push(press);
    }
    // Not under a screen, not dead, not mid-reaction.
    if presses.is_empty()
        || menu.0
        || screen.open
        || screen.blocks_actions(now)
        || action.is_dead()
        || health.cur <= 0.0
    {
        return;
    }
    let mut audio = audio.map(|a| a.into_inner());
    for press in presses {
        let slot = match press {
            Press::Cycle => {
                if !bar.slots.is_empty() {
                    bar.selected = (bar.selected + 1) % bar.slots.len();
                }
                continue;
            }
            Press::Slot(i) => i,
            Press::UseSelected => bar.selected,
        };
        let Some(&spell) = bar.slots.get(slot) else {
            continue;
        };
        bar.selected = slot;
        let Some(ability) = abilities::find(spell) else {
            continue;
        };
        // A charge needs a free body and an enemy to run at; the others can be cast at any moment.
        let aim = (ability.effect == Effect::Charge).then(|| {
            if action.owns_body() || charging.0.is_some() {
                return Err(Refusal::Busy);
            }
            let ahead: Vec<(Entity, Vec3)> = foes
                .iter()
                .filter(|(_, _, reaction)| **reaction != Reaction::Dead)
                .map(|(e, at, _)| (e, at.translation()))
                .collect();
            moves::pick_target(xf.translation, *xf.forward(), lock.target, &ahead)
        });
        // A spell is a committed cast: its clips, a free body and (an instant one) a target in range
        // are checked before anything is paid.
        let cast_plan = match wow_world::spells::def(spell) {
            None => None,
            Some(def) => {
                let ahead: Vec<(Entity, Vec3)> = foes
                    .iter()
                    .filter(|(_, _, reaction)| **reaction != Reaction::Dead)
                    .map(|(e, at, _)| (e, at.translation()))
                    .collect();
                let timing = casts.as_deref().and_then(|k| k.timing(spell));
                match crate::spells::plan(
                    def,
                    action.can_cast(),
                    motor.grounded,
                    timing,
                    xf.translation,
                    *xf.forward(),
                    lock.target,
                    &ahead,
                ) {
                    Ok(p) => Some(p),
                    Err(why) => {
                        info!("abilities: {} refused: {}", ability.name, why.message());
                        toasts.push(now, why.message());
                        continue;
                    }
                }
            }
        };
        let signature = matches!(
            ability.effect,
            Effect::MortalStrike | Effect::Bloodthirst | Effect::ShieldWall
        );
        let melee_plan = if signature {
            if talents.signature() != Some(spell) || charging.0.is_some() {
                toasts.push(now, "This talent is not available.");
                continue;
            }
            match crate::warrior::signature_plan(
                spell,
                &action,
                motor.grounded,
                weapon.as_deref(),
                &stamina,
                gear.0.block > 0 && crate::gear_effect::off_hand_held(weapon.as_deref()),
            ) {
                Ok(plan) => plan,
                Err(why) => {
                    toasts.push(now, why.message());
                    continue;
                }
            }
        } else {
            None
        };
        match activate(
            &mut rage,
            &mut mana,
            &mut buffs,
            &mut cooldowns,
            ability,
            aim,
        ) {
            Ok(cast) => {
                info!(
                    "abilities: {} ({} {:.0})",
                    ability.name,
                    ability.power.word(),
                    match ability.power {
                        Power::Rage => rage.cur,
                        Power::Mana => mana.cur,
                    }
                );
                toasts.push(now, ability.name);
                if ability.effect == Effect::Charge {
                    cooldowns.start(
                        spell,
                        12.0 - 0.5
                            * f32::from(talents.rank(wow_world::talents::TalentId::ImprovedCharge)),
                    );
                }
                match cast {
                    Cast::Melee(spell) => {
                        if let Some((kind, spec)) = melee_plan {
                            action.swings = action.swings.wrapping_add(1);
                            let serial = action.swings;
                            action.buffer = None;
                            action.state =
                                crate::combat::ActionState::Attack(crate::combat::Swing {
                                    t: 0.0,
                                    kind,
                                    spec,
                                    reference: true,
                                    serial,
                                });
                            stamina.spend(spec.cost);
                            warrior.signature = Some((serial, spell));
                        }
                    }
                    Cast::Wall => {
                        warrior.wall = 8.0;
                        action.training.damage_taken = 0.6;
                        commands
                            .entity(me)
                            .insert(moves::UpperCast::new(moves::ANIM_ROAR, 0.6));
                        fx::sound(audio.as_deref_mut(), fx::SOUND_SHOUT, "shield wall");
                    }
                    Cast::Shout => {
                        // Standing, the warrior roars with his whole body; on the move his upper
                        // body roars over the legs that keep running, as in WoW.
                        if moves::shouts_on_the_move(motor.horiz_vel) {
                            commands
                                .entity(me)
                                .insert(moves::UpperCast::new(moves::ANIM_ROAR, moves::ROAR_SECS));
                        } else {
                            commands.entity(me).insert(Casting {
                                clip: moves::ANIM_ROAR,
                                left: moves::ROAR_SECS,
                            });
                        }
                        fx::spawn(&mut commands, &fx, fx::Kind::Shout, me);
                        fx::sound(audio.as_deref_mut(), fx::SOUND_SHOUT, "battle shout");
                    }
                    Cast::Charge(target) => {
                        charging.0 = Some(Rush::at(target));
                        fx::spawn(&mut commands, &fx, fx::Kind::ChargeTrail, me);
                        fx::sound(audio.as_deref_mut(), fx::SOUND_CHARGE, "charge");
                    }
                    Cast::Clap => {
                        commands.entity(me).insert(Casting {
                            clip: moves::ANIM_CLAP,
                            left: moves::CLAP_SECS,
                        });
                        claps.write(ThunderClap);
                    }
                    Cast::Spell(spell) => {
                        if let (Some(p), Some(def)) = (cast_plan, wow_world::spells::def(spell)) {
                            action.begin_cast(spell, p.spec, p.target);
                            let fire = def.school == wow_world::spells::School::Fire;
                            // The wind-up's flames and sound are kept: the release (or whatever breaks
                            // the cast) puts them out, they are not the length of their own models.
                            let mut effects = Vec::new();
                            for hand in [0x15, 0x16] {
                                let kind = if fire {
                                    fx::Kind::FirePrecastHand(hand)
                                } else {
                                    fx::Kind::IceHand(hand)
                                };
                                effects.extend(fx::spawn(&mut commands, &fx, kind, me));
                            }
                            let sound = fx::sound_leveled(
                                audio.as_deref_mut(),
                                if fire {
                                    fx::SOUND_FIRE_PRECAST
                                } else {
                                    fx::SOUND_ICE_PRECAST
                                },
                                "spell precast",
                            );
                            commands
                                .entity(me)
                                .insert(crate::spells::CastFx { effects, sound });
                        }
                    }
                }
            }
            Err(why) => {
                info!("abilities: {} refused: {}", ability.name, why.message());
                toasts.push(now, why.message());
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    let start = std::env::var("WOWSOULS_RAGE")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(0.0)
        .clamp(0.0, RAGE_MAX as f32);
    let start_mana = std::env::var("WOWSOULS_MANA")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(0.0)
        .max(0.0);
    app.insert_resource(Rage {
        cur: start,
        ..default()
    })
    .insert_resource(Mana {
        cur: start_mana,
        ..default()
    })
    .init_resource::<ActiveClass>()
    .init_resource::<Buffs>()
    .init_resource::<Hotbar>()
    .init_resource::<Cooldowns>()
    .init_resource::<Charging>()
    .add_message::<ThunderClap>()
    .add_message::<moves::Stun>()
    .add_systems(Startup, load_art)
    .add_systems(
        Update,
        (
            follow_class,
            refit,
            input.after(crate::combat::act),
            upkeep,
            moves::thunder_clap,
            moves::cast_play,
        )
            .chain()
            .run_if(in_state(Phase::Playing)),
    )
    // After the body's own actions, so a swing or a roll that was pressed this frame ends them.
    .add_systems(
        Update,
        (moves::rush, moves::casting, moves::upper_cast)
            .in_set(PlaySet::Intent)
            .after(crate::combat::act)
            .run_if(in_state(Phase::Playing)),
    )
    .add_plugins((ui::plugin, fx::plugin));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn ability(spell: u32) -> &'static Ability {
        abilities::find(spell).unwrap()
    }

    fn foe() -> Entity {
        Entity::from_raw_u32(7).unwrap()
    }

    fn rage_of(cur: f32) -> Rage {
        Rage {
            cur,
            ..Rage::default()
        }
    }

    #[test]
    fn signature_payments_are_once_and_cooldowns_refuse_without_spending() {
        let mut rage = rage_of(100.0);
        let mut mana = Mana::default();
        let mut buffs = Buffs::default();
        let mut cooldowns = Cooldowns::default();
        for (spell, expected, cost, wait) in [
            (12294, Cast::Melee(12294), 30.0, 8.0),
            (23881, Cast::Melee(23881), 30.0, 10.0),
            (871, Cast::Wall, 40.0, 45.0),
        ] {
            let before = rage.cur;
            assert_eq!(
                activate(
                    &mut rage,
                    &mut mana,
                    &mut buffs,
                    &mut cooldowns,
                    ability(spell),
                    None
                ),
                Ok(expected)
            );
            assert_eq!(rage.cur, before - cost);
            assert_eq!(cooldowns.left(spell), wait);
            let after = rage.cur;
            assert_eq!(
                activate(
                    &mut rage,
                    &mut mana,
                    &mut buffs,
                    &mut cooldowns,
                    ability(spell),
                    None
                ),
                Err(Refusal::NotReady(wait))
            );
            assert_eq!(rage.cur, after);
        }
    }

    #[test]
    fn rage_builds_gain_from_blows_caps_and_fades_after_the_fight() {
        let mut r = Rage::default();
        r.gain(20.0, true);
        assert_eq!(r.cur, 10.0, "half of a dealt blow");
        r.gain(40.0, false);
        assert_eq!(r.cur, 20.0, "a quarter of a taken one");
        r.gain(9999.0, true);
        assert_eq!(r.cur, RAGE_MAX as f32);
        // No fading while the fight is on, then 2 a second.
        r.tick(4.0);
        assert_eq!(r.cur, RAGE_MAX as f32);
        r.tick(2.0);
        assert!(r.cur < RAGE_MAX as f32 - 1.0, "{}", r.cur);
        for _ in 0..100 {
            r.tick(1.0);
        }
        assert_eq!(r.cur, 0.0);
        // A blow starts the quiet over.
        r.gain(10.0, true);
        r.tick(3.0);
        assert_eq!(r.cur, 5.0);
    }

    #[test]
    fn what_an_ability_deals_builds_no_rage_but_a_sword_blow_beside_it_does() {
        // A clap that hit two enemies.
        let mut r = Rage {
            ability_damage: 60.0,
            ..Rage::default()
        };
        r.gain_from_blow(30.0);
        r.gain_from_blow(30.0);
        assert_eq!(r.cur, 0.0, "the clap does not pay for itself");
        assert_eq!(r.ability_damage, 0.0);
        r.gain_from_blow(20.0);
        assert_eq!(r.cur, 10.0, "a sword blow builds rage as before");
    }

    #[test]
    fn battle_shout_lasts_two_minutes_and_renews() {
        let (mut rage, mut buffs, mut cd) = (rage_of(60.0), Buffs::default(), Cooldowns::default());
        assert_eq!(buffs.damage_factor(), 1.0);
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                ability(6673),
                None
            ),
            Ok(Cast::Shout)
        );
        assert_eq!((rage.cur, buffs.shout), (40.0, SHOUT_SECS));
        assert!((buffs.damage_factor() - SHOUT_BONUS).abs() < 1e-6);
        // A shout again just renews it.
        buffs.shout = 3.0;
        assert!(activate(
            &mut rage,
            &mut Mana::default(),
            &mut buffs,
            &mut cd,
            ability(6673),
            None
        )
        .is_ok());
        assert_eq!(buffs.shout, SHOUT_SECS);
        assert_eq!(cd.left(6673), 0.0, "the shout has no cooldown");
    }

    #[test]
    fn charge_costs_no_rage_needs_a_target_and_has_a_cooldown() {
        let (mut rage, mut buffs, mut cd) = (rage_of(0.0), Buffs::default(), Cooldowns::default());
        let charge = ability(100);
        // No target: refused, and nothing is spent or started.
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                charge,
                None
            ),
            Err(Refusal::NoTarget)
        );
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                charge,
                Some(Err(Refusal::TooFar))
            ),
            Err(Refusal::TooFar)
        );
        assert_eq!(cd.left(100), 0.0);
        // With a target and no rage at all: it goes (a charge makes rage).
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                charge,
                Some(Ok(foe()))
            ),
            Ok(Cast::Charge(foe()))
        );
        assert_eq!(cd.left(100), 12.0);
        // Not again until it is ready.
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                charge,
                Some(Ok(foe()))
            ),
            Err(Refusal::NotReady(12.0))
        );
        cd.tick(11.5);
        assert_eq!(cd.left(100), 0.5);
        cd.tick(1.0);
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                charge,
                Some(Ok(foe()))
            ),
            Ok(Cast::Charge(foe()))
        );
    }

    #[test]
    fn thunder_clap_costs_twenty_rage_and_five_seconds() {
        let (mut rage, mut buffs, mut cd) = (rage_of(15.0), Buffs::default(), Cooldowns::default());
        let clap = ability(6343);
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                clap,
                None
            ),
            Err(Refusal::NotEnough(wow_world::class::Power::Rage))
        );
        assert_eq!(rage.cur, 15.0, "a refusal changes nothing");
        rage.cur = 50.0;
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                clap,
                None
            ),
            Ok(Cast::Clap)
        );
        assert_eq!((rage.cur, cd.left(6343)), (30.0, 5.0));
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                clap,
                None
            ),
            Err(Refusal::NotReady(5.0)),
            "the cooldown comes before the rage is asked"
        );
        assert_eq!(rage.cur, 30.0);
    }

    #[test]
    fn an_ability_the_game_cannot_do_is_refused_and_costs_nothing() {
        let (mut rage, mut buffs, mut cd) =
            (rage_of(100.0), Buffs::default(), Cooldowns::default());
        let mut broken = *ability(6343);
        broken.usable = false;
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                &broken,
                None
            ),
            Err(Refusal::NotYet)
        );
        assert_eq!((rage.cur, buffs), (100.0, Buffs::default()));
    }

    #[test]
    fn the_cooldowns_run_down_and_are_dropped_at_zero() {
        let mut cd = Cooldowns::default();
        cd.start(100, 2.0);
        cd.start(6343, 5.0);
        cd.tick(2.5);
        assert_eq!((cd.left(100), cd.left(6343)), (0.0, 2.5));
        cd.clear();
        assert_eq!(cd, Cooldowns::default());
    }

    #[test]
    fn the_refusals_speak_plainly() {
        assert_eq!(Refusal::TooFar.message(), "Out of range.");
        assert_eq!(Refusal::TooClose.message(), "Target too close.");
        assert_eq!(Refusal::NotReady(4.2).message(), "Not ready yet (5 s).");
        assert_eq!(
            Refusal::NotEnough(wow_world::class::Power::Rage).message(),
            "Not enough rage."
        );
    }

    #[test]
    fn the_bar_holds_the_known_usable_abilities_in_list_order() {
        let mut learned = BTreeSet::new();
        assert!(
            slots_for(&learned, 1).is_empty(),
            "nothing is known at the start"
        );
        learned.insert(6343);
        learned.insert(100);
        assert_eq!(
            slots_for(&learned, 1),
            vec![100, 6343],
            "Charge comes first"
        );
        learned.insert(6673);
        assert_eq!(slots_for(&learned, 1), vec![100, 6673, 6343]);
    }

    #[test]
    fn the_buffs_run_down_together_and_never_go_negative() {
        let mut b = Buffs {
            shout: 3.0,
            frost_armor: 1800.0,
        };
        b.tick(2.0);
        assert_eq!((b.shout, b.frost_armor), (1.0, 1798.0));
        b.tick(5.0);
        assert_eq!((b.shout, b.frost_armor), (0.0, 1793.0));
        assert_eq!(b.damage_factor(), 1.0, "the shout is over");
    }

    #[test]
    fn the_pool_starts_full_and_a_longer_bar_is_not_a_refill() {
        let mut m = Mana::default();
        m.sync(165.0, 22.0);
        assert_eq!(
            (m.cur, m.max, m.spirit),
            (165.0, 165.0, 22.0),
            "the first sync fills the pool"
        );
        m.spend(60.0);
        assert_eq!(m.cur, 105.0);
        // A level-up lengthens the bar by 20: the pool moves by the same 20.
        m.sync(185.0, 23.0);
        assert_eq!((m.cur, m.max), (125.0, 185.0));
        // Intellect gear comes off: the bar is 85 shorter and so is the pool.
        m.sync(100.0, 22.0);
        assert_eq!((m.cur, m.max), (40.0, 100.0));
        // A full pool stays full whatever the bar does.
        m.cur = 100.0;
        m.sync(60.0, 22.0);
        assert_eq!((m.cur, m.max), (60.0, 60.0));
    }

    #[test]
    fn taking_gear_off_and_putting_it_back_is_neither_a_loss_nor_a_free_refill() {
        let mut m = Mana::default();
        m.sync(165.0, 22.0);
        m.cur = 5.0;
        m.sync(90.0, 22.0);
        assert_eq!(m.cur, 0.0, "never below zero");
        m.sync(165.0, 22.0);
        assert_eq!(m.cur, 5.0, "what the floor took is paid back, not made up");
        assert!(m.cur <= m.max);
    }

    #[test]
    fn a_debug_start_keeps_a_positive_pool_clamped_to_the_maximum() {
        let mut m = Mana {
            cur: 300.0,
            ..Mana::default()
        };
        m.sync(165.0, 22.0);
        assert_eq!(m.cur, 165.0);
        let mut low = Mana {
            cur: 12.0,
            ..Mana::default()
        };
        low.sync(165.0, 22.0);
        assert_eq!(low.cur, 12.0, "WOWSOULS_MANA=12 starts short");
    }

    #[test]
    fn mana_waits_five_seconds_after_a_cast_then_trickles_back_with_spirit() {
        let mut m = Mana::default();
        m.sync(100.0, 20.0);
        m.spend(50.0);
        m.tick(4.9);
        assert_eq!(m.cur, 50.0, "the five-second rule");
        m.tick(0.2);
        // 0.1 s past the rule: 0.07 per Spirit point per second.
        assert!(m.cur > 50.0 && m.cur < 50.5, "{}", m.cur);
        for _ in 0..100 {
            m.tick(1.0);
        }
        assert_eq!(m.cur, 100.0, "it stops at the maximum");
        // A cast starts the wait over; a new life is a full pool.
        m.spend(10.0);
        assert_eq!(m.quiet, 0.0);
        m.refill();
        assert_eq!(m.cur, 100.0);
    }

    #[test]
    fn a_spell_is_paid_in_its_own_power() {
        let (mut rage, mut mana) = (rage_of(100.0), Mana::default());
        mana.sync(100.0, 20.0);
        let (mut buffs, mut cd) = (Buffs::default(), Cooldowns::default());
        let mut usable = *ability(116); // Frostbolt, 25 mana
        usable.usable = true;
        assert_eq!(
            activate(&mut rage, &mut mana, &mut buffs, &mut cd, &usable, None),
            Ok(Cast::Spell(116))
        );
        assert_eq!(
            (rage.cur, mana.cur),
            (100.0, 75.0),
            "mana paid, rage untouched"
        );
        mana.cur = 20.0;
        assert_eq!(
            activate(&mut rage, &mut mana, &mut buffs, &mut cd, &usable, None),
            Err(Refusal::NotEnough(wow_world::class::Power::Mana))
        );
        assert_eq!(mana.cur, 20.0, "a refusal spends nothing");
        assert_eq!(
            Refusal::NotEnough(wow_world::class::Power::Mana).message(),
            "Not enough mana."
        );
    }

    #[test]
    fn a_fight_builds_rage_only_for_a_class_that_has_it() {
        assert!(builds_rage(wow_world::class::Power::Rage));
        assert!(!builds_rage(wow_world::class::Power::Mana));
    }

    #[test]
    fn the_mage_bar_lists_what_is_known_in_the_classes_order_and_a_warrior_never_has_it() {
        let mut learned = BTreeSet::new();
        assert_eq!(
            slots_for(&learned, 8),
            vec![133, 168],
            "Fireball and Frost Armor are known from the start"
        );
        learned.insert(116);
        assert_eq!(slots_for(&learned, 8), vec![133, 168, 116], "Frostbolt");
        learned.insert(2136);
        assert_eq!(
            slots_for(&learned, 8),
            vec![133, 168, 116, 2136],
            "Fire Blast fills the fourth slot"
        );
        assert!(
            slots_for(&learned, 1).is_empty(),
            "and a warrior never lists them"
        );
    }

    #[test]
    fn a_mage_ability_is_refused_as_not_yet_and_costs_nothing() {
        let (mut rage, mut buffs, mut cd) =
            (rage_of(100.0), Buffs::default(), Cooldowns::default());
        // A mage ability the game cannot do (switched off here) is refused and costs nothing.
        let mut broken = *ability(168);
        broken.usable = false;
        assert_eq!(
            activate(
                &mut rage,
                &mut Mana::default(),
                &mut buffs,
                &mut cd,
                &broken,
                None
            ),
            Err(Refusal::NotYet)
        );
        assert_eq!((rage.cur, buffs), (100.0, Buffs::default()));
    }

    #[test]
    fn the_card_names_mana_for_a_mage_spell() {
        let c = card(116).unwrap();
        assert!(c.tooltip.starts_with("Rank 1   25 Mana\n"), "{}", c.tooltip);
        assert_eq!(c.versus, "Requires level 4");
        assert_eq!(
            card(133).unwrap().versus,
            "",
            "Fireball is known from the start"
        );
    }

    #[test]
    fn the_card_names_the_rank_the_rage_and_the_level() {
        let c = card(6673).unwrap();
        assert_eq!(c.name, "Battle Shout");
        assert!(c.tooltip.starts_with("Rank 1   20 Rage\n"), "{}", c.tooltip);
        assert_eq!(c.versus, "Requires level 1");
        assert_eq!(c.warning, None);
        let charge = card(100).unwrap();
        assert!(
            charge.tooltip.starts_with("Rank 1   No rage\n"),
            "{}",
            charge.tooltip
        );
        assert_eq!(charge.versus, "Requires level 1", "Charge is learned first");
        assert_eq!(card(6343).unwrap().versus, "Requires level 4");
        assert!(card(1).is_none());
        assert!(card(78).is_none(), "no Heroic Strike");
    }

    #[test]
    fn the_script_names_slots_and_the_cycle() {
        let s = parse_script("3:1, 4:cycle,5:use,6:0,x:2,7:9");
        let kinds: Vec<_> = s.iter().map(|x| x.1).collect();
        assert_eq!(
            kinds,
            vec![
                Press::Slot(0),
                Press::Cycle,
                Press::UseSelected,
                Press::Slot(8)
            ]
        );
    }

    #[test]
    fn every_ability_has_an_icon_in_the_install() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        for a in abilities::ALL {
            let path = format!("Interface\\Icons\\{}.blp", a.icon);
            let bytes = chain
                .read_file(&path)
                .unwrap_or_else(|e| panic!("{path}: {e}"));
            assert!(bytes.starts_with(b"BLP"), "{path}");
        }
    }
}
