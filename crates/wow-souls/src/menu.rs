//! The title and character-creation flow, in [`Phase::Menu`] after the world has streamed in and before
//! play: the character stands in the valley behind the panel (the camera circles it slowly), and the
//! player picks Enter the world, makes a new character, or quits.
//!
//! The creation screen is WoW's own ([`crate::create`]): this module keeps its state ([`Menu`], pure and
//! tested without a window: the focus ring, the selection, what a press decides) and the flow around it
//! (the saved character, the replace-and-restart rule, the unattended script).
//!
//! Keys: Up / Down move the focus, Left / Right change the focused row, Enter confirms, Esc goes back,
//! letters type the name. The pad: D-pad, A, B. The mouse clicks the screen's own buttons.
//! `WOWSOULS_MENU=0` skips the menu (an unattended run never shows it); the saved character is then
//! used, else a default Human Warrior.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::sprite::{BorderRect, TextureSlicer};
use bevy::time::Real;
use bevy::ui::widget::NodeImageMode;

use crate::avatar::Avatar;
use crate::character::{self, check_name, Character, Class, Dials, Race, Sex, NAME_MAX};
use crate::create::rules::{Rules, Selection};
use crate::settle::Phase;

const IVORY: Color = Color::srgb(0.88, 0.85, 0.77);
const GOLD: Color = Color::srgb(0.92, 0.76, 0.28);
const MUTED: Color = Color::srgb(0.58, 0.56, 0.5);
const WARN: Color = Color::srgb(0.85, 0.42, 0.32);

/// The most rows the title screen has.
const ROWS: usize = 4;

/// The races in the order the left and right keys walk them: the Alliance column, then the Horde's,
/// each top to bottom, as the buttons are laid out.
const RACE_ORDER: [u8; 8] = [1, 3, 4, 7, 2, 5, 6, 8];

/// Whether the menu shows at all: not with `WOWSOULS_MENU=0`, not in an unattended run unless
/// `WOWSOULS_MENU=1` asks for it (a check that photographs or scripts it).
pub fn wanted() -> bool {
    match std::env::var("WOWSOULS_MENU").as_deref() {
        Ok("0") => false,
        Ok("1") => true,
        _ => !crate::run_mode::unattended(),
    }
}

/// `WOWSOULS_MENU_SCRIPT=<s>:<press>,...` presses at real second `s` (an unattended check's hands):
/// `up down left right confirm back backspace create random char:<c> race:<id> sex:<0|1> class:<slot>
/// dial:<i>:<+1|-1>`.
#[derive(Resource, Default)]
struct Script {
    steps: Vec<(f32, Press)>,
    next: usize,
}

fn parse_script(s: &str) -> Vec<(f32, Press)> {
    let mut steps: Vec<(f32, Press)> = s
        .split(',')
        .filter_map(|step| {
            let (t, what) = step.trim().split_once(':')?;
            let what = what.trim();
            let press = match what {
                "up" => Press::Up,
                "down" => Press::Down,
                "left" => Press::Left,
                "right" => Press::Right,
                "confirm" => Press::Confirm,
                "back" => Press::Back,
                "backspace" => Press::Backspace,
                "create" => Press::Create,
                "random" => Press::Randomize,
                other => {
                    if let Some(c) = other.strip_prefix("char:") {
                        Press::Char(c.chars().next()?)
                    } else if let Some(v) = other.strip_prefix("race:") {
                        Press::Race(v.trim().parse().ok()?)
                    } else if let Some(v) = other.strip_prefix("sex:") {
                        Press::Sex(v.trim().parse().ok()?)
                    } else if let Some(v) = other.strip_prefix("class:") {
                        Press::ClassSlot(v.trim().parse().ok()?)
                    } else {
                        let v = other.strip_prefix("dial:")?;
                        let (i, d) = v.split_once(':')?;
                        Press::Dial(i.trim().parse().ok()?, d.trim().parse().ok()?)
                    }
                }
            };
            Some((t.trim().parse().ok()?, press))
        })
        .collect();
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    steps
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    Backspace,
    Char(char),
    /// A race button (its `ChrRaces` id).
    Race(u8),
    /// The male (0) or female (1) button.
    Sex(u8),
    /// The `n`th class button of the race's list.
    ClassSlot(u8),
    /// A dial's arrow: the dial (0 skin .. 4 facial hair) and the direction.
    Dial(u8, i8),
    Randomize,
    /// The Create button itself (Enter on the Create row is the same).
    Create,
}

/// A press the creation screen's buttons send (the screen is [`crate::create::ui`]).
#[derive(Message, Debug, Clone, Copy)]
pub struct MenuPress(pub Press);

/// What a press decided, beyond moving around the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Play the saved character.
    Enter,
    /// The options pages (the system menu's), over the title.
    Options,
    /// A first character, made from scratch.
    Create(Character),
    /// A new character replacing the saved one (its progress goes to a backup folder).
    Replace(Character),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    Create,
}

/// A row of the creation screen's focus ring (keyboard and pad).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Race,
    Class,
    Sex,
    /// A customization dial: 0 skin, 1 face, 2 hair style, 3 hair colour, 4 facial hair.
    Dial(u8),
    Randomize,
    Name,
    Create,
    Back,
}

/// The menu's state: pure, tested without a window.
#[derive(Resource, Debug, Clone)]
pub struct Menu {
    pub screen: Screen,
    /// The focused row: of the title's list, or of the creation screen's [`Menu::ring`].
    pub cursor: usize,
    pub existing: Option<Character>,
    /// The creation screen's selection: race, sex, class, dials, name, facing.
    pub sel: Selection,
    pub rules: Arc<Rules>,
    pub message: String,
    default_name: String,
    /// The name is still the pre-filled one: the first letter typed replaces it.
    name_fresh: bool,
    /// The player has been told a new character replaces the saved one and pressed Create once.
    confirm_replace: bool,
    /// Randomize's dice.
    rng: u64,
}

