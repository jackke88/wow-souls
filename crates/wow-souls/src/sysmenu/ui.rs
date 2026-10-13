//! The system menu's game side: opening it (Esc, the pad's Menu button), its panel in the title's
//! parchment style, the input that drives it (keys, pad, mouse, the capture of the next key), the pause,
//! and what its rows do outside the menu (open the pack, the equipment page or the quest log, quit).
//!
//! `WOWSOULS_SYSMENU_SCRIPT=<s>:<verb>,...` is an unattended run's hands, at real second `s`:
//! `open close up down left right confirm back delete capture:<chord> pad:<button>`
//! (`capture:Shift+KeyG`, `capture:Mouse:Back`, `pad:North`).

use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::{MouseButton, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::sprite::{BorderRect, TextureSlicer};
use bevy::time::{Real, Virtual};
use bevy::ui::widget::NodeImageMode;

use super::{capture_input, capture_step, Captured, Effect, Kind, Press, Slot, SysMenu, VISIBLE};
use crate::settings::binds::{parse_chord, parse_pad, Action, Chord};
use crate::settings::{MenuUp, Settings};
use crate::settle::Phase;

const IVORY: Color = Color::srgb(0.88, 0.85, 0.77);
const GOLD: Color = Color::srgb(0.92, 0.76, 0.28);
const MUTED: Color = Color::srgb(0.62, 0.6, 0.54);
const WARN: Color = Color::srgb(0.85, 0.52, 0.34);
const DIM: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);
const INK: Color = Color::srgba(0.04, 0.03, 0.02, 0.9);

/// A waiting pad capture gives up after this many seconds (a pad cannot press Esc).
const PAD_CAPTURE_SECS: f32 = 6.0;

/// `WOWSOULS_SYSMENU_SCRIPT` steps.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Open,
    Close,
    Press(Press),
    Chord(Chord),
    Pad(GamepadButton),
}

#[derive(Resource, Default)]
struct Script {
    steps: Vec<(f32, Step)>,
    next: usize,
}

fn parse_script(text: &str) -> Vec<(f32, Step)> {
    let mut steps: Vec<(f32, Step)> = text
        .split(',')
        .filter_map(|part| {
            let (t, what) = part.trim().split_once(':')?;
            let step = match what.trim() {
                "open" => Step::Open,
                "close" => Step::Close,
                "up" => Step::Press(Press::Up),
                "down" => Step::Press(Press::Down),
                "left" => Step::Press(Press::Left),
                "right" => Step::Press(Press::Right),
                "confirm" => Step::Press(Press::Confirm),
                "back" => Step::Press(Press::Back),
                "delete" => Step::Press(Press::Delete),
                other => {
                    if let Some(c) = other.strip_prefix("capture:") {
                        Step::Chord(parse_chord(c)?)
                    } else {
                        Step::Pad(parse_pad(other.strip_prefix("pad:")?)?)
                    }
                }
            };
            Some((t.trim().parse().ok()?, step))
        })
        .collect();
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    steps
}

#[derive(Component)]
struct Root;

#[derive(Component)]
struct Title;

#[derive(Component)]
struct NoteLine;

/// A row of the visible window.
#[derive(Component)]
struct RowBox(usize);

#[derive(Component)]
struct RowLabel(usize);

#[derive(Component)]
struct RowValue(usize);

#[derive(Resource)]
struct Look {
    font: Handle<Font>,
    parchment: Option<Handle<Image>>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SysMenu>()
        .insert_resource(Script {
            steps: std::env::var("WOWSOULS_SYSMENU_SCRIPT")
                .map(|s| parse_script(&s))
                .unwrap_or_default(),
            next: 0,
        })
        // Not at start: the HUD's art (the parchment) is loaded by the time play or the title begins.
        .add_systems(OnEnter(Phase::Playing), load_look)
        .add_systems(OnEnter(Phase::Menu), load_look)
        .add_systems(OnExit(Phase::Playing), close_all)
        .add_systems(OnExit(Phase::Menu), close_all)
        // Opened by Esc in play, by the title's Options row before it: the pages work in both.
        .add_systems(Update, toggle.run_if(in_state(Phase::Playing)))
        .add_systems(
            Update,
            (drive, sync_root, refresh)
                .chain()
                .after(toggle)
                .run_if(in_state(Phase::Playing).or(in_state(Phase::Menu))),
        )
        .add_systems(Update, pause.run_if(in_state(Phase::Playing)));
}

