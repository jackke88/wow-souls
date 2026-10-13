//! The shops, the Abbey's strongbox and the warrior trainer: three kinds of NPC business that spend or keep
//! souls, one screen. The player pays and is paid in souls, the one currency ([`wow_world::shop`]: WoW's
//! copper prices at four souls to the copper).
//!
//! - A **vendor** (the `vendor` flag and rows in the database's stock table) has two tabs: Buy (its own stock,
//!   in the database's order) and Sell (the pack, a stack at a time).
//! - The **banker** (Brother Danil keeps the Abbey's strongbox) takes souls out of the risk of a death: a
//!   deposit pays a tenth to the Abbey ([`wow_world::shop::TITHE_PERCENT`]), a withdrawal is free; the list
//!   offers all, 100 and 1000.
//! - The **trainer** (Llane Beshere) teaches the warrior's abilities for souls, at their levels
//!   ([`wow_world::abilities`]).
//!
//! The card on the left shows what is highlighted: an item with its comparison against what is worn, or
//! an ability's rank, rage cost and words. Keys: Up / Down (D-pad) move, Left / Right (LB / RB, `[` `]`)
//! switch tab, Enter / A does it, Backspace / Esc / B leaves. A purchase or a lesson is saved with the
//! pack, then the souls change hands; walking away, being hurt or dying closes the screen.
//!
//! `WOWSOULS_SHOP_SCRIPT=<s>:<up|down|left|right|confirm|back>,...` is the hands of an unattended run.

use bevy::prelude::*;

use wow_world::abilities;
use wow_world::inventory::Progress;
use wow_world::model::Item;
use wow_world::shop::{self, Refusal, Step};

use crate::hud::souls::{Counter, Souls};
use crate::loot::{Data, Owned, Screen};
use crate::quests::dialogue::{Service, Services};
use crate::quests::QuestGame;
use crate::settle::Phase;

pub mod ui;

/// The NPCs that keep the Abbey's strongbox (WoW's valley has no banker: this one is ours).
pub const BANKERS: [u32; 1] = [152];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Buy,
    Sell,
    Bank,
    Train,
}

impl Tab {
    pub fn name(self) -> &'static str {
        match self {
            Tab::Buy => "Buy",
            Tab::Sell => "Sell",
            Tab::Bank => "Strongbox",
            Tab::Train => "Abilities",
        }
    }
}

/// A screen that is open.
#[derive(Debug, Clone)]
pub struct Open {
    /// The NPC's creature entry.
    pub npc: u32,
    pub name: String,
    pub subtitle: String,
    pub tabs: Vec<Tab>,
    /// Which of the tabs is up.
    pub at: usize,
    /// The highlighted row of each tab.
    pub cursor: Vec<usize>,
    /// The last result, and whether it was a refusal.
    pub message: String,
    pub bad: bool,
}

impl Open {
    pub fn new(npc: u32, name: &str, subtitle: &str, service: Service) -> Self {
        let tabs = match service {
            Service::Shop => vec![Tab::Buy, Tab::Sell],
            Service::Bank => vec![Tab::Bank],
            Service::Train => vec![Tab::Train],
        };
        Self {
            npc,
            name: name.to_string(),
            subtitle: subtitle.to_string(),
            cursor: vec![0; tabs.len()],
            tabs,
            at: 0,
            message: String::new(),
            bad: false,
        }
    }

    pub fn tab(&self) -> Tab {
        self.tabs[self.at]
    }

    pub fn row(&self) -> usize {
        self.cursor[self.at]
    }
}

/// What the player asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Buy(u32),
    Sell(u32),
    /// Learn the ability of this spell.
    Train(u32),
    Bank(Step),
}

/// What the card on the left shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Item(u32),
    Spell(u32),
}

#[derive(Resource, Default)]
pub struct Shop {
    pub open: Option<Open>,
    pub request: Option<Request>,
    /// The row under the cursor, for the card (kept by the screen's refresh).
    pub focus: Option<Focus>,
}

/// One row of a tab's list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    /// The item, or 0.
    pub item: u32,
    /// The ability's spell, or 0.
    pub spell: u32,
    pub name: String,
    pub quality: u32,
    /// What a sale takes of the stack (0 elsewhere).
    pub count: u32,
    /// Souls to pay (Buy, Train), to be paid for the stack (Sell), or the tithe of a deposit (Bank).
    pub price: u32,
    /// Why it cannot be done, in a few words; empty when it can.
    pub note: String,
    /// The strongbox's line.
    pub step: Option<Step>,
}

