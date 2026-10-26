//! The kobold's pattern: which move it starts and when, as a hand-written table built from the
//! ghoul's real moves (the ghoul's Lua AI is not read; this stands in for it and is replaced,
//! not extended). Pure: the caller owns the random numbers.
//!
//! The kobold *wishes* for a move when it starts closing in (`wish`: weighted over the rows that
//! may open a fight) and starts it once the player is inside that move's band (`choose`); if the
//! player is already inside another band it picks from the rows that fit. After a move it may
//! chain into a follow-up (`follow_up`).

/// One row: a move by name, the player distances (yd, centre to centre) it can start from, its
/// weight as an opener (0: only as a follow-up), and a follow-up with its chance in percent.
pub struct Row {
    pub name: &'static str,
    pub band: (f32, f32),
    pub weight: u32,
    pub then: Option<(&'static str, u32)>,
}

/// Starting table, tuned live. The ghoul's `NpcThinkParam` bands are near 2 m, mid 5, far 10; the
/// kobold's legs are 0.62 of the ghoul's and its lunges follow, so the bands are in kobold yards.
pub const KOBOLD: &[Row] = &[
    Row {
        name: "small_1",
        band: (0.0, 2.0),
        weight: 4,
        then: Some(("small_2", 40)),
    },
    Row {
        name: "small_2",
        band: (0.0, 2.4),
        weight: 0,
        then: Some(("big", 30)),
    },
    Row {
        name: "big",
        band: (0.0, 2.0),
        weight: 2,
        then: None,
    },
    Row {
        name: "body_check",
        band: (0.0, 1.7),
        weight: 1,
        then: None,
    },
    Row {
        name: "run_strike",
        band: (2.2, 3.4),
        weight: 3,
        then: None,
    },
    Row {
        name: "pounce",
        band: (2.2, 3.2),
        weight: 1,
        then: None,
    },
];

/// The Kobold Vermin's: the kobold's two light swings and nothing else, the bands of the kobold's
/// (0.85) scaled to its 0.7 (x 0.82); `small_2` follows `small_1` three times in ten.
pub const VERMIN: &[Row] = &[
    Row {
        name: "small_1",
        band: (0.0, 1.65),
        weight: 4,
        then: Some(("small_2", 30)),
    },
    Row {
        name: "small_2",
        band: (0.0, 1.97),
        weight: 0,
        then: None,
    },
];

/// The Kobold Worker's: the kobold's two light swings and the big one, no charges (the laborer's
/// `run_strike`, `body_check` and `pounce`): it digs, it does not hunt.
pub const WORKER: &[Row] = &[
    Row {
        name: "small_1",
        band: (0.0, 2.0),
        weight: 4,
        then: Some(("small_2", 40)),
    },
    Row {
        name: "small_2",
        band: (0.0, 2.4),
        weight: 0,
        then: Some(("big", 30)),
    },
    Row {
        name: "big",
        band: (0.0, 2.0),
        weight: 2,
        then: None,
    },
];

/// New Elwynn kobolds borrow only the ghoul's two single-hit light swings. The held tools have
/// shorter reach than the laborer's extended hand volume: begin close enough for the actual
/// blade, rather than scaling the old 2 yd hand band. The first swing advances about 0.6 model
/// yards by its hit window; these conservative bands leave the tool within striking distance.
const fn melee_kobold(scale: f32) -> [Row; 2] {
    [
        Row {
            name: "small_1",
            band: (0.0, 1.15 * scale),
            weight: 4,
            then: Some(("small_2", 40)),
        },
        Row {
            name: "small_2",
            band: (0.0, 1.25 * scale),
            weight: 0,
            then: None,
        },
    ]
}

pub const MINER: &[Row] = &melee_kobold(1.15);
pub const TUNNELER: &[Row] = &melee_kobold(1.0);

/// The wolf's: the black dog's two borrowed moves, a bite close in and a lunge from a few yards
/// out. At the Timber Wolf's entity scale (0.55) the lunge carries it about 1.0 yd by the time its hit
/// window is open (0.33-0.70 s, scaled from the live 1.2 yd at 0.65) and 1.4 yd in all, so it must
/// start about 2 yd off to reach a player standing still; the bands are in wolf yards, tuned live.
pub const WOLF: &[Row] = &[
    Row {
        name: "bite",
        band: (0.0, 1.3),
        weight: 3,
        then: None,
    },
    Row {
        name: "lunge",
        band: (1.6, 2.4),
        weight: 2,
        then: None,
    },
];

/// The larger Elwynn wolves use the same Wolf rig and two dog moves. Scale the start bands
/// with the model, so their longer root motion starts its lunge further away.
const fn forest_wolf(scale: f32) -> [Row; 2] {
    let ratio = scale / 0.55;
    [
        Row {
            name: "bite",
            band: (0.0, 1.3 * ratio),
            weight: 3,
            then: None,
        },
        Row {
            name: "lunge",
            band: (1.6 * ratio, 2.4 * ratio),
            weight: 2,
            then: None,
        },
    ]
}

pub const MANGY_WOLF: &[Row] = &forest_wolf(0.7);
pub const GRAY_FOREST_WOLF: &[Row] = &forest_wolf(0.85);

/// DireWolf's verified bite only. Its lunges fail the existing foot-contact gate; keep
/// approaching a nearby target rather than selecting an unsuitable distant attack.
pub const PROWLER: &[Row] = &[Row {
    name: "bite",
    band: (0.0, 1.5),
    weight: 3,
    then: None,
}];

/// The Young Wolf's: the black dog's bite alone, the Timber Wolf's band (0-1.3 at 0.55) scaled to its
/// 0.4 (x 0.73).
pub const YOUNG_WOLF: &[Row] = &[Row {
    name: "bite",
    band: (0.0, 0.95),
    weight: 3,
    then: None,
}];

/// The Defias Thug's: the undead thief's two borrowed slashes. `slash_1` carries it about 1.5 yd and
/// hits at 0.87 s, so it starts from a few yards out; `slash_2` is the close one and follows
/// `slash_1` four times in ten. The bands are in human yards, tuned live.
pub const THUG: &[Row] = &[
    Row {
        name: "slash_1",
        band: (0.9, 2.6),
        weight: 3,
        then: Some(("slash_2", 40)),
    },
    Row {
        name: "slash_2",
        band: (0.0, 1.8),
        weight: 2,
        then: None,
    },
];

/// The Riverpaw gnolls': the undead soldier's two borrowed club swings. `swing_1` (`body_attack_01`) carries it
/// about 0.75 yd and hits at 0.87 s, so it starts from a few yards out; `swing_2` (`body_attack_05`) is the quick
/// one (hit at 0.37 s) and follows `swing_1` a third of the time. Bands in the gnoll's own yards,
/// tuned live.
pub const GNOLL: &[Row] = &[
    Row {
        name: "swing_1",
        band: (0.9, 2.5),
        weight: 3,
        then: Some(("swing_2", 35)),
    },
    Row {
        name: "swing_2",
        band: (0.0, 1.8),
        weight: 2,
        then: None,
    },
];

/// The murlocs': the same two club swings on a body 0.78 of the soldier's size, so the bands shrink with it.
/// The murlocs (2026-10-18, Elden Ring's demi-humans as the model): a running lunge from a few yards closes in,
/// the three-swing flurry stumbles on through whatever stands close; a lunge that lands often goes straight
/// into the flurry.
pub const MURLOC: &[Row] = &[
    Row {
        name: "lunge",
        band: (2.4, 4.4),
        weight: 3,
        then: Some(("flurry", 40)),
    },
    Row {
        name: "flurry",
        band: (0.0, 2.4),
        weight: 3,
        then: None,
    },
];

/// The full-size murlocs (Forager, Lurker: display scale 1).
pub const MURLOC_LARGE: &[Row] = &[
    Row {
        name: "lunge",
        band: (3.0, 5.6),
        weight: 3,
        then: Some(("flurry", 40)),
    },
    Row {
        name: "flurry",
        band: (0.0, 3.0),
        weight: 3,
        then: None,
    },
];

/// The boars': the Armored Tusk's gore close in, and from four to ten yards out the charge, a run through
/// the player ([`crate::boar_rush`], 2026-10-17; it was a short lunge from 1.6-2.8 yd).
pub const BOAR: &[Row] = &[
    Row {
        name: "gore",
        band: (0.0, 1.4),
        weight: 3,
        then: None,
    },
    Row {
        name: "charge",
        band: (4.0, 10.0),
        weight: 2,
        then: None,
    },
];

/// The larger boars (display 0.75): the bands scale by 0.75 / 0.6.
pub const BOAR_LARGE: &[Row] = &[
    Row {
        name: "gore",
        band: (0.0, 1.75),
        weight: 3,
        then: None,
    },
    Row {
        name: "charge",
        band: (4.5, 12.0),
        weight: 2,
        then: None,
    },
];

/// The bear's: the Great Feline's lunging bite (it dives to the ground half a yard ahead) and the drake's claw
/// swipe (a wide sweep of the left forepaw, carrying the bear 1.2 yd), both close in; tuned live.
pub const BEAR: &[Row] = &[
    Row {
        name: "bite",
        band: (0.0, 2.0),
        weight: 3,
        then: None,
    },
    Row {
        name: "swipe",
        band: (0.0, 2.0),
        weight: 2,
        then: None,
    },
];

/// The Dockmaster's: the thug's two slashes as punches on a goblin body 0.47 of the thug's effective size,
/// so the bands shrink with it; tuned live.
pub const GOBLIN: &[Row] = &[
    Row {
        name: "slash_1",
        band: (0.4, 1.4),
        weight: 3,
        then: Some(("slash_2", 40)),
    },
    Row {
        name: "slash_2",
        band: (0.0, 1.1),
        weight: 2,
        then: None,
    },
];

/// The spiders': the claw `bite` close in, and the `leap` from a few yards: a hop that commits to where the
/// player stood and bites on landing; a leap that lands holds on and chews, then `hop_back` (never chosen
/// on its own: weight 0, the latch sends it). Bands are in yards of the drawn size (the hop's 5.6 yd of kit
/// travel times the display scale: 3.1 yd for the Mine Spider, 2.2 for the Forest Spider, 3.9 for Mother Fang).
/// Tuned live.
/// The hairy spiders' own (2026-10-18, the user: their leap and latch went through the world): a spitter that
/// keeps its distance. It spits venom from range, bites what comes close, and after a bite often hops back
/// out and spits again.
pub const FOREST_SPIDER: &[Row] = &[
    Row {
        name: "bite",
        band: (0.0, 1.3),
        weight: 3,
        then: Some(("hop_back", 45)),
    },
    Row {
        name: "spit",
        band: (2.5, 9.0),
        weight: 3,
        then: None,
    },
    Row {
        name: "hop_back",
        band: (0.0, 9.0),
        weight: 0,
        then: Some(("spit", 70)),
    },
];

pub const MINE_SPIDER: &[Row] = &[
    Row {
        name: "bite",
        band: (0.0, 2.1),
        weight: 3,
        then: None,
    },
    Row {
        name: "leap",
        band: (2.6, 4.0),
        weight: 2,
        then: None,
    },
    Row {
        name: "hop_back",
        band: (0.0, 9.0),
        weight: 0,
        then: None,
    },
];

/// The casters' (Rogue Wizard, Surena): the bolt from a distance (the cast thrusts the staff, the bolt flies on
/// from the hand), the staff's swat when the player is on them; tuned live.
pub const CASTER: &[Row] = &[
    Row {
        name: "bolt",
        band: (2.5, 14.0),
        weight: 4,
        then: None,
    },
    Row {
        name: "swat",
        band: (0.0, 2.0),
        weight: 3,
        then: None,
    },
];

/// The Kobold Geomancer's: the bolt from a distance, the ghoul's light swing (a kobold's reach) close in.
pub const GEOMANCER: &[Row] = &[
    Row {
        name: "bolt",
        band: (2.5, 14.0),
        weight: 4,
        then: None,
    },
    Row {
        name: "small_1",
        band: (0.0, 1.3),
        weight: 3,
        then: None,
    },
];

/// A single sword swing plus Snap Kick at close range; both bodies use the same two roles.
pub const BANDIT: &[Row] = &[
    Row {
        name: "sword",
        band: (0.6, 2.3),
        weight: 4,
        then: Some(("kick", 30)),
    },
    Row {
        name: "kick",
        band: (0.0, 1.25),
        weight: 2,
        then: None,
    },
];

/// Garrick's: the assassin's five borrowed moves. `quick` (1.37 s, hit 0.33 s) is the close poke and
/// leads into the double a third of the time; `double` (two thrusts at 0.53 and 1.17 s, 3.35 yd of
/// travel) and `triple` (three hits, 2.1 yd) are the mid-range combos; `lunge` (3.97 yd, hit at 0.87
/// s) and `dash` (4.53 yd, hit at 0.50 s) close a gap. The bands are in human yards, tuned live.
pub const GARRICK: &[Row] = &[
    Row {
        name: "quick",
        band: (0.0, 1.8),
        weight: 3,
        then: Some(("double", 35)),
    },
    Row {
        name: "double",
        band: (0.0, 3.0),
        weight: 3,
        then: None,
    },
    Row {
        name: "triple",
        band: (0.6, 2.6),
        weight: 2,
        then: None,
    },
    Row {
        name: "lunge",
        band: (2.4, 4.2),
        weight: 2,
        then: None,
    },
    Row {
        name: "dash",
        band: (3.0, 4.8),
        weight: 1,
        then: None,
    },
];

/// Hogger's: the Taurus Demon's axe moves, bands in yards of the drawn size (the demon's think
/// distances near 3.0 / mid 5.0 / far 10.0 DS m times the kit's scale 0.307 times Hogger's drawn
/// scale 1.5, then tuned live). `overhead` and `quick` open a fight at the player's feet and chain;
/// `step_smash` and `sweep` take a step in; `lunge_smash` closes a gap; `feint_plunge` is the slam in
/// place at the player's feet, held back a beat to catch an early roll (its clip has no travel: it
/// is a close-range move). The demon's leap `plunge` (`body_attack_09`) is a jump down from a tower and
/// floats on flat ground: it is not in the kit.
pub const HOGGER: &[Row] = &[
    Row {
        name: "charge",
        band: (2.6, 7.0),
        weight: 2,
        then: None,
    },
    Row {
        name: "hop_back",
        band: (0.0, 1.8),
        weight: 1,
        then: None,
    },
    Row {
        name: "overhead",
        band: (0.0, 2.2),
        weight: 3,
        then: Some(("backhand", 45)),
    },
    Row {
        name: "quick",
        band: (0.0, 2.0),
        weight: 3,
        then: Some(("overhead", 40)),
    },
    Row {
        name: "step_smash",
        band: (1.0, 3.8),
        weight: 3,
        then: Some(("sweep", 35)),
    },
    Row {
        name: "sweep",
        band: (0.0, 2.5),
        weight: 2,
        then: Some(("heavy_swing", 30)),
    },
    Row {
        name: "heavy_swing",
        band: (0.0, 2.2),
        weight: 1,
        then: None,
    },
    Row {
        name: "backhand",
        band: (0.0, 2.1),
        weight: 0,
        then: None,
    },
    Row {
        name: "lunge_smash",
        band: (2.2, 4.4),
        weight: 2,
        then: Some(("quick", 35)),
    },
    Row {
        name: "feint_plunge",
        band: (0.0, 2.4),
        weight: 2,
        then: Some(("quick", 30)),
    },
];

/// Hogger as Knight Artorias (kit `hogger_knight`, scale 0.5049, drawn 1.3: one DS metre is 0.66 yd). The
/// knight's think bands near 3 / mid 6 / far 20 DS m are 2.0 / 3.9 / 13 yd. Close in he cuts and chains (`slash`,
/// `close_two`, the jab `head_butt`); a few yards off he steps or spins in; from range he leaps (`pounce`, 10 m),
/// lunges (`lunge`, 11 m, the opener) or thrusts (`thrust`, 9.5 m), and from far off the flip slam (15 m) comes
/// down on the player. The big slams (`flip_slam`, `overhead`) weigh least. The upper bands are each move's
/// travel to its blow plus the blade; first guesses, tuned live.
const fn row(name: &'static str, band: (f32, f32), weight: u32) -> Row {
    Row {
        name,
        band,
        weight,
        then: None,
    }
}

pub const HOGGER_KNIGHT: &[Row] = &[
    row("slash", (1.4, 2.8), 3),
    row("close_two", (1.2, 2.2), 3),
    row("head_butt", (1.0, 2.0), 2),
    row("step_slash", (2.4, 4.6), 3),
    row("spin", (1.2, 2.8), 2),
    row("overhead", (1.2, 4.4), 1),
    row("pounce", (3.5, 7.5), 2),
    row("lunge", (4.0, 8.5), 2),
    row("thrust", (3.5, 7.0), 1),
    row("flip_slam", (5.5, 10.5), 1),
    row("backflip", (0.0, 1.4), 1),
];

/// The knight enraged: the gap closers and the big slams come more often.
pub const HOGGER_KNIGHT_TWO: &[Row] = &[
    row("slash", (1.4, 2.8), 3),
    row("close_two", (1.2, 2.2), 3),
    row("head_butt", (1.0, 2.0), 2),
    row("step_slash", (2.4, 4.6), 3),
    row("spin", (1.2, 2.8), 3),
    row("overhead", (1.2, 4.4), 2),
    row("pounce", (3.5, 7.5), 3),
    row("lunge", (4.0, 8.5), 3),
    row("thrust", (3.5, 7.0), 2),
    row("flip_slam", (5.5, 10.5), 2),
    row("backflip", (0.0, 1.4), 2),
];

/// Hogger enraged (half health): more chains, and the heavy ones become openers.
pub const HOGGER_TWO: &[Row] = &[
    Row {
        name: "charge",
        band: (2.6, 7.0),
        weight: 3,
        then: None,
    },
    Row {
        name: "hop_back",
        band: (0.0, 1.8),
        weight: 2,
        then: None,
    },
    Row {
        name: "overhead",
        band: (0.0, 2.2),
        weight: 3,
        then: Some(("backhand", 65)),
    },
    Row {
        name: "quick",
        band: (0.0, 2.0),
        weight: 3,
        then: Some(("overhead", 60)),
    },
    Row {
        name: "step_smash",
        band: (1.0, 3.8),
        weight: 3,
        then: Some(("sweep", 55)),
    },
    Row {
        name: "sweep",
        band: (0.0, 2.5),
        weight: 2,
        then: Some(("heavy_swing", 45)),
    },
    Row {
        name: "heavy_swing",
        band: (0.0, 2.2),
        weight: 2,
        then: Some(("quick", 40)),
    },
    Row {
        name: "backhand",
        band: (0.0, 2.1),
        weight: 0,
        then: None,
    },
    Row {
        name: "lunge_smash",
        band: (2.2, 4.4),
        weight: 2,
        then: Some(("quick", 55)),
    },
    Row {
        name: "feint_plunge",
        band: (0.0, 2.4),
        weight: 2,
        then: Some(("overhead", 45)),
    },
];

/// A move held back at its wind-up: Dark Souls' delayed swing, which catches the roll made at the
/// tell. `at` is the clip second where the axe waits (its raised pose), `hold` the seconds it
/// waits (a range, picked once per swing) and `chance` how often (percent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feint {
    pub at: f32,
    pub hold: (f32, f32),
    pub chance: u32,
}

