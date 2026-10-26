# Player gait and move clips as editable text — 2026-10-16

**Historical entry.** The later user instruction supersedes the ignored-file policy below.
All imported kits now play from regular project text files, with no DSR install/binary animation cache
required. See [current guide](../../anim/README.md) and
[completed migration](2026-10-16-project-animations.md). Malformed text now reports an error.

First try (path B, an authored run of key poses + stance IK in `ownanim.rs`) was rejected by the user ("Absolut
den mest horribla animation jag sett") and removed. New instruction: put the exact DSR animation into our code
first, then modify it from there.

## What exists
- `crates/wscl/src/text.rs`: `.wsanim`, a `wscl::Clip` as editable text. Header (name, fps, parents, driven bones,
  flag/hit windows), then per frame `frame <i> root x y z yaw` and per bone `b <bone> pitch yaw roll x y z`
  (degrees, `Ry * Rx * Rz` = glam `EulerRot::YXZ`; yards, bone-local). `#` lines are comments. Round trip tested.
- `wow-souls-dsr bake --clips a00_0500,... [--from cache/dsr] --out anim/source`: the retargeted WoW-body clip
  from the cache written as text.
- `crates/wow-souls/src/ownanim.rs`: `load(name)` reads `anim/source/<name>.wsanim` (or `WOWSOULS_ANIM_DIR`).
- `dsloco::load`: for every gait clip named in `locomotion.txt`, an editable file of the same name plays in place
  of the cache's binary clip (a bad file: warn, the cache's plays). Log: `dsloco: N of the gait clips from their
  editable text (anim/source)`.
- Baked: all 66 gait clips in `locomotion.txt` (every weapon set's walk/run/sprint/lock-on), 15 MB.

## Not shippable yet
`anim/source/` is gitignored: the text is still FromSoftware's clip, and so is a hand-edited copy of it
(`docs/SOURCE-TO-ORIGINAL.md`). It removes the runtime need for the DSR *install* for these clips only when the
files are present; `locomotion.txt` (the set → clip table, from the cache) is still read. Status:
`SOURCE_INTEGRATING`.

## Verified
- `ownanim` test: the baked a00_0535 equals the cache clip (quaternion dot > 1-1e-5, translation < 1e-4); the
  same test fails on a copy with one bone turned 20°. Run it with
  `WOWSOULS_DSR_CACHE=$PWD/cache/dsr WOWSOULS_ANIM_DIR=$PWD/anim/source` (tests run in the crate dir; without
  them the test returns early).
- Live: `cache/motion-remix/run-capture.ps1 -Tag text-run` vs `-Tag cache-run -Cache` (empty anim dir): PSNR
  48.3 dB average, min 46, some frames identical (h264 noise only); side strips pose for pose the same
  (`cache/motion-remix/run-compare.png`).
- fmt, clippy clean; wow-souls/wscl/dsr-cache/wow-world tests 1,297 pass.

## Editing a clip
Open e.g. `anim/source/a00_0535.wsanim` (sprint, 19 frames at 30 fps). Bones the clip drives are on the `driven`
line (0 hips root, 18 spine, …); change a `b` line's degrees, restart the game. Bone names: `PROBE_BONES` in the
extractor's probe, or the HumanMale M2 (`benilla-extract <Data> m2bones`).

## Move clips (attacks, roll, backstep, idle, guard, casts) — same day
- `dsclip::player_clip(dir, name)`: the editable `anim/source/<name>.wsanim` in place of the cache's
  `<dir>/human_male/<name>.wscl`, checked (≥2 frames, real fps) and dressed (`remix::dress_player`) exactly as a
  cache clip is; a bad file warns once and the cache's plays. Used for every WoW-body clip `dsclip::load` /
  `load_moves` reads (roll, backstep, the six moves, idle, guardclips: main cache and all 19 weapon families) and
  for the casts (`spells/cast.rs`). The `c0000` (DS-body) clips stay from the cache. One line at `PostStartup`:
  `dsclip: N of the player's move clips from their editable text (anim/source)`.
- Same-named clips are byte-identical across the weapon families (md5 checked), so one file per name serves all.
- Baked: 100 move clips (list taken from `manifest.txt` + `casts.txt` with `--from cache/dsr`, and each family's
  `manifest.txt` with `--from cache/dsr/<family dir>`; lines `roll|backstep|move|idle|guardclip`); `anim/source` is
  now 166 files, 57 MB, still gitignored. Not wired yet: hit reactions (`reaction.rs`), `defense.rs`, interaction
  clips (`loot/interaction.rs`) still read the cache directly.
- Test `ownanim::…an_installed_baked_attack_plays_as_the_cache_clip_with_its_windows`: a23_3000 (straight sword
  r1_1) baked = cache turn for turn, root motion, rate, and all 16 flag windows + the hit window (`hit 0 0.533333
  0.666667`); `player_clip` returns it; an in-memory copy with bone 18 turned 20° is caught. With
  `WOWSOULS_ANIM_DIR` at a copy whose frame-10 bone-18 pitch is +20°, the test fails (`frame 10 bone 18: …`).
- Live: `cache/motion-remix/attack-capture.ps1 -Tag attack-text` vs `-Tag attack-cache -Cache` (one light attack at
  1.5 s, side observer): PSNR 48.6 dB average, min 47.3 over 150 frames (h264 noise; the same video against itself
  shifted 1 s is 17.9 dB); strips pose for pose the same (`cache/motion-remix/attack-compare-small.png`). Only the
  text run logs the 100-clip line.
- The synthetic-cache tests in `dsclip` name their made-up clips after real ones (a00_0710, a23_3000, …), so with
  `WOWSOULS_ANIM_DIR` set the baked file would replace them: `cache_dir_named` calls
  `ownanim::off_in_this_test()` (a test-only per-thread switch; no runtime effect).
- fmt (`-p` list), clippy -j 4 `--all-targets` clean; wow-souls/wscl/dsr-cache/wow-world tests 1,298 pass with and
  without the two env vars.

## Next
- Hook the reaction/defense/interaction reads to `player_clip` too, if those should be editable.
- Ship path stays open: a clip changed enough to be ours, or a new one, before `SOURCE_REMOVED`.
