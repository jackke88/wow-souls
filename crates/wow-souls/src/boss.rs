//! Hogger's second phase and Pierce Armor. Pure rules first (tested), then the systems.

use bevy::prelude::*;

use crate::species::SpeciesDef;

/// A boss's phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    One,
    Two,
}

/// The creature's phase (a component on every creature; only a boss with a second one changes).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct BossPhase(pub Phase);

/// The share of health at which the second phase begins.
pub const PHASE_TWO_AT: f32 = 0.5;
/// A boss back at (nearly) full health is reset: the fight starts again.
const RESET_AT: f32 = 0.99;

/// The phase for `health_fraction` (0..=1), given the current one: Two is entered at half health
/// and kept while the creature heals a little; only a reset (full health) leaves it.
pub fn phase_for(current: Phase, health_fraction: f32) -> Phase {
    if health_fraction >= RESET_AT {
        Phase::One
    } else if health_fraction <= PHASE_TWO_AT {
        Phase::Two
    } else {
        current
    }
}

/// What a phase changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mods {
    pub tempo: f32,
    pub chase: f32,
    pub cooldown: f32,
}

impl Phase {
    pub fn mods(self, species: &SpeciesDef) -> Mods {
        match (self, species.phase_two.as_ref()) {
            (Phase::Two, Some(p)) => Mods {
                tempo: species.tempo * p.tempo,
                chase: p.chase,
                cooldown: p.cooldown,
            },
            _ => Mods {
                tempo: species.tempo,
                chase: 1.0,
                cooldown: 1.0,
            },
        }
    }

    /// The pattern table in force.
    pub fn table(self, species: &SpeciesDef) -> &'static [crate::pattern::Row] {
        match (self, species.phase_two.as_ref()) {
            (Phase::Two, Some(p)) => p.table,
            _ => species.table,
        }
    }
}

/// The clock multiplier of a creature's swing and kit clip (1 outside a boss's second phase).
#[derive(Component, Debug, Clone, Copy)]
pub struct Tempo(pub f32);

/// How many stacks Pierce Armor holds at most, how long they last (s), and what each costs the
/// player (a share more damage taken: Hogger's spell takes armour, the game models it as damage).
const MAX_STACKS: u8 = 3;
const SHRED_SECS: f32 = 30.0;
const SHRED_PER_STACK: f32 = 0.08;

/// Pierce Armor on the player.
#[derive(Resource, Debug, Default)]
pub struct ArmorShred {
    stacks: u8,
    left: f32,
}

impl ArmorShred {
    pub fn add(&mut self) {
        self.stacks = (self.stacks + 1).min(MAX_STACKS);
        self.left = SHRED_SECS;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.stacks > 0 {
            self.left -= dt;
            if self.left <= 0.0 {
                self.clear();
            }
        }
    }

    pub fn clear(&mut self) {
        *self = ArmorShred::default();
    }

    /// What the damage taken is multiplied by.
    pub fn factor(&self) -> f32 {
        1.0 + SHRED_PER_STACK * f32::from(self.stacks)
    }

    pub fn stacks(&self) -> u8 {
        self.stacks
    }
}

/// Species whose blows pierce armour.
pub fn pierces(species: &str) -> bool {
    species == "hogger"
}

/// The least share of health a run may start a boss at (`WOWSOULS_BOSS_HP`).
const MIN_START: f32 = 0.05;

/// `WOWSOULS_BOSS_HP=<share>`: the boss starts with that share of its health (to see its second
/// phase without a long fight); `None`, junk or a share over 1 is a full-health boss. An
/// unattended-run instrument.
pub fn start_fraction(var: Option<&str>) -> f32 {
    var.and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|f| f.is_finite() && *f <= 1.0)
        .map_or(1.0, |f| f.max(MIN_START))
}

/// Hogger's theme, WoW's own: the Naxxramas boss tracks of the 1.12 client (the Abomination Wing's
/// Patchwerk fight, then Kel'Thuzad's), the heaviest orchestral music the game ships. The fight plays
/// them round, starting with the first; `WOWSOULS_BOSS_MUSIC=<path in the client's data>` names a
/// single track instead.
const HOGGER_THEME: &[&str] = &[
    "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasAbominationBoss1.mp3",
    "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasKelThuzad1.mp3",
    "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasAbominationBoss2.mp3",
    "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasKelThuzad2.mp3",
    "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasSpiderBoss1.mp3",
];

