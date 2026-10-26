//! A creature's kit: which Dark Souls animation is which move, flinch and death, for one
//! creature of the game. Hand-made, like the bone map and the weapon classes. The ghoul's
//! animation files are `aXX_YYYY` (TAE id `XX × 10000 + YYYY`); the sword kit is `a00_3xxx`.

use anyhow::{ensure, Context, Result};
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
    /// A creature that leaps at the player and hops away again (the spiders): the Common budget of two blows
    /// and a third move, the hop, that carries no blow of its own (its window lies past the clip's end).
    Skirmish,
    /// An ordinary creature whose frenzy is a combo (the murlocs, 2026-10-18): the Common budget of two attacks,
    /// but an attack may strike more than once (each blow tamed light).
    Frenzy,
    /// A miniboss: at most 5 attacks, hit windows may repeat, flinch level 1 or 2.
    Elite,
    /// The kobold as it was built before the rule: its 6 attacks, no more.
    Legacy,
    /// A zone boss (Hogger): up to 12 attacks, any flinch level up to Dark Souls' 10, hit windows
    /// may repeat. Worn by no ordinary creature.
    Boss,
}

impl Tier {
    pub fn max_attacks(self) -> usize {
        match self {
            Tier::Common | Tier::Frenzy => 2,
            Tier::Skirmish => 3,
            Tier::Elite => 5,
            Tier::Legacy => 6,
            Tier::Boss => 12,
        }
    }

    /// Whether one attack fits the tier: its flinch level (`AtkParam_Npc.dmgLevel`) and how many
    /// hit windows its clip carries.
    pub fn check_attack(self, name: &str, level: i64, windows: usize) -> Result<()> {
        if self == Tier::Boss {
            ensure!(
                (1..=10).contains(&level),
                "{name}: flinch level {level} is not a Dark Souls level (1 to 10)"
            );
            return Ok(());
        }
        if self == Tier::Legacy {
            return Ok(());
        }
        ensure!(
            (1..=2).contains(&level),
            "{name}: flinch level {level} is too hard for a {self:?} creature (1 or 2)"
        );
        ensure!(
            matches!(self, Tier::Elite | Tier::Frenzy) || windows == 1,
            "{name}: {windows} hit windows; a {self:?} creature's attacks have one"
        );
        Ok(())
    }
}

/// The souls a boss gives when its `NpcParam` row carries none (a Dark Souls boss's reward is not in
/// that row): the Taurus Demon's, 3000.
pub const BOSS_SOULS: i64 = 3000;

/// The most souls a Common creature gives: an ordinary mob wearing a heavier enemy's moves (the boar on the
/// Armored Tusk, 750 in its row) pays what an ordinary mob does.
pub const COMMON_SOULS: i64 = 100;

/// The souls a kill gives: the row's, or a boss's [`BOSS_SOULS`] when the row has none; a Common creature's
/// at most [`COMMON_SOULS`].
pub fn souls_for(tier: Tier, row_souls: i64) -> i64 {
    match tier {
        Tier::Boss if row_souls <= 0 => BOSS_SOULS,
        Tier::Common | Tier::Frenzy => row_souls.min(COMMON_SOULS),
        _ => row_souls,
    }
}

/// A boss's blow keeps only the hit windows of the judge its kit declares: the demon's swings carry
/// windows of other judges too (`a00_3000`: 100 is the axe, 101 the shockwave on the ground), and
/// the game deals one damage per blow.
pub fn keep_judge(hits: &mut Vec<wscl::HitWindow>, judge: i32) {
    hits.retain(|h| h.judge == judge);
}

/// How far a leap carries the creature (a multiple of the claw lunge's own travel) and how high it arches the body
/// (WoW model yards, before the creature's drawn scale).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leap {
    pub travel: f32,
    pub arc: f32,
    /// The claw lunge's mirror (the other claw): the leap is the two lunges blended half and half, so the body
    /// goes straight at the player instead of twisting its front to one side.
    pub mirror: Option<&'static str>,
}

/// The mine spider is drawn at 0.55 to 0.7 of its model.
pub const LEAP_MINE: Leap = Leap {
    travel: 2.6,
    arc: 1.3,
    mirror: Some("a00_3000"),
};
/// The tarantula is drawn at 0.4 and is a flat model with a smaller kit scale: its model yards count for less, so it
/// travels and rises more of them.
pub const LEAP_TARANTULA: Leap = Leap {
    travel: 3.0,
    arc: 2.2,
    mirror: Some("a00_3000"),
};

/// The leap of a kit.
pub fn leap_of(kit_name: &str) -> Leap {
    if kit_name == "tarantula" {
        LEAP_TARANTULA
    } else {
        LEAP_MINE
    }
}
/// Blends `other` into `clip` half and half, frame by frame (positions averaged, rotations by a normalised
/// lerp along the shorter arc), root motion too. The Crag-Spider's two claw lunges (`a00_3000` left,
/// `a00_3001` right, 46 frames each, the same window) are mirrors: their mean is a straight lunge.
pub fn blend_mirror(clip: &mut wscl::Clip, other: &wscl::Clip) {
    for (frame, mirror) in clip.frames.iter_mut().zip(&other.frames) {
        for (a, b) in frame.iter_mut().zip(mirror) {
            let dot: f32 = a.r.iter().zip(&b.r).map(|(x, y)| x * y).sum();
            let sign = if dot < 0.0 { -1.0 } else { 1.0 };
            let r =
                glam::Quat::from_array(std::array::from_fn(|k| a.r[k] + sign * b.r[k])).normalize();
            a.r = r.to_array();
            a.t = std::array::from_fn(|k| (a.t[k] + b.t[k]) * 0.5);
        }
    }
    for (a, b) in clip.root_motion.iter_mut().zip(&other.root_motion) {
        *a = std::array::from_fn(|k| (a[k] + b[k]) * 0.5);
    }
}

/// The leap's flight starts this long before the claw lunge's blow window opens (s) and ends this long after it
/// closes.
const LEAP_LAUNCH: f32 = 0.25;
const LEAP_LAND: f32 = 0.20;

/// A leap's flight in the cache. The claw lunge it is made of travels early and rears before its blow; the
/// leap holds still through its crouch, then carries the body the stretched distance ([`Leap::travel`]) along a
/// smooth step and lifts it along a half sine ([`Leap::arc`], every driven top-level bone, so a body whose parts
/// are roots of their own rises as one). The blow's window becomes the flight: the creature tracks the player
/// until it leaves the ground, then is committed, so a step aside or a roll through the flight avoids it.
pub fn bake_leap(clip: &mut wscl::Clip, hit: (f32, f32), leap: Leap) {
    let (from, to) = (hit.0 - LEAP_LAUNCH, hit.1 + LEAP_LAND);
    let flight = |i: usize| ((i as f32 / clip.fps - from) / (to - from)).clamp(0.0, 1.0);
    let (total_x, total_z) = clip.root_motion.last().map_or((0.0, 0.0), |r| (r[0], r[2]));
    let steps: Vec<f32> = (0..clip.root_motion.len()).map(flight).collect();
    for (r, u) in clip.root_motion.iter_mut().zip(&steps) {
        let ease = u * u * (3.0 - 2.0 * u);
        r[0] = total_x * leap.travel * ease;
        r[2] = total_z * leap.travel * ease;
    }
    let (parents, driven) = (&clip.parents, &clip.driven);
    for (frame, u) in clip.frames.iter_mut().zip(&steps) {
        let lift = leap.arc * (std::f32::consts::PI * u).sin().max(0.0);
        for (b, trs) in frame.iter_mut().enumerate() {
            if parents[b] < 0 && driven[b] {
                trs.t[1] += lift;
            }
        }
    }
    for h in &mut clip.hits {
        h.start = from + 0.05;
        h.end = to - 0.10;
    }
}

/// What a death must look like by its last frame: the body (every driven bone, lowest to highest)
/// ends less than `body` of its standing height, and the pelvis moves less than `settle` yd over
/// the last ten frames.
#[derive(Debug, Clone, Copy)]
pub struct Lying {
    pub body: f32,
    /// The pelvis ends under this fraction of its first height (0.6 unless the body's hips barely drop).
    pub pelvis: f32,
    pub settle: f32,
}

/// A biped lies flat: under half its height.
pub const LYING_BIPED: Lying = Lying {
    body: 0.5,
    pelvis: 0.6,
    settle: 0.02,
};

/// The assassin's deaths settle a little slower than the others' (`a00_2204` still moves 0.024 yd over
/// its last ten frames, `a00_2205` 0.017): a biped lying flat, with the settle limit of 0.03.
pub const LYING_ASSASSIN: Lying = Lying {
    body: 0.5,
    pelvis: 0.6,
    settle: 0.03,
};

/// The gnoll soldier's deaths: `a00_2205` still moves 0.031 yd over its last ten frames (a hunched body settling),
/// `a00_2202` 0.013; both end under half the gnoll's height (0.46 and 0.49).
pub const LYING_GNOLL: Lying = Lying {
    body: 0.5,
    pelvis: 0.6,
    settle: 0.035,
};

/// The murloc is squat: lying on its back it is still over half as thick as it stood tall (`a00_2205` ends at
/// 0.51 of its height, `a00_2202` at 0.52).
pub const LYING_MURLOC: Lying = Lying {
    body: 0.55,
    pelvis: 0.6,
    settle: 0.035,
};

/// The boar is deep-bodied and lifted by its ground lock: on its side (`a00_2205`) it ends at 0.78 of its
/// standing height. Judged in motion before it is accepted.
pub const LYING_BOAR: Lying = Lying {
    body: 0.85,
    pelvis: 0.6,
    settle: 0.08,
};

/// The tarantula is only 0.17 yd tall at rest, so the legs thrown up in the death are taller than the whole
/// standing body: only the pelvis is judged. Seen in motion before it is accepted.
pub const LYING_TARANTULA: Lying = Lying {
    body: 12.0,
    ..LYING_SPIDER
};

/// The Crag-Spider dies onto its back with the legs curled up over it: its pelvis ends at 0.8 of its height,
/// the legs and abdomen reach above where they stood (so no body-height test), and the last frames of the
/// 1.5 s clips are still settling. Judged in motion before it is accepted.
pub const LYING_SPIDER: Lying = Lying {
    body: 2.5,
    pelvis: 0.85,
    settle: 0.6,
};

