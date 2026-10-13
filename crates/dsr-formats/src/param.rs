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
