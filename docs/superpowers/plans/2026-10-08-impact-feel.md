# D2a — impact feel Implementation Plan

> **For agentic workers:** Execute inline (the user's usual choice), task by task, checkboxes as you go.
> Spec: `docs/superpowers/specs/2026-10-08-impact-feel-design.md`. Always `-j 4`. Install-dependent tests
> need `DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data"` exported in
> the same shell command. Nothing is committed (the user commits).

**Goal:** Hit-stop and knock-back come from the install's `AtkParam` rows (`hitStopTime`,
`knockbackDist`); a blow shoves the body it strikes; the camera jolts on heavy blows.

**Architecture:** The extractor writes `stop`/`knock` onto every `move`/`attack` manifest line
(`MANIFEST_VERSION` 4). The game carries them on `SwingSpec`/`AttackSpec`, `Hit`/`PlayerHurt`.
`shove.rs` (`Shove` component, eased slide through the existing `MoveIntent`/motor) and `shake.rs`
(trauma resource, offset applied in `camera::seat`) are new, small, pure-core modules.

**Tech Stack:** Rust 1.98, Bevy 0.18.1, the workspace crates `dsr-cache` (GPL extractor), `wow-souls`.

**Deviations from the spec, decided while planning:** `Hit` carries only `knock` (`stop` is used inside
`strike`, so it need not travel); `PlayerHurt` carries `level`, `stop`, `knock` but not a position: `suffer`
finds the direction from the `from` entity's `Transform`.

---

## Task 1: Extractor reads and writes the two numbers

**Files:** modify `crates/dsr-cache/src/params.rs`, `crates/dsr-cache/src/cache.rs`.

- [ ] **Step 1: Failing test.** In `params.rs` tests, in `install_the_shortsword_and_its_moves_read_as_measured`
  after the stamina loop, and in `install_the_ghoul_reads_as_measured` after the attack loop, add:

```rust
        // (judge, hitStopTime s, knockbackDist m): the impact spec's table.
        for (judge, stop, knock) in [
            (0, 0.08, 0.72),
            (10, 0.08, 0.84),
            (20, 0.08, 0.72),
            (100, 0.08, 0.95),
            (110, 0.08, 0.90),
            (540, 0.08, 0.80),
        ] {
            let b = p.behavior(2300, judge).unwrap();
            assert!(
                (b.stop - stop).abs() < 1.0e-6 && (b.knock - knock).abs() < 1.0e-6,
                "judge {judge}: {b:?}"
            );
        }
```
```rust
        // (judge, hitStopTime, knockbackDist): the kobold's blows freeze nothing.
        for (judge, knock) in [
            (100, 0.45),
            (110, 0.45),
            (120, 1.0),
            (130, 1.0),
            (140, 1.37),
            (160, 1.0),
        ] {
            let a = p.npc_attack(20_600, judge).unwrap();
            assert!(
                a.stop == 0.0 && (a.knock - knock).abs() < 1.0e-6,
                "judge {judge}: {a:?}"
            );
        }
```
- [ ] **Step 2:** `DSR_DIR=... cargo test -j 4 -p dsr-cache params` → FAIL (no field `stop`).
- [ ] **Step 3: Implement.** `Behavior` and `NpcAttack` each gain
  `pub stop: f32, pub knock: f32` (doc: `hitStopTime` s, `knockbackDist` DS metres). In `behavior()` add
  `stop: a.float(arow, "hitStopTime")?, knock: a.float(arow, "knockbackDist")?,`; the same in `npc_attack()`.
  In `cache.rs`: `MANIFEST_VERSION` 3 → 4; the `attack` line format ends
  `radius2 {:.2} stop {:.2} knock {:.2}` (args `n.stop, n.knock` appended); the `move` line ends
  `poise {} stop {:.2} knock {:.2}` (args `b.stop, b.knock` appended).
- [ ] **Step 4:** the same test command → PASS; `cargo clippy -j 4 -p dsr-cache --all-targets -- -D warnings`.

## Task 2: The game carries the numbers; hit-stop from the data

**Files:** modify `combat/action.rs`, `dsclip.rs`, `monster.rs`, `hits.rs`, `npc.rs`, `hurt.rs`
(all under `crates/wow-souls/src/`).

