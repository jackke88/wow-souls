//! Motion remix (`WOWSOULS_MOTION=1`): one recipe engine for every Dark Souls clip the game plays.
//! DSR-derived, not an independent animation asset: each clip keeps its source body, feet, root
//! motion and events, and gets four things laid over it (the wolf experiment's method, made
//! general, [`crate::monster::wolf_motion`] keeps its hand-authored wolf curves):
//!
//! - **Timing** (enemy attacks only): a longer, held wind-up, a shorter strike and a longer
//!   recovery, through one monotone clock that also carries root motion, flags and hit windows,
//!   so the move's contact band moves with its picture. The player's own moves keep the DS
//!   timing to the frame: their windows live in the manifest, and the cancel windows are DS data.
//! - **Coil and follow-through** (attacks): the trunk winds up away from the swing, reads that
//!   direction from where the striking hand really goes in the clip, holds, then releases past
//!   neutral and settles, while the head keeps half its gaze on the target.
//! - **Secondary motion** (every non-looping clip): neck, head, jaw, tail and ears are springs in
//!   model space, so they lag a quick trunk, overshoot and settle. Fades to the source pose at
//!   the clip's end, so blends, holds and the next clip's start are as before.
//! - Nothing else: legs, arms, translations, scales and root motion are the source's; looping
//!   gaits and the stance are left to [`crate::lively`], which already dresses them.
//!
//! The load keeps first and last frames bit-identical. A rig or clip that does not look as
//! expected is left as it was (a warning in the log). Cache files and the DS reference are never
//! rewritten. `WOWSOULS_WOLF_MOTION=1` alone still remixes only the wolves' bite and lunge.

use bevy::prelude::*;
use wscl::{Clip, Trs};

use super::{wolf_motion, MonsterClips};
use crate::motion_clip::sample;

/// The rate the retimed clips are baked at (a retime needs more than DS's 30 to stay smooth).
pub(super) const FPS: f32 = 60.0;

/// `WOWSOULS_MOTION=1`: remix every clip. Off by default; the originals stay the standard.
pub(crate) fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("WOWSOULS_MOTION").as_deref() == Ok("1"))
}

// ---------------------------------------------------------------------------------------------
// The clock

/// Monotone cubic Hermite map: (new seconds, source seconds). Harmonic-mean tangents keep the
/// source time moving forward without the abrupt speed changes of a piecewise-linear map.
pub(super) struct Clock(Vec<(f32, f32)>);

impl Clock {
    /// `knots` start at (0, 0) and both coordinates strictly increase.
    pub(super) fn new(knots: Vec<(f32, f32)>) -> Result<Self, String> {
        if knots.len() < 2
            || knots[0] != (0.0, 0.0)
            || knots.iter().any(|k| !k.0.is_finite() || !k.1.is_finite())
            || knots
                .windows(2)
                .any(|p| !(p[1].0 > p[0].0 && p[1].1 > p[0].1))
        {
            return Err("phase clock must start at zero and increase strictly".into());
        }
        Ok(Self(knots))
    }

    /// The remixed clip's length (s).
    pub(super) fn length(&self) -> f32 {
        self.0[self.0.len() - 1].0
    }

    fn slope(&self, j: usize) -> f32 {
        (self.0[j + 1].1 - self.0[j].1) / (self.0[j + 1].0 - self.0[j].0)
    }

    fn tangent(&self, i: usize) -> f32 {
        let last = self.0.len() - 1;
        if i == 0 {
            return self.slope(0);
        }
        if i == last {
            return self.slope(last - 1);
        }
        let a = self.0[i].0 - self.0[i - 1].0;
        let b = self.0[i + 1].0 - self.0[i].0;
        let (w1, w2) = (2.0 * b + a, b + 2.0 * a);
        (w1 + w2) / (w1 / self.slope(i - 1) + w2 / self.slope(i))
    }

    /// The source time shown at remixed time `t`.
    pub(super) fn source_at(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, self.length());
        let i = self
            .0
            .partition_point(|p| p.0 < t)
            .saturating_sub(1)
            .min(self.0.len() - 2);
        let ((x0, y0), (x1, y1)) = (self.0[i], self.0[i + 1]);
        let h = x1 - x0;
        let u = (t - x0) / h;
        let (u2, u3) = (u * u, u * u * u);
        (2.0 * u3 - 3.0 * u2 + 1.0) * y0
            + (u3 - 2.0 * u2 + u) * h * self.tangent(i)
            + (-2.0 * u3 + 3.0 * u2) * y1
            + (u3 - u2) * h * self.tangent(i + 1)
    }

    /// The remixed time at which source time `source` is shown (the inverse of [`Self::source_at`]).
    pub(super) fn target_at(&self, source: f32) -> f32 {
        if source <= 0.0 {
            return 0.0;
        }
        if source >= self.0[self.0.len() - 1].1 {
            return self.length();
        }
        let (mut lo, mut hi) = (0.0, self.length());
        for _ in 0..28 {
            let mid = (lo + hi) * 0.5;
            if self.source_at(mid) < source {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) * 0.5
    }

    /// The remixed time of a knot's source time (the knots are where the phases meet).
    fn at_source(&self, source: f32) -> f32 {
        self.0
            .iter()
            .find(|k| (k.1 - source).abs() < 1e-5)
            .map_or_else(|| self.target_at(source), |k| k.0)
    }
}

// ---------------------------------------------------------------------------------------------
// The rigs

/// Where the bones a remix touches sit on one WoW skeleton. Indices from `benilla-extract
/// m2bones` (see `dsr-cache/src/bonemap.rs`); every rig is checked against the clip's parents
/// before anything is written.
pub(super) struct Rig {
    /// Trunk bones, low to high; the last one carries the shoulders.
    spine: &'static [usize],
    /// The neck bone; `None` for a body whose head hangs straight from its chest (the murloc).
    pub(super) neck: Option<usize>,
    pub(super) head: usize,
    pub(super) jaw: Option<usize>,
    /// Tail bones in the order they hang from the body, and how much faster each one's spring is
    /// than the base tail's (the tip is slower, so it trails).
    pub(super) tail: &'static [(usize, f32)],
    pub(super) ears: &'static [usize],
    /// The striking hand of a bare move.
    hand: usize,
}

const HUMAN_MALE: Rig = Rig {
    spine: &[20, 22],
    neck: Some(48),
    head: 64,
    jaw: None,
    tail: &[],
    ears: &[],
    hand: 97,
};
const HUMAN_FEMALE: Rig = Rig {
    spine: &[21, 23],
    neck: Some(44),
    head: 63,
    jaw: None,
    tail: &[],
    ears: &[],
    hand: 96,
};
const KOBOLD: Rig = Rig {
    spine: &[14, 16],
    neck: Some(29),
    head: 35,
    jaw: None,
    tail: &[],
    ears: &[],
    hand: 53,
};
const GNOLL: Rig = Rig {
    spine: &[11, 13],
    neck: Some(15),
    head: 30,
    jaw: Some(39),
    tail: &[(18, 1.0), (33, 0.85), (42, 0.7)],
    ears: &[],
    hand: 40,
};
const WOLF: Rig = Rig {
    spine: &[],
    neck: Some(25),
    head: 29,
    jaw: Some(47),
    tail: &[(42, 1.0), (41, 0.93), (40, 0.86), (39, 0.79), (38, 0.72)],
    ears: &[45, 46],
    hand: 47,
};
const DIREWOLF: Rig = Rig {
    spine: &[],
    neck: Some(21),
    head: 25,
    jaw: Some(35),
    tail: &[(46, 1.0), (52, 0.9), (57, 0.8), (60, 0.7)],
    ears: &[],
    hand: 35,
};
const MURLOC: Rig = Rig {
    // The chest only: the hip root (14) carries the legs, and a trunk turn must not move the feet.
    spine: &[15],
    neck: None,
    head: 25,
    jaw: Some(42),
    tail: &[],
    ears: &[],
    hand: 49,
};
const BOAR: Rig = Rig {
    spine: &[],
    neck: Some(17),
    head: 21,
    jaw: Some(33),
    tail: &[(30, 1.0), (43, 0.9), (47, 0.8), (52, 0.7)],
    ears: &[31, 32],
    hand: 33,
};
const BEAR: Rig = Rig {
    spine: &[],
    neck: Some(28),
    head: 38,
    jaw: Some(45),
    tail: &[(18, 1.0), (24, 0.85)],
    ears: &[],
    hand: 45,
};
/// The Defias Ambusher's `HumanThief` body: spine 11 / 13, neck 27, head 35, jaw 44, the right hand 53.
const AMBUSHER: Rig = Rig {
    spine: &[11, 13],
    neck: Some(27),
    head: 35,
    jaw: Some(44),
    tail: &[],
    ears: &[],
    hand: 53,
};
const GOBLIN: Rig = Rig {
    spine: &[16, 18],
    neck: Some(34),
    head: 50,
    jaw: Some(64),
    tail: &[],
    ears: &[],
    hand: 83,
};
const MINESPIDER: Rig = Rig {
    spine: &[23],
    neck: None,
    head: 35,
    jaw: Some(64),
    tail: &[],
    ears: &[],
    hand: 64,
};
const TARANTULA: Rig = Rig {
    spine: &[],
    neck: None,
    head: 2,
    jaw: Some(34),
    tail: &[],
    ears: &[],
    hand: 34,
};

