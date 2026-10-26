//! Assemble installed map tiles in memory; geography, lettering and overlay alignment stay original.
use anyhow::{Context, Result};
use benilla_formats::{Chain, WorldMapArea};
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub const WIDTH: u32 = 1002;
pub const HEIGHT: u32 = 668;

pub struct Chart {
    pub area: WorldMapArea,
    pub name: String,
    pub pixels: Vec<u8>,
}

/// AreaTable id, rather than the unrelated WorldMapArea row id. Elwynn is area 12 / map row 30.
pub fn read(chain: &mut Chain, area_id: u32) -> Result<Chart> {
    let maps = benilla_formats::load_world_map_area_catalog(chain)?;
    let (id, area) = maps
        .iter()
        .find(|(_, a)| a.area_id == area_id)
        .with_context(|| format!("No area map for area {area_id}"))?;
    let areas = benilla_formats::load_area_table_catalog(chain)?;
    let name = areas.name(area_id).unwrap_or(&area.name).to_string();
    let directory = format!("Interface\\WorldMap\\{}\\", area.name);
    let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];
    for tile in 0..12 {
        let bytes = chain.read_file(&format!("{directory}{}{n}.blp", area.name, n = tile + 1))?;
        let (w, h, rgba) = benilla_formats::blp_to_rgba(&bytes)?;
        stamp(
            &mut pixels,
            &rgba,
            UVec2::new(w, h),
            UVec2::new(tile % 4 * 256, tile / 4 * 256),
            UVec2::new(256, 256),
        );
    }
    let overlays = benilla_formats::load_world_map_overlay_catalog(chain)?;
    for overlay in overlays.for_area(id) {
        if overlay.texture_name.is_empty() {
            continue;
        }
        let columns = overlay.texture_width.div_ceil(256);
        let rows = overlay.texture_height.div_ceil(256);
        // Corrupt dimensions must not turn one map into an unbounded allocation/read loop.
        anyhow::ensure!(columns <= 8 && rows <= 8, "Oversized map overlay");
        for y in 0..rows {
            for x in 0..columns {
                let tile = y * columns + x + 1;
                let bytes =
                    chain.read_file(&format!("{directory}{}{tile}.blp", overlay.texture_name))?;
                let (w, h, rgba) = benilla_formats::blp_to_rgba(&bytes)?;
                stamp(
                    &mut pixels,
                    &rgba,
                    UVec2::new(w, h),
                    UVec2::new(overlay.offset_x + x * 256, overlay.offset_y + y * 256),
                    UVec2::new(
                        (overlay.texture_width - x * 256).min(256),
                        (overlay.texture_height - y * 256).min(256),
                    ),
                );
            }
        }
    }
    // One shared ink/paper treatment for every zone; no region-specific generated artwork.
    for (i, p) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let light = p[0] as f32 * 0.2126 + p[1] as f32 * 0.7152 + p[2] as f32 * 0.0722;
        let grain = ((i as u32).wrapping_mul(747_796_405).rotate_left(13) % 7) as f32 - 3.0;
        for (c, gain) in [0.98, 0.92, 0.77].into_iter().enumerate() {
            p[c] =
                ((light * 0.72 + p[c] as f32 * 0.28) * gain + grain * 0.24).clamp(0.0, 255.0) as u8;
        }
        p[3] = 255;
    }
    Ok(Chart {
        area: area.clone(),
        name,
        pixels,
    })
}

fn stamp(dst: &mut [u8], src: &[u8], size: UVec2, at: UVec2, valid: UVec2) {
    for y in 0..size.y.min(valid.y).min(HEIGHT.saturating_sub(at.y)) {
        for x in 0..size.x.min(valid.x).min(WIDTH.saturating_sub(at.x)) {
            let s = ((y * size.x + x) * 4) as usize;
            let d = (((at.y + y) * WIDTH + at.x + x) * 4) as usize;
            let a = src[s + 3] as u32;
            for c in 0..3 {
                dst[d + c] =
                    ((src[s + c] as u32 * a + dst[d + c] as u32 * (255 - a) + 127) / 255) as u8;
            }
            dst[d + 3] = 255;
        }
    }
}

pub fn image(pixels: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    image
}

