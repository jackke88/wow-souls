//! The hand-made DS c0000 ↔ WoW HumanMale bone table. Each entry pairs a bone on both
//! skeletons and an aim bone on each that defines the bone's direction for the bind alignment.
//! WoW indices from `benilla-extract m2bones Character\Human\Male\HumanMale.m2` (2026-10-07).

use glam::Vec3;

pub struct Pair {
    pub ds: &'static str,
    pub ds_aim: &'static str,
    pub wow: usize,
    pub wow_aim: usize,
    /// The aim is a child moved by this bone (not its parent): the direction error is checked.
    pub check: bool,
    /// The bind directions differ by pose (T-pose vs A-pose limbs): align them. Off where the
    /// difference is anatomy (pelvis, spine, neck, head, feet), which an alignment would turn
    /// into a posture change.
    pub align: bool,
    /// Pose-relative: the bone's rotation is the DS bone's change from the clip's FIRST frame, applied to the
    /// WoW bind pose, instead of its change from the DS bind (and no bind alignment). For a creature whose DS
    /// bind is not a standing pose (a spider's legs are spread flat in it): frame 0 is then the WoW stance.
    pub rel: bool,
}

const fn p(
    ds: &'static str,
    ds_aim: &'static str,
    wow: usize,
    wow_aim: usize,
    check: bool,
    align: bool,
) -> Pair {
    Pair {
        ds,
        ds_aim,
        wow,
        wow_aim,
        check,
        align,
        rel: false,
    }
}

/// A pose-relative pair ([`Pair::rel`]): no bind alignment, the DS bone's motion from the clip's first frame.
const fn pr(
    ds: &'static str,
    ds_aim: &'static str,
    wow: usize,
    wow_aim: usize,
    check: bool,
) -> Pair {
    Pair {
        ds,
        ds_aim,
        wow,
        wow_aim,
        check,
        align: false,
        rel: true,
    }
}

pub const HUMAN_MALE: &[Pair] = &[
    p("Pelvis", "L_Thigh", 21, 25, false, false),
    p("Spine", "Spine1", 20, 22, true, false),
    p("Spine1", "Neck", 22, 48, true, false),
    p("Neck", "Head", 48, 64, true, false),
    p("Head", "Neck", 64, 48, false, false),
    p("L_Clavicle", "L_UpperArm", 49, 65, true, true),
    p("L_UpperArm", "L_Forearm", 65, 78, true, true),
    p("L_Forearm", "L_Hand", 78, 95, true, true),
    p("L_Hand", "L_Finger1", 95, 105, true, true),
    p("R_Clavicle", "R_UpperArm", 50, 66, true, true),
    p("R_UpperArm", "R_Forearm", 66, 79, true, true),
    p("R_Forearm", "R_Hand", 79, 97, true, true),
    p("R_Hand", "R_Finger1", 97, 104, true, true),
    p("L_Thigh", "L_Calf", 25, 53, true, true),
    p("L_Calf", "L_Foot", 53, 68, true, true),
    p("L_Foot", "L_Toe0", 68, 80, true, false),
    p("R_Thigh", "R_Calf", 26, 54, true, true),
    p("R_Calf", "R_Foot", 54, 70, true, true),
    p("R_Foot", "R_Toe0", 70, 81, true, false),
];

/// A WoW skeleton's hips. The hip root (the hips' common parent) turns with the DS pelvis and
/// moves so that the midpoint of the WoW hip joints lands on the midpoint of the DS ones. The
/// anchor is the hip joints, not the waist: WoW's hip joints hang 0.083 yd below its waist
/// while DS's sit level with its pelvis, so a waist anchor lifts the legs ~0.15 yd once the
/// pelvis pitches mid-roll. The hip joints must hang off bones that turn with the hip root.
#[derive(Debug, Clone, Copy)]
pub struct Hips {
    pub wow_hip_root: usize,
    pub wow_hip_joints: [usize; 2],
    pub ds_hip_joints: [&'static str; 2],
    /// The body lifts and drops with the hips (a jump, a crouch, a collapse); see [`BodyLift`].
    /// `None`: the hips move alone, as before (a body whose torso hangs from a ground-fixed root
    /// stretches between the two when the pelvis travels).
    pub body_lift: Option<BodyLift>,
}

/// A WoW root bone the body lifts and drops on with the hips: everything that hangs from it (spine,
/// arms, head) rises by what the DS pelvis rose from its height in the clip's first frame, and the
/// hip root's own offset gives that much back, so the legs stay where they reach the ground. A
/// leap of the source creature, which is far larger, is shortened: the rise is kept in full up to
/// `full_to` (WoW yd, model units) and only `beyond` of it above that.
#[derive(Debug, Clone, Copy)]
pub struct BodyLift {
    pub root: usize,
    pub full_to: f32,
    pub beyond: f32,
}

impl BodyLift {
    /// The rise `rise` (yd, negative: a drop) comes to.
    pub fn shorten(self, rise: f32) -> f32 {
        if rise > self.full_to {
            self.full_to + (rise - self.full_to) * self.beyond
        } else {
            rise
        }
    }
}

pub const HUMAN_MALE_HIPS: Hips = Hips {
    wow_hip_root: 18,
    wow_hip_joints: [25, 26],
    ds_hip_joints: ["L_Thigh", "R_Thigh"],
    body_lift: None,
};

/// Hip → knee → ankle on both sides (left, right) of each skeleton: the leg lengths set the
/// DS → WoW scale, so stride and crouch follow the legs.
pub struct Legs {
    pub ds: &'static [[&'static str; 3]],
    pub wow: &'static [[usize; 3]],
}

pub const HUMAN_MALE_LEGS: Legs = Legs {
    ds: &[
        ["L_Thigh", "L_Calf", "L_Foot"],
        ["R_Thigh", "R_Calf", "R_Foot"],
    ],
    wow: &[[25, 53, 68], [26, 54, 70]],
};

/// Checks that bone directions alone cannot make: directions across the body (DS from, DS to,
/// WoW from, WoW to), and the WoW bones whose lowest one stands on the ground.
pub struct Checks {
    pub cross: &'static [(&'static str, &'static str, usize, usize)],
    pub wow_soles: &'static [usize],
    /// The soles' heights are compared as drift from their heights at bind (the WoW paw joints sit
    /// 0.1 yd above the ground, the DS digit joints 0.05 m: a constant offset, not an error).
    pub relative_soles: bool,
    /// Each frame the body is raised or lowered so that the lowest WoW sole stands as high above
    /// the ground as the lowest DS sole (relative to bind). For a creature whose legs are
    /// proportioned unlike its Dark Souls source's (the WoW wolf's are uniform, the dog's are not),
    /// whose paws would otherwise sink or hover as the legs bend; the foot gap then reports how far
    /// the body had to be moved.
    pub ground_lock: bool,
    /// The gate on the foot gap (yd).
    pub max_foot_gap: f32,
}

/// The foot gap a biped may show on contact frames (yd).
pub const DEFAULT_FOOT_GAP: f32 = 0.08;

pub const HUMAN_MALE_CHECKS: Checks = Checks {
    // The hip axis and the spine base.
    cross: &[("L_Thigh", "R_Thigh", 25, 26), ("Pelvis", "Spine", 21, 20)],
    // Ankles and toes.
    wow_soles: &[68, 70, 80, 81],
    relative_soles: false,
    ground_lock: false,
    max_foot_gap: DEFAULT_FOOT_GAP,
};

/// The blade's axis in the weapon bone's own axes. Measured on the straight sword's moves, not
/// read from the weapon model (whose blade runs along -Y, flipped by an attach transform that is
/// not read): the grip runs across the palm (the thumb is on +Y, the fingers split along Y), and
/// over the hit windows of all six moves +Y reaches furthest in front of the hand (the thrust
/// `a23_3300`: 1.59 m, against 0.20 m for -Y).
pub const DS_BLADE_AXIS: Vec3 = Vec3::Y;

/// How a DS skeleton's forward is read at bind.
pub enum Facing {
    /// A biped: the feet → toes directions, summed.
    FeetToToes([(&'static str, &'static str); 2]),
    /// A quadruped: along the body from one bone to another (the pelvis to the head).
    Along(&'static str, &'static str),
}

/// The names of a DS skeleton's landmark bones: they differ between `c0000` (`L_Foot`) and an
/// enemy (`L Foot`).
pub struct DsNames {
    /// The bone whose rotation turns the hips.
    pub pelvis: &'static str,
    /// The shoulders that define the DS left and right.
    pub left: &'static str,
    pub right: &'static str,
    /// What defines the DS facing.
    pub facing: Facing,
    /// The bones whose lowest one stands on the ground (the counterpart of `Checks::wow_soles`).
    pub soles: &'static [&'static str],
    /// The sword hand and the weapon bone that sits unrotated in it (the weapon bone's local
    /// rotation is the identity in every frame of every clip).
    pub hand: &'static str,
    pub weapon: &'static str,
    /// The head, whose push a flinch or a death is measured by, and the sword forearm, which a
    /// bare-handed creature's blow runs along.
    pub head: &'static str,
    pub forearm: &'static str,
}

/// The player's skeleton.
pub const C0000: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "L_UpperArm",
    right: "R_UpperArm",
    facing: Facing::FeetToToes([("L_Foot", "L_Toe0"), ("R_Foot", "R_Toe0")]),
    soles: &["L_Foot", "R_Foot", "L_Toe0", "R_Toe0"],
    hand: "R_Hand",
    weapon: "R_Weapon",
    head: "Head",
    forearm: "R_Forearm",
};

