//! Breath and drowning, as in WoW (2026-10-18, the user: "drunkningsfunktion. Precis som i wow"). With
//! the head under water the breath runs down over [`BREATH`] seconds (1.12's breath timer, one minute)
//! and shows as WoW's mirror bar at the top of the screen; at the end of it the body drowns, losing
//! [`DROWN_SHARE`] of its health every [`DROWN_EVERY`] seconds until it surfaces or dies. Out of the
//! water the breath fills again in [`REFILL`] seconds.

use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::combat::{Action, Health};
use crate::motor::{swim::Swimming, CAPSULE_HEIGHT};
use crate::settle::Phase;

/// Seconds of breath (WoW's `BREATH` mirror timer, 60 000 ms).
pub const BREATH: f32 = 60.0;
/// Seconds between drowning ticks, and the share of the health each takes.
pub const DROWN_EVERY: f32 = 2.0;
pub const DROWN_SHARE: f32 = 0.10;
/// Seconds a spent breath takes to fill again in the air.
pub const REFILL: f32 = 6.0;
/// The head's height over the feet (share of the body's height): under the surface, it is under water.
const HEAD: f32 = 0.9;

/// The avatar's breath.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Breath {
    pub left: f32,
    /// Seconds to the next drowning tick.
    pub tick: f32,
    pub under: bool,
}

impl Default for Breath {
    fn default() -> Self {
        Breath {
            left: BREATH,
            tick: DROWN_EVERY,
            under: false,
        }
    }
}

impl Breath {
    /// One frame: `under` the head is under water. Returns the drowning damage share due this frame
    /// (0 or [`DROWN_SHARE`]).
    pub fn step(&mut self, under: bool, dt: f32) -> f32 {
        self.under = under;
        if !under {
            self.left = (self.left + dt * BREATH / REFILL).min(BREATH);
            self.tick = DROWN_EVERY;
            return 0.0;
        }
        if self.left > 0.0 {
            self.left = (self.left - dt).max(0.0);
            return 0.0;
        }
        self.tick -= dt;
        if self.tick <= 0.0 {
            self.tick += DROWN_EVERY;
            DROWN_SHARE
        } else {
            0.0
        }
    }

    /// The bar's share, `None` while it is full (WoW hides a full mirror bar).
    pub fn shown(&self) -> Option<f32> {
        (self.left < BREATH - 1.0e-3).then_some(self.left / BREATH)
    }
}

#[derive(Component)]
struct Bar;
#[derive(Component)]
struct Fill;

pub fn plugin(app: &mut App) {
    app.init_resource::<Breath>()
        .add_systems(OnEnter(Phase::Playing), spawn)
        .add_systems(
            Update,
            (breathe, draw)
                .chain()
                .after(crate::controller::PlaySet::Motor)
                .run_if(in_state(Phase::Playing)),
        );
}

fn breathe(
    time: Res<Time>,
    mut breath: ResMut<Breath>,
    water: benilla_world::world_point::WorldPoint,
    avatar: Query<(Entity, &Transform, &Action, &Health, Option<&Swimming>), With<Avatar>>,
    mut hurts: MessageWriter<crate::npc::PlayerHurt>,
) {
    let Ok((me, xf, action, hp, swimming)) = avatar.single() else {
        return;
    };
    if action.is_dead() || hp.cur <= 0.0 {
        *breath = Breath::default();
        return;
    }
    let head = xf.translation + Vec3::Y * (CAPSULE_HEIGHT * HEAD);
    let surface = swimming.and_then(|s| s.surface).or_else(|| {
        water.water_surface_at(
            benilla_world::world_point::Subject::Player,
            benilla_assets::coords::bevy_to_wow(xf.translation),
        )
    });
    let under = surface.is_some_and(|s| head.y < s);
    let was = breath.under;
    let share = breath.step(under, time.delta_secs());
    if under != was {
        info!(
            "breath: head {} the water ({:.0} s left)",
            if under { "under" } else { "out of" },
            breath.left
        );
    }
    if share > 0.0 {
        let damage = (hp.max * share).round().max(1.0);
        info!("breath: drowning for {damage:.0}");
        hurts.write(crate::npc::PlayerHurt {
            from: me,
            damage,
            level: 0,
            stop: 0.0,
            knock: 0.0,
            poise: 0.0,
            blocked: false,
        });
    }
}

fn spawn(mut commands: Commands) {
    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: percent(14),
                left: percent(50),
                margin: UiRect::left(px(-130)),
                width: px(260),
                height: px(12),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.75)),
            BorderColor::all(crate::hud::theme::EDGE),
            Visibility::Hidden,
            GlobalZIndex(20),
            Bar,
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(1),
            top: px(1),
            bottom: px(1),
            width: percent(100),
            ..default()
        },
        // WoW's breath bar blue.
        BackgroundColor(Color::srgb(0.0, 0.5, 1.0)),
        Fill,
        ChildOf(bar),
    ));
}

fn draw(
    breath: Res<Breath>,
    mut bar: Query<&mut Visibility, With<Bar>>,
    mut fill: Query<&mut Node, With<Fill>>,
) {
    let shown = breath.shown();
    if let Ok(mut v) = bar.single_mut() {
        let want = if shown.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    if let (Some(share), Ok(mut node)) = (shown, fill.single_mut()) {
        let width = percent(share * 100.0);
        if node.width != width {
            node.width = width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minute_of_breath_then_a_tenth_of_health_every_two_seconds() {
        let mut b = Breath::default();
        assert_eq!(b.shown(), None);
        assert_eq!(b.step(true, 59.0), 0.0);
        assert!(b.shown().is_some());
        assert_eq!(b.step(true, 1.0), 0.0, "the last of the breath");
        assert_eq!(b.step(true, 1.0), 0.0);
        assert_eq!(
            b.step(true, 1.0),
            DROWN_SHARE,
            "two seconds after the breath ran out"
        );
        assert_eq!(b.step(true, 2.0), DROWN_SHARE);
        // Surfacing stops it and the breath fills again.
        assert_eq!(b.step(false, REFILL), 0.0);
        assert_eq!(b.left, BREATH);
        assert_eq!(b.shown(), None);
    }
}
