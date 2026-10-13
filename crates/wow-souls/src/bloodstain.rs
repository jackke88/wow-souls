//! Dying costs the souls the player carries, as in Dark Souls: they stay behind as a bloodstain where
//! the body fell, and the count goes to 0. Walking into the stain gives them back. Dying again before
//! that replaces the stain with the new one, but only when the player carries something; with 0 souls
//! the old stain stays. Experience, level, quests and items are not touched by a death.
//!
//! The carried souls and the stain are saved (`souls-v1.txt`, written first like the other saves), so a
//! restart does not give the souls back for free. The stain is a dark patch on the ground with WoW's own
//! loot sparkles ([`crate::loot::vfx`]) over it.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::time::Real;

use benilla_world::collision::WorldCollision;

use crate::avatar::Avatar;
use crate::combat::Action;
use crate::hud::souls::{Counter, Souls};
use crate::hurt::Death;
use crate::npc::PlayerHurt;
use crate::quests::Toasts;
use crate::settle::{snap_to_ground, Phase};

/// How near (yd, along the ground) the player must come to take the souls back, and how far above or
/// below the stain still counts.
pub const RECOVER_RANGE: f32 = 1.5;
pub const RECOVER_HEIGHT: f32 = 2.5;
/// The stain's patch on the ground: radius (yd) and the lift that keeps it off the terrain.
const PATCH_RADIUS: f32 = 0.8;
const PATCH_LIFT: f32 = 0.08;

/// The souls left where the player died.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stain {
    /// Bevy position of the spot (on the ground).
    pub at: Vec3,
    pub souls: u32,
}

/// The player dies at `at` carrying `carried` souls: they leave the player for a stain there, an older
/// stain being lost. Carrying nothing changes nothing (an older stain stays).
pub fn die(carried: &mut u32, stain: &mut Option<Stain>, at: Vec3) {
    if *carried == 0 {
        return;
    }
    *stain = Some(Stain {
        at,
        souls: *carried,
    });
    *carried = 0;
}

/// Whether a player standing at `feet` is on the stain.
pub fn near(stain: &Stain, feet: Vec3) -> bool {
    let along = Vec2::new(feet.x - stain.at.x, feet.z - stain.at.z).length();
    along <= RECOVER_RANGE && (feet.y - stain.at.y).abs() <= RECOVER_HEIGHT
}

/// The stain's souls go back to the player; returns how many (0 without a stain).
pub fn recover(carried: &mut u32, stain: &mut Option<Stain>) -> u32 {
    let Some(s) = stain.take() else {
        return 0;
    };
    *carried = carried.saturating_add(s.souls);
    s.souls
}

/// The souls in the Abbey's bank: safe from a death, put in at a tithe ([`wow_world::shop`]).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Banked(pub u32);

/// The save file: `wssouls 1`, `carried <n>`, `banked <n>` (when there are any) and, when a stain waits,
/// `stain <x> <y> <z> <souls>`.
pub fn to_text(carried: u32, banked: u32, stain: Option<&Stain>) -> String {
    let mut text = format!("wssouls 1\ncarried {carried}\n");
    if banked > 0 {
        text.push_str(&format!("banked {banked}\n"));
    }
    if let Some(s) = stain {
        text.push_str(&format!(
            "stain {} {} {} {}\n",
            s.at.x, s.at.y, s.at.z, s.souls
        ));
    }
    text
}

/// Reads [`to_text`]; anything else is an error naming the line (the game then plays without saving
/// over the file).
pub fn from_text(text: &str) -> Result<(u32, u32, Option<Stain>), String> {
    let mut lines = text.lines();
    if lines.next() != Some("wssouls 1") {
        return Err("not a wssouls 1 file".into());
    }
    let mut carried = None;
    let mut banked = None;
    let mut stain = None;
    for (n, line) in lines.enumerate() {
        let at = n + 2;
        let mut words = line.split_whitespace();
        match words.next() {
            Some("carried") if carried.is_none() => {
                let v = words.next().and_then(|w| w.parse::<u32>().ok());
                carried = Some(v.ok_or(format!("line {at}: a souls count was expected"))?);
            }
            Some("banked") if banked.is_none() => {
                let v = words.next().and_then(|w| w.parse::<u32>().ok());
                banked = Some(v.ok_or(format!("line {at}: a souls count was expected"))?);
            }
            Some("stain") if stain.is_none() => {
                let f: Vec<f32> = words
                    .by_ref()
                    .take(3)
                    .filter_map(|w| w.parse().ok())
                    .collect();
                let souls = words.next().and_then(|w| w.parse::<u32>().ok());
                match (f.as_slice(), souls) {
                    ([x, y, z], Some(souls)) if words.next().is_none() => {
                        stain = Some(Stain {
                            at: Vec3::new(*x, *y, *z),
                            souls,
                        });
                    }
                    _ => return Err(format!("line {at}: `stain x y z souls` was expected")),
                }
            }
            _ => return Err(format!("line {at}: unexpected {line:?}")),
        }
    }
    Ok((
        carried.ok_or("no `carried` line")?,
        banked.unwrap_or(0),
        stain,
    ))
}