/// Everything the retarget needs about one WoW skeleton.
pub struct Rig<'a> {
    pub pairs: &'a [Pair],
    pub hips: Hips,
    pub legs: Legs,
    pub checks: Checks,
    pub names: &'a DsNames,
    /// A frame is a contact frame when the lowest DS sole is below this height (yd).
    pub contact_height: f32,
}

pub const HUMAN_MALE_RIG: Rig<'static> = Rig {
    pairs: HUMAN_MALE,
    hips: HUMAN_MALE_HIPS,
    legs: HUMAN_MALE_LEGS,
    checks: HUMAN_MALE_CHECKS,
    names: &C0000,
    contact_height: crate::retarget::CONTACT_HEIGHT,
};

/// The player (`c0000`, bone names with underscores) onto the kobold: [`KOBOLD`]'s pairs, for the staff cast the
/// Kobold Geomancer borrows.
pub const PLAYER_KOBOLD: &[Pair] = &[
    p("Pelvis", "L_Thigh", 15, 27, false, false),
    p("Spine", "Spine1", 14, 16, true, false),
    p("Spine1", "Neck", 16, 29, true, false),
    p("Neck", "Head", 29, 35, true, false),
    p("Head", "Neck", 35, 29, false, false),
    p("L_Clavicle", "L_UpperArm", 30, 36, true, true),
    p("L_UpperArm", "L_Forearm", 36, 44, true, true),
    p("L_Forearm", "L_Hand", 44, 52, true, true),
    p("L_Hand", "L_Finger1", 52, 58, true, true),
    p("R_Clavicle", "R_UpperArm", 31, 37, true, true),
    p("R_UpperArm", "R_Forearm", 37, 45, true, true),
    p("R_Forearm", "R_Hand", 45, 53, true, true),
    p("R_Hand", "R_Finger1", 53, 60, true, true),
    p("L_Thigh", "L_Calf", 27, 39, true, true),
    p("L_Calf", "L_Foot", 39, 47, true, true),
    p("L_Foot", "L_Toe0", 47, 55, true, false),
    p("R_Thigh", "R_Calf", 28, 40, true, true),
    p("R_Calf", "R_Foot", 40, 48, true, true),
    p("R_Foot", "R_Toe0", 48, 56, true, false),
];

pub const PLAYER_KOBOLD_RIG: Rig<'static> = Rig {
    pairs: PLAYER_KOBOLD,
    hips: Hips {
        wow_hip_root: 13,
        wow_hip_joints: [27, 28],
        ds_hip_joints: ["L_Thigh", "R_Thigh"],
        body_lift: None,
    },
    legs: Legs {
        ds: &[
            ["L_Thigh", "L_Calf", "L_Foot"],
            ["R_Thigh", "R_Calf", "R_Foot"],
        ],
        wow: &[[27, 39, 47], [28, 40, 48]],
    },
    checks: Checks {
        cross: &[
            ("L_Thigh", "R_Thigh", 27, 28),
            ("L_UpperArm", "R_UpperArm", 36, 37),
        ],
        wow_soles: &[47, 48],
        // The player's feet sit unlike the ghoul's on the kobold's short legs (0.12 yd off unlocked).
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: DEFAULT_FOOT_GAP,
    },
    names: &C0000,
    contact_height: KOBOLD_RIG.contact_height,
};

/// The ghoul's (`c2060`) skeleton: its bones have spaces in their names. Its soles are its ankles:
/// the kobold's feet are twice as long as the ghoul's scaled ones, so toes cannot be compared, only
/// where the ankles stand.
pub const GHOUL: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "L UpperArm",
    right: "R UpperArm",
    facing: Facing::FeetToToes([("L Foot", "L Toe0"), ("R Foot", "R Toe0")]),
    soles: &["L Foot", "R Foot"],
    hand: "R Hand",
    weapon: "R_Weapon",
    head: "Head",
    forearm: "R Forearm",
};

/// The ghoul → kobold pairs. Kobold indices from `benilla-extract m2bones Creature\Kobold\Kobold.m2`
/// (2026-10-08): 13 root (key bone 26), 14 spine base (4), 15 waist (5), 16 chest, 29 neck,
/// 35 head (6), 18 the hip root under the waist; shoulders 30/31, upper arms 36/37, forearms
/// 44/45, hands 52/53 (left is WoW +Y: the even index), fingers 58/60; thighs 27/28, knees 39/40,
/// ankles 47/48, toes 55/56.
pub const KOBOLD: &[Pair] = &[
    p("Pelvis", "L Thigh", 15, 27, false, false),
    p("Spine", "Spine1", 14, 16, true, false),
    p("Spine1", "Neck", 16, 29, true, false),
    p("Neck", "Head", 29, 35, true, false),
    p("Head", "Neck", 35, 29, false, false),
    p("L Clavicle", "L UpperArm", 30, 36, true, true),
    p("L UpperArm", "L Forearm", 36, 44, true, true),
    p("L Forearm", "L Hand", 44, 52, true, true),
    p("L Hand", "L Finger1", 52, 58, true, true),
    p("R Clavicle", "R UpperArm", 31, 37, true, true),
    p("R UpperArm", "R Forearm", 37, 45, true, true),
    p("R Forearm", "R Hand", 45, 53, true, true),
    p("R Hand", "R Finger1", 53, 60, true, true),
    p("L Thigh", "L Calf", 27, 39, true, true),
    p("L Calf", "L Foot", 39, 47, true, true),
    p("L Foot", "L Toe0", 47, 55, true, false),
    p("R Thigh", "R Calf", 28, 40, true, true),
    p("R Calf", "R Foot", 40, 48, true, true),
    p("R Foot", "R Toe0", 48, 56, true, false),
];

pub const KOBOLD_RIG: Rig<'static> = Rig {
    pairs: KOBOLD,
    hips: Hips {
        wow_hip_root: 13,
        wow_hip_joints: [27, 28],
        ds_hip_joints: ["L Thigh", "R Thigh"],
        body_lift: None,
    },
    legs: Legs {
        ds: &[
            ["L Thigh", "L Calf", "L Foot"],
            ["R Thigh", "R Calf", "R Foot"],
        ],
        wow: &[[27, 39, 47], [28, 40, 48]],
    },
    checks: Checks {
        // The hip axis and the shoulder line (the kobold's waist and spine base sit 12 mm apart:
        // no direction to compare).
        cross: &[
            ("L Thigh", "R Thigh", 27, 28),
            ("L UpperArm", "R UpperArm", 36, 37),
        ],
        wow_soles: &[47, 48],
        relative_soles: false,
        ground_lock: false,
        max_foot_gap: DEFAULT_FOOT_GAP,
    },
    names: &GHOUL,
    // The ankles stand 0.068 above the ground at rest.
    contact_height: 0.09,
};

/// The Taurus Demon's (`c2250`) skeleton: the ghoul's naming (spaces in the names). Its soles are
/// its feet joints (the gnoll's feet are not the demon's, so only where the ankles stand is
/// compared). The axe is in the LEFT hand (`L_weapon`), so the hand, weapon and forearm names are
/// the left ones: the hit volume of a blow runs from the left hand along the left forearm.
pub const TAURUS: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "L UpperArm",
    right: "R UpperArm",
    facing: Facing::FeetToToes([("L Foot", "L Toe0"), ("R Foot", "R Toe0")]),
    soles: &["L Foot", "R Foot"],
    hand: "L Hand",
    weapon: "L_weapon",
    head: "Head",
    forearm: "L Forearm",
};

/// The Taurus Demon → WoW gnoll (`GnollMelee`) pairs; indices from `benilla-extract m2bones
/// Creature\GnollMelee\GnollMelee.m2` (2026-10-13; WoW +Y is left, so a left bone has the positive Y
/// pivot): 10 root (key 26) → 11 spine base (key 4) → 13 chest, 12 waist (key 5) → 14 the hip root
/// (parent of the tail 18 and the hips 19 / 20); 13 → 15 neck → 30 head (key 6) → 39 jaw (key 7),
/// 16 / 17 the arm roots (left / right) → 31 / 32 elbows → 40 / 41 wrists → 48 / 50 fingers
/// (attachments 55 left hand, id 2, and 54 right hand, id 1); legs 19 → 34 → 43 → 52 and
/// 20 → 35 → 44 → 53 (hip, knee, ankle, toe); tail 18 → 33 → 42. The gnoll has no clavicles and
/// one spine bone fewer than the demon. The demon's clavicles and head hang from its `Neck`: so the
/// gnoll's waist (12) follows `Pelvis` (the hip axis then drifts 0.2 degrees; following the demon's
/// `Spine`, which the thighs hang from in the bind pose, it drifted 10-32), its spine base (11)
/// `Spine1` and its chest (13), which carries the shoulders, the `Neck`. The first pair of a WoW bone
/// drives it, a second one only measures: two demon bones on one gnoll bone showed as a 30 degree
/// drift. The gnoll's neck (15) is left to follow its chest. The tail's six bones map onto the
/// gnoll's three.
pub const GNOLL: &[Pair] = &[
    p("Pelvis", "L Thigh", 12, 19, false, false),
    p("Spine1", "Spine2", 11, 13, true, false),
    p("Neck", "Head", 13, 30, true, false),
    p("Head", "Neck", 30, 15, false, false),
    p("L UpperArm", "L Forearm", 16, 31, true, true),
    p("L Forearm", "L Hand", 31, 40, true, true),
    p("L Hand", "L Finger1", 40, 48, true, true),
    p("R UpperArm", "R Forearm", 17, 32, true, true),
    p("R Forearm", "R Hand", 32, 41, true, true),
    p("R Hand", "R Finger1", 41, 50, true, true),
    p("L Thigh", "L Calf", 19, 34, true, true),
    p("L Calf", "L Foot", 34, 43, true, true),
    p("L Foot", "L Toe0", 43, 52, true, false),
    p("R Thigh", "R Calf", 20, 35, true, true),
    p("R Calf", "R Foot", 35, 44, true, true),
    p("R Foot", "R Toe0", 44, 53, true, false),
    p("Tail", "Tail1", 18, 33, false, false),
    p("Tail2", "Tail3", 33, 42, false, false),
];

