//! The whole valley played through by the pure engine: a Human Warrior takes, completes and hands in every
//! quest it can, in the report's order (section 6.2), with the inventory, the XP and the level the way a
//! game would keep them. It is the check that the rules, the log, the XP table and the real data agree.
//! Skipped (with a note) when there is no manifest.

use std::collections::BTreeMap;

use super::log::{Event, Rewards, TurnInError};
use super::rules::Refusal;
use super::xp::XpTable;
use super::{testing::real_world, Npc, Player, QuestBook, QuestLog};

/// A game's side of the engine: the inventory, the XP, the copper, the level.
struct Game {
    book: QuestBook,
    table: XpTable,
    log: QuestLog,
    p: Player,
    inv: BTreeMap<u32, u32>,
    xp: u32,
    money: i64,
}

impl Game {
    fn new(w: &crate::model::World) -> Game {
        Game {
            book: QuestBook::new(w),
            table: XpTable::from_world(w),
            log: QuestLog::default(),
            p: Player::warrior("Adventurer", 1),
            inv: BTreeMap::new(),
            xp: 0,
            money: 0,
        }
    }

    fn held(&self, item: u32) -> u32 {
        self.inv.get(&item).copied().unwrap_or(0)
    }

    fn set_items(&mut self, item: u32, count: u32) {
        if count == 0 {
            self.inv.remove(&item);
        } else {
            self.inv.insert(item, count);
        }
        self.log.item_count(&self.book, item, count);
    }

    fn give(&mut self, item: u32, n: u32) {
        let now = self.held(item) + n;
        self.set_items(item, now);
    }

    fn accept(&mut self, quest: u32) -> Result<(), Refusal> {
        let inv = self.inv.clone();
        let events = self.log.accept(&self.book, &self.p, quest, &|i| {
            inv.get(&i).copied().unwrap_or(0)
        })?;
        for e in events {
            if let Event::Accepted {
                source_item: Some((item, n)),
                ..
            } = e
            {
                self.give(item, n);
            }
        }
        Ok(())
    }

    fn kill(&mut self, entry: u32, n: u32) {
        for _ in 0..n {
            self.log.kill(&self.book, entry);
        }
    }

    /// Kills drop the quest item at its chance only while a quest wants it: here, always, so the test
    /// loots until the log stops wanting it.
    fn loot_until_enough(&mut self, item: u32) -> u32 {
        let mut dropped = 0;
        while self.log.wants_item(&self.book, item, self.held(item)) {
            self.give(item, 1);
            dropped += 1;
            assert!(dropped < 100, "the log never stopped wanting item {item}");
        }
        dropped
    }

    fn turn_in(
        &mut self,
        quest: u32,
        npc: u32,
        choice: Option<usize>,
    ) -> Result<Rewards, TurnInError> {
        let inv = self.inv.clone();
        let events = self.log.turn_in(
            &self.book,
            &self.p,
            quest,
            Npc::Creature(npc),
            choice,
            &|i| inv.get(&i).copied().unwrap_or(0),
        )?;
        let Event::TurnedIn { rewards, .. } = events.into_iter().next().expect("one event") else {
            panic!("a turn-in gives TurnedIn")
        };
        for &(item, n) in &rewards.consume {
            let left = self.held(item).saturating_sub(n);
            self.set_items(item, left);
        }
        for &(item, n) in &rewards.items {
            self.give(item, n);
        }
        self.money += i64::from(rewards.money);
        self.xp += rewards.xp;
        self.p.level = self.table.level_for(self.xp);
        Ok(rewards)
    }
}

