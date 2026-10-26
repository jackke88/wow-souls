//! Opt-in wolf motion experiment (`WOWSOULS_WOLF_MOTION=1`). DSR-derived, not an independent
//! animation asset: the source body/feet are resampled, with newly authored head/jaw/tail curves.
//! Recipes below are the editable numbers. Cache files and the original DS reference are never
//! rewritten. Pose, root motion and combat events share one monotone clock.

use bevy::prelude::*;

use super::remix::{Clock, FPS};
use super::MonsterClips;

/// Degrees about parent-local X/Y/Z: pitch, yaw, roll. Jaw is pitch; tail is yaw.
#[derive(Clone, Copy)]
struct Key {
    at: f32,
    neck: [f32; 3],
    head: [f32; 3],
    jaw: f32,
    tail: f32,
}

impl Key {
    const fn new(at: f32, neck: [f32; 3], head: [f32; 3], jaw: f32, tail: f32) -> Self {
        Self {
            at,
            neck,
            head,
            jaw,
            tail,
        }
    }
}

struct Recipe {
    name: &'static str,
    source: &'static str,
    length: f32,
    gather: f32,
    strike: (f32, f32),
    keys: &'static [Key],
}

// Gather low with the nose turned aside, whip across the centre, then recoil past it.
// Full wind-up -> sharp bite -> a recovery the player can punish; no damage/reach increase.
const BITE: Recipe = Recipe {
    name: "bite",
    source: "body_attack_01",
    length: 1.5,
    gather: 0.40,
    strike: (0.58, 0.74),
    keys: &[
        Key::new(0.00, [0.0; 3], [0.0; 3], 0.0, 0.0),
        Key::new(0.18, [-3.0, 2.0, 0.0], [3.0, -7.0, -2.0], -4.0, 4.0),
        Key::new(0.40, [-8.0, 4.0, 0.0], [6.0, -18.0, -4.0], -16.0, 10.0),
        Key::new(0.58, [-3.0, -2.0, 0.0], [-8.0, 10.0, 3.0], -3.0, -6.0),
        Key::new(0.74, [2.0, -2.0, 0.0], [-2.0, 13.0, 2.0], 5.0, -12.0),
        Key::new(1.02, [3.0, 1.0, 0.0], [7.0, -5.0, -2.0], -5.0, 5.0),
        Key::new(1.50, [0.0; 3], [0.0; 3], 0.0, 0.0),
    ],
};

// A longer visible gather than the source's 0.333 s; the body commits to the same distance.
// Nose low during the spring, head lifts on landing and tail counter-swings after the bite.
const LUNGE: Recipe = Recipe {
    name: "lunge",
    source: "body_attack_05",
    length: 1.6,
    gather: 0.30,
    strike: (0.46, 0.70),
    keys: &[
        Key::new(0.00, [0.0; 3], [0.0; 3], 0.0, 0.0),
        Key::new(0.16, [-4.0, -2.0, 0.0], [4.0, 5.0, 2.0], -5.0, -3.0),
        Key::new(0.30, [-9.0, -3.0, 0.0], [6.0, 10.0, 3.0], -14.0, -8.0),
        Key::new(0.46, [-4.0, 1.0, 0.0], [-9.0, -7.0, -2.0], -8.0, 4.0),
        Key::new(0.70, [3.0, 2.0, 0.0], [5.0, -10.0, -3.0], 4.0, 12.0),
        Key::new(1.00, [4.0, -1.0, 0.0], [8.0, 4.0, 1.0], -4.0, -5.0),
        Key::new(1.60, [0.0; 3], [0.0; 3], 0.0, 0.0),
    ],
};

struct Rig {
    bones: usize,
    neck: usize,
    head: usize,
    jaw: usize,
    // Wolf's five tail bones are siblings; DireWolf's four form a chain.
    tail: &'static [(usize, f32)],
}

const WOLF: Rig = Rig {
    bones: 64,
    neck: 25,
    head: 29,
    jaw: 47,
    tail: &[(42, 0.25), (41, 0.45), (40, 0.65), (39, 0.85), (38, 1.0)],
};
const DIREWOLF: Rig = Rig {
    bones: 61,
    neck: 21,
    head: 25,
    jaw: 35,
    tail: &[(46, 0.20), (52, 0.20), (57, 0.20), (60, 0.20)],
};

