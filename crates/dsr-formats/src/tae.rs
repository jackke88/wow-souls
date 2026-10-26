//! DS1 TAE (`TAE `, version 0x1000B, 32-bit offsets): per animation, its timed events
//! (SoulsFormatsNEXT `TAE.cs`, `Animation.cs`, `Event.cs`, DS1 branches).

use anyhow::{bail, Result};

use crate::bytes::{f32_at, i32_at, slice, u32_at, u8_at};

/// How many parameter bytes an event keeps: enough for every field the extractor reads.
pub const PARAM_BYTES: usize = 16;
/// Event type 0, `ChrActionFlag`; its first parameter is the flag.
pub const CHR_ACTION_FLAG: i32 = 0;
/// Event type 1, `InvokeAttackBehavior`: an attack's hit window. Its third parameter int is the
/// behavior judge id, which keys `BehaviorParam_PC`.
pub const ATTACK_BEHAVIOR: i32 = 1;
/// Event type 64, `InvokeBulletBehavior`: a magic bullet leaves the hand (a pyromancy or a miracle
/// clip has one, a sorcery clip does not: its release is the state machine's). Its first parameter
/// is the behavior judge id.
pub const BULLET_BEHAVIOR: i32 = 64;

/// One timed event of an animation.
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
}

/// An animation id and its events.
#[derive(Debug, Clone, PartialEq)]
pub struct Anim {
    pub id: u32,
    pub events: Vec<Event>,
    /// Standard mini-header HKX import, or a full animation import.
    pub import: Option<(u32, bool)>,
}

/// Reads every animation's events out of a DS1 (32-bit, little-endian) TAE file.
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
            events.push(Event {
                kind,
                start,
                end,
                params,
            });
        }
        let file = u32_at(b, record + 24)? as usize;
        let import = if file == 0 {
            None
        } else {
            let kind = u32_at(b, file)?;
            let inner = u32_at(b, file + 4)? as usize;
            if inner == 0 {
                None
            } else if kind == 1 {
                Some((u32_at(b, inner + 4)?, true))
            } else if kind == 0 && u8_at(b, inner + 5)? != 0 {
                Some((u32_at(b, inner + 8)?, false))
            } else {
                None
            }
        };
        anims.push(Anim { id, events, import });
    }
    Ok(anims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bnd3, dcx};

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
    fn imports_preserve_whether_only_hkx_or_events_are_reused() {
        let mut b = sample();
        b.resize(0xD0, 0);
        b[0x80..0x84].copy_from_slice(&0xB0u32.to_le_bytes());
        b[0xB4..0xB8].copy_from_slice(&0xB8u32.to_le_bytes());
        b[0xBD] = 1;
        b[0xC0..0xC4].copy_from_slice(&354001u32.to_le_bytes());
        assert_eq!(read(&b).unwrap()[0].import, Some((354001, false)));
        b[0xB0..0xB4].copy_from_slice(&1u32.to_le_bytes());
        b[0xBC..0xC0].copy_from_slice(&4001u32.to_le_bytes());
        let a = read(&b).unwrap();
        assert_eq!(a[0].import, Some((4001, true)));
        assert_eq!(a[0].events[0].action_flag(), Some(8));
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

    #[test]
    fn install_player_tae_has_invincible_rolls() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let raw = std::fs::read(format!("{dir}/chr/c0000.anibnd.dcx")).unwrap();
        let entries = bnd3::read(&dcx::decompress(&raw).unwrap()).unwrap();
        let tae = entries
            .iter()
            .find(|e| e.file_name() == "a00.tae")
            .expect("a00.tae in c0000.anibnd");
        let anims = read(&tae.data).unwrap();
        let invincible: Vec<(u32, f32, f32)> = anims
            .iter()
            .flat_map(|a| {
                a.events
                    .iter()
                    .filter(|e| e.action_flag() == Some(8))
                    .map(move |e| (a.id, e.start, e.end))
            })
            .filter(|&(_, s, e)| 0.0 <= s && s < e && e < 2.0)
            .collect();
        eprintln!(
            "{} animations; {} flag-8 events with 0 <= start < end < 2.0; first: {:?}",
            anims.len(),
            invincible.len(),
            invincible.first()
        );
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
        assert_eq!(
            event(ATTACK_BEHAVIOR, [0, 0, 540, 0]).attack_judge(),
            Some(540)
        );
        assert_eq!(
            event(ATTACK_BEHAVIOR, [0, 0, 0, 128]).attack_judge(),
            Some(0)
        );
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
        for (id, want) in [
            (3000, 0),
            (3001, 10),
            (3002, 20),
            (3300, 100),
            (3310, 110),
            (3900, 540),
        ] {
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
