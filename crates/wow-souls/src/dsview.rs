//! `WOWSOULS_DSVIEW=<clip>`: a cached Dark Souls clip on the DS skeleton (gizmo bones and the DS
//! blade, 1.5 yd to the avatar's right) beside the WoW human playing the retargeted clip, in
//! lockstep, looping, in place (no root motion). `1` is the roll; a clip name (`a23_3000`) or a
//! move's name (`r1_1`) is a sword move. With `WOWSOULS_OBSERVE` and `WOWSOULS_SHOT` it makes the
//! comparison frames. Every 0.1 s it logs the clip time and the root distance covered; once per
//! loop it logs how far the WoW sword's blade strays from the Dark Souls blade.

use bevy::prelude::*;

use crate::actor::held::HeldBuilt;
use crate::avatar::Avatar;
use crate::combat::MoveKind;
use crate::dsclip::{clips_of, progress, sample, ClipId, DsMoves, DsRoll};
use crate::motor::Motor;
use crate::reaction::PlayerReacts;

/// The DS skeleton's `R_Weapon` bone in `c0000`'s clips (`dsr-cache`'s install test pins it).
pub const DS_WEAPON_BONE: usize = 56;
/// The DS blade as drawn on the DS skeleton (yd).
const DS_BLADE_LENGTH: f32 = 1.0;

/// The side-by-side's clip and time (s); once the cache has loaded and the name is known,
/// [`crate::dsclip`] poses the avatar from it.
#[derive(Resource, Debug)]
pub struct DsView {
    want: String,
    clip: Option<ClipId>,
    pub t: f32,
    /// `WOWSOULS_DSVIEW=<clip>@<s>`: hold the clip at this time instead of looping (for stills).
    hold: Option<f32>,
}

impl DsView {
    /// The clip on show and its time.
    pub fn playing(&self) -> Option<(ClipId, f32)> {
        self.clip.map(|c| (c, self.t))
    }
}

/// The clip `want` names: `1` or the roll's clip name, a move's clip name or its table name.
fn resolve(
    want: &str,
    roll: &DsRoll,
    moves: &DsMoves,
    reacts: Option<&PlayerReacts>,
    defense: Option<&crate::defense::Kit>,
    interaction: Option<&crate::loot::interaction::Kit>,
) -> Option<ClipId> {
    if want == "1" || want == roll.wow.name {
        return Some(ClipId::Roll);
    }
    if roll
        .back
        .as_ref()
        .is_some_and(|b| want == "backstep" || want == b.wow.name)
    {
        return Some(ClipId::Backstep);
    }
    MoveKind::ALL
        .into_iter()
        .find(|k| k.name() == want || moves.clip(*k).is_some_and(|m| m.wow.name == want))
        .map(ClipId::Move)
        .or_else(|| {
            reacts
                .and_then(|k| k.reacts.iter().position(|r| r.wow.name == want))
                .map(ClipId::React)
        })
        .or_else(|| {
            defense
                .and_then(|d| {
                    d.clips
                        .iter()
                        .find(|c| c.kind.name() == want || c.wow.name == want)
                })
                .map(|c| ClipId::Defense(c.kind))
        })
        .or_else(|| {
            interaction
                .and_then(|k| {
                    k.clips
                        .iter()
                        .find(|c| c.kind.name() == want || c.wow.name == want)
                })
                .map(|c| ClipId::Interaction(c.kind))
        })
        .or_else(|| {
            // A gait clip of the held weapon's set: `walk`, `lock_run_l`, or its clip name.
            moves.loco.as_deref().and_then(|k| {
                crate::dsloco::Role::ALL
                    .into_iter()
                    .find(|r| r.name() == want || k.get(*r).is_some_and(|c| c.wow.name == want))
                    .map(ClipId::Loco)
            })
        })
}

/// The angle (degrees) between the WoW sword's blade `wow` and the Dark Souls blade, both in the
/// body's frame: the DS weapon bone's rotation `ds_weapon` turning its blade axis `axis`.
pub fn blade_error_deg(wow: Vec3, ds_weapon: Quat, axis: Vec3) -> f32 {
    wow.angle_between(ds_weapon * axis).to_degrees()
}

/// The blade errors seen over one loop of the clip.
#[derive(Default)]
struct BladeStats {
    /// The clip time of the first frame measured: a loop that starts late (the held sword was not
    /// built yet) is not reported.
    start: Option<f32>,
    worst: f32,
    at: f32,
    sum: f32,
    frames: u32,
}