/// Faceted brass/gold cartography beacon pin with an obsidian enamel core and four-point star.
/// The sharp lower point sits near the bottom center so both map and compass bearings read cleanly.
pub(crate) fn marker_image() -> Image {
    const W: u32 = 96;
    const H: u32 = 120;
    let body_w = |y: f32| -> f32 {
        if (-0.66..=-0.20).contains(&y) {
            0.46 * ((y + 0.66) / 0.46).powf(0.92)
        } else if (-0.20..=0.86).contains(&y) {
            0.46 * (1.0 - (y + 0.20) / 1.06).powf(0.84) + 0.015
        } else {
            0.0
        }
    };
    let sdf_outer = |x: f32, y: f32| -> f32 {
        let d_body = if y < -0.66 {
            Vec2::new(x, y + 0.66).length()
        } else if y > 0.86 {
            Vec2::new(x, y - 0.86).length()
        } else {
            (x.abs() - body_w(y)) * 0.78
        };
        let d_finial = (x.abs() * 1.15 + (y + 0.74).abs() * 0.95 - 0.135) * 0.75;
        let d_spurs = (x.abs() * 0.82 + (y + 0.20).abs() * 1.75 - 0.465) * 0.72;
        d_body.min(d_finial).min(d_spurs)
    };
    let mut pixels = vec![0u8; (W * H * 4) as usize];
    for py in 0..H {
        for px in 0..W {
            let hash = px.wrapping_mul(374_761_393) ^ py.wrapping_mul(668_265_263);
            let grain =
                (((hash ^ (hash >> 13)).wrapping_mul(1_274_126_177) >> 24) as f32 / 255.0) - 0.5;
            let mut acc = [0.0f32; 4];
            for sy in 0..3 {
                for sx in 0..3 {
                    let u = (px as f32 + (sx as f32 + 0.5) / 3.0) / W as f32;
                    let v = (py as f32 + (sy as f32 + 0.5) / 3.0) / H as f32;
                    let x = (u * 2.0 - 1.0) * 0.8;
                    let y = v * 2.0 - 1.0;
                    let d = sdf_outer(x, y);
                    let d_shadow = sdf_outer(x - 0.022, y - 0.032);
                    let shadow_a = ((0.055 - d_shadow) / 0.075).clamp(0.0, 1.0) * 0.68;
                    let shape_a = ((0.012 - d) / 0.024).clamp(0.0, 1.0);
                    if shape_a <= 0.0 && shadow_a <= 0.0 {
                        continue;
                    }
                    let w_in = (body_w(y) - 0.105).max(0.0);
                    let d_enamel = if (-0.51..=0.42).contains(&y) && w_in > 0.01 {
                        (x.abs() - w_in) * 0.82
                    } else {
                        1.0
                    };
                    let cy = y + 0.18;
                    let r = Vec2::new(x, cy).length();
                    let d_ring = (r - 0.215).abs() - 0.013;
                    let d_star =
                        ((x.abs() / 0.27).powf(0.58) + (cy.abs() / 0.32).powf(0.58) - 1.0) * 0.16;
                    let d_stem = if (-0.18..=0.45).contains(&y) {
                        x.abs() - 0.014
                    } else {
                        1.0
                    };
                    let d_emblem = d_ring.min(d_star).min(d_stem);

                    // Base metal shading: upper-left light + faceted central spine on the lower spear.
                    let facet = if y > 0.24 {
                        if x < 0.0 {
                            0.14
                        } else {
                            -0.14
                        }
                    } else {
                        -x * 0.22
                    };
                    let spine = if y > 0.22 && x.abs() < 0.020 {
                        0.18 * (1.0 - x.abs() / 0.020)
                    } else {
                        0.0
                    };
                    let rim_hi = ((-0.018 - d).abs() < 0.016) as u8 as f32 * 0.10;
                    let shade =
                        (0.76 - y * 0.20 + facet + spine + rim_hi + grain * 0.06).clamp(0.42, 1.18);
                    let mut rgb = [
                        (212.0 * shade).clamp(0.0, 255.0),
                        (182.0 * shade).clamp(0.0, 255.0),
                        (110.0 * shade).clamp(0.0, 255.0),
                    ];

                    // Dark enamel window with warm radial glow behind the star.
                    let enamel_t = ((-d_enamel) / 0.022).clamp(0.0, 1.0);
                    if enamel_t > 0.0 {
                        let glow = (1.0 - (r / 0.34).clamp(0.0, 1.0)).powi(2) * 0.35;
                        let enamel = [
                            20.0 + 95.0 * glow + grain * 6.0,
                            22.0 + 80.0 * glow + grain * 6.0,
                            17.0 + 42.0 * glow + grain * 5.0,
                        ];
                        for c in 0..3 {
                            rgb[c] = rgb[c] * (1.0 - enamel_t) + enamel[c] * enamel_t;
                        }
                    }

                    // Luminous ivory-gold star, astrolabe ring and vertical needle inside the window.
                    let emb_t = ((-d_emblem) / 0.020).clamp(0.0, 1.0) * enamel_t;
                    if emb_t > 0.0 {
                        let core = (1.0 - (r / 0.26).clamp(0.0, 1.0)) * 0.22;
                        let star_rgb = [
                            (236.0 + 38.0 * core).min(255.0),
                            (218.0 + 42.0 * core).min(255.0),
                            (158.0 + 55.0 * core).min(255.0),
                        ];
                        for c in 0..3 {
                            rgb[c] = rgb[c] * (1.0 - emb_t) + star_rgb[c] * emb_t;
                        }
                    }

                    // Small dark-etched eyelet at the heart of the star.
                    let eye_t = ((0.036 - r) / 0.015).clamp(0.0, 1.0) * enamel_t;
                    if eye_t > 0.0 {
                        for c in &mut rgb {
                            *c = *c * (1.0 - eye_t) + 24.0 * eye_t;
                        }
                    }

                    // Crisp dark outer contour for legibility over pale parchment.
                    let border_t = ((d + 0.024) / 0.024).clamp(0.0, 1.0);
                    for c in &mut rgb {
                        *c = *c * (1.0 - border_t * 0.78) + 14.0 * (border_t * 0.78);
                    }

                    let out_a = shape_a + shadow_a * (1.0 - shape_a);
                    if out_a > 0.0 {
                        let shadow_rgb = 10.0;
                        for c in 0..3 {
                            let ch = (rgb[c] * shape_a + shadow_rgb * shadow_a * (1.0 - shape_a))
                                / out_a;
                            acc[c] += ch * out_a;
                        }
                        acc[3] += out_a;
                    }
                }
            }
            let idx = ((py * W + px) * 4) as usize;
            let a = (acc[3] / 9.0).clamp(0.0, 1.0);
            if a > 1e-4 {
                for c in 0..3 {
                    pixels[idx + c] = (acc[c] / acc[3]).round().clamp(0.0, 255.0) as u8;
                }
                pixels[idx + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::linear();
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marker_pin_texture_has_transparent_corners_and_gilded_silhouette() {
        let icon = marker_image();
        assert_eq!((icon.width(), icon.height()), (96, 120));
        let data = icon.data.as_ref().unwrap();
        assert_eq!(data[3], 0, "top-left corner stays transparent");
        let star_arm = ((48 * 96 + 44) * 4) as usize;
        assert!(data[star_arm + 3] > 240, "medallion interior is solid");
        assert!(data[star_arm] > 180, "star arm is warm gold/ivory");
        let tip = ((110 * 96 + 48) * 4) as usize;
        assert!(data[tip + 3] > 200, "downward pointer tip is present");
    }
    #[test]
    fn partial_overlay_clips_to_its_authored_size_and_the_chart_edge() {
        let mut dst = vec![0; (WIDTH * HEIGHT * 4) as usize];
        let src = [200, 100, 50, 128].repeat(16);
        stamp(
            &mut dst,
            &src,
            UVec2::splat(4),
            UVec2::new(WIDTH - 2, HEIGHT - 2),
            UVec2::new(1, 4),
        );
        let d = ((HEIGHT - 1) * WIDTH + WIDTH - 2) as usize * 4;
        assert_eq!(&dst[d..d + 4], &[100, 50, 25, 255]);
        assert_eq!(
            dst[d + 7],
            0,
            "padded last-tile pixels do not paint the neighboring region"
        );
    }
    #[test]
    fn installed_elwynn_tiles_and_overlays_make_a_complete_chart() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let chart = read(&mut chain, 12).unwrap();
        assert_eq!(chart.name, "Elwynn Forest");
        assert_eq!(chart.area.map_id, 0);
        assert_eq!(chart.pixels.len(), (WIDTH * HEIGHT * 4) as usize);
        assert!(chart.pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
        assert!(chart.pixels.as_chunks::<4>().0.iter().any(|p| p[0] > 150));
    }
}
