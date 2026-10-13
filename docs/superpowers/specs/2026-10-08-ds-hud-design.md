# F — the Dark Souls HUD (design)

Date: 2026-10-08. Sub-project F of the A-H decomposition (A-C player data, D impact / sound / reactions, E the
kobold are built). The user's wish: "the Dark Souls HUD", and "run with it, no questions" (they were away), so the
design below was decided by Claude from the survey and is open to the user's eyes afterwards. Nothing here is
committed (the user commits).

## Goal

The HUD is Dark Souls Remastered's own: its art and its layout, read from the user's install the way the
animations were, drawn in the game from `cache/dsr/hud/`. Health and stamina bars with their plates, ruler and
skull, the souls counter, the enemy bar with name and damage number, the lock-on reticle and the YOU DIED banner.
With no cache the old plain HUD stays (one warning), as with every earlier sub-project.

## What the install says (surveyed 2026-10-08)

- **Layout is data.** `menu/menu.drb.dcx` (DCX/zlib, 998 400 bytes) is the whole UI layout: 638 dialogs, 8 543
  dialog objects, all in a 1280 x 720 space (DSR scales it by 1.5 at 1080p). Sections in order: `STR` (UTF-16
  strings, 4 183), `TEXI` (53 texture refs, 16 bytes), `SHPR` (shape property blob), `CTPR`, `ANIP`, `INTP`,
  `SCDP`, `SHAP` (9 181 x 8: shape type string offset + `SHPR` offset), `CTRL` (9 181 x 8), `ANIK`, `ANIO`,
  `ANIM`, `SCDL`, `DLGO` (8 543 x 32: name, `SHAP` byte offset, `CTRL` byte offset), `DLG` (638 x 64: name,
  `SHAP` / `CTRL` offsets, object count at +32, `DLGO` byte offset at +36, packed size at +44), `END`. `SHAP`
  and `CTRL` hold one entry per dialog and per dialog object in file order (a dialog, then its objects).