impl Menu {
    /// A first run (no saved character) opens on the creation screen.
    pub fn new(existing: Option<Character>, default_name: &str, rules: Arc<Rules>) -> Menu {
        let mut sel = Selection::new(&rules);
        sel.name = default_name.to_string();
        Menu {
            screen: if existing.is_some() {
                Screen::Main
            } else {
                Screen::Create
            },
            cursor: 0,
            existing,
            sel,
            rules,
            message: String::new(),
            default_name: default_name.to_string(),
            name_fresh: true,
            confirm_replace: false,
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }

    /// Seeds Randomize's dice (the game side uses the clock; a test keeps the fixed seed).
    pub fn seed(&mut self, seed: u64) {
        self.rng = seed | 1;
    }

    /// The rows of the title screen.
    pub fn main_rows(&self) -> Vec<&'static str> {
        if self.existing.is_some() {
            vec!["Enter the world", "New character", "Options", "Quit"]
        } else {
            vec!["Create character", "Options", "Quit"]
        }
    }

    /// The creation screen's focus ring: the three pickers, the dials (facial hair only where the race
    /// has it), Randomize, the name, Create, Back.
    pub fn ring(&self) -> Vec<Row> {
        let mut rows = vec![Row::Race, Row::Class, Row::Sex];
        for dial in 0..5u8 {
            if dial == 4 && self.rules.facial_hair_hidden(self.sel.race, self.sel.sex) {
                continue;
            }
            rows.push(Row::Dial(dial));
        }
        rows.extend([Row::Randomize, Row::Name, Row::Create, Row::Back]);
        rows
    }

    /// The focused row of the creation screen.
    pub fn focus(&self) -> Row {
        let ring = self.ring();
        ring[self.cursor.min(ring.len() - 1)]
    }

    fn rows(&self) -> usize {
        match self.screen {
            Screen::Main => self.main_rows().len(),
            Screen::Create => self.ring().len(),
        }
    }

    fn open_create(&mut self) {
        self.screen = Screen::Create;
        self.cursor = 0;
        self.message.clear();
        self.sel = Selection::new(&self.rules);
        self.sel.name = self.default_name.clone();
        self.name_fresh = true;
        self.confirm_replace = false;
    }

    fn open_main(&mut self) {
        self.screen = Screen::Main;
        self.cursor = 0;
        self.message.clear();
        self.confirm_replace = false;
    }

    /// One press; `Some` when it decided something outside the menu.
    pub fn press(&mut self, p: Press) -> Option<Outcome> {
        let creating = matches!(p, Press::Create)
            || (self.screen == Screen::Create
                && p == Press::Confirm
                && self.focus() == Row::Create);
        if !creating {
            self.confirm_replace = false;
        }
        match p {
            Press::Up => {
                self.cursor = (self.cursor + self.rows() - 1) % self.rows();
                self.message.clear();
                None
            }
            Press::Down => {
                self.cursor = (self.cursor + 1) % self.rows();
                self.message.clear();
                None
            }
            _ => match self.screen {
                Screen::Main => self.press_main(p),
                Screen::Create => self.press_create(p),
            },
        }
    }

    fn press_main(&mut self, p: Press) -> Option<Outcome> {
        if p != Press::Confirm {
            return None;
        }
        match self.main_rows().get(self.cursor).copied() {
            Some("Enter the world") => Some(Outcome::Enter),
            Some("New character" | "Create character") => {
                self.open_create();
                None
            }
            Some("Options") => Some(Outcome::Options),
            Some("Quit") => Some(Outcome::Quit),
            _ => None,
        }
    }

    fn press_create(&mut self, p: Press) -> Option<Outcome> {
        let rules = self.rules.clone();
        match p {
            Press::Back => {
                self.open_main();
                None
            }
            Press::Race(r) => {
                self.sel.pick_race(r, &rules);
                self.message.clear();
                None
            }
            Press::Sex(s) => {
                self.sel.pick_sex(s, &rules);
                self.message.clear();
                None
            }
            Press::ClassSlot(n) => {
                self.sel.pick_class_slot(usize::from(n), &rules);
                self.message.clear();
                None
            }
            Press::Dial(dial, dir) => {
                self.sel.cycle_dial(usize::from(dial), dir.signum(), &rules);
                self.message.clear();
                None
            }
            Press::Randomize => {
                self.sel.randomize(&mut self.rng, &rules);
                self.message.clear();
                None
            }
            Press::Left => {
                self.step(-1);
                None
            }
            Press::Right => {
                self.step(1);
                None
            }
            Press::Char(c) => {
                // Letters go to the name wherever the focus is, as in the original (its box is always
                // the one with the keyboard); the first one replaces the pre-filled name.
                if c.is_ascii_alphabetic() {
                    if self.name_fresh {
                        self.sel.name.clear();
                        self.name_fresh = false;
                    }
                    if self.sel.name.chars().count() < NAME_MAX {
                        self.sel.name.push(c);
                        self.message.clear();
                    }
                }
                None
            }
            Press::Backspace => {
                if self.name_fresh {
                    self.sel.name.clear();
                    self.name_fresh = false;
                } else {
                    self.sel.name.pop();
                }
                self.message.clear();
                None
            }
            Press::Create => self.make(),
            Press::Confirm => match self.focus() {
                Row::Race | Row::Class | Row::Sex | Row::Dial(_) => {
                    self.step(1);
                    None
                }
                Row::Randomize => {
                    self.sel.randomize(&mut self.rng, &rules);
                    self.message.clear();
                    None
                }
                Row::Name => {
                    let ring = self.ring();
                    self.cursor = ring.iter().position(|r| *r == Row::Create).unwrap_or(0);
                    None
                }
                Row::Create => self.make(),
                Row::Back => {
                    self.open_main();
                    None
                }
            },
            Press::Up | Press::Down => None,
        }
    }

