# wow-souls — Technical Feasibility Report

**Verdict: feasible, no hard blocker.** Build `wow-souls` as its own Cargo workspace that
depends on **`benilla-world` (the engine) only, never `benilla-app` (the client)**. The world
renderer, streaming, M2/WMO rendering, the rig pose evaluator and Avian collision queries all run
with no server. The player controller, follow camera, character dressing and animation selection
all live in `benilla-app` behind `pub(crate)` and are wired to the network types. Those parts have
to be ported (copied with attribution, MIT/Apache-2.0) or newly written. Benilla needs no changes.

| Item | Value |
|---|---|
| benilla revision studied | `2e82d34c1b561e3bb033f7cd910036ba63ce1760` (2026-10-06, v0.2.0) |
| Engine stack | Bevy `0.18.1` (audio features off), avian3d `0.6`, wgpu `27`, Rust `1.98.1` (pinned in `rust-toolchain.toml`) |
| Install | `C:\Spel\World of Warcraft 1.12` — `WoW.exe` FileVersion `1, 12, 1, 5875`, English, 14 MPQs in `Data\` (no locale subfolder, as 1.12 ships) |
| Repos cloned (read-only study) | `C:\Spel\research\{benilla, ai-game-modding-guides, universal-modder}` |
| Verification run | see §8.1 |

Constraints honoured throughout: the retail client is not touched or injected into, nothing
connects to Battle.net or any server, Warden and anti-cheat are not involved, and benilla only
*reads* the MPQs. No game files are committed.

---

## 1. Minimum benilla crates to render one WoW zone

The dependency edge is enforced by benilla itself. `benilla-world`'s manifest says
`# No benilla-protocol: the engine renders a world and does not speak the protocol`, and
`benilla-worldview` links `benilla-world` only, so that *"a world concept that needs the game fails
to build here."*

| Crate | Role | Bevy? | ~LOC |
|---|---|---|---|
| **benilla-world** | Engine: terrain stream, WMO portals, doodads, liquid, sky, lighting, weather, particles, rig pose + skin palette, model render lanes, **collision** (`WorldPlugins`) | yes | 61k |
| **benilla-assets** | `mpq://` AssetSource, M2/WMO/ADT/WDT/BLP loaders, `WowModelMaterial`/`TerrainMaterial`, `ModelAnimations`, coords (`wow_to_bevy`) | yes | 8k |
| **benilla-formats** | DBC tables (`CreatureCatalog`, `CharSections`, `CharacterGeosets`, `AnimDataCatalog`, `Light`…), install resolver (`wow_data()`), `benilla-extract` CLI | no | 66k |
| benilla-mpq / blp / dbc / adt / wdt / wmo / m2 / bytes | Leaf format readers | no | ~6k total |
| benilla-buildstamp | `build.rs` git stamp (optional; only for `BuildId`) | no | 0.2k |

**Not needed:** `benilla-app` (398k LOC: login, net mirror, Lua/FrameXML UI, kira audio, player,
entities), `benilla-protocol`, `benilla-srp`, `benilla-ui`, `benilla-visual`. Leaving these out
also drops the vendored Lua (`lua-src` patch) and kira patches, so the build needs no C toolchain
beyond what Bevy itself needs.

**Bootstrap sequence** (copied from `benilla-world/src/worldview.rs::run`, the reference for a
no-game program):

1. `benilla_formats::set_project_folder(...)` → `benilla_formats::wow_data()` (honours `WOW_DATA`)
2. `benilla_assets::register_mpq_source(&mut app, &data_dir)`, **before** `DefaultPlugins`
3. `boot::tuned_default_plugins(Window{..})` + `ThreadQosPlugin` + `BgWinPlugin` + `MacQuitPlugin`
4. `app.add_plugins(benilla_world::world_plugins::WorldPlugins)`
5. `app.insert_resource(benilla_world::schedule::WorldLive(true))`, the only stub the engine needs
6. Your own plugins (camera, etc.)
7. `benilla_assets::register_asset_loaders(&mut app)`, **after** `AssetPlugin`

