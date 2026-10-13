//! `extract`: decode the roll and the straight sword's moves, retarget them onto the human, and
//! write `cache/dsr/`: per clip `human_male/<clip>.wscl` (WoW bones, local) and
//! `c0000/<clip>.wscl` (DS bones, model space, Bevy axes, WoW scale), then `manifest.txt` last, so
//! a cache with a manifest is a whole one.

use std::fmt::Write as _;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{bail, ensure, Context, Result};
use dsr_formats::hkx::skeleton::Skeleton;
use glam::{Quat, Vec3};

use crate::basis::Basis;
use crate::bonemap::{self, Rig};
use crate::dsr::Player;
use crate::retarget::Retargeted;
use crate::wow::WowSkel;
use crate::{dsr, loco, monster, moveset, params, reacts, retarget, roll, wow};

pub const MANIFEST_VERSION: u32 = 6;
/// The no-tearing criterion (spec, success criterion 2), as drift from the bind angle.
pub const MAX_ERROR_DEG: f32 = 15.0;
/// The worst a direction across the body (hip axis, spine base) may drift from its bind angle
/// to its DS counterpart.
pub const MAX_CROSS_DEG: f32 = 15.0;
/// A knocked-down body lies from the first frame its pelvis is this close to its lowest (yd).
pub const LIES_WITHIN: f32 = 0.05;
/// A knocked-down body is still this long (s) after its slide ends.
pub const SETTLE: f32 = 0.3;
/// The most the blade axis may differ between two frames, or two moves (deg): it is a constant
/// by construction.
pub const MAX_GRIP_DRIFT_DEG: f32 = 1.0;

pub(crate) fn source_line(path: &Path) -> Result<String> {
    let meta = std::fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    let mtime = meta.modified()?.duration_since(UNIX_EPOCH)?.as_secs();
    Ok(format!(
        "source {} {} {}",
        meta.len(),
        mtime,
        path.display()
    ))
}

fn write_clip(path: &Path, clip: &wscl::Clip) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    clip.write(&mut f)?;
    // Flush explicitly: BufWriter's drop swallows write errors.
    std::io::Write::flush(&mut f)?;
    Ok(())
}

/// Writes a clip's two files: on the WoW rig (`wow_dir`) and on the DS skeleton (`ds_dir`).
fn write_pair(out: &Path, wow_dir: &str, ds_dir: &str, name: &str, r: &Retargeted) -> Result<()> {
    write_clip(&out.join(wow_dir).join(format!("{name}.wscl")), &r.wow)?;
    write_clip(&out.join(ds_dir).join(format!("{name}.wscl")), &r.ds)
}

/// One clip, extracted.
struct Extracted {
    r: Retargeted,
    basis: Basis,
}

/// Decodes a clip, retargets it onto a WoW rig, checks it, and marks its hit windows.
fn extract_clip(player: &Player, skel: &WowSkel, rig: &Rig, name: &str) -> Result<Extracted> {
    let clip = dsr::decode(player, name)?;
    let mut r = retarget::retarget(&player.skeleton, &clip, skel, rig, player.flags(name))?;
    if r.max_error_deg > MAX_ERROR_DEG {
        bail!(
            "{name}: a mapped bone drifts {:.1}° from its bind angle to DS (limit {MAX_ERROR_DEG})",
            r.max_error_deg
        );
    }
    if r.max_cross_deg > MAX_CROSS_DEG {
        bail!(
            "{name}: a direction across the body drifts {:.1}° from its bind angle to DS \
             (limit {MAX_CROSS_DEG})",
            r.max_cross_deg
        );
    }
    if r.max_foot_gap > rig.checks.max_foot_gap {
        bail!(
            "{name}: on contact the feet are {:.3} yd off the DS feet's height \
             (limit {})",
            r.max_foot_gap,
            rig.checks.max_foot_gap
        );
    }
    let hits = player.hits(name);
    r.wow.hits = hits.clone();
    r.ds.hits = hits;
    let basis = retarget::basis_for(&player.skeleton, &clip, skel, rig)?;
    Ok(Extracted { r, basis })
}

/// The DS blade's axis in the retargeted WoW right-hand bone's local frame. The weapon bone sits
/// unrotated in the DS hand and the retarget carries the hand's rotation, so the vector is the
/// same in every frame; a clip where it drifts more than [`MAX_GRIP_DRIFT_DEG`] is an error.
fn grip_axis(e: &Extracted, ds: &Skeleton, rig: &Rig, blade_axis: Vec3) -> Result<Vec3> {
    let weapon = ds
        .bone(rig.names.weapon)
        .context("the DS weapon bone is missing")?;
    let hand = rig
        .pairs
        .iter()
        .find(|p| p.ds == rig.names.hand)
        .map(|p| p.wow)
        .context("the bone map has no DS hand")?;
    let blade = e.basis.dir(blade_axis);
    let parents = &e.r.wow.parents;
    // The hand and its ancestors, root first.
    let mut chain = vec![hand];
    while let Ok(p) = usize::try_from(parents[*chain.last().expect("never empty")]) {
        ensure!(p < parents[..].len(), "bone {p} is past the skeleton");
        chain.push(p);
    }
    chain.reverse();
    let mut first: Option<Vec3> = None;
    for (f, frame) in e.r.wow.frames.iter().enumerate() {
        let world = chain.iter().fold(Quat::IDENTITY, |w, &b| {
            (w * Quat::from_array(frame[b].r)).normalize()
        });
        let ds_weapon = Quat::from_array(e.r.ds.frames[f][weapon].r).normalize();
        let local = world.inverse() * (ds_weapon * blade);
        match first {
            None => first = Some(local),
            Some(a) => ensure!(
                a.angle_between(local).to_degrees() < MAX_GRIP_DRIFT_DEG,
                "frame {f}: the blade axis in the hand drifts {:.2}°",
                a.angle_between(local).to_degrees()
            ),
        }
    }
    first.context("the clip has no frames")
}

/// The worst checks over every extracted clip.
#[derive(Default)]
struct Worst {
    error: f32,
    cross: f32,
    foot: f32,
    foot_all: f32,
}

impl Worst {
    fn add(&mut self, r: &Retargeted) {
        self.error = self.error.max(r.max_error_deg);
        self.cross = self.cross.max(r.max_cross_deg);
        self.foot = self.foot.max(r.max_foot_gap);
        self.foot_all = self.foot_all.max(r.max_foot_gap_all);
    }
}

/// One console line for an extracted clip.
fn summary(name: &str, r: &Retargeted) -> String {
    let rm = r.wow.root_motion.last().copied().unwrap_or([0.0; 4]);
    let contact = if r.contact_frames == 0 {
        "no contact frames".to_string()
    } else {
        format!(
            "{:.3} yd over {} contact frames",
            r.max_foot_gap, r.contact_frames
        )
    };
    format!(
        "{name}: {} frames at {:.1} fps ({:.3} s), root motion ({:.2}, {:.2}, {:.2}) yd, yaw {:.1}°, \
         invulnerable {:?}, hit {:?}, max direction drift {:.2}°, max cross drift {:.2}°, \
         max foot gap {contact} ({:.3} yd over all frames)",
        r.wow.frames.len(),
        r.wow.fps,
        r.wow.duration(),
        rm[0],
        rm[1],
        rm[2],
        rm[3].to_degrees(),
        r.wow.windows(wscl::FLAG_INVINCIBLE).collect::<Vec<_>>(),
        r.wow
            .hits
            .iter()
            .map(|h| (h.start, h.end))
            .collect::<Vec<_>>(),
        r.max_error_deg,
        r.max_cross_deg,
        r.max_foot_gap_all
    )
}

