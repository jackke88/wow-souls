//! Stable, per-source loot rolls. Quest-only items need explicit eligibility from the quest log.
use crate::model::{ItemCount, LootKind, LootRow};
use anyhow::{ensure, Result};
use std::collections::{BTreeMap, BTreeSet};

struct Dice(u64);
impl Dice {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((z ^ (z >> 31)) >> 40) as f32 / 16_777_216.0
    }
}
pub fn roll(
    rows: &[LootRow],
    kind: LootKind,
    table: u32,
    seed: u64,
    eligible: &BTreeSet<u32>,
) -> Result<Vec<ItemCount>> {
    roll_with(rows, kind, table, seed, eligible, Chances::Wow)
}

/// How a roll treats the table's chances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chances {
    /// The table's own percentages.
    Wow,
    /// Everything drops (2026-10-17, the user: "Jag vill ha 100% drop rate på allt"): every row outside a
    /// group (and every reference) is awarded, every group gives one of its rows, quest items whenever the
    /// quest log asks for them. A group's pick still follows its rows' weights.
    Always,
    /// Quest items always drop when the quest log asks for them (2026-10-18, the user: "drop rates ska
    /// bara vara 100% på quest items - inte på allt"); everything else at the table's own chance.
    QuestItems,
}

/// [`roll`] with the chances chosen.
pub fn roll_with(
    rows: &[LootRow],
    kind: LootKind,
    table: u32,
    seed: u64,
    eligible: &BTreeSet<u32>,
    chances: Chances,
) -> Result<Vec<ItemCount>> {
    let mut items = BTreeMap::<u32, u32>::new();
    let mut roll = Roll {
        dice: Dice(seed),
        eligible,
        always: chances == Chances::Always,
        quests_always: chances == Chances::QuestItems,
    };
    process(rows, kind, table, &mut roll, &mut items, 0)?;
    Ok(items.into_iter().collect())
}

struct Roll<'a> {
    dice: Dice,
    eligible: &'a BTreeSet<u32>,
    always: bool,
    quests_always: bool,
}
/// The item a boss or a rare is guaranteed the first time it dies: of the table's non-quest rows
/// (reference groups expanded), gear (a weapon or armor that goes on a slot) before anything else, then
/// the higher quality, the higher item level, and the lower drop chance. `None` for a table with no
/// known item. Quest rows (a negative chance) and items the item table lacks are left out.
pub fn best_drop(
    rows: &[LootRow],
    items: &[crate::model::Item],
    kind: LootKind,
    table: u32,
) -> Option<u32> {
    let mut found: Vec<(u32, f32)> = Vec::new();
    collect(rows, kind, table, 0, &mut found);
    found
        .into_iter()
        .filter_map(|(entry, chance)| {
            let item = items.iter().find(|i| i.entry == entry)?;
            let is_gear = matches!(item.class, 2 | 4) && item.inventory_type != 0;
            // A chance of 0 is "an equal share of its group": treated as common.
            let chance = if chance > 0.0 { chance } else { 100.0 };
            Some((entry, (is_gear, item.quality, item.item_level), chance))
        })
        .max_by(|a, b| {
            a.1.cmp(&b.1)
                .then(b.2.total_cmp(&a.2)) // the lower chance is the better
                .then(b.0.cmp(&a.0)) // a stable tie: the lower entry
        })
        .map(|(entry, _, _)| entry)
}

fn collect(rows: &[LootRow], kind: LootKind, table: u32, depth: usize, out: &mut Vec<(u32, f32)>) {
    if depth >= 12 {
        return;
    }
    for row in rows.iter().filter(|r| r.kind == kind && r.table == table) {
        if row.chance < 0.0 {
            continue;
        }
        if row.min_or_ref < 0 {
            collect(
                rows,
                LootKind::Reference,
                row.min_or_ref.unsigned_abs(),
                depth + 1,
                out,
            );
        } else if row.item > 0 {
            out.push((row.item, row.chance));
        }
    }
}

