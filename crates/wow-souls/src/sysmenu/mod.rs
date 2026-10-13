//! The in-game system menu: Esc (the pad's Menu button) in play. Resume, Pack, Equipment, Quest log,
//! Options (Sound, Graphics, Controls, Gamepad, Game), Quit game. The world stands still while it is up
//! (an option), and nothing is bound ([`crate::settings::MenuUp`]), so no other system needs its own gate.
//!
//! [`SysMenu`] is the state: the page stack, the cursor, the capture of the next key, and what a press
//! does ([`SysMenu::press`] returns the [`Effect`]s the game side carries out); it is pure and tested
//! without a window. [`ui`] draws it and feeds it.

pub mod ui;

use bevy::input::gamepad::GamepadButton;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;

use crate::gfx::config::Rung;
use crate::settings::binds::{chord_label, is_modifier, Action, Chord, Group, Mods, Source};
use crate::settings::{Settings, GRASS_STEPS};

/// What the menu shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Root,
    Options,
    Sound,
    Graphics,
    Controls,
    Pad,
    Game,
}

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Page::Root => "Menu",
            Page::Options => "Options",
            Page::Sound => "Sound",
            Page::Graphics => "Graphics",
            Page::Controls => "Controls: keyboard and mouse",
            Page::Pad => "Controls: gamepad",
            Page::Game => "Game",
        }
    }
}

/// A slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slide {
    Master,
    Music,
    Ambience,
    Effects,
    Voices,
    Mouse,
    Stick,
    Shake,
    Farclip,
}

/// A switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    LoopMusic,
    Grading,
    Fullscreen,
    Vsync,
    InvertY,
    HelpLine,
    PauseInMenu,
    AutoEquip,
}

/// A list of named values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Preset,
    Shadows,
    Occlusion,
    Bloom,
    Fog,
    Light,
    Sky,
    Wind,
    Water,
    Grass,
}

/// Which of an action's bindings a row edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// Key or mouse slot 0 or 1.
    Key(usize),
    Pad,
}

/// A command row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    Resume,
    Pack,
    Equipment,
    QuestLog,
    Quit,
    ResetKeys,
    ResetPad,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Heading,
    Go(Page),
    Do(Cmd),
    Slider(Slide),
    Toggle(Flag),
    Choice(Pick),
    Bind(Action),
}

/// One line of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub kind: Kind,
    pub label: String,
    pub value: String,
}

impl Row {
    fn new(kind: Kind, label: impl Into<String>, value: impl Into<String>) -> Row {
        Row {
            kind,
            label: label.into(),
            value: value.into(),
        }
    }

    /// Whether the cursor can stand on it.
    pub fn selectable(&self) -> bool {
        self.kind != Kind::Heading
    }
}

/// What the game side does for a press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Close,
    OpenPack,
    OpenEquipment,
    OpenQuestLog,
    Quit,
}

/// A press the menu understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    Delete,
}

/// The next input the menu waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capture {
    pub action: Action,
    pub slot: Slot,
}

/// What a frame of input did to a capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Captured {
    Nothing,
    Cancel,
    Chord(Chord),
    Pad(GamepadButton),
}

/// One frame of input for a capture: what went down, the modifiers held, a modifier that came up.
#[derive(Debug, Default, Clone)]
pub struct CaptureInput {
    pub keys: Vec<KeyCode>,
    pub mouse: Vec<MouseButton>,
    pub pad: Vec<GamepadButton>,
    pub mods: Mods,
    pub released_modifier: Option<KeyCode>,
    pub escape: bool,
}

/// Decides a capture's frame. A key slot takes the first key or mouse button that went down, with the
/// modifiers held (Shift + Space); a modifier pressed and let go alone is itself the binding (Ctrl for
/// walking); Esc cancels. A pad slot takes the first button that went down; Esc cancels too.
pub fn capture_step(slot: Slot, input: &CaptureInput) -> Captured {
    if input.escape {
        return Captured::Cancel;
    }
    match slot {
        Slot::Pad => input
            .pad
            .first()
            .map_or(Captured::Nothing, |&b| Captured::Pad(b)),
        Slot::Key(_) => {
            if let Some(&key) = input.keys.iter().find(|k| !is_modifier(**k)) {
                return Captured::Chord(Chord {
                    source: Source::Key(key),
                    mods: input.mods,
                });
            }
            if let Some(&button) = input.mouse.first() {
                return Captured::Chord(Chord {
                    source: Source::Mouse(button),
                    mods: input.mods,
                });
            }
            input
                .released_modifier
                .map_or(Captured::Nothing, |k| Captured::Chord(Chord::key(k)))
        }
    }
}

/// How many rows a page shows at once (the rest scroll).
pub const VISIBLE: usize = 13;

#[derive(Resource, Debug, Clone)]
pub struct SysMenu {
    pub open: bool,
    pub page: Page,
    stack: Vec<(Page, usize, usize)>,
    pub cursor: usize,
    pub scroll: usize,
    /// On a key-binding row: which of its two slots Left / Right moved to.
    pub col: usize,
    pub capture: Option<Capture>,
    /// The last thing worth saying (a key that moved, a binding reset).
    pub note: String,
    /// Opened this frame: the press that opened it is not also the menu's first press.
    pub fresh: bool,
}

