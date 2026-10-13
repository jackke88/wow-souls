//! `wow-souls-world`: import Northshire Valley from the exported vMaNGOS tables and the WoW client's
//! terrain data into `cache/world/northshire.txt`.
//!
//! ```text
//! wow-souls-world import  [--tsv <dir>] [--wow <Data dir>] [--out <file>] [--svg]
//! wow-souls-world areas   [--tsv <dir>] [--wow <Data dir>]
//! wow-souls-world summary [--out <file>]
//! ```
//!
//! Defaults: `cache/mangos/tsv` (written by `tools/export-mangos.ps1`), the `WOW_DATA` environment
//! variable or `C:\Spel\World of Warcraft 1.12\Data`, and `cache/world/northshire.txt` (`import` writes
//! it, so pass `--out cache/world/northshire.next.txt` to write the re-import beside the live file;
//! `summary` reads `wow_world::default_manifest_path()`: the `.next` file when it exists).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use wow_world::db::Db;
use wow_world::model::World;
use wow_world::select::{select, spawns_without_path};
use wow_world::valley::Valley;
use wow_world::{manifest, valley};

const DEFAULT_WOW: &str = r"C:\Spel\World of Warcraft 1.12\Data";
/// The terrain tiles to read: the valley and a good margin of Elwynn around it (x north, y west).
const BOUNDS: ([f32; 2], [f32; 2]) = ([-9600.0, -600.0], [-8500.0, 200.0]);

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct Args {
    tsv: PathBuf,
    wow: PathBuf,
    /// `--out`; `import` writes `cache/world/northshire.txt` without it, `summary` reads
    /// `wow_world::default_manifest_path()`.
    out: Option<PathBuf>,
    svg: bool,
}

impl Args {
    fn import_out(&self) -> PathBuf {
        self.out
            .clone()
            .unwrap_or_else(|| repo().join("cache/world/northshire.txt"))
    }

    fn summary_in(&self) -> PathBuf {
        self.out
            .clone()
            .unwrap_or_else(wow_world::default_manifest_path)
    }
}

fn parse_args(rest: &[String]) -> Result<Args> {
    let mut a = Args {
        tsv: repo().join("cache/mangos/tsv"),
        wow: PathBuf::from(std::env::var("WOW_DATA").unwrap_or_else(|_| DEFAULT_WOW.to_string())),
        out: None,
        svg: false,
    };
    let mut it = rest.iter();
    while let Some(flag) = it.next() {
        let mut value = |name: &str| -> Result<PathBuf> {
            Ok(PathBuf::from(
                it.next().with_context(|| format!("{name} needs a value"))?,
            ))
        };
        match flag.as_str() {
            "--tsv" => a.tsv = value("--tsv")?,
            "--wow" => a.wow = value("--wow")?,
            "--out" => a.out = Some(value("--out")?),
            "--svg" => a.svg = true,
            other => bail!("unknown option {other}"),
        }
    }
    Ok(a)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((cmd, rest)) = args.split_first() else {
        bail!("usage: wow-souls-world import|areas|summary [options] (see the file header)");
    };
    let a = parse_args(rest)?;
    match cmd.as_str() {
        "import" => import(&a),
        "areas" => areas(&a),
        "summary" => summary(&a),
        other => bail!("unknown command {other}"),
    }
}

fn import(a: &Args) -> Result<()> {
    let out = a.import_out();
    let db = Db::load(&a.tsv)?;
    let v = Valley::load(&a.wow, BOUNDS)?;
    let world = select(&db, &v);
    if world.spawns.is_empty() || world.quests.is_empty() {
        bail!(
            "the selection is empty ({} spawns, {} quests): refusing to write {}",
            world.spawns.len(),
            world.quests.len(),
            out.display()
        );
    }
    check_paths(&world)?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&out, manifest::write(&world))?;
    println!("wrote {}", out.display());
    print_summary(&world);
    if a.svg {
        let svg = out.with_file_name("northshire-spawns.svg");
        std::fs::write(&svg, draw(&world, &v))?;
        println!("wrote {}", svg.display());
    }
    Ok(())
}

fn summary(a: &Args) -> Result<()> {
    let path = a.summary_in();
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    println!("manifest {}", path.display());
    print_summary(&manifest::parse(&text)?);
    Ok(())
}

