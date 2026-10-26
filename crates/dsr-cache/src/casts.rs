//! The Mage's casts: Dark Souls' own cast animations and the numbers of the missile each spell flies,
//! published as `casts.txt` beside the clips (spec `docs/superpowers/specs/2026-10-14-mage-class-design.md`,
//! section 3). A separate publication like `interaction.txt`: optional clips, independent of the combat
//! kits.
//!
//! A cast clip says when it releases in one of three ways. A pyromancy clip (`a00_64NN`, and `a00_65NN`
//! for the left hand: NN is the spell's `Magic.refType`) carries its own bullet event (a type-64 event). A
//! weapon move used as a cast (a two-handed swing or thrust, `aNN_4000` / `aNN_4300`) releases where its
//! first hit window opens: the instant the weapon connects. In both, the chain and evade flags open where
//! the body may act again. A sorcery clip (`a00_62NN`) has neither: it carries three `ChrActionFlag`
//! windows (hypothesis, checked in motion), 91 the wind-up, 90 the release, 89 the recovery; the release
//! is where flag 90 opens and the body may act again where flag 89 opens (the clip's end when it has none).

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{ensure, Context, Result};

use crate::cache::{extract_clip, source_line, summary, write_pair};
use crate::{bonemap, dsr, params, wow};

pub const FLAG_WINDUP: i32 = 91;
pub const FLAG_RELEASE: i32 = 90;
pub const FLAG_RECOVER: i32 = 89;

/// One of the Mage's casts: the DS clip it wears and the `Magic` row whose `Bullet` it borrows its
/// homing and hit radius from (`None`: no missile).
pub struct Role {
    pub name: &'static str,
    pub clip: &'static str,
    pub magic: Option<i32>,
}

/// The picks, from Elden Ring's sorcery showcase as the reference for the weight of a staff cast (a raise
/// overhead, then a hard swing or thrust with the whole body) and Dark Souls' own two-handed weapon moves
/// as the motions that have it (the cast pyromancy and sorcery clips are small, and were judged to have no
/// power): the staff swings as a greatsword does, and the spell leaves it where the weapon connects.
pub const ROLES: [Role; 4] = [
    // Fireball: the greatsword's two-handed R1, a lunging diagonal slash; the weapon connects at 0.60 s.
    Role {
        name: "fireball",
        clip: "a25_4000",
        magic: Some(4000),
    },
    // Frostbolt: the staff's (polearm's) R1, a quick thrust; connects at 0.53 s.
    Role {
        name: "frostbolt",
        clip: "a38_4000",
        magic: Some(3000),
    },
    // Fire Blast: the two-handed mace's R2, the staff raised overhead then slammed down; 0.93 s.
    Role {
        name: "fire_blast",
        clip: "a59_4300",
        magic: Some(4100),
    },
    // Frost Armor: the pyromancy buff gesture (Iron Flesh, motion 6): the staff raised overhead.
    Role {
        name: "frost_armor",
        clip: "a00_6506",
        magic: None,
    },
];

/// The instant a clip releases its spell by an event of its own: the bullet event, else the first hit
/// window of a weapon move (the weapon connects). `None` for a staff cast, which the flags time.
pub fn release_event(bullet: Option<f32>, hits: &[wscl::HitWindow]) -> Option<f32> {
    bullet.or_else(|| hits.iter().map(|h| h.start).reduce(f32::min))
}

/// `(release, recover)` of a cast clip. The release is the clip's own bullet event (`spawn`) when it has
/// one, else where flag 90 opens; the body may act again where the chain or evade flag opens, else where
/// flag 89 opens, else when the clip ends. `None` when the clip has no release or the times do not make a
/// cast (the release must come before the recovery, inside the clip).
pub fn timing(flags: &[wscl::FlagWindow], spawn: Option<f32>, duration: f32) -> Option<(f32, f32)> {
    let open = |wanted: &[i32]| {
        flags
            .iter()
            .filter(|f| wanted.contains(&f.flag))
            .map(|f| f.start)
            .reduce(f32::min)
    };
    let release = spawn.or_else(|| open(&[FLAG_RELEASE]))?;
    let recover = open(&[wscl::FLAG_CHAIN, wscl::FLAG_EVADE])
        .or_else(|| open(&[FLAG_RECOVER]))
        .unwrap_or(duration);
    (release >= 0.0 && release < recover && release < duration && recover <= duration + 1.0e-3)
        .then_some((release, recover.min(duration)))
}

/// The `impact` line of a role: `impact <role> <blast m> <knock m> <flinch level> <poise>`.
pub fn impact_line(role: &str, b: &params::SpellBlow) -> String {
    format!(
        "impact {role} {} {} {} {}",
        b.blast, b.knock, b.level, b.poise
    )
}