/// A quadruped on its side is still as thick as its body and its raised legs: under 0.65 of its
/// standing height (the wolf's `a00_2205` ends at 0.56), and it may still be settling.
pub const LYING_QUADRUPED: Lying = Lying {
    body: 0.65,
    pelvis: 0.6,
    settle: 0.08,
};

/// One attack: the game's name for it, its clip and the judge id its TAE hit window must carry.
pub struct Attack {
    pub name: &'static str,
    pub clip: &'static str,
    pub judge: i32,
    /// A hit window (clip s) for a move whose clip carries none of its own: a charge or a hop is
    /// a movement, so the clip has no blow, and the kit needs one to know when the move is
    /// "striking" (the creature tracks the player until it opens). The judge's params give the
    /// blow; a window set past the clip's end never fires (a pure movement).
    pub window: Option<(f32, f32)>,
}

/// One flinch clip and the hit level it answers.
pub struct React {
    pub level: u8,
    pub clip: &'static str,
}

/// Player weapon attacks borrowed by a creature. The base kit still identifies its actual
/// creature source for reactions, deaths and rewards; these attacks resolve through
/// `BehaviorParam_PC` and `AtkParam_Pc`, never a made-up creature variation.
#[derive(Clone, Copy)]
pub struct PlayerAttacks {
    pub weapon: i32,
    pub variation: i64,
    pub rig: &'static Rig<'static>,
    pub response: Option<PlayerResponse>,
}

/// A creature blow borrowed from a heavier Dark Souls enemy, tamed for an ordinary mob (rule R1: a Common
/// creature's blows are flinch level 1 or 2): the motion and the window stay the source's, the blow's
/// weight is this. The source numbers are checked against the row, so a changed row cannot slip through,
/// and the manifest records both (`attack-adaptation <name> tame ...`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tame {
    pub source_level: i64,
    pub source_phys: i64,
    pub phys: i64,
    pub poise: i64,
    pub level: i64,
    pub knock: f32,
    /// The guard's stamina cost (`guardAtkRate`); a tamed blow is always blockable (the Tusk's and the Feline's
    /// rows cost 100).
    pub guard_rate: i64,
}

/// Every tamed blow: (kit, judge, tame). The boar wears the Armored Tusk (`c3460`, HP 256): its gore
/// (judge 100) is a level 9 knockdown with a 4 m push, its lunge (judge 160) level 7. As a Stonetusk they
/// are the black dog's weight (phys 104 / 100, which the boars wore before) and level 1 like every Elwynn
/// Common blow; the gore keeps a heavier shove (1.0 m) than the charge (0.6 m). The bear wears the Great Feline
/// (`c5360`, HP 666), whose bite (judge 120) is level 3, and the drake's claw swipe (`c3471`, judge 150, knock 1.5
/// m): both become ordinary blows.
pub const TAMED: &[(&str, i32, Tame)] = &[
    // The murlocs' frenzy (2026-10-18): three light swings instead of one full one, the lunge a little
    // under the soldier's.
    (
        "murloc",
        170,
        Tame {
            source_level: 1,
            source_phys: 160,
            phys: 105,
            poise: 15,
            level: 1,
            knock: 0.35,
            guard_rate: 25,
        },
    ),
    (
        "murloc",
        150,
        Tame {
            source_level: 2,
            source_phys: 210,
            phys: 170,
            poise: 30,
            level: 1,
            knock: 1.0,
            guard_rate: 40,
        },
    ),
    (
        "erlan",
        300,
        Tame {
            source_level: 3,
            source_phys: 143,
            phys: 143,
            poise: 60,
            level: 2,
            knock: 1.0,
            guard_rate: 60,
        },
    ),
    (
        "erlan",
        310,
        Tame {
            source_level: 3,
            source_phys: 143,
            phys: 143,
            poise: 60,
            level: 2,
            knock: 1.0,
            guard_rate: 60,
        },
    ),
    (
        "morgaine",
        110,
        Tame {
            source_level: 8,
            source_phys: 102,
            phys: 110,
            poise: 10,
            level: 1,
            knock: 0.3,
            guard_rate: 20,
        },
    ),
    // The specials (spec 2026-10-17): source blows above an Elite's level 2 brought down to it; a guard breaker
    // costs a raised guard all its stamina. Princess wears the Tusk's blows at a quest boar's weight.
    (
        "fedfennel",
        180,
        Tame {
            source_level: 2,
            source_phys: 212,
            phys: 212,
            poise: 30,
            level: 2,
            knock: 1.0,
            guard_rate: 100,
        },
    ),
    (
        "thuros",
        100,
        Tame {
            source_level: 8,
            source_phys: 100,
            phys: 110,
            poise: 10,
            level: 1,
            knock: 0.3,
            guard_rate: 20,
        },
    ),
    (
        "thuros",
        110,
        Tame {
            source_level: 8,
            source_phys: 102,
            phys: 110,
            poise: 10,
            level: 1,
            knock: 0.3,
            guard_rate: 20,
        },
    ),
    (
        "morgaine",
        100,
        Tame {
            source_level: 8,
            source_phys: 100,
            phys: 110,
            poise: 10,
            level: 1,
            knock: 0.3,
            guard_rate: 20,
        },
    ),
    (
        "surena",
        200,
        Tame {
            source_level: 1,
            source_phys: 0,
            phys: 130,
            poise: 20,
            level: 1,
            knock: 0.6,
            guard_rate: 30,
        },
    ),
    (
        "princess",
        100,
        Tame {
            source_level: 9,
            source_phys: 280,
            phys: 130,
            poise: 45,
            level: 2,
            knock: 1.5,
            guard_rate: 60,
        },
    ),
    (
        "princess",
        110,
        Tame {
            source_level: 7,
            source_phys: 280,
            phys: 120,
            poise: 45,
            level: 2,
            knock: 1.2,
            guard_rate: 50,
        },
    ),
    (
        "princess",
        120,
        Tame {
            source_level: 7,
            source_phys: 190,
            phys: 110,
            poise: 30,
            level: 2,
            knock: 1.0,
            guard_rate: 40,
        },
    ),
    (
        "princess",
        160,
        Tame {
            source_level: 7,
            source_phys: 190,
            phys: 110,
            poise: 30,
            level: 2,
            knock: 1.0,
            guard_rate: 40,
        },
    ),
    (
        "princess",
        180,
        Tame {
            source_level: 9,
            source_phys: 280,
            phys: 130,
            poise: 45,
            level: 2,
            knock: 1.5,
            guard_rate: 60,
        },
    ),
    // The ghoul's shoulder shove (`a00_3004`, level 5, phys 106) for the kobold elites: a level 2 blow that
    // costs a raised guard all its stamina (it breaks a block).
    (
        "ruklar",
        140,
        Tame {
            source_level: 5,
            source_phys: 106,
            phys: 150,
            poise: 30,
            level: 2,
            knock: 1.37,
            guard_rate: 100,
        },
    ),
    (
        "narg",
        140,
        Tame {
            source_level: 5,
            source_phys: 106,
            phys: 150,
            poise: 30,
            level: 2,
            knock: 1.37,
            guard_rate: 100,
        },
    ),
    (
        "geomancer",
        200,
        Tame {
            source_level: 1,
            source_phys: 0,
            phys: 120,
            poise: 20,
            level: 1,
            knock: 0.6,
            guard_rate: 30,
        },
    ),
    (
        "caster",
        200,
        Tame {
            source_level: 1,
            source_phys: 0,
            phys: 130,
            poise: 20,
            level: 1,
            knock: 0.6,
            guard_rate: 30,
        },
    ),
    (
        "caster_female",
        200,
        Tame {
            source_level: 1,
            source_phys: 0,
            phys: 130,
            poise: 20,
            level: 1,
            knock: 0.6,
            guard_rate: 30,
        },
    ),
    (
        "bear",
        150,
        Tame {
            source_level: 1,
            source_phys: 131,
            phys: 110,
            poise: 20,
            level: 1,
            knock: 0.9,
            guard_rate: 20,
        },
    ),
    (
        "bear",
        120,
        Tame {
            source_level: 3,
            source_phys: 190,
            phys: 110,
            poise: 20,
            level: 1,
            knock: 0.9,
            guard_rate: 20,
        },
    ),
    (
        "boar",
        100,
        Tame {
            source_level: 9,
            source_phys: 280,
            phys: 110,
            poise: 20,
            level: 1,
            knock: 1.0,
            guard_rate: 30,
        },
    ),
    (
        "boar",
        160,
        Tame {
            source_level: 7,
            source_phys: 190,
            phys: 100,
            poise: 20,
            level: 1,
            knock: 0.6,
            guard_rate: 20,
        },
    ),
];

/// An attack a kit takes from another Dark Souls creature than its own `chr` (its moves, flinches and deaths stay
/// the kit's): the source's chr, its `NpcParam` row and behavior variation (the blow's `AtkParam_Npc` row), the
/// rig that carries the source onto the kit's WoW model, and the WoW bones the blow's volume runs between (the
/// kit's own reach is its bite).
#[derive(Clone, Copy)]
pub struct Borrow {
    pub chr: &'static str,
    pub npc: i32,
    pub variation: i64,
    pub rig: &'static Rig<'static>,
    pub reach: Option<(u16, u16)>,
    /// A spell: the player's (`c0000`) cast and the `Magic` row whose bullet it releases at the clip's blow (its
    /// flight in the manifest's `attack-bullet` line). Its damage is the kit's [`Tame`] (a spell's damage is the
    /// catalyst's, not the row's); `npc` and `variation` are 0.
    pub spell: Option<i32>,
}

