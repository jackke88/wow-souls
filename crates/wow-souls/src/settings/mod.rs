//! The player's settings: sound levels, the graphics preset and its lanes, the camera and the game's
//! few options, and the key bindings ([`binds`]). One text file, `settings-v1.txt` beside the other
//! saves (`%LOCALAPPDATA%\WoWSouls`, `WOWSOULS_SETTINGS=<file>` overrides), written the moment something
//! changes. The environment switches the instruments use still win over the file for that run
//! ([`Settings::with_env`], [`crate::gfx::VideoConfig::startup`]); an unattended run neither reads nor
//! writes the file unless `WOWSOULS_SETTINGS` names one.
//!
//! The format is `wssettings 1` then `key value` lines. A line that cannot be read is skipped and
//! reported ([`Loaded::problems`]); the rest still loads, so a hand-edited file never costs the player
//! their bindings.

pub mod binds;

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use crate::gfx::config::{Rung, VideoConfig};

pub use binds::{Binds, Controls, MenuUp};

/// The master volume when nothing says otherwise (tuned by ear in `audio.rs`).
pub const DEFAULT_MASTER: f32 = 0.6;

/// Sound levels, 0 to 1 (the music and ambience sit under the master; the effects and the quest voices
/// scale their own kind).
#[derive(Debug, Clone, PartialEq)]
pub struct SoundPrefs {
    pub master: f32,
    pub music: f32,
    pub ambience: f32,
    pub effects: f32,
    pub voices: f32,
    /// WoW's "Loop Music": no silence between tracks.
    pub loop_music: bool,
}

impl Default for SoundPrefs {
    fn default() -> Self {
        SoundPrefs {
            master: DEFAULT_MASTER,
            music: 1.0,
            ambience: 1.0,
            effects: 1.0,
            voices: 1.0,
            loop_music: true,
        }
    }
}

/// The graphics lanes a player can reach, and the window's two switches.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoPrefs {
    /// The preset the values below started from (the rest of the config comes from it).
    pub rung: Rung,
    /// 0 off, 1 characters only, 2 characters and the world.
    pub shadows: u8,
    pub ao: u8,
    pub bloom: u8,
    pub fog: u8,
    pub light: u8,
    pub sky: u8,
    pub wind: u8,
    pub water: u8,
    pub grading: bool,
    pub farclip: f32,
    pub grass: f32,
    pub fullscreen: bool,
    pub vsync: bool,
}

/// The ground-cover steps the Graphics page offers.
pub const GRASS_STEPS: [f32; 4] = [0.0, 2.0, 6.0, 16.0];

impl VideoPrefs {
    pub fn of_rung(rung: Rung) -> VideoPrefs {
        VideoPrefs::from_config(rung, &VideoConfig::rung(rung))
    }

    fn from_config(rung: Rung, c: &VideoConfig) -> VideoPrefs {
        VideoPrefs {
            rung,
            shadows: if c.world_shadows {
                2
            } else {
                u8::from(c.character_shadows)
            },
            ao: c.ambient_occlusion,
            bloom: c.bloom,
            fog: c.volumetric_fog,
            light: c.volumetric_light,
            sky: c.sky_quality,
            wind: c.foliage_wind,
            water: c.water_quality,
            grading: c.color_grading,
            farclip: c.farclip,
            grass: c.grass_density,
            fullscreen: false,
            vsync: true,
        }
    }

    /// Writes the exposed values over `cfg` (the rest of it is the preset's).
    pub fn apply_to(&self, cfg: &mut VideoConfig) {
        match self.shadows {
            0 => {
                cfg.character_shadows = false;
                cfg.world_shadows = false;
                cfg.interior_shadows = false;
                cfg.exterior_shadows = false;
            }
            1 => {
                cfg.character_shadows = true;
                cfg.world_shadows = false;
            }
            _ => {
                cfg.character_shadows = true;
                cfg.world_shadows = true;
            }
        }
        cfg.ambient_occlusion = self.ao.min(2);
        cfg.bloom = self.bloom.min(2);
        cfg.volumetric_fog = self.fog.min(2);
        cfg.volumetric_light = self.light.min(2);
        cfg.sky_quality = self.sky.min(2);
        cfg.foliage_wind = self.wind.min(2);
        cfg.water_quality = self.water.min(2);
        cfg.color_grading = self.grading;
        cfg.farclip = self.farclip.clamp(177.0, 1497.0);
        cfg.grass_density = self.grass.clamp(0.0, 16.0);
    }

