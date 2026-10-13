# D2b — blood from WoW's own hit effect (design)

Date: 2026-10-08. Sub-project D2b of the A-H decomposition (D1 sound and D2a hit-stop / knock-back /
shake are built). The user's wish: "blood and sparks". Nothing here is committed (the user commits).

## Goal

A landed blow, either way, shows the blood spurt WoW 1.12 itself plays: a short particle burst on the
victim's chest or back, red for the kobold and the human, small or large by the blow. It replaces the
debug gizmo cross (`hits::flash_impacts`), which stays only as the fallback when the effect cannot be
built.

## What the install says (surveyed 2026-10-08)

- WoW 1.12's melee hit visual **is** the blood spurt: `0x624530` -> `0x625010` in the client, in
  benilla's `creature_anim/blood.rs` (crate-private, so ported here with attribution). The chain:
  victim -> `CreatureDisplayInfo.BloodLevel`, else `CreatureModelData.BloodID`, else the first
  `UnitBloodLevels` row -> `UnitBloodLevels` row per violence level -> `UnitBlood` row -> four
  `SpellVisualEffectName` ids (front small, front large, back small, back large) -> a model path.
- Both our bodies take the same row: the kobold (display 365, model `Kobold`) and the human male
  (display 49, model `HumanMale`) have `BloodLevel` 0 and model `BloodID` 1. `UnitBloodLevels` row 1 at
  violence 2 is `UnitBlood` 1, red:

  | | small | large |
  |---|---|---|
  | front | `Particles\BloodSpurts\BloodSpurt.m2` | `...\BloodSpurtLarge.m2` |
  | back | `...\BloodSpurt.m2` | `...\BloodSpurtLarge.m2` |

  (The DBC names say `.mdl` / `.mdx`; the chain holds `.m2`.) Violence 2 is the client's default for
  enUS (benilla `VIOLENCE_LEVEL`).
- `BloodSpurt.m2` is particle-only: 4 emitters, among them red droplet sprites (100/s for about 0.33 s,
  life 0.5 s, gravity) and an additive white-orange glow ball (200/s peak for about 0.1 s, life 0.15 s).
  That glow is the "spark": 1.12 has no separate spark effect (`Hit Splat` rows that name
  `HitSprite.m2` are marked `zzOLD__` and no table reaches them), and Dark Souls shows none on flesh.
  So there is no spark system in this sub-project; the spurt's flash is it.
- Both bodies carry the spurt's attachment points: id 15 (front) and id 16 (back) on the kobold (bones
  32 and 33) and on `HumanMale` (bones 56 and 57), at chest height.
- The machinery is public in benilla-world: `ParticlePlugin` (in `WorldPlugins`), `spawn_emitter(commands,
  &ModelEmitter, placement, EmitterFrames, EmitClock)`, and `benilla_assets::M2Model.emitters`. The
  tables load through public `benilla_formats::{load_blood_catalog, load_spell_visual_catalog}` and
  `CreatureCatalog::model(display)` (`blood_display`, `blood_model`), the way `audio.rs` loads its own.
  benilla-app's effect lane (`entities/spell_fx`) is crate-private and heavy (parts, materials, rig); a
  particle-only model needs only its emitters.

## Design

### 1. `blood.rs`

- `BloodTables { blood: BloodCatalog, visuals: SpellVisualCatalog }` as a resource, loaded once from
  `Catalogs.chain` on entering `Playing` (absent tables -> one warning, no spurts, the gizmo cross stays).
- Pure, unit-tested: `is_front(victim_forward, victim_pos, attacker_pos) -> bool` (the sign of the flat dot,
  `>= 0` front; an unresolved attacker counts as front, as the client does); `large(level) -> bool`
  (`level >= 2`: R2, the kobold's `big`, `run_strike`, `body_check`; the stand-in for the client's crushing
  bit, which our blows do not carry); `m2_path(name) -> String` (`.mdl` / `.mdx` -> `.m2`).
- `spurt(tables, creatures, display, front, large) -> Option<String>` runs the table chain above.
- Triggers: a `Hit` (the player's blow on a creature) and a `PlayerHurt` (a creature's blow on the player),
  each read in `Update` by a system of its own. The victim's display id comes from its `ActorModel`; the
  attacker's position from `Hit.from` / the `from` entity's `Transform`.

### 2. The spurt entity

