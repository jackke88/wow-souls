//! Dark Souls' bitmap font: text laid out from the glyph table of the HUD cache, drawn as one
//! image node per glyph (a rectangle of a font page). The layout is pure ([`layout`]); a
//! [`BitmapText`] component on a node makes its children.
//!
//! The metrics are the font file's own: a glyph's bitmap is drawn at `pen + pre`, then the pen
//! moves by `advance`; every glyph's cell is `font.size` atlas pixels tall and the line is drawn
//! `size` layout pixels tall, so everything scales by `size / font.size`.

use bevy::prelude::*;

use super::art::{FontDef, HudArt, Rgba};
use super::tint;

/// The project's bundled OFL font; UI text needs no operating-system font files.
pub(crate) fn project_font(fonts: &mut Assets<Font>) -> Handle<Font> {
    fonts.add(
        Font::try_from_bytes(
            include_bytes!("../../assets/fonts/eb-garamond/EBGaramond.ttf").to_vec(),
        )
        .expect("bundled OFL EB Garamond is a valid font"),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Right,
    /// Unused by the HUD's own fields (they are left or right aligned); laid out in the tests.
    #[allow(dead_code)]
    Center,
}

/// One glyph to draw: its place in the line (layout px, from the field's left and top), its size
/// and the rectangle of its page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// The rectangle in the page's pixels: left, top, right, bottom.
    pub uv: [f32; 4],
    pub page: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Layout {
    pub quads: Vec<Quad>,
    /// The pen's travel: the text's width in layout px.
    pub width: f32,
}

/// Lays `text` out at `size` layout pixels (the line's height) in a field `field_width` wide: the
/// quads are positioned from the field's left edge, shifted for `align`. A character the font
/// lacks is skipped without advancing; a space (or a glyph with no bitmap) only advances.
pub fn layout(font: &FontDef, text: &str, size: f32, align: Align, field_width: f32) -> Layout {
    if font.size <= 0.0 {
        return Layout::default();
    }
    let scale = size / font.size;
    let mut pen = 0.0;
    let mut quads = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let Some(g) = font.glyphs.get(&(ch as u32)) else {
            continue;
        };
        let (w, h) = (g.uv[2] - g.uv[0], g.uv[3] - g.uv[1]);
        if !ch.is_whitespace() && g.width > 0.0 && w > 0.0 && h > 0.0 {
            quads.push(Quad {
                x: pen + g.pre * scale,
                y: 0.0,
                w: w * scale,
                h: h * scale,
                uv: g.uv,
                page: g.page,
            });
        }
        pen += g.advance * scale;
    }
    let shift = match align {
        Align::Left => 0.0,
        Align::Right => field_width - pen,
        Align::Center => (field_width - pen) * 0.5,
    };
    for q in &mut quads {
        q.x += shift;
    }
    Layout { quads, width: pen }
}

/// A line of text in the Dark Souls font on a node: its children are the glyphs, rebuilt when this
/// changes. Give the node the field's rectangle (`field_width` is its width).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct BitmapText {
    pub text: String,
    /// The line height in layout px.
    pub size: f32,
    pub align: Align,
    pub field_width: f32,
    pub color: Rgba,
    /// A fade on top of the colour's own alpha.
    pub alpha: f32,
}

impl BitmapText {
    pub fn new(
        text: impl Into<String>,
        size: f32,
        align: Align,
        field_width: f32,
        color: Rgba,
    ) -> Self {
        Self {
            text: text.into(),
            size,
            align,
            field_width,
            color,
            alpha: 1.0,
        }
    }
}

/// Marks a glyph node so a rebuild removes only these.
#[derive(Component)]
struct GlyphNode;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, rebuild.run_if(resource_exists::<HudArt>));
}

