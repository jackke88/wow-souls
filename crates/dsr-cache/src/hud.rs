//! `wow-souls-dsr hud`: the Dark Souls HUD's art and layout, cut from the install (read-only) into
//! `cache/dsr/hud/`.
//!
//! The layout comes from `menu/menu.drb.dcx` (dialogs of named objects in a 1280 x 720 space),
//! the art from the menu atlases (`menu/menu_N.tpf.dcx`, the English ones in
//! `menu/ENGLISH/menu_local.tpf.dcx` winning), the digits and letters from `font/english/DSFont24`.
//! [`PARTS`] names what the HUD draws; each sprite is cropped out of its atlas into a PNG of its
//! own (an additive sprite on opaque black becomes straight alpha), and one text manifest says
//! where everything goes:
//!
//! ```text
//! wscl-hud 1
//! source <len> <mtime> <path>                     (every file read; the game checks them)
//! screen <w> <h>
//! dialog <name> <l> <t> <r> <b>                   (a dialog's own rectangle)
//! place <dialog>/<object> <l> <t> <r> <b>         (a sub-dialog or anchor, in its parent)
//! sprite <dialog>/<object> <png|-> <l> <t> <r> <b> <AARRGGBB> <alpha|add>
//! rect <dialog>/<object> <l> <t> <r> <b> <AARRGGBB>
//! gradient <dialog>/<object> <l> <t> <r> <b> <tl> <tr> <bl> <br>
//! text <dialog>/<object> <l> <t> <r> <b> <AARRGGBB> <size>
//! font <name> <size> <page w> <page h>
//! page <n> <png>
//! glyph <code point> <page> <u0> <v0> <u1> <v1> <pre> <width> <advance>
//! ```
//!
//! Colours are lower-case hex `AARRGGBB`; the rectangles are in the dialog's own space.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use dsr_formats::drb::{self, Drb, Object, Shape};
use dsr_formats::{ccm, dcx, tpf};

use crate::cache::source_line;
use crate::dds::{self, Image};

/// The manifest's version word; the game refuses another.
pub const HUD_VERSION: u32 = 1;

/// The HUD font: its files under `font/english/`.
const FONT: &str = "DSFont24";

/// Code points exported besides Latin-1 (32..=255): the typographic quotes and the ellipsis.
const EXTRA_CODE_POINTS: [u32; 5] = [0x2018, 0x2019, 0x201C, 0x201D, 0x2026];

/// What the HUD draws: dialogs of `menu.drb` and the objects of each the game uses. A sprite is
/// exported with its art, a rectangle, gradient or text field as a line, a sub-dialog or anchor as
/// a `place` line.
pub const PARTS: &[(&str, &[&str])] = &[
    (
        "F20-00",
        &[
            "plate_hp",
            "bar_0_hp",
            "bar_1_hp",
            "bar_2_hp",
            "HP_endbar",
            "plate_sp",
            "bar_0_sp",
            "bar_1_sp",
            "bar_2_sp",
            "SP_endbar",
            "bar_BG_0",
            "bar_BG_1",
            "bar_start",
            "bar_start1",
            "soul_param",
            "F20_LockCursor",
            "F20-00_arms_l",
            // The DRB's own spelling: the last letter is a full-width `r` (U+FF52).
            "F20-00_arms_\u{FF52}",
            "F20-00_item",
            "F20-00_magic",
        ],
    ),
    ("F20-02", &["ring_BG", "icon_ring_1", "icon_stone"]),
    (
        "soul_param",
        &[
            "bg_parchment_0",
            "Option_line_0",
            "text_param_soul",
            "text_param_getsoul",
        ],
    ),
    (
        "enemy_HP",
        &[
            "BG",
            "HP_bar1",
            "HP_bar0",
            "text_param_name",
            "text_param_damage",
        ],
    ),
    ("F20_LockCursor", &["lock"]),
    // The boss bar: the layout's own, 667 wide at the bottom centre. Its background object is spelt
    // with two full-width letters (U+FF22 U+FF27).
    (
        "F20_boss_HP",
        &[
            "\u{FF22}\u{FF27}",
            "HP_bar0",
            "HP_bar1",
            "HP_bar2",
            "frame01",
            "frame02",
            "boss_bar_memori4",
            "text_boss_name",
            "text_param_damage",
        ],
    ),
    (
        "F23_YouDied",
        &[
            "BG",
            "BG_grad00",
            "BG_bar",
            "BG_grad01",
            "main_text_youdied",
        ],
    ),
];

