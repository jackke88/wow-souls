//! Legacy widget data retained for plain-style compatibility and pure layout tests.
//! There is no source-game HUD loader or external-art file reader.

use std::collections::HashMap;

use bevy::prelude::*;

/// The manifest's version word; the extractor writes the same.
#[cfg(test)]
pub const HUD_VERSION: &str = "1";

/// A rectangle in layout pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LRect {
    pub l: f32,
    pub t: f32,
    pub r: f32,
    pub b: f32,
}

impl LRect {
    pub fn w(&self) -> f32 {
        self.r - self.l
    }

    pub fn h(&self) -> f32 {
        self.b - self.t
    }
}

/// A colour as straight sRGB `[r, g, b, a]`, 0..1.
pub type Rgba = [f32; 4];

/// `0xAARRGGBB` to [`Rgba`].
#[cfg(test)]
pub fn argb(c: u32) -> Rgba {
    [
        ((c >> 16) & 0xFF) as f32 / 255.0,
        ((c >> 8) & 0xFF) as f32 / 255.0,
        (c & 0xFF) as f32 / 255.0,
        (c >> 24) as f32 / 255.0,
    ]
}

/// A sprite: its art (`None` when the game assigns one), where it goes in its dialog, its tint.
#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    pub png: Option<String>,
    pub rect: LRect,
    pub tint: Rgba,
    pub additive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    pub rect: LRect,
    pub color: Rgba,
}

/// A rectangle with a colour per corner: top-left, top-right, bottom-left, bottom-right.
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    pub rect: LRect,
    pub colors: [Rgba; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextField {
    pub rect: LRect,
    pub color: Rgba,
    pub size: f32,
}

/// One glyph of the HUD font: its page, its rectangle in the page's pixels and its metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphDef {
    pub page: u32,
    pub uv: [f32; 4],
    pub pre: f32,
    pub width: f32,
    pub advance: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FontDef {
    pub name: String,
    /// The nominal size (the atlas cell height) in atlas pixels.
    pub size: f32,
    pub page_size: (f32, f32),
    pub pages: HashMap<u32, String>,
    pub glyphs: HashMap<u32, GlyphDef>,
}

/// The manifest as data. Keys are `dialog/object`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Manifest {
    pub screen: (f32, f32),
    pub dialogs: HashMap<String, LRect>,
    pub places: HashMap<String, LRect>,
    pub sprites: HashMap<String, Sprite>,
    pub rects: HashMap<String, Fill>,
    pub gradients: HashMap<String, Gradient>,
    pub texts: HashMap<String, TextField>,
    pub font: FontDef,
}

/// What the widgets look up; [`Manifest::validate`] checks the cache has all of it.
#[cfg(test)]
pub const REQUIRED_SPRITES: &[&str] = &[
    "F20-00/bar_0_hp",
    "F20-00/bar_1_hp",
    "F20-00/bar_2_hp",
    "F20-00/HP_endbar",
    "F20-00/bar_0_sp",
    "F20-00/bar_1_sp",
    "F20-00/bar_2_sp",
    "F20-00/SP_endbar",
    "F20-00/bar_BG_0",
    "F20-00/bar_BG_1",
    "F20-00/bar_start",
    "F20-00/bar_start1",
    "F20-02/ring_BG",
    "F20-02/icon_ring_1",
    "F20-02/icon_stone",
    "soul_param/bg_parchment_0",
    "soul_param/Option_line_0",
    "enemy_HP/BG",
    "enemy_HP/HP_bar1",
    "enemy_HP/HP_bar0",
    "F20_LockCursor/lock",
    "F23_YouDied/main_text_youdied",
];
#[cfg(test)]
pub const REQUIRED_RECTS: &[&str] = &[
    "F20-00/plate_hp",
    "F20-00/plate_sp",
    "F23_YouDied/BG",
    "F23_YouDied/BG_bar",
];
#[cfg(test)]
pub const REQUIRED_GRADIENTS: &[&str] = &["F23_YouDied/BG_grad00", "F23_YouDied/BG_grad01"];
#[cfg(test)]
pub const REQUIRED_TEXTS: &[&str] = &[
    "soul_param/text_param_soul",
    "soul_param/text_param_getsoul",
    "enemy_HP/text_param_name",
    "enemy_HP/text_param_damage",
];
#[cfg(test)]
pub const REQUIRED_PLACES: &[&str] = &["F20-00/soul_param", "F20-00/F20_LockCursor"];
#[cfg(test)]
pub const REQUIRED_DIALOGS: &[&str] = &["enemy_HP", "F20_LockCursor"];

