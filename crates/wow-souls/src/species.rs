//! The game's creatures as data: which WoW model one wears, how much health it has, how it
//! moves and which pattern table it fights by. The Dark Souls side of a creature (its moves, flinches
//! and deaths) is its kit in [`crate::monster::Roster`], keyed by the species' `kit`, which is the
//! kit's name in `anim/source/manifest.txt` and the folder of its clips. Several species may share a
//! kit (the valley's three kinds of kobold fight with the ghoul's moves, each by its own table).
//!
//! `creature_template` (which creature wears which display) is server data, not in the client: each
//! display is picked by hand from `CreatureDisplayInfo`.

use bevy::prelude::*;

use crate::actor::CharLook;
use crate::pattern::Row;

/// Where a creature's blow starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blow {
    /// The right hand (attachment 1), along the forearm: a kobold's.
    Hand,
    /// The bone the kit's `reach` line names first (a wolf's jaw), along the line from the second
    /// bone (its head) to it.
    Bone,
    /// The blade of the weapon it holds, between the item's `$WTB` and `$WTT` markers (a thief's
    /// dagger); a weapon without markers falls back to the hand's volume.
    Blade,
}

/// How special a creature is. A database rare keeps its rank; named encounters such as Garrick
/// can also be assigned a game boss rank by their species row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    Normal,
    /// Respawns like anything else (4 x its database time), but drops the best of its table the first
    /// time it is killed.
    Rare,
    /// Never respawns.
    Miniboss,
    /// Never respawns, and drops the best of its table the first time it is killed.
    Boss,
}

impl Rank {
    pub fn never_respawns(self) -> bool {
        matches!(self, Rank::Miniboss | Rank::Boss)
    }

    pub fn first_kill_best(self) -> bool {
        matches!(self, Rank::Rare | Rank::Boss)
    }

    pub fn parse(s: &str) -> Option<Rank> {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Rank::Normal),
            "rare" => Some(Rank::Rare),
            "miniboss" => Some(Rank::Miniboss),
            "boss" => Some(Rank::Boss),
            _ => None,
        }
    }
}

/// `WOWSOULS_RANK=<species>:<rank>[,...]` as a map (a pair that does not read is dropped).
pub fn parse_rank_overrides(v: &str) -> std::collections::HashMap<String, Rank> {
    v.split(',')
        .filter_map(|p| p.split_once(':'))
        .filter_map(|(name, rank)| Some((name.trim().to_string(), Rank::parse(rank)?)))
        .collect()
}

/// The rank a species plays at: its row, unless `WOWSOULS_RANK` names it (a live check can make a kobold
/// a boss to watch the first-kill drop and the stay-dead rule).
pub fn rank_of(def: &SpeciesDef) -> Rank {
    static OVERRIDES: std::sync::OnceLock<std::collections::HashMap<String, Rank>> =
        std::sync::OnceLock::new();
    OVERRIDES
        .get_or_init(|| {
            std::env::var("WOWSOULS_RANK")
                .map(|v| parse_rank_overrides(&v))
                .unwrap_or_default()
        })
        .get(def.name)
        .copied()
        .unwrap_or(def.rank)
}

/// What a boss becomes at half health: another table, quicker swings and feet, shorter pauses.
pub struct PhaseTwo {
    pub table: &'static [Row],
    /// Multiplies the swing clock and the kit clip clock.
    pub tempo: f32,
    /// Multiplies the chase speed.
    pub chase: f32,
    /// Multiplies the pause after a swing.
    pub cooldown: f32,
}

/// One kind of creature.
pub struct SpeciesDef {
    /// The species' own name (`vermin`): the log lines, `WOWSOULS_RIG_VIEW=<name>:<clip>` and the
    /// `WOWSOULS_<NAME>_DISPLAY` switch use it.
    pub name: &'static str,
    /// The kit it fights with (`kobold`): the manifest's `monster <name>` line and the clips' folder.
    pub kit: &'static str,
    /// What the enemy bar calls it.
    pub title: &'static str,
    /// The creature's WoW level (`creature_template.level_max` of its entry): the attacker level of the
    /// armor formula ([`wow_world::gear_effect::armor_reduction`]).
    pub level: u8,
    /// The `CreatureDisplayInfo` row it wears; for a character body (`look`) the player's
    /// `CreatureDisplayInfo` 49, the human male.
    pub display: u32,
    /// A character body's own look (the player's; an NPC wears its display's), and the
    /// `ItemDisplayInfo` it holds in its right hand.
    pub look: Option<CharLook>,
    pub weapon: Option<u32>,
    /// A second weapon's item display, held in the left palm (not an extra damage window).
    pub offhand_weapon: Option<u32>,
    pub health: f32,
    /// How much of its kit's damage a blow of it does (1 for the creature the kit was measured on;
    /// a tuning value for the weaker kinds, the client has no creature levels).
    pub damage_scale: f32,
    /// The model's collision height and radius (yd) before scale (`CreatureModelData`).
    pub collision: (f32, f32),
    /// How fast it chases (yd/s): slower than the player's run, so the player can break off.
    pub chase_speed: f32,
    /// How near it closes in before it stops and fights (yd, centre to centre): inside the band of
    /// one of its moves, or it would stand there and never strike.
    pub stop_distance: f32,
    /// Which move it starts and when.
    pub table: &'static [Row],
    /// Where its blow starts, and how far past that point its volume reaches (yd before the
    /// entity scale): a club's length for a kobold that holds nothing, a snout's for a wolf.
    pub blow: Blow,
    pub reach_length: f32,
    /// How much the Dark Souls hit radius counts for: 1 for a creature whose natural weapon is the
    /// volume (a kobold's hand, a wolf's jaw), less for a held blade, whose own length is the reach.
    pub hit_radius_scale: f32,
    /// How much fuller than its body its hurtbox is (radius, height): the arms, the hat and the
    /// fur take blows too.
    pub hurt_grow: (f32, f32),
    /// The least thickness (yd) the player's blade sees in each of its body capsules; 0 for a
    /// creature the sword's own radius already reaches, more for a small one ([`crate::hits::HitFloor`]).
    pub hit_floor: f32,
    /// Its poise (a tuning value: the kit's own, the Dark Souls row's, is 0 for every creature of the
    /// valley, which makes every blow flinch it). The player's light blow takes 8 of it, a heavy one 20.
    pub poise: f32,
    /// How special it is: a [`Rank::Boss`] gets the screen's bar at the bottom while it fights ([`Boss`])
    /// and neither respawns nor drops twice (see [`Rank`]).
    pub rank: Rank,
    /// How it holds itself at a distance between its swings ([`crate::footwork`]).
    pub temper: crate::footwork::Temper,
    /// Family combat decisions, independent of the animation kit.
    pub behavior: crate::behavior::Role,
    /// The axe is in the left hand (the Taurus Demon's moves are a left-hander's).
    pub left_handed: bool,
    /// A second phase at half health ([`crate::boss`]).
    pub phase_two: Option<PhaseTwo>,
    /// The model's size next to the display's own (1 = as the client draws it): a boss is drawn
    /// larger than its model; its reach, root motion and hurtbox follow the size.
    pub size: f32,
    /// The swing and kit clip clock of the creature at all times (1 = the kit's own speed); a boss's
    /// second phase multiplies it ([`PhaseTwo::tempo`]).
    pub tempo: f32,
    /// A move ends this long (clip s) after its last hit window, not at its clip's end: the creature
    /// is back on its feet sooner than the Dark Souls clip's long tail (`None` = the whole clip).
    pub recovery: Option<f32>,
}

/// Marks the creature of a [`Rank::Boss`] species: the boss mechanics (music, slam shake, phases) and the
/// respawn rules of a boss.
#[derive(Component, Debug, Clone, Copy)]
pub struct Boss;

/// Marks a creature that gets the Dark Souls boss bar while it fights: every boss, and Elwynn's rares and named
/// quest mobs ([`BOSS_BAR_SPECIES`]), who keep their own rank (they respawn).
#[derive(Component, Debug, Clone, Copy)]
pub struct BossBar;

/// A miniboss's health and damage over its table's, and its least poise ([`SpeciesDef::named`]).
pub const NAMED_HEALTH: f32 = 2.2;
pub const NAMED_DAMAGE: f32 = 1.3;
pub const NAMED_POISE: f32 = 35.0;

/// The species that wear the boss bar without being bosses (spec 2026-10-17 elwynn-specials).
pub const BOSS_BAR_SPECIES: &[&str] = &[
    "thuros",
    "morgaine",
    "morgan",
    "dead_tooth_jack",
    "james_clark",
    "erlan",
    "surena",
    "goldtooth",
    "ruklar",
    "narg",
    "gruff",
    "fedfennel",
    "princess",
    "mother_fang",
];

