# G1b Critters And Friendly NPCs From The Manifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The valley's life as WoW Classic has it: the 53 critters (Rabbit 721, Deer 883, Fawn 890) wander their areas and run from the player and die to one blow; the 31 friendly NPCs (the Abbey's trainers, vendors, McBride, Willem, Eagan, Milly, the guards, the peasants, the White Stallion) stand at their posts or walk their paths, dressed as WoW dresses them, and do nothing hostile. Both appear through the G1a ring.

**Architecture:** `ring_tick` (`world.rs`) already decides which spots exist and calls `npc::place` for `Kind::Foe`; spots of other kinds are `State::Off` today. G1b adds two builders next to `place`: `critter::place` and `friendly::place`, switches `Kind::Critter` / `Kind::Friendly` spots from `Off` to normal ring spots, and adds one mover system for non-hostile walkers (`walkers.rs`: the same `motor::step` + `Roam` + sim-radius rules as `move_hostiles`, for `Critter` and `Npc` entities). The pure rules (flee) are unit-tested like `roam.rs`.

**Tech Stack:** Rust 2021, Bevy 0.18.1, `wow-world` manifest (`Creature.display`, `scale`, `kind`, `npc_flags`, `equipment_id`; `World.equipment`, `World.items`), the actor builder `ActorModel::for_display` (E2-2b dressing for character displays).

Spec: `docs/superpowers/specs/2026-10-09-northshire-quests-design.md` (G1). Built before this: G0 (importer), G1a (ring, roaming, respawn; `docs/superpowers/plans/2026-10-09-g1a-spawns-ring-roam.md`, HANDOVER "G1a BUILT"). Read both, then `npc.rs` (`place`, `Roam`, `think`, `move_hostiles`, `follow_bodies`, `spawn_body`), `world.rs`, `ring.rs`, `roam.rs`, `species.rs`, `actor/mod.rs` (`ActorModel::for_display`, `NpcWorn`), `actor/anim.rs` (`drive`: Stand / Walk / Run from `Motor`), `actor/held.rs` (`HeldItem`), `hits.rs` (`Hit`, `Hurtbox`, `HitFloor`, `strike`), `lockon.rs`.

Facts from the code map (2026-10-09): a creature with no `Hostile` + `Brain` + kit is skipped by every hostile system (silently, no panic); `anim::drive` needs `&Motor` to arm Stand / Walk / Run and a never-stepped `Motor::default()` reads as Fall after 0.25 s, so a resting creature needs `Motor { grounded: true, .. }` (G1a parks resting foes this way; reuse its helper); `hits::strike` damages any entity with `Hurtbox` + `Health`, `npc::react` needs the whole hostile set, so a critter needs its own `react_critter`; a body collider is a separate top-level kinematic entity (`spawn_body`, private in `npc.rs`), despawned with the creature by `ring_tick`; plain creature displays (rabbit 328, deer 347, fawn 654, stallion 2410) build with `Look::Creature` (no geoset filtering: check each by picture), character displays (`CreatureDisplayInfoExtra`) build with `Look::Character` plus `NpcWorn` (baked atlas, equipment geosets, helm / shoulder models); `Creature.scale` of the manifest is not used for the approved foe species and must be compared with the model's own scale for new kinds (decide by picture); `held.rs` supports weapons on attachment 1 only.

