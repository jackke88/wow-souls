//! The day and the night. The engine has no server here, so its clock stands at noon for ever
//! (`WorldTime` "live: false"); this module is the clock: it advances a game day in
//! `WOWSOULS_DAY_MINUTES` real minutes (default 24, WoW's own pace: one game hour a real minute) while
//! the player is in the world, and publishes it as `benilla_world::lighting::WorldTime` every frame.
//! The sun, the moon and its phase, the stars, the `Light.dbc` colours, the zone music's day and night
//! tracks (`zone.rs` reads `GameClock`) and the sound beds all follow it.
//!
//! - A first run starts at 08:00; later runs resume where the last one stopped (`clock-v1.txt` beside
//!   the other saves, `WOWSOULS_CLOCK_SAVE=<file>` overrides). A new character starts a new clock.
//! - `WOWSOULS_HOUR=<h | h:mm>` starts the clock at that time (and ignores the save),
//!   `WOWSOULS_DAY_MINUTES=<real minutes per game day>` sets the pace, `0` freezes the clock.
//!   `WOW_CLOCK=<minute 0..1439>` (the engine's own lever) pins the hour for good and wins over all of this.
//! - An unattended run (any instrument switch, see `run_mode`) neither reads nor writes the save (unless
//!   `WOWSOULS_CLOCK_SAVE` names one) and
//!   stands still at 12:00 unless one of the two switches above is set, so scripted checks keep the
//!   noon they always had.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::time::Real;

use benilla_world::lighting::{LightingResolveSet, WorldTime};

use crate::settle::Phase;

/// Game minutes in a day.
pub const MINUTES_PER_DAY: f64 = 1440.0;
/// Real minutes a game day lasts when nothing says otherwise: WoW's one game hour a real minute.
const DEFAULT_DAY_MINUTES: f64 = 24.0;
/// Where a new game starts.
const NEW_GAME_MINUTE: f64 = 8.0 * 60.0;
/// Where an unattended run stands.
const UNATTENDED_MINUTE: f64 = 12.0 * 60.0;
/// How often a changed clock is written (real seconds).
const SAVE_EVERY_S: f64 = 5.0;

/// The game time: the minute of the day and how many whole days have gone by.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct DayClock {
    pub minute: f64,
    pub days: u64,
}

impl DayClock {
    pub fn at(minute: f64) -> Self {
        Self {
            minute: minute.rem_euclid(MINUTES_PER_DAY),
            days: 0,
        }
    }

    /// The clock `game_minutes` later (a day wraps and counts).
    pub fn advance(self, game_minutes: f64) -> Self {
        let total = self.minute + game_minutes.max(0.0);
        let wraps = (total / MINUTES_PER_DAY).floor();
        Self {
            minute: total - wraps * MINUTES_PER_DAY,
            days: self.days + wraps as u64,
        }
    }

    /// The engine's clock: whole minute, fractional minute, and the unwrapped day count its moon
    /// phase runs on.
    pub fn world_time(self) -> WorldTime {
        WorldTime {
            minute: (self.minute as u32).min(1439),
            minute_f: self.minute as f32,
            day: self.days as f64 + self.minute / MINUTES_PER_DAY,
            live: true,
        }
    }

    /// `(hour, minute)` for a log line.
    pub fn hhmm(self) -> (u32, u32) {
        let m = self.minute as u32;
        (m / 60, m % 60)
    }
}

/// Game minutes that pass per real second for a day of `day_minutes` real minutes (0 = frozen).
pub fn game_minutes_per_second(day_minutes: f64) -> f64 {
    if day_minutes.is_finite() && day_minutes > 0.0 {
        MINUTES_PER_DAY / (day_minutes * 60.0)
    } else {
        0.0
    }
}

/// `h`, `h.5` or `h:mm` as a minute of the day.
pub fn parse_hour(text: &str) -> Option<f64> {
    let text = text.trim();
    let minutes = match text.split_once(':') {
        Some((h, m)) => h.trim().parse::<f64>().ok()? * 60.0 + m.trim().parse::<f64>().ok()?,
        None => text.parse::<f64>().ok()? * 60.0,
    };
    minutes
        .is_finite()
        .then(|| minutes.rem_euclid(MINUTES_PER_DAY))
}

/// The save text: a header, the minute and the day count.
pub fn encode(clock: DayClock) -> String {
    format!(
        "wsclock 1\nminute {:.3}\ndays {}\n",
        clock.minute, clock.days
    )
}

/// The clock a save holds, or `None` for anything that is not one.
pub fn decode(text: &str) -> Option<DayClock> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "wsclock 1" {
        return None;
    }
    let (mut minute, mut days) = (None, None);
    for line in lines {
        match line.split_once(' ') {
            Some(("minute", v)) => minute = v.trim().parse::<f64>().ok(),
            Some(("days", v)) => days = v.trim().parse::<u64>().ok(),
            _ => {}
        }
    }
    let minute = minute.filter(|m| m.is_finite() && (0.0..MINUTES_PER_DAY).contains(m))?;
    Some(DayClock {
        minute,
        days: days?,
    })
}

