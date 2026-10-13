//! The Dark Souls clips from `cache/dsr/` (written by `wow-souls-dsr extract`): the roll and the
//! straight sword's moves. They load at startup when the cache is present and fresh, are sampled
//! per frame onto the avatar's rig in benilla's `PosePost` window, and give the roll its
//! `EvadeSpec` and every move its `SwingSpec` (length, windows, price) from their own data.
//! Without a cache the procedural roll and swing stay.
//!
//! A clip blends over the rig's own pose (the `AnimationPlayer`'s clip, evaluated this frame
//! before `PosePost`): in over [`BLEND_IN`] from its start, and out over [`BLEND_OUT`] once the
//! action lets go of the body, holding the pose it ended on. A clip that takes over from another
//! (a chained move, a chained roll) leaves the old end pose fading under it.
//!
//! While clips pose the rig, the held item's blade is turned onto the Dark Souls blade axis
//! ([`grip_rotation`]): the retarget carries the hand's rotation, not the sword in it.

use std::path::{Path, PathBuf};

use benilla_world::rig_anim::{PosePost, RigPose};
use bevy::prelude::*;

use crate::actor::held::HeldBuilt;
use crate::avatar::Avatar;
use crate::combat::{
    Action, ActionState, EvadeKind, EvadeSpec, MoveKind, Moveset, SwingSpec, BACKSTEP, ROLL,
};

/// `ChrActionFlag` values that open a cancel window (attack, movement, step).
const CANCEL_FLAGS: [i32; 3] = [4, 11, 26];

/// Seconds of a clip over which its weight rises from the rig's pose to the clip's.
pub const BLEND_IN: f32 = 0.1;
/// Real seconds after the action ends over which the clip's last pose fades back to the rig's.
pub const BLEND_OUT: f32 = 0.2;

/// One bone of a clip at time `t`: translation, rotation, scale (linear / slerp between frames).
pub fn sample(clip: &wscl::Clip, t: f32, bone: usize) -> (Vec3, Quat, Vec3) {
    let last = clip.frames.len().saturating_sub(1);
    let f = (t.max(0.0) * clip.fps).min(last as f32);
    let i = (f as usize).min(last.saturating_sub(1));
    let a = if last == 0 { 0.0 } else { f - i as f32 };
    let (p, q) = (&clip.frames[i][bone], &clip.frames[(i + 1).min(last)][bone]);
    let qa = Quat::from_array(p.r).normalize();
    let qb = Quat::from_array(q.r).normalize();
    (
        Vec3::from_array(p.t).lerp(Vec3::from_array(q.t), a),
        qa.slerp(qb, a),
        Vec3::from_array(p.s).lerp(Vec3::from_array(q.s), a),
    )
}

/// The root motion's forward distance (yd, along Bevy -Z of the model) covered by `t`, in model
/// units: the body's world distance is this times the model's scale.
pub fn progress(clip: &wscl::Clip, t: f32) -> f32 {
    let n = clip.root_motion.len();
    if n == 0 {
        return 0.0;
    }
    let f = (t.max(0.0) * clip.fps).min((n - 1) as f32);
    let i = (f as usize).min(n.saturating_sub(2));
    let a = if n == 1 { 0.0 } else { f - i as f32 };
    let z0 = clip.root_motion[0][2];
    let z = clip.root_motion[i][2]
        + (clip.root_motion[(i + 1).min(n - 1)][2] - clip.root_motion[i][2]) * a;
    -(z - z0)
}

/// The root motion's `(x, z)` offset (yd, the body's frame: forward -Z, left -X) covered by `t`.
pub fn root_offset(clip: &wscl::Clip, t: f32) -> Vec2 {
    let n = clip.root_motion.len();
    if n == 0 {
        return Vec2::ZERO;
    }
    let f = (t.max(0.0) * clip.fps).min((n - 1) as f32);
    let i = (f as usize).min(n.saturating_sub(2));
    let a = if n == 1 { 0.0 } else { f - i as f32 };
    let at = |k: usize| Vec2::new(clip.root_motion[k][0], clip.root_motion[k][2]);
    let (p, q) = (at(i), at((i + 1).min(n - 1)));
    p.lerp(q, a) - at(0)
}

/// The roll's evade shape from its clip; the procedural roll's cost.
pub fn roll_spec(clip: &wscl::Clip) -> EvadeSpec {
    let duration = clip.duration();
    let iframes = clip
        .windows(wscl::FLAG_INVINCIBLE)
        .next()
        .unwrap_or(ROLL.iframes);
    let cancel_from = clip
        .flags
        .iter()
        .filter(|w| CANCEL_FLAGS.contains(&w.flag))
        .map(|w| w.start)
        .fold(duration, f32::min);
    EvadeSpec {
        duration,
        travel: duration,
        distance: progress(clip, duration),
        iframes,
        cancel_from,
        cost: ROLL.cost,
    }
}

/// The backstep's evade shape from its clip: the roll's reading of the windows (length, the
/// invulnerable frames, the first cancel), the distance it travels backwards as a positive number,
/// and the backstep's own price.
pub fn backstep_spec(clip: &wscl::Clip) -> EvadeSpec {
    EvadeSpec {
        distance: progress(clip, clip.duration()).abs(),
        cost: BACKSTEP.cost,
        ..roll_spec(clip)
    }
}

/// A move's timing and price from its clip's TAE windows: the hit window, and the three cancel
/// windows (attack, evade, movement), whose first starts are the times they open.
#[allow(clippy::too_many_arguments)]
pub fn swing_spec(
    clip: &wscl::Clip,
    cost: f32,
    mult: f32,
    level: u8,
    poise: f32,
    stop: f32,
    knock: f32,
    radius: f32,
) -> Result<SwingSpec, String> {
    let opens = |flag: i32, what: &str| {
        clip.windows(flag)
            .map(|(start, _)| start)
            .min_by(f32::total_cmp)
            .ok_or_else(|| format!("{} has no {what} window (flag {flag})", clip.name))
    };
    let hit = clip
        .hits
        .first()
        .map(|h| (h.start, h.end))
        .ok_or_else(|| format!("{} has no hit window", clip.name))?;
    Ok(SwingSpec {
        duration: clip.duration(),
        hit,
        chain_from: opens(wscl::FLAG_CHAIN, "chain")?,
        evade_from: opens(wscl::FLAG_EVADE, "evade")?,
        move_from: opens(wscl::FLAG_MOVE, "movement")?,
        cost,
        mult,
        level,
        poise,
        stop,
        knock,
        radius: radius * crate::shove::METRES_TO_YARDS,
    })
}

/// The clip's weight over the rig's pose `t` seconds into the clip: rising over [`BLEND_IN`],
/// then, `since_end` real seconds after the action ended, falling to 0 over [`BLEND_OUT`].
pub fn blend_weight(t: f32, since_end: Option<f32>) -> f32 {
    let rise = (t / BLEND_IN).clamp(0.0, 1.0);
    let fall = since_end.map_or(1.0, |s| (1.0 - s / BLEND_OUT).clamp(0.0, 1.0));
    rise * fall
}

/// Which cached clip a layer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipId {
    Stance,
    Roll,
    Move(MoveKind),
    /// A reaction of the player's kit ([`crate::reaction`]), by index.
    React(usize),
    /// A death of the kit, by index: a knockdown held on the frame where it lies.
    Death(usize),
    Defense(crate::defense::Kind),
    Interaction(crate::loot::interaction::Kind),
    /// A gait clip of the held weapon's motion set ([`crate::dsloco`]); the gait poses the body
    /// itself, so this is only named by the side-by-side.
    Loco(crate::dsloco::Role),
    /// The Dark Souls backstep ([`DsRoll::back`]).
    Backstep,
}

/// The loaded Dark Souls roll.
#[derive(Resource)]
pub struct DsRoll {
    /// On the WoW HumanMale skeleton, bone-local.
    pub wow: wscl::Clip,
    /// On the DS skeleton, model space, Bevy axes, WoW scale (for the side-by-side).
    pub ds: wscl::Clip,
    pub spec: EvadeSpec,
    /// The backstep, when the cache has it (an older one does not: the WoW sway stays).
    pub back: Option<EvadeClip>,
}

/// A Dark Souls evade clip with the shape it gives the action machine.
pub struct EvadeClip {
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    pub spec: EvadeSpec,
}