fn process(
    rows: &[LootRow],
    kind: LootKind,
    table: u32,
    roll: &mut Roll,
    items: &mut BTreeMap<u32, u32>,
    depth: usize,
) -> Result<()> {
    ensure!(depth < 12, "cyclic/deep loot reference");
    let mut groups = BTreeMap::<u32, Vec<&LootRow>>::new();
    for row in rows.iter().filter(|r| r.kind == kind && r.table == table) {
        ensure!(
            row.chance.is_finite() && row.chance.abs() <= 100.0,
            "invalid loot chance"
        );
        if row.chance < 0.0 && !roll.eligible.contains(&row.item) {
            continue;
        }
        if row.group == 0 || row.min_or_ref < 0 {
            let quest = row.chance < 0.0 && roll.quests_always;
            if roll.always || quest || roll.dice.unit() * 100.0 < row.chance.abs() {
                award(rows, row, roll, items, depth)?;
            }
        } else {
            groups.entry(row.group).or_default().push(row);
        }
    }
    for group in groups.values() {
        // Always: the draw spans only what the rows give, so it never falls past them all.
        let explicit: f32 = group.iter().map(|r| r.chance.abs()).sum();
        let has_equal = group.iter().any(|r| r.chance == 0.0);
        let span = if !roll.always {
            100.0
        } else if has_equal {
            explicit.max(100.0)
        } else {
            explicit
        };
        let mut draw = roll.dice.unit() * span;
        let mut selected = None;
        for row in group.iter().filter(|r| r.chance != 0.0) {
            draw -= row.chance.abs();
            if draw < 0.0 {
                selected = Some(*row);
                break;
            }
        }
        if selected.is_none() {
            let equal: Vec<_> = group.iter().filter(|r| r.chance == 0.0).collect();
            if !equal.is_empty() {
                selected = Some(*equal[(roll.dice.unit() * equal.len() as f32) as usize]);
            } else if roll.always {
                // Rounding at the very end of the span: the last row.
                selected = group.last().copied();
            }
        }
        if let Some(row) = selected {
            award(rows, row, roll, items, depth)?;
        }
    }
    Ok(())
}
fn award(
    rows: &[LootRow],
    row: &LootRow,
    roll: &mut Roll,
    items: &mut BTreeMap<u32, u32>,
    depth: usize,
) -> Result<()> {
    if row.min_or_ref < 0 {
        ensure!(row.max_count <= 100, "unbounded loot reference count");
        for _ in 0..row.max_count {
            process(
                rows,
                LootKind::Reference,
                row.min_or_ref.unsigned_abs(),
                roll,
                items,
                depth + 1,
            )?;
        }
    } else {
        let min = row.min_or_ref as u32;
        ensure!(
            min > 0 && row.max_count >= min && row.item > 0,
            "invalid loot count/item"
        );
        let span = row.max_count - min;
        let count = min + (roll.dice.unit() * (span as f32 + 1.0)) as u32;
        let total = items
            .get(&row.item)
            .copied()
            .unwrap_or(0)
            .checked_add(count)
            .ok_or_else(|| anyhow::anyhow!("loot count overflow"))?;
        items.insert(row.item, total);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row(item: u32, chance: f32, group: u32) -> LootRow {
        LootRow {
            kind: LootKind::Creature,
            table: 1,
            item,
            chance,
            group,
            min_or_ref: 1,
            max_count: 1,
        }
    }
    #[test]
    fn quest_items_always_drop_and_the_rest_keep_their_chance() {
        let rows = [row(1, 0.5, 0), row(2, -3.0, 0)];
        let mut normal = 0;
        for seed in 0..200 {
            let got = roll_with(
                &rows,
                LootKind::Creature,
                1,
                seed,
                &BTreeSet::from([2]),
                Chances::QuestItems,
            )
            .unwrap();
            assert!(
                got.iter().any(|(i, _)| *i == 2),
                "the quest item every time"
            );
            normal += usize::from(got.iter().any(|(i, _)| *i == 1));
        }
        assert!(normal < 20, "the 0.5 % item stays rare: {normal}");
    }

    #[test]
    fn always_awards_every_loose_row_and_one_of_every_group() {
        let rows = [
            row(1, 0.5, 0),
            row(2, -3.0, 0),
            row(3, 2.0, 1),
            row(4, 1.0, 1),
            row(5, 0.0, 2),
            row(6, 0.0, 2),
        ];
        for seed in 0..64 {
            let got = roll_with(
                &rows,
                LootKind::Creature,
                1,
                seed,
                &BTreeSet::from([2]),
                Chances::Always,
            )
            .unwrap();
            let has = |e: u32| got.iter().any(|(i, _)| *i == e);
            assert!(has(1) && has(2), "{got:?}");
            assert_eq!([3, 4].iter().filter(|&&e| has(e)).count(), 1, "{got:?}");
            assert_eq!([5, 6].iter().filter(|&&e| has(e)).count(), 1, "{got:?}");
        }
        // A quest item nobody asks for still stays away.
        let got = roll_with(
            &rows,
            LootKind::Creature,
            1,
            7,
            &BTreeSet::new(),
            Chances::Always,
        )
        .unwrap();
        assert!(!got.iter().any(|(i, _)| *i == 2));
    }

    #[test]
    fn quest_only_drops_require_an_eligible_quest_and_normal_drops_do_not() {
        let rows = [row(1, 100.0, 0), row(2, -100.0, 0)];
        assert_eq!(
            roll(&rows, LootKind::Creature, 1, 0, &BTreeSet::new()).unwrap(),
            vec![(1, 1)]
        );
        assert_eq!(
            roll(&rows, LootKind::Creature, 1, 0, &BTreeSet::from([2])).unwrap(),
            vec![(1, 1), (2, 1)]
        );
    }
    #[test]
    fn grouped_roll_selects_one_and_is_stable_for_a_source() {
        let rows = [row(1, 40.0, 1), row(2, 0.0, 1), row(3, 0.0, 1)];
        for seed in 0..100 {
            let a = roll(&rows, LootKind::Creature, 1, seed, &BTreeSet::new()).unwrap();
            assert_eq!(a.len(), 1);
            assert_eq!(
                a,
                roll(&rows, LootKind::Creature, 1, seed, &BTreeSet::new()).unwrap()
            );
        }
    }
    #[test]
    fn references_expand_and_cycles_fail() {
        let mut r = row(0, 100.0, 0);
        r.min_or_ref = -2;
        let mut child = row(3, 100.0, 0);
        child.kind = LootKind::Reference;
        child.table = 2;
        assert_eq!(
            roll(
                &[r.clone(), child],
                LootKind::Creature,
                1,
                0,
                &BTreeSet::new()
            )
            .unwrap(),
            vec![(3, 1)]
        );
        let mut cycle = r.clone();
        cycle.kind = LootKind::Reference;
        cycle.table = 2;
        assert!(roll(&[r, cycle], LootKind::Creature, 1, 0, &BTreeSet::new()).is_err());
    }

    fn gear(entry: u32, class: u32, quality: u32, ilvl: u32, inv_type: u32) -> crate::model::Item {
        crate::model::Item {
            entry,
            name: format!("Item {entry}"),
            class,
            subclass: 0,
            display: 0,
            quality,
            flags: 0,
            buy_price: 0,
            sell_price: 0,
            inventory_type: inv_type,
            allowable_class: -1,
            allowable_race: -1,
            item_level: ilvl,
            required_level: 1,
            max_count: 0,
            stackable: 1,
            delay: 0,
            damage: (0.0, 0.0, 0),
            block: 0,
            armor: 0,
            stats: vec![],
        }
    }

    #[test]
    fn the_best_drop_prefers_gear_then_quality_then_item_level_then_the_rarer_chance() {
        let rows = [
            row(1, 12.0, 0),
            row(2, 0.8, 0),
            row(3, 5.0, 0),
            row(4, 2.0, 0),
            row(5, 9.0, 0),
        ];
        let items = [
            gear(1, 0, 1, 5, 0),  // a consumable, ilvl 5
            gear(2, 2, 1, 4, 21), // a q1 weapon, ilvl 4, 0.8 %
            gear(3, 4, 1, 4, 5),  // a q1 chest piece, ilvl 4, 5 %
            gear(4, 4, 2, 3, 7),  // a q2 legs piece, ilvl 3, 2 %
            gear(5, 4, 2, 3, 8),  // a q2 boots piece, ilvl 3, 9 %: commoner than 4
        ];
        assert_eq!(
            best_drop(&rows, &items, LootKind::Creature, 1),
            Some(4),
            "q2 gear, then the rarer"
        );
        assert_eq!(
            best_drop(&rows[..3], &items[..3], LootKind::Creature, 1),
            Some(2),
            "q1 gear of ilvl 4: the rarer chance wins"
        );
    }

    #[test]
    fn quest_rows_and_unknown_items_are_never_the_best() {
        let rows = [row(9, -100.0, 0), row(8, 50.0, 0)];
        let items = [gear(9, 2, 3, 30, 21)]; // 8 has no row in the item table
        assert_eq!(best_drop(&rows, &items, LootKind::Creature, 1), None);
    }

    #[test]
    fn references_are_expanded_and_a_loop_ends() {
        let mut to_ref = row(0, 2.5, 0);
        to_ref.min_or_ref = -2;
        to_ref.max_count = 1;
        let mut inside = row(7, 0.0, 1);
        inside.kind = LootKind::Reference;
        inside.table = 2;
        let items = [gear(7, 4, 0, 9, 5)];
        assert_eq!(
            best_drop(&[to_ref.clone(), inside], &items, LootKind::Creature, 1),
            Some(7)
        );
        let mut cycle = to_ref.clone();
        cycle.kind = LootKind::Reference;
        cycle.table = 2;
        assert_eq!(
            best_drop(&[to_ref, cycle], &items, LootKind::Creature, 1),
            None,
            "a loop yields nothing, no panic"
        );
    }

    #[test]
    fn an_empty_table_has_no_best_drop() {
        assert_eq!(best_drop(&[], &[], LootKind::Creature, 1), None);
    }
}