/// How many of an item an accepted quest still wants (they are not for sale).
pub fn needed_by(game: Option<&QuestGame>, entry: u32) -> u32 {
    let Some(g) = game else {
        return 0;
    };
    let accepted = g
        .log
        .active()
        .into_iter()
        .filter_map(|q| g.book.quest(q))
        .flat_map(|q| q.req_items.iter())
        .filter(|&&(item, _)| item == entry)
        .map(|&(_, n)| n)
        .max()
        .unwrap_or(0);
    // A quest not taken yet but takeable will want it too: it is not for sale either.
    accepted.max(g.log.pre_need(&g.book, &g.player, entry))
}

/// Everything the rows and the requests read.
pub struct Ctx<'a> {
    pub npc: u32,
    pub stock: &'a [u32],
    pub items: &'a [Item],
    pub progress: &'a Progress,
    pub pack_order: &'a [u32],
    pub souls: u32,
    pub banked: u32,
    pub level: u32,
    pub needed: &'a dyn Fn(u32) -> u32,
}

fn find(items: &[Item], entry: u32) -> Option<&Item> {
    items.iter().find(|i| i.entry == entry)
}

/// A refusal's few words for a row.
fn short(r: abilities::Refusal) -> String {
    match r {
        abilities::Refusal::Known => "Known".into(),
        abilities::Refusal::Level(n) => format!("Level {n}"),
        abilities::Refusal::NotEnoughSouls { .. } => "Not enough souls".into(),
        abilities::Refusal::NotYet => "Not yet".into(),
    }
}

/// The rows of a tab.
pub fn lines(tab: Tab, c: &Ctx) -> Vec<Line> {
    match tab {
        Tab::Buy => c
            .stock
            .iter()
            .filter_map(|&e| {
                let i = find(c.items, e)?;
                let price = shop::buy_cost(i)?;
                Some(Line {
                    item: e,
                    name: i.name.clone(),
                    quality: i.quality,
                    price,
                    note: if price > c.souls {
                        "Not enough souls".into()
                    } else {
                        String::new()
                    },
                    ..Line::default()
                })
            })
            .collect(),
        Tab::Sell => c
            .pack_order
            .iter()
            .filter_map(|&e| {
                let i = find(c.items, e)?;
                let carried = c.progress.inventory.count(e);
                let value = shop::sell_value(i)?;
                if carried == 0 {
                    return None;
                }
                let sellable = carried.saturating_sub((c.needed)(e));
                Some(Line {
                    item: e,
                    name: i.name.clone(),
                    quality: i.quality,
                    count: sellable,
                    price: value.saturating_mul(sellable),
                    note: if sellable == 0 {
                        "Needed for a quest".into()
                    } else {
                        String::new()
                    },
                    ..Line::default()
                })
            })
            .collect(),
        Tab::Bank => shop::steps(c.souls, c.banked)
            .into_iter()
            .map(|s| {
                let verb = if s.deposit { "Deposit" } else { "Withdraw" };
                let name = if s.all {
                    format!("{verb} all ({} souls)", s.amount)
                } else {
                    format!("{verb} {} souls", s.amount)
                };
                Line {
                    name,
                    quality: 1,
                    price: if s.deposit { shop::tithe(s.amount) } else { 0 },
                    step: Some(s),
                    ..Line::default()
                }
            })
            .collect(),
        Tab::Train => abilities::trainer_list(c.npc)
            .into_iter()
            .map(|a| Line {
                spell: a.spell,
                name: format!("{} (Rank {})", a.name, a.rank),
                quality: 1,
                price: a.souls(),
                note: abilities::can_learn(&c.progress.abilities, a, c.level, c.souls)
                    .err()
                    .map(short)
                    .unwrap_or_default(),
                ..Line::default()
            })
            .collect(),
    }
}

/// What a request did: the new pack, the new purses and the words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Done {
    pub progress: Progress,
    pub souls: u32,
    pub banked: u32,
    pub message: String,
}

/// Why a request was refused: the shop's or the trainer's reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Shop(Refusal),
    Train(abilities::Refusal),
}

impl Refused {
    pub fn message(self) -> String {
        match self {
            Refused::Shop(r) => r.message(),
            Refused::Train(r) => r.message(),
        }
    }
}

