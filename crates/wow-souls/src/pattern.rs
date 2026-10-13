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

/// A small xorshift generator: repeatable runs (`WOWSOULS_SEED`) and tests.
#[derive(Debug, Clone, Copy)]
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
