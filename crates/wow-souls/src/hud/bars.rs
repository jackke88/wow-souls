//! The player's health and stamina bars in Dark Souls' own layout: a black plate, a pale-yellow
//! "chip" layer that keeps the value a blow just took off and then drains, the red / green fill
//! on top, the ruler with its ticks, the end cap, the fixed head of the bar and the skull.
//!
//! Every position and picture comes from the HUD cache ([`super::art`]); the length of a bar is
//! its stat's maximum times [`PX_PER_POINT`] layout pixels, cropped, not stretched, so the
//! ruler's ticks keep their spacing.

use bevy::prelude::*;

use super::art::HudArt;
use super::{clip_sprite, node_at, sprite_node, tint, Anchor};
use crate::avatar::Avatar;
use crate::combat::{Health, Stamina, HEALTH_MAX, STAMINA_MAX};
use crate::settle::Phase;

/// Layout pixels of bar per point of the stat (the plain HUD used 3 as well).
pub const PX_PER_POINT: f32 = 3.0;
/// How long the chip stays after a drop (s) and how fast it then drains (of the maximum per s).
const HEALTH_HOLD: f32 = 0.55;
const HEALTH_DRAIN: f32 = 1.6;
const STAMINA_HOLD: f32 = 0.25;
const STAMINA_DRAIN: f32 = 3.0;

/// The length of a bar (layout px) for a stat maximum, never longer than the plate Dark Souls
/// drew (`longest`) nor shorter than a pixel.
pub fn bar_len(max: f32, longest: f32) -> f32 {
    (max * PX_PER_POINT).clamp(1.0, longest)
}

/// A value as a fraction of its maximum, in 0..=1.
pub fn fraction(value: f32, max: f32) -> f32 {
    if max <= 0.0 {
        0.0
    } else {
        (value / max).clamp(0.0, 1.0)
    }
}

/// The "chip": the value the bar had before the last drop, held for a moment, then eased down to
/// the current value. A rise takes it up at once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chip {
    pub shown: f32,
    hold: f32,
    last: f32,
}

impl Chip {
    pub fn new(value: f32) -> Self {
        Self {
            shown: value,
            hold: 0.0,
            last: value,
        }
    }

    /// One frame of `dt` seconds with the stat at `cur` of `max`: a drop restarts the hold, the
    /// chip waits `hold_s`, then falls `drain_per_s` of the maximum per second, never below
    /// `cur`.
    pub fn step(&mut self, cur: f32, max: f32, dt: f32, hold_s: f32, drain_per_s: f32) {
        if cur >= self.shown {
            self.shown = cur;
            self.hold = 0.0;
        } else {
            if cur < self.last {
                self.hold = hold_s;
            }
            if self.hold > 0.0 {
                self.hold = (self.hold - dt).max(0.0);
            } else {
                self.shown = (self.shown - drain_per_s * max * dt).max(cur);
            }
        }
        self.last = cur;
    }
}

#[derive(Resource)]
struct Chips {
    health: Chip,
    stamina: Chip,
}