pub const GNOLL_RIG: Rig<'static> = Rig {
    pairs: GNOLL,
    hips: Hips {
        wow_hip_root: 14,
        wow_hip_joints: [19, 20],
        ds_hip_joints: ["L Thigh", "R Thigh"],
        // The torso hangs from the root (10); the demon's jump slam takes its pelvis ten metres up,
        // which on a gnoll is six yards: a leap of a yard and a half to two is plenty.
        body_lift: Some(BodyLift {
            root: 10,
            full_to: 0.6,
            beyond: 0.2,
        }),
    },
    legs: Legs {
        ds: &[
            ["L Thigh", "L Calf", "L Foot"],
            ["R Thigh", "R Calf", "R Foot"],
        ],
        wow: &[[19, 34, 43], [20, 35, 44]],
    },
    checks: Checks {
        // The hip axis only: the gnoll has no clavicles, so the demon's shoulder line (its upper
        // arms hang from clavicles that roll and shrug through every heavy swing) cannot be
        // compared with the gnoll's, whose arms are rigid on its chest (measured 2026-10-13: the
        // hip axis drifts 0.2 degrees over the kit's clips, the shoulder line 20 to 37).
        cross: &[("L Thigh", "R Thigh", 19, 20)],
        wow_soles: &[43, 44],
        relative_soles: false,
        ground_lock: false,
        max_foot_gap: DEFAULT_FOOT_GAP,
    },
    names: &TAURUS,
    // The demon's ankles stand 0.60 m above the ground at bind: a contact frame is within 0.15 of it.
    contact_height: 0.75,
};

/// The black dog's (`c3340`) skeleton, a quadruped: `LLeg_A_*` are the front legs, `LLeg_B_*` the
/// hind legs (collarbone or hip, two segments, palm, digits), `Spine1-3`, `Ribcage`, `Neck1-3`,
/// `Head`, `Jaw`, `Tail1-7`. The forward is the body's axis, pelvis to head. Its soles are the
/// first digit joints of the four paws (the palms are the wrist and the hock, well above the ground).
pub const BLACK_DOG: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "LLeg_A_1",
    right: "RLeg_A_1",
    facing: Facing::Along("Pelvis", "Head"),
    soles: &[
        "LLeg_A_Digit11",
        "RLeg_A_Digit11",
        "LLeg_B_Digit11",
        "RLeg_B_Digit11",
    ],
    hand: "Jaw",
    weapon: "Jaw",
    head: "Head",
    forearm: "Head",
};

/// The black dog → WoW wolf pairs. Wolf indices from `benilla-extract m2bones
/// Creature\Wolf\Wolf.m2` (2026-10-09; WoW +Y is left, so a pair's left bone has the positive Y
/// pivot): 0 root (key 26) with three children at its pivot: 19 → 22 waist (key 5) → 26 pelvis,
/// 20 → the front legs, 21 → 25 neck → 29 head (key 6) → 47 jaw (key 7), 45/46 ears; hind legs
/// 36/37 (hip) → 55/56 (stifle) → 60/61 (hock) → 62/63 (paw); front legs 23/24 (shoulder) →
/// 27/28 (elbow) → 43/44 (wrist) → 57/58 (toe); tail 38-42, each a child of the pelvis. The wolf
/// has one neck bone to the dog's three, one waist to its three spine bones, and a hock to the dog's
/// palm, so those map many to one and the head carries the DS head's final turn.
pub const WOLF: &[Pair] = &[
    // The body: the rear turns with the DS pelvis, the front group and the neck group with the ribcage.
    p("Pelvis", "Spine1", 26, 22, false, false),
    p("Spine1", "Spine2", 22, 26, false, false),
    p("Ribcage", "Neck1", 20, 23, false, false),
    p("Ribcage", "Neck1", 21, 25, false, false),
    p("Neck1", "Neck2", 25, 29, false, false),
    p("Head", "Jaw", 29, 47, false, false),
    p("Jaw", "Head", 47, 29, false, false),
    // Front legs: left (+Y) then right.
    p("LLeg_A_1", "LLeg_A_2", 23, 27, true, true),
    p("LLeg_A_2", "LLeg_A_Palm", 27, 43, true, true),
    p("LLeg_A_Palm", "LLeg_A_Digit21", 43, 57, true, true),
    p("RLeg_A_1", "RLeg_A_2", 24, 28, true, true),
    p("RLeg_A_2", "RLeg_A_Palm", 28, 44, true, true),
    p("RLeg_A_Palm", "RLeg_A_Digit21", 44, 58, true, true),
    // Hind legs.
    p("LLeg_B_1", "LLeg_B_2", 36, 55, true, true),
    p("LLeg_B_2", "LLeg_B_Palm", 55, 60, true, true),
    p("LLeg_B_Palm", "LLeg_B_Digit21", 60, 62, true, true),
    p("RLeg_B_1", "RLeg_B_2", 37, 56, true, true),
    p("RLeg_B_2", "RLeg_B_Palm", 56, 61, true, true),
    p("RLeg_B_Palm", "RLeg_B_Digit21", 61, 63, true, true),
    // The tail: five WoW segments, each turned like a DS segment along it.
    p("Tail1", "Tail2", 42, 41, false, false),
    p("Tail2", "Tail3", 41, 40, false, false),
    p("Tail4", "Tail5", 40, 39, false, false),
    p("Tail5", "Tail6", 39, 38, false, false),
    p("Tail7", "Tail6", 38, 39, false, false),
];

pub const WOLF_RIG: Rig<'static> = Rig {
    pairs: WOLF,
    hips: Hips {
        wow_hip_root: 0,
        wow_hip_joints: [36, 37],
        ds_hip_joints: ["LLeg_B_1", "RLeg_B_1"],
        body_lift: None,
    },
    // The hind legs, hip to paw, set the scale: they stand the body on the ground (the front legs
    // are relatively longer on the WoW wolf than on the dog: 1.65 against 1.3).
    legs: Legs {
        ds: &[
            ["LLeg_B_1", "LLeg_B_2", "LLeg_B_Digit11"],
            ["RLeg_B_1", "RLeg_B_2", "RLeg_B_Digit11"],
        ],
        wow: &[[36, 55, 62], [37, 56, 63]],
    },
    checks: Checks {
        // The hip line. The shoulders ride on moving collarbones and the WoW front hangs off the
        // root rather than off a chest bone, so neither their line nor the body's axis (three DS
        // spine bones against one WoW waist) says anything about the retarget.
        cross: &[("LLeg_B_1", "RLeg_B_1", 36, 37)],
        // The wrists and the paws.
        wow_soles: &[43, 44, 62, 63],
        relative_soles: true,
        ground_lock: true,
        // The body may be moved this far to stand the paws on the ground.
        max_foot_gap: 0.45,
    },
    names: &BLACK_DOG,
    // The DS digit joints stand 0.05 m up at rest, 0.065 yd scaled.
    contact_height: 0.10,
};

/// The black dog onto `Creature\DireWolf\DireWolf.m2` (61 bones, measured 2026-10-15).
/// Unlike Wolf, body root 19 owns a front/waist branch (20 -> 24 -> 30 pelvis) and the neck
/// branch (21 -> 25 head -> 35 jaw). All four legs therefore rise together from root 19.
/// The tail is four connected segments rather than Wolf's five siblings of the pelvis.
pub const DIREWOLF: &[Pair] = &[
    p("Pelvis", "Spine1", 30, 24, false, false),
    p("Spine1", "Spine2", 24, 30, false, false),
    p("Ribcage", "Neck1", 20, 22, false, false),
    p("Neck1", "Neck2", 21, 25, false, false),
    p("Head", "Jaw", 25, 35, false, false),
    p("Jaw", "Head", 35, 25, false, false),
    // Front legs, then hind legs: left (+Y) before right.
    p("LLeg_A_1", "LLeg_A_2", 22, 28, true, true),
    p("LLeg_A_2", "LLeg_A_Palm", 28, 42, true, true),
    p("LLeg_A_Palm", "LLeg_A_Digit21", 42, 53, true, true),
    p("RLeg_A_1", "RLeg_A_2", 23, 29, true, true),
    p("RLeg_A_2", "RLeg_A_Palm", 29, 43, true, true),
    p("RLeg_A_Palm", "RLeg_A_Digit21", 43, 54, true, true),
    p("LLeg_B_1", "LLeg_B_2", 44, 50, true, true),
    p("LLeg_B_2", "LLeg_B_Palm", 50, 55, true, true),
    p("LLeg_B_Palm", "LLeg_B_Digit21", 55, 58, true, true),
    p("RLeg_B_1", "RLeg_B_2", 45, 51, true, true),
    p("RLeg_B_2", "RLeg_B_Palm", 51, 56, true, true),
    p("RLeg_B_Palm", "RLeg_B_Digit21", 56, 59, true, true),
    p("Tail1", "Tail2", 46, 52, false, false),
    p("Tail3", "Tail4", 52, 57, false, false),
    p("Tail5", "Tail6", 57, 60, false, false),
    p("Tail7", "Tail6", 60, 57, false, false),
];

