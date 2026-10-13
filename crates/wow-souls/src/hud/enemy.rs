//! Enemy bars in the Dark Souls style: every creature the player has struck gets its own small bar
//! over its head (up to eight, as the layout has eight slots), with its name above and the damage
//! just dealt below, in the HUD font. The dark fill is its health; behind it an orange chip keeps
//! what the last blow took off for a moment. A living enemy's bar lingers after the last blow, a
//! dead one's goes with its damage number.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::time::Real;

use super::art::HudArt;
use super::bars::{fraction, Chip};
use super::font::{Align, BitmapText};
use super::{clip_sprite, node_at, sprite_tint, Anchor};
use crate::combat::Health;
use crate::hits::Hit;
use crate::settle::Phase;

/// Blows closer together than this (s) add up in the shown number.
const DAMAGE_RUN: f32 = 1.5;
/// A living enemy's bar stays this long (s) after the last blow.
const BAR_LINGER: f32 = 4.0;
/// The layout has eight enemy slots.
const MAX_BARS: usize = 8;
/// The chip holds this long (s) after a blow, then drains this fast (of the maximum per s).
const CHIP_HOLD: f32 = 0.5;
const CHIP_DRAIN: f32 = 1.2;
/// How far over the creature's feet the bar hangs (yd): above a kobold's head.
const HEAD_LIFT: f32 = 1.45;

/// The damage run of one enemy: blows that land within [`DAMAGE_RUN`] of each other add up.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Run {
    pub total: f32,
    pub last_hit: f32,
}

impl Run {
    pub fn add(&mut self, damage: f32, now: f32) {
        if now - self.last_hit > DAMAGE_RUN {
            self.total = 0.0;
        }
        self.total += damage;
        self.last_hit = now;
    }

    /// The number to show while the run is fresh.
    pub fn shown(&self, now: f32) -> Option<u32> {
        (now - self.last_hit < DAMAGE_RUN).then(|| self.total.round() as u32)
    }

    /// Whether the bar still shows: a living enemy's lingers, a dead one's goes with its number.
    pub fn visible(&self, now: f32, alive: bool) -> bool {
        now - self.last_hit < if alive { BAR_LINGER } else { DAMAGE_RUN }
    }
}