impl BladeStats {
    fn add(&mut self, t: f32, err: f32) {
        self.start.get_or_insert(t);
        if err > self.worst {
            self.worst = err;
            self.at = t;
        }
        self.sum += err;
        self.frames += 1;
    }

    fn mean(&self) -> f32 {
        self.sum / self.frames.max(1) as f32
    }
}

/// `WOWSOULS_DSVIEW=<species>:<clip>` (`kobold:small_1`): the creature plays a kit clip in place,
/// looping, beside the Dark Souls creature's skeleton (gold gizmo bones, 1.2 yd to its right), the
/// same time on both. `<clip>` is a move's name (`small_1`) or any kit clip's name (`a00_3000`,
/// `a00_2007`, `a00_2202`).
#[derive(Resource, Debug)]
pub struct SpeciesView {
    species: &'static str,
    /// The kit its clips come from (a vermin plays the kobold's).
    kit: &'static str,
    want: String,
    clip: Option<crate::monster::MonsterClip>,
    /// `kobold:<clip>@<s>`: hold the clip at this time instead of looping (for stills).
    hold: Option<f32>,
}

/// The species the side-by-side asks for, if it does: its creatures stand still and show the clip.
pub fn view_species() -> Option<&'static str> {
    static ON: std::sync::OnceLock<Option<&'static str>> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        let v = std::env::var("WOWSOULS_DSVIEW").ok()?;
        let (species, _) = v.split_once(':')?;
        crate::species::by_name(species).map(|s| s.name)
    })
}

pub fn plugin(app: &mut App) {
    if let Some(species) = view_species() {
        let want = std::env::var("WOWSOULS_DSVIEW")
            .ok()
            .and_then(|v| v.split_once(':').map(|(_, w)| w.to_string()))
            .unwrap_or_default();
        let (want, hold) = match want.split_once('@') {
            Some((w, t)) => (w.to_string(), t.parse::<f32>().ok()),
            None => (want, None),
        };
        app.insert_resource(SpeciesView {
            species,
            kit: crate::species::by_name(species).map_or(species, |s| s.kit),
            want,
            clip: None,
            hold,
        })
        .add_systems(Update, species_view)
        .add_systems(
            PostUpdate,
            species_draw.after(benilla_world::billboard::BillboardPlace),
        );
        return;
    }
    if let Ok(want) = std::env::var("WOWSOULS_DSVIEW") {
        let (want, hold) = match want.split_once('@') {
            Some((w, t)) => (w.to_string(), t.parse::<f32>().ok()),
            None => (want, None),
        };
        app.insert_resource(DsView {
            want,
            clip: None,
            t: hold.unwrap_or(0.0),
            hold,
        })
        // `measure` reads the pose the last frame drew, at the time `advance` has not yet moved;
        // `draw` runs once the frame's pose is placed, so the overlay is not a frame behind.
        .add_systems(Update, (measure, advance).chain())
        .add_systems(
            PostUpdate,
            draw.after(benilla_world::billboard::BillboardPlace),
        );
    }
}

/// Finds the clip, then keeps the creatures of the species playing it in a loop (the clip's own
/// length).
fn species_view(
    roster: Option<Res<crate::monster::Roster>>,
    mut view: ResMut<SpeciesView>,
    mut npcs: Query<(&crate::species::Species, &mut crate::monster::MonsterPlay)>,
    mut warned: Local<bool>,
) {
    use crate::monster::MonsterClip;
    let Some(kit) = roster.as_deref().and_then(|r| r.get(view.kit)) else {
        return;
    };
    if view.clip.is_none() {
        let id = (0..kit.attacks.len())
            .map(|i| MonsterClip::Attack(i as u8))
            .chain((0..kit.reacts.len()).map(|i| MonsterClip::React(i as u8)))
            .chain((0..kit.deaths.len()).map(|i| MonsterClip::Death(i as u8)))
            .find(|&id| {
                let named = matches!(id, MonsterClip::Attack(i) if kit.attacks[usize::from(i)].spec.name == view.want);
                named || kit.clips(id).is_some_and(|(w, _)| w.name == view.want)
            });
        view.clip = id;
        if id.is_none() && !*warned {
            *warned = true;
            warn!("dsview: no {} clip is called {:?}", view.species, view.want);
        }
    }
    let Some(id) = view.clip else {
        return;
    };
    let Some((wow, _)) = kit.clips(id) else {
        return;
    };
    let len = wow.duration().max(1.0e-3);
    for (species, mut p) in &mut npcs {
        if species.0.name != view.species {
            continue;
        }
        let t = match (view.hold, p.clip) {
            (Some(t), _) => t.min(len),
            (None, Some((c, t))) if c == id => t % len,
            _ => 0.0,
        };
        p.clip = Some((id, t));
    }
}