fn load_look(
    mut commands: Commands,
    mut fonts: ResMut<Assets<Font>>,
    art: Option<Res<crate::hud::art::HudArt>>,
) {
    let font = std::fs::read("C:/Windows/Fonts/georgia.ttf")
        .ok()
        .and_then(|b| Font::try_from_bytes(b).ok())
        .map(|f| fonts.add(f))
        .unwrap_or_default();
    let parchment = art
        .as_deref()
        .map(|a| a.sprite_image("soul_param/bg_parchment_0"))
        .filter(|h| *h != Handle::default());
    commands.insert_resource(Look { font, parchment });
}

/// The menu is down and the world runs again: out of play (a quit to the title, an exit).
fn close_all(mut menu: ResMut<SysMenu>, mut up: ResMut<MenuUp>, mut time: ResMut<Time<Virtual>>) {
    menu.hide();
    up.0 = false;
    if time.is_paused() {
        time.unpause();
    }
}

/// Esc or the Menu binding opens the menu when no screen is up; a screen that Esc closes does not
/// also open it (a screen was up when the frame began).
#[allow(clippy::too_many_arguments)]
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    settings: Res<Settings>,
    screen: Res<crate::loot::Screen>,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    mut script: ResMut<Script>,
    real: Res<Time<Real>>,
    mut menu: ResMut<SysMenu>,
    mut up: ResMut<MenuUp>,
    mut screen_was_up: Local<bool>,
) {
    let was_up = std::mem::replace(&mut *screen_was_up, screen.open || screen.modal);
    if menu.open {
        return;
    }
    let live = unattended.is_none();
    let asked = live
        && (keys.just_pressed(KeyCode::Escape)
            || settings
                .binds
                .just_pressed_in(Action::Menu, &keys, &mouse, &pads, true));
    // A scripted `open` (an unattended check's hands); the other steps are `drive`'s.
    let mut scripted = false;
    while let Some((t, Step::Open)) = script.steps.get(script.next).cloned() {
        if t > real.elapsed_secs() {
            break;
        }
        script.next += 1;
        scripted = true;
    }
    if (asked && !was_up && !screen.open && !screen.modal) || scripted {
        menu.show();
        up.0 = true;
        info!("sysmenu: open");
    }
}

/// The world stands still while the menu is up (the player's option).
fn pause(menu: Res<SysMenu>, settings: Res<Settings>, mut time: ResMut<Time<Virtual>>) {
    let want = menu.open && settings.game.pause;
    if want && !time.is_paused() {
        time.pause();
    } else if !want && time.is_paused() {
        time.unpause();
    }
}

