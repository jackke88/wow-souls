//! The XP table and the levels. `player_xp_for_level` of the database (400 XP takes a level-1 character to
//! level 2, 900 more to level 3 ...): the quests pay real XP here because five of them need level 2.
//! Kill XP is vMaNGOS's own formula (`MaNGOS::XP` in `src/game/Formulas.h`): [`kill_xp`] and [`MobXp`].

use crate::model::{Creature, World};

/// The creature level at or below which a character of `player` level earns nothing from a kill.
pub fn gray_level(player: u32) -> u32 {
    match player {
        0..=5 => 0,
        6..=39 => player - 5 - player / 10,
        _ => player - 1 - player / 5,
    }
}

/// How far below its own level a creature still pays: the divisor of the falling-off factor.
pub fn zero_difference(player: u32) -> u32 {
    match player {
        0..=7 => 5,
        8..=9 => 6,
        10..=11 => 7,
        12..=15 => 8,
        16..=19 => 9,
        20..=29 => 11,
        30..=39 => 12,
        40..=44 => 13,
        45..=49 => 14,
        50..=54 => 15,
        55..=59 => 16,
        _ => 17,
    }
}

/// The XP a character of level `player` earns from killing a creature of level `mob`: `5 x level + 45`,
/// +5 % per creature level above (at most four), falling off below until the gray level, doubled for an
/// elite, rounded half to even. The server's rate settings are not modelled (1.0).
pub fn kill_xp(player: u32, mob: u32, elite: bool) -> u32 {
    let base = u64::from(player) * 5 + 45;
    // The factor as a fraction, so a tie rounds the same way on every machine.
    let (num, den) = if mob >= player {
        (100 + 5 * u64::from((mob - player).min(4)), 100)
    } else if mob <= gray_level(player) {
        return 0;
    } else {
        let zd = zero_difference(player);
        (u64::from((zd + mob).saturating_sub(player)), u64::from(zd))
    };
    div_round_even(base * num * if elite { 2 } else { 1 }, den)
}

/// `n / d` rounded to the nearest whole number, a tie going to the even one (what `nearbyint` does).
fn div_round_even(n: u64, d: u64) -> u32 {
    let (q, r) = (n / d, n % d);
    let up = match (2 * r).cmp(&d) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Equal => q % 2 == 1,
        std::cmp::Ordering::Less => false,
    };
    (q + u64::from(up)) as u32
}

/// What a creature template says about the XP of killing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobXp {
    pub level_min: u32,
    pub level_max: u32,
    /// `rank` 1-3 (elite, rare elite, world boss) pays double; the valley has none.
    pub elite: bool,
    /// Critters (`type` 8) pay nothing.
    pub pays: bool,
}

impl MobXp {
    /// A template whose highest level is under its lowest is read as one level.
    pub fn new(level_min: u32, level_max: u32, elite: bool, pays: bool) -> MobXp {
        MobXp {
            level_min,
            level_max: level_max.max(level_min),
            elite,
            pays,
        }
    }

    pub fn of(c: &Creature) -> MobXp {
        MobXp::new(
            c.level_min,
            c.level_max,
            (1..=3).contains(&c.rank),
            c.kind != 8,
        )
    }

    /// The level of one spawn: spread over the template's range by its guid, so a seeded fight repeats. A
    /// creature without a spawn record (an old hand-placed one) is its template's lowest level.
    pub fn level_for(&self, guid: Option<u32>) -> u32 {
        match guid {
            Some(g) => self.level_min + g % (self.level_max - self.level_min + 1),
            None => self.level_min,
        }
    }

    /// The XP a killer of level `player` earns from this creature's spawn `guid`.
    pub fn payout(&self, player: u32, guid: Option<u32>) -> u32 {
        if !self.pays {
            return 0;
        }
        kill_xp(player, self.level_for(guid), self.elite)
    }
}

/// `for_level[l - 1]` is the XP that takes a character from level `l` to level `l + 1`. The table has one
/// row per level (60 in the database), so its length is the level cap and the last row is never used.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XpTable {
    pub for_level: Vec<u32>,
}

impl XpTable {
    pub fn from_world(world: &World) -> XpTable {
        XpTable {
            for_level: world.xp_for_level.clone(),
        }
    }

    /// The highest level (the table's length; 1 for an empty table).
    pub fn max_level(&self) -> u32 {
        self.for_level.len().max(1) as u32
    }

    /// The level that `total_xp` (all the XP ever earned) reaches, capped at [`XpTable::max_level`].
    pub fn level_for(&self, total_xp: u32) -> u32 {
        self.xp_into_level(total_xp).0
    }

    /// `(level, XP earned into that level, XP the level needs)`. At the cap the last two are 0.
    pub fn xp_into_level(&self, total_xp: u32) -> (u32, u32, u32) {
        let mut level = 1;
        let mut left = total_xp;
        while level < self.max_level() {
            let need = self.for_level[level as usize - 1];
            if left < need {
                return (level, left, need);
            }
            left -= need;
            level += 1;
        }
        (level, 0, 0)
    }

