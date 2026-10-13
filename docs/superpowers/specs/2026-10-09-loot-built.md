# First loot slice — built, 2026-10-09

The user's "Kör" approved `docs/plans/2026-10-09-loot-design.md`. This adds an actual opening /
pickup / inventory loop, while the separate world/quest work continues. The user's later visual
instructions supersede the first original-art proposal: use real WoW and Dark Souls art as much
as possible, matching the supplied Dark Souls counter panel.
Nothing is committed. The pinned benilla checkout and installed games remain read-only.

## Player-visible result

- **X / pad Y**: open a nearby chest, then press again to take its contents; collect rolled corpse loot.
- **Tab / Start**: inventory, recent acquisitions first. **Up/Down, W/S or D-pad** selects;
  **Enter / A** or the mouse Use button uses a selected Abbey Salve. **Escape / B** or Close exits.
  Inventory holds movement, combat, camera and lock input. It does not pause enemies; damage closes it.
- **V / pad X**: quick-use a salve while alive, idle and below maximum health. One salve restores
  up to 30 health; the save must succeed before either consuming the item or healing.
- The authored **Abbey cache** now sits at rear supplies, WoW `[-8896,-235,80.259]`, with
  **three Abbey Salves**, preserving the old ID and saved claims. The user requested removal
  from the chapel passage; two more caches now occupy optional woodland supply detours.
  See [verified placement pass](2026-10-09-treasure-placements-built.md) for the current layout.
  Existing DB Battered Chests retain their imported loot tables and restock timers (at least five minutes).
- Three distinct DS interaction clips and the real WoW chest lid. Hands are freed during playback;
  alignment uses the existing motor, with a two-second approach timeout and no position teleport.
  Attack, guard, evade, damage, leaving the ground or free-camera mode interrupts interaction.
  These interactions confer no invulnerability.
- Opening saves pending contents; taking saves inventory and the claim together. A canceled pickup
  leaves the chest open and unclaimed. A canceled early opening returns the lid through native Close.
  Once collected, the authored cache remains open and never awards again, including after restart.
- Pickup is a compact Souls row with an actual WoW icon, name and quantity; up to four rows stack
  for a multi-item pickup. Both pickup and inventory use the installed Dark Souls
  `soul_param/bg_parchment_0` panel texture, with nine-slice edges and installed Georgia text.
  Dense inventory rows have native icons, quantity at the right and a warm selected row; mouse
  buttons and pad/keyboard focus work. Inventory draws above the combat HUD, with a dark backdrop
  and inset that preserve the native rough edge. This is our layout using native art, not a port of DS menus.
- Icons resolve imported ItemTemplate display IDs through the installed WoW ItemDisplayInfo:
  Broken Fang (7073 → display 6002 → INV_Misc_Bone_08), Melted Candle (755 → 6677 → INV_Misc_Candle_01).
  Abbey Salve is our custom reward and borrows WoW's INV_Potion_49, also used by Minor Healing Potion.
- Lootable dead units use WoW's actual `HARDCODED Loot Art` row 14, `Particles/LootFX.m2`, attachment
  19, its native first sequence (2.400 s), bones and four particle emitters. No particle rate, size,
  colour or timing retuning. Taking the items immediately reaps the effect; revival/despawn also
  cleans it up. This replaces the placeholder sphere. Sparkles currently mark corpse loot.

## Source and animation proof

`wow-souls-dsr loot` extracts optional `cache/dsr/interaction.txt` plus paired `human_male` / `c0000`
clips. Normal `extract` includes it too. Cache role names pin the intended clip and check the common
source metadata. Retargeting goes through the existing 15-degree bone/cross drift and 0.08-yard
contact-foot gates. Those limits were not relaxed.

| Role | Installed DS clip | Frames @ 30 fps | Duration | Our commit time | Max bone / cross drift | Contact foot gap |
|---|---|---:|---:|---:|---:|---:|
| Ground/corpse pickup | a00_7520 | 56 | 1.833 s | 0.800 s | 0.06 / 2.73 degrees | 0.038 yd |
| Open chest | a00_7521 | 61 | 2.000 s | 1.200 s | 0.06 / 3.05 degrees | 0.041 yd |
| Take from open chest | a00_7522 | 61 | 2.000 s | 1.000 s | 0.06 / 2.74 degrees | 0.040 yd |

All three have zero root travel and no invincibility window. The commit times use the observed
flag-87 start in each source clip as this prototype's success point; that is not a claim to have
reimplemented Dark Souls' interaction ESD. Extraction prints all source events and both hand samples.

The native model is `World/SkillActivated/Containers/TreasureChest01.m2`, display 259, scale 0.85:
147 Closed, 148 Open (0.666 s), 149 Opened, 146 Close. Lid playback waits until player time 0.533 s,
then follows the measured lift until 1.200 s. It stays Opened after the saved opening. Partial
opening interruption seeks the matching point in Close; baseline restock also plays Close.