/// A creature that walks a path (movement type 2) with no waypoints would stand still: refuse.
fn check_paths(w: &World) -> Result<()> {
    let bare = spawns_without_path(w);
    if bare.is_empty() {
        return Ok(());
    }
    bail!(
        "{} selected spawn(s) have movement type 2 but no rows in creature_movement (guids {:?}): refusing to write the manifest",
        bare.len(),
        &bare[..bare.len().min(20)]
    )
}

/// `waypoint paths: <spawns with a path> spawns, <points> points`.
fn paths_line(w: &World) -> String {
    let walkers: BTreeSet<u32> = w.waypoints.iter().map(|p| p.guid).collect();
    format!(
        "waypoint paths: {} spawns, {} points",
        walkers.len(),
        w.waypoints.len()
    )
}

/// `quest data: <start scripts> start / <end scripts> end script rows, <xp rows> xp rows (level 2 at <xp>), ...`.
fn quest_data_line(w: &World) -> String {
    format!(
        "quest data: {} start / {} end script rows, {} xp rows (first {:?}), {} item extras ({} start a quest), {} quests pay reputation",
        w.scripts.iter().filter(|s| s.start).count(),
        w.scripts.iter().filter(|s| !s.start).count(),
        w.xp_for_level.len(),
        &w.xp_for_level[..w.xp_for_level.len().min(3)],
        w.item_extras.len(),
        w.item_extras.iter().filter(|i| i.start_quest != 0).count(),
        w.quests.iter().filter(|q| !q.reward_rep.is_empty()).count(),
    )
}

fn print_summary(w: &World) {
    println!(
        "{} spawns, {} creature templates, {} quests, {} start / {} end relations, {} items, {} objects ({} spawns), {} loot rows, {} vendor rows, {} equipment rows",
        w.spawns.len(),
        w.creatures.len(),
        w.quests.len(),
        w.starts.len(),
        w.ends.len(),
        w.items.len(),
        w.objects.len(),
        w.object_spawns.len(),
        w.loot.len(),
        w.vendors.len(),
        w.equipment.len()
    );
    println!("{}", paths_line(w));
    println!("{}", quest_data_line(w));
    println!("areas used:");
    for (id, name) in &w.areas {
        let n = w.spawns.iter().filter(|s| s.area == *id).count();
        println!("  {id:>5} {name} ({n} creature spawns)");
    }
    let names: BTreeMap<u32, &str> = w
        .creatures
        .iter()
        .map(|c| (c.entry, c.name.as_str()))
        .collect();
    let mut per_entry: BTreeMap<u32, usize> = BTreeMap::new();
    for s in &w.spawns {
        *per_entry.entry(s.entry).or_default() += 1;
    }
    println!("creature spawns by entry:");
    let mut rows: Vec<_> = per_entry.into_iter().collect();
    rows.sort_by_key(|(entry, n)| (std::cmp::Reverse(*n), *entry));
    for (entry, n) in rows {
        let c = w.creatures.iter().find(|c| c.entry == entry);
        println!(
            "  {n:>4} x {entry:>6} {} (level {}-{}, flags {:#x}, type {})",
            names.get(&entry).copied().unwrap_or("?"),
            c.map_or(0, |c| c.level_min),
            c.map_or(0, |c| c.level_max),
            c.map_or(0, |c| c.npc_flags),
            c.map_or(0, |c| c.kind),
        );
    }
    println!("quests:");
    for q in &w.quests {
        let givers: Vec<String> = w
            .starts
            .iter()
            .filter(|r| r.quest == q.id)
            .map(|r| {
                if r.object {
                    format!("object {}", r.npc)
                } else {
                    names.get(&r.npc).copied().unwrap_or("?").to_string()
                }
            })
            .collect();
        println!(
            "  {:>5} L{:<2} xp {:>4} {}  <- {}",
            q.id,
            q.level,
            q.reward_xp,
            q.title,
            if givers.is_empty() {
                "(no giver)".to_string()
            } else {
                givers.join(", ")
            }
        );
    }
    println!("objects:");
    for o in &w.objects {
        let n = w
            .object_spawns
            .iter()
            .filter(|s| s.entry == o.entry)
            .count();
        println!("  {n:>4} x {:>6} {} (type {})", o.entry, o.name, o.kind);
    }
}