/// Every kit of the game's creatures (`monster::KITS`), one after the other; returns their
/// manifest lines.
fn extract_monsters(
    dsr_dir: &Path,
    wow_dir: &Path,
    out: &Path,
    params: &params::Params,
    worst: &mut Worst,
) -> Result<String> {
    let mut lines = String::new();
    for kit in monster::KITS {
        lines.push_str(
            &extract_kit(kit, dsr_dir, wow_dir, out, params, worst)
                .with_context(|| format!("the {} kit", kit.name))?,
        );
    }
    Ok(lines)
}

/// One creature's kit: every clip retargeted onto its WoW model, written to `<kit.name>/` (WoW
/// bones, local) and `<kit.chr>/` (the Dark Souls creature's bones, model space); returns the
/// manifest's `monster` lines.
fn extract_kit(
    kit: &monster::Kit,
    dsr_dir: &Path,
    wow_dir: &Path,
    out: &Path,
    params: &params::Params,
    worst: &mut Worst,
) -> Result<String> {
    kit.check(params)?;
    let source = dsr::load_chr(dsr_dir, kit.chr)?;
    let skel = wow::skeleton(wow_dir, kit.wow_model)?;
    let rig = kit.rig;
    let bone = |n: &str| {
        source
            .skeleton
            .bone(n)
            .with_context(|| format!("{} has no bone {n}", kit.chr))
    };
    let (head, pelvis) = (bone(rig.names.head)?, bone(rig.names.pelvis)?);
    let wow_of = |ds: &str| {
        rig.pairs
            .iter()
            .find(|p| p.ds == ds)
            .map(|p| p.wow)
            .with_context(|| format!("the {} table has no {ds}", kit.name))
    };
    let (hand, forearm) = (wow_of(rig.names.hand)?, wow_of(rig.names.forearm)?);
    let npc = params.npc(kit.npc)?;
    let think = params.think(i32::try_from(npc.think)?)?;
    let mut lines = String::new();
    let mut extracted = std::collections::BTreeMap::new();
    for clip in kit.clips() {
        let e =
            extract_clip(&source, &skel, rig, clip).with_context(|| format!("kit clip {clip}"))?;
        worst.add(&e.r);
        write_pair(out, kit.name, kit.chr, clip, &e.r)?;
        println!("{}", summary(clip, &e.r));
        extracted.insert(clip, e);
    }
    let scale = extracted
        .values()
        .next()
        .map(|e| e.basis.scale)
        .context("the kit has no clips")?;
    writeln!(
        lines,
        "monster {} chr {} npc {} hp {} poise {} scale {scale:.4}",
        kit.name, kit.chr, kit.npc, npc.hp, npc.poise
    )?;
    writeln!(lines, "souls {} {}", kit.name, npc.get_soul)?;
    for a in kit.attacks {
        let e = &extracted[a.clip];
        let judge =
            e.r.wow
                .hits
                .first()
                .map(|h| h.judge)
                .with_context(|| format!("{} has no hit window", a.clip))?;
        ensure!(
            judge == a.judge,
            "{}: the TAE judge is {judge}, the kit says {}",
            a.clip,
            a.judge
        );
        // Every window of a multi-hit move is the same blow (one `AtkParam_Npc` row).
        ensure!(
            e.r.wow.hits.iter().all(|h| h.judge == judge),
            "{}: its hit windows carry different judge ids",
            a.clip
        );
        let n = params.npc_attack(kit.variation, judge)?;
        kit.tier.check_attack(a.name, n.level, e.r.wow.hits.len())?;
        writeln!(
            lines,
            "attack {} {} judge {judge} phys {} poise {} level {} radius {:.2} radius2 {:.2} stop {:.2} knock {:.2}",
            a.name, a.clip, n.phys, n.poise, n.level, n.radius, n.radius2, n.stop, n.knock
        )?;
    }
    for r in kit.reacts {
        let e = &extracted[r.clip];
        let (fwd, left) = monster::push(&e.r, head, pelvis)?;
        writeln!(
            lines,
            "react {} {} push {fwd:.3} {left:.3}",
            r.level, r.clip
        )?;
        println!(
            "  {} (level {}) pushes the head ({fwd:+.2} forward, {left:+.2} left)",
            r.clip, r.level
        );
    }
    for d in kit.deaths {
        let e = &extracted[d];
        // `WOWSOULS_LOOKDEV=1` only warns about a death that does not lie: for looking at a
        // candidate clip in the game (`WOWSOULS_DSVIEW=<species>:<clip>`) before choosing deaths.
        match monster::check_death(&e.r, pelvis, &kit.lying) {
            Err(why) if std::env::var_os("WOWSOULS_LOOKDEV").is_some() => {
                println!("  WARNING (look-dev): {why}");
            }
            checked => checked?,
        }
        let (fwd, left) = monster::push(&e.r, head, pelvis)?;
        writeln!(lines, "death {d} push {fwd:.3} {left:.3}")?;
        println!("  {d} (death) pushes the head ({fwd:+.2} forward, {left:+.2} left)");
    }
    writeln!(
        lines,
        "think near {:.1} mid {:.1} far {:.1} out {:.1}",
        think.near, think.mid, think.far, think.out
    )?;
    // The hit volume runs from the right hand along the forearm: a creature holds nothing.
    writeln!(lines, "reach {hand} {forearm}")?;
    Ok(lines)
}

/// The first time a flag's window opens (s), if the clip has one.
fn first_open(clip: &wscl::Clip, flag: i32) -> Option<f32> {
    clip.windows(flag)
        .map(|(start, _)| start)
        .min_by(f32::total_cmp)
}

