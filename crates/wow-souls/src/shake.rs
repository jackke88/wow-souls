//! Camera shake (ours: the blow params have no shake value and benilla's `camera_shake.rs` plays only
//! WoW's footstep and spell presets). A `trauma` in `0..=1` is kicked up by heavy blows and decays in
//! real time; the camera's offset is a deterministic sum of sines scaled by `trauma^2`.
//! The player's shake setting (the options menu, `WOWSOULS_SHAKE=0` at start) scales what a blow kicks;
//! 0 turns it off.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::hits::Hit;
use crate::npc::PlayerHurt;

/// Trauma lost per second: a full kick is gone in 0.4 s.
pub const DECAY: f32 = 2.5;
/// The turn at full trauma (rad, 1.2 deg) and the shift (yd).
const MAX_TURN: f32 = 0.0209;
const MAX_SHIFT: f32 = 0.04;

#[derive(Resource, Debug)]
pub struct Shake {
    pub trauma: f32,
    t: f32,
    /// The player's setting, 0 to 1: what a kick is scaled by.
    scale: f32,
}

impl Default for Shake {
    fn default() -> Self {
        Shake {
            trauma: 0.0,
            t: 0.0,
            scale: 1.0,
        }
    }
}

impl Shake {
    pub fn kick(&mut self, amount: f32) {
        if self.scale > 0.0 {
            self.trauma = (self.trauma + amount * self.scale).min(1.0);
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.clamp(0.0, 1.0);
    }

    pub fn tick(&mut self, dt: f32) {
        self.t += dt;
        self.trauma = (self.trauma - DECAY * dt).max(0.0);
    }

    /// The camera's shift (in its own frame) and tilt now.
    pub fn offset(&self) -> (Vec3, Quat) {
        shake_offset(self.trauma, self.t)
    }
}

/// The offset at `trauma` and time `t`: zero at no trauma, bounded by the maxima at full.
pub fn shake_offset(trauma: f32, t: f32) -> (Vec3, Quat) {
    let k = trauma.clamp(0.0, 1.0).powi(2);
    if k == 0.0 {
        return (Vec3::ZERO, Quat::IDENTITY);
    }
    let w = |hz: f32, phase: f32| (t * hz * TAU + phase).sin();
    let tilt = Quat::from_euler(
        EulerRot::YXZ,
        MAX_TURN * k * w(17.0, 0.3),
        MAX_TURN * k * w(23.0, 1.7),
        MAX_TURN * 0.5 * k * w(13.0, 2.9),
    );
    let shift = Vec3::new(w(19.0, 0.9), w(29.0, 2.2), 0.0) * (MAX_SHIFT * k);
    (shift, tilt)
}

/// The trauma of a blow of flinch `level`: taken by the player, or dealt by the player's sword
/// (a level-1 blow dealt is a light one and does not shake). Tuning values.
pub fn trauma_for(level: u8, taken: bool) -> f32 {
    match (taken, level) {
        (true, 0..=1) => 0.35,
        (true, 2..=4) => 0.55,
        (true, _) => 0.9,
        (false, 0..=1) => 0.0,
        (false, 2..=4) => 0.25,
        (false, _) => 0.4,
    }
}

fn tick(real: Res<Time<bevy::time::Real>>, mut shake: ResMut<Shake>) {
    shake.tick(real.delta_secs());
}

fn kick_on_blows(
    mut shake: ResMut<Shake>,
    mut dealt: MessageReader<Hit>,
    mut taken: MessageReader<PlayerHurt>,
) {
    let mut kick = |level: u8, taken: bool| {
        let amount = trauma_for(level, taken);
        if amount > 0.0 {
            info!(
                "shake: kick {amount:.2} (level {level}, {})",
                if taken { "taken" } else { "dealt" }
            );
            shake.kick(amount);
        }
    };
    for h in dealt.read() {
        kick(h.level, false);
    }
    for h in taken.read() {
        kick(if h.blocked { 0 } else { h.level }, true);
    }
}

fn sync_scale(settings: Res<crate::settings::Settings>, mut shake: ResMut<Shake>) {
    if shake.scale != settings.game.shake {
        shake.set_scale(settings.game.shake);
    }
}

pub fn plugin(app: &mut App) {
    let mut shake = Shake::default();
    shake.set_scale(crate::settings::initial().game.shake);
    app.insert_resource(shake)
        .add_systems(Update, (sync_scale, tick, kick_on_blows).chain());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_trauma_no_shake_and_full_trauma_is_bounded() {
        assert_eq!(shake_offset(0.0, 3.3), (Vec3::ZERO, Quat::IDENTITY));
        for i in 0..200 {
            let (shift, tilt) = shake_offset(1.0, i as f32 * 0.013);
            assert!(shift.length() <= MAX_SHIFT * 2.0_f32.sqrt() + 1.0e-6);
            assert!(tilt.angle_between(Quat::IDENTITY) < 0.04);
        }
        let (a, _) = shake_offset(0.5, 1.0);
        let (b, _) = shake_offset(1.0, 1.0);
        assert!(a.length() < b.length(), "less trauma, less shake");
    }

    #[test]
    fn the_players_setting_scales_a_kick_and_zero_turns_it_off() {
        let mut s = Shake::default();
        s.set_scale(0.5);
        s.kick(0.6);
        assert!((s.trauma - 0.3).abs() < 1.0e-6, "{}", s.trauma);
        let mut off = Shake::default();
        off.set_scale(0.0);
        off.kick(1.0);
        assert_eq!(off.trauma, 0.0);
        off.set_scale(9.0);
        off.kick(0.25);
        assert_eq!(off.trauma, 0.25, "a setting is capped at full");
    }

    #[test]
    fn a_kick_decays_to_nothing_in_the_time_it_says() {
        let mut s = Shake::default();
        s.kick(1.0);
        s.kick(1.0);
        assert_eq!(s.trauma, 1.0, "clamped");
        for _ in 0..23 {
            s.tick(0.0167);
        }
        assert!(s.trauma > 0.0, "0.4 s is not over yet");
        for _ in 0..3 {
            s.tick(0.0167);
        }
        assert_eq!(s.trauma, 0.0);
    }

    #[test]
    fn heavier_blows_shake_more_and_the_light_ones_dealt_do_not() {
        assert_eq!(trauma_for(1, false), 0.0);
        assert!(trauma_for(2, false) > 0.0);
        assert!(trauma_for(5, false) > trauma_for(2, false));
        assert!(trauma_for(1, true) < trauma_for(2, true));
        assert!(trauma_for(2, true) < trauma_for(5, true));
    }
}