/// The stain the world shows: the patch and, under it as children, the sparkles.
#[derive(Component)]
struct StainMark(Stain);

/// The stain waiting for the player, if any.
#[derive(Resource, Default, Debug)]
pub struct Bloodstain(pub Option<Stain>);

/// Where the souls are saved, and whether the file could not be read (then it is never written over).
#[derive(Resource)]
struct SaveFile {
    path: PathBuf,
    blocked: Option<String>,
}

/// Unattended checks: `WOWSOULS_SOULS_SCRIPT=<s>:<cmd>,...` at real second `s`. Commands: `give:<n>` (n souls
/// as a kill would give) and `die` (a blow that kills the player).
#[derive(Resource, Default)]
struct Script {
    steps: Vec<(f32, String)>,
    next: usize,
}

fn parse_script(s: &str) -> Vec<(f32, String)> {
    let mut steps: Vec<(f32, String)> = s
        .split(',')
        .filter_map(|step| {
            let (t, cmd) = step.trim().split_once(':')?;
            Some((t.trim().parse().ok()?, cmd.trim().to_string()))
        })
        .collect();
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    steps
}

fn save_path() -> PathBuf {
    std::env::var("WOWSOULS_SOULS_SAVE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if crate::run_mode::unattended() {
                PathBuf::from(format!("cache/loot-check/souls-{}.txt", std::process::id()))
            } else {
                PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "saves".into()))
                    .join("WoWSouls/souls-v1.txt")
            }
        })
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Bloodstain>()
        .init_resource::<Banked>()
        .insert_resource(Script {
            steps: std::env::var("WOWSOULS_SOULS_SCRIPT")
                .map(|s| parse_script(&s))
                .unwrap_or_default(),
            next: 0,
        })
        .add_systems(Startup, load)
        .add_systems(
            Update,
            (script, drop_on_death, pick_up, sync_mark, save)
                .chain()
                .run_if(in_state(Phase::Playing)),
        );
}

