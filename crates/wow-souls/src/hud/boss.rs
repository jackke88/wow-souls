//! The boss bar in the Dark Souls style: the layout's own `F20_boss_HP`, a wide bar at the bottom
//! centre with the boss's name above its left end and the damage just dealt at its right, drawn
//! while a creature of a boss species ([`crate::species::Boss`]) fights. The dark red fill is its
//! health; behind it the olive chip keeps what the last blow took off for a moment. The bar goes
//! when the boss loses interest, and a moment after it dies (with its last damage number).
//!
//! Without the layout in the HUD cache (an old `wow-souls-dsr hud` run) there is no boss bar.

use bevy::prelude::*;
use bevy::time::Real;

use super::art::HudArt;
use super::bars::{fraction, Chip};
use super::enemy::{display_name, Run};
use super::font::{Align, BitmapText};
use super::{clip_sprite, node_at, sprite_node, Anchor, LAYOUT_H, LAYOUT_W};
use crate::combat::Health;
use crate::hits::Hit;
use crate::npc::Hostile;
use crate::settle::Phase;
use crate::species::Boss;

/// The chip holds this long (s) after a blow, then drains this fast (of the maximum per s).
const CHIP_HOLD: f32 = 0.6;
const CHIP_DRAIN: f32 = 0.9;
/// A dead boss's bar stays this long (s) after its last blow.
const DEATH_LINGER: f32 = 2.5;

/// The keys of the layout this widget needs: all of them, or no bar.
const KEYS: &[&str] = &[
    "F20_boss_HP/\u{FF22}\u{FF27}",
    "F20_boss_HP/HP_bar0",
    "F20_boss_HP/HP_bar1",
    "F20_boss_HP/frame01",
    "F20_boss_HP/frame02",
    "F20_boss_HP/boss_bar_memori4",
];
const BG: &str = "F20_boss_HP/\u{FF22}\u{FF27}";

/// Whether the HUD cache carries the boss layout.
pub fn available(art: &HudArt) -> bool {
    KEYS.iter().all(|k| art.m.sprites.contains_key(*k))
        && art.m.texts.contains_key("F20_boss_HP/text_boss_name")
        && art.m.texts.contains_key("F20_boss_HP/text_param_damage")
}

/// The pieces of the bar.
#[derive(Resource, Debug, Clone, Copy)]
struct BossUi {
    root: Entity,
    chip: Entity,
    cur: Entity,
    name: Entity,
    damage: Entity,
}

/// What the bar follows: the boss, its damage run and its chip.
#[derive(Resource, Debug, Default)]
struct Follow {
    boss: Option<Entity>,
    run: Run,
    chip: Option<Chip>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Follow>()
        .add_systems(OnEnter(Phase::Playing), spawn)
        .add_systems(
            Update,
            update
                .run_if(in_state(Phase::Playing))
                .run_if(resource_exists::<BossUi>),
        );
}

