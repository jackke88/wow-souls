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

/// Reads every file out of a little-endian BND3 archive, decompressing entries that are DCX.
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
        let flags = if bit_big_endian {
            raw_flags
        } else {
            raw_flags.reverse_bits()
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-file BND3 in DSR's layout (raw format byte 0x74, 24-byte headers).
    fn sample() -> Vec<u8> {
        let files: [(&str, i32, &[u8]); 2] = [
            ("N:\\x\\a00.tae", 5_000_000, b"TAE data"),
            ("N:\\x\\Skeleton.hkx", 4_000_000, b"HKX"),
        ];
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
        assert_eq!(
            (e[0].id, e[0].file_name(), e[0].stem()),
            (5_000_000, "a00.tae", "a00")
        );
        assert_eq!(e[0].data, b"TAE data");
        assert_eq!(
            (e[1].file_name(), e[1].data.as_slice()),
            ("Skeleton.hkx", &b"HKX"[..])
        );
    }

    #[test]
    fn a_non_bnd3_is_refused() {
        assert!(read(b"BND4 nope").is_err());
    }

    #[test]
    fn install_player_archive_lists_its_files() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let raw = std::fs::read(format!("{dir}/chr/c0000.anibnd.dcx")).unwrap();
        let entries = read(&dcx::decompress(&raw).unwrap()).unwrap();
        eprintln!(
            "{} entries; first: {:?} id {}",
            entries.len(),
            entries[0].name,
            entries[0].id
        );
        assert_eq!(entries.len(), 162);
        assert!(
            entries[0].name.ends_with("hkxx64\\Skeleton.hkx"),
            "{}",
            entries[0].name
        );
        assert_eq!(entries[0].id, 4_000_000);
    }
}