impl Manifest {
    /// `Ok` when every key the widgets look up is present and the font can print digits.
    #[cfg(test)]
    pub fn validate(&self) -> Result<(), String> {
        fn need<T>(map: &HashMap<String, T>, kind: &str, keys: &[&str]) -> Result<(), String> {
            match keys.iter().find(|k| !map.contains_key(**k)) {
                Some(k) => Err(format!("the HUD cache has no {kind} {k}")),
                None => Ok(()),
            }
        }
        need(&self.sprites, "sprite", REQUIRED_SPRITES)?;
        need(&self.rects, "rect", REQUIRED_RECTS)?;
        need(&self.gradients, "gradient", REQUIRED_GRADIENTS)?;
        need(&self.texts, "text", REQUIRED_TEXTS)?;
        need(&self.places, "place", REQUIRED_PLACES)?;
        need(&self.dialogs, "dialog", REQUIRED_DIALOGS)?;
        for c in '0'..='9' {
            if !self.font.glyphs.contains_key(&(c as u32)) {
                return Err(format!("the HUD font has no glyph for {c:?}"));
            }
        }
        for key in self.sprites.values().filter_map(|s| s.png.as_ref()) {
            if key.is_empty() {
                return Err("a sprite has an empty file name".into());
            }
        }
        for g in self.font.glyphs.values() {
            if !self.font.pages.contains_key(&g.page) && g.width > 0.0 && g.uv[2] > g.uv[0] {
                return Err(format!(
                    "the HUD font uses page {} but lists no file",
                    g.page
                ));
            }
        }
        Ok(())
    }

    pub fn sprite(&self, key: &str) -> &Sprite {
        self.sprites
            .get(key)
            .unwrap_or_else(|| panic!("HUD sprite {key} (validated at load)"))
    }

    pub fn text(&self, key: &str) -> &TextField {
        self.texts
            .get(key)
            .unwrap_or_else(|| panic!("HUD text {key} (validated at load)"))
    }

    pub fn fill(&self, key: &str) -> &Fill {
        self.rects
            .get(key)
            .unwrap_or_else(|| panic!("HUD rect {key} (validated at load)"))
    }

    pub fn gradient(&self, key: &str) -> &Gradient {
        self.gradients
            .get(key)
            .unwrap_or_else(|| panic!("HUD gradient {key} (validated at load)"))
    }

    pub fn place(&self, key: &str) -> LRect {
        *self
            .places
            .get(key)
            .unwrap_or_else(|| panic!("HUD place {key} (validated at load)"))
    }

    pub fn dialog(&self, key: &str) -> LRect {
        *self
            .dialogs
            .get(key)
            .unwrap_or_else(|| panic!("HUD dialog {key} (validated at load)"))
    }
}

#[cfg(test)]
fn num(w: &[&str], i: usize, line: &str) -> Result<f32, String> {
    w.get(i)
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|f| f.is_finite())
        .ok_or_else(|| format!("bad HUD manifest line {line:?}"))
}

#[cfg(test)]
fn rect(w: &[&str], i: usize, line: &str) -> Result<LRect, String> {
    Ok(LRect {
        l: num(w, i, line)?,
        t: num(w, i + 1, line)?,
        r: num(w, i + 2, line)?,
        b: num(w, i + 3, line)?,
    })
}

#[cfg(test)]
fn color(w: &[&str], i: usize, line: &str) -> Result<Rgba, String> {
    w.get(i)
        .and_then(|s| u32::from_str_radix(s, 16).ok())
        .map(argb)
        .ok_or_else(|| format!("bad HUD manifest line {line:?}"))
}

#[cfg(test)]
fn word<'a>(w: &[&'a str], i: usize, line: &str) -> Result<&'a str, String> {
    w.get(i)
        .copied()
        .ok_or_else(|| format!("bad HUD manifest line {line:?}"))
}

