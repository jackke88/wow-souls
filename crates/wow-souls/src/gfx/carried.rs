//! The two markers the torch shadow lane reads about LIGHTS CARRIED BY A BODY (a held torch).
//!
//! In the fork these live in `benilla-app/src/entities/carried_light.rs`, with the systems that
//! spawn a point light for every casting M2 light of an entity's model. This game spawns no
//! carried lights (nothing in Northshire holds a lit torch), so the lane's queries find nothing and
//! the types are only here to keep its candidacy code as ported. See `docs/PROVENANCE.md`.

use bevy::prelude::*;

/// A light carried by a body, not a placed fixture: it never earns an EXTERIOR shadow slot.
#[derive(Component, Clone, Copy)]
pub struct HeldLight;

/// How long a carried light has held still; a settled one may be trusted with a cached cube map.
#[derive(Component)]
pub struct CarriedLightMotion {
    still_for: f32,
}

/// Seconds a carried light must hold still before it counts as settled.
const STILL_HOLD: f32 = 0.5;

impl CarriedLightMotion {
    /// Has this light held still long enough to be trusted with a cached cube shadow?
    pub fn settled(&self) -> bool {
        self.still_for >= STILL_HOLD
    }
}
