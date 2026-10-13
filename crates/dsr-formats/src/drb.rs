//! DRB UI layouts (SoulsFormatsNEXT `DRB.cs`; Dark Souls' `menu.drb`): dialogs made of named
//! objects, each a shape (sprite, rectangle, text, a sub-dialog) with a rectangle in the dialog's
//! space. The in-game HUD is a handful of these dialogs in a 1280 x 720 space.
//!
//! The file is `"DRB\0"` and three zero words, then sections `tag, i32 length, i32 count, i32 0`
//! and `length` bytes of body: `STR\0` (UTF-16 strings, referenced by byte offset), `TEXI`
//! (count x 16: name offset, path offset, 2 words), `SHPR` (the shape records), `CTPR`, `ANIP`,
//! `INTP`, `SCDP`, `SHAP` (count x 8: type string offset, `SHPR` offset), `CTRL` (count x 8),
//! `ANIK`, `ANIO`, `ANIM` (its header length covers only a table; the animations run on to the
//! next section), `SCDL`, `DLGO` (count x 32: name, `SHAP` byte offset, `CTRL` byte offset, 5
//! words), `DLG\0` (count x 64) and `END\0`. `SHAP` and `CTRL` hold one entry per dialog and per
//! dialog object in file order: a dialog, then its objects, the next dialog, and so on.
//!
//! A `DLG` record is: name offset (+0), `SHAP` byte offset (+4), `CTRL` byte offset (+8), object
//! count (+32), the first object's `DLGO` byte offset (+36). Every shape record starts with
//! `i16 left, top, right, bottom` in the parent's space. Colours are four bytes `A R G B` in file
//! order.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use crate::bytes::{i16_at, i32_at, slice, u8_at};

/// A rectangle in pixels of its dialog's space (or of an atlas, for a sprite's texture rectangle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
}

impl Rect {
    pub fn width(&self) -> i32 {
        i32::from(self.right) - i32::from(self.left)
    }

    pub fn height(&self) -> i32 {
        i32::from(self.bottom) - i32::from(self.top)
    }
}

/// What an object draws.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// Nothing: a pure rectangle other objects are placed by.
    Null,
    /// A sub-dialog; its name is the object's name.
    Dialog,
    /// A texture rectangle stretched over the object's rectangle.
    Sprite {
        /// The source rectangle in the texture's pixels.
        uv: Rect,
        /// Index into [`Drb::textures`]; -1 when the game assigns one at run time (item icons).
        texture: i16,
        /// 1 alpha blend, 2 additive.
        blend: u8,
        /// `0xAARRGGBB`.
        tint: u32,
    },
    /// A flat rectangle, `0xAARRGGBB`.
    MonoRect { color: u32 },
    /// A rectangle with a colour per corner: top-left, top-right, bottom-left, bottom-right.
    GouraudRect { colors: [u32; 4] },
    /// A text field: the colour (`0xAARRGGBB`) and the font size in layout pixels.
    Text { color: u32, size: i32 },
    /// A type this reader does not decode (`ScrollText`, `Mask`, `MonoFrame`, `GouraudSprite`, ...).
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub name: String,
    /// In the dialog's space.
    pub rect: Rect,
    pub shape: Shape,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dialog {
    pub name: String,
    pub rect: Rect,
    pub objects: Vec<Object>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Drb {
    /// The texture names, indexed by a sprite's `texture`.
    pub textures: Vec<String>,
    pub dialogs: Vec<Dialog>,
}

impl Drb {
    /// The first dialog of that name.
    pub fn dialog(&self, name: &str) -> Option<&Dialog> {
        self.dialogs.iter().find(|d| d.name == name)
    }
}

#[derive(Debug, Clone, Copy)]
struct Section {
    body: usize,
    len: usize,
    count: usize,
}

const TAGS: [&[u8; 4]; 16] = [
    b"STR\0", b"TEXI", b"SHPR", b"CTPR", b"ANIP", b"INTP", b"SCDP", b"SHAP", b"CTRL", b"ANIK",
    b"ANIO", b"ANIM", b"SCDL", b"DLGO", b"DLG\0", b"END\0",
];

/// Where the section after `ANIM` starts: its header length covers a table only, so look for the
/// `SCDL` header whose body is followed by `DLGO`.
fn after_anim(b: &[u8], from: usize) -> Result<usize> {
    let mut at = from;
    while at + 20 <= b.len() {
        if &b[at..at + 4] == b"SCDL" {
            let len = i32_at(b, at + 4)?;
            let zero = i32_at(b, at + 12)?;
            if len >= 0 && zero == 0 {
                let next = at + 16 + len as usize;
                if b.get(next..next + 4) == Some(b"DLGO") {
                    return Ok(at);
                }
            }
        }
        at += 1;
    }
    bail!("no SCDL section after ANIM")
}

fn sections(b: &[u8]) -> Result<HashMap<[u8; 4], Section>> {
    if slice(b, 0, 4)? != b"DRB\0" {
        bail!("not a DRB file");
    }
    let mut out = HashMap::new();
    let mut at = 16;
    loop {
        let tag: [u8; 4] = slice(b, at, 4)?.try_into().expect("four bytes");
        if !TAGS.iter().any(|t| **t == tag) {
            bail!(
                "unknown DRB section {:?} at {at:#x}",
                String::from_utf8_lossy(&tag)
            );
        }
        let len = i32_at(b, at + 4)?;
        let count = i32_at(b, at + 8)?;
        if len < 0 || count < 0 {
            bail!("DRB section at {at:#x} has length {len}, count {count}");
        }
        let (len, count) = (len as usize, count as usize);
        let body = at + 16;
        out.insert(tag, Section { body, len, count });
        if &tag == b"END\0" {
            return Ok(out);
        }
        at = if &tag == b"ANIM" {
            after_anim(b, body + len)?
        } else {
            body + len
        };
    }
}

struct Reader<'a> {
    b: &'a [u8],
    secs: HashMap<[u8; 4], Section>,
}

