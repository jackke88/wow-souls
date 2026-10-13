//! `hkaSplineCompressedAnimation` data: per block, per transform track, a mask (quantization
//! and static/spline flags per axis), then position, rotation and scale as static values or
//! B-spline control points with byte knots; sampled per frame.
//!
//! Ported from SoulsAssetPipeline `SplineCompressedAnimation.cs` (GPL-3.0, after HavokLib's
//! `hka_spline_decompressor.cpp`). Deviation: `POLAR32` converts its 10-bit radius numerically,
//! as HavokLib does (the C# port reinterprets the bits as a float).

use anyhow::{bail, Context, Result};

use super::animation::SplineAnim;
use super::skeleton::Qs;
use crate::bytes::{f32_at, i16_at, slice, u16_at, u8_at};

const STATIC: [u8; 4] = [0x01, 0x02, 0x04, 0x08];
const SPLINE: [u8; 4] = [0x10, 0x20, 0x40, 0x80];

#[derive(Debug, Clone, Copy)]
enum Scalar {
    B8,
    B16,
}

#[derive(Debug, Clone, Copy)]
enum Rot {
    Polar32,
    ThreeComp40,
    ThreeComp48,
    Uncompressed,
}

impl Rot {
    fn from_bits(v: u8) -> Result<Rot> {
        Ok(match v {
            0 => Rot::Polar32,
            1 => Rot::ThreeComp40,
            2 => Rot::ThreeComp48,
            5 => Rot::Uncompressed,
            other => bail!("rotation quantization {other} is not supported"),
        })
    }