- **Shape records** (`SHPR`, little endian) all start with `i16 left, top, right, bottom` (parent space).
  `Sprite` (36 bytes): `+8` first colour, `+16` texture rect `i16 u0 v0 u1 v1` (atlas pixels), `+24` `i16` texture
  index into `TEXI` (-1 = none, set at run time, the item icons), `+27` blend byte (1 alpha, 2 additive), `+32`
  tint as the bytes `A R G B`. `MonoRect` (28): the last four bytes `A R G B`. `GouraudRect` (36): four colours.
  `Text` (46 or 50): colour at `+16` as `A R G B`, size `i32` at `+24` (28 for the HUD's numbers). `Null`, `Dialog`
  (a sub-dialog by the same name), `ScrollText`, `Mask`, `MonoFrame`, `GouraudSprite` exist.
- **The HUD dialogs**: `F20-00` (92 objects: `plate_hp`, `bar_0_hp` yellow chip, `bar_1_hp` red, `bar_2_hp` ruler,
  `HP_endbar`, the same `_sp` set for stamina, `bar_BG_0/1`, `bar_start/1`, sub-dialogs `F20-00_arms_l/r`,
  `F20-00_item`, `F20-00_magic`, `soul_param`, `F20_LockCursor`, `enemy_HP0-7`), `F20-02` (the skull `icon_stone`
  with `ring_BG`, `icon_ring_0/1` and the humanity digits), `soul_param` (parchment plate, the two number
  fields, the line), `enemy_HP` (95 x 35: `BG`, `HP_bar1` the chip, `HP_bar0` the current value, `text_param_name`,
  `text_param_damage`), `F20_LockCursor` (102 x 102, sprite `lock` 75 x 75), `F23_YouDied` (a full-screen dim
  rect alpha 50, a centre band `BG_bar` alpha 200 with two Gouraud edge gradients, `main_text_youdied` additive).
  At 1280 x 720: HP bar 175..907 x 76..89 (plate), stamina 96..109, the skull ring at 96..173 x 54..131, souls
  plate 1027..1184 x 626..666, number field 1041..1169 x 630..658 (right aligned), the equipment cross at
  93..372 x 413..641 (`arms_l` 93,468, `arms_r` 282,468, `item` 187,527, `magic` 99,413; 90 x 100 each).
- **Art** is in `menu/menu_0..5.tpf.dcx` (TPF, platform 0, flag 3, Shift-JIS names, DDS DXT1/3/5) and the
  language file `menu/ENGLISH/menu_local.tpf.dcx` (the YOU DIED strip is `Menu14` at v 992-1192, additive, red on
  opaque black). The bars come from `Menu08` (red 6,488-138,512; green 6,550-138,574; yellow chip 6,584-138,608;
  ruler 194,976-1660,1016; end cap 1658,976-1676,1016; plate and ornaments beside), the reticle is `Menu08`
  592,130-742,280, the souls plate `Menu08` 286,488-984,752, the digits of the humanity ring `Menu12`. The stripes
  are dark (red 66,24,24): the plain art is the look, the user judges brightness.
- **The font** is `font/english/DSFont24.ccm.dcx` + `.tpf.dcx`: a CCM (`version 0x10001`, file size, font size 58,
  texture 1024 x 1024, 135 code point groups of `i32 first, last, glyph index`, 1 284 glyphs of 24 bytes: four
  `f32` u0 v0 u1 v1 as normalised atlas coordinates, `i16` pre-space, width, advance, page) and 16 DXT5 pages of
  white glyphs on transparent (page 0 holds Latin, Greek, Cyrillic and symbols).
- Every texture the HUD needs is DXT1/3/5. `texpresso` (decoder, already in `Cargo.lock` through benilla) and `png`
  (already in `Cargo.lock`) are enough: no new download.

## Approaches considered

1. **Own-drawn HUD in the Souls style** (flat colours, measured by eye): what exists. Cheap, never looks right.
2. **DS art, layout measured by hand**: the art is right, every number is a guess.
3. **DS art and DS layout, both read from the install** (chosen): the numbers are DS's own, the extractor gets a
   `.drb` reader (about 250 lines), the game keeps no DS coordinates. The reader also opens the rest of the
   UI (menus, bonfire banner, item pop-ups) for later sub-projects.

Drawing: Bevy UI nodes with images (`ImageNode`), not a custom render pass: a handful of quads. Scale: the
layout is 1280 x 720; `UiScale = window height / 720` makes every node a layout pixel, left-side items anchor to
the left edge, right-side to the right, so a wider window grows the margins, not the HUD.

## Design

### 1. Extractor (`wow-souls-dsr hud`, GPL, in `dsr-cache` / `dsr-formats`)

- `dsr-formats/tpf.rs`: `read(&[u8]) -> Vec<Texture {name, format, dds}>` (TPF, PC platform, UTF-16 / Shift-JIS /
  ASCII names, DCX-wrapped entries).
- `dsr-formats/drb.rs`: `read(&[u8]) -> Drb {textures, dialogs}`; `Dialog {name, rect, objects}`; `Object {name,
  shape}`; `Shape::{Null, Dialog, Sprite{uv, texture, blend, tint}, MonoRect{color}, GouraudRect{colors},
  Text{color, size}, Other(type)}`.
- `dsr-formats/ccm.rs`: `read(&[u8]) -> Font {size, texture, groups, glyphs}`, `Font::glyph(cp)`.
- `dsr-cache/dds.rs`: DXT1/3/5 mip 0 to RGBA8 through `texpresso`; `png` writer.
- `dsr-cache/hud.rs`: a hand table of the dialogs and objects the HUD uses (`PARTS`), resolved against the DRB
  by name. Per sprite: crop the atlas rectangle, convert an additive sprite on opaque black to straight alpha
  (`alpha = max(r, g, b)`, colour divided by it: alpha blending then matches additive on a dark ground), write
  `cache/dsr/hud/<dialog>__<object>.png`. Fonts: the pages the glyph table uses, as PNG. One manifest
  `cache/dsr/hud/manifest.txt`:
  ```
  wscl-hud 1
  source <path> <size> <mtime>         (menu.drb.dcx, the TPFs, the font)
  screen 1280 720
  dialog <name> <l> <t> <r> <b>        (the dialog's rect in its parent: sub-dialog placement)
  sprite <dialog>/<object> <png> <l> <t> <r> <b> <tint AARRGGBB> <alpha|add>
  rect   <dialog>/<object> <l> <t> <r> <b> <AARRGGBB>
  gradient <dialog>/<object> <l> <t> <r> <b> <c_tl> <c_tr> <c_bl> <c_br>
  font <name> <size> <texture w> <h>
  page <n> <png>
  glyph <codepoint> <page> <u0> <v0> <u1> <v1> <pre> <width> <advance>
  ```
  A separate command so the long animation extraction does not rerun; the game checks the HUD manifest's own
  version and source sizes.
- The souls a kill gives (`NpcParam.getSoul`) rides in the kobold's cache section (a `souls <n>` line; absent =
  0), next to its HP.

