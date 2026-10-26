//! The rewards of the quest screens: an icon for each reward item, the one in focus marked, and a card
//! for it (WoW's tooltip and its comparison with what the slot holds now) docked on the left. The offer,
//! the hand-in and the quest log all use the same strips and the same card; what is in focus comes from
//! the dialogue ([`super::dialogue::Dialogue::focus`]) or the log ([`super::LogScreen::reward`]), which the
//! D-pad, LB / RB and the arrow keys move.

use bevy::prelude::*;
use bevy::sprite::{BorderRect, TextureSlicer};
use bevy::ui::widget::NodeImageMode;

use super::dialogue::Page;
use super::{LogScreen, QuestGame, Talk};
use crate::equip::{self, Card, Trend};
use crate::loot::ui::Art;
use crate::loot::{Data, Owned};

const CELL: f32 = 50.0;
/// Icons in a row: choices, then the sure rewards (a quest has up to six choices and four of those).
pub const MAX_CHOOSE: usize = 6;
pub const MAX_SURE: usize = 4;

const IVORY: Color = Color::srgb(0.88, 0.85, 0.77);
const GOLD: Color = Color::srgb(0.92, 0.76, 0.28);
const MUTED: Color = Color::srgb(0.67, 0.65, 0.58);
const DIM: Color = Color::srgb(0.31, 0.27, 0.19);
const PICKED: Color = Color::srgb(0.96, 0.96, 0.82);
const GREEN: Color = Color::srgb(0.55, 0.85, 0.45);
const RED: Color = Color::srgb(0.92, 0.45, 0.4);
/// Compare rows the card has room for.
const LINES: usize = 5;

/// Which screen's strips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    Dialogue,
    Log,
}

/// A row of icons: the items to choose one of, or the ones that come with the quest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Choose,
    Sure,
}

/// One icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Pos {
    at: Where,
    group: Group,
    index: usize,
}

/// A node of the rewards: a strip (heading and row), an icon's box, or the card.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ui {
    Strip(Where, Group),
    Cell(Pos),
    Card,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pic {
    Cell(Pos),
    Card,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum Words {
    Head(Where, Group),
    Count(Pos),
    Name,
    Versus,
    Tooltip,
    Warning,
    Line(usize),
}

/// The icons of a screen: a heading and a row for each group.
pub fn spawn_strips(commands: &mut Commands, parent: Entity, at: Where, font: &Handle<Font>) {
    for (group, max) in [(Group::Choose, MAX_CHOOSE), (Group::Sure, MAX_SURE)] {
        let strip = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    margin: UiRect::bottom(px(8)),
                    display: Display::None,
                    ..default()
                },
                Ui::Strip(at, group),
                ChildOf(parent),
            ))
            .id();
        commands.spawn((
            Text::new(""),
            TextFont {
                font: font.clone(),
                font_size: 15.0,
                ..default()
            },
            TextColor(GOLD),
            Words::Head(at, group),
            ChildOf(strip),
        ));
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(6),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
                ChildOf(strip),
            ))
            .id();
        for index in 0..max {
            let pos = Pos { at, group, index };
            let cell = commands
                .spawn((
                    Node {
                        width: px(CELL),
                        height: px(CELL),
                        border: UiRect::all(px(2)),
                        justify_content: JustifyContent::End,
                        align_items: AlignItems::End,
                        display: Display::None,
                        ..default()
                    },
                    BorderColor::all(DIM),
                    BackgroundColor(Color::srgba(0.02, 0.018, 0.014, 0.55)),
                    Ui::Cell(pos),
                    ChildOf(row),
                ))
                .id();
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                ImageNode::default(),
                Pic::Cell(pos),
                ChildOf(cell),
            ));
            commands.spawn((
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(IVORY),
                Node {
                    margin: UiRect::all(px(2)),
                    ..default()
                },
                Words::Count(pos),
                ChildOf(cell),
            ));
        }
    }
}

/// The card of the reward in focus, docked on the left under the health bars.
pub fn spawn_card(commands: &mut Commands, font: &Handle<Font>, parchment: Option<Handle<Image>>) {
    let node = Node {
        position_type: PositionType::Absolute,
        left: px(36),
        top: px(150),
        width: px(340),
        flex_direction: FlexDirection::Column,
        row_gap: px(6),
        padding: UiRect::all(px(24)),
        display: Display::None,
        ..default()
    };
    let mut panel = commands.spawn((node, Ui::Card));
    match parchment.filter(|_| !crate::hud::theme::enabled()) {
        Some(image) => {
            let mut img = ImageNode::new(image);
            img.image_mode = NodeImageMode::Sliced(TextureSlicer {
                border: BorderRect::all(18.0),
                ..default()
            });
            panel.insert(img);
        }
        None => {
            panel.insert(BackgroundColor(Color::srgba(0.05, 0.04, 0.03, 0.92)));
        }
    }
    if crate::hud::theme::enabled() {
        crate::hud::theme::dress(&mut panel);
    }
    let panel = panel.id();
    let head = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(12),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        Node {
            width: px(56),
            height: px(56),
            min_width: px(56),
            border: UiRect::all(px(1)),
            ..default()
        },
        BorderColor::all(DIM),
        ImageNode::default(),
        Pic::Card,
        ChildOf(head),
    ));
    let text = |commands: &mut Commands, parent: Entity, size: f32, color: Color, kind: Words| {
        commands.spawn((
            Text::new(""),
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            TextColor(color),
            Node {
                flex_shrink: 1.0,
                min_width: px(0),
                ..default()
            },
            kind,
            ChildOf(parent),
        ));
    };
    text(commands, head, 20.0, IVORY, Words::Name);
    text(commands, panel, 15.0, MUTED, Words::Tooltip);
    text(commands, panel, 15.0, RED, Words::Warning);
    text(commands, panel, 14.0, MUTED, Words::Versus);
    for i in 0..LINES {
        text(commands, panel, 16.0, IVORY, Words::Line(i));
    }
}