    fn align(self) -> usize {
        match self {
            Rot::Polar32 | Rot::Uncompressed => 4,
            Rot::ThreeComp40 => 1,
            Rot::ThreeComp48 => 2,
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Result<u8> {
        let v = u8_at(self.b, self.p)?;
        self.p += 1;
        Ok(v)
    }
    fn u16(&mut self) -> Result<u16> {
        let v = u16_at(self.b, self.p)?;
        self.p += 2;
        Ok(v)
    }
    fn i16(&mut self) -> Result<i16> {
        let v = i16_at(self.b, self.p)?;
        self.p += 2;
        Ok(v)
    }
    fn f32(&mut self) -> Result<f32> {
        let v = f32_at(self.b, self.p)?;
        self.p += 4;
        Ok(v)
    }
    fn bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        let v = slice(self.b, self.p, n)?.to_vec();
        self.p += n;
        Ok(v)
    }
    fn align(&mut self, n: usize) {
        self.p = self.p.div_ceil(n) * n;
    }
    fn scalar(&mut self, q: Scalar, min: f32, max: f32) -> Result<f32> {
        let ratio = match q {
            Scalar::B8 => f32::from(self.u8()?) / 255.0,
            Scalar::B16 => f32::from(self.u16()?) / 65535.0,
        };
        Ok(min + (max - min) * ratio)
    }
    fn quat(&mut self, q: Rot) -> Result<[f32; 4]> {
        match q {
            Rot::Polar32 => {
                let v = u32::from_le_bytes(self.bytes(4)?.try_into().expect("4 bytes"));
                Ok(polar32(v))
            }
            Rot::ThreeComp40 => {
                let b = self.bytes(5)?;
                let mut w = [0u8; 8];
                w[..5].copy_from_slice(&b);
                Ok(three_comp40(u64::from_le_bytes(w)))
            }
            Rot::ThreeComp48 => {
                let (x, y, z) = (self.i16()?, self.i16()?, self.i16()?);
                Ok(three_comp48(x, y, z))
            }
            Rot::Uncompressed => Ok([self.f32()?, self.f32()?, self.f32()?, self.f32()?]),
        }
    }
}

/// Three stored components and the dropped one's index and sign.
fn rebuild(c: [f32; 3], dropped: usize, negative: bool) -> [f32; 4] {
    let mut q = [0.0; 4];
    let mut k = 0;
    for (i, slot) in q.iter_mut().enumerate() {
        if i != dropped {
            *slot = c[k];
            k += 1;
        }
    }
    let rest = 1.0 - c[0] * c[0] - c[1] * c[1] - c[2] * c[2];
    let w = if rest <= 0.0 { 0.0 } else { rest.sqrt() };
    q[dropped] = if negative { -w } else { w };
    q
}

pub(crate) fn three_comp40(v: u64) -> [f32; 4] {
    const MASK: u64 = (1 << 12) - 1;
    const HALF: i32 = (MASK >> 1) as i32;
    const FRACTAL: f32 = 0.000_345_436;
    let comp = |s: u32| ((v >> s) & MASK) as i32 - HALF;
    let c = [comp(0), comp(12), comp(24)].map(|x| x as f32 * FRACTAL);
    rebuild(c, ((v >> 36) & 3) as usize, (v >> 38) & 1 != 0)
}

pub(crate) fn three_comp48(x: i16, y: i16, z: i16) -> [f32; 4] {
    const MASK: i32 = (1 << 15) - 1;
    const FRACTAL: f32 = 0.000_043_161;
    let (x, y, z) = (i32::from(x), i32::from(y), i32::from(z));
    let dropped = (((y >> 14) & 2) | ((x >> 15) & 1)) as usize;
    let negative = (z >> 15) != 0;
    let c = [x, y, z].map(|v| ((v & MASK) - (MASK >> 1)) as f32 * FRACTAL);
    rebuild(c, dropped, negative)
}

pub(crate) fn polar32(v: u32) -> [f32; 4] {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
    let r = ((v >> 18) & 0x3FF) as f32 / 1023.0;
    let r = 1.0 - r * r;
    let phi_theta = (v & 0x3_FFFF) as f32;
    let mut phi = phi_theta.sqrt().floor();
    let mut theta = 0.0;
    if phi > 0.0 {
        theta = FRAC_PI_4 * (phi_theta - phi * phi) / phi;
        phi *= FRAC_PI_2 / 511.0;
    }
    let mag = (1.0 - r * r).max(0.0).sqrt();
    let mut q = [
        phi.sin() * theta.cos() * mag,
        phi.sin() * theta.sin() * mag,
        phi.cos() * mag,
        r,
    ];
    for (i, bit) in [0x1000_0000u32, 0x2000_0000, 0x4000_0000, 0x8000_0000]
        .iter()
        .enumerate()
    {
        if v & bit != 0 {
            q[i] = -q[i];
        }
    }
    q
}

#[derive(Debug, Clone)]
enum Channel {
    Absent,
    Static(f32),
    Spline(Vec<f32>),
}

#[derive(Debug, Clone)]
struct Curve {
    degree: usize,
    knots: Vec<u8>,
}

#[derive(Debug, Clone)]
struct VecTrack {
    curve: Option<Curve>,
    axes: [Channel; 3],
}

#[derive(Debug, Clone)]
enum QuatTrack {
    None,
    Static([f32; 4]),
    Spline(Curve, Vec<[f32; 4]>),
}

#[derive(Debug, Clone)]
struct Track {
    pos: VecTrack,
    rot: QuatTrack,
    scale: VecTrack,
}

fn read_curve(r: &mut Reader) -> Result<(Curve, usize)> {
    let n = usize::try_from(r.i16()?).context("negative control point count")?;
    let degree = usize::from(r.u8()?);
    if degree > 4 {
        bail!("spline degree {degree} is above 4");
    }
    if n > 0 && degree > n {
        bail!(
            "spline degree {degree} needs more than {} control points",
            n + 1
        );
    }
    let knots = r.bytes(n + degree + 2)?;
    Ok((Curve { degree, knots }, n))
}

fn read_vec(r: &mut Reader, flags: u8, q: Scalar) -> Result<VecTrack> {
    if flags & 0x70 == 0 {
        let mut axes = [Channel::Absent, Channel::Absent, Channel::Absent];
        for (i, a) in axes.iter_mut().enumerate() {
            if flags & STATIC[i] != 0 {
                *a = Channel::Static(r.f32()?);
            }
        }
        return Ok(VecTrack { curve: None, axes });
    }
    let (curve, n) = read_curve(r)?;
    r.align(4);
    let mut bounds = [(0.0f32, 0.0f32); 3];
    let mut axes = [Channel::Absent, Channel::Absent, Channel::Absent];
    for i in 0..3 {
        if flags & SPLINE[i] != 0 {
            bounds[i] = (r.f32()?, r.f32()?);
            axes[i] = Channel::Spline(Vec::with_capacity(n + 1));
        } else if flags & STATIC[i] != 0 {
            axes[i] = Channel::Static(r.f32()?);
        }
    }
    for _ in 0..=n {
        for i in 0..3 {
            if let Channel::Spline(v) = &mut axes[i] {
                v.push(r.scalar(q, bounds[i].0, bounds[i].1)?);
            }
        }
    }
    Ok(VecTrack {
        curve: Some(curve),
        axes,
    })
}

fn read_quat(r: &mut Reader, flags: u8, q: Rot) -> Result<QuatTrack> {
    if flags & 0xF0 != 0 {
        let (curve, n) = read_curve(r)?;
        r.align(q.align());
        let points = (0..=n).map(|_| r.quat(q)).collect::<Result<Vec<_>>>()?;
        return Ok(QuatTrack::Spline(curve, points));
    }
    if flags & 0x0F != 0 {
        r.align(q.align());
        return Ok(QuatTrack::Static(r.quat(q)?));
    }
    Ok(QuatTrack::None)
}

/// The block's tracks and the position where its parse ended.
fn read_block(data: &[u8], at: usize, tracks: usize) -> Result<(Vec<Track>, usize)> {
    let mut r = Reader { b: data, p: at };
    let mut masks = Vec::with_capacity(tracks);
    for _ in 0..tracks {
        let q = r.u8()?;
        let (pos, rot, scale) = (r.u8()?, r.u8()?, r.u8()?);
        masks.push((q, pos, rot, scale));
    }
    r.align(4);
    let scalar = |v: u8| if v & 3 == 0 { Scalar::B8 } else { Scalar::B16 };
    let mut out = Vec::with_capacity(tracks);
    for &(q, pos_f, rot_f, scale_f) in &masks {
        let pos = read_vec(&mut r, pos_f, scalar(q))?;
        r.align(4);
        let rot = read_quat(&mut r, rot_f, Rot::from_bits((q >> 2) & 0xF)?)?;
        r.align(4);
        let scale = read_vec(&mut r, scale_f, scalar(q >> 6))?;
        r.align(4);
        out.push(Track { pos, rot, scale });
    }
    Ok((out, r.p))
}

/// The knot span holding `u` (The NURBS Book, A2.1).
fn find_span(degree: usize, u: f32, points: usize, knots: &[u8]) -> usize {
    if u >= f32::from(knots[points]) {
        return points - 1;
    }
    let (mut low, mut high) = (degree, points);
    let mut mid = (low + high) / 2;
    for _ in 0..64 {
        if u >= f32::from(knots[mid]) && u < f32::from(knots[mid + 1]) {
            break;
        }
        if u < f32::from(knots[mid]) {
            high = mid;
        } else {
            low = mid;
        }
        mid = (low + high) / 2;
    }
    mid
}

/// The B-spline basis weights at `u` for the span (as the reference evaluates them).
fn basis(span: usize, degree: usize, u: f32, knots: &[u8]) -> [f32; 5] {
    let k = |i: usize| f32::from(knots[i]);
    let mut n = [1.0, 0.0, 0.0, 0.0, 0.0];
    for i in 1..=degree {
        for j in (0..i).rev() {
            let denom = k(span + i - j) - k(span - j);
            let a = if denom == 0.0 {
                0.0
            } else {
                (u - k(span - j)) / denom
            };
            let tmp = n[j] * a;
            n[j + 1] += n[j] - tmp;
            n[j] = tmp;
        }
    }
    n
}

fn eval_scalar(c: &Curve, u: f32, points: &[f32]) -> f32 {
    if points.len() == 1 {
        return points[0];
    }
    let span = find_span(c.degree, u, points.len(), &c.knots);
    let n = basis(span, c.degree, u, &c.knots);
    (0..=c.degree).map(|i| points[span - i] * n[i]).sum()
}

fn eval_quat(c: &Curve, u: f32, points: &[[f32; 4]]) -> [f32; 4] {
    if points.len() == 1 {
        return points[0];
    }
    let span = find_span(c.degree, u, points.len(), &c.knots);
    let n = basis(span, c.degree, u, &c.knots);
    let mut q = [0.0; 4];
    for i in 0..=c.degree {
        for (k, v) in q.iter_mut().enumerate() {
            *v += points[span - i][k] * n[i];
        }
    }
    let len = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if len > 0.0 {
        q.map(|v| v / len)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

fn vec_at(t: &VecTrack, u: f32, default: f32) -> [f32; 3] {
    let mut out = [default; 3];
    for (i, a) in t.axes.iter().enumerate() {
        out[i] = match a {
            Channel::Absent => default,
            Channel::Static(v) => *v,
            Channel::Spline(p) => eval_scalar(t.curve.as_ref().expect("spline has a curve"), u, p),
        };
    }
    out
}

/// Every frame of every transform track: `frames[f][track]`.
pub fn decode(anim: &SplineAnim) -> Result<Vec<Vec<Qs>>> {
    if anim.blocks == 0 || anim.block_offsets.len() < anim.blocks {
        bail!(
            "animation has {} blocks and {} block offsets",
            anim.blocks,
            anim.block_offsets.len()
        );
    }
    let mut blocks = Vec::with_capacity(anim.blocks);
    for b in 0..anim.blocks {
        let (tracks, end) = read_block(&anim.data, anim.block_offsets[b], anim.tracks)?;
        // Blocks are padded to 16 bytes, so a parse that ends past the next block or more than
        // 15 bytes short of it has misread the layout.
        let next = if b + 1 < anim.blocks {
            anim.block_offsets[b + 1]
        } else {
            anim.data.len()
        };
        if end > next || next - end > 15 {
            bail!("block {b} parse ends at {end:#x}, next starts at {next:#x}");
        }
        blocks.push(tracks);
    }
    let mut frames = Vec::with_capacity(anim.frames);
    for f in 0..anim.frames {
        let t = f as f32 * anim.frame_duration;
        // Real data has multi-block clips (a00_7891: 2 blocks, 261 frames); each block covers
        // `blockDuration` and its splines run on block-local frame time.
        let block = (((t + 1.0e-4) / anim.block_duration) as usize).min(anim.blocks - 1);
        // HavokLib clamps a negative local time.
        let u = ((t - block as f32 * anim.block_duration) / anim.frame_duration).max(0.0);
        let frame = blocks[block]
            .iter()
            .map(|tr| Qs {
                t: vec_at(&tr.pos, u, 0.0),
                r: match &tr.rot {
                    QuatTrack::None => [0.0, 0.0, 0.0, 1.0],
                    QuatTrack::Static(q) => *q,
                    QuatTrack::Spline(c, p) => eval_quat(c, u, p),
                },
                s: vec_at(&tr.scale, u, 1.0),
            })
            .collect();
        frames.push(frame);
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 4], b: [f32; 4]) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1.0e-3)
    }

