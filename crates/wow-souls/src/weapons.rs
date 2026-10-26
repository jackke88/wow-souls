//! Equipment selects Souls combat independently of an item's visual display ID.
use crate::settings::binds::Action as BindAction;
use crate::settings::Controls;
use crate::{
    avatar::Avatar,
    combat::Action,
    loot::{Data, Owned},
    motion_clip::{self, WeaponClips},
};
use bevy::prelude::*;
use std::collections::BTreeMap;
use wow_world::{equipment::Slot, model::Item};

pub fn family(item: Option<&Item>) -> &'static str {
    let Some(item) = item else { return "unarmed" };
    if item.class != 2 {
        return "unarmed";
    }
    family_for_subclass(item.subclass)
}

pub fn family_for_subclass(subclass: u32) -> &'static str {
    match subclass {
        0 => "axe",
        1 => "greataxe",
        4 => "mace",
        5 => "great_hammer",
        6 | 17 => "polearm",
        7 => "straight_sword",
        8 => "greatsword",
        10 => "staff",
        13 => "fist",
        15 => "dagger",
        2 => "bow",
        3 => "gun",
        16 => "thrown",
        18 => "crossbow",
        _ => "fist",
    }
}

#[derive(Resource, Default)]
struct Bank(BTreeMap<String, WeaponClips>);

#[derive(Resource)]
pub struct Ready(pub bool);
impl Default for Ready {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Default)]
pub struct Stance {
    pub ranged: bool,
    pub two_handed: bool,
}

fn stance(
    controls: Controls,
    owned: Res<Owned>,
    data: Res<Data>,
    mut stance: ResMut<Stance>,
    avatar: Query<&Action, With<Avatar>>,
    screen: Res<crate::loot::Screen>,
    phase: Res<State<crate::settle::Phase>>,
) {
    if *phase.get() != crate::settle::Phase::Playing
        || screen.open
        || screen.modal
        || avatar.iter().any(|a| a.owns_body() || a.is_dead())
    {
        return;
    }
    let toggle = controls.just_pressed(BindAction::Ranged);
    if toggle
        && (stance.ranged
            || owned
                .progress
                .gear
                .get(Slot::Ranged)
                .and_then(|id| crate::equip::find(&data.0.items, id))
                .is_some())
    {
        stance.ranged = !stance.ranged;
    }
    if controls.just_pressed(BindAction::TwoHand) {
        stance.two_handed = !stance.two_handed;
    }
}

fn load(mut commands: Commands, roll: Option<Res<motion_clip::EvadeClips>>) {
    let Some(roll) = roll else { return };
    let root = motion_clip::cache_dir();
    let mut bank = Bank::default();
    match std::fs::read_to_string(root.join("weapons.txt")) {
        Ok(text) if text.lines().next() == Some("wsweapons 1") => {
            for line in text.lines().skip(1) {
                let w: Vec<_> = line.split_whitespace().collect();
                if let ["family", name, path] = w[..] {
                    let dir = root.join(path);
                    match motion_clip::checked_manifest(&dir)
                        .and_then(|m| motion_clip::load_moves(&dir, &m, roll.wow.bones()))
                    {
                        Ok(moves) => {
                            bank.0.insert(name.into(), moves);
                        }
                        Err(e) => warn!("weapons: {name} unavailable: {e}"),
                    }
                }
            }
        }
        _ => warn!("weapons: restore the project anim/source/weapons.txt and weapon-family clips"),
    }
    info!("weapons: loaded {} families", bank.0.len());
    commands.insert_resource(bank);
}

