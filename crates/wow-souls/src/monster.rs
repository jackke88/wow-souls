//! Source-derived creature kits from the tracked `anim/source/` project library: the
//! ghoul's six melee moves, its flinch clips and its deaths, retargeted onto the kobold rig, with
//! each move's hit window, damage and poise from the install's data. Loaded at startup with the
//! player's clips; without a cache, or with a stale one, the old kobold fights.
//!
//! While a creature plays a kit clip ([`MonsterPlay`]) the clip owns the rig's driven bones,
//! blended over the rig's own pose at its ends (the player's blend code, [`crate::motion_clip`]); a
//! death holds its last pose.

use std::collections::HashMap;
use std::path::Path;

use benilla_world::rig_anim::{PosePost, RigPose};
use bevy::prelude::*;

use crate::motion_clip::{layers, sample, Fade};

#[path = "remix.rs"]
pub(crate) mod remix;
#[path = "remix_live.rs"]
mod remix_live;
#[path = "wolf_motion.rs"]
mod wolf_motion;

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
    pub reference: wscl::Clip,
    pub spec: AttackSpec,
    /// Mixed-source attacks retain their own DS-to-target scale; legacy kits use the kit scale.
    pub source_scale: Option<f32>,
    /// An actual foot/toe segment for a kick, instead of the species' held weapon.
    pub reach: Option<Reach>,
    pub grip: Option<AttackGrip>,
    /// A cast: the bolt it releases at its blow instead of a swept volume ([`crate::foebolt`]).
    pub bullet: Option<Bullet>,
    /// A hit on the ground where the blow lands ([`crate::shock`]): it hurts a player standing in it even when the
    /// blade misses.
    pub shock: Option<Shock>,
    /// A blast all round the creature at a moment of the clip (the knight's power-up).
    pub blast: Option<Shock>,
}

/// A ground shock or a blast (`attack-shock` / `attack-blast`): when it lands (clip s), its radius and push (DS
/// metres, scaled as a reach), its damage (`atkPhys` x [`DAMAGE_K`]) and its flinch level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shock {
    pub at: f32,
    pub radius: f32,
    pub damage: f32,
    pub level: u8,
    /// Its poise damage (`atkSuperArmor` of its own row: the knight's blast breaks 300).
    pub poise: f32,
    pub knock: f32,
}

/// A cast's bolt, from the Dark Souls `Bullet` row (metres, seconds, degrees as the row has them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bullet {
    pub speed: f32,
    pub max: f32,
    pub accel: f32,
    pub radius: f32,
    pub homing: f32,
    pub life: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackGrip {
    pub hand: usize,
    pub weapon: usize,
    pub blade_axis: Vec3,
}

impl AttackClip {
    /// Carry the source weapon's full rotation in the target hand, including its axial twist.
    pub fn animated_grip(&self, t: f32, blade: [Vec3; 2]) -> Option<Quat> {
        let grip = self.grip?;
        let mut q = Quat::IDENTITY;
        let mut bone = Some(grip.hand);
        while let Some(i) = bone {
            q = sample(&self.wow, t, i).1 * q;
            bone = usize::try_from(self.wow.parents[i]).ok();
        }
        let mount = crate::motion_clip::grip_rotation(blade, grip.blade_axis);
        Some(q.inverse() * sample(&self.reference, t, grip.weapon).1 * mount)
    }
}

/// One flinch clip, the level it answers and where it pushes the head.
pub struct ReactClip {
    pub level: u8,
    pub wow: wscl::Clip,
    pub reference: wscl::Clip,
    /// `(forward, left)` of the head's push in the creature's frame (negative forward: head back).
    pub push: Vec2,
}

