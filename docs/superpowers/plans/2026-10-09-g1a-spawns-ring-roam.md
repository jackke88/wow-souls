# G1a Spawns From The Manifest, The Ring, Roaming And Respawn Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The game spawns its hostile creatures from `cache/world/northshire.txt` at their real positions, only inside a ring around the player, each one roaming its own area (random wander or a waypoint path) as in WoW Classic, with a long respawn that never fires near the player. Critters, friendly NPCs and the two unmapped monsters are G1b / G1c (their records are counted and skipped here).

**Architecture:** Three pure modules (`roam`, `ring`, `world`) hold the rules and are unit-tested without Bevy; a thin Bevy layer in `npc.rs` (`place` extracted from `spawn`, `ring_tick`, `Roam` in `think`) wires them. The old hand-placed groups in `slice.rs` stay for unattended scripted runs (the HANDOVER's checks assume one lone kobold) and are switched by `WOWSOULS_WORLD`.

**Tech Stack:** Rust 2021, Bevy 0.18.1, the workspace crate `wow-world` (`manifest::parse` -> `World`), `pattern::Rng` (xorshift) for the dice.

Spec: `docs/superpowers/specs/2026-10-09-northshire-quests-design.md` (G1, G2, "Roaming"). Findings this plan is built on (read from the code on 2026-10-09 by an explorer agent, line numbers as of then): `npc::spawn` is `npc.rs:355-443` (the only caller of `slice::spawn_plan()`), `think_step` `npc.rs:200`, `think` `npc.rs:710`, `exclude_bodies` / `move_hostiles` / `follow_bodies` `npc.rs:1099-1136`, `reset` `npc.rs:1162`; leash is measured from the player, not from home; `stand_down` sends any creature more than `HOME_SLACK` 0.6 yd from `Home` back home, so roaming needs a bigger "home" for a wanderer; `npc::spawn` aborts the whole loop on one missing display (`return` where `continue` is meant); the per-frame cost of the creature motors and body-collider exclusions grows with the square of the live creatures.

Rules that bind every task:
- Gates: `cargo fmt --all -- --check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`, tests `DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 --workspace` (wow-souls 290 + the new ones, dsr-cache 51, dsr-formats 51, wscl 4, wow-world 55). Build with `-j 4` only (RAM-tight machine). No commits unless the user asks. Never write into the WoW, Dark Souls or MaNGOS installs (check by `find ... -newer <this plan>` at the end).
- Live checks are unattended (`docs/HANDOVER.md` "Build, run, verify"): never let a test run grab the user's mouse. `WOWSOULS_CHECK`, `WOWSOULS_TRACE=1`, `WOWSOULS_SHOT` as usual.
- Existing behaviour stays: with `WOWSOULS_WORLD=0` (and in unattended runs without `WOWSOULS_WORLD=1`) the game spawns exactly what it spawns today.

## File structure

| File | Responsibility |
|---|---|
| `crates/wow-souls/Cargo.toml`, `Cargo.toml` (workspace deps) | `wow-world` dependency |
| `crates/wow-souls/src/roam.rs` (new) | pure: `Area`, `Node`, `Walk`, `step`, `home_distance`, `slack` |
| `crates/wow-souls/src/ring.rs` (new) | pure: `Ring`, `State`, `Rec`, `plan` (what to spawn and despawn this tick) |
| `crates/wow-souls/src/world.rs` (new) | the manifest as a `Book` of `Spot`s, `Kind` mapping, `SpawnId`, the `ring_tick` / `note_deaths` systems and the plugin |
| `crates/wow-souls/src/npc.rs` | `place` extracted from `spawn`; `Roam` in `think`; the sim radius gate; `reset` clears respawn timers |
| `crates/wow-souls/src/species.rs` | `SpeciesDef::walk_speed()`; no table changes |
| `crates/wow-souls/src/main.rs` | `mod roam; mod ring; mod world;` and the plugin |
| `docs/HANDOVER.md` | the step's entry, the architecture rows, the env switches |

Switches: `WOWSOULS_WORLD=0|1` (0: legacy groups; 1: the manifest even in unattended runs; default: manifest when attended, legacy when unattended), `WOWSOULS_WORLD_FILE=<path>` (default `cache/world/northshire.txt`), `WOWSOULS_RING=<spawn>,<despawn>` (yards, default 100,130), `WOWSOULS_SIM=<yd>` (default 70), `WOWSOULS_RESPAWN=<min seconds>` (default 600).

---

### Task 1: `roam.rs`, the pure walking rules

**Files:**
- Create: `crates/wow-souls/src/roam.rs`
- Modify: `crates/wow-souls/src/main.rs` (add `mod roam;`)

Positions here are Bevy ground coordinates `[x, z]` (yards), so the module has no Bevy dependency.

- [ ] **Step 1: Write the module with its tests**

```rust
//! Where a creature walks while nothing is after it: nothing stands still unless the database says
//! so (`creature.movement_type`): type 0 stands at its post, type 1 wanders at random inside
//! `wander_distance` yards of its spawn point, type 2 walks the waypoints of its path and loops.

use crate::pattern::Rng;
use std::sync::Arc;

/// How close counts as arrived (yd).
pub const ARRIVE: f32 = 0.6;
/// A wanderer pauses this long (s) at each stop: `PAUSE.0 ..= PAUSE.1`.
pub const PAUSE: (f32, f32) = (3.0, 9.0);
/// A new wander target is at least this far (yd) from where the creature stands, if the area allows.
const MIN_LEG: f32 = 2.0;
/// How far a path walker may stray from its line before it counts as lost (yd).
pub const PATH_SLACK: f32 = 2.5;

/// One waypoint: where (Bevy ground `[x, z]`) and how long to wait there (s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    pub at: [f32; 2],
    pub wait: f32,
}

/// The area a creature belongs to.
#[derive(Debug, Clone, PartialEq)]
pub enum Area {
    Stand,
    /// Random wander inside this radius (yd) of its home.
    Wander(f32),
    /// Walk these nodes in order, and from the last back to the first.
    Path(Arc<[Node]>),
}

/// The walker's own bookkeeping.
#[derive(Debug, Clone, Default)]
pub struct Walk {
    target: Option<[f32; 2]>,
    pause: f32,
    next: usize,
}

fn unit(rng: &mut Rng) -> f32 {
    rng.next_u32() as f32 / 4_294_967_296.0
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// One frame of roaming: the ground point to walk toward, or `None` to stand (a pause, a post).
pub fn step(
    w: &mut Walk,
    area: &Area,
    home: [f32; 2],
    pos: [f32; 2],
    dt: f32,
    rng: &mut Rng,
) -> Option<[f32; 2]> {
    match area {
        Area::Stand => None,
        Area::Wander(radius) if *radius <= 0.0 => None,
        Area::Wander(radius) => {
            if w.pause > 0.0 {
                w.pause -= dt;
                return None;
            }
            if let Some(t) = w.target {
                if dist(pos, t) > ARRIVE {
                    return Some(t);
                }
                w.target = None;
                w.pause = PAUSE.0 + (PAUSE.1 - PAUSE.0) * unit(rng);
                return None;
            }
            // Pick a point inside the area, preferably a real walk away.
            let mut pick = home;
            for _ in 0..6 {
                let angle = unit(rng) * std::f32::consts::TAU;
                let r = unit(rng).sqrt() * radius;
                pick = [home[0] + r * angle.cos(), home[1] + r * angle.sin()];
                if dist(pos, pick) >= MIN_LEG.min(*radius) {
                    break;
                }
            }
            w.target = Some(pick);
            Some(pick)
        }
        Area::Path(nodes) => {
            if nodes.is_empty() {
                return None;
            }
            if w.pause > 0.0 {
                w.pause -= dt;
                return None;
            }
            let i = w.next % nodes.len();
            let node = nodes[i];
            if dist(pos, node.at) > ARRIVE {
                return Some(node.at);
            }
            w.pause = node.wait;
            w.next = (i + 1) % nodes.len();
            if w.pause > 0.0 {
                None
            } else {
                Some(nodes[w.next].at)
            }
        }
    }
}

/// How far a creature at `pos` is from where it belongs (yd) and the ground point to walk to when it
/// has to go back: 0 inside its area. A wanderer is at home anywhere in its radius, a path walker
/// anywhere within [`PATH_SLACK`] of the polyline through its nodes (closed from the last node to the
/// first); a creature at a post stands at `home`.
pub fn home_distance(area: &Area, home: [f32; 2], pos: [f32; 2]) -> (f32, [f32; 2]) {
    match area {
        Area::Stand => (dist(pos, home), home),
        Area::Wander(radius) => ((dist(pos, home) - radius).max(0.0), home),
        Area::Path(nodes) if nodes.is_empty() => (dist(pos, home), home),
        Area::Path(nodes) => {
            let mut best = (f32::MAX, nodes[0].at);
            for i in 0..nodes.len() {
                let a = nodes[i].at;
                let b = nodes[(i + 1) % nodes.len()].at;
                let p = nearest_on_segment(a, b, pos);
                let d = dist(pos, p);
                if d < best.0 {
                    best = (d, p);
                }
            }
            ((best.0 - PATH_SLACK).max(0.0), best.1)
        }
    }
}

fn nearest_on_segment(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> [f32; 2] {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 < 1.0e-6 {
        return a;
    }
    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0);
    [a[0] + ab[0] * t, a[1] + ab[1] * t]
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 30.0;

    /// Walk a creature for `secs` seconds at `speed` yd/s toward whatever `step` asks.
    fn simulate(area: &Area, home: [f32; 2], secs: f32, speed: f32) -> (Vec<[f32; 2]>, Walk) {
        let mut rng = Rng(7);
        let mut walk = Walk::default();
        let mut pos = home;
        let mut trail = vec![pos];
        for _ in 0..(secs / DT) as usize {
            if let Some(t) = step(&mut walk, area, home, pos, DT, &mut rng) {
                let d = dist(pos, t);
                let stride = (speed * DT).min(d);
                pos = [pos[0] + (t[0] - pos[0]) / d * stride, pos[1] + (t[1] - pos[1]) / d * stride];
            }
            trail.push(pos);
        }
        (trail, walk)
    }

    #[test]
    fn a_post_never_moves() {
        let (trail, _) = simulate(&Area::Stand, [10.0, 20.0], 60.0, 2.5);
        assert!(trail.iter().all(|p| *p == [10.0, 20.0]));
        let (trail, _) = simulate(&Area::Wander(0.0), [10.0, 20.0], 60.0, 2.5);
        assert!(trail.iter().all(|p| *p == [10.0, 20.0]), "radius 0 is a post");
    }

    #[test]
    fn a_wanderer_roams_inside_its_radius_and_pauses() {
        let home = [100.0, -50.0];
        let (trail, _) = simulate(&Area::Wander(7.0), home, 600.0, 2.5);
        let farthest = trail.iter().map(|p| dist(*p, home)).fold(0.0, f32::max);
        assert!(farthest <= 7.0 + 0.01, "strayed {farthest} yd");
        assert!(farthest > 3.0, "it did walk (farthest {farthest})");
        // It stands for stretches: some consecutive frames are identical.
        let still = trail.windows(2).filter(|w| w[0] == w[1]).count();
        assert!(still > trail.len() / 4, "pauses: {still} of {}", trail.len());
    }

    #[test]
    fn a_path_walker_visits_its_nodes_in_order_waits_and_loops() {
        let nodes: Arc<[Node]> = Arc::from(vec![
            Node { at: [0.0, 0.0], wait: 2.0 },
            Node { at: [10.0, 0.0], wait: 0.0 },
            Node { at: [10.0, 10.0], wait: 1.0 },
        ]);
        let area = Area::Path(nodes.clone());
        let (trail, _) = simulate(&area, [0.0, 0.0], 40.0, 2.5);
        // Every node is reached.
        for n in nodes.iter() {
            assert!(trail.iter().any(|p| dist(*p, n.at) <= ARRIVE + 0.01), "never reached {:?}", n.at);
        }
        // It loops: after the last node it is back near the first.
        let last = trail.iter().rposition(|p| dist(*p, nodes[2].at) <= ARRIVE + 0.01).unwrap();
        assert!(
            trail[last..].iter().any(|p| dist(*p, nodes[0].at) <= ARRIVE + 0.01),
            "it did not walk back to the first node"
        );
        // It waits 2 s at the first node: at least 60 identical frames in a row somewhere.
        let mut run = 0;
        let mut longest = 0;
        for w in trail.windows(2) {
            run = if w[0] == w[1] { run + 1 } else { 0 };
            longest = longest.max(run);
        }
        assert!(longest >= 55, "longest pause {longest} frames");
    }

    #[test]
    fn an_empty_path_stands() {
        let (trail, _) = simulate(&Area::Path(Arc::from(Vec::new())), [1.0, 1.0], 10.0, 2.5);
        assert!(trail.iter().all(|p| *p == [1.0, 1.0]));
    }

    #[test]
    fn home_distance_is_zero_inside_the_area_and_points_back_outside_it() {
        let home = [0.0, 0.0];
        assert_eq!(home_distance(&Area::Stand, home, [3.0, 4.0]).0, 5.0);
        let (d, back) = home_distance(&Area::Wander(7.0), home, [3.0, 4.0]);
        assert_eq!((d, back), (0.0, home));
        let (d, _) = home_distance(&Area::Wander(7.0), home, [9.0, 12.0]);
        assert!((d - 8.0).abs() < 1e-4, "{d}");
        let nodes: Arc<[Node]> = Arc::from(vec![
            Node { at: [0.0, 0.0], wait: 0.0 },
            Node { at: [20.0, 0.0], wait: 0.0 },
        ]);
        // Beside the middle of the line, 1 yd off: inside the slack.
        assert_eq!(home_distance(&Area::Path(nodes.clone()), home, [10.0, 1.0]).0, 0.0);
        // 10 yd off the line: it must walk back to the point on the line below it.
        let (d, back) = home_distance(&Area::Path(nodes), home, [10.0, 10.0]);
        assert!((d - (10.0 - PATH_SLACK)).abs() < 1e-3, "{d}");
        assert!((back[0] - 10.0).abs() < 1e-3 && back[1].abs() < 1e-3, "{back:?}");
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls roam::`
Expected: 5 passed. (If the `simulate` helper's division by `d` hits 0 when `pos == t`, the creature arrived: guard `if d > 1.0e-6`.)

- [ ] **Step 3: Gates** (`cargo fmt --all -- --check`, `cargo clippy -j 4 -p wow-souls --all-targets -- -D warnings`).

---

### Task 2: `ring.rs`, the pure spawn / despawn / respawn rules

**Files:**
- Create: `crates/wow-souls/src/ring.rs`
- Modify: `crates/wow-souls/src/main.rs` (`mod ring;`)

- [ ] **Step 1: Write the module with its tests**

```rust
//! Which creatures exist this tick: a ring around the player. A record spawns inside the spawn
//! radius, despawns beyond the despawn radius (the gap is hysteresis so a walk along the edge does
//! not flicker), and a killed creature comes back only after its respawn time AND only while the
//! player is at least [`CLEAR`] yards away: a cave round trip never meets a respawn behind it.

/// The two radii (yd).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ring {
    pub spawn: f32,
    pub despawn: f32,
}

/// WoW's object visibility is about 90 yd; the despawn radius sits well inside the engine's 267 yd
/// minimum terrain window (`farclip` 177), so a spawn always has ground under it.
pub const RING: Ring = Ring { spawn: 100.0, despawn: 130.0 };
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
/// returning, flinching). At most `max_spawn` records are spawned per tick.
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
            State::Absent => {
                let due = r.until.is_none_or(|u| now >= u && d >= CLEAR);
                if d <= ring.spawn && due {
                    want.push((d, i));
                }
            }
            State::Live => {
                // A corpse that is due is replaced, but never in front of the player.
                let corpse_due = r.until.is_some_and(|u| now >= u && d >= CLEAR);
                if (d > ring.despawn && !keep(i)) || corpse_due {
                    out.despawn.push(i);
                }
            }
        }
    }
    want.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.spawn = want.into_iter().take(max_spawn).map(|(_, i)| i).collect();
    out
}

/// How long a creature stays dead (s): four times its database respawn time, never less than
/// `floor`. The database's times are minutes at most; a Souls player needs to be able to walk into a
/// cave and out again.
pub fn respawn_delay(db_max_secs: u32, floor: f32) -> f32 {
    (db_max_secs as f32 * 4.0).max(floor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(x: f32, state: State, until: Option<f32>) -> Rec {
        Rec { at: [x, 0.0], state, until }
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
        let recs = [rec(110.0, State::Live, None), rec(110.0, State::Absent, None)];
        let p = plan(&recs, [0.0, 0.0], 0.0, RING, 9, &nobody);
        assert_eq!(p, Plan::default());
        // Beyond the despawn radius the live one goes, unless it is kept.
        let far = [rec(140.0, State::Live, None)];
        assert_eq!(plan(&far, [0.0, 0.0], 0.0, RING, 9, &nobody).despawn, vec![0]);
        assert!(plan(&far, [0.0, 0.0], 0.0, RING, 9, &|_| true).despawn.is_empty());
    }

    #[test]
    fn a_killed_creature_waits_for_its_time_and_for_the_player_to_be_away() {
        // Dead, absent, due at t=100.
        let dead = [rec(80.0, State::Absent, Some(100.0))];
        assert!(plan(&dead, [0.0, 0.0], 50.0, RING, 9, &nobody).spawn.is_empty(), "not yet");
        assert_eq!(plan(&dead, [0.0, 0.0], 150.0, RING, 9, &nobody).spawn, vec![0], "due and 80 yd away");
        let near = [rec(30.0, State::Absent, Some(100.0))];
        assert!(plan(&near, [0.0, 0.0], 150.0, RING, 9, &nobody).spawn.is_empty(), "due but the player is 30 yd away");
    }

    #[test]
    fn a_due_corpse_is_replaced_only_out_of_the_players_sight_distance() {
        let corpse_near = [rec(30.0, State::Live, Some(100.0))];
        assert!(plan(&corpse_near, [0.0, 0.0], 150.0, RING, 9, &nobody).despawn.is_empty());
        let corpse_away = [rec(80.0, State::Live, Some(100.0))];
        assert_eq!(plan(&corpse_away, [0.0, 0.0], 150.0, RING, 9, &nobody).despawn, vec![0]);
        // A corpse that is not due stays even far away only until the despawn radius.
        let corpse_early = [rec(80.0, State::Live, Some(500.0))];
        assert!(plan(&corpse_early, [0.0, 0.0], 150.0, RING, 9, &nobody).despawn.is_empty());
    }

    #[test]
    fn a_cave_round_trip_meets_no_respawn() {
        // Killed at t=0, back after at least 600 s: the player returns at t=240 and nothing is there.
        let delay = respawn_delay(270, 600.0);
        assert_eq!(delay, 1080.0);
        let dead = [rec(20.0, State::Absent, Some(delay))];
        assert!(plan(&dead, [0.0, 0.0], 240.0, RING, 9, &nobody).spawn.is_empty());
        assert_eq!(respawn_delay(30, 600.0), 600.0, "the floor");
    }
}
```

- [ ] **Step 2: Run** `cargo test -j 4 -p wow-souls ring::` — expected 5 passed. If `Option::is_none_or` is not stable in the toolchain (Rust 1.98: it is), replace by `map_or(true, ...)`.
- [ ] **Step 3: Gates.**

---

### Task 3: `world.rs` (pure part), the manifest as a Book of spots

**Files:**
- Create: `crates/wow-souls/src/world.rs`
- Modify: `crates/wow-souls/Cargo.toml` (`wow-world.workspace = true`), `Cargo.toml` workspace (`wow-world = { path = "crates/wow-world" }`), `crates/wow-souls/src/main.rs` (`mod world;`)

- [ ] **Step 1: The pure half with its tests**

```rust
//! The valley as a book of spawn spots read from `cache/world/northshire.txt` (the manifest of
//! `wow-world`), and the rules that turn a creature entry into a kind of creature.

use crate::ring::{Rec, State};
use crate::roam::{Area, Node};
use std::collections::HashMap;
use std::sync::Arc;
use wow_world::model::{Creature, World};

/// What a spawn becomes in the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A hostile of this species (`species::by_name`).
    Foe(&'static str),
    /// A critter (G1b).
    Critter,
    /// A friendly NPC, guard, peasant or horse (G1c).
    Friendly,
    /// A hostile the roster does not have yet (logged, not spawned): Mine Spider, Kobold Miner.
    Unmapped,
}

/// Creature entry -> species (the kit rows built in E / E2; displays and numbers stay the species').
pub fn foe_of(entry: u32) -> Option<&'static str> {
    Some(match entry {
        80 => "kobold",
        6 => "vermin",
        257 => "worker",
        69 => "wolf",
        299 => "young_wolf",
        38 => "thug",
        103 => "garrick",
        _ => return None,
    })
}

/// Entries with no `npc_flags` that are still friendly: the valley's guards, its peasants and the
/// White Stallion.
const FRIENDLY_EXTRA: [u32; 3] = [1642, 11260, 5403];

pub fn kind_of(c: &Creature) -> Kind {
    if let Some(name) = foe_of(c.entry) {
        Kind::Foe(name)
    } else if c.kind == 8 {
        Kind::Critter
    } else if c.npc_flags != 0 || FRIENDLY_EXTRA.contains(&c.entry) {
        Kind::Friendly
    } else {
        Kind::Unmapped
    }
}

/// One spawn spot, in the game's own terms.
#[derive(Debug, Clone)]
pub struct Spot {
    pub guid: u32,
    pub entry: u32,
    pub kind: Kind,
    /// WoW coordinates (x north, y west, z up): the ground height is a seed, the spawn snaps it.
    pub wow: [f32; 3],
    /// Bevy yaw (the database's orientation is the same angle).
    pub facing: f32,
    pub area: Area,
    /// The database's longest respawn time (s).
    pub respawn_secs: u32,
}

/// Bevy ground `[x, z]` of a WoW point.
pub fn flat(wow: [f32; 3]) -> [f32; 2] {
    [-wow[1], -wow[0]]
}

pub struct Book {
    pub spots: Vec<Spot>,
    pub recs: Vec<Rec>,
}

impl Book {
    pub fn from_world(w: &World) -> Book {
        let templates: HashMap<u32, &Creature> = w.creatures.iter().map(|c| (c.entry, c)).collect();
        let mut paths: HashMap<u32, Vec<&wow_world::model::Waypoint>> = HashMap::new();
        for p in &w.waypoints {
            paths.entry(p.guid).or_default().push(p);
        }
        let mut spots = Vec::new();
        for s in &w.spawns {
            let Some(t) = templates.get(&s.entry) else { continue };
            let area = match s.movement {
                2 => match paths.get(&s.guid) {
                    Some(points) if !points.is_empty() => {
                        let mut nodes: Vec<&&wow_world::model::Waypoint> = points.iter().collect();
                        nodes.sort_by_key(|p| (p.path_id, p.point));
                        Area::Path(Arc::from(
                            nodes
                                .into_iter()
                                .map(|p| Node { at: flat(p.pos), wait: p.wait_ms as f32 / 1000.0 })
                                .collect::<Vec<_>>(),
                        ))
                    }
                    _ => Area::Stand,
                },
                1 if s.wander > 0.0 => Area::Wander(s.wander),
                _ => Area::Stand,
            };
            spots.push(Spot {
                guid: s.guid,
                entry: s.entry,
                kind: kind_of(t),
                wow: s.pos,
                facing: s.orientation,
                area,
                respawn_secs: s.respawn_max.max(s.respawn_min),
            });
        }
        let recs = spots
            .iter()
            .map(|s| Rec { at: flat(s.wow), state: State::Absent, until: None })
            .collect();
        Book { spots, recs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::model::*;

    fn creature(entry: u32, kind: u32, flags: u32) -> Creature {
        Creature {
            entry, name: format!("c{entry}"), subname: String::new(), level_min: 1, level_max: 1, faction: 0,
            npc_flags: flags, display: [0; 4], scale: [0.0; 4], kind, rank: 0, detection_range: 0.0,
            call_for_help_range: 0.0, leash_range: 0.0, speed_walk: 0.0, speed_run: 0.0, loot_id: 0,
            equipment_id: 0, vendor_id: 0, trainer_type: 0, trainer_class: 0, movement_type: 0, civilian: false,
            health_multiplier: 1.0, damage_multiplier: 1.0, unit_class: 0,
        }
    }

    fn spawn(guid: u32, entry: u32, x: f32, y: f32, movement: u8, wander: f32) -> Spawn {
        Spawn {
            guid, entry, alt: [0; 4], pos: [x, y, 80.0], orientation: 1.5, respawn_min: 120, respawn_max: 270,
            wander, movement, area: 9, event: false,
        }
    }

    #[test]
    fn entries_map_to_kinds() {
        assert_eq!(kind_of(&creature(38, 7, 0)), Kind::Foe("thug"));
        assert_eq!(kind_of(&creature(103, 7, 0)), Kind::Foe("garrick"));
        assert_eq!(kind_of(&creature(721, 8, 0)), Kind::Critter);
        assert_eq!(kind_of(&creature(197, 7, 3)), Kind::Friendly, "McBride has npc flags");
        assert_eq!(kind_of(&creature(1642, 7, 0)), Kind::Friendly, "a guard has none");
        assert_eq!(kind_of(&creature(43, 1, 0)), Kind::Unmapped, "the Mine Spider");
    }

    #[test]
    fn every_foe_entry_names_a_species_of_the_roster() {
        for entry in [80, 6, 257, 69, 299, 38, 103] {
            let name = foe_of(entry).unwrap();
            assert!(crate::species::by_name(name).is_some(), "{name}");
        }
    }

    #[test]
    fn the_book_reads_areas_facing_and_respawn_from_the_world() {
        let mut w = World::default();
        w.creatures = vec![creature(38, 7, 0), creature(721, 8, 0)];
        w.spawns = vec![
            spawn(1, 38, -9056.0, -460.0, 0, 0.0),
            spawn(2, 721, -9000.0, -300.0, 1, 10.0),
            spawn(3, 38, -9010.0, -250.0, 2, 0.0),
            spawn(4, 999, 0.0, 0.0, 0, 0.0),
        ];
        w.waypoints = vec![
            Waypoint { guid: 3, point: 2, pos: [-9010.0, -240.0, 80.0], orientation: 0.0, wait_ms: 0, script_id: 0, path_id: 0 },
            Waypoint { guid: 3, point: 1, pos: [-9010.0, -250.0, 80.0], orientation: 0.0, wait_ms: 4000, script_id: 0, path_id: 0 },
        ];
        let b = Book::from_world(&w);
        assert_eq!(b.spots.len(), 3, "spawn 4 has no template and is dropped");
        assert_eq!(b.spots[0].area, Area::Stand);
        assert_eq!(b.spots[1].area, Area::Wander(10.0));
        let Area::Path(nodes) = &b.spots[2].area else { panic!("a path") };
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0], Node { at: [250.0, 9010.0], wait: 4.0 }, "sorted by point; WoW (x, y) is Bevy [-y, -x]");
        assert_eq!(b.spots[0].respawn_secs, 270);
        assert_eq!(b.recs.len(), 3);
        assert_eq!(b.recs[0].at, [460.0, 9056.0]);
        assert!((b.spots[0].facing - 1.5).abs() < 1e-6);
    }
}
```

(If a `Creature` field in `crates/wow-world/src/model.rs` differs from the constructor above, follow the model; it is the contract.)

- [ ] **Step 2: Run** `cargo test -j 4 -p wow-souls world::` — expected 3 passed. Then gates.
- [ ] **Step 3: Install test** (skipped without the file): the real manifest gives 334 spots, 7 foe entries cover 276 of them (77 + 43 + 37 + 36 + 33 + 17 + 1... assert the actual per-kind counts the importer printed: Foe sum, Critter 53 (41 + 9 + 3), Friendly the rest, Unmapped 5 (4 spiders + 1 miner)), and the 18 walkers have `Area::Path`. Write the test against `cache/world/northshire.txt` relative to `CARGO_MANIFEST_DIR/../..`; skip with a message when the file is missing.

---

### Task 4: Wire it into the game (npc.rs, world.rs systems)

**Files:** Modify `crates/wow-souls/src/npc.rs`, `species.rs`, `world.rs`, `main.rs`. This is the delicate task: do it in the small steps below and run the gates after each.

- [ ] **Step 1: `SpeciesDef::walk_speed`** in `species.rs`:

```rust
impl SpeciesDef {
    /// How fast it walks when nothing is after it (yd/s): the chase speed scaled down the way WoW's
    /// walk is slower than its run, never slower than a crawl and never faster than the player's walk.
    pub fn walk_speed(&self) -> f32 {
        (self.chase_speed * 0.45).clamp(0.9, crate::motor::WALK_SPEED)
    }
}
```
Test: every roster species walks between 0.9 and 2.5 yd/s.

- [ ] **Step 2: Extract `place`.** Move the body of the `for` loop in `npc::spawn` (npc.rs:367-442) into

```rust
/// One creature of `def` standing at `spot` (WoW coords), facing `facing` (Bevy yaw), belonging to
/// `area`; `id` links it to its spawn record.
#[allow(clippy::too_many_arguments)]
pub(crate) fn place(
    commands: &mut Commands,
    def: &'static crate::species::SpeciesDef,
    spot: [f32; 3],
    facing: Option<f32>,
    area: crate::roam::Area,
    id: Option<usize>,
    roster: Option<&Roster>,
    catalogs: &Catalogs,
    asset_server: &AssetServer,
    collide: &WorldCollision,
    epoch: &mut ColliderEpoch,
) -> Option<Entity>
```

(`facing: None` = toward the slice anchor as today.) Change the early `return` on a missing display to `return None` (the caller continues), and the creature's bundle gains `crate::roam::Walk::default()`, `Roam(area)` (a new `#[derive(Component)] pub struct Roam(pub Area)`), and `SpawnId(id)` when `id` is `Some` (`#[derive(Component)] pub struct SpawnId(pub usize);` in `world.rs`). `spawn` (legacy) becomes a loop calling `place(..., None, Area::Stand, None, ...)`. Run the existing tests: nothing changes yet.

- [ ] **Step 3: Roaming in `think`.** Add to the query `&Roam, &mut crate::roam::Walk`, and to `Sense` nothing new; compute `home_dist` and the walk-back point through `roam::home_distance`:

```rust
let flat2 = |v: Vec3| [v.x, v.z];
let (home_gap, back_to) =
    crate::roam::home_distance(&roam.0, flat2(home.feet), flat2(xf.translation));
// ... in Sense: home_dist: home_gap,
```

`Mind::Return` walks toward `back_to` (as a Vec3 direction `Vec3::new(back_to[0] - xf.translation.x, 0.0, back_to[1] - xf.translation.z)`) at `species.0.walk_speed().max(RETURN_SPEED)`... keep `RETURN_SPEED` for the walk home (a creature that lost the player hurries back), and `Mind::Idle` becomes:

```rust
Mind::Idle => {
    let now = flat2(xf.translation);
    match crate::roam::step(&mut walk, &roam.0, flat2(home.feet), now, dt, &mut rng.0) {
        Some(t) => {
            let d = Vec3::new(t[0] - now[0], 0.0, t[1] - now[1]);
            (d, species.0.walk_speed(), d)
        }
        // At its post (or paused): it turns back to the way it was placed.
        None => (Vec3::ZERO, 0.0, Quat::from_rotation_y(home.facing) * Vec3::NEG_Z),
    }
}
```

A paused wanderer must not snap to `home.facing`: only a `Stand` area turns back to its post; for the others pass `look = Vec3::ZERO` (keep the current facing). Aggro resets `walk` (`Walk::default()`) when the mind leaves `Idle`.

Tests (pure, in `npc.rs` tests): `think_step` with `home_dist: 0.0` while roaming stays `Idle`; the existing tests keep passing.

- [ ] **Step 4: The sim radius.** `think` and `move_hostiles` skip a creature whose ground distance to the player exceeds `ring::SIM` unless `brain.mind != Mind::Idle`; `think` resets `*intent` before the skip (it already sets `MoveIntent::default()` first). `exclude_bodies` only lists bodies of creatures inside `ring::SIM` of the player (query `(&BodyCollider, &Transform)` plus the avatar's transform). Read `WOWSOULS_SIM` once in `ring.rs` (`pub fn sim() -> f32`).

- [ ] **Step 5: The ring system** (`world.rs`, Bevy half):

```rust
#[derive(Resource)]
pub struct Book { /* the pure Book plus */ }  // keep `Book` pure; the resource wraps it:
#[derive(Resource)]
pub struct WorldBook { pub book: Book, pub ring: Ring, pub floor_secs: f32 }

#[derive(Component)]
pub struct SpawnId(pub usize);
```

`world::plugin` reads the manifest at `Startup` (`std::fs::read_to_string(path)` then `wow_world::manifest::parse`), inserts `WorldBook`, and logs `world: N spots (Foe a, Critter b, Friendly c, Unmapped d)`; it runs only when `world_on()` says so (the switch above). `ring_tick` (Update, in `Phase::Playing`, `.before(crate::actor::build)` if that is the system's public name, otherwise `.before` the actor set; `Phase::Playing` entry already exists) every 0.25 s real time:

1. player ground position from the `Avatar` transform; `now` from `Time<Real>`;
2. `ring::plan(&recs, player, now, ring, 3, &|i| engaged(i))` where `engaged(i)` looks up the live entity of record `i` (keep a `Vec<Option<Entity>>` in the resource) and is true when its `Brain.mind != Mind::Idle` or its `Reaction` is `Flinch`;
3. despawn: `commands.entity(e).despawn()` and the creature's `BodyCollider` entity (not a child), `epoch.bump()`, record `state = Absent`, `entity = None`;
4. spawn: for each index, only `Kind::Foe(name)` goes through `npc::place(...)` (`species::by_name(name)`, `Some(spot.facing)`, `spot.area.clone()`, `Some(i)`); the other kinds are skipped (counted once in the log); `place` returning `None` leaves the record `Absent` and logs once per record; record `state = Live`, `until = None`. Ground snap: if `snap_to_ground` fails (no collider there yet) `place` must report it so the record stays `Absent` and is retried next tick: add a `strict_ground: bool` parameter (true for ring spawns, false for the legacy groups).

`note_deaths` (Update): for each `npc::Slain { who, .. }` message, find `SpawnId(i)` on `who` and set `recs[i].until = Some(now + ring::respawn_delay(spot.respawn_secs, floor_secs))` (`WOWSOULS_RESPAWN` seconds floor, default 600).

`npc::reset` (F6, player death): it already restores live creatures to their home; additionally clear every `until` and set it `None` (so corpses are replaced at once), implemented by a `Respawned` reader in `world.rs`: for every record with `until.is_some()` and a live entity, despawn the corpse (as step 5.3); the next tick spawns it fresh.

- [ ] **Step 6: Gate switch.** `world_on()`: `WOWSOULS_WORLD=0` off, `=1` on, otherwise on when `!run_mode::unattended()`. When on, the legacy `npc::spawn` is not added; when off, `world::plugin` adds nothing. A unit test for the switch table.

- [ ] **Step 7: Gates**, then Task 5.

---

### Task 5: Verify live and measure

- [ ] **Step 1: A scripted walk through the valley**, unattended: `WOWSOULS_WORLD=1 WOWSOULS_CHECK=60 WOWSOULS_TRACE=1 WOWSOULS_DRIVE=-1,0 WOWSOULS_SHOT=... timeout 120 ./target/debug/wow-souls.exe` (drive west along the meadow, then another run east toward the Echo Ridge, then south to the vineyard with `WOWSOULS_AT=-9000,-250`). Expected log lines: `world: 334 spots (...)`, `ring: spawned N, despawned M (live L)` per tick that changes, no ECS fault, `npc: kobold ...` lines for the creatures met. Check by trace that a wandering creature moves (its `kobold t=` position changes while `Mind::Idle`) and stays inside its radius from its spawn; that a path walker (the guards at `x -9047, y -101`) walks its nodes; that a creature dragged home returns to its area.
- [ ] **Step 2: Respawn**: kill one (`WOWSOULS_ATTACK`), walk 130+ yd away and back before `WOWSOULS_RESPAWN=30` s: not there; after: there. (`WOWSOULS_RESPAWN=30` shortens the floor for the test; the DB-times-four rule still applies, so use a creature with a short DB time or lower the multiplier with a second env `WOWSOULS_RESPAWN_X`; the plan's default is x4.)
- [ ] **Step 3: Cost**: with `--profile ship` (`cargo build -j 4 --profile ship -p wow-souls`, slow) note the live creature count and the frame time at the Abbey, at the Echo Ridge Mine (about 50 spawns) and in the Defias camp (60-80 spawns): `WOWSOULS_TRACE` prints frame time? If it does not, add a one-line frame-time report (average and worst over 5 s) behind `WOWSOULS_TRACE`. If the frame time is bad, do the measured fixes in this order: (1) cap `actor::build` per frame, (2) cache `BodyHits` per model, (3) `AnimParked` outside 45 yd for idle creatures (read `benilla_world::rig_anim` first: does the marker stop the pose evaluation?), (4) a distance pre-reject in `hits::strike`. Each fix gets its own measured before / after in the HANDOVER.
- [ ] **Step 4: Pictures**: three screenshots (the Abbey road with wandering kobolds, the Defias camp, the vineyard) read with the Read tool.
- [ ] **Step 5: Gates (whole workspace), installs untouched, update HANDOVER** (this step's entry, the architecture rows for `roam`, `ring`, `world`, the switches). 

## Self-review against the spec

- G1 "every hostile spawn from the DB becomes a species instance by entry ... wander distance and path": Tasks 1, 3, 4. "Lazy streaming": Tasks 2, 4 (ring). G2 "respawn max(10 min, 4 x DB), only when the player is 60 yd away": `ring::respawn_delay`, `ring::CLEAR`, Task 4 step 5. "Roaming": Task 1 and Task 4 step 3. Critters, friendly NPCs, Mine Spider / Kobold Miner: counted and skipped here, G1b / G1c.
- Names used across tasks: `roam::{Area, Node, Walk, step, home_distance, PATH_SLACK}`, `ring::{Ring, RING, State, Rec, Plan, plan, respawn_delay, CLEAR, SIM}`, `world::{Kind, Spot, Book, kind_of, foe_of, flat, SpawnId, WorldBook}`, `npc::{place, Roam}`, `species::SpeciesDef::walk_speed` are all defined above.
- Known soft spots (decide when reached, report rather than guess): whether a ring spawn's `snap_to_ground` can see colliders at 100 yd (the explorer said the terrain window is at least 267 yd, colliders attach under a 2 ms budget); `AnimParked`'s effect; whether the existing scripted checks in the HANDOVER still hold with `WOWSOULS_WORLD` unset in unattended runs (they must).
