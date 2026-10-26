//! Deliberate target-change gestures, independent of free-camera sensitivity.
use super::Side;
use bevy::prelude::*;

const GAP: f32 = 0.45;
const REST: f32 = 0.12;
const MOUSE_DISTANCE: f32 = 120.0;
const MOUSE_WINDOW: f32 = 0.20;
const MOUSE_SPEED: f32 = 800.0;
const QUIET_SPEED: f32 = 50.0;

#[derive(Debug)]
pub struct Flick {
    since: f32,
    stick_held: bool,
    stick_quiet: f32,
    mouse_armed: bool,
    mouse_quiet: f32,
    stroke: Vec2,
    stroke_age: f32,
}
impl Default for Flick {
    fn default() -> Self {
        Self {
            since: GAP,
            stick_held: true,
            stick_quiet: 0.0,
            mouse_armed: false,
            mouse_quiet: 0.0,
            stroke: Vec2::ZERO,
            stroke_age: 0.0,
        }
    }
}
impl Flick {
    /// Raw mouse pixels, not camera radians. A consumed stroke must actually stop before rearming.
    pub fn step(&mut self, stick: f32, mouse: Vec2, dt: f32) -> Option<Side> {
        if !dt.is_finite() || dt <= 0.0 || dt > 0.1 || !mouse.is_finite() || !stick.is_finite() {
            self.rest();
            return None;
        }
        self.since = (self.since + dt).min(GAP);
        let mut stick_side = None;
        if stick.abs() < 0.25 {
            self.stick_quiet += dt;
            if self.stick_quiet >= REST {
                self.stick_held = false;
            }
        } else {
            self.stick_quiet = 0.0;
            if !self.stick_held && stick.abs() >= 0.75 {
                self.stick_held = true;
                stick_side = Some(side(stick));
            }
        }
        let mouse_side = self.mouse_step(mouse, dt);
        let request = stick_side.or(mouse_side);
        if request.is_some() && self.since >= GAP {
            self.since = 0.0;
            request
        } else {
            // An input during cooldown is consumed, never queued for a later surprise switch.
            None
        }
    }

    fn clear_stroke(&mut self) {
        self.stroke = Vec2::ZERO;
        self.stroke_age = 0.0;
    }

    fn mouse_step(&mut self, delta: Vec2, dt: f32) -> Option<Side> {
        if self.stroke != Vec2::ZERO {
            self.stroke_age += dt;
            if self.stroke_age > MOUSE_WINDOW {
                self.clear_stroke();
            }
        }
        if delta.length() <= QUIET_SPEED * dt {
            self.mouse_quiet += dt;
            if self.mouse_quiet >= REST {
                self.mouse_armed = true;
                self.clear_stroke();
            }
            return None;
        }
        self.mouse_quiet = 0.0;
        if !self.mouse_armed {
            return None;
        }
        if self.stroke.x * delta.x < 0.0 {
            self.clear_stroke();
        }
        if self.stroke == Vec2::ZERO {
            self.stroke_age = dt;
        }
        self.stroke += Vec2::new(delta.x, delta.y.abs());
        let across = self.stroke.x.abs();
        if across >= MOUSE_DISTANCE
            && across >= self.stroke.y * 1.7
            && across / self.stroke_age >= MOUSE_SPEED
        {
            let direction = side(self.stroke.x);
            self.mouse_armed = false;
            self.clear_stroke();
            Some(direction)
        } else {
            None
        }
    }

