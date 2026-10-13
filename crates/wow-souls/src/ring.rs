//! Which creatures exist this tick: a ring around the player. A record spawns inside the spawn
//! radius, despawns beyond the despawn radius (the gap is hysteresis so a walk along the edge does
//! not flicker), and a killed creature comes back only after its respawn time AND only while the
//! player is at least [`CLEAR`] yards away: a cave round trip never meets a respawn behind it.

use std::sync::OnceLock;

/// The two radii (yd).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ring {
    pub spawn: f32,
    pub despawn: f32,
}

/// WoW's object visibility is about 90 yd; the despawn radius sits well inside the engine's 267 yd
/// minimum terrain window (`farclip` 177), so a spawn always has ground under it.
pub const RING: Ring = Ring {
    spawn: 100.0,
    despawn: 130.0,
};
/// A respawn waits until the player is at least this far (yd).
pub const CLEAR: f32 = 60.0;
/// A creature beyond this distance (yd) is not simulated (no thinking, no walking, no motor).
pub const SIM: f32 = 70.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    /// No entity.
    Absent,
    /// An entity exists (a corpse too: `until` says when it is due to be replaced).
    Live,
    /// Never spawned in this game: not a creature of this step (a critter, a friendly NPC, a hostile
    /// the roster lacks) or one whose model is missing.
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rec {
    /// Bevy ground `[x, z]`.
    pub at: [f32; 2],
    pub state: State,
    /// Set when the creature died: the real-time second it may come back (`None` while alive).
    pub until: Option<f32>,
}

/// What to do this tick: record indices to spawn (nearest first) and to despawn.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub spawn: Vec<usize>,
    pub despawn: Vec<usize>,
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// The tick's plan. `keep(i)` is true for a creature that must not be despawned (engaged,
/// returning, flinching, or a corpse the player stands near). At most `max_spawn` records are
/// spawned per tick.
pub fn plan(
    recs: &[Rec],
    player: [f32; 2],
    now: f32,
    ring: Ring,
    max_spawn: usize,
    keep: &dyn Fn(usize) -> bool,
) -> Plan {
    let mut out = Plan::default();
    let mut want: Vec<(f32, usize)> = Vec::new();
    for (i, r) in recs.iter().enumerate() {
        let d = dist(player, r.at);
        match r.state {
            State::Off => {}
            State::Absent => {
                let due = r.until.is_none_or(|u| now >= u && d >= CLEAR);
                if d <= ring.spawn && due {
                    want.push((d, i));
                }
            }
            State::Live => {
                // A corpse that is due is replaced, but never in front of the player.
                let corpse_due = r.until.is_some_and(|u| now >= u && d >= CLEAR);
                if (d > ring.despawn || corpse_due) && !keep(i) {
                    out.despawn.push(i);
                }
            }
        }
    }
    want.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.spawn = want.into_iter().take(max_spawn).map(|(_, i)| i).collect();
    out
}

/// How long a creature stays dead (s): `mult` times its database respawn time, never less than
/// `floor`. The database's times are minutes at most; a Souls player needs to be able to walk into a
/// cave and out again.
pub fn respawn_delay(db_max_secs: u32, mult: f32, floor: f32) -> f32 {
    (db_max_secs as f32 * mult).max(floor)
}

/// The respawn rule: [`respawn_delay`]'s floor (s) and multiplier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Respawn {
    pub floor: f32,
    pub mult: f32,
}

impl Respawn {
    /// Ten minutes at least, four times the database's time.
    pub const DEFAULT: Respawn = Respawn {
        floor: 600.0,
        mult: 4.0,
    };

    pub fn delay(&self, db_max_secs: u32) -> f32 {
        respawn_delay(db_max_secs, self.mult, self.floor)
    }
}

/// `WOWSOULS_RESPAWN=<floor s>` and `WOWSOULS_RESPAWN_X=<multiplier>` (a test aid: `_X=0` makes the
/// floor the whole delay); anything unreadable or negative keeps the default.
pub fn parse_respawn(floor: Option<&str>, mult: Option<&str>) -> Respawn {
    let num = |v: Option<&str>| {
        v.and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|f| f.is_finite() && *f >= 0.0)
    };
    Respawn {
        floor: num(floor).unwrap_or(Respawn::DEFAULT.floor),
        mult: num(mult).unwrap_or(Respawn::DEFAULT.mult),
    }
}

/// `WOWSOULS_RING=<spawn>,<despawn>` (yd): the despawn radius must lie beyond the spawn radius.
pub fn parse_ring(v: &str) -> Option<Ring> {
    let (a, b) = v.split_once(',')?;
    let (spawn, despawn) = (a.trim().parse::<f32>().ok()?, b.trim().parse::<f32>().ok()?);
    (spawn.is_finite() && despawn.is_finite() && spawn > 0.0 && despawn > spawn)
        .then_some(Ring { spawn, despawn })
}

/// The `until` of a creature that never comes back: no clock reaches it and a reset keeps it.
pub const NEVER: f32 = f32::INFINITY;