/// One request on a copy of the pack and the purses.
pub fn attempt(request: Request, c: &Ctx) -> Result<Done, Refused> {
    let mut progress = c.progress.clone();
    let (mut souls, mut banked) = (c.souls, c.banked);
    let message = match request {
        Request::Buy(e) => {
            let item = find(c.items, e).ok_or(Refused::Shop(Refusal::NotForSale))?;
            let cost = shop::buy(&mut progress, &mut souls, item, 1).map_err(Refused::Shop)?;
            format!("Bought {} for {cost} souls.", item.name)
        }
        Request::Sell(e) => {
            let item = find(c.items, e).ok_or(Refused::Shop(Refusal::NoValue))?;
            let count = c.progress.inventory.count(e).saturating_sub((c.needed)(e));
            let pay = shop::sell(&mut progress, &mut souls, item, count, c.needed)
                .map_err(Refused::Shop)?;
            if count > 1 {
                format!("Sold {} x{count} for {pay} souls.", item.name)
            } else {
                format!("Sold {} for {pay} souls.", item.name)
            }
        }
        Request::Train(spell) => {
            let ability =
                abilities::find(spell).ok_or(Refused::Train(abilities::Refusal::NotYet))?;
            let cost = abilities::learn(&mut progress.abilities, &mut souls, ability, c.level)
                .map_err(Refused::Train)?;
            format!("You have learned {} for {cost} souls.", ability.name)
        }
        Request::Bank(step) if step.deposit => {
            let (keep, fee) =
                shop::deposit(&mut souls, &mut banked, step.amount).map_err(Refused::Shop)?;
            format!("{keep} souls are in the strongbox; {fee} went to the Abbey.")
        }
        Request::Bank(step) => {
            let got =
                shop::withdraw(&mut souls, &mut banked, step.amount).map_err(Refused::Shop)?;
            format!("You took {got} souls out of the strongbox.")
        }
    };
    Ok(Done {
        progress,
        souls,
        banked,
        message,
    })
}

/// What the keys, the buttons or the script asked of the screen this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Acts {
    up: bool,
    down: bool,
    prev: bool,
    next: bool,
    confirm: bool,
    back: bool,
}

fn parse_script(text: &str) -> Vec<(f32, Acts)> {
    text.split(',')
        .filter_map(|step| {
            let (at, act) = step.trim().split_once(':')?;
            let mut a = Acts::default();
            match act.trim() {
                "up" => a.up = true,
                "down" => a.down = true,
                "left" => a.prev = true,
                "right" => a.next = true,
                "confirm" => a.confirm = true,
                "back" => a.back = true,
                _ => return None,
            }
            Some((at.trim().parse().ok()?, a))
        })
        .collect()
}

fn live_acts(keys: &ButtonInput<KeyCode>, pads: &Query<&Gamepad>) -> Acts {
    let any = |codes: &[KeyCode]| codes.iter().any(|&k| keys.just_pressed(k));
    let pad = |b: GamepadButton| pads.iter().any(|p| p.just_pressed(b));
    Acts {
        up: any(&[KeyCode::ArrowUp]) || pad(GamepadButton::DPadUp),
        down: any(&[KeyCode::ArrowDown]) || pad(GamepadButton::DPadDown),
        prev: any(&[KeyCode::ArrowLeft, KeyCode::BracketLeft]) || pad(GamepadButton::LeftTrigger),
        next: any(&[KeyCode::ArrowRight, KeyCode::BracketRight])
            || pad(GamepadButton::RightTrigger),
        confirm: any(&[KeyCode::Enter, KeyCode::NumpadEnter]) || pad(GamepadButton::South),
        back: any(&[KeyCode::Backspace, KeyCode::Escape]) || pad(GamepadButton::East),
    }
}

