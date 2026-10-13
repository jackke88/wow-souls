//! The Dark Souls params the moves need, read through their PARAMDEFs: the weapon row that names
//! a class's motion category and behavior variation, the behavior row of an attack (stamina, the
//! attack row it points at) and that attack row's damage multiplier.

use std::path::Path;

use anyhow::{ensure, Context, Result};
use dsr_formats::param::{param_type, Param, Paramdef};

use crate::dsr::archive;

pub struct Params {
    goods: Param,
    weapon: Param,
    behavior: Param,
    attack: Param,
    npc: Param,
    npc_behavior: Param,
    npc_attack: Param,
    think: Param,
}

/// What a weapon row says about its moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weapon {
    pub motion_category: i64,
    pub variation: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponGuard {
    pub cut: f32,
    pub stability: i64,
    pub enabled: bool,
    pub parry: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuardAttack {
    pub rate: i64,
    pub unblockable: bool,
}

/// What an attack costs and does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behavior {
    pub stamina: i64,
    /// The damage as a multiple of the base hit: `AtkParam_Pc.atkPhysCorrection` is a percent.
    pub mult: f32,
    /// `AtkParam_Pc.dmgLevel` and `atkSuperArmor`: the flinch level and the poise damage.
    pub level: i64,
    pub poise: f32,
    pub poise_correction: f32,
    /// `AtkParam_Pc.hitStopTime` (s) and `knockbackDist` (DS metres).
    pub stop: f32,
    pub knock: f32,
    /// `AtkParam_Pc.hit0_Radius` / `hit1_Radius` (DS metres): the thickness of the blow's hit volume.
    pub radius: f32,
    pub radius2: f32,
}

/// An `NpcParam` row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Npc {
    pub hp: i64,
    pub poise: i64,
    pub variation: i64,
    pub think: i64,
    /// `getSoul`: the souls a kill gives.
    pub get_soul: i64,
}

/// A creature attack: `BehaviorParam` (variation, judge) → `AtkParam_Npc`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcAttack {
    pub phys: i64,
    pub poise: i64,
    pub level: i64,
    pub radius: f32,
    pub radius2: f32,
    /// `AtkParam_Npc.hitStopTime` (s) and `knockbackDist` (DS metres).
    pub stop: f32,
    pub knock: f32,
}

/// An `NpcThinkParam` row's distance bands (DS metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Think {
    pub near: f32,
    pub mid: f32,
    pub far: f32,
    pub out: f32,
}

pub fn load(dsr: &Path) -> Result<Params> {
    let defs: Vec<Paramdef> = archive(&dsr.join("paramdef").join("paramdef.paramdefbnd.dcx"))?
        .iter()
        .map(|e| Paramdef::read(&e.data).with_context(|| format!("PARAMDEF {}", e.name)))
        .collect::<Result<_>>()?;
    let entries = archive(
        &dsr.join("param")
            .join("GameParam")
            .join("GameParam.parambnd.dcx"),
    )?;
    let take = |stem: &str| -> Result<Param> {
        let e = entries
            .iter()
            .find(|e| e.stem() == stem)
            .with_context(|| format!("GameParam has no {stem}.param"))?;
        let ty = param_type(&e.data)?;
        let def = defs
            .iter()
            .find(|d| d.param_type == ty)
            .with_context(|| format!("no PARAMDEF for {ty}"))?
            .clone();
        Param::read(e.data.clone(), def).with_context(|| format!("{stem}.param"))
    };
    Ok(Params {
        goods: take("EquipParamGoods")?,
        weapon: take("EquipParamWeapon")?,
        behavior: take("BehaviorParam_PC")?,
        attack: take("AtkParam_Pc")?,
        npc: take("NpcParam")?,
        npc_behavior: take("BehaviorParam")?,
        npc_attack: take("AtkParam_Npc")?,
        think: take("NpcThinkParam")?,
    })
}