/// The tracks a boss's fight plays; empty for a species with no theme of its own (the zone's music
/// plays on).
pub fn music_playlist(species: &str) -> &'static [&'static str] {
    match species {
        "hogger" => HOGGER_THEME,
        _ => &[],
    }
}

/// The boss that is fighting the player now (its species name): the music's cue. `None` when no boss
/// is engaged (it chases, swings or catches its breath) with health left.
#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub struct Engaged(pub Option<&'static str>);

fn engagement(
    mut engaged: ResMut<Engaged>,
    bosses: Query<
        (
            &crate::species::Species,
            &crate::npc::Brain,
            &crate::combat::Health,
        ),
        With<crate::species::Boss>,
    >,
) {
    let now = bosses
        .iter()
        .find(|(s, b, h)| b.mind.engaged() && h.cur > 0.0 && !music_playlist(s.0.name).is_empty())
        .map(|(s, ..)| s.0.name);
    if engaged.0 != now {
        info!("boss: engaged {:?} -> {:?}", engaged.0, now);
        engaged.0 = now;
    }
}

/// A boss has entered its second phase: its roar and the camera's shake.
#[derive(Message, Debug, Clone, Copy)]
pub struct Roar {
    pub from: Entity,
}

/// What the camera is kicked by a boss's slam or roar at `dist` yd (0 at and past [`SLAM_RANGE`]).
pub const SLAM_RANGE: f32 = 16.0;
const SLAM_KICK: f32 = 0.5;

pub fn slam_kick(dist: f32) -> f32 {
    SLAM_KICK * (1.0 - dist / SLAM_RANGE).clamp(0.0, 1.0)
}

/// The window of `hits` (s into the clip) whose start the clock passed in `(prev, now]`, if any: the
/// instant a blow lands, once.
pub fn window_opened(hits: &[(f32, f32)], prev: f32, now: f32) -> Option<usize> {
    hits.iter().position(|h| h.0 > prev && h.0 <= now)
}

/// Each boss's phase follows its health; a phase change is logged once and roared.
pub(crate) fn phases(
    mut bosses: Query<
        (
            Entity,
            &crate::species::Species,
            &crate::combat::Health,
            &mut BossPhase,
            &mut Tempo,
            Option<&Frenzy>,
        ),
        With<crate::species::Boss>,
    >,
    mut roars: MessageWriter<Roar>,
) {
    for (e, species, hp, mut phase, mut tempo, frenzy) in &mut bosses {
        let next = phase_for(phase.0, hp.cur / hp.max.max(1.0));
        if next != phase.0 {
            info!("boss: {} is in phase {next:?}", species.0.name);
            // A boss with a Frenzy roars as it rises into it ([`frenzy`]), not at the threshold.
            if next == Phase::Two && frenzy.is_none() {
                roars.write(Roar { from: e });
            }
            phase.0 = next;
        }
        tempo.0 = phase.0.mods(species.0).tempo;
    }
}

/// The camera shakes where a boss's blow lands (the ground slam reads even when it misses) and at its
/// roar.
#[allow(clippy::type_complexity)]
fn slam_shake(
    roster: Option<Res<crate::monster::Roster>>,
    mut shake: ResMut<crate::shake::Shake>,
    mut roars: MessageReader<Roar>,
    avatar: Query<&Transform, (With<crate::avatar::Avatar>, Without<crate::species::Boss>)>,
    bosses: Query<
        (
            Entity,
            &crate::species::Species,
            &crate::npc::Brain,
            &GlobalTransform,
        ),
        With<crate::species::Boss>,
    >,
    mut last: Local<std::collections::HashMap<Entity, (u8, f32)>>,
) {
    let Ok(player) = avatar.single() else {
        roars.clear();
        return;
    };
    let dist_to =
        |at: Vec3| Vec2::new(player.translation.x - at.x, player.translation.z - at.z).length();
    for r in roars.read() {
        if let Ok((_, _, _, at)) = bosses.get(r.from) {
            shake.kick(slam_kick(dist_to(at.translation())) * 1.4);
        }
    }
    let Some(roster) = roster else {
        return;
    };
    for (e, species, brain, at) in &bosses {
        let crate::npc::Mind::Attack { t, mv } = brain.mind else {
            last.remove(&e);
            continue;
        };
        let Some(a) = roster
            .get(species.kit())
            .and_then(|k| k.attacks.get(usize::from(mv)))
        else {
            continue;
        };
        // A move that has just begun counts from before its first instant.
        let prev = match last.get(&e) {
            Some(&(m, p)) if m == mv => p,
            _ => -1.0,
        };
        if window_opened(&a.spec.hits, prev, t).is_some() {
            shake.kick(slam_kick(dist_to(at.translation())));
        }
        last.insert(e, (mv, t));
    }
}

