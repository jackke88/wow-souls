// Ported from pkuzic/benilla-everwood_graphics (MIT OR Apache-2.0), crates/benilla-app/src/post/mod.rs @ f545081e; see docs/PROVENANCE.md.
//! MONKEY (post): optional world-camera effects ahead of FFXGlow's gamma clamp.

mod bloom;
// MONKEY (visualfix): crate-visible for `rendered_minute` (volumetric fog).
pub mod grading;
mod sun_shafts;

pub struct PostPlugin;

impl bevy::prelude::Plugin for PostPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins(bloom::BloomPlugin);
        app.add_plugins(grading::GradingPlugin);
        app.add_plugins(sun_shafts::SunShaftsPlugin);
    }
}
