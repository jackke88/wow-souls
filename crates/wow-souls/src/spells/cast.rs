//! The Mage's source-derived cast clips from the tracked `anim/source/casts.txt` table (see
//! the source-history spec `docs/superpowers/specs/2026-10-14-mage-class-design.md`, section 3).
//! Each spell wears one clip; its flag windows give the action machine its release and recovery.

use std::path::Path;

use bevy::prelude::*;

/// The Mage's four casts, by the spell each is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Fireball,
    Frostbolt,
    FireBlast,
    FrostArmor,
}

impl Role {
    pub const ALL: [Role; 4] = [
        Role::Fireball,
        Role::Frostbolt,
        Role::FireBlast,
        Role::FrostArmor,
    ];

    /// The name `casts.txt` gives it.
    pub fn name(self) -> &'static str {
        match self {
            Role::Fireball => "fireball",
            Role::Frostbolt => "frostbolt",
            Role::FireBlast => "fire_blast",
            Role::FrostArmor => "frost_armor",
        }
    }

    /// The spell id (rank 1).
    pub fn spell(self) -> u32 {
        match self {
            Role::Fireball => 133,
            Role::Frostbolt => 116,
            Role::FireBlast => 2136,
            Role::FrostArmor => 168,
        }
    }

    pub fn of_spell(spell: u32) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.spell() == spell)
    }

    fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == name)
    }
}

/// A cast's timing in seconds from its start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    pub duration: f32,
    pub release: f32,
    pub recover: f32,
}

/// The Dark Souls `Bullet` row a spell borrows (metres, seconds, degrees).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bullet {
    pub life: f32,
    pub dist: f32,
    pub speed0: f32,
    pub speed1: f32,
    pub accel: f32,
    pub gravity: f32,
    pub homing_deg: f32,
    pub radius: f32,
    pub burst: i64,
}

/// What a spell does where it lands, from the attack row of its last Dark Souls bullet (metres): how wide
/// the blast spreads (0: a plain hit), how far it pushes, its flinch level and its poise damage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impact {
    pub blast: f32,
    pub knock: f32,
    pub level: u8,
    pub poise: f32,
}

/// One role's lines of `casts.txt`.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub role: Role,
    pub clip: String,
    pub timing: Timing,
    pub bullet: Option<Bullet>,
    pub impact: Option<Impact>,
}

/// Reads `casts.txt`; every role must be there, with a release before its recovery inside the clip.
pub fn parse(text: &str) -> Result<Vec<Line>, String> {
    if text.lines().next() != Some("casts-version 2") {
        return Err("wrong casts cache version".into());
    }
    let num = |w: &str, line: &str| -> Result<f32, String> {
        w.parse::<f32>()
            .ok()
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("bad number in `{line}`"))
    };
    let mut lines: Vec<Line> = Vec::new();
    for line in text.lines().skip(1) {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            ["cast", role, clip, d, r, c] => {
                let role = Role::from_name(role).ok_or_else(|| format!("unknown role `{role}`"))?;
                let timing = Timing {
                    duration: num(d, line)?,
                    release: num(r, line)?,
                    recover: num(c, line)?,
                };
                if !(timing.release >= 0.0
                    && timing.release < timing.recover
                    && timing.recover <= timing.duration + 1.0e-3)
                {
                    return Err(format!(
                        "{}: release must come before the recovery",
                        role.name()
                    ));
                }
                lines.push(Line {
                    role,
                    clip: (*clip).to_string(),
                    timing,
                    bullet: None,
                    impact: None,
                });
            }
            ["bullet", role, life, dist, s0, s1, accel, gravity, homing, radius, burst] => {
                let role = Role::from_name(role).ok_or_else(|| format!("unknown role `{role}`"))?;
                let b = Bullet {
                    life: num(life, line)?,
                    dist: num(dist, line)?,
                    speed0: num(s0, line)?,
                    speed1: num(s1, line)?,
                    accel: num(accel, line)?,
                    gravity: num(gravity, line)?,
                    homing_deg: num(homing, line)?,
                    radius: num(radius, line)?,
                    burst: burst
                        .parse::<i64>()
                        .map_err(|_| format!("bad burst in `{line}`"))?,
                };
                lines
                    .iter_mut()
                    .find(|l| l.role == role)
                    .ok_or_else(|| format!("bullet before cast: {}", role.name()))?
                    .bullet = Some(b);
            }
            ["impact", role, blast, knock, level, poise] => {
                let role = Role::from_name(role).ok_or_else(|| format!("unknown role `{role}`"))?;
                let impact = Impact {
                    blast: num(blast, line)?,
                    knock: num(knock, line)?,
                    level: level
                        .parse::<u8>()
                        .map_err(|_| format!("bad flinch level in `{line}`"))?,
                    poise: num(poise, line)?,
                };
                lines
                    .iter_mut()
                    .find(|l| l.role == role)
                    .ok_or_else(|| format!("impact before cast: {}", role.name()))?
                    .impact = Some(impact);
            }
            _ => {}
        }
    }
    for role in Role::ALL {
        if !lines.iter().any(|l| l.role == role) {
            return Err(format!("incomplete casts cache: no {}", role.name()));
        }
    }
    Ok(lines)
}