impl Reader<'_> {
    fn sec(&self, tag: &[u8; 4]) -> Result<Section> {
        self.secs
            .get(tag)
            .copied()
            .with_context(|| format!("no {} section", String::from_utf8_lossy(tag)))
    }

    /// The UTF-16 string at a byte offset of the string table.
    fn string(&self, offset: i32) -> Result<String> {
        let s = self.sec(b"STR\0")?;
        let offset = usize::try_from(offset).context("negative string offset")?;
        if offset >= s.len {
            bail!("string offset {offset:#x} is outside the string table");
        }
        let blob = slice(self.b, s.body, s.len)?;
        let units: Vec<u16> = blob[offset..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .take_while(|&u| u != 0)
            .collect();
        Ok(String::from_utf16_lossy(&units))
    }

    fn rect(&self, at: usize) -> Result<Rect> {
        Ok(Rect {
            left: i16_at(self.b, at)?,
            top: i16_at(self.b, at + 2)?,
            right: i16_at(self.b, at + 4)?,
            bottom: i16_at(self.b, at + 6)?,
        })
    }

    fn argb(&self, at: usize) -> Result<u32> {
        let c = slice(self.b, at, 4)?;
        Ok(u32::from_be_bytes([c[0], c[1], c[2], c[3]]))
    }

    /// The shape of `SHAP` entry `index`: its rectangle and what it draws.
    fn shape(&self, index: usize) -> Result<(Rect, Shape)> {
        let shap = self.sec(b"SHAP")?;
        if index >= shap.count {
            bail!("shape {index} of {}", shap.count);
        }
        let entry = shap.body + index * 8;
        let ty = self.string(i32_at(self.b, entry)?)?;
        let at = self.sec(b"SHPR")?.body + usize::try_from(i32_at(self.b, entry + 4)?)?;
        let rect = self.rect(at)?;
        let shape = match ty.as_str() {
            "Null" => Shape::Null,
            "Dialog" => Shape::Dialog,
            "Sprite" => Shape::Sprite {
                uv: self.rect(at + 16)?,
                texture: i16_at(self.b, at + 24)?,
                blend: u8_at(self.b, at + 27)?,
                tint: self.argb(at + 32)?,
            },
            "MonoRect" => Shape::MonoRect {
                color: self.argb(at + 24)?,
            },
            "GouraudRect" => Shape::GouraudRect {
                colors: [
                    self.argb(at + 20)?,
                    self.argb(at + 24)?,
                    self.argb(at + 28)?,
                    self.argb(at + 32)?,
                ],
            },
            "Text" => Shape::Text {
                color: self.argb(at + 24)?,
                size: i32_at(self.b, at + 32)?,
            },
            _ => Shape::Other(ty),
        };
        Ok((rect, shape))
    }
}