/// What the menu is fed this frame: presses from the keys, the pad and the script, and the capture.
#[allow(clippy::too_many_arguments)]
fn drive(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    mut wheel: MessageReader<MouseWheel>,
    rows: Query<(&Interaction, &RowBox), Changed<Interaction>>,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    real: Res<Time<Real>>,
    mut script: ResMut<Script>,
    mut settings: ResMut<Settings>,
    mut menu: ResMut<SysMenu>,
    mut up: ResMut<MenuUp>,
    mut screen: ResMut<crate::loot::Screen>,
    mut log: ResMut<crate::quests::LogScreen>,
    mut exit: MessageWriter<AppExit>,
    mut waited: Local<f32>,
) {
    if !menu.open {
        *waited = 0.0;
        return;
    }
    // The press that opened the menu was this frame's; the menu's first press is the next.
    if std::mem::take(&mut menu.fresh) {
        return;
    }
    let live = unattended.is_none();
    let now = real.elapsed_secs();
    let pad_list: Vec<&Gamepad> = pads.iter().collect();

    let mut presses: Vec<Press> = Vec::new();
    let mut captured: Option<Captured> = None;
    let mut close_now = false;

    // The script's steps whose time has come (the `open` step is `toggle`'s).
    while let Some((t, step)) = script.steps.get(script.next).cloned() {
        if t > now {
            break;
        }
        match step {
            Step::Open => {}
            Step::Close => close_now = true,
            Step::Press(p) => presses.push(p),
            Step::Chord(c) => captured = Some(Captured::Chord(c)),
            Step::Pad(b) => captured = Some(Captured::Pad(b)),
        }
        script.next += 1;
    }

    if live {
        if let Some(capture) = menu.capture {
            let input = capture_input(&keys, &mouse, &pad_list);
            let got = capture_step(capture.slot, &input);
            if got != Captured::Nothing {
                captured = Some(got);
            }
            // A pad capture that nobody answers gives up.
            if capture.slot == Slot::Pad {
                *waited += real.delta_secs();
                if *waited >= PAD_CAPTURE_SECS {
                    captured = Some(Captured::Cancel);
                }
            }
        } else {
            *waited = 0.0;
            let any = |ks: &[KeyCode]| ks.iter().any(|&k| keys.just_pressed(k));
            let pad = |b: GamepadButton| pad_list.iter().any(|p| p.just_pressed(b));
            if any(&[KeyCode::ArrowUp]) || pad(GamepadButton::DPadUp) {
                presses.push(Press::Up);
            }
            if any(&[KeyCode::ArrowDown]) || pad(GamepadButton::DPadDown) {
                presses.push(Press::Down);
            }
            if any(&[KeyCode::ArrowLeft]) || pad(GamepadButton::DPadLeft) {
                presses.push(Press::Left);
            }
            if any(&[KeyCode::ArrowRight]) || pad(GamepadButton::DPadRight) {
                presses.push(Press::Right);
            }
            if any(&[KeyCode::Enter, KeyCode::NumpadEnter]) || pad(GamepadButton::South) {
                presses.push(Press::Confirm);
            }
            if any(&[KeyCode::Escape]) || pad(GamepadButton::East) {
                presses.push(Press::Back);
            }
            if any(&[KeyCode::Delete, KeyCode::Backspace]) || pad(GamepadButton::West) {
                presses.push(Press::Delete);
            }
            // The Menu button closes the whole menu; the keyboard's own binding is Esc, which steps back.
            if settings.binds.pad_button(Action::Menu).is_some_and(pad) {
                close_now = true;
            }
            // The wheel moves the value of the row under the cursor.
            for ev in wheel.read() {
                let dy = match ev.unit {
                    MouseScrollUnit::Line => ev.y,
                    MouseScrollUnit::Pixel => ev.y / 24.0,
                };
                if dy > 0.0 {
                    presses.push(Press::Right);
                } else if dy < 0.0 {
                    presses.push(Press::Left);
                }
            }
            // The mouse: pointing at a row moves the cursor, a click acts on it.
            for (interaction, row) in &rows {
                let index = menu.scroll + row.0;
                match interaction {
                    Interaction::Hovered | Interaction::Pressed => {
                        let rows = menu.rows(&settings);
                        if rows.get(index).is_some_and(|r| r.selectable()) {
                            menu.cursor = index;
                        }
                        if *interaction == Interaction::Pressed {
                            presses.push(Press::Confirm);
                        }
                    }
                    Interaction::None => {}
                }
            }
        }
    } else {
        wheel.clear();
    }

    let mut effects: Vec<Effect> = Vec::new();
    if let Some(got) = captured {
        if matches!(got, Captured::Cancel) {
            *waited = 0.0;
        }
        if menu.capture.is_some() && menu.captured(got, &mut settings) {
            *waited = 0.0;
        }
    }
    if menu.capture.is_none() {
        for p in presses {
            effects.extend(menu.press(p, &mut settings));
        }
    }
    if close_now {
        effects.push(Effect::Close);
    }
    for effect in effects {
        match effect {
            Effect::Close => {
                menu.hide();
                up.0 = false;
                info!("sysmenu: closed");
            }
            Effect::OpenPack => {
                screen.open = true;
                screen.page = crate::equip::Page::Items;
                screen.selected = 0;
            }
            Effect::OpenEquipment => {
                screen.open = true;
                screen.page = crate::equip::Page::Gear;
                screen.selected = 0;
            }
            Effect::OpenQuestLog => log.open = true,
            Effect::Quit => {
                info!("sysmenu: quit");
                exit.write(AppExit::Success);
            }
        }
    }
}

fn sync_root(
    mut commands: Commands,
    menu: Res<SysMenu>,
    look: Option<Res<Look>>,
    roots: Query<Entity, With<Root>>,
) {
    match (menu.open, roots.is_empty()) {
        (true, true) => {
            let Some(look) = look else { return };
            spawn(&mut commands, &look);
        }
        (false, false) => {
            for e in &roots {
                commands.entity(e).despawn();
            }
        }
        _ => {}
    }
}

