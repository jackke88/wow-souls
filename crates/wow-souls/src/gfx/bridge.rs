//! [`VideoConfig`] -> the benilla-world resources the renderer reads (layer 1 of the graphics
//! update: the engine already draws everything; these resources say how much).
//!
//! Ported from the fork's `dynamic_interior.rs`, `monkey_gfx.rs`, `sky_quality.rs` and
//! `zone_skybox.rs`, whose cvar observers become one guarded system: every write lands only on a
//! genuine change, so the resources' change ticks stay quiet on idle frames. See
//! `docs/PROVENANCE.md`.

use benilla_assets::WaterQuality;
use benilla_world::clutter::ClutterConfig;
use benilla_world::ffx_glow::SkyDither;
use benilla_world::lighting::{
    DynamicInteriors, EmissiveTier, FireLightGain, FogModelSetting, LavaLightGain, MoonLight,
    MoonShadowStrength, SpellLightGain,
};
use benilla_world::sky_fx::SkyQuality;
use benilla_world::skybox::ZoneSkyboxes;
use benilla_world::view::ViewDistance;
use benilla_world::weather::RainSurfaces;
use benilla_world::wind::{FoliageWind, FoliageWindStrength};
use bevy::prelude::*;

use super::config::VideoConfig;

pub fn plugin(app: &mut App) {
    // `SkyQuality` is the one resource the engine leaves for its client to create.
    app.init_resource::<SkyQuality>().add_systems(
        Update,
        (
            lighting_bridge,
            // The tier lands before the sky and cloud readers.
            sky_bridge.before(benilla_world::lighting::LightingConsumeSet),
            world_bridge,
        ),
    );
}

/// The lighting resources: light gains, the interior lane, water, bloom's HDR tier.
#[allow(clippy::too_many_arguments)]
fn lighting_bridge(
    video: Res<VideoConfig>,
    mut interiors: ResMut<DynamicInteriors>,
    mut fire: ResMut<FireLightGain>,
    mut spell: ResMut<SpellLightGain>,
    mut moon_shadow: ResMut<MoonShadowStrength>,
    mut moon_light: ResMut<MoonLight>,
    mut water: ResMut<WaterQuality>,
    mut lava: ResMut<LavaLightGain>,
    mut emissive: ResMut<EmissiveTier>,
) {
    set(&mut moon_light.0, video.moon_light);
    set(&mut water.0, video.water_quality);
    set(&mut lava.0, video.lava_light_gain);
    set(&mut emissive.0, video.bloom);
    set(&mut fire.0, video.fire_light_gain);
    set(&mut spell.0, video.spell_light_gain);
    set(&mut moon_shadow.0, video.moon_shadow_strength);
    // Read when a placement's seeds are built, so it applies to WMOs loaded afterwards.
    benilla_world::lighting::set_window_split(video.daylight_window_split);
    let want = DynamicInteriors {
        enabled: video.interior_light,
        ambient: video.interior_ambient,
        fill: video.interior_fill,
        exposure: video.interior_exposure,
        atten_scale: video.interior_atten_scale,
        room_gate: video.interior_room_gate,
        debug: video.interior_debug,
        flicker: video.fire_flicker,
        night_gain: video.night_gain,
        interior_gain: video.interior_gain,
        daylight: video.interior_daylight,
        bake_floor: video.interior_bake_floor,
    };
    if *interiors != want {
        *interiors = want;
    }
}

/// The sky tier.
fn sky_bridge(video: Res<VideoConfig>, mut out: ResMut<SkyQuality>) {
    let want = SkyQuality(video.sky_quality.min(SkyQuality::MAX));
    if *out != want {
        *out = want;
    }
}

