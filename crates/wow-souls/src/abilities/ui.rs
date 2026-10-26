//! The rage or mana bar and the four ability slots. The bar is a thin one under the stamina bar and as
//! long as it ([`crate::hud`] places both from the Dark Souls layout): red for a warrior's rage, blue for
//! a mage's mana. The slots are a row of icons at the bottom left, beside the pouch: the selected one has
//! a gold edge, one the power cannot pay for is dim, and the keys 1 to 4 are written on them.

use bevy::prelude::*;

use wow_world::abilities;
use wow_world::class::Power;

use super::{AbilityArt, ActiveClass, Buffs, Cooldowns, Hotbar, Mana, Rage};
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
const FILL_RAGE: Color = Color::srgb(0.78, 0.2, 0.12);
const FILL_MANA: Color = Color::srgb(0.2, 0.42, 0.85);
const DIM: Color = Color::srgb(0.31, 0.27, 0.19);
const CELL: f32 = 44.0;

/// What the bar shows for `power`: the current value and the most there is (never 0).
pub fn bar_of(power: Power, rage: &Rage, mana: &Mana) -> (f32, f32) {
    match power {
        Power::Rage => (rage.cur, abilities::RAGE_MAX as f32),
        Power::Mana => (mana.cur, mana.max.max(1.0)),
    }
}

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
    app.add_systems(
        OnEnter(Phase::Playing),
        (
            spawn_bar.run_if(not(resource_exists::<crate::hud::VitalsArt>)),
            spawn_slots,
            spawn_buffs,
        ),
    )
    .add_systems(
        Update,
        (
            update_bar.run_if(not(resource_exists::<crate::hud::VitalsArt>)),
            update_slots,
            update_buffs,
        )
            .run_if(in_state(Phase::Playing)),
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
                BackgroundColor(FILL_RAGE),
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
                    border: UiRect::all(px(1)),
                    padding: UiRect::all(px(2)),
                    display: Display::None,
                    ..default()
                },
                BorderColor::all(DIM),
                BackgroundColor(Color::srgba(0.02, 0.018, 0.014, 0.7)),
                Cell(i),
                crate::hud::theme::Surface::Slot(false),
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
            bottom: px(-19),
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

