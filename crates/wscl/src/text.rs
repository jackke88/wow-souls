//! `.wsanim`: a [`Clip`] as editable text. Every bone's turn is written as degrees (pitch about X, yaw about Y,
//! roll about Z, applied yaw, then pitch, then roll: `Ry * Rx * Rz`, glam's `EulerRot::YXZ`) and its place in
//! yards, one line per bone per frame, so a clip can be read, changed by hand and read back.
//!
//! ```text
//! wsanim 1
//! name a00_0535
//! fps 30
//! model_space 0
//! bones 119
//! parents -1 0 0 ...
//! driven 0 18 20 ...
//! frames 19
//! flag 8 0.000000 0.366667
//! hit 100 0.700000 0.866667
//! frame 0 root 0.000000 0.000000 -0.000000 0.000000
//! b 18 pitch yaw roll x y z
//! ```
//!
//! Version 2 preserves non-unit scale as three optional values after a bone's translation.
//! Version 1 files remain readable.
use crate::{Clip, FlagWindow, HitWindow, Trs};
use std::fmt::Write as _;

pub const TEXT_VERSION: u32 = 2;

/// A rotation `[x, y, z, w]` as `[pitch, yaw, roll]` in degrees (`Ry(yaw) * Rx(pitch) * Rz(roll)`).
pub fn euler_degrees(q: [f32; 4]) -> [f64; 3] {
    let [x, y, z, w] = q.map(f64::from);
    let n = (x * x + y * y + z * z + w * w).sqrt();
    let (x, y, z, w) = (x / n, y / n, z / n, w / n);
    // The rotation matrix's elements this order needs.
    let m12 = 2.0 * (y * z - w * x);
    let m02 = 2.0 * (x * z + w * y);
    let m22 = 1.0 - 2.0 * (x * x + y * y);
    let m10 = 2.0 * (x * y + w * z);
    let m11 = 1.0 - 2.0 * (x * x + z * z);
    let pitch = (-m12).clamp(-1.0, 1.0).asin();
    let (yaw, roll) = if m12.abs() < 1.0 - 1.0e-12 {
        (m02.atan2(m22), m10.atan2(m11))
    } else {
        // Straight up or down: yaw and roll turn about the same axis; give it all to yaw (with no roll the
        // matrix is `Ry * Rx`, whose first column is `(cos yaw, 0, -sin yaw)`).
        let m00 = 1.0 - 2.0 * (y * y + z * z);
        let m20 = 2.0 * (x * z - w * y);
        ((-m20).atan2(m00), 0.0)
    };
    [pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees()]
}

/// `[pitch, yaw, roll]` degrees back to `[x, y, z, w]`.
pub fn quat_of_degrees([pitch, yaw, roll]: [f64; 3]) -> [f32; 4] {
    let (hp, hy, hr) = (
        pitch.to_radians() / 2.0,
        yaw.to_radians() / 2.0,
        roll.to_radians() / 2.0,
    );
    // Ry * Rx * Rz.
    let (sy, cy) = hy.sin_cos();
    let (sp, cp) = hp.sin_cos();
    let (sr, cr) = hr.sin_cos();
    let x = cy * sp * cr + sy * cp * sr;
    let y = sy * cp * cr - cy * sp * sr;
    let z = cy * cp * sr - sy * sp * cr;
    let w = cy * cp * cr + sy * sp * sr;
    [x as f32, y as f32, z as f32, w as f32]
}

/// The clip as text.
pub fn write(clip: &Clip) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "wsanim {TEXT_VERSION}");
    let _ = writeln!(s, "name {}", clip.name);
    let _ = writeln!(s, "fps {}", clip.fps);
    let _ = writeln!(s, "model_space {}", u8::from(clip.model_space));
    let _ = writeln!(s, "bones {}", clip.parents.len());
    let join = |v: Vec<String>| v.join(" ");
    let _ = writeln!(
        s,
        "parents {}",
        join(clip.parents.iter().map(|p| p.to_string()).collect())
    );
    let _ = writeln!(
        s,
        "driven {}",
        join(
            clip.driven
                .iter()
                .enumerate()
                .filter(|(_, d)| **d)
                .map(|(i, _)| i.to_string())
                .collect()
        )
    );
    let _ = writeln!(s, "frames {}", clip.frames.len());
    for f in &clip.flags {
        let _ = writeln!(s, "flag {} {:.6} {:.6}", f.flag, f.start, f.end);
    }
    for h in &clip.hits {
        let _ = writeln!(s, "hit {} {:.6} {:.6}", h.judge, h.start, h.end);
    }
    for (i, frame) in clip.frames.iter().enumerate() {
        let r = clip.root_motion.get(i).copied().unwrap_or([0.0; 4]);
        let _ = writeln!(
            s,
            "frame {i} root {:.6} {:.6} {:.6} {:.6}",
            r[0], r[1], r[2], r[3]
        );
        for (b, trs) in frame.iter().enumerate() {
            let [p, y, rr] = euler_degrees(trs.r);
            let _ = write!(
                s,
                "b {b} {p:.5} {y:.5} {rr:.5} {:.6} {:.6} {:.6}",
                trs.t[0], trs.t[1], trs.t[2]
            );
            if trs.s != [1.0; 3] {
                let _ = write!(s, " {} {} {}", trs.s[0], trs.s[1], trs.s[2]);
            }
            s.push('\n');
        }
    }
    s
}

