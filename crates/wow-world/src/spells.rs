//! What the Mage's four spells do, in WoW 1.12's rank 1 numbers (vMaNGOS `spell_template`, build 5875,
//! and the client's `SpellCastTimes` / `SpellDuration` / `SpellRange` rows read 2026-10-14). Pure data
//! and rules; the game's `spells` module delivers them. Spec
//! `docs/superpowers/specs/2026-10-14-mage-class-design.md`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum School {
    Fire,
    Frost,
}

/// How a spell reaches its target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Delivery {
    /// A missile at this speed (yd/s), homing on the target.
    Missile { speed: f32 },
    /// A hit on a target in range, no flight.
    Instant,
    /// A buff on the caster.
    SelfBuff,
}

/// Damage over time after the hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dot {
    pub per_tick: f32,
    pub ticks: u32,
    pub every: f32,
}

/// A movement slow after the hit: `share` of the speed is taken for `secs`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slow {
    pub share: f32,
    pub secs: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpellDef {
    pub spell: u32,
    pub school: School,
    pub delivery: Delivery,
    /// The inclusive range one cast rolls (0, 0 for a buff).
    pub damage: (u32, u32),
    pub dot: Option<Dot>,
    pub slow: Option<Slow>,
    /// Armor a buff adds, and for how long (s).
    pub armor: u32,
    pub secs: f32,
    /// How far it reaches (yd).
    pub range: f32,
    /// Seconds before it can be cast again.
    pub cooldown: f32,
}

pub const SPELLS: [SpellDef; 4] = [
    // Fireball: `SpellRange` 35 yd, speed 24, 14-22 fire and 2 over 4 s (periodic 1 every 2 s).
    SpellDef {
        spell: 133,
        school: School::Fire,
        delivery: Delivery::Missile { speed: 24.0 },
        damage: (14, 22),
        dot: Some(Dot {
            per_tick: 1.0,
            ticks: 2,
            every: 2.0,
        }),
        slow: None,
        armor: 0,
        secs: 0.0,
        range: 35.0,
        cooldown: 0.0,
    },
    // Frostbolt: `SpellRange` 30 yd, speed 28, 18-20 frost, 40 % slower for 5 s (base -41, die 1).
    SpellDef {
        spell: 116,
        school: School::Frost,
        delivery: Delivery::Missile { speed: 28.0 },
        damage: (18, 20),
        dot: None,
        slow: Some(Slow {
            share: 0.40,
            secs: 5.0,
        }),
        armor: 0,
        secs: 0.0,
        range: 30.0,
        cooldown: 0.0,
    },
    // Fire Blast: instant, 20 yd, 24-32 fire, 8 s cooldown (`categoryRecoveryTime` 8000).
    SpellDef {
        spell: 2136,
        school: School::Fire,
        delivery: Delivery::Instant,
        damage: (24, 32),
        dot: None,
        slow: None,
        armor: 0,
        secs: 0.0,
        range: 20.0,
        cooldown: 8.0,
    },
    // Frost Armor: +30 armor for 30 minutes (`SpellDuration` 30).
    SpellDef {
        spell: 168,
        school: School::Frost,
        delivery: Delivery::SelfBuff,
        damage: (0, 0),
        dot: None,
        slow: None,
        armor: 30,
        secs: 1800.0,
        range: 0.0,
        cooldown: 0.0,
    },
];

pub fn def(spell: u32) -> Option<&'static SpellDef> {
    SPELLS.iter().find(|s| s.spell == spell)
}

/// One cast's damage: a whole number in the spell's range, `u` in `0..1` picking it.
pub fn roll(def: &SpellDef, u: f32) -> f32 {
    let (lo, hi) = def.damage;
    if hi <= lo {
        return lo as f32;
    }
    let span = (hi - lo + 1) as f32;
    (lo as f32 + (span * u.clamp(0.0, 0.999_999)).floor()).min(hi as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_spells_are_rank_one_as_the_database_has_them() {
        let f = def(133).unwrap();
        assert_eq!(f.damage, (14, 22), "Fireball: basePoints 13, die 9");
        assert_eq!(f.delivery, Delivery::Missile { speed: 24.0 });
        assert_eq!((f.range, f.cooldown), (35.0, 0.0));
        let d = f.dot.unwrap();
        assert_eq!(
            (d.per_tick, d.ticks, d.every),
            (1.0, 2, 2.0),
            "2 fire damage over 4 s"
        );
        let b = def(116).unwrap();
        assert_eq!(b.damage, (18, 20), "Frostbolt: basePoints 17, die 3");
        assert_eq!(b.delivery, Delivery::Missile { speed: 28.0 });
        assert_eq!(b.range, 30.0);
        let s = b.slow.unwrap();
        assert_eq!(
            (s.share, s.secs),
            (0.40, 5.0),
            "-41 base points + the 1..1 die = -40, SpellDuration 28 = 5 s"
        );
        let x = def(2136).unwrap();
        assert_eq!(x.damage, (24, 32), "Fire Blast: basePoints 23, die 9");
        assert_eq!(
            (x.delivery, x.range, x.cooldown),
            (Delivery::Instant, 20.0, 8.0)
        );
        let a = def(168).unwrap();
        assert_eq!(
            (a.delivery, a.armor, a.secs),
            (Delivery::SelfBuff, 30, 1800.0)
        );
        assert!(def(100).is_none(), "Charge is a warrior's");
    }

    #[test]
    fn the_damage_roll_covers_the_whole_range_and_nothing_else() {
        let f = def(133).unwrap();
        assert_eq!(roll(f, 0.0), 14.0);
        assert_eq!(roll(f, 0.999_999), 22.0);
        assert_eq!(roll(f, 0.5), 18.0);
        for i in 0..=100 {
            let r = roll(f, i as f32 / 100.0);
            assert!((14.0..=22.0).contains(&r) && r.fract() == 0.0, "{r}");
        }
        assert_eq!(roll(def(168).unwrap(), 0.7), 0.0, "a buff deals nothing");
    }

    #[test]
    fn every_spell_of_the_mage_class_has_a_definition() {
        for s in crate::class::MAGE.spells {
            assert!(def(*s).is_some(), "{s}");
        }
    }
}
