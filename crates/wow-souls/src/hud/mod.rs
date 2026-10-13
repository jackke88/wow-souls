//! The player's HUD, our own (not the WoW UI): a one-line developer readout (camera mode, ground
//! state, achieved speed, gait, lock) and, at the bottom left, a help line (the keys, and the
//! gamepad the game sees: [`crate::pad`]). ASCII only, the default font has no `·`. F3 hides both.
//!
//! The HUD proper is Dark Souls' own, drawn from `cache/dsr/hud` ([`art`]) in a 1280 x 720
//! layout space; without that cache [`plain`] draws the bars as coloured rectangles.

pub mod art;
mod banner;
mod bars;
mod boss;
mod enemy;
pub mod font;
mod lock;
mod plain;
pub mod souls;
mod xp;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub use art::{HudArt, LRect};

use crate::avatar::Avatar;
use crate::camera::CamMode;
use crate::motor::Motor;
use crate::pad::PadStatus;
use crate::settle::Phase;

#[derive(Component)]
struct DevLine;

/// The bottom-left help: the controls and the pad status.
#[derive(Component)]
struct HelpLine;

/// The layout space of the Dark Souls HUD, in its own pixels.
pub const LAYOUT_W: f32 = 1280.0;
pub const LAYOUT_H: f32 = 720.0;

/// `UiScale` that makes one layout pixel `window_height / 720` screen pixels, as Dark Souls
/// scales its 720p layout to a bigger window.
pub fn scale_for(window_height: f32) -> f32 {
    (window_height / LAYOUT_H).max(0.1)
}

/// Which screen edge a piece of the HUD hangs from: a wider window grows the margins, not the
/// HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Left,
    Right,
    Center,
}

/// An absolutely placed node for `rect` (layout pixels), measured from its anchor's edge.
pub fn node_at(rect: LRect, anchor: Anchor) -> Node {
    let mut n = Node {
        position_type: PositionType::Absolute,
        top: px(rect.t),
        width: px(rect.w()),
        height: px(rect.h()),
        ..default()
    };
    match anchor {
        Anchor::Left => n.left = px(rect.l),
        Anchor::Right => n.right = px(LAYOUT_W - rect.r),
        Anchor::Center => {
            n.left = percent(50.0);
            n.margin.left = px(rect.l - LAYOUT_W / 2.0);
        }
    }
    n
}

/// The length (layout px) of a stat's bar: the other HUD parts hang under the stamina bar and match it.
pub(crate) fn bar_length(max: f32, longest: f32) -> f32 {
    bars::bar_len(max, longest)
}

/// A manifest colour as a Bevy one.
pub(crate) fn tint(c: art::Rgba) -> Color {
    Color::srgba(c[0], c[1], c[2], c[3])
}

/// `WOWSOULS_HUD_GAIN=<x>`: multiplies the colour of the HUD's art (not its text or its plain
/// rectangles); the Dark Souls bars are dark, 1.0 is the art as shipped.
fn gain() -> f32 {
    static GAIN: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *GAIN.get_or_init(|| {
        std::env::var("WOWSOULS_HUD_GAIN")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|g| g.is_finite() && *g > 0.0)
            .unwrap_or(1.0)
    })
}

/// A sprite's tint with the art's gain applied.
pub(crate) fn sprite_tint(c: art::Rgba) -> Color {
    let g = gain();
    Color::srgba(
        (c[0] * g).min(1.0),
        (c[1] * g).min(1.0),
        (c[2] * g).min(1.0),
        c[3],
    )
}

/// A sprite at its manifest rectangle: the node and its picture (tinted as Dark Souls tints it).
pub(crate) fn sprite_node(art: &HudArt, key: &str, anchor: Anchor) -> (Node, ImageNode) {
    let s = art.m.sprite(key);
    (
        node_at(s.rect, anchor),
        ImageNode::new(art.sprite_image(key)).with_color(sprite_tint(s.tint)),
    )
}

/// A sprite cropped to `len` layout pixels: a clipping container at the sprite's place (anchored
/// left), the picture inside at its full width. Returns the container, whose width a caller may
/// change to crop more or less of the picture.
pub(crate) fn clip_sprite(
    commands: &mut Commands,
    parent: Entity,
    art: &HudArt,
    key: &str,
    len: f32,
) -> Entity {
    let s = art.m.sprite(key);
    let mut node = node_at(s.rect, Anchor::Left);
    node.width = px(len);
    node.overflow = Overflow::clip();
    let holder = commands.spawn((node, ChildOf(parent))).id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: px(s.rect.w()),
            height: percent(100),
            ..default()
        },
        ImageNode::new(art.sprite_image(key)).with_color(sprite_tint(s.tint)),
        ChildOf(holder),
    ));
    holder
}

pub fn plugin(app: &mut App) {
    plain::plugin(app);
    bars::plugin(app);
    font::plugin(app);
    souls::plugin(app);
    xp::plugin(app);
    enemy::plugin(app);
    boss::plugin(app);
    lock::plugin(app);
    banner::plugin(app);
    app.add_systems(Startup, load_art)
        .add_systems(OnEnter(Phase::Playing), spawn)
        .add_systems(
            Update,
            (toggle, update, update_help).run_if(in_state(Phase::Playing)),
        )
        .add_systems(
            Update,
            (fit_ui_scale, keep_dev_text_size)
                .chain()
                .run_if(resource_exists::<HudArt>),
        );
}

