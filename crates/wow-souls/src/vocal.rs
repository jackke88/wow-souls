//! The voices and small sounds that make the world feel inhabited, all WoW's own, streamed from the
//! install through the sound kits: the player's grunt when swinging, cry when wounded and death cry
//! (the human male's `CreatureSoundData` row); an NPC's hello when its dialogue opens and goodbye when
//! it closes (`NPCSounds`); idle creatures grunting and growling now and then (their `Fidget` kits); the
//! coin when the purse changes (`LOOTWINDOWCOINSOUND`); the bag opening and closing.
//!
//! The chances follow the reference's classes (benilla-app's `sound/combat.rs`, `greeting.rs`,
//! `money.rs`, docs/PROVENANCE.md).

use std::collections::HashMap;

use benilla_world::view::WorldCamera;
use bevy::prelude::*;
use bevy::time::Real;

use crate::actor::ActorModel;
use crate::audio::{listener, Audio, VoiceOf};
use crate::avatar::{Avatar, HUMAN_MALE_DISPLAY};
use crate::combat::{Action, ActionState};
use crate::friendly::Npc;
use crate::loot::Screen;
use crate::npc::{Brain, Hostile, Mind, PlayerHurt};
use crate::quests::{QuestGame, Talk};
use crate::settle::Phase;
use crate::sfx;

/// The class chances (percent) of the player's grunt and wound cry.
const EXERTION_CHANCE: u32 = 70;
const INJURY_CHANCE: u32 = 60;
/// How loud a voice is next to the other sounds.
const SCALE_VOICE: f32 = 0.9;
/// A creature may make an idle sound when it is within this of the listener (yd) and every so often (s).
const IDLE_HEARD_WITHIN: f32 = 35.0;
const IDLE_EVERY: (f64, f64) = (14.0, 34.0);
/// A blow this hard (damage) takes the critical wound cry.
const CRITICAL_WOUND: f32 = 30.0;

const BAG_OPEN: &str = "Sound\\interface\\PickUp\\PickUpBag.wav";
const BAG_CLOSE: &str = "Sound\\interface\\PutDownBag.wav";
/// The coin sound's name in `SoundEntries`.
const COIN: &str = "LOOTWINDOWCOINSOUND";

/// The player's grunt at the start of a swing, and the cry of a blow that landed; the death cry once.
#[allow(clippy::type_complexity)]
fn player_voice(
    audio: Option<ResMut<Audio>>,
    mut hurts: MessageReader<PlayerHurt>,
    avatar: Query<&Action, With<Avatar>>,
    mut swung: Local<Option<u32>>,
    mut was_dead: Local<bool>,
) {
    let Some(mut audio) = audio else {
        hurts.clear();
        return;
    };
    let voice = audio
        .creature_voice(HUMAN_MALE_DISPLAY)
        .map(|v| (v.exertion, v.injury, v.death));
    let Some((exertion, injury, death)) = voice else {
        hurts.clear();
        return;
    };
    let Ok(action) = avatar.single() else {
        return;
    };
    // The grunt: once per swing, on the chance.
    if let ActionState::Attack(swing) = action.state {
        if *swung != Some(swing.serial) {
            *swung = Some(swing.serial);
            let roll = audio.roll();
            if sfx::chance(EXERTION_CHANCE, roll) {
                let kit = exertion[usize::from(swing.spec.mult >= 1.2)];
                audio.play_kit(kit, None, SCALE_VOICE, "player grunt");
            }
        }
    }
    // The wound cry of each blow that was not blocked, and the death cry when it kills.
    let dead = action.is_dead();
    for h in hurts.read() {
        if h.blocked || dead {
            continue;
        }
        let roll = audio.roll();
        if sfx::chance(INJURY_CHANCE, roll) {
            let kit = injury[usize::from(h.damage >= CRITICAL_WOUND)];
            audio.play_kit(kit, None, SCALE_VOICE, "player wound cry");
        }
    }
    if dead && !*was_dead {
        audio.play_kit(death, None, SCALE_VOICE, "player death cry");
    }
    *was_dead = dead;
}

