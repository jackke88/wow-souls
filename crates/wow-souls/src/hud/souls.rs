//! The souls counter at the bottom right, as Dark Souls draws it: a dark plate, the count in the
//! HUD font, and above it the souls a kill just gave, which fade after a moment while the count
//! rolls up to its new value. A kill's souls are the creature's `getSoul` (the kit's `souls`).
//! Dying loses them to a bloodstain ([`crate::bloodstain`]).

use bevy::prelude::*;
use bevy::time::Real;

use super::art::{HudArt, LRect};
use super::font::{Align, BitmapText};
use super::{node_at, sprite_tint, Anchor};
use crate::npc::Slain;
use crate::settle::Phase;

/// The souls the player carries.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Souls(pub u32);

/// How fast the shown count closes on the real one (per second, exponential).
const ROLL_RATE: f32 = 5.0;
/// How long a gain stays above the counter (s), and how much of that it spends fading out.
const GAIN_SHOW: f32 = 2.2;
const GAIN_FADE: f32 = 0.6;

/// What the counter shows: the rolling count and the latest gain.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct Counter {
    pub shown: f32,
    pub gain: u32,
    age: f32,
}

impl Counter {
    /// A kill's souls appear above the counter; gains that follow each other close up add together.
    pub fn add(&mut self, n: u32) {
        if n == 0 {
            return;
        }
        if self.age >= GAIN_SHOW {
            self.gain = 0;
        }
        self.gain += n;
        self.age = 0.0;
    }

    /// Advances `dt` seconds towards `value`: the count rolls up (never past it), the gain ages.
    pub fn step(&mut self, value: u32, dt: f32) {
        let v = value as f32;
        if (v - self.shown).abs() < 0.5 {
            self.shown = v;
        } else {
            self.shown += (v - self.shown) * (1.0 - (-ROLL_RATE * dt).exp());
        }
        if self.gain > 0 {
            self.age += dt;
            if self.age >= GAIN_SHOW {
                self.gain = 0;
            }
        }
    }

    /// The count as Dark Souls prints it: plain digits.
    pub fn text(&self) -> String {
        format!("{}", self.shown.round() as u32)
    }

    /// The gain's digits, empty when none shows.
    pub fn gain_text(&self) -> String {
        if self.gain > 0 {
            format!("{}", self.gain)
        } else {
            String::new()
        }
    }

    /// 1 while the gain holds, falling to 0 over its last [`GAIN_FADE`] seconds.
    pub fn gain_alpha(&self) -> f32 {
        if self.gain == 0 {
            0.0
        } else {
            ((GAIN_SHOW - self.age) / GAIN_FADE).clamp(0.0, 1.0)
        }
    }
}

#[derive(Component)]
struct CountText;

#[derive(Component)]
struct GainText;
#[derive(Component)]
struct NativeRoot;

pub fn plugin(app: &mut App) {
    app.init_resource::<Souls>()
        .init_resource::<Counter>()
        .add_systems(Update, credit)
        .add_systems(
            OnEnter(Phase::Playing),
            spawn.run_if(resource_exists::<HudArt>.or(super::theme::enabled)),
        )
        .add_systems(
            Update,
            update
                .run_if(in_state(Phase::Playing))
                .run_if(resource_exists::<HudArt>.or(super::theme::enabled)),
        );
}

/// A kill's souls go to the player and to the counter.
fn credit(mut slain: MessageReader<Slain>, mut souls: ResMut<Souls>, mut counter: ResMut<Counter>) {
    for s in slain.read() {
        souls.0 = souls.0.saturating_add(s.souls);
        counter.add(s.souls);
        if s.souls > 0 {
            info!("souls: +{} from {:?} ({} carried)", s.souls, s.who, souls.0);
        }
    }
}

/// The rectangle moved by the dialog's place in the HUD.
fn shifted(r: LRect, to: LRect) -> LRect {
    LRect {
        l: r.l + to.l,
        t: r.t + to.t,
        r: r.r + to.l,
        b: r.b + to.t,
    }
}