#[allow(clippy::type_complexity)]
fn update_bar(
    art: Option<Res<HudArt>>,
    active: Res<ActiveClass>,
    (rage, mana): (Res<Rage>, Res<Mana>),
    avatar: Query<&Stamina, With<Avatar>>,
    mut backing: Query<&mut Node, (With<Backing>, Without<Fill>)>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), (With<Fill>, Without<Backing>)>,
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
    if let Ok((mut n, mut color)) = fill.single_mut() {
        let (cur, max) = bar_of(active.power, &rage, &mana);
        let want = percent((cur / max * 100.0).clamp(0.0, 100.0));
        if n.width != want {
            n.width = want;
        }
        let tint = match active.power {
            Power::Rage => FILL_RAGE,
            Power::Mana => FILL_MANA,
        };
        if color.0 != tint {
            color.0 = tint;
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_slots(
    bar: Res<Hotbar>,
    active: Res<ActiveClass>,
    (rage, mana): (Res<Rage>, Res<Mana>),
    cooldowns: Res<Cooldowns>,
    art: Option<Res<AbilityArt>>,
    screen: Res<crate::loot::Screen>,
    mut cells: Query<
        (
            &Cell,
            &mut Node,
            &mut BorderColor,
            &mut crate::hud::theme::Surface,
        ),
        Without<Pic>,
    >,
    mut pics: Query<(&Pic, &mut ImageNode)>,
    mut waits: Query<(&Wait, &mut Text), Without<Pic>>,
) {
    let Some(art) = art else {
        return;
    };
    for (cell, mut node, mut border, mut surface) in &mut cells {
        let skin = crate::hud::theme::Surface::Slot(cell.0 == bar.selected);
        if *surface != skin {
            *surface = skin;
        }
        let spell = bar.slots.get(cell.0).copied();
        let display = if spell.is_some() && !screen.open && !screen.modal {
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
        let cost = abilities::find(spell).map_or(0, |a| a.cost) as f32;
        // Dim while the power is short or the ability is not ready.
        let have = bar_of(active.power, &rage, &mana).0;
        let tint = if have >= cost && cooldowns.left(spell) <= 0.0 {
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

/// The buffs row: an icon per running buff under the status meters (where Elden Ring keeps its own),
/// the time left written under it, and the icon blinking in its last [`BLINK_SECS`].
const BUFF_CELL: f32 = 30.0;
const BUFF_LEFT: f32 = 96.0;
const BUFF_TOP: f32 = 74.0;
const BLINK_SECS: f32 = 5.0;
/// Battle Shout and Frost Armor, the buffs [`Buffs`] keeps.
const BUFF_SLOTS: usize = 3;

#[derive(Component, Clone, Copy)]
struct BuffCell(usize);
#[derive(Component, Clone, Copy)]
struct BuffPic(usize);
#[derive(Component, Clone, Copy)]
struct BuffLeft(usize);

/// The running buffs in the row's order, as `(spell, seconds left)`.
pub fn running(buffs: &Buffs) -> Vec<(u32, f32)> {
    [(6673, buffs.shout), (168, buffs.frost_armor)]
        .into_iter()
        .filter(|&(_, left)| left > 0.0)
        .collect()
}

/// The time left as WoW writes it on a buff: whole minutes from a minute up, seconds below.
pub fn left_text(secs: f32) -> String {
    let s = secs.ceil().max(0.0) as u32;
    if s >= 60 {
        format!("{}m", s.div_ceil(60))
    } else {
        format!("{s}s")
    }
}

fn spawn_buffs(mut commands: Commands, fonts: Res<Fonts>) {
    for i in 0..BUFF_SLOTS {
        let cell = commands
            .spawn((
                Name::new("hud buff"),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(BUFF_LEFT + i as f32 * (BUFF_CELL + 8.0)),
                    top: px(BUFF_TOP),
                    width: px(BUFF_CELL),
                    height: px(BUFF_CELL),
                    border: UiRect::all(px(1)),
                    display: Display::None,
                    ..default()
                },
                BorderColor::all(EDGE),
                BackgroundColor(PLATE),
                GlobalZIndex(10),
                BuffCell(i),
            ))
            .id();
        commands.spawn((
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            ImageNode::default(),
            BuffPic(i),
            ChildOf(cell),
        ));
        let left = plain(&mut commands, cell, "", &fonts.body, 12.0, IVORY);
        commands.entity(left).insert((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                width: px(BUFF_CELL),
                bottom: px(-16),
                justify_content: JustifyContent::Center,
                ..default()
            },
            TextLayout::new(Justify::Center, LineBreak::NoWrap),
            BuffLeft(i),
        ));
    }
}

#[allow(clippy::type_complexity)]
fn update_buffs(
    buffs: Res<Buffs>,
    warrior: Res<crate::warrior::Warrior>,
    art: Option<Res<AbilityArt>>,
    screen: Res<crate::loot::Screen>,
    time: Res<Time>,
    mut cells: Query<(&BuffCell, &mut Node)>,
    mut pics: Query<(&BuffPic, &mut ImageNode)>,
    mut lefts: Query<(&BuffLeft, &mut Text)>,
) {
    let mut on = running(&buffs);
    if warrior.wall > 0.0 {
        on.push((crate::warrior::SHIELD_WALL, warrior.wall));
    }
    let shown = !screen.open && !screen.modal;
    for (cell, mut node) in &mut cells {
        let display = if shown && cell.0 < on.len() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    let Some(art) = art else {
        return;
    };
    // Two blinks a second, as a WoW buff about to fade.
    let blink = (time.elapsed_secs() * std::f32::consts::TAU * 2.0).sin() * 0.35 + 0.65;
    for (pic, mut image) in &mut pics {
        let Some(&(spell, left)) = on.get(pic.0) else {
            continue;
        };
        if let Some(h) = art.0.get(&spell) {
            if image.image != *h {
                image.image = h.clone();
            }
        }
        let alpha = if left <= BLINK_SECS { blink } else { 1.0 };
        let tint = Color::srgba(1.0, 1.0, 1.0, alpha);
        if image.color != tint {
            image.color = tint;
        }
    }
    for (slot, mut text) in &mut lefts {
        let want = on.get(slot.0).map_or(String::new(), |&(_, s)| left_text(s));
        if text.0 != want {
            text.0 = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_buff_row_lists_running_buffs_with_their_time() {
        let mut b = Buffs::default();
        assert!(running(&b).is_empty());
        b.shout = 95.0;
        assert_eq!(running(&b), vec![(6673, 95.0)]);
        b.frost_armor = 4.2;
        assert_eq!(running(&b), vec![(6673, 95.0), (168, 4.2)]);
        b.shout = 0.0;
        assert_eq!(
            running(&b),
            vec![(168, 4.2)],
            "an ended buff leaves the row"
        );
        assert_eq!(left_text(120.0), "2m");
        assert_eq!(left_text(95.0), "2m");
        assert_eq!(left_text(60.0), "1m");
        assert_eq!(left_text(59.2), "1m", "59.2 s left rounds up to a minute");
        assert_eq!(left_text(58.5), "59s");
        assert_eq!(left_text(4.2), "5s");
    }

    #[test]
    fn the_bar_shows_the_class_power() {
        let rage = Rage {
            cur: 40.0,
            ..Rage::default()
        };
        let mut mana = Mana::default();
        mana.sync(165.0, 22.0);
        mana.cur = 80.0;
        assert_eq!(bar_of(Power::Rage, &rage, &mana), (40.0, 100.0));
        assert_eq!(bar_of(Power::Mana, &rage, &mana), (80.0, 165.0));
        assert_eq!(
            bar_of(Power::Mana, &rage, &Mana::default()).1,
            1.0,
            "never divides by zero"
        );
    }
}