/// The rewards a screen is showing and the one in focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub at: Where,
    pub choose: Vec<(u32, u32)>,
    pub sure: Vec<(u32, u32)>,
    pub focus: Option<usize>,
    /// The choice in focus is picked and waits for a second press.
    pub armed: bool,
}

impl Shown {
    /// The item in focus: the choices come first.
    pub fn item(&self) -> Option<(u32, u32)> {
        let i = self.focus?;
        if i < self.choose.len() {
            self.choose.get(i).copied()
        } else {
            self.sure.get(i - self.choose.len()).copied()
        }
    }
}

/// What the open screen shows: the offer or the hand-in of the dialogue, else the selected quest of the log.
pub fn shown(game: &QuestGame, talk: &Talk, log: &LogScreen) -> Option<Shown> {
    let (at, quest, focus, armed) = if let Some(d) = talk.0.as_ref() {
        match d.page {
            Page::Detail(q) | Page::Reward(q) => (Where::Dialogue, q, d.focus, d.armed),
            _ => return None,
        }
    } else if log.open {
        let active = game.log.active();
        (Where::Log, *active.get(log.selected)?, log.reward, false)
    } else {
        return None;
    };
    let q = game.book.quest(quest)?;
    let total = q.reward_choice.len() + q.reward_items.len();
    Some(Shown {
        at,
        choose: q.reward_choice.clone(),
        sure: q.reward_items.clone(),
        focus: (total > 0).then(|| focus.min(total - 1)),
        armed,
    })
}

fn heading(group: Group, shown: &Shown) -> &'static str {
    match group {
        Group::Choose if shown.choose.len() >= 2 => "Choose one of these rewards:",
        Group::Choose => "You will receive:",
        Group::Sure if shown.choose.is_empty() => "You will receive:",
        Group::Sure => "You will also receive:",
    }
}

