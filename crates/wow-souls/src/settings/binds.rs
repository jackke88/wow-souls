//! The key bindings: which key, mouse button or gamepad button does what.
//!
//! An [`Action`] has up to two [`Chord`]s (a key or a mouse button, with Shift / Ctrl / Alt held) and
//! one gamepad button. [`Binds::default`] is the game's own layout, the keys the modules used to
//! hard-code, so the defaults change nothing. The rules of a press are pure ([`Held`] is a snapshot of
//! the input) and tested without a window; [`Controls`] is the system parameter the modules read.
//!
//! A chord is active when its source is pressed, the modifiers it names are held, and no *more
//! specific* chord on the same source is also held: with Space rolling and Shift+Space jumping, Shift
//! held makes Space a jump and only a jump, while Ctrl (walk) held still rolls. A mouse button counts
//! only while the cursor is captured (the click that captures it is no swing).

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

/// Everything the player can do that is worth a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    MoveForward,
    MoveBack,
    MoveLeft,
    MoveRight,
    Walk,
    Jump,
    Light,
    Heavy,
    Evade,
    Block,
    Parry,
    LockOn,
    Interact,
    QuickUse,
    TwoHand,
    Ranged,
    Ability1,
    Ability2,
    Ability3,
    Ability4,
    AbilityCycle,
    AbilityUse,
    Pack,
    Equipment,
    QuestLog,
    Talk,
    Menu,
    LookUp,
    LookDown,
    LookLeft,
    LookRight,
}

/// The Controls page's sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Movement,
    Combat,
    World,
    Abilities,
    Screens,
    Camera,
}

impl Group {
    pub const ALL: [Group; 6] = [
        Group::Movement,
        Group::Combat,
        Group::World,
        Group::Abilities,
        Group::Screens,
        Group::Camera,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Group::Movement => "Movement",
            Group::Combat => "Combat",
            Group::World => "World",
            Group::Abilities => "Abilities",
            Group::Screens => "Screens",
            Group::Camera => "Camera",
        }
    }
}

impl Action {
    pub const ALL: [Action; 31] = [
        Action::MoveForward,
        Action::MoveBack,
        Action::MoveLeft,
        Action::MoveRight,
        Action::Walk,
        Action::Jump,
        Action::Light,
        Action::Heavy,
        Action::Evade,
        Action::Block,
        Action::Parry,
        Action::LockOn,
        Action::Interact,
        Action::QuickUse,
        Action::TwoHand,
        Action::Ranged,
        Action::Ability1,
        Action::Ability2,
        Action::Ability3,
        Action::Ability4,
        Action::AbilityCycle,
        Action::AbilityUse,
        Action::Pack,
        Action::Equipment,
        Action::QuestLog,
        Action::Talk,
        Action::Menu,
        Action::LookUp,
        Action::LookDown,
        Action::LookLeft,
        Action::LookRight,
    ];

    /// The key of the settings file.
    pub fn id(self) -> &'static str {
        match self {
            Action::MoveForward => "move_forward",
            Action::MoveBack => "move_back",
            Action::MoveLeft => "move_left",
            Action::MoveRight => "move_right",
            Action::Walk => "walk",
            Action::Jump => "jump",
            Action::Light => "light",
            Action::Heavy => "heavy",
            Action::Evade => "evade",
            Action::Block => "block",
            Action::Parry => "parry",
            Action::LockOn => "lock_on",
            Action::Interact => "interact",
            Action::QuickUse => "quick_use",
            Action::TwoHand => "two_hand",
            Action::Ranged => "ranged",
            Action::Ability1 => "ability_1",
            Action::Ability2 => "ability_2",
            Action::Ability3 => "ability_3",
            Action::Ability4 => "ability_4",
            Action::AbilityCycle => "ability_cycle",
            Action::AbilityUse => "ability_use",
            Action::Pack => "pack",
            Action::Equipment => "equipment",
            Action::QuestLog => "quest_log",
            Action::Talk => "talk",
            Action::Menu => "menu",
            Action::LookUp => "look_up",
            Action::LookDown => "look_down",
            Action::LookLeft => "look_left",
            Action::LookRight => "look_right",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }

    /// The name on the Controls page.
    pub fn label(self) -> &'static str {
        match self {
            Action::MoveForward => "Move forward",
            Action::MoveBack => "Move back",
            Action::MoveLeft => "Move left",
            Action::MoveRight => "Move right",
            Action::Walk => "Walk (hold)",
            Action::Jump => "Jump",
            Action::Light => "Light attack",
            Action::Heavy => "Heavy attack",
            Action::Evade => "Roll / backstep",
            Action::Block => "Block (hold)",
            Action::Parry => "Parry",
            Action::LockOn => "Lock on",
            Action::Interact => "Use / loot / talk",
            Action::QuickUse => "Quick-use Salve",
            Action::TwoHand => "Two-hand grip",
            Action::Ranged => "Draw ranged weapon",
            Action::Ability1 => "Ability slot 1",
            Action::Ability2 => "Ability slot 2",
            Action::Ability3 => "Ability slot 3",
            Action::Ability4 => "Ability slot 4",
            Action::AbilityCycle => "Next ability",
            Action::AbilityUse => "Use selected ability",
            Action::Pack => "Pack",
            Action::Equipment => "Equipment",
            Action::QuestLog => "Quest log",
            Action::Talk => "Talk to NPC",
            Action::Menu => "Menu",
            Action::LookUp => "Look up",
            Action::LookDown => "Look down",
            Action::LookLeft => "Look left",
            Action::LookRight => "Look right",
        }
    }

    pub fn group(self) -> Group {
        match self {
            Action::MoveForward
            | Action::MoveBack
            | Action::MoveLeft
            | Action::MoveRight
            | Action::Walk
            | Action::Jump => Group::Movement,
            Action::Light
            | Action::Heavy
            | Action::Evade
            | Action::Block
            | Action::Parry
            | Action::LockOn
            | Action::TwoHand
            | Action::Ranged => Group::Combat,
            Action::Interact | Action::QuickUse | Action::Talk => Group::World,
            Action::Ability1
            | Action::Ability2
            | Action::Ability3
            | Action::Ability4
            | Action::AbilityCycle
            | Action::AbilityUse => Group::Abilities,
            Action::Pack | Action::Equipment | Action::QuestLog | Action::Menu => Group::Screens,
            Action::LookUp | Action::LookDown | Action::LookLeft | Action::LookRight => {
                Group::Camera
            }
        }
    }
}

