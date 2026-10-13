//! The creation screen's 3D body: the chosen race, sex, look and starting outfit, standing where the
//! avatar stands and turned by the screen's yaw, framed by the body's height.
//!
//! A new race or sex is another model, so the next body is spawned hidden and swapped in when its
//! parts are built (no flash of nothing); a dial or a class only dresses the same body again (the
//! actor builder redoes the parts when its `Dress` changes, reading the model's look again). The
//! avatar is hidden while a preview shows.

use std::time::Duration;

use bevy::animation::transition::AnimationTransitions;
use bevy::animation::RepeatAnimation;
use bevy::prelude::*;

use benilla_assets::ModelAnimations;
use benilla_formats::StartOutfitItem;
use benilla_world::view::WorldCamera;

use super::rules::Selection;
use super::CreateData;
use crate::actor::anim::ids;
use crate::actor::{ActorBuilt, ActorModel, Catalogs, CharLook, Dress, Look};
use crate::avatar::Avatar;
use crate::menu::{Menu, Screen};
use crate::settle::Phase;

/// The avatar stays out of sight while the preview stands in its place.
#[derive(Resource, Default)]
pub struct HideAvatar(pub bool);

/// A preview body.
#[derive(Component)]
pub struct Preview;

/// What a preview shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    race: u8,
    sex: u8,
    class: u8,
    dials: [u8; 5],
}

impl Key {
    fn of(sel: &Selection) -> Key {
        Key {
            race: sel.race,
            sex: sel.sex,
            class: sel.class,
            dials: sel.dials,
        }
    }

    /// The model: another race or sex is another file.
    fn body(self) -> (u8, u8) {
        (self.race, self.sex)
    }

    fn look(self) -> CharLook {
        CharLook {
            race: self.race,
            sex: self.sex,
            skin: self.dials[0],
            face: self.dials[1],
            hair_style: self.dials[2],
            hair_color: self.dials[3],
            facial_hair: self.dials[4],
        }
    }
}

#[derive(Resource, Default)]
struct Stage {
    shown: Option<(Entity, Key)>,
    pending: Option<(Entity, Key)>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<HideAvatar>()
        .init_resource::<Stage>()
        .add_systems(
            Update,
            (drive, place, camera, idle)
                .chain()
                .run_if(in_state(Phase::Menu)),
        )
        .add_systems(OnExit(Phase::Menu), cleanup);
}

/// The worn level-1 gear of a (race, class, sex) as the body's `Dress` slots: head, shoulder, shirt,
/// chest (a robe too), belt, pants, boots, wrist, gloves, tabard, and the cloak. Weapons, shields and
/// the rest map to nothing.
pub fn outfit_dress(items: &[StartOutfitItem]) -> Dress {
    let mut dress = Dress::default();
    for item in items {
        let slot = match item.inv_type {
            1 => Some(0),
            3 => Some(1),
            4 => Some(2),
            5 | 20 => Some(3),
            6 => Some(4),
            7 => Some(5),
            8 => Some(6),
            9 => Some(7),
            10 => Some(8),
            19 => Some(9),
            _ => None,
        };
        match (slot, item.inv_type) {
            (Some(slot), _) => dress.worn[slot] = item.display_id,
            (None, 16) => dress.cloak = item.display_id,
            _ => {}
        }
    }
    dress
}

/// How tall a race's body stands, in yards, roughly: what the camera frames. (The models carry their
/// size in their geometry, not in the display's scale.)
pub fn body_height(race: u8) -> f32 {
    match race {
        3 => 1.55, // dwarf
        7 => 1.15, // gnome
        4 => 2.2,  // night elf
        2 => 2.15, // orc
        6 => 2.7,  // tauren
        8 => 2.5,  // troll
        _ => 1.95, // human, undead
    }
}

/// The camera for a body of height `h`: `(distance, look-at height, camera height)`. A human (about
/// 2 yd) is the menu's old framing, 3.3 yd away at head height.
pub fn framing(h: f32) -> (f32, f32, f32) {
    (1.5 + 0.9 * h, 0.5 * h, 0.72 * h)
}

