//! `WOWSOULS_RECORD=<file.mp4>,<secs>[,<fps>]`: records the game as a video, for trailers and
//! feature reels. The game runs on a fixed 60 Hz clock, whatever the machine manages in real
//! time, so a recording is smooth and repeatable: scripted presses (`WOWSOULS_ATTACK` and the
//! like) happen at the same moment of every take. The clock holds at zero while the world streams
//! in and for [`HOLD`] after the body is released (so the models finish building), then runs for
//! [`WARMUP_TICKS`] steps unrecorded (the camera settles), and the next `secs` seconds are the
//! video: each frame of the primary window is read back and piped, raw, into an `ffmpeg` child
//! that encodes the file. Script times count from the start of the clock, so a clip starts at
//! script time 1.0 s. The recorder stalls the clock while the encoder is behind, so no frame is
//! dropped. The dev readout and the help line stay hidden. `WOWSOULS_WIN=<w>x<h>` sets the
//! picture size (1920x1080 for landscape, 1080x1920 for a vertical video); `WOWSOULS_FFMPEG`
//! names the ffmpeg binary (default: `ffmpeg` on the path).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::time::TimeUpdateStrategy;

use crate::settle::Phase;

/// One step of the recording clock: 60 Hz.
const STEP: Duration = Duration::from_nanos(16_666_667);
/// Simulation steps per second of game time.
const STEPS_PER_SEC: u32 = 60;
/// Wall-clock seconds the clock holds at zero after the body is released: the kobold, the sword
/// and the rest finish building while no game time passes.
const HOLD: Duration = Duration::from_secs(3);
/// Unrecorded steps before the first frame: the camera settles into its seat.
const WARMUP_TICKS: u32 = 60;
/// Screenshots in flight at most: each is a full frame in memory.
const MAX_BACKLOG: u64 = 3;
/// A world that is not playable by now (wall-clock seconds) is a failed run.
const GIVE_UP: Duration = Duration::from_secs(180);
/// The frame rates the 60 Hz clock divides into.
const FRAME_RATES: [u32; 4] = [15, 20, 30, 60];

/// What `WOWSOULS_RECORD` asks for.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub path: PathBuf,
    pub secs: f32,
    pub fps: u32,
}

impl Config {
    /// `<file>,<secs>[,<fps>]`: seconds up to ten minutes, a frame rate from [`FRAME_RATES`]
    /// (default 30). The file name has no comma.
    pub fn parse(v: &str) -> Option<Config> {
        let parts: Vec<&str> = v.split(',').map(str::trim).collect();
        let (path, secs, fps) = match parts[..] {
            [path, secs] => (path, secs, "30"),
            [path, secs, fps] => (path, secs, fps),
            _ => return None,
        };
        let secs: f32 = secs.parse().ok().filter(|s: &f32| s.is_finite())?;
        let fps: u32 = fps.parse().ok()?;
        ((0.0..=600.0).contains(&secs)
            && secs > 0.0
            && FRAME_RATES.contains(&fps)
            && !path.is_empty())
        .then(|| Config {
            path: PathBuf::from(path),
            secs,
            fps,
        })
    }

    /// Frames in the video.
    pub fn frames(&self) -> u64 {
        (self.secs * self.fps as f32).round().max(1.0) as u64
    }

    /// Clock steps per video frame.
    pub fn stride(&self) -> u32 {
        STEPS_PER_SEC / self.fps
    }
}

/// Whether this run is a recording (the HUD hides its developer lines).
pub fn active() -> bool {
    std::env::var_os("WOWSOULS_RECORD").is_some()
}

/// Where the recording is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The world is streaming in; the clock is held.
    Waiting,
    /// The body is released; the clock is held while the models build.
    Holding,
    /// The clock runs, nothing is recorded yet: this many steps left.
    Warming(u32),
    /// Every `stride`th step is a frame.
    Capturing,
    /// All frames are asked for; waiting for the last to reach the encoder.
    Draining,
    Finished,
}

/// The recording's pure state machine: what each frame of the app does.
#[derive(Debug)]
pub struct Recording {
    stage: Stage,
    total: u64,
    stride: u32,
    /// Steps taken while capturing.
    ticks: u64,
    /// Frames asked for so far.
    requested: u64,
}

/// What the machine reads each frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Seen {
    /// The game is in its playing phase.
    pub playing: bool,
    /// The clock moved this frame.
    pub advanced: bool,
    /// The hold after the release is over.
    pub held: bool,
    /// Frames asked for and not yet written.
    pub backlog: u64,
    /// Every frame asked for has been written.
    pub drained: bool,
}

