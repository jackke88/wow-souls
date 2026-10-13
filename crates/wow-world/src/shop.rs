//! The shops: what a vendor sells and what everything costs in souls. WoW's own vendor rows and prices
//! (copper) are the data; the game has one currency, the souls the player carries (the Dark Souls
//! counter), so a copper price is turned into souls at [`SOULS_PER_COPPER`]. A kobold's kill gives 150
//! souls, so a Cracked Leather Vest (60 copper) is 240: about a kill and a half of work.
//!
//! Pure rules over [`Item`] rows and the pack ([`Progress`]); the game's shop screen draws them and the
//! save is published by the caller after a purchase or a sale succeeded.

use crate::inventory::Progress;
use crate::model::{Item, Vendor};

/// How many souls one copper piece of WoW's prices is worth: quest copper is paid out in souls at this
/// rate, vendor prices are charged at it, and a sale pays WoW's sell price at it.
pub const SOULS_PER_COPPER: u32 = 4;

/// The souls an item costs at a vendor; `None` for an item that is not for sale (no price).
pub fn buy_cost(item: &Item) -> Option<u32> {
    (item.buy_price > 0).then(|| item.buy_price.saturating_mul(SOULS_PER_COPPER))
}

/// The souls a vendor pays for one of an item; `None` for an item that has no value (a quest item).
pub fn sell_value(item: &Item) -> Option<u32> {
    (item.sell_price > 0).then(|| item.sell_price.saturating_mul(SOULS_PER_COPPER))
}

/// The copper of a quest reward as souls.
pub fn souls_for_copper(copper: u32) -> u32 {
    copper.saturating_mul(SOULS_PER_COPPER)
}

/// What the vendor `npc` sells: the item entries, in the database's order, each once.
pub fn stock(vendors: &[Vendor], npc: u32) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    for v in vendors.iter().filter(|v| v.npc == npc) {
        if !out.contains(&v.item) {
            out.push(v.item);
        }
    }
    out
}

/// Why a purchase or a sale did not happen. [`Refusal::message`] is WoW's wording where it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The vendor does not sell it, or it has no price.
    NotForSale,
    /// The souls are short.
    NotEnoughSouls { need: u32, have: u32 },
    /// The pack does not hold that many.
    NotCarried,
    /// It is worth nothing to a vendor.
    NoValue,
    /// An accepted quest still wants it.
    Needed,
    /// The pack cannot take it (a count that overflows).
    Full,
    /// The banker is offered more souls than are carried (or none).
    NotEnoughCarried,
    /// More is asked of the bank than it holds.
    NotEnoughBanked,
}

impl Refusal {
    pub fn message(self) -> String {
        match self {
            Refusal::NotForSale => "That item is not for sale.".into(),
            Refusal::NotEnoughSouls { need, have } => {
                format!("Not enough souls: it costs {need}, you carry {have}.")
            }
            Refusal::NotCarried => "You do not have that item.".into(),
            Refusal::NoValue => "That item cannot be sold.".into(),
            Refusal::Needed => "A quest still needs that item.".into(),
            Refusal::Full => "You cannot carry any more of that.".into(),
            Refusal::NotEnoughCarried => "You do not carry that many souls.".into(),
            Refusal::NotEnoughBanked => "The bank does not hold that many souls.".into(),
        }
    }
}

/// Buys `count` of `item`: takes the souls and puts the items in the pack. Returns the cost; nothing
/// changes when it refuses.
pub fn buy(
    progress: &mut Progress,
    souls: &mut u32,
    item: &Item,
    count: u32,
) -> Result<u32, Refusal> {
    let each = buy_cost(item).ok_or(Refusal::NotForSale)?;
    let cost = each.checked_mul(count.max(1)).ok_or(Refusal::Full)?;
    if cost > *souls {
        return Err(Refusal::NotEnoughSouls {
            need: cost,
            have: *souls,
        });
    }
    progress
        .inventory
        .add(&[(item.entry, count.max(1))])
        .map_err(|_| Refusal::Full)?;
    *souls -= cost;
    Ok(cost)
}