/// A creature's name as the HUD prints it: each word capitalised.
pub fn display_name(name: &str) -> String {
    name.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Viewport pixels (what `world_to_viewport` gives) to layout pixels under `UiScale`.
pub fn to_layout(viewport: Vec2, ui_scale: f32) -> Vec2 {
    viewport / ui_scale.max(0.1)
}

/// The UI of one bar.
#[derive(Debug, Clone, Copy)]
struct BarUi {
    root: Entity,
    chip: Entity,
    cur: Entity,
    damage: Entity,
}

struct Tracked {
    run: Run,
    chip: Chip,
    ui: BarUi,
}

#[derive(Resource, Default)]
struct Tracking(HashMap<Entity, Tracked>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Tracking>().add_systems(
        PostUpdate,
        update
            .after(bevy::transform::TransformSystems::Propagate)
            .run_if(in_state(Phase::Playing))
            .run_if(resource_exists::<HudArt>),
    );
}

fn spawn_ui(commands: &mut Commands, art: &HudArt, name: &str) -> BarUi {
    let d = art.m.dialog("enemy_HP");
    let root = commands
        .spawn((
            Name::new("enemy bar"),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: px(d.w()),
                height: px(d.h()),
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    let bg = art.m.sprite("enemy_HP/BG");
    commands.spawn((
        node_at(bg.rect, Anchor::Left),
        ImageNode::new(art.sprite_image("enemy_HP/BG")).with_color(sprite_tint(bg.tint)),
        ChildOf(root),
    ));
    let full = art.m.sprite("enemy_HP/HP_bar1").rect.w();
    let chip = clip_sprite(commands, root, art, "enemy_HP/HP_bar1", full);
    let cur = clip_sprite(commands, root, art, "enemy_HP/HP_bar0", full);
    let nm = art.m.text("enemy_HP/text_param_name");
    commands.spawn((
        node_at(nm.rect, Anchor::Left),
        BitmapText::new(
            display_name(name),
            nm.rect.h(),
            Align::Left,
            nm.rect.w(),
            nm.color,
        ),
        ChildOf(root),
    ));
    let dm = art.m.text("enemy_HP/text_param_damage");
    let damage = commands
        .spawn((
            node_at(dm.rect, Anchor::Left),
            BitmapText::new("", dm.rect.h(), Align::Left, dm.rect.w(), dm.color),
            ChildOf(root),
        ))
        .id();
    BarUi {
        root,
        chip,
        cur,
        damage,
    }
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut commands: Commands,
    real: Res<Time<Real>>,
    art: Res<HudArt>,
    ui_scale: Res<UiScale>,
    mut hits: MessageReader<Hit>,
    mut tracking: ResMut<Tracking>,
    targets: Query<(&GlobalTransform, &Health, Option<&Name>)>,
    bosses: Query<(), With<crate::species::Boss>>,
    cams: Query<(&Camera, &GlobalTransform), With<benilla_world::view::WorldCamera>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut BitmapText>,
    mut vis: Query<&mut Visibility>,
) {
    let now = real.elapsed_secs();
    let dt = real.delta_secs();
    for h in hits.read() {
        let Ok((_, hp, name)) = targets.get(h.target) else {
            continue;
        };
        // A boss has the wide bar at the bottom ([`super::boss`]), not one over its head.
        if bosses.contains(h.target) {
            continue;
        }
        if !tracking.0.contains_key(&h.target) {
            if tracking.0.len() >= MAX_BARS {
                let oldest = tracking
                    .0
                    .iter()
                    .min_by(|a, b| a.1.run.last_hit.total_cmp(&b.1.run.last_hit))
                    .map(|(e, _)| *e);
                if let Some(e) = oldest {
                    if let Some(t) = tracking.0.remove(&e) {
                        commands.entity(t.ui.root).despawn();
                    }
                }
            }
            let ui = spawn_ui(&mut commands, &art, name.map_or("Enemy", |n| n.as_str()));
            // The bar starts at what the creature had before this blow.
            tracking.0.insert(
                h.target,
                Tracked {
                    run: Run::default(),
                    chip: Chip::new((hp.cur + h.damage).min(hp.max)),
                    ui,
                },
            );
        }
        if let Some(t) = tracking.0.get_mut(&h.target) {
            t.run.add(h.damage, now);
        }
    }
    let (cam, cam_xf) = match cams.single() {
        Ok(c) => c,
        Err(_) => return,
    };
    let full = art.m.sprite("enemy_HP/HP_bar1").rect.w();
    let bar_y = art.m.sprite("enemy_HP/HP_bar1").rect;
    let mut gone = Vec::new();
    for (e, t) in &mut tracking.0 {
        let Ok((at, hp, _)) = targets.get(*e) else {
            gone.push(*e);
            continue;
        };
        if !t.run.visible(now, hp.cur > 0.0) {
            gone.push(*e);
            continue;
        }
        t.chip.step(hp.cur, hp.max, dt, CHIP_HOLD, CHIP_DRAIN);
        if let Ok(mut n) = nodes.get_mut(t.ui.cur) {
            n.width = px(full * fraction(hp.cur, hp.max));
        }
        if let Ok(mut n) = nodes.get_mut(t.ui.chip) {
            n.width = px(full * fraction(t.chip.shown, hp.max));
        }
        if let Ok(mut text) = texts.get_mut(t.ui.damage) {
            let want = t.run.shown(now).map_or_else(String::new, |n| n.to_string());
            if text.text != want {
                text.text = want;
            }
        }
        let screen = cam
            .world_to_viewport(cam_xf, at.translation() + Vec3::Y * HEAD_LIFT)
            .ok()
            .map(|p| to_layout(p, ui_scale.0));
        if let Ok(mut v) = vis.get_mut(t.ui.root) {
            let want = if screen.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *v != want {
                *v = want;
            }
        }
        if let (Some(p), Ok(mut n)) = (screen, nodes.get_mut(t.ui.root)) {
            // The bar's middle sits on the point.
            n.left = px(p.x - (bar_y.l + bar_y.r) * 0.5);
            n.top = px(p.y - (bar_y.t + bar_y.b) * 0.5);
        }
    }
    for e in gone {
        if let Some(t) = tracking.0.remove(&e) {
            commands.entity(t.ui.root).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_capitalised_for_display() {
        assert_eq!(display_name("kobold"), "Kobold");
        assert_eq!(display_name("Kobold"), "Kobold");
        assert_eq!(display_name("kobold  laborer"), "Kobold Laborer");
        assert_eq!(display_name(""), "");
    }

    #[test]
    fn the_run_adds_blows_that_follow_each_other_and_resets_after_a_pause() {
        let mut r = Run::default();
        r.add(20.0, 5.0);
        assert_eq!(r.shown(5.2), Some(20));
        r.add(20.0, 5.9);
        assert_eq!(r.shown(6.0), Some(40));
        assert_eq!(r.shown(7.6), None, "the number goes after a pause");
        r.add(27.0, 9.0);
        assert_eq!(r.shown(9.1), Some(27), "a new run starts from the new blow");
    }

    #[test]
    fn a_living_bar_lingers_and_a_dead_one_goes_with_its_number() {
        let mut r = Run::default();
        r.add(20.0, 10.0);
        assert!(r.visible(13.0, true));
        assert!(!r.visible(14.5, true));
        assert!(r.visible(11.0, false));
        assert!(!r.visible(11.6, false));
    }

    #[test]
    fn viewport_pixels_become_layout_pixels() {
        assert_eq!(
            to_layout(Vec2::new(800.0, 450.0), 1.25),
            Vec2::new(640.0, 360.0)
        );
        assert_eq!(to_layout(Vec2::new(10.0, 10.0), 1.0), Vec2::new(10.0, 10.0));
        assert_eq!(
            to_layout(Vec2::new(10.0, 10.0), 0.0),
            Vec2::new(100.0, 100.0)
        );
    }
}
