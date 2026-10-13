//! DS model space → Bevy model space at WoW scale: a change of basis (possibly a reflection)
//! built from the animation's up/forward and the skeleton's left side, and a uniform scale.

use glam::{Mat3, Quat, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Basis {
    pub m: Mat3,
    pub scale: f32,
}

impl Basis {
    /// `up`, `forward` from the reference frame; `left` points from the right shoulder to the
    /// left one; all in DS space.
    pub fn new(up: Vec3, forward: Vec3, left: Vec3, scale: f32) -> Basis {
        let up = up.normalize();
        let forward = (forward - up * forward.dot(up)).normalize();
        let right = (-left - up * (-left).dot(up) - forward * (-left).dot(forward)).normalize();
        let ds = Mat3::from_cols(right, up, forward);
        let bevy = Mat3::from_cols(Vec3::X, Vec3::Y, Vec3::NEG_Z);
        Basis {
            m: bevy * ds.transpose(),
            scale,
        }
    }

    /// A DS position, in Bevy axes at WoW scale.
    pub fn pos(&self, v: Vec3) -> Vec3 {
        self.m * v * self.scale
    }

    /// A DS direction, in Bevy axes.
    pub fn dir(&self, v: Vec3) -> Vec3 {
        self.m * v
    }

    /// A DS rotation, as the same rotation seen in Bevy axes (proper even under reflection).
    pub fn rot(&self, q: Quat) -> Quat {
        Quat::from_mat3(&(self.m * Mat3::from_quat(q) * self.m.transpose())).normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bevy_axes_map_to_themselves() {
        let b = Basis::new(Vec3::Y, Vec3::NEG_Z, Vec3::NEG_X, 1.0);
        assert!((b.m - Mat3::IDENTITY).abs_diff_eq(Mat3::ZERO, 1.0e-6));
    }

    fn assert_consistent(b: &Basis) {
        let q = Quat::from_rotation_y(0.7) * Quat::from_rotation_x(0.3);
        let v = Vec3::new(0.2, 0.5, 0.9);
        let lhs = b.dir(q * v);
        let rhs = b.rot(q) * b.dir(v);
        assert!((lhs - rhs).length() < 1.0e-5, "{lhs} vs {rhs}");
    }

    #[test]
    fn a_space_turned_about_up_keeps_rotations_consistent() {
        // Forward +Z, left +X: Bevy's axes turned 180° about up (a proper rotation).
        let b = Basis::new(Vec3::Y, Vec3::Z, Vec3::X, 2.0);
        assert!((b.dir(Vec3::Z) - Vec3::NEG_Z).length() < 1.0e-6);
        assert!(
            (b.dir(Vec3::X) - Vec3::NEG_X).length() < 1.0e-6,
            "left stays left"
        );
        assert!((b.m.determinant() - 1.0).abs() < 1.0e-6);
        assert_consistent(&b);
        assert!((b.pos(Vec3::Y) - Vec3::Y * 2.0).length() < 1.0e-6);
    }

    #[test]
    fn a_mirrored_space_keeps_rotations_proper_and_consistent() {
        // DSR's c0000: forward -Z, left +X, a mirror of Bevy's handedness.
        let b = Basis::new(Vec3::Y, Vec3::NEG_Z, Vec3::X, 1.0);
        assert!((b.dir(Vec3::NEG_Z) - Vec3::NEG_Z).length() < 1.0e-6);
        assert!(
            (b.dir(Vec3::X) - Vec3::NEG_X).length() < 1.0e-6,
            "left stays left"
        );
        assert!((b.m.determinant() + 1.0).abs() < 1.0e-6);
        assert_consistent(&b);
    }
}
