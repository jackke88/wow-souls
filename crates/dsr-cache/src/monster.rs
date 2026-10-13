//! A creature's kit: which Dark Souls animation is which move, flinch and death, for one
//! creature of the game. Hand-made, like the bone map and the weapon classes. The ghoul's
//! animation files are `aXX_YYYY` (TAE id `XX × 10000 + YYYY`); the sword kit is `a00_3xxx`.

use anyhow::{ensure, Result};
use glam::Vec3;

use crate::bonemap::{self, Rig};
use crate::params::Params;
use crate::retarget::Retargeted;

/// How many of its Dark Souls creature's attacks a game creature may borrow (the user's rule R1,
/// 2026-10-09: an ordinary creature that wears a tough enemy's moves gets one or two of them, so
/// nothing ordinary is far harder than its WoW self). A kit over its tier is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// `Common` and `Elite` are worn by the wolf, the thug, the geomancer and Garrick, who arrive one
// by one (spec 2026-10-09-enemy-roster-design.md); the rule is built and tested before them.
#[allow(dead_code)]
pub enum Tier {
    /// At most 2 attacks, each a single hit window of flinch level 1 or 2.
    Common,
    /// A miniboss: at most 5 attacks, hit windows may repeat, flinch level 1 or 2.
    Elite,
    /// The kobold as it was built before the rule: its 6 attacks, no more.
    Legacy,
}

impl Tier {
    pub fn max_attacks(self) -> usize {
        match self {
            Tier::Common => 2,
            Tier::Elite => 5,
            Tier::Legacy => 6,
        }
    }

    /// Whether one attack fits the tier: its flinch level (`AtkParam_Npc.dmgLevel`) and how many
    /// hit windows its clip carries.
    pub fn check_attack(self, name: &str, level: i64, windows: usize) -> Result<()> {
        if self == Tier::Legacy {
            return Ok(());
        }
        ensure!(
            (1..=2).contains(&level),
            "{name}: flinch level {level} is too hard for a {self:?} creature (1 or 2)"
        );
        ensure!(
            self == Tier::Elite || windows == 1,
            "{name}: {windows} hit windows; a {self:?} creature's attacks have one"
        );
        Ok(())
    }
}

/// What a death must look like by its last frame: the body (every driven bone, lowest to highest)
/// ends less than `body` of its standing height, and the pelvis moves less than `settle` yd over
/// the last ten frames.
#[derive(Debug, Clone, Copy)]
pub struct Lying {
    pub body: f32,
    pub settle: f32,
}

/// A biped lies flat: under half its height.
pub const LYING_BIPED: Lying = Lying {
    body: 0.5,
    settle: 0.02,
};

/// The assassin's deaths settle a little slower than the others' (`a00_2204` still moves 0.024 yd over
/// its last ten frames, `a00_2205` 0.017): a biped lying flat, with the settle limit of 0.03.
pub const LYING_ASSASSIN: Lying = Lying {
    body: 0.5,
    settle: 0.03,
};

/// A quadruped on its side is still as thick as its body and its raised legs: under 0.65 of its
/// standing height (the wolf's `a00_2205` ends at 0.56), and it may still be settling.
pub const LYING_QUADRUPED: Lying = Lying {
    body: 0.65,
    settle: 0.08,
};

/// One attack: the game's name for it, its clip and the judge id its TAE hit window must carry.
pub struct Attack {
    pub name: &'static str,
    pub clip: &'static str,
    pub judge: i32,
}

/// One flinch clip and the hit level it answers.
pub struct React {
    pub level: u8,
    pub clip: &'static str,
}

pub struct Kit {
    pub name: &'static str,
    pub tier: Tier,
    /// The WoW model the clips are retargeted onto, and its bone table.
    pub wow_model: &'static str,
    pub rig: &'static Rig<'static>,
    pub chr: &'static str,
    /// What its deaths must look like at their end.
    pub lying: Lying,
    /// An `NpcParam` row of the creature, and the behavior variation it must carry.
    pub npc: i32,
    pub variation: i64,
    pub attacks: &'static [Attack],
    pub reacts: &'static [React],
    /// Death clips, the way the head falls decides which one a blow gets (as for flinches).
    pub deaths: &'static [&'static str],
}