- [ ] **Step 1: Failing tests** (they fail to compile until Step 3):
  - `dsclip.rs` tests: `swing_spec(&c, 20.0, 1.35, 2, 0.0, 0.08, 0.72)` (and the two error calls with
    `0.08, 0.72`), assert `(s.stop, s.knock) == (0.08, 0.72)`; `whole_cache` writes
    `... level {} poise 0 stop 0.08 knock 0.72\n` and the load test asserts
    `m.r1[0].stop == 0.08 && m.r1[0].knock == 0.72`; the two `parse_move` negative strings gain
    ` stop 0.08 knock 0.72`; add `assert!(parse_move("move r1_1 a23_3000 judge 0 stamina 20 mult 1.0 level 1 poise 0").is_err(), "stop and knock are required")`.
  - `monster.rs` tests: `MANIFEST` attack lines end `radius2 0.00 stop 0.00 knock 0.45` (small_1) and
    `radius2 0.00 stop 0.00 knock 1.00` (big); `a_whole_kit_loads_with_its_numbers` asserts
    `(s.stop, s.knock) == (0.0, 0.45)` and `k.attacks[1].spec.knock == 1.0`.
  - `combat/action.rs` test fixture `ds_moves()`: the `SwingSpec` literal gains `stop: 0.08, knock: 0.72,`.