/// How a move is carried out beyond its table row (a boss's rhythm): what may follow it, how much
/// of its wind-up a follow-up skips, where it is cut short and whether it is held back. A creature
/// with no styles runs on the table's `then` alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub name: &'static str,
    /// What may follow, each with its chance (percent); their sum is the chance of a follow-up
    /// at all. The pick is made when the move starts (so its recovery can be cut for the link)
    /// and holds if the player is inside the follow-up's band when it ends.
    pub then: &'static [(&'static str, u32)],
    /// Clip seconds skipped when the move starts as a follow-up: a combo's blows come closer than
    /// an opener's.
    pub link: f32,
    /// The move ends at this clip second (a hop is over when it lands).
    pub cut: Option<f32>,
    pub feint: Option<Feint>,
    /// The move is what a fight opens with, once, when the player is far off.
    pub opening: bool,
}

const fn style(
    name: &'static str,
    then: &'static [(&'static str, u32)],
    link: f32,
    feint: Option<Feint>,
) -> Style {
    Style {
        name,
        then,
        link,
        cut: None,
        feint,
        opening: false,
    }
}

const fn feint(at: f32, lo: f32, hi: f32, chance: u32) -> Option<Feint> {
    Some(Feint {
        at,
        hold: (lo, hi),
        chance,
    })
}

