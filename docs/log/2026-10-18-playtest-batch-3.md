# Playtest batch 3 (2026-10-17/18) — PAUSED mid-task

User list (13 items). Last build compiled clean; nothing committed; full test/clippy/fmt gates NOT run yet.

## Done (built, some verified live)
- Rarity colors: pack detail/row names + loot notice use `quality_color` (`loot/ui.rs` `quality_of`); loot notice smaller (rows 50 px, icon 34, text 17, 36 % margins).
- Equipment page: preview list of pack items for the cursor slot, each row shows deltas ("+31 Armor  +1 Agi", green/red) via `equip::card_at` + `short_delta`; chosen item shows "Compared with" + compare lines (`equip/ui.rs`).
- Gnoll stun-lock: `Poise::hit_chained` (monster.rs) — after 2 flinches within 2.5 s the creature shrugs blows off 2.2 s; player poise unchanged. Test added.
- Boar charge: x2.2 damage, knock 1.8, poise 45 when it connects (`boar_rush::DAMAGE/KNOCK/POISE`, applied in `npc.rs` strike).
- Hogger `damage_scale` 0.88 -> 1.25.
- 100 % drops: `wow_world::loot::roll_with(.., Chances::Always)`; every loose row + one per group. `WOWSOULS_LOOT_CHANCES=wow` restores. NOTE: a kobold now drops ~11 items.
- Spiders: `slope.rs` tilts spider models to the ground plane (ModelFrame); latch hold clamped against walls/ground (`npc.rs`).
- Hairy spiders (Forest Spider, kit `tarantula`): leap+latch replaced by Crag Spider spit `a00_3006` -> `anim/source/tarantula{,_reference}/body_attack_07.wsanim`, manifest `attack spit` + `attack-bullet spit`, WoW Venom Spit missile (`fx::Kind::VenomMissile`, GooBolt_Missile_Low). Bolt origin lifted 0.6 yd over feet. Verified: spits land (28 dmg). dsr-cache TARANTULA_KIT has own attacks.
- Murlocs: new `Tier::Frenzy`; MURLOC_KIT = lunge `a00_3005` + 3-hit flurry `a00_3007` (tamed) -> `murloc{,_reference}/body_attack_06/08`, manifest + pattern bands + `behavior::preferred`. Verified live: both used, flurry hits land 22.
- Graveyards: `soulbind` save v2 (`known` list); death respawns at nearest bound graveyard (`Binding::respawn_site`). Tests green.
- Manifest backup before spit edit: scratchpad only. Provenance TSV not extended (fixed 1,203 rename record).

## Not started / next
1. Swimming + drowning (was reading code): plan = `swim.rs`: latch on depth > 0.75*CAPSULE_HEIGHT (exit -1/36), surface via `WorldPoint::water_surface_at`; in `controller::drive_motor` branch to a swim step (slide_body, no gravity, speed 4.72 / back 2.5, dive along `OrbitCam.pitch` when moving forward, rise capped at feet = surface-0.75h, jump = swim up); set `motor.grounded=false` so DS gait/stance step aside; add WoW Gait SwimIdle 41 / Swim 42 / SwimBackwards 45 in `actor/anim.rs`; block attacks/rolls while swimming; breath 60 s when head under surface, then 10 % max HP every 2 s; breath bar HUD. Reference: `C:\Spel\benilla\crates\benilla-app\src\player\swim.rs`.
2. Torch: shop item, extra gear slot (belt), hangs at belt, burns out after X min, light source.
3. Graphics: rain indoor light switch lag; overbright foggy shot (graphics chat owns daycycle/weather/gfx — coordinate).
4. Gates: fmt (-p list), clippy, full tests; HANDOVER update.

## Resumed 2026-10-18 (later)
- **Swimming** (`motor/swim.rs`, `controller::drive_motor`): latch at depth > 0.75·h (exit −1/36), no gravity, 4.72 / back 2.5 yd/s, forward stroke dives/climbs with the camera pitch past a 0.3 rad dead zone, Jump held swims up, climb capped with head out (`cap_redirect`), body lean on `ModelFrame` (`combat::tumble`), WoW gaits SwimIdle 41 / Swim 42 / SwimBackwards 45 (`actor/anim.rs`), no combat actions afloat (`combat::act`), `motor.grounded=false` so the DS gait/stance step aside. `WOWSOULS_DIVE=<pitch>` = test camera pitch. Verified live in Crystal Lake (-9440,-400): latch, rise to surface, swim clip + ripples, swims out north.
- **Drowning** (`breath.rs`): 60 s breath with head under the surface, then 10 % max HP every 2 s (`PlayerHurt` level 0), refill 6 s; blue mirror bar under the compass. Verified live: 6 dmg ticks every 2 s on a 60 HP char.
- **Torch**: `Slot::Torch` (index 20, key `torch`, `inv::TORCH` 900, `BELT_ROW`), item 9100001 "Torch" (display 12312, 25 c = 100 souls) added by `wow_world::torch::add_to` in `loot::load` to the 15 traders who sell water. `torch.rs`: burns 600 s while worn (saved in `torch-v1.txt`, a fresh torch when fuel is 0), burnt out = removed from slot + toast; hung via `WornKind::Belt` (attachment 10) and placed against the body by `torch::orient` (left hip, head up-back); engine `point_light` + `FlameFlicker::Torch` + `HeldLight` at the head. Verified live day/night (shots); no flame particles on the model (light only).
- **Rain indoors**: vendored engine `lighting/resolve.rs` local fix: WMO fog crossfade in 1 s (`WMO_FOG_RAMP_IN_PER_SEC`), out stays 4 s; engine test updated + passing. Second cause untouched: the switch keys on the CAMERA being inside the building.
- **Overbright fog**: NOT reproduced (Crystal Lake, rain 1.0, High, several angles). Needs the user's time/place; graphics chat territory.
- Gates: fmt clean; clippy clean except `village.rs:243` explicit_counter_loop (another chat's file).