/// One death clip and the way it throws the head.
pub struct DeathClip {
    pub wow: wscl::Clip,
    pub reference: wscl::Clip,
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
            MonsterClip::Attack(i) => self
                .attacks
                .get(usize::from(i))
                .map(|a| (&a.wow, &a.reference)),
            MonsterClip::React(i) => self
                .reacts
                .get(usize::from(i))
                .map(|r| (&r.wow, &r.reference)),
            MonsterClip::Death(i) => self
                .deaths
                .get(usize::from(i))
                .map(|d| (&d.wow, &d.reference)),
            MonsterClip::Parried => self.parried.as_ref().map(|d| (&d.wow, &d.reference)),
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

/// Kits whose Dark Souls deaths do not carry over: the Armored Tusk rolls its whole body onto its side,
/// and the boar rig turns only its spine (the hips stay), so the front half lay on its back over upright
/// hind legs (2026-10-17: up to 140 degrees of spine twist against still hips). These die with the WoW
/// model's own Death, which plays under every kit death anyway.
pub fn wow_death(kit: &str) -> bool {
    matches!(kit, "boar" | "princess")
}

/// The death clip for a killing blow that pushes the creature along `push`.
pub fn choose_death(kit: &MonsterClips, push: Vec2) -> Option<u8> {
    if wow_death(&kit.name) {
        return None;
    }
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
    let read = |sub: &str, name: &str| crate::motion_clip::read_clip(dir, sub, name);
    let (mut scale, mut poise_max, mut chr, mut kit_name) = (None, None, None, None);
    let mut reference_dir = None;
    let (mut attacks, mut reacts, mut deaths) = (Vec::new(), Vec::new(), Vec::new());
    let (mut reach, mut think) = (None, None);
    let mut souls = 0u32;
    let mut parried_name = None;
    let mut source_scales = HashMap::new();
    let mut attack_reaches = HashMap::new();
    let mut attack_grips = HashMap::new();
    let mut attack_bullets = HashMap::new();
    let mut attack_shocks = HashMap::new();
    let mut attack_blasts = HashMap::new();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let bad = || format!("bad monster line {line:?}");
        match w.first().copied() {
            Some("attack-source") => {
                let [_, name, "chr", _, "weapon", _, "variation", _, "scale", value] = w[..] else {
                    return Err(bad());
                };
                let scale = value.parse::<f32>().map_err(|_| bad())?;
                if !scale.is_finite()
                    || scale <= 0.0
                    || source_scales.insert(name.to_string(), scale).is_some()
                {
                    return Err(bad());
                }
            }
            Some("attack-reach") => {
                let [_, name, "bones", start, end] = w[..] else {
                    return Err(bad());
                };
                let reach = Reach {
                    hand: start.parse().map_err(|_| bad())?,
                    forearm: end.parse().map_err(|_| bad())?,
                };
                if reach.hand == reach.forearm
                    || attack_reaches.insert(name.to_string(), reach).is_some()
                {
                    return Err(bad());
                }
            }
            Some("attack-grip") => {
                let [_, name, "hand", hand, "weapon", weapon, "axis", x, y, z, "blade", bx, by, bz] =
                    w[..]
                else {
                    return Err(bad());
                };
                let vector = |x: &str, y: &str, z: &str| -> Result<Vec3, String> {
                    let v = Vec3::new(
                        x.parse().map_err(|_| bad())?,
                        y.parse().map_err(|_| bad())?,
                        z.parse().map_err(|_| bad())?,
                    );
                    if !v.is_finite() || v.length_squared() < 1e-6 {
                        return Err(bad());
                    }
                    Ok(v.normalize())
                };
                vector(x, y, z)?;
                let grip = AttackGrip {
                    hand: hand.parse().map_err(|_| bad())?,
                    weapon: weapon.parse().map_err(|_| bad())?,
                    blade_axis: vector(bx, by, bz)?,
                };
                if attack_grips.insert(name.to_string(), grip).is_some() {
                    return Err(bad());
                }
            }
            Some(kind @ ("attack-shock" | "attack-blast")) => {
                let (name, rest) = match (kind, &w[..]) {
                    ("attack-shock", [_, name, "judge", _, rest @ ..]) => (*name, rest),
                    ("attack-blast", [_, name, rest @ ..]) => (*name, rest),
                    _ => return Err(bad()),
                };
                let ["at", at, "radius", radius, "phys", phys, "poise", poise, "level", level, "knock", knock] =
                    rest[..]
                else {
                    return Err(bad());
                };
                let f = |s: &str| s.parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0);
                let shock = Shock {
                    at: f(at).ok_or_else(bad)?,
                    radius: f(radius).filter(|v| *v > 0.0).ok_or_else(bad)?,
                    damage: (f(phys).ok_or_else(bad)? * DAMAGE_K).round(),
                    poise: f(poise).ok_or_else(bad)?,
                    level: level.parse().map_err(|_| bad())?,
                    knock: f(knock).ok_or_else(bad)?,
                };
                let map = if kind == "attack-shock" {
                    &mut attack_shocks
                } else {
                    &mut attack_blasts
                };
                if map.insert(name.to_string(), shock).is_some() {
                    return Err(bad());
                }
            }
            Some("attack-bullet") => {
                let [_, name, "speed", speed, "max", max, "accel", accel, "radius", radius, "homing", homing, "life", life] =
                    w[..]
                else {
                    return Err(bad());
                };
                let f = |s: &str| s.parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0);
                let bullet = Bullet {
                    speed: f(speed).ok_or_else(bad)?,
                    max: f(max).ok_or_else(bad)?,
                    accel: f(accel).ok_or_else(bad)?,
                    radius: f(radius).ok_or_else(bad)?,
                    homing: f(homing).ok_or_else(bad)?,
                    life: f(life).filter(|v| *v > 0.0).ok_or_else(bad)?,
                };
                if attack_bullets.insert(name.to_string(), bullet).is_some() {
                    return Err(bad());
                }
            }
            Some("reference") => {
                let [_, dir] = w[..] else { return Err(bad()) };
                reference_dir = Some(dir.to_string());
            }
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
    // A DS source may be retargeted onto several bodies. Its reference clips are scaled for
    // that target; older kits keep their original source-character folder.
    let reference_dir = reference_dir.unwrap_or(chr);
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
        let reference = read(&reference_dir, &clip)?;
        check(&wow)?;
        let hits: Vec<(f32, f32)> = wow.hits.iter().map(|h| (h.start, h.end)).collect();
        let hit = *hits
            .first()
            .ok_or_else(|| format!("{clip} has no hit window"))?;
        let source_scale = source_scales.remove(&name);
        let reach = attack_reaches.remove(&name);
        if let Some(r) = reach {
            // A toe inherits the driven foot's pose and need not have its own rotation track.
            if [r.hand, r.forearm]
                .iter()
                .any(|&i| usize::from(i) >= wow.bones())
                || !wow.driven[usize::from(r.hand)]
            {
                return Err(format!(
                    "{name}: attack reach is outside the driven target rig"
                ));
            }
        }
        let grip = attack_grips.remove(&name);
        let bullet = attack_bullets.remove(&name);
        let shock = attack_shocks.remove(&name);
        let blast = attack_blasts.remove(&name);
        if grip.is_some_and(|g| g.hand >= wow.bones() || g.weapon >= reference.bones()) {
            return Err(format!("{name}: grip is outside the clip skeletons"));
        }
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
        attack_clips.push(AttackClip {
            wow,
            reference,
            spec,
            source_scale,
            reach,
            grip,
            bullet,
            shock,
            blast,
        });
    }
    if !source_scales.is_empty()
        || !attack_reaches.is_empty()
        || !attack_grips.is_empty()
        || !attack_bullets.is_empty()
        || !attack_shocks.is_empty()
        || !attack_blasts.is_empty()
    {
        return Err("attack metadata names an unknown move".into());
    }
    if attack_clips.is_empty() {
        return Err("manifest has no attack lines".into());
    }
    let mut react_clips = Vec::new();
    for (level, clip, push) in reacts {
        let wow = read(&kit_name, &clip)?;
        let reference = read(&reference_dir, &clip)?;
        check(&wow)?;
        react_clips.push(ReactClip {
            level,
            wow,
            reference,
            push,
        });
    }
    if react_clips.is_empty() {
        return Err("manifest has no react lines".into());
    }
    let mut death_clips = Vec::new();
    for (clip, push) in deaths {
        let wow = read(&kit_name, &clip)?;
        let reference = read(&reference_dir, &clip)?;
        check(&wow)?;
        death_clips.push(DeathClip {
            wow,
            reference,
            push,
        });
    }
    if death_clips.is_empty() {
        return Err("manifest has no death lines".into());
    }
    Ok(MonsterClips {
        parried: parried_name
            .map(|name| {
                let wow = read(&kit_name, &name)?;
                let reference = read(&reference_dir, &name)?;
                check(&wow)?;
                Ok::<_, String>(DeathClip {
                    wow,
                    reference,
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
    let text = crate::motion_clip::checked_manifest(dir)?;
    let mut roster = parse_roster(&text, dir)?;
    // The spiders' legs stop at the ground ([`crate::legfloor`]).
    for kit in roster.0.values_mut() {
        if crate::legfloor::wants_floor(&kit.name) {
            let mut lifted = 0;
            for a in &mut kit.attacks {
                lifted += crate::legfloor::keep_off_the_ground(&mut a.wow);
            }
            for r in &mut kit.reacts {
                lifted += crate::legfloor::keep_off_the_ground(&mut r.wow);
            }
            info!(
                "monster: {} legs kept out of the ground ({lifted} joint-frames)",
                kit.name
            );
        }
    }
    if remix::enabled() {
        for kit in roster.0.values_mut() {
            remix::apply_loaded(kit);
        }
    } else if wolf_motion::enabled() {
        for kit in roster.0.values_mut() {
            wolf_motion::apply_loaded(kit);
        }
    }
    Ok(roster)
}

/// Poise: a number that soaks up blows. A hit takes its poise damage off `left`; the creature
/// flinches when `left` falls to zero or below, then `left` is whole again; it is whole again
/// after [`POISE_RECOVER`] seconds without a hit. With `max` 0 (the ghoul's) every blow flinches.
///
/// No stun-lock (2026-10-17, the user: a one-handed sword chained gnolls so they "kan inte göra något"):
/// after [`CHAIN_FLINCHES`] flinches less than [`CHAIN_WINDOW`] s apart the creature shrugs off blows for
/// [`CHAIN_ARMOR`] s, long enough to stand up and swing back, as Dark Souls' hollows trade with a player
/// who keeps swinging.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Poise {
    pub max: f32,
    pub left: f32,
    pub since_hit: f32,
    /// Flinches in the current chain, and seconds since the last one.
    pub chain: u8,
    pub since_flinch: f32,
    /// Seconds left of shrugging blows off.
    pub armor: f32,
}

/// Seconds without a blow after which poise is whole again.
pub const POISE_RECOVER: f32 = 5.0;
/// Flinches in a row a creature takes before it shrugs the next blows off.
pub const CHAIN_FLINCHES: u8 = 2;
/// Two flinches further apart than this (s) are not one chain.
pub const CHAIN_WINDOW: f32 = 2.5;
/// How long it shrugs blows off once a chain is full (s).
pub const CHAIN_ARMOR: f32 = 2.2;

impl Poise {
    pub fn new(max: f32) -> Poise {
        Poise {
            max,
            left: max,
            since_hit: 0.0,
            chain: 0,
            since_flinch: CHAIN_WINDOW,
            armor: 0.0,
        }
    }

    /// Takes a blow's poise damage; whether it makes the creature flinch.
    pub fn hit(&mut self, damage: f32) -> bool {
        self.since_hit = 0.0;
        self.left -= damage;
        if self.left > 0.0 {
            return false;
        }
        self.left = self.max;
        true
    }

    /// [`Poise::hit`] for a creature: a full chain of flinches buys it [`CHAIN_ARMOR`] (the player's own
    /// poise keeps flinching every time).
    pub fn hit_chained(&mut self, damage: f32) -> bool {
        if self.armor > 0.0 {
            self.since_hit = 0.0;
            return false;
        }
        if !self.hit(damage) {
            return false;
        }
        if self.since_flinch >= CHAIN_WINDOW {
            self.chain = 0;
        }
        self.since_flinch = 0.0;
        if self.chain >= CHAIN_FLINCHES {
            // The chain is full: this blow lands without a flinch, and the next ones for a while.
            self.chain = 0;
            self.armor = CHAIN_ARMOR;
            return false;
        }
        self.chain += 1;
        true
    }

    pub fn tick(&mut self, dt: f32) {
        self.since_hit += dt;
        self.since_flinch += dt;
        self.armor = (self.armor - dt).max(0.0);
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
    match load(&crate::motion_clip::cache_dir()) {
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
        Option<&crate::actor::held::HeldBuilt>,
    )>,
    mut items: Query<&mut Transform>,
) {
    let Some(roster) = roster else {
        return;
    };
    for (species, play, mut fade, mut pose, held) in &mut creatures {
        let Some(kit) = roster.get(species.kit()) else {
            continue;
        };
        let layers = layers(&mut fade.0, play.clip, real.delta_secs());
        if kit.attacks.iter().any(|a| a.grip.is_some()) {
            if let Some((held, blade)) = held.and_then(|h| h.blade.map(|b| (h, b))) {
                let rotation =
                    layers
                        .iter()
                        .flatten()
                        .fold(Quat::IDENTITY, |rotation, &(id, t, w)| {
                            let grip = match id {
                                MonsterClip::Attack(i) => kit
                                    .attacks
                                    .get(usize::from(i))
                                    .and_then(|a| a.animated_grip(t, blade)),
                                _ => None,
                            };
                            grip.map_or(rotation, |q| rotation.slerp(q, w))
                        });
                if let Ok(mut item) = items.get_mut(held.root) {
                    item.rotation = rotation;
                }
            }
        }
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
        if let Some(torso) = torso_for(species.kit()) {
            let RigPose {
                locals, parents, ..
            } = &mut *pose;
            reshape(locals, parents, torso, torso_scale());
        }
        pose.pose_dirty = true;
    }
}

/// One bone of a kit's torso chain: where it may stand next to the bind pose and next to the bone
/// above it. The Taurus Demon bends its whole trunk over four spine bones and rolls its shoulders
/// through the neck bone; the gnoll it is drawn on has a short trunk of two bones (a spine base and
/// a chest) and no collar bones, so the same bend lands on one hinge and the mesh folds and pinches
/// (the chest rolled 70 degrees sideways over hips rolled 20 is a thin column with a head on top).
/// So every bone is held both near the model's own stance and near the bone it hangs from: the
/// trunk follows the hips, bends a little between its bones, and never folds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Link {
    pub bone: usize,
    /// The bone above it in the chain, whose placed rotation this one is held against.
    pub after: usize,
    /// The most (radians) the bone's lean (its swing off the vertical) and its twist (the turn
    /// about it) may stand from the bind pose, in the model's frame. Soft: a small angle is kept as
    /// it is, a large one saturates at the limit.
    pub from_bind: (f32, f32),
    /// The same against `after`, in its frame.
    pub from_after: (f32, f32),
    /// Forward pitch (radians) added on top of `after`: a model drawn upright is made to stand
    /// the way its kind does.
    pub hunch: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Torso {
    /// The chain, parents first.
    pub chain: &'static [Link],
    /// Bones below the chain that keep the clip's own orientation in the model's frame (the arms:
    /// the swing is theirs).
    pub hold: &'static [usize],
    /// Bones that keep the clip's orientation and their place in the model (the waist, whose
    /// hips and legs hang off the spine base).
    pub anchor: &'static [usize],
}

/// The gnoll's (`GnollMelee`, `bonemap::GNOLL`): 11 spine base, 13 chest, 30 head; 12 the waist
/// (the hips and legs hang off it), 16 / 17 the arm roots. The spine base stays within 11 degrees of
/// the hips, the chest within 29 of the spine base and 31 of the bind pose, the head within 46 of
/// the chest; 17 degrees of the hunch go on the spine base and 17 on the chest.
pub const GNOLL_TORSO: Torso = Torso {
    chain: &[
        Link {
            bone: 11,
            after: 12,
            from_bind: (0.45, 0.7),
            from_after: (0.2, 0.45),
            hunch: 0.2,
        },
        Link {
            bone: 13,
            after: 11,
            from_bind: (0.55, 0.9),
            from_after: (0.5, 0.7),
            hunch: 0.3,
        },
        Link {
            bone: 30,
            after: 13,
            from_bind: (0.8, 1.1),
            from_after: (0.8, 1.0),
            hunch: 0.0,
        },
    ],
    hold: &[16, 17],
    anchor: &[12],
};

/// The torso shape of a kit, if it has one.
pub fn torso_for(kit: &str) -> Option<&'static Torso> {
    (kit == "hogger").then_some(&GNOLL_TORSO)
}

/// How much of the shaping is applied (`WOWSOULS_TORSO`, 1 by default, 0 = the clips as they are).
pub fn torso_scale() -> f32 {
    static SCALE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| {
        std::env::var("WOWSOULS_TORSO")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .map_or(1.0, |v| v.min(1.0))
    })
}

/// How much of the table's hunch is applied (`WOWSOULS_HUNCH`, 1 by default, 0 = none, at most 2).
pub fn hunch_scale() -> f32 {
    static SCALE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| {
        std::env::var("WOWSOULS_HUNCH")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .map_or(1.0, |v| v.min(2.0))
    })
}

/// `angle` held to `max` softly: the same for a small one, saturating at `max`.
fn soft(angle: f32, max: f32) -> f32 {
    if max <= 0.0 {
        0.0
    } else {
        max * (angle / max).tanh()
    }
}

/// `q`'s angle (the shortest turn) held softly to `max`.
fn limit_angle(q: Quat, max: f32) -> Quat {
    let q = if q.w < 0.0 { -q } else { q };
    let (axis, angle) = q.normalize().to_axis_angle();
    if angle < 1.0e-6 || !axis.is_finite() {
        return Quat::IDENTITY;
    }
    Quat::from_axis_angle(axis, soft(angle, max))
}

/// `rotation` with its lean (the swing off the vertical axis) held softly to `lean_max` radians
/// and its twist (the turn about the vertical) to `twist_max`.
pub fn capped(rotation: Quat, lean_max: f32, twist_max: f32) -> Quat {
    let about_up = Quat::from_xyzw(0.0, rotation.y, 0.0, rotation.w);
    let twist = if about_up.length_squared() > 1.0e-9 {
        about_up.normalize()
    } else {
        Quat::IDENTITY
    };
    let swing = rotation * twist.inverse();
    (limit_angle(swing, lean_max) * limit_angle(twist, twist_max)).normalize()
}

/// The world rotation of `bone` from the locals (the model's frame; the root's parent is the
/// model).
fn world_rotation(locals: &[Transform], parents: &[i16], bone: usize) -> Quat {
    let mut chain = vec![bone];
    while let Ok(p) = usize::try_from(parents[*chain.last().expect("never empty")]) {
        if p >= locals.len() || chain.len() > locals.len() {
            break;
        }
        chain.push(p);
    }
    chain
        .iter()
        .rev()
        .fold(Quat::IDENTITY, |q, &b| q * locals[b].rotation)
}

/// Holds a posed rig's torso as `torso` says, at `scale` (0 leaves it alone, 1 is the table's).
/// The chain's bones are re-placed from the first down, each against the one it follows; what
/// hangs below keeps what `hold` and `anchor` ask for, everything else follows its parent as it
/// did.
pub fn reshape(locals: &mut [Transform], parents: &[i16], torso: &Torso, scale: f32) {
    let n = locals.len();
    let named = torso
        .chain
        .iter()
        .flat_map(|l| [l.bone, l.after])
        .chain(torso.hold.iter().copied())
        .chain(torso.anchor.iter().copied());
    if scale <= 0.0 || parents.len() != n || named.into_iter().any(|b| b >= n) {
        return;
    }
    let scale = scale.min(1.0);
    // A limit at scale 1 is the table's; towards 0 it opens up to a half turn (no limit).
    let open = |max: f32| max + (std::f32::consts::PI - max) * (1.0 - scale);
    let world: Vec<Quat> = (0..n).map(|b| world_rotation(locals, parents, b)).collect();
    let mut target: Vec<Option<Quat>> = vec![None; n];
    for link in torso.chain {
        let after = target[link.after].unwrap_or(world[link.after]);
        let from_bind = capped(
            world[link.bone],
            open(link.from_bind.0),
            open(link.from_bind.1),
        );
        let from_after = capped(
            after.inverse() * from_bind,
            open(link.from_after.0),
            open(link.from_after.1),
        );
        // Pitched forward in the frame it hangs from: -Z is forward, +X tips the top back.
        let forward = Quat::from_rotation_x(-link.hunch * hunch_scale() * scale);
        target[link.bone] = Some((after * forward * from_after).normalize());
    }
    for &bone in torso.hold.iter().chain(torso.anchor) {
        target[bone].get_or_insert(world[bone]);
    }
    // Parents first: a bone follows its (new) parent unless it has a target of its own.
    let mut fresh = world.clone();
    let order = {
        let mut order: Vec<usize> = (0..n).collect();
        let depth = |mut b: usize| {
            let mut d = 0;
            while let Ok(p) = usize::try_from(parents[b]) {
                if p >= n || d > n {
                    break;
                }
                b = p;
                d += 1;
            }
            d
        };
        order.sort_by_key(|&b| depth(b));
        order
    };
    for b in order {
        let parent = usize::try_from(parents[b]).ok().filter(|&p| p < n);
        let parent_now = parent.map_or(Quat::IDENTITY, |p| fresh[p]);
        fresh[b] = match target[b] {
            Some(q) => q,
            None => parent_now * locals[b].rotation,
        };
        // An anchored bone stays where it was: its offset in its parent's turned frame.
        if torso.anchor.contains(&b) {
            if let Some(p) = parent {
                let moved = parent_now.inverse() * world[p] * locals[b].translation;
                locals[b].translation = moved;
            }
        }
        locals[b].rotation = (parent_now.inverse() * fresh[b]).normalize();
    }
}

/// Runs each creature's kit clip clock (a boss's second phase runs it faster, [`crate::boss::Tempo`]).
fn advance(
    time: Res<Time>,
    mut creatures: Query<(
        &mut MonsterPlay,
        Option<&crate::boss::Tempo>,
        Option<&crate::npc::Brain>,
    )>,
) {
    for (mut p, tempo, brain) in &mut creatures {
        if let Some((id, t)) = &mut p.clip {
            // The mind owns a swing's clock: it holds a feinted swing at its tell.
            if matches!(id, MonsterClip::Attack(_)) && brain.is_some_and(|b| b.owns_body()) {
                continue;
            }
            *t += time.delta_secs() * tempo.map_or(1.0, |t| t.0);
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<FoeRng>()
        .add_systems(Startup, startup)
        .add_systems(Update, advance)
        .add_systems(PostUpdate, pose.in_set(PosePost));
    if remix::enabled() {
        remix_live::plugin(app);
    }
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

    /// The rotation and place of `bone` in the model's frame.
    fn placed(locals: &[Transform], parents: &[i16], bone: usize) -> (Quat, Vec3) {
        let mut chain = vec![bone];
        while let Ok(p) = usize::try_from(parents[*chain.last().unwrap()]) {
            chain.push(p);
        }
        chain
            .iter()
            .rev()
            .fold((Quat::IDENTITY, Vec3::ZERO), |(q, p), &b| {
                (q * locals[b].rotation, p + q * locals[b].translation)
            })
    }

    /// A root, a spine base (1), a waist (2) and a chest (3) off it, an arm (4) off the chest.
    fn spine() -> (Vec<Transform>, Vec<i16>, Torso) {
        let at = |t: Vec3| Transform::from_translation(t);
        (
            vec![
                at(Vec3::ZERO),
                at(Vec3::new(0.0, 1.0, 0.0)),
                at(Vec3::new(0.0, -0.1, 0.0)),
                at(Vec3::new(0.0, 0.4, 0.0)),
                at(Vec3::new(0.2, 0.3, 0.0)),
            ],
            vec![-1, 0, 1, 1, 3],
            Torso {
                chain: &[
                    Link {
                        bone: 1,
                        after: 2,
                        from_bind: (0.5, 0.5),
                        from_after: (0.3, 0.3),
                        hunch: 0.0,
                    },
                    Link {
                        bone: 3,
                        after: 1,
                        from_bind: (0.8, 0.8),
                        from_after: (0.4, 0.4),
                        hunch: 0.0,
                    },
                ],
                hold: &[4],
                anchor: &[2],
            },
        )
    }

    #[test]
    fn a_cap_keeps_a_small_angle_and_saturates_a_large_one() {
        // A pitch of 0.05 under a cap of 0.5 is left (the soft limit has slope 1 at zero) ...
        let small = capped(Quat::from_rotation_x(-0.05), 0.5, 0.5);
        assert!((small.angle_between(Quat::IDENTITY) - 0.05).abs() < 2.0e-3);
        // ... a half turn comes out under the cap, and a wrapped-around one is the same turn.
        let big = capped(Quat::from_rotation_x(-3.0), 0.5, 0.5);
        let angle = big.angle_between(Quat::IDENTITY);
        assert!(angle > 0.45 && angle <= 0.5, "{angle}");
        // A turn about the vertical is the twist's cap, not the lean's.
        let yaw = capped(Quat::from_rotation_y(1.4), 0.1, 0.9);
        assert!((yaw.angle_between(Quat::IDENTITY) - 0.9 * (1.4_f32 / 0.9).tanh()).abs() < 1.0e-3);
        assert!(capped(Quat::IDENTITY, 0.3, 0.3).angle_between(Quat::IDENTITY) < 1.0e-5);
        // A cap of nothing stands the bone at its bind.
        assert!(
            capped(Quat::from_rotation_z(0.9), 0.0, 0.0).angle_between(Quat::IDENTITY) < 1.0e-5
        );
    }

    #[test]
    fn the_trunk_follows_the_hips_and_never_folds_while_arms_and_waist_stay() {
        let (mut locals, parents, torso) = spine();
        // The hips pitched 57 degrees, the chest rolled 70 on top of that: a fold.
        locals[1].rotation = Quat::from_rotation_x(-1.0);
        locals[3].rotation = Quat::from_rotation_z(1.2);
        locals[4].rotation = Quat::from_rotation_z(0.5);
        let before: Vec<(Quat, Vec3)> = (0..5).map(|b| placed(&locals, &parents, b)).collect();
        reshape(&mut locals, &parents, &torso, 1.0);
        let after: Vec<(Quat, Vec3)> = (0..5).map(|b| placed(&locals, &parents, b)).collect();
        // The spine base is within its reach of the waist, the chest within its of the base.
        let apart = |a: Quat, b: Quat| (a.inverse() * b).angle_between(Quat::IDENTITY);
        assert!(
            apart(after[2].0, after[1].0) <= 0.43,
            "{}",
            apart(after[2].0, after[1].0)
        );
        assert!(
            apart(after[1].0, after[3].0) <= 0.57,
            "{}",
            apart(after[1].0, after[3].0)
        );
        // ... where the clip had the chest 1.2 rad off its base.
        assert!(apart(before[1].0, before[3].0) > 1.0);
        // The arm keeps its orientation in the model; the waist its orientation and its place.
        assert!(after[4].0.angle_between(before[4].0) < 1.0e-4);
        assert!(after[2].0.angle_between(before[2].0) < 1.0e-4);
        assert!(
            (after[2].1 - before[2].1).length() < 1.0e-4,
            "{:?} {:?}",
            after[2].1,
            before[2].1
        );
    }

    #[test]
    fn a_hunch_pitches_the_chest_forward_over_the_bone_it_hangs_from() {
        let (mut locals, parents, mut torso) = spine();
        let mut chain = torso.chain.to_vec();
        chain[1].hunch = 0.3;
        // A leak is fine here: a test's table, a few bytes.
        torso.chain = Box::leak(chain.into_boxed_slice());
        reshape(&mut locals, &parents, &torso, 1.0);
        let (base, _) = placed(&locals, &parents, 1);
        let (chest, _) = placed(&locals, &parents, 3);
        let forward = (base.inverse() * chest) * Vec3::Y;
        // Forward is -Z: the top of the chest leans toward it by 0.3 rad.
        assert!((forward.z + 0.3_f32.sin()).abs() < 1.0e-3, "{forward:?}");
        assert!(forward.x.abs() < 1.0e-4);
    }

    #[test]
    fn a_zero_scale_or_a_wrong_sized_rig_leaves_the_pose_alone() {
        let (mut locals, parents, torso) = spine();
        locals[1].rotation = Quat::from_rotation_x(-1.0);
        let orig = locals.clone();
        reshape(&mut locals, &parents, &torso, 0.0);
        assert_eq!(locals, orig);
        reshape(&mut locals[..3], &parents[..3], &torso, 1.0);
        assert_eq!(locals, orig);
        // The gnoll's table names bones a gnoll has (it has 54), each against a bone above it.
        assert_eq!(torso_for("hogger"), Some(&GNOLL_TORSO));
        assert_eq!(torso_for("kobold"), None);
        for l in GNOLL_TORSO.chain {
            assert!(l.bone < 54 && l.after < 54 && l.bone != l.after);
            assert!((0.0..=1.2).contains(&l.hunch));
            for m in [l.from_bind.0, l.from_bind.1, l.from_after.0, l.from_after.1] {
                assert!(m > 0.0 && m <= std::f32::consts::PI);
            }
        }
    }

    fn put(dir: &Path, sub: &str, c: &wscl::Clip) {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
        std::fs::write(
            dir.join(sub).join(format!("{}.wsanim", c.name)),
            wscl::text::write(c),
        )
        .unwrap();
    }

    const MANIFEST: &str = "\
monster kobold chr kobold_reference npc 206000 hp 203 poise 0 scale 0.6180
souls kobold 150
attack small_1 body_attack_01 judge 100 phys 138 poise 10 level 1 radius 0.30 radius2 0.00 stop 0.00 knock 0.45
attack big body_attack_03 judge 120 phys 195 poise 20 level 2 radius 0.30 radius2 0.00 stop 0.00 knock 1.00
react 1 body_reaction_01 push -0.300 0.050
react 2 body_reaction_08 push -0.500 0.000
death body_death_03 push -0.600 0.100
death body_death_05 push 0.200 0.250
think near 2.0 mid 5.0 far 10.0 out 20.0
reach 53 45
";

    fn kit_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wow-souls-monster-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (c, hit) in [
            ("body_attack_01", Some((0.3, 0.4))),
            ("body_attack_03", Some((0.5, 0.6))),
            ("body_reaction_01", None),
            ("body_reaction_08", None),
            ("body_death_03", None),
            ("body_death_05", None),
        ] {
            put(&dir, "kobold", &clip(c, 4, hit));
            put(&dir, "kobold_reference", &clip(c, 6, hit));
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
        assert_eq!(k.deaths[1].wow.name, "body_death_05");
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
    fn a_moves_ground_shock_and_blast_are_read_from_their_lines() {
        let dir = kit_dir("shock");
        let text = format!(
            "{MANIFEST}attack-shock big judge 180 at 2.467 radius 3.00 phys 210 poise 40 level 6 knock 1.60\nattack-blast small_1 at 5.400 radius 7.00 phys 220 poise 300 level 10 knock 2.00\n"
        );
        let kit = parse(&text, &dir).unwrap();
        let shock = kit.attacks[1].shock.unwrap();
        assert_eq!((shock.at, shock.radius, shock.level), (2.467, 3.0, 6));
        assert_eq!(shock.damage, (210.0 * DAMAGE_K).round());
        assert_eq!(shock.knock, 1.6);
        let blast = kit.attacks[0].blast.unwrap();
        assert_eq!((blast.at, blast.radius, blast.level), (5.4, 7.0, 10));
        assert_eq!((shock.poise, blast.poise), (40.0, 300.0));
        assert!(kit.attacks[0].shock.is_none() && kit.attacks[1].blast.is_none());
        // A line for a move the kit lacks, or a malformed one, makes the kit unusable.
        for bad in [
            "attack-shock missing judge 180 at 1 radius 1 phys 1 poise 1 level 1 knock 1\n",
            "attack-shock big judge 180 at -1 radius 1 phys 1 poise 1 level 1 knock 1\n",
            "attack-blast big at 1 radius NaN phys 1 poise 1 level 1 knock 1\n",
            // The old line without its poise.
            "attack-blast big at 1 radius 1 phys 1 level 1 knock 1\n",
        ] {
            assert!(parse(&format!("{MANIFEST}{bad}"), &dir).is_err(), "{bad}");
        }
    }

    #[test]
    fn mixed_attacks_keep_their_scale_foot_segment_and_authored_weapon_rotation() {
        let dir = kit_dir("mixed");
        let mut foot = crate::motion_clip::read_clip(&dir, "kobold", "body_attack_01").unwrap();
        foot.driven[1] = false;
        put(&dir, "kobold", &foot);
        let text = format!("{MANIFEST}attack-source big chr human_reference weapon 701000 variation 3000 scale 1.25\nattack-reach small_1 bones 0 1\nattack-grip big hand 1 weapon 2 axis 0 1 0 blade 0 1 0\n");
        let mut kit = parse(&text, &dir).unwrap();
        assert_eq!(kit.scale, 0.618);
        assert_eq!(
            kit.attacks[0].reach,
            Some(Reach {
                hand: 0,
                forearm: 1
            })
        );
        assert_eq!(kit.attacks[0].source_scale, None);
        assert_eq!(kit.attacks[1].source_scale, Some(1.25));
        assert_eq!(kit.attacks[1].reach, None);
        let attack = &mut kit.attacks[1];
        let hand = Quat::from_rotation_x(0.7);
        let source_weapon = Quat::from_rotation_y(0.9);
        for frame in &mut attack.wow.frames {
            frame[1].r = hand.to_array();
        }
        for frame in &mut attack.reference.frames {
            frame[2].r = source_weapon.to_array();
        }
        let fitted = attack.animated_grip(0.5, [Vec3::X, Vec3::ZERO]).unwrap();
        assert!((hand * fitted * Vec3::X - source_weapon * Vec3::Y).length() < 1e-5);
        assert!(kit.attacks[0]
            .animated_grip(0.5, [Vec3::X, Vec3::ZERO])
            .is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn malformed_or_unmatched_attack_metadata_is_refused() {
        let dir = kit_dir("bad-mixed");
        for extra in [
            "attack-source big chr human_reference weapon 701000 variation 3000 scale NaN\n",
            "attack-source big chr human_reference weapon 701000 variation 3000 scale 0\n",
            "attack-source missing chr human_reference weapon 701000 variation 3000 scale 1\n",
            "attack-reach small_1 bones 1 1\n",
            "attack-reach small_1 bones 0 4\n",
            "attack-reach missing bones 0 1\n",
            "attack-grip big hand 4 weapon 2 axis 0 1 0 blade 0 1 0\n",
            "attack-grip big hand 1 weapon 6 axis 0 1 0 blade 0 1 0\n",
            "attack-grip big hand 1 weapon 2 axis 0 0 0 blade 0 1 0\n",
            "attack-grip big hand 1 weapon 2 axis 0 1 0 blade 0 NaN 0\n",
            "attack-reach big bones 0 1\nattack-reach big bones 0 2\n",
        ] {
            assert!(
                parse(&format!("{MANIFEST}{extra}"), &dir).is_err(),
                "accepted {extra}"
            );
        }
        let legacy = parse(MANIFEST, &dir).unwrap();
        assert!(legacy
            .attacks
            .iter()
            .all(|a| a.reach.is_none() && a.source_scale.is_none() && a.grip.is_none()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_manifest_of_two_creatures_loads_each_kit_by_name() {
        let dir = kit_dir("roster");
        for (c, hit) in [
            ("body_attack_01", Some((0.3, 0.4))),
            ("body_attack_03", Some((0.5, 0.6))),
            ("body_reaction_01", None),
            ("body_reaction_08", None),
            ("body_death_03", None),
            ("body_death_05", None),
        ] {
            put(&dir, "wolf", &clip(c, 7, hit));
            put(&dir, "wolf_reference", &clip(c, 9, hit));
        }
        let wolf = MANIFEST
            .replace(
                "monster kobold chr kobold_reference",
                "monster wolf chr wolf_reference",
            )
            .replace("souls kobold 150", "souls wolf 80");
        // The player's reaction lines after the last kit are not the kit's.
        let text = format!(
            "{MANIFEST}{wolf}preact s body_reaction_01 something else\nmax-error-deg 1.0\n"
        );
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
    fn targets_of_one_source_load_their_own_scaled_references_including_parried() {
        let dir = kit_dir("target-references");
        let target = MANIFEST
            .replace("monster kobold", "monster direwolf")
            .replace("scale 0.6180", "scale 1.1795")
            + "reference direwolf_reference\nparried body_death_03\n";
        for (name, hit) in [
            ("body_attack_01", Some((0.3, 0.4))),
            ("body_attack_03", Some((0.5, 0.6))),
            ("body_reaction_01", None),
            ("body_reaction_08", None),
            ("body_death_03", None),
            ("body_death_05", None),
        ] {
            put(&dir, "direwolf", &clip(name, 61, hit));
            let mut reference = clip(name, 6, hit);
            reference.frames[0][0].t = [0.0, 2.0, 0.0];
            put(&dir, "direwolf_reference", &reference);
        }
        let legacy = format!("{MANIFEST}parried body_death_03\n");
        let roster = parse_roster(&format!("{legacy}{target}"), &dir).unwrap();
        let old = roster.get("kobold").unwrap();
        let new = roster.get("direwolf").unwrap();
        for id in [
            MonsterClip::Attack(0),
            MonsterClip::React(0),
            MonsterClip::Death(0),
            MonsterClip::Parried,
        ] {
            let (old_wow, old_ds) = old.clips(id).unwrap();
            let (new_wow, new_ds) = new.clips(id).unwrap();
            assert_eq!((old_wow.bones(), new_wow.bones()), (4, 61));
            assert_eq!(old_ds.frames[0][0].t, [0.0; 3]);
            assert_eq!(new_ds.frames[0][0].t, [0.0, 2.0, 0.0]);
        }
        let bad = target.replace("reference direwolf_reference", "reference");
        assert!(parse(&bad, &dir)
            .err()
            .unwrap()
            .contains("bad monster line"));
        std::fs::remove_dir_all(&dir).ok();
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
                "body_reaction_01 has no hit window",
                MANIFEST.replace(
                    "attack small_1 body_attack_01",
                    "attack small_1 body_reaction_01",
                ),
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
        put(&dir, "kobold", &clip("body_reaction_08", 5, None));
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
    fn a_creature_chained_by_quick_blows_shrugs_the_third_off_and_can_swing_back() {
        let mut gnoll = Poise::new(0.0);
        assert!(gnoll.hit_chained(8.0) && gnoll.hit_chained(8.0));
        assert!(!gnoll.hit_chained(8.0), "the chain is full");
        gnoll.tick(1.0);
        assert!(!gnoll.hit_chained(8.0), "still shrugging blows off");
        gnoll.tick(CHAIN_ARMOR);
        assert!(gnoll.hit_chained(8.0), "the armour is spent");
        // Blows far apart never make a chain.
        let mut slow = Poise::new(0.0);
        for _ in 0..5 {
            assert!(slow.hit_chained(8.0));
            slow.tick(CHAIN_WINDOW);
        }
        // The player's own poise has no chain.
        let mut player = Poise::new(0.0);
        assert!((0..5).all(|_| player.hit(8.0)));
    }

    #[test]
    fn a_blow_from_the_front_takes_the_clip_that_throws_the_head_back() {
        let dir = kit_dir("react");
        let k = parse(MANIFEST, &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        // Level 1 has the one clip (body_reaction_01, push back); level 2 the other.
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
        let mut c = clip("body_death_03", 2, None);
        c.driven = vec![true, false];
        c.frames[10][0].t = [0.0, 5.0, 0.0];
        let (t, _, _) = crate::motion_clip::sample(&c, 99.0, 0);
        assert_eq!(t.y, 5.0, "clamped to the last frame: the pose holds");
        assert!(!c.driven[1]);
    }

    #[test]
    fn a_move_with_two_hit_windows_keeps_both_and_the_first_leads() {
        let mut c = clip("body_attack_03", 2, Some((0.2, 0.3)));
        c.hits.push(wscl::HitWindow {
            judge: 1,
            start: 0.6,
            end: 0.7,
        });
        let dir = std::env::temp_dir().join(format!("wscl-two-windows-{}", std::process::id()));
        put(&dir, "garrick", &c);
        put(&dir, "defias_brawler_reference", &c);
        put(&dir, "garrick", &clip("body_reaction_01", 2, None));
        put(
            &dir,
            "defias_brawler_reference",
            &clip("body_reaction_01", 2, None),
        );
        put(&dir, "garrick", &clip("body_death_05", 2, None));
        put(
            &dir,
            "defias_brawler_reference",
            &clip("body_death_05", 2, None),
        );
        let text = "monster garrick chr defias_brawler_reference npc 240000 hp 110 poise 0 scale 1.1
                    attack double body_attack_03 judge 120 phys 140 poise 20 level 1 radius 0.40 radius2 0.00 stop 0.00 knock 0.45
                    react 1 body_reaction_01 push -0.3 0.0
                    death body_death_05 push -0.6 0.0
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
