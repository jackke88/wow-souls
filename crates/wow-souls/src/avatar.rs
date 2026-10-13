//! The player's body: one entity whose `Transform` is the feet position (and facing) in Bevy
//! space, carrying the motor's state and dressed as a human male ([`crate::actor`]). F2 draws the
//! collision capsule over it.

use avian3d::prelude::Collider;
use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

use benilla_assets::coords::wow_to_bevy;
use benilla_world::world_unit::WorldUnit;

use crate::actor::{ActorBuilt, ActorModel, Catalogs, CharLook, Dress, Look};
use crate::camera::BodyFade;
use crate::character::{Character, Chosen};
use crate::motor::{Motor, MoveIntent, CAPSULE_HEIGHT, CAPSULE_RADIUS};
use crate::settle::Phase;
use crate::slice;

/// Collision height of the body (yd), the motor's capsule.
pub const HEIGHT: f32 = CAPSULE_HEIGHT;

/// The body the player drives.
#[derive(Component)]
pub struct Avatar;

/// `CreatureDisplayInfo` 49: the HumanMale body every human male player's display carries.
pub const HUMAN_MALE_DISPLAY: u32 = 49;

/// The default look: race 1 (human), sex 0 (male). `WOWSOULS_LOOK=<skin>,<face>,<hair style>,
/// <hair colour>,<facial hair>` overrides the customization.
pub const DEFAULT_LOOK: CharLook = CharLook {
    race: 1,
    sex: 0,
    skin: 2,
    face: 1,
    hair_style: 3,
    hair_color: 1,
    facial_hair: 1,
};

/// Whether the debug capsule draws (F2); always while the body has no model.
#[derive(Resource, Default)]
pub struct ShowCapsule(pub bool);

/// The swept shape of the body. Avian's capsule length is the segment between the hemisphere
/// centres.
#[derive(Resource)]
pub struct BodyShape(pub Collider);

pub fn plugin(app: &mut App) {
    app.insert_resource(BodyShape(Collider::capsule(
        CAPSULE_RADIUS,
        CAPSULE_HEIGHT - 2.0 * CAPSULE_RADIUS,
    )))
    // `WOWSOULS_SHOW_BODY=1` starts with the F2 overlay on (capsule, blade segment).
    .insert_resource(ShowCapsule(
        std::env::var_os("WOWSOULS_SHOW_BODY").is_some(),
    ))
    .add_systems(Startup, spawn)
    .add_systems(Update, (dress, toggle_capsule, draw_body, hide_when_close))
    .add_systems(OnEnter(Phase::Playing), apply_chosen_look);
}

/// `WOWSOULS_LOOK`, when it names a look.
pub fn env_look() -> Option<CharLook> {
    std::env::var("WOWSOULS_LOOK")
        .ok()
        .and_then(|v| parse_look(&v))
}

/// The body's look: `WOWSOULS_LOOK`, else the chosen character's customization dials on the human male
/// body (the only one playable), else [`DEFAULT_LOOK`].
pub fn look_for(env: Option<CharLook>, chosen: Option<&Character>) -> CharLook {
    if let Some(look) = env {
        return look;
    }
    match chosen {
        Some(c) => CharLook {
            skin: c.dials.skin,
            face: c.dials.face,
            hair_style: c.dials.hair_style,
            hair_color: c.dials.hair_color,
            facial_hair: c.dials.facial_hair,
            ..DEFAULT_LOOK
        },
        None => DEFAULT_LOOK,
    }
}

/// `skin,face,hair style,hair colour,facial hair` onto a human male.
pub fn parse_look(v: &str) -> Option<CharLook> {
    let n: Vec<u8> = v
        .split(',')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    let [skin, face, hair_style, hair_color, facial_hair] = n[..] else {
        return None;
    };
    Some(CharLook {
        skin,
        face,
        hair_style,
        hair_color,
        facial_hair,
        ..DEFAULT_LOOK
    })
}

/// Names the body's model once the catalogs are in, dressed as its gear says ([`crate::equip`]):
/// a new character wears WoW's starting kit, the Worn Shortsword (display 1542) in the main hand.
fn dress(
    mut commands: Commands,
    catalogs: Option<Res<Catalogs>>,
    asset_server: Res<AssetServer>,
    avatar: Query<Entity, (With<Avatar>, Without<ActorModel>)>,
    owned: Res<crate::loot::Owned>,
    data: Res<crate::loot::Data>,
    wear: Res<crate::equip::WearOverride>,
    chosen: Option<Res<Chosen>>,
    mut tried: Local<bool>,
) {
    let (Some(c), Ok(body)) = (catalogs, avatar.single()) else {
        return;
    };
    if *tried {
        return;
    }
    *tried = true;
    let look = look_for(env_look(), chosen.as_deref().and_then(|c| c.0.as_ref()));
    match ActorModel::for_display(&c.creatures, HUMAN_MALE_DISPLAY, Some(look), &asset_server) {
        Some(model) => {
            let look = crate::equip::wanted(&owned.progress.gear, &data.0.items, &wear, Some(&c));
            commands.entity(body).insert(model);
            crate::equip::dress_new(&mut commands, body, &look);
        }
        None => warn!("avatar: display {HUMAN_MALE_DISPLAY} has no model; staying a capsule"),
    }
}

fn toggle_capsule(keys: Res<ButtonInput<KeyCode>>, mut show: ResMut<ShowCapsule>) {
    if keys.just_pressed(KeyCode::F2) {
        show.0 = !show.0;
    }
}

