//! What a level is worth. Quest XP and kill XP both feed the one total the quest log keeps
//! ([`QuestGame`]); the level it reaches sets the player's maximum health and stamina. The HUD bars take
//! their length from those maxima, so a level shows as a longer bar without any drawing code here.

use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::combat::{Health, Stamina};
use crate::quests::QuestGame;

/// Maximum health of a level-1 player and what each further level adds.
pub const HEALTH_BASE: f32 = 100.0;
pub const HEALTH_PER_LEVEL: f32 = 10.0;
/// The same for stamina.
pub const STAMINA_BASE: f32 = 100.0;
pub const STAMINA_PER_LEVEL: f32 = 5.0;

/// `(maximum health, maximum stamina)` of a level (level 0 reads as level 1).
pub fn vitals(level: u32) -> (f32, f32) {
    let up = level.max(1) - 1;
    (
        HEALTH_BASE + HEALTH_PER_LEVEL * up as f32,
        STAMINA_BASE + STAMINA_PER_LEVEL * up as f32,
    )
}

/// The stamina bar of a level (a Dark Souls bar, WoW has none): 100 + 5 per level.
pub fn max_stamina(level: u32) -> f32 {
    STAMINA_BASE + STAMINA_PER_LEVEL * (level.max(1) - 1) as f32
}

/// Brings the maxima to `hp_max` / `st_max`. The first time (`first`, the avatar was just built from a
/// save or a fresh start) the player is full; after that the current values move by the same difference
/// as the maxima, so a level-up or a new piece of gear is a longer bar, not a full heal. A dead player
/// (no health) stays dead; a living one is never taken below 1 by a smaller maximum.
///
/// What the floor at 1 takes away is kept in `debt` and paid back when the maximum grows again, so taking
/// stamina gear off and putting it back is neither a loss nor a free heal. A full bar owes nothing.
pub fn follow(
    hp_max: f32,
    st_max: f32,
    first: bool,
    debt: &mut f32,
    hp: &mut Health,
    st: &mut Stamina,
) {
    if first {
        *hp = Health::full(hp_max);
        *st = Stamina::full(st_max);
        *debt = 0.0;
        return;
    }
    if hp.cur >= hp.max {
        *debt = 0.0;
    }
    if hp.cur > 0.0 {
        let raw = hp.cur + hp_max - hp.max - *debt;
        if raw < 1.0 {
            hp.cur = 1.0;
            *debt = 1.0 - raw;
        } else {
            hp.cur = raw.min(hp_max);
            *debt = 0.0;
        }
    }
    hp.max = hp_max;
    st.cur = (st.cur + st_max - st.max).clamp(0.0, st_max);
    st.max = st_max;
}

/// WoW's own level-up chime, from the install's `sound` archive.
const LEVEL_UP_SOUND: &str = "Sound\\interface\\LevelUp.wav";

/// Whether `level` is a rise over the one already applied (the first application, a save loaded at some
/// level, is not a rise: nobody dings on loading).
pub fn is_rise(applied: Option<u32>, level: u32) -> bool {
    applied.is_some_and(|before| level > before)
}

