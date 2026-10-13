//! The player's equipment: WoW 1.12's paper doll. What is worn lives in the items save
//! ([`wow_world::inventory::Progress::gear`], the rules in [`wow_world::equipment`]); this module
//! turns it into what the body shows and into the screen that changes it.
//!
//! - [`appearance`] reads the worn items' `ItemDisplayInfo` ids: armor pieces paint the body atlas and
//!   pick geosets, the head and shoulders hang their models from the body's attachment points, the back
//!   piece is the cloak (cloth, [`crate::cloak`]), the main hand is the held weapon (a bare fist when
//!   empty), the off hand a shield on the forearm or a weapon in the left hand. The avatar carries the
//!   [`Appearance`] it is dressed in; a different one is applied in place ([`apply`]).
//! - [`Request`]s (from the item list's equip button or the equipment page) go through [`act`], which
//!   applies the rules to a copy of the save and publishes it, or says why not in WoW's words.
//! - The equipment page ([`ui`]) is the 1.12 character pane: its slots in its order, a choice of what
//!   fits a slot, and the item's tooltip.
//!
//! Debug switches: `WOWSOULS_KIT=0` skips the starting kit; `WOWSOULS_EQUIP=<item>[,<item>...]` adds the
//! items to the pack and puts them on once (`WOWSOULS_EQUIP_AT=<s>` waits that many real seconds, to see
//! a built body dressed again; `WOWSOULS_UNEQUIP=<slot key>[,...]` takes slots off first; `WOWSOULS_GIVE=<item>[,...]` only fills the pack); `WOWSOULS_WEAR=<slot>:<display>[,...]` overrides what the
//! body shows (slot keys of the save, `cloak` for the back) without touching the save.

use bevy::prelude::*;

use wow_world::equipment::{self, Gear, Refusal, Slot, Wearer};
use wow_world::model::{Item, ItemCount};

use crate::actor::held::{HeldBuilt, HeldItem, LeftGrip, OffHandBuilt, OffHandItem, Unarmed};
use crate::actor::{Catalogs, Dress};
use crate::avatar::Avatar;
use crate::character::Chosen;
use crate::loot::{Acquired, Data, Owned, Screen};
use crate::quests::{QuestGame, Toasts};
use crate::settings::Settings;
use crate::settle::Phase;

pub mod ui;

/// Which page of the inventory screen is up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Items,
    Gear,
}

// ---------------------------------------------------------------------------------------------
// What the body shows
// ---------------------------------------------------------------------------------------------

/// What a body shows of its gear: the armor and cloak ([`Dress`]), the main hand's weapon and the off
/// hand's shield or weapon, each an `ItemDisplayInfo` id (0 or `None`: nothing).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    pub dress: Dress,
    pub main: Option<u32>,
    pub left_main: bool,
    /// The off hand's display and whether it is a shield (on the forearm) rather than a weapon.
    pub off: Option<(u32, bool)>,
}

/// The index of a slot in [`Dress::worn`]; the back (the cloak) and the slots nobody sees have none.
pub fn dress_index(slot: Slot) -> Option<usize> {
    Some(match slot {
        Slot::Head => 0,
        Slot::Shoulders => 1,
        Slot::Shirt => 2,
        Slot::Chest => 3,
        Slot::Waist => 4,
        Slot::Legs => 5,
        Slot::Feet => 6,
        Slot::Wrists => 7,
        Slot::Hands => 8,
        Slot::Tabard => 9,
        _ => return None,
    })
}

/// The row of an item entry.
pub fn find(items: &[Item], entry: u32) -> Option<&Item> {
    items.iter().find(|i| i.entry == entry)
}

/// What `gear` looks like.
pub fn appearance(gear: &Gear, items: &[Item]) -> Appearance {
    let display = |slot: Slot| {
        gear.get(slot)
            .and_then(|e| find(items, e))
            .map_or(0, |i| i.display)
    };
    let mut a = Appearance::default();
    for slot in Slot::ALL {
        if let Some(i) = dress_index(slot) {
            a.dress.worn[i] = display(slot);
        }
    }
    a.dress.cloak = display(Slot::Back);
    a.main = Some(display(Slot::MainHand)).filter(|&d| d != 0);
    a.off = gear
        .get(Slot::OffHand)
        .and_then(|e| find(items, e))
        .filter(|i| i.display != 0)
        .map(|i| (i.display, i.inventory_type == equipment::inv::SHIELD));
    a
}

/// `WOWSOULS_WEAR`: displays the body wears whatever the save says (a debug switch for looking at
/// gear the valley does not hand out, such as helms).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct WearOverride(pub Vec<(String, u32)>);

impl WearOverride {
    pub fn parse(text: &str) -> Self {
        Self(
            text.split(',')
                .filter_map(|pair| {
                    let (slot, display) = pair.split_once(':')?;
                    let display = display.trim().parse().ok()?;
                    Some((slot.trim().to_ascii_lowercase(), display))
                })
                .collect(),
        )
    }

    fn from_env() -> Self {
        std::env::var("WOWSOULS_WEAR")
            .map(|v| Self::parse(&v))
            .unwrap_or_default()
    }

    /// The appearance with the overrides laid over it.
    pub fn over(&self, mut a: Appearance) -> Appearance {
        for (key, display) in &self.0 {
            if key == "cloak" {
                a.dress.cloak = *display;
            } else if key == "mainhand" {
                a.main = Some(*display).filter(|&d| d != 0);
            } else if key == "offhand" {
                a.off = Some((*display, false)).filter(|&(d, _)| d != 0);
            } else if key == "shield" {
                a.off = Some((*display, true)).filter(|&(d, _)| d != 0);
            } else if let Some(i) = Slot::from_key(key).and_then(dress_index) {
                a.dress.worn[i] = *display;
            }
        }
        a
    }
}

/// The appearance the avatar should have now: the save's gear with the debug overrides on it, and
/// a weapon or shield whose model the client does not have shown as nothing.
pub fn wanted(
    gear: &Gear,
    items: &[Item],
    wear: &WearOverride,
    catalogs: Option<&Catalogs>,
) -> Appearance {
    let mut a = wear.over(appearance(gear, items));
    if let Some(c) = catalogs {
        let has_model = |d: u32| c.items.get(d).is_some_and(|r| r.model[0].is_some());
        a.main = a.main.filter(|&d| has_model(d));
        a.off = a.off.filter(|&(d, _)| has_model(d));
    }
    a
}

/// Puts a freshly spawned avatar's first look on it (before its model is built).
pub fn dress_new(commands: &mut Commands, body: Entity, a: &Appearance) {
    let mut e = commands.entity(body);
    e.insert((a.clone(), a.dress.clone()));
    match a.main {
        Some(d) => {
            let mut h = HeldItem::right_hand(d);
            if a.left_main {
                h.attach = 2;
            }
            e.insert(h)
        }
        None => e.insert(Unarmed),
    };
    if let Some((d, shield)) = a.off {
        e.insert(OffHandItem::new(d, shield));
    }
}

