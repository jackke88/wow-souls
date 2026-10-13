# Straight-Sword Moveset Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Play Dark Souls Remastered's straight-sword R1 chain, R2 and rolling attack on the WoW human from the install's own animations, timing, stamina and damage multipliers, replacing the procedural light attack whenever the cache is present.

**Architecture:** The GPL extractor (`dsr-cache`, with a new `param.rs` in `dsr-formats`) retargets six sword clips (`a23_*`), reads each move's hit window and cancel windows from its TAE and its stamina and damage multiplier from the params, and writes them with the roll into `cache/dsr/` (one manifest, written last). The MIT/Apache game reads only the cache: `combat/action.rs` is the pure action machine with timing as data (`SwingSpec`, `Moveset`), `dsclip.rs` poses the rig from any cached clip and turns the held sword's blade onto the Dark Souls blade axis.

**Tech Stack:** Rust 1.98.1, Bevy 0.18.1, glam 0.31, anyhow. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-07-straight-sword-moveset-design.md` (amended 2026-10-07 for what planning measured; see below).

**Repository rules (override the skill's defaults):** the user commits; never `git commit` unless asked (the repo has no commits). Each task ends with a gates checkpoint instead. Always `-j 4` (RAM-tight machine). Never write inside `C:\Spel\Dark Souls Remastered` or `C:\Spel\World of Warcraft 1.12`. Install tests need `DSR_DIR='C:/Spel/Dark Souls Remastered'` and, where noted, `WOW_DATA='C:/Spel/World of Warcraft 1.12/Data'`. Run every command from `C:\Spel\wow-souls` (`cd /c/Spel/wow-souls`).

**Gates (the checkpoint at the end of a task):**

```bash
cd /c/Spel/wow-souls && cargo fmt --all -- --check && cargo clippy -j 4 --workspace --all-targets -- -D warnings && cargo test -j 4 --workspace
```

Expected at the start: fmt clean, clippy clean, tests pass (wow-souls 157, dsr-formats 20, dsr-cache 13, wscl 3).

**Deviations from the spec, decided while planning (the spec is amended to match):**
- The moves ride in the manifest (`class`, `move`, `grip` lines), not in a second `moves.txt`: one file, written last, is one atomic switch.
- The DS blade axis is the `R_Weapon` bone's **+Y**, measured, not read from the weapon model (see Facts). No FLVER reader.
- The hit radius stays the game's `BLADE_RADIUS`; Dark Souls' own (`AtkParam_Pc.hit0_Radius`, 0.30-0.40 m) is left for the impact spec (D).
- A move's root motion is applied whole, recoil included (R1-2 steps back 0.16 yd after its thrust): a step back is a negative `MoveIntent.speed` along the facing, so the body does not turn. The spec's lunge column is the net travel.
- The side-by-side's blade check is one line per complete loop (`blade error: worst ... mean ...`), with the body standing, instead of a number added to every `dsview` line.

**Facts used below (measured on the install, 2026-10-07):**
- `a20` is the dagger set. Straight swords: `EquipParamWeapon.wepmotionCategory` 23, animation set `a23`; the Shortsword is row 200000 (`behaviorVariationId` 2300, 2.0 kg).
- PARAMDEF, DSR format: header 0x30 (u16 header size at 0x04, u16 field count at 0x08, u16 field size 0xB0 at 0x0A, 32-byte type name at 0x0C, u16 version 104 at 0x2E); field records 0xB0 bytes (i32 byte count at +0x64, 32-byte internal type at +0x6C, 32-byte internal name at +0x8C). A name `x:3` is a bit field of 3 bits in a unit of the field's byte count; consecutive bit fields of one unit size share a unit while they fit. Enum types are unsigned integers of their byte count. With this layout every one of the 40 `GameParam` params has the row size its PARAM stores as the stride.
- PARAM: row count u16 at 0x0A, 32-byte type name at 0x0C, row table at 0x30 of (i32 id, u32 data offset, u32 name offset).
- `BehaviorParam_PC` row id is `100_000_000 + variation * 1000 + judge`; fields `variationId` (s32), `behaviorJudgeId` (s32), `refType` (u8, 0 = attack), `refId` (s32), `stamina` (s32). `AtkParam_Pc`: `atkPhysCorrection` (u16, percent), `hit0_Radius` (f32). `atkPhys` is 0 everywhere.
- TAE: a type-1 event is a hit window; its third parameter int is the behavior judge id. Type-0 events are `ChrActionFlag` windows (4 chain, 26 evade, 11 movement, inferred).
- The c0000 skeleton has 61 bones; `R_Hand` is bone 45 and `R_Weapon` bone 56, a child of the hand with a local rotation that is the identity in every frame of every clip. The skeleton faces -Z. In the c0000 hand frame the thumb is on +Y, the fingers split along Y, and the weapon bone sits on the palm side (+Z).
- WoW HumanMale: the right hand is bone 97 (bone map pair `R_Hand` ↔ 97). `HeldBuilt.blade` is `[top, bottom]` (`$WTT`, `$WTB`) in the held item root's frame.
- The six moves (clip, judge, length s, hit window s, f4 / f26 / f11 s, lunge yd, stamina, damage ×): `a23_3000` 0, 2.500, 0.533-0.667, 0.767 / 0.900 / 1.033, 1.98, 20, 1.00; `a23_3001` 10, 2.233, 0.467-0.567, 0.733 / 0.900 / 1.000, 1.42, 20, 1.00; `a23_3002` 20, 2.500, 0.467-0.600, 0.767 / 0.900 / 1.033, 1.98, 20, 1.00; `a23_3300` 100, 2.533, 0.733-0.867, 1.300 / 1.300 / 1.367, 1.98, 45, 1.35; `a23_3310` 110, 1.900, 0.333-0.467, 0.933 / 0.933 / 1.200, 0.94, 35, 1.25; `a23_3900` 540, 2.100, 0.233-0.333, 0.700 / 0.767 / 0.867, 1.42, 20, 0.90.

---

## File map

| File | Responsibility |
|---|---|
| `crates/dsr-formats/src/tae.rs` | + `Event::attack_judge` |
| `crates/dsr-formats/src/param.rs` (new) | PARAMDEF + PARAM → rows read by field name |
| `crates/dsr-formats/src/lib.rs` | `pub mod param;` |
| `crates/wscl/src/lib.rs` | clip format v2: hit windows; flag constants; extractor version 3 |
| `crates/dsr-cache/src/dsr.rs` | + `Player::hits`; `archive` crate-visible |
| `crates/dsr-cache/src/params.rs` (new) | the three params the moves need: weapon row, behavior row, attack row |
| `crates/dsr-cache/src/moveset.rs` (new) | the hand-made class record: which clip is which move |
| `crates/dsr-cache/src/bonemap.rs` | + the DS hand, weapon bone and blade axis |
| `crates/dsr-cache/src/cache.rs` | extracts the roll and the six moves; manifest lines; grip axis |
| `crates/wow-souls/src/combat/mod.rs`, `combat/action.rs` | `combat.rs` split; then `SwingSpec`, `MoveKind`, `Moveset`, heavy press |
| `crates/wow-souls/src/dsclip.rs` | loads the roll and the moves; poses any cached clip; grip fit |
| `crates/wow-souls/src/dsview.rs` | side-by-side for any cached clip; blade error |
| `crates/wow-souls/src/hits.rs` | swing serial; damage × multiplier (`hit_damage`) |
| `crates/wow-souls/src/motor/mod.rs` | `MoveIntent.speed` may be negative (a clip's recoil) |
| `crates/wow-souls/src/hud.rs` | help line text |
| `README.md`, `docs/HANDOVER.md`, the spec | controls, switches, what was built and measured |

---

## Before Task 1

Pick a scratch folder outside the repo (`SCRATCH=...`; the examples use `$SCRATCH`) and record what is in the two installs, to prove at the end (Task 12) that nothing wrote to them. The Dark Souls listing takes about 45 s:

```bash
mkdir -p "$SCRATCH/installs"
find "/c/Spel/Dark Souls Remastered" -type f -printf '%p %s %T@\n' | sort > "$SCRATCH/installs/dsr-before.txt"
find "/c/Spel/World of Warcraft 1.12" -type f -not -name realmlist.wtf -printf '%p %s %T@\n' | sort > "$SCRATCH/installs/wow-before.txt"
```

The plan's edits are exact-text edits and file creations. Where a step says **Create** `path`, write the whole block into that file (replacing any file there); where it says **Edit** `path` with Old/New, the Old text occurs exactly once in the file and is replaced by the New text. A path starting `scratch:` is a file in `$SCRATCH`, not in the repo. Appendix A is a small script that applies these blocks for you, task by task, and copes with files that use CRLF line endings (parts of `tae.rs` do). The Task 4 and 5 code is GPL (the extractor crates); everything the game reads is `.wscl` and the manifest.

---

### Task 1: `Event::attack_judge`

**Files:**
- Modify: `crates/dsr-formats/src/tae.rs`

- [ ] **Step 1: Write the failing tests**

**Edit** `crates/dsr-formats/src/tae.rs`:

Old:
```rust
        assert!(anims.len() > 100);
        assert!(!invincible.is_empty());
    }
}
```
New:
```rust
        assert!(anims.len() > 100);
        assert!(!invincible.is_empty());
    }

    fn event(kind: i32, params: [i32; 4]) -> Event {
        Event {
            kind,
            start: 0.1,
            end: 0.2,
            params: params.iter().flat_map(|p| p.to_le_bytes()).collect(),
        }
    }

    #[test]
    fn an_attack_event_carries_its_judge_id_in_its_third_int() {
        assert_eq!(event(ATTACK_BEHAVIOR, [0, 0, 540, 0]).attack_judge(), Some(540));
        assert_eq!(event(ATTACK_BEHAVIOR, [0, 0, 0, 128]).attack_judge(), Some(0));
        // Other event types have no judge, whatever their ints say.
        assert_eq!(event(CHR_ACTION_FLAG, [8, 0, 540, 0]).attack_judge(), None);
        // A short parameter block (the end of the file) has none either.
        let short = Event {
            kind: ATTACK_BEHAVIOR,
            start: 0.0,
            end: 0.1,
            params: vec![0; 8],
        };
        assert_eq!(short.attack_judge(), None);
    }

    #[test]
    fn install_sword_attacks_carry_their_judge_ids() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let raw = std::fs::read(format!("{dir}/chr/c0000.anibnd.dcx")).unwrap();
        let entries = bnd3::read(&dcx::decompress(&raw).unwrap()).unwrap();
        let tae = entries
            .iter()
            .find(|e| e.file_name() == "a23.tae")
            .expect("a23.tae in c0000.anibnd");
        let anims = read(&tae.data).unwrap();
        for (id, want) in [(3000, 0), (3001, 10), (3002, 20), (3300, 100), (3310, 110), (3900, 540)] {
            let hits: Vec<(i32, f32, f32)> = anims
                .iter()
                .find(|a| a.id == id)
                .unwrap_or_else(|| panic!("a23 has no animation {id}"))
                .events
                .iter()
                .filter_map(|e| Some((e.attack_judge()?, e.start, e.end)))
                .collect();
            assert_eq!(hits.len(), 1, "a23_{id}: {hits:?}");
            assert_eq!(hits[0].0, want, "a23_{id}");
            assert!(hits[0].1 < hits[0].2, "a23_{id}: {hits:?}");
        }
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -j 4 -p dsr-formats an_attack_event`
Expected: FAIL to compile: `cannot find value ATTACK_BEHAVIOR` / `no method named attack_judge`.

- [ ] **Step 3: Implement**

**Edit** `crates/dsr-formats/src/tae.rs`:

Old:
```rust
pub const CHR_ACTION_FLAG: i32 = 0;
```
New:
```rust
pub const CHR_ACTION_FLAG: i32 = 0;
/// Event type 1, `InvokeAttackBehavior`: an attack's hit window. Its third parameter int is the
/// behavior judge id, which keys `BehaviorParam_PC`.
pub const ATTACK_BEHAVIOR: i32 = 1;
```

**Edit** `crates/dsr-formats/src/tae.rs`:

Old:
```rust
    /// The `ChrActionFlag` value, for a type-0 event.
    pub fn action_flag(&self) -> Option<i32> {
        (self.kind == CHR_ACTION_FLAG)
            .then(|| self.param_i32(0))
            .flatten()
    }