    /// The whole config these prefs mean.
    pub fn config(&self) -> VideoConfig {
        let mut cfg = VideoConfig::rung(self.rung);
        self.apply_to(&mut cfg);
        cfg
    }

    /// Whether the values differ from the preset's own (the page then says "Custom").
    pub fn is_custom(&self) -> bool {
        let base = VideoPrefs::of_rung(self.rung);
        VideoPrefs {
            fullscreen: self.fullscreen,
            vsync: self.vsync,
            ..base
        } != *self
    }

    /// Chooses a preset: every lane takes the preset's value (the window switches stay).
    pub fn choose(&mut self, rung: Rung) {
        let (fullscreen, vsync) = (self.fullscreen, self.vsync);
        *self = VideoPrefs::of_rung(rung);
        self.fullscreen = fullscreen;
        self.vsync = vsync;
    }
}

impl Default for VideoPrefs {
    fn default() -> Self {
        VideoPrefs::of_rung(Rung::High)
    }
}

/// The camera, the shake and the HUD line.
#[derive(Debug, Clone, PartialEq)]
pub struct GamePrefs {
    /// Mouse look, a multiple of the base rate.
    pub mouse: f32,
    /// Stick and arrow-key look, a multiple of the base rate.
    pub stick: f32,
    pub invert_y: bool,
    /// Camera shake, 0 to 1.
    pub shake: f32,
    /// The key line along the bottom of the screen.
    pub help: bool,
    /// The world stands still while the system menu is up.
    pub pause: bool,
    /// Gear that is handed over (a drop, a chest, a reward, a purchase) goes on at once when its
    /// slot is empty; nothing worn is ever replaced.
    pub auto_equip: bool,
}

impl Default for GamePrefs {
    fn default() -> Self {
        GamePrefs {
            mouse: 1.0,
            stick: 1.0,
            invert_y: false,
            shake: 1.0,
            help: true,
            pause: true,
            auto_equip: true,
        }
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct Settings {
    pub sound: SoundPrefs,
    pub video: VideoPrefs,
    pub game: GamePrefs,
    pub binds: Binds,
}

/// A settings file as read: what it held and what it could not use.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub settings: Settings,
    pub problems: Vec<String>,
}

const HEADER: &str = "wssettings 1";

fn rung_id(r: Rung) -> &'static str {
    match r {
        Rung::Classic => "classic",
        Rung::Low => "low",
        Rung::Medium => "medium",
        Rung::High => "high",
        Rung::Ultra => "ultra",
    }
}

impl Settings {
    pub fn to_text(&self) -> String {
        let mut out = String::from(HEADER);
        out.push('\n');
        let mut put = |k: &str, v: String| {
            out.push_str(k);
            out.push(' ');
            out.push_str(&v);
            out.push('\n');
        };
        let flag = |b: bool| u8::from(b).to_string();
        let s = &self.sound;
        put("sound.master", format!("{:.2}", s.master));
        put("sound.music", format!("{:.2}", s.music));
        put("sound.ambience", format!("{:.2}", s.ambience));
        put("sound.effects", format!("{:.2}", s.effects));
        put("sound.voices", format!("{:.2}", s.voices));
        put("sound.loop", flag(s.loop_music));
        let v = &self.video;
        put("video.preset", rung_id(v.rung).to_string());
        put("video.shadows", v.shadows.to_string());
        put("video.ao", v.ao.to_string());
        put("video.bloom", v.bloom.to_string());
        put("video.fog", v.fog.to_string());
        put("video.light", v.light.to_string());
        put("video.sky", v.sky.to_string());
        put("video.wind", v.wind.to_string());
        put("video.water", v.water.to_string());
        put("video.grading", flag(v.grading));
        put("video.farclip", format!("{:.0}", v.farclip));
        put("video.grass", format!("{:.0}", v.grass));
        put("video.fullscreen", flag(v.fullscreen));
        put("video.vsync", flag(v.vsync));
        let g = &self.game;
        put("game.mouse", format!("{:.2}", g.mouse));
        put("game.stick", format!("{:.2}", g.stick));
        put("game.invert_y", flag(g.invert_y));
        put("game.shake", format!("{:.2}", g.shake));
        put("game.help", flag(g.help));
        put("game.pause", flag(g.pause));
        put("game.auto_equip", flag(g.auto_equip));
        for line in self.binds.to_lines() {
            out.push_str(&line);
            out.push('\n');
        }
        out
    }

