//! `hkaSkeleton`: bone names, parents and the reference pose (SoulsAssetPipeline
//! `HKX.GenFakeFromTagFile.cs` field names).

use anyhow::{bail, Context, Result};

use super::tagfile::{Obj, Tagfile};

/// A Havok `hkQsTransform`: translation, rotation `(x, y, z, w)`, scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Qs {
    pub t: [f32; 3],
    pub r: [f32; 4],
    pub s: [f32; 3],
}

impl Qs {
    /// No translation, no rotation, unit scale.
    pub const IDENTITY: Qs = Qs {
        t: [0.0; 3],
        r: [0.0, 0.0, 0.0, 1.0],
        s: [1.0; 3],
    };
}

/// Bones in parent-sorted order: names, parent indices (`-1`: root) and the reference pose.
#[derive(Debug, Clone)]
pub struct Skeleton {
    pub names: Vec<String>,
    pub parents: Vec<i16>,
    pub reference: Vec<Qs>,
}

impl Skeleton {
    /// The index of the bone with this name.
    pub fn bone(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name)
    }
}

fn three(v: &[f32], what: &str) -> Result<[f32; 3]> {
    Ok([
        *v.first().with_context(|| format!("{what} is empty"))?,
        *v.get(1).with_context(|| format!("{what} has 1 value"))?,
        *v.get(2).with_context(|| format!("{what} has 2 values"))?,
    ])
}

pub(crate) fn qs(o: &Obj) -> Result<Qs> {
    let r = o.field("rotation")?.floats()?;
    if r.len() < 4 {
        bail!("rotation has {} values", r.len());
    }
    Ok(Qs {
        t: three(&o.field("translation")?.floats()?, "translation")?,
        r: [r[0], r[1], r[2], r[3]],
        s: three(&o.field("scale")?.floats()?, "scale")?,
    })
}

/// Reads the first skeleton of the file's animation container.
pub fn read(tf: &Tagfile) -> Result<Skeleton> {
    let c = super::animation_container(tf)?;
    let sk = c
        .field("skeletons")?
        .array()?
        .into_iter()
        .next()
        .context("no skeleton in the container")?
        .ptr()?
        .context("null skeleton")?;
    let parents: Vec<i16> = sk
        .field("parentIndices")?
        .ints()?
        .into_iter()
        .map(|v| v as i16)
        .collect();
    let names = sk
        .field("bones")?
        .array()?
        .iter()
        .map(|b| b.field("name")?.string())
        .collect::<Result<Vec<_>>>()?;
    let reference = sk
        .field("referencePose")?
        .array()?
        .iter()
        .map(qs)
        .collect::<Result<Vec<_>>>()?;
    if names.len() != parents.len() || reference.len() != parents.len() {
        bail!(
            "skeleton tables disagree: {} names, {} parents, {} reference transforms",
            names.len(),
            parents.len(),
            reference.len()
        );
    }
    Ok(Skeleton {
        names,
        parents,
        reference,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_player_skeleton_has_its_named_bones() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let bnd = std::fs::read(format!("{dir}/chr/c0000.anibnd.dcx")).unwrap();
        let entries = crate::bnd3::read(&crate::dcx::decompress(&bnd).unwrap()).unwrap();
        let e = entries
            .iter()
            .find(|e| e.file_name() == "Skeleton.hkx")
            .unwrap();
        let sk = read(&Tagfile::parse(e.data.clone()).unwrap()).unwrap();
        for name in [
            "Master",
            "Pelvis",
            "Spine",
            "Spine1",
            "Neck",
            "Head",
            "L_UpperArm",
            "R_Thigh",
            "L_Toe0",
        ] {
            assert!(sk.bone(name).is_some(), "missing {name}");
        }
        // Parent-sorted, as the retarget assumes.
        for (i, &p) in sk.parents.iter().enumerate() {
            assert!(p < 0 || (p as usize) < i, "bone {i} has parent {p}");
        }
        for q in &sk.reference {
            let n = q.r.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!(
                (n - 1.0).abs() < 1.0e-3,
                "reference rotation not unit: {q:?}"
            );
        }
    }
}