#[test]
fn a_human_warrior_plays_the_whole_valley() {
    let Some(w) = real_world() else { return };
    let xp_of = |id: u32| w.quests.iter().find(|q| q.id == id).unwrap().reward_xp;
    let money_of = |id: u32| i64::from(w.quests.iter().find(|q| q.id == id).unwrap().reward_money);
    let mut g = Game::new(&w);
    let mcbride = 197;
    let willem = 823;
    let eagan = 196;
    let milly = 9296;

    // 1. A Threat Within (nothing to do): handed in at McBride, who offers the kobolds at once.
    g.accept(783).unwrap();
    let r = g.turn_in(783, mcbride, None).unwrap();
    assert_eq!((r.xp, r.next_in_chain), (40, Some(7)));
    // The level-2 quests are shut at level 1.
    assert_eq!(g.accept(18), Err(Refusal::Level { need: 2 }));
    assert_eq!(g.accept(6), Err(Refusal::Level { need: 2 }));

    // 2. Kobold Camp Cleanup: ten Kobold Vermin.
    g.accept(7).unwrap();
    g.kill(6, 10);
    let r = g.turn_in(7, mcbride, None).unwrap();
    assert_eq!(r.xp, 170);
    assert_eq!(g.turn_in(7, mcbride, None), Err(TurnInError::NotInLog));

    // 3. The Warrior's letter completes at once (its source item is what it asks for) and goes to Llane.
    g.accept(3100).unwrap();
    assert_eq!(g.log.active().len(), 1);
    g.turn_in(3100, 911, None).unwrap();
    assert_eq!(
        g.accept(3101),
        Err(Refusal::Class),
        "the Paladin's letter is not for a Warrior"
    );
    assert_eq!(g.accept(1598), Err(Refusal::Class));

    // 4. Eagan Peltskinner, then the wolves (offered at once): 8 Tough Wolf Meat.
    g.accept(5261).unwrap();
    let r = g.turn_in(5261, eagan, None).unwrap();
    assert_eq!(r.next_in_chain, Some(33));
    g.accept(33).unwrap();
    assert_eq!(g.loot_until_enough(750), 8);
    assert_eq!(g.p.level, 1, "{} XP so far", g.xp);
    g.turn_in(33, eagan, Some(0)).unwrap();
    assert_eq!(g.held(750), 0, "the meat is handed in");
    assert_eq!(g.p.level, 2, "505 XP: level 2");

    // 5. Investigate Echo Ridge, then Skirmish (offered at once), then Report to Goldshire.
    g.accept(15).unwrap();
    g.kill(257, 10);
    let r = g.turn_in(15, mcbride, None).unwrap();
    assert_eq!(r.next_in_chain, Some(21));
    g.accept(21).unwrap();
    g.kill(80, 12);
    g.turn_in(21, mcbride, Some(0)).unwrap();
    g.accept(54).unwrap();
    assert_eq!(
        g.held(745),
        1,
        "McBride's Documents were handed over at the accept"
    );
    assert_eq!(
        g.turn_in(54, 240, None),
        Err(TurnInError::OutsideTheValley),
        "Marshal Dughan stands in Goldshire"
    );

    // 6. The level-2 quests: Brotherhood of Thieves, Garrick, Milly.
    g.accept(18).unwrap();
    assert_eq!(g.loot_until_enough(752), 12);
    assert_eq!(
        g.turn_in(18, willem, None),
        Err(TurnInError::ChoiceRequired)
    );
    g.turn_in(18, willem, Some(2)).unwrap();
    g.accept(6).unwrap();
    assert_eq!(g.loot_until_enough(182), 1, "Garrick's Head");
    g.turn_in(6, willem, Some(0)).unwrap();
    g.accept(3903).unwrap();
    let r = g.turn_in(3903, milly, None).unwrap();
    assert_eq!(r.next_in_chain, Some(3904));
    g.accept(3904).unwrap();
    assert_eq!(g.loot_until_enough(11119), 8, "eight crates of grapes");
    let r = g.turn_in(3904, milly, None).unwrap();
    assert_eq!(r.next_in_chain, Some(3905));
    g.accept(3905).unwrap();
    g.turn_in(3905, 952, Some(0)).unwrap();

    // 7. The inn-keeper's errand can be taken too, and is parked like 54.
    g.accept(2158).unwrap();
    assert_eq!(
        g.turn_in(2158, 295, Some(0)),
        Err(TurnInError::OutsideTheValley)
    );

    // The end state.
    let done = [783, 7, 3100, 5261, 33, 15, 21, 18, 6, 3903, 3904, 3905];
    for q in done {
        assert!(g.log.is_rewarded(q), "quest {q} is handed in");
    }
    assert_eq!(
        g.log.active(),
        vec![54, 2158],
        "the two whose enders are in Goldshire"
    );
    let want_xp: u32 = done.map(xp_of).iter().sum();
    assert_eq!((g.xp, want_xp), (2465, 2465));
    assert_eq!(g.p.level, 3);
    assert_eq!(g.money, done.map(money_of).iter().sum::<i64>());
    // Nothing else is takeable: the other classes' letters, the Priest and Warlock quests, the
    // Stormwind chain, the orphans.
    for q in [
        3101, 3102, 3103, 3104, 3105, 5623, 1598, 344, 345, 346, 347, 5805, 7961, 7962,
    ] {
        assert!(
            super::rules::why_not(&g.book, &g.log, &g.p, q).is_some(),
            "quest {q} must not be takeable"
        );
    }
    // Everything the Warrior can take is taken: the marks over the valley's NPCs show nothing to give.
    for npc in [mcbride, willem, eagan, milly] {
        let mark = super::rules::marks(&g.book, &g.log, &g.p, Npc::Creature(npc));
        assert_ne!(mark, super::rules::Mark::Available, "NPC {npc}");
    }
}
