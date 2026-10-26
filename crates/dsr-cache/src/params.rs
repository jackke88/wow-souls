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
    magic: Param,
    bullet: Param,
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

/// A creature's bullet ([`Params::npc_bullet`]): its `Bullet` row id, the row that hits (the explosion it spawns, or
/// itself) and that row's attack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcBullet {
    pub bullet: i32,
    pub hit_bullet: i32,
    pub row: BulletRow,
    pub attack: Option<NpcAttack>,
}

/// An `NpcThinkParam` row's distance bands (DS metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Think {
    pub near: f32,
    pub mid: f32,
    pub far: f32,
    pub out: f32,
}

/// A `Magic` row: what a spell flies (`refCategory` 1: `refId` is a `Bullet` row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicRow {
    pub ref_id: i64,
    pub ref_category: i64,
    pub ref_type: i64,
}

/// A `Bullet` row: a missile's flight in Dark Souls metres, seconds and degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BulletRow {
    pub life: f32,
    pub dist: f32,
    pub init_speed: f32,
    pub max_speed: f32,
    pub accel_time: f32,
    pub gravity: f32,
    pub homing_deg: i64,
    pub radius: f32,
    /// `hitRadiusMax`: how wide an explosion spreads (-1: it does not).
    pub radius_max: f32,
    pub hit_bullet: i64,
    /// `atkId_Bullet`: the `AtkParam_Pc` row the bullet hits with (0: it carries none).
    pub atk_id: i64,
}