/// The name of the kit move a boss powers up with at its second phase (the knight's Abyss power-up).
pub const FRENZY_MOVE: &str = "frenzy";
/// How long the rage burns on the boss once the power-up begins (s): through the power-up (8 s) and a little past.
const FRENZY_GLOW_SECS: f32 = 10.0;

/// Where a boss's Frenzy is (spec 2026-10-16 §5.3): it waits for phase two and for the swing in hand to end, asks
/// for its power-up until the power-up plays, rises through it (no flinch, full damage taken) and is done for the
/// fight; a reset (phase one again) arms it anew.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrenzyState {
    #[default]
    Waiting,
    Asked,
    Rising,
    Done,
}

impl FrenzyState {
    /// It does not flinch: the power-up is not to be broken (the player punishes it with damage instead).
    pub fn armored(self) -> bool {
        self == FrenzyState::Rising
    }
}

/// What the boss is swinging this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Swing {
    None,
    Frenzy,
    Other,
}

/// What a Frenzy step asks of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrenzyEvent {
    /// Start the power-up now (again, if the last ask was lost).
    Ask,
    /// The fight was reset before the power-up ended: take back a start still waiting in the creature.s mind.
    Withdraw,
    /// The power-up is playing: the roar.
    Start,
    End,
}

/// A boss's Frenzy, on a boss whose kit has the power-up.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Frenzy(pub FrenzyState);

/// One step of the Frenzy.
pub fn frenzy_step(
    state: FrenzyState,
    phase_two: bool,
    swing: Swing,
) -> (FrenzyState, Option<FrenzyEvent>) {
    use FrenzyState::*;
    if !phase_two {
        let withdraw = matches!(state, Asked | Rising).then_some(FrenzyEvent::Withdraw);
        return (Waiting, withdraw);
    }
    match (state, swing) {
        (Waiting | Asked, Swing::None) => (Asked, Some(FrenzyEvent::Ask)),
        (Asked, Swing::Frenzy) => (Rising, Some(FrenzyEvent::Start)),
        (Rising, Swing::Frenzy) => (Rising, None),
        (Rising, _) => (Done, Some(FrenzyEvent::End)),
        (s, _) => (s, None),
    }
}

