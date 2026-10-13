//! The weather. The engine draws it (overcast and fog by the storm blend, rain and its mist, wet
//! surfaces, ripples, puddles, the sky) from `benilla_world::weather::WeatherState`, which a server's
//! `SMSG_WEATHER` drives; here there is no server, so this module plays the server's part: it rolls
//! the valley's weather and sends the engine the same `WeatherMessage`, and starts the matching sound
//! loop.
//!
//! The rolls follow MaNGOS's `Weather::ReGenerate`: every few minutes a 25 % chance to leave things as
//! they are, else a fine sky turns to rain with the zone's chance for the season (Elwynn Forest, from
//! the vMaNGOS `game_weather` table: 20 % spring, 15 % summer, 20 % fall and winter, never snow or a
//! storm), and rain either clears or changes its grade. The season is the real calendar's, as the
//! server's is. The engine ramps a change over about ten seconds.
//!
//! The sound is the server's `Weather::GetSound` kit (`SoundEntries` 8533 to 8535 for rain by grade,
//! 8536 to 8538 snow, 8556 to 8558 sand): while the player is outdoors it replaces the zone's
//! ambience bed (`zone.rs` crossfades over five seconds), indoors the area's own bed returns.
//!
//! - `WOWSOULS_WEATHER=<fine | rain | snow | sand>[,<grade 0..1>]` fixes the weather from the moment the
//!   player is in the world (no rolls); `random` or nothing rolls it. An unattended run (see `run_mode`)
//!   has clear skies unless this is set. `WOW_WEATHER` (the engine's own script) still works and wins.

use bevy::prelude::*;
use bevy::time::Real;

use benilla_world::weather::WeatherMessage;

use crate::settle::Phase;

/// Real seconds of play between two rolls, and before the first one.
const ROLL_EVERY_S: f64 = 300.0;
const FIRST_ROLL_S: f64 = 150.0;
/// The share (in 100) of rolls that change nothing, MaNGOS's `u < 25`.
const NO_CHANGE: u32 = 25;
/// The share (in 100) of rolls on which rain clears.
const RAIN_CLEARS: u32 = 55;
/// Rain chance per season (spring, summer, fall, winter) in Elwynn Forest, vMaNGOS `game_weather`.
const ELWYNN_RAIN: [u32; 4] = [20, 15, 20, 20];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Fine,
    Rain,
    Snow,
    Sand,
}

impl Kind {
    /// The wire type of `SMSG_WEATHER`.
    pub fn wire(self) -> u32 {
        match self {
            Kind::Fine => 0,
            Kind::Rain => 1,
            Kind::Snow => 2,
            Kind::Sand => 3,
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "fine" | "clear" | "off" | "none" => Some(Kind::Fine),
            "rain" => Some(Kind::Rain),
            "snow" => Some(Kind::Snow),
            "sand" | "dust" | "sandstorm" => Some(Kind::Sand),
            _ => None,
        }
    }
}

/// The weather loop's `SoundEntries` kit, `Weather::GetSound`: light under 0.4, medium under 0.7,
/// heavy above; 0 for a fine sky.
pub fn sound_kit(kind: Kind, grade: f32) -> u32 {
    let base = match kind {
        Kind::Fine => return 0,
        Kind::Rain => 8533,
        Kind::Snow => 8536,
        Kind::Sand => 8556,
    };
    base + if grade < 0.4 {
        0
    } else if grade < 0.7 {
        1
    } else {
        2
    }
}

/// The loop kit the weather wants now, read by `zone.rs` (0 = none).
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeatherAmbience(pub u32);

/// The season of a Unix time as MaNGOS counts it: `((day of year - 78 + 365) / 91) % 4`, 0 spring.
pub fn season(unix_secs: u64) -> usize {
    // Civil date from the day count (Howard Hinnant's algorithm), then the day of the year.
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy_mar = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy_mar + 2) / 153;
    let day = doy_mar - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    const BEFORE: [i64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let yday = BEFORE[(month - 1) as usize] + day - 1 + i64::from(leap && month > 2);
    (((yday - 78 + 365) / 91) % 4) as usize
}