/// The creature of `r` is dead for good (a boss, a miniboss).
pub fn note_gone_for_good(r: &mut Rec) {
    r.until = Some(NEVER);
}

/// `WOWSOULS_SIM=<yd>`, read once; [`SIM`] otherwise.
pub fn sim() -> f32 {
    static SIM_YD: OnceLock<f32> = OnceLock::new();
    *SIM_YD.get_or_init(|| {
        std::env::var("WOWSOULS_SIM")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|f| f.is_finite() && *f > 0.0)
            .unwrap_or(SIM)
    })
}

/// Whether a creature is simulated this frame (thinks, walks, steps its motor): inside `sim` yards of
/// the player, or anything but idle (a chase, a fight, the walk home is seen through wherever it is).
pub fn simulated(dist: f32, idle: bool, sim: f32) -> bool {
    !idle || dist <= sim
}

/// The creature of `r` died at `now`: it may come back `delay` seconds later.
pub fn note_death(r: &mut Rec, now: f32, delay: f32) {
    if r.until.is_none() {
        r.until = Some(now + delay);
    }
}

/// The player respawned (or pressed the reset): every creature is due again at once.
pub fn clear_timers(recs: &mut [Rec]) {
    for r in recs {
        if r.until != Some(NEVER) {
            r.until = None;
        }
    }
}

/// A record's creature was placed: alive, no timer.
pub fn spawned(r: &mut Rec) {
    r.state = State::Live;
    r.until = None;
}