/// What the machine decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Act {
    /// Ask for a frame of this one.
    pub capture: bool,
    /// Let the clock run for the next frame.
    pub clock: bool,
    /// The recording is over.
    pub finished: bool,
}

impl Recording {
    pub fn new(total: u64, stride: u32) -> Self {
        Self {
            stage: Stage::Waiting,
            total,
            stride: stride.max(1),
            ticks: 0,
            requested: 0,
        }
    }

    pub fn requested(&self) -> u64 {
        self.requested
    }

    pub fn step(&mut self, seen: Seen) -> Act {
        let mut act = Act {
            capture: false,
            clock: false,
            finished: false,
        };
        match self.stage {
            Stage::Waiting => {
                if seen.playing {
                    self.stage = Stage::Holding;
                }
            }
            Stage::Holding => {
                if seen.held {
                    self.stage = Stage::Warming(WARMUP_TICKS);
                    act.clock = true;
                }
            }
            Stage::Warming(left) => {
                let left = left.saturating_sub(u32::from(seen.advanced));
                self.stage = if left == 0 {
                    Stage::Capturing
                } else {
                    Stage::Warming(left)
                };
                act.clock = true;
            }
            Stage::Capturing => {
                if seen.advanced {
                    if self.ticks.is_multiple_of(u64::from(self.stride)) {
                        act.capture = true;
                        self.requested += 1;
                    }
                    self.ticks += 1;
                }
                if self.requested >= self.total {
                    self.stage = Stage::Draining;
                } else {
                    // Behind the encoder: hold the clock, so no frame is dropped.
                    act.clock = seen.backlog + u64::from(act.capture) < MAX_BACKLOG;
                }
            }
            Stage::Draining => {
                if seen.drained {
                    self.stage = Stage::Finished;
                }
            }
            Stage::Finished => {}
        }
        act.finished = self.stage == Stage::Finished;
        act
    }
}

/// The ffmpeg pixel format for a captured frame's texture format.
fn pixel_format(format: TextureFormat) -> Option<&'static str> {
    match format {
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => Some("bgra"),
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => Some("rgba"),
        _ => None,
    }
}

/// ffmpeg's arguments: raw frames in on stdin, H.264 out, tagged BT.709 so players do not guess.
fn ffmpeg_args(size: UVec2, pix_fmt: &str, fps: u32, out: &Path) -> Vec<String> {
    let s = |v: &str| v.to_string();
    vec![
        s("-hide_banner"),
        s("-loglevel"),
        s("error"),
        s("-y"),
        s("-f"),
        s("rawvideo"),
        s("-pix_fmt"),
        s(pix_fmt),
        s("-video_size"),
        format!("{}x{}", size.x, size.y),
        s("-framerate"),
        fps.to_string(),
        s("-i"),
        s("pipe:0"),
        s("-vf"),
        s("scale=out_color_matrix=bt709:out_range=tv,format=yuv420p"),
        s("-c:v"),
        s("libx264"),
        s("-preset"),
        s("veryfast"),
        s("-crf"),
        s("12"),
        s("-colorspace"),
        s("bt709"),
        s("-color_primaries"),
        s("bt709"),
        s("-color_trc"),
        s("bt709"),
        s("-color_range"),
        s("tv"),
        s("-movflags"),
        s("+faststart"),
        s("-an"),
        out.display().to_string(),
    ]
}

/// The running encoder.
struct Sink {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl Sink {
    fn spawn(size: UVec2, pix_fmt: &str, fps: u32, out: &Path) -> std::io::Result<Self> {
        if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let ffmpeg = std::env::var("WOWSOULS_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
        let mut cmd = Command::new(ffmpeg);
        cmd.args(ffmpeg_args(size, pix_fmt, fps, out))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // No console window for the encoder.
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take();
        Ok(Self { child, stdin })
    }
}

#[derive(Resource)]
struct Recorder {
    cfg: Config,
    plan: Recording,
    born: Instant,
    /// When the body was released.
    since: Option<Instant>,
    sink: Option<Sink>,
    /// Frames that arrived ahead of an earlier one.
    arrived: BTreeMap<u64, Vec<u8>>,
    /// The next frame to write.
    next_write: u64,
    failed: Option<String>,
}

impl Recorder {
    fn new(cfg: Config) -> Self {
        let plan = Recording::new(cfg.frames(), cfg.stride());
        Self {
            cfg,
            plan,
            born: Instant::now(),
            since: None,
            sink: None,
            arrived: BTreeMap::new(),
            next_write: 0,
            failed: None,
        }
    }

