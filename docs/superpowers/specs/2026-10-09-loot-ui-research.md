# Loot, exploration and UI — research and user direction

Date: 2026-10-09. Discussion and read-only survey; no loot/UI implementation yet.
This records requirements and a proposed next slice, not an approved implementation design.

## Confirmed user direction

- Keep WoW quests and baseline loot. Add our own secrets and fixed rewards in Northshire.
  The user selected this hybrid approach explicitly.
- Looting must use the correct contextual animations, including opening chests.
- The user is considering original UI art inspired by Dark Souls / Elden Ring, leaning toward
  Elden Ring, rather than continuing to use extracted Dark Souls UI art.
- The user wants exploration with hidden chests and other discoveries. Decide how rewards
  work and how acquired items are shown together.

## Verified local animation data

Read-only tools: existing `C:\Spel\research\benilla\target\debug\benilla-extract.exe`,
opening `C:\Spel\World of Warcraft 1.12\Data`. No source-install writes or extractor changes.

- Imported `Battered Chest` object entry 2843 uses display 259 and loot table 2265.
- `goanimscan` resolves display 259 to
  `World\SkillActivated\Containers\TreasureChest01.m2`.
- The model owns these sequences, verified with `m2seq`:

  | Animation ID | Role | File slot | Duration |
  |---|---|---|---|
  | 147 | Closed | 3 | 0.133 s, loop |
  | 148 | Open motion | 0 | 0.666 s, clamp |
  | 149 | Opened rest | 1 | 0.100 s, clamp |
  | 146 | Close motion | 2 | 0.667 s, clamp |

- `goanimscan` confirms READY settled uses Closed, ACTIVE mid uses Open, ACTIVE settled uses
  Opened, and READY mid uses Close. Use model animation IDs, not guessed file-slot ordering.
- `m2events` reports `$GO1` at 0 s in Open and `$GO3` at 0 s in Close. Resolve their display
  sound slots if retaining the model's authored opening/closing sounds.
- `Character\Human\Male\HumanMale.m2` owns animation 50 (Loot), file slot 19, 0.500 s,
  clamp. Its presence alone does not establish that the motion fits a chest.

The primary community animation-index source distinguishes player animations 7520
(ground item pickup), 7521 (opening a chest), and 7522 (pickup from a chest):
https://darksoulsdebug.wikidot.com/animation-ids
These are candidate source IDs. Their presence in the installed DSR archives, durations,
events and retarget quality have not been surveyed for this feature. Do not label them built
or choose a substitute solely because its name says Loot.

## Animation acceptance requirements

- Chest opening, pickup from an already open chest, and ground/corpse pickup have separate
  context roles. Reuse a clip only after confirming it fits that role visually.
- Align the player to a short, reachable interaction position in front of the chest. Match
  facing, hand contact, chest height, feet and lid timing; avoid sliding or warping through props.
- Drive the real lid sequence from the player's reach/lift phase. The model's native 0.666 s
  opening duration is a source fact, not a chosen duration for the whole player interaction.
- Persist an opened chest visibly in Opened; do not freeze it in Closed after awarding loot.
- Inventory, claim state and acquisition notification must agree. No reward before the
  selected successful acquisition event, no duplicate reward on repeat input or reload.
- Define interruption and cancel windows from the selected animation during design. An
  interrupted interaction must leave a recoverable, consistent state, including an open but
  unclaimed chest if opening and pickup are separate actions.
- Inspect retargeted clips against their source and validate them live with the chest, not
  just as an isolated animation preview.

## Recommended next slice — proposal

Build one complete chest interaction with a fixed, currently usable reward, original pickup
UI, a minimal inventory view and persistence. Include one ordinary corpse pickup and one
quest-item pickup to establish the distinct presentation and quest hooks. Prove the first
chest in a reachable spot before distributing authored secrets across the valley.

Compared with building the entire UI first, this tests the presentation against real actions.
Compared with implementing loot logic alone, it exposes animation and reward readability
early. Avoid a whole-HUD rewrite or broad equipment support in the first slice.

Suggested presentation: small notifications for ordinary and quest drops; a more prominent
non-modal discovery card for an important fixed reward, with icon, name and one useful effect
sentence. Keep a Recent/New inventory view so discoveries can be found afterward. Explore
original charcoal panels, ivory text and restrained bronze/old-gold detail with Northshire
identity. This visual direction is proposed, not a finished art specification.

Keep original respawning WoW objects separate from new, authored one-time treasures. Quest
items should count on actual acquisition. Integrate with the ongoing quest engine's item,
reward and save semantics rather than introducing a second owner of inventory/progression.
G3b currently plans extracted DS UI; reconsider its visual dependency when that design is
revisited, in light of the user's preference for considering original art.

For exploration, use optional detours, readable environmental clues and fixed useful rewards,
not just more containers. Start with a clear teaching discovery, an optional detour and a
guarded discovery after surveying reachable locations. No finalized placements yet.

Primary exploration references:
- https://www.bandainamcoent.com/news/elden-ring-introduction-part-3-gameplay-overview
- https://news.xbox.com/en-us/2022/05/27/from-softwares-hidetaka-miyazaki-on-the-secrets-of-elden-rings-development/amp/

## Remaining design choices

- Exact first reward and its supported gameplay effect.
- One action that opens and acquires, or separate open / acquire actions; interruption rules.
- Final original UI art direction and how broadly to apply it to existing HUD / quest UI.
- Save integration boundary with G3a/G3c, inventory capacity and failure behavior.
- Chest interaction controls (E / LB already guards; do not silently reuse that input).

No commits, game source edits, test runs or live-animation proof were made for this discussion.

Later follow-up: the user approved the first slice with "Kör". Implementation and measured animation
proof are now in `2026-10-09-loot-built.md`; the proposal and open questions above are historical.

Visual correction, 2026-10-09: the user rejected the first flat UI, supplied the Dark Souls souls-counter
panel as a texture/style reference, and explicitly requested real WoW icons and exact WoW loot sparkles.
They asked to use as much WoW / Dark Souls art as possible. This supersedes the original-art proposal.
The revised implementation uses the installed DS `soul_param/bg_parchment_0` texture, ItemDisplayInfo
icon mappings and native `Particles/LootFX.m2`. Additional visual comparisons: [DS pickup rows](https://cdn-ak.f.st-hatena.com/images/fotolife/t/toshi833/20180528/20180528170305.jpg)
and [DS inventory](https://steamuserimages-a.akamaihd.net/ugc/1818898229050689498/9C62B4EC27C1F7441F300EBCD1E992FB4D67585E/).
