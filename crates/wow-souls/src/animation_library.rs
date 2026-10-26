//! Editable clips: player clips as text (`wscl::text`, `.wsanim`), read from `anim/source` (or
//! `WOWSOULS_ANIM_DIR`). A file there is the Dark Souls clip
//! baked out of the cache (`wow-souls-dsr bake`), turn by turn in degrees, ready to be changed by hand; the game
//! plays it as written and needs no DSR install for it. These are regular project files, including the
//! namespaced creature/reference clips and tables. Source-derived provenance remains recorded;
//! independent original replacements are a later step (`docs/SOURCE-TO-ORIGINAL.md`).
use std::path::{Path, PathBuf};

/// Where the editable clips are: `WOWSOULS_ANIM_DIR`; else the project's `anim/source` when it holds
/// clips (a developer's checkout); else the library built on this machine from the player's own install
/// ([`crate::install::local_anim_dir`]). A public checkout has only the tables in `anim/source`, so
/// without a local build the loaders find no manifest and keep WoW's own motion.
pub fn anim_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("WOWSOULS_ANIM_DIR") {
        return PathBuf::from(dir);
    }
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let project = wow_world::project_path("anim/source");
        if has_clips(&project) {
            project
        } else {
            crate::install::local_anim_dir()
        }
    })
    .clone()
}

/// Whether `dir` holds any `.wsanim` of its own (not just the tables).
pub fn has_clips(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|e| e.path().extension().is_some_and(|x| x == "wsanim"))
    })
}

pub fn read(path: &Path) -> Result<wscl::Clip, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    wscl::text::read(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The editable player clip `name`; an invalid/unreadable file is an error, never a hidden fallback.
pub fn load(name: &str) -> Option<Result<wscl::Clip, String>> {
    #[cfg(test)]
    if OFF.with(std::cell::Cell::get) {
        return None;
    }
    let path = anim_dir().join(format!("{name}.wsanim"));
    match std::fs::metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        _ => Some(read(&path).and_then(|clip| {
            if clip.name == name {
                Ok(clip)
            } else {
                Err(format!(
                    "{}: expected clip {name}, found {}",
                    path.display(),
                    clip.name
                ))
            }
        })),
    }
}

#[cfg(test)]
thread_local! {
    static OFF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// No editable clip for the rest of this test (its thread): a made-up cache whose clips carry real names must not
/// pick up the baked files of `WOWSOULS_ANIM_DIR`.
#[cfg(test)]
pub fn off_in_this_test() {
    OFF.with(|o| o.set(true));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Err` names the first place the baked clip `a` differs from the cache's `b`: a turn (quaternion dot below
    /// 1-1e-5), a translation or root step (1e-4 yd), the rate, or a flag or hit window (1e-5 s).
    fn same(a: &wscl::Clip, b: &wscl::Clip) -> Result<(), String> {
        let near = |x: f32, y: f32, tol: f32| (x - y).abs() < tol;
        if a.frames.len() != b.frames.len() || a.driven != b.driven || !near(a.fps, b.fps, 1.0e-4) {
            return Err("frames, driven bones or rate".into());
        }
        for (i, (fa, fb)) in a.frames.iter().zip(&b.frames).enumerate() {
            for (bone, (x, y)) in fa.iter().zip(fb).enumerate() {
                let dot: f32 = x.r.iter().zip(&y.r).map(|(p, q)| p * q).sum();
                let gap: f32 = x.t.iter().zip(&y.t).map(|(p, q)| (p - q).abs()).sum();
                if dot.abs() <= 1.0 - 1.0e-5 || gap >= 1.0e-4 {
                    return Err(format!("frame {i} bone {bone}: {x:?} vs {y:?}"));
                }
            }
        }
        for (i, (x, y)) in a.root_motion.iter().zip(&b.root_motion).enumerate() {
            if x.iter().zip(y).any(|(p, q)| !near(*p, *q, 1.0e-4)) {
                return Err(format!("root at frame {i}: {x:?} vs {y:?}"));
            }
        }
        let flags = a.flags.len() == b.flags.len()
            && a.flags.iter().zip(&b.flags).all(|(x, y)| {
                x.flag == y.flag && near(x.start, y.start, 1.0e-5) && near(x.end, y.end, 1.0e-5)
            });
        let hits = a.hits.len() == b.hits.len()
            && a.hits.iter().zip(&b.hits).all(|(x, y)| {
                x.judge == y.judge && near(x.start, y.start, 1.0e-5) && near(x.end, y.end, 1.0e-5)
            });
        if !(flags && hits) {
            return Err(format!(
                "windows: {:?} {:?} vs {:?} {:?}",
                a.flags, a.hits, b.flags, b.hits
            ));
        }
        Ok(())
    }

    #[test]
    fn an_installed_baked_clip_is_the_cache_clip_turn_for_turn() {
        // Only where both the cache and a baked file exist.
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source");
        let clip = crate::motion_clip::read_clip(&cache, "human_male", "body_sprint").unwrap();
        let baked = load("body_sprint").expect("project sprint").unwrap();
        same(&baked, &clip).unwrap();
    }

    #[test]
    fn an_installed_baked_attack_plays_as_the_cache_clip_with_its_windows() {
        // The straight sword's first light attack, where both the cache and a baked file exist.
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source");
        let clip =
            crate::motion_clip::read_clip(&cache, "human_male", "straight_sword_light_1").unwrap();
        let baked = load("straight_sword_light_1").expect("project attack");
        let baked = baked.unwrap();
        assert!(!clip.hits.is_empty() && !clip.flags.is_empty());
        same(&baked, &clip).unwrap();
        // The move loader plays the editable file, dressed as the cache's would be.
        let played = crate::motion_clip::player_clip(&cache, "straight_sword_light_1").unwrap();
        same(&played, &clip).unwrap();
        // One bone turned 20° is caught.
        let mut edited = baked;
        let turn = bevy::math::Quat::from_rotation_x(20f32.to_radians());
        let mid = edited.frames.len() / 2;
        let r = &mut edited.frames[mid][18].r;
        *r = (bevy::math::Quat::from_array(*r) * turn).to_array();
        assert!(same(&edited, &clip).is_err());
    }
}

#[cfg(test)]
#[path = "animation_library/project_tests.rs"]
mod project_tests;
