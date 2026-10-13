//! Who can take a quest, and what shows over an NPC: vMaNGOS's `Player::CanTakeQuest` for the data
//! the valley has (report section 1.2 in its order) and the 1.12 overhead marks (section 4).

use super::log::{QuestLog, Status};
use super::{Npc, Player, QuestBook};

/// Why a quest cannot be taken (the first check that fails, in vMaNGOS's order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No quest of that id in the book.
    Unknown,
    /// The quest is data only for this character (another class's, nobody gives it, it hangs on one
    /// that nobody gives): see [`QuestBook::playable`]. Checked last, after the rules of the game.
    NotPlayable,
    /// The quest is in the log (Incomplete or Complete).
    AlreadyHave,
    /// The quest is handed in; none of the valley's quests is repeatable.
    AlreadyDone,
    /// The player is below `MinLevel`.
    Level {
        need: u32,
    },
    /// The player is above `MaxLevel` (0 = no upper bound).
    TooHigh,
    Class,
    Race,
    /// The previous quest is not handed in (`SatisfyQuestPreviousQuest`).
    Prerequisite,
    /// The quest's chain successor is in the log, or a quest that leads to it is still current
    /// (`SatisfyQuestNextChain` / `SatisfyQuestPrevChain`).
    Chain,
    /// Another quest of the same positive `ExclusiveGroup` is in the log (any status).
    Exclusive,
    /// Reserved: the model carries no `RequiredSkill` (it is 0 for all 28 quests), so this is never given.
    Skill,
    /// The quest asks for reputation. The character has none with any faction (neutral, 0).
    Reputation,
    /// `RequiredCondition` is set (the condition table is not modelled; none of the 28 has one).
    Condition,
}

/// Can `p` take `quest` now?
pub fn can_take(book: &QuestBook, log: &QuestLog, p: &Player, quest: u32) -> bool {
    why_not(book, log, p, quest).is_none()
}

/// Whether the player could still take `quest` some day: it is not handed in, not another class's or
/// race's, not shut by a taken exclusive-group sibling or a chain, and the game offers it. A level or a
/// prerequisite that is not met yet does not count (the player will meet it): this is what lets a quest
/// item be picked up before the quest is accepted.
pub fn takeable_later(book: &QuestBook, log: &QuestLog, p: &Player, quest: u32) -> bool {
    matches!(
        why_not(book, log, p, quest),
        None | Some(Refusal::Level { .. } | Refusal::Prerequisite)
    )
}

/// `None`: the quest can be taken. Otherwise the first reason it cannot.
pub fn why_not(book: &QuestBook, log: &QuestLog, p: &Player, quest: u32) -> Option<Refusal> {
    let Some(q) = book.quest(quest) else {
        return Some(Refusal::Unknown);
    };
    // 1. MaxLevel (when non-zero) is not below the player's level.
    if q.max_level != 0 && p.level > q.max_level {
        return Some(Refusal::TooHigh);
    }
    // 2. SatisfyQuestStatus: a quest that has a log entry (a handed-in one keeps it) is never taken again.
    match log.status(quest) {
        Some(Status::Rewarded) => return Some(Refusal::AlreadyDone),
        Some(_) => return Some(Refusal::AlreadyHave),
        None => {}
    }
    // 3. SatisfyQuestExclusiveGroup: only a positive group excludes (section 1.5): another member in the
    // log, whatever its status, blocks. A negative group is "each from all", used by the prerequisites.
    if q.exclusive_group > 0
        && book
            .group(q.exclusive_group)
            .iter()
            .any(|&m| m != quest && log.entry(m).is_some())
    {
        return Some(Refusal::Exclusive);
    }
    // 4. Class and race masks: 0 passes, otherwise the player's bit must be in it.
    if q.required_classes != 0 && q.required_classes & p.class.mask() == 0 {
        return Some(Refusal::Class);
    }
    if q.required_races != 0 && q.required_races & p.race.mask() == 0 {
        return Some(Refusal::Race);
    }
    // 5. SatisfyQuestLevel.
    if p.level < q.min_level {
        return Some(Refusal::Level { need: q.min_level });
    }
    // 6. SatisfyQuestSkill: not modelled (RequiredSkill is 0 for all 28).
    // 7. SatisfyQuestCondition: the condition table is not modelled.
    if q.required_condition != 0 {
        return Some(Refusal::Condition);
    }
    // 8. SatisfyQuestReputation, with the character's standing 0 with every faction.
    if (q.rep_min.0 != 0 && 0 < q.rep_min.1) || (q.rep_max.0 != 0 && 0 >= q.rep_max.1) {
        return Some(Refusal::Reputation);
    }
    // 9. SatisfyQuestPreviousQuest.
    if !previous_quest_satisfied(book, log, quest) {
        return Some(Refusal::Prerequisite);
    }
    // 10. SatisfyQuestTimed: no quest here is timed (LimitTime 0).
    // 11. SatisfyQuestNextChain: the quest after this one in its chain is already in the log (any status).
    if q.next_in_chain != 0 && log.entry(q.next_in_chain).is_some() {
        return Some(Refusal::Chain);
    }
    //     SatisfyQuestPrevChain: a quest that leads straight to this one is still current.
    if book
        .prev_chain(quest)
        .iter()
        .any(|&c| matches!(log.status(c), Some(Status::Incomplete | Status::Complete)))
    {
        return Some(Refusal::Chain);
    }
    // 12. Breadcrumbs: none of the 28 uses them. 13. Method (disabled quests): all 28 are Method 2.
    // The game's own decision: the quest is offered to this character at all.
    if !book.playable(quest) {
        return Some(Refusal::NotPlayable);
    }
    None
}

