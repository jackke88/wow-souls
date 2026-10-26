//! Three trees share one point budget. The system menu keeps the world/input paused.

use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::time::Real;

use super::{Build, TalentId, Talents, Tree};
use crate::avatar::Avatar;
use crate::combat::{Action, ActionState, Health};
use crate::hud::theme::{GOLD, IVORY, MUTED};
use crate::npc::{Hostile, Reaction};
use crate::settings::{MenuUp, Settings};
use crate::settle::Phase;
use crate::sysmenu::{Effect, Page, Press, SysMenu};

const SELECTED: Color = Color::srgba(0.36, 0.29, 0.14, 0.72);
const PANEL: Color = Color::srgba(0.045, 0.035, 0.025, 0.97);
const LOCKED: Color = Color::srgb(0.52, 0.48, 0.42);
const NOTICE: Color = Color::srgb(0.9, 0.65, 0.4);
const RESET: usize = 21;
const BACK: usize = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    Spend(TalentId),
    Reset,
    Back,
    Close,
}

#[derive(Resource, Default)]
struct Board {
    selected: usize,
    confirm_reset: bool,
    note: String,
    was_open: bool,
}

impl Board {
    /// Opening input belongs to the parent menu, including an immediate re-entry after Back.
    fn begin_frame(&mut self, open: bool, scripted_open: bool) -> bool {
        let entered = open && !self.was_open;
        self.was_open = open;
        if entered {
            self.selected = 0;
            self.confirm_reset = false;
            self.note.clear();
        }
        open && (!entered || scripted_open)
    }

    fn leave(&mut self) {
        self.was_open = false;
        self.confirm_reset = false;
    }

