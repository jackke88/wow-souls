//! Character selection: a bounded roster, live saved profile, separate Play action and 3D preview.
use super::{MainRow, Menu, MenuPress, RowButton, Screen, SlotInfo};
use crate::hud::theme::{self, Surface, GOLD, MUTED};
use crate::settle::Phase;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use std::collections::BTreeMap;

const VISIBLE: usize = 6;
#[derive(Default)]
pub(crate) struct Summary {
    pub xp: Option<u32>,
    pub progress: Option<wow_world::inventory::Progress>,
}
#[derive(Resource, Default)]
pub(crate) struct Summaries(pub BTreeMap<u32, Summary>);
#[derive(Component)]
struct Root;
#[derive(Component)]
pub(super) struct RosterRow(usize);
#[derive(Component)]
pub(super) struct Command(MainRow);
#[derive(Component)]
struct Play;
#[derive(Component, Clone, Copy)]
enum Words {
    Name,
    Identity,
    Details,
    Count,
    Message,
    Play,
    RowName(usize),
    RowInfo(usize),
}
#[derive(Component)]
struct Glyph(usize);

pub(super) fn window(cursor: usize, count: usize) -> usize {
    cursor
        .min(count.saturating_sub(1))
        .saturating_sub(VISIBLE - 1)
}

fn summary(slot: &SlotInfo, active: u32) -> Summary {
    let dir = crate::saves::slot_dir(&crate::saves::root(), slot.id);
    let path = |env: &str, name: &str| {
        if slot.id == active {
            std::env::var_os(env)
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| dir.join(name))
        } else {
            dir.join(name)
        }
    };
    let quest = path("WOWSOULS_QUESTS_SAVE", "quests-v1.txt");
    let xp = match std::fs::read_to_string(quest) {
        Ok(text) => wow_world::quest::save::SaveGame::from_text(&text)
            .ok()
            .map(|s| s.xp),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(0),
        Err(_) => None,
    };
    let progress =
        wow_world::inventory::Progress::load(&path("WOWSOULS_ITEMS_SAVE", "items-v1.txt")).ok();
    Summary { xp, progress }
}

fn text(
    commands: &mut Commands,
    parent: Entity,
    font: &Handle<Font>,
    size: f32,
    label: Words,
    value: &str,
) -> Entity {
    let id = theme::words(commands, parent, value, font, size, Node::default());
    commands.entity(id).insert(label);
    id
}

