//! The graphics settings: [`VideoConfig`], one resource every lane of [`super`] reads.
//!
//! Ported from `benilla-app/src/video.rs` of the `pkuzic/benilla-everwood_graphics` fork (MIT OR
//! Apache-2.0; see `docs/PROVENANCE.md`). The fork keeps these as cvars behind an options screen;
//! this game has neither, so the struct is the setting. [`VideoConfig::default`] is the fork's
//! *registered* defaults (what its cvars hold before any preset: the opt-in lanes off), which the
//! ported lane tests assume; the game runs [`Rung::High`], the fork's shipped default
//! (`GRAPHICS_PRESETS` column High over `LIGHTING_PRESETS` High). The window and vsync fields are
//! not ours and are left out.
//!
//! Switches (read once at startup, see [`VideoConfig::from_env`]):
//! - `WOWSOULS_GFX=classic|low|medium|high|ultra` picks the rung (default `high`); `classic` is the
//!   reference client's look with every lane off.
//! - `WOWSOULS_SHADOWS=0`, `WOWSOULS_AO=0|1|2`, `WOWSOULS_BLOOM=0|1|2`, `WOWSOULS_VOLFOG=0|1|2`,
//!   `WOWSOULS_VOLLIGHT=0|1|2`, `WOWSOULS_SKY=0|1|2`, `WOWSOULS_WIND=0|1|2`, `WOWSOULS_FARCLIP=<yd>`,
//!   `WOWSOULS_GRADING=0|1`, `WOWSOULS_SHAFTS=0|1`, `WOWSOULS_ZONESKY=0|1`, `WOWSOULS_NIGHT_GAIN=0.2..1.5`,
//!   `WOWSOULS_MOONLIGHT=0..2` override one lane.

use bevy::prelude::*;

use super::shadow_core::{
    DEFAULT_SHADOW_DISTANCE, DEFAULT_SHADOW_FILTER, DEFAULT_SHADOW_MAP_SIZE, DEFAULT_SHADOW_RATE,
};

