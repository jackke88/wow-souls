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

use std::path::PathBuf;
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
pub(crate) mod roster;

use crate::hud::theme::{GOLD, IVORY, MUTED};
const WARN: Color = Color::srgb(0.85, 0.42, 0.32);

/// The most rows the title screen has: every character the list can hold, New character, Options
/// and Quit.
const ROWS: usize = crate::saves::MAX_CHARACTERS + 3;

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
/// `up down left right confirm back backspace create delete random char:<c> race:<id> sex:<0|1>
/// class:<slot> dial:<i>:<+1|-1>`.
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
                "play" => Press::Play,
                "back" => Press::Back,
                "backspace" => Press::Backspace,
                "create" => Press::Create,
                "delete" => Press::Delete,
                "random" => Press::Randomize,
                "name" => Press::NameFocus,
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
    /// Start the chosen character, regardless of which menu command has keyboard focus.
    Play,
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
    NameFocus,
    /// The Create button itself (Enter on the Create row is the same).
    Create,
    /// The Delete key (the pad's West button) on a character of the title's list.
    Delete,
}

/// A press the creation screen's buttons send (the screen is [`crate::create::ui`]).
#[derive(Message, Debug, Clone, Copy)]
pub struct MenuPress(pub Press);

/// What a press decided, beyond moving around the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Play the character this run loaded.
    Enter,
    /// Play another character: every save is read at start, so the game relaunches on its slot.
    Switch(u32),
    /// The options pages (the system menu's), over the title.
    Options,
    /// The first character, made in this run (nobody was saved).
    Create(Character),
    /// A new character beside the saved ones: a slot of its own, the game relaunches on it.
    Add(Character),
    /// Put a character's saves in a backup folder (nothing is erased).
    Delete(u32),
    Quit,
}

/// A character of the title's list: its slot number and what the creation menu saved of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotInfo {
    pub id: u32,
    pub character: Character,
}

/// A row of the title's list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainRow {
    /// A saved character: an index into [`Menu::slots`].
    Slot(usize),
    New,
    Options,
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
    /// The saved characters, in slot order.
    pub slots: Vec<SlotInfo>,
    /// The slot this run loaded: Enter on it plays on, any other is a relaunch.
    pub active: u32,
    /// The creation screen's selection: race, sex, class, dials, name, facing.
    pub sel: Selection,
    pub rules: Arc<Rules>,
    pub message: String,
    default_name: String,
    /// The name is still the pre-filled one: the first letter typed replaces it.
    name_fresh: bool,
    /// The slot whose Delete was pressed once (the second press does it).
    pending_delete: Option<u32>,
    last_character: u32,
    /// Randomize's dice.
    rng: u64,
}