impl Default for SysMenu {
    fn default() -> Self {
        SysMenu {
            open: false,
            page: Page::Root,
            stack: Vec::new(),
            cursor: 0,
            scroll: 0,
            col: 0,
            capture: None,
            note: String::new(),
            fresh: false,
        }
    }
}

const PRESETS: [(Rung, &str); 5] = [
    (Rung::Classic, "Classic"),
    (Rung::Low, "Low"),
    (Rung::Medium, "Medium"),
    (Rung::High, "High"),
    (Rung::Ultra, "Ultra"),
];

fn tier_name(pick: Pick, v: u8) -> &'static str {
    let names: [&str; 3] = match pick {
        Pick::Shadows => ["Off", "Characters", "Everything"],
        Pick::Occlusion | Pick::Bloom | Pick::Fog => ["Off", "Low", "High"],
        Pick::Light => ["Off", "Medium", "High"],
        Pick::Sky | Pick::Water => ["Classic", "Enhanced", "High"],
        Pick::Wind => ["Off", "Grass", "Grass and trees"],
        Pick::Preset | Pick::Grass => ["", "", ""],
    };
    names[usize::from(v.min(2))]
}

const GRASS_NAMES: [&str; 4] = ["Off", "Low", "Medium", "High"];

fn grass_index(v: f32) -> usize {
    GRASS_STEPS
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - v).abs().total_cmp(&(b.1 - v).abs()))
        .map_or(0, |(i, _)| i)
}

fn preset_index(r: Rung) -> usize {
    PRESETS.iter().position(|(p, _)| *p == r).unwrap_or(3)
}

/// `(value, minimum, maximum, step)` of a slider.
fn slide_get(s: &Settings, w: Slide) -> (f32, f32, f32, f32) {
    match w {
        Slide::Master => (s.sound.master, 0.0, 1.0, 0.05),
        Slide::Music => (s.sound.music, 0.0, 1.0, 0.05),
        Slide::Ambience => (s.sound.ambience, 0.0, 1.0, 0.05),
        Slide::Effects => (s.sound.effects, 0.0, 1.0, 0.05),
        Slide::Voices => (s.sound.voices, 0.0, 1.0, 0.05),
        Slide::Mouse => (s.game.mouse, 0.25, 3.0, 0.05),
        Slide::Stick => (s.game.stick, 0.25, 3.0, 0.05),
        Slide::Shake => (s.game.shake, 0.0, 1.0, 0.05),
        Slide::Farclip => (s.video.farclip, 177.0, 1497.0, 50.0),
    }
}

fn slide_set(s: &mut Settings, w: Slide, v: f32) {
    let (_, lo, hi, step) = slide_get(s, w);
    // Land on the step's grid, so a slider that was left at 0.6999 shows 70 %.
    let v = ((v / step).round() * step).clamp(lo, hi);
    match w {
        Slide::Master => s.sound.master = v,
        Slide::Music => s.sound.music = v,
        Slide::Ambience => s.sound.ambience = v,
        Slide::Effects => s.sound.effects = v,
        Slide::Voices => s.sound.voices = v,
        Slide::Mouse => s.game.mouse = v,
        Slide::Stick => s.game.stick = v,
        Slide::Shake => s.game.shake = v,
        Slide::Farclip => s.video.farclip = v,
    }
}

fn slide_label(w: Slide) -> &'static str {
    match w {
        Slide::Master => "Master volume",
        Slide::Music => "Music",
        Slide::Ambience => "Ambience",
        Slide::Effects => "Sound effects",
        Slide::Voices => "Quest voices",
        Slide::Mouse => "Mouse look speed",
        Slide::Stick => "Stick and arrow look speed",
        Slide::Shake => "Camera shake",
        Slide::Farclip => "Render distance",
    }
}

fn slide_text(s: &Settings, w: Slide) -> String {
    let (v, ..) = slide_get(s, w);
    match w {
        Slide::Farclip => format!("{v:.0} yd"),
        _ => format!("{:.0}%", v * 100.0),
    }
}

fn flag_get(s: &Settings, f: Flag) -> bool {
    match f {
        Flag::LoopMusic => s.sound.loop_music,
        Flag::Grading => s.video.grading,
        Flag::Fullscreen => s.video.fullscreen,
        Flag::Vsync => s.video.vsync,
        Flag::InvertY => s.game.invert_y,
        Flag::HelpLine => s.game.help,
        Flag::PauseInMenu => s.game.pause,
        Flag::AutoEquip => s.game.auto_equip,
    }
}

fn flag_set(s: &mut Settings, f: Flag, on: bool) {
    match f {
        Flag::LoopMusic => s.sound.loop_music = on,
        Flag::Grading => s.video.grading = on,
        Flag::Fullscreen => s.video.fullscreen = on,
        Flag::Vsync => s.video.vsync = on,
        Flag::InvertY => s.game.invert_y = on,
        Flag::HelpLine => s.game.help = on,
        Flag::PauseInMenu => s.game.pause = on,
        Flag::AutoEquip => s.game.auto_equip = on,
    }
}