/// One weather roll. `a` and `b` are draws in `0..100`, `g` a draw in `[0, 1)`; `rain_chance` is the
/// zone's, in 100. `None` leaves the weather as it is.
pub fn roll(
    kind: Kind,
    grade: f32,
    a: u32,
    b: u32,
    g: f32,
    rain_chance: u32,
) -> Option<(Kind, f32)> {
    if a < NO_CHANGE {
        return None;
    }
    match kind {
        Kind::Fine => (b < rain_chance).then_some((Kind::Rain, 0.35 + 0.65 * g)),
        _ if b < RAIN_CLEARS => Some((Kind::Fine, 0.0)),
        other => Some((other, (grade + (g - 0.5) * 0.5).clamp(0.3, 1.0))),
    }
}

/// A small generator for the rolls (the weather needs no better).
struct Rng(u64);

impl Rng {
    fn from_clock() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        Self(nanos | 1)
    }

    fn next(&mut self) -> u32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// What the environment says to do with the weather.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Plan {
    /// Roll it as the server does.
    Natural,
    /// Hold it: nothing is rolled.
    Fixed(Kind, f32),
}

/// `WOWSOULS_WEATHER`, over any variable source; `unattended` runs have clear skies unless it is set.
pub fn plan(var: Option<String>, unattended: bool) -> Plan {
    let Some(text) = var.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) else {
        return if unattended {
            Plan::Fixed(Kind::Fine, 0.0)
        } else {
            Plan::Natural
        };
    };
    if text.eq_ignore_ascii_case("random") {
        return Plan::Natural;
    }
    let (name, grade) = text.split_once(',').unwrap_or((&text, ""));
    match Kind::parse(name) {
        Some(Kind::Fine) => Plan::Fixed(Kind::Fine, 0.0),
        Some(kind) => Plan::Fixed(
            kind,
            grade
                .trim()
                .parse::<f32>()
                .map_or(0.8, |g| g.clamp(0.0, 1.0)),
        ),
        None => Plan::Natural,
    }
}

#[derive(Resource)]
struct Sim {
    plan: Plan,
    kind: Kind,
    grade: f32,
    /// Seconds of play so far, and when the next roll is due.
    played: f64,
    next_roll: f64,
    rng: Rng,
    started: bool,
    /// Whether a run that is not instrumented may ramp (a fixed weather in a check snaps).
    smooth: bool,
}

pub fn plugin(app: &mut App) {
    let unattended = crate::run_mode::unattended();
    let plan = plan(std::env::var("WOWSOULS_WEATHER").ok(), unattended);
    info!("weather: {plan:?}");
    app.init_resource::<WeatherAmbience>()
        .insert_resource(Sim {
            plan,
            kind: Kind::Fine,
            grade: 0.0,
            played: 0.0,
            next_roll: FIRST_ROLL_S,
            rng: Rng::from_clock(),
            started: false,
            smooth: !unattended,
        })
        .add_systems(Update, tick.run_if(in_state(Phase::Playing)));
}

fn send(
    sim: &mut Sim,
    kind: Kind,
    grade: f32,
    writer: &mut MessageWriter<WeatherMessage>,
    ambience: &mut WeatherAmbience,
) {
    let kit = sound_kit(kind, grade);
    info!("weather: {kind:?} grade {grade:.2} (sound kit {kit})");
    writer.write(WeatherMessage {
        weather_type: kind.wire(),
        grade,
        sound_id: kit,
        instant: !sim.smooth,
    });
    sim.kind = kind;
    sim.grade = grade;
    ambience.0 = kit;
}

