//! The bootstrap: benilla's engine with no game attached, in the order
//! `benilla_world::worldview::run` uses (benilla @ 2e82d34). The `mpq://` source is registered
//! before `DefaultPlugins` builds the `AssetPlugin`, and the loaders after it.

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

use benilla_world::build_id::BuildId;

use crate::{
    actor, audio, avatar, blood, camera, check, combat, controller, freecam, hits, hud, hurt,
    loading, lockon, npc, pad, reaction, record, run_mode, settle, shake, shot, shove, walkers,
    world,
};

/// Window size when `WOWSOULS_WIN=<w>x<h>` does not say otherwise.
const DEFAULT_WINDOW: UVec2 = UVec2::new(1600, 900);

/// Builds and runs the game; `build` is the launcher's compile-time stamp.
pub fn run(build: BuildId) -> AppExit {
    // A dev build finds its install through the `WoW` junction in the project folder; `WOW_DATA`
    // overrides it in either build.
    benilla_formats::set_project_folder(build.project_dir);
    // A player build has no junction: the client folder they picked once (or pick now).
    if !crate::install::choose(run_mode::unattended()) && !run_mode::unattended() {
        crate::install::report_missing();
        return AppExit::error();
    }
    crate::install::animations(run_mode::unattended());
    let mut app = App::new();
    app.insert_resource(build);

    let check = check::configure(&mut app);
    let unattended = run_mode::unattended();
    if unattended {
        app.insert_resource(run_mode::Unattended);
    }

    match benilla_formats::wow_data() {
        Some(data_dir) => {
            if let Err(e) = benilla_assets::register_mpq_source(&mut app, &data_dir) {
                eprintln!("wow-souls: mpq:// source unavailable ({e:#})");
            }
        }
        None => eprintln!(
            "wow-souls: no WoW 1.12.1 install found. Start with --choose-wow, set WOW_DATA to its \
             Data folder, or link it as `WoW` in the project folder. Looked in {:?}",
            benilla_formats::candidates()
        ),
    }

    app.add_plugins(default_plugins(Window {
        title: "WoW Souls".into(),
        resolution: window_size().into(),
        // An unattended run stays out of the way of whoever is at the desk.
        focused: !unattended,
        window_level: if unattended {
            bevy::window::WindowLevel::AlwaysOnBottom
        } else {
            bevy::window::WindowLevel::Normal
        },
        // The player's window: full-screen when they chose it (never in an unattended run), vsync
        // unless they turned it off.
        mode: if crate::settings::initial().video.fullscreen && !unattended {
            bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current)
        } else {
            bevy::window::WindowMode::Windowed
        },
        // `WOWSOULS_NOVSYNC=1`: frames as fast as they come, to measure a frame's cost (the
        // `frames:` line of `WOWSOULS_TRACE` / `WOWSOULS_FRAMES`), as benilla's `WOW_NOVSYNC`.
        present_mode: if std::env::var("WOWSOULS_NOVSYNC").as_deref() == Ok("1")
            || !crate::settings::initial().video.vsync
        {
            bevy::window::PresentMode::AutoNoVsync
        } else {
            bevy::window::PresentMode::default()
        },
        ..default()
    }))
    .add_plugins(benilla_world::thread_qos::ThreadQosPlugin)
    .add_plugins(benilla_world::bgwin::BgWinPlugin)
    .add_plugins(benilla_world::mac_quit::MacQuitPlugin);

    // The whole engine, the same group the benilla client adds.
    app.add_plugins(benilla_world::world_plugins::WorldPlugins);
    // The engine's one stub: a world is always live here, there is no login or loading screen.
    app.insert_resource(benilla_world::schedule::WorldLive(true));
    // The graphics update (the everwood fork's renderer settings): after the engine's plugins,
    // whose resources it sets.
    // The player's settings and key bindings first: the lanes below read what they start with.
    app.add_plugins(crate::settings::plugin);
    app.add_plugins(crate::gfx::plugin)
        // The game's own clock and weather: what a server would send the engine.
        .add_plugins(crate::daycycle::plugin)
        .add_plugins(crate::weather::plugin)
        .add_plugins(crate::cloak::plugin);

    app.add_systems(Startup, benilla_world::build_id::banner)
        // Apart from the tuple: Bevy implements `Plugins` for tuples of up to fifteen.
        .add_plugins(pad::plugin)
        .add_plugins(audio::plugin)
        .add_plugins(crate::loot::plugin)
        .add_plugins(crate::equip::plugin)
        .add_plugins(crate::gear_effect::plugin)
        .add_plugins(crate::fx_census::plugin)
        .add_plugins(crate::decor::plugin)
        .add_plugins(crate::shop::plugin)
        .add_plugins(crate::abilities::plugin)
        .add_plugins(crate::spells::plugin)
        .add_plugins(crate::quests::plugin)
        .add_plugins(crate::talents::plugin)
        .add_plugins(crate::warrior::plugin)
        .add_plugins(crate::level::plugin)
        .add_plugins(crate::foot::plugin)
        .add_plugins(crate::puddle::plugin)
        .add_plugins(crate::locomotion::plugin)
        .add_plugins(crate::boss::plugin)
        .add_plugins(crate::lively::plugin)
        .add_plugins(crate::zone::plugin)
        .add_plugins(crate::vocal::plugin)
        .add_plugins(crate::create::plugin)
        .add_plugins(crate::sysmenu::ui::plugin)
        .add_plugins(crate::cartography::plugin)
        .add_plugins(crate::menu::plugin)
        .add_plugins(crate::bloodstain::plugin)
        .add_plugins(crate::soulbind::plugin)
        .add_plugins(crate::world_state::plugin)
        .add_plugins(crate::tour::plugin)
        .add_plugins(record::plugin)
        .add_plugins((
            shove::plugin,
            shake::plugin,
            blood::plugin,
            reaction::plugin,
            crate::defense::plugin,
            // The valley's spawns from the manifest, or nothing (`npc` then places the old groups).
            world::plugin,
            // Its critters and friendly NPCs walking (nothing without the manifest).
            walkers::plugin,
        ))
        .add_plugins(crate::village::plugin)
        .add_plugins(crate::breath::plugin)
        .add_plugins(crate::torch::plugin)
        .add_plugins((
            freecam::plugin,
            avatar::plugin,
            actor::plugin,
            settle::plugin,
            loading::plugin,
            controller::plugin,
            lockon::plugin,
            combat::plugin,
            hits::plugin,
            npc::plugin,
            hurt::plugin,
            camera::plugin,
            camera::instruments,
            hud::plugin,
            shot::plugin,
        ));

    // After `AssetPlugin`: the loaders go into the live `AssetServer`.
    benilla_assets::register_asset_loaders(&mut app);

    let exit = app.run();
    check::report(check, exit)
}