fn flag_label(f: Flag) -> &'static str {
    match f {
        Flag::LoopMusic => "Loop music",
        Flag::Grading => "Colour grading",
        Flag::Fullscreen => "Fullscreen",
        Flag::Vsync => "Vertical sync",
        Flag::InvertY => "Invert vertical look",
        Flag::HelpLine => "Show the key line",
        Flag::PauseInMenu => "Pause while the menu is open",
        Flag::AutoEquip => "Put new gear on in an empty slot",
    }
}

fn pick_label(p: Pick) -> &'static str {
    match p {
        Pick::Preset => "Preset",
        Pick::Shadows => "Shadows",
        Pick::Occlusion => "Ambient occlusion",
        Pick::Bloom => "Bloom",
        Pick::Fog => "Volumetric fog",
        Pick::Light => "Volumetric light",
        Pick::Sky => "Sky",
        Pick::Wind => "Foliage wind",
        Pick::Water => "Water",
        Pick::Grass => "Grass",
    }
}

/// How many values a list has.
fn pick_len(p: Pick) -> usize {
    match p {
        Pick::Preset => PRESETS.len(),
        Pick::Grass => GRASS_NAMES.len(),
        _ => 3,
    }
}

fn pick_get(s: &Settings, p: Pick) -> usize {
    match p {
        Pick::Preset => preset_index(s.video.rung),
        Pick::Shadows => usize::from(s.video.shadows),
        Pick::Occlusion => usize::from(s.video.ao),
        Pick::Bloom => usize::from(s.video.bloom),
        Pick::Fog => usize::from(s.video.fog),
        Pick::Light => usize::from(s.video.light),
        Pick::Sky => usize::from(s.video.sky),
        Pick::Wind => usize::from(s.video.wind),
        Pick::Water => usize::from(s.video.water),
        Pick::Grass => grass_index(s.video.grass),
    }
}

fn pick_set(s: &mut Settings, p: Pick, i: usize) {
    let t = i.min(pick_len(p) - 1) as u8;
    match p {
        Pick::Preset => s.video.choose(PRESETS[i.min(PRESETS.len() - 1)].0),
        Pick::Shadows => s.video.shadows = t,
        Pick::Occlusion => s.video.ao = t,
        Pick::Bloom => s.video.bloom = t,
        Pick::Fog => s.video.fog = t,
        Pick::Light => s.video.light = t,
        Pick::Sky => s.video.sky = t,
        Pick::Wind => s.video.wind = t,
        Pick::Water => s.video.water = t,
        Pick::Grass => s.video.grass = GRASS_STEPS[i.min(GRASS_STEPS.len() - 1)],
    }
}

fn pick_text(s: &Settings, p: Pick) -> String {
    let i = pick_get(s, p);
    match p {
        Pick::Preset => {
            let name = PRESETS[i].1;
            if s.video.is_custom() {
                format!("Custom ({name})")
            } else {
                name.to_string()
            }
        }
        Pick::Grass => GRASS_NAMES[i].to_string(),
        _ => tier_name(p, i as u8).to_string(),
    }
}

impl SysMenu {
    /// Opens at the root.
    pub fn show(&mut self) {
        *self = SysMenu {
            open: true,
            fresh: true,
            ..SysMenu::default()
        };
    }

    /// Opens on the Options page (from the title): Back there closes the menu.
    pub fn show_options(&mut self) {
        self.show();
        self.page = Page::Options;
    }

    pub fn hide(&mut self) {
        self.open = false;
        self.capture = None;
    }

    /// The rows of the current page.
    pub fn rows(&self, s: &Settings) -> Vec<Row> {
        self.rows_of(self.page, s)
    }