/// Changes what a built avatar shows from `have` to `want`: only the parts that differ are redone.
fn change_look(
    commands: &mut Commands,
    body: Entity,
    have: &Appearance,
    want: &Appearance,
    held: Option<&HeldBuilt>,
    off: Option<&OffHandBuilt>,
) {
    if have.dress != want.dress {
        commands.entity(body).insert(want.dress.clone());
    }
    if have.main != want.main || have.left_main != want.left_main {
        if let Some(h) = held {
            commands.entity(h.root).try_despawn();
        }
        let mut e = commands.entity(body);
        e.remove::<(HeldBuilt, HeldItem, Unarmed, crate::actor::held::HandGrip)>();
        match want.main {
            Some(d) => {
                let mut h = HeldItem::right_hand(d);
                if want.left_main {
                    h.attach = 2;
                }
                e.insert(h)
            }
            None => e.insert(Unarmed),
        };
    }
    if have.off != want.off {
        if let Some(o) = off {
            commands.entity(o.root).try_despawn();
        }
        let mut e = commands.entity(body);
        e.remove::<(OffHandBuilt, OffHandItem, LeftGrip)>();
        if let Some((d, shield)) = want.off {
            e.insert(OffHandItem::new(d, shield));
        }
    }
    commands.entity(body).insert(want.clone());
}