Zone selection: the stream focus defaults to `terrain_stream::SPAWN_XY = (-8949.95, -132.49)`,
the Human start in Northshire, on map 0. That is exactly the milestone area, so no map plumbing is
needed.

## 2. How player models and animations are instantiated today

There are two layers, and the split matters for us:

**Engine side (public, reusable) — `benilla-world` + `benilla-assets`:**
- `M2Model` asset (`mpq://…m2`): `submeshes`, `skeleton: ModelSkeleton`, `inverse_bindposes`,
  `animations: Option<ModelAnimations>`, `attachments` (hand/sheath points), `sequences`, `bounds`.
- `ModelAnimations`: a Bevy `AnimationGraph` handle plus `clips`, with lookups
  `find(anim_id)`, `idle_clip()`, `pick_variation()`, `resolve(id, &AnimDataCatalog)`, and a baked
  `PoseSource`.
- `model_forms::ModelForms::require_rigged(model, prio)` builds the skinned render meshes, paced
  per frame. `benilla_assets::submesh_to_skinned_mesh` is also public.
- `rig_palette::RigSkin::allocate_bones(palettes, nbones, ibp)` claims a joint-palette slot. The
  `wow_model.wgsl` shader skins from that palette, not from Bevy's `SkinnedMesh`.
- `rig_anim::RigPose::new(joints_root, &skeleton)` plus `GlobalSeqDrive::new_rig(...)`. The
  evaluator `evaluate_rig_poses` (PostUpdate, `AnimationSystems`) samples **Bevy's own
  `AnimationPlayer`** weights and seeks into the pose. So *we pick clips with a plain
  `AnimationPlayer` + `AnimationTransitions`*, and the engine does the rest.
- `model_render::model_material(...)` builds `WowModelMaterial` per batch.

**Game side (`pub(crate)`, NOT reachable from another crate) — `benilla-app/src/entities/`:**
- The trigger is `Added<NetEntity { kind, display_id, scale }>` plus `ObjectStore` (descriptor
  fields) plus `Guid`. `attach::attach_entity_visuals` resolves `display_id` →
  `CreatureDisplayInfo` → M2 (`entities/display.rs::build_parts`), spawns parts, then:
  - `setup_skinned_instance` (palette slot + `RigPose` + `GlobalSeqDrive`), ~60 lines
  - `arm_sequence_clock` (`AnimationPlayer`, `AnimationGraphHandle`, `AnimationTransitions`,
    `AnimDriver`), ~70 lines
  - players: `char_skin.rs` picks geosets (`CharacterGeosets`, `equip_geosets`) and
    `skin_composite.rs` composites the 256² body atlas from `CharSections` (skin + face + underwear
    + armour) off-thread.
- Clip choice: `creature_anim/driver.rs` + `select.rs` (`AnimDriver`), driven by
  `MovementState`/move flags. Constants: Stand 0, Walk 4, Run 5, ShuffleLeft 11,
  MountSpecial 94, Knockdown 121.
- Proof it runs without a server: `capture/fixtures.rs` spawns a HumanMale player (`display_id:
  Some(49)`) and a Timber Wolf as bare `NetEntity` + `ObjectStore` components in server-less capture
  mode, and the same attach pipeline builds their visuals. Data is not the blocker. Visibility is.

## 3. Current player movement / controller

`benilla-app/src/player/` (~18k LOC), plugin `PlayerPlugin`, per-frame system
`controller::control` in `WorldStage::Input`, gated `InWorldGated` (`ClientState::InWorld`).

- **State:** `Player` resource (pos, vel_y, airborne_since, move flags, `MoveModes` root, hover,
  water walk and so on), `PlayerCapsule(Collider::capsule(1/3, 2.0278 − 2/3))`, `MoveSpeed` (7.0
  yd/s run), plus reference constants in `state.rs` (`GRAVITY 19.291`, `JUMP_SPEED 7.9555`,
  `GROUND_COS 0.6428` ≈ 50°, `STEP_UP_HEIGHT 1.0`, `TERMINAL_VELOCITY 60.148`, …).
