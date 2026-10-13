//! The game's creatures as data: which WoW model one wears, how much health it has, how it
//! moves and which pattern table it fights by. The Dark Souls side of a creature (its moves, flinches
//! and deaths) is its kit in [`crate::monster::Roster`], keyed by the species' `kit`, which is the
//! kit's name in `cache/dsr/manifest.txt` and the folder of its clips. Several species may share a
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

/// How special a creature is. The database cannot say (every valley creature, Garrick included, is
/// rank 0 there), so the species row does.
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

/// One kind of creature.
pub struct SpeciesDef {
    /// The species' own name (`vermin`): the log lines, `WOWSOULS_DSVIEW=<name>:<clip>` and the
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
}

/// Marks the creature of a [`Rank::Boss`] species: it gets the Dark Souls boss bar.
#[derive(Component, Debug, Clone, Copy)]
pub struct Boss;

impl SpeciesDef {
    /// How fast it walks when nothing is after it (yd/s): the chase speed scaled down the way WoW's
    /// walk is slower than its run, never slower than a crawl and never faster than the player's walk.
    pub fn walk_speed(&self) -> f32 {
        (self.chase_speed * 0.45).clamp(0.9, crate::motor::WALK_SPEED)
    }
}

impl std::fmt::Debug for SpeciesDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Species({})", self.name)
    }
}

/// `CreatureDisplayInfo` 365: the kobold model at 0.85 scale with the normal skin; Dark Souls'
/// Blighttown ghoul (`c2060`) gives it its moves. Health: a few light attacks. Collision from
/// `CreatureModelData` 26: 1.389 high, 0.6944 wide.
pub const KOBOLD: SpeciesDef = SpeciesDef {
    name: "kobold",
    kit: "kobold",
    title: "Kobold",
    level: 4,
    display: 365,
    look: None,
    weapon: None,
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
};

/// `CreatureDisplayInfo` 604: the Timber Wolf (classicdb.ch, npc 69: the wolf inside Northshire
/// Valley), the wolf model (`Creature\Wolf\Wolf.m2`) at 0.55 scale with the timber skin; its run clip
/// covers 6.94 yd/s at scale 1, so 3.8 at this one. Dark Souls' black dog (`c3340`) gives it two
/// moves. Health: two light attacks. Collision: the model's data says 2.083 high, which is a tall
/// wolf; the wolf stands 1.3 yd at the ears.
pub const WOLF: SpeciesDef = SpeciesDef {
    name: "wolf",
    kit: "wolf",
    title: "Timber Wolf",
    level: 2,
    display: 604,
    look: None,
    weapon: None,
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
};

/// The Defias Thug, the Defias of Northshire Valley (with Garrick Padfoot): `CreatureDisplayInfo`
/// 5035 (classicdb.ch, npc 38), a human male whose `CreatureDisplayInfoExtra` row 883 dresses him
/// (baked body atlas, bandana helm, chest, belt, legs, boots), with a copper dagger
/// (`ItemDisplayInfo` 2704, `Knife_1H_Dagger_A_01`) in his hand. Dark Souls' undead thief (`c2520`)
/// gives him two moves. Health: a few light attacks; he chases a little faster than a kobold. The
/// collision is the player's own capsule.
pub const THUG: SpeciesDef = SpeciesDef {
    name: "thug",
    kit: "thug",
    title: "Defias Thug",
    level: 4,
    display: 5035,
    look: None,
    weapon: Some(2704),
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
};

/// `CreatureDisplayInfo` 10913: the Kobold Vermin (classicdb.ch, npc 6; level 1-2, 42-55 health in
/// the sources): the kobold model at 0.7 scale, the normal skin. The weakest creature of the game:
/// the ghoul's two light swings only, a smaller body (bands and chase speed scaled by 0.7 / 0.85), two
/// light attacks of health and 70 % of the kit's damage.
pub const VERMIN: SpeciesDef = SpeciesDef {
    name: "vermin",
    kit: "kobold",
    title: "Kobold Vermin",
    level: 2,
    display: 10913,
    look: None,
    weapon: None,
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
};

/// `CreatureDisplayInfo` 10912: the Kobold Worker (classicdb.ch, npc 257; level 3, 71 health in the
/// sources): the kobold model at 0.85 with the albino skin. The laborer's size and health, without
/// its charges: the two light swings and the big one, 90 % of the kit's damage.
pub const WORKER: SpeciesDef = SpeciesDef {
    name: "worker",
    kit: "kobold",
    title: "Kobold Worker",
    level: 3,
    display: 10912,
    look: None,
    weapon: None,
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
};