/// Sells `count` of `item` from the pack for souls. `needed(entry)` is how many of an item an accepted
/// quest still wants (they stay). Returns the souls paid; nothing changes when it refuses.
pub fn sell(
    progress: &mut Progress,
    souls: &mut u32,
    item: &Item,
    count: u32,
    needed: &dyn Fn(u32) -> u32,
) -> Result<u32, Refusal> {
    let value = sell_value(item).ok_or(Refusal::NoValue)?;
    let carried = progress.inventory.count(item.entry);
    if count == 0 || carried < count {
        return Err(Refusal::NotCarried);
    }
    if carried - count < needed(item.entry) {
        return Err(Refusal::Needed);
    }
    let pay = value.checked_mul(count).ok_or(Refusal::Full)?;
    progress
        .inventory
        .remove(item.entry, count)
        .map_err(|_| Refusal::NotCarried)?;
    *souls = souls.saturating_add(pay);
    Ok(pay)
}

/// The share of souls put in the bank that goes to the Abbey (percent): safety costs, so dying with a full
/// purse still hurts.
pub const TITHE_PERCENT: u32 = 10;

/// The Abbey's share of a deposit of `amount`.
pub fn tithe(amount: u32) -> u32 {
    (u64::from(amount) * u64::from(TITHE_PERCENT) / 100) as u32
}

/// Puts `amount` of the carried souls in the bank, less the tithe. Returns `(banked, tithe)`; nothing
/// changes on a refusal.
pub fn deposit(carried: &mut u32, banked: &mut u32, amount: u32) -> Result<(u32, u32), Refusal> {
    if amount == 0 || amount > *carried {
        return Err(Refusal::NotEnoughCarried);
    }
    let fee = tithe(amount);
    let keep = amount - fee;
    let total = banked.checked_add(keep).ok_or(Refusal::Full)?;
    *carried -= amount;
    *banked = total;
    Ok((keep, fee))
}

/// Takes `amount` souls out of the bank, free of charge.
pub fn withdraw(carried: &mut u32, banked: &mut u32, amount: u32) -> Result<u32, Refusal> {
    if amount == 0 || amount > *banked {
        return Err(Refusal::NotEnoughBanked);
    }
    let total = carried.checked_add(amount).ok_or(Refusal::Full)?;
    *banked -= amount;
    *carried = total;
    Ok(amount)
}

/// One line of the banker's list: put in or take out `amount` souls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub deposit: bool,
    pub amount: u32,
    /// "all" rather than a fixed sum.
    pub all: bool,
}

