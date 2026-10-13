//! One animation file: the `hkaSplineCompressedAnimation`, its binding (track → bone) and its
//! root motion (`hkaDefaultAnimatedReferenceFrame`), by the field names of SoulsAssetPipeline's
//! `HKX.GenFakeFromTagFile.cs`.

use anyhow::{bail, Context, Result};

use super::tagfile::{Obj, Tagfile};

/// The spline-compressed animation's parameters and its raw block data.
#[derive(Debug, Clone)]
pub struct SplineAnim {
    pub duration: f32,
    pub tracks: usize,
    pub frames: usize,
    pub blocks: usize,
    pub frames_per_block: usize,
    pub block_duration: f32,
    pub frame_duration: f32,
    pub block_offsets: Vec<usize>,
    pub data: Vec<u8>,
}

/// Accumulated root motion samples `(x, y, z, yaw)`, evenly spaced over `duration`.
#[derive(Debug, Clone)]
pub struct RootMotion {
    pub up: [f32; 3],
    pub forward: [f32; 3],
    pub duration: f32,
    pub samples: Vec<[f32; 4]>,
}

/// An animation file's animation, root motion and binding.
#[derive(Debug, Clone)]
pub struct AnimFile {
    pub anim: SplineAnim,
    pub root: Option<RootMotion>,
    /// Per transform track, the skeleton bone it drives (`-1`: none).
    pub track_to_bone: Vec<i16>,
}

fn int(o: &Obj, name: &str) -> Result<usize> {
    usize::try_from(o.field(name)?.int()?).with_context(|| format!("{name} is negative"))
}

fn vec3(o: &Obj, name: &str) -> Result<[f32; 3]> {
    let v = o.field(name)?.floats()?;
    if v.len() < 3 {
        bail!("{name} has {} values", v.len());
    }
    Ok([v[0], v[1], v[2]])
}

