//! DDS files (DXT1 / DXT3 / DXT5, mip 0) to RGBA8 images, crops of them, and PNG files: the
//! HUD cache's pixel work.

use std::io::BufWriter;
use std::path::Path;

use anyhow::{bail, ensure, Context, Result};
use texpresso::Format;

/// Where the pixel data starts in a DDS file without a DX10 header: magic and the 124-byte header.
const DATA_AT: usize = 128;

/// An 8-bit RGBA image, rows top to bottom, straight (not premultiplied) alpha.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

fn u32_at(b: &[u8], at: usize) -> Result<u32> {
    let s = b.get(at..at + 4).context("DDS header cut short")?;
    Ok(u32::from_le_bytes(s.try_into().expect("four bytes")))
}

/// Mip 0 of a DXT1 / DXT3 / DXT5 DDS file.
pub fn decode(dds: &[u8]) -> Result<Image> {
    ensure!(dds.get(..4) == Some(b"DDS "), "not a DDS file");
    let height = u32_at(dds, 12)? as usize;
    let width = u32_at(dds, 16)? as usize;
    let four = dds.get(84..88).context("DDS header cut short")?;
    let format = match four {
        b"DXT1" => Format::Bc1,
        b"DXT3" => Format::Bc2,
        b"DXT5" => Format::Bc3,
        other => bail!(
            "DDS format {:?} is not DXT1 / DXT3 / DXT5",
            String::from_utf8_lossy(other)
        ),
    };
    ensure!(
        width > 0 && height > 0 && width <= 8192 && height <= 8192,
        "DDS size {width} x {height}"
    );
    let size = format.compressed_size(width, height);
    let data = dds
        .get(DATA_AT..DATA_AT + size)
        .with_context(|| format!("DDS pixel data cut short ({size} bytes wanted)"))?;
    let mut rgba = vec![0u8; width * height * 4];
    format.decompress(data, width, height, &mut rgba);
    Ok(Image {
        width,
        height,
        rgba,
    })
}

impl Image {
    /// The pixels of the rectangle `left..right` x `top..bottom`, clamped to the image. A
    /// rectangle with no pixels left in the image is an error.
    pub fn crop(&self, left: i32, top: i32, right: i32, bottom: i32) -> Result<Image> {
        let l = left.clamp(0, self.width as i32) as usize;
        let r = right.clamp(0, self.width as i32) as usize;
        let t = top.clamp(0, self.height as i32) as usize;
        let b = bottom.clamp(0, self.height as i32) as usize;
        ensure!(
            r > l && b > t,
            "crop {left},{top} to {right},{bottom} has no pixels in a {} x {} image",
            self.width,
            self.height
        );
        let mut rgba = Vec::with_capacity((r - l) * (b - t) * 4);
        for y in t..b {
            let row = (y * self.width + l) * 4;
            rgba.extend_from_slice(&self.rgba[row..row + (r - l) * 4]);
        }
        Ok(Image {
            width: r - l,
            height: b - t,
            rgba,
        })
    }