/// Keeps the avatar dressed as its gear says.
#[allow(clippy::too_many_arguments)]
fn apply(
    mut commands: Commands,
    owned: Res<Owned>,
    data: Res<Data>,
    wear: Res<WearOverride>,
    moves: Option<Res<crate::dsclip::DsMoves>>,
    catalogs: Option<Res<Catalogs>>,
    fresh: Query<(), (With<Avatar>, Added<Appearance>)>,
    avatar: Query<
        (
            Entity,
            &Appearance,
            Option<&HeldBuilt>,
            Option<&OffHandBuilt>,
        ),
        With<Avatar>,
    >,
) {
    if !owned.is_changed()
        && !wear.is_changed()
        && !moves.as_ref().is_some_and(|m| m.is_changed())
        && fresh.is_empty()
    {
        return;
    }
    let mut want = wanted(
        &owned.progress.gear,
        &data.0.items,
        &wear,
        catalogs.as_deref(),
    );
    if let Some(m) = moves.as_ref() {
        if m.projectile.is_some() {
            want.main = owned
                .progress
                .gear
                .get(Slot::Ranged)
                .and_then(|id| find(&data.0.items, id))
                .map(|i| i.display);
            want = wear.over(want);
            want.left_main = m.family == "bow";
        }
        if m.two_handed || m.projectile.is_some() {
            want.off = None;
        }
    }
    for (body, have, held, off) in &avatar {
        if *have != want {
            info!("equip: dressing the avatar again ({have:?} -> {want:?})");
            change_look(&mut commands, body, have, &want, held, off);
        }
        if moves
            .as_ref()
            .is_some_and(|m| m.two_handed || m.family == "bow")
        {
            commands.entity(body).insert(LeftGrip);
        } else if want.off.is_none() {
            commands.entity(body).remove::<LeftGrip>();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------------------------

/// What the player asked of the paper doll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Put a carried item on, in a slot or where a quick equip would.
    Equip { item: u32, slot: Option<Slot> },
    /// Take what is worn in a slot off.
    Unequip(Slot),
}

/// The equipment page's state.
#[derive(Resource, Debug, Default)]
pub struct GearScreen {
    /// The slot the cursor is on.
    pub cursor: Slot,
    /// The choice list of the cursor's slot, while it is open.
    pub choosing: Option<Choosing>,
    /// The last result, in WoW's words.
    pub message: String,
    /// A request waiting for [`act`].
    pub request: Option<Request>,
}

/// What can go in a slot: the options of the choice list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choosing {
    pub options: Vec<Option<u32>>,
    pub selected: usize,
}

/// Which way the cursor moves on the pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// Where the cursor goes from `slot`: the two columns, then the weapon row under them.
pub fn neighbor(slot: Slot, dir: Dir) -> Slot {
    let left = equipment::LEFT_COLUMN.iter().position(|&s| s == slot);
    let right = equipment::RIGHT_COLUMN.iter().position(|&s| s == slot);
    let row = equipment::WEAPON_ROW.iter().position(|&s| s == slot);
    let last = equipment::LEFT_COLUMN.len() - 1;
    match (left, right, row, dir) {
        (Some(i), _, _, Dir::Up) => equipment::LEFT_COLUMN[i.saturating_sub(1)],
        (Some(i), _, _, Dir::Down) if i < last => equipment::LEFT_COLUMN[i + 1],
        (Some(_), _, _, Dir::Down) => equipment::WEAPON_ROW[0],
        (Some(i), _, _, Dir::Right) => equipment::RIGHT_COLUMN[i],
        (_, Some(i), _, Dir::Up) => equipment::RIGHT_COLUMN[i.saturating_sub(1)],
        (_, Some(i), _, Dir::Down) if i < last => equipment::RIGHT_COLUMN[i + 1],
        (_, Some(_), _, Dir::Down) => equipment::WEAPON_ROW[2],
        (_, Some(i), _, Dir::Left) => equipment::LEFT_COLUMN[i],
        (_, _, Some(j), Dir::Up) => {
            if j < 2 {
                equipment::LEFT_COLUMN[last]
            } else {
                equipment::RIGHT_COLUMN[last]
            }
        }
        (_, _, Some(j), Dir::Left) => equipment::WEAPON_ROW[j.saturating_sub(1)],
        (_, _, Some(j), Dir::Right) => {
            equipment::WEAPON_ROW[(j + 1).min(equipment::WEAPON_ROW.len() - 1)]
        }
        _ => slot,
    }
}

/// The carried items that go in `slot`, each with whether the wearer may use it, in the order of the
/// pack. A refused item stays listed so the player sees why (a plate vest needs level 40).
pub fn candidates(
    owned: &wow_world::inventory::Progress,
    items: &[Item],
    slot: Slot,
    wearer: Wearer,
    order: &[u32],
) -> Vec<(u32, Result<(), Refusal>)> {
    order
        .iter()
        .copied()
        .filter(|&e| owned.inventory.count(e) > 0)
        .filter_map(|e| find(items, e))
        .filter(|i| equipment::slots_for(i.inventory_type).contains(&slot))
        .map(|i| {
            let verdict = owned.gear.plan(items, i, slot, wearer).map(|_| ());
            (i.entry, verdict)
        })
        .collect()
}

/// The result of one request on a copy of the save: the new save and the message to show.
pub fn attempt(
    progress: &wow_world::inventory::Progress,
    items: &[Item],
    request: Request,
    wearer: Wearer,
) -> Result<(wow_world::inventory::Progress, String), Refusal> {
    let mut next = progress.clone();
    let name = |e: u32| find(items, e).map_or_else(|| format!("Item {e}"), |i| i.name.clone());
    let message = match request {
        Request::Equip { item, slot } => {
            next.equip(items, item, slot, wearer)?;
            format!("Equipped: {}", name(item))
        }
        Request::Unequip(slot) => {
            let item = next.unequip(slot)?;
            format!("Taken off: {}", name(item))
        }
    };
    Ok((next, message))
}

/// The one rule for what the character is handed (a drop, a chest, a quest reward, a purchase):
/// each piece of `got`, already carried in `next`, that fits an EMPTY slot goes on at once. Nothing
/// worn moves, what an accepted quest counts stays in the pack, what the wearer cannot use stays
/// too. One line ("Equipped: ...") per piece put on.
pub(crate) fn put_on_handed(
    next: &mut wow_world::inventory::Progress,
    items: &[Item],
    got: &[ItemCount],
    game: Option<&QuestGame>,
) -> Vec<String> {
    let put = next.put_on_new(items, got, wearer(game), &|entry| {
        crate::shop::needed_by(game, entry) > 0
    });
    put.into_iter()
        .map(|(entry, _)| {
            let name =
                find(items, entry).map_or_else(|| format!("Item {entry}"), |i| i.name.clone());
            format!("Equipped: {name}")
        })
        .collect()
}

/// Puts on what a pickup, a chest or a quest handed over (the loot module's `Acquired` messages):
/// the same rule a purchase follows ([`put_on_handed`]), unless the Game menu switched it off.
#[allow(clippy::too_many_arguments)]
fn auto_equip(
    mut acquired: MessageReader<Acquired>,
    mut owned: ResMut<Owned>,
    data: Res<Data>,
    game: Option<Res<QuestGame>>,
    settings: Res<Settings>,
    toasts: Option<ResMut<Toasts>>,
    real: Res<Time<Real>>,
) {
    let got: Vec<ItemCount> = acquired
        .read()
        .flat_map(|e| e.items.iter().copied())
        .collect();
    if got.is_empty() || !settings.game.auto_equip {
        return;
    }
    let mut next = owned.progress.clone();
    let lines = put_on_handed(&mut next, &data.0.items, &got, game.as_deref());
    if lines.is_empty() {
        return;
    }
    match owned.publish(next) {
        Ok(()) => {
            for line in &lines {
                info!("equip: {line} (handed over, its slot was empty)");
            }
            if let Some(mut toasts) = toasts {
                for line in lines {
                    toasts.push(real.elapsed_secs(), line);
                }
            }
        }
        Err(e) => warn!("equip: what was handed over was not put on: {e}"),
    }
}

/// Who the rules see: the character at its quest level.
pub(crate) fn wearer(game: Option<&QuestGame>) -> Wearer {
    game.map_or(
        Wearer::new(equipment::RACE_HUMAN, equipment::CLASS_WARRIOR, 1),
        |g| Wearer::new(g.player.race as u8, g.player.class as u8, g.player.level),
    )
}

/// Applies the pending requests to the save.
#[allow(clippy::too_many_arguments)]
fn act(
    mut owned: ResMut<Owned>,
    data: Res<Data>,
    game: Option<Res<QuestGame>>,
    mut ui: ResMut<GearScreen>,
    mut screen: ResMut<Screen>,
    mut audio: Option<ResMut<crate::audio::Audio>>,
    real: Res<Time<Real>>,
    mut scripted_press: Local<bool>,
) {
    let mut request = ui.request.take();
    // `WOWSOULS_ITEMS_EQUIP_AT=<s>`: the equip press on the pack's selected item, for unattended runs.
    if !*scripted_press
        && std::env::var("WOWSOULS_ITEMS_EQUIP_AT")
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .is_some_and(|at| real.elapsed_secs() >= at && screen.open)
    {
        *scripted_press = true;
        screen.equip_requested = true;
    }
    if screen.equip_requested {
        screen.equip_requested = false;
        let ids = crate::loot::ui::ordered(&owned.progress.inventory);
        if let Some(&item) = ids.get(screen.selected) {
            request = Some(Request::Equip { item, slot: None });
        }
    }
    let Some(request) = request else {
        return;
    };
    let who = wearer(game.as_deref());
    match attempt(&owned.progress, &data.0.items, request, who) {
        Ok((next, message)) => match owned.publish(next) {
            Ok(()) => {
                info!("equip: {message}");
                ui.message = message;
                ui.choosing = None;
                if let Some(audio) = audio.as_deref_mut() {
                    audio.play_path("Sound\\interface\\UiLootPickUpItem.wav", 0.6, "item equip");
                }
                let count = owned.progress.inventory.items.len();
                screen.selected = screen.selected.min(count.saturating_sub(1));
            }
            Err(e) => ui.message = e,
        },
        Err(refusal) => {
            info!("equip: refused {request:?}: {}", refusal.message());
            ui.message = refusal.message();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The starting kit and the debug switches
// ---------------------------------------------------------------------------------------------

/// Dresses a new character (or an old save, once) in its starting kit.
fn grant_kit(mut owned: ResMut<Owned>, chosen: Option<Res<Chosen>>) {
    if std::env::var("WOWSOULS_KIT").is_ok_and(|v| v.trim() == "0") {
        return;
    }
    let (race, class) = chosen
        .as_deref()
        .and_then(|c| c.0.as_ref())
        .map_or((equipment::RACE_HUMAN, equipment::CLASS_WARRIOR), |c| {
            (c.race.id(), c.class.id())
        });
    let mut next = owned.progress.clone();
    if !next.grant_start_kit(race, class) {
        return;
    }
    match owned.publish(next) {
        Ok(()) => info!(
            "equip: starting kit on: {:?}",
            owned.progress.gear.iter().collect::<Vec<_>>()
        ),
        Err(e) => warn!("equip: the starting kit was not saved: {e}"),
    }
}

/// `WOWSOULS_EQUIP=<item>[,<item>...]`: adds each to the pack and puts it on, once the world is up.
fn scripted_equip(
    mut owned: ResMut<Owned>,
    data: Res<Data>,
    game: Option<Res<QuestGame>>,
    mut ui: ResMut<GearScreen>,
    real: Res<Time<Real>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let at: f32 = std::env::var("WOWSOULS_EQUIP_AT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    if real.elapsed_secs() < at {
        return;
    }
    *done = true;
    // `WOWSOULS_UNEQUIP=<slot key>[,...]` takes slots off first (`mainhand` for the bare fist).
    if let Ok(list) = std::env::var("WOWSOULS_UNEQUIP") {
        for slot in list.split(',').filter_map(|k| Slot::from_key(k.trim())) {
            let mut next = owned.progress.clone();
            if next.unequip(slot).is_ok() {
                info!("equip: scripted: {} taken off", slot.key());
                if let Err(e) = owned.publish(next) {
                    warn!("equip: scripted: {e}");
                }
            }
        }
    }
    // `WOWSOULS_GIVE=<item>[,...]` only fills the pack.
    if let Ok(list) = std::env::var("WOWSOULS_GIVE") {
        let give: Vec<(u32, u32)> = list
            .split(',')
            .filter_map(|s| s.trim().parse::<u32>().ok())
            .map(|i| (i, 1))
            .collect();
        let mut next = owned.progress.clone();
        if next.inventory.add(&give).is_ok() {
            if let Err(e) = owned.publish(next) {
                warn!("equip: scripted: {e}");
            }
        }
    }
    let Ok(list) = std::env::var("WOWSOULS_EQUIP") else {
        return;
    };
    let who = wearer(game.as_deref());
    for item in list.split(',').filter_map(|s| s.trim().parse::<u32>().ok()) {
        let mut next = owned.progress.clone();
        if next.inventory.add(&[(item, 1)]).is_err() {
            continue;
        }
        match next.equip(&data.0.items, item, None, who) {
            Ok(slot) => {
                info!("equip: scripted: item {item} on {}", slot.key());
                if let Err(e) = owned.publish(next) {
                    warn!("equip: scripted: {e}");
                }
            }
            Err(r) => {
                info!("equip: scripted: item {item} refused: {}", r.message());
                ui.message = r.message();
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<GearScreen>()
        .insert_resource(WearOverride::from_env())
        .add_systems(
            Startup,
            grant_kit
                .after(crate::loot::load)
                .after(crate::menu::load_character),
        )
        .add_systems(
            Update,
            (scripted_equip, act, auto_equip, apply)
                .chain()
                .after(crate::loot::Drive)
                .run_if(in_state(Phase::Playing)),
        )
        .add_plugins(ui::plugin);
}

// ---------------------------------------------------------------------------------------------
// Words
// ---------------------------------------------------------------------------------------------

/// The colour WoW prints an item's name in, by `quality`.
pub fn quality_color(quality: u32) -> Color {
    match quality {
        0 => Color::srgb(0.62, 0.62, 0.62),
        2 => Color::srgb(0.12, 1.0, 0.0),
        3 => Color::srgb(0.0, 0.44, 0.87),
        4 => Color::srgb(0.64, 0.21, 0.93),
        5 => Color::srgb(1.0, 0.5, 0.0),
        _ => Color::srgb(1.0, 1.0, 1.0),
    }
}

/// The place word of a tooltip's first line (`INVTYPE_*`).
pub fn place_word(inventory_type: u32) -> &'static str {
    use equipment::inv;
    match inventory_type {
        inv::HEAD => "Head",
        inv::NECK => "Neck",
        inv::SHOULDERS => "Shoulder",
        inv::BODY => "Shirt",
        inv::CHEST | inv::ROBE => "Chest",
        inv::WAIST => "Waist",
        inv::LEGS => "Legs",
        inv::FEET => "Feet",
        inv::WRISTS => "Wrist",
        inv::HANDS => "Hands",
        inv::FINGER => "Finger",
        inv::TRINKET => "Trinket",
        inv::WEAPON => "One-Hand",
        inv::SHIELD => "Off Hand",
        inv::RANGED | inv::RANGED_RIGHT => "Ranged",
        inv::CLOAK => "Back",
        inv::TWO_HAND => "Two-Hand",
        inv::BAG | inv::QUIVER => "Bag",
        inv::TABARD => "Tabard",
        inv::MAIN_HAND => "Main Hand",
        inv::OFF_HAND => "Off Hand",
        inv::HOLDABLE => "Held In Off-hand",
        inv::AMMO => "Ammo",
        inv::THROWN => "Thrown",
        _ => "",
    }
}

/// The kind word of a weapon (`item_template.subclass` of class 2) or armor (class 4).
pub fn kind_word(class: u32, subclass: u32) -> &'static str {
    match (class, subclass) {
        (2, 0 | 1) => "Axe",
        (2, 2) => "Bow",
        (2, 3) => "Gun",
        (2, 4 | 5) => "Mace",
        (2, 6) => "Polearm",
        (2, 7 | 8) => "Sword",
        (2, 10) => "Staff",
        (2, 13) => "Fist Weapon",
        (2, 14) => "Miscellaneous",
        (2, 15) => "Dagger",
        (2, 16) => "Thrown",
        (2, 17) => "Spear",
        (2, 18) => "Crossbow",
        (2, 19) => "Wand",
        (2, 20) => "Fishing Pole",
        (4, 1) => "Cloth",
        (4, 2) => "Leather",
        (4, 3) => "Mail",
        (4, 4) => "Plate",
        (4, 6) => "Shield",
        (6, 2) => "Arrow",
        (6, 3) => "Bullet",
        _ => "",
    }
}

/// What an item that is not worn is, in a word (`item_template.class`).
pub fn class_word(class: u32) -> &'static str {
    match class {
        0 => "Consumable",
        1 => "Bag",
        5 => "Reagent",
        7 => "Trade Goods",
        9 => "Recipe",
        12 => "Quest Item",
        15 => "Miscellaneous",
        _ => "",
    }
}

/// A stat type's word in a tooltip (1.12's names); `None` for the types the game does not show.
fn stat_word(kind: u32) -> Option<&'static str> {
    use wow_world::attributes::stat;
    Some(match kind {
        stat::AGILITY => "Agility",
        stat::STRENGTH => "Strength",
        stat::INTELLECT => "Intellect",
        stat::SPIRIT => "Spirit",
        stat::STAMINA => "Stamina",
        stat::MANA => "Mana",
        stat::HEALTH => "Health",
        _ => return None,
    })
}

/// The order a tooltip lists the five attributes in (WoW: Strength, Agility, Stamina, Intellect, Spirit).
const STAT_ORDER: [u32; 5] = [4, 3, 7, 5, 6];

/// The tooltip of an item as WoW prints it, one line per row.
pub fn tooltip(item: &Item) -> String {
    let mut lines: Vec<String> = Vec::new();
    let place = place_word(item.inventory_type);
    let kind = kind_word(item.class, item.subclass);
    lines.push(match (place.is_empty(), kind.is_empty()) {
        (false, false) => format!("{place}   {kind}"),
        (false, true) => place.to_string(),
        (true, false) => kind.to_string(),
        (true, true) => class_word(item.class).to_string(),
    });
    if item.class == 2 && item.delay > 0 {
        lines.push(format!(
            "{:.0} - {:.0} Damage   Speed {:.2}",
            item.damage.0,
            item.damage.1,
            item.delay as f32 / 1000.0
        ));
        lines.push(format!(
            "({:.1} damage per second)",
            equipment::weapon_dps(item)
        ));
    }
    if item.armor > 0 {
        lines.push(format!("{} Armor", item.armor));
    }
    if item.block > 0 {
        lines.push(format!("{} Block", item.block));
    }
    for kind in STAT_ORDER.into_iter().chain([0, 1]) {
        let value: i32 = item.stats.iter().filter(|s| s.0 == kind).map(|s| s.1).sum();
        if value != 0 {
            if let Some(word) = stat_word(kind) {
                lines.push(format!("{value:+} {word}"));
            }
        }
    }
    if item.required_level > 1 {
        lines.push(format!("Requires Level {}", item.required_level));
    }
    lines.retain(|l| !l.is_empty());
    lines.join("\n")
}

// ---------------------------------------------------------------------------------------------
// Looking at an item you do not own yet: the card and the comparison
// ---------------------------------------------------------------------------------------------

/// Whether a number is better, worse or the same as what is worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trend {
    Better,
    Worse,
    Same,
}

/// One row of the comparison, "46 Armor (+13)", and whether that is a gain.
#[derive(Clone, Debug, PartialEq)]
pub struct CompareLine {
    pub text: String,
    pub trend: Trend,
}

/// What the quest screens show of a reward: the tooltip, why the wearer cannot use it, and how it
/// stands against what the slot holds now.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub name: String,
    pub quality: u32,
    pub tooltip: String,
    /// WoW's refusal in its own words, for an item the character may not wear (yet).
    pub warning: Option<String>,
    /// "Compared with Worn Shortsword (Main Hand)", or that the slot is empty; empty for what is not worn.
    pub versus: String,
    pub compare: Vec<CompareLine>,
    /// What the slot holds now, as WoW's second tooltip ("Currently Equipped") shows it; `None` when the
    /// slot is empty or the item is not worn.
    pub worn: Option<Worn>,
}

/// The piece the card is held against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worn {
    pub name: String,
    pub quality: u32,
    pub tooltip: String,
}

