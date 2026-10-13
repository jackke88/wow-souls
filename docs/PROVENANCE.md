# Provenance

## benilla pin

| | |
|---|---|
| Upstream | https://github.com/samwhosung/benilla |
| Checkout | `C:\Spel\benilla` (sibling of this workspace; path dependencies) |
| Commit | `2e82d34c1b561e3bb033f7cd910036ba63ce1760` (2026-10-06, v0.2.0) |
| License | MIT OR Apache-2.0 |
| Crates used | `benilla-world`, `benilla-assets`, `benilla-formats`, `benilla-buildstamp` (and their leaf readers) |
| Local modifications | none |

`Cargo.lock` was seeded from benilla's lockfile at that commit, so Bevy, avian3d and wgpu resolve
to the exact versions the engine is built and tested with.

## Code ported from benilla

Each file copied or adapted from benilla is listed here with its source path at the pinned
commit. The file's own header names the source too.

| wow-souls file | benilla source | Notes |
|---|---|---|
| `src/boot.rs` (bootstrap order) | `crates/benilla-world/src/worldview.rs` (`run`) | re-written, same order |
| `src/check.rs` | `crates/benilla-world/src/worldview.rs` (`WOW_WORLDVIEW_CHECK`) | adapted |
| `src/shot.rs` | `crates/benilla-world/src/worldview.rs` (`shoot_and_exit`) | adapted |
| `src/settle.rs` (hold release + stall backstop) | `crates/benilla-app/src/player/world_focus.rs` (`publish_view_focus`, `release_post_snap_hold`) | re-written for one body, no net |
| `src/motor/resolve.rs` | `crates/benilla-app/src/player/mover.rs` (`Support`, `grounded_step`, `fall_step`, `airborne_step`, `walkable_ride_velocity`, `foot_cone_ride`, `steep_contact_shear`, `StepVerdict`, `step_up`, and their tests) | copied; liquid floor and hover offset removed |
| `src/motor/testkit.rs` | `crates/benilla-app/src/player/mover.rs` tests (`world_from_profile`, `player_capsule`) | copied |
| `src/camera.rs` (boom sweep from the head, snap in / ease out, `RETURN_RATE`) | `crates/benilla-app/src/player/camera.rs` (`seat_camera`, `CAM_RETURN_RATE`) | re-written; WoW's follow styles, smart pivot, tilt and bob not taken |
| `src/actor/mod.rs` (`build`) | `crates/benilla-app/src/entities/display.rs` (`build_parts`, `resolve_skin`), `entities/attach/mod.rs` (`attach_entity_visuals`, `setup_skinned_instance`, `arm_sequence_clock`), `entities/attach/dress.rs` (`spawn_part`), `entities/attach/char_skin.rs` (`build_char_skin_materials`) | re-written for one body; equipment, mount, corpse, fades, billboards, emitters and picking not taken |
| `src/actor/skin.rs` | `crates/benilla-app/src/entities/skin_composite.rs` (`work`, `read_texture`, `install`) | re-written synchronous, one body |
| `src/actor/anim.rs` (gait rate) | `crates/benilla-app/src/creature_anim/select.rs` (`playback_rate`), `driver/play.rs` (`arm`) | the rate law and the arm; the state machine is ours |
| `src/actor/held.rs` | `crates/benilla-app/src/entities/equipment/spawn.rs` (held item under the attachment anchor at its offset), `creature_anim/driver/grip.rs` (`drive_hand_grip`, `HAND_GRIP_WEIGHT`) | re-written for one weapon; sheathing, glows, trails not taken |
| `src/run_mode.rs` | `crates/benilla-world/src/bgwin.rs` (`background_run`) | the idea, re-written |
| `src/loot/vfx.rs` | `crates/benilla-app/src/creature_anim/spell_visual.rs` (`arm_loot_fx`), `entities/spell_fx/mod.rs` (`attach_effect_visuals`, `arm_effect_rig`, `fx_part_material`, `tick_fx_tint`), `entities/display.rs` (`build_parts`, `EntityPart`) at the graphics fork `C:Spelesearchenilla-everwood` | re-written for one-shot and looping effects on a unit (loot sparkles, level-up, the abilities): the instance rig, parts with their own alpha / colour / texture clocks, ground quads as decals, billboard cards, emitters, ribbons; kit stages, fades, lights and the host chain not taken |
| `src/sfx.rs`, `src/audio.rs` (rules only) | `crates/benilla-app/src/sound/combat.rs` (impact slot by victim material, metal weapon rows, natural weapon falls back to the unarmed row, swish by `ItemSubClass.WeaponSwingSize`), `sound/kit.rs` (variation by `Freq`, exertion 70 % and injury 60 % chances, kit volume and distances) | the rules re-written for one weapon and one creature; benilla-app's mixer (a patched kira, own backends) is not used: `src/mixer.rs` is our own small mixer on cpal 0.17 |
| `src/motor/mod.rs` (`step`, constants) | `crates/benilla-app/src/player/mover.rs` (`step`), `player/state.rs` (constants) | adapted: land movement only; root, hover, water walk, knockback, wire and trace removed |