/// Parses the manifest text.
#[cfg(test)]
pub fn parse(text: &str) -> Result<Manifest, String> {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default();
    if first != format!("wscl-hud {HUD_VERSION}") {
        return Err(format!(
            "HUD cache version {first:?}, expected \"wscl-hud {HUD_VERSION}\""
        ));
    }
    let mut m = Manifest::default();
    for line in lines {
        let w: Vec<&str> = line.split(' ').collect();
        match w[0] {
            "source" => {}
            "screen" => m.screen = (num(&w, 1, line)?, num(&w, 2, line)?),
            "dialog" => {
                m.dialogs
                    .insert(word(&w, 1, line)?.to_string(), rect(&w, 2, line)?);
            }
            "place" => {
                m.places
                    .insert(word(&w, 1, line)?.to_string(), rect(&w, 2, line)?);
            }
            "sprite" => {
                let png = word(&w, 2, line)?;
                m.sprites.insert(
                    word(&w, 1, line)?.to_string(),
                    Sprite {
                        png: (png != "-").then(|| png.to_string()),
                        rect: rect(&w, 3, line)?,
                        tint: color(&w, 7, line)?,
                        additive: match word(&w, 8, line)? {
                            "add" => true,
                            "alpha" => false,
                            _ => return Err(format!("bad HUD manifest line {line:?}")),
                        },
                    },
                );
            }
            "rect" => {
                m.rects.insert(
                    word(&w, 1, line)?.to_string(),
                    Fill {
                        rect: rect(&w, 2, line)?,
                        color: color(&w, 6, line)?,
                    },
                );
            }
            "gradient" => {
                m.gradients.insert(
                    word(&w, 1, line)?.to_string(),
                    Gradient {
                        rect: rect(&w, 2, line)?,
                        colors: [
                            color(&w, 6, line)?,
                            color(&w, 7, line)?,
                            color(&w, 8, line)?,
                            color(&w, 9, line)?,
                        ],
                    },
                );
            }
            "text" => {
                m.texts.insert(
                    word(&w, 1, line)?.to_string(),
                    TextField {
                        rect: rect(&w, 2, line)?,
                        color: color(&w, 6, line)?,
                        size: num(&w, 7, line)?,
                    },
                );
            }
            "font" => {
                m.font.name = word(&w, 1, line)?.to_string();
                m.font.size = num(&w, 2, line)?;
                m.font.page_size = (num(&w, 3, line)?, num(&w, 4, line)?);
            }
            "page" => {
                let n = num(&w, 1, line)? as u32;
                m.font.pages.insert(n, word(&w, 2, line)?.to_string());
            }
            "glyph" => {
                let cp = num(&w, 1, line)? as u32;
                m.font.glyphs.insert(
                    cp,
                    GlyphDef {
                        page: num(&w, 2, line)? as u32,
                        uv: [
                            num(&w, 3, line)?,
                            num(&w, 4, line)?,
                            num(&w, 5, line)?,
                            num(&w, 6, line)?,
                        ],
                        pre: num(&w, 7, line)?,
                        width: num(&w, 8, line)?,
                        advance: num(&w, 9, line)?,
                    },
                );
            }
            other => return Err(format!("unknown HUD manifest line {other:?}")),
        }
    }
    Ok(m)
}

/// The loaded art: the manifest and a handle per PNG it names. Present only when the cache is
/// whole; the plain HUD runs without it.
#[derive(Resource)]
pub struct HudArt {
    pub m: Manifest,
    images: HashMap<String, Handle<Image>>,
}

impl HudArt {
    /// The handle of a PNG the manifest names (a default handle for an unknown name: nothing
    /// shows, the manifest was validated at load).
    pub fn handle(&self, png: &str) -> Handle<Image> {
        self.images.get(png).cloned().unwrap_or_default()
    }