/// What is pressed: a key or a mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    Key(KeyCode),
    Mouse(MouseButton),
}

/// The modifier keys a chord needs held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Mods {
    pub const NONE: Mods = Mods {
        shift: false,
        ctrl: false,
        alt: false,
    };

    /// Whether every modifier `self` names is also in `held`.
    pub fn within(self, held: Mods) -> bool {
        (!self.shift || held.shift) && (!self.ctrl || held.ctrl) && (!self.alt || held.alt)
    }

    fn count(self) -> usize {
        usize::from(self.shift) + usize::from(self.ctrl) + usize::from(self.alt)
    }
}

/// A source with its modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub source: Source,
    pub mods: Mods,
}

impl Chord {
    pub fn key(key: KeyCode) -> Chord {
        Chord {
            source: Source::Key(key),
            mods: Mods::NONE,
        }
    }

    pub fn mouse(button: MouseButton) -> Chord {
        Chord {
            source: Source::Mouse(button),
            mods: Mods::NONE,
        }
    }

    pub fn with(self, mods: Mods) -> Chord {
        Chord { mods, ..self }
    }
}

/// A snapshot of the input a binding is asked about.
pub struct Held<'a> {
    pub keys: &'a ButtonInput<KeyCode>,
    pub mouse: &'a ButtonInput<MouseButton>,
    pub pads: &'a [&'a Gamepad],
    /// The cursor is captured: the mouse buttons are the player's, not the window's.
    pub captured: bool,
}

/// The modifier keys (both sides of each) as `Mods`.
fn held_mods(keys: &ButtonInput<KeyCode>, ignoring: Option<KeyCode>) -> Mods {
    let family = |a: KeyCode, b: KeyCode| {
        let skip = ignoring.is_some_and(|k| k == a || k == b);
        !skip && (keys.pressed(a) || keys.pressed(b))
    };
    Mods {
        shift: family(KeyCode::ShiftLeft, KeyCode::ShiftRight),
        ctrl: family(KeyCode::ControlLeft, KeyCode::ControlRight),
        alt: family(KeyCode::AltLeft, KeyCode::AltRight),
    }
}

/// The two slots of keys and the pad button of every action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binds {
    keys: HashMap<Action, [Option<Chord>; 2]>,
    pad: HashMap<Action, Option<GamepadButton>>,
}

/// What an assignment displaced: the action that had the key (or button) and lost it.
pub type Displaced = Option<Action>;

fn default_keys(action: Action) -> [Option<Chord>; 2] {
    use Action::*;
    use KeyCode as K;
    let key = |k| Some(Chord::key(k));
    let mouse = |b| Some(Chord::mouse(b));
    match action {
        MoveForward => [key(K::KeyW), None],
        MoveBack => [key(K::KeyS), None],
        MoveLeft => [key(K::KeyA), None],
        MoveRight => [key(K::KeyD), None],
        Walk => [key(K::ControlLeft), key(K::ControlRight)],
        Jump => [
            Some(Chord::key(K::Space).with(Mods {
                shift: true,
                ..Mods::NONE
            })),
            None,
        ],
        Light => [key(K::KeyF), mouse(MouseButton::Left)],
        Heavy => [key(K::KeyR), mouse(MouseButton::Right)],
        Evade => [key(K::Space), None],
        Block => [key(K::KeyE), None],
        Parry => [key(K::KeyC), None],
        LockOn => [key(K::KeyQ), mouse(MouseButton::Middle)],
        Interact => [key(K::KeyX), None],
        QuickUse => [key(K::KeyV), None],
        TwoHand => [key(K::KeyH), None],
        Ranged => [key(K::KeyY), None],
        Ability1 => [key(K::Digit1), None],
        Ability2 => [key(K::Digit2), None],
        Ability3 => [key(K::Digit3), None],
        Ability4 => [key(K::Digit4), None],
        AbilityCycle => [key(K::KeyZ), None],
        AbilityUse => [None, None],
        Pack => [key(K::Tab), None],
        Equipment => [key(K::KeyG), None],
        QuestLog => [key(K::KeyL), None],
        Talk => [key(K::KeyT), None],
        Menu => [key(K::Escape), None],
        LookUp => [key(K::ArrowUp), None],
        LookDown => [key(K::ArrowDown), None],
        LookLeft => [key(K::ArrowLeft), None],
        LookRight => [key(K::ArrowRight), None],
    }
}