    /// Left / Right on the focused row: the race along its columns, the class along its list, the sex,
    /// or a dial one step.
    fn step(&mut self, dir: i8) {
        let rules = self.rules.clone();
        match self.focus() {
            Row::Race => {
                // Along the columns, over the races that can be chosen (the grey ones are skipped).
                let order: Vec<u8> = RACE_ORDER
                    .iter()
                    .copied()
                    .filter(|&r| rules.race_choosable(r))
                    .collect();
                let at = order.iter().position(|&r| r == self.sel.race).unwrap_or(0) as i32;
                let next = (at + i32::from(dir)).rem_euclid(order.len().max(1) as i32) as usize;
                if let Some(&race) = order.get(next) {
                    self.sel.pick_race(race, &rules);
                }
            }
            Row::Class => {
                let n = rules.classes(self.sel.race).len().max(1) as i32;
                let at = self.sel.class_slot(&rules).unwrap_or(0) as i32;
                self.sel
                    .pick_class_slot((at + i32::from(dir)).rem_euclid(n) as usize, &rules);
            }
            Row::Sex => self.sel.pick_sex(1 - self.sel.sex, &rules),
            Row::Dial(dial) => self.sel.cycle_dial(usize::from(dial), dir, &rules),
            _ => return,
        }
        self.message.clear();
    }

    /// The character the selection describes, if every id is a known one.
    pub fn character(&self, name: String) -> Option<Character> {
        Some(Character {
            name,
            race: Race::from_id(self.sel.race)?,
            class: Class::from_id(self.sel.class)?,
            sex: Sex::from_index(self.sel.sex)?,
            dials: Dials::from_array(self.sel.dials),
        })
    }

    fn make(&mut self) -> Option<Outcome> {
        let name = match check_name(&self.sel.name) {
            Ok(n) => n,
            Err(m) => {
                self.message = m;
                return None;
            }
        };
        let Some(c) = self.character(name).filter(Character::playable) else {
            self.message =
                "Only a male Human Warrior can be made yet; the others are coming.".into();
            return None;
        };
        match &self.existing {
            None => Some(Outcome::Create(c)),
            Some(_) if self.confirm_replace => Some(Outcome::Replace(c)),
            Some(old) => {
                self.confirm_replace = true;
                self.message = format!(
                    "This replaces {}: the old progress goes to a backup folder and the game \
                     restarts. Press Create again to confirm.",
                    old.name
                );
                None
            }
        }
    }

    /// The text of each row of the title screen: `(line, note)`.
    pub fn row_texts(&self) -> Vec<(String, String)> {
        self.main_rows()
            .into_iter()
            .enumerate()
            .map(|(i, r)| (format!("{} {r}", marker(i == self.cursor)), String::new()))
            .collect()
    }

    pub fn title(&self) -> &'static str {
        "WoW Souls"
    }

    pub fn subtitle(&self) -> String {
        match &self.existing {
            Some(c) => format!("{}, {} {}", c.name, c.race.name(), c.class.name()),
            None => "Northshire Valley".to_string(),
        }
    }
}

fn marker(selected: bool) -> &'static str {
    if selected {
        ">"
    } else {
        " "
    }
}