/// How long a chained move's recovery lasts (clip s after its last hit window): a combo links
/// its blows instead of waiting out the pause of a move that stands alone.
pub const CHAIN_RECOVERY: f32 = 0.5;

/// Hogger's: a rhythm of combos. A charge ends in a heavy blow, a hop back disengages and charges,
/// the quick moves chain three deep, and the big wind-ups are held at the top now and then. The
/// feint seconds are where the axe is raised in each clip (the hit windows open at 1.37 s for the
/// overhead, 1.97 s for the sweep and 2.67 s for the heavy swing).
pub const HOGGER_STYLE: &[Style] = &[
    Style {
        opening: true,
        ..style(
            "charge",
            &[
                ("overhead", 35),
                ("step_smash", 25),
                ("lunge_smash", 15),
                ("quick", 15),
            ],
            0.0,
            None,
        )
    },
    Style {
        cut: Some(1.25),
        ..style(
            "hop_back",
            &[("charge", 75), ("lunge_smash", 15)],
            0.0,
            None,
        )
    },
    style(
        "overhead",
        &[("backhand", 30), ("quick", 20), ("step_smash", 10)],
        0.45,
        feint(1.05, 0.35, 0.65, 30),
    ),
    style(
        "quick",
        &[("overhead", 30), ("backhand", 25), ("sweep", 10)],
        0.4,
        None,
    ),
    style(
        "step_smash",
        &[("sweep", 30), ("quick", 20), ("heavy_swing", 10)],
        0.4,
        None,
    ),
    style(
        "sweep",
        &[("heavy_swing", 20), ("backhand", 25), ("overhead", 15)],
        0.6,
        feint(1.5, 0.3, 0.55, 25),
    ),
    style(
        "heavy_swing",
        &[("quick", 30)],
        0.8,
        feint(2.2, 0.45, 0.85, 40),
    ),
    style("backhand", &[("overhead", 25)], 0.4, None),
    style("lunge_smash", &[("quick", 30), ("overhead", 20)], 0.5, None),
    style(
        "feint_plunge",
        &[("quick", 25), ("backhand", 20)],
        0.3,
        None,
    ),
];

