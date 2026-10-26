//! `.wscl`: a sampled skeletal clip. The Dark Souls extractor (`dsr-cache`, GPL-3.0) writes
//! them and the game reads them; this crate is the whole contract between the two, so the game
//! never links the extractor. Little-endian throughout.
//!
//! Layout: magic `WSCL`, version u32, name (u32 length + UTF-8), fps f32, model-space flag u8,
//! bone count u32, frame count u32, parents (i16 per bone), driven (u8 per bone), frames
//! (per frame per bone: translation 3 f32, rotation x y z w 4 f32, scale 3 f32), root motion
//! (per frame 4 f32), flag-window count u32 and windows (flag i32, start f32, end f32), hit-window
//! count u32 and windows (behavior judge i32, start f32, end f32).

use std::io::{self, Read, Write};

pub mod text;

pub const MAGIC: [u8; 4] = *b"WSCL";
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
pub const EXTRACTOR_VERSION: u32 = 5;

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

/// An attack's hit window from the animation's TAE (an attack-behavior event): the blade can hit
/// over `[start, end)`; `judge` keys the behavior params (stamina, damage).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitWindow {
    pub judge: i32,
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
    pub hits: Vec<HitWindow>,
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
        put_u32(w, self.hits.len() as u32)?;
        for h in &self.hits {
            w.write_all(&h.judge.to_le_bytes())?;
            put_f32(w, h.start)?;
            put_f32(w, h.end)?;
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_1_SQRT_2;

    fn sample() -> Clip {
        let a = Trs {
            t: [1.0, 2.0, 3.0],
            r: [0.0, FRAC_1_SQRT_2, 0.0, FRAC_1_SQRT_2],
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
            hits: vec![HitWindow {
                judge: 540,
                start: 0.25,
                end: 0.35,
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