/// The rows of the open screen's current tab.
pub fn current_lines(
    open: &Open,
    data: &Data,
    owned: &Owned,
    souls: u32,
    banked: u32,
    game: Option<&QuestGame>,
) -> Vec<Line> {
    let stock = shop::stock(&data.0.vendors, open.npc);
    let order = crate::loot::ui::ordered(&owned.progress.inventory);
    let needed = |e: u32| needed_by(game, e);
    lines(
        open.tab(),
        &Ctx {
            npc: open.npc,
            stock: &stock,
            items: &data.0.items,
            progress: &owned.progress,
            pack_order: &order,
            souls,
            banked,
            level: game.map_or(1, |g| g.player.level),
            needed: &needed,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn input(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    real: Res<Time<Real>>,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    mut shop: ResMut<Shop>,
    mut screen: ResMut<Screen>,
    data: Res<Data>,
    owned: Res<Owned>,
    souls: Res<Souls>,
    bank: Res<crate::bloodstain::Banked>,
    game: Option<Res<QuestGame>>,
    mut script: Local<Option<Vec<(f32, Acts)>>>,
    mut next_step: Local<usize>,
) {
    let Some(open) = shop.open.as_mut() else {
        return;
    };
    let steps = script.get_or_insert_with(|| {
        std::env::var("WOWSOULS_SHOP_SCRIPT")
            .map(|t| parse_script(&t))
            .unwrap_or_default()
    });
    let acts = if unattended.is_some() {
        match steps.get(*next_step) {
            Some(&(at, acts)) if real.elapsed_secs() >= at => {
                *next_step += 1;
                acts
            }
            _ => return,
        }
    } else {
        live_acts(&keys, &pads)
    };
    if acts == Acts::default() {
        return;
    }
    // A press the screen took is not also a jump or a roll, now or as it closes.
    screen.swallow_until = real.elapsed_secs() + 0.25;
    if acts.back {
        shop.open = None;
        return;
    }
    if acts.prev && open.at > 0 {
        open.at -= 1;
        open.message.clear();
    }
    if acts.next && open.at + 1 < open.tabs.len() {
        open.at += 1;
        open.message.clear();
    }
    let rows = current_lines(open, &data, &owned, souls.0, bank.0, game.as_deref());
    let at = open.at;
    if acts.down {
        open.cursor[at] = (open.cursor[at] + 1).min(rows.len().saturating_sub(1));
    }
    if acts.up {
        open.cursor[at] = open.cursor[at].saturating_sub(1);
    }
    open.cursor[at] = open.cursor[at].min(rows.len().saturating_sub(1));
    if acts.confirm {
        let tab = open.tab();
        let request = rows.get(open.cursor[at]).and_then(|l| match tab {
            Tab::Buy => Some(Request::Buy(l.item)),
            Tab::Sell => Some(Request::Sell(l.item)),
            Tab::Train => Some(Request::Train(l.spell)),
            Tab::Bank => l.step.map(Request::Bank),
        });
        shop.request = request;
    }
}

/// Carries out the pending request: the pack is saved first, then the souls change hands.
#[allow(clippy::too_many_arguments)]
fn act(
    mut shop: ResMut<Shop>,
    mut owned: ResMut<Owned>,
    data: Res<Data>,
    mut souls: ResMut<Souls>,
    mut bank: ResMut<crate::bloodstain::Banked>,
    mut counter: ResMut<Counter>,
    game: Option<Res<QuestGame>>,
    mut audio: Option<ResMut<crate::audio::Audio>>,
    settings: Res<crate::settings::Settings>,
) {
    let Some(request) = shop.request.take() else {
        return;
    };
    let needed = |e: u32| needed_by(game.as_deref(), e);
    let npc = shop.open.as_ref().map_or(0, |o| o.npc);
    let stock = shop::stock(&data.0.vendors, npc);
    let order = crate::loot::ui::ordered(&owned.progress.inventory);
    let outcome = attempt(
        request,
        &Ctx {
            npc,
            stock: &stock,
            items: &data.0.items,
            progress: &owned.progress,
            pack_order: &order,
            souls: souls.0,
            banked: bank.0,
            level: game.as_deref().map_or(1, |g| g.player.level),
            needed: &needed,
        },
    );
    let Some(open) = shop.open.as_mut() else {
        return;
    };
    match outcome {
        Ok(mut done) => {
            // A purchase is handed over like a drop: gear that fits an empty slot goes on, and
            // the line says so (the same rule as a corpse, a chest or a reward).
            let mut put_on = Vec::new();
            if let Request::Buy(entry) = request {
                if settings.game.auto_equip {
                    put_on = crate::equip::put_on_handed(
                        &mut done.progress,
                        &data.0.items,
                        &[(entry, 1)],
                        game.as_deref(),
                    );
                }
            }
            if !put_on.is_empty() {
                done.message = format!("{} {}.", done.message, put_on.join(", "));
            }
            // The pack is saved first (a lesson or a purchase), the souls change hands after.
            if done.progress != owned.progress {
                if let Err(e) = owned.publish(done.progress) {
                    open.message = e;
                    open.bad = true;
                    return;
                }
            }
            // A sale's souls show as a gain over the counter; a withdrawal only moves your own.
            if matches!(request, Request::Sell(_)) && done.souls > souls.0 {
                counter.add(done.souls - souls.0);
            }
            info!(
                "shop: {} (souls {} -> {}, bank {} -> {})",
                done.message, souls.0, done.souls, bank.0, done.banked
            );
            souls.0 = done.souls;
            bank.0 = done.banked;
            open.message = done.message;
            open.bad = false;
            if let Some(audio) = audio.as_deref_mut() {
                audio.play_named("LOOTWINDOWCOINSOUND", 0.8, "shop coin");
            }
        }
        Err(refusal) => {
            info!("shop: refused {request:?}: {}", refusal.message());
            open.message = refusal.message();
            open.bad = true;
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Shop>()
        .add_systems(
            Update,
            (input, act).chain().run_if(in_state(Phase::Playing)),
        )
        .add_plugins(ui::plugin);
}

/// What an NPC offers besides quests: wares (the vendor flag and a stock row), the strongbox (a banker),
/// lessons (the trainer flag and a list).
pub fn services(data: Option<&Data>, flags: u32, entry: u32) -> Services {
    Services {
        vendor: data.is_some_and(|d| flags & 4 != 0 && d.0.vendors.iter().any(|v| v.npc == entry)),
        banker: BANKERS.contains(&entry),
        trainer: flags & 16 != 0 && !abilities::trainer_list(entry).is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::model::Item;

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

    fn ctx<'a>(
        stock: &'a [u32],
        items: &'a [Item],
        progress: &'a Progress,
        order: &'a [u32],
        souls: u32,
        needed: &'a dyn Fn(u32) -> u32,
    ) -> Ctx<'a> {
        Ctx {
            npc: 0,
            stock,
            items,
            progress,
            pack_order: order,
            souls,
            banked: 0,
            level: 1,
            needed,
        }
    }

    #[test]
    fn the_buy_tab_lists_the_stock_in_order_with_prices_and_what_cannot_be_afforded() {
        let items = vec![item(1, 50, 10), item(2, 100, 20), item(3, 0, 5)];
        let p = Progress::default();
        let none = |_| 0;
        let rows = lines(Tab::Buy, &ctx(&[2, 1, 3, 99], &items, &p, &[], 300, &none));
        // 3 has no price and 99 no row: both are left out.
        assert_eq!(rows.iter().map(|l| l.item).collect::<Vec<_>>(), vec![2, 1]);
        assert_eq!((rows[0].price, rows[1].price), (400, 200));
        assert_eq!(rows[0].note, "Not enough souls");
        assert_eq!(rows[1].note, "");
    }

    #[test]
    fn the_sell_tab_lists_the_sellable_stacks_and_marks_what_a_quest_needs() {
        let items = vec![item(1, 50, 10), item(2, 0, 3), item(3, 0, 0)];
        let mut p = Progress::default();
        p.inventory.add(&[(1, 1), (2, 23), (3, 2)]).unwrap();
        let needed = |e: u32| if e == 2 { 8 } else { 0 };
        let rows = lines(Tab::Sell, &ctx(&[], &items, &p, &[1, 2, 3, 77], 0, &needed));
        // 3 is worth nothing, 77 is not carried.
        assert_eq!(rows.iter().map(|l| l.item).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!((rows[0].count, rows[0].price), (1, 40));
        assert_eq!((rows[1].count, rows[1].price), (15, 180));
        let all_needed = |_: u32| 99;
        let rows = lines(Tab::Sell, &ctx(&[], &items, &p, &[1], 0, &all_needed));
        assert_eq!((rows[0].count, rows[0].price), (0, 0));
        assert_eq!(rows[0].note, "Needed for a quest");
    }

    #[test]
    fn a_purchase_and_a_sale_give_the_new_pack_the_new_souls_and_a_message() {
        let items = vec![item(1, 60, 12)];
        let p = Progress::default();
        let none = |_| 0;
        let c = ctx(&[], &items, &p, &[], 300, &none);
        let bought = attempt(Request::Buy(1), &c).unwrap();
        assert_eq!((bought.souls, bought.progress.inventory.count(1)), (60, 1));
        assert_eq!(bought.message, "Bought Item 1 for 240 souls.");
        let c2 = ctx(&[], &items, &bought.progress, &[1], 60, &none);
        let sold = attempt(Request::Sell(1), &c2).unwrap();
        assert_eq!((sold.souls, sold.progress.inventory.count(1)), (108, 0));
        assert_eq!(sold.message, "Sold Item 1 for 48 souls.");
        assert_eq!(p.inventory.count(1), 0, "the old pack is untouched");
        let poor = ctx(&[], &items, &p, &[], 10, &none);
        assert_eq!(
            attempt(Request::Buy(1), &poor).unwrap_err(),
            Refused::Shop(Refusal::NotEnoughSouls {
                need: 240,
                have: 10
            })
        );
        assert_eq!(
            attempt(Request::Buy(9), &poor).unwrap_err(),
            Refused::Shop(Refusal::NotForSale)
        );
    }

    #[test]
    fn the_trainer_lists_charge_shout_and_clap_and_teaches_one_for_souls() {
        let items: Vec<Item> = Vec::new();
        let p = Progress::default();
        let none = |_| 0;
        let mut c = ctx(&[], &items, &p, &[], 100, &none);
        c.npc = abilities::WARRIOR_TRAINER;
        let rows = lines(Tab::Train, &c);
        assert_eq!(
            rows.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
            vec![
                "Charge (Rank 1)",
                "Battle Shout (Rank 1)",
                "Thunder Clap (Rank 1)"
            ],
            "the user's set, Charge first; no Heroic Strike, no Rend"
        );
        assert_eq!(rows[1].price, 40);
        assert_eq!(rows[1].note, "", "the shout is affordable at level 1");
        assert_ne!(rows[0].note, "", "Charge costs 400 and he carries 100");
        assert_ne!(rows[2].note, "", "Thunder Clap waits for level 4");
        let done = attempt(Request::Train(6673), &c).unwrap();
        assert_eq!(done.souls, 60);
        assert!(done.progress.abilities.contains(&6673));
        assert_eq!(done.message, "You have learned Battle Shout for 40 souls.");
        // Charge, learned first, is learnable at level 1 with the souls.
        let rich = ctx(&[], &items, &p, &[], 400, &none);
        let mut rich = rich;
        rich.npc = abilities::WARRIOR_TRAINER;
        let charge = attempt(Request::Train(100), &rich).unwrap();
        assert_eq!(charge.souls, 0);
        assert_eq!(charge.message, "You have learned Charge for 400 souls.");
        // Known once learned.
        let learned = done.progress;
        let mut c2 = ctx(&[], &items, &learned, &[], 60, &none);
        c2.npc = abilities::WARRIOR_TRAINER;
        assert_eq!(lines(Tab::Train, &c2)[1].note, "Known");
        assert_eq!(
            attempt(Request::Train(6673), &c2).unwrap_err(),
            Refused::Train(abilities::Refusal::Known)
        );
    }

    #[test]
    fn the_strongbox_banks_with_a_tithe_and_gives_back_for_free() {
        let items: Vec<Item> = Vec::new();
        let p = Progress::default();
        let none = |_| 0;
        let mut c = ctx(&[], &items, &p, &[], 1000, &none);
        c.banked = 50;
        let rows = lines(Tab::Bank, &c);
        let names: Vec<_> = rows.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Deposit all (1000 souls)",
                "Deposit 100 souls",
                "Withdraw all (50 souls)"
            ]
        );
        assert_eq!(rows[0].price, 100, "the tithe of all");
        let deposit = attempt(Request::Bank(rows[1].step.unwrap()), &c).unwrap();
        assert_eq!((deposit.souls, deposit.banked), (900, 140));
        assert_eq!(deposit.progress, p, "the pack is not touched");
        let out = attempt(Request::Bank(rows[2].step.unwrap()), &c).unwrap();
        assert_eq!((out.souls, out.banked), (1050, 0));
    }

    #[test]
    fn the_script_names_presses_and_drops_the_rest() {
        let steps = parse_script("5:down, 6:confirm,7:right,x:up,8:bogus,9:back");
        assert_eq!(steps.len(), 4);
        assert!(steps[0].1.down && steps[1].1.confirm && steps[2].1.next && steps[3].1.back);
    }
}
