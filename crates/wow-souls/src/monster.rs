//! The kobold's Dark Souls kit from `cache/dsr/` (written by `wow-souls-dsr extract`): the
//! ghoul's six melee moves, its flinch clips and its deaths, retargeted onto the kobold rig, with
//! each move's hit window, damage and poise from the install's data. Loaded at startup with the
//! player's clips; without a cache, or with a stale one, the old kobold fights.
//!
//! While a creature plays a kit clip ([`MonsterPlay`]) the clip owns the rig's driven bones,
//! blended over the rig's own pose at its ends (the player's blend code, [`crate::dsclip`]); a
//! death holds its last pose.

use std::collections::HashMap;
use std::path::Path;

use benilla_world::rig_anim::{PosePost, RigPose};
use bevy::prelude::*;

use crate::dsclip::{layers, sample, Fade};

/// What one kobold move is, from its clip's TAE window and the install's params.
#[derive(Debug, Clone, PartialEq)]
pub struct AttackSpec {
    pub name: String,
    /// The clip's length (s).
    pub length: f32,
    /// The first hit window (s into the clip), and every window of the move (a double or a triple
    /// has several; each strikes once).
    pub hit: (f32, f32),
    pub hits: Vec<(f32, f32)>,
    /// The damage to the player: `atkPhys × DAMAGE_K`, rounded.
    pub damage: f32,
    /// Poise damage and flinch level of the blow.
    pub poise: f32,
    pub level: u8,
    /// The hit volume's radius (DS metres; the game scales it) and the second hand's.
    pub radius: f32,
    pub radius2: f32,
    /// The blow's hit-stop on the player (s) and push (DS metres): `hitStopTime`, `knockbackDist`.
    pub stop: f32,
    pub knock: f32,
}

/// `damage = atkPhys × DAMAGE_K`: small hit 1 (`atkPhys` 138) stays today's 25.
pub const DAMAGE_K: f32 = 25.0 / 138.0;

/// One move: its clips and its numbers.
pub struct AttackClip {
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    pub spec: AttackSpec,
}

/// One flinch clip, the level it answers and where it pushes the head.
pub struct ReactClip {
    pub level: u8,
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    /// `(forward, left)` of the head's push in the creature's frame (negative forward: head back).
    pub push: Vec2,
}

/// One death clip and the way it throws the head.
pub struct DeathClip {
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    pub push: Vec2,
}

/// Where the kobold's blow comes from: its right-hand bone and its forearm bone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reach {
    pub hand: u16,
    pub forearm: u16,
}

/// The distance bands of the ghoul's `NpcThinkParam` (DS metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Think {
    pub near: f32,
    pub mid: f32,
    pub far: f32,
    pub out: f32,
}

/// One creature's loaded kit.
pub struct MonsterClips {
    /// The kit's name in the manifest (`kobold`), also the folder of its clips.
    pub name: String,
    pub attacks: Vec<AttackClip>,
    pub reacts: Vec<ReactClip>,
    pub deaths: Vec<DeathClip>,
    /// The leg ratio (kobold model units per DS metre) the extractor scaled the clips by.
    pub scale: f32,
    /// `NpcParam.superArmorDurability`.
    pub poise_max: f32,
    pub reach: Reach,
    pub think: Think,
    /// `NpcParam.getSoul`: the souls a kill gives.
    pub souls: u32,
    pub parried: Option<DeathClip>,
}

/// Every loaded kit, by the creature's name: the game's creatures that fight with Dark Souls moves.
#[derive(Resource, Default)]
pub struct Roster(HashMap<String, MonsterClips>);

impl Roster {
    /// The kit of the creature called `name`.
    pub fn get(&self, name: &str) -> Option<&MonsterClips> {
        self.0.get(name)
    }
}

/// Which clip of the kit is playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterClip {
    Attack(u8),
    React(u8),
    Death(u8),
    Parried,
}

impl MonsterClips {
    /// The index of the move called `name`.
    pub fn attack_index(&self, name: &str) -> Option<usize> {
        self.attacks.iter().position(|a| a.spec.name == name)
    }