fn default_pad(action: Action) -> Option<GamepadButton> {
    use Action::*;
    use GamepadButton as B;
    match action {
        Jump => Some(B::South),
        Light => Some(B::RightTrigger),
        Heavy => Some(B::RightTrigger2),
        Evade => Some(B::East),
        Block => Some(B::LeftTrigger),
        Parry => Some(B::LeftTrigger2),
        LockOn => Some(B::RightThumb),
        Interact => Some(B::North),
        QuickUse => Some(B::West),
        Ranged => Some(B::DPadLeft),
        AbilityCycle => Some(B::LeftThumb),
        AbilityUse => Some(B::DPadDown),
        Equipment => Some(B::Select),
        QuestLog => Some(B::DPadRight),
        Talk => Some(B::DPadUp),
        Menu => Some(B::Start),
        _ => None,
    }
}

impl Default for Binds {
    fn default() -> Self {
        Binds {
            keys: Action::ALL.iter().map(|&a| (a, default_keys(a))).collect(),
            pad: Action::ALL.iter().map(|&a| (a, default_pad(a))).collect(),
        }
    }
}

impl Binds {
    pub fn chords(&self, a: Action) -> [Option<Chord>; 2] {
        self.keys.get(&a).copied().unwrap_or([None, None])
    }

    pub fn pad_button(&self, a: Action) -> Option<GamepadButton> {
        self.pad.get(&a).copied().flatten()
    }

    /// Puts `chord` in `slot` (0 or 1) of `action`. A chord is the action's alone: an action that had it
    /// loses it (and is returned, so the menu can say so); the same chord in the action's other slot
    /// moves.
    pub fn assign(&mut self, action: Action, slot: usize, chord: Chord) -> Displaced {
        let slot = slot.min(1);
        let mut displaced = None;
        for other in Action::ALL {
            let entry = self.keys.entry(other).or_insert([None, None]);
            for (s, held) in entry.iter_mut().enumerate() {
                if *held == Some(chord) && !(other == action && s == slot) {
                    *held = None;
                    if other != action {
                        displaced = Some(other);
                    }
                }
            }
        }
        self.keys.entry(action).or_insert([None, None])[slot] = Some(chord);
        displaced
    }

    pub fn clear(&mut self, action: Action, slot: usize) {
        self.keys.entry(action).or_insert([None, None])[slot.min(1)] = None;
    }

    /// Puts a pad button on `action`; the action that had the button loses it.
    pub fn assign_pad(&mut self, action: Action, button: GamepadButton) -> Displaced {
        let mut displaced = None;
        for other in Action::ALL {
            if other != action && self.pad_button(other) == Some(button) {
                self.pad.insert(other, None);
                displaced = Some(other);
            }
        }
        self.pad.insert(action, Some(button));
        displaced
    }

    pub fn clear_pad(&mut self, action: Action) {
        self.pad.insert(action, None);
    }

    pub fn reset(&mut self) {
        *self = Binds::default();
    }

    #[cfg(test)]
    pub fn is_default(&self) -> bool {
        *self == Binds::default()
    }