/// `SatisfyQuestPreviousQuest` (section 1.4). No prerequisites: true. Otherwise the first prerequisite
/// that qualifies decides: a positive id must be handed in, a negative id (`NextQuestId` below zero)
/// must be current (in the log, not handed in); in a negative ("each from all") exclusive group every
/// other member of the group must be in the same state too, or the answer is false at once. A
/// prerequisite that is not a quest of the book, or not in the log, is skipped; none qualifying: false.
fn previous_quest_satisfied(book: &QuestBook, log: &QuestLog, quest: u32) -> bool {
    let prevs = book.prev_quests(quest);
    if prevs.is_empty() {
        return true;
    }
    for &p in prevs {
        let id = p.unsigned_abs();
        let Some(prev) = book.quest(id) else {
            continue;
        };
        let Some(entry) = log.entry(id) else {
            continue;
        };
        let state = |status: Status| {
            if p > 0 {
                status == Status::Rewarded
            } else {
                status != Status::Rewarded
            }
        };
        if !state(entry.status) {
            continue;
        }
        if prev.exclusive_group >= 0 {
            return true;
        }
        return book
            .group(prev.exclusive_group)
            .iter()
            .filter(|&&m| m != id)
            .all(|&m| log.entry(m).is_some_and(|e| state(e.status)));
    }
    false
}

/// Over a NPC or object: the best of what its quests say, for this player (the 1.12 marks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    None,
    /// A yellow "!": the NPC has a quest the player can take.
    Available,
    /// The quest can be taken but is trivial for the player (the player's level is more than
    /// `LOW_LEVEL_HIDE_DIFF` above the quest's): shown grey, or not at all.
    AvailableLow,
    /// A grey "?": the NPC takes in a quest the player has not finished.
    InProgress,
    /// A yellow "?": the NPC takes in a quest the player has finished.
    Ready,
}

impl Mark {
    /// The order the marks win in: Ready > Available > AvailableLow > InProgress > None.
    fn rank(self) -> u8 {
        match self {
            Mark::None => 0,
            Mark::InProgress => 1,
            Mark::AvailableLow => 2,
            Mark::Available => 3,
            Mark::Ready => 4,
        }
    }
}

/// vMaNGOS `Quests.LowLevelHideDiff` (default 4): a quest more than this many levels below the player is
/// no longer shown as available. (`Quests.HighLevelHideDiff`, 7, hides a quest that many levels above
/// the player; `can_take` has already refused every quest the player is below `MinLevel` for.)
pub const LOW_LEVEL_HIDE_DIFF: u32 = 4;