fn spawn(commands: &mut Commands, look: &Look) {
    let font = |size: f32| TextFont {
        font: look.font.clone(),
        font_size: size,
        ..default()
    };
    commands
        .spawn((
            Root,
            GlobalZIndex(120),
            BackgroundColor(DIM),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            let mut panel = root.spawn((
                Node {
                    width: px(700),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(px(34), px(26)),
                    row_gap: px(2),
                    ..default()
                },
                BackgroundColor(INK),
            ));
            if let Some(image) = &look.parchment {
                let mut img = ImageNode::new(image.clone());
                img.image_mode = NodeImageMode::Sliced(TextureSlicer {
                    border: BorderRect::all(18.0),
                    ..default()
                });
                panel.insert(img);
            }
            panel.with_children(|p| {
                p.spawn((Title, Text::new(""), font(34.0), TextColor(GOLD)));
                for i in 0..VISIBLE {
                    p.spawn((
                        RowBox(i),
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            height: px(30),
                            padding: UiRect::axes(px(10), px(0)),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            margin: UiRect::top(px(if i == 0 { 12 } else { 0 })),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                    ))
                    .with_children(|row| {
                        row.spawn((RowLabel(i), Text::new(""), font(21.0), TextColor(IVORY)));
                        row.spawn((RowValue(i), Text::new(""), font(21.0), TextColor(MUTED)));
                    });
                }
                p.spawn((
                    NoteLine,
                    Text::new(""),
                    font(14.0),
                    TextColor(WARN),
                    Node {
                        margin: UiRect::top(px(10)),
                        min_height: px(18),
                        ..default()
                    },
                ));
                p.spawn((
                    Text::new(
                        "Up / Down  Move      Left / Right  Change      Enter / A  Select      \
                         Delete  Clear a key      Esc / B  Back",
                    ),
                    font(12.0),
                    TextColor(MUTED),
                ));
            });
        });
}

#[allow(clippy::type_complexity)]
fn refresh(
    menu: Res<SysMenu>,
    settings: Res<Settings>,
    mut title: Query<
        &mut Text,
        (
            With<Title>,
            Without<NoteLine>,
            Without<RowLabel>,
            Without<RowValue>,
        ),
    >,
    mut note: Query<
        &mut Text,
        (
            With<NoteLine>,
            Without<Title>,
            Without<RowLabel>,
            Without<RowValue>,
        ),
    >,
    mut boxes: Query<(&RowBox, &mut Node, &mut BackgroundColor)>,
    mut labels: Query<
        (&RowLabel, &mut Text, &mut TextColor),
        (Without<RowValue>, Without<Title>, Without<NoteLine>),
    >,
    mut values: Query<
        (&RowValue, &mut Text, &mut TextColor),
        (Without<RowLabel>, Without<Title>, Without<NoteLine>),
    >,
) {
    if !menu.open {
        return;
    }
    let (rows, at) = menu.window(&settings);
    if let Ok(mut t) = title.single_mut() {
        let want = menu.page.title().to_string();
        if t.0 != want {
            t.0 = want;
        }
    }
    if let Ok(mut n) = note.single_mut() {
        if n.0 != menu.note {
            n.0 = menu.note.clone();
        }
    }
    for (row, mut node, mut bg) in &mut boxes {
        let want_display = if row.0 < rows.len() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want_display {
            node.display = want_display;
        }
        let shade = if row.0 == at && rows.get(row.0).is_some_and(|r| r.selectable()) {
            Color::srgba(1.0, 0.9, 0.6, 0.1)
        } else {
            Color::NONE
        };
        if bg.0 != shade {
            bg.0 = shade;
        }
    }
    for (row, mut text, mut color) in &mut labels {
        let r = rows.get(row.0);
        let want = r.map(|r| r.label.clone()).unwrap_or_default();
        if text.0 != want {
            text.0 = want;
        }
        let tint = match r {
            Some(r) if r.kind == Kind::Heading => MUTED,
            Some(_) if row.0 == at => GOLD,
            _ => IVORY,
        };
        if color.0 != tint {
            color.0 = tint;
        }
    }
    for (row, mut text, mut color) in &mut values {
        let r = rows.get(row.0);
        let want = r.map(|r| r.value.clone()).unwrap_or_default();
        if text.0 != want {
            text.0 = want;
        }
        let tint = if row.0 == at { GOLD } else { MUTED };
        if color.0 != tint {
            color.0 = tint;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::binds::Mods;

    #[test]
    fn a_script_is_sorted_and_its_verbs_parse() {
        let steps = parse_script(
            "5:confirm, 2:open, 3:down, 4:capture:Shift+KeyG, 6:pad:North, 7:close, x:up, 8:bogus, \
             9:capture:Mouse:Back, 10:delete",
        );
        assert_eq!(
            steps,
            vec![
                (2.0, Step::Open),
                (3.0, Step::Press(Press::Down)),
                (
                    4.0,
                    Step::Chord(Chord::key(KeyCode::KeyG).with(Mods {
                        shift: true,
                        ..Mods::NONE
                    }))
                ),
                (5.0, Step::Press(Press::Confirm)),
                (6.0, Step::Pad(GamepadButton::North)),
                (7.0, Step::Close),
                (9.0, Step::Chord(Chord::mouse(MouseButton::Back))),
                (10.0, Step::Press(Press::Delete)),
            ]
        );
    }
}