    fn all_chords(&self) -> impl Iterator<Item = Chord> + '_ {
        self.keys.values().flatten().flatten().copied()
    }

    /// Whether `chord` is active under `held`, by the level (`pressed`) or the edge (`just_pressed`).
    fn chord_active(&self, chord: Chord, held: &Held, edge: bool) -> bool {
        let down = match chord.source {
            Source::Key(k) => {
                if edge {
                    held.keys.just_pressed(k)
                } else {
                    held.keys.pressed(k)
                }
            }
            Source::Mouse(b) => {
                held.captured
                    && if edge {
                        held.mouse.just_pressed(b)
                    } else {
                        held.mouse.pressed(b)
                    }
            }
        };
        if !down {
            return false;
        }
        let ignoring = match chord.source {
            Source::Key(k) => Some(k),
            Source::Mouse(_) => None,
        };
        let now = held_mods(held.keys, ignoring);
        if !chord.mods.within(now) {
            return false;
        }
        // A more specific chord on the same source, whose modifiers are also held, takes the press.
        !self.all_chords().any(|c| {
            c.source == chord.source
                && c.mods != chord.mods
                && chord.mods.within(c.mods)
                && c.mods.count() > chord.mods.count()
                && c.mods.within(now)
        })
    }

    fn pad_active(&self, action: Action, held: &Held, edge: bool) -> bool {
        self.pad_button(action).is_some_and(|b| {
            held.pads.iter().any(|p| {
                if edge {
                    p.just_pressed(b)
                } else {
                    p.pressed(b)
                }
            })
        })
    }

    /// The action is held down now (keys, a mouse button with the cursor captured, or its pad button).
    pub fn pressed(&self, action: Action, held: &Held) -> bool {
        self.chords(action)
            .into_iter()
            .flatten()
            .any(|c| self.chord_active(c, held, false))
            || self.pad_active(action, held, false)
    }

    /// The action went down this frame.
    pub fn just_pressed(&self, action: Action, held: &Held) -> bool {
        self.chords(action)
            .into_iter()
            .flatten()
            .any(|c| self.chord_active(c, held, true))
            || self.pad_active(action, held, true)
    }

    /// The action's key and mouse chords went down this frame (its pad button left out): for a site
    /// that gives the pad's button another job while a screen is up.
    pub fn just_pressed_keys(&self, action: Action, held: &Held) -> bool {
        self.chords(action)
            .into_iter()
            .flatten()
            .any(|c| self.chord_active(c, held, true))
    }

    /// [`Binds::pressed`] for a site that holds the input resources and a gamepad query.
    pub fn pressed_in(
        &self,
        action: Action,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
        pads: &Query<&Gamepad>,
        captured: bool,
    ) -> bool {
        let pads: Vec<&Gamepad> = pads.iter().collect();
        self.pressed(
            action,
            &Held {
                keys,
                mouse,
                pads: &pads,
                captured,
            },
        )
    }

    /// [`Binds::just_pressed`] for a site that holds the input resources and a gamepad query.
    pub fn just_pressed_in(
        &self,
        action: Action,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
        pads: &Query<&Gamepad>,
        captured: bool,
    ) -> bool {
        let pads: Vec<&Gamepad> = pads.iter().collect();
        self.just_pressed(
            action,
            &Held {
                keys,
                mouse,
                pads: &pads,
                captured,
            },
        )
    }

    /// `+1` for `positive` held, `-1` for `negative`, 0 for both or neither (keys and pad buttons).
    pub fn axis(&self, negative: Action, positive: Action, held: &Held) -> f32 {
        f32::from(u8::from(self.pressed(positive, held)))
            - f32::from(u8::from(self.pressed(negative, held)))
    }

    /// The keyboard and mouse bindings, written out: `F / Mouse L`, or `-`.
    #[cfg(test)]
    pub fn label(&self, action: Action) -> String {
        let parts: Vec<String> = self
            .chords(action)
            .into_iter()
            .flatten()
            .map(chord_label)
            .collect();
        if parts.is_empty() {
            "-".to_string()
        } else {
            parts.join(" / ")
        }
    }

    /// The short prompt for an action: its first key and its pad button, `X / Y`; `-` when unbound.
    pub fn hint(&self, action: Action) -> String {
        let mut parts = Vec::new();
        if let Some(chord) = self.chords(action).into_iter().flatten().next() {
            parts.push(chord_label(chord));
        }
        if let Some(button) = self.pad_button(action) {
            parts.push(crate::pad::button_label(button));
        }
        if parts.is_empty() {
            "-".to_string()
        } else {
            parts.join(" / ")
        }
    }

    /// The action's keys for the help line: short (`F/LMB`), `-` when unbound.
    pub fn short(&self, action: Action) -> String {
        let parts: Vec<String> = self
            .chords(action)
            .into_iter()
            .flatten()
            .map(|c| {
                chord_label(c)
                    .replace("Mouse L", "LMB")
                    .replace("Mouse R", "RMB")
                    .replace("Mouse M", "MMB")
                    .replace("Left ", "L")
                    .replace("Right ", "R")
            })
            .collect();
        if parts.is_empty() {
            "-".to_string()
        } else {
            parts.join("/")
        }
    }

    /// The two help lines of the HUD, from the bindings: the keys, then the pad (without the pad status).
    pub fn help_lines(&self) -> (String, String) {
        let moves = [
            Action::MoveForward,
            Action::MoveLeft,
            Action::MoveBack,
            Action::MoveRight,
        ]
        .map(|a| self.short(a));
        let wasd = if moves == ["W", "A", "S", "D"] {
            "WASD".to_string()
        } else {
            moves.join("/")
        };
        let key = |a: Action, what: &str| format!("{} {what}", self.short(a));
        let keys = format!(
            "keys: {wasd} move  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}  F1 free cam  F2 capsule  F6 reset  F3 hide",
            key(Action::Walk, "walk"),
            key(Action::Light, "light"),
            key(Action::Heavy, "heavy"),
            key(Action::Block, "block"),
            key(Action::Parry, "parry"),
            key(Action::Evade, "roll"),
            key(Action::Jump, "jump"),
            key(Action::LockOn, "lock"),
            key(Action::Ranged, "ranged"),
            key(Action::TwoHand, "two-hand"),
            key(Action::Interact, "use"),
            key(Action::Pack, "pack"),
            key(Action::Equipment, "gear"),
            key(Action::Menu, "menu"),
        );
        let pad = |a: Action, what: &str| {
            self.pad_button(a)
                .map(|b| format!("{} {what}", crate::pad::button_label(b)))
        };
        let buttons: Vec<String> = [
            pad(Action::Jump, "jump"),
            pad(Action::Evade, "roll"),
            pad(Action::Light, "light"),
            pad(Action::Heavy, "heavy"),
            pad(Action::Block, "block"),
            pad(Action::Parry, "parry"),
            pad(Action::LockOn, "lock"),
            pad(Action::Menu, "menu"),
        ]
        .into_iter()
        .flatten()
        .chain([
            "left stick move".to_string(),
            "right stick look".to_string(),
        ])
        .collect();
        (keys, format!("({})", buttons.join("  ")))
    }

    pub fn pad_label(&self, action: Action) -> String {
        self.pad_button(action)
            .map_or_else(|| "-".to_string(), crate::pad::button_label)
    }
}

// ---------------------------------------------------------------------------------------------
// Names.

macro_rules! keys {
    ($($code:ident => $label:expr),* $(,)?) => {
        /// Every key a binding may name: the code, its file id (the Bevy variant's name) and its label.
        pub const KEYS: &[(KeyCode, &str, &str)] = &[
            $((KeyCode::$code, stringify!($code), $label)),*
        ];
    };
}

