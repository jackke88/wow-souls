//! `WOWSOULS_FPS=1`: a frame-time line every three seconds (average and worst frame, in
//! milliseconds), to price a lane: run the same scene with `WOWSOULS_GFX=classic` and `high`, or
//! one lane switched off, and compare. Use `WOWSOULS_NOVSYNC=1` so the frame is not capped.

use bevy::prelude::*;

/// Window the line averages over, in seconds.
const WINDOW_S: f32 = 3.0;

#[derive(Default)]
struct Window {
    secs: f32,
    frames: u32,
    worst: f32,
}

pub fn plugin(app: &mut App) {
    if std::env::var("WOWSOULS_FPS").is_ok_and(|v| v != "0") {
        app.add_systems(Last, report);
    }
}

fn report(time: Res<Time<Real>>, mut window: Local<Window>) {
    let dt = time.delta_secs();
    window.secs += dt;
    window.frames += 1;
    window.worst = window.worst.max(dt);
    if window.secs >= WINDOW_S {
        let avg = window.secs / window.frames as f32;
        info!(
            "gfx-perf: {:.2} ms avg ({:.0} fps), {:.1} ms worst over {} frames",
            avg * 1e3,
            1.0 / avg,
            window.worst * 1e3,
            window.frames
        );
        *window = Window::default();
    }
}
