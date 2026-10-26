//! The interact prompt: when the player stands near someone to talk to (or a crate to gather), an icon
//! with the key to press shows at the bottom of the screen, as Dark Souls shows its button prompts. The
//! key is the unified interact key: `X` on the keyboard, `Y` on the pad (`T` and the D-pad up still work
//! as they always did); the icon follows whichever the player touched last.

use bevy::prelude::*;
use wow_world::quest::rules::{marks, Mark};
use wow_world::quest::Npc as QuestNpc;

use super::{flat_distance, objects, LogScreen, QuestGame, Talk, TALK_RANGE};
use crate::avatar::Avatar;
use crate::combat::{Action, Health};
use crate::friendly::Npc;
use crate::loot::{QuestLootEligibility, Screen};

/// What the interact key would do right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub verb: &'static str,
    pub name: String,
    /// A second line: what the person has for the player (a quest to take, one to hand in ...).
    pub hint: &'static str,
}

#[derive(Resource, Default)]
pub struct Interact(pub Option<Target>);

/// The icon's letter: the keyboard's interact key, or the pad's.
pub fn glyph(pad: bool) -> &'static str {
    if pad {
        "Y"
    } else {
        "X"
    }
}

/// A line for what the person offers, from the quest mark over their head.
pub fn hint_of(mark: Mark) -> &'static str {
    match mark {
        Mark::Available => "Quest available",
        Mark::AvailableLow => "Quest available (low level)",
        Mark::InProgress => "Quest in progress",
        Mark::Ready => "Quest ready to hand in",
        Mark::None => "",
    }
}

/// Whether a person is worth a prompt: a quest business or any gossip, vendor or trainer flag. A
/// peasant with nothing to say gets none.
pub fn worth_a_prompt(mark: Mark, npc_flags: u32) -> bool {
    mark != Mark::None || npc_flags != 0
}

/// The interact key's target this frame: the nearest person in talking range, or a crate that a quest
/// still wants. Nothing while a screen is up, the player is busy, or a corpse / chest has its own prompt.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn find(
    game: Option<Res<QuestGame>>,
    talk: Res<Talk>,
    log: Res<LogScreen>,
    screen: Res<Screen>,
    eligibility: Res<QuestLootEligibility>,
    objects: Res<objects::Objects>,
    avatar: Query<(&Transform, &Action, &Health), With<Avatar>>,
    npcs: Query<(&Transform, &Npc, Option<&Name>), Without<Avatar>>,
    crates: Query<(Entity, &Transform, &objects::QuestObject), Without<Avatar>>,
    (binding, mut bound_events, real, mut bound_notice_until): (
        Res<crate::soulbind::Binding>,
        MessageReader<crate::soulbind::SoulBound>,
        Res<Time<Real>>,
        Local<f32>,
    ),
    mut interact: ResMut<Interact>,
) {
    // Let the binding and quest confirmations use this space before offering another bind.
    if bound_events.read().count() > 0 {
        *bound_notice_until = real.elapsed_secs() + 5.0;
    }
    let mut want: Option<Target> = None;
    if let (Some(game), Ok((me, action, health))) = (game, avatar.single()) {
        let free = talk.0.is_none()
            && real.elapsed_secs() >= *bound_notice_until
            && !log.open
            && !screen.locked()
            && !screen.modal
            && screen.prompt.is_empty()
            && health.cur > 0.0
            && !action.is_dead()
            && !action.owns_body();
        if free {
            want = crate::soulbind::nearest(&binding.sites, me.translation)
                .map(|site| Target {
                    verb: "Bind Soul",
                    name: format!("{} Memorial", site.name),
                    hint: if binding.bound == Some(site.id) {
                        "Your soul is bound here"
                    } else {
                        "Return here when you die"
                    },
                })
                .or_else(|| nearest_person(&game, me.translation, &npcs))
                .or_else(|| {
                    objects::nearest_talkable(&crates, &objects, &game, me.translation).and_then(
                        |(_, idx, mark)| {
                            let spot = objects.spots.get(idx)?;
                            Some(Target {
                                verb: "Talk",
                                name: spot.name.clone(),
                                hint: hint_of(mark),
                            })
                        },
                    )
                })
                .or_else(|| {
                    objects::nearest_gatherable(&crates, &objects, me.translation, &eligibility.0)
                        .and_then(|(_, idx)| {
                            let spot = objects.spots.get(idx)?;
                            let name = if spot.entry == objects::CRATE_ENTRY {
                                "Grape crate".to_string()
                            } else {
                                spot.name.clone()
                            };
                            Some(Target {
                                verb: "Gather",
                                name,
                                hint: "",
                            })
                        })
                });
        }
    }
    if interact.0 != want {
        interact.0 = want;
    }
}

fn nearest_person(
    game: &QuestGame,
    me: Vec3,
    npcs: &Query<(&Transform, &Npc, Option<&Name>), Without<Avatar>>,
) -> Option<Target> {
    npcs.iter()
        .map(|(xf, n, name)| (flat_distance(me, xf.translation), n, name))
        .filter(|(d, _, _)| *d <= TALK_RANGE)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .and_then(|(_, n, name)| {
            let mark = marks(
                &game.book,
                &game.log,
                &game.player,
                QuestNpc::Creature(n.entry),
            );
            // A vendor, banker or trainer with no quest business is dealt with, not talked to.
            let (verb, hint) = match mark {
                Mark::None if n.flags & 16 != 0 => ("Train", "Abilities for souls"),
                Mark::None if crate::shop::BANKERS.contains(&n.entry) => {
                    ("Trade", "Wares and the strongbox")
                }
                Mark::None if n.flags & 4 != 0 => ("Trade", "Wares for souls"),
                _ => ("Talk", hint_of(mark)),
            };
            worth_a_prompt(mark, n.flags).then(|| Target {
                verb,
                name: name.map_or_else(|| "Stranger".to_string(), |n| n.as_str().to_string()),
                hint,
            })
        })
}

/// Which device the icon shows: the last one the player pressed something on.
#[derive(Resource, Default)]
pub struct PadUsed(pub bool);

pub(super) fn track_device(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut used: ResMut<PadUsed>,
) {
    let pad = pads
        .iter()
        .any(|p| p.get_just_pressed().next().is_some() || p.left_stick().length() > 0.5);
    let key = keys.get_just_pressed().next().is_some();
    let want = if pad {
        true
    } else if key {
        false
    } else {
        used.0
    };
    if used.0 != want {
        used.0 = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_names_the_key_of_the_device_in_hand() {
        assert_eq!(glyph(false), "X");
        assert_eq!(glyph(true), "Y");
    }

    #[test]
    fn only_a_person_with_business_gets_a_prompt() {
        assert!(
            !worth_a_prompt(Mark::None, 0),
            "a peasant with nothing to say"
        );
        assert!(worth_a_prompt(Mark::Available, 0));
        assert!(worth_a_prompt(Mark::Ready, 0));
        assert!(
            worth_a_prompt(Mark::None, 1),
            "a gossip, vendor or trainer flag"
        );
    }

    #[test]
    fn the_hint_follows_the_quest_mark() {
        assert_eq!(hint_of(Mark::Ready), "Quest ready to hand in");
        assert_eq!(hint_of(Mark::None), "");
        assert!(hint_of(Mark::AvailableLow).contains("low level"));
    }
}
