//! The vertical slice's place in the world: Northshire Valley, map 0, inside the tiles the engine
//! streams by default (`benilla_world::terrain_stream::SPAWN_XY`).

use crate::species::SpeciesDef;

/// The slice anchor in WoW world coords (x north, y west, z up, yards): the path in front of
/// Northshire Abbey's main doors.
pub const ANCHOR: [f32; 3] = [-8913.0, -136.0, 82.0];

/// `WOWSOULS_AT=<x>,<y>[,<z>]` moves the anchor (raw WoW coords; `z` defaults to the anchor's).
/// Without it a character with a start of its own ([`crate::saves::start`]) starts there.
pub fn anchor() -> [f32; 3] {
    std::env::var("WOWSOULS_AT")
        .ok()
        .and_then(|v| parse_wow_point(&v, ANCHOR[2]))
        .or_else(|| crate::saves::start().map(|s| s.at))
        .unwrap_or(ANCHOR)
}

/// The yaw (radians, Bevy's: 0 faces north) the body starts with: toward the start's `face` point;
/// north otherwise, and with `WOWSOULS_AT` (which has no facing).
pub fn start_yaw() -> f32 {
    if std::env::var_os("WOWSOULS_AT").is_some() {
        return 0.0;
    }
    match crate::saves::start() {
        Some(crate::saves::Start {
            at,
            face: Some([x, y]),
        }) => yaw_toward(at, [x, y]),
        _ => 0.0,
    }
}

/// The yaw that turns a body at `from` (WoW coordinates) to face the ground point `to`.
pub fn yaw_toward(from: [f32; 3], to: [f32; 2]) -> f32 {
    // Bevy: forward is -Z (WoW north), left is -X (WoW west is +Y), so `(-dx_bevy).atan2(-dz_bevy)`
    // with bevy x = -wow y, bevy z = -wow x.
    let (north, west) = (to[0] - from[0], to[1] - from[1]);
    let (dx, dz) = (-west, -north);
    if dx.abs() + dz.abs() < 1.0e-4 {
        return 0.0;
    }
    (-dx).atan2(-dz)
}

/// A group of creatures of one species: where the first stands, where the others stand from it
/// (as (north, west) yards, WoW axes), and what the environment switches are called.
pub struct Group {
    pub species: &'static SpeciesDef,
    /// Where the first stands, in WoW coords. The height is a seed; the spawn snaps it to the ground.
    pub first: [f32; 3],
    pub extra: &'static [[f32; 2]],
    /// `WOWSOULS_<NAME>_AT=<x>,<y>[,<z>]` moves the first.
    pub at_var: &'static str,
    /// `WOWSOULS_<NAMES>=<n>` says how many stand.
    pub count_var: &'static str,
    /// How many stand in an instrumented run: the scripted checks in `docs/HANDOVER.md` all assume
    /// a lone kobold and no one else, so the kobolds are 1 and the others 0 (a placed first creature
    /// always makes it 1).
    pub scripted: usize,
}

/// Where the kobold stands: open ground north of the Abbey, about 25 yd up the path from the
/// anchor, so the player walks into its range; the other kobolds stand across the open meadow, 8 to
/// 15 yd apart, so a fight can be pulled one kobold at a time.
pub const KOBOLD_AT: [f32; 3] = [-8888.0, -132.0, 82.0];
pub const EXTRA_KOBOLDS: [[f32; 2]; 3] = [[8.0, 8.0], [-6.0, 14.0], [14.0, 2.0]];

/// The wolves: open meadow north-west of the kobolds, about 45 yd from the anchor, so the player
/// finds them after the kobolds.
pub const WOLF_AT: [f32; 3] = [-8898.0, -94.0, 82.0];
pub const EXTRA_WOLVES: [[f32; 2]; 1] = [[-6.0, 14.0]];

/// The Defias Thugs: north-east of the kobolds, about 25 yd from the nearest of them.
pub const THUG_AT: [f32; 3] = [-8874.0, -100.0, 82.0];
pub const EXTRA_THUGS: [[f32; 2]; 1] = [[10.0, 12.0]];