pub(super) fn rig_of_kit(name: &str) -> Option<&'static Rig> {
    Some(match name {
        "kobold" | "geomancer" | "goldtooth" | "ruklar" | "narg" => &KOBOLD,
        "wolf" => &WOLF,
        "direwolf" => &DIREWOLF,
        "hogger" | "hogger_knight" | "gnoll" | "gruff" | "fedfennel" => &GNOLL,
        "murloc" => &MURLOC,
        "boar" | "boar_dog" | "princess" => &BOAR,
        "bear" | "bear_dog" => &BEAR,
        "goblin" => &GOBLIN,
        "ambusher" => &AMBUSHER,
        "minespider" | "mother_fang" => &MINESPIDER,
        "tarantula" => &TARANTULA,
        "thug_female" | "human_bandit_female" | "caster_female" | "morgaine" | "surena" => {
            &HUMAN_FEMALE
        }
        "thug" | "human_axe_2h" | "human_bandit" | "garrick" | "caster" | "thuros" | "morgan"
        | "dead_tooth_jack" | "james_clark" | "erlan" => &HUMAN_MALE,
        _ => None?,
    })
}

/// The rig of a WoW model by its bone count (the client's models: HumanMale 119, HumanFemale 117,
/// Kobold 72, Wolf 64, DireWolf 61, GnollMelee 60); `fits` still has to agree before it is used.
pub(super) fn rig_for_bones(bones: usize) -> Option<&'static Rig> {
    Some(match bones {
        119 => &HUMAN_MALE,
        117 => &HUMAN_FEMALE,
        72 => &KOBOLD,
        64 => &WOLF,
        61 => &DIREWOLF,
        60 => &GNOLL,
        62 => &MURLOC,
        57 => &BOAR,
        54 => &BEAR,
        _ => None?,
    })
}

impl Rig {
    pub(super) fn bones(&self) -> impl Iterator<Item = usize> + '_ {
        self.spine
            .iter()
            .copied()
            .chain(self.neck)
            .chain([self.head])
            .chain(self.jaw)
            .chain(self.tail.iter().map(|t| t.0))
            .chain(self.ears.iter().copied())
    }

    /// Whether `clip` is this skeleton: every bone exists, the head hangs from the neck, the neck
    /// and jaw from where they should, each trunk bone is an ancestor of the neck and the tail
    /// hangs from the body (not from the head).
    fn check(&self, clip: &Clip) -> Result<(), String> {
        let n = clip.bones();
        if self.bones().chain([self.hand]).any(|b| b >= n)
            || clip.driven.len() != n
            || clip.frames.iter().any(|f| f.len() != n)
        {
            return Err(format!("{}: {n} bones do not fit this rig", clip.name));
        }
        if self.fits(&clip.parents) {
            Ok(())
        } else {
            Err(format!("{}: bone parents do not match this rig", clip.name))
        }
    }

    /// Whether a skeleton with these `parents` hangs together as this rig does.
    pub(super) fn fits(&self, parents: &[i16]) -> bool {
        if self.bones().chain([self.hand]).any(|b| b >= parents.len()) {
            return false;
        }
        let parent = |b: usize| usize::try_from(parents[b]).ok();
        let ancestor = |mut b: usize, of: usize| {
            while let Some(p) = parent(b) {
                if p == of {
                    return true;
                }
                b = p;
            }
            false
        };
        // The head hangs from the neck, or from the last trunk bone when there is no neck.
        let above = self.neck.or(self.spine.last().copied());
        let base = self.neck.unwrap_or(self.head);
        above.is_none_or(|b| parent(self.head) == Some(b))
            && self.jaw.is_none_or(|j| parent(j) == Some(self.head))
            && self.spine.iter().all(|&s| ancestor(base, s))
            && self.tail.iter().all(|t| !ancestor(t.0, self.head))
            && self
                .ears
                .iter()
                .all(|&e| ancestor(e, base) || ancestor(e, self.head))
    }
}

// ---------------------------------------------------------------------------------------------
// Pose maths

fn rot(t: &Trs) -> Quat {
    Quat::from_array(t.r).normalize()
}

/// The model-space rotation of everything above `bone`: its parents' rotations, root first.
fn parent_rot(frame: &[Trs], parents: &[i16], bone: usize) -> Quat {
    let mut q = Quat::IDENTITY;
    let mut cur = parents[bone];
    while let Ok(i) = usize::try_from(cur) {
        q = rot(&frame[i]) * q;
        cur = parents[i];
    }
    q
}

/// A bone's model-space position in the source clip at `t` (no root motion).
fn model_point(clip: &Clip, t: f32, bone: usize) -> Vec3 {
    let mut p = Vec3::ZERO;
    let mut cur = bone as i16;
    while let Ok(i) = usize::try_from(cur) {
        let (tr, r, s) = sample(clip, t, i);
        p = tr + r * (s * p);
        cur = clip.parents[i];
    }
    p
}

fn smooth(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

fn euler(pitch: f32, yaw: f32) -> Quat {
    Quat::from_euler(EulerRot::XYZ, pitch.to_radians(), yaw.to_radians(), 0.0)
}

/// Turn `bone` by `delta` about its parent's axes, if the clip drives it.
fn turn(frame: &mut [Trs], driven: &[bool], bone: usize, delta: Quat) {
    if driven[bone] {
        frame[bone].r = (delta * rot(&frame[bone])).normalize().to_array();
    }
}

// ---------------------------------------------------------------------------------------------
// What a blow weighs

/// How heavy a blow reads: the timing multipliers of its phases and the degrees of its coil.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Weight {
    /// Wind-up length, and how much slower than the rest of it its first half runs (the hold).
    tele: f32,
    hold: f32,
    /// Length of each hit window and of the recovery after the last one.
    strike: f32,
    rec: f32,
    /// Trunk yaw and pitch, neck dip, all at full coil.
    yaw: f32,
    pitch: f32,
    dip: f32,
}

const LIGHT: Weight = Weight {
    tele: 1.12,
    hold: 0.12,
    strike: 0.88,
    rec: 1.10,
    yaw: 7.0,
    pitch: 4.0,
    dip: 4.0,
};
const HEAVY: Weight = Weight {
    tele: 1.22,
    hold: 0.15,
    strike: 0.82,
    rec: 1.18,
    yaw: 11.0,
    pitch: 7.0,
    dip: 6.0,
};
const BOSS: Weight = Weight {
    tele: 1.15,
    hold: 0.12,
    strike: 0.90,
    rec: 1.12,
    yaw: 13.0,
    pitch: 9.0,
    dip: 6.0,
};

/// A monster move's weight from its stagger level and poise damage.
fn weight_of_move(level: u8, poise: f32) -> Weight {
    if level >= 6 || poise >= 50.0 {
        BOSS
    } else if level >= 2 || poise >= 25.0 {
        HEAVY
    } else {
        LIGHT
    }
}

/// Where the striking hand goes between the first hit window's start and the last one's end, in
/// the body's frame (right +X, up +Y, forward -Z): the trunk coils the other way.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Swing {
    /// Sideways share (+: to the right), times the swing's size.
    lateral: f32,
    /// Overhead share minus thrust share: +: the trunk winds back, -: forward.
    lean: f32,
}

impl Swing {
    fn measure(clip: &Clip, rig: &Rig, hand: usize, from: f32, to: f32) -> Swing {
        let low = rig.spine.first().copied().or(rig.neck).unwrap_or(rig.head);
        let torso = (model_point(clip, from, rig.head) - model_point(clip, from, low)).length();
        let v = model_point(clip, to, hand) - model_point(clip, from, hand);
        let size = v.length();
        if !(torso > 1e-4 && size > 0.02 * torso) {
            return Swing::default();
        }
        let n = v / size;
        let k = (size / torso).clamp(0.35, 1.0);
        Swing {
            lateral: n.x * k,
            lean: (-0.9 * n.y - 0.35 * n.z).clamp(-1.0, 1.0) * k,
        }
    }
}

/// When the blow lands on the clip's own timeline.
#[derive(Clone, Copy)]
struct Phases {
    strike: f32,
    end: f32,
    len: f32,
}

/// How far past neutral the trunk is carried by the release.
const FOLLOW: f32 = 0.55;

/// The coil, -[`FOLLOW`] to 1: wound up fast, held, and let go before the blow, so that it is
/// zero for the whole of the hit windows (the blade is swept from the hand the clip puts there:
/// a hand moved by the coil is a blow that misses). The follow-through comes after the last
/// window and settles in the recovery. Zero at both ends of the clip.
fn coil(t: f32, p: Phases) -> f32 {
    if t <= 0.0 || t >= p.len {
        0.0
    } else if t < p.strike {
        let u = t / p.strike;
        if u < 0.45 {
            let v = 1.0 - u / 0.45;
            1.0 - v * v
        } else if u < 0.65 {
            1.0
        } else {
            1.0 - smooth((u - 0.65) / 0.35)
        }
    } else if t <= p.end {
        0.0
    } else {
        let u = (t - p.end) / (p.len - p.end).max(1e-3);
        if u < 0.2 {
            -FOLLOW * smooth(u / 0.2)
        } else {
            -FOLLOW * (1.0 - smooth((u - 0.2) / 0.5))
        }
    }
}