fn select(
    mut bank: Option<ResMut<Bank>>,
    mut moves: Option<ResMut<WeaponClips>>,
    owned: Res<Owned>,
    data: Res<Data>,
    stance: Res<Stance>,
    avatar: Query<&Action, With<Avatar>>,
    mut defense: Option<ResMut<crate::defense::Kit>>,
    mut ready: ResMut<Ready>,
    mut initialized: Local<bool>,
    mut missing: Local<String>,
) {
    let (Some(bank), Some(moves)) = (bank.as_mut(), moves.as_mut()) else {
        return;
    };
    if avatar.iter().any(Action::owns_body) {
        return;
    }
    let item = owned
        .progress
        .gear
        .get(if stance.ranged {
            Slot::Ranged
        } else {
            Slot::MainHand
        })
        .and_then(|id| crate::equip::find(&data.0.items, id));
    let forced = std::env::var("WOWSOULS_WEAPON").ok();
    let base = family(item);
    let hand = match (base, stance.two_handed) {
        ("mace", true) => "mace_2h",
        ("axe", true) => "axe_2h",
        ("straight_sword", true) => "straight_sword_2h",
        ("dagger", true) => "dagger_2h",
        _ => base,
    };
    let want = forced.as_deref().unwrap_or(hand);
    if moves.family == want && *initialized {
        ready.0 = true;
        return;
    }
    if let Some(next) = bank.0.remove(want) {
        *initialized = true;
        ready.0 = true;
        if let Some(kit) = defense.as_mut() {
            if let Some((cut, stability, _)) = next.guard {
                kit.cut = cut;
                kit.stability = stability;
            }
            for clip in &next.guards {
                if let Some(c) = kit.clips.iter_mut().find(|c| c.kind == clip.kind) {
                    *c = clip.clone();
                }
                use crate::defense::Kind;
                match clip.kind {
                    Kind::Raise => kit.specs.raise = clip.spec,
                    Kind::Hold => kit.specs.hold = clip.spec,
                    Kind::Lower => kit.specs.lower = clip.spec,
                    _ => {}
                }
            }
            kit.grip = next.grip;
        }
        let old = std::mem::replace(&mut **moves, next);
        bank.0.insert(old.family.clone(), old);
        info!("weapons: selected {want} for {:?}", item.map(|i| i.entry));
        for kind in crate::combat::MoveKind::ALL {
            if let Some(c) = moves.clip(kind) {
                info!(
                    "weapons: {} {} hit {:.3}-{:.3} stamina {} multiplier {}",
                    kind.name(),
                    c.wow.name,
                    c.spec.hit.0,
                    c.spec.hit.1,
                    c.spec.cost,
                    c.spec.mult
                );
            }
        }
    } else {
        ready.0 = false;
        if *missing != want {
            warn!("weapons: no verified {want} cache; attacks disabled. Restore the project anim/source weapon-family clips");
            *missing = want.into();
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Stance>()
        .init_resource::<Ready>()
        .add_plugins(crate::projectiles::plugin)
        .add_systems(Startup, load.after(motion_clip::startup))
        .add_systems(Update, (stance, select).chain().before(crate::combat::act));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_weapon_cache_covers_every_family_and_original_mace_heavy() {
        let Ok(root) = std::env::var("WOWSOULS_TEST_WEAPONS") else {
            return;
        };
        let root = std::path::Path::new(&root);
        let index = std::fs::read_to_string(root.join("weapons.txt")).unwrap();
        let mut names = Vec::new();
        for line in index.lines().skip(1) {
            let v: Vec<_> = line.split_whitespace().collect();
            let dir = root.join(v[2]);
            let manifest = motion_clip::checked_manifest(&dir).unwrap();
            let m = motion_clip::load_moves(
                &dir,
                &manifest,
                motion_clip::read_clip(root, "human_male", "body_roll")
                    .unwrap()
                    .bones(),
            )
            .unwrap_or_else(|e| panic!("{}: {e}", v[1]));
            assert_eq!(m.family, v[1]);
            for kind in crate::combat::MoveKind::ALL {
                let c = m.clip(kind).unwrap();
                assert!(c.hits.iter().all(|h| h.hit.0 >= 0.0
                    && h.hit.0 < h.hit.1
                    && h.hit.1 <= c.wow.duration() + 0.001));
                assert!(c.spec.cost > 0.0 && c.spec.mult > 0.0);
            }
            if m.family == "mace_2h" {
                assert_eq!(
                    m.clip(crate::combat::MoveKind::R2(0)).unwrap().wow.name,
                    "mace_2h_heavy_1"
                );
            }
            if m.family == "great_hammer" {
                assert_eq!(m.moveset.chain_len, 2);
                let c = m.clip(crate::combat::MoveKind::R2(0)).unwrap();
                assert_eq!(c.hits.len(), 2);
                assert_eq!(c.spec.cost, 85.0);
                assert!(c.spec.poise > 30.0);
            }
            if let Some(p) = m.projectile {
                assert_eq!(m.two_handed, m.family != "thrown");
                if matches!(m.family.as_str(), "crossbow" | "gun") {
                    let clip = m.clip(crate::combat::MoveKind::R1(0)).unwrap();
                    let (_, weapon) = m.grip_bones.unwrap();
                    let q = crate::motion_clip::sample(&clip.reference, p.release, weapon).1;
                    let barrel = q * m.grip.weapon;
                    assert!(
                        barrel.dot(Vec3::NEG_Z) > 0.98,
                        "{}: barrel {barrel}",
                        m.family
                    );
                }
                assert!(
                    p.release
                        < m.clip(crate::combat::MoveKind::R1(0))
                            .unwrap()
                            .wow
                            .duration()
                );
            } else {
                assert_eq!(m.guards.len(), 3);
            }
            let (idle, _) = m.idle.as_ref().expect("every weapon needs a rest pose");
            // The runtime must not have to hide an actual discontinuity in the source loop.
            for bone in [48, 64, 49, 65, 78, 95, 50, 66, 79, 97] {
                let start = crate::motion_clip::sample(idle, 0.0, bone).1;
                let end = crate::motion_clip::sample(idle, idle.duration(), bone).1;
                assert!(
                    start.angle_between(end).to_degrees() < 0.2,
                    "{} rest seam",
                    m.family
                );
            }
            names.push(m.family);
        }
        for family in [
            "unarmed",
            "fist",
            "dagger",
            "dagger_2h",
            "straight_sword",
            "straight_sword_2h",
            "greatsword",
            "axe",
            "axe_2h",
            "greataxe",
            "mace",
            "mace_2h",
            "great_hammer",
            "polearm",
            "staff",
            "bow",
            "crossbow",
            "gun",
            "thrown",
        ] {
            assert!(names.iter().any(|n| n == family), "{family}");
        }
    }
    #[test]
    fn warrior_subclasses_choose_distinct_souls_families() {
        assert_eq!(family(None), "unarmed");
        for (subclass, expected) in [
            (0, "axe"),
            (1, "greataxe"),
            (2, "bow"),
            (3, "gun"),
            (4, "mace"),
            (5, "great_hammer"),
            (6, "polearm"),
            (7, "straight_sword"),
            (8, "greatsword"),
            (10, "staff"),
            (13, "fist"),
            (15, "dagger"),
            (16, "thrown"),
            (18, "crossbow"),
        ] {
            assert_eq!(family_for_subclass(subclass), expected);
        }
    }
}