/// The Kobold Vermin, the Workers and the Young Wolves: candidate meadow spots, to be checked by
/// picture.
pub const VERMIN_AT: [f32; 3] = [-8860.0, -160.0, 82.0];
pub const EXTRA_VERMIN: [[f32; 2]; 2] = [[8.0, 6.0], [-4.0, 12.0]];
pub const WORKER_AT: [f32; 3] = [-8850.0, -125.0, 82.0];
pub const EXTRA_WORKERS: [[f32; 2]; 1] = [[8.0, 8.0]];
pub const YOUNG_WOLF_AT: [f32; 3] = [-8930.0, -70.0, 82.0];
pub const EXTRA_YOUNG_WOLVES: [[f32; 2]; 1] = [[-5.0, 8.0]];

/// Garrick Padfoot stands alone, north-west of the thugs, off the wolves' ground: a candidate spot,
/// to be checked by picture.
pub const GARRICK_AT: [f32; 3] = [-8856.0, -64.0, 82.0];

/// Hogger, for a fight in the valley (his real place is the world manifest's): west-south-west of
/// the anchor, 20 yd or more from every other group (`the_groups_stand_clear_of_each_other` holds
/// that), a candidate spot to be checked by picture. Off unless `WOWSOULS_HOGGERS` or
/// `WOWSOULS_HOGGER_AT` is set (the group is not part of the default slice).
pub const HOGGER_AT: [f32; 3] = [-8930.0, -112.0, 82.0];

/// Every group the slice has.
pub const GROUPS: [Group; 8] = [
    Group {
        species: &crate::species::KOBOLD,
        first: KOBOLD_AT,
        extra: &EXTRA_KOBOLDS,
        at_var: "WOWSOULS_KOBOLD_AT",
        count_var: "WOWSOULS_KOBOLDS",
        scripted: 1,
    },
    Group {
        species: &crate::species::WOLF,
        first: WOLF_AT,
        extra: &EXTRA_WOLVES,
        at_var: "WOWSOULS_WOLF_AT",
        count_var: "WOWSOULS_WOLVES",
        scripted: 0,
    },
    Group {
        species: &crate::species::THUG,
        first: THUG_AT,
        extra: &EXTRA_THUGS,
        at_var: "WOWSOULS_THUG_AT",
        count_var: "WOWSOULS_THUGS",
        scripted: 0,
    },
    Group {
        species: &crate::species::VERMIN,
        first: VERMIN_AT,
        extra: &EXTRA_VERMIN,
        at_var: "WOWSOULS_VERMIN_AT",
        count_var: "WOWSOULS_VERMIN",
        scripted: 0,
    },
    Group {
        species: &crate::species::WORKER,
        first: WORKER_AT,
        extra: &EXTRA_WORKERS,
        at_var: "WOWSOULS_WORKER_AT",
        count_var: "WOWSOULS_WORKERS",
        scripted: 0,
    },
    Group {
        species: &crate::species::YOUNG_WOLF,
        first: YOUNG_WOLF_AT,
        extra: &EXTRA_YOUNG_WOLVES,
        at_var: "WOWSOULS_YOUNG_WOLF_AT",
        count_var: "WOWSOULS_YOUNG_WOLVES",
        scripted: 0,
    },
    Group {
        species: &crate::species::GARRICK,
        first: GARRICK_AT,
        extra: &[],
        at_var: "WOWSOULS_GARRICK_AT",
        count_var: "WOWSOULS_GARRICKS",
        scripted: 0,
    },
    Group {
        species: &crate::species::HOGGER,
        first: HOGGER_AT,
        extra: &[],
        at_var: "WOWSOULS_HOGGER_AT",
        count_var: "WOWSOULS_HOGGERS",
        scripted: 0,
    },
];

/// The boss group is not part of the default slice: it stands only when asked for.
pub fn stays_off(species: &str, asked: bool) -> bool {
    species == "hogger" && !asked
}