    /// A captured frame reached the main world: queue it, start the encoder on the first, and
    /// write every frame that is next in order.
    fn arrive(&mut self, index: u64, image: &Image) {
        if self.failed.is_some() {
            return;
        }
        let Some(data) = image.data.as_ref() else {
            self.failed = Some("a captured frame carries no pixels".into());
            return;
        };
        if self.sink.is_none() {
            let Some(pix_fmt) = pixel_format(image.texture_descriptor.format) else {
                self.failed = Some(format!(
                    "cannot record frames in {:?}",
                    image.texture_descriptor.format
                ));
                return;
            };
            let size = UVec2::new(image.width(), image.height());
            match Sink::spawn(size, pix_fmt, self.cfg.fps, &self.cfg.path) {
                Ok(sink) => {
                    info!(
                        "record: {}x{} {pix_fmt} at {} fps into {}",
                        size.x,
                        size.y,
                        self.cfg.fps,
                        self.cfg.path.display()
                    );
                    self.sink = Some(sink);
                }
                Err(e) => {
                    self.failed = Some(format!("cannot start ffmpeg: {e}"));
                    return;
                }
            }
        }
        self.arrived.insert(index, data.clone());
        while let Some(frame) = self.arrived.remove(&self.next_write) {
            let written = self
                .sink
                .as_mut()
                .and_then(|s| s.stdin.as_mut())
                .map(|stdin| stdin.write_all(&frame));
            match written {
                Some(Ok(())) => self.next_write += 1,
                Some(Err(e)) => {
                    self.failed = Some(format!("ffmpeg stopped taking frames: {e}"));
                    return;
                }
                None => {
                    self.failed = Some("the encoder is gone".into());
                    return;
                }
            }
        }
    }

