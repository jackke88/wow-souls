//! A boss move's ground shock and blast (spec `2026-10-16-hogger-rework-design.md` §6.1, §5.3): a hit on the
//! ground where the blow lands, or all round the creature, that hurts a player standing in it even when the
//! blade misses. Pure rules first (tested); [`crate::npc::strike`] fires them once per swing at the clip's moment
//! (the kit's `attack-shock` / `attack-blast` lines, [`crate::monster::Shock`]) and announces each with
//! [`Landed`], hit or miss, for the dust, the thud and the shake.

use bevy::prelude::*;

/// The bits of a swing's `Brain.struck` that say its shock and its blast have fired (the low bits are its hit
/// windows; a swing has far fewer than 29).
pub const SHOCK_BIT: u32 = 1 << 30;
pub const BLAST_BIT: u32 = 1 << 29;

/// How far above or below the shock's centre the player's feet may be and still be hit (yd): a player on a
/// ledge above a slam, or in the air over it, is not.
pub const SHOCK_HEIGHT: f32 = 1.0;
/// How much more height difference each yard of distance allows (yd per yd): a player up or down a hillside
/// from the slam stands on the same ground, a player beside it but high above is in the air.
pub const SHOCK_SLOPE: f32 = 0.4;

/// A shock or blast hit the ground (whether or not it hit the player).
#[derive(Message, Debug, Clone, Copy)]
pub struct Landed {
    pub from: Entity,
    pub at: Vec3,
    /// Its radius (yd).
    pub radius: f32,
    /// A blast all round the creature, not a slam's shock.
    pub blast: bool,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Landed>();
}

/// Where a shock lands: under the blade's tip, at the ground's height.
pub fn centre(tip: Vec3, ground_y: f32) -> Vec3 {
    Vec3::new(tip.x, ground_y, tip.z)
}

/// A shock's radius in yards: DS metres x the kit's scale x the creature's drawn scale, as a blow's volume.
pub fn radius_yd(radius_m: f32, kit_scale: f32, scale: f32) -> f32 {
    radius_m * kit_scale * scale
}

/// Whether the shock fires this frame: its moment has come and it has not fired yet.
pub fn due(at: f32, t: f32, fired: bool) -> bool {
    !fired && t >= at
}

/// Whether a shock of `radius` at `centre` hits a player whose feet are at `feet` (body radius `body`): within
/// reach on the ground, level with it, and not in a roll's invulnerability.
pub fn hits(centre: Vec3, radius: f32, feet: Vec3, body: f32, intangible: bool) -> bool {
    let flat = Vec2::new(feet.x - centre.x, feet.z - centre.z).length();
    !intangible
        && flat <= radius + body
        && (feet.y - centre.y).abs() <= SHOCK_HEIGHT + SHOCK_SLOPE * flat
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shock_centre_sits_on_the_ground() {
        let c = centre(Vec3::new(2.0, 3.5, -1.0), 0.25);
        assert_eq!(c, Vec3::new(2.0, 0.25, -1.0));
    }

    #[test]
    fn a_shock_hits_inside_its_radius_and_not_outside() {
        let c = Vec3::ZERO;
        assert!(hits(c, 2.0, Vec3::new(2.2, 0.0, 0.0), 0.3, false));
        assert!(!hits(c, 2.0, Vec3::new(2.4, 0.0, 0.0), 0.3, false));
        assert!(hits(c, 2.0, Vec3::new(0.0, 0.0, -1.0), 0.3, false));
    }

    #[test]
    fn a_roll_in_its_iframes_passes_a_shock() {
        let feet = Vec3::new(0.5, 0.0, 0.0);
        assert!(!hits(Vec3::ZERO, 2.0, feet, 0.3, true));
        // The frames after the i-frames do not.
        assert!(hits(Vec3::ZERO, 2.0, feet, 0.3, false));
    }

    #[test]
    fn a_shock_reaches_a_player_level_with_it_but_not_one_far_above() {
        let c = Vec3::ZERO;
        assert!(hits(c, 2.0, Vec3::new(1.0, 0.8, 0.0), 0.3, false));
        assert!(!hits(c, 2.0, Vec3::new(1.0, 1.5, 0.0), 0.3, false));
    }

    #[test]
    fn a_shock_reaches_a_player_up_a_slope_but_not_one_in_the_air() {
        let c = Vec3::ZERO;
        // 2 yd off, 1.3 yd higher: a hillside of about 33 degrees.
        assert!(hits(c, 2.5, Vec3::new(2.0, 1.3, 0.0), 0.3, false));
        // Right beside it but 1.5 yd up: in the air over the slam.
        assert!(!hits(c, 2.5, Vec3::new(0.3, 1.5, 0.0), 0.3, false));
    }

    #[test]
    fn a_shock_fires_once_at_its_moment() {
        assert!(!due(2.47, 2.4, false));
        assert!(due(2.47, 2.5, false));
        assert!(!due(2.47, 2.6, true));
        assert_eq!(radius_yd(3.0, 0.5, 1.5), 2.25);
    }
}