/// How many of a group of `all` stand, from the count variable `var`, whether the run is
/// unattended (`scripted` stand) and whether it moved the first (one stands); a person playing
/// gets the whole group.
pub fn count_for(
    var: Option<&str>,
    unattended: bool,
    moved: bool,
    all: usize,
    scripted: usize,
) -> usize {
    match var.and_then(|v| v.trim().parse::<usize>().ok()) {
        Some(n) => n.clamp(0, all),
        None if moved => 1,
        None if unattended => scripted.min(all),
        None => all,
    }
}

/// The feet of a group in WoW coords: the first at its `_AT` variable or [`Group::first`], the
/// rest at [`Group::extra`] from it.
pub fn group_spots(g: &Group) -> Vec<[f32; 3]> {
    let asked = std::env::var_os(g.count_var).is_some() || std::env::var_os(g.at_var).is_some();
    if stays_off(g.species.name, asked) {
        return Vec::new();
    }
    let first = std::env::var(g.at_var)
        .ok()
        .and_then(|v| parse_wow_point(&v, g.first[2]))
        .unwrap_or(g.first);
    let n = count_for(
        std::env::var(g.count_var).ok().as_deref(),
        crate::run_mode::unattended(),
        std::env::var_os(g.at_var).is_some(),
        1 + g.extra.len(),
        g.scripted,
    );
    std::iter::once([0.0, 0.0])
        .chain(g.extra.iter().copied())
        .take(n)
        .map(|[dx, dy]| [first[0] + dx, first[1] + dy, first[2]])
        .collect()
}

/// Who stands where: each creature's species and feet in WoW coords, group by group (the
/// remaining species arrive with their kits, spec `2026-10-09-enemy-roster-design.md`).
pub fn spawn_plan() -> Vec<(&'static SpeciesDef, [f32; 3])> {
    GROUPS
        .iter()
        .flat_map(|g| group_spots(g).into_iter().map(|at| (g.species, at)))
        .collect()
}

