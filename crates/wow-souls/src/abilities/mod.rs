//! Rage and the warrior's abilities in play ([`wow_world::abilities`]: what exists, what the trainer
//! asks). A warrior has no mana: **rage** builds from the blows he deals and takes and fades away when
//! the fighting stops, and an ability spends it. The abilities known (what the trainer taught: nothing is
//! known at the start) sit on a four-slot bar; the keys 1 to 4 use a slot, Z / the left-stick click cycles
//! the selected one and the D-pad down uses it (the D-pad left is the ranged stance).
//!
//! The set is the user's: **Charge** (a rush at an enemy that stuns it and gives rage), **Battle Shout**
//! (blows do 15 % more damage for two minutes) and **Thunder Clap** (the enemies around take damage and are
//! slowed). What each does in the world is in [`moves`]; how it looks and sounds is in [`fx`] (WoW's own
//! spell models and sounds); the bar and the rage bar are in [`ui`].
//!
//! The damage bonus is read by the strike ([`Buffs::damage_factor`] in `hits.rs`); the rest is here.
//!
//! Switches: `WOWSOULS_RAGE=<n>` starts with that much rage; `WOWSOULS_ABILITY_SCRIPT=<s>:<1-4|cycle|use>,..`
//! presses the ability keys of an unattended run.

use std::collections::BTreeMap;

use bevy::prelude::*;

use wow_world::abilities::{self, Ability, Effect, RAGE_MAX};

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
use crate::quests::Toasts;
use crate::settings::binds::Action as BindAction;
use crate::settings::Controls;
use crate::settle::Phase;

pub mod fx;
pub mod moves;
pub mod ui;

pub use moves::{apply_slow, stun};
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

/// The effects that are running.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct Buffs {
    /// Seconds of Battle Shout left.
    pub shout: f32,
}

impl Buffs {
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

/// The slots for what is learned: the known and usable abilities in list order, four at most.
pub fn slots_for(learned: &std::collections::BTreeSet<u32>) -> Vec<u32> {
    abilities::known(learned)
        .into_iter()
        .filter(|a| a.usable)
        .map(|a| a.spell)
        .take(4)
        .collect()
}

/// Why an ability could not be used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refusal {
    NotEnoughRage(u32),
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
            Refusal::NotEnoughRage(_) => "Not enough rage.".into(),
            Refusal::NotReady(s) => format!("Not ready yet ({:.0} s).", s.ceil()),
            Refusal::NotYet => "You cannot do that yet.".into(),
            Refusal::NoTarget => "You have no target to charge.".into(),
            Refusal::TooClose => "Target too close.".into(),
            Refusal::TooFar => "Out of range.".into(),
            Refusal::Busy => "You cannot do that now.".into(),
        }
    }
}

/// What a used ability asks of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cast {
    Shout,
    Charge(Entity),
    Clap,
}