/// The graphics knobs. Field meanings are documented where they are used (the lane files, and the
/// fork's `LIGHTING.md`); the ranges are the fork's slider ranges.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct VideoConfig {
    /// Water tier: 0 Classic, 1 Enhanced, 2 High with planar mirror reflections.
    pub water_quality: u8,
    /// Near-field haze and shafts: 0 Off, 1 Low, 2 High.
    pub volumetric_fog: u8,
    /// HDR bloom: 0 Off, 1 Low (quarter-resolution halo), 2 High (half-resolution).
    pub bloom: u8,
    /// Depth-occluded radial sun shafts.
    pub sun_shafts: bool,
    /// Zone / day-night LUT grading.
    pub color_grading: bool,
    /// Sky tier: 0 Classic, 1 Enhanced, 2 High (detailed sun-lit clouds).
    pub sky_quality: u8,
    /// Screen-space contact shadows: 0 Off, 1 Low, 2 High.
    pub ambient_occlusion: u8,
    /// Point-light fog: 0 Off, 1 nearest 16, 2 nearest 32.
    pub lamp_fog: u8,
    /// Shadow-mapped sun / moon shafts: 0 Off, 1 Medium, 2 High.
    pub volumetric_light: u8,
    /// The shafts' brightness multiplier, 0..2.
    pub volumetric_light_strength: f32,
    /// The additive moon term's multiplier, 0..2 (0 = the stock night exactly).
    pub moon_light: f32,
    /// Brightness of lava lighting its surroundings, 0..4.
    pub lava_light_gain: f32,
    /// The static world (trees, buildings, foliage) casts realtime shadows.
    pub world_shadows: bool,
    /// Characters cast realtime silhouettes instead of the oval blob.
    pub character_shadows: bool,
    /// Realtime-shadow render distance in yards.
    pub shadow_distance: f32,
    /// The sun shadow map's edge in texels (1024 / 2048 / 4096).
    pub shadow_map_size: u32,
    /// The receivers' PCF kernel: 0 hardware 2x2, 1 Gaussian.
    pub shadow_filter: u32,
    /// Hz cap on the character lane's proxy re-skin (0 = every frame).
    pub character_shadow_rate: u32,
    /// Hz cap on the world lane's entity caster (0 = every frame).
    pub world_shadow_rate: u32,
    /// Multiplier on the caster-collection reach, 0.25..2.
    pub shadow_caster_reach: f32,
    /// How dark a moon-shadowed fragment gets at night, 0..1.
    pub moon_shadow_strength: f32,
    /// WMO interiors light from the room's own fixtures.
    pub interior_light: bool,
    pub interior_ambient: f32,
    pub interior_fill: f32,
    pub interior_exposure: f32,
    pub interior_atten_scale: f32,
    /// Interior fixtures cast cube-map shadows.
    pub interior_shadows: bool,
    /// Exterior fires (campfires, braziers, lampposts) cast cube-map shadows at night.
    pub exterior_shadows: bool,
    /// The ground casts into an exterior torch's cube map.
    pub torch_terrain_shadows: bool,
    /// Resident torch shadow maps (1..16).
    pub interior_shadow_casters: u32,
    /// Torch maps with per-frame moving-caster overlays (0..16).
    pub interior_shadow_dynamic: u32,
    /// Hz of the moving-caster regather (0 = every frame).
    pub interior_shadow_entity_rate: u32,
    /// Torch shadow PCF contact radius, 0.5..3.
    pub interior_shadow_soft: f32,
    /// How much of the direct term a torch shadow removes, 0..1.
    pub torch_shadow_strength: f32,
    pub interior_room_gate: bool,
    pub interior_debug: u32,
    /// The exterior night dim, 0.2..1.5 (0.45 = nights 55 % darker).
    pub night_gain: f32,
    /// The interior dim, 0.2..1.5.
    pub interior_gain: f32,
    pub interior_daylight: f32,
    pub daylight_window_split: bool,
    pub interior_bake_floor: f32,
    /// Gain on lights synthesised from fire props, 0..4.
    pub fire_light_gain: f32,
    /// Gain on lights spell effects invent, 0..4.
    pub spell_light_gain: f32,
    /// Flame brightness wobble, 0..2.
    pub fire_flicker: f32,
    // The bridges that, in the fork, are cvar observers on benilla-world resources.
    /// A faint dither so smooth sky and fog gradients do not band.
    pub sky_dither: bool,
    /// Modern radial fog with a sun lobe (the fork's `fogModel` 1).
    pub modern_fog: bool,
    /// Rain darkens surfaces, puddles and rings.
    pub rain_surfaces: bool,
    /// The painted zone skies of `LightSkybox.dbc`.
    pub zone_skyboxes: bool,
    /// Foliage wind: 0 Off, 1 Grass, 2 Grass + trees.
    pub foliage_wind: u8,
    /// The sway gain of the wind, 0.25..3.
    pub foliage_wind_strength: f32,
    /// Render distance in yards (`farclip`): 350 Classic / Low, 477 Medium, 777 High, 1497 Ultra.
    pub farclip: f32,
    /// Ground cover density, a multiple of the reference's 16 cells per chunk (`frillDensity`):
    /// 2 is the engine's default (frill 32, the 1.12 panel's Medium), 16 is the engine's ceiling
    /// (frill 256).
    pub grass_density: f32,
    /// How far ground cover is drawn, in yards (the reference's 70).
    pub grass_fade: f32,
    /// A multiple of every tuft's height (the fork's `ClutterConfig::height`, a wow-souls patch).
    pub grass_height: f32,
}

impl Default for VideoConfig {
    /// The fork's registered defaults: every opt-in lane (AO, lamp fog, volumetric light,
    /// moonlight, zone skies, the modern sky and fog) off. See [`Rung::High`] for the shipped look.
    fn default() -> Self {
        Self {
            water_quality: 2,
            volumetric_fog: 1,
            bloom: 2,
            sun_shafts: true,
            color_grading: true,
            sky_quality: 0,
            ambient_occlusion: 0,
            lamp_fog: 0,
            volumetric_light: 0,
            volumetric_light_strength: 1.0,
            moon_light: 0.0,
            lava_light_gain: 1.0,
            world_shadows: true,
            character_shadows: true,
            shadow_distance: DEFAULT_SHADOW_DISTANCE,
            shadow_map_size: DEFAULT_SHADOW_MAP_SIZE,
            shadow_filter: DEFAULT_SHADOW_FILTER,
            character_shadow_rate: DEFAULT_SHADOW_RATE,
            world_shadow_rate: DEFAULT_SHADOW_RATE,
            shadow_caster_reach: 1.0,
            moon_shadow_strength: 0.35,
            interior_light: true,
            interior_ambient: 0.015,
            interior_fill: 0.08,
            interior_exposure: 2.5,
            interior_atten_scale: 1.6,
            interior_shadows: true,
            exterior_shadows: true,
            torch_terrain_shadows: false,
            interior_shadow_casters: 12,
            interior_shadow_dynamic: 4,
            interior_shadow_entity_rate: 30,
            interior_shadow_soft: 1.5,
            torch_shadow_strength: 0.7,
            interior_room_gate: true,
            interior_debug: 0,
            night_gain: 0.45,
            interior_gain: 0.5,
            interior_daylight: 0.0,
            daylight_window_split: true,
            interior_bake_floor: 0.12,
            fire_light_gain: 1.0,
            spell_light_gain: 1.0,
            fire_flicker: 1.0,
            sky_dither: false,
            modern_fog: false,
            rain_surfaces: true,
            zone_skyboxes: false,
            foliage_wind: 2,
            foliage_wind_strength: 1.0,
            farclip: 350.0,
            grass_density: 2.0,
            grass_fade: 70.0,
            grass_height: 1.0,
        }
    }
}