/// Lay the coil on one frame at `t`.
fn coil_frame(
    frame: &mut [Trs],
    driven: &[bool],
    rig: &Rig,
    w: Weight,
    s: Swing,
    p: Phases,
    t: f32,
) {
    let c = coil(t, p);
    if c == 0.0 {
        return;
    }
    let yaw = w.yaw * s.lateral * c;
    let pitch = w.pitch * s.lean * c;
    let n = rig.spine.len();
    for (i, &bone) in rig.spine.iter().enumerate() {
        // The upper trunk carries 60 %, the rest share the other 40.
        let share = if i + 1 == n {
            if n == 1 {
                1.0
            } else {
                0.6
            }
        } else {
            0.4 / (n - 1) as f32
        };
        turn(frame, driven, bone, euler(pitch * share, yaw * share));
    }
    // The head holds half its gaze on the target while the trunk turns; the nose dips in the
    // wind-up and lifts a little in the release.
    let dip = -w.dip * c.max(0.0) + 0.5 * w.dip * (-c).max(0.0);
    if let Some(neck) = rig.neck {
        turn(frame, driven, neck, euler(dip, 0.0));
    }
    turn(frame, driven, rig.head, euler(0.0, -0.5 * yaw));
}

// ---------------------------------------------------------------------------------------------
// Secondary motion

/// One spring: how fast it answers, how much it overshoots, how much of it shows, and the most
/// it may lag the source (degrees).
#[derive(Clone, Copy)]
pub(super) struct Spring {
    pub(super) omega: f32,
    zeta: f32,
    pub(super) mix: f32,
    pub(super) max: f32,
}

/// The springs of a kind of clip.
#[derive(Clone, Copy)]
pub(super) struct Springs {
    pub(super) neck: Spring,
    pub(super) head: Spring,
    pub(super) jaw: Spring,
    pub(super) tail: Spring,
    pub(super) ear: Spring,
}

const fn sp(omega: f32, zeta: f32, mix: f32, max: f32) -> Spring {
    Spring {
        omega,
        zeta,
        mix,
        max,
    }
}

/// An attack: the head trails the trunk and snaps through; the jaw and the tail overshoot.
const ATTACK: Springs = Springs {
    neck: sp(20.0, 0.7, 0.5, 12.0),
    head: sp(17.0, 0.5, 0.7, 20.0),
    jaw: sp(24.0, 0.35, 0.7, 22.0),
    tail: sp(10.0, 0.3, 0.85, 32.0),
    ear: sp(15.0, 0.3, 0.7, 22.0),
};
/// A flinch or a death: the head whips after the blow and rings out.
const HURT: Springs = Springs {
    neck: sp(18.0, 0.55, 0.6, 16.0),
    head: sp(14.0, 0.4, 0.9, 28.0),
    jaw: sp(22.0, 0.3, 0.8, 26.0),
    tail: sp(9.0, 0.3, 0.9, 34.0),
    ear: sp(14.0, 0.3, 0.8, 26.0),
};
/// A creature on WoW's own animation (`remix_live`): softer than a clip, which the bake can
/// afford to dress more, since this one runs on top of motion it does not know.
pub(super) const LIVE: Springs = Springs {
    neck: sp(22.0, 0.8, 0.3, 6.0),
    head: sp(16.0, 0.65, 0.5, 10.0),
    jaw: sp(22.0, 0.5, 0.4, 8.0),
    tail: sp(10.0, 0.35, 0.7, 24.0),
    ear: sp(15.0, 0.4, 0.5, 14.0),
};
/// A gait or the stance: the head trails the body's bob a little, never far.
const LOOP: Springs = Springs {
    neck: sp(22.0, 0.8, 0.35, 6.0),
    head: sp(16.0, 0.65, 0.5, 9.0),
    jaw: sp(22.0, 0.5, 0.4, 8.0),
    tail: sp(10.0, 0.4, 0.6, 20.0),
    ear: sp(15.0, 0.4, 0.5, 12.0),
};
/// Anything else (a guard, a spell, a drink): a light trail.
const MISC: Springs = Springs {
    neck: sp(20.0, 0.7, 0.4, 10.0),
    head: sp(15.0, 0.55, 0.6, 16.0),
    jaw: sp(22.0, 0.4, 0.6, 18.0),
    tail: sp(10.0, 0.35, 0.8, 28.0),
    ear: sp(15.0, 0.35, 0.6, 20.0),
};
/// A roll or a backstep turns the whole body over: the head trails a little, never far.
const EVADE: Springs = Springs {
    neck: sp(22.0, 0.8, 0.3, 6.0),
    head: sp(16.0, 0.6, 0.45, 10.0),
    jaw: sp(22.0, 0.5, 0.4, 10.0),
    tail: sp(10.0, 0.4, 0.6, 20.0),
    ear: sp(15.0, 0.4, 0.5, 14.0),
};

/// The rotation that takes `from` onto `to`, as a world-space rotation vector, the short way.
fn error(to: Quat, from: Quat) -> Vec3 {
    let d = to * from.inverse();
    (if d.w < 0.0 { -d } else { d }).to_scaled_axis()
}

/// A spring chasing a rotation in model space: its orientation and angular velocity.
#[derive(Clone, Copy, Debug)]
pub(super) struct Chase {
    q: Quat,
    w: Vec3,
}

impl Chase {
    /// At rest on `target`.
    pub(super) fn at(target: Quat) -> Self {
        Self {
            q: target,
            w: Vec3::ZERO,
        }
    }

    /// Advance `dt` seconds toward `target`; the spring's own orientation, held within its cap of
    /// the target.
    pub(super) fn step(&mut self, target: Quat, sp: Spring, dt: f32) -> Quat {
        const SUB: usize = 4;
        let h = dt / SUB as f32;
        for _ in 0..SUB {
            let e = error(target, self.q);
            self.w += (sp.omega * sp.omega * e - 2.0 * sp.zeta * sp.omega * self.w) * h;
            self.q = (Quat::from_scaled_axis(self.w * h) * self.q).normalize();
        }
        let e = error(target, self.q);
        let max = sp.max.to_radians();
        if e.length() > max {
            self.q = Quat::from_scaled_axis(-e.normalize() * max) * target;
        }
        self.q
    }
}

/// Make `bone` a spring in model space. Its target each frame is what the frames already say
/// (its parents as they now stand, plus its own local rotation); the spring chases that and the
/// bone shows `mix` of the way there, fading to the target over the last `fade` seconds.
fn follow_bone(
    frames: &mut [Vec<Trs>],
    parents: &[i16],
    bone: usize,
    sp: Spring,
    fps: f32,
    fade: f32,
    looping: bool,
) {
    let n = frames.len();
    let mut chase = Chase::at(Quat::IDENTITY);
    for (i, frame) in frames.iter_mut().enumerate() {
        let above = parent_rot(frame, parents, bone);
        let target = above * rot(&frame[bone]);
        if i == 0 {
            chase = Chase::at(target);
            continue;
        }
        let q = chase.step(target, sp, 1.0 / fps);
        let left = (n - 1 - i) as f32 / fps;
        let shown = if looping {
            sp.mix
        } else {
            sp.mix * smooth(left / fade)
        };
        let out = target.slerp(q, shown);
        frame[bone].r = (above.inverse() * out).normalize().to_array();
    }
}

/// Bones a spring must leave alone: the striking bone and everything it hangs from (a boar's jaw hangs
/// from its head, so its head springs would move the blow through a hit window). Empty for a body
/// whose blow is struck by a hand: its head chain is not in the way.
fn shield_of(parents: &[i16], strike: usize, rig: &Rig) -> Vec<usize> {
    let mut chain = vec![strike];
    let mut up = parents[strike];
    while let Ok(i) = usize::try_from(up) {
        chain.push(i);
        up = parents[i];
    }
    let head_chain = rig.neck.into_iter().chain([rig.head]).chain(rig.jaw);
    chain.retain(|b| head_chain.clone().any(|h| h == *b));
    chain
}

/// Run the rig's springs over the frames, parents before children, but not on the `shield` bones.
fn follow(
    frames: &mut [Vec<Trs>],
    clip: &Clip,
    rig: &Rig,
    shield: &[usize],
    springs: Springs,
    fps: f32,
    looping: bool,
) {
    let seconds = frames.len().saturating_sub(1) as f32 / fps;
    let fade = (seconds * 0.4).clamp(0.05, 0.25);
    let free =
        |b: usize| clip.driven[b] && usize::try_from(clip.parents[b]).is_ok_and(|p| clip.driven[p]);
    let mut go = |bone: usize, mut spring: Spring, speed: f32| {
        if free(bone) && !shield.contains(&bone) {
            spring.omega *= speed;
            follow_bone(frames, &clip.parents, bone, spring, fps, fade, looping);
        }
    };
    if let Some(neck) = rig.neck {
        go(neck, springs.neck, 1.0);
    }
    go(rig.head, springs.head, 1.0);
    if let Some(jaw) = rig.jaw {
        go(jaw, springs.jaw, 1.0);
    }
    for &(bone, speed) in rig.tail {
        go(bone, springs.tail, speed);
    }
    for &bone in rig.ears {
        go(bone, springs.ear, 1.0);
    }
}

