//! A creature's blow against the player: the hand's volume (a capsule from the hand along the
//! forearm, `radius` thick) swept from its last pose to this one against the player's `Hurtbox`
//! axis. Pure; `npc` feeds it the posed hand.

use bevy::prelude::*;

use crate::hits::segment_distance;

/// A volume: its two ends (hand, tip) and its radius (yd).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volume {
    pub ends: [Vec3; 2],
    pub radius: f32,
}

/// Whether a volume sweeping from `prev` to `now` touches the capsule with axis `cap` and radius
/// `cap_r`: the volume at both ends of the frame and the paths of its two ends and its middle.
pub fn touches(now: Volume, prev: Option<[Vec3; 2]>, cap: (Vec3, Vec3), cap_r: f32) -> bool {
    let reach = cap_r + now.radius;
    let mut segments = vec![(now.ends[0], now.ends[1])];
    if let Some(p) = prev {
        segments.push((p[0], p[1]));
        segments.push((p[0], now.ends[0]));
        segments.push((p[1], now.ends[1]));
        segments.push((p[0].lerp(p[1], 0.5), now.ends[0].lerp(now.ends[1], 0.5)));
    }
    segments
        .into_iter()
        .any(|(a, b)| segment_distance(a, b, cap.0, cap.1).0 <= reach)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> ((Vec3, Vec3), f32) {
        ((Vec3::new(0.0, 0.3, 0.0), Vec3::new(0.0, 1.5, 0.0)), 0.33)
    }

    fn volume(a: Vec3, b: Vec3) -> Volume {
        Volume {
            ends: [a, b],
            radius: 0.15,
        }
    }

    #[test]
    fn a_hand_through_the_body_touches_and_one_out_of_reach_does_not() {
        let (cap, r) = player();
        assert!(touches(
            volume(Vec3::new(0.3, 1.0, 0.0), Vec3::new(0.3, 1.0, 0.5)),
            None,
            cap,
            r
        ));
        assert!(!touches(
            volume(Vec3::new(1.5, 1.0, 0.0), Vec3::new(1.5, 1.0, 0.5)),
            None,
            cap,
            r
        ));
    }

    #[test]
    fn a_fast_swing_that_jumps_the_body_between_frames_still_touches() {
        let (cap, r) = player();
        let prev = [Vec3::new(-2.0, 1.0, 0.1), Vec3::new(-2.0, 1.0, 0.6)];
        let now = volume(Vec3::new(2.0, 1.0, 0.1), Vec3::new(2.0, 1.0, 0.6));
        assert!(!touches(now, None, cap, r));
        assert!(touches(now, Some(prev), cap, r));
    }

    #[test]
    fn a_thicker_volume_reaches_further() {
        let (cap, r) = player();
        let at = |radius| Volume {
            ends: [Vec3::new(0.7, 1.0, 0.0), Vec3::new(0.7, 1.0, 0.4)],
            radius,
        };
        assert!(!touches(at(0.1), None, cap, r));
        assert!(touches(at(0.5), None, cap, r));
    }
}