/// Every borrowed attack: (kit, attack name, source). The bear's Great Feline only bites; its claw swipe is the
/// drake's (`c3471`) wide sweep of the left forepaw `a00_3005` (1.70 s, hit 0.27-0.43 s, level 1), the volume
/// along the bear's left forearm (wrist 43 to elbow 36).
pub const BORROWED: &[(&str, &str, Borrow)] = &[
    (
        "surena",
        "bolt",
        Borrow {
            chr: "c0000",
            npc: 0,
            variation: 0,
            rig: &bonemap::HUMAN_FEMALE_RIG,
            reach: None,
            spell: Some(3000),
        },
    ),
    // Heavy Soul Arrow: the same cast, a slower and larger bolt.
    (
        "surena",
        "heavy_bolt",
        Borrow {
            chr: "c0000",
            npc: 0,
            variation: 0,
            rig: &bonemap::HUMAN_FEMALE_RIG,
            reach: None,
            spell: Some(3020),
        },
    ),
    (
        "bear",
        "swipe",
        Borrow {
            chr: "c3471",
            npc: 347_100,
            variation: 34_710,
            rig: &bonemap::DRAKE_BEAR_RIG,
            reach: Some((43, 36)),
            spell: None,
        },
    ),
    (
        "caster",
        "bolt",
        Borrow {
            chr: "c0000",
            npc: 0,
            variation: 0,
            rig: &bonemap::HUMAN_MALE_RIG,
            reach: None,
            spell: Some(3000),
        },
    ),
    (
        "caster_female",
        "bolt",
        Borrow {
            chr: "c0000",
            npc: 0,
            variation: 0,
            rig: &bonemap::HUMAN_FEMALE_RIG,
            reach: None,
            spell: Some(3000),
        },
    ),
    (
        "geomancer",
        "bolt",
        Borrow {
            chr: "c0000",
            npc: 0,
            variation: 0,
            rig: &bonemap::PLAYER_KOBOLD_RIG,
            reach: None,
            spell: Some(3000),
        },
    ),
];

/// Where a kit's attack comes from when it is not the kit's own creature.
pub fn borrowed(kit: &str, attack: &str) -> Option<Borrow> {
    BORROWED
        .iter()
        .find(|(k, a, _)| *k == kit && *a == attack)
        .map(|&(_, _, b)| b)
}

/// The tame of a kit's blow, if it has one.
pub fn tamed(kit: &str, judge: i32) -> Option<Tame> {
    TAMED
        .iter()
        .find(|(k, j, _)| *k == kit && *j == judge)
        .map(|&(_, _, t)| t)
}

/// An explicit gameplay adaptation of a source behavior, recorded separately in the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerResponse {
    /// PC kicks are zero-HP level 5 guard breaks. A Common Defias instead deals a small hit
    /// with a level 1 response. This applies only to the verified kick judge, never a swing.
    SnapKick,
}