impl Params {
    pub fn weapon_damage(&self, id: i32) -> Result<i64> {
        self.weapon.int(
            self.weapon.row_of(id).context("weapon row")?,
            "attackBasePhysics",
        )
    }
    pub fn weapon_poise(&self, id: i32) -> Result<f32> {
        Ok(self.weapon.int(
            self.weapon.row_of(id).context("weapon row")?,
            "saWeaponDamage",
        )? as f32)
    }
    pub fn projectile_stamina(&self, variation: i64, judge: i32) -> Result<i64> {
        let p = &self.behavior;
        let rows: Vec<_> = (0..p.len())
            .filter(|&r| {
                p.int(r, "variationId").ok() == Some(variation)
                    && p.int(r, "behaviorJudgeId").ok() == Some(i64::from(judge))
            })
            .collect();
        ensure!(
            rows.len() == 1,
            "projectile behavior {variation}/{judge}: {} rows",
            rows.len()
        );
        p.int(rows[0], "stamina")
    }
    pub fn goods_animations(&self) -> Vec<(i32, i64)> {
        (0..self.goods.len())
            .filter_map(|r| Some((self.goods.id(r), self.goods.int(r, "goodsUseAnim").ok()?)))
            .collect()
    }
    /// Base weapon rows for a reproducible local survey (upgrade rows are omitted).
    pub fn weapon_rows(&self) -> Result<Vec<(i32, Weapon)>> {
        (0..self.weapon.len())
            .map(|r| self.weapon.id(r))
            .filter(|id| id % 1000 == 0)
            .map(|id| Ok((id, self.weapon(id)?)))
            .collect()
    }

    pub fn weapon_motion(&self, id: i32) -> Result<(i64, i64)> {
        let r = self.weapon.row_of(id).context("weapon row")?;
        Ok((
            self.weapon.int(r, "wepmotionOneHandId")?,
            self.weapon.int(r, "wepmotionBothHandId")?,
        ))
    }

    pub fn special_attack(&self, id: i32) -> Result<i64> {
        self.weapon.int(
            self.weapon.row_of(id).context("weapon row")?,
            "spAtkcategory",
        )
    }

    pub fn weapon_guard(&self, id: i32) -> Result<WeaponGuard> {
        let p = &self.weapon;
        let r = p
            .row_of(id)
            .with_context(|| format!("EquipParamWeapon has no row {id}"))?;
        Ok(WeaponGuard {
            cut: p.float(r, "physGuardCutRate")?,
            stability: p.int(r, "staminaGuardDef")?,
            enabled: p.int(r, "enableGuard")? != 0,
            parry: p.int(r, "enableParry")? != 0,
        })
    }

    pub fn npc_guard_attack(&self, variation: i64, judge: i32) -> Result<GuardAttack> {
        let b = &self.npc_behavior;
        let rows: Vec<_> = (0..b.len())
            .filter(|&r| {
                matches!((b.int(r, "variationId"), b.int(r, "behaviorJudgeId")),
                (Ok(v), Ok(j)) if v == variation && j == i64::from(judge))
            })
            .collect();
        ensure!(
            rows.len() == 1,
            "BehaviorParam variation {variation}, judge {judge}: {} rows",
            rows.len()
        );
        let row = rows[0];
        ensure!(b.int(row, "refType")? == 0, "guard attack is not an attack");
        let id = i32::try_from(b.int(row, "refId")?)?;
        let a = &self.npc_attack;
        let r = a
            .row_of(id)
            .with_context(|| format!("AtkParam_Npc has no row {id}"))?;
        Ok(GuardAttack {
            rate: a.int(r, "guardAtkRate")?,
            unblockable: a.int(r, "disableGuard")? != 0 || a.int(r, "throwFlag")? != 0,
        })
    }

    pub fn weapon(&self, id: i32) -> Result<Weapon> {
        let row = self
            .weapon
            .row_of(id)
            .with_context(|| format!("EquipParamWeapon has no row {id}"))?;
        Ok(Weapon {
            motion_category: self.weapon.int(row, "wepmotionCategory")?,
            variation: self.weapon.int(row, "behaviorVariationId")?,
        })
    }

    /// The behavior of an attack: the `BehaviorParam_PC` row with this variation and judge id
    /// (matched on the two fields, there must be exactly one) and the `AtkParam_Pc` row it points at.
    pub fn behavior(&self, variation: i64, judge: i32) -> Result<Behavior> {
        let b = &self.behavior;
        let mut rows = (0..b.len()).filter(|&r| {
            matches!(
                (b.int(r, "variationId"), b.int(r, "behaviorJudgeId")),
                (Ok(v), Ok(j)) if v == variation && j == i64::from(judge)
            )
        });
        let row = rows.next().with_context(|| {
            format!("BehaviorParam_PC has no row for variation {variation}, judge {judge}")
        })?;
        ensure!(
            rows.next().is_none(),
            "BehaviorParam_PC has two rows for variation {variation}, judge {judge}"
        );
        let id = b.id(row);
        ensure!(
            b.int(row, "refType")? == 0,
            "BehaviorParam_PC row {id} is not an attack"
        );
        let atk = i32::try_from(b.int(row, "refId")?).context("refId")?;
        let a = &self.attack;
        let arow = a
            .row_of(atk)
            .with_context(|| format!("AtkParam_Pc has no row {atk} (BehaviorParam_PC row {id})"))?;
        Ok(Behavior {
            stamina: b.int(row, "stamina")?,
            mult: a.int(arow, "atkPhysCorrection")? as f32 / 100.0,
            level: a.int(arow, "dmgLevel")?,
            poise: a.int(arow, "atkSuperArmor")? as f32,
            poise_correction: a.int(arow, "atkSuperArmorCorrection")? as f32 / 100.0,
            stop: a.float(arow, "hitStopTime")?,
            knock: a.float(arow, "knockbackDist")?,
            radius: a.float(arow, "hit0_Radius")?,
            radius2: a.float(arow, "hit1_Radius")?,
        })
    }