    /// Reads [`Settings::to_text`]. A missing or wrong header is the defaults and one problem; a bad
    /// line is a problem and the rest still loads; every number is clamped into its range.
    pub fn from_text(text: &str) -> Loaded {
        let mut settings = Settings::default();
        let mut problems = Vec::new();
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some(HEADER) {
            problems.push(format!("not a {HEADER:?} file: defaults"));
            return Loaded { settings, problems };
        }
        // A preset line comes first in the file, but a hand-edited one may not: the preset is applied
        // before the lanes, whatever the order.
        let mut preset: Option<Rung> = None;
        let mut rest: Vec<(usize, &str, &str)> = Vec::new();
        for (n, line) in lines.enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once(' ') else {
                problems.push(format!("line {}: `key value` was expected", n + 2));
                continue;
            };
            match key {
                "video.preset" => match Rung::parse(value) {
                    Some(r) => preset = Some(r),
                    None => problems.push(format!("line {}: unknown preset {value:?}", n + 2)),
                },
                "bind" | "pad" => {
                    if let Err(e) = settings.binds.read_line(key, value) {
                        problems.push(format!("line {}: {e}", n + 2));
                    }
                }
                _ => rest.push((n + 2, key, value.trim())),
            }
        }
        if let Some(r) = preset {
            settings.video = VideoPrefs::of_rung(r);
        }
        for (n, key, value) in rest {
            if let Err(e) = settings.read(key, value) {
                problems.push(format!("line {n}: {e}"));
            }
        }
        Loaded { settings, problems }
    }

    fn read(&mut self, key: &str, value: &str) -> Result<(), String> {
        let num = |lo: f32, hi: f32| -> Result<f32, String> {
            value
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
                .map(|v| v.clamp(lo, hi))
                .ok_or_else(|| format!("{key}: a number was expected, not {value:?}"))
        };
        let tier = || -> Result<u8, String> { num(0.0, 2.0).map(|v| v.round() as u8) };
        let flag = || -> Result<bool, String> {
            match value {
                "1" | "on" | "true" | "yes" => Ok(true),
                "0" | "off" | "false" | "no" => Ok(false),
                _ => Err(format!("{key}: 0 or 1 was expected, not {value:?}")),
            }
        };
        match key {
            "sound.master" => self.sound.master = num(0.0, 1.0)?,
            "sound.music" => self.sound.music = num(0.0, 1.0)?,
            "sound.ambience" => self.sound.ambience = num(0.0, 1.0)?,
            "sound.effects" => self.sound.effects = num(0.0, 1.0)?,
            "sound.voices" => self.sound.voices = num(0.0, 1.0)?,
            "sound.loop" => self.sound.loop_music = flag()?,
            "video.shadows" => self.video.shadows = tier()?,
            "video.ao" => self.video.ao = tier()?,
            "video.bloom" => self.video.bloom = tier()?,
            "video.fog" => self.video.fog = tier()?,
            "video.light" => self.video.light = tier()?,
            "video.sky" => self.video.sky = tier()?,
            "video.wind" => self.video.wind = tier()?,
            "video.water" => self.video.water = tier()?,
            "video.grading" => self.video.grading = flag()?,
            "video.farclip" => self.video.farclip = num(177.0, 1497.0)?,
            "video.grass" => self.video.grass = num(0.0, 16.0)?,
            "video.fullscreen" => self.video.fullscreen = flag()?,
            "video.vsync" => self.video.vsync = flag()?,
            "game.mouse" => self.game.mouse = num(0.25, 3.0)?,
            "game.stick" => self.game.stick = num(0.25, 3.0)?,
            "game.invert_y" => self.game.invert_y = flag()?,
            "game.shake" => self.game.shake = num(0.0, 1.0)?,
            "game.help" => self.game.help = flag()?,
            "game.pause" => self.game.pause = flag()?,
            "game.auto_equip" => self.game.auto_equip = flag()?,
            _ => return Err(format!("unknown setting {key:?}")),
        }
        Ok(())
    }

    /// The environment's switches over the file's values, for this run only.
    pub fn with_env(mut self, var: impl Fn(&str) -> Option<String>) -> Settings {
        let level = |name: &str| {
            var(name)
                .and_then(|v| v.trim().parse::<f32>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0)
        };
        if let Some(v) = level("WOWSOULS_VOLUME") {
            self.sound.master = v.min(1.0);
        }
        if let Some(v) = level("WOWSOULS_MUSIC") {
            self.sound.music = v.min(1.0);
        }
        if let Some(v) = level("WOWSOULS_AMBIENCE") {
            self.sound.ambience = v.min(1.0);
        }
        if let Some(v) = var("WOWSOULS_MUSIC_LOOP") {
            self.sound.loop_music = !matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "off" | "false" | "no"
            );
        }
        if var("WOWSOULS_SHAKE").is_some_and(|v| v.trim() == "0") {
            self.game.shake = 0.0;
        }
        if let Some(r) = var("WOWSOULS_GFX").and_then(|v| Rung::parse(&v)) {
            self.video.choose(r);
        }
        self
    }
}

