//! The experience bar: a thin gold bar under the stamina bar, as long as the stamina bar is, filled by how
//! far the player is into the current level, with "Level 3   1099 / 1400" under it in Dark Souls' font.
//! The fill glides to a new value (a quest or a kill pays out in one step); a level-up snaps it to the
//! start of the next level. At the highest level the bar is full and says only the level.
//!
//! The numbers are the quest log's ([`QuestGame`]: the XP total and the table that says what each level
//! needs); the position and the length follow the stamina plate ([`super::art`]).

use bevy::prelude::*;

use super::art::HudArt;
use super::bars::{bar_len, fraction};
use super::font::{Align, BitmapText};
use super::{node_at, Anchor, LRect};
use crate::avatar::Avatar;
use crate::combat::Stamina;
use crate::quests::QuestGame;
use crate::settle::Phase;

/// Layout pixels between the stamina plate and the bar, the bar's height, and the label's line height.
const GAP: f32 = 14.0;
const HEIGHT: f32 = 6.0;
const LABEL_SIZE: f32 = 14.0;
/// How fast the fill glides (of the whole bar per second).
const GLIDE: f32 = 0.35;

const PLATE: Color = Color::srgba(0.02, 0.018, 0.014, 0.82);
const EDGE: Color = Color::srgba(0.42, 0.34, 0.2, 0.9);
const FILL: Color = Color::srgb(0.86, 0.68, 0.26);
const TEXT: [f32; 4] = [0.86, 0.82, 0.7, 1.0];

/// What the bar shows of an XP total: the level, the XP into it and what it takes (0 at the top level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub level: u32,
    pub into: u32,
    pub need: u32,
}

impl Reading {
    /// How far through the level: 0..=1, the top level is full.
    pub fn fraction(self) -> f32 {
        if self.need == 0 {
            1.0
        } else {
            fraction(self.into as f32, self.need as f32)
        }
    }

    /// The words under the bar.
    pub fn label(self) -> String {
        if self.need == 0 {
            format!("Level {}", self.level)
        } else {
            format!("Level {}   {} / {}", self.level, self.into, self.need)
        }
    }
}

/// The fill's own value, gliding toward the target and snapping when the level changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glide {
    pub shown: f32,
    level: u32,
}

impl Glide {
    pub fn new(reading: Reading) -> Self {
        Self {
            shown: reading.fraction(),
            level: reading.level,
        }
    }

    /// One frame of `dt` seconds toward `reading`.
    pub fn step(&mut self, reading: Reading, dt: f32) {
        let target = reading.fraction();
        if reading.level != self.level {
            // A level-up (or a reset): the bar starts the new level where it stands.
            self.level = reading.level;
            self.shown = if reading.need == 0 { 1.0 } else { 0.0 };
        }
        let step = GLIDE * dt;
        self.shown = if self.shown < target {
            (self.shown + step).min(target)
        } else {
            (self.shown - step).max(target)
        };
    }
}

#[derive(Component)]
struct Fill;
#[derive(Component)]
struct Backing;
#[derive(Component)]
struct Label;

/// What the bar glides from, between frames.
#[derive(Resource, Default)]
struct State(Option<Glide>);

pub fn plugin(app: &mut App) {
    let art = resource_exists::<HudArt>;
    app.init_resource::<State>()
        .add_systems(OnEnter(Phase::Playing), spawn.run_if(art))
        .add_systems(Update, update.run_if(in_state(Phase::Playing)).run_if(art));
}

/// The stamina plate's rectangle, which the bar hangs under.
fn anchor_rect(art: &HudArt) -> LRect {
    art.m.fill("F20-00/plate_sp").rect
}