fn tick(
    time: Res<Time<Real>>,
    mut sim: ResMut<Sim>,
    mut writer: MessageWriter<WeatherMessage>,
    mut ambience: ResMut<WeatherAmbience>,
) {
    sim.played += f64::from(time.delta_secs());
    if !sim.started {
        sim.started = true;
        if let Plan::Fixed(kind, grade) = sim.plan {
            if kind != Kind::Fine {
                send(&mut sim, kind, grade, &mut writer, &mut ambience);
            }
        }
        return;
    }
    if sim.plan != Plan::Natural || sim.played < sim.next_roll {
        return;
    }
    sim.next_roll = sim.played + ROLL_EVERY_S;
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (a, b, g) = (sim.rng.below(100), sim.rng.below(100), sim.rng.unit());
    if let Some((kind, grade)) = roll(sim.kind, sim.grade, a, b, g, ELWYNN_RAIN[season(unix)]) {
        send(&mut sim, kind, grade, &mut writer, &mut ambience);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sound_kit_follows_the_kind_and_the_grade() {
        assert_eq!(sound_kit(Kind::Fine, 1.0), 0);
        assert_eq!(sound_kit(Kind::Rain, 0.1), 8533);
        assert_eq!(sound_kit(Kind::Rain, 0.4), 8534);
        assert_eq!(sound_kit(Kind::Rain, 0.69), 8534);
        assert_eq!(sound_kit(Kind::Rain, 0.7), 8535);
        assert_eq!(sound_kit(Kind::Snow, 0.9), 8538);
        assert_eq!(sound_kit(Kind::Sand, 0.0), 8556);
    }

    #[test]
    fn the_season_is_the_servers() {
        // 2026-01-15, 03-25, 06-21, 09-23, 12-21 (UTC, noon)
        let at = |days_from_epoch: u64| days_from_epoch * 86_400 + 43_200;
        assert_eq!(season(at(20_468)), 3, "mid January is winter");
        assert_eq!(season(at(20_537)), 0, "late March is spring");
        assert_eq!(season(at(20_624)), 1, "the summer solstice");
        assert_eq!(season(at(20_719)), 2, "the autumn equinox is fall");
        assert_eq!(season(at(20_808)), 3, "the winter solstice");
    }

    #[test]
    fn a_fine_sky_turns_to_rain_by_the_zones_chance() {
        assert_eq!(
            roll(Kind::Fine, 0.0, 10, 0, 0.5, 20),
            None,
            "a roll under 25 changes nothing"
        );
        assert_eq!(
            roll(Kind::Fine, 0.0, 50, 19, 0.0, 20),
            Some((Kind::Rain, 0.35))
        );
        let (kind, grade) = roll(Kind::Fine, 0.0, 99, 0, 0.999, 20).unwrap();
        assert_eq!(kind, Kind::Rain);
        assert!((0.35..=1.0).contains(&grade));
        assert_eq!(
            roll(Kind::Fine, 0.0, 50, 20, 0.5, 20),
            None,
            "the chance is under, not up to"
        );
        assert_eq!(
            roll(Kind::Fine, 0.0, 99, 99, 0.5, 0),
            None,
            "a zone with no rain never rains"
        );
    }

    #[test]
    fn rain_clears_or_changes_its_grade() {
        assert_eq!(
            roll(Kind::Rain, 0.8, 60, 54, 0.5, 20),
            Some((Kind::Fine, 0.0))
        );
        let (kind, grade) = roll(Kind::Rain, 0.8, 60, 55, 1.0, 20).unwrap();
        assert_eq!(kind, Kind::Rain);
        assert!((grade - 1.0).abs() < 1e-6, "a swing past the top clamps");
        let (_, low) = roll(Kind::Rain, 0.3, 60, 80, 0.0, 20).unwrap();
        assert_eq!(low, 0.3, "and one below the bottom");
    }

    #[test]
    fn the_environment_plans_the_weather() {
        assert_eq!(plan(None, false), Plan::Natural);
        assert_eq!(
            plan(None, true),
            Plan::Fixed(Kind::Fine, 0.0),
            "a check has clear skies"
        );
        assert_eq!(
            plan(Some("rain".into()), true),
            Plan::Fixed(Kind::Rain, 0.8)
        );
        assert_eq!(
            plan(Some("Rain, 0.35".into()), false),
            Plan::Fixed(Kind::Rain, 0.35)
        );
        assert_eq!(
            plan(Some("snow,9".into()), false),
            Plan::Fixed(Kind::Snow, 1.0)
        );
        assert_eq!(
            plan(Some("fine".into()), false),
            Plan::Fixed(Kind::Fine, 0.0)
        );
        assert_eq!(plan(Some("random".into()), true), Plan::Natural);
        assert_eq!(
            plan(Some("hail".into()), false),
            Plan::Natural,
            "unknown reads as natural"
        );
    }

    #[test]
    fn the_rolls_come_out_of_range_and_spread() {
        let mut rng = Rng(0x1234_5678_9abc_def1);
        let mut seen = [0u32; 4];
        for _ in 0..4000 {
            let b = rng.below(100);
            assert!(b < 100);
            let u = rng.unit();
            assert!((0.0..1.0).contains(&u));
            seen[(b / 25) as usize] += 1;
        }
        assert!(
            seen.iter().all(|&n| n > 800),
            "a quarter each, roughly: {seen:?}"
        );
    }
}