/// Hogger enraged: the chains are longer and the feints likelier, the hop and the charge come
/// more often.
pub const HOGGER_TWO_STYLE: &[Style] = &[
    Style {
        opening: true,
        ..style(
            "charge",
            &[
                ("overhead", 35),
                ("step_smash", 25),
                ("lunge_smash", 20),
                ("quick", 20),
            ],
            0.0,
            None,
        )
    },
    Style {
        cut: Some(1.15),
        ..style(
            "hop_back",
            &[("charge", 85), ("lunge_smash", 15)],
            0.0,
            None,
        )
    },
    style(
        "overhead",
        &[("backhand", 40), ("quick", 25), ("step_smash", 15)],
        0.5,
        feint(1.05, 0.3, 0.6, 40),
    ),
    style(
        "quick",
        &[("overhead", 35), ("backhand", 30), ("sweep", 15)],
        0.45,
        None,
    ),
    style(
        "step_smash",
        &[("sweep", 35), ("quick", 25), ("heavy_swing", 15)],
        0.45,
        None,
    ),
    style(
        "sweep",
        &[("heavy_swing", 25), ("backhand", 30), ("overhead", 20)],
        0.65,
        feint(1.5, 0.3, 0.5, 35),
    ),
    style(
        "heavy_swing",
        &[("quick", 35), ("overhead", 20)],
        0.85,
        feint(2.2, 0.4, 0.8, 45),
    ),
    style("backhand", &[("overhead", 35), ("sweep", 15)], 0.45, None),
    style(
        "lunge_smash",
        &[("quick", 35), ("overhead", 25)],
        0.55,
        None,
    ),
    style(
        "feint_plunge",
        &[("quick", 30), ("backhand", 25)],
        0.35,
        None,
    ),
];

/// Hogger the knight: he does not let go. Chains of two or three, a disengage (`backflip`) that comes straight
/// back in with a leap or a lunge, and every chain may end, leaving an opening. No feints: the knight's tells are
/// short already, and aggression without tells is the failure to avoid (spec §8).
pub const HOGGER_KNIGHT_STYLE: &[Style] = &[
    Style {
        opening: true,
        ..style(
            "lunge",
            &[("slash", 30), ("step_slash", 20), ("spin", 10)],
            0.0,
            None,
        )
    },
    style(
        "slash",
        &[
            ("close_two", 25),
            ("step_slash", 15),
            ("head_butt", 10),
            ("backflip", 10),
        ],
        0.3,
        None,
    ),
    style(
        "close_two",
        &[("slash", 25), ("spin", 20), ("backflip", 10)],
        0.3,
        None,
    ),
    style("head_butt", &[("slash", 35), ("overhead", 20)], 0.2, None),
    style(
        "step_slash",
        &[("slash", 25), ("spin", 20), ("close_two", 15)],
        0.3,
        None,
    ),
    style(
        "spin",
        &[("slash", 20), ("backflip", 20), ("overhead", 10)],
        0.2,
        None,
    ),
    style("overhead", &[("slash", 25), ("close_two", 25)], 0.3, None),
    style(
        "pounce",
        &[("slash", 30), ("close_two", 20), ("spin", 10)],
        0.0,
        None,
    ),
    style("thrust", &[("slash", 30), ("close_two", 20)], 0.0, None),
    style("flip_slam", &[("slash", 20), ("backflip", 20)], 0.0, None),
    style(
        "backflip",
        &[("pounce", 35), ("lunge", 25), ("thrust", 10)],
        0.0,
        None,
    ),
];

/// The knight enraged: longer chains, and every disengage comes back in.
pub const HOGGER_KNIGHT_TWO_STYLE: &[Style] = &[
    Style {
        opening: true,
        ..style(
            "lunge",
            &[("slash", 35), ("step_slash", 25), ("spin", 15)],
            0.0,
            None,
        )
    },
    style(
        "slash",
        &[
            ("close_two", 30),
            ("step_slash", 20),
            ("head_butt", 15),
            ("backflip", 15),
        ],
        0.35,
        None,
    ),
    style(
        "close_two",
        &[("slash", 30), ("spin", 25), ("backflip", 15)],
        0.35,
        None,
    ),
    style("head_butt", &[("slash", 40), ("overhead", 30)], 0.25, None),
    style(
        "step_slash",
        &[("slash", 30), ("spin", 25), ("close_two", 20)],
        0.35,
        None,
    ),
    style(
        "spin",
        &[("slash", 25), ("backflip", 25), ("overhead", 20)],
        0.25,
        None,
    ),
    style(
        "overhead",
        &[("slash", 30), ("close_two", 30), ("spin", 10)],
        0.35,
        None,
    ),
    style(
        "pounce",
        &[("slash", 35), ("close_two", 25), ("spin", 15)],
        0.0,
        None,
    ),
    style(
        "thrust",
        &[("slash", 35), ("close_two", 25), ("spin", 10)],
        0.0,
        None,
    ),
    style(
        "flip_slam",
        &[("slash", 30), ("backflip", 25), ("spin", 15)],
        0.0,
        None,
    ),
    style(
        "backflip",
        &[("pounce", 40), ("lunge", 30), ("flip_slam", 15)],
        0.0,
        None,
    ),
];

// ---------------------------------------------------------------------------------------------
// Elwynn's rares and named quest mobs (spec 2026-10-17 elwynn-specials): each its own Elite kit (the family's
// source, more of its moves) and its own rhythm. Bands in the family's yards, first guesses, tuned live.

/// Thuros Lightfingers: three quick cuts in a row, the lunge from a few yards off.
pub const THUROS: &[Row] = &[
    row("cut_1", (0.0, 2.0), 4),
    row("cut_2", (0.0, 1.8), 0),
    row("slash_1", (0.9, 2.6), 2),
    row("slash_2", (0.0, 1.8), 1),
    row("lunge", (2.4, 4.2), 2),
];
pub const THUROS_STYLE: &[Style] = &[
    style("cut_1", &[("cut_2", 65), ("slash_2", 10)], 0.25, None),
    style("cut_2", &[("slash_1", 50)], 0.25, None),
    style("slash_1", &[("slash_2", 35), ("cut_1", 20)], 0.3, None),
    style("slash_2", &[("cut_1", 25)], 0.3, None),
    Style {
        opening: true,
        ..style("lunge", &[("cut_1", 45), ("slash_2", 20)], 0.0, None)
    },
];