/// A cast clip on both rigs with its timing.
pub struct Clip {
    pub role: Role,
    pub wow: wscl::Clip,
    pub reference: wscl::Clip,
    pub timing: Timing,
    pub bullet: Option<Bullet>,
    pub impact: Option<Impact>,
}

/// The loaded casts (absent without a fresh Dark Souls cache: the Mage's spells then stay unusable).
#[derive(Resource)]
pub struct Kit {
    pub clips: Vec<Clip>,
}

impl Kit {
    pub fn clip(&self, role: Role) -> Option<&Clip> {
        self.clips.iter().find(|c| c.role == role)
    }

    /// The timing of the cast of `spell`.
    pub fn timing(&self, spell: u32) -> Option<Timing> {
        self.clip(Role::of_spell(spell)?).map(|c| c.timing)
    }

    /// What the cast of `spell` does where it lands (`None`: a buff, or no such spell).
    pub fn impact(&self, spell: u32) -> Option<Impact> {
        self.clip(Role::of_spell(spell)?).and_then(|c| c.impact)
    }

    pub fn load(dir: &Path) -> Result<Kit, String> {
        let text = std::fs::read_to_string(dir.join("casts.txt")).map_err(|e| e.to_string())?;
        let parsed = parse(&text)?;
        // The same strict source checks as the other publications; the common cache is required by the
        // avatar's pose pipeline anyway.
        crate::motion_clip::checked_manifest(dir)?;
        let mut clips = Vec::new();
        for l in parsed {
            let wow = crate::motion_clip::player_clip(dir, &l.clip)?;
            let reference = crate::motion_clip::read_clip(dir, "human_reference", &l.clip)?;
            if (wow.duration() - l.timing.duration).abs() > 0.02 {
                return Err(format!(
                    "{}: the clip is {:.3} s, the manifest says {:.3}",
                    l.role.name(),
                    wow.duration(),
                    l.timing.duration
                ));
            }
            if wow.windows(wscl::FLAG_INVINCIBLE).next().is_some() {
                return Err(format!(
                    "{}: a cast confers no invulnerability",
                    l.role.name()
                ));
            }
            info!(
                "spells: cast {} {} ({:.3} s, release {:.3}, recover {:.3})",
                l.role.name(),
                l.clip,
                l.timing.duration,
                l.timing.release,
                l.timing.recover
            );
            clips.push(Clip {
                role: l.role,
                wow,
                reference,
                timing: l.timing,
                bullet: l.bullet,
                impact: l.impact,
            });
        }
        Ok(Kit { clips })
    }
}