fn spawn(mut commands: Commands, art: Option<Res<HudArt>>) {
    let Some(art) = art else {
        return;
    };
    if !available(&art) {
        info!("hud: the HUD cache has no boss bar (run `wow-souls-dsr hud`)");
        return;
    }
    // A layout-sized box in the middle of the window: the layout's own rectangles sit inside it.
    let root = commands
        .spawn((
            Name::new("boss bar"),
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.0),
                margin: UiRect::left(px(-LAYOUT_W / 2.0)),
                top: px(0),
                width: px(LAYOUT_W),
                height: px(LAYOUT_H),
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    let (node, image) = sprite_node(&art, BG, Anchor::Left);
    commands.spawn((node, image, ChildOf(root)));
    let full = art.m.sprite("F20_boss_HP/HP_bar1").rect.w();
    let chip = clip_sprite(&mut commands, root, &art, "F20_boss_HP/HP_bar0", full);
    let cur = clip_sprite(&mut commands, root, &art, "F20_boss_HP/HP_bar1", full);
    for key in [
        "F20_boss_HP/frame02",
        "F20_boss_HP/frame01",
        "F20_boss_HP/boss_bar_memori4",
    ] {
        let (node, image) = sprite_node(&art, key, Anchor::Left);
        commands.spawn((node, image, ChildOf(root)));
    }
    let nm = art.m.text("F20_boss_HP/text_boss_name");
    let name = commands
        .spawn((
            node_at(nm.rect, Anchor::Left),
            BitmapText::new("", nm.rect.h(), Align::Left, nm.rect.w(), nm.color),
            ChildOf(root),
        ))
        .id();
    let dm = art.m.text("F20_boss_HP/text_param_damage");
    let damage = commands
        .spawn((
            node_at(dm.rect, Anchor::Left),
            BitmapText::new("", dm.rect.h(), Align::Right, dm.rect.w(), dm.color),
            ChildOf(root),
        ))
        .id();
    commands.insert_resource(BossUi {
        root,
        chip,
        cur,
        name,
        damage,
    });
}

/// Whether the bar shows: a boss that is engaged and alive, or one that died moments ago.
pub fn shows(engaged: bool, alive: bool, since_last_blow: f32) -> bool {
    if alive {
        engaged
    } else {
        since_last_blow < DEATH_LINGER
    }
}

#[allow(clippy::too_many_arguments)]
fn update(
    real: Res<Time<Real>>,
    art: Res<HudArt>,
    ui: Res<BossUi>,
    mut follow: ResMut<Follow>,
    mut hits: MessageReader<Hit>,
    bosses: Query<(Entity, &Health, &Hostile, &Name), With<Boss>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut BitmapText>,
    mut vis: Query<&mut Visibility>,
) {
    let now = real.elapsed_secs();
    let dt = real.delta_secs();
    // The boss to follow: the first that is engaged, else the one already followed.
    let engaged = bosses
        .iter()
        .find(|(_, hp, h, _)| h.alert && hp.cur > 0.0)
        .map(|(e, ..)| e);
    if let Some(e) = engaged.filter(|e| follow.boss != Some(*e)) {
        follow.boss = Some(e);
        follow.run = Run::default();
        follow.chip = bosses.get(e).ok().map(|(_, hp, ..)| Chip::new(hp.cur));
    }
    for h in hits.read() {
        if follow.boss == Some(h.target) {
            follow.run.add(h.damage, now);
        }
    }
    let shown = follow
        .boss
        .and_then(|e| bosses.get(e).ok())
        .filter(|(_, hp, h, _)| shows(h.alert, hp.cur > 0.0, now - follow.run.last_hit));
    let want = if shown.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if let Ok(mut v) = vis.get_mut(ui.root) {
        if *v != want {
            *v = want;
        }
    }
    let Some((_, hp, _, name)) = shown else {
        return;
    };
    let full = art.m.sprite("F20_boss_HP/HP_bar1").rect.w();
    let chip = follow.chip.get_or_insert_with(|| Chip::new(hp.cur));
    chip.step(hp.cur, hp.max, dt, CHIP_HOLD, CHIP_DRAIN);
    let chip_shown = chip.shown;
    if let Ok(mut n) = nodes.get_mut(ui.cur) {
        n.width = px(full * fraction(hp.cur, hp.max));
    }
    if let Ok(mut n) = nodes.get_mut(ui.chip) {
        n.width = px(full * fraction(chip_shown, hp.max));
    }
    let title = display_name(name.as_str());
    if let Ok(mut t) = texts.get_mut(ui.name) {
        if t.text != title {
            t.text = title;
        }
    }
    let number = follow
        .run
        .shown(now)
        .map_or_else(String::new, |n| n.to_string());
    if let Ok(mut t) = texts.get_mut(ui.damage) {
        if t.text != number {
            t.text = number;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_shows_while_the_boss_fights_and_a_moment_after_it_dies() {
        assert!(shows(true, true, 100.0));
        assert!(!shows(false, true, 0.0), "it lost interest");
        assert!(shows(false, false, 1.0), "just dead: the last number shows");
        assert!(!shows(false, false, 3.0), "dead for a while");
    }

    #[test]
    fn every_key_of_the_widget_is_a_layout_key() {
        for k in KEYS {
            assert!(k.starts_with("F20_boss_HP/"), "{k}");
        }
        assert!(KEYS.contains(&BG));
    }
}