/// The player's reaction kit: every candidate clip retargeted onto the human (written to
/// `human_male/` and `c0000/`), its head push measured, the times its control flags open
/// (attack, evade, movement) and its invulnerable window read; returns the manifest's `preact`
/// and `pdeath` lines. A death is a knockdown clip held on the frame where the pelvis first comes
/// within [`LIES_WITHIN`] of its lowest. Every clip's failure is collected and reported together.
fn extract_player_reacts(
    player: &Player,
    skel: &WowSkel,
    out: &Path,
    worst: &mut Worst,
) -> Result<String> {
    let rig = &bonemap::HUMAN_MALE_RIG;
    let bone = |n: &str| {
        player
            .skeleton
            .bone(n)
            .with_context(|| format!("the player has no bone {n}"))
    };
    let (head, pelvis) = (bone("Head")?, bone(rig.names.pelvis)?);
    let mut lines = String::new();
    let mut deaths = std::collections::BTreeMap::new();
    let mut failures = Vec::new();
    for r in reacts::REACTS {
        let result = (|| -> Result<String> {
            let e = extract_clip(player, skel, rig, r.clip)?;
            worst.add(&e.r);
            write_pair(out, "human_male", "c0000", r.clip, &e.r)?;
            let (fwd, left) = monster::push(&e.r, head, pelvis)?;
            let open = |flag, what: &str| {
                first_open(&e.r.wow, flag).with_context(|| format!("{}: no {what} window", r.clip))
            };
            let (chain, evade, mv) = (
                open(wscl::FLAG_CHAIN, "attack")?,
                open(wscl::FLAG_EVADE, "evade")?,
                open(wscl::FLAG_MOVE, "movement")?,
            );
            let down =
                e.r.wow
                    .windows(wscl::FLAG_INVINCIBLE)
                    .next()
                    .unwrap_or((0.0, 0.0));
            let rm = e.r.wow.root_motion.last().copied().unwrap_or([0.0; 4]);
            // Where the body lies: the first frame the pelvis is within LIES_WITHIN of its lowest.
            let heights: Vec<f32> = e.r.ds.frames.iter().map(|f| f[pelvis].t[1]).collect();
            let lowest = heights.iter().copied().fold(f32::MAX, f32::min);
            let lies = heights
                .iter()
                .position(|h| *h <= lowest + LIES_WITHIN)
                .unwrap_or(0) as f32
                / e.r.ds.fps;
            // ...and the slide it was thrown into has ended (99 % of the travel, plus a settling
            // moment): the body is still.
            let total = Vec3::new(rm[0], 0.0, rm[2]).length();
            let slid =
                e.r.wow
                    .root_motion
                    .iter()
                    .position(|r| Vec3::new(r[0], 0.0, r[2]).length() >= 0.99 * total)
                    .unwrap_or(0) as f32
                    / e.r.wow.fps;
            let lies = lies.max(slid + SETTLE);
            deaths.insert(r.clip, (fwd, left, lies));
            println!(
                "{}  [{}: {:.2} s, push ({fwd:+.2} forward, {left:+.2} left), control {chain:.2}/{evade:.2}/{mv:.2}, down {:.2}-{:.2}, travel ({:.2}, {:.2}), lies from {lies:.2} s]",
                summary(r.clip, &e.r),
                r.class,
                e.r.wow.duration(),
                down.0,
                down.1,
                rm[0],
                rm[2]
            );
            Ok(format!(
                "preact {} {} push {fwd:.3} {left:.3} chain {chain:.3} evade {evade:.3} move {mv:.3} down {:.3} {:.3}\n",
                r.class, r.clip, down.0, down.1
            ))
        })();
        match result {
            Ok(line) => lines.push_str(&line),
            Err(e) => failures.push(format!("{}: {e:#}", r.clip)),
        }
    }
    for clip in reacts::DEATHS {
        match deaths.get(clip) {
            Some((fwd, left, lies)) => writeln!(
                lines,
                "pdeath {clip} push {fwd:.3} {left:.3} hold {lies:.3}"
            )?,
            None => failures.push(format!("{clip}: a death that is no extracted reaction")),
        }
    }
    ensure!(
        failures.is_empty(),
        "{} reaction clip(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Ok(lines)
}

pub fn extract(dsr_dir: &Path, wow_dir: &Path, out: &Path, roll_name: Option<&str>) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    let skel = wow::human_male(wow_dir)?;
    let params = params::load(dsr_dir)?;
    let class = &moveset::STRAIGHT_SWORD;
    class.check(&params)?;
    let roll_name = match roll_name {
        Some(n) => n.to_string(),
        None => roll::pick(&roll::candidates(&player)?)
            .context("no roll candidate; run `list` and pass --roll")?
            .name
            .clone(),
    };

    // The backstep of the same weight class: the roll's stick-still twin (four invulnerable
    // frames, 2 m back), measured before it is trusted.
    let backstep_name = roll::pick_backstep(&roll::candidates(&player)?)
        .context("no backstep candidate: a00_0690 is not what was measured; run `list`")?
        .name
        .clone();

    let mut worst = Worst::default();
    let rig = &bonemap::HUMAN_MALE_RIG;
    let rolled = extract_clip(&player, &skel, rig, &roll_name)?;
    worst.add(&rolled.r);
    write_pair(out, "human_male", "c0000", &roll_name, &rolled.r)?;
    println!("{}", summary(&roll_name, &rolled.r));
    let backed = extract_clip(&player, &skel, rig, &backstep_name)?;
    worst.add(&backed.r);
    write_pair(out, "human_male", "c0000", &backstep_name, &backed.r)?;
    println!("{}  [backstep]", summary(&backstep_name, &backed.r));

    let mut move_lines = String::new();
    let mut grip: Option<(Vec3, Vec3)> = None;
    for m in class.moves {
        let e = extract_clip(&player, &skel, rig, m.clip)
            .with_context(|| format!("move {} ({})", m.name, m.clip))?;
        worst.add(&e.r);
        let judge =
            e.r.wow
                .hits
                .first()
                .map(|h| h.judge)
                .with_context(|| format!("{} has no hit window", m.clip))?;
        let b = params
            .behavior(class.variation, judge)
            .with_context(|| format!("move {} ({}), judge {judge}", m.name, m.clip))?;
        let hand = grip_axis(&e, &player.skeleton, rig, bonemap::DS_BLADE_AXIS)
            .with_context(|| m.clip.to_string())?;
        let weapon = e.basis.dir(bonemap::DS_BLADE_AXIS);
        match grip {
            None => grip = Some((hand, weapon)),
            Some((h, w)) => ensure!(
                h.angle_between(hand).to_degrees() < MAX_GRIP_DRIFT_DEG
                    && w.angle_between(weapon).to_degrees() < MAX_GRIP_DRIFT_DEG,
                "{}: its blade axis differs from the first move's",
                m.clip
            ),
        }
        write_pair(out, "human_male", "c0000", m.clip, &e.r)?;
        writeln!(
            move_lines,
            "move {} {} judge {judge} stamina {} mult {:.2} level {} poise {} stop {:.2} knock {:.2} radius {:.2}",
            m.name,
            m.clip,
            b.stamina,
            b.mult,
            b.level,
            b.poise,
            b.stop,
            b.knock,
            b.radius.max(b.radius2)
        )?;
        println!(
            "{}  [{}: judge {judge}, stamina {}, x{:.2}]",
            summary(m.clip, &e.r),
            m.name,
            b.stamina,
            b.mult
        );
    }
    let (hand, weapon) = grip.context("the class has no moves")?;
    let monster_lines = extract_monsters(dsr_dir, wow_dir, out, &params, &mut worst)?;
    let react_lines = extract_player_reacts(&player, &skel, out, &mut worst)?;

    let mut m = String::new();
    writeln!(m, "wscl-cache {MANIFEST_VERSION}")?;
    writeln!(m, "extractor {}", wscl::EXTRACTOR_VERSION)?;
    writeln!(m, "roll {roll_name}")?;
    writeln!(m, "backstep {backstep_name}")?;
    writeln!(m, "class {}", class.name)?;
    m.push_str(&move_lines);
    writeln!(
        m,
        "grip {:.5} {:.5} {:.5} {:.5} {:.5} {:.5}",
        hand.x, hand.y, hand.z, weapon.x, weapon.y, weapon.z
    )?;
    m.push_str(&monster_lines);
    m.push_str(&react_lines);
    writeln!(m, "max-error-deg {:.3}", worst.error)?;
    writeln!(m, "max-cross-deg {:.3}", worst.cross)?;
    writeln!(m, "max-foot-gap {:.4}", worst.foot)?;
    writeln!(m, "max-foot-gap-all {:.4}", worst.foot_all)?;
    for path in dsr::animation_archives(dsr_dir)? {
        writeln!(m, "{}", source_line(&path)?)?;
    }
    writeln!(
        m,
        "{}",
        source_line(&dsr_dir.join("chr").join("c0000.anibnd.dcx"))?
    )?;
    for kit in monster::KITS {
        writeln!(
            m,
            "{}",
            source_line(&dsr_dir.join("chr").join(format!("{}.anibnd.dcx", kit.chr)))?
        )?;
    }
    for file in [
        "param/GameParam/GameParam.parambnd.dcx",
        "paramdef/paramdef.paramdefbnd.dcx",
    ] {
        writeln!(m, "{}", source_line(&dsr_dir.join(file))?)?;
    }
    std::fs::write(out.join("manifest.txt"), m)?;
    extract_defense(dsr_dir, wow_dir, out)?;
    extract_loot(dsr_dir, wow_dir, out)?;
    extract_weapons(dsr_dir, wow_dir, out)?;
    extract_locomotion(dsr_dir, wow_dir, out)
}