pub const DIREWOLF_RIG: Rig<'static> = Rig {
    pairs: DIREWOLF,
    hips: Hips {
        wow_hip_root: 19,
        wow_hip_joints: [44, 45],
        ds_hip_joints: ["LLeg_B_1", "RLeg_B_1"],
        body_lift: None,
    },
    legs: Legs {
        ds: &[
            ["LLeg_B_1", "LLeg_B_2", "LLeg_B_Digit11"],
            ["RLeg_B_1", "RLeg_B_2", "RLeg_B_Digit11"],
        ],
        wow: &[[44, 50, 58], [45, 51, 59]],
    },
    checks: Checks {
        cross: &[("LLeg_B_1", "RLeg_B_1", 44, 45)],
        // DireWolf's front toes lie below its wrists; Wolf's are above them.
        wow_soles: &[53, 54, 58, 59],
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: 0.45,
    },
    names: &BLACK_DOG,
    contact_height: 0.10,
};

/// The undead thief's (`c2520`) skeleton: the ghoul's naming (spaces: `L Thigh`) with the weapon
/// bone spelled `R_weapon`. Humanoid, so its soles are the feet and the toes, as the player's.
pub const THIEF: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "L UpperArm",
    right: "R UpperArm",
    facing: Facing::FeetToToes([("L Foot", "L Toe0"), ("R Foot", "R Toe0")]),
    soles: &["L Foot", "R Foot", "L Toe0", "R Toe0"],
    hand: "R Hand",
    weapon: "R_weapon",
    head: "Head",
    forearm: "R Forearm",
};

/// The `HUMAN_MALE` pairs for a DS skeleton that spells its bones with spaces (`L Thigh`).
pub const HUMAN_MALE_SPACED: &[Pair] = &[
    p("Pelvis", "L Thigh", 21, 25, false, false),
    p("Spine", "Spine1", 20, 22, true, false),
    p("Spine1", "Neck", 22, 48, true, false),
    p("Neck", "Head", 48, 64, true, false),
    p("Head", "Neck", 64, 48, false, false),
    p("L Clavicle", "L UpperArm", 49, 65, true, true),
    p("L UpperArm", "L Forearm", 65, 78, true, true),
    p("L Forearm", "L Hand", 78, 95, true, true),
    p("L Hand", "L Finger1", 95, 105, true, true),
    p("R Clavicle", "R UpperArm", 50, 66, true, true),
    p("R UpperArm", "R Forearm", 66, 79, true, true),
    p("R Forearm", "R Hand", 79, 97, true, true),
    p("R Hand", "R Finger1", 97, 104, true, true),
    p("L Thigh", "L Calf", 25, 53, true, true),
    p("L Calf", "L Foot", 53, 68, true, true),
    p("L Foot", "L Toe0", 68, 80, true, false),
    p("R Thigh", "R Calf", 26, 54, true, true),
    p("R Calf", "R Foot", 54, 70, true, true),
    p("R Foot", "R Toe0", 70, 81, true, false),
];

/// The thief onto the WoW human male.
pub const THIEF_RIG: Rig<'static> = Rig {
    pairs: HUMAN_MALE_SPACED,
    hips: Hips {
        wow_hip_root: 18,
        wow_hip_joints: [25, 26],
        ds_hip_joints: ["L Thigh", "R Thigh"],
        body_lift: None,
    },
    legs: Legs {
        ds: &[
            ["L Thigh", "L Calf", "L Foot"],
            ["R Thigh", "R Calf", "R Foot"],
        ],
        wow: &[[25, 53, 68], [26, 54, 70]],
    },
    checks: Checks {
        // The hip axis and the spine base.
        cross: &[("L Thigh", "R Thigh", 25, 26), ("Pelvis", "Spine", 21, 20)],
        wow_soles: &[68, 70, 80, 81],
        relative_soles: false,
        ground_lock: false,
        max_foot_gap: DEFAULT_FOOT_GAP,
    },
    names: &THIEF,
    contact_height: crate::retarget::CONTACT_HEIGHT,
};

/// The thief onto `Creature\HumanThief\HumanThief.m2` (64 bones, measured 2026-10-16 with `benilla-extract
/// m2bones`; WoW +Y is left), the Defias Ambusher's body: 10 root (key 26) → 11 spine (key 4) → 13 → 24 chest → 27
/// neck → 35 head (key 6) → 44 jaw; 10 → 12 waist (key 5) → 14 → hips 22 / 23 → knees 30 / 31 → ankles 38 / 40 →
/// toes 45 / 46; the chest → shoulders 28 / 29 → elbows 36 / 37 → wrists 47 / 48 → hands 52 / 53. No collar bones:
/// the upper arm turns the shoulder. The root is the hips' and the spine's common parent.
pub const HUMAN_THIEF_SPACED: &[Pair] = &[
    p("Pelvis", "L Thigh", 12, 22, false, false),
    p("Spine", "Spine1", 11, 13, true, false),
    p("Spine1", "Neck", 24, 27, true, false),
    p("Neck", "Head", 27, 35, true, false),
    p("Head", "Neck", 35, 27, false, false),
    p("L UpperArm", "L Forearm", 28, 36, true, true),
    p("L Forearm", "L Hand", 36, 47, true, true),
    p("L Hand", "L Finger1", 47, 52, true, true),
    p("R UpperArm", "R Forearm", 29, 37, true, true),
    p("R Forearm", "R Hand", 37, 48, true, true),
    p("R Hand", "R Finger1", 48, 53, true, true),
    p("L Thigh", "L Calf", 22, 30, true, true),
    p("L Calf", "L Foot", 30, 38, true, true),
    p("L Foot", "L Toe0", 38, 45, true, false),
    p("R Thigh", "R Calf", 23, 31, true, true),
    p("R Calf", "R Foot", 31, 40, true, true),
    p("R Foot", "R Toe0", 40, 46, true, false),
];

pub const HUMAN_THIEF_RIG: Rig<'static> = Rig {
    pairs: HUMAN_THIEF_SPACED,
    hips: Hips {
        wow_hip_root: 10,
        wow_hip_joints: [22, 23],
        ds_hip_joints: ["L Thigh", "R Thigh"],
        body_lift: None,
    },
    legs: Legs {
        ds: &[
            ["L Thigh", "L Calf", "L Foot"],
            ["R Thigh", "R Calf", "R Foot"],
        ],
        wow: &[[22, 30, 38], [23, 31, 40]],
    },
    checks: Checks {
        cross: &[("L Thigh", "R Thigh", 22, 23), ("Pelvis", "Spine", 12, 11)],
        wow_soles: &[38, 40, 45, 46],
        relative_soles: false,
        ground_lock: false,
        max_foot_gap: DEFAULT_FOOT_GAP,
    },
    names: &THIEF,
    contact_height: crate::retarget::CONTACT_HEIGHT,
};

/// The same thief onto `HumanFemale.m2` (117 bones, measured 2026-10-15). Its common body
/// root is 20, above both spine 21 and pelvis 22; driving the pelvis alone stretches a death.
/// Left remains WoW +Y (Bevy -X). The right hand is 96, not the male target's 97.
pub const HUMAN_FEMALE_SPACED: &[Pair] = &[
    p("Pelvis", "L Thigh", 22, 24, false, false),
    p("Spine", "Spine1", 21, 23, true, false),
    p("Spine1", "Neck", 23, 44, true, false),
    p("Neck", "Head", 44, 63, true, false),
    p("Head", "Neck", 63, 44, false, false),
    p("L Clavicle", "L UpperArm", 50, 64, true, true),
    p("L UpperArm", "L Forearm", 64, 77, true, true),
    p("L Forearm", "L Hand", 77, 94, true, true),
    p("L Hand", "L Finger1", 94, 99, true, true),
    p("R Clavicle", "R UpperArm", 51, 65, true, true),
    p("R UpperArm", "R Forearm", 65, 78, true, true),
    p("R Forearm", "R Hand", 78, 96, true, true),
    p("R Hand", "R Finger1", 96, 101, true, true),
    p("L Thigh", "L Calf", 24, 45, true, true),
    p("L Calf", "L Foot", 45, 66, true, true),
    p("L Foot", "L Toe0", 66, 79, true, true),
    p("R Thigh", "R Calf", 25, 46, true, true),
    p("R Calf", "R Foot", 46, 68, true, true),
    p("R Foot", "R Toe0", 68, 80, true, true),
];

/// The same measured female target for the player's underscore-named skeleton (`c0000`).
pub const HUMAN_FEMALE: &[Pair] = &[
    p("Pelvis", "L_Thigh", 22, 24, false, false),
    p("Spine", "Spine1", 21, 23, true, false),
    p("Spine1", "Neck", 23, 44, true, false),
    p("Neck", "Head", 44, 63, true, false),
    p("Head", "Neck", 63, 44, false, false),
    p("L_Clavicle", "L_UpperArm", 50, 64, true, true),
    p("L_UpperArm", "L_Forearm", 64, 77, true, true),
    p("L_Forearm", "L_Hand", 77, 94, true, true),
    p("L_Hand", "L_Finger1", 94, 99, true, true),
    p("R_Clavicle", "R_UpperArm", 51, 65, true, true),
    p("R_UpperArm", "R_Forearm", 65, 78, true, true),
    p("R_Forearm", "R_Hand", 78, 96, true, true),
    p("R_Hand", "R_Finger1", 96, 101, true, true),
    p("L_Thigh", "L_Calf", 24, 45, true, true),
    p("L_Calf", "L_Foot", 45, 66, true, true),
    p("L_Foot", "L_Toe0", 66, 79, true, true),
    p("R_Thigh", "R_Calf", 25, 46, true, true),
    p("R_Calf", "R_Foot", 46, 68, true, true),
    p("R_Foot", "R_Toe0", 68, 80, true, true),
];