/// An NPC greets when its dialogue opens and says goodbye when it closes.
fn greetings(
    audio: Option<ResMut<Audio>>,
    talk: Res<Talk>,
    npcs: Query<(&Npc, &ActorModel, &GlobalTransform)>,
    cams: Query<&GlobalTransform, With<WorldCamera>>,
    mut with: Local<Option<u32>>,
) {
    let now = talk.creature();
    if now == *with {
        return;
    }
    let (before, after) = (*with, now);
    *with = now;
    let Some(mut audio) = audio else {
        return;
    };
    let l = listener(&cams);
    let say = |audio: &mut Audio, entry: u32, goodbye: bool| {
        let Some((npc, model, at)) = npcs.iter().find(|(n, _, _)| n.entry == entry) else {
            return;
        };
        let _ = npc;
        let Some((hello, bye)) = audio.greeting(model.display) else {
            return;
        };
        let kit = if goodbye { bye } else { hello };
        let p = at.translation() + Vec3::Y * 1.5;
        audio.play_kit(
            kit,
            l.map(|l| (p, l)),
            SCALE_VOICE,
            if goodbye { "goodbye" } else { "greeting" },
        );
    };
    if let Some(entry) = before {
        say(&mut audio, entry, true);
    }
    if let Some(entry) = after {
        say(&mut audio, entry, false);
    }
}

/// Idle creatures make their own sounds now and then: a grunt, a growl, a chitter.
fn idle_creatures(
    audio: Option<ResMut<Audio>>,
    real: Res<Time<Real>>,
    cams: Query<&GlobalTransform, With<WorldCamera>>,
    foes: Query<(Entity, &Brain, &GlobalTransform, &VoiceOf), With<Hostile>>,
    mut next: Local<HashMap<Entity, f64>>,
) {
    let (Some(mut audio), Some(l)) = (audio, listener(&cams)) else {
        return;
    };
    let now = real.elapsed_secs_f64();
    for (e, brain, at, display) in &foes {
        let p = at.translation();
        if !matches!(brain.mind, Mind::Idle) || (p - l.pos).length() > IDLE_HEARD_WITHIN {
            continue;
        }
        let due = *next.entry(e).or_insert_with(|| {
            // The first one after a few seconds, not all together.
            now + 4.0 + f64::from(audio.roll() % 10_000) / 10_000.0 * IDLE_EVERY.1
        });
        if now < due {
            continue;
        }
        let fidgets: Vec<u32> = audio
            .creature_voice(display.0)
            .map(|v| v.fidget.iter().copied().filter(|k| *k != 0).collect())
            .unwrap_or_default();
        if !fidgets.is_empty() {
            let kit = fidgets[audio.roll() as usize % fidgets.len()];
            audio.play_kit(kit, Some((p + Vec3::Y * 0.6, l)), SCALE_VOICE * 0.8, "idle");
        }
        let gap = IDLE_EVERY.0
            + f64::from(audio.roll() % 10_000) / 10_000.0 * (IDLE_EVERY.1 - IDLE_EVERY.0);
        next.insert(e, now + gap);
    }
    if next.len() > 256 {
        next.retain(|e, _| foes.contains(*e));
    }
}

/// The coin when the purse changes, either way.
fn coins(audio: Option<ResMut<Audio>>, game: Option<Res<QuestGame>>, mut last: Local<Option<i64>>) {
    let Some(game) = game else {
        return;
    };
    let before = last.replace(game.money);
    if before.is_some_and(|b| b != game.money) {
        if let Some(mut audio) = audio {
            audio.play_named(COIN, 0.8, "coin");
        }
    }
}

/// The bag opens and closes with the inventory.
fn bag(audio: Option<ResMut<Audio>>, screen: Res<Screen>, mut was: Local<bool>) {
    let open = screen.open;
    if open != *was {
        *was = open;
        if let Some(mut audio) = audio {
            audio.play_path(if open { BAG_OPEN } else { BAG_CLOSE }, 0.8, "bag");
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (player_voice, greetings, idle_creatures, coins, bag).run_if(in_state(Phase::Playing)),
    );
}