    /// Closes the pipe and waits for ffmpeg to finish the file.
    fn close(&mut self) -> Result<(), String> {
        let Some(mut sink) = self.sink.take() else {
            return Err("no frame was captured".into());
        };
        drop(sink.stdin.take());
        let status = sink.child.wait().map_err(|e| e.to_string())?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| format!("ffmpeg exited with {status}"))
    }
}

pub fn plugin(app: &mut App) {
    let Ok(v) = std::env::var("WOWSOULS_RECORD") else {
        return;
    };
    let Some(cfg) = Config::parse(&v) else {
        eprintln!("wow-souls: WOWSOULS_RECORD={v:?} is not <file.mp4>,<secs>[,<fps>] (fps 15, 20, 30 or 60)");
        std::process::exit(2);
    };
    app.insert_resource(Recorder::new(cfg))
        // The clock is held until the recording releases it.
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .add_systems(Last, record);
}

fn record(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut rec: ResMut<Recorder>,
    mut strategy: ResMut<TimeUpdateStrategy>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let rec = &mut *rec;
    if let Some(why) = rec.failed.take() {
        error!("record: {why}");
        let _ = rec.close();
        exit.write(AppExit::error());
        return;
    }
    let playing = *phase.get() == Phase::Playing;
    if playing && rec.since.is_none() {
        rec.since = Some(Instant::now());
    }
    if !playing && rec.born.elapsed() > GIVE_UP {
        error!("record: the world was not playable after {GIVE_UP:?}");
        exit.write(AppExit::error());
        return;
    }
    let act = rec.plan.step(Seen {
        playing,
        advanced: !time.delta().is_zero(),
        held: rec.since.is_some_and(|t| t.elapsed() >= HOLD),
        backlog: rec.plan.requested() - rec.next_write,
        drained: rec.next_write >= rec.plan.requested(),
    });
    if act.capture {
        let index = rec.plan.requested() - 1;
        commands.spawn(Screenshot::primary_window()).observe(
            move |captured: On<ScreenshotCaptured>, mut rec: ResMut<Recorder>| {
                rec.arrive(index, &captured.image);
            },
        );
    }
    *strategy = TimeUpdateStrategy::ManualDuration(if act.clock { STEP } else { Duration::ZERO });
    if act.finished {
        match rec.close() {
            Ok(()) => {
                info!(
                    "record: {} frames, {:.1} s, written to {}",
                    rec.next_write,
                    rec.next_write as f32 / rec.cfg.fps as f32,
                    rec.cfg.path.display()
                );
                exit.write(AppExit::Success);
            }
            Err(why) => {
                error!("record: {why}");
                exit.write(AppExit::error());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_is_a_file_seconds_and_a_frame_rate() {
        let c = Config::parse("C:\\clips\\a.mp4,12.5").unwrap();
        assert_eq!(c.path, PathBuf::from("C:\\clips\\a.mp4"));
        assert_eq!((c.secs, c.fps), (12.5, 30));
        assert_eq!((c.frames(), c.stride()), (375, 2));
        let c = Config::parse("a.mp4, 4, 60").unwrap();
        assert_eq!((c.frames(), c.stride()), (240, 1));
    }

    #[test]
    fn a_request_the_clock_cannot_keep_is_refused() {
        for bad in [
            "",
            "a.mp4",
            ",5",
            "a.mp4,0",
            "a.mp4,-3",
            "a.mp4,nan",
            "a.mp4,601",
            "a.mp4,5,25",
            "a.mp4,5,30,1",
        ] {
            assert_eq!(Config::parse(bad), None, "{bad:?}");
        }
    }

    /// The seen state of a frame in which the clock moved or not.
    fn seen(playing: bool, advanced: bool, held: bool, backlog: u64) -> Seen {
        Seen {
            playing,
            advanced,
            held,
            backlog,
            drained: backlog == 0,
        }
    }

    #[test]
    fn the_clock_waits_for_the_world_the_hold_and_then_the_warm_up() {
        let mut r = Recording::new(2, 2);
        // Streaming: held. Released: still held until the hold is over.
        assert!(!r.step(seen(false, false, false, 0)).clock);
        assert!(!r.step(seen(true, false, false, 0)).clock);
        assert!(!r.step(seen(true, false, false, 0)).clock);
        // The hold ends: the clock runs, and the warm-up counts only steps that moved it.
        assert!(r.step(seen(true, false, true, 0)).clock);
        for _ in 0..WARMUP_TICKS - 1 {
            let a = r.step(seen(true, true, true, 0));
            assert!(a.clock && !a.capture);
        }
        // The last warm-up step; the next step is the first of the video.
        let a = r.step(seen(true, true, true, 0));
        assert!(a.clock && !a.capture);
        let a = r.step(seen(true, true, true, 0));
        assert!(a.capture, "the first step after the warm-up is frame 0");
        assert_eq!(r.requested(), 1);
    }

    #[test]
    fn every_stride_th_step_is_a_frame_and_the_last_one_ends_the_run() {
        let mut r = Recording::new(3, 2);
        r.stage = Stage::Capturing;
        let mut frames = Vec::new();
        for tick in 0..12 {
            let a = r.step(seen(true, true, true, 0));
            if a.capture {
                frames.push(tick);
            }
            if r.stage == Stage::Draining {
                assert!(!a.clock, "the clock stops once every frame is asked for");
                break;
            }
        }
        assert_eq!(frames, vec![0, 2, 4]);
        assert!(
            !r.step(seen(true, false, true, 1)).finished,
            "a frame is still on its way"
        );
        assert!(r.step(seen(true, false, true, 0)).finished);
    }

    #[test]
    fn a_stalled_frame_is_not_a_tick_and_a_backlog_holds_the_clock() {
        let mut r = Recording::new(10, 2);
        r.stage = Stage::Capturing;
        let a = r.step(seen(true, true, true, 0));
        assert!(a.capture && a.clock);
        // The clock did not move this frame (it was held): no tick, no frame.
        let a = r.step(seen(true, false, true, 0));
        assert!(!a.capture);
        assert_eq!(r.ticks, 1);
        // The encoder is behind: the next frame does not advance the clock.
        let a = r.step(seen(true, true, true, MAX_BACKLOG));
        assert!(!a.clock);
        // A frame asked for now counts against the backlog at once.
        let mut r = Recording::new(10, 1);
        r.stage = Stage::Capturing;
        let a = r.step(seen(true, true, true, MAX_BACKLOG - 1));
        assert!(a.capture && !a.clock);
    }

    #[test]
    fn frames_are_raw_bgra_or_rgba_and_nothing_else() {
        assert_eq!(pixel_format(TextureFormat::Bgra8UnormSrgb), Some("bgra"));
        assert_eq!(pixel_format(TextureFormat::Rgba8Unorm), Some("rgba"));
        assert_eq!(pixel_format(TextureFormat::Rgba16Float), None);
    }

    #[test]
    fn ffmpeg_takes_raw_frames_of_the_captured_size_and_writes_the_file() {
        let args = ffmpeg_args(UVec2::new(1080, 1920), "bgra", 30, Path::new("out.mp4"));
        let at = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| args[i + 1].as_str())
        };
        assert_eq!(at("-video_size"), Some("1080x1920"));
        assert_eq!(at("-pix_fmt"), Some("bgra"));
        assert_eq!(at("-framerate"), Some("30"));
        assert_eq!(at("-i"), Some("pipe:0"));
        assert_eq!(args.last().map(String::as_str), Some("out.mp4"));
    }
}
