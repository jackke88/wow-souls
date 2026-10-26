//! Native Elden Ring reference HUD: thin unequal meters, quiet level seal and separate XP line.
//! Values and layout are native; small embedded material sheets add wear without DSR HUD art.
use super::bars::{fraction, Chip};
use super::theme;
use super::xp::{Glide, Reading};
use crate::abilities::{ActiveClass, Mana, Rage};
use crate::avatar::Avatar;
use crate::combat::{Health, Stamina};
use crate::loot::ui::Fonts;
use crate::quests::QuestGame;
use crate::settle::Phase;
use bevy::prelude::*;
use wow_world::class::Power;

#[derive(Resource)]
pub(crate) struct VitalsArt;
#[derive(Component)]
struct Root;
#[derive(Component, Clone, Copy)]
enum Row {
    Health,
    Power,
    Stamina,
    Xp,
}
#[derive(Component)]
struct Track(Row);
#[derive(Component)]
struct Fill {
    row: Row,
    chip: bool,
}
#[derive(Component)]
enum Label {
    Level,
    Xp,
}
#[derive(Resource)]
struct State {
    health: Chip,
    stamina: Chip,
    xp: Option<Glide>,
    reading: Option<Reading>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            health: Chip::new(0.0),
            stamina: Chip::new(0.0),
            xp: None,
            reading: None,
        }
    }
}
fn color(row: Row, power: Power) -> Color {
    match row {
        Row::Health => Color::srgb(0.55, 0.115, 0.11),
        Row::Power if power == Power::Mana => Color::srgb(0.18, 0.33, 0.46),
        Row::Power => Color::srgb(0.64, 0.29, 0.105),
        Row::Stamina => Color::srgb(0.28, 0.41, 0.20),
        Row::Xp => Color::srgb(0.53, 0.49, 0.30),
    }
}
fn length(row: Row, max: f32) -> f32 {
    match row {
        Row::Health => (135.0 + max * 0.36).clamp(150.0, 390.0),
        Row::Power => (90.0 + max * 0.23).clamp(120.0, 330.0),
        Row::Stamina => (105.0 + max * 0.65).clamp(150.0, 300.0),
        Row::Xp => 420.0,
    }
}
fn load(mut commands: Commands) {
    if theme::enabled() {
        commands.insert_resource(VitalsArt);
        info!("hud vitals: native Elden Ring reference meters with embedded material trim");
    }
}
fn spawn(mut commands: Commands, fonts: Res<Fonts>, active: Res<ActiveClass>) {
    let root = commands
        .spawn((
            Name::new("native status HUD"),
            Root,
            Visibility::Inherited,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(10),
        ))
        .id();
    let seal = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(34),
                top: px(22),
                width: px(46),
                height: px(52),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            theme::Surface::Seal,
            ChildOf(root),
        ))
        .id();
    let level = theme::words(&mut commands, seal, "", &fonts.body, 19.0, Node::default());
    commands.entity(level).insert(Label::Level);
    for (row, y, h) in [
        (Row::Health, 31.0, 9.0),
        (Row::Power, 44.5, 7.5),
        (Row::Stamina, 58.0, 7.5),
        (Row::Xp, 0.0, 3.0),
    ] {
        let node = if matches!(row, Row::Xp) {
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                margin: UiRect::left(px(-210)),
                bottom: px(20),
                width: px(420),
                height: px(h),
                ..default()
            }
        } else {
            Node {
                position_type: PositionType::Absolute,
                left: px(96),
                top: px(y),
                width: px(200),
                height: px(h),
                ..default()
            }
        };
        let (track, chip, fill) = theme::meter(&mut commands, root, node, color(row, active.power));
        commands.entity(track).insert(Track(row));
        commands.entity(fill).insert(Fill { row, chip: false });
        if matches!(row, Row::Health | Row::Stamina) {
            commands.entity(chip).insert(Fill { row, chip: true });
        } else {
            commands.entity(chip).despawn();
        }
    }
    let xp = theme::words(
        &mut commands,
        root,
        "",
        &fonts.body,
        11.0,
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            margin: UiRect::left(px(-210)),
            bottom: px(28),
            width: px(420),
            justify_content: JustifyContent::Center,
            ..default()
        },
    );
    commands.entity(xp).insert((
        Label::Xp,
        TextLayout::new(Justify::Center, LineBreak::NoWrap),
        TextColor(theme::MUTED),
    ));
}
#[allow(clippy::too_many_arguments)]
fn update(
    real: Res<Time<Real>>,
    active: Res<ActiveClass>,
    rage: Res<Rage>,
    mana: Res<Mana>,
    game: Option<Res<QuestGame>>,
    avatar: Query<(&Health, &Stamina), With<Avatar>>,
    mut state: ResMut<State>,
    mut tracks: Query<(&Track, &mut Node), Without<Fill>>,
    mut fills: Query<(
        &Fill,
        &mut Node,
        &mut BackgroundGradient,
        &mut theme::Surface,
    )>,
    mut labels: Query<(&Label, &mut Text)>,
    screen: Res<crate::loot::Screen>,
    log: Res<crate::quests::LogScreen>,
    mut root: Query<&mut Visibility, With<Root>>,
) {
    for mut vis in &mut root {
        *vis = if screen.open || log.open {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    let Ok((hp, sp)) = avatar.single() else {
        return;
    };
    let xp = game
        .map(|g| {
            let (level, into, need) = g.table.xp_into_level(g.xp);
            Reading { level, into, need }
        })
        .unwrap_or(Reading {
            level: 1,
            into: 0,
            need: 400,
        });
    let (power, power_max) = match active.power {
        Power::Mana => (mana.cur, mana.max),
        Power::Rage => (rage.cur, wow_world::abilities::RAGE_MAX as f32),
    };
    state
        .health
        .step(hp.cur, hp.max, real.delta_secs(), 0.55, 1.6);
    state
        .stamina
        .step(sp.cur, sp.max, real.delta_secs(), 0.25, 3.0);
    let glide = state.xp.get_or_insert_with(|| Glide::new(xp));
    glide.step(xp, real.delta_secs());
    let shown_xp = glide.shown;
    for (track, mut node) in &mut tracks {
        let max = match track.0 {
            Row::Health => hp.max,
            Row::Power => power_max,
            Row::Stamina => sp.max,
            Row::Xp => 1.0,
        };
        let width = px(length(track.0, max));
        if node.width != width {
            node.width = width;
        }
    }
    for (fill, mut node, mut gradient, mut surface) in &mut fills {
        let f = match fill.row {
            Row::Health => fraction(
                if fill.chip {
                    state.health.shown
                } else {
                    hp.cur
                },
                hp.max,
            ),
            Row::Power => fraction(power, power_max),
            Row::Stamina => fraction(
                if fill.chip {
                    state.stamina.shown
                } else {
                    sp.cur
                },
                sp.max,
            ),
            Row::Xp => shown_xp,
        };
        if node.width != percent(f * 100.0) {
            node.width = percent(f * 100.0);
        }
        if !fill.chip {
            let tint = color(fill.row, active.power);
            let want = theme::fill_gradient(tint);
            if *gradient != want {
                *gradient = want;
            }
            let want = theme::Surface::Fill(tint);
            if *surface != want {
                *surface = want;
            }
        }
    }
    if state.reading == Some(xp) {
        return;
    }
    for (label, mut text) in &mut labels {
        let want = match label {
            Label::Level => xp.level.to_string(),
            Label::Xp if xp.need == 0 => format!("Level {}", xp.level),
            Label::Xp => format!("Level {}     ·     {} / {} XP", xp.level, xp.into, xp.need),
        };
        if text.0 != want {
            text.0 = want;
        }
    }
    state.reading = Some(xp);
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(Startup, load)
        .add_systems(
            OnEnter(Phase::Playing),
            spawn.run_if(resource_exists::<VitalsArt>),
        )
        .add_systems(
            Update,
            update
                .run_if(in_state(Phase::Playing))
                .run_if(resource_exists::<VitalsArt>),
        );
}