impl Default for Chips {
    fn default() -> Self {
        Self {
            health: Chip::new(HEALTH_MAX),
            stamina: Chip::new(STAMINA_MAX),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Health,
    Stamina,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    Plate,
    Chip,
    Fill,
    Ruler,
    Cap,
}

/// A node whose width (or, for the cap, position) follows a stat.
#[derive(Component, Debug, Clone, Copy)]
struct Bar {
    row: Row,
    layer: Layer,
}

/// The art keys of a row.
struct Keys {
    plate: &'static str,
    chip: &'static str,
    fill: &'static str,
    ruler: &'static str,
    cap: &'static str,
}

fn keys(row: Row) -> Keys {
    match row {
        Row::Health => Keys {
            plate: "F20-00/plate_hp",
            chip: "F20-00/bar_0_hp",
            fill: "F20-00/bar_1_hp",
            ruler: "F20-00/bar_2_hp",
            cap: "F20-00/HP_endbar",
        },
        Row::Stamina => Keys {
            plate: "F20-00/plate_sp",
            chip: "F20-00/bar_0_sp",
            fill: "F20-00/bar_1_sp",
            ruler: "F20-00/bar_2_sp",
            cap: "F20-00/SP_endbar",
        },
    }
}

/// The parts that never move: the bar's head and the skull, in Dark Souls' draw order.
const HEAD: [&str; 7] = [
    "F20-00/bar_BG_0",
    "F20-00/bar_BG_1",
    "F20-00/bar_start",
    "F20-00/bar_start1",
    "F20-02/ring_BG",
    "F20-02/icon_ring_1",
    "F20-02/icon_stone",
];

pub fn plugin(app: &mut App) {
    let art = resource_exists::<HudArt>;
    app.init_resource::<Chips>()
        .add_systems(OnEnter(Phase::Playing), spawn.run_if(art))
        .add_systems(Update, update.run_if(in_state(Phase::Playing)).run_if(art));
}

fn spawn(mut commands: Commands, art: Res<HudArt>) {
    let root = commands
        .spawn((
            Name::new("hud bars"),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
        ))
        .id();
    for (row, max) in [(Row::Health, HEALTH_MAX), (Row::Stamina, STAMINA_MAX)] {
        let k = keys(row);
        let plate = art.m.fill(k.plate);
        let len = bar_len(max, plate.rect.w());
        let mut node = node_at(plate.rect, Anchor::Left);
        node.width = px(len);
        commands.spawn((
            node,
            BackgroundColor(tint(plate.color)),
            Bar {
                row,
                layer: Layer::Plate,
            },
            ChildOf(root),
        ));
        for (key, layer) in [(k.chip, Layer::Chip), (k.fill, Layer::Fill)] {
            let holder = clip_sprite(&mut commands, root, &art, key, len);
            commands.entity(holder).insert(Bar { row, layer });
        }
        let ruler = art.m.sprite(k.ruler);
        let holder = clip_sprite(
            &mut commands,
            root,
            &art,
            k.ruler,
            len + (ruler.rect.w() - plate.rect.w()),
        );
        commands.entity(holder).insert(Bar {
            row,
            layer: Layer::Ruler,
        });
    }
    for row in [Row::Health, Row::Stamina] {
        let k = keys(row);
        let (mut node, image) = sprite_node(&art, k.cap, Anchor::Left);
        let plate = art.m.fill(k.plate);
        node.left = px(plate.rect.l + bar_len(HEALTH_MAX, plate.rect.w()));
        commands.spawn((
            node,
            image,
            Bar {
                row,
                layer: Layer::Cap,
            },
            ChildOf(root),
        ));
    }
    for key in HEAD {
        let (node, image) = sprite_node(&art, key, Anchor::Left);
        commands.spawn((node, image, ChildOf(root)));
    }
}

fn update(
    real: Res<Time<bevy::time::Real>>,
    art: Res<HudArt>,
    avatar: Query<(&Health, &Stamina), With<Avatar>>,
    mut chips: ResMut<Chips>,
    mut bars: Query<(&Bar, &mut Node)>,
) {
    let Ok((hp, st)) = avatar.single() else {
        return;
    };
    let dt = real.delta_secs();
    chips
        .health
        .step(hp.cur, hp.max, dt, HEALTH_HOLD, HEALTH_DRAIN);
    chips
        .stamina
        .step(st.cur, st.max, dt, STAMINA_HOLD, STAMINA_DRAIN);
    for (bar, mut node) in &mut bars {
        let (cur, max, chip) = match bar.row {
            Row::Health => (hp.cur, hp.max, chips.health.shown),
            Row::Stamina => (st.cur, st.max, chips.stamina.shown),
        };
        let k = keys(bar.row);
        let plate = art.m.fill(k.plate);
        let len = bar_len(max, plate.rect.w());
        let want = match bar.layer {
            Layer::Plate => (Some(len), None),
            Layer::Chip => (Some(len * fraction(chip, max)), None),
            Layer::Fill => (Some(len * fraction(cur, max)), None),
            Layer::Ruler => (
                Some(len + (art.m.sprite(k.ruler).rect.w() - plate.rect.w())),
                None,
            ),
            Layer::Cap => (None, Some(plate.rect.l + len)),
        };
        if let Some(w) = want.0 {
            if node.width != px(w) {
                node.width = px(w);
            }
        }
        if let Some(l) = want.1 {
            if node.left != px(l) {
                node.left = px(l);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `chip` for `secs` in `dt` steps with the stat at `cur` of 100.
    fn run(chip: &mut Chip, cur: f32, secs: f32) {
        let dt = 0.01;
        for _ in 0..(secs / dt).round() as usize {
            chip.step(cur, 100.0, dt, 0.5, 1.6);
        }
    }

    #[test]
    fn a_drop_holds_then_drains() {
        let mut c = Chip::new(100.0);
        c.step(60.0, 100.0, 0.01, 0.5, 1.6);
        assert_eq!(c.shown, 100.0, "the chip keeps the old value at first");
        run(&mut c, 60.0, 0.4);
        assert_eq!(c.shown, 100.0, "and holds through the hold");
        run(&mut c, 60.0, 0.2);
        assert!(c.shown < 100.0, "then it starts to fall: {}", c.shown);
        run(&mut c, 60.0, 0.5);
        assert_eq!(c.shown, 60.0, "and rests on the current value");
    }

    #[test]
    fn a_rise_snaps() {
        let mut c = Chip::new(100.0);
        run(&mut c, 40.0, 2.0);
        assert_eq!(c.shown, 40.0);
        c.step(55.0, 100.0, 0.01, 0.5, 1.6);
        assert_eq!(c.shown, 55.0);
        c.step(100.0, 100.0, 0.01, 0.5, 1.6);
        assert_eq!(c.shown, 100.0);
    }

    #[test]
    fn the_chip_is_never_below_the_current_value() {
        let mut c = Chip::new(100.0);
        for (i, cur) in [100.0, 80.0, 80.0, 30.0, 45.0, 0.0, 0.0, 100.0]
            .into_iter()
            .enumerate()
        {
            for _ in 0..50 {
                c.step(cur, 100.0, 0.02, 0.5, 1.6);
                assert!(c.shown >= cur, "step {i}: {} < {cur}", c.shown);
            }
        }
    }

    #[test]
    fn a_second_drop_restarts_the_hold() {
        let mut c = Chip::new(100.0);
        c.step(80.0, 100.0, 0.01, 0.5, 1.6);
        run(&mut c, 80.0, 0.45);
        assert_eq!(c.shown, 100.0);
        // Just before the first hold would end, another blow lands.
        c.step(60.0, 100.0, 0.01, 0.5, 1.6);
        run(&mut c, 60.0, 0.4);
        assert_eq!(c.shown, 100.0, "the clock started again");
        run(&mut c, 60.0, 0.2);
        assert!(c.shown < 100.0);
    }

    #[test]
    fn bar_length_scales_with_the_maximum_and_stops_at_the_plate() {
        assert_eq!(bar_len(100.0, 732.0), 300.0);
        assert_eq!(bar_len(150.0, 732.0), 450.0);
        assert_eq!(bar_len(1000.0, 732.0), 732.0);
        assert_eq!(bar_len(0.0, 732.0), 1.0);
    }

    #[test]
    fn fractions_clamp() {
        assert_eq!(fraction(50.0, 100.0), 0.5);
        assert_eq!(fraction(-5.0, 100.0), 0.0);
        assert_eq!(fraction(120.0, 100.0), 1.0);
        assert_eq!(fraction(1.0, 0.0), 0.0);
    }
}