/// Retargets the roles' clips through the same gates as every other family and writes `casts.txt`.
pub fn extract(dsr_dir: &Path, wow_dir: &Path, out: &Path) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    let skel = wow::human_male(wow_dir)?;
    let params = params::load(dsr_dir)?;
    let mut lines = String::from("casts-version 2\n");
    for archive in dsr::animation_archives(dsr_dir)? {
        writeln!(lines, "{}", source_line(&archive)?)?;
    }
    for role in &ROLES {
        let e = extract_clip(&player, &skel, &bonemap::HUMAN_MALE_RIG, role.clip)
            .with_context(|| format!("cast {} ({})", role.name, role.clip))?;
        ensure!(
            e.r.wow.windows(wscl::FLAG_INVINCIBLE).next().is_none(),
            "{}: a cast carries no invulnerability",
            role.clip
        );
        let duration = e.r.wow.duration();
        let spawn = release_event(player.bullet_spawn(role.clip), &player.hits(role.clip));
        let (release, recover) = timing(&player.flags(role.clip), spawn, duration)
            .with_context(|| {
                format!(
                    "{} has no release (a bullet event or flag {FLAG_RELEASE}): pick another clip for {}",
                    role.clip, role.name
                )
            })?;
        println!(
            "cast {}: {}  release {release:.3} recover {recover:.3} of {duration:.3}",
            role.name,
            summary(role.clip, &e.r)
        );
        write_pair(out, "human_male", "c0000", role.clip, &e.r)?;
        writeln!(
            lines,
            "cast {} {} {duration:.3} {release:.3} {recover:.3}",
            role.name, role.clip
        )?;
        if let Some(magic) = role.magic {
            let m = params.magic(magic)?;
            ensure!(
                m.ref_category == 1,
                "Magic {magic}: refId is not a Bullet row"
            );
            let b = params.bullet(m.ref_id as i32)?;
            writeln!(
                lines,
                "bullet {} {} {} {} {} {} {} {} {} {}",
                role.name,
                b.life,
                b.dist,
                b.init_speed,
                b.max_speed,
                b.accel_time,
                b.gravity,
                b.homing_deg,
                b.radius,
                b.hit_bullet
            )?;
            writeln!(
                lines,
                "{}",
                impact_line(role.name, &params.spell_blow(magic)?)
            )?;
        }
    }
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("casts.txt"), lines)?;
    Ok(())
}

