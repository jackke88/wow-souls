//! The equipment page: the 1.12 character pane in the inventory screen's parchment. Two columns of
//! slots and the weapon row under them (`PaperDollFrame.xml`), each slot showing the worn item's icon
//! or WoW's own empty-slot art; the player's body stays visible between the columns. Enter / A on a
//! slot opens a list of what the pack holds for it (and Take off), Enter equips; the right panel is
//! the tooltip of the item under the cursor and the last result in WoW's words.
//!
//! Keys: arrows / WASD / D-pad move, Enter / A opens and confirms, Delete takes the slot's item off,
//! Esc / B goes back one step (the list, then the screen), G / Select leaves the page, Tab / Start
//! goes to the pack. The mouse clicks slots and rows.

use bevy::prelude::*;
use bevy::sprite::{BorderRect, TextureSlicer};
use bevy::ui::widget::NodeImageMode;
use std::collections::BTreeMap;

use wow_world::equipment::{self, Refusal, Slot, Wearer};

use super::{
    candidates, card_at, find, neighbor, quality_color, short_delta, tooltip, verdict, Choosing,
    Dir, GearScreen, Page, Request, Trend,
};
use crate::loot::ui::{container, load_icon, plain, Art, Fonts, GOLD, IVORY, MUTED};
use crate::loot::{Data, Owned, Screen};
use crate::settle::Phase;

/// Side of a slot (px).
const CELL: f32 = 44.0;
/// Rows of the choice list.
const ROWS: usize = 7;
const DIM: Color = Color::srgb(0.31, 0.27, 0.19);
const CURSOR: Color = GOLD;
const RED: Color = Color::srgb(0.94, 0.45, 0.4);
const GREEN: Color = Color::srgb(0.55, 0.85, 0.45);
/// Lines of the comparison under the tooltip.
const COMPARE_LINES: usize = 5;

const GRID: [Slot; 21] = [
    Slot::MainHand,
    Slot::OffHand,
    Slot::Ranged,
    Slot::Ammo,
    Slot::Head,
    Slot::Chest,
    Slot::Hands,
    Slot::Legs,
    Slot::Feet,
    Slot::Shoulders,
    Slot::Back,
    Slot::Wrists,
    Slot::Waist,
    Slot::Shirt,
    Slot::Neck,
    Slot::Tabard,
    Slot::Finger1,
    Slot::Finger2,
    Slot::Trinket1,
    Slot::Trinket2,
    Slot::Torch,
];
fn grid_neighbor(slot: Slot, dir: Dir) -> Slot {
    let i = GRID.iter().position(|s| *s == slot).unwrap_or(0);
    let next = match dir {
        Dir::Up if i >= 4 => i - 4,
        Dir::Down if i + 4 < GRID.len() => i + 4,
        Dir::Left if i % 4 > 0 => i - 1,
        Dir::Right if i % 4 < 3 && i + 1 < GRID.len() => i + 1,
        _ => i,
    };
    GRID[next]
}

/// WoW's empty-slot picture for each slot (`GetInventorySlotInfo`); the back reuses the chest's.
fn empty_art(slot: Slot) -> &'static str {
    match slot {
        Slot::Head => "Interface/PaperDoll/UI-PaperDoll-Slot-Head",
        Slot::Neck => "Interface/PaperDoll/UI-PaperDoll-Slot-Neck",
        Slot::Shoulders => "Interface/PaperDoll/UI-PaperDoll-Slot-Shoulder",
        Slot::Shirt => "Interface/PaperDoll/UI-PaperDoll-Slot-Shirt",
        Slot::Chest | Slot::Back => "Interface/PaperDoll/UI-PaperDoll-Slot-Chest",
        Slot::Waist => "Interface/PaperDoll/UI-PaperDoll-Slot-Waist",
        Slot::Legs => "Interface/PaperDoll/UI-PaperDoll-Slot-Legs",
        Slot::Feet => "Interface/PaperDoll/UI-PaperDoll-Slot-Feet",
        Slot::Wrists => "Interface/PaperDoll/UI-PaperDoll-Slot-Wrists",
        Slot::Hands => "Interface/PaperDoll/UI-PaperDoll-Slot-Hands",
        Slot::Finger1 | Slot::Finger2 => "Interface/PaperDoll/UI-PaperDoll-Slot-Finger",
        Slot::Trinket1 | Slot::Trinket2 => "Interface/PaperDoll/UI-PaperDoll-Slot-Trinket",
        Slot::MainHand => "Interface/PaperDoll/UI-PaperDoll-Slot-MainHand",
        Slot::OffHand => "Interface/PaperDoll/UI-PaperDoll-Slot-SecondaryHand",
        Slot::Ranged => "Interface/PaperDoll/UI-PaperDoll-Slot-Ranged",
        Slot::Tabard => "Interface/PaperDoll/UI-PaperDoll-Slot-Tabard",
        Slot::Ammo => "Interface/PaperDoll/UI-PaperDoll-Slot-Ammo",
        // WoW has no torch slot: its unlit torch icon stands for the empty one.
        Slot::Torch => "Interface/Icons/INV_Torch_Unlit",
    }
}