pub const HUMAN_FEMALE_RIG: Rig<'static> = Rig {
    pairs: HUMAN_FEMALE,
    hips: Hips {
        wow_hip_root: 20,
        wow_hip_joints: [24, 25],
        ds_hip_joints: ["L_Thigh", "R_Thigh"],
        body_lift: None,
    },
    legs: Legs {
        ds: &[
            ["L_Thigh", "L_Calf", "L_Foot"],
            ["R_Thigh", "R_Calf", "R_Foot"],
        ],
        wow: &[[24, 45, 66], [25, 46, 68]],
    },
    checks: Checks {
        cross: &[("L_Thigh", "R_Thigh", 24, 25), ("Pelvis", "Spine", 22, 21)],
        wow_soles: &[66, 68, 79, 80],
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: DEFAULT_FOOT_GAP,
    },
    names: &C0000,
    contact_height: crate::retarget::CONTACT_HEIGHT,
};

pub const THIEF_FEMALE_RIG: Rig<'static> = Rig {
    pairs: HUMAN_FEMALE_SPACED,
    hips: Hips {
        wow_hip_root: 20,
        wow_hip_joints: [24, 25],
        ds_hip_joints: ["L Thigh", "R Thigh"],
        body_lift: None,
    },
    legs: Legs {
        ds: &[
            ["L Thigh", "L Calf", "L Foot"],
            ["R Thigh", "R Calf", "R Foot"],
        ],
        wow: &[[24, 45, 66], [25, 46, 68]],
    },
    checks: Checks {
        cross: &[("L Thigh", "R Thigh", 24, 25), ("Pelvis", "Spine", 22, 21)],
        wow_soles: &[66, 68, 79, 80],
        // Female soles sit 0.0269 yd above the model origin at bind. Preserve that measured
        // offset while physically locking the posed soles to the source contact, as for Wolf.
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: DEFAULT_FOOT_GAP,
    },
    names: &THIEF,
    contact_height: crate::retarget::CONTACT_HEIGHT,
};

/// The undead soldier's (`c2550`) skeleton, the thief's naming (spaces: `L Thigh`; `R_weapon` in the
/// right hand, a shield in the left). A humanoid with clavicles, so its feet and toes are its soles.
pub const SOLDIER: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "L UpperArm",
    right: "R UpperArm",
    facing: Facing::FeetToToes([("L Foot", "L Toe0"), ("R Foot", "R Toe0")]),
    soles: &["L Foot", "R Foot", "L Toe0", "R Toe0"],
    hand: "R Hand",
    weapon: "R_weapon",
    head: "Head",
    forearm: "R Forearm",
};

/// The soldier onto the same 60-bone gnoll body Hogger wears (`GnollMelee.m2`; `Gnoll\gnoll.m2` has the
/// identical bone table): the gnoll's waist 12 follows `Pelvis`, its spine base 11 `Spine`, its chest 13
/// `Spine1` (the soldier's chest, which its neck and clavicles hang from), its neck 15 `Neck`, its head
/// 30 `Head`. The gnoll has no clavicles: its arm roots 16 / 17 hang from the chest and take the upper
/// arms. The soldier has no tail; the gnoll's tail (18, 33, 42) keeps the model's own motion.
pub const GNOLL_SOLDIER: &[Pair] = &[
    p("Pelvis", "L Thigh", 12, 19, false, false),
    p("Spine", "Spine1", 11, 13, true, false),
    p("Spine1", "Neck", 13, 15, true, false),
    p("Neck", "Head", 15, 30, true, false),
    p("Head", "Neck", 30, 15, false, false),
    p("L UpperArm", "L Forearm", 16, 31, true, true),
    p("L Forearm", "L Hand", 31, 40, true, true),
    p("L Hand", "L Finger1", 40, 48, true, true),
    p("R UpperArm", "R Forearm", 17, 32, true, true),
    p("R Forearm", "R Hand", 32, 41, true, true),
    p("R Hand", "R Finger1", 41, 50, true, true),
    p("L Thigh", "L Calf", 19, 34, true, true),
    p("L Calf", "L Foot", 34, 43, true, true),
    p("L Foot", "L Toe0", 43, 52, true, false),
    p("R Thigh", "R Calf", 20, 35, true, true),
    p("R Calf", "R Foot", 35, 44, true, true),
    p("R Foot", "R Toe0", 44, 53, true, false),
];

pub const GNOLL_SOLDIER_RIG: Rig<'static> = Rig {
    pairs: GNOLL_SOLDIER,
    hips: Hips {
        wow_hip_root: 14,
        wow_hip_joints: [19, 20],
        ds_hip_joints: ["L Thigh", "R Thigh"],
        // The torso (11, 13) hangs from the model root (10), not from the hip root (14): a body that
        // drops with its hips (a collapse) takes the torso with it, as Hogger's does.
        body_lift: Some(BodyLift {
            root: 10,
            full_to: 0.6,
            beyond: 0.2,
        }),
    },
    legs: Legs {
        ds: &[
            ["L Thigh", "L Calf", "L Foot"],
            ["R Thigh", "R Calf", "R Foot"],
        ],
        wow: &[[19, 34, 52], [20, 35, 53]],
    },
    checks: Checks {
        // The hip axis only: the gnoll's arms are rigid on its chest, so the soldier's shoulder line
        // (clavicles that roll and shrug) is not compared.
        cross: &[("L Thigh", "R Thigh", 19, 20)],
        wow_soles: &[52, 53],
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: 0.25,
    },
    names: &SOLDIER,
    contact_height: crate::retarget::CONTACT_HEIGHT,
};

/// Knight Artorias (`c4100`) onto the gnoll: the soldier's pairs and names (the knight's skeleton carries the same
/// biped names and `R_weapon`). Only the foot gate differs: the ground lock moves the knight's body further than
/// the soldier's (his stance is wider and his flips leave the ground), measured per clip in the extraction log.
pub const GNOLL_KNIGHT_RIG: Rig<'static> = Rig {
    checks: Checks {
        max_foot_gap: 0.45,
        ..GNOLL_SOLDIER_RIG.checks
    },
    ..GNOLL_SOLDIER_RIG
};

/// The undead soldier onto the murloc (`Creature\Murloc\Murloc.m2`, 62 bones, measured 2026-10-16;
/// WoW +Y is left, so a left bone has the positive Y pivot): 13 root (key 26) → 14 the hip root, which
/// the chest 15 and the hips 16 / 17 hang from; 15 → 25 head (key 6, no neck bone) → 42 jaw (key 7);
/// the arms 23 / 24 (shoulders) → 32 / 33 (elbows) → 48 / 49 (wrists, with their fingers); the legs
/// 16 → 26 → 43 → 50 (hip, knee, ankle, toe) and 17 → 27 → 44 → 51. The soldier's `Pelvis` turns the hip
/// root, its chest `Spine1` the chest, its `Head` the head; its `Spine` and `Neck` have no murloc bone.
pub const MURLOC: &[Pair] = &[
    p("Pelvis", "L Thigh", 14, 16, false, false),
    p("Spine1", "Neck", 15, 25, true, false),
    p("Head", "Neck", 25, 15, false, false),
    p("L UpperArm", "L Forearm", 23, 32, true, true),
    p("L Forearm", "L Hand", 32, 48, true, true),
    p("L Hand", "L Finger1", 48, 53, true, true),
    p("R UpperArm", "R Forearm", 24, 33, true, true),
    p("R Forearm", "R Hand", 33, 49, true, true),
    p("R Hand", "R Finger1", 49, 55, true, true),
    p("L Thigh", "L Calf", 16, 26, true, true),
    p("L Calf", "L Foot", 26, 43, true, true),
    p("L Foot", "L Toe0", 43, 50, true, false),
    p("R Thigh", "R Calf", 17, 27, true, true),
    p("R Calf", "R Foot", 27, 44, true, true),
    p("R Foot", "R Toe0", 44, 51, true, false),
];

pub const MURLOC_RIG: Rig<'static> = Rig {
    pairs: MURLOC,
    hips: Hips {
        wow_hip_root: 14,
        wow_hip_joints: [16, 17],
        ds_hip_joints: ["L Thigh", "R Thigh"],
        // The model root (13) sits at hip height and is driven as the hip root's parent: it drops with a
        // collapse, or a lying murloc would measure as tall as its root stands.
        body_lift: Some(BodyLift {
            root: 13,
            full_to: 0.6,
            beyond: 0.2,
        }),
    },
    legs: Legs {
        ds: &[
            ["L Thigh", "L Calf", "L Foot"],
            ["R Thigh", "R Calf", "R Foot"],
        ],
        wow: &[[16, 26, 43], [17, 27, 44]],
    },
    checks: Checks {
        // The hip axis only: the murloc's arms hang from one chest bone with no collar bones.
        cross: &[("L Thigh", "R Thigh", 16, 17)],
        wow_soles: &[43, 44, 50, 51],
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: 0.25,
    },
    names: &SOLDIER,
    contact_height: crate::retarget::CONTACT_HEIGHT,
};

