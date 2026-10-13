//! The panel of the shop, the strongbox and the trainer, docked on the right like the quest dialogue (the
//! card of the highlighted item or ability is the quest screens' card on the left, [`crate::quests`]): the
//! NPC's name and trade, the souls carried (and banked), the tabs, a list of rows with an icon, a name and
//! a price, the last result and the keys.

use bevy::prelude::*;

use super::{current_lines, Focus, Line, Shop, Tab};
use crate::abilities::AbilityArt;
use crate::bloodstain::Banked;
use crate::equip::quality_color;
use crate::hud::souls::Souls;
use crate::loot::ui::{container, frame, load_icon, plain, Art, Fonts, GOLD, IVORY, MUTED};
use crate::loot::{Data, Owned};
use crate::quests::QuestGame;
use crate::settle::Phase;

/// Rows on screen at once.
const ROWS: usize = 9;
const RED: Color = Color::srgb(0.94, 0.45, 0.4);
const GREEN: Color = Color::srgb(0.6, 0.86, 0.5);

#[derive(Component)]
struct Root;

#[derive(Component, Clone, Copy)]
struct Row(usize);

#[derive(Component, Clone, Copy)]
struct Icon(usize);

#[derive(Component, Clone, Copy)]
enum Words {
    Title,
    Subtitle,
    Souls,
    Tabs,
    Message,
    Hint,
    Name(usize),
    Price(usize),
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Phase::Playing), spawn).add_systems(
        Update,
        refresh.after(super::act).run_if(in_state(Phase::Playing)),
    );
}

fn spawn(
    mut commands: Commands,
    fonts: Res<Fonts>,
    assets: Res<AssetServer>,
    hud: Option<Res<crate::hud::art::HudArt>>,
) {
    let leather = hud
        .as_deref()
        .map(|h| h.sprite_image("soul_param/bg_parchment_0"))
        .unwrap_or_else(|| load_icon(&assets, "Interface/DialogFrame/UI-DialogBox-Background"));
    let root = frame(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            right: px(36),
            top: px(64),
            width: px(520),
            padding: UiRect::all(px(24)),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        },
        &leather,
        None,
    );
    commands
        .entity(root)
        .insert((Root, Visibility::Hidden, GlobalZIndex(35)));
    let head = container(
        &mut commands,
        root,
        Node {
            width: percent(100),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::End,
            padding: UiRect::bottom(px(6)),
            border: UiRect::bottom(px(1)),
            ..default()
        },
    );
    commands.entity(head).insert(BorderColor::all(GOLD));
    let title = plain(&mut commands, head, "", &fonts.title, 24.0, GOLD);
    commands.entity(title).insert(Words::Title);
    let souls = plain(&mut commands, head, "", &fonts.body, 18.0, IVORY);
    commands.entity(souls).insert(Words::Souls);
    let sub = plain(&mut commands, root, "", &fonts.body, 14.0, MUTED);
    commands.entity(sub).insert(Words::Subtitle);
    let tabs = plain(&mut commands, root, "", &fonts.body, 18.0, IVORY);
    commands.entity(tabs).insert(Words::Tabs);
    let list = container(
        &mut commands,
        root,
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
                width: percent(100),
                padding: UiRect::axes(px(7), px(2)),
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            },
        );
        commands
            .entity(row)
            .insert((Row(index), BackgroundColor(Color::NONE)));
        commands.spawn((
            Node {
                width: px(32),
                height: px(32),
                min_width: px(32),
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.31, 0.27, 0.19)),
            ImageNode::default(),
            Icon(index),
            ChildOf(row),
        ));
        let name = plain(&mut commands, row, "", &fonts.body, 17.0, IVORY);
        commands.entity(name).insert((
            Words::Name(index),
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                ..default()
            },
        ));
        let price = plain(&mut commands, row, "", &fonts.body, 16.0, GOLD);
        commands.entity(price).insert(Words::Price(index));
    }
    let message = plain(&mut commands, root, "", &fonts.body, 15.0, IVORY);
    commands.entity(message).insert((
        Words::Message,
        Node {
            margin: UiRect::top(px(4)),
            ..default()
        },
    ));
    let hint = plain(&mut commands, root, "", &fonts.body, 13.0, MUTED);
    commands.entity(hint).insert(Words::Hint);
}