/// The Dark Souls creature's skeleton beside the creature, at the clip's time.
fn species_draw(
    roster: Option<Res<crate::monster::Roster>>,
    view: Res<SpeciesView>,
    npcs: Query<(
        &crate::species::Species,
        &Transform,
        &crate::monster::MonsterPlay,
    )>,
    mut gizmos: Gizmos,
) {
    let (Some(kit), Some(id)) = (roster.as_deref().and_then(|r| r.get(view.kit)), view.clip) else {
        return;
    };
    let Some((_, ds)) = kit.clips(id) else {
        return;
    };
    let gold = Color::srgb(0.95, 0.75, 0.2);
    for (species, body, play) in &npcs {
        if species.0.name != view.species {
            continue;
        }
        let Some((_, t)) = play.clip else {
            continue;
        };
        let origin = body.translation + *body.right() * 1.2;
        let at = |b: usize| origin + body.rotation * (sample(ds, t, b).0 * body.scale.y);
        for (i, &p) in ds.parents.iter().enumerate() {
            let joint = at(i);
            gizmos.sphere(Isometry3d::from_translation(joint), 0.02, gold);
            if let Ok(p) = usize::try_from(p) {
                gizmos.line(at(p), joint, gold);
            }
        }
    }
}

fn advance(
    time: Res<Time>,
    roll: Option<Res<DsRoll>>,
    moves: Option<Res<DsMoves>>,
    reacts: Option<Res<PlayerReacts>>,
    defense: Option<Res<crate::defense::Kit>>,
    interaction: Option<Res<crate::loot::interaction::Kit>>,
    mut view: ResMut<DsView>,
    mut next_log: Local<f32>,
    mut warned: Local<bool>,
) {
    let (Some(roll), Some(moves)) = (roll, moves) else {
        return;
    };
    if view.clip.is_none() {
        view.clip = resolve(
            &view.want,
            &roll,
            &moves,
            reacts.as_deref(),
            defense.as_deref(),
            interaction.as_deref(),
        );
        if view.clip.is_none() {
            if !*warned {
                *warned = true;
                warn!("dsview: no cached clip is called {:?}", view.want);
            }
            return;
        }
    }
    let Some((wow, _)) = view.clip.and_then(|c| {
        clips_of(
            &roll,
            &moves,
            reacts.as_deref(),
            defense.as_deref(),
            interaction.as_deref(),
            c,
        )
    }) else {
        return;
    };
    let d = wow.duration().max(1.0e-3);
    view.t = view.hold.unwrap_or((view.t + time.delta_secs()) % d);
    *next_log -= time.delta_secs();
    if *next_log <= 0.0 {
        *next_log = 0.1;
        info!(
            "dsview {} t={:.3}/{d:.3} root={:.2} yd",
            wow.name,
            view.t,
            progress(wow, view.t)
        );
    }
}

/// Each frame, the drawn WoW blade (the held item's `$WTT`/`$WTB` markers through its pose in the
/// world) against the DS blade at the same clip time, in the body's frame; one line per loop that
/// was measured from its start, with the body standing (before that it hangs in its spawn pose).
fn measure(
    view: Res<DsView>,
    roll: Option<Res<DsRoll>>,
    moves: Option<Res<DsMoves>>,
    reacts: Option<Res<PlayerReacts>>,
    defense: Option<Res<crate::defense::Kit>>,
    interaction: Option<Res<crate::loot::interaction::Kit>>,
    avatar: Query<(&GlobalTransform, &HeldBuilt, &Motor), With<Avatar>>,
    items: Query<&GlobalTransform>,
    mut stats: Local<BladeStats>,
    mut last_t: Local<f32>,
) {
    let (Some(roll), Some(moves), Some((clip, t))) = (roll, moves, view.playing()) else {
        return;
    };
    let Some((wow, ds)) = clips_of(
        &roll,
        &moves,
        reacts.as_deref(),
        defense.as_deref(),
        interaction.as_deref(),
        clip,
    ) else {
        return;
    };
    // The loop wrapped since the last frame: report it, and start over.
    if t < *last_t {
        if stats.start.is_some_and(|start| start < 0.1) {
            info!(
                "dsview {} blade error: worst {:.1} deg at t={:.3} s, mean {:.1} deg over {} frames",
                wow.name,
                stats.worst,
                stats.at,
                stats.mean(),
                stats.frames
            );
        }
        *stats = BladeStats::default();
    }
    *last_t = t;
    let Ok((body, held, motor)) = avatar.single() else {
        return;
    };
    let (true, Some([top, bottom]), Ok(item)) = (motor.grounded, held.blade, items.get(held.root))
    else {
        return;
    };
    let drawn = item.affine().transform_vector3(top - bottom);
    let wow_blade = body.compute_transform().rotation.inverse() * drawn.normalize_or_zero();
    let ds_weapon = sample(ds, t, moves.grip_bones.map_or(DS_WEAPON_BONE, |b| b.1)).1;
    stats.add(
        t,
        blade_error_deg(
            wow_blade,
            ds_weapon,
            crate::dsclip::clip_grip(clip, &moves, defense.as_deref()).weapon,
        ),
    );
}