pub fn read(b: &[u8]) -> Result<Drb> {
    let r = Reader {
        b,
        secs: sections(b)?,
    };
    let texi = r.sec(b"TEXI")?;
    let mut textures = Vec::with_capacity(texi.count);
    for i in 0..texi.count {
        textures.push(r.string(i32_at(b, texi.body + i * 16)?)?);
    }
    let dlg = r.sec(b"DLG\0")?;
    let dlgo = r.sec(b"DLGO")?;
    let mut dialogs = Vec::with_capacity(dlg.count);
    for k in 0..dlg.count {
        let at = dlg.body + k * 64;
        let name = r.string(i32_at(b, at)?)?;
        let one = || -> Result<Dialog> {
            let (rect, _) = r.shape(usize::try_from(i32_at(b, at + 4)?)? / 8)?;
            let count = usize::try_from(i32_at(b, at + 32)?)?;
            let first = usize::try_from(i32_at(b, at + 36)?)?;
            let mut objects = Vec::with_capacity(count);
            for i in 0..count {
                let o = dlgo.body + first + i * 32;
                if o + 32 > dlgo.body + dlgo.len {
                    bail!("object {i} runs past the DLGO section");
                }
                let name = r.string(i32_at(b, o)?)?;
                let (rect, shape) = r.shape(usize::try_from(i32_at(b, o + 4)?)? / 8)?;
                objects.push(Object { name, rect, shape });
            }
            Ok(Dialog {
                name: name.clone(),
                rect,
                objects,
            })
        };
        dialogs.push(one().with_context(|| format!("dialog {name}"))?);
    }
    Ok(Drb { textures, dialogs })
}

#[cfg(test)]
mod tests {
    use super::*;

    enum S {
        Null,
        Dialog,
        Sprite {
            uv: Rect,
            texture: i16,
            blend: u8,
            tint: [u8; 4],
        },
        Mono([u8; 4]),
        Gouraud([[u8; 4]; 4]),
        Text {
            color: [u8; 4],
            size: i32,
        },
        Odd(&'static str),
    }