    fn rows_of(&self, page: Page, s: &Settings) -> Vec<Row> {
        let go = |p: Page| Row::new(Kind::Go(p), p.title(), "");
        let cmd = |c: Cmd, label: &str| Row::new(Kind::Do(c), label, "");
        let slider = |w: Slide| Row::new(Kind::Slider(w), slide_label(w), slide_text(s, w));
        let toggle = |f: Flag| {
            Row::new(
                Kind::Toggle(f),
                flag_label(f),
                if flag_get(s, f) { "On" } else { "Off" },
            )
        };
        let pick = |p: Pick| Row::new(Kind::Choice(p), pick_label(p), pick_text(s, p));
        match page {
            Page::Root => vec![
                cmd(Cmd::Resume, "Resume"),
                cmd(Cmd::Pack, "Pack"),
                cmd(Cmd::Equipment, "Equipment"),
                cmd(Cmd::QuestLog, "Quest log"),
                go(Page::Options),
                cmd(Cmd::Quit, "Quit game"),
            ],
            Page::Options => vec![
                go(Page::Sound),
                go(Page::Graphics),
                go(Page::Controls),
                go(Page::Pad),
                go(Page::Game),
                cmd(Cmd::Back, "Back"),
            ],
            Page::Sound => vec![
                slider(Slide::Master),
                slider(Slide::Music),
                slider(Slide::Ambience),
                slider(Slide::Effects),
                slider(Slide::Voices),
                toggle(Flag::LoopMusic),
                cmd(Cmd::Back, "Back"),
            ],
            Page::Graphics => vec![
                pick(Pick::Preset),
                pick(Pick::Shadows),
                pick(Pick::Occlusion),
                pick(Pick::Bloom),
                pick(Pick::Fog),
                pick(Pick::Light),
                pick(Pick::Sky),
                pick(Pick::Wind),
                pick(Pick::Water),
                toggle(Flag::Grading),
                slider(Slide::Farclip),
                pick(Pick::Grass),
                toggle(Flag::Fullscreen),
                toggle(Flag::Vsync),
                cmd(Cmd::Back, "Back"),
            ],
            Page::Game => vec![
                slider(Slide::Mouse),
                slider(Slide::Stick),
                toggle(Flag::InvertY),
                slider(Slide::Shake),
                toggle(Flag::HelpLine),
                toggle(Flag::PauseInMenu),
                toggle(Flag::AutoEquip),
                cmd(Cmd::Back, "Back"),
            ],
            Page::Controls | Page::Pad => {
                let pad = page == Page::Pad;
                let mut rows = Vec::new();
                for group in Group::ALL {
                    rows.push(Row::new(Kind::Heading, group.label(), ""));
                    for action in Action::ALL.into_iter().filter(|a| a.group() == group) {
                        rows.push(Row::new(
                            Kind::Bind(action),
                            action.label(),
                            self.bind_text(s, action, pad),
                        ));
                    }
                }
                rows.push(cmd(
                    if pad { Cmd::ResetPad } else { Cmd::ResetKeys },
                    "Reset to defaults",
                ));
                rows.push(cmd(Cmd::Back, "Back"));
                rows
            }
        }
    }

    fn bind_text(&self, s: &Settings, action: Action, pad: bool) -> String {
        let capturing = |slot: Slot| {
            self.capture
                .is_some_and(|c| c.action == action && c.slot == slot)
        };
        if pad {
            if capturing(Slot::Pad) {
                return "[Press a button]".to_string();
            }
            return s.binds.pad_label(action);
        }
        let chords = s.binds.chords(action);
        let cell = |slot: usize| {
            if capturing(Slot::Key(slot)) {
                "Press a key".to_string()
            } else {
                chords[slot].map_or_else(|| "-".to_string(), chord_label)
            }
        };
        format!("{}    {}", cell(0), cell(1))
    }

    /// The row the cursor stands on.
    #[cfg(test)]
    pub fn current(&self, s: &Settings) -> Option<Row> {
        self.rows(s).into_iter().nth(self.cursor)
    }

    fn keep_in_view(&mut self) {
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + VISIBLE {
            self.scroll = self.cursor + 1 - VISIBLE;
        }
    }

    fn move_cursor(&mut self, rows: &[Row], dir: i32) {
        if rows.is_empty() {
            return;
        }
        let n = rows.len() as i32;
        let mut at = self.cursor as i32;
        for _ in 0..rows.len() {
            at = (at + dir).rem_euclid(n);
            if rows[at as usize].selectable() {
                break;
            }
        }
        self.cursor = at as usize;
        self.col = 0;
        self.keep_in_view();
    }

    fn enter(&mut self, page: Page, s: &Settings) {
        self.stack.push((self.page, self.cursor, self.scroll));
        self.page = page;
        self.cursor = 0;
        self.scroll = 0;
        self.col = 0;
        self.capture = None;
        // The first row the cursor can stand on.
        let rows = self.rows(s);
        if let Some(first) = rows.iter().position(Row::selectable) {
            self.cursor = first;
        }
        self.note.clear();
    }

    fn leave(&mut self) -> Vec<Effect> {
        match self.stack.pop() {
            Some((page, cursor, scroll)) => {
                self.page = page;
                self.cursor = cursor;
                self.scroll = scroll;
                self.col = 0;
                self.capture = None;
                self.note.clear();
                Vec::new()
            }
            None => vec![Effect::Close],
        }
    }

    /// Hands a capture's outcome to the bindings. Returns whether anything changed.
    pub fn captured(&mut self, got: Captured, s: &mut Settings) -> bool {
        let Some(cap) = self.capture.take() else {
            return false;
        };
        match got {
            Captured::Nothing => {
                self.capture = Some(cap);
                false
            }
            Captured::Cancel => false,
            Captured::Chord(chord) => {
                let Slot::Key(slot) = cap.slot else {
                    return false;
                };
                let displaced = s.binds.assign(cap.action, slot, chord);
                self.note = match displaced {
                    Some(other) => format!(
                        "{} is {} now; {} lost it.",
                        chord_label(chord),
                        cap.action.label(),
                        other.label()
                    ),
                    None => String::new(),
                };
                true
            }
            Captured::Pad(button) => {
                let displaced = s.binds.assign_pad(cap.action, button);
                self.note = match displaced {
                    Some(other) => format!(
                        "{} is {} now; {} lost it.",
                        crate::pad::button_label(button),
                        cap.action.label(),
                        other.label()
                    ),
                    None => String::new(),
                };
                true
            }
        }
    }

