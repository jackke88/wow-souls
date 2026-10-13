# DSR Data Reader and Roll Retarget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read Dark Souls Remastered's player animations and animation events from the user's install, retarget the medium forward roll onto the WoW human, and play it in the game in place of the procedural roll, with a side-by-side instrument against the original.

**Architecture:** Three new crates. `wscl` (MIT/Apache) defines the clip cache file shared by extractor and game. `dsr-formats` (GPL-3.0-or-later, ported from SoulsFormatsNEXT / SoulsAssetPipeline / HavokLib) parses DCX, BND3, TAE and Havok 2015 tagfiles with spline-compressed animations. `dsr-cache` (GPL-3.0-or-later) is the offline tool `wow-souls-dsr` that reads the DSR install and the WoW HumanMale skeleton, retargets, and writes `cache/dsr/`. The game (MIT/Apache) reads only `.wscl` files and a text manifest.

**Tech Stack:** Rust 2021, Bevy 0.18.1 (game only), glam 0.31 (extractor), flate2 1.1, anyhow, benilla-formats / benilla-m2 (WoW M2 bones, extractor only).

**Spec:** `docs/superpowers/specs/2026-10-07-dsr-data-and-roll-retarget-design.md`.

**Repository rules (override the skill's defaults):** the user commits; never `git commit` unless asked. Each task ends with a gates checkpoint instead. Always `-j 4` (RAM-tight machine). Never write inside `C:\Spel\Dark Souls Remastered` or `C:\Spel\World of Warcraft 1.12`.

**Deviations from the spec, decided here:**
- `param.rs` (PARAM/PARAMDEF) moves to the movesets plan (C): no success criterion of this plan needs params.
- The bone map is a Rust `const` table (`bonemap.rs`), not RON: same hand-made content, no new dependency.
- The clip cache uses the `.wscl` binary format of Task 1 plus a text manifest, instead of RON metadata.

**Facts used below (measured on the install, 2026-10-07):**
- DCX: big-endian header; `DFLT` at 0x28; uncompressed size (BE u32) at 0x1C; zlib stream at 0x4C.
- Player animation archives: `c0000_a00_lo` (226 files: `a00_0000`-`a00_2213`, incl. rolls `a00_0700/0701/0710/0720`), `c0000_a00_md` (309: `a00_6000`-`a00_8401`), `c0000_a00_hi` (24: `a00_9xxx`), `c0000_a0x`-`a9x` (weapon sets, e.g. `a23_3000` straight sword R1; `a20` is the dagger set, corrected 2026-10-07 from `EquipParamWeapon.wepmotionCategory`). The plan loads all `c0000_a*.anibnd.dcx`.
- BND3 (`chr/c0000.anibnd.dcx`): version `07D7R6`; raw format byte 0x74 → bit-reversed 0x2E = IDs | Names1 | Names2 | Compression; 24-byte file headers (flags, 3 pad, compressed size, offset, id, name offset, uncompressed size); 162 entries.
- TAE: `TAE `, little-endian, 32-bit offsets (byte 7 = 0), version 0x1000B; anim count at 0x54, anim table offset at 0x58; anim table entries (id u32, record offset u32); record: event count, event headers offset, …; event header: start-time offset, end-time offset, event-data offset; event data: type i32, params offset u32.
- TAE event type 0 = `ChrActionFlag`, first param i32 = flag; flag 8 = Invincible; 4, 11, 26 = anim cancels (DSAnimStudio DS1 template).
- HKX: Havok 2015.01 tagfile (`TAG0`, `SDKV` = `20150100`); every player animation is `hkaSplineCompressedAnimation`; root motion in `hkaDefaultAnimatedReferenceFrame.referenceFrameSamples` as (x, y, z, yaw).
- Player skeleton bones include `Pelvis`, `Spine`, `Spine1`, `Neck`, `Head`, `L_/R_Clavicle`, `L_/R_UpperArm`, `L_/R_Forearm`, `L_/R_Hand`, `L_/R_Finger1`, `L_/R_Thigh`, `L_/R_Calf`, `L_/R_Foot`, `L_/R_Toe0`.
- WoW HumanMale bones (index: role; pivots in WoW axes x fwd, y left, z up): 0 top root; 18 Root (key 26, z 1.22); 20 SpineLow; 21 Waist (z 1.16, legs' parent); 22 chest; 48 neck; 64 Head; 49/50 Shoulder L/R; 65/66 Arm L/R; 78/79 elbow L/R; 95/97 wrist L/R; 105/104 hand child L/R; 25/26 hip L/R; 53/54 knee L/R; 68/70 ankle L/R; 80/81 toe L/R. Bones are parent-sorted.

---

## File map

| File | Responsibility |
|---|---|
| `crates/wscl/src/lib.rs` | `.wscl` clip format: `Clip`, `Trs`, `FlagWindow`, read/write |
| `crates/dsr-formats/src/bytes.rs` | bounds-checked little/big-endian reads |
| `crates/dsr-formats/src/dcx.rs` | DCX `DFLT` → bytes |
| `crates/dsr-formats/src/bnd3.rs` | BND3 → entries |
| `crates/dsr-formats/src/tae.rs` | DS1 TAE → animations and events |
| `crates/dsr-formats/src/hkx/tagfile.rs` | Havok 2015 tagfile: sections, types, items, `Obj` field access |
| `crates/dsr-formats/src/hkx/mod.rs` | the `hkaAnimationContainer` lookup |
| `crates/dsr-formats/src/hkx/skeleton.rs` | `hkaSkeleton` → `Skeleton` |
| `crates/dsr-formats/src/hkx/animation.rs` | `hkaSplineCompressedAnimation` + binding + reference frame → `AnimFile` |
| `crates/dsr-formats/src/hkx/spline.rs` | spline block decoding and B-spline sampling |
| `crates/dsr-cache/src/main.rs` | CLI `wow-souls-dsr list|extract` |
| `crates/dsr-cache/src/dsr.rs` | open the player archives, decode a DS clip (full skeleton per frame) |
| `crates/dsr-cache/src/wow.rs` | WoW HumanMale skeleton from the WoW install |
| `crates/dsr-cache/src/basis.rs` | DS model space → Bevy model space at WoW scale |
| `crates/dsr-cache/src/bonemap.rs` | DS c0000 ↔ WoW HumanMale bone table |
| `crates/dsr-cache/src/retarget.rs` | bind-aligned delta transfer → WoW clip + DS model-space clip + error |
| `crates/dsr-cache/src/roll.rs` | roll candidates and selection |
| `crates/dsr-cache/src/cache.rs` | write `.wscl` files and `manifest.txt` |
| `crates/wow-souls/src/dsclip.rs` | load the cache; sample clips; drive the rig in `PosePost`; the DS roll's `EvadeSpec` |
| `crates/wow-souls/src/dsview.rs` | `WOWSOULS_DSVIEW` side-by-side instrument |
| `crates/wow-souls/src/combat.rs` | the roll uses the DS clip when present |

---

### Task 1: `wscl` clip file crate

**Files:**
- Create: `crates/wscl/Cargo.toml`
- Create: `crates/wscl/src/lib.rs`

- [ ] **Step 1: Create the crate manifest**

```toml
[package]
name = "wscl"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Sampled skeletal clip files: the contract between the Dark Souls extractor and the game"

[lints]
workspace = true
```

- [ ] **Step 2: Write the library with its tests (tests first in the file's `mod tests`)**

```rust
//! `.wscl`: a sampled skeletal clip. The Dark Souls extractor (`dsr-cache`, GPL-3.0) writes
//! them and the game reads them; this crate is the whole contract between the two, so the game
//! never links the extractor. Little-endian throughout.
//!
//! Layout: magic `WSCL`, version u32, name (u32 length + UTF-8), fps f32, model-space flag u8,
//! bone count u32, frame count u32, parents (i16 per bone), driven (u8 per bone), frames
//! (per frame per bone: translation 3 f32, rotation x y z w 4 f32, scale 3 f32), root motion
//! (per frame 4 f32), flag-window count u32 and windows (flag i32, start f32, end f32).

use std::io::{self, Read, Write};

pub const MAGIC: [u8; 4] = *b"WSCL";
pub const VERSION: u32 = 1;
/// The `ChrActionFlag` value for invulnerability.
pub const FLAG_INVINCIBLE: i32 = 8;

/// A bone's transform: translation, rotation `(x, y, z, w)`, scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trs {
    pub t: [f32; 3],
    pub r: [f32; 4],
    pub s: [f32; 3],
}

impl Trs {
    pub const IDENTITY: Trs = Trs {
        t: [0.0; 3],
        r: [0.0, 0.0, 0.0, 1.0],
        s: [1.0; 3],
    };
}

/// A `ChrActionFlag` window from the animation's TAE.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlagWindow {
    pub flag: i32,
    pub start: f32,
    pub end: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Clip {
    pub name: String,
    pub fps: f32,
    /// Transforms are bone-local (`false`) or in model space (`true`).
    pub model_space: bool,
    pub parents: Vec<i16>,
    /// Bones a player writes; the others keep whatever the rig had.
    pub driven: Vec<bool>,
    /// `frames[frame][bone]`.
    pub frames: Vec<Vec<Trs>>,
    /// Per frame: accumulated root motion `(x, y, z)` and yaw (rad), Bevy axes, yards.
    pub root_motion: Vec<[f32; 4]>,
    pub flags: Vec<FlagWindow>,
}

fn bad(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

fn put_u32(w: &mut impl Write, v: u32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn put_f32(w: &mut impl Write, v: f32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn get<const N: usize>(r: &mut impl Read) -> io::Result<[u8; N]> {
    let mut b = [0u8; N];
    r.read_exact(&mut b)?;
    Ok(b)
}

fn get_u32(r: &mut impl Read) -> io::Result<u32> {
    Ok(u32::from_le_bytes(get(r)?))
}

fn get_f32(r: &mut impl Read) -> io::Result<f32> {
    Ok(f32::from_le_bytes(get(r)?))
}

fn get_f32s<const N: usize>(r: &mut impl Read) -> io::Result<[f32; N]> {
    let mut out = [0.0; N];
    for v in &mut out {
        *v = get_f32(r)?;
    }
    Ok(out)
}

impl Clip {
    /// Seconds from the first frame to the last.
    pub fn duration(&self) -> f32 {
        self.frames.len().saturating_sub(1) as f32 / self.fps
    }

    pub fn bones(&self) -> usize {
        self.parents.len()
    }

    /// The windows of one flag, `(start, end)` in seconds.
    pub fn windows(&self, flag: i32) -> impl Iterator<Item = (f32, f32)> + '_ {
        self.flags
            .iter()
            .filter(move |w| w.flag == flag)
            .map(|w| (w.start, w.end))
    }

    pub fn write(&self, w: &mut impl Write) -> io::Result<()> {
        let bones = self.parents.len();
        if self.driven.len() != bones {
            return Err(bad("driven mask length differs from the bone count"));
        }
        if self.frames.iter().any(|f| f.len() != bones) {
            return Err(bad("a frame's bone count differs from the skeleton's"));
        }
        if self.root_motion.len() != self.frames.len() {
            return Err(bad("root motion length differs from the frame count"));
        }
        w.write_all(&MAGIC)?;
        put_u32(w, VERSION)?;
        put_u32(w, self.name.len() as u32)?;
        w.write_all(self.name.as_bytes())?;
        put_f32(w, self.fps)?;
        w.write_all(&[u8::from(self.model_space)])?;
        put_u32(w, bones as u32)?;
        put_u32(w, self.frames.len() as u32)?;
        for p in &self.parents {
            w.write_all(&p.to_le_bytes())?;
        }
        for d in &self.driven {
            w.write_all(&[u8::from(*d)])?;
        }
        for frame in &self.frames {
            for trs in frame {
                for v in trs.t.iter().chain(&trs.r).chain(&trs.s) {
                    put_f32(w, *v)?;
                }
            }
        }
        for rm in &self.root_motion {
            for v in rm {
                put_f32(w, *v)?;
            }
        }
        put_u32(w, self.flags.len() as u32)?;
        for f in &self.flags {
            w.write_all(&f.flag.to_le_bytes())?;
            put_f32(w, f.start)?;
            put_f32(w, f.end)?;
        }
        Ok(())
    }

    pub fn read(r: &mut impl Read) -> io::Result<Clip> {
        if get::<4>(r)? != MAGIC {
            return Err(bad("not a .wscl file"));
        }
        let version = get_u32(r)?;
        if version != VERSION {
            return Err(bad(format!(".wscl version {version}, expected {VERSION}")));
        }
        let name_len = get_u32(r)? as usize;
        let mut name = vec![0u8; name_len];
        r.read_exact(&mut name)?;
        let name = String::from_utf8(name).map_err(|_| bad("clip name is not UTF-8"))?;
        let fps = get_f32(r)?;
        let model_space = get::<1>(r)?[0] != 0;
        let bones = get_u32(r)? as usize;
        let frame_count = get_u32(r)? as usize;
        let mut parents = Vec::with_capacity(bones);
        for _ in 0..bones {
            parents.push(i16::from_le_bytes(get(r)?));
        }
        let mut driven = Vec::with_capacity(bones);
        for _ in 0..bones {
            driven.push(get::<1>(r)?[0] != 0);
        }
        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let mut frame = Vec::with_capacity(bones);
            for _ in 0..bones {
                frame.push(Trs {
                    t: get_f32s(r)?,
                    r: get_f32s(r)?,
                    s: get_f32s(r)?,
                });
            }
            frames.push(frame);
        }
        let mut root_motion = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            root_motion.push(get_f32s(r)?);
        }
        let n = get_u32(r)? as usize;
        let mut flags = Vec::with_capacity(n);
        for _ in 0..n {
            flags.push(FlagWindow {
                flag: i32::from_le_bytes(get(r)?),
                start: get_f32(r)?,
                end: get_f32(r)?,
            });
        }
        Ok(Clip {
            name,
            fps,
            model_space,
            parents,
            driven,
            frames,
            root_motion,
            flags,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Clip {
        let a = Trs {
            t: [1.0, 2.0, 3.0],
            r: [0.0, 0.7071, 0.0, 0.7071],
            s: [1.0; 3],
        };
        Clip {
            name: "a00_6000".into(),
            fps: 30.0,
            model_space: true,
            parents: vec![-1, 0],
            driven: vec![true, false],
            frames: vec![vec![Trs::IDENTITY, a], vec![a, Trs::IDENTITY]],
            root_motion: vec![[0.0; 4], [0.0, 0.0, -1.5, 0.1]],
            flags: vec![FlagWindow {
                flag: FLAG_INVINCIBLE,
                start: 0.05,
                end: 0.4,
            }],
        }
    }

    #[test]
    fn a_clip_survives_a_round_trip() {
        let clip = sample();
        let mut bytes = Vec::new();
        clip.write(&mut bytes).unwrap();
        let back = Clip::read(&mut bytes.as_slice()).unwrap();
        assert_eq!(back, clip);
        assert!((back.duration() - 1.0 / 30.0).abs() < 1.0e-6);
        assert_eq!(back.windows(FLAG_INVINCIBLE).collect::<Vec<_>>(), [(0.05, 0.4)]);
    }

    #[test]
    fn a_wrong_magic_is_refused() {
        let err = Clip::read(&mut &b"NOPE\x01\x00\x00\x00"[..]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_ragged_frame_is_refused_on_write() {
        let mut clip = sample();
        clip.frames[1].pop();
        assert!(clip.write(&mut Vec::new()).is_err());
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wscl`
Expected: `test result: ok. 3 passed`

- [ ] **Step 4: Gates checkpoint**

Run: `cargo fmt --all -- --check && cargo clippy -j 4 -p wscl --all-targets -- -D warnings`
Expected: no output from fmt, clippy `Finished` with no warnings.

---

### Task 2: `dsr-formats` scaffold, byte reads and DCX

**Files:**
- Create: `crates/dsr-formats/Cargo.toml`
- Create: `crates/dsr-formats/src/lib.rs`
- Create: `crates/dsr-formats/src/bytes.rs`
- Create: `crates/dsr-formats/src/dcx.rs`
- Modify: `Cargo.toml` (workspace dependencies)
- Modify: `docs/PROVENANCE.md`

- [ ] **Step 1: Add workspace dependencies** — in `Cargo.toml` under `[workspace.dependencies]`, after `anyhow = "1.0.102"`, add:

```toml
flate2 = "1.1"
glam = "0.31"
benilla-m2 = { path = "../benilla/crates/benilla-m2" }
wscl = { path = "crates/wscl" }
dsr-formats = { path = "crates/dsr-formats" }
```

- [ ] **Step 2: Crate manifest** — `crates/dsr-formats/Cargo.toml`:

```toml
[package]
name = "dsr-formats"
version.workspace = true
edition.workspace = true
# Ported from GPL-3.0 sources (SoulsFormatsNEXT, SoulsAssetPipeline, HavokLib): see
# docs/PROVENANCE.md. The game crate never links this crate.
license = "GPL-3.0-or-later"
description = "Readers for Dark Souls Remastered data: DCX, BND3, TAE, Havok 2015 tagfiles"

[lints]
workspace = true

[dependencies]
anyhow.workspace = true
flate2.workspace = true
```

- [ ] **Step 3: `src/lib.rs`**

```rust
//! Readers for Dark Souls Remastered's data formats: DCX compression, BND3 archives, DS1 TAE
//! animation events and Havok 2015 tagfiles (skeletons, spline-compressed animations).
//!
//! GPL-3.0-or-later. Ported from SoulsFormatsNEXT (DCX, BND3, TAE), SoulsAssetPipeline's Havoc
//! tagfile reader and its spline decompressor (Meowmaritus, Katalash), itself after HavokLib
//! (Lukas Cone). Layouts were checked against the user's own install; see docs/PROVENANCE.md.

pub mod bnd3;
mod bytes;
pub mod dcx;
pub mod hkx;
pub mod tae;
```

(`bnd3`, `hkx` and `tae` are created in Tasks 3-8; until then create empty files `src/bnd3.rs`, `src/tae.rs` and `src/hkx/mod.rs` so the crate compiles.)

- [ ] **Step 4: `src/bytes.rs`**

```rust
//! Bounds-checked reads; every out-of-range read names its offset.

use anyhow::{Context, Result};

pub(crate) fn slice(b: &[u8], at: usize, n: usize) -> Result<&[u8]> {
    let end = at.checked_add(n).context("offset overflow")?;
    b.get(at..end)
        .with_context(|| format!("{n} bytes at {at:#x} run past the end ({:#x})", b.len()))
}

fn arr<const N: usize>(b: &[u8], at: usize) -> Result<[u8; N]> {
    Ok(slice(b, at, N)?.try_into().expect("slice length is N"))
}

pub(crate) fn u8_at(b: &[u8], at: usize) -> Result<u8> {
    Ok(slice(b, at, 1)?[0])
}

pub(crate) fn u16_at(b: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(arr(b, at)?))
}

pub(crate) fn i16_at(b: &[u8], at: usize) -> Result<i16> {
    Ok(i16::from_le_bytes(arr(b, at)?))
}

pub(crate) fn u32_at(b: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(arr(b, at)?))
}

pub(crate) fn i32_at(b: &[u8], at: usize) -> Result<i32> {
    Ok(i32::from_le_bytes(arr(b, at)?))
}

pub(crate) fn u64_at(b: &[u8], at: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(arr(b, at)?))
}

pub(crate) fn f32_at(b: &[u8], at: usize) -> Result<f32> {
    Ok(f32::from_le_bytes(arr(b, at)?))
}

pub(crate) fn u32_be_at(b: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(arr(b, at)?))
}

/// A NUL-terminated string at `at` (Shift-JIS in the archives; DSR's paths are ASCII).
pub(crate) fn cstr_at(b: &[u8], at: usize) -> Result<String> {
    let tail = b.get(at..).with_context(|| format!("string at {at:#x} past the end"))?;
    let len = tail
        .iter()
        .position(|&c| c == 0)
        .with_context(|| format!("unterminated string at {at:#x}"))?;
    Ok(String::from_utf8_lossy(&tail[..len]).into_owned())
}
```

- [ ] **Step 5: Write the failing DCX test** — `src/dcx.rs`:

```rust
//! DCX `DFLT`: a big-endian header around one zlib stream (SoulsFormatsNEXT `DCX.cs`,
//! `DecompressDCXDFLT`).

use std::io::Read;

use anyhow::{bail, Result};

use crate::bytes::{slice, u32_be_at};

/// Where the zlib stream starts: after `DCX`, `DCS`, `DCP`/`DFLT` and `DCA` blocks.
const ZLIB_AT: usize = 0x4C;

/// The decompressed payload of a `DCX`/`DFLT` file.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let _ = data;
    bail!("not implemented")
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    /// A DCX around `payload`, laid out as DSR's files are.
    pub(crate) fn dcx_of(payload: &[u8]) -> Vec<u8> {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(payload).unwrap();
        let zlib = z.finish().unwrap();
        let mut out = vec![0u8; ZLIB_AT];
        out[0..4].copy_from_slice(b"DCX\0");
        out[0x18..0x1C].copy_from_slice(b"DCS\0");
        out[0x1C..0x20].copy_from_slice(&(payload.len() as u32).to_be_bytes());
        out[0x20..0x24].copy_from_slice(&(zlib.len() as u32).to_be_bytes());
        out[0x24..0x28].copy_from_slice(b"DCP\0");
        out[0x28..0x2C].copy_from_slice(b"DFLT");
        out[0x44..0x48].copy_from_slice(b"DCA\0");
        out.extend_from_slice(&zlib);
        out
    }

    #[test]
    fn a_dflt_dcx_inflates_to_its_payload() {
        let payload = b"BND307D7R6 and the rest of an archive".repeat(20);
        assert_eq!(decompress(&dcx_of(&payload)).unwrap(), payload);
    }

    #[test]
    fn other_files_are_refused() {
        assert!(decompress(b"BND3....").is_err());
        let mut krak = dcx_of(b"x");
        krak[0x28..0x2C].copy_from_slice(b"KRAK");
        assert!(decompress(&krak).is_err());
    }

    #[test]
    fn a_size_mismatch_is_an_error() {
        let mut d = dcx_of(b"hello");
        d[0x1C..0x20].copy_from_slice(&9u32.to_be_bytes());
        assert!(decompress(&d).is_err());
    }
}
```

- [ ] **Step 6: Run to verify it fails**

Run: `cargo test -j 4 -p dsr-formats dcx`
Expected: `a_dflt_dcx_inflates_to_its_payload` FAILS with `not implemented`.

- [ ] **Step 7: Implement `decompress`** — replace its body:

```rust
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < ZLIB_AT || slice(data, 0, 4)? != b"DCX\0" {
        bail!("not a DCX file");
    }
    let method = slice(data, 0x28, 4)?;
    if method != b"DFLT" {
        bail!("DCX compression {} is not DFLT", String::from_utf8_lossy(method));
    }
    let size = u32_be_at(data, 0x1C)? as usize;
    let mut out = Vec::with_capacity(size);
    flate2::read::ZlibDecoder::new(&data[ZLIB_AT..]).read_to_end(&mut out)?;
    if out.len() != size {
        bail!("DCX inflated {} bytes, its header says {size}", out.len());
    }
    Ok(out)
}
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -j 4 -p dsr-formats dcx`
Expected: `3 passed`.

- [ ] **Step 9: Provenance** — append to `docs/PROVENANCE.md`:

```markdown
## Dark Souls Remastered readers (GPL-3.0-or-later crates)

`crates/dsr-formats` and `crates/dsr-cache` are GPL-3.0-or-later. Ported from:
- SoulsFormatsNEXT (soulsmods, GPL-3.0): `Formats/DCX.cs`, `Formats/Binder/*` (BND3),
  `Formats/TAE/*` (DS1 TAE).
- SoulsAssetPipeline (Meowmaritus, GPL-3.0): `Havoc/IO/Tagfile/Binary/*` (Havok 2015 tagfile),
  `SoulsAssetPipeline/Animation/HKX/SplineCompressedAnimation.cs` (spline decompression),
  `HKX.GenFakeFromTagFile.cs` (field names).
- HavokLib (Lukas Cone, GPL-3.0), via the above.
- DSAnimStudio's `TAE.Template.DS1.xml` for event meanings (type 0 `ChrActionFlag`, flag 8
  Invincible).
The game crate (`wow-souls`, MIT OR Apache-2.0) does not link them; it reads `.wscl` files
(`crates/wscl`, MIT OR Apache-2.0, our own format).
```

- [ ] **Step 10: Gates checkpoint**

Run: `cargo fmt --all -- --check && cargo clippy -j 4 -p dsr-formats --all-targets -- -D warnings`
Expected: clean. (Empty `bnd3.rs`/`tae.rs`/`hkx/mod.rs` compile.)

---

### Task 3: BND3

**Files:**
- Modify: `crates/dsr-formats/src/bnd3.rs`

- [ ] **Step 1: Write the failing test with a stub**

```rust
//! BND3 archives (SoulsFormatsNEXT `BND3.cs`, `BinderFileHeader.ReadBinder3FileHeader`).

use anyhow::{bail, Result};

use crate::bytes::{cstr_at, i32_at, slice, u32_at, u64_at, u8_at};
use crate::dcx;

/// One file in an archive.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: i32,
    /// The full internal path, e.g. `N:\FRPG\data\Model\chr\c0000\taeNew\x64\a00.tae`.
    pub name: String,
    pub data: Vec<u8>,
}

impl Entry {
    /// The last path component.
    pub fn file_name(&self) -> &str {
        self.name.rsplit(['\\', '/']).next().unwrap_or(&self.name)
    }

    /// The file name without its extension.
    pub fn stem(&self) -> &str {
        let f = self.file_name();
        f.rsplit_once('.').map_or(f, |(s, _)| s)
    }
}

pub fn read(b: &[u8]) -> Result<Vec<Entry>> {
    let _ = (b, cstr_at, i32_at, slice, u32_at, u64_at, u8_at, dcx::decompress);
    bail!("not implemented")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-file BND3 in DSR's layout (raw format byte 0x74, 24-byte headers).
    fn sample() -> Vec<u8> {
        let files: [(&str, i32, &[u8]); 2] =
            [("N:\\x\\a00.tae", 5_000_000, b"TAE data"), ("N:\\x\\Skeleton.hkx", 4_000_000, b"HKX")];
        let names_at = 0x20 + 24 * files.len();
        let mut names = Vec::new();
        let mut name_offsets = Vec::new();
        for (n, _, _) in &files {
            name_offsets.push(names_at + names.len());
            names.extend_from_slice(n.as_bytes());
            names.push(0);
        }
        let data_at = names_at + names.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"BND307D7R6\0\0");
        out.extend_from_slice(&[0x74, 0, 0, 0]);
        out.extend_from_slice(&(files.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data_at as u32).to_le_bytes());
        out.extend_from_slice(&[0; 8]);
        let mut at = data_at;
        for (i, (_, id, data)) in files.iter().enumerate() {
            out.extend_from_slice(&[0x40, 0, 0, 0]);
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(at as u32).to_le_bytes());
            out.extend_from_slice(&id.to_le_bytes());
            out.extend_from_slice(&(name_offsets[i] as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            at += data.len();
        }
        out.extend_from_slice(&names);
        for (_, _, data) in &files {
            out.extend_from_slice(data);
        }
        out
    }

    #[test]
    fn entries_come_back_with_ids_names_and_data() {
        let e = read(&sample()).unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].id, e[0].file_name(), e[0].stem()), (5_000_000, "a00.tae", "a00"));
        assert_eq!(e[0].data, b"TAE data");
        assert_eq!((e[1].file_name(), e[1].data.as_slice()), ("Skeleton.hkx", &b"HKX"[..]));
    }

    #[test]
    fn a_non_bnd3_is_refused() {
        assert!(read(b"BND4 nope").is_err());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -j 4 -p dsr-formats bnd3`
Expected: FAIL `not implemented`.

- [ ] **Step 3: Implement `read`** — replace the stub:

```rust
pub fn read(b: &[u8]) -> Result<Vec<Entry>> {
    if slice(b, 0, 4)? != b"BND3" {
        bail!("not a BND3 archive");
    }
    let raw = u8_at(b, 0x0C)?;
    let big_endian = u8_at(b, 0x0D)? != 0;
    let bit_big_endian = u8_at(b, 0x0E)? != 0;
    // `Binder.ReadFormat`: the flag byte is stored bit-reversed unless bit-big-endian.
    let reverse = bit_big_endian || (raw & 1 != 0 && raw & 0x80 == 0);
    let format = if reverse { raw } else { raw.reverse_bits() };
    if big_endian || format & 0x01 != 0 {
        bail!("big-endian BND3 is not supported");
    }
    let has_ids = format & 0x02 != 0;
    let has_names = format & 0x0C != 0;
    let long_offsets = format & 0x10 != 0;
    let has_compression = format & 0x20 != 0;
    let count = u32_at(b, 0x10)? as usize;
    let mut at = 0x20;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let raw_flags = u8_at(b, at)?;
        let flags = if bit_big_endian { raw_flags } else { raw_flags.reverse_bits() };
        let size = u32_at(b, at + 4)? as usize;
        at += 8;
        let offset = if long_offsets {
            at += 8;
            u64_at(b, at - 8)? as usize
        } else {
            at += 4;
            u32_at(b, at - 4)? as usize
        };
        let id = if has_ids {
            at += 4;
            i32_at(b, at - 4)?
        } else {
            -1
        };
        let name = if has_names {
            at += 4;
            cstr_at(b, u32_at(b, at - 4)? as usize)?
        } else {
            String::new()
        };
        if has_compression {
            at += 4; // the uncompressed size
        }
        let raw_data = slice(b, offset, size)?;
        // `FileFlags.Compressed` is bit 0: the entry is itself a DCX.
        let data = if flags & 0x01 != 0 {
            dcx::decompress(raw_data)?
        } else {
            raw_data.to_vec()
        };
        out.push(Entry { id, name, data });
    }
    Ok(out)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -j 4 -p dsr-formats bnd3`
Expected: `2 passed`.

- [ ] **Step 5: Gates checkpoint** — `cargo fmt --all -- --check && cargo clippy -j 4 -p dsr-formats --all-targets -- -D warnings` clean.

---

### Task 4: TAE (DS1, 32-bit)

**Files:**
- Modify: `crates/dsr-formats/src/tae.rs`

- [ ] **Step 1: Failing test with stub**

```rust
//! DS1 TAE (`TAE `, version 0x1000B, 32-bit offsets): per animation, its timed events
//! (SoulsFormatsNEXT `TAE.cs`, `Animation.cs`, `Event.cs`, DS1 branches).

use anyhow::{bail, Result};

use crate::bytes::{f32_at, i32_at, slice, u32_at, u8_at};

/// How many parameter bytes an event keeps: enough for every field the extractor reads.
pub const PARAM_BYTES: usize = 16;
/// Event type 0, `ChrActionFlag`; its first parameter is the flag.
pub const CHR_ACTION_FLAG: i32 = 0;

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub kind: i32,
    pub start: f32,
    pub end: f32,
    /// The first [`PARAM_BYTES`] parameter bytes (fewer at the end of the file).
    pub params: Vec<u8>,
}

impl Event {
    /// The `i`-th 32-bit parameter.
    pub fn param_i32(&self, i: usize) -> Option<i32> {
        let b = self.params.get(i * 4..i * 4 + 4)?;
        Some(i32::from_le_bytes(b.try_into().ok()?))
    }

    /// The `ChrActionFlag` value, for a type-0 event.
    pub fn action_flag(&self) -> Option<i32> {
        (self.kind == CHR_ACTION_FLAG).then(|| self.param_i32(0)).flatten()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Anim {
    pub id: u32,
    pub events: Vec<Event>,
}

pub fn read(b: &[u8]) -> Result<Vec<Anim>> {
    let _ = (b, f32_at, i32_at, slice, u32_at, u8_at);
    bail!("not implemented")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One animation (6000) with one event: type 0, flag 8, 0.1-0.4 s.
    fn sample() -> Vec<u8> {
        let mut b = vec![0u8; 0xB0];
        b[0..4].copy_from_slice(b"TAE ");
        b[8..12].copy_from_slice(&0x1000Bu32.to_le_bytes());
        b[0x54..0x58].copy_from_slice(&1u32.to_le_bytes());
        b[0x58..0x5C].copy_from_slice(&0x60u32.to_le_bytes());
        // Anim table: id 6000, record at 0x68.
        b[0x60..0x64].copy_from_slice(&6000u32.to_le_bytes());
        b[0x64..0x68].copy_from_slice(&0x68u32.to_le_bytes());
        // Record: one event, headers at 0x84.
        b[0x68..0x6C].copy_from_slice(&1u32.to_le_bytes());
        b[0x6C..0x70].copy_from_slice(&0x84u32.to_le_bytes());
        // Event header: start at 0x90, end at 0x94, data at 0x98.
        b[0x84..0x88].copy_from_slice(&0x90u32.to_le_bytes());
        b[0x88..0x8C].copy_from_slice(&0x94u32.to_le_bytes());
        b[0x8C..0x90].copy_from_slice(&0x98u32.to_le_bytes());
        b[0x90..0x94].copy_from_slice(&0.1f32.to_le_bytes());
        b[0x94..0x98].copy_from_slice(&0.4f32.to_le_bytes());
        // Event data: type 0, params at 0xA0: flag 8.
        b[0x98..0x9C].copy_from_slice(&0i32.to_le_bytes());
        b[0x9C..0xA0].copy_from_slice(&0xA0u32.to_le_bytes());
        b[0xA0..0xA4].copy_from_slice(&8i32.to_le_bytes());
        b
    }

    #[test]
    fn an_invincibility_window_is_read() {
        let anims = read(&sample()).unwrap();
        assert_eq!(anims.len(), 1);
        assert_eq!(anims[0].id, 6000);
        let e = &anims[0].events[0];
        assert_eq!((e.kind, e.start, e.end), (0, 0.1, 0.4));
        assert_eq!(e.action_flag(), Some(8));
    }

    #[test]
    fn other_tae_variants_are_refused() {
        let mut b = sample();
        b[7] = 0xFF; // 64-bit
        assert!(read(&b).is_err());
        let mut b = sample();
        b[8..12].copy_from_slice(&0x1000Cu32.to_le_bytes());
        assert!(read(&b).is_err());
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -j 4 -p dsr-formats tae` → FAIL `not implemented`.

- [ ] **Step 3: Implement `read`**

```rust
pub fn read(b: &[u8]) -> Result<Vec<Anim>> {
    if slice(b, 0, 4)? != b"TAE " {
        bail!("not a TAE file");
    }
    if u8_at(b, 4)? != 0 {
        bail!("big-endian TAE is not supported");
    }
    if u8_at(b, 7)? == 0xFF {
        bail!("64-bit TAE is not supported (DSR ships 32-bit TAE)");
    }
    let version = u32_at(b, 8)?;
    if version != 0x1000B {
        bail!("TAE version {version:#x} is not DS1's 0x1000B");
    }
    let count = u32_at(b, 0x54)? as usize;
    let table = u32_at(b, 0x58)? as usize;
    let mut anims = Vec::with_capacity(count);
    for i in 0..count {
        let id = u32_at(b, table + i * 8)?;
        let record = u32_at(b, table + i * 8 + 4)? as usize;
        let n = u32_at(b, record)? as usize;
        let headers = u32_at(b, record + 4)? as usize;
        let mut events = Vec::with_capacity(n);
        for e in 0..n {
            let h = headers + e * 12;
            let start = f32_at(b, u32_at(b, h)? as usize)?;
            let end = f32_at(b, u32_at(b, h + 4)? as usize)?;
            let data = u32_at(b, h + 8)? as usize;
            let kind = i32_at(b, data)?;
            let params_at = u32_at(b, data + 4)? as usize;
            let params = if params_at == 0 {
                Vec::new()
            } else {
                b.get(params_at..(params_at + PARAM_BYTES).min(b.len()))
                    .unwrap_or(&[])
                    .to_vec()
            };
            events.push(Event { kind, start, end, params });
        }
        anims.push(Anim { id, events });
    }
    Ok(anims)
}
```

- [ ] **Step 4: Run** — `cargo test -j 4 -p dsr-formats tae` → `2 passed`.
- [ ] **Step 5: Gates checkpoint** — fmt + clippy for `dsr-formats` clean.

---

### Task 5: Havok tagfile reader

**Files:**
- Create: `crates/dsr-formats/src/hkx/tagfile.rs`
- Modify: `crates/dsr-formats/src/hkx/mod.rs`

- [ ] **Step 1: `hkx/mod.rs`** (skeleton/animation/spline modules arrive in Tasks 6-8; create them as empty files now):

```rust
//! Havok 2015 tagfiles: the animation container, skeletons and spline-compressed animations.

use anyhow::{bail, Result};

pub mod animation;
pub mod skeleton;
pub mod spline;
pub mod tagfile;

use tagfile::{Obj, Tagfile};

/// The file's `hkaAnimationContainer`: root → `namedVariants[i].variant`.
pub fn animation_container(tf: &Tagfile) -> Result<Obj<'_>> {
    let root = tf.root()?;
    for v in root.field("namedVariants")?.array()? {
        if let Some(obj) = v.field("variant")?.ptr()? {
            if obj.type_name() == "hkaAnimationContainer" {
                return Ok(obj);
            }
        }
    }
    bail!("no hkaAnimationContainer in the root container")
}
```

- [ ] **Step 2: Write `hkx/tagfile.rs` with tests for the pure pieces**

```rust
//! Havok 2015 binary tagfile (`TAG0`): sections, the type table and the item index, read
//! lazily: an [`Obj`] is a type and an offset, and fields are read by name.
//!
//! Ported from SoulsAssetPipeline's `HkBinaryTagfileReader`, `HkBinaryTypeReader`, `HkSection`,
//! `HkType` and `BinaryReaderEx.ReadPackedInt` (GPL-3.0). Pointers and non-fixed arrays are
//! stored in `DATA` as u32 indices into the `INDX/ITEM` table; item 1 holds the root object.

use anyhow::{bail, Context, Result};

use crate::bytes::{f32_at, slice, u32_at, u32_be_at, u8_at};

/// A section: 4-byte big-endian size (top 2 bits: leaf flags), 4-byte tag, then payload or
/// child sections.
#[derive(Debug, Clone)]
pub struct Section {
    pub tag: [u8; 4],
    pub start: usize,
    pub end: usize,
    pub children: Vec<Section>,
}

impl Section {
    pub fn child(&self, tag: &[u8; 4]) -> Option<&Section> {
        self.children.iter().find(|c| &c.tag == tag)
    }
}

pub fn read_section(b: &[u8], at: usize) -> Result<Section> {
    let word = u32_be_at(b, at)?;
    let size = (word & 0x3FFF_FFFF) as usize;
    let tag: [u8; 4] = slice(b, at + 4, 4)?.try_into()?;
    let (start, end) = (at + 8, at + size);
    if size < 8 || end > b.len() {
        bail!(
            "section {} at {at:#x} (size {size:#x}) runs past the end",
            String::from_utf8_lossy(&tag)
        );
    }
    let mut children = Vec::new();
    if word & 0xC000_0000 == 0 {
        let mut p = start;
        while p < end {
            let c = read_section(b, p)?;
            p = c.end;
            children.push(c);
        }
    }
    Ok(Section { tag, start, end, children })
}

/// Havok's packed integer: 1 to 4 bytes, the length in the first byte's top bits.
pub fn packed(b: &[u8], p: &mut usize) -> Result<u64> {
    let first = u64::from(u8_at(b, *p)?);
    let at = *p;
    *p += 1;
    if first & 0x80 == 0 {
        return Ok(first);
    }
    let more = |n: usize, p: &mut usize| -> Result<u64> {
        let mut v = first;
        for _ in 0..n {
            v = (v << 8) | u64::from(u8_at(b, *p)?);
            *p += 1;
        }
        Ok(v)
    };
    Ok(match first >> 3 {
        0x10..=0x17 => more(1, p)? & 0x3FFF,
        0x18..=0x1B => more(2, p)? & 0x1F_FFFF,
        0x1C => more(3, p)? & 0x7FF_FFFF,
        f => bail!("packed int form {f:#x} at {at:#x} is not supported"),
    })
}

/// A 1-based type reference; 0 is none.
fn index(v: u64) -> Option<usize> {
    (v != 0).then(|| v as usize - 1)
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub offset: usize,
    pub ty: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Type {
    pub name: String,
    pub parent: Option<usize>,
    pub flags: u64,
    format_info: u64,
    sub_type: Option<usize>,
    byte_size: u64,
    pub fields: Vec<Field>,
}

const HAS_FORMAT: u64 = 1 << 0;
const HAS_SUBTYPE: u64 = 1 << 1;
const HAS_VERSION: u64 = 1 << 2;
const HAS_SIZE: u64 = 1 << 3;
const HAS_UNKNOWN: u64 = 1 << 4;
const HAS_FIELDS: u64 = 1 << 5;
const HAS_INTERFACES: u64 = 1 << 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Void,
    Opaque,
    Bool,
    String,
    Int,
    Float,
    Ptr,
    Class,
    Array,
}

#[derive(Debug, Clone, Copy)]
struct Item {
    ty: Option<usize>,
    at: usize,
    count: usize,
}

pub struct Tagfile {
    bytes: Vec<u8>,
    pub types: Vec<Type>,
    items: Vec<Item>,
}

fn strings(b: &[u8], s: &Section) -> Result<Vec<String>> {
    slice(b, s.start, s.end - s.start)?
        .split(|&c| c == 0)
        .map(|w| Ok(String::from_utf8_lossy(w).into_owned()))
        .collect()
}

fn name_at(list: &[String], i: u64) -> Result<String> {
    list.get(i as usize)
        .cloned()
        .with_context(|| format!("string index {i} past the string table ({})", list.len()))
}

fn read_types(b: &[u8], sec: &Section) -> Result<Vec<Type>> {
    let (mut tstr, mut fstr, mut types) = (Vec::new(), Vec::new(), Vec::<Type>::new());
    for s in &sec.children {
        let mut p = s.start;
        match &s.tag {
            b"TSTR" | b"TST1" => tstr = strings(b, s)?,
            b"FSTR" | b"FST1" => fstr = strings(b, s)?,
            b"TNAM" | b"TNA1" => {
                let count = packed(b, &mut p)? as usize;
                types = vec![Type::default(); count];
                for t in types.iter_mut() {
                    t.name = name_at(&tstr, packed(b, &mut p)?)?;
                    let params = packed(b, &mut p)? & 0x3F;
                    for _ in 0..params {
                        packed(b, &mut p)?; // parameter name
                        packed(b, &mut p)?; // parameter value (a type index or an int)
                    }
                }
            }
            b"TBOD" | b"TBDY" => {
                while p < s.end {
                    let Some(ti) = index(packed(b, &mut p)?) else {
                        continue;
                    };
                    let parent = index(packed(b, &mut p)?);
                    let flags = packed(b, &mut p)?;
                    let mut t = types
                        .get(ti)
                        .cloned()
                        .with_context(|| format!("TBOD names type {ti} past TNAM"))?;
                    t.parent = parent;
                    t.flags = flags;
                    if flags & HAS_FORMAT != 0 {
                        t.format_info = packed(b, &mut p)?;
                    }
                    if flags & HAS_SUBTYPE != 0 {
                        t.sub_type = index(packed(b, &mut p)?);
                    }
                    if flags & HAS_VERSION != 0 {
                        packed(b, &mut p)?;
                    }
                    if flags & HAS_SIZE != 0 {
                        t.byte_size = packed(b, &mut p)?;
                        packed(b, &mut p)?; // alignment
                    }
                    if flags & HAS_UNKNOWN != 0 {
                        packed(b, &mut p)?;
                    }
                    if flags & HAS_FIELDS != 0 {
                        // As the reference reads it: a count byte whose top bits are flags,
                        // with `C3 00 <packed>` as hkPropertyId's escape.
                        let mut c = u64::from(u8_at(b, p)?);
                        p += 1;
                        if c == 0xC3 {
                            c = u64::from(u8_at(b, p)?);
                            p += 1;
                            if c == 0 {
                                c = packed(b, &mut p)?;
                            }
                        }
                        for _ in 0..(c & 0x3F) {
                            let name = name_at(&fstr, packed(b, &mut p)?)?;
                            packed(b, &mut p)?; // field flags
                            let offset = packed(b, &mut p)? as usize;
                            let ty = index(packed(b, &mut p)?);
                            t.fields.push(Field { name, offset, ty });
                        }
                    }
                    if flags & HAS_INTERFACES != 0 {
                        for _ in 0..packed(b, &mut p)? {
                            packed(b, &mut p)?;
                            packed(b, &mut p)?;
                        }
                    }
                    types[ti] = t;
                }
            }
            b"THSH" | b"TPTR" | b"TPAD" => {}
            other => bail!("unexpected TYPE subsection {}", String::from_utf8_lossy(other)),
        }
    }
    Ok(types)
}

fn read_items(b: &[u8], indx: &Section, data: usize) -> Result<Vec<Item>> {
    let item = indx.child(b"ITEM").context("tagfile has no INDX/ITEM")?;
    let mut out = Vec::new();
    let mut p = item.start;
    while p + 12 <= item.end {
        out.push(Item {
            ty: index(u64::from(u32_at(b, p)? & 0x00FF_FFFF)),
            at: data + u32_at(b, p + 4)? as usize,
            count: u32_at(b, p + 8)? as usize,
        });
        p += 12;
    }
    Ok(out)
}

impl Tagfile {
    pub fn parse(bytes: Vec<u8>) -> Result<Tagfile> {
        let root = read_section(&bytes, 0)?;
        if &root.tag != b"TAG0" {
            bail!("not a Havok tagfile (root {})", String::from_utf8_lossy(&root.tag));
        }
        let sdk = root.child(b"SDKV").context("tagfile has no SDKV")?;
        let version = slice(&bytes, sdk.start, 8)?;
        if version != b"20150100" {
            bail!("Havok SDK {} is not 2015.1", String::from_utf8_lossy(version));
        }
        let data = root.child(b"DATA").context("tagfile has no DATA")?.start;
        let types = read_types(&bytes, root.child(b"TYPE").context("tagfile has no TYPE")?)?;
        let items = read_items(&bytes, root.child(b"INDX").context("tagfile has no INDX")?, data)?;
        Ok(Tagfile { bytes, types, items })
    }

    /// The root object (`hkRootLevelContainer`): item 1's first element.
    pub fn root(&self) -> Result<Obj<'_>> {
        let it = self.items.get(1).context("tagfile has no root item")?;
        Ok(Obj {
            tf: self,
            ty: it.ty.context("root item has no type")?,
            at: it.at,
        })
    }

    /// A type property, inherited from the parents when the type does not carry it.
    fn inherit<T>(&self, mut i: usize, flag: u64, get: impl Fn(&Type) -> T, default: T) -> T {
        for _ in 0..64 {
            let t = &self.types[i];
            if t.flags & flag != 0 {
                return get(t);
            }
            match t.parent {
                Some(p) => i = p,
                None => return default,
            }
        }
        default
    }

    fn format_info(&self, i: usize) -> u64 {
        self.inherit(i, HAS_FORMAT, |t| t.format_info, 0)
    }

    pub fn kind(&self, i: usize) -> Kind {
        match self.format_info(i) & 0xF {
            0 => Kind::Void,
            2 => Kind::Bool,
            3 => Kind::String,
            4 => Kind::Int,
            5 => Kind::Float,
            6 => Kind::Ptr,
            7 => Kind::Class,
            8 => Kind::Array,
            _ => Kind::Opaque,
        }
    }

    fn byte_size(&self, i: usize) -> usize {
        self.inherit(i, HAS_SIZE, |t| t.byte_size, 0) as usize
    }

    fn sub_type(&self, i: usize) -> Option<usize> {
        self.inherit(i, HAS_SUBTYPE, |t| t.sub_type, None)
    }

    /// Fields of the type and its parents, the parents' first.
    fn all_fields(&self, i: usize) -> Vec<&Field> {
        let mut chain = vec![i];
        while let Some(p) = self.types[*chain.last().expect("non-empty")].parent {
            if chain.len() > 64 {
                break;
            }
            chain.push(p);
        }
        chain.iter().rev().flat_map(|&t| self.types[t].fields.iter()).collect()
    }

    /// The elements of item `idx`; a pointer or string element is a 4-byte item index.
    fn item_elements(&self, idx: usize) -> Result<Vec<Obj<'_>>> {
        if idx == 0 {
            return Ok(Vec::new());
        }
        let it = *self
            .items
            .get(idx)
            .with_context(|| format!("item index {idx} past the item table"))?;
        let ty = it.ty.context("item has no type")?;
        let fixed_string = self.format_info(ty) & 0x20 != 0;
        let stride = match self.kind(ty) {
            Kind::Ptr => 4,
            Kind::String if !fixed_string => 4,
            _ => self.byte_size(ty),
        };
        Ok((0..it.count)
            .map(|i| Obj { tf: self, ty, at: it.at + i * stride })
            .collect())
    }
}

/// An object: a type and where its bytes start.
#[derive(Clone, Copy)]
pub struct Obj<'a> {
    tf: &'a Tagfile,
    pub ty: usize,
    at: usize,
}

impl<'a> Obj<'a> {
    pub fn type_name(&self) -> &'a str {
        &self.tf.types[self.ty].name
    }

    pub fn kind(&self) -> Kind {
        self.tf.kind(self.ty)
    }

    pub fn field(&self, name: &str) -> Result<Obj<'a>> {
        let f = self
            .tf
            .all_fields(self.ty)
            .into_iter()
            .find(|f| f.name == name)
            .with_context(|| format!("{} has no field {name}", self.type_name()))?;
        Ok(Obj {
            tf: self.tf,
            ty: f.ty.with_context(|| format!("field {name} has no type"))?,
            at: self.at + f.offset,
        })
    }

    /// The only field of a one-field class (a wrapper such as `hkRefVariant`).
    fn unwrap_single(&self) -> Option<Obj<'a>> {
        let fields = self.tf.all_fields(self.ty);
        (self.kind() == Kind::Class && fields.len() == 1)
            .then(|| self.field(&fields[0].name).ok())
            .flatten()
    }

    pub fn int(&self) -> Result<i64> {
        let fi = self.tf.format_info(self.ty);
        if !matches!(self.kind(), Kind::Int | Kind::Bool) {
            bail!("{} is not an integer", self.type_name());
        }
        let signed = fi & 0x200 != 0;
        let b = &self.tf.bytes;
        Ok(match (fi >> 10, signed) {
            (8, false) => i64::from(u8_at(b, self.at)?),
            (8, true) => i64::from(u8_at(b, self.at)? as i8),
            (16, false) => i64::from(crate::bytes::u16_at(b, self.at)?),
            (16, true) => i64::from(crate::bytes::i16_at(b, self.at)?),
            (32, false) => i64::from(u32_at(b, self.at)?),
            (32, true) => i64::from(crate::bytes::i32_at(b, self.at)?),
            (64, _) => crate::bytes::u64_at(b, self.at)? as i64,
            (bits, _) => bail!("{}: unexpected integer width {bits}", self.type_name()),
        })
    }

    pub fn float(&self) -> Result<f32> {
        if self.kind() != Kind::Float || self.tf.format_info(self.ty) >> 8 != 0x1746 {
            bail!("{} is not a 32-bit float", self.type_name());
        }
        f32_at(&self.tf.bytes, self.at)
    }

    /// A pointer's target, `None` for null; a one-field wrapper is looked through.
    pub fn ptr(&self) -> Result<Option<Obj<'a>>> {
        if self.kind() != Kind::Ptr {
            return match self.unwrap_single() {
                Some(inner) => inner.ptr(),
                None => bail!("{} is not a pointer", self.type_name()),
            };
        }
        let idx = u32_at(&self.tf.bytes, self.at)? as usize;
        Ok(self.tf.item_elements(idx)?.into_iter().next())
    }

    pub fn array(&self) -> Result<Vec<Obj<'a>>> {
        if self.kind() != Kind::Array {
            bail!("{} is not an array", self.type_name());
        }
        let fi = self.tf.format_info(self.ty);
        if fi & 0x20 != 0 {
            let sub = self.tf.sub_type(self.ty).context("fixed array without element type")?;
            let size = self.tf.byte_size(sub);
            return Ok((0..(fi >> 8) as usize)
                .map(|i| Obj { tf: self.tf, ty: sub, at: self.at + i * size })
                .collect());
        }
        self.tf.item_elements(u32_at(&self.tf.bytes, self.at)? as usize)
    }

    pub fn string(&self) -> Result<String> {
        if self.kind() != Kind::String {
            bail!("{} is not a string", self.type_name());
        }
        let fi = self.tf.format_info(self.ty);
        let raw: Vec<u8> = if fi & 0x20 != 0 {
            slice(&self.tf.bytes, self.at, (fi >> 16) as usize)?.to_vec()
        } else {
            self.tf
                .item_elements(u32_at(&self.tf.bytes, self.at)? as usize)?
                .iter()
                .map(|c| u8_at(&self.tf.bytes, c.at))
                .collect::<Result<_>>()?
        };
        let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
        Ok(String::from_utf8_lossy(&raw[..end]).into_owned())
    }

    /// The floats of a float array, or of a one-field class wrapping one (`hkVector4f`).
    pub fn floats(&self) -> Result<Vec<f32>> {
        if self.kind() == Kind::Array {
            return self.array()?.iter().map(Obj::float).collect();
        }
        match self.unwrap_single() {
            Some(inner) => inner.floats(),
            None => bail!("{} is not a float vector", self.type_name()),
        }
    }

    /// The integers of an integer array.
    pub fn ints(&self) -> Result<Vec<i64>> {
        self.array()?.iter().map(Obj::int).collect()
    }

    /// A byte array's bytes, borrowed from the file.
    pub fn bytes(&self) -> Result<&'a [u8]> {
        let idx = u32_at(&self.tf.bytes, self.at)? as usize;
        let elems = self.tf.item_elements(idx)?;
        let Some(first) = elems.first() else {
            return Ok(&[]);
        };
        if self.tf.byte_size(first.ty) != 1 {
            bail!("{} is not a byte array", self.type_name());
        }
        slice(&self.tf.bytes, first.at, elems.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_ints_take_one_to_four_bytes() {
        let read = |b: &[u8]| {
            let mut p = 0;
            (packed(b, &mut p).unwrap(), p)
        };
        assert_eq!(read(&[0x05]), (5, 1));
        assert_eq!(read(&[0x81, 0x23]), (0x0123, 2));
        assert_eq!(read(&[0xC1, 0x02, 0x03]), (0x01_0203, 3));
        assert_eq!(read(&[0xE1, 0x00, 0x00, 0x01]), (0x100_0001, 4));
    }

    #[test]
    fn sections_nest_until_a_leaf_flag() {
        // TAG0 (container) holding SDKV (leaf, 8 bytes payload).
        let mut b = Vec::new();
        b.extend_from_slice(&(8u32 + 16).to_be_bytes());
        b.extend_from_slice(b"TAG0");
        b.extend_from_slice(&(0x4000_0000u32 | 16).to_be_bytes());
        b.extend_from_slice(b"SDKV");
        b.extend_from_slice(b"20150100");
        let s = read_section(&b, 0).unwrap();
        assert_eq!(&s.tag, b"TAG0");
        let sdk = s.child(b"SDKV").unwrap();
        assert_eq!(&b[sdk.start..sdk.end], b"20150100");
    }

    #[test]
    fn a_section_past_the_end_is_an_error() {
        let mut b = Vec::new();
        b.extend_from_slice(&64u32.to_be_bytes());
        b.extend_from_slice(b"TAG0");
        assert!(read_section(&b, 0).is_err());
    }
}
```

- [ ] **Step 3: Run** — `cargo test -j 4 -p dsr-formats tagfile` → `3 passed`.

- [ ] **Step 4: Install smoke test (real file)** — append to `tagfile.rs` tests:

```rust
    /// `DSR_DIR=<install>`: the player skeleton file parses, and its root is the container.
    #[test]
    fn install_player_skeleton_parses() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let bnd = std::fs::read(format!("{dir}/chr/c0000.anibnd.dcx")).unwrap();
        let entries = crate::bnd3::read(&crate::dcx::decompress(&bnd).unwrap()).unwrap();
        let skel = entries.iter().find(|e| e.file_name() == "Skeleton.hkx").unwrap();
        let tf = Tagfile::parse(skel.data.clone()).unwrap();
        assert_eq!(tf.root().unwrap().type_name(), "hkRootLevelContainer");
        let c = crate::hkx::animation_container(&tf).unwrap();
        assert_eq!(c.field("skeletons").unwrap().array().unwrap().len(), 1);
    }
```

Run: `DSR_DIR='C:/Spel/Dark Souls Remastered' cargo test -j 4 -p dsr-formats install_player_skeleton -- --nocapture`
Expected: PASS. If it fails, print the offending type and offset with a temporary `eprintln!` in `read_types` and compare against `HkBinaryTypeReader.cs`; fix the reader, not the test.

- [ ] **Step 5: Gates checkpoint** — fmt + clippy for `dsr-formats` clean.

---

### Task 6: Skeleton

**Files:**
- Modify: `crates/dsr-formats/src/hkx/skeleton.rs`

- [ ] **Step 1: Implement with an install test**

```rust
//! `hkaSkeleton`: bone names, parents and the reference pose (SoulsAssetPipeline
//! `HKX.GenFakeFromTagFile.cs` field names).

use anyhow::{bail, Context, Result};

use super::tagfile::{Obj, Tagfile};

/// A Havok `hkQsTransform`: translation, rotation `(x, y, z, w)`, scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Qs {
    pub t: [f32; 3],
    pub r: [f32; 4],
    pub s: [f32; 3],
}

impl Qs {
    pub const IDENTITY: Qs = Qs {
        t: [0.0; 3],
        r: [0.0, 0.0, 0.0, 1.0],
        s: [1.0; 3],
    };
}

#[derive(Debug, Clone)]
pub struct Skeleton {
    pub names: Vec<String>,
    pub parents: Vec<i16>,
    pub reference: Vec<Qs>,
}

impl Skeleton {
    pub fn bone(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name)
    }
}

fn three(v: &[f32], what: &str) -> Result<[f32; 3]> {
    Ok([
        *v.first().with_context(|| format!("{what} is empty"))?,
        *v.get(1).with_context(|| format!("{what} has 1 value"))?,
        *v.get(2).with_context(|| format!("{what} has 2 values"))?,
    ])
}

pub(crate) fn qs(o: &Obj) -> Result<Qs> {
    let r = o.field("rotation")?.floats()?;
    if r.len() < 4 {
        bail!("rotation has {} values", r.len());
    }
    Ok(Qs {
        t: three(&o.field("translation")?.floats()?, "translation")?,
        r: [r[0], r[1], r[2], r[3]],
        s: three(&o.field("scale")?.floats()?, "scale")?,
    })
}

pub fn read(tf: &Tagfile) -> Result<Skeleton> {
    let c = super::animation_container(tf)?;
    let sk = c
        .field("skeletons")?
        .array()?
        .into_iter()
        .next()
        .context("no skeleton in the container")?
        .ptr()?
        .context("null skeleton")?;
    let parents: Vec<i16> = sk
        .field("parentIndices")?
        .ints()?
        .into_iter()
        .map(|v| v as i16)
        .collect();
    let names = sk
        .field("bones")?
        .array()?
        .iter()
        .map(|b| b.field("name")?.string())
        .collect::<Result<Vec<_>>>()?;
    let reference = sk
        .field("referencePose")?
        .array()?
        .iter()
        .map(qs)
        .collect::<Result<Vec<_>>>()?;
    if names.len() != parents.len() || reference.len() != parents.len() {
        bail!(
            "skeleton tables disagree: {} names, {} parents, {} reference transforms",
            names.len(),
            parents.len(),
            reference.len()
        );
    }
    Ok(Skeleton { names, parents, reference })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_player_skeleton_has_its_named_bones() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let bnd = std::fs::read(format!("{dir}/chr/c0000.anibnd.dcx")).unwrap();
        let entries = crate::bnd3::read(&crate::dcx::decompress(&bnd).unwrap()).unwrap();
        let e = entries.iter().find(|e| e.file_name() == "Skeleton.hkx").unwrap();
        let sk = read(&Tagfile::parse(e.data.clone()).unwrap()).unwrap();
        for name in ["Master", "Pelvis", "Spine", "Spine1", "Neck", "Head", "L_UpperArm", "R_Thigh", "L_Toe0"] {
            assert!(sk.bone(name).is_some(), "missing {name}");
        }
        // Parent-sorted, as the retarget assumes.
        for (i, &p) in sk.parents.iter().enumerate() {
            assert!(p < 0 || (p as usize) < i, "bone {i} has parent {p}");
        }
        for q in &sk.reference {
            let n = q.r.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!((n - 1.0).abs() < 1.0e-3, "reference rotation not unit: {q:?}");
        }
    }
}
```

- [ ] **Step 2: Run** — `DSR_DIR='C:/Spel/Dark Souls Remastered' cargo test -j 4 -p dsr-formats install_player_skeleton -- --nocapture` → both install tests PASS.
- [ ] **Step 3: Gates checkpoint.**

---

### Task 7: Spline decompression

**Files:**
- Modify: `crates/dsr-formats/src/hkx/spline.rs`

- [ ] **Step 1: Write the module with its unit tests; `decode` stubbed to fail**

```rust
//! `hkaSplineCompressedAnimation` data: per block, per transform track, a mask (quantization
//! and static/spline flags per axis), then position, rotation and scale as static values or
//! B-spline control points with byte knots; sampled per frame.
//!
//! Ported from SoulsAssetPipeline `SplineCompressedAnimation.cs` (GPL-3.0, after HavokLib's
//! `hka_spline_decompressor.cpp`). Deviation: `POLAR32` converts its 10-bit radius numerically,
//! as HavokLib does (the C# port reinterprets the bits as a float).

use anyhow::{bail, Context, Result};

use super::animation::SplineAnim;
use super::skeleton::Qs;
use crate::bytes::{f32_at, i16_at, slice, u16_at, u8_at};

const STATIC: [u8; 4] = [0x01, 0x02, 0x04, 0x08];
const SPLINE: [u8; 4] = [0x10, 0x20, 0x40, 0x80];

#[derive(Debug, Clone, Copy)]
enum Scalar {
    B8,
    B16,
}

#[derive(Debug, Clone, Copy)]
enum Rot {
    Polar32,
    ThreeComp40,
    ThreeComp48,
    Uncompressed,
}

impl Rot {
    fn from_bits(v: u8) -> Result<Rot> {
        Ok(match v {
            0 => Rot::Polar32,
            1 => Rot::ThreeComp40,
            2 => Rot::ThreeComp48,
            5 => Rot::Uncompressed,
            other => bail!("rotation quantization {other} (24-bit/straight-16) is not supported"),
        })
    }

    fn align(self) -> usize {
        match self {
            Rot::Polar32 | Rot::Uncompressed => 4,
            Rot::ThreeComp40 => 1,
            Rot::ThreeComp48 => 2,
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Result<u8> {
        let v = u8_at(self.b, self.p)?;
        self.p += 1;
        Ok(v)
    }
    fn u16(&mut self) -> Result<u16> {
        let v = u16_at(self.b, self.p)?;
        self.p += 2;
        Ok(v)
    }
    fn i16(&mut self) -> Result<i16> {
        let v = i16_at(self.b, self.p)?;
        self.p += 2;
        Ok(v)
    }
    fn f32(&mut self) -> Result<f32> {
        let v = f32_at(self.b, self.p)?;
        self.p += 4;
        Ok(v)
    }
    fn bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        let v = slice(self.b, self.p, n)?.to_vec();
        self.p += n;
        Ok(v)
    }
    fn align(&mut self, n: usize) {
        self.p = self.p.div_ceil(n) * n;
    }
    fn scalar(&mut self, q: Scalar, min: f32, max: f32) -> Result<f32> {
        let ratio = match q {
            Scalar::B8 => f32::from(self.u8()?) / 255.0,
            Scalar::B16 => f32::from(self.u16()?) / 65535.0,
        };
        Ok(min + (max - min) * ratio)
    }
    fn quat(&mut self, q: Rot) -> Result<[f32; 4]> {
        match q {
            Rot::Polar32 => {
                let v = u32::from_le_bytes(self.bytes(4)?.try_into().expect("4 bytes"));
                Ok(polar32(v))
            }
            Rot::ThreeComp40 => {
                let b = self.bytes(5)?;
                let mut w = [0u8; 8];
                w[..5].copy_from_slice(&b);
                Ok(three_comp40(u64::from_le_bytes(w)))
            }
            Rot::ThreeComp48 => {
                let (x, y, z) = (self.i16()?, self.i16()?, self.i16()?);
                Ok(three_comp48(x, y, z))
            }
            Rot::Uncompressed => Ok([self.f32()?, self.f32()?, self.f32()?, self.f32()?]),
        }
    }
}

/// Three stored components and the dropped one's index and sign.
fn rebuild(c: [f32; 3], dropped: usize, negative: bool) -> [f32; 4] {
    let mut q = [0.0; 4];
    let mut k = 0;
    for (i, slot) in q.iter_mut().enumerate() {
        if i != dropped {
            *slot = c[k];
            k += 1;
        }
    }
    let rest = 1.0 - c[0] * c[0] - c[1] * c[1] - c[2] * c[2];
    let w = if rest <= 0.0 { 0.0 } else { rest.sqrt() };
    q[dropped] = if negative { -w } else { w };
    q
}

pub(crate) fn three_comp40(v: u64) -> [f32; 4] {
    const MASK: u64 = (1 << 12) - 1;
    const HALF: i32 = (MASK >> 1) as i32;
    const FRACTAL: f32 = 0.000_345_436;
    let comp = |s: u32| ((v >> s) & MASK) as i32 - HALF;
    let c = [comp(0), comp(12), comp(24)].map(|x| x as f32 * FRACTAL);
    rebuild(c, ((v >> 36) & 3) as usize, (v >> 38) & 1 != 0)
}

pub(crate) fn three_comp48(x: i16, y: i16, z: i16) -> [f32; 4] {
    const MASK: i32 = (1 << 15) - 1;
    const FRACTAL: f32 = 0.000_043_161;
    let (x, y, z) = (i32::from(x), i32::from(y), i32::from(z));
    let dropped = (((y >> 14) & 2) | ((x >> 15) & 1)) as usize;
    let negative = (z >> 15) != 0;
    let c = [x, y, z].map(|v| ((v & MASK) - (MASK >> 1)) as f32 * FRACTAL);
    rebuild(c, dropped, negative)
}

pub(crate) fn polar32(v: u32) -> [f32; 4] {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
    let r = ((v >> 18) & 0x3FF) as f32 / 1023.0;
    let r = 1.0 - r * r;
    let phi_theta = (v & 0x3_FFFF) as f32;
    let mut phi = phi_theta.sqrt().floor();
    let mut theta = 0.0;
    if phi > 0.0 {
        theta = FRAC_PI_4 * (phi_theta - phi * phi) / phi;
        phi *= FRAC_PI_2 / 511.0;
    }
    let mag = (1.0 - r * r).max(0.0).sqrt();
    let mut q = [
        phi.sin() * theta.cos() * mag,
        phi.sin() * theta.sin() * mag,
        phi.cos() * mag,
        r,
    ];
    for (i, bit) in [0x1000_0000u32, 0x2000_0000, 0x4000_0000, 0x8000_0000].iter().enumerate() {
        if v & bit != 0 {
            q[i] = -q[i];
        }
    }
    q
}

#[derive(Debug, Clone)]
enum Channel {
    Absent,
    Static(f32),
    Spline(Vec<f32>),
}

#[derive(Debug, Clone)]
struct Curve {
    degree: usize,
    knots: Vec<u8>,
}

#[derive(Debug, Clone)]
struct VecTrack {
    curve: Option<Curve>,
    axes: [Channel; 3],
}

#[derive(Debug, Clone)]
enum QuatTrack {
    None,
    Static([f32; 4]),
    Spline(Curve, Vec<[f32; 4]>),
}

#[derive(Debug, Clone)]
struct Track {
    pos: VecTrack,
    rot: QuatTrack,
    scale: VecTrack,
}

fn read_curve(r: &mut Reader) -> Result<(Curve, usize)> {
    let n = usize::try_from(r.i16()?).context("negative control point count")?;
    let degree = usize::from(r.u8()?);
    if degree > 4 {
        bail!("spline degree {degree} is above 4");
    }
    let knots = r.bytes(n + degree + 2)?;
    Ok((Curve { degree, knots }, n))
}

fn read_vec(r: &mut Reader, flags: u8, q: Scalar) -> Result<VecTrack> {
    if flags & 0x70 == 0 {
        let mut axes = [Channel::Absent, Channel::Absent, Channel::Absent];
        for (i, a) in axes.iter_mut().enumerate() {
            if flags & STATIC[i] != 0 {
                *a = Channel::Static(r.f32()?);
            }
        }
        return Ok(VecTrack { curve: None, axes });
    }
    let (curve, n) = read_curve(r)?;
    r.align(4);
    let mut bounds = [(0.0f32, 0.0f32); 3];
    let mut axes = [Channel::Absent, Channel::Absent, Channel::Absent];
    for i in 0..3 {
        if flags & SPLINE[i] != 0 {
            bounds[i] = (r.f32()?, r.f32()?);
            axes[i] = Channel::Spline(Vec::with_capacity(n + 1));
        } else if flags & STATIC[i] != 0 {
            axes[i] = Channel::Static(r.f32()?);
        }
    }
    for _ in 0..=n {
        for i in 0..3 {
            if let Channel::Spline(v) = &mut axes[i] {
                v.push(r.scalar(q, bounds[i].0, bounds[i].1)?);
            }
        }
    }
    Ok(VecTrack { curve: Some(curve), axes })
}

fn read_quat(r: &mut Reader, flags: u8, q: Rot) -> Result<QuatTrack> {
    if flags & 0xF0 != 0 {
        let (curve, n) = read_curve(r)?;
        r.align(q.align());
        let points = (0..=n).map(|_| r.quat(q)).collect::<Result<Vec<_>>>()?;
        return Ok(QuatTrack::Spline(curve, points));
    }
    if flags & 0x0F != 0 {
        r.align(q.align());
        return Ok(QuatTrack::Static(r.quat(q)?));
    }
    Ok(QuatTrack::None)
}

fn read_block(data: &[u8], at: usize, tracks: usize) -> Result<Vec<Track>> {
    let mut r = Reader { b: data, p: at };
    let mut masks = Vec::with_capacity(tracks);
    for _ in 0..tracks {
        let q = r.u8()?;
        let (pos, rot, scale) = (r.u8()?, r.u8()?, r.u8()?);
        masks.push((q, pos, rot, scale));
    }
    r.align(4);
    let scalar = |v: u8| if v & 3 == 0 { Scalar::B8 } else { Scalar::B16 };
    let mut out = Vec::with_capacity(tracks);
    for &(q, pos_f, rot_f, scale_f) in &masks {
        let pos = read_vec(&mut r, pos_f, scalar(q))?;
        r.align(4);
        let rot = read_quat(&mut r, rot_f, Rot::from_bits((q >> 2) & 0xF)?)?;
        r.align(4);
        let scale = read_vec(&mut r, scale_f, scalar(q >> 6))?;
        r.align(4);
        out.push(Track { pos, rot, scale });
    }
    Ok(out)
}

/// The knot span holding `u` (The NURBS Book, A2.1).
fn find_span(degree: usize, u: f32, points: usize, knots: &[u8]) -> usize {
    if u >= f32::from(knots[points]) {
        return points - 1;
    }
    let (mut low, mut high) = (degree, points);
    let mut mid = (low + high) / 2;
    for _ in 0..64 {
        if u >= f32::from(knots[mid]) && u < f32::from(knots[mid + 1]) {
            break;
        }
        if u < f32::from(knots[mid]) {
            high = mid;
        } else {
            low = mid;
        }
        mid = (low + high) / 2;
    }
    mid
}

/// The B-spline basis weights at `u` for the span (as the reference evaluates them).
fn basis(span: usize, degree: usize, u: f32, knots: &[u8]) -> [f32; 5] {
    let k = |i: usize| f32::from(knots[i]);
    let mut n = [1.0, 0.0, 0.0, 0.0, 0.0];
    for i in 1..=degree {
        for j in (0..i).rev() {
            let denom = k(span + i - j) - k(span - j);
            let a = if denom == 0.0 { 0.0 } else { (u - k(span - j)) / denom };
            let tmp = n[j] * a;
            n[j + 1] += n[j] - tmp;
            n[j] = tmp;
        }
    }
    n
}

fn eval_scalar(c: &Curve, u: f32, points: &[f32]) -> f32 {
    if points.len() == 1 {
        return points[0];
    }
    let span = find_span(c.degree, u, points.len(), &c.knots);
    let n = basis(span, c.degree, u, &c.knots);
    (0..=c.degree).map(|i| points[span - i] * n[i]).sum()
}

fn eval_quat(c: &Curve, u: f32, points: &[[f32; 4]]) -> [f32; 4] {
    if points.len() == 1 {
        return points[0];
    }
    let span = find_span(c.degree, u, points.len(), &c.knots);
    let n = basis(span, c.degree, u, &c.knots);
    let mut q = [0.0; 4];
    for i in 0..=c.degree {
        for (k, v) in q.iter_mut().enumerate() {
            *v += points[span - i][k] * n[i];
        }
    }
    let len = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if len > 0.0 {
        q.map(|v| v / len)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

fn vec_at(t: &VecTrack, u: f32, default: f32) -> [f32; 3] {
    let mut out = [default; 3];
    for (i, a) in t.axes.iter().enumerate() {
        out[i] = match a {
            Channel::Absent => default,
            Channel::Static(v) => *v,
            Channel::Spline(p) => eval_scalar(t.curve.as_ref().expect("spline has a curve"), u, p),
        };
    }
    out
}

/// Every frame of every transform track: `frames[f][track]`.
pub fn decode(anim: &SplineAnim) -> Result<Vec<Vec<Qs>>> {
    let _ = (anim, read_block, vec_at, eval_quat);
    bail!("not implemented")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 4], b: [f32; 4]) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1.0e-3)
    }

    #[test]
    fn quantized_identities_decode_to_identity() {
        let id = [0.0, 0.0, 0.0, 1.0];
        let v40 = 0x7FFu64 | (0x7FF << 12) | (0x7FF << 24) | (3 << 36);
        assert!(close(three_comp40(v40), id), "{:?}", three_comp40(v40));
        // x and y carry the dropped index (3) in their top bits; all three are the midpoint.
        let x = 0xBFFFu16 as i16;
        let y = 0xBFFFu16 as i16;
        assert!(close(three_comp48(x, y, 0x3FFF), id), "{:?}", three_comp48(x, y, 0x3FFF));
        assert!(close(polar32(0), id), "{:?}", polar32(0));
    }

    #[test]
    fn a_degree_one_spline_interpolates_between_its_points() {
        let c = Curve {
            degree: 1,
            knots: vec![0, 0, 10, 10],
        };
        assert!((eval_scalar(&c, 0.0, &[2.0, 4.0]) - 2.0).abs() < 1.0e-6);
        assert!((eval_scalar(&c, 5.0, &[2.0, 4.0]) - 3.0).abs() < 1.0e-6);
        assert!((eval_scalar(&c, 10.0, &[2.0, 4.0]) - 4.0).abs() < 1.0e-6);
    }

    /// One block, one track: static position (1, 2, 3), an uncompressed degree-1 rotation
    /// spline from identity to a half turn about Y over frames 0..10, no scale.
    fn one_track_anim() -> SplineAnim {
        let mut d = Vec::new();
        d.extend_from_slice(&[0x14, 0x07, 0xF0, 0x00]); // rot UNCOMPRESSED; static xyz; spline xyzw
        for v in [1.0f32, 2.0, 3.0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&1i16.to_le_bytes()); // n = 1: two control points
        d.push(1); // degree
        d.extend_from_slice(&[0, 0, 10, 10]); // knots
        while d.len() % 4 != 0 {
            d.push(0);
        }
        for q in [[0.0f32, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 0.0]] {
            for v in q {
                d.extend_from_slice(&v.to_le_bytes());
            }
        }
        while d.len() % 16 != 0 {
            d.push(0);
        }
        SplineAnim {
            duration: 10.0 / 30.0,
            tracks: 1,
            frames: 11,
            blocks: 1,
            frames_per_block: 11,
            block_duration: 10.0 / 30.0,
            frame_duration: 1.0 / 30.0,
            block_offsets: vec![0],
            data: d,
        }
    }

    #[test]
    fn a_block_decodes_to_its_frames() {
        let frames = decode(&one_track_anim()).unwrap();
        assert_eq!(frames.len(), 11);
        assert_eq!(frames[0][0].t, [1.0, 2.0, 3.0]);
        assert_eq!(frames[0][0].s, [1.0, 1.0, 1.0]);
        assert!(close(frames[0][0].r, [0.0, 0.0, 0.0, 1.0]));
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!(close(frames[5][0].r, [0.0, h, 0.0, h]), "{:?}", frames[5][0].r);
        assert!(close(frames[10][0].r, [0.0, 1.0, 0.0, 0.0]), "{:?}", frames[10][0].r);
    }
}
```

- [ ] **Step 2: Run to verify** — `cargo test -j 4 -p dsr-formats spline` → `quantized_identities…` and `a_degree_one…` PASS, `a_block_decodes_to_its_frames` FAILS `not implemented`. `SplineAnim` is defined in Task 8: write Task 8's module (Step 1) before this step so the crate compiles; its install test runs in Task 8 Step 2.

- [ ] **Step 3: Implement `decode`**

```rust
pub fn decode(anim: &SplineAnim) -> Result<Vec<Vec<Qs>>> {
    if anim.blocks == 0 || anim.block_offsets.len() < anim.blocks {
        bail!("animation has {} blocks and {} block offsets", anim.blocks, anim.block_offsets.len());
    }
    let blocks = anim
        .block_offsets
        .iter()
        .take(anim.blocks)
        .map(|&at| read_block(&anim.data, at, anim.tracks))
        .collect::<Result<Vec<_>>>()?;
    let mut frames = Vec::with_capacity(anim.frames);
    for f in 0..anim.frames {
        let t = f as f32 * anim.frame_duration;
        let block = (((t + 1.0e-4) / anim.block_duration) as usize).min(anim.blocks - 1);
        let u = (t - block as f32 * anim.block_duration) / anim.frame_duration;
        let frame = blocks[block]
            .iter()
            .map(|tr| Qs {
                t: vec_at(&tr.pos, u, 0.0),
                r: match &tr.rot {
                    QuatTrack::None => [0.0, 0.0, 0.0, 1.0],
                    QuatTrack::Static(q) => *q,
                    QuatTrack::Spline(c, p) => eval_quat(c, u, p),
                },
                s: vec_at(&tr.scale, u, 1.0),
            })
            .collect();
        frames.push(frame);
    }
    Ok(frames)
}
```

- [ ] **Step 4: Run** — `cargo test -j 4 -p dsr-formats spline` → `3 passed`.
- [ ] **Step 5: Gates checkpoint.**

---

### Task 8: Animation file (spline params, binding, root motion)

**Files:**
- Modify: `crates/dsr-formats/src/hkx/animation.rs`

- [ ] **Step 1: Write the module**

```rust
//! One animation file: the `hkaSplineCompressedAnimation`, its binding (track → bone) and its
//! root motion (`hkaDefaultAnimatedReferenceFrame`), by the field names of SoulsAssetPipeline's
//! `HKX.GenFakeFromTagFile.cs`.

use anyhow::{bail, Context, Result};

use super::tagfile::{Obj, Tagfile};

#[derive(Debug, Clone)]
pub struct SplineAnim {
    pub duration: f32,
    pub tracks: usize,
    pub frames: usize,
    pub blocks: usize,
    pub frames_per_block: usize,
    pub block_duration: f32,
    pub frame_duration: f32,
    pub block_offsets: Vec<usize>,
    pub data: Vec<u8>,
}

/// Accumulated root motion samples `(x, y, z, yaw)`, evenly spaced over `duration`.
#[derive(Debug, Clone)]
pub struct RootMotion {
    pub up: [f32; 3],
    pub forward: [f32; 3],
    pub duration: f32,
    pub samples: Vec<[f32; 4]>,
}

#[derive(Debug, Clone)]
pub struct AnimFile {
    pub anim: SplineAnim,
    pub root: Option<RootMotion>,
    /// Per transform track, the skeleton bone it drives (`-1`: none).
    pub track_to_bone: Vec<i16>,
}

fn int(o: &Obj, name: &str) -> Result<usize> {
    usize::try_from(o.field(name)?.int()?).with_context(|| format!("{name} is negative"))
}

fn vec3(o: &Obj, name: &str) -> Result<[f32; 3]> {
    let v = o.field(name)?.floats()?;
    if v.len() < 3 {
        bail!("{name} has {} values", v.len());
    }
    Ok([v[0], v[1], v[2]])
}

fn root_motion(o: &Obj) -> Result<RootMotion> {
    let samples = o
        .field("referenceFrameSamples")?
        .array()?
        .iter()
        .map(|s| {
            let v = s.floats()?;
            if v.len() < 4 {
                bail!("root sample has {} values", v.len());
            }
            Ok([v[0], v[1], v[2], v[3]])
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(RootMotion {
        up: vec3(o, "up")?,
        forward: vec3(o, "forward")?,
        duration: o.field("duration")?.float()?,
        samples,
    })
}

pub fn read(tf: &Tagfile) -> Result<AnimFile> {
    let c = super::animation_container(tf)?;
    let a = c
        .field("animations")?
        .array()?
        .into_iter()
        .next()
        .context("no animation in the container")?
        .ptr()?
        .context("null animation")?;
    if a.type_name() != "hkaSplineCompressedAnimation" {
        bail!("animation is a {}, not spline-compressed", a.type_name());
    }
    let anim = SplineAnim {
        duration: a.field("duration")?.float()?,
        tracks: int(&a, "numberOfTransformTracks")?,
        frames: int(&a, "numFrames")?,
        blocks: int(&a, "numBlocks")?,
        frames_per_block: int(&a, "maxFramesPerBlock")?,
        block_duration: a.field("blockDuration")?.float()?,
        frame_duration: a.field("frameDuration")?.float()?,
        block_offsets: a
            .field("blockOffsets")?
            .ints()?
            .into_iter()
            .map(|v| v as usize)
            .collect(),
        data: a.field("data")?.bytes()?.to_vec(),
    };
    let root = match a.field("extractedMotion")?.ptr()? {
        Some(rf) if rf.type_name() == "hkaDefaultAnimatedReferenceFrame" => Some(root_motion(&rf)?),
        _ => None,
    };
    let track_to_bone = match c.field("bindings")?.array()?.into_iter().next() {
        Some(b) => match b.ptr()? {
            Some(b) => b
                .field("transformTrackToBoneIndices")?
                .ints()?
                .into_iter()
                .map(|v| v as i16)
                .collect(),
            None => (0..anim.tracks as i16).collect(),
        },
        None => (0..anim.tracks as i16).collect(),
    };
    Ok(AnimFile { anim, root, track_to_bone })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DSR_DIR`: every player animation in `c0000_a0x` reads and decodes; rotations are unit and
    /// no bone jumps more than 60° between frames.
    #[test]
    fn install_player_animations_decode() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let bnd = std::fs::read(format!("{dir}/chr/c0000_a0x.anibnd.dcx")).unwrap();
        let entries = crate::bnd3::read(&crate::dcx::decompress(&bnd).unwrap()).unwrap();
        let mut decoded = 0;
        for e in entries.iter().filter(|e| e.file_name().ends_with(".hkx")) {
            let tf = Tagfile::parse(e.data.clone()).unwrap_or_else(|err| panic!("{}: {err:#}", e.name));
            let f = read(&tf).unwrap_or_else(|err| panic!("{}: {err:#}", e.name));
            let frames = super::super::spline::decode(&f.anim)
                .unwrap_or_else(|err| panic!("{}: {err:#}", e.name));
            assert_eq!(frames.len(), f.anim.frames, "{}", e.name);
            for (fi, frame) in frames.iter().enumerate() {
                for (ti, q) in frame.iter().enumerate() {
                    let n = q.r.iter().map(|v| v * v).sum::<f32>().sqrt();
                    assert!((n - 1.0).abs() < 1.0e-3, "{} f{fi} t{ti}: {q:?}", e.name);
                    if fi > 0 {
                        let p = frames[fi - 1][ti].r;
                        let dot = p.iter().zip(&q.r).map(|(a, b)| a * b).sum::<f32>().abs().min(1.0);
                        let deg = 2.0 * dot.acos().to_degrees();
                        assert!(deg <= 60.0, "{} f{fi} t{ti} jumps {deg:.1}°", e.name);
                    }
                }
            }
            decoded += 1;
        }
        assert!(decoded > 100, "only {decoded} animations");
    }
}
```

- [ ] **Step 2: Run** — `DSR_DIR='C:/Spel/Dark Souls Remastered' cargo test -j 4 -p dsr-formats -- --nocapture`
Expected: all unit tests and all three install tests PASS. If a quantization type other than 0/1/2/5 appears, the error names it: implement it from `SplineCompressedAnimation.cs` and add a unit test for it before continuing.
- [ ] **Step 3: Gates checkpoint.**

---

### Task 9: `dsr-cache` scaffold, DS loading and clip decoding

**Files:**
- Create: `crates/dsr-cache/Cargo.toml`
- Create: `crates/dsr-cache/src/main.rs`
- Create: `crates/dsr-cache/src/dsr.rs`

- [ ] **Step 1: Manifest**

```toml
[package]
name = "dsr-cache"
version.workspace = true
edition.workspace = true
# Links dsr-formats (GPL-3.0-or-later): see docs/PROVENANCE.md.
license = "GPL-3.0-or-later"
description = "wow-souls-dsr: reads Dark Souls Remastered data, retargets it to WoW skeletons, writes cache/dsr"

[lints]
workspace = true

[[bin]]
name = "wow-souls-dsr"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
glam.workspace = true
dsr-formats.workspace = true
wscl.workspace = true
benilla-formats.workspace = true
benilla-m2.workspace = true
```

- [ ] **Step 2: `src/dsr.rs`**

```rust
//! The player's Dark Souls data: its skeleton, its TAE files and its animation files, read from
//! `chr/c0000.anibnd.dcx` and every `chr/c0000_a*.anibnd.dcx` (`a00_lo/md/hi` hold the common
//! `a00_*` set with the rolls; `a0x`-`a9x` the weapon sets); one animation decoded to
//! full-skeleton frames.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use dsr_formats::hkx::skeleton::{Qs, Skeleton};
use dsr_formats::hkx::tagfile::Tagfile;
use dsr_formats::{bnd3, dcx, hkx, tae};

pub struct Player {
    pub skeleton: Skeleton,
    /// TAE stem (`a00`) → its animations.
    pub taes: BTreeMap<String, Vec<tae::Anim>>,
    /// Animation file stem (`a00_6000`) → its tagfile bytes.
    pub hkx: BTreeMap<String, Vec<u8>>,
}

fn archive(path: &Path) -> Result<Vec<bnd3::Entry>> {
    let raw = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    bnd3::read(&dcx::decompress(&raw)?).with_context(|| format!("unpacking {}", path.display()))
}

/// Every player animation archive, sorted: `c0000_a00_lo/md/hi` (the common `a00_*` set:
/// locomotion, rolls, backstep, damage, death) and `c0000_a0x` … `c0000_a9x` (weapon sets).
pub fn animation_archives(dsr: &Path) -> Result<Vec<std::path::PathBuf>> {
    let chr = dsr.join("chr");
    let mut out: Vec<_> = std::fs::read_dir(&chr)
        .with_context(|| format!("listing {}", chr.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("c0000_a") && n.ends_with(".anibnd.dcx"))
        })
        .collect();
    out.sort();
    if out.is_empty() {
        bail!("no c0000_a*.anibnd.dcx in {}", chr.display());
    }
    Ok(out)
}

pub fn load(dsr: &Path) -> Result<Player> {
    let chr = dsr.join("chr");
    let mut skeleton = None;
    let mut taes = BTreeMap::new();
    for e in archive(&chr.join("c0000.anibnd.dcx"))? {
        if e.file_name() == "Skeleton.hkx" {
            skeleton = Some(hkx::skeleton::read(&Tagfile::parse(e.data)?)?);
        } else if e.file_name().ends_with(".tae") {
            let anims = tae::read(&e.data).with_context(|| format!("TAE {}", e.name))?;
            taes.insert(e.stem().to_string(), anims);
        }
    }
    let mut hkx = BTreeMap::new();
    for path in animation_archives(dsr)? {
        for e in archive(&path)? {
            if e.file_name().ends_with(".hkx") {
                hkx.insert(e.stem().to_string(), e.data);
            }
        }
    }
    Ok(Player {
        skeleton: skeleton.context("c0000.anibnd has no Skeleton.hkx")?,
        taes,
        hkx,
    })
}

impl Player {
    /// The TAE events of animation file `a00_6000` (TAE `a00`, animation id 6000).
    pub fn events(&self, stem: &str) -> &[tae::Event] {
        let Some((tae, id)) = stem.split_once('_') else {
            return &[];
        };
        let (Some(anims), Ok(id)) = (self.taes.get(tae), id.parse::<u32>()) else {
            return &[];
        };
        anims
            .iter()
            .find(|a| a.id == id)
            .map_or(&[][..], |a| a.events.as_slice())
    }

    /// The `ChrActionFlag` windows of an animation.
    pub fn flags(&self, stem: &str) -> Vec<wscl::FlagWindow> {
        self.events(stem)
            .iter()
            .filter_map(|e| {
                Some(wscl::FlagWindow {
                    flag: e.action_flag()?,
                    start: e.start,
                    end: e.end,
                })
            })
            .collect()
    }
}

/// One DS animation over the whole skeleton (unanimated bones at the reference pose).
pub struct DsClip {
    pub name: String,
    pub fps: f32,
    /// `frames[f][bone]`, bone-local, DS space.
    pub frames: Vec<Vec<Qs>>,
    /// Per frame, accumulated `(x, y, z, yaw)`, DS space and units.
    pub root: Vec<[f32; 4]>,
    pub up: [f32; 3],
    pub forward: [f32; 3],
}

impl DsClip {
    pub fn duration(&self) -> f32 {
        self.frames.len().saturating_sub(1) as f32 / self.fps
    }
}

/// The root sample at `t` seconds, linear between samples.
fn root_at(rm: &hkx::animation::RootMotion, t: f32) -> [f32; 4] {
    let n = rm.samples.len();
    if n == 0 {
        return [0.0; 4];
    }
    if n == 1 || rm.duration <= 0.0 {
        return rm.samples[0];
    }
    let s = (t / rm.duration).clamp(0.0, 1.0) * (n - 1) as f32;
    let i = (s as usize).min(n - 2);
    let a = s - i as f32;
    let (p, q) = (rm.samples[i], rm.samples[i + 1]);
    [0, 1, 2, 3].map(|k| p[k] + (q[k] - p[k]) * a)
}

pub fn decode(player: &Player, stem: &str) -> Result<DsClip> {
    let bytes = player
        .hkx
        .get(stem)
        .with_context(|| format!("no animation file {stem}.hkx"))?;
    let file = hkx::animation::read(&Tagfile::parse(bytes.clone())?)?;
    let tracks = hkx::spline::decode(&file.anim)?;
    let bones = player.skeleton.parents.len();
    if file.anim.frame_duration <= 0.0 {
        bail!("{stem}: frame duration {}", file.anim.frame_duration);
    }
    let fps = 1.0 / file.anim.frame_duration;
    let frames = tracks
        .iter()
        .map(|tr| {
            let mut f = player.skeleton.reference.clone();
            for (t, &b) in file.track_to_bone.iter().enumerate() {
                if let (Ok(b), Some(q)) = (usize::try_from(b), tr.get(t)) {
                    if b < bones {
                        f[b] = *q;
                    }
                }
            }
            f
        })
        .collect::<Vec<_>>();
    let (root, up, forward) = match &file.root {
        Some(rm) => (
            (0..frames.len()).map(|i| root_at(rm, i as f32 / fps)).collect(),
            rm.up,
            rm.forward,
        ),
        None => (vec![[0.0; 4]; frames.len()], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
    };
    Ok(DsClip {
        name: stem.to_string(),
        fps,
        frames,
        root,
        up,
        forward,
    })
}
```

- [ ] **Step 3: `src/main.rs` (CLI skeleton; commands are filled in by Tasks 10-13)**

```rust
//! `wow-souls-dsr`: reads the Dark Souls Remastered install (read-only), retargets the player's
//! roll onto the WoW human, writes `cache/dsr/`.
//!
//! ```text
//! wow-souls-dsr list    [--dsr <dir>]
//! wow-souls-dsr extract [--dsr <dir>] [--wow <Data dir>] [--out <dir>] [--roll <a00_NNNN>]
//! ```

mod basis;
mod bonemap;
mod cache;
mod dsr;
mod retarget;
mod roll;
mod wow;

use std::path::PathBuf;

use anyhow::{bail, Result};

const DEFAULT_DSR: &str = "C:/Spel/Dark Souls Remastered";
const DEFAULT_WOW: &str = "C:/Spel/World of Warcraft 1.12/Data";
const DEFAULT_OUT: &str = "cache/dsr";

struct Args {
    command: String,
    dsr: PathBuf,
    wow: PathBuf,
    out: PathBuf,
    roll: Option<String>,
}

fn args() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let command = it.next().unwrap_or_default();
    let mut a = Args {
        command,
        dsr: DEFAULT_DSR.into(),
        wow: DEFAULT_WOW.into(),
        out: DEFAULT_OUT.into(),
        roll: None,
    };
    while let Some(flag) = it.next() {
        let Some(value) = it.next() else {
            bail!("{flag} needs a value");
        };
        match flag.as_str() {
            "--dsr" => a.dsr = value.into(),
            "--wow" => a.wow = value.into(),
            "--out" => a.out = value.into(),
            "--roll" => a.roll = Some(value),
            other => bail!("unknown option {other}"),
        }
    }
    Ok(a)
}

fn main() -> Result<()> {
    let a = args()?;
    match a.command.as_str() {
        "list" => roll::list(&dsr::load(&a.dsr)?),
        "extract" => cache::extract(&a.dsr, &a.wow, &a.out, a.roll.as_deref()),
        _ => bail!("usage: wow-souls-dsr list|extract [--dsr DIR] [--wow DATA_DIR] [--out DIR] [--roll a00_NNNN]"),
    }
}
```

(Create empty `basis.rs`, `bonemap.rs`, `cache.rs`, `retarget.rs`, `roll.rs`, `wow.rs` with the functions referenced by `main` stubbed as `pub fn list(_: &crate::dsr::Player) -> anyhow::Result<()> { anyhow::bail!("not implemented") }` and `pub fn extract(_: &std::path::Path, _: &std::path::Path, _: &std::path::Path, _: Option<&str>) -> anyhow::Result<()> { anyhow::bail!("not implemented") }` so it builds.)

- [ ] **Step 4: Build** — `cargo build -j 4 -p dsr-cache` → `Finished`.
- [ ] **Step 5: Gates checkpoint** — fmt + `cargo clippy -j 4 -p dsr-cache --all-targets -- -D warnings`. The stub modules carry `#![allow(dead_code)]` at their top until Tasks 10-13 replace them; Task 13 removes the last ones.

---

### Task 10: WoW HumanMale skeleton, basis and bone map

**Files:**
- Modify: `crates/dsr-cache/src/wow.rs`, `basis.rs`, `bonemap.rs`

- [ ] **Step 1: `wow.rs`**

```rust
//! The WoW HumanMale skeleton (parents, pivots in Bevy axes) from the user's WoW install.

use std::io::Cursor;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use glam::Vec3;

pub const HUMAN_MALE_M2: &str = "Character\\Human\\Male\\HumanMale.m2";

#[derive(Debug, Clone)]
pub struct WowSkel {
    pub parents: Vec<i16>,
    /// Bone pivots, model space, Bevy axes.
    pub pivots: Vec<Vec3>,
}

/// WoW (x north, y west, z up) → Bevy (x east, y up, -z north).
pub fn wow_to_bevy(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(-y, z, -x)
}

pub fn human_male(data_dir: &Path) -> Result<WowSkel> {
    let mut chain = benilla_formats::Chain::open(data_dir)
        .with_context(|| format!("opening WoW data {}", data_dir.display()))?;
    let bytes = chain.read_file(HUMAN_MALE_M2)?;
    let fmt = benilla_m2::parse_m2(&mut Cursor::new(&bytes[..]))
        .map_err(|e| anyhow!("parsing {HUMAN_MALE_M2}: {e:?}"))?;
    let bones = &fmt.model().bones;
    Ok(WowSkel {
        parents: bones.iter().map(|b| b.parent).collect(),
        pivots: bones
            .iter()
            .map(|b| wow_to_bevy(b.pivot.x, b.pivot.y, b.pivot.z))
            .collect(),
    })
}
```

- [ ] **Step 2: `basis.rs` with tests**

```rust
//! DS model space → Bevy model space at WoW scale: a change of basis (possibly a reflection)
//! built from the animation's up/forward and the skeleton's left side, and a uniform scale.

use glam::{Mat3, Quat, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Basis {
    pub m: Mat3,
    pub scale: f32,
}

impl Basis {
    /// `up`, `forward` from the reference frame; `left` points from the right shoulder to the
    /// left one; all in DS space.
    pub fn new(up: Vec3, forward: Vec3, left: Vec3, scale: f32) -> Basis {
        let up = up.normalize();
        let forward = (forward - up * forward.dot(up)).normalize();
        let right = (-left - up * (-left).dot(up) - forward * (-left).dot(forward)).normalize();
        let ds = Mat3::from_cols(right, up, forward);
        let bevy = Mat3::from_cols(Vec3::X, Vec3::Y, Vec3::NEG_Z);
        Basis {
            m: bevy * ds.transpose(),
            scale,
        }
    }

    /// A DS position, in Bevy axes at WoW scale.
    pub fn pos(&self, v: Vec3) -> Vec3 {
        self.m * v * self.scale
    }

    /// A DS direction, in Bevy axes.
    pub fn dir(&self, v: Vec3) -> Vec3 {
        self.m * v
    }

    /// A DS rotation, as the same rotation seen in Bevy axes (proper even under reflection).
    pub fn rot(&self, q: Quat) -> Quat {
        Quat::from_mat3(&(self.m * Mat3::from_quat(q) * self.m.transpose())).normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bevy_axes_map_to_themselves() {
        let b = Basis::new(Vec3::Y, Vec3::NEG_Z, Vec3::NEG_X, 1.0);
        assert!((b.m - Mat3::IDENTITY).abs_diff_eq(Mat3::ZERO, 1.0e-6));
    }

    #[test]
    fn a_mirrored_space_keeps_rotations_proper_and_consistent() {
        // DS-like: forward +Z, left +X (a mirror of Bevy's handedness).
        let b = Basis::new(Vec3::Y, Vec3::Z, Vec3::X, 2.0);
        assert!((b.dir(Vec3::Z) - Vec3::NEG_Z).length() < 1.0e-6);
        assert!((b.dir(Vec3::X) - Vec3::NEG_X).length() < 1.0e-6, "left stays left");
        let q = Quat::from_rotation_y(0.7) * Quat::from_rotation_x(0.3);
        let v = Vec3::new(0.2, 0.5, 0.9);
        let lhs = b.dir(q * v);
        let rhs = b.rot(q) * b.dir(v);
        assert!((lhs - rhs).length() < 1.0e-5, "{lhs} vs {rhs}");
        assert!((b.pos(Vec3::Y) - Vec3::Y * 2.0).length() < 1.0e-6);
    }
}
```

- [ ] **Step 3: `bonemap.rs`**

```rust
//! The hand-made DS c0000 ↔ WoW HumanMale bone table. Each entry pairs a bone on both
//! skeletons and an aim bone on each that defines the bone's direction for the bind alignment.
//! WoW indices from `benilla-extract m2bones Character\Human\Male\HumanMale.m2` (2026-10-07).

pub struct Pair {
    pub ds: &'static str,
    pub ds_aim: &'static str,
    pub wow: usize,
    pub wow_aim: usize,
    /// The aim is a child moved by this bone (not its parent): the direction error is checked.
    pub check: bool,
}

const fn p(ds: &'static str, ds_aim: &'static str, wow: usize, wow_aim: usize, check: bool) -> Pair {
    Pair { ds, ds_aim, wow, wow_aim, check }
}

pub const HUMAN_MALE: &[Pair] = &[
    p("Pelvis", "L_Thigh", 21, 25, true),
    p("Spine", "Spine1", 20, 22, true),
    p("Spine1", "Neck", 22, 48, true),
    p("Neck", "Head", 48, 64, true),
    p("Head", "Neck", 64, 48, false),
    p("L_Clavicle", "L_UpperArm", 49, 65, true),
    p("L_UpperArm", "L_Forearm", 65, 78, true),
    p("L_Forearm", "L_Hand", 78, 95, true),
    p("L_Hand", "L_Finger1", 95, 105, true),
    p("R_Clavicle", "R_UpperArm", 50, 66, true),
    p("R_UpperArm", "R_Forearm", 66, 79, true),
    p("R_Forearm", "R_Hand", 79, 97, true),
    p("R_Hand", "R_Finger1", 97, 104, true),
    p("L_Thigh", "L_Calf", 25, 53, true),
    p("L_Calf", "L_Foot", 53, 68, true),
    p("L_Foot", "L_Toe0", 68, 80, true),
    p("R_Thigh", "R_Calf", 26, 54, true),
    p("R_Calf", "R_Foot", 54, 70, true),
    p("R_Foot", "R_Toe0", 70, 81, true),
];

/// The DS bone whose height scales DS to WoW and whose translation moves the hips.
pub const DS_PELVIS: &str = "Pelvis";

/// A WoW skeleton's hips: the pelvis counterpart (its height sets the scale) and the bone that
/// carries the DS pelvis' translation (the hips' common parent).
#[derive(Debug, Clone, Copy)]
pub struct Hips {
    pub wow_pelvis: usize,
    pub wow_hip_root: usize,
}

pub const HUMAN_MALE_HIPS: Hips = Hips { wow_pelvis: 21, wow_hip_root: 18 };
/// Shoulders that define the DS left side.
pub const DS_LEFT: &str = "L_UpperArm";
pub const DS_RIGHT: &str = "R_UpperArm";
```

- [ ] **Step 4: Run** — `cargo test -j 4 -p dsr-cache basis` → `2 passed`.
- [ ] **Step 5: Gates checkpoint.**

---

### Task 11: Retarget

**Files:**
- Modify: `crates/dsr-cache/src/retarget.rs`

- [ ] **Step 1: Write the module with a synthetic test first (stub `retarget` returning an error)**

```rust
//! Bind-aligned delta transfer (spec "Retargeting method"): for each mapped bone, the DS
//! rotation's delta from its bind, applied after a bind alignment of the two bones' directions.
//! Produces the WoW clip (bone-local), the DS clip (model space, Bevy axes, WoW scale) for the
//! side-by-side, and the worst direction error over checked bones and frames.

use anyhow::{bail, Context, Result};
use glam::{EulerRot, Quat, Vec3};

use crate::basis::Basis;
use crate::bonemap::{self, Hips, Pair};
use crate::dsr::DsClip;
use crate::wow::WowSkel;
use dsr_formats::hkx::skeleton::{Qs, Skeleton};

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

fn q4(a: [f32; 4]) -> Quat {
    Quat::from_xyzw(a[0], a[1], a[2], a[3]).normalize()
}

/// Model-space positions and rotations of a parent-sorted skeleton.
pub fn model_space(parents: &[i16], locals: &[Qs]) -> Result<Vec<(Vec3, Quat)>> {
    let mut out: Vec<(Vec3, Quat)> = Vec::with_capacity(locals.len());
    for (i, l) in locals.iter().enumerate() {
        let (t, r) = (v3(l.t), q4(l.r));
        out.push(match usize::try_from(parents[i]) {
            Ok(p) if p < i => (out[p].0 + out[p].1 * t, out[p].1 * r),
            Ok(p) => bail!("bone {i}'s parent {p} is not before it"),
            Err(_) => (t, r),
        });
    }
    Ok(out)
}

struct Mapped {
    ds: usize,
    ds_aim: usize,
    wow: usize,
    wow_aim: usize,
    check: bool,
}

fn resolve(map: &[Pair], ds: &Skeleton, wow: &WowSkel) -> Result<Vec<Mapped>> {
    map.iter()
        .map(|p| {
            let ds_i = ds.bone(p.ds).with_context(|| format!("DS bone {} missing", p.ds))?;
            let ds_aim = ds.bone(p.ds_aim).with_context(|| format!("DS bone {} missing", p.ds_aim))?;
            if p.wow >= wow.parents.len() || p.wow_aim >= wow.parents.len() {
                bail!("WoW bone {} or {} past the skeleton", p.wow, p.wow_aim);
            }
            Ok(Mapped { ds: ds_i, ds_aim, wow: p.wow, wow_aim: p.wow_aim, check: p.check })
        })
        .collect()
}

pub struct Retargeted {
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    /// The worst angle (deg) between a checked WoW bone's direction and its DS counterpart's.
    pub max_error_deg: f32,
}

/// The basis for `clip` on `ds`: DS up/forward from the root motion, left from the shoulders,
/// scale from the pelvis heights.
pub fn basis_for(ds: &Skeleton, clip: &DsClip, wow: &WowSkel, hips: Hips) -> Result<Basis> {
    let bind = model_space(&ds.parents, &ds.reference)?;
    let bone = |n: &str| ds.bone(n).with_context(|| format!("DS bone {n} missing"));
    let left = bind[bone(bonemap::DS_LEFT)?].0 - bind[bone(bonemap::DS_RIGHT)?].0;
    let unit = Basis::new(v3(clip.up), v3(clip.forward), left, 1.0);
    let ds_h = unit.pos(bind[bone(bonemap::DS_PELVIS)?].0).y;
    let wow_h = wow
        .pivots
        .get(hips.wow_pelvis)
        .context("WoW pelvis past the skeleton")?
        .y;
    if ds_h <= 0.0 {
        bail!("DS pelvis height {ds_h} is not above the origin");
    }
    Ok(Basis { scale: wow_h / ds_h, ..unit })
}

pub fn retarget(ds: &Skeleton, clip: &DsClip, wow: &WowSkel, map: &[Pair], hips: Hips, flags: Vec<wscl::FlagWindow>) -> Result<Retargeted> {
    let _ = (ds, clip, wow, map, hips, flags, resolve, EulerRot::YXZ);
    bail!("not implemented")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DS: an upper arm along -X (T-pose) that turns 90° about Z to point down by frame 1.
    /// WoW: an upper arm bound 45° down-left (A-pose). The WoW bone must follow the DS bone's
    /// direction in every frame.
    #[test]
    fn a_mapped_bone_follows_its_ds_direction_despite_a_different_bind() {
        let ds = Skeleton {
            names: vec!["Pelvis".into(), "L_UpperArm".into(), "L_Forearm".into(), "R_UpperArm".into()],
            parents: vec![-1, 0, 1, 0],
            reference: vec![
                Qs { t: [0.0, 1.0, 0.0], ..Qs::IDENTITY },
                Qs { t: [-0.2, 0.5, 0.0], ..Qs::IDENTITY },
                Qs { t: [-0.3, 0.0, 0.0], ..Qs::IDENTITY },
                Qs { t: [0.2, 0.5, 0.0], ..Qs::IDENTITY },
            ],
        };
        let down = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let mut f1 = ds.reference.clone();
        f1[1].r = down.to_array();
        let clip = DsClip {
            name: "test".into(),
            fps: 30.0,
            frames: vec![ds.reference.clone(), f1],
            root: vec![[0.0; 4]; 2],
            up: [0.0, 1.0, 0.0],
            forward: [0.0, 0.0, -1.0],
        };
        // WoW: 0 root, 1 hips (pelvis), 2 shoulder → 3 elbow pointing 45° down-left.
        let wow = WowSkel {
            parents: vec![-1, 0, 1, 2],
            pivots: vec![
                Vec3::ZERO,
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(-0.2, 1.5, 0.0),
                Vec3::new(-0.4, 1.3, 0.0),
            ],
        };
        let map = [
            Pair { ds: "Pelvis", ds_aim: "L_UpperArm", wow: 1, wow_aim: 2, check: false },
            Pair { ds: "L_UpperArm", ds_aim: "L_Forearm", wow: 2, wow_aim: 3, check: true },
        ];
        let hips = Hips { wow_pelvis: 1, wow_hip_root: 1 };
        let r = retarget(&ds, &clip, &wow, &map, hips, Vec::new()).unwrap();
        assert!(r.max_error_deg < 1.0, "error {}", r.max_error_deg);
        assert_eq!(r.wow.frames.len(), 2);
        assert_eq!(r.wow.parents, wow.parents);
        assert!(r.wow.driven[2] && r.wow.driven[0], "mapped bones and their ancestors are driven");
        assert!(!r.wow.driven[3], "an unmapped leaf is left alone");
        assert!(r.ds.model_space);
    }

    #[test]
    fn model_space_composes_parents_first() {
        let locals = [
            Qs { t: [0.0, 1.0, 0.0], r: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array(), s: [1.0; 3] },
            Qs { t: [0.0, 0.0, -1.0], ..Qs::IDENTITY },
        ];
        let ms = model_space(&[-1, 0], &locals).unwrap();
        assert!((ms[1].0 - Vec3::new(-1.0, 1.0, 0.0)).length() < 1.0e-5, "{}", ms[1].0);
        assert!(model_space(&[1, -1], &locals).is_err());
    }
}
```

Note for the test's `basis_for`: with up +Y, forward -Z and left = L - R = (-0.4, 0, 0) the basis is the identity, and the scale is `wow pelvis y / ds pelvis y` = 1.0 / 1.0.

- [ ] **Step 2: Run to verify failure** — `cargo test -j 4 -p dsr-cache retarget` → `model_space…` PASS, `a_mapped_bone…` FAIL `not implemented`.

- [ ] **Step 3: Implement `retarget`**

```rust
pub fn retarget(
    ds: &Skeleton,
    clip: &DsClip,
    wow: &WowSkel,
    map: &[Pair],
    hips: Hips,
    flags: Vec<wscl::FlagWindow>,
) -> Result<Retargeted> {
    let pairs = resolve(map, ds, wow)?;
    let basis = basis_for(ds, clip, wow, hips)?;
    let bind = model_space(&ds.parents, &ds.reference)?;
    let n = wow.parents.len();
    let parent = |i: usize| usize::try_from(wow.parents[i]).ok().filter(|&p| p < i);
    // Bind alignment and DS bind rotation per pair.
    let align: Vec<(Quat, Quat)> = pairs
        .iter()
        .map(|m| {
            let dir_w = (wow.pivots[m.wow_aim] - wow.pivots[m.wow]).normalize();
            let dir_d = basis.dir(bind[m.ds_aim].0 - bind[m.ds].0).normalize();
            (Quat::from_rotation_arc(dir_w, dir_d), basis.rot(bind[m.ds].1))
        })
        .collect();
    let mut driven = vec![false; n];
    for m in &pairs {
        let mut i = Some(m.wow);
        while let Some(b) = i {
            driven[b] = true;
            i = parent(b);
        }
    }
    let ds_pelvis = ds.bone(bonemap::DS_PELVIS).context("DS pelvis missing")?;
    let bind_local_t = |i: usize| match parent(i) {
        Some(p) => wow.pivots[i] - wow.pivots[p],
        None => wow.pivots[i],
    };
    let mut wow_frames = Vec::with_capacity(clip.frames.len());
    let mut ds_frames = Vec::with_capacity(clip.frames.len());
    let mut max_error = 0.0f32;
    for locals in &clip.frames {
        let ms = model_space(&ds.parents, locals)?;
        let mut world = vec![Quat::IDENTITY; n];
        let mut pos = vec![Vec3::ZERO; n];
        let mut out = Vec::with_capacity(n);
        let hip = basis.pos(ms[ds_pelvis].0 - bind[ds_pelvis].0);
        for i in 0..n {
            let parent_world = parent(i).map_or(Quat::IDENTITY, |p| world[p]);
            world[i] = match pairs.iter().position(|m| m.wow == i) {
                Some(k) => {
                    let delta = basis.rot(ms[pairs[k].ds].1) * align[k].1.inverse();
                    (delta * align[k].0).normalize()
                }
                None => parent_world,
            };
            let mut t = bind_local_t(i);
            if i == hips.wow_hip_root {
                t += hip;
            }
            pos[i] = match parent(i) {
                Some(p) => pos[p] + world[p] * t,
                None => t,
            };
            let local = (parent_world.inverse() * world[i]).normalize();
            out.push(wscl::Trs { t: t.to_array(), r: local.to_array(), s: [1.0; 3] });
        }
        for m in pairs.iter().filter(|m| m.check) {
            let dw = (pos[m.wow_aim] - pos[m.wow]).normalize();
            let dd = basis.dir(ms[m.ds_aim].0 - ms[m.ds].0).normalize();
            max_error = max_error.max(dw.dot(dd).clamp(-1.0, 1.0).acos().to_degrees());
        }
        wow_frames.push(out);
        ds_frames.push(
            ms.iter()
                .map(|(p, r)| wscl::Trs { t: basis.pos(*p).to_array(), r: basis.rot(*r).to_array(), s: [1.0; 3] })
                .collect::<Vec<_>>(),
        );
    }
    let up = v3(clip.up);
    let root_motion: Vec<[f32; 4]> = clip
        .root
        .iter()
        .map(|r| {
            let p = basis.pos(Vec3::new(r[0], r[1], r[2]));
            let yaw = basis.rot(Quat::from_axis_angle(up.normalize(), r[3])).to_euler(EulerRot::YXZ).0;
            [p.x, p.y, p.z, yaw]
        })
        .collect();
    let wow_clip = wscl::Clip {
        name: clip.name.clone(),
        fps: clip.fps,
        model_space: false,
        parents: wow.parents.clone(),
        driven,
        frames: wow_frames,
        root_motion: root_motion.clone(),
        flags: flags.clone(),
    };
    let ds_clip = wscl::Clip {
        name: clip.name.clone(),
        fps: clip.fps,
        model_space: true,
        parents: ds.parents.clone(),
        driven: vec![true; ds.parents.len()],
        frames: ds_frames,
        root_motion,
        flags,
    };
    Ok(Retargeted { wow: wow_clip, ds: ds_clip, max_error_deg: max_error })
}
```

- [ ] **Step 4: Run** — `cargo test -j 4 -p dsr-cache retarget` → `2 passed`.
- [ ] **Step 5: Gates checkpoint.**

---

### Task 12: Roll candidates and the `list` command

**Files:**
- Modify: `crates/dsr-cache/src/roll.rs`

- [ ] **Step 1: Write with tests**

```rust
//! Choosing the medium forward roll from data: forward root motion, under 1.6 s, an
//! invulnerability window. The medium roll's window is 11 frames at 30 fps; `docs/dodge.md`
//! gives the fast roll the same 11 frames and a shorter length (0.8 s against 1.1 s), and the
//! fat roll 9 frames: so the closest window (to the frame) wins, and among equals the longer
//! roll.

use anyhow::Result;
use dsr_formats::hkx::tagfile::Tagfile;
use glam::Vec3;

use crate::dsr::Player;

pub const MEDIUM_ROLL_IFRAMES: f32 = 11.0 / 30.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub name: String,
    pub duration: f32,
    /// Root motion along the forward axis and across it (DS units, metres).
    pub forward: f32,
    pub side: f32,
    /// Total invulnerable time (s).
    pub iframes: f32,
}

pub fn pick(c: &[Candidate]) -> Option<&Candidate> {
    // Distance from the medium roll's window, in whole 30 fps frames.
    let off = |c: &Candidate| ((c.iframes - MEDIUM_ROLL_IFRAMES).abs() * 30.0).round() as i32;
    c.iter()
        .filter(|c| c.iframes > 0.0 && c.duration < 1.6 && c.forward >= 2.5 && c.side.abs() < 0.15 * c.forward)
        .min_by(|a, b| off(a).cmp(&off(b)).then(b.duration.total_cmp(&a.duration)))
}

/// Every animation with a motion file and an invulnerability window, measured.
pub fn candidates(player: &Player) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (name, bytes) in &player.hkx {
        let iframes: f32 = player
            .flags(name)
            .iter()
            .filter(|w| w.flag == wscl::FLAG_INVINCIBLE)
            .map(|w| (w.end - w.start).max(0.0))
            .sum();
        if iframes <= 0.0 {
            continue;
        }
        let Ok(tf) = Tagfile::parse(bytes.clone()) else { continue };
        let Ok(file) = dsr_formats::hkx::animation::read(&tf) else { continue };
        let Some(rm) = file.root else { continue };
        let (Some(first), Some(last)) = (rm.samples.first(), rm.samples.last()) else { continue };
        let d = Vec3::new(last[0] - first[0], last[1] - first[1], last[2] - first[2]);
        let up = Vec3::from_array(rm.up).normalize();
        let fwd = (Vec3::from_array(rm.forward) - up * Vec3::from_array(rm.forward).dot(up)).normalize();
        let flat = d - up * d.dot(up);
        out.push(Candidate {
            name: name.clone(),
            duration: file.anim.duration,
            forward: flat.dot(fwd),
            side: (flat - fwd * flat.dot(fwd)).length(),
            iframes,
        });
    }
    out
}

pub fn list(player: &Player) -> Result<()> {
    let c = candidates(player);
    println!("{:<14} {:>7} {:>8} {:>6} {:>8}", "animation", "dur s", "fwd m", "side m", "inv s");
    for x in &c {
        println!("{:<14} {:>7.3} {:>8.2} {:>6.2} {:>8.3}", x.name, x.duration, x.forward, x.side, x.iframes);
    }
    match pick(&c) {
        Some(r) => println!("medium forward roll: {}", r.name),
        None => println!("no roll candidate"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(name: &str, duration: f32, forward: f32, side: f32, iframes: f32) -> Candidate {
        Candidate { name: name.into(), duration, forward, side, iframes }
    }

    #[test]
    fn the_medium_roll_is_the_forward_one_with_eleven_frames() {
        // docs/dodge.md: fast and medium share 11 i-frames, the medium is longer.
        let all = [
            c("fast", 0.8, 4.2, 0.0, 11.0 / 30.0),
            c("medium", 1.1, 3.8, 0.1, 11.0 / 30.0),
            c("fat", 1.53, 3.0, 0.0, 9.0 / 30.0),
            c("backstep", 0.6, -1.65, 0.0, 0.1),
            c("side", 1.1, 0.2, 3.7, 11.0 / 30.0),
        ];
        assert_eq!(pick(&all).unwrap().name, "medium");
        assert_eq!(pick(&[]), None);
    }
}
```

- [ ] **Step 2: Run unit test** — `cargo test -j 4 -p dsr-cache roll` → `1 passed`.
- [ ] **Step 3: Run `list` on the install**

Run: `cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- list`
Expected: a table of animations with invulnerability (rolls, backsteps), ending with `medium forward roll: aNN_NNNN`. Record the chosen name in the plan's notes below. If none qualifies, inspect the table: adjust the thresholds in `pick` only with a test case built from the real numbers.

Notes (fill in when run): chosen roll = `________`, duration ____ s, forward ____ m, invulnerable ____ s.

- [ ] **Step 4: Gates checkpoint.**

---

### Task 13: Cache writer and `extract`

**Files:**
- Modify: `crates/dsr-cache/src/cache.rs`

- [ ] **Step 1: Write**

```rust
//! `extract`: decode the roll, retarget it, write `cache/dsr/`:
//! `human_male/<roll>.wscl` (WoW bones, local), `c0000/<roll>.wscl` (DS bones, model space,
//! Bevy axes, WoW scale) and `manifest.txt`.

use std::fmt::Write as _;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{bail, Context, Result};

use crate::{bonemap, dsr, retarget, roll, wow};

pub const MANIFEST_VERSION: u32 = 1;
/// The no-tearing criterion (spec, success criterion 2).
pub const MAX_ERROR_DEG: f32 = 15.0;

fn source_line(path: &Path) -> Result<String> {
    let meta = std::fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    let mtime = meta.modified()?.duration_since(UNIX_EPOCH)?.as_secs();
    Ok(format!("source {} {} {}", meta.len(), mtime, path.display()))
}

fn write_clip(path: &Path, clip: &wscl::Clip) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    clip.write(&mut f)?;
    Ok(())
}

pub fn extract(dsr_dir: &Path, wow_dir: &Path, out: &Path, roll_name: Option<&str>) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    let name = match roll_name {
        Some(n) => n.to_string(),
        None => roll::pick(&roll::candidates(&player))
            .context("no roll candidate; run `list` and pass --roll")?
            .name
            .clone(),
    };
    let clip = dsr::decode(&player, &name)?;
    let skel = wow::human_male(wow_dir)?;
    let r = retarget::retarget(
        &player.skeleton,
        &clip,
        &skel,
        bonemap::HUMAN_MALE,
        bonemap::HUMAN_MALE_HIPS,
        player.flags(&name),
    )?;
    if r.max_error_deg > MAX_ERROR_DEG {
        bail!("{name}: a mapped bone is {:.1}° off its DS direction (limit {MAX_ERROR_DEG})", r.max_error_deg);
    }
    write_clip(&out.join("human_male").join(format!("{name}.wscl")), &r.wow)?;
    write_clip(&out.join("c0000").join(format!("{name}.wscl")), &r.ds)?;
    let mut m = String::new();
    writeln!(m, "wscl-cache {MANIFEST_VERSION}")?;
    writeln!(m, "roll {name}")?;
    writeln!(m, "max-error-deg {:.3}", r.max_error_deg)?;
    for path in dsr::animation_archives(dsr_dir)? {
        writeln!(m, "{}", source_line(&path)?)?;
    }
    writeln!(m, "{}", source_line(&dsr_dir.join("chr").join("c0000.anibnd.dcx"))?)?;
    std::fs::write(out.join("manifest.txt"), m)?;
    let rm = r.wow.root_motion.last().copied().unwrap_or([0.0; 4]);
    println!(
        "{name}: {} frames at {:.1} fps ({:.3} s), root motion ({:.2}, {:.2}, {:.2}) yd, yaw {:.1}°, \
         invulnerable {:?}, max direction error {:.2}°",
        r.wow.frames.len(),
        r.wow.fps,
        r.wow.duration(),
        rm[0],
        rm[1],
        rm[2],
        rm[3].to_degrees(),
        r.wow.windows(wscl::FLAG_INVINCIBLE).collect::<Vec<_>>(),
        r.max_error_deg
    );
    Ok(())
}
```

Remove the `#![allow(dead_code)]` stubs added in Task 9.

- [ ] **Step 2: Run the extractor**

Run: `cd /c/Spel/wow-souls && cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- extract`
Expected: one line naming the roll, ~0.7-1.0 s, root motion mostly along -Z (forward) of a few yards, an invulnerable window, max direction error under 15°. Files exist: `cache/dsr/manifest.txt`, `cache/dsr/human_male/<roll>.wscl`, `cache/dsr/c0000/<roll>.wscl`. `git status` shows nothing under `cache/` (gitignored).

- [ ] **Step 3: Gates checkpoint** — fmt; clippy for `dsr-formats`, `dsr-cache`, `wscl`; `cargo test -j 4 -p wscl -p dsr-formats -p dsr-cache` green.

---

### Task 14: Game: load the cache and play the DS roll

**Files:**
- Create: `crates/wow-souls/src/dsclip.rs`
- Modify: `crates/wow-souls/Cargo.toml` (add `wscl.workspace = true`)
- Modify: `crates/wow-souls/src/main.rs` (`mod dsclip;`)
- Modify: `crates/wow-souls/src/hits.rs` (register `crate::dsclip::plugin` beside `hitbody`)
- Modify: `crates/wow-souls/src/combat.rs`

- [ ] **Step 1: `dsclip.rs` (pure sampling first, with tests)**

```rust
//! The Dark Souls roll from `cache/dsr/` (written by `wow-souls-dsr extract`): loaded at startup
//! when present and fresh, sampled per frame onto the avatar's rig in benilla's `PosePost`
//! window, and turned into the roll's `EvadeSpec` (duration, distance, invulnerability, cancel)
//! from its own data. Without a cache the procedural roll stays.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use benilla_world::rig_anim::{PosePost, RigPose};

use crate::avatar::Avatar;
use crate::combat::{Action, EvadeSpec, ROLL};

/// `ChrActionFlag` values that open a cancel window (attack, movement, step).
const CANCEL_FLAGS: [i32; 3] = [4, 11, 26];

/// One bone of a clip at time `t`: translation, rotation, scale (linear / slerp between frames).
pub fn sample(clip: &wscl::Clip, t: f32, bone: usize) -> (Vec3, Quat, Vec3) {
    let last = clip.frames.len().saturating_sub(1);
    let f = (t.max(0.0) * clip.fps).min(last as f32);
    let i = (f as usize).min(last.saturating_sub(1));
    let a = if last == 0 { 0.0 } else { f - i as f32 };
    let (p, q) = (&clip.frames[i][bone], &clip.frames[(i + 1).min(last)][bone]);
    let qa = Quat::from_array(p.r).normalize();
    let qb = Quat::from_array(q.r).normalize();
    (
        Vec3::from_array(p.t).lerp(Vec3::from_array(q.t), a),
        qa.slerp(qb, a),
        Vec3::from_array(p.s).lerp(Vec3::from_array(q.s), a),
    )
}

/// The root motion's forward distance (yd, along Bevy -Z of the model) covered by `t`.
pub fn progress(clip: &wscl::Clip, t: f32) -> f32 {
    let n = clip.root_motion.len();
    if n == 0 {
        return 0.0;
    }
    let f = (t.max(0.0) * clip.fps).min((n - 1) as f32);
    let i = (f as usize).min(n.saturating_sub(2));
    let a = if n == 1 { 0.0 } else { f - i as f32 };
    let z0 = clip.root_motion[0][2];
    let z = clip.root_motion[i][2] + (clip.root_motion[(i + 1).min(n - 1)][2] - clip.root_motion[i][2]) * a;
    -(z - z0)
}

/// The roll's evade shape from its clip; the procedural roll's cost.
pub fn roll_spec(clip: &wscl::Clip) -> EvadeSpec {
    let duration = clip.duration();
    let iframes = clip
        .windows(wscl::FLAG_INVINCIBLE)
        .next()
        .unwrap_or(ROLL.iframes);
    let cancel_from = clip
        .flags
        .iter()
        .filter(|w| CANCEL_FLAGS.contains(&w.flag))
        .map(|w| w.start)
        .fold(duration, f32::min);
    EvadeSpec {
        duration,
        travel: duration,
        distance: progress(clip, duration),
        iframes,
        cancel_from,
        cost: ROLL.cost,
    }
}

/// The loaded DS roll.
#[derive(Resource)]
pub struct DsRoll {
    /// On the WoW HumanMale skeleton, bone-local.
    pub wow: wscl::Clip,
    /// On the DS skeleton, model space, Bevy axes, WoW scale (for the side-by-side).
    pub ds: wscl::Clip,
    pub spec: EvadeSpec,
}

fn cache_dir() -> PathBuf {
    std::env::var("WOWSOULS_DSR_CACHE").map_or_else(|_| PathBuf::from("cache/dsr"), PathBuf::from)
}

/// Reads the manifest; `Err` names why the cache is unusable (missing, stale, other version).
fn load(dir: &Path) -> Result<DsRoll, String> {
    let manifest = std::fs::read_to_string(dir.join("manifest.txt"))
        .map_err(|e| format!("no manifest in {} ({e})", dir.display()))?;
    let mut roll = None;
    for line in manifest.lines() {
        let mut w = line.splitn(4, ' ');
        match (w.next(), w.next(), w.next(), w.next()) {
            (Some("wscl-cache"), Some(v), _, _) if v != "1" => return Err(format!("cache version {v}")),
            (Some("roll"), Some(name), _, _) => roll = Some(name.to_string()),
            (Some("source"), Some(len), Some(mtime), Some(path)) => {
                let meta = std::fs::metadata(path).map_err(|e| format!("source {path}: {e}"))?;
                let now = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs().to_string());
                if meta.len().to_string() != len || now.as_deref() != Some(mtime) {
                    return Err(format!("stale: {path} changed since extraction"));
                }
            }
            _ => {}
        }
    }
    let name = roll.ok_or("manifest names no roll")?;
    let read = |sub: &str| -> Result<wscl::Clip, String> {
        let p = dir.join(sub).join(format!("{name}.wscl"));
        let f = std::fs::File::open(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        wscl::Clip::read(&mut std::io::BufReader::new(f)).map_err(|e| format!("{}: {e}", p.display()))
    };
    let wow = read("human_male")?;
    let ds = read("c0000")?;
    let spec = roll_spec(&wow);
    Ok(DsRoll { wow, ds, spec })
}

fn startup(mut commands: Commands) {
    match load(&cache_dir()) {
        Ok(r) => {
            info!(
                "dsclip: Dark Souls roll {} ({:.3} s, {:.2} yd, invulnerable {:.3}-{:.3} s, cancel from {:.3} s)",
                r.wow.name, r.spec.duration, r.spec.distance, r.spec.iframes.0, r.spec.iframes.1, r.spec.cancel_from
            );
            commands.insert_resource(r);
        }
        Err(why) => warn!("dsclip: no Dark Souls roll ({why}); the procedural roll stays"),
    }
}

/// While the avatar rolls the DS roll (or the side-by-side runs), the clip owns the driven bones.
fn pose(
    roll: Option<Res<DsRoll>>,
    view: Option<Res<crate::dsview::DsView>>,
    mut avatar: Query<(&Action, &mut RigPose), With<Avatar>>,
    mut warned: Local<bool>,
) {
    let (Some(roll), Ok((action, mut pose))) = (roll, avatar.single_mut()) else {
        return;
    };
    let t = match (view.as_deref(), action.evading()) {
        (Some(v), _) => v.t,
        (None, Some(e)) if e.ds => e.t,
        _ => return,
    };
    if roll.wow.bones() != pose.locals.len() {
        if !*warned {
            *warned = true;
            warn!("dsclip: clip has {} bones, the rig {}", roll.wow.bones(), pose.locals.len());
        }
        return;
    }
    for (i, local) in pose.locals.iter_mut().enumerate() {
        if roll.wow.driven[i] {
            let (t, r, s) = sample(&roll.wow, t, i);
            *local = Transform { translation: t, rotation: r, scale: s };
        }
    }
    pose.pose_dirty = true;
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, startup)
        .add_systems(PostUpdate, pose.in_set(PosePost));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip() -> wscl::Clip {
        let a = wscl::Trs { t: [0.0, 1.0, 0.0], ..wscl::Trs::IDENTITY };
        let b = wscl::Trs {
            t: [0.0, 3.0, 0.0],
            r: Quat::from_rotation_y(1.0).to_array(),
            s: [1.0; 3],
        };
        wscl::Clip {
            name: "r".into(),
            fps: 10.0,
            model_space: false,
            parents: vec![-1],
            driven: vec![true],
            frames: vec![vec![a], vec![b]],
            root_motion: vec![[0.0; 4], [0.0, 0.0, -4.0, 0.0]],
            flags: vec![
                wscl::FlagWindow { flag: wscl::FLAG_INVINCIBLE, start: 0.02, end: 0.07 },
                wscl::FlagWindow { flag: 11, start: 0.08, end: 0.1 },
            ],
        }
    }

    #[test]
    fn sampling_interpolates_between_frames() {
        let (t, r, _) = sample(&clip(), 0.05, 0);
        assert!((t.y - 2.0).abs() < 1.0e-5);
        assert!((r.to_euler(EulerRot::YXZ).0 - 0.5).abs() < 1.0e-4);
        let (t, _, _) = sample(&clip(), 9.0, 0);
        assert!((t.y - 3.0).abs() < 1.0e-5, "clamped to the last frame");
    }

    #[test]
    fn the_spec_comes_from_the_clip() {
        let s = roll_spec(&clip());
        assert!((s.duration - 0.1).abs() < 1.0e-6);
        assert!((s.distance - 4.0).abs() < 1.0e-5);
        assert_eq!(s.iframes, (0.02, 0.07));
        assert!((s.cancel_from - 0.08).abs() < 1.0e-6);
        assert!((progress(&clip(), 0.05) - 2.0).abs() < 1.0e-5);
    }
}
```

- [ ] **Step 2: Combat changes** — in `combat.rs`:

(a) `Evade` carries its shape and whether it is the DS roll. Replace the struct and its `spec()`:

```rust
/// An evade in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Evade {
    pub kind: EvadeKind,
    pub t: f32,
    /// The ground direction of travel, unit length.
    pub dir: Vec3,
    /// Its shape, fixed when it starts (the DS roll's comes from its clip).
    pub spec: EvadeSpec,
    /// Played from the Dark Souls clip ([`crate::dsclip`]): no somersault, root-motion travel.
    pub ds: bool,
}

impl Evade {
    pub fn spec(&self) -> &EvadeSpec {
        &self.spec
    }
```

(b) `tumble()` returns 0 for a DS roll: change its `EvadeKind::Roll =>` arm to `EvadeKind::Roll if !self.ds =>` and the `EvadeKind::Backstep => 0.0` arm to `_ => 0.0`.

(c) `StepInput` gains two fields after `facing`:

```rust
    /// The roll this body rolls: the DS roll's shape when loaded, else [`ROLL`].
    pub roll: EvadeSpec,
    /// The roll is the Dark Souls clip.
    pub ds_roll: bool,
```

(d) In `start`, replace `a.state = ActionState::Evade(Evade { kind, t: 0.0, dir });` with:

```rust
            let (spec, ds) = match kind {
                EvadeKind::Roll => (i.roll, i.ds_roll),
                EvadeKind::Backstep => (BACKSTEP, false),
            };
            a.state = ActionState::Evade(Evade { kind, t: 0.0, dir, spec, ds });
```

and in `Decision::Evade(kind)` handling in `act`, replace `stamina.spend(kind.spec().cost);` with `stamina.spend(action.evading().map_or(kind.spec().cost, |e| e.spec.cost));` and wrap the `play_evade` call: `if !e.ds { … play_evade … }`.

(e) In `act`'s parameters add `ds_roll: Option<Res<crate::dsclip::DsRoll>>,` and set the new `StepInput` fields:

```rust
        roll: ds_roll.as_deref().map_or(ROLL, |r| r.spec),
        ds_roll: ds_roll.is_some(),
```

(f) The evade's travel: replace the body of the `ActionState::Evade(e) => { … }` arm in `act` with:

```rust
        ActionState::Evade(e) => {
            // This frame's share of the evade's distance, as a speed along its direction: the DS
            // roll's own root motion, or the procedural curve.
            let prev = (e.t - dt).max(0.0);
            let step = match (e.ds, ds_roll.as_deref()) {
                (true, Some(r)) => crate::dsclip::progress(&r.wow, e.t) - crate::dsclip::progress(&r.wow, prev),
                _ => {
                    let spec = e.spec();
                    (Evade::covered(spec, e.t) - Evade::covered(spec, prev)) * spec.distance
                }
            };
            *intent = MoveIntent {
                dir: if step > 0.0 { e.dir } else { Vec3::ZERO },
                speed: if dt > 0.0 { step / dt } else { 0.0 },
                jump: false,
            };
        }
```

(g) Tests: in `mod tests`, the `input()` helper gains `roll: ROLL, ds_roll: false,`; the two closures that build `Evade { kind…, t, dir: Vec3::NEG_Z }` become `Evade { kind: EvadeKind::Roll, t, dir: Vec3::NEG_Z, spec: ROLL, ds: false }` (in `the_roll_is_invulnerable_only_inside_its_window`) and `Evade { kind, t, dir: Vec3::NEG_Z, spec: *kind.spec(), ds: false }` (in `the_somersault_turns_once_and_only_in_a_roll`). Add:

```rust
    #[test]
    fn a_ds_roll_takes_its_shape_from_the_input_and_does_not_tumble() {
        let ds_spec = EvadeSpec { duration: 1.0, travel: 1.0, distance: 3.0, iframes: (0.1, 0.4), cancel_from: 0.7, cost: 20.0 };
        let mut a = Action::default();
        let i = StepInput { roll: ds_spec, ds_roll: true, stick: Vec3::NEG_Z, ..input(Some(Press::Evade), true) };
        assert_eq!(step_action(&mut a, &full(), i, DT), Decision::Evade(EvadeKind::Roll));
        let e = *a.evading().unwrap();
        assert!(e.ds && e.spec == ds_spec);
        assert_eq!(Evade { t: 0.5, ..e }.tumble(), 0.0);
    }
```

- [ ] **Step 3: Wire up** — `main.rs`: add `mod dsclip;` and `mod dsview;` (Task 15 creates `dsview.rs`; until then create it with `#[derive(bevy::prelude::Resource)] pub struct DsView { pub t: f32 }` and `pub fn plugin(_: &mut bevy::prelude::App) {}`). In `hits.rs`'s plugin, change `app.add_plugins(crate::hitbody::plugin)` to `app.add_plugins((crate::hitbody::plugin, crate::dsclip::plugin, crate::dsview::plugin))`. `Cargo.toml` of `wow-souls`: add `wscl.workspace = true`.

- [ ] **Step 4: Run tests** — `cargo test -j 4 -p wow-souls` → all pass (the existing 134 plus 3 new).

- [ ] **Step 5: Live check: the roll from the cache**

Run: `WOWSOULS_CHECK=8 WOWSOULS_DRIVE=-1,0 WOWSOULS_ROLL=4 WOWSOULS_TRACE=1 timeout 90 ./target/debug/wow-souls.exe 2>&1 | grep -E "dsclip|combat: Roll|moved|action t=  4"`
Expected: `dsclip: Dark Souls roll <name> (<dur> s, <dist> yd, invulnerable a-b s …)`; `combat: Roll moved <dist ±2 %> yd in <dur ±1 frame> s`; trace `invuln=true` exactly inside the DS window.

- [ ] **Step 6: Gates checkpoint** — fmt, clippy `-p wow-souls --all-targets -D warnings`, tests.

---

### Task 15: Side-by-side instrument and verification

**Files:**
- Modify: `crates/wow-souls/src/dsview.rs`
- Modify: `README.md`, `docs/HANDOVER.md`

- [ ] **Step 1: `dsview.rs`**

```rust
//! `WOWSOULS_DSVIEW=1`: the Dark Souls roll on the DS skeleton (gizmo bones, 1.5 yd to the
//! avatar's right) beside the WoW human playing the retargeted clip, in lockstep, looping, in
//! place (no root motion). With `WOWSOULS_OBSERVE` and `WOWSOULS_SHOT` it makes the comparison
//! frames; every 0.1 s it logs the clip time and the root distance covered.

use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::dsclip::{progress, DsRoll};

#[derive(Resource, Debug, Default)]
pub struct DsView {
    pub t: f32,
}

pub fn plugin(app: &mut App) {
    if std::env::var_os("WOWSOULS_DSVIEW").is_some() {
        app.init_resource::<DsView>().add_systems(Update, (advance, draw).chain());
    }
}

fn advance(time: Res<Time>, roll: Option<Res<DsRoll>>, mut view: ResMut<DsView>, mut next_log: Local<f32>) {
    let Some(roll) = roll else { return };
    let d = roll.wow.duration().max(1.0e-3);
    view.t = (view.t + time.delta_secs()) % d;
    *next_log -= time.delta_secs();
    if *next_log <= 0.0 {
        *next_log = 0.1;
        info!("dsview t={:.3}/{d:.3} root={:.2} yd", view.t, progress(&roll.wow, view.t));
    }
}

fn draw(roll: Option<Res<DsRoll>>, view: Res<DsView>, avatar: Query<&Transform, With<Avatar>>, mut gizmos: Gizmos) {
    let (Some(roll), Ok(body)) = (roll, avatar.single()) else { return };
    let origin = body.translation + *body.right() * 1.5;
    let at = |b: usize| origin + body.rotation * crate::dsclip::sample(&roll.ds, view.t, b).0;
    for (i, &p) in roll.ds.parents.iter().enumerate() {
        let joint = at(i);
        gizmos.sphere(Isometry3d::from_translation(joint), 0.025, Color::srgb(0.95, 0.75, 0.2));
        if let Ok(p) = usize::try_from(p) {
            gizmos.line(at(p), joint, Color::srgb(0.95, 0.75, 0.2));
        }
    }
}
```

- [ ] **Step 2: Build and capture the side-by-side** — the avatar faces north at the anchor; the observer stands to the west, side-on:

```bash
cd /c/Spel/wow-souls && cargo build -j 4 -p wow-souls
for at in 4.10 4.25 4.40 4.55; do WOWSOULS_DSVIEW=1 WOWSOULS_CHECK=8 WOWSOULS_OBSERVE=-4,1.2,0 WOWSOULS_SHOT="C:\\Users\\jacob\\Documents\\GitHub\\wow darksouls\\docs\\evidence\\dsview-$at.png" WOWSOULS_SHOT_AT=$at timeout 90 ./target/debug/wow-souls.exe 2>&1 | grep -E "dsclip|dsview t" | tail -2; done
```

Expected: four PNGs, each showing the gizmo DS skeleton and the WoW human in the same phase of the roll. Read them; the look is the user's call (success criterion 1).

- [ ] **Step 3: Numbers (success criterion 2)** — from Task 13's extractor line and Task 14's live check, confirm and write into `docs/HANDOVER.md`: duration (clip vs live, ±1 frame), distance (root motion vs `moved`, ±2 %), invulnerable window (TAE vs trace, ±1 frame), max direction error (< 15°).

- [ ] **Step 4: Fallback check (success criterion 3)** — `WOWSOULS_DSR_CACHE=/nonexistent WOWSOULS_CHECK=6 WOWSOULS_ROLL=4 WOWSOULS_DRIVE=-1,0 timeout 60 ./target/debug/wow-souls.exe 2>&1 | grep -E "dsclip|Roll moved"` → one `no Dark Souls roll` warning and the procedural `Roll moved 4.00 yd`.

- [ ] **Step 5: Install untouched** — snapshot both installs before Task 9 and after this step (`find "<install>" -type f -printf '%p %s %T@\n' | sort`), diff: identical.

- [ ] **Step 6: Docs** — README: `WOWSOULS_DSVIEW`, `WOWSOULS_DSR_CACHE`, and the `wow-souls-dsr` commands (`cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- extract`); HANDOVER: new crates, licence split, numbers, screenshots, next step (user judges the look; then plan C movesets).

- [ ] **Step 7: Final gates** — `cargo fmt --all -- --check`; `cargo clippy -j 4 --workspace --all-targets -- -D warnings`; `cargo test -j 4 --workspace`; `DSR_DIR='C:/Spel/Dark Souls Remastered' cargo test -j 4 -p dsr-formats`.

---

## Self-review notes

- Spec coverage: data reader (Tasks 2-8), cache layout (13), retarget method (11), roll identification (12), playback in `PosePost` (14), evade integration (14), side-by-side (15), error handling (manifest staleness and bone-count checks in 14; per-file errors in 9/13), testing (unit + install smoke + live), locality (`/cache` gitignored; install diff in 15). Params deferred to plan C (stated at the top).
- Types used across tasks: `Qs` (Task 6) used by `spline` (7), `dsr` (9), `retarget` (11); `SplineAnim` (8) used by `spline` (7) — do Task 8 Step 1 before Task 7 Step 3; `wscl::Clip`/`Trs`/`FlagWindow`/`FLAG_INVINCIBLE` (1) used in 9-15; `EvadeSpec` fields as in `combat.rs` today (`duration, travel, distance, iframes, cancel_from, cost`).