    fn press(&mut self, press: Press) -> Option<Intent> {
        if press == Press::Back {
            if self.confirm_reset {
                self.confirm_reset = false;
                self.note = "Reset cancelled.".into();
                return None;
            }
            return Some(Intent::Back);
        }
        if press == Press::Delete {
            self.selected = RESET;
            self.confirm_reset = true;
            self.note = "Reset all talent points? Select Confirm reset. Esc / B cancels.".into();
            return None;
        }
        if press == Press::Confirm {
            return Some(match self.selected {
                RESET => Intent::Reset,
                BACK => Intent::Back,
                n => Intent::Spend(TalentId::ALL[n.min(20)]),
            });
        }
        self.confirm_reset = false;
        self.note.clear();
        self.selected = match press {
            Press::Up if self.selected >= RESET => 6,
            Press::Down if self.selected >= RESET => 0,
            Press::Up if self.selected.is_multiple_of(7) => BACK,
            Press::Down if self.selected % 7 == 6 => RESET,
            Press::Up => self.selected - 1,
            Press::Down => self.selected + 1,
            Press::Left if self.selected >= RESET => RESET,
            Press::Right if self.selected >= RESET => BACK,
            Press::Left => (self.selected + 14) % 21,
            Press::Right => (self.selected + 7) % 21,
            _ => self.selected,
        };
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Step {
    Open,
    Select(TalentId),
    Spend(TalentId),
    Press(Press),
}

#[derive(Resource, Default)]
struct Script {
    steps: Vec<(f32, Step)>,
    next: usize,
}

fn parse_script(text: &str) -> Vec<(f32, Step)> {
    let mut steps: Vec<_> = text
        .split(',')
        .filter_map(|part| {
            let (time, verb) = part.trim().split_once(':')?;
            let time: f32 = time.parse().ok()?;
            if !time.is_finite() || time < 0.0 {
                return None;
            }
            let step = match verb {
                "open" => Step::Open,
                "up" => Step::Press(Press::Up),
                "down" => Step::Press(Press::Down),
                "left" => Step::Press(Press::Left),
                "right" => Step::Press(Press::Right),
                "confirm" => Step::Press(Press::Confirm),
                "back" | "cancel" => Step::Press(Press::Back),
                "reset" => Step::Press(Press::Delete),
                _ => {
                    let (action, id) = verb.split_once(':')?;
                    let id = TalentId::from_id(id)?;
                    match action {
                        "select" => Step::Select(id),
                        "spend" => Step::Spend(id),
                        _ => return None,
                    }
                }
            };
            Some((time, step))
        })
        .collect();
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    steps
}

#[derive(Component)]
struct Root;

#[derive(Component)]
enum Label {
    Header,
    Summary,
    Tree(Tree),
    Talent(TalentId),
    Details,
    Help,
    Status,
    Reset,
}

#[derive(Component, Clone, Copy)]
enum Click {
    Talent(TalentId),
    Reset,
    Back,
}

impl Click {
    fn index(self) -> usize {
        match self {
            Self::Talent(id) => id as usize,
            Self::Reset => RESET,
            Self::Back => BACK,
        }
    }
}

#[derive(Resource)]
struct FontHandle(Handle<Font>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Board>()
        .insert_resource(Script {
            steps: std::env::var("WOWSOULS_TALENTS_SCRIPT")
                .map(|s| parse_script(&s))
                .unwrap_or_default(),
            next: 0,
        })
        .add_systems(OnEnter(Phase::Playing), load_font)
        .add_systems(
            Update,
            (drive, sync_root, refresh)
                .chain()
                .after(crate::sysmenu::ui::MenuInput)
                .before(crate::controller::PlaySet::Intent)
                .before(crate::sysmenu::ui::pause)
                .run_if(in_state(Phase::Playing)),
        )
        .add_systems(OnExit(Phase::Playing), cleanup);
}

fn load_font(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    commands.insert_resource(FontHandle(crate::hud::font::project_font(&mut fonts)));
}

fn editable(class: u8, alive: bool, idle: bool, combat: bool) -> Result<(), &'static str> {
    if class != wow_world::talents::WARRIOR {
        Err("Talent trees are currently available for Warriors.")
    } else if !alive {
        Err("You cannot change talents while dead.")
    } else if combat {
        Err("Leave combat before changing talents.")
    } else if !idle {
        Err("Finish your current action before changing talents.")
    } else {
        Ok(())
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct Context<'w, 's> {
    game: Option<Res<'w, crate::quests::QuestGame>>,
    avatar: Query<'w, 's, (&'static Action, &'static Health), With<Avatar>>,
    foes: Query<'w, 's, (&'static Hostile, &'static Reaction)>,
}

impl Context<'_, '_> {
    fn permission(&self) -> Result<(u8, u8), &'static str> {
        let game = self
            .game
            .as_ref()
            .ok_or("Character progress is still loading.")?;
        let (action, health) = self
            .avatar
            .single()
            .map_err(|_| "Character is still loading.")?;
        let combat = self
            .foes
            .iter()
            .any(|(foe, reaction)| foe.alert && *reaction != Reaction::Dead);
        let class = game.player.class as u8;
        let level = game.player.level as u8;
        editable(
            class,
            health.cur > 0.0 && !action.is_dead(),
            action.state == ActionState::Idle,
            combat,
        )?;
        Ok((class, level))
    }
}

fn leave_menu(intent: Intent, board: &mut Board, menu: &mut SysMenu, up: &mut MenuUp) -> bool {
    if matches!(intent, Intent::Back | Intent::Close) {
        if intent == Intent::Close || menu.leave().contains(&Effect::Close) {
            menu.hide();
            up.0 = false;
        }
        board.leave();
        return true;
    }
    false
}

fn apply(
    intent: Intent,
    board: &mut Board,
    talents: &mut Talents,
    context: &Context,
    menu: &mut SysMenu,
    up: &mut MenuUp,
) {
    if leave_menu(intent, board, menu, up) {
        return;
    }
    let (_, level) = match context.permission() {
        Ok(value) => value,
        Err(why) => {
            board.note = why.into();
            board.confirm_reset = false;
            return;
        }
    };
    if intent == Intent::Reset && !board.confirm_reset {
        board.selected = RESET;
        board.confirm_reset = true;
        board.note = "Reset all talent points? Select Confirm reset. Esc / B cancels.".into();
        return;
    }
    let mut next = talents.build.clone();
    let result = match intent {
        Intent::Spend(id) => next.spend(id, level),
        Intent::Reset => {
            next.reset();
            Ok(())
        }
        Intent::Back | Intent::Close => unreachable!(),
    }
    .and_then(|()| talents.replace(next, level));
    board.confirm_reset = false;
    board.note = match result {
        Ok(()) => match intent {
            Intent::Spend(id) => format!(
                "{}: rank {}/{}. Saved.",
                id.name(),
                talents.rank(id),
                id.max_rank()
            ),
            _ => "All talent points refunded. Saved.".into(),
        },
        Err(why) => why,
    };
    info!("talents: {}", board.note);
}

#[allow(clippy::too_many_arguments)]
fn drive(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    clicks: Query<(&Interaction, &Click), Changed<Interaction>>,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    real: Res<Time<Real>>,
    mut script: ResMut<Script>,
    mut board: ResMut<Board>,
    mut talents: ResMut<Talents>,
    mut menu: ResMut<SysMenu>,
    mut up: ResMut<MenuUp>,
    context: Context,
    settings: Res<Settings>,
) {
    let mut steps = Vec::new();
    while let Some((time, step)) = script.steps.get(script.next).cloned() {
        if time > real.elapsed_secs() {
            break;
        }
        steps.push(step);
        script.next += 1;
    }
    let scripted_open = steps.contains(&Step::Open);
    if scripted_open {
        menu.show();
        menu.page = Page::Talents;
        up.0 = true;
    }
    let open = menu.open && menu.page == Page::Talents;
    let entered = open && !board.was_open;
    let accept_input = board.begin_frame(open, scripted_open);
    if !open {
        return;
    }
    if entered {
        menu.fresh = false;
    }
    // A click/Enter that opened Talents must not buy the first talent too.
    if !accept_input {
        return;
    }
    let mut intents = Vec::new();
    for step in steps {
        match step {
            Step::Open => {}
            Step::Select(id) => {
                board.selected = id as usize;
                board.confirm_reset = false;
            }
            Step::Spend(id) => {
                board.selected = id as usize;
                apply(
                    Intent::Spend(id),
                    &mut board,
                    &mut talents,
                    &context,
                    &mut menu,
                    &mut up,
                );
            }
            Step::Press(press) => {
                if let Some(intent) = board.press(press) {
                    apply(
                        intent,
                        &mut board,
                        &mut talents,
                        &context,
                        &mut menu,
                        &mut up,
                    );
                }
            }
        }
    }
    if !menu.open || menu.page != Page::Talents {
        return;
    }
    if unattended.is_none() {
        let pad = |button| pads.iter().any(|p| p.just_pressed(button));
        for (key, button, press) in [
            (KeyCode::ArrowUp, GamepadButton::DPadUp, Press::Up),
            (KeyCode::ArrowDown, GamepadButton::DPadDown, Press::Down),
            (KeyCode::ArrowLeft, GamepadButton::DPadLeft, Press::Left),
            (KeyCode::ArrowRight, GamepadButton::DPadRight, Press::Right),
            (KeyCode::Enter, GamepadButton::South, Press::Confirm),
            (KeyCode::Escape, GamepadButton::East, Press::Back),
            (KeyCode::Delete, GamepadButton::West, Press::Delete),
        ] {
            if keys.just_pressed(key) || pad(button) {
                if let Some(intent) = board.press(press) {
                    intents.push(intent);
                }
            }
        }
        if settings
            .binds
            .pad_button(crate::settings::binds::Action::Menu)
            .is_some_and(pad)
        {
            intents.push(Intent::Close);
        }
        for (interaction, click) in &clicks {
            if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
                if board.selected != click.index() {
                    board.confirm_reset = false;
                    board.note.clear();
                }
                board.selected = click.index();
            }
            if *interaction == Interaction::Pressed {
                if let Some(intent) = board.press(Press::Confirm) {
                    intents.push(intent);
                }
            }
        }
        // UI clicks cannot be reused as a weapon press on the frame the board closes.
        mouse.clear_just_pressed(MouseButton::Left);
    }
    for intent in intents {
        if !menu.open || menu.page != Page::Talents {
            break;
        }
        apply(
            intent,
            &mut board,
            &mut talents,
            &context,
            &mut menu,
            &mut up,
        );
    }
}

fn cleanup(mut commands: Commands, roots: Query<Entity, With<Root>>, mut board: ResMut<Board>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    board.leave();
}

fn sync_root(
    mut commands: Commands,
    menu: Res<SysMenu>,
    font: Option<Res<FontHandle>>,
    roots: Query<Entity, With<Root>>,
) {
    let open = menu.open && menu.page == Page::Talents;
    if !open {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    if !roots.is_empty() {
        return;
    }
    let Some(font) = font else {
        return;
    };
    let text_font = |size| TextFont {
        font: font.0.clone(),
        font_size: size,
        ..default()
    };
    commands.spawn((Root, GlobalZIndex(125), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)), Node {
        position_type: PositionType::Absolute, width: percent(100), height: percent(100),
        justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default()
    })).with_children(|root| {
        let mut panel = root.spawn((BackgroundColor(PANEL), BorderColor::all(crate::hud::theme::EDGE), Node {
            width: percent(94), max_width: px(1180), padding: UiRect::all(px(22)),
            border: UiRect::all(px(1)), flex_direction: FlexDirection::Column, row_gap: px(8), ..default()
        }));
        panel.with_children(|panel| {
            panel.spawn((Label::Header, Text::new("Warrior talents"), text_font(28.0), TextColor(GOLD)));
            panel.spawn((Label::Summary, Text::new(""), text_font(17.0), TextColor(IVORY)));
            panel.spawn(Node { width: percent(100), column_gap: px(12), ..default() }).with_children(|trees| {
                for tree in Tree::ALL {
                    trees.spawn(Node { flex_basis: percent(33.33), flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: px(3), ..default() }).with_children(|column| {
                        column.spawn((Label::Tree(tree), Text::new(tree.name()), text_font(21.0), TextColor(GOLD)));
                        for id in tree.talents() {
                            column.spawn((Button, Click::Talent(id), BackgroundColor(Color::NONE), Node {
                                width: percent(100), height: px(36), align_items: AlignItems::Center,
                                padding: UiRect::horizontal(px(8)), ..default()
                            })).with_children(|button| {
                                button.spawn((Label::Talent(id), Text::new(""), text_font(17.0), TextColor(IVORY)));
                            });
                        }
                    });
                }
            });
            panel.spawn((Label::Details, Text::new(""), text_font(16.0), TextColor(IVORY), Node { min_height: px(62), ..default() }));
            panel.spawn((Label::Help, Text::new(""), text_font(14.0), TextColor(MUTED)));
            panel.spawn((Label::Status, Text::new(""), text_font(15.0), TextColor(NOTICE), Node { min_height: px(22), ..default() }));
            panel.spawn(Node { column_gap: px(16), ..default() }).with_children(|actions| {
                for (click, label) in [(Click::Reset, "Reset talents"), (Click::Back, "Back")] {
                    actions.spawn((Button, click, BackgroundColor(Color::NONE), Node {
                        min_width: px(180), height: px(34), align_items: AlignItems::Center,
                        padding: UiRect::horizontal(px(10)), ..default()
                    })).with_children(|button| {
                        if matches!(click, Click::Reset) { button.spawn((Label::Reset, Text::new(label), text_font(17.0), TextColor(GOLD))); }
                        else { button.spawn((Text::new(label), text_font(17.0), TextColor(IVORY))); }
                    });
                }
            });
            panel.spawn((Text::new("Arrows / D-pad: navigate     Enter / A / click: spend     Delete / X: reset     Esc / B: back"), text_font(12.0), TextColor(MUTED)));
        });
    });
}

