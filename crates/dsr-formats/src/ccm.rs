//! CCM bitmap-font tables (SoulsFormatsNEXT `CCM.cs`, the `0x10001` layout of Dark Souls): which
//! rectangle of which texture page holds each glyph, and how far to move the pen.
//!
//! Header (little endian): `i32` version, `i32` file size, `i16` font size, `i16` page width,
//! `i16` page height, `i16` (32, unknown), `u16` group count at 0x10, `u16` glyph count at 0x12,
//! `i32` groups offset at 0x14 and `i32` glyphs offset at 0x18. A group is `i32 first, last,
//! glyph index` (12 bytes: the code points `first..=last` take the glyphs from `index` on). A glyph
//! is 24 bytes: `f32` u0 v0 u1 v1 (normalised page coordinates), `i16` pre-space, bitmap width,
//! advance, page. Draw the bitmap at `pen + pre`, then move the pen by `advance`.

use anyhow::{bail, Result};

use crate::bytes::{f32_at, i16_at, i32_at, slice, u16_at};

/// The version word of the layout this reader knows.
const VERSION: i32 = 0x0001_0001;

/// One glyph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    /// The bitmap's rectangle in its page as the file stores it, normalised: left, top, right,
    /// bottom (see [`Font::pixels`]).
    pub uv: [f32; 4],
    /// Pixels between the pen and the bitmap's left edge.
    pub pre: i16,
    /// The bitmap's width in pixels.
    pub width: i16,
    /// How far the pen moves after the glyph.
    pub advance: i16,
    /// The texture page (the TPF's texture index).
    pub page: i16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Group {
    first: u32,
    last: u32,
    index: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    /// The nominal size in pixels (the atlas cell height, 58 for DSFont24).
    pub size: i16,
    /// The page size in pixels.
    pub width: i16,
    pub height: i16,
    groups: Vec<Group>,
    pub glyphs: Vec<Glyph>,
}

impl Font {
    /// A glyph's rectangle in its page's pixels (the file's normalised values are whole pixels).
    pub fn pixels(&self, g: &Glyph) -> [f32; 4] {
        let (w, h) = (f32::from(self.width), f32::from(self.height));
        [
            (g.uv[0] * w).round(),
            (g.uv[1] * h).round(),
            (g.uv[2] * w).round(),
            (g.uv[3] * h).round(),
        ]
    }

    /// The glyph for a code point, if the font has one.
    pub fn glyph(&self, cp: u32) -> Option<&Glyph> {
        let g = self
            .groups
            .iter()
            .find(|g| (g.first..=g.last).contains(&cp))?;
        self.glyphs.get((g.index + cp - g.first) as usize)
    }
}