## Game data

Nothing from the WoW install is in this repository. The engine reads the player's own 1.12.1
install at runtime through the gitignored `WoW` junction or `WOW_DATA`, and never writes to it.

## Dark Souls Remastered readers (GPL-3.0-or-later crates)

`crates/dsr-formats` and `crates/dsr-cache` are GPL-3.0-or-later. Ported from:
- SoulsFormatsNEXT (soulsmods, GPL-3.0): `Formats/DCX.cs`, `Formats/Binder/*` (BND3),
  `Formats/TAE/*` (DS1 TAE), `Formats/PARAM.cs` and `Formats/PARAMDEF.cs` (the layout of DSR's
  PARAM files and PARAMDEF version 104), `Formats/TPF.cs` (texture archives), `Formats/DRB.cs`
  (UI layouts: sections, dialogs, shape records) and `Formats/CCM.cs` (bitmap font tables). The
  DRB and CCM readers decode only what the HUD needs, and their field offsets were found and
  checked against the user's own `menu.drb` and `DSFont24` (sub-project F).
- SoulsAssetPipeline (Meowmaritus, GPL-3.0): `Havoc/IO/Tagfile/Binary/*` (Havok 2015 tagfile),
  `SoulsAssetPipeline/Animation/HKX/SplineCompressedAnimation.cs` (spline decompression),
  `HKX.GenFakeFromTagFile.cs` (field names).
- HavokLib (Lukas Cone, GPL-3.0), via the above.
- DSAnimStudio's `TAE.Template.DS1.xml` for event meanings (type 0 `ChrActionFlag`, flag 8
  Invincible).
- Souls Modding wiki (soulsmodding.com, DS1 reference material): the attack animation id scheme
  and the field meanings of `BehaviorParam`, `AtkParam` and `EquipParamWeapon`.
The game crate (`wow-souls`, MIT OR Apache-2.0) does not link them; it reads `.wscl` files
(`crates/wscl`, MIT OR Apache-2.0, our own format).

## WoW audio (2026-10-10): rules ported from benilla-app, no code copied wholesale

The game reads every WoW sound from the player's own install (the patch chain, nothing copied). The rules
below are benilla-app's (benilla @ 2e82d34, MIT OR Apache-2.0), re-written for our own mixer; the tables
are `benilla-formats` (a dependency):
- `foot.rs`: the event-key scan of `creature_anim/events.rs` (`scan_events`, the `(prev, cur]` window and
  the loop wrap) and the footstep rule of `sound/footsteps.rs` (`$FSD` -> `TerrainType` x the creature's
  `FootstepID` -> `FootstepTerrainLookup` -> `SoundEntries`, the splash kit when wading, silence when swimming).
- `zone.rs`: the zone music and ambience schedule of `sound/zone.rs` (the area -> `ZoneMusic` /
  `SoundAmbience` with the parent walk, day 05:30-21:00, a new zone's track opens at once, the old fades over
  4 s, a random silence between tracks, ambience crossfades over 5 s). Not ported: intro fanfares,
  interiors, weather, ghost and underwater beds, server-pushed music.
- `vocal.rs`: the class chances of `sound/combat.rs` (exertion 70 %, injury 60 %), the NPC hello / goodbye of
  `sound/greeting.rs` (`NPCSounds` by display) and the coin of `sound/money.rs` (`LOOTWINDOWCOINSOUND`).
- `mixer.rs` / `stream.rs`: our own (benilla uses Kira); a streamed voice fed by a decoder thread with a
  stateful linear resampler.
- The Spoken Player quest voices (`quests/voice.rs`): MIT, by rustykey, github.com/rusty-key/spoken-wow;
  a separate download, bundled with the release package by the user's decision.

## benilla-everwood graphics fork (2026-10-10): the engine dependency and the ported lane drivers

| | |
|---|---|
| Upstream | https://github.com/pkuzic/benilla-everwood_graphics (a fork of samwhosung/benilla) |
| Checkout | `C:\Spel\research\benilla-everwood` (the workspace `Cargo.toml` path dependencies point here; the old pin `C:\Spel\benilla` @ `2e82d34` stays untouched as the fallback) |
| Commit | `f545081e9f60668b4f5b1252b67b8058c5995bfd` (210 commits past the old pin; upstream benilla merged to `c2fc3cf3`) |
| License | MIT OR Apache-2.0 |
| Crates used | the same as before (`benilla-world`, `benilla-assets`, `benilla-formats`, `benilla-buildstamp`, `benilla-m2` and the leaf readers); `benilla-app` is **not** a dependency |
| Local modifications | the grass patch only (branch `wow-souls` of the checkout, uncommitted working tree; the diff is `docs/patches/benilla-everwood-wow-souls-grass.patch`): `ClutterConfig::height`, `CLUTTER_BUILDS_PER_FRAME` 8 -> 3, and the trample in `wind_hook.wgsl`. Everything else is the fork at `f545081e`. **Never run `cargo fmt --all` without `--check` in the wow-souls workspace: it formats the path dependencies too and rewrites the checkout.** |

