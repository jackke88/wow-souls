//! Bearings use the camera, not the body's facing: Bevy -Z is north and +X is east.
use bevy::prelude::*;
use std::collections::BTreeMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use wow_world::quest::{Npc, QuestBook, QuestLog, Status};

pub(super) const WIDTH: f32 = 320.0;
pub(super) const DISCOVERY_RANGE: f32 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Remains,
    Marker,
    Return(u32),
    Memorial {
        site: u32,
        tutorial: bool,
        bound: bool,
    },
}

/// The saved recoverable death spot is always navigable, including beyond the
/// spawn ring. Reading the stain directly also removes it immediately on recovery.
pub(super) fn remains(stain: &crate::bloodstain::Bloodstain) -> Option<Destination> {
    let spot = stain
        .0
        .filter(|spot| spot.souls > 0 && spot.at.is_finite())?;
    Some(Destination {
        kind: Kind::Remains,
        name: "Your remains".into(),
        quests: 0,
        sites: vec![spot.at],
    })
}

pub(super) fn marker(point: Option<Vec3>) -> Option<Destination> {
    let point = point.filter(|p| p.is_finite())?;
    Some(Destination {
        kind: Kind::Marker,
        name: "Map marker".into(),
        quests: 0,
        sites: vec![point],
    })
}

#[derive(Debug, Clone)]
pub(super) struct Destination {
    pub kind: Kind,
    pub name: String,
    pub quests: usize,
    pub sites: Vec<Vec3>,
}

pub(super) fn completed(log: &QuestLog) -> Vec<u32> {
    log.entries()
        .filter_map(|(id, e)| (e.status == Status::Complete).then_some(id))
        .collect()
}

pub(super) fn destinations(
    book: &QuestBook,
    ready: &[u32],
    spots: impl Iterator<Item = (u32, Vec3)>,
) -> Vec<Destination> {
    let mut enders: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &quest in ready {
        for npc in book.enders(quest) {
            if let Npc::Creature(entry) = npc {
                let quests = enders.entry(*entry).or_default();
                if !quests.contains(&quest) {
                    quests.push(quest);
                }
            }
        }
    }
    let mut result: BTreeMap<u32, Destination> = BTreeMap::new();
    for (entry, point) in spots {
        let Some(quests) = enders.get(&entry) else {
            continue;
        };
        if !point.is_finite() {
            continue;
        }
        result
            .entry(entry)
            .or_insert_with(|| Destination {
                kind: Kind::Return(entry),
                name: book.creature_name(entry).unwrap_or("Quest giver").into(),
                quests: quests.len(),
                sites: Vec::new(),
            })
            .sites
            .push(point);
    }
    result.into_values().collect()
}

/// Ordinary stones are discovered nearby. The accepted tutorial remains a destination
/// at any distance, without revealing every other graveyard on the map.
pub(super) fn memorials(
    binding: &crate::soulbind::Binding,
    tutorial: Option<u32>,
    player: Vec3,
) -> Vec<Destination> {
    binding
        .sites
        .iter()
        .filter_map(|site| {
            let offset = site.at - player;
            let distance = Vec2::new(offset.x, offset.z).length();
            let tutorial = tutorial == Some(site.id);
            (distance.is_finite() && (tutorial || distance <= DISCOVERY_RANGE)).then(|| {
                Destination {
                    kind: Kind::Memorial {
                        site: site.id,
                        tutorial,
                        bound: binding.bound == Some(site.id),
                    },
                    name: format!("{} Memorial", site.name),
                    quests: 1,
                    sites: vec![site.at],
                }
            })
        })
        .collect()
}

pub(super) fn heading(forward: Vec3) -> Option<f32> {
    let horizontal = Vec2::new(forward.x, -forward.z);
    (horizontal.is_finite() && horizontal.length_squared() > 1e-6)
        .then(|| horizontal.x.atan2(horizontal.y).rem_euclid(TAU))
}