fn load(
    mut commands: Commands,
    mut souls: ResMut<Souls>,
    mut bank: ResMut<Banked>,
    mut stain: ResMut<Bloodstain>,
) {
    let path = save_path();
    let mut blocked = None;
    match std::fs::read_to_string(&path) {
        Ok(text) => match from_text(&text) {
            Ok((carried, banked, spot)) => {
                souls.0 = carried;
                bank.0 = banked;
                stain.0 = spot;
                info!(
                    "souls: save {} (carried {carried}, banked {banked}, stain {})",
                    path.display(),
                    spot.map_or("none".to_string(), |s| format!(
                        "{} souls at {:.1} {:.1} {:.1}",
                        s.souls, s.at.x, s.at.y, s.at.z
                    ))
                );
            }
            Err(e) => blocked = Some(format!("{}: {e}", path.display())),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => blocked = Some(format!("{}: {e}", path.display())),
    }
    if let Some(why) = &blocked {
        warn!("souls: the save cannot be read ({why}); playing without saving over it");
    }
    commands.insert_resource(SaveFile { path, blocked });
}

/// The player's death takes the souls to a stain on the ground under the body.
#[allow(clippy::too_many_arguments)]
fn drop_on_death(
    death: Res<Death>,
    mut was_dead: Local<bool>,
    avatar: Query<&Transform, With<Avatar>>,
    collide: WorldCollision,
    mut souls: ResMut<Souls>,
    mut stain: ResMut<Bloodstain>,
) {
    let dead = death.at.is_some();
    let fresh = dead && !*was_dead;
    *was_dead = dead;
    if !fresh {
        return;
    }
    let Ok(body) = avatar.single() else {
        return;
    };
    let feet = body.translation;
    let ground = snap_to_ground(&collide, feet).unwrap_or(feet.y);
    let at = Vec3::new(feet.x, ground, feet.z);
    let lost = souls.0;
    if lost == 0 {
        info!(
            "souls: died carrying nothing{}",
            if stain.0.is_some() {
                "; the old stain stays"
            } else {
                ""
            }
        );
        return;
    }
    die(&mut souls.0, &mut stain.0, at);
    info!(
        "souls: died carrying {lost}: stain at {:.1} {:.1} {:.1}",
        at.x, at.y, at.z
    );
}

/// Walking into the stain gives the souls back.
fn pick_up(
    death: Res<Death>,
    real: Res<Time<Real>>,
    avatar: Query<(&Transform, &Action), With<Avatar>>,
    mut souls: ResMut<Souls>,
    mut counter: ResMut<Counter>,
    mut stain: ResMut<Bloodstain>,
    mut toasts: ResMut<Toasts>,
) {
    let Some(spot) = stain.0 else {
        return;
    };
    if death.at.is_some() {
        return;
    }
    let Ok((body, action)) = avatar.single() else {
        return;
    };
    if action.is_dead() || !near(&spot, body.translation) {
        return;
    }
    let got = recover(&mut souls.0, &mut stain.0);
    counter.add(got);
    toasts.push(real.elapsed_secs(), format!("Souls recovered: {got}"));
    info!("souls: recovered {got} ({} carried)", souls.0);
}

/// Keeps one mark in the world for the stain, none without it.
#[allow(clippy::too_many_arguments)]
fn sync_mark(
    mut commands: Commands,
    stain: Res<Bloodstain>,
    marks: Query<(Entity, &StainMark)>,
    art: Option<Res<crate::loot::vfx::Art>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    if !stain.is_changed() && !marks.is_empty() == stain.0.is_some() {
        return;
    }
    let mut kept = false;
    for (e, mark) in &marks {
        if Some(mark.0) == stain.0 && !kept {
            kept = true;
        } else {
            commands.entity(e).despawn();
        }
    }
    let Some(spot) = stain.0.filter(|_| !kept) else {
        return;
    };
    let marker = commands
        .spawn((
            Name::new("bloodstain"),
            StainMark(spot),
            Transform::from_translation(spot.at),
            Visibility::default(),
        ))
        .id();
    if let (Some(mut meshes), Some(mut materials)) = (meshes, materials) {
        commands.spawn((
            Name::new("bloodstain patch"),
            Mesh3d(meshes.add(Circle::new(PATCH_RADIUS))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(0.30, 0.02, 0.02, 0.85),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Transform::from_translation(Vec3::Y * PATCH_LIFT)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ChildOf(marker),
        ));
    }
    if let Some(art) = art.as_deref() {
        crate::loot::vfx::spawn(&mut commands, marker, art);
    }
    info!(
        "souls: stain shown at {:.1} {:.1} {:.1}",
        spot.at.x, spot.at.y, spot.at.z
    );
}

/// Writes the souls and the stain when either changed.
fn save(souls: Res<Souls>, bank: Res<Banked>, stain: Res<Bloodstain>, file: Option<Res<SaveFile>>) {
    let Some(file) = file else {
        return;
    };
    if file.blocked.is_some() || !(souls.is_changed() || bank.is_changed() || stain.is_changed()) {
        return;
    }
    if let Some(dir) = file.path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = file.path.with_extension("tmp");
    let text = to_text(souls.0, bank.0, stain.0.as_ref());
    if let Err(e) = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, &file.path)) {
        warn!("souls: could not save {}: {e}", file.path.display());
    }
}