/// Runs each boss's Frenzy: the power-up is started through the creature's own think (`Brain.start`, the
/// footwork told it has made up its mind), the roar and the shake come as it plays.
#[allow(clippy::type_complexity)]
fn frenzy(
    mut commands: Commands,
    roster: Option<Res<crate::monster::Roster>>,
    fx: Option<Res<crate::abilities::fx::Fx>>,
    mut bosses: Query<
        (
            Entity,
            &crate::species::Species,
            &BossPhase,
            &mut crate::npc::Brain,
            Option<&mut Frenzy>,
        ),
        With<crate::species::Boss>,
    >,
    mut roars: MessageWriter<Roar>,
) {
    let Some(roster) = roster else {
        return;
    };
    for (e, species, phase, mut brain, frenzy) in &mut bosses {
        let Some(index) = roster
            .get(species.kit())
            .and_then(|k| k.attack_index(FRENZY_MOVE))
            .and_then(|i| u8::try_from(i).ok())
        else {
            continue;
        };
        let Some(mut frenzy) = frenzy else {
            commands.entity(e).insert(Frenzy::default());
            continue;
        };
        let swing = match brain.mind {
            crate::npc::Mind::Attack { mv, .. } if mv == index => Swing::Frenzy,
            crate::npc::Mind::Attack { .. } => Swing::Other,
            _ => Swing::None,
        };
        let (next, event) = frenzy_step(frenzy.0, phase.0 == Phase::Two, swing);
        match event {
            Some(FrenzyEvent::Ask) => {
                brain.start = Some(index);
                brain.next = None;
                brain.foot.committed = true;
            }
            Some(FrenzyEvent::Start) => {
                info!("boss: {} rises into its Frenzy", species.0.name);
                roars.write(Roar { from: e });
                if let Some(fx) = fx.as_deref() {
                    crate::abilities::fx::spawn_for(
                        &mut commands,
                        fx,
                        crate::abilities::fx::Kind::Enrage,
                        e,
                        FRENZY_GLOW_SECS,
                    );
                }
            }
            Some(FrenzyEvent::End) => info!("boss: {}'s Frenzy is up", species.0.name),
            Some(FrenzyEvent::Withdraw) => {
                if brain.start == Some(index) {
                    brain.start = None;
                }
                info!("boss: {}'s Frenzy is withdrawn (a reset)", species.0.name);
            }
            None => {}
        }
        frenzy.0 = next;
    }
}

/// The radius (yd) Thunder Clap's ground burst (`ClapBase`) is drawn at, unscaled: a shock's burst is scaled to
/// its own radius against this.
const CLAP_RADIUS: f32 = 4.0;
/// How long a shock's burst and its holder stay (s).
const BURST_SECS: f32 = 1.6;

/// A shock or blast hit the ground: the camera kicks (a blast harder), and Thunder Clap's ground burst marks
/// where and how wide, hit or miss.
fn on_landed(
    mut commands: Commands,
    fx: Option<Res<crate::abilities::fx::Fx>>,
    mut shake: ResMut<crate::shake::Shake>,
    mut landed: MessageReader<crate::shock::Landed>,
    avatar: Query<&Transform, With<crate::avatar::Avatar>>,
) {
    let player = avatar.single().ok().map(|t| t.translation);
    for l in landed.read() {
        let dist = player.map_or(0.0, |p| Vec2::new(p.x - l.at.x, p.z - l.at.z).length());
        shake.kick(slam_kick(dist) * if l.blast { 1.8 } else { 1.3 });
        let Some(fx) = fx.as_deref() else { continue };
        let holder = commands
            .spawn((
                Name::new(if l.blast { "Boss blast" } else { "Boss shock" }),
                Transform::from_translation(l.at).with_scale(Vec3::splat(l.radius / CLAP_RADIUS)),
                Visibility::default(),
                crate::abilities::fx::Expires(BURST_SECS),
            ))
            .id();
        crate::abilities::fx::spawn_for(
            &mut commands,
            fx,
            crate::abilities::fx::Kind::ClapBase,
            holder,
            BURST_SECS,
        );
        debug!(
            "boss: {} burst at {:?} from {:?}",
            if l.blast { "blast" } else { "shock" },
            l.at,
            l.from
        );
    }
}

/// "Yipe! Help Hogger!" (spec 2026-10-16 §5.4): the first time a fight takes the boss to a quarter of its health,
/// two of its pack run in from the far side, once; a reset (full health again) arms the call for the next fight.
pub const PACK_AT: f32 = 0.25;
/// How far from the boss the pack appears (yd), opposite the player.
pub const PACK_DISTANCE: f32 = 15.0;
const PACK_SIZE: usize = 2;
/// How far apart the pack appears (yd).
const PACK_SPREAD: f32 = 1.5;
/// How long the pack stays once out of the fight (s): the Dockmaster's guards' 30.
const PACK_LINGER: f32 = 30.0;

/// The boss has called its pack this fight.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct PackCall(pub bool);

/// Who a boss calls, by its kit: Hogger the knight his Riverpaw Outrunners (the Taurus Hogger of
/// `WOWSOULS_HOGGER=taurus` calls nobody, as before).
pub fn pack_of(kit: &str) -> Option<&'static crate::species::SpeciesDef> {
    (kit == "hogger_knight").then_some(&crate::species::OUTRUNNER)
}