/// The first row on screen so the cursor's row is.
fn offset(cursor: usize) -> usize {
    (cursor + 1).saturating_sub(ROWS)
}

/// What a row shows on the right: the price text and its colour.
fn price_text(tab: Tab, line: &Line) -> (String, Color) {
    match tab {
        Tab::Buy => (
            format!("{} souls", line.price),
            if line.note.is_empty() { GOLD } else { RED },
        ),
        Tab::Sell if line.count == 0 => (line.note.clone(), RED),
        Tab::Sell => (format!("+{} souls", line.price), GREEN),
        Tab::Bank if line.price > 0 => (format!("tithe {}", line.price), MUTED),
        Tab::Bank => ("free".to_string(), GREEN),
        Tab::Train if line.note == "Known" => (line.note.clone(), MUTED),
        Tab::Train if line.note.is_empty() => (format!("{} souls", line.price), GOLD),
        Tab::Train => (line.note.clone(), RED),
    }
}

/// The tab strip: the tabs of the screen with the one that is up in brackets.
fn tabs_text(tabs: &[Tab], at: usize) -> String {
    tabs.iter()
        .enumerate()
        .map(|(i, t)| {
            if i == at {
                format!("[ {} ]", t.name())
            } else {
                format!("  {}  ", t.name())
            }
        })
        .collect::<Vec<_>>()
        .join("    ")
}

