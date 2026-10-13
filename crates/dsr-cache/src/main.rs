//! `wow-souls-dsr`: reads the Dark Souls Remastered install (read-only), retargets the player's
//! roll onto the WoW human, writes `cache/dsr/`.
//!
//! ```text
//! wow-souls-dsr list    [--dsr <dir>]
//! wow-souls-dsr extract [--dsr <dir>] [--wow <Data dir>] [--out <dir>] [--roll <a00_NNNN>]
//! wow-souls-dsr locomotion [--dsr <dir>] [--wow <Data dir>] [--out <dir>]   (the gait of every weapon set: <out>/locomotion.txt)
//! wow-souls-dsr loco    [--dsr <dir>] [--set a00] [--clips a00_0500,...]   (the player's locomotion clips, measured; read-only)
//! wow-souls-dsr hud     [--dsr <dir>] [--out <dir>]      (the HUD's art and layout: <out>/hud)
//! wow-souls-dsr survey  --chr <cNNNN> [--npc <NpcParam id>] [--dsr <dir>]   (read-only listing)
//! wow-souls-dsr probe   --kit <name> --clips <a00_NNNN,...> [--dsr <dir>] [--wow <Data dir>]   (retarget numbers, no gates)
//! ```

mod basis;
mod bonemap;
mod cache;
mod dds;
mod dsr;
mod hud;
mod loco;
mod monster;
mod moveset;
mod params;
mod probe;
mod reacts;
mod retarget;
mod roll;
mod survey;
mod wow;

use std::path::PathBuf;

use anyhow::{bail, Result};

const DEFAULT_DSR: &str = "C:/Spel/Dark Souls Remastered";
const DEFAULT_WOW: &str = "C:/Spel/World of Warcraft 1.12/Data";
const DEFAULT_OUT: &str = "cache/dsr";

struct Args {
    command: String,
    dsr: PathBuf,
    wow: PathBuf,
    out: PathBuf,
    roll: Option<String>,
    chr: Option<String>,
    npc: Option<i32>,
    kit: Option<String>,
    clips: Vec<String>,
    set: Option<String>,
}

fn args() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let command = it.next().unwrap_or_default();
    let mut a = Args {
        command,
        dsr: DEFAULT_DSR.into(),
        wow: DEFAULT_WOW.into(),
        out: DEFAULT_OUT.into(),
        roll: None,
        chr: None,
        npc: None,
        kit: None,
        clips: Vec::new(),
        set: None,
    };
    while let Some(flag) = it.next() {
        let Some(value) = it.next() else {
            bail!("{flag} needs a value");
        };
        match flag.as_str() {
            "--dsr" => a.dsr = value.into(),
            "--wow" => a.wow = value.into(),
            "--out" => a.out = value.into(),
            "--roll" => a.roll = Some(value),
            "--chr" => a.chr = Some(value),
            "--npc" => a.npc = Some(value.parse()?),
            "--kit" => a.kit = Some(value),
            "--clips" => a.clips = value.split(',').map(str::to_string).collect(),
            "--set" => a.set = Some(value),
            other => bail!("unknown option {other}"),
        }
    }
    Ok(a)
}

fn main() -> Result<()> {
    let a = args()?;
    match a.command.as_str() {
        "list" => roll::list(&dsr::load(&a.dsr)?),
        "loco" => loco::run(&a.dsr, a.set.as_deref(), &a.clips),
        "weapons-survey" => moveset::survey(&a.dsr),
        "weapons" => cache::extract_weapons(&a.dsr, &a.wow, &a.out),
        "locomotion" => cache::extract_locomotion(&a.dsr, &a.wow, &a.out),
        "extract" => cache::extract(&a.dsr, &a.wow, &a.out, a.roll.as_deref()),
        "defense" => cache::extract_defense(&a.dsr, &a.wow, &a.out),
        "loot" => cache::extract_loot(&a.dsr, &a.wow, &a.out),
        "hud" => hud::run(&a.dsr, &a.out).map(|_| ()),
        "survey" => match &a.chr {
            Some(chr) => survey::run(&a.dsr, chr, a.npc),
            None => bail!("survey needs --chr cNNNN"),
        },
        "probe" => match &a.kit {
            Some(kit) => probe::run(&a.dsr, &a.wow, kit, &a.clips),
            None => bail!("probe needs --kit <name> --clips <a00_NNNN,...>"),
        },
        _ => bail!("usage: wow-souls-dsr list|extract|defense|loot|weapons|locomotion|loco|hud|survey|probe [--dsr DIR] [--wow DATA_DIR] [--out DIR] [--roll a00_NNNN] [--chr cNNNN] [--npc ID]"),
    }
}