/// Loads the Dark Souls art once; without it the plain HUD stays, with one warning.
fn load_art(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    if std::env::var("WOWSOULS_HUD").is_ok_and(|v| v.trim() == "0") {
        info!("hud: WOWSOULS_HUD=0, the plain HUD");
        return;
    }
    let dir = crate::dsclip::cache_dir().join("hud");
    match HudArt::load(&dir, &mut images) {
        Ok(art) => {
            info!(
                "hud: Dark Souls art loaded ({} sprites, {} glyphs)",
                art.m.sprites.len(),
                art.m.font.glyphs.len()
            );
            commands.insert_resource(art);
        }
        Err(why) => warn!("hud: no Dark Souls art ({why}); the plain HUD stays"),
    }
}

/// The developer lines are plain text, not layout: undo the HUD's scale on their size so a bigger
/// window does not wrap them.
fn keep_dev_text_size(
    ui: Res<UiScale>,
    mut lines: Query<&mut TextFont, Or<(With<DevLine>, With<HelpLine>)>>,
) {
    let want = DEV_TEXT_SIZE / ui.0.max(0.1);
    for mut f in &mut lines {
        if (f.font_size - want).abs() > 0.01 {
            f.font_size = want;
        }
    }
}

/// The developer lines' font size in screen pixels.
const DEV_TEXT_SIZE: f32 = 14.0;

/// Keeps the layout's height fitted to the window's.
fn fit_ui_scale(windows: Query<&Window, With<PrimaryWindow>>, mut ui: ResMut<UiScale>) {
    let Ok(window) = windows.single() else {
        return;
    };
    let s = scale_for(window.resolution.height());
    if (ui.0 - s).abs() > 1.0e-4 {
        ui.0 = s;
    }
}

fn spawn(mut commands: Commands) {
    // A recording shows the game, not its developer lines.
    let dev = if crate::record::active() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    commands.spawn((
        DevLine,
        dev,
        Text::new(""),
        TextFont::from_font_size(14.0),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
        Node {
            position_type: PositionType::Absolute,
            left: px(10),
            top: px(8),
            ..default()
        },
    ));
    commands.spawn((
        HelpLine,
        dev,
        Text::new(""),
        TextFont::from_font_size(14.0),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
        Node {
            position_type: PositionType::Absolute,
            left: px(10),
            bottom: px(8),
            ..default()
        },
    ));
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut lines: Query<&mut Visibility, Or<(With<DevLine>, With<HelpLine>)>>,
) {
    if keys.just_pressed(KeyCode::F3) {
        for mut v in &mut lines {
            v.toggle_visible_hidden();
        }
    }
}

/// The keys (the player's own bindings), and what the game sees of the pad: a pad that is plugged in
/// but silent shows here. The line is off with the game's "help line" setting.
fn update_help(
    pad: Res<PadStatus>,
    settings: Res<crate::settings::Settings>,
    mut line: Query<(&mut Text, &mut Node), With<HelpLine>>,
) {
    let Ok((mut text, mut node)) = line.single_mut() else {
        return;
    };
    let (keys, buttons) = settings.binds.help_lines();
    let wanted = format!("{keys}\n{}   {buttons}", crate::pad::describe(&pad));
    if text.0 != wanted {
        text.0 = wanted;
    }
    let display = if settings.game.help {
        Display::Flex
    } else {
        Display::None
    };
    if node.display != display {
        node.display = display;
    }
}

fn update(
    mode: Res<CamMode>,
    lock: Res<crate::lockon::LockOn>,
    names: Query<&Name>,
    avatar: Query<(&Motor, Option<&crate::actor::anim::Locomotion>), With<Avatar>>,
    mut line: Query<&mut Text, With<DevLine>>,
) {
    let (Ok((m, loco)), Ok(mut text)) = (avatar.single(), line.single_mut()) else {
        return;
    };
    let state = if m.grounded { "grounded" } else { "airborne" };
    let wanted = format!(
        "{:?} cam | {state} | {:.1} yd/s | {} | lock {}",
        *mode,
        m.velocity.xz().length(),
        loco.map_or("-".to_string(), |l| format!("{:?}", l.gait)),
        lock.target
            .and_then(|e| names.get(e).ok())
            .map_or("-", |n| n.as_str())
    );
    if text.0 != wanted {
        text.0 = wanted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_scale_follows_the_window_height() {
        assert_eq!(scale_for(720.0), 1.0);
        assert_eq!(scale_for(1080.0), 1.5);
        assert_eq!(scale_for(360.0), 0.5);
        assert_eq!(scale_for(0.0), 0.1);
    }

    #[test]
    fn right_items_hang_from_the_right_edge_and_left_ones_from_the_left() {
        let r = LRect {
            l: 1027.0,
            t: 626.0,
            r: 1184.0,
            b: 666.0,
        };
        let right = node_at(r, Anchor::Right);
        assert_eq!(right.right, px(96.0));
        assert_eq!(right.left, Val::Auto);
        assert_eq!(
            (right.width, right.height, right.top),
            (px(157.0), px(40.0), px(626.0))
        );
        let left = node_at(r, Anchor::Left);
        assert_eq!(left.left, px(1027.0));
        assert_eq!(left.right, Val::Auto);
        let mid = node_at(r, Anchor::Center);
        assert_eq!(mid.left, percent(50.0));
        assert_eq!(mid.margin.left, px(387.0));
    }
}
