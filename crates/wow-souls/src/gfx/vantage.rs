//! `WOWSOULS_VANTAGE=<ex>,<ey>,<ez>,<lx>,<ly>,<lz>`: park the free camera at an eye point looking
//! at a target (raw WoW coordinates, +X north, +Y west), for lane-by-lane screenshots from a fixed
//! framing (the fork's capture scenarios name such points: `OVERLOOK_EYE`, `GROUND_EYE`...).
//! The world streams around the *player*, so set `WOWSOULS_AT` near the eye.
//! With `WOWSOULS_VANTAGE_REL=1` the six numbers are offsets in yards from the player's feet
//! (WoW axes), so an eye height above the ground does not need the terrain's z.

use benilla_assets::coords::wow_to_bevy;
use benilla_world::view::WorldCamera;
use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::camera::CamMode;
use crate::settle::Phase;

/// Parses `ex,ey,ez,lx,ly,lz` into (eye, look) in WoW coordinates.
pub fn parse(text: &str) -> Option<([f32; 3], [f32; 3])> {
    let v: Vec<f32> = text
        .split(',')
        .map(|part| part.trim().parse::<f32>().ok())
        .collect::<Option<_>>()?;
    let [ex, ey, ez, lx, ly, lz] = v.as_slice() else {
        return None;
    };
    v.iter()
        .all(|n| n.is_finite())
        .then_some(([*ex, *ey, *ez], [*lx, *ly, *lz]))
}

pub fn plugin(app: &mut App) {
    let Some((eye, look)) = std::env::var("WOWSOULS_VANTAGE")
        .ok()
        .and_then(|v| parse(&v))
    else {
        return;
    };
    let relative = std::env::var("WOWSOULS_VANTAGE_REL").is_ok_and(|v| v.trim() == "1");
    app.insert_resource(Vantage {
        eye,
        look,
        relative,
    })
    .add_systems(Update, apply.run_if(in_state(Phase::Playing)));
}

#[derive(Resource)]
struct Vantage {
    eye: [f32; 3],
    look: [f32; 3],
    /// The numbers are offsets from the player's feet, not world coordinates.
    relative: bool,
}

fn apply(
    vantage: Res<Vantage>,
    mut mode: ResMut<CamMode>,
    mut cameras: Query<&mut Transform, With<WorldCamera>>,
    avatar: Query<&GlobalTransform, With<Avatar>>,
    mut done: Local<u32>,
) {
    // A few frames in a row: the orbit rig seats the camera until the mode flips.
    if *done >= 3 {
        return;
    }
    let Ok(mut xf) = cameras.single_mut() else {
        return;
    };
    let (eye, look) = if vantage.relative {
        let Ok(feet) = avatar.single().map(GlobalTransform::translation) else {
            return;
        };
        (
            feet + wow_to_bevy(vantage.eye),
            feet + wow_to_bevy(vantage.look),
        )
    } else {
        (wow_to_bevy(vantage.eye), wow_to_bevy(vantage.look))
    };
    *xf = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    if *mode != CamMode::Free {
        *mode = CamMode::Free;
    }
    *done += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_numbers_make_a_vantage() {
        let (eye, look) = parse("-8955, -98.5, 91.1, -8912.9, -125.4, 87.7").unwrap();
        assert_eq!(eye, [-8955.0, -98.5, 91.1]);
        assert_eq!(look, [-8912.9, -125.4, 87.7]);
    }

    #[test]
    fn anything_else_is_rejected() {
        assert!(parse("1,2,3").is_none());
        assert!(parse("1,2,3,4,5,x").is_none());
        assert!(parse("1,2,3,4,5,6,7").is_none());
        assert!(parse("1,2,3,4,5,inf").is_none());
    }
}