/// The kobold wears the Blighttown ghoul's sword kit. Left out: the plunge (`a00_3401`, judge
/// 170), the feigned death (`a00_9061`, 240) and the head throw (judge 150, a bullet).
/// Flinches: `a00_2000`-`2002` are 0.6 s head-back clips, `2004`-`2007` 1.0 s ones; which way
/// each pushes the head is measured ([`push`]), not assumed. Death: `a00_2202` falls on its back
/// (a blow from the front), `a00_2204` on its face (a blow from behind), both end lying; `a00_9000`
/// and its siblings start lying (the feigned death) and are no death.
pub const KOBOLD: Kit = Kit {
    name: "kobold",
    tier: Tier::Legacy,
    wow_model: crate::wow::KOBOLD_M2,
    rig: &bonemap::KOBOLD_RIG,
    chr: "c2060",
    lying: LYING_BIPED,
    npc: 206_000,
    variation: 20_600,
    attacks: &[
        Attack {
            name: "small_1",
            clip: "a00_3000",
            judge: 100,
        },
        Attack {
            name: "small_2",
            clip: "a00_3001",
            judge: 110,
        },
        Attack {
            name: "big",
            clip: "a00_3002",
            judge: 120,
        },
        Attack {
            name: "run_strike",
            clip: "a00_3003",
            judge: 130,
        },
        Attack {
            name: "body_check",
            clip: "a00_3004",
            judge: 140,
        },
        Attack {
            name: "pounce",
            clip: "a00_3006",
            judge: 160,
        },
    ],
    reacts: &[
        React {
            level: 1,
            clip: "a00_2000",
        },
        React {
            level: 1,
            clip: "a00_2001",
        },
        React {
            level: 1,
            clip: "a00_2004",
        },
        React {
            level: 1,
            clip: "a00_2005",
        },
        React {
            level: 1,
            clip: "a00_2006",
        },
        React {
            level: 2,
            clip: "a00_2007",
        },
        React {
            level: 2,
            clip: "a00_2004",
        },
        React {
            level: 2,
            clip: "a00_2005",
        },
        React {
            level: 2,
            clip: "a00_2006",
        },
    ],
    deaths: &["a00_2202", "a00_2204"],
};

/// The wolf wears the black dog's (`c3340`, NpcParam 334000: an ordinary Undead Burg enemy, 84 HP)
/// moves, two of its six (rule R1, [`Tier::Common`]): the readable close bite `a00_3000` (judge 100,
/// hit 0.63-0.87 s, phys 104) and the short lunge `a00_3004` (judge 130, hit 0.33-0.70 s, 2.07 m,
/// phys 100). Left out: `a00_3001` (4.0 m, phys 140), `a00_3002` (5.1 m, 140), `a00_3005` (2.7 m,
/// 140) and `a00_3006` (a pounce whose hit opens 0.20 s in: no time to read it). Flinches:
/// `a00_2000`-`2002` are 0.7 s (level 1), `2004`-`2007` 1.03 s (level 2); the launched ones
/// (`2014`, `2020`) and the turning ones (`2050`-`2053`) are left. Of the dog's four deaths only
/// `a00_2205` (1.37 s: knocked onto its side, ends lying flat) lies down; `a00_2200` and `a00_2209`
/// sit up on the haunches with the forepaws raised and `a00_2207` is thrown 5.9 m (contact sheets
/// 2026-10-09). One death serves every blow; `choose_death` has nothing to choose between.
pub const WOLF: Kit = Kit {
    name: "wolf",
    tier: Tier::Common,
    wow_model: crate::wow::WOLF_M2,
    rig: &bonemap::WOLF_RIG,
    chr: "c3340",
    lying: LYING_QUADRUPED,
    npc: 334_000,
    variation: 33_400,
    attacks: &[
        Attack {
            name: "bite",
            clip: "a00_3000",
            judge: 100,
        },
        Attack {
            name: "lunge",
            clip: "a00_3004",
            judge: 130,
        },
    ],
    reacts: &[
        React {
            level: 1,
            clip: "a00_2000",
        },
        React {
            level: 1,
            clip: "a00_2001",
        },
        React {
            level: 1,
            clip: "a00_2002",
        },
        React {
            level: 2,
            clip: "a00_2004",
        },
        React {
            level: 2,
            clip: "a00_2005",
        },
        React {
            level: 2,
            clip: "a00_2006",
        },
        React {
            level: 2,
            clip: "a00_2007",
        },
    ],
    deaths: &["a00_2205"],
};

