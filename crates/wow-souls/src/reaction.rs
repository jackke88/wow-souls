//! The player's Dark Souls reactions from `cache/dsr/` (written by `wow-souls-dsr extract`): the
//! flinch clips (small, medium, large, four directions each), the knockdowns (thrown straight back
//! or forward, invulnerable while down, a get-up at the end) and the death (a knockdown held on the
//! frame where it lies), all retargeted onto the human, with the times the clip's own TAE flags let
//! the player attack, evade and move again. Loaded at startup with the player's other clips;
//! without a cache, or with a stale one, the player staggers and dies as before.
//!
//! Which blow level answers which class is [`LEVEL_CLASSES`], a tuning table (the params have no
//! such mapping); which clip of the class is chosen from the way the blow pushes the body: a flinch
//! by the measured push of its head, a knockdown by the way it throws the body.

use std::path::Path;

use bevy::prelude::*;

use crate::combat::ReelSpec;
use crate::dsclip::read_clip;
use crate::monster::best_by_push;

/// The flinch (or knockdown) class a blow of up to this level gets, in order: level 1 is small,
/// 2 medium, 3 and 4 large, 5 and up the short knockdown (the kobold's body check is level 5).
pub const LEVEL_CLASSES: [(u8, &str); 4] = [(1, "s"), (2, "m"), (4, "l"), (u8::MAX, "down2")];

/// The class a blow of `level` gets.
pub fn class_for(level: u8) -> &'static str {
    LEVEL_CLASSES
        .iter()
        .find(|(max, _)| level <= *max)
        .map_or("down2", |(_, class)| class)
}

/// The player's poise (a naked body: none, so every blow flinches, as in Dark Souls);
/// `WOWSOULS_POISE=<n>` tries another.
pub fn player_poise() -> f32 {
    std::env::var("WOWSOULS_POISE")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|p| p.is_finite() && *p >= 0.0)
        .unwrap_or(0.0)
}

/// A reaction clip travels the body when its root motion goes this far (yd): the knockdowns do.
pub const TRAVELS_OVER: f32 = 0.3;

/// One reaction clip.
pub struct ReactClip {
    pub class: String,
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    /// `(forward, left)` of the head's push in the body's frame (negative forward: head back).
    pub push: Vec2,
    /// `(forward, left)` the clip's root motion throws the body (yd, the body's frame).
    pub travel: Vec2,
    pub spec: ReelSpec,
}

impl ReactClip {
    /// The clip throws the body across the ground, so the game's own shove stays out of it.
    pub fn travels(&self) -> bool {
        self.travel.length() > TRAVELS_OVER
    }

    /// The way this clip sends the body: its travel when it has one, else its head's push.
    fn aim(&self) -> Vec2 {
        if self.travels() {
            self.travel
        } else {
            self.push
        }
    }
}

/// A death: a reaction clip (by index) held from `hold` seconds on, the frame where it lies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeathRef {
    pub react: usize,
    pub hold: f32,
}

/// The loaded kit.
#[derive(Resource)]
pub struct PlayerReacts {
    pub reacts: Vec<ReactClip>,
    pub deaths: Vec<DeathRef>,
}

impl PlayerReacts {
    /// The reaction for a blow of `level` that pushes the body along `push` (`(forward, left)` in
    /// the body's frame): among its class's clips, the one that sends the body most nearly the same
    /// way. `None` when the class has no clip.
    pub fn choose(&self, level: u8, push: Vec2) -> Option<usize> {
        let class = class_for(level);
        best_by_push(
            self.reacts
                .iter()
                .enumerate()
                .filter(|(_, r)| r.class == class)
                .map(|(i, r)| (i, r.aim())),
            push,
        )
        .map(usize::from)
    }

    /// The death for a killing blow that pushes the body along `push`.
    pub fn choose_death(&self, push: Vec2) -> Option<usize> {
        best_by_push(
            self.deaths
                .iter()
                .enumerate()
                .map(|(i, d)| (i, self.reacts[d.react].aim())),
            push,
        )
        .map(usize::from)
    }

    /// The clip pair a reaction or a death plays, and the clip time a death holds from.
    pub fn death_clip(&self, death: usize) -> Option<(&ReactClip, f32)> {
        let d = self.deaths.get(death)?;
        self.reacts.get(d.react).map(|r| (r, d.hold))
    }
}

fn number(s: &str, line: &str) -> Result<f32, String> {
    s.parse::<f32>()
        .ok()
        .filter(|f| f.is_finite())
        .ok_or_else(|| format!("bad reaction line {line:?}"))
}