fn draw(
    roll: Option<Res<DsRoll>>,
    moves: Option<Res<DsMoves>>,
    reacts: Option<Res<PlayerReacts>>,
    defense: Option<Res<crate::defense::Kit>>,
    interaction: Option<Res<crate::loot::interaction::Kit>>,
    view: Res<DsView>,
    avatar: Query<(&Transform, Option<&HeldBuilt>), With<Avatar>>,
    items: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    let (Some(roll), Some(moves), Ok((body, held))) = (roll, moves, avatar.single()) else {
        return;
    };
    let Some((_, ds)) = view.clip.and_then(|c| {
        clips_of(
            &roll,
            &moves,
            reacts.as_deref(),
            defense.as_deref(),
            interaction.as_deref(),
            c,
        )
    }) else {
        return;
    };
    let origin = body.translation + *body.right() * 1.5;
    let at = |b: usize| origin + body.rotation * sample(ds, view.t, b).0;
    let gold = Color::srgb(0.95, 0.75, 0.2);
    for (i, &p) in ds.parents.iter().enumerate() {
        let joint = at(i);
        gizmos.sphere(Isometry3d::from_translation(joint), 0.025, gold);
        if let Ok(p) = usize::try_from(p) {
            gizmos.line(at(p), joint, gold);
        }
    }
    // The DS blade: the weapon bone's blade axis from the weapon bone, and again from the base of
    // the WoW sword (magenta): where the drawn blade should lie.
    let weapon_bone = moves.grip_bones.map_or(DS_WEAPON_BONE, |b| b.1);
    if weapon_bone < ds.parents.len() {
        let (_, rot, _) = sample(ds, view.t, weapon_bone);
        let dir = body.rotation * (rot * moves.grip.weapon);
        let pale = Color::srgb(1.0, 0.95, 0.6);
        let tip = at(weapon_bone) + dir * DS_BLADE_LENGTH;
        gizmos.line(at(weapon_bone), tip, pale);
        gizmos.sphere(Isometry3d::from_translation(tip), 0.03, pale);
        if let Some((Some([top, bottom]), Ok(item))) = held.map(|h| (h.blade, items.get(h.root))) {
            let base = item.transform_point(bottom);
            let length = item.affine().transform_vector3(top - bottom).length();
            let magenta = Color::srgb(1.0, 0.2, 0.9);
            gizmos.line(base, base + dir * length, magenta);
            gizmos.sphere(
                Isometry3d::from_translation(base + dir * length),
                0.03,
                magenta,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blade_error_is_the_angle_between_the_two_blades() {
        let none = Quat::IDENTITY;
        // The same direction: no error. A quarter turn and a half turn.
        assert!(blade_error_deg(Vec3::Y, none, Vec3::Y) < 1.0e-3);
        assert!((blade_error_deg(Vec3::X, none, Vec3::Y) - 90.0).abs() < 1.0e-3);
        assert!((blade_error_deg(Vec3::NEG_Y, none, Vec3::Y) - 180.0).abs() < 1.0e-3);
        // The weapon bone's rotation turns the axis first: +Y turned 90 deg about Z is -X.
        let quarter = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        assert!(blade_error_deg(Vec3::NEG_X, quarter, Vec3::Y) < 1.0e-3);
        assert!((blade_error_deg(Vec3::Y, quarter, Vec3::Y) - 90.0).abs() < 1.0e-3);
    }

    #[test]
    fn the_stats_keep_the_worst_frame_and_the_mean() {
        let mut s = BladeStats::default();
        assert_eq!(s.mean(), 0.0);
        for (t, e) in [(0.0, 2.0), (0.1, 9.0), (0.2, 4.0)] {
            s.add(t, e);
        }
        assert_eq!((s.worst, s.at, s.frames, s.start), (9.0, 0.1, 3, Some(0.0)));
        assert!((s.mean() - 5.0).abs() < 1.0e-6);
    }
}