    /// One press. A press during a capture is not a press: the capture owns the input
    /// ([`SysMenu::captured`]).
    pub fn press(&mut self, p: Press, s: &mut Settings) -> Vec<Effect> {
        if self.capture.is_some() {
            return Vec::new();
        }
        let rows = self.rows(s);
        let Some(row) = rows.get(self.cursor).cloned() else {
            return Vec::new();
        };
        match p {
            Press::Up => {
                self.move_cursor(&rows, -1);
                Vec::new()
            }
            Press::Down => {
                self.move_cursor(&rows, 1);
                Vec::new()
            }
            Press::Back => self.leave(),
            Press::Left | Press::Right => {
                let dir: i32 = if p == Press::Left { -1 } else { 1 };
                match row.kind {
                    Kind::Slider(w) => {
                        let (v, _, _, step) = slide_get(s, w);
                        slide_set(s, w, v + step * dir as f32);
                    }
                    Kind::Toggle(f) => flag_set(s, f, !flag_get(s, f)),
                    Kind::Choice(pick) => {
                        let n = pick_len(pick) as i32;
                        let at = pick_get(s, pick) as i32;
                        pick_set(s, pick, (at + dir).rem_euclid(n) as usize);
                    }
                    Kind::Bind(_) if self.page == Page::Controls => {
                        self.col = if dir < 0 { 0 } else { 1 };
                    }
                    _ => {}
                }
                Vec::new()
            }
            Press::Delete => {
                if let Kind::Bind(action) = row.kind {
                    match self.page {
                        Page::Controls => s.binds.clear(action, self.col),
                        Page::Pad => s.binds.clear_pad(action),
                        _ => {}
                    }
                    self.note.clear();
                }
                Vec::new()
            }
            Press::Confirm => match row.kind {
                Kind::Heading => Vec::new(),
                Kind::Go(page) => {
                    self.enter(page, s);
                    Vec::new()
                }
                Kind::Do(cmd) => match cmd {
                    Cmd::Resume => vec![Effect::Close],
                    Cmd::Pack => vec![Effect::Close, Effect::OpenPack],
                    Cmd::Equipment => vec![Effect::Close, Effect::OpenEquipment],
                    Cmd::QuestLog => vec![Effect::Close, Effect::OpenQuestLog],
                    Cmd::Quit => vec![Effect::Quit],
                    Cmd::Back => self.leave(),
                    Cmd::ResetKeys => {
                        let pads: Vec<_> = Action::ALL
                            .into_iter()
                            .map(|a| (a, s.binds.pad_button(a)))
                            .collect();
                        s.binds.reset();
                        for (a, b) in pads {
                            match b {
                                Some(b) => {
                                    s.binds.assign_pad(a, b);
                                }
                                None => s.binds.clear_pad(a),
                            }
                        }
                        self.note = "Keys and mouse buttons are back to the defaults.".into();
                        Vec::new()
                    }
                    Cmd::ResetPad => {
                        let keys: Vec<_> = Action::ALL
                            .into_iter()
                            .map(|a| (a, s.binds.chords(a)))
                            .collect();
                        s.binds.reset();
                        for (a, [one, two]) in keys {
                            s.binds.clear(a, 0);
                            s.binds.clear(a, 1);
                            if let Some(c) = one {
                                s.binds.assign(a, 0, c);
                            }
                            if let Some(c) = two {
                                s.binds.assign(a, 1, c);
                            }
                        }
                        self.note = "The gamepad buttons are back to the defaults.".into();
                        Vec::new()
                    }
                },
                Kind::Slider(_) => Vec::new(),
                Kind::Toggle(f) => {
                    flag_set(s, f, !flag_get(s, f));
                    Vec::new()
                }
                Kind::Choice(pick) => {
                    let at = pick_get(s, pick);
                    pick_set(s, pick, (at + 1) % pick_len(pick));
                    Vec::new()
                }
                Kind::Bind(action) => {
                    self.capture = Some(Capture {
                        action,
                        slot: if self.page == Page::Pad {
                            Slot::Pad
                        } else {
                            Slot::Key(self.col)
                        },
                    });
                    self.note.clear();
                    Vec::new()
                }
            },
        }
    }

    /// The rows the screen draws now: the visible window of the page, and where the cursor is in it.
    pub fn window(&self, s: &Settings) -> (Vec<Row>, usize) {
        let rows = self.rows(s);
        let end = (self.scroll + VISIBLE).min(rows.len());
        let shown = rows[self.scroll.min(end)..end].to_vec();
        (shown, self.cursor.saturating_sub(self.scroll))
    }
}