/// The black dog onto `Creature\Boar\Boar.m2` (57 bones, measured 2026-10-16; WoW +Y is left): 14 root → 15
/// body root (key 26) → 16 body (the front legs 18 / 19 → knees 28 / 29 → ankles 41 / 42 → hooves 50 / 51
/// hang from it) → 20 waist (key 5; the tail 30 → 43 → 47 → 52 and the hind-leg group 35 hang from it;
/// hind hips 44 / 45 → knees 48 / 49 → hocks 53 / 54 → hooves 55 / 56); 15 → 17 → 21 head (key 6, the
/// neck is part of it) → 33 jaw (key 7). The rear turns with the DS pelvis, the front with the ribcage.
pub const BOAR: &[Pair] = &[
    p("Pelvis", "Spine1", 20, 16, false, false),
    p("Ribcage", "Neck1", 16, 21, false, false),
    p("Head", "Jaw", 21, 33, false, false),
    p("Jaw", "Head", 33, 21, false, false),
    // Front legs: left (+Y) then right.
    p("LLeg_A_1", "LLeg_A_2", 18, 28, true, true),
    p("LLeg_A_2", "LLeg_A_Palm", 28, 41, true, true),
    p("LLeg_A_Palm", "LLeg_A_Digit21", 41, 50, true, true),
    p("RLeg_A_1", "RLeg_A_2", 19, 29, true, true),
    p("RLeg_A_2", "RLeg_A_Palm", 29, 42, true, true),
    p("RLeg_A_Palm", "RLeg_A_Digit21", 42, 51, true, true),
    // Hind legs.
    p("LLeg_B_1", "LLeg_B_2", 44, 48, true, true),
    p("LLeg_B_2", "LLeg_B_Palm", 48, 53, true, true),
    p("LLeg_B_Palm", "LLeg_B_Digit21", 53, 55, true, true),
    p("RLeg_B_1", "RLeg_B_2", 45, 49, true, true),
    p("RLeg_B_2", "RLeg_B_Palm", 49, 54, true, true),
    p("RLeg_B_Palm", "RLeg_B_Digit21", 54, 56, true, true),
    // The tail: a chain of four.
    p("Tail1", "Tail2", 30, 43, false, false),
    p("Tail3", "Tail4", 43, 47, false, false),
    p("Tail5", "Tail6", 47, 52, false, false),
];

pub const BOAR_RIG: Rig<'static> = Rig {
    pairs: BOAR,
    hips: Hips {
        wow_hip_root: 16,
        wow_hip_joints: [44, 45],
        ds_hip_joints: ["LLeg_B_1", "RLeg_B_1"],
        // The body root (15) stands at the body's height and is driven as the hip root's parent: it falls
        // with a collapse, or a lying boar would measure as tall as its root.
        body_lift: Some(BodyLift {
            root: 15,
            full_to: 0.6,
            beyond: 0.2,
        }),
    },
    // The scale follows one front and one hind leg: the front leg reaches the ground, the hind leg
    // ends 0.25 yd above it, so either alone is off.
    legs: Legs {
        ds: &[
            ["LLeg_A_1", "LLeg_A_2", "LLeg_A_Digit11"],
            ["LLeg_B_1", "LLeg_B_2", "LLeg_B_Digit11"],
        ],
        wow: &[[18, 28, 50], [44, 48, 55]],
    },
    checks: Checks {
        cross: &[("LLeg_B_1", "RLeg_B_1", 44, 45)],
        wow_soles: &[41, 42, 55, 56],
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: 0.7,
    },
    names: &BLACK_DOG,
    contact_height: 0.10,
};

/// The Armored Tusk's (`c3460`, NpcParam 346000) skeleton: Pelvis (the rump) → Spine1-4 → Neck1-2 → Head → Nose,
/// the front legs `LLeg_aCollarbone` → `a1` → `a2` → `aAnkle` from Spine4, the hind legs `LLeg_b1` → `b2` → `b3` →
/// `bAnkle` from the pelvis, `tail1-5`, and a mantle of cloth bones on the neck and head (not mapped). No jaw: the
/// head swings the tusks. Forward is -Z; the ankles stand 0.20 m off the ground at bind.
pub const ARMORED_TUSK: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "LLeg_a1",
    right: "RLeg_a1",
    facing: Facing::Along("Pelvis", "Head"),
    soles: &["LLeg_aAnkle", "RLeg_aAnkle", "LLeg_bAnkle", "RLeg_bAnkle"],
    hand: "Nose",
    weapon: "Nose",
    head: "Head",
    forearm: "Head",
};

/// The Armored Tusk onto `Creature\Boar\Boar.m2` (the bones as in [`BOAR`]): the rear turns with the Tusk's
/// pelvis, the front body with its last spine bone, the head (the WoW head holds the neck) with its head and
/// the jaw with the nose, so the tusks gore where the Tusk's do. Front legs a1 → a2 → ankle onto shoulder →
/// knee → ankle (the hoof follows); hind legs b1 → b2 → b3 → ankle onto hip → knee → hock → hoof.
pub const TUSK_BOAR: &[Pair] = &[
    p("Pelvis", "Spine1", 20, 16, false, false),
    p("Spine4", "Neck1", 16, 21, false, false),
    p("Head", "Nose", 21, 33, false, false),
    p("Nose", "Head", 33, 21, false, false),
    // Front legs: left (+Y) then right.
    p("LLeg_a1", "LLeg_a2", 18, 28, true, true),
    p("LLeg_a2", "LLeg_aAnkle", 28, 41, true, true),
    p("RLeg_a1", "RLeg_a2", 19, 29, true, true),
    p("RLeg_a2", "RLeg_aAnkle", 29, 42, true, true),
    // Hind legs.
    p("LLeg_b1", "LLeg_b2", 44, 48, true, true),
    p("LLeg_b2", "LLeg_b3", 48, 53, true, true),
    p("LLeg_b3", "LLeg_bAnkle", 53, 55, true, true),
    p("RLeg_b1", "RLeg_b2", 45, 49, true, true),
    p("RLeg_b2", "RLeg_b3", 49, 54, true, true),
    p("RLeg_b3", "RLeg_bAnkle", 54, 56, true, true),
    // The tail.
    p("tail1", "tail2", 30, 43, false, false),
    p("tail2", "tail3", 43, 47, false, false),
    p("tail3", "tail4", 47, 52, false, false),
];

pub const TUSK_BOAR_RIG: Rig<'static> = Rig {
    pairs: TUSK_BOAR,
    hips: Hips {
        wow_hip_root: 16,
        wow_hip_joints: [44, 45],
        ds_hip_joints: ["LLeg_b1", "RLeg_b1"],
        body_lift: Some(BodyLift {
            root: 15,
            full_to: 0.6,
            beyond: 0.2,
        }),
    },
    // One front and one hind leg, as for the dog: shoulder to the hoof, hip to the hoof.
    legs: Legs {
        ds: &[
            ["LLeg_a1", "LLeg_a2", "LLeg_aAnkle"],
            ["LLeg_b1", "LLeg_b2", "LLeg_bAnkle"],
        ],
        wow: &[[18, 28, 50], [44, 48, 55]],
    },
    checks: Checks {
        cross: &[("LLeg_b1", "RLeg_b1", 44, 45)],
        // The front ankles, not the hooves: the Tusk's front leg ends at its ankle (measured: the foot gap
        // of the gore falls from 0.54 to 0.40 yd).
        wow_soles: &[41, 42, 55, 56],
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: 0.7,
    },
    names: &ARMORED_TUSK,
    // The Tusk's ankles stand 0.20 m up at bind.
    contact_height: 0.35,
};

/// The black dog onto `Creature\bear\Bear.m2` (54 bones, measured 2026-10-16; WoW +Y is left): 13 root (key
/// 26) → 14 the rear body (the hind hips 16 / 17 → knees 22 / 23 → hocks 34 / 35 → feet 41 / 42 and the tail
/// 18 → 24 hang from it) and 15 spine (key 4) → 20 chest (the shoulders 26 / 27 → elbows 36 / 37 → wrists
/// 43 / 44 → toes 50 / 51, and the neck 28 → head 38, key 6 → jaw 45, key 7). The rear turns with the DS
/// pelvis, the front with the ribcage.
pub const BEAR: &[Pair] = &[
    p("Pelvis", "Spine1", 14, 15, false, false),
    p("Spine1", "Spine2", 15, 20, false, false),
    p("Ribcage", "Neck1", 20, 28, false, false),
    p("Neck1", "Neck2", 28, 38, false, false),
    p("Head", "Jaw", 38, 45, false, false),
    p("Jaw", "Head", 45, 38, false, false),
    // Front legs: left (+Y) then right.
    p("LLeg_A_1", "LLeg_A_2", 26, 36, true, true),
    p("LLeg_A_2", "LLeg_A_Palm", 36, 43, true, true),
    p("LLeg_A_Palm", "LLeg_A_Digit21", 43, 50, true, true),
    p("RLeg_A_1", "RLeg_A_2", 27, 37, true, true),
    p("RLeg_A_2", "RLeg_A_Palm", 37, 44, true, true),
    p("RLeg_A_Palm", "RLeg_A_Digit21", 44, 51, true, true),
    // Hind legs.
    p("LLeg_B_1", "LLeg_B_2", 16, 22, true, true),
    p("LLeg_B_2", "LLeg_B_Palm", 22, 34, true, true),
    p("LLeg_B_Palm", "LLeg_B_Digit21", 34, 41, true, true),
    p("RLeg_B_1", "RLeg_B_2", 17, 23, true, true),
    p("RLeg_B_2", "RLeg_B_Palm", 23, 35, true, true),
    p("RLeg_B_Palm", "RLeg_B_Digit21", 35, 42, true, true),
    p("Tail1", "Tail2", 18, 24, false, false),
];