/// Uses an ability: checks it (usable, ready, rage, for a charge its target `aim`), then spends the rage,
/// starts the cooldown and the buff, and says what the world is to do. Nothing changes on a refusal.
pub fn activate(
    rage: &mut Rage,
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
    if rage.cur < ability.rage as f32 {
        return Err(Refusal::NotEnoughRage(ability.rage));
    }
    let cast = match ability.effect {
        Effect::BattleShout => Cast::Shout,
        Effect::Charge => Cast::Charge(aim.unwrap_or(Err(Refusal::NoTarget))?),
        Effect::ThunderClap => Cast::Clap,
    };
    rage.cur -= ability.rage as f32;
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
    let cost = if a.rage > 0 {
        format!("{} Rage", a.rage)
    } else {
        "No rage".to_string()
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

/// Rage from the blows of the fight, the fading, the timers, and a death that ends it all.
#[allow(clippy::too_many_arguments)]
fn upkeep(
    time: Res<Time>,
    mut rage: ResMut<Rage>,
    mut buffs: ResMut<Buffs>,
    mut cooldowns: ResMut<Cooldowns>,
    mut hits: MessageReader<Hit>,
    mut hurts: MessageReader<PlayerHurt>,
    mut respawned: MessageReader<Respawned>,
) {
    for h in hits.read() {
        rage.gain_from_blow(h.damage);
    }
    for h in hurts.read() {
        // A death blow's thousands are not a fight.
        rage.gain(h.damage.min(100.0), false);
    }
    let dt = time.delta_secs();
    rage.tick(dt);
    cooldowns.tick(dt);
    buffs.shout = (buffs.shout - dt).max(0.0);
    if respawned.read().count() > 0 {
        *rage = Rage::default();
        *buffs = Buffs::default();
        cooldowns.clear();
    }
}

/// The bar follows what is learned.
fn refit(owned: Res<Owned>, mut bar: ResMut<Hotbar>) {
    let slots = slots_for(&owned.progress.abilities);
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
    (mut bar, mut rage, mut buffs, mut cooldowns, mut charging): (
        ResMut<Hotbar>,
        ResMut<Rage>,
        ResMut<Buffs>,
        ResMut<Cooldowns>,
        ResMut<Charging>,
    ),
    (mut toasts, mut claps, audio): (
        ResMut<Toasts>,
        MessageWriter<ThunderClap>,
        Option<ResMut<Audio>>,
    ),
    avatar: Query<(Entity, &Transform, &Action, &Health), With<Avatar>>,
    foes: Query<(Entity, &GlobalTransform, &Reaction), (With<Hostile>, Without<Avatar>)>,
    mut script: Local<Option<Vec<(f32, Press)>>>,
    mut next_step: Local<usize>,
) {
    let Ok((me, xf, action, health)) = avatar.single() else {
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
        match activate(&mut rage, &mut buffs, &mut cooldowns, ability, aim) {
            Ok(cast) => {
                info!("abilities: {} (rage {:.0})", ability.name, rage.cur);
                toasts.push(now, ability.name);
                match cast {
                    Cast::Shout => {
                        commands.entity(me).insert(Casting {
                            clip: moves::ANIM_ROAR,
                            left: moves::ROAR_SECS,
                        });
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
    app.insert_resource(Rage {
        cur: start,
        ..default()
    })
    .init_resource::<Buffs>()
    .init_resource::<Hotbar>()
    .init_resource::<Cooldowns>()
    .init_resource::<Charging>()
    .add_message::<ThunderClap>()
    .add_message::<moves::Stun>()
    .add_systems(Startup, load_art)
    .add_systems(
        Update,
        (refit, input, upkeep, moves::thunder_clap, moves::cast_play)
            .chain()
            .run_if(in_state(Phase::Playing)),
    )
    // After the body's own actions, so a swing or a roll that was pressed this frame ends them.
    .add_systems(
        Update,
        (moves::rush, moves::casting)
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
            activate(&mut rage, &mut buffs, &mut cd, ability(6673), None),
            Ok(Cast::Shout)
        );
        assert_eq!((rage.cur, buffs.shout), (40.0, SHOUT_SECS));
        assert!((buffs.damage_factor() - SHOUT_BONUS).abs() < 1e-6);
        // A shout again just renews it.
        buffs.shout = 3.0;
        assert!(activate(&mut rage, &mut buffs, &mut cd, ability(6673), None).is_ok());
        assert_eq!(buffs.shout, SHOUT_SECS);
        assert_eq!(cd.left(6673), 0.0, "the shout has no cooldown");
    }

    #[test]
    fn charge_costs_no_rage_needs_a_target_and_has_a_cooldown() {
        let (mut rage, mut buffs, mut cd) = (rage_of(0.0), Buffs::default(), Cooldowns::default());
        let charge = ability(100);
        // No target: refused, and nothing is spent or started.
        assert_eq!(
            activate(&mut rage, &mut buffs, &mut cd, charge, None),
            Err(Refusal::NoTarget)
        );
        assert_eq!(
            activate(
                &mut rage,
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
            activate(&mut rage, &mut buffs, &mut cd, charge, Some(Ok(foe()))),
            Ok(Cast::Charge(foe()))
        );
        assert_eq!(cd.left(100), 12.0);
        // Not again until it is ready.
        assert_eq!(
            activate(&mut rage, &mut buffs, &mut cd, charge, Some(Ok(foe()))),
            Err(Refusal::NotReady(12.0))
        );
        cd.tick(11.5);
        assert_eq!(cd.left(100), 0.5);
        cd.tick(1.0);
        assert_eq!(
            activate(&mut rage, &mut buffs, &mut cd, charge, Some(Ok(foe()))),
            Ok(Cast::Charge(foe()))
        );
    }

    #[test]
    fn thunder_clap_costs_twenty_rage_and_five_seconds() {
        let (mut rage, mut buffs, mut cd) = (rage_of(15.0), Buffs::default(), Cooldowns::default());
        let clap = ability(6343);
        assert_eq!(
            activate(&mut rage, &mut buffs, &mut cd, clap, None),
            Err(Refusal::NotEnoughRage(20))
        );
        assert_eq!(rage.cur, 15.0, "a refusal changes nothing");
        rage.cur = 50.0;
        assert_eq!(
            activate(&mut rage, &mut buffs, &mut cd, clap, None),
            Ok(Cast::Clap)
        );
        assert_eq!((rage.cur, cd.left(6343)), (30.0, 5.0));
        assert_eq!(
            activate(&mut rage, &mut buffs, &mut cd, clap, None),
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
            activate(&mut rage, &mut buffs, &mut cd, &broken, None),
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
        assert_eq!(Refusal::NotEnoughRage(20).message(), "Not enough rage.");
    }

    #[test]
    fn the_bar_holds_the_known_usable_abilities_in_list_order() {
        let mut learned = BTreeSet::new();
        assert!(
            slots_for(&learned).is_empty(),
            "nothing is known at the start"
        );
        learned.insert(6343);
        learned.insert(100);
        assert_eq!(slots_for(&learned), vec![100, 6343], "Charge comes first");
        learned.insert(6673);
        assert_eq!(slots_for(&learned), vec![100, 6673, 6343]);
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