/// Reads the kit from the manifest `text` and the clips under `dir`; `Err` says why it is unusable
/// (a line malformed, a clip missing, bone counts that disagree, no reaction or death at all).
pub fn parse(text: &str, dir: &Path) -> Result<PlayerReacts, String> {
    let mut reacts: Vec<ReactClip> = Vec::new();
    let mut death_lines: Vec<(String, f32)> = Vec::new();
    let mut bones = None;
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let bad = || format!("bad reaction line {line:?}");
        match w.first().copied() {
            Some("preact") => {
                let [_, class, clip, "push", fwd, left, "chain", chain, "evade", evade, "move", mv, "down", d0, d1] =
                    w[..]
                else {
                    return Err(bad());
                };
                let wow = read_clip(dir, "human_male", clip)?;
                let ds = read_clip(dir, "c0000", clip)?;
                match bones {
                    None => bones = Some(wow.bones()),
                    Some(n) if n == wow.bones() => {}
                    Some(n) => {
                        return Err(format!(
                            "{clip}: {} bones, the kit's other clips have {n}",
                            wow.bones()
                        ))
                    }
                }
                let (d0, d1) = (number(d0, line)?, number(d1, line)?);
                let rm = wow.root_motion.last().copied().unwrap_or([0.0; 4]);
                reacts.push(ReactClip {
                    class: class.to_string(),
                    push: Vec2::new(number(fwd, line)?, number(left, line)?),
                    // The retargeted root motion is in Bevy axes: forward -Z, left -X.
                    travel: Vec2::new(-rm[2], -rm[0]),
                    spec: ReelSpec {
                        duration: wow.duration(),
                        chain_from: number(chain, line)?,
                        evade_from: number(evade, line)?,
                        move_from: number(mv, line)?,
                        invuln: (d1 > d0).then_some((d0, d1)),
                    },
                    wow,
                    ds,
                });
            }
            Some("pdeath") => {
                let [_, clip, "push", _, _, "hold", hold] = w[..] else {
                    return Err(bad());
                };
                death_lines.push((clip.to_string(), number(hold, line)?));
            }
            _ => {}
        }
    }
    if reacts.is_empty() {
        return Err("manifest has no preact lines".into());
    }
    let mut deaths = Vec::new();
    for (clip, hold) in death_lines {
        let react = reacts
            .iter()
            .position(|r| r.wow.name == clip)
            .ok_or_else(|| format!("pdeath {clip} names no preact clip"))?;
        deaths.push(DeathRef { react, hold });
    }
    if deaths.is_empty() {
        return Err("manifest has no pdeath lines".into());
    }
    Ok(PlayerReacts { reacts, deaths })
}

/// Reads the kit from `dir`: a checked manifest, then [`parse`].
pub fn load(dir: &Path) -> Result<PlayerReacts, String> {
    let text = crate::dsclip::checked_manifest(dir)?;
    parse(&text, dir)
}