/// `AARRGGBB` as the manifest writes it.
fn hex(argb: u32) -> String {
    format!("{argb:08x}")
}

/// The part `dialog`/`object` of the layout, or an error naming it.
fn part<'a>(drb: &'a Drb, dialog: &str, object: &str) -> Result<&'a Object> {
    drb.dialog(dialog)
        .with_context(|| format!("HUD dialog {dialog} is not in menu.drb"))?
        .objects
        .iter()
        .find(|o| o.name == object)
        .with_context(|| format!("HUD part {dialog}/{object} is not in menu.drb"))
}

/// A file name for a part's art: letters, digits, `-` and `_` only.
fn png_name(dialog: &str, object: &str) -> String {
    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    format!("{}__{}.png", clean(dialog), clean(object))
}

/// The manifest line for one object. `png` names its art for a sprite (`None`: the game assigns
/// the texture at run time).
fn line_for(dialog: &str, o: &Object, png: Option<&str>) -> String {
    let (l, t, r, b) = (o.rect.left, o.rect.top, o.rect.right, o.rect.bottom);
    let key = format!("{dialog}/{}", o.name);
    match &o.shape {
        Shape::Sprite { blend, tint, .. } => format!(
            "sprite {key} {} {l} {t} {r} {b} {} {}",
            png.unwrap_or("-"),
            hex(*tint),
            if *blend == 2 { "add" } else { "alpha" }
        ),
        Shape::MonoRect { color } => format!("rect {key} {l} {t} {r} {b} {}", hex(*color)),
        Shape::GouraudRect { colors } => format!(
            "gradient {key} {l} {t} {r} {b} {} {} {} {}",
            hex(colors[0]),
            hex(colors[1]),
            hex(colors[2]),
            hex(colors[3])
        ),
        Shape::Text { color, size } => {
            format!("text {key} {l} {t} {r} {b} {} {size}", hex(*color))
        }
        Shape::Null | Shape::Dialog | Shape::Other(_) => format!("place {key} {l} {t} {r} {b}"),
    }
}

/// Every texture of the menu archives by name; the English language file wins over the rest.
struct Atlases {
    textures: HashMap<String, tpf::Texture>,
    decoded: HashMap<String, Image>,
    sources: Vec<PathBuf>,
}

impl Atlases {
    fn load(dsr: &Path) -> Result<Atlases> {
        let menu = dsr.join("menu");
        let mut files: Vec<PathBuf> = (0..10)
            .map(|i| menu.join(format!("menu_{i}.tpf.dcx")))
            .filter(|p| p.exists())
            .collect();
        files.push(menu.join("ENGLISH").join("menu_local.tpf.dcx"));
        let mut textures = HashMap::new();
        for path in &files {
            let raw = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
            let list = tpf::read(&dcx::decompress(&raw)?)
                .with_context(|| format!("unpacking {}", path.display()))?;
            for t in list {
                textures.insert(t.name.clone(), t);
            }
        }
        Ok(Atlases {
            textures,
            decoded: HashMap::new(),
            sources: files,
        })
    }

    /// The decoded atlas `name`.
    fn image(&mut self, name: &str) -> Result<&Image> {
        if !self.decoded.contains_key(name) {
            let t = self
                .textures
                .get(name)
                .with_context(|| format!("atlas {name} is in no menu archive"))?;
            let img = dds::decode(&t.dds).with_context(|| format!("atlas {name}"))?;
            self.decoded.insert(name.to_string(), img);
        }
        Ok(&self.decoded[name])
    }
}

