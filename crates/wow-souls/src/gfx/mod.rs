//! The graphics update: the renderer of `pkuzic/benilla-everwood_graphics` (a fork of benilla) in
//! this game. Two layers:
//!
//! 1. **The engine** (`benilla-world`, now the fork) already carries the shaders and resources for
//!    sky, fog, water, wind, rain, interior and fire light; [`bridge`] sets them from the
//!    [`config::VideoConfig`] (default: the fork's High preset).
//! 2. **The lane drivers** that live in the fork's `benilla-app`, which this game does not depend on,
//!    are ported into this module with attribution (`docs/PROVENANCE.md`).
//!
//! `WOWSOULS_GFX=classic|low|medium|high|ultra` picks the rung, see [`config`].

pub mod bridge;
pub mod carried;
pub mod character_shadow;
pub mod config;
pub mod perf;
pub mod post;
pub mod shadow_core;
pub mod ssao;
pub mod torch_shadow;
pub mod vantage;
pub mod volumetric_fog;
pub mod volumetric_light;
pub mod world_shadow;

use bevy::prelude::*;

use benilla_assets::{AssetSet, LockRecover, WorldAssets};
use benilla_formats::AreaTableCatalog;

pub use config::VideoConfig;

/// Does this build offer developer levers (the lanes' `WOW_*` capture overrides)? The game's own
/// `dev` feature, as the fork's `run_mode::dev_affordances`.
pub(crate) fn dev_affordances() -> bool {
    cfg!(feature = "dev")
}

/// The shared `AreaTable.dbc` catalog the colour grading reads to find a leaf area's zone.
#[derive(Resource)]
pub struct AreaTableRes(pub AreaTableCatalog);

fn load_area_table(mut commands: Commands, assets: Option<Res<WorldAssets>>) {
    let Some(assets) = assets else { return };
    let loaded = {
        let mut chain = assets.chain.lock_recover();
        benilla_formats::load_area_table_catalog(&mut chain)
    };
    match loaded {
        Ok(catalog) => {
            info!("gfx: {} rows in the AreaTable catalog", catalog.len());
            commands.insert_resource(AreaTableRes(catalog));
        }
        Err(e) => warn!("gfx: AreaTable catalog failed to load: {e:#}"),
    }
}

pub fn plugin(app: &mut App) {
    let config = VideoConfig::startup(&crate::settings::initial().video, |name| {
        std::env::var(name).ok()
    });
    info!(
        "gfx: world shadows {} / characters {} | map {} | AO {} | bloom {} | sky {} | wind {} | farclip {}",
        config.world_shadows,
        config.character_shadows,
        config.shadow_map_size,
        config.ambient_occlusion,
        config.bloom,
        config.sky_quality,
        config.foliage_wind,
        config.farclip,
    );
    // The lane shaders, embedded under `embedded://wow_souls/gfx/shaders/`: registered here, from
    // this file, because the macro bakes the calling file's directory into the served path.
    bevy::asset::embedded_asset!(app, "shaders/shadow_caster.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/shadow_caster_cutout_prepass.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/ssao.wgsl");
    // The fork's registration order (`game_plugins.rs`): the rig first, then the lanes.
    app.insert_resource(config)
        .add_systems(Startup, load_area_table.after(AssetSet::Open))
        // The water passes the fork's app adds beside the engine group: the scene-depth copy
        // (refraction, shore depth; the AO pass orders against its node) and the High tier's
        // planar mirror.
        .add_plugins(benilla_world::liquid::WaterDepthPlugin)
        .add_plugins(benilla_world::liquid::WaterMirrorPlugin)
        .add_plugins(bridge::plugin)
        .add_plugins(perf::plugin)
        .add_plugins(vantage::plugin)
        .add_plugins(shadow_core::ShadowCorePlugin)
        .add_plugins(character_shadow::CharacterShadowPlugin)
        .add_plugins(world_shadow::WorldShadowPlugin)
        .add_plugins(torch_shadow::TorchShadowPlugin)
        .add_plugins(volumetric_fog::VolumetricFogPlugin)
        // After the fog plugin, whose graph node it follows.
        .add_plugins(volumetric_light::VolumetricLightPlugin)
        // World-only HDR effects, before the FFX clamp.
        .add_plugins(post::PostPlugin)
        // Contact shadows after the opaque pass, before the water copy.
        .add_plugins(ssao::AmbientOcclusionPlugin);
}

#[cfg(test)]
mod tests {
    use bevy::asset::io::AssetSourceId;
    use bevy::prelude::*;

    /// Every shader file in `shaders/` answers at the embedded path the lanes load it from.
    #[test]
    fn every_lane_shader_answers_at_its_embedded_path() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        bevy::asset::embedded_asset!(app, "shaders/shadow_caster.wgsl");
        bevy::asset::embedded_asset!(app, "shaders/shadow_caster_cutout_prepass.wgsl");
        bevy::asset::embedded_asset!(app, "shaders/ssao.wgsl");
        let server = app.world().resource::<AssetServer>();
        let source = server.get_source(AssetSourceId::from("embedded")).unwrap();
        for name in [
            "shadow_caster.wgsl",
            "shadow_caster_cutout_prepass.wgsl",
            "ssao.wgsl",
        ] {
            let path = format!("wow_souls/gfx/shaders/{name}");
            let read = bevy::tasks::block_on(async {
                source.reader().read(std::path::Path::new(&path)).await
            });
            assert!(read.is_ok(), "embedded://{path} does not resolve");
        }
    }
}