    struct Obj(&'static str, Rect, S);

    struct Dlg(&'static str, Rect, Vec<Obj>);

    fn r(l: i16, t: i16, rr: i16, b: i16) -> Rect {
        Rect {
            left: l,
            top: t,
            right: rr,
            bottom: b,
        }
    }

    /// The bytes of a shape record: the rectangle, then the type's fields where the reader looks.
    fn record(rect: Rect, s: &S) -> (&'static str, Vec<u8>) {
        let mut v = Vec::new();
        for x in [rect.left, rect.top, rect.right, rect.bottom] {
            v.extend_from_slice(&x.to_le_bytes());
        }
        let (ty, size) = match s {
            S::Null => ("Null", 8),
            S::Dialog => ("Dialog", 32),
            S::Sprite { .. } => ("Sprite", 36),
            S::Mono(_) => ("MonoRect", 28),
            S::Gouraud(_) => ("GouraudRect", 36),
            S::Text { .. } => ("Text", 46),
            S::Odd(t) => (*t, 30),
        };
        v.resize(size, 0);
        match s {
            S::Sprite {
                uv,
                texture,
                blend,
                tint,
            } => {
                for (i, x) in [uv.left, uv.top, uv.right, uv.bottom].iter().enumerate() {
                    v[16 + i * 2..18 + i * 2].copy_from_slice(&x.to_le_bytes());
                }
                v[24..26].copy_from_slice(&texture.to_le_bytes());
                v[27] = *blend;
                v[32..36].copy_from_slice(tint);
            }
            S::Mono(c) => v[24..28].copy_from_slice(c),
            S::Gouraud(cs) => {
                for (i, c) in cs.iter().enumerate() {
                    v[20 + i * 4..24 + i * 4].copy_from_slice(c);
                }
            }
            S::Text { color, size } => {
                v[24..28].copy_from_slice(color);
                v[32..36].copy_from_slice(&size.to_le_bytes());
            }
            _ => {}
        }
        (ty, v)
    }

    fn section(out: &mut Vec<u8>, tag: &[u8; 4], count: usize, body: &[u8]) {
        out.extend_from_slice(tag);
        out.extend_from_slice(&(body.len() as i32).to_le_bytes());
        out.extend_from_slice(&(count as i32).to_le_bytes());
        out.extend_from_slice(&0i32.to_le_bytes());
        out.extend_from_slice(body);
    }

    /// A DRB holding `dialogs`, with `extra_anim` bytes after the (empty) animation table.
    fn drb_of(textures: &[&str], dialogs: &[Dlg], extra_anim: &[u8]) -> Vec<u8> {
        let mut strs: Vec<u8> = Vec::new();
        let mut at = HashMap::new();
        let mut intern = |s: &str, strs: &mut Vec<u8>| -> i32 {
            *at.entry(s.to_string()).or_insert_with(|| {
                let off = strs.len() as i32;
                for u in s.encode_utf16() {
                    strs.extend_from_slice(&u.to_le_bytes());
                }
                strs.extend_from_slice(&[0, 0]);
                off
            })
        };
        let mut texi = Vec::new();
        for t in textures {
            texi.extend_from_slice(&intern(t, &mut strs).to_le_bytes());
            texi.extend_from_slice(&intern("N:\\path", &mut strs).to_le_bytes());
            texi.extend_from_slice(&[0; 8]);
        }
        let (mut shpr, mut shap, mut ctrl) = (Vec::new(), Vec::new(), Vec::new());
        let (mut dlgo, mut dlg) = (Vec::new(), Vec::new());
        let mut entries = 0usize;
        let mut push = |name: &str, rect: Rect, s: &S, strs: &mut Vec<u8>, shpr: &mut Vec<u8>| {
            let (ty, rec) = record(rect, s);
            shap.extend_from_slice(&intern(ty, strs).to_le_bytes());
            shap.extend_from_slice(&(shpr.len() as i32).to_le_bytes());
            shpr.extend_from_slice(&rec);
            ctrl.extend_from_slice(&intern("Static", strs).to_le_bytes());
            ctrl.extend_from_slice(&0i32.to_le_bytes());
            entries += 1;
            (intern(name, strs), ((entries - 1) * 8) as i32)
        };
        let mut dlgo_count = 0usize;
        for Dlg(name, rect, objs) in dialogs {
            let (name_off, shap_off) = push(name, *rect, &S::Null, &mut strs, &mut shpr);
            let first = dlgo.len();
            for Obj(oname, orect, s) in objs {
                let (n, so) = push(oname, *orect, s, &mut strs, &mut shpr);
                dlgo.extend_from_slice(&n.to_le_bytes());
                dlgo.extend_from_slice(&so.to_le_bytes());
                dlgo.extend_from_slice(&so.to_le_bytes());
                dlgo.extend_from_slice(&[0; 20]);
                dlgo_count += 1;
            }
            let mut rec = vec![0u8; 64];
            rec[0..4].copy_from_slice(&name_off.to_le_bytes());
            rec[4..8].copy_from_slice(&shap_off.to_le_bytes());
            rec[8..12].copy_from_slice(&shap_off.to_le_bytes());
            rec[32..36].copy_from_slice(&(objs.len() as i32).to_le_bytes());
            rec[36..40].copy_from_slice(&(first as i32).to_le_bytes());
            dlg.extend_from_slice(&rec);
        }
        let mut out = b"DRB\0".to_vec();
        out.extend_from_slice(&[0; 12]);
        section(&mut out, b"STR\0", 0, &strs);
        section(&mut out, b"TEXI", textures.len(), &texi);
        section(&mut out, b"SHPR", 1, &shpr);
        section(&mut out, b"CTPR", 1, &[]);
        section(&mut out, b"ANIP", 1, &[]);
        section(&mut out, b"INTP", 1, &[]);
        section(&mut out, b"SCDP", 1, &[]);
        section(&mut out, b"SHAP", entries, &shap);
        section(&mut out, b"CTRL", entries, &ctrl);
        section(&mut out, b"ANIK", 0, &[]);
        section(&mut out, b"ANIO", 0, &[]);
        // The header length covers a 96-byte table; `extra_anim` follows it, uncounted.
        let table = [0u8; 96];
        out.extend_from_slice(b"ANIM");
        out.extend_from_slice(&(table.len() as i32).to_le_bytes());
        out.extend_from_slice(&2i32.to_le_bytes());
        out.extend_from_slice(&0i32.to_le_bytes());
        out.extend_from_slice(&table);
        out.extend_from_slice(extra_anim);
        section(&mut out, b"SCDL", 0, &[]);
        section(&mut out, b"DLGO", dlgo_count, &dlgo);
        section(&mut out, b"DLG\0", dialogs.len(), &dlg);
        section(&mut out, b"END\0", 0, &[]);
        out
    }

    fn hud() -> Vec<Dlg> {
        vec![
            Dlg(
                "F20-00",
                r(0, 0, 1280, 720),
                vec![
                    Obj(
                        "bar_1_hp",
                        r(175, 76, 907, 89),
                        S::Sprite {
                            uv: r(6, 488, 138, 512),
                            texture: 1,
                            blend: 1,
                            tint: [0xFF, 0xFF, 0xFF, 0xFF],
                        },
                    ),
                    Obj(
                        "plate_hp",
                        r(175, 76, 907, 89),
                        S::Mono([0xFF, 0x0A, 0x0A, 0x0A]),
                    ),
                    Obj("enemy_HP0", r(370, 577, 466, 611), S::Dialog),
                    Obj(
                        "fade",
                        r(0, 283, 1280, 327),
                        S::Gouraud([[0; 4], [0; 4], [0xC8, 0, 0, 0], [0xC8, 0, 0, 0]]),
                    ),
                    Obj(
                        "n",
                        r(1041, 630, 1169, 658),
                        S::Text {
                            color: [0xFF, 0x9C, 0xB9, 0xCA],
                            size: 28,
                        },
                    ),
                    Obj("odd", r(1, 2, 3, 4), S::Odd("ScrollText")),
                    Obj("nothing", r(5, 6, 7, 8), S::Null),
                ],
            ),
            Dlg(
                "enemy_HP",
                r(0, 0, 95, 35),
                vec![Obj(
                    "BG",
                    r(1, 27, 94, 31),
                    S::Sprite {
                        uv: r(1054, 586, 1270, 594),
                        texture: 0,
                        blend: 2,
                        tint: [0xFF, 0xC8, 0xC8, 0xFF],
                    },
                )],
            ),
        ]
    }

    #[test]
    fn a_dialog_with_a_sprite_decodes() {
        let drb = read(&drb_of(&["Menu04", "Menu08"], &hud(), &[])).unwrap();
        assert_eq!(drb.textures, ["Menu04", "Menu08"]);
        assert_eq!(drb.dialogs.len(), 2);
        let d = drb.dialog("F20-00").unwrap();
        assert_eq!(d.rect, r(0, 0, 1280, 720));
        assert_eq!(d.rect.width(), 1280);
        assert_eq!(d.objects.len(), 7);
        let bar = &d.objects[0];
        assert_eq!(bar.name, "bar_1_hp");
        assert_eq!(bar.rect, r(175, 76, 907, 89));
        assert_eq!(
            bar.shape,
            Shape::Sprite {
                uv: r(6, 488, 138, 512),
                texture: 1,
                blend: 1,
                tint: 0xFFFF_FFFF
            }
        );
    }

    #[test]
    fn mono_rect_color_reads_argb_in_byte_order() {
        let drb = read(&drb_of(&[], &hud(), &[])).unwrap();
        let plate = &drb.dialog("F20-00").unwrap().objects[1];
        assert_eq!(plate.shape, Shape::MonoRect { color: 0xFF0A_0A0A });
    }

    #[test]
    fn gouraud_colours_run_top_left_top_right_bottom_left_bottom_right() {
        let drb = read(&drb_of(&[], &hud(), &[])).unwrap();
        let fade = &drb.dialog("F20-00").unwrap().objects[3];
        assert_eq!(
            fade.shape,
            Shape::GouraudRect {
                colors: [0, 0, 0xC800_0000, 0xC800_0000]
            }
        );
    }

    #[test]
    fn text_reads_its_colour_and_size() {
        let drb = read(&drb_of(&[], &hud(), &[])).unwrap();
        let t = &drb.dialog("F20-00").unwrap().objects[4];
        assert_eq!(
            t.shape,
            Shape::Text {
                color: 0xFF9C_B9CA,
                size: 28
            }
        );
    }

    #[test]
    fn a_sub_dialog_object_is_a_dialog_shape_found_by_name() {
        let drb = read(&drb_of(&["a"], &hud(), &[])).unwrap();
        let sub = &drb.dialog("F20-00").unwrap().objects[2];
        assert_eq!(
            (sub.name.as_str(), &sub.shape),
            ("enemy_HP0", &Shape::Dialog)
        );
        assert_eq!(sub.rect, r(370, 577, 466, 611));
        let e = drb.dialog("enemy_HP").unwrap();
        assert_eq!(e.rect, r(0, 0, 95, 35));
        assert!(matches!(e.objects[0].shape, Shape::Sprite { blend: 2, .. }));
        assert!(drb.dialog("nope").is_none());
    }

    #[test]
    fn unknown_shape_types_are_other_and_null_is_null() {
        let drb = read(&drb_of(&[], &hud(), &[])).unwrap();
        let d = drb.dialog("F20-00").unwrap();
        assert_eq!(d.objects[5].shape, Shape::Other("ScrollText".into()));
        assert_eq!(d.objects[6].shape, Shape::Null);
        assert_eq!(d.objects[6].rect, r(5, 6, 7, 8));
    }

    #[test]
    fn an_anim_section_longer_than_its_table_is_skipped() {
        // Junk after the table, with a false `SCDL` in it that no `DLGO` follows.
        let mut junk = vec![7u8; 200];
        junk[50..54].copy_from_slice(b"SCDL");
        junk[54..58].copy_from_slice(&4i32.to_le_bytes());
        let drb = read(&drb_of(&["a"], &hud(), &junk)).unwrap();
        assert_eq!(drb.dialogs.len(), 2);
        assert_eq!(drb.textures, ["a"]);
    }

    #[test]
    fn foreign_and_broken_files_are_errors() {
        assert!(read(b"BND3 not a drb at all..").is_err());
        let good = drb_of(&["a"], &hud(), &[]);
        assert!(read(&good[..good.len() - 40]).is_err());
        let mut bad = good.clone();
        bad[16..20].copy_from_slice(b"XXXX");
        assert!(read(&bad).is_err());
    }

    /// `DSR_DIR`: the shipped HUD layout, the facts the HUD is built on.
    #[test]
    fn install_the_menu_drb_reads() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let raw = std::fs::read(format!("{dir}/menu/menu.drb.dcx")).unwrap();
        let drb = read(&crate::dcx::decompress(&raw).unwrap()).unwrap();
        assert_eq!(drb.dialogs.len(), 638);
        assert_eq!(drb.textures.len(), 53);
        assert_eq!(drb.textures[9], "Menu08");
        let hud = drb.dialog("F20-00").unwrap();
        assert_eq!(hud.objects.len(), 92);
        let find = |d: &Dialog, n: &str| d.objects.iter().find(|o| o.name == n).cloned().unwrap();
        let bar = find(hud, "bar_1_hp");
        assert_eq!(bar.rect, r(175, 76, 907, 89));
        assert_eq!(
            bar.shape,
            Shape::Sprite {
                uv: r(6, 488, 138, 512),
                texture: 9,
                blend: 1,
                tint: 0xFFFF_FFFF
            }
        );
        assert_eq!(
            find(hud, "plate_hp").shape,
            Shape::MonoRect { color: 0xFF0A_0A0A }
        );
        let died = drb.dialog("F23_YouDied").unwrap();
        assert_eq!(
            find(died, "BG_bar").shape,
            Shape::MonoRect { color: 0xC800_0000 }
        );
        assert_eq!(
            find(died, "BG_grad00").shape,
            Shape::GouraudRect {
                colors: [0, 0, 0xC800_0000, 0xC800_0000]
            }
        );
        let text = find(died, "main_text_youdied");
        assert!(matches!(text.shape, Shape::Sprite { blend: 2, .. }));
        assert_eq!(text.rect, r(128, 324, 1152, 424));
        let enemy = drb.dialog("enemy_HP").unwrap();
        assert!(matches!(
            find(enemy, "text_param_name").shape,
            Shape::Text { size: 28, .. }
        ));
        let soul = drb.dialog("soul_param").unwrap();
        assert_eq!(find(soul, "text_param_soul").rect, r(1041, 630, 1169, 658));
    }
}