/// The wolf phase clock: gather, strike window, recovery (source time against remixed time).
fn clock(source: &wscl::Clip, recipe: &Recipe) -> Result<Clock, String> {
    let Some(hit) = source.hits.first() else {
        return Err("clip has no hit window".into());
    };
    let end = source.hits.iter().map(|h| h.end).fold(hit.end, f32::max);
    Clock::new(vec![
        (0.0, 0.0),
        (recipe.gather, hit.start * 0.55),
        (recipe.strike.0, hit.start),
        (recipe.strike.1, end),
        (recipe.length, source.duration()),
    ])
}

fn angles(v: [f32; 3]) -> Quat {
    Quat::from_euler(
        EulerRot::XYZ,
        v[0].to_radians(),
        v[1].to_radians(),
        v[2].to_radians(),
    )
}

/// Smoothstep between authored offsets: zero velocity at each pose, no sudden jaw/tail snaps.
fn shape(frame: &mut [wscl::Trs], t: f32, recipe: &Recipe, rig: &Rig) {
    let i = recipe
        .keys
        .partition_point(|k| k.at < t)
        .saturating_sub(1)
        .min(recipe.keys.len() - 2);
    let (a, b) = (&recipe.keys[i], &recipe.keys[i + 1]);
    let u = ((t - a.at) / (b.at - a.at)).clamp(0.0, 1.0);
    let u = u * u * (3.0 - 2.0 * u);
    let lerp = |a: f32, b: f32| a + (b - a) * u;
    let triple = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|i| lerp(a[i], b[i]));
    let mut turn = |bone: usize, delta: Quat| {
        frame[bone].r = (delta * Quat::from_array(frame[bone].r))
            .normalize()
            .to_array();
    };
    turn(rig.neck, angles(triple(a.neck, b.neck)));
    turn(rig.head, angles(triple(a.head, b.head)));
    turn(
        rig.jaw,
        Quat::from_rotation_x(lerp(a.jaw, b.jaw).to_radians()),
    );
    for &(bone, weight) in rig.tail {
        turn(
            bone,
            Quat::from_rotation_y((lerp(a.tail, b.tail) * weight).to_radians()),
        );
    }
}

fn bake(source: &wscl::Clip, recipe: &Recipe, rig: &Rig) -> Result<wscl::Clip, String> {
    if source.name != recipe.source
        || source.model_space
        || source.bones() != rig.bones
        || source.parents[rig.head] != rig.neck as i16
        || source.parents[rig.jaw] != rig.head as i16
    {
        return Err(format!("unexpected {} source/rig", recipe.name));
    }
    let clock = clock(source, recipe)?;
    let mut out = source.clone();
    out.fps = FPS;
    let count = (recipe.length * FPS).round() as usize + 1;
    out.frames = (0..count)
        .map(|i| {
            if i == 0 {
                return source.frames[0].clone();
            }
            if i + 1 == count {
                return source.frames.last().unwrap().clone();
            }
            let t = i as f32 / FPS;
            let old = clock.source_at(t);
            let mut frame: Vec<wscl::Trs> = (0..rig.bones)
                .map(|b| {
                    let (t, r, s) = crate::motion_clip::sample(source, old, b);
                    wscl::Trs {
                        t: t.to_array(),
                        r: r.to_array(),
                        s: s.to_array(),
                    }
                })
                .collect();
            shape(&mut frame, t, recipe, rig);
            frame
        })
        .collect();
    out.root_motion = (0..count)
        .map(|i| {
            if i == 0 {
                return source.root_motion[0];
            }
            if i + 1 == count {
                return *source.root_motion.last().unwrap();
            }
            let f = clock.source_at(i as f32 / FPS) * source.fps;
            let j = (f as usize).min(source.root_motion.len() - 2);
            let u = f - j as f32;
            std::array::from_fn(|axis| {
                source.root_motion[j][axis]
                    + (source.root_motion[j + 1][axis] - source.root_motion[j][axis]) * u
            })
        })
        .collect();
    for flag in &mut out.flags {
        flag.start = clock.target_at(flag.start);
        flag.end = clock.target_at(flag.end);
    }
    for hit in &mut out.hits {
        hit.start = clock.target_at(hit.start);
        hit.end = clock.target_at(hit.end);
    }
    Ok(out)
}