fn root_motion(o: &Obj) -> Result<RootMotion> {
    let samples = o
        .field("referenceFrameSamples")?
        .array()?
        .iter()
        .map(|s| {
            let v = s.floats()?;
            if v.len() < 4 {
                bail!("root sample has {} values", v.len());
            }
            Ok([v[0], v[1], v[2], v[3]])
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(RootMotion {
        up: vec3(o, "up")?,
        forward: vec3(o, "forward")?,
        duration: o.field("duration")?.float()?,
        samples,
    })
}

/// Reads the file's first animation, which must be spline-compressed.
pub fn read(tf: &Tagfile) -> Result<AnimFile> {
    let c = super::animation_container(tf)?;
    let a = c
        .field("animations")?
        .array()?
        .into_iter()
        .next()
        .context("no animation in the container")?
        .ptr()?
        .context("null animation")?;
    if a.type_name() != "hkaSplineCompressedAnimation" {
        bail!("animation is a {}, not spline-compressed", a.type_name());
    }
    let anim = SplineAnim {
        duration: a.field("duration")?.float()?,
        tracks: int(&a, "numberOfTransformTracks")?,
        frames: int(&a, "numFrames")?,
        blocks: int(&a, "numBlocks")?,
        frames_per_block: int(&a, "maxFramesPerBlock")?,
        block_duration: a.field("blockDuration")?.float()?,
        frame_duration: a.field("frameDuration")?.float()?,
        block_offsets: a
            .field("blockOffsets")?
            .ints()?
            .into_iter()
            .map(|v| v as usize)
            .collect(),
        data: a.field("data")?.bytes()?.to_vec(),
    };
    let root = match a.field("extractedMotion")?.ptr()? {
        Some(rf) if rf.type_name() == "hkaDefaultAnimatedReferenceFrame" => Some(root_motion(&rf)?),
        _ => None,
    };
    let track_to_bone = match c.field("bindings")?.array()?.into_iter().next() {
        Some(b) => match b.ptr()? {
            Some(b) => {
                let map: Vec<i16> = b
                    .field("transformTrackToBoneIndices")?
                    .ints()?
                    .into_iter()
                    .map(|v| v as i16)
                    .collect();
                if map.len() != anim.tracks {
                    bail!(
                        "binding maps {} tracks, the animation has {}",
                        map.len(),
                        anim.tracks
                    );
                }
                map
            }
            None => (0..anim.tracks as i16).collect(),
        },
        None => (0..anim.tracks as i16).collect(),
    };
    Ok(AnimFile {
        anim,
        root,
        track_to_bone,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DSR_DIR`: every animation in every player archive (`chr/c0000_a*.anibnd.dcx`) reads and
    /// decodes; rotations are unit, and bone rotations jumping more than 60° between frames stay
    /// rare. The game data has such steps as stored control points (measured 2026-10-07: 4807 of
    /// 7 265 649 transitions, 0.066%; worst clip `a38_4800` 11 of 1037, 1.06%; mostly twist,
    /// hand and foot bones), so the bounds are 0.1% in total and 2% per clip; a misread block
    /// gives near-random rotations, of which about 94% jump more than 60°.
    #[test]
    fn install_player_animations_decode() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let mut archives: Vec<_> = std::fs::read_dir(format!("{dir}/chr"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("c0000_a") && n.ends_with(".anibnd.dcx"))
            .collect();
        archives.sort();
        let (mut total, mut all_transitions, mut all_jumps) = (0, 0usize, 0usize);
        for archive in &archives {
            let bnd = std::fs::read(format!("{dir}/chr/{archive}")).unwrap();
            let entries = crate::bnd3::read(&crate::dcx::decompress(&bnd).unwrap()).unwrap();
            let mut decoded = 0;
            for e in entries.iter().filter(|e| e.file_name().ends_with(".hkx")) {
                let tf = Tagfile::parse(e.data.clone())
                    .unwrap_or_else(|err| panic!("{archive} {}: {err:#}", e.name));
                let f = read(&tf).unwrap_or_else(|err| panic!("{archive} {}: {err:#}", e.name));
                let frames = super::super::spline::decode(&f.anim)
                    .unwrap_or_else(|err| panic!("{archive} {}: {err:#}", e.name));
                assert_eq!(frames.len(), f.anim.frames, "{}", e.name);
                let (mut transitions, mut jumps) = (0usize, 0usize);
                for (fi, frame) in frames.iter().enumerate() {
                    for (ti, q) in frame.iter().enumerate() {
                        let n = q.r.iter().map(|v| v * v).sum::<f32>().sqrt();
                        assert!((n - 1.0).abs() < 1.0e-3, "{} f{fi} t{ti}: {q:?}", e.name);
                        if fi > 0 {
                            let p = frames[fi - 1][ti].r;
                            let dot = p
                                .iter()
                                .zip(&q.r)
                                .map(|(a, b)| a * b)
                                .sum::<f32>()
                                .abs()
                                .min(1.0);
                            let deg = 2.0 * dot.acos().to_degrees();
                            transitions += 1;
                            if deg > 60.0 {
                                jumps += 1;
                            }
                        }
                    }
                }
                assert!(
                    jumps * 50 <= transitions,
                    "{} has {jumps} of {transitions} bone transitions above 60°",
                    e.name
                );
                all_transitions += transitions;
                all_jumps += jumps;
                if ["a00_0700", "a00_0710", "a00_0720"].contains(&e.stem()) {
                    let root = f.root.as_ref();
                    println!(
                        "{archive} {}: numFrames {} frameDuration {} duration {} root samples {} last {:?}",
                        e.stem(),
                        f.anim.frames,
                        f.anim.frame_duration,
                        f.anim.duration,
                        root.map_or(0, |r| r.samples.len()),
                        root.and_then(|r| r.samples.last()),
                    );
                }
                decoded += 1;
            }
            println!("{archive}: {decoded} animations decoded");
            total += decoded;
        }
        println!("{total} animations; {all_jumps} of {all_transitions} bone transitions above 60°");
        assert!(total > 1000, "only {total} animations");
        assert!(
            all_jumps * 1000 <= all_transitions,
            "{all_jumps} of {all_transitions} bone transitions above 60°"
        );
    }
}