### 2. Game (`hud/`, MIT/Apache; reads only the manifest and PNGs)

`hud.rs` becomes `hud/` (`mod.rs` plugin + dev / help lines + F3, `art.rs` manifest and PNG loading,
`font.rs` bitmap text, `bars.rs` player bars, `enemy.rs`, `souls.rs`, `banner.rs`, `lock.rs`, `plain.rs` the
old HUD for the fallback). `HudArt` is a resource, present only when the manifest loads; the plain systems run
when it is absent.

- **Bars**: plate rect, chip layer, fill layer, ruler (cropped to the length, so tick spacing stays), end cap,
  the fixed head (`bar_BG`, `bar_start`, skull). Length = `max x PX_PER_POINT` layout px (3, as before).
  Pure `Chip`: the lost-value layer holds `HOLD` s after a drop, then drains at `DRAIN` per s; a rise snaps. HP
  chip yellow behind the red fill (the DRB order: chip first). Stamina has the same layers with a shorter hold.
- **Font**: `BitmapFont` from the glyph lines; pure `layout(text, size, align, width) -> Vec<Quad>`; a `BitmapText`
  component whose children (one `ImageNode` with a sub-rect per glyph) are rebuilt when the string changes.
  Size 28 layout px = 58 atlas px x 0.483.
- **Souls**: `Souls(u32)` resource; a creature's death credits its `souls`; the counter rolls up to the value
  over 0.8 s and the gain shows above it for 2 s and fades. Dying does not lose them yet (the bloodstain is G).
- **Enemy bar**: the struck target's `enemy_HP` set (BG, chip, current) with its name and the damage run in
  the font, over its head as now; the old numbers (1.5 s run, 4 s linger) stay.
- **Lock reticle**: `F20_LockCursor/lock` at the projected aim point replaces the gizmo sphere (which stays only
  without art).
- **YOU DIED**: dim, centre band with its two gradients, the text sprite; fades in over 1.2 s as now, from the
  death time; replaces `hurt.rs`'s plain text when art exists.
- **Equipment cross** (stretch, after the rest is verified): the four `F20-00_*` frames at their DRB places; the
  sword icon in the right-hand slot if the icon cell can be found (`EquipParamWeapon.iconId`).

### 3. Not in F

FP bar, humanity number, status build-up bars, boss bar, item and bonfire banners, area-name banner, menus,
the HUD options, gamepad prompts, losing souls on death and the bloodstain (G), the `ANIM` open / close
animations (YOU DIED is timed by eye: fade in 1.2 s, as the plain version), other languages.

## Verification