fn spawn(
    commands: &mut Commands,
    catalogs: &Catalogs,
    data: &CreateData,
    key: Key,
    at: &Transform,
    assets: &AssetServer,
) -> Option<Entity> {
    let display = data.catalog.body_display(key.race, key.sex)?;
    let model = ActorModel::for_display(&catalogs.creatures, display, Some(key.look()), assets)?;
    let dress = outfit_dress(data.catalog.start_outfit(key.race, key.class, key.sex));
    Some(
        commands
            .spawn((
                Name::new("Creation preview"),
                Preview,
                Transform::from_translation(at.translation).with_rotation(at.rotation),
                Visibility::Hidden,
                model,
                dress,
            ))
            .id(),
    )
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    menu: Option<Res<Menu>>,
    data: Option<Res<CreateData>>,
    catalogs: Option<Res<Catalogs>>,
    assets: Res<AssetServer>,
    avatar: Query<&Transform, (With<Avatar>, Without<Preview>)>,
    built: Query<(), (With<Preview>, With<ActorBuilt>)>,
    mut bodies: Query<(&mut ActorModel, &mut Dress, &mut Visibility), With<Preview>>,
    mut stage: ResMut<Stage>,
    mut hide: ResMut<HideAvatar>,
    mut failed: Local<Option<Key>>,
) {
    let on = menu.as_ref().is_some_and(|m| m.screen == Screen::Create);
    if !on {
        for (e, _) in stage.shown.take().into_iter().chain(stage.pending.take()) {
            commands.entity(e).despawn();
        }
        hide.0 = false;
        return;
    }
    let (Some(menu), Some(data), Some(catalogs), Ok(at)) = (menu, data, catalogs, avatar.single())
    else {
        return;
    };
    let want = Key::of(&menu.sel);

    // A body that finished building takes the stage.
    if let Some((e, key)) = stage.pending {
        if built.get(e).is_ok() {
            if let Ok((_, _, mut vis)) = bodies.get_mut(e) {
                *vis = Visibility::Inherited;
            }
            if let Some((old, _)) = stage.shown.replace((e, key)) {
                commands.entity(old).despawn();
            }
            stage.pending = None;
        }
    }

    let redress =
        |bodies: &mut Query<(&mut ActorModel, &mut Dress, &mut Visibility), With<Preview>>,
         entity: Entity,
         key: Key| {
            if let Ok((mut model, mut dress, _)) = bodies.get_mut(entity) {
                model.look = Look::Character(key.look());
                *dress = outfit_dress(data.catalog.start_outfit(key.race, key.class, key.sex));
            }
        };

    match stage.shown {
        Some((_, key)) if key == want => {
            if let Some((stale, _)) = stage.pending.take() {
                commands.entity(stale).despawn();
            }
        }
        Some((e, key)) if key.body() == want.body() => {
            redress(&mut bodies, e, want);
            stage.shown = Some((e, want));
            if let Some((stale, _)) = stage.pending.take() {
                commands.entity(stale).despawn();
            }
        }
        _ => match stage.pending {
            Some((e, key)) if key.body() == want.body() => {
                if key != want {
                    redress(&mut bodies, e, want);
                    stage.pending = Some((e, want));
                }
            }
            other => {
                if let Some((stale, _)) = other {
                    commands.entity(stale).despawn();
                    stage.pending = None;
                }
                if *failed != Some(want) {
                    match spawn(&mut commands, &catalogs, &data, want, at, &assets) {
                        Some(e) => stage.pending = Some((e, want)),
                        None => {
                            warn!(
                                "create: no body for race {} sex {}; the preview stays as it is",
                                want.race, want.sex
                            );
                            *failed = Some(want);
                        }
                    }
                }
            }
        },
    }
    hide.0 = stage.shown.is_some();
}

/// Stands the bodies where the avatar stands, turned by the screen's yaw (the scale was set by the
/// builder).
fn place(
    menu: Option<Res<Menu>>,
    avatar: Query<&Transform, (With<Avatar>, Without<Preview>)>,
    mut bodies: Query<&mut Transform, With<Preview>>,
) {
    let (Some(menu), Ok(at)) = (menu, avatar.single()) else {
        return;
    };
    let rotation = at.rotation * Quat::from_rotation_y(menu.sel.yaw);
    for mut xf in &mut bodies {
        if xf.translation != at.translation {
            xf.translation = at.translation;
        }
        if xf.rotation != rotation {
            xf.rotation = rotation;
        }
    }
}

