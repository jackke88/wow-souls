//! The belt torch (2026-10-18, the user: "facklor ... en extra gear slot längs ned ... hänger vid bältet,
//! eftersom det kan bli så mörkt nu med den nya grafiken ... brinna slut efter x mycket tid"). The item and
//! its slot are [`wow_world::torch`]; here it hangs from the left hip (the worn-item lane,
//! [`WornKind::Belt`]), lights the ground round the wearer with the engine's flickering torch light, and
//! burns down while it is worn: [`wow_world::torch::BURN_SECS`], then it is gone from the slot. What is
//! left of the burning torch is saved, so a torch taken off and put on again goes on burning from there.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use wow_world::equipment::Slot;
use wow_world::torch::{BURN_SECS, DISPLAY};

use crate::actor::held::{BeltTorch, Worn, WornItems, WornKind, WornRoot};
use crate::actor::ActorBuilt;
use crate::avatar::Avatar;
use crate::loot::Owned;
use crate::settle::Phase;

pub const FILE_NAME: &str = "torch-v1.txt";
/// Seconds between saves of the burn.
const SAVE_EVERY: f32 = 5.0;
/// The flame's warm light (linear RGB) and its strength against the engine's placed fires.
const COLOR: [f32; 3] = [1.0, 0.62, 0.28];
const STRENGTH: f32 = 0.9;

/// What is left of the torch that burns (seconds); 0 when no torch has been lit.
#[derive(Resource, Debug, Default)]
pub struct Fuel {
    pub left: f32,
    path: PathBuf,
    since_save: f32,
}

/// The light hung under a belt torch.
#[derive(Component)]
struct Flame;

pub fn to_text(left: f32) -> String {
    format!("wstorch 1\nleft {left:.1}\n")
}

pub fn from_text(text: &str) -> Option<f32> {
    let mut lines = text.lines();
    (lines.next()? == "wstorch 1").then_some(())?;
    let left: f32 = lines.next()?.strip_prefix("left ")?.trim().parse().ok()?;
    (left.is_finite() && (0.0..=BURN_SECS).contains(&left)).then_some(left)
}

/// One second of burning: `left` the torch's fuel. A torch put on with nothing left is a fresh one.
/// Returns the fuel after `dt` and whether it burnt out in it.
pub fn burn(left: f32, dt: f32) -> (f32, bool) {
    let left = if left <= 0.0 { BURN_SECS } else { left };
    let after = (left - dt).max(0.0);
    (after, after <= 0.0)
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load)
        .add_systems(
            Update,
            (burn_down, hang, light).run_if(in_state(Phase::Playing)),
        )
        .add_systems(
            PostUpdate,
            orient
                .before(TransformSystems::Propagate)
                .run_if(in_state(Phase::Playing)),
        );
}

fn load(mut commands: Commands) {
    let path = if crate::run_mode::unattended() {
        PathBuf::from(format!("cache/loot-check/torch-{}.txt", std::process::id()))
    } else {
        crate::saves::file(FILE_NAME)
    };
    let left = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| from_text(&t))
        .unwrap_or(0.0);
    commands.insert_resource(Fuel {
        left,
        path,
        since_save: 0.0,
    });
}

fn save(path: &Path, left: f32) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(path, to_text(left)) {
        warn!("torch: cannot save {}: {e}", path.display());
    }
}

fn burn_down(
    time: Res<Time>,
    real: Res<Time<Real>>,
    fuel: Option<ResMut<Fuel>>,
    owned: Option<ResMut<Owned>>,
    mut toasts: ResMut<crate::quests::Toasts>,
) {
    let (Some(mut fuel), Some(mut owned)) = (fuel, owned) else {
        return;
    };
    if owned.progress.gear.get(Slot::Torch).is_none() {
        return;
    }
    let fresh = fuel.left <= 0.0;
    let (left, out) = burn(fuel.left, time.delta_secs());
    if fresh {
        info!("torch: a fresh torch is lit ({BURN_SECS:.0} s)");
    }
    fuel.left = left;
    fuel.since_save += time.delta_secs();
    if out {
        let mut next = owned.progress.clone();
        next.gear.set(Slot::Torch, None);
        match owned.publish(next) {
            Ok(()) => {
                toasts.push(real.elapsed_secs(), "Your torch burns out.");
                info!("torch: burnt out");
            }
            Err(e) => warn!("torch: {e}"),
        }
        fuel.left = 0.0;
        fuel.since_save = SAVE_EVERY;
    }
    if fuel.since_save >= SAVE_EVERY {
        fuel.since_save = 0.0;
        save(&fuel.path, fuel.left);
    }
}