    #[test]
    fn quantized_identities_decode_to_identity() {
        let id = [0.0, 0.0, 0.0, 1.0];
        let v40 = 0x7FFu64 | (0x7FF << 12) | (0x7FF << 24) | (3 << 36);
        assert!(close(three_comp40(v40), id), "{:?}", three_comp40(v40));
        // x and y carry the dropped index (3) in their top bits; all three are the midpoint.
        let x = 0xBFFFu16 as i16;
        let y = 0xBFFFu16 as i16;
        assert!(
            close(three_comp48(x, y, 0x3FFF), id),
            "{:?}",
            three_comp48(x, y, 0x3FFF)
        );
        assert!(close(polar32(0), id), "{:?}", polar32(0));
    }

    #[test]
    fn three_comp40_places_the_dropped_component_with_its_sign() {
        const FRACTAL: f32 = 0.000_345_436;
        let (x, y, z) = (1000u64, 3000u64, 2047u64);
        let v = x | (y << 12) | (z << 24) | (1 << 36) | (1 << 38);
        let c = [x, y, z].map(|f| (f as f32 - 2047.0) * FRACTAL);
        let dropped = -(1.0 - c[0] * c[0] - c[1] * c[1] - c[2] * c[2]).sqrt();
        let expected = [c[0], dropped, c[1], c[2]];
        let q = three_comp40(v);
        assert!(
            q.iter().zip(&expected).all(|(a, b)| (a - b).abs() < 1.0e-6),
            "{q:?} != {expected:?}"
        );
        assert!(q[1] < 0.0 && q[1].abs() > q[0].abs().max(q[2].abs()).max(q[3].abs()));
    }

