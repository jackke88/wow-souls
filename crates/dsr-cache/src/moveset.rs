//! The moves of a weapon class: which Dark Souls animation is which move. Hand-made, like the bone
//! map: the animation id scheme is `a<motion category>_<hand + action>` (hand 3000 one-handed;
//! action 000-002 the light chain, 300/301/310 heavy, 900 after a roll).

use anyhow::{ensure, Result};

use crate::params::Params;

pub struct Family {
    pub name: &'static str,
    pub weapon: i32,
    pub two_handed: bool,
    pub chain: u8,
}

/// WoW subclasses select these counterparts. Staff is an explicit polearm adaptation.
pub const FAMILIES: &[Family] = &[
    Family {
        name: "dagger",
        weapon: 100000,
        two_handed: false,
        chain: 3,
    },
    Family {
        name: "straight_sword",
        weapon: 200000,
        two_handed: false,
        chain: 3,
    },
    Family {
        name: "straight_sword_2h",
        weapon: 200000,
        two_handed: true,
        chain: 3,
    },
    Family {
        name: "dagger_2h",
        weapon: 100000,
        two_handed: true,
        chain: 3,
    },
    Family {
        name: "greatsword",
        weapon: 300000,
        two_handed: true,
        chain: 3,
    },
    Family {
        name: "axe",
        weapon: 701000,
        two_handed: false,
        chain: 2,
    },
    Family {
        name: "axe_2h",
        weapon: 701000,
        two_handed: true,
        chain: 2,
    },
    Family {
        name: "greataxe",
        weapon: 750000,
        two_handed: true,
        chain: 2,
    },
    Family {
        name: "mace",
        weapon: 801000,
        two_handed: false,
        chain: 3,
    },
    Family {
        name: "mace_2h",
        weapon: 801000,
        two_handed: true,
        chain: 3,
    },
    Family {
        name: "great_hammer",
        weapon: 852000,
        two_handed: true,
        chain: 2,
    },
    Family {
        name: "polearm",
        weapon: 1103000,
        two_handed: true,
        chain: 3,
    },
    Family {
        name: "staff",
        weapon: 1103000,
        two_handed: true,
        chain: 3,
    },
    Family {
        name: "fist",
        weapon: 901000,
        two_handed: false,
        chain: 3,
    },
    Family {
        name: "unarmed",
        weapon: 900000,
        two_handed: false,
        chain: 3,
    },
];

impl Family {
    pub fn moves(&self, params: &Params) -> Result<Vec<MoveOwned>> {
        let category = params.weapon(self.weapon)?.motion_category;
        let special = params.special_attack(self.weapon)?;
        let hand = if self.two_handed { 4000 } else { 3000 };
        Ok([
            ("r1_1", 0),
            ("r1_2", 1),
            ("r1_3", 2),
            ("r2_1", 300),
            ("r2_2", 310),
            ("roll_attack", 900),
        ]
        .into_iter()
        .map(|(name, action)| {
            let set = if special != 0 && (300..=310).contains(&action) {
                special
            } else {
                category
            };
            MoveOwned {
                name,
                clip: format!("a{set:02}_{:04}", hand + action),
            }
        })
        .collect())
    }
}

pub struct MoveOwned {
    pub name: &'static str,
    pub clip: String,
}

