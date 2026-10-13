//! The player's Dark Souls data: its skeleton, its TAE files and its animation files, read from
//! `chr/c0000.anibnd.dcx` and every `chr/c0000_a*.anibnd.dcx` (`a00_lo/md/hi` hold the common
//! `a00_*` set with the rolls; `a0x`-`a9x` the weapon sets); one animation decoded to
//! full-skeleton frames.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use dsr_formats::hkx::skeleton::{Qs, Skeleton};
use dsr_formats::hkx::tagfile::Tagfile;
use dsr_formats::{bnd3, dcx, hkx, tae};

pub struct Player {
    pub skeleton: Skeleton,
    /// TAE stem (`a00`) → its animations.
    pub taes: BTreeMap<String, Vec<tae::Anim>>,
    /// Animation file stem (`a00_6000`) → its tagfile bytes.
    pub hkx: BTreeMap<String, Vec<u8>>,
    /// A creature's chr id (`c2060`): its animations are `aXX_YYYY.hkx` with the TAE id
    /// `XX × 10000 + YYYY`, all in the one TAE named after the chr. `None` for the player.
    pub chr: Option<String>,
}

pub(crate) fn archive(path: &Path) -> Result<Vec<bnd3::Entry>> {
    let raw = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    bnd3::read(&dcx::decompress(&raw)?).with_context(|| format!("unpacking {}", path.display()))
}

/// Every player animation archive, sorted: `c0000_a00_lo/md/hi` (the common `a00_*` set:
/// locomotion, rolls, backstep, damage, death) and `c0000_a0x` … `c0000_a9x` (weapon sets).
pub fn animation_archives(dsr: &Path) -> Result<Vec<std::path::PathBuf>> {
    let chr = dsr.join("chr");
    let mut out: Vec<_> = std::fs::read_dir(&chr)
        .with_context(|| format!("listing {}", chr.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("c0000_a") && n.ends_with(".anibnd.dcx"))
        })
        .collect();
    out.sort();
    if out.is_empty() {
        bail!("no c0000_a*.anibnd.dcx in {}", chr.display());
    }
    Ok(out)
}

pub fn load(dsr: &Path) -> Result<Player> {
    let chr = dsr.join("chr");
    let mut skeleton = None;
    let mut taes = BTreeMap::new();
    for e in archive(&chr.join("c0000.anibnd.dcx"))? {
        if e.file_name() == "Skeleton.hkx" {
            skeleton = Some(hkx::skeleton::read(&Tagfile::parse(e.data)?)?);
        } else if e.file_name().ends_with(".tae") {
            let anims = tae::read(&e.data).with_context(|| format!("TAE {}", e.name))?;
            taes.insert(e.stem().to_string(), anims);
        }
    }
    let mut hkx = BTreeMap::new();
    for path in animation_archives(dsr)? {
        for e in archive(&path)? {
            if e.file_name().ends_with(".hkx") {
                // The first archive's copy wins; a different copy elsewhere is ambiguous.
                match hkx.entry(e.stem().to_string()) {
                    Entry::Vacant(v) => {
                        v.insert(e.data);
                    }
                    Entry::Occupied(o) if o.get().len() != e.data.len() => bail!(
                        "{} in {} ({} bytes) differs from an earlier copy ({} bytes)",
                        e.name,
                        path.display(),
                        e.data.len(),
                        o.get().len()
                    ),
                    Entry::Occupied(_) => {}
                }
            }
        }
    }
    Ok(Player {
        skeleton: skeleton.context("c0000.anibnd has no Skeleton.hkx")?,
        taes,
        hkx,
        chr: None,
    })
}

/// A creature's data: `chr/<chr>.anibnd.dcx` holds its skeleton, its one TAE and its animation
/// files. File names compare without case (`skeleton.hkx` and `Skeleton.HKX` both occur).
pub fn load_chr(dsr: &Path, chr: &str) -> Result<Player> {
    let mut skeleton = None;
    let mut taes = BTreeMap::new();
    let mut hkx = BTreeMap::new();
    for e in archive(&dsr.join("chr").join(format!("{chr}.anibnd.dcx")))? {
        let name = e.file_name().to_ascii_lowercase();
        let stem = e.stem().to_ascii_lowercase();
        if name == "skeleton.hkx" {
            skeleton = Some(hkx::skeleton::read(&Tagfile::parse(e.data)?)?);
        } else if name.ends_with(".tae") {
            let anims = tae::read(&e.data).with_context(|| format!("TAE {}", e.name))?;
            taes.insert(chr.to_string(), anims);
        } else if name.ends_with(".hkx") {
            hkx.insert(stem, e.data);
        }
    }
    Ok(Player {
        skeleton: skeleton.with_context(|| format!("{chr}.anibnd has no skeleton"))?,
        taes,
        hkx,
        chr: Some(chr.to_string()),
    })
}