impl Menu {
    /// A first run (nobody saved) opens on the creation screen; otherwise the title, on the
    /// character this run loaded.
    pub fn new(slots: Vec<SlotInfo>, active: u32, default_name: &str, rules: Arc<Rules>) -> Menu {
        let mut sel = Selection::new(&rules);
        sel.name = default_name.to_string();
        let cursor = slots.iter().position(|s| s.id == active).unwrap_or(0);
        Menu {
            screen: if slots.is_empty() {
                Screen::Create
            } else {
                Screen::Main
            },
            cursor,
            slots,
            active,
            sel,
            rules,
            message: String::new(),
            default_name: default_name.to_string(),
            name_fresh: true,
            pending_delete: None,
            last_character: active,
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }

    /// Seeds Randomize's dice (the game side uses the clock; a test keeps the fixed seed).
    pub fn seed(&mut self, seed: u64) {
        self.rng = seed | 1;
    }

    /// The rows of the title screen: each saved character, New character (not on a full list),
    /// Options, Quit. With nobody saved the first row is "Create character".
    pub fn main_rows(&self) -> Vec<MainRow> {
        let mut rows: Vec<MainRow> = (0..self.slots.len()).map(MainRow::Slot).collect();
        if self.slots.len() < crate::saves::MAX_CHARACTERS {
            rows.push(MainRow::New);
        }
        rows.extend([MainRow::Options, MainRow::Quit]);
        rows
    }

    /// The words of [`Menu::main_rows`].
    pub fn main_labels(&self) -> Vec<String> {
        self.main_rows()
            .into_iter()
            .map(|row| match row {
                MainRow::Slot(i) => self.slots.get(i).map_or_else(String::new, |s| {
                    format!(
                        "{}, {} {}",
                        s.character.name,
                        s.character.race.name(),
                        s.character.class.name()
                    )
                }),
                MainRow::New if self.slots.is_empty() => "Create character".to_string(),
                MainRow::New => "New character".to_string(),
                MainRow::Options => "Options".to_string(),
                MainRow::Quit => "Quit".to_string(),
            })
            .collect()
    }

    /// A character taken off the list (its saves are in a backup folder): the cursor stays on the
    /// list, and with nobody left the creation screen opens as on a first run.
    pub fn remove_slot(&mut self, id: u32) {
        self.slots.retain(|s| s.id != id);
        self.pending_delete = None;
        if self.slots.is_empty() {
            self.open_create();
        } else {
            self.cursor = self.cursor.min(self.rows().saturating_sub(1));
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
        self.pending_delete = None;
    }

    fn open_main(&mut self) {
        self.screen = Screen::Main;
        self.cursor = self
            .slots
            .iter()
            .position(|s| s.id == self.last_character)
            .unwrap_or(0);
        self.message.clear();
        self.pending_delete = None;
    }

    /// One press; `Some` when it decided something outside the menu.
    pub fn press(&mut self, p: Press) -> Option<Outcome> {
        if p != Press::Delete {
            // Anything but a second Delete takes the warning back.
            self.pending_delete = None;
        }
        let result = match p {
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
        };
        self.remember_character();
        result
    }

    fn remember_character(&mut self) {
        if self.screen == Screen::Main {
            if let Some(MainRow::Slot(i)) = self.main_rows().get(self.cursor) {
                if let Some(slot) = self.slots.get(*i) {
                    self.last_character = slot.id;
                }
            }
        }
    }

    fn press_main(&mut self, p: Press) -> Option<Outcome> {
        let row = self.main_rows().get(self.cursor).copied();
        match (p, row) {
            (Press::Play, _) => self.shown_slot().map(|s| {
                if s.id == self.active {
                    Outcome::Enter
                } else {
                    Outcome::Switch(s.id)
                }
            }),
            (Press::Confirm, Some(MainRow::Slot(i))) => {
                let id = self.slots.get(i)?.id;
                Some(if id == self.active {
                    Outcome::Enter
                } else {
                    Outcome::Switch(id)
                })
            }
            (Press::Confirm, Some(MainRow::New)) => {
                self.open_create();
                None
            }
            (Press::Confirm, Some(MainRow::Options)) => Some(Outcome::Options),
            (Press::Confirm, Some(MainRow::Quit)) => Some(Outcome::Quit),
            (Press::Delete, Some(MainRow::Slot(i))) => {
                let slot = self.slots.get(i)?;
                let (id, name) = (slot.id, slot.character.name.clone());
                if self.pending_delete == Some(id) {
                    self.pending_delete = None;
                    return Some(Outcome::Delete(id));
                }
                self.pending_delete = Some(id);
                self.message = format!(
                    "Delete {name}? The progress goes to a backup folder, nothing is erased. \
                     Press Delete again."
                );
                None
            }
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
                    self.cursor = self
                        .ring()
                        .iter()
                        .position(|r| *r == Row::Name)
                        .unwrap_or(0);
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
                self.cursor = self
                    .ring()
                    .iter()
                    .position(|r| *r == Row::Name)
                    .unwrap_or(0);
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
            Press::NameFocus => {
                self.cursor = self
                    .ring()
                    .iter()
                    .position(|r| *r == Row::Name)
                    .unwrap_or(0);
                None
            }
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
            Press::Up | Press::Down | Press::Delete | Press::Play => None,
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
                "Choose a male Human Warrior or Mage. Other characters are not available yet."
                    .into();
            return None;
        };
        if self.slots.is_empty() {
            Some(Outcome::Create(c))
        } else if self.slots.len() >= crate::saves::MAX_CHARACTERS {
            self.message = format!(
                "The list holds {} characters: delete one first.",
                crate::saves::MAX_CHARACTERS
            );
            None
        } else {
            Some(Outcome::Add(c))
        }
    }

    /// The text of each row of the title screen: `(line, note)`; a character's note says it is the
    /// one in the world behind the panel.
    pub fn row_texts(&self) -> Vec<(String, String)> {
        let rows = self.main_rows();
        self.main_labels()
            .into_iter()
            .zip(rows)
            .enumerate()
            .map(|(i, (label, row))| {
                let note = match row {
                    MainRow::Slot(s) if self.slots.get(s).is_some_and(|s| s.id == self.active) => {
                        "In the valley now".to_string()
                    }
                    _ => String::new(),
                };
                (format!("{} {label}", marker(i == self.cursor)), note)
            })
            .collect()
    }

    pub fn title(&self) -> &'static str {
        "WoW Souls"
    }

    /// The highlighted character, else where the world is.
    pub fn subtitle(&self) -> String {
        let shown = match self.main_rows().get(self.cursor) {
            Some(MainRow::Slot(i)) => self.slots.get(*i),
            _ => self.slots.iter().find(|s| s.id == self.active),
        };
        match shown {
            Some(s) => {
                let c = &s.character;
                format!("{}, {} {}", c.name, c.race.name(), c.class.name())
            }
            None => "Northshire Valley".to_string(),
        }
    }

    pub(crate) fn shown_slot(&self) -> Option<&SlotInfo> {
        match self.main_rows().get(self.cursor) {
            Some(MainRow::Slot(i)) => self.slots.get(*i),
            _ => self
                .slots
                .iter()
                .find(|s| s.id == self.last_character)
                .or_else(|| self.slots.first()),
        }
    }

    pub(crate) fn preview_selection(&self) -> Selection {
        if self.screen == Screen::Create {
            return self.sel.clone();
        }
        let Some(slot) = self.shown_slot() else {
            return self.sel.clone();
        };
        let c = &slot.character;
        Selection {
            race: c.race.id(),
            sex: u8::from(c.sex == Sex::Female),
            class: c.class.id(),
            dials: [
                c.dials.skin,
                c.dials.face,
                c.dials.hair_style,
                c.dials.hair_color,
                c.dials.facial_hair,
            ],
            name: c.name.clone(),
            yaw: 0.0,
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

/// Where the loaded character is saved, and whether that file could not be read (never written
/// over then).
#[derive(Resource)]
struct CharacterFile {
    path: PathBuf,
    blocked: Option<String>,
}

/// The characters saved, read at start: the title's list.
#[derive(Resource, Clone, Default)]
struct SavedList {
    slots: Vec<SlotInfo>,
    active: u32,
}

/// The saves folder (`%LOCALAPPDATA%\WoWSouls`) the slots live under.
fn saves_root() -> PathBuf {
    crate::saves::root()
}

/// Starts the game again (every save is read at start), on `slot` or, with `None`, on the slot
/// played last; whether it did. The child must not inherit this run's own `WOWSOULS_SLOT`.
fn relaunch(slot: Option<u32>) -> bool {
    if crate::run_mode::unattended() {
        // A check ends here: a child would run the same script again, and again.
        info!("menu: unattended, not relaunching (slot {slot:?})");
        return true;
    }
    match std::env::current_exe().and_then(|exe| {
        let mut command = std::process::Command::new(exe);
        match slot {
            Some(id) => command.env("WOWSOULS_SLOT", id.to_string()),
            None => command.env_remove("WOWSOULS_SLOT"),
        };
        command.spawn()
    }) {
        Ok(_) => true,
        Err(e) => {
            warn!("menu: could not restart ({e}); start the game again");
            false
        }
    }
}

fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
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
    roster::plugin(app);
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
    // The title's list: every slot with a character (`WOWSOULS_CHARACTER` names one file, the list
    // is then that one).
    let active = crate::saves::active();
    let slots: Vec<SlotInfo> = if std::env::var_os("WOWSOULS_CHARACTER").is_some() {
        chosen
            .iter()
            .map(|c| SlotInfo {
                id: active,
                character: c.clone(),
            })
            .collect()
    } else {
        let root = saves_root();
        crate::saves::slots(&root)
            .into_iter()
            .filter_map(|id| {
                let file = crate::saves::slot_dir(&root, id).join(crate::saves::CHARACTER_FILE);
                match character::load(&file) {
                    Ok(Some(character)) => Some(SlotInfo { id, character }),
                    Ok(None) => None,
                    Err(e) => {
                        warn!("menu: slot {id} cannot be read ({e}); it is left out of the list");
                        None
                    }
                }
            })
            .collect()
    };
    info!(
        "menu: {} saved character(s), slot {active} loaded",
        slots.len()
    );
    commands.insert_resource(character::Chosen(chosen));
    commands.insert_resource(CharacterFile { path, blocked });
    commands.insert_resource(SavedList { slots, active });
}

fn default_name() -> String {
    std::env::var("WOWSOULS_NAME")
        .ok()
        .and_then(|n| check_name(&n).ok())
        .unwrap_or_else(|| "Adventurer".to_string())
}

fn enter(
    mut commands: Commands,
    saved: Res<SavedList>,
    hud: Option<Res<crate::hud::art::HudArt>>,
    data: Option<Res<crate::create::CreateData>>,
    mut fonts: ResMut<Assets<Font>>,
) {
    let rules = data.as_deref().map(|d| d.rules.clone()).unwrap_or_default();
    let mut menu = Menu::new(saved.slots.clone(), saved.active, &default_name(), rules);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
    menu.seed(nanos);
    let font = crate::hud::font::project_font(&mut fonts);
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
    if crate::hud::theme::enabled() {
        crate::hud::theme::dress(&mut panel);
    } else if let Some(image) = parchment {
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
    if crate::hud::theme::enabled() {
        roster::spawn(&mut commands, root, &font, &menu);
    }
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
    if key(KeyCode::Delete) || pad(GamepadButton::West) {
        out.push(Press::Delete);
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
    rows: Query<(&Interaction, &RowButton, Has<roster::RosterRow>), Changed<Interaction>>,
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
    for (interaction, row, roster_row) in &rows {
        if menu.screen != Screen::Main {
            continue;
        }
        match interaction {
            Interaction::Hovered if !roster_row => {
                menu.cursor = row.0.min(menu.rows().saturating_sub(1))
            }
            Interaction::Pressed => {
                menu.cursor = row.0.min(menu.rows().saturating_sub(1));
                if roster_row {
                    menu.pending_delete = None;
                    menu.message.clear();
                    menu.remember_character();
                } else {
                    all.push(Press::Confirm);
                }
            }
            _ => {}
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
        if let (Outcome::Create(_) | Outcome::Add(_), Some(audio)) =
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
                // The slot played last is the one the game opens on next time.
                if let Err(e) = crate::saves::set_active(&saves_root(), menu.active) {
                    warn!("menu: could not remember the slot ({e})");
                }
                next.set(Phase::Playing);
                return;
            }
            Outcome::Switch(id) => {
                let root = saves_root();
                match crate::saves::set_active(&root, id) {
                    Ok(()) if relaunch(Some(id)) => {
                        info!("menu: switching to slot {id}, restarting");
                        exit.write(AppExit::Success);
                        return;
                    }
                    Ok(()) => menu.message = "Could not restart the game; start it again.".into(),
                    Err(e) => menu.message = format!("Could not switch character: {e}"),
                }
            }
            Outcome::Create(c) if blocked.is_some() => {
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
                        if let Err(e) = crate::saves::set_active(&saves_root(), menu.active) {
                            warn!("menu: could not remember the slot ({e})");
                        }
                        if let Some(g) = quests.as_deref_mut() {
                            g.set_player_character(&c);
                        }
                        commands.insert_resource(character::Chosen(Some(c)));
                        next.set(Phase::Playing);
                        return;
                    }
                    Err(e) => menu.message = format!("Could not save the character: {e}"),
                }
            }
            Outcome::Add(c) => {
                // A slot of its own; the saves of this run stay where they are.
                let root = saves_root();
                let Some(id) = crate::saves::next_id(&root) else {
                    menu.message = format!(
                        "The list holds {} characters: delete one first.",
                        crate::saves::MAX_CHARACTERS
                    );
                    continue;
                };
                let file = crate::saves::slot_dir(&root, id).join(crate::saves::CHARACTER_FILE);
                let result =
                    character::save(&file, &c).and_then(|()| crate::saves::set_active(&root, id));
                match result {
                    Ok(()) if relaunch(Some(id)) => {
                        info!("menu: {} is slot {id}, restarting", c.name);
                        exit.write(AppExit::Success);
                        return;
                    }
                    Ok(()) => {
                        menu.message = "Could not restart the game; start it again.".into();
                    }
                    Err(e) => menu.message = format!("Could not save the character: {e}"),
                }
            }
            Outcome::Delete(id) => {
                let root = saves_root();
                match crate::saves::archive_slot(&root, id, unix_seconds()) {
                    Ok(backup) => {
                        info!("menu: slot {id} went to {}", backup.display());
                        if id == menu.active {
                            // Its saves are in memory and would be written back: start again, on the
                            // slot played last, or on a first run.
                            relaunch(None);
                            exit.write(AppExit::Success);
                            return;
                        }
                        menu.remove_slot(id);
                        menu.message = "The progress is in a backup folder.".into();
                    }
                    Err(e) => menu.message = format!("Could not delete the character: {e}"),
                }
            }
        }
    }
}

/// The title panel's text and row shading; the creation screen draws itself ([`crate::create::ui`]).
fn render(
    menu: Res<Menu>,
    mut labels: Query<(&Label, &mut Text, &mut TextColor)>,
    mut rows: Query<
        (&RowButton, &mut Node, &mut BackgroundColor),
        (
            Without<MenuPanel>,
            Without<roster::RosterRow>,
            Without<roster::Command>,
        ),
    >,
    mut panel: Query<&mut Node, With<MenuPanel>>,
) {
    if let Ok(mut node) = panel.single_mut() {
        let want = if menu.screen == Screen::Main && !crate::hud::theme::enabled() {
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
            Label::Footer => (
                if menu.slots.is_empty() {
                    "Up / Down  Move      Enter / A  Select".to_string()
                } else {
                    "Up / Down  Move      Enter / A  Select      Delete / X  Delete character"
                        .to_string()
                },
                MUTED,
            ),
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
            crate::hud::theme::SELECTED
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
    if menu.screen != Screen::Main || crate::hud::theme::enabled() {
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

    fn slot(id: u32, name: &str) -> SlotInfo {
        SlotInfo {
            id,
            character: Character::human_warrior(name),
        }
    }

    fn fresh() -> Menu {
        Menu::new(Vec::new(), 1, "Adventurer", rules())
    }

    fn saved() -> Menu {
        Menu::new(vec![slot(1, "Jacob")], 1, "Adventurer", rules())
    }

    /// Two characters, the second (Garrick, slot 2) the one this process loaded.
    fn two() -> Menu {
        Menu::new(
            vec![slot(1, "Jacob"), slot(2, "Garrick")],
            2,
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
            [
                MainRow::Slot(0),
                MainRow::New,
                MainRow::Options,
                MainRow::Quit
            ]
        );
        assert_eq!(
            m.main_labels(),
            ["Jacob, Human Warrior", "New character", "Options", "Quit"]
        );
        assert_eq!(
            fresh().main_labels(),
            ["Create character", "Options", "Quit"]
        );
        assert!(m.subtitle().contains("Jacob") && m.subtitle().contains("Warrior"));
    }

    #[test]
    fn the_title_lists_every_character_and_opens_on_the_one_that_is_loaded() {
        let m = two();
        assert_eq!(
            m.main_labels(),
            [
                "Jacob, Human Warrior",
                "Garrick, Human Warrior",
                "New character",
                "Options",
                "Quit"
            ]
        );
        assert_eq!(m.cursor, 1, "the loaded character is the highlighted one");
        assert!(m.subtitle().contains("Garrick"));
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
    fn the_loaded_character_enters_and_another_is_a_switch() {
        let mut m = two();
        assert_eq!(m.press(Press::Confirm), Some(Outcome::Enter));
        m.press(Press::Up);
        assert_eq!(m.cursor, 0);
        assert_eq!(
            m.press(Press::Confirm),
            Some(Outcome::Switch(1)),
            "another character's saves are read at start: the game relaunches on it"
        );
    }

    #[test]
    fn choosing_an_inactive_mage_previews_his_look_and_preserves_it_across_commands() {
        let mut mage = slot(2, "Aldren");
        mage.character.class = Class::Mage;
        mage.character.dials = Dials::from_array([2, 3, 1, 4, 0]);
        let mut m = Menu::new(vec![slot(1, "Jacob"), mage], 1, "Adventurer", rules());
        assert_eq!(
            m.press(Press::Down),
            None,
            "selection alone never starts play"
        );
        let preview = m.preview_selection();
        assert_eq!(preview.class, Class::Mage.id());
        assert_eq!(preview.name, "Aldren");
        assert_eq!(preview.dials, [2, 3, 1, 4, 0]);
        m.press(Press::Down); // New character
        assert_eq!(m.preview_selection().name, "Aldren");
        assert_eq!(
            m.press(Press::Play),
            Some(Outcome::Switch(2)),
            "the Play button always starts the chosen profile"
        );
        m.press(Press::Confirm);
        assert_eq!(m.screen, Screen::Create);
        m.press(Press::Back);
        assert_eq!(
            m.cursor, 1,
            "Back returns to the character we were choosing"
        );
        assert_eq!(m.press(Press::Confirm), Some(Outcome::Switch(2)));
    }

    #[test]
    fn a_new_character_beside_saved_ones_is_added_and_never_replaces() {
        let mut m = two();
        m.press(Press::Down); // New character
        assert_eq!(m.press(Press::Confirm), None);
        assert_eq!(m.screen, Screen::Create);
        let made = m.press(Press::Create);
        assert_eq!(
            made,
            Some(Outcome::Add(Character {
                dials: Dials::from_array([0; 5]),
                ..Character::human_warrior("Adventurer")
            })),
            "no warning, no replace: a slot of its own"
        );
        // The very first character is made in this run, no relaunch.
        assert!(matches!(
            fresh().press(Press::Create),
            Some(Outcome::Create(_))
        ));
    }

    #[test]
    fn a_full_list_offers_no_new_character_and_says_why() {
        let slots: Vec<SlotInfo> = (1..=crate::saves::MAX_CHARACTERS as u32)
            .map(|i| slot(i, "Jacob"))
            .collect();
        let m = Menu::new(slots, 1, "Adventurer", rules());
        assert!(!m.main_rows().contains(&MainRow::New));
        assert_eq!(m.main_labels().last().map(String::as_str), Some("Quit"));
        assert!(m.subtitle().contains("Jacob"));
    }

    #[test]
    fn delete_asks_twice_and_only_on_a_character() {
        let mut m = two();
        assert_eq!(m.press(Press::Delete), None, "the first press only warns");
        assert!(m.message.contains("Garrick"), "{}", m.message);
        assert_eq!(m.press(Press::Delete), Some(Outcome::Delete(2)));
        // Moving away in between cancels the warning.
        let mut m = two();
        m.press(Press::Delete);
        m.press(Press::Up);
        assert_eq!(
            m.press(Press::Delete),
            None,
            "armed again for Jacob, not deleted"
        );
        assert!(m.message.contains("Jacob"), "{}", m.message);
        // A row that is not a character cannot be deleted.
        let mut m = two();
        m.press(Press::Down);
        assert_eq!(m.press(Press::Delete), None);
        assert!(m.message.is_empty());
        assert_eq!(m.press(Press::Delete), None);
    }

    #[test]
    fn a_deleted_character_leaves_the_list_and_the_cursor_stays_inside_it() {
        let mut m = two();
        m.remove_slot(2);
        assert_eq!(
            m.main_labels().len(),
            4,
            "Jacob, New character, Options, Quit"
        );
        assert!(m.cursor < m.main_rows().len());
        m.remove_slot(1);
        assert_eq!(m.main_labels(), ["Create character", "Options", "Quit"]);
        assert_eq!(
            m.screen,
            Screen::Create,
            "with nobody left the creation screen opens, as on a first run"
        );
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
        assert_eq!(m.focus(), Row::Name, "typing visibly focuses the field");
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
        let mut m = Menu::new(Vec::new(), 1, "Adventurer", only);
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
            Vec::new(),
            1,
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
    fn clicking_the_name_then_confirming_creates_a_supported_mage() {
        let mut m = fresh();
        m.sel.class = Class::Mage.id();
        m.press(Press::NameFocus);
        assert_eq!(m.focus(), Row::Name);
        for c in "Aldren".chars() {
            m.press(Press::Char(c));
        }
        assert_eq!(m.press(Press::Confirm), None);
        assert_eq!(m.focus(), Row::Create);
        let Some(Outcome::Create(character)) = m.press(Press::Confirm) else {
            panic!("Mage should be available: {}", m.message);
        };
        assert_eq!(character.class, Class::Mage);
        assert_eq!(character.name, "Aldren");
    }

    #[test]
    fn unsupported_races_classes_and_sexes_cannot_be_created_and_the_menu_says_so() {
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
    fn enter_on_the_create_row_is_the_same_press_as_the_button() {
        let mut m = saved();
        m.press(Press::Down);
        m.press(Press::Confirm); // New character
        focus(&mut m, Row::Create);
        assert!(matches!(m.press(Press::Confirm), Some(Outcome::Add(_))));
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
}
