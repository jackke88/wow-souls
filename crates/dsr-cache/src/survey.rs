//! `wow-souls-dsr survey --chr cNNNN [--npc ID]`: what one Dark Souls creature is made of, for
//! choosing which one a game creature wears. Read-only: prints the skeleton's bones, the
//! `NpcParam` row and its think distances when an id is given, and for every animation file its
//! length, root travel, TAE event kinds, hit windows with their attack rows, and the parameters
//! of bullet events (kind 2), which the casters fire.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::{dsr, params};

/// TAE event kind 2, `InvokeBulletBehavior`: a creature fires a projectile.
const BULLET: i32 = 2;

pub fn run(dsr_dir: &Path, chr: &str, npc: Option<i32>) -> Result<()> {
    if std::env::var_os("WOWSOULS_NAMES").is_some() {
        for (id, name) in crate::names::npc_names(dsr_dir)? {
            println!("{id}	{name}");
        }
        return Ok(());
    }
    let c = dsr::load_chr(dsr_dir, chr)?;
    let sk = &c.skeleton;
    println!(
        "{chr}: {} bones, {} animation files",
        sk.parents.len(),
        c.hkx.len()
    );
    // Each bone with its parent and its bind position in model space (DS metres, Y up).
    let bind = crate::retarget::model_space(&sk.parents, &sk.reference)?;
    let bones: Vec<String> = sk
        .names
        .iter()
        .zip(&sk.parents)
        .zip(&bind)
        .enumerate()
        .map(|(i, ((n, p), (at, _)))| {
            format!("{i}:{n}<{p} ({:+.2},{:+.2},{:+.2})", at.x, at.y, at.z)
        })
        .collect();
    for row in bones.chunks(3) {
        println!("  {}", row.join("  "));
    }
    let params = params::load(dsr_dir)?;
    let mut variation = None;
    if let Some(id) = npc {
        let n = params.npc(id)?;
        variation = Some(n.variation);
        println!(
            "NpcParam {id}: hp {} poise {} variation {} think {} souls {}",
            n.hp, n.poise, n.variation, n.think, n.get_soul
        );
        // The game's own name for it: `NPC_name_.fmg` is keyed by the chr number (c2250 is 2250);
        // `NpcParam.nameId` is 0 for every creature of the install.
        if let Some(num) = chr.strip_prefix('c').and_then(|n| n.parse::<i64>().ok()) {
            match crate::names::npc_names(dsr_dir) {
                Ok(names) => println!(
                    "  game name: {}",
                    names
                        .get(&num)
                        .map_or("(not in the name table)", String::as_str)
                ),
                Err(e) => println!("  game name: names unreadable: {e:#}"),
            }
        }
        match i32::try_from(n.think).ok().map(|t| params.think(t)) {
            Some(Ok(t)) => println!(
                "  think distances (DS m): near {:.1} mid {:.1} far {:.1} out {:.1}",
                t.near, t.mid, t.far, t.out
            ),
            _ => println!("  think {} unreadable", n.think),
        }
    }
    let mut stems: Vec<&String> = c.hkx.keys().collect();
    stems.sort();
    for stem in stems {
        let clip = match dsr::decode(&c, stem) {
            Ok(clip) => clip,
            Err(e) => {
                println!("{stem}: cannot decode: {e:#}");
                continue;
            }
        };
        let secs = clip.frames.len() as f32 / clip.fps;
        let last = clip.root.last().copied().unwrap_or([0.0; 4]);
        let events = c.events(stem);
        let mut kinds: BTreeMap<i32, usize> = BTreeMap::new();
        for e in events {
            *kinds.entry(e.kind).or_default() += 1;
        }
        let kinds: Vec<String> = kinds.iter().map(|(k, n)| format!("{k}x{n}")).collect();
        println!(
            "{stem}: {}f {secs:.2}s root x{:+.2} y{:+.2} z{:+.2} yaw{:+.2}  events {}",
            clip.frames.len(),
            last[0],
            last[1],
            last[2],
            last[3],
            kinds.join(" ")
        );
        for h in c.hits(stem) {
            let attack = variation.map(|v| params.npc_attack(v, h.judge));
            match attack {
                Some(Ok(a)) => println!(
                    "    hit judge {} {:.2}-{:.2}s  phys {} poise {} level {} radius {:.2}/{:.2} stop {:.2} knock {:.2}",
                    h.judge, h.start, h.end, a.phys, a.poise, a.level, a.radius, a.radius2, a.stop, a.knock
                ),
                Some(Err(e)) => println!(
                    "    hit judge {} {:.2}-{:.2}s  ({e:#})",
                    h.judge, h.start, h.end
                ),
                None => println!("    hit judge {} {:.2}-{:.2}s", h.judge, h.start, h.end),
            }
        }
        for e in events.iter().filter(|e| e.kind == BULLET) {
            let p: Vec<String> = (0..4)
                .filter_map(|i| e.param_i32(i).map(|v| v.to_string()))
                .collect();
            println!(
                "    bullet event {:.2}-{:.2}s params [{}]",
                e.start,
                e.end,
                p.join(" ")
            );
            // The bullet's behavior judge is its third parameter.
            let (Some(v), Some(judge)) = (variation, e.param_i32(2)) else {
                continue;
            };
            match params.npc_bullet(v, judge) {
                Ok(b) => {
                    let r = b.row;
                    print!(
                        "      bullet {} hits as {} radius {:.2} max {:.2} speed {:.1} life {:.2}",
                        b.bullet, b.hit_bullet, r.radius, r.radius_max, r.max_speed, r.life
                    );
                    match b.attack {
                        Some(a) => println!(
                            "  phys {} magic {} poise {} level {} radius {:.2} knock {:.2}",
                            a.phys,
                            params
                                .npc_atk_magic(r.atk_id as i32)
                                .map_or("?".into(), |m| m.to_string()),
                            a.poise,
                            a.level,
                            a.radius,
                            a.knock
                        ),
                        None => println!("  (no attack row)"),
                    }
                }
                Err(e) => println!("      ({e:#})"),
            }
        }
    }
    Ok(())
}
