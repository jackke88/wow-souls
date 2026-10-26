//! Embedded material pieces; the game owns layout, text, selection and resource values.
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::{BorderRect, TextureSlicer};
use bevy::ui::widget::NodeImageMode;

const ICONS: &[u8] = include_bytes!("../../../assets/ui/etched-icons-v1.png");
const TRIM: &[u8] = include_bytes!("../../../assets/ui/weathered-trim-v1.png");

#[derive(Component, Clone, Copy, PartialEq)]
pub(crate) enum Surface {
    Panel,
    Slot(bool),
    Icon(usize),
    Bar,
    Fill(Color),
    Seal,
}

#[derive(Resource)]
struct Skins {
    icons: Handle<Image>,
    trim: Handle<Image>,
}

fn decode(bytes: &[u8]) -> Result<Image, String> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("UI sheet too large")?];
    let info = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    if (info.width, info.height, info.color_type, info.bit_depth)
        != (1536, 1024, png::ColorType::Rgba, png::BitDepth::Eight)
    {
        return Err("UI sheet must be 1536x1024 RGBA8".into());
    }
    pixels.truncate(info.buffer_size());
    let mut image = Image::new(
        Extent3d {
            width: info.width,
            height: info.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    Ok(image)
}

fn load(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    if !super::enabled() {
        return;
    }
    match decode(ICONS).and_then(|icons| decode(TRIM).map(|trim| (icons, trim))) {
        Ok((icons, trim)) => {
            commands.insert_resource(Skins {
                icons: images.add(icons),
                trim: images.add(trim),
            });
        }
        Err(error) => warn!("UI material atlas: {error}; keeping native shapes"),
    }
}

fn tile(index: usize) -> Rect {
    let x = [63.0, 430.0, 795.0, 1160.0][index % 4];
    let y = if index < 4 { 148.0 } else { 568.0 };
    Rect::new(x, y, x + 314.0, y + 309.0)
}

fn bind(
    mut commands: Commands,
    skins: Option<Res<Skins>>,
    surfaces: Query<(Entity, &Surface), Changed<Surface>>,
) {
    let Some(skins) = skins else { return };
    for (entity, surface) in &surfaces {
        let (handle, rect, border, color) = match *surface {
            Surface::Icon(index) => (&skins.icons, tile(index.min(5)), None, Color::WHITE),
            Surface::Slot(selected) => (
                &skins.icons,
                tile(if selected { 7 } else { 6 }),
                Some(14.0),
                Color::WHITE,
            ),
            Surface::Panel => (
                &skins.trim,
                Rect::new(427.0, 469.0, 1484.0, 914.0),
                None,
                Color::srgba(0.88, 0.89, 0.83, 1.0),
            ),
            Surface::Bar => (
                &skins.trim,
                Rect::new(35.0, 176.0, 1503.0, 231.0),
                Some(42.0),
                Color::WHITE,
            ),
            Surface::Fill(color) => (
                &skins.trim,
                Rect::new(72.0, 334.0, 1466.0, 369.0),
                None,
                color,
            ),
            Surface::Seal => (
                &skins.trim,
                Rect::new(65.0, 526.0, 388.0, 867.0),
                None,
                Color::WHITE,
            ),
        };
        let mut image = ImageNode {
            image: handle.clone(),
            rect: Some(rect),
            color,
            image_mode: NodeImageMode::Stretch,
            ..default()
        };
        if let Some(border) = border {
            let border = if matches!(surface, Surface::Bar) {
                BorderRect {
                    min_inset: Vec2::new(52.0, 8.0),
                    max_inset: Vec2::new(52.0, 8.0),
                }
            } else {
                BorderRect::all(border)
            };
            image.image_mode = NodeImageMode::Sliced(TextureSlicer {
                border,
                ..default()
            });
        }
        let mut target = commands.entity(entity);
        target.insert(image);
        if matches!(surface, Surface::Panel) {
            target.remove::<BackgroundColor>();
            target.insert(BackgroundGradient(vec![Gradient::Linear(
                LinearGradient::new(
                    LinearGradient::TO_RIGHT,
                    vec![
                        ColorStop::percent(Color::NONE, 0.0),
                        ColorStop::percent(Color::srgba(0.02, 0.025, 0.02, 0.58), 6.0),
                        ColorStop::percent(Color::srgba(0.02, 0.025, 0.02, 0.58), 91.0),
                        ColorStop::percent(Color::NONE, 100.0),
                    ],
                ),
            )]));
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, load)
        .add_systems(Update, bind.run_if(super::enabled));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn material_sheets_decode_and_keep_the_background_transparent() {
        for bytes in [ICONS, TRIM] {
            let image = decode(bytes).unwrap();
            let pixels = image.data.unwrap();
            for (x, y) in [(0, 0), (768, 10), (1500, 1000)] {
                assert_eq!(pixels[(y * 1536 + x) * 4 + 3], 0);
            }
            assert!(pixels.iter().skip(3).step_by(4).any(|a| *a > 240));
        }
    }
}