- **Kinematic mover:** `mover::step(player, time, &WorldCollision, capsule, moving, dir, speed,
  want_jump, …) -> Outcome`. Ground classify by a downward shape-cast, jump/fall integration,
  `MoveAndSlide` with **one-sided faces**, step-up and step-down, wedge escape, foot-cone ride.
  `mover.rs` imports almost nothing from `crate::` (3 constants), so it is the most portable
  piece in the app.
- **Steering model:** WoW tank/facing-relative. A/D turn (`TURN_RATE π rad/s`), right-drag steers,
  both buttons run, autorun. The body yaw is the facing, and move direction is derived from facing.
  **This is the opposite of Souls camera-relative input** and gets replaced, not reused.
- **Coupling:** `control` takes ~40 params: `NetCommands`, `SelfGuid`, `TeleportMessage`,
  `SpeedChangeMessage`, `KnockBackMessage`, `UiKeyboardCapture`, `BindingsState`,
  `LoadingScreen`, `tutorial::InputHooks`, `ui_loot::LootMoveStart`… and writes `MSG_MOVE_*`
  packets. It can't be lifted as-is.
- **Spawn hold:** `Player.settling` freezes the body with gravity off until the terrain streamer
  reports the destination resident (`WorldLoadProgress::presentable()`, which includes
  `colliders_pending == 0`). We need the same idea, or the body falls through tiles whose trimesh
  is still building on the async pool.

## 4. Follow camera and its collision

`player/camera.rs` (2.2k) + `camera_dynamics.rs` (1.3k) + `camera_zoom.rs` + `camera_channel.rs`.

- `CameraControl` resource (distance, target_distance, collision_distance, look session,
  follow, `SmartPivot` bias, `TerrainTilt`, head bob, self-fade alpha). The camera entity carries
  `FlyCam { yaw, pitch, speed }` and `benilla_world::view::WorldCamera`.
- `seat_camera(...)`: pivot = `feet + cam_pivot_height`, seat = `pivot − fwd·distance`, one sweep
  **from the head to the seat** via `WorldCollision::cast_camera(head, boom, water)`. Snap in on
  intrusion, ease out at `CAM_RETURN_RATE`. A self-avatar fade within `SELF_FADE_WINDOW`
  (`model_fade::self_model_fade_alpha`, public).