pub const BEAR_RIG: Rig<'static> = Rig {
    pairs: BEAR,
    hips: Hips {
        wow_hip_root: 13,
        wow_hip_joints: [16, 17],
        ds_hip_joints: ["LLeg_B_1", "RLeg_B_1"],
        // The root stands at the body's height: it falls with a collapse.
        body_lift: Some(BodyLift {
            root: 13,
            full_to: 0.6,
            beyond: 0.2,
        }),
    },
    // The scale follows one front and one hind leg, as the boar's.
    legs: Legs {
        ds: &[
            ["LLeg_A_1", "LLeg_A_2", "LLeg_A_Digit11"],
            ["LLeg_B_1", "LLeg_B_2", "LLeg_B_Digit11"],
        ],
        wow: &[[26, 36, 43], [16, 22, 34]],
    },
    checks: Checks {
        cross: &[("LLeg_B_1", "RLeg_B_1", 16, 17)],
        wow_soles: &[50, 51, 41, 42],
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: 0.7,
    },
    names: &BLACK_DOG,
    contact_height: 0.10,
};

/// The Great Feline's (`c5360`, NpcParam 536000) skeleton: Pelvis → Spine1-4 → Neck → Head → Jaw; front legs
/// `LLeg_a_Collarbone` → `a_1` → `a_2` → `a_3` → `a_Ankle` → four digits from Spine4, hind legs `LLeg_b_1` → `b_2` →
/// `b_3` (the hock, behind) → `b_4` → `b_Ankle` → four digits from the pelvis, `Tail1-10`, cloth and face bones (not
/// mapped). Digitigrade: it stands on its digits (0.05 m). Forward is -Z. Its blows are bites: the volume runs from
/// the jaw along the head (measured: the forepaws stay planted or come down with the body).
pub const GREAT_FELINE: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "LLeg_a_1",
    right: "RLeg_a_1",
    facing: Facing::Along("Pelvis", "Head"),
    soles: &[
        "LLeg_a_Digit21",
        "RLeg_a_Digit21",
        "LLeg_b_Digit21",
        "RLeg_b_Digit21",
    ],
    hand: "Jaw",
    weapon: "Jaw",
    head: "Head",
    forearm: "Head",
};

/// The Great Feline onto `Creature\bear\Bear.m2` (the bones as in [`BEAR`], pivots measured 2026-10-16): front
/// shoulder 26 (1.38) → elbow 36 (0.70) → wrist 43 (0.13) → toes 50; hind hip 16 (1.19) → knee 22 (0.55) → heel 34
/// (0.20, behind) → toes 41. The cat's paw (ankle → digits) turns the bear's (wrist → toes, heel → toes); its short
/// a_3 → ankle segment and b_4 are left to follow. The rear turns with the pelvis, the spine with Spine2, the chest
/// with Spine4, neck, head and jaw with theirs.
pub const FELINE_BEAR: &[Pair] = &[
    p("Pelvis", "Spine1", 14, 15, false, false),
    p("Spine2", "Spine3", 15, 20, false, false),
    p("Spine4", "Neck", 20, 28, false, false),
    p("Neck", "Head", 28, 38, false, false),
    p("Head", "Jaw", 38, 45, false, false),
    p("Jaw", "Head", 45, 38, false, false),
    // Front legs: left (+Y) then right.
    p("LLeg_a_1", "LLeg_a_2", 26, 36, true, true),
    p("LLeg_a_2", "LLeg_a_3", 36, 43, true, true),
    p("LLeg_a_Ankle", "LLeg_a_Digit21", 43, 50, true, false),
    p("RLeg_a_1", "RLeg_a_2", 27, 37, true, true),
    p("RLeg_a_2", "RLeg_a_3", 37, 44, true, true),
    p("RLeg_a_Ankle", "RLeg_a_Digit21", 44, 51, true, false),
    // Hind legs.
    p("LLeg_b_1", "LLeg_b_2", 16, 22, true, true),
    p("LLeg_b_2", "LLeg_b_3", 22, 34, true, true),
    p("LLeg_b_Ankle", "LLeg_b_Digit21", 34, 41, true, false),
    p("RLeg_b_1", "RLeg_b_2", 17, 23, true, true),
    p("RLeg_b_2", "RLeg_b_3", 23, 35, true, true),
    p("RLeg_b_Ankle", "RLeg_b_Digit21", 35, 42, true, false),
    p("Tail1", "Tail2", 18, 24, false, false),
];

pub const FELINE_BEAR_RIG: Rig<'static> = Rig {
    pairs: FELINE_BEAR,
    hips: Hips {
        wow_hip_root: 13,
        wow_hip_joints: [16, 17],
        ds_hip_joints: ["LLeg_b_1", "RLeg_b_1"],
        body_lift: Some(BodyLift {
            root: 13,
            full_to: 0.6,
            beyond: 0.2,
        }),
    },
    legs: Legs {
        ds: &[
            ["LLeg_a_1", "LLeg_a_2", "LLeg_a_Ankle"],
            ["LLeg_b_1", "LLeg_b_2", "LLeg_b_Ankle"],
        ],
        wow: &[[26, 36, 43], [16, 22, 34]],
    },
    checks: Checks {
        cross: &[("LLeg_b_1", "RLeg_b_1", 16, 17)],
        wow_soles: &[50, 51, 41, 42],
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: 0.7,
    },
    names: &GREAT_FELINE,
    contact_height: 0.15,
};

/// The drake's (`c3471`, NpcParam 347100) skeleton, the parts a bear can wear: Pelvis → Spine1-4 → Neck1-2 → Head →
/// Jaw; front legs `LLeg_a_Collarbone` → `a_1` → `a_2` → `a_3` → `a_Ankle` → four digits from Spine4, hind legs
/// `LLeg_b_1` → `b_2` → `b_3` → `b_4` → `b_Ankle` from the pelvis, `tail1-12`; the wings are not mapped. Forward
/// is -Z.
pub const DRAKE: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "LLeg_a_1",
    right: "RLeg_a_1",
    facing: Facing::Along("Pelvis", "Head"),
    soles: &[
        "LLeg_a_Digit21",
        "RLeg_a_Digit21",
        "LLeg_b_Ankle",
        "RLeg_b_Ankle",
    ],
    hand: "Jaw",
    weapon: "Jaw",
    head: "Head",
    forearm: "Head",
};

/// The drake onto `Creature\bear\Bear.m2`, for the claw swipe the bear borrows (see [`FELINE_BEAR`] for the bear's
/// bones): the same joints as the Great Feline's, the drake's neck folded into the bear's.
pub const DRAKE_BEAR: &[Pair] = &[
    pr("Pelvis", "Spine1", 14, 15, false),
    pr("Spine2", "Spine3", 15, 20, false),
    pr("Spine4", "Neck1", 20, 28, false),
    pr("Neck2", "Head", 28, 38, false),
    pr("Head", "Jaw", 38, 45, false),
    pr("Jaw", "Head", 45, 38, false),
    pr("LLeg_a_1", "LLeg_a_2", 26, 36, true),
    pr("LLeg_a_2", "LLeg_a_3", 36, 43, true),
    pr("LLeg_a_Ankle", "LLeg_a_Digit21", 43, 50, false),
    pr("RLeg_a_1", "RLeg_a_2", 27, 37, true),
    pr("RLeg_a_2", "RLeg_a_3", 37, 44, true),
    pr("RLeg_a_Ankle", "RLeg_a_Digit21", 44, 51, false),
    pr("LLeg_b_1", "LLeg_b_2", 16, 22, true),
    pr("LLeg_b_2", "LLeg_b_3", 22, 34, true),
    pr("LLeg_b_4", "LLeg_b_Ankle", 34, 41, false),
    pr("RLeg_b_1", "RLeg_b_2", 17, 23, true),
    pr("RLeg_b_2", "RLeg_b_3", 23, 35, true),
    pr("RLeg_b_4", "RLeg_b_Ankle", 35, 42, false),
    pr("tail1", "tail2", 18, 24, false),
];

pub const DRAKE_BEAR_RIG: Rig<'static> = Rig {
    pairs: DRAKE_BEAR,
    names: &DRAKE,
    contact_height: 0.3,
    ..FELINE_BEAR_RIG
};

/// The undead thief onto `Character\Goblin\Male\GoblinMale.m2` (measured 2026-10-16; WoW +Y is left): 15 root
/// (key 26) → 16 spine low (key 4) → 18 chest → 34 neck → 50 head (key 6) → 64 jaw (key 7); 18 → 35 / 36
/// collar bones → 51 / 52 upper arms → 67 / 68 forearms → 81 / 83 hands (fingers 85 / 89); 15 → 17 waist
/// (key 5) → 21 / 22 hips → 39 / 40 knees → 54 / 56 ankles → 69 / 70 toes. The spine and the waist are
/// siblings, so the model root (15) is the hip root. The same pairs as the human male, with the goblin's indices.
pub const GOBLIN: &[Pair] = &[
    p("Pelvis", "L Thigh", 17, 21, false, false),
    p("Spine", "Spine1", 16, 18, true, false),
    p("Spine1", "Neck", 18, 34, true, false),
    p("Neck", "Head", 34, 50, true, false),
    p("Head", "Neck", 50, 34, false, false),
    p("L Clavicle", "L UpperArm", 35, 51, true, true),
    p("L UpperArm", "L Forearm", 51, 67, true, true),
    p("L Forearm", "L Hand", 67, 81, true, true),
    p("L Hand", "L Finger1", 81, 85, true, true),
    p("R Clavicle", "R UpperArm", 36, 52, true, true),
    p("R UpperArm", "R Forearm", 52, 68, true, true),
    p("R Forearm", "R Hand", 68, 83, true, true),
    p("R Hand", "R Finger1", 83, 89, true, true),
    p("L Thigh", "L Calf", 21, 39, true, true),
    p("L Calf", "L Foot", 39, 54, true, true),
    p("L Foot", "L Toe0", 54, 69, true, false),
    p("R Thigh", "R Calf", 22, 40, true, true),
    p("R Calf", "R Foot", 40, 56, true, true),
    p("R Foot", "R Toe0", 56, 70, true, false),
];