/// Read-only evidence from the installed game, including missing behavior references.
pub fn survey(dir: &std::path::Path) -> Result<()> {
    let params = crate::params::load(dir)?;
    let player = crate::dsr::load(dir)?;
    for name in [
        "a10_0100", "a10_0001", "a12_0100", "a12_0001", "a13_0100", "a13_0001",
    ] {
        println!("guard-source {name} {:?}", player.flags(name));
    }
    println!("goods {:?}", params.goods_animations());
    println!(
        "crossbow clips {:?}",
        player
            .hkx
            .keys()
            .filter(|k| k.starts_with("a46_"))
            .collect::<Vec<_>>()
    );
    for a in &player.taes["a00"] {
        if a.events.iter().any(|e| e.kind == 65) {
            println!(
                "goodsclip a00_{:04} {:?}",
                a.id,
                a.events
                    .iter()
                    .filter(|e| [0, 65, 66].contains(&e.kind))
                    .map(|e| (e.kind, e.start, e.end, e.param_i32(0)))
                    .collect::<Vec<_>>()
            );
        }
    }
    println!(
        "bow locomotion {:?}",
        player
            .hkx
            .keys()
            .filter(|k| k.starts_with("a14_") || k.starts_with("a15_"))
            .collect::<Vec<_>>()
    );
    for (id, w) in params.weapon_rows()? {
        println!(
            "weapon {id} category {} variation {} motion {:?} special {}",
            w.motion_category,
            w.variation,
            params.weapon_motion(id)?,
            params.special_attack(id)?
        );
    }
    for (set, anims) in &player.taes {
        if ["a44", "a46", "a14", "a16", "a00"].contains(&set.as_str()) {
            for anim in anims.iter().filter(|a| {
                if set == "a00" {
                    [6020, 6021, 6120, 6121].contains(&a.id)
                } else if set == "a14" || set == "a16" {
                    a.id < 160
                } else {
                    a.id < 5000
                }
            }) {
                let name = format!("{set}_{:04}", anim.id);
                if let Ok(clip) = crate::dsr::decode(&player, &name) {
                    let events: Vec<_> = player
                        .events(&name)
                        .iter()
                        .map(|e| {
                            (
                                e.kind,
                                e.start,
                                e.end,
                                e.param_i32(0),
                                e.param_i32(1),
                                e.param_i32(2),
                            )
                        })
                        .collect();
                    println!(
                        "ranged {name} duration {:.3} events {events:?}",
                        (clip.frames.len() - 1) as f32 / clip.fps
                    );
                }
            }
        }
        for anim in anims.iter().filter(|a| matches!(a.id, 0..=5 | 3000..=3002 | 3300 | 3301 | 3310 | 3900 | 4000..=4002 | 4300 | 4301 | 4310 | 4900)) {
            let name = format!("{set}_{:04}", anim.id);
            let resolved = player.resolve(&name, false)?;
            if !player.hkx.contains_key(&resolved) { continue; }
            let hits = player.hits(&name);
            let flags: Vec<_> = player.flags(&name).into_iter().filter(|f| [4,11,26].contains(&f.flag)).collect();
            println!("clip {name} hits {hits:?} cancels {flags:?}");
        }
    }
    Ok(())
}

/// One move: the name the game's move table uses (`r1_1`: the first light hit) and its animation.
pub struct Move {
    pub name: &'static str,
    pub clip: &'static str,
}

/// A weapon class: its motion category and behavior variation in the weapon params, a sample
/// weapon row of it, and its moves.
pub struct Class {
    pub name: &'static str,
    /// An `EquipParamWeapon` row of the class (the Shortsword).
    pub weapon: i32,
    pub motion_category: i64,
    pub variation: i64,
    pub moves: &'static [Move],
}

/// The straight swords, one-handed: light chain `3000`-`3002`, heavy `3300` and its follow-up
/// `3310` (`3301` shares `3300`'s judge and hit window and is not used), and the attack out of a
/// roll `3900`.
pub const STRAIGHT_SWORD: Class = Class {
    name: "straight_sword",
    weapon: 200_000,
    motion_category: 23,
    variation: 2300,
    moves: &[
        Move {
            name: "r1_1",
            clip: "a23_3000",
        },
        Move {
            name: "r1_2",
            clip: "a23_3001",
        },
        Move {
            name: "r1_3",
            clip: "a23_3002",
        },
        Move {
            name: "r2_1",
            clip: "a23_3300",
        },
        Move {
            name: "r2_2",
            clip: "a23_3310",
        },
        Move {
            name: "roll_attack",
            clip: "a23_3900",
        },
    ],
};

impl Class {
    /// The sample weapon row says what this record says, so a class recorded wrong cannot pass.
    pub fn check(&self, params: &Params) -> Result<()> {
        let w = params.weapon(self.weapon)?;
        ensure!(
            w.motion_category == self.motion_category && w.variation == self.variation,
            "weapon {} has motion category {} and behavior variation {}; the {} record says {} and {}",
            self.weapon,
            w.motion_category,
            w.variation,
            self.name,
            self.motion_category,
            self.variation
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_straight_sword_names_six_distinct_moves() {
        let c = &STRAIGHT_SWORD;
        assert_eq!(c.moves.len(), 6);
        let mut names: Vec<_> = c.moves.iter().map(|m| m.name).collect();
        let mut clips: Vec<_> = c.moves.iter().map(|m| m.clip).collect();
        names.sort_unstable();
        names.dedup();
        clips.sort_unstable();
        clips.dedup();
        assert_eq!((names.len(), clips.len()), (6, 6));
        // The animation set is the class's motion category.
        let set = format!("a{}_", c.motion_category);
        assert!(c.moves.iter().all(|m| m.clip.starts_with(&set)));
    }

    #[test]
    fn install_the_class_record_agrees_with_the_weapon_row() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let p = crate::params::load(std::path::Path::new(&dir)).unwrap();
        STRAIGHT_SWORD.check(&p).unwrap();
        let wrong = Class {
            variation: 2301,
            ..STRAIGHT_SWORD
        };
        let err = wrong.check(&p).unwrap_err();
        assert!(err.to_string().contains("2301"), "{err}");
    }
}