/// The fork's presets. `Classic` is every lane off; the middle rungs keep the cheap lanes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rung {
    Classic,
    Low,
    Medium,
    High,
    Ultra,
}

impl Rung {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "classic" | "off" | "0" => Some(Self::Classic),
            "low" | "1" => Some(Self::Low),
            "medium" | "med" | "2" => Some(Self::Medium),
            "high" | "3" => Some(Self::High),
            "ultra" | "4" => Some(Self::Ultra),
            _ => None,
        }
    }
}

impl VideoConfig {
    /// One rung of the fork's Graphics Preset (`GRAPHICS_PRESETS` over `LIGHTING_PRESETS`).
    pub fn rung(rung: Rung) -> Self {
        // The Graphics Preset's High column over the registered defaults.
        let high = Self {
            farclip: 777.0,
            sky_quality: 2,
            sky_dither: true,
            modern_fog: true,
            torch_terrain_shadows: true,
            ambient_occlusion: 2,
            zone_skyboxes: true,
            lamp_fog: 2,
            volumetric_light: 2,
            // Nights you can play in: the fork's 0.45 / 1 leaves the valley near black once the
            // moon is up; this is dark and moonlit, readable from the lit path to the grass.
            moon_light: 2.0,
            night_gain: 0.85,
            // Elden Ring / Sekiro-level meadows: the engine's densest ground cover, taller tufts,
            // and a longer reach; the shader presses them down around every unit so monsters stay
            // readable (the wow-souls patch of the fork).
            grass_density: 16.0,
            grass_fade: 100.0,
            grass_height: 1.3,
            ..Self::default()
        };
        match rung {
            Rung::High => high,
            Rung::Ultra => Self {
                farclip: 1497.0,
                grass_fade: 130.0,
                shadow_map_size: 4096,
                interior_shadow_casters: 16,
                interior_shadow_dynamic: 8,
                volumetric_fog: 2,
                ..high
            },
            Rung::Medium => Self {
                farclip: 477.0,
                moon_light: 1.0,
                night_gain: 0.45,
                grass_density: 6.0,
                grass_fade: 80.0,
                grass_height: 1.1,
                character_shadows: true,
                world_shadows: true,
                shadow_map_size: 2048,
                interior_shadow_casters: 6,
                interior_shadow_dynamic: 2,
                exterior_shadows: false,
                torch_terrain_shadows: false,
                water_quality: 1,
                volumetric_fog: 1,
                bloom: 1,
                sun_shafts: false,
                color_grading: false,
                sky_quality: 1,
                ambient_occlusion: 1,
                lamp_fog: 1,
                volumetric_light: 1,
                zone_skyboxes: true,
                foliage_wind: 2,
                ..high
            },
            Rung::Low => Self {
                farclip: 350.0,
                moon_light: 1.0,
                night_gain: 0.45,
                grass_density: 2.0,
                grass_fade: 70.0,
                grass_height: 1.0,
                character_shadows: true,
                world_shadows: false,
                shadow_map_size: 1024,
                interior_shadows: false,
                exterior_shadows: false,
                torch_terrain_shadows: false,
                water_quality: 1,
                volumetric_fog: 1,
                bloom: 1,
                sun_shafts: false,
                color_grading: false,
                sky_quality: 1,
                ambient_occlusion: 0,
                lamp_fog: 0,
                volumetric_light: 0,
                moon_shadow_strength: 0.0,
                zone_skyboxes: false,
                foliage_wind: 1,
                ..high
            },
            Rung::Classic => Self {
                farclip: 350.0,
                grass_density: 2.0,
                grass_fade: 70.0,
                grass_height: 1.0,
                character_shadows: false,
                world_shadows: false,
                interior_light: false,
                interior_shadows: false,
                exterior_shadows: false,
                torch_terrain_shadows: false,
                spell_light_gain: 0.0,
                water_quality: 0,
                volumetric_fog: 0,
                bloom: 0,
                sun_shafts: false,
                color_grading: false,
                sky_quality: 0,
                ambient_occlusion: 0,
                lamp_fog: 0,
                volumetric_light: 0,
                moon_light: 0.0,
                lava_light_gain: 0.0,
                fire_light_gain: 0.0,
                night_gain: 1.0,
                interior_gain: 1.0,
                moon_shadow_strength: 0.0,
                fire_flicker: 0.0,
                daylight_window_split: false,
                sky_dither: false,
                modern_fog: false,
                rain_surfaces: false,
                zone_skyboxes: false,
                foliage_wind: 0,
                ..high
            },
        }
    }