/// All weapon families, independently of the existing roll/monster/loot cache.
pub fn extract_weapons(dsr_dir: &Path, wow_dir: &Path, out: &Path) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    let skel = wow::human_male(wow_dir)?;
    let params = params::load(dsr_dir)?;
    let rig = &bonemap::HUMAN_MALE_RIG;
    // A unique generation keeps a failed extraction from damaging a published cache.
    let generation = format!(
        "weapons-{}",
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis()
    );
    let root = out.join(&generation);
    let mut index = String::from("wsweapons 1\n");
    for family in moveset::FAMILIES {
        let dir = root.join(family.name);
        let w = params.weapon(family.weapon)?;
        let mut manifest = format!("wscl-cache {MANIFEST_VERSION}\nextractor {}\nclass {}\nweapon {}\nchain {}\ntwo-handed {}\n", wscl::EXTRACTOR_VERSION, family.name, family.weapon, family.chain, u8::from(family.two_handed));
        let mut grip = None;
        let hand_bone = rig
            .pairs
            .iter()
            .find(|p| p.ds == rig.names.hand)
            .context("hand bone")?
            .wow;
        let weapon_bone = player
            .skeleton
            .bone(rig.names.weapon)
            .context("weapon bone")?;
        writeln!(manifest, "grip-bones {hand_bone} {weapon_bone}")?;
        writeln!(
            manifest,
            "base-damage {} {}",
            params.weapon_damage(family.weapon)?,
            params.weapon_damage(200000)?
        )?;
        let motion = params.weapon_motion(family.weapon)?;
        let idle = format!(
            "a{:02}_0000",
            if family.two_handed {
                motion.1
            } else {
                motion.0
            }
        );
        let e = extract_clip(&player, &skel, rig, &idle)?;
        write_pair(&dir, "human_male", "c0000", &idle, &e.r)?;
        writeln!(manifest, "idle {idle}")?;
        let guard = params.weapon_guard(family.weapon)?;
        writeln!(
            manifest,
            "guard {} {} {}",
            guard.cut / 100.0,
            guard.stability,
            u8::from(guard.enabled)
        )?;
        for (name, id) in [("raise", 100), ("hold", 1), ("lower", 101)] {
            let clip = format!("a{:02}_{id:04}", motion.1);
            let e = extract_clip(&player, &skel, rig, &clip)?;
            write_pair(&dir, "human_male", "c0000", &clip, &e.r)?;
            writeln!(manifest, "guardclip {name} {clip}")?;
        }
        for m in family.moves(&params)? {
            let mut e = extract_clip(&player, &skel, rig, &m.clip)
                .with_context(|| format!("{} {}", family.name, m.clip))?;
            // Fist strikes use PCBehavior(type 16), not InvokeAttackBehavior.
            if w.motion_category == 42 {
                let hits: Vec<_> = player
                    .events(&m.clip)
                    .iter()
                    .filter(|ev| ev.kind == 307 && ev.param_i32(1) == Some(16))
                    .filter_map(|ev| {
                        Some(wscl::HitWindow {
                            judge: ev.param_i32(2)?,
                            start: ev.start,
                            end: ev.end,
                        })
                    })
                    .collect();
                e.r.wow.hits = hits.clone();
                e.r.ds.hits = hits;
            }
            ensure!(!e.r.wow.hits.is_empty(), "{} has no attack windows", m.clip);
            let mut b = params.behavior(w.variation, e.r.wow.hits[0].judge)?;
            b.poise += params.weapon_poise(family.weapon)? * b.poise_correction;
            let weapon = e.basis.dir(bonemap::DS_BLADE_AXIS);
            // Dagger R2 rotates the weapon within the palm. New family caches carry bone IDs
            // so runtime follows its authored weapon orientation on every sampled frame.
            let mut rotation = Quat::IDENTITY;
            let mut bone = Some(hand_bone);
            while let Some(i) = bone {
                rotation = Quat::from_array(e.r.wow.frames[0][i].r) * rotation;
                bone = usize::try_from(e.r.wow.parents[i]).ok();
            }
            let hand =
                rotation.inverse() * (Quat::from_array(e.r.ds.frames[0][weapon_bone].r) * weapon);
            grip = Some((hand, weapon));
            writeln!(manifest, "move {} {} judge {} stamina {} mult {} level {} poise {} stop {} knock {} radius {}", m.name, m.clip, e.r.wow.hits[0].judge, b.stamina, b.mult, b.level, b.poise, b.stop, b.knock, b.radius.max(b.radius2))?;
            for hit in &e.r.wow.hits {
                let mut hit_b = params.behavior(w.variation, hit.judge)?;
                hit_b.poise += params.weapon_poise(family.weapon)? * hit_b.poise_correction;
                writeln!(
                    manifest,
                    "hit {} {} {} {} {} {} {} {} {} {}",
                    m.name,
                    hit.judge,
                    hit.start,
                    hit.end,
                    hit_b.mult,
                    hit_b.level,
                    hit_b.poise,
                    hit_b.stop,
                    hit_b.knock,
                    hit_b.radius.max(hit_b.radius2)
                )?;
            }
            write_pair(&dir, "human_male", "c0000", &m.clip, &e.r)?;
            println!(
                "{} {} stamina {} mult {} {}",
                family.name,
                m.name,
                b.stamina,
                b.mult,
                summary(&m.clip, &e.r)
            );
        }
        let (h, w) = grip.context("missing grip")?;
        writeln!(
            manifest,
            "grip {} {} {} {} {} {}",
            h.x, h.y, h.z, w.x, w.y, w.z
        )?;
        for path in dsr::animation_archives(dsr_dir)?.into_iter().chain([
            dsr_dir.join("chr/c0000.anibnd.dcx"),
            dsr_dir.join("param/GameParam/GameParam.parambnd.dcx"),
            dsr_dir.join("paramdef/paramdef.paramdefbnd.dcx"),
        ]) {
            writeln!(manifest, "{}", source_line(&path)?)?;
        }
        std::fs::write(dir.join("manifest.txt"), manifest)?;
        writeln!(index, "family {} {generation}/{}", family.name, family.name)?;
    }
    for (name, shot, draw, weapon, speed, gravity, damage) in [
        (
            "bow",
            "a44_4000",
            Some("a14_0100"),
            1201000,
            45.0,
            1.5,
            24.0,
        ),
        ("crossbow", "a46_4000", None, 1250000, 55.0, 0.6, 30.0),
        ("gun", "a46_4000", None, 1250000, 90.0, 0.0, 32.0),
        ("thrown", "a00_7582", None, 0, 28.0, 3.0, 16.0),
    ] {
        let dir = root.join(name);
        let mut e = extract_clip(&player, &skel, rig, shot)?;
        let event = player
            .events(shot)
            .iter()
            .find(|e| e.kind == if weapon == 0 { 65 } else { 2 })
            .context("projectile release event")?;
        let mut release = event.start;
        let judge = if weapon == 0 {
            0
        } else {
            event.param_i32(2).context("bullet judge")?
        };
        let stamina = if weapon == 0 {
            10
        } else {
            params.projectile_stamina(params.weapon(weapon)?.variation, judge)?
        };
        if let Some(draw) = draw {
            let prefix = extract_clip(&player, &skel, rig, draw)?;
            release += prefix.r.wow.duration();
            prepend(&mut e.r.wow, &prefix.r.wow);
            prepend(&mut e.r.ds, &prefix.r.ds);
        }
        // The source shot starts already aimed. Guns use the same source motion with a
        // deliberately estimated 0.3 s preparation; this is not a Bloodborne asset.
        if name == "gun" {
            hold_start(&mut e.r.wow, 9);
            hold_start(&mut e.r.ds, 9);
            release += 0.3;
        }
        for c in [&mut e.r.wow, &mut e.r.ds] {
            c.name = format!("{name}_shot");
            c.hits = vec![wscl::HitWindow {
                judge,
                start: release,
                end: release + 1.0 / 30.0,
            }];
            // No authored early cancel (crossbow): commitment lasts through the whole clip.
            for flag in [wscl::FLAG_CHAIN, wscl::FLAG_EVADE, wscl::FLAG_MOVE] {
                if c.windows(flag).next().is_none() {
                    c.flags.push(wscl::FlagWindow {
                        flag,
                        start: c.duration(),
                        end: c.duration() + 1.0 / 30.0,
                    });
                }
            }
        }
        let mut manifest = format!("wscl-cache {MANIFEST_VERSION}\nextractor {}\nclass {name}\nchain 1\nprojectile {release} {speed} {gravity} {damage}\nprovenance source-motion:{shot} projectile-physics:estimated\n", wscl::EXTRACTOR_VERSION);
        writeln!(manifest, "two-handed {}", u8::from(name != "thrown"))?;
        let idle = format!(
            "a{:02}_0000",
            if weapon == 0 {
                0
            } else {
                params.weapon_motion(weapon)?.1
            }
        );
        let resting = extract_clip(&player, &skel, rig, &idle)?;
        write_pair(&dir, "human_male", "c0000", &idle, &resting.r)?;
        writeln!(manifest, "idle {idle}")?;
        // Bow is held by the left hand. Other projectiles use the right hand.
        let hand_bone = if name == "bow" { 95 } else { 97 };
        let ds_bone = player
            .skeleton
            .bone(if name == "bow" {
                "L_Weapon"
            } else {
                rig.names.weapon
            })
            .context("ranged weapon bone")?;
        // Crossbow's barrel runs along source +X, unlike a melee blade's +Y.
        // This mount convention is also used by the explicitly adapted gun profile.
        let axis = e.basis.dir(if matches!(name, "crossbow" | "gun") {
            Vec3::X
        } else {
            bonemap::DS_BLADE_AXIS
        });
        writeln!(
            manifest,
            "grip-bones {hand_bone} {ds_bone}\ngrip 0 1 0 {} {} {}",
            axis.x, axis.y, axis.z
        )?;
        for name in ["r1_1", "r1_2", "r1_3", "r2_1", "r2_2", "roll_attack"] {
            writeln!(manifest, "move {name} {} judge {judge} stamina {stamina} mult 1 level 1 poise 8 stop 0 knock 0 radius 0.06",e.r.wow.name)?;
        }
        write_pair(&dir, "human_male", "c0000", &e.r.wow.name, &e.r)?;
        for path in dsr::animation_archives(dsr_dir)?.into_iter().chain([
            dsr_dir.join("chr/c0000.anibnd.dcx"),
            dsr_dir.join("param/GameParam/GameParam.parambnd.dcx"),
            dsr_dir.join("paramdef/paramdef.paramdefbnd.dcx"),
        ]) {
            writeln!(manifest, "{}", source_line(&path)?)?;
        }
        std::fs::write(dir.join("manifest.txt"), manifest)?;
        writeln!(index, "family {name} {generation}/{name}")?;
        println!("{name}: source {shot}, release {release:.3}, stamina {stamina}, duration {:.3}; projectile physics estimated",e.r.wow.duration());
    }
    std::fs::write(out.join("weapons.txt"), index)?;
    Ok(())
}