- Unit: TPF / DRB / CCM readers on built bytes (no DS bytes in tests); install tests (skip without `DSR_DIR`)
  that read the real files and assert the facts above (53 textures, 638 dialogs, `F20-00` has `bar_1_hp` with
  uv 6,488-138,512, the font has 'A'); DXT decode of hand-made blocks; manifest parse; `Chip`; bar geometry;
  glyph layout (width, right alignment); anchor mapping.
- Live (unattended `WOWSOULS_SHOT`): the idle HUD at 1280 x 720 and 1920 x 1080; after a hit (enemy bar, damage
  number, chip); locked on; low stamina; dead (YOU DIED); with `WOWSOULS_DSR_CACHE=/nonexistent` the plain HUD.
  Pictures in `docs/evidence/hud-*.png`, set beside the Python composite of the DRB as the reference.
- Gates: fmt, clippy `-D warnings`, tests wow-souls / dsr-cache / dsr-formats / wscl; both installs identical
  before and after.

## Open for the user's eyes

Bar brightness (the art is dark), bar length per point, the position of the skull and the souls plate, the
YOU DIED timing, the reticle size, the digits' size and colour.

## Built (2026-10-08, night)

Built inline in the real repo, as planned (`plans/2026-10-08-ds-hud.md`); nothing committed. What differs from
the design above, found while building:

- **Manifest**: `dialog` lines give a dialog's own rectangle and `place` lines the rectangle a sub-dialog or
  anchor object takes in its parent (the lock cursor's 102-pixel dialog is fitted into a 46-pixel place, so the
  ring is about 34 layout px; `soul_param` is shifted by (2, 6)). `text` lines carry colour and size (the DRB's
  `Text` record has its colour at +24 and size at +32; the first draft of this spec said +16 / +24). The
  full-width `r` of `F20-00_arms_r` in the DRB is U+FF52. `source` lines are the cache's existing
  `source <len> <mtime> <path>`, ten of them.
- **CCM**: the four floats are normalised page coordinates (`Font::pixels` rounds them to whole pixels), not
  pixels; glyph `page` is the TPF texture index (`-` lives on page 6).
- **Fonts and colours**: DS's HUD font has one page for Latin; DS blends in gamma space, Bevy in linear, so the
  black overlays of YOU DIED use `ds_alpha(a) = 1 - (1 - a)^2.2`. The art is shown as shipped
  (`WOWSOULS_HUD_GAIN` brightens it): the bars are dark.
- **Souls**: `NpcParam.getSoul` is 150 for the ghoul's row; the kobold's section has a `souls kobold 150` line, a
  creature's death sends `npc::Slain`, `hud::souls::credit` adds them.
- **Enemy bars**: one per struck creature (up to eight, as the layout has eight slots), not only the last.
- **Not built**: the equipment cross (task 13), the layout's open / close animations, the FP bar and the rest of
  "Not in F". The YOU DIED timing is ours (fade in 1.2 s, black from 3.4 s).
- **Two things the user asked for on the way** ("add a few more kobolds, and when they die they lie strangely"):
  `slice::kobold_spots` (a loose group of four across the meadow; `WOWSOULS_KOBOLDS`, F6 `reset_key`,
  `WOWSOULS_RESET`) and the corpse. The corpse was a bone-map error, not the HUD: `KOBOLD_RIG.hips.wow_hip_root`
  was bone 18 (the legs' root) but the spine hangs from bone 13, so a pelvis that dropped to the ground left the
  spine, head and arms standing (bone capsules up to 1.21 yd over the feet at rest, DS joints within 0.18 yd).
  Bone 13 is the pelvis root now; the extractor checks that a death ends less than half as tall as it began
  (`Retargeted.body_height`, it fails with the old map: "a00_2202: the body ends 1.03 yd tall from 1.39"), the
  corpse's capsules end at most 0.55 yd over the feet, `EXTRACTOR_VERSION` is 5.