    #[test]
    fn a_degree_one_spline_interpolates_between_its_points() {
        let c = Curve {
            degree: 1,
            knots: vec![0, 0, 10, 10],
        };
        assert!((eval_scalar(&c, 0.0, &[2.0, 4.0]) - 2.0).abs() < 1.0e-6);
        assert!((eval_scalar(&c, 5.0, &[2.0, 4.0]) - 3.0).abs() < 1.0e-6);
        assert!((eval_scalar(&c, 10.0, &[2.0, 4.0]) - 4.0).abs() < 1.0e-6);
    }

    /// One block, one track: static position (1, 2, 3), an uncompressed degree-1 rotation
    /// spline from identity to a half turn about Y over frames 0..10, no scale.
    fn one_track_anim() -> SplineAnim {
        let mut d = Vec::new();
        d.extend_from_slice(&[0x14, 0x07, 0xF0, 0x00]); // rot UNCOMPRESSED; static xyz; spline xyzw
        for v in [1.0f32, 2.0, 3.0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&1i16.to_le_bytes()); // n = 1: two control points
        d.push(1); // degree
        d.extend_from_slice(&[0, 0, 10, 10]); // knots
        while d.len() % 4 != 0 {
            d.push(0);
        }
        for q in [[0.0f32, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 0.0]] {
            for v in q {
                d.extend_from_slice(&v.to_le_bytes());
            }
        }
        while d.len() % 16 != 0 {
            d.push(0);
        }
        SplineAnim {
            duration: 10.0 / 30.0,
            tracks: 1,
            frames: 11,
            blocks: 1,
            frames_per_block: 11,
            block_duration: 10.0 / 30.0,
            frame_duration: 1.0 / 30.0,
            block_offsets: vec![0],
            data: d,
        }
    }

    #[test]
    fn a_block_decodes_to_its_frames() {
        let frames = decode(&one_track_anim()).unwrap();
        assert_eq!(frames.len(), 11);
        assert_eq!(frames[0][0].t, [1.0, 2.0, 3.0]);
        assert_eq!(frames[0][0].s, [1.0, 1.0, 1.0]);
        assert!(close(frames[0][0].r, [0.0, 0.0, 0.0, 1.0]));
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            close(frames[5][0].r, [0.0, h, 0.0, h]),
            "{:?}",
            frames[5][0].r
        );
        assert!(
            close(frames[10][0].r, [0.0, 1.0, 0.0, 0.0]),
            "{:?}",
            frames[10][0].r
        );
    }
}