/// A snapshot of the input a capture needs this frame, from the engine's input state.
pub fn capture_input(
    keys: &bevy::input::ButtonInput<KeyCode>,
    mouse: &bevy::input::ButtonInput<MouseButton>,
    pads: &[&bevy::input::gamepad::Gamepad],
) -> CaptureInput {
    let pressed_modifier = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    CaptureInput {
        keys: keys.get_just_pressed().copied().collect(),
        mouse: mouse.get_just_pressed().copied().collect(),
        pad: pads
            .iter()
            .flat_map(|p| p.get_just_pressed().copied())
            .collect(),
        mods: Mods {
            shift: pressed_modifier(KeyCode::ShiftLeft, KeyCode::ShiftRight),
            ctrl: pressed_modifier(KeyCode::ControlLeft, KeyCode::ControlRight),
            alt: pressed_modifier(KeyCode::AltLeft, KeyCode::AltRight),
        },
        released_modifier: keys.get_just_released().copied().find(|k| is_modifier(*k)),
        escape: keys.just_pressed(KeyCode::Escape),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> (SysMenu, Settings) {
        let mut m = SysMenu::default();
        m.show();
        (m, Settings::default())
    }

    fn go(m: &mut SysMenu, s: &mut Settings, label: &str) {
        let rows = m.rows(s);
        m.cursor = rows
            .iter()
            .position(|r| r.label == label)
            .unwrap_or_else(|| panic!("no row {label:?}"));
        m.press(Press::Confirm, s);
    }

    fn at(m: &mut SysMenu, s: &Settings, label: &str) {
        m.cursor = m
            .rows(s)
            .iter()
            .position(|r| r.label == label)
            .unwrap_or_else(|| panic!("no row {label:?}"));
    }

    #[test]
    fn it_opens_on_the_root_and_the_root_lists_the_six_things() {
        let (m, s) = menu();
        let labels: Vec<String> = m.rows(&s).into_iter().map(|r| r.label).collect();
        assert_eq!(
            labels,
            [
                "Resume",
                "Pack",
                "Equipment",
                "Quest log",
                "Options",
                "Quit game"
            ]
        );
        assert!(m.open && m.page == Page::Root && m.cursor == 0);
    }

    #[test]
    fn the_cursor_wraps_and_confirm_acts_on_the_root() {
        let (mut m, mut s) = menu();
        m.press(Press::Up, &mut s);
        assert_eq!(m.cursor, 5);
        assert_eq!(m.press(Press::Confirm, &mut s), [Effect::Quit]);
        m.cursor = 0;
        assert_eq!(m.press(Press::Confirm, &mut s), [Effect::Close]);
        m.cursor = 1;
        assert_eq!(
            m.press(Press::Confirm, &mut s),
            [Effect::Close, Effect::OpenPack]
        );
        m.cursor = 2;
        assert_eq!(
            m.press(Press::Confirm, &mut s),
            [Effect::Close, Effect::OpenEquipment]
        );
        m.cursor = 3;
        assert_eq!(
            m.press(Press::Confirm, &mut s),
            [Effect::Close, Effect::OpenQuestLog]
        );
    }

    #[test]
    fn options_from_the_title_open_on_the_options_page_and_back_closes() {
        let mut m = SysMenu::default();
        let mut s = Settings::default();
        m.show_options();
        assert!(m.open && m.page == Page::Options);
        go(&mut m, &mut s, "Sound");
        assert_eq!(m.page, Page::Sound);
        assert!(m.press(Press::Back, &mut s).is_empty());
        assert_eq!(m.page, Page::Options);
        assert_eq!(m.press(Press::Back, &mut s), [Effect::Close]);
    }

    #[test]
    fn back_walks_out_of_the_pages_and_closes_at_the_root() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        assert_eq!(m.page, Page::Options);
        go(&mut m, &mut s, "Sound");
        assert_eq!(m.page, Page::Sound);
        assert!(m.press(Press::Back, &mut s).is_empty());
        assert_eq!(m.page, Page::Options);
        assert!(m.press(Press::Back, &mut s).is_empty());
        assert_eq!(m.page, Page::Root);
        assert_eq!(m.cursor, 4, "the cursor is back on Options");
        assert_eq!(m.press(Press::Back, &mut s), [Effect::Close]);
    }

    #[test]
    fn a_slider_steps_clamps_and_shows_a_percentage() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Sound");
        at(&mut m, &s, "Master volume");
        assert_eq!(m.current(&s).unwrap().value, "60%");
        m.press(Press::Right, &mut s);
        assert!((s.sound.master - 0.65).abs() < 1.0e-5, "{}", s.sound.master);
        for _ in 0..20 {
            m.press(Press::Right, &mut s);
        }
        assert_eq!(s.sound.master, 1.0, "clamped at full");
        for _ in 0..40 {
            m.press(Press::Left, &mut s);
        }
        assert_eq!(s.sound.master, 0.0);
        assert_eq!(m.current(&s).unwrap().value, "0%");
    }

    #[test]
    fn a_toggle_flips_with_either_arrow_or_confirm() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Sound");
        at(&mut m, &s, "Loop music");
        assert!(s.sound.loop_music);
        m.press(Press::Confirm, &mut s);
        assert!(!s.sound.loop_music);
        m.press(Press::Right, &mut s);
        assert!(s.sound.loop_music);
        assert_eq!(m.current(&s).unwrap().value, "On");
    }

    #[test]
    fn the_game_page_switches_new_gear_going_on_by_itself() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Game");
        at(&mut m, &s, "Put new gear on in an empty slot");
        assert!(s.game.auto_equip);
        assert_eq!(m.current(&s).unwrap().value, "On");
        m.press(Press::Confirm, &mut s);
        assert!(!s.game.auto_equip);
        assert_eq!(m.current(&s).unwrap().value, "Off");
    }

    #[test]
    fn the_preset_row_sets_every_lane_and_a_lane_makes_it_custom() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Graphics");
        at(&mut m, &s, "Preset");
        assert_eq!(m.current(&s).unwrap().value, "High");
        m.press(Press::Left, &mut s); // Medium
        assert_eq!(s.video.rung, Rung::Medium);
        assert_eq!(s.video.bloom, 1, "the lanes took Medium's values");
        assert_eq!(m.current(&s).unwrap().value, "Medium");
        at(&mut m, &s, "Bloom");
        m.press(Press::Right, &mut s);
        assert_eq!(s.video.bloom, 2);
        at(&mut m, &s, "Preset");
        assert_eq!(m.current(&s).unwrap().value, "Custom (Medium)");
        // Left from Classic wraps to Ultra.
        s.video.choose(Rung::Classic);
        m.press(Press::Left, &mut s);
        assert_eq!(s.video.rung, Rung::Ultra);
    }

    #[test]
    fn the_distance_and_grass_rows_land_on_their_steps() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Graphics");
        at(&mut m, &s, "Render distance");
        let before = s.video.farclip;
        m.press(Press::Right, &mut s);
        assert!(s.video.farclip > before && s.video.farclip <= 1497.0);
        assert!(
            (s.video.farclip % 50.0).abs() < 1.0e-3,
            "{}",
            s.video.farclip
        );
        at(&mut m, &s, "Grass");
        assert_eq!(m.current(&s).unwrap().value, "High", "the preset's 16");
        m.press(Press::Confirm, &mut s);
        assert_eq!(m.current(&s).unwrap().value, "Off", "wraps to the first");
        assert_eq!(s.video.grass, 0.0);
    }

    #[test]
    fn the_controls_page_lists_every_action_once_under_its_group() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Controls: keyboard and mouse");
        let rows = m.rows(&s);
        let binds = rows
            .iter()
            .filter(|r| matches!(r.kind, Kind::Bind(_)))
            .count();
        assert_eq!(binds, Action::ALL.len());
        let headings: Vec<&str> = rows
            .iter()
            .filter(|r| r.kind == Kind::Heading)
            .map(|r| r.label.as_str())
            .collect();
        assert_eq!(
            headings,
            [
                "Movement",
                "Combat",
                "World",
                "Abilities",
                "Screens",
                "Camera"
            ]
        );
        // The cursor starts on the first action, never on a heading, and skips headings going down.
        assert!(matches!(
            m.current(&s).unwrap().kind,
            Kind::Bind(Action::MoveForward)
        ));
        let light = rows
            .iter()
            .find(|r| r.kind == Kind::Bind(Action::Light))
            .unwrap();
        assert_eq!(light.value, "F    Mouse L");
    }

    #[test]
    fn cursor_walks_over_headings_without_stopping_on_them() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Controls: keyboard and mouse");
        for _ in 0..40 {
            m.press(Press::Down, &mut s);
            assert!(m.current(&s).unwrap().selectable());
        }
        for _ in 0..40 {
            m.press(Press::Up, &mut s);
            assert!(m.current(&s).unwrap().selectable());
        }
    }

    #[test]
    fn a_binding_is_captured_into_the_slot_the_arrows_chose() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Controls: keyboard and mouse");
        at(&mut m, &s, "Heavy attack");
        m.press(Press::Right, &mut s); // slot 1: the mouse button
        assert_eq!(m.col, 1);
        m.press(Press::Confirm, &mut s);
        assert!(m.capture.is_some());
        assert_eq!(m.current(&s).unwrap().value, "R    Press a key");
        // A press during the capture is not a press.
        assert!(m.press(Press::Down, &mut s).is_empty());
        assert!(m.capture.is_some());
        let got = capture_step(
            Slot::Key(1),
            &CaptureInput {
                keys: vec![KeyCode::KeyG],
                mods: Mods {
                    shift: true,
                    ..Mods::NONE
                },
                ..default()
            },
        );
        assert_eq!(
            got,
            Captured::Chord(Chord::key(KeyCode::KeyG).with(Mods {
                shift: true,
                ..Mods::NONE
            }))
        );
        assert!(m.captured(got, &mut s));
        assert!(m.capture.is_none());
        assert_eq!(
            s.binds.chords(Action::Heavy)[1].map(chord_label).as_deref(),
            Some("Shift + G")
        );
        assert_eq!(
            s.binds.chords(Action::Heavy)[0],
            Some(Chord::key(KeyCode::KeyR))
        );
    }

    #[test]
    fn a_key_that_was_taken_says_who_lost_it() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Controls: keyboard and mouse");
        at(&mut m, &s, "Heavy attack");
        m.press(Press::Confirm, &mut s);
        assert!(m.captured(Captured::Chord(Chord::key(KeyCode::KeyF)), &mut s));
        assert!(m.note.contains("Light attack lost it"), "{}", m.note);
        assert_eq!(s.binds.chords(Action::Light)[0], None);
    }

    #[test]
    fn escape_cancels_a_capture_and_changes_nothing() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Controls: keyboard and mouse");
        at(&mut m, &s, "Block (hold)");
        m.press(Press::Confirm, &mut s);
        let got = capture_step(
            Slot::Key(0),
            &CaptureInput {
                escape: true,
                keys: vec![KeyCode::Escape],
                ..default()
            },
        );
        assert_eq!(got, Captured::Cancel);
        assert!(!m.captured(got, &mut s));
        assert!(m.capture.is_none());
        assert!(s.binds.is_default());
    }

    #[test]
    fn a_modifier_let_go_alone_is_the_binding_and_a_chord_is_not() {
        // Ctrl pressed and released: Ctrl itself.
        let got = capture_step(
            Slot::Key(0),
            &CaptureInput {
                released_modifier: Some(KeyCode::ControlLeft),
                ..default()
            },
        );
        assert_eq!(got, Captured::Chord(Chord::key(KeyCode::ControlLeft)));
        // Shift held, Space pressed: the chord, not Shift.
        let got = capture_step(
            Slot::Key(0),
            &CaptureInput {
                keys: vec![KeyCode::ShiftLeft, KeyCode::Space],
                mods: Mods {
                    shift: true,
                    ..Mods::NONE
                },
                ..default()
            },
        );
        assert_eq!(
            got,
            Captured::Chord(Chord::key(KeyCode::Space).with(Mods {
                shift: true,
                ..Mods::NONE
            }))
        );
        // A mouse button.
        let got = capture_step(
            Slot::Key(1),
            &CaptureInput {
                mouse: vec![MouseButton::Back],
                ..default()
            },
        );
        assert_eq!(got, Captured::Chord(Chord::mouse(MouseButton::Back)));
        // Nothing yet.
        assert_eq!(
            capture_step(Slot::Key(0), &CaptureInput::default()),
            Captured::Nothing
        );
        // A pad slot ignores keys and takes a button.
        assert_eq!(
            capture_step(
                Slot::Pad,
                &CaptureInput {
                    keys: vec![KeyCode::KeyA],
                    ..default()
                }
            ),
            Captured::Nothing
        );
        assert_eq!(
            capture_step(
                Slot::Pad,
                &CaptureInput {
                    pad: vec![GamepadButton::North],
                    ..default()
                }
            ),
            Captured::Pad(GamepadButton::North)
        );
    }

    #[test]
    fn delete_clears_the_chosen_slot_and_reset_restores_the_defaults() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Controls: keyboard and mouse");
        at(&mut m, &s, "Light attack");
        m.press(Press::Right, &mut s);
        m.press(Press::Delete, &mut s);
        assert_eq!(s.binds.chords(Action::Light)[1], None);
        assert_eq!(
            s.binds.chords(Action::Light)[0],
            Some(Chord::key(KeyCode::KeyF))
        );
        // A gamepad change survives the keyboard page's reset and the other way round.
        s.binds.assign_pad(Action::Pack, GamepadButton::North);
        go(&mut m, &mut s, "Reset to defaults");
        assert_eq!(
            s.binds.chords(Action::Light)[1].map(chord_label).as_deref(),
            Some("Mouse L")
        );
        assert_eq!(s.binds.pad_button(Action::Pack), Some(GamepadButton::North));
        s.binds.assign(Action::Light, 0, Chord::key(KeyCode::KeyG));
        go(&mut m, &mut s, "Back");
        go(&mut m, &mut s, "Controls: gamepad");
        go(&mut m, &mut s, "Reset to defaults");
        assert_eq!(s.binds.pad_button(Action::Pack), None, "the pad is back");
        assert_eq!(
            s.binds.chords(Action::Light)[0],
            Some(Chord::key(KeyCode::KeyG)),
            "the keys are not"
        );
    }

    #[test]
    fn a_long_page_scrolls_to_keep_the_cursor_in_view() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Controls: keyboard and mouse");
        for _ in 0..30 {
            m.press(Press::Down, &mut s);
        }
        let (window, at) = m.window(&s);
        assert_eq!(window.len(), VISIBLE);
        assert!(at < VISIBLE);
        assert_eq!(window[at], m.current(&s).unwrap());
    }

    #[test]
    fn every_action_has_a_pad_row_and_the_pad_capture_binds_a_button() {
        let (mut m, mut s) = menu();
        go(&mut m, &mut s, "Options");
        go(&mut m, &mut s, "Controls: gamepad");
        at(&mut m, &s, "Pack");
        assert_eq!(m.current(&s).unwrap().value, "-");
        m.press(Press::Confirm, &mut s);
        assert_eq!(m.current(&s).unwrap().value, "[Press a button]");
        assert!(m.captured(Captured::Pad(GamepadButton::West), &mut s));
        assert_eq!(s.binds.pad_button(Action::Pack), Some(GamepadButton::West));
        assert!(m.note.contains("Quick-use Salve lost it"), "{}", m.note);
    }
}