/// The scripted steps of a check.
fn script(
    real: Res<Time<Real>>,
    mut script: ResMut<Script>,
    avatar: Query<Entity, With<Avatar>>,
    mut souls: ResMut<Souls>,
    mut counter: ResMut<Counter>,
    mut hurt: MessageWriter<PlayerHurt>,
) {
    let now = real.elapsed_secs();
    while let Some((t, step)) = script.steps.get(script.next).cloned() {
        if t > now {
            break;
        }
        script.next += 1;
        match step.split_once(':') {
            Some(("give", n)) => {
                if let Ok(n) = n.trim().parse::<u32>() {
                    souls.0 = souls.0.saturating_add(n);
                    counter.add(n);
                    info!("souls: script gave {n} ({} carried)", souls.0);
                }
            }
            None if step == "die" => {
                if let Ok(me) = avatar.single() {
                    hurt.write(PlayerHurt {
                        from: me,
                        damage: 9999.0,
                        level: 0,
                        stop: 0.0,
                        knock: 0.0,
                        poise: 0.0,
                        blocked: false,
                    });
                    info!("souls: script strikes the player dead");
                }
            }
            _ => warn!("souls: unknown script step {step:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spot(x: f32, souls: u32) -> Stain {
        Stain {
            at: Vec3::new(x, 10.0, 5.0),
            souls,
        }
    }

    #[test]
    fn dying_leaves_the_carried_souls_in_a_stain() {
        let (mut carried, mut stain) = (300, None);
        die(&mut carried, &mut stain, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(carried, 0);
        assert_eq!(
            stain,
            Some(Stain {
                at: Vec3::new(1.0, 2.0, 3.0),
                souls: 300
            })
        );
    }

    #[test]
    fn dying_again_replaces_the_stain_when_souls_are_carried() {
        let (mut carried, mut stain) = (40, Some(spot(0.0, 300)));
        die(&mut carried, &mut stain, Vec3::new(9.0, 9.0, 9.0));
        assert_eq!(stain.unwrap().souls, 40, "the 300 are gone for good");
        assert_eq!(stain.unwrap().at, Vec3::new(9.0, 9.0, 9.0));
        assert_eq!(carried, 0);
    }

    #[test]
    fn dying_with_nothing_keeps_the_old_stain() {
        let (mut carried, mut stain) = (0, Some(spot(0.0, 300)));
        die(&mut carried, &mut stain, Vec3::new(9.0, 9.0, 9.0));
        assert_eq!(stain, Some(spot(0.0, 300)));
        let mut none = None;
        die(&mut 0, &mut none, Vec3::ZERO);
        assert_eq!(none, None, "no souls, no stain");
    }

    #[test]
    fn the_stain_is_reached_on_foot_not_from_far_above() {
        let s = spot(0.0, 10);
        assert!(near(&s, Vec3::new(1.0, 10.0, 5.0)));
        assert!(
            near(&s, Vec3::new(1.0, 11.5, 5.0)),
            "a step up still counts"
        );
        assert!(!near(&s, Vec3::new(1.6, 10.0, 5.0)), "beyond 1.5 yd");
        assert!(!near(&s, Vec3::new(0.0, 14.0, 5.0)), "a floor above");
        assert!(near(&s, Vec3::new(0.0, 10.0, 6.4)));
    }

    #[test]
    fn recovering_adds_the_souls_to_what_is_carried_and_clears_the_stain() {
        let (mut carried, mut stain) = (25, Some(spot(0.0, 300)));
        assert_eq!(recover(&mut carried, &mut stain), 300);
        assert_eq!((carried, stain), (325, None));
        assert_eq!(recover(&mut carried, &mut stain), 0, "nothing left to take");
        assert_eq!(carried, 325);
    }

    #[test]
    fn the_save_text_round_trips() {
        let s = Stain {
            at: Vec3::new(-8910.5, 80.25, -136.125),
            souls: 450,
        };
        let text = to_text(12, 0, Some(&s));
        assert_eq!(
            text,
            "wssouls 1\ncarried 12\nstain -8910.5 80.25 -136.125 450\n"
        );
        assert_eq!(from_text(&text), Ok((12, 0, Some(s))));
        assert_eq!(from_text(&to_text(0, 0, None)), Ok((0, 0, None)));
        // The bank is saved beside the purse, and an old file without it reads as an empty bank.
        assert_eq!(from_text(&to_text(5, 360, None)), Ok((5, 360, None)));
        assert_eq!(from_text("wssouls 1\ncarried 9\n"), Ok((9, 0, None)));
        assert!(from_text("wssouls 1\ncarried 9\nbanked\n").is_err());
    }

    #[test]
    fn a_broken_save_is_an_error_naming_the_line() {
        for bad in [
            "",
            "wssouls 2\ncarried 1\n",
            "wssouls 1\n",
            "wssouls 1\ncarried x\n",
            "wssouls 1\ncarried 1\ncarried 2\n",
            "wssouls 1\ncarried 1\nstain 1 2 3\n",
            "wssouls 1\ncarried 1\nstain 1 2 3 4 5\n",
            "wssouls 1\ncarried 1\nbonfire 1\n",
        ] {
            assert!(from_text(bad).is_err(), "{bad:?}");
        }
        let e = from_text("wssouls 1\ncarried 1\nbonfire 1\n").unwrap_err();
        assert!(e.contains("line 3"), "{e}");
    }

    #[test]
    fn a_script_is_sorted_and_malformed_steps_are_dropped() {
        assert_eq!(
            parse_script("5:die, 1:give:300,bogus, x:die"),
            vec![(1.0, "give:300".to_string()), (5.0, "die".to_string())]
        );
    }
}
