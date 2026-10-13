//! DCX `DFLT`: a big-endian header around one zlib stream (SoulsFormatsNEXT `DCX.cs`,
//! `DecompressDCXDFLT`).

use std::io::Read;

use anyhow::{bail, Result};

use crate::bytes::{slice, u32_be_at};

/// Where the zlib stream starts: after `DCX`, `DCS`, `DCP`/`DFLT` and `DCA` blocks.
const ZLIB_AT: usize = 0x4C;

/// The decompressed payload of a `DCX`/`DFLT` file.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < ZLIB_AT || slice(data, 0, 4)? != b"DCX\0" {
        bail!("not a DCX file");
    }
    let method = slice(data, 0x28, 4)?;
    if method != b"DFLT" {
        bail!(
            "DCX compression {} is not DFLT",
            String::from_utf8_lossy(method)
        );
    }
    let size = u32_be_at(data, 0x1C)? as usize;
    let mut out = Vec::with_capacity(size);
    flate2::read::ZlibDecoder::new(&data[ZLIB_AT..]).read_to_end(&mut out)?;
    if out.len() != size {
        bail!("DCX inflated {} bytes, its header says {size}", out.len());
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write;

    use super::*;

    /// A DCX around `payload`, laid out as DSR's files are.
    pub(crate) fn dcx_of(payload: &[u8]) -> Vec<u8> {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(payload).unwrap();
        let zlib = z.finish().unwrap();
        let mut out = vec![0u8; ZLIB_AT];
        out[0..4].copy_from_slice(b"DCX\0");
        out[0x18..0x1C].copy_from_slice(b"DCS\0");
        out[0x1C..0x20].copy_from_slice(&(payload.len() as u32).to_be_bytes());
        out[0x20..0x24].copy_from_slice(&(zlib.len() as u32).to_be_bytes());
        out[0x24..0x28].copy_from_slice(b"DCP\0");
        out[0x28..0x2C].copy_from_slice(b"DFLT");
        out[0x44..0x48].copy_from_slice(b"DCA\0");
        out.extend_from_slice(&zlib);
        out
    }

    #[test]
    fn a_dflt_dcx_inflates_to_its_payload() {
        let payload = b"BND307D7R6 and the rest of an archive".repeat(20);
        assert_eq!(decompress(&dcx_of(&payload)).unwrap(), payload);
    }

    #[test]
    fn other_files_are_refused() {
        assert!(decompress(b"BND3....").is_err());
        let mut krak = dcx_of(b"x");
        krak[0x28..0x2C].copy_from_slice(b"KRAK");
        assert!(decompress(&krak).is_err());
    }

    #[test]
    fn a_size_mismatch_is_an_error() {
        let mut d = dcx_of(b"hello");
        d[0x1C..0x20].copy_from_slice(&9u32.to_be_bytes());
        assert!(decompress(&d).is_err());
    }
}