/// What a run wrote, for the summary line.
#[derive(Debug, Default, PartialEq)]
pub struct Summary {
    pub sprites: usize,
    pub other: usize,
    pub glyphs: usize,
    pub pages: usize,
}

/// Builds `<out>/hud/`: the PNGs and `manifest.txt`.
pub fn run(dsr: &Path, out: &Path) -> Result<Summary> {
    let dir = out.join("hud");
    std::fs::create_dir_all(&dir)?;
    // Only our own files go: a part that left the table must not linger as art.
    for e in std::fs::read_dir(&dir)? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "png" || x == "txt") {
            std::fs::remove_file(&p)?;
        }
    }
    let drb_path = dsr.join("menu").join("menu.drb.dcx");
    let drb = drb::read(&dcx::decompress(&std::fs::read(&drb_path)?)?)
        .with_context(|| format!("reading {}", drb_path.display()))?;
    let mut atlases = Atlases::load(dsr)?;

    let mut lines = String::new();
    let mut sum = Summary::default();
    let screen = drb.dialog("F20-00").context("menu.drb has no F20-00")?.rect;
    writeln!(lines, "screen {} {}", screen.width(), screen.height())?;
    for (dialog, objects) in PARTS {
        let d = drb
            .dialog(dialog)
            .with_context(|| format!("HUD dialog {dialog} is not in menu.drb"))?;
        writeln!(
            lines,
            "dialog {dialog} {} {} {} {}",
            d.rect.left, d.rect.top, d.rect.right, d.rect.bottom
        )?;
        for name in *objects {
            let o = part(&drb, dialog, name)?;
            let mut png = None;
            if let Shape::Sprite {
                uv, texture, blend, ..
            } = &o.shape
            {
                if let Ok(idx) = usize::try_from(*texture) {
                    let atlas = drb
                        .textures
                        .get(idx)
                        .with_context(|| format!("{dialog}/{name}: texture {idx}"))?;
                    let mut crop = atlases
                        .image(atlas)?
                        .crop(
                            i32::from(uv.left),
                            i32::from(uv.top),
                            i32::from(uv.right),
                            i32::from(uv.bottom),
                        )
                        .with_context(|| format!("{dialog}/{name} from {atlas}"))?;
                    if *blend == 2 {
                        crop.black_to_alpha();
                    }
                    let file = png_name(dialog, name);
                    crop.write_png(&dir.join(&file))?;
                    png = Some(file);
                }
                sum.sprites += 1;
            } else {
                sum.other += 1;
            }
            writeln!(lines, "{}", line_for(dialog, o, png.as_deref()))?;
        }
    }

    // The font: the glyph table and the pages it uses.
    let font_dir = dsr.join("font").join("english");
    let ccm_path = font_dir.join(format!("{FONT}.ccm.dcx"));
    let tpf_path = font_dir.join(format!("{FONT}.tpf.dcx"));
    let font = ccm::read(&dcx::decompress(&std::fs::read(&ccm_path)?)?)
        .with_context(|| format!("reading {}", ccm_path.display()))?;
    let pages = tpf::read(&dcx::decompress(&std::fs::read(&tpf_path)?)?)
        .with_context(|| format!("reading {}", tpf_path.display()))?;
    let mut used = BTreeSet::new();
    let mut glyph_lines = String::new();
    for cp in (32u32..=255).chain(EXTRA_CODE_POINTS) {
        let Some(g) = font.glyph(cp) else { continue };
        let [u0, v0, u1, v1] = font.pixels(g);
        writeln!(
            glyph_lines,
            "glyph {cp} {} {u0} {v0} {u1} {v1} {} {} {}",
            g.page, g.pre, g.width, g.advance
        )?;
        if g.width > 0 && u1 > u0 {
            used.insert(g.page);
        }
        sum.glyphs += 1;
    }
    writeln!(
        lines,
        "font {FONT} {} {} {}",
        font.size, font.width, font.height
    )?;
    for &page in &used {
        let tex = pages
            .get(usize::try_from(page)?)
            .with_context(|| format!("{FONT} has no page {page}"))?;
        let file = format!("font__{FONT}_{page}.png");
        dds::decode(&tex.dds)
            .with_context(|| format!("{FONT} page {page}"))?
            .write_png(&dir.join(&file))?;
        writeln!(lines, "page {page} {file}")?;
        sum.pages += 1;
    }
    lines.push_str(&glyph_lines);

    let mut manifest = String::new();
    writeln!(manifest, "wscl-hud {HUD_VERSION}")?;
    for p in std::iter::once(&drb_path)
        .chain(atlases.sources.iter())
        .chain([&ccm_path, &tpf_path])
    {
        writeln!(manifest, "{}", source_line(p)?)?;
    }
    manifest.push_str(&lines);
    std::fs::write(dir.join("manifest.txt"), manifest)?;
    println!(
        "hud: {} sprites, {} other parts, {} glyphs on {} pages -> {}",
        sum.sprites,
        sum.other,
        sum.glyphs,
        sum.pages,
        dir.display()
    );
    if sum.sprites == 0 {
        bail!("no sprites were exported");
    }
    Ok(sum)
}