#[derive(Resource)]
struct PaneArt {
    empty: BTreeMap<Slot, Handle<Image>>,
}

#[derive(Component)]
struct Root;
/// A slot of the pane: a button.
#[derive(Component)]
struct Cell(Slot);
/// A row of the choice list: a button.
#[derive(Component)]
struct Row(usize);
/// Every picture of the page.
#[derive(Component, Clone, Copy)]
enum Pic {
    Slot(Slot),
    Choice(usize),
    Detail,
}
#[derive(Component, Clone, Copy)]
enum Words {
    Heading,
    ItemName,
    Tooltip,
    Totals,
    Message,
    Hint,
    ChoiceName(usize),
    ChoiceNote(usize),
    /// "Compared with ..." under the highlighted option's tooltip.
    Versus,
    /// One row of that comparison, green for a gain, red for a loss.
    Compare(usize),
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Phase::Playing), spawn).add_systems(
        Update,
        (
            input.before(crate::loot::Drive),
            buttons.before(crate::loot::Drive),
        )
            .run_if(in_state(Phase::Playing)),
    );
    app.add_systems(
        Update,
        refresh
            .after(crate::loot::Drive)
            .run_if(in_state(Phase::Playing)),
    );
}

fn frame_node(
    commands: &mut Commands,
    node: Node,
    image: &Handle<Image>,
    parent: Entity,
) -> Entity {
    if crate::hud::theme::enabled() {
        return crate::hud::theme::panel(commands, node, Some(parent));
    }
    let mut image = ImageNode::new(image.clone());
    image.image_mode = NodeImageMode::Sliced(TextureSlicer {
        border: BorderRect::all(18.0),
        ..default()
    });
    commands.spawn((node, image, ChildOf(parent))).id()
}

fn pic(commands: &mut Commands, parent: Entity, size: f32, kind: Pic) -> Entity {
    commands
        .spawn((
            Node {
                width: px(size),
                height: px(size),
                min_width: px(size),
                ..default()
            },
            ImageNode::default(),
            kind,
            ChildOf(parent),
        ))
        .id()
}

