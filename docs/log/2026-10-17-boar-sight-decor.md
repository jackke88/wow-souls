# Boar death and charge, sight through the ground, world objects (2026-10-17)

User (playtest, four screenshots): "Boar: Dödsanimationen är konstig. Charge ska vara en attack så den ska välja en position när den kommer nära nog och springa igenom spelaren - typ som en fireball. Boars attackerar för långsamt. Man ska inte kunna targeta någon under marken och dom ska inte kunna göra aggro. Aggro problem inuti grottan vid kobolds i Elwynn. Eldarna i eldstäderna är inte där - kolla alla vfx'er."

## Boar death (`monster::wow_death`)

Probe of the boar kit: `body_death_08` turns the spine bones 130-140 degrees within 10 frames while the hip root (bone 14) stays put; `body_death_01` ends about 100 degrees twisted. The Armored Tusk rolls its whole body; the boar rig only turns its spine, so the front lay belly-up over standing hind legs. `boar` and `princess` now die with the WoW model's own Death (it always played under the kit death).

## Boar charge (`boar_rush.rs`, `npc.rs`, `pattern.rs`)

- Charge band 1.6-2.8 -> 4.0-10.0 yd (large boars 4.5-12.0): it chooses its spot at range.
- As the clip's burst begins (1.2 s, after tracking through the wind-up) the line is fixed and the run is the distance to the player plus 3 yd (3-13 yd) at 12 yd/s; the burst (clip 1.2-1.5 s) is spread over the yards, so the hit window covers the run. No stop at the player's edge, no turning. The blow is the kit's own (posed tusks), so a roll's invulnerability passes it. Boar kit only (Princess keeps her tuned short + long charge).

## Sight through the ground (`npc.rs`, `lockon.rs`)

- Live before (player on the meadow over Fargodeep Mine, `-9815,125`): 8 kobolds 40 yd below noticed, ran to the point under the player and swung; lockable through the turf (the user's screenshot).
- Now: noticing needs `rise <= AGGRO_RISE` 5 yd and a clear camera-probe ray (eye to chest; terrain, WMO rock and walls block); a fight ends past `LEASH_RISE` 8 yd. Lock-on, switch and keep: `LOCK_RISE` 6 yd and a clear ray from the player's eye; a lock breaks after 2 s out of sight. After: 1 kobold notices (the one standing on the surface by the entrance); inside the mine the 5 on the player's level do.
- "Allt är buggit i grottan" beyond aggro was not reproduced in one unattended run; the mine is very dark. Ask the user what else breaks there.

## World objects (`decor.rs`, instrument `fx_census.rs`)

- Cause: hearth fires ("Cozy Fire"), campfires, forges, stoves are spell-focus game objects; benches, chairs, braziers, crates and mailboxes are game objects too. Only quest objects and chests were placed.
- `decor.rs` places types 5, 7, 8, 19 (917 in Elwynn+Westfall: 193 / 580 / 138 / 6) in a 110/140 yd ring as native effect instances (`loot::vfx::effect`: geometry, looping first sequence, emitters, ribbons), at the database place, yaw and size, no collision.
- Verified: the kobold camp campfire burns; the Goldshire inn hearth burns (logs, flames, embers) and its chairs are there.
- Northshire Abbey: neither the database nor the WMO has a fire for its hearths (census over the whole building: candle flames, glows, a blue bonfire flame only). Not added.
- `WOWSOULS_FX_CENSUS=1`: every 3 s the emitters within 40 yd, nearest 24 with WoW place, drawn state and texture, plus a texture histogram.

Tests: wow-souls 1043 pass; fmt clean; clippy clean on these files (one lint in another session's new `village.rs`).
