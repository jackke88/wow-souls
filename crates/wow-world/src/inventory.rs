//! Canonical item ownership, shared by loot, rewards and future quest item requirements.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;

use crate::equipment::{self, Gear, Refusal, Slot, Wearer};
use crate::model::{Item, ItemCount};
use anyhow::{bail, ensure, Context, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inventory {
    pub items: BTreeMap<u32, u32>,
    /// Most recently acquired item first, unique, capped at 24 entries.
    pub recent: Vec<u32>,
}
impl Inventory {
    pub fn count(&self, item: u32) -> u32 {
        self.items.get(&item).copied().unwrap_or(0)
    }
    pub fn add(&mut self, items: &[ItemCount]) -> Result<()> {
        let mut next = self.clone();
        for &(item, count) in items {
            ensure!(item > 0 && count > 0, "invalid item/count");
            let total = next
                .count(item)
                .checked_add(count)
                .context("inventory count overflow")?;
            next.items.insert(item, total);
            next.recent.retain(|&i| i != item);
            next.recent.insert(0, item);
        }
        next.recent.truncate(24);
        *self = next;
        Ok(())
    }
    pub fn remove(&mut self, item: u32, count: u32) -> Result<()> {
        ensure!(count > 0 && self.count(item) >= count, "item not owned");
        let left = self.count(item) - count;
        if left == 0 {
            self.items.remove(&item);
            self.recent.retain(|&i| i != item);
        } else {
            self.items.insert(item, left);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChestState {
    Open(Vec<ItemCount>),
    /// UNIX seconds when a baseline chest can restock; MAX means authored one-time treasure.
    Claimed(u64),
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    pub inventory: Inventory,
    /// What the character wears; a worn item is not counted in `inventory`.
    pub gear: Gear,
    /// The starting kit was handed out (a new character, or an old save the first time it loads).
    pub kit_given: bool,
    /// The trainer's abilities the character has learned (spell ids; the starting ones are not listed).
    pub abilities: BTreeSet<u32>,
    pub chests: BTreeMap<u32, ChestState>,
}
impl Progress {
    /// Puts a carried item on, in `slot` or where a quick equip would. The slot's old occupant (and
    /// the off hand, under a two-hander) goes back to the inventory. Nothing changes when it refuses.
    pub fn equip(
        &mut self,
        items: &[Item],
        entry: u32,
        slot: Option<Slot>,
        wearer: Wearer,
    ) -> Result<Slot, Refusal> {
        let item = items
            .iter()
            .find(|i| i.entry == entry)
            .ok_or(Refusal::NotWearable)?;
        equipment::can_wear(item, wearer)?;
        if self.inventory.count(entry) == 0 {
            return Err(Refusal::NotOwned);
        }
        let slot = match slot.or_else(|| self.gear.best_slot(items, item, wearer)) {
            Some(slot) => slot,
            // Every slot refused: say why the first one did.
            None => {
                let first = equipment::slots_for(item.inventory_type)[0];
                return Err(self
                    .gear
                    .plan(items, item, first, wearer)
                    .expect_err("best_slot found none, so the first slot refuses"));
            }
        };
        let taken = self.gear.plan(items, item, slot, wearer)?;
        let mut inventory = self.inventory.clone();
        inventory.remove(entry, 1).map_err(|_| Refusal::NotOwned)?;
        let back: Vec<ItemCount> = taken.iter().map(|&(_, e)| (e, 1)).collect();
        inventory.add(&back).map_err(|_| Refusal::NotOwned)?;
        let mut gear = self.gear.clone();
        for &(s, _) in &taken {
            gear.set(s, None);
        }
        gear.set(slot, Some(entry));
        self.inventory = inventory;
        self.gear = gear;
        Ok(slot)
    }

    /// The one rule for what a character is handed (a drop, a chest, a reward, a purchase): each
    /// piece of `got` that fits an EMPTY slot goes on at once, one unit at a time, in order. Nothing
    /// worn is ever moved or replaced, and what `keep` names (a quest counts it in the pack) and
    /// what the wearer cannot use stays carried. `got` must already be in the pack. Returns the
    /// items that went on and the slots they took.
    pub fn put_on_new(
        &mut self,
        items: &[Item],
        got: &[ItemCount],
        wearer: Wearer,
        keep: &dyn Fn(u32) -> bool,
    ) -> Vec<(u32, Slot)> {
        let mut put = Vec::new();
        for &(entry, count) in got {
            if keep(entry) {
                continue;
            }
            let Some(item) = items.iter().find(|i| i.entry == entry) else {
                continue;
            };
            for _ in 0..count {
                if self.inventory.count(entry) == 0 {
                    break;
                }
                let Some(slot) = self.gear.free_slot(items, item, wearer) else {
                    break;
                };
                if self.equip(items, entry, Some(slot), wearer).is_err() {
                    break;
                }
                put.push((entry, slot));
            }
        }
        put
    }

    /// Takes what is worn in `slot` off, back into the inventory; the item's entry.
    pub fn unequip(&mut self, slot: Slot) -> Result<u32, Refusal> {
        let entry = self.gear.get(slot).ok_or(Refusal::Empty)?;
        let mut inventory = self.inventory.clone();
        inventory
            .add(&[(entry, 1)])
            .map_err(|_| Refusal::NotOwned)?;
        self.inventory = inventory;
        self.gear.set(slot, None);
        Ok(entry)
    }

    /// Dresses a new character in its starting kit, once; whether it did.
    pub fn grant_start_kit(&mut self, race: u8, class: u8) -> bool {
        if self.kit_given {
            return false;
        }
        for &(slot, item) in equipment::start_kit(race, class) {
            if self.gear.get(slot).is_none() {
                self.gear.set(slot, Some(item));
            }
        }
        self.kit_given = true;
        true
    }

    pub fn open(&mut self, id: u32, items: Vec<ItemCount>) -> Result<()> {
        ensure!(!self.chests.contains_key(&id), "chest already opened");
        self.chests.insert(id, ChestState::Open(items));
        Ok(())
    }
    pub fn claim(&mut self, id: u32, restock_at: u64) -> Result<Vec<ItemCount>> {
        let items = match self.chests.get(&id) {
            Some(ChestState::Open(items)) => items.clone(),
            _ => bail!("chest not open or already claimed"),
        };
        self.inventory.add(&items)?;
        self.chests.insert(id, ChestState::Claimed(restock_at));
        Ok(items)
    }
    pub fn text(&self) -> String {
        let mut s = String::from("wow-souls-items 1\n");
        for (id, count) in &self.inventory.items {
            writeln!(s, "item {id} {count}").unwrap();
        }
        for id in &self.inventory.recent {
            writeln!(s, "recent {id}").unwrap();
        }
        equipment::write_lines(&self.gear, self.kit_given, &mut s);
        for spell in &self.abilities {
            writeln!(s, "learn {spell}").unwrap();
        }
        for (id, state) in &self.chests {
            match state {
                ChestState::Open(items) => {
                    writeln!(s, "chest {id} open").unwrap();
                    for (item, count) in items {
                        writeln!(s, "pending {id} {item} {count}").unwrap();
                    }
                }
                ChestState::Claimed(at) => {
                    writeln!(s, "chest {id} claimed {at}").unwrap();
                }
            }
        }
        s
    }
    pub fn parse(s: &str) -> Result<Self> {
        let mut lines = s.lines();
        ensure!(
            lines.next() == Some("wow-souls-items 1"),
            "unknown inventory save version"
        );
        let mut p = Self::default();
        let number = |s: &str| -> Result<u32> {
            let n = s.parse()?;
            ensure!(n > 0, "zero save id/count");
            Ok(n)
        };
        for line in lines {
            let w: Vec<_> = line.split_whitespace().collect();
            match w[..] {
                ["item", id, count] => {
                    ensure!(
                        p.inventory
                            .items
                            .insert(number(id)?, number(count)?)
                            .is_none(),
                        "duplicate item"
                    );
                }
                ["recent", id] => {
                    let id = number(id)?;
                    ensure!(
                        !p.inventory.recent.contains(&id) && p.inventory.recent.len() < 24,
                        "invalid recent items"
                    );
                    p.inventory.recent.push(id);
                }
                ["chest", id, "open"] => {
                    ensure!(
                        p.chests
                            .insert(number(id)?, ChestState::Open(Vec::new()))
                            .is_none(),
                        "duplicate chest"
                    );
                }
                ["chest", id, "claimed", at] => {
                    ensure!(
                        p.chests
                            .insert(number(id)?, ChestState::Claimed(at.parse()?))
                            .is_none(),
                        "duplicate chest"
                    );
                }
                ["pending", id, item, count] => {
                    let Some(ChestState::Open(items)) = p.chests.get_mut(&number(id)?) else {
                        bail!("pending items without open chest");
                    };
                    items.push((number(item)?, number(count)?));
                }
                ["learn", spell] => {
                    ensure!(
                        p.abilities.insert(number(spell)?),
                        "duplicate learned ability"
                    );
                }
                _ => ensure!(
                    equipment::parse_line(&w, &mut p.gear, &mut p.kit_given)?,
                    "invalid inventory save line: {line}"
                ),
            }
        }
        ensure!(
            p.inventory
                .recent
                .iter()
                .all(|id| p.inventory.items.contains_key(id)),
            "recent item not owned"
        );
        Ok(p)
    }
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::parse(&s).with_context(|| format!("reading {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    /// A failed write leaves both the previous file and runtime owner unchanged.
    pub fn publish(&mut self, next: Self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("tmp");
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(next.text().as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_unclaimed_and_claimed_chests_round_trip_without_duplicate_rewards() {
        let mut p = Progress::default();
        p.open(900001, vec![(900001, 3)]).unwrap();
        let mut p = Progress::parse(&p.text()).unwrap();
        p.claim(900001, u64::MAX).unwrap();
        let mut p = Progress::parse(&p.text()).unwrap();
        assert!(p.claim(900001, u64::MAX).is_err());
        assert_eq!(p.inventory.count(900001), 3);
    }
    #[test]
    fn overflow_in_a_multi_item_grant_leaves_every_item_unchanged() {
        let mut p = Inventory::default();
        p.add(&[(1, u32::MAX)]).unwrap();
        let old = p.clone();
        assert!(p.add(&[(2, 3), (1, 1)]).is_err());
        assert_eq!(p, old);
    }
    #[test]
    fn failed_save_does_not_publish_ownership() {
        let mut p = Progress::default();
        let mut n = p.clone();
        n.inventory.add(&[(1, 3)]).unwrap();
        // Parent is a file, on all platforms, so this deterministically fails before publication.
        let file = std::env::temp_dir().join(format!("loot-save-fail-{}", std::process::id()));
        std::fs::write(&file, "keep").unwrap();
        assert!(p.publish(n, &file.join("save.txt")).is_err());
        assert!(p.inventory.items.is_empty());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "keep");
        std::fs::remove_file(file).unwrap();
    }
    #[test]
    fn corrupt_saves_are_rejected_instead_of_resetting_progress() {
        for body in [
            "item 1 2\nitem 1 3",
            "recent 1",
            "pending 2 1 3",
            "chest 2 claimed nope",
            "item 0 3",
            "item 1 0",
        ] {
            assert!(
                Progress::parse(&format!("wow-souls-items 1\n{body}")).is_err(),
                "{body}"
            );
        }
    }
    fn gear_items() -> Vec<crate::model::Item> {
        use crate::equipment::{inv, tests::item};
        vec![
            item(100, inv::MAIN_HAND, 2, 7),
            item(101, inv::TWO_HAND, 2, 10),
            item(102, inv::SHIELD, 4, 6),
            item(103, inv::CLOAK, 4, 1),
            item(104, inv::FINGER, 4, 0),
            item(105, 0, 0, 0),
            item(106, inv::CLOAK, 4, 1),
            item(107, inv::WEAPON, 2, 7),
        ]
    }
    const WARRIOR: Wearer = Wearer {
        race: 1,
        class: 1,
        level: 5,
    };

    #[test]
    fn equipping_moves_one_item_from_the_inventory_to_its_slot() {
        let items = gear_items();
        let mut p = Progress::default();
        p.inventory.add(&[(103, 1), (104, 2)]).unwrap();
        assert_eq!(p.equip(&items, 103, None, WARRIOR), Ok(Slot::Back));
        assert_eq!(p.gear.get(Slot::Back), Some(103));
        assert_eq!(p.inventory.count(103), 0);
        // A stack of two rings: one goes on, then the other on the second finger.
        assert_eq!(p.equip(&items, 104, None, WARRIOR), Ok(Slot::Finger1));
        assert_eq!(p.equip(&items, 104, None, WARRIOR), Ok(Slot::Finger2));
        assert_eq!(p.inventory.count(104), 0);
        assert_eq!(p.equip(&items, 104, None, WARRIOR), Err(Refusal::NotOwned));
    }

    #[test]
    fn a_refused_equip_changes_nothing() {
        let items = gear_items();
        let mut p = Progress::default();
        p.inventory.add(&[(100, 1), (102, 1), (105, 1)]).unwrap();
        let before = p.clone();
        assert_eq!(
            p.equip(&items, 100, Some(Slot::OffHand), WARRIOR),
            Err(Refusal::WrongSlot)
        );
        assert_eq!(
            p.equip(&items, 105, None, WARRIOR),
            Err(Refusal::NotWearable)
        );
        assert_eq!(
            p.equip(&items, 999, None, WARRIOR),
            Err(Refusal::NotWearable)
        );
        let green = Wearer {
            level: 0,
            ..WARRIOR
        };
        let mut late = items.clone();
        late[0].required_level = 9;
        assert_eq!(p.equip(&late, 100, None, green), Err(Refusal::Level(9)));
        assert_eq!(p, before);
    }

    #[test]
    fn a_swap_returns_the_old_piece_and_a_two_hander_returns_the_shield_too() {
        let items = gear_items();
        let mut p = Progress::default();
        p.inventory.add(&[(100, 1), (101, 1), (102, 1)]).unwrap();
        p.equip(&items, 100, None, WARRIOR).unwrap();
        p.equip(&items, 102, None, WARRIOR).unwrap();
        assert_eq!(p.gear.get(Slot::OffHand), Some(102));
        // The staff displaces both the sword and the shield.
        assert_eq!(p.equip(&items, 101, None, WARRIOR), Ok(Slot::MainHand));
        assert_eq!(p.gear.get(Slot::MainHand), Some(101));
        assert_eq!(p.gear.get(Slot::OffHand), None);
        assert_eq!((p.inventory.count(100), p.inventory.count(102)), (1, 1));
        // And a shield will not go on beside it.
        assert_eq!(p.equip(&items, 102, None, WARRIOR), Err(Refusal::TwoHanded));
        // Taking it off puts it back in the bag.
        assert_eq!(p.unequip(Slot::MainHand), Ok(101));
        assert_eq!(p.inventory.count(101), 1);
        assert_eq!(p.unequip(Slot::MainHand), Err(Refusal::Empty));
    }

    /// What a character is handed goes into the pack first (loot, a chest, a reward, a purchase),
    /// then `put_on_new` is told what came.
    fn handed(p: &mut Progress, got: &[(u32, u32)]) -> Vec<(u32, Slot)> {
        let items = gear_items();
        p.inventory.add(got).unwrap();
        p.put_on_new(&items, got, WARRIOR, &|_| false)
    }

    #[test]
    fn gear_handed_over_goes_on_when_its_slot_is_empty() {
        let mut p = Progress::default();
        let put = handed(&mut p, &[(103, 1), (100, 1)]);
        assert_eq!(put, vec![(103, Slot::Back), (100, Slot::MainHand)]);
        assert_eq!(p.gear.get(Slot::Back), Some(103));
        assert_eq!(p.gear.get(Slot::MainHand), Some(100));
        assert_eq!((p.inventory.count(103), p.inventory.count(100)), (0, 0));
    }

    #[test]
    fn a_taken_slot_is_never_touched_by_a_new_piece() {
        let mut p = Progress::default();
        handed(&mut p, &[(103, 1)]);
        // A second cloak and a second one-hander: both slots are taken (a warrior cannot dual wield yet).
        let put = handed(&mut p, &[(106, 1), (107, 1)]);
        assert_eq!(
            put,
            vec![(107, Slot::MainHand)],
            "the weapon fills the empty main hand"
        );
        let put = handed(&mut p, &[(106, 1)]);
        assert!(put.is_empty());
        assert_eq!(p.gear.get(Slot::Back), Some(103));
        assert_eq!(p.inventory.count(106), 2, "both cloaks stay carried");
    }

    #[test]
    fn a_two_hander_does_not_push_a_worn_shield_off_and_a_shield_does_not_join_one() {
        let mut p = Progress::default();
        handed(&mut p, &[(100, 1), (102, 1)]);
        // Sword and shield are on; the two-hander would take both off, so it waits in the pack.
        assert!(handed(&mut p, &[(101, 1)]).is_empty());
        assert_eq!(p.gear.get(Slot::MainHand), Some(100));
        assert_eq!(p.gear.get(Slot::OffHand), Some(102));
        assert_eq!(p.inventory.count(101), 1);
        // Two-hander worn, shield off: a shield waits in the pack as well.
        let mut q = Progress::default();
        handed(&mut q, &[(101, 1)]);
        assert!(handed(&mut q, &[(102, 1)]).is_empty());
        assert_eq!(q.gear.get(Slot::OffHand), None);
        assert_eq!(q.inventory.count(102), 1);
    }

    #[test]
    fn a_stack_of_rings_fills_both_fingers_and_the_rest_stays() {
        let mut p = Progress::default();
        let put = handed(&mut p, &[(104, 3)]);
        assert_eq!(put, vec![(104, Slot::Finger1), (104, Slot::Finger2)]);
        assert_eq!(p.inventory.count(104), 1);
    }

    #[test]
    fn quest_items_and_pieces_the_wearer_cannot_use_stay_in_the_pack() {
        let items = gear_items();
        let mut p = Progress::default();
        p.inventory.add(&[(103, 1), (104, 1), (105, 1)]).unwrap();
        // 103 is wanted by a quest: counted in the pack, never worn. 105 is no gear at all.
        let put = p.put_on_new(&items, &[(103, 1), (104, 1), (105, 1)], WARRIOR, &|e| {
            e == 103
        });
        assert_eq!(put, vec![(104, Slot::Finger1)]);
        assert_eq!((p.inventory.count(103), p.inventory.count(105)), (1, 1));
        // A piece above the wearer's level.
        let mut late = items.clone();
        late[3].required_level = 9;
        let mut q = Progress::default();
        q.inventory.add(&[(103, 1)]).unwrap();
        assert!(q
            .put_on_new(&late, &[(103, 1)], WARRIOR, &|_| false)
            .is_empty());
        assert_eq!(q.inventory.count(103), 1);
    }

    #[test]
    fn only_what_was_handed_over_goes_on_and_an_unowned_entry_is_ignored() {
        let items = gear_items();
        let mut p = Progress::default();
        // 103 was carried before; the new thing is the ring.
        p.inventory.add(&[(103, 1), (104, 1)]).unwrap();
        let put = p.put_on_new(&items, &[(104, 1)], WARRIOR, &|_| false);
        assert_eq!(put, vec![(104, Slot::Finger1)]);
        assert_eq!(
            p.inventory.count(103),
            1,
            "older pack items are not retroactively worn"
        );
        // A grant that names something not in the pack changes nothing.
        let before = p.clone();
        assert!(p
            .put_on_new(&items, &[(106, 1)], WARRIOR, &|_| false)
            .is_empty());
        assert_eq!(p, before);
    }

    #[test]
    fn worn_items_and_the_kit_flag_survive_a_save() {
        let items = gear_items();
        let mut p = Progress::default();
        p.inventory.add(&[(103, 1)]).unwrap();
        p.equip(&items, 103, None, WARRIOR).unwrap();
        assert!(p.grant_start_kit(1, 1));
        let back = Progress::parse(&p.text()).unwrap();
        assert_eq!(back, p);
        assert!(back.kit_given);
        assert_eq!(back.gear.get(Slot::MainHand), Some(25));
        assert_eq!(back.gear.get(Slot::Back), Some(103));
        // An old save has no kit line and no gear: it still reads, and gets the kit once.
        let mut old = Progress::parse(
            "wow-souls-items 1
item 9 2
",
        )
        .unwrap();
        assert!(!old.kit_given && old.gear.is_empty());
        assert!(old.grant_start_kit(1, 1));
        assert!(!old.grant_start_kit(1, 1));
        assert_eq!(old.inventory.count(9), 2);
    }

    #[test]
    fn the_kit_never_overwrites_worn_pieces_and_other_characters_get_none() {
        let mut p = Progress::default();
        p.gear.set(Slot::MainHand, Some(7));
        assert!(p.grant_start_kit(1, 1));
        assert_eq!(p.gear.get(Slot::MainHand), Some(7));
        assert_eq!(p.gear.get(Slot::Shirt), Some(38));
        let mut orc = Progress::default();
        assert!(orc.grant_start_kit(2, 1));
        assert!(orc.gear.is_empty() && orc.kit_given);
    }

    #[test]
    fn learned_abilities_survive_a_save_and_a_bad_line_is_refused() {
        let mut p = Progress::default();
        p.abilities.insert(6673);
        p.abilities.insert(772);
        let text = p.text();
        assert!(
            text.contains(
                "learn 772
learn 6673
"
            ),
            "{text}"
        );
        assert_eq!(Progress::parse(&text).unwrap(), p);
        for body in [
            "learn 5
learn 5",
            "learn 0",
            "learn x",
        ] {
            assert!(
                Progress::parse(&format!(
                    "wow-souls-items 1
{body}"
                ))
                .is_err(),
                "{body}"
            );
        }
    }

    #[test]
    fn corrupt_gear_lines_are_rejected_like_the_rest() {
        for body in [
            "equip belt 5",
            "equip head 5
equip head 6",
            "equip head 0",
        ] {
            assert!(
                Progress::parse(&format!(
                    "wow-souls-items 1
{body}"
                ))
                .is_err(),
                "{body}"
            );
        }
    }

    #[test]
    fn consume_cannot_underflow_or_leave_empty_owned_entries() {
        let mut p = Inventory::default();
        p.add(&[(1, 1)]).unwrap();
        assert!(p.remove(1, 2).is_err());
        p.remove(1, 1).unwrap();
        assert!(p.items.is_empty());
        assert!(p.recent.is_empty());
    }
}
