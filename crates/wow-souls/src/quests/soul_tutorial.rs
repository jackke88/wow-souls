//! The authored soul-binding tutorial, separate from vMaNGOS's imported quests and scripts.
//! Falkhaan gives both this and Rest and Relaxation; the inn quest remains independent.

use bevy::prelude::*;
use wow_world::model::{Relation, World};
use wow_world::quest::{Event, Status};

use super::{report, QuestGame, Toasts};
use crate::soulbind::{Binding, SoulBound};

pub const QUEST: u32 = 9_000_101;
pub const MEMORIAL: u32 = 9_000_101;
const FALKHAAN: u32 = 6774;
const INN_QUEST: u32 = 2158;

/// Guide an accepted, unfinished objective to its actual Northshire stone. Once binding
/// completes it, the ordinary return marker leads back to Falkhaan instead.
pub(crate) fn target<'a>(
    game: Option<&QuestGame>,
    binding: &'a Binding,
) -> Option<&'a crate::soulbind::Site> {
    (game?.log.status(QUEST) == Some(&Status::Incomplete))
        .then(|| binding.sites.iter().find(|s| s.name == "Northshire Valley"))
        .flatten()
}

/// Runtime overlay: never rewrite the source DB, imported manifest, or inn quest.
pub(super) fn add(world: &mut World) {
    if world.quests.iter().any(|q| q.id == QUEST) {
        return;
    }
    let Some(mut quest) = world.quests.iter().find(|q| q.id == INN_QUEST).cloned() else {
        return;
    };
    let Some(site) = crate::soulbind::sites(world)
        .into_iter()
        .find(|s| s.name == "Northshire Valley")
    else {
        return;
    };
    quest.id = QUEST;
    quest.title = "Bind Your Soul".into();
    quest.details = "Before you take the road to Goldshire, give your soul a place to return to, $N.$B$BA memorial stone stands in Northshire's graveyard, beside the abbey. Approach it and choose Bind Soul. Should you fall, you will awaken at the last stone you bound yourself to.$B$BThe souls you carry will remain where you fell. Return there to reclaim them.$B$BBind your soul, then come back to me.".into();
    quest.objectives = "Bind your soul at the memorial stone in Northshire's graveyard, then return to Falkhaan Isenstrider.".into();
    quest.offer_reward = "Your soul has an anchor now, $N.$B$BWhen you find another graveyard, you may bind yourself to its memorial instead. Your last binding will always be the place you return to.$B$BNow you are ready for the road to Goldshire.".into();
    quest.request_items = "Visit the memorial stone in Northshire's graveyard and choose Bind Soul. Walking past the stone will not bind you.".into();
    quest.end_text.clear();
    quest.objective_text = [
        "Soul bound at a memorial stone".into(),
        String::new(),
        String::new(),
        String::new(),
    ];
    quest.zone_or_sort = 9;
    quest.min_level = 1;
    quest.level = 1;
    quest.reward_xp = 40;
    quest.reward_money = 0;
    quest.reward_choice.clear();
    quest.reward_items.clear();
    quest.req_items.clear();
    quest.req_kills = vec![(-(MEMORIAL as i32), 1)];
    quest.req_spell_cast.clear();
    quest.prev = 0;
    quest.next = 0;
    quest.next_in_chain = 0;
    quest.exclusive_group = 0;
    quest.start_script = 0;
    quest.complete_script = 0;
    quest.reward_spell = 0;
    quest.src_item = 0;
    quest.src_item_count = 0;
    let wow = benilla_assets::coords::bevy_to_wow(site.at);
    quest.point = (0, wow[0], wow[1]);
    world.quests.push(quest);
    let relation = Relation {
        npc: FALKHAAN,
        quest: QUEST,
        object: false,
    };
    world.starts.push(relation);
    world.ends.push(relation);
}

pub(super) fn credit(game: &mut QuestGame, toasts: &mut Toasts, now: f32) {
    if game.log.status(QUEST) != Some(&Status::Incomplete) {
        return;
    }
    let events = game.log.use_object(&game.book, MEMORIAL);
    if !events.is_empty() {
        // The authored objective should read as soul binding, not a generic object use.
        for event in &events {
            if matches!(event, Event::Progress { quest: QUEST, .. }) {
                toasts.sounds.push(super::CHIME_UPDATE);
            } else {
                report(toasts, now, &game.book, std::slice::from_ref(event));
            }
        }
        game.save();
        info!("soulbind: tutorial objective complete");
    }
}