    /// The rung `WOWSOULS_GFX` names (default High) with the per-lane overrides applied, over any
    /// variable source (the game starts through [`Self::startup`]; this is the rule the tests pin).
    #[cfg(test)]
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        let rung = var("WOWSOULS_GFX")
            .and_then(|v| Rung::parse(&v))
            .unwrap_or(Rung::High);
        Self::with_lane_overrides(Self::rung(rung), var)
    }

    /// What the game starts with: the player's settings (their preset and lanes, or the rung
    /// `WOWSOULS_GFX` named instead, which [`crate::settings::Settings::with_env`] already applied),
    /// then the instruments' per-lane switches over that.
    pub fn startup(
        prefs: &crate::settings::VideoPrefs,
        var: impl Fn(&str) -> Option<String>,
    ) -> Self {
        Self::with_lane_overrides(prefs.config(), var)
    }

    /// The per-lane environment switches over a base config.
    pub fn with_lane_overrides(base: Self, var: impl Fn(&str) -> Option<String>) -> Self {
        let mut cfg = base;
        let tier = |name: &str, max: u8| {
            var(name)
                .and_then(|v| v.trim().parse::<u8>().ok())
                .map(|v| v.min(max))
        };
        if var("WOWSOULS_SHADOWS").is_some_and(|v| v.trim() == "0") {
            cfg.world_shadows = false;
            cfg.character_shadows = false;
            cfg.interior_shadows = false;
            cfg.exterior_shadows = false;
        }
        let flag = |name: &str| var(name).map(|v| v.trim() != "0");
        if let Some(v) = flag("WOWSOULS_GRADING") {
            cfg.color_grading = v;
        }
        if let Some(v) = flag("WOWSOULS_SHAFTS") {
            cfg.sun_shafts = v;
        }
        if let Some(v) = flag("WOWSOULS_ZONESKY") {
            cfg.zone_skyboxes = v;
        }
        if let Some(v) = tier("WOWSOULS_AO", 2) {
            cfg.ambient_occlusion = v;
        }
        if let Some(v) = tier("WOWSOULS_BLOOM", 2) {
            cfg.bloom = v;
        }
        if let Some(v) = tier("WOWSOULS_VOLFOG", 2) {
            cfg.volumetric_fog = v;
        }
        if let Some(v) = tier("WOWSOULS_VOLLIGHT", 2) {
            cfg.volumetric_light = v;
        }
        if let Some(v) = tier("WOWSOULS_SKY", 2) {
            cfg.sky_quality = v;
        }
        if let Some(v) = tier("WOWSOULS_WIND", 2) {
            cfg.foliage_wind = v;
        }
        if let Some(v) = var("WOWSOULS_FARCLIP").and_then(|v| v.trim().parse::<f32>().ok()) {
            cfg.farclip = v.clamp(177.0, 1497.0);
        }
        if let Some(v) = var("WOWSOULS_GRASS").and_then(|v| v.trim().parse::<f32>().ok()) {
            cfg.grass_density = v.clamp(0.0, 16.0);
        }
        if let Some(v) = var("WOWSOULS_GRASS_FADE").and_then(|v| v.trim().parse::<f32>().ok()) {
            cfg.grass_fade = v.clamp(10.0, 300.0);
        }
        if let Some(v) = var("WOWSOULS_GRASS_HEIGHT").and_then(|v| v.trim().parse::<f32>().ok()) {
            cfg.grass_height = v.clamp(0.1, 4.0);
        }
        let level = |name: &str, lo: f32, hi: f32| {
            var(name)
                .and_then(|v| v.trim().parse::<f32>().ok())
                .map(|v| v.clamp(lo, hi))
        };
        if let Some(v) = level("WOWSOULS_NIGHT_GAIN", 0.2, 1.5) {
            cfg.night_gain = v;
        }
        if let Some(v) = level("WOWSOULS_MOONLIGHT", 0.0, 2.0) {
            cfg.moon_light = v;
        }
        if let Some(v) = var("WOWSOULS_WIND_STRENGTH").and_then(|v| v.trim().parse::<f32>().ok()) {
            cfg.foliage_wind_strength = v.clamp(0.25, 3.0);
        }
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn the_default_is_the_forks_registered_defaults_and_high_is_the_shipped_column() {
        let r = VideoConfig::default();
        assert_eq!(
            (
                r.ambient_occlusion,
                r.lamp_fog,
                r.volumetric_light,
                r.sky_quality
            ),
            (0, 0, 0, 0),
            "the opt-in lanes are off until a preset turns them on"
        );
        assert!(r.world_shadows && r.character_shadows && r.interior_light);
        assert_eq!(r.farclip, 350.0);
        let c = VideoConfig::rung(Rung::High);
        assert!(c.world_shadows && c.character_shadows);
        assert_eq!(c.shadow_map_size, 2048);
        assert_eq!((c.water_quality, c.bloom, c.sky_quality), (2, 2, 2));
        assert_eq!(
            (c.ambient_occlusion, c.lamp_fog, c.volumetric_light),
            (2, 2, 2)
        );
        assert!(c.sun_shafts && c.color_grading && c.zone_skyboxes && c.modern_fog);
        assert_eq!(c.foliage_wind, 2);
        assert_eq!(c.farclip, 777.0);
        assert_eq!(
            VideoConfig::from_vars(env(&[])),
            c,
            "High is what the game runs"
        );
    }

    #[test]
    fn classic_turns_every_lane_off() {
        let c = VideoConfig::rung(Rung::Classic);
        assert!(!c.world_shadows && !c.character_shadows && !c.interior_shadows);
        assert_eq!(
            (c.bloom, c.ambient_occlusion, c.volumetric_light),
            (0, 0, 0)
        );
        assert_eq!((c.sky_quality, c.foliage_wind, c.water_quality), (0, 0, 0));
        assert!(!c.sun_shafts && !c.color_grading && !c.modern_fog && !c.zone_skyboxes);
        assert_eq!(c.night_gain, 1.0, "classic keeps the reference night");
    }

    #[test]
    fn the_rungs_climb() {
        let low = VideoConfig::rung(Rung::Low);
        let medium = VideoConfig::rung(Rung::Medium);
        let ultra = VideoConfig::rung(Rung::Ultra);
        assert!(!low.world_shadows && medium.world_shadows);
        assert_eq!(low.shadow_map_size, 1024);
        assert_eq!(ultra.shadow_map_size, 4096);
        assert!(ultra.interior_shadow_casters > medium.interior_shadow_casters);
        assert!(low.grass_density < medium.grass_density);
        assert!(medium.grass_density < ultra.grass_density);
        let classic = VideoConfig::rung(Rung::Classic);
        assert_eq!(
            (classic.grass_density, classic.grass_height),
            (2.0, 1.0),
            "classic keeps the reference's meadow"
        );
    }

    #[test]
    fn the_environment_picks_a_rung_and_overrides_lanes() {
        let c = VideoConfig::from_vars(env(&[("WOWSOULS_GFX", "Low"), ("WOWSOULS_AO", "2")]));
        assert_eq!(c.shadow_map_size, 1024);
        assert_eq!(c.ambient_occlusion, 2);
        let c = VideoConfig::from_vars(env(&[("WOWSOULS_SHADOWS", "0")]));
        assert!(!c.world_shadows && !c.character_shadows && !c.exterior_shadows);
        assert_eq!(c.bloom, 2, "only the shadow lanes go");
        let c = VideoConfig::from_vars(env(&[("WOWSOULS_BLOOM", "9"), ("WOWSOULS_GFX", "junk")]));
        assert_eq!(
            c.bloom, 2,
            "a tier clamps; an unknown rung falls back to High"
        );
        let c = VideoConfig::from_vars(env(&[("WOWSOULS_WIND_STRENGTH", "50")]));
        assert_eq!(c.foliage_wind_strength, 3.0);
        let c = VideoConfig::from_vars(env(&[
            ("WOWSOULS_GRASS", "99"),
            ("WOWSOULS_GRASS_FADE", "5"),
            ("WOWSOULS_GRASS_HEIGHT", "0.7"),
        ]));
        assert_eq!(
            (c.grass_density, c.grass_fade, c.grass_height),
            (16.0, 10.0, 0.7),
            "density and fade clamp to the engine's range"
        );
    }
}
