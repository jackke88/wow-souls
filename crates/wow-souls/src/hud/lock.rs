//! The lock-on reticle: Dark Souls' own sprite over the locked target's aim point (the plain
//! gizmo dot in `lockon.rs` stays only without the art). The layout fits the 102-pixel dialog of
//! the cursor into a 46-pixel place in the HUD, so the ring is about 34 layout pixels across.

use bevy::prelude::*;

use super::art::HudArt;
use super::enemy::to_layout;
use super::sprite_tint;
use crate::lockon::{locked_aim, LockOn, Lockable};
use crate::settle::Phase;

#[derive(Component)]
struct Reticle {
    /// The ring's size on screen, layout px.
    size: Vec2,
}

/// The top-left corner that centres a box of `size` on `point`.
pub fn centred(point: Vec2, size: Vec2) -> Vec2 {
    point - size * 0.5
}

pub fn plugin(app: &mut App) {
    let art = resource_exists::<HudArt>;
    app.add_systems(OnEnter(Phase::Playing), spawn.run_if(art))
        .add_systems(
            PostUpdate,
            update
                .after(bevy::transform::TransformSystems::Propagate)
                .run_if(in_state(Phase::Playing))
                .run_if(art),
        );
}

fn spawn(mut commands: Commands, art: Res<HudArt>) {
    let key = "F20_LockCursor/lock";
    let s = art.m.sprite(key);
    let (dialog, place) = (
        art.m.dialog("F20_LockCursor"),
        art.m.place("F20-00/F20_LockCursor"),
    );
    let fit = place.w() / dialog.w();
    let size = Vec2::new(s.rect.w() * fit, s.rect.h() * fit);
    commands.spawn((
        Name::new("lock reticle"),
        Reticle { size },
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(art.sprite_image(key)).with_color(sprite_tint(s.tint)),
        Visibility::Hidden,
    ));
}

fn update(
    lock: Res<LockOn>,
    targets: Query<(&GlobalTransform, &Lockable)>,
    cams: Query<(&Camera, &GlobalTransform), With<benilla_world::view::WorldCamera>>,
    ui_scale: Res<UiScale>,
    mut reticle: Query<(&Reticle, &mut Node, &mut Visibility)>,
) {
    let Ok((r, mut node, mut vis)) = reticle.single_mut() else {
        return;
    };
    let screen = locked_aim(&lock, &targets).and_then(|aim| {
        let (cam, xf) = cams.single().ok()?;
        cam.world_to_viewport(xf, aim)
            .ok()
            .map(|p| to_layout(p, ui_scale.0))
    });
    match screen {
        Some(p) => {
            let at = centred(p, r.size);
            node.left = px(at.x);
            node.top = px(at.y);
            if *vis != Visibility::Inherited {
                *vis = Visibility::Inherited;
            }
        }
        None => {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reticle_is_centred_on_the_point() {
        let at = centred(Vec2::new(640.0, 360.0), Vec2::new(34.0, 34.0));
        assert_eq!(at, Vec2::new(623.0, 343.0));
    }
}