/// Whether the pack is called now.
pub fn pack_call_due(called: bool, health_fraction: f32) -> bool {
    !called && health_fraction <= PACK_AT
}

/// Whether the call still counts as made: until the boss is back at full health.
pub fn pack_called_after(called: bool, health_fraction: f32) -> bool {
    called && health_fraction < RESET_AT
}

/// Where the pack appears (ground x/z): `dist` from the boss, on the far side from the player.
pub fn pack_spot(boss: Vec2, player: Vec2, dist: f32) -> Vec2 {
    boss + (boss - player).normalize_or(Vec2::X) * dist
}

#[allow(clippy::type_complexity)]
fn pack_call(
    mut commands: Commands,
    roster: Option<Res<crate::monster::Roster>>,
    catalogs: Option<Res<crate::actor::Catalogs>>,
    asset_server: Res<AssetServer>,
    collide: benilla_world::collision::WorldCollision,
    mut epoch: ResMut<benilla_world::collision::ColliderEpoch>,
    mut audio: Option<ResMut<crate::audio::Audio>>,
    avatar: Query<&Transform, With<crate::avatar::Avatar>>,
    mut bosses: Query<
        (
            Entity,
            &crate::species::Species,
            &crate::combat::Health,
            &Transform,
            &crate::npc::Hostile,
            Option<&mut PackCall>,
        ),
        (With<crate::species::Boss>, Without<crate::avatar::Avatar>),
    >,
) {
    let (Some(catalogs), Ok(player)) = (catalogs, avatar.single()) else {
        return;
    };
    for (e, species, hp, xf, hostile, call) in &mut bosses {
        let Some(def) = pack_of(species.kit()) else {
            continue;
        };
        let Some(mut call) = call else {
            commands.entity(e).insert(PackCall::default());
            continue;
        };
        let frac = hp.cur / hp.max.max(1.0);
        if hp.cur <= 0.0 {
            continue;
        }
        // Only in a fight: a boss left alone at low health calls nobody.
        if !hostile.alert || !pack_call_due(call.0, frac) {
            call.0 = pack_called_after(call.0, frac);
            continue;
        }
        call.0 = true;
        info!(
            "boss: {} yelps \"Yipe! Help {}!\"",
            species.0.name, species.0.title
        );
        if let Some(a) = audio.as_deref_mut() {
            a.play_path(PACK_YELP, 1.4, "boss yelp");
        }
        let flat = |v: Vec3| Vec2::new(v.x, v.z);
        let spot = pack_spot(
            flat(xf.translation),
            flat(player.translation),
            PACK_DISTANCE,
        );
        let side = (spot - flat(xf.translation)).perp().normalize_or(Vec2::Y);
        for k in 0..PACK_SIZE {
            let offset = side * PACK_SPREAD * (k as f32 - (PACK_SIZE as f32 - 1.0) * 0.5);
            let at = spot + offset;
            let wow = benilla_assets::coords::bevy_to_wow(Vec3::new(at.x, xf.translation.y, at.y));
            if let Some(add) = crate::named::call(
                &mut commands,
                def,
                [wow[0], wow[1], wow[2] + 1.0],
                None,
                e.to_bits() ^ (k as u64 + 7),
                PACK_LINGER,
                (
                    roster.as_deref(),
                    &catalogs,
                    &asset_server,
                    &collide,
                    &mut epoch,
                ),
            ) {
                commands.entity(add).insert(PackMember);
                info!("boss: a {} answers {}'s call", def.title, species.0.title);
            }
        }
    }
}

/// Whether a stun takes: not while the boss rises into its Frenzy (a Charge must not cancel the power-up).
pub fn stuns(frenzy: Option<FrenzyState>) -> bool {
    !frenzy.is_some_and(FrenzyState::armored)
}

/// One of a boss's pack called into the fight ([`pack_call`]): it goes when the player dies (spec §11.4).
#[derive(Component, Debug, Clone, Copy)]
pub struct PackMember;