/// The Defias Thug (Northshire's Defias, with Garrick) wears the undead thief's (`c2520`, NpcParam
/// 252000, 115 HP) moves, two of its
/// seven (rule R1, [`Tier::Common`]): `a00_3003` (judge 130, hit 0.87-0.97 s of 2.37, phys 150) and
/// `a00_3004` (judge 140, hit 0.73-0.90 s of 2.13, phys 153), both flinch level 1 with one hit
/// window. Left out: `a00_3000`-`3002` (their `dmgLevel` is 8, which the game would read as a
/// knockdown; its meaning is not in the params), `a00_3005` (level 2, a 3.9 m lunge) and `a00_3006`
/// (a thrown bullet). Flinches: `a00_2000`-`2002` 0.77 s (level 1), `2004`-`2007` 1.2 s (level 2).
pub const THUG: Kit = Kit {
    name: "thug",
    tier: Tier::Common,
    wow_model: crate::wow::HUMAN_MALE_M2,
    rig: &bonemap::THIEF_RIG,
    chr: "c2520",
    lying: LYING_BIPED,
    npc: 252_000,
    variation: 25_200,
    attacks: &[
        Attack {
            name: "slash_1",
            clip: "a00_3003",
            judge: 130,
        },
        Attack {
            name: "slash_2",
            clip: "a00_3004",
            judge: 140,
        },
    ],
    reacts: &[
        React {
            level: 1,
            clip: "a00_2000",
        },
        React {
            level: 1,
            clip: "a00_2001",
        },
        React {
            level: 1,
            clip: "a00_2002",
        },
        React {
            level: 2,
            clip: "a00_2004",
        },
        React {
            level: 2,
            clip: "a00_2005",
        },
        React {
            level: 2,
            clip: "a00_2006",
        },
        React {
            level: 2,
            clip: "a00_2007",
        },
    ],
    deaths: &["a00_2204", "a00_2200"],
};

/// Garrick Padfoot wears the Anor Londo assassin's (`c2400`, NpcParam 240000, 110 HP, 300 souls)
/// moves, five of its twelve ([`Tier::Elite`]): `a00_3002` (two hits at 0.53-0.77 and 1.17-1.40 s of
/// 2.77, phys 140), `a00_3003` (three hits, 0.57-0.70, 1.03-1.13 and 1.53-1.67 s of 2.70, phys 155),
/// `a00_3005` (a level 2 lunge, 3.5 m, hit 0.87-1.00 s of 2.30, phys 179), `a00_3006` (level 2, 4.0 m,
/// hit 0.50-0.57 s of 1.67, phys 173) and `a00_3008` (the quick one: 1.37 s, hit 0.33-0.47, phys 105).
/// Left out: `a00_3000` / `3001` (their `dmgLevel` is 8, like the thief's), `3004` (a hop backward),
/// `3007` (level 2, a hop), `3010` / `3011` (a bullet at 1.03 s). The assassin wears the player's
/// bone names, so the player's table fits. Flinches: `a00_2000`-`2002` 0.87 s (level 1),
/// `2004`-`2007` 1.7-1.9 s (level 2).
pub const GARRICK: Kit = Kit {
    name: "garrick",
    tier: Tier::Elite,
    wow_model: crate::wow::HUMAN_MALE_M2,
    rig: &bonemap::HUMAN_MALE_RIG,
    chr: "c2400",
    lying: LYING_ASSASSIN,
    npc: 240_000,
    variation: 24_000,
    attacks: &[
        Attack {
            name: "double",
            clip: "a00_3002",
            judge: 120,
        },
        Attack {
            name: "triple",
            clip: "a00_3003",
            judge: 130,
        },
        Attack {
            name: "lunge",
            clip: "a00_3005",
            judge: 150,
        },
        Attack {
            name: "dash",
            clip: "a00_3006",
            judge: 160,
        },
        Attack {
            name: "quick",
            clip: "a00_3008",
            judge: 180,
        },
    ],
    reacts: &[
        React {
            level: 1,
            clip: "a00_2000",
        },
        React {
            level: 1,
            clip: "a00_2001",
        },
        React {
            level: 1,
            clip: "a00_2002",
        },
        React {
            level: 2,
            clip: "a00_2004",
        },
        React {
            level: 2,
            clip: "a00_2005",
        },
        React {
            level: 2,
            clip: "a00_2006",
        },
        React {
            level: 2,
            clip: "a00_2007",
        },
    ],
    deaths: &["a00_2204", "a00_2205"],
};

