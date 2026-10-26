//! The creation screen: WoW 1.12's `CharacterCreate.xml` layout in Bevy UI with the shared worn
//! material theme. The authored numbers are the client's own (a 1024 x 768 screen; the fork's
//! `benilla-app/src/char_create/screen.rs` is the worked reference); one authored unit is `720 / 768`
//! layout pixels, because the HUD's `UiScale` already makes a layout pixel `window height / 720`.
//!
//! The screen is built when [`Screen::Create`] comes up and torn down when it goes. Everything live is
//! refreshed from the [`Menu`] every frame (writes only on a change); the buttons send [`MenuPress`]
//! messages, which `menu.rs` turns into outcomes exactly as it does a key.

use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::sprite::{BorderRect, TextureSlicer};
use bevy::time::Real;
use bevy::ui::widget::NodeImageMode;
use bevy::ui::FocusPolicy;
use bevy::window::PrimaryWindow;

use super::art::{self, Art};
use super::rules::{self, Faction};
use super::CreateData;
use crate::menu::{Menu, MenuPress, MenuStyle, Press, Row, Screen};
use crate::settle::Phase;

use crate::hud::theme::{GOLD, IVORY, MUTED};
const GOLD_DIM: Color = crate::hud::theme::EDGE;
const WARN: Color = Color::srgb(0.85, 0.42, 0.32);
const INK: Color = crate::hud::theme::INK;
const PLATE: Color = Color::srgba(0.05, 0.04, 0.03, 0.78);
const PLATE_HOT: Color = Color::srgba(0.2, 0.16, 0.08, 0.9);
const FILL_ALLIANCE: Color = Color::srgba(0.03, 0.045, 0.1, 0.9);
const FILL_HORDE: Color = Color::srgba(0.1, 0.035, 0.03, 0.9);
const CLEAR: Color = Color::NONE;
/// A race the game cannot start yet: dark, with a quiet edge.
const GREYED: Color = Color::srgb(0.26, 0.26, 0.27);
const GREYED_EDGE: Color = Color::srgb(0.2, 0.19, 0.17);

/// One authored unit in layout pixels (see the module docs).
const K: f32 = 720.0 / 768.0;

fn u(v: f32) -> Val {
    Val::Px(v * K)
}

/// The drag's turn per authored unit: `CHARACTER_ROTATION_CONSTANT` 0.6 degrees.
const ROTATION_PER_UNIT: f32 = 0.6 * std::f32::consts::PI / 180.0;
/// The rotate buttons' rate: 2 degrees per 60 Hz tick of the reference.
const ROTATE_RATE: f32 = 120.0 * std::f32::consts::PI / 180.0;
/// The name caret blinks every half second (the reference's `blinkSpeed`).
const BLINK: f32 = 0.5;

/// What a clickable element does.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    Race(u8),
    Sex(u8),
    ClassSlot(u8),
    Dial(u8, i8),
    Randomize,
    Create,
    Back,
    Name,
    /// Hold to turn the body.
    RotateLeft,
    RotateRight,
    /// The 3D area: drag to turn the body.
    Model,
}

impl Act {
    fn press(self) -> Option<Press> {
        Some(match self {
            Act::Race(r) => Press::Race(r),
            Act::Sex(s) => Press::Sex(s),
            Act::ClassSlot(n) => Press::ClassSlot(n),
            Act::Dial(d, dir) => Press::Dial(d, dir),
            Act::Randomize => Press::Randomize,
            Act::Create => Press::Create,
            Act::Back => Press::Back,
            Act::Name => Press::NameFocus,
            Act::RotateLeft | Act::RotateRight | Act::Model => return None,
        })
    }
}

#[derive(Component)]
struct CreateRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Info {
    Faction,
    Race,
    Class,
}

/// What the refresh rewrites.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Dyn {
    RaceIcon(u8),
    SexIcon(u8),
    ClassIcon(u8),
    InfoIcon(Info),
    RaceCaption(u8),
    SexCaption(u8),
    ClassCaption(u8),
    /// The caption's dark strip (shown for the chosen button only).
    CaptionStrip(u8, u8),
    DialRow(u8),
    DialLabel(u8),
    InfoTitle(Info),
    InfoBody(Info),
    InfoAbilities,
    PanelFill,
    NameText,
    Status,
    Create,
}

/// A row of the focus ring this element belongs to (keyboard and pad): it is outlined while focused.
#[derive(Component, Clone, Copy)]
struct FocusRow(Row);

/// A scrolling text pane of an info panel.
#[derive(Component)]
struct InfoScroll;