/// Draws the icons and the card.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn refresh(
    game: Option<Res<QuestGame>>,
    talk: Res<Talk>,
    log: Res<LogScreen>,
    owned: Option<Res<Owned>>,
    data: Option<Res<Data>>,
    art: Option<Res<Art>>,
    shop: Res<crate::shop::Shop>,
    spells: Option<Res<crate::abilities::AbilityArt>>,
    mut boxes: Query<(&Ui, &mut Node, Option<&mut BorderColor>), Without<Words>>,
    mut pics: Query<(&Pic, &mut ImageNode)>,
    mut words: Query<(&Words, &mut Text, &mut TextColor, &mut Node), Without<Ui>>,
) {
    let (Some(game), Some(owned), Some(data), Some(art)) = (game, owned, data, art) else {
        return;
    };
    let shown = shown(&game, &talk, &log);
    let items = &data.0.items;
    let wearer = equip::wearer(Some(&game));
    // The card is the shop's too: the highlighted row of an open shop.
    let shop_focus = shop.open.as_ref().and(shop.focus);
    let focus_item = shown.as_ref().and_then(Shown::item).or(match shop_focus {
        Some(crate::shop::Focus::Item(item)) => Some((item, 1)),
        _ => None,
    });
    let focus_spell = match (focus_item, shop_focus) {
        (None, Some(crate::shop::Focus::Spell(s))) => Some(s),
        _ => None,
    };
    let card: Option<Card> = focus_item
        .and_then(|(item, _)| equip::card(items, &owned.progress.gear, wearer, item))
        .or_else(|| focus_spell.and_then(crate::abilities::card));
    let list = |s: &Shown, group: Group| match group {
        Group::Choose => s.choose.clone(),
        Group::Sure => s.sure.clone(),
    };
    let set_display = |node: &mut Node, on: bool| {
        let want = if on { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    };
    let cell_item = |pos: Pos| -> Option<(u32, u32)> {
        let s = shown.as_ref().filter(|s| s.at == pos.at)?;
        list(s, pos.group).get(pos.index).copied()
    };
    let focused = |pos: Pos| -> bool {
        shown.as_ref().is_some_and(|s| {
            s.at == pos.at
                && s.focus.is_some_and(|f| match pos.group {
                    Group::Choose => f == pos.index,
                    Group::Sure => f == s.choose.len() + pos.index,
                })
        })
    };

    for (ui, mut node, border) in &mut boxes {
        match *ui {
            Ui::Strip(at, group) => {
                let on = shown
                    .as_ref()
                    .is_some_and(|s| s.at == at && !list(s, group).is_empty());
                set_display(&mut node, on);
            }
            Ui::Cell(pos) => {
                set_display(&mut node, cell_item(pos).is_some());
                if let Some(mut border) = border {
                    let armed = shown.as_ref().is_some_and(|s| s.armed);
                    let want = match (focused(pos), armed) {
                        (true, true) => PICKED,
                        (true, false) => GOLD,
                        _ => DIM,
                    };
                    if border.top != want {
                        *border = BorderColor::all(want);
                    }
                }
            }
            Ui::Card => set_display(&mut node, card.is_some()),
        }
    }
    for (pic, mut image) in &mut pics {
        let item = match *pic {
            Pic::Cell(pos) => cell_item(pos).map(|(i, _)| i),
            Pic::Card => focus_item.map(|(i, _)| i),
        };
        let handle = item.and_then(|i| art.icons.get(&i)).or_else(|| match *pic {
            Pic::Card => focus_spell.and_then(|s| spells.as_deref().and_then(|a| a.0.get(&s))),
            Pic::Cell(_) => None,
        });
        if let Some(h) = handle {
            if image.image != *h {
                image.image = h.clone();
            }
        }
    }
    for (kind, mut text, mut color, mut node) in &mut words {
        let (value, tint): (String, Color) = match *kind {
            Words::Head(at, group) => (
                shown
                    .as_ref()
                    .filter(|s| s.at == at && !list(s, group).is_empty())
                    .map_or(String::new(), |s| heading(group, s).to_string()),
                GOLD,
            ),
            Words::Count(pos) => (
                cell_item(pos)
                    .filter(|&(_, n)| n > 1)
                    .map_or(String::new(), |(_, n)| n.to_string()),
                IVORY,
            ),
            Words::Name => card.as_ref().map_or((String::new(), IVORY), |c| {
                (c.name.clone(), equip::quality_color(c.quality))
            }),
            Words::Tooltip => (
                card.as_ref().map_or(String::new(), |c| c.tooltip.clone()),
                MUTED,
            ),
            Words::Warning => (
                card.as_ref()
                    .and_then(|c| c.warning.clone())
                    .unwrap_or_default(),
                RED,
            ),
            Words::Versus => (
                card.as_ref().map_or(String::new(), |c| c.versus.clone()),
                MUTED,
            ),
            Words::Line(i) => {
                card.as_ref()
                    .and_then(|c| c.compare.get(i))
                    .map_or((String::new(), IVORY), |l| {
                        (
                            l.text.clone(),
                            match l.trend {
                                Trend::Better => GREEN,
                                Trend::Worse => RED,
                                Trend::Same => MUTED,
                            },
                        )
                    })
            }
        };
        if text.0 != value {
            text.0 = value;
        }
        if color.0 != tint {
            color.0 = tint;
        }
        // An empty line takes no room.
        set_display(&mut node, !text.0.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown_of(choose: &[(u32, u32)], sure: &[(u32, u32)], focus: Option<usize>) -> Shown {
        Shown {
            at: Where::Dialogue,
            choose: choose.to_vec(),
            sure: sure.to_vec(),
            focus,
            armed: false,
        }
    }

    #[test]
    fn the_focus_runs_over_the_choices_and_then_the_sure_rewards() {
        let s = shown_of(&[(10, 1), (11, 1)], &[(20, 3)], Some(0));
        assert_eq!(s.item(), Some((10, 1)));
        let s = shown_of(&[(10, 1), (11, 1)], &[(20, 3)], Some(1));
        assert_eq!(s.item(), Some((11, 1)));
        let s = shown_of(&[(10, 1), (11, 1)], &[(20, 3)], Some(2));
        assert_eq!(s.item(), Some((20, 3)));
        assert_eq!(shown_of(&[], &[(20, 3)], Some(0)).item(), Some((20, 3)));
        assert_eq!(shown_of(&[(10, 1)], &[], None).item(), None);
    }

    #[test]
    fn the_headings_say_choose_only_when_there_is_a_choice() {
        let two = shown_of(&[(1, 1), (2, 1)], &[(3, 1)], Some(0));
        assert_eq!(heading(Group::Choose, &two), "Choose one of these rewards:");
        assert_eq!(heading(Group::Sure, &two), "You will also receive:");
        let one = shown_of(&[(1, 1)], &[], Some(0));
        assert_eq!(heading(Group::Choose, &one), "You will receive:");
        let only_sure = shown_of(&[], &[(3, 1)], Some(0));
        assert_eq!(heading(Group::Sure, &only_sure), "You will receive:");
    }
}
