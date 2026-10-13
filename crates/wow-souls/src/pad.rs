//! The gamepad.
//!
//! benilla turns Bevy's gamepad plugin off in `tuned_default_plugins` ("No gamepad input; 1.12's
//! bindings are keyboard/mouse"), so with it off no `Gamepad` entity ever existed and every pad
//! binding in [`crate::controller`], [`crate::camera`], [`crate::lockon`] and [`crate::combat`]
//! was dead code. [`crate::boot`] switches the plugin back on; Bevy then spawns a `Gamepad`, named
//! by the OS, when a pad connects and removes the component when it goes.
//!
//! This module reports what the game sees: [`PadStatus`] holds the first pad's name and the last
//! button pressed, the log says when a pad connects or goes, and the help line ([`describe`]) shows
//! both, so a pad that is plugged in but not heard is visible at a glance.

use bevy::prelude::*;

/// What the game sees of the gamepad.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct PadStatus {
    /// The first connected pad's name, the OS's (e.g. "HID-compliant game controller").
    pub name: Option<String>,
    /// The last button pressed on it.
    pub last: Option<GamepadButton>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<PadStatus>().add_systems(Update, track);
}

/// Follows the first pad: its name and its last button; logs a connect and a disconnect (and,
/// under `WOWSOULS_TRACE`, every press).
fn track(pads: Query<(&Gamepad, Option<&Name>)>, mut status: ResMut<PadStatus>) {
    let first = pads.iter().next();
    let name = first.map(|(_, n)| n.map_or_else(|| "gamepad".to_string(), |n| n.to_string()));
    if name != status.name {
        match &name {
            Some(n) => info!("gamepad: connected '{n}'"),
            None => info!("gamepad: disconnected"),
        }
        *status = PadStatus { name, last: None };
    }
    if let Some(&button) = first.and_then(|(pad, _)| pad.get_just_pressed().next()) {
        if crate::actor::anim::trace_on() {
            info!("gamepad: {} pressed", button_label(button));
        }
        status.last = Some(button);
    }
}

/// A button as printed on an Xbox pad.
pub fn button_label(b: GamepadButton) -> String {
    use GamepadButton::*;
    let label = match b {
        South => "A",
        East => "B",
        West => "X",
        North => "Y",
        LeftTrigger => "LB",
        RightTrigger => "RB",
        LeftTrigger2 => "LT",
        RightTrigger2 => "RT",
        LeftThumb => "LS",
        RightThumb => "RS",
        Select => "View",
        Start => "Menu",
        DPadUp => "Up",
        DPadDown => "Down",
        DPadLeft => "Left",
        DPadRight => "Right",
        other => return format!("{other:?}"),
    };
    label.to_string()
}

/// The help line's pad part: `pad: none`, or the pad's name and its last button. ASCII only: the
/// default font has no other glyphs.
pub fn describe(status: &PadStatus) -> String {
    let ascii = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_graphic() || c == ' ' {
                    c
                } else {
                    '?'
                }
            })
            .collect()
    };
    match (&status.name, status.last) {
        (None, _) => "pad: none".to_string(),
        (Some(name), None) => format!("pad: {}", ascii(name)),
        (Some(name), Some(b)) => format!("pad: {} (last {})", ascii(name), button_label(b)),
    }
}

#[cfg(test)]
pub mod testkit {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    /// A world holding one connected pad, which `setup` presses and pushes.
    pub fn world_with_pad(setup: impl FnOnce(&mut Gamepad)) -> World {
        let mut world = World::new();
        let mut pad = Gamepad::default();
        setup(&mut pad);
        world.spawn((Name::new("Test pad"), pad));
        world
    }

    /// Runs `f` over the world's pads, as a system would.
    pub fn read<T: Send + 'static>(
        world: &mut World,
        f: impl FnOnce(&Query<&Gamepad>) -> T + Send + Sync + 'static,
    ) -> T {
        let mut f = Some(f);
        world
            .run_system_once(move |pads: Query<&Gamepad>| (f.take().expect("runs once"))(&pads))
            .expect("the system runs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn world() -> World {
        let mut w = World::new();
        w.init_resource::<PadStatus>();
        w
    }

    fn status(world: &mut World) -> PadStatus {
        world.run_system_once(track).expect("track runs");
        world.resource::<PadStatus>().clone()
    }

    #[test]
    fn without_a_pad_the_status_is_empty() {
        let mut w = world();
        assert_eq!(status(&mut w), PadStatus::default());
    }

    #[test]
    fn a_connected_pad_is_named() {
        let mut w = world();
        w.spawn((Name::new("Xbox Wireless Controller"), Gamepad::default()));
        assert_eq!(
            status(&mut w).name.as_deref(),
            Some("Xbox Wireless Controller")
        );
    }

    #[test]
    fn the_last_button_pressed_is_kept() {
        let mut w = world();
        let pad = w.spawn((Name::new("p"), Gamepad::default())).id();
        let press = |w: &mut World, b: GamepadButton| {
            let mut g = w.get_mut::<Gamepad>(pad).unwrap();
            g.digital_mut().clear();
            g.digital_mut().press(b);
        };
        press(&mut w, GamepadButton::RightTrigger);
        assert_eq!(status(&mut w).last, Some(GamepadButton::RightTrigger));
        // No new press: it stays. A later one replaces it.
        w.get_mut::<Gamepad>(pad).unwrap().digital_mut().clear();
        assert_eq!(status(&mut w).last, Some(GamepadButton::RightTrigger));
        press(&mut w, GamepadButton::East);
        assert_eq!(status(&mut w).last, Some(GamepadButton::East));
    }

    #[test]
    fn a_pad_that_goes_clears_the_status() {
        let mut w = world();
        let pad = w.spawn((Name::new("p"), Gamepad::default())).id();
        w.get_mut::<Gamepad>(pad)
            .unwrap()
            .digital_mut()
            .press(GamepadButton::South);
        let seen = status(&mut w);
        assert!(seen.name.is_some() && seen.last.is_some());
        // Bevy removes the `Gamepad` component on a disconnect and keeps the entity.
        w.entity_mut(pad).remove::<Gamepad>();
        assert_eq!(status(&mut w), PadStatus::default());
    }

    #[test]
    fn buttons_read_as_an_xbox_pad_prints_them() {
        use GamepadButton::*;
        let labels: Vec<_> = [
            South,
            East,
            West,
            North,
            LeftTrigger,
            RightTrigger,
            LeftTrigger2,
            RightTrigger2,
            LeftThumb,
            RightThumb,
            Select,
            Start,
            DPadUp,
        ]
        .map(button_label)
        .into();
        assert_eq!(
            labels,
            ["A", "B", "X", "Y", "LB", "RB", "LT", "RT", "LS", "RS", "View", "Menu", "Up"]
        );
        assert_eq!(button_label(Other(7)), "Other(7)");
    }

    #[test]
    fn the_dev_line_says_what_the_game_sees() {
        assert_eq!(describe(&PadStatus::default()), "pad: none");
        let named = PadStatus {
            name: Some("Xbox".into()),
            last: None,
        };
        assert_eq!(describe(&named), "pad: Xbox");
        let pressed = PadStatus {
            name: Some("Xbox".into()),
            last: Some(GamepadButton::RightTrigger),
        };
        assert_eq!(describe(&pressed), "pad: Xbox (last RB)");
        // The default font is ASCII only.
        let swedish = PadStatus {
            name: Some("Spelenhet \u{e5}".into()),
            last: None,
        };
        assert_eq!(describe(&swedish), "pad: Spelenhet ?");
    }
}