/// A button whose plate lights while the pointer is on it.
#[derive(Component)]
struct Hot;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load_art)
        .add_systems(OnExit(Phase::Menu), despawn_root)
        .add_systems(
            Update,
            (sync_root, refresh, clicks, hot_plates, wheel, rotate)
                .chain()
                .run_if(in_state(Phase::Menu).and(resource_exists::<Menu>)),
        );
}

fn load_art(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Art::load(&assets));
}

/// Leaving the menu (into the world, or a quit) takes the screen down with it.
fn despawn_root(mut commands: Commands, roots: Query<Entity, With<CreateRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

/// What the spawn needs to know.
struct Ctx<'a> {
    font: Handle<Font>,
    parchment: Option<Handle<Image>>,
    art: &'a Art,
    strings: Option<&'a super::Strings>,
}

impl Ctx<'_> {
    fn caption<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.strings.map_or(fallback, |s| s.text(key, fallback))
    }
}

fn abs(left: f32, top: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: u(left),
        top: u(top),
        width: u(w),
        height: u(h),
        ..default()
    }
}

fn text(ctx: &Ctx, size: f32, color: Color, value: &str) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font: ctx.font.clone(),
            font_size: size * K,
            ..default()
        },
        TextColor(color),
    )
}

/// A parchment-framed (or plain dark) panel's look.
fn parchment(ctx: &Ctx, node: &mut EntityCommands) {
    if crate::hud::theme::enabled() {
        crate::hud::theme::dress(node);
        return;
    }
    node.insert(BackgroundColor(INK));
    if let Some(image) = &ctx.parchment {
        let mut img = ImageNode::new(image.clone());
        img.image_mode = NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect::all(18.0),
            ..default()
        });
        node.insert(img);
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_root(
    mut commands: Commands,
    menu: Res<Menu>,
    style: Option<Res<MenuStyle>>,
    art: Option<Res<Art>>,
    data: Option<Res<CreateData>>,
    roots: Query<Entity, With<CreateRoot>>,
) {
    let want = menu.screen == Screen::Create;
    match (want, roots.is_empty()) {
        (true, true) => {
            let (Some(style), Some(art)) = (style, art) else {
                return;
            };
            let ctx = Ctx {
                font: style.font.clone(),
                parchment: style.parchment.clone(),
                art: &art,
                strings: data.as_deref().map(|d| &*d.strings),
            };
            spawn_screen(&mut commands, &ctx, &menu);
        }
        (false, false) => {
            for e in &roots {
                commands.entity(e).despawn();
            }
        }
        _ => {}
    }
}