pub fn read(b: &[u8]) -> Result<Font> {
    let version = i32_at(b, 0)?;
    if version != VERSION {
        bail!("CCM version {version:#x} is not {VERSION:#x}");
    }
    let size = i16_at(b, 8)?;
    let width = i16_at(b, 0x0A)?;
    let height = i16_at(b, 0x0C)?;
    let group_count = u16_at(b, 0x10)? as usize;
    let glyph_count = u16_at(b, 0x12)? as usize;
    let groups_at = i32_at(b, 0x14)? as usize;
    let glyphs_at = i32_at(b, 0x18)? as usize;
    slice(b, groups_at, group_count * 12)?;
    slice(b, glyphs_at, glyph_count * 24)?;
    let mut groups = Vec::with_capacity(group_count);
    for i in 0..group_count {
        let at = groups_at + i * 12;
        groups.push(Group {
            first: i32_at(b, at)? as u32,
            last: i32_at(b, at + 4)? as u32,
            index: i32_at(b, at + 8)? as u32,
        });
    }
    let mut glyphs = Vec::with_capacity(glyph_count);
    for i in 0..glyph_count {
        let at = glyphs_at + i * 24;
        glyphs.push(Glyph {
            uv: [
                f32_at(b, at)?,
                f32_at(b, at + 4)?,
                f32_at(b, at + 8)?,
                f32_at(b, at + 12)?,
            ],
            pre: i16_at(b, at + 16)?,
            width: i16_at(b, at + 18)?,
            advance: i16_at(b, at + 20)?,
            page: i16_at(b, at + 22)?,
        });
    }
    Ok(Font {
        size,
        width,
        height,
        groups,
        glyphs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A CCM with `groups` and `glyphs`, laid out as DSR's: groups at 0x20, glyphs after them.
    fn ccm_of(groups: &[(i32, i32, i32)], glyphs: &[Glyph]) -> Vec<u8> {
        let groups_at = 0x20usize;
        let glyphs_at = groups_at + groups.len() * 12;
        let mut out = Vec::new();
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&((glyphs_at + glyphs.len() * 24) as i32).to_le_bytes());
        out.extend_from_slice(&58i16.to_le_bytes());
        out.extend_from_slice(&1024i16.to_le_bytes());
        out.extend_from_slice(&512i16.to_le_bytes());
        out.extend_from_slice(&32i16.to_le_bytes());
        out.extend_from_slice(&(groups.len() as u16).to_le_bytes());
        out.extend_from_slice(&(glyphs.len() as u16).to_le_bytes());
        out.extend_from_slice(&(groups_at as i32).to_le_bytes());
        out.extend_from_slice(&(glyphs_at as i32).to_le_bytes());
        out.extend_from_slice(&[1, 0, 0x10, 0]);
        for (a, b, c) in groups {
            for v in [a, b, c] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for g in glyphs {
            for f in g.uv {
                out.extend_from_slice(&f.to_le_bytes());
            }
            for v in [g.pre, g.width, g.advance, g.page] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out
    }

    fn glyph(n: i16) -> Glyph {
        Glyph {
            uv: [n as f32, 1.0, n as f32 + 20.0, 59.0],
            pre: n % 7,
            width: 20,
            advance: n,
            page: n % 3,
        }
    }

    fn font() -> Font {
        let glyphs: Vec<Glyph> = (0..191).map(glyph).collect();
        read(&ccm_of(&[(32, 126, 0), (160, 255, 95)], &glyphs)).unwrap()
    }

    #[test]
    fn a_code_point_finds_its_glyph_through_the_groups() {
        let f = font();
        assert_eq!((f.size, f.width, f.height), (58, 1024, 512));
        assert_eq!(f.glyph(32).unwrap().advance, 0);
        assert_eq!(f.glyph(0x41).unwrap().advance, 33);
        assert_eq!(f.glyph(126).unwrap().advance, 94);
        assert_eq!(f.glyph(160).unwrap().advance, 95);
        assert_eq!(f.glyph(255).unwrap().advance, 190);
    }

    #[test]
    fn a_code_point_between_groups_is_none() {
        let f = font();
        assert!(f.glyph(31).is_none());
        assert!(f.glyph(127).is_none());
        assert!(f.glyph(159).is_none());
        assert!(f.glyph(256).is_none());
    }

    #[test]
    fn glyph_fields_read_back() {
        let g = font().glyph(0x30).unwrap().to_owned();
        assert_eq!(g, glyph(16));
        assert_eq!(g.uv, [16.0, 1.0, 36.0, 59.0]);
    }

    #[test]
    fn normalised_coordinates_become_page_pixels() {
        let g = Glyph {
            uv: [354.0 / 1024.0, 1.0 / 512.0, 381.0 / 1024.0, 59.0 / 512.0],
            pre: 3,
            width: 27,
            advance: 27,
            page: 0,
        };
        let f = read(&ccm_of(&[(48, 48, 0)], &[g])).unwrap();
        assert_eq!(f.pixels(f.glyph(48).unwrap()), [354.0, 1.0, 381.0, 59.0]);
    }

    #[test]
    fn a_group_pointing_past_the_glyphs_gives_none() {
        let f = read(&ccm_of(&[(32, 40, 0)], &[glyph(1), glyph(2)])).unwrap();
        assert!(f.glyph(33).is_some());
        assert!(f.glyph(34).is_none());
    }

    #[test]
    fn short_or_foreign_files_are_errors() {
        let bytes = ccm_of(&[(32, 126, 0)], &(0..95).map(glyph).collect::<Vec<_>>());
        assert!(read(&bytes[..bytes.len() - 1]).is_err());
        assert!(read(&bytes[..0x10]).is_err());
        let mut other = bytes.clone();
        other[0..4].copy_from_slice(&0x20000i32.to_le_bytes());
        assert!(read(&other).is_err());
    }

    /// `DSR_DIR`: the English HUD font reads, with the numbers the survey found.
    #[test]
    fn install_the_dsfont_reads() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let raw = std::fs::read(format!("{dir}/font/english/DSFont24.ccm.dcx")).unwrap();
        let f = read(&crate::dcx::decompress(&raw).unwrap()).unwrap();
        assert_eq!((f.size, f.width, f.height), (58, 1024, 1024));
        assert_eq!(f.glyphs.len(), 1284);
        let zero = f.glyph('0' as u32).unwrap();
        assert_eq!(f.pixels(zero), [354.0, 1.0, 381.0, 59.0]);
        assert_eq!(
            (zero.pre, zero.width, zero.advance, zero.page),
            (3, 27, 27, 0)
        );
        let one = f.glyph('1' as u32).unwrap();
        assert_eq!((one.pre, one.width, one.advance), (7, 18, 23));
        assert_eq!(f.glyph('-' as u32).unwrap().page, 6);
        assert!(f.glyph('A' as u32).is_some());
    }
}