    /// The clip pair `id` plays.
    pub fn clips(&self, id: MonsterClip) -> Option<(&wscl::Clip, &wscl::Clip)> {
        match id {
            MonsterClip::Attack(i) => self.attacks.get(usize::from(i)).map(|a| (&a.wow, &a.ds)),
            MonsterClip::React(i) => self.reacts.get(usize::from(i)).map(|r| (&r.wow, &r.ds)),
            MonsterClip::Death(i) => self.deaths.get(usize::from(i)).map(|d| (&d.wow, &d.ds)),
            MonsterClip::Parried => self.parried.as_ref().map(|d| (&d.wow, &d.ds)),
        }
    }
}

/// The candidate whose push points most nearly along `want` (`(forward, left)`); a blow with no
/// direction takes the first.
pub(crate) fn best_by_push(
    candidates: impl Iterator<Item = (usize, Vec2)>,
    want: Vec2,
) -> Option<u8> {
    let mut candidates = candidates.peekable();
    if want.length_squared() < 1.0e-6 {
        return candidates.next().map(|(i, _)| i as u8);
    }
    let cos = |p: Vec2| p.normalize_or_zero().dot(want.normalize());
    candidates
        .max_by(|a, b| cos(a.1).total_cmp(&cos(b.1)))
        .map(|(i, _)| i as u8)
}

/// The flinch clip for a blow of `level` that pushes the creature along `push` (`(forward, left)`
/// in its frame): among the level's clips, the one whose measured push points most nearly the
/// same way. Levels above 2 take level 2's.
pub fn choose_react(kit: &MonsterClips, level: u8, push: Vec2) -> Option<u8> {
    best_by_push(
        kit.reacts
            .iter()
            .enumerate()
            .filter(|(_, r)| r.level == level.clamp(1, 2))
            .map(|(i, r)| (i, r.push)),
        push,
    )
}

/// The death clip for a killing blow that pushes the creature along `push`.
pub fn choose_death(kit: &MonsterClips, push: Vec2) -> Option<u8> {
    best_by_push(
        kit.deaths.iter().enumerate().map(|(i, d)| (i, d.push)),
        push,
    )
}

fn pair(w: &[&str], line: &str) -> Result<Vec2, String> {
    let bad = || format!("bad monster line {line:?}");
    let [a, b] = w else { return Err(bad()) };
    let f = |s: &str| s.parse::<f32>().ok().filter(|f| f.is_finite());
    Ok(Vec2::new(f(a).ok_or_else(bad)?, f(b).ok_or_else(bad)?))
}

