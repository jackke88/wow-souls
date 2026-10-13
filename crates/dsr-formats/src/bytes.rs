//! Bounds-checked reads; every out-of-range read names its offset.

use anyhow::{Context, Result};

pub(crate) fn slice(b: &[u8], at: usize, n: usize) -> Result<&[u8]> {
    let end = at.checked_add(n).context("offset overflow")?;
    b.get(at..end)
        .with_context(|| format!("{n} bytes at {at:#x} run past the end ({:#x})", b.len()))
}

fn arr<const N: usize>(b: &[u8], at: usize) -> Result<[u8; N]> {
    Ok(slice(b, at, N)?.try_into().expect("slice length is N"))
}

pub(crate) fn u8_at(b: &[u8], at: usize) -> Result<u8> {
    Ok(slice(b, at, 1)?[0])
}

pub(crate) fn u16_at(b: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(arr(b, at)?))
}

pub(crate) fn i16_at(b: &[u8], at: usize) -> Result<i16> {
    Ok(i16::from_le_bytes(arr(b, at)?))
}

pub(crate) fn u32_at(b: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(arr(b, at)?))
}

pub(crate) fn i32_at(b: &[u8], at: usize) -> Result<i32> {
    Ok(i32::from_le_bytes(arr(b, at)?))
}

pub(crate) fn u64_at(b: &[u8], at: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(arr(b, at)?))
}

pub(crate) fn f32_at(b: &[u8], at: usize) -> Result<f32> {
    Ok(f32::from_le_bytes(arr(b, at)?))
}

pub(crate) fn u32_be_at(b: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(arr(b, at)?))
}

/// A NUL-terminated string at `at` (Shift-JIS in the archives; DSR's paths are ASCII).
pub(crate) fn cstr_at(b: &[u8], at: usize) -> Result<String> {
    let tail = b
        .get(at..)
        .with_context(|| format!("string at {at:#x} past the end"))?;
    let len = tail
        .iter()
        .position(|&c| c == 0)
        .with_context(|| format!("unterminated string at {at:#x}"))?;
    Ok(String::from_utf8_lossy(&tail[..len]).into_owned())
}