keys! {
    KeyA => "A", KeyB => "B", KeyC => "C", KeyD => "D", KeyE => "E", KeyF => "F", KeyG => "G",
    KeyH => "H", KeyI => "I", KeyJ => "J", KeyK => "K", KeyL => "L", KeyM => "M", KeyN => "N",
    KeyO => "O", KeyP => "P", KeyQ => "Q", KeyR => "R", KeyS => "S", KeyT => "T", KeyU => "U",
    KeyV => "V", KeyW => "W", KeyX => "X", KeyY => "Y", KeyZ => "Z",
    Digit0 => "0", Digit1 => "1", Digit2 => "2", Digit3 => "3", Digit4 => "4", Digit5 => "5",
    Digit6 => "6", Digit7 => "7", Digit8 => "8", Digit9 => "9",
    F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6", F7 => "F7",
    F8 => "F8", F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12",
    Space => "Space", Tab => "Tab", Enter => "Enter", Backspace => "Backspace", Escape => "Esc",
    Delete => "Delete", Insert => "Insert", Home => "Home", End => "End", PageUp => "Page Up",
    PageDown => "Page Down", CapsLock => "Caps Lock",
    ArrowUp => "Up", ArrowDown => "Down", ArrowLeft => "Left", ArrowRight => "Right",
    ShiftLeft => "Left Shift", ShiftRight => "Right Shift", ControlLeft => "Left Ctrl",
    ControlRight => "Right Ctrl", AltLeft => "Left Alt", AltRight => "Right Alt",
    Numpad0 => "Num 0", Numpad1 => "Num 1", Numpad2 => "Num 2", Numpad3 => "Num 3",
    Numpad4 => "Num 4", Numpad5 => "Num 5", Numpad6 => "Num 6", Numpad7 => "Num 7",
    Numpad8 => "Num 8", Numpad9 => "Num 9", NumpadAdd => "Num +", NumpadSubtract => "Num -",
    NumpadMultiply => "Num *", NumpadDivide => "Num /", NumpadEnter => "Num Enter",
    NumpadDecimal => "Num .",
    Minus => "-", Equal => "=", BracketLeft => "[", BracketRight => "]", Semicolon => ";",
    Quote => "'", Comma => ",", Period => ".", Slash => "/", Backslash => "\\",
    Backquote => "`",
}

/// The mouse buttons a binding may name: the button, its file id and its label.
pub const MOUSE: &[(MouseButton, &str, &str)] = &[
    (MouseButton::Left, "Left", "Mouse L"),
    (MouseButton::Right, "Right", "Mouse R"),
    (MouseButton::Middle, "Middle", "Mouse M"),
    (MouseButton::Back, "Back", "Mouse 4"),
    (MouseButton::Forward, "Forward", "Mouse 5"),
];

/// The gamepad buttons a binding may name, with their file ids.
pub const PAD: &[(GamepadButton, &str)] = &[
    (GamepadButton::South, "South"),
    (GamepadButton::East, "East"),
    (GamepadButton::North, "North"),
    (GamepadButton::West, "West"),
    (GamepadButton::LeftTrigger, "LeftTrigger"),
    (GamepadButton::LeftTrigger2, "LeftTrigger2"),
    (GamepadButton::RightTrigger, "RightTrigger"),
    (GamepadButton::RightTrigger2, "RightTrigger2"),
    (GamepadButton::Select, "Select"),
    (GamepadButton::Start, "Start"),
    (GamepadButton::LeftThumb, "LeftThumb"),
    (GamepadButton::RightThumb, "RightThumb"),
    (GamepadButton::DPadUp, "DPadUp"),
    (GamepadButton::DPadDown, "DPadDown"),
    (GamepadButton::DPadLeft, "DPadLeft"),
    (GamepadButton::DPadRight, "DPadRight"),
];

/// Whether a key is one of the modifier keys.
pub fn is_modifier(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
    )
}

/// A chord as the player reads it: `Shift + Space`, `F`, `Mouse L`.
pub fn chord_label(chord: Chord) -> String {
    let mut out = String::new();
    for (on, name) in [
        (chord.mods.shift, "Shift"),
        (chord.mods.ctrl, "Ctrl"),
        (chord.mods.alt, "Alt"),
    ] {
        if on {
            out.push_str(name);
            out.push_str(" + ");
        }
    }
    out.push_str(match chord.source {
        Source::Key(k) => KEYS
            .iter()
            .find(|(code, _, _)| *code == k)
            .map_or("?", |(_, _, label)| label),
        Source::Mouse(b) => MOUSE
            .iter()
            .find(|(button, _, _)| *button == b)
            .map_or("Mouse ?", |(_, _, label)| label),
    });
    out
}

/// A chord as the file writes it: `Shift+Space`, `KeyF`, `Mouse:Left`.
pub fn chord_id(chord: Chord) -> Option<String> {
    let mut out = String::new();
    for (on, name) in [
        (chord.mods.shift, "Shift"),
        (chord.mods.ctrl, "Ctrl"),
        (chord.mods.alt, "Alt"),
    ] {
        if on {
            out.push_str(name);
            out.push('+');
        }
    }
    match chord.source {
        Source::Key(k) => out.push_str(KEYS.iter().find(|(code, _, _)| *code == k)?.1),
        Source::Mouse(b) => {
            out.push_str("Mouse:");
            out.push_str(MOUSE.iter().find(|(button, _, _)| *button == b)?.1);
        }
    }
    Some(out)
}

