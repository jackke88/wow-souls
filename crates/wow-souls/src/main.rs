//! wow-souls: a single-player, third-person action prototype that runs on benilla's WoW 1.12.1
//! world engine (`benilla-world`) with no server, no login and no WoW UI. The engine reads the
//! player's own 1.12.1 install, read-only; nothing here touches the retail client or a network.

mod abilities;
mod actor;
mod animation_library;
mod audio;
mod avatar;
mod behavior;
mod blood;
mod bloodstain;
mod boar_rush;
mod boot;
mod boss;
mod breath;
mod camera;
mod cartography;
mod character;
mod check;
mod cloak;
mod combat;
mod controller;
mod create;
mod creature_motion;
mod critter;
mod daycycle;
mod decor;
mod defense;
mod equip;
mod foebolt;
mod foehit;
mod foot;
mod footwork;
mod freecam;
mod friendly;
mod fx_census;
mod gear_effect;
mod gfx;
mod glide;
mod hitbody;
mod hits;
mod hud;
mod hurt;
mod install;
mod latch;
mod legfloor;
mod level;
mod lively;
mod loading;
mod lockon;
mod locomotion;
mod loot;
mod menu;
mod mixer;
mod monster;
mod motion_clip;
mod motor;
mod named;
mod npc;
mod pad;
mod pattern;
mod projectiles;
mod puddle;
mod quests;
mod reaction;
mod record;
mod rig_view;
mod ring;
mod roam;
mod run_mode;
mod saves;
mod settings;
mod settle;
mod sfx;
mod shake;
mod shock;
mod shop;
mod shot;
mod shove;
mod slice;
mod slope;
mod soulbind;
mod species;
mod spells;
mod stream;
mod sysmenu;
mod talents;
mod torch;
mod trial;
mod village;
mod vocal;
mod walkers;
mod warrior;
mod weapons;
mod weather;
mod world;
mod world_audit;
mod world_state;
mod zone;

use benilla_world::build_id::BuildId;

fn main() -> benilla_world::AppExit {
    if std::env::args().any(|a| a == "--audit-westfall") {
        match world_audit::run() {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("Westfall audit: {e:#}");
                std::process::exit(1);
            }
        }
    }
    // `--trial-character <level> [slot]` writes a save slot and exits (no window, no engine).
    if let Some(code) = trial::cli(std::env::args().skip(1).collect()) {
        std::process::exit(code);
    }
    boot::run(BuildId {
        version: env!("CARGO_PKG_VERSION"),
        describe: env!("BENILLA_GIT_DESCRIBE"),
        sha: env!("BENILLA_GIT_SHA"),
        short: env!("BENILLA_GIT_SHORT"),
        date: env!("BENILLA_GIT_DATE"),
        profile: env!("BENILLA_PROFILE"),
        project_dir: env!("BENILLA_PROJECT_DIR"),
        ..Default::default()
    })
}