pub(crate) fn sync(
    binding: Res<Binding>,
    mut events: MessageReader<SoulBound>,
    mut game: Option<ResMut<QuestGame>>,
    mut toasts: ResMut<Toasts>,
    real: Res<Time<Real>>,
) {
    for event in events.read() {
        debug!("soulbind: tutorial received binding at {}", event.site);
    }
    if binding.site().is_some() {
        if let Some(game) = game.as_deref_mut() {
            credit(game, &mut toasts, real.elapsed_secs());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::quest::{Class, Npc, Player, QuestBook, QuestLog};

    #[test]
    fn soul_tutorial_requires_binding_before_falkhaan_can_take_it() {
        let Some(mut world) = super::super::testing::real_world() else {
            return;
        };
        let original = world
            .quests
            .iter()
            .find(|q| q.id == INN_QUEST)
            .unwrap()
            .clone();
        add(&mut world);
        add(&mut world);
        assert_eq!(world.quests.iter().filter(|q| q.id == QUEST).count(), 1);
        assert_eq!(
            world.quests.iter().find(|q| q.id == INN_QUEST),
            Some(&original)
        );
        let book = QuestBook::new(&world);
        for class in [Class::Warrior, Class::Mage] {
            assert!(book.playable_for(QUEST, class));
        }
        assert!(book.by_giver(FALKHAAN, false).contains(&INN_QUEST));
        assert!(book.by_giver(FALKHAAN, false).contains(&QUEST));
        let mut log = QuestLog::default();
        let player = Player::warrior("Jacob", 1);
        log.accept(&book, &player, QUEST, &|_| 0).unwrap();
        assert_eq!(log.status(QUEST), Some(&Status::Incomplete));
        assert!(log
            .turn_in(&book, &player, QUEST, Npc::Creature(FALKHAAN), None, &|_| 0)
            .is_err());
        log.use_object(&book, MEMORIAL);
        assert_eq!(log.status(QUEST), Some(&Status::Complete));
        let restored = wow_world::quest::save::SaveGame::from_text(
            &wow_world::quest::save::SaveGame { log, ..default() }.to_text(),
        )
        .unwrap();
        let mut log = restored.log;
        assert_eq!(log.status(QUEST), Some(&Status::Complete));
        let events = log
            .turn_in(&book, &player, QUEST, Npc::Creature(FALKHAAN), None, &|_| 0)
            .unwrap();
        assert!(matches!(&events[0], Event::TurnedIn { rewards, .. } if rewards.xp == 40));
        assert!(log.is_rewarded(QUEST));
        assert!(log.accept(&book, &player, QUEST, &|_| 0).is_err());
    }

    #[test]
    fn soul_tutorial_counts_a_previously_saved_binding_after_acceptance() {
        let Some(mut world) = super::super::testing::real_world() else {
            return;
        };
        add(&mut world);
        let mut game = super::super::tests::game_of(&world);
        game.log
            .accept(&game.book, &game.player, QUEST, &|_| 0)
            .unwrap();
        let mut binding = Binding::default();
        binding.sites = vec![crate::soulbind::Site {
            id: 87047,
            name: "Northshire Valley".into(),
            at: Vec3::ZERO,
            yaw: 0.0,
        }];
        assert!(target(None, &binding).is_none());
        assert_eq!(target(Some(&game), &binding).map(|s| s.id), Some(87047));
        let mut app = App::new();
        app.insert_resource(game)
            .insert_resource(binding)
            .insert_resource(Time::<Real>::default())
            .init_resource::<Toasts>()
            .add_message::<SoulBound>()
            .add_systems(Update, sync);
        app.update();
        assert_eq!(
            app.world().resource::<QuestGame>().log.status(QUEST),
            Some(&Status::Incomplete)
        );
        app.world_mut().resource_mut::<Binding>().bound = Some(87047);
        app.update();
        assert_eq!(
            app.world().resource::<QuestGame>().log.status(QUEST),
            Some(&Status::Complete)
        );
        assert!(
            target(
                Some(app.world().resource::<QuestGame>()),
                app.world().resource::<Binding>(),
            )
            .is_none(),
            "binding removes the objective marker; Falkhaan is now the return target"
        );
        let count = app.world().resource::<Toasts>().lines.len();
        app.update();
        assert_eq!(app.world().resource::<Toasts>().lines.len(), count);
    }

    #[test]
    fn tutorial_guidance_requires_an_accepted_quest_and_stops_after_abandonment() {
        let Some(mut world) = super::super::testing::real_world() else {
            return;
        };
        add(&mut world);
        let mut game = super::super::tests::game_of(&world);
        let mut binding = Binding::default();
        binding.sites = crate::soulbind::sites(&world);
        assert!(target(Some(&game), &binding).is_none());
        game.log
            .accept(&game.book, &game.player, QUEST, &|_| 0)
            .unwrap();
        assert_eq!(
            target(Some(&game), &binding).unwrap().name,
            "Northshire Valley"
        );
        game.log.abandon(QUEST);
        assert!(target(Some(&game), &binding).is_none());
    }
}