/// Join two original segments without changing either segment's playback speed.
fn prepend(clip: &mut wscl::Clip, prefix: &wscl::Clip) {
    let offset = prefix.duration();
    let mut frames = prefix.frames[..prefix.frames.len() - 1].to_vec();
    frames.append(&mut clip.frames);
    clip.frames = frames;
    let mut root = prefix.root_motion[..prefix.root_motion.len() - 1].to_vec();
    let base = *prefix.root_motion.last().unwrap();
    let origin = clip.root_motion[0];
    root.extend(
        clip.root_motion
            .iter()
            .map(|p| std::array::from_fn(|i| p[i] - origin[i] + base[i])),
    );
    clip.root_motion = root;
    for flag in &mut clip.flags {
        flag.start += offset;
        flag.end += offset;
    }
}

fn hold_start(clip: &mut wscl::Clip, frames: usize) {
    clip.frames
        .splice(0..0, vec![clip.frames[0].clone(); frames]);
    clip.root_motion
        .splice(0..0, vec![clip.root_motion[0]; frames]);
    let shift = frames as f32 / clip.fps;
    for flag in &mut clip.flags {
        flag.start += shift;
        flag.end += shift;
    }
}

/// The player's own locomotion (`locomotion.txt`): the walk, the run, the sprint and the four
/// locked-on directions of each weapon motion set the families and ranged weapons use, so the
/// body carries its weapon the way Dark Souls does while it travels. Clips are shared by name in
/// `human_male/` and `c0000/` (a set that has no clip of its own uses the common set's), and the
/// publication is independent of the combat kits, like the interactions.
pub fn extract_locomotion(dsr_dir: &Path, wow_dir: &Path, out: &Path) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    let skel = wow::human_male(wow_dir)?;
    let params = params::load(dsr_dir)?;
    let rig = &bonemap::HUMAN_MALE_RIG;
    // The sets the idle clips come from (`extract_weapons`), and the common one as the fallback.
    let mut sets = std::collections::BTreeSet::from([0_i64]);
    for family in moveset::FAMILIES {
        let motion = params.weapon_motion(family.weapon)?;
        sets.insert(if family.two_handed {
            motion.1
        } else {
            motion.0
        });
    }
    for ranged in [1201000, 1250000] {
        sets.insert(params.weapon_motion(ranged)?.1);
    }
    let mut lines = String::from("locomotion-version 1\n");
    for archive in dsr::animation_archives(dsr_dir)? {
        writeln!(lines, "{}", source_line(&archive)?)?;
    }
    let mut written = std::collections::BTreeSet::new();
    for set in sets {
        for role in &loco::ROLES {
            let name = loco::clip_for(&player, set, role)
                .with_context(|| format!("set a{set:02} has no {} clip", role.name))?;
            if written.insert(name.clone()) {
                let e = extract_clip(&player, &skel, rig, &name)
                    .with_context(|| format!("locomotion {} ({name})", role.name))?;
                ensure!(
                    e.r.wow.windows(wscl::FLAG_INVINCIBLE).next().is_none()
                        && e.r.wow.hits.is_empty(),
                    "{name}: a gait clip must carry no hit and no invulnerability"
                );
                println!(
                    "locomotion a{set:02} {}: {}",
                    role.name,
                    summary(&name, &e.r)
                );
                write_pair(out, "human_male", "c0000", &name, &e.r)?;
            }
            writeln!(lines, "loco {set} {} {name}", role.name)?;
        }
    }
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("locomotion.txt"), lines)?;
    Ok(())
}