/// Every creature of the game that wears a Dark Souls creature's moves.
pub const KITS: &[&Kit] = &[&KOBOLD, &WOLF, &THUG, &GARRICK];

impl Kit {
    /// The `NpcParam` row says what this record says, every attack's judge resolves, and the
    /// attacks fit the kit's [`Tier`], so a kit recorded wrong, or one that borrows too much,
    /// cannot pass.
    pub fn check(&self, params: &Params) -> Result<()> {
        let n = params.npc(self.npc)?;
        ensure!(
            n.variation == self.variation,
            "NpcParam {} has behavior variation {}; the {} record says {}",
            self.npc,
            n.variation,
            self.name,
            self.variation
        );
        ensure!(
            self.attacks.len() <= self.tier.max_attacks(),
            "{} borrows {} attacks; a {:?} creature gets at most {}",
            self.name,
            self.attacks.len(),
            self.tier,
            self.tier.max_attacks()
        );
        for a in self.attacks {
            let row = params
                .npc_attack(self.variation, a.judge)
                .map_err(|e| e.context(format!("attack {} ({})", a.name, a.clip)))?;
            // The windows are counted on the clip, in the extractor; one is assumed here.
            self.tier.check_attack(a.name, row.level, 1)?;
        }
        Ok(())
    }

    /// Every distinct clip of the kit, in file order of the record.
    pub fn clips(&self) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = Vec::new();
        for c in self
            .attacks
            .iter()
            .map(|a| a.clip)
            .chain(self.reacts.iter().map(|r| r.clip))
            .chain(self.deaths.iter().copied())
        {
            if !v.contains(&c) {
                v.push(c);
            }
        }
        v
    }
}

/// Which way a flinch clip pushes the head: the head's largest deviation from its first frame,
/// relative to the pelvis, over the clip, as `(forward, left)` in the creature's own frame (the
/// retargeted DS clip is in Bevy axes, forward -Z, left -X). A hit from the front pushes the head
/// back: forward negative.
pub fn push(r: &Retargeted, head: usize, pelvis: usize) -> Result<(f32, f32)> {
    ensure!(!r.ds.frames.is_empty(), "{} has no frames", r.ds.name);
    let rel = |f: usize| {
        let fr = &r.ds.frames[f];
        Vec3::from_array(fr[head].t) - Vec3::from_array(fr[pelvis].t)
    };
    let base = rel(0);
    let mut best = (0.0_f32, Vec3::ZERO);
    for f in 0..r.ds.frames.len() {
        let d = rel(f) - base;
        let flat = Vec3::new(d.x, 0.0, d.z).length();
        if flat > best.0 {
            best = (flat, d);
        }
    }
    Ok((-best.1.z, -best.1.x))
}