Rules that bind every task: same gates and conventions as G1a (fmt, clippy `-D warnings` workspace, tests with `DSR_DIR` / `WOW_DATA`, `-j 4`, no commits, installs untouched, unattended live checks only, nothing in `docs/HANDOVER.md` edited by the implementer). The old behaviour with `WOWSOULS_WORLD=0` and in unattended runs by default stays identical (the HANDOVER's scripted checks).

## File structure

| File | Responsibility |
|---|---|
| `crates/wow-souls/src/critter.rs` (new) | `Critter` component, pure `flee` rules, `place`, `react_critter`, `critter_think` |
| `crates/wow-souls/src/friendly.rs` (new) | `Npc` component, `place` (display, dressing, held item, body collider), the roster exclusions |
| `crates/wow-souls/src/walkers.rs` (new) | `move_walkers`, `follow_walker_bodies` for non-hostile movers |
| `crates/wow-souls/src/world.rs` | `ring_tick` calls the new builders; `Off` only for `Unmapped` and the excluded entries |
| `crates/wow-souls/src/npc.rs` | `spawn_body` and the parked-motor helper become `pub(crate)`; `follow_bodies` stays hostile-only |
| `crates/wow-souls/src/main.rs` | `mod critter; mod friendly; mod walkers;` and the plugins |

---

### Task 1: The critter rules (pure)

- [ ] **Step 1: tests first in `critter.rs`** for a pure `flee_step`:

```rust
/// What a critter does this frame, from its distance to the player (ground yd) and its state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mood {
    /// Grazing: walks its area like any wanderer.
    Graze,
    /// Running from the player until it is `SAFE` yards off or `FLEE_MAX` seconds have passed.
    Flee { left: f32 },
}

pub const FLEE_NEAR: f32 = 9.0; // a person closer than this startles it (deer: 14, see `startle_for`)
pub const SAFE: f32 = 22.0;
pub const FLEE_MAX: f32 = 6.0;

pub fn mood_step(mood: Mood, dist: f32, near: f32, dt: f32) -> Mood
```

Rules to test: `Graze` with `dist <= near` becomes `Flee { left: FLEE_MAX }`; `Flee` counts `left` down by `dt`, returns to `Graze` when `left <= 0` or `dist >= SAFE`; a critter already fleeing is re-startled (`left` reset) while the player stays within `near`. `startle_for(entry)`: Rabbit 721 -> 8.0, Fawn 890 -> 10.0, Deer 883 -> 14.0, other -> 9.0. A pure `flee_dir(from_player: [f32;2], pos: [f32;2], home: [f32;2]) -> [f32;2]` returns the unit vector away from the player, bent toward home when the critter is more than twice its wander radius from it (tested: away when near home, bent when far).

- [ ] **Step 2:** run, then gates.

### Task 2: Critters in the game

- [ ] **Step 1: `critter::place`** builds one critter at a spot (WoW coords, Bevy yaw, `Area`, `SpawnId`): `Name` (the template name), `Transform` + `Visibility`, `ActorModel::for_display(&catalogs.creatures, display, None, &asset_server)` with the template's first display id (`Creature.display[0]`; when several have probabilities pick by the record index), ground snap through `pick_ground` as `npc::place` does (a ring spawn keeps retrying without ground), `Motor`, `MoveIntent`, `Roam`, `Walk`, `Critter { entry, mood, near, run_speed }`, `Health::full(1.0)`, `Hurtbox` from the model's collision height and a radius of 0.3 x height (plus `HitFloor(0.35)` as the Young Wolf has: they are small), `Reaction`, `SpawnId`. No `Lockable`, no `BodyCollider` (the player walks through critters, as in WoW they are not solid). Their run speed is `species-like`: Rabbit 4.5, Fawn 5.0, Deer 7.0 yd/s, walk speed 1.0-1.5.
- [ ] **Step 2: `critter_think`** (Update, in the creature chain after `think`, same sim gate): player distance -> `mood_step`; `Graze` runs `roam::step` at the walk speed and plays Stand / Walk through `anim::drive`; `Flee` sets `MoveIntent` along `flee_dir` at the run speed. Critters pause and graze like wanderers.
- [ ] **Step 3: `react_critter`**: on a `hits::Hit` whose target is a `Critter`, play the model's Death clip (id 1) once (`play_once`, as `npc::react` does for the kobold's `DEATH`), set `Reaction::Dead`, remove `Hurtbox`, send `npc::Slain { who, souls: 0 }` (G3 counts kills from it), blood spurt (the existing `blood.rs` trigger keys on `Hit`; check it still plays for a creature without `Species`). A dead critter stays as a corpse until the ring replaces it (the G1a respawn rules apply: `note_deaths` finds `SpawnId`).
- [ ] **Step 4: switch the spots on**: in `world.rs` the `Kind::Critter` spots are no longer `State::Off`; `ring_tick` calls `critter::place` for them; a missing model makes that spot `Off` and logs once (as for foes).
- [ ] **Step 5: tests**: pure tests from Task 1; a spot table test that every `Kind::Critter` entry of the real manifest has a display (install test, skipped without the manifest); `react_critter`'s decision as a pure function if it has one.

### Task 3: Friendly NPCs in the game