The lane drivers live in the fork's `benilla-app`, which this game does not depend on, so they are copied
into `crates/wow-souls/src/gfx/` (each file keeps the fork's `MONKEY (...)` / `GFX (...)` comments). What was
changed: `crate::video::VideoConfig` -> `gfx::config::VideoConfig` (a trimmed copy, no window / vsync fields);
`ClientState::InWorld` -> `Phase != Streaming`; `SelfPlayer` -> `Avatar`; `run_mode::dev_affordances` -> the
crate's own `dev` feature; embedded shader paths `embedded://benilla_app/shaders/` -> `embedded://wow_souls/gfx/shaders/`;
the cvar observers of `dynamic_interior.rs`, `monkey_gfx.rs`, `sky_quality.rs`, `zone_skybox.rs` -> one guarded
system in `bridge.rs`; the fork's `GRAPHICS_PRESETS` / `LIGHTING_PRESETS` columns -> `config::Rung`; clippy-driven
rewrites of a few idioms (`as_chunks`, `is_none_or`, `first()`).

| wow-souls file | fork source (`crates/benilla-app/src/`) | Notes |
|---|---|---|
| `gfx/shadow_core.rs`, `character_shadow.rs`, `world_shadow.rs` | `shadow_core.rs`, `character_shadow.rs`, `world_shadow.rs` | the sun shadow rig and its two lanes |
| `gfx/torch_shadow.rs` | `torch_shadow.rs` | cube-map torch shadows; `gfx/carried.rs` stubs `entities::carried_light`'s two markers (no carried lights here) |
| `gfx/volumetric_fog.rs`, `volumetric_light.rs` (+ `.wgsl`) | `volumetric_fog.rs`, `volumetric_light.rs`, `volumetric_light.wgsl` | haze, lamp fog, shadow-mapped shafts |
| `gfx/ssao.rs`, `gfx/shaders/ssao.wgsl` | `ssao.rs`, `shaders/ssao.wgsl` | contact shadows |
| `gfx/shaders/shadow_caster*.wgsl` | `shaders/shadow_caster.wgsl`, `shaders/shadow_caster_cutout_prepass.wgsl` | the invisible caster proxies |
| `gfx/post/bloom.rs`, `grading.rs`, `sun_shafts.rs` (+ `.wgsl`), `mod.rs` | `post/*` | HDR bloom, LUT grading, radial sun shafts; grading's table and LUT strips are read from the compiled-in `gfx/data/` instead of loose files in the `Data` folder |
| `gfx/data/DBFilesClient/MonkeyZoneGrade.dbc`, `gfx/data/World/LUTs/*.blp`, `gfx/data/LICENSE.md` | `Optional/sky-and-grading/data/` | the fork's own project-authored grading data (MIT OR Apache-2.0); no Blizzard asset |
| `gfx/bridge.rs`, `gfx/config.rs` | `dynamic_interior.rs`, `monkey_gfx.rs`, `sky_quality.rs`, `zone_skybox.rs`, `video.rs` (`VideoConfig`), `cvars/mod.rs` (`GRAPHICS_PRESETS`, `LIGHTING_PRESETS`) | re-written as a resource + one bridge system |

**WarcraftXL credit is required.** The fork ports techniques from WarcraftXL (https://github.com/WarcraftXL,
iThorgrim, used with the author's permission, attribution required) into the grading lane (`gfx/post/grading.rs`,
`grading.wgsl`) and into engine files this game links (sky clouds, wind, enhanced water). The ported file keeps
its header line; the package must carry the fork's `THIRD-PARTY.md` (copy it at release).

## Equipment (2026-10-11)

No file is ported. `actor/held.rs` (off hand, fist), `actor/mod.rs` (`Dress`) and `equip/` follow the placement
and slot rules of the fork's `benilla-app/src/entities/equipment/` (`resolve.rs`: the attachment ids 0 shield,
1 right hand, 2 left hand, 5 / 6 shoulders, 11 helm; `mod.rs`: the equipment slot numbers and the eight
armor bodyslots) and the engine calls `CharSections::composite_plan`, `CharacterGeosets::visible_geosets` and
`EquipGeosets`; the slot numbers are vMaNGOS's `EQUIPMENT_SLOT_*` and the starting kit its
`playercreateinfo_item` (race 1, class 1), both read from the local database copy.

## Character creation (2026-10-12)

No file is copied. `src/create/` re-builds the 1.12 `CharacterCreate` screen in our own UI kit and follows the
fork's worked port `crates/benilla-app/src/char_create/` (`screen.rs`, `panels.rs`, `refresh.rs`, `mod.rs`: the
authored layout numbers of `CharacterCreate.xml`, the race / class icon texcoord tables of `CharacterCreate.lua`,
the Alliance / Horde column order, the reset and clamp rules) and `glue_strings.rs` (the `KEY = "value";` parser
`create/strings.rs`). The data is `benilla_formats::CharCreateCatalog` (the engine crate). Text and art are read from the
player's own install at run time (`Interface\GlueXML\GlueStrings.lua`, `Interface\Glues\CharacterCreate\*`),
nothing is embedded.