fn spawn(mut commands: Commands, art: Option<Res<HudArt>>, fonts: Res<crate::loot::ui::Fonts>) {
    if super::theme::enabled() {
        let root = super::theme::panel(
            &mut commands,
            Node {
                position_type: PositionType::Absolute,
                right: px(34),
                bottom: px(22),
                width: px(134),
                height: px(26),
                padding: UiRect::horizontal(px(10)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            None,
        );
        commands.entity(root).insert(NativeRoot);
        super::theme::words(
            &mut commands,
            root,
            "Souls",
            &fonts.body,
            12.0,
            Node::default(),
        );
        let count =
            super::theme::words(&mut commands, root, "0", &fonts.body, 17.0, Node::default());
        commands.entity(count).insert(CountText);
        let gain = super::theme::words(
            &mut commands,
            root,
            "",
            &fonts.body,
            14.0,
            Node {
                position_type: PositionType::Absolute,
                right: px(8),
                bottom: px(32),
                ..default()
            },
        );
        commands.entity(gain).insert(GainText);
        return;
    }
    let Some(art) = art else { return };
    let place = art.m.place("F20-00/soul_param");
    let root = commands
        .spawn((
            Name::new("hud souls"),
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
    for key in ["soul_param/bg_parchment_0", "soul_param/Option_line_0"] {
        let s = art.m.sprite(key);
        commands.spawn((
            node_at(shifted(s.rect, place), Anchor::Right),
            ImageNode::new(art.sprite_image(key)).with_color(sprite_tint(s.tint)),
            ChildOf(root),
        ));
    }
    for (key, count) in [
        ("soul_param/text_param_soul", true),
        ("soul_param/text_param_getsoul", false),
    ] {
        let t = art.m.text(key);
        let rect = shifted(t.rect, place);
        let text = BitmapText::new("", t.size, Align::Right, rect.w(), t.color);
        let node = node_at(rect, Anchor::Right);
        if count {
            commands.spawn((node, text, CountText, ChildOf(root)));
        } else {
            commands.spawn((node, text, GainText, ChildOf(root)));
        }
    }
}

fn update(
    real: Res<Time<Real>>,
    souls: Res<Souls>,
    mut counter: ResMut<Counter>,
    mut count: Query<&mut BitmapText, (With<CountText>, Without<GainText>)>,
    mut gain: Query<&mut BitmapText, (With<GainText>, Without<CountText>)>,
    mut native: Query<
        (&mut Text, &mut TextColor, Has<GainText>),
        Or<(With<CountText>, With<GainText>)>,
    >,
    screen: Res<crate::loot::Screen>,
    log: Res<crate::quests::LogScreen>,
    mut root: Query<&mut Visibility, With<NativeRoot>>,
) {
    for mut vis in &mut root {
        *vis = if screen.open || log.open {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    counter.step(souls.0, real.delta_secs());
    for (mut text, mut color, is_gain) in &mut native {
        let want = if is_gain {
            counter.gain_text()
        } else {
            counter.text()
        };
        if text.0 != want {
            text.0 = want;
        }
        if is_gain {
            color.0 = super::theme::IVORY.with_alpha(counter.gain_alpha());
        }
    }
    if let Ok(mut t) = count.single_mut() {
        let want = counter.text();
        if t.text != want {
            t.text = want;
        }
    }
    if let Ok(mut t) = gain.single_mut() {
        let (want, alpha) = (counter.gain_text(), counter.gain_alpha());
        if t.text != want {
            t.text = want;
        }
        if (t.alpha - alpha).abs() > 0.01 {
            t.alpha = alpha;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_counter_rolls_up_and_stops_on_the_value() {
        let mut c = Counter::default();
        let mut last = 0.0;
        for _ in 0..(3.0 * 60.0) as usize {
            c.step(150, 1.0 / 60.0);
            assert!(c.shown >= last && c.shown <= 150.0, "{}", c.shown);
            last = c.shown;
        }
        assert_eq!(c.shown, 150.0);
        assert_eq!(c.text(), "150");
    }

    #[test]
    fn the_counter_never_shows_more_than_it_is_given() {
        let mut c = Counter::default();
        c.step(7, 10.0);
        assert!(c.shown <= 7.0);
        assert_eq!(c.text(), "7");
    }

    #[test]
    fn the_gain_shows_then_fades_then_hides() {
        let mut c = Counter::default();
        c.add(150);
        assert_eq!((c.gain_text().as_str(), c.gain_alpha()), ("150", 1.0));
        c.step(150, 1.5);
        assert_eq!(c.gain_alpha(), 1.0, "it holds before it fades");
        c.step(150, 0.4);
        assert!(
            c.gain_alpha() > 0.0 && c.gain_alpha() < 1.0,
            "{}",
            c.gain_alpha()
        );
        c.step(150, 0.5);
        assert_eq!(c.gain_text(), "");
        assert_eq!(c.gain_alpha(), 0.0);
    }

    #[test]
    fn gains_close_together_add_up_and_a_late_one_starts_over() {
        let mut c = Counter::default();
        c.add(150);
        c.step(150, 1.0);
        c.add(150);
        assert_eq!(c.gain, 300);
        c.step(300, GAIN_SHOW + 0.1);
        assert_eq!(c.gain, 0);
        c.add(25);
        assert_eq!(c.gain, 25);
        c.add(0);
        assert_eq!(c.gain, 25);
    }

    #[test]
    fn a_kill_credits_the_souls_once() {
        let mut app = App::new();
        app.add_message::<Slain>()
            .init_resource::<Souls>()
            .init_resource::<Counter>()
            .add_systems(Update, credit);
        app.world_mut().write_message(Slain {
            who: Entity::from_raw_u32(7).unwrap(),
            souls: 150,
        });
        app.update();
        assert_eq!(app.world().resource::<Souls>().0, 150);
        assert_eq!(app.world().resource::<Counter>().gain, 150);
        app.update();
        assert_eq!(app.world().resource::<Souls>().0, 150, "read once");
    }
}