/// The save file: `WOWSOULS_CLOCK_SAVE`, else `clock-v1.txt` beside the other saves.
pub fn save_path() -> PathBuf {
    std::env::var("WOWSOULS_CLOCK_SAVE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "saves".into()))
                .join("WoWSouls/clock-v1.txt")
        })
}

/// How the clock runs, decided once from the environment.
#[derive(Resource, Clone, Debug)]
struct Pace {
    minutes_per_second: f64,
    /// The file the clock is saved to; `None` when unattended.
    save: Option<PathBuf>,
}

/// What the environment says: (the start minute if set, the day length if set).
fn from_vars(var: impl Fn(&str) -> Option<String>) -> (Option<f64>, Option<f64>) {
    let hour = var("WOWSOULS_HOUR").and_then(|v| parse_hour(&v));
    let day = var("WOWSOULS_DAY_MINUTES")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0);
    (hour, day)
}

pub fn plugin(app: &mut App) {
    let unattended = crate::run_mode::unattended();
    let (hour, day) = from_vars(|name| std::env::var(name).ok());
    let day_minutes = day.unwrap_or(if unattended { 0.0 } else { DEFAULT_DAY_MINUTES });
    // A run that names its save file wants it read and written, attended or not.
    let save = (!unattended || std::env::var_os("WOWSOULS_CLOCK_SAVE").is_some()).then(save_path);
    let start = match (hour, &save) {
        (Some(minute), _) => DayClock::at(minute),
        (None, Some(path)) => std::fs::read_to_string(path)
            .ok()
            .and_then(|text| decode(&text))
            .unwrap_or_else(|| DayClock::at(NEW_GAME_MINUTE)),
        (None, None) => DayClock::at(UNATTENDED_MINUTE),
    };
    let (h, m) = start.hhmm();
    info!(
        "daycycle: {h:02}:{m:02} (day {}), {}",
        start.days,
        if day_minutes > 0.0 {
            format!("a day lasts {day_minutes} real minutes")
        } else {
            "the clock is frozen".to_string()
        }
    );
    let menu_hour = menu_minute(
        hour.is_some(),
        std::env::var_os("WOW_CLOCK").is_some(),
        unattended,
        crate::menu::wanted(),
        std::env::var("WOWSOULS_MENU_HOUR")
            .ok()
            .and_then(|v| parse_hour(&v)),
    );
    app.insert_resource(start)
        .insert_resource(MenuHour(menu_hour))
        .insert_resource(Pace {
            minutes_per_second: game_minutes_per_second(day_minutes),
            save,
        })
        .add_systems(
            Update,
            (tick.run_if(in_state(Phase::Playing)), publish, save_clock)
                .chain()
                // The lighting resolves the sky from this frame's time.
                .before(LightingResolveSet),
        );
}

fn tick(time: Res<Time<Real>>, pace: Res<Pace>, mut clock: ResMut<DayClock>) {
    if pace.minutes_per_second > 0.0 {
        *clock = clock.advance(f64::from(time.delta_secs()) * pace.minutes_per_second);
    }
}

/// Where the menu's scene is lit: the end of the day, the valley in its warm low light.
const MENU_MINUTE: f64 = 18.0 * 60.0 + 40.0;

/// The light the menu's scene wears, as a minute of the day: dusk, unless the run names the hour
/// (`WOWSOULS_HOUR`, the engine's `WOW_CLOCK`), is an unattended check (which keeps its noon) or skips
/// the menu. `WOWSOULS_MENU_HOUR=<h | h:mm>` names it outright. The game clock itself is untouched.
pub fn menu_minute(
    hour_named: bool,
    pinned: bool,
    unattended: bool,
    menu_shown: bool,
    named_menu_hour: Option<f64>,
) -> Option<f64> {
    if !menu_shown {
        return None;
    }
    if let Some(minute) = named_menu_hour {
        return Some(minute);
    }
    if hour_named || pinned || unattended {
        None
    } else {
        Some(MENU_MINUTE)
    }
}

/// The minute the streaming and menu phases publish instead of the clock's, if any.
#[derive(Resource, Clone, Copy, Debug)]
struct MenuHour(Option<f64>);

fn publish(
    clock: Res<DayClock>,
    phase: Res<State<Phase>>,
    menu: Res<MenuHour>,
    mut out: ResMut<WorldTime>,
) {
    let shown = match (menu.0, phase.get()) {
        (Some(minute), Phase::Streaming | Phase::Menu) => DayClock {
            minute,
            days: clock.days,
        },
        _ => *clock,
    };
    *out = shown.world_time();
}