/// Whether a creature of `def` gets the boss bar.
pub fn shows_boss_bar(def: &SpeciesDef) -> bool {
    rank_of(def) == Rank::Boss || BOSS_BAR_SPECIES.contains(&def.name)
}

impl SpeciesDef {
    /// How fast it walks when nothing is after it (yd/s): the chase speed scaled down the way WoW's
    /// walk is slower than its run, never slower than a crawl and never faster than the player's walk.
    pub fn walk_speed(&self) -> f32 {
        (self.chase_speed * 0.45).clamp(0.9, crate::motor::WALK_SPEED)
    }

    /// Its health in the world: the table's level-1 figure carried to its level
    /// ([`wow_world::balance::creature_health_factor`]).
    pub fn max_health(&self) -> f32 {
        (self.health * wow_world::balance::creature_health_factor(self.level) * self.named().0)
            .round()
    }

    /// Elwynn's rares and named quest mobs are minibosses (2026-10-18, the user: "ska vara som minibossar",
    /// "svåra. Inte hogger svåra"): every boss-bar species that is not a boss gets [`NAMED_HEALTH`] its
    /// table health, [`NAMED_DAMAGE`] its blows and at least [`NAMED_POISE`] poise. `(1, 1, 0)` for the rest.
    pub fn named(&self) -> (f32, f32, f32) {
        if rank_of(self) != Rank::Boss && BOSS_BAR_SPECIES.contains(&self.name) {
            (NAMED_HEALTH, NAMED_DAMAGE, NAMED_POISE)
        } else {
            (1.0, 1.0, 0.0)
        }
    }

    /// Its poise: the table's, at least a miniboss's ([`SpeciesDef::named`]).
    pub fn poise(&self) -> f32 {
        self.poise.max(self.named().2)
    }

    /// What its kit's damage is multiplied by: the table's scale carried to its level
    /// ([`wow_world::balance::creature_damage_factor`]).
    pub fn blow_scale(&self) -> f32 {
        self.damage_scale * wow_world::balance::creature_damage_factor(self.level) * self.named().1
    }
}

impl std::fmt::Debug for SpeciesDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Species({})", self.name)
    }
}