/// Contextual interactions. Separate publication keeps optional clips independent of combat.
pub fn extract_loot(dsr_dir: &Path, wow_dir: &Path, out: &Path) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    let skel = wow::human_male(wow_dir)?;
    let mut lines = String::from("interaction-version 1\n");
    for archive in dsr::animation_archives(dsr_dir)? {
        writeln!(lines, "{}", source_line(&archive)?)?;
    }
    for (role, name) in [
        ("ground", "a00_7520"),
        ("open", "a00_7521"),
        ("take", "a00_7522"),
    ] {
        let e = extract_clip(&player, &skel, &bonemap::HUMAN_MALE_RIG, name)?;
        ensure!(
            e.r.wow.windows(wscl::FLAG_INVINCIBLE).next().is_none(),
            "loot must not confer invulnerability"
        );
        println!("loot {role}: {}", summary(name, &e.r));
        for event in player.events(name) {
            println!("  source event {event:?}");
        }
        // Model-space hand samples expose reach/lift timing for the native chest lid.
        for f in (0..e.r.wow.frames.len()).step_by(5) {
            let frame = &e.r.wow.frames[f];
            for (side, bone) in [("left", 95usize), ("right", 97usize)] {
                let mut chain = vec![bone];
                while let Ok(p) = usize::try_from(e.r.wow.parents[*chain.last().unwrap()]) {
                    chain.push(p);
                }
                let world = chain.iter().rev().fold(glam::Mat4::IDENTITY, |m, &b| {
                    let t = frame[b];
                    m * glam::Mat4::from_scale_rotation_translation(
                        Vec3::from_array(t.s),
                        Quat::from_array(t.r),
                        Vec3::from_array(t.t),
                    )
                });
                println!(
                    "  {side} hand t={:.3}: {:?}",
                    f as f32 / e.r.wow.fps,
                    world.transform_point3(Vec3::ZERO)
                );
            }
        }
        write_pair(out, "human_male", "c0000", name, &e.r)?;
        writeln!(lines, "interact {role} {name}")?;
    }
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("interaction.txt"), lines)?;
    Ok(())
}