/// Reads [`chord_id`]'s text back.
pub fn parse_chord(text: &str) -> Option<Chord> {
    let mut rest = text.trim();
    let mut mods = Mods::NONE;
    loop {
        if let Some(r) = rest.strip_prefix("Shift+") {
            mods.shift = true;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("Ctrl+") {
            mods.ctrl = true;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("Alt+") {
            mods.alt = true;
            rest = r;
        } else {
            break;
        }
    }
    let source = if let Some(b) = rest.strip_prefix("Mouse:") {
        Source::Mouse(MOUSE.iter().find(|(_, id, _)| *id == b)?.0)
    } else {
        Source::Key(KEYS.iter().find(|(_, id, _)| *id == rest)?.0)
    };
    Some(Chord { source, mods })
}

pub fn pad_id(button: GamepadButton) -> Option<&'static str> {
    PAD.iter().find(|(b, _)| *b == button).map(|(_, id)| *id)
}

pub fn parse_pad(text: &str) -> Option<GamepadButton> {
    PAD.iter()
        .find(|(_, id)| *id == text.trim())
        .map(|(b, _)| *b)
}

/// The text of one binding line's value, or `-`.
fn chord_text(chord: Option<Chord>) -> String {
    chord.and_then(chord_id).unwrap_or_else(|| "-".to_string())
}

impl Binds {
    /// `bind <action> <chord 1> <chord 2>` and `pad <action> <button>` lines for the settings file.
    pub fn to_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for a in Action::ALL {
            let [one, two] = self.chords(a);
            lines.push(format!(
                "bind {} {} {}",
                a.id(),
                chord_text(one),
                chord_text(two)
            ));
            lines.push(format!(
                "pad {} {}",
                a.id(),
                self.pad_button(a).and_then(pad_id).unwrap_or("-")
            ));
        }
        lines
    }

    /// One `bind` or `pad` line (the part after the keyword): `Err` names what is wrong.
    pub fn read_line(&mut self, keyword: &str, rest: &str) -> Result<(), String> {
        let mut words = rest.split_whitespace();
        let id = words.next().ok_or("no action")?;
        let action = Action::from_id(id).ok_or_else(|| format!("unknown action {id:?}"))?;
        match keyword {
            "bind" => {
                let mut slots = [None, None];
                for slot in &mut slots {
                    match words.next() {
                        None | Some("-") => {}
                        Some(text) => {
                            *slot = Some(
                                parse_chord(text).ok_or_else(|| format!("unknown key {text:?}"))?,
                            );
                        }
                    }
                }
                self.keys.insert(action, slots);
                Ok(())
            }
            "pad" => {
                let button = match words.next() {
                    None | Some("-") => None,
                    Some(text) => {
                        Some(parse_pad(text).ok_or_else(|| format!("unknown button {text:?}"))?)
                    }
                };
                self.pad.insert(action, button);
                Ok(())
            }
            other => Err(format!("unknown line {other:?}")),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The system parameter.

/// The bindings and the input they are asked against, for a system: `controls.pressed(Action::Block)`.
/// Every action is false while the system menu is up ([`MenuUp`]), so no site needs its own gate.
#[derive(SystemParam)]
pub struct Controls<'w, 's> {
    binds: Res<'w, super::Settings>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    pads: Query<'w, 's, &'static Gamepad>,
    window: Query<'w, 's, &'static CursorOptions, With<PrimaryWindow>>,
    menu: Res<'w, MenuUp>,
}

/// Set while the system menu is up: nothing is bound then.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuUp(pub bool);

impl Controls<'_, '_> {
    fn with_held<R>(&self, f: impl FnOnce(&Binds, &Held) -> R) -> R {
        let pads: Vec<&Gamepad> = self.pads.iter().collect();
        let held = Held {
            keys: &self.keys,
            mouse: &self.mouse,
            pads: &pads,
            captured: self
                .window
                .single()
                .is_ok_and(|c| c.grab_mode == CursorGrabMode::Locked),
        };
        f(&self.binds.binds, &held)
    }

    pub fn just_pressed(&self, action: Action) -> bool {
        !self.menu.0 && self.with_held(|b, h| b.just_pressed(action, h))
    }

    /// [`Binds::just_pressed_keys`]: the key and mouse bindings only.
    pub fn just_pressed_keys(&self, action: Action) -> bool {
        !self.menu.0 && self.with_held(|b, h| b.just_pressed_keys(action, h))
    }

    /// Runs `f` over the bindings and the input snapshot (for the pure helpers that take both); the
    /// type's default while the menu is up, as every other lookup.
    pub fn read<R: Default>(&self, f: impl FnOnce(&Binds, &Held) -> R) -> R {
        if self.menu.0 {
            R::default()
        } else {
            self.with_held(f)
        }
    }

    pub fn binds(&self) -> &Binds {
        &self.binds.binds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rig {
        keys: ButtonInput<KeyCode>,
        mouse: ButtonInput<MouseButton>,
    }

    impl Rig {
        fn new() -> Rig {
            Rig {
                keys: ButtonInput::default(),
                mouse: ButtonInput::default(),
            }
        }

        fn held(&self, captured: bool) -> Held<'_> {
            Held {
                keys: &self.keys,
                mouse: &self.mouse,
                pads: &[],
                captured,
            }
        }
    }

    #[test]
    fn the_ids_are_unique_and_read_back() {
        let mut seen = std::collections::HashSet::new();
        for a in Action::ALL {
            assert!(seen.insert(a.id()), "{a:?}");
            assert_eq!(Action::from_id(a.id()), Some(a));
            assert!(!a.label().is_empty());
        }
        assert_eq!(seen.len(), Action::ALL.len());
        assert_eq!(Action::from_id("nonsense"), None);
    }

    #[test]
    fn the_defaults_are_the_keys_the_modules_used_to_hard_code() {
        let b = Binds::default();
        let one = |a| b.chords(a)[0];
        assert_eq!(one(Action::MoveForward), Some(Chord::key(KeyCode::KeyW)));
        assert_eq!(one(Action::Light), Some(Chord::key(KeyCode::KeyF)));
        assert_eq!(
            b.chords(Action::Light)[1],
            Some(Chord::mouse(MouseButton::Left))
        );
        assert_eq!(one(Action::Heavy), Some(Chord::key(KeyCode::KeyR)));
        assert_eq!(one(Action::Evade), Some(Chord::key(KeyCode::Space)));
        assert_eq!(
            one(Action::Jump),
            Some(Chord::key(KeyCode::Space).with(Mods {
                shift: true,
                ..Mods::NONE
            }))
        );
        assert_eq!(one(Action::Block), Some(Chord::key(KeyCode::KeyE)));
        assert_eq!(one(Action::Parry), Some(Chord::key(KeyCode::KeyC)));
        assert_eq!(one(Action::LockOn), Some(Chord::key(KeyCode::KeyQ)));
        assert_eq!(one(Action::Pack), Some(Chord::key(KeyCode::Tab)));
        assert_eq!(one(Action::Equipment), Some(Chord::key(KeyCode::KeyG)));
        assert_eq!(one(Action::Menu), Some(Chord::key(KeyCode::Escape)));
        assert_eq!(
            b.pad_button(Action::Light),
            Some(GamepadButton::RightTrigger)
        );
        assert_eq!(
            b.pad_button(Action::Heavy),
            Some(GamepadButton::RightTrigger2)
        );
        assert_eq!(b.pad_button(Action::Evade), Some(GamepadButton::East));
        assert_eq!(b.pad_button(Action::Jump), Some(GamepadButton::South));
        assert_eq!(
            b.pad_button(Action::Block),
            Some(GamepadButton::LeftTrigger)
        );
        assert_eq!(
            b.pad_button(Action::Parry),
            Some(GamepadButton::LeftTrigger2)
        );
        assert_eq!(b.pad_button(Action::Menu), Some(GamepadButton::Start));
        assert_eq!(b.pad_button(Action::Pack), None, "Start is the menu now");
    }

    #[test]
    fn no_chord_is_bound_to_two_actions_by_default() {
        let b = Binds::default();
        let mut seen: HashMap<Chord, Action> = HashMap::new();
        for a in Action::ALL {
            for chord in b.chords(a).into_iter().flatten() {
                if let Some(other) = seen.insert(chord, a) {
                    panic!("{chord:?} is bound to {other:?} and {a:?}");
                }
            }
        }
        let mut pads: HashMap<GamepadButton, Action> = HashMap::new();
        for a in Action::ALL {
            if let Some(button) = b.pad_button(a) {
                if let Some(other) = pads.insert(button, a) {
                    panic!("{button:?} is bound to {other:?} and {a:?}");
                }
            }
        }
    }

    #[test]
    fn space_rolls_shift_space_jumps_and_ctrl_space_still_rolls() {
        let b = Binds::default();
        let mut rig = Rig::new();
        rig.keys.press(KeyCode::Space);
        assert!(b.just_pressed(Action::Evade, &rig.held(false)));
        assert!(!b.just_pressed(Action::Jump, &rig.held(false)));

        let mut rig = Rig::new();
        rig.keys.press(KeyCode::ShiftLeft);
        rig.keys.press(KeyCode::Space);
        assert!(b.just_pressed(Action::Jump, &rig.held(false)));
        assert!(
            !b.just_pressed(Action::Evade, &rig.held(false)),
            "shift makes Space the jump and only the jump"
        );

        let mut rig = Rig::new();
        rig.keys.press(KeyCode::ControlLeft);
        rig.keys.press(KeyCode::Space);
        assert!(
            b.just_pressed(Action::Evade, &rig.held(false)),
            "walking and rolling at once"
        );
        assert!(b.pressed(Action::Walk, &rig.held(false)));
    }

    #[test]
    fn walk_holds_with_either_ctrl_or_both() {
        let b = Binds::default();
        for keys in [
            vec![KeyCode::ControlLeft],
            vec![KeyCode::ControlRight],
            vec![KeyCode::ControlLeft, KeyCode::ControlRight],
        ] {
            let mut rig = Rig::new();
            for k in keys {
                rig.keys.press(k);
            }
            assert!(b.pressed(Action::Walk, &rig.held(false)));
        }
        assert!(!b.pressed(Action::Walk, &Rig::new().held(false)));
    }

    #[test]
    fn the_mouse_counts_only_with_the_cursor_captured() {
        let b = Binds::default();
        let mut rig = Rig::new();
        rig.mouse.press(MouseButton::Left);
        assert!(b.just_pressed(Action::Light, &rig.held(true)));
        assert!(
            !b.just_pressed(Action::Light, &rig.held(false)),
            "the capturing click is no swing"
        );
        let mut rig = Rig::new();
        rig.keys.press(KeyCode::KeyF);
        assert!(b.just_pressed(Action::Light, &rig.held(false)), "F always");
    }

    #[test]
    fn movement_keys_make_an_axis() {
        let b = Binds::default();
        let mut rig = Rig::new();
        rig.keys.press(KeyCode::KeyD);
        assert_eq!(
            b.axis(Action::MoveLeft, Action::MoveRight, &rig.held(false)),
            1.0
        );
        rig.keys.press(KeyCode::KeyA);
        assert_eq!(
            b.axis(Action::MoveLeft, Action::MoveRight, &rig.held(false)),
            0.0,
            "both cancel"
        );
    }

    #[test]
    fn assigning_a_key_takes_it_from_its_owner_and_says_who() {
        let mut b = Binds::default();
        let displaced = b.assign(Action::Heavy, 0, Chord::key(KeyCode::KeyF));
        assert_eq!(displaced, Some(Action::Light));
        assert_eq!(b.chords(Action::Heavy)[0], Some(Chord::key(KeyCode::KeyF)));
        assert_eq!(b.chords(Action::Light)[0], None, "light lost F");
        assert_eq!(
            b.chords(Action::Light)[1],
            Some(Chord::mouse(MouseButton::Left)),
            "its mouse slot is untouched"
        );
        // The same key again into the other slot of the same action moves it.
        assert_eq!(b.assign(Action::Heavy, 1, Chord::key(KeyCode::KeyF)), None);
        assert_eq!(
            b.chords(Action::Heavy),
            [None, Some(Chord::key(KeyCode::KeyF))]
        );
        b.reset();
        assert!(b.is_default());
    }

    #[test]
    fn a_pad_button_goes_to_one_action() {
        let mut b = Binds::default();
        let displaced = b.assign_pad(Action::Pack, GamepadButton::North);
        assert_eq!(displaced, Some(Action::Interact));
        assert_eq!(b.pad_button(Action::Pack), Some(GamepadButton::North));
        assert_eq!(b.pad_button(Action::Interact), None);
        b.clear_pad(Action::Pack);
        assert_eq!(b.pad_button(Action::Pack), None);
    }

    #[test]
    fn every_listed_key_and_button_round_trips_through_its_text() {
        for (code, id, label) in KEYS {
            let chord = Chord::key(*code);
            assert_eq!(chord_id(chord).as_deref(), Some(*id));
            assert_eq!(parse_chord(id), Some(chord), "{id}");
            assert!(!label.is_empty());
            let chord = chord.with(Mods {
                shift: true,
                ctrl: true,
                alt: true,
            });
            assert_eq!(parse_chord(&chord_id(chord).unwrap()), Some(chord));
        }
        for (button, id, _) in MOUSE {
            let chord = Chord::mouse(*button);
            assert_eq!(parse_chord(&chord_id(chord).unwrap()), Some(chord), "{id}");
        }
        for (button, id) in PAD {
            assert_eq!(parse_pad(id), Some(*button));
            assert_eq!(pad_id(*button), Some(*id));
        }
        assert_eq!(parse_chord("Nonsense"), None);
        assert_eq!(parse_chord("Shift+"), None);
        assert_eq!(parse_pad("Nonsense"), None);
    }

    #[test]
    fn the_key_ids_are_unique() {
        let mut ids = std::collections::HashSet::new();
        let mut codes = std::collections::HashSet::new();
        for (code, id, _) in KEYS {
            assert!(ids.insert(*id), "{id}");
            assert!(codes.insert(*code), "{id}");
        }
    }

    #[test]
    fn the_help_lines_name_the_bindings_in_use() {
        let mut b = Binds::default();
        let (keys, pad) = b.help_lines();
        assert!(keys.contains("WASD move"), "{keys}");
        assert!(keys.contains("F/LMB light"), "{keys}");
        assert!(keys.contains("R/RMB heavy"), "{keys}");
        assert!(keys.contains("Space roll"), "{keys}");
        assert!(keys.contains("Shift + Space jump"), "{keys}");
        assert!(keys.contains("Esc menu"), "{keys}");
        assert!(pad.contains("A jump") && pad.contains("RB light"), "{pad}");
        assert!(pad.contains("Menu menu"), "{pad}");
        b.assign(Action::Light, 0, Chord::key(KeyCode::KeyG));
        b.assign(Action::MoveForward, 0, Chord::key(KeyCode::ArrowUp));
        let (keys, _) = b.help_lines();
        assert!(keys.contains("G/LMB light"), "{keys}");
        assert!(keys.contains("Up/A/S/D move"), "{keys}");
    }

    #[test]
    fn labels_read_as_a_player_would() {
        let b = Binds::default();
        assert_eq!(b.label(Action::Light), "F / Mouse L");
        assert_eq!(b.label(Action::Jump), "Shift + Space");
        assert_eq!(b.label(Action::AbilityUse), "-");
        assert_eq!(b.pad_label(Action::Evade), "B");
        assert_eq!(b.pad_label(Action::Pack), "-");
    }

    #[test]
    fn the_lines_of_a_file_load_back_to_the_same_binds() {
        let mut b = Binds::default();
        b.assign(Action::Heavy, 0, Chord::key(KeyCode::KeyG));
        b.assign_pad(Action::Talk, GamepadButton::LeftThumb);
        b.clear(Action::Walk, 1);
        let mut read = Binds::default();
        for line in b.to_lines() {
            let (keyword, rest) = line.split_once(' ').unwrap();
            read.read_line(keyword, rest).unwrap();
        }
        assert_eq!(read, b);
        assert!(read.read_line("bind", "nonsense KeyA -").is_err());
        assert!(read.read_line("bind", "light NotAKey -").is_err());
        assert!(read.read_line("pad", "light NotAButton").is_err());
        assert!(read.read_line("bind", "").is_err());
    }
}