/// Sky dither, fog model, rain, zone skies, wind and the view distance.
#[allow(clippy::too_many_arguments)]
fn world_bridge(
    video: Res<VideoConfig>,
    mut dither: ResMut<SkyDither>,
    mut fog: ResMut<FogModelSetting>,
    mut rain: ResMut<RainSurfaces>,
    mut zone: ResMut<ZoneSkyboxes>,
    mut wind: ResMut<FoliageWind>,
    mut strength: ResMut<FoliageWindStrength>,
    mut view: ResMut<ViewDistance>,
    mut clutter: ResMut<ClutterConfig>,
) {
    set(&mut dither.0, video.sky_dither);
    fog.set_from_cvar(if video.modern_fog { 1.0 } else { 0.0 });
    set(&mut rain.0, video.rain_surfaces);
    set(&mut zone.0, video.zone_skyboxes);
    set(&mut wind.0, video.foliage_wind.min(2));
    let want = FoliageWindStrength::from_cvar(video.foliage_wind_strength);
    if *strength != want {
        *strength = want;
    }
    set(&mut view.farclip, video.farclip);
    // Ground cover: a density write re-scatters the loaded tiles, so only a real change lands.
    set(&mut clutter.density, video.grass_density);
    set(&mut clutter.fade_far, video.grass_fade);
    set(&mut clutter.height, video.grass_height);
}

/// Assign only on a difference, so change detection stays honest.
fn set<T: PartialEq>(slot: &mut T, value: T) {
    if *slot != value {
        *slot = value;
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::Rung;
    use super::*;
    use benilla_world::lighting::FogModel;

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(VideoConfig::rung(Rung::High))
            .init_resource::<DynamicInteriors>()
            .init_resource::<FireLightGain>()
            .init_resource::<SpellLightGain>()
            .init_resource::<MoonShadowStrength>()
            .init_resource::<MoonLight>()
            .init_resource::<WaterQuality>()
            .init_resource::<LavaLightGain>()
            .init_resource::<EmissiveTier>()
            .init_resource::<SkyDither>()
            .init_resource::<FogModelSetting>()
            .init_resource::<RainSurfaces>()
            .init_resource::<ZoneSkyboxes>()
            .insert_resource(FoliageWind(0))
            .init_resource::<FoliageWindStrength>()
            .insert_resource(ViewDistance {
                farclip: 350.0,
                nearclip: 0.1,
            })
            .insert_resource(ClutterConfig::default())
            .add_plugins(plugin);
        app
    }

    #[test]
    fn the_high_preset_reaches_the_engine_resources() {
        let mut app = app();
        app.update();
        let w = app.world();
        assert_eq!(w.resource::<WaterQuality>().0, 2);
        assert_eq!(w.resource::<EmissiveTier>().0, 2);
        assert_eq!(w.resource::<SkyQuality>().0, 2);
        assert_eq!(w.resource::<FoliageWind>().0, 2);
        assert_eq!(w.resource::<ViewDistance>().farclip, 777.0);
        assert!(w.resource::<SkyDither>().0);
        assert!(w.resource::<RainSurfaces>().0);
        assert!(w.resource::<ZoneSkyboxes>().0);
        assert_eq!(w.resource::<MoonLight>().0, 2.0);
        assert!(w.resource::<DynamicInteriors>().enabled);
        assert_ne!(w.resource::<FogModelSetting>().0, FogModel::Classic);
    }

    #[test]
    fn a_rung_change_lands_and_idle_frames_stay_quiet() {
        let mut app = app();
        app.update();
        *app.world_mut().resource_mut::<VideoConfig>() = VideoConfig::rung(Rung::Classic);
        app.update();
        let w = app.world();
        assert_eq!(w.resource::<WaterQuality>().0, 0);
        assert_eq!(w.resource::<SkyQuality>().0, 0);
        assert_eq!(w.resource::<FoliageWind>().0, 0);
        assert_eq!(w.resource::<ViewDistance>().farclip, 350.0);
        assert!(!w.resource::<DynamicInteriors>().enabled);
        app.world_mut().clear_trackers();
        app.update();
        assert!(!app.world().resource_ref::<WaterQuality>().is_changed());
        assert!(!app.world().resource_ref::<DynamicInteriors>().is_changed());
        assert!(!app.world().resource_ref::<ViewDistance>().is_changed());
    }
}
