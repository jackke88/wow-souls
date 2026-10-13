//! wow-souls: a single-player, third-person action prototype that runs on benilla's WoW 1.12.1
//! world engine (`benilla-world`) with no server, no login and no WoW UI. The engine reads the
//! player's own 1.12.1 install, read-only; nothing here touches the retail client or a network.

mod abilities;
mod actor;
mod audio;
mod avatar;
mod blood;
mod bloodstain;
mod boot;
mod camera;
mod character;
mod check;
mod cloak;
mod combat;
mod controller;
mod create;
mod critter;
mod daycycle;
mod defense;
mod dsclip;
mod dsloco;
mod dsview;
mod equip;
mod foehit;
mod foot;
mod freecam;
mod friendly;
mod gear_effect;
mod gfx;
mod hitbody;
mod hits;
mod hud;
mod hurt;
mod level;
mod loading;
mod lockon;
mod loot;
mod menu;
mod mixer;
mod monster;
mod motor;
mod npc;
mod pad;
mod pattern;
mod projectiles;
mod quests;
mod reaction;
mod record;
mod ring;
mod roam;
mod run_mode;
mod settings;
mod settle;
mod sfx;
mod shake;
mod shop;
mod shot;
mod shove;
mod slice;
mod species;
mod stream;
mod sysmenu;
mod vocal;
mod walkers;
mod weapons;
mod weather;
mod world;
mod world_state;
mod zone;

use benilla_world::build_id::BuildId;

fn main() -> benilla_world::AppExit {
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