/// The camera: in front of the body, at its head, looking at its chest.
fn camera(
    menu: Option<Res<Menu>>,
    stage: Res<Stage>,
    avatar: Query<&Transform, (With<Avatar>, Without<WorldCamera>)>,
    mut cam: Query<&mut Transform, With<WorldCamera>>,
) {
    let Some(menu) = menu else { return };
    if menu.screen != Screen::Create {
        return;
    }
    let (Ok(body), Ok(mut cam)) = (avatar.single(), cam.single_mut()) else {
        return;
    };
    let race = stage
        .shown
        .or(stage.pending)
        .map_or(menu.sel.race, |(_, k)| k.race);
    let (dist, look_h, cam_h) = framing(body_height(race));
    let forward = body.rotation * Vec3::NEG_Z;
    let at = body.translation + Vec3::Y * look_h;
    let from = body.translation + forward * dist + Vec3::Y * cam_h;
    *cam = Transform::from_translation(from).looking_at(at, Vec3::Y);
}

/// A freshly built body stands in its idle clip.
fn idle(
    mut bodies: Query<
        (
            &ModelAnimations,
            &mut AnimationPlayer,
            &mut AnimationTransitions,
        ),
        (With<Preview>, Added<ActorBuilt>),
    >,
) {
    for (anims, mut player, mut transitions) in &mut bodies {
        if let Some(clip) = anims.find(ids::STAND) {
            transitions
                .play(&mut player, clip.node, Duration::ZERO)
                .set_repeat(RepeatAnimation::Forever);
        }
    }
}

fn cleanup(
    mut commands: Commands,
    bodies: Query<Entity, With<Preview>>,
    mut stage: ResMut<Stage>,
    mut hide: ResMut<HideAvatar>,
) {
    for e in &bodies {
        commands.entity(e).despawn();
    }
    *stage = Stage::default();
    hide.0 = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(display_id: u32, inv_type: u8) -> StartOutfitItem {
        StartOutfitItem {
            display_id,
            inv_type,
        }
    }

    #[test]
    fn the_outfit_lands_in_the_dress_slots_by_inventory_type() {
        let dress = outfit_dress(&[
            item(11, 1),  // head
            item(12, 3),  // shoulders
            item(13, 4),  // shirt
            item(14, 5),  // chest
            item(15, 6),  // belt
            item(16, 7),  // legs
            item(17, 8),  // feet
            item(18, 9),  // wrists
            item(19, 10), // hands
            item(20, 19), // tabard
            item(21, 16), // cloak
            item(22, 13), // a weapon: not worn on the body
            item(23, 14), // a shield: nor this
        ]);
        assert_eq!(dress.worn, [11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);
        assert_eq!(dress.cloak, 21);
    }

    #[test]
    fn a_robe_is_a_chest_and_an_empty_outfit_is_a_bare_body() {
        assert_eq!(outfit_dress(&[item(7, 20)]).worn[3], 7);
        assert_eq!(outfit_dress(&[]), Dress::default());
    }

    #[test]
    fn a_taller_body_is_framed_from_further_away_and_higher() {
        let (d_gnome, l_gnome, c_gnome) = framing(body_height(7));
        let (d_human, l_human, c_human) = framing(body_height(1));
        let (d_tauren, l_tauren, c_tauren) = framing(body_height(6));
        assert!(d_gnome < d_human && d_human < d_tauren);
        assert!(l_gnome < l_human && l_human < l_tauren);
        assert!(c_gnome < c_human && c_human < c_tauren);
        // The human is the menu's old framing: about 3.3 yd away, the camera at 1.4 yd.
        assert!((d_human - 3.3).abs() < 0.3, "{d_human}");
        assert!((c_human - 1.4).abs() < 0.2, "{c_human}");
    }

    #[test]
    fn a_key_names_the_body_and_the_look() {
        let a = Key {
            race: 2,
            sex: 1,
            class: 4,
            dials: [1, 2, 3, 4, 5],
        };
        assert_eq!(a.body(), (2, 1));
        let look = a.look();
        assert_eq!((look.race, look.sex), (2, 1));
        assert_eq!(
            (
                look.skin,
                look.face,
                look.hair_style,
                look.hair_color,
                look.facial_hair
            ),
            (1, 2, 3, 4, 5)
        );
        let b = Key { class: 1, ..a };
        assert_eq!(a.body(), b.body());
        assert_ne!(a, b);
    }
}