/// Whether wearing the item would be a step up, down or sideways, for a list that marks its gear:
/// the compare rows weighed (one better row against one worse is a draw). `None` for what is not
/// gear, and for gear the wearer may not put on yet (its warning says why).
pub fn verdict(card: &Card) -> Option<Trend> {
    if card.warning.is_some() || card.compare.is_empty() {
        return None;
    }
    let count = |t: Trend| card.compare.iter().filter(|l| l.trend == t).count();
    let (better, worse) = (count(Trend::Better), count(Trend::Worse));
    Some(match better.cmp(&worse) {
        std::cmp::Ordering::Greater => Trend::Better,
        std::cmp::Ordering::Less => Trend::Worse,
        std::cmp::Ordering::Equal => Trend::Same,
    })
}

fn trend_of(delta: f32) -> Trend {
    if delta > 0.049 {
        Trend::Better
    } else if delta < -0.049 {
        Trend::Worse
    } else {
        Trend::Same
    }
}

/// `new` against `old` (what the slot holds now): damage per second for a weapon, armor and block for
/// whatever armor it replaces. `also` is what a two-hander would take off as well.
pub fn compare(new: &Item, old: Option<&Item>, also: Option<&Item>) -> Vec<CompareLine> {
    let mut out = Vec::new();
    let is_weapon = |i: &Item| i.class == 2 && i.delay > 0;
    if is_weapon(new) {
        let before = old
            .filter(|o| is_weapon(o))
            .map_or(0.0, equipment::weapon_dps);
        let now = equipment::weapon_dps(new);
        out.push(CompareLine {
            text: format!("{now:.1} damage per second ({:+.1})", now - before),
            trend: trend_of(now - before),
        });
    }
    let old_armor = old.map_or(0, |o| o.armor);
    let old_block = old.map_or(0, |o| o.block);
    if new.armor > 0 || (old_armor > 0 && !is_weapon(new)) {
        let d = i64::from(new.armor) - i64::from(old_armor);
        out.push(CompareLine {
            text: format!("{} Armor ({d:+})", new.armor),
            trend: trend_of(d as f32),
        });
    }
    if new.block > 0 || old_block > 0 {
        let d = i64::from(new.block) - i64::from(old_block);
        out.push(CompareLine {
            text: format!("{} Block ({d:+})", new.block),
            trend: trend_of(d as f32),
        });
    }
    // Stats: each of the five attributes held against the worn piece. Intellect and Spirit do nothing
    // yet (no mana, no regeneration): shown, not weighed.
    let sum =
        |i: &Item, kind: u32| -> i32 { i.stats.iter().filter(|s| s.0 == kind).map(|s| s.1).sum() };
    for kind in STAT_ORDER {
        let (now, before) = (sum(new, kind), old.map_or(0, |o| sum(o, kind)));
        if now == 0 && before == 0 {
            continue;
        }
        let inert = matches!(kind, 5 | 6);
        let word = stat_word(kind).unwrap_or("");
        out.push(CompareLine {
            text: format!("{now:+} {word} ({:+})", now - before),
            trend: if inert {
                Trend::Same
            } else {
                trend_of((now - before) as f32)
            },
        });
    }
    if let Some(also) = also {
        out.push(CompareLine {
            text: format!("Also takes off your {}", also.name),
            trend: Trend::Worse,
        });
    }
    out
}

