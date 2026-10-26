# Goldshire original ambience inventory — 2026-10-17

Read-only MPQ inspection used the project's existing `benilla_formats` compiled
library from a temporary Rust probe; no source archive was modified. Goldshire
reference point: WoW `[-9464,65]`, nine surrounding Azeroth ADTs.

## Native flight already present

The original MDDF placements east of Goldshire include:

| UID | Model under `world\critter\` | WoW position |
|---|---|---|
| 13080 | `birds\bird02.mdx` | `[-9430.047,-130.945,49.415]` |
| 13188 | `birds\bird02.mdx` | `[-9450.281,-146.055,57.117]` |
| 13194 | `birds\bird02.mdx` | `[-9450.262,-197.898,51.145]` |
| 13289 | `birds\bird02.mdx` | `[-9482.891,-183.816,57.982]` |
| 13431 | `birds\bird02.mdx` | `[-9504.521,-303.275,54.915]` |
| 65419 | `butterflies\butterflyorange01.mdx` | `[-9476.994,-81.820,59.368]` |

Purple and white native butterflies also surround the southern, western and
northern approaches. Preserve these placements rather than doubling their density.

Bird02 has one sequence and five moving bones. Its authored all-animation local
box is `[-36.334812,-13.470577,8.820747]` to
`[30.321472,4.1873794,15.970191]`, sphere radius `34.66258 yd`. The body is tiny;
the flight circuit covers approximately **67 × 18 × 7 yd**. New buildings or
supplementary flight placements must consider the whole circuit. Bird01 is similar.

The orange/purple/white butterflies have one sequence-driven moving bone plus
five global-sequence channels (approximately 10-second cycles); their flight
boxes span approximately 4.3 × 4.0 × 0.93 yd.

## Existing runtime

- `vendor/benilla/crates/benilla-world/src/doodad_anim/mod.rs` already animates
  both ADT MDDF and WMO MODD placements, weighted idle variations and global bones.
- `terrain_stream/spawn/assemble.rs` creates their hosts and skins.
- `terrain_stream/spawn/mod.rs::m2_anim_bound` preserves the authored flight box;
  regression tests explicitly cover birds disappearing when bind-pose bounds cull.
- The lazy rig budget wakes visible animated props and parks distant ones.
- `WOW_CAPTURE` deliberately disables doodad rigs and freezes material clocks.
  It is unsuitable for judging bird motion. Use game film/frame-series instruments
  without that environment variable.

## Original audio, clock and critters

Goldshire (AreaTable 87) inherits Elwynn's native SoundAmbience row **35**:

- day kit **4183**, `ForestNormalDay`,
  `Sound\Ambience\ZoneAmbience\ForestNormalDay.wav`;
- night kit **4184**, `ForestNormalNight`,
  `Sound\Ambience\ZoneAmbience\ForestNormalNight.wav`.

Native zone music is row **1**, `Zone-Forest`, kit **2523** for both phases.
`crates/wow-souls/src/daycycle.rs` drives the saved clock; `zone.rs` uses day from
05:30 through 20:59 and crossfades ambience/weather/interior changes. Unattended
runs require `WOWSOULS_AUDIO=1` or `null` to exercise audio; otherwise silent is
expected. No synthetic village sound bed is needed.

The imported Goldshire area has original chickens, rabbits, deer and fawns.
Global density defaults to 0.5 (`world.rs::thin`). Applying that exact GUID-order
rule leaves two chickens, two deer and one fawn in area 87, plus rabbits and more
animals on the immediate approaches. `critter.rs` preserves grazing/wandering;
proximity startle is opt-in through `WOWSOULS_STARTLE=1`.

Verified AnimationData names for civilian activities are 60 `EmoteTalk`,
62 `EmoteWork`, and 71 `EmoteSleep`. Runtime must only play an activity if its
actual model contains the clip.

## Acceptance checks

Inspect day, evening and night in motion, with `WOW_CAPTURE` absent. Confirm
flight remains visible while orbiting the camera; house roofs and trees must not
intersect the full circuit. Compare the western extension with the existing
eastern flights before adding one or two native birds. Preserve original
butterflies, chimney particles, water animation, ground clutter and small animals.
Use `WOWSOULS_AUDIO=null` for unattended audio resolution logs and a device run
for actual balance. Native doodad sound-event host bookkeeping exists in the
engine, but this game's audio module has no general `$DSL`/`$DSO` dispatcher;
do not claim every native positioned prop sound has been restored.