/// Morgaine the Sly: her swings are held at the top now and then, so a roll made at the tell is over when the
/// blade comes.
pub const MORGAINE: &[Row] = &[
    row("slash_1", (0.9, 2.6), 3),
    row("slash_2", (0.0, 1.8), 2),
    row("cut_1", (0.0, 2.0), 3),
    row("cut_2", (0.0, 1.8), 0),
];
pub const MORGAINE_STYLE: &[Style] = &[
    style(
        "slash_1",
        &[("slash_2", 35), ("cut_1", 20)],
        0.3,
        feint(0.6, 0.3, 0.6, 40),
    ),
    style("slash_2", &[("cut_1", 30)], 0.3, feint(0.5, 0.3, 0.55, 35)),
    style("cut_1", &[("cut_2", 55), ("slash_1", 15)], 0.25, None),
    style(
        "cut_2",
        &[("slash_1", 30)],
        0.25,
        feint(0.35, 0.25, 0.45, 25),
    ),
];

/// Morgan the Collector: cuts and steps back out of reach, then lunges in again.
pub const MORGAN: &[Row] = &[
    row("slash_1", (0.9, 2.6), 3),
    row("slash_2", (0.0, 1.8), 2),
    row("lunge", (2.4, 4.2), 2),
    row("hop_back", (0.0, 1.6), 1),
];
pub const MORGAN_STYLE: &[Style] = &[
    style("slash_1", &[("hop_back", 35), ("slash_2", 30)], 0.3, None),
    style("slash_2", &[("hop_back", 40)], 0.3, None),
    Style {
        cut: Some(1.0),
        ..style("hop_back", &[("lunge", 70)], 0.0, None)
    },
    Style {
        opening: true,
        ..style("lunge", &[("slash_1", 40), ("slash_2", 20)], 0.0, None)
    },
];

/// Dead-Tooth Jack: the axe's three-cut chain, the kick that breaks a guard and a cut behind it.
pub const DEAD_TOOTH_JACK: &[Row] = &[
    row("r1_1", (0.6, 2.3), 4),
    row("r1_2", (0.0, 2.3), 0),
    row("r1_3", (0.0, 2.3), 0),
    row("kick", (0.0, 1.4), 2),
    row("roll_attack", (2.0, 3.5), 2),
];
pub const DEAD_TOOTH_JACK_STYLE: &[Style] = &[
    style("r1_1", &[("r1_2", 55), ("kick", 15)], 0.3, None),
    style("r1_2", &[("r1_3", 50), ("kick", 15)], 0.3, None),
    style("r1_3", &[("kick", 20)], 0.3, None),
    style("kick", &[("r1_1", 55)], 0.2, None),
    Style {
        opening: true,
        ..style("roll_attack", &[("r1_1", 35), ("kick", 20)], 0.0, None)
    },
];

/// James Clark: the sword pair, the thrust that reaches a backing player, the kick.
pub const JAMES_CLARK: &[Row] = &[
    row("r1_1", (0.6, 2.3), 4),
    row("r1_2", (0.0, 2.3), 0),
    row("thrust", (0.8, 2.6), 2),
    row("kick", (0.0, 1.4), 2),
    row("roll_attack", (2.0, 3.5), 2),
];
pub const JAMES_CLARK_STYLE: &[Style] = &[
    style("r1_1", &[("r1_2", 45), ("thrust", 20)], 0.3, None),
    style("r1_2", &[("thrust", 30), ("kick", 15)], 0.3, None),
    style("thrust", &[("r1_1", 25)], 0.35, feint(0.45, 0.3, 0.55, 30)),
    style("kick", &[("thrust", 50), ("r1_1", 20)], 0.2, None),
    Style {
        opening: true,
        ..style("roll_attack", &[("r1_1", 35)], 0.0, None)
    },
];

/// Erlan Drudgemoor: a slow axe flow, the heavy pair, the rolling cut from a few yards off.
pub const ERLAN_ELITE: &[Row] = &[
    row("axe_1", (0.0, 2.2), 3),
    row("axe_2", (0.0, 2.2), 0),
    row("heavy_1", (0.0, 2.4), 2),
    row("heavy_2", (0.0, 2.4), 0),
    // The rolling cut barely carries him (0.2 yd): from inside a wide band he would only ever roll.
    row("roll_attack", (3.0, 4.0), 1),
];
pub const ERLAN_STYLE: &[Style] = &[
    style("axe_1", &[("axe_2", 45), ("heavy_1", 15)], 0.35, None),
    style("axe_2", &[("heavy_1", 25)], 0.35, None),
    style(
        "heavy_1",
        &[("heavy_2", 55)],
        0.4,
        feint(0.55, 0.3, 0.6, 30),
    ),
    style("heavy_2", &[("axe_1", 20)], 0.4, None),
    Style {
        opening: true,
        ..style("roll_attack", &[("axe_1", 40)], 0.0, None)
    },
];

/// Surena Caledon: two bolts, the heavy one slower and larger; a player who closes meets the swat and the slash.
pub const SURENA: &[Row] = &[
    row("bolt", (2.5, 14.0), 3),
    row("heavy_bolt", (3.0, 14.0), 3),
    row("swat", (0.0, 2.0), 3),
    row("slash", (0.0, 1.8), 1),
];
pub const SURENA_STYLE: &[Style] = &[
    style("bolt", &[("heavy_bolt", 30), ("bolt", 20)], 0.2, None),
    style("heavy_bolt", &[("bolt", 30)], 0.2, None),
    style("swat", &[("slash", 45)], 0.3, None),
    style("slash", &[("swat", 20)], 0.3, None),
];

/// Goldtooth: quick jabs in and out, a running strike and a pounce from a few yards off.
pub const GOLDTOOTH_ELITE: &[Row] = &[
    row("small_1", (0.0, 1.35), 4),
    row("small_2", (0.0, 1.45), 0),
    row("run_strike", (2.0, 3.4), 3),
    row("pounce", (2.2, 3.2), 2),
];
pub const GOLDTOOTH_STYLE: &[Style] = &[
    style("small_1", &[("small_2", 50)], 0.3, None),
    style("small_2", &[("small_1", 20)], 0.3, None),
    Style {
        opening: true,
        ..style("run_strike", &[("small_1", 40)], 0.0, None)
    },
    style("pounce", &[("small_1", 35)], 0.0, None),
];

/// Ruklar the Trapper: the net is thrown with the overhead (`named.rs`), and an overhead follows it; the shove breaks
/// a guard. His hatchet sits off the hammer's axis: on his 1.3-scale body the light swings pass above the player, so
/// his kit's pair is not in his table.
pub const RUKLAR_ELITE: &[Row] = &[row("big", (0.0, 1.5), 3), row("body_check", (0.0, 1.4), 2)];
pub const RUKLAR_STYLE: &[Style] = &[
    // A miniboss (2026-10-18): the overhead now and then holds at its height (a Souls delay) before it falls.
    style(
        "big",
        &[("big", 35), ("body_check", 25)],
        0.3,
        feint(0.55, 0.3, 0.6, 35),
    ),
    style("body_check", &[("big", 60)], 0.25, None),
];

/// Narg the Taskmaster holds his ground: the shove that breaks a guard, and the overhead behind it.
pub const NARG_ELITE: &[Row] = &[
    row("small_1", (0.0, 1.5), 3),
    row("small_2", (0.0, 1.6), 0),
    row("body_check", (0.0, 1.5), 3),
    row("big", (0.0, 1.6), 1),
];
pub const NARG_STYLE: &[Style] = &[
    style("small_1", &[("small_2", 50), ("body_check", 15)], 0.3, None),
    style("small_2", &[("body_check", 30)], 0.3, None),
    style("body_check", &[("big", 60), ("small_1", 15)], 0.25, None),
    style("big", &[("small_1", 20)], 0.3, feint(0.9, 0.3, 0.6, 30)),
];