/// Loads the casts at startup; without them the Mage's spells say "You cannot do that yet."
pub fn load_kit(mut commands: Commands) {
    match Kit::load(&crate::motion_clip::cache_dir()) {
        Ok(k) => {
            commands.insert_resource(k);
        }
        Err(why) => {
            warn!("spells: no Dark Souls cast clips ({why}); the mage's spells stay unusable");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "casts-version 2\n\
        source 100 5 C:/x/c0000_a00_md.anibnd.dcx\n\
        cast fireball body_interaction_506 2.000 0.500 1.500\n\
        bullet fireball 0.5 10 15 15 0 9.8 10 0.25 6010\n\
        impact fireball 1 1.2 3 40\n\
        cast frostbolt body_interaction_210 1.667 0.700 1.333\n\
        bullet frostbolt 1.1 20 4 30 0.32 0 40 0.1 -1\n\
        impact frostbolt 0 0.7 1 20\n\
        cast fire_blast body_interaction_509 1.833 0.500 1.400\n\
        impact fire_blast 0.6 0.8 2 40\n\
        cast frost_armor body_self_enchant 1.633 0.367 1.433\n";

    #[test]
    fn the_manifest_gives_each_role_its_clip_timing_and_bullet() {
        let p = parse(TEXT).unwrap();
        assert_eq!(p.len(), 4);
        let fireball = p.iter().find(|c| c.role == Role::Fireball).unwrap();
        assert_eq!(fireball.clip, "body_interaction_506");
        assert_eq!(
            fireball.timing,
            Timing {
                duration: 2.0,
                release: 0.5,
                recover: 1.5
            }
        );
        assert_eq!(fireball.bullet.unwrap().burst, 6010);
        assert_eq!(
            fireball.impact,
            Some(Impact {
                blast: 1.0,
                knock: 1.2,
                level: 3,
                poise: 40.0
            }),
            "the explosion: 1 m wide, a heavy flinch, 1.2 m back"
        );
        let frost = p.iter().find(|c| c.role == Role::Frostbolt).unwrap();
        assert_eq!(frost.bullet.unwrap().homing_deg, 40.0);
        assert_eq!(frost.impact.unwrap().blast, 0.0, "a bolt has no splash");
        let blast = p.iter().find(|c| c.role == Role::FireBlast).unwrap();
        assert!(blast.bullet.is_none(), "an instant spell flies nothing");
        assert_eq!(blast.impact.unwrap().level, 2);
        let armor = p.iter().find(|c| c.role == Role::FrostArmor).unwrap();
        assert!(armor.impact.is_none(), "a buff hits no one");
    }

    #[test]
    fn a_wrong_version_a_missing_role_or_a_bad_timing_is_an_error() {
        assert!(
            parse("casts-version 1\n").is_err(),
            "the old cache has no impact"
        );
        assert!(parse("casts-version 3\n").is_err());
        let missing = TEXT.replace("cast frost_armor body_self_enchant 1.633 0.367 1.433\n", "");
        assert!(parse(&missing).unwrap_err().contains("frost_armor"));
        let bad = TEXT.replace("0.500 1.500", "1.4 1.2");
        assert!(
            parse(&bad).unwrap_err().contains("fireball"),
            "release after recovery"
        );
        let unknown = format!("{TEXT}cast lightning body_interaction_201 1 0.5 0.9\n");
        assert!(parse(&unknown).unwrap_err().contains("lightning"));
    }

    #[test]
    fn a_spell_id_names_its_role() {
        assert_eq!(Role::of_spell(133), Some(Role::Fireball));
        assert_eq!(Role::of_spell(116), Some(Role::Frostbolt));
        assert_eq!(Role::of_spell(2136), Some(Role::FireBlast));
        assert_eq!(Role::of_spell(168), Some(Role::FrostArmor));
        assert_eq!(Role::of_spell(100), None, "Charge is no cast");
        for r in Role::ALL {
            assert_eq!(Role::of_spell(r.spell()), Some(r));
        }
    }
}