fn spawn(
    mut commands: Commands,
    fonts: Res<Fonts>,
    assets: Res<AssetServer>,
    hud: Option<Res<crate::hud::art::HudArt>>,
) {
    let empty = Slot::ALL
        .into_iter()
        .map(|s| (s, load_icon(&assets, empty_art(s))))
        .collect();
    commands.insert_resource(PaneArt { empty });
    let leather = hud
        .as_deref()
        .map(|h| h.sprite_image("soul_param/bg_parchment_0"))
        .unwrap_or_else(|| load_icon(&assets, "Interface/DialogFrame/UI-DialogBox-Background"));
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.42)),
            Root,
            Visibility::Hidden,
            GlobalZIndex(41),
        ))
        .id();

    // The doll: heading, the two columns, the weapon row, the totals.
    let doll = frame_node(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            left: percent(3),
            top: percent(5),
            bottom: percent(8),
            width: percent(34),
            padding: UiRect::all(px(22)),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            ..default()
        },
        &leather,
        root,
    );
    let heading = container(
        &mut commands,
        doll,
        Node {
            width: percent(100),
            padding: UiRect::bottom(px(8)),
            border: UiRect::bottom(px(1)),
            ..default()
        },
    );
    commands.entity(heading).insert(BorderColor::all(GOLD));
    plain(
        &mut commands,
        heading,
        "Equipment",
        &fonts.title,
        26.0,
        IVORY,
    );
    if crate::hud::theme::enabled() {
        let grid = container(
            &mut commands,
            doll,
            Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::px(4, 64.0),
                row_gap: px(10),
                column_gap: px(10),
                margin: UiRect::axes(px(4), px(14)),
                ..default()
            },
        );
        for slot in GRID {
            cell(&mut commands, grid, slot);
        }
    } else {
        let columns = container(
            &mut commands,
            doll,
            Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(px(18)),
                margin: UiRect::top(px(6)),
                ..default()
            },
        );
        for slots in [&equipment::LEFT_COLUMN[..], &equipment::RIGHT_COLUMN[..]] {
            let column = container(
                &mut commands,
                columns,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(5),
                    ..default()
                },
            );
            for &slot in slots {
                cell(&mut commands, column, slot);
            }
        }
        let weapons = container(
            &mut commands,
            doll,
            Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                column_gap: px(14),
                margin: UiRect::top(px(8)),
                ..default()
            },
        );
        for slot in equipment::WEAPON_ROW.into_iter().chain(equipment::BELT_ROW) {
            cell(&mut commands, weapons, slot);
        }
    }
    let totals = plain(&mut commands, doll, "", &fonts.body, 16.0, IVORY);
    commands.entity(totals).insert((
        Words::Totals,
        Node {
            align_self: AlignSelf::Center,
            margin: UiRect::top(px(8)),
            ..default()
        },
    ));

    // The right panel: the slot or item, the list, the tooltip, the result.
    let info = frame_node(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            right: percent(3),
            top: percent(5),
            bottom: percent(8),
            width: percent(42),
            padding: UiRect::all(px(22)),
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            ..default()
        },
        &leather,
        root,
    );
    let head = container(
        &mut commands,
        info,
        Node {
            width: percent(100),
            padding: UiRect::bottom(px(8)),
            border: UiRect::bottom(px(1)),
            ..default()
        },
    );
    commands.entity(head).insert(BorderColor::all(GOLD));
    let title = plain(&mut commands, head, "", &fonts.title, 24.0, IVORY);
    commands.entity(title).insert(Words::Heading);
    let list = container(
        &mut commands,
        info,
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            ..default()
        },
    );
    for index in 0..ROWS {
        let row = container(
            &mut commands,
            list,
            Node {
                height: px(40),
                min_height: px(40),
                width: percent(100),
                padding: UiRect::axes(px(7), px(2)),
                border: UiRect::bottom(px(1)),
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            },
        );
        commands.entity(row).insert((
            Button,
            Row(index),
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::srgba(0.36, 0.29, 0.17, 0.55)),
        ));
        pic(&mut commands, row, 32.0, Pic::Choice(index));
        let name = plain(&mut commands, row, "", &fonts.body, 17.0, IVORY);
        commands.entity(name).insert((
            Words::ChoiceName(index),
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                ..default()
            },
        ));
        let note = plain(&mut commands, row, "", &fonts.body, 13.0, MUTED);
        commands.entity(note).insert(Words::ChoiceNote(index));
    }
    let detail = container(
        &mut commands,
        info,
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(14),
            margin: UiRect::top(px(6)),
            ..default()
        },
    );
    pic(&mut commands, detail, 56.0, Pic::Detail);
    let words = container(
        &mut commands,
        detail,
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            flex_grow: 1.0,
            min_width: px(0),
            ..default()
        },
    );
    let name = plain(&mut commands, words, "", &fonts.title, 20.0, IVORY);
    commands.entity(name).insert(Words::ItemName);
    let tip = plain(&mut commands, words, "", &fonts.body, 15.0, MUTED);
    commands.entity(tip).insert(Words::Tooltip);
    let versus = plain(&mut commands, words, "", &fonts.body, 14.0, MUTED);
    commands.entity(versus).insert(Words::Versus);
    for i in 0..COMPARE_LINES {
        let line = plain(&mut commands, words, "", &fonts.body, 15.0, IVORY);
        commands.entity(line).insert(Words::Compare(i));
    }
    let message = plain(&mut commands, info, "", &fonts.body, 16.0, IVORY);
    commands.entity(message).insert(Words::Message);
    let hint = plain(&mut commands, info, "", &fonts.body, 14.0, MUTED);
    commands.entity(hint).insert((
        Words::Hint,
        Node {
            margin: UiRect::top(px(4)),
            ..default()
        },
    ));
}

