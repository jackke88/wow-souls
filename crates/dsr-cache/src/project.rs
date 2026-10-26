//! Export the current cache as a complete, editable project library without install-file checks.
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{ensure, Context, Result};

fn relative(value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    ensure!(
        !path.as_os_str().is_empty()
            && !value.contains([':', '\\'])
            && path.components().all(|c| matches!(c, Component::Normal(_))),
        "not a relative project path: {value}"
    );
    Ok(path)
}

/// Machine paths become provenance comments; runtime versions and gameplay/event tables stay exact.
fn metadata(text: &str) -> String {
    let mut out = String::new();
    if text.starts_with("wscl-cache ") {
        out.push_str("# project-animations 1; source-derived DSR reference data\n");
    }
    for line in text.lines() {
        if let Some(source) = line.strip_prefix("source ") {
            let fields: Vec<_> = source.splitn(3, ' ').collect();
            if fields.len() == 3 {
                let normalized = fields[2].replace('\\', "/");
                let file = normalized.rsplit('/').next().unwrap_or(&normalized);
                out.push_str(&format!(
                    "# imported-source {} {} {file}\n",
                    fields[0], fields[1]
                ));
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn equivalent(a: &wscl::Clip, b: &wscl::Clip) -> Result<()> {
    ensure!(
        a.name == b.name
            && a.fps == b.fps
            && a.model_space == b.model_space
            && a.parents == b.parents
            && a.driven == b.driven
            && a.frames.len() == b.frames.len()
            && a.root_motion.len() == b.root_motion.len()
            && a.flags.len() == b.flags.len()
            && a.hits.len() == b.hits.len(),
        "clip shape/header changed"
    );
    let near = |a: f32, b: f32| (a - b).abs() < 2.0e-5;
    for (index, (af, bf)) in a.frames.iter().zip(&b.frames).enumerate() {
        ensure!(af.len() == bf.len(), "frame {index} bone count");
        for (bone, (a, b)) in af.iter().zip(bf).enumerate() {
            let norm = a.r.iter().map(|v| v * v).sum::<f32>().sqrt();
            let dot: f32 = a.r.iter().zip(b.r).map(|(x, y)| x * y).sum();
            let sign = if dot < 0.0 { -1.0 } else { 1.0 };
            ensure!(
                a.t.iter().zip(b.t).all(|(&x, y)| near(x, y))
                    && a.s.iter().zip(b.s).all(|(&x, y)| near(x, y))
                    && a.r.iter().zip(b.r).all(|(&x, y)| near(x / norm, sign * y)),
                "frame {index}, bone {bone}: transform changed"
            );
        }
    }
    ensure!(
        a.root_motion
            .iter()
            .zip(&b.root_motion)
            .all(|(a, b)| a.iter().zip(b).all(|(&x, &y)| near(x, y))),
        "root motion changed"
    );
    ensure!(
        a.flags
            .iter()
            .zip(&b.flags)
            .all(|(a, b)| a.flag == b.flag && near(a.start, b.start) && near(a.end, b.end)),
        "flag windows changed"
    );
    ensure!(
        a.hits
            .iter()
            .zip(&b.hits)
            .all(|(a, b)| a.judge == b.judge && near(a.start, b.start) && near(a.end, b.end)),
        "hit windows changed"
    );
    Ok(())
}

fn files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    let mut out = Vec::new();
    for entry in entries {
        if entry.is_dir() {
            out.extend(files(&entry)?);
        } else if entry.extension().is_some_and(|e| e == "wscl") {
            out.push(entry);
        }
    }
    Ok(out)
}

/// Existing text is intentionally preserved: it may contain the user's own edits.
pub fn export(from: &Path, out: &Path) -> Result<()> {
    ensure!(
        from.canonicalize()? != out.canonicalize().unwrap_or_default(),
        "source and output must differ"
    );
    let mut directories = BTreeMap::new();
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() && !name.starts_with("weapons-") && name != "hud" {
            directories.insert(entry.path(), PathBuf::from(name));
        }
    }
    let index = std::fs::read_to_string(from.join("weapons.txt"))?;
    ensure!(
        index.lines().next() == Some("wsweapons 1"),
        "unsupported weapons index"
    );
    let mut weapon_index = "wsweapons 1\n".to_string();
    for line in index.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let fields: Vec<_> = line.split_whitespace().collect();
        ensure!(
            fields.len() == 3 && fields[0] == "family",
            "bad weapon family: {line}"
        );
        let family = relative(fields[1])?;
        let path = relative(fields[2])?;
        let dest = PathBuf::from("weapons").join(family);
        directories.insert(from.join(path), dest);
        weapon_index.push_str(&format!("family {} weapons/{}\n", fields[1], fields[1]));
    }
    std::fs::create_dir_all(out)?;
    let mut inventory = "# source clip\tproject text\tframes\tbones\tfps\tstate\n".to_string();
    let (mut count, mut kept, mut bytes) = (0, 0, 0u64);
    for (input, target) in &directories {
        for path in files(input)? {
            let rel = path.strip_prefix(input)?;
            let dest_rel = if target == Path::new("human_male") {
                rel.with_extension("wsanim")
            } else {
                target.join(rel).with_extension("wsanim")
            };
            let dest = out.join(&dest_rel);
            let clip = wscl::Clip::read(&mut std::io::BufReader::new(std::fs::File::open(&path)?))
                .with_context(|| path.display().to_string())?;
            let text = wscl::text::write(&clip);
            let roundtrip = wscl::text::read(&text)
                .map_err(anyhow::Error::msg)
                .with_context(|| path.display().to_string())?;
            equivalent(&clip, &roundtrip).with_context(|| path.display().to_string())?;
            let state = if dest.exists() {
                let existing = std::fs::read_to_string(&dest)?;
                let existing_clip = wscl::text::read(&existing)
                    .map_err(anyhow::Error::msg)
                    .with_context(|| format!("preserved editable file {}", dest.display()))?;
                kept += 1;
                if equivalent(&clip, &existing_clip).is_ok() {
                    "preserved"
                } else {
                    "preserved-edited"
                }
            } else {
                std::fs::create_dir_all(dest.parent().unwrap())?;
                std::fs::write(&dest, text)?;
                "exported"
            };
            count += 1;
            bytes += dest.metadata()?.len();
            inventory.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{state}\n",
                path.strip_prefix(from)?
                    .to_string_lossy()
                    .replace('\\', "/"),
                dest_rel.to_string_lossy().replace('\\', "/"),
                clip.frames.len(),
                clip.bones(),
                clip.fps
            ));
        }
        let manifest = input.join("manifest.txt");
        if manifest.is_file() {
            let dest = out.join(target).join("manifest.txt");
            std::fs::create_dir_all(dest.parent().unwrap())?;
            let content = metadata(&std::fs::read_to_string(manifest)?);
            if dest.exists() {
                ensure!(
                    std::fs::read_to_string(&dest)? == content,
                    "existing manifest differs, preserving {}",
                    dest.display()
                );
            } else {
                std::fs::write(dest, content)?;
            }
        }
    }
    for name in [
        "manifest.txt",
        "locomotion.txt",
        "casts.txt",
        "interaction.txt",
    ] {
        let content = metadata(&std::fs::read_to_string(from.join(name))?);
        let dest = out.join(name);
        if dest.exists() {
            ensure!(
                std::fs::read_to_string(&dest)? == content,
                "existing table differs, preserving {}",
                dest.display()
            );
        } else {
            std::fs::write(dest, content)?;
        }
    }
    let dest = out.join("weapons.txt");
    if dest.exists() {
        ensure!(
            std::fs::read_to_string(&dest)? == weapon_index,
            "existing weapon index differs"
        );
    } else {
        std::fs::write(dest, weapon_index)?;
    }
    std::fs::write(out.join("inventory.tsv"), inventory)?;
    println!("Project animations: {count} validated clips, {kept} existing files preserved, {bytes} text bytes in {}",out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_metadata_keeps_events_but_never_checks_an_install() {
        let text = metadata("wscl-cache 6\nextractor 5\nsource 123 456 Z:/missing/chr/c0000.anibnd.dcx\nmove r1 a00_3000\n");
        assert!(text.contains("# imported-source 123 456 c0000.anibnd.dcx"));
        assert!(!text.lines().any(|l| l.starts_with("source ")));
        assert!(!text.contains("Z:/"));
        assert!(text.contains("move r1 a00_3000\n"));
        assert!(relative("../other").is_err() && relative("C:/other").is_err());
    }
}
