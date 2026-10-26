//! The boars' charge as a rush through the player (2026-10-17, the user: "Charge ska vara en attack så
//! den ska välja en position när den kommer nära nog och springa igenom spelaren - typ som en fireball").
//! The Armored Tusk's charge clip rears back for a second, then bursts forward a yard and a half. Here
//! the burst is stretched into a run: as it begins the boar fixes its line (the way it faces after
//! tracking the player through the wind-up) and a length (through the player and [`OVERSHOOT`] past),
//! then covers it at [`SPEED`] while the clip's burst plays out over the run. It does not stop at the
//! player's edge and does not turn: a step aside or a roll through it is the answer. The blow is the
//! kit's own (the posed tusks in the clip's hit window), so a roll's invulnerability passes it as any.

use bevy::prelude::*;

/// The clip seconds of the charge's burst (body_attack_08: frames 36-45 of 81 at 30 fps; its hit window
/// opens at 1.233).
pub const BURST: (f32, f32) = (1.2, 1.5);
/// Yd/s of the run: past a running player (7), so it cannot simply be outrun.
pub const SPEED: f32 = 12.0;
/// How far past the player it carries on.
pub const OVERSHOOT: f32 = 3.0;
/// The shortest and the longest run (yd).
pub const RUN: (f32, f32) = (3.0, 13.0);
/// The charge move's name in the boar kits.
pub const MOVE: &str = "charge";
/// A charge that connects hits this many times harder than the kit's tamed number, and throws the
/// player further (2026-10-17, the user: "Boar charge bör göra mer skada ifall den träffar"): a full
/// run at 12 yd/s is the boar's big commitment, the gore its small one.
pub const DAMAGE: f32 = 2.2;
pub const KNOCK: f32 = 1.8;
pub const POISE: f32 = 45.0;

/// Whether a boar's move `name` is one of its charges (Princess has a long one too).
pub fn is_charge(name: &str) -> bool {
    matches!(name, "charge" | "long_charge")
}

/// A run under way: its line, its length and what is left of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rush {
    pub dir: Vec3,
    pub length: f32,
    pub left: f32,
}

impl Rush {
    /// A run along `forward` (flattened) that ends `OVERSHOOT` past a player `dist` yd away.
    pub fn start(forward: Vec3, dist: f32) -> Option<Rush> {
        let dir = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        if dir == Vec3::ZERO || !dist.is_finite() {
            return None;
        }
        let length = (dist + OVERSHOOT).clamp(RUN.0, RUN.1);
        Some(Rush {
            dir,
            length,
            left: length,
        })
    }

    /// Advances the run by `dt`: the yards it moves this frame, and the clip second it is at (the
    /// burst spread over the run).
    pub fn step(&mut self, dt: f32) -> (f32, f32) {
        let step = (SPEED * dt.max(0.0)).min(self.left);
        self.left -= step;
        if self.left < 1.0e-4 {
            self.left = 0.0;
        }
        let done = 1.0 - self.left / self.length;
        (step, BURST.0 + (BURST.1 - BURST.0) * done)
    }

    pub fn done(&self) -> bool {
        self.left <= 1.0e-4
    }
}

/// Whether a move of a boar starts a run at clip second `t` (crossing the burst's start this frame).
pub fn begins(before: f32, t: f32) -> bool {
    before < BURST.0 && t >= BURST.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_goes_through_the_player_and_past_at_its_speed() {
        let mut r = Rush::start(Vec3::new(0.0, 0.3, -2.0), 6.0).unwrap();
        assert_eq!(r.dir, Vec3::NEG_Z, "the line is flat");
        assert_eq!(r.length, 9.0, "six to the player, three past");
        let (step, t) = r.step(0.25);
        assert!((step - 3.0).abs() < 1.0e-5);
        assert!((t - (BURST.0 + (BURST.1 - BURST.0) / 3.0)).abs() < 1.0e-5);
        let mut frames = 0;
        while !r.done() {
            r.step(1.0 / 60.0);
            frames += 1;
        }
        assert!(
            (30..=31).contains(&frames),
            "the other six yards take half a second: {frames}"
        );
        assert_eq!(
            r.step(0.1),
            (0.0, BURST.1),
            "a finished run stays at the burst's end"
        );
    }

    #[test]
    fn a_run_is_never_shorter_or_longer_than_its_bounds() {
        assert_eq!(Rush::start(Vec3::NEG_Z, 0.0).unwrap().length, RUN.0);
        assert_eq!(Rush::start(Vec3::NEG_Z, 40.0).unwrap().length, RUN.1);
        assert!(Rush::start(Vec3::Y, 5.0).is_none(), "no line straight up");
    }

    #[test]
    fn the_run_begins_as_the_clip_crosses_the_burst() {
        assert!(!begins(1.0, 1.1));
        assert!(begins(1.19, 1.21));
        assert!(!begins(1.21, 1.3), "only once");
    }
}