Source/retarget comparisons and the real chest were inspected in game. Files in
`C:\Users\jacob\Documents\GitHub\wow darksouls\docs\evidence\`:

- `loot-open.png`: crouched player reaching the native opening lid, feet on the Abbey floor.
- `loot-source-{open,take,ground}.png`: source skeleton and human at the same phase.
- `loot-acquired.png`: authored reward, open lid, native DS pickup frame, real WoW potion icon.
- `loot-inventory.png`, `loot-inventory-wide.png`: dense inventory at 1000×750 and 1280×720.
  These use an isolated ten-item UI fixture, with Broken Fang selected, to inspect item icons and
  list/detail layout. They do not represent ten items earned in the live pickup check.
- `loot-inventory-preview.png`: same fixture in the normal orbit camera, with recording mode
  hiding developer text and no observer gizmos; clean view for reviewing the icon/art direction.
- `loot-corpse.png`: ordinary Melted Candle pickup from the actual killed kobold.
- `loot-sparkle.png`: native WoW loot effect on that lootable dead unit before collecting.
- `loot-use.png`, `loot-interrupted.png`, `loot-interrupted-take.png`: supporting check frames.
  The roll-interruption views enter nearby geometry, so their logs/save state are the interruption
  proof; those frames are not good evidence for judging the lid's closing motion.

## Ownership and quest boundary

`wow-world::inventory::{Inventory,Progress}` is the canonical owned-items foundation. The strict
version-1 text save holds item counts, recent order and pending/claimed chest states. It publishes
through a synced same-directory temporary file and rename before updating runtime ownership.
Failed writes leave previous ownership unchanged. A corrupt save blocks grants with a visible
error instead of overwriting the user's progress. Default save:
`%LOCALAPPDATA%\WoWSouls\items-v1.txt`; `WOWSOULS_ITEMS_SAVE` overrides it. Unattended runs otherwise
use isolated process-specific paths under `cache/loot-check`, never the player's normal save.

`wow-world::loot::roll` covers independent, grouped/equal-chance and recursively referenced imported
loot rows, with a recursion bound. A corpse rolls once when Slain is received. Its collectible marker
disappears on acquisition, despawn or revival; revival also cancels an in-progress pickup.

The gameplay quest/loot integration is **not wired yet**. `loot::Acquired` emits only after durable
acquisition; `QuestLootEligibility` is empty until the real quest log supplies still-needed item IDs.
Negative-chance quest drops are tested with explicit eligibility and stay unavailable without it.
G3 must use the shared Inventory, wire actual-acquisition progress and merge save ownership; do not
add another competing item owner. No fake quest acceptance or live quest-completion claim here.
Reference-table group restrictions are not yet generalized; the imported Northshire reference rows
used by this slice all have group zero. Ordinary equipment/food have names and categories, but only
the custom salve is currently usable; equipping, vendors, copper and other item effects remain G3/G4.

## Validation

Final gates and rebuilt-executable checks are recorded in `cache/loot-work/` (local, ignored).
The workspace run with **DSR_DIR and WOW_DATA set passed 527 tests**: dsr-cache 53,
dsr-formats 51, game 356, wow-world 61 + 2, wscl 4. Native-art cases verify the actual LootFX model
and the imported Broken Fang / Melted Candle display-to-icon mappings against the installed MPQs.
New meaningful cases cover inventory overflow,
reload/duplicate reward, corrupt save, failed publication/consumption, quest eligibility, grouped
and cyclic-reference loot, actual Bevy-driver pad input, damage interruption, inventory ownership
and a revived corpse during pickup.

Live flow checks passed the ECS fault gate: opening saves pending salves without granting them;
reload then take grants exactly three; repeated input does not grant again; interrupted take keeps
the open/pending save; inventory prevents a scheduled attack and stick movement. Salve use after a
Garrick hit restored 85 → 100 and durably changed three → two. Seeded real kobold loot rolled table
80 into one Melted Candle (755), collected it with a00_7520 and saved exactly one.

The revised native-art executable passed open/reload/take/repeat, real Melted Candle acquisition,
and a ten-item inventory/input check at 1000×750. Native loot VFX built four emitters and cycled
live particle counts through repeated 2.4 s loops, with all four textures resident and draw gates
admitting them; the actual sparkle was inspected in game. Acquisition changed roots/emitters/live to zero.
Final gates passed: `cargo fmt --all -- --check`, workspace/all-target clippy with `-D warnings`,
527 workspace tests with both install paths set, and rebuilt `wow-souls` plus the loot extractor.
The final inventory layering was inspected at 1280×720 and 1000×750; its title stays above the
combat HUD, fonts have no missing-glyph boxes, and selection/icons/counts remain legible.
Physical mouse/controller hardware was not driven during unattended checks; pad buttons were tested
against the real ECS driver, and the UI was inspected at both aspects.

Install metadata comparison against the pre-session listings: zero changes in 175 WoW files
(excluding the separately managed realmlist.wtf) and 6038 DSR files. Benilla still resolves to
2e82d34c1b561e3bb033f7cd910036ba63ce1760 with a clean working tree.

The optional supply-cache placement pass is now built and verified; see
`docs/superpowers/specs/2026-10-09-treasure-placements-built.md`. Next: connect this shared inventory
to G3's real gameplay quest state and later add guarded discoveries. The broad HUD redesign remains separate.