- [ ] **Step 2:** `cargo test -j 4 -p wow-souls` → compile FAIL.
- [ ] **Step 3: Implement.**
  - `combat/action.rs` `SwingSpec`: after `poise` add
    `/// Hit-stop (s, hitStopTime) and push (DS metres, knockbackDist) of the blow.\n pub stop: f32,\n pub knock: f32,`;
    `procedural`: `stop: crate::hits::HIT_STOP, knock: 0.0,`.
  - `dsclip.rs`: `CACHE_VERSION` → `"4"`; `MoveLine` gains `stop: f32, knock: f32`; `parse_move` pattern
    ends `"poise", poise, "stop", stop, "knock", knock]` with
    `stop: stop.parse().map_err(|_| bad())?, knock: knock.parse().map_err(|_| bad())?`; doc comment
    `... poise <n> stop <s> knock <m>`; `swing_spec(clip, cost, mult, level, poise, stop, knock)` stores
    both; the call in `load` passes `line.stop, line.knock`.
  - `monster.rs`: `AttackSpec` gains `pub stop: f32, pub knock: f32`; the `attack` line pattern ends
    `"radius2", r2, "stop", stop, "knock", knock]`; the `attacks` tuple gains both (parsed `f32`); the loop
    `for (name, clip, phys, poise, level, r1, r2, stop, knock) in attacks` fills the spec.
  - `hits.rs`: `Hit` gains `pub knock: f32` (doc: the blow's `knockbackDist`, DS metres). `strike`
    writes `knock: swing.spec.knock`, logs `... (stop {:.2} s, knock {:.2} m)`, and freezes only when
    `swing.spec.stop > 0.0`, for `swing.spec.stop` (the constant `HIT_STOP` stays, now `pub` use by
    `SwingSpec::procedural`; `HIT_STOP_SPEED` unchanged).
  - `hurt.rs`: `const HURT_STOP` → `pub const HURT_STOP`; in `suffer`, `stop.freeze(...)` only when
    `hurt.stop > 0.0`, with `hurt.stop`; log lines gain `knock {:.2}`.
  - `npc.rs`: `PlayerHurt` gains `pub level: u8, pub stop: f32, pub knock: f32`; `procedural_attack`
    gains `stop: crate::hurt::HURT_STOP, knock: 0.0`; both `PlayerHurt` writes fill
    `level: spec.level, stop: spec.stop, knock: spec.knock` (kit strike: `a.spec.*`).
- [ ] **Step 4:** `cargo test -j 4 -p wow-souls` → PASS (existing count + the new asserts).

## Task 3: `shove.rs`, the knock-back

**Files:** create `crates/wow-souls/src/shove.rs`; modify `main.rs` (`mod shove;`), `boot.rs`
(`shove::plugin` in the tuple), `controller.rs` (`face_motion`), `npc.rs` (`react`, plugin chain),
`hurt.rs` (`suffer`).

- [ ] **Step 1: Write `shove.rs` with its tests first** (the tests are in the file's `tests` module):

```rust
//! Knock-back: a blow shoves the body it strikes along the way it came, `knockbackDist` metres
//! (Dark Souls params), as a slide of [`SHOVE_TIME`] that is fastest first. The slide goes through the
//! body's `MoveIntent`, so the motor does the move and walls stop it. The flinch clips carry no
//! root motion (measured), so this is the only push a flinching body gets.

use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::controller::PlaySet;
use crate::hurt::Respawned;
use crate::motor::MoveIntent;
use crate::npc::Hostile;
use crate::settle::Phase;

/// How long the slide lasts (s). A tuning value: the data gives a distance, not a time.
pub const SHOVE_TIME: f32 = 0.30;
/// Dark Souls params are in metres, the world in yards.
pub const METRES_TO_YARDS: f32 = 1.0936;

/// A slide in progress on a body.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Shove {
    /// Horizontal, unit.
    dir: Vec3,
    /// Total yards.
    dist: f32,
    /// Seconds in.
    t: f32,
}

/// The fraction of the slide done after `u` (0..=1) of its time: `1 - (1-u)^2`.
pub fn covered(u: f32) -> f32 {
    let rest = 1.0 - u.clamp(0.0, 1.0);
    1.0 - rest * rest
}

impl Shove {
    /// A slide of `metres` along `dir` (flattened); `None` for no distance or no direction.
    pub fn new(dir: Vec3, metres: f32) -> Option<Shove> {
        let dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        (metres > 0.0 && dir != Vec3::ZERO).then_some(Shove {
            dir,
            dist: metres * METRES_TO_YARDS,
            t: 0.0,
        })
    }

    /// Runs the slide on by `dt` seconds; the yards it covers in that step.
    pub fn advance(&mut self, dt: f32) -> f32 {
        let before = covered(self.t / SHOVE_TIME);
        self.t = (self.t + dt.max(0.0)).min(SHOVE_TIME);
        (covered(self.t / SHOVE_TIME) - before) * self.dist
    }

    pub fn done(&self) -> bool {
        self.t >= SHOVE_TIME
    }
}

/// Starts a slide on `e` (a log line says how far); nothing for no distance or no direction.
pub fn start(commands: &mut Commands, e: Entity, who: &str, dir: Vec3, metres: f32) {
    if let Some(s) = Shove::new(dir, metres) {
        info!("shove: {who} {:.2} yd over {SHOVE_TIME:.2} s", s.dist);
        commands.entity(e).insert(s);
    }
}

/// One frame of a slide: the body's intent for this step; whether the slide is over.
fn drive(shove: &mut Shove, intent: &mut MoveIntent, dt: f32) -> bool {
    let step = shove.advance(dt);
    *intent = crate::combat::root_step(shove.dir, step, dt);
    shove.done()
}

/// The player's slide, between reading input and moving the body.
fn push_avatar(
    mut commands: Commands,
    time: Res<Time>,
    mut bodies: Query<(Entity, &mut Shove, &mut MoveIntent), With<Avatar>>,
) {
    for (e, mut shove, mut intent) in &mut bodies {
        if drive(&mut shove, &mut intent, time.delta_secs()) {
            commands.entity(e).remove::<Shove>();
        }
    }
}

/// A creature's slide, between its thinking and its motor step.
pub fn push_hostiles(
    mut commands: Commands,
    time: Res<Time>,
    mut bodies: Query<(Entity, &mut Shove, &mut MoveIntent), With<Hostile>>,
) {
    for (e, mut shove, mut intent) in &mut bodies {
        if drive(&mut shove, &mut intent, time.delta_secs()) {
            commands.entity(e).remove::<Shove>();
        }
    }
}

/// A respawn ends every slide.
fn clear(
    mut commands: Commands,
    mut respawned: MessageReader<Respawned>,
    shoved: Query<Entity, With<Shove>>,
) {
    if respawned.read().count() == 0 {
        return;
    }
    for e in &shoved {
        commands.entity(e).remove::<Shove>();
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        push_avatar
            .after(PlaySet::Intent)
            .before(PlaySet::Motor)
            .run_if(in_state(Phase::Playing)),
    )
    .add_systems(Update, clear.run_if(in_state(Phase::Playing)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slide_covers_its_whole_distance_at_any_frame_rate() {
        for dt in [1.0 / 144.0, 1.0 / 60.0, 1.0 / 30.0, 0.11] {
            let mut s = Shove::new(Vec3::X, 0.72).unwrap();
            let mut total = 0.0;
            let mut frames = 0;
            while !s.done() {
                total += s.advance(dt);
                frames += 1;
                assert!(frames < 1000);
            }
            assert!((total - 0.72 * METRES_TO_YARDS).abs() < 1.0e-4, "dt {dt}: {total}");
        }
    }

    #[test]
    fn the_push_is_fastest_first() {
        let mut s = Shove::new(Vec3::X, 1.0).unwrap();
        let first = s.advance(0.05);
        s.advance(0.15);
        let last = s.advance(0.05);
        assert!(first > 3.0 * last, "{first} vs {last}");
        assert!(covered(0.0) == 0.0 && (covered(1.0) - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn a_shove_needs_a_distance_and_a_way_and_stays_flat() {
        assert!(Shove::new(Vec3::X, 0.0).is_none());
        assert!(Shove::new(Vec3::ZERO, 1.0).is_none());
        assert!(Shove::new(Vec3::Y, 1.0).is_none(), "straight up is no way");
        let s = Shove::new(Vec3::new(3.0, 5.0, 0.0), 1.0).unwrap();
        assert_eq!(s.dir, Vec3::X);
    }

    #[test]
    fn driving_a_slide_asks_the_motor_for_each_step_and_ends_it() {
        let mut s = Shove::new(Vec3::NEG_Z, 0.45).unwrap();
        let mut intent = MoveIntent::default();
        let dt = 1.0 / 60.0;
        let mut moved = 0.0;
        let mut ended = false;
        for _ in 0..40 {
            ended = drive(&mut s, &mut intent, dt);
            assert_eq!(intent.dir, Vec3::NEG_Z);
            moved += intent.speed * dt;
            if ended {
                break;
            }
        }
        assert!(ended);
        assert!((moved - 0.45 * METRES_TO_YARDS).abs() < 1.0e-3, "{moved}");
    }
}
```
- [ ] **Step 2:** add `mod shove;` to `main.rs` (alphabetical, after `mod shot;`... between `shot` and
  `slice`: order `settle, sfx, shake, shot, shove, slice`), `shove::plugin` after `hurt::plugin` in the `boot.rs`
  tuple. `cargo test -j 4 -p wow-souls shove` → PASS.
- [ ] **Step 3: Wire it.**
  - `controller.rs` `face_motion`: the avatar query tuple gains `Option<&crate::shove::Shove>` as its last
    element (`let Ok((mut xf, m, intent, action, shoved)) = ...`); the guard becomes
    `if !m.grounded || shoved.is_some() || action.is_some_and(|a| a.evading().is_some())`. Doc: a shove
    never turns the body.
  - `npc.rs` plugin chain: `think, crate::shove::push_hostiles, exclude_bodies, move_hostiles, ...`.
    `react`: in the flinch branch after `*reaction = Reaction::Flinch(len);` add
    `crate::shove::start(&mut commands, hit.target, "kobold", away, hit.knock);`.
  - `hurt.rs` `suffer`: add parameters `mut commands: Commands`, `foes: Query<&Transform, Without<Avatar>>`;
    the avatar query gains `Entity, &Transform` as its first two items; in the non-lethal branch after
    `action.stagger(STAGGER);` add
    `if let Ok(foe) = foes.get(hurt.from) { crate::shove::start(&mut commands, me, "player", body.translation - foe.translation, hurt.knock); }`;
    add `#[allow(clippy::too_many_arguments)]` to `suffer`.
- [ ] **Step 4:** `cargo test -j 4 -p wow-souls` and `cargo clippy -j 4 -p wow-souls --all-targets -- -D warnings` clean.

## Task 4: `shake.rs`, the camera jolt

**Files:** create `crates/wow-souls/src/shake.rs`; modify `main.rs` (`mod shake;`), `boot.rs`
(`shake::plugin`), `camera.rs` (`seat`).

- [ ] **Step 1: Write `shake.rs` with its tests:**

```rust
//! Camera shake (ours: the blow params have no shake value and benilla's `camera_shake.rs` plays only
//! WoW's footstep and spell presets). A `trauma` in `0..=1` is kicked up by heavy blows and decays in
//! real time; the camera's offset is a deterministic sum of sines scaled by `trauma^2`.
//! `WOWSOULS_SHAKE=0` turns it off.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::hits::Hit;
use crate::npc::PlayerHurt;

/// Trauma lost per second: a full kick is gone in 0.4 s.
pub const DECAY: f32 = 2.5;
/// The turn at full trauma (rad, 1.2 deg) and the shift (yd).
const MAX_TURN: f32 = 0.0209;
const MAX_SHIFT: f32 = 0.04;

#[derive(Resource, Debug, Default)]
pub struct Shake {
    pub trauma: f32,
    t: f32,
    off: bool,
}

impl Shake {
    pub fn kick(&mut self, amount: f32) {
        if !self.off {
            self.trauma = (self.trauma + amount).min(1.0);
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.t += dt;
        self.trauma = (self.trauma - DECAY * dt).max(0.0);
    }

    /// The camera's shift (in its own frame) and tilt now.
    pub fn offset(&self) -> (Vec3, Quat) {
        shake_offset(self.trauma, self.t)
    }
}

/// The offset at `trauma` and time `t`: zero at no trauma, bounded by the maxima at full.
pub fn shake_offset(trauma: f32, t: f32) -> (Vec3, Quat) {
    let k = trauma.clamp(0.0, 1.0).powi(2);
    if k == 0.0 {
        return (Vec3::ZERO, Quat::IDENTITY);
    }
    let w = |hz: f32, phase: f32| (t * hz * TAU + phase).sin();
    let tilt = Quat::from_euler(
        EulerRot::YXZ,
        MAX_TURN * k * w(17.0, 0.3),
        MAX_TURN * k * w(23.0, 1.7),
        MAX_TURN * 0.5 * k * w(13.0, 2.9),
    );
    let shift = Vec3::new(w(19.0, 0.9), w(29.0, 2.2), 0.0) * (MAX_SHIFT * k);
    (shift, tilt)
}

/// The trauma of a blow of flinch `level`: taken by the player, or dealt by the player's sword
/// (a level-1 blow dealt is a light one and does not shake). Tuning values.
pub fn trauma_for(level: u8, taken: bool) -> f32 {
    match (taken, level) {
        (true, 0..=1) => 0.35,
        (true, 2..=4) => 0.55,
        (true, _) => 0.9,
        (false, 0..=1) => 0.0,
        (false, 2..=4) => 0.25,
        (false, _) => 0.4,
    }
}

fn tick(real: Res<Time<bevy::time::Real>>, mut shake: ResMut<Shake>) {
    shake.tick(real.delta_secs());
}

fn kick_on_blows(
    mut shake: ResMut<Shake>,
    mut dealt: MessageReader<Hit>,
    mut taken: MessageReader<PlayerHurt>,
) {
    let mut kick = |level: u8, taken: bool| {
        let amount = trauma_for(level, taken);
        if amount > 0.0 && !shake.off {
            info!(
                "shake: kick {amount:.2} (level {level}, {})",
                if taken { "taken" } else { "dealt" }
            );
            shake.kick(amount);
        }
    };
    for h in dealt.read() {
        kick(h.level, false);
    }
    for h in taken.read() {
        kick(h.level, true);
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Shake {
        off: std::env::var("WOWSOULS_SHAKE").is_ok_and(|v| v == "0"),
        ..default()
    })
    .add_systems(Update, (tick, kick_on_blows).chain());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_trauma_no_shake_and_full_trauma_is_bounded() {
        assert_eq!(shake_offset(0.0, 3.3), (Vec3::ZERO, Quat::IDENTITY));
        for i in 0..200 {
            let (shift, tilt) = shake_offset(1.0, i as f32 * 0.013);
            assert!(shift.length() <= MAX_SHIFT * 2.0_f32.sqrt() + 1.0e-6);
            assert!(tilt.angle_between(Quat::IDENTITY) < 0.04);
        }
        let (a, _) = shake_offset(0.5, 1.0);
        let (b, _) = shake_offset(1.0, 1.0);
        assert!(a.length() < b.length(), "less trauma, less shake");
    }

    #[test]
    fn a_kick_decays_to_nothing_in_the_time_it_says() {
        let mut s = Shake::default();
        s.kick(1.0);
        s.kick(1.0);
        assert_eq!(s.trauma, 1.0, "clamped");
        for _ in 0..23 {
            s.tick(0.0167);
        }
        assert!(s.trauma > 0.0, "0.4 s is not over yet");
        for _ in 0..3 {
            s.tick(0.0167);
        }
        assert_eq!(s.trauma, 0.0);
    }

    #[test]
    fn heavier_blows_shake_more_and_the_light_ones_dealt_do_not() {
        assert_eq!(trauma_for(1, false), 0.0);
        assert!(trauma_for(2, false) > 0.0);
        assert!(trauma_for(5, false) > trauma_for(2, false));
        assert!(trauma_for(1, true) < trauma_for(2, true));
        assert!(trauma_for(2, true) < trauma_for(5, true));
    }

    #[test]
    fn switched_off_it_never_gathers() {
        let mut s = Shake {
            off: true,
            ..default()
        };
        s.kick(1.0);
        assert_eq!(s.trauma, 0.0);
    }
}
```
- [ ] **Step 2:** `mod shake;` in `main.rs`, `shake::plugin` in the `boot.rs` tuple;
  `cargo test -j 4 -p wow-souls shake` → PASS.
- [ ] **Step 3: Camera.** `camera.rs` `seat`: add `shake: Res<crate::shake::Shake>` after `fade`
  (before `observe`), `#[allow(clippy::too_many_arguments)]`; the non-observe branch becomes
  `let (shift, tilt) = shake.offset(); xf.rotation = rotation * tilt; xf.translation = seated + rotation * shift;`
  (the base `rotation`, so nothing accumulates).
- [ ] **Step 4:** the full gates: `cargo fmt --all`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`,
  `cargo test -j 4 --workspace` (and with the install env set).

## Task 5: Re-extract, verify live, document

- [ ] **Step 1:** install snapshot before: `find "/c/Spel/World of Warcraft 1.12" -type f -not -name realmlist.wtf -printf '%p %s %T@\n' | sort > before-wow.txt`, the same for the DSR install.
- [ ] **Step 2:** `cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- extract`; check `cache/dsr/manifest.txt`:
  `move r1_1 ... stop 0.08 knock 0.72`, `attack body_check ... stop 0.00 knock 1.37`.
- [ ] **Step 3: Live (unattended), numbers:**
  - kobold shoved by R1-1: kobold 2 yd ahead on flat ground (`WOWSOULS_AT=-8906,-136
    WOWSOULS_KOBOLD_AT=-8908,-136 WOWSOULS_ATTACK=3 WOWSOULS_TRACE=1`): log `hit: ... (stop 0.08 s, knock 0.72 m)`,
    `shove: kobold 0.79 yd over 0.30 s`, and the kobold's `feet=` trace moves about 0.79 yd.
  - player shoved: `WOWSOULS_SEED=7` fight, `shove: player 0.49 yd` for small_1 and `1.50` for body_check;
    the player's facing is unchanged (trace); no `HURT_STOP` freeze (hit-stop count only from the sword).
  - `WOWSOULS_SHAKE=0` logs no `shake:` line; unset logs `shake: kick 0.35 (level 1, taken)` per taken blow
    and `0.25` for a dealt R2.
  - `WOWSOULS_DSR_CACHE=/nonexistent`: one stale warning, the old fight unchanged (swing 1.43 s, hit 25).
- [ ] **Step 4:** install snapshots after = before (diff empty).
- [ ] **Step 5: Docs.** README: `WOWSOULS_SHAKE=0` row; HANDOVER "Latest" block for D2a (what was built,
  numbers, what is open for the user); module rows `shove.rs`, `shake.rs`; test counts; memory note.