/// The creation screen's sound for what a press changed (the client's `gsCharacterCreation*` sounds):
/// the click of a race, class or sex, the look of a dial, the cancel of Back.
pub fn sound_for(before: &Selection, after: &Selection, p: Press) -> Option<&'static str> {
    if before.race != after.race || before.sex != after.sex || before.class != after.class {
        return Some("gsCharacterCreationClass");
    }
    if before.dials != after.dials {
        return Some("gsCharacterCreationLook");
    }
    match p {
        // The reference always clicks a picker, change or not.
        Press::Race(_) | Press::Sex(_) | Press::ClassSlot(_) => Some("gsCharacterCreationClass"),
        Press::Dial(..) | Press::Randomize => Some("gsCharacterCreationLook"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// The game side.

/// Where the character is saved, and whether that file could not be read (never written over then).
#[derive(Resource)]
struct CharacterFile {
    path: PathBuf,
    blocked: Option<String>,
}

#[derive(Component)]
struct MenuRoot;

/// The title panel (hidden while the creation screen is up).
#[derive(Component)]
struct MenuPanel;

/// The menu's font and parchment, shared with the creation screen.
#[derive(Resource, Clone)]
pub struct MenuStyle {
    pub font: Handle<Font>,
    pub parchment: Option<Handle<Image>>,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Label {
    Title,
    Subtitle,
    Message,
    Footer,
    Row(usize),
    Note(usize),
}

/// A clickable row.
#[derive(Component)]
struct RowButton(usize);

pub fn plugin(app: &mut App) {
    app.insert_resource(Script {
        steps: std::env::var("WOWSOULS_MENU_SCRIPT")
            .map(|s| parse_script(&s))
            .unwrap_or_default(),
        next: 0,
    })
    .add_message::<MenuPress>()
    .add_systems(Startup, load_character)
    .add_systems(OnEnter(Phase::Menu), enter)
    .add_systems(OnExit(Phase::Menu), exit)
    .add_systems(
        Update,
        (input, render, camera)
            .chain()
            .run_if(in_state(Phase::Menu)),
    );
}

pub(crate) fn load_character(mut commands: Commands) {
    let path = character::save_path();
    let (chosen, blocked) = match character::load(&path) {
        Ok(c) => (c, None),
        Err(e) => {
            warn!("menu: the character file cannot be read ({e}); it is not written over");
            (None, Some(e))
        }
    };
    if let Some(c) = &chosen {
        info!(
            "menu: saved character {} the {} {}",
            c.name,
            c.race.name(),
            c.class.name()
        );
    }
    commands.insert_resource(character::Chosen(chosen));
    commands.insert_resource(CharacterFile { path, blocked });
}

fn default_name() -> String {
    std::env::var("WOWSOULS_NAME")
        .ok()
        .and_then(|n| check_name(&n).ok())
        .unwrap_or_else(|| "Adventurer".to_string())
}

fn enter(
    mut commands: Commands,
    chosen: Res<character::Chosen>,
    hud: Option<Res<crate::hud::art::HudArt>>,
    data: Option<Res<crate::create::CreateData>>,
    mut fonts: ResMut<Assets<Font>>,
) {
    let rules = data.as_deref().map(|d| d.rules.clone()).unwrap_or_default();
    let mut menu = Menu::new(chosen.0.clone(), &default_name(), rules);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
    menu.seed(nanos);
    let font = std::fs::read("C:/Windows/Fonts/georgia.ttf")
        .ok()
        .and_then(|b| Font::try_from_bytes(b).ok())
        .map(|f| fonts.add(f))
        .unwrap_or_default();
    let parchment = hud
        .as_deref()
        .map(|h| h.sprite_image("soul_param/bg_parchment_0"))
        .filter(|h| *h != Handle::default());
    commands.insert_resource(MenuStyle {
        font: font.clone(),
        parchment: parchment.clone(),
    });

    let root = commands
        .spawn((
            MenuRoot,
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(100),
        ))
        .id();
    let mut panel = commands.spawn((
        MenuPanel,
        Node {
            position_type: PositionType::Absolute,
            left: px(70),
            top: px(90),
            width: px(560),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(30)),
            row_gap: px(6),
            ..default()
        },
        ChildOf(root),
    ));
    // A dark backing under the parchment (which is translucent), so the small print reads over the
    // lit valley behind.
    panel.insert(BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.86)));
    if let Some(image) = parchment {
        let mut img = ImageNode::new(image);
        img.image_mode = NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect::all(18.0),
            ..default()
        });
        panel.insert(img);
    }
    let panel = panel.id();
    let text = |commands: &mut Commands, parent: Entity, size: f32, color: Color, label: Label| {
        commands
            .spawn((
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: size,
                    ..default()
                },
                TextColor(color),
                label,
                ChildOf(parent),
            ))
            .id()
    };
    text(&mut commands, panel, 36.0, GOLD, Label::Title);
    text(&mut commands, panel, 16.0, MUTED, Label::Subtitle);
    for i in 0..ROWS {
        let row = commands
            .spawn((
                Button,
                RowButton(i),
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(px(8), px(5)),
                    margin: UiRect::top(px(if i == 0 { 14 } else { 0 })),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                ChildOf(panel),
            ))
            .id();
        text(&mut commands, row, 24.0, IVORY, Label::Row(i));
        text(&mut commands, row, 13.0, MUTED, Label::Note(i));
    }
    text(&mut commands, panel, 16.0, WARN, Label::Message);
    text(&mut commands, panel, 13.0, MUTED, Label::Footer);
    commands.insert_resource(menu);
    info!("menu: open");
}

fn exit(mut commands: Commands, roots: Query<Entity, With<MenuRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<Menu>();
    commands.remove_resource::<MenuStyle>();
}

/// The saves a replaced character takes with it to the backup folder. The world state (dead bosses,
/// first kills, held loot) is the old character's history too: a new one must find the valley whole.
const REPLACED_SAVES: [&str; 6] = [
    "character-v1.txt",
    "quests-v1.txt",
    "items-v1.txt",
    "souls-v1.txt",
    "clock-v1.txt",
    crate::world_state::FILE_NAME,
];

/// Archives the saves of the old character into a backup folder next to them (nothing is deleted) and
/// returns the folder. `names` are the files, looked up in `dir`.
pub fn archive_saves(dir: &Path, names: &[&str], stamp: u64) -> std::io::Result<PathBuf> {
    let backup = dir.join(format!("backup-{stamp}"));
    std::fs::create_dir_all(&backup)?;
    for name in names {
        let from = dir.join(name);
        if from.is_file() {
            std::fs::rename(&from, backup.join(name))?;
        }
    }
    Ok(backup)
}