/// Rebuilds the glyph children of every text whose [`BitmapText`] changed.
fn rebuild(
    mut commands: Commands,
    art: Res<HudArt>,
    texts: Query<(Entity, &BitmapText, Option<&Children>), Changed<BitmapText>>,
    glyphs: Query<(), With<GlyphNode>>,
) {
    for (e, t, children) in &texts {
        for c in children.into_iter().flatten().copied() {
            if glyphs.contains(c) {
                commands.entity(c).despawn();
            }
        }
        let l = layout(&art.m.font, &t.text, t.size, t.align, t.field_width);
        let mut color = t.color;
        color[3] *= t.alpha;
        for q in &l.quads {
            let Some(page) = art.m.font.pages.get(&q.page) else {
                continue;
            };
            commands.spawn((
                GlyphNode,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(q.x),
                    top: px(q.y),
                    width: px(q.w),
                    height: px(q.h),
                    ..default()
                },
                ImageNode {
                    image: art.handle(page),
                    color: tint(color),
                    rect: Some(Rect::new(q.uv[0], q.uv[1], q.uv[2], q.uv[3])),
                    ..default()
                },
                ChildOf(e),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::hud::art::GlyphDef;

    /// A font of size 58: `0` and `1` as in DSFont24 (pre 3 / 7, advance 27 / 23), a space that
    /// only advances.
    fn font() -> FontDef {
        let g = |u0: f32, u1: f32, pre: f32, width: f32, advance: f32| GlyphDef {
            page: 0,
            uv: [u0, 1.0, u1, 59.0],
            pre,
            width,
            advance,
        };
        FontDef {
            name: "test".into(),
            size: 58.0,
            page_size: (1024.0, 1024.0),
            pages: HashMap::from([(0, "page0.png".to_string())]),
            glyphs: HashMap::from([
                ('0' as u32, g(354.0, 381.0, 3.0, 27.0, 27.0)),
                ('1' as u32, g(385.0, 403.0, 7.0, 18.0, 23.0)),
                (' ' as u32, g(1.0, 4.0, 0.0, 3.0, 17.0)),
            ]),
        }
    }

    #[test]
    fn layout_advances_by_advance_and_offsets_by_pre() {
        let l = layout(&font(), "01", 58.0, Align::Left, 100.0);
        assert_eq!(l.quads.len(), 2);
        assert_eq!(l.quads[0].x, 3.0);
        assert_eq!(l.quads[1].x, 27.0 + 7.0);
        assert_eq!(l.width, 27.0 + 23.0);
        assert_eq!((l.quads[0].w, l.quads[0].h), (27.0, 58.0));
        assert_eq!(l.quads[0].uv, [354.0, 1.0, 381.0, 59.0]);
    }

    #[test]
    fn size_scales_every_quad_and_the_pen() {
        let l = layout(&font(), "01", 29.0, Align::Left, 100.0);
        assert_eq!(l.quads[0].x, 1.5);
        assert_eq!((l.quads[0].w, l.quads[0].h), (13.5, 29.0));
        assert_eq!(l.width, 25.0);
    }

    #[test]
    fn right_alignment_ends_the_pen_at_the_field_edge() {
        let l = layout(&font(), "01", 58.0, Align::Right, 100.0);
        // The pen ends at 100: the first glyph starts at 100 - 50 + its offset.
        assert_eq!(l.quads[0].x, 100.0 - 50.0 + 3.0);
        let c = layout(&font(), "01", 58.0, Align::Center, 100.0);
        assert_eq!(c.quads[0].x, 25.0 + 3.0);
    }

    #[test]
    fn a_missing_glyph_is_skipped_and_a_space_only_advances() {
        let l = layout(&font(), "0?0 1", 58.0, Align::Left, 0.0);
        // '0', (skipped '?'), '0', ' ', '1': four with bitmaps? the space has none drawn.
        let xs: Vec<f32> = l.quads.iter().map(|q| q.x).collect();
        assert_eq!(xs, vec![3.0, 27.0 + 3.0, 27.0 + 27.0 + 17.0 + 7.0]);
        assert_eq!(l.width, 27.0 + 27.0 + 17.0 + 23.0);
    }

    #[test]
    fn an_empty_text_or_a_sizeless_font_lays_out_nothing() {
        assert_eq!(layout(&font(), "", 28.0, Align::Right, 50.0).quads.len(), 0);
        let none = FontDef::default();
        assert_eq!(
            layout(&none, "0", 28.0, Align::Left, 0.0),
            Layout::default()
        );
    }
}