    pub fn npc(&self, id: i32) -> Result<Npc> {
        let n = &self.npc;
        let row = n
            .row_of(id)
            .with_context(|| format!("NpcParam has no row {id}"))?;
        Ok(Npc {
            hp: n.int(row, "hp")?,
            poise: n.int(row, "superArmorDurability")?,
            variation: n.int(row, "behaviorVariationId")?,
            think: n.int(row, "aiThinkId")?,
            get_soul: n.int(row, "getSoul")?,
        })
    }

    /// A creature's attack: the `BehaviorParam` row with this variation and judge (exactly one,
    /// an attack, not a bullet) and the `AtkParam_Npc` row it points at.
    pub fn npc_attack(&self, variation: i64, judge: i32) -> Result<NpcAttack> {
        let b = &self.npc_behavior;
        let mut rows = (0..b.len()).filter(|&r| {
            matches!(
                (b.int(r, "variationId"), b.int(r, "behaviorJudgeId")),
                (Ok(v), Ok(j)) if v == variation && j == i64::from(judge)
            )
        });
        let row = rows.next().with_context(|| {
            format!("BehaviorParam has no row for variation {variation}, judge {judge}")
        })?;
        ensure!(
            rows.next().is_none(),
            "BehaviorParam has two rows for variation {variation}, judge {judge}"
        );
        let id = b.id(row);
        ensure!(
            b.int(row, "refType")? == 0,
            "BehaviorParam row {id} is not an attack"
        );
        let atk = i32::try_from(b.int(row, "refId")?).context("refId")?;
        let a = &self.npc_attack;
        let arow = a
            .row_of(atk)
            .with_context(|| format!("AtkParam_Npc has no row {atk} (BehaviorParam row {id})"))?;
        Ok(NpcAttack {
            phys: a.int(arow, "atkPhys")?,
            poise: a.int(arow, "atkSuperArmor")?,
            level: a.int(arow, "dmgLevel")?,
            radius: a.float(arow, "hit0_Radius")?,
            radius2: a.float(arow, "hit1_Radius")?,
            stop: a.float(arow, "hitStopTime")?,
            knock: a.float(arow, "knockbackDist")?,
        })
    }

    pub fn think(&self, id: i32) -> Result<Think> {
        let t = &self.think;
        let row = t
            .row_of(id)
            .with_context(|| format!("NpcThinkParam has no row {id}"))?;
        Ok(Think {
            near: t.float(row, "nearDist")?,
            mid: t.float(row, "midDist")?,
            far: t.float(row, "farDist")?,
            out: t.float(row, "outDist")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_guard_and_fist_parry_match_the_survey() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            return;
        };
        let p = load(Path::new(&dir)).unwrap();
        assert_eq!(
            p.weapon_guard(200000).unwrap(),
            WeaponGuard {
                cut: 50.0,
                stability: 32,
                enabled: true,
                parry: false,
            }
        );
        assert!(p.weapon_guard(900000).unwrap().parry);
        let parry = p.behavior(4201, 450).unwrap();
        assert_eq!(parry.stamina, 14);
        assert!((parry.radius - 0.55).abs() < 0.001);
        assert_eq!(
            p.npc_guard_attack(20600, 100).unwrap(),
            GuardAttack {
                rate: 20,
                unblockable: false
            }
        );
        assert!(p.npc_guard_attack(20600, 160).unwrap().unblockable);
        assert_eq!(p.npc_guard_attack(24000, 150).unwrap().rate, 100);
    }

    #[test]
    fn install_the_shortsword_and_its_moves_read_as_measured() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let p = load(Path::new(&dir)).unwrap();
        assert_eq!(
            p.weapon(200_000).unwrap(),
            Weapon {
                motion_category: 23,
                variation: 2300
            }
        );
        assert!(p.weapon(1).is_err());
        // (judge, stamina, damage multiplier): the spec's table.
        for (judge, stamina, mult) in [
            (0, 20, 1.00),
            (10, 20, 1.00),
            (20, 20, 1.00),
            (100, 45, 1.35),
            (110, 35, 1.25),
            (540, 20, 0.90),
        ] {
            let b = p.behavior(2300, judge).unwrap();
            assert_eq!(b.stamina, stamina, "judge {judge}");
            assert!((b.mult - mult).abs() < 1.0e-6, "judge {judge}: {}", b.mult);
        }
        assert!(p.behavior(2300, 12_345).is_err(), "no such judge");
        // (judge, hitStopTime s, knockbackDist m): the impact spec's table.
        for (judge, stop, knock) in [
            (0, 0.08, 0.72),
            (10, 0.08, 0.84),
            (20, 0.08, 0.72),
            (100, 0.08, 0.95),
            (110, 0.08, 0.90),
            (540, 0.08, 0.80),
        ] {
            let b = p.behavior(2300, judge).unwrap();
            assert!(
                (b.stop - stop).abs() < 1.0e-6 && (b.knock - knock).abs() < 1.0e-6,
                "judge {judge}: {b:?}"
            );
        }
    }

