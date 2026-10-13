//! TPF texture archives, PC platform (SoulsFormatsNEXT `TPF.cs`): a list of named DDS files.
//!
//! Header: `"TPF\0"`, `i32` data size, `i32` count, `u8` platform (0 = PC), `u8` flag 2, `u8` name
//! encoding (1 = UTF-16, else a NUL-terminated byte string, Shift-JIS in the originals and ASCII
//! in DSR's), `u8` 0; then `count` 0x14-byte entries (`u32` offset, `i32` size, `u8` format,
//! `u8` type, `u8` mip count, `u8` flags, `i32` name offset, `i32` float-struct flag).

use anyhow::{bail, Context, Result};

use crate::bytes::{cstr_at, i32_at, slice, u32_at, u8_at};
use crate::dcx;

/// One texture of the archive.
#[derive(Debug, Clone, PartialEq)]
pub struct Texture {
    pub name: String,
    /// Dark Souls' own format byte (0 / 1 DXT1, 3 DXT3, 5 DXT5, ...); the DDS header decides.
    pub format: u8,
    /// A complete DDS file (a DCX-wrapped entry is inflated).
    pub dds: Vec<u8>,
}

/// Every texture in a PC TPF.
pub fn read(b: &[u8]) -> Result<Vec<Texture>> {
    if slice(b, 0, 4)? != b"TPF\0" {
        bail!("not a TPF archive");
    }
    let count = i32_at(b, 8)?;
    let platform = u8_at(b, 12)?;
    let encoding = u8_at(b, 14)?;
    if platform != 0 {
        bail!("TPF platform {platform} is not PC");
    }
    if !(0..=0xFFFF).contains(&count) {
        bail!("TPF claims {count} textures");
    }
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let at = 0x10 + i * 0x14;
        let offset = u32_at(b, at)? as usize;
        let size = u32_at(b, at + 4)? as usize;
        let format = u8_at(b, at + 8)?;
        let name_at = u32_at(b, at + 12)? as usize;
        let name = if encoding == 1 {
            utf16_at(b, name_at)?
        } else {
            cstr_at(b, name_at)?
        };
        let raw = slice(b, offset, size).with_context(|| format!("texture {name}"))?;
        // `flags1` says 2 or 3 for a wrapped entry; the magic is the surer test.
        let dds = if raw.starts_with(b"DCX\0") {
            dcx::decompress(raw).with_context(|| format!("texture {name}"))?
        } else {
            raw.to_vec()
        };
        out.push(Texture { name, format, dds });
    }
    Ok(out)
}

/// A NUL-terminated UTF-16LE string at `at`.
fn utf16_at(b: &[u8], at: usize) -> Result<String> {
    let tail = b
        .get(at..)
        .with_context(|| format!("string at {at:#x} past the end"))?;
    let units: Vec<u16> = tail
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .take_while(|&u| u != 0)
        .collect();
    if units.len() * 2 + 2 > tail.len() {
        bail!("unterminated string at {at:#x}");
    }
    Ok(String::from_utf16_lossy(&units))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TPF around `textures` = (name, bytes, flags byte).
    fn tpf_of(textures: &[(&str, Vec<u8>, u8)], encoding: u8, platform: u8) -> Vec<u8> {
        let entries_end = 0x10 + 0x14 * textures.len();
        let mut names = Vec::new();
        let mut name_at = Vec::new();
        for (n, _, _) in textures {
            name_at.push(entries_end + names.len());
            if encoding == 1 {
                for u in n.encode_utf16() {
                    names.extend_from_slice(&u.to_le_bytes());
                }
                names.extend_from_slice(&[0, 0]);
            } else {
                names.extend_from_slice(n.as_bytes());
                names.push(0);
            }
        }
        let mut at = entries_end + names.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"TPF\0");
        out.extend_from_slice(&0i32.to_le_bytes());
        out.extend_from_slice(&(textures.len() as i32).to_le_bytes());
        out.extend_from_slice(&[platform, 3, encoding, 0]);
        for (i, (_, data, flags)) in textures.iter().enumerate() {
            out.extend_from_slice(&(at as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&[5, 0, 1, *flags]);
            out.extend_from_slice(&(name_at[i] as u32).to_le_bytes());
            out.extend_from_slice(&0i32.to_le_bytes());
            at += data.len();
        }
        out.extend_from_slice(&names);
        for (_, d, _) in textures {
            out.extend_from_slice(d);
        }
        out
    }

    #[test]
    fn names_formats_and_bytes_come_back() {
        let tpf = tpf_of(
            &[
                ("Menu08", b"DDS first".to_vec(), 0),
                ("flags", b"DDS second".to_vec(), 0),
            ],
            2,
            0,
        );
        let t = read(&tpf).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].name, "Menu08");
        assert_eq!(t[0].format, 5);
        assert_eq!(t[0].dds, b"DDS first");
        assert_eq!(t[1].name, "flags");
        assert_eq!(t[1].dds, b"DDS second");
    }

    #[test]
    fn utf16_names_decode() {
        let tpf = tpf_of(&[("Icon00", b"DDS".to_vec(), 0)], 1, 0);
        assert_eq!(read(&tpf).unwrap()[0].name, "Icon00");
    }

    #[test]
    fn a_dcx_wrapped_texture_is_inflated() {
        let payload = b"DDS payload that is long enough to compress".repeat(8);
        let tpf = tpf_of(&[("Wrapped", crate::dcx::tests::dcx_of(&payload), 2)], 2, 0);
        assert_eq!(read(&tpf).unwrap()[0].dds, payload);
    }

    #[test]
    fn other_platforms_are_refused() {
        let tpf = tpf_of(&[("x", b"DDS".to_vec(), 0)], 2, 4);
        assert!(read(&tpf).is_err());
    }

    #[test]
    fn truncated_input_is_an_error() {
        let tpf = tpf_of(&[("x", b"DDS data".to_vec(), 0)], 2, 0);
        assert!(read(&tpf[..tpf.len() - 3]).is_err());
        assert!(read(&tpf[..0x18]).is_err());
        assert!(read(b"BND3").is_err());
    }

    /// `DSR_DIR`: the menu archives read; `menu_1` holds the HUD atlas as a DXT5 2048 x 1024 DDS.
    #[test]
    fn install_the_menu_tpfs_read() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let raw = std::fs::read(format!("{dir}/menu/menu_1.tpf.dcx")).unwrap();
        let t = read(&dcx::decompress(&raw).unwrap()).unwrap();
        assert_eq!(t.len(), 9);
        let atlas = t.iter().find(|t| t.name == "Menu08").expect("Menu08");
        assert_eq!(&atlas.dds[..4], b"DDS ");
        let height = u32::from_le_bytes(atlas.dds[0x0C..0x10].try_into().unwrap());
        let width = u32::from_le_bytes(atlas.dds[0x10..0x14].try_into().unwrap());
        assert_eq!((width, height), (2048, 1024));
        assert_eq!(&atlas.dds[0x54..0x58], b"DXT5");
    }
}