// ---------------------------------------------------------------------------------------------
// Dressing a clip

/// What a clip is, for the springs and the coil.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// A blow, with its weight.
    Attack(Weight),
    Hurt,
    Evade,
    Misc,
    /// A loop (a gait, the stance): not touched.
    Loop,
}

/// What a player-rig clip is, from its name and events.
fn kind_of_player(clip: &Clip) -> Kind {
    if !clip.hits.is_empty() {
        // DS's long moves are its heavy ones.
        let d = clip.duration();
        return Kind::Attack(if d >= 1.8 {
            BOSS
        } else if d >= 1.2 {
            HEAVY
        } else {
            LIGHT
        });
    }
    let name = clip.name.as_str();
    if name.contains("_reaction_") || name.contains("_death_") || name.contains("_stagger_") {
        Kind::Hurt
    } else if name.ends_with("_roll") || name.ends_with("_backstep") || name == "body_evade_variant"
    {
        Kind::Evade
    } else if [
        "_guard_raise",
        "_guard_lower",
        "_interaction_",
        "_spell_cast_",
        "_light_",
        "_heavy_",
        "_attack_",
        "_motion_",
        "_critical_response",
        "_emote_variant",
        "_aim_release",
        "_pickup_",
        "_chest_open",
        "_self_enchant",
        "_followup",
    ]
    .iter()
    .any(|part| name.contains(part))
    {
        Kind::Misc
    } else {
        Kind::Loop
    }
}

/// The frames of `clip` dressed as `kind`, at the clip's own rate: the coil from the clip's hit
/// windows (when it is an attack), then the springs. First and last frames are the source's.
fn dress(clip: &Clip, rig: &Rig, kind: Kind, hand: usize) -> Result<Clip, String> {
    rig.check(clip)?;
    let (springs, coiling) = match kind {
        Kind::Attack(w) => (ATTACK, Some(w)),
        Kind::Hurt => (HURT, None),
        Kind::Evade => (EVADE, None),
        Kind::Misc => (MISC, None),
        Kind::Loop => return Ok(dress_loop(clip, rig)),
    };
    let mut out = clip.clone();
    if let (Some(w), Some(first), Some(last)) = (
        coiling,
        clip.hits.iter().map(|h| h.start).reduce(f32::min),
        clip.hits.iter().map(|h| h.end).reduce(f32::max),
    ) {
        let swing = Swing::measure(clip, rig, hand, first, last);
        let p = Phases {
            strike: first,
            end: last.min(clip.duration() - 1e-3).max(first + 1e-3),
            len: clip.duration(),
        };
        for (i, frame) in out.frames.iter_mut().enumerate() {
            coil_frame(frame, &clip.driven, rig, w, swing, p, i as f32 / clip.fps);
        }
    }
    let shield = if coiling.is_some() {
        shield_of(&clip.parents, hand, rig)
    } else {
        Vec::new()
    };
    follow(
        &mut out.frames,
        clip,
        rig,
        &shield,
        springs,
        clip.fps,
        false,
    );
    out.frames[0] = clip.frames[0].clone();
    *out.frames.last_mut().expect("a clip has frames") = clip.frames.last().unwrap().clone();
    Ok(out)
}

/// A loop (a gait, the stance) with its springs run to the steady state: three laps are
/// simulated and the last is kept, so the head does not start from rest and the seam closes. A
/// clip whose last frame repeats the first one's pose of everything the springs depend on (the
/// rig's bones and their ancestors; a gait's feet and hips may still travel) is one frame shorter
/// than its lap.
fn dress_loop(clip: &Clip, rig: &Rig) -> Clip {
    let n = clip.frames.len();
    let mut chain: Vec<usize> = rig.bones().collect();
    let mut up = clip.parents[rig.neck.unwrap_or(rig.head)];
    while let Ok(i) = usize::try_from(up) {
        chain.push(i);
        up = clip.parents[i];
    }
    let closed = chain
        .iter()
        .all(|&b| rot(&clip.frames[0][b]).angle_between(rot(&clip.frames[n - 1][b])) < 1e-3);
    let lap = if closed { n - 1 } else { n };
    let mut frames: Vec<Vec<Trs>> = (0..3)
        .flat_map(|_| clip.frames[..lap].iter().cloned())
        .collect();
    follow(&mut frames, clip, rig, &[], LOOP, clip.fps, true);
    let mut out = clip.clone();
    out.frames[..lap].clone_from_slice(&frames[2 * lap..]);
    if closed {
        for bone in rig.bones() {
            out.frames[n - 1][bone] = out.frames[0][bone];
        }
    }
    out
}

/// The hit windows of `clip` a retime can work with: sorted, inside the clip, not overlapping.
/// `None` when it has none (a window can lie past the end of its clip in the DS data).
fn usable_windows(clip: &Clip) -> Option<Vec<(f32, f32)>> {
    let d = clip.duration();
    let mut windows: Vec<(f32, f32)> = clip
        .hits
        .iter()
        .map(|h| (h.start.max(0.0), h.end.min(d - 1e-3)))
        .collect();
    windows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let ok = !windows.is_empty()
        && windows.iter().all(|h| h.1 > h.0)
        && windows.windows(2).all(|p| p[1].0 >= p[0].1);
    ok.then_some(windows)
}

/// The clock of an enemy attack from its hit windows and weight: a held wind-up, shorter
/// windows, an unchanged gap between them, a longer recovery.
fn attack_clock(clip: &Clip, w: Weight) -> Result<Clock, String> {
    let d = clip.duration();
    let windows =
        usable_windows(clip).ok_or_else(|| format!("{}: no usable hit window", clip.name))?;
    // Each phase is a knot `(remixed s, source s)`; a phase too short to matter is dropped.
    fn phase(knots: &mut Vec<(f32, f32)>, length: f32, src: f32) {
        let (at, from) = *knots.last().unwrap();
        if src - from > 1e-4 && length > 1e-4 {
            knots.push((at + length, src));
        }
    }
    let mut knots = vec![(0.0, 0.0)];
    let first = windows[0].0;
    if first >= 0.12 {
        // The first half of the wind-up runs slower: the hold the player reads.
        let cut = first * 0.55;
        phase(&mut knots, cut * w.tele * (1.0 + w.hold), cut);
    }
    let held = knots.last().unwrap().0;
    phase(&mut knots, first * w.tele - held, first);
    for (k, &(s, e)) in windows.iter().enumerate() {
        phase(&mut knots, (e - s) * w.strike, e);
        if let Some(following) = windows.get(k + 1) {
            phase(&mut knots, following.0 - e, following.0);
        }
    }
    let last_end = windows[windows.len() - 1].1;
    phase(&mut knots, (d - last_end) * w.rec, d);
    let clock = Clock::new(knots)?;
    if !(0.9 * d..=1.4 * d).contains(&clock.length()) {
        return Err(format!(
            "{}: remix length {:.3}s out of range",
            clip.name,
            clock.length()
        ));
    }
    Ok(clock)
}

/// A clip resampled through `clock` at [`FPS`], with its root motion, flags and hit windows
/// carried through the same clock.
fn retime(source: &Clip, clock: &Clock) -> Clip {
    let mut out = source.clone();
    out.fps = FPS;
    let count = (clock.length() * FPS).round() as usize + 1;
    out.frames = (0..count)
        .map(|i| {
            if i == 0 {
                return source.frames[0].clone();
            }
            if i + 1 == count {
                return source.frames.last().unwrap().clone();
            }
            let old = clock.source_at(i as f32 / FPS);
            (0..source.bones())
                .map(|b| {
                    let (t, r, s) = sample(source, old, b);
                    Trs {
                        t: t.to_array(),
                        r: r.to_array(),
                        s: s.to_array(),
                    }
                })
                .collect()
        })
        .collect();
    let rm = &source.root_motion;
    out.root_motion = if rm.len() < 2 {
        rm.clone()
    } else {
        (0..count)
            .map(|i| {
                if i == 0 {
                    return rm[0];
                }
                if i + 1 == count {
                    return *rm.last().unwrap();
                }
                let f = clock.source_at(i as f32 / FPS) * source.fps;
                let j = (f as usize).min(rm.len() - 2);
                let u = f - j as f32;
                std::array::from_fn(|axis| rm[j][axis] + (rm[j + 1][axis] - rm[j][axis]) * u)
            })
            .collect()
    };
    for flag in &mut out.flags {
        flag.start = clock.target_at(flag.start);
        flag.end = clock.target_at(flag.end);
    }
    for hit in &mut out.hits {
        hit.start = clock.at_source(hit.start);
        hit.end = clock.at_source(hit.end);
    }
    out
}