/// Apply a complete kit atomically. Unknown kits are untouched; a mismatched rig keeps the
/// existing kit. The source DS skeleton stays unedited for comparison/provenance.
fn apply(kit: &mut MonsterClips) -> Result<usize, String> {
    let rig = match kit.name.as_str() {
        "wolf" => &WOLF,
        "direwolf" => &DIREWOLF,
        _ => return Ok(0),
    };
    let mut changed = Vec::new();
    for (i, a) in kit.attacks.iter().enumerate() {
        let recipe = match a.spec.name.as_str() {
            "bite" => &BITE,
            "lunge" => &LUNGE,
            _ => continue,
        };
        changed.push((i, bake(&a.wow, recipe, rig)?));
    }
    let count = changed.len();
    for (i, clip) in changed {
        let attack = &mut kit.attacks[i];
        let before = (attack.spec.length, attack.spec.hit);
        attack.spec.length = clip.duration();
        attack.spec.hits = clip.hits.iter().map(|h| (h.start, h.end)).collect();
        attack.spec.hit = attack.spec.hits[0];
        info!("wolf-motion: {} {} source-derived remix: {:.3}s hit {:.3}-{:.3} -> {:.3}s hit {:.3}-{:.3}; source {} (reference unedited)",
            kit.name,attack.spec.name,before.0,before.1.0,before.1.1,
            attack.spec.length,attack.spec.hit.0,attack.spec.hit.1,clip.name);
        attack.wow = clip;
    }
    Ok(count)
}

/// Whether the wolf kits carry a hand-authored recipe for `attack`: the general remix leaves it.
pub(super) fn authored(kit: &str, attack: &str) -> bool {
    matches!(
        (kit, attack),
        ("wolf", "bite" | "lunge") | ("direwolf", "bite")
    )
}

/// `WOWSOULS_WOLF_MOTION=1`: only the wolves' bite and lunge (`WOWSOULS_MOTION=1` does it all).
pub(super) fn enabled() -> bool {
    std::env::var("WOWSOULS_WOLF_MOTION").as_deref() == Ok("1")
}