/// One cached move: its clips and its timing and price.
pub struct MoveClip {
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    pub spec: SwingSpec,
    pub hits: Vec<SwingSpec>,
}

/// Where the Dark Souls blade points, from the cache: its axis in the WoW right-hand bone's frame
/// (what a held item's blade is turned onto), and in the DS weapon bone's own axes (Bevy axes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grip {
    pub hand: Vec3,
    pub weapon: Vec3,
}

/// The loaded straight sword.
#[derive(Resource)]
pub struct DsMoves {
    pub family: String,
    pub base_damage: f32,
    pub two_handed: bool,
    pub projectile: Option<ProjectileSpec>,
    pub idle: Option<(wscl::Clip, wscl::Clip)>,
    pub guards: Vec<crate::defense::Clip>,
    pub guard: Option<(f32, f32, bool)>,
    pub grip_bones: Option<(usize, usize)>,
    clips: Vec<(MoveKind, MoveClip)>,
    /// The six specs, as the action machine reads them.
    pub moveset: Moveset,
    pub grip: Grip,
    /// The gait of this weapon's motion set, attached by [`crate::dsloco`] once the cache is read.
    pub loco: Option<std::sync::Arc<crate::dsloco::Kit>>,
}

#[derive(Clone, Copy, Debug)]
pub struct ProjectileSpec {
    pub release: f32,
    pub speed: f32,
    pub gravity: f32,
    pub damage: f32,
}

impl DsMoves {
    /// Weapon orientation can change inside the hand (notably dagger heavies).
    pub fn animated_grip(&self, id: ClipId, t: f32, blade: [Vec3; 2]) -> Option<Quat> {
        let (hand, weapon) = self.grip_bones?;
        let (wow, ds) = match id {
            ClipId::Move(kind) => {
                let c = self.clip(kind)?;
                (&c.wow, &c.ds)
            }
            ClipId::Stance => {
                let c = self.idle.as_ref()?;
                (&c.0, &c.1)
            }
            ClipId::Defense(kind) => {
                let c = self.guards.iter().find(|c| c.kind == kind)?;
                (&c.wow, &c.ds)
            }
            _ => return None,
        };
        if hand >= wow.bones() || weapon >= ds.bones() {
            return None;
        }
        let mut q = Quat::IDENTITY;
        let mut bone = Some(hand);
        while let Some(i) = bone {
            q = sample(wow, t, i).1 * q;
            bone = usize::try_from(wow.parents[i]).ok();
        }
        // Carry the complete authored weapon rotation, including twist around its long axis.
        let mount = grip_rotation(blade, self.grip.weapon);
        Some(q.inverse() * sample(ds, t, weapon).1 * mount)
    }

    pub fn clip(&self, kind: MoveKind) -> Option<&MoveClip> {
        self.clips.iter().find(|(k, _)| *k == kind).map(|(_, c)| c)
    }
}

/// A clip's two files: on the WoW rig, and on the DS skeleton.
pub fn clips_of<'a>(
    roll: &'a DsRoll,
    moves: &'a DsMoves,
    reacts: Option<&'a crate::reaction::PlayerReacts>,
    defense: Option<&'a crate::defense::Kit>,
    interaction: Option<&'a crate::loot::interaction::Kit>,
    id: ClipId,
) -> Option<(&'a wscl::Clip, &'a wscl::Clip)> {
    match id {
        ClipId::Stance => moves.idle.as_ref().map(|c| (&c.0, &c.1)),
        ClipId::Roll => Some((&roll.wow, &roll.ds)),
        ClipId::Move(k) => moves.clip(k).map(|m| (&m.wow, &m.ds)),
        ClipId::React(i) => reacts?.reacts.get(i).map(|r| (&r.wow, &r.ds)),
        ClipId::Death(i) => reacts?.death_clip(i).map(|(r, _)| (&r.wow, &r.ds)),
        ClipId::Defense(k) => defense?.clip(k).map(|c| (&c.wow, &c.ds)),
        ClipId::Interaction(k) => {
            let c = interaction?.clip(k);
            Some((&c.wow, &c.ds))
        }
        ClipId::Loco(role) => moves.loco.as_deref()?.get(role).map(|c| (&c.wow, &c.ds)),
        ClipId::Backstep => roll.back.as_ref().map(|b| (&b.wow, &b.ds)),
    }
}

/// The rotation, in the hand's frame, that turns a held item's blade (its `$WTB` to `$WTT`
/// markers, `[top, bottom]`) onto the Dark Souls blade axis `hand`.
pub fn grip_rotation(blade: [Vec3; 2], hand: Vec3) -> Quat {
    let from = (blade[0] - blade[1]).normalize_or_zero();
    let to = hand.normalize_or_zero();
    if from == Vec3::ZERO || to == Vec3::ZERO {
        return Quat::IDENTITY;
    }
    Quat::from_rotation_arc(from, to)
}

/// The item's rotation `weight` of the way from none to `fit`.
pub fn item_rotation(fit: Quat, weight: f32) -> Quat {
    Quat::IDENTITY.slerp(fit, weight.clamp(0.0, 1.0))
}

/// `WOWSOULS_GRIP=rx,ry,rz`: the rotation (degrees about the hand's X, Y, Z) of the held item
/// while clips pose the rig, replacing the automatic fit. A tuning aid for the side-by-side.
#[derive(Resource, Debug, Clone, Copy)]
pub struct GripOverride(pub Quat);

pub fn parse_grip_override(v: &str) -> Option<Quat> {
    let n: Vec<f32> = v
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect::<Option<_>>()?;
    let [x, y, z] = n[..] else { return None };
    Some(Quat::from_euler(
        EulerRot::XYZ,
        x.to_radians(),
        y.to_radians(),
        z.to_radians(),
    ))
}

/// The manifest format this game reads (its `wscl-cache` line).
const CACHE_VERSION: &str = "6";

pub(crate) fn cache_dir() -> PathBuf {
    std::env::var("WOWSOULS_DSR_CACHE").map_or_else(|_| PathBuf::from("cache/dsr"), PathBuf::from)
}

/// A `move` line of the manifest.
struct MoveLine {
    kind: MoveKind,
    clip: String,
    stamina: f32,
    mult: f32,
    level: u8,
    poise: f32,
    stop: f32,
    knock: f32,
    /// `hit0_Radius` (DS metres): the blade's thickness.
    radius: f32,
}

/// `move <name> <clip> judge <n> stamina <n> mult <x> level <n> poise <n> stop <s> knock <m> radius <m>`.
fn parse_move(line: &str) -> Result<MoveLine, String> {
    let bad = || format!("bad move line {line:?}");
    let w: Vec<&str> = line.split_whitespace().collect();
    let [_, name, clip, "judge", _, "stamina", stamina, "mult", mult, "level", level, "poise", poise, "stop", stop, "knock", knock, "radius", radius] =
        w[..]
    else {
        return Err(bad());
    };
    Ok(MoveLine {
        kind: MoveKind::from_name(name)
            .ok_or_else(|| format!("the manifest has a move this game does not know: {name}"))?,
        clip: clip.to_string(),
        stamina: stamina.parse().map_err(|_| bad())?,
        mult: mult.parse().map_err(|_| bad())?,
        level: level.parse().map_err(|_| bad())?,
        poise: poise.parse().map_err(|_| bad())?,
        stop: stop.parse().map_err(|_| bad())?,
        knock: knock.parse().map_err(|_| bad())?,
        radius: radius.parse().map_err(|_| bad())?,
    })
}