fn hint_of(tabs: &[Tab], tab: Tab) -> &'static str {
    match (tabs.len() > 1, tab) {
        (true, _) => "Up/Down  Choose      Left/Right  Tab      Enter / A  Do it      B  Leave",
        (false, Tab::Train) => "Up/Down  Choose      Enter / A  Learn      B  Leave",
        (false, _) => "Up/Down  Choose      Enter / A  Do it      B  Leave",
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh(
    mut shop: ResMut<Shop>,
    data: Res<Data>,
    owned: Res<Owned>,
    souls: Res<Souls>,
    bank: Res<Banked>,
    game: Option<Res<QuestGame>>,
    art: Option<Res<Art>>,
    spells: Option<Res<AbilityArt>>,
    mut root: Query<&mut Visibility, With<Root>>,
    mut rows: Query<(&Row, &mut Node, &mut BackgroundColor)>,
    mut icons: Query<(&Icon, &mut ImageNode, &mut Node), Without<Row>>,
    mut words: Query<(&Words, &mut Text, &mut TextColor), (Without<Row>, Without<Icon>)>,
) {
    let want = if shop.open.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if let Ok(mut v) = root.single_mut() {
        if *v != want {
            *v = want;
        }
    }
    let Some(open) = shop.open.as_ref() else {
        if shop.focus.is_some() {
            shop.focus = None;
        }
        return;
    };
    let lines = current_lines(open, &data, &owned, souls.0, bank.0, game.as_deref());
    let cursor = open.row().min(lines.len().saturating_sub(1));
    let first = offset(cursor);
    let focus = lines.get(cursor).and_then(|l| {
        if l.item != 0 {
            Some(Focus::Item(l.item))
        } else if l.spell != 0 {
            Some(Focus::Spell(l.spell))
        } else {
            None
        }
    });
    let (tab, tabs, at, title, subtitle, message, bad) = (
        open.tab(),
        open.tabs.clone(),
        open.at,
        open.name.clone(),
        open.subtitle.clone(),
        open.message.clone(),
        open.bad,
    );
    if shop.focus != focus {
        shop.focus = focus;
    }
    for (row, mut node, mut background) in &mut rows {
        let live = first + row.0 < lines.len();
        let display = if live { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        let color = if live && first + row.0 == cursor {
            Color::srgba(0.46, 0.21, 0.07, 0.65)
        } else {
            Color::NONE
        };
        if background.0 != color {
            background.0 = color;
        }
    }
    for (icon, mut image, mut node) in &mut icons {
        let line = lines.get(first + icon.0);
        let handle = line.and_then(|l| {
            if l.item != 0 {
                art.as_deref().and_then(|a| a.icons.get(&l.item))
            } else if l.spell != 0 {
                spells.as_deref().and_then(|a| a.0.get(&l.spell))
            } else {
                None
            }
        });
        let display = if handle.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
        if let Some(h) = handle {
            if image.image != *h {
                image.image = h.clone();
            }
        }
    }
    for (kind, mut text, mut color) in &mut words {
        let (value, tint): (String, Color) = match *kind {
            Words::Title => (title.clone(), GOLD),
            Words::Subtitle => (subtitle.clone(), MUTED),
            Words::Souls => (
                if tab == Tab::Bank {
                    format!("Souls  {}      Strongbox  {}", souls.0, bank.0)
                } else {
                    format!("Souls  {}", souls.0)
                },
                IVORY,
            ),
            Words::Tabs => (tabs_text(&tabs, at), IVORY),
            Words::Message => (message.clone(), if bad { RED } else { IVORY }),
            Words::Hint => (hint_of(&tabs, tab).to_string(), MUTED),
            Words::Name(i) => match lines.get(first + i) {
                Some(l) if tab == Tab::Sell && l.count > 1 => {
                    (format!("{} x{}", l.name, l.count), quality_color(l.quality))
                }
                Some(l) => (l.name.clone(), quality_color(l.quality)),
                None => (String::new(), IVORY),
            },
            Words::Price(i) => lines
                .get(first + i)
                .map_or((String::new(), GOLD), |l| price_text(tab, l)),
        };
        if text.0 != value {
            text.0 = value;
        }
        if color.0 != tint {
            color.0 = tint;
        }
    }
    if lines.is_empty() {
        // An empty list says why.
        for (kind, mut text, _) in &mut words {
            if let Words::Name(0) = *kind {
                text.0 = match tab {
                    Tab::Buy => "Nothing for sale".into(),
                    Tab::Sell => "You carry nothing to sell".into(),
                    Tab::Bank => "Nothing to put in or take out".into(),
                    Tab::Train => "Nothing to teach you".into(),
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(price: u32, count: u32, note: &str) -> Line {
        Line {
            name: "X".into(),
            quality: 1,
            price,
            count,
            note: note.into(),
            ..Line::default()
        }
    }

    #[test]
    fn the_list_scrolls_to_keep_the_cursor_in_view() {
        assert_eq!(offset(0), 0);
        assert_eq!(offset(ROWS - 1), 0);
        assert_eq!(offset(ROWS), 1);
        assert_eq!(offset(ROWS + 4), 5);
    }

    #[test]
    fn a_row_prices_in_souls_and_a_sale_pays_with_a_plus() {
        assert_eq!(price_text(Tab::Buy, &line(240, 0, "")).0, "240 souls");
        assert_eq!(price_text(Tab::Buy, &line(240, 0, "")).1, GOLD);
        assert_eq!(
            price_text(Tab::Buy, &line(240, 0, "Not enough souls")).1,
            RED
        );
        assert_eq!(price_text(Tab::Sell, &line(36, 3, "")).0, "+36 souls");
        assert_eq!(
            price_text(Tab::Sell, &line(0, 0, "Needed for a quest")).0,
            "Needed for a quest"
        );
    }

    #[test]
    fn the_strongbox_and_the_trainer_price_in_their_own_words() {
        assert_eq!(price_text(Tab::Bank, &line(100, 0, "")).0, "tithe 100");
        assert_eq!(price_text(Tab::Bank, &line(0, 0, "")).0, "free");
        assert_eq!(price_text(Tab::Train, &line(40, 0, "")).0, "40 souls");
        assert_eq!(price_text(Tab::Train, &line(40, 0, "Known")).0, "Known");
        assert_eq!(price_text(Tab::Train, &line(40, 0, "Level 4")).1, RED);
    }

    #[test]
    fn the_tab_strip_brackets_the_one_that_is_up() {
        assert_eq!(tabs_text(&[Tab::Buy, Tab::Sell], 1), "  Buy      [ Sell ]");
        assert_eq!(tabs_text(&[Tab::Bank], 0), "[ Strongbox ]");
    }
}