/// The player died: the pack that came to the boss's call goes at once.
fn pack_leaves(
    mut commands: Commands,
    mut respawned: MessageReader<crate::hurt::Respawned>,
    pack: Query<(Entity, Option<&crate::npc::BodyCollider>), With<PackMember>>,
) {
    if respawned.read().count() == 0 {
        return;
    }
    for (e, body) in &pack {
        if let Some(b) = body {
            commands.entity(b.0).despawn();
        }
        commands.entity(e).despawn();
    }
}

/// The yelp of a gnoll in trouble.
const PACK_YELP: &str = "Sound\\Creature\\Gnoll\\mGnollAggro3.wav";

/// Pierce Armor wears off, and ends with the player's death.
fn shred_clock(
    time: Res<Time>,
    mut shred: ResMut<ArmorShred>,
    mut respawned: MessageReader<crate::hurt::Respawned>,
) {
    shred.tick(time.delta_secs());
    if respawned.read().count() > 0 {
        shred.clear();
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ArmorShred>()
        .init_resource::<Engaged>()
        .add_message::<Roar>()
        .add_systems(
            Update,
            (
                shred_clock,
                engagement,
                pack_call,
                pack_leaves,
                slam_shake,
                on_landed,
            )
                .after(crate::controller::PlaySet::Motor)
                .run_if(in_state(crate::settle::Phase::Playing)),
        )
        // After the phase is read (a reset puts it back to one before the Frenzy looks at it) and before the
        // creature thinks with the start it asks for.
        .add_systems(
            Update,
            frenzy
                .after(phases)
                .before(crate::npc::think)
                .run_if(in_state(crate::settle::Phase::Playing)),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_knight_calls_the_pack() {
        assert!(pack_of("hogger_knight").is_some());
        // `WOWSOULS_HOGGER=taurus` brings back the old fight, which had no pack.
        assert!(pack_of("hogger").is_none());
        assert!(pack_of("gnoll").is_none());
    }

    #[test]
    fn a_reset_withdraws_a_frenzy_that_was_asked_for() {
        use FrenzyState::*;
        // The player died while it waited to start: the next fight must not open with it.
        assert_eq!(
            frenzy_step(Asked, false, Swing::None),
            (Waiting, Some(FrenzyEvent::Withdraw))
        );
        assert_eq!(
            frenzy_step(Rising, false, Swing::Frenzy),
            (Waiting, Some(FrenzyEvent::Withdraw))
        );
        assert_eq!(frenzy_step(Waiting, false, Swing::None), (Waiting, None));
    }

    #[test]
    fn a_rising_frenzy_shrugs_off_a_stun() {
        assert!(!stuns(Some(FrenzyState::Rising)));
        assert!(stuns(Some(FrenzyState::Asked)));
        assert!(stuns(Some(FrenzyState::Done)));
        assert!(stuns(None));
    }

    #[test]
    fn the_pack_leaves_when_the_player_dies() {
        let mut app = App::new();
        app.add_message::<crate::hurt::Respawned>()
            .add_systems(Update, pack_leaves);
        let add = app.world_mut().spawn(PackMember).id();
        let other = app.world_mut().spawn_empty().id();
        app.update();
        assert!(app.world().get_entity(add).is_ok(), "no death, it stays");
        app.world_mut().write_message(crate::hurt::Respawned);
        app.update();
        assert!(app.world().get_entity(add).is_err());
        assert!(app.world().get_entity(other).is_ok());
    }

    #[test]
    fn the_pack_is_called_once_at_a_quarter() {
        assert!(!pack_call_due(false, 0.30));
        assert!(pack_call_due(false, 0.25));
        assert!(pack_call_due(false, 0.10));
        assert!(!pack_call_due(true, 0.10));
    }

    #[test]
    fn the_pack_comes_from_the_far_side() {
        let boss = Vec2::new(10.0, 0.0);
        let spot = pack_spot(boss, Vec2::ZERO, PACK_DISTANCE);
        assert!((spot - Vec2::new(10.0 + PACK_DISTANCE, 0.0)).length() < 1e-4);
        // A player on top of the boss: some side, the right distance.
        let spot = pack_spot(boss, boss, PACK_DISTANCE);
        assert!(((spot - boss).length() - PACK_DISTANCE).abs() < 1e-4);
    }

    #[test]
    fn a_reset_clears_the_pack_call() {
        assert!(pack_called_after(true, 0.5));
        assert!(!pack_called_after(true, 1.0));
        assert!(!pack_called_after(false, 0.5));
    }

    #[test]
    fn frenzy_waits_for_the_move_to_end() {
        use FrenzyState::*;
        // Phase two begins mid-swing: it waits.
        assert_eq!(frenzy_step(Waiting, true, Swing::Other), (Waiting, None));
        // The swing is over: it asks for the power-up, and keeps asking until it plays.
        assert_eq!(
            frenzy_step(Waiting, true, Swing::None),
            (Asked, Some(FrenzyEvent::Ask))
        );
        assert_eq!(
            frenzy_step(Asked, true, Swing::None),
            (Asked, Some(FrenzyEvent::Ask))
        );
        assert_eq!(frenzy_step(Asked, true, Swing::Other), (Asked, None));
        assert_eq!(
            frenzy_step(Asked, true, Swing::Frenzy),
            (Rising, Some(FrenzyEvent::Start))
        );
        assert!(Rising.armored() && !Asked.armored() && !Done.armored());
    }

    #[test]
    fn frenzy_happens_once_per_fight() {
        use FrenzyState::*;
        assert_eq!(frenzy_step(Rising, true, Swing::Frenzy), (Rising, None));
        assert_eq!(
            frenzy_step(Rising, true, Swing::None),
            (Done, Some(FrenzyEvent::End))
        );
        assert_eq!(frenzy_step(Done, true, Swing::None), (Done, None));
        // Not before phase two; a reset (phase one again) arms it for the next fight.
        assert_eq!(frenzy_step(Waiting, false, Swing::None), (Waiting, None));
        assert_eq!(frenzy_step(Done, false, Swing::None), (Waiting, None));
    }

    #[test]
    fn the_second_phase_starts_at_half_health_and_stays() {
        assert_eq!(phase_for(Phase::One, 1.0), Phase::One);
        assert_eq!(phase_for(Phase::One, 0.51), Phase::One);
        assert_eq!(phase_for(Phase::One, 0.50), Phase::Two);
        assert_eq!(phase_for(Phase::Two, 0.30), Phase::Two);
        // A creature that is reset to full health starts again; one that heals a little does not.
        assert_eq!(phase_for(Phase::Two, 0.80), Phase::Two);
        assert_eq!(phase_for(Phase::Two, 1.0), Phase::One);
    }

    #[test]
    fn a_phase_is_what_the_species_says() {
        let h = crate::species::by_name("hogger").unwrap();
        // The first phase already runs at the species' own tempo; the second quickens it again.
        assert_eq!(
            Phase::One.mods(h),
            Mods {
                tempo: h.tempo,
                chase: 1.0,
                cooldown: 1.0
            }
        );
        let two = Phase::Two.mods(h);
        assert!(two.tempo > h.tempo && two.chase > 1.0 && two.cooldown < 1.0);
        // A species without a second phase never changes.
        let k = crate::species::by_name("kobold").unwrap();
        assert_eq!(
            Phase::Two.mods(k),
            Mods {
                tempo: 1.0,
                chase: 1.0,
                cooldown: 1.0
            }
        );
    }

    #[test]
    fn pierce_armor_stacks_to_three_and_wears_off() {
        let mut s = ArmorShred::default();
        assert_eq!(s.factor(), 1.0);
        for _ in 0..5 {
            s.add();
        }
        assert!((s.factor() - 1.24).abs() < 1.0e-6, "{}", s.factor());
        s.tick(29.0);
        assert!(s.factor() > 1.0);
        s.tick(2.0);
        assert_eq!(s.factor(), 1.0);
        // A new blow refreshes the time of every stack.
        s.add();
        s.tick(25.0);
        s.add();
        s.tick(10.0);
        assert!(s.factor() > 1.0);
        s.clear();
        assert_eq!(s.factor(), 1.0);
    }

    #[test]
    fn the_phases_system_turns_a_wounded_boss_enraged_and_a_reset_one_calm() {
        let hogger = crate::species::by_name("hogger").unwrap();
        let mut app = App::new();
        app.add_message::<Roar>().add_systems(Update, phases);
        let e = app
            .world_mut()
            .spawn((
                crate::species::Species::new(hogger),
                crate::species::Boss,
                crate::combat::Health::full(900.0),
                BossPhase::default(),
                Tempo(1.0),
            ))
            .id();
        let state = |app: &App| {
            let w = app.world();
            (
                w.get::<BossPhase>(e).unwrap().0,
                w.get::<Tempo>(e).unwrap().0,
            )
        };
        app.update();
        assert_eq!(state(&app), (Phase::One, hogger.tempo));
        app.world_mut()
            .get_mut::<crate::combat::Health>(e)
            .unwrap()
            .cur = 449.0;
        app.update();
        let (phase, tempo) = state(&app);
        assert_eq!(phase, Phase::Two);
        assert!(tempo > hogger.tempo, "{tempo}");
        // A reset (the player's death) brings the first phase and the plain tempo back.
        app.world_mut()
            .get_mut::<crate::combat::Health>(e)
            .unwrap()
            .cur = 900.0;
        app.update();
        assert_eq!(state(&app), (Phase::One, hogger.tempo));
    }

    #[test]
    fn a_window_opens_once_as_the_clock_passes_its_start() {
        let hits = [(1.4, 1.7), (2.8, 3.0)];
        assert_eq!(window_opened(&hits, 0.0, 1.0), None);
        assert_eq!(window_opened(&hits, 1.3, 1.5), Some(0));
        // The frame after, the same window is behind.
        assert_eq!(window_opened(&hits, 1.5, 1.7), None);
        assert_eq!(window_opened(&hits, 2.7, 2.9), Some(1));
        // A move that has just begun counts from its first instant.
        assert_eq!(window_opened(&[(0.0, 0.2)], -1.0, 0.0), Some(0));
        assert_eq!(window_opened(&[], 0.0, 9.0), None);
    }

    #[test]
    fn the_slam_shakes_the_camera_near_the_boss_and_not_far_from_it() {
        assert!(slam_kick(2.0) > slam_kick(10.0));
        assert!(slam_kick(10.0) > 0.0);
        assert_eq!(slam_kick(SLAM_RANGE), 0.0);
        assert_eq!(slam_kick(SLAM_RANGE + 5.0), 0.0);
        assert!(slam_kick(0.0) <= 1.0);
    }

    #[test]
    fn only_hogger_has_a_boss_theme_and_it_is_the_games_best() {
        let list = music_playlist("hogger");
        assert!(list.len() >= 4, "{list:?}");
        assert!(list.iter().all(|p| p.starts_with("Sound\\Music\\")));
        let mut sorted = list.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), list.len(), "no track twice");
        assert!(music_playlist("garrick").is_empty());
        assert!(music_playlist("kobold").is_empty());
    }

    /// The install has every track of the theme, whole (a missing one would be silence at the fight).
    #[test]
    fn install_the_boss_theme_is_in_the_clients_data() {
        let Some(data) = benilla_formats::wow_data() else {
            eprintln!("skipped: no WoW install");
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        for path in music_playlist("hogger") {
            let bytes = chain
                .read_file(path)
                .unwrap_or_else(|e| panic!("{path}: {e:#}"));
            assert!(bytes.len() > 500_000, "{path}: {} bytes", bytes.len());
        }
    }

    #[test]
    fn a_bosss_starting_health_can_be_set_for_a_run_and_nothing_else_breaks_it() {
        assert_eq!(start_fraction(None), 1.0);
        assert_eq!(start_fraction(Some("0.45")), 0.45);
        assert_eq!(start_fraction(Some(" 0.5 ")), 0.5);
        // Junk, out of range and non-finite values read as a full-health boss or a nearly dead one.
        assert_eq!(start_fraction(Some("x")), 1.0);
        assert_eq!(start_fraction(Some("NaN")), 1.0);
        assert_eq!(start_fraction(Some("7")), 1.0);
        assert_eq!(start_fraction(Some("0")), MIN_START);
        assert_eq!(start_fraction(Some("-3")), MIN_START);
    }

    #[test]
    fn only_hogger_pierces() {
        assert!(pierces("hogger"));
        assert!(!pierces("kobold") && !pierces("garrick"));
    }
}