/// `settings-v1.txt`: `WOWSOULS_SETTINGS`, else beside the other saves.
pub fn save_path() -> PathBuf {
    std::env::var("WOWSOULS_SETTINGS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "saves".into()))
                .join("WoWSouls/settings-v1.txt")
        })
}

/// Whether the file is read and written: not in an unattended run, unless a file is named.
pub fn persists() -> bool {
    !crate::run_mode::unattended() || std::env::var_os("WOWSOULS_SETTINGS").is_some()
}

/// Reads a settings file; a missing one is the defaults, an unreadable one the defaults and a problem.
pub fn load(path: &Path) -> Loaded {
    match std::fs::read_to_string(path) {
        Ok(text) => Settings::from_text(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Loaded {
            settings: Settings::default(),
            problems: Vec::new(),
        },
        Err(e) => Loaded {
            settings: Settings::default(),
            problems: vec![format!("{}: {e}", path.display())],
        },
    }
}

/// Writes the file whole (to a temporary beside it first, then renamed over it).
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, settings.to_text())?;
    std::fs::rename(&tmp, path)
}

/// The settings as a resource, saved a moment after they change.
#[derive(Resource, Default)]
struct SaveState {
    /// Real second the settings last changed and have not been written since.
    dirty_since: Option<f64>,
}

/// How long a change waits for the next one before it is written (a slider drags through many).
const SAVE_AFTER_S: f64 = 0.5;

pub fn plugin(app: &mut App) {
    app.insert_resource(initial().clone())
        .init_resource::<MenuUp>()
        .init_resource::<SaveState>()
        .add_systems(Update, apply_video)
        .add_systems(Last, save_when_changed);
}

/// The video prefs reach the engine when they change: the config the lanes read, and the window's mode
/// and present mode. (At start the config already is what the prefs mean, with the instruments'
/// switches over it, so the first frame only takes note.)
fn apply_video(
    settings: Res<Settings>,
    mut config: ResMut<VideoConfig>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut seen: Local<Option<VideoPrefs>>,
) {
    use bevy::window::{MonitorSelection, PresentMode, WindowMode};
    let Some(previous) = seen.clone() else {
        *seen = Some(settings.video.clone());
        return;
    };
    if previous == settings.video {
        return;
    }
    *config = settings.video.config();
    for mut window in &mut windows {
        let mode = if settings.video.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
        if window.mode != mode {
            window.mode = mode;
        }
        let present = if settings.video.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
        if window.present_mode != present {
            window.present_mode = present;
        }
    }
    *seen = Some(settings.video.clone());
}

