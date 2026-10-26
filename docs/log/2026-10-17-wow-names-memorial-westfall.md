# Readable motion names, memorial decal and Westfall

The production runtime uses project motion/rig names rather than source-game module names
or raw animation IDs. DSR remains attribution in comments and provenance; the separate,
optional format/extraction tools remain outside the production workspace.

## Motion naming

- Renamed dsclip/dsloco/dsview/ownanim to motion_clip/locomotion/rig_view/animation_library.
- Resource and pose fields use action/rig terminology. Reference folders use names such as
  human_reference, kobold_reference and wolf_reference. Active clip names describe actions.
- anim/provenance/motion-names.tsv maps all 1,203 migrated clips and records SHA-256 hashes
  of the unchanged numeric payloads. Test-MotionNames.ps1 passed for every original payload.
- Updated locomotion-set lookup and the optional remix classifier to use semantic names,
  including interactions, enchanting, reactions and evade variants. Timings and curves stay intact.
- Indexed all 1,591 current project clips in inventory.tsv, including existing flat overrides
  and the additional project enemy kits. No source installation or binary cache is consulted.

## Memorial

The original generated RGBA decal is embedded from assets/soulbind/memorial-rune-v1.png.
The prompt and generation provenance live beside it. Two outward-facing cutout meshes show
the upright rune on both sides. Removed the black inset and five neon bars; restrained emissive
tint keeps the painted stone/bronze details legible. The WoW install was not edited.

Both faces were inspected in the actual renderer. The observer's optional CLEAN switch hides
debug gizmos for screenshots; it changes no normal camera behavior.

## Westfall

The importer now supports --zone westfall, using client area ancestry and interior/terrain
floors, without Elwynn's level-13 cap. The existing full-world TSV export was reused; no
original database server was started.

Westfall: 1,245 creature sites, 100 local entries including alternate entries, 111 templates
including external quest relations, 48 quests, 937 object sites and 1,191 patrol points.
The combined default data/world/elwynn-westfall.txt has 3,194 sites and 152 distinct quests.
Shared data is deduplicated; pool members are combined under the existing active limits.
All old spawn GUIDs and graveyard IDs remain stable.

New opponents use an exactly compatible project body kit or native WoW M2 motions. No
humanoid/quadruped retarget is stretched onto an unsupported body. Native attacks choose an
available one-hand, unarmed or two-hand sequence. New combat tuning is provisional.

WESTFALL.md, WESTFALL-NPCS.tsv and WESTFALL-QUESTS.tsv contain every imported NPC/quest.
Seven new body groups require custom work: Scourge male, air elemental, carrion bird, crab,
gnoll caster, gnoll melee and harvest golem (21 local entries). The report also separates
casters, named encounters, friendly NPCs, critters, escorts/events and external relations.
The installed Westfall chart now follows the player's area and retains quest pins.

Quest data is not a claim that every server event is implemented. Escort/event and spell
objectives cannot complete merely from empty kill/item counters. Creature/event scripts,
Chicken's interaction unlock and class-specific mechanics still require their world hooks.
The event-credit rule follows
[vMaNGOS Player::CanCompleteQuest](https://github.com/vmangos/core/blob/development/src/game/Objects/Player.cpp).

## Graveyard guidance follow-up

The accepted, unfinished Bind Your Soul objective now gives the Northshire memorial the
client's own animated yellow question mark. It is attached just above the grounded stone's
cap, using its native scale, and disappears when binding completes or the
quest is abandoned. The completed quest then uses the existing Falkhaan return bearing.

The compass now guides this tutorial target at any distance. Ordinary memorials use the
embedded rune within 120 horizontal yards, even with no quest; the focused caption gives
the location, distance and Bind Soul/Bound state. Distant ordinary graveyards stay hidden.
Markers retain the camera-relative bearings, behind-camera arrows, overlap grouping and
modal/death visibility rules of the existing compass.

Tests cover quest acceptance/abandonment/completion, discovery at and just outside 120 yd,
saved binding state, distant tutorial guidance and stationary bearings despite live NPCs.
Test-SoulGuidance.ps1 reproduces five isolated runtime cases and captures before/after
binding, a distant objective, a nearby Westfall memorial, no out-of-range discoveries and
the normal gameplay camera. Coordinate formatting is invariant under the Swedish locale.
The first visual pass caught a sign placed too high in close views; its origin is now
1.75 yd above the ground. The native model's art/bob bounds leave a gap over the 1.64 yd cap.

The concurrent boar test was adjusted by one 50 ms tick at the deadline: repeated f32
subtractions need not reach exactly zero at 0.30/0.60 s. Its under-one-second contract is
preserved at 0.35/0.65 s; production movement and timing values were not changed here.

## Verification

- Scoped formatting and Clippy with all targets and -D warnings passed.
- Workspace tests with WOW_DATA after the guidance follow-up: 1,021 game + 247 world + 4 CLI
  + 1 packaged-world integration + 7 clip-format tests = 1,280 passed. The shared checkout
  also contains the concurrently added combat/balance tests.
- All 1,203 pre-migration motion payload hashes unchanged.
- Dependency audit: all 15 local packages are inside this repository; no DSR production package.
- Live Sentinel Hill: Gryan Stoutmantle placed, installed Westfall map and quest pins visible.
- Live Furlbrow farm: accepted and rewarded Poor Old Blanchy (151) through Verna Furlbrow
  (238), using isolated saves and the scripted item/XP test inputs.
- Live Longshore: Sand Crawler (830) placed with native motion.
- Live Dust Devil: 832 placed at an active site of pool 10003; its database limit is 8 of 16
  sites, so an unselected source coordinate correctly stays empty.
- Test-Westfall.ps1 preserves those reproducible checks and screenshots under cache/.
- Player build (no default features) including final memorial placement compiled successfully.
  SHA-256: 473C3F705532ED9A28B70E9B57C00DB3B5DCC9A1C6F18B77C87A907C50308C9A.
- Isolated packaged combat, retired-selector and world cases all passed from an empty working
  directory with absent DSR/ER/cache paths. Evidence:
  C:/Users/jacob/AppData/Local/Temp/wow-only-aa8737fbf96643c78270279b20a3d944
  (final memorial-placement binary).
- Population/quest evidence: cache/westfall-runtime-48b959849cb44857a09ce07108afbbe5
  (Sentinel Hill), cache/westfall-runtime-f63109d97edc4e6b8a088494a34ec180
  (Poor Old Blanchy and Longshore), cache/westfall-runtime-b18626704ea5402d8c60fb7bff412ab3
  (active Dust Devil sites).
- Final guidance evidence: cache/soul-guidance-31584f37136b465ca570f66f1890478d;
  before/after binding shows the native question mark being removed, Falkhaan's return
  marker appearing, the tutorial completing and site 87047 persisted. Quest-free Westfall
  discovers site 40524; the ordinary distant case remains empty.
- Normal gameplay-camera capture: cache/soul-guidance-f4d255874ac446c9afec5997c8d9c876/
  tutorial-gameplay.png. The full yellow question mark clears the stone and stays in frame.
- Final all-target Clippy passed after applying its range-contains suggestion to the
  concurrently added legfloor test (equivalent assertion); no floor/animation behavior changed.

No commits or pushes were made. All existing user changes were preserved.