/// `CreatureDisplayInfo` 365: the kobold model at 0.85 scale with the normal skin; Dark Souls'
/// Blighttown ghoul (`kobold_reference`) gives it its moves. Health: a few light attacks. Collision from
/// `CreatureModelData` 26: 1.389 high, 0.6944 wide.
pub const KOBOLD: SpeciesDef = SpeciesDef {
    name: "kobold",
    behavior: crate::behavior::Role::KoboldSkirmisher,
    kit: "kobold",
    title: "Kobold",
    level: 4,
    display: 365,
    look: None,
    weapon: None,
    offhand_weapon: None,
    health: 60.0,
    damage_scale: 1.0,
    collision: (1.389, 0.347),
    chase_speed: 4.0,
    stop_distance: crate::npc::STOP_DISTANCE,
    table: crate::pattern::KOBOLD,
    blow: Blow::Hand,
    reach_length: 0.55,
    hit_radius_scale: 1.0,
    hurt_grow: (1.35, 1.1),
    hit_floor: 0.0,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::KOBOLD,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// `CreatureDisplayInfo` 604: the Timber Wolf (classicdb.ch, npc 69: the wolf inside Northshire
/// Valley), the wolf model (`Creature\Wolf\Wolf.m2`) at 0.55 scale with the timber skin; its run clip
/// covers 6.94 yd/s at scale 1, so 3.8 at this one. Dark Souls' black dog (`wolf_reference`) gives it two
/// moves. Health: two light attacks. Collision: the model's data says 2.083 high, which is a tall
/// wolf; the wolf stands 1.3 yd at the ears.
pub const WOLF: SpeciesDef = SpeciesDef {
    name: "wolf",
    behavior: crate::behavior::Role::WolfPack,
    kit: "wolf",
    title: "Timber Wolf",
    level: 2,
    display: 604,
    look: None,
    weapon: None,
    offhand_weapon: None,
    health: 40.0,
    damage_scale: 1.0,
    collision: (1.3, 0.45),
    chase_speed: 3.8,
    stop_distance: crate::npc::STOP_DISTANCE,
    table: crate::pattern::WOLF,
    blow: Blow::Bone,
    reach_length: 0.35,
    hit_radius_scale: 1.0,
    hurt_grow: (1.35, 1.1),
    hit_floor: 0.0,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::WOLF,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// The Defias Thug, the Defias of Northshire Valley (with Garrick Padfoot): `CreatureDisplayInfo`
/// 5035 (classicdb.ch, npc 38), a human male whose `CreatureDisplayInfoExtra` row 883 dresses him
/// (baked body atlas, bandana helm, chest, belt, legs, boots), with a copper dagger
/// (`ItemDisplayInfo` 2704, `Knife_1H_Dagger_A_01`) in his hand. Dark Souls' undead thief (`defias_duelist_reference`)
/// gives him two moves. Health: a few light attacks; he chases a little faster than a kobold. The
/// collision is the player's own capsule.
pub const THUG: SpeciesDef = SpeciesDef {
    name: "thug",
    behavior: crate::behavior::Role::DefiasRogue,
    kit: "thug",
    title: "Defias Thug",
    level: 4,
    display: 5035,
    look: None,
    weapon: Some(2704),
    offhand_weapon: None,
    health: 60.0,
    damage_scale: 1.0,
    collision: (crate::motor::CAPSULE_HEIGHT, crate::motor::CAPSULE_RADIUS),
    chase_speed: 4.5,
    stop_distance: crate::npc::STOP_DISTANCE,
    table: crate::pattern::THUG,
    blow: Blow::Blade,
    reach_length: 0.5,
    hit_radius_scale: 0.5,
    hurt_grow: (1.0, 1.0),
    hit_floor: 0.0,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::THUG,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// Elwynn's rare thief (entry 61). Both equipment slots hold item 1900 / display 7492.
/// The off-hand sword follows the actual left palm; the two single-hit attacks use the main blade.
pub const THUROS: SpeciesDef = SpeciesDef {
    name: "thuros",
    kit: "thuros",
    table: crate::pattern::THUROS,
    behavior: crate::behavior::Role::DefiasRogue,
    title: "Thuros Lightfingers",
    level: 11,
    display: 3341,
    weapon: Some(7492),
    offhand_weapon: Some(7492),
    health: 180.0,
    rank: Rank::Rare,
    ..THUG
};

/// Entry 94 has equally weighted male/female bodies and a basic dagger (2184 / 7434).
pub const CUTPURSE: SpeciesDef = SpeciesDef {
    name: "cutpurse",
    behavior: crate::behavior::Role::DefiasRogue,
    title: "Defias Cutpurse",
    level: 6,
    display: 2361,
    weapon: Some(7434),
    health: 90.0,
    ..THUG
};

/// Entry 99 is always female, with its own 117-bone target and curved dagger (5285 / 6469).
pub const MORGAINE: SpeciesDef = SpeciesDef {
    name: "morgaine",
    kit: "morgaine",
    table: crate::pattern::MORGAINE,
    behavior: crate::behavior::Role::DefiasRogue,
    title: "Morgaine the Sly",
    level: 10,
    display: 3320,
    weapon: Some(6469),
    health: 160.0,
    rank: Rank::Rare,
    ..THUG
};

/// Entry 473, a normal-rank named collector with a Thieves Blade (1900 / 7492).
pub const MORGAN: SpeciesDef = SpeciesDef {
    name: "morgan",
    kit: "morgan",
    table: crate::pattern::MORGAN,
    behavior: crate::behavior::Role::DefiasRogue,
    title: "Morgan the Collector",
    level: 10,
    display: 2074,
    weapon: Some(7492),
    health: 140.0,
    ..THUG
};

/// Entry 6093 carries a one-handed metal axe (1905 / 7428).
pub const DEAD_TOOTH_JACK: SpeciesDef = SpeciesDef {
    name: "dead_tooth_jack",
    kit: "dead_tooth_jack",
    table: crate::pattern::DEAD_TOOTH_JACK,
    behavior: crate::behavior::Role::DefiasBrawler,
    title: "Dead-Tooth Jack",
    level: 11,
    display: 4947,
    weapon: Some(7428),
    health: 160.0,
    ..THUG
};

/// Entry 13159 carries a longsword (1899 / 7483); neutral in the database, aggressive here like every enemy.
pub const JAMES_CLARK: SpeciesDef = SpeciesDef {
    name: "james_clark",
    kit: "james_clark",
    table: crate::pattern::JAMES_CLARK,
    behavior: crate::behavior::Role::DefiasBrawler,
    title: "James Clark",
    level: 12,
    display: 13091,
    weapon: Some(7483),
    health: 180.0,
    ..THUG
};

/// Entry 880 carries a two-handed pick/axe (5289 / 3797), with real PC two-handed axe swings.
pub const ERLAN: SpeciesDef = SpeciesDef {
    name: "erlan",
    kit: "erlan",
    table: crate::pattern::ERLAN_ELITE,
    behavior: crate::behavior::Role::HeavyRaider,
    title: "Erlan Drudgemoor",
    level: 8,
    display: 3322,
    weapon: Some(3797),
    health: 150.0,
    ..THUG
};

/// Entry 116: both human bodies, a short sword (1896 / 7487) and the actual right-foot kick.
pub const BANDIT: SpeciesDef = SpeciesDef {
    name: "bandit",
    behavior: crate::behavior::Role::DefiasBrawler,
    kit: "human_bandit",
    title: "Defias Bandit",
    level: 9,
    display: 2357,
    weapon: Some(7487),
    health: 130.0,
    stop_distance: 1.1,
    table: crate::pattern::BANDIT,
    ..THUG
};

/// Defias Rogue Wizard (entry 474, level 9-10): both human bodies (displays 2360 / 2359), a basic staff (item
/// 1907, display 10654). Kit `caster`: the player's staff cast throwing a bolt (WoW: Frostbolt, `creature_spells`
/// 4740) from a distance, the thief's slash as a swat close in. It stops at five yards to cast.
pub const ROGUE_WIZARD: SpeciesDef = SpeciesDef {
    name: "rogue_wizard",
    behavior: crate::behavior::Role::CasterKite,
    kit: "caster",
    title: "Defias Rogue Wizard",
    level: 10,
    display: 2360,
    weapon: Some(10654),
    health: 110.0,
    stop_distance: 5.0,
    table: crate::pattern::CASTER,
    ..THUG
};

/// Surena Caledon (entry 881, level 9): the female body (display 3321) and the same staff; her bolt is fire (WoW:
/// Fireball, 8810).
pub const SURENA: SpeciesDef = SpeciesDef {
    name: "surena",
    kit: "surena",
    table: crate::pattern::SURENA,
    behavior: crate::behavior::Role::CasterKite,
    title: "Surena Caledon",
    level: 9,
    display: 3321,
    health: 160.0,
    ..ROGUE_WIZARD
};

/// Entry 6927 shares the sword/kick role and displays, with its own identity, HP and loot entry.
pub const DOCKWORKER: SpeciesDef = SpeciesDef {
    name: "dockworker",
    behavior: crate::behavior::Role::DefiasBrawler,
    title: "Defias Dockworker",
    health: 110.0,
    ..BANDIT
};

/// Defias Ambusher (entry 583, level 8-9, display 184, a basic dagger: item 2184, display 7434): no spawn of its own;
/// Clara Charles's script (591701) sends one out when the player comes near her ([`crate::named`]). The thief's
/// two slashes, as the Thugs, on its own body (the `HumanThief` creature model, kit `ambusher`).
pub const AMBUSHER: SpeciesDef = SpeciesDef {
    name: "ambusher",
    behavior: crate::behavior::Role::DefiasRogue,
    kit: "ambusher",
    title: "Defias Ambusher",
    level: 9,
    display: 184,
    weapon: Some(7434),
    health: 120.0,
    ..THUG
};

/// Defias Bodyguard (entry 6866, level 10, displays 2340 / 2341): no spawn of its own; the Dockmaster's script
/// (684602) calls three into its fight ([`crate::named`]). The bandits' sword and kick; it backstabs.
pub const BODYGUARD: SpeciesDef = SpeciesDef {
    name: "bodyguard",
    behavior: crate::behavior::Role::LeaderEscort,
    title: "Defias Bodyguard",
    level: 10,
    display: 2340,
    health: 140.0,
    ..BANDIT
};

/// `CreatureDisplayInfo` 10913: the Kobold Vermin (classicdb.ch, npc 6; level 1-2, 42-55 health in
/// the sources): the kobold model at 0.7 scale, the normal skin. The weakest creature of the game:
/// the ghoul's two light swings only, a smaller body (bands and chase speed scaled by 0.7 / 0.85), two
/// light attacks of health and 70 % of the kit's damage.
pub const VERMIN: SpeciesDef = SpeciesDef {
    name: "vermin",
    behavior: crate::behavior::Role::KoboldSkirmisher,
    kit: "kobold",
    title: "Kobold Vermin",
    level: 2,
    display: 10913,
    look: None,
    weapon: None,
    offhand_weapon: None,
    health: 40.0,
    damage_scale: 0.7,
    collision: (1.389, 0.347),
    chase_speed: 3.3,
    stop_distance: crate::npc::STOP_DISTANCE,
    table: crate::pattern::VERMIN,
    blow: Blow::Hand,
    reach_length: 0.55,
    hit_radius_scale: 1.0,
    hurt_grow: (1.35, 1.1),
    hit_floor: 0.0,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::VERMIN,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// `CreatureDisplayInfo` 10912: the Kobold Worker (classicdb.ch, npc 257; level 3, 71 health in the
/// sources): the kobold model at 0.85 with the albino skin. The laborer's size and health, without
/// its charges: the two light swings and the big one, 90 % of the kit's damage.
pub const WORKER: SpeciesDef = SpeciesDef {
    name: "worker",
    behavior: crate::behavior::Role::KoboldSkirmisher,
    kit: "kobold",
    title: "Kobold Worker",
    level: 3,
    display: 10912,
    look: None,
    weapon: None,
    offhand_weapon: None,
    health: 60.0,
    damage_scale: 0.9,
    collision: (1.389, 0.347),
    chase_speed: 4.0,
    stop_distance: crate::npc::STOP_DISTANCE,
    table: crate::pattern::WORKER,
    blow: Blow::Hand,
    reach_length: 0.55,
    hit_radius_scale: 1.0,
    hurt_grow: (1.35, 1.1),
    hit_floor: 0.0,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::WORKER,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// Elwynn's level 6-7 miner (entry 40): display 373 scales the same Kobold rig to 1.15.
/// Its DB equipment is a pick (item 1910, item display 7493); only the two light swings are used.
pub const MINER: SpeciesDef = SpeciesDef {
    name: "miner",
    behavior: crate::behavior::Role::KoboldSkirmisher,
    title: "Kobold Miner",
    level: 7,
    display: 373,
    weapon: Some(7493),
    health: 110.0,
    chase_speed: 5.4,
    stop_distance: 0.9 * 1.15,
    table: crate::pattern::MINER,
    blow: Blow::Blade,
    ..KOBOLD
};

/// Kobold Geomancer (entry 476, level 7-8): display 163, the kobold body with a basic staff (item 1907, display
/// 10654). Kit `geomancer`: the staff cast throwing fire (WoW: Fireball, `creature_spells` 4760) from a distance,
/// the ghoul's light swing close in.
pub const GEOMANCER: SpeciesDef = SpeciesDef {
    name: "geomancer",
    behavior: crate::behavior::Role::CasterKite,
    kit: "geomancer",
    title: "Kobold Geomancer",
    level: 8,
    display: 163,
    weapon: Some(10654),
    health: 100.0,
    stop_distance: 4.5,
    table: crate::pattern::GEOMANCER,
    blow: Blow::Blade,
    ..KOBOLD
};

/// Entry 475, level 5-6: display 139 at scale 1, with the DB stone hammer (1901 / 7440).
pub const TUNNELER: SpeciesDef = SpeciesDef {
    name: "tunneler",
    behavior: crate::behavior::Role::KoboldSkirmisher,
    title: "Kobold Tunneler",
    level: 6,
    display: 139,
    weapon: Some(7440),
    health: 90.0,
    chase_speed: 4.7,
    stop_distance: 0.9,
    table: crate::pattern::TUNNELER,
    blow: Blow::Blade,
    ..KOBOLD
};

/// Entry 327: Goldtooth is an ordinary level 8 kobold, display 2299 at 1.15, holding the
/// DB metal hammer (1903 / 7439). Being named does not give him a boss rank or extra attacks.
pub const GOLDTOOTH: SpeciesDef = SpeciesDef {
    name: "goldtooth",
    kit: "goldtooth",
    table: crate::pattern::GOLDTOOTH_ELITE,
    behavior: crate::behavior::Role::KoboldSkirmisher,
    title: "Goldtooth",
    level: 8,
    display: 2299,
    weapon: Some(7439),
    health: 150.0,
    chase_speed: 5.4,
    stop_distance: 0.9 * 1.15,
    blow: Blow::Blade,
    ..KOBOLD
};

/// Entry 60: level 8 Ruklar wears display 2153 at 1.3 and the DB axe (1905 / 7428).
/// The axe's overhead is his sole melee move; its light arcs pass above/beside the player.
/// Net and Pierce Armor are not part of this melee kit.
pub const RUKLAR: SpeciesDef = SpeciesDef {
    name: "ruklar",
    kit: "ruklar",
    table: crate::pattern::RUKLAR_ELITE,
    behavior: crate::behavior::Role::KoboldSkirmisher,
    title: "Ruklar the Trapper",
    level: 8,
    display: 2153,
    weapon: Some(7428),
    health: 170.0,
    chase_speed: 6.1,
    stop_distance: 0.9 * 1.3,
    blow: Blow::Blade,
    ..KOBOLD
};

/// Entry 79: the level 10 rare Narg, display 774 at 1.3, with the DB metal hammer (1903 / 7439).
/// His rare loot/respawn rules apply now; Battle Shout awaits the named-ability pass.
pub const NARG: SpeciesDef = SpeciesDef {
    name: "narg",
    kit: "narg",
    table: crate::pattern::NARG_ELITE,
    behavior: crate::behavior::Role::KoboldSkirmisher,
    title: "Narg the Taskmaster",
    level: 10,
    display: 774,
    weapon: Some(7439),
    health: 210.0,
    chase_speed: 6.1,
    stop_distance: 0.9 * 1.3,
    blow: Blow::Blade,
    rank: Rank::Rare,
    ..KOBOLD
};

/// Entry 525: a level 5-6 Mangy Wolf, display 903 at 0.7 on the existing Wolf rig.
pub const MANGY_WOLF: SpeciesDef = SpeciesDef {
    name: "mangy_wolf",
    behavior: crate::behavior::Role::WolfPack,
    title: "Mangy Wolf",
    level: 6,
    display: 903,
    health: 90.0,
    chase_speed: 4.8,
    stop_distance: crate::npc::STOP_DISTANCE * (0.7 / 0.55),
    table: crate::pattern::MANGY_WOLF,
    ..WOLF
};

/// Entry 1922: a level 7-8 Gray Forest Wolf, display 380 at 0.85 on the existing Wolf rig.
pub const GRAY_FOREST_WOLF: SpeciesDef = SpeciesDef {
    name: "gray_forest_wolf",
    behavior: crate::behavior::Role::WolfPack,
    title: "Gray Forest Wolf",
    level: 8,
    display: 380,
    health: 120.0,
    chase_speed: 5.9,
    stop_distance: crate::npc::STOP_DISTANCE * (0.85 / 0.55),
    table: crate::pattern::GRAY_FOREST_WOLF,
    ..WOLF
};

/// Entry 118: level 9-10 Prowler, display 11415 at 0.85. DireWolf has 61 bones and its
/// own retargeted black-dog bite; the 64-bone Wolf clips cannot pose this body.
pub const PROWLER: SpeciesDef = SpeciesDef {
    name: "prowler",
    behavior: crate::behavior::Role::WolfPack,
    kit: "direwolf",
    title: "Prowler",
    level: 10,
    display: 11415,
    health: 160.0,
    // Bind ears stand 1.56 yd high; the client's collision header is a generic wolf capsule.
    collision: (1.55, 0.45),
    chase_speed: 5.9,
    stop_distance: 1.0,
    table: crate::pattern::PROWLER,
    ..WOLF
};

/// `CreatureDisplayInfo` 447: the Young Wolf (classicdb.ch, npc 299; level 1, 42 health in the
/// sources; the valley's before Cataclysm, by the wiki): the wolf model at 0.4 scale with the timber
/// skin. The black dog's bite only, a smaller body (the bite band scaled by 0.4 / 0.55), two light
/// attacks of health (30), its run clip's 6.94 yd/s at this scale (2.8) and 70 % of the kit's damage.
pub const YOUNG_WOLF: SpeciesDef = SpeciesDef {
    name: "young_wolf",
    behavior: crate::behavior::Role::WolfPack,
    kit: "wolf",
    title: "Young Wolf",
    level: 1,
    display: 447,
    look: None,
    weapon: None,
    offhand_weapon: None,
    health: 30.0,
    damage_scale: 0.7,
    collision: (1.3, 0.45),
    chase_speed: 2.8,
    stop_distance: 0.85,
    table: crate::pattern::YOUNG_WOLF,
    blow: Blow::Bone,
    reach_length: 0.35,
    hit_radius_scale: 1.0,
    hurt_grow: (1.35, 1.1),
    hit_floor: 0.35,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::YOUNG_WOLF,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// Riverpaw Outrunner (entry 478, level 9-10): display 512, the Gnoll Fighter model (`GnollMelee`) at
/// 0.85 with the basic metal axe (item 1905, display 7428). Dark Souls' undead soldier (`murloc_reference`, kit
/// `gnoll`) gives the Riverpaw gnolls two light club swings. Health: about four light blows of a level
/// 10 player; 70 % of the kit's damage, as the other level 8-10 humans.
pub const OUTRUNNER: SpeciesDef = SpeciesDef {
    name: "outrunner",
    behavior: crate::behavior::Role::GnollPressure,
    kit: "gnoll",
    title: "Riverpaw Outrunner",
    level: 10,
    display: 512,
    look: None,
    weapon: Some(7428),
    offhand_weapon: None,
    health: 120.0,
    damage_scale: 0.7,
    collision: (1.564, 0.417),
    chase_speed: 4.4,
    stop_distance: crate::npc::STOP_DISTANCE,
    table: crate::pattern::GNOLL,
    blow: Blow::Blade,
    reach_length: 0.9,
    hit_radius_scale: 0.7,
    hurt_grow: (1.1, 1.05),
    hit_floor: 0.0,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::THUG,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// Riverpaw Runt (entry 97, level 8-9): display 10791, the whelp's own model (`Gnoll.m2`, the same 60-bone
/// skeleton as `GnollMelee`) at 0.85 with a rusty short sword (item 1895, display 7488).
pub const RUNT: SpeciesDef = SpeciesDef {
    name: "runt",
    behavior: crate::behavior::Role::GnollPressure,
    title: "Riverpaw Runt",
    level: 9,
    display: 10791,
    weapon: Some(7488),
    health: 100.0,
    reach_length: 0.7,
    ..OUTRUNNER
};

/// Gruff Swiftbite (entry 100, rare, level 12): display 175, the Gnoll Fighter at its own size with the
/// stone axe (item 1904, display 7429). Thrash (a double swing) is S6 work.
pub const GRUFF: SpeciesDef = SpeciesDef {
    name: "gruff",
    kit: "gruff",
    table: crate::pattern::GRUFF,
    behavior: crate::behavior::Role::GnollPressure,
    title: "Gruff Swiftbite",
    level: 12,
    display: 175,
    weapon: Some(7429),
    health: 230.0,
    chase_speed: 4.8,
    rank: Rank::Rare,
    ..OUTRUNNER
};

/// Fedfennel (entry 472, rare, level 12): display 175 with the large axe (item 1909, display 7426).
pub const FEDFENNEL: SpeciesDef = SpeciesDef {
    name: "fedfennel",
    kit: "fedfennel",
    table: crate::pattern::FEDFENNEL,
    behavior: crate::behavior::Role::GnollPressure,
    title: "Fedfennel",
    weapon: Some(7426),
    health: 250.0,
    reach_length: 1.0,
    ..GRUFF
};

/// Murloc (entry 285, level 6-7): display 617, the murloc model at 0.85 with a rusty short sword (item 1895,
/// display 7488). The undead soldier's (`murloc_reference`, kit `murloc`: its running lunge and three-swing flurry) on the
/// 62-bone murloc body. The kit's body scale is 0.78, so the bands and the stop distance are smaller.
pub const MURLOC: SpeciesDef = SpeciesDef {
    name: "murloc",
    behavior: crate::behavior::Role::MurlocSwarm,
    kit: "murloc",
    title: "Murloc",
    level: 7,
    display: 617,
    look: None,
    weapon: Some(7488),
    offhand_weapon: None,
    health: 70.0,
    damage_scale: 0.7,
    collision: (1.5, 0.45),
    chase_speed: 3.8,
    stop_distance: 1.0,
    table: crate::pattern::MURLOC,
    blow: Blow::Blade,
    reach_length: 0.65,
    hit_radius_scale: 0.7,
    hurt_grow: (1.15, 1.1),
    hit_floor: 0.0,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::THUG,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// Murloc Streamrunner (entry 735, level 6-7): display 527 with a spiked club (item 2695, display 7432).
pub const STREAMRUNNER: SpeciesDef = SpeciesDef {
    name: "streamrunner",
    behavior: crate::behavior::Role::MurlocSwarm,
    title: "Murloc Streamrunner",
    display: 527,
    weapon: Some(7432),
    chase_speed: 4.4,
    ..MURLOC
};

/// Murloc Forager (entry 46, level 9-10): display 441 at full size with a basic scimitar (item 1897,
/// display 7485).
pub const FORAGER: SpeciesDef = SpeciesDef {
    name: "forager",
    behavior: crate::behavior::Role::MurlocSwarm,
    title: "Murloc Forager",
    level: 10,
    display: 441,
    weapon: Some(7485),
    health: 100.0,
    stop_distance: 1.25,
    table: crate::pattern::MURLOC_LARGE,
    reach_length: 0.8,
    ..MURLOC
};

/// Murloc Lurker (entry 732, level 9-10): display 983 with a basic dagger (item 2184, display 7434).
/// Backstab is S6 work.
pub const LURKER: SpeciesDef = SpeciesDef {
    name: "lurker",
    behavior: crate::behavior::Role::MurlocSwarm,
    title: "Murloc Lurker",
    display: 983,
    weapon: Some(7434),
    health: 90.0,
    reach_length: 0.6,
    ..FORAGER
};

/// Stonetusk Boar (entry 113, level 5-6; neutral in WoW, aggressive here): display 503, the boar model at
/// 0.6. The Armored Tusk's gore and short charge (kit `boar`, both tamed to an ordinary mob's weight). The
/// Tusk's kit scale (0.82)
/// is 0.7 of the dog's the boars wore before, so the blow's radius is grown back; a little poise (two light
/// blows or one heavy break it) keeps its long staggers from coming every swing. Rushing Charge is S6 work.
pub const STONETUSK: SpeciesDef = SpeciesDef {
    name: "stonetusk",
    behavior: crate::behavior::Role::BoarCharge,
    kit: "boar",
    title: "Stonetusk Boar",
    level: 6,
    display: 503,
    look: None,
    weapon: None,
    offhand_weapon: None,
    health: 80.0,
    damage_scale: 0.8,
    collision: (1.3, 0.55),
    chase_speed: 4.8,
    stop_distance: crate::npc::STOP_DISTANCE,
    table: crate::pattern::BOAR,
    blow: Blow::Bone,
    reach_length: 0.4,
    hit_radius_scale: 1.5,
    hurt_grow: (1.3, 1.1),
    hit_floor: 0.0,
    poise: 16.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::WOLF,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    // The Tusk's gore is a 3.7 s clip with its blow done at 1.7 s: the boar is back on its trotters
    // 0.6 s after a blow instead of standing through the tail (2026-10-17, "attackerar inte tillräckligt").
    recovery: Some(0.6),
};

/// Porcine Entourage (entry 390, level 7): display 377 at 0.6.
pub const PORCINE: SpeciesDef = SpeciesDef {
    name: "porcine",
    behavior: crate::behavior::Role::BoarCharge,
    title: "Porcine Entourage",
    level: 7,
    display: 377,
    health: 85.0,
    ..STONETUSK
};

/// Rockhide Boar (entry 524, level 7-8): display 389 at 0.75, so its bands scale by 0.75 / 0.6.
pub const ROCKHIDE: SpeciesDef = SpeciesDef {
    name: "rockhide",
    behavior: crate::behavior::Role::BoarCharge,
    title: "Rockhide Boar",
    level: 8,
    display: 389,
    health: 110.0,
    stop_distance: crate::npc::STOP_DISTANCE * 1.25,
    table: crate::pattern::BOAR_LARGE,
    ..STONETUSK
};

/// Princess (entry 330, level 9): display 8871 at 0.75.
pub const PRINCESS: SpeciesDef = SpeciesDef {
    name: "princess",
    kit: "princess",
    table: crate::pattern::PRINCESS,
    behavior: crate::behavior::Role::BoarCharge,
    title: "Princess",
    level: 9,
    display: 8871,
    // A miniboss ([`SpeciesDef::named`]; 2026-10-18, the user: she "dog på två hits med 2 handed mace"), up
    // again soon after each blow.
    health: 160.0,
    recovery: Some(0.4),
    ..ROCKHIDE
};

/// Young Forest Bear (entry 822, level 8-9): display 1006, the bear model at 0.85. The Great Feline's lunging bite
/// and a drake's claw swipe (kit `bear`, both tamed). Its kit scale (0.78) is half the dog's the bear wore before, so
/// the blow's radius is grown back; the Feline's poise (45) comes with it.
pub const BEAR: SpeciesDef = SpeciesDef {
    name: "bear",
    behavior: crate::behavior::Role::BearPressure,
    kit: "bear",
    title: "Young Forest Bear",
    level: 9,
    display: 1006,
    look: None,
    weapon: None,
    offhand_weapon: None,
    health: 160.0,
    damage_scale: 1.0,
    collision: (2.0, 0.8),
    chase_speed: 4.6,
    stop_distance: 1.4,
    table: crate::pattern::BEAR,
    blow: Blow::Bone,
    reach_length: 0.6,
    hit_radius_scale: 1.6,
    hurt_grow: (1.2, 1.1),
    hit_floor: 0.0,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::WOLF,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// Defias Dockmaster (entry 6846, level 10): display 7849, the goblin male body (`GoblinMale`, 0.54 of the
/// human's size in the kit's scale), bare-handed (the database gives it no equipment). The thug's two slashes
/// as punches (kit `goblin`). Its Bodyguard summons are S6 work.
pub const DOCKMASTER: SpeciesDef = SpeciesDef {
    name: "dockmaster",
    behavior: crate::behavior::Role::ProtectedLeader,
    kit: "goblin",
    title: "Defias Dockmaster",
    level: 10,
    display: 7849,
    look: None,
    weapon: None,
    offhand_weapon: None,
    health: 150.0,
    damage_scale: 0.8,
    collision: (1.35, 0.4),
    chase_speed: 4.2,
    stop_distance: 0.9,
    table: crate::pattern::GOBLIN,
    blow: Blow::Hand,
    reach_length: 0.45,
    hit_radius_scale: 1.0,
    hurt_grow: (1.1, 1.05),
    hit_floor: 0.0,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::THUG,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// Forest Spider (entry 30, level 5-6): display 382, the tarantula model at 0.4. The black dog's bite and
/// lunge (kit `tarantula`) turn its carapace and head over its own legs (only those are driven). The kit's
/// body scale is 1.55, 0.88 of the wolf's effective size, so the wolf's bands carry over scaled by it.
pub const FOREST_SPIDER: SpeciesDef = SpeciesDef {
    name: "forest_spider",
    behavior: crate::behavior::Role::SpiderAmbush,
    kit: "tarantula",
    title: "Forest Spider",
    level: 6,
    display: 382,
    look: None,
    weapon: None,
    offhand_weapon: None,
    health: 70.0,
    damage_scale: 0.8,
    collision: (0.7, 0.8),
    chase_speed: 4.4,
    stop_distance: 0.9,
    table: crate::pattern::FOREST_SPIDER,
    blow: Blow::Bone,
    reach_length: 1.8,
    hit_radius_scale: 1.4,
    hurt_grow: (1.2, 1.2),
    hit_floor: 0.0,
    poise: 0.0,
    rank: Rank::Normal,
    temper: crate::footwork::Temper::WOLF,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// Mine Spider (entry 43, level 8-9): display 368, the mine spider model at 0.55. The kit's body scale is
/// 2.49 (the model is large), so its bands are about twice the wolf's.
pub const MINE_SPIDER: SpeciesDef = SpeciesDef {
    name: "mine_spider",
    behavior: crate::behavior::Role::SpiderAmbush,
    kit: "minespider",
    title: "Mine Spider",
    level: 9,
    display: 368,
    health: 100.0,
    collision: (0.9, 1.4),
    chase_speed: 4.6,
    stop_distance: 1.6,
    table: crate::pattern::MINE_SPIDER,
    reach_length: 1.2,
    ..FOREST_SPIDER
};

/// Mother Fang (entry 471, rare, level 10): display 2541, the mine spider at 0.7. Web is S6 work.
pub const MOTHER_FANG: SpeciesDef = SpeciesDef {
    name: "mother_fang",
    kit: "mother_fang",
    table: crate::pattern::MOTHER_FANG_ELITE,
    behavior: crate::behavior::Role::SpiderAmbush,
    title: "Mother Fang",
    level: 10,
    display: 2541,
    health: 230.0,
    rank: Rank::Rare,
    stop_distance: 2.6,
    reach_length: 1.4,
    ..MINE_SPIDER
};

/// Garrick Padfoot, the valley's named Defias (classicdb.ch, npc 103: level 5, 102 HP in the sources):
/// `CreatureDisplayInfo` 2073, a human male whose extra row 676 dresses him (red outfit, bandana
/// helm, shirt, belt, legs, boots, gloves), with the thug's copper dagger (the client has no creature
/// equipment; a guess). Dark Souls' Anor Londo assassin (`defias_brawler_reference`, [`crate::monster`] kit `garrick`)
/// gives him five moves: a quick poke, a double and a triple, a lunge and a dash. Health: nine light
/// attacks (the sources' 102 is an ordinary creature's two to three times over); poise 20, so three
/// light blows or one heavy one break his swing; 80 % of the kit's damage, since a triple lands three.
pub const GARRICK: SpeciesDef = SpeciesDef {
    name: "garrick",
    behavior: crate::behavior::Role::EliteDuelist,
    kit: "garrick",
    title: "Garrick Padfoot",
    level: 5,
    display: 2073,
    look: None,
    weapon: Some(2704),
    offhand_weapon: None,
    health: 180.0,
    damage_scale: 0.8,
    collision: (crate::motor::CAPSULE_HEIGHT, crate::motor::CAPSULE_RADIUS),
    chase_speed: 5.0,
    stop_distance: crate::npc::STOP_DISTANCE,
    table: crate::pattern::GARRICK,
    blow: Blow::Blade,
    reach_length: 0.5,
    hit_radius_scale: 0.5,
    hurt_grow: (1.0, 1.0),
    hit_floor: 0.0,
    poise: 20.0,
    rank: Rank::Boss,
    temper: crate::footwork::Temper::GARRICK,
    left_handed: false,
    phase_two: None,
    size: 1.0,
    tempo: 1.0,
    recovery: None,
};

/// Hogger (vMaNGOS entry 448: level 11 elite, health x3, damage x1.7): display 384 is the Riverpaw
/// Gnoll Fighter model (`GnollMelee`) at 1.15 with a basic metal axe (item 1905, item display
/// 7428), in the Taurus Demon's (`hogger_brute_reference`) moves. A boss in the Dark Souls sense: the boss bar, one
/// kill, a second phase at half health. Health 500 in level-1 units, 927 at level 11 ([`SpeciesDef::max_health`];
/// about 30 light blows of a level 11 player's sword),
/// poise 40 (a light blow takes 8, a heavy one 20); the kit's damage (`atkPhys` 280-310 x 25/138 =
/// 51-56) at 0.6, 1.5 once the level's x2.5 ([`SpeciesDef::blow_scale`]) is on: four blows of a level 11 player's 200 health before armour. Kept for comparison and
/// fallback: `WOWSOULS_HOGGER=taurus` ([`resolve`]); not in [`ROSTER`].
pub const HOGGER_TAURUS: SpeciesDef = SpeciesDef {
    behavior: crate::behavior::Role::BossCommitted,
    name: "hogger",
    kit: "hogger",
    title: "Hogger",
    level: 11,
    display: 384,
    look: None,
    weapon: Some(7428),
    offhand_weapon: None,
    health: 500.0,
    damage_scale: 0.6,
    collision: (1.564, 0.417),
    chase_speed: 3.6,
    stop_distance: crate::npc::STOP_DISTANCE,
    table: crate::pattern::HOGGER,
    blow: Blow::Blade,
    reach_length: 1.3,
    hit_radius_scale: 0.9,
    hurt_grow: (1.1, 1.05),
    hit_floor: 0.0,
    poise: 40.0,
    rank: Rank::Boss,
    temper: crate::footwork::Temper::HOGGER,
    left_handed: true,
    phase_two: Some(PhaseTwo {
        table: crate::pattern::HOGGER_TWO,
        tempo: 1.15,
        chase: 1.15,
        cooldown: 0.7,
    }),
    size: 1.3,
    tempo: 1.4,
    recovery: Some(1.1),
};

/// Hogger as Knight Artorias (spec `2026-10-16-hogger-rework-design.md`): the same gnoll, axe, place, health
/// and phases as [`HOGGER_TAURUS`], fighting with the knight's moves (kit `hogger_knight`, right-handed, the
/// source's own clip clock). Aggressive: short pauses, a pressing temper, chains and gap closers. Damage: the
/// knight's `atkPhys` 140-250 x 25/138 x 2.2 = 56-100 before armour (0.88 here, times level 11's x2.5); on the level 11 trial player (290 health,
/// armour 1067) a medium blow takes about an eighth, the flip slam a fifth (scripted runs 2026-10-16).
pub const HOGGER: SpeciesDef = SpeciesDef {
    name: "hogger",
    behavior: crate::behavior::Role::BossCommitted,
    kit: "hogger_knight",
    // A greatsword's arcs pass round a player closer than this: he keeps the blade's distance.
    stop_distance: 2.0,
    // A two-handed war axe ("Monster - Axe, 2H War A03 White", item 14475) in one hand: the knight's arcs are
    // spaced for a greatsword, and the 1H axe's blade (0.43 yd) passed beside a player in reach; this one's is 0.65.
    weapon: Some(3367),
    // 0.88 until 2026-10-17; the user: his blows were "lite för svaga" for how hard they look.
    damage_scale: 1.25,
    chase_speed: 4.2,
    table: crate::pattern::HOGGER_KNIGHT,
    temper: crate::footwork::Temper::HOGGER_KNIGHT,
    left_handed: false,
    phase_two: Some(PhaseTwo {
        table: crate::pattern::HOGGER_KNIGHT_TWO,
        tempo: 1.1,
        chase: 1.15,
        cooldown: 0.6,
    }),
    tempo: 1.0,
    recovery: Some(1.0),
    ..HOGGER_TAURUS
};

/// `WOWSOULS_HOGGER=taurus` brings back the Taurus Demon's Hogger (kit, table, styles, temper, clock); any other
/// value, or none, keeps the knight. Only Hogger is ever swapped.
pub fn resolve(def: &'static SpeciesDef) -> &'static SpeciesDef {
    static CHOICE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let choice = CHOICE.get_or_init(|| std::env::var("WOWSOULS_HOGGER").ok());
    resolve_with(def, choice.as_deref())
}

/// [`resolve`] with the switch's value given.
pub fn resolve_with(def: &'static SpeciesDef, choice: Option<&str>) -> &'static SpeciesDef {
    match choice {
        Some(c) if def.kit == HOGGER.kit && c.trim().eq_ignore_ascii_case("taurus") => {
            &HOGGER_TAURUS
        }
        _ => def,
    }
}

/// Every species the game spawns.
pub const ROSTER: &[&SpeciesDef] = &[
    &KOBOLD,
    &VERMIN,
    &WORKER,
    &MINER,
    &TUNNELER,
    &GOLDTOOTH,
    &RUKLAR,
    &NARG,
    &WOLF,
    &YOUNG_WOLF,
    &MANGY_WOLF,
    &GRAY_FOREST_WOLF,
    &PROWLER,
    &THUG,
    &THUROS,
    &CUTPURSE,
    &MORGAINE,
    &MORGAN,
    &DEAD_TOOTH_JACK,
    &JAMES_CLARK,
    &ERLAN,
    &BANDIT,
    &DOCKWORKER,
    &OUTRUNNER,
    &RUNT,
    &GRUFF,
    &FEDFENNEL,
    &MURLOC,
    &STREAMRUNNER,
    &FORAGER,
    &LURKER,
    &STONETUSK,
    &PORCINE,
    &ROCKHIDE,
    &PRINCESS,
    &BEAR,
    &DOCKMASTER,
    &FOREST_SPIDER,
    &MINE_SPIDER,
    &MOTHER_FANG,
    &ROGUE_WIZARD,
    &SURENA,
    &GEOMANCER,
    &BODYGUARD,
    &AMBUSHER,
    &GARRICK,
    &HOGGER,
];

/// The species of a creature entity, with the attack target chosen for its actual display.
#[derive(Component, Debug, Clone, Copy)]
pub struct Species(pub &'static SpeciesDef, &'static str);

impl Species {
    pub fn new(def: &'static SpeciesDef) -> Self {
        Self(def, def.kit)
    }

    /// Select only a registered target; a female body must never receive male bone indices.
    pub fn for_model(
        def: &'static SpeciesDef,
        default_model: &str,
        actual_model: &str,
    ) -> Option<Self> {
        let path = |p: &str| {
            p.replace('/', "\\")
                .to_ascii_lowercase()
                .trim_end_matches(".mdx")
                .trim_end_matches(".m2")
                .to_string()
        };
        let (default, actual) = (path(default_model), path(actual_model));
        if default == actual {
            return Some(Self::new(def));
        }
        match (def.kit, actual.as_str()) {
            ("thug", "character\\human\\female\\humanfemale")
                if default == "character\\human\\male\\humanmale" =>
            {
                Some(Self(def, "thug_female"))
            }
            ("human_bandit", "character\\human\\female\\humanfemale")
                if default == "character\\human\\male\\humanmale" =>
            {
                Some(Self(def, "human_bandit_female"))
            }
            ("caster", "character\\human\\female\\humanfemale")
                if default == "character\\human\\male\\humanmale" =>
            {
                Some(Self(def, "caster_female"))
            }
            _ => None,
        }
    }

    pub fn kit(&self) -> &'static str {
        self.1
    }
}

/// The species called `name`.
pub fn by_name(name: &str) -> Option<&'static SpeciesDef> {
    ROSTER.iter().copied().find(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_actual_female_display_selects_its_target_without_changing_species_identity() {
        let male = "Character\\Human\\Male\\HumanMale.mdx";
        let female = "Character/Human/Female/HumanFemale.m2";
        let selected = Species::for_model(&THUG, male, female).unwrap();
        assert_eq!(selected.kit(), "thug_female");
        assert_eq!(selected.0.name, "thug");
        assert_eq!(crate::world::foe_entry(selected.0.name), Some(38));
        assert_eq!(selected.0.weapon, THUG.weapon);
        assert_eq!(Species::for_model(&THUG, male, male).unwrap().kit(), "thug");
        for def in [&BANDIT, &DOCKWORKER] {
            assert_eq!(
                Species::for_model(def, male, female).unwrap().kit(),
                "human_bandit_female"
            );
        }
        let morgaine = Species::for_model(&MORGAINE, female, female).unwrap();
        assert_eq!(morgaine.kit(), "morgaine");
        assert_eq!(morgaine.0.name, "morgaine");
        assert!(Species::for_model(&MORGAINE, female, male).is_none());
        assert!(Species::for_model(&GARRICK, male, female).is_none());
        assert!(Species::for_model(&THUG, male, "Creature\\Goblin\\Goblin.m2").is_none());
        assert!(Species::for_model(&WOLF, "Creature\\Wolf\\Wolf.m2", female).is_none());
    }

    #[test]
    fn species_names_are_distinct_and_found_by_name() {
        let mut names: Vec<_> = ROSTER.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ROSTER.len());
        assert_eq!(by_name("kobold").map(|s| s.display), Some(365));
        assert!(by_name("dragon").is_none());
    }

    /// `creature_template.level_max` of each mapped Northshire/Elwynn entry.
    #[test]
    fn every_species_carries_its_database_level() {
        let levels = [
            ("kobold", 4),
            ("vermin", 2),
            ("worker", 3),
            ("miner", 7),
            ("tunneler", 6),
            ("goldtooth", 8),
            ("ruklar", 8),
            ("narg", 10),
            ("wolf", 2),
            ("young_wolf", 1),
            ("mangy_wolf", 6),
            ("gray_forest_wolf", 8),
            ("prowler", 10),
            ("thug", 4),
            ("thuros", 11),
            ("cutpurse", 6),
            ("morgaine", 10),
            ("morgan", 10),
            ("dead_tooth_jack", 11),
            ("james_clark", 12),
            ("erlan", 8),
            ("bandit", 9),
            ("dockworker", 9),
            ("outrunner", 10),
            ("runt", 9),
            ("gruff", 12),
            ("fedfennel", 12),
            ("murloc", 7),
            ("streamrunner", 7),
            ("forager", 10),
            ("lurker", 10),
            ("stonetusk", 6),
            ("porcine", 7),
            ("rockhide", 8),
            ("princess", 9),
            ("bear", 9),
            ("dockmaster", 10),
            ("forest_spider", 6),
            ("mine_spider", 9),
            ("mother_fang", 10),
            ("rogue_wizard", 10),
            ("geomancer", 8),
            ("bodyguard", 10),
            ("ambusher", 9),
            ("surena", 9),
            ("garrick", 5),
            ("hogger", 11),
        ];
        for (name, level) in levels {
            assert_eq!(by_name(name).expect(name).level, level, "{name}");
        }
        assert_eq!(levels.len(), ROSTER.len(), "a species without a level row");
    }

    #[test]
    fn the_specials_fight_within_the_elite_budget_of_their_installed_kits() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source");
        let roster = crate::monster::load(&dir).expect("the installed animation cache loads");
        for name in BOSS_BAR_SPECIES {
            let def = by_name(name).unwrap();
            let kit = roster.get(def.kit).expect(def.kit);
            // Rule R1's Elite: at most five attacks, flinch level 1 or 2.
            assert!(
                kit.attacks.len() <= 5,
                "{name} has {} attacks",
                kit.attacks.len()
            );
            for a in &kit.attacks {
                assert!(
                    (1..=2).contains(&a.spec.level),
                    "{name}'s {} is level {}",
                    a.spec.name,
                    a.spec.level
                );
            }
            for row in def.table {
                assert!(
                    kit.attack_index(row.name).is_some(),
                    "{name}'s kit has no {}",
                    row.name
                );
            }
        }
    }

    #[test]
    fn elwynn_kits_stay_within_the_common_single_hit_attack_budget() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source");
        let roster = crate::monster::load(&dir).expect("the installed animation cache loads");
        for def in [
            &MINER,
            &TUNNELER,
            &MANGY_WOLF,
            &GRAY_FOREST_WOLF,
            &PROWLER,
            &CUTPURSE,
            &BANDIT,
            &DOCKWORKER,
            &OUTRUNNER,
            &RUNT,
            &MURLOC,
            &STREAMRUNNER,
            &FORAGER,
            &LURKER,
            &STONETUSK,
            &PORCINE,
            &ROCKHIDE,
            &BEAR,
            &DOCKMASTER,
            &FOREST_SPIDER,
            &MINE_SPIDER,
        ] {
            let kit = roster.get(def.kit).expect(def.kit);
            let expected: &[&str] = if def.name == "prowler" {
                &["bite"]
            } else if def.name == "ruklar" {
                &["big"]
            } else if def.kit == "kobold" {
                &["small_1", "small_2"]
            } else if matches!(def.kit, "thug" | "thug_female") {
                &["slash_1", "slash_2"]
            } else if def.kit == "human_axe_2h" {
                &["axe_1", "axe_2"]
            } else if def.kit == "human_bandit" {
                &["sword", "kick"]
            } else if matches!(def.kit, "thug" | "goblin") {
                &["slash_1", "slash_2"]
            } else if def.kit == "gnoll" {
                &["swing_1", "swing_2"]
            } else if def.kit == "murloc" {
                &["lunge", "flurry"]
            } else if def.kit == "tarantula" {
                // The hairy spiders' own: the bite, the spit and the hop back (no leap, no latch).
                &["bite", "spit", "hop_back"]
            } else if def.kit == "minespider" {
                // The Skirmish tier: two blows and the hop back, which strikes nothing.
                &["bite", "leap", "hop_back"]
            } else if matches!(def.kit, "caster" | "caster_female") {
                &["bolt", "swat"]
            } else if def.kit == "ambusher" {
                &["slash_1", "slash_2"]
            } else if def.kit == "geomancer" {
                &["bolt", "small_1"]
            } else if def.kit == "boar" {
                &["gore", "charge"]
            } else if def.kit == "bear" {
                &["bite", "swipe"]
            } else {
                &["bite", "lunge"]
            };
            let limit = if matches!(def.kit, "tarantula" | "minespider") {
                3
            } else {
                2
            };
            assert!(
                def.table.len() <= limit,
                "{} exceeds its tier's limit",
                def.name
            );
            assert_eq!(
                def.table.len(),
                expected.len(),
                "{} selected moves",
                def.name
            );
            for (row, name) in def.table.iter().zip(expected) {
                assert_eq!(row.name, *name, "{} selected another attack", def.name);
                let attack = &kit.attacks[kit.attack_index(row.name).expect(row.name)];
                assert!(
                    (1..=2).contains(&attack.spec.level),
                    "{} / {} exceeds the Common flinch level",
                    def.name,
                    row.name
                );
                assert_eq!(
                    attack.spec.level,
                    if matches!(def.name, "ruklar" | "erlan") {
                        2
                    } else {
                        1
                    },
                    "{} / {} gameplay flinch level",
                    def.name,
                    row.name
                );
                // The murlocs' flurry is three light swings (the extractor's Frenzy tier, 2026-10-18).
                let hits = if (def.kit, row.name) == ("murloc", "flurry") {
                    3
                } else {
                    1
                };
                assert_eq!(attack.spec.hits.len(), hits, "{} / {}", def.name, row.name);
                assert_eq!(
                    attack.wow.parents.len(),
                    match def.kit {
                        "kobold" => 72,
                        "direwolf" => 61,
                        "thug" | "human_axe_2h" | "human_bandit" => 119,
                        "thug_female" => 117,
                        "caster" => 119,
                        "caster_female" => 117,
                        "geomancer" => 72,
                        "ambusher" => 64,
                        "gnoll" => 60,
                        "murloc" => 62,
                        "boar" => 57,
                        "bear" => 54,
                        "goblin" => 111,
                        "tarantula" => 74,
                        "minespider" => 91,
                        _ => 64,
                    },
                    "{} must use its original model rig",
                    def.name
                );
            }
        }
    }

    #[test]
    fn rank_rules_follow_the_spec_table() {
        assert!(!Rank::Normal.never_respawns() && !Rank::Normal.first_kill_best());
        assert!(!Rank::Rare.never_respawns() && Rank::Rare.first_kill_best());
        assert!(Rank::Miniboss.never_respawns() && !Rank::Miniboss.first_kill_best());
        assert!(Rank::Boss.never_respawns() && Rank::Boss.first_kill_best());
    }

    #[test]
    fn the_taurus_switch_brings_back_the_whole_old_hogger() {
        // The knight by default.
        assert_eq!(resolve_with(&HOGGER, None).kit, "hogger_knight");
        assert_eq!(resolve_with(&HOGGER, Some("knight")).kit, "hogger_knight");
        let old = resolve_with(&HOGGER, Some("taurus"));
        assert_eq!(old.name, "hogger");
        assert_eq!(old.kit, "hogger");
        let names = |t: &[crate::pattern::Row]| t.iter().map(|r| r.name).collect::<Vec<_>>();
        assert_eq!(names(old.table), names(crate::pattern::HOGGER));
        assert!(old.left_handed && old.tempo > 1.0);
        assert_eq!(
            crate::pattern::styles(old.kit, false),
            crate::pattern::HOGGER_STYLE
        );
        // Nobody else is swapped.
        assert_eq!(resolve_with(&KOBOLD, Some("taurus")).kit, "kobold");
        assert!(!ROSTER.iter().any(|s| s.kit == HOGGER_TAURUS.kit));
    }

    #[test]
    fn hogger_the_knight_is_a_right_handed_two_phase_boss() {
        let h = by_name("hogger").expect("hogger");
        assert_eq!(h.kit, "hogger_knight");
        assert!(!h.left_handed);
        assert_eq!(h.display, 384);
        // The knight's clips are fast already: no quicker clock.
        assert!(h.size > 1.0 && h.tempo == 1.0);
        let two = h.phase_two.as_ref().expect("a second phase");
        assert!(two.chase > 1.0 && two.cooldown < 1.0);
        let moves = [
            "slash",
            "step_slash",
            "close_two",
            "pounce",
            "lunge",
            "thrust",
            "spin",
            "backflip",
            "head_butt",
            "flip_slam",
            "overhead",
        ];
        for r in h.table.iter().chain(two.table) {
            assert!(moves.contains(&r.name), "{}", r.name);
        }
        // The power-up is phase two's, never a row.
        assert!(h.table.iter().chain(two.table).all(|r| r.name != "frenzy"));
    }

    #[test]
    fn hogger_is_a_left_handed_two_phase_boss_with_an_axe() {
        let h = &HOGGER_TAURUS;
        assert!(h.left_handed);
        assert_eq!(h.weapon, Some(7428));
        assert_eq!(h.display, 384);
        // Drawn larger than the client's model and fought at a quicker clip clock, always.
        assert!(h.size > 1.0 && h.tempo > 1.0);
        assert!(ROSTER
            .iter()
            .filter(|s| s.name != "hogger")
            .all(|s| s.size == 1.0
                && s.tempo == 1.0
                // The boars too stand up soon after a blow.
                && (s.recovery.is_none() || s.behavior == crate::behavior::Role::BoarCharge)));
        let two = h.phase_two.as_ref().expect("a second phase");
        assert!(two.tempo > 1.0 && two.chase > 1.0 && two.cooldown < 1.0);
        // Every row of both tables names a move of the kit.
        let moves = [
            "overhead",
            "heavy_swing",
            "step_smash",
            "sweep",
            "lunge_smash",
            "quick",
            "backhand",
            "feint_plunge",
            "charge",
            "hop_back",
        ];
        for r in h.table.iter().chain(two.table) {
            assert!(moves.contains(&r.name), "{}", r.name);
        }
        // The other species have neither.
        assert!(ROSTER
            .iter()
            .filter(|s| s.name != "hogger")
            .all(|s| !s.left_handed && s.phase_two.is_none()));
    }

    #[test]
    fn the_rares_and_named_wear_the_boss_bar_without_becoming_bosses() {
        for name in BOSS_BAR_SPECIES {
            let def = by_name(name).unwrap_or_else(|| panic!("{name} is not in the roster"));
            assert!(shows_boss_bar(def), "{name}");
            // The bar only: a quest mob still respawns.
            assert_ne!(def.rank, Rank::Boss, "{name}");
        }
        assert_eq!(BOSS_BAR_SPECIES.len(), 14);
        assert!(shows_boss_bar(by_name("garrick").unwrap()));
        assert!(shows_boss_bar(by_name("hogger").unwrap()));
        assert!(!shows_boss_bar(by_name("kobold").unwrap()));
        assert!(!shows_boss_bar(by_name("outrunner").unwrap()));
    }

    #[test]
    fn the_zone_bosses_are_garrick_and_hogger() {
        let bosses: Vec<_> = ROSTER
            .iter()
            .filter(|s| s.rank == Rank::Boss)
            .map(|s| s.name)
            .collect();
        assert_eq!(bosses, vec!["garrick", "hogger"]);
        assert!(ROSTER
            .iter()
            .all(|s| matches!(s.rank, Rank::Boss | Rank::Normal | Rank::Rare)));
        let rares: Vec<_> = ROSTER
            .iter()
            .filter(|s| s.rank == Rank::Rare)
            .map(|s| s.name)
            .collect();
        assert_eq!(
            rares,
            vec![
                "narg",
                "thuros",
                "morgaine",
                "gruff",
                "fedfennel",
                "mother_fang"
            ]
        );
    }

    #[test]
    fn the_rank_switch_parses_names_and_ignores_garbage() {
        assert_eq!(Rank::parse("boss"), Some(Rank::Boss));
        assert_eq!(Rank::parse(" Rare "), Some(Rank::Rare));
        assert_eq!(Rank::parse("miniboss"), Some(Rank::Miniboss));
        assert_eq!(Rank::parse("normal"), Some(Rank::Normal));
        assert_eq!(Rank::parse("elite"), None);
        let map = parse_rank_overrides("kobold:boss, wolf:rare ,bad,thug:nope");
        assert_eq!(map.get("kobold"), Some(&Rank::Boss));
        assert_eq!(map.get("wolf"), Some(&Rank::Rare));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn a_species_fights_with_a_kit_and_never_by_a_move_its_kit_lacks() {
        for s in ROSTER {
            assert!(
                matches!(
                    s.kit,
                    "kobold"
                        | "thuros"
                        | "morgaine"
                        | "morgan"
                        | "dead_tooth_jack"
                        | "james_clark"
                        | "erlan"
                        | "surena"
                        | "goldtooth"
                        | "ruklar"
                        | "narg"
                        | "gruff"
                        | "fedfennel"
                        | "princess"
                        | "mother_fang"
                        | "hogger_knight"
                        | "wolf"
                        | "direwolf"
                        | "thug"
                        | "thug_female"
                        | "human_axe_2h"
                        | "human_bandit"
                        | "gnoll"
                        | "murloc"
                        | "boar"
                        | "bear"
                        | "goblin"
                        | "tarantula"
                        | "minespider"
                        | "caster"
                        | "caster_female"
                        | "geomancer"
                        | "ambusher"
                        | "garrick"
                        | "hogger"
                ),
                "{} has kit {}",
                s.name,
                s.kit
            );
            // A boss may hit harder than the Dark Souls numbers it borrows; no one else. Hogger the knight: x2.2 on
            // Artorias' lighter rows (phys 140-250) is the Taurus kit's raw weight (phys 280-310 x1.5).
            let ceiling = if s.rank == Rank::Boss && s.name == "hogger" {
                2.2
            } else {
                1.0
            };
            assert!(
                s.damage_scale > 0.0 && s.damage_scale <= ceiling,
                "{}",
                s.name
            );
        }
        assert_eq!(by_name("vermin").map(|s| s.kit), Some("kobold"));
        assert_eq!(by_name("young_wolf").map(|s| s.kit), Some("wolf"));
    }

    #[test]
    fn a_creature_can_strike_from_where_it_stops_chasing() {
        // The Young Wolf once stopped at 1.3 yd with a bite that reached 0.95: it stood beside the
        // player and never attacked.
        for s in ROSTER {
            assert!(
                s.table.iter().any(|r| r.weight > 0
                    && r.band.0 <= s.stop_distance
                    && s.stop_distance <= r.band.1),
                "the {} stops at {} yd, inside no opening move's band",
                s.name,
                s.stop_distance
            );
        }
    }

    #[test]
    fn every_species_walks_between_a_crawl_and_the_players_walk() {
        for s in ROSTER {
            let w = s.walk_speed();
            assert!((0.9..=2.5).contains(&w), "the {} walks {w} yd/s", s.name);
            assert!(
                w < s.chase_speed,
                "the {} walks slower than it chases",
                s.name
            );
            // Above the gait's walk line, so roaming plays the walk clip, not the stand.
            assert!(w > 0.6, "{}", s.name);
        }
        assert!((KOBOLD.walk_speed() - 1.8).abs() < 1e-6);
    }

    #[test]
    fn every_species_has_a_pattern_and_sane_numbers() {
        for s in ROSTER {
            assert!(!s.table.is_empty(), "{} has no pattern", s.name);
            assert!(s.health > 0.0 && s.chase_speed > 0.0, "{}", s.name);
            assert!(s.collision.0 > 0.0 && s.collision.1 > 0.0, "{}", s.name);
        }
    }
}