fn save_when_changed(
    settings: Res<Settings>,
    time: Res<Time<bevy::time::Real>>,
    mut state: ResMut<SaveState>,
    mut seen_first: Local<bool>,
) {
    let now = time.elapsed_secs_f64();
    if settings.is_changed() {
        // The insertion at start is not a change.
        if *seen_first {
            state.dirty_since = Some(now);
        }
        *seen_first = true;
    }
    let Some(since) = state.dirty_since else {
        return;
    };
    if now - since < SAVE_AFTER_S {
        return;
    }
    state.dirty_since = None;
    if !persists() {
        return;
    }
    let path = save_path();
    match save(&path, &settings) {
        Ok(()) => info!("settings: saved to {}", path.display()),
        Err(e) => warn!("settings: could not save {}: {e}", path.display()),
    }
}

/// What the run starts with: the file (when it persists), then the environment. Read once.
pub fn initial() -> &'static Settings {
    static INITIAL: std::sync::OnceLock<Settings> = std::sync::OnceLock::new();
    INITIAL.get_or_init(|| {
        let mut settings = Settings::default();
        if persists() {
            let path = save_path();
            let loaded = load(&path);
            for problem in &loaded.problems {
                warn!("settings: {}: {problem}", path.display());
            }
            settings = loaded.settings;
        }
        settings.with_env(|name| std::env::var(name).ok())
    })
}

#[cfg(test)]
mod tests {
    use super::binds::{Action, Chord};
    use super::*;
    use bevy::input::keyboard::KeyCode;
    use bevy::input::mouse::MouseButton;

    #[test]
    fn the_defaults_round_trip_through_the_file() {
        let s = Settings::default();
        let loaded = Settings::from_text(&s.to_text());
        assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
        assert_eq!(loaded.settings, s);
    }

    #[test]
    fn a_saved_file_reads_back_whole_including_the_bindings() {
        let dir = std::env::temp_dir().join(format!("wowsouls-settings-{}", std::process::id()));
        let path = dir.join("settings-v1.txt");
        let mut s = Settings::default();
        s.sound.master = 0.35;
        s.sound.music = 0.0;
        s.sound.loop_music = false;
        s.video.choose(Rung::Low);
        s.video.fog = 2;
        s.video.fullscreen = true;
        s.game.mouse = 1.75;
        s.game.invert_y = true;
        s.game.pause = false;
        s.game.auto_equip = false;
        s.binds.assign(
            Action::Heavy,
            0,
            Chord::key(KeyCode::KeyG).with(binds::Mods {
                shift: true,
                ..binds::Mods::NONE
            }),
        );
        s.binds
            .assign(Action::Light, 1, Chord::mouse(MouseButton::Back));
        save(&path, &s).unwrap();
        let loaded = load(&path);
        assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
        assert_eq!(loaded.settings, s);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn new_gear_goes_on_by_default_and_an_older_file_without_the_line_keeps_that() {
        assert!(Settings::default().game.auto_equip);
        let older = Settings::from_text("wssettings 1\ngame.mouse 2\n");
        assert!(older.problems.is_empty(), "{:?}", older.problems);
        assert!(older.settings.game.auto_equip);
        let off = Settings::from_text("wssettings 1\ngame.auto_equip 0\n");
        assert!(!off.settings.game.auto_equip);
    }

    #[test]
    fn a_missing_file_is_the_defaults_and_a_foreign_one_too() {
        let path = std::env::temp_dir().join("wowsouls-settings-nothing-here.txt");
        let _ = std::fs::remove_file(&path);
        let loaded = load(&path);
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.problems.is_empty());
        let foreign = Settings::from_text("hello\n");
        assert_eq!(foreign.settings, Settings::default());
        assert_eq!(foreign.problems.len(), 1);
    }