fn startup(mut commands: Commands) {
    match load(&crate::dsclip::cache_dir()) {
        Ok(k) => {
            for r in &k.reacts {
                let s = &r.spec;
                info!(
                    "reaction: {} {} ({:.3} s, push ({:+.2}, {:+.2}), control {:.2}/{:.2}/{:.2}, travel ({:+.2}, {:+.2}), invulnerable {:?})",
                    r.wow.name,
                    r.class,
                    s.duration,
                    r.push.x,
                    r.push.y,
                    s.chain_from,
                    s.evade_from,
                    s.move_from,
                    r.travel.x,
                    r.travel.y,
                    s.invuln
                );
            }
            info!(
                "reaction: {} clips, {} deaths ({} s hold)",
                k.reacts.len(),
                k.deaths.len(),
                k.deaths
                    .iter()
                    .map(|d| format!("{:.2}", d.hold))
                    .collect::<Vec<_>>()
                    .join("/")
            );
            commands.insert_resource(k);
        }
        Err(why) => warn!(
            "reaction: no Dark Souls reactions ({why}); the player staggers and dies as before"
        ),
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, startup);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(name: &str, root: [f32; 4]) -> wscl::Clip {
        wscl::Clip {
            name: name.into(),
            fps: 30.0,
            model_space: false,
            parents: vec![-1, 0],
            driven: vec![true; 2],
            frames: vec![vec![wscl::Trs::IDENTITY; 2]; 31],
            root_motion: (0..31)
                .map(|i| {
                    let f = i as f32 / 30.0;
                    [root[0] * f, 0.0, root[2] * f, 0.0]
                })
                .collect(),
            flags: Vec::new(),
            hits: Vec::new(),
        }
    }

    fn put(dir: &Path, sub: &str, c: &wscl::Clip) {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
        c.write(
            &mut std::fs::File::create(dir.join(sub).join(format!("{}.wscl", c.name))).unwrap(),
        )
        .unwrap();
    }

    const MANIFEST: &str = "\
preact s a00_2000 push 0.187 -0.144 chain 0.600 evade 0.600 move 0.767 down 0.000 0.000
preact s a00_2002 push -0.431 0.028 chain 0.600 evade 0.600 move 0.767 down 0.000 0.000
preact m a00_2004 push 0.215 0.423 chain 1.033 evade 1.067 move 1.067 down 0.000 0.000
preact m a00_2007 push -0.390 0.059 chain 1.033 evade 1.067 move 1.067 down 0.000 0.000
preact down2 a00_2026 push -0.776 0.011 chain 0.8 evade 0.7 move 0.9 down 0.000 0.8
preact down2 a00_2027 push 0.467 0.186 chain 0.8 evade 0.7 move 0.9 down 0.000 0.8
pdeath a00_2026 push -0.776 0.011 hold 1.133
pdeath a00_2027 push 0.467 0.186 hold 1.400
";

    fn kit_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wow-souls-reaction-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (c, root) in [
            ("a00_2000", [0.0; 4]),
            ("a00_2002", [0.0; 4]),
            ("a00_2004", [0.0; 4]),
            ("a00_2007", [0.0; 4]),
            // Thrown straight back (+Z) and straight forward (-Z), 3 yd.
            ("a00_2026", [0.0, 0.0, 3.0, 0.0]),
            ("a00_2027", [0.0, 0.0, -3.0, 0.0]),
        ] {
            let k = clip(c, root);
            put(&dir, "human_male", &k);
            put(&dir, "c0000", &k);
        }
        dir
    }

    #[test]
    fn a_whole_kit_loads_with_its_timing() {
        let dir = kit_dir("whole");
        let k = parse(MANIFEST, &dir).unwrap_or_else(|e| panic!("{e}"));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!((k.reacts.len(), k.deaths.len()), (6, 2));
        let s = &k.reacts[0];
        assert_eq!(s.class, "s");
        assert_eq!(s.push, Vec2::new(0.187, -0.144));
        assert!((s.spec.duration - 1.0).abs() < 1.0e-6, "the clip's length");
        assert_eq!(
            (s.spec.chain_from, s.spec.evade_from, s.spec.move_from),
            (0.6, 0.6, 0.767)
        );
        assert_eq!(s.spec.invuln, None, "no window");
        assert!(!s.travels());
        let down = &k.reacts[4];
        assert_eq!(down.spec.invuln, Some((0.0, 0.8)));
        assert!(down.travels());
        // Thrown back (+Z) is forward -3 in the body's frame.
        assert_eq!(down.travel, Vec2::new(-3.0, 0.0));
        assert_eq!(
            k.deaths[0],
            DeathRef {
                react: 4,
                hold: 1.133
            }
        );
    }

    #[test]
    fn an_incomplete_or_malformed_kit_is_refused() {
        let dir = kit_dir("bad");
        let without = |prefix: &str| {
            MANIFEST
                .lines()
                .filter(|l| !l.starts_with(prefix))
                .collect::<Vec<_>>()
                .join("\n")
        };
        for (why, text) in [
            ("no preact lines", without("preact")),
            ("no pdeath lines", without("pdeath")),
            (
                "bad reaction line",
                MANIFEST.replace("chain 0.600", "chain x"),
            ),
            (
                "names no preact clip",
                MANIFEST.replace("pdeath a00_2026", "pdeath a00_9999"),
            ),
            ("a00_2999.wscl", MANIFEST.replace("a00_2000", "a00_2999")),
        ] {
            let err = parse(&text, &dir)
                .err()
                .unwrap_or_else(|| panic!("{why} loads"));
            assert!(err.contains(why), "{why}: {err}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_class_follows_the_blows_level() {
        for (level, class) in [
            (0, "s"),
            (1, "s"),
            (2, "m"),
            (3, "l"),
            (4, "l"),
            (5, "down2"),
            (9, "down2"),
        ] {
            assert_eq!(class_for(level), class, "level {level}");
        }
    }

    #[test]
    fn a_blow_from_the_front_takes_the_clip_that_throws_the_head_back() {
        let dir = kit_dir("choose");
        let k = parse(MANIFEST, &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        // Level 1 (small): the front blow pushes the head back, index 1 (a00_2002); behind, index 0.
        assert_eq!(k.choose(1, Vec2::new(-1.0, 0.0)), Some(1));
        assert_eq!(k.choose(1, Vec2::new(1.0, 0.0)), Some(0));
        // Level 2 (medium): from the side the head goes left, a00_2004 (+0.42 left).
        assert_eq!(k.choose(2, Vec2::new(0.1, 1.0)), Some(2));
        assert_eq!(k.choose(2, Vec2::new(-1.0, 0.0)), Some(3));
        // Level 5: the knockdown that throws the body the way the blow does.
        assert_eq!(k.choose(5, Vec2::new(-1.0, 0.0)), Some(4));
        assert_eq!(k.choose(5, Vec2::new(1.0, 0.0)), Some(5));
        // A class with no clip (large, here) chooses nothing.
        assert_eq!(k.choose(3, Vec2::new(-1.0, 0.0)), None);
        // Deaths go the same way.
        assert_eq!(k.choose_death(Vec2::new(-1.0, 0.0)), Some(0));
        assert_eq!(k.choose_death(Vec2::new(1.0, 0.0)), Some(1));
        let (clip, hold) = k.death_clip(1).unwrap();
        assert_eq!((clip.wow.name.as_str(), hold), ("a00_2027", 1.4));
    }
}