/// Applies the quest log's level to the avatar whenever it changes; a rise flashes WoW's level-up
/// effect on the player and sounds its chime.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn follow_level(
    mut commands: Commands,
    game: Option<Res<QuestGame>>,
    owned: Option<Res<crate::loot::Owned>>,
    data: Option<Res<crate::loot::Data>>,
    mut applied: Local<Option<(u32, f32)>>,
    mut debt: Local<f32>,
    art: Option<Res<crate::loot::vfx::Art>>,
    audio: Option<ResMut<crate::audio::Audio>>,
    mut mana: ResMut<crate::abilities::Mana>,
    mut avatar: Query<(Entity, &mut Health, &mut Stamina), With<Avatar>>,
) {
    let Some(game) = game else {
        return;
    };
    let level = game.player.level;
    // From the class tables and the worn gear; without them (an older manifest) today's fixed vitals.
    let character = match (owned.as_deref(), data.as_deref()) {
        (Some(o), Some(d)) => crate::gear_effect::character(&game, o, d),
        _ => None,
    };
    // The mana pool follows the class tables, Intellect and Spirit every frame (cheap, and a piece of
    // gear changes it without changing the level).
    if let Some(c) = &character {
        mana.sync(c.max_mana, c.total.spirit as f32);
    }
    let (hp_max, st_max) = match &character {
        Some(c) => (c.max_health, max_stamina(level)),
        None => vitals(level),
    };
    if *applied == Some((level, hp_max)) {
        return;
    }
    let Ok((me, mut hp, mut st)) = avatar.single_mut() else {
        return;
    };
    let rise = is_rise(applied.map(|a| a.0), level);
    follow(
        hp_max,
        st_max,
        applied.is_none(),
        &mut debt,
        &mut hp,
        &mut st,
    );
    info!(
        "level: {level} -> health {hp_max}, stamina {st_max}{}",
        if character.is_some() {
            ""
        } else {
            " (no class tables: fixed vitals)"
        }
    );
    *applied = Some((level, hp_max));
    if rise {
        let flash = art
            .as_deref()
            .and_then(|a| crate::loot::vfx::spawn_level_up(&mut commands, me, a));
        info!(
            "level: ding, {}",
            if flash.is_some() {
                "WoW's level-up flash"
            } else {
                "no flash (no effect model yet)"
            }
        );
        if let Some(mut audio) = audio {
            audio.play_path(LEVEL_UP_SOUND, 1.0, "level up");
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, follow_level);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_rise_over_an_applied_level_dings() {
        assert!(!is_rise(None, 3), "a save loaded at level 3 does not ding");
        assert!(!is_rise(Some(3), 3));
        assert!(is_rise(Some(1), 2));
        assert!(!is_rise(Some(4), 2), "a fall is no ding");
    }

    #[test]
    fn a_level_adds_ten_health_and_five_stamina() {
        assert_eq!(vitals(1), (100.0, 100.0));
        assert_eq!(vitals(2), (110.0, 105.0));
        assert_eq!(vitals(5), (140.0, 120.0));
        assert_eq!(vitals(0), vitals(1), "level 0 reads as level 1");
    }

    #[test]
    fn the_first_application_fills_the_player() {
        let mut hp = Health::full(100.0);
        let mut st = Stamina::full(100.0);
        hp.cur = 40.0;
        st.cur = 10.0;
        follow(120.0, 110.0, true, &mut 0.0, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (120.0, 120.0));
        assert_eq!((st.cur, st.max), (110.0, 110.0));
    }

    #[test]
    fn a_change_of_maxima_moves_the_current_values_by_the_same_difference() {
        let mut hp = Health::full(100.0);
        let mut st = Stamina::full(100.0);
        hp.cur = 40.0;
        st.cur = 30.0;
        follow(120.0, 110.0, false, &mut 0.0, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (60.0, 120.0), "40 + 20, no full heal");
        assert_eq!((st.cur, st.max), (40.0, 110.0), "30 + 10");
    }

    #[test]
    fn a_fall_in_the_maximum_never_kills_and_a_full_player_stays_full() {
        let mut hp = Health::full(100.0);
        let mut st = Stamina::full(100.0);
        hp.cur = 5.0;
        follow(60.0, 100.0, false, &mut 0.0, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (1.0, 60.0), "floored at 1 while alive");
        let mut full = Health::full(100.0);
        follow(140.0, 100.0, false, &mut 0.0, &mut full, &mut st);
        assert_eq!((full.cur, full.max), (140.0, 140.0));
    }

    #[test]
    fn a_dead_player_is_not_healed_by_a_new_maximum() {
        let mut hp = Health::full(100.0);
        let mut st = Stamina::full(100.0);
        hp.cur = 0.0;
        follow(110.0, 105.0, false, &mut 0.0, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (0.0, 110.0));
    }

    #[test]
    fn taking_stamina_gear_off_and_on_again_is_no_free_heal() {
        // Final review, Important: the floor at 1 used to swallow the part of the loss below 1, and
        // putting the gear back gave all of it back.
        let mut hp = Health::full(100.0);
        let mut st = Stamina::full(100.0);
        let mut debt = 0.0;
        hp.cur = 10.0;
        follow(60.0, 100.0, false, &mut debt, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (1.0, 60.0), "floored at 1 while alive");
        assert_eq!(debt, 31.0, "10 + (60 - 100) = -30, so 31 below the floor");
        follow(100.0, 100.0, false, &mut debt, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (10.0, 100.0), "back where it was");
        assert_eq!(debt, 0.0);
    }

    #[test]
    fn a_heal_taken_while_in_debt_is_kept() {
        let mut hp = Health::full(60.0);
        let mut st = Stamina::full(100.0);
        let mut debt = 31.0;
        hp.cur = 31.0; // 1 / 60 and then a 30 point Salve
        follow(100.0, 100.0, false, &mut debt, &mut hp, &mut st);
        assert_eq!(hp.cur, 40.0, "10 + 30, not 71");
        assert_eq!(debt, 0.0);
    }

    #[test]
    fn full_health_owes_nothing() {
        let mut hp = Health::full(60.0);
        let mut st = Stamina::full(100.0);
        let mut debt = 5.0;
        follow(100.0, 100.0, false, &mut debt, &mut hp, &mut st);
        assert_eq!((hp.cur, hp.max), (100.0, 100.0), "a full bar grows full");
        assert_eq!(debt, 0.0);
    }

    #[test]
    fn the_stamina_bar_still_grows_five_a_level() {
        assert_eq!(max_stamina(1), 100.0);
        assert_eq!(max_stamina(5), 120.0);
        assert_eq!(max_stamina(0), 100.0);
    }
}