fn presses(
    keys: &ButtonInput<KeyCode>,
    pads: &Query<&Gamepad>,
    typed: &mut MessageReader<KeyboardInput>,
) -> Vec<Press> {
    let mut out = Vec::new();
    let pad = |b: GamepadButton| pads.iter().any(|p| p.just_pressed(b));
    let key = |k: KeyCode| keys.just_pressed(k);
    if key(KeyCode::ArrowUp) || pad(GamepadButton::DPadUp) {
        out.push(Press::Up);
    }
    if key(KeyCode::ArrowDown) || pad(GamepadButton::DPadDown) {
        out.push(Press::Down);
    }
    if key(KeyCode::ArrowLeft) || pad(GamepadButton::DPadLeft) {
        out.push(Press::Left);
    }
    if key(KeyCode::ArrowRight) || pad(GamepadButton::DPadRight) {
        out.push(Press::Right);
    }
    if key(KeyCode::Enter) || key(KeyCode::NumpadEnter) || pad(GamepadButton::South) {
        out.push(Press::Confirm);
    }
    if key(KeyCode::Escape) || pad(GamepadButton::East) {
        out.push(Press::Back);
    }
    // Typing: letters and the backspace, repeating while held.
    for ev in typed.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Backspace => out.push(Press::Backspace),
            Key::Character(s) => out.extend(s.chars().map(Press::Char)),
            _ => {}
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn input(
    mut commands: Commands,
    mut menu: ResMut<Menu>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut typed: MessageReader<KeyboardInput>,
    mut clicks: MessageReader<MenuPress>,
    rows: Query<(&Interaction, &RowButton), Changed<Interaction>>,
    file: Option<Res<CharacterFile>>,
    mut quests: Option<ResMut<crate::quests::QuestGame>>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
    mut audio: Option<ResMut<crate::audio::Audio>>,
    (unattended, real, mut script, mut sys, mut up): (
        Option<Res<crate::run_mode::Unattended>>,
        Res<Time<Real>>,
        ResMut<Script>,
        ResMut<crate::sysmenu::SysMenu>,
        ResMut<crate::settings::MenuUp>,
    ),
) {
    // The options (the system menu's pages) are up over the title: they have the keys.
    if sys.open {
        typed.clear();
        clicks.clear();
        return;
    }
    // A live key never moves an unattended run's menu; only its script does.
    let mut all = if unattended.is_some() {
        typed.clear();
        Vec::new()
    } else {
        presses(&keys, &pads, &mut typed)
    };
    // The creation screen's buttons (mouse), live or not: an unattended run sends none.
    all.extend(clicks.read().map(|c| c.0));
    while let Some(&(t, p)) = script.steps.get(script.next) {
        if t > real.elapsed_secs() {
            break;
        }
        script.next += 1;
        all.push(p);
    }
    for (interaction, row) in &rows {
        if menu.screen != Screen::Main {
            continue;
        }
        match interaction {
            Interaction::Hovered => menu.cursor = row.0.min(menu.rows().saturating_sub(1)),
            Interaction::Pressed => {
                menu.cursor = row.0.min(menu.rows().saturating_sub(1));
                all.push(Press::Confirm);
            }
            Interaction::None => {}
        }
    }
    for p in all {
        let before = menu.sel.clone();
        let on_create = menu.screen == Screen::Create;
        let outcome = menu.press(p);
        if on_create {
            let cue = sound_for(&before, &menu.sel, p).or_else(|| {
                (p == Press::Back || (p == Press::Confirm && menu.screen == Screen::Main))
                    .then_some("gsCharacterCreationCancel")
            });
            if let (Some(cue), Some(audio)) = (cue, audio.as_deref_mut()) {
                audio.play_named(cue, 0.8, "menu");
            }
        }
        let Some(outcome) = outcome else {
            continue;
        };
        let blocked = file.as_deref().and_then(|f| f.blocked.clone());
        if let (Outcome::Create(_) | Outcome::Replace(_), Some(audio)) =
            (&outcome, audio.as_deref_mut())
        {
            audio.play_named("gsCharacterCreationCreateChar", 0.8, "menu");
        }
        match outcome {
            Outcome::Options => {
                info!("menu: options");
                sys.show_options();
                up.0 = true;
                return;
            }
            Outcome::Quit => {
                info!("menu: quit");
                exit.write(AppExit::Success);
                return;
            }
            Outcome::Enter => {
                info!("menu: enter the world");
                next.set(Phase::Playing);
                return;
            }
            Outcome::Create(c) | Outcome::Replace(c) if blocked.is_some() => {
                menu.message = format!(
                    "The saved character cannot be read ({}); nothing was written.",
                    blocked.unwrap_or_default()
                );
                let _ = c;
            }
            Outcome::Create(c) => {
                let path = file
                    .as_deref()
                    .map_or_else(character::save_path, |f| f.path.clone());
                match character::save(&path, &c) {
                    Ok(()) => {
                        info!(
                            "menu: created {} the {} {}",
                            c.name,
                            c.race.name(),
                            c.class.name()
                        );
                        if let Some(g) = quests.as_deref_mut() {
                            g.set_player_name(&c.name);
                        }
                        commands.insert_resource(character::Chosen(Some(c)));
                        next.set(Phase::Playing);
                        return;
                    }
                    Err(e) => menu.message = format!("Could not save the character: {e}"),
                }
            }
            Outcome::Replace(c) => {
                let path = file
                    .as_deref()
                    .map_or_else(character::save_path, |f| f.path.clone());
                let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let result = archive_saves(&dir, &REPLACED_SAVES, stamp).and_then(|backup| {
                    info!(
                        "menu: the old character's saves went to {}",
                        backup.display()
                    );
                    character::save(&path, &c)
                });
                match result {
                    Ok(()) => {
                        // A fresh process reads the fresh saves.
                        match std::env::current_exe()
                            .and_then(|exe| std::process::Command::new(exe).spawn())
                        {
                            Ok(_) => info!("menu: restarting for {}", c.name),
                            Err(e) => warn!("menu: could not restart ({e}); start the game again"),
                        }
                        exit.write(AppExit::Success);
                        return;
                    }
                    Err(e) => menu.message = format!("Could not replace the character: {e}"),
                }
            }
        }
    }
}

/// The title panel's text and row shading; the creation screen draws itself ([`crate::create::ui`]).
fn render(
    menu: Res<Menu>,
    mut labels: Query<(&Label, &mut Text, &mut TextColor)>,
    mut rows: Query<(&RowButton, &mut Node, &mut BackgroundColor), Without<MenuPanel>>,
    mut panel: Query<&mut Node, With<MenuPanel>>,
) {
    if let Ok(mut node) = panel.single_mut() {
        let want = if menu.screen == Screen::Main {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
    if menu.screen != Screen::Main {
        return;
    }
    let texts = menu.row_texts();
    for (label, mut text, mut color) in &mut labels {
        let (value, tint) = match *label {
            Label::Title => (menu.title().to_string(), GOLD),
            Label::Subtitle => (menu.subtitle(), MUTED),
            Label::Message => (menu.message.clone(), WARN),
            Label::Footer => ("Up / Down  Move      Enter / A  Select".to_string(), MUTED),
            Label::Row(i) => (
                texts.get(i).map(|t| t.0.clone()).unwrap_or_default(),
                if i == menu.cursor { GOLD } else { IVORY },
            ),
            Label::Note(i) => (texts.get(i).map(|t| t.1.clone()).unwrap_or_default(), MUTED),
        };
        if text.0 != value {
            text.0 = value;
        }
        if color.0 != tint {
            color.0 = tint;
        }
    }
    for (row, mut node, mut bg) in &mut rows {
        let want = if row.0 < texts.len() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
        let shade = if row.0 == menu.cursor {
            Color::srgba(1.0, 0.9, 0.6, 0.08)
        } else {
            Color::NONE
        };
        if bg.0 != shade {
            bg.0 = shade;
        }
    }
}

/// The title's camera circles the character slowly, from the front, the character to the right of the
/// panel. The creation screen frames its own preview ([`crate::create::preview`]).
fn camera(
    menu: Res<Menu>,
    time: Res<Time<Real>>,
    avatar: Query<&Transform, (With<Avatar>, Without<benilla_world::view::WorldCamera>)>,
    mut cam: Query<&mut Transform, With<benilla_world::view::WorldCamera>>,
) {
    if menu.screen != Screen::Main {
        return;
    }
    let (Ok(body), Ok(mut cam)) = (avatar.single(), cam.single_mut()) else {
        return;
    };
    let swing = (time.elapsed_secs() * 0.22).sin() * 0.35;
    let facing = body.rotation * Quat::from_rotation_y(swing);
    let forward = facing * Vec3::NEG_Z;
    let right = facing * Vec3::X;
    let at = body.translation + Vec3::Y * 1.0 + right * 0.85;
    let from = body.translation + forward * 3.3 + Vec3::Y * 1.45;
    *cam = Transform::from_translation(from).looking_at(at, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Arc<Rules> {
        Arc::new(Rules::standard())
    }

    fn fresh() -> Menu {
        Menu::new(None, "Adventurer", rules())
    }

    fn saved() -> Menu {
        Menu::new(
            Some(Character::human_warrior("Jacob")),
            "Adventurer",
            rules(),
        )
    }

    /// Moves the creation screen's focus to `row`.
    fn focus(m: &mut Menu, row: Row) {
        m.cursor = m.ring().iter().position(|r| *r == row).expect("row");
    }

    #[test]
    fn a_script_is_sorted_and_malformed_steps_are_dropped() {
        assert_eq!(
            parse_script("5:confirm, 2:down,bogus, x:up, 3:char:j, 4:char:"),
            vec![
                (2.0, Press::Down),
                (3.0, Press::Char('j')),
                (5.0, Press::Confirm)
            ]
        );
    }

    #[test]
    fn the_script_picks_races_sexes_classes_and_dials() {
        assert_eq!(
            parse_script(
                "1:race:2, 2:sex:1, 3:class:4, 4:dial:2:+1, 5:dial:0:-1, 6:random, 7:create, \
                 8:race:x, 9:dial:1"
            ),
            vec![
                (1.0, Press::Race(2)),
                (2.0, Press::Sex(1)),
                (3.0, Press::ClassSlot(4)),
                (4.0, Press::Dial(2, 1)),
                (5.0, Press::Dial(0, -1)),
                (6.0, Press::Randomize),
                (7.0, Press::Create),
            ]
        );
    }

    #[test]
    fn a_first_run_opens_on_the_creation_screen_and_a_saved_character_on_the_title() {
        assert_eq!(fresh().screen, Screen::Create);
        let m = saved();
        assert_eq!(m.screen, Screen::Main);
        assert_eq!(
            m.main_rows(),
            ["Enter the world", "New character", "Options", "Quit"]
        );
        assert_eq!(fresh().main_rows(), ["Create character", "Options", "Quit"]);
        assert!(m.subtitle().contains("Jacob") && m.subtitle().contains("Warrior"));
    }

    #[test]
    fn the_cursor_wraps_over_the_title_rows() {
        let mut m = saved();
        m.press(Press::Up);
        assert_eq!(m.cursor, 3, "up from the first row lands on the last");
        m.press(Press::Down);
        assert_eq!(m.cursor, 0);
    }

    #[test]
    fn enter_the_world_quit_and_new_character_follow_their_rows() {
        let mut m = saved();
        assert_eq!(m.press(Press::Confirm), Some(Outcome::Enter));
        let mut m = saved();
        m.press(Press::Down);
        assert_eq!(m.press(Press::Confirm), None);
        assert_eq!(
            m.screen,
            Screen::Create,
            "New character opens the creation screen"
        );
        let mut m = saved();
        m.press(Press::Up);
        assert_eq!(m.press(Press::Confirm), Some(Outcome::Quit));
    }

    #[test]
    fn the_options_row_asks_for_the_options_pages() {
        let mut m = saved();
        m.press(Press::Down);
        m.press(Press::Down);
        assert_eq!(m.press(Press::Confirm), Some(Outcome::Options));
        assert_eq!(m.screen, Screen::Main, "the title stays under them");
        let mut m = fresh();
        m.screen = Screen::Main;
        m.cursor = 1;
        assert_eq!(m.press(Press::Confirm), Some(Outcome::Options));
    }

    #[test]
    fn the_focus_ring_walks_the_rows_and_wraps() {
        let mut m = fresh();
        assert_eq!(m.ring().len(), 12);
        assert_eq!(m.focus(), Row::Race);
        m.press(Press::Up);
        assert_eq!(
            m.focus(),
            Row::Back,
            "up from the first row lands on the last"
        );
        m.press(Press::Down);
        assert_eq!(m.focus(), Row::Race);
        for expected in [
            Row::Class,
            Row::Sex,
            Row::Dial(0),
            Row::Dial(1),
            Row::Dial(2),
            Row::Dial(3),
            Row::Dial(4),
            Row::Randomize,
            Row::Name,
            Row::Create,
            Row::Back,
        ] {
            m.press(Press::Down);
            assert_eq!(m.focus(), expected);
        }
    }

    #[test]
    fn a_name_is_typed_with_letters_only_up_to_twelve_and_deleted_with_backspace() {
        let mut m = fresh();
        assert_eq!(m.sel.name, "Adventurer");
        // The pre-filled name goes at the first letter, wherever the focus is.
        m.press(Press::Char('j'));
        assert_eq!(m.sel.name, "j");
        for c in "acob".chars() {
            m.press(Press::Char(c));
        }
        m.press(Press::Char('7'));
        m.press(Press::Char(' '));
        assert_eq!(m.sel.name, "jacob", "digits and spaces are not letters");
        m.press(Press::Backspace);
        assert_eq!(m.sel.name, "jaco");
        for _ in 0..30 {
            m.press(Press::Char('x'));
        }
        assert_eq!(m.sel.name.chars().count(), NAME_MAX);
    }

    #[test]
    fn backspace_clears_a_pre_filled_name_at_once() {
        let mut m = fresh();
        m.press(Press::Backspace);
        assert_eq!(m.sel.name, "");
        m.press(Press::Char('a'));
        m.press(Press::Backspace);
        assert_eq!(m.sel.name, "");
    }

    #[test]
    fn left_and_right_change_the_focused_row() {
        let mut m = fresh();
        // Race: along the Alliance column, then the Horde's.
        m.press(Press::Right);
        assert_eq!(m.sel.race, 3, "dwarf follows human");
        m.press(Press::Left);
        m.press(Press::Left);
        assert_eq!(m.sel.race, 8, "left from human wraps to the troll");
        // Class: along the race's list.
        m.sel.pick_race(1, &m.rules.clone());
        focus(&mut m, Row::Class);
        m.press(Press::Right);
        assert_eq!(m.sel.class, 2, "paladin follows warrior");
        m.press(Press::Left);
        m.press(Press::Left);
        assert_eq!(m.sel.class, 9, "warlock is the human's last class");
        // Sex toggles.
        focus(&mut m, Row::Sex);
        m.press(Press::Right);
        assert_eq!(m.sel.sex, 1);
        m.press(Press::Left);
        assert_eq!(m.sel.sex, 0);
        // A dial steps and wraps.
        focus(&mut m, Row::Dial(0));
        m.press(Press::Left);
        assert_eq!(m.sel.dials[0], m.rules.ranges(1, 0)[0] - 1);
        m.press(Press::Right);
        assert_eq!(m.sel.dials[0], 0);
    }

    #[test]
    fn the_grey_races_are_skipped_by_the_keys_and_ignored_by_the_mouse() {
        let only = Arc::new(Rules::standard().only_races(&[1, 6]));
        let mut m = Menu::new(None, "Adventurer", only);
        m.press(Press::Race(2));
        assert_eq!(m.sel.race, 1, "the orc is grey");
        m.press(Press::Right);
        assert_eq!(
            m.sel.race, 6,
            "right from the human lands on the tauren, the next that can be chosen"
        );
        m.press(Press::Right);
        assert_eq!(m.sel.race, 1, "and wraps");
        // With the human alone the key does nothing and nothing breaks.
        let mut m = Menu::new(
            None,
            "Adventurer",
            Arc::new(Rules::standard().only_races(&[1])),
        );
        m.press(Press::Right);
        m.press(Press::Left);
        assert_eq!(m.sel.race, 1);
    }

    #[test]
    fn a_click_on_a_picker_chooses_it_and_a_race_resets_the_class() {
        let mut m = fresh();
        m.press(Press::ClassSlot(3));
        assert_eq!(m.sel.class, 5, "priest");
        m.press(Press::Race(2));
        assert_eq!((m.sel.race, m.sel.class), (2, 1), "orc, its first class");
        m.press(Press::Sex(1));
        assert_eq!(m.sel.sex, 1);
        m.press(Press::Dial(1, 1));
        assert_eq!(m.sel.dials[1], 1);
        m.press(Press::Randomize);
        let counts = m.rules.ranges(2, 1);
        for (d, n) in m.sel.dials.iter().zip(counts) {
            assert!(*d < n);
        }
    }

    #[test]
    fn enter_on_the_rows_acts_and_enter_on_the_name_moves_to_create() {
        let mut m = fresh();
        m.press(Press::Confirm); // race: next
        assert_eq!(m.sel.race, 3);
        focus(&mut m, Row::Randomize);
        let before = m.sel.dials;
        m.press(Press::Confirm);
        assert_ne!(m.sel.dials, before, "randomize drew new dials");
        focus(&mut m, Row::Name);
        m.press(Press::Confirm);
        assert_eq!(m.focus(), Row::Create);
    }

    #[test]
    fn creating_checks_the_name_and_returns_the_character() {
        let mut m = fresh();
        m.sel.name = "J".into();
        assert_eq!(m.press(Press::Create), None);
        assert!(m.message.contains("at least"), "{}", m.message);
        m.sel.name = "gARRICK".into();
        m.sel.dials = [4, 3, 2, 1, 0];
        let mut expected = Character::human_warrior("Garrick");
        expected.dials = Dials::from_array([4, 3, 2, 1, 0]);
        assert_eq!(m.press(Press::Create), Some(Outcome::Create(expected)));
    }

    #[test]
    fn only_a_male_human_warrior_can_be_created_and_the_menu_says_so() {
        for (race, slot, sex) in [(2u8, 0u8, 0u8), (1, 1, 0), (1, 0, 1)] {
            let mut m = fresh();
            m.press(Press::Race(race));
            m.press(Press::ClassSlot(slot));
            m.press(Press::Sex(sex));
            assert_eq!(m.press(Press::Create), None, "{race}/{slot}/{sex}");
            assert!(
                m.message.contains("Human Warrior"),
                "{race}/{slot}/{sex}: {}",
                m.message
            );
        }
    }

    #[test]
    fn replacing_a_saved_character_asks_twice() {
        let mut m = saved();
        m.press(Press::Down);
        m.press(Press::Confirm); // New character
        assert_eq!(m.screen, Screen::Create);
        assert_eq!(m.press(Press::Create), None, "the first press only warns");
        assert!(m.message.contains("replaces Jacob"), "{}", m.message);
        assert_eq!(
            m.press(Press::Create),
            Some(Outcome::Replace(Character {
                dials: Dials::from_array([0; 5]),
                ..Character::human_warrior("Adventurer")
            })),
            "the second confirms"
        );
        // Moving away in between cancels the warning.
        let mut m = saved();
        m.press(Press::Down);
        m.press(Press::Confirm);
        m.press(Press::Create);
        m.press(Press::Down);
        assert_eq!(m.press(Press::Create), None, "warned again, not replaced");
        // Enter on the Create row is the same press as the button.
        let mut m = saved();
        m.press(Press::Down);
        m.press(Press::Confirm);
        focus(&mut m, Row::Create);
        assert_eq!(m.press(Press::Confirm), None);
        assert!(m.confirm_replace);
        assert!(matches!(m.press(Press::Confirm), Some(Outcome::Replace(_))));
    }

    #[test]
    fn back_leaves_the_creation_screen() {
        let mut m = fresh();
        m.press(Press::Back);
        assert_eq!(m.screen, Screen::Main);
        let mut m = fresh();
        focus(&mut m, Row::Back);
        m.press(Press::Confirm);
        assert_eq!(m.screen, Screen::Main);
    }

    #[test]
    fn a_new_character_screen_starts_fresh_each_time() {
        let mut m = saved();
        m.press(Press::Down);
        m.press(Press::Confirm);
        m.press(Press::Race(6));
        m.press(Press::Char('z'));
        m.press(Press::Back);
        m.press(Press::Down);
        m.press(Press::Confirm);
        assert_eq!(m.sel, {
            let mut s = Selection::new(&m.rules);
            s.name = "Adventurer".into();
            s
        });
    }

    #[test]
    fn the_sounds_follow_what_changed() {
        let r = Rules::standard();
        let a = Selection::new(&r);
        let mut b = a.clone();
        b.pick_race(2, &r);
        assert_eq!(
            sound_for(&a, &b, Press::Race(2)),
            Some("gsCharacterCreationClass")
        );
        let mut c = a.clone();
        c.cycle_dial(0, 1, &r);
        assert_eq!(
            sound_for(&a, &c, Press::Dial(0, 1)),
            Some("gsCharacterCreationLook")
        );
        assert_eq!(
            sound_for(&a, &a, Press::Race(1)),
            Some("gsCharacterCreationClass"),
            "a picker clicks even when nothing changes"
        );
        assert_eq!(sound_for(&a, &a, Press::Char('x')), None);
    }

    #[test]
    fn the_old_saves_are_moved_to_a_backup_not_deleted() {
        let dir = std::env::temp_dir().join(format!("wowsouls-menu-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("quests-v1.txt"), "q").unwrap();
        std::fs::write(dir.join("items-v1.txt"), "i").unwrap();
        let backup =
            archive_saves(&dir, &["quests-v1.txt", "items-v1.txt", "souls-v1.txt"], 7).unwrap();
        assert!(!dir.join("quests-v1.txt").exists() && !dir.join("items-v1.txt").exists());
        assert_eq!(
            std::fs::read_to_string(backup.join("quests-v1.txt")).unwrap(),
            "q"
        );
        assert_eq!(
            std::fs::read_to_string(backup.join("items-v1.txt")).unwrap(),
            "i"
        );
        assert!(
            !backup.join("souls-v1.txt").exists(),
            "a file that was not there is skipped"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A replaced character must not inherit the old one's dead bosses (final review, Critical): the
    /// world state (slain, first kills, held loot) goes to the backup with the rest.
    #[test]
    fn replacing_a_character_archives_every_save_including_the_world_state() {
        for name in [
            "character-v1.txt",
            "quests-v1.txt",
            "items-v1.txt",
            "souls-v1.txt",
            "clock-v1.txt",
            crate::world_state::FILE_NAME,
        ] {
            assert!(REPLACED_SAVES.contains(&name), "{name}");
        }
        assert_eq!(REPLACED_SAVES.len(), 6);
    }
}