/// Gruff Swiftbite: the soldier's three-blow combo and heavy lunge on top of the pair (Thrash still chains it).
pub const GRUFF: &[Row] = &[
    row("swing_1", (0.9, 2.5), 3),
    row("swing_2", (0.0, 1.8), 2),
    row("triple", (0.6, 2.8), 2),
    row("heavy_lunge", (1.6, 3.4), 2),
    row("heavy", (1.4, 3.2), 1),
];
pub const GRUFF_STYLE: &[Style] = &[
    style("swing_1", &[("swing_2", 40), ("triple", 20)], 0.3, None),
    style("swing_2", &[("heavy", 25)], 0.3, None),
    style("triple", &[("heavy", 25)], 0.3, None),
    Style {
        opening: true,
        ..style("heavy_lunge", &[("swing_1", 40), ("triple", 20)], 0.0, None)
    },
    style("heavy", &[("swing_1", 30)], 0.35, None),
];

/// Fedfennel: a quick cut opens the guard-breaking heavy cut or the slow overhead.
pub const FEDFENNEL: &[Row] = &[
    row("swing_1", (0.9, 2.5), 3),
    row("swing_2", (0.0, 1.8), 2),
    row("quick", (0.0, 1.8), 2),
    row("heavy", (1.4, 3.2), 1),
    row("overhead", (0.0, 2.0), 1),
];
pub const FEDFENNEL_STYLE: &[Style] = &[
    style("swing_1", &[("swing_2", 40), ("quick", 20)], 0.3, None),
    style("swing_2", &[("heavy", 25)], 0.3, None),
    style("quick", &[("heavy", 45), ("overhead", 25)], 0.25, None),
    style("heavy", &[("swing_1", 25)], 0.35, None),
    style(
        "overhead",
        &[("swing_1", 25)],
        0.4,
        feint(1.0, 0.3, 0.6, 35),
    ),
];

/// Princess: the long charge from across the clearing, the double gore close in, the rear-up.
pub const PRINCESS: &[Row] = &[
    row("gore", (0.0, 1.75), 3),
    row("double_gore", (0.0, 1.9), 2),
    row("charge", (2.5, 6.0), 3),
    row("long_charge", (6.0, 12.0), 2),
    row("rear", (0.0, 2.2), 1),
];
pub const PRINCESS_STYLE: &[Style] = &[
    style("gore", &[("rear", 25), ("double_gore", 15)], 0.3, None),
    style("double_gore", &[("gore", 20)], 0.3, None),
    style("charge", &[("double_gore", 35)], 0.0, None),
    Style {
        opening: true,
        ..style(
            "long_charge",
            &[("double_gore", 40), ("gore", 20)],
            0.0,
            None,
        )
    },
    style("rear", &[("gore", 30)], 0.3, None),
];

/// Mother Fang: bites twice, and her Web (thrown with the bite, `named.rs`) is followed by the leap.
pub const MOTHER_FANG_ELITE: &[Row] = &[
    row("bite", (0.0, 3.2), 3),
    row("leap", (3.6, 6.0), 2),
    row("hop_back", (0.0, 9.0), 0),
    // A miniboss (2026-10-19): she spits venom from range, and hops back to spit again.
    row("spit", (4.0, 10.0), 2),
];
pub const MOTHER_FANG_STYLE: &[Style] = &[
    style(
        "bite",
        &[("bite", 35), ("leap", 30), ("hop_back", 20)],
        0.25,
        None,
    ),
    style("leap", &[("bite", 45)], 0.0, None),
    style("hop_back", &[("spit", 70)], 0.0, None),
    style("spit", &[("leap", 35)], 0.2, None),
];

/// The styles of a kit in a phase (keyed by kit, so the Taurus Hogger and the knight keep their own); none for
/// the creatures that run on their table alone.
pub fn styles(kit: &str, second_phase: bool) -> &'static [Style] {
    match (kit, second_phase) {
        ("hogger", false) => HOGGER_STYLE,
        ("hogger", true) => HOGGER_TWO_STYLE,
        ("hogger_knight", false) => HOGGER_KNIGHT_STYLE,
        ("hogger_knight", true) => HOGGER_KNIGHT_TWO_STYLE,
        ("thuros", _) => THUROS_STYLE,
        ("morgaine", _) => MORGAINE_STYLE,
        ("morgan", _) => MORGAN_STYLE,
        ("dead_tooth_jack", _) => DEAD_TOOTH_JACK_STYLE,
        ("james_clark", _) => JAMES_CLARK_STYLE,
        ("erlan", _) => ERLAN_STYLE,
        ("surena", _) => SURENA_STYLE,
        ("goldtooth", _) => GOLDTOOTH_STYLE,
        ("ruklar", _) => RUKLAR_STYLE,
        ("narg", _) => NARG_STYLE,
        ("gruff", _) => GRUFF_STYLE,
        ("fedfennel", _) => FEDFENNEL_STYLE,
        ("princess", _) => PRINCESS_STYLE,
        ("mother_fang", _) => MOTHER_FANG_STYLE,
        _ => &[],
    }
}

/// The style of the move called `name`.
pub fn style_of<'a>(styles: &'a [Style], name: &str) -> Option<&'a Style> {
    styles.iter().find(|s| s.name == name)
}

/// The follow-up a move plans as it starts: one of its `then`, by their chances, or none.
pub fn plan_next(style: &Style, rng: &mut Rng) -> Option<&'static str> {
    let total: u32 = style.then.iter().map(|t| t.1).sum();
    let mut roll = rng.below(100);
    if total == 0 || roll >= total.min(100) {
        return None;
    }
    for &(name, percent) in style.then {
        if roll < percent {
            return Some(name);
        }
        roll -= percent;
    }
    None
}

/// The hold a move takes this swing, if its chance comes up: `(clip second, seconds)`.
pub fn plan_hold(style: &Style, rng: &mut Rng) -> Option<(f32, f32)> {
    let f = style.feint?;
    if rng.below(100) >= f.chance {
        return None;
    }
    let span = f.hold.1 - f.hold.0;
    let secs = f.hold.0 + span * (rng.below(1000) as f32 / 1000.0);
    Some((f.at, secs))
}

/// A small xorshift generator: repeatable runs (`WOWSOULS_SEED`) and tests.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }

    /// A number in `0..n` (`n` > 0).
    pub fn below(&mut self, n: u32) -> u32 {
        self.next_u32() % n.max(1)
    }
}

/// The weighted pick among `candidates` (row index, weight); `None` when every weight is 0.
fn weighted(candidates: &[(usize, u32)], rng: &mut Rng) -> Option<usize> {
    let total: u32 = candidates.iter().map(|c| c.1).sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.below(total);
    for &(i, w) in candidates {
        if roll < w {
            return Some(i);
        }
        roll -= w;
    }
    None
}

/// The band a creature with no move in mind holds itself to: from the nearest to the furthest
/// reach of the rows that can open a fight.
pub fn reach(table: &[Row]) -> (f32, f32) {
    let openers = table.iter().filter(|r| r.weight > 0);
    let near = openers.clone().map(|r| r.band.0).fold(f32::MAX, f32::min);
    let far = openers.map(|r| r.band.1).fold(0.0, f32::max);
    if far > 0.0 {
        (near, far)
    } else {
        (0.0, 2.0)
    }
}

/// The move it wishes for next, weighted over the openers; the move it just made weighs half.
pub fn wish(table: &[Row], last: Option<usize>, rng: &mut Rng) -> Option<usize> {
    let c: Vec<(usize, u32)> = table
        .iter()
        .enumerate()
        .map(|(i, r)| {
            (
                i,
                if Some(i) == last {
                    r.weight / 2
                } else {
                    r.weight
                },
            )
        })
        .collect();
    weighted(&c, rng)
}

