//! Whether anyone is at the keyboard. A run carrying a verification instrument
//! (`WOWSOULS_CHECK`, `_SHOT`, `_RECORD`, `_DRIVE`, `_SPIN`) is unattended: its window opens unfocused below
//! other windows, it never captures the cursor, and live mouse, keyboard and gamepad input is
//! ignored, so it neither takes over the desk nor gets steered by whoever is using it.
//! `WOWSOULS_BG=0` forces an attended run (to watch an instrumented run live); any other
//! `WOWSOULS_BG` forces unattended. benilla's `bgwin::background_run` is the model.

use bevy::prelude::*;

/// The instruments that make a run unattended. `WOWSOULS_TRACE` and `WOWSOULS_OBSERVE` are not:
/// a person can play with them on.
const UNATTENDED_ENVS: [&str; 5] = [
    "WOWSOULS_CHECK",
    "WOWSOULS_SHOT",
    "WOWSOULS_RECORD",
    "WOWSOULS_DRIVE",
    "WOWSOULS_SPIN",
];

/// Present when nobody is at the keyboard.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Unattended;

/// Decided once, from the environment.
pub fn unattended() -> bool {
    decide(std::env::var("WOWSOULS_BG").ok().as_deref(), |name| {
        std::env::var_os(name).is_some()
    })
}

fn decide(bg: Option<&str>, set: impl Fn(&str) -> bool) -> bool {
    match bg {
        Some("0") => false,
        Some(_) => true,
        None => UNATTENDED_ENVS.iter().any(|name| set(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_instrument_makes_the_run_unattended() {
        assert!(decide(None, |n| n == "WOWSOULS_SHOT"));
        assert!(decide(None, |n| n == "WOWSOULS_RECORD"));
        assert!(decide(None, |n| n == "WOWSOULS_DRIVE"));
    }

    #[test]
    fn a_plain_or_traced_run_is_attended() {
        assert!(!decide(None, |_| false));
        assert!(!decide(None, |n| n == "WOWSOULS_TRACE"));
    }

    #[test]
    fn bg_overrides_either_way() {
        assert!(!decide(Some("0"), |_| true));
        assert!(decide(Some("1"), |_| false));
    }
}