/// Reads one creature's kit from its block of the manifest `text` (from its `monster` line on)
/// and the clips under `dir`; `Err` says why the kit is unusable (a line missing or malformed, a
/// clip missing, bone counts that disagree).
pub fn parse(text: &str, dir: &Path) -> Result<MonsterClips, String> {
    let read = |sub: &str, name: &str| crate::dsclip::read_clip(dir, sub, name);
    let (mut scale, mut poise_max, mut chr, mut kit_name) = (None, None, None, None);
    let (mut attacks, mut reacts, mut deaths) = (Vec::new(), Vec::new(), Vec::new());
    let (mut reach, mut think) = (None, None);
    let mut souls = 0u32;
    let mut parried_name = None;
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let bad = || format!("bad monster line {line:?}");
        match w.first().copied() {
            Some("parried") => {
                let [_, name] = w[..] else { return Err(bad()) };
                parried_name = Some(name.to_string());
            }
            Some("monster") => {
                let [_, n, "chr", c, "npc", _, "hp", _, "poise", p, "scale", s] = w[..] else {
                    return Err(bad());
                };
                kit_name = Some(n.to_string());
                chr = Some(c.to_string());
                poise_max = Some(p.parse::<f32>().map_err(|_| bad())?);
                scale = Some(s.parse::<f32>().map_err(|_| bad())?);
            }
            Some("souls") => {
                let [_, _, n] = w[..] else {
                    return Err(bad());
                };
                souls = n.parse::<u32>().map_err(|_| bad())?;
            }
            Some("attack") => {
                let [_, name, clip, "judge", _, "phys", phys, "poise", poise, "level", level, "radius", r1, "radius2", r2, "stop", stop, "knock", knock] =
                    w[..]
                else {
                    return Err(bad());
                };
                attacks.push((
                    name.to_string(),
                    clip.to_string(),
                    phys.parse::<f32>().map_err(|_| bad())?,
                    poise.parse::<f32>().map_err(|_| bad())?,
                    level.parse::<u8>().map_err(|_| bad())?,
                    r1.parse::<f32>().map_err(|_| bad())?,
                    r2.parse::<f32>().map_err(|_| bad())?,
                    stop.parse::<f32>().map_err(|_| bad())?,
                    knock.parse::<f32>().map_err(|_| bad())?,
                ));
            }
            Some("react") => {
                let [_, level, clip, "push", fwd, left] = w[..] else {
                    return Err(bad());
                };
                reacts.push((
                    level.parse::<u8>().map_err(|_| bad())?,
                    clip.to_string(),
                    pair(&[fwd, left], line)?,
                ));
            }
            Some("death") => {
                let [_, clip, "push", fwd, left] = w[..] else {
                    return Err(bad());
                };
                deaths.push((clip.to_string(), pair(&[fwd, left], line)?));
            }
            Some("think") => {
                let [_, "near", a, "mid", b, "far", c, "out", d] = w[..] else {
                    return Err(bad());
                };
                let f = |s: &str| s.parse::<f32>().map_err(|_| bad());
                think = Some(Think {
                    near: f(a)?,
                    mid: f(b)?,
                    far: f(c)?,
                    out: f(d)?,
                });
            }
            Some("reach") => {
                let [_, hand, forearm] = w[..] else {
                    return Err(bad());
                };
                reach = Some(Reach {
                    hand: hand.parse().map_err(|_| bad())?,
                    forearm: forearm.parse().map_err(|_| bad())?,
                });
            }
            _ => {}
        }
    }
    let chr = chr.ok_or("manifest has no monster line")?;
    let kit_name = kit_name.ok_or("manifest has no monster line")?;
    let mut bones = None;
    let mut check = |wow: &wscl::Clip| match bones {
        None => {
            bones = Some(wow.bones());
            Ok(())
        }
        Some(n) if n == wow.bones() => Ok(()),
        Some(n) => Err(format!(
            "{}: {} bones, the kit's other clips have {n}",
            wow.name,
            wow.bones()
        )),
    };
    let mut attack_clips = Vec::new();
    for (name, clip, phys, poise, level, r1, r2, stop, knock) in attacks {
        let wow = read(&kit_name, &clip)?;
        let ds = read(&chr, &clip)?;
        check(&wow)?;
        let hits: Vec<(f32, f32)> = wow.hits.iter().map(|h| (h.start, h.end)).collect();
        let hit = *hits
            .first()
            .ok_or_else(|| format!("{clip} has no hit window"))?;
        let spec = AttackSpec {
            name,
            length: wow.duration(),
            hit,
            hits,
            damage: (phys * DAMAGE_K).round(),
            poise,
            level,
            radius: r1,
            radius2: r2,
            stop,
            knock,
        };
        attack_clips.push(AttackClip { wow, ds, spec });
    }
    if attack_clips.is_empty() {
        return Err("manifest has no attack lines".into());
    }
    let mut react_clips = Vec::new();
    for (level, clip, push) in reacts {
        let wow = read(&kit_name, &clip)?;
        let ds = read(&chr, &clip)?;
        check(&wow)?;
        react_clips.push(ReactClip {
            level,
            wow,
            ds,
            push,
        });
    }
    if react_clips.is_empty() {
        return Err("manifest has no react lines".into());
    }
    let mut death_clips = Vec::new();
    for (clip, push) in deaths {
        let wow = read(&kit_name, &clip)?;
        let ds = read(&chr, &clip)?;
        check(&wow)?;
        death_clips.push(DeathClip { wow, ds, push });
    }
    if death_clips.is_empty() {
        return Err("manifest has no death lines".into());
    }
    Ok(MonsterClips {
        parried: parried_name
            .map(|name| {
                let wow = read(&kit_name, &name)?;
                let ds = read(&chr, &name)?;
                check(&wow)?;
                Ok::<_, String>(DeathClip {
                    wow,
                    ds,
                    push: Vec2::ZERO,
                })
            })
            .transpose()?,
        name: kit_name,
        attacks: attack_clips,
        reacts: react_clips,
        deaths: death_clips,
        scale: scale.ok_or("manifest has no monster scale")?,
        poise_max: poise_max.ok_or("manifest has no monster poise")?,
        reach: reach.ok_or("manifest has no reach line")?,
        think: think.ok_or("manifest has no think line")?,
        souls,
    })
}