pub const GOBLIN_RIG: Rig<'static> = Rig {
    pairs: GOBLIN,
    hips: Hips {
        wow_hip_root: 15,
        wow_hip_joints: [21, 22],
        ds_hip_joints: ["L Thigh", "R Thigh"],
        body_lift: None,
    },
    legs: Legs {
        ds: &[
            ["L Thigh", "L Calf", "L Foot"],
            ["R Thigh", "R Calf", "R Foot"],
        ],
        wow: &[[21, 39, 54], [22, 40, 56]],
    },
    checks: Checks {
        // The hip axis only: the goblin's spine low (16) and waist (17) stand 4 cm apart, so no direction runs between them.
        cross: &[("L Thigh", "R Thigh", 21, 22)],
        wow_soles: &[54, 56, 69, 70],
        relative_soles: true,
        ground_lock: true,
        max_foot_gap: 0.25,
    },
    names: &THIEF,
    contact_height: crate::retarget::CONTACT_HEIGHT,
};

/// The Crag-Spider's (`c3400`, NpcParam 340000, 93 HP) skeleton: Pelvis (the abdomen's base) → Spine1-3 → Neck1-2
/// → Head, `Tail1-7` from the pelvis; four legs a side, front to back by their root's z: `LLeg_d` (five bones, from the
/// collar bone), `LLeg_e` (three), `LLeg_f` (five), `LLeg_h` (three); the big forearms `LArm_c*` and the small
/// `LArm_g`/`LArm_i` are not mapped. Its forward is -Z (read from the pelvis to the head).
pub const CRAG_SPIDER: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "LLeg_d1",
    right: "RLeg_d1",
    facing: Facing::Along("Pelvis", "Head"),
    soles: &[
        "LLeg_d5", "RLeg_d5", "LLeg_f5", "RLeg_f5", "LLeg_h3", "RLeg_h3", "LLeg_e3", "RLeg_e3",
    ],
    hand: "Head",
    weapon: "Head",
    head: "Head",
    forearm: "Tail1",
};

/// The Crag-Spider onto `Creature\MineSpider\MineSpider.m2` (91 bones, measured 2026-10-16): 22 root (key 26) →
/// 23 body (key-less thorax) → 35 head (key 6) → fangs 63-67; the abdomen 47 → 68 hangs from the body; the eight
/// legs hang from the root in five segments each. Left legs front to back (root, then each segment's child):
/// 28 → 37 → 53 → 70 → 80, 27 → 36 → 62 → 69 → 79, 29 → 38 → 54 → 71 → 81, 30 → 39 → 55 → 72 → 82; right front
/// to back 32 → 41 → 57 → 74 → 84, 31 → 40 → 56 → 73 → 83, 33 → 42 → 58 → 75 → 85, 34 → 43 → 59 → 76 → 86. The
/// DS legs d, e, f, h take them in that order; a three-bone DS leg (e, h) drives the first three segments and the
/// last two follow.
pub const MINESPIDER: &[Pair] = &[
    pr("Spine3", "Neck1", 23, 35, false),
    pr("Head", "Neck2", 35, 23, false),
    pr("Tail1", "Tail2", 47, 68, false),
    // Left d / e / f / h.
    pr("LLeg_d1", "LLeg_d2", 28, 37, true),
    pr("LLeg_d2", "LLeg_d3", 37, 53, true),
    pr("LLeg_d3", "LLeg_d4", 53, 70, true),
    pr("LLeg_d4", "LLeg_d5", 70, 80, true),
    pr("LLeg_e1", "LLeg_e2", 27, 36, true),
    pr("LLeg_e2", "LLeg_e3", 36, 62, true),
    pr("LLeg_f1", "LLeg_f2", 29, 38, true),
    pr("LLeg_f2", "LLeg_f3", 38, 54, true),
    pr("LLeg_f3", "LLeg_f4", 54, 71, true),
    pr("LLeg_f4", "LLeg_f5", 71, 81, true),
    pr("LLeg_h1", "LLeg_h2", 30, 39, true),
    pr("LLeg_h2", "LLeg_h3", 39, 55, true),
    // Right d / e / f / h.
    pr("RLeg_d1", "RLeg_d2", 32, 41, true),
    pr("RLeg_d2", "RLeg_d3", 41, 57, true),
    pr("RLeg_d3", "RLeg_d4", 57, 74, true),
    pr("RLeg_d4", "RLeg_d5", 74, 84, true),
    pr("RLeg_e1", "RLeg_e2", 31, 40, true),
    pr("RLeg_e2", "RLeg_e3", 40, 56, true),
    pr("RLeg_f1", "RLeg_f2", 33, 42, true),
    pr("RLeg_f2", "RLeg_f3", 42, 58, true),
    pr("RLeg_f3", "RLeg_f4", 58, 75, true),
    pr("RLeg_f4", "RLeg_f5", 75, 85, true),
    pr("RLeg_h1", "RLeg_h2", 34, 43, true),
    pr("RLeg_h2", "RLeg_h3", 43, 59, true),
];

pub const MINESPIDER_RIG: Rig<'static> = Rig {
    pairs: MINESPIDER,
    hips: Hips {
        wow_hip_root: 22,
        wow_hip_joints: [27, 31],
        ds_hip_joints: ["LLeg_e1", "RLeg_e1"],
        body_lift: None,
    },
    legs: Legs {
        ds: &[
            ["LLeg_d1", "LLeg_d3", "LLeg_d5"],
            ["RLeg_d1", "RLeg_d3", "RLeg_d5"],
        ],
        wow: &[[28, 53, 80], [32, 57, 84]],
    },
    checks: Checks {
        // No cross check: the DS d legs hang from the collar bones, which turn on their own.
        cross: &[],
        wow_soles: &[],
        relative_soles: false,
        ground_lock: false,
        max_foot_gap: 2.0,
    },
    names: &CRAG_SPIDER,
    contact_height: 0.8,
};

/// The Crag-Spider onto `Creature\Tarantula\Tarantula.m2` (74 bones; every top-level bone is its own root, so the
/// body parts keep their bind positions and only turn): the front body 2 (with the fangs 33-36), the abdomen 5 and
/// the legs: left front to back 6 → 50 → 58 → 66, 7 → 51 → 59 → 67, 8 → 52 → 60 → 68, 9 → 53 → 61 → 69; right
/// 10 → 54 → 62 → 70, 11 → 55 → 63 → 71, 12 → 56 → 64 → 72, 13 → 57 → 65 → 73. The DS legs d, e, f, h take them
/// in that order (four segments of the five-bone ones, three of the others); bone 15 is the childless hip root.
pub const TARANTULA: &[Pair] = &[
    pr("Spine3", "Neck1", 2, 34, false),
    pr("Head", "Neck2", 34, 2, false),
    pr("Tail1", "Tail2", 5, 41, false),
    // Left d / e / f / h.
    pr("LLeg_d1", "LLeg_d2", 6, 50, true),
    pr("LLeg_d2", "LLeg_d3", 50, 58, true),
    pr("LLeg_d3", "LLeg_d4", 58, 66, true),
    pr("LLeg_e1", "LLeg_e2", 7, 51, true),
    pr("LLeg_e2", "LLeg_e3", 51, 59, true),
    pr("LLeg_f1", "LLeg_f2", 8, 52, true),
    pr("LLeg_f2", "LLeg_f3", 52, 60, true),
    pr("LLeg_f3", "LLeg_f4", 60, 68, true),
    pr("LLeg_h1", "LLeg_h2", 9, 53, true),
    pr("LLeg_h2", "LLeg_h3", 53, 61, true),
    // Right d / e / f / h.
    pr("RLeg_d1", "RLeg_d2", 10, 54, true),
    pr("RLeg_d2", "RLeg_d3", 54, 62, true),
    pr("RLeg_d3", "RLeg_d4", 62, 70, true),
    pr("RLeg_e1", "RLeg_e2", 11, 55, true),
    pr("RLeg_e2", "RLeg_e3", 55, 63, true),
    pr("RLeg_f1", "RLeg_f2", 12, 56, true),
    pr("RLeg_f2", "RLeg_f3", 56, 64, true),
    pr("RLeg_f3", "RLeg_f4", 64, 72, true),
    pr("RLeg_h1", "RLeg_h2", 13, 57, true),
    pr("RLeg_h2", "RLeg_h3", 57, 65, true),
];

pub const TARANTULA_RIG: Rig<'static> = Rig {
    pairs: TARANTULA,
    hips: Hips {
        wow_hip_root: 15,
        wow_hip_joints: [7, 11],
        ds_hip_joints: ["LLeg_e1", "RLeg_e1"],
        body_lift: None,
    },
    legs: Legs {
        ds: &[
            ["LLeg_d1", "LLeg_d3", "LLeg_d5"],
            ["RLeg_d1", "RLeg_d3", "RLeg_d5"],
        ],
        wow: &[[6, 58, 66], [10, 62, 70]],
    },
    checks: Checks {
        cross: &[],
        wow_soles: &[],
        relative_soles: false,
        ground_lock: false,
        max_foot_gap: 2.0,
    },
    names: &CRAG_SPIDER,
    contact_height: 0.8,
};