/// The move to start with the player `dist` away: the wish if its band holds `dist`, else a
/// weighted pick among the openers whose band does; `None` when nothing fits yet.
pub fn choose(
    table: &[Row],
    dist: f32,
    wish: Option<usize>,
    last: Option<usize>,
    rng: &mut Rng,
) -> Option<usize> {
    let fits = |r: &Row| dist >= r.band.0 && dist <= r.band.1;
    if let Some(w) = wish.filter(|&w| table.get(w).is_some_and(fits)) {
        return Some(w);
    }
    let c: Vec<(usize, u32)> = table
        .iter()
        .enumerate()
        .filter(|(_, r)| fits(r))
        .map(|(i, r)| {
            (
                i,
                if Some(i) == last {
                    r.weight / 2
                } else {
                    r.weight
                },
            )
        })
        .collect();
    weighted(&c, rng)
}

/// The follow-up of `prev`, if its chance comes up and the player is still inside its band.
pub fn follow_up(table: &[Row], prev: usize, dist: f32, rng: &mut Rng) -> Option<usize> {
    let (name, percent) = table.get(prev)?.then?;
    let next = table.iter().position(|r| r.name == name)?;
    let band = table[next].band;
    (dist >= band.0 && dist <= band.1 && rng.below(100) < percent).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(name: &str) -> usize {
        KOBOLD.iter().position(|r| r.name == name).unwrap()
    }

    #[test]
    fn the_table_names_the_six_moves_and_its_follow_ups_exist() {
        assert_eq!(KOBOLD.len(), 6);
        for r in KOBOLD {
            if let Some((n, p)) = r.then {
                assert!(KOBOLD.iter().any(|x| x.name == n), "{n}");
                assert!(p <= 100);
            }
        }
        assert_eq!(
            KOBOLD[index("small_2")].weight,
            0,
            "small_2 only follows small_1"
        );
    }

    #[test]
    fn common_elwynn_kobolds_close_for_the_held_tool_and_stop_after_two_light_swings() {
        for (table, scale) in [(MINER, 1.15), (TUNNELER, 1.0)] {
            let mut rng = Rng(37);
            let edge = 1.15 * scale;
            assert_eq!(choose(table, edge - 0.01, None, None, &mut rng), Some(0));
            assert_eq!(choose(table, edge + 0.01, None, None, &mut rng), None);
            // A tool is shorter than the scaled bare-hand band: keep approaching the target.
            let old_hand_edge = 2.0 * scale / 0.85;
            assert_eq!(
                choose(table, old_hand_edge - 0.01, None, None, &mut rng),
                None
            );
            assert_eq!(table[1].name, "small_2");
            assert!(table[1].band.1 > edge);
            assert_eq!(follow_up(table, 1, 0.5, &mut rng), None);
        }
    }

    #[test]
    fn the_prowler_closes_to_bite_without_selecting_a_failed_lunge() {
        let mut rng = Rng(37);
        assert_eq!(choose(PROWLER, 1.49, None, None, &mut rng), Some(0));
        assert_eq!(choose(PROWLER, 1.51, None, None, &mut rng), None);
        assert_eq!(follow_up(PROWLER, 0, 1.0, &mut rng), None);
    }

    #[test]
    fn larger_wolves_choose_the_bite_and_lunge_at_their_display_scale() {
        for (table, scale) in [(MANGY_WOLF, 0.7), (GRAY_FOREST_WOLF, 0.85)] {
            let mut rng = Rng(37);
            let ratio = scale / 0.55;
            assert_eq!(choose(table, 1.2 * ratio, None, None, &mut rng), Some(0));
            assert_eq!(choose(table, 2.0 * ratio, None, None, &mut rng), Some(1));
            assert_eq!(choose(table, 2.5 * ratio, None, None, &mut rng), None);
        }
    }

    #[test]
    fn hoggers_tables_open_at_every_range_and_chain() {
        for table in [HOGGER, HOGGER_TWO] {
            // Something opens from touching range, from a few yards and from the far end of his reach.
            assert!(table.iter().any(|r| r.weight > 0 && r.band.0 <= 0.5));
            assert!(table.iter().any(|r| r.weight > 0 && r.band.1 >= 3.0));
            assert!(table
                .iter()
                .any(|r| r.weight > 0 && r.band.0 >= 2.0 && r.band.1 >= 4.0));
            for r in table {
                if let Some((next, pct)) = r.then {
                    assert!(table.iter().any(|x| x.name == next), "{} -> {next}", r.name);
                    assert!((1..=100).contains(&pct));
                }
            }
        }
        // The second phase is the first with more chains.
        let chains = |t: &[Row]| t.iter().filter(|r| r.then.is_some()).count();
        assert!(chains(HOGGER_TWO) > chains(HOGGER));
    }

    #[test]
    fn hoggers_styles_cover_his_tables_and_name_only_moves_he_has() {
        for (table, styles) in [(HOGGER, HOGGER_STYLE), (HOGGER_TWO, HOGGER_TWO_STYLE)] {
            for row in table {
                assert!(
                    style_of(styles, row.name).is_some(),
                    "{} has no style",
                    row.name
                );
            }
            for s in styles {
                assert!(table.iter().any(|r| r.name == s.name), "{}", s.name);
                let total: u32 = s.then.iter().map(|t| t.1).sum();
                assert!(total <= 100, "{}: {total} %", s.name);
                for &(next, percent) in s.then {
                    assert!(table.iter().any(|r| r.name == next), "{} -> {next}", s.name);
                    assert!((1..=100).contains(&percent));
                }
                assert!((0.0..=1.2).contains(&s.link), "{}", s.name);
                if let Some(f) = s.feint {
                    assert!(f.hold.0 > 0.0 && f.hold.0 <= f.hold.1 && f.hold.1 <= 1.0);
                    assert!(f.chance <= 100 && f.at > s.link);
                }
            }
            // The charge opens a fight, and it is the only opener of one.
            assert_eq!(styles.iter().filter(|s| s.opening).count(), 1);
            assert!(style_of(styles, "charge").unwrap().opening);
        }
        // Enraged, he chains more and holds more.
        let chain = |s: &[Style]| {
            s.iter()
                .map(|s| s.then.iter().map(|t| t.1).sum::<u32>())
                .sum::<u32>()
        };
        assert!(chain(HOGGER_TWO_STYLE) > chain(HOGGER_STYLE));
        assert!(styles("kobold", false).is_empty() && styles("garrick", true).is_empty());
    }

    #[test]
    fn the_knights_styles_chain_with_openings_and_name_its_rows() {
        for (rows, styles) in [
            (HOGGER_KNIGHT, HOGGER_KNIGHT_STYLE),
            (HOGGER_KNIGHT_TWO, HOGGER_KNIGHT_TWO_STYLE),
        ] {
            for s in styles {
                assert!(rows.iter().any(|r| r.name == s.name), "{}", s.name);
                for (next, _) in s.then {
                    assert!(rows.iter().any(|r| r.name == *next), "{} -> {next}", s.name);
                }
                // Every chain may end: an opening after it, sometimes.
                assert!(s.then.iter().map(|t| t.1).sum::<u32>() < 100, "{}", s.name);
                assert!((0.0..=1.2).contains(&s.link), "{}", s.name);
            }
            for r in rows {
                assert!(
                    style_of(styles, r.name).is_some(),
                    "{} has no style",
                    r.name
                );
                assert!(r.band.0 < r.band.1, "{}", r.name);
            }
            // The lunge opens a fight, and it is the only opener.
            assert_eq!(styles.iter().filter(|s| s.opening).count(), 1);
            assert!(style_of(styles, "lunge").unwrap().opening);
            assert!(rows.iter().all(|r| r.name != "frenzy"));
        }
        let chain = |s: &[Style]| {
            s.iter()
                .map(|s| s.then.iter().map(|t| t.1).sum::<u32>())
                .sum::<u32>()
        };
        assert!(chain(HOGGER_KNIGHT_TWO_STYLE) > chain(HOGGER_KNIGHT_STYLE));
        // Styles are the kit's: the Taurus kit keeps its own.
        assert_eq!(styles("hogger_knight", true), HOGGER_KNIGHT_TWO_STYLE);
        assert_eq!(styles("hogger", false), HOGGER_STYLE);
    }

    #[test]
    fn every_special_has_its_own_rhythm_over_its_own_rows() {
        for name in crate::species::BOSS_BAR_SPECIES {
            let def = crate::species::by_name(name).unwrap();
            assert_eq!(def.kit, *name, "{name} fights with its own kit");
            let styles = styles(def.kit, false);
            assert!(!styles.is_empty(), "{name} has no styles");
            for s in styles {
                assert!(
                    def.table.iter().any(|r| r.name == s.name),
                    "{name}: {}",
                    s.name
                );
                for (next, _) in s.then {
                    assert!(
                        def.table.iter().any(|r| r.name == *next),
                        "{name}: {} -> {next}",
                        s.name
                    );
                }
                // Every chain may end, leaving an opening.
                assert!(
                    s.then.iter().map(|t| t.1).sum::<u32>() < 100,
                    "{name}: {}",
                    s.name
                );
                if let Some(f) = s.feint {
                    assert!(f.hold.0 > 0.0 && f.hold.0 <= f.hold.1 && f.hold.1 <= 1.0);
                    assert!(f.at > s.link, "{name}: {}", s.name);
                }
            }
            // At most one opener, and the Elite budget of five moves.
            assert!(styles.iter().filter(|s| s.opening).count() <= 1, "{name}");
            assert!(def.table.len() <= 5, "{name}");
        }
    }

    #[test]
    fn a_planned_follow_up_comes_up_as_often_as_its_chances_say() {
        let mut rng = Rng(23);
        let style = style_of(HOGGER_STYLE, "overhead").unwrap();
        let mut counts = std::collections::HashMap::<&str, u32>::new();
        let rolls = 6000;
        for _ in 0..rolls {
            if let Some(n) = plan_next(style, &mut rng) {
                *counts.entry(n).or_default() += 1;
            }
        }
        // 30 / 20 / 10 percent of the rolls.
        let near = |name: &str, pct: u32| {
            let got = counts.get(name).copied().unwrap_or(0) as f32 / rolls as f32 * 100.0;
            assert!(
                (got - pct as f32).abs() < 3.0,
                "{name}: {got:.1} % for {pct}"
            );
        };
        near("backhand", 30);
        near("quick", 20);
        near("step_smash", 10);
        // A move that chains nowhere plans nothing.
        let none = Style {
            then: &[],
            ..*style
        };
        assert!((0..100).all(|_| plan_next(&none, &mut rng).is_none()));
    }

    #[test]
    fn a_feint_holds_the_swing_at_its_tell_for_a_time_in_its_range() {
        let mut rng = Rng(5);
        let style = style_of(HOGGER_STYLE, "heavy_swing").unwrap();
        let f = style.feint.unwrap();
        let (mut held, mut total) = (0, 4000);
        for _ in 0..total {
            if let Some((at, secs)) = plan_hold(style, &mut rng) {
                held += 1;
                assert_eq!(at, f.at);
                assert!(secs >= f.hold.0 && secs <= f.hold.1, "{secs}");
            }
        }
        total = total.max(1);
        let share = held as f32 / total as f32 * 100.0;
        assert!((share - f.chance as f32).abs() < 3.0, "{share:.1} %");
        // A move with no feint is never held.
        let quick = style_of(HOGGER_STYLE, "quick").unwrap();
        assert!((0..200).all(|_| plan_hold(quick, &mut rng).is_none()));
    }

    #[test]
    fn a_creature_with_no_move_in_mind_holds_the_reach_of_its_openers() {
        let (near, far) = reach(KOBOLD);
        assert_eq!(near, 0.0);
        assert!((far - 3.4).abs() < 1e-6, "{far}");
        // A table of no openers has a plain default, never nothing.
        assert_eq!(reach(&[]), (0.0, 2.0));
        // Rows that only follow up (weight 0) do not stretch it.
        let (_, far) = reach(VERMIN);
        assert!((far - 1.65).abs() < 1e-6, "{far}");
    }

    #[test]
    fn a_wish_is_an_opener_and_the_last_move_is_less_likely() {
        let mut rng = Rng(7);
        let mut counts = vec![0u32; KOBOLD.len()];
        for _ in 0..4000 {
            counts[wish(KOBOLD, None, &mut rng).unwrap()] += 1;
        }
        assert_eq!(counts[index("small_2")], 0);
        // Weights 4 : 2 : 1 : 3 : 1 out of 11.
        assert!(counts[index("small_1")] > counts[index("big")]);
        assert!(counts[index("run_strike")] > counts[index("pounce")]);
        let mut again = vec![0u32; KOBOLD.len()];
        let last = Some(index("small_1"));
        for _ in 0..4000 {
            again[wish(KOBOLD, last, &mut rng).unwrap()] += 1;
        }
        assert!(
            again[index("small_1")] < counts[index("small_1")],
            "{again:?} vs {counts:?}"
        );
    }

    #[test]
    fn it_starts_the_wish_inside_its_band_and_otherwise_picks_what_fits() {
        let mut rng = Rng(3);
        let run = Some(index("run_strike"));
        // Too far for any band, then inside the wish's.
        assert_eq!(choose(KOBOLD, 6.0, run, None, &mut rng), None);
        assert_eq!(choose(KOBOLD, 3.0, run, None, &mut rng), run);
        // The wish is the far move but the player is already close: a near move.
        for _ in 0..50 {
            let c = choose(KOBOLD, 1.0, run, None, &mut rng).unwrap();
            assert!(
                matches!(KOBOLD[c].name, "small_1" | "big" | "body_check"),
                "{}",
                KOBOLD[c].name
            );
        }
    }

    #[test]
    fn a_follow_up_needs_its_chance_and_its_band() {
        let mut rng = Rng(11);
        let one = index("small_1");
        let hits = (0..1000)
            .filter(|_| follow_up(KOBOLD, one, 1.5, &mut rng) == Some(index("small_2")))
            .count();
        assert!(
            (300..500).contains(&hits),
            "{hits} follow-ups in 1000 (40 %)"
        );
        assert_eq!(
            follow_up(KOBOLD, one, 3.0, &mut rng),
            None,
            "the player backed off"
        );
        assert_eq!(
            follow_up(KOBOLD, index("big"), 1.0, &mut rng),
            None,
            "big has none"
        );
    }

    #[test]
    fn the_same_seed_gives_the_same_fight() {
        let run = |seed| {
            let mut rng = Rng(seed);
            (0..20)
                .map(|_| wish(KOBOLD, None, &mut rng).unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(run(5), run(5));
        assert_ne!(run(5), run(6));
    }
}
