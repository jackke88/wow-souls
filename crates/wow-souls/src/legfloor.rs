//! Keeps a many-legged creature's joints out of the ground (2026-10-17). The spiders' kits are the
//! Crag Spider's moves on WoW's flat tarantula and mine spider through a pose-relative retarget with no
//! ground lock (their feet differ too much from the source's for a foot gate): the bite drove the front
//! legs about a model yard under the turf. Here, frame by frame, a joint that sinks below its height in
//! the clip's first frame (or below the ground, whichever is lower) has the turns of the bones above it
//! eased back toward that first frame, just as far as it takes to bring it up again. The move keeps its
//! timing and shape; the legs stop where the ground is.

use bevy::prelude::*;

/// Model yards a joint may sink below its floor before it is lifted (rounding, not a gap).
const SLACK: f32 = 0.01;
/// Halvings of the blend search: 2^-12 of the way is far finer than a pixel.
const STEPS: usize = 12;

/// The kits whose clips get the floor: the spiders'.
pub fn wants_floor(kit: &str) -> bool {
    matches!(kit, "tarantula" | "minespider" | "mother_fang")
}

fn quat(r: [f32; 4]) -> Quat {
    Quat::from_array(r).normalize()
}

/// Every bone's model-space position in `frame` (bone-local clips).
fn positions(parents: &[i16], frame: &[wscl::Trs]) -> Vec<Vec3> {
    let n = frame.len();
    let mut at = vec![Vec3::ZERO; n];
    let mut turn = vec![Quat::IDENTITY; n];
    for b in 0..n {
        let local = Vec3::from_array(frame[b].t);
        let r = quat(frame[b].r);
        match parents
            .get(b)
            .copied()
            .filter(|&p| p >= 0 && (p as usize) < b)
        {
            Some(p) => {
                let p = p as usize;
                at[b] = at[p] + turn[p] * local;
                turn[b] = turn[p] * r;
            }
            None => {
                at[b] = local;
                turn[b] = r;
            }
        }
    }
    at
}

/// The bones above `bone`, nearest first.
fn ancestors(parents: &[i16], bone: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut b = bone;
    while let Some(p) = parents
        .get(b)
        .copied()
        .filter(|&p| p >= 0 && (p as usize) < b)
    {
        out.push(p as usize);
        b = p as usize;
    }
    out
}

fn depth(parents: &[i16], bone: usize) -> usize {
    ancestors(parents, bone).len()
}

/// Lifts every sunken joint of `clip` (see the module docs). Returns how many joint-frames it lifted.
/// Model-space clips and clips without a first frame are left alone.
pub fn keep_off_the_ground(clip: &mut wscl::Clip) -> usize {
    if clip.model_space || clip.frames.len() < 2 {
        return 0;
    }
    let parents = clip.parents.clone();
    let rest = clip.frames[0].clone();
    let n = rest.len();
    let floor: Vec<f32> = positions(&parents, &rest)
        .iter()
        .map(|p| p.y.min(0.0) - SLACK)
        .collect();
    // The tips first: lifting a leg's end mostly lifts its middle too.
    let mut order: Vec<usize> = (0..n).filter(|&b| depth(&parents, b) > 0).collect();
    order.sort_by_key(|&b| std::cmp::Reverse(depth(&parents, b)));
    let mut lifted = 0;
    for frame in clip.frames.iter_mut().skip(1) {
        if frame.len() != n {
            continue;
        }
        for &bone in &order {
            if positions(&parents, frame)[bone].y >= floor[bone] {
                continue;
            }
            let chain = ancestors(&parents, bone);
            let moved: Vec<Quat> = chain.iter().map(|&k| quat(frame[k].r)).collect();
            let at = |w: f32, frame: &mut [wscl::Trs]| {
                for (i, &k) in chain.iter().enumerate() {
                    frame[k].r = quat(rest[k].r).slerp(moved[i], w).to_array();
                }
            };
            // The largest share of the move that keeps the joint up (the first frame always does).
            let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
            for _ in 0..STEPS {
                let mid = (lo + hi) * 0.5;
                at(mid, frame);
                if positions(&parents, frame)[bone].y >= floor[bone] {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            at(lo, frame);
            lifted += 1;
        }
    }
    lifted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trs(t: [f32; 3], r: Quat) -> wscl::Trs {
        wscl::Trs {
            t,
            r: r.to_array(),
            s: [1.0; 3],
        }
    }

    /// A hip at the body's height and a one-yard leg hanging forward, level: the leg's end at the hip's height.
    fn leg(pitch: f32) -> Vec<wscl::Trs> {
        vec![
            trs([0.0, 0.5, 0.0], Quat::from_rotation_x(pitch)),
            trs([0.0, 0.0, 1.0], Quat::IDENTITY),
        ]
    }

    fn clip(frames: Vec<Vec<wscl::Trs>>) -> wscl::Clip {
        let n = frames.len();
        wscl::Clip {
            name: "test".into(),
            fps: 30.0,
            model_space: false,
            parents: vec![-1, 0],
            driven: vec![true, true],
            frames,
            root_motion: vec![[0.0; 4]; n],
            flags: Vec::new(),
            hits: Vec::new(),
        }
    }

    #[test]
    fn a_leg_driven_into_the_ground_stops_at_it_and_the_rest_is_untouched() {
        // Pitched 80 degrees down, the leg's end is 0.48 yd under the turf; 20 degrees down it is above.
        let mut c = clip(vec![leg(0.0), leg(0.35), leg(1.4)]);
        let before = c.frames[1].clone();
        assert_eq!(keep_off_the_ground(&mut c), 1);
        assert_eq!(
            c.frames[1], before,
            "a frame above the ground is not touched"
        );
        let end = positions(&c.parents, &c.frames[2])[1].y;
        assert!(
            (-SLACK..0.02).contains(&end),
            "the end rests on the ground: {end}"
        );
    }

    #[test]
    fn a_joint_that_starts_below_zero_may_go_back_there_but_no_lower() {
        let mut c = clip(vec![leg(0.6), leg(1.2)]);
        let rest = positions(&c.parents, &c.frames[0])[1].y;
        assert!(rest < 0.0);
        keep_off_the_ground(&mut c);
        let end = positions(&c.parents, &c.frames[1])[1].y;
        assert!(
            (end - rest).abs() < 0.02,
            "{end} against the first frame's {rest}"
        );
    }

    #[test]
    fn only_the_spiders_get_the_floor() {
        assert!(
            wants_floor("tarantula") && wants_floor("minespider") && wants_floor("mother_fang")
        );
        assert!(!wants_floor("wolf") && !wants_floor("boar"));
    }
}
