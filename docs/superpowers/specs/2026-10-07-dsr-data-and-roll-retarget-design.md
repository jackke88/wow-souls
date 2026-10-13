# DSR data reader and roll retarget: design

Date: 2026-10-07. Sub-projects A (DS data reader) and B (animation retargeting, roll prototype) of
the Dark Souls integration. Later sub-projects (C player movesets, D impact, E enemy movesets,
F DS HUD, G playable Northshire, H WoW abilities) get their own specs.

## Goal

Read Dark Souls Remastered's own data (animations, animation events, params) from the user's
install, and play one DS animation, the medium forward roll, on the WoW human model. This settles
whether DS motion on WoW models is good enough to build the weapon movesets (C) and enemy
movesets (E) on.

## Success criteria

1. Side by side, same camera, same frames: the DS player skeleton playing the original roll next
   to the WoW human playing the retargeted roll (`WOWSOULS_DSVIEW`). The user judges the look.
2. Numbers, checked by trace and tests:
   - the roll's duration equals the DS clip's length (to one 30 fps frame);
   - its ground distance equals the DS root motion scaled by the body-height ratio (±2 %);
   - its invulnerability window equals the DS invulnerability event's window from the roll's TAE
     (to one frame);
   - no mesh tearing: every mapped WoW bone stays within 15° of its DS counterpart's world
     direction in every frame.
3. With no cache present the game runs exactly as today (procedural roll) and logs one warning.

## Non-goals here

Movesets, other animations, enemy animations, params beyond what the roll needs, the DS HUD,
Northshire content, WoW abilities. No change to benilla.

## Facts about the install (measured 2026-10-07)

- Loose files, no `.bdt` archives: `chr/`, `param/GameParam`, `paramdef/`, `menu/*.tpf.dcx`.
- `chr/c0000.anibnd.dcx`: DCX (`DCX\0`, `DFLT`, zlib) wrapping BND3 (`BND307D7R6`); 117 entries:
  `Skeleton.hkx` and 116 TAE files (`taeNew\x64\aNN.tae`).
- `chr/c0000_a0x.anibnd.dcx` … `c0000_a5x`: the player's animations (202 in `a0x`), each a Havok
  2015.01 tagfile (`TAG0`, `SDKV20150100`), every one `hkaSplineCompressedAnimation`; root motion
  in `hkaAnimatedReferenceFrame`.
- Player skeleton: about 60 named bones (`Master`, `Pelvis`, `Spine`, `Spine1`, `Neck`, `Head`,
  `L_/R_Clavicle`, `UpperArm`, `Forearm`, `Hand`, fingers, `Thigh`, `Calf`, `Foot`, `Toe0`, twist
  and weapon/shield bones).
- WoW HumanMale (display 49): 119 unnamed M2 bones; a few carry KeyBoneIDs (SpineLow, Head, …).

## Architecture

```
crates/
  dsr-formats/      pure parsing, no Bevy, no file system policy
    dcx.rs            DCX "DFLT" → bytes
    bnd3.rs           BND3 → named entries
    param.rs          PARAM + PARAMDEF → typed rows
    tae.rs            TAE → per-animation events (type, start, end, raw params)
    hkx/tagfile.rs    Havok 2015 tagfile: sections, type table, objects
    hkx/skeleton.rs   hkaSkeleton → bones (name, parent, reference pose TRS)
    hkx/spline.rs     hkaSplineCompressedAnimation → per-bone TRS samples
    hkx/refframe.rs   hkaAnimatedReferenceFrame → root motion samples
  dsr-cache/        binary `wow-souls-dsr extract`: the only code that reads the DSR install
    extract.rs        DSR install (read-only) → cache/dsr/
    retarget.rs       DS clip + bone map + both skeletons → WoW-skeleton clip
    bonemap/human_male.ron   DS c0000 bone → WoW HumanMale bone index, by hand
  wow-souls/        the game
    dsclip.rs         loads cached WoW-skeleton clips; plays one on a rig in PosePost
    dsview.rs         instrument: side-by-side DS skeleton vs WoW human
```

`dsr-formats` knows nothing about WoW or Bevy. `dsr-cache` is the only crate that touches the DSR
install and the only one that knows both skeletons. The game reads only `cache/dsr/`.

## Data flow

