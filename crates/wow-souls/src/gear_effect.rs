//! Worn gear in a fight: the [`GearFx`] resource the damage hooks read (`hits.rs`, `projectiles.rs`,
//! `npc.rs`), kept current from the items save and the item table. The rules are
//! `wow_world::gear_effect`; the spec is `docs/superpowers/specs/2026-10-12-gear-effects-design.md`.
//!
//! `WOWSOULS_GEAR_EFFECT=0` makes gear do nothing in a fight (scripted checks that count exact damage).

use bevy::prelude::*;
use wow_world::gear_effect::{GearEffect, DISPLAY_LEVEL};

use crate::loot::{Data, Owned};
use crate::quests::QuestGame;

/// The character the class tables and the worn gear make (`None` without class tables).
pub fn character(
    game: &QuestGame,
    owned: &Owned,
    data: &Data,
) -> Option<wow_world::attributes::Character> {
    wow_world::attributes::character(
        &data.0.level_stats,
        &data.0.class_stats,
        game.player.class as u32,
        game.player.level,
        &owned.progress.gear,
        &data.0.items,
    )
}

/// What the avatar's gear does now. Neutral until the save and the item table are loaded.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct GearFx(pub GearEffect);

/// Whether gear counts (`WOWSOULS_GEAR_EFFECT`, default yes).
#[derive(Resource, Debug, Clone, Copy)]
struct Enabled(bool);

/// `WOWSOULS_GEAR_EFFECT=0` is the only value that turns gear off.
fn switch_on(value: Option<&str>) -> bool {
    !value.is_some_and(|v| v.trim() == "0")
}

/// Whether the off hand is actually held. A two-handed grip and a drawn bow put the shield away (the
/// body shows none, `equip::apply`), and a shield that is put away neither guards nor protects.
pub fn off_hand_in_use(two_handed: bool, ranged: bool) -> bool {
    !(two_handed || ranged)
}

/// [`off_hand_in_use`] for the held weapon's moveset (no moveset yet: the sword and shield stance).
pub fn off_hand_held(moves: Option<&crate::dsclip::DsMoves>) -> bool {
    moves.is_none_or(|m| off_hand_in_use(m.two_handed, m.projectile.is_some()))
}

/// Recomputes the effect when the worn gear, the item table, the stance or the switch changed, and
/// says so.
fn refresh(
    owned: Option<Res<Owned>>,
    data: Option<Res<Data>>,
    game: Option<Res<QuestGame>>,
    moves: Option<Res<crate::dsclip::DsMoves>>,
    enabled: Res<Enabled>,
    mut fx: ResMut<GearFx>,
) {
    let (Some(owned), Some(data)) = (owned, data) else {
        return;
    };
    if !owned.is_changed()
        && !data.is_changed()
        && !enabled.is_changed()
        && !game.as_ref().is_some_and(|g| g.is_changed())
        && !moves.as_ref().is_some_and(|m| m.is_changed())
    {
        return;
    }
    let held = off_hand_held(moves.as_deref());
    let mut now = if enabled.0 {
        GearEffect::from_gear(&owned.progress.gear, &data.0.items, held)
    } else {
        GearEffect::default()
    };
    if enabled.0 {
        if let Some(c) = game.as_deref().and_then(|g| character(g, &owned, &data)) {
            now = now.with_character(&c);
        }
    }
    if fx.0 != now {
        fx.0 = now;
        info!(
            "gear: armor {} ({:.0} % off a level {DISPLAY_LEVEL} blow), melee x{:.2}, ranged x{:.2}, block {}{}{}",
            now.armor,
            now.reduction(DISPLAY_LEVEL) * 100.0,
            now.melee_factor,
            now.ranged_factor,
            now.block,
            if held { "" } else { " (shield stowed)" },
            if enabled.0 { "" } else { " (switched off)" }
        );
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<GearFx>()
        .insert_resource(Enabled(switch_on(
            std::env::var("WOWSOULS_GEAR_EFFECT").ok().as_deref(),
        )))
        .add_systems(Update, refresh);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_explicit_zero_turns_gear_off() {
        assert!(switch_on(None));
        assert!(switch_on(Some("1")));
        assert!(switch_on(Some("")));
        assert!(!switch_on(Some("0")));
        assert!(!switch_on(Some(" 0 ")));
    }

    #[test]
    fn the_off_hand_is_in_use_unless_two_handed_or_ranged() {
        assert!(off_hand_in_use(false, false));
        assert!(
            !off_hand_in_use(true, false),
            "a two-handed grip stows the shield"
        );
        assert!(!off_hand_in_use(false, true), "a drawn bow stows it too");
        assert!(!off_hand_in_use(true, true));
    }

    #[test]
    fn without_tables_there_is_no_character() {
        let none = wow_world::attributes::character(
            &[],
            &[],
            1,
            3,
            &wow_world::equipment::Gear::default(),
            &[],
        );
        assert!(none.is_none());
    }

    #[test]
    fn the_resource_starts_neutral() {
        let fx = GearFx::default().0;
        assert_eq!((fx.armor, fx.block), (0, 0));
        assert_eq!((fx.melee_factor, fx.ranged_factor), (1.0, 1.0));
    }
}