pub(super) fn delta(bearing: f32, heading: f32) -> f32 {
    let d = (bearing - heading + PI).rem_euclid(TAU) - PI;
    if d <= -PI + 1e-6 {
        PI
    } else {
        d
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Edge {
    Left,
    Right,
    None,
}

pub(super) fn projection(angle: f32) -> (f32, Edge) {
    let edge = if angle < -FRAC_PI_2 {
        Edge::Left
    } else if angle > FRAC_PI_2 {
        Edge::Right
    } else {
        Edge::None
    };
    ((WIDTH * (0.5 + angle / PI)).clamp(12.0, WIDTH - 12.0), edge)
}

#[derive(Debug, Clone)]
pub(super) struct Target {
    pub destination: usize,
    pub angle: f32,
    pub distance: f32,
    pub quests: usize,
}

/// One site per NPC entry. A loaded NPC's actual position replaces its static manifest sites.
pub(super) fn targets(
    destinations: &[Destination],
    player: Vec3,
    camera: f32,
    live: &[(u32, Vec3)],
) -> Vec<Target> {
    let flat = |p: Vec3| Vec2::new(p.x - player.x, p.z - player.z).length_squared();
    destinations
        .iter()
        .enumerate()
        .filter_map(|(i, dest)| {
            let nearest_live = live
                .iter()
                .filter(|(e, p)| dest.kind == Kind::Return(*e) && p.is_finite())
                .map(|(_, p)| *p)
                .min_by(|a, b| flat(*a).total_cmp(&flat(*b)));
            let point = nearest_live.or_else(|| {
                dest.sites
                    .iter()
                    .copied()
                    .min_by(|a, b| flat(*a).total_cmp(&flat(*b)))
            })?;
            let distance = flat(point).sqrt();
            let angle =
                if !matches!(dest.kind, Kind::Marker) && distance <= crate::quests::TALK_RANGE {
                    0.0
                } else {
                    heading(point - player).map_or(0.0, |b| delta(b, camera))
                };
            Some(Target {
                destination: i,
                angle,
                distance,
                quests: dest.quests,
            })
        })
        .collect()
}

#[derive(Debug)]
pub(super) struct Group {
    pub x: f32,
    pub edge: Edge,
    pub count: usize,
    pub members: Vec<usize>,
}

/// Closest destination leads each group. Edge arrows still work when all goals are behind us.
/// A personal map marker (`quests == 0`) stays in its own slot so nearby memorials or quest
/// NPCs never swallow its icon.
pub(super) fn groups(targets: &[Target]) -> Vec<Group> {
    let mut order: Vec<_> = (0..targets.len()).collect();
    order.sort_by(|&a, &b| targets[a].distance.total_cmp(&targets[b].distance));
    let mut groups: Vec<Group> = Vec::new();
    for i in order {
        let (x, edge) = projection(targets[i].angle);
        let is_marker = targets[i].quests == 0;
        if let Some(group) = groups
            .iter_mut()
            .find(|g| !is_marker && g.count > 0 && (g.x - x).abs() < 28.0)
        {
            group.members.push(i);
            group.count += targets[i].quests;
            if group.edge == Edge::None {
                group.edge = edge;
            }
        } else {
            groups.push(Group {
                x,
                edge,
                count: targets[i].quests,
                members: vec![i],
            });
        }
    }
    if let Some(m) = groups.iter().position(|g| g.count == 0) {
        let mx = groups[m].x;
        if groups
            .iter()
            .enumerate()
            .any(|(i, g)| i != m && (g.x - mx).abs() < 22.0)
        {
            let step = if mx <= WIDTH * 0.5 { 22.0 } else { -22.0 };
            groups[m].x = (mx + step).clamp(12.0, WIDTH - 12.0);
        }
    }
    groups.sort_by(|a, b| a.x.total_cmp(&b.x));
    groups
}

pub(super) fn focus(targets: &[Target]) -> Option<usize> {
    let ahead = targets
        .iter()
        .enumerate()
        .filter(|(_, t)| t.angle.abs() <= 12_f32.to_radians())
        .min_by(|(_, a), (_, b)| {
            a.angle
                .abs()
                .total_cmp(&b.angle.abs())
                .then(a.distance.total_cmp(&b.distance))
        })
        .map(|(i, _)| i);
    ahead.or_else(|| {
        targets
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (a.quests > 0)
                    .cmp(&(b.quests > 0))
                    .then(a.distance.total_cmp(&b.distance))
            })
            .map(|(i, _)| i)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::quest::save::SaveGame;

    #[test]
    fn saved_remains_guide_from_far_away_and_follow_replacement_and_recovery() {
        use crate::bloodstain::{self, Bloodstain, Stain};
        let mut stain = Bloodstain(Some(Stain {
            at: Vec3::new(0.0, 7.0, 1000.0),
            souls: 100,
        }));
        let destination = remains(&stain).unwrap();
        let target = targets(&[destination], Vec3::ZERO, 0.0, &[(1, Vec3::X)]).remove(0);
        assert_eq!(target.distance, 1000.0);
        assert_eq!(projection(target.angle).1, Edge::Right);
        let mut carried = 50;
        bloodstain::die(&mut carried, &mut stain.0, Vec3::new(500.0, 0.0, 0.0));
        assert_eq!(
            remains(&stain).unwrap().sites,
            vec![Vec3::new(500.0, 0.0, 0.0)]
        );
        bloodstain::die(&mut carried, &mut stain.0, Vec3::ZERO);
        assert_eq!(remains(&stain).unwrap().sites[0].x, 500.0);
        assert_eq!(bloodstain::recover(&mut carried, &mut stain.0), 50);
        assert!(remains(&stain).is_none());
    }

    #[test]
    fn cardinal_bearings_wrap_and_behind_goals_stay_on_the_correct_side() {
        for (v, want) in [
            (-Vec3::Z, 0.0),
            (Vec3::X, FRAC_PI_2),
            (Vec3::Z, PI),
            (-Vec3::X, PI * 1.5),
        ] {
            assert!((heading(v).unwrap() - want).abs() < 1e-5);
        }
        assert!(heading(Vec3::Y).is_none());
        assert!(heading(Vec3::splat(f32::NAN)).is_none());
        assert!(
            (delta(1_f32.to_radians(), 359_f32.to_radians()) - 2_f32.to_radians()).abs() < 1e-5
        );
        assert!(
            (delta(359_f32.to_radians(), 1_f32.to_radians()) + 2_f32.to_radians()).abs() < 1e-5
        );
        assert_eq!(projection(-PI).1, Edge::Left);
        assert_eq!(projection(PI).1, Edge::Right);
        assert_eq!(projection(0.0), (WIDTH * 0.5, Edge::None));
        assert!((delta(PI, 0.0) - PI).abs() < 1e-5);
    }

    #[test]
    fn only_complete_quests_produce_real_enders_and_shared_npcs_are_not_duplicated() {
        let save = SaveGame::from_text("wsquests 1\nname\tCompasscheck\nxp\t400\nmoney\t0\nquest\t7\tcomplete\t10 0 0 0\t0 0 0 0\nquest\t783\tcomplete\t0 0 0 0\t0 0 0 0\nquest\t33\tincomplete\t0 0 0 0\t0 0 0 0\nquest\t18\trewarded\t0 0 0 0\t0 0 0 0\n").unwrap();
        assert_eq!(completed(&save.log), vec![7, 783]);
        let Some(world) = crate::quests::testing::real_world() else {
            return;
        };
        let book = QuestBook::new(&world);
        let sites = vec![
            (197, Vec3::new(10.0, 0.0, -10.0)),
            (197, Vec3::new(100.0, 0.0, 0.0)),
            (823, Vec3::ZERO),
        ];
        let dests = destinations(&book, &completed(&save.log), sites.clone().into_iter());
        assert_eq!(dests.len(), 1);
        assert_eq!(
            (dests[0].kind, dests[0].quests, dests[0].sites.len()),
            (Kind::Return(197), 2, 2)
        );
        let after =
            SaveGame::from_text("wsquests 1\nname\tCompasscheck\nxp\t400\nmoney\t0\nquest\t7\trewarded\t10 0 0 0\t0 0 0 0\n").unwrap();
        assert!(destinations(&book, &completed(&after.log), sites.into_iter()).is_empty());
    }

    #[test]
    fn live_npc_arrival_grouping_and_focus_follow_the_camera_not_the_body() {
        let dests = vec![Destination {
            kind: Kind::Return(197),
            name: "Marshal".into(),
            quests: 2,
            sites: vec![Vec3::new(100.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -30.0)],
        }];
        let t = targets(&dests, Vec3::ZERO, 0.0, &[]);
        assert_eq!(t[0].angle, 0.0);
        assert_eq!(t[0].distance, 30.0);
        let t = targets(
            &dests,
            Vec3::ZERO,
            FRAC_PI_2,
            &[(197, Vec3::new(0.0, 10.0, 2.0))],
        );
        assert_eq!((t[0].angle, t[0].distance), (0.0, 2.0));
        let t = vec![
            Target {
                destination: 0,
                angle: 0.0,
                distance: 30.0,
                quests: 2,
            },
            Target {
                destination: 1,
                angle: 0.05,
                distance: 40.0,
                quests: 1,
            },
            Target {
                destination: 2,
                angle: PI,
                distance: 50.0,
                quests: 1,
            },
        ];
        let groups = groups(&t);
        assert_eq!(groups.len(), 2);
        assert_eq!((groups[0].count, groups[0].members.len()), (3, 2));
        assert_eq!(groups[1].edge, Edge::Right);
        assert_eq!(focus(&t), Some(0));
        assert_eq!(focus(&[]), None);
    }

    #[test]
    fn graves_are_discovered_nearby_but_the_tutorial_guides_from_far_away() {
        let mut binding = crate::soulbind::Binding::default();
        binding.sites = vec![
            crate::soulbind::Site {
                id: 1,
                name: "Northshire Valley".into(),
                at: Vec3::new(0.0, 0.0, -500.0),
                yaw: 0.0,
            },
            crate::soulbind::Site {
                id: 2,
                name: "Goldshire".into(),
                at: Vec3::new(DISCOVERY_RANGE, 20.0, 0.0),
                yaw: 0.0,
            },
        ];
        // There is no quest log here: ordinary discoveries do not depend on a quest.
        let near = memorials(&binding, None, Vec3::ZERO);
        assert_eq!(near.len(), 1);
        assert_eq!(
            near[0].kind,
            Kind::Memorial {
                site: 2,
                tutorial: false,
                bound: false
            }
        );
        assert!(memorials(&binding, None, Vec3::new(-0.1, 0.0, 0.0)).is_empty());
        binding.bound = Some(2);
        let guided = memorials(&binding, Some(1), Vec3::ZERO);
        assert_eq!(guided.len(), 2);
        assert_eq!(
            guided[0].kind,
            Kind::Memorial {
                site: 1,
                tutorial: true,
                bound: false
            }
        );
        assert_eq!(
            guided[1].kind,
            Kind::Memorial {
                site: 2,
                tutorial: false,
                bound: true
            }
        );
        // A live NPC cannot move the stationary memorial's bearing.
        let t = targets(&guided, Vec3::ZERO, 0.0, &[(6491, Vec3::X)]);
        assert_eq!((t[0].angle, t[0].distance), (0.0, 500.0));
        assert!((t[1].angle - FRAC_PI_2).abs() < 1e-5);
        assert_eq!(t[1].distance, DISCOVERY_RANGE);
        assert!(memorials(&binding, Some(1), Vec3::splat(f32::NAN)).is_empty());
    }

    #[test]
    fn personal_map_marker_stays_separate_from_quest_and_memorial_groups() {
        assert!(marker(None).is_none());
        assert!(marker(Some(Vec3::splat(f32::NAN))).is_none());
        let pin = marker(Some(Vec3::new(0.0, 0.0, -250.0))).unwrap();
        assert_eq!((pin.kind, pin.quests), (Kind::Marker, 0));
        let dests = vec![
            Destination {
                kind: Kind::Memorial {
                    site: 1,
                    tutorial: false,
                    bound: true,
                },
                name: "Northshire Memorial".into(),
                quests: 1,
                sites: vec![Vec3::new(-40.0, 0.0, 40.0)],
            },
            pin,
        ];
        // Both targets are behind/left when looking north (0.0), so both clamp to Edge::Left.
        let both_left = vec![
            Target {
                destination: 0,
                angle: -2.2,
                distance: 56.0,
                quests: 1,
            },
            Target {
                destination: 1,
                angle: -2.4,
                distance: 250.0,
                quests: 0,
            },
        ];
        let g = groups(&both_left);
        assert_eq!(
            g.len(),
            2,
            "map marker is not swallowed by a closer memorial"
        );
        assert!(
            (g[0].x - g[1].x).abs() >= 20.0,
            "overlapping edge icons step apart"
        );
        assert_eq!(
            focus(&both_left),
            Some(1),
            "map marker takes fallback caption when nothing is centered"
        );
        let looking_at_memorial = vec![
            Target {
                destination: 0,
                angle: 0.05,
                distance: 56.0,
                quests: 1,
            },
            Target {
                destination: 1,
                angle: 1.0,
                distance: 250.0,
                quests: 0,
            },
        ];
        assert_eq!(
            focus(&looking_at_memorial),
            Some(0),
            "looking directly at another compass icon still focuses it"
        );
        let _ = dests;
    }
}
