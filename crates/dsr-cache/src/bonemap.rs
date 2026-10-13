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
}

pub const HUMAN_MALE_HIPS: Hips = Hips {
    wow_hip_root: 18,
    wow_hip_joints: [25, 26],
    ds_hip_joints: ["L_Thigh", "R_Thigh"],
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