    /// An additive sprite drawn on opaque black becomes a straight-alpha one: alpha is the
    /// brightest channel (times the old alpha), the colour divided by it. Alpha blending the
    /// result matches adding the original wherever the ground is dark.
    pub fn black_to_alpha(&mut self) {
        for px in self.rgba.as_chunks_mut::<4>().0.iter_mut() {
            let m = px[0].max(px[1]).max(px[2]) as u32;
            if m == 0 {
                px.copy_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + m / 2) / m).min(255) as u8;
            }
            px[3] = ((m * px[3] as u32 + 127) / 255) as u8;
        }
    }

    pub fn write_png(&self, path: &Path) -> Result<()> {
        let file =
            std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
        let mut enc =
            png::Encoder::new(BufWriter::new(file), self.width as u32, self.height as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header()?;
        w.write_image_data(&self.rgba)?;
        w.finish()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A DDS header (DX10-less) for `four`, then `blocks`.
    fn dds_of(four: &[u8; 4], width: u32, height: u32, blocks: &[u8]) -> Vec<u8> {
        let mut h = vec![0u8; DATA_AT];
        h[0..4].copy_from_slice(b"DDS ");
        h[4..8].copy_from_slice(&124u32.to_le_bytes());
        h[12..16].copy_from_slice(&height.to_le_bytes());
        h[16..20].copy_from_slice(&width.to_le_bytes());
        h[76..80].copy_from_slice(&32u32.to_le_bytes());
        h[84..88].copy_from_slice(four);
        h.extend_from_slice(blocks);
        h
    }

    /// A colour block: red (565 `F800`), blue (`001F`), row 0 blue, rows 1-3 red.
    const RED_BLUE: [u8; 8] = [0x00, 0xF8, 0x1F, 0x00, 0b0101_0101, 0, 0, 0];

    fn px(img: &Image, x: usize, y: usize) -> [u8; 4] {
        let i = (y * img.width + x) * 4;
        img.rgba[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn a_bc1_block_decodes_to_its_two_colours() {
        let img = decode(&dds_of(b"DXT1", 4, 4, &RED_BLUE)).unwrap();
        assert_eq!((img.width, img.height), (4, 4));
        assert_eq!(px(&img, 0, 0), [0, 0, 255, 255]);
        assert_eq!(px(&img, 3, 0), [0, 0, 255, 255]);
        assert_eq!(px(&img, 0, 1), [255, 0, 0, 255]);
        assert_eq!(px(&img, 3, 3), [255, 0, 0, 255]);
    }

    #[test]
    fn a_bc3_block_keeps_alpha() {
        // Alpha endpoints 200 and 100, every index 0 (= the first endpoint), then the colour block.
        let mut block = vec![200, 100, 0, 0, 0, 0, 0, 0];
        block.extend_from_slice(&RED_BLUE);
        let img = decode(&dds_of(b"DXT5", 4, 4, &block)).unwrap();
        assert_eq!(px(&img, 0, 1), [255, 0, 0, 200]);
        assert_eq!(px(&img, 2, 0), [0, 0, 255, 200]);
    }

    #[test]
    fn a_bc2_block_has_explicit_alpha_nibbles() {
        let mut block = vec![0xFF; 8];
        block.extend_from_slice(&RED_BLUE);
        let img = decode(&dds_of(b"DXT3", 4, 4, &block)).unwrap();
        assert_eq!(px(&img, 1, 1)[3], 255);
    }

    #[test]
    fn unsupported_or_cut_files_are_errors() {
        assert!(decode(b"not a dds at all, not even close, no").is_err());
        assert!(decode(&dds_of(b"ATI2", 4, 4, &RED_BLUE)).is_err());
        assert!(decode(&dds_of(b"DXT1", 4, 4, &RED_BLUE[..5])).is_err());
        assert!(decode(&dds_of(b"DXT1", 0, 4, &RED_BLUE)).is_err());
        assert!(decode(&dds_of(b"DXT1", 4, 4, &RED_BLUE)[..60]).is_err());
    }

    fn numbered(w: usize, h: usize) -> Image {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&[x as u8, y as u8, 0, 255]);
            }
        }
        Image {
            width: w,
            height: h,
            rgba,
        }
    }

    #[test]
    fn crop_clamps_to_the_image() {
        let img = numbered(4, 4);
        let c = img.crop(2, 2, 10, 10).unwrap();
        assert_eq!((c.width, c.height), (2, 2));
        assert_eq!(px(&c, 0, 0), [2, 2, 0, 255]);
        assert_eq!(px(&c, 1, 1), [3, 3, 0, 255]);
        let c = img.crop(-3, -3, 2, 2).unwrap();
        assert_eq!((c.width, c.height), (2, 2));
        assert_eq!(px(&c, 0, 0), [0, 0, 0, 255]);
        let whole = img.crop(0, 0, 4, 4).unwrap();
        assert_eq!(whole, img);
    }

    #[test]
    fn an_empty_crop_is_an_error() {
        let img = numbered(4, 4);
        assert!(img.crop(5, 5, 8, 8).is_err());
        assert!(img.crop(2, 2, 2, 4).is_err());
        assert!(img.crop(3, 1, 1, 4).is_err());
    }

    #[test]
    fn black_background_becomes_alpha() {
        let mut img = Image {
            width: 4,
            height: 1,
            rgba: vec![
                255, 0, 0, 255, // pure red
                128, 0, 0, 255, // half red
                0, 0, 0, 255, // black
                100, 200, 50, 128, // dim, half-transparent
            ],
        };
        img.black_to_alpha();
        assert_eq!(px(&img, 0, 0), [255, 0, 0, 255]);
        assert_eq!(px(&img, 1, 0), [255, 0, 0, 128]);
        assert_eq!(px(&img, 2, 0), [0, 0, 0, 0]);
        assert_eq!(px(&img, 3, 0), [128, 255, 64, 100]);
    }

    #[test]
    fn png_round_trips() {
        let img = numbered(5, 3);
        let path =
            std::env::temp_dir().join(format!("wow-souls-dds-test-{}.png", std::process::id()));
        img.write_png(&path).unwrap();
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (5, 3));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(&buf[..info.buffer_size()], &img.rgba[..]);
        std::fs::remove_file(&path).ok();
    }
}