pub struct Kit {
    pub name: &'static str,
    pub tier: Tier,
    /// The WoW model the clips are retargeted onto, and its bone table.
    pub wow_model: &'static str,
    pub rig: &'static Rig<'static>,
    pub chr: &'static str,
    /// Target-scaled DS reference clips live here instead of `chr` when a source has another
    /// WoW target. `None` preserves the original kit's reference folder and manifest.
    pub reference_dir: Option<&'static str>,
    /// What its deaths must look like at their end.
    pub lying: Lying,
    /// An `NpcParam` row of the creature, and the behavior variation it must carry.
    pub npc: i32,
    pub variation: i64,
    pub player_attacks: Option<PlayerAttacks>,
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
    reference_dir: None,
    lying: LYING_BIPED,
    npc: 206_000,
    variation: 20_600,
    player_attacks: None,
    attacks: &[
        Attack {
            name: "small_1",
            clip: "a00_3000",
            judge: 100,
            window: None,
        },
        Attack {
            name: "small_2",
            clip: "a00_3001",
            judge: 110,
            window: None,
        },
        Attack {
            name: "big",
            clip: "a00_3002",
            judge: 120,
            window: None,
        },
        Attack {
            name: "run_strike",
            clip: "a00_3003",
            judge: 130,
            window: None,
        },
        Attack {
            name: "body_check",
            clip: "a00_3004",
            judge: 140,
            window: None,
        },
        Attack {
            name: "pounce",
            clip: "a00_3006",
            judge: 160,
            window: None,
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
    reference_dir: None,
    lying: LYING_QUADRUPED,
    npc: 334_000,
    variation: 33_400,
    player_attacks: None,
    attacks: &[
        Attack {
            name: "bite",
            clip: "a00_3000",
            judge: 100,
            window: None,
        },
        Attack {
            name: "lunge",
            clip: "a00_3004",
            judge: 130,
            window: None,
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

/// Prowler's 61-bone DireWolf target borrows only the black dog's close bite. The dog's lunges
/// need 0.60-0.70 yd of ground correction on this shorter-legged body, past the unchanged 0.45
/// yd gate. Its seven standing flinches and side-fall death fit; Wolf's 64-bone clips do not.
pub const DIREWOLF: Kit = Kit {
    name: "direwolf",
    wow_model: crate::wow::DIREWOLF_M2,
    rig: &bonemap::DIREWOLF_RIG,
    reference_dir: Some("direwolf_ds"),
    attacks: &[Attack {
        name: "bite",
        clip: "a00_3000",
        judge: 100,
        window: None,
    }],
    ..WOLF
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
    reference_dir: None,
    lying: LYING_BIPED,
    npc: 252_000,
    variation: 25_200,
    player_attacks: None,
    attacks: &[
        Attack {
            name: "slash_1",
            clip: "a00_3003",
            judge: 130,
            window: None,
        },
        Attack {
            name: "slash_2",
            clip: "a00_3004",
            judge: 140,
            window: None,
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

/// The same dagger moves on the distinct 117-bone human female body. Source references use
/// this target's scale and must not overwrite the male c2520 clips.
/// The Defias Ambusher (Clara Charles's ambush) wears the thief's two slashes on its own WoW body, the
/// `HumanThief` creature model ([`bonemap::HUMAN_THIEF_RIG`]).
pub const AMBUSHER_KIT: Kit = Kit {
    name: "ambusher",
    wow_model: crate::wow::HUMAN_THIEF_M2,
    rig: &bonemap::HUMAN_THIEF_RIG,
    reference_dir: Some("ambusher_ds"),
    ..THUG
};

pub const THUG_FEMALE: Kit = Kit {
    name: "thug_female",
    wow_model: crate::wow::HUMAN_FEMALE_M2,
    rig: &bonemap::THIEF_FEMALE_RIG,
    reference_dir: Some("thug_female_ds"),
    ..THUG
};

/// The Kobold Geomancer: the kobold body, flinches and deaths ([`KOBOLD`]), the ghoul's light swing close in, and
/// the player's staff cast borrowed onto the kobold rig ([`BORROWED`]); WoW: Fireball (`creature_spells` 4760).
pub const GEOMANCER_KIT: Kit = Kit {
    name: "geomancer",
    tier: Tier::Common,
    reference_dir: Some("geomancer_ds"),
    attacks: &[
        Attack {
            name: "bolt",
            clip: "a38_4000",
            judge: 200,
            window: None,
        },
        Attack {
            name: "small_1",
            clip: "a00_3000",
            judge: 100,
            window: None,
        },
    ],
    ..KOBOLD
};

/// The Defias Rogue Wizard (and Surena Caledon, female): the thief's body, flinches and deaths ([`THUG`]), one of
/// its slashes as a close swat, and the player's staff cast `a38_4000` (a quick thrust that releases its spell at
/// 0.53 s) borrowed from `c0000` with Soul Arrow's bullet ([`BORROWED`]): the game flies the bolt (`attack-bullet`).
/// Whether it reads as frost or fire is the species' (WoW: the Rogue Wizard casts Frostbolt, Surena Fireball).
pub const CASTER: Kit = Kit {
    name: "caster",
    reference_dir: Some("caster_ds"),
    attacks: &[
        Attack {
            name: "bolt",
            clip: "a38_4000",
            judge: 200,
            window: None,
        },
        Attack {
            name: "swat",
            clip: "a00_3003",
            judge: 130,
            window: None,
        },
    ],
    ..THUG
};

pub const CASTER_FEMALE: Kit = Kit {
    name: "caster_female",
    wow_model: crate::wow::HUMAN_FEMALE_M2,
    rig: &bonemap::THIEF_FEMALE_RIG,
    reference_dir: Some("caster_female_ds"),
    ..CASTER
};

/// Erlan's equipped two-handed axe uses the Battle Axe's two light two-handed attacks.
/// Both are single-window level 2 blows; the greataxe's level 3 blows would break Common.
/// The thief's proven male reactions and deaths remain separate from the c0000 attack source.
pub const HUMAN_AXE_2H: Kit = Kit {
    name: "human_axe_2h",
    reference_dir: Some("human_axe_2h_ds"),
    player_attacks: Some(PlayerAttacks {
        weapon: 701_000,
        variation: 3_000,
        rig: &bonemap::HUMAN_MALE_RIG,
        response: None,
    }),
    attacks: &[
        Attack {
            name: "axe_1",
            clip: "a30_4000",
            judge: 200,
            window: None,
        },
        Attack {
            name: "axe_2",
            clip: "a30_4001",
            judge: 210,
            window: None,
        },
    ],
    ..THUG
};

/// Ordinary bandits alternate a one-handed sword blow with the authored right-foot kick.
/// The kick's bounded Common response is explicit; its original level 5 guard break remains
/// visible as source provenance instead of being mislabeled as a gentle DS attack.
pub const HUMAN_BANDIT: Kit = Kit {
    name: "human_bandit",
    reference_dir: Some("human_bandit_ds"),
    player_attacks: Some(PlayerAttacks {
        weapon: 201_000,
        variation: 2_300,
        rig: &bonemap::HUMAN_MALE_RIG,
        response: Some(PlayerResponse::SnapKick),
    }),
    attacks: &[
        Attack {
            name: "sword",
            clip: "a23_3000",
            judge: 0,
            window: None,
        },
        Attack {
            name: "kick",
            clip: "a23_3100",
            judge: 410,
            window: None,
        },
    ],
    ..THUG
};

pub const HUMAN_BANDIT_FEMALE: Kit = Kit {
    name: "human_bandit_female",
    reference_dir: Some("human_bandit_female_ds"),
    player_attacks: Some(PlayerAttacks {
        weapon: 201_000,
        variation: 2_300,
        rig: &bonemap::HUMAN_FEMALE_RIG,
        response: Some(PlayerResponse::SnapKick),
    }),
    attacks: HUMAN_BANDIT.attacks,
    ..THUG_FEMALE
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
    reference_dir: None,
    lying: LYING_ASSASSIN,
    npc: 240_000,
    variation: 24_000,
    player_attacks: None,
    attacks: &[
        Attack {
            name: "double",
            clip: "a00_3002",
            judge: 120,
            window: None,
        },
        Attack {
            name: "triple",
            clip: "a00_3003",
            judge: 130,
            window: None,
        },
        Attack {
            name: "lunge",
            clip: "a00_3005",
            judge: 150,
            window: None,
        },
        Attack {
            name: "dash",
            clip: "a00_3006",
            judge: 160,
            window: None,
        },
        Attack {
            name: "quick",
            clip: "a00_3008",
            judge: 180,
            window: None,
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

// ---------------------------------------------------------------------------------------------
// Elwynn's rares and named quest mobs (spec 2026-10-17 elwynn-specials): each its own Elite kit from the source
// its family already wears, so the bodies and retargets are the verified ones; patterns and styles in the game.

const fn atk(name: &'static str, clip: &'static str, judge: i32) -> Attack {
    Attack {
        name,
        clip,
        judge,
        window: None,
    }
}

/// Goldtooth (the Fargodeep kobold of "The Lost Necklace"): the ghoul's quick pair, its running strike (3.2 m,
/// level 2) and its pounce: a greedy darter that jabs in and out.
pub const GOLDTOOTH_KIT: Kit = Kit {
    name: "goldtooth",
    tier: Tier::Elite,
    reference_dir: Some("goldtooth_ds"),
    attacks: &[
        atk("small_1", "a00_3000", 100),
        atk("small_2", "a00_3001", 110),
        atk("run_strike", "a00_3003", 130),
        atk("pounce", "a00_3006", 160),
    ],
    ..KOBOLD
};

/// Ruklar the Trapper: his Net is thrown with `big` (`named.rs`), the overhead it sets up; the pair and the
/// shoulder shove (`a00_3004`, level 5 tamed to 2, a guard breaker).
pub const RUKLAR_KIT: Kit = Kit {
    name: "ruklar",
    tier: Tier::Elite,
    reference_dir: Some("ruklar_ds"),
    attacks: &[
        atk("big", "a00_3002", 120),
        atk("small_1", "a00_3000", 100),
        atk("small_2", "a00_3001", 110),
        atk("body_check", "a00_3004", 140),
    ],
    ..KOBOLD
};

/// Narg the Taskmaster holds his ground: the pair, the guard-breaking shove and the overhead it opens.
pub const NARG_KIT: Kit = Kit {
    name: "narg",
    tier: Tier::Elite,
    reference_dir: Some("narg_ds"),
    attacks: &[
        atk("small_1", "a00_3000", 100),
        atk("small_2", "a00_3001", 110),
        atk("body_check", "a00_3004", 140),
        atk("big", "a00_3002", 120),
    ],
    ..KOBOLD
};

/// Gruff Swiftbite: the soldier's two light cuts, its three-blow combo (`a00_3007`, 3.4 m, level 1), its heavy
/// lunge (`3005`, 2.4 m, level 2) and its heavy cut (`3008`, 2.6 m, level 2). Thrash (`named.rs`) chains the pair.
pub const GRUFF_KIT: Kit = Kit {
    name: "gruff",
    tier: Tier::Elite,
    reference_dir: Some("gruff_ds"),
    attacks: &[
        atk("swing_1", "a00_3000", 100),
        atk("swing_2", "a00_3004", 140),
        atk("triple", "a00_3007", 170),
        atk("heavy_lunge", "a00_3005", 150),
        atk("heavy", "a00_3008", 180),
    ],
    ..GNOLL
};

/// Fedfennel: the pair, the soldier's quick cut (`a00_3006`, hit at 0.5 s), the heavy cut (`3008`, a guard breaker
/// through its tame) and the slow overhead (`3002`, hit at 1.27 s). The shield bash (`3009`) strikes with the left
/// arm, where the gnoll holds no shield and the blow volume is not: not his.
pub const FEDFENNEL_KIT: Kit = Kit {
    name: "fedfennel",
    tier: Tier::Elite,
    reference_dir: Some("fedfennel_ds"),
    attacks: &[
        atk("swing_1", "a00_3000", 100),
        atk("swing_2", "a00_3004", 140),
        atk("quick", "a00_3006", 160),
        atk("heavy", "a00_3008", 180),
        atk("overhead", "a00_3002", 120),
    ],
    ..GNOLL
};

/// Thuros Lightfingers: the thief's two slashes, its lunge (`a00_3005`, 3.9 m, level 2) and two of its quick
/// cuts (`3000`, `3001`: level 8 in the row, tamed to 1): three cuts in a row.
pub const THUROS_KIT: Kit = Kit {
    name: "thuros",
    tier: Tier::Elite,
    reference_dir: Some("thuros_ds"),
    attacks: &[
        atk("slash_1", "a00_3003", 130),
        atk("slash_2", "a00_3004", 140),
        atk("lunge", "a00_3005", 150),
        atk("cut_1", "a00_3000", 100),
        atk("cut_2", "a00_3001", 110),
    ],
    ..THUG
};

/// Morgaine the Sly (the female body): the slashes and two quick cuts; her rhythm is the feint. The thief's
/// lunge (`a00_3005`) leaves the female body's feet 0.19 yd off the ground on contact: not hers.
pub const MORGAINE_KIT: Kit = Kit {
    name: "morgaine",
    tier: Tier::Elite,
    reference_dir: Some("morgaine_ds"),
    attacks: &[
        atk("slash_1", "a00_3003", 130),
        atk("slash_2", "a00_3004", 140),
        atk("cut_1", "a00_3000", 100),
        atk("cut_2", "a00_3001", 110),
    ],
    ..THUG_FEMALE
};

/// Morgan the Collector: cuts, steps back (`a00_3500`, 0.6 m back, no blow: its window lies past its end) and
/// comes in again with the lunge. The thief's longer hop (`3301`) twists the body 18.8 degrees off: not used.
pub const MORGAN_KIT: Kit = Kit {
    name: "morgan",
    tier: Tier::Elite,
    reference_dir: Some("morgan_ds"),
    attacks: &[
        atk("slash_1", "a00_3003", 130),
        atk("slash_2", "a00_3004", 140),
        atk("lunge", "a00_3005", 150),
        Attack {
            name: "hop_back",
            clip: "a00_3500",
            judge: 150,
            window: Some((9.0, 9.1)),
        },
    ],
    ..THUG
};

/// Dead-Tooth Jack: the player's one-handed axe (`a30`, weapon 701000, variation 3000): the R1 chain, the
/// rolling attack as a lunge, and the kick that breaks a guard.
pub const JACK_KIT: Kit = Kit {
    name: "dead_tooth_jack",
    tier: Tier::Elite,
    reference_dir: Some("dead_tooth_jack_ds"),
    player_attacks: Some(PlayerAttacks {
        weapon: 701_000,
        variation: 3_000,
        rig: &bonemap::HUMAN_MALE_RIG,
        response: Some(PlayerResponse::SnapKick),
    }),
    attacks: &[
        atk("r1_1", "a30_3000", 0),
        atk("r1_2", "a30_3001", 10),
        atk("r1_3", "a30_3002", 10),
        atk("kick", "a30_3100", 410),
        atk("roll_attack", "a30_3900", 540),
    ],
    ..HUMAN_BANDIT
};

/// James Clark: the player's straight sword (as the Defias Bandit's): the R1 pair, the R2 thrust, the kick and the
/// rolling attack.
pub const JAMES_KIT: Kit = Kit {
    name: "james_clark",
    tier: Tier::Elite,
    reference_dir: Some("james_clark_ds"),
    attacks: &[
        atk("r1_1", "a23_3000", 0),
        atk("r1_2", "a23_3001", 10),
        atk("thrust", "a23_3300", 100),
        atk("kick", "a23_3100", 410),
        atk("roll_attack", "a23_3900", 540),
    ],
    ..HUMAN_BANDIT
};

/// Erlan Drudgemoor: the player's two-handed axe: the R1 chain, the heavy R2 pair (level 3 tamed to 2) and the
/// rolling attack.
pub const ERLAN_KIT: Kit = Kit {
    name: "erlan",
    tier: Tier::Elite,
    reference_dir: Some("erlan_ds"),
    attacks: &[
        atk("axe_1", "a30_4000", 200),
        atk("axe_2", "a30_4001", 210),
        atk("heavy_1", "a30_4300", 300),
        atk("heavy_2", "a30_4310", 310),
        atk("roll_attack", "a30_4900", 580),
    ],
    ..HUMAN_AXE_2H
};

/// Surena Caledon: the staff cast with Soul Arrow, the same cast with Heavy Soul Arrow (a slower, larger bolt),
/// and the thief's two slashes for a player who closes (its lunge leaves the female body's feet off the ground).
pub const SURENA_KIT: Kit = Kit {
    name: "surena",
    tier: Tier::Elite,
    reference_dir: Some("surena_ds"),
    attacks: &[
        atk("bolt", "a38_4000", 200),
        atk("heavy_bolt", "a38_4000", 200),
        atk("swat", "a00_3003", 130),
        atk("slash", "a00_3004", 140),
    ],
    ..CASTER_FEMALE
};

/// Princess (the boar of "Princess Must Die!"): the Tusk's gore, its long charge (`a00_3001`, 11.5 m, two
/// windows), its double gore (`3002`), its short charge and its rear-up (`3009`, 6.3 m); every blow tamed to the
/// Elite's level 2 (`TAMED`).
pub const PRINCESS_KIT: Kit = Kit {
    name: "princess",
    tier: Tier::Elite,
    reference_dir: Some("princess_ds"),
    attacks: &[
        atk("gore", "a00_3000", 100),
        atk("long_charge", "a00_3001", 110),
        atk("double_gore", "a00_3002", 120),
        atk("charge", "a00_3007", 160),
        atk("rear", "a00_3009", 180),
    ],
    ..BOAR_KIT
};

/// Mother Fang: the Crag Spider's bite (her Web is thrown with it, `named.rs`), the leap and the hop back, as an
/// elite: her rhythm (Web into leap, bite twice) is in her styles.
pub const MOTHER_FANG_KIT: Kit = Kit {
    name: "mother_fang",
    tier: Tier::Elite,
    reference_dir: Some("mother_fang_ds"),
    // A miniboss (2026-10-19): the mine spider's three and the Crag Spider's spit (the Forest Spider's).
    attacks: &[
        Attack {
            name: "bite",
            clip: "a00_3000",
            judge: 100,
            window: None,
        },
        Attack {
            name: "leap",
            clip: "a00_3001",
            judge: 110,
            window: None,
        },
        Attack {
            name: "hop_back",
            clip: "a00_3500",
            judge: 110,
            window: Some((9.0, 9.1)),
        },
        Attack {
            name: "spit",
            clip: "a00_3006",
            judge: 100,
            window: Some((1.0, 1.1)),
        },
    ],
    ..MINESPIDER_KIT
};

/// Every creature of the game that wears a Dark Souls creature's moves.
/// The Taurus Demon dies on its knees: `a00_2200` (5.5 s) ends with the pelvis at 0.34 yd of 0.64 and
/// the body at 0.93 of its height, `a00_2521` (2.7 s) at 0.83: a collapse, not a body flat on the
/// ground. The limit is just over what they measure; the pelvis settles within 0.05 yd.
pub const LYING_BOSS: Lying = Lying {
    body: 0.95,
    pelvis: 0.6,
    settle: 0.05,
};

/// Hogger wears the Taurus Demon's (`c2250`, "Taurus Demon" in the game's `NPC_name_.fmg`, NpcParam
/// 225000: hp 1215, poise 120) axe moves; the axe is in its LEFT hand, so Hogger is left-handed. Of
/// its attacks `a00_3000`-`3006` and the slam `3013` are kept; so are `3007`, the Rushing Charge
/// (an 8.5 m run, no blow of its own: the kit gives it a window), and `0701`, a 6.8 m hop back
/// (a window past its end: it never strikes). `3008` (a jump down from a tower) and `3009` (a
/// 7.9 s climbing chain) are left for a later pass. The shockwave windows (`101`, `111`, ... radius 2.0, level 1) are dropped by the boss
/// judge filter ([`keep_judge`]). Flinch: `a00_2000` (2.9 s). Deaths: `a00_2200` (5.5 s, a collapse
/// onto the knees, the head pushed back) and `a00_2521` (2.7 s, the head pushed forward and left).
pub const HOGGER: Kit = Kit {
    name: "hogger",
    tier: Tier::Boss,
    wow_model: crate::wow::GNOLL_M2,
    rig: &bonemap::GNOLL_RIG,
    chr: "c2250",
    reference_dir: None,
    lying: LYING_BOSS,
    npc: 225_000,
    variation: 22_500,
    player_attacks: None,
    attacks: &[
        Attack {
            name: "overhead",
            clip: "a00_3000",
            judge: 100,
            window: None,
        },
        Attack {
            name: "heavy_swing",
            clip: "a00_3001",
            judge: 110,
            window: None,
        },
        Attack {
            name: "step_smash",
            clip: "a00_3002",
            judge: 120,
            window: None,
        },
        Attack {
            name: "sweep",
            clip: "a00_3003",
            judge: 130,
            window: None,
        },
        Attack {
            name: "lunge_smash",
            clip: "a00_3004",
            judge: 140,
            window: None,
        },
        Attack {
            name: "quick",
            clip: "a00_3005",
            judge: 150,
            window: None,
        },
        Attack {
            name: "backhand",
            clip: "a00_3006",
            judge: 160,
            window: None,
        },
        // `a00_3008` (the plunge) is the demon's jump down from the tower: root motion -7.9 m
        // vertically, the pose floats 3 yd above flat ground at the blow, so it is not used. This
        // is its on-the-spot sibling: the slam held back a beat (the window opens at 2.8 s, not
        // 1.4 s), so a roll that came early is over when the axe lands. Its feint window (judge
        // 190) has no behavior row.
        Attack {
            name: "feint_plunge",
            clip: "a00_3013",
            judge: 170,
            window: None,
        },
        // The Rushing Charge: the demon's 8.5 m run at the player (`a00_3007`, 2.27 s, no blow of
        // its own). Its window is the last of the run, a body check of the judge 171 shock (phys
        // 120, level 2).
        Attack {
            name: "charge",
            clip: "a00_3007",
            judge: 171,
            window: Some((1.55, 1.85)),
        },
        // The hop back: a 6.8 m backward leap (`a00_0701`, 2.7 s, no blow): he disengages and
        // comes again. Its window lies past the clip's end, so it never strikes.
        Attack {
            name: "hop_back",
            clip: "a00_0701",
            judge: 171,
            window: Some((9.0, 9.1)),
        },
    ],
    reacts: &[
        React {
            level: 1,
            clip: "a00_2000",
        },
        React {
            level: 2,
            clip: "a00_2000",
        },
    ],
    deaths: &["a00_2200", "a00_2521"],
};

/// The follow-up blows of a boss's combo: (kit, the kit attack's judge, the judge of a later window in the same
/// clip). A boss keeps only its declared judge's windows ([`keep_judge`]); these windows are a second real blow
/// of the move, so they are relabelled to the declared judge and strike with its row. Artorias' lunge
/// (`a00_3004`) is a run-in thrust (judge 150, level 2) into a heavy cut (160, level 7); his close two-hit
/// (`a00_3011`) a light cut (220) into a heavier one (230).
pub const FOLLOW_JUDGES: &[(&str, i32, i32)] =
    &[("hogger_knight", 160, 150), ("hogger_knight", 220, 230)];

/// Relabels a boss move's follow-up windows ([`FOLLOW_JUDGES`]) to `judge`, so [`keep_judge`] keeps them.
pub fn merge_follow_judges(hits: &mut [wscl::HitWindow], kit: &str, judge: i32) {
    for h in hits.iter_mut() {
        if FOLLOW_JUDGES
            .iter()
            .any(|&(k, j, f)| k == kit && j == judge && f == h.judge)
        {
            h.judge = judge;
        }
    }
}

/// A boss move's ground shock: the hit around where the blow lands, which hurts a player standing in it even
/// when the blade misses (spec 2026-10-16 §6.1). Its numbers are `judge`'s attack row; its radius that row's
/// times `radius_scale`; it lands at the start of `judge`'s window in the clip, or, for a move whose clip has no
/// such window, at the end of the move's own last window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shock {
    pub kit: &'static str,
    pub attack: &'static str,
    pub judge: i32,
    pub radius_scale: f32,
}

/// Every ground shock. Artorias' flip slam carries its own (judge 180: radius 3.0 m, phys 210, level 6, at
/// 2.47 s); his overhead has none (its second judge, 141, is a 0.05 m point), so it borrows the flip's row at
/// half the radius.
pub const SHOCKS: &[Shock] = &[
    Shock {
        kit: "hogger_knight",
        attack: "flip_slam",
        judge: 180,
        radius_scale: 1.0,
    },
    Shock {
        kit: "hogger_knight",
        attack: "overhead",
        judge: 180,
        radius_scale: 0.5,
    },
];

/// The ground shock of a kit's attack.
pub fn shock_of(kit: &str, attack: &str) -> Option<&'static Shock> {
    SHOCKS.iter().find(|s| s.kit == kit && s.attack == attack)
}

/// The moves whose clip's bullet (TAE kind 2) is a blast around the creature (the knight's Abyss power-up:
/// Bullet 41510, radius 7.0 m, at 5.40 s): (kit, attack).
pub const BLASTS: &[(&str, &str)] = &[("hogger_knight", "frenzy")];

/// When a move's shock lands (clip s): its judge's window, else the end of the move's last window.
pub fn shock_at(hits: &[wscl::HitWindow], shock: &Shock, own: i32) -> Option<f32> {
    hits.iter()
        .find(|h| h.judge == shock.judge)
        .map(|h| h.start)
        .or_else(|| {
            hits.iter()
                .filter(|h| h.judge == own)
                .map(|h| h.end)
                .reduce(f32::max)
        })
}

/// Hogger as Knight Artorias (`c4100`, "Knight Artorias" in `NPC_name_.fmg`, NpcParam 410000: hp 1500, poise 130,
/// variation 41000; think near 3 / mid 6 / far 20 DS m) on the gnoll body, through the undead soldier's rig (the
/// knight's skeleton carries the same biped names; `R Hand` swings the greatsword, the left arm hangs: Hogger is
/// right-handed in this kit). Blows land 0.5-1.4 s into a swing (the Taurus Demon's 1.2-2.0 s): aggression
/// comes from the source, not from a faster clock. Measured 2026-10-16 (`docs/log/2026-10-16-hogger-artorias-
/// survey.txt`). Left out: `a00_3001` (a second quick slash; the tier's twelve), `3014` (a slash variant),
/// `3012` (a dark burst with no attack row), `3006` (no blow). `frenzy` (`a00_3013`, the Abyss power-up, 8.0 s,
/// its burst a bullet at 5.40 s) is no table move: phase two forces it; its window lies past its end.
pub const HOGGER_KNIGHT: Kit = Kit {
    name: "hogger_knight",
    tier: Tier::Boss,
    wow_model: crate::wow::GNOLL_M2,
    rig: &bonemap::GNOLL_KNIGHT_RIG,
    chr: "c4100",
    reference_dir: Some("hogger_knight_ds"),
    lying: LYING_BOSS,
    npc: 410_000,
    variation: 41_000,
    player_attacks: None,
    attacks: &[
        // 0.87-1.00 s, phys 165, level 2: the plain cut.
        Attack {
            name: "slash",
            clip: "a00_3000",
            judge: 100,
            window: None,
        },
        // 1.17-1.30 s after a 5.5 m step, phys 200, level 3.
        Attack {
            name: "step_slash",
            clip: "a00_3002",
            judge: 120,
            window: None,
        },
        // 0.87-1.17 s then 1.17-1.33 s ([`FOLLOW_JUDGES`]), on the spot.
        Attack {
            name: "close_two",
            clip: "a00_3011",
            judge: 220,
            window: None,
        },
        // The leaping strike: 10 m, 1.23-1.33 s, level 6. Not named `leap` (that name bakes a spider's flight).
        Attack {
            name: "pounce",
            clip: "a00_3003",
            judge: 130,
            window: None,
        },
        // 11 m: a run-in thrust (0.97-1.37 s) into a heavy cut (1.37-1.50 s, phys 180, level 7). The opener.
        Attack {
            name: "lunge",
            clip: "a00_3004",
            judge: 160,
            window: None,
        },
        // 9.5 m, 1.40-1.60 s, level 7.
        Attack {
            name: "thrust",
            clip: "a00_3007",
            judge: 190,
            window: None,
        },
        // A turn and a half, 0.50-0.67 s, level 9.
        Attack {
            name: "spin",
            clip: "a00_3008",
            judge: 200,
            window: None,
        },
        // 6.5 m backwards, cutting at 0.87-0.97 s: the disengage that still hits.
        Attack {
            name: "backflip",
            clip: "a00_3010",
            judge: 210,
            window: None,
        },
        // The fast jab, 0.70-0.87 s: WoW's Head Butt (6730), a guard break in the game.
        Attack {
            name: "head_butt",
            clip: "a00_3023",
            judge: 240,
            window: None,
        },
        // The flip slam: 15 m, the blade 2.37-2.47 s (phys 250, level 6, poise 300), its shock 2.47-2.57 s
        // (judge 180, radius 3.0 m) dropped here and written as the move's ground shock.
        Attack {
            name: "flip_slam",
            clip: "a00_3005",
            judge: 170,
            window: None,
        },
        // The overhead: 5.4 m, 0.80-0.90 s, level 6 (judge 141 is a 0.05 m point, dropped).
        Attack {
            name: "overhead",
            clip: "a00_3009",
            judge: 140,
            window: None,
        },
        // The Abyss power-up (8.03 s): no blow; its burst is the bullet at 5.40 s.
        Attack {
            name: "frenzy",
            clip: "a00_3013",
            judge: 140,
            window: Some((9.0, 9.1)),
        },
    ],
    reacts: &[
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
            level: 1,
            clip: "a00_2007",
        },
        React {
            level: 2,
            clip: "a00_2010",
        },
        React {
            level: 2,
            clip: "a00_2011",
        },
        React {
            level: 2,
            clip: "a00_2012",
        },
        React {
            level: 2,
            clip: "a00_2013",
        },
    ],
    deaths: &["a00_2200"],
};

/// The Riverpaw gnolls (Runt, Outrunner, Gruff, Fedfennel) wear the undead soldier's (`c2550`, NpcParam
/// 255000, 85 HP) club moves on the gnoll body, two of its seven (rule R1, [`Tier::Common`]):
/// `a00_3000` (judge 100, hit 0.87-0.97 s of 2.37, phys 165) and `a00_3004` (judge 140, hit 0.37-0.53 s
/// of 1.80, phys 164), both flinch level 1 with one hit window. Flinches: `a00_2000`-`2002` 0.7 s
/// (level 1), `2004`-`2007` 1.5 s (level 2). Deaths: `a00_2204` and `a00_2200`.
pub const GNOLL: Kit = Kit {
    name: "gnoll",
    tier: Tier::Common,
    wow_model: crate::wow::GNOLL_M2,
    rig: &bonemap::GNOLL_SOLDIER_RIG,
    chr: "c2550",
    reference_dir: None,
    lying: LYING_GNOLL,
    npc: 255_000,
    variation: 25_500,
    player_attacks: None,
    attacks: &[
        Attack {
            name: "swing_1",
            clip: "a00_3000",
            judge: 100,
            window: None,
        },
        Attack {
            name: "swing_2",
            clip: "a00_3004",
            judge: 140,
            window: None,
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
    deaths: &["a00_2205", "a00_2202"],
};

/// The murlocs wear the undead soldier's (`c2550`, NpcParam 255000) club moves, as the gnolls do, on the
/// murloc body: `a00_3000` (hit 0.87-0.97 s of 2.37) and `a00_3004` (hit 0.37-0.53 s of 1.80), both flinch
/// level 1 with one hit window. Flinches and deaths as the gnoll's.
/// The murlocs: the same undead soldier, its two other ordinary moves (2026-10-18, the user: murlocs needed a
/// more interesting pattern; Elden Ring's demi-humans as the model: dash in, then flail). The running lunge
/// `a00_3005` (2.20 s, hit 0.70-0.87 s, 2.43 m forward) closes the gap; the flurry `a00_3007` (4.17 s, three
/// wild swings at 0.93, 1.73 and 2.53 s while it stumbles 3.37 m on) is the frenzy.
pub const MURLOC_KIT: Kit = Kit {
    name: "murloc",
    tier: Tier::Frenzy,
    lying: LYING_MURLOC,
    wow_model: crate::wow::MURLOC_M2,
    rig: &bonemap::MURLOC_RIG,
    attacks: &[
        Attack {
            name: "lunge",
            clip: "a00_3005",
            judge: 150,
            window: None,
        },
        Attack {
            name: "flurry",
            clip: "a00_3007",
            judge: 170,
            window: None,
        },
    ],
    ..GNOLL
};

/// The boars before they had their own source: the black dog's two wolf moves on the 57-bone boar body.
/// Kept as the fallback (a species with kit `boar_dog` wears it).
pub const BOAR_DOG_KIT: Kit = Kit {
    name: "boar_dog",
    lying: LYING_BOAR,
    wow_model: crate::wow::BOAR_M2,
    rig: &bonemap::BOAR_RIG,
    reference_dir: Some("boar_dog_ds"),
    ..WOLF
};

/// The boars (Stonetusk, Rockhide, Princess, Porcine Entourage) wear the Armored Tusk (`c3460`, NpcParam
/// 346000, HP 256), Dark Souls' own boar, on the 57-bone boar body. Two of its nine blows (rule R1, both
/// tamed, [`TAMED`]): the gore `a00_3000` (3.70 s, a long head-low wind-up, hit 1.20-1.67 s, 1.73 m
/// forward) and the short charge `a00_3007` (2.70 s, hit 1.23-1.57 s, 3.23 m). Left out: the long
/// charges `a00_3001`/`3008` (11.5 and 9.8 m), the turning gores `3004`/`3005`, the rear-up `3009`
/// (level 9). The Tusk has no level 1 flinch: its four level 2 staggers serve both (the species' poise
/// keeps light blows from staggering it every swing). Deaths: on its side (`a00_2200`), thrown back
/// (`a00_2207`).
pub const BOAR_KIT: Kit = Kit {
    name: "boar",
    tier: Tier::Common,
    wow_model: crate::wow::BOAR_M2,
    rig: &bonemap::TUSK_BOAR_RIG,
    chr: "c3460",
    reference_dir: Some("boar_ds"),
    lying: LYING_BOAR,
    npc: 346_000,
    variation: 34_600,
    player_attacks: None,
    attacks: &[
        Attack {
            name: "gore",
            clip: "a00_3000",
            judge: 100,
            window: None,
        },
        Attack {
            name: "charge",
            clip: "a00_3007",
            judge: 160,
            window: None,
        },
    ],
    reacts: &[
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
            level: 1,
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
        React {
            level: 2,
            clip: "a00_2007",
        },
    ],
    deaths: &["a00_2200", "a00_2207"],
};

/// The bear before it had its own source: the black dog's bite and lunge on the 54-bone bear body. Kept as the
/// fallback (a species with kit `bear_dog` wears it).
pub const BEAR_DOG_KIT: Kit = Kit {
    name: "bear_dog",
    wow_model: crate::wow::BEAR_M2,
    rig: &bonemap::BEAR_RIG,
    reference_dir: Some("bear_dog_ds"),
    lying: LYING_BOAR,
    ..WOLF
};

/// The Young Forest Bear wears the Great Feline (`c5360`, NpcParam 536000, HP 666) on the 54-bone bear body. Two
/// attacks (rule R1, both tamed, [`TAMED`]): its lunging bite `a00_3002` (2.03 s, hit 0.57-0.83 s, 1 m), and a claw
/// swipe it borrows from the drake ([`BORROWED`]), since the Feline's blows are all bites. Left out: the side
/// bite `3000`, the rear-up slam `3003` (a third move; it also bites), the pounces `3001`/`3004`/`3005` (5-30 m). Flinches: three 0.93 s ones (level
/// 1) and four 1.7 s ones (level 2). Deaths `a00_2200` (onto its side) and `a00_2209`.
pub const BEAR_KIT: Kit = Kit {
    name: "bear",
    tier: Tier::Common,
    wow_model: crate::wow::BEAR_M2,
    rig: &bonemap::FELINE_BEAR_RIG,
    chr: "c5360",
    reference_dir: Some("bear_ds"),
    lying: LYING_BOAR,
    npc: 536_000,
    variation: 53_600,
    player_attacks: None,
    attacks: &[
        Attack {
            name: "bite",
            clip: "a00_3002",
            judge: 120,
            window: None,
        },
        Attack {
            name: "swipe",
            clip: "a00_3005",
            judge: 150,
            window: None,
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
    deaths: &["a00_2200", "a00_2209"],
};

/// The Defias Dockmaster wears the thug's two slashes on the goblin body (`GoblinMale`), bare-handed: its
/// database equipment is empty, so the thief's dagger swings are punches.
pub const GOBLIN_KIT: Kit = Kit {
    name: "goblin",
    wow_model: crate::wow::GOBLIN_M2,
    rig: &bonemap::GOBLIN_RIG,
    reference_dir: Some("goblin_ds"),
    ..THUG
};

/// The spiders wear three moves of the Crag-Spider (`c3400`, NpcParam 340000, 93 HP) on the 91-bone mine spider: the
/// claw bite `a00_3000` (hit 0.70-0.87 s of 1.53, level 1, 1.2 m of root travel), a leap and the hop back they make
/// when a leap has landed and chewed. The leap is the second claw lunge `a00_3001` (the same window, judge 110),
/// carried further and arched by [`bake_leap`] into a jump the player can step or roll away from: the creature
/// commits to where the player stood when the window opens. The hop back is `a00_3500` (1.53 s, 1.2 m backwards,
/// no blow: a window past its end). Flinches: `a00_2000`-`2002` 0.7 s (level 1), `2004`-`2007` 1.5 s (level 2);
/// deaths `a00_2200`, `a00_2202` (1.5 s). Left out: `a00_3002`-`3005` (4 s fire sprays) and `3006`.
pub const MINESPIDER_KIT: Kit = Kit {
    name: "minespider",
    tier: Tier::Skirmish,
    wow_model: crate::wow::MINESPIDER_M2,
    rig: &bonemap::MINESPIDER_RIG,
    chr: "c3400",
    reference_dir: Some("minespider_ds"),
    lying: LYING_SPIDER,
    npc: 340_000,
    variation: 34_000,
    player_attacks: None,
    attacks: &[
        Attack {
            name: "bite",
            clip: "a00_3000",
            judge: 100,
            window: None,
        },
        Attack {
            name: "leap",
            clip: "a00_3001",
            judge: 110,
            window: None,
        },
        Attack {
            name: "hop_back",
            clip: "a00_3500",
            judge: 110,
            window: Some((9.0, 9.1)),
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
    deaths: &["a00_2200", "a00_2202"],
};

/// The Forest Spider: the same moves on the 74-bone tarantula.
pub const TARANTULA_KIT: Kit = Kit {
    name: "tarantula",
    wow_model: crate::wow::TARANTULA_M2,
    rig: &bonemap::TARANTULA_RIG,
    lying: LYING_TARANTULA,
    reference_dir: Some("tarantula_ds"),
    // Its own moves (2026-10-18, the user: the hairy spiders' leap and latch went through the world; "Dom får
    // sin egen attack pattern"): the bite, the Crag Spider's spit (a bullet from the mouth at 1.00 s, flown by
    // the game as `attack-bullet spit`, the blow the bite's row) and the hop back. No leap, so no latch.
    attacks: &[
        Attack {
            name: "bite",
            clip: "a00_3000",
            judge: 100,
            window: None,
        },
        Attack {
            name: "spit",
            clip: "a00_3006",
            judge: 100,
            window: Some((1.0, 1.1)),
        },
        Attack {
            name: "hop_back",
            clip: "a00_3500",
            judge: 110,
            window: Some((9.0, 9.1)),
        },
    ],
    ..MINESPIDER_KIT
};

pub const KITS: &[&Kit] = &[
    &KOBOLD,
    &WOLF,
    &DIREWOLF,
    &THUG,
    &THUG_FEMALE,
    &HUMAN_AXE_2H,
    &HUMAN_BANDIT,
    &HUMAN_BANDIT_FEMALE,
    &GARRICK,
    &HOGGER,
    &HOGGER_KNIGHT,
    &GNOLL,
    &MURLOC_KIT,
    &BOAR_KIT,
    &BOAR_DOG_KIT,
    &BEAR_KIT,
    &BEAR_DOG_KIT,
    &CASTER,
    &CASTER_FEMALE,
    &GEOMANCER_KIT,
    &AMBUSHER_KIT,
    &GOBLIN_KIT,
    &MINESPIDER_KIT,
    &TARANTULA_KIT,
    &GOLDTOOTH_KIT,
    &RUKLAR_KIT,
    &NARG_KIT,
    &GRUFF_KIT,
    &FEDFENNEL_KIT,
    &THUROS_KIT,
    &MORGAINE_KIT,
    &MORGAN_KIT,
    &JACK_KIT,
    &JAMES_KIT,
    &ERLAN_KIT,
    &SURENA_KIT,
    &PRINCESS_KIT,
    &MOTHER_FANG_KIT,
];

impl Kit {
    /// The physical blow written to the shared monster cache. Player-sourced damage is the
    /// declared DS weapon's base damage times its actual PC correction; poise includes that
    /// weapon's poise correction. No PC damage or flinch field comes from the base NPC row.
    /// The behavior variation whose `AtkParam_Npc` rows hold the blow of `judge`: a borrowed attack's source's, else
    /// the kit's own.
    pub fn variation_of(&self, judge: i32) -> i64 {
        self.attacks
            .iter()
            .filter(|a| a.judge == judge)
            .find_map(|a| borrowed(self.name, a.name))
            .map_or(self.variation, |b| b.variation)
    }

    /// The `Magic` row of the spell a kit's blow of `judge` casts, if it is one ([`Borrow::spell`]).
    pub fn spell_of(&self, judge: i32) -> Option<i32> {
        self.attacks
            .iter()
            .filter(|a| a.judge == judge)
            .find_map(|a| borrowed(self.name, a.name))
            .and_then(|b| b.spell)
    }

    pub fn attack(&self, params: &Params, judge: i32) -> Result<crate::params::NpcAttack> {
        if let Some(magic) = self.spell_of(judge) {
            let blow = params.spell_blow(magic)?;
            let m = params.magic(magic)?;
            let bullet = params.bullet(i32::try_from(m.ref_id).context("refId")?)?;
            let t = tamed(self.name, judge).with_context(|| {
                format!(
                    "the {} spell of judge {judge} needs a tame (its damage)",
                    self.name
                )
            })?;
            ensure!(
                blow.level == t.source_level && t.source_phys == 0,
                "the {} spell of judge {judge} has level {}; its tame says level {} phys {}",
                self.name,
                blow.level,
                t.source_level,
                t.source_phys
            );
            return Ok(crate::params::NpcAttack {
                phys: t.phys,
                poise: t.poise,
                level: t.level,
                radius: bullet.radius,
                radius2: 0.0,
                stop: 0.0,
                knock: t.knock,
            });
        }
        let Some(source) = self.player_attacks else {
            let mut blow = params.npc_attack(self.variation_of(judge), judge)?;
            if let Some(t) = tamed(self.name, judge) {
                ensure!(
                    blow.level == t.source_level && blow.phys == t.source_phys,
                    "the {} tame of judge {judge} expects level {} phys {}, the row has level {} phys {}",
                    self.name,
                    t.source_level,
                    t.source_phys,
                    blow.level,
                    blow.phys
                );
                blow.phys = t.phys;
                blow.poise = t.poise;
                blow.level = t.level;
                blow.knock = t.knock;
            }
            return Ok(blow);
        };
        let b = params.behavior(source.variation, judge)?;
        let mut blow = crate::params::NpcAttack {
            phys: (params.weapon_damage(source.weapon)? as f32 * b.mult).round() as i64,
            poise: (b.poise + params.weapon_poise(source.weapon)? * b.poise_correction).round()
                as i64,
            level: b.level,
            radius: b.radius,
            radius2: b.radius2,
            stop: b.stop,
            knock: b.knock,
        };
        if source.response == Some(PlayerResponse::SnapKick) && judge == 410 {
            // The kick of any one-handed weapon family (the Defias Bandit's sword, Dead-Tooth Jack's axe) is the same
            // zero-damage level 5 shove in the source; a Common or an Elite creature wears it as this light blow.
            ensure!(
                matches!(self.tier, Tier::Common | Tier::Elite) && b.level == 5 && b.mult == 0.0,
                "SnapKick adaptation requires PC judge 410's zero-HP level 5 source"
            );
            // Monster damage is scaled by runtime DAMAGE_K (25/138): 44 gives 8 HP.
            blow.phys = 44;
            blow.poise = 10;
            blow.level = 1;
            blow.knock = 0.45;
        }
        // A player blow above the creature's tier, brought down to it (Erlan's heavy R2).
        if let Some(t) = tamed(self.name, judge) {
            ensure!(
                blow.level == t.source_level && blow.phys == t.source_phys,
                "the {} tame of PC judge {judge} expects level {} phys {}, the blow has level {} phys {}",
                self.name,
                t.source_level,
                t.source_phys,
                blow.level,
                blow.phys
            );
            blow.phys = t.phys;
            blow.poise = t.poise;
            blow.level = t.level;
            blow.knock = t.knock;
        }
        Ok(blow)
    }

    pub fn guard_attack(&self, params: &Params, judge: i32) -> Result<crate::params::GuardAttack> {
        if self.spell_of(judge).is_some() {
            // A spell is blocked like any blow, at its tame's cost.
            return Ok(crate::params::GuardAttack {
                rate: tamed(self.name, judge).map_or(20, |t| t.guard_rate),
                unblockable: false,
            });
        }
        match self.player_attacks {
            Some(source) => {
                let mut guard =
                    params.player_guard_attack(source.weapon, source.variation, judge)?;
                if source.response == Some(PlayerResponse::SnapKick) && judge == 410 {
                    // Source kicks are guard breaks. The declared Common response can be
                    // blocked as an ordinary light hit, without the source's stamina tax.
                    self.attack(params, judge)?;
                    guard.rate = 20;
                    guard.unblockable = false;
                }
                Ok(guard)
            }
            None => {
                let mut guard = params.npc_guard_attack(self.variation_of(judge), judge)?;
                if let Some(t) = tamed(self.name, judge) {
                    guard.rate = t.guard_rate;
                    guard.unblockable = false;
                }
                Ok(guard)
            }
        }
    }

    /// The DS poses use this target's leg scale, even when another kit shares its source chr.
    pub fn reference_dir(&self) -> &'static str {
        self.reference_dir.unwrap_or(self.chr)
    }

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
        if let Some(source) = self.player_attacks {
            ensure!(
                self.reference_dir.is_some(),
                "a mixed source needs its own reference folder"
            );
            ensure!(
                self.attacks
                    .iter()
                    .all(|a| !self.reacts.iter().any(|r| r.clip == a.clip)
                        && !self.deaths.contains(&a.clip)),
                "a mixed-source attack shares a reaction or death clip"
            );
            ensure!(
                params.weapon(source.weapon)?.variation == source.variation,
                "weapon {} disagrees with PC behavior variation {}",
                source.weapon,
                source.variation
            );
        }
        ensure!(
            self.attacks.len() <= self.tier.max_attacks(),
            "{} borrows {} attacks; a {:?} creature gets at most {}",
            self.name,
            self.attacks.len(),
            self.tier,
            self.tier.max_attacks()
        );
        for a in self.attacks {
            if let Some(b) = borrowed(self.name, a.name).filter(|b| b.spell.is_none()) {
                ensure!(
                    params.npc(b.npc)?.variation == b.variation,
                    "{}: NpcParam {} does not carry variation {}",
                    a.name,
                    b.npc,
                    b.variation
                );
                ensure!(
                    self.attacks.iter().filter(|o| o.judge == a.judge).count() == 1,
                    "{}: a borrowed judge must be the kit's only one of that number",
                    a.name
                );
            }
            let row = self
                .attack(params, a.judge)
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
        y(last) < lying.pelvis * y(0),
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
    fn a_leap_is_carried_further_and_arched_over_its_blow() {
        let frames = 46;
        let at_rest = wscl::Trs {
            t: [0.0; 3],
            r: [0.0, 0.0, 0.0, 1.0],
            s: [1.0; 3],
        };
        let mut clip = wscl::Clip {
            name: "leap".into(),
            fps: 30.0,
            model_space: false,
            parents: vec![-1, 0],
            driven: vec![true, true],
            frames: vec![vec![at_rest, at_rest]; frames],
            root_motion: (0..frames)
                .map(|i| [0.0, 0.0, -0.1 * i as f32, 0.0])
                .collect(),
            flags: vec![],
            hits: vec![wscl::HitWindow {
                judge: 110,
                start: 0.70,
                end: 0.87,
            }],
        };
        bake_leap(&mut clip, (0.70, 0.87), LEAP_MINE);
        // The blow's window is the flight: from just after the launch to just before the landing.
        assert!(
            (clip.hits[0].start - 0.50).abs() < 1.0e-5 && (clip.hits[0].end - 0.97).abs() < 1.0e-5
        );
        // The travel is stretched and starts only at the launch; the yaw and height of the root motion
        // are not touched.
        assert!((clip.root_motion[45][2] + 4.5 * LEAP_MINE.travel).abs() < 1.0e-4);
        assert_eq!(clip.root_motion[12][2], 0.0, "still crouched at 0.4 s");
        assert!(
            clip.root_motion[20][2] > clip.root_motion[30][2],
            "forward is negative z"
        );
        assert_eq!(clip.root_motion[45][3], 0.0);
        let root = |i: usize| clip.frames[i][0].t[1];
        // On the ground before the launch and after the landing, at the top as the blow opens.
        assert_eq!(root(0), 0.0);
        assert_eq!(root(45), 0.0);
        let top = (0..frames)
            .max_by(|&a, &b| root(a).total_cmp(&root(b)))
            .unwrap();
        assert!(
            (0.70..=0.87).contains(&(top as f32 / 30.0)),
            "top at frame {top}"
        );
        assert!(root(top) > LEAP_MINE.arc * 0.95 && root(top) <= LEAP_MINE.arc + 1.0e-4);
        // Only the top-level bone is lifted: the child hangs from it.
        assert!(clip.frames.iter().all(|f| f[1].t[1] == 0.0));
    }

    #[test]
    fn mirrored_lunges_blend_into_a_straight_one() {
        let turned = |yaw: f32| wscl::Trs {
            t: [yaw, 1.0, 0.0],
            r: glam::Quat::from_rotation_y(yaw).to_array(),
            s: [1.0; 3],
        };
        let clip = |yaw: f32| wscl::Clip {
            name: "lunge".into(),
            fps: 30.0,
            model_space: false,
            parents: vec![-1],
            driven: vec![true],
            frames: vec![vec![turned(yaw)]; 3],
            root_motion: vec![[0.2 * yaw, 0.0, -1.2, 0.0]; 3],
            flags: vec![],
            hits: vec![],
        };
        let mut left = clip(0.7);
        blend_mirror(&mut left, &clip(-0.7));
        let r = glam::Quat::from_array(left.frames[2][0].r);
        assert!(r.angle_between(glam::Quat::IDENTITY) < 1.0e-5, "{r}");
        assert_eq!(left.frames[2][0].t, [0.0, 1.0, 0.0]);
        assert_eq!(left.root_motion[1], [0.0, 0.0, -1.2, 0.0]);
    }

    #[test]
    fn a_shock_lands_at_its_judges_window_else_at_the_blows_end() {
        let w = |judge, start, end| wscl::HitWindow { judge, start, end };
        let flip = shock_of("hogger_knight", "flip_slam").unwrap();
        let hits = [w(170, 2.37, 2.47), w(180, 2.47, 2.57)];
        assert_eq!(shock_at(&hits, flip, 170), Some(2.47));
        let over = shock_of("hogger_knight", "overhead").unwrap();
        assert_eq!(over.radius_scale, 0.5);
        assert_eq!(
            shock_at(&[w(140, 0.8, 0.9), w(141, 0.8, 0.9)], over, 140),
            Some(0.9)
        );
        assert!(shock_of("hogger_knight", "slash").is_none());
        // Every shock and blast names an attack of a kit.
        for s in SHOCKS {
            let k = KITS.iter().find(|k| k.name == s.kit).unwrap();
            assert!(k.attacks.iter().any(|a| a.name == s.attack), "{}", s.attack);
        }
        for (kit, attack) in BLASTS {
            let k = KITS.iter().find(|k| k.name == *kit).unwrap();
            assert!(k.attacks.iter().any(|a| a.name == *attack), "{attack}");
        }
    }

    #[test]
    fn hogger_knight_wears_artorias() {
        let k = &HOGGER_KNIGHT;
        assert_eq!((k.chr, k.npc, k.variation), ("c4100", 410_000, 41_000));
        assert_eq!(k.tier, Tier::Boss);
        assert!(k.attacks.len() <= k.tier.max_attacks());
        for name in [
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
            "frenzy",
        ] {
            assert!(k.attacks.iter().any(|a| a.name == name), "{name}");
        }
        // `leap` would bake a spider's flight onto a knight's leap.
        assert!(k.attacks.iter().all(|a| a.name != "leap"));
        assert!(KITS.iter().any(|k| k.name == "hogger_knight"));
    }

    #[test]
    fn a_combos_follow_up_window_takes_the_moves_judge() {
        let w = |judge, start| wscl::HitWindow {
            judge,
            start,
            end: start + 0.1,
        };
        let mut hits = vec![w(150, 0.97), w(160, 1.37), w(999, 2.0)];
        merge_follow_judges(&mut hits, "hogger_knight", 160);
        keep_judge(&mut hits, 160);
        assert_eq!(
            hits.iter().map(|h| h.start).collect::<Vec<_>>(),
            [0.97, 1.37]
        );
        // Another kit's windows are left alone.
        let mut other = vec![w(150, 0.97), w(160, 1.37)];
        merge_follow_judges(&mut other, "hogger", 160);
        keep_judge(&mut other, 160);
        assert_eq!(other.len(), 1);
    }

    #[test]
    fn the_install_calls_the_knights_chr_artorias() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let names = crate::names::npc_names(std::path::Path::new(&dir)).unwrap();
        assert_eq!(
            names.get(&4100).map(String::as_str),
            Some("Knight Artorias")
        );
    }

    #[test]
    fn hogger_wears_the_taurus_demons_axe_moves() {
        let k = &HOGGER;
        assert_eq!(k.chr, "c2250");
        assert_eq!(k.tier, Tier::Boss);
        assert!(k.attacks.len() <= k.tier.max_attacks());
        // Every attack names a clip of the demon's attack range, once, under a name of its own (a
        // held-back variant of a move shares its judge with the move: `plunge` and `feint_plunge`).
        let mut clips: Vec<&str> = k.attacks.iter().map(|a| a.clip).collect();
        clips.sort_unstable();
        clips.dedup();
        assert_eq!(clips.len(), k.attacks.len());
        let mut names: Vec<&str> = k.attacks.iter().map(|a| a.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), k.attacks.len());
        assert!(k
            .attacks
            .iter()
            .all(|a| a.clip.starts_with("a00_30") || a.clip == "a00_0701"));
        // A move with no blow of its own carries the window the kit gives it.
        assert!(k
            .attacks
            .iter()
            .filter(|a| a.window.is_some())
            .all(|a| matches!(a.name, "charge" | "hop_back")));
        assert!(KITS.iter().any(|k| k.name == "hogger"));
    }

    #[test]
    fn the_install_calls_hoggers_chr_the_taurus_demon() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let names = crate::names::npc_names(std::path::Path::new(&dir)).unwrap();
        let num: i64 = HOGGER.chr.trim_start_matches('c').parse().unwrap();
        assert_eq!(names.get(&num).map(String::as_str), Some("Taurus Demon"));
    }

    #[test]
    fn a_boss_blow_keeps_only_the_windows_of_its_own_judge() {
        let w = |judge, start| wscl::HitWindow {
            judge,
            start,
            end: start + 0.3,
        };
        // The axe (judge 100) and the shockwave on the ground (101): the axe's damage is not
        // dealt again by the shock.
        let mut hits = vec![w(100, 1.37), w(101, 1.70), w(100, 2.0)];
        keep_judge(&mut hits, 100);
        assert_eq!(hits, vec![w(100, 1.37), w(100, 2.0)]);
        let mut none = vec![w(101, 1.0)];
        keep_judge(&mut none, 100);
        assert!(none.is_empty());
    }

    #[test]
    fn a_boss_without_souls_in_its_row_gives_the_taurus_reward() {
        assert_eq!(souls_for(Tier::Boss, 0), BOSS_SOULS);
        assert_eq!(souls_for(Tier::Boss, 5000), 5000);
        // An ordinary creature with none gives none.
        assert_eq!(souls_for(Tier::Common, 0), 0);
        assert_eq!(souls_for(Tier::Legacy, 150), 150);
        assert_eq!(souls_for(Tier::Common, 750), COMMON_SOULS);
        assert_eq!(souls_for(Tier::Common, 80), 80);
    }

    #[test]
    fn a_boss_may_have_many_heavy_multi_window_attacks() {
        assert_eq!(Tier::Boss.max_attacks(), 12);
        assert!(Tier::Boss.check_attack("smash", 10, 3).is_ok());
        assert!(Tier::Boss.check_attack("too_hard", 11, 1).is_err());
        assert!(Tier::Boss.check_attack("none", 0, 1).is_err());
        // An ordinary creature still may not.
        assert!(Tier::Common.check_attack("smash", 10, 1).is_err());
        assert!(Tier::Elite.check_attack("smash", 10, 1).is_err());
    }

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
    fn the_direwolf_borrows_one_bite_without_replacing_wolfs_reference() {
        assert_eq!(DIREWOLF.chr, WOLF.chr);
        assert_eq!(DIREWOLF.tier, Tier::Common);
        assert_eq!(DIREWOLF.attacks.len(), 1);
        assert_eq!(DIREWOLF.attacks[0].clip, "a00_3000");
        assert_eq!(DIREWOLF.reacts.len(), 7);
        assert_eq!(DIREWOLF.deaths, &["a00_2205"]);
        assert_eq!(DIREWOLF.clips().len(), 9);
        assert_eq!(WOLF.reference_dir(), "c3340");
        assert_eq!(DIREWOLF.reference_dir(), "direwolf_ds");
        for k in [&KOBOLD, &WOLF, &THUG, &GARRICK, &HOGGER] {
            assert_eq!(k.reference_dir, None, "{} keeps its existing files", k.name);
        }
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