/// benilla's tuned `DefaultPlugins` for our window, with the gamepad plugin switched back on:
/// benilla disables `GilrsPlugin` ("1.12's bindings are keyboard/mouse"), and with it off no
/// `Gamepad` entity can exist, which left every pad binding in the game dead.
fn default_plugins(window: Window) -> PluginGroupBuilder {
    benilla_world::boot::tuned_default_plugins(window).enable::<bevy::gilrs::GilrsPlugin>()
}

/// `WOWSOULS_WIN=<w>x<h>`, else [`DEFAULT_WINDOW`].
fn window_size() -> UVec2 {
    std::env::var("WOWSOULS_WIN")
        .ok()
        .and_then(|v| parse_window_size(&v))
        .unwrap_or(DEFAULT_WINDOW)
}

fn parse_window_size(v: &str) -> Option<UVec2> {
    let (w, h) = v.trim().split_once('x')?;
    let size = UVec2::new(w.trim().parse().ok()?, h.trim().parse().ok()?);
    (size.x > 0 && size.y > 0).then_some(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_size_parses_width_by_height() {
        assert_eq!(parse_window_size("1280x720"), Some(UVec2::new(1280, 720)));
        assert_eq!(parse_window_size(" 800 x 600 "), Some(UVec2::new(800, 600)));
    }

    #[test]
    fn window_size_rejects_junk_and_zero() {
        assert_eq!(parse_window_size("1280"), None);
        assert_eq!(parse_window_size("ax720"), None);
        assert_eq!(parse_window_size("0x720"), None);
    }

    #[test]
    fn the_gamepad_plugin_is_on() {
        // benilla's group disables it; with it off no `Gamepad` entity can exist and every pad
        // binding in the game is dead.
        assert!(default_plugins(Window::default()).enabled::<bevy::gilrs::GilrsPlugin>());
    }
}