/// Hangs the torch at the belt while one is worn, and takes it down when it is not.
fn hang(
    mut commands: Commands,
    real: Res<Time<Real>>,
    mut asked: Local<Option<f32>>,
    owned: Option<Res<Owned>>,
    mut avatar: Query<(Entity, Option<&mut WornItems>), (With<Avatar>, With<ActorBuilt>)>,
    roots: Query<(Entity, &WornRoot), With<BeltTorch>>,
) {
    let Some(owned) = owned else {
        return;
    };
    let worn = owned.progress.gear.get(Slot::Torch).is_some();
    let Ok((me, pending)) = avatar.single_mut() else {
        return;
    };
    let mine: Vec<Entity> = roots
        .iter()
        .filter(|(_, w)| w.0 == me)
        .map(|(e, _)| e)
        .collect();
    let hung = !mine.is_empty();
    // Two hung by a race of deferred commands: one goes.
    for extra in mine.iter().skip(1) {
        commands.entity(*extra).despawn();
    }
    // A request is being built (its root may not be visible for a frame): ask again only after a while.
    let now = real.elapsed_secs();
    let waiting = asked.is_some_and(|t| now - t < 2.0);
    let queued = pending
        .as_ref()
        .is_some_and(|p| p.0.iter().any(|w| w.kind == WornKind::Belt));
    if worn && !hung && !queued && !waiting {
        *asked = Some(now);
        match pending {
            Some(mut p) => p.0.push(Worn::new(DISPLAY, WornKind::Belt)),
            None => {
                commands
                    .entity(me)
                    .insert(WornItems(vec![Worn::new(DISPLAY, WornKind::Belt)]));
            }
        }
    } else if !worn {
        for (root, w) in &roots {
            if w.0 == me {
                commands.entity(root).despawn();
            }
        }
        if let Some(mut p) = pending.filter(|_| queued) {
            p.0.retain(|w| w.kind != WornKind::Belt);
        }
    }
}

/// A belt torch's flame: the engine's torch light, flickering, carried (it casts no outdoor shadow).
fn light(
    mut commands: Commands,
    roots: Query<(Entity, &GlobalTransform), (With<BeltTorch>, Without<Children>)>,
    lit: Query<(Entity, &Children), With<BeltTorch>>,
    flames: Query<(), With<Flame>>,
) {
    let mut spawn = |root: Entity, at: Vec3| {
        commands.spawn((
            Name::new("Torch flame"),
            benilla_world::terrain_stream::point_light(COLOR, STRENGTH),
            benilla_world::lighting::FlameFlicker::new(
                benilla_world::lighting::FlameKind::Torch,
                benilla_world::lighting::flicker_seed(at),
            ),
            crate::gfx::carried::HeldLight,
            Flame,
            // The head of the shaft, 0.6 yd along it.
            Transform::from_translation(Vec3::NEG_Z * 0.6),
            Visibility::default(),
            ChildOf(root),
        ));
    };
    for (root, xf) in &roots {
        spawn(root, xf.translation());
    }
    for (root, children) in &lit {
        if !children.iter().any(|c| flames.contains(c)) {
            spawn(root, Vec3::ZERO);
        }
    }
}

/// The torch in the belt as the body carries it: tucked at the left hip, its head (the shaft runs
/// along the root's -Z, measured on screen) up and back, leaning out from the leg. Held against the
/// body, not the hip bone, so a stride does not swing it through the thigh.
fn carried_turn() -> Quat {
    Quat::from_rotation_z(0.3) * Quat::from_rotation_x(2.1)
}

/// Where the handle sits in the body's frame (yd before the body's scale): left, hip height, a little
/// back.
const AT_HIP: Vec3 = Vec3::new(-0.24, 0.98, 0.1);

fn orient(
    avatar: Query<&GlobalTransform, With<Avatar>>,
    parents: Query<&GlobalTransform, Without<BeltTorch>>,
    mut roots: Query<(&mut Transform, &ChildOf), With<BeltTorch>>,
) {
    let Ok(body) = avatar.single() else {
        return;
    };
    let want = body.rotation() * carried_turn();
    let at = body.transform_point(AT_HIP);
    for (mut xf, parent) in &mut roots {
        let Ok(joint) = parents.get(parent.parent()) else {
            continue;
        };
        let rotation = joint.rotation().inverse() * want;
        let translation = joint.affine().inverse().transform_point3(at);
        if xf.rotation.angle_between(rotation) > 1.0e-4
            || xf.translation.distance_squared(translation) > 1.0e-8
        {
            xf.rotation = rotation;
            xf.translation = translation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_worn_torch_burns_down_and_a_fresh_one_starts_full() {
        let (left, out) = burn(0.0, 1.0);
        assert_eq!(left, BURN_SECS - 1.0);
        assert!(!out);
        let (left, out) = burn(0.5, 1.0);
        assert_eq!(left, 0.0);
        assert!(out);
    }

    #[test]
    fn the_burn_save_round_trips_and_refuses_nonsense() {
        assert_eq!(from_text(&to_text(123.4)), Some(123.4));
        assert_eq!(from_text("wstorch 1\nleft -3\n"), None);
        assert_eq!(from_text("wstorch 2\nleft 3\n"), None);
        assert_eq!(from_text("junk"), None);
    }
}