/// `grip <x> <y> <z> <x> <y> <z>`: the blade axis in the WoW hand bone's frame, then in the DS
/// weapon bone's axes.
fn parse_grip_line(line: &str) -> Result<Grip, String> {
    let n: Vec<f32> = line
        .split_whitespace()
        .skip(1)
        .map(|w| w.parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect::<Option<_>>()
        .ok_or_else(|| format!("bad grip line {line:?}"))?;
    let [hx, hy, hz, wx, wy, wz] = n[..] else {
        return Err(format!("the grip line has {} numbers, expected 6", n.len()));
    };
    Ok(Grip {
        hand: Vec3::new(hx, hy, hz),
        weapon: Vec3::new(wx, wy, wz),
    })
}

/// What the cache holds.
struct Cache {
    roll: DsRoll,
    moves: DsMoves,
}

/// The manifest's text once its versions are this game's and every `source` file is as it was
/// at extraction; `Err` says why the cache is unusable.
pub fn checked_manifest(dir: &Path) -> Result<String, String> {
    let manifest = std::fs::read_to_string(dir.join("manifest.txt"))
        .map_err(|e| format!("no manifest in {} ({e})", dir.display()))?;
    let (mut cache_version, mut extractor) = (false, false);
    for line in manifest.lines() {
        let mut w = line.splitn(4, ' ');
        match (w.next(), w.next(), w.next(), w.next()) {
            (Some("wscl-cache"), Some(v), _, _) => {
                if v != CACHE_VERSION {
                    return Err(format!("cache version {v}, expected {CACHE_VERSION}"));
                }
                cache_version = true;
            }
            (Some("extractor"), Some(v), _, _) => {
                if v != wscl::EXTRACTOR_VERSION.to_string() {
                    return Err(format!(
                        "extractor version {v}, expected {}",
                        wscl::EXTRACTOR_VERSION
                    ));
                }
                extractor = true;
            }
            (Some("source"), Some(len), Some(mtime), Some(path)) => {
                let meta = std::fs::metadata(path).map_err(|e| format!("source {path}: {e}"))?;
                let now = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs().to_string());
                if meta.len().to_string() != len || now.as_deref() != Some(mtime) {
                    return Err(format!("stale: {path} changed since extraction"));
                }
            }
            _ => {}
        }
    }
    if !(cache_version && extractor) {
        return Err("manifest lacks a version".into());
    }
    Ok(manifest)
}

