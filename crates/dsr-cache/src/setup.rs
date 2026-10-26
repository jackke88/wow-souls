//! `setup --dsr <DSR dir> --wow <WoW Data dir> --project <dir> --out <anim dir>`: builds the game's animation
//! library on the player's own machine, from their own install. `extract` and `bake-project` write the clips
//! under the game's original clip names into a scratch folder; `anim/provenance/clip-sources.tsv` (in
//! `--project`) names the project clip each one becomes and the SHA-256 of its motion payload, so every clip is
//! checked against the developer's copy before it is used. The tables (`anim/source/**/*.txt|tsv`) are copied from
//! `--project`. The result replaces `--out` only when every clip matched; a failed run leaves `--out` as it was.

use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use sha2::{Digest, Sha256};

/// Written last into `--out`: the game takes the folder as built only when this file is there.
pub const COMPLETE: &str = "built.txt";

struct Source {
    project: String,
    baked: String,
    header: u32,
    sha: String,
}

fn sources(path: &Path) -> Result<Vec<Source>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("{}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        ensure!(
            f.len() == 4,
            "{}:{}: expected 4 columns",
            path.display(),
            i + 1
        );
        out.push(Source {
            project: f[0].to_string(),
            baked: f[1].to_string(),
            header: f[2]
                .parse()
                .with_context(|| format!("{}:{}", path.display(), i + 1))?,
            sha: f[3].to_ascii_lowercase(),
        });
    }
    ensure!(!out.is_empty(), "{} lists no clips", path.display());
    Ok(out)
}

/// The motion payload as `tools/Test-MotionNames.ps1` hashes it: every line but `#` comments and `name`, with the
/// header's version set to `header`.
fn payload(text: &str, header: u32) -> String {
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .filter(|l| !l.starts_with('#') && !l.starts_with("name "))
        .map(str::to_string)
        .collect();
    if let Some(first) = lines.first_mut() {
        if first.starts_with("wsanim ") {
            *first = format!("wsanim {header}");
        }
    }
    lines.join("\n").trim_end_matches('\n').to_string()
}

fn sha_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Every `*.txt` / `*.tsv` under `dir`, relative.
fn tables(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for e in std::fs::read_dir(dir).with_context(|| format!("{}", dir.display()))? {
        let e = e?;
        let p = e.path();
        let r = rel.join(e.file_name());
        if p.is_dir() {
            tables(&p, &r, out)?;
        } else if matches!(
            p.extension().and_then(|x| x.to_str()),
            Some("txt") | Some("tsv")
        ) {
            out.push(r);
        }
    }
    Ok(())
}

pub fn run(dsr: &Path, wow: &Path, project: &Path, out: &Path) -> Result<()> {
    ensure!(
        dsr.join("chr").join("c0000.anibnd.dcx").is_file(),
        "{} is not a Dark Souls Remastered folder (no chr\\c0000.anibnd.dcx)",
        dsr.display()
    );
    let list = sources(&project.join("anim/provenance/clip-sources.tsv"))?;
    let tables_dir = project.join("anim/source");

    let work = out.with_extension("building");
    if work.exists() {
        std::fs::remove_dir_all(&work)?;
    }
    let (cache, baked, built) = (work.join("cache"), work.join("baked"), work.join("anim"));
    println!("1/3 reading the animations from {}", dsr.display());
    crate::cache::extract(dsr, wow, &cache, None)?;
    println!("2/3 writing them as text");
    crate::project::export(&cache, &baked)?;

    println!("3/3 checking {} clips", list.len());
    let mut wrong = Vec::new();
    for s in &list {
        let from = baked.join(&s.baked);
        let Ok(text) = std::fs::read_to_string(&from) else {
            wrong.push(format!("{} (no {})", s.project, s.baked));
            continue;
        };
        let body = payload(&text, s.header);
        if sha_hex(&body) != s.sha {
            wrong.push(format!(
                "{} (from {}: different motion)",
                s.project, s.baked
            ));
            continue;
        }
        let stem = Path::new(&s.project)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let original = Path::new(&s.baked)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let mut lines = body.lines();
        let header = lines.next().unwrap_or_default();
        let mut file =
            format!("# Built locally from the player's install; original clip {original}.\n{header}\nname {stem}\n");
        for l in lines {
            file.push_str(l);
            file.push('\n');
        }
        let to = built.join(&s.project);
        std::fs::create_dir_all(to.parent().unwrap_or(&built))?;
        std::fs::write(&to, file)?;
    }
    if !wrong.is_empty() {
        for w in wrong.iter().take(20) {
            eprintln!("  {w}");
        }
        bail!(
            "{} of {} clips did not match (another game version?); {} is unchanged",
            wrong.len(),
            list.len(),
            out.display()
        );
    }

    let mut rel = Vec::new();
    tables(&tables_dir, Path::new(""), &mut rel)?;
    ensure!(!rel.is_empty(), "no tables in {}", tables_dir.display());
    for r in &rel {
        let to = built.join(r);
        std::fs::create_dir_all(to.parent().unwrap_or(&built))?;
        std::fs::copy(tables_dir.join(r), &to)?;
    }
    std::fs::write(
        built.join(COMPLETE),
        format!("clips {}\ntables {}\n", list.len(), rel.len()),
    )?;

    if out.exists() {
        std::fs::remove_dir_all(out)?;
    }
    std::fs::rename(&built, out)?;
    let _ = std::fs::remove_dir_all(&work);
    println!(
        "done: {} clips and {} tables in {}",
        list.len(),
        rel.len(),
        out.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_drops_comments_and_name_and_sets_the_header() {
        let text = "# a comment\r\nwsanim 2\r\nname a00_0710\r\nfps 30\r\nframes 1\r\n";
        assert_eq!(payload(text, 1), "wsanim 1\nfps 30\nframes 1");
        assert_eq!(payload(text, 2), "wsanim 2\nfps 30\nframes 1");
    }

    #[test]
    fn sha_is_lowercase_hex_sha256() {
        assert_eq!(
            sha_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