/// One slot of the pane: a bordered button holding a picture.
fn cell(commands: &mut Commands, parent: Entity, slot: Slot) {
    let size = if crate::hud::theme::enabled() {
        64.0
    } else {
        CELL
    };
    let button = commands
        .spawn((
            Node {
                width: px(size),
                height: px(size),
                border: UiRect::all(px(1)),
                padding: UiRect::all(px(5)),
                ..default()
            },
            Button,
            Cell(slot),
            crate::hud::theme::Surface::Slot(false),
            BorderColor::all(DIM),
            BackgroundColor(Color::srgba(0.02, 0.018, 0.014, 0.55)),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        ImageNode::default(),
        Pic::Slot(slot),
        ChildOf(button),
    ));
}

// ---------------------------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------------------------

/// Who the rules see.
fn who(game: Option<&crate::quests::QuestGame>) -> Wearer {
    game.map_or(
        Wearer::new(equipment::RACE_HUMAN, equipment::CLASS_WARRIOR, 1),
        |g| Wearer::new(g.player.race as u8, g.player.class as u8, g.player.level),
    )
}

/// What the pack holds for `slot`, the ones the wearer may use first and the refused after; Take off
/// (`None`) leads when `take_off` is asked and the slot is worn.
fn slot_options(
    slot: Slot,
    owned: &Owned,
    data: &Data,
    game: Option<&crate::quests::QuestGame>,
    take_off: bool,
) -> Vec<Option<u32>> {
    let order = crate::loot::ui::ordered(&owned.progress.inventory);
    let fits = candidates(&owned.progress, &data.0.items, slot, who(game), &order);
    let mut options: Vec<Option<u32>> = Vec::new();
    if take_off && owned.progress.gear.get(slot).is_some() {
        options.push(None);
    }
    options.extend(
        fits.iter()
            .filter(|(_, v)| v.is_ok())
            .map(|&(e, _)| Some(e)),
    );
    options.extend(
        fits.iter()
            .filter(|(_, v)| v.is_err())
            .map(|&(e, _)| Some(e)),
    );
    options
}

/// Opens the list of what the pack holds for the cursor's slot (and Take off when it is worn).
fn open_choice(
    ui: &mut GearScreen,
    owned: &Owned,
    data: &Data,
    game: Option<&crate::quests::QuestGame>,
) {
    let slot = ui.cursor;
    let options = slot_options(slot, owned, data, game, true);
    if options.is_empty() {
        ui.message = format!("Nothing in your pack goes in {}.", slot.name());
        ui.choosing = None;
    } else {
        ui.message.clear();
        ui.choosing = Some(Choosing {
            options,
            selected: 0,
        });
    }
}

/// Confirms the highlighted option of the list.
fn confirm_choice(ui: &mut GearScreen) {
    let Some(choice) = ui.choosing.as_ref() else {
        return;
    };
    let slot = ui.cursor;
    ui.request = Some(
        match choice.options.get(choice.selected).copied().flatten() {
            Some(item) => Request::Equip {
                item,
                slot: Some(slot),
            },
            None => Request::Unequip(slot),
        },
    );
}

/// What the player pressed this frame on the equipment page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Acts {
    dir: Option<Dir>,
    confirm: bool,
    back: bool,
    take_off: bool,
}