/// `casts-survey`: every clip of the cast blocks `a00_6200-6226`, `6300-6326`, `6400-6426`,
/// `6500-6526` with its flag windows, so a role's clip can be chosen by the rule. Read-only: nothing
/// is written.
pub fn survey(dsr_dir: &Path, wow_dir: &Path) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    let skel = wow::human_male(wow_dir)?;
    for base in [6200, 6300, 6400, 6500] {
        for id in base..=base + 26 {
            let name = format!("a00_{id}");
            if dsr::decode(&player, &name).is_err() {
                continue;
            }
            let flags = player.flags(&name);
            let windows = |flag: i32| {
                flags
                    .iter()
                    .filter(|f| f.flag == flag)
                    .map(|f| format!("{:.2}-{:.2}", f.start, f.end))
                    .collect::<Vec<_>>()
                    .join("+")
            };
            let gates = match extract_clip(&player, &skel, &bonemap::HUMAN_MALE_RIG, &name) {
                Ok(e) => {
                    let d = e.r.wow.duration();
                    match timing(
                        &flags,
                        release_event(player.bullet_spawn(&name), &player.hits(&name)),
                        d,
                    ) {
                        Some((r, c)) => format!("OK len {d:.2} release {r:.2} recover {c:.2}"),
                        None => format!("no release, len {d:.2}"),
                    }
                }
                Err(why) => format!("gate: {why}"),
            };
            println!(
                "{name} 91:[{}] 90:[{}] 89:[{}]  {gates}",
                windows(FLAG_WINDUP),
                windows(FLAG_RELEASE),
                windows(FLAG_RECOVER)
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag(flag: i32, start: f32, end: f32) -> wscl::FlagWindow {
        wscl::FlagWindow { flag, start, end }
    }

    #[test]
    fn the_release_is_where_flag_90_opens_and_the_recovery_where_89_does() {
        // a00_6205: wind-up 0.23-0.77, release 0.77-1.23, recovery 1.23-1.73.
        let f = [
            flag(91, 0.23, 0.77),
            flag(90, 0.77, 1.23),
            flag(89, 1.23, 1.73),
        ];
        assert_eq!(timing(&f, None, 1.73), Some((0.77, 1.23)));
    }

    #[test]
    fn a_clip_with_no_recovery_window_recovers_when_it_ends() {
        // a00_6217: wind-up 0.20-0.70, release 0.70-1.47, no 89.
        let f = [flag(91, 0.20, 0.70), flag(90, 0.70, 1.47)];
        assert_eq!(timing(&f, None, 1.47), Some((0.70, 1.47)));
    }

    #[test]
    fn a_clip_with_no_release_window_is_no_cast() {
        // a00_6200 has only flag 91.
        let f = [flag(91, 0.17, 1.90)];
        assert_eq!(timing(&f, None, 1.90), None);
        assert_eq!(timing(&[], None, 1.0), None);
    }

    #[test]
    fn a_pyromancy_clip_releases_at_its_bullet_event_and_is_free_at_its_chain_flag() {
        // a00_6505 (Fireball, left hand): the bullet event at 0.50, the chain and evade flags open at
        // 1.50 (the move flag 0.07 later), the clip ends at 2.0; 87 and 7 are other windows.
        let f = [
            flag(7, 0.0, 2.17),
            flag(87, 0.67, 2.17),
            flag(4, 1.50, 2.17),
            flag(26, 1.50, 2.17),
            flag(11, 1.57, 2.17),
        ];
        assert_eq!(timing(&f, Some(0.50), 2.0), Some((0.50, 1.50)));
    }

    #[test]
    fn a_bullet_event_is_the_release_even_where_a_staff_flag_says_otherwise() {
        let f = [flag(90, 0.77, 1.23), flag(89, 1.23, 1.73)];
        assert_eq!(timing(&f, Some(0.40), 1.73), Some((0.40, 1.23)));
    }

    #[test]
    fn a_bullet_event_with_no_flag_to_leave_by_recovers_when_the_clip_ends() {
        assert_eq!(timing(&[], Some(0.37), 1.63), Some((0.37, 1.63)));
        assert_eq!(timing(&[], Some(2.5), 1.63), None, "released after the end");
    }

    #[test]
    fn the_release_event_is_the_bullet_else_the_first_hit_of_a_weapon_move() {
        let hit = |start: f32| wscl::HitWindow {
            judge: 0,
            start,
            end: start + 0.1,
        };
        assert_eq!(release_event(Some(0.5), &[hit(0.8)]), Some(0.5));
        assert_eq!(release_event(None, &[hit(0.9), hit(0.6)]), Some(0.6));
        assert_eq!(release_event(None, &[]), None);
    }

    #[test]
    fn every_role_clip_has_a_release_and_a_way_out_in_the_install() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            return;
        };
        let player = dsr::load(Path::new(&dir)).unwrap();
        for role in &ROLES {
            let clip = dsr::decode(&player, role.clip).unwrap();
            let duration = clip.frames.len() as f32 / clip.fps;
            let spawn = release_event(player.bullet_spawn(role.clip), &player.hits(role.clip));
            let (release, recover) = timing(&player.flags(role.clip), spawn, duration)
                .unwrap_or_else(|| panic!("{} ({}) has no release", role.name, role.clip));
            assert!(
                release < recover && recover <= duration + 0.05,
                "{}: {release} .. {recover} of {duration}",
                role.clip
            );
        }
    }

    #[test]
    fn a_blow_is_written_as_one_impact_line() {
        let b = params::SpellBlow {
            blast: 1.0,
            knock: 1.2,
            level: 3,
            poise: 40.0,
        };
        assert_eq!(impact_line("fireball", &b), "impact fireball 1 1.2 3 40");
    }

    #[test]
    fn a_release_after_the_recovery_or_past_the_end_is_refused() {
        let f = [flag(90, 1.0, 1.5), flag(89, 0.5, 1.5)];
        assert_eq!(timing(&f, None, 2.0), None, "recovery before the release");
        let f = [flag(90, 3.0, 4.0)];
        assert_eq!(timing(&f, None, 2.0), None, "release after the clip ends");
    }

    #[test]
    fn the_roles_name_four_distinct_clips_and_two_missiles() {
        let names: std::collections::BTreeSet<_> = ROLES.iter().map(|r| r.clip).collect();
        assert_eq!(names.len(), 4);
        assert_eq!(
            ROLES.iter().filter(|r| r.magic.is_some()).count(),
            3,
            "two missiles and Fire Blast's blow"
        );
        assert_eq!(
            ROLES.iter().map(|r| r.name).collect::<Vec<_>>(),
            vec!["fireball", "frostbolt", "fire_blast", "frost_armor"]
        );
    }
}