fn refresh(
    menu: Res<SysMenu>,
    board: Res<Board>,
    talents: Res<Talents>,
    context: Context,
    mut labels: Query<(&Label, &mut Text, &mut TextColor)>,
    mut buttons: Query<(&Click, &mut BackgroundColor)>,
) {
    if !menu.open || menu.page != Page::Talents {
        return;
    }
    let level = context
        .game
        .as_ref()
        .map_or(1, |game| game.player.level as u8);
    let warrior = talents.class == wow_world::talents::WARRIOR;
    let selected = TalentId::ALL.get(board.selected).copied();
    for (label, mut text, mut color) in &mut labels {
        let (value, tint) = match label {
            Label::Header => ("Warrior talents".into(), GOLD),
            Label::Summary => (if warrior { format!("Level {level}  •  {} points available  •  {} / {} spent  •  One point per level 10–60. Mix trees freely.", talents.build.available(level), talents.build.spent(), Build::budget(level)) } else { "Warrior talent trees. This character has no Warrior talent effects.".into() }, IVORY),
            Label::Tree(tree) => (format!("{}  ({})", tree.name(), talents.build.points_in(*tree)), GOLD),
            Label::Talent(id) => {
                let rank = talents.rank(*id);
                let locked = !warrior || talents.build.points_in(id.tree()) < id.gate();
                (format!("{}   {}/{}{}", id.name(), rank, id.max_rank(), if locked { format!("  [{} pts]", id.gate()) } else { String::new() }), if selected == Some(*id) { GOLD } else if locked { LOCKED } else { IVORY })
            },
            Label::Details => (selected.map_or_else(|| {
                if board.selected == RESET { "Reset refunds every point. You must confirm, be alive and outside combat.".into() }
                else { "Return to the menu.".into() }
            }, |id| format!("{} — {}\nRequires {} points in {}. Rank {}/{}.", id.name(), id.description(), id.gate(), id.tree().name(), talents.rank(id), id.max_rank())), IVORY),
            Label::Help => ("Perfect block: face a melee hit and raise guard within 160 ms. One contact per raise; 450 ms to rearm. Costs 35% normal block stamina (minimum 1); no chip if affordable. Warrior: +4 rage.\nPerfect parry: first 120 ms of your active parry, within 100 ms before the enemy hit or during it. Staggers; +25% next melee counter against that enemy for 3 s. Warrior: +8 rage.".into(), MUTED),
            Label::Status => (if !board.note.is_empty() { board.note.clone() } else if talents.blocked.is_some() { "The saved talent file is unreadable and has been preserved. Changes are disabled.".into() } else { context.permission().err().unwrap_or("Select a talent to spend one point. Changes save immediately.").into() }, NOTICE),
            Label::Reset => ((if board.confirm_reset { "Confirm reset" } else { "Reset talents" }).into(), GOLD),
        };
        if text.0 != value {
            text.0 = value;
        }
        if color.0 != tint {
            color.0 = tint;
        }
    }
    for (click, mut background) in &mut buttons {
        let tint = if click.index() == board.selected {
            SELECTED
        } else {
            Color::NONE
        };
        if background.0 != tint {
            background.0 = tint;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_reaches_each_tree_and_requires_deliberate_reset() {
        let mut board = Board::default();
        board.press(Press::Right);
        assert_eq!(
            board.press(Press::Confirm),
            Some(Intent::Spend(TalentId::UnbridledWrath))
        );
        board.press(Press::Right);
        assert_eq!(
            board.press(Press::Confirm),
            Some(Intent::Spend(TalentId::ShieldSpecialization))
        );
        board.press(Press::Delete);
        assert!(board.confirm_reset);
        assert_eq!(board.press(Press::Back), None);
        assert!(!board.confirm_reset);
        board.press(Press::Delete);
        assert_eq!(board.press(Press::Confirm), Some(Intent::Reset));
    }

    #[test]
    fn immediate_reentry_consumes_its_opening_enter_or_pad_a() {
        let mut board = Board::default();
        assert!(!board.begin_frame(true, false), "opening input is consumed");
        assert!(
            board.begin_frame(true, false),
            "following frame accepts input"
        );
        assert_eq!(board.press(Press::Back), Some(Intent::Back));
        board.leave();
        assert!(
            !board.begin_frame(true, false),
            "Back then immediate re-entry consumes Enter/A too"
        );
        assert!(board.begin_frame(true, false));
        assert_eq!(
            board.press(Press::Confirm),
            Some(Intent::Spend(TalentId::TacticalMastery))
        );
    }

    #[test]
    fn pad_b_returns_to_root_and_pad_menu_closes_every_page() {
        let mut menu = SysMenu::default();
        let mut settings = Settings::default();
        menu.show();
        menu.cursor = menu
            .rows(&settings)
            .iter()
            .position(|row| row.label == "Talents")
            .unwrap();
        menu.press(Press::Confirm, &mut settings);
        let mut up = MenuUp(true);
        let mut board = Board::default();
        board.begin_frame(true, false);
        assert!(leave_menu(Intent::Back, &mut board, &mut menu, &mut up));
        assert!(menu.open && menu.page == Page::Root && up.0);
        assert!(!board.was_open);
        menu.press(Press::Confirm, &mut settings);
        board.begin_frame(true, false);
        assert!(leave_menu(Intent::Close, &mut board, &mut menu, &mut up));
        assert!(!menu.open && !up.0 && !board.was_open);
    }

    #[test]
    fn no_mutation_during_combat_death_or_an_action_or_for_other_classes() {
        assert!(editable(1, true, true, false).is_ok());
        assert!(editable(8, true, true, false).is_err());
        assert!(editable(1, false, true, false).is_err());
        assert!(editable(1, true, false, false).is_err());
        assert!(editable(1, true, true, true).is_err());
    }

    #[test]
    fn automation_uses_stable_ids_and_rejects_invalid_times() {
        assert_eq!(
            parse_script(
                "2:spend:flurry,1:open,3:reset,4:confirm,-1:open,NaN:open,5:spend:unknown"
            ),
            vec![
                (1.0, Step::Open),
                (2.0, Step::Spend(TalentId::Flurry)),
                (3.0, Step::Press(Press::Delete)),
                (4.0, Step::Press(Press::Confirm))
            ]
        );
    }
}
