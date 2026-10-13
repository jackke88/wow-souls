//! Souls framed pickup rows and inventory, using installed DS panel art and WoW item icons.
use super::{Acquired, Data, Owned, Screen, SALVE};
use crate::{actor::Catalogs, settle::Phase};
use bevy::prelude::*;
use bevy::sprite::{BorderRect, TextureSlicer};
use bevy::ui::widget::NodeImageMode;
use std::collections::BTreeMap;
use wow_world::inventory::Inventory;
pub(crate) const IVORY: Color = Color::srgb(0.88, 0.85, 0.77);
pub(crate) const GOLD: Color = Color::srgb(0.49, 0.39, 0.22);
pub(crate) const MUTED: Color = Color::srgb(0.67, 0.65, 0.58);
/// A step up and a step down against what is worn (the quest screens' card uses the same two).
const GREEN: Color = Color::srgb(0.55, 0.85, 0.45);
const RED: Color = Color::srgb(0.92, 0.45, 0.4);
const ROWS: usize = 7;
/// Comparison rows the detail panel has room for (a shield compares armor and block, a two-hander adds
/// what it takes off).
const COMPARE_LINES: usize = 3;
#[derive(Resource)]
pub(crate) struct Fonts {
    pub(crate) body: Handle<Font>,
    pub(crate) title: Handle<Font>,
}
#[derive(Resource)]
pub(crate) struct Art {
    pub(crate) icons: BTreeMap<u32, Handle<Image>>,
}
#[derive(Component)]
enum Label {
    Prompt,
    Error,
    Title,
    DetailName,
    Detail,
    Hint,
    /// The use button's word: Use item, or Equip on a wearable.
    UseButton,
    PouchCount,
    NoticeName(usize),
    NoticeCount(usize),
    NoticeFoot,
    Status,
    RowName(usize),
    RowCount(usize),
    RowEffect(usize),
    /// Better / Worse / Equal / Not yet: a gear row against what is worn in its slot.
    RowTrend(usize),
    /// Why the selected gear cannot be worn yet.
    Warning,
    /// "Compared with Worn Shortsword (Main Hand)".
    Versus,
    /// One row of the comparison, green for a gain, red for a loss.
    Compare(usize),
    /// The piece worn in that slot now: its name, then its tooltip (WoW's "Currently Equipped").
    WornName,
    WornTooltip,
}
#[derive(Component)]
enum Panel {
    Prompt,
    Notice,
    Inventory,
    Error,
    Pouch,
}
#[derive(Component)]
enum Control {
    Row(usize),
    Use,
    Close,
}
#[derive(Component)]
enum Icon {
    Row(usize),
    Detail,
    Notice(usize),
    Pouch,
}
#[derive(Component)]
struct NoticeRow(usize);
pub fn ordered(items: &Inventory) -> Vec<u32> {
    let mut ids = items.recent.clone();
    ids.extend(items.items.keys().filter(|id| !items.recent.contains(id)));
    ids
}
pub fn plugin(app: &mut App) {
    app.add_systems(Startup, fonts)
        .add_systems(OnEnter(Phase::Playing), spawn)
        .add_systems(
            Update,
            buttons
                .before(crate::combat::act)
                .run_if(in_state(Phase::Playing)),
        )
        .add_systems(
            Update,
            (notices, refresh)
                .chain()
                .after(super::drive)
                .run_if(in_state(Phase::Playing)),
        );
}
fn fonts(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let load = |name: &str, fonts: &mut Assets<Font>| {
        std::fs::read(format!("C:/Windows/Fonts/{name}"))
            .ok()
            .and_then(|b| Font::try_from_bytes(b).ok())
            .map(|f| fonts.add(f))
            .unwrap_or_default()
    };
    commands.insert_resource(Fonts {
        body: load("georgia.ttf", &mut fonts),
        title: load("georgia.ttf", &mut fonts),
    });
}
fn text(
    commands: &mut Commands,
    parent: Entity,
    value: &str,
    font: &Handle<Font>,
    size: f32,
    color: Color,
    label: Option<Label>,
) -> Entity {
    let mut e = commands.spawn((
        Text::new(value),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        ChildOf(parent),
    ));
    if let Some(label) = label {
        e.insert(label);
    }
    e.id()
}
/// Text without a label, for the pages other modules draw in the same style.
pub(crate) fn plain(
    commands: &mut Commands,
    parent: Entity,
    value: &str,
    font: &Handle<Font>,
    size: f32,
    color: Color,
) -> Entity {
    text(commands, parent, value, font, size, color, None)
}
pub(crate) fn container(commands: &mut Commands, parent: Entity, node: Node) -> Entity {
    commands.spawn((node, ChildOf(parent))).id()
}
pub(crate) fn frame(
    commands: &mut Commands,
    node: Node,
    image: &Handle<Image>,
    parent: Option<Entity>,
) -> Entity {
    let mut image = ImageNode::new(image.clone());
    image.image_mode = NodeImageMode::Sliced(TextureSlicer {
        border: BorderRect::all(18.0),
        ..default()
    });
    let mut e = commands.spawn((node, image));
    if let Some(parent) = parent {
        e.insert(ChildOf(parent));
    }
    e.id()
}
fn icon(commands: &mut Commands, parent: Entity, size: f32, kind: Icon) -> Entity {
    commands
        .spawn((
            Node {
                width: px(size),
                height: px(size),
                min_width: px(size),
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.31, 0.27, 0.19)),
            ImageNode::default(),
            kind,
            ChildOf(parent),
        ))
        .id()
}
pub(crate) fn load_icon(assets: &AssetServer, path: &str) -> Handle<Image> {
    assets.load_with_settings(
        benilla_assets::texture_url(&format!("{path}.blp"), (false, false)),
        |s: &mut benilla_assets::BlpLoaderSettings| s.variant = benilla_assets::BlpVariant::Sprite,
    )
}
#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    fonts: Res<Fonts>,
    assets: Res<AssetServer>,
    data: Res<Data>,
    catalogs: Res<Catalogs>,
    hud: Option<Res<crate::hud::art::HudArt>>,
) {
    let fallback = load_icon(&assets, "Interface/Icons/INV_Misc_QuestionMark");
    let icons = data
        .0
        .items
        .iter()
        .map(|item| {
            let h = catalogs
                .items
                .get(item.display)
                .and_then(|d| d.icon.as_deref())
                .map_or_else(|| fallback.clone(), |p| load_icon(&assets, p));
            (item.entry, h)
        })
        .chain(std::iter::once((
            SALVE,
            load_icon(&assets, "Interface/Icons/INV_Potion_49"),
        )))
        .collect();
    commands.insert_resource(Art { icons });
    let leather = hud
        .as_deref()
        .map(|h| h.sprite_image("soul_param/bg_parchment_0"))
        .unwrap_or_else(|| load_icon(&assets, "Interface/DialogFrame/UI-DialogBox-Background"));
    let prompt = frame(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            left: percent(32),
            right: percent(32),
            bottom: percent(10),
            padding: UiRect::axes(px(20), px(10)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        &leather,
        None,
    );
    commands
        .entity(prompt)
        .insert((Panel::Prompt, Visibility::Hidden));
    text(
        &mut commands,
        prompt,
        "",
        &fonts.body,
        18.0,
        IVORY,
        Some(Label::Prompt),
    );
    let notice = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(26),
                right: percent(26),
                bottom: percent(24),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                ..default()
            },
            Panel::Notice,
            Visibility::Hidden,
        ))
        .id();
    for index in 0..4 {
        let row = frame(
            &mut commands,
            Node {
                height: px(78),
                width: percent(100),
                padding: UiRect::axes(px(18), px(10)),
                align_items: AlignItems::Center,
                column_gap: px(16),
                ..default()
            },
            &leather,
            Some(notice),
        );
        commands.entity(row).insert(NoticeRow(index));
        icon(&mut commands, row, 54.0, Icon::Notice(index));
        let name = text(
            &mut commands,
            row,
            "",
            &fonts.body,
            22.0,
            IVORY,
            Some(Label::NoticeName(index)),
        );
        commands.entity(name).insert(Node {
            flex_grow: 1.0,
            min_width: px(0),
            ..default()
        });
        let count = text(
            &mut commands,
            row,
            "",
            &fonts.body,
            22.0,
            IVORY,
            Some(Label::NoticeCount(index)),
        );
        commands.entity(count).insert(Node {
            min_width: px(28),
            margin: UiRect::right(px(8)),
            ..default()
        });
    }
    text(
        &mut commands,
        notice,
        "",
        &fonts.body,
        15.0,
        IVORY,
        Some(Label::NoticeFoot),
    );
    text(
        &mut commands,
        notice,
        "",
        &fonts.body,
        18.0,
        IVORY,
        Some(Label::Status),
    );
    let error = frame(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            left: percent(20),
            right: percent(20),
            top: px(110),
            padding: UiRect::all(px(15)),
            ..default()
        },
        &leather,
        None,
    );
    commands
        .entity(error)
        .insert((Panel::Error, Visibility::Hidden, GlobalZIndex(50)));
    text(
        &mut commands,
        error,
        "",
        &fonts.body,
        18.0,
        Color::srgb(0.94, 0.7, 0.54),
        Some(Label::Error),
    );
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.60)),
        Panel::Inventory,
        Visibility::Hidden,
        GlobalZIndex(30),
    ));
    let inventory = frame(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            left: percent(7),
            right: percent(7),
            top: percent(6),
            bottom: percent(14),
            padding: UiRect::all(px(22)),
            flex_direction: FlexDirection::Column,
            row_gap: px(12),
            ..default()
        },
        &leather,
        None,
    );
    commands
        .entity(inventory)
        .insert((Panel::Inventory, Visibility::Hidden, GlobalZIndex(40)));
    // Keep the native ragged texture edge, with a dark inset for readable menu text.
    let backing = container(
        &mut commands,
        inventory,
        Node {
            position_type: PositionType::Absolute,
            left: px(18),
            right: px(18),
            top: px(18),
            bottom: px(18),
            ..default()
        },
    );
    commands
        .entity(backing)
        .insert(BackgroundColor(Color::srgba(0.015, 0.012, 0.01, 0.40)));
    let heading = container(
        &mut commands,
        inventory,
        Node {
            width: percent(100),
            padding: UiRect::bottom(px(9)),
            border: UiRect::bottom(px(1)),
            ..default()
        },
    );
    commands.entity(heading).insert(BorderColor::all(GOLD));
    text(
        &mut commands,
        heading,
        "Inventory",
        &fonts.title,
        26.0,
        IVORY,
        Some(Label::Title),
    );
    let body = container(
        &mut commands,
        inventory,
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(22),
            flex_grow: 1.0,
            min_height: px(0),
            ..default()
        },
    );
    let list = container(
        &mut commands,
        body,
        Node {
            width: percent(60),
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
                height: px(48),
                min_height: px(48),
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
            Control::Row(index),
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::srgba(0.36, 0.29, 0.17, 0.55)),
        ));
        icon(&mut commands, row, 42.0, Icon::Row(index));
        let copy = container(
            &mut commands,
            row,
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: px(0),
                ..default()
            },
        );
        text(
            &mut commands,
            copy,
            "",
            &fonts.body,
            18.0,
            IVORY,
            Some(Label::RowName(index)),
        );
        text(
            &mut commands,
            copy,
            "",
            &fonts.body,
            13.0,
            MUTED,
            Some(Label::RowEffect(index)),
        );
        let trend = text(
            &mut commands,
            row,
            "",
            &fonts.body,
            14.0,
            MUTED,
            Some(Label::RowTrend(index)),
        );
        commands.entity(trend).insert(Node {
            min_width: px(58),
            margin: UiRect::right(px(6)),
            justify_content: JustifyContent::End,
            ..default()
        });
        let count = text(
            &mut commands,
            row,
            "",
            &fonts.body,
            18.0,
            IVORY,
            Some(Label::RowCount(index)),
        );
        commands.entity(count).insert(Node {
            min_width: px(28),
            ..default()
        });
    }
    let detail = container(
        &mut commands,
        body,
        Node {
            width: percent(38),
            border: UiRect::all(px(1)),
            padding: UiRect::all(px(16)),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        },
    );
    commands.entity(detail).insert((
        BorderColor::all(GOLD),
        BackgroundColor(Color::srgba(0.02, 0.018, 0.014, 0.35)),
    ));
    text(
        &mut commands,
        detail,
        "",
        &fonts.title,
        22.0,
        IVORY,
        Some(Label::DetailName),
    );
    icon(&mut commands, detail, 56.0, Icon::Detail);
    text(
        &mut commands,
        detail,
        "",
        &fonts.body,
        16.0,
        IVORY,
        Some(Label::Detail),
    );
    // The comparison with what is worn; each line takes no room while it is empty.
    let compare_row = |commands: &mut Commands, size: f32, color: Color, label: Label| {
        let line = text(commands, detail, "", &fonts.body, size, color, Some(label));
        commands.entity(line).insert(Node {
            display: Display::None,
            ..default()
        });
    };
    compare_row(&mut commands, 15.0, RED, Label::Warning);
    compare_row(&mut commands, 14.0, MUTED, Label::Versus);
    for i in 0..COMPARE_LINES {
        compare_row(&mut commands, 16.0, IVORY, Label::Compare(i));
    }
    compare_row(&mut commands, 16.0, IVORY, Label::WornName);
    compare_row(&mut commands, 14.0, MUTED, Label::WornTooltip);
    let use_button = container(
        &mut commands,
        detail,
        Node {
            width: percent(100),
            padding: UiRect::axes(px(12), px(7)),
            justify_content: JustifyContent::Center,
            margin: UiRect::top(px(3)),
            ..default()
        },
    );
    commands.entity(use_button).insert((
        Button,
        Control::Use,
        BackgroundColor(Color::srgb(0.34, 0.15, 0.055)),
    ));
    text(
        &mut commands,
        use_button,
        "Use item",
        &fonts.body,
        17.0,
        IVORY,
        Some(Label::UseButton),
    );
    let footer = container(
        &mut commands,
        inventory,
        Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            border: UiRect::top(px(1)),
            padding: UiRect::top(px(10)),
            column_gap: px(12),
            ..default()
        },
    );
    commands.entity(footer).insert(BorderColor::all(GOLD));
    text(
        &mut commands,
        footer,
        "W / S / D-pad: Select",
        &fonts.body,
        15.0,
        IVORY,
        Some(Label::Hint),
    );
    let close = container(
        &mut commands,
        footer,
        Node {
            padding: UiRect::axes(px(9), px(4)),
            ..default()
        },
    );
    commands
        .entity(close)
        .insert((Button, Control::Close, BackgroundColor(Color::NONE)));
    text(
        &mut commands,
        close,
        "Tab / B: Close",
        &fonts.body,
        15.0,
        IVORY,
        None,
    );
    let pouch = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(5),
                bottom: percent(19),
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                ..default()
            },
            Panel::Pouch,
        ))
        .id();
    let slot = container(
        &mut commands,
        pouch,
        Node {
            width: px(68),
            height: px(68),
            ..default()
        },
    );
    icon(&mut commands, slot, 64.0, Icon::Pouch);
    let count = text(
        &mut commands,
        slot,
        "",
        &fonts.body,
        22.0,
        IVORY,
        Some(Label::PouchCount),
    );
    commands.entity(count).insert(Node {
        position_type: PositionType::Absolute,
        bottom: px(2),
        right: px(4),
        ..default()
    });
    text(
        &mut commands,
        pouch,
        "V / X: Use\nTab / Start: Inventory",
        &fonts.body,
        14.0,
        IVORY,
        None,
    );
}
fn notices(
    mut events: MessageReader<Acquired>,
    data: Res<Data>,
    time: Res<Time<bevy::time::Real>>,
    mut screen: ResMut<Screen>,
    mut audio: Option<ResMut<crate::audio::Audio>>,
) {
    for e in events.read() {
        // WoW's own pickup sound, once per grant.
        if !e.items.is_empty() {
            if let Some(audio) = audio.as_deref_mut() {
                audio.play_path("Sound\\interface\\UiLootPickUpItem.wav", 0.8, "item pickup");
            }
        }
        screen.notice.clear();
        screen.notice_items = e.items.clone();
        screen.notice_item = e.items.first().map(|i| i.0);
        screen.discovery = e.discovery;
        screen.notice_detail = if e.discovery {
            e.items
                .first()
                .map(|i| super::item_detail(&data, i.0))
                .unwrap_or_default()
        } else {
            String::new()
        };
        if e.items.is_empty() {
            screen.notice = "Empty".into();
        }
        screen.notice_until = time.elapsed_secs() + if e.discovery { 6.0 } else { 3.0 };
    }
}
fn buttons(
    mut screen: ResMut<Screen>,
    owned: Res<Owned>,
    buttons: Query<(&Interaction, &Control), Changed<Interaction>>,
) {
    if !screen.open {
        return;
    }
    let ids = ordered(&owned.progress.inventory);
    let offset = screen.selected.saturating_sub(ROWS - 1);
    for (interaction, control) in &buttons {
        if *interaction == Interaction::Pressed {
            match *control {
                Control::Row(row) if offset + row < ids.len() => screen.selected = offset + row,
                Control::Use => screen.use_requested = true,
                Control::Close => screen.close_requested = true,
                _ => {}
            }
        }
    }
}
/// A gear row's mark against the piece worn in its slot: Better / Worse / Equal, or in red what stops
/// the wearer ("Level 4" when the level does, else "Not yet"). Nothing for what is not gear.
fn mark(card: &crate::equip::Card) -> Option<(String, Color)> {
    use crate::equip::Trend;
    if let Some(why) = &card.warning {
        // "You must reach level 4 to use that item."
        let level = why
            .split("level ")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|n| n.parse::<u32>().ok());
        return Some((
            level.map_or("Not yet".into(), |l| format!("Level {l}")),
            RED,
        ));
    }
    crate::equip::verdict(card).map(|t| match t {
        Trend::Better => ("Better".to_string(), GREEN),
        Trend::Worse => ("Worse".to_string(), RED),
        Trend::Same => ("Equal".to_string(), MUTED),
    })
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh(
    screen: Res<Screen>,
    owned: Res<Owned>,
    data: Res<Data>,
    art: Res<Art>,
    time: Res<Time<bevy::time::Real>>,
    gear: Res<crate::equip::GearScreen>,
    game: Option<Res<crate::quests::QuestGame>>,
    mut panels: Query<(&Panel, &mut Visibility), Without<Control>>,
    mut labels: Query<(&Label, &mut Text, &mut TextColor, &mut Node), Without<NoticeRow>>,
    mut icons: Query<(&Icon, &mut ImageNode, &mut Visibility), (Without<Panel>, Without<Control>)>,
    mut notice_rows: Query<(&NoticeRow, &mut Node), Without<Label>>,
    mut controls: Query<
        (&Control, &mut BackgroundColor, &mut Visibility),
        (Without<Panel>, Without<Icon>),
    >,
) {
    for (panel, mut visibility) in &mut panels {
        let show = match panel {
            Panel::Inventory => screen.open && screen.page == crate::equip::Page::Items,
            Panel::Notice => !screen.open && time.elapsed_secs() < screen.notice_until,
            Panel::Prompt => !screen.open && !screen.prompt.is_empty(),
            Panel::Error => screen.error.is_some() || owned.blocked.is_some(),
            // Not under a quest screen: the pouch sits where its text would be.
            Panel::Pouch => {
                !screen.open && !screen.modal && owned.progress.inventory.count(SALVE) > 0
            }
        };
        let v = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != v {
            *visibility = v;
        }
    }
    if !screen.is_changed() && !owned.is_changed() && !gear.is_changed() {
        return;
    }
    let ids = ordered(&owned.progress.inventory);
    let selected = ids.get(screen.selected).copied();
    let wearable = selected
        .and_then(|id| data.0.items.iter().find(|i| i.entry == id))
        .is_some_and(|i| !wow_world::equipment::slots_for(i.inventory_type).is_empty());
    let offset = screen.selected.saturating_sub(ROWS - 1);
    // Gear is held up against what is worn in its slot, as the quest rewards and the shop do.
    let wearer = crate::equip::wearer(game.as_deref());
    let card_of = |id: u32| crate::equip::card(&data.0.items, &owned.progress.gear, wearer, id);
    let selected_card = selected.and_then(&card_of);
    let row_trend = |id: u32| card_of(id).as_ref().and_then(mark);
    for (row, mut node) in &mut notice_rows {
        let display = if row.0 < screen.notice_items.len() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    for (label, mut text, mut color, mut node) in &mut labels {
        let value = match *label {
            Label::Prompt => screen.prompt.clone(),
            Label::Error => screen
                .error
                .as_ref()
                .or(owned.blocked.as_ref())
                .cloned()
                .unwrap_or_default(),
            Label::Title => "Inventory".into(),
            Label::DetailName => selected
                .map(|id| super::item_name(&data, id))
                .unwrap_or_else(|| "Items".into()),
            Label::Detail => selected
                .map(|id| {
                    // Gear shows WoW's tooltip (damage, armor, the level it asks); the rest its word.
                    let about = match (&selected_card, wearable) {
                        (Some(card), true) => card.tooltip.clone(),
                        _ => format!("\n{}", super::item_detail(&data, id)),
                    };
                    format!(
                        "Carried     {}\n{about}",
                        owned.progress.inventory.count(id),
                    )
                })
                .unwrap_or_else(|| {
                    "No items carried.\n\nExplore Northshire to find supplies and hidden treasures."
                        .into()
                }),
            Label::Hint => {
                if !gear.message.is_empty() {
                    gear.message.clone()
                } else if selected == Some(SALVE) {
                    "W / S / D-pad: Select    Enter / A: Use    G: Equipment".into()
                } else if wearable {
                    "W / S / D-pad: Select    Enter / A: Equip    G: Equipment".into()
                } else {
                    "W / S / D-pad: Select    G: Equipment".into()
                }
            }
            Label::UseButton => if wearable { "Equip" } else { "Use item" }.into(),
            Label::PouchCount => owned.progress.inventory.count(SALVE).to_string(),
            Label::NoticeName(row) => screen
                .notice_items
                .get(row)
                .map(|i| super::item_name(&data, i.0))
                .unwrap_or_default(),
            Label::NoticeCount(row) => screen
                .notice_items
                .get(row)
                .map(|i| i.1.to_string())
                .unwrap_or_default(),
            Label::NoticeFoot => {
                if screen.notice_items.len() > 4 {
                    format!(
                        "{} more items · Tab / Start: Inventory",
                        screen.notice_items.len() - 4
                    )
                } else {
                    screen.notice_detail.clone()
                }
            }
            Label::Status => screen.notice.clone(),
            Label::RowName(row) => ids
                .get(offset + row)
                .map(|&id| {
                    format!(
                        "{}{}",
                        if offset + row == screen.selected {
                            "› "
                        } else {
                            ""
                        },
                        super::item_name(&data, id)
                    )
                })
                .unwrap_or_default(),
            Label::RowCount(row) => ids
                .get(offset + row)
                .map(|&id| owned.progress.inventory.count(id).to_string())
                .unwrap_or_default(),
            Label::RowEffect(row) => ids
                .get(offset + row)
                .map(|&id| {
                    if id == SALVE {
                        "Restores 30 health".into()
                    } else {
                        super::item_detail(&data, id)
                    }
                })
                .unwrap_or_default(),
            Label::RowTrend(row) => ids
                .get(offset + row)
                .and_then(|&id| row_trend(id))
                .map_or(String::new(), |(word, _)| word),
            Label::Warning => selected_card
                .as_ref()
                .and_then(|c| c.warning.clone())
                .unwrap_or_default(),
            Label::Versus => selected_card
                .as_ref()
                .map_or(String::new(), |c| c.versus.clone()),
            Label::Compare(i) => selected_card
                .as_ref()
                .and_then(|c| c.compare.get(i))
                .map_or(String::new(), |l| l.text.clone()),
            Label::WornName => selected_card
                .as_ref()
                .and_then(|c| c.worn.as_ref())
                .map_or(String::new(), |w| w.name.clone()),
            Label::WornTooltip => selected_card
                .as_ref()
                .and_then(|c| c.worn.as_ref())
                .map_or(String::new(), |w| w.tooltip.clone()),
        };
        // The colour of what compares: gains green, losses red, the worn piece in its quality.
        let tint = match *label {
            Label::RowTrend(row) => ids
                .get(offset + row)
                .and_then(|&id| row_trend(id))
                .map(|(_, c)| c),
            Label::Compare(i) => Some(
                match selected_card
                    .as_ref()
                    .and_then(|c| c.compare.get(i))
                    .map(|l| l.trend)
                {
                    Some(crate::equip::Trend::Better) => GREEN,
                    Some(crate::equip::Trend::Worse) => RED,
                    _ => MUTED,
                },
            ),
            Label::WornName => selected_card
                .as_ref()
                .and_then(|c| c.worn.as_ref())
                .map(|w| crate::equip::quality_color(w.quality)),
            _ => None,
        };
        if let Some(tint) = tint {
            if color.0 != tint {
                color.0 = tint;
            }
        }
        // A comparison line that says nothing takes no room.
        if matches!(
            *label,
            Label::Warning
                | Label::Versus
                | Label::Compare(_)
                | Label::WornName
                | Label::WornTooltip
        ) {
            let want = if value.is_empty() {
                Display::None
            } else {
                Display::Flex
            };
            if node.display != want {
                node.display = want;
            }
        }
        if text.0 != value {
            text.0 = value;
        }
    }
    for (kind, mut image, mut visibility) in &mut icons {
        let id = match *kind {
            Icon::Row(row) => ids.get(offset + row).copied(),
            Icon::Detail => selected,
            Icon::Notice(row) => screen.notice_items.get(row).map(|i| i.0),
            Icon::Pouch => (owned.progress.inventory.count(SALVE) > 0).then_some(SALVE),
        };
        let v = if id.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != v {
            *visibility = v;
        }
        if let Some(h) = id.and_then(|id| art.icons.get(&id)) {
            if image.image != *h {
                image.image = h.clone();
            }
        }
    }
    for (control, mut background, mut visibility) in &mut controls {
        let (visible, focused) = match *control {
            Control::Row(row) => (offset + row < ids.len(), offset + row == screen.selected),
            Control::Use => (selected == Some(SALVE) || wearable, false),
            Control::Close => (true, false),
        };
        let v = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != v {
            *visibility = v;
        }
        let color = if focused || matches!(control, Control::Use) {
            Color::srgba(0.46, 0.21, 0.07, 0.65)
        } else {
            Color::NONE
        };
        if background.0 != color {
            background.0 = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equip::{Card, CompareLine, Trend};

    fn card(warning: Option<&str>, rows: &[Trend]) -> Card {
        Card {
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
        }
    }

    #[test]
    fn a_gear_row_says_better_worse_equal_or_what_stops_the_wearer() {
        let word = |c: Card| mark(&c).map(|(w, _)| w);
        assert_eq!(
            word(card(None, &[Trend::Better])).as_deref(),
            Some("Better")
        );
        assert_eq!(word(card(None, &[Trend::Worse])).as_deref(), Some("Worse"));
        assert_eq!(word(card(None, &[Trend::Same])).as_deref(), Some("Equal"));
        assert_eq!(
            word(card(
                Some("You must reach level 4 to use that item."),
                &[Trend::Better]
            ))
            .as_deref(),
            Some("Level 4"),
            "the level that stops it, not a verdict"
        );
        assert_eq!(
            word(card(Some("You can never use that item."), &[Trend::Better])).as_deref(),
            Some("Not yet")
        );
        assert_eq!(word(card(None, &[])), None, "a potion has no mark");
    }

    #[test]
    fn a_better_row_is_green_and_a_worse_one_red() {
        assert_eq!(mark(&card(None, &[Trend::Better])).unwrap().1, GREEN);
        assert_eq!(mark(&card(None, &[Trend::Worse])).unwrap().1, RED);
        assert_eq!(mark(&card(None, &[Trend::Same])).unwrap().1, MUTED);
    }

    #[test]
    fn installed_fang_and_candle_icons_follow_the_imported_item_displays() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../cache/world/northshire.txt");
        let world =
            wow_world::manifest::parse(&std::fs::read_to_string(manifest).unwrap()).unwrap();
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let catalog = benilla_formats::load_item_display_catalog(&mut chain).unwrap();
        for (id, expected) in [(7073, "INV_Misc_Bone_08"), (755, "INV_Misc_Candle_01")] {
            let item = world.items.iter().find(|i| i.entry == id).unwrap();
            let icon = catalog.get(item.display).unwrap().icon.as_deref().unwrap();
            assert_eq!(icon, format!("Interface\\Icons\\{expected}"));
            let bytes = chain.read_file(&format!("{icon}.blp")).unwrap();
            assert!(bytes.starts_with(b"BLP"));
        }
    }
}