/// `CreatureDisplayInfo` 447: the Young Wolf (classicdb.ch, npc 299; level 1, 42 health in the
/// sources; the valley's before Cataclysm, by the wiki): the wolf model at 0.4 scale with the timber
/// skin. The black dog's bite only, a smaller body (the bite band scaled by 0.4 / 0.55), two light
/// attacks of health (30), its run clip's 6.94 yd/s at this scale (2.8) and 70 % of the kit's damage.
pub const YOUNG_WOLF: SpeciesDef = SpeciesDef {
    name: "young_wolf",
    kit: "wolf",
    title: "Young Wolf",
    level: 1,
    display: 447,
    look: None,
    weapon: None,
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
};

/// Garrick Padfoot, the valley's named Defias (classicdb.ch, npc 103: level 5, 102 HP in the sources):
/// `CreatureDisplayInfo` 2073, a human male whose extra row 676 dresses him (red outfit, bandana
/// helm, shirt, belt, legs, boots, gloves), with the thug's copper dagger (the client has no creature
/// equipment; a guess). Dark Souls' Anor Londo assassin (`c2400`, [`crate::monster`] kit `garrick`)
/// gives him five moves: a quick poke, a double and a triple, a lunge and a dash. Health: nine light
/// attacks (the sources' 102 is an ordinary creature's two to three times over); poise 20, so three
/// light blows or one heavy one break his swing; 80 % of the kit's damage, since a triple lands three.
pub const GARRICK: SpeciesDef = SpeciesDef {
    name: "garrick",
    kit: "garrick",
    title: "Garrick Padfoot",
    level: 5,
    display: 2073,
    look: None,
    weapon: Some(2704),
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
};

/// Every species the game spawns.
pub const ROSTER: &[&SpeciesDef] = &[
    &KOBOLD,
    &VERMIN,
    &WORKER,
    &WOLF,
    &YOUNG_WOLF,
    &THUG,
    &GARRICK,
];

/// The species of a creature entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct Species(pub &'static SpeciesDef);

/// The species called `name`.
pub fn by_name(name: &str) -> Option<&'static SpeciesDef> {
    ROSTER.iter().copied().find(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_names_are_distinct_and_found_by_name() {
        let mut names: Vec<_> = ROSTER.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ROSTER.len());
        assert_eq!(by_name("kobold").map(|s| s.display), Some(365));
        assert!(by_name("dragon").is_none());
    }

    /// `creature_template.level_max` of each entry in `cache/world/northshire.txt`:
    /// Kobold Laborer (80) 4, Vermin (6) 2, Worker (257) 3, Timber Wolf (69) 2, Young Wolf (299) 1,
    /// Defias Thug (38) 4, Garrick Padfoot (103) 5.
    #[test]
    fn every_species_carries_its_database_level() {
        let levels = [
            ("kobold", 4),
            ("vermin", 2),
            ("worker", 3),
            ("wolf", 2),
            ("young_wolf", 1),
            ("thug", 4),
            ("garrick", 5),
        ];
        for (name, level) in levels {
            assert_eq!(by_name(name).expect(name).level, level, "{name}");
        }
        assert_eq!(levels.len(), ROSTER.len(), "a species without a level row");
    }

    #[test]
    fn rank_rules_follow_the_spec_table() {
        assert!(!Rank::Normal.never_respawns() && !Rank::Normal.first_kill_best());
        assert!(!Rank::Rare.never_respawns() && Rank::Rare.first_kill_best());
        assert!(Rank::Miniboss.never_respawns() && !Rank::Miniboss.first_kill_best());
        assert!(Rank::Boss.never_respawns() && Rank::Boss.first_kill_best());
    }

    #[test]
    fn garrick_is_the_valleys_one_boss() {
        let bosses: Vec<_> = ROSTER
            .iter()
            .filter(|s| s.rank == Rank::Boss)
            .map(|s| s.name)
            .collect();
        assert_eq!(bosses, vec!["garrick"]);
        assert!(ROSTER
            .iter()
            .all(|s| s.rank == Rank::Boss || s.rank == Rank::Normal));
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
                matches!(s.kit, "kobold" | "wolf" | "thug" | "garrick"),
                "{} has kit {}",
                s.name,
                s.kit
            );
            assert!(s.damage_scale > 0.0 && s.damage_scale <= 1.0, "{}", s.name);
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