/// The card of item `entry` for `wearer` wearing `gear`: the slot a quick equip would use is the one
/// it is held up against.
pub fn card(items: &[Item], gear: &Gear, wearer: Wearer, entry: u32) -> Option<Card> {
    let item = find(items, entry)?;
    let slots = equipment::slots_for(item.inventory_type);
    let slot = gear
        .best_slot(items, item, wearer)
        .or_else(|| slots.first().copied());
    let warning = slot
        .and_then(|s| gear.plan(items, item, s, wearer).err())
        .map(Refusal::message);
    let worn = slot.and_then(|s| gear.get(s)).and_then(|e| find(items, e));
    let also = (slot == Some(Slot::MainHand) && item.inventory_type == equipment::inv::TWO_HAND)
        .then(|| gear.get(Slot::OffHand).and_then(|e| find(items, e)))
        .flatten();
    let versus = match (slot, worn) {
        (Some(s), Some(w)) => format!("Compared with {} ({})", w.name, s.name()),
        (Some(s), None) => format!("Your {} slot is empty", s.name()),
        (None, _) => String::new(),
    };
    Some(Card {
        name: item.name.clone(),
        quality: item.quality,
        tooltip: tooltip(item),
        warning,
        versus,
        compare: if slot.is_some() {
            compare(item, worn, also)
        } else {
            Vec::new()
        },
        worn: worn.map(|w| Worn {
            name: w.name.clone(),
            quality: w.quality,
            tooltip: tooltip(w),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::equipment::{inv, Gear, Slot};

    #[test]
    fn a_weapon_is_held_against_the_weapon_in_its_hand_by_damage_per_second() {
        let mut worn = item(25, inv::MAIN_HAND, 2, 7);
        worn.name = "Worn Shortsword".into();
        worn.damage = (1.0, 3.0, 0);
        worn.delay = 1900;
        let mut better = item(26, inv::MAIN_HAND, 2, 7);
        better.name = "Militia Shortsword".into();
        better.damage = (4.0, 8.0, 0);
        better.delay = 2000;
        let items = vec![worn, better];
        let mut gear = Gear::default();
        gear.set(Slot::MainHand, Some(25));
        let who = Wearer::new(1, 1, 5);
        let c = card(&items, &gear, who, 26).unwrap();
        assert_eq!(c.versus, "Compared with Worn Shortsword (Main Hand)");
        assert_eq!(c.compare.len(), 1);
        assert_eq!(c.compare[0].trend, Trend::Better);
        assert_eq!(c.compare[0].text, "3.0 damage per second (+1.9)");
        assert_eq!(c.warning, None);
        let worn = c.worn.as_ref().expect("the piece it is held against");
        assert_eq!(worn.name, "Worn Shortsword");
        assert!(
            worn.tooltip.contains("damage per second"),
            "{}",
            worn.tooltip
        );
        assert_eq!(verdict(&c), Some(Trend::Better));
        // The other way round it is a loss.
        gear.set(Slot::MainHand, Some(26));
        let c = card(&items, &gear, who, 25).unwrap();
        assert_eq!(c.compare[0].trend, Trend::Worse);
        // Against empty hands every point is a gain.
        let c = card(&items, &Gear::default(), who, 25).unwrap();
        assert_eq!(c.versus, "Your Main Hand slot is empty");
        assert_eq!(c.compare[0].trend, Trend::Better);
    }

    #[test]
    fn the_verdict_weighs_the_rows_and_is_silent_for_what_is_not_gear() {
        let card = |rows: &[Trend], warning: Option<&str>| Card {
            name: "x".into(),
            quality: 1,
            tooltip: String::new(),
            warning: warning.map(Into::into),
            versus: String::new(),
            compare: rows
                .iter()
                .map(|&trend| CompareLine {
                    text: String::new(),
                    trend,
                })
                .collect(),
            worn: None,
        };
        assert_eq!(verdict(&card(&[Trend::Better], None)), Some(Trend::Better));
        assert_eq!(verdict(&card(&[Trend::Worse], None)), Some(Trend::Worse));
        assert_eq!(
            verdict(&card(&[Trend::Better, Trend::Worse], None)),
            Some(Trend::Same),
            "one up, one down is a draw"
        );
        assert_eq!(verdict(&card(&[Trend::Same], None)), Some(Trend::Same));
        assert_eq!(
            verdict(&card(&[], None)),
            None,
            "a potion has nothing to compare"
        );
        assert_eq!(
            verdict(&card(&[Trend::Better], Some("Level 4"))),
            None,
            "gear the wearer may not use yet is no step up"
        );
    }

    #[test]
    fn a_tooltip_lists_the_stats_after_armor_in_wows_words() {
        let mut vest = item(3288, inv::CHEST, 4, 2);
        vest.armor = 65;
        vest.required_level = 8;
        vest.stats = vec![(3, 2), (7, 1)];
        assert_eq!(
            tooltip(&vest),
            "Chest   Leather
65 Armor
+2 Agility
+1 Stamina
Requires Level 8"
        );
    }

    #[test]
    fn a_comparison_holds_each_stat_against_the_worn_piece_and_ignores_the_inert_ones() {
        let mut worn = item(1, inv::CHEST, 4, 2);
        worn.stats = vec![(7, 1)];
        let mut new = item(2, inv::CHEST, 4, 2);
        new.stats = vec![(7, 4), (6, 2)];
        let lines = compare(&new, Some(&worn), None);
        let stamina = lines.iter().find(|l| l.text.contains("Stamina")).unwrap();
        assert_eq!(stamina.text, "+4 Stamina (+3)");
        assert_eq!(stamina.trend, Trend::Better);
        let spirit = lines.iter().find(|l| l.text.contains("Spirit")).unwrap();
        assert_eq!(spirit.text, "+2 Spirit (+2)");
        assert_eq!(
            spirit.trend,
            Trend::Same,
            "spirit does nothing yet: shown, not weighed"
        );
    }

    #[test]
    fn losing_a_stat_is_a_worse_row() {
        let mut worn = item(1, inv::CHEST, 4, 2);
        worn.stats = vec![(4, 3)];
        let new = item(2, inv::CHEST, 4, 2);
        let lines = compare(&new, Some(&worn), None);
        let strength = lines.iter().find(|l| l.text.contains("Strength")).unwrap();
        assert_eq!(strength.text, "+0 Strength (-3)");
        assert_eq!(strength.trend, Trend::Worse);
    }

    #[test]
    fn armor_is_held_against_the_piece_it_would_replace() {
        let mut leather = item(1, inv::CHEST, 4, 2);
        leather.armor = 33;
        let mut chain = item(2, inv::CHEST, 4, 3);
        chain.armor = 67;
        let items = vec![leather, chain];
        let mut gear = Gear::default();
        gear.set(Slot::Chest, Some(1));
        let who = Wearer::new(1, 1, 1);
        let c = card(&items, &gear, who, 2).unwrap();
        assert_eq!(
            c.compare,
            vec![CompareLine {
                text: "67 Armor (+34)".into(),
                trend: Trend::Better
            }]
        );
        let c = card(&items, &gear, who, 1).unwrap();
        assert_eq!(
            c.compare,
            vec![CompareLine {
                text: "33 Armor (+0)".into(),
                trend: Trend::Same
            }]
        );
    }

    #[test]
    fn an_item_the_wearer_cannot_use_says_why_and_is_still_compared() {
        let mut vest = item(10, inv::CHEST, 4, 2);
        vest.armor = 46;
        vest.required_level = 4;
        let items = vec![vest];
        let c = card(&items, &Gear::default(), Wearer::new(1, 1, 1), 10).unwrap();
        assert_eq!(
            c.warning.as_deref(),
            Some("You must reach level 4 to use that item.")
        );
        assert_eq!(c.compare[0].text, "46 Armor (+46)");
        let c = card(&items, &Gear::default(), Wearer::new(1, 1, 4), 10).unwrap();
        assert_eq!(c.warning, None);
    }

    #[test]
    fn a_two_hander_says_it_takes_the_shield_off_too() {
        let mut staff = item(30, inv::TWO_HAND, 2, 10);
        staff.damage = (3.0, 5.0, 0);
        staff.delay = 2500;
        let mut shield = item(31, inv::SHIELD, 4, 6);
        shield.name = "Worn Wooden Shield".into();
        shield.armor = 5;
        let items = vec![staff, shield];
        let mut gear = Gear::default();
        gear.set(Slot::OffHand, Some(31));
        let c = card(&items, &gear, Wearer::new(1, 1, 5), 30).unwrap();
        assert!(
            c.compare
                .iter()
                .any(|l| l.text == "Also takes off your Worn Wooden Shield"
                    && l.trend == Trend::Worse),
            "{:?}",
            c.compare
        );
    }

    #[test]
    fn what_is_not_worn_has_a_card_without_a_comparison() {
        let mut salve = item(40, 0, 0, 0);
        salve.name = "Salve".into();
        let items = vec![salve];
        let who = Wearer::new(1, 1, 1);
        let c = card(&items, &Gear::default(), who, 40).unwrap();
        assert_eq!(c.tooltip, "Consumable");
        assert!(c.versus.is_empty() && c.compare.is_empty() && c.warning.is_none());
        assert!(card(&items, &Gear::default(), who, 999).is_none());
    }

    /// A bare item row.
    fn item(entry: u32, inventory_type: u32, class: u32, subclass: u32) -> Item {
        Item {
            entry,
            name: format!("Item {entry}"),
            class,
            subclass,
            display: 0,
            quality: 1,
            flags: 0,
            buy_price: 0,
            sell_price: 0,
            inventory_type,
            allowable_class: -1,
            allowable_race: -1,
            item_level: 1,
            required_level: 1,
            max_count: 1,
            stackable: 1,
            delay: 0,
            damage: (0.0, 0.0, 0),
            block: 0,
            armor: 0,
            stats: Vec::new(),
        }
    }

    fn shown(entry: u32, inventory_type: u32, display: u32) -> Item {
        Item {
            display,
            ..item(entry, inventory_type, 4, 2)
        }
    }

    #[test]
    fn gear_becomes_displays_by_slot() {
        let items = vec![
            shown(1, inv::CHEST, 100),
            shown(2, inv::LEGS, 101),
            shown(3, inv::CLOAK, 102),
            shown(4, inv::HEAD, 103),
            shown(5, inv::SHOULDERS, 104),
            shown(6, inv::MAIN_HAND, 105),
            shown(7, inv::SHIELD, 106),
            shown(8, inv::FINGER, 107),
            shown(9, inv::TABARD, 0),
        ];
        let mut gear = Gear::default();
        for (slot, entry) in [
            (Slot::Chest, 1),
            (Slot::Legs, 2),
            (Slot::Back, 3),
            (Slot::Head, 4),
            (Slot::Shoulders, 5),
            (Slot::MainHand, 6),
            (Slot::OffHand, 7),
            (Slot::Finger1, 8),
            (Slot::Tabard, 9),
        ] {
            gear.set(slot, Some(entry));
        }
        let a = appearance(&gear, &items);
        // head, shoulder, shirt, chest, belt, pants, boots, wrist, gloves, tabard
        assert_eq!(a.dress.worn, [103, 104, 0, 100, 0, 101, 0, 0, 0, 0]);
        assert_eq!(a.dress.cloak, 102);
        assert_eq!(a.main, Some(105));
        assert_eq!(a.off, Some((106, true)));
        assert_eq!(appearance(&Gear::default(), &items), Appearance::default());
    }

    #[test]
    fn an_off_hand_weapon_is_not_a_shield() {
        let items = vec![shown(1, inv::WEAPON, 50), shown(2, inv::HOLDABLE, 51)];
        let mut gear = Gear::default();
        gear.set(Slot::OffHand, Some(1));
        assert_eq!(appearance(&gear, &items).off, Some((50, false)));
        gear.set(Slot::OffHand, Some(2));
        assert_eq!(appearance(&gear, &items).off, Some((51, false)));
    }

    #[test]
    fn the_wear_override_lays_displays_over_the_save() {
        let w = WearOverride::parse("head:1200, cloak:15050,shield:7,bogus:9,legs:x");
        let a = w.over(Appearance::default());
        assert_eq!(a.dress.worn[0], 1200);
        assert_eq!(a.dress.cloak, 15050);
        assert_eq!(a.off, Some((7, true)));
        assert_eq!(w.0.len(), 4, "the unreadable pair is dropped");
        assert_eq!(WearOverride::default().over(a.clone()), a);
    }

    #[test]
    fn the_cursor_walks_the_pane_the_way_the_character_frame_is_laid_out() {
        // Down the left column to the weapon row, across, and back up the right column.
        assert_eq!(neighbor(Slot::Head, Dir::Down), Slot::Neck);
        assert_eq!(neighbor(Slot::Head, Dir::Up), Slot::Head);
        assert_eq!(neighbor(Slot::Wrists, Dir::Down), Slot::MainHand);
        assert_eq!(neighbor(Slot::Trinket2, Dir::Down), Slot::Ranged);
        assert_eq!(neighbor(Slot::Head, Dir::Right), Slot::Hands);
        assert_eq!(neighbor(Slot::Hands, Dir::Left), Slot::Head);
        assert_eq!(neighbor(Slot::Hands, Dir::Right), Slot::Hands);
        assert_eq!(neighbor(Slot::MainHand, Dir::Right), Slot::OffHand);
        assert_eq!(neighbor(Slot::Ammo, Dir::Right), Slot::Ammo);
        assert_eq!(neighbor(Slot::MainHand, Dir::Left), Slot::MainHand);
        assert_eq!(neighbor(Slot::OffHand, Dir::Up), Slot::Wrists);
        assert_eq!(neighbor(Slot::Ranged, Dir::Up), Slot::Trinket2);
        // Every slot is reachable from the head by moving.
        let mut seen = std::collections::BTreeSet::new();
        let mut frontier = vec![Slot::Head];
        while let Some(s) = frontier.pop() {
            if seen.insert(s) {
                for d in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
                    frontier.push(neighbor(s, d));
                }
            }
        }
        assert_eq!(seen.len(), Slot::ALL.len());
    }

    #[test]
    fn the_choice_list_shows_what_fits_and_why_the_rest_does_not() {
        use wow_world::inventory::Progress;
        let mut plate = item(10, inv::CHEST, 4, 4);
        plate.name = "Plate Vest".into();
        let leather = item(11, inv::CHEST, 4, 2);
        let ring = item(12, inv::FINGER, 4, 0);
        let items = vec![plate, leather, ring];
        let mut p = Progress::default();
        p.inventory.add(&[(10, 1), (11, 1), (12, 1)]).unwrap();
        let who = Wearer::new(1, 1, 5);
        let order = [12, 11, 10];
        let list = candidates(&p, &items, Slot::Chest, who, &order);
        assert_eq!(
            list,
            vec![(11, Ok(())), (10, Err(Refusal::Proficiency))],
            "the ring does not go on the chest, the plate is listed but refused"
        );
        assert!(candidates(&p, &items, Slot::Head, who, &order).is_empty());
    }

    #[test]
    fn a_request_gives_a_new_save_and_a_message_or_a_refusal() {
        use wow_world::inventory::Progress;
        let mut cloak = item(20, inv::CLOAK, 4, 1);
        cloak.name = "Frayed Cloak".into();
        let items = vec![cloak];
        let mut p = Progress::default();
        p.inventory.add(&[(20, 1)]).unwrap();
        let who = Wearer::new(1, 1, 1);
        let (next, message) = attempt(
            &p,
            &items,
            Request::Equip {
                item: 20,
                slot: None,
            },
            who,
        )
        .unwrap();
        assert_eq!(message, "Equipped: Frayed Cloak");
        assert_eq!(next.gear.get(Slot::Back), Some(20));
        assert_eq!(p.gear.get(Slot::Back), None, "the old save is untouched");
        let (back, message) = attempt(&next, &items, Request::Unequip(Slot::Back), who).unwrap();
        assert_eq!(message, "Taken off: Frayed Cloak");
        assert_eq!(back.inventory.count(20), 1);
        assert_eq!(
            attempt(&p, &items, Request::Unequip(Slot::Head), who).unwrap_err(),
            Refusal::Empty
        );
    }

    #[test]
    fn what_is_handed_over_goes_on_in_an_empty_slot_and_says_so() {
        use wow_world::inventory::Progress;
        let mut cloak = item(30, inv::CLOAK, 4, 1);
        cloak.name = "Ensign Cloak".into();
        let mut belt = item(31, inv::WAIST, 4, 2);
        belt.name = "Outfitter Belt".into();
        let mut spare = item(32, inv::CLOAK, 4, 1);
        spare.name = "Frayed Cloak".into();
        let junk = item(33, 0, 0, 0);
        let items = vec![cloak, belt, spare, junk];
        let mut p = Progress::default();
        let got = [(30, 1), (31, 1), (32, 1), (33, 2)];
        p.inventory.add(&got).unwrap();
        let lines = put_on_handed(&mut p, &items, &got, None);
        assert_eq!(
            lines,
            ["Equipped: Ensign Cloak", "Equipped: Outfitter Belt"]
        );
        assert_eq!(p.gear.get(Slot::Back), Some(30));
        assert_eq!(p.gear.get(Slot::Waist), Some(31));
        // The second cloak finds the back taken and stays carried, like the junk.
        assert_eq!((p.inventory.count(32), p.inventory.count(33)), (1, 2));
        // Nothing handed over that fits: nothing said.
        let more = [(32, 1)];
        assert!(put_on_handed(&mut p, &items, &more, None).is_empty());
    }

    /// The system on a bare world: an `Acquired` message for `got` (already in the pack) and the
    /// setting; the save afterwards and the toasts.
    fn hand_over(got: &[(u32, u32)], on: bool) -> (wow_world::inventory::Progress, Vec<String>) {
        use bevy::ecs::world::World;
        use bevy::time::Real;
        use wow_world::inventory::Progress;
        let mut cloak = item(30, inv::CLOAK, 4, 1);
        cloak.name = "Ensign Cloak".into();
        let mut w = World::new();
        let data = wow_world::model::World {
            items: vec![cloak],
            ..Default::default()
        };
        let mut progress = Progress::default();
        progress.inventory.add(got).unwrap();
        let path =
            std::env::temp_dir().join(format!("wow-auto-equip-{}-{}.txt", std::process::id(), on));
        w.insert_resource(Data(data));
        w.insert_resource(Owned {
            progress,
            path: path.clone(),
            blocked: None,
        });
        let mut settings = Settings::default();
        settings.game.auto_equip = on;
        w.insert_resource(settings);
        w.insert_resource(Time::<Real>::default());
        w.init_resource::<Toasts>();
        w.init_resource::<Messages<Acquired>>();
        w.write_message(Acquired {
            items: got.to_vec(),
            discovery: false,
        });
        let system = w.register_system(auto_equip);
        w.run_system(system).unwrap();
        let progress = w.resource::<Owned>().progress.clone();
        let toasts = w
            .resource::<Toasts>()
            .lines
            .iter()
            .map(|(l, _)| l.clone())
            .collect();
        let _ = std::fs::remove_file(path);
        (progress, toasts)
    }

    #[test]
    fn a_pickup_puts_gear_on_by_itself_and_the_game_menu_can_switch_that_off() {
        let (p, toasts) = hand_over(&[(30, 1)], true);
        assert_eq!(p.gear.get(Slot::Back), Some(30));
        assert_eq!(p.inventory.count(30), 0);
        assert_eq!(toasts, ["Equipped: Ensign Cloak"]);
        let (p, toasts) = hand_over(&[(30, 1)], false);
        assert_eq!(p.gear.get(Slot::Back), None);
        assert_eq!(p.inventory.count(30), 1);
        assert!(toasts.is_empty());
    }

    #[test]
    fn a_tooltip_reads_like_wows() {
        let mut sword = item(25, inv::MAIN_HAND, 2, 7);
        sword.damage = (1.0, 3.0, 0);
        sword.delay = 1900;
        assert_eq!(
            tooltip(&sword),
            "Main Hand   Sword\n1 - 3 Damage   Speed 1.90\n(1.1 damage per second)"
        );
        let mut shield = item(2362, inv::SHIELD, 4, 6);
        shield.armor = 5;
        shield.block = 1;
        shield.required_level = 1;
        assert_eq!(tooltip(&shield), "Off Hand   Shield\n5 Armor\n1 Block");
        let mut vest = item(1425, inv::CHEST, 4, 2);
        vest.armor = 46;
        vest.required_level = 4;
        assert_eq!(
            tooltip(&vest),
            "Chest   Leather\n46 Armor\nRequires Level 4"
        );
        let ring = item(9, inv::FINGER, 4, 0);
        assert_eq!(tooltip(&ring), "Finger");
    }

    #[test]
    fn quality_colours_follow_wow() {
        assert_eq!(quality_color(1), Color::srgb(1.0, 1.0, 1.0));
        assert_eq!(quality_color(2), Color::srgb(0.12, 1.0, 0.0));
        assert_ne!(quality_color(0), quality_color(1));
    }
}