pub(super) fn spawn(commands: &mut Commands, parent: Entity, font: &Handle<Font>, menu: &Menu) {
    commands.insert_resource(Summaries(
        menu.slots
            .iter()
            .map(|s| (s.id, summary(s, menu.active)))
            .collect(),
    ));
    let root = commands
        .spawn((
            Root,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(
            LinearGradient::TO_RIGHT,
            vec![
                ColorStop::percent(Color::srgba(0.015, 0.019, 0.016, 0.82), 0.0),
                ColorStop::percent(Color::srgba(0.015, 0.019, 0.016, 0.65), 25.0),
                ColorStop::percent(Color::NONE, 41.0),
                ColorStop::percent(Color::NONE, 70.0),
                ColorStop::percent(Color::srgba(0.015, 0.019, 0.016, 0.68), 87.0),
                ColorStop::percent(Color::srgba(0.015, 0.019, 0.016, 0.78), 100.0),
            ],
        ))]),
        ChildOf(root),
    ));
    let heading = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(40),
                top: px(32),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    theme::words(commands, heading, "WoW Souls", font, 30.0, Node::default());
    let sub = theme::words(
        commands,
        heading,
        "Choose your character",
        font,
        15.0,
        Node::default(),
    );
    commands.entity(sub).insert(TextColor(MUTED));
    let list = theme::panel(
        commands,
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            top: px(112),
            width: px(340),
            height: px(42.0 + 63.0 * menu.slots.len().min(VISIBLE) as f32),
            padding: UiRect::all(px(12)),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        },
        Some(root),
    );
    text(commands, list, font, 12.0, Words::Count, "");
    for i in 0..VISIBLE {
        let row = commands
            .spawn((
                RosterRow(i),
                RowButton(i),
                Button,
                Node {
                    width: percent(100),
                    height: px(57),
                    min_height: px(57),
                    padding: UiRect::all(px(7)),
                    border: UiRect::left(px(2)),
                    align_items: AlignItems::Center,
                    column_gap: px(12),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                ChildOf(list),
            ))
            .id();
        commands.spawn((
            Glyph(i),
            Surface::Icon(2),
            Node {
                width: px(40),
                height: px(40),
                min_width: px(40),
                ..default()
            },
            ChildOf(row),
        ));
        let copy = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    min_width: px(0),
                    ..default()
                },
                ChildOf(row),
            ))
            .id();
        text(commands, copy, font, 20.0, Words::RowName(i), "");
        let info = text(commands, copy, font, 12.0, Words::RowInfo(i), "");
        commands.entity(info).insert(TextColor(MUTED));
    }
    let actions = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(40),
                bottom: px(42),
                width: px(316),
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    for (kind, caption) in [
        (MainRow::New, "New character"),
        (MainRow::Options, "Settings"),
        (MainRow::Quit, "Quit game"),
    ] {
        let row = commands
            .spawn((
                Command(kind),
                RowButton(0),
                Button,
                Node {
                    width: percent(100),
                    height: px(30),
                    padding: UiRect::horizontal(px(10)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                ChildOf(actions),
            ))
            .id();
        theme::words(commands, row, caption, font, 17.0, Node::default());
    }
    let profile = theme::panel(
        commands,
        Node {
            position_type: PositionType::Absolute,
            right: px(28),
            top: px(142),
            width: px(252),
            min_height: px(300),
            padding: UiRect::all(px(22)),
            flex_direction: FlexDirection::Column,
            row_gap: px(14),
            ..default()
        },
        Some(root),
    );
    text(commands, profile, font, 28.0, Words::Name, "");
    text(commands, profile, font, 15.0, Words::Identity, "");
    let details = text(commands, profile, font, 14.0, Words::Details, "");
    commands.entity(details).insert((
        TextLayout::new_with_justify(Justify::Left),
        Node {
            margin: UiRect::top(px(16)),
            ..default()
        },
    ));
    let play = theme::panel(
        commands,
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            margin: UiRect::left(px(-130)),
            bottom: px(48),
            width: px(260),
            height: px(44),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        Some(root),
    );
    commands.entity(play).insert((Play, Button));
    text(commands, play, font, 19.0, Words::Play, "Enter world");
    let message = theme::words(
        commands,
        root,
        "",
        font,
        13.0,
        Node {
            position_type: PositionType::Absolute,
            left: percent(34),
            right: percent(23),
            bottom: px(107),
            ..default()
        },
    );
    commands.entity(message).insert((
        Words::Message,
        TextColor(Color::srgb(0.87, 0.55, 0.40)),
        TextLayout::new_with_justify(Justify::Center),
    ));
    theme::words(
        commands,
        root,
        "Arrows / D-pad: Choose     Enter / A: Play     Delete / X: Remove",
        font,
        12.0,
        Node {
            position_type: PositionType::Absolute,
            left: percent(35),
            bottom: px(20),
            ..default()
        },
    );
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn refresh(
    menu: Res<Menu>,
    sys: Res<crate::sysmenu::SysMenu>,
    data: Res<Summaries>,
    game: Option<Res<crate::quests::QuestGame>>,
    mut roots: Query<&mut Visibility, With<Root>>,
    mut rows: Query<
        (
            &RosterRow,
            &mut RowButton,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Without<Command>,
    >,
    mut actions: Query<
        (&Command, &mut RowButton, &mut Node, &mut BackgroundColor),
        Without<RosterRow>,
    >,
    mut words: Query<(&Words, &mut Text)>,
    mut glyphs: Query<(&Glyph, &mut Surface)>,
    mut play: Query<&mut Node, (With<Play>, Without<Command>, Without<RosterRow>)>,
) {
    let showing = menu.screen == Screen::Main && !sys.open;
    for mut vis in &mut roots {
        *vis = if showing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if !showing {
        return;
    }
    let main = menu.main_rows();
    let selected = menu.shown_slot();
    let start = window(
        selected
            .and_then(|s| menu.slots.iter().position(|r| r.id == s.id))
            .unwrap_or(0),
        menu.slots.len(),
    );
    let level = |slot: &SlotInfo| {
        data.0
            .get(&slot.id)
            .and_then(|s| s.xp)
            .map(|xp| {
                game.as_ref().map_or(
                    if xp == 0 {
                        "1".to_string()
                    } else {
                        "—".into()
                    },
                    |g| g.table.level_for(xp).to_string(),
                )
            })
            .unwrap_or_else(|| "—".into())
    };
    for (row, mut button, mut node, mut bg, mut border) in &mut rows {
        let index = start + row.0;
        button.0 = index;
        node.display = if index < menu.slots.len() {
            Display::Flex
        } else {
            Display::None
        };
        bg.0 = if index == menu.cursor {
            theme::SELECTED
        } else {
            Color::NONE
        };
        *border = BorderColor::all(
            if selected.is_some_and(|s| menu.slots.get(index).is_some_and(|r| r.id == s.id)) {
                GOLD
            } else {
                Color::NONE
            },
        );
    }
    for (command, mut button, mut node, mut bg) in &mut actions {
        let index = main.iter().position(|r| *r == command.0);
        button.0 = index.unwrap_or(0);
        node.display = if index.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        bg.0 = if index == Some(menu.cursor) {
            theme::SELECTED
        } else {
            Color::NONE
        };
    }
    for (glyph, mut surface) in &mut glyphs {
        if let Some(slot) = menu.slots.get(start + glyph.0) {
            let want = Surface::Icon(if slot.character.class == crate::character::Class::Mage {
                3
            } else {
                2
            });
            if *surface != want {
                *surface = want;
            }
        }
    }
    for (label, mut text) in &mut words {
        let value = match *label {
            Words::Count => {
                if menu.slots.len() > VISIBLE {
                    format!(
                        "CHARACTERS    {}–{} / {}",
                        start + 1,
                        (start + VISIBLE).min(menu.slots.len()),
                        menu.slots.len()
                    )
                } else {
                    format!(
                        "CHARACTERS    {} / {}",
                        menu.slots.len(),
                        crate::saves::MAX_CHARACTERS
                    )
                }
            }
            Words::RowName(i) => menu
                .slots
                .get(start + i)
                .map(|s| s.character.name.clone())
                .unwrap_or_default(),
            Words::RowInfo(i) => menu
                .slots
                .get(start + i)
                .map(|s| {
                    format!(
                        "Lv. {}  ·  {} {}",
                        level(s),
                        s.character.race.name(),
                        s.character.class.name()
                    )
                })
                .unwrap_or_default(),
            Words::Name => selected
                .map(|s| s.character.name.clone())
                .unwrap_or_default(),
            Words::Identity => selected
                .map(|s| {
                    format!(
                        "Level {}\n{} {}",
                        level(s),
                        s.character.race.name(),
                        s.character.class.name()
                    )
                })
                .unwrap_or_default(),
            Words::Details => selected
                .map(|s| {
                    let xp = data
                        .0
                        .get(&s.id)
                        .and_then(|m| m.xp)
                        .map_or("Unavailable".into(), |n| n.to_string());
                    let role = if s.character.class == crate::character::Class::Mage {
                        "Mana\nRanged magic"
                    } else {
                        "Rage\nClose combat"
                    };
                    format!("Elwynn Forest\n\nExperience    {xp}\n\n{role}")
                })
                .unwrap_or_default(),
            Words::Message => menu.message.clone(),
            Words::Play => "Enter world".into(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    for mut node in &mut play {
        node.display = if selected.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn actions(
    menu: Res<Menu>,
    sys: Res<crate::sysmenu::SysMenu>,
    mut clicks: MessageWriter<MenuPress>,
    buttons: Query<&Interaction, (With<Play>, Changed<Interaction>)>,
    mut wheel: MessageReader<MouseWheel>,
) {
    if menu.screen != Screen::Main || sys.open {
        wheel.clear();
        return;
    }
    if buttons.iter().any(|i| *i == Interaction::Pressed) {
        clicks.write(MenuPress(super::Press::Play));
    }
    for ev in wheel.read() {
        if ev.y != 0.0 {
            clicks.write(MenuPress(if ev.y > 0.0 {
                super::Press::Up
            } else {
                super::Press::Down
            }));
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (actions.before(super::input), refresh.after(super::render))
            .run_if(in_state(Phase::Menu))
            .run_if(theme::enabled)
            .run_if(resource_exists::<Menu>)
            .run_if(resource_exists::<Summaries>),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roster_window_keeps_the_selected_character_visible_at_every_list_size() {
        for count in 1..=crate::saves::MAX_CHARACTERS {
            for cursor in 0..count {
                let first = window(cursor, count);
                assert!(cursor >= first && cursor < first + VISIBLE);
                assert!(first + VISIBLE <= count.max(VISIBLE));
            }
        }
        assert_eq!(window(12, 10), 4);
    }
}