/// What a spell does where it lands, read from the attack row of its last bullet (Dark Souls metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpellBlow {
    /// How wide the blast spreads (0: a plain hit with no splash).
    pub blast: f32,
    /// `knockbackDist`, `dmgLevel` (the flinch level) and `atkSuperArmor` (the poise damage).
    pub knock: f32,
    pub level: i64,
    pub poise: f32,
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
        magic: take("Magic")?,
        bullet: take("Bullet")?,
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
        self.guard_attack(
            &self.npc_behavior,
            &self.npc_attack,
            "NPC",
            None,
            variation,
            judge,
        )
    }

    /// Guard behavior of an actual PC weapon blow, used when a creature borrows that motion.
    pub fn player_guard_attack(
        &self,
        weapon: i32,
        variation: i64,
        judge: i32,
    ) -> Result<GuardAttack> {
        self.guard_attack(
            &self.behavior,
            &self.attack,
            "PC",
            Some(weapon),
            variation,
            judge,
        )
    }

    fn guard_attack(
        &self,
        b: &Param,
        a: &Param,
        source: &str,
        weapon: Option<i32>,
        variation: i64,
        judge: i32,
    ) -> Result<GuardAttack> {
        let rows: Vec<_> = (0..b.len())
            .filter(|&r| {
                matches!((b.int(r, "variationId"), b.int(r, "behaviorJudgeId")),
                (Ok(v), Ok(j)) if v == variation && j == i64::from(judge))
            })
            .collect();
        ensure!(
            rows.len() == 1,
            "{source} behavior variation {variation}, judge {judge}: {} rows",
            rows.len()
        );
        let row = rows[0];
        ensure!(b.int(row, "refType")? == 0, "guard attack is not an attack");
        let id = i32::try_from(b.int(row, "refId")?)?;
        let r = a
            .row_of(id)
            .with_context(|| format!("{source} attack has no row {id}"))?;
        // PC attacks carry their stamina budget in the weapon and atkStamCorrection,
        // while their raw guardAtkRate is zero. Preserve the existing NPC cache contract.
        let rate = match weapon {
            Some(_) if a.int(r, "disableStaminaAttack")? != 0 => 0,
            Some(weapon) => {
                let wr = self.weapon.row_of(weapon).context("PC guard weapon")?;
                let base = self.weapon.int(wr, "attackBaseStamina")? as f32;
                let correction = a.int(r, "atkStamCorrection")? as f32 / 100.0;
                (base * correction + a.int(r, "atkStam")? as f32).round() as i64
            }
            None => a.int(r, "guardAtkRate")?,
        };
        Ok(GuardAttack {
            rate,
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
        self.npc_atk(atk)
            .with_context(|| format!("BehaviorParam row {id}"))
    }

    /// The `AtkParam_Npc` row `atk`.
    pub fn npc_atk(&self, atk: i32) -> Result<NpcAttack> {
        let a = &self.npc_attack;
        let arow = a
            .row_of(atk)
            .with_context(|| format!("AtkParam_Npc has no row {atk}"))?;
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

    /// The magic damage (`atkMag`) of the `AtkParam_Npc` row `atk`: a blast of the Abyss carries no physical damage.
    pub fn npc_atk_magic(&self, atk: i32) -> Result<i64> {
        let a = &self.npc_attack;
        let arow = a
            .row_of(atk)
            .with_context(|| format!("AtkParam_Npc has no row {atk}"))?;
        a.int(arow, "atkMag")
    }

    /// A creature's bullet: the `BehaviorParam` row with this variation and judge whose `refType` is 1 (a
    /// `Bullet` row), that bullet, and the attack it (or the explosion it spawns, `HitBulletID`) hits with
    /// (`atkId_Bullet` is an `AtkParam_Npc` row for a creature's bullet).
    pub fn npc_bullet(&self, variation: i64, judge: i32) -> Result<NpcBullet> {
        let b = &self.npc_behavior;
        let row = (0..b.len())
            .find(|&r| {
                matches!(
                    (b.int(r, "variationId"), b.int(r, "behaviorJudgeId"), b.int(r, "refType")),
                    (Ok(v), Ok(j), Ok(1)) if v == variation && j == i64::from(judge)
                )
            })
            .with_context(|| {
                format!("BehaviorParam has no bullet row for variation {variation}, judge {judge}")
            })?;
        let id = i32::try_from(b.int(row, "refId")?).context("refId")?;
        let first = self.bullet(id)?;
        let (hit_id, last) = if first.hit_bullet > 0 {
            let h = i32::try_from(first.hit_bullet).context("HitBulletID")?;
            (h, self.bullet(h)?)
        } else {
            (id, first)
        };
        let attack = if last.atk_id > 0 {
            Some(self.npc_atk(i32::try_from(last.atk_id).context("atkId_Bullet")?)?)
        } else {
            None
        };
        Ok(NpcBullet {
            bullet: id,
            hit_bullet: hit_id,
            row: last,
            attack,
        })
    }

    /// The `Magic` row `id` (3000 is the first sorcery, 4000 the first pyromancy).
    pub fn magic(&self, id: i32) -> Result<MagicRow> {
        let p = &self.magic;
        let r = p
            .row_of(id)
            .with_context(|| format!("Magic has no row {id}"))?;
        Ok(MagicRow {
            ref_id: p.int(r, "refId")?,
            ref_category: p.int(r, "refCategory")?,
            ref_type: p.int(r, "refType")?,
        })
    }

    /// The `Bullet` row `id` (the spellings `Vellocity` are the install's).
    pub fn bullet(&self, id: i32) -> Result<BulletRow> {
        let p = &self.bullet;
        let r = p
            .row_of(id)
            .with_context(|| format!("Bullet has no row {id}"))?;
        Ok(BulletRow {
            life: p.float(r, "life")?,
            dist: p.float(r, "dist")?,
            init_speed: p.float(r, "initVellocity")?,
            max_speed: p.float(r, "maxVellocity")?,
            accel_time: p.float(r, "accelTime")?,
            gravity: p.float(r, "gravityInRange")?,
            homing_deg: p.int(r, "homingAngle")?,
            radius: p.float(r, "hitRadius")?,
            radius_max: p.float(r, "hitRadiusMax")?,
            hit_bullet: p.int(r, "HitBulletID")?,
            atk_id: p.int(r, "atkId_Bullet")?,
        })
    }

    /// What the spell `magic` does where it lands: the attack row of the bullet that does the hitting
    /// (the explosion its bullet spawns, else the bullet itself) and how wide that explosion spreads.
    pub fn spell_blow(&self, magic: i32) -> Result<SpellBlow> {
        let m = self.magic(magic)?;
        ensure!(
            m.ref_category == 1,
            "Magic {magic}: refId is not a Bullet row"
        );
        let first = self.bullet(i32::try_from(m.ref_id).context("refId")?)?;
        let spreads = first.hit_bullet >= 0;
        let last = if spreads {
            self.bullet(i32::try_from(first.hit_bullet).context("HitBulletID")?)?
        } else {
            first
        };
        ensure!(
            last.atk_id > 0,
            "Magic {magic}: its last bullet carries no attack row"
        );
        let a = &self.attack;
        let atk = i32::try_from(last.atk_id).context("atkId_Bullet")?;
        let row = a
            .row_of(atk)
            .with_context(|| format!("AtkParam_Pc has no row {atk} (Magic {magic})"))?;
        Ok(SpellBlow {
            blast: if spreads {
                last.radius_max.max(last.radius)
            } else {
                0.0
            },
            knock: a.float(row, "knockbackDist")?,
            level: a.int(row, "dmgLevel")?,
            poise: a.int(row, "atkSuperArmor")? as f32,
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

    #[test]
    fn soul_arrow_and_fireball_fly_as_the_survey_read_them() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            return;
        };
        let p = load(Path::new(&dir)).unwrap();
        let soul_magic = p.magic(3000).unwrap();
        assert_eq!(soul_magic.ref_category, 1, "refId is a Bullet row");
        let soul = p.bullet(soul_magic.ref_id as i32).unwrap();
        assert_eq!(
            (soul.life, soul.max_speed, soul.homing_deg),
            (1.1, 30.0, 40)
        );
        assert!((soul.radius - 0.1).abs() < 1.0e-6);
        let fire = p.bullet(p.magic(4000).unwrap().ref_id as i32).unwrap();
        assert_eq!(
            (fire.max_speed, fire.gravity, fire.hit_bullet),
            (15.0, 9.8, 6010)
        );
        assert!(p.magic(1).is_err(), "Magic rows start at 3000");
        assert!(p.bullet(1_999_999).is_err());
    }

    #[test]
    fn a_spells_blow_is_what_its_last_bullet_hits_with() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            return;
        };
        let p = load(Path::new(&dir)).unwrap();
        let fire = p.spell_blow(4000).unwrap();
        assert_eq!(
            (fire.blast, fire.knock, fire.level, fire.poise),
            (1.0, 1.2, 3, 40.0),
            "Bullet 6000 flies, 6010 is the explosion (1.0 m at its widest), AtkParam_Pc 13000"
        );
        let arrow = p.spell_blow(3000).unwrap();
        assert_eq!(
            (arrow.blast, arrow.knock, arrow.level, arrow.poise),
            (0.0, 0.7, 1, 20.0),
            "Soul Arrow hits with the bullet itself: no blast"
        );
        let flame = p.spell_blow(4100).unwrap();
        assert_eq!(
            (flame.blast, flame.knock, flame.level, flame.poise),
            (0.6, 0.8, 2, 40.0),
            "Combustion: Bullet 7000 spawns the flame 7010"
        );
        assert!(p.spell_blow(1).is_err());
    }
}