fn spawn_screen(commands: &mut Commands, ctx: &Ctx, menu: &Menu) {
    let _ = menu;
    commands
        .spawn((
            CreateRoot,
            GlobalZIndex(110),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|root| {
            // The 3D area: the whole window, dragged to turn the body. The chrome sits above it.
            root.spawn((
                Act::Model,
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
            left_tower(root, ctx);
            root.spawn((
                abs(28.0, 18.0, 650.0, 40.0),
                text(ctx, 26.0, IVORY, "Create character"),
            ));
            right_stack(root, ctx);
            bottom_bar(root, ctx);
        });
}

/// `CharacterCreateConfigurationFrame`: the tower of pickers, every child at its authored offset from
/// (28, 74).
fn left_tower(root: &mut ChildSpawnerCommands, ctx: &Ctx) {
    let mut tower = root.spawn(abs(28.0, 74.0, 206.0, 600.0));
    tower.with_children(|t| {
        // The parchment behind the whole tower.
        let mut back = t.spawn(abs(-12.0, -14.0, 230.0, 700.0));
        parchment(ctx, &mut back);
        back.insert(FocusPolicy::Block)
            .insert(Interaction::default());

        // The faction banners behind the race grid (`CharacterCreateBanners`, 256 x 259 at (-27, 60)).
        t.spawn((
            ImageNode {
                image: ctx.art.banners.clone(),
                color: Color::srgba(1.0, 1.0, 1.0, 0.55),
                ..default()
            },
            abs(-27.0, 60.0, 256.0, 259.0),
        ));
        for (key, fallback, center) in [("ALLIANCE", "Alliance", 51.0), ("HORDE", "Horde", 151.0)] {
            t.spawn(Node {
                justify_content: JustifyContent::Center,
                ..abs(center - 60.0, 38.0, 120.0, 20.0)
            })
            .with_children(|l| {
                l.spawn(text(ctx, 15.0, GOLD, ctx.caption(key, fallback)));
            });
        }

        // Two columns of 48 px race buttons at (33, 68) and (127, 68), pitch 53.
        for (races, left) in [(rules::ALLIANCE, 33.0), (rules::HORDE, 127.0)] {
            t.spawn(Node {
                position_type: PositionType::Absolute,
                left: u(left),
                top: u(68.0),
                flex_direction: FlexDirection::Column,
                row_gap: u(5.0),
                ..default()
            })
            .with_children(|col| {
                for race in races {
                    icon_button(
                        col,
                        ctx,
                        Act::Race(race),
                        Dyn::RaceIcon(race),
                        Some(Dyn::RaceCaption(race)),
                        FocusRow(Row::Race),
                        ctx.art.races.clone(),
                    );
                }
            });
        }

        // The gender pair at (53, 303).
        t.spawn(Node {
            position_type: PositionType::Absolute,
            left: u(53.0),
            top: u(303.0),
            flex_direction: FlexDirection::Row,
            column_gap: u(5.0),
            ..default()
        })
        .with_children(|g| {
            for sex in 0..=1u8 {
                icon_button(
                    g,
                    ctx,
                    Act::Sex(sex),
                    Dyn::SexIcon(sex),
                    Some(Dyn::SexCaption(sex)),
                    FocusRow(Row::Sex),
                    ctx.art.gender.clone(),
                );
            }
        });

        // The class grid, three wide from (27, 369); only the race's classes show.
        t.spawn(Node {
            position_type: PositionType::Absolute,
            left: u(27.0),
            top: u(369.0),
            width: u(3.0 * 48.0 + 2.0 * 4.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: u(4.0),
            row_gap: u(4.0),
            ..default()
        })
        .with_children(|grid| {
            for slot in 0..8u8 {
                icon_button(
                    grid,
                    ctx,
                    Act::ClassSlot(slot),
                    Dyn::ClassIcon(slot),
                    Some(Dyn::ClassCaption(slot)),
                    FocusRow(Row::Class),
                    ctx.art.classes.clone(),
                );
            }
        });

        // The five dial rows, 198 x 32 stacked from (4, 480).
        t.spawn(Node {
            position_type: PositionType::Absolute,
            left: u(4.0),
            top: u(480.0),
            width: u(198.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|dials| {
            for dial in 0..5u8 {
                dial_row(dials, ctx, dial);
            }
        });

        // Randomize at (30, 645), 146 x 30.
        t.spawn((
            Act::Randomize,
            Button,
            Hot,
            FocusRow(Row::Randomize),
            Outline::new(u(2.0), Val::Px(0.0), CLEAR),
            BackgroundColor(PLATE),
            BorderColor::all(GOLD_DIM),
            Node {
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..abs(30.0, 645.0, 146.0, 30.0)
            },
        ))
        .with_children(|b| {
            b.spawn(text(
                ctx,
                14.0,
                IVORY,
                ctx.caption("RANDOMIZE", "Randomize"),
            ));
        });
    });
}

/// A 48 px picker button: the sheet's cell as its face, a gold border when chosen, a caption strip
/// (the chosen name) over its foot.
fn icon_button(
    parent: &mut ChildSpawnerCommands,
    ctx: &Ctx,
    act: Act,
    face: Dyn,
    caption: Option<Dyn>,
    focus: FocusRow,
    sheet: Handle<Image>,
) {
    parent
        .spawn((
            act,
            face,
            focus,
            Button,
            Hot,
            Outline::new(u(2.0), u(1.0), CLEAR),
            BorderColor::all(GOLD_DIM),
            ImageNode {
                image: sheet,
                ..default()
            },
            Node {
                width: u(48.0),
                height: u(48.0),
                border: UiRect::all(Val::Px(2.0)),
                align_items: AlignItems::FlexEnd,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|b| {
            let Some(caption) = caption else { return };
            let (kind, index) = match caption {
                Dyn::RaceCaption(i) => (0, i),
                Dyn::SexCaption(i) => (1, i),
                Dyn::ClassCaption(i) => (2, i),
                _ => (3, 0),
            };
            b.spawn((
                Dyn::CaptionStrip(kind, index),
                Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
            ))
            .with_children(|strip| {
                strip.spawn((caption, text(ctx, 10.0, IVORY, "")));
            });
        });
}

/// One customization row: the plate with its label and the two arrows.
fn dial_row(dials: &mut ChildSpawnerCommands, ctx: &Ctx, dial: u8) {
    dials
        .spawn((
            Dyn::DialRow(dial),
            FocusRow(Row::Dial(dial)),
            Outline::new(u(1.0), u(1.0), CLEAR),
            Node {
                width: u(198.0),
                height: u(32.0),
                margin: UiRect::bottom(u(2.0)),
                ..default()
            },
        ))
        .with_children(|row| {
            // The plate (the label's frame): x 20 .. 129 of the row.
            row.spawn((
                Node {
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..abs(4.0, 3.0, 128.0, 26.0)
                },
                BackgroundColor(PLATE),
                BorderColor::all(GOLD_DIM),
            ))
            .with_children(|plate| {
                plate.spawn((Dyn::DialLabel(dial), text(ctx, 12.0, IVORY, "")));
            });
            for (act, left, glyph) in [
                (Act::Dial(dial, -1), 137.0, "<"),
                (Act::Dial(dial, 1), 166.0, ">"),
            ] {
                row.spawn((
                    act,
                    Button,
                    Hot,
                    BackgroundColor(PLATE),
                    BorderColor::all(GOLD_DIM),
                    Node {
                        border: UiRect::all(Val::Px(1.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..abs(left, 3.0, 26.0, 26.0)
                    },
                ))
                .with_children(|b| {
                    b.spawn(text(ctx, 16.0, GOLD, glyph));
                });
            }
        });
}

/// The right stack: the faction, race and class panels, each a parchment panel with its header icon
/// and a scrolling text.
fn right_stack(root: &mut ChildSpawnerCommands, ctx: &Ctx) {
    root.spawn(Node {
        position_type: PositionType::Absolute,
        right: u(20.0),
        top: u(20.0),
        width: u(270.0),
        flex_direction: FlexDirection::Column,
        row_gap: u(10.0),
        ..default()
    })
    .with_children(|stack| {
        for (kind, height, sheet) in [
            (Info::Faction, 130.0, ctx.art.factions.clone()),
            (Info::Race, 280.0, ctx.art.races.clone()),
            (Info::Class, 220.0, ctx.art.classes.clone()),
        ] {
            let mut panel = stack.spawn((
                Dyn::PanelFill,
                Interaction::default(),
                FocusPolicy::Block,
                Node {
                    width: Val::Percent(100.0),
                    height: u(height),
                    ..default()
                },
            ));
            parchment(ctx, &mut panel);
            panel.with_children(|p| {
                // The header icon, overhanging the panel's corner (-3, -8), above the text.
                p.spawn((
                    Dyn::InfoIcon(kind),
                    ImageNode {
                        image: sheet,
                        ..default()
                    },
                    ZIndex(1),
                    BorderColor::all(CLEAR),
                    abs(-3.0, -8.0, 48.0, 48.0),
                ));
                p.spawn((
                    InfoScroll,
                    Interaction::default(),
                    FocusPolicy::Block,
                    ScrollPosition::default(),
                    Node {
                        position_type: PositionType::Absolute,
                        left: u(17.0),
                        top: u(10.0),
                        width: u(236.0),
                        bottom: u(10.0),
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                ))
                .with_children(|s| {
                    s.spawn((
                        Dyn::InfoTitle(kind),
                        Node {
                            margin: UiRect::left(u(32.0)),
                            ..default()
                        },
                        text(ctx, 18.0, GOLD, ""),
                    ));
                    s.spawn((
                        Dyn::InfoBody(kind),
                        Node {
                            margin: UiRect::top(u(2.0)),
                            ..default()
                        },
                        text(ctx, 11.5, IVORY, ""),
                    ));
                    if kind == Info::Race {
                        s.spawn((
                            Dyn::InfoAbilities,
                            Node {
                                margin: UiRect::top(u(4.0)),
                                ..default()
                            },
                            text(ctx, 11.5, GOLD, ""),
                        ));
                    }
                });
            });
        }
    });
}

/// The name box, the rotate buttons, Create and Back, and the key line.
fn bottom_bar(root: &mut ChildSpawnerCommands, ctx: &Ctx) {
    // NAME over the edit box, centred on a dark plate so it reads over any ground.
    root.spawn(Node {
        position_type: PositionType::Absolute,
        bottom: u(30.0),
        left: Val::Px(0.0),
        width: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        ..default()
    })
    .with_children(|row| {
        row.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: u(3.0),
                padding: UiRect::axes(u(14.0), u(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                max_width: u(460.0),
                ..default()
            },
            BackgroundColor(INK),
            BorderColor::all(GOLD_DIM),
        ))
        .with_children(|c| {
            c.spawn(text(ctx, 17.0, GOLD, ctx.caption("NAME", "Name")));
            c.spawn((
                FocusRow(Row::Name),
                Button,
                Act::Name,
                Outline::new(u(2.0), u(1.0), CLEAR),
                Node {
                    width: u(170.0),
                    height: u(34.0),
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::left(u(12.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(PLATE),
                BorderColor::all(GOLD_DIM),
            ))
            .with_children(|b| {
                b.spawn((Dyn::NameText, text(ctx, 16.0, IVORY, "")));
            });
            c.spawn((
                Dyn::Status,
                Node {
                    max_width: u(420.0),
                    ..default()
                },
                text(ctx, 12.0, MUTED, ""),
            ));
        });
    });

    // The two rotate buttons at (237, bottom 0), 50 x 50, overlapping by 19.
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: u(237.0),
        bottom: u(0.0),
        flex_direction: FlexDirection::Row,
        ..default()
    })
    .with_children(|r| {
        for (act, flip, overlap) in [
            (Act::RotateLeft, true, 0.0),
            (Act::RotateRight, false, -19.0),
        ] {
            r.spawn((
                act,
                Button,
                ImageNode {
                    image: ctx.art.rotate_up.clone(),
                    flip_x: flip,
                    ..default()
                },
                Node {
                    width: u(50.0),
                    height: u(50.0),
                    margin: UiRect::left(u(overlap)),
                    ..default()
                },
            ));
        }
    });

    // Create over Back at (right 50, bottom 20).
    root.spawn(Node {
        position_type: PositionType::Absolute,
        right: u(50.0),
        bottom: u(20.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: u(5.0),
        ..default()
    })
    .with_children(|c| {
        for (act, row, label, w, h, size) in [
            (
                Act::Create,
                Row::Create,
                "Create character",
                190.0,
                35.0,
                16.0,
            ),
            (
                Act::Back,
                Row::Back,
                ctx.caption("BACK", "Back"),
                120.0,
                30.0,
                14.0,
            ),
        ] {
            c.spawn((
                act,
                Button,
                Hot,
                FocusRow(row),
                Outline::new(u(2.0), u(1.0), CLEAR),
                BackgroundColor(PLATE),
                BorderColor::all(GOLD_DIM),
                Node {
                    width: u(w),
                    height: u(h),
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|b| {
                let mut e = b.spawn(text(ctx, size, GOLD, label));
                if act == Act::Create {
                    e.insert(Dyn::Create);
                }
            });
        }
    });

    // The key line along the bottom, on its own strip.
    root.spawn(Node {
        position_type: PositionType::Absolute,
        bottom: u(4.0),
        left: Val::Px(0.0),
        width: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        ..default()
    })
    .with_children(|c| {
        c.spawn((
            Node {
                padding: UiRect::axes(u(12.0), u(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.7)),
        ))
        .with_children(|strip| {
            strip.spawn(text(
                ctx,
                11.0,
                MUTED,
                "Up / Down  Move      Left / Right  Choose      Enter / A  Select      Esc / B  Back",
            ));
        });
    });
}

// -------------------------------------------------------------------------------------------------
// Live.

/// The text of a dial's plate: its name and where it stands.
pub fn dial_label(
    strings: Option<&super::Strings>,
    dial: u8,
    hair_token: &str,
    facial_token: &str,
) -> String {
    let get = |key: &str, fallback: &str| -> String {
        strings
            .map_or(fallback, |s| s.text(key, fallback))
            .to_string()
    };
    match dial {
        0 => get("CHAR_CUSTOMIZATION1_DESC", "Skin Color"),
        1 => get("CHAR_CUSTOMIZATION2_DESC", "Face"),
        2 => get(&format!("HAIR_{hair_token}_STYLE"), "Hair Style"),
        3 => get(&format!("HAIR_{hair_token}_COLOR"), "Hair Color"),
        _ => get(&format!("FACIAL_HAIR_{facial_token}"), "Facial Hair"),
    }
}

/// The ability lines of a race: `ABILITY_INFO_<FILE>1`, `2`, ... until one is missing.
pub fn ability_lines(strings: &super::Strings, file: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut n = 1;
    while let Some(line) = strings.get(&format!("ABILITY_INFO_{file}{n}")) {
        lines.push(line.to_string());
        n += 1;
    }
    lines
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh(
    menu: Res<Menu>,
    data: Option<Res<CreateData>>,
    time: Res<Time<Real>>,
    mut icons: Query<(&Dyn, &mut ImageNode, &mut Node, &mut BorderColor)>,
    mut texts: Query<(&Dyn, &mut Text, &mut TextColor)>,
    mut shows: Query<(&Dyn, &mut Node), Without<ImageNode>>,
    mut fills: Query<(&Dyn, &mut BackgroundColor), Without<Hot>>,
    mut focus_rows: Query<(&FocusRow, &mut Outline)>,
) {
    let sel = &menu.sel;
    let rules = &menu.rules;
    let strings = data.as_deref().map(|d| &*d.strings);
    let catalog = data.as_deref().map(|d| &*d.catalog);
    let classes = rules.classes(sel.race);
    let alliance = rules::faction_of(sel.race) == Faction::Alliance;
    let file = catalog
        .and_then(|c| c.race_file(sel.race))
        .unwrap_or("Human")
        .to_uppercase();
    let hair_token = catalog
        .and_then(|c| c.hair_customization(sel.race))
        .unwrap_or("NORMAL")
        .to_string();
    let facial_token = catalog
        .and_then(|c| c.facial_hair_customization(sel.race, sel.sex))
        .unwrap_or("NORMAL")
        .to_string();
    let counts = rules.ranges(sel.race, sex_or_zero(sel.sex));
    let chosen_class_slot = sel.class_slot(rules);
    let focus = menu.focus();

    for (what, mut image, mut node, mut border) in &mut icons {
        let (tc, size, chosen, shown) = match *what {
            Dyn::RaceIcon(r) => (
                art::race_tc(r, sel.sex),
                art::RACES_SIZE,
                sel.race == r,
                true,
            ),
            Dyn::SexIcon(s) => (
                Some(art::gender_tc(s)),
                art::GENDER_SIZE,
                sel.sex == s,
                true,
            ),
            Dyn::ClassIcon(slot) => {
                let class = classes.get(usize::from(slot)).copied();
                (
                    class.and_then(art::class_tc),
                    art::CLASSES_SIZE,
                    chosen_class_slot == Some(usize::from(slot)),
                    class.is_some(),
                )
            }
            Dyn::InfoIcon(Info::Faction) => (
                Some(art::faction_tc(alliance)),
                art::FACTIONS_SIZE,
                false,
                true,
            ),
            Dyn::InfoIcon(Info::Race) => (
                art::race_tc(sel.race, sel.sex),
                art::RACES_SIZE,
                false,
                true,
            ),
            Dyn::InfoIcon(Info::Class) => {
                (art::class_tc(sel.class), art::CLASSES_SIZE, false, true)
            }
            _ => continue,
        };
        // A sex icon is half a 128 x 64 sheet: 64 x 64 cells, like the others.
        let rect = tc.map(|tc| art::rect(size, tc));
        if image.rect != rect {
            image.rect = rect;
        }
        let want_display = if shown { Display::Flex } else { Display::None };
        if node.display != want_display {
            node.display = want_display;
        }
        // A race the game cannot start yet is drawn grey: dark, with a quiet edge.
        let grey = matches!(*what, Dyn::RaceIcon(r) if !rules.race_choosable(r));
        let tint = if grey {
            GREYED
        } else if chosen {
            Color::WHITE
        } else {
            Color::srgb(0.72, 0.7, 0.66)
        };
        if image.color != tint && !matches!(*what, Dyn::InfoIcon(_)) {
            image.color = tint;
        }
        if !matches!(*what, Dyn::InfoIcon(_)) {
            let edge = if grey {
                GREYED_EDGE
            } else if chosen {
                GOLD
            } else {
                GOLD_DIM
            };
            *border = BorderColor::all(edge);
        }
    }

    for (what, mut value, mut color) in &mut texts {
        let (new, tint): (String, Option<Color>) = match *what {
            Dyn::RaceCaption(r) => (
                if sel.race == r {
                    rules::race_name(r).to_string()
                } else {
                    String::new()
                },
                None,
            ),
            Dyn::SexCaption(s) => (
                if sel.sex == s {
                    strings
                        .map_or(if s == 0 { "Male" } else { "Female" }, |t| {
                            t.text(
                                if s == 0 { "MALE" } else { "FEMALE" },
                                if s == 0 { "Male" } else { "Female" },
                            )
                        })
                        .to_string()
                } else {
                    String::new()
                },
                None,
            ),
            Dyn::ClassCaption(slot) => (
                if chosen_class_slot == Some(usize::from(slot)) {
                    classes
                        .get(usize::from(slot))
                        .map(|c| rules::class_name(*c).to_string())
                        .unwrap_or_default()
                } else {
                    String::new()
                },
                None,
            ),
            Dyn::DialLabel(d) => {
                let n = counts[usize::from(d)];
                let at = sel.dials[usize::from(d)] + 1;
                (
                    format!(
                        "{}  {at}/{n}",
                        dial_label(strings, d, &hair_token, &facial_token)
                    ),
                    None,
                )
            }
            Dyn::InfoTitle(Info::Faction) => (
                strings
                    .map_or(if alliance { "Alliance" } else { "Horde" }, |t| {
                        t.text(
                            if alliance { "ALLIANCE" } else { "HORDE" },
                            if alliance { "Alliance" } else { "Horde" },
                        )
                    })
                    .to_string(),
                None,
            ),
            Dyn::InfoTitle(Info::Race) => (rules::race_name(sel.race).to_string(), None),
            Dyn::InfoTitle(Info::Class) => (rules::class_name(sel.class).to_string(), None),
            // The strings open with eight spaces: the first line's clearance past the header icon.
            Dyn::InfoBody(Info::Faction) => (
                strings
                    .and_then(|t| {
                        t.get(if alliance {
                            "FACTION_INFO_ALLIANCE"
                        } else {
                            "FACTION_INFO_HORDE"
                        })
                    })
                    .unwrap_or("")
                    .to_string(),
                None,
            ),
            Dyn::InfoBody(Info::Race) => (
                strings
                    .and_then(|t| t.get(&format!("RACE_INFO_{file}")))
                    .unwrap_or("")
                    .to_string(),
                None,
            ),
            Dyn::InfoBody(Info::Class) => (
                strings
                    .and_then(|t| t.get(&format!("CLASS_{}", rules::class_file(sel.class))))
                    .unwrap_or("")
                    .to_string(),
                None,
            ),
            Dyn::InfoAbilities => (
                strings
                    .map(|t| ability_lines(t, &file).join("\n"))
                    .unwrap_or_default(),
                None,
            ),
            Dyn::NameText => {
                let caret = menu.focus() == Row::Name
                    && ((time.elapsed_secs() / BLINK) as u32).is_multiple_of(2);
                (
                    format!("{}{}", sel.name, if caret { "_" } else { " " }),
                    None,
                )
            }
            Dyn::Status => {
                if !menu.message.is_empty() {
                    (menu.message.clone(), Some(WARN))
                } else if !sel.playable() {
                    (
                        "Choose a male Human Warrior or Mage. Other characters are not available yet."
                            .to_string(),
                        Some(MUTED),
                    )
                } else {
                    (String::new(), Some(MUTED))
                }
            }
            Dyn::Create => {
                if !sel.playable() {
                    ("Unavailable".into(), Some(MUTED))
                } else if crate::character::check_name(&sel.name).is_err() {
                    ("Enter a name".into(), Some(MUTED))
                } else {
                    ("Create character".into(), Some(GOLD))
                }
            }
            _ => continue,
        };
        if value.0 != new {
            value.0 = new;
        }
        if let Some(tint) = tint {
            if color.0 != tint {
                color.0 = tint;
            }
        }
    }

    for (what, mut node) in &mut shows {
        let want = match *what {
            Dyn::DialRow(4) => {
                if rules.facial_hair_hidden(sel.race, sel.sex) {
                    Display::None
                } else {
                    Display::Flex
                }
            }
            Dyn::CaptionStrip(kind, index) => {
                let chosen = match kind {
                    0 => sel.race == index,
                    1 => sel.sex == index,
                    _ => chosen_class_slot == Some(usize::from(index)),
                };
                if chosen {
                    Display::Flex
                } else {
                    Display::None
                }
            }
            _ => continue,
        };
        if node.display != want {
            node.display = want;
        }
    }

    for (what, mut bg) in &mut fills {
        if *what == Dyn::PanelFill {
            let fill = if alliance { FILL_ALLIANCE } else { FILL_HORDE };
            if bg.0 != fill {
                bg.0 = fill;
            }
        }
    }

    for (row, mut outline) in &mut focus_rows {
        let on = row.0 == focus || matches!((row.0, focus), (Row::Dial(a), Row::Dial(b)) if a == b);
        let want = if on { IVORY } else { CLEAR };
        if outline.color != want {
            outline.color = want;
        }
    }
}

fn sex_or_zero(sex: u8) -> u8 {
    sex.min(1)
}

/// A click: pressed and released over the same button (the reference fires on the release).
fn clicks(
    buttons: Query<(Entity, &Interaction, &Act), Changed<Interaction>>,
    mut pressed: Local<Option<Entity>>,
    mut out: MessageWriter<MenuPress>,
) {
    for (entity, interaction, act) in &buttons {
        match interaction {
            Interaction::Pressed => *pressed = Some(entity),
            Interaction::Hovered => {
                if *pressed == Some(entity) {
                    *pressed = None;
                    if let Some(p) = act.press() {
                        out.write(MenuPress(p));
                    }
                }
            }
            Interaction::None => {
                if *pressed == Some(entity) {
                    *pressed = None;
                }
            }
        }
    }
}

/// The plates light under the pointer.
fn hot_plates(
    mut plates: Query<
        (&Interaction, &mut BackgroundColor, Option<&ImageNode>),
        (With<Hot>, With<Button>),
    >,
) {
    for (interaction, mut bg, image) in &mut plates {
        if image.is_some() {
            continue;
        }
        let want = if *interaction == Interaction::None {
            PLATE
        } else {
            PLATE_HOT
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// A scroll frame's furthest offset, layout pixels (`ComputedNode` sizes are physical).
fn max_scroll(node: &ComputedNode) -> f32 {
    ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0)
}

/// The wheel scrolls the info pane under the pointer.
fn wheel(
    mut wheel: MessageReader<MouseWheel>,
    mut panes: Query<(&Interaction, &ComputedNode, &mut ScrollPosition), With<InfoScroll>>,
) {
    for ev in wheel.read() {
        let dy = match ev.unit {
            MouseScrollUnit::Line => ev.y * 24.0,
            MouseScrollUnit::Pixel => ev.y,
        };
        for (interaction, node, mut pos) in &mut panes {
            if *interaction != Interaction::None {
                pos.0.y = (pos.0.y - dy).clamp(0.0, max_scroll(node));
            }
        }
    }
}

/// Turning the body: drag the 3D area, or hold a rotate button (also swaps its art while held).
fn rotate(
    mut acts: Query<(&Interaction, &Act, Option<&mut ImageNode>)>,
    art: Res<Art>,
    motion: Res<AccumulatedMouseMotion>,
    time: Res<Time<Real>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut menu: ResMut<Menu>,
) {
    let height = window.single().map_or(768.0, |w| w.height().max(1.0));
    for (interaction, act, image) in &mut acts {
        let held = *interaction == Interaction::Pressed;
        match act {
            Act::Model if held && motion.delta.x != 0.0 => {
                menu.sel.yaw += motion.delta.x * (768.0 / height) * ROTATION_PER_UNIT;
            }
            Act::RotateLeft if held => menu.sel.yaw -= ROTATE_RATE * time.delta_secs(),
            Act::RotateRight if held => menu.sel.yaw += ROTATE_RATE * time.delta_secs(),
            _ => {}
        }
        if matches!(act, Act::RotateLeft | Act::RotateRight) {
            if let Some(mut image) = image {
                let face = if held {
                    art.rotate_down.clone()
                } else {
                    art.rotate_up.clone()
                };
                if image.image != face {
                    image.image = face;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create::Strings;

    #[test]
    fn a_dial_plate_names_the_dial_from_the_race_token() {
        let s = Strings::from_pairs(&[
            ("CHAR_CUSTOMIZATION1_DESC", "Skin Color"),
            ("HAIR_NORMAL_STYLE", "Hair Style"),
            ("HAIR_HORNS_STYLE", "Horns"),
            ("HAIR_HORNS_COLOR", "Horn Color"),
            ("FACIAL_HAIR_TUSKS", "Tusks"),
        ]);
        assert_eq!(dial_label(Some(&s), 0, "NORMAL", "NORMAL"), "Skin Color");
        assert_eq!(dial_label(Some(&s), 2, "NORMAL", "NORMAL"), "Hair Style");
        assert_eq!(dial_label(Some(&s), 2, "HORNS", "NORMAL"), "Horns");
        assert_eq!(dial_label(Some(&s), 3, "HORNS", "NORMAL"), "Horn Color");
        assert_eq!(dial_label(Some(&s), 4, "NORMAL", "TUSKS"), "Tusks");
        assert_eq!(
            dial_label(None, 1, "NORMAL", "NORMAL"),
            "Face",
            "no strings"
        );
    }

    #[test]
    fn the_ability_lines_run_until_the_first_gap() {
        let s = Strings::from_pairs(&[
            ("ABILITY_INFO_HUMAN1", "a"),
            ("ABILITY_INFO_HUMAN2", "b"),
            ("ABILITY_INFO_HUMAN4", "d"),
            ("ABILITY_INFO_ORC1", "o"),
        ]);
        assert_eq!(ability_lines(&s, "HUMAN"), ["a", "b"]);
        assert_eq!(ability_lines(&s, "ORC"), ["o"]);
        assert!(ability_lines(&s, "TROLL").is_empty());
    }

    #[test]
    fn every_button_act_is_a_press_except_the_turning_ones() {
        assert_eq!(Act::Race(3).press(), Some(Press::Race(3)));
        assert_eq!(Act::Dial(2, -1).press(), Some(Press::Dial(2, -1)));
        assert_eq!(Act::Create.press(), Some(Press::Create));
        assert_eq!(Act::Back.press(), Some(Press::Back));
        assert_eq!(Act::Model.press(), None);
        assert_eq!(Act::RotateLeft.press(), None);
    }
}