1. `wow-souls-dsr extract` reads `chr/c0000.anibnd.dcx` (skeleton, TAE) and the roll's clip from
   `c0000_aNx.anibnd.dcx`, decodes it to 30 fps samples, and writes:
   - `cache/dsr/c0000/skeleton.ron` (bones, parents, reference pose);
   - `cache/dsr/c0000/clips/<id>.bin` (decoded samples: frame count, bone count, per frame per
     bone translation, rotation, scale as little-endian f32; root motion track);
   - `cache/dsr/c0000/tae/<id>.ron` (events);
   - `cache/dsr/human_male/clips/<id>.bin` (the retargeted clip, same layout over the 119 WoW
     bones);
   - `cache/dsr/manifest.ron` (extractor version; each source file's size and modified time).
2. The game loads `cache/dsr/manifest.ron` at startup. A missing or stale cache (version or source
   mismatch) is ignored with one warning; the game keeps its procedural roll.
3. When the cache holds the roll, the evade uses it: duration from the clip, travel from its root
   motion (per-frame displacement into the motor instead of the `(3u - u³)/2` curve),
   invulnerability from its TAE event, and `dsclip` writes the bone locals; the ModelFrame
   somersault stays off.

## Identifying the roll

The roll's animation id is chosen by data, not guessed: among the c0000 clips whose TAE carries an
invulnerability event, the forward medium roll is the one whose root motion is straight ahead,
about 4-5 m long, under 1 s. The community animation-name tables (DSAnimStudio's DS1 names) are a
cross-check. The chosen id is written in the cache manifest and in the spec's follow-up notes.

## Retargeting method (bind-aligned delta transfer)

Axes: DS model space is converted once into Bevy model space (a fixed basis change, found from the
reference pose: up from Pelvis → Head, forward from the roll's root motion).

For each mapped bone pair (DS `d`, WoW `w`):

- DS world rotation at time t: `R_d(t)`; at reference pose: `B_d`. Delta from bind:
  `D(t) = R_d(t) · B_d⁻¹`.
- Bind alignment: `A = arc(dir_w_bind, dir_d_bind)`, the rotation taking the WoW bone's bind
  direction (bone → first mapped child) onto the DS bone's bind direction. This absorbs T-pose
  versus A-pose differences.
- WoW world rotation: `R_w(t) = D(t) · A · B_w` (M2 bind rotations are identity, so `B_w = I`).
- WoW local rotation: `parent_world(t)⁻¹ · R_w(t)`; local translations stay the WoW bind pivots.

Root motion: the DS reference-frame track, scaled by `h_w / h_d` (pelvis height at bind) and
applied to the body through the motor, never baked into bones. Pelvis height changes (the duck in
the roll) go on the WoW pelvis bone, scaled by the same ratio.

Unmapped WoW bones (fingers beyond the map, cloth, face) keep their Stand-clip pose. Fingers keep
the existing `HandsClosed` grip overlay.

## Playback in the game

`dsclip` holds `DsClip { samples, fps }` per cached clip and a `DsPlaying { clip, t, rate }`
component. A system in `benilla_world::rig_anim::PosePost` writes the sampled locals (slerp between
frames) into `RigPose.locals` for mapped bones and raises `pose_dirty`. `anim::drive` already
yields while an action owns the body. This uses the same public window benilla's own body twist
uses; benilla is not modified.

## Side-by-side instrument

`WOWSOULS_DSVIEW=<anim id>`: spawns the DS skeleton as gizmo lines (bones, joints) 1.5 yd to the
right of the avatar, plays the original clip and the retargeted clip in lockstep, loops, and
logs per frame: time, root distance, per-bone world-direction error (max, mean). Combined with
`WOWSOULS_OBSERVE` and `WOWSOULS_SHOT` this produces the comparison frames.

## Error handling

- Extractor: a missing install, an unreadable file or an unknown Havok class is a clear error
  naming the file and offset; nothing is written for that clip; other clips continue.
- Game: a missing or stale cache, or a clip whose bone count differs from the model's, is skipped
  with one warning; the procedural roll remains.

## Testing

- `dsr-formats`: unit tests on hand-built buffers (DCX of a known payload, a two-entry BND3, a
  two-field PARAM/PARAMDEF, a two-event TAE); the spline decoder against hand-computed values
  for a synthetic one-bone track (quantized quaternion and B-spline control points).
- Install smoke test (`#[ignore]` unless `DSR_DIR` is set): every c0000 clip decodes; unit
  quaternions; no frame-to-frame jump above 60°; skeleton has the expected named bones.
- `retarget`: synthetic two-skeleton tests (differing bind poses): the WoW bone direction follows
  the DS bone direction within 1°; twist is carried.
- Game: the side-by-side frames and the numeric trace above; existing tests stay green.

## Locality rule

Everything read from the DSR install, decoded or derived from it (cache, dumps, screenshots), stays
on this machine and is gitignored (`/cache`). Only our code and hand-made tables (the bone map) are
source.

## Risks and fallback

- Havok spline decoding is the largest porting job. If it stalls, decoding alone falls back to the
  community .NET tooling exporting glTF, imported by `dsr-cache`; everything else is unchanged.
- Retarget quality may be poor on some bones (shoulders, spine twist). The side-by-side shows it;
  fixes go into the bone map or per-bone alignment, not into special cases in the player.
- DS root motion distances are metres; WoW is yards. The height-ratio scale absorbs both units and
  proportions, verified by criterion 2.