#[cfg(test)]
mod tests {
    use dsr_formats::drb::Rect;

    use super::*;

    fn rect(l: i16, t: i16, r: i16, b: i16) -> Rect {
        Rect {
            left: l,
            top: t,
            right: r,
            bottom: b,
        }
    }

    fn object(name: &str, rect: Rect, shape: Shape) -> Object {
        Object {
            name: name.into(),
            rect,
            shape,
        }
    }

    #[test]
    fn the_manifest_lines_have_the_documented_shape() {
        let sprite = object(
            "bar_1_hp",
            rect(175, 76, 907, 89),
            Shape::Sprite {
                uv: rect(6, 488, 138, 512),
                texture: 9,
                blend: 1,
                tint: 0xFFFF_FFFF,
            },
        );
        assert_eq!(
            line_for("F20-00", &sprite, Some("F20-00__bar_1_hp.png")),
            "sprite F20-00/bar_1_hp F20-00__bar_1_hp.png 175 76 907 89 ffffffff alpha"
        );
        let glow = object(
            "main_text_youdied",
            rect(128, 324, 1152, 424),
            Shape::Sprite {
                uv: rect(-1, 992, 2047, 1192),
                texture: 37,
                blend: 2,
                tint: 0xFFFF_FFFF,
            },
        );
        assert_eq!(
            line_for("F23_YouDied", &glow, None),
            "sprite F23_YouDied/main_text_youdied - 128 324 1152 424 ffffffff add"
        );
        let plate = object(
            "plate_hp",
            rect(175, 76, 907, 89),
            Shape::MonoRect { color: 0xFF0A_0A0A },
        );
        assert_eq!(
            line_for("F20-00", &plate, None),
            "rect F20-00/plate_hp 175 76 907 89 ff0a0a0a"
        );
        let fade = object(
            "BG_grad00",
            rect(0, 283, 1280, 327),
            Shape::GouraudRect {
                colors: [0, 0, 0xC800_0000, 0xC800_0000],
            },
        );
        assert_eq!(
            line_for("F23_YouDied", &fade, None),
            "gradient F23_YouDied/BG_grad00 0 283 1280 327 00000000 00000000 c8000000 c8000000"
        );
        let text = object(
            "text_param_soul",
            rect(1041, 630, 1169, 658),
            Shape::Text {
                color: 0xFF9C_B9CA,
                size: 28,
            },
        );
        assert_eq!(
            line_for("soul_param", &text, None),
            "text soul_param/text_param_soul 1041 630 1169 658 ff9cb9ca 28"
        );
        let sub = object("F20-00_arms_l", rect(93, 468, 183, 568), Shape::Dialog);
        assert_eq!(
            line_for("F20-00", &sub, None),
            "place F20-00/F20-00_arms_l 93 468 183 568"
        );
    }

