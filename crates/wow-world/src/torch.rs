//! The torch (2026-10-18, the user: "facklor ... man kan köpa det från shops ... en extra gear slot ...
//! hänger vid bältet ... brinna slut efter x mycket tid"). WoW 1.12 sells no torch for a belt, so the item
//! is ours: WoW's own lit-torch model and icon (`ItemDisplayInfo` 12312, `Club_1H_Torch_A_01`,
//! `INV_Torch_Lit`), worn in [`Slot::Torch`](crate::equipment::Slot::Torch), sold by every vendor who
//! sells Refreshing Spring Water (the general goods traders). A worn torch burns for [`BURN_SECS`] and is
//! gone.

use crate::model::{Item, Vendor, World};

/// The item's entry: past every WoW entry, beside the game's other own rows.
pub const ITEM: u32 = 9_100_001;
/// WoW's lit torch (`ItemDisplayInfo` 12312).
pub const DISPLAY: u32 = 12_312;
/// Seconds a worn torch burns.
pub const BURN_SECS: f32 = 600.0;
/// Its price in copper (100 souls at the shops' rate).
pub const PRICE: u32 = 25;
/// Refreshing Spring Water: whoever sells it is a general goods trader.
const WATER: u32 = 159;

/// The torch's item row.
pub fn item() -> Item {
    Item {
        entry: ITEM,
        name: "Torch".into(),
        class: 15,
        subclass: 0,
        display: DISPLAY,
        quality: 1,
        flags: 0,
        buy_price: PRICE,
        sell_price: PRICE / 4,
        inventory_type: crate::equipment::inv::TORCH,
        allowable_class: -1,
        allowable_race: -1,
        item_level: 1,
        required_level: 0,
        max_count: 0,
        stackable: 20,
        delay: 0,
        damage: (0.0, 0.0, 0),
        block: 0,
        armor: 0,
        stats: Vec::new(),
    }
}

/// Puts the torch in `world`: its item row and a place on every general goods trader's shelf. Twice is
/// once.
pub fn add_to(world: &mut World) {
    if !world.items.iter().any(|i| i.entry == ITEM) {
        world.items.push(item());
    }
    let mut traders: Vec<u32> = world
        .vendors
        .iter()
        .filter(|v| v.item == WATER)
        .map(|v| v.npc)
        .collect();
    traders.sort_unstable();
    traders.dedup();
    for npc in traders {
        if !world.vendors.iter().any(|v| v.npc == npc && v.item == ITEM) {
            world.vendors.push(Vendor {
                npc,
                item: ITEM,
                max_count: 0,
                incr_time: 0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::{can_wear, slots_for, Slot, Wearer};

    #[test]
    fn the_torch_goes_on_the_belt_and_onto_the_water_sellers_shelves() {
        let t = item();
        assert_eq!(slots_for(t.inventory_type), &[Slot::Torch]);
        assert_eq!(can_wear(&t, Wearer::new(1, 1, 1)), Ok(()));
        let mut w = World::default();
        w.vendors.push(Vendor {
            npc: 7,
            item: WATER,
            max_count: 0,
            incr_time: 0,
        });
        w.vendors.push(Vendor {
            npc: 8,
            item: 25,
            max_count: 0,
            incr_time: 0,
        });
        add_to(&mut w);
        add_to(&mut w);
        assert_eq!(w.items.iter().filter(|i| i.entry == ITEM).count(), 1);
        let sellers: Vec<u32> = w
            .vendors
            .iter()
            .filter(|v| v.item == ITEM)
            .map(|v| v.npc)
            .collect();
        assert_eq!(sellers, vec![7]);
    }
}