    #[test]
    fn a_bad_line_is_reported_and_the_rest_still_loads() {
        let text = "wssettings 1\nsound.master 0.2\nsound.music loud\nvideo.nonsense 3\n\
                    bogus\ngame.mouse 9\nvideo.ao 7\n";
        let loaded = Settings::from_text(text);
        assert_eq!(loaded.settings.sound.master, 0.2);
        assert_eq!(
            loaded.settings.sound.music, 1.0,
            "the bad value keeps the default"
        );
        assert_eq!(loaded.settings.game.mouse, 3.0, "clamped into its range");
        assert_eq!(loaded.settings.video.ao, 2, "a tier is 0 to 2");
        assert_eq!(loaded.problems.len(), 3, "{:?}", loaded.problems);
    }

    #[test]
    fn the_preset_line_is_applied_before_the_lanes_whatever_the_order() {
        let text = "wssettings 1\nvideo.bloom 0\nvideo.preset classic\nvideo.fog 2\n";
        let loaded = Settings::from_text(text);
        assert_eq!(loaded.settings.video.rung, Rung::Classic);
        assert_eq!(loaded.settings.video.bloom, 0);
        assert_eq!(
            loaded.settings.video.fog, 2,
            "an explicit lane survives the preset"
        );
        assert!(loaded.settings.video.is_custom());
    }

    #[test]
    fn a_preset_is_not_custom_until_a_lane_moves() {
        for rung in [
            Rung::Classic,
            Rung::Low,
            Rung::Medium,
            Rung::High,
            Rung::Ultra,
        ] {
            let mut v = VideoPrefs::of_rung(rung);
            assert!(!v.is_custom(), "{rung:?}");
            // The prefs mean exactly the preset's config where the preset has no hidden surprises.
            let cfg = v.config();
            let base = VideoConfig::rung(rung);
            assert_eq!(cfg.ambient_occlusion, base.ambient_occlusion);
            assert_eq!(cfg.bloom, base.bloom);
            assert_eq!(cfg.farclip, base.farclip);
            assert_eq!(cfg.world_shadows, base.world_shadows, "{rung:?}");
            assert_eq!(cfg.character_shadows, base.character_shadows, "{rung:?}");
            v.ao = (v.ao + 1) % 3;
            assert!(v.is_custom(), "{rung:?}");
            v.choose(rung);
            assert!(!v.is_custom());
        }
    }

    #[test]
    fn choosing_a_preset_keeps_the_window_switches() {
        let mut v = VideoPrefs {
            fullscreen: true,
            vsync: false,
            ..VideoPrefs::default()
        };
        v.choose(Rung::Ultra);
        assert!(v.fullscreen && !v.vsync);
        assert_eq!(v.rung, Rung::Ultra);
        assert_eq!(v.farclip, 1497.0);
    }

    #[test]
    fn shadows_off_turns_every_shadow_lane_off() {
        let mut v = VideoPrefs::of_rung(Rung::High);
        v.shadows = 0;
        let cfg = v.config();
        assert!(!cfg.world_shadows && !cfg.character_shadows);
        assert!(!cfg.interior_shadows && !cfg.exterior_shadows);
        v.shadows = 1;
        let cfg = v.config();
        assert!(cfg.character_shadows && !cfg.world_shadows);
    }

    #[test]
    fn the_environment_wins_over_the_file_for_one_run() {
        let mut s = Settings::default();
        s.sound.master = 0.2;
        s.game.shake = 1.0;
        let env = |name: &str| -> Option<String> {
            match name {
                "WOWSOULS_VOLUME" => Some("0.9".into()),
                "WOWSOULS_MUSIC_LOOP" => Some("0".into()),
                "WOWSOULS_SHAKE" => Some("0".into()),
                "WOWSOULS_GFX" => Some("classic".into()),
                _ => None,
            }
        };
        let s = s.with_env(env);
        assert_eq!(s.sound.master, 0.9);
        assert!(!s.sound.loop_music);
        assert_eq!(s.game.shake, 0.0);
        assert_eq!(s.video.rung, Rung::Classic);
        assert!(!s.video.is_custom());
        assert_eq!(
            Settings::default().with_env(|_| None),
            Settings::default(),
            "no environment, no change"
        );
    }
}
