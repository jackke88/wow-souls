//! Optional, fixed treasures on detours through Northshire and Elwynn.
//! Exact rewards, class coverage and placement evidence: `docs/ELWYNN-TREASURES.md`.
use wow_world::model::{ItemCount, World};

use super::{ABBEY_CHEST, SALVE};

pub(super) struct Treasure {
    pub id: u32,
    pub name: &'static str,
    pub wow: [f32; 3],
    pub yaw: f32,
    pub items: &'static [ItemCount],
}

impl Treasure {
    /// A smaller/custom world manifest must not offer items it cannot describe or equip.
    pub fn available(&self, world: &World) -> bool {
        self.items
            .iter()
            .all(|&(id, _)| id == SALVE || world.items.iter().any(|item| item.entry == id))
    }
}

pub(super) const TREASURES: &[Treasure] = &[
    Treasure {
        // This ID previously stood in the Abbey doorway. Moving it preserves existing claims.
        id: ABBEY_CHEST,
        name: "Abbey supply cache",
        wow: [-8896.0, -235.0, 80.3],
        yaw: 0.0,
        items: &[(SALVE, 3)],
    },
    Treasure {
        // The back of the canvas shelter, on the woods side of its boxes and barrels.
        id: 9_000_002,
        name: "Woodland supply cache",
        wow: [-8768.0, -170.0, 83.0],
        yaw: std::f32::consts::FRAC_PI_2,
        items: &[(SALVE, 2)],
    },
    Treasure {
        // Beyond the supplies at the clearing's edge, reached by circling the camp.
        id: 9_000_003,
        name: "Camp reserve cache",
        wow: [-8786.0, -254.0, 83.0],
        yaw: -std::f32::consts::FRAC_PI_2,
        items: &[(SALVE, 2)],
    },
    Treasure {
        id: 9_000_010,
        name: "Goldshire supply cache",
        wow: [-9434.0, 93.0, 58.0],
        yaw: 0.0,
        items: &[(SALVE, 3)],
    },
    Treasure {
        id: 9_000_011,
        name: "Mirror Lake reserve",
        wow: [-9460.34, 280.899, 53.0244],
        yaw: 0.0,
        items: &[(SALVE, 3), (774, 1)],
    },
    Treasure {
        id: 9_000_012,
        name: "Stonefield tool chest",
        wow: [-9880.0, 350.0, 37.0],
        yaw: 0.0,
        items: &[(2488, 1), (SALVE, 2)],
    },
    Treasure {
        id: 9_000_013,
        name: "Maclure orchard cache",
        wow: [-10017.0, 62.0, 35.0],
        yaw: 0.0,
        items: &[(2495, 1), (SALVE, 2)],
    },
    Treasure {
        id: 9_000_014,
        name: "Crystal Lake shore cache",
        wow: [-9484.18, -349.415, 59.8835],
        yaw: 2.70526,
        items: &[(4683, 1)],
    },
    Treasure {
        // Native mine chest pockets anchor the authored rewards, never the surface above them.
        id: 9_000_015,
        name: "Jasperlode blade cache",
        wow: [-9051.11, -625.67, 53.4202],
        yaw: 1.36136,
        items: &[(727, 1)],
    },
    Treasure {
        id: 9_000_016,
        name: "Fargodeep forgotten coffer",
        wow: [-9828.59, 87.2501, 0.875297],
        yaw: -0.471239,
        items: &[(6511, 1)],
    },
    Treasure {
        id: 9_000_017,
        name: "Brackwell reserve chest",
        wow: [-9711.93, -943.498, 38.4165],
        yaw: 3.08923,
        items: &[(2975, 1)],
    },
    Treasure {
        id: 9_000_018,
        name: "Forest's Edge provisions",
        wow: [-9983.97, 568.819, 37.7626],
        yaw: 0.0,
        items: &[(SALVE, 4), (818, 1)],
    },
    Treasure {
        id: 9_000_019,
        name: "Stone Cairn hidden coffer",
        wow: [-8881.7, -761.708, 68.4141],
        yaw: 3.05433,
        items: &[(3307, 1)],
    },
    Treasure {
        id: 9_000_020,
        name: "Eastvale lumber cache",
        wow: [-9478.0, -1304.0, 42.0],
        yaw: 0.0,
        items: &[(SALVE, 3), (818, 1)],
    },
    Treasure {
        id: 9_000_021,
        name: "Ridgepoint watchman's coffer",
        wow: [-9878.0, -1378.0, 33.3],
        yaw: 0.0,
        items: &[(3652, 1)],
    },
    Treasure {
        id: 9_000_022,
        name: "Jerod's Landing supplies",
        wow: [-9955.09, -128.517, 25.3672],
        yaw: -0.767945,
        items: &[(SALVE, 3), (774, 1)],
    },
    Treasure {
        id: 9_000_023,
        name: "Westbrook patrol coffer",
        wow: [-9645.0, 708.0, 37.55],
        yaw: 2.3561945,
        items: &[(2965, 1)],
    },
    Treasure {
        id: 9_000_024,
        name: "Fargodeep miner's strongbox",
        wow: [-9866.26, 221.78, 14.0336],
        yaw: -2.74017,
        items: &[(2962, 1)],
    },
    Treasure {
        id: 9_000_025,
        name: "Jasperlode jeweled strongbox",
        wow: [-9021.75, -606.646, 56.4091],
        yaw: 2.61799,
        items: &[(1917, 1)],
    },
    Treasure {
        id: 9_000_026,
        // The east bank is reachable on foot; reserve the island for when swimming exists.
        name: "Stone Cairn ranger's coffer",
        wow: [-9267.23, -1180.15, 70.147],
        yaw: 0.436332,
        items: &[(2980, 1)],
    },
    Treasure {
        id: 9_000_027,
        name: "Azora traveller's cache",
        wow: [-9548.0, -686.0, 63.0],
        yaw: 0.0,
        items: &[(SALVE, 3), (774, 1)],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn caches_have_independent_ids_and_smaller_manifests_hide_unknown_rewards() {
        let mut ids = BTreeSet::new();
        let empty = World::default();
        for t in TREASURES {
            assert!(ids.insert(t.id), "duplicate chest {}", t.id);
            assert!(t.wow.iter().all(|v| v.is_finite()));
            assert!(!t.items.is_empty());
            assert!(t.items.iter().all(|&(_, n)| n > 0));
            assert_eq!(
                t.available(&empty),
                t.items.iter().all(|&(id, _)| id == SALVE)
            );
        }
        assert_eq!(TREASURES.len(), 21, "three Northshire + eighteen Elwynn");
    }

    #[test]
    fn imported_treasure_gear_covers_every_vanilla_class_by_level_ten() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/world/elwynn.txt");
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("skip treasure metadata check: import Elwynn first");
            return;
        };
        let world = wow_world::manifest::parse(&text).unwrap();
        let gear: Vec<_> = TREASURES[3..]
            .iter()
            .flat_map(|t| {
                assert!(t.available(&world), "missing imported item in {}", t.name);
                t.items.iter().filter_map(|&(id, _)| {
                    world
                        .items
                        .iter()
                        .find(|i| i.entry == id && i.quality == 2 && i.class == 4)
                })
            })
            .collect();
        // Actual armor proficiencies and useful primary stats, not can_wear's still-permissive
        // fallback for classes that are not yet playable. Hunters/Shamans use leather below 40.
        for (class, armor, stat) in [
            ("Warrior", 3, 4),
            ("Paladin", 3, 4),
            ("Hunter", 2, 3),
            ("Rogue", 2, 3),
            ("Priest", 1, 5),
            ("Shaman", 2, 3),
            ("Mage", 1, 5),
            ("Warlock", 1, 5),
            ("Druid", 2, 3),
        ] {
            assert!(
                gear.iter().any(|i| i.subclass == armor
                    && i.required_level <= 10
                    && i.allowable_class == -1
                    && i.allowable_race == -1
                    && i.stats
                        .iter()
                        .any(|&(kind, value)| kind == stat && value > 0)),
                "no useful green armor for {class}"
            );
        }
        use wow_world::equipment::{can_wear, Wearer};
        for (class, id) in [(1, 727), (1, 2965), (8, 6511), (8, 3307)] {
            let item = world.items.iter().find(|i| i.entry == id).unwrap();
            assert!(can_wear(item, Wearer::new(1, class, 10)).is_ok());
            assert!(can_wear(item, Wearer::new(1, class, item.required_level - 1)).is_err());
        }
    }
}