    #[test]
    fn install_the_swords_blows_are_as_fat_as_measured() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let p = load(Path::new(&dir)).unwrap();
        // (judge, hit0_Radius m, hit1_Radius m): the straight sword's R1 chain, R2 and rolling attack.
        for (judge, r0, r1) in [
            (0, 0.4, 0.2),
            (10, 0.4, 0.2),
            (20, 0.4, 0.2),
            (100, 0.3, 0.1),
            (110, 0.35, 0.1),
            (540, 0.35, 0.2),
        ] {
            let b = p.behavior(2300, judge).unwrap();
            assert!(
                (b.radius - r0).abs() < 1.0e-6 && (b.radius2 - r1).abs() < 1.0e-6,
                "judge {judge}: {b:?}"
            );
        }
    }

    #[test]
    fn install_the_ghoul_reads_as_measured() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let p = load(Path::new(&dir)).unwrap();
        assert_eq!(
            p.npc(206_000).unwrap(),
            Npc {
                hp: 203,
                poise: 0,
                variation: 20_600,
                think: 206_000,
                get_soul: 150
            }
        );
        assert!(p.npc(9_999_999).is_err());
        // (judge, atkPhys, poise damage, level, radius, radius2): the spec's table.
        for (judge, phys, poise, level, r1, r2) in [
            (100, 138, 10, 1, 0.30, 0.0),
            (110, 142, 10, 1, 0.30, 0.0),
            (120, 195, 20, 2, 0.30, 0.0),
            (130, 186, 20, 2, 0.40, 0.0),
            (140, 106, 30, 5, 0.20, 0.20),
            (160, 124, 20, 1, 0.60, 0.60),
        ] {
            let a = p.npc_attack(20_600, judge).unwrap();
            assert_eq!(
                (a.phys, a.poise, a.level),
                (phys, poise, level),
                "judge {judge}"
            );
            assert!(
                (a.radius - r1).abs() < 1.0e-6 && (a.radius2 - r2).abs() < 1.0e-6,
                "judge {judge}: {a:?}"
            );
        }
        // (judge, knockbackDist m): the kobold's blows freeze nothing.
        for (judge, knock) in [
            (100, 0.45),
            (110, 0.45),
            (120, 1.0),
            (130, 1.0),
            (140, 1.37),
            (160, 1.0),
        ] {
            let a = p.npc_attack(20_600, judge).unwrap();
            assert!(
                a.stop == 0.0 && (a.knock - knock).abs() < 1.0e-6,
                "judge {judge}: {a:?}"
            );
        }
        assert!(
            p.npc_attack(20_600, 150).is_err(),
            "the head throw is a bullet"
        );
        assert!(p.npc_attack(20_600, 12_345).is_err());
        let t = p.think(206_000).unwrap();
        assert_eq!((t.near, t.mid, t.far, t.out), (2.0, 5.0, 10.0, 20.0));
        // The player's moves: R1 is level 1, R2 level 2, no poise damage.
        assert_eq!(
            (
                p.behavior(2300, 0).unwrap().level,
                p.behavior(2300, 100).unwrap().level
            ),
            (1, 2)
        );
        assert_eq!(p.behavior(2300, 0).unwrap().poise, 0.0);
    }
}