/// The death clip ends lying or slumped and holds still: the pelvis ends lower than 0.6 of its
/// first height and moves less than `lying.settle` yd over the last ten frames, and the whole body
/// (every driven bone, lowest to highest) ends less than `lying.body` as tall as it began, so a
/// pelvis that drops while the spine, head or arms stay standing is refused.
pub fn check_death(r: &Retargeted, pelvis: usize, lying: &Lying) -> Result<()> {
    let f = &r.ds.frames;
    ensure!(
        f.len() > 12,
        "{}: {} frames is no death",
        r.ds.name,
        f.len()
    );
    let y = |i: usize| f[i][pelvis].t[1];
    let p = |i: usize| Vec3::from_array(f[i][pelvis].t);
    let last = f.len() - 1;
    ensure!(
        y(last) < 0.6 * y(0),
        "{}: the pelvis ends at {:.2} yd from {:.2}: not down",
        r.ds.name,
        y(last),
        y(0)
    );
    ensure!(
        (last - 10..=last).all(|i| (p(i) - p(last)).length() < lying.settle),
        "{}: the pelvis is still moving at the end",
        r.ds.name
    );
    let (first, end) = r.body_height;
    ensure!(
        end < lying.body * first,
        "{}: the body ends {end:.2} yd tall from {first:.2}: it is not lying",
        r.ds.name
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kobold_kit_names_distinct_attacks_and_clips() {
        let k = &KOBOLD;
        let mut names: Vec<_> = k.attacks.iter().map(|a| a.name).collect();
        let mut clips: Vec<_> = k.attacks.iter().map(|a| a.clip).collect();
        names.sort_unstable();
        names.dedup();
        clips.sort_unstable();
        clips.dedup();
        assert_eq!((names.len(), clips.len()), (6, 6));
        assert!(k.attacks.iter().all(|a| a.clip.starts_with("a00_3")));
        // Flinch clips repeat across levels; the kit lists each file once.
        assert_eq!(k.clips().len(), 6 + 6 + 2);
    }

    #[test]
    fn a_common_creature_gets_two_gentle_single_window_attacks() {
        assert_eq!(Tier::Common.max_attacks(), 2);
        assert!(Tier::Common.check_attack("bite", 1, 1).is_ok());
        assert!(Tier::Common.check_attack("bite", 2, 1).is_ok());
        let e = Tier::Common
            .check_attack("slam", 5, 1)
            .unwrap_err()
            .to_string();
        assert!(e.contains("too hard"), "{e}");
        // dmgLevel 8 (the thief's and the assassin's first moves) is no flinch level to trust.
        assert!(Tier::Common.check_attack("stab", 8, 1).is_err());
        let e = Tier::Common
            .check_attack("flurry", 1, 3)
            .unwrap_err()
            .to_string();
        assert!(e.contains("hit windows"), "{e}");
    }

    #[test]
    fn an_elite_creature_may_chain_hits_but_not_knock_down() {
        assert_eq!(Tier::Elite.max_attacks(), 5);
        assert!(Tier::Elite.check_attack("flurry", 1, 3).is_ok());
        assert!(Tier::Elite.check_attack("slam", 5, 1).is_err());
        assert!(Tier::Elite.check_attack("stab", 8, 1).is_err());
    }

    #[test]
    fn the_legacy_kobold_keeps_its_six_and_nothing_more() {
        assert_eq!(Tier::Legacy.max_attacks(), 6);
        assert!(Tier::Legacy.check_attack("body_check", 5, 1).is_ok());
    }

    #[test]
    fn every_kit_has_at_most_its_tiers_attacks() {
        for k in KITS {
            assert!(
                k.attacks.len() <= k.tier.max_attacks(),
                "{} has {} attacks as a {:?} creature",
                k.name,
                k.attacks.len(),
                k.tier
            );
        }
        let mut names: Vec<_> = KITS.iter().map(|k| k.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), KITS.len(), "kit names are distinct");
    }

    #[test]
    fn install_the_kit_record_agrees_with_the_params() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let p = crate::params::load(std::path::Path::new(&dir)).unwrap();
        for k in KITS {
            k.check(&p)
                .unwrap_or_else(|e| panic!("the {} kit: {e:#}", k.name));
        }
        let wrong = Kit {
            variation: 20_601,
            ..KOBOLD
        };
        assert!(wrong.check(&p).unwrap_err().to_string().contains("20601"));
        // The same six attacks as a Common creature break the rule.
        let greedy = Kit {
            tier: Tier::Common,
            ..KOBOLD
        };
        let e = greedy.check(&p).unwrap_err().to_string();
        assert!(e.contains("borrows 6 attacks"), "{e}");
    }

    fn clip_with(frames: Vec<Vec<wscl::Trs>>) -> Retargeted {
        let c = wscl::Clip {
            name: "t".into(),
            fps: 30.0,
            model_space: true,
            parents: vec![-1, 0],
            driven: vec![true; 2],
            frames,
            root_motion: Vec::new(),
            flags: Vec::new(),
            hits: Vec::new(),
        };
        Retargeted {
            wow: c.clone(),
            ds: c,
            max_error_deg: 0.0,
            max_cross_deg: 0.0,
            max_foot_gap: 0.0,
            max_foot_gap_all: 0.0,
            contact_frames: 0,
            // A body that lies: a fifth as tall at the end.
            body_height: (1.0, 0.2),
        }
    }

    fn at(pelvis: [f32; 3], head: [f32; 3]) -> Vec<wscl::Trs> {
        [pelvis, head]
            .map(|t| wscl::Trs {
                t,
                ..wscl::Trs::IDENTITY
            })
            .to_vec()
    }

    #[test]
    fn a_head_thrown_back_pushes_backward_and_a_head_to_the_left_pushes_left() {
        // Bevy: forward -Z, left -X. The head ends 0.3 behind (+Z) and 0.1 to the left (-X).
        let r = clip_with(vec![
            at([0.0; 3], [0.0, 1.0, 0.0]),
            at([0.0; 3], [-0.1, 1.0, 0.3]),
        ]);
        let (fwd, left) = push(&r, 1, 0).unwrap();
        assert!(
            (fwd + 0.3).abs() < 1.0e-6 && (left - 0.1).abs() < 1.0e-6,
            "{fwd} {left}"
        );
    }

    #[test]
    fn a_death_whose_body_stays_upright_is_refused() {
        let frames: Vec<_> = (0..20)
            .map(|i| at([0.0, 1.0 - 0.05 * i as f32, 0.0], [0.0, 1.5, 0.0]))
            .chain((0..12).map(|_| at([0.0, 0.05, 0.0], [0.0, 0.2, 0.0])))
            .collect();
        let mut r = clip_with(frames);
        assert!(check_death(&r, 0, &LYING_BIPED).is_ok());
        // The pelvis is down but the head and spine stand: the body is still 0.9 yd tall.
        r.body_height = (1.0, 0.9);
        let e = check_death(&r, 0, &LYING_BIPED).unwrap_err().to_string();
        assert!(e.contains("not lying"), "{e}");
    }

    #[test]
    fn a_quadruped_on_its_side_is_thicker_than_a_biped_lying_flat() {
        let frames: Vec<_> = (0..20)
            .map(|i| at([0.0, 1.0 - 0.04 * i as f32, 0.0], [0.0, 1.5, 0.0]))
            .chain((0..12).map(|_| at([0.0, 0.2, 0.0], [0.0, 0.3, 0.0])))
            .collect();
        let mut r = clip_with(frames);
        // The wolf's `a00_2205` ends 0.56 as tall as it began.
        r.body_height = (0.84, 0.47);
        assert!(check_death(&r, 0, &LYING_BIPED).is_err());
        assert!(check_death(&r, 0, &LYING_QUADRUPED).is_ok());
        r.body_height = (0.84, 0.78);
        assert!(
            check_death(&r, 0, &LYING_QUADRUPED).is_err(),
            "sitting up on the haunches is no death"
        );
    }

    #[test]
    fn a_death_must_end_low_and_still() {
        let mut frames: Vec<_> = (0..20)
            .map(|i| at([0.0, 1.0 - 0.05 * i as f32, 0.0], [0.0, 1.5, 0.0]))
            .collect();
        assert!(
            check_death(&clip_with(frames.clone()), 0, &LYING_BIPED).is_err(),
            "low at the end, but still falling"
        );
        frames.extend((0..12).map(|_| at([0.0, 0.05, 0.0], [0.0, 0.2, 0.0])));
        assert!(check_death(&clip_with(frames), 0, &LYING_BIPED).is_ok());
        let standing = vec![at([0.0, 1.0, 0.0], [0.0, 1.5, 0.0]); 20];
        assert!(check_death(&clip_with(standing), 0, &LYING_BIPED).is_err());
    }
}