impl Player {
    fn animation(&self, stem: &str) -> Option<&tae::Anim> {
        let (set, id) = stem.split_once('_')?;
        let id: u32 = id.parse().ok()?;
        let (key, id) = match &self.chr {
            Some(chr) => (
                chr.as_str(),
                set.trim_start_matches('a').parse::<u32>().ok()? * 10000 + id,
            ),
            None => (set, id),
        };
        self.taes.get(key)?.iter().find(|a| a.id == id)
    }

    /// Resolve authored TAE imports, with a cycle guard. IDs below 10000 stay in this set.
    pub fn resolve(&self, stem: &str, events: bool) -> Result<String> {
        let mut name = stem.to_string();
        let mut seen = std::collections::BTreeSet::new();
        loop {
            if !seen.insert(name.clone()) {
                bail!("TAE import cycle: {name}");
            }
            let Some((id, full)) = self.animation(&name).and_then(|a| a.import) else {
                return Ok(name);
            };
            if events && !full {
                return Ok(name);
            }
            name = if id < 10000 {
                format!("{}_{id:04}", name.split_once('_').unwrap().0)
            } else {
                format!("a{:02}_{:04}", id / 10000, id % 10000)
            };
        }
    }

    /// The TAE events of animation file `a00_6000` (TAE `a00`, animation id 6000); for a creature,
    /// of file `aXX_YYYY` (the chr's TAE, animation id `XX × 10000 + YYYY`).
    pub fn events(&self, stem: &str) -> &[tae::Event] {
        let Ok(resolved) = self.resolve(stem, true) else {
            return &[];
        };
        let stem = resolved.as_str();
        let Some((set, idx)) = stem.split_once('_') else {
            return &[];
        };
        let Ok(idx) = idx.parse::<u32>() else {
            return &[];
        };
        let (key, id) = match &self.chr {
            Some(chr) => {
                let Ok(set) = set.trim_start_matches('a').parse::<u32>() else {
                    return &[];
                };
                (chr.as_str(), set * 10_000 + idx)
            }
            None => (set, idx),
        };
        let Some(anims) = self.taes.get(key) else {
            return &[];
        };
        anims
            .iter()
            .find(|a| a.id == id)
            .map_or(&[][..], |a| a.events.as_slice())
    }

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
        self.events(stem)
            .iter()
            .filter_map(|e| {
                Some(wscl::FlagWindow {
                    flag: e.action_flag()?,
                    start: e.start,
                    end: e.end,
                })
            })
            .collect()
    }
}

/// One DS animation over the whole skeleton (unanimated bones at the reference pose).
pub struct DsClip {
    pub name: String,
    pub fps: f32,
    /// `frames[f][bone]`, bone-local, DS space.
    pub frames: Vec<Vec<Qs>>,
    /// Per frame, accumulated `(x, y, z, yaw)`, DS space and units.
    pub root: Vec<[f32; 4]>,
    pub up: [f32; 3],
}

/// The root sample at `t` seconds, linear between samples.
fn root_at(rm: &hkx::animation::RootMotion, t: f32) -> [f32; 4] {
    let n = rm.samples.len();
    if n == 0 {
        return [0.0; 4];
    }
    if n == 1 || rm.duration <= 0.0 {
        return rm.samples[0];
    }
    let s = (t / rm.duration).clamp(0.0, 1.0) * (n - 1) as f32;
    let i = (s as usize).min(n - 2);
    let a = s - i as f32;
    let (p, q) = (rm.samples[i], rm.samples[i + 1]);
    [0, 1, 2, 3].map(|k| p[k] + (q[k] - p[k]) * a)
}