fn save_clock(
    clock: Res<DayClock>,
    pace: Res<Pace>,
    time: Res<Time<Real>>,
    mut last: Local<Option<(f64, DayClock)>>,
) {
    let Some(path) = &pace.save else {
        return;
    };
    let now = time.elapsed_secs_f64();
    match *last {
        None => *last = Some((now, *clock)),
        Some((at, saved)) if now - at >= SAVE_EVERY_S => {
            if saved != *clock {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = std::fs::write(path, encode(*clock)) {
                    warn!(
                        "daycycle: the clock could not be saved to {}: {e}",
                        path.display()
                    );
                }
            }
            *last = Some((now, *clock));
        }
        Some(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_wraps_and_counts() {
        let c = DayClock::at(1430.0).advance(20.0);
        assert!((c.minute - 10.0).abs() < 1e-9);
        assert_eq!(c.days, 1);
        let c = DayClock::at(0.0).advance(3.0 * MINUTES_PER_DAY + 5.0);
        assert!((c.minute - 5.0).abs() < 1e-9 && c.days == 3);
        assert_eq!(
            DayClock::at(100.0).advance(-5.0),
            DayClock::at(100.0),
            "time never runs back"
        );
    }

    #[test]
    fn the_engine_clock_is_whole_and_fractional_minute_and_the_unwrapped_day() {
        let t = DayClock {
            minute: 90.5,
            days: 2,
        }
        .world_time();
        assert_eq!(t.minute, 90);
        assert!((t.minute_f - 90.5).abs() < 1e-6);
        assert!((t.day - (2.0 + 90.5 / 1440.0)).abs() < 1e-9);
        assert!(t.live);
        assert_eq!(
            DayClock {
                minute: 1439.99,
                days: 0
            }
            .world_time()
            .minute,
            1439
        );
    }

    #[test]
    fn a_day_of_24_real_minutes_is_one_game_minute_a_second() {
        assert!((game_minutes_per_second(24.0) - 1.0).abs() < 1e-12);
        assert!((game_minutes_per_second(48.0) - 0.5).abs() < 1e-12);
        assert_eq!(game_minutes_per_second(0.0), 0.0);
        assert_eq!(game_minutes_per_second(f64::NAN), 0.0);
    }

    #[test]
    fn the_menu_is_lit_at_dusk_unless_something_names_the_hour() {
        let dusk = Some(18.0 * 60.0 + 40.0);
        assert_eq!(menu_minute(false, false, false, true, None), dusk);
        assert_eq!(
            menu_minute(true, false, false, true, None),
            None,
            "WOWSOULS_HOUR"
        );
        assert_eq!(
            menu_minute(false, true, false, true, None),
            None,
            "WOW_CLOCK"
        );
        assert_eq!(
            menu_minute(false, false, true, true, None),
            None,
            "a check keeps its noon"
        );
        assert_eq!(
            menu_minute(false, false, false, false, None),
            None,
            "no menu, no dusk"
        );
        assert_eq!(
            menu_minute(true, true, true, true, Some(300.0)),
            Some(300.0),
            "WOWSOULS_MENU_HOUR names it outright"
        );
        assert_eq!(menu_minute(false, false, false, false, Some(300.0)), None);
    }

    #[test]
    fn hours_parse_as_hours_or_clock_times() {
        assert_eq!(parse_hour("18"), Some(1080.0));
        assert_eq!(parse_hour("18.5"), Some(1110.0));
        assert_eq!(parse_hour("6:30"), Some(390.0));
        assert_eq!(parse_hour("25"), Some(60.0), "past midnight wraps");
        assert_eq!(parse_hour("noon"), None);
    }

    #[test]
    fn a_save_round_trips_and_junk_is_refused() {
        let c = DayClock {
            minute: 512.25,
            days: 7,
        };
        assert_eq!(decode(&encode(c)), Some(c));
        assert_eq!(decode("wsclock 2\nminute 1\ndays 0\n"), None);
        assert_eq!(
            decode("wsclock 1\nminute 2000\ndays 0\n"),
            None,
            "a minute past the day"
        );
        assert_eq!(decode("wsclock 1\nminute x\ndays 0\n"), None);
        assert_eq!(decode("wsclock 1\nminute 5\n"), None, "no day count");
        assert_eq!(decode(""), None);
    }

    #[test]
    fn the_environment_names_the_hour_and_the_pace() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| (*v).to_string())
            }
        };
        assert_eq!(from_vars(env(&[])), (None, None));
        assert_eq!(
            from_vars(env(&[
                ("WOWSOULS_HOUR", "21:30"),
                ("WOWSOULS_DAY_MINUTES", "60")
            ])),
            (Some(1290.0), Some(60.0))
        );
        assert_eq!(
            from_vars(env(&[("WOWSOULS_DAY_MINUTES", "-3")])),
            (None, None)
        );
    }
}
