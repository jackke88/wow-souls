//! The loading cover: our own plain screen (not the WoW UI) over the world while the spawn hold
//! streams Northshire in, with the engine's load fraction. It goes when the hold releases.

use bevy::prelude::*;

use benilla_world::terrain_stream::WorldLoadProgress;

use crate::settle::Phase;

#[derive(Component)]
struct LoadingCover;

#[derive(Component)]
struct LoadingLine;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Phase::Streaming), spawn_cover)
        .add_systems(Update, update_line.run_if(in_state(Phase::Streaming)))
        .add_systems(OnExit(Phase::Streaming), despawn_cover);
}

fn spawn_cover(mut commands: Commands) {
    commands.spawn((
        LoadingCover,
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: px(12),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        // Above anything else this game draws.
        GlobalZIndex(i32::MAX),
        children![
            (
                Text::new("Northshire Valley"),
                TextFont::from_font_size(40.0),
                TextColor(Color::srgb(0.85, 0.78, 0.6)),
            ),
            (
                LoadingLine,
                Text::new("loading"),
                TextFont::from_font_size(18.0),
                TextColor(Color::srgb(0.55, 0.55, 0.55)),
            ),
        ],
    ));
}

fn update_line(
    progress: Option<Res<WorldLoadProgress>>,
    mut line: Query<&mut Text, With<LoadingLine>>,
) {
    let (Some(p), Ok(mut text)) = (progress, line.single_mut()) else {
        return;
    };
    let wanted = format!("loading {:>3.0}%", p.fraction() * 100.0);
    if text.0 != wanted {
        text.0 = wanted;
    }
}

fn despawn_cover(mut commands: Commands, cover: Query<Entity, With<LoadingCover>>) {
    for e in &cover {
        commands.entity(e).despawn();
    }
}