/// `x,y` or `x,y,z`, finite numbers only; `default_z` fills a missing `z`.
pub fn parse_wow_point(v: &str, default_z: f32) -> Option<[f32; 3]> {
    let parts: Vec<f32> = v
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect::<Option<_>>()?;
    match parts[..] {
        [x, y] => Some([x, y, default_z]),
        [x, y, z] => Some([x, y, z]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_start_faces_the_point_it_names() {
        let here = [-9900.0, 500.0, 40.0];
        let yaw_of = |to: [f32; 2]| yaw_toward(here, to);
        // North is 0 (Bevy -Z), west a quarter turn to the left, south a half, east to the right.
        assert!(yaw_of([-9890.0, 500.0]).abs() < 1.0e-5);
        assert!((yaw_of([-9900.0, 510.0]) - std::f32::consts::FRAC_PI_2).abs() < 1.0e-5);
        assert!((yaw_of([-9910.0, 500.0]).abs() - std::f32::consts::PI).abs() < 1.0e-5);
        assert!((yaw_of([-9900.0, 490.0]) + std::f32::consts::FRAC_PI_2).abs() < 1.0e-5);
        // Facing where it stands is no turn.
        assert_eq!(yaw_of([here[0], here[1]]), 0.0);
    }

    fn kobolds(var: Option<&str>, unattended: bool, moved: bool) -> usize {
        count_for(var, unattended, moved, 1 + EXTRA_KOBOLDS.len(), 1)
    }

    fn wolves(var: Option<&str>, unattended: bool, moved: bool) -> usize {
        count_for(var, unattended, moved, 1 + EXTRA_WOLVES.len(), 0)
    }

    #[test]
    fn two_components_take_the_default_height() {
        assert_eq!(
            parse_wow_point("-8913, -136", 82.0),
            Some([-8913.0, -136.0, 82.0])
        );
    }

    #[test]
    fn three_components_are_taken_as_given() {
        assert_eq!(parse_wow_point("1,2,3", 82.0), Some([1.0, 2.0, 3.0]));
    }

    #[test]
    fn junk_is_rejected_whole() {
        assert_eq!(parse_wow_point("1,x,3", 0.0), None);
        assert_eq!(parse_wow_point("1", 0.0), None);
        assert_eq!(parse_wow_point("1,2,3,4", 0.0), None);
        assert_eq!(parse_wow_point("1,NaN", 0.0), None);
    }

    #[test]
    fn a_person_playing_meets_the_whole_group() {
        assert_eq!(kobolds(None, false, false), 4);
    }

    #[test]
    fn an_instrumented_run_or_a_placed_kobold_stays_alone() {
        assert_eq!(kobolds(None, true, false), 1);
        assert_eq!(kobolds(None, false, true), 1);
    }

    #[test]
    fn wolves_stand_for_a_person_but_not_in_a_scripted_run() {
        assert_eq!(wolves(None, false, false), 2);
        assert_eq!(wolves(None, true, false), 0);
        assert_eq!(wolves(None, true, true), 1, "a placed wolf stands");
        assert_eq!(wolves(Some("5"), true, false), 2);
        assert_eq!(wolves(Some("0"), false, false), 0);
    }

    #[test]
    fn the_hogger_group_is_off_unless_asked() {
        assert!(stays_off("hogger", false));
        assert!(!stays_off("hogger", true));
        // Every other group stands as before.
        assert!(!stays_off("kobold", false));
        assert!(!stays_off("garrick", false));
    }

    #[test]
    fn the_groups_stand_clear_of_each_other() {
        // Within a group the creatures stand apart by their own rule; between groups at least
        // 20 yd (the aggro range), so meeting one group does not pull another.
        let spots = |g: &Group| -> Vec<[f32; 2]> {
            std::iter::once([0.0, 0.0])
                .chain(g.extra.iter().copied())
                .map(|[dx, dy]| [g.first[0] + dx, g.first[1] + dy])
                .collect()
        };
        for (i, a) in GROUPS.iter().enumerate() {
            for b in &GROUPS[i + 1..] {
                for p in spots(a) {
                    for q in spots(b) {
                        let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt();
                        assert!(
                            d >= 20.0,
                            "a {} at {p:?} is {d} yd from a {} at {q:?}",
                            a.species.name,
                            b.species.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_scripted_run_has_the_kobold_alone_and_a_placed_creature_stands() {
        assert_eq!(count_for(None, true, false, 4, 1), 1);
        assert_eq!(count_for(None, true, false, 2, 0), 0);
        assert_eq!(count_for(None, true, true, 2, 0), 1);
        assert_eq!(count_for(None, false, false, 2, 0), 2);
        assert_eq!(count_for(Some("9"), true, false, 2, 0), 2);
    }

    #[test]
    fn the_variable_overrides_and_is_clamped() {
        assert_eq!(kobolds(Some("3"), true, true), 3);
        assert_eq!(kobolds(Some(" 2 "), false, false), 2);
        assert_eq!(kobolds(Some("99"), false, false), 4);
        assert_eq!(kobolds(Some("0"), false, false), 0);
        assert_eq!(kobolds(Some("many"), false, false), 4);
        assert_eq!(kobolds(Some("many"), true, false), 1);
    }

    #[test]
    fn the_extras_stand_apart_from_each_other() {
        let spots: Vec<[f32; 2]> = std::iter::once([0.0, 0.0]).chain(EXTRA_KOBOLDS).collect();
        for (i, a) in spots.iter().enumerate() {
            for b in &spots[i + 1..] {
                let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
                assert!(d >= 8.0, "{a:?} and {b:?} are {d} yd apart");
            }
        }
    }

    #[test]
    fn the_anchor_is_in_northshire() {
        // Within 100 yd of the engine's own boot anchor, so the default stream window holds it.
        let (sx, sy) = benilla_world::terrain_stream::SPAWN_XY;
        assert!((ANCHOR[0] - sx).abs() < 100.0 && (ANCHOR[1] - sy).abs() < 100.0);
    }
}