/// A first character made in the creation screen: the body was built with the default look before the
/// player chose, so it takes the chosen dials now (the actor builder redoes its parts when `Dress`
/// changes, reading the model's look again). An `WOWSOULS_LOOK` override keeps winning.
fn apply_chosen_look(
    chosen: Option<Res<Chosen>>,
    mut avatar: Query<(&mut ActorModel, Option<&mut Dress>), With<Avatar>>,
) {
    let Some(c) = chosen.as_deref().and_then(|c| c.0.as_ref()) else {
        return;
    };
    let Ok((mut model, dress)) = avatar.single_mut() else {
        return;
    };
    let want = look_for(env_look(), Some(c));
    if matches!(&model.look, Look::Character(l) if *l == want) {
        return;
    }
    model.look = Look::Character(want);
    // The same dress, marked changed: the builder redoes the parts.
    if let Some(mut dress) = dress {
        dress.set_changed();
    }
    info!("avatar: takes the chosen look {want:?}");
}

/// The camera inside the head hides the body (the model has no fade twins wired yet, so it is a
/// cut at half fade rather than benilla's feather); so does the creation screen's preview standing
/// in its place.
fn hide_when_close(
    fade: Res<BodyFade>,
    hidden: Res<crate::create::preview::HideAvatar>,
    mut avatar: Query<(&mut Visibility, Option<&mut WorldUnit>), With<Avatar>>,
    mut kept_bound: Local<Option<Aabb>>,
) {
    let Ok((mut vis, unit)) = avatar.single_mut() else {
        return;
    };
    let hide = fade.0 < 0.5 || hidden.0;
    // The engine's exterior cull writes `Visibility` of every unit that has a `bound` each frame
    // (Inherited when a window admits it), so a hide written here would be undone before it is
    // drawn. A unit without a bound is not its to decide: the bound is set aside while hidden.
    if let Some(mut unit) = unit {
        if hide {
            if let Some(bound) = unit.bound.take() {
                *kept_bound = Some(bound);
            }
        } else if unit.bound.is_none() {
            unit.bound = kept_bound.take();
        }
    }
    let want = if hide {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    if *vis != want {
        *vis = want;
    }
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        Name::new("Avatar"),
        Avatar,
        Transform::from_translation(wow_to_bevy(slice::anchor())),
        Motor::default(),
        MoveIntent::default(),
        crate::combat::Action::default(),
        crate::hits::BladeTrail::default(),
        crate::combat::Health::full(crate::combat::HEALTH_MAX),
        crate::combat::Stamina::full(crate::combat::STAMINA_MAX),
        crate::monster::Poise::new(crate::reaction::player_poise()),
        crate::hits::Hurtbox {
            radius: CAPSULE_RADIUS,
            height: CAPSULE_HEIGHT,
        },
        Visibility::default(),
        benilla_world::world_unit::ViewerUnit,
    ));
}

/// The body as a wire capsule (amber held, green grounded, sky blue airborne) with a facing arrow,
/// faded as the camera closes on it.
fn draw_body(
    phase: Res<State<Phase>>,
    fade: Res<BodyFade>,
    show: Res<ShowCapsule>,
    avatar: Query<(&Transform, &Motor, Has<ActorBuilt>), With<Avatar>>,
    mut gizmos: Gizmos,
) {
    let Ok((body, motor, dressed)) = avatar.single() else {
        return;
    };
    if dressed && !show.0 {
        return;
    }
    let color = match (phase.get(), motor.grounded) {
        (Phase::Streaming | Phase::Menu, _) => Color::srgb(1.0, 0.7, 0.1),
        (Phase::Playing, true) => Color::srgb(0.2, 1.0, 0.3),
        (Phase::Playing, false) => Color::srgb(0.3, 0.7, 1.0),
    }
    // The camera closing on the body fades it, as the model will (step 5).
    .with_alpha(fade.0);
    if fade.0 <= 0.0 {
        return;
    }
    let center = body.translation + Vec3::Y * (HEIGHT * 0.5);
    gizmos.primitive_3d(
        &Capsule3d::new(CAPSULE_RADIUS, HEIGHT - 2.0 * CAPSULE_RADIUS),
        Isometry3d::from_translation(center),
        color,
    );
    let chest = body.translation + Vec3::Y * (HEIGHT * 0.7);
    gizmos.arrow(chest, chest + *body.forward() * 0.9, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_look_overrides_the_customization_of_a_human_male() {
        let l = parse_look("4, 2, 7, 3, 0").unwrap();
        assert_eq!((l.race, l.sex), (1, 0));
        assert_eq!(
            (l.skin, l.face, l.hair_style, l.hair_color, l.facial_hair),
            (4, 2, 7, 3, 0)
        );
    }

    #[test]
    fn the_body_takes_the_chosen_dials_and_the_environment_wins() {
        let mut c = Character::human_warrior("Jacob");
        c.dials = crate::character::Dials::from_array([4, 3, 2, 1, 0]);
        let l = look_for(None, Some(&c));
        assert_eq!((l.race, l.sex), (1, 0), "the human male body");
        assert_eq!(
            (l.skin, l.face, l.hair_style, l.hair_color, l.facial_hair),
            (4, 3, 2, 1, 0)
        );
        assert_eq!(
            look_for(None, None),
            DEFAULT_LOOK,
            "no character: the default"
        );
        let env = parse_look("7,7,7,7,7");
        assert_eq!(look_for(env, Some(&c)), env.unwrap(), "WOWSOULS_LOOK wins");
        // A character saved before the dials existed looks as the default always did.
        let old = Character::human_warrior("Jacob");
        assert_eq!(look_for(None, Some(&old)), DEFAULT_LOOK);
    }

    #[test]
    fn a_short_or_bad_look_is_refused() {
        assert_eq!(parse_look("1,2,3"), None);
        assert_eq!(parse_look("1,2,3,4,x"), None);
        assert_eq!(parse_look("1,2,3,4,300"), None);
    }
}