- [ ] **Step 1: `friendly::place`**: `Name` (template name; keep `subname` in the `Npc` component), `ActorModel::for_display(&catalogs.creatures, display, None, &asset_server)` (a character display is dressed by the E2-2b pipeline: check that McBride (display 1859), Willem (2072), the trainers (3343-3351), Eagan (3251), Milly (8489), the guards (3257 / 3167) and peasants (11354) each build; the White Stallion (2410) is a plain creature), `Npc { entry, flags, subname }`, the parked `Motor` for posts (grounded, never stepped) or a stepped one for walkers, `Roam` / `Walk` from the spot's area (the guards and peasants walk their 8 paths; the rest stand at their post and keep the DB orientation), `SpawnId`, a `BodyCollider` (so the player cannot walk through them; `follow_walker_bodies` moves it for walkers). A held weapon: `World.equipment` row of the template's `equipment_id` -> item entry -> `Item.display` -> `HeldItem::right_hand(display)` when the item is a weapon (`class 2`); armour comes from the display's own extra row. No `Hurtbox`, `Health`, `Lockable`, `Hostile`, `Brain`: the player's sword passes through them (the quest phase adds talking). The Spirit Healer (6491, ghost-only in WoW) is skipped (`Off`), and nobody is `Lockable`.
- [ ] **Step 2: `walkers.rs`**: `move_walkers` steps `Motor` for `Critter` and `Npc` with a `MoveIntent` the way `move_hostiles` does (same sim radius gate, `motor::step`, no reaction check for NPCs, skip dead critters), `follow_walker_bodies` keeps NPC bodies on their NPCs, and an NPC / critter outside the sim radius gets the parked motor so it shows Stand. NPC walkers use `roam::step` in their own small system `npc_think` (walk speed 1.8 yd/s for guards and peasants, WoW's walk).
- [ ] **Step 3: switch the spots on** in `world.rs` as in Task 2 step 4; the Friendly spots are the Abbey crowd (about 20 flagged NPCs within 40 yd of the Abbey doors): confirm the ring still spawns at most 3 per tick and that the Abbey fills within a second or two of loading.
- [ ] **Step 4: tests**: `friendly::skipped(entry)` (Spirit Healer), the held-weapon choice as a pure function over `World` fixtures, and an install test (skipped without the manifest) that all 31 friendly entries have a display row.

### Task 4: Verify live and look

- [ ] **Step 1: unattended runs** (`WOWSOULS_WORLD=1 WOWSOULS_CHECK=... WOWSOULS_TRACE=1`): (a) `world: ...` shows Critter and Friendly spots live (no longer `Off`), (b) at the Abbey doors the NPCs stand where the database puts them (compare three trace positions with the manifest) and the guards near WoW (-9047, -101) walk their path, (c) a rabbit and a deer flee when the player runs at them (trace: mood, distance, speed) and graze again after, (d) one blow kills a rabbit (`Slain`, Death clip, the corpse stays, a replacement only per the G1a rules), (e) a critter pulled out of its area walks home, (f) no ECS faults in any run, (g) cost: the same three places as in G1a (Abbey with 20+ NPCs, Echo Ridge, Defias camp) with `WOWSOULS_FRAMES=1`, dev profile, compared with the G1a numbers (Abbey 4.2-4.5 / 5.5-7.6 ms), (h) the old unattended default still gives the lone kobold.
- [ ] **Step 2: pictures** (`WOWSOULS_SHOT`, three to five): the Abbey front with its NPCs (do they stand on the ground, look dressed, face the right way), a deer and a rabbit in the meadow, a guard on a path, a peasant. Read them with the Read tool and report anything wrong (naked NPCs, floating, wrong scale, a critter model with extra submeshes drawn: critters build without geoset filtering).
- [ ] **Step 3: gates (whole workspace), installs untouched, report** (what was built per task, deviations, numbers from every check above, what could not be verified, what the user should look at).

## Self-review against the spec

- G1 "friendly NPCs (the built NPC dressing pipeline) ... critters (rabbit, deer, fawn ... a new `Critter` species: a wander / flee brain, the WoW model, die in one blow, no Dark Souls kit)": Tasks 1-3. "no mob only stands" for critters and walking NPCs: `roam::step` through `critter_think` / `npc_think`. Quest hooks (`Npc.entry`, `Slain` from critters) are provided for G3. Mine Spider and Kobold Miner stay `Unmapped` (decision pending); the Spirit Healer is skipped.
- Names used across tasks: `Critter`, `Mood`, `mood_step`, `startle_for`, `flee_dir`, `critter::place`, `react_critter`, `critter_think`, `Npc`, `friendly::place`, `walkers::{move_walkers, follow_walker_bodies}`, `npc_think` are defined here; `npc::{place, spawn_body, Roam, Slain}`, `roam::step`, `world::{ring_tick, SpawnId}`, `hits::{Hit, Hurtbox, HitFloor}` already exist.
