//! The game's own names for its creatures: `NpcParam.nameId` looked up in the `NpcName` text of
//! `msg/ENGLISH/menu.msgbnd.dcx` (a BND3 of FMG files). Read-only; used by `survey` so that a
//! creature is chosen by what the game calls it, not by a guess from its souls or its bones.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, ensure, Context, Result};
use dsr_formats::{bnd3, dcx};

/// Every `id -> string` of one FMG (Dark Souls 1, little-endian, 32-bit offsets): a header of
/// `0x1C` bytes (group count at 0x0C, string count at 0x10, offset table at 0x14), `groupCount` groups of `(first string index, first id, last id)`, an offset table
/// of `stringCount` 32-bit offsets (0 = no string) and UTF-16LE strings (the install's `NPC_name_.fmg`: 20 groups, 153 strings).
pub fn parse_fmg(b: &[u8]) -> Result<BTreeMap<i64, String>> {
    let u32_at = |at: usize| -> Result<u32> {
        let s = b.get(at..at + 4).context("FMG cut short")?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    ensure!(b.len() > 0x1C, "FMG too short");
    let groups = u32_at(0x0C)? as usize;
    let strings = u32_at(0x10)? as usize;
    let offsets = u32_at(0x14)? as usize;
    ensure!(
        groups < 100_000 && strings < 1_000_000,
        "FMG counts are not sane"
    );
    let mut out = BTreeMap::new();
    for g in 0..groups {
        let at = 0x1C + g * 12;
        let first_index = u32_at(at)? as usize;
        let first_id = u32_at(at + 4)? as i64;
        let last_id = u32_at(at + 8)? as i64;
        ensure!(last_id >= first_id, "FMG group {g} runs backwards");
        for k in 0..=(last_id - first_id) as usize {
            let index = first_index + k;
            if index >= strings {
                bail!("FMG group {g} points past the string table");
            }
            let offset = u32_at(offsets + index * 4)? as usize;
            if offset == 0 {
                continue;
            }
            let mut units = Vec::new();
            let mut p = offset;
            loop {
                let s = b.get(p..p + 2).context("FMG string cut short")?;
                let unit = u16::from_le_bytes([s[0], s[1]]);
                if unit == 0 {
                    break;
                }
                units.push(unit);
                p += 2;
            }
            out.insert(first_id + k as i64, String::from_utf16_lossy(&units));
        }
    }
    Ok(out)
}

/// The creature names of the install: the FMG of `menu.msgbnd` that names NPCs.
pub fn npc_names(dsr: &Path) -> Result<BTreeMap<i64, String>> {
    let mut seen = Vec::new();
    // The NPC name table is in `item.msgbnd` or `menu.msgbnd`; its file name carries `NPC`.
    for bundle in ["item.msgbnd.dcx", "menu.msgbnd.dcx"] {
        let path = dsr.join("msg").join("ENGLISH").join(bundle);
        let raw = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let entries = bnd3::read(&dcx::decompress(&raw)?)?;
        for e in entries
            .iter()
            .filter(|e| e.file_name().to_ascii_lowercase().ends_with(".fmg"))
        {
            let n = e.file_name().to_string();
            if n.to_ascii_lowercase().contains("npc") {
                match parse_fmg(&e.data) {
                    Ok(map) if !map.is_empty() => return Ok(map),
                    Ok(_) => seen.push(format!("{n} (empty)")),
                    Err(err) => seen.push(format!(
                        "{n} ({err:#}; head {:02x?})",
                        &e.data[..e.data.len().min(48)]
                    )),
                }
                continue;
            }
            seen.push(n);
        }
    }
    bail!(
        "no NPC name table in the msg bundles (FMG files: {})",
        seen.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-group FMG of two strings (ids 10 and 11) and a hole (id 12).
    fn tiny() -> Vec<u8> {
        let mut b = vec![0u8; 0x1C];
        b[8] = 1;
        b[0x0C..0x10].copy_from_slice(&1u32.to_le_bytes());
        b[0x10..0x14].copy_from_slice(&3u32.to_le_bytes());
        // Group: index 0, ids 10..=12.
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&10u32.to_le_bytes());
        b.extend_from_slice(&12u32.to_le_bytes());
        let table = b.len();
        b[0x14..0x18].copy_from_slice(&(table as u32).to_le_bytes());
        let strings = table + 12;
        let s1: Vec<u8> = "Ab\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let s2: Vec<u8> = "Cd\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
        b.extend_from_slice(&(strings as u32).to_le_bytes());
        b.extend_from_slice(&((strings + s1.len()) as u32).to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&s1);
        b.extend_from_slice(&s2);
        b
    }

    #[test]
    fn an_fmg_gives_its_strings_by_id_and_skips_holes() {
        let m = parse_fmg(&tiny()).unwrap();
        assert_eq!(m.get(&10).map(String::as_str), Some("Ab"));
        assert_eq!(m.get(&11).map(String::as_str), Some("Cd"));
        assert_eq!(m.get(&12), None);
    }

    #[test]
    fn the_install_names_the_bosses_by_chr_number() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let names = npc_names(std::path::Path::new(&dir)).unwrap();
        assert_eq!(names.get(&2250).map(String::as_str), Some("Taurus Demon"));
        assert_eq!(names.get(&2240).map(String::as_str), Some("Capra Demon"));
        assert_eq!(
            names.get(&2360).map(String::as_str),
            Some("Executioner Smough")
        );
    }

    #[test]
    fn a_cut_fmg_is_an_error() {
        let mut b = tiny();
        b.truncate(0x20);
        assert!(parse_fmg(&b).is_err());
        assert!(parse_fmg(&[0; 8]).is_err());
    }
}
