//! The plain HUD: Souls-style bars made of coloured rectangles, used when the Dark Souls art
//! native themed HUD is disabled. Health and stamina at the top left, their length the
//! stat's maximum and their fill its current value; the struck enemy's bar follows it on screen
//! with the damage run beside it.

use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::settle::Phase;

/// Which stat a bar fill shows.
#[derive(Component, Clone, Copy)]
enum BarFill {
    Health,
    Stamina,
}

/// The struck enemy's bar, which follows it on screen: the fill, and the damage dealt in the
/// last moments beside it, as Souls shows it.
#[derive(Component)]
struct EnemyBar;

#[derive(Component)]
struct EnemyFill;

#[derive(Component)]
struct EnemyDamage;

/// Who the enemy bar follows and the damage run it shows.
#[derive(Resource, Debug, Default)]
struct EnemyBarState {
    target: Option<Entity>,
    /// Damage summed over hits that land within [`DAMAGE_RUN`] of each other.
    run: f32,
    last_hit: f32,
}

/// Hits closer together than this (s) add up in the shown number.
const DAMAGE_RUN: f32 = 1.5;
/// The bar stays this long (s) after the last hit, and after a death.
const BAR_LINGER: f32 = 4.0;
const ENEMY_BAR_WIDTH: f32 = 90.0;

/// Screen pixels per point of a stat, so a bigger maximum is a longer bar, as in Souls.
const PX_PER_POINT: f32 = 3.0;
const BAR_HEIGHT: f32 = 9.0;

pub fn plugin(app: &mut App) {
    // Only without the Dark Souls art (`hud::art::HudArt`).
    let plain = not(resource_exists::<super::art::HudArt>).and(not(super::theme::enabled));
    app.add_systems(
        OnEnter(Phase::Playing),
        (
            spawn_bars.run_if(not(resource_exists::<super::VitalsArt>)),
            spawn_enemy_bar,
        )
            .run_if(plain.clone()),
    )
    .init_resource::<EnemyBarState>()
    .add_systems(
        PostUpdate,
        update_enemy_bar
            .after(bevy::transform::TransformSystems::Propagate)
            .run_if(in_state(Phase::Playing))
            .run_if(plain.clone()),
    )
    .add_systems(
        Update,
        update_bars
            .run_if(in_state(Phase::Playing))
            .run_if(plain)
            .run_if(not(resource_exists::<super::VitalsArt>)),
    );
}

fn spawn_bars(mut commands: Commands) {
    let bar = |commands: &mut Commands, parent: Entity, fill: BarFill, color: Color, max: f32| {
        let frame = commands
            .spawn((
                Node {
                    width: px(max * PX_PER_POINT),
                    height: px(BAR_HEIGHT),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.75, 0.65, 0.45, 0.9)),
                BackgroundColor(Color::srgba(0.05, 0.04, 0.03, 0.75)),
                ChildOf(parent),
            ))
            .id();
        commands.spawn((
            fill,
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(color),
            ChildOf(frame),
        ));
    };
    let column = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(24),
            top: px(34),
            flex_direction: FlexDirection::Column,
            row_gap: px(5),
            ..default()
        })
        .id();
    bar(
        &mut commands,
        column,
        BarFill::Health,
        Color::srgb(0.62, 0.08, 0.06),
        crate::combat::HEALTH_MAX,
    );
    bar(
        &mut commands,
        column,
        BarFill::Stamina,
        Color::srgb(0.18, 0.52, 0.16),
        crate::combat::STAMINA_MAX,
    );
}

fn spawn_enemy_bar(mut commands: Commands) {
    let root = commands
        .spawn((
            EnemyBar,
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(6),
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    let frame = commands
        .spawn((
            Node {
                width: px(ENEMY_BAR_WIDTH),
                height: px(6),
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(Color::srgba(0.75, 0.65, 0.45, 0.9)),
            BackgroundColor(Color::srgba(0.05, 0.04, 0.03, 0.75)),
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        EnemyFill,
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(Color::srgb(0.62, 0.08, 0.06)),
        ChildOf(frame),
    ));
    commands.spawn((
        EnemyDamage,
        Text::new(""),
        TextFont::from_font_size(13.0),
        TextColor(Color::srgb(0.95, 0.9, 0.8)),
        ChildOf(root),
    ));
}

/// Follows the struck enemy on screen with its health and the damage run.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_enemy_bar(
    real: Res<Time<bevy::time::Real>>,
    mut hits: MessageReader<crate::hits::Hit>,
    mut state: ResMut<EnemyBarState>,
    targets: Query<(&GlobalTransform, &crate::combat::Health)>,
    cams: Query<(&Camera, &GlobalTransform), With<benilla_world::view::WorldCamera>>,
    mut bar: Query<(&mut Node, &mut Visibility), (With<EnemyBar>, Without<EnemyFill>)>,
    mut fill: Query<&mut Node, (With<EnemyFill>, Without<EnemyBar>)>,
    mut text: Query<&mut Text, With<EnemyDamage>>,
) {
    let now = real.elapsed_secs();
    for h in hits.read() {
        if state.target != Some(h.target) || now - state.last_hit > DAMAGE_RUN {
            state.run = 0.0;
        }
        state.target = Some(h.target);
        state.run += h.damage;
        state.last_hit = now;
    }
    let (Ok((mut node, mut vis)), Ok(mut fill), Ok(mut text)) =
        (bar.single_mut(), fill.single_mut(), text.single_mut())
    else {
        return;
    };
    // A living enemy's bar lingers after the last hit; a dead one's goes with its damage number.
    let shown = state
        .target
        .and_then(|t| targets.get(t).ok())
        .filter(|(_, hp)| {
            let since = now - state.last_hit;
            since < if hp.cur > 0.0 { BAR_LINGER } else { DAMAGE_RUN }
        });
    let screen = shown.and_then(|(at, _)| {
        let (cam, cam_xf) = cams.single().ok()?;
        cam.world_to_viewport(cam_xf, at.translation() + Vec3::Y * 1.8)
            .ok()
    });
    let (Some((_, hp)), Some(px_at)) = (shown, screen) else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    };
    *vis = Visibility::Inherited;
    node.left = px(px_at.x - ENEMY_BAR_WIDTH * 0.5);
    node.top = px(px_at.y);
    fill.width = percent((hp.cur / hp.max).clamp(0.0, 1.0) * 100.0);
    let number = if now - state.last_hit < DAMAGE_RUN {
        format!("{:.0}", state.run)
    } else {
        String::new()
    };
    if text.0 != number {
        text.0 = number;
    }
}

fn update_bars(
    avatar: Query<(&crate::combat::Health, &crate::combat::Stamina), With<Avatar>>,
    mut fills: Query<(&BarFill, &mut Node)>,
) {
    let Ok((hp, st)) = avatar.single() else {
        return;
    };
    for (fill, mut node) in &mut fills {
        let frac = match fill {
            BarFill::Health => hp.cur / hp.max,
            BarFill::Stamina => st.cur / st.max,
        };
        let want = percent(frac.clamp(0.0, 1.0) * 100.0);
        if node.width != want {
            node.width = want;
        }
    }
}