/// An enemy attack remixed: retimed through [`attack_clock`], coiled, sprung.
fn remix_attack(source: &Clip, rig: &Rig, w: Weight, hand: usize) -> Result<(Clip, Clock), String> {
    rig.check(source)?;
    let clock = attack_clock(source, w)?;
    let first = source
        .hits
        .iter()
        .map(|h| h.start)
        .reduce(f32::min)
        .unwrap();
    let last = source.hits.iter().map(|h| h.end).reduce(f32::max).unwrap();
    let swing = Swing::measure(source, rig, hand, first, last);
    let mut out = retime(source, &clock);
    let p = Phases {
        strike: clock.at_source(first),
        end: clock.at_source(last.min(source.duration() - 1e-3)),
        len: clock.length(),
    };
    for (i, frame) in out.frames.iter_mut().enumerate() {
        coil_frame(frame, &source.driven, rig, w, swing, p, i as f32 / FPS);
    }
    let shield = shield_of(&source.parents, hand, rig);
    follow(&mut out.frames, source, rig, &shield, ATTACK, FPS, false);
    out.frames[0] = source.frames[0].clone();
    *out.frames.last_mut().unwrap() = source.frames.last().unwrap().clone();
    Ok((out, clock))
}

/// A clip of the player's skeleton (`human_male`), dressed in place: timing is the DS clip's.
/// Called from [`crate::motion_clip::read_clip`]; does nothing unless `WOWSOULS_MOTION=1`.
pub(crate) fn dress_player(clip: &mut Clip, sub: &str) {
    if !enabled() || sub != "human_male" {
        return;
    }
    match dress(clip, &HUMAN_MALE, kind_of_player(clip), HUMAN_MALE.hand) {
        Ok(dressed) => *clip = dressed,
        Err(why) => warn!("motion remix: keeping {} as it was: {why}", clip.name),
    }
}

/// One kit's replacements, all or nothing.
/// A remixed attack: (index, WoW clip, DS clip when it carries a grip, shock, blast).
type RemixedAttack = (
    usize,
    Clip,
    Option<Clip>,
    Option<crate::monster::Shock>,
    Option<crate::monster::Shock>,
);

/// A move's ground shock or blast at the remixed clock: it lands when the source moment it marks is shown.
fn retime_shock(
    shock: Option<crate::monster::Shock>,
    clock: &Clock,
) -> Option<crate::monster::Shock> {
    shock.map(|s| crate::monster::Shock {
        at: clock.target_at(s.at),
        ..s
    })
}

#[derive(Default)]
struct Plan {
    /// Each remixed attack: its clips and its ground shock and blast moved to the new clock.
    attacks: Vec<RemixedAttack>,
    /// Attacks that keep their timing (no usable hit window): the springs only.
    still: Vec<(usize, Clip)>,
    reacts: Vec<(usize, Clip)>,
    deaths: Vec<(usize, Clip)>,
    parried: Option<Clip>,
}

fn plan(kit: &MonsterClips, rig: &Rig) -> Result<Plan, String> {
    let mut plan = Plan::default();
    for (i, a) in kit.attacks.iter().enumerate() {
        if wolf_motion::authored(&kit.name, &a.spec.name) {
            continue;
        }
        let hand = usize::from(a.reach.map_or(kit.reach.hand, |r| r.hand));
        if usable_windows(&a.wow).is_none() {
            plan.still.push((i, dress(&a.wow, rig, Kind::Misc, hand)?));
            continue;
        }
        let w = weight_of_move(a.spec.level, a.spec.poise);
        let (wow, clock) = remix_attack(&a.wow, rig, w, hand)?;
        // The weapon is turned in the hand from the DS clip at the same time: keep them in step.
        let reference = a.grip.is_some().then(|| retime(&a.reference, &clock));
        plan.attacks.push((
            i,
            wow,
            reference,
            retime_shock(a.shock, &clock),
            retime_shock(a.blast, &clock),
        ));
    }
    for (i, r) in kit.reacts.iter().enumerate() {
        plan.reacts.push((i, dress(&r.wow, rig, Kind::Hurt, 0)?));
    }
    for (i, d) in kit.deaths.iter().enumerate() {
        plan.deaths.push((i, dress(&d.wow, rig, Kind::Hurt, 0)?));
    }
    if let Some(p) = &kit.parried {
        plan.parried = Some(dress(&p.wow, rig, Kind::Hurt, 0)?);
    }
    Ok(plan)
}

