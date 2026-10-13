//! The rage bar and the four ability slots. The bar is a thin red one under the stamina bar and as long
//! as it ([`crate::hud`] places both from the Dark Souls layout); the slots are a row of icons at the
//! bottom left, beside the pouch: the selected one has a gold edge, one the rage cannot pay for is dim,
//! and the keys 1 to 4 are written on them.

use bevy::prelude::*;

use wow_world::abilities;

use super::{AbilityArt, Cooldowns, Hotbar, Rage};
use crate::avatar::Avatar;
use crate::combat::Stamina;
use crate::hud::art::HudArt;
use crate::hud::{node_at, Anchor, LRect};
use crate::loot::ui::{plain, Fonts, GOLD, IVORY};
use crate::settle::Phase;

/// Layout pixels between the stamina plate and the rage bar, and the bar's height.
const GAP: f32 = 5.0;
const HEIGHT: f32 = 5.0;
const PLATE: Color = Color::srgba(0.02, 0.018, 0.014, 0.82);
const EDGE: Color = Color::srgba(0.42, 0.34, 0.2, 0.9);
const FILL: Color = Color::srgb(0.78, 0.2, 0.12);
const DIM: Color = Color::srgb(0.31, 0.27, 0.19);
const CELL: f32 = 44.0;

#[derive(Component)]
struct Backing;
#[derive(Component)]
struct Fill;
#[derive(Component, Clone, Copy)]
struct Cell(usize);
#[derive(Component, Clone, Copy)]
struct Pic(usize);
#[derive(Component, Clone, Copy)]
struct Wait(usize);

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Phase::Playing), (spawn_bar, spawn_slots))
        .add_systems(
            Update,
            (update_bar, update_slots).run_if(in_state(Phase::Playing)),
        );
}

fn plate(art: &HudArt) -> LRect {
    art.m.fill("F20-00/plate_sp").rect
}

fn spawn_bar(mut commands: Commands, art: Option<Res<HudArt>>) {
    let Some(art) = art else {
        return;
    };
    let p = plate(&art);
    let top = p.b + GAP;
    let field = LRect {
        l: p.l,
        t: top,
        r: p.l + 1.0,
        b: top + HEIGHT,
    };
    let mut node = node_at(field, Anchor::Left);
    node.border = UiRect::all(px(1));
    commands
        .spawn((
            Name::new("hud rage"),
            node,
            BackgroundColor(PLATE),
            BorderColor::all(EDGE),
            Backing,
        ))
        .with_children(|c| {
            c.spawn((
                Node {
                    width: percent(0),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(FILL),
                Fill,
            ));
        });
}

fn spawn_slots(mut commands: Commands, fonts: Res<Fonts>) {
    for i in 0..4 {
        let cell = commands
            .spawn((
                Name::new("hud ability slot"),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(150.0 + i as f32 * (CELL + 6.0)),
                    top: px(596),
                    width: px(CELL),
                    height: px(CELL),
                    border: UiRect::all(px(2)),
                    display: Display::None,
                    ..default()
                },
                BorderColor::all(DIM),
                BackgroundColor(Color::srgba(0.02, 0.018, 0.014, 0.7)),
                Cell(i),
            ))
            .id();
        commands.spawn((
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            ImageNode::default(),
            Pic(i),
            ChildOf(cell),
        ));
        let key = plain(
            &mut commands,
            cell,
            &(i + 1).to_string(),
            &fonts.body,
            13.0,
            IVORY,
        );
        commands.entity(key).insert(Node {
            position_type: PositionType::Absolute,
            left: px(2),
            top: px(0),
            ..default()
        });
        // The seconds left of its cooldown, over the icon.
        let wait = plain(&mut commands, cell, "", &fonts.body, 18.0, IVORY);
        commands.entity(wait).insert((
            Node {
                position_type: PositionType::Absolute,
                left: px(10),
                top: px(10),
                ..default()
            },
            Wait(i),
        ));
    }
}

fn update_bar(
    art: Option<Res<HudArt>>,
    rage: Res<Rage>,
    avatar: Query<&Stamina, With<Avatar>>,
    mut backing: Query<&mut Node, (With<Backing>, Without<Fill>)>,
    mut fill: Query<&mut Node, (With<Fill>, Without<Backing>)>,
) {
    let (Some(art), Ok(stamina)) = (art, avatar.single()) else {
        return;
    };
    let len = crate::hud::bar_length(stamina.max, plate(&art).w());
    if let Ok(mut n) = backing.single_mut() {
        if n.width != px(len) {
            n.width = px(len);
        }
    }
    if let Ok(mut n) = fill.single_mut() {
        let want = percent((rage.cur / abilities::RAGE_MAX as f32 * 100.0).clamp(0.0, 100.0));
        if n.width != want {
            n.width = want;
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_slots(
    bar: Res<Hotbar>,
    rage: Res<Rage>,
    cooldowns: Res<Cooldowns>,
    art: Option<Res<AbilityArt>>,
    mut cells: Query<(&Cell, &mut Node, &mut BorderColor), Without<Pic>>,
    mut pics: Query<(&Pic, &mut ImageNode)>,
    mut waits: Query<(&Wait, &mut Text), Without<Pic>>,
) {
    let Some(art) = art else {
        return;
    };
    for (cell, mut node, mut border) in &mut cells {
        let spell = bar.slots.get(cell.0).copied();
        let display = if spell.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
        let want = if cell.0 == bar.selected { GOLD } else { DIM };
        if border.top != want {
            *border = BorderColor::all(want);
        }
    }
    for (pic, mut image) in &mut pics {
        let Some(spell) = bar.slots.get(pic.0).copied() else {
            continue;
        };
        if let Some(h) = art.0.get(&spell) {
            if image.image != *h {
                image.image = h.clone();
            }
        }
        let cost = abilities::find(spell).map_or(0, |a| a.rage) as f32;
        // Dim while the rage is short or the ability is not ready.
        let tint = if rage.cur >= cost && cooldowns.left(spell) <= 0.0 {
            Color::WHITE
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.4)
        };
        if image.color != tint {
            image.color = tint;
        }
    }
    for (wait, mut text) in &mut waits {
        let left = bar
            .slots
            .get(wait.0)
            .map_or(0.0, |&spell| cooldowns.left(spell));
        let want = if left > 0.0 {
            format!("{:.0}", left.ceil())
        } else {
            String::new()
        };
        if text.0 != want {
            text.0 = want;
        }
    }
}