/// A clip read back from [`write`]'s text; `Err` names the line that is wrong.
pub fn read(text: &str) -> Result<Clip, String> {
    let mut lines = text.lines().enumerate().filter(|(_, l)| {
        let l = l.trim();
        !l.is_empty() && !l.starts_with('#')
    });
    let bad = |n: usize, l: &str| format!("line {}: {l:?}", n + 1);
    let mut next = || {
        lines
            .next()
            .ok_or_else(|| "the text ends early".to_string())
    };
    let (n, l) = next()?;
    if !matches!(l.trim(), "wsanim 1" | "wsanim 2") {
        return Err(bad(n, l));
    }
    let field = |n: usize, l: &str, key: &str| -> Result<String, String> {
        l.strip_prefix(key)
            .and_then(|r| r.strip_prefix(' '))
            .map(|r| r.trim().to_string())
            .ok_or_else(|| bad(n, l))
    };
    let (n, l) = next()?;
    let name = field(n, l, "name")?;
    let (n, l) = next()?;
    let fps: f32 = field(n, l, "fps")?.parse().map_err(|_| bad(n, l))?;
    let (n, l) = next()?;
    let model_space = field(n, l, "model_space")? == "1";
    let (n, l) = next()?;
    let bones: usize = field(n, l, "bones")?.parse().map_err(|_| bad(n, l))?;
    let (n, l) = next()?;
    let parents = field(n, l, "parents")?
        .split_whitespace()
        .map(|v| v.parse::<i16>().map_err(|_| bad(n, l)))
        .collect::<Result<Vec<_>, _>>()?;
    if parents.len() != bones {
        return Err(bad(n, l));
    }
    let (n, l) = next()?;
    let mut driven = vec![false; bones];
    for v in field(n, l, "driven")?.split_whitespace() {
        let i: usize = v.parse().map_err(|_| bad(n, l))?;
        *driven.get_mut(i).ok_or_else(|| bad(n, l))? = true;
    }
    let (n, l) = next()?;
    let count: usize = field(n, l, "frames")?.parse().map_err(|_| bad(n, l))?;
    let mut flags = Vec::new();
    let mut hits = Vec::new();
    let mut frames: Vec<Vec<Trs>> = Vec::with_capacity(count);
    let mut root_motion = Vec::with_capacity(count);
    for (n, l) in lines {
        let w: Vec<&str> = l.split_whitespace().collect();
        let num = |s: &str| {
            s.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && (*v as f32).is_finite())
                .ok_or_else(|| bad(n, l))
        };
        match w.first().copied() {
            Some("flag") | Some("hit") if w.len() == 4 => {
                let id: i32 = w[1].parse().map_err(|_| bad(n, l))?;
                let (start, end) = (num(w[2])? as f32, num(w[3])? as f32);
                if w[0] == "flag" {
                    flags.push(FlagWindow {
                        flag: id,
                        start,
                        end,
                    });
                } else {
                    hits.push(HitWindow {
                        judge: id,
                        start,
                        end,
                    });
                }
            }
            Some("frame") if w.len() == 7 && w[2] == "root" => {
                if w[1].parse::<usize>().ok() != Some(frames.len()) {
                    return Err(bad(n, l));
                }
                root_motion.push([
                    num(w[3])? as f32,
                    num(w[4])? as f32,
                    num(w[5])? as f32,
                    num(w[6])? as f32,
                ]);
                frames.push(Vec::with_capacity(bones));
            }
            Some("b") if matches!(w.len(), 8 | 11) => {
                let frame = frames.last_mut().ok_or_else(|| bad(n, l))?;
                if w[1].parse::<usize>().ok() != Some(frame.len()) {
                    return Err(bad(n, l));
                }
                frame.push(Trs {
                    t: [num(w[5])? as f32, num(w[6])? as f32, num(w[7])? as f32],
                    r: quat_of_degrees([num(w[2])?, num(w[3])?, num(w[4])?]),
                    s: if w.len() == 11 {
                        [num(w[8])? as f32, num(w[9])? as f32, num(w[10])? as f32]
                    } else {
                        [1.0; 3]
                    },
                });
            }
            _ => return Err(bad(n, l)),
        }
    }
    if !fps.is_finite()
        || fps <= 0.0
        || count < 2
        || bones == 0
        || parents
            .iter()
            .enumerate()
            .any(|(i, &p)| p < -1 || p >= i as i16)
        || frames.len() != count
        || frames.iter().any(|f| f.len() != bones)
    {
        return Err(format!(
            "{name}: {} frames of {count}, or a frame without all {bones} bones",
            frames.len()
        ));
    }
    Ok(Clip {
        name,
        fps,
        model_space,
        parents,
        driven,
        frames,
        root_motion,
        flags,
        hits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q_close(a: [f32; 4], b: [f32; 4]) -> bool {
        let dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        dot.abs() > 1.0 - 1.0e-6
    }

    #[test]
    fn degrees_and_back_give_the_same_turn() {
        for q in [
            [0.0, 0.0, 0.0, 1.0],
            [0.3, 0.1, -0.2, 0.927],
            [-0.5, 0.5, 0.5, 0.5],
            [0.0, 0.70710677, 0.0, 0.70710677],
            [0.6, -0.2, 0.1, 0.766],
        ] {
            let n = q.iter().map(|v| v * v).sum::<f32>().sqrt();
            let q = q.map(|v| v / n);
            assert!(q_close(quat_of_degrees(euler_degrees(q)), q), "{q:?}");
        }
        // The order is yaw, then pitch, then roll: a quarter pitch then turned a quarter yaw.
        let q = quat_of_degrees([90.0, 0.0, 0.0]);
        assert!((q[0] - 0.70710677).abs() < 1.0e-6 && q[3] > 0.7);
    }

    #[test]
    fn a_clip_written_as_text_reads_back_the_same() {
        let clip = Clip {
            name: "t".into(),
            fps: 30.0,
            model_space: false,
            parents: vec![-1, 0],
            driven: vec![false, true],
            frames: vec![
                vec![
                    Trs::IDENTITY,
                    Trs {
                        t: [0.1, 1.2, -0.3],
                        r: [0.3, 0.1, -0.2, 0.927],
                        s: [1.0; 3],
                    },
                ];
                3
            ],
            root_motion: vec![[0.0, 0.0, -1.5, 0.25]; 3],
            flags: vec![FlagWindow {
                flag: 8,
                start: 0.1,
                end: 0.4,
            }],
            hits: vec![],
        };
        let back = read(&write(&clip)).unwrap();
        assert_eq!(back.parents, clip.parents);
        assert_eq!(back.driven, clip.driven);
        assert_eq!(back.flags.len(), 1);
        assert_eq!(back.root_motion[2], [0.0, 0.0, -1.5, 0.25]);
        let (a, b) = (&back.frames[1][1], &clip.frames[1][1]);
        let n = b.r.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(q_close(a.r, b.r.map(|v| v / n)));
        assert!((a.t[1] - 1.2).abs() < 1.0e-6);
    }

    #[test]
    fn near_vertical_turns_and_non_unit_scale_survive_text() {
        let q = quat_of_degrees([89.999, 132.0, -47.0]);
        let back = quat_of_degrees(euler_degrees(q));
        let dot: f32 = q.iter().zip(back).map(|(a, b)| a * b).sum();
        let sign = if dot < 0.0 { -1.0 } else { 1.0 };
        assert!(q
            .iter()
            .zip(back)
            .all(|(&a, b)| (a - sign * b).abs() < 2.0e-5));
        let clip = Clip {
            name: "scale".into(),
            fps: 30.0,
            parents: vec![-1],
            driven: vec![true],
            frames: vec![
                vec![Trs {
                    r: q,
                    s: [0.75, 1.25, 0.5],
                    ..Trs::IDENTITY
                }];
                2
            ],
            root_motion: vec![[0.0; 4]; 2],
            ..Default::default()
        };
        let text = write(&clip);
        assert_eq!(read(&text).unwrap().frames[0][0].s, [0.75, 1.25, 0.5]);
        assert!(read(&text.replace("fps 30", "fps NaN")).is_err());
        assert!(read(&text.replace("0.75", "NaN")).is_err());
        let mut unscaled = clip;
        for frame in &mut unscaled.frames {
            frame[0].s = [1.0; 3];
        }
        assert!(read(&write(&unscaled).replacen("wsanim 2", "wsanim 1", 1)).is_ok());
    }
}