/// What shows over `npc` for `p` (section 4.1). A quest this NPC takes in that the log holds Complete gives
/// `Ready`, Incomplete gives `InProgress`; a quest it gives that `p` can take gives `Available`, or
/// `AvailableLow` when `p.level` is more than [`LOW_LEVEL_HIDE_DIFF`] above the quest's level. A parked
/// quest (its ender is outside the valley) shows nothing at any valley NPC: its ender is not here.
pub fn marks(book: &QuestBook, log: &QuestLog, p: &Player, npc: Npc) -> Mark {
    let (entry, object) = match npc {
        Npc::Creature(e) => (e, false),
        Npc::Object(e) => (e, true),
    };
    let mut best = Mark::None;
    let mut see = |m: Mark| {
        if m.rank() > best.rank() {
            best = m;
        }
    };
    for &quest in book.by_ender(entry, object) {
        match log.status(quest) {
            Some(Status::Complete) => see(Mark::Ready),
            Some(Status::Incomplete) => see(Mark::InProgress),
            _ => {}
        }
    }
    for &quest in book.by_giver(entry, object) {
        if !can_take(book, log, p, quest) {
            continue;
        }
        let level = book.quest(quest).map_or(0, |q| q.level);
        // A quest level of -1 or less means "the player's level": never low.
        if level > 0 && p.level > level as u32 + LOW_LEVEL_HIDE_DIFF {
            see(Mark::AvailableLow);
        } else {
            see(Mark::Available);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Quest, Relation, Spawn, World};
    use crate::quest::log::Entry;
    use crate::quest::testing::{quest, real_world};
    use crate::quest::{Class, Race};
    use std::collections::BTreeSet;

    fn log_of(rewarded: &[u32], current: &[(u32, Status)]) -> QuestLog {
        let mut log = QuestLog::default();
        for &q in rewarded {
            log.insert(q, Entry::new(Status::Rewarded));
        }
        for &(q, s) in current {
            log.insert(q, Entry::new(s));
        }
        log
    }

    /// The quests of the book `p` can take, ascending.
    fn takeable(book: &QuestBook, log: &QuestLog, p: &Player) -> Vec<u32> {
        book.ids().filter(|&q| can_take(book, log, p, q)).collect()
    }

    // ---- synthetic worlds: one rule at a time -------------------------------------------------------

    /// A world whose quests are all given and taken by NPCs that stand in the valley (giver 100 + id,
    /// ender 200 + id), edited by `edit` on the quest of that id.
    fn world(ids: &[u32], edit: &dyn Fn(&mut Quest)) -> World {
        let mut w = World::default();
        for &id in ids {
            let mut q = quest(id);
            edit(&mut q);
            w.quests.push(q);
            for (npc, list) in [(100 + id, 0), (200 + id, 1)] {
                let r = Relation {
                    npc,
                    quest: id,
                    object: false,
                };
                if list == 0 {
                    w.starts.push(r);
                } else {
                    w.ends.push(r);
                }
                w.spawns.push(Spawn {
                    guid: npc,
                    entry: npc,
                    alt: [0; 4],
                    pos: [0.0; 3],
                    orientation: 0.0,
                    respawn_min: 0,
                    respawn_max: 0,
                    wander: 0.0,
                    movement: 0,
                    area: 9,
                    event: false,
                });
            }
        }
        w
    }

    fn one(id: u32, edit: impl Fn(&mut Quest)) -> (QuestBook, u32) {
        (QuestBook::new(&world(&[id], &edit)), id)
    }

    fn warrior(level: u32) -> Player {
        Player::warrior("Tester", level)
    }

    #[test]
    fn an_unknown_quest_and_a_fresh_one() {
        let (book, id) = one(1, |_| {});
        let log = QuestLog::default();
        assert_eq!(
            why_not(&book, &log, &warrior(1), 999),
            Some(Refusal::Unknown)
        );
        assert_eq!(why_not(&book, &log, &warrior(1), id), None);
    }

    #[test]
    fn a_quest_in_the_log_or_handed_in_is_never_taken_again() {
        let (book, id) = one(1, |_| {});
        let p = warrior(1);
        for s in [Status::Incomplete, Status::Complete] {
            let log = log_of(&[], &[(id, s)]);
            assert_eq!(why_not(&book, &log, &p, id), Some(Refusal::AlreadyHave));
        }
        let log = log_of(&[id], &[]);
        assert_eq!(why_not(&book, &log, &p, id), Some(Refusal::AlreadyDone));
    }

    #[test]
    fn the_level_gates_use_min_level_and_max_level_where_zero_means_no_bound() {
        let (book, id) = one(1, |q| {
            q.min_level = 3;
            q.max_level = 5;
        });
        let log = QuestLog::default();
        assert_eq!(
            why_not(&book, &log, &warrior(2), id),
            Some(Refusal::Level { need: 3 })
        );
        for level in [3, 4, 5] {
            assert_eq!(why_not(&book, &log, &warrior(level), id), None, "{level}");
        }
        assert_eq!(
            why_not(&book, &log, &warrior(6), id),
            Some(Refusal::TooHigh)
        );
        // MaxLevel 0: no upper bound, even at level 60.
        let (open, id) = one(1, |q| q.min_level = 3);
        assert_eq!(why_not(&open, &log, &warrior(60), id), None);
        // MaxLevel is checked before the log: a high-level player who has the quest is told "too high".
        let have = log_of(&[], &[(1, Status::Incomplete)]);
        assert_eq!(
            why_not(&book, &have, &warrior(6), 1),
            Some(Refusal::TooHigh)
        );
    }

    #[test]
    fn class_and_race_masks_let_zero_and_listed_bits_through() {
        let masked = |classes: u32, races: u32| {
            one(1, move |q| {
                q.required_classes = classes;
                q.required_races = races;
            })
        };
        let log = QuestLog::default();
        let (book, id) = masked(Class::Priest.mask(), 0);
        assert_eq!(why_not(&book, &log, &warrior(1), id), Some(Refusal::Class));
        let (book, id) = masked(Class::Priest.mask() | Class::Warrior.mask(), 0);
        assert_eq!(why_not(&book, &log, &warrior(1), id), None);
        let (book, id) = masked(0, Race::Orc.mask() | Race::Dwarf.mask());
        assert_eq!(why_not(&book, &log, &warrior(1), id), Some(Refusal::Race));
        let mut dwarf = warrior(1);
        dwarf.race = Race::Dwarf;
        // The mask lets a Dwarf in; what stops the Dwarf is that the book is the Human Warrior's
        // (`playable` is for that character), which is checked after the rules of the game.
        assert_eq!(why_not(&book, &log, &dwarf, id), Some(Refusal::NotPlayable));
        // The masks of the data: Warrior 1, Paladin 2, Hunter 4, Rogue 8, Priest 16, Shaman 64, Mage 128,
        // Warlock 256, Druid 1024; Human 1, Orc 2, Dwarf 4, Night Elf 8, Undead 16, Tauren 32, Gnome 64, Troll 128.
        let classes = [
            Class::Warrior,
            Class::Paladin,
            Class::Hunter,
            Class::Rogue,
            Class::Priest,
            Class::Shaman,
            Class::Mage,
            Class::Warlock,
            Class::Druid,
        ]
        .map(Class::mask);
        assert_eq!(classes, [1, 2, 4, 8, 16, 64, 128, 256, 1024]);
        let races = [
            Race::Human,
            Race::Orc,
            Race::Dwarf,
            Race::NightElf,
            Race::Undead,
            Race::Tauren,
            Race::Gnome,
            Race::Troll,
        ]
        .map(Race::mask);
        assert_eq!(races, [1, 2, 4, 8, 16, 32, 64, 128]);
    }

    #[test]
    fn reputation_and_condition_gates_refuse_a_character_with_no_standing() {
        let log = QuestLog::default();
        let p = warrior(1);
        let cases: [(&dyn Fn(&mut Quest), Option<Refusal>); 6] = [
            (&|q| q.required_condition = 5, Some(Refusal::Condition)),
            // Needs at least Friendly (3000) with faction 72: standing 0 is below it.
            (&|q| q.rep_min = (72, 3000), Some(Refusal::Reputation)),
            // A minimum of 0 or less is met by standing 0.
            (&|q| q.rep_min = (72, 0), None),
            // Needs to be below 3000 with a faction: standing 0 is.
            (&|q| q.rep_max = (72, 3000), None),
            // Needs to be below 0 (hostile side): standing 0 is not.
            (&|q| q.rep_max = (72, 0), Some(Refusal::Reputation)),
            // The reputation a quest pays is not a requirement.
            (&|q| q.reward_rep = vec![(72, 100)], None),
        ];
        for (n, (edit, want)) in cases.iter().enumerate() {
            let (book, id) = one(1, edit);
            assert_eq!(why_not(&book, &log, &p, id), *want, "case {n}");
        }
    }

    #[test]
    fn a_positive_exclusive_group_allows_one_member_and_a_negative_one_never_blocks() {
        let group = |g: i32| {
            QuestBook::new(&world(&[1, 2, 3], &|q| {
                if q.id != 3 {
                    q.exclusive_group = g
                }
            }))
        };
        let p = warrior(1);
        let book = group(5);
        assert_eq!(why_not(&book, &QuestLog::default(), &p, 1), None);
        for s in [Status::Incomplete, Status::Complete, Status::Rewarded] {
            let log = log_of(&[], &[(1, s)]);
            assert_eq!(
                why_not(&book, &log, &p, 2),
                Some(Refusal::Exclusive),
                "{s:?}"
            );
        }
        // Quest 3 is in no group.
        assert_eq!(
            why_not(&book, &log_of(&[], &[(1, Status::Complete)]), &p, 3),
            None
        );
        // A negative group does not exclude: 18 and 33 can both be in the log.
        let book = group(-5);
        let log = log_of(&[], &[(1, Status::Incomplete)]);
        assert_eq!(why_not(&book, &log, &p, 2), None);
    }

    #[test]
    fn a_prerequisite_must_be_handed_in_not_just_complete() {
        let book = QuestBook::new(&world(&[1, 2], &|q| {
            if q.id == 2 {
                q.prev = 1
            }
        }));
        let p = warrior(1);
        assert_eq!(
            why_not(&book, &QuestLog::default(), &p, 2),
            Some(Refusal::Prerequisite)
        );
        for s in [Status::Incomplete, Status::Complete] {
            assert_eq!(
                why_not(&book, &log_of(&[], &[(1, s)]), &p, 2),
                Some(Refusal::Prerequisite),
                "{s:?}"
            );
        }
        assert_eq!(why_not(&book, &log_of(&[1], &[]), &p, 2), None);
    }

    #[test]
    fn a_negative_next_quest_id_means_available_while_the_quest_is_current() {
        // Quest 1 names quest 2 as its NextQuestId, negative: 2 is takeable while 1 is in the log, not before
        // and not once 1 is handed in.
        let book = QuestBook::new(&world(&[1, 2], &|q| {
            if q.id == 1 {
                q.next = -2
            }
        }));
        let p = warrior(1);
        assert_eq!(book.prev_quests(2), &[-1]);
        assert_eq!(
            why_not(&book, &QuestLog::default(), &p, 2),
            Some(Refusal::Prerequisite)
        );
        for s in [Status::Incomplete, Status::Complete] {
            assert_eq!(
                why_not(&book, &log_of(&[], &[(1, s)]), &p, 2),
                None,
                "{s:?}"
            );
        }
        assert_eq!(
            why_not(&book, &log_of(&[1], &[]), &p, 2),
            Some(Refusal::Prerequisite)
        );
    }

    #[test]
    fn a_prerequisite_in_a_negative_group_needs_every_member_in_the_same_state() {
        // 1 and 2 are an each-from-all pair; 3 follows 1 (prev) and 4 follows both through NextQuestId.
        let book = QuestBook::new(&world(&[1, 2, 3, 4], &|q| match q.id {
            1 => {
                q.exclusive_group = -9;
                q.next = 4
            }
            2 => {
                q.exclusive_group = -9;
                q.next = 4
            }
            3 => q.prev = 1,
            _ => {}
        }));
        let p = warrior(1);
        assert_eq!(book.prev_quests(4), &[1, 2]);
        let cases: [(&[u32], bool, bool); 4] = [
            // (rewarded, 3 takeable, 4 takeable)
            (&[], false, false),
            (&[1], false, false),
            (&[2], false, false),
            (&[1, 2], true, true),
        ];
        for (rewarded, three, four) in cases {
            let log = log_of(rewarded, &[]);
            assert_eq!(can_take(&book, &log, &p, 3), three, "3 with {rewarded:?}");
            assert_eq!(can_take(&book, &log, &p, 4), four, "4 with {rewarded:?}");
        }
        // The other member only in the log, not handed in: still not enough.
        let log = log_of(&[1], &[(2, Status::Complete)]);
        assert!(!can_take(&book, &log, &p, 4));
    }

    #[test]
    fn chain_rules_block_a_quest_whose_successor_is_in_the_log_or_whose_predecessor_is_current() {
        // 1 --chain--> 2 --chain--> 3; 2 and 3 also need the previous one handed in.
        let book = QuestBook::new(&world(&[1, 2, 3], &|q| match q.id {
            1 => q.next_in_chain = 2,
            2 => {
                q.next_in_chain = 3;
                q.prev = 1
            }
            3 => q.prev = 2,
            _ => {}
        }));
        let p = warrior(1);
        // The successor is in the log (taken early): the predecessor cannot be taken any more.
        for s in [Status::Incomplete, Status::Complete, Status::Rewarded] {
            let log = log_of(&[], &[(2, s)]);
            assert_eq!(why_not(&book, &log, &p, 1), Some(Refusal::Chain), "{s:?}");
        }
        // A predecessor still current (the prerequisite check passes with a rewarded 1; use a quest whose
        // only link is the chain).
        let book = QuestBook::new(&world(&[1, 2], &|q| {
            if q.id == 1 {
                q.next_in_chain = 2
            }
        }));
        for s in [Status::Incomplete, Status::Complete] {
            let log = log_of(&[], &[(1, s)]);
            assert_eq!(why_not(&book, &log, &p, 2), Some(Refusal::Chain), "{s:?}");
        }
        // Once handed in, the chain no longer blocks.
        assert_eq!(why_not(&book, &log_of(&[1], &[]), &p, 2), None);
    }

    #[test]
    fn the_prerequisite_check_comes_before_the_chain_check() {
        let book = QuestBook::new(&world(&[1, 2], &|q| match q.id {
            1 => q.next_in_chain = 2,
            _ => q.prev = 1,
        }));
        let log = log_of(&[], &[(1, Status::Complete)]);
        assert_eq!(
            why_not(&book, &log, &warrior(1), 2),
            Some(Refusal::Prerequisite)
        );
    }

    #[test]
    fn a_quest_that_is_data_only_for_this_character_is_not_playable() {
        // Nobody gives quest 1.
        let mut w = world(&[1, 2], &|_| {});
        w.starts.retain(|r| r.quest != 1);
        let book = QuestBook::new(&w);
        assert_eq!(
            why_not(&book, &QuestLog::default(), &warrior(1), 1),
            Some(Refusal::NotPlayable)
        );
        assert!(can_take(&book, &QuestLog::default(), &warrior(1), 2));
        // The class check is first: a Priest-only quest is a Class refusal for a Warrior, not NotPlayable.
        let mut w = world(&[3], &|q| q.required_classes = Class::Priest.mask());
        w.starts.clear();
        let book = QuestBook::new(&w);
        assert_eq!(
            why_not(&book, &QuestLog::default(), &warrior(1), 3),
            Some(Refusal::Class)
        );
    }

    // ---- marks ------------------------------------------------------------------------------------

    #[test]
    fn marks_pick_the_best_of_what_the_npc_gives_and_takes() {
        // NPC 50 gives 1 and takes 2; NPC 51 takes 1. (Nobody gives 2: it only matters as a log entry.)
        let mut w = world(&[1, 2], &|_| {});
        w.starts = vec![Relation {
            npc: 50,
            quest: 1,
            object: false,
        }];
        w.ends = vec![
            Relation {
                npc: 50,
                quest: 2,
                object: false,
            },
            Relation {
                npc: 51,
                quest: 1,
                object: false,
            },
        ];
        for e in [50, 51] {
            w.spawns.push(Spawn {
                guid: e,
                entry: e,
                alt: [0; 4],
                pos: [0.0; 3],
                orientation: 0.0,
                respawn_min: 0,
                respawn_max: 0,
                wander: 0.0,
                movement: 0,
                area: 9,
                event: false,
            });
        }
        let book = QuestBook::new(&w);
        let p = warrior(1);
        let at = |log: &QuestLog, e: u32| marks(&book, log, &p, Npc::Creature(e));

        assert_eq!(at(&QuestLog::default(), 50), Mark::Available);
        assert_eq!(at(&QuestLog::default(), 51), Mark::None);
        // Quest 1 taken: 51 (its ender) shows a grey ?, 50 no longer offers it.
        let log = log_of(&[], &[(1, Status::Incomplete)]);
        assert_eq!((at(&log, 50), at(&log, 51)), (Mark::None, Mark::InProgress));
        let log = log_of(&[], &[(1, Status::Complete)]);
        assert_eq!((at(&log, 50), at(&log, 51)), (Mark::None, Mark::Ready));
        // Quest 2 in progress at 50 while 1 is still available: the "!" wins over a grey "?".
        let log = log_of(&[], &[(2, Status::Incomplete)]);
        assert_eq!(at(&log, 50), Mark::Available);
        // ... and a finished quest beats an available one.
        let log = log_of(&[], &[(2, Status::Complete)]);
        assert_eq!(at(&log, 50), Mark::Ready);
        // Handed in: nothing.
        assert_eq!(at(&log_of(&[1, 2], &[]), 50), Mark::None);
        // An object is not a creature.
        assert_eq!(
            marks(&book, &QuestLog::default(), &p, Npc::Object(50)),
            Mark::None
        );
    }

    #[test]
    fn a_quest_far_below_the_players_level_is_available_low_by_the_hide_diff() {
        let (book, _) = one(1, |q| q.level = 3);
        let log = QuestLog::default();
        let at = |level: u32| marks(&book, &log, &warrior(level), Npc::Creature(101));
        // QuestLevel 3: yellow up to level 3 + 4 = 7, low from level 8 (vMaNGOS: `level <= quest level + diff`).
        assert_eq!(at(7), Mark::Available);
        assert_eq!(at(8), Mark::AvailableLow);
        assert_eq!(at(60), Mark::AvailableLow);
        // A quest level below 1 means "the player's level" and is never low.
        let (book, _) = one(1, |q| q.level = -1);
        assert_eq!(
            marks(&book, &log, &warrior(60), Npc::Creature(101)),
            Mark::Available
        );
        // Below MinLevel the quest is not offered, so nothing shows (the report's "unavailable" grey ! is not drawn).
        let (book, _) = one(1, |q| q.min_level = 5);
        assert_eq!(
            marks(&book, &log, &warrior(1), Npc::Creature(101)),
            Mark::None
        );
    }

    #[test]
    fn a_quest_that_is_not_ours_yet_is_still_takeable_later() {
        let Some(w) = real_world() else { return };
        let book = QuestBook::new(&w);
        let fresh = log_of(&[], &[]);
        // 33 wants its prerequisite, 18 and 6 want a level: all of them come later.
        for q in [33, 18, 6] {
            assert!(takeable_later(&book, &fresh, &warrior(1), q), "{q}");
        }
        // Another class's: never.
        assert!(!takeable_later(&book, &fresh, &warrior(1), 3101));
        // Handed in: never again.
        assert!(!takeable_later(&book, &log_of(&[18], &[]), &warrior(2), 18));
        // In the log: the accepted rule's business, not an early pickup.
        assert!(!takeable_later(
            &book,
            &log_of(&[], &[(18, Status::Incomplete)]),
            &warrior(2),
            18
        ));
    }

    // ---- the real valley ---------------------------------------------------------------------------

    #[test]
    fn install_report_section_1_12_test_vectors() {
        let Some(w) = real_world() else { return };
        let book = QuestBook::new(&w);
        let level = |l| warrior(l);
        let ids = |v: &[u32]| v.to_vec();

        // Rewarded {783}: level 1 -> {7, 5261} and 2158 (always); 3100 not yet (its prerequisite is 7).
        let log = log_of(&[783], &[]);
        assert_eq!(takeable(&book, &log, &level(1)), ids(&[7, 2158, 5261]));
        // Level 2 adds 18.
        assert_eq!(takeable(&book, &log, &level(2)), ids(&[7, 18, 2158, 5261]));
        // Rewarded {783, 7}: adds {15, 3100} (7 itself is done).
        let log = log_of(&[783, 7], &[]);
        assert_eq!(
            takeable(&book, &log, &level(1)),
            ids(&[15, 2158, 3100, 5261])
        );
        // 15 current (not rewarded): 21 is not takeable (needs 15 handed in; the chain forbids it too).
        for s in [Status::Incomplete, Status::Complete] {
            let log = log_of(&[783, 7], &[(15, s)]);
            assert!(!can_take(&book, &log, &level(1), 21), "{s:?}");
            assert_eq!(
                why_not(&book, &log, &level(1), 21),
                Some(Refusal::Prerequisite)
            );
        }
        assert!(can_take(&book, &log_of(&[783, 7, 15], &[]), &level(1), 21));

        // 18 handed in alone: neither 3903 nor 6 (both need 33 as well: the each-from-all group -18).
        let log = log_of(&[783, 5261, 18], &[]);
        for q in [3903, 6] {
            assert_eq!(
                why_not(&book, &log, &level(2), q),
                Some(Refusal::Prerequisite),
                "{q}"
            );
        }
        // 33 handed in alone: not either.
        let log = log_of(&[783, 5261, 33], &[]);
        for q in [3903, 6] {
            assert!(!can_take(&book, &log, &level(2), q), "{q}");
        }
        // Both handed in: both takeable (at level 2, not at level 1).
        let log = log_of(&[783, 5261, 18, 33], &[]);
        for q in [3903, 6] {
            assert!(can_take(&book, &log, &level(2), q), "{q}");
            assert_eq!(
                why_not(&book, &log, &level(1), q),
                Some(Refusal::Level { need: 2 })
            );
        }
        // 18 or 33 in the log (not handed in): 3903 is not takeable.
        for (rewarded, current) in [
            (&[783, 5261, 18][..], (33, Status::Incomplete)),
            (&[783, 5261, 33][..], (18, Status::Complete)),
        ] {
            let log = log_of(rewarded, &[current]);
            assert!(!can_take(&book, &log, &level(2), 3903), "{current:?}");
        }
        // 18 and 33 can both be in the log at once (the negative group does not exclude).
        let log = log_of(&[783, 5261], &[(18, Status::Incomplete)]);
        assert!(can_take(&book, &log, &level(2), 33));

        // A quest already in the log or handed in is not takeable.
        let log = log_of(&[783], &[(7, Status::Incomplete)]);
        assert_eq!(
            why_not(&book, &log, &level(1), 7),
            Some(Refusal::AlreadyHave)
        );
        assert_eq!(
            why_not(&book, &log, &level(1), 783),
            Some(Refusal::AlreadyDone)
        );

        // A Warrior never sees the other classes' quests, whatever the level and log.
        let everything: Vec<u32> = book.ids().collect();
        let log = log_of(&[783, 7, 21, 15], &[]);
        for q in [3101, 3102, 3103, 3104, 3105, 5623, 1598] {
            assert_eq!(
                why_not(&book, &log, &level(60), q),
                Some(Refusal::Class),
                "{q}"
            );
        }
        assert_eq!(everything.len(), 28);
        // The orphans, and the Stormwind chain (its prerequisite 343 is not even in the book).
        for q in [5805, 7961, 7962] {
            assert_eq!(
                why_not(&book, &log, &level(60), q),
                Some(Refusal::NotPlayable)
            );
        }
        for q in [344, 345, 346, 347] {
            assert!(!can_take(&book, &log, &level(60), q), "{q}");
        }
    }

    /// Take everything takeable whose giver stands in the valley, then hand in everything whose ender does
    /// (objectives met at once), until nothing changes. The report's waves (section 1.12).
    fn waves(book: &QuestBook, level: u32) -> Vec<(Vec<u32>, Vec<u32>)> {
        let p = warrior(level);
        let mut log = QuestLog::default();
        let mut out = Vec::new();
        loop {
            let taken: Vec<u32> = book
                .ids()
                .filter(|&q| book.giver_spawned(q) && can_take(book, &log, &p, q))
                .collect();
            for &q in &taken {
                log.insert(q, Entry::new(Status::Complete));
            }
            let done: Vec<u32> = log
                .active()
                .into_iter()
                .filter(|&q| book.ender_spawned(q))
                .collect();
            for &q in &done {
                log.insert(q, Entry::new(Status::Rewarded));
            }
            if taken.is_empty() && done.is_empty() {
                return out;
            }
            out.push((taken, done));
        }
    }

    #[test]
    fn install_the_waves_of_the_report_for_level_1() {
        let Some(w) = real_world() else { return };
        let book = QuestBook::new(&w);
        assert_eq!(
            waves(&book, 1),
            vec![
                (vec![783, 2158], vec![783]),
                (vec![7, 5261], vec![7, 5261]),
                (vec![15, 33, 3100], vec![15, 33, 3100]),
                (vec![21], vec![21]),
                (vec![54], vec![]),
            ]
        );
    }

    #[test]
    fn install_the_waves_of_the_report_for_level_2() {
        let Some(w) = real_world() else { return };
        let book = QuestBook::new(&w);
        let got = waves(&book, 2);
        assert_eq!(
            got,
            vec![
                (vec![783, 2158], vec![783]),
                (vec![7, 18, 5261], vec![7, 18, 5261]),
                (vec![15, 33, 3100], vec![15, 33, 3100]),
                (vec![6, 21, 3903], vec![6, 21, 3903]),
                (vec![54, 3904], vec![3904]),
                (vec![3905], vec![3905]),
            ]
        );
        // Everything reachable was taken: the 14 playable quests, 54 and 2158 left in the log.
        let taken: BTreeSet<u32> = got.iter().flat_map(|(t, _)| t.iter().copied()).collect();
        let playable: BTreeSet<u32> = book.ids().filter(|&q| book.playable(q)).collect();
        assert_eq!(taken, playable);
    }

    #[test]
    fn install_marks_follow_the_log_through_the_first_quests() {
        let Some(w) = real_world() else { return };
        let book = QuestBook::new(&w);
        let p = warrior(1);
        let at = |log: &QuestLog, npc: u32| marks(&book, log, &p, Npc::Creature(npc));
        let (mcbride, willem, eagan, llane, dughan) = (197, 823, 196, 911, 240);

        // A fresh character: Willem has 783 (and 5261 / 18 / 6 / 3903 are not takeable yet).
        let log = QuestLog::default();
        assert_eq!(at(&log, willem), Mark::Available);
        assert_eq!(
            at(&log, mcbride),
            Mark::None,
            "McBride has nothing until 783 is handed in"
        );
        assert_eq!(
            at(&log, 6774),
            Mark::Available,
            "Falkhaan Isenstrider gives 2158"
        );

        // 783 complete: McBride takes it in.
        let log = log_of(&[], &[(783, Status::Complete), (2158, Status::Complete)]);
        assert_eq!(at(&log, mcbride), Mark::Ready);
        // 783 handed in: McBride has 7, Willem has 5261, and 2158 is parked: no ? over any valley NPC.
        let log = log_of(&[783], &[(2158, Status::Complete)]);
        assert_eq!(at(&log, mcbride), Mark::Available);
        assert_eq!(at(&log, willem), Mark::Available);
        assert_eq!(
            at(&log, 295),
            Mark::Ready,
            "Innkeeper Farley would show it, but he is not here"
        );
        assert_eq!(at(&log, 6774), Mark::None);
        // 3100 complete, 5261 in progress with Eagan:
        let log = log_of(
            &[783, 7],
            &[(3100, Status::Complete), (5261, Status::Complete)],
        );
        assert_eq!(at(&log, llane), Mark::Ready);
        assert_eq!(at(&log, eagan), Mark::Ready);
        // 54 complete (parked): McBride shows nothing for it, Dughan would.
        let log = log_of(&[783, 7, 15, 21, 3100], &[(54, Status::Complete)]);
        assert_eq!(at(&log, mcbride), Mark::None);
        assert_eq!(at(&log, dughan), Mark::Ready);
    }
}