/// Every area under the creature and object spawns inside `BOUNDS`, with its parent and whether it
/// counts as the valley: the list to read before trusting the selection.
fn areas(a: &Args) -> Result<()> {
    let db = Db::load(&a.tsv)?;
    let v = Valley::load(&a.wow, BOUNDS)?;
    let inside = |p: &[f32; 3]| {
        (BOUNDS.0[0]..=BOUNDS.1[0]).contains(&p[0]) && (BOUNDS.0[1]..=BOUNDS.1[1]).contains(&p[1])
    };
    let mut count: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    for s in db.spawns.iter().filter(|s| !s.event && inside(&s.pos)) {
        count.entry(v.area_of(s.pos[0], s.pos[1])).or_default().0 += 1;
    }
    for s in db
        .object_spawns
        .iter()
        .filter(|s| !s.event && inside(&s.pos))
    {
        count.entry(v.area_of(s.pos[0], s.pos[1])).or_default().1 += 1;
    }
    println!("area id  name  (parent)  creature spawns / object spawns  valley?");
    for (id, (c, o)) in count {
        let (p, pn) = v.parent(id);
        println!(
            "{id:>7}  {}  ({p} {pn})  {c} / {o}  {}",
            v.name(id),
            if v.is_valley_area(id) { "VALLEY" } else { "-" }
        );
    }
    // What stands in each area: the top entries by count and the spawn extent (x north, y west).
    let name_of: BTreeMap<u32, &str> = db
        .creatures
        .iter()
        .map(|c| (c.entry, c.name.as_str()))
        .collect();
    let mut by_area: BTreeMap<u32, BTreeMap<u32, usize>> = BTreeMap::new();
    let mut extent: BTreeMap<u32, [f32; 4]> = BTreeMap::new();
    for s in db.spawns.iter().filter(|s| !s.event && inside(&s.pos)) {
        let area = v.area_of(s.pos[0], s.pos[1]);
        *by_area.entry(area).or_default().entry(s.entry).or_default() += 1;
        let e = extent
            .entry(area)
            .or_insert([f32::MAX, f32::MIN, f32::MAX, f32::MIN]);
        e[0] = e[0].min(s.pos[0]);
        e[1] = e[1].max(s.pos[0]);
        e[2] = e[2].min(s.pos[1]);
        e[3] = e[3].max(s.pos[1]);
    }
    for (area, entries) in by_area {
        let mut top: Vec<_> = entries.into_iter().collect();
        top.sort_by_key(|(entry, n)| (std::cmp::Reverse(*n), *entry));
        let e = extent[&area];
        println!(
            "area {area} {}: x {:.0}..{:.0} y {:.0}..{:.0}: {}",
            v.name(area),
            e[0],
            e[1],
            e[2],
            e[3],
            top.iter()
                .take(8)
                .map(|(entry, n)| format!("{n} {}", name_of.get(entry).copied().unwrap_or("?")))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!("(valley root: area {})", valley::NORTHSHIRE_VALLEY);
    Ok(())
}

/// A top-down picture of the selection: the valley's terrain chunks as grey squares, spawns as
/// dots (red hostile, green critter, blue NPC with flags, yellow objects), north up, west left.
fn draw(w: &World, v: &Valley) -> String {
    let cells: Vec<_> = v
        .cells()
        .into_iter()
        .filter(|(_, a)| v.is_valley_area(*a))
        .collect();
    let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    let mut grow = |x: f32, y: f32| {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    };
    for (p, _) in &cells {
        grow(p[0], p[1]);
    }
    for s in &w.spawns {
        grow(s.pos[0], s.pos[1]);
    }
    // 1.2 px per yard; east (= -y) to the right, north (= +x) up.
    let k = 1.2_f32;
    let width = (y1 - y0) * k + 40.0;
    let height = (x1 - x0) * k + 40.0;
    let px = |y: f32| (y1 - y) * k + 20.0;
    let py = |x: f32| (x1 - x) * k + 20.0;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\"><rect width=\"100%\" height=\"100%\" fill=\"#fff\"/>\n"
    );
    let cell = 33.333 * k;
    for (p, a) in &cells {
        let shade = if *a == valley::NORTHSHIRE_VALLEY {
            "#ddd"
        } else {
            "#c9d6e3"
        };
        out += &format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{cell:.1}\" height=\"{cell:.1}\" fill=\"{shade}\"/>\n",
            px(p[1]) - cell / 2.0,
            py(p[0]) - cell / 2.0
        );
    }
    let kind_of: BTreeMap<u32, (u32, u32)> = w
        .creatures
        .iter()
        .map(|c| (c.entry, (c.kind, c.npc_flags)))
        .collect();
    for s in &w.spawns {
        let (kind, flags) = kind_of.get(&s.entry).copied().unwrap_or((0, 0));
        let colour = if kind == 8 {
            "#2a9d3a"
        } else if flags != 0 {
            "#1f5fd0"
        } else {
            "#d02020"
        };
        out += &format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.2\" fill=\"{colour}\"/>\n",
            px(s.pos[1]),
            py(s.pos[0])
        );
    }
    for o in &w.object_spawns {
        out += &format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"1.4\" fill=\"#e0a000\"/>\n",
            px(o.pos[1]),
            py(o.pos[0])
        );
    }
    // The slice anchor: the Abbey's doors.
    out += &format!(
        "<text x=\"{:.0}\" y=\"{:.0}\" font-size=\"10\" fill=\"#000\">Abbey doors</text>\n",
        px(-136.0) + 4.0,
        py(-8913.0)
    );
    out += "</svg>\n";
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::model::{ItemExtra, Spawn, Waypoint};

    fn spawn(guid: u32, movement: u8) -> Spawn {
        Spawn {
            guid,
            entry: 38,
            alt: [0; 4],
            pos: [-8900.0, -100.0, 70.0],
            orientation: 0.0,
            respawn_min: 120,
            respawn_max: 120,
            wander: 0.0,
            movement,
            area: 9,
            event: false,
        }
    }

    fn point(guid: u32, point: u32) -> Waypoint {
        Waypoint {
            guid,
            point,
            pos: [0.0; 3],
            orientation: 0.0,
            wait_ms: 0,
            script_id: 0,
            path_id: 0,
        }
    }

    #[test]
    fn the_summary_line_counts_spawns_with_a_path_and_their_points() {
        let w = World {
            spawns: vec![spawn(1, 2), spawn(2, 2), spawn(3, 0)],
            waypoints: vec![point(1, 1), point(1, 2), point(1, 3), point(2, 1)],
            ..World::default()
        };
        assert_eq!(paths_line(&w), "waypoint paths: 2 spawns, 4 points");
        assert_eq!(
            paths_line(&World::default()),
            "waypoint paths: 0 spawns, 0 points"
        );
    }

    #[test]
    fn the_quest_data_line_counts_scripts_xp_rows_and_item_extras() {
        let w = World {
            xp_for_level: vec![400, 900, 1400, 2100],
            item_extras: vec![
                ItemExtra {
                    entry: 14646,
                    start_quest: 5805,
                    bonding: 4,
                },
                ItemExtra {
                    entry: 1,
                    start_quest: 0,
                    bonding: 0,
                },
            ],
            ..World::default()
        };
        assert_eq!(
            quest_data_line(&w),
            "quest data: 0 start / 0 end script rows, 4 xp rows (first [400, 900, 1400]), 2 item extras (1 start a quest), 0 quests pay reputation"
        );
    }

    #[test]
    fn import_writes_the_live_file_by_default_and_summary_reads_the_default_path() {
        let a = parse_args(&[]).unwrap();
        assert!(a.import_out().ends_with("cache/world/northshire.txt"));
        assert_eq!(a.summary_in(), wow_world::default_manifest_path());
        let b = parse_args(&["--out".into(), "x/y.txt".into()]).unwrap();
        assert_eq!(b.import_out(), PathBuf::from("x/y.txt"));
        assert_eq!(b.summary_in(), PathBuf::from("x/y.txt"));
    }

    #[test]
    fn a_walker_without_waypoints_stops_the_import() {
        let ok = World {
            spawns: vec![spawn(1, 2), spawn(2, 1), spawn(3, 0)],
            waypoints: vec![point(1, 1)],
            ..World::default()
        };
        assert!(check_paths(&ok).is_ok());

        let bad = World {
            spawns: vec![spawn(1, 2), spawn(7, 2), spawn(9, 2)],
            waypoints: vec![point(1, 1)],
            ..World::default()
        };
        let e = check_paths(&bad).unwrap_err().to_string();
        assert!(e.contains("movement type 2") && e.contains("[7, 9]"), "{e}");
    }
}