/// One clip file of the cache: `<dir>/<sub>/<name>.wscl`, with at least two frames.
pub fn read_clip(dir: &Path, sub: &str, name: &str) -> Result<wscl::Clip, String> {
    let p = dir.join(sub).join(format!("{name}.wscl"));
    let f = std::fs::File::open(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let clip = wscl::Clip::read(&mut std::io::BufReader::new(f))
        .map_err(|e| format!("{}: {e}", p.display()))?;
    if clip.frames.len() < 2 || !(clip.fps.is_finite() && clip.fps > 0.0) {
        return Err(format!(
            "{}: {} frames at {} fps is no clip",
            p.display(),
            clip.frames.len(),
            clip.fps
        ));
    }
    Ok(clip)
}

/// Reads the manifest and the clips it names; `Err` says why the cache is unusable (missing,
/// stale, another version, incomplete).
fn load(dir: &Path) -> Result<Cache, String> {
    let manifest = checked_manifest(dir)?;
    let mut roll = None;
    for line in manifest.lines() {
        let mut w = line.splitn(4, ' ');
        if let (Some("roll"), Some(name)) = (w.next(), w.next()) {
            roll = Some(name.to_string());
        }
    }
    let name = roll.ok_or("manifest names no roll")?;
    let read = read_clip;
    let wow = read(dir, "human_male", &name)?;
    let ds = read(dir, "c0000", &name)?;
    let spec = roll_spec(&wow);
    let back = manifest
        .lines()
        .find_map(|l| l.strip_prefix("backstep "))
        .map(|name| {
            let name = name.trim();
            let (wow, ds) = (read(dir, "human_male", name)?, read(dir, "c0000", name)?);
            let spec = backstep_spec(&wow);
            Ok::<_, String>(EvadeClip { wow, ds, spec })
        })
        .transpose()?;
    let roll = DsRoll {
        wow,
        ds,
        spec,
        back,
    };

    let moves = load_moves(dir, &manifest, roll.wow.bones())?;
    if let Some(b) = &roll.back {
        if b.wow.bones() != roll.wow.bones() {
            return Err(format!(
                "{}: {} bones, the roll's clip has {}",
                b.wow.name,
                b.wow.bones(),
                roll.wow.bones()
            ));
        }
    }
    Ok(Cache { roll, moves })
}

pub(crate) fn load_moves(dir: &Path, manifest: &str, bones: usize) -> Result<DsMoves, String> {
    let lines = manifest
        .lines()
        .filter(|l| l.starts_with("move "))
        .map(parse_move)
        .collect::<Result<Vec<_>, _>>()?;
    let grip = manifest
        .lines()
        .find(|l| l.starts_with("grip "))
        .map(parse_grip_line)
        .transpose()?;
    let read = read_clip;
    let mut clips = Vec::new();
    for kind in MoveKind::ALL {
        let line = lines
            .iter()
            .find(|l| l.kind == kind)
            .ok_or_else(|| format!("manifest lacks move {}", kind.name()))?;
        let wow = read(dir, "human_male", &line.clip)?;
        let ds = read(dir, "c0000", &line.clip)?;
        if wow.bones() != bones {
            return Err(format!(
                "{}: {} bones, the roll's clip has {}",
                line.clip,
                wow.bones(),
                bones
            ));
        }
        let spec = swing_spec(
            &wow,
            line.stamina,
            line.mult,
            line.level,
            line.poise,
            line.stop,
            line.knock,
            line.radius,
        )?;
        let mut hits = Vec::new();
        for line in manifest.lines().filter(|l| l.starts_with("hit ")) {
            let w: Vec<_> = line.split_whitespace().collect();
            if w.len() != 11 {
                return Err(format!("bad hit {line}"));
            }
            if w[1] != kind.name() {
                continue;
            }
            let n = |i: usize| {
                w[i].parse::<f32>()
                    .ok()
                    .filter(|n| n.is_finite() && *n >= 0.0)
                    .ok_or_else(|| format!("bad hit {line}"))
            };
            let hit = SwingSpec {
                hit: (n(3)?, n(4)?),
                mult: n(5)?,
                level: w[6]
                    .parse::<u8>()
                    .map_err(|_| format!("bad hit level {line}"))?,
                poise: n(7)?,
                stop: n(8)?,
                knock: n(9)?,
                radius: n(10)? * crate::shove::METRES_TO_YARDS,
                ..spec
            };
            if hit.hit.0 >= hit.hit.1 || hit.hit.1 > wow.duration() + 0.001 {
                return Err(format!("hit outside clip {line}"));
            }
            hits.push(hit);
        }
        if hits.is_empty() {
            hits.push(spec);
        }
        clips.push((
            kind,
            MoveClip {
                wow,
                ds,
                spec,
                hits,
            },
        ));
    }
    let grip = grip.ok_or("manifest has no grip line")?;
    let spec = |k: MoveKind| {
        clips
            .iter()
            .find(|(c, _)| *c == k)
            .map(|(_, m)| m.spec)
            .expect("every move was read")
    };
    let moveset = Moveset {
        r1: [
            spec(MoveKind::R1(0)),
            spec(MoveKind::R1(1)),
            spec(MoveKind::R1(2)),
        ],
        r2: Some([spec(MoveKind::R2(0)), spec(MoveKind::R2(1))]),
        roll_attack: Some(spec(MoveKind::RollAttack)),
        ds: true,
        chain_len: manifest
            .lines()
            .find_map(|l| l.strip_prefix("chain ").and_then(|v| v.parse::<u8>().ok()))
            .unwrap_or(3)
            .clamp(1, 3),
    };
    let family = manifest
        .lines()
        .find_map(|l| l.strip_prefix("class "))
        .unwrap_or("straight_sword")
        .to_string();
    let base_damage = manifest
        .lines()
        .find_map(|l| l.strip_prefix("base-damage "))
        .map(|v| {
            let (base, reference) = v.split_once(' ').ok_or("bad base-damage")?;
            let base = base.parse::<f32>().map_err(|_| "bad base-damage")?;
            let reference = reference
                .parse::<f32>()
                .map_err(|_| "bad reference damage")?;
            if !base.is_finite() || !reference.is_finite() || base < 0.0 || reference <= 0.0 {
                return Err("invalid base damage");
            }
            Ok(crate::hits::LIGHT_DAMAGE * base / reference)
        })
        .transpose()?
        .unwrap_or(crate::hits::LIGHT_DAMAGE);
    let grip_bones = manifest
        .lines()
        .find_map(|l| l.strip_prefix("grip-bones "))
        .and_then(|v| {
            let (h, w) = v.split_once(' ')?;
            Some((h.parse().ok()?, w.parse().ok()?))
        });
    let two_handed = manifest.lines().any(|l| l == "two-handed 1");
    let projectile = manifest
        .lines()
        .find_map(|l| l.strip_prefix("projectile "))
        .map(|v| {
            let n = v
                .split_whitespace()
                .map(str::parse::<f32>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "bad projectile numbers".to_string())?;
            if n.len() != 4 || n.iter().any(|v| !v.is_finite() || *v < 0.0) || n[1] == 0.0 {
                return Err("bad projectile profile".to_string());
            }
            Ok(ProjectileSpec {
                release: n[0],
                speed: n[1],
                gravity: n[2],
                damage: n[3],
            })
        })
        .transpose()?;
    let idle = manifest
        .lines()
        .find_map(|l| l.strip_prefix("idle "))
        .map(|n| Ok::<_, String>((read(dir, "human_male", n)?, read(dir, "c0000", n)?)))
        .transpose()?;
    let guard = manifest
        .lines()
        .find_map(|l| l.strip_prefix("guard "))
        .map(|v| {
            let n = v
                .split_whitespace()
                .map(str::parse::<f32>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "bad guard".to_string())?;
            if n.len() != 3
                || n.iter().any(|v| !v.is_finite() || *v < 0.0)
                || n[0] > 1.0
                || n[1] > 100.0
            {
                return Err("bad guard".to_string());
            }
            Ok((n[0], n[1], n[2] == 1.0))
        })
        .transpose()?;
    let mut guards = Vec::new();
    for line in manifest
        .lines()
        .filter_map(|l| l.strip_prefix("guardclip "))
    {
        let (name, clip) = line.split_once(' ').ok_or("bad guardclip")?;
        use crate::defense::Kind;
        let kind = match name {
            "raise" => Kind::Raise,
            "hold" => Kind::Hold,
            "lower" => Kind::Lower,
            _ => return Err("bad guard kind".into()),
        };
        let wow = read(dir, "human_male", clip)?;
        let ds = read(dir, "c0000", clip)?;
        let spec = crate::defense::spec(&wow, kind, 0.0)?;
        guards.push(crate::defense::Clip {
            kind,
            wow,
            ds,
            spec,
            push: Vec2::ZERO,
        });
    }
    Ok(DsMoves {
        base_damage,
        clips,
        moveset,
        grip,
        family,
        grip_bones,
        two_handed,
        projectile,
        idle,
        guards,
        guard,
        loco: None,
    })
}

pub(crate) fn startup(mut commands: Commands) {
    match load(&cache_dir()) {
        Ok(c) => {
            info!(
                "dsclip: Dark Souls roll {} ({:.3} s, {:.2} yd, invulnerable {:.3}-{:.3} s, cancel from {:.3} s)",
                c.roll.wow.name,
                c.roll.spec.duration,
                c.roll.spec.distance,
                c.roll.spec.iframes.0,
                c.roll.spec.iframes.1,
                c.roll.spec.cancel_from
            );
            for kind in MoveKind::ALL {
                if let Some(m) = c.moves.clip(kind) {
                    let s = &m.spec;
                    info!(
                        "dsclip: {} {} ({:.3} s, hit {:.3}-{:.3}, chain {:.3}, evade {:.3}, move {:.3}, {:.2} yd, stamina {:.0}, x{:.2})",
                        kind.name(),
                        m.wow.name,
                        s.duration,
                        s.hit.0,
                        s.hit.1,
                        s.chain_from,
                        s.evade_from,
                        s.move_from,
                        progress(&m.wow, s.duration),
                        s.cost,
                        s.mult
                    );
                }
            }
            commands.insert_resource(c.roll);
            commands.insert_resource(c.moves);
        }
        Err(why) => {
            warn!("dsclip: no Dark Souls clips ({why}); the procedural roll and swing stay");
        }
    }
}

/// The clip the avatar's action plays and its time: the Dark Souls roll or a Dark Souls move;
/// `None` for anything else (including the procedural roll and swing).
fn playing(action: &Action) -> Option<(ClipId, f32)> {
    match action.state {
        ActionState::Evade(e) if e.ds => Some((
            match e.kind {
                EvadeKind::Roll => ClipId::Roll,
                EvadeKind::Backstep => ClipId::Backstep,
            },
            e.t,
        )),
        ActionState::Attack(s) if s.ds => Some((ClipId::Move(s.kind), s.t)),
        ActionState::Reel(r) => Some((ClipId::React(r.clip), r.t)),
        ActionState::Defend(d) => Some((ClipId::Defense(d.kind), d.time())),
        ActionState::Dead => action.death.map(|d| (ClipId::Death(d), action.dead_t)),
        _ => None,
    }
}

/// The clip last shown and the one fading out.
#[derive(Debug)]
pub struct Fade<K> {
    /// The running clip and its time, as of the last frame.
    last: Option<(K, f32)>,
    /// After it ended: the clip, the time it ended on, and the real seconds since.
    out: Option<(K, f32, f32)>,
}

impl<K> Default for Fade<K> {
    fn default() -> Self {
        Fade {
            last: None,
            out: None,
        }
    }
}

/// One clip layer to blend over the rig: which clip, its time, its weight.
pub type Layer<K> = (K, f32, f32);

/// The clip layers to blend over the rig this frame, in order: the last clip's end pose fading
/// out, then the running clip (`now`: its identity and time) blending in. A clip that ends, is
/// replaced by another clip, or restarts its clock (a chained roll) leaves its end pose fading
/// under the new one, so the clips never let go of the rig between two moves. `dt` is real
/// seconds.
pub fn layers<K: Copy + PartialEq>(
    fade: &mut Fade<K>,
    now: Option<(K, f32)>,
    dt: f32,
) -> [Option<Layer<K>>; 2] {
    if let Some((last_clip, last_t)) = fade.last {
        if now.is_none_or(|(clip, t)| clip != last_clip || t < last_t) {
            fade.out = Some((last_clip, last_t, 0.0));
            fade.last = None;
        }
    }
    let out = fade.out.as_mut().and_then(|(clip, t, since)| {
        let w = blend_weight(*t, Some(*since));
        *since += dt;
        (w > 0.0).then_some((*clip, *t, w))
    });
    if out.is_none() {
        fade.out = None;
    }
    let inn = now.map(|(clip, t)| {
        fade.last = Some((clip, t));
        (clip, t, blend_weight(t, None))
    });
    [out, inn]
}

/// A passive stance is one continuous action, even when its breathing clip loops.
/// Only the sample phase wraps; passing wrapped time to `layers` restarts the blend
/// every two seconds and briefly exposes the underlying WoW hand pose.
#[derive(Default)]
struct RestPose {
    elapsed: Option<f32>,
    legs: f32,
    /// How much of the carry pose (spine, arms, hands) holds: 1 standing and walking, 0 at a run, where
    /// WoW's own run clip swings the arms (a frozen upper body over running legs looked absurd).
    carry: f32,
}

/// The speeds (yd/s) up to which the carry pose holds in full and from which it is gone: above the
/// walk (2.5) and below the locked-on strafe (4.5) and the run (7).
const CARRY_FULL: f32 = 3.0;
const CARRY_GONE: f32 = 4.4;

/// The carry pose's share at body speed `speed`: whole while standing and walking, none from a fast
/// walk, a straight ramp between.
fn carry_share(speed: f32) -> f32 {
    ((CARRY_GONE - speed) / (CARRY_GONE - CARRY_FULL)).clamp(0.0, 1.0)
}

impl RestPose {
    /// Eases the carry pose toward its share for `speed`, as fast as the legs ease.
    fn ease_carry(&mut self, speed: f32, dt: f32) {
        let target = carry_share(speed);
        self.carry += (target - self.carry).clamp(-dt / BLEND_OUT, dt / BLEND_OUT);
    }

    fn tick(&mut self, active: bool, moving: bool, dt: f32) -> Option<(ClipId, f32)> {
        self.elapsed = active.then(|| self.elapsed.map_or(0.0, |t| t + dt));
        let target = if moving { 0.0 } else { 1.0 };
        self.legs += (target - self.legs).clamp(-dt / BLEND_OUT, dt / BLEND_OUT);
        self.elapsed.map(|t| (ClipId::Stance, t))
    }
}

fn stance_phase(layers: &mut [Option<Layer<ClipId>>; 2], duration: f32) {
    for (id, t, _) in layers.iter_mut().flatten() {
        if *id == ClipId::Stance {
            *t = t.rem_euclid(duration);
        }
    }
}

fn carry_bone(i: usize) -> bool {
    // HumanMale spine, shoulders, arms and hands, matching the retarget map.
    [20, 22, 48, 64, 49, 65, 78, 95, 50, 66, 79, 97].contains(&i)
}

/// While the avatar plays a Dark Souls clip (or the side-by-side runs), the clip owns the driven
/// bones, blended over the rig's pose at its ends, and the held blade is turned onto the Dark
/// Souls blade axis by as much as the clips hold the rig. A clip that does not fit the rig is
/// dropped, and the procedural roll and swing take over.
#[allow(clippy::too_many_arguments)]
fn pose(
    mut commands: Commands,
    real: Res<Time<bevy::time::Real>>,
    time: Res<Time>,
    roll: Option<Res<DsRoll>>,
    moves: Option<Res<DsMoves>>,
    reacts: Option<Res<crate::reaction::PlayerReacts>>,
    defense: Option<Res<crate::defense::Kit>>,
    interaction: Option<Res<crate::loot::interaction::Kit>>,
    interacting: Res<crate::loot::interaction::Active>,
    view: Option<Res<crate::dsview::DsView>>,
    tune: Option<Res<GripOverride>>,
    mut avatar: Query<
        (
            &Action,
            &mut RigPose,
            Option<&HeldBuilt>,
            &crate::motor::Motor,
            Option<&crate::dsloco::DsGait>,
        ),
        With<Avatar>,
    >,
    mut items: Query<&mut Transform, Without<Avatar>>,
    mut fade: Local<Fade<ClipId>>,
    mut rest: Local<RestPose>,
    mut told: Local<bool>,
) {
    let (Some(roll), Some(moves), Ok((action, mut pose, held, motor, gait))) =
        (roll, moves, avatar.single_mut())
    else {
        return;
    };
    if moves.is_changed() {
        *fade = Fade::default();
        *rest = RestPose::default();
    }
    // With the Dark Souls gait in use ([`crate::dsloco`], the base layer under everything here)
    // the legs and the carry are the gait's: the stance gives way to it by its weight, and no
    // longer borrows the WoW legs or hands the arms back to WoW's run clip.
    let gait_kit = gait.is_some_and(|g| g.kit);
    let gait_weight = gait.map_or(0.0, |g| g.weight.clamp(0.0, 1.0));
    let action_clip = playing(action).or_else(|| {
        interacting
            .playing()
            .map(|(k, t)| (ClipId::Interaction(k), t))
    });
    let stance = rest.tick(
        action_clip.is_none()
            && matches!(action.state, ActionState::Idle)
            && motor.grounded
            && moves.idle.is_some(),
        motor.velocity.xz().length() > 0.1 && !gait_kit,
        time.delta_secs(),
    );
    let mut layers = match view.as_deref().and_then(crate::dsview::DsView::playing) {
        Some((clip, t)) => [Some((clip, t, 1.0)), None],
        None => layers(&mut fade, action_clip.or(stance), real.delta_secs()),
    };
    if let Some((idle, _)) = &moves.idle {
        stance_phase(&mut layers, idle.duration());
    }
    // Without a gait of its own a run hands the arms back to WoW's run clip: the stance fades
    // with the speed. With one, the stance gives way to the gait instead.
    rest.ease_carry(
        if gait_kit {
            0.0
        } else {
            motor.velocity.xz().length()
        },
        time.delta_secs(),
    );
    for (id, _, w) in layers.iter_mut().flatten() {
        if *id == ClipId::Stance {
            *w *= rest.carry * (1.0 - gait_weight);
        }
    }
    // The held blade onto the Dark Souls blade axis, as far as the clips hold the rig.
    if let Some((held, blade)) = held.and_then(|h| {
        // WoW firearms/crossbows omit trail markers. Their barrel follows model -Z;
        // use that direction for mounting only, never manufacture a melee hitbox.
        h.blade
            .or_else(|| {
                matches!(moves.family.as_str(), "crossbow" | "gun")
                    .then_some([Vec3::NEG_Z, Vec3::ZERO])
            })
            .map(|b| (h, b))
    }) {
        let tuned = tune.is_some();
        let fit = tune.map_or_else(|| grip_rotation(blade, moves.grip.hand), |t| t.0);
        if !*told {
            *told = true;
            info!(
                "dsclip: the held blade turns {:.0} deg onto the Dark Souls blade axis ({})",
                fit.to_axis_angle().1.to_degrees(),
                if tuned { "WOWSOULS_GRIP" } else { "fitted" }
            );
        }
        // The gait is the base layer: the blade sits in its grip as far as the gait owns the pose.
        let gait_layer = (gait_weight > 0.0).then_some((
            ClipId::Loco(crate::dsloco::Role::Walk),
            0.0,
            gait_weight,
        ));
        let want = gait_layer
            .into_iter()
            .chain(layers.iter().flatten().copied())
            .fold(Quat::IDENTITY, |q, (id, t, w)| {
                let fit = if tuned {
                    fit
                } else {
                    moves.animated_grip(id, t, blade).unwrap_or_else(|| {
                        grip_rotation(blade, clip_grip(id, &moves, defense.as_deref()).hand)
                    })
                };
                if q == Quat::IDENTITY {
                    item_rotation(fit, w)
                } else {
                    q.slerp(fit, w)
                }
            });
        if let Ok(mut item) = items.get_mut(held.root) {
            if item.rotation != want {
                item.rotation = want;
            }
        }
    }
    if layers.iter().all(Option::is_none) {
        return;
    }
    if roll.wow.bones() != pose.locals.len() {
        warn!(
            "dsclip: clips have {} bones, the rig {}; the procedural roll and swing take over",
            roll.wow.bones(),
            pose.locals.len()
        );
        commands.remove_resource::<DsRoll>();
        commands.remove_resource::<DsMoves>();
        return;
    }
    for (clip, t, w) in layers.into_iter().flatten() {
        if w <= 0.0 {
            continue;
        }
        let Some((wow, _)) = clips_of(
            &roll,
            &moves,
            reacts.as_deref(),
            defense.as_deref(),
            interaction.as_deref(),
            clip,
        ) else {
            continue;
        };
        if wow.bones() != pose.locals.len() {
            continue;
        }
        // A death holds the frame where the body lies.
        let t = match (clip, reacts.as_deref()) {
            (ClipId::Death(i), Some(k)) => k.deaths.get(i).map_or(t, |d| t.min(d.hold)),
            _ => t,
        };
        for (i, local) in pose.locals.iter_mut().enumerate() {
            if !wow.driven[i] {
                continue;
            }
            // Moving guard keeps WoW's walking legs under the DS upper-body stance.
            // HumanMale's checked spine/arm map (bonemap::HUMAN_MALE); idle guard uses the full pose.
            if matches!(clip, ClipId::Defense(k) if k.guarding() || k == crate::defense::Kind::Lower)
                && motor.velocity.xz().length() > 0.05
                && !carry_bone(i)
            {
                continue;
            }
            // Walking keeps the weapon's source carry pose over the locomotion legs.
            // Ease the lower body in/out when starting or stopping, keeping hand ownership.
            let w = if clip == ClipId::Stance && !carry_bone(i) {
                w * rest.legs
            } else {
                w
            };
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

pub fn clip_grip(id: ClipId, moves: &DsMoves, defense: Option<&crate::defense::Kit>) -> Grip {
    match id {
        ClipId::Defense(k) if k.guarding() || k == crate::defense::Kind::Lower => {
            defense.map_or(moves.grip, |k| k.grip)
        }
        _ => moves.grip,
    }
}

/// The clip layers of the avatar's pose (the stance, the actions): they run over the gait's base
/// layer ([`crate::dsloco`]), which is ordered before this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ClipLayers;

pub fn plugin(app: &mut App) {
    if let Some(q) = std::env::var("WOWSOULS_GRIP")
        .ok()
        .and_then(|v| parse_grip_override(&v))
    {
        app.insert_resource(GripOverride(q));
    }
    // In `PosePost` beside benilla's global-sequence channels, which drive no bone the clip does.
    app.add_systems(Startup, startup)
        .add_systems(PostUpdate, pose.in_set(PosePost).in_set(ClipLayers));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::Swing;

    /// A 2-bone, 2-frame clip with a position and a turn.
    fn clip() -> wscl::Clip {
        let a = wscl::Trs {
            t: [0.0, 1.0, 0.0],
            ..wscl::Trs::IDENTITY
        };
        let b = wscl::Trs {
            t: [0.0, 3.0, 0.0],
            r: Quat::from_rotation_y(1.0).to_array(),
            s: [1.0; 3],
        };
        wscl::Clip {
            name: "r".into(),
            fps: 10.0,
            model_space: false,
            parents: vec![-1],
            driven: vec![true],
            frames: vec![vec![a], vec![b]],
            root_motion: vec![[0.0; 4], [0.0, 0.0, -4.0, 0.0]],
            flags: vec![
                wscl::FlagWindow {
                    flag: wscl::FLAG_INVINCIBLE,
                    start: 0.02,
                    end: 0.07,
                },
                wscl::FlagWindow {
                    flag: 11,
                    start: 0.08,
                    end: 0.1,
                },
            ],
            hits: Vec::new(),
        }
    }

    #[test]
    fn sampling_interpolates_between_frames() {
        let (t, r, _) = sample(&clip(), 0.05, 0);
        assert!((t.y - 2.0).abs() < 1.0e-5);
        assert!((r.to_euler(EulerRot::YXZ).0 - 0.5).abs() < 1.0e-4);
        let (t, _, _) = sample(&clip(), 9.0, 0);
        assert!((t.y - 3.0).abs() < 1.0e-5, "clamped to the last frame");
    }

    #[test]
    fn the_spec_comes_from_the_clip() {
        let s = roll_spec(&clip());
        assert!((s.duration - 0.1).abs() < 1.0e-6);
        assert!((s.distance - 4.0).abs() < 1.0e-5);
        assert_eq!(s.iframes, (0.02, 0.07));
        assert!((s.cancel_from - 0.08).abs() < 1.0e-6);
        assert!((progress(&clip(), 0.05) - 2.0).abs() < 1.0e-5);
    }

    /// A move's clip: one second at 10 fps, with a hit window and the three cancel flags.
    fn move_clip(name: &str, hit: (f32, f32), flags: [f32; 3]) -> wscl::Clip {
        let flag = |flag, start| wscl::FlagWindow {
            flag,
            start,
            end: 1.0,
        };
        wscl::Clip {
            name: name.into(),
            fps: 10.0,
            model_space: false,
            parents: vec![-1, 0],
            driven: vec![true, true],
            frames: vec![vec![wscl::Trs::IDENTITY; 2]; 11],
            root_motion: vec![[0.0; 4]; 11],
            // The extractor also writes the clip's other flags; the loader reads only these.
            flags: vec![
                wscl::FlagWindow {
                    flag: 7,
                    start: 0.0,
                    end: 1.0,
                },
                flag(wscl::FLAG_MOVE, flags[2]),
                flag(wscl::FLAG_CHAIN, flags[0]),
                flag(wscl::FLAG_EVADE, flags[1]),
            ],
            hits: vec![wscl::HitWindow {
                judge: 0,
                start: hit.0,
                end: hit.1,
            }],
        }
    }

    #[test]
    fn a_move_takes_its_timing_from_its_clip() {
        let c = move_clip("a23_3000", (0.3, 0.4), [0.6, 0.7, 0.8]);
        let s = swing_spec(&c, 20.0, 1.35, 2, 0.0, 0.08, 0.72, 0.4).unwrap();
        assert_eq!(s.duration, 1.0);
        assert_eq!(s.hit, (0.3, 0.4));
        assert_eq!(
            (s.chain_from, s.evade_from, s.move_from),
            (0.6, 0.7, 0.8),
            "flag 4 chains, 26 evades, 11 moves"
        );
        assert_eq!((s.cost, s.mult), (20.0, 1.35));
        assert_eq!((s.level, s.poise), (2, 0.0));
        assert_eq!((s.stop, s.knock), (0.08, 0.72));
        // Without a hit window, or without a cancel flag, the clip is no move.
        let mut c = c;
        c.flags.retain(|f| f.flag != wscl::FLAG_EVADE);
        let err = swing_spec(&c, 20.0, 1.0, 1, 0.0, 0.08, 0.72, 0.4).unwrap_err();
        assert!(err.contains("no evade window"), "{err}");
        c.hits.clear();
        let err = swing_spec(&c, 20.0, 1.0, 1, 0.0, 0.08, 0.72, 0.4).unwrap_err();
        assert!(err.contains("no hit window"), "{err}");
    }

    #[test]
    fn the_clip_blends_in_at_the_start_and_out_after_the_end() {
        assert_eq!(blend_weight(0.0, None), 0.0);
        assert!((blend_weight(0.5 * BLEND_IN, None) - 0.5).abs() < 1.0e-6);
        assert_eq!(blend_weight(1.5, None), 1.0);
        // Ended at full weight: falls linearly to nothing over the fade.
        assert_eq!(blend_weight(1.1, Some(0.0)), 1.0);
        assert!((blend_weight(1.1, Some(0.5 * BLEND_OUT)) - 0.5).abs() < 1.0e-6);
        assert_eq!(blend_weight(1.1, Some(BLEND_OUT)), 0.0);
        // Cut off while still blending in: the fade starts from the weight it had.
        assert!((blend_weight(0.5 * BLEND_IN, Some(0.5 * BLEND_OUT)) - 0.25).abs() < 1.0e-6);
    }

    /// The clips' total weight over the rig from a frame's layers, each blended over the last.
    fn total(layers: [Option<Layer<ClipId>>; 2]) -> f32 {
        1.0 - layers
            .iter()
            .flatten()
            .fold(1.0, |rest, &(_, _, w)| rest * (1.0 - w))
    }

    #[test]
    fn a_chained_roll_blends_in_over_the_last_ones_end_pose() {
        const DT: f32 = 1.0 / 60.0;
        let mut fade = Fade::default();
        let mut t = 0.0;
        let mut end = 0.0;
        while t < 1.1 {
            let l = layers(&mut fade, Some((ClipId::Roll, t)), DT);
            assert!(l[0].is_none() && l[1].is_some());
            end = t;
            t += DT;
        }
        // The chained roll restarts the clock: the old end pose holds at full weight under it.
        let l = layers(&mut fade, Some((ClipId::Roll, 0.0)), DT);
        assert_eq!(l[0], Some((ClipId::Roll, end, 1.0)), "{l:?}");
        assert_eq!(l[1], Some((ClipId::Roll, 0.0, 0.0)));
        // No frame lets the rig's own clip through between the two rolls.
        let mut t = DT;
        while t < 0.5 {
            let l = layers(&mut fade, Some((ClipId::Roll, t)), DT);
            assert!(total(l) > 0.8, "t={t} {l:?}");
            t += DT;
        }
        assert!(fade.out.is_none(), "the old pose has faded");
    }

    #[test]
    fn a_swing_chained_out_of_another_clip_fades_the_old_one_under_it() {
        const DT: f32 = 1.0 / 60.0;
        let first = ClipId::Move(MoveKind::R1(0));
        let second = ClipId::Move(MoveKind::R1(1));
        let mut fade = Fade::default();
        let mut t = 0.0;
        let mut end = 0.0;
        while t < 0.9 {
            layers(&mut fade, Some((first, t)), DT);
            end = t;
            t += DT;
        }
        let l = layers(&mut fade, Some((second, 0.0)), DT);
        assert_eq!(l[0], Some((first, end, 1.0)), "{l:?}");
        assert_eq!(l[1], Some((second, 0.0, 0.0)));
        // Another clip takes over even when its time is later than the old one's.
        let mut fade = Fade::default();
        layers(&mut fade, Some((first, 0.5)), DT);
        let l = layers(&mut fade, Some((second, 0.9)), DT);
        assert!(l[0].is_some_and(|(c, ..)| c == first), "{l:?}");
    }

    #[test]
    fn an_ended_clip_fades_out_and_a_new_one_keeps_the_fade() {
        const DT: f32 = 1.0 / 60.0;
        let mut fade = Fade::default();
        layers(&mut fade, Some((ClipId::Roll, 1.1)), DT);
        // Cancelled by movement: the end pose fades from full weight.
        let l = layers(&mut fade, None, DT);
        assert_eq!(l, [Some((ClipId::Roll, 1.1, 1.0)), None]);
        layers(&mut fade, None, DT);
        // A new move inside the fade keeps the fade under its blend-in.
        let l = layers(&mut fade, Some((ClipId::Move(MoveKind::R1(0)), 0.0)), DT);
        assert!(
            l[0].is_some_and(|(c, t, w)| c == ClipId::Roll && t == 1.1 && w > 0.8),
            "{l:?}"
        );
        // Then the fade runs out.
        let mut fade = Fade::default();
        layers(&mut fade, Some((ClipId::Roll, 1.1)), DT);
        let mut l = [None, None];
        for _ in 0..30 {
            l = layers(&mut fade, None, DT);
        }
        assert_eq!(l, [None, None]);
    }

    #[test]
    fn the_action_names_the_clip_it_plays() {
        let spec = SwingSpec::procedural(0.8);
        let swing = |ds| Swing {
            t: 0.4,
            kind: MoveKind::R2(0),
            spec,
            ds,
            serial: 1,
        };
        let state = |s| Action {
            state: s,
            ..Action::default()
        };
        assert_eq!(playing(&Action::default()), None);
        assert_eq!(
            playing(&state(ActionState::Attack(swing(true)))),
            Some((ClipId::Move(MoveKind::R2(0)), 0.4))
        );
        assert_eq!(
            playing(&state(ActionState::Attack(swing(false)))),
            None,
            "the procedural swing plays the WoW clip"
        );
        let evade = |ds| crate::combat::Evade {
            kind: crate::combat::EvadeKind::Roll,
            t: 0.7,
            dir: Vec3::NEG_Z,
            spec: ROLL,
            ds,
        };
        assert_eq!(
            playing(&state(ActionState::Evade(evade(true)))),
            Some((ClipId::Roll, 0.7))
        );
        assert_eq!(playing(&state(ActionState::Evade(evade(false)))), None);
        // The backstep names its own clip, only when it is the Dark Souls one.
        let back = |ds| crate::combat::Evade {
            kind: crate::combat::EvadeKind::Backstep,
            t: 0.3,
            dir: Vec3::Z,
            spec: BACKSTEP,
            ds,
        };
        assert_eq!(
            playing(&state(ActionState::Evade(back(true)))),
            Some((ClipId::Backstep, 0.3))
        );
        assert_eq!(playing(&state(ActionState::Evade(back(false)))), None);
    }

    #[test]
    fn the_blade_turns_onto_the_dark_souls_axis() {
        // A blade along +Y (top above bottom), to be turned onto -Z.
        let fit = grip_rotation([Vec3::new(0.0, 0.9, 0.0), Vec3::ZERO], Vec3::NEG_Z);
        assert!((fit * Vec3::Y - Vec3::NEG_Z).length() < 1.0e-5);
        // Already there: no turn. Opposite: a half turn that still maps one onto the other.
        let already = grip_rotation([Vec3::Y, Vec3::ZERO], Vec3::Y);
        assert!(alike(already, Quat::IDENTITY));
        let back = grip_rotation([Vec3::Y, Vec3::ZERO], Vec3::NEG_Y);
        assert!((back * Vec3::Y - Vec3::NEG_Y).length() < 1.0e-5);
        // A blade with no length or an axis of zero is left alone.
        assert_eq!(
            grip_rotation([Vec3::ZERO, Vec3::ZERO], Vec3::Y),
            Quat::IDENTITY
        );
        assert_eq!(
            grip_rotation([Vec3::Y, Vec3::ZERO], Vec3::ZERO),
            Quat::IDENTITY
        );
        // The item follows the clips' weight: none, half, all.
        assert_eq!(item_rotation(fit, 0.0), Quat::IDENTITY);
        let half = item_rotation(fit, 0.5);
        let turn = |q: Quat| q.angle_between(Quat::IDENTITY);
        assert!((turn(half) - 0.5 * turn(fit)).abs() < 1.0e-4);
        assert!(alike(item_rotation(fit, 1.0), fit));
        assert!(alike(item_rotation(fit, 3.0), fit), "clamped");
    }

    /// Whether two rotations turn every axis alike (`q` and `-q` are one rotation, and
    /// `angle_between` loses its precision near zero).
    fn alike(a: Quat, b: Quat) -> bool {
        [Vec3::X, Vec3::Y, Vec3::Z]
            .into_iter()
            .all(|v| (a * v - b * v).length() < 1.0e-5)
    }

    #[test]
    fn the_grip_override_reads_degrees_about_the_hands_axes() {
        let q = parse_grip_override("90, 0, 0").unwrap();
        assert!((q * Vec3::Y - Vec3::Z).length() < 1.0e-5);
        assert_eq!(parse_grip_override("90,0"), None);
        assert_eq!(parse_grip_override("a,b,c"), None);
        assert_eq!(parse_grip_override("1,2,nan"), None);
    }

    #[test]
    fn the_carry_pose_holds_through_a_walk_and_gives_the_arms_back_at_a_run() {
        use crate::motor::{RUN_SPEED, WALK_SPEED};
        assert_eq!(carry_share(0.0), 1.0, "standing");
        assert_eq!(
            carry_share(WALK_SPEED),
            1.0,
            "walking keeps the weapon carried"
        );
        assert_eq!(
            carry_share(crate::controller::LOCKED_SPEED),
            0.0,
            "the locked strafe is WoW's"
        );
        assert_eq!(carry_share(RUN_SPEED), 0.0, "a run swings its arms");
        let mid = carry_share((CARRY_FULL + CARRY_GONE) * 0.5);
        assert!((mid - 0.5).abs() < 1.0e-6, "a straight ramp between: {mid}");
        // Starting to run takes the carry pose away within the blend, stopping brings it back.
        let mut rest = RestPose {
            carry: 1.0,
            ..RestPose::default()
        };
        let dt = 1.0 / 60.0;
        for _ in 0..(BLEND_OUT / dt).ceil() as usize + 1 {
            rest.ease_carry(RUN_SPEED, dt);
        }
        assert_eq!(rest.carry, 0.0);
        for _ in 0..(BLEND_OUT / dt).ceil() as usize + 1 {
            rest.ease_carry(0.0, dt);
        }
        assert_eq!(rest.carry, 1.0);
    }

    #[test]
    fn resting_keeps_full_hand_ownership_across_breathing_loops() {
        let mut rest = RestPose::default();
        let mut fade = Fade::default();
        let dt = 1.0 / 60.0;
        for frame in 0..720 {
            let now = rest.tick(true, false, dt);
            let mut draw = layers(&mut fade, now, dt);
            stance_phase(&mut draw, 2.0);
            assert!(draw[0].is_none(), "a loop is not a new action: {frame}");
            let (id, phase, weight) = draw[1].unwrap();
            assert_eq!(id, ClipId::Stance);
            assert!((0.0..2.0).contains(&phase));
            if frame > 6 {
                assert_eq!(weight, 1.0, "hands let go at frame {frame}");
            }
        }
    }

    #[test]
    fn rest_interruption_and_return_have_distinct_entry_blends() {
        let mut rest = RestPose::default();
        let mut fade = Fade::default();
        for _ in 0..150 {
            layers(&mut fade, rest.tick(true, false, 0.02), 0.02);
        }
        rest.tick(false, false, 0.02);
        let mut draw = layers(&mut fade, Some((ClipId::Move(MoveKind::R1(0)), 0.0)), 0.02);
        stance_phase(&mut draw, 2.0);
        let (id, t, weight) = draw[0].unwrap();
        assert_eq!(id, ClipId::Stance);
        assert!((0.9..1.0).contains(&t));
        assert_eq!(weight, 1.0);
        layers(&mut fade, Some((ClipId::Move(MoveKind::R1(0)), 1.5)), 0.02);
        let draw = layers(&mut fade, rest.tick(true, false, 0.02), 0.02);
        assert_eq!(draw[1], Some((ClipId::Stance, 0.0, 0.0)));
        assert_eq!(draw[0].unwrap().2, 1.0);
    }

    #[test]
    fn walking_keeps_the_carry_clock_and_eases_the_legs() {
        let mut rest = RestPose::default();
        for _ in 0..20 {
            rest.tick(true, false, 0.02);
        }
        let before = rest.elapsed.unwrap();
        assert_eq!(rest.legs, 1.0);
        rest.tick(true, true, 0.02);
        assert!(rest.elapsed.unwrap() > before);
        assert!((rest.legs - 0.9).abs() < 1.0e-5);
        for _ in 0..20 {
            rest.tick(true, true, 0.02);
        }
        assert_eq!(rest.legs, 0.0);
        rest.tick(true, false, 0.02);
        assert!((rest.legs - 0.1).abs() < 1.0e-5);
        assert_eq!(rest.tick(false, false, 0.02), None);
    }

    /// A cache directory under the temp dir.
    fn cache_dir_named(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wow-souls-dsclip-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("human_male")).unwrap();
        std::fs::create_dir_all(dir.join("c0000")).unwrap();
        dir
    }

    fn put(dir: &Path, clip: &wscl::Clip) {
        for sub in ["human_male", "c0000"] {
            let path = dir.join(sub).join(format!("{}.wscl", clip.name));
            clip.write(&mut std::fs::File::create(path).unwrap())
                .unwrap();
        }
    }

    /// A whole cache: the roll, the six moves (each with its own price, `10 + index` stamina)
    /// and a manifest, minus any line `without` names.
    fn whole_cache(name: &str, without: Option<&str>) -> PathBuf {
        let dir = cache_dir_named(name);
        put(&dir, &{
            let mut r = move_clip("a00_0710", (0.0, 0.1), [0.5, 0.5, 0.5]);
            r.hits.clear();
            r.flags.push(wscl::FlagWindow {
                flag: wscl::FLAG_INVINCIBLE,
                start: 0.0,
                end: 0.3,
            });
            r
        });
        // The backstep: backwards root travel (+Z) and invulnerable from the start.
        put(&dir, &{
            let mut b = move_clip("a00_0690", (0.0, 0.1), [0.5, 0.5, 0.5]);
            b.hits.clear();
            b.flags.push(wscl::FlagWindow {
                flag: wscl::FLAG_INVINCIBLE,
                start: 0.0,
                end: 0.1,
            });
            for (i, r) in b.root_motion.iter_mut().enumerate() {
                r[2] = 0.2 * i as f32;
            }
            b
        });
        let mut manifest = format!(
            "wscl-cache {CACHE_VERSION}\nextractor {}\nroll a00_0710\nclass straight_sword\n",
            wscl::EXTRACTOR_VERSION
        );
        if without != Some("backstep") {
            manifest += "backstep a00_0690\n";
        }
        for (i, kind) in MoveKind::ALL.into_iter().enumerate() {
            let clip = format!("clip_{}", kind.name());
            let at = 0.1 * (i as f32 + 1.0);
            put(&dir, &move_clip(&clip, (at, at + 0.1), [0.6, 0.7, 0.8]));
            if without != Some(kind.name()) {
                manifest += &format!(
                    "move {} {clip} judge 0 stamina {} mult 1.{i}0 level {} poise 0 stop 0.08 knock 0.72 radius 0.40\n",
                    kind.name(),
                    10 + i,
                    if i < 3 { 1 } else { 2 }
                );
            }
        }
        if without != Some("grip") {
            manifest += "grip 0.1 0.2 0.3 0.0 1.0 0.0\n";
        }
        std::fs::write(dir.join("manifest.txt"), manifest).unwrap();
        dir
    }

    #[test]
    fn a_whole_cache_loads_the_roll_and_six_moves() {
        let dir = whole_cache("whole", None);
        let c = load(&dir).unwrap_or_else(|e| panic!("{e}"));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(c.roll.wow.name, "a00_0710");
        // The backstep has its own clip, and a shape that goes backwards by a positive distance.
        let back = c.roll.back.as_ref().expect("the cache names a backstep");
        assert_eq!(back.wow.name, "a00_0690");
        assert!(
            (back.spec.distance - 2.0).abs() < 1.0e-4,
            "{}",
            back.spec.distance
        );
        assert_eq!(back.spec.cost, BACKSTEP.cost);
        assert_eq!(back.spec.iframes, (0.0, 0.1));
        assert!((back.spec.cancel_from - 0.5).abs() < 1.0e-6);
        let m = &c.moves.moveset;
        assert!(m.ds && m.has_heavy());
        // Each move got its own clip's windows and its own manifest price.
        assert_eq!(m.r1[0].hit, (0.1, 0.2));
        assert_eq!((m.r1[0].stop, m.r1[0].knock), (0.08, 0.72));
        assert_eq!(m.r1[1].cost, 11.0);
        assert_eq!(m.r1[2].cost, 12.0);
        let r2 = m.r2.unwrap();
        assert_eq!((r2[0].cost, r2[1].cost), (13.0, 14.0));
        let roll_attack = m.roll_attack.unwrap();
        assert_eq!(roll_attack.cost, 15.0);
        assert!((roll_attack.mult - 1.5).abs() < 1.0e-6);
        assert_eq!(roll_attack.chain_from, 0.6);
        assert_eq!(c.moves.clip(MoveKind::R2(1)).unwrap().wow.name, "clip_r2_2");
        assert_eq!(c.moves.grip.hand, Vec3::new(0.1, 0.2, 0.3));
        assert_eq!(c.moves.grip.weapon, Vec3::Y);
    }

    #[test]
    fn an_older_cache_without_a_backstep_loads_and_keeps_the_wow_sway() {
        let dir = whole_cache("old-no-backstep", Some("backstep"));
        let c = load(&dir).unwrap_or_else(|e| panic!("{e}"));
        std::fs::remove_dir_all(&dir).ok();
        assert!(c.roll.back.is_none());
        assert_eq!(c.roll.wow.name, "a00_0710");
    }

    #[test]
    fn a_cache_lacking_a_move_or_the_grip_is_refused() {
        for (name, without, why) in [
            ("no-move", "r2_2", "manifest lacks move r2_2"),
            ("no-grip", "grip", "manifest has no grip line"),
        ] {
            let dir = whole_cache(name, Some(without));
            let err = load(&dir).err().unwrap_or_else(|| panic!("{name} loads"));
            std::fs::remove_dir_all(&dir).ok();
            assert!(err.contains(why), "{name}: {err}");
        }
        assert!(parse_move(
            "move r1_1 a23_3000 judge 0 stamina x mult 1.0 level 1 poise 0 stop 0.08 knock 0.72"
        )
        .is_err());
        assert!(parse_move(
            "move r9_9 a23_3000 judge 0 stamina 20 mult 1.0 level 1 poise 0 stop 0.08 knock 0.72"
        )
        .is_err());
        assert!(
            parse_move("move r1_1 a23_3000 judge 0 stamina 20 mult 1.0 level 1 poise 0").is_err(),
            "stop and knock are required"
        );
        assert!(parse_grip_line("grip 1 2 3").is_err());
    }

    /// A cache dir under the temp dir with `manifest` as its manifest.
    fn manifest_dir(name: &str, manifest: &str) -> PathBuf {
        let dir = cache_dir_named(name);
        std::fs::write(dir.join("manifest.txt"), manifest).unwrap();
        dir
    }

    #[test]
    fn the_manifest_must_carry_this_games_versions() {
        let roll = "roll a00_0710\n";
        let cases = [
            ("no-versions", roll.to_string(), "manifest lacks a version"),
            (
                "no-extractor",
                format!("wscl-cache {CACHE_VERSION}\n{roll}"),
                "manifest lacks a version",
            ),
            (
                "old-extractor",
                format!("wscl-cache {CACHE_VERSION}\nextractor 1\n{roll}"),
                "extractor version 1, expected",
            ),
            (
                "old-cache",
                format!(
                    "wscl-cache 0\nextractor {}\n{roll}",
                    wscl::EXTRACTOR_VERSION
                ),
                "cache version 0",
            ),
        ];
        for (name, manifest, why) in cases {
            let dir = manifest_dir(name, &manifest);
            let err = load(&dir).err().unwrap_or_else(|| panic!("{name} loads"));
            std::fs::remove_dir_all(&dir).ok();
            assert!(err.starts_with(why), "{name}: {err}");
        }
        // Both versions right: it gets as far as the clips.
        let manifest = format!(
            "wscl-cache {CACHE_VERSION}\nextractor {}\n{roll}",
            wscl::EXTRACTOR_VERSION
        );
        let dir = manifest_dir("ok", &manifest);
        let err = load(&dir).err().unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(err.contains("a00_0710.wscl"), "{err}");
    }
}
