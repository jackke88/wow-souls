//! `QuestLog`: the state of a player's quests and its transitions (accept, kill, object, items, turn in,
//! abandon). Every transition returns [`Event`]s; the caller does the effects (hands items out, takes
//! them, pays money and XP, shows toasts) and tells the log what the inventory holds.

use std::collections::{BTreeMap, BTreeSet};

use super::rules::{self, Refusal};
use super::{Npc, Player, QuestBook};
use crate::model::{ItemCount, Quest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// In the log, objectives not all met.
    Incomplete,
    /// In the log, objectives met, not yet handed in.
    Complete,
    /// Handed in. A quest here is never repeatable, so it stays rewarded for good.
    Rewarded,
}

/// One quest of the log: its status and its counters. `kills[i]` counts `req_kills[i]` of the quest
/// (a creature or object), `items[i]` mirrors the inventory's count of `req_items[i]` (capped at the need).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub status: Status,
    pub kills: [u32; 4],
    pub items: [u32; 4],
}

impl Entry {
    pub fn new(status: Status) -> Entry {
        Entry {
            status,
            kills: [0; 4],
            items: [0; 4],
        }
    }
}

/// What an objective counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// A creature of this entry, or an object (a negative id).
    Kill {
        entry: i32,
    },
    Item {
        item: u32,
    },
}

/// What a turn-in hands over and takes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rewards {
    /// Given to the player: the fixed items and the chosen one.
    pub items: Vec<ItemCount>,
    /// The quest's required items, which the caller takes from the inventory.
    pub consume: Vec<ItemCount>,
    /// Copper (`RewOrReqMoney`; negative when the player pays).
    pub money: i32,
    pub xp: u32,
    /// The quest the same NPC offers at once (`NextQuestInChain`), when the player can take it.
    pub next_in_chain: Option<u32>,
}