/// Optional defense records, published only after all their clips pass the same retarget gates.
/// This command extends an existing cache without replacing the world data or its combat kits.
pub fn extract_defense(dsr_dir: &Path, wow_dir: &Path, out: &Path) -> Result<()> {
    let path = out.join("manifest.txt");
    let old = std::fs::read_to_string(&path).context("extract the base cache before defense")?;
    ensure!(
        old.lines().next() == Some("wscl-cache 6"),
        "defense needs cache version 6"
    );
    let player = dsr::load(dsr_dir)?;
    let skel = wow::human_male(wow_dir)?;
    let params = params::load(dsr_dir)?;
    let guard = params.weapon_guard(200000)?;
    ensure!(
        guard.enabled && !guard.parry,
        "the Shortsword guard row changed"
    );
    ensure!(params.weapon_guard(900000)?.parry, "Fists no longer parry");
    let parry = params.behavior(4201, 450)?;
    let mut lines = String::from("defense-version 1\n");
    writeln!(
        lines,
        "defense cut {:.3} stability {} hand 95",
        guard.cut / 100.0,
        guard.stability
    )?;
    writeln!(
        lines,
        "defparry stamina {} radius {:.3}",
        parry.stamina, parry.radius
    )?;
    let rig = &bonemap::HUMAN_MALE_RIG;
    let head = player
        .skeleton
        .bone(rig.names.head)
        .context("player head")?;
    let pelvis = player
        .skeleton
        .bone(rig.names.pelvis)
        .context("player pelvis")?;
    for (name, clip) in [
        ("raise", "a10_0100"),
        ("hold", "a10_0001"),
        ("lower", "a10_0101"),
        ("parry", "a42_5100"),
        ("break0", "a00_2080"),
        ("break1", "a00_2081"),
        ("break2", "a00_2082"),
        ("break3", "a00_2083"),
    ] {
        let e =
            extract_clip(&player, &skel, rig, clip).with_context(|| format!("defense {name}"))?;
        if name == "raise" || name == "hold" {
            ensure!(
                e.r.wow.windows(3).next().is_some(),
                "{clip}: no guard flag 3"
            );
        }
        if name == "parry" {
            ensure!(
                player
                    .events(clip)
                    .iter()
                    .any(|x| x.attack_judge() == Some(450) && x.param_i32(0) == Some(64)),
                "{clip}: no parry behavior"
            );
            ensure!(
                e.r.wow.windows(wscl::FLAG_INVINCIBLE).next().is_none(),
                "parry must not gain i-frames"
            );
        }
        let (fwd, left) = if name.starts_with("break") {
            monster::push(&e.r, head, pelvis)?
        } else {
            (0.0, 0.0)
        };
        writeln!(lines, "defclip {name} {clip} push {fwd:.4} {left:.4}")?;
        if name == "hold" {
            let hand = grip_axis(&e, &player.skeleton, rig, bonemap::DS_BLADE_AXIS)?;
            let weapon = e.basis.dir(bonemap::DS_BLADE_AXIS);
            writeln!(
                lines,
                "defgrip {:.5} {:.5} {:.5} {:.5} {:.5} {:.5}",
                hand.x, hand.y, hand.z, weapon.x, weapon.y, weapon.z
            )?;
        }
        write_pair(out, "human_male", "c0000", clip, &e.r)?;
        println!("defense {name}: {}", summary(clip, &e.r));
    }
    let mut parried_kits = Vec::new();
    for kit in monster::KITS {
        for a in kit.attacks {
            let g = params.npc_guard_attack(kit.variation, a.judge)?;
            writeln!(
                lines,
                "defattack {} {} rate {} unblockable {}",
                kit.name,
                a.name,
                g.rate,
                u8::from(g.unblockable)
            )?;
        }
        let source = dsr::load_chr(dsr_dir, kit.chr)?;
        if source.hkx.contains_key("a00_2100") {
            let skel = wow::skeleton(wow_dir, kit.wow_model)?;
            let e = extract_clip(&source, &skel, kit.rig, "a00_2100")?;
            write_pair(out, kit.name, kit.chr, "a00_2100", &e.r)?;
            parried_kits.push(kit.name);
            println!("{} parried: {}", kit.name, summary("a00_2100", &e.r));
        }
    }
    let mut manifest = String::new();
    for line in old
        .lines()
        .filter(|l| !l.starts_with("def") && !l.starts_with("parried "))
    {
        writeln!(manifest, "{line}")?;
        if let Some(kit) = line
            .strip_prefix("monster ")
            .and_then(|s| s.split_whitespace().next())
        {
            if parried_kits.contains(&kit) {
                writeln!(manifest, "parried a00_2100")?;
            }
        }
    }
    manifest.push_str(&lines);
    std::fs::write(path, manifest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("dsr-cache-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn written_clip_reads_back() {
        let dir = temp_dir("clip");
        let clip = wscl::Clip {
            name: "a00_test".into(),
            fps: 30.0,
            model_space: false,
            parents: vec![-1, 0],
            driven: vec![true, false],
            frames: vec![
                vec![wscl::Trs::IDENTITY; 2],
                vec![
                    wscl::Trs {
                        t: [1.0, 2.0, 3.0],
                        ..wscl::Trs::IDENTITY
                    };
                    2
                ],
            ],
            root_motion: vec![[0.0; 4], [0.0, 0.0, -1.5, 0.25]],
            flags: vec![wscl::FlagWindow {
                flag: wscl::FLAG_INVINCIBLE,
                start: 0.0,
                end: 0.033,
            }],
            hits: vec![wscl::HitWindow {
                judge: 7,
                start: 0.01,
                end: 0.02,
            }],
        };
        let path = dir.join("nested").join("a00_test.wscl");
        write_clip(&path, &clip).unwrap();
        let back = wscl::Clip::read(&mut std::fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(back, clip);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn source_line_reports_length_and_path() {
        let dir = temp_dir("source");
        let path = dir.join("blob.bin");
        std::fs::write(&path, [7u8; 123]).unwrap();
        let line = source_line(&path).unwrap();
        let mut parts = line.splitn(4, ' ');
        assert_eq!(parts.next(), Some("source"));
        assert_eq!(parts.next(), Some("123"));
        assert!(parts.next().unwrap().parse::<u64>().is_ok());
        assert_eq!(parts.next(), Some(path.display().to_string().as_str()));
        assert!(source_line(&dir.join("missing")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn install_the_moves_match_the_data_table() {
        let (Ok(dsr_dir), Ok(wow_dir)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("DSR_DIR and WOW_DATA not both set; skipped");
            return;
        };
        let player = dsr::load(Path::new(&dsr_dir)).unwrap();
        assert_eq!(player.skeleton.bone(bonemap::C0000.hand), Some(45));
        assert_eq!(
            player.skeleton.bone(bonemap::C0000.weapon),
            Some(56),
            "the game names the DS weapon bone by this index"
        );
        let skel = wow::human_male(Path::new(&wow_dir)).unwrap();
        let params = params::load(Path::new(&dsr_dir)).unwrap();
        // clip, judge, length s, hit window s, f4 / f26 / f11 s, lunge yd, stamina, damage x.
        #[allow(clippy::type_complexity)]
        let table: [(&str, i32, f32, (f32, f32), [f32; 3], f32, i64, f32); 6] = [
            (
                "a23_3000",
                0,
                2.500,
                (0.533, 0.667),
                [0.767, 0.900, 1.033],
                1.98,
                20,
                1.00,
            ),
            (
                "a23_3001",
                10,
                2.233,
                (0.467, 0.567),
                [0.733, 0.900, 1.000],
                1.42,
                20,
                1.00,
            ),
            (
                "a23_3002",
                20,
                2.500,
                (0.467, 0.600),
                [0.767, 0.900, 1.033],
                1.98,
                20,
                1.00,
            ),
            (
                "a23_3300",
                100,
                2.533,
                (0.733, 0.867),
                [1.300, 1.300, 1.367],
                1.98,
                45,
                1.35,
            ),
            (
                "a23_3310",
                110,
                1.900,
                (0.333, 0.467),
                [0.933, 0.933, 1.200],
                0.94,
                35,
                1.25,
            ),
            (
                "a23_3900",
                540,
                2.100,
                (0.233, 0.333),
                [0.700, 0.767, 0.867],
                1.42,
                20,
                0.90,
            ),
        ];
        let rig = &bonemap::HUMAN_MALE_RIG;
        for (clip, judge, len, hit, cancels, lunge, stamina, mult) in table {
            let e = extract_clip(&player, &skel, rig, clip).unwrap();
            let w = &e.r.wow;
            assert!(
                (w.duration() - len).abs() < 0.002,
                "{clip}: {}",
                w.duration()
            );
            let h = w
                .hits
                .first()
                .unwrap_or_else(|| panic!("{clip}: no hit window"));
            assert_eq!(h.judge, judge, "{clip}");
            assert!(
                (h.start - hit.0).abs() < 0.001,
                "{clip}: hit start {}",
                h.start
            );
            assert!((h.end - hit.1).abs() < 0.001, "{clip}: hit end {}", h.end);
            for (flag, want) in [
                (wscl::FLAG_CHAIN, cancels[0]),
                (wscl::FLAG_EVADE, cancels[1]),
                (wscl::FLAG_MOVE, cancels[2]),
            ] {
                let got = w
                    .windows(flag)
                    .map(|(s, _)| s)
                    .fold(f32::INFINITY, f32::min);
                assert!(
                    (got - want).abs() < 0.001,
                    "{clip}: flag {flag} opens at {got}"
                );
            }
            let z = -w.root_motion.last().unwrap()[2];
            assert!((z - lunge).abs() < 0.02 * lunge, "{clip}: lunge {z} yd");
            let b = params.behavior(2300, judge).unwrap();
            assert_eq!(b.stamina, stamina, "{clip}");
            assert!((b.mult - mult).abs() < 1.0e-6, "{clip}: {}", b.mult);
            grip_axis(&e, &player.skeleton, rig, bonemap::DS_BLADE_AXIS).unwrap();
        }
    }

    #[test]
    fn install_the_kobolds_moves_match_the_data_table() {
        let (Ok(dsr_dir), Ok(wow_dir)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("DSR_DIR and WOW_DATA not both set; skipped");
            return;
        };
        let ghoul = dsr::load_chr(Path::new(&dsr_dir), "c2060").unwrap();
        let skel = wow::skeleton(Path::new(&wow_dir), wow::KOBOLD_M2).unwrap();
        let rig = &bonemap::KOBOLD_RIG;
        // clip, judge, length s, hit window s, lunge in DS metres (the clip's root motion).
        let table = [
            ("a00_3000", 100, 2.867, (0.767, 0.933), 1.59),
            ("a00_3001", 110, 2.167, (0.833, 1.033), 0.49),
            ("a00_3002", 120, 3.167, (1.433, 1.667), 1.35),
            ("a00_3003", 130, 3.167, (0.767, 0.967), 3.17),
            ("a00_3004", 140, 1.400, (0.600, 0.833), 1.43),
            ("a00_3006", 160, 3.333, (2.067, 2.167), 3.37),
        ];
        for (clip, judge, len, hit, lunge) in table {
            let e = extract_clip(&ghoul, &skel, rig, clip).unwrap();
            let w = &e.r.wow;
            assert!(
                (w.duration() - len).abs() < 0.002,
                "{clip}: {}",
                w.duration()
            );
            let h = w.hits.first().unwrap();
            assert_eq!(h.judge, judge, "{clip}");
            assert!(
                (h.start - hit.0).abs() < 0.001 && (h.end - hit.1).abs() < 0.001,
                "{clip}: {h:?}"
            );
            // The clip's root motion times the leg ratio is what the kobold travels (+-2 %).
            let z = -w.root_motion.last().unwrap()[2];
            let want = lunge * e.basis.scale;
            assert!(
                (z - want).abs() < 0.02 * want.max(0.1),
                "{clip}: lunge {z} vs {want}"
            );
        }
    }

    #[test]
    fn install_the_wolfs_moves_match_the_data_table() {
        let (Ok(dsr_dir), Ok(wow_dir)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("DSR_DIR and WOW_DATA not both set; skipped");
            return;
        };
        let dog = dsr::load_chr(Path::new(&dsr_dir), "c3340").unwrap();
        let skel = wow::skeleton(Path::new(&wow_dir), wow::WOLF_M2).unwrap();
        let rig = &bonemap::WOLF_RIG;
        // clip, judge, length s, hit window s, travel in DS metres (the clip's root motion): the
        // survey of the black dog (`wow-souls-dsr survey --chr c3340 --npc 334000`).
        let table = [
            ("a00_3000", 100, 1.667, (0.633, 0.867), 0.65),
            ("a00_3004", 130, 1.500, (0.333, 0.700), 2.07),
        ];
        for (clip, judge, len, hit, travel) in table {
            let e = extract_clip(&dog, &skel, rig, clip).unwrap();
            let w = &e.r.wow;
            assert!(
                (w.duration() - len).abs() < 0.002,
                "{clip}: {}",
                w.duration()
            );
            assert_eq!(w.hits.len(), 1, "{clip}: one hit window");
            let h = w.hits.first().unwrap();
            assert_eq!(h.judge, judge, "{clip}");
            assert!(
                (h.start - hit.0).abs() < 0.001 && (h.end - hit.1).abs() < 0.001,
                "{clip}: {h:?}"
            );
            // The clip's root motion times the scale is what the wolf travels (+-2 %).
            let z = -w.root_motion.last().unwrap()[2];
            let want = travel * e.basis.scale;
            assert!(
                (z - want).abs() < 0.02 * want.max(0.1),
                "{clip}: travel {z} vs {want}"
            );
        }
        // The hind legs stand the body on the ground: a scale between 1.2 and 1.4.
        let scale = extract_clip(&dog, &skel, rig, "a00_3000")
            .unwrap()
            .basis
            .scale;
        assert!((1.2..1.4).contains(&scale), "scale {scale}");
    }

    #[test]
    fn install_the_thiefs_moves_match_the_data_table() {
        let (Ok(dsr_dir), Ok(wow_dir)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("DSR_DIR and WOW_DATA not both set; skipped");
            return;
        };
        let thief = dsr::load_chr(Path::new(&dsr_dir), "c2520").unwrap();
        let skel = wow::skeleton(Path::new(&wow_dir), wow::HUMAN_MALE_M2).unwrap();
        let rig = &bonemap::THIEF_RIG;
        // clip, judge, length s, hit window s, travel in DS metres (the clip's root motion): the
        // survey of the undead thief (`wow-souls-dsr survey --chr c2520 --npc 252000`).
        let table = [
            ("a00_3003", 130, 2.333, (0.867, 0.967), 1.54),
            ("a00_3004", 140, 2.100, (0.733, 0.900), 0.94),
        ];
        for (clip, judge, len, hit, travel) in table {
            let e = extract_clip(&thief, &skel, rig, clip).unwrap();
            let w = &e.r.wow;
            assert!(
                (w.duration() - len).abs() < 0.002,
                "{clip}: {}",
                w.duration()
            );
            assert_eq!(w.hits.len(), 1, "{clip}: one hit window");
            let h = w.hits.first().unwrap();
            assert_eq!(h.judge, judge, "{clip}");
            assert!(
                (h.start - hit.0).abs() < 0.001 && (h.end - hit.1).abs() < 0.001,
                "{clip}: {h:?}"
            );
            // The clip's root motion times the leg ratio is what the thief travels (+-2 %).
            let z = -w.root_motion.last().unwrap()[2];
            let want = travel * e.basis.scale;
            assert!(
                (z - want).abs() < 0.02 * want.max(0.1),
                "{clip}: travel {z} vs {want}"
            );
        }
        // The thief's legs are the player's size.
        let scale = extract_clip(&thief, &skel, rig, "a00_3003")
            .unwrap()
            .basis
            .scale;
        assert!((1.0..1.3).contains(&scale), "scale {scale}");
    }

    #[test]
    fn install_garricks_moves_match_the_data_table() {
        let (Ok(dsr_dir), Ok(wow_dir)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("DSR_DIR and WOW_DATA not both set; skipped");
            return;
        };
        let assassin = dsr::load_chr(Path::new(&dsr_dir), "c2400").unwrap();
        let skel = wow::skeleton(Path::new(&wow_dir), wow::HUMAN_MALE_M2).unwrap();
        let rig = &bonemap::HUMAN_MALE_RIG;
        // clip, judge, length s, hit windows s, travel in DS metres: the survey of the assassin
        // (`wow-souls-dsr survey --chr c2400 --npc 240000`).
        let table: [(&str, i32, f32, &[(f32, f32)], f32); 5] = [
            (
                "a00_3002",
                120,
                2.733,
                &[(0.533, 0.767), (1.167, 1.400)],
                2.95,
            ),
            (
                "a00_3003",
                130,
                2.667,
                &[(0.567, 0.700), (1.033, 1.133), (1.533, 1.667)],
                1.82,
            ),
            ("a00_3005", 150, 2.267, &[(0.867, 1.000)], 3.50),
            ("a00_3006", 160, 1.633, &[(0.500, 0.567)], 3.99),
            ("a00_3008", 180, 1.333, &[(0.333, 0.467)], 1.30),
        ];
        for (clip, judge, len, windows, travel) in table {
            let e = extract_clip(&assassin, &skel, rig, clip).unwrap();
            let w = &e.r.wow;
            assert!(
                (w.duration() - len).abs() < 0.002,
                "{clip}: {}",
                w.duration()
            );
            assert_eq!(w.hits.len(), windows.len(), "{clip}: hit windows");
            for (h, want) in w.hits.iter().zip(windows) {
                assert_eq!(h.judge, judge, "{clip}");
                assert!(
                    (h.start - want.0).abs() < 0.001 && (h.end - want.1).abs() < 0.001,
                    "{clip}: {h:?} vs {want:?}"
                );
            }
            // The clip's root motion times the leg ratio is what he travels (+-2 %).
            let z = -w.root_motion.last().unwrap()[2];
            let want = travel * e.basis.scale;
            assert!(
                (z - want).abs() < 0.02 * want.max(0.1),
                "{clip}: travel {z} vs {want}"
            );
        }
        let scale = extract_clip(&assassin, &skel, rig, "a00_3003")
            .unwrap()
            .basis
            .scale;
        assert!((1.0..1.3).contains(&scale), "scale {scale}");
    }

    #[test]
    fn install_the_players_reactions_match_the_data_table() {
        let (Ok(dsr_dir), Ok(wow_dir)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("DSR_DIR and WOW_DATA not both set; skipped");
            return;
        };
        let player = dsr::load(Path::new(&dsr_dir)).unwrap();
        let skel = wow::human_male(Path::new(&wow_dir)).unwrap();
        let rig = &bonemap::HUMAN_MALE_RIG;
        // clip, length s, attack / evade / movement first open s, invulnerable until s (0: none).
        let table = [
            ("a00_2000", 0.833, 0.600, 0.600, 0.767, 0.0),
            ("a00_2004", 1.900, 1.033, 1.067, 1.067, 0.0),
            ("a00_2010", 1.967, 1.433, 1.500, 1.500, 0.0),
            ("a00_2016", 4.267, 3.567, 3.467, 3.700, 3.6),
            ("a00_2026", 3.500, 3.100, 3.000, 3.167, 3.1),
        ];
        for (clip, len, chain, evade, mv, down) in table {
            let e = extract_clip(&player, &skel, rig, clip).unwrap();
            let w = &e.r.wow;
            let near = |a: f32, b: f32| (a - b).abs() < 0.04;
            assert!(near(w.duration(), len), "{clip}: {}", w.duration());
            assert!(
                near(first_open(w, wscl::FLAG_CHAIN).unwrap(), chain),
                "{clip} attack"
            );
            assert!(
                near(first_open(w, wscl::FLAG_EVADE).unwrap(), evade),
                "{clip} evade"
            );
            assert!(
                near(first_open(w, wscl::FLAG_MOVE).unwrap(), mv),
                "{clip} move"
            );
            let end = w
                .windows(wscl::FLAG_INVINCIBLE)
                .next()
                .map_or(0.0, |(_, e)| e);
            assert!(near(end, down), "{clip} invulnerable until {end}");
        }
        // The knockdowns travel straight back (2016, 2026) or forward (2017, 2027), 6.4 and 3.2-3.7 yd.
        let travel = |clip: &str| {
            let e = extract_clip(&player, &skel, rig, clip).unwrap();
            e.r.wow.root_motion.last().unwrap()[2]
        };
        assert!((travel("a00_2016") - 6.36).abs() < 0.1);
        assert!((travel("a00_2017") + 6.54).abs() < 0.1);
        assert!((travel("a00_2026") - 3.22).abs() < 0.1);
        assert!((travel("a00_2027") + 3.71).abs() < 0.1);
    }
}