pub(super) fn apply_loaded(kit: &mut MonsterClips) {
    if let Err(why) = apply(kit) {
        warn!("wolf-motion: keeping original {} kit: {why}", kit.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(frame: &[wscl::Trs], parents: &[i16], bone: usize) -> Vec3 {
        let mut point = Vec3::from_array(frame[bone].t);
        let mut parent = parents[bone];
        while let Ok(i) = usize::try_from(parent) {
            let tr = frame[i];
            point = Vec3::from_array(tr.t)
                + Quat::from_array(tr.r).normalize() * (Vec3::from_array(tr.s) * point);
            parent = parents[i];
        }
        point
    }

    #[test]
    fn installed_wolves_keep_feet_distance_endpoints_and_synchronized_events() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source");
        assert!(
            dir.join("wolf/body_attack_01.wsanim").is_file(),
            "project wolf clips are required"
        );
        let text = crate::motion_clip::checked_manifest(&dir).unwrap();
        let mut roster = super::super::parse_roster(&text, &dir).unwrap();
        for (name, rig, feet) in [
            ("wolf", &WOLF, &[43, 44, 62, 63][..]),
            ("direwolf", &DIREWOLF, &[53, 54, 58, 59][..]),
        ] {
            let kit = roster.0.get_mut(name).unwrap();
            for a in &kit.attacks {
                let recipe = if a.spec.name == "bite" { &BITE } else { &LUNGE };
                let c = bake(&a.wow, recipe, rig).unwrap();
                let clock = clock(&a.wow, recipe).unwrap();
                assert_eq!(c.frames[0], a.wow.frames[0]);
                assert_eq!(c.frames.last(), a.wow.frames.last());
                assert_eq!(c.root_motion.last(), a.wow.root_motion.last());
                assert_eq!(c.driven, a.wow.driven);
                assert!((c.hits[0].start - recipe.strike.0).abs() < 1e-5);
                assert!((c.hits[0].end - recipe.strike.1).abs() < 1e-5);
                for (old, new) in a.wow.flags.iter().zip(&c.flags) {
                    assert!(
                        (clock.source_at(new.start) - old.start.min(a.wow.duration())).abs() < 1e-5
                    );
                    assert!(
                        (clock.source_at(new.end) - old.end.min(a.wow.duration())).abs() < 1e-5
                    );
                }
                let mut max_foot_drift = 0.0_f32;
                for (i, frame) in c.frames.iter().enumerate() {
                    let old = clock.source_at(i as f32 / FPS);
                    assert!(i == 0 || old > clock.source_at((i - 1) as f32 / FPS));
                    let mut original_frame = Vec::new();
                    for (b, tr) in frame.iter().enumerate() {
                        let (t, r, s) = crate::motion_clip::sample(&a.wow, old, b);
                        original_frame.push(wscl::Trs {
                            t: t.to_array(),
                            r: r.to_array(),
                            s: s.to_array(),
                        });
                        assert!(
                            (Vec3::from_array(tr.t) - t).length() < 1e-5,
                            "bone length/position {b}"
                        );
                        assert!((Vec3::from_array(tr.s) - s).length() < 1e-5, "scale {b}");
                        assert!((Quat::from_array(tr.r).length() - 1.0).abs() < 1e-5);
                    }
                    for &b in feet {
                        let (_, r, _) = crate::motion_clip::sample(&a.wow, old, b);
                        let direction = Quat::from_array(frame[b].r).normalize() * Vec3::NEG_Z;
                        assert!((direction - r.normalize() * Vec3::NEG_Z).length() < 1e-5);
                        let drift = (point(frame, &c.parents, b)
                            - point(&original_frame, &a.wow.parents, b))
                        .length();
                        max_foot_drift = max_foot_drift.max(drift);
                        assert!(
                            drift < 1e-4,
                            "{name} {} frame {i} paw {b}: {drift}",
                            a.spec.name
                        );
                    }
                    assert!(
                        (crate::motion_clip::progress(&c, i as f32 / FPS)
                            - crate::motion_clip::progress(&a.wow, old))
                        .abs()
                            < 1e-5
                    );
                }
                let peak = (recipe.gather * FPS).round() as usize;
                let old = clock.source_at(peak as f32 / FPS);
                let (_, head, _) = crate::motion_clip::sample(&a.wow, old, rig.head);
                let head_delta = Quat::from_array(c.frames[peak][rig.head].r).angle_between(head);
                assert!(head_delta > 0.17);
                println!("wolf-motion measured: {name} {} {:.3}s -> {:.3}s, hit {:.3}-{:.3}, head delta {:.2}deg, max paw drift {:.8}yd, root end {:.6}yd",
                    a.spec.name,a.wow.duration(),c.duration(),c.hits[0].start,c.hits[0].end,
                    head_delta.to_degrees(),max_foot_drift,-c.root_motion.last().unwrap()[2]);
                // Only explicit verification exports local derived data; normal tests just inspect it.
                if let Some(out) = std::env::var_os("WOWSOULS_WOLF_EXPORT") {
                    let out = std::path::PathBuf::from(out).join(name);
                    std::fs::create_dir_all(&out).unwrap();
                    c.write(
                        &mut std::fs::File::create(out.join(format!("{}.wscl", c.name))).unwrap(),
                    )
                    .unwrap();
                }
            }
            let original_damage: Vec<_> = kit.attacks.iter().map(|a| a.spec.damage).collect();
            let original_reference: Vec<_> = kit
                .attacks
                .iter()
                .map(|a| a.reference.frames.clone())
                .collect();
            assert_eq!(apply(kit).unwrap(), kit.attacks.len());
            for ((a, damage), reference) in kit
                .attacks
                .iter()
                .zip(original_damage)
                .zip(original_reference)
            {
                assert_eq!(a.spec.damage, damage);
                assert_eq!(a.reference.frames, reference);
                assert!((a.spec.length - a.wow.duration()).abs() < 1e-5);
                assert_eq!(
                    a.spec.hits,
                    a.wow
                        .hits
                        .iter()
                        .map(|h| (h.start, h.end))
                        .collect::<Vec<_>>()
                );
            }
        }
        assert_eq!(apply(roster.0.get_mut("kobold").unwrap()).unwrap(), 0);
    }

    #[test]
    fn a_mismatched_rig_fails_before_any_attack_is_changed() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source");
        assert!(
            dir.join("wolf/body_attack_01.wsanim").is_file(),
            "project wolf clips are required"
        );
        let text = crate::motion_clip::checked_manifest(&dir).unwrap();
        let mut roster = super::super::parse_roster(&text, &dir).unwrap();
        let kit = roster.0.get_mut("wolf").unwrap();
        kit.attacks[1].wow.name = "different-source".into();
        let before = kit.attacks[0].wow.frames.clone();
        assert!(apply(kit).is_err());
        assert_eq!(kit.attacks[0].wow.frames, before);
    }
}