```
New:
```rust
    /// The `ChrActionFlag` value, for a type-0 event.
    pub fn action_flag(&self) -> Option<i32> {
        (self.kind == CHR_ACTION_FLAG)
            .then(|| self.param_i32(0))
            .flatten()
    }

    /// The behavior judge id, for a type-1 (attack) event.
    pub fn attack_judge(&self) -> Option<i32> {
        (self.kind == ATTACK_BEHAVIOR)
            .then(|| self.param_i32(2))
            .flatten()
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -j 4 -p dsr-formats tae`
Expected: PASS (the new unit test; the install test prints "skipped").
Run: `DSR_DIR='C:/Spel/Dark Souls Remastered' cargo test -j 4 -p dsr-formats install_sword_attacks -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Checkpoint**

Run the gates (top of this plan). Expected: all green.

---

### Task 2: `param.rs`: PARAMDEF and PARAM

**Files:**
- Create: `crates/dsr-formats/src/param.rs`
- Modify: `crates/dsr-formats/src/lib.rs`
- Modify: `docs/PROVENANCE.md`

- [ ] **Step 1: Declare the module**

**Edit** `crates/dsr-formats/src/lib.rs`:

Old:
```rust
//! animation events and Havok 2015 tagfiles (skeletons, spline-compressed animations).
```
New:
```rust
//! animation events, PARAM files with their PARAMDEFs, and Havok 2015 tagfiles (skeletons,
//! spline-compressed animations).
```

**Edit** `crates/dsr-formats/src/lib.rs`:

Old:
```rust
pub mod hkx;
pub mod tae;
```
New:
```rust
pub mod hkx;
pub mod param;
pub mod tae;
```

- [ ] **Step 2: Write the file with its tests**

**Create** `crates/dsr-formats/src/param.rs`:
```rust
//! PARAMDEF and PARAM, in the format Dark Souls Remastered ships (PARAMDEF version 104, PARAM with a
//! 0x30-byte header and a row table). The layout follows SoulsFormatsNEXT `PARAMDEF.cs` and
//! `PARAM.cs`, and is checked against the user's install: the row size a PARAMDEF's fields add up
//! to is the row stride the PARAM stores, for all 40 params of `GameParam`.

use anyhow::{bail, ensure, Context, Result};

use crate::bytes::{f32_at, i32_at, slice, u16_at, u32_at, u8_at};

/// The PARAMDEF version of DSR's files (at 0x2E).
pub const PARAMDEF_VERSION: u16 = 104;
/// The header of both files, and the size of one PARAMDEF field record.
const HEADER: usize = 0x30;
const FIELD_RECORD: usize = 0xB0;

/// One field of a PARAMDEF, placed in the row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    /// `s32`, `u8`, `f32`, `dummy8`, or an enum's name (stored as an unsigned integer).
    pub ty: String,
    pub offset: usize,
    pub size: usize,
    /// A bit field: its first bit and width inside the `size`-byte unit at `offset`.
    pub bits: Option<(u32, u32)>,
}

impl Field {
    fn signed(&self) -> bool {
        matches!(self.ty.as_str(), "s8" | "s16" | "s32")
    }

    fn float(&self) -> bool {
        self.ty == "f32"
    }
}

/// The layout of one param's rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paramdef {
    pub param_type: String,
    pub fields: Vec<Field>,
    pub row_size: usize,
}

/// Text in a fixed-size field: up to the first NUL, trimmed.
fn fixed_str(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim().to_string()
}

/// The param type a PARAM file declares (`BEHAVIOR_PARAM_ST`), to pick its PARAMDEF by.
pub fn param_type(bytes: &[u8]) -> Result<String> {
    Ok(fixed_str(slice(bytes, 0x0C, 32)?))
}

impl Paramdef {
    pub fn read(b: &[u8]) -> Result<Paramdef> {
        let header = usize::from(u16_at(b, 0x04)?);
        let count = usize::from(u16_at(b, 0x08)?);
        let record = usize::from(u16_at(b, 0x0A)?);
        let version = u16_at(b, 0x2E)?;
        ensure!(
            version == PARAMDEF_VERSION,
            "PARAMDEF version {version}, expected {PARAMDEF_VERSION}"
        );
        ensure!(
            header == HEADER && record == FIELD_RECORD,
            "PARAMDEF header {header:#x} with {record:#x}-byte fields, expected {HEADER:#x} and {FIELD_RECORD:#x}"
        );
        let param_type = fixed_str(slice(b, 0x0C, 32)?);
        let mut fields = Vec::with_capacity(count);
        let mut offset = 0;
        // The bit field unit being filled: its offset, the bits used, the bits it holds.
        let mut unit: Option<(usize, u32, u32)> = None;
        for i in 0..count {
            let at = HEADER + i * FIELD_RECORD;
            let size = usize::try_from(i32_at(b, at + 0x64)?)
                .with_context(|| format!("field {i} of {param_type}: negative size"))?;
            let ty = fixed_str(slice(b, at + 0x6C, 32)?);
            let name = fixed_str(slice(b, at + 0x8C, 32)?);
            if let Some((bare, width)) = name.split_once(':') {
                let width: u32 = width
                    .trim()
                    .parse()
                    .with_context(|| format!("bit field {name} of {param_type}"))?;
                let bits = u32::try_from(size * 8).context("field size")?;
                ensure!(
                    (1..=bits).contains(&width),
                    "bit field {name}: {width} bits in {size} bytes"
                );
                let (at_offset, first) = match unit {
                    Some((u, used, held)) if held == bits && used + width <= held => (u, used),
                    _ => {
                        let u = offset;
                        offset += size;
                        (u, 0)
                    }
                };
                unit = Some((at_offset, first + width, bits));
                fields.push(Field {
                    name: bare.trim().to_string(),
                    ty,
                    offset: at_offset,
                    size,
                    bits: Some((first, width)),
                });
            } else {
                unit = None;
                fields.push(Field {
                    name,
                    ty,
                    offset,
                    size,
                    bits: None,
                });
                offset += size;
            }
        }
        Ok(Paramdef {
            param_type,
            fields,
            row_size: offset,
        })
    }
}

/// One param file's rows, read through its [`Paramdef`].
#[derive(Debug, Clone)]
pub struct Param {
    pub param_type: String,
    def: Paramdef,
    bytes: Vec<u8>,
    /// Per row: its id and where its data starts.
    rows: Vec<(i32, usize)>,
}

impl Param {
    pub fn read(bytes: Vec<u8>, def: Paramdef) -> Result<Param> {
        let declared = param_type(&bytes)?;
        ensure!(
            declared == def.param_type,
            "PARAM {declared} read with the PARAMDEF of {}",
            def.param_type
        );
        let count = usize::from(u16_at(&bytes, 0x0A)?);
        let mut rows = Vec::with_capacity(count);
        for i in 0..count {
            let at = HEADER + i * 12;
            let id = i32_at(&bytes, at)?;
            let data = usize::try_from(u32_at(&bytes, at + 4)?).context("row offset")?;
            slice(&bytes, data, def.row_size).with_context(|| format!("row {id} of {declared}"))?;
            rows.push((id, data));
        }
        if let [(_, a), (_, b), ..] = rows[..] {
            ensure!(
                b.checked_sub(a) == Some(def.row_size),
                "{declared}: its PARAMDEF makes rows {} bytes, the file spaces them {} apart",
                def.row_size,
                b.wrapping_sub(a)
            );
        }
        Ok(Param {
            param_type: declared,
            def,
            bytes,
            rows,
        })
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The id of row `row` (an index, `0..len`).
    pub fn id(&self, row: usize) -> i32 {
        self.rows[row].0
    }

    /// The index of the row with this id.
    pub fn row_of(&self, id: i32) -> Option<usize> {
        self.rows.iter().position(|&(i, _)| i == id)
    }

    fn field(&self, name: &str) -> Result<&Field> {
        self.def
            .fields
            .iter()
            .find(|f| f.name == name)
            .with_context(|| format!("{} has no field {name}", self.param_type))
    }

    /// An integer field (signed, unsigned, enum or bit field) of row `row`.
    pub fn int(&self, row: usize, name: &str) -> Result<i64> {
        let f = self.field(name)?;
        ensure!(!f.float(), "{name} is a float");
        let at = self.rows[row].1 + f.offset;
        let raw = match f.size {
            1 => u64::from(u8_at(&self.bytes, at)?),
            2 => u64::from(u16_at(&self.bytes, at)?),
            4 => u64::from(u32_at(&self.bytes, at)?),
            n => bail!("{name}: {n}-byte integers are not read"),
        };
        if let Some((first, width)) = f.bits {
            return Ok(((raw >> first) & ((1u64 << width) - 1)) as i64);
        }
        Ok(match (f.signed(), f.size) {
            (true, 1) => i64::from(raw as u8 as i8),
            (true, 2) => i64::from(raw as u16 as i16),
            (true, _) => i64::from(raw as u32 as i32),
            (false, _) => raw as i64,
        })
    }

    /// A float field of row `row`.
    pub fn float(&self, row: usize, name: &str) -> Result<f32> {
        let f = self.field(name)?;
        ensure!(f.float() && f.size == 4, "{name} is not an f32");
        f32_at(&self.bytes, self.rows[row].1 + f.offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PARAMDEF: `(type, name, byte count)` per field.
    fn paramdef(param_type: &str, fields: &[(&str, &str, i32)]) -> Vec<u8> {
        let mut b = vec![0u8; HEADER + fields.len() * FIELD_RECORD];
        b[0x04..0x06].copy_from_slice(&(HEADER as u16).to_le_bytes());
        b[0x06..0x08].copy_from_slice(&2u16.to_le_bytes());
        b[0x08..0x0A].copy_from_slice(&(fields.len() as u16).to_le_bytes());
        b[0x0A..0x0C].copy_from_slice(&(FIELD_RECORD as u16).to_le_bytes());
        b[0x0C..0x0C + param_type.len()].copy_from_slice(param_type.as_bytes());
        b[0x2E..0x30].copy_from_slice(&PARAMDEF_VERSION.to_le_bytes());
        for (i, (ty, name, size)) in fields.iter().enumerate() {
            let at = HEADER + i * FIELD_RECORD;
            b[at + 0x64..at + 0x68].copy_from_slice(&size.to_le_bytes());
            b[at + 0x6C..at + 0x6C + ty.len()].copy_from_slice(ty.as_bytes());
            b[at + 0x8C..at + 0x8C + name.len()].copy_from_slice(name.as_bytes());
        }
        let len = b.len() as i32;
        b[0..4].copy_from_slice(&len.to_le_bytes());
        b
    }

    /// A PARAM of `param_type` with these `(id, row data)` rows, each `row_size` bytes.
    fn param(param_type: &str, row_size: usize, rows: &[(i32, Vec<u8>)]) -> Vec<u8> {
        let table = HEADER + rows.len() * 12;
        let mut b = vec![0u8; table];
        b[0x0A..0x0C].copy_from_slice(&(rows.len() as u16).to_le_bytes());
        b[0x0C..0x0C + param_type.len()].copy_from_slice(param_type.as_bytes());
        for (i, (id, data)) in rows.iter().enumerate() {
            assert_eq!(data.len(), row_size);
            let at = HEADER + i * 12;
            b[at..at + 4].copy_from_slice(&id.to_le_bytes());
            b[at + 4..at + 8].copy_from_slice(&((table + i * row_size) as u32).to_le_bytes());
        }
        for (_, data) in rows {
            b.extend_from_slice(data);
        }
        b
    }

    /// The layout the tests share: a signed int; bit fields of 1, 3 and 4 bits that fill a byte;
    /// one that starts the next byte; two padding bytes; a float, an enum and a signed short.
    fn def() -> Paramdef {
        Paramdef::read(&paramdef(
            "TEST_ST",
            &[
                ("s32", "a", 4),
                ("TEST_BOOL", "lo:1", 1),
                ("TEST_BOOL", "mid:3", 1),
                ("TEST_BOOL", "hi:4", 1),
                ("TEST_BOOL", "next:1", 1),
                ("dummy8", "pad[2]", 2),
                ("f32", "x", 4),
                ("TEST_KIND", "kind", 1),
                ("s16", "small", 2),
            ],
        ))
        .unwrap()
    }

    fn row(a: i32, byte4: u8, byte5: u8, x: f32, kind: u8, small: i16) -> Vec<u8> {
        let mut r = a.to_le_bytes().to_vec();
        r.extend([byte4, byte5, 0, 0]);
        r.extend(x.to_le_bytes());
        r.push(kind);
        r.extend(small.to_le_bytes());
        r
    }

    fn sample() -> Param {
        let rows = [
            (10, row(-5, 0b1010_0101, 0b1, 1.5, 7, -2)),
            (20, row(300, 0, 0, -0.25, 255, 12)),
        ];
        Param::read(param("TEST_ST", 15, &rows), def()).unwrap()
    }

    #[test]
    fn bit_fields_share_a_unit_until_it_is_full() {
        let d = def();
        let at = |n: &str| {
            let f = d.fields.iter().find(|f| f.name == n).unwrap();
            (f.offset, f.bits)
        };
        assert_eq!(at("a"), (0, None));
        assert_eq!(at("lo"), (4, Some((0, 1))));
        assert_eq!(at("mid"), (4, Some((1, 3))));
        assert_eq!(at("hi"), (4, Some((4, 4))));
        assert_eq!(at("next"), (5, Some((0, 1))), "the byte was full");
        assert_eq!(at("pad[2]"), (6, None));
        assert_eq!(at("x"), (8, None));
        assert_eq!(at("kind"), (12, None));
        assert_eq!(at("small"), (13, None));
        assert_eq!(d.row_size, 15);
    }

    #[test]
    fn rows_read_by_field() {
        let p = sample();
        assert_eq!((p.len(), p.id(0), p.id(1)), (2, 10, 20));
        assert_eq!(p.row_of(20), Some(1));
        assert_eq!(p.row_of(99), None);
        let first = p.row_of(10).unwrap();
        assert_eq!(p.int(first, "a").unwrap(), -5);
        assert_eq!(p.int(first, "lo").unwrap(), 1);
        assert_eq!(p.int(first, "mid").unwrap(), 2);
        assert_eq!(p.int(first, "hi").unwrap(), 10);
        assert_eq!(p.int(first, "next").unwrap(), 1);
        assert_eq!(p.float(first, "x").unwrap(), 1.5);
        assert_eq!(p.int(first, "kind").unwrap(), 7);
        assert_eq!(p.int(first, "small").unwrap(), -2, "a signed short");
        let second = p.row_of(20).unwrap();
        assert_eq!(p.int(second, "a").unwrap(), 300);
        assert_eq!(p.int(second, "kind").unwrap(), 255, "an enum is unsigned");
        assert_eq!(p.float(second, "x").unwrap(), -0.25);
    }

    #[test]
    fn an_unknown_field_or_the_wrong_kind_is_an_error() {
        let p = sample();
        assert!(p.int(0, "nope").is_err());
        assert!(p.int(0, "x").is_err(), "x is a float");
        assert!(p.float(0, "a").is_err(), "a is an integer");
    }

    #[test]
    fn a_param_of_another_type_is_refused() {
        let err = Param::read(param("OTHER_ST", 15, &[(1, vec![0; 15])]), def()).unwrap_err();
        assert!(err.to_string().contains("OTHER_ST"), "{err}");
    }

    #[test]
    fn rows_that_do_not_fit_the_layout_are_refused() {
        // The PARAMDEF says 15 bytes; this file spaces its rows 16 apart.
        let b = param("TEST_ST", 16, &[(1, vec![0; 16]), (2, vec![0; 16])]);
        let err = Param::read(b, def()).unwrap_err();
        assert!(err.to_string().contains("16 apart"), "{err}");
    }

    #[test]
    fn other_paramdef_versions_are_refused() {
        let mut b = paramdef("TEST_ST", &[("s32", "a", 4)]);
        b[0x2E..0x30].copy_from_slice(&101u16.to_le_bytes());
        let err = Paramdef::read(&b).unwrap_err();
        assert!(err.to_string().contains("version 101"), "{err}");
    }

    #[test]
    fn install_every_game_param_loads_with_its_paramdef() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let read = |path: &str| {
            let raw = std::fs::read(format!("{dir}/{path}")).unwrap();
            crate::bnd3::read(&crate::dcx::decompress(&raw).unwrap()).unwrap()
        };
        let defs: Vec<Paramdef> = read("paramdef/paramdef.paramdefbnd.dcx")
            .iter()
            .map(|e| Paramdef::read(&e.data).unwrap_or_else(|er| panic!("{}: {er:#}", e.name)))
            .collect();
        let mut by_stem = std::collections::BTreeMap::new();
        for e in read("param/GameParam/GameParam.parambnd.dcx") {
            let ty = param_type(&e.data).unwrap();
            let def = defs
                .iter()
                .find(|d| d.param_type == ty)
                .unwrap_or_else(|| panic!("no PARAMDEF for {ty}"))
                .clone();
            let stem = e.stem().to_string();
            let p = Param::read(e.data, def).unwrap_or_else(|er| panic!("{stem}: {er:#}"));
            by_stem.insert(stem, p);
        }
        assert!(by_stem.len() >= 40, "{} params", by_stem.len());

        let w = &by_stem["EquipParamWeapon"];
        let r = w.row_of(200_000).expect("the Shortsword");
        assert_eq!(w.int(r, "wepmotionCategory").unwrap(), 23);
        assert_eq!(w.int(r, "behaviorVariationId").unwrap(), 2300);

        let b = &by_stem["BehaviorParam_PC"];
        let r = b.row_of(102_300_010).expect("variation 2300, judge 10");
        assert_eq!(b.int(r, "variationId").unwrap(), 2300);
        assert_eq!(b.int(r, "behaviorJudgeId").unwrap(), 10);
        assert_eq!(b.int(r, "refType").unwrap(), 0);
        assert_eq!(b.int(r, "refId").unwrap(), 2_300_010);
        assert_eq!(b.int(r, "stamina").unwrap(), 20);

        let a = &by_stem["AtkParam_Pc"];
        let r = a.row_of(2_300_100).expect("the R2");
        assert_eq!(a.int(r, "atkPhysCorrection").unwrap(), 135);
        assert!((a.float(r, "hit0_Radius").unwrap() - 0.30).abs() < 1.0e-6);
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -j 4 -p dsr-formats param`
Expected: the six unit tests PASS; the install test prints "skipped".
Run: `DSR_DIR='C:/Spel/Dark Souls Remastered' cargo test -j 4 -p dsr-formats install_every_game_param -- --nocapture`
Expected: PASS (all 41 params of `GameParam` load; this is the proof that the layout algorithm is right).

- [ ] **Step 4: Provenance**

**Edit** `docs/PROVENANCE.md`:

Old:
```
- SoulsFormatsNEXT (soulsmods, GPL-3.0): `Formats/DCX.cs`, `Formats/Binder/*` (BND3),
  `Formats/TAE/*` (DS1 TAE).
```
New:
```
- SoulsFormatsNEXT (soulsmods, GPL-3.0): `Formats/DCX.cs`, `Formats/Binder/*` (BND3),
  `Formats/TAE/*` (DS1 TAE), `Formats/PARAM.cs` and `Formats/PARAMDEF.cs` (the layout of DSR's
  PARAM files and PARAMDEF version 104).
```

**Edit** `docs/PROVENANCE.md`:

Old:
```
- DSAnimStudio's `TAE.Template.DS1.xml` for event meanings (type 0 `ChrActionFlag`, flag 8
  Invincible).
```
New:
```
- DSAnimStudio's `TAE.Template.DS1.xml` for event meanings (type 0 `ChrActionFlag`, flag 8
  Invincible).
- Souls Modding wiki (soulsmodding.com, DS1 reference material): the attack animation id scheme
  and the field meanings of `BehaviorParam`, `AtkParam` and `EquipParamWeapon`.
```

- [ ] **Step 5: Checkpoint**

Run the gates. Expected: all green (dsr-formats now 29 tests).

---

### Task 3: `wscl` v2: hit windows

**Files:**
- Modify: `crates/wscl/src/lib.rs`
- Modify: `crates/dsr-cache/src/retarget.rs` (two `Clip` literals)
- Modify: `crates/dsr-cache/src/cache.rs` (one test literal)
- Modify: `crates/wow-souls/src/dsclip.rs` (one test literal)

- [ ] **Step 1: Write the failing tests**

**Edit** `crates/wscl/src/lib.rs`:

Old:
```rust
            flags: vec![FlagWindow {
                flag: FLAG_INVINCIBLE,
                start: 0.05,
                end: 0.4,
            }],
        }
    }
```
New:
```rust
            flags: vec![FlagWindow {
                flag: FLAG_INVINCIBLE,
                start: 0.05,
                end: 0.4,
            }],
            hits: vec![HitWindow {
                judge: 540,
                start: 0.25,
                end: 0.35,
            }],
        }
    }
```

**Edit** `crates/wscl/src/lib.rs`:

Old:
```rust
        assert_eq!(
            back.windows(FLAG_INVINCIBLE).collect::<Vec<_>>(),
            [(0.05, 0.4)]
        );
    }
```
New:
```rust
        assert_eq!(
            back.windows(FLAG_INVINCIBLE).collect::<Vec<_>>(),
            [(0.05, 0.4)]
        );
        assert_eq!(
            back.hits,
            [HitWindow {
                judge: 540,
                start: 0.25,
                end: 0.35
            }]
        );
    }

    #[test]
    fn a_version_1_file_is_refused() {
        let err = Clip::read(&mut &b"WSCL\x01\x00\x00\x00"[..]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("version 1, expected 2"), "{err}");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -j 4 -p wscl`
Expected: FAIL to compile: `cannot find struct HitWindow`.

- [ ] **Step 3: Implement**

**Edit** `crates/wscl/src/lib.rs`:

Old:
```rust
//! (per frame 4 f32), flag-window count u32 and windows (flag i32, start f32, end f32).
```
New:
```rust
//! (per frame 4 f32), flag-window count u32 and windows (flag i32, start f32, end f32), hit-window
//! count u32 and windows (behavior judge i32, start f32, end f32).
```

**Edit** `crates/wscl/src/lib.rs`:

Old:
```rust
pub const VERSION: u32 = 1;
/// The `ChrActionFlag` value for invulnerability.
pub const FLAG_INVINCIBLE: i32 = 8;
/// The extractor's version, written to the cache manifest: bump it when the retarget output
/// changes. The game rejects a cache written by any other version.
pub const EXTRACTOR_VERSION: u32 = 2;
```
New:
```rust
pub const VERSION: u32 = 2;
/// The `ChrActionFlag` value for invulnerability.
pub const FLAG_INVINCIBLE: i32 = 8;
/// The `ChrActionFlag` values whose windows open when the next action may start: an attack
/// (flag 4), an evade (26) or movement (11). Read from the data's pattern, not documented.
pub const FLAG_CHAIN: i32 = 4;
pub const FLAG_EVADE: i32 = 26;
pub const FLAG_MOVE: i32 = 11;
/// The extractor's version, written to the cache manifest: bump it when the extractor's output
/// changes. The game rejects a cache written by any other version.
pub const EXTRACTOR_VERSION: u32 = 3;
```

**Edit** `crates/wscl/src/lib.rs`:

Old:
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlagWindow {
    pub flag: i32,
    pub start: f32,
    pub end: f32,
}
```
New:
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlagWindow {
    pub flag: i32,
    pub start: f32,
    pub end: f32,
}

/// An attack's hit window from the animation's TAE (an attack-behavior event): the blade can hit
/// over `[start, end)`; `judge` keys the behavior params (stamina, damage).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitWindow {
    pub judge: i32,
    pub start: f32,
    pub end: f32,
}
```

**Edit** `crates/wscl/src/lib.rs`:

Old:
```rust
    pub root_motion: Vec<[f32; 4]>,
    pub flags: Vec<FlagWindow>,
}
```
New:
```rust
    pub root_motion: Vec<[f32; 4]>,
    pub flags: Vec<FlagWindow>,
    pub hits: Vec<HitWindow>,
}
```

**Edit** `crates/wscl/src/lib.rs`:

Old:
```rust
        put_u32(w, self.flags.len() as u32)?;
        for f in &self.flags {
            w.write_all(&f.flag.to_le_bytes())?;
            put_f32(w, f.start)?;
            put_f32(w, f.end)?;
        }
        Ok(())
```
New:
```rust
        put_u32(w, self.flags.len() as u32)?;
        for f in &self.flags {
            w.write_all(&f.flag.to_le_bytes())?;
            put_f32(w, f.start)?;
            put_f32(w, f.end)?;
        }
        put_u32(w, self.hits.len() as u32)?;
        for h in &self.hits {
            w.write_all(&h.judge.to_le_bytes())?;
            put_f32(w, h.start)?;
            put_f32(w, h.end)?;
        }
        Ok(())
```

**Edit** `crates/wscl/src/lib.rs`:

Old:
```rust
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
```
New:
```rust
        let n = get_u32(r)? as usize;
        let mut hits = Vec::with_capacity(n);
        for _ in 0..n {
            hits.push(HitWindow {
                judge: i32::from_le_bytes(get(r)?),
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
            hits,
        })
```

**Edit** `crates/dsr-cache/src/retarget.rs`:

Old:
```rust
        root_motion: root_motion.clone(),
        flags: flags.clone(),
    };
```
New:
```rust
        root_motion: root_motion.clone(),
        flags: flags.clone(),
        hits: Vec::new(),
    };
```

**Edit** `crates/dsr-cache/src/retarget.rs`:

Old:
```rust
        root_motion,
        flags,
    };
    Ok(Retargeted {
```
New:
```rust
        root_motion,
        flags,
        hits: Vec::new(),
    };
    Ok(Retargeted {
```

**Edit** `crates/dsr-cache/src/cache.rs`:

Old:
```rust
            flags: vec![wscl::FlagWindow {
                flag: wscl::FLAG_INVINCIBLE,
                start: 0.0,
                end: 0.033,
            }],
        };
        let path = dir.join("nested").join("a00_test.wscl");
```
New:
```rust
            flags: vec![wscl::FlagWindow {
                flag: wscl::FLAG_INVINCIBLE,
                start: 0.0,
                end: 0.033,
            }],
            hits: vec![wscl::HitWindow {
                judge: 7,
                start: 0.01,
                end: 0.02,
            }],
        };
        let path = dir.join("nested").join("a00_test.wscl");
```

**Edit** `crates/wow-souls/src/dsclip.rs`:

Old:
```rust
                wscl::FlagWindow {
                    flag: 11,
                    start: 0.08,
                    end: 0.1,
                },
            ],
        }
    }
```
New:
```rust
                wscl::FlagWindow {
                    flag: 11,
                    start: 0.08,
                    end: 0.1,
                },
            ],
            hits: Vec::new(),
        }
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -j 4 -p wscl`
Expected: PASS (4 tests).

- [ ] **Step 5: Checkpoint**

Run the gates. Expected: all green. (The manifest versions are not bumped yet: the game still loads the old cache; the extractor writes `EXTRACTOR_VERSION` 3 only in Task 5.)

---

### Task 4: `dsr-cache`: params, the class record, hit windows

**Files:**
- Create: `crates/dsr-cache/src/params.rs`
- Create: `crates/dsr-cache/src/moveset.rs`
- Modify: `crates/dsr-cache/src/main.rs`
- Modify: `crates/dsr-cache/src/dsr.rs`
- Modify: `crates/dsr-cache/src/bonemap.rs`

- [ ] **Step 1: Declare the modules and open the archive reader**

**Edit** `crates/dsr-cache/src/main.rs`:

Old:
```rust
mod dsr;
mod retarget;
```
New:
```rust
mod dsr;
mod moveset;
mod params;
mod retarget;
```

**Edit** `crates/dsr-cache/src/dsr.rs`:

Old:
```rust
fn archive(path: &Path) -> Result<Vec<bnd3::Entry>> {
```
New:
```rust
pub(crate) fn archive(path: &Path) -> Result<Vec<bnd3::Entry>> {
```

**Edit** `crates/dsr-cache/src/dsr.rs`:

Old:
```rust
    /// The `ChrActionFlag` windows of an animation.
    pub fn flags(&self, stem: &str) -> Vec<wscl::FlagWindow> {
```
New:
```rust
    /// The hit windows (attack-behavior events) of an animation, with their judge ids.
    pub fn hits(&self, stem: &str) -> Vec<wscl::HitWindow> {
        self.events(stem)
            .iter()
            .filter_map(|e| {
                Some(wscl::HitWindow {
                    judge: e.attack_judge()?,
                    start: e.start,
                    end: e.end,
                })
            })
            .collect()
    }

    /// The `ChrActionFlag` windows of an animation.
    pub fn flags(&self, stem: &str) -> Vec<wscl::FlagWindow> {
```

- [ ] **Step 2: The DS hand constants**

**Edit** `crates/dsr-cache/src/bonemap.rs`:

Old:
```rust
//! WoW indices from `benilla-extract m2bones Character\Human\Male\HumanMale.m2` (2026-10-07).

pub struct Pair {
```
New:
```rust
//! WoW indices from `benilla-extract m2bones Character\Human\Male\HumanMale.m2` (2026-10-07).

use glam::Vec3;

pub struct Pair {
```

**Edit** `crates/dsr-cache/src/bonemap.rs`:

Old:
```rust
/// The DS bones whose lowest one stands on the ground (the counterpart of `Checks::wow_soles`).
pub const DS_SOLES: &[&str] = &["L_Foot", "R_Foot", "L_Toe0", "R_Toe0"];
```
New:
```rust
/// The DS bones whose lowest one stands on the ground (the counterpart of `Checks::wow_soles`).
pub const DS_SOLES: &[&str] = &["L_Foot", "R_Foot", "L_Toe0", "R_Toe0"];

/// The DS sword hand and the weapon bone that sits unrotated in it (the weapon bone's local
/// rotation is the identity in every frame of every clip).
pub const DS_HAND: &str = "R_Hand";
pub const DS_WEAPON: &str = "R_Weapon";

/// The blade's axis in the weapon bone's own axes. Measured on the straight sword's moves, not
/// read from the weapon model (whose blade runs along -Y, flipped by an attach transform that is
/// not read): the grip runs across the palm (the thumb is on +Y, the fingers split along Y), and
/// over the hit windows of all six moves +Y reaches furthest in front of the hand (the thrust
/// `a23_3300`: 1.59 m, against 0.20 m for -Y).
pub const DS_BLADE_AXIS: Vec3 = Vec3::Y;
```

- [ ] **Step 3: `params.rs`**

**Create** `crates/dsr-cache/src/params.rs`:
```rust
//! The Dark Souls params the moves need, read through their PARAMDEFs: the weapon row that names
//! a class's motion category and behavior variation, the behavior row of an attack (stamina, the
//! attack row it points at) and that attack row's damage multiplier.

use std::path::Path;

use anyhow::{ensure, Context, Result};
use dsr_formats::param::{param_type, Param, Paramdef};

use crate::dsr::archive;

pub struct Params {
    weapon: Param,
    behavior: Param,
    attack: Param,
}

/// What a weapon row says about its moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weapon {
    pub motion_category: i64,
    pub variation: i64,
}

/// What an attack costs and does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behavior {
    pub stamina: i64,
    /// The damage as a multiple of the base hit: `AtkParam_Pc.atkPhysCorrection` is a percent.
    pub mult: f32,
}

pub fn load(dsr: &Path) -> Result<Params> {
    let defs: Vec<Paramdef> = archive(&dsr.join("paramdef").join("paramdef.paramdefbnd.dcx"))?
        .iter()
        .map(|e| Paramdef::read(&e.data).with_context(|| format!("PARAMDEF {}", e.name)))
        .collect::<Result<_>>()?;
    let entries = archive(
        &dsr.join("param")
            .join("GameParam")
            .join("GameParam.parambnd.dcx"),
    )?;
    let take = |stem: &str| -> Result<Param> {
        let e = entries
            .iter()
            .find(|e| e.stem() == stem)
            .with_context(|| format!("GameParam has no {stem}.param"))?;
        let ty = param_type(&e.data)?;
        let def = defs
            .iter()
            .find(|d| d.param_type == ty)
            .with_context(|| format!("no PARAMDEF for {ty}"))?
            .clone();
        Param::read(e.data.clone(), def).with_context(|| format!("{stem}.param"))
    };
    Ok(Params {
        weapon: take("EquipParamWeapon")?,
        behavior: take("BehaviorParam_PC")?,
        attack: take("AtkParam_Pc")?,
    })
}

impl Params {
    pub fn weapon(&self, id: i32) -> Result<Weapon> {
        let row = self
            .weapon
            .row_of(id)
            .with_context(|| format!("EquipParamWeapon has no row {id}"))?;
        Ok(Weapon {
            motion_category: self.weapon.int(row, "wepmotionCategory")?,
            variation: self.weapon.int(row, "behaviorVariationId")?,
        })
    }

    /// The behavior of an attack: the `BehaviorParam_PC` row with this variation and judge id
    /// (matched on the two fields, there must be exactly one) and the `AtkParam_Pc` row it points at.
    pub fn behavior(&self, variation: i64, judge: i32) -> Result<Behavior> {
        let b = &self.behavior;
        let mut rows = (0..b.len()).filter(|&r| {
            matches!(
                (b.int(r, "variationId"), b.int(r, "behaviorJudgeId")),
                (Ok(v), Ok(j)) if v == variation && j == i64::from(judge)
            )
        });
        let row = rows.next().with_context(|| {
            format!("BehaviorParam_PC has no row for variation {variation}, judge {judge}")
        })?;
        ensure!(
            rows.next().is_none(),
            "BehaviorParam_PC has two rows for variation {variation}, judge {judge}"
        );
        let id = b.id(row);
        ensure!(
            b.int(row, "refType")? == 0,
            "BehaviorParam_PC row {id} is not an attack"
        );
        let atk = i32::try_from(b.int(row, "refId")?).context("refId")?;
        let a = &self.attack;
        let arow = a
            .row_of(atk)
            .with_context(|| format!("AtkParam_Pc has no row {atk} (BehaviorParam_PC row {id})"))?;
        Ok(Behavior {
            stamina: b.int(row, "stamina")?,
            mult: a.int(arow, "atkPhysCorrection")? as f32 / 100.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_the_shortsword_and_its_moves_read_as_measured() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let p = load(Path::new(&dir)).unwrap();
        assert_eq!(
            p.weapon(200_000).unwrap(),
            Weapon {
                motion_category: 23,
                variation: 2300
            }
        );
        assert!(p.weapon(1).is_err());
        // (judge, stamina, damage multiplier): the spec's table.
        for (judge, stamina, mult) in [
            (0, 20, 1.00),
            (10, 20, 1.00),
            (20, 20, 1.00),
            (100, 45, 1.35),
            (110, 35, 1.25),
            (540, 20, 0.90),
        ] {
            let b = p.behavior(2300, judge).unwrap();
            assert_eq!(b.stamina, stamina, "judge {judge}");
            assert!((b.mult - mult).abs() < 1.0e-6, "judge {judge}: {}", b.mult);
        }
        assert!(p.behavior(2300, 12_345).is_err(), "no such judge");
    }
}
```

- [ ] **Step 4: `moveset.rs`**

**Create** `crates/dsr-cache/src/moveset.rs`:
```rust
//! The moves of a weapon class: which Dark Souls animation is which move. Hand-made, like the bone
//! map: the animation id scheme is `a<motion category>_<hand + action>` (hand 3000 one-handed;
//! action 000-002 the light chain, 300/301/310 heavy, 900 after a roll).

use anyhow::{ensure, Result};

use crate::params::Params;

/// One move: the name the game's move table uses (`r1_1`: the first light hit) and its animation.
pub struct Move {
    pub name: &'static str,
    pub clip: &'static str,
}

/// A weapon class: its motion category and behavior variation in the weapon params, a sample
/// weapon row of it, and its moves.
pub struct Class {
    pub name: &'static str,
    /// An `EquipParamWeapon` row of the class (the Shortsword).
    pub weapon: i32,
    pub motion_category: i64,
    pub variation: i64,
    pub moves: &'static [Move],
}

/// The straight swords, one-handed: light chain `3000`-`3002`, heavy `3300` and its follow-up
/// `3310` (`3301` shares `3300`'s judge and hit window and is not used), and the attack out of a
/// roll `3900`.
pub const STRAIGHT_SWORD: Class = Class {
    name: "straight_sword",
    weapon: 200_000,
    motion_category: 23,
    variation: 2300,
    moves: &[
        Move {
            name: "r1_1",
            clip: "a23_3000",
        },
        Move {
            name: "r1_2",
            clip: "a23_3001",
        },
        Move {
            name: "r1_3",
            clip: "a23_3002",
        },
        Move {
            name: "r2_1",
            clip: "a23_3300",
        },
        Move {
            name: "r2_2",
            clip: "a23_3310",
        },
        Move {
            name: "roll_attack",
            clip: "a23_3900",
        },
    ],
};

impl Class {
    /// The sample weapon row says what this record says, so a class recorded wrong cannot pass.
    pub fn check(&self, params: &Params) -> Result<()> {
        let w = params.weapon(self.weapon)?;
        ensure!(
            w.motion_category == self.motion_category && w.variation == self.variation,
            "weapon {} has motion category {} and behavior variation {}; the {} record says {} and {}",
            self.weapon,
            w.motion_category,
            w.variation,
            self.name,
            self.motion_category,
            self.variation
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_straight_sword_names_six_distinct_moves() {
        let c = &STRAIGHT_SWORD;
        assert_eq!(c.moves.len(), 6);
        let mut names: Vec<_> = c.moves.iter().map(|m| m.name).collect();
        let mut clips: Vec<_> = c.moves.iter().map(|m| m.clip).collect();
        names.sort_unstable();
        names.dedup();
        clips.sort_unstable();
        clips.dedup();
        assert_eq!((names.len(), clips.len()), (6, 6));
        // The animation set is the class's motion category.
        let set = format!("a{}_", c.motion_category);
        assert!(c.moves.iter().all(|m| m.clip.starts_with(&set)));
    }

    #[test]
    fn install_the_class_record_agrees_with_the_weapon_row() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let p = crate::params::load(std::path::Path::new(&dir)).unwrap();
        STRAIGHT_SWORD.check(&p).unwrap();
        let wrong = Class {
            variation: 2301,
            ..STRAIGHT_SWORD
        };
        let err = wrong.check(&p).unwrap_err();
        assert!(err.to_string().contains("2301"), "{err}");
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -j 4 -p dsr-cache moveset params`
Expected: PASS (the install tests print "skipped").
Run: `DSR_DIR='C:/Spel/Dark Souls Remastered' cargo test -j 4 -p dsr-cache install_the -- --nocapture`
Expected: PASS (both install tests).

- [ ] **Step 6: Checkpoint**

Run `cargo fmt --all -- --check && cargo test -j 4 --workspace`. Expected: green. Clippy waits for Task 5: `params.rs` and `moveset.rs` are unused until then and rustc would flag them as dead code.

---

### Task 5: `cache.rs`: extract the roll and the six moves

**Files:**
- Modify (rewrite): `crates/dsr-cache/src/cache.rs`

- [ ] **Step 1: Write the file**

**Create** `crates/dsr-cache/src/cache.rs`:
```rust
//! `extract`: decode the roll and the straight sword's moves, retarget them onto the human, and
//! write `cache/dsr/`: per clip `human_male/<clip>.wscl` (WoW bones, local) and
//! `c0000/<clip>.wscl` (DS bones, model space, Bevy axes, WoW scale), then `manifest.txt` last, so
//! a cache with a manifest is a whole one.

use std::fmt::Write as _;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{bail, ensure, Context, Result};
use dsr_formats::hkx::skeleton::Skeleton;
use glam::{Quat, Vec3};

use crate::basis::Basis;
use crate::bonemap::{self, Rig};
use crate::dsr::Player;
use crate::retarget::Retargeted;
use crate::wow::WowSkel;
use crate::{dsr, moveset, params, retarget, roll, wow};

pub const MANIFEST_VERSION: u32 = 2;
/// The no-tearing criterion (spec, success criterion 2), as drift from the bind angle.
pub const MAX_ERROR_DEG: f32 = 15.0;
/// The worst a direction across the body (hip axis, spine base) may drift from its bind angle
/// to its DS counterpart.
pub const MAX_CROSS_DEG: f32 = 15.0;
/// The worst the WoW feet may float above or sink below the DS feet on contact frames (yd).
pub const MAX_FOOT_GAP: f32 = 0.08;
/// The most the blade axis may differ between two frames, or two moves (deg): it is a constant
/// by construction.
pub const MAX_GRIP_DRIFT_DEG: f32 = 1.0;

fn source_line(path: &Path) -> Result<String> {
    let meta = std::fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    let mtime = meta.modified()?.duration_since(UNIX_EPOCH)?.as_secs();
    Ok(format!(
        "source {} {} {}",
        meta.len(),
        mtime,
        path.display()
    ))
}

fn write_clip(path: &Path, clip: &wscl::Clip) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    clip.write(&mut f)?;
    // Flush explicitly: BufWriter's drop swallows write errors.
    std::io::Write::flush(&mut f)?;
    Ok(())
}

/// Writes a clip's two files: on the WoW rig and on the DS skeleton.
fn write_pair(out: &Path, name: &str, r: &Retargeted) -> Result<()> {
    write_clip(&out.join("human_male").join(format!("{name}.wscl")), &r.wow)?;
    write_clip(&out.join("c0000").join(format!("{name}.wscl")), &r.ds)
}

/// One clip, extracted.
struct Extracted {
    r: Retargeted,
    basis: Basis,
}

/// Decodes a clip, retargets it onto the human, checks it, and marks its hit windows.
fn extract_clip(player: &Player, skel: &WowSkel, name: &str) -> Result<Extracted> {
    let clip = dsr::decode(player, name)?;
    let rig = &bonemap::HUMAN_MALE_RIG;
    let mut r = retarget::retarget(&player.skeleton, &clip, skel, rig, player.flags(name))?;
    if r.max_error_deg > MAX_ERROR_DEG {
        bail!(
            "{name}: a mapped bone drifts {:.1}° from its bind angle to DS (limit {MAX_ERROR_DEG})",
            r.max_error_deg
        );
    }
    if r.max_cross_deg > MAX_CROSS_DEG {
        bail!(
            "{name}: a direction across the body drifts {:.1}° from its bind angle to DS \
             (limit {MAX_CROSS_DEG})",
            r.max_cross_deg
        );
    }
    if r.max_foot_gap > MAX_FOOT_GAP {
        bail!(
            "{name}: on contact the feet are {:.3} yd off the DS feet's height \
             (limit {MAX_FOOT_GAP})",
            r.max_foot_gap
        );
    }
    let hits = player.hits(name);
    r.wow.hits = hits.clone();
    r.ds.hits = hits;
    let basis = retarget::basis_for(&player.skeleton, &clip, skel, &rig.legs)?;
    Ok(Extracted { r, basis })
}

/// The DS blade's axis in the retargeted WoW right-hand bone's local frame. The weapon bone sits
/// unrotated in the DS hand and the retarget carries the hand's rotation, so the vector is the
/// same in every frame; a clip where it drifts more than [`MAX_GRIP_DRIFT_DEG`] is an error.
fn grip_axis(e: &Extracted, ds: &Skeleton, rig: &Rig) -> Result<Vec3> {
    let weapon = ds
        .bone(bonemap::DS_WEAPON)
        .context("the DS weapon bone is missing")?;
    let hand = rig
        .pairs
        .iter()
        .find(|p| p.ds == bonemap::DS_HAND)
        .map(|p| p.wow)
        .context("the bone map has no DS hand")?;
    let blade = e.basis.dir(bonemap::DS_BLADE_AXIS);
    let parents = &e.r.wow.parents;
    // The hand and its ancestors, root first.
    let mut chain = vec![hand];
    while let Ok(p) = usize::try_from(parents[*chain.last().expect("never empty")]) {
        ensure!(p < parents[..].len(), "bone {p} is past the skeleton");
        chain.push(p);
    }
    chain.reverse();
    let mut first: Option<Vec3> = None;
    for (f, frame) in e.r.wow.frames.iter().enumerate() {
        let world = chain.iter().fold(Quat::IDENTITY, |w, &b| {
            (w * Quat::from_array(frame[b].r)).normalize()
        });
        let ds_weapon = Quat::from_array(e.r.ds.frames[f][weapon].r).normalize();
        let local = world.inverse() * (ds_weapon * blade);
        match first {
            None => first = Some(local),
            Some(a) => ensure!(
                a.angle_between(local).to_degrees() < MAX_GRIP_DRIFT_DEG,
                "frame {f}: the blade axis in the hand drifts {:.2}°",
                a.angle_between(local).to_degrees()
            ),
        }
    }
    first.context("the clip has no frames")
}

/// The worst checks over every extracted clip.
#[derive(Default)]
struct Worst {
    error: f32,
    cross: f32,
    foot: f32,
    foot_all: f32,
}

impl Worst {
    fn add(&mut self, r: &Retargeted) {
        self.error = self.error.max(r.max_error_deg);
        self.cross = self.cross.max(r.max_cross_deg);
        self.foot = self.foot.max(r.max_foot_gap);
        self.foot_all = self.foot_all.max(r.max_foot_gap_all);
    }
}

/// One console line for an extracted clip.
fn summary(name: &str, r: &Retargeted) -> String {
    let rm = r.wow.root_motion.last().copied().unwrap_or([0.0; 4]);
    let contact = if r.contact_frames == 0 {
        "no contact frames".to_string()
    } else {
        format!(
            "{:.3} yd over {} contact frames",
            r.max_foot_gap, r.contact_frames
        )
    };
    format!(
        "{name}: {} frames at {:.1} fps ({:.3} s), root motion ({:.2}, {:.2}, {:.2}) yd, yaw {:.1}°, \
         invulnerable {:?}, hit {:?}, max direction drift {:.2}°, max cross drift {:.2}°, \
         max foot gap {contact} ({:.3} yd over all frames)",
        r.wow.frames.len(),
        r.wow.fps,
        r.wow.duration(),
        rm[0],
        rm[1],
        rm[2],
        rm[3].to_degrees(),
        r.wow.windows(wscl::FLAG_INVINCIBLE).collect::<Vec<_>>(),
        r.wow
            .hits
            .iter()
            .map(|h| (h.start, h.end))
            .collect::<Vec<_>>(),
        r.max_error_deg,
        r.max_cross_deg,
        r.max_foot_gap_all
    )
}

pub fn extract(dsr_dir: &Path, wow_dir: &Path, out: &Path, roll_name: Option<&str>) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    let skel = wow::human_male(wow_dir)?;
    let params = params::load(dsr_dir)?;
    let class = &moveset::STRAIGHT_SWORD;
    class.check(&params)?;
    let roll_name = match roll_name {
        Some(n) => n.to_string(),
        None => roll::pick(&roll::candidates(&player)?)
            .context("no roll candidate; run `list` and pass --roll")?
            .name
            .clone(),
    };

    let mut worst = Worst::default();
    let rolled = extract_clip(&player, &skel, &roll_name)?;
    worst.add(&rolled.r);
    write_pair(out, &roll_name, &rolled.r)?;
    println!("{}", summary(&roll_name, &rolled.r));

    let rig = &bonemap::HUMAN_MALE_RIG;
    let mut move_lines = String::new();
    let mut grip: Option<(Vec3, Vec3)> = None;
    for m in class.moves {
        let e = extract_clip(&player, &skel, m.clip)
            .with_context(|| format!("move {} ({})", m.name, m.clip))?;
        worst.add(&e.r);
        let judge = e
            .r
            .wow
            .hits
            .first()
            .map(|h| h.judge)
            .with_context(|| format!("{} has no hit window", m.clip))?;
        let b = params
            .behavior(class.variation, judge)
            .with_context(|| format!("move {} ({}), judge {judge}", m.name, m.clip))?;
        let hand = grip_axis(&e, &player.skeleton, rig).with_context(|| m.clip.to_string())?;
        let weapon = e.basis.dir(bonemap::DS_BLADE_AXIS);
        match grip {
            None => grip = Some((hand, weapon)),
            Some((h, w)) => ensure!(
                h.angle_between(hand).to_degrees() < MAX_GRIP_DRIFT_DEG
                    && w.angle_between(weapon).to_degrees() < MAX_GRIP_DRIFT_DEG,
                "{}: its blade axis differs from the first move's",
                m.clip
            ),
        }
        write_pair(out, m.clip, &e.r)?;
        writeln!(
            move_lines,
            "move {} {} judge {judge} stamina {} mult {:.2}",
            m.name, m.clip, b.stamina, b.mult
        )?;
        println!(
            "{}  [{}: judge {judge}, stamina {}, x{:.2}]",
            summary(m.clip, &e.r),
            m.name,
            b.stamina,
            b.mult
        );
    }
    let (hand, weapon) = grip.context("the class has no moves")?;

    let mut m = String::new();
    writeln!(m, "wscl-cache {MANIFEST_VERSION}")?;
    writeln!(m, "extractor {}", wscl::EXTRACTOR_VERSION)?;
    writeln!(m, "roll {roll_name}")?;
    writeln!(m, "class {}", class.name)?;
    m.push_str(&move_lines);
    writeln!(
        m,
        "grip {:.5} {:.5} {:.5} {:.5} {:.5} {:.5}",
        hand.x, hand.y, hand.z, weapon.x, weapon.y, weapon.z
    )?;
    writeln!(m, "max-error-deg {:.3}", worst.error)?;
    writeln!(m, "max-cross-deg {:.3}", worst.cross)?;
    writeln!(m, "max-foot-gap {:.4}", worst.foot)?;
    writeln!(m, "max-foot-gap-all {:.4}", worst.foot_all)?;
    for path in dsr::animation_archives(dsr_dir)? {
        writeln!(m, "{}", source_line(&path)?)?;
    }
    writeln!(
        m,
        "{}",
        source_line(&dsr_dir.join("chr").join("c0000.anibnd.dcx"))?
    )?;
    for file in [
        "param/GameParam/GameParam.parambnd.dcx",
        "paramdef/paramdef.paramdefbnd.dcx",
    ] {
        writeln!(m, "{}", source_line(&dsr_dir.join(file))?)?;
    }
    std::fs::write(out.join("manifest.txt"), m)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("dsr-cache-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn written_clip_reads_back() {
        let dir = temp_dir("clip");
        let clip = wscl::Clip {
            name: "a00_test".into(),
            fps: 30.0,
            model_space: false,
            parents: vec![-1, 0],
            driven: vec![true, false],
            frames: vec![
                vec![wscl::Trs::IDENTITY; 2],
                vec![
                    wscl::Trs {
                        t: [1.0, 2.0, 3.0],
                        ..wscl::Trs::IDENTITY
                    };
                    2
                ],
            ],
            root_motion: vec![[0.0; 4], [0.0, 0.0, -1.5, 0.25]],
            flags: vec![wscl::FlagWindow {
                flag: wscl::FLAG_INVINCIBLE,
                start: 0.0,
                end: 0.033,
            }],
            hits: vec![wscl::HitWindow {
                judge: 7,
                start: 0.01,
                end: 0.02,
            }],
        };
        let path = dir.join("nested").join("a00_test.wscl");
        write_clip(&path, &clip).unwrap();
        let back = wscl::Clip::read(&mut std::fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(back, clip);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn source_line_reports_length_and_path() {
        let dir = temp_dir("source");
        let path = dir.join("blob.bin");
        std::fs::write(&path, [7u8; 123]).unwrap();
        let line = source_line(&path).unwrap();
        let mut parts = line.splitn(4, ' ');
        assert_eq!(parts.next(), Some("source"));
        assert_eq!(parts.next(), Some("123"));
        assert!(parts.next().unwrap().parse::<u64>().is_ok());
        assert_eq!(parts.next(), Some(path.display().to_string().as_str()));
        assert!(source_line(&dir.join("missing")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn install_the_moves_match_the_data_table() {
        let (Ok(dsr_dir), Ok(wow_dir)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("DSR_DIR and WOW_DATA not both set; skipped");
            return;
        };
        let player = dsr::load(Path::new(&dsr_dir)).unwrap();
        assert_eq!(player.skeleton.bone(bonemap::DS_HAND), Some(45));
        assert_eq!(
            player.skeleton.bone(bonemap::DS_WEAPON),
            Some(56),
            "the game names the DS weapon bone by this index"
        );
        let skel = wow::human_male(Path::new(&wow_dir)).unwrap();
        let params = params::load(Path::new(&dsr_dir)).unwrap();
        // clip, judge, length s, hit window s, f4 / f26 / f11 s, lunge yd, stamina, damage x.
        #[allow(clippy::type_complexity)]
        let table: [(&str, i32, f32, (f32, f32), [f32; 3], f32, i64, f32); 6] = [
            ("a23_3000", 0, 2.500, (0.533, 0.667), [0.767, 0.900, 1.033], 1.98, 20, 1.00),
            ("a23_3001", 10, 2.233, (0.467, 0.567), [0.733, 0.900, 1.000], 1.42, 20, 1.00),
            ("a23_3002", 20, 2.500, (0.467, 0.600), [0.767, 0.900, 1.033], 1.98, 20, 1.00),
            ("a23_3300", 100, 2.533, (0.733, 0.867), [1.300, 1.300, 1.367], 1.98, 45, 1.35),
            ("a23_3310", 110, 1.900, (0.333, 0.467), [0.933, 0.933, 1.200], 0.94, 35, 1.25),
            ("a23_3900", 540, 2.100, (0.233, 0.333), [0.700, 0.767, 0.867], 1.42, 20, 0.90),
        ];
        let rig = &bonemap::HUMAN_MALE_RIG;
        for (clip, judge, len, hit, cancels, lunge, stamina, mult) in table {
            let e = extract_clip(&player, &skel, clip).unwrap();
            let w = &e.r.wow;
            assert!((w.duration() - len).abs() < 0.002, "{clip}: {}", w.duration());
            let h = w.hits.first().unwrap_or_else(|| panic!("{clip}: no hit window"));
            assert_eq!(h.judge, judge, "{clip}");
            assert!((h.start - hit.0).abs() < 0.001, "{clip}: hit start {}", h.start);
            assert!((h.end - hit.1).abs() < 0.001, "{clip}: hit end {}", h.end);
            for (flag, want) in [
                (wscl::FLAG_CHAIN, cancels[0]),
                (wscl::FLAG_EVADE, cancels[1]),
                (wscl::FLAG_MOVE, cancels[2]),
            ] {
                let got = w
                    .windows(flag)
                    .map(|(s, _)| s)
                    .fold(f32::INFINITY, f32::min);
                assert!((got - want).abs() < 0.001, "{clip}: flag {flag} opens at {got}");
            }
            let z = -w.root_motion.last().unwrap()[2];
            assert!((z - lunge).abs() < 0.02 * lunge, "{clip}: lunge {z} yd");
            let b = params.behavior(2300, judge).unwrap();
            assert_eq!(b.stamina, stamina, "{clip}");
            assert!((b.mult - mult).abs() < 1.0e-6, "{clip}: {}", b.mult);
            grip_axis(&e, &player.skeleton, rig).unwrap();
        }
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -j 4 -p dsr-cache cache`
Expected: PASS (the install test prints "skipped").
Run: `DSR_DIR='C:/Spel/Dark Souls Remastered' WOW_DATA='C:/Spel/World of Warcraft 1.12/Data' cargo test -j 4 -p dsr-cache install_the_moves -- --nocapture`
Expected: PASS: every move's length, hit window, three cancel times, lunge (±2 %), stamina and multiplier equal the table, and the blade axis is constant over each clip. If a number is off by a hair, the table (Facts) and this test are the same data: fix the tolerance only if the difference is under a frame (0.033 s) and the spec says so.

- [ ] **Step 3: Extract the cache**

Run: `cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- extract`
Expected: seven lines (the roll, then the six moves), each ending `[<move>: judge <n>, stamina <n>, x<m>]` for the moves, no error. Then:

```bash
cat cache/dsr/manifest.txt | grep -E "^(wscl-cache|extractor|roll|class|move|grip)"
ls cache/dsr/human_male cache/dsr/c0000
```
Expected: `wscl-cache 2`, `extractor 3`, `roll a00_0710`, `class straight_sword`, six `move` lines (`move r1_1 a23_3000 judge 0 stamina 20 mult 1.00` … `move roll_attack a23_3900 judge 540 stamina 20 mult 0.90`), one `grip` line of six numbers; seven `.wscl` files in each folder.

- [ ] **Step 4: Checkpoint**

Run the gates. Expected: all green. (The game still loads the old cache format until Task 9: it will log one warning and keep the procedural roll. That is expected.)

---

### Task 6: Split `combat.rs` (no change in behaviour)

`combat.rs` is 1,500 lines. Its pure action machine (constants, types, `step_action`, `start`) is what the moveset changes; move it into its own file first, code unchanged, so the real change in Task 7 stays reviewable.

**Files:**
- Move: `crates/wow-souls/src/combat.rs` → `crates/wow-souls/src/combat/mod.rs`
- Create: `crates/wow-souls/src/combat/action.rs`

- [ ] **Step 1: Write the split script (outside the repo)**

**Create** `scratch:split_combat.py`:
```python
"""Splits crates/wow-souls/src/combat.rs into combat/mod.rs and combat/action.rs, moving code
unchanged: the pure action machine (its constants, types, `step_action`, `start`) and its tests.

Usage: python split_combat.py <src dir>      (the directory holding combat.rs)
Writes <src dir>/combat/mod.rs and <src dir>/combat/action.rs and removes combat.rs.
Every cut is asserted: a marker that is missing or not unique stops the script before writing.
"""
import os
import sys

src_dir = sys.argv[1]
text = open(os.path.join(src_dir, "combat.rs"), encoding="utf-8").read()


def cut(text, start_marker, end_marker):
    """Removes and returns the text from start_marker up to end_marker (exclusive)."""
    assert text.count(start_marker) == 1, f"start marker not unique: {start_marker!r}"
    a = text.index(start_marker)
    assert end_marker in text[a:], f"end marker missing: {end_marker!r}"
    b = text.index(end_marker, a)
    return text[:a] + text[b:], text[a:b]


moved = []  # pieces of production code, in the order they go into action.rs

# 1. The swing windows and the buffer: three consts with their doc, one const.
text, piece = cut(
    text,
    "/// Swing windows as fractions of its length",
    "/// The forward lunge speed while winding up and swinging (yd/s).",
)
moved.append(piece)
text, piece = cut(
    text,
    "/// How long a press waits for the action to accept it (s).",
    "pub const HEALTH_MAX: f32 = 100.0;",
)
moved.append(piece)
# 2. The roll push threshold.
text, piece = cut(
    text,
    "/// A stick push over this aims a roll; under it the evade is a backstep.",
    "/// An evade's shape: times (s), distance (yd) and stamina.",
)
moved.append(piece)
# 3. EvadeSpec, ROLL, BACKSTEP.
text, piece = cut(
    text,
    "/// An evade's shape: times (s), distance (yd) and stamina.",
    "/// Stamina regained per second once regeneration runs.",
)
moved.append(piece)
# 4. The machine: SwingPhase ... `start`.
text, piece = cut(
    text,
    "/// Where a swing is.",
    "/// A scripted press: at `at` seconds, then every `every`.",
)
moved.append(piece)

# 5. The machine's tests, by name, with their helpers.
tests_start = text.index("#[cfg(test)]\nmod tests {\n")
head, tests = text[:tests_start], text[tests_start:]
body_start = tests.index("mod tests {\n") + len("mod tests {\n")
body = tests[body_start:]
assert body.rstrip().endswith("}"), "tests module does not end with a brace"
body = body.rstrip()[:-1]  # drop the module's closing brace


def take_item(body, header):
    """Removes and returns an item at 4-space indent starting with `header`, with the doc comment
    and attribute lines above it."""
    assert body.count(header) == 1, f"item not unique: {header!r}"
    at = body.index(header)
    i = at
    while True:
        j = body.rfind("\n", 0, i - 1)
        line = body[j + 1 : i - 1]
        if line.lstrip().startswith(("///", "#[")):
            i = j + 1
        else:
            break
    first_line_end = body.index("\n", at)
    if body[at:first_line_end].rstrip().endswith(";"):
        end = first_line_end + 1
    else:
        end = body.index("\n    }\n", at) + len("\n    }\n")
    return body[:i] + body[end:].lstrip("\n"), body[i:end]


machine_tests = [
    "fn the_swing_runs_its_phases_in_order(",
    "fn a_press_swings_once_and_ends(",
    "fn a_press_in_the_wind_up_is_buffered_into_a_chain(",
    "fn a_press_too_early_expires(",
    "fn no_stamina_no_swing_and_airborne_no_swing(",
    "fn a_pushed_evade_rolls_along_the_stick_and_a_still_one_backsteps(",
    "fn a_roll_runs_its_length_and_ends(",
    "fn a_ds_roll_takes_its_shape_from_the_input_and_does_not_tumble(",
    "fn a_pushed_stick_cancels_an_evade_only_from_its_cancel_window(",
    "fn the_roll_covers_its_distance_fast_first_then_stops(",
    "fn the_roll_is_invulnerable_only_inside_its_window(",
    "fn the_somersault_turns_once_and_only_in_a_roll(",
    "fn a_roll_cancels_a_swing_only_from_its_recovery(",
    "fn an_attack_buffered_in_a_roll_starts_when_it_has_travelled(",
    "fn a_stagger_stops_the_swing_holds_presses_and_wears_off(",
    "fn the_dead_take_no_action(",
]
helpers = [
    "const LEN: f32 = 0.8;",
    "fn input(",
    "fn run_with(",
    "fn run(",
]
moved_tests = []
for name in helpers + machine_tests:
    body, item = take_item(body, "    " + name)
    moved_tests.append(item)

# `DT` and `full()` are used by both halves: they stay here and are copied there.
mod_rs = head + "#[cfg(test)]\nmod tests {\n" + body.rstrip() + "\n}\n"

action_header = """//! The pure action machine: what a body is committed to (a swing, an evade, a stagger, death),
//! which presses it accepts when, and how a press becomes a decision. No Bevy systems here: the
//! caller ([`super`]) feeds it a frame of input and carries out the decision.

use bevy::prelude::*;

use super::Stamina;

"""
action_rs = action_header + "".join(moved)
action_rs = action_rs.rstrip() + "\n\n"
action_rs += (
    "#[cfg(test)]\nmod tests {\n    use super::*;\n    use crate::combat::STAMINA_MAX;\n\n"
    "    const DT: f32 = 1.0 / 60.0;\n\n"
    "    fn full() -> Stamina {\n        Stamina::full(STAMINA_MAX)\n    }\n\n"
    + "\n".join(moved_tests)
    + "}\n"
)

# mod.rs: declare and re-export the machine.
anchor = "use crate::actor::ModelFrame;"
assert mod_rs.count(anchor) == 1
mod_rs = mod_rs.replace(
    anchor,
    "mod action;\npub use action::*;\n"
    "// Bevy's prelude has a `Press` too (picking): name ours, or the two globs clash.\n"
    "use action::Press;\n\n" + anchor,
)

out = os.path.join(src_dir, "combat")
os.makedirs(out, exist_ok=True)
open(os.path.join(out, "mod.rs"), "w", encoding="utf-8", newline="\n").write(mod_rs)
open(os.path.join(out, "action.rs"), "w", encoding="utf-8", newline="\n").write(action_rs)
os.remove(os.path.join(src_dir, "combat.rs"))
print(f"combat/mod.rs {mod_rs.count(chr(10))} lines, combat/action.rs {action_rs.count(chr(10))} lines")
```

- [ ] **Step 2: Run it**

Run (from `C:\Spel\wow-souls`; `<scratch>` is the scratchpad directory the script was written to):
```bash
python "<scratch>/split_combat.py" crates/wow-souls/src && cargo fmt --all
```
Expected: `combat/mod.rs 758 lines, combat/action.rs 723 lines` (the numbers may differ by a few lines if `combat.rs` changed), `combat.rs` gone.

- [ ] **Step 3: Run the tests**

Run: `cargo test -j 4 -p wow-souls`
Expected: PASS, the same 157 tests as before the split. If the compile fails on a missing import, the fix is a `use` line in `combat/action.rs` or `combat/mod.rs`, nothing else.

- [ ] **Step 4: Checkpoint**

Run the gates. Expected: all green.

---

### Task 7: `SwingSpec`, `MoveKind`, `Moveset`: timing as data

The action machine reads each swing's windows from a `SwingSpec` instead of fractions of a length. The procedural swing is the same struct built from today's fractions, so the machine has one code path and a test pins the fallback to today's numbers. Heavy presses, the R2 chain and the roll attack are in the machine; the game only feeds it a Dark Souls moveset in Task 9, so after this task the game plays exactly as before.

**Files:**
- Modify: `crates/wow-souls/src/combat/action.rs`
- Modify: `crates/wow-souls/src/combat/mod.rs`
- Modify: `crates/wow-souls/src/hits.rs`

- [ ] **Step 1: Write the failing tests**

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
    /// One frame's input: grounded, the stick still, facing `-Z`.
    fn input(press: Option<Press>, grounded: bool) -> StepInput {
        StepInput {
            press,
            grounded,
            swing_len: LEN,
            stick: Vec3::ZERO,
```
New:
```rust
    /// One frame's input: grounded, the stick still, facing `-Z`, today's swing.
    fn input(press: Option<Press>, grounded: bool) -> StepInput {
        StepInput {
            press,
            grounded,
            moves: Moveset::procedural(LEN),
            stick: Vec3::ZERO,
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
    /// Runs the machine for `secs` with presses at the given times and a held stick; returns the
    /// decisions.
    fn run_with(
        a: &mut Action,
        presses: &[(f32, Press)],
        stick: Vec3,
        secs: f32,
    ) -> Vec<(f32, Decision)> {
        let s = full();
        let mut out = Vec::new();
        let mut t = 0.0;
        while t < secs {
            let press = presses
                .iter()
                .find(|&&(p, _)| p >= t && p < t + DT)
                .map(|&(_, k)| k);
            let d = step_action(
                a,
                &s,
                StepInput {
                    stick,
                    ..input(press, true)
                },
                DT,
            );
            if d != Decision::None {
                out.push((t, d));
            }
            t += DT;
        }
        out
    }
```
New:
```rust
    /// The Dark Souls straight sword's table (the spec's), as a moveset.
    fn ds_moves() -> Moveset {
        let s = |duration, hit, chain_from, evade_from, move_from, cost, mult| SwingSpec {
            duration,
            hit,
            chain_from,
            evade_from,
            move_from,
            cost,
            mult,
        };
        Moveset {
            r1: [
                s(2.500, (0.533, 0.667), 0.767, 0.900, 1.033, 20.0, 1.00),
                s(2.233, (0.467, 0.567), 0.733, 0.900, 1.000, 20.0, 1.00),
                s(2.500, (0.467, 0.600), 0.767, 0.900, 1.033, 20.0, 1.00),
            ],
            r2: Some([
                s(2.533, (0.733, 0.867), 1.300, 1.300, 1.367, 45.0, 1.35),
                s(1.900, (0.333, 0.467), 0.933, 0.933, 1.200, 35.0, 1.25),
            ]),
            roll_attack: Some(s(2.100, (0.233, 0.333), 0.700, 0.767, 0.867, 20.0, 0.90)),
            ds: true,
        }
    }

    fn swing(kind: MoveKind, chained: bool) -> Decision {
        Decision::Swing { kind, chained }
    }

    /// Runs the machine on `moves` for `secs` with presses at the given times and a held stick;
    /// returns the decisions with their times.
    fn run_in(
        moves: Moveset,
        a: &mut Action,
        presses: &[(f32, Press)],
        stick: Vec3,
        secs: f32,
    ) -> Vec<(f32, Decision)> {
        let s = full();
        let mut out = Vec::new();
        let mut t = 0.0;
        while t < secs {
            let press = presses
                .iter()
                .find(|&&(p, _)| p >= t && p < t + DT)
                .map(|&(_, k)| k);
            let d = step_action(
                a,
                &s,
                StepInput {
                    stick,
                    moves,
                    ..input(press, true)
                },
                DT,
            );
            if d != Decision::None {
                out.push((t, d));
            }
            t += DT;
        }
        out
    }

    /// The same on today's procedural swing.
    fn run_with(
        a: &mut Action,
        presses: &[(f32, Press)],
        stick: Vec3,
        secs: f32,
    ) -> Vec<(f32, Decision)> {
        run_in(Moveset::procedural(LEN), a, presses, stick, secs)
    }
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
    fn the_swing_runs_its_phases_in_order() {
        assert_eq!(swing_phase(0.0, LEN), Some(SwingPhase::Startup));
        assert_eq!(swing_phase(0.3 * LEN, LEN), Some(SwingPhase::Active));
        assert_eq!(swing_phase(0.6 * LEN, LEN), Some(SwingPhase::Recovery));
        assert_eq!(swing_phase(LEN, LEN), None);
    }
```
New:
```rust
    fn the_swing_runs_its_phases_in_order() {
        let at = |t: f32| Swing {
            t,
            kind: MoveKind::R1(0),
            spec: SwingSpec::procedural(LEN),
            ds: false,
            serial: 1,
        };
        assert_eq!(at(0.0).phase(), Some(SwingPhase::Startup));
        assert_eq!(at(0.3 * LEN).phase(), Some(SwingPhase::Active));
        assert_eq!(at(0.6 * LEN).phase(), Some(SwingPhase::Recovery));
        assert_eq!(at(LEN).phase(), None);
    }
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
        assert_eq!(d[0].1, Decision::Swing(0));
```
New:
```rust
        assert_eq!(d[0].1, swing(MoveKind::R1(0), false));
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
        let swings: Vec<_> = d
            .iter()
            .filter(|(_, d)| matches!(d, Decision::Swing(_)))
            .collect();
        assert_eq!(swings.len(), 2, "{d:?}");
        assert_eq!(swings[1].1, Decision::Swing(1));
```
New:
```rust
        let swings: Vec<_> = d
            .iter()
            .filter(|(_, d)| matches!(d, Decision::Swing { .. }))
            .collect();
        assert_eq!(swings.len(), 2, "{d:?}");
        assert_eq!(swings[1].1, swing(MoveKind::R1(1), true));
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
        let swings = d
            .iter()
            .filter(|(_, d)| matches!(d, Decision::Swing(_)))
            .count();
```
New:
```rust
        let swings = d
            .iter()
            .filter(|(_, d)| matches!(d, Decision::Swing { .. }))
            .count();
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
        assert_eq!(
            step(&mut a, Some(Press::Attack), Vec3::X),
            Decision::Swing(0)
        );
```
New:
```rust
        assert_eq!(
            step(&mut a, Some(Press::Attack), Vec3::X),
            swing(MoveKind::R1(0), false)
        );
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
            .find(|(_, d)| *d == Decision::Swing(0))
```
New:
```rust
            .find(|(_, d)| *d == swing(MoveKind::R1(0), false))
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
        assert_eq!(
            step_action(&mut a, &s, input(Some(Press::Attack), true), DT),
            Decision::Swing(0)
        );
        a.stagger(0.5);
```
New:
```rust
        assert_eq!(
            step_action(&mut a, &s, input(Some(Press::Attack), true), DT),
            swing(MoveKind::R1(0), false)
        );
        a.stagger(0.5);
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
        assert!(a.is_dead() && a.owns_body());
    }
}
```
New:
```rust
        assert!(a.is_dead() && a.owns_body());
    }

    #[test]
    fn the_procedural_swing_keeps_todays_windows() {
        let s = SwingSpec::procedural(LEN);
        assert_eq!(s.hit, (ACTIVE_FROM * LEN, ACTIVE_TO * LEN));
        assert_eq!(s.chain_from, CANCEL_FROM * LEN);
        assert_eq!(s.evade_from, ACTIVE_TO * LEN);
        assert_eq!(s.move_from, LEN, "a pushed stick never cuts it short");
        assert_eq!((s.cost, s.mult), (ATTACK_COST, 1.0));
        let m = Moveset::procedural(LEN);
        assert!(!m.has_heavy() && m.roll_attack.is_none() && !m.ds);
    }

    #[test]
    fn the_light_chain_runs_three_hits_and_loops() {
        let mut a = Action::default();
        // A press every 0.9 s: each lands in the chain window of the swing before.
        let presses: Vec<_> = (0..5).map(|n| (n as f32 * 0.9, Press::Attack)).collect();
        let d = run_in(ds_moves(), &mut a, &presses, Vec3::ZERO, 4.6);
        let kinds: Vec<_> = d
            .iter()
            .filter_map(|(_, d)| match d {
                Decision::Swing { kind, chained } => Some((*kind, *chained)),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                (MoveKind::R1(0), false),
                (MoveKind::R1(1), true),
                (MoveKind::R1(2), true),
                (MoveKind::R1(0), true),
                (MoveKind::R1(1), true),
            ],
            "{d:?}"
        );
    }

    #[test]
    fn the_next_hit_waits_for_the_chain_flag() {
        let mut a = Action::default();
        // Pressed at 0.5 s into R1-1, the chain flag opens at 0.767 s: the buffer (0.35 s) holds it.
        let d = run_in(
            ds_moves(),
            &mut a,
            &[(0.0, Press::Attack), (0.5, Press::Attack)],
            Vec3::ZERO,
            2.0,
        );
        let second = d
            .iter()
            .find(|(_, d)| *d == swing(MoveKind::R1(1), true))
            .expect("the second hit lands");
        assert!((second.0 - 0.767).abs() < 2.0 * DT, "{d:?}");
    }

    #[test]
    fn a_swing_serial_counts_every_swing() {
        let mut a = Action::default();
        run_in(
            ds_moves(),
            &mut a,
            &[(0.0, Press::Attack), (0.9, Press::Attack)],
            Vec3::ZERO,
            1.2,
        );
        assert_eq!(a.swings, 2);
        let ActionState::Attack(s) = a.state else {
            panic!("a swing is running")
        };
        assert_eq!(s.serial, 2);
        assert!(s.ds);
    }

    #[test]
    fn heavy_starts_r2_and_then_its_follow_up_and_then_r2_again() {
        let mut a = Action::default();
        let presses = [
            (0.0, Press::Heavy),
            (1.4, Press::Heavy),
            (2.4, Press::Heavy),
        ];
        let d = run_in(ds_moves(), &mut a, &presses, Vec3::ZERO, 3.6);
        let kinds: Vec<_> = d
            .iter()
            .filter_map(|(_, d)| match d {
                Decision::Swing { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![MoveKind::R2(0), MoveKind::R2(1), MoveKind::R2(0)],
            "{d:?}"
        );
    }

    #[test]
    fn light_and_heavy_swap_by_restarting_the_other() {
        let mut a = Action::default();
        let presses = [
            (0.0, Press::Attack),
            (0.9, Press::Heavy),
            (2.4, Press::Attack),
        ];
        let d = run_in(ds_moves(), &mut a, &presses, Vec3::ZERO, 3.5);
        let kinds: Vec<_> = d
            .iter()
            .filter_map(|(_, d)| match d {
                Decision::Swing { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![MoveKind::R1(0), MoveKind::R2(0), MoveKind::R1(0)],
            "{d:?}"
        );
    }

    #[test]
    fn an_evade_waits_for_the_evade_flag() {
        // Pressed at 0.7 s into R1-1, the roll lands when the flag opens, at 0.9 s.
        let mut a = Action::default();
        let d = run_in(
            ds_moves(),
            &mut a,
            &[(0.0, Press::Attack), (0.7, Press::Evade)],
            Vec3::NEG_Z,
            1.5,
        );
        let roll = d
            .iter()
            .find(|(_, d)| *d == Decision::Evade(EvadeKind::Roll))
            .expect("the roll lands");
        assert!((roll.0 - 0.9).abs() < 2.0 * DT, "{d:?}");
        // Pressed at 0.5 s, the press is 0.4 s old by then, past the buffer: no roll.
        let mut a = Action::default();
        let d = run_in(
            ds_moves(),
            &mut a,
            &[(0.0, Press::Attack), (0.5, Press::Evade)],
            Vec3::NEG_Z,
            1.5,
        );
        assert!(
            d.iter().all(|(_, d)| !matches!(d, Decision::Evade(_))),
            "{d:?}"
        );
    }

    #[test]
    fn a_pushed_stick_ends_a_swing_only_from_its_movement_flag() {
        let s = full();
        let step = |a: &mut Action, press: Option<Press>, stick: Vec3| {
            step_action(
                a,
                &s,
                StepInput {
                    stick,
                    moves: ds_moves(),
                    ..input(press, true)
                },
                DT,
            )
        };
        let mut a = Action::default();
        assert_eq!(
            step(&mut a, Some(Press::Attack), Vec3::ZERO),
            swing(MoveKind::R1(0), false)
        );
        let mut t = 0.0;
        let mut ended = None;
        while t < 2.5 && ended.is_none() {
            t += DT;
            // Pushed from 0.5 s on: the swing holds until 1.033 s.
            let stick = if t >= 0.5 { Vec3::X } else { Vec3::ZERO };
            if step(&mut a, None, stick) == Decision::Ended {
                ended = Some(t);
            }
        }
        let ended = ended.expect("the push ends it");
        assert!((ended - 1.033).abs() < 2.0 * DT, "{ended}");
        // The procedural swing is never cut short by the stick.
        let mut a = Action::default();
        step_action(&mut a, &s, input(Some(Press::Attack), true), DT);
        for _ in 0..30 {
            let i = StepInput {
                stick: Vec3::X,
                ..input(None, true)
            };
            assert_eq!(step_action(&mut a, &s, i, DT), Decision::None);
        }
    }

    #[test]
    fn a_light_press_in_a_ds_roll_is_the_roll_attack() {
        let ds_roll = EvadeSpec {
            duration: 2.0,
            travel: 2.0,
            distance: 4.5,
            iframes: (0.0, 0.367),
            cancel_from: 1.1,
            cost: 20.0,
        };
        let s = full();
        let step = |a: &mut Action, press: Option<Press>| {
            step_action(
                a,
                &s,
                StepInput {
                    roll: ds_roll,
                    ds_roll: true,
                    stick: Vec3::NEG_Z,
                    moves: ds_moves(),
                    ..input(press, true)
                },
                DT,
            )
        };
        let mut a = Action::default();
        assert_eq!(
            step(&mut a, Some(Press::Evade)),
            Decision::Evade(EvadeKind::Roll)
        );
        let mut t = 0.0;
        let mut out = None;
        while t < 2.0 && out.is_none() {
            t += DT;
            let press = (1.0..1.0 + DT).contains(&t).then_some(Press::Attack);
            // The stick is held: a push cancels a roll from 1.1 s, after the press is taken.
            if let Decision::Swing { kind, chained } = step(&mut a, press) {
                out = Some((t, kind, chained));
            }
        }
        let (t, kind, chained) = out.expect("the roll attack starts");
        assert_eq!(kind, MoveKind::RollAttack);
        assert!(!chained);
        assert!((t - 1.1).abs() < 2.0 * DT, "{t}");
        // With no roll attack in the moveset it is the first light hit.
        let mut a = Action::default();
        let plain = |press| StepInput {
            roll: ds_roll,
            ds_roll: true,
            ..input(press, true)
        };
        step_action(
            &mut a,
            &s,
            StepInput {
                stick: Vec3::NEG_Z,
                ..plain(Some(Press::Evade))
            },
            DT,
        );
        while a.evading().is_some_and(|e| e.t < 1.2) {
            step_action(&mut a, &s, plain(None), DT);
        }
        let d = step_action(&mut a, &s, plain(Some(Press::Attack)), DT);
        assert_eq!(d, swing(MoveKind::R1(0), false));
    }

    #[test]
    fn a_heavy_press_without_a_heavy_attack_does_nothing() {
        let mut a = Action::default();
        let s = full();
        assert_eq!(
            step_action(&mut a, &s, input(Some(Press::Heavy), true), DT),
            Decision::None
        );
        assert!(!a.owns_body() && a.buffer.is_none(), "the press is dropped");
    }

    #[test]
    fn move_names_round_trip() {
        for k in MoveKind::ALL {
            assert_eq!(MoveKind::from_name(k.name()), Some(k));
        }
        assert_eq!(MoveKind::from_name("r3_1"), None);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -j 4 -p wow-souls combat`
Expected: FAIL to compile: `cannot find type Moveset`, `MoveKind`, `SwingSpec`; `Decision::Swing` has no field `kind`.

- [ ] **Step 3: Implement the machine**

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
use super::Stamina;
```
New:
```rust
use super::{Stamina, ATTACK_COST};
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
/// The phase at `t` seconds into a swing lasting `duration`; `None` once it is over.
pub fn swing_phase(t: f32, duration: f32) -> Option<SwingPhase> {
    if t >= duration {
        return None;
    }
    let f = t / duration;
    Some(if f < ACTIVE_FROM {
        SwingPhase::Startup
    } else if f < ACTIVE_TO {
        SwingPhase::Active
    } else {
        SwingPhase::Recovery
    })
}
```
New:
```rust
/// How many hits the one-handed light chain has, and the heavy one.
pub const R1_CHAIN: u8 = 3;
pub const R2_CHAIN: u8 = 2;

/// Which move a swing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveKind {
    /// The light chain, hit `n` (0, 1, 2).
    R1(u8),
    /// The heavy attack and its follow-up (0, 1).
    R2(u8),
    /// The attack out of a roll or a backstep.
    RollAttack,
}

impl MoveKind {
    /// Every move, in the order of the cache's table.
    pub const ALL: [MoveKind; 6] = [
        MoveKind::R1(0),
        MoveKind::R1(1),
        MoveKind::R1(2),
        MoveKind::R2(0),
        MoveKind::R2(1),
        MoveKind::RollAttack,
    ];

    /// The name the cache's move table uses.
    pub fn name(self) -> &'static str {
        match self {
            MoveKind::R1(0) => "r1_1",
            MoveKind::R1(1) => "r1_2",
            MoveKind::R1(_) => "r1_3",
            MoveKind::R2(0) => "r2_1",
            MoveKind::R2(_) => "r2_2",
            MoveKind::RollAttack => "roll_attack",
        }
    }

    pub fn from_name(name: &str) -> Option<MoveKind> {
        MoveKind::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// A swing's timing and price, in seconds from its start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwingSpec {
    pub duration: f32,
    /// The blade can hit over `[hit.0, hit.1)`.
    pub hit: (f32, f32),
    /// A buffered attack may start from here (Dark Souls' action flag 4).
    pub chain_from: f32,
    /// A buffered evade may start from here (flag 26).
    pub evade_from: f32,
    /// A pushed stick ends the swing from here (flag 11).
    pub move_from: f32,
    pub cost: f32,
    /// The damage of a hit, as a multiple of the base hit.
    pub mult: f32,
}

impl SwingSpec {
    /// Today's procedural swing of length `len`: its phases and its cancel are fractions of it,
    /// and only an evade or the next press interrupts it.
    pub fn procedural(len: f32) -> SwingSpec {
        SwingSpec {
            duration: len,
            hit: (ACTIVE_FROM * len, ACTIVE_TO * len),
            chain_from: CANCEL_FROM * len,
            evade_from: ACTIVE_TO * len,
            move_from: len,
            cost: ATTACK_COST,
            mult: 1.0,
        }
    }
}

/// The swings a body can make.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moveset {
    pub r1: [SwingSpec; R1_CHAIN as usize],
    pub r2: Option<[SwingSpec; R2_CHAIN as usize]>,
    pub roll_attack: Option<SwingSpec>,
    /// The specs come from Dark Souls clips, which pose the rig and move the body.
    pub ds: bool,
}

impl Moveset {
    /// Today's: one light swing of length `len`, no heavy attack, no roll attack.
    pub fn procedural(len: f32) -> Moveset {
        Moveset {
            r1: [SwingSpec::procedural(len); R1_CHAIN as usize],
            r2: None,
            roll_attack: None,
            ds: false,
        }
    }

    pub fn spec(&self, kind: MoveKind) -> Option<SwingSpec> {
        match kind {
            MoveKind::R1(n) => self.r1.get(usize::from(n)).copied(),
            MoveKind::R2(n) => self.r2.and_then(|r| r.get(usize::from(n)).copied()),
            MoveKind::RollAttack => self.roll_attack,
        }
    }

    /// There is a heavy attack to press.
    pub fn has_heavy(&self) -> bool {
        self.r2.is_some()
    }
}
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
/// A swing in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swing {
    pub t: f32,
    pub duration: f32,
    /// 0 for the first swing of a chain, then 1, 2, …
    pub chain: u8,
}

impl Swing {
    pub fn phase(&self) -> Option<SwingPhase> {
        swing_phase(self.t, self.duration)
    }

    /// A buffered follow-up may start now.
    pub fn cancellable(&self) -> bool {
        self.t >= CANCEL_FROM * self.duration
    }
}
```
New:
```rust
/// A swing in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swing {
    pub t: f32,
    pub kind: MoveKind,
    pub spec: SwingSpec,
    /// From a Dark Souls clip: it poses the rig and moves the body.
    pub ds: bool,
    /// The number of this swing since the body was made: a new serial may strike everyone again.
    pub serial: u32,
}

impl Swing {
    /// The phase now; `None` once the swing is over.
    pub fn phase(&self) -> Option<SwingPhase> {
        let s = &self.spec;
        if self.t >= s.duration {
            None
        } else if self.t < s.hit.0 {
            Some(SwingPhase::Startup)
        } else if self.t < s.hit.1 {
            Some(SwingPhase::Active)
        } else {
            Some(SwingPhase::Recovery)
        }
    }

    /// A buffered attack may start now.
    pub fn cancellable(&self) -> bool {
        self.t >= self.spec.chain_from
    }
}
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
/// Which action a press asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Attack,
    Evade,
}
```
New:
```rust
/// Which action a press asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// R1, the light attack.
    Attack,
    /// R2, the heavy attack.
    Heavy,
    Evade,
}
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
#[derive(Component, Debug, Default)]
pub struct Action {
    pub state: ActionState,
    pub buffer: Option<Buffered>,
}
```
New:
```rust
#[derive(Component, Debug, Default)]
pub struct Action {
    pub state: ActionState,
    pub buffer: Option<Buffered>,
    /// Swings started so far: the last one's serial.
    pub swings: u32,
}
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
    /// The current state takes a buffered `press` now: an idle body anything; a swing chains from
    /// its cancel window and evades from its recovery; an evade takes anything once it has
    /// travelled.
    fn accepts(&self, press: Press) -> bool {
        match (&self.state, press) {
            (ActionState::Idle, _) => true,
            (ActionState::Attack(s), Press::Attack) => s.cancellable(),
            (ActionState::Attack(s), Press::Evade) => s.t >= ACTIVE_TO * s.duration,
            (ActionState::Evade(e), _) => e.t >= e.spec().cancel_from,
            (ActionState::Stagger { .. } | ActionState::Dead, _) => false,
        }
    }
```
New:
```rust
    /// The current state takes a buffered `press` now: an idle body anything; a swing chains
    /// (light or heavy) from its chain time and evades from its evade time; an evade takes
    /// anything once it has travelled.
    fn accepts(&self, press: Press) -> bool {
        match (&self.state, press) {
            (ActionState::Idle, _) => true,
            (ActionState::Attack(s), Press::Attack | Press::Heavy) => s.cancellable(),
            (ActionState::Attack(s), Press::Evade) => s.t >= s.spec.evade_from,
            (ActionState::Evade(e), _) => e.t >= e.spec().cancel_from,
            (ActionState::Stagger { .. } | ActionState::Dead, _) => false,
        }
    }
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
    /// A swing started (`chain` index); the caller plays the clip and spends stamina.
    Swing(u8),
```
New:
```rust
    /// A swing started; `chained` when it came out of another swing. The caller plays its clip
    /// (the procedural swing) and spends its stamina.
    Swing { kind: MoveKind, chained: bool },
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
    /// The swing's length (s).
    pub swing_len: f32,
```
New:
```rust
    /// The swings this body can make.
    pub moves: Moveset,
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
    // Movement cancels an evade from its cancel window (Dark Souls' flag 11): a pushed stick ends
    // it, unless a buffered press was taken above.
    if let ActionState::Evade(e) = a.state {
        if e.t >= e.spec.cancel_from && Vec3::new(i.stick.x, 0.0, i.stick.z).length() > ROLL_PUSH {
            a.state = ActionState::Idle;
            return Decision::Ended;
        }
    }
```
New:
```rust
    // Movement cancels an evade from its cancel window (Dark Souls' flag 11) and a swing from its
    // own: a pushed stick ends it, unless a buffered press was taken above.
    let pushed = Vec3::new(i.stick.x, 0.0, i.stick.z).length() > ROLL_PUSH;
    let free = match a.state {
        ActionState::Evade(e) => e.t >= e.spec.cancel_from,
        ActionState::Attack(s) => s.t >= s.spec.move_from,
        _ => false,
    };
    if pushed && free {
        a.state = ActionState::Idle;
        return Decision::Ended;
    }
```

**Edit** `crates/wow-souls/src/combat/action.rs`:

Old:
```rust
/// Starts the action `press` asks for: a swing (the next of a chain out of a swing), or an evade,
/// a roll along a pushed stick or else a backstep.
fn start(a: &mut Action, press: Press, i: &StepInput) -> Decision {
    match press {
        Press::Attack => {
            let chain = match a.state {
                ActionState::Attack(s) => s.chain.saturating_add(1),
                _ => 0,
            };
            a.state = ActionState::Attack(Swing {
                t: 0.0,
                duration: i.swing_len,
                chain,
            });
            Decision::Swing(chain)
        }
```
New:
```rust
/// The move a press starts out of `state`: the chain's next light hit (looping after the last),
/// the first light hit out of a heavy one, a heavy press starts the heavy attack or its
/// follow-up (and the heavy attack again after that), and a light press out of an evade is the
/// roll attack where the moveset has one.
fn next_move(state: &ActionState, press: Press, moves: &Moveset) -> MoveKind {
    match (state, press) {
        (ActionState::Attack(s), Press::Heavy) => match s.kind {
            MoveKind::R2(0) => MoveKind::R2(1),
            _ => MoveKind::R2(0),
        },
        (_, Press::Heavy) => MoveKind::R2(0),
        (ActionState::Attack(s), _) => match s.kind {
            MoveKind::R1(n) => MoveKind::R1((n + 1) % R1_CHAIN),
            _ => MoveKind::R1(0),
        },
        (ActionState::Evade(_), _) if moves.roll_attack.is_some() => MoveKind::RollAttack,
        _ => MoveKind::R1(0),
    }
}

/// Starts the action `press` asks for: a swing (the next of a chain out of a swing), or an evade,
/// a roll along a pushed stick or else a backstep. A swing the moveset lacks is no action.
fn start(a: &mut Action, press: Press, i: &StepInput) -> Decision {
    match press {
        Press::Attack | Press::Heavy => {
            let kind = next_move(&a.state, press, &i.moves);
            let Some(spec) = i.moves.spec(kind) else {
                return Decision::None;
            };
            let chained = matches!(a.state, ActionState::Attack(_));
            a.swings += 1;
            a.state = ActionState::Attack(Swing {
                t: 0.0,
                kind,
                spec,
                ds: i.moves.ds,
                serial: a.swings,
            });
            Decision::Swing { kind, chained }
        }
```

- [ ] **Step 4: Keep the game compiling and playing as before**

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
    let input = StepInput {
        press,
        grounded: m.grounded,
        swing_len,
```
New:
```rust
    let input = StepInput {
        press,
        grounded: m.grounded,
        moves: Moveset::procedural(swing_len),
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
        Decision::Swing(chain) => {
            stamina.spend(ATTACK_COST);
            if let (Some(c), Some(mut p), Some(mut tr)) = (clip, player, transitions) {
                play_once(&mut p, &mut tr, c.node, ATTACK_RATE, chain > 0);
            }
            if trace {
                evade_trace.close(xf.translation, now);
                info!(
                    "combat: swing {chain} ({swing_len:.2} s), stamina {:.0}",
                    stamina.cur
                );
            }
        }
```
New:
```rust
        Decision::Swing { kind, chained } => {
            stamina.spend(input.moves.spec(kind).map_or(ATTACK_COST, |s| s.cost));
            if let (Some(c), Some(mut p), Some(mut tr)) = (clip, player, transitions) {
                play_once(&mut p, &mut tr, c.node, ATTACK_RATE, chained);
            }
            if trace {
                evade_trace.close(xf.translation, now);
                info!(
                    "combat: move {} ({swing_len:.2} s), stamina {:.0}",
                    kind.name(),
                    stamina.cur
                );
            }
        }
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
            ActionState::Attack(sw) => {
                format!("attack#{} {:?} t={:.2}", sw.chain, sw.phase(), sw.t)
            }
```
New:
```rust
            ActionState::Attack(sw) => {
                format!("{} {:?} t={:.2}", sw.kind.name(), sw.phase(), sw.t)
            }
```

**Edit** `crates/wow-souls/src/hits.rs`:

Old:
```rust
/// Damage of a light attack.
pub const LIGHT_DAMAGE: f32 = 20.0;
```
New:
```rust
/// The base hit; a move's damage multiplier (`SwingSpec::mult`) scales it.
pub const LIGHT_DAMAGE: f32 = 20.0;

/// The damage of one hit of a move: the base hit times the move's multiplier.
fn hit_damage(spec: &SwingSpec) -> f32 {
    LIGHT_DAMAGE * spec.mult
}
```

**Edit** `crates/wow-souls/src/hits.rs`:

Old:
```rust
use crate::combat::{Action, ActionState, Health, SwingPhase};
```
New:
```rust
use crate::combat::{Action, ActionState, Health, SwingPhase, SwingSpec};
```

**Edit** `crates/wow-souls/src/hits.rs`:

Old:
```rust
    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1.0e-4
    }

    #[test]
    fn crossing_segments_touch() {
```
New:
```rust
    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1.0e-4
    }

    #[test]
    fn a_moves_damage_is_the_base_hit_times_its_multiplier() {
        let hit = |mult| {
            hit_damage(&SwingSpec {
                mult,
                ..SwingSpec::procedural(0.8)
            })
        };
        // The spec's table: R1 x1.00, R2 x1.35, R2 second x1.25, rolling attack x0.90.
        for (mult, want) in [(1.0, 20.0), (1.35, 27.0), (1.25, 25.0), (0.9, 18.0)] {
            assert!(close(hit(mult), want), "x{mult}: {}", hit(mult));
        }
        assert!(close(hit(SwingSpec::procedural(0.8).mult), LIGHT_DAMAGE));
    }

    #[test]
    fn crossing_segments_touch() {
```

**Edit** `crates/wow-souls/src/hits.rs`:

Old:
```rust
    prev: Option<[Vec3; 2]>,
    swing: Option<u8>,
    struck: Vec<Entity>,
```
New:
```rust
    prev: Option<[Vec3; 2]>,
    /// The serial of the swing that `struck` belongs to.
    swing: Option<u32>,
    struck: Vec<Entity>,
```

**Edit** `crates/wow-souls/src/hits.rs`:

Old:
```rust
    // A new swing (or a chained one) may strike everyone again.
    if trail.swing != Some(swing.chain) || swing.t <= 0.0 {
        trail.swing = Some(swing.chain);
        trail.struck.clear();
    }
```
New:
```rust
    // A new swing (or a chained one) may strike everyone again.
    if trail.swing != Some(swing.serial) || swing.t <= 0.0 {
        trail.swing = Some(swing.serial);
        trail.struck.clear();
    }
```

**Edit** `crates/wow-souls/src/hits.rs`:

Old:
```rust
        trail.struck.push(target);
        health.cur = (health.cur - LIGHT_DAMAGE).max(0.0);
        let lethal = health.cur <= 0.0;
        hits.write(Hit {
            target,
            damage: LIGHT_DAMAGE,
            point,
            lethal,
        });
        info!(
            "hit: {target} for {LIGHT_DAMAGE} at {point:.2}, health {:.0}/{:.0}{}",
```
New:
```rust
        trail.struck.push(target);
        let damage = hit_damage(&swing.spec);
        health.cur = (health.cur - damage).max(0.0);
        let lethal = health.cur <= 0.0;
        hits.write(Hit {
            target,
            damage,
            point,
            lethal,
        });
        info!(
            "hit: {target} for {damage:.0} at {point:.2}, health {:.0}/{:.0}{}",
```

**Edit** `crates/wow-souls/src/hits.rs`:

Old:
```rust
            swing.t / swing.duration,
```
New:
```rust
            swing.t / swing.spec.duration,
```

- [ ] **Step 5: Run the tests**

Run: `cargo fmt --all && cargo test -j 4 -p wow-souls`
Expected: PASS: the old tests (adapted) and the twelve new ones (169 in all). In particular `the_procedural_swing_keeps_todays_windows` pins the fallback.

- [ ] **Step 6: Checkpoint**

Run `cargo fmt --all -- --check && cargo test -j 4 --workspace`. Expected: green. The game plays exactly as before: `act` still builds `Moveset::procedural`. Clippy waits for Task 9: until then `MoveKind::ALL`, `MoveKind::from_name`, `Moveset::has_heavy` and `Press::Heavy` are unused and `-D warnings` stops on them.

---

### Task 8: `dsclip.rs` and `dsview.rs`: load and pose any cached clip

The game learns to read the new cache: the roll and the six moves, each move's timing from its clip's windows, its price from the manifest; any clip poses the rig; a chained clip fades the last one out under it; while clips pose the rig, the held sword's blade is turned onto the Dark Souls blade axis, and the side-by-side measures, every frame, how far the drawn blade strays from the Dark Souls one (Task 11 reads the numbers). `act` does not start a Dark Souls swing until Task 9, so after this task the game still swings the procedural swing; the roll is the Dark Souls roll as before.

**Files:**
- Create (replacing the old file): `crates/wow-souls/src/dsclip.rs`
- Create (replacing the old file): `crates/wow-souls/src/dsview.rs`

- [ ] **Step 1: Write `dsclip.rs`**

**Create** `crates/wow-souls/src/dsclip.rs`:
```rust
//! The Dark Souls clips from `cache/dsr/` (written by `wow-souls-dsr extract`): the roll and the
//! straight sword's moves. They load at startup when the cache is present and fresh, are sampled
//! per frame onto the avatar's rig in benilla's `PosePost` window, and give the roll its
//! `EvadeSpec` and every move its `SwingSpec` (length, windows, price) from their own data.
//! Without a cache the procedural roll and swing stay.
//!
//! A clip blends over the rig's own pose (the `AnimationPlayer`'s clip, evaluated this frame
//! before `PosePost`): in over [`BLEND_IN`] from its start, and out over [`BLEND_OUT`] once the
//! action lets go of the body, holding the pose it ended on. A clip that takes over from another
//! (a chained move, a chained roll) leaves the old end pose fading under it.
//!
//! While clips pose the rig, the held item's blade is turned onto the Dark Souls blade axis
//! ([`grip_rotation`]): the retarget carries the hand's rotation, not the sword in it.

use std::path::{Path, PathBuf};

use benilla_world::rig_anim::{PosePost, RigPose};
use bevy::prelude::*;

use crate::actor::held::HeldBuilt;
use crate::avatar::Avatar;
use crate::combat::{Action, ActionState, EvadeSpec, MoveKind, Moveset, SwingSpec, ROLL};

/// `ChrActionFlag` values that open a cancel window (attack, movement, step).
const CANCEL_FLAGS: [i32; 3] = [4, 11, 26];

/// Seconds of a clip over which its weight rises from the rig's pose to the clip's.
pub const BLEND_IN: f32 = 0.1;
/// Real seconds after the action ends over which the clip's last pose fades back to the rig's.
pub const BLEND_OUT: f32 = 0.2;

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

/// The root motion's forward distance (yd, along Bevy -Z of the model) covered by `t`, in model
/// units: the body's world distance is this times the model's scale.
pub fn progress(clip: &wscl::Clip, t: f32) -> f32 {
    let n = clip.root_motion.len();
    if n == 0 {
        return 0.0;
    }
    let f = (t.max(0.0) * clip.fps).min((n - 1) as f32);
    let i = (f as usize).min(n.saturating_sub(2));
    let a = if n == 1 { 0.0 } else { f - i as f32 };
    let z0 = clip.root_motion[0][2];
    let z = clip.root_motion[i][2]
        + (clip.root_motion[(i + 1).min(n - 1)][2] - clip.root_motion[i][2]) * a;
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

/// A move's timing and price from its clip's TAE windows: the hit window, and the three cancel
/// windows (attack, evade, movement), whose first starts are the times they open.
pub fn swing_spec(clip: &wscl::Clip, cost: f32, mult: f32) -> Result<SwingSpec, String> {
    let opens = |flag: i32, what: &str| {
        clip.windows(flag)
            .map(|(start, _)| start)
            .min_by(f32::total_cmp)
            .ok_or_else(|| format!("{} has no {what} window (flag {flag})", clip.name))
    };
    let hit = clip
        .hits
        .first()
        .map(|h| (h.start, h.end))
        .ok_or_else(|| format!("{} has no hit window", clip.name))?;
    Ok(SwingSpec {
        duration: clip.duration(),
        hit,
        chain_from: opens(wscl::FLAG_CHAIN, "chain")?,
        evade_from: opens(wscl::FLAG_EVADE, "evade")?,
        move_from: opens(wscl::FLAG_MOVE, "movement")?,
        cost,
        mult,
    })
}

/// The clip's weight over the rig's pose `t` seconds into the clip: rising over [`BLEND_IN`],
/// then, `since_end` real seconds after the action ended, falling to 0 over [`BLEND_OUT`].
pub fn blend_weight(t: f32, since_end: Option<f32>) -> f32 {
    let rise = (t / BLEND_IN).clamp(0.0, 1.0);
    let fall = since_end.map_or(1.0, |s| (1.0 - s / BLEND_OUT).clamp(0.0, 1.0));
    rise * fall
}

/// Which cached clip a layer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipId {
    Roll,
    Move(MoveKind),
}

/// The loaded Dark Souls roll.
#[derive(Resource)]
pub struct DsRoll {
    /// On the WoW HumanMale skeleton, bone-local.
    pub wow: wscl::Clip,
    /// On the DS skeleton, model space, Bevy axes, WoW scale (for the side-by-side).
    pub ds: wscl::Clip,
    pub spec: EvadeSpec,
}

/// One cached move: its clips and its timing and price.
pub struct MoveClip {
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    pub spec: SwingSpec,
}

/// Where the Dark Souls blade points, from the cache: its axis in the WoW right-hand bone's frame
/// (what a held item's blade is turned onto), and in the DS weapon bone's own axes (Bevy axes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grip {
    pub hand: Vec3,
    pub weapon: Vec3,
}

/// The loaded straight sword.
#[derive(Resource)]
pub struct DsMoves {
    clips: Vec<(MoveKind, MoveClip)>,
    /// The six specs, as the action machine reads them.
    pub moveset: Moveset,
    pub grip: Grip,
}

impl DsMoves {
    pub fn clip(&self, kind: MoveKind) -> Option<&MoveClip> {
        self.clips.iter().find(|(k, _)| *k == kind).map(|(_, c)| c)
    }
}

/// A clip's two files: on the WoW rig, and on the DS skeleton.
pub fn clips_of<'a>(
    roll: &'a DsRoll,
    moves: &'a DsMoves,
    id: ClipId,
) -> Option<(&'a wscl::Clip, &'a wscl::Clip)> {
    match id {
        ClipId::Roll => Some((&roll.wow, &roll.ds)),
        ClipId::Move(k) => moves.clip(k).map(|m| (&m.wow, &m.ds)),
    }
}

/// The rotation, in the hand's frame, that turns a held item's blade (its `$WTB` to `$WTT`
/// markers, `[top, bottom]`) onto the Dark Souls blade axis `hand`.
pub fn grip_rotation(blade: [Vec3; 2], hand: Vec3) -> Quat {
    let from = (blade[0] - blade[1]).normalize_or_zero();
    let to = hand.normalize_or_zero();
    if from == Vec3::ZERO || to == Vec3::ZERO {
        return Quat::IDENTITY;
    }
    Quat::from_rotation_arc(from, to)
}

/// The item's rotation `weight` of the way from none to `fit`.
pub fn item_rotation(fit: Quat, weight: f32) -> Quat {
    Quat::IDENTITY.slerp(fit, weight.clamp(0.0, 1.0))
}

/// `WOWSOULS_GRIP=rx,ry,rz`: the rotation (degrees about the hand's X, Y, Z) of the held item
/// while clips pose the rig, replacing the automatic fit. A tuning aid for the side-by-side.
#[derive(Resource, Debug, Clone, Copy)]
pub struct GripOverride(pub Quat);

pub fn parse_grip_override(v: &str) -> Option<Quat> {
    let n: Vec<f32> = v
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect::<Option<_>>()?;
    let [x, y, z] = n[..] else { return None };
    Some(Quat::from_euler(
        EulerRot::XYZ,
        x.to_radians(),
        y.to_radians(),
        z.to_radians(),
    ))
}

/// The manifest format this game reads (its `wscl-cache` line).
const CACHE_VERSION: &str = "2";

fn cache_dir() -> PathBuf {
    std::env::var("WOWSOULS_DSR_CACHE").map_or_else(|_| PathBuf::from("cache/dsr"), PathBuf::from)
}

/// A `move` line of the manifest.
struct MoveLine {
    kind: MoveKind,
    clip: String,
    stamina: f32,
    mult: f32,
}

/// `move <name> <clip> judge <n> stamina <n> mult <x>`.
fn parse_move(line: &str) -> Result<MoveLine, String> {
    let bad = || format!("bad move line {line:?}");
    let w: Vec<&str> = line.split_whitespace().collect();
    let [_, name, clip, "judge", _, "stamina", stamina, "mult", mult] = w[..] else {
        return Err(bad());
    };
    Ok(MoveLine {
        kind: MoveKind::from_name(name)
            .ok_or_else(|| format!("the manifest has a move this game does not know: {name}"))?,
        clip: clip.to_string(),
        stamina: stamina.parse().map_err(|_| bad())?,
        mult: mult.parse().map_err(|_| bad())?,
    })
}

/// `grip <x> <y> <z> <x> <y> <z>`: the blade axis in the WoW hand bone's frame, then in the DS
/// weapon bone's axes.
fn parse_grip_line(line: &str) -> Result<Grip, String> {
    let n: Vec<f32> = line
        .split_whitespace()
        .skip(1)
        .map(|w| w.parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect::<Option<_>>()
        .ok_or_else(|| format!("bad grip line {line:?}"))?;
    let [hx, hy, hz, wx, wy, wz] = n[..] else {
        return Err(format!("the grip line has {} numbers, expected 6", n.len()));
    };
    Ok(Grip {
        hand: Vec3::new(hx, hy, hz),
        weapon: Vec3::new(wx, wy, wz),
    })
}

/// What the cache holds.
struct Cache {
    roll: DsRoll,
    moves: DsMoves,
}

/// Reads the manifest and the clips it names; `Err` says why the cache is unusable (missing,
/// stale, another version, incomplete).
fn load(dir: &Path) -> Result<Cache, String> {
    let manifest = std::fs::read_to_string(dir.join("manifest.txt"))
        .map_err(|e| format!("no manifest in {} ({e})", dir.display()))?;
    let (mut roll, mut cache_version, mut extractor) = (None, false, false);
    let (mut lines, mut grip) = (Vec::new(), None);
    for line in manifest.lines() {
        let mut w = line.splitn(4, ' ');
        match (w.next(), w.next(), w.next(), w.next()) {
            (Some("wscl-cache"), Some(v), _, _) => {
                if v != CACHE_VERSION {
                    return Err(format!("cache version {v}, expected {CACHE_VERSION}"));
                }
                cache_version = true;
            }
            (Some("extractor"), Some(v), _, _) => {
                if v != wscl::EXTRACTOR_VERSION.to_string() {
                    return Err(format!(
                        "extractor version {v}, expected {}",
                        wscl::EXTRACTOR_VERSION
                    ));
                }
                extractor = true;
            }
            (Some("roll"), Some(name), _, _) => roll = Some(name.to_string()),
            (Some("move"), ..) => lines.push(parse_move(line)?),
            (Some("grip"), ..) => grip = Some(parse_grip_line(line)?),
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
    if !(cache_version && extractor) {
        return Err("manifest lacks a version".into());
    }
    let name = roll.ok_or("manifest names no roll")?;
    let read = |sub: &str, name: &str| -> Result<wscl::Clip, String> {
        let p = dir.join(sub).join(format!("{name}.wscl"));
        let f = std::fs::File::open(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let clip = wscl::Clip::read(&mut std::io::BufReader::new(f))
            .map_err(|e| format!("{}: {e}", p.display()))?;
        if clip.frames.len() < 2 || !(clip.fps.is_finite() && clip.fps > 0.0) {
            return Err(format!(
                "{}: {} frames at {} fps is no clip",
                p.display(),
                clip.frames.len(),
                clip.fps
            ));
        }
        Ok(clip)
    };
    let wow = read("human_male", &name)?;
    let ds = read("c0000", &name)?;
    let spec = roll_spec(&wow);
    let roll = DsRoll { wow, ds, spec };

    let mut clips = Vec::new();
    for kind in MoveKind::ALL {
        let line = lines
            .iter()
            .find(|l| l.kind == kind)
            .ok_or_else(|| format!("manifest lacks move {}", kind.name()))?;
        let wow = read("human_male", &line.clip)?;
        let ds = read("c0000", &line.clip)?;
        if wow.bones() != roll.wow.bones() {
            return Err(format!(
                "{}: {} bones, the roll's clip has {}",
                line.clip,
                wow.bones(),
                roll.wow.bones()
            ));
        }
        let spec = swing_spec(&wow, line.stamina, line.mult)?;
        clips.push((kind, MoveClip { wow, ds, spec }));
    }
    let grip = grip.ok_or("manifest has no grip line")?;
    let spec = |k: MoveKind| {
        clips
            .iter()
            .find(|(c, _)| *c == k)
            .map(|(_, m)| m.spec)
            .expect("every move was read")
    };
    let moveset = Moveset {
        r1: [
            spec(MoveKind::R1(0)),
            spec(MoveKind::R1(1)),
            spec(MoveKind::R1(2)),
        ],
        r2: Some([spec(MoveKind::R2(0)), spec(MoveKind::R2(1))]),
        roll_attack: Some(spec(MoveKind::RollAttack)),
        ds: true,
    };
    Ok(Cache {
        roll,
        moves: DsMoves {
            clips,
            moveset,
            grip,
        },
    })
}

fn startup(mut commands: Commands) {
    match load(&cache_dir()) {
        Ok(c) => {
            info!(
                "dsclip: Dark Souls roll {} ({:.3} s, {:.2} yd, invulnerable {:.3}-{:.3} s, cancel from {:.3} s)",
                c.roll.wow.name,
                c.roll.spec.duration,
                c.roll.spec.distance,
                c.roll.spec.iframes.0,
                c.roll.spec.iframes.1,
                c.roll.spec.cancel_from
            );
            for kind in MoveKind::ALL {
                if let Some(m) = c.moves.clip(kind) {
                    let s = &m.spec;
                    info!(
                        "dsclip: {} {} ({:.3} s, hit {:.3}-{:.3}, chain {:.3}, evade {:.3}, move {:.3}, {:.2} yd, stamina {:.0}, x{:.2})",
                        kind.name(),
                        m.wow.name,
                        s.duration,
                        s.hit.0,
                        s.hit.1,
                        s.chain_from,
                        s.evade_from,
                        s.move_from,
                        progress(&m.wow, s.duration),
                        s.cost,
                        s.mult
                    );
                }
            }
            commands.insert_resource(c.roll);
            commands.insert_resource(c.moves);
        }
        Err(why) => {
            warn!("dsclip: no Dark Souls clips ({why}); the procedural roll and swing stay");
        }
    }
}

/// The clip the avatar's action plays and its time: the Dark Souls roll or a Dark Souls move;
/// `None` for anything else (including the procedural roll and swing).
fn playing(action: &Action) -> Option<(ClipId, f32)> {
    match action.state {
        ActionState::Evade(e) if e.ds => Some((ClipId::Roll, e.t)),
        ActionState::Attack(s) if s.ds => Some((ClipId::Move(s.kind), s.t)),
        _ => None,
    }
}

/// The clip last shown and the one fading out.
#[derive(Debug)]
pub struct Fade<K> {
    /// The running clip and its time, as of the last frame.
    last: Option<(K, f32)>,
    /// After it ended: the clip, the time it ended on, and the real seconds since.
    out: Option<(K, f32, f32)>,
}

impl<K> Default for Fade<K> {
    fn default() -> Self {
        Fade {
            last: None,
            out: None,
        }
    }
}

/// One clip layer to blend over the rig: which clip, its time, its weight.
pub type Layer<K> = (K, f32, f32);

/// The clip layers to blend over the rig this frame, in order: the last clip's end pose fading
/// out, then the running clip (`now`: its identity and time) blending in. A clip that ends, is
/// replaced by another clip, or restarts its clock (a chained roll) leaves its end pose fading
/// under the new one, so the clips never let go of the rig between two moves. `dt` is real
/// seconds.
pub fn layers<K: Copy + PartialEq>(
    fade: &mut Fade<K>,
    now: Option<(K, f32)>,
    dt: f32,
) -> [Option<Layer<K>>; 2] {
    if let Some((last_clip, last_t)) = fade.last {
        if now.is_none_or(|(clip, t)| clip != last_clip || t < last_t) {
            fade.out = Some((last_clip, last_t, 0.0));
            fade.last = None;
        }
    }
    let out = fade.out.as_mut().and_then(|(clip, t, since)| {
        let w = blend_weight(*t, Some(*since));
        *since += dt;
        (w > 0.0).then_some((*clip, *t, w))
    });
    if out.is_none() {
        fade.out = None;
    }
    let inn = now.map(|(clip, t)| {
        fade.last = Some((clip, t));
        (clip, t, blend_weight(t, None))
    });
    [out, inn]
}

/// While the avatar plays a Dark Souls clip (or the side-by-side runs), the clip owns the driven
/// bones, blended over the rig's pose at its ends, and the held blade is turned onto the Dark
/// Souls blade axis by as much as the clips hold the rig. A clip that does not fit the rig is
/// dropped, and the procedural roll and swing take over.
#[allow(clippy::too_many_arguments)]
fn pose(
    mut commands: Commands,
    real: Res<Time<bevy::time::Real>>,
    roll: Option<Res<DsRoll>>,
    moves: Option<Res<DsMoves>>,
    view: Option<Res<crate::dsview::DsView>>,
    tune: Option<Res<GripOverride>>,
    mut avatar: Query<(&Action, &mut RigPose, Option<&HeldBuilt>), With<Avatar>>,
    mut items: Query<&mut Transform, Without<Avatar>>,
    mut fade: Local<Fade<ClipId>>,
    mut told: Local<bool>,
) {
    let (Some(roll), Some(moves), Ok((action, mut pose, held))) =
        (roll, moves, avatar.single_mut())
    else {
        return;
    };
    let layers = match view.as_deref().and_then(crate::dsview::DsView::playing) {
        Some((clip, t)) => [Some((clip, t, 1.0)), None],
        None => layers(&mut fade, playing(action), real.delta_secs()),
    };
    // The held blade onto the Dark Souls blade axis, as far as the clips hold the rig.
    if let Some((held, blade)) = held.and_then(|h| h.blade.map(|b| (h, b))) {
        let tuned = tune.is_some();
        let fit = tune.map_or_else(|| grip_rotation(blade, moves.grip.hand), |t| t.0);
        if !*told {
            *told = true;
            info!(
                "dsclip: the held blade turns {:.0} deg onto the Dark Souls blade axis ({})",
                fit.to_axis_angle().1.to_degrees(),
                if tuned { "WOWSOULS_GRIP" } else { "fitted" }
            );
        }
        let weight = 1.0
            - layers
                .iter()
                .flatten()
                .fold(1.0, |rest, &(_, _, w)| rest * (1.0 - w));
        let want = item_rotation(fit, weight);
        if let Ok(mut item) = items.get_mut(held.root) {
            if item.rotation != want {
                item.rotation = want;
            }
        }
    }
    if layers.iter().all(Option::is_none) {
        return;
    }
    if roll.wow.bones() != pose.locals.len() {
        warn!(
            "dsclip: clips have {} bones, the rig {}; the procedural roll and swing take over",
            roll.wow.bones(),
            pose.locals.len()
        );
        commands.remove_resource::<DsRoll>();
        commands.remove_resource::<DsMoves>();
        return;
    }
    for (clip, t, w) in layers.into_iter().flatten() {
        let Some((wow, _)) = clips_of(&roll, &moves, clip) else {
            continue;
        };
        for (i, local) in pose.locals.iter_mut().enumerate() {
            if !wow.driven[i] {
                continue;
            }
            let (tr, r, s) = sample(wow, t, i);
            *local = if w >= 1.0 {
                Transform {
                    translation: tr,
                    rotation: r,
                    scale: s,
                }
            } else {
                Transform {
                    translation: local.translation.lerp(tr, w),
                    rotation: local.rotation.slerp(r, w),
                    scale: local.scale.lerp(s, w),
                }
            };
        }
    }
    pose.pose_dirty = true;
}

pub fn plugin(app: &mut App) {
    if let Some(q) = std::env::var("WOWSOULS_GRIP")
        .ok()
        .and_then(|v| parse_grip_override(&v))
    {
        app.insert_resource(GripOverride(q));
    }
    // In `PosePost` beside benilla's global-sequence channels, which drive no bone the clip does.
    app.add_systems(Startup, startup)
        .add_systems(PostUpdate, pose.in_set(PosePost));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::Swing;

    /// A 2-bone, 2-frame clip with a position and a turn.
    fn clip() -> wscl::Clip {
        let a = wscl::Trs {
            t: [0.0, 1.0, 0.0],
            ..wscl::Trs::IDENTITY
        };
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
                wscl::FlagWindow {
                    flag: wscl::FLAG_INVINCIBLE,
                    start: 0.02,
                    end: 0.07,
                },
                wscl::FlagWindow {
                    flag: 11,
                    start: 0.08,
                    end: 0.1,
                },
            ],
            hits: Vec::new(),
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

    /// A move's clip: one second at 10 fps, with a hit window and the three cancel flags.
    fn move_clip(name: &str, hit: (f32, f32), flags: [f32; 3]) -> wscl::Clip {
        let flag = |flag, start| wscl::FlagWindow {
            flag,
            start,
            end: 1.0,
        };
        wscl::Clip {
            name: name.into(),
            fps: 10.0,
            model_space: false,
            parents: vec![-1, 0],
            driven: vec![true, true],
            frames: vec![vec![wscl::Trs::IDENTITY; 2]; 11],
            root_motion: vec![[0.0; 4]; 11],
            // The extractor also writes the clip's other flags; the loader reads only these.
            flags: vec![
                wscl::FlagWindow {
                    flag: 7,
                    start: 0.0,
                    end: 1.0,
                },
                flag(wscl::FLAG_MOVE, flags[2]),
                flag(wscl::FLAG_CHAIN, flags[0]),
                flag(wscl::FLAG_EVADE, flags[1]),
            ],
            hits: vec![wscl::HitWindow {
                judge: 0,
                start: hit.0,
                end: hit.1,
            }],
        }
    }

    #[test]
    fn a_move_takes_its_timing_from_its_clip() {
        let c = move_clip("a23_3000", (0.3, 0.4), [0.6, 0.7, 0.8]);
        let s = swing_spec(&c, 20.0, 1.35).unwrap();
        assert_eq!(s.duration, 1.0);
        assert_eq!(s.hit, (0.3, 0.4));
        assert_eq!(
            (s.chain_from, s.evade_from, s.move_from),
            (0.6, 0.7, 0.8),
            "flag 4 chains, 26 evades, 11 moves"
        );
        assert_eq!((s.cost, s.mult), (20.0, 1.35));
        // Without a hit window, or without a cancel flag, the clip is no move.
        let mut c = c;
        c.flags.retain(|f| f.flag != wscl::FLAG_EVADE);
        let err = swing_spec(&c, 20.0, 1.0).unwrap_err();
        assert!(err.contains("no evade window"), "{err}");
        c.hits.clear();
        let err = swing_spec(&c, 20.0, 1.0).unwrap_err();
        assert!(err.contains("no hit window"), "{err}");
    }

    #[test]
    fn the_clip_blends_in_at_the_start_and_out_after_the_end() {
        assert_eq!(blend_weight(0.0, None), 0.0);
        assert!((blend_weight(0.5 * BLEND_IN, None) - 0.5).abs() < 1.0e-6);
        assert_eq!(blend_weight(1.5, None), 1.0);
        // Ended at full weight: falls linearly to nothing over the fade.
        assert_eq!(blend_weight(1.1, Some(0.0)), 1.0);
        assert!((blend_weight(1.1, Some(0.5 * BLEND_OUT)) - 0.5).abs() < 1.0e-6);
        assert_eq!(blend_weight(1.1, Some(BLEND_OUT)), 0.0);
        // Cut off while still blending in: the fade starts from the weight it had.
        assert!((blend_weight(0.5 * BLEND_IN, Some(0.5 * BLEND_OUT)) - 0.25).abs() < 1.0e-6);
    }

    /// The clips' total weight over the rig from a frame's layers, each blended over the last.
    fn total(layers: [Option<Layer<ClipId>>; 2]) -> f32 {
        1.0 - layers
            .iter()
            .flatten()
            .fold(1.0, |rest, &(_, _, w)| rest * (1.0 - w))
    }

    #[test]
    fn a_chained_roll_blends_in_over_the_last_ones_end_pose() {
        const DT: f32 = 1.0 / 60.0;
        let mut fade = Fade::default();
        let mut t = 0.0;
        let mut end = 0.0;
        while t < 1.1 {
            let l = layers(&mut fade, Some((ClipId::Roll, t)), DT);
            assert!(l[0].is_none() && l[1].is_some());
            end = t;
            t += DT;
        }
        // The chained roll restarts the clock: the old end pose holds at full weight under it.
        let l = layers(&mut fade, Some((ClipId::Roll, 0.0)), DT);
        assert_eq!(l[0], Some((ClipId::Roll, end, 1.0)), "{l:?}");
        assert_eq!(l[1], Some((ClipId::Roll, 0.0, 0.0)));
        // No frame lets the rig's own clip through between the two rolls.
        let mut t = DT;
        while t < 0.5 {
            let l = layers(&mut fade, Some((ClipId::Roll, t)), DT);
            assert!(total(l) > 0.8, "t={t} {l:?}");
            t += DT;
        }
        assert!(fade.out.is_none(), "the old pose has faded");
    }

    #[test]
    fn a_swing_chained_out_of_another_clip_fades_the_old_one_under_it() {
        const DT: f32 = 1.0 / 60.0;
        let first = ClipId::Move(MoveKind::R1(0));
        let second = ClipId::Move(MoveKind::R1(1));
        let mut fade = Fade::default();
        let mut t = 0.0;
        let mut end = 0.0;
        while t < 0.9 {
            layers(&mut fade, Some((first, t)), DT);
            end = t;
            t += DT;
        }
        let l = layers(&mut fade, Some((second, 0.0)), DT);
        assert_eq!(l[0], Some((first, end, 1.0)), "{l:?}");
        assert_eq!(l[1], Some((second, 0.0, 0.0)));
        // Another clip takes over even when its time is later than the old one's.
        let mut fade = Fade::default();
        layers(&mut fade, Some((first, 0.5)), DT);
        let l = layers(&mut fade, Some((second, 0.9)), DT);
        assert!(l[0].is_some_and(|(c, ..)| c == first), "{l:?}");
    }

    #[test]
    fn an_ended_clip_fades_out_and_a_new_one_keeps_the_fade() {
        const DT: f32 = 1.0 / 60.0;
        let mut fade = Fade::default();
        layers(&mut fade, Some((ClipId::Roll, 1.1)), DT);
        // Cancelled by movement: the end pose fades from full weight.
        let l = layers(&mut fade, None, DT);
        assert_eq!(l, [Some((ClipId::Roll, 1.1, 1.0)), None]);
        layers(&mut fade, None, DT);
        // A new move inside the fade keeps the fade under its blend-in.
        let l = layers(&mut fade, Some((ClipId::Move(MoveKind::R1(0)), 0.0)), DT);
        assert!(
            l[0].is_some_and(|(c, t, w)| c == ClipId::Roll && t == 1.1 && w > 0.8),
            "{l:?}"
        );
        // Then the fade runs out.
        let mut fade = Fade::default();
        layers(&mut fade, Some((ClipId::Roll, 1.1)), DT);
        let mut l = [None, None];
        for _ in 0..30 {
            l = layers(&mut fade, None, DT);
        }
        assert_eq!(l, [None, None]);
    }

    #[test]
    fn the_action_names_the_clip_it_plays() {
        let spec = SwingSpec::procedural(0.8);
        let swing = |ds| Swing {
            t: 0.4,
            kind: MoveKind::R2(0),
            spec,
            ds,
            serial: 1,
        };
        let state = |s| Action {
            state: s,
            ..Action::default()
        };
        assert_eq!(playing(&Action::default()), None);
        assert_eq!(
            playing(&state(ActionState::Attack(swing(true)))),
            Some((ClipId::Move(MoveKind::R2(0)), 0.4))
        );
        assert_eq!(
            playing(&state(ActionState::Attack(swing(false)))),
            None,
            "the procedural swing plays the WoW clip"
        );
        let evade = |ds| crate::combat::Evade {
            kind: crate::combat::EvadeKind::Roll,
            t: 0.7,
            dir: Vec3::NEG_Z,
            spec: ROLL,
            ds,
        };
        assert_eq!(
            playing(&state(ActionState::Evade(evade(true)))),
            Some((ClipId::Roll, 0.7))
        );
        assert_eq!(playing(&state(ActionState::Evade(evade(false)))), None);
    }

    #[test]
    fn the_blade_turns_onto_the_dark_souls_axis() {
        // A blade along +Y (top above bottom), to be turned onto -Z.
        let fit = grip_rotation([Vec3::new(0.0, 0.9, 0.0), Vec3::ZERO], Vec3::NEG_Z);
        assert!((fit * Vec3::Y - Vec3::NEG_Z).length() < 1.0e-5);
        // Already there: no turn. Opposite: a half turn that still maps one onto the other.
        let already = grip_rotation([Vec3::Y, Vec3::ZERO], Vec3::Y);
        assert!(alike(already, Quat::IDENTITY));
        let back = grip_rotation([Vec3::Y, Vec3::ZERO], Vec3::NEG_Y);
        assert!((back * Vec3::Y - Vec3::NEG_Y).length() < 1.0e-5);
        // A blade with no length or an axis of zero is left alone.
        assert_eq!(
            grip_rotation([Vec3::ZERO, Vec3::ZERO], Vec3::Y),
            Quat::IDENTITY
        );
        assert_eq!(
            grip_rotation([Vec3::Y, Vec3::ZERO], Vec3::ZERO),
            Quat::IDENTITY
        );
        // The item follows the clips' weight: none, half, all.
        assert_eq!(item_rotation(fit, 0.0), Quat::IDENTITY);
        let half = item_rotation(fit, 0.5);
        let turn = |q: Quat| q.angle_between(Quat::IDENTITY);
        assert!((turn(half) - 0.5 * turn(fit)).abs() < 1.0e-4);
        assert!(alike(item_rotation(fit, 1.0), fit));
        assert!(alike(item_rotation(fit, 3.0), fit), "clamped");
    }

    /// Whether two rotations turn every axis alike (`q` and `-q` are one rotation, and
    /// `angle_between` loses its precision near zero).
    fn alike(a: Quat, b: Quat) -> bool {
        [Vec3::X, Vec3::Y, Vec3::Z]
            .into_iter()
            .all(|v| (a * v - b * v).length() < 1.0e-5)
    }

    #[test]
    fn the_grip_override_reads_degrees_about_the_hands_axes() {
        let q = parse_grip_override("90, 0, 0").unwrap();
        assert!((q * Vec3::Y - Vec3::Z).length() < 1.0e-5);
        assert_eq!(parse_grip_override("90,0"), None);
        assert_eq!(parse_grip_override("a,b,c"), None);
        assert_eq!(parse_grip_override("1,2,nan"), None);
    }

    /// A cache directory under the temp dir.
    fn cache_dir_named(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wow-souls-dsclip-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("human_male")).unwrap();
        std::fs::create_dir_all(dir.join("c0000")).unwrap();
        dir
    }

    fn put(dir: &Path, clip: &wscl::Clip) {
        for sub in ["human_male", "c0000"] {
            let path = dir.join(sub).join(format!("{}.wscl", clip.name));
            clip.write(&mut std::fs::File::create(path).unwrap())
                .unwrap();
        }
    }

    /// A whole cache: the roll, the six moves (each with its own price, `10 + index` stamina)
    /// and a manifest, minus any line `without` names.
    fn whole_cache(name: &str, without: Option<&str>) -> PathBuf {
        let dir = cache_dir_named(name);
        put(&dir, &{
            let mut r = move_clip("a00_0710", (0.0, 0.1), [0.5, 0.5, 0.5]);
            r.hits.clear();
            r.flags.push(wscl::FlagWindow {
                flag: wscl::FLAG_INVINCIBLE,
                start: 0.0,
                end: 0.3,
            });
            r
        });
        let mut manifest = format!(
            "wscl-cache {CACHE_VERSION}\nextractor {}\nroll a00_0710\nclass straight_sword\n",
            wscl::EXTRACTOR_VERSION
        );
        for (i, kind) in MoveKind::ALL.into_iter().enumerate() {
            let clip = format!("clip_{}", kind.name());
            let at = 0.1 * (i as f32 + 1.0);
            put(&dir, &move_clip(&clip, (at, at + 0.1), [0.6, 0.7, 0.8]));
            if without != Some(kind.name()) {
                manifest += &format!(
                    "move {} {clip} judge 0 stamina {} mult 1.{i}0\n",
                    kind.name(),
                    10 + i
                );
            }
        }
        if without != Some("grip") {
            manifest += "grip 0.1 0.2 0.3 0.0 1.0 0.0\n";
        }
        std::fs::write(dir.join("manifest.txt"), manifest).unwrap();
        dir
    }

    #[test]
    fn a_whole_cache_loads_the_roll_and_six_moves() {
        let dir = whole_cache("whole", None);
        let c = load(&dir).unwrap_or_else(|e| panic!("{e}"));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(c.roll.wow.name, "a00_0710");
        let m = &c.moves.moveset;
        assert!(m.ds && m.has_heavy());
        // Each move got its own clip's windows and its own manifest price.
        assert_eq!(m.r1[0].hit, (0.1, 0.2));
        assert_eq!(m.r1[1].cost, 11.0);
        assert_eq!(m.r1[2].cost, 12.0);
        let r2 = m.r2.unwrap();
        assert_eq!((r2[0].cost, r2[1].cost), (13.0, 14.0));
        let roll_attack = m.roll_attack.unwrap();
        assert_eq!(roll_attack.cost, 15.0);
        assert!((roll_attack.mult - 1.5).abs() < 1.0e-6);
        assert_eq!(roll_attack.chain_from, 0.6);
        assert_eq!(c.moves.clip(MoveKind::R2(1)).unwrap().wow.name, "clip_r2_2");
        assert_eq!(c.moves.grip.hand, Vec3::new(0.1, 0.2, 0.3));
        assert_eq!(c.moves.grip.weapon, Vec3::Y);
    }

    #[test]
    fn a_cache_lacking_a_move_or_the_grip_is_refused() {
        for (name, without, why) in [
            ("no-move", "r2_2", "manifest lacks move r2_2"),
            ("no-grip", "grip", "manifest has no grip line"),
        ] {
            let dir = whole_cache(name, Some(without));
            let err = load(&dir).err().unwrap_or_else(|| panic!("{name} loads"));
            std::fs::remove_dir_all(&dir).ok();
            assert!(err.contains(why), "{name}: {err}");
        }
        assert!(parse_move("move r1_1 a23_3000 judge 0 stamina x mult 1.0").is_err());
        assert!(parse_move("move r9_9 a23_3000 judge 0 stamina 20 mult 1.0").is_err());
        assert!(parse_grip_line("grip 1 2 3").is_err());
    }

    /// A cache dir under the temp dir with `manifest` as its manifest.
    fn manifest_dir(name: &str, manifest: &str) -> PathBuf {
        let dir = cache_dir_named(name);
        std::fs::write(dir.join("manifest.txt"), manifest).unwrap();
        dir
    }

    #[test]
    fn the_manifest_must_carry_this_games_versions() {
        let roll = "roll a00_0710\n";
        let cases = [
            ("no-versions", roll.to_string(), "manifest lacks a version"),
            (
                "no-extractor",
                format!("wscl-cache {CACHE_VERSION}\n{roll}"),
                "manifest lacks a version",
            ),
            (
                "old-extractor",
                format!("wscl-cache {CACHE_VERSION}\nextractor 1\n{roll}"),
                "extractor version 1, expected",
            ),
            (
                "old-cache",
                format!(
                    "wscl-cache 0\nextractor {}\n{roll}",
                    wscl::EXTRACTOR_VERSION
                ),
                "cache version 0",
            ),
        ];
        for (name, manifest, why) in cases {
            let dir = manifest_dir(name, &manifest);
            let err = load(&dir).err().unwrap_or_else(|| panic!("{name} loads"));
            std::fs::remove_dir_all(&dir).ok();
            assert!(err.starts_with(why), "{name}: {err}");
        }
        // Both versions right: it gets as far as the clips.
        let manifest = format!(
            "wscl-cache {CACHE_VERSION}\nextractor {}\n{roll}",
            wscl::EXTRACTOR_VERSION
        );
        let dir = manifest_dir("ok", &manifest);
        let err = load(&dir).err().unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(err.contains("a00_0710.wscl"), "{err}");
    }
}
```

- [ ] **Step 2: Write `dsview.rs`**

**Create** `crates/wow-souls/src/dsview.rs`:
```rust
//! `WOWSOULS_DSVIEW=<clip>`: a cached Dark Souls clip on the DS skeleton (gizmo bones and the DS
//! blade, 1.5 yd to the avatar's right) beside the WoW human playing the retargeted clip, in
//! lockstep, looping, in place (no root motion). `1` is the roll; a clip name (`a23_3000`) or a
//! move's name (`r1_1`) is a sword move. With `WOWSOULS_OBSERVE` and `WOWSOULS_SHOT` it makes the
//! comparison frames. Every 0.1 s it logs the clip time and the root distance covered; once per
//! loop it logs how far the WoW sword's blade strays from the Dark Souls blade.

use bevy::prelude::*;

use crate::actor::held::HeldBuilt;
use crate::avatar::Avatar;
use crate::combat::MoveKind;
use crate::dsclip::{clips_of, progress, sample, ClipId, DsMoves, DsRoll};
use crate::motor::Motor;

/// The DS skeleton's `R_Weapon` bone in `c0000`'s clips (`dsr-cache`'s install test pins it).
pub const DS_WEAPON_BONE: usize = 56;
/// The DS blade as drawn on the DS skeleton (yd).
const DS_BLADE_LENGTH: f32 = 1.0;

/// The side-by-side's clip and time (s); once the cache has loaded and the name is known,
/// [`crate::dsclip`] poses the avatar from it.
#[derive(Resource, Debug)]
pub struct DsView {
    want: String,
    clip: Option<ClipId>,
    pub t: f32,
}

impl DsView {
    /// The clip on show and its time.
    pub fn playing(&self) -> Option<(ClipId, f32)> {
        self.clip.map(|c| (c, self.t))
    }
}

/// The clip `want` names: `1` or the roll's clip name, a move's clip name or its table name.
fn resolve(want: &str, roll: &DsRoll, moves: &DsMoves) -> Option<ClipId> {
    if want == "1" || want == roll.wow.name {
        return Some(ClipId::Roll);
    }
    MoveKind::ALL
        .into_iter()
        .find(|k| k.name() == want || moves.clip(*k).is_some_and(|m| m.wow.name == want))
        .map(ClipId::Move)
}

/// The angle (degrees) between the WoW sword's blade `wow` and the Dark Souls blade, both in the
/// body's frame: the DS weapon bone's rotation `ds_weapon` turning its blade axis `axis`.
pub fn blade_error_deg(wow: Vec3, ds_weapon: Quat, axis: Vec3) -> f32 {
    wow.angle_between(ds_weapon * axis).to_degrees()
}

/// The blade errors seen over one loop of the clip.
#[derive(Default)]
struct BladeStats {
    /// The clip time of the first frame measured: a loop that starts late (the held sword was not
    /// built yet) is not reported.
    start: Option<f32>,
    worst: f32,
    at: f32,
    sum: f32,
    frames: u32,
}

impl BladeStats {
    fn add(&mut self, t: f32, err: f32) {
        self.start.get_or_insert(t);
        if err > self.worst {
            self.worst = err;
            self.at = t;
        }
        self.sum += err;
        self.frames += 1;
    }

    fn mean(&self) -> f32 {
        self.sum / self.frames.max(1) as f32
    }
}

pub fn plugin(app: &mut App) {
    if let Ok(want) = std::env::var("WOWSOULS_DSVIEW") {
        app.insert_resource(DsView {
            want,
            clip: None,
            t: 0.0,
        })
        // `measure` reads the pose the last frame drew, at the time `advance` has not yet moved;
        // `draw` runs once the frame's pose is placed, so the overlay is not a frame behind.
        .add_systems(Update, (measure, advance).chain())
        .add_systems(
            PostUpdate,
            draw.after(benilla_world::billboard::BillboardPlace),
        );
    }
}

fn advance(
    time: Res<Time>,
    roll: Option<Res<DsRoll>>,
    moves: Option<Res<DsMoves>>,
    mut view: ResMut<DsView>,
    mut next_log: Local<f32>,
    mut warned: Local<bool>,
) {
    let (Some(roll), Some(moves)) = (roll, moves) else {
        return;
    };
    if view.clip.is_none() {
        view.clip = resolve(&view.want, &roll, &moves);
        if view.clip.is_none() {
            if !*warned {
                *warned = true;
                warn!("dsview: no cached clip is called {:?}", view.want);
            }
            return;
        }
    }
    let Some((wow, _)) = view.clip.and_then(|c| clips_of(&roll, &moves, c)) else {
        return;
    };
    let d = wow.duration().max(1.0e-3);
    view.t = (view.t + time.delta_secs()) % d;
    *next_log -= time.delta_secs();
    if *next_log <= 0.0 {
        *next_log = 0.1;
        info!(
            "dsview {} t={:.3}/{d:.3} root={:.2} yd",
            wow.name,
            view.t,
            progress(wow, view.t)
        );
    }
}

/// Each frame, the drawn WoW blade (the held item's `$WTT`/`$WTB` markers through its pose in the
/// world) against the DS blade at the same clip time, in the body's frame; one line per loop that
/// was measured from its start, with the body standing (before that it hangs in its spawn pose).
fn measure(
    view: Res<DsView>,
    roll: Option<Res<DsRoll>>,
    moves: Option<Res<DsMoves>>,
    avatar: Query<(&GlobalTransform, &HeldBuilt, &Motor), With<Avatar>>,
    items: Query<&GlobalTransform>,
    mut stats: Local<BladeStats>,
    mut last_t: Local<f32>,
) {
    let (Some(roll), Some(moves), Some((clip, t))) = (roll, moves, view.playing()) else {
        return;
    };
    let Some((wow, ds)) = clips_of(&roll, &moves, clip) else {
        return;
    };
    // The loop wrapped since the last frame: report it, and start over.
    if t < *last_t {
        if stats.start.is_some_and(|start| start < 0.1) {
            info!(
                "dsview {} blade error: worst {:.1} deg at t={:.3} s, mean {:.1} deg over {} frames",
                wow.name,
                stats.worst,
                stats.at,
                stats.mean(),
                stats.frames
            );
        }
        *stats = BladeStats::default();
    }
    *last_t = t;
    let Ok((body, held, motor)) = avatar.single() else {
        return;
    };
    let (true, Some([top, bottom]), Ok(item)) = (motor.grounded, held.blade, items.get(held.root))
    else {
        return;
    };
    let drawn = item.affine().transform_vector3(top - bottom);
    let wow_blade = body.compute_transform().rotation.inverse() * drawn.normalize_or_zero();
    let ds_weapon = sample(ds, t, DS_WEAPON_BONE).1;
    stats.add(t, blade_error_deg(wow_blade, ds_weapon, moves.grip.weapon));
}

fn draw(
    roll: Option<Res<DsRoll>>,
    moves: Option<Res<DsMoves>>,
    view: Res<DsView>,
    avatar: Query<(&Transform, Option<&HeldBuilt>), With<Avatar>>,
    items: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    let (Some(roll), Some(moves), Ok((body, held))) = (roll, moves, avatar.single()) else {
        return;
    };
    let Some((_, ds)) = view.clip.and_then(|c| clips_of(&roll, &moves, c)) else {
        return;
    };
    let origin = body.translation + *body.right() * 1.5;
    let at = |b: usize| origin + body.rotation * sample(ds, view.t, b).0;
    let gold = Color::srgb(0.95, 0.75, 0.2);
    for (i, &p) in ds.parents.iter().enumerate() {
        let joint = at(i);
        gizmos.sphere(Isometry3d::from_translation(joint), 0.025, gold);
        if let Ok(p) = usize::try_from(p) {
            gizmos.line(at(p), joint, gold);
        }
    }
    // The DS blade: the weapon bone's blade axis from the weapon bone, and again from the base of
    // the WoW sword (magenta): where the drawn blade should lie.
    if DS_WEAPON_BONE < ds.parents.len() {
        let (_, rot, _) = sample(ds, view.t, DS_WEAPON_BONE);
        let dir = body.rotation * (rot * moves.grip.weapon);
        let pale = Color::srgb(1.0, 0.95, 0.6);
        let tip = at(DS_WEAPON_BONE) + dir * DS_BLADE_LENGTH;
        gizmos.line(at(DS_WEAPON_BONE), tip, pale);
        gizmos.sphere(Isometry3d::from_translation(tip), 0.03, pale);
        if let Some((Some([top, bottom]), Ok(item))) = held.map(|h| (h.blade, items.get(h.root))) {
            let base = item.transform_point(bottom);
            let length = item.affine().transform_vector3(top - bottom).length();
            let magenta = Color::srgb(1.0, 0.2, 0.9);
            gizmos.line(base, base + dir * length, magenta);
            gizmos.sphere(
                Isometry3d::from_translation(base + dir * length),
                0.03,
                magenta,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blade_error_is_the_angle_between_the_two_blades() {
        let none = Quat::IDENTITY;
        // The same direction: no error. A quarter turn and a half turn.
        assert!(blade_error_deg(Vec3::Y, none, Vec3::Y) < 1.0e-3);
        assert!((blade_error_deg(Vec3::X, none, Vec3::Y) - 90.0).abs() < 1.0e-3);
        assert!((blade_error_deg(Vec3::NEG_Y, none, Vec3::Y) - 180.0).abs() < 1.0e-3);
        // The weapon bone's rotation turns the axis first: +Y turned 90 deg about Z is -X.
        let quarter = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        assert!(blade_error_deg(Vec3::NEG_X, quarter, Vec3::Y) < 1.0e-3);
        assert!((blade_error_deg(Vec3::Y, quarter, Vec3::Y) - 90.0).abs() < 1.0e-3);
    }

    #[test]
    fn the_stats_keep_the_worst_frame_and_the_mean() {
        let mut s = BladeStats::default();
        assert_eq!(s.mean(), 0.0);
        for (t, e) in [(0.0, 2.0), (0.1, 9.0), (0.2, 4.0)] {
            s.add(t, e);
        }
        assert_eq!((s.worst, s.at, s.frames, s.start), (9.0, 0.1, 3, Some(0.0)));
        assert!((s.mean() - 5.0).abs() < 1.0e-6);
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo fmt --all && cargo test -j 4 -p wow-souls dsclip`
Expected: PASS: the sampling, spec, blend, three layer tests, action-names, blade, override, whole-cache, refused-cache and manifest tests.
Run: `cargo test -j 4 -p wow-souls dsview`
Expected: PASS: the blade-error angle and the loop statistics.
Run: `cargo test -j 4 -p wow-souls`
Expected: PASS (the whole crate).

- [ ] **Step 4: Run the game against the real cache (unattended)**

(The cache in `cache/dsr` is the one Task 5 wrote.)

```bash
WOWSOULS_CHECK=8 WOWSOULS_TRACE=1 timeout 120 ./target/debug/wow-souls.exe 2>&1 | grep -E "dsclip|WOWSOULS_CHECK" | sed 's/\x1b\[[0-9;]*m//g' | cut -c1-230
```
Run it after `cargo build -j 4 -p wow-souls`. Expected: one `dsclip: Dark Souls roll a00_0710 (2.033 s, 4.47 yd, invulnerable 0.000-0.367 s, cancel from 1.100 s)` line, then these six, in this order, to the digit:

```
dsclip: r1_1 a23_3000 (2.500 s, hit 0.533-0.667, chain 0.767, evade 0.900, move 1.033, 1.98 yd, stamina 20, x1.00)
dsclip: r1_2 a23_3001 (2.233 s, hit 0.467-0.567, chain 0.733, evade 0.900, move 1.000, 1.42 yd, stamina 20, x1.00)
dsclip: r1_3 a23_3002 (2.500 s, hit 0.467-0.600, chain 0.767, evade 0.900, move 1.033, 1.98 yd, stamina 20, x1.00)
dsclip: r2_1 a23_3300 (2.533 s, hit 0.733-0.867, chain 1.300, evade 1.300, move 1.367, 1.98 yd, stamina 45, x1.35)
dsclip: r2_2 a23_3310 (1.900 s, hit 0.333-0.467, chain 0.933, evade 0.933, move 1.200, 0.94 yd, stamina 35, x1.25)
dsclip: roll_attack a23_3900 (2.100 s, hit 0.233-0.333, chain 0.700, evade 0.767, move 0.867, 1.42 yd, stamina 20, x0.90)
```

then `dsclip: the held blade turns 26 deg onto the Dark Souls blade axis (fitted)` (the angle is how far the WoW sword sits from the Dark Souls grip; Task 11 judges whether the sword looks right) and `WOWSOULS_CHECK ok`. These are the numbers `docs/HANDOVER.md`'s table (Task 12) records; a different number means the cache or the loader is wrong.

- [ ] **Step 5: Checkpoint**

Run `cargo fmt --all -- --check && cargo test -j 4 --workspace`. Expected: green (wow-souls 178 tests). Clippy waits for Task 9: until `act` starts a Dark Souls swing, `Moveset::has_heavy`, `Press::Heavy` and `DsMoves::moveset` are unused, and `cargo build` says so in exactly those three dead-code warnings.

---

### Task 9: `act` plays the Dark Souls moves; heavy input; help line

`act` takes the moveset from the cache when there is one, spends each move's price, and drives the body by the clip's root motion (recoil included: a step back is a negative speed along the facing, which the motor allows). R, the right mouse button and the right trigger press heavy; `WOWSOULS_HEAVY` scripts it. The help line names the keys.

**Files:**
- Modify: `crates/wow-souls/src/combat/mod.rs`
- Modify: `crates/wow-souls/src/motor/mod.rs`
- Modify: `crates/wow-souls/src/hud.rs`

- [ ] **Step 1: Write the failing test**

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
        assert!(evade_for(keys_with(&[]), Some(GamepadButton::East)));
        assert!(!evade_for(keys_with(&[]), Some(GamepadButton::South)));
    }
}
```
New:
```rust
        assert!(evade_for(keys_with(&[]), Some(GamepadButton::East)));
        assert!(!evade_for(keys_with(&[]), Some(GamepadButton::South)));
    }

    fn heavy_for(
        keys: ButtonInput<KeyCode>,
        mouse: ButtonInput<MouseButton>,
        captured: bool,
        pad: Option<GamepadButton>,
    ) -> bool {
        use crate::pad::testkit::{read, world_with_pad};
        let mut world = match pad {
            Some(b) => world_with_pad(move |p| {
                p.digital_mut().press(b);
            }),
            None => World::new(),
        };
        read(&mut world, move |pads| {
            heavy_pressed(&keys, &mouse, pads, captured)
        })
    }

    #[test]
    fn heavy_is_r_the_right_mouse_or_the_right_trigger() {
        let none = || ButtonInput::<KeyCode>::default();
        let no_mouse = ButtonInput::<MouseButton>::default;
        assert!(heavy_for(keys_with(&[KeyCode::KeyR]), no_mouse(), false, None));
        // The right mouse counts only with the cursor captured, as the left does.
        let rmb = || mouse_with(&[MouseButton::Right]);
        assert!(!heavy_for(none(), rmb(), false, None));
        assert!(heavy_for(none(), rmb(), true, None));
        assert!(heavy_for(
            none(),
            no_mouse(),
            false,
            Some(GamepadButton::RightTrigger2)
        ));
        // The right bumper is the light attack, not the heavy one.
        assert!(!heavy_for(
            none(),
            no_mouse(),
            true,
            Some(GamepadButton::RightTrigger)
        ));
        assert!(!heavy_for(none(), no_mouse(), true, None));
    }

    #[test]
    fn root_motion_steps_back_along_the_facing_without_turning_it() {
        let near = |a: f32, b: f32| (a - b).abs() < 1.0e-3;
        let fwd = Vec3::NEG_Z;
        let go = root_step(fwd, 0.1, 0.02);
        assert_eq!(go.dir, fwd);
        assert!(near(go.speed, 5.0), "{go:?}");
        // Recoil: the same direction, a negative speed.
        let back = root_step(fwd, -0.1, 0.02);
        assert_eq!(back.dir, fwd);
        assert!(near(back.speed, -5.0), "{back:?}");
        // No step: no push at all.
        assert_eq!(root_step(fwd, 0.0, 0.02).dir, Vec3::ZERO);
        // A hitch longer than the motor integrates still delivers its step.
        let hitch = root_step(fwd, 0.2, 0.5);
        assert!(near(hitch.speed * crate::motor::MAX_STEP, 0.2), "{hitch:?}");
        // A frame of no time asks for nothing.
        assert_eq!(root_step(fwd, 0.1, 0.0).speed, 0.0);
    }
}
```

**Edit** `crates/wow-souls/src/motor/mod.rs`:

Old:
```rust
    #[test]
    fn a_standing_jump_reaches_the_reference_apex_and_lands() {
```
New:
```rust
    #[test]
    fn a_negative_speed_backs_the_body_along_its_direction() {
        fn back_x(_: usize) -> MoveIntent {
            MoveIntent {
                dir: Vec3::X,
                speed: -RUN_SPEED,
                jump: false,
            }
        }
        let frames = run(floor(false), Vec3::new(-10.0, 0.0, 0.0), 60, back_x);
        let (feet, o, _) = frames.last().unwrap();
        assert!(o.grounded);
        assert!(
            (feet.x - (-10.0 - RUN_SPEED)).abs() < 0.05,
            "one second backing at a run should cover {RUN_SPEED} yd the other way, ended at {feet:?}"
        );
    }

    #[test]
    fn a_standing_jump_reaches_the_reference_apex_and_lands() {
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -j 4 -p wow-souls heavy_is`
Expected: FAIL to compile: `cannot find function heavy_pressed` (and `root_step`).

- [ ] **Step 3: Heavy input and the scripted heavy press**

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
/// The evade press this frame: Space (Shift+Space is the jump), or gamepad East.
```
New:
```rust
/// The heavy press this frame: R, RMB with the cursor captured, or the right trigger (RT).
fn heavy_pressed(
    keys: &ButtonInput<KeyCode>,
    buttons: &ButtonInput<MouseButton>,
    gamepads: &Query<&Gamepad>,
    captured: bool,
) -> bool {
    keys.just_pressed(KeyCode::KeyR)
        || (captured && buttons.just_pressed(MouseButton::Right))
        || gamepads
            .iter()
            .any(|p| p.just_pressed(GamepadButton::RightTrigger2))
}

/// The evade press this frame: Space (Shift+Space is the jump), or gamepad East.
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
/// `WOWSOULS_ROLL=<s>[,<every s>]`: press evade on a [`Schedule`]; `WOWSOULS_DRIVE` aims it.
```
New:
```rust
/// `WOWSOULS_HEAVY=<s>[,<every s>]`: press heavy on a [`Schedule`] (nothing without the Dark
/// Souls moveset, which has the only heavy attack).
#[derive(Resource, Debug, Clone, Copy)]
pub struct ScriptedHeavy(pub Schedule);

/// `WOWSOULS_ROLL=<s>[,<every s>]`: press evade on a [`Schedule`]; `WOWSOULS_DRIVE` aims it.
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
    if let Some(s) = schedule("WOWSOULS_ROLL") {
        app.insert_resource(ScriptedRoll(s));
    }
```
New:
```rust
    if let Some(s) = schedule("WOWSOULS_HEAVY") {
        app.insert_resource(ScriptedHeavy(s));
    }
    if let Some(s) = schedule("WOWSOULS_ROLL") {
        app.insert_resource(ScriptedRoll(s));
    }
```

**Edit** `crates/wow-souls/src/motor/mod.rs`:

Old:
```rust
    /// Speed at full push (yd/s).
    pub speed: f32,
```
New:
```rust
    /// Speed at full push (yd/s); a negative one backs the body along `dir` (a clip's recoil).
    pub speed: f32,
```

- [ ] **Step 4: `act` reads the moveset and plays the move**

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
/// Where the last evade started and when (real s), for the trace's distance line.
#[derive(Default)]
struct EvadeTrace(Option<(EvadeKind, Vec3, f32)>);

impl EvadeTrace {
    fn close(&mut self, at: Vec3, now: f32) {
        if let Some((kind, from, t0)) = self.0.take() {
            let d = at - from;
            info!(
                "combat: {kind:?} moved {:.2} yd in {:.2} s",
                Vec3::new(d.x, 0.0, d.z).length(),
                now - t0
            );
        }
    }
}
```
New:
```rust
/// A body's intent for one frame's share `step` (yd) of a clip's root motion along `dir`. A
/// negative step (a sword's recoil) backs the body along `dir` through a negative speed: the body
/// turns toward its `dir`, so `dir` stays the way the clip faces. The motor integrates at most
/// [`crate::motor::MAX_STEP`] of a frame, so the step is spread over that much.
fn root_step(dir: Vec3, step: f32, dt: f32) -> MoveIntent {
    let secs = dt.min(crate::motor::MAX_STEP);
    MoveIntent {
        dir: if step == 0.0 { Vec3::ZERO } else { dir },
        speed: if secs > 0.0 { step / secs } else { 0.0 },
        jump: false,
    }
}

/// Where the last evade or move started and when (real s), for the trace's distance line.
#[derive(Default)]
struct ActionTrace(Option<(String, Vec3, f32)>);

impl ActionTrace {
    fn close(&mut self, at: Vec3, now: f32) {
        if let Some((what, from, t0)) = self.0.take() {
            let d = at - from;
            info!(
                "combat: {what} moved {:.2} yd in {:.2} s",
                Vec3::new(d.x, 0.0, d.z).length(),
                now - t0
            );
        }
    }
}
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
    scripted: (Option<Res<ScriptedAttack>>, Option<Res<ScriptedRoll>>),
    mut next_scripted: Local<(Option<f32>, Option<f32>)>,
    mut evade_trace: Local<EvadeTrace>,
```
New:
```rust
    scripted: (
        Option<Res<ScriptedAttack>>,
        Option<Res<ScriptedHeavy>>,
        Option<Res<ScriptedRoll>>,
    ),
    mut next_scripted: Local<(Option<f32>, Option<f32>, Option<f32>)>,
    mut action_trace: Local<ActionTrace>,
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
    ds_roll: Option<Res<crate::dsclip::DsRoll>>,
    mut avatar: Query<
```
New:
```rust
    ds: (
        Option<Res<crate::dsclip::DsRoll>>,
        Option<Res<crate::dsclip::DsMoves>>,
    ),
    mut avatar: Query<
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
    let dt = time.delta_secs();
    let now = real.elapsed_secs();
    // Live presses only drive the body under the orbit camera, never in an unattended run.
```
New:
```rust
    let (ds_roll, ds_moves) = ds;
    let dt = time.delta_secs();
    let now = real.elapsed_secs();
    // Live presses only drive the body under the orbit camera, never in an unattended run.
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
    let evade = dodge_stick.is_some()
        || (live && evade_pressed(&keys, &gamepads))
        || scripted
            .1
            .is_some_and(|s| s.0.due(&mut next_scripted.1, now));
    let press = if attack {
        Some(Press::Attack)
    } else if evade {
        Some(Press::Evade)
    } else {
        None
    };
    let clip = anims.and_then(|a| a.find(ATTACK_1H));
    let swing_len = clip.map_or(0.8, |c| c.duration / ATTACK_RATE);
    let input = StepInput {
        press,
        grounded: m.grounded,
        moves: Moveset::procedural(swing_len),
```
New:
```rust
    let evade = dodge_stick.is_some()
        || (live && evade_pressed(&keys, &gamepads))
        || scripted
            .2
            .is_some_and(|s| s.0.due(&mut next_scripted.2, now));
    let clip = anims.and_then(|a| a.find(ATTACK_1H));
    let swing_len = clip.map_or(0.8, |c| c.duration / ATTACK_RATE);
    // The swings this body can make: Dark Souls' from the cache, else the procedural one.
    let moves = ds_moves
        .as_deref()
        .map_or_else(|| Moveset::procedural(swing_len), |d| d.moveset);
    // A heavy press needs a heavy attack, which only the Dark Souls moveset has.
    let heavy = moves.has_heavy()
        && ((live && heavy_pressed(&keys, &buttons, &gamepads, captured))
            || scripted
                .1
                .is_some_and(|s| s.0.due(&mut next_scripted.1, now)));
    let press = if attack {
        Some(Press::Attack)
    } else if heavy {
        Some(Press::Heavy)
    } else if evade {
        Some(Press::Evade)
    } else {
        None
    };
    let input = StepInput {
        press,
        grounded: m.grounded,
        moves,
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
        Decision::Swing { kind, chained } => {
            stamina.spend(input.moves.spec(kind).map_or(ATTACK_COST, |s| s.cost));
            if let (Some(c), Some(mut p), Some(mut tr)) = (clip, player, transitions) {
                play_once(&mut p, &mut tr, c.node, ATTACK_RATE, chained);
            }
            if trace {
                evade_trace.close(xf.translation, now);
                info!(
                    "combat: move {} ({swing_len:.2} s), stamina {:.0}",
                    kind.name(),
                    stamina.cur
                );
            }
        }
```
New:
```rust
        Decision::Swing { kind, chained } => {
            let spec = moves
                .spec(kind)
                .expect("a swing that started is in the moveset");
            stamina.spend(spec.cost);
            // The procedural swing plays the WoW clip; a Dark Souls move poses the rig itself
            // ([`crate::dsclip`]).
            if !moves.ds {
                if let (Some(c), Some(mut p), Some(mut tr)) = (clip, player, transitions) {
                    play_once(&mut p, &mut tr, c.node, ATTACK_RATE, chained);
                }
            }
            if trace {
                action_trace.close(xf.translation, now);
                action_trace.0 = Some((kind.name().to_string(), xf.translation, now));
                info!(
                    "combat: move {} ({:.2} s, hit {:.3}-{:.3}), stamina {:.0}",
                    kind.name(),
                    spec.duration,
                    spec.hit.0,
                    spec.hit.1,
                    stamina.cur
                );
            }
        }
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
            if trace {
                evade_trace.close(xf.translation, now);
                evade_trace.0 = Some((kind, xf.translation, now));
                info!(
                    "combat: {kind:?} toward {:.2?}, stamina {:.0}",
                    e.dir, stamina.cur
                );
            }
```
New:
```rust
            if trace {
                action_trace.close(xf.translation, now);
                action_trace.0 = Some((format!("{kind:?}"), xf.translation, now));
                info!(
                    "combat: {kind:?} toward {:.2?}, stamina {:.0}",
                    e.dir, stamina.cur
                );
            }
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
        Decision::Ended => {
            if trace {
                evade_trace.close(xf.translation, now);
                info!("combat: action ended");
            }
        }
```
New:
```rust
        Decision::Ended => {
            if trace {
                action_trace.close(xf.translation, now);
                info!("combat: action ended");
            }
        }
```

**Edit** `crates/wow-souls/src/combat/mod.rs`:

Old:
```rust
        ActionState::Attack(s) => {
            // The swing carries the body a little forward through its wind-up and strike, but not
            // into an enemy already in reach: pressed point-blank, the arc passes over a short one.
            let in_reach = targets.iter().any(|(at, hurt)| {
                enemy_in_reach(xf.translation, *xf.forward(), at.translation(), hurt.radius)
            });
            let lunge = !in_reach && !matches!(s.phase(), Some(SwingPhase::Recovery) | None);
            *intent = MoveIntent {
                dir: if lunge { *xf.forward() } else { Vec3::ZERO },
                speed: LUNGE_SPEED,
                jump: false,
            };
        }
```
New:
```rust
        ActionState::Attack(s) => {
            let ds_clip = ds_moves
                .as_deref()
                .and_then(|d| d.clip(s.kind))
                .filter(|_| s.ds);
            *intent = match ds_clip {
                // A Dark Souls move: this frame's share of its clip's root motion, along the
                // facing, recoil included. The motor stops it at a body.
                Some(c) => {
                    let prev = (s.t - dt).max(0.0);
                    let step = (crate::dsclip::progress(&c.wow, s.t)
                        - crate::dsclip::progress(&c.wow, prev))
                        * xf.scale.y;
                    root_step(*xf.forward(), step, dt)
                }
                // The procedural swing carries the body a little forward through its wind-up and
                // strike, but not into an enemy already in reach: pressed point-blank, the arc
                // passes over a short one.
                None => {
                    let in_reach = targets.iter().any(|(at, hurt)| {
                        enemy_in_reach(xf.translation, *xf.forward(), at.translation(), hurt.radius)
                    });
                    let lunge =
                        !in_reach && !matches!(s.phase(), Some(SwingPhase::Recovery) | None);
                    MoveIntent {
                        dir: if lunge { *xf.forward() } else { Vec3::ZERO },
                        speed: LUNGE_SPEED,
                        jump: false,
                    }
                }
            };
        }
```

- [ ] **Step 5: The help line names the heavy keys**

**Edit** `crates/wow-souls/src/hud.rs`:

Old:
```rust
        "keys: WASD move  Ctrl walk  F or LMB attack  Space roll  Shift+Space jump  Q lock  arrows look  F1 free cam  F2 capsule  Esc cursor  F3 hide\n{}   (A jump  B roll  RB attack  RS lock  left stick move  right stick look)",
```
New:
```rust
        "keys: WASD move  Ctrl walk  F or LMB light  R or RMB heavy  Space roll  Shift+Space jump  Q lock  arrows look  F1 free cam  F2 capsule  Esc cursor  F3 hide\n{}   (A jump  B roll  RB light  RT heavy  RS lock  left stick move  right stick look)",
```

- [ ] **Step 6: Run the tests**

Run: `cargo fmt --all && cargo test -j 4 -p wow-souls`
Expected: PASS (three more tests: 181 in all).

- [ ] **Step 7: Checkpoint**

Run the gates (clippy included). Expected: all green. If clippy reports `type_complexity` or `too_many_arguments` on `act` or `pose`, the workspace lints already allow both; if it reports anything else, fix it where reported.

---

### Task 10: Live check against the data (unattended)

Nothing is written here: the game is run and what it logs is held against the table in the spec. Every run is unattended (the window opens unfocused and never grabs the mouse; see `run_mode.rs`). The numbers under "Expected" were measured when this plan was written; frame quantisation moves a time by about ±0.02 s and a travel by ±0.01 yd, nothing else. The results go into `docs/HANDOVER.md` in Task 12.

**Files:** none.

- [ ] **Step 1: Build, warm up, and write the helper**

Run: `cargo build -j 4 -p wow-souls`

Run once, to warm the file cache and to see that the bare game is clean: `WOWSOULS_CHECK=25 ./target/debug/wow-souls.exe 2>&1 | grep -E "WOWSOULS_CHECK|WARN|ERROR"`
Expected: `WOWSOULS_CHECK ok` and nothing else (no warning: the cache is whole and fresh).

Scripted presses (`WOWSOULS_ATTACK`, `_HEAVY`, `_ROLL`) count seconds from the start of the run, and the body can act only once the world has loaded: about 2.4 s into a warm run, twice that on the first run after a build or a reboot. A press scheduled before the body can act fires on the first frame it can, together with every other press already due, and only one of them wins (attack, then heavy, then evade): the scenario is then shifted. The commands below are timed for a warm load. A shifted run shows in the log (the first line comes later than its press time, a press is missing): run it again.

Write `$SCRATCH/live.sh` (outside the repo; `$SCRATCH` is any scratch folder) and `source` it from the repo root:

```bash
#!/bin/bash
# One unattended run of the game with the given WOWSOULS_* settings, filtered to the lines that
# matter. Usage: live WOWSOULS_CHECK=15 WOWSOULS_ATTACK=3.0,2.2 ...
export DSR_DIR='C:/Spel/Dark Souls Remastered' WOW_DATA='C:/Spel/World of Warcraft 1.12/Data'
GAME="${GAME:-./target/debug/wow-souls.exe}"
# Flat ground with the kobold sent away: the default anchor has a 0.8 yd ridge 3 yd north of it.
FLAT="WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8960,-136"
live() {
    env WOWSOULS_TRACE=1 "$@" timeout 120 "$GAME" 2>&1 \
        | sed 's/\x1b\[[0-9;]*m//g' \
        | grep -E "combat: (move|Roll|Backstep|action ended|.* moved)|hits: hit|npc: kobold dies|player: struck|dsclip: no|WOWSOULS_CHECK" \
        | sed -E 's/^[0-9T:.Z-]+ +(INFO|WARN) [a-z_:]+: //' \
        | cut -c1-200
}
```

- [ ] **Step 2: The R1 chain and its lunge**

Run: `live $FLAT WOWSOULS_CHECK=15 WOWSOULS_ATTACK=3.0,2.2`

Expected:
```
combat: move r1_1 (2.50 s, hit 0.533-0.667), stamina 80
combat: r1_1 moved 1.97 yd in 2.20 s
combat: move r1_2 (2.23 s, hit 0.467-0.567), stamina 60
combat: r1_2 moved 1.42 yd in 2.20 s
combat: move r1_3 (2.50 s, hit 0.467-0.600), stamina 40
combat: r1_3 moved 1.98 yd in 2.20 s
combat: move r1_1 (2.50 s, hit 0.533-0.667), stamina 20
combat: r1_1 moved 1.97 yd in 2.20 s
combat: move r1_2 (2.23 s, hit 0.467-0.567), stamina 0
combat: r1_2 moved 1.42 yd in 2.23 s
combat: action ended
WOWSOULS_CHECK ok
```
The chain runs r1_1 → r1_2 → r1_3 and loops. Each press came 2.2 s into the swing, long past its f4, so the next move started at once and each travel is its clip's whole root motion: 1.98 / 1.42 / 1.98 yd, the spec's lunge column to ±2 %. The last line is a swing run to its own end (2.23 s). Each move costs 20 stamina and regeneration does not run during an action.

- [ ] **Step 3: R2, its follow-up, and the stamina gate**

Run: `live $FLAT WOWSOULS_CHECK=15 WOWSOULS_HEAVY=3.0,1.4`

Expected, the first six lines:
```
combat: move r2_1 (2.53 s, hit 0.733-0.867), stamina 55
combat: r2_1 moved 1.89 yd in 1.40 s
combat: move r2_2 (1.90 s, hit 0.333-0.467), stamina 20
combat: r2_2 moved 1.06 yd in 1.40 s
combat: move r2_1 (2.53 s, hit 0.733-0.867), stamina 0
combat: r2_1 moved 1.97 yd in 2.53 s
```
R2 → follow-up (`r2_2`) → R2 again, each chained at 1.40 s (past f4 = 1.30 s and 0.93 s); the first two are cut short by the chain, the third runs out. With the stamina gone the next press does nothing until some has regenerated (the run goes on with another `r2_1` and an `r2_2` that ends at its own 1.90 s with `moved 0.93 yd`).

Run: `live $FLAT WOWSOULS_CHECK=15 WOWSOULS_HEAVY=3.0,2.7`

Expected: three full `r2_1`, each `moved 1.98 yd in 2.5x s`.

- [ ] **Step 4: The rolling attack, and the cancels**

```bash
live $FLAT WOWSOULS_CHECK=11 WOWSOULS_DRIVE=0,1 WOWSOULS_ROLL=3.0 WOWSOULS_ATTACK=3.8
live $FLAT WOWSOULS_CHECK=11 WOWSOULS_ROLL=3.0 WOWSOULS_ATTACK=3.3
```
Expected, first run (the stick held forward: a roll, cut by the movement cancel):
```
combat: Roll toward Vec3(0.00, 0.00, -1.00), stamina 80
combat: Roll moved 4.14 yd in 1.10 s
combat: move roll_attack (2.10 s, hit 0.233-0.333), stamina 60
combat: roll_attack moved 1.10 yd in 0.88 s
combat: action ended
```
(the roll's cancel opens at 1.10 s; the stick cancels the rolling attack from its own f11, 0.867 s.) Second run (the stick still: a backstep, then the rolling attack to its end):
```
combat: Backstep toward Vec3(0.00, -0.00, 1.00), stamina 85
combat: Backstep moved 1.75 yd in 0.4x s
combat: move roll_attack (2.10 s, hit 0.233-0.333), stamina 65
combat: roll_attack moved 1.42 yd in 2.10 s
combat: action ended
```
(a light press is taken once the backstep's cancel time, 0.40 s, has passed, or at that time if it came sooner: after 0.40-0.55 s.)
```bash
live $FLAT WOWSOULS_CHECK=9 WOWSOULS_ATTACK=3.0 WOWSOULS_ROLL=3.95
live $FLAT WOWSOULS_CHECK=9 WOWSOULS_ATTACK=3.0 WOWSOULS_ROLL=3.7
live $FLAT WOWSOULS_CHECK=10 WOWSOULS_ATTACK=3.0,0.55
```
Expected: the first two end `r1_1 moved 1.37 yd in 0.95 s` and `... in 0.90 s`, then `combat: Backstep ...`: an evade is accepted from f26 (0.90 s); one pressed at 0.70 s waits in the buffer and starts at 0.90 s. The third shows R1 pressed every 0.55 s: `r1_1`, then `r1_2` after `r1_1 moved 1.38 yd in 0.78 s` (the press at 0.55 s waited for f4 = 0.77 s), then `r1_3` after `r1_2 moved 1.09 yd in 0.87 s` (a press 0.4 s before its f4 had expired in the buffer, the next one chained at once), and so on round the chain.

- [ ] **Step 5: Hits and damage on the kobold**

The kobold's strike lands 0.5 s into its wind-up; R1's hit window opens 0.53 s into the swing and R2's 0.73 s, so at 2 yd it often strikes first (`player: struck for 25`, the swing is staggered out). That is the Dark Souls timing, not a fault. Read only the `hit:` lines.

```bash
live WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8904,-136 WOWSOULS_CHECK=14 WOWSOULS_ATTACK=2.2,0.9 | grep -E "hit:|dies"
```
Expected: three `hit: <entity> for 20 at [...], health 40/60`, `20/60`, `0/60 (killed)`, then `npc: kobold dies` (the player may be struck once to three times meanwhile; if it dies first, run again).

```bash
live WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8904,-136 WOWSOULS_DODGE=0.3 WOWSOULS_HEAVY=3.7,1.4 WOWSOULS_CHECK=9 | grep -E "move|hit:"
```
Expected (the backstep dodge, `WOWSOULS_DODGE=0.3`, takes the kobold's first strike, so the slow R2 lands in its cooldown):
```
combat: move r2_1 (2.53 s, hit 0.733-0.867), stamina 40
hit: <entity> for 27 at [...], health 33/60
combat: move r2_2 (1.90 s, hit 0.333-0.467), stamina 5
hit: <entity> for 25 at [...], health 8/60
```
27 = 20 × 1.35 and 25 = 20 × 1.25: the `AtkParam_Pc` multipliers reach the damage.

```bash
live WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8904,-136 WOWSOULS_DODGE=0.3,0,1 WOWSOULS_ATTACK=3.5,0.1 WOWSOULS_CHECK=8 | grep -E "move|hit:|Roll"
```
Expected: `combat: Roll toward ...` (a roll through the kobold's strike, `WOWSOULS_DODGE=0.3,0,1`: stick forward), `Roll moved ~1.1 yd in 1.10 s` (the kobold's body stops it), then `combat: move roll_attack (2.10 s, hit 0.233-0.333), stamina 60` (the train of light presses, one every 0.1 s, always has one fresh in the buffer when the roll's cancel time, 1.10 s, comes). Then, when its early window finds the kobold, `hit: <entity> for 18 at [...], health 42/60` (18 = 20 × 0.9); a kobold that has shifted can be missed, in which case the next light move lands a `for 20` instead: run it up to three times. The 0.9 is also pinned by the unit test `a_moves_damage_is_the_base_hit_times_its_multiplier`.

- [ ] **Step 6: The fallback**

Run: `live $FLAT WOWSOULS_DSR_CACHE=C:/nonexistent WOWSOULS_CHECK=9 WOWSOULS_ATTACK=3.0,0.9 WOWSOULS_HEAVY=3.5,1.0`

Expected: one warning line `dsclip: no Dark Souls clips (no manifest in C:/nonexistent (...)); the procedural roll and swing stay` (the text in the brackets is the operating system's), then only the old swing: `combat: move r1_1 (0.80 s, hit 0.240-0.440), stamina 82`, `combat: r1_1 moved 0.72 yd in 0.80 s`, `combat: action ended`, repeating; no `r2_` line (the heavy press has nothing to start); `WOWSOULS_CHECK ok`.

---

### Task 11: The blade against Dark Souls' blade, and the look

The retarget carries the hand's rotation, not the sword in it. Task 8 turns the held WoW sword's blade onto the Dark Souls blade axis (the manifest's `grip` line); this task measures how well that holds on every frame of every move, and makes the pictures the user judges the look by.

**Files:** none (the numbers and the verdict go into `docs/HANDOVER.md` in Task 12).

- [ ] **Step 1: Measure every clip**

```bash
for m in r1_1 r1_2 r1_3 r2_1 r2_2 roll_attack 1; do
  echo "== $m"
  env WOWSOULS_CHECK=11 WOWSOULS_DSVIEW=$m DSR_DIR='C:/Spel/Dark Souls Remastered' WOW_DATA='C:/Spel/World of Warcraft 1.12/Data' \
    timeout 120 ./target/debug/wow-souls.exe 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep "blade error" | sed -E 's/^[0-9T:.Z-]+ +INFO [a-z_:]+: //'
done
```
Expected: per clip at least two lines `dsview <clip> blade error: worst <w> deg at t=<t> s, mean <m> deg over <n> frames`, one per complete loop with the body standing, near these numbers:

| clip | worst (deg) | mean (deg) |
|---|---|---|
| `a23_3000` (r1_1) | 9.7-12.2 | 3.2 |
| `a23_3001` (r1_2) | 5.4-5.5 | 3.1 |
| `a23_3002` (r1_3) | 4.5-4.8 | 3.1 |
| `a23_3300` (r2_1) | 6.2-6.9 | 3.1 |
| `a23_3310` (r2_2) | 5.2-5.5 | 3.0 |
| `a23_3900` (roll_attack) | 4.7-4.9 | 3.1 |
| `a00_0710` (the roll) | 3.5-3.6 | 3.0 |

Acceptance (spec criterion 4): every worst under 15 deg. The worst frame is always one between two 30 fps keyframes in the fastest part of a swing (r1_1 at t≈0.51 s): the retarget keyframes agree with Dark Souls', the in-between interpolation of the chain of local rotations does not quite. The 3 deg mean is a constant offset: it is 0 while the body hangs in its spawn pose, and appears when it lands (WoW's own pose of the bones the clips do not drive).

- [ ] **Step 2: What the fit buys (optional)**

Run:
```bash
env WOWSOULS_GRIP=0,0,0 WOWSOULS_CHECK=11 WOWSOULS_DSVIEW=r1_1 DSR_DIR='C:/Spel/Dark Souls Remastered' WOW_DATA='C:/Spel/World of Warcraft 1.12/Data' \
  timeout 120 ./target/debug/wow-souls.exe 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -E "blade error|the held blade" | sed -E 's/^[0-9T:.Z-]+ +INFO [a-z_:]+: //'
```
Expected: `dsclip: the held blade turns 0 deg onto the Dark Souls blade axis (WOWSOULS_GRIP)` and a mean near 25 deg, worst near 32: the sword as WoW authored it, unfitted. `WOWSOULS_GRIP=rx,ry,rz` takes degrees about the hand's axes; a turn about the blade itself (about the hand's Z, near enough) leaves the number unchanged and only turns the flat of the blade, which only the eye can judge.

- [ ] **Step 3: Frames for the eye**

`WOWSOULS_DSVIEW=<clip>` loops the clip from game time 0, so a clip's phase at `WOWSOULS_SHOT_AT` is that time modulo its length. These shot times show each move at its hit window (the gold bones and the pale line are Dark Souls' skeleton and blade; the magenta line is Dark Souls' blade axis laid from the base of the WoW sword: the drawn blade should lie along it):

| clip | shot at (s) |
|---|---|
| r1_1 | 25.60 |
| r1_2 | 25.08 |
| r1_3 | 25.50 |
| r2_1 | 26.13 |
| r2_2 | 25.10 |
| roll_attack | 25.48 |

```bash
for spec in r1_1:25.60 r1_2:25.08 r1_3:25.50 r2_1:26.13 r2_2:25.10 roll_attack:25.48; do
  clip=${spec%%:*}; at=${spec##*:}
  env WOWSOULS_SHOT="$EVID\\blade-$clip.png" WOWSOULS_SHOT_AT=$at WOWSOULS_OBSERVE=-1.5,1.4,-3.5 WOWSOULS_DSVIEW=$clip \
    DSR_DIR='C:/Spel/Dark Souls Remastered' WOW_DATA='C:/Spel/World of Warcraft 1.12/Data' timeout 150 ./target/debug/wow-souls.exe > /dev/null 2>&1
done
```
(Set `EVID='C:\Users\jacob\Documents\GitHub\wow darksouls\docs\evidence'` first: it is the evidence folder, as a Windows path. Each run takes about 35 s. The observer sits 1.5 yd west, 1.4 yd up and 3.5 yd north of the feet, looking at the human; the Dark Souls skeleton stands 1.5 yd east of the human.)

Read each picture with the Read tool. Expected: the human in the move's pose (R1-1 a downward slash with the blade low on the right, R2 a straight thrust), the sword in the right hand lying along the magenta line, the pale Dark Souls blade pointing the same way as the magenta one (the perspective makes two parallel lines differ on screen). A sword visibly across the line, or out of the hand, is a fault: find it with `WOWSOULS_GRIP` and the Step 1 numbers.

- [ ] **Step 4: Show the user**

Send the six pictures with the Step 1 table to the user and ask them to judge the look: the sword's grip, the swing against Dark Souls', the pops at the start and end of a move (the 0.1 s and 0.2 s blends). Their verdict, and any `WOWSOULS_GRIP` they settle on, go into `docs/HANDOVER.md` (Task 12). A different fixed grip would be a change to `dsclip::grip_rotation`, not to the manifest, and is its own small step.

---

### Task 12: Documents, install check, final gates

**Files:**
- Modify: `README.md`
- Modify: `docs/HANDOVER.md`
- Modify: `docs/superpowers/specs/2026-10-07-straight-sword-moveset-design.md`

- [ ] **Step 1: README: controls, switches, the Dark Souls data**

**Edit** `README.md`:

Old:
```
| Left mouse / F / pad RB | light attack (press again during the swing to chain) |
| Space / pad B | roll along the stick; with the stick still, backstep |
```
New:
```
| Left mouse / F / pad RB | light attack, R1 (press again once the move allows, to chain: three moves, then round again) |
| R / right mouse / pad RT | heavy attack, R2 (press again for its follow-up); the mouse buttons need the cursor captured |
| Space / pad B | roll along the stick; with the stick still, backstep; a light press just after it is the rolling attack |
```

**Edit** `README.md`:

Old:
```
| `WOWSOULS_ATTACK=<s>[,<every>]` | press attack at `s` seconds, then on a period |
```
New:
```
| `WOWSOULS_ATTACK=<s>[,<every>]` | press attack (R1) at `s` seconds, then on a period |
| `WOWSOULS_HEAVY=<s>[,<every>]` | press heavy (R2) at `s` seconds, then on a period (only with the Dark Souls moveset) |
```

**Edit** `README.md`:

Old:
```
| `WOWSOULS_DSVIEW=1` | side-by-side: the Dark Souls roll on its own skeleton (gold gizmo bones, 1.5 yd to the avatar's right) beside the human playing the retargeted clip, looping in place; logs `dsview t=` 10×/s |
| `WOWSOULS_DSR_CACHE=<dir>` | read the Dark Souls cache from `<dir>` (default `cache/dsr`) |
```
New:
```
| `WOWSOULS_DSVIEW=<clip>` | side-by-side: a cached Dark Souls clip (`1` is the roll, `a23_3000` or `r1_1` a sword move) on its own skeleton (gold gizmo bones and a pale blade, 1.5 yd to the avatar's right) beside the human playing the retargeted clip, looping in place; a magenta line lays Dark Souls' blade axis over the WoW sword; logs `dsview t=` 10×/s and, per loop, `blade error` (the WoW blade against Dark Souls') |
| `WOWSOULS_GRIP=rx,ry,rz` | rotate the held sword (degrees about the hand's axes) while Dark Souls clips pose the body, instead of the automatic fit onto Dark Souls' blade axis |
| `WOWSOULS_DSR_CACHE=<dir>` | read the Dark Souls cache from `<dir>` (default `cache/dsr`) |
```

**Edit** `README.md`:

Old:
```
With a cache present, the roll is Dark Souls Remastered's medium roll (`a00_0710`) retargeted
onto the WoW human: its length, distance, invulnerability window and cancel window come from the
DS data. Without one the game runs as before (procedural roll) and logs one warning. Build the
cache from your own DSR install:
```
New:
```
With a cache present, the roll is Dark Souls Remastered's medium roll (`a00_0710`) and the light
attack is its straight sword's (animation set `a23`): the one-handed R1 chain (`a23_3000`-`3002`),
R2 (`a23_3300`, then `a23_3310`) and the rolling attack (`a23_3900`), all retargeted onto the WoW
human. Lengths, hit windows, cancel windows, lunge distances (the clips' root motion), stamina
costs and damage multipliers come from the DS data (TAE events and `GameParam`). Without a cache
the game runs as before (procedural roll and swing) and logs one warning. Build the cache from
your own DSR install:
```

**Edit** `README.md`:

Old:
```
- Read (never written): `chr/c0000.anibnd.dcx` (the player skeleton and TAE events) and
  `chr/c0000_a*.anibnd.dcx` (the animations) from DSR; the HumanMale model from WoW's MPQs.
- Written: `cache/dsr/manifest.txt` (version, the roll, the retarget's measured errors, each
  source file's size and mtime: the game ignores a stale cache) and the roll as `.wscl` on the
  WoW skeleton (`human_male/`) and on the DS skeleton (`c0000/`, for `WOWSOULS_DSVIEW`).
```
New:
```
- Read (never written): `chr/c0000.anibnd.dcx` (the player skeleton and TAE events) and
  `chr/c0000_a*.anibnd.dcx` (the animations), `param/GameParam/GameParam.parambnd.dcx` and
  `paramdef/paramdef.paramdefbnd.dcx` (stamina and damage multipliers) from DSR; the HumanMale
  model from WoW's MPQs.
- Written: `cache/dsr/manifest.txt` (version, the roll, the weapon class with one `move` line per
  move and the blade `grip`, the retarget's measured errors, each source file's size and mtime:
  the game ignores a stale cache) and the roll and the six moves as `.wscl` on the WoW skeleton
  (`human_male/`) and on the DS skeleton (`c0000/`, for `WOWSOULS_DSVIEW`).
```

**Edit** `README.md`:

Old:
```
- Without the Dark Souls cache the roll is procedural: its crouch is StealthStand, not a tucked
```
New:
```
- With the cache, the light attack is Dark Souls' own and slower than the prototype's: R1's hit
  window opens 0.47-0.53 s in, the next move chains from 0.73-0.77 s, an evade from 0.9 s; a kobold
  at 2 yd strikes (0.5 s into its wind-up) before an R1 or R2 can land. The held WoW short sword is
  turned so its blade lies along Dark Souls' (mean 3 degrees off, worst 12); its twist about the
  blade is as WoW authored it (`WOWSOULS_GRIP` tunes it by eye).
- Without the Dark Souls cache the roll is procedural: its crouch is StealthStand, not a tucked
```

- [ ] **Step 2: HANDOVER: switches, architecture, status**

**Edit** `docs/HANDOVER.md`:

Old:
```
  `_SPIN=rad/s`, `_LOCK=s` (press lock), `_ATTACK=s[,every]`, `_ROLL=s[,every]` (evade, aimed by `_DRIVE`), `_DODGE=d[,x,y]` (evade d s after each enemy wind-up), `_KOBOLD_AT=x,y`, `_TRACE=1` (body/camera/gait logs), `_OBSERVE[=x,y,z]`
  (overhead debug view of the camera rig), `_AT`, `_KOBOLD_AT`, `_LOOK`, `_WIN`, `_BG`.
- Install untouched check: snapshot `find "/c/Spel/World of Warcraft 1.12" -type f -printf '%p %s %T@\n' | sort` before/after and diff.
```
New:
```
  `_SPIN=rad/s`, `_LOCK=s` (press lock), `_ATTACK=s[,every]` (R1), `_HEAVY=s[,every]` (R2), `_ROLL=s[,every]` (evade, aimed by `_DRIVE`), `_DODGE=d[,x,y]` (evade d s after each enemy wind-up), `_KOBOLD_AT=x,y`, `_TRACE=1` (body/camera/gait logs), `_OBSERVE[=x,y,z]`
  (overhead debug view of the camera rig), `_AT`, `_KOBOLD_AT`, `_LOOK`, `_WIN`, `_BG`, `_DSVIEW=<clip>`
  (side-by-side of a cached Dark Souls clip, `1` = roll, `a23_3000`/`r1_1` = a sword move, one
  `blade error` line per loop), `_GRIP=rx,ry,rz` (held-sword rotation while DS clips pose the body),
  `_DSR_CACHE=<dir>`.
- Flat ground for measuring a lunge: `WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8960,-136` (the
  default anchor has a 0.8 yd ridge 3 yd north of it, which stops a lunge short).
- Install untouched check: snapshot `find "/c/Spel/World of Warcraft 1.12" -type f -not -name realmlist.wtf -printf '%p %s %T@\n' | sort` and the same for `"/c/Spel/Dark Souls Remastered"` before/after and diff.
```

**Edit** `docs/HANDOVER.md`:

Old:
```
| `combat.rs` | `Health`, `Stamina`, `Action {state: Idle / Attack(Swing) / Evade(Evade), buffer: Buffered {press: Attack/Evade}}`; pure `step_action(a, stamina, StepInput, dt)`; evades `ROLL`/`BACKSTEP` (`EvadeSpec`: duration, travel, distance, iframes, cancel_from, cost), `Action::invulnerable()` for step 5; `tumble` writes the somersault onto `ModelFrame` (`tumble_transform`); `act` (PlaySet::Intent after `read_intent`) plays Attack1H via `play_once`, overrides `MoveIntent` with the lunge; `anim::drive` yields while `Action::owns_body()` |
```
New:
```
| `combat/mod.rs` | `Health`, `Stamina`, the plugin and systems: `act` (PlaySet::Intent after `read_intent`) feeds `step_action` the presses (F / LMB / RB = `Press::Attack`, R / RMB / RT = `Press::Heavy`, Space / B = `Press::Evade`; scripted `_ATTACK` / `_HEAVY` / `_ROLL`), plays the procedural swing's WoW clip via `play_once` (a Dark Souls move poses the rig in `dsclip`), overrides `MoveIntent` with the lunge (`root_step`: a Dark Souls move's root motion frame by frame, recoil included; the procedural swing's `LUNGE_SPEED` rule); `tumble` writes the somersault onto `ModelFrame` (`tumble_transform`); `anim::drive` yields while `Action::owns_body()` |
| `combat/action.rs` | the pure action machine: `Action {state: Idle / Attack(Swing) / Evade(Evade) / Stagger / Dead, buffer}`, `SwingSpec {duration, hit, chain_from, evade_from, move_from, cost, mult}` (f4 chain, f26 evade, f11 movement cancel), `Moveset {r1[3], r2, roll_attack, ds}`, `MoveKind {R1(n), R2(n), RollAttack}`, `Press`, `step_action(a, stamina, StepInput, dt) -> Decision`, `next_move`, `start`; evades `ROLL`/`BACKSTEP` (`EvadeSpec`), `Action::invulnerable()`; the procedural swing is `SwingSpec::procedural(len)`, pinned to the old phase fractions by a test |
```

**Edit** `docs/HANDOVER.md`:

Old:
```
| `dsclip.rs`, `dsview.rs` | DS roll from `cache/dsr/` (spec, root motion, `PosePost` pose, blend); `WOWSOULS_DSVIEW` side-by-side |
```
New:
```
| `dsclip.rs`, `dsview.rs` | the DS roll and the six sword moves from `cache/dsr/` (manifest: version, `move` lines, `grip`; clips give `EvadeSpec` / `SwingSpec`, root motion `progress`, the `PosePost` pose with blends and a chained clip fading over the last one's end pose; the held sword's blade turned onto DS's blade axis while clips pose the body, `grip_rotation`); `WOWSOULS_DSVIEW=<clip>` side-by-side with the per-loop `blade error` line |
```

**Edit** `docs/HANDOVER.md`:

Old:
```
| `hits.rs` | `Hurtbox` (vertical capsule), `Hit` message, `BladeTrail`; `strike` sweeps [tip marker, hand] prev->now in `SwingPhase::Active` (pure `segment_distance`, `blade_hits`); `BLADE_RADIUS` 0.18;
```
New:
```
| `hits.rs` | `Hurtbox` (vertical capsule), `Hit` message, `BladeTrail`; `strike` sweeps [tip marker, hand] prev->now in `SwingPhase::Active` (pure `segment_distance`, `blade_hits`); damage `hit_damage` = `LIGHT_DAMAGE` 20 x the move's `mult`; a swing's `serial` lets a chained or looped move strike again; `BLADE_RADIUS` 0.18;
```

**Edit** `docs/HANDOVER.md`:

Old:
```
The user judged the roll look good (2026-10-07). Plan C (straight-sword moveset) is brainstormed;
spec `docs/superpowers/specs/2026-10-07-straight-sword-moveset-design.md` is written and waits for
the user's review, then `superpowers:writing-plans`. Scope: R1 chain, R2, rolling attack. Findings:
```
New:
```
The user judged the roll look good (2026-10-07). **Plan C (straight-sword moveset) is built**
(plan `docs/superpowers/plans/2026-10-07-straight-sword-moveset.md`, spec
`docs/superpowers/specs/2026-10-07-straight-sword-moveset-design.md`; nothing committed yet). The
light attack is Dark Souls' own, from the install's data: the one-handed R1 chain (`a23_3000`,
`a23_3001`, `a23_3002`, round again after three), R2 (`a23_3300`, then `a23_3310`) and the rolling
attack (`a23_3900`: a light press in a roll or backstep once its cancel time has passed),
retargeted onto the human, timed by their TAE windows, priced by `GameParam`. Without a cache the
old procedural swing and roll stay. Keys: F / LMB / pad RB light, R / RMB / pad RT heavy.
- Extractor: `dsr-formats/param.rs` (PARAMDEF v104 + PARAM), the TAE attack judge, `wscl` v2 (hit
  windows), `dsr-cache` `params.rs` + `moveset.rs` (the hand-made class table `straight_sword`,
  variation 2300) and `cache.rs` (manifest: `class`, six `move` lines, `grip`).
  `EXTRACTOR_VERSION` 3, `wscl-cache` 2. Rebuild: `cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- extract`.
- Game: see the `combat/` and `dsclip.rs` rows above. Flags read as: f4 attack input (next move),
  f26 evade, f11 movement (inferred from the data pattern, not from the player's ESD).
- Numbers, live (table = the spec's; lunge = net travel on flat ground, the chain pressed at 2.2 s
  so each move runs its whole root motion):

  | move | clip | length s | hit window s | f4 / f26 / f11 s | lunge yd, table / live | stamina | damage, live |
  |---|---|---|---|---|---|---|---|
  | R1-1 | a23_3000 | 2.500 | 0.533-0.667 | 0.767 / 0.900 / 1.033 | 1.98 / 1.97 | 20 | 20 |
  | R1-2 | a23_3001 | 2.233 | 0.467-0.567 | 0.733 / 0.900 / 1.000 | 1.42 / 1.42 | 20 | 20 |
  | R1-3 | a23_3002 | 2.500 | 0.467-0.600 | 0.767 / 0.900 / 1.033 | 1.98 / 1.98 | 20 | 20 |
  | R2 | a23_3300 | 2.533 | 0.733-0.867 | 1.300 / 1.300 / 1.367 | 1.98 / 1.98 | 45 | 27 |
  | R2 second | a23_3310 | 1.900 | 0.333-0.467 | 0.933 / 0.933 / 1.200 | 0.94 / 0.93 | 35 | 25 |
  | rolling attack | a23_3900 | 2.100 | 0.233-0.333 | 0.700 / 0.767 / 0.867 | 1.42 / 1.42 | 20 | 18 (unit test; the live hit is timing-sensitive) |

  Chain rules seen live: an R1 pressed before f4 waits in the 0.35 s buffer and chains at f4 (0.78 s
  after the swing began); the chain loops after three; R2 -> R2 second -> R2; an evade from f26
  (pressed at 0.70 s it starts at 0.90 s); a held stick ends a swing at f11 (`roll_attack moved
  1.10 yd in 0.88 s`); a light press in a roll after 1.10 s or in a backstep after 0.40 s is the
  rolling attack; a stagger ends the swing; no stamina, no swing. The root motion's recoil
  (R1-2 steps back 0.16 yd after its thrust) is applied: `MoveIntent.speed` may be negative.
- The look, measured: the drawn WoW sword's blade against Dark Souls' blade, both in the body's
  frame, every frame of every clip (`WOWSOULS_DSVIEW=<clip>`, `blade error` per loop): mean
  3.0-3.2 deg, worst 4.5-12.2 deg (limit 15) on all six moves and the roll; unfitted
  (`WOWSOULS_GRIP=0,0,0`) mean 25, worst 32. The 3 deg mean is WoW's own pose of the bones the
  clips do not drive (0 while the body hangs in its spawn pose). The worst frame sits between two
  keyframes in the fastest part of R1-1 (interpolating local rotations is not model-space
  interpolation). Pictures: `blade-<clip>.png` in the evidence folder.
- Gates at the end: fmt, `cargo clippy -j 4 --workspace --all-targets -- -D warnings` clean,
  `cargo test -j 4 --workspace`: wow-souls 181, dsr-formats 29, dsr-cache 17, wscl 4 (with
  `DSR_DIR`/`WOW_DATA` set the install tests run too). Installs: before/after listings identical
  (the WoW listing leaves out `realmlist.wtf`, see above).
- Open: the user's eye on the look (grip, swing, the blends at a move's start and end; the flat
  of the blade is only judged by eye) and on the feel (Dark Souls' timing is slower than the
  prototype's: R1 hits 0.5 s in, chains from 0.77 s, evades from 0.9 s; a kobold at 2 yd strikes
  first, a dodge such as `WOWSOULS_DODGE=0.3` lets an R2 land); the pad with RT = R2 (live,
  awake pad, focused window); parked: running attack (needs a sprint), two-handing and the 4000
  series, jump and plunge, backstab and riposte, left hand, guard and parry, DS locomotion, other
  weapon classes (data-only additions to `moveset.rs`), DS impact (D), poise.
Findings:
```

**Edit** `docs/HANDOVER.md`:

Old:
```
**Latest (2026-10-07, evening):** plan C's implementation plan is written and was replayed task by
task in a throw-away copy of this repo (`docs/superpowers/plans/2026-10-07-straight-sword-moveset.md`,
12 tasks, every gate green there); it has not been executed in this repo yet. Start at its Task 1.
```
New:
```
**Latest (2026-10-07):** plan C (the straight-sword moveset) is built; see the status below.
```

**Edit** `docs/HANDOVER.md`:

Old:
```
  warnings`, 157 wow-souls tests. R2 (R / RMB / RT) arrives with plan C's R2 move.
```
New:
```
  warnings`, 157 wow-souls tests. R2 (R / RMB / RT) arrived with plan C's R2 move.
```

- [ ] **Step 3: Spec: say what was built**

**Edit** `docs/superpowers/specs/2026-10-07-straight-sword-moveset-design.md`:

Old:
```
- **Lunge.** Each frame's share of the clip's root motion goes into the motor as the roll's does, along
  the body's facing. The motor stops at bodies. The old stop-gap rule (`enemy_in_reach`) stays for
  the procedural swing only.
```
New:
```
- **Lunge.** Each frame's share of the clip's root motion goes into the motor as the roll's does, along
  the body's facing, recoil included (a step back is a negative speed along the facing, so the body
  does not turn: R1-2 steps back 0.16 yd after its thrust and ends 1.42 yd on, as the clip does). The
  motor stops at bodies. The old stop-gap rule (`enemy_in_reach`) stays for the procedural swing only.
```

**Edit** `docs/superpowers/specs/2026-10-07-straight-sword-moveset-design.md`:

Old:
```
- **Instruments.** `WOWSOULS_HEAVY=<s>[,every]` presses R2 on a schedule. `WOWSOULS_DSVIEW=<clip>`
  plays any cached clip (the value `1` keeps meaning the roll). `WOWSOULS_TRACE` logs
  `combat: move <name> ...` at each start and the DSVIEW line adds the blade angle.
```
New:
```
- **Instruments.** `WOWSOULS_HEAVY=<s>[,every]` presses R2 on a schedule. `WOWSOULS_DSVIEW=<clip>`
  plays any cached clip (the value `1` keeps meaning the roll) and logs, once per complete loop with
  the body standing, `blade error: worst <w> deg ..., mean <m> deg` (the drawn WoW blade against the
  DS blade, both in the body's frame); it also draws the DS blade (pale) and its axis laid over the WoW
  sword (magenta). `WOWSOULS_TRACE` logs `combat: move <name> (<length>, hit <window>), stamina <n>`
  at each start and `combat: <name> moved <yd> yd in <s> s` at each end.
```

- [ ] **Step 4: The installs are untouched**

```bash
find "/c/Spel/Dark Souls Remastered" -type f -printf '%p %s %T@\n' | sort > "$SCRATCH/installs/dsr-after.txt"
find "/c/Spel/World of Warcraft 1.12" -type f -not -name realmlist.wtf -printf '%p %s %T@\n' | sort > "$SCRATCH/installs/wow-after.txt"
diff "$SCRATCH/installs/dsr-before.txt" "$SCRATCH/installs/dsr-after.txt" && diff "$SCRATCH/installs/wow-before.txt" "$SCRATCH/installs/wow-after.txt" && echo untouched
```
Expected: `untouched` (the `-before` files were written before Task 1; the Dark Souls listing takes about 45 s).

- [ ] **Step 5: Final gates**

Run: `cargo fmt --all -- --check && cargo clippy -j 4 --workspace --all-targets -- -D warnings && cargo test -j 4 --workspace`
Expected: all green: wow-souls 181, dsr-formats 29, dsr-cache 17, wscl 4.
Run: `DSR_DIR='C:/Spel/Dark Souls Remastered' WOW_DATA='C:/Spel/World of Warcraft 1.12/Data' cargo test -j 4 --workspace`
Expected: the same, with the install tests really running (they print nothing and pass).

- [ ] **Step 6: Memory and hand-over**

Update the memory note `wow-souls-combat-plan.md` (plan C built, the numbers above in one line, the open items) and `docs/HANDOVER.md` is already current from Step 2. Do not commit: the user commits when they ask.

---

## Appendix A: `apply_plan.py`

Applies the **Create** and **Edit** blocks of this plan to the repo, asserting that every Edit's Old text occurs exactly once (it also copes with files that use CRLF line endings, as parts of `dsr-formats/src/tae.rs` do). Save it as `$SCRATCH/apply_plan.py` and use it to apply a task, or the whole plan, in one go:

```bash
python "$SCRATCH/apply_plan.py" /c/Spel/wow-souls "$SCRATCH" docs/superpowers/plans/2026-10-07-straight-sword-moveset.md --tasks 1-2
```

The manual steps between tasks stay manual: after Task 5 run the extractor (Task 5 Step 3), after Task 6's script is written run it (Task 6 Step 2), and run `cargo fmt --all` and each task's tests before moving on. `--only <path>` applies just the blocks for one file.

```python
"""Applies the Create and Edit directives of plan files to a workspace.

    python apply_plan.py <workspace> <scratch dir> <plan.md> [<plan.md> ...] [--tasks 1-5] [--only path]

Directives (in the plan's Markdown):

    **Create** `path`:
    ```lang
    <whole file>
    ```

    **Edit** `path`:

    Old:
    ```lang
    <text that occurs exactly once in the file>
    ```
    New:
    ```lang
    <its replacement>
    ```

A path starting with `scratch:` goes to the scratch directory instead of the workspace.
Every edit asserts that its old text occurs exactly once.
"""
import os
import re
import sys

args = sys.argv[1:]
tasks = None
if "--tasks" in args:
    i = args.index("--tasks")
    lo, _, hi = args[i + 1].partition("-")
    tasks = (int(lo), int(hi or lo))
    del args[i : i + 2]
only = None
if "--only" in args:
    i = args.index("--only")
    only = args[i + 1]
    del args[i : i + 2]
workspace, scratch, plans = args[0], args[1], args[2:]

CREATE = re.compile(r"\*\*Create\*\* `([^`]+)`:\n```[^\n]*\n(.*?)\n```\n", re.S)
EDIT = re.compile(
    r"\*\*Edit\*\* `([^`]+)`:\n\nOld:\n```[^\n]*\n(.*?)\n```\nNew:\n```[^\n]*\n(.*?)\n```\n", re.S
)
TASK = re.compile(r"^### Task (\d+):", re.M)

applied = 0
for plan in plans:
    text = open(plan, encoding="utf-8").read()
    marks = [(m.start(), int(m.group(1))) for m in TASK.finditer(text)]

    def task_at(pos):
        n = 0
        for start, num in marks:
            if start <= pos:
                n = num
        return n

    items = []
    for m in CREATE.finditer(text):
        items.append((m.start(), "create", m.group(1), m.group(2), None))
    for m in EDIT.finditer(text):
        items.append((m.start(), "edit", m.group(1), m.group(2), m.group(3)))
    items.sort()
    for pos, kind, path, a, b in items:
        n = task_at(pos)
        if tasks and not (tasks[0] <= n <= tasks[1]):
            continue
        if only and path != only:
            continue
        full = (
            os.path.join(scratch, path[len("scratch:") :])
            if path.startswith("scratch:")
            else os.path.join(workspace, path)
        )
        if kind == "create":
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, "w", encoding="utf-8", newline="\n") as f:
                f.write(a + "\n")
            print(f"task {n}: created {path}")
        else:
            src = open(full, encoding="utf-8", newline="").read()
            # The plan's text is LF. A file may use CRLF (or a mix): try the text as given, then
            # with CRLF line endings, and keep the endings of whichever matches.
            crlf = False
            if src.count(a) != 1:
                a2, b2 = a.replace("\n", "\r\n"), b.replace("\n", "\r\n")
                if src.count(a2) == 1:
                    a, b, crlf = a2, b2, True
            count = src.count(a)
            if count != 1:
                print(f"task {n}: EDIT FAILED in {path}: old text occurs {count} times:\n{a[:300]}")
                sys.exit(1)
            with open(full, "w", encoding="utf-8", newline="") as f:
                f.write(src.replace(a, b))
            print(f"task {n}: edited {path}" + (" (CRLF)" if crlf else ""))
        applied += 1
print(f"{applied} directives applied")
```

## Appendix B: spec coverage

| Spec requirement | Where |
|---|---|
| Criterion 1: clip length, hit window, f4 / f26 / f11 and lunge equal the data | extractor install test (Task 5), loader tests (Task 8), live traces (Task 10 Steps 2-4) |
| Criterion 2: stamina and damage from the params; the extractor fails naming the row | `param.rs` and `params.rs` (Tasks 2, 4), manifest (Task 5), `hit_damage` test (Task 7), live 20 / 27 / 25 (Task 10 Step 5) |
| Criterion 3: chain rules under scripted presses | machine tests (Task 7), live chain, heavy, rolling attack, cancels, buffer (Task 10 Steps 2-4) |
| Criterion 4: the look side by side, blade within 15 deg | retarget gates (Task 5), `blade error` per loop and the pictures (Task 11) |
| Criterion 5: no cache, as today, one warning | `dsclip::startup` (Task 8), live fallback (Task 10 Step 6), the procedural-spec test (Task 7) |
| Criterion 6: the pad drives R2 as well | `heavy_pressed` and its test (Task 9); the user's live pad run stays open (HANDOVER) |
| Extractor: PARAMDEF / PARAM reader, class record check, `cache.rs` | Tasks 2, 4, 5 |
| `wscl` v2 with hit windows; a v1 file refused | Task 3 |
| `combat.rs` split, then the moveset work in `action.rs` | Tasks 6, 7 |
| Heavy press, R2 chain, rolling attack, buffer, cancels, stamina at the move's start | Tasks 7, 9 |
| Playing: 1.0x, `PosePost`, blends, a chained move over the last one's end pose | Task 8 |
| Lunge from root motion through the motor | Task 9 (`root_step`) |
| Inputs R / RMB / RT; `WOWSOULS_HEAVY`, `WOWSOULS_DSVIEW=<clip>`, `WOWSOULS_GRIP` | Tasks 8, 9 |
| Weapon grip: the blade axis in the hand frame, the fit, the correction only while clips pose the rig | Tasks 4, 5, 8; measured in Task 11 |
| Locality rule: nothing from the install is committed | `/cache` stays gitignored; Task 12 Step 4 proves the installs untouched |