- `Spurt { path, model: Handle<M2Model>, born: f32, built: bool }` on a root entity that is a child of the
  victim's attachment anchor (`RigPose::anchor_for(commands, victim, bone)`, the held-item pattern in
  `actor/held.rs`), at the attachment's offset. One spurt per (victim, front / back): a new one replaces the
  old, so a busy fight does not stack them (the client's rule).
- When the `M2Model` has loaded, `build_spurts` calls `spawn_emitter` for each `ModelEmitter` with
  `EmitterFrames { owner: Some((root, [0.0; 3])), anchor: Some(root), alpha: None, light_node: None,
  on_owner_loss: OwnerLoss::Free }` and `EmitClock::Effect(None)` (the spawn-age clock, so a fresh play opens
  its one-shot rate tracks at phase 0). The emitters ride the root; the effect model's own bone motion is
  ignored (the model is static; if the picture shows it matters, a rig is a follow-up).
- `reap_spurts` despawns the root (and with it the emitters, `OwnerLoss::Free`) once it is older than
  `SPURT_LIFE`: the longest rate track plus the longest particle life, measured from the model's emitters
  (about 0.7 s + 0.5 s for the small one), with a 2 s cap. A model that never loads is dropped after 10 s.

### 3. Switches, fallback, logs

- `WOWSOULS_BLOOD=0` turns the spurts off (the gizmo cross returns). Particle density is the engine's
  `ParticleTuning` (default full).
- `hits::flash_impacts` draws only while no spurt can play (tables missing or `WOWSOULS_BLOOD=0`).
- Log line per spurt: `blood: <who> <front|back> <small|large> <path>`; and `WOWSOULS_TRACE=1` adds the live
  particle count of a spurt every 0.1 s (`ParticleEmitter::live`).

## Not in this sub-project

Ground blood splats (1.12.1 never reads them), blood on the weapon, wound decals, green or black blood
(only red bodies exist in the slice; the table chain already handles them), a spurt on a lethal blow beyond
the normal one, Dark Souls' own blood effects (FFX in the install, a separate extractor job), persistent
gore.

## Verification

- Unit tests: `is_front` both ways and for a degenerate attacker; `large` at levels 1, 2, 5; `m2_path`; the
  lifetime from a def with a known rate track; "replace, not stack" on the (victim, side) key. Install tests (with
  `DSR_DIR` / `WOW_DATA`): the kobold and the human male resolve to `BloodSpurt.m2` small / `BloodSpurtLarge.m2`
  large for front and back; both models have attachments 15 and 16; the spurt model loads and has 4 emitters.
- Live, unattended: R1 on a kobold 2 yd away logs `blood: kobold front small ...`, the particle count rises above
  0 and returns to 0, the root is gone after the life; R2 logs `large`; a kobold blow on the player logs
  `blood: player ...`; hit from behind logs `back`; `WOWSOULS_BLOOD=0` logs none and shows the cross. One
  screenshot at the hit (`WOWSOULS_SHOT`, a close `WOWSOULS_OBSERVE`) for the user's eyes.
- Gates: `cargo fmt --all -- --check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`,
  `cargo test -j 4 --workspace` (with `DSR_DIR` / `WOW_DATA` exported in the same shell command), both installs
  identical before and after.

## Open for the user (their eyes)

How the spurt looks on the kobold and on you (size, brightness, where on the chest); WoW's red is brighter
than Dark Souls' dark, thick blood, and there is no Dark Souls version in this sub-project. Whether the flash
reads as the spark you wanted.

## Built (2026-10-08)

Plan `docs/superpowers/plans/2026-10-08-blood.md`, executed inline. Deviations from the design above:
- The lifetime is the constant `SPURT_LIFE` 1.6 s, not computed from the emitters; the live particle trace
  shows the pool drained by about 0.7 s after the build (peak about 40 live particles at +0.25 s).
- `m2_url` already maps `.mdl` / `.mdx` to `.m2`, so there is no `m2_path`.
- "Replace, not stack" is checked live (two hits 0.7 s apart on the same side keep one spurt), not by a
  unit test; the install test pins the table chain for both bodies instead.
- `ActorModel` gained `display` so a body knows its blood.
Evidence: `docs/evidence/blood-r1-3.85*.png` and `blood-r1-3.92*.png` (`-zoom` = the kobold's chest, 4x): the
white-orange flash and the red droplet streaks, as in WoW.

### Turned up (2026-10-08, the user asked for more blood)

WoW's spurt is thin (one droplet emitter, 100 a second for 0.33 s, half-extent 0.03 yd), so `build_spurts` now
spawns each alpha-blended (droplet) emitter `DROPLET_COPIES` 3 times, each with its own random seed (the
placement seeds the emitter), at `DROPLET_SIZE` 1.6 times the size (`ParticleEmitter::set_size_scale`); the
additive flash emitters stay single. Live particles at the peak go from about 40 to about 80.
`WOWSOULS_BLOOD=<x>` scales the copies (default 1, `0` off, capped at 10). Pictures `docs/evidence/blood2-r1-3.85*.png`,
`blood2-r1-3.92*.png` against the first pair `blood-r1-*`.