/// `WOWSOULS_GEAR_SCRIPT=<s>:<act>,...` (acts: up down left right confirm back takeoff): the hands of an
/// unattended run on the equipment page, each press once real second `s` has passed.
fn parse_script(text: &str) -> Vec<(f32, Acts)> {
    text.split(',')
        .filter_map(|step| {
            let (at, act) = step.trim().split_once(':')?;
            let mut a = Acts::default();
            match act.trim() {
                "up" => a.dir = Some(Dir::Up),
                "down" => a.dir = Some(Dir::Down),
                "left" => a.dir = Some(Dir::Left),
                "right" => a.dir = Some(Dir::Right),
                "confirm" => a.confirm = true,
                "back" => a.back = true,
                "takeoff" => a.take_off = true,
                _ => return None,
            }
            Some((at.trim().parse().ok()?, a))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn input(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    real: Res<Time<Real>>,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    mut screen: ResMut<Screen>,
    mut ui: ResMut<GearScreen>,
    owned: Res<Owned>,
    data: Res<Data>,
    game: Option<Res<crate::quests::QuestGame>>,
    mut scripted: Local<bool>,
    mut script: Local<Option<Vec<(f32, Acts)>>>,
    mut next_step: Local<usize>,
) {
    let on = screen.open && screen.page == Page::Gear;
    if !screen.open && !ui.message.is_empty() {
        ui.message.clear();
    }
    if !on {
        if ui.choosing.is_some() {
            ui.choosing = None;
        }
        *scripted = false;
        return;
    }
    // Scripted views for unattended runs: `WOWSOULS_GEAR_CURSOR=<slot key>`, `WOWSOULS_GEAR_CHOOSE=1`.
    if !*scripted {
        *scripted = true;
        if let Some(slot) = std::env::var("WOWSOULS_GEAR_CURSOR")
            .ok()
            .and_then(|k| Slot::from_key(k.trim()))
        {
            ui.cursor = slot;
        }
        if std::env::var("WOWSOULS_GEAR_CHOOSE").is_ok_and(|v| v.trim() == "1") {
            open_choice(&mut ui, &owned, &data, game.as_deref());
        }
    }
    let steps = script.get_or_insert_with(|| {
        std::env::var("WOWSOULS_GEAR_SCRIPT")
            .map(|t| parse_script(&t))
            .unwrap_or_default()
    });
    let acts = if unattended.is_some() {
        // The script's next press, when its second has come.
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
    let Acts {
        dir,
        confirm,
        back,
        take_off,
    } = acts;
    if let Some(choice) = ui.choosing.as_mut() {
        let n = choice.options.len();
        match dir {
            Some(Dir::Down) => choice.selected = (choice.selected + 1) % n,
            Some(Dir::Up) => choice.selected = (choice.selected + n - 1) % n,
            _ => {}
        }
        if back {
            ui.choosing = None;
            screen.back_taken = true;
        } else if confirm {
            confirm_choice(&mut ui);
        }
        return;
    }
    if let Some(dir) = dir {
        ui.cursor = if crate::hud::theme::enabled() {
            grid_neighbor(ui.cursor, dir)
        } else {
            neighbor(ui.cursor, dir)
        };
        ui.message.clear();
    }
    if confirm {
        open_choice(&mut ui, &owned, &data, game.as_deref());
    } else if take_off {
        ui.request = Some(Request::Unequip(ui.cursor));
    }
}

/// The keys and buttons pressed this frame.
fn live_acts(keys: &ButtonInput<KeyCode>, pads: &Query<&Gamepad>) -> Acts {
    let pressed = |codes: &[KeyCode]| codes.iter().any(|&k| keys.just_pressed(k));
    let pad = |b: GamepadButton| pads.iter().any(|p| p.just_pressed(b));
    let dir = if pressed(&[KeyCode::ArrowUp, KeyCode::KeyW]) || pad(GamepadButton::DPadUp) {
        Some(Dir::Up)
    } else if pressed(&[KeyCode::ArrowDown, KeyCode::KeyS]) || pad(GamepadButton::DPadDown) {
        Some(Dir::Down)
    } else if pressed(&[KeyCode::ArrowLeft, KeyCode::KeyA]) || pad(GamepadButton::DPadLeft) {
        Some(Dir::Left)
    } else if pressed(&[KeyCode::ArrowRight, KeyCode::KeyD]) || pad(GamepadButton::DPadRight) {
        Some(Dir::Right)
    } else {
        None
    };
    Acts {
        dir,
        confirm: pressed(&[KeyCode::Enter, KeyCode::NumpadEnter]) || pad(GamepadButton::South),
        back: pressed(&[KeyCode::Escape]) || pad(GamepadButton::East),
        take_off: pressed(&[KeyCode::Delete, KeyCode::Backspace]),
    }
}

/// Mouse: a click on a slot moves the cursor there and opens its list; a click on a row confirms it.
fn buttons(
    mut ui: ResMut<GearScreen>,
    screen: Res<Screen>,
    owned: Res<Owned>,
    data: Res<Data>,
    game: Option<Res<crate::quests::QuestGame>>,
    cells: Query<(&Interaction, &Cell), Changed<Interaction>>,
    rows: Query<(&Interaction, &Row), Changed<Interaction>>,
) {
    if !(screen.open && screen.page == Page::Gear) {
        return;
    }
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(choice) = ui.choosing.as_mut() {
            if row.0 < choice.options.len() {
                choice.selected = row.0;
                confirm_choice(&mut ui);
            }
        } else {
            // A row of the preview: open the list on that piece (Take off leads it when worn).
            let preview = slot_options(ui.cursor, &owned, &data, game.as_deref(), false);
            if let Some(&Some(entry)) = preview.get(row.0) {
                open_choice(&mut ui, &owned, &data, game.as_deref());
                if let Some(choice) = ui.choosing.as_mut() {
                    if let Some(i) = choice.options.iter().position(|o| *o == Some(entry)) {
                        choice.selected = i;
                    }
                }
            }
        }
    }
    for (interaction, cell) in &cells {
        if *interaction == Interaction::Pressed && ui.choosing.is_none() {
            ui.cursor = cell.0;
            open_choice(&mut ui, &owned, &data, game.as_deref());
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

/// The slot's name, the two fingers and the two trinkets told apart.
fn label(slot: Slot) -> String {
    match slot {
        Slot::Finger1 => "Finger 1".into(),
        Slot::Finger2 => "Finger 2".into(),
        Slot::Trinket1 => "Trinket 1".into(),
        Slot::Trinket2 => "Trinket 2".into(),
        other => other.name().to_string(),
    }
}

/// A list row's right edge for gear the wearer may put on: the changes in a few words, "+31 Armor
/// +1 Agi", or Equal when nothing changes.
fn row_note(card: &super::Card) -> String {
    let parts: Vec<String> = card
        .compare
        .iter()
        .filter(|l| l.trend != Trend::Same)
        .filter_map(short_delta)
        .take(3)
        .collect();
    if parts.is_empty() {
        "Equal".into()
    } else {
        parts.join("  ")
    }
}

fn trend_color(trend: Option<Trend>) -> Color {
    match trend {
        Some(Trend::Better) => GREEN,
        Some(Trend::Worse) => RED,
        _ => MUTED,
    }
}

/// The refusal in a few words, for the list's right edge.
fn short(refusal: Refusal) -> String {
    match refusal {
        Refusal::Level(n) => format!("Level {n}"),
        Refusal::TwoHanded => "Two-hander".into(),
        Refusal::DualWield => "No dual wield".into(),
        Refusal::Proficiency => "Not skilled".into(),
        Refusal::Never => "Not for you".into(),
        other => other.message(),
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh(
    screen: Res<Screen>,
    owned: Res<Owned>,
    data: Res<Data>,
    ui: Res<GearScreen>,
    game: Option<Res<crate::quests::QuestGame>>,
    moves: Option<Res<crate::motion_clip::WeaponClips>>,
    art: Option<Res<Art>>,
    pane: Option<Res<PaneArt>>,
    mut root: Query<&mut Visibility, (With<Root>, Without<Pic>)>,
    mut cells: Query<(&Cell, &mut BorderColor, &mut crate::hud::theme::Surface)>,
    mut rows: Query<(&Row, &mut Node, &mut BackgroundColor), Without<Cell>>,
    mut pics: Query<(&Pic, &mut ImageNode, &mut Visibility), Without<Root>>,
    mut words: Query<(&Words, &mut Text, &mut TextColor)>,
) {
    let show = screen.open && screen.page == Page::Gear;
    if let Ok(mut v) = root.single_mut() {
        let want = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    if !show {
        return;
    }
    let (Some(art), Some(pane)) = (art, pane) else {
        return;
    };
    let items = &data.0.items;
    let who = who(game.as_deref());
    let gear = &owned.progress.gear;
    let icon_of = |entry: u32| art.icons.get(&entry).cloned();
    // The list: the open choice, else a preview of what the pack holds for the cursor's slot.
    let options: Vec<Option<u32>> = match ui.choosing.as_ref() {
        Some(c) => c.options.clone(),
        None => slot_options(ui.cursor, &owned, &data, game.as_deref(), false),
    };
    let option_item = |index: usize| options.get(index).copied().flatten();
    // Each piece held against what the cursor's slot holds now.
    let card_of = |entry: u32| card_at(items, gear, who, entry, ui.cursor);

    // What the right panel describes: the highlighted option, else the item worn under the cursor.
    let highlighted = match ui.choosing.as_ref().and_then(|c| c.options.get(c.selected)) {
        Some(Some(entry)) => Some(*entry),
        _ => gear.get(ui.cursor),
    };
    let described = highlighted.and_then(|e| find(items, e));
    let chosen_card = ui
        .choosing
        .as_ref()
        .and_then(|c| c.options.get(c.selected).copied().flatten())
        .and_then(card_of);

    for (cell, mut border, mut surface) in &mut cells {
        let skin = crate::hud::theme::Surface::Slot(cell.0 == ui.cursor);
        if *surface != skin {
            *surface = skin;
        }
        let want = if cell.0 == ui.cursor { CURSOR } else { DIM };
        if border.top != want {
            *border = BorderColor::all(want);
        }
    }
    for (row, mut node, mut background) in &mut rows {
        let live = row.0 < options.len();
        let display = if live { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        let focused = ui.choosing.as_ref().is_some_and(|c| c.selected == row.0);
        let color = if focused {
            crate::hud::theme::SELECTED
        } else {
            Color::NONE
        };
        if background.0 != color {
            background.0 = color;
        }
    }
    for (kind, mut image, mut visibility) in &mut pics {
        let (handle, tint, visible) = match *kind {
            Pic::Slot(slot) => match gear.get(slot) {
                Some(entry) => (icon_of(entry), Color::WHITE, true),
                None => (
                    pane.empty.get(&slot).cloned(),
                    Color::srgba(1.0, 1.0, 1.0, 0.85),
                    true,
                ),
            },
            Pic::Choice(row) => match options.get(row) {
                Some(Some(entry)) => (icon_of(*entry), Color::WHITE, true),
                // Take off: the slot's own empty art.
                Some(None) => (
                    pane.empty.get(&ui.cursor).cloned(),
                    Color::srgba(1.0, 1.0, 1.0, 0.85),
                    true,
                ),
                None => (None, Color::WHITE, false),
            },
            Pic::Detail => (
                described.and_then(|i| icon_of(i.entry)),
                Color::WHITE,
                described.is_some(),
            ),
        };
        let v = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != v {
            *visibility = v;
        }
        if let Some(h) = handle {
            if image.image != h {
                image.image = h;
            }
        }
        if image.color != tint {
            image.color = tint;
        }
    }
    for (kind, mut text, mut color) in &mut words {
        let (value, tint): (String, Color) = match *kind {
            Words::Heading => (
                if ui.choosing.is_some() {
                    format!("Put on: {}", label(ui.cursor))
                } else if options.is_empty() {
                    label(ui.cursor)
                } else {
                    format!("{}  ·  {} in your pack", label(ui.cursor), options.len())
                },
                IVORY,
            ),
            Words::ItemName => match described {
                Some(i) => (i.name.clone(), quality_color(i.quality)),
                None => ("Empty".to_string(), MUTED),
            },
            Words::Tooltip => (described.map(tooltip).unwrap_or_default(), MUTED),
            Words::Totals => {
                let fx = wow_world::gear_effect::GearEffect::from_gear(
                    gear,
                    items,
                    crate::gear_effect::off_hand_held(moves.as_deref()),
                );
                let character = game
                    .as_deref()
                    .and_then(|g| crate::gear_effect::character(g, &owned, &data));
                let (fx, head) = match character {
                    Some(c) => (
                        fx.with_character(&c),
                        wow_world::attributes::sheet(&c) + "\n",
                    ),
                    None => (fx, String::new()),
                };
                (format!("{head}{}", fx.summary()), IVORY)
            }
            Words::Message => (ui.message.clone(), IVORY),
            Words::Hint => (
                if ui.choosing.is_some() {
                    "W / S: Choose    Enter / A: Confirm    Esc / B: Back".into()
                } else {
                    "Arrows / D-pad: Move    Enter / A: Change    Delete: Take off    Tab: Pack"
                        .into()
                },
                MUTED,
            ),
            Words::ChoiceName(row) => match options.get(row) {
                Some(Some(entry)) => find(items, *entry).map_or((String::new(), IVORY), |i| {
                    (i.name.clone(), quality_color(i.quality))
                }),
                Some(None) => ("Take off".to_string(), IVORY),
                None => (String::new(), IVORY),
            },
            Words::ChoiceNote(row) => {
                let refusal = option_item(row)
                    .and_then(|e| find(items, e))
                    .and_then(|i| gear.plan(items, i, ui.cursor, who).err());
                match (refusal, option_item(row).and_then(card_of)) {
                    (Some(r), _) => (short(r), RED),
                    // How much better or worse than what is worn, as the pack marks its gear.
                    (None, Some(card)) => (row_note(&card), trend_color(verdict(&card))),
                    (None, None) => (String::new(), MUTED),
                }
            }
            Words::Versus => (
                chosen_card
                    .as_ref()
                    .map_or(String::new(), |c| c.versus.clone()),
                MUTED,
            ),
            Words::Compare(i) => match chosen_card.as_ref().and_then(|c| c.compare.get(i)) {
                Some(line) => (line.text.clone(), trend_color(Some(line.trend))),
                None => (String::new(), MUTED),
            },
        };
        if text.0 != value {
            text.0 = value;
        }
        if color.0 != tint {
            color.0 = tint;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slot_has_empty_slot_art_and_the_back_borrows_the_chest() {
        for slot in Slot::ALL {
            assert!(
                empty_art(slot).starts_with("Interface/PaperDoll/UI-PaperDoll-Slot-")
                    || slot == Slot::Torch
            );
        }
        assert_eq!(empty_art(Slot::Back), empty_art(Slot::Chest));
        assert_eq!(empty_art(Slot::Finger1), empty_art(Slot::Finger2));
    }

    #[test]
    fn grid_navigation_follows_visual_neighbors_and_keeps_all_slots_reachable() {
        assert_eq!(grid_neighbor(Slot::MainHand, Dir::Down), Slot::Head);
        assert_eq!(grid_neighbor(Slot::Head, Dir::Right), Slot::Chest);
        assert_eq!(grid_neighbor(Slot::Trinket2, Dir::Right), Slot::Trinket2);
        assert_eq!(grid_neighbor(Slot::MainHand, Dir::Up), Slot::MainHand);
        let mut seen = std::collections::BTreeSet::new();
        let mut todo = vec![Slot::MainHand];
        while let Some(slot) = todo.pop() {
            if seen.insert(slot) {
                for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
                    todo.push(grid_neighbor(slot, dir));
                }
            }
        }
        assert_eq!(seen, Slot::ALL.into_iter().collect());
    }

    #[test]
    fn the_empty_slot_art_is_in_the_install() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        for slot in Slot::ALL {
            let path = format!("{}.blp", empty_art(slot).replace('/', "\\"));
            let bytes = chain
                .read_file(&path)
                .unwrap_or_else(|e| panic!("{path} is missing: {e}"));
            assert!(bytes.starts_with(b"BLP"), "{path}");
        }
    }

    #[test]
    fn the_script_names_presses_and_skips_what_it_cannot_read() {
        let steps = parse_script("7:confirm, 8.5:down,9:bogus,x:up,10:takeoff,11:back");
        let times: Vec<f32> = steps.iter().map(|s| s.0).collect();
        assert_eq!(times, vec![7.0, 8.5, 10.0, 11.0]);
        assert!(steps[0].1.confirm);
        assert_eq!(steps[1].1.dir, Some(Dir::Down));
        assert!(steps[2].1.take_off && steps[3].1.back);
        assert!(parse_script("").is_empty());
    }

    #[test]
    fn the_rings_and_trinkets_are_told_apart_in_the_heading() {
        assert_eq!(label(Slot::Finger1), "Finger 1");
        assert_eq!(label(Slot::Finger2), "Finger 2");
        assert_eq!(label(Slot::Trinket2), "Trinket 2");
        assert_eq!(label(Slot::MainHand), "Main Hand");
    }

    #[test]
    fn refusals_are_short_on_the_list() {
        assert_eq!(short(Refusal::Level(8)), "Level 8");
        assert_eq!(short(Refusal::Proficiency), "Not skilled");
    }
}