pub fn decode(player: &Player, stem: &str) -> Result<DsClip> {
    let resolved = player.resolve(stem, false)?;
    let bytes = player
        .hkx
        .get(&resolved)
        .with_context(|| format!("no animation file {stem}.hkx"))?;
    let file = hkx::animation::read(&Tagfile::parse(bytes.clone())?)?;
    let tracks = hkx::spline::decode(&file.anim)?;
    let bones = player.skeleton.parents.len();
    if file.anim.frame_duration <= 0.0 {
        bail!("{stem}: frame duration {}", file.anim.frame_duration);
    }
    let fps = 1.0 / file.anim.frame_duration;
    let frames = tracks
        .iter()
        .map(|tr| {
            let mut f = player.skeleton.reference.clone();
            for (t, &b) in file.track_to_bone.iter().enumerate() {
                if let (Ok(b), Some(q)) = (usize::try_from(b), tr.get(t)) {
                    if b < bones {
                        f[b] = *q;
                    }
                }
            }
            f
        })
        .collect::<Vec<_>>();
    let (root, up) = match &file.root {
        Some(rm) => (
            (0..frames.len())
                .map(|i| root_at(rm, i as f32 / fps))
                .collect(),
            rm.up,
        ),
        None => (vec![[0.0; 4]; frames.len()], [0.0, 1.0, 0.0]),
    };
    Ok(DsClip {
        name: stem.to_string(),
        fps,
        frames,
        root,
        up,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_player_loads_and_a_roll_decodes() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let player = load(Path::new(&dir)).unwrap();
        assert_eq!(player.skeleton.parents.len(), 61);
        assert!(player.hkx.contains_key("a00_0700"));
        assert!(
            player.hkx.len() > 2000,
            "{} animation files",
            player.hkx.len()
        );
        assert!(player.taes.contains_key("a00"));

        let clip = decode(&player, "a00_0700").unwrap();
        assert_eq!(clip.frames.len(), 51);
        assert!(clip.frames.iter().all(|f| f.len() == 61));
        assert!((clip.fps - 30.0).abs() < 0.5, "fps {}", clip.fps);
        let last = clip.root.last().unwrap();
        let want = [0.0, 0.0, -3.94, 0.0];
        for k in 0..4 {
            assert!(
                (last[k] - want[k]).abs() < 0.01,
                "root.last() = {last:?}, want {want:?}"
            );
        }
        println!(
            "{} hkx, {} taes, flags(a00_0700) = {:?}",
            player.hkx.len(),
            player.taes.len(),
            player.flags("a00_0700")
        );
    }

    #[test]
    fn install_weapon_imports_resolve_and_cycles_are_refused() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            return;
        };
        let mut p = load(Path::new(&dir)).unwrap();
        assert_eq!(p.resolve("a35_4002", false).unwrap(), "a35_4001");
        let a = decode(&p, "a35_4002").unwrap();
        let b = decode(&p, "a35_4001").unwrap();
        assert_eq!(a.frames.len(), b.frames.len());
        p.taes
            .get_mut("a35")
            .unwrap()
            .iter_mut()
            .find(|a| a.id == 4001)
            .unwrap()
            .import = Some((4002, false));
        assert!(p
            .resolve("a35_4002", false)
            .unwrap_err()
            .to_string()
            .contains("cycle"));
    }

    #[test]
    fn install_the_ghoul_loads_and_its_moves_read_as_measured() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let g = load_chr(Path::new(&dir), "c2060").unwrap();
        assert_eq!(g.skeleton.parents.len(), 53);
        assert!(g.skeleton.bone("L Thigh").is_some() && g.skeleton.bone("R_Weapon").is_some());
        // 135 animations; two of the files are spelled `.HKX` in the archive.
        assert_eq!(g.hkx.len(), 135);
        assert!(g.hkx.contains_key("a00_3301"));
        let h = g.hits("a00_3000");
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].judge, 100);
        assert!((h[0].start - 0.767).abs() < 0.002 && (h[0].end - 0.933).abs() < 0.002);
        // The spear kit lives at +10000: file a01_3000 is animation 13000.
        assert_eq!(g.hits("a01_3000")[0].judge, 180);
        let clip = decode(&g, "a00_3000").unwrap();
        assert_eq!(clip.frames.len(), 87);
        assert!((clip.fps - 30.0).abs() < 0.5);
        assert!((clip.root.last().unwrap()[2] + 1.59).abs() < 0.02);
    }
}