- Camera collision filter = `Default | Camera` layers. WMOs bake **two trimeshes per group**: a
  walk collider (drops MOPY DETAIL `0x04`) and a camera collider (drops NOCAMCOLLIDE `0x02`). That
  way the camera hits a low beam the body walks under. The probe is a 0.3 yd sphere
  (`CAMERA_PROBE_RADIUS`, a documented deviation from the reference's bare ray), plus a waterline
  ray on the `Liquid` layer.
- WoW-specific behaviour layered on top that we don't want for Souls: `cameraSmoothStyle`
  auto-follow, SmartPivot, terrain tilt, head bob, CVar observers.

**Reuse:** `WorldCollision::cast_camera` (public) is the whole collision part. The ~60-line sweep
and ease in `seat_camera` is a good template. A Souls camera (free orbit independent of the body,
then lock-on) is authored new.

## 5. How Avian3D is integrated

`benilla-world/src/world_plugins.rs` + `collision.rs` + `terrain_stream/collider.rs`:

- `PhysicsPlugins::default()` with **dynamics stripped**: `BvhBroadPhasePlugin`,
  `NarrowPhasePlugin`, `ForcePlugin`, `MassPropertyPlugin`, solver, integrator, CCD, islands,
  joints and interpolation are all disabled. Kept: `ColliderTreePlugin` (BVH) and the spatial
  query pipeline. `SubstepCount(1)`, `Gravity(19.291)`, `propagate_before_physics: false`.
  Code comment: *"Re-enable when a dynamic body enters the world."*
- Everything is `RigidBody::Static` trimeshes built on `AsyncComputeTaskPool`
  (`build_collider_task`), attached under a per-frame budget (`finish_colliders`). Terrain is one
  welded trimesh per tile. Doodads get their M2 collision hull, WMOs get per-audience trimeshes.
  **Units carry no colliders.**
- Layers (`CollisionLayer`, `pub(crate)`): `Default` (terrain, doodads, GOs), `Walk`, `Camera`,
  `Liquid`. Filters `body_filter()` = Default|Walk, `camera_filter()` = Default|Camera.
- **Public query API**, the `WorldCollision` SystemParam: `cast_body`, `cast_mover`,
  `slide_body`, `slide_mover` (MoveAndSlide, one-sided), `cast_camera`, `ray_body`,
  `faces_near_body`, plus `ColliderEpoch` and `MoverTraceExclusions`.
- **Implications for Souls combat later:** there is no contact pipeline, so hitboxes must be
  *queries* (`SpatialQuery::shape_intersections` / shape-casts against our own sensor colliders),
  not collision events. That is the deterministic, frame-data-driven way Souls hitboxes work anyway.
  Our layers must use raw `LayerMask` bits ≥ 4, since the enum is `pub(crate)`. An enemy body
  capsule on `Default` would give body-blocking for free through `cast_mover`.

## 6. Proposed standalone `wow-souls` crate

**Principle:** zero edits to benilla. Benilla is pinned at a known SHA and consumed as a path
dependency. Anything we need from `benilla-app` is **copied** into `wow-souls` with a provenance
header (file + SHA + license) and trimmed to the single-player case.

```
C:\Spel\
├─ World of Warcraft 1.12\          (read-only install; never written)
├─ benilla\                          (clone pinned @ 2e82d34; untouched)
└─ wow-souls\                        (our Cargo workspace, own git repo)
   ├─ Cargo.toml                     workspace; [profile.*] copied from benilla (deps opt-level 3!)
   ├─ Cargo.lock                     seeded from benilla's lockfile (exact bevy/avian/wgpu pins)
   ├─ rust-toolchain.toml            channel = "1.98.1"
   ├─ .gitignore                     whitelist; WoW/, target/, *.png captures ignored
   ├─ docs/ (this report, PROVENANCE.md listing every ported file + SHA)
   └─ crates/
      └─ wow-souls/
         ├─ Cargo.toml   deps: benilla-world, benilla-assets, benilla-formats (path, default-features=false + "dev" opt-in),
         │                     bevy = { workspace }, avian3d = "0.6"
         └─ src/
            ├─ main.rs           bootstrap (§1 sequence), WorldLive(true), GameState
            ├─ state.rs          AppState: Boot → Streaming(hold) → Playing
            ├─ focus.rs          writes terrain_stream::ViewFocus::body(..) + view::Viewer each frame
            ├─ motor/            ported mover::step (land only; swim/hover/transport/knockback cut)
            ├─ controller.rs     NEW: camera-relative input → desired dir; body yaw slerps to it
            ├─ camera.rs         NEW: Souls orbit cam; collision via WorldCollision::cast_camera
            ├─ actor/
            │  ├─ spawn.rs       ported setup_skinned_instance + arm_sequence_clock + build_parts subset
            │  ├─ character.rs   ported geoset pick + skin composite (human male, fixed look, naked)
            │  ├─ creature.rs    display_id → CreatureCatalog → M2 + creature skin variations
            │  └─ anim.rs        NEW: small clip state machine on AnimationPlayer/AnimationTransitions
            ├─ npc.rs            NEW: one hostile placed by hand (idle, face player in range)
            └─ debug.rs          F-key overlays, capsule gizmos, pos/fps readout (dev only)
```

**Schedule placement** (reuse benilla's sets so ordering edges hold):
`WorldStage::Input` → our `controller`, `motor`, `camera`, `focus`, *before* `WorldStage::Stream`.
The anim state machine goes in `Update` after the motor. The engine poses rigs in `PostUpdate`.

**Alternative considered and rejected:** `benilla_app::run_with(build, |app| ...)`, the
sanctioned "build on top" seam. It boots the full client (login screen → realm → char select →
`ClientState::InWorld`, net thread, Lua UI). Every player, entity and anim type is `pub(crate)`,
and the controller is welded to `NetCommands`/`SelfGuid`. Going that way would mean forking
benilla-app to open an offline seam, which contradicts "don't modify benilla" and drags in 398k
LOC we'd then have to suppress. *Fallback only:* if porting the dressed-character pipeline
(armour composite, held weapons) turns out too costly in a later milestone, a fork could add one
`pub fn spawn_local_unit(...)` seam. That would be a small, isolated patch.

## 7. Reuse vs. author

| Area | Reuse as-is (dependency) | Port (copy + trim from benilla-app) | Author new |
|---|---|---|---|
| World render (terrain, WMO, doodads, water, sky, light, fog, weather, particles) | ✅ `WorldPlugins` | | |
| Asset IO (MPQ chain, BLP, M2, WMO, ADT, DBC) | ✅ assets + formats | | |
| Collision world + queries | ✅ `WorldCollision`, streamed colliders | | own `LayerMask` bits for actors/hitboxes |
| Rig pose, skin palette, global seqs, model materials | ✅ `rig_anim`, `rig_palette`, `model_render`, `model_forms` | | |
| Spawn an animated M2 instance | | `setup_skinned_instance`, `arm_sequence_clock`, `display::build_parts` subset (~400 lines) | |
| Human character look | `CharSections`, `CharacterGeosets`, `CompositePlan` (formats, public) | `skin_composite.rs`, `char_skin.rs` geoset subset (~500 lines) | fixed default look table |
| Creature (hostile NPC) look | `CreatureCatalog` | display→model+skin part of `build_parts` | hand-picked display id |
| Kinematic movement | | `mover::step` + `grounded_step`/`airborne_step`/`step_up` + constants (~1.2k after trimming) | camera-relative input, facing slerp |
| Third-person camera | `cast_camera`, `self_model_fade_alpha` | sweep/ease from `seat_camera` (~60 lines) | Souls orbit, later lock-on, soft follow |
| Animation selection | `ModelAnimations::find/resolve` | | locomotion FSM (Stand/Walk/Run/Jump*/Fall) |
| Spawn settle | `WorldLoadProgress::presentable()` | `settling` idea | state gate |
| Stream focus / zone | `ViewFocus::body`, `Viewer`, `SPAWN_XY` | | per-frame writer |
| Day/night | fallback clock / `WOW_CLOCK` | | optional fixed hour |
| NPC data (entry→display, level, faction, HP, position) | — (server-side in WoW; not in client DBCs) | | hand-authored RON/const table |
| Audio | — (kira mixer lives in app) | | out of scope for milestone 1 |
| UI / quests / inventory / net | — | — | out of scope (by design) |

Combat-phase notes (not milestone 1): `Attack1H/2H`, `Parry*`, `ShieldBlock`, `CombatWound`,
`Death`, `Ready*` and `Stun` are standard 1.12 animation ids. **A dodge roll is not expected** in
the vanilla human rig (unverified). If it's missing, it needs procedural motion over a borrowed
clip, or a custom animation track. Inventory clips per model with the existing `benilla-extract`
CLI before the combat design is locked.

## 8. Blockers to running the world without a WoW server

### 8.1 Verified empirically

Stock benilla at `2e82d34`, unmodified, built and run on this machine (Windows 11):

| Step | Command / result |
|---|---|
| Build | `cargo build -p benilla-worldview` → `Finished dev profile [optimized + debuginfo] target(s) in 10m 38s`, exit 0. rustup auto-installed 1.98.1. |
| No-game gate | `WOW_DATA="C:\Spel\World of Warcraft 1.12\Data" WOW_WORLDVIEW_CHECK=60 benilla-worldview` → `WORLDVIEW_CHECK ok — the engine booted and ran with no game attached` |
| Render | `WOW_WORLDVIEW_AT=-8913,-136,82 WOW_WORLDVIEW_SHOT=… WOW_WORLDVIEW_SHOT_AT=35` → Northshire Abbey, trees, doodads, terrain splats, sky and fog rendered from the local MPQs ([evidence/worldview-northshire-abbey.png](evidence/worldview-northshire-abbey.png), gitignored game art) |
| Install untouched | file list + size + mtime of all 176 files in `C:\Spel\World of Warcraft 1.12`, diffed before/after → `INSTALL_UNCHANGED` |
| Network | none: worldview links no protocol crate. No server and no Battle.net were involved. |

Worth knowing: the default worldview eye (`SPAWN_XY` at z 100, +60 yd) lands inside a tree
canopy, so the first shot was foliage. The slice should pick its own spawn and camera.

### 8.2 Analysis

**No hard blocker.** benilla treats "engine runs with no game" as a *gated invariant*:
`benilla-worldview` plus `WOW_WORLDVIEW_CHECK=<s>` (run in `scripts/gates.sh`) boots
`WorldPlugins` with no server, login, UI or player, and fails on any unmet system parameter.
`WOW_WORLDVIEW_SHOT=<png>` renders a frame and exits.

What the server normally supplies, and our replacement:

| Server-provided | Replacement |
|---|---|
| Map + spawn position | constant (map 0, Northshire, near `SPAWN_XY`); ground height by `ray_body` down once colliders are resident |
| Character appearance (`PLAYER_BYTES*`) | fixed look constants (race 1, sex 0, skin/face/hair) |
| Creature template (entry → display, level, faction, HP) | hand-authored table; `creature_template` is server DB, not client DBC |
| Unit positions & movement | our own simulation |
| Movement speeds | 7.0 run / 2.5 walk constants (as `setup.rs` defaults) |
| Game time | engine fallback clock (`ClockSource::Fallback`) or fixed `WOW_CLOCK` |
| Weather | none (clear) |

**Soft risks (not blockers):**
1. **API visibility.** Everything in §2/§3 game-side is `pub(crate)`, so porting is required.
   benilla "promises no stable API" (CONTRIBUTING), which is why the revision is pinned.
2. **Async collider streaming.** The body must be held until `presentable()`, or it falls through.
3. **Build profile.** Deps must be at `opt-level = 3` in dev (*"at opt-level 0, avian/parry
   collision drops a run to <1 fps"*). Copy benilla's `[profile.*]` blocks; Cargo reads profiles
   from the root manifest only.
4. **Toolchain pin.** `1.98.1` is fetched by rustup automatically (observed during the
   verification build).
5. **Install is read-only** (benilla's hard rule). Captures and logs go under `wow-souls/`, never
   into `C:\Spel\World of Warcraft 1.12`.
6. **Name clash:** benilla-protocol ships a CLI *binary* called `benilla-world`. It is unrelated to
   the crate. Never build `-p benilla-protocol`.

## 9. Milestone 1 plan (after approval — not started)

Scope: Northshire slice, one human male (naked, default look), Souls third-person camera,
camera-relative movement, one hand-spawned hostile creature (idle; turns to face the player
within ~20 yd; no combat), no UI, quests, inventory or networking.

Order, each step a runnable checkpoint:
1. Workspace + bootstrap: free-fly worldview-equivalent in our own binary (proves deps/profile).
2. State gate + focus writer: stream around a fixed point, hold until `presentable()`.
3. Capsule + ported motor + camera-relative controller, drawn as a gizmo capsule (no model yet).
4. Souls camera with `cast_camera` collision.
5. Actor spawn: HumanMale M2, geosets, skin composite, Stand/Walk/Run/Jump FSM.
6. Hostile creature by display id (kobold or wolf from Northshire; id confirmed from
   `CreatureDisplayInfo` via `benilla-extract`), idle + face-player.

Acceptance criteria:
- `cargo run -p wow-souls` with `WOW_DATA=C:\Spel\World of Warcraft 1.12\Data` opens directly in
  Northshire. No login, no network socket opened, no UI frames.
- WASD moves relative to the camera. The body turns to the move direction. It doesn't fall through
  terrain at spawn, climbs Northshire Abbey steps, slides on steep slopes.
- The camera orbits with the mouse, doesn't clip through the Abbey walls (WMO camera collider),
  and fades the body when pressed against geometry.
- Human male renders with the correct body geosets and skin, and plays Stand/Run/Jump.
- One hostile creature renders animated at a fixed spot.
- `C:\Spel\World of Warcraft 1.12` is byte-unchanged after a run.
