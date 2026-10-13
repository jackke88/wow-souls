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
    Ok(Section {
        tag,
        start,
        end,
        children,
    })
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
            other => bail!(
                "unexpected TYPE subsection {}",
                String::from_utf8_lossy(other)
            ),
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

/// Every type index the file gives (parents, subtypes, field and item types) is in the table,
/// so the lookups after parsing cannot go out of range.
fn validate(types: &[Type], items: &[Item]) -> Result<()> {
    let n = types.len();
    let ok = |i: Option<usize>| i.is_none_or(|i| i < n);
    for (i, t) in types.iter().enumerate() {
        if !ok(t.parent) || !ok(t.sub_type) || !t.fields.iter().all(|f| ok(f.ty)) {
            bail!(
                "type {i} ({}) refers to a type past the table ({n})",
                t.name
            );
        }
    }
    for (i, it) in items.iter().enumerate() {
        if !ok(it.ty) {
            bail!("item {i} refers to a type past the table ({n})");
        }
    }
    Ok(())
}

impl Tagfile {
    pub fn parse(bytes: Vec<u8>) -> Result<Tagfile> {
        let root = read_section(&bytes, 0)?;
        if &root.tag != b"TAG0" {
            bail!(
                "not a Havok tagfile (root {})",
                String::from_utf8_lossy(&root.tag)
            );
        }
        let sdk = root.child(b"SDKV").context("tagfile has no SDKV")?;
        let version = slice(&bytes, sdk.start, 8)?;
        if version != b"20150100" {
            bail!(
                "Havok SDK {} is not 2015.1",
                String::from_utf8_lossy(version)
            );
        }
        let data = root.child(b"DATA").context("tagfile has no DATA")?.start;
        let types = read_types(&bytes, root.child(b"TYPE").context("tagfile has no TYPE")?)?;
        let items = read_items(
            &bytes,
            root.child(b"INDX").context("tagfile has no INDX")?,
            data,
        )?;
        validate(&types, &items)?;
        Ok(Tagfile {
            bytes,
            types,
            items,
        })
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
        chain
            .iter()
            .rev()
            .flat_map(|&t| self.types[t].fields.iter())
            .collect()
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
        let end = it
            .count
            .checked_mul(stride)
            .and_then(|n| n.checked_add(it.at));
        if end.is_none_or(|e| e > self.bytes.len()) {
            bail!(
                "item {idx} ({} x {stride} bytes at {:#x}) runs past the end",
                it.count,
                it.at
            );
        }
        Ok((0..it.count)
            .map(|i| Obj {
                tf: self,
                ty,
                at: it.at + i * stride,
            })
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
            let sub = self
                .tf
                .sub_type(self.ty)
                .context("fixed array without element type")?;
            let size = self.tf.byte_size(sub);
            return Ok((0..(fi >> 8) as usize)
                .map(|i| Obj {
                    tf: self.tf,
                    ty: sub,
                    at: self.at + i * size,
                })
                .collect());
        }
        self.tf
            .item_elements(u32_at(&self.tf.bytes, self.at)? as usize)
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
        if self.kind() != Kind::Array || self.tf.format_info(self.ty) & 0x20 != 0 {
            bail!("{} is not a variable-size array", self.type_name());
        }
        let idx = u32_at(&self.tf.bytes, self.at)? as usize;
        if idx == 0 {
            return Ok(&[]);
        }
        let it = self
            .tf
            .items
            .get(idx)
            .with_context(|| format!("item index {idx} past the item table"))?;
        let ty = it.ty.context("item has no type")?;
        if self.tf.byte_size(ty) != 1 {
            bail!("{} is not a byte array", self.type_name());
        }
        slice(&self.tf.bytes, it.at, it.count)
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
    fn a_type_index_past_the_table_is_an_error() {
        let types = vec![Type::default(); 2];
        let item = |ty| Item {
            ty,
            at: 0,
            count: 1,
        };
        assert!(validate(&types, &[item(None), item(Some(1))]).is_ok());
        assert!(validate(&types, &[item(None), item(Some(2))]).is_err());
        let mut bad = types.clone();
        bad[0].parent = Some(5);
        assert!(validate(&bad, &[]).is_err());
    }

    #[test]
    fn a_section_past_the_end_is_an_error() {
        let mut b = Vec::new();
        b.extend_from_slice(&64u32.to_be_bytes());
        b.extend_from_slice(b"TAG0");
        assert!(read_section(&b, 0).is_err());
    }

    /// `DSR_DIR=<install>`: the player skeleton file parses, and its root is the container.
    #[test]
    fn install_player_skeleton_parses() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let bnd = std::fs::read(format!("{dir}/chr/c0000.anibnd.dcx")).unwrap();
        let entries = crate::bnd3::read(&crate::dcx::decompress(&bnd).unwrap()).unwrap();
        let skel = entries
            .iter()
            .find(|e| e.file_name() == "Skeleton.hkx")
            .unwrap();
        let tf = Tagfile::parse(skel.data.clone()).unwrap();
        eprintln!(
            "Skeleton.hkx: {} types, {} items",
            tf.types.len(),
            tf.items.len()
        );
        assert_eq!(tf.root().unwrap().type_name(), "hkRootLevelContainer");
        let c = crate::hkx::animation_container(&tf).unwrap();
        assert_eq!(c.field("skeletons").unwrap().array().unwrap().len(), 1);
    }
}
