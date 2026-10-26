//! YOU DIED, Dark Souls' own banner: the scene dims, a dark band with soft edges crosses the
//! middle of the screen and the red text comes up in it, then the whole screen goes black before
//! the respawn. The pieces, their places and their colours are the layout's `F23_YouDied`; the
//! timing is ours (the layout's animation is not read yet).
//!
//! Dark Souls blends in gamma space and Bevy in linear, so a black overlay's alpha is raised to
//! what a gamma blend would show ([`reference_alpha`]).

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::time::Real;

use super::art::{HudArt, Rgba};
use super::{node_at, sprite_tint, Anchor};
use crate::hurt::Death;
use crate::settle::Phase;

/// The opacity (0..=1) of each layer at a moment of the banner.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Alphas {
    pub dim: f32,
    pub band: f32,
    pub text: f32,
    pub black: f32,
}

fn smooth(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}

/// The layers `t` seconds after the death: the dim rises over 1.2 s, the band over 0.8 s, the text
/// from 0.2 s over 1.2 s; from 3.4 s the screen goes black over a second (the respawn is at 4.5).
pub fn phase(t: f32) -> Alphas {
    let ramp = |from: f32, len: f32| smooth(((t - from) / len).clamp(0.0, 1.0));
    Alphas {
        dim: ramp(0.0, 1.2),
        band: ramp(0.0, 0.8),
        text: ramp(0.2, 1.2),
        black: ramp(3.4, 1.0),
    }
}

/// The alpha a linear blend needs so that a black overlay darkens as much as a gamma-space blend
/// of alpha `a` does: `1 - (1 - a)^2.2`.
pub fn reference_alpha(a: f32) -> f32 {
    1.0 - (1.0 - a.clamp(0.0, 1.0)).powf(2.2)
}