    pub fn rest(&mut self) {
        *self = Self::default();
    }
}
fn side(x: f32) -> Side {
    if x > 0.0 {
        Side::Right
    } else {
        Side::Left
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Mouse(Vec2),
    Stick(f32),
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stroke {
    at: f32,
    duration: f32,
    kind: Kind,
}

/// Unattended raw-input replay, using the same gesture detector as live input.
#[derive(Resource, Debug, PartialEq)]
pub(super) struct InputScript(Vec<Stroke>);
impl InputScript {
    pub fn parse(text: &str) -> Option<Self> {
        let number = |v: &str| v.parse::<f32>().ok().filter(|v| v.is_finite());
        let strokes = text
            .split(',')
            .map(|part| {
                let fields: Vec<_> = part.trim().split(':').collect();
                let (at, kind, duration) = match fields.as_slice() {
                    [at, "mouse", x, y, duration] => (
                        number(at)?,
                        Kind::Mouse(Vec2::new(number(x)?, number(y)?)),
                        number(duration)?,
                    ),
                    [at, "stick", x, duration] => (
                        number(at)?,
                        Kind::Stick(number(x)?.clamp(-1.0, 1.0)),
                        number(duration)?,
                    ),
                    _ => return None,
                };
                (at >= 0.0 && duration > 0.0 && duration <= 10.0).then_some(Stroke {
                    at,
                    duration,
                    kind,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        (!strokes.is_empty()).then_some(Self(strokes))
    }

    pub fn sample(&self, now: f32, dt: f32) -> (f32, Vec2) {
        let mut stick: f32 = 0.0;
        let mut mouse = Vec2::ZERO;
        for stroke in &self.0 {
            let overlap =
                (now.min(stroke.at + stroke.duration) - (now - dt).max(stroke.at)).max(0.0);
            if overlap == 0.0 {
                continue;
            }
            match stroke.kind {
                Kind::Mouse(total) => mouse += total * (overlap / stroke.duration),
                Kind::Stick(push) if push.abs() > stick.abs() => stick = push,
                Kind::Stick(_) => {}
            }
        }
        (stick, mouse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn replay(script: &str, fps: u32) -> Vec<Side> {
        let script = InputScript::parse(script).unwrap();
        let mut flick = Flick::default();
        let dt = 1.0 / fps as f32;
        (1..=fps * 5)
            .filter_map(|frame| {
                let (stick, mouse) = script.sample(frame as f32 * dt, dt);
                flick.step(stick, mouse, dt)
            })
            .collect()
    }

    #[test]
    fn deliberate_strokes_work_at_30_60_and_144_fps() {
        for fps in [30, 60, 144] {
            assert_eq!(
                replay("0.5:mouse:160:8:0.16,1.5:mouse:-160:-8:0.16", fps),
                [Side::Right, Side::Left],
                "{fps}"
            );
            assert!(
                replay(
                    "0.5:mouse:40:0:0.1,1.5:mouse:600:0:2,4:mouse:180:300:0.16",
                    fps
                )
                .is_empty(),
                "{fps}"
            );
        }
    }

    #[test]
    fn continuous_motion_and_held_sticks_never_repeat() {
        for fps in [30, 60, 144] {
            assert_eq!(replay("0.5:mouse:2000:0:2", fps), [Side::Right]);
            assert_eq!(
                replay("0.5:stick:0.9:2,3:stick:-0.9:0.5", fps),
                [Side::Right, Side::Left]
            );
        }
    }

    #[test]
    fn cooldown_consumes_strokes_and_does_not_queue_them() {
        assert_eq!(
            replay(
                "0.5:mouse:160:0:0.1,0.75:mouse:-160:0:0.1,1.5:mouse:-160:0:0.1",
                60
            ),
            [Side::Right, Side::Left]
        );
    }

    #[test]
    fn jitter_reversals_and_vertical_motion_do_not_add_up() {
        let mut flick = Flick::default();
        for _ in 0..30 {
            flick.step(0.0, Vec2::ZERO, 1.0 / 60.0);
        }
        for n in 0..240 {
            assert_eq!(
                flick.step(
                    0.0,
                    Vec2::new(if n % 2 == 0 { 12.0 } else { -12.0 }, 2.0),
                    1.0 / 60.0
                ),
                None
            );
        }
        assert!(replay("0.5:mouse:180:600:0.18", 60).is_empty());
    }

    #[test]
    fn reset_stale_input_and_frame_hitches_require_new_neutral_input() {
        let mut flick = Flick::default();
        for _ in 0..30 {
            flick.step(0.0, Vec2::ZERO, 1.0 / 60.0);
        }
        flick.rest();
        assert_eq!(flick.step(0.0, Vec2::new(1000.0, 0.0), 1.0 / 60.0), None);
        assert_eq!(flick.step(0.0, Vec2::new(1000.0, 0.0), 0.5), None);
        assert_eq!(flick.step(0.0, Vec2::new(1000.0, 0.0), 1.0 / 60.0), None);
    }

    #[test]
    fn replay_parser_rejects_invalid_time_and_motion() {
        for text in [
            "",
            "x:mouse:1:0:0.2",
            "-1:stick:1:1",
            "1:mouse:NaN:0:1",
            "1:mouse:1:0:0",
            "1:stick:1:11",
        ] {
            assert!(InputScript::parse(text).is_none(), "{text}");
        }
    }
}