/// Remix a loaded kit: wolves' authored bite and lunge first, then everything else the kit
/// has. Atomic: a kit that does not look as expected keeps all its original clips.
pub(super) fn apply_loaded(kit: &mut MonsterClips) {
    wolf_motion::apply_loaded(kit);
    let Some(rig) = rig_of_kit(&kit.name) else {
        return;
    };
    let plan = match plan(kit, rig) {
        Ok(plan) => plan,
        Err(why) => {
            warn!("motion remix: keeping the original {} kit: {why}", kit.name);
            return;
        }
    };
    let (attacks, others) = (plan.attacks.len(), plan.reacts.len() + plan.deaths.len());
    for (i, wow, reference, shock, blast) in plan.attacks {
        let a = &mut kit.attacks[i];
        a.shock = shock;
        a.blast = blast;
        let before = (a.spec.length, a.spec.hit);
        a.spec.length = wow.duration();
        a.spec.hits = wow.hits.iter().map(|h| (h.start, h.end)).collect();
        a.spec.hit = a.spec.hits[0];
        info!(
            "motion remix: {} {} {:.3}s hit {:.3}-{:.3} -> {:.3}s hit {:.3}-{:.3}",
            kit.name,
            a.spec.name,
            before.0,
            before.1 .0,
            before.1 .1,
            a.spec.length,
            a.spec.hit.0,
            a.spec.hit.1
        );
        a.wow = wow;
        if let Some(reference) = reference {
            a.reference = reference;
        }
    }
    for (i, clip) in plan.still {
        kit.attacks[i].wow = clip;
    }
    for (i, clip) in plan.reacts {
        kit.reacts[i].wow = clip;
    }
    for (i, clip) in plan.deaths {
        kit.deaths[i].wow = clip;
    }
    if let (Some(clip), Some(p)) = (plan.parried, kit.parried.as_mut()) {
        p.wow = clip;
    }
    info!(
        "motion remix: {} dressed: {attacks} attacks, {others} flinches and deaths",
        kit.name
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shock_moves_with_its_remixed_clip() {
        // The source's first second is shown over 1.5 s, the rest at its own pace.
        let clock = Clock::new(vec![(0.0, 0.0), (1.5, 1.0), (2.5, 2.0)]).unwrap();
        let shock = crate::monster::Shock {
            at: 1.0,
            radius: 3.0,
            damage: 40.0,
            level: 6,
            poise: 300.0,
            knock: 1.6,
        };
        let moved = retime_shock(Some(shock), &clock).unwrap();
        assert!((moved.at - 1.5).abs() < 1e-3, "{}", moved.at);
        assert_eq!((moved.radius, moved.level), (3.0, 6));
        assert!(retime_shock(None, &clock).is_none());
    }

    /// A rig of 100 bones: root 0, a trunk 20 -> 22 -> 48 (neck) -> 64 (head) -> 70 (jaw), a hand
    /// 97 under the head, and a bone 5 standing for a leg.
    fn rig() -> Rig {
        Rig {
            spine: &[20, 22],
            neck: Some(48),
            head: 64,
            jaw: Some(70),
            tail: &[(10, 1.0), (11, 0.8)],
            ears: &[],
            hand: 97,
        }
    }

    fn clip(frames: usize, fps: f32, hits: &[(f32, f32)]) -> Clip {
        let mut parents = vec![0i16; 100];
        parents[0] = -1;
        parents[22] = 20;
        parents[48] = 22;
        parents[64] = 48;
        parents[70] = 64;
        parents[97] = 22;
        parents[11] = 10;
        let seconds = (frames - 1) as f32 / fps;
        let pose = |i: usize| -> Vec<Trs> {
            let t = i as f32 / (frames - 1) as f32;
            (0..100)
                .map(|b| {
                    let mut tr = Trs::IDENTITY;
                    tr.t = [0.0, b as f32 * 0.01, 0.0];
                    if b == 97 {
                        // The hand sweeps from the left to the right and down.
                        tr.t = [-0.5 + t, 0.8 - 0.6 * t, -0.2];
                    }
                    if b == 22 {
                        tr.r = Quat::from_rotation_y(0.3 * (t * std::f32::consts::PI).sin())
                            .to_array();
                    }
                    tr
                })
                .collect()
        };
        Clip {
            name: "weapon_99_attack_01".into(),
            fps,
            model_space: false,
            parents,
            driven: vec![true; 100],
            frames: (0..frames).map(pose).collect(),
            root_motion: (0..frames)
                .map(|i| [0.0, 0.0, -(i as f32) * 0.05, 0.0])
                .collect(),
            flags: vec![wscl::FlagWindow {
                flag: wscl::FLAG_CHAIN,
                start: seconds * 0.7,
                end: seconds,
            }],
            hits: hits
                .iter()
                .map(|&(start, end)| wscl::HitWindow {
                    judge: 100,
                    start,
                    end,
                })
                .collect(),
        }
    }

    fn angle(a: &Trs, b: &Trs) -> f32 {
        rot(a).angle_between(rot(b)).to_degrees()
    }

    #[test]
    fn the_clock_is_monotone_and_inverts() {
        let c = clip(61, 30.0, &[(0.8, 1.2)]);
        let clock = attack_clock(&c, HEAVY).unwrap();
        let mut last = -1.0;
        for i in 0..=400 {
            let s = clock.source_at(clock.length() * i as f32 / 400.0);
            assert!(s > last || i == 0, "source time must advance");
            last = s;
        }
        for s in [0.0, 0.3, 0.8, 1.0, 1.2, 1.7, c.duration()] {
            assert!(
                (clock.source_at(clock.target_at(s)) - s).abs() < 1e-3,
                "{s}"
            );
        }
        assert!((clock.source_at(clock.length()) - c.duration()).abs() < 1e-5);
    }

    #[test]
    fn an_attack_is_held_then_snapped_then_recovers_longer() {
        let c = clip(61, 30.0, &[(0.8, 1.2)]);
        let clock = attack_clock(&c, HEAVY).unwrap();
        let strike = clock.at_source(0.8);
        let end = clock.at_source(1.2);
        assert!((strike - 0.8 * HEAVY.tele).abs() < 1e-5);
        assert!((end - strike - 0.4 * HEAVY.strike).abs() < 1e-5);
        assert!((clock.length() - end - (c.duration() - 1.2) * HEAVY.rec).abs() < 1e-5);
        // The first half of the wind-up runs slower than the source, the second faster.
        let cut = clock.target_at(0.8 * 0.55);
        assert!(cut > 0.8 * 0.55 * HEAVY.tele);
        assert!(clock.source_at(0.2) < 0.2);
    }

    #[test]
    fn a_double_keeps_the_gap_and_both_windows_in_order() {
        let c = clip(91, 30.0, &[(0.5, 0.7), (1.2, 1.5)]);
        let clock = attack_clock(&c, LIGHT).unwrap();
        let out = retime(&c, &clock);
        assert_eq!(out.hits.len(), 2);
        assert!(out.hits[0].start < out.hits[0].end && out.hits[0].end < out.hits[1].start);
        assert!((out.hits[1].start - out.hits[0].end - 0.5).abs() < 1e-4);
        assert!((out.hits[0].end - out.hits[0].start - 0.2 * LIGHT.strike).abs() < 1e-4);
        let flag = out.flags[0];
        assert!(flag.start > out.hits[1].end - 1e-3 && flag.end <= out.duration() + 1e-3);
    }

    #[test]
    fn a_retimed_attack_keeps_its_ends_translations_and_distance() {
        let c = clip(61, 30.0, &[(0.8, 1.2)]);
        let (out, clock) = remix_attack(&c, &rig(), HEAVY, 97).unwrap();
        assert_eq!(out.frames[0], c.frames[0]);
        assert_eq!(out.frames.last(), c.frames.last());
        assert_eq!(out.root_motion.last(), c.root_motion.last());
        assert_eq!(out.driven, c.driven);
        assert!((out.duration() - clock.length()).abs() < 0.02);
        let last = out.frames.len() - 1;
        for (i, frame) in out.frames.iter().enumerate().take(last).skip(1) {
            let old = clock.source_at(i as f32 / FPS);
            for (b, tr) in frame.iter().enumerate() {
                let (t, _, s) = sample(&c, old, b);
                assert!(
                    (Vec3::from_array(tr.t) - t).length() < 1e-5,
                    "bone {b} moved"
                );
                assert!((Vec3::from_array(tr.s) - s).length() < 1e-5);
                assert!((Quat::from_array(tr.r).length() - 1.0).abs() < 1e-4);
                // Only the head chain, the trunk and the tail turn; the legs and the hand's own
                // rotation are the source's.
                if ![20, 22, 48, 64, 70, 10, 11].contains(&b) {
                    assert!(
                        angle(
                            tr,
                            &Trs {
                                r: sample(&c, old, b).1.to_array(),
                                ..Trs::IDENTITY
                            }
                        ) < 1e-3,
                        "bone {b} turned"
                    );
                }
            }
        }
    }

    #[test]
    fn the_coil_winds_away_from_the_swing_and_releases_through_it() {
        let c = clip(61, 30.0, &[(0.8, 1.2)]);
        let swing = Swing::measure(&c, &rig(), 97, 0.8, 1.2);
        assert!(swing.lateral > 0.0, "the hand goes to the right");
        let p = Phases {
            strike: 1.0,
            end: 1.4,
            len: 2.0,
        };
        assert_eq!(coil(0.0, p), 0.0);
        assert_eq!(coil(2.0, p), 0.0);
        // Wound up and held before the blow, let go by its first frame, nothing during the
        // windows, then the follow-through past neutral and the settle.
        assert!((coil(0.5, p) - 1.0).abs() < 1e-5);
        assert_eq!(coil(1.0, p), 0.0);
        for i in 0..=40 {
            assert_eq!(
                coil(1.0 + 0.4 * i as f32 / 40.0, p),
                0.0,
                "inside the hit window"
            );
        }
        assert!((coil(1.4 + 0.2 * 0.6, p) + FOLLOW).abs() < 1e-5);
        assert_eq!(coil(1.4 + 0.7 * 0.6, p), 0.0);
        let mut frame = c.frames[10].clone();
        let before = rot(&frame[22]);
        coil_frame(&mut frame, &c.driven, &rig(), HEAVY, swing, p, 0.5);
        // Winding up for a swing to the right turns the chest to the left (+Y).
        let delta = rot(&frame[22]) * before.inverse();
        assert!(delta.to_scaled_axis().y > 0.0);
    }

    #[test]
    fn the_springs_trail_a_quick_turn_and_settle_to_the_source() {
        let mut c = clip(31, 30.0, &[]);
        // A fast chest turn in the middle of the clip.
        for (i, f) in c.frames.iter_mut().enumerate() {
            let a = if (10..14).contains(&i) {
                0.5 * (i - 9) as f32 / 4.0
            } else if i >= 14 {
                0.5
            } else {
                0.0
            };
            f[22].r = Quat::from_rotation_y(a).to_array();
        }
        let out = dress(&c, &rig(), Kind::Hurt, 97).unwrap();
        assert_eq!(out.frames[0], c.frames[0]);
        assert_eq!(out.frames.last(), c.frames.last());
        let world_yaw = |clip: &Clip, i: usize| {
            (parent_rot(&clip.frames[i], &clip.parents, 64) * rot(&clip.frames[i][64]))
                .to_scaled_axis()
                .y
        };
        // Mid-turn the head has turned less than the source's; it never strays past the neck's
        // and its own cap together.
        assert!(world_yaw(&out, 12) < world_yaw(&c, 12) - 0.01);
        let cap = (HURT.neck.max + HURT.head.max).to_radians() + 0.05;
        for i in 0..c.frames.len() {
            assert!((world_yaw(&out, i) - world_yaw(&c, i)).abs() < cap);
        }
        // Nothing outside the head chain moved.
        for i in 0..c.frames.len() {
            for b in [0usize, 5, 20, 22, 97] {
                assert_eq!(out.frames[i][b], c.frames[i][b], "bone {b} frame {i}");
            }
        }
    }

    #[test]
    fn loops_keep_their_seam_and_only_the_head_chain_trails() {
        let mut walk = clip(31, 30.0, &[]);
        walk.name = "body_run".into();
        assert_eq!(kind_of_player(&walk), Kind::Loop);
        // A closed loop: a quick chest sway that repeats its first frame at the end.
        for (i, f) in walk.frames.iter_mut().enumerate() {
            f[97].t = [0.2, 0.7, -0.2];
            let a = 0.4 * (std::f32::consts::TAU * i as f32 / 30.0).sin();
            f[22].r = Quat::from_rotation_y(a).to_array();
        }
        walk.frames[30] = walk.frames[0].clone();
        let out = dress(&walk, &rig(), Kind::Loop, 97).unwrap();
        assert_eq!(out.frames[0], out.frames[30], "the seam stays closed");
        assert_eq!((out.fps, out.frames.len()), (walk.fps, walk.frames.len()));
        let mut trail = 0.0_f32;
        for (a, b) in out.frames.iter().zip(&walk.frames) {
            for bone in 0..100 {
                if [48, 64, 70].contains(&bone) {
                    trail = trail.max(angle(&a[bone], &b[bone]));
                } else {
                    assert_eq!(a[bone], b[bone], "bone {bone}");
                }
            }
        }
        assert!(trail > 0.3, "the head trails the sway: {trail}");
        assert!(trail < LOOP.neck.max + LOOP.head.max + 0.5);
    }

    #[test]
    fn unlike_rigs_are_left_alone() {
        let mut odd = clip(31, 30.0, &[]);
        odd.parents[64] = 20;
        assert!(dress(&odd, &rig(), Kind::Hurt, 97).is_err());
        let mut small = clip(31, 30.0, &[]);
        small.parents.truncate(60);
        small.frames.iter_mut().for_each(|f| f.truncate(60));
        small.driven.truncate(60);
        assert!(dress(&small, &rig(), Kind::Hurt, 97).is_err());
    }

    /// Model-space position of a bone in one frame.
    fn point(frame: &[Trs], parents: &[i16], bone: usize) -> Vec3 {
        let mut p = Vec3::from_array(frame[bone].t);
        let mut cur = parents[bone];
        while let Ok(i) = usize::try_from(cur) {
            let tr = frame[i];
            p = Vec3::from_array(tr.t) + rot(&tr) * (Vec3::from_array(tr.s) * p);
            cur = parents[i];
        }
        p
    }

    fn installed() -> Option<std::path::PathBuf> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source");
        assert!(
            dir.join("wolf/body_attack_01.wsanim").is_file(),
            "project wolf clips are required"
        );
        Some(dir)
    }

    /// What a remix may change in a frame: the rig's own bones. Everything else (legs, arms,
    /// translations, scales) has to be the source's at the time the clock shows.
    fn assert_untouched(
        source: &Clip,
        out: &Clip,
        rig: &Rig,
        feet: &[usize],
        i: usize,
        old: f32,
        what: &str,
    ) -> f32 {
        let mine: Vec<usize> = rig.bones().collect();
        let mut original = Vec::new();
        let mut spread = 0.0_f32;
        for (b, tr) in out.frames[i].iter().enumerate() {
            let (t, r, s) = sample(source, old, b);
            original.push(Trs {
                t: t.to_array(),
                r: r.to_array(),
                s: s.to_array(),
            });
            assert!(
                (Vec3::from_array(tr.t) - t).length() < 1e-4,
                "{what} bone {b} moved"
            );
            assert!(
                (Vec3::from_array(tr.s) - s).length() < 1e-4,
                "{what} bone {b} scaled"
            );
            assert!(
                (Quat::from_array(tr.r).length() - 1.0).abs() < 1e-4,
                "{what} bone {b}"
            );
            if mine.contains(&b) {
                spread = spread.max(rot(tr).angle_between(r.normalize()).to_degrees());
            } else {
                assert!(
                    rot(tr).angle_between(r.normalize()) < 2e-3,
                    "{what} bone {b} turned"
                );
            }
        }
        for &f in feet {
            let drift = (point(&out.frames[i], &out.parents, f)
                - point(&original, &source.parents, f))
            .length();
            assert!(drift < 1e-3, "{what} frame {i} foot {f}: {drift}");
        }
        spread
    }

    #[test]
    fn installed_kits_keep_their_feet_arms_and_events() {
        let Some(dir) = installed() else { return };
        let text = crate::motion_clip::checked_manifest(&dir).unwrap();
        let mut roster = super::super::parse_roster(&text, &dir).unwrap();
        let feet: &[(&str, &[usize])] = &[
            ("kobold", &[47, 48]),
            ("geomancer", &[47, 48]),
            ("wolf", &[43, 44, 62, 63]),
            ("direwolf", &[53, 54, 58, 59]),
            ("thug", &[68, 70, 80, 81]),
            ("caster", &[68, 70, 80, 81]),
            ("caster_female", &[66, 68, 79, 80]),
            ("human_axe_2h", &[68, 70, 80, 81]),
            ("human_bandit", &[68, 70, 80, 81]),
            ("garrick", &[68, 70, 80, 81]),
            ("thug_female", &[66, 68, 79, 80]),
            ("human_bandit_female", &[66, 68, 79, 80]),
            ("hogger", &[43, 44]),
            ("hogger_knight", &[43, 44, 52, 53]),
            ("gnoll", &[43, 44, 52, 53]),
            ("murloc", &[43, 44, 50, 51]),
            ("boar", &[41, 42, 55, 56]),
            ("boar_dog", &[41, 42, 55, 56]),
            ("bear", &[50, 51, 41, 42]),
            ("bear_dog", &[50, 51, 41, 42]),
            ("goblin", &[54, 56, 69, 70]),
            ("ambusher", &[38, 40, 45, 46]),
            ("minespider", &[]),
            ("tarantula", &[]),
            ("goldtooth", &[47, 48]),
            ("ruklar", &[47, 48]),
            ("narg", &[47, 48]),
            ("gruff", &[43, 44, 52, 53]),
            ("fedfennel", &[43, 44, 52, 53]),
            ("thuros", &[68, 70, 80, 81]),
            ("morgan", &[68, 70, 80, 81]),
            ("dead_tooth_jack", &[68, 70, 80, 81]),
            ("james_clark", &[68, 70, 80, 81]),
            ("erlan", &[68, 70, 80, 81]),
            ("morgaine", &[66, 68, 79, 80]),
            ("surena", &[66, 68, 79, 80]),
            ("princess", &[41, 42, 55, 56]),
            ("mother_fang", &[]),
        ];
        let mut names: Vec<String> = roster.0.keys().cloned().collect();
        names.sort();
        for name in names {
            let kit = roster.0.get_mut(&name).unwrap();
            let feet = feet.iter().find(|f| f.0 == name).unwrap().1;
            let rig = rig_of_kit(&name).unwrap();
            let before: Vec<Clip> = kit.attacks.iter().map(|a| a.wow.clone()).collect();
            let specs: Vec<_> = kit.attacks.iter().map(|a| a.spec.clone()).collect();
            let grips: Vec<_> = kit.attacks.iter().map(|a| a.reference.clone()).collect();
            let reacts: Vec<Clip> = kit.reacts.iter().map(|r| r.wow.clone()).collect();
            let deaths: Vec<Clip> = kit.deaths.iter().map(|d| d.wow.clone()).collect();
            if let Err(why) = plan(kit, rig) {
                panic!("{name}: the remix would keep the original kit: {why}");
            }
            apply_loaded(kit);
            for (k, a) in kit.attacks.iter().enumerate() {
                let (src, out) = (&before[k], &a.wow);
                let what = format!("{name} {}", a.spec.name);
                assert_eq!(out.frames[0], src.frames[0], "{what}");
                assert_eq!(out.frames.last(), src.frames.last(), "{what}");
                assert_eq!(out.root_motion.last(), src.root_motion.last(), "{what}");
                assert_eq!(out.driven, src.driven);
                assert_eq!(a.spec.damage, specs[k].damage);
                assert_eq!(a.spec.poise, specs[k].poise);
                assert_eq!(a.spec.radius, specs[k].radius);
                if usable_windows(src).is_none() {
                    assert_eq!(out.frames.len(), src.frames.len());
                    assert_eq!(a.spec, specs[k]);
                    for i in 0..out.frames.len() {
                        assert_untouched(src, out, rig, feet, i, i as f32 / src.fps, &what);
                    }
                    println!(
                        "remix {what}: hit window outside its clip, timing kept, springs only"
                    );
                    continue;
                }
                assert!((a.spec.length - out.duration()).abs() < 1e-4);
                assert_eq!(
                    a.spec.hits,
                    out.hits
                        .iter()
                        .map(|h| (h.start, h.end))
                        .collect::<Vec<_>>()
                );
                for h in &out.hits {
                    assert!(
                        0.0 <= h.start && h.start < h.end && h.end <= out.duration() + 0.02,
                        "{what}"
                    );
                }
                assert!(
                    out.hits.windows(2).all(|p| p[0].end <= p[1].start + 1e-4),
                    "{what}"
                );
                for f in &out.flags {
                    assert!(
                        f.start <= f.end + 1e-4 && f.end <= out.duration() + 0.02,
                        "{what} flag {f:?} of {:.3}s from {:?} of {:.3}s",
                        out.duration(),
                        src.flags,
                        src.duration()
                    );
                }
                if wolf_motion::authored(&name, &a.spec.name) {
                    println!(
                        "remix {what}: authored wolf recipe {:.3}s -> {:.3}s",
                        src.duration(),
                        out.duration()
                    );
                    continue;
                }
                let w = weight_of_move(a.spec.level, a.spec.poise);
                let clock = attack_clock(src, w).unwrap();
                let mut trunk = 0.0_f32;
                for i in 1..out.frames.len() - 1 {
                    let old = clock.source_at(i as f32 / FPS);
                    trunk = trunk.max(assert_untouched(src, out, rig, feet, i, old, &what));
                }
                if a.grip.is_some() {
                    // The weapon is turned from the DS clip at the same time: it moved with it.
                    assert!((grips[k].duration() - src.duration()).abs() < 1e-4);
                    assert!(
                        (a.reference.duration() - out.duration()).abs() < 0.02,
                        "{what} reference clip"
                    );
                } else {
                    assert_eq!(a.reference.frames, grips[k].frames, "{what} reference clip");
                }
                println!(
                    "remix {what}: {:.3}s hit {:.3}-{:.3} -> {:.3}s hit {:.3}-{:.3}, trunk/head turn up to {trunk:.1} deg",
                    src.duration(), specs[k].hit.0, specs[k].hit.1, out.duration(), a.spec.hit.0, a.spec.hit.1
                );
            }
            let flinches = reacts.iter().chain(&deaths).zip(
                kit.reacts
                    .iter()
                    .map(|r| &r.wow)
                    .chain(kit.deaths.iter().map(|d| &d.wow)),
            );
            let mut worst = 0.0_f32;
            for (k, (src, out)) in flinches.enumerate() {
                let what = format!("{name} flinch/death {k} {}", src.name);
                assert_eq!(out.frames[0], src.frames[0]);
                assert_eq!(out.frames.last(), src.frames.last());
                assert_eq!(out.root_motion, src.root_motion);
                assert_eq!(out.flags, src.flags);
                assert_eq!(out.hits, src.hits);
                assert_eq!(out.frames.len(), src.frames.len());
                for i in 0..out.frames.len() {
                    worst = worst.max(assert_untouched(
                        src,
                        out,
                        rig,
                        feet,
                        i,
                        i as f32 / src.fps,
                        &what,
                    ));
                }
            }
            println!("remix {name}: {} flinches and deaths, head chain off the source by up to {worst:.1} deg", reacts.len() + deaths.len());
        }
    }

    #[test]
    fn installed_player_clips_keep_their_timing_arms_and_legs() {
        let Some(dir) = installed() else { return };
        let sub = dir.clone();
        let mut names: Vec<_> = std::fs::read_dir(&sub)
            .unwrap()
            .filter_map(|e| {
                let path = e.ok()?.path();
                (path.extension()?.to_str()? == "wsanim")
                    .then(|| path.file_stem()?.to_str().map(String::from))?
            })
            .collect();
        names.sort();
        let feet = [68, 70, 80, 81];
        let (mut dressed, mut loops, mut kept) = (0, 0, 0);
        let mut worst = ("", 0.0_f32);
        for name in &names {
            let src = crate::motion_clip::read_clip(&dir, "human_male", name).unwrap();
            let kind = kind_of_player(&src);
            let out = match dress(&src, &HUMAN_MALE, kind, HUMAN_MALE.hand) {
                Ok(out) => out,
                Err(why) => {
                    println!("remix player {name}: kept ({why})");
                    kept += 1;
                    continue;
                }
            };
            if kind != Kind::Loop {
                assert_eq!(out.frames[0], src.frames[0], "{name}");
                assert_eq!(out.frames.last(), src.frames.last(), "{name}");
            }
            assert_eq!(
                (
                    out.fps,
                    out.root_motion.clone(),
                    out.flags.clone(),
                    out.hits.clone(),
                    out.frames.len()
                ),
                (
                    src.fps,
                    src.root_motion.clone(),
                    src.flags.clone(),
                    src.hits.clone(),
                    src.frames.len()
                ),
                "{name}: timing and events are the DS clip's"
            );
            if kind == Kind::Loop {
                loops += 1;
                // The seam of a loop is as closed as the DS clip's own.
                let seam = |c: &Clip| {
                    HUMAN_MALE
                        .bones()
                        .map(|b| angle(&c.frames[0][b], &c.frames[c.frames.len() - 1][b]))
                        .fold(0.0_f32, f32::max)
                };
                assert!(
                    seam(&out) <= seam(&src) + 0.5,
                    "{name}: seam {} against {}",
                    seam(&out),
                    seam(&src)
                );
            } else {
                dressed += 1;
            }
            let mut turn = 0.0_f32;
            for i in 0..out.frames.len() {
                turn = turn.max(assert_untouched(
                    &src,
                    &out,
                    &HUMAN_MALE,
                    &feet,
                    i,
                    i as f32 / src.fps,
                    name,
                ));
            }
            if turn > worst.1 {
                worst = (name, turn);
            }
        }
        println!(
            "remix player: {dressed} clips dressed plus {loops} loops, {kept} kept as they were; largest turn {:.1} deg in {}",
            worst.1, worst.0
        );
        assert!(
            dressed > 20 && loops > 20,
            "only {dressed} dressed, {loops} loops"
        );
    }

    /// How far the striking hand is from where the source clip puts it, over every frame of every
    /// hit window of `out` (`src_time` maps a time of `out` onto the source's). The blade is swept
    /// from this hand, so a hand that is off is a blow that misses.
    fn hand_off_in_hit_windows(
        src: &Clip,
        out: &Clip,
        hand: usize,
        src_time: &dyn Fn(f32) -> f32,
    ) -> f32 {
        let mut worst = 0.0_f32;
        for h in &out.hits {
            let (from, to) = (
                (h.start * out.fps).ceil() as usize,
                (h.end * out.fps) as usize,
            );
            for i in from..=to.min(out.frames.len() - 1) {
                let t = i as f32 / out.fps;
                let dressed = point(&out.frames[i], &out.parents, hand);
                let (mut original, mut cur) = (Vec3::ZERO, hand as i16);
                while let Ok(b) = usize::try_from(cur) {
                    let (tr, r, s) = sample(src, src_time(t), b);
                    original = tr + r * (s * original);
                    cur = src.parents[b];
                }
                worst = worst.max((dressed - original).length());
            }
        }
        worst
    }

    #[test]
    fn installed_player_blows_keep_the_hand_where_the_clip_puts_it_while_they_can_hit() {
        let Some(dir) = installed() else { return };
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| {
                let path = e.ok()?.path();
                (path.extension()?.to_str()? == "wsanim")
                    .then(|| path.file_stem()?.to_str().map(String::from))?
            })
            .collect();
        names.sort();
        let mut worst = (String::new(), 0.0_f32);
        let mut blows = 0;
        for name in &names {
            let src = crate::motion_clip::read_clip(&dir, "human_male", name).unwrap();
            let kind = kind_of_player(&src);
            if !matches!(kind, Kind::Attack(_)) {
                continue;
            }
            let out = dress(&src, &HUMAN_MALE, kind, HUMAN_MALE.hand).unwrap();
            let off = hand_off_in_hit_windows(&src, &out, HUMAN_MALE.hand, &|t| t);
            blows += 1;
            println!(
                "remix player blow {name}: hand off by {:.4} yd in its hit windows",
                off
            );
            if off > worst.1 {
                worst = (name.clone(), off);
            }
        }
        println!(
            "remix player blows: {blows}, worst {} {:.4} yd",
            worst.0, worst.1
        );
        assert!(blows >= 10);
        assert!(
            worst.1 < 1e-3,
            "{} moves the hand {:.4} yd inside a hit window",
            worst.0,
            worst.1
        );
    }

    #[test]
    fn installed_enemy_blows_keep_the_hand_where_the_clip_puts_it_while_they_can_hit() {
        let Some(dir) = installed() else { return };
        let text = crate::motion_clip::checked_manifest(&dir).unwrap();
        let roster = super::super::parse_roster(&text, &dir).unwrap();
        let mut worst = (String::new(), 0.0_f32);
        for (name, kit) in &roster.0 {
            let rig = rig_of_kit(name).unwrap();
            for a in &kit.attacks {
                if wolf_motion::authored(name, &a.spec.name) || usable_windows(&a.wow).is_none() {
                    continue;
                }
                let hand = usize::from(a.reach.map_or(kit.reach.hand, |r| r.hand));
                let w = weight_of_move(a.spec.level, a.spec.poise);
                let (out, clock) = remix_attack(&a.wow, rig, w, hand).unwrap();
                let off = hand_off_in_hit_windows(&a.wow, &out, hand, &|t| clock.source_at(t));
                println!(
                    "remix {name} {}: hand off by {off:.4} yd in its hit windows",
                    a.spec.name
                );
                if off > worst.1 {
                    worst = (format!("{name} {}", a.spec.name), off);
                }
            }
        }
        assert!(
            worst.1 < 1e-3,
            "{} moves the hand {:.4} yd inside a hit window",
            worst.0,
            worst.1
        );
    }

    #[test]
    fn a_player_clip_is_classified_by_its_action() {
        let mut c = clip(31, 30.0, &[]);
        for (name, kind) in [
            ("body_idle", Kind::Loop),
            ("shield_guard_hold", Kind::Loop),
            ("body_walk", Kind::Loop),
            ("body_sprint", Kind::Loop),
            ("body_lock_run_r", Kind::Loop),
            ("shield_guard_raise", Kind::Misc),
            ("body_backstep", Kind::Evade),
            ("body_roll", Kind::Evade),
            ("body_reaction_01", Kind::Hurt),
            ("body_death_05", Kind::Hurt),
            ("body_interaction_206", Kind::Misc),
            ("body_pickup_ground", Kind::Misc),
            ("body_chest_open", Kind::Misc),
            ("body_pickup_take", Kind::Misc),
            ("body_self_enchant", Kind::Misc),
            ("body_critical_response", Kind::Misc),
            ("body_emote_variant", Kind::Misc),
            ("bow_aim_release", Kind::Misc),
        ] {
            c.name = name.into();
            assert_eq!(kind_of_player(&c), kind, "{name}");
        }
        let hit = clip(31, 30.0, &[(0.4, 0.6)]);
        assert_eq!(kind_of_player(&hit), Kind::Attack(LIGHT));
        assert_eq!(
            kind_of_player(&clip(46, 30.0, &[(0.4, 0.6)])),
            Kind::Attack(HEAVY)
        );
        assert_eq!(
            kind_of_player(&clip(61, 30.0, &[(0.4, 0.6)])),
            Kind::Attack(BOSS)
        );
    }
}