/// A record's entity was removed; a running timer stays (a corpse beyond the ring still waits).
pub fn despawned(r: &mut Rec) {
    r.state = State::Absent;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(x: f32, state: State, until: Option<f32>) -> Rec {
        Rec {
            at: [x, 0.0],
            state,
            until,
        }
    }
    fn nobody(_: usize) -> bool {
        false
    }

    #[test]
    fn creatures_inside_the_spawn_radius_spawn_nearest_first_and_capped() {
        let recs = [
            rec(90.0, State::Absent, None),
            rec(10.0, State::Absent, None),
            rec(50.0, State::Absent, None),
            rec(150.0, State::Absent, None),
        ];
        let p = plan(&recs, [0.0, 0.0], 0.0, RING, 2, &nobody);
        assert_eq!(p.spawn, vec![1, 2], "nearest two, the far one never");
        assert!(p.despawn.is_empty());
    }

    #[test]
    fn the_gap_between_the_radii_is_hysteresis() {
        // A live creature at 110 yd stays; an absent one at 110 yd does not spawn.
        let recs = [
            rec(110.0, State::Live, None),
            rec(110.0, State::Absent, None),
        ];
        let p = plan(&recs, [0.0, 0.0], 0.0, RING, 9, &nobody);
        assert_eq!(p, Plan::default());
        // Beyond the despawn radius the live one goes, unless it is kept.
        let far = [rec(140.0, State::Live, None)];
        assert_eq!(
            plan(&far, [0.0, 0.0], 0.0, RING, 9, &nobody).despawn,
            vec![0]
        );
        assert!(plan(&far, [0.0, 0.0], 0.0, RING, 9, &|_| true)
            .despawn
            .is_empty());
    }

    #[test]
    fn a_killed_creature_waits_for_its_time_and_for_the_player_to_be_away() {
        // Dead, absent, due at t=100.
        let dead = [rec(80.0, State::Absent, Some(100.0))];
        assert!(
            plan(&dead, [0.0, 0.0], 50.0, RING, 9, &nobody)
                .spawn
                .is_empty(),
            "not yet"
        );
        assert_eq!(
            plan(&dead, [0.0, 0.0], 150.0, RING, 9, &nobody).spawn,
            vec![0],
            "due and 80 yd away"
        );
        let near = [rec(30.0, State::Absent, Some(100.0))];
        assert!(
            plan(&near, [0.0, 0.0], 150.0, RING, 9, &nobody)
                .spawn
                .is_empty(),
            "due but the player is 30 yd away"
        );
    }

    #[test]
    fn a_due_corpse_is_replaced_only_out_of_the_players_sight_distance() {
        let corpse_near = [rec(30.0, State::Live, Some(100.0))];
        assert!(plan(&corpse_near, [0.0, 0.0], 150.0, RING, 9, &nobody)
            .despawn
            .is_empty());
        let corpse_away = [rec(80.0, State::Live, Some(100.0))];
        assert_eq!(
            plan(&corpse_away, [0.0, 0.0], 150.0, RING, 9, &nobody).despawn,
            vec![0]
        );
        // A corpse that is not due stays even far away only until the despawn radius.
        let corpse_early = [rec(80.0, State::Live, Some(500.0))];
        assert!(plan(&corpse_early, [0.0, 0.0], 150.0, RING, 9, &nobody)
            .despawn
            .is_empty());
    }

    #[test]
    fn a_cave_round_trip_meets_no_respawn() {
        // Killed at t=0, back after at least 600 s: the player returns at t=240 and nothing is there.
        let delay = respawn_delay(270, 4.0, 600.0);
        assert_eq!(delay, 1080.0);
        assert_eq!(Respawn::DEFAULT.delay(270), 1080.0);
        let dead = [rec(20.0, State::Absent, Some(delay))];
        assert!(plan(&dead, [0.0, 0.0], 240.0, RING, 9, &nobody)
            .spawn
            .is_empty());
        assert_eq!(respawn_delay(30, 4.0, 600.0), 600.0, "the floor");
    }

    #[test]
    fn off_records_never_spawn_or_despawn() {
        let recs = [
            rec(5.0, State::Off, None),
            rec(500.0, State::Off, Some(0.0)),
        ];
        assert_eq!(
            plan(&recs, [0.0, 0.0], 9.0, RING, 9, &nobody),
            Plan::default()
        );
    }

    #[test]
    fn a_due_corpse_the_player_stands_near_is_kept() {
        // Its spawn spot is 80 yd away, but it died beside the player: it stays until they leave.
        let corpse = [rec(80.0, State::Live, Some(100.0))];
        assert!(plan(&corpse, [0.0, 0.0], 150.0, RING, 9, &|_| true)
            .despawn
            .is_empty());
    }

    #[test]
    fn the_respawn_rule_reads_its_two_switches() {
        assert_eq!(parse_respawn(None, None), Respawn::DEFAULT);
        let quick = parse_respawn(Some("30"), Some("0"));
        assert_eq!(quick.delay(270), 30.0, "x0: the floor is the whole delay");
        assert_eq!(parse_respawn(Some("30"), None).delay(270), 1080.0);
        assert_eq!(parse_respawn(Some("-5"), Some("x")), Respawn::DEFAULT);
    }

    #[test]
    fn the_ring_switch_needs_the_despawn_radius_beyond_the_spawn_radius() {
        assert_eq!(
            parse_ring("60, 80"),
            Some(Ring {
                spawn: 60.0,
                despawn: 80.0
            })
        );
        assert_eq!(parse_ring("80,60"), None);
        assert_eq!(parse_ring("80"), None);
        assert_eq!(parse_ring("a,b"), None);
    }

    #[test]
    fn only_idle_creatures_beyond_the_sim_radius_rest() {
        assert!(simulated(10.0, true, SIM));
        assert!(simulated(SIM, true, SIM));
        assert!(!simulated(SIM + 0.1, true, SIM));
        assert!(simulated(200.0, false, SIM), "a creature walking home");
    }

    #[test]
    fn the_respawn_bookkeeping_runs_kill_wait_replace_and_reset() {
        let r = Respawn {
            floor: 30.0,
            mult: 0.0,
        };
        let mut recs = [rec(80.0, State::Absent, None)];
        // Spawned at t=0, killed at t=10: the corpse stays until t=40.
        spawned(&mut recs[0]);
        assert_eq!(recs[0].state, State::Live);
        note_death(&mut recs[0], 10.0, r.delay(270));
        assert_eq!(recs[0].until, Some(40.0));
        // A second death message (it cannot die twice) does not move the time.
        note_death(&mut recs[0], 20.0, r.delay(270));
        assert_eq!(recs[0].until, Some(40.0));
        let p = [0.0, 0.0];
        assert!(plan(&recs, p, 39.0, RING, 9, &nobody).despawn.is_empty());
        assert_eq!(plan(&recs, p, 40.0, RING, 9, &nobody).despawn, vec![0]);
        despawned(&mut recs[0]);
        // The next tick brings it back.
        assert_eq!(plan(&recs, p, 40.25, RING, 9, &nobody).spawn, vec![0]);
        spawned(&mut recs[0]);
        assert_eq!(recs[0].until, None);
        // A reset clears every timer, so a dead one is due at once, the player near or not.
        note_death(&mut recs[0], 50.0, r.delay(270));
        despawned(&mut recs[0]);
        let near = [70.0, 10.0];
        assert!(plan(&recs, near, 51.0, RING, 9, &nobody).spawn.is_empty());
        clear_timers(&mut recs);
        assert_eq!(plan(&recs, near, 51.0, RING, 9, &nobody).spawn, vec![0]);
    }

    #[test]
    fn a_creature_gone_for_good_is_never_due_and_a_reset_leaves_it() {
        let mut dead = rec(5.0, State::Absent, None);
        note_gone_for_good(&mut dead);
        assert_eq!(dead.until, Some(NEVER));
        let p = plan(&[dead], [0.0, 0.0], 1.0e9, RING, 9, &nobody);
        assert!(p.spawn.is_empty(), "never spawns, however late");
        let mut recs = [dead, rec(5.0, State::Absent, Some(100.0))];
        clear_timers(&mut recs);
        assert_eq!(recs[0].until, Some(NEVER), "the reset does not revive it");
        assert_eq!(recs[1].until, None, "an ordinary timer is cleared");
    }

    #[test]
    fn a_corpse_that_is_gone_for_good_is_not_replaced() {
        let mut live = rec(5.0, State::Live, None);
        note_gone_for_good(&mut live);
        let p = plan(&[live], [0.0, 0.0], 1.0e9, RING, 9, &nobody);
        assert!(p.despawn.is_empty() && p.spawn.is_empty());
    }
}