/// What a transition changed, for the toasts and the tracker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The quest is in the log; the caller gives `source_item` (the quest's start item) to the player.
    Accepted {
        quest: u32,
        source_item: Option<ItemCount>,
    },
    /// An objective moved: `now` of `need`. `slot` is its place in the quest's kill or item list.
    Progress {
        quest: u32,
        slot: usize,
        now: u32,
        need: u32,
        what: Need,
    },
    /// Every objective is met.
    ReadyToTurnIn {
        quest: u32,
    },
    TurnedIn {
        quest: u32,
        rewards: Rewards,
    },
    Abandoned {
        quest: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnInError {
    /// The quest is not in the log, or is already handed in.
    NotInLog,
    /// Its objectives are not met (or the inventory no longer holds the required items).
    NotComplete,
    /// No NPC of the valley takes this quest in: it is parked (quests 54 and 2158 end in Goldshire).
    OutsideTheValley,
    /// This NPC is not the quest's ender.
    WrongNpc,
    /// The quest has reward choices and none was chosen.
    ChoiceRequired,
    /// The chosen reward is not one of the choices.
    BadChoice,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuestLog {
    entries: BTreeMap<u32, Entry>,
}

/// Are all objectives of `q` met by `e`?
fn objectives_met(q: &Quest, e: &Entry) -> bool {
    // vMaNGOS requires separate exploration/event credit; empty counters cannot grant it.
    // Imported scripted objectives await world hooks; spell objectives are not kill credit.
    if q.special_flags & 2 != 0 || q.req_spell_cast.iter().any(|id| *id != 0) {
        return false;
    }
    q.req_kills
        .iter()
        .enumerate()
        .all(|(i, &(_, need))| e.kills.get(i).copied().unwrap_or(0) >= need)
        && q.req_items
            .iter()
            .enumerate()
            .all(|(i, &(_, need))| e.items.get(i).copied().unwrap_or(0) >= need)
}

/// Move an entry between Incomplete and Complete to match its counters.
fn settle(quest: u32, q: &Quest, e: &mut Entry, events: &mut Vec<Event>) {
    match e.status {
        Status::Incomplete if objectives_met(q, e) => {
            e.status = Status::Complete;
            events.push(Event::ReadyToTurnIn { quest });
        }
        Status::Complete if !objectives_met(q, e) => e.status = Status::Incomplete,
        _ => {}
    }
}

impl QuestLog {
    pub fn status(&self, quest: u32) -> Option<&Status> {
        self.entries.get(&quest).map(|e| &e.status)
    }

    pub fn entry(&self, quest: u32) -> Option<&Entry> {
        self.entries.get(&quest)
    }

    /// The quests in the log that are not handed in (Incomplete or Complete), in id order.
    pub fn active(&self) -> Vec<u32> {
        self.entries
            .iter()
            .filter(|(_, e)| e.status != Status::Rewarded)
            .map(|(&q, _)| q)
            .collect()
    }

    pub fn is_rewarded(&self, quest: u32) -> bool {
        self.status(quest) == Some(&Status::Rewarded)
    }

    /// Every entry, in quest id order (the save writes them).
    pub fn entries(&self) -> impl Iterator<Item = (u32, &Entry)> {
        self.entries.iter().map(|(&q, e)| (q, e))
    }

    /// Put an entry in the log as it is (the save loads them; tests set a state with it). The transitions
    /// below are how a game changes the log.
    pub fn insert(&mut self, quest: u32, entry: Entry) {
        self.entries.insert(quest, entry);
    }

    /// Take a quest. The quest's start item (`SrcItemId`) is reported in the event for the caller to hand
    /// out and counts toward the quest's required items at once (quests 54, 346, 3100-3105 and 3905 ask
    /// for the item they start with, so they are complete the moment they are accepted); a quest with no
    /// objectives is complete at once too (report section 6, finding 5).
    pub fn accept(
        &mut self,
        book: &QuestBook,
        p: &Player,
        quest: u32,
        inventory: &dyn Fn(u32) -> u32,
    ) -> Result<Vec<Event>, Refusal> {
        if let Some(why) = rules::why_not(book, self, p, quest) {
            return Err(why);
        }
        let q = book.quest(quest).ok_or(Refusal::Unknown)?;
        let src = (q.src_item != 0).then_some((q.src_item, q.src_item_count.max(1)));
        let mut entry = Entry::new(Status::Incomplete);
        for (i, &(item, need)) in q.req_items.iter().enumerate().take(4) {
            let pending = src.filter(|s| s.0 == item).map_or(0, |s| s.1);
            entry.items[i] = (inventory(item) + pending).min(need);
        }
        let mut events = vec![Event::Accepted {
            quest,
            source_item: src,
        }];
        settle(quest, q, &mut entry, &mut events);
        self.entries.insert(quest, entry);
        Ok(events)
    }

    /// A creature of `entry` died by the player's hand: credit by exact entry (report section 5.1).
    pub fn kill(&mut self, book: &QuestBook, entry: u32) -> Vec<Event> {
        self.credit(book, entry as i32)
    }

    /// An object of `entry` was used (a kill objective with a negative id).
    pub fn use_object(&mut self, book: &QuestBook, entry: u32) -> Vec<Event> {
        self.credit(book, -(entry as i32))
    }

    fn credit(&mut self, book: &QuestBook, what: i32) -> Vec<Event> {
        let mut events = Vec::new();
        for (&quest, e) in self.entries.iter_mut() {
            if e.status != Status::Incomplete {
                continue;
            }
            let Some(q) = book.quest(quest) else { continue };
            for (i, &(id, need)) in q.req_kills.iter().enumerate().take(4) {
                if id == what && e.kills[i] < need {
                    e.kills[i] += 1;
                    events.push(Event::Progress {
                        quest,
                        slot: i,
                        now: e.kills[i],
                        need,
                        what: Need::Kill { entry: id },
                    });
                }
            }
            settle(quest, q, e, &mut events);
        }
        events
    }

    /// The inventory's count of `item` is now `count`: every quest in the log that needs it follows (a
    /// Complete quest whose item was lost goes back to Incomplete).
    pub fn item_count(&mut self, book: &QuestBook, item: u32, count: u32) -> Vec<Event> {
        let mut events = Vec::new();
        for (&quest, e) in self.entries.iter_mut() {
            if e.status == Status::Rewarded {
                continue;
            }
            let Some(q) = book.quest(quest) else { continue };
            for (i, &(it, need)) in q.req_items.iter().enumerate().take(4) {
                let now = count.min(need);
                if it == item && now != e.items[i] {
                    e.items[i] = now;
                    events.push(Event::Progress {
                        quest,
                        slot: i,
                        now,
                        need,
                        what: Need::Item { item },
                    });
                }
            }
            settle(quest, q, e, &mut events);
        }
        events
    }

    /// Should a kill drop this quest item for this player: a quest in the log still needs it (report
    /// section 5.2). `held` is the inventory's count.
    pub fn wants_item(&self, book: &QuestBook, item: u32, held: u32) -> bool {
        self.entries.iter().any(|(&quest, e)| {
            e.status == Status::Incomplete
                && book.quest(quest).is_some_and(|q| {
                    q.req_items
                        .iter()
                        .any(|&(it, need)| it == item && held < need)
                })
        })
    }

    /// Every item some quest in the log still needs: what `loot::QuestLootEligibility` holds.
    pub fn wanted_items(&self, book: &QuestBook, inventory: &dyn Fn(u32) -> u32) -> BTreeSet<u32> {
        let mut out = BTreeSet::new();
        for (&quest, e) in &self.entries {
            if e.status != Status::Incomplete {
                continue;
            }
            let Some(q) = book.quest(quest) else { continue };
            for &(item, need) in &q.req_items {
                if inventory(item) < need {
                    out.insert(item);
                }
            }
        }
        out
    }

    /// What a corpse may drop for quests: everything [`wanted_items`](Self::wanted_items) holds, and the
    /// required items of every quest the player has not taken yet but could (see
    /// [`rules::takeable_later`]), up to the count already carried.
    pub fn droppable_items(
        &self,
        book: &QuestBook,
        p: &Player,
        inventory: &dyn Fn(u32) -> u32,
    ) -> BTreeSet<u32> {
        let mut out = self.wanted_items(book, inventory);
        for id in book.ids() {
            if self.entry(id).is_some() || !rules::takeable_later(book, self, p, id) {
                continue;
            }
            let Some(q) = book.quest(id) else { continue };
            for &(item, need) in &q.req_items {
                if inventory(item) < need {
                    out.insert(item);
                }
            }
        }
        out
    }

    /// The most of `item` a quest the player has not taken yet (but could) will ask for: the shop does
    /// not sell that many.
    pub fn pre_need(&self, book: &QuestBook, p: &Player, item: u32) -> u32 {
        book.ids()
            .filter(|&id| self.entry(id).is_none() && rules::takeable_later(book, self, p, id))
            .filter_map(|id| book.quest(id))
            .flat_map(|q| q.req_items.iter())
            .filter(|&&(it, _)| it == item)
            .map(|&(_, n)| n)
            .max()
            .unwrap_or(0)
    }

    /// Hand a quest in at `npc` (its ender), choosing reward `choice` (an index into the quest's reward
    /// choices; required when it has any). The log does not touch items: the result names what to give
    /// (`items`), what to take (`consume`), the copper, the XP and the quest the same NPC offers next.
    pub fn turn_in(
        &mut self,
        book: &QuestBook,
        p: &Player,
        quest: u32,
        npc: Npc,
        choice: Option<usize>,
        inventory: &dyn Fn(u32) -> u32,
    ) -> Result<Vec<Event>, TurnInError> {
        let status = self.status(quest).copied();
        if !matches!(status, Some(Status::Incomplete | Status::Complete)) {
            return Err(TurnInError::NotInLog);
        }
        let q = book.quest(quest).ok_or(TurnInError::NotInLog)?;
        if !book.ender_spawned(quest) {
            return Err(TurnInError::OutsideTheValley);
        }
        if !book.enders(quest).contains(&npc) {
            return Err(TurnInError::WrongNpc);
        }
        let entry = self
            .entries
            .get(&quest)
            .copied()
            .ok_or(TurnInError::NotInLog)?;
        // The log mirrors the inventory, but the inventory has the last word.
        let held_all = q
            .req_items
            .iter()
            .all(|&(item, need)| inventory(item) >= need);
        if entry.status != Status::Complete || !held_all {
            return Err(TurnInError::NotComplete);
        }
        let mut items = q.reward_items.clone();
        if q.reward_choice.is_empty() {
            if choice.is_some_and(|c| c != 0) {
                return Err(TurnInError::BadChoice);
            }
        } else {
            let c = choice.ok_or(TurnInError::ChoiceRequired)?;
            items.push(*q.reward_choice.get(c).ok_or(TurnInError::BadChoice)?);
        }
        self.entries.insert(quest, Entry::new(Status::Rewarded));
        let next = (q.next_in_chain != 0 && rules::can_take(book, self, p, q.next_in_chain))
            .then_some(q.next_in_chain);
        Ok(vec![Event::TurnedIn {
            quest,
            rewards: Rewards {
                items,
                consume: q.req_items.clone(),
                money: q.reward_money,
                xp: q.reward_xp,
                next_in_chain: next,
            },
        }])
    }

    /// Drop a quest from the log (the caller takes its start item back); a handed-in quest stays.
    pub fn abandon(&mut self, quest: u32) -> Vec<Event> {
        match self.entries.get(&quest) {
            Some(e) if e.status != Status::Rewarded => {
                self.entries.remove(&quest);
                vec![Event::Abandoned { quest }]
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest::testing::real_world;

    #[test]
    fn imported_event_and_spell_objectives_cannot_complete_from_empty_counters() {
        let mut q = crate::quest::testing::quest(999);
        let entry = Entry::new(Status::Incomplete);
        assert!(objectives_met(&q, &entry));
        q.special_flags = 2;
        assert!(!objectives_met(&q, &entry));
        q.special_flags = 0;
        q.req_spell_cast = vec![123];
        assert!(!objectives_met(&q, &entry));
    }

    fn nothing(_: u32) -> u32 {
        0
    }

    fn book() -> Option<QuestBook> {
        real_world().map(|w| QuestBook::new(&w))
    }

    fn warrior(level: u32) -> Player {
        Player::warrior("Adventurer", level)
    }

    #[test]
    fn a_quest_with_no_objectives_is_complete_when_accepted() {
        let Some(book) = book() else { return };
        let mut log = QuestLog::default();
        let ev = log.accept(&book, &warrior(1), 783, &nothing).unwrap();
        assert_eq!(
            ev,
            vec![
                Event::Accepted {
                    quest: 783,
                    source_item: None
                },
                Event::ReadyToTurnIn { quest: 783 }
            ]
        );
        assert_eq!(log.status(783), Some(&Status::Complete));
        assert_eq!(log.active(), vec![783]);
    }

    #[test]
    fn kill_credit_is_by_exact_entry_capped_and_completes() {
        let Some(book) = book() else { return };
        let mut log = QuestLog::default();
        let p = warrior(1);
        log.accept(&book, &p, 783, &nothing).unwrap();
        log.turn_in(&book, &p, 783, Npc::Creature(197), None, &nothing)
            .unwrap();
        let ev = log.accept(&book, &p, 7, &nothing).unwrap();
        assert_eq!(ev.len(), 1, "quest 7 has objectives: not complete at once");
        // A kobold laborer is not a vermin.
        assert!(log.kill(&book, 80).is_empty());
        let mut events = Vec::new();
        for _ in 0..10 {
            events.extend(log.kill(&book, 6));
        }
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Progress { .. }))
                .count(),
            10
        );
        assert_eq!(events.last(), Some(&Event::ReadyToTurnIn { quest: 7 }));
        assert_eq!(log.status(7), Some(&Status::Complete));
        assert!(log.kill(&book, 6).is_empty(), "no credit past the need");
        assert_eq!(log.entry(7).unwrap().kills[0], 10);
    }

    #[test]
    fn droppable_items_cover_untaken_quests_and_stop_when_carried_or_handed_in() {
        let Some(book) = book() else { return };
        let p = warrior(1);
        let log = QuestLog::default();
        // Before any quest: the wolf meat of 33, the bandanas of 18 and Garrick's head of 6 are worth
        // picking up.
        let fresh = log.droppable_items(&book, &p, &nothing);
        for item in [750, 752, 182] {
            assert!(fresh.contains(&item), "{item}");
        }
        // Carrying what a quest needs: no more of it.
        let carried = log.droppable_items(&book, &p, &|i| if i == 182 { 1 } else { 0 });
        assert!(!carried.contains(&182));
        // Quest 6 handed in: its head is wanted no more.
        let mut done = QuestLog::default();
        done.insert(6, Entry::new(Status::Rewarded));
        assert!(!done.droppable_items(&book, &p, &nothing).contains(&182));
        // The shop does not sell what a quest the player has not taken yet will ask for.
        assert_eq!(log.pre_need(&book, &p, 750), 8);
        assert_eq!(log.pre_need(&book, &p, 182), 1);
        assert_eq!(done.pre_need(&book, &p, 182), 0);
    }

    #[test]
    fn an_accepted_quest_is_the_accepted_rules_business_not_an_early_pickup() {
        let Some(book) = book() else { return };
        let p = warrior(1);
        let mut log = QuestLog::default();
        log.accept(&book, &p, 783, &nothing).unwrap();
        log.turn_in(&book, &p, 783, Npc::Creature(197), None, &nothing)
            .unwrap();
        log.accept(&book, &p, 5261, &nothing).unwrap();
        log.turn_in(&book, &p, 5261, Npc::Creature(196), None, &nothing)
            .unwrap();
        log.accept(&book, &p, 33, &nothing).unwrap();
        assert!(
            log.droppable_items(&book, &p, &nothing).contains(&750),
            "while incomplete"
        );
        assert_eq!(log.pre_need(&book, &p, 750), 0, "33 is in the log");
    }

    #[test]
    fn items_follow_the_inventory_up_and_down() {
        let Some(book) = book() else { return };
        let mut log = QuestLog::default();
        let p = warrior(1);
        // 33 Wolves Across the Border needs 8 x item 750 and follows 5261 (chain from Eagan).
        log.accept(&book, &p, 783, &nothing).unwrap();
        log.turn_in(&book, &p, 783, Npc::Creature(197), None, &nothing)
            .unwrap();
        log.accept(&book, &p, 5261, &nothing).unwrap();
        let ev = log
            .turn_in(&book, &p, 5261, Npc::Creature(196), None, &nothing)
            .unwrap();
        let Event::TurnedIn { rewards, .. } = &ev[0] else {
            panic!("turned in")
        };
        assert_eq!(
            rewards.next_in_chain,
            Some(33),
            "Eagan offers the wolves at once"
        );
        log.accept(&book, &p, 33, &nothing).unwrap();
        assert!(log.wants_item(&book, 750, 0));
        assert!(log.wants_item(&book, 750, 7));
        assert!(!log.wants_item(&book, 750, 8), "enough held");
        assert_eq!(
            log.wanted_items(&book, &nothing)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![750]
        );
        let ev = log.item_count(&book, 750, 5);
        assert_eq!(
            ev,
            vec![Event::Progress {
                quest: 33,
                slot: 0,
                now: 5,
                need: 8,
                what: Need::Item { item: 750 }
            }]
        );
        let ev = log.item_count(&book, 750, 12);
        assert_eq!(ev.last(), Some(&Event::ReadyToTurnIn { quest: 33 }));
        assert_eq!(log.entry(33).unwrap().items[0], 8, "capped at the need");
        // The meat is lost (sold, destroyed): the quest is open again.
        log.item_count(&book, 750, 7);
        assert_eq!(log.status(33), Some(&Status::Incomplete));
        assert!(!log.wanted_items(&book, &|_| 8).contains(&750));
    }

    #[test]
    fn turning_in_needs_the_right_npc_the_choice_and_the_items() {
        let Some(book) = book() else { return };
        let mut log = QuestLog::default();
        let p = warrior(1);
        log.accept(&book, &p, 783, &nothing).unwrap();
        log.turn_in(&book, &p, 783, Npc::Creature(197), None, &nothing)
            .unwrap();
        log.accept(&book, &p, 5261, &nothing).unwrap();
        log.turn_in(&book, &p, 5261, Npc::Creature(196), None, &nothing)
            .unwrap();
        log.accept(&book, &p, 33, &nothing).unwrap();
        let eight = |i: u32| if i == 750 { 8 } else { 0 };
        log.item_count(&book, 750, 8);
        // Not Deputy Willem: Eagan takes it in.
        assert_eq!(
            log.turn_in(&book, &p, 33, Npc::Creature(823), Some(0), &eight),
            Err(TurnInError::WrongNpc)
        );
        // It has reward choices (shoes or bracers).
        assert_eq!(
            log.turn_in(&book, &p, 33, Npc::Creature(196), None, &eight),
            Err(TurnInError::ChoiceRequired)
        );
        assert_eq!(
            log.turn_in(&book, &p, 33, Npc::Creature(196), Some(9), &eight),
            Err(TurnInError::BadChoice)
        );
        // The meat is gone from the inventory: not complete after all.
        assert_eq!(
            log.turn_in(&book, &p, 33, Npc::Creature(196), Some(0), &nothing),
            Err(TurnInError::NotComplete)
        );
        let ev = log
            .turn_in(&book, &p, 33, Npc::Creature(196), Some(1), &eight)
            .unwrap();
        let Event::TurnedIn { quest, rewards } = &ev[0] else {
            panic!("turned in")
        };
        assert_eq!(*quest, 33);
        assert_eq!(rewards.xp, 170);
        assert_eq!(rewards.consume, vec![(750, 8)]);
        assert_eq!(rewards.items.len(), 1, "the chosen one of the two");
        assert!(log.is_rewarded(33));
        assert_eq!(
            log.turn_in(&book, &p, 33, Npc::Creature(196), Some(1), &eight),
            Err(TurnInError::NotInLog),
            "handed in once"
        );
    }

    #[test]
    fn a_quest_that_starts_with_its_own_item_is_complete_at_once_and_parked_in_the_valley() {
        let Some(book) = book() else { return };
        let mut log = QuestLog::default();
        let p = warrior(1);
        // 54 Report to Goldshire: McBride's Documents (745) start it and it asks for them.
        // McBride's line up to 54: take each, kill what it asks, hand it in to McBride (197).
        for quest in [783, 7, 15, 21] {
            log.accept(&book, &p, quest, &nothing).unwrap();
            let q = book.quest(quest).unwrap();
            for &(id, need) in &q.req_kills {
                for _ in 0..need {
                    log.kill(&book, id as u32);
                }
            }
            let choice = (!q.reward_choice.is_empty()).then_some(0);
            log.turn_in(&book, &p, quest, Npc::Creature(197), choice, &nothing)
                .unwrap();
        }
        let ev = log.accept(&book, &p, 54, &nothing).unwrap();
        assert_eq!(
            ev,
            vec![
                Event::Accepted {
                    quest: 54,
                    source_item: Some((745, 1))
                },
                Event::ReadyToTurnIn { quest: 54 }
            ]
        );
        // Marshal Dughan stands in Goldshire: nothing in the valley takes it in.
        assert_eq!(
            log.turn_in(&book, &p, 54, Npc::Creature(240), None, &|_| 1),
            Err(TurnInError::OutsideTheValley)
        );
        assert_eq!(log.active(), vec![54]);
        assert_eq!(log.abandon(54), vec![Event::Abandoned { quest: 54 }]);
        assert!(log.abandon(54).is_empty());
        assert_eq!(
            log.abandon(783),
            Vec::<Event>::new(),
            "handed in quests stay"
        );
    }

    #[test]
    fn a_level_gate_refuses_and_the_refusal_is_the_rules() {
        let Some(book) = book() else { return };
        let mut log = QuestLog::default();
        assert_eq!(
            log.accept(&book, &warrior(1), 18, &nothing),
            Err(Refusal::Level { need: 2 })
        );
        assert_eq!(
            log.accept(&book, &warrior(1), 99_999, &nothing),
            Err(Refusal::Unknown)
        );
        assert!(log.active().is_empty());
    }
}