    /// The XP that takes a character from `level` to the next one (`None` at the cap or above it).
    pub fn xp_to_next(&self, level: u32) -> Option<u32> {
        if level == 0 || level >= self.max_level() {
            return None;
        }
        self.for_level.get(level as usize - 1).copied()
    }

    /// The total XP at which a character reaches `level` (0 for level 1; the cap's total above the cap).
    pub fn total_for(&self, level: u32) -> u32 {
        self.for_level
            .iter()
            .take(level.min(self.max_level()).saturating_sub(1) as usize)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest::testing::real_world;

    fn table(rows: &[u32]) -> XpTable {
        XpTable {
            for_level: rows.to_vec(),
        }
    }

    #[test]
    fn levels_come_from_the_running_total() {
        let t = table(&[400, 900, 1400]);
        for (xp, level) in [
            (0, 1),
            (399, 1),
            (400, 2),
            (1299, 2),
            (1300, 3),
            (999_999, 3),
        ] {
            assert_eq!(t.level_for(xp), level, "{xp} XP");
        }
        assert_eq!(t.max_level(), 3, "three rows: the cap is level 3");
    }

    #[test]
    fn xp_into_level_says_how_far_and_how_much_is_left() {
        let t = table(&[400, 900, 1400]);
        assert_eq!(t.xp_into_level(0), (1, 0, 400));
        assert_eq!(t.xp_into_level(450), (2, 50, 900));
        assert_eq!(t.xp_into_level(1299), (2, 899, 900));
        assert_eq!(t.xp_into_level(1300), (3, 0, 0), "the cap needs nothing");
        assert_eq!(t.xp_into_level(u32::MAX), (3, 0, 0));
    }

    #[test]
    fn xp_to_next_and_total_for_agree_with_the_table() {
        let t = table(&[400, 900, 1400]);
        assert_eq!(
            (t.xp_to_next(1), t.xp_to_next(2), t.xp_to_next(3)),
            (Some(400), Some(900), None)
        );
        assert_eq!((t.xp_to_next(0), t.xp_to_next(4)), (None, None));
        assert_eq!(
            [1, 2, 3, 4, 60].map(|l| t.total_for(l)),
            [0, 400, 1300, 1300, 1300]
        );
        for l in 1..=3 {
            assert_eq!(t.level_for(t.total_for(l)), l);
            if l > 1 {
                assert_eq!(t.level_for(t.total_for(l) - 1), l - 1);
            }
        }
    }

    #[test]
    fn a_table_of_one_row_or_none_never_levels() {
        let one = table(&[100]);
        assert_eq!(one.max_level(), 1);
        assert_eq!(one.level_for(5000), 1);
        assert_eq!(one.xp_into_level(5000), (1, 0, 0));
        assert_eq!(one.xp_to_next(1), None);
        let none = XpTable::default();
        assert_eq!((none.max_level(), none.level_for(5000)), (1, 1));
    }

    #[test]
    fn the_gray_level_follows_the_three_bands() {
        for (player, gray) in [
            (1, 0),
            (5, 0),
            (6, 1),
            (10, 4),
            (39, 31),
            (40, 31),
            (60, 47),
        ] {
            assert_eq!(gray_level(player), gray, "player level {player}");
        }
    }

    #[test]
    fn the_zero_difference_changes_at_every_band_edge() {
        let at = |l| zero_difference(l);
        assert_eq!(
            [1, 7, 8, 9, 10, 11, 12, 15, 16, 19, 20, 29, 30, 39].map(at),
            [5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 11, 11, 12, 12]
        );
        assert_eq!(
            [40, 44, 45, 49, 50, 54, 55, 59, 60, 70].map(at),
            [13, 13, 14, 14, 15, 15, 16, 16, 17, 17]
        );
    }

    #[test]
    fn a_kill_at_your_own_level_pays_five_per_level_plus_forty_five() {
        assert_eq!(kill_xp(1, 1, false), 50);
        assert_eq!(kill_xp(5, 5, false), 70);
        assert_eq!(kill_xp(10, 10, false), 95);
        // About eight such kills take a level-1 character to level 2 (400 XP).
        assert!(kill_xp(1, 1, false) * 8 >= 400);
    }

    #[test]
    fn a_higher_creature_pays_five_percent_a_level_up_to_four_levels() {
        assert_eq!(kill_xp(1, 3, false), 55, "two above: +10 %");
        assert_eq!(kill_xp(1, 5, false), 60, "four above: +20 %");
        assert_eq!(kill_xp(1, 9, false), 60, "the cap holds");
        assert_eq!(kill_xp(5, 7, false), 77);
        // A tie goes to the even number: 50 x 1.05 = 52.5 -> 52, 50 x 1.15 = 57.5 -> 58.
        assert_eq!(kill_xp(1, 2, false), 52);
        assert_eq!(kill_xp(1, 4, false), 58);
    }

    #[test]
    fn a_lower_creature_pays_less_until_it_turns_gray() {
        // Level 5 against level 1: zero difference 5, factor (5 + 1 - 5) / 5 = 0.2 of 70.
        assert_eq!(kill_xp(5, 1, false), 14);
        // Level 10 against 6: 95 x (7 + 6 - 10) / 7 = 40.7. Gray level of 10 is 4: level 4 pays nothing, 5 does.
        assert_eq!(kill_xp(10, 6, false), 41);
        assert_eq!(kill_xp(10, 5, false), 27);
        assert_eq!(kill_xp(10, 4, false), 0);
        // A level-6 character has gray level 1.
        assert_eq!(kill_xp(6, 1, false), 0);
        assert_eq!(kill_xp(6, 2, false), 15, "75 x (5 + 2 - 6) / 5");
    }

    #[test]
    fn an_elite_pays_double() {
        assert_eq!(kill_xp(3, 3, true), 120);
        assert_eq!(kill_xp(3, 3, true), 2 * kill_xp(3, 3, false));
        assert_eq!(kill_xp(10, 4, true), 0, "gray stays gray");
    }

    fn mob(level_min: u32, level_max: u32) -> MobXp {
        MobXp::new(level_min, level_max, false, true)
    }

    #[test]
    fn a_spawns_level_is_spread_over_its_templates_range_by_guid() {
        let thug = mob(3, 4);
        assert_eq!(thug.level_for(Some(80170)), 3);
        assert_eq!(thug.level_for(Some(80171)), 4);
        assert_eq!(thug.level_for(None), 3, "no record: the lowest level");
        assert_eq!(mob(1, 1).level_for(Some(12345)), 1);
        // A template whose max is under its min is read as one level.
        let odd = mob(4, 2);
        assert_eq!((odd.level_min, odd.level_max), (4, 4));
    }

    #[test]
    fn a_critter_pays_nothing_whatever_its_level() {
        let rabbit = MobXp::new(1, 1, false, false);
        assert_eq!(rabbit.payout(1, Some(7)), 0);
        assert_eq!(mob(1, 1).payout(1, Some(7)), 50);
        assert_eq!(mob(3, 4).payout(1, Some(80170)), 55, "level 3");
        assert_eq!(mob(3, 4).payout(1, Some(80171)), 58, "level 4: 57.5");
    }

    #[test]
    fn install_the_valleys_creatures_pay_as_the_database_says() {
        let Some(w) = real_world() else { return };
        let of = |entry: u32| MobXp::of(w.creatures.iter().find(|c| c.entry == entry).unwrap());
        // Kobold Vermin 1-2, Defias Thug 3-4, Garrick 5 (rank 0: not elite), Rabbit and Deer are critters.
        assert_eq!((of(6).level_min, of(6).level_max), (1, 2));
        assert_eq!((of(38).level_min, of(38).level_max), (3, 4));
        assert_eq!((of(103).level_min, of(103).level_max), (5, 5));
        assert!(!of(103).elite && of(103).pays);
        assert!(!of(721).pays && !of(883).pays, "critters give nothing");
        assert!(of(80).pays && of(257).pays && of(69).pays && of(299).pays);
        assert_eq!(
            of(103).payout(1, Some(80247)),
            60,
            "Garrick at level 1: the +20 % cap"
        );
    }

    #[test]
    fn install_the_real_table_levels_the_valley_quests_to_level_2_then_3() {
        let Some(w) = real_world() else { return };
        let t = XpTable::from_world(&w);
        assert_eq!(t.for_level.len(), 60);
        assert_eq!(&t.for_level[..5], &[400, 900, 1400, 2100, 2800]);
        assert_eq!((t.max_level(), t.level_for(u32::MAX)), (60, 60));
        assert_eq!(
            (t.level_for(0), t.level_for(399), t.level_for(400)),
            (1, 1, 2)
        );
        // The level-1 quests the valley can hand in give 1205 XP (783, 5261, 7, 33, 3100, 15, 21): level 2,
        // not 3. All of the valley's hand-ins (plus the five level-2 quests, 1260) give 2465: level 3.
        let by_id = |id: u32| w.quests.iter().find(|q| q.id == id).unwrap().reward_xp;
        let level_one: u32 = [783, 5261, 7, 33, 3100, 15, 21].map(by_id).iter().sum();
        let level_two: u32 = [18, 6, 3903, 3904, 3905].map(by_id).iter().sum();
        assert_eq!((level_one, level_two), (1205, 1260));
        assert_eq!(t.level_for(level_one), 2);
        assert_eq!(t.level_for(level_one + level_two), 3);
        assert_eq!(t.total_for(60), t.for_level[..59].iter().sum::<u32>());
    }
}