    /// The handle of a sprite's art.
    pub fn sprite_image(&self, key: &str) -> Handle<Image> {
        self.m
            .sprite(key)
            .png
            .as_deref()
            .map_or_else(Handle::default, |p| self.handle(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "wscl-hud 1
source 12 345 C:/nowhere/menu.drb.dcx
screen 1280 720
dialog F20-00 0 0 1280 720
place F20-00/soul_param 2 6 1282 726
sprite F20-00/bar_1_hp F20-00__bar_1_hp.png 175 76 907 89 ffffffff alpha
sprite F20_LockCursor/lock F20_LockCursor__lock.png 14 14 89 89 b4ffffff add
sprite F20-00/icon - 3 1 88 96 ffffffff alpha
rect F20-00/plate_hp 175 76 907 89 ff0a0a0a
gradient F23_YouDied/BG_grad00 0 283 1280 327 00000000 00000000 c8000000 c8000000
text soul_param/text_param_soul 1041 630 1169 658 ff9cb9ca 28
font TestGlyphAtlas 58 1024 1024
page 0 font__DSFont24_0.png
glyph 48 0 354 1 381 59 3 27 27
";

    #[test]
    fn the_manifest_parses_every_line_kind() {
        let m = parse(SAMPLE).unwrap();
        assert_eq!(m.screen, (1280.0, 720.0));
        assert_eq!(
            m.dialogs["F20-00"],
            LRect {
                l: 0.0,
                t: 0.0,
                r: 1280.0,
                b: 720.0
            }
        );
        assert_eq!(m.places["F20-00/soul_param"].l, 2.0);
        let bar = m.sprite("F20-00/bar_1_hp");
        assert_eq!(bar.png.as_deref(), Some("F20-00__bar_1_hp.png"));
        assert_eq!((bar.rect.w(), bar.rect.h()), (732.0, 13.0));
        assert_eq!(bar.tint, [1.0, 1.0, 1.0, 1.0]);
        assert!(!bar.additive);
        let lock = m.sprite("F20_LockCursor/lock");
        assert!(lock.additive);
        assert!((lock.tint[3] - 180.0 / 255.0).abs() < 1e-6);
        assert_eq!(m.sprite("F20-00/icon").png, None);
        let plate = m.fill("F20-00/plate_hp");
        assert!((plate.color[0] - 10.0 / 255.0).abs() < 1e-6);
        assert_eq!(plate.color[3], 1.0);
        let g = m.gradient("F23_YouDied/BG_grad00");
        assert_eq!(g.colors[0][3], 0.0);
        assert!((g.colors[2][3] - 200.0 / 255.0).abs() < 1e-6);
        let t = m.text("soul_param/text_param_soul");
        assert_eq!((t.rect.l, t.size), (1041.0, 28.0));
        assert_eq!(m.font.size, 58.0);
        assert_eq!(m.font.page_size, (1024.0, 1024.0));
        assert_eq!(m.font.pages[&0], "font__DSFont24_0.png");
        let zero = m.font.glyphs[&48];
        assert_eq!(zero.uv, [354.0, 1.0, 381.0, 59.0]);
        assert_eq!((zero.pre, zero.width, zero.advance), (3.0, 27.0, 27.0));
    }

    #[test]
    fn a_wrong_version_is_refused() {
        let e = parse("wscl-hud 2\nscreen 1 1\n").unwrap_err();
        assert!(e.contains("version"), "{e}");
        assert!(parse("").is_err());
    }

    #[test]
    fn a_bad_line_names_itself() {
        let e = parse("wscl-hud 1\nsprite a/b x.png 1 2 x 4 ffffffff alpha\n").unwrap_err();
        assert!(e.contains("sprite a/b x.png 1 2 x 4"), "{e}");
        let e = parse("wscl-hud 1\nsprite a/b x.png 1 2 3 4 ffffffff blend\n").unwrap_err();
        assert!(e.contains("blend"), "{e}");
        assert!(parse("wscl-hud 1\nglyph 48 0\n").is_err());
        assert!(parse("wscl-hud 1\nmystery 1 2\n")
            .unwrap_err()
            .contains("mystery"));
    }

    #[test]
    fn colours_unpack_argb() {
        let c = argb(0xFF9C_B9CA);
        assert!((c[0] - 0x9C as f32 / 255.0).abs() < 1e-6);
        assert!((c[1] - 0xB9 as f32 / 255.0).abs() < 1e-6);
        assert!((c[2] - 0xCA as f32 / 255.0).abs() < 1e-6);
        assert_eq!(c[3], 1.0);
        assert_eq!(argb(0x8000_0000)[3], 128.0 / 255.0);
    }

    #[test]
    fn missing_required_parts_are_reported_by_name() {
        let m = parse(SAMPLE).unwrap();
        let e = m.validate().unwrap_err();
        assert!(e.contains("F20-00/bar_0_hp"), "{e}");
    }
}