/// The banker's list for what is carried and banked: everything, then 100 and 1000 where there is more
/// than that.
pub fn steps(carried: u32, banked: u32) -> Vec<Step> {
    let mut out = Vec::new();
    for (deposit, have) in [(true, carried), (false, banked)] {
        if have > 0 {
            out.push(Step {
                deposit,
                amount: have,
                all: true,
            });
        }
        for amount in [100, 1000] {
            if have > amount {
                out.push(Step {
                    deposit,
                    amount,
                    all: false,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(entry: u32, buy: u32, sell: u32) -> Item {
        Item {
            entry,
            name: format!("Item {entry}"),
            class: 4,
            subclass: 2,
            display: 0,
            quality: 1,
            flags: 0,
            buy_price: buy,
            sell_price: sell,
            inventory_type: 5,
            allowable_class: -1,
            allowable_race: -1,
            item_level: 1,
            required_level: 1,
            max_count: 0,
            stackable: 1,
            delay: 0,
            damage: (0.0, 0.0, 0),
            block: 0,
            armor: 0,
            stats: Vec::new(),
        }
    }

    #[test]
    fn prices_are_wows_copper_at_four_souls_to_the_copper() {
        let vest = item(2127, 60, 12);
        assert_eq!(buy_cost(&vest), Some(240));
        assert_eq!(sell_value(&vest), Some(48));
        assert_eq!(buy_cost(&item(1, 0, 5)), None, "no price, not for sale");
        assert_eq!(sell_value(&item(1, 5, 0)), None, "no value, cannot be sold");
        assert_eq!(souls_for_copper(25), 100);
        assert_eq!(buy_cost(&item(1, u32::MAX, 0)), Some(u32::MAX));
    }

    #[test]
    fn a_vendors_stock_is_its_rows_in_order_without_repeats() {
        let rows = |npc, item| Vendor {
            npc,
            item,
            max_count: 0,
            incr_time: 0,
        };
        let v = vec![
            rows(78, 2131),
            rows(152, 159),
            rows(78, 2130),
            rows(78, 2131),
        ];
        assert_eq!(stock(&v, 78), vec![2131, 2130]);
        assert_eq!(stock(&v, 152), vec![159]);
        assert!(stock(&v, 1).is_empty());
    }

    #[test]
    fn buying_takes_souls_and_fills_the_pack_and_selling_does_the_reverse() {
        let vest = item(2127, 60, 12);
        let mut p = Progress::default();
        let mut souls = 300;
        assert_eq!(buy(&mut p, &mut souls, &vest, 1), Ok(240));
        assert_eq!((souls, p.inventory.count(2127)), (60, 1));
        assert_eq!(sell(&mut p, &mut souls, &vest, 1, &|_| 0), Ok(48));
        assert_eq!((souls, p.inventory.count(2127)), (108, 0));
    }

    #[test]
    fn a_refused_purchase_changes_nothing() {
        let vest = item(2127, 60, 12);
        let mut p = Progress::default();
        let mut souls = 100;
        let before = p.clone();
        assert_eq!(
            buy(&mut p, &mut souls, &vest, 1),
            Err(Refusal::NotEnoughSouls {
                need: 240,
                have: 100
            })
        );
        assert_eq!(
            buy(&mut p, &mut souls, &item(9, 0, 0), 1),
            Err(Refusal::NotForSale)
        );
        assert_eq!((souls, p), (100, before));
        assert!(Refusal::NotEnoughSouls {
            need: 240,
            have: 100
        }
        .message()
        .contains("240"));
    }

    #[test]
    fn a_stack_sells_whole_but_keeps_what_a_quest_needs() {
        let fang = item(7073, 0, 3);
        let mut p = Progress::default();
        p.inventory.add(&[(7073, 23)]).unwrap();
        let mut souls = 0;
        // A quest wants 8 of them: all 23 may not go, 15 may.
        let needed = |e: u32| if e == 7073 { 8 } else { 0 };
        assert_eq!(
            sell(&mut p, &mut souls, &fang, 23, &needed),
            Err(Refusal::Needed)
        );
        assert_eq!(sell(&mut p, &mut souls, &fang, 15, &needed), Ok(180));
        assert_eq!((souls, p.inventory.count(7073)), (180, 8));
        assert_eq!(
            sell(&mut p, &mut souls, &fang, 1, &needed),
            Err(Refusal::Needed)
        );
        assert_eq!(
            sell(&mut p, &mut souls, &fang, 0, &needed),
            Err(Refusal::NotCarried)
        );
        assert_eq!(
            sell(&mut p, &mut souls, &item(1, 5, 0), 1, &needed),
            Err(Refusal::NoValue)
        );
        assert_eq!(
            sell(&mut p, &mut souls, &item(2, 5, 5), 1, &needed),
            Err(Refusal::NotCarried)
        );
    }

    #[test]
    fn a_deposit_pays_a_tenth_to_the_abbey_and_a_withdrawal_is_free() {
        assert_eq!((tithe(0), tithe(9), tithe(10), tithe(1234)), (0, 0, 1, 123));
        let (mut carried, mut banked) = (1000, 50);
        assert_eq!(deposit(&mut carried, &mut banked, 400), Ok((360, 40)));
        assert_eq!((carried, banked), (600, 410));
        assert_eq!(withdraw(&mut carried, &mut banked, 410), Ok(410));
        assert_eq!((carried, banked), (1010, 0));
    }

    #[test]
    fn the_bank_refuses_what_it_cannot_do_and_changes_nothing() {
        let (mut carried, mut banked) = (100, 20);
        assert_eq!(
            deposit(&mut carried, &mut banked, 101),
            Err(Refusal::NotEnoughCarried)
        );
        assert_eq!(
            deposit(&mut carried, &mut banked, 0),
            Err(Refusal::NotEnoughCarried)
        );
        assert_eq!(
            withdraw(&mut carried, &mut banked, 21),
            Err(Refusal::NotEnoughBanked)
        );
        let (mut c, mut b) = (10, u32::MAX);
        assert_eq!(deposit(&mut c, &mut b, 10), Err(Refusal::Full));
        assert_eq!(((carried, banked), (c, b)), ((100, 20), (10, u32::MAX)));
    }

    #[test]
    fn the_list_offers_all_then_the_round_sums_that_fit() {
        assert!(steps(0, 0).is_empty());
        let s = steps(150, 0);
        assert_eq!(s.len(), 2, "all (150) and 100: {s:?}");
        assert!(s[0].deposit && s[0].all && s[0].amount == 150);
        assert_eq!(s[1].amount, 100);
        let s = steps(5, 2500);
        let words: Vec<_> = s.iter().map(|x| (x.deposit, x.amount, x.all)).collect();
        assert_eq!(
            words,
            vec![
                (true, 5, true),
                (false, 2500, true),
                (false, 100, false),
                (false, 1000, false)
            ]
        );
    }
}