/// Every kit of the manifest `text`: it is split at its `monster` lines (a block runs to the next
/// one, lines of other kinds in it are ignored) and each block is [`parse`]d.
pub fn parse_roster(text: &str, dir: &Path) -> Result<Roster, String> {
    let lines: Vec<&str> = text.lines().collect();
    let starts: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].split_whitespace().next() == Some("monster"))
        .collect();
    if starts.is_empty() {
        return Err("manifest has no monster line".into());
    }
    let mut roster = Roster::default();
    for (k, &from) in starts.iter().enumerate() {
        let to = starts.get(k + 1).copied().unwrap_or(lines.len());
        let kit = parse(
            &lines[from..to].join(
                "
",
            ),
            dir,
        )?;
        if roster.0.contains_key(&kit.name) {
            return Err(format!("manifest has two {} kits", kit.name));
        }
        roster.0.insert(kit.name.clone(), kit);
    }
    Ok(roster)
}

/// Reads the kits from `dir`: a checked manifest, then [`parse_roster`].
pub fn load(dir: &Path) -> Result<Roster, String> {
    let text = crate::dsclip::checked_manifest(dir)?;
    parse_roster(&text, dir)
}

/// Poise: a number that soaks up blows. A hit takes its poise damage off `left`; the creature
/// flinches when `left` falls to zero or below, then `left` is whole again; it is whole again
/// after [`POISE_RECOVER`] seconds without a hit. With `max` 0 (the ghoul's) every blow flinches.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Poise {
    pub max: f32,
    pub left: f32,
    pub since_hit: f32,
}

/// Seconds without a blow after which poise is whole again.
pub const POISE_RECOVER: f32 = 5.0;

impl Poise {
    pub fn new(max: f32) -> Poise {
        Poise {
            max,
            left: max,
            since_hit: 0.0,
        }
    }