fn spawn(mut commands: Commands, art: Res<HudArt>) {
    let plate = anchor_rect(&art);
    let top = plate.b + GAP;
    let root = commands
        .spawn((
            Name::new("hud xp"),
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
    let field = LRect {
        l: plate.l,
        t: top,
        r: plate.l + 1.0,
        b: top + HEIGHT,
    };
    let mut node = node_at(field, Anchor::Left);
    node.border = UiRect::all(px(1));
    commands
        .spawn((
            node,
            BackgroundColor(PLATE),
            BorderColor::all(EDGE),
            Backing,
            ChildOf(root),
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: percent(0),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(FILL),
                Fill,
            ));
        });
    let label = LRect {
        l: plate.l,
        t: top + HEIGHT + 3.0,
        r: plate.l + 360.0,
        b: top + HEIGHT + 3.0 + LABEL_SIZE,
    };
    commands.spawn((
        node_at(label, Anchor::Left),
        BitmapText::new("", LABEL_SIZE, Align::Left, label.w(), TEXT),
        Label,
        ChildOf(root),
    ));
}

fn update(
    real: Res<Time<Real>>,
    art: Res<HudArt>,
    game: Option<Res<QuestGame>>,
    avatar: Query<&Stamina, With<Avatar>>,
    mut state: ResMut<State>,
    mut backing: Query<&mut Node, (With<Backing>, Without<Fill>)>,
    mut fill: Query<&mut Node, (With<Fill>, Without<Backing>)>,
    mut label: Query<&mut BitmapText, With<Label>>,
) {
    let (Some(game), Ok(stamina)) = (game, avatar.single()) else {
        return;
    };
    let (level, into, need) = game.table.xp_into_level(game.xp);
    let reading = Reading { level, into, need };
    let glide = state.0.get_or_insert_with(|| Glide::new(reading));
    glide.step(reading, real.delta_secs());
    let shown = glide.shown;
    // As long as the stamina bar, which grows with the level.
    let len = bar_len(stamina.max, anchor_rect(&art).w());
    if let Ok(mut n) = backing.single_mut() {
        if n.width != px(len) {
            n.width = px(len);
        }
    }
    if let Ok(mut n) = fill.single_mut() {
        let want = percent(shown * 100.0);
        if n.width != want {
            n.width = want;
        }
    }
    if let Ok(mut t) = label.single_mut() {
        let want = reading.label();
        if t.text != want {
            t.text = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(level: u32, into: u32, need: u32) -> Reading {
        Reading { level, into, need }
    }

    #[test]
    fn the_bar_is_how_far_into_the_level_and_the_top_level_is_full() {
        assert_eq!(at(2, 0, 900).fraction(), 0.0);
        assert_eq!(at(2, 450, 900).fraction(), 0.5);
        assert_eq!(at(2, 900, 900).fraction(), 1.0);
        assert_eq!(at(3, 0, 0).fraction(), 1.0);
    }

    #[test]
    fn the_words_say_the_level_and_how_much_is_left_to_go() {
        assert_eq!(at(3, 1099, 1400).label(), "Level 3   1099 / 1400");
        assert_eq!(at(60, 0, 0).label(), "Level 60");
    }

    #[test]
    fn the_fill_glides_up_to_a_gain_and_never_overshoots() {
        let mut g = Glide::new(at(2, 100, 900));
        let to = at(2, 500, 900);
        g.step(to, 0.5);
        assert!(
            g.shown > 100.0 / 900.0 && g.shown < to.fraction(),
            "{}",
            g.shown
        );
        for _ in 0..400 {
            g.step(to, 0.05);
        }
        assert_eq!(g.shown, to.fraction());
    }

    #[test]
    fn a_level_up_starts_the_next_bar_empty_and_a_reset_goes_back_down() {
        let mut g = Glide::new(at(2, 850, 900));
        g.step(at(3, 10, 1400), 0.0);
        assert_eq!(g.shown, 0.0, "snapped to the start of the new level");
        g.step(at(3, 10, 1400), 1.0);
        assert!(g.shown > 0.0 && g.shown <= 10.0 / 1400.0 + 1e-6);
        // Falling (a new character's reset) glides down too.
        let mut g = Glide::new(at(3, 700, 1400));
        g.step(at(3, 0, 1400), 0.5);
        assert!(g.shown < 0.5 && g.shown > 0.0);
    }

    #[test]
    fn the_top_level_fills_the_bar_at_once() {
        let mut g = Glide::new(at(2, 0, 900));
        g.step(at(3, 0, 0), 0.0);
        assert_eq!(g.shown, 1.0);
    }
}