/// A 1 x `rows` straight-alpha gradient from the `top` colour to the `bottom` one, as RGBA bytes.
pub fn gradient_pixels(top: Rgba, bottom: Rgba, rows: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(rows * 4);
    for y in 0..rows {
        let k = if rows > 1 {
            y as f32 / (rows - 1) as f32
        } else {
            0.0
        };
        for c in 0..4 {
            let mut v = top[c] + (bottom[c] - top[c]) * k;
            if c == 3 {
                v = reference_alpha(v);
            }
            out.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    out
}

#[derive(Component)]
struct BannerRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Layer {
    Dim,
    Band,
    EdgeAbove,
    EdgeBelow,
    Text,
    Black,
}

pub fn plugin(app: &mut App) {
    let art = resource_exists::<HudArt>.or(super::theme::enabled);
    app.add_systems(OnEnter(Phase::Playing), spawn.run_if(art.clone()))
        .add_systems(Update, update.run_if(in_state(Phase::Playing)).run_if(art));
}

fn gradient_image(top: Rgba, bottom: Rgba) -> Image {
    const ROWS: usize = 32;
    let mut image = Image::new(
        Extent3d {
            width: 1,
            height: ROWS as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        gradient_pixels(top, bottom, ROWS),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// A full-width strip at the layout's rows.
fn strip(top: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(0),
        width: percent(100),
        top: px(top),
        height: px(height),
        ..default()
    }
}

fn spawn(
    mut commands: Commands,
    art: Option<Res<HudArt>>,
    mut images: ResMut<Assets<Image>>,
    fonts: Res<crate::loot::ui::Fonts>,
) {
    if super::theme::enabled() {
        let root = commands
            .spawn((
                BannerRoot,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                GlobalZIndex(60),
                Visibility::Hidden,
            ))
            .id();
        let full = || Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        };
        commands.spawn((
            full(),
            BackgroundColor(Color::NONE),
            Layer::Dim,
            ChildOf(root),
        ));
        commands.spawn((
            strip(280.0, 135.0),
            BackgroundColor(Color::NONE),
            Layer::Band,
            ChildOf(root),
        ));
        let text = super::theme::words(
            &mut commands,
            root,
            "YOU DIED",
            &fonts.title,
            62.0,
            Node {
                position_type: PositionType::Absolute,
                top: px(300),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
        );
        commands
            .entity(text)
            .insert((Layer::Text, TextLayout::new_with_justify(Justify::Center)));
        commands.spawn((
            full(),
            BackgroundColor(Color::NONE),
            Layer::Black,
            ChildOf(root),
        ));
        return;
    }
    let Some(art) = art else { return };
    let m = &art.m;
    let root = commands
        .spawn((
            Name::new("hud you died"),
            BannerRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(60),
            Visibility::Hidden,
        ))
        .id();
    let full = Node {
        position_type: PositionType::Absolute,
        left: px(0),
        top: px(0),
        width: percent(100),
        height: percent(100),
        ..default()
    };
    commands.spawn((
        full.clone(),
        BackgroundColor(Color::NONE),
        Layer::Dim,
        ChildOf(root),
    ));
    for (key, layer) in [
        ("F23_YouDied/BG_grad00", Layer::EdgeAbove),
        ("F23_YouDied/BG_grad01", Layer::EdgeBelow),
    ] {
        let g = m.gradient(key);
        let handle = images.add(gradient_image(g.colors[0], g.colors[2]));
        commands.spawn((
            strip(g.rect.t, g.rect.h()),
            ImageNode::new(handle).with_color(Color::srgba(1.0, 1.0, 1.0, 0.0)),
            layer,
            ChildOf(root),
        ));
    }
    let band = m.fill("F23_YouDied/BG_bar");
    commands.spawn((
        strip(band.rect.t, band.rect.h()),
        BackgroundColor(Color::NONE),
        Layer::Band,
        ChildOf(root),
    ));
    let text = m.sprite("F23_YouDied/main_text_youdied");
    commands.spawn((
        node_at(text.rect, Anchor::Center),
        ImageNode::new(art.sprite_image("F23_YouDied/main_text_youdied"))
            .with_color(Color::srgba(1.0, 1.0, 1.0, 0.0)),
        Layer::Text,
        ChildOf(root),
    ));
    commands.spawn((
        full,
        BackgroundColor(Color::NONE),
        Layer::Black,
        ChildOf(root),
    ));
}

fn update(
    real: Res<Time<Real>>,
    death: Res<Death>,
    art: Option<Res<HudArt>>,
    mut root: Query<&mut Visibility, With<BannerRoot>>,
    mut layers: Query<(
        &Layer,
        Option<&mut BackgroundColor>,
        Option<&mut ImageNode>,
        Option<&mut TextColor>,
    )>,
) {
    let Ok(mut vis) = root.single_mut() else {
        return;
    };
    let Some(at) = death.at else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    };
    if *vis != Visibility::Inherited {
        *vis = Visibility::Inherited;
    }
    let a = phase(real.elapsed_secs() - at);
    if super::theme::enabled() {
        for (layer, bg, _, text) in &mut layers {
            if let Some(mut bg) = bg {
                let alpha = match layer {
                    Layer::Dim => a.dim * 0.32,
                    Layer::Band => a.band * 0.65,
                    Layer::Black => a.black,
                    _ => 0.0,
                };
                bg.0 = Color::srgba(0.0, 0.0, 0.0, alpha);
            }
            if let Some(mut text) = text {
                text.0 = Color::srgba(0.49, 0.045, 0.035, a.text);
            }
        }
        return;
    }
    let Some(art) = art else { return };
    let m = &art.m;
    let dim = reference_alpha(m.fill("F23_YouDied/BG").color[3]) * a.dim;
    let band = reference_alpha(m.fill("F23_YouDied/BG_bar").color[3]) * a.band;
    let text_tint = m.sprite("F23_YouDied/main_text_youdied").tint;
    for (layer, bg, image, _) in &mut layers {
        match (layer, bg, image) {
            (Layer::Dim, Some(mut bg), _) => bg.0 = Color::srgba(0.0, 0.0, 0.0, dim),
            (Layer::Band, Some(mut bg), _) => bg.0 = Color::srgba(0.0, 0.0, 0.0, band),
            (Layer::Black, Some(mut bg), _) => bg.0 = Color::srgba(0.0, 0.0, 0.0, a.black),
            (Layer::EdgeAbove | Layer::EdgeBelow, _, Some(mut img)) => {
                img.color = Color::srgba(1.0, 1.0, 1.0, a.band);
            }
            (Layer::Text, _, Some(mut img)) => {
                let c = sprite_tint(text_tint).to_srgba();
                img.color = Color::srgba(c.red, c.green, c.blue, c.alpha * a.text);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_has_one_text_without_legacy_hud_cache() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .insert_resource(crate::loot::ui::Fonts {
                body: Handle::default(),
                title: Handle::default(),
            });
        crate::hurt::plugin(&mut app);
        plugin(&mut app);
        app.world_mut().run_schedule(OnEnter(Phase::Playing));

        let mut texts = app.world_mut().query::<&Text>();
        assert_eq!(
            texts
                .iter(app.world())
                .filter(|text| text.0 == "YOU DIED")
                .count(),
            1,
            "the banner and its plain fallback must never overlap"
        );
    }

    #[test]
    fn nothing_shows_before_the_death() {
        assert_eq!(phase(-1.0), Alphas::default());
        assert_eq!(phase(0.0).band, 0.0);
        assert_eq!(phase(0.0).text, 0.0);
    }

    #[test]
    fn the_text_arrives_after_the_band() {
        let a = phase(0.5);
        assert!(a.band > a.text, "{a:?}");
        let b = phase(1.0);
        assert!(b.band == 1.0 && b.text > 0.5 && b.text < 1.0, "{b:?}");
        assert_eq!(phase(1.5).text, 1.0);
        assert_eq!(phase(2.0).text, 1.0);
        assert_eq!(phase(2.0).dim, 1.0);
    }

    #[test]
    fn the_layers_only_ever_rise() {
        let mut last = Alphas::default();
        for i in 0..=480 {
            let a = phase(i as f32 / 100.0);
            assert!(
                a.dim >= last.dim
                    && a.band >= last.band
                    && a.text >= last.text
                    && a.black >= last.black
            );
            last = a;
        }
    }

    #[test]
    fn black_covers_the_screen_before_the_respawn() {
        assert_eq!(phase(3.3).black, 0.0);
        assert!(phase(3.9).black > 0.0 && phase(3.9).black < 1.0);
        assert_eq!(phase(crate::hurt::RESPAWN_AFTER).black, 1.0);
    }

    #[test]
    fn black_overlays_blend_like_dark_souls() {
        assert_eq!(reference_alpha(0.0), 0.0);
        assert_eq!(reference_alpha(1.0), 1.0);
        // The band's 200 / 255: a gamma blend leaves 22 % of the scene, a linear one needs 96 %.
        assert!((reference_alpha(200.0 / 255.0) - 0.964).abs() < 0.005);
        assert!(reference_alpha(0.5) > 0.5);
    }

    #[test]
    fn a_gradient_runs_from_one_colour_to_the_other() {
        let px = gradient_pixels([0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 200.0 / 255.0], 5);
        assert_eq!(px.len(), 20);
        assert_eq!(&px[0..4], &[0, 0, 0, 0]);
        assert_eq!(
            px[19],
            (reference_alpha(200.0 / 255.0) * 255.0).round() as u8
        );
        assert!(px[7] < px[11] && px[11] < px[15], "{px:?}");
    }
}