    #[test]
    fn part_names_are_unique() {
        let mut seen = BTreeSet::new();
        for (d, objects) in PARTS {
            for o in *objects {
                assert!(seen.insert((*d, *o)), "{d}/{o} is listed twice");
            }
        }
        // Art names too: two parts never share a PNG.
        let mut files = BTreeSet::new();
        for (d, objects) in PARTS {
            for o in *objects {
                assert!(files.insert(png_name(d, o)), "{d}/{o} shares a file name");
            }
        }
    }

    #[test]
    fn an_unknown_part_is_an_error_naming_it() {
        let drb = Drb {
            textures: vec![],
            dialogs: vec![drb::Dialog {
                name: "F20-00".into(),
                rect: rect(0, 0, 1280, 720),
                objects: vec![object("a", rect(0, 0, 1, 1), Shape::Null)],
            }],
        };
        assert!(part(&drb, "F20-00", "a").is_ok());
        let e = part(&drb, "F20-00", "nope").unwrap_err().to_string();
        assert!(e.contains("F20-00/nope"), "{e}");
        let e = part(&drb, "F99", "a").unwrap_err().to_string();
        assert!(e.contains("F99"), "{e}");
    }

    #[test]
    fn art_file_names_are_clean() {
        assert_eq!(png_name("F20-00", "HP icon"), "F20-00__HP_icon.png");
        assert_eq!(png_name("a/b", "c.d"), "a_b__c_d.png");
        assert_eq!(png_name("F20-00", "arms_\u{FF52}"), "F20-00__arms__.png");
    }

    /// `DSR_DIR`: the whole HUD cache builds into a scratch directory, with the numbers the survey
    /// found.
    #[test]
    fn install_the_hud_cache_builds() {
        let Ok(dsr) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let out = std::env::temp_dir().join(format!("wow-souls-hud-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let sum = run(Path::new(&dsr), &out).unwrap();
        assert!(
            sum.sprites >= 20 && sum.glyphs >= 190 && sum.pages >= 1,
            "{sum:?}"
        );
        let dir = out.join("hud");
        let m = std::fs::read_to_string(dir.join("manifest.txt")).unwrap();
        let has = |line: &str| assert!(m.lines().any(|l| l == line), "missing: {line}");
        assert_eq!(m.lines().next(), Some("wscl-hud 1"));
        has("screen 1280 720");
        has("dialog F20-00 0 0 1280 720");
        has("sprite F20-00/bar_1_hp F20-00__bar_1_hp.png 175 76 907 89 ffffffff alpha");
        has("rect F20-00/plate_hp 175 76 907 89 ff0a0a0a");
        has("place F20-00/F20-00_arms_l 93 468 183 568");
        has("text soul_param/text_param_soul 1041 630 1169 658 ff9cb9ca 28");
        has("glyph 48 0 354 1 381 59 3 27 27");
        assert!(
            m.lines()
                .any(|l| l.starts_with("sprite F23_YouDied/main_text_youdied ")
                    && l.ends_with(" add"))
        );
        assert_eq!(m.lines().filter(|l| l.starts_with("source ")).count(), 10);
        // The bar fill is the 132 x 24 stripe.
        let img = std::fs::File::open(dir.join("F20-00__bar_1_hp.png")).unwrap();
        let dec = png::Decoder::new(std::io::BufReader::new(img));
        let info = dec.read_info().unwrap().info().clone();
        assert_eq!((info.width, info.height), (132, 24));
        let _ = std::fs::remove_dir_all(&out);
    }
}