    /// Takes a blow's poise damage; whether it makes the creature flinch.
    pub fn hit(&mut self, damage: f32) -> bool {
        self.since_hit = 0.0;
        self.left -= damage;
        if self.left <= 0.0 {
            self.left = self.max;
            true
        } else {
            false
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.since_hit += dt;
        if self.since_hit >= POISE_RECOVER {
            self.left = self.max;
        }
    }
}

/// The kit clip a creature is playing and its time (virtual seconds: a hit-stop holds it).
#[derive(Component, Debug, Default)]
pub struct MonsterPlay {
    pub clip: Option<(MonsterClip, f32)>,
}

/// The pose blend state: the last clip's end pose fading under the next.
#[derive(Component, Default)]
pub struct MonsterFade(pub Fade<MonsterClip>);

/// The hand volume's last pose, for the sweep.
#[derive(Component, Debug, Default)]
pub struct HandTrail(pub Option<[Vec3; 2]>);

/// The kobold's dice: `WOWSOULS_SEED=<n>` makes a run repeatable.
#[derive(Resource)]
pub struct FoeRng(pub crate::pattern::Rng);

impl Default for FoeRng {
    fn default() -> Self {
        let seed = std::env::var("WOWSOULS_SEED")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        FoeRng(crate::pattern::Rng(seed))
    }
}

/// `WOWSOULS_MOVE=<name>`: the kobold's next attack is this move (a test aid).
pub fn forced_move(kit: &MonsterClips) -> Option<u8> {
    static WANT: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let want = WANT.get_or_init(|| std::env::var("WOWSOULS_MOVE").ok());
    want.as_deref()
        .and_then(|n| kit.attack_index(n))
        .map(|i| i as u8)
}

fn startup(mut commands: Commands) {
    match load(&crate::dsclip::cache_dir()) {
        Ok(roster) => {
            let mut names: Vec<&String> = roster.0.keys().collect();
            names.sort();
            for name in names {
                let k = &roster.0[name];
                for a in &k.attacks {
                    let s = &a.spec;
                    info!(
                        "monster: {} {} ({:.3} s, hit {:.3}-{:.3}, damage {:.0}, poise {:.0}, level {}, radius {:.2})",
                        s.name,
                        a.wow.name,
                        s.length,
                        s.hit.0,
                        s.hit.1,
                        s.damage,
                        s.poise,
                        s.level,
                        s.radius
                    );
                }
                info!(
                    "monster: {} flinch clips, {} death clips, poise {:.0}, scale {:.3}, the source's bands {:.0}/{:.0}/{:.0}/{:.0} m (the pattern's are in {name} yd)",
                    k.reacts.len(),
                    k.deaths.len(),
                    k.poise_max,
                    k.scale,
                    k.think.near,
                    k.think.mid,
                    k.think.far,
                    k.think.out
                );
            }
            commands.insert_resource(roster);
        }
        Err(why) => warn!("monster: no Dark Souls kits ({why}); the old creatures fight"),
    }
}

/// While a creature plays a kit clip, the clip owns the rig's driven bones, blended over the rig's
/// own pose at its ends; a death holds its last pose (the clip's time keeps running and the
/// sample clamps to the last frame).
fn pose(
    real: Res<Time<bevy::time::Real>>,
    roster: Option<Res<Roster>>,
    mut creatures: Query<(
        &crate::species::Species,
        &MonsterPlay,
        &mut MonsterFade,
        &mut RigPose,
    )>,
) {
    let Some(roster) = roster else {
        return;
    };
    for (species, play, mut fade, mut pose) in &mut creatures {
        let Some(kit) = roster.get(species.0.kit) else {
            continue;
        };
        let layers = layers(&mut fade.0, play.clip, real.delta_secs());
        if layers.iter().all(Option::is_none) {
            continue;
        }
        for (id, t, w) in layers.into_iter().flatten() {
            let Some((wow, _)) = kit.clips(id) else {
                continue;
            };
            if wow.bones() != pose.locals.len() {
                continue;
            }
            for (i, local) in pose.locals.iter_mut().enumerate() {
                if !wow.driven[i] {
                    continue;
                }
                let (tr, r, s) = sample(wow, t, i);
                *local = if w >= 1.0 {
                    Transform {
                        translation: tr,
                        rotation: r,
                        scale: s,
                    }
                } else {
                    Transform {
                        translation: local.translation.lerp(tr, w),
                        rotation: local.rotation.slerp(r, w),
                        scale: local.scale.lerp(s, w),
                    }
                };
            }
        }
        pose.pose_dirty = true;
    }
}

/// Runs each creature's kit clip clock.
fn advance(time: Res<Time>, mut creatures: Query<&mut MonsterPlay>) {
    for mut p in &mut creatures {
        if let Some((_, t)) = &mut p.clip {
            *t += time.delta_secs();
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<FoeRng>()
        .add_systems(Startup, startup)
        .add_systems(Update, advance)
        .add_systems(PostUpdate, pose.in_set(PosePost));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clip of `bones` bones, one second at 10 fps, with an optional hit window.
    fn clip(name: &str, bones: usize, hit: Option<(f32, f32)>) -> wscl::Clip {
        wscl::Clip {
            name: name.into(),
            fps: 10.0,
            model_space: false,
            parents: vec![-1; bones],
            driven: vec![true; bones],
            frames: vec![vec![wscl::Trs::IDENTITY; bones]; 11],
            root_motion: vec![[0.0; 4]; 11],
            flags: Vec::new(),
            hits: hit
                .map(|(start, end)| wscl::HitWindow {
                    judge: 1,
                    start,
                    end,
                })
                .into_iter()
                .collect(),
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
monster kobold chr c2060 npc 206000 hp 203 poise 0 scale 0.6180
souls kobold 150
attack small_1 a00_3000 judge 100 phys 138 poise 10 level 1 radius 0.30 radius2 0.00 stop 0.00 knock 0.45
attack big a00_3002 judge 120 phys 195 poise 20 level 2 radius 0.30 radius2 0.00 stop 0.00 knock 1.00
react 1 a00_2000 push -0.300 0.050
react 2 a00_2007 push -0.500 0.000
death a00_2202 push -0.600 0.100
death a00_2204 push 0.200 0.250
think near 2.0 mid 5.0 far 10.0 out 20.0
reach 53 45
";

    fn kit_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wow-souls-monster-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (c, hit) in [
            ("a00_3000", Some((0.3, 0.4))),
            ("a00_3002", Some((0.5, 0.6))),
            ("a00_2000", None),
            ("a00_2007", None),
            ("a00_2202", None),
            ("a00_2204", None),
        ] {
            put(&dir, "kobold", &clip(c, 4, hit));
            put(&dir, "c2060", &clip(c, 6, hit));
        }
        dir
    }

    #[test]
    fn a_whole_kit_loads_with_its_numbers() {
        let dir = kit_dir("whole");
        let k = parse(MANIFEST, &dir).unwrap_or_else(|e| panic!("{e}"));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(k.attacks.len(), 2);
        let s = &k.attacks[0].spec;
        assert_eq!(
            (s.name.as_str(), s.hit, s.level),
            ("small_1", (0.3, 0.4), 1)
        );
        assert!((s.length - 1.0).abs() < 1.0e-6);
        // 138 · K = 25 exactly; 195 · K = 35.3.
        assert_eq!(s.damage, 25.0);
        assert_eq!(k.attacks[1].spec.damage, 35.0);
        assert_eq!((s.stop, s.knock), (0.0, 0.45));
        assert_eq!(k.attacks[1].spec.knock, 1.0);
        assert_eq!(k.attack_index("big"), Some(1));
        assert_eq!(k.attack_index("nope"), None);
        assert_eq!(
            (k.reacts[0].level, k.reacts[0].push),
            (1, Vec2::new(-0.3, 0.05))
        );
        assert_eq!(k.deaths.len(), 2);
        assert_eq!(k.deaths[1].wow.name, "a00_2204");
        assert_eq!((k.poise_max, k.scale), (0.0, 0.618));
        assert_eq!(k.souls, 150);
        assert_eq!(
            k.reach,
            Reach {
                hand: 53,
                forearm: 45
            }
        );
        assert_eq!(k.think.near, 2.0);
        assert!(k.clips(MonsterClip::Attack(1)).is_some());
        assert!(k.clips(MonsterClip::Attack(2)).is_none());
        assert!(k.clips(MonsterClip::Death(1)).is_some());
    }

    #[test]
    fn a_manifest_of_two_creatures_loads_each_kit_by_name() {
        let dir = kit_dir("roster");
        for (c, hit) in [
            ("a00_3000", Some((0.3, 0.4))),
            ("a00_3002", Some((0.5, 0.6))),
            ("a00_2000", None),
            ("a00_2007", None),
            ("a00_2202", None),
            ("a00_2204", None),
        ] {
            put(&dir, "wolf", &clip(c, 7, hit));
            put(&dir, "c3340", &clip(c, 9, hit));
        }
        let wolf = MANIFEST
            .replace("monster kobold chr c2060", "monster wolf chr c3340")
            .replace("souls kobold 150", "souls wolf 80");
        // The player's reaction lines after the last kit are not the kit's.
        let text = format!("{MANIFEST}{wolf}preact s a00_2000 something else\nmax-error-deg 1.0\n");
        let roster = parse_roster(&text, &dir).unwrap_or_else(|e| panic!("{e}"));
        let (k, w) = (roster.get("kobold").unwrap(), roster.get("wolf").unwrap());
        assert_eq!(
            (k.name.as_str(), k.souls, k.attacks[0].wow.bones()),
            ("kobold", 150, 4)
        );
        assert_eq!(
            (w.name.as_str(), w.souls, w.attacks[0].wow.bones()),
            ("wolf", 80, 7)
        );
        assert!(roster.get("dragon").is_none());
        // Two kits of one name are an error.
        let err = parse_roster(&format!("{MANIFEST}{MANIFEST}"), &dir)
            .err()
            .unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(err.contains("two kobold kits"), "{err}");
        assert!(parse_roster("max-error-deg 1.0\n", &dir)
            .err()
            .unwrap()
            .contains("no monster line"));
    }

    #[test]
    fn an_incomplete_or_malformed_kit_is_refused() {
        let dir = kit_dir("bad");
        let without = |lines: &[&str]| {
            MANIFEST
                .lines()
                .filter(|l| !lines.iter().any(|x| l.starts_with(x)))
                .collect::<Vec<_>>()
                .join("\n")
        };
        for (why, text) in [
            (
                "no monster line",
                MANIFEST.replace("monster kobold", "x kobold"),
            ),
            ("no death lines", without(&["death"])),
            ("no react lines", without(&["react"])),
            ("no reach line", without(&["reach"])),
            ("bad monster line", MANIFEST.replace("phys 138", "phys x")),
            (
                "a00_2000 has no hit window",
                MANIFEST.replace("attack small_1 a00_3000", "attack small_1 a00_2000"),
            ),
        ] {
            let err = parse(&text, &dir)
                .err()
                .unwrap_or_else(|| panic!("{why} loads"));
            assert!(err.contains(why), "{why}: {err}");
        }
        std::fs::remove_dir_all(&dir).ok();
        // A clip with another bone count than the rest.
        let dir = kit_dir("bones");
        put(&dir, "kobold", &clip("a00_2007", 5, None));
        let err = parse(MANIFEST, &dir).err().unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(err.contains("5 bones"), "{err}");
    }

    #[test]
    fn poise_zero_always_flinches_and_poise_soaks_a_weak_blow() {
        let mut ghoul = Poise::new(0.0);
        assert!(ghoul.hit(0.0) && ghoul.hit(10.0));
        let mut stout = Poise::new(30.0);
        assert!(!stout.hit(10.0), "20 left");
        assert!(!stout.hit(10.0), "10 left");
        assert!(stout.hit(10.0), "0 left: it breaks and is whole again");
        assert_eq!(stout.left, 30.0);
        stout.hit(10.0);
        stout.tick(POISE_RECOVER);
        assert_eq!(stout.left, 30.0, "rested");
    }

    #[test]
    fn a_blow_from_the_front_takes_the_clip_that_throws_the_head_back() {
        let dir = kit_dir("react");
        let k = parse(MANIFEST, &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        // Level 1 has the one clip (a00_2000, push back); level 2 the other.
        assert_eq!(choose_react(&k, 1, Vec2::new(-1.0, 0.0)), Some(0));
        assert_eq!(choose_react(&k, 2, Vec2::new(-1.0, 0.0)), Some(1));
        assert_eq!(choose_react(&k, 2, Vec2::ZERO), Some(1));
        assert_eq!(
            choose_react(&k, 9, Vec2::new(-1.0, 0.0)),
            Some(1),
            "levels above 2 clamp"
        );
        // Deaths: the head thrown back falls on its back, a blow from behind falls on its face.
        assert_eq!(choose_death(&k, Vec2::new(-1.0, 0.0)), Some(0));
        assert_eq!(choose_death(&k, Vec2::new(1.0, 0.2)), Some(1));
    }

    #[test]
    fn a_death_holds_its_last_pose_and_undriven_bones_are_left_alone() {
        let mut c = clip("a00_2202", 2, None);
        c.driven = vec![true, false];
        c.frames[10][0].t = [0.0, 5.0, 0.0];
        let (t, _, _) = crate::dsclip::sample(&c, 99.0, 0);
        assert_eq!(t.y, 5.0, "clamped to the last frame: the pose holds");
        assert!(!c.driven[1]);
    }

    #[test]
    fn a_move_with_two_hit_windows_keeps_both_and_the_first_leads() {
        let mut c = clip("a00_3002", 2, Some((0.2, 0.3)));
        c.hits.push(wscl::HitWindow {
            judge: 1,
            start: 0.6,
            end: 0.7,
        });
        let dir = std::env::temp_dir().join(format!("wscl-two-windows-{}", std::process::id()));
        put(&dir, "garrick", &c);
        put(&dir, "c2400", &c);
        put(&dir, "garrick", &clip("a00_2000", 2, None));
        put(&dir, "c2400", &clip("a00_2000", 2, None));
        put(&dir, "garrick", &clip("a00_2204", 2, None));
        put(&dir, "c2400", &clip("a00_2204", 2, None));
        let text = "monster garrick chr c2400 npc 240000 hp 110 poise 0 scale 1.1
                    attack double a00_3002 judge 120 phys 140 poise 20 level 1 radius 0.40 radius2 0.00 stop 0.00 knock 0.45
                    react 1 a00_2000 push -0.3 0.0
                    death a00_2204 push -0.6 0.0
                    think near 2.0 mid 5.0 far 10.0 out 20.0
                    reach 3 4
";
        let k = parse(text, &dir).unwrap_or_else(|e| panic!("{e}"));
        std::fs::remove_dir_all(&dir).ok();
        let s = &k.attacks[0].spec;
        assert_eq!(s.hit, (0.2, 0.3));
        assert_eq!(s.hits, vec![(0.2, 0.3), (0.6, 0.7)]);
    }
}
