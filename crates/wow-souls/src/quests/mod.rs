//! The quest phase (G3b): the player talks to the valley's NPCs and takes, plays and hands in WoW
//! Classic's Northshire quests. The rules are the pure engine of `wow_world::quest`; this module is its
//! game side: the log, XP and copper kept as a resource and saved after every change, kills and picked-up
//! items fed to the log, quest loot unlocked in the loot module through `QuestLootEligibility`, the
//! rewards put into the one shared inventory (`loot::Owned`), the dialogue and the quest log as modal
//! screens (`loot::Screen::modal` holds the player's attacks, evades and jumps but not their running or
//! camera: the player can walk about while talking, and a dialogue closes 7 yd away), the "!" and "?" over
//! the NPCs, the tracker and the toasts ([`ui`]).
//!
//! Keys: `X` (pad `Y`, or `T` / D-pad up) talks to the nearest NPC within 3.5 yd, an icon at the bottom
//! of the screen showing the key when someone is near ([prompt]); `L` opens the quest log; in a screen the arrows
//! choose (WASD is the player's), `Enter` confirms, `Backspace` goes back, `Delete` abandons in the log
//! (twice). The pad: D-pad up talks, right opens the log, up / down choose, A confirms, B goes back.

pub(crate) mod dialogue;
mod lines;
pub(crate) mod objects;
mod prompt;
mod rewards;
pub(crate) mod soul_tutorial;
mod ui;
mod voice;

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use bevy::prelude::*;
use wow_world::model::ItemCount;
use wow_world::quest::save::SaveGame;
use wow_world::quest::xp::{MobXp, XpTable};
use wow_world::quest::{Event, Need, Npc as QuestNpc, Player, QuestBook, QuestLog};

use crate::avatar::Avatar;
use crate::combat::{Action, Health};
use crate::critter::Critter;
use crate::friendly::Npc;
use crate::loot::{Acquired, Owned, QuestLootEligibility, Screen};
use crate::npc::Slain;
use crate::settings::binds::Action as BindAction;
use crate::settings::Controls;
use crate::settle::Phase;
use crate::world::{SpawnId, WorldBook};
use dialogue::{Dialogue, Key, Outcome};

/// How near an NPC must be to talk (yd, ground distance), and how far the player may walk off before
/// the dialogue closes.
/// How long a press a quest screen took stays the screen's (real s): longer than a frame, so the
/// systems that read the same press in any order still skip it.
const SWALLOW_SECS: f32 = 0.25;
pub const TALK_RANGE: f32 = 3.5;
pub const CLOSE_RANGE: f32 = 7.0;

#[cfg(test)]
pub(crate) mod testing {
    /// The Northshire regression fixture, independent of the active gameplay manifest.
    pub fn real_world() -> Option<wow_world::model::World> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/world/northshire.txt");
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("skipped: no manifest at {}", path.display());
            return None;
        };
        Some(wow_world::manifest::parse(&text).unwrap())
    }
}

/// The player's quests: the engine's data and the player's progress.
#[derive(Resource)]
pub struct QuestGame {
    pub book: QuestBook,
    pub table: XpTable,
    /// What killing each creature template pays, by entry.
    pub mobs: HashMap<u32, MobXp>,
    pub log: QuestLog,
    pub player: Player,
    pub xp: u32,
    /// Copper.
    pub money: i64,
    /// Souls a hand-in has earned and [`pay_out`] has not yet put in the counter (not saved).
    payout: u32,
    path: PathBuf,
    /// A save that could not be read: the game plays on without writing over it.
    blocked: Option<String>,
}

impl QuestGame {
    fn save(&mut self) {
        if self.blocked.is_some() {
            return;
        }
        let text = SaveGame {
            log: self.log.clone(),
            xp: self.xp,
            money: self.money,
            name: self.player.name.clone(),
        }
        .to_text();
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("tmp");
        if let Err(e) = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, &self.path))
        {
            warn!("quests: could not save {}: {e}", self.path.display());
        }
    }

    /// Creation happens after Startup: publish the chosen identity before any class systems run.
    pub fn set_player_character(&mut self, character: &crate::character::Character) {
        self.player.name = character.name.clone();
        self.player.class = wow_world::quest::Class::from_id(character.class.id())
            .unwrap_or(wow_world::quest::Class::Warrior);
        use crate::character::Race as Choice;
        use wow_world::quest::Race;
        self.player.race = match character.race {
            Choice::Human => Race::Human,
            Choice::Dwarf => Race::Dwarf,
            Choice::Gnome => Race::Gnome,
            Choice::NightElf => Race::NightElf,
            Choice::Orc => Race::Orc,
            Choice::Troll => Race::Troll,
            Choice::Undead => Race::Undead,
            Choice::Tauren => Race::Tauren,
        };
        self.save();
    }

    fn set_xp(&mut self, xp: u32) -> Option<u32> {
        let before = self.player.level;
        self.xp = xp;
        self.player.level = self.table.level_for(xp);
        (self.player.level > before).then_some(self.player.level)
    }

    /// A kill of creature `entry` (spawn `guid`, if it came from a spawn record) pays XP: `(XP, the
    /// creature's level, the new level if it rose)`. `None` for an unknown entry and for a kill that pays
    /// nothing (a critter, a gray creature).
    fn kill_reward(&mut self, entry: u32, guid: Option<u32>) -> Option<(u32, u32, Option<u32>)> {
        let mob = *self.mobs.get(&entry)?;
        let gain = mob.payout(self.player.level, guid);
        if gain == 0 {
            return None;
        }
        let leveled = self.set_xp(self.xp.saturating_add(gain));
        Some((gain, mob.level_for(guid), leveled))
    }
}

/// The open dialogue, if any.
#[derive(Resource, Default)]
pub struct Talk(pub Option<Dialogue>);

impl Talk {
    /// The creature entry the open dialogue is with.
    pub fn creature(&self) -> Option<u32> {
        match self.0.as_ref()?.npc {
            QuestNpc::Creature(entry) => Some(entry),
            _ => None,
        }
    }
}

/// The quest log screen.
#[derive(Resource, Default)]
pub struct LogScreen {
    pub open: bool,
    pub selected: usize,
    /// The reward of the selected quest being looked at, over its choices and then its sure rewards.
    pub reward: usize,
    /// Real second until which a second `D` abandons the selected quest.
    abandon_until: Option<f32>,
}

/// The lines that fade after a few seconds.
#[derive(Resource, Default)]
pub struct Toasts {
    pub lines: Vec<(String, f32)>,
    /// WoW's own interface sounds (chain paths) waiting to be played by [`play_chimes`].
    pub sounds: Vec<&'static str>,
}

/// WoW's quest chimes, `Sound\interface\` in the install.
const CHIME_ACCEPT: &str = "Sound\\interface\\iQuestActivate.wav";
const CHIME_UPDATE: &str = "Sound\\interface\\iQuestUpdate.wav";
const CHIME_COMPLETE: &str = "Sound\\interface\\iQuestComplete.wav";
const CHIME_LOG_OPEN: &str = "Sound\\interface\\iQuestLogOpenA.wav";
const CHIME_LOG_CLOSE: &str = "Sound\\interface\\iQuestLogCloseA.wav";

/// The chime of a quest event: accepting, an objective moving, handing in. The rest are silent.
fn chime_of(e: &Event) -> Option<&'static str> {
    match e {
        Event::Accepted { .. } => Some(CHIME_ACCEPT),
        Event::Progress { .. } => Some(CHIME_UPDATE),
        Event::TurnedIn { .. } => Some(CHIME_COMPLETE),
        Event::ReadyToTurnIn { .. } | Event::Abandoned { .. } => None,
    }
}

/// Plays the queued chimes, once each per frame however many events asked for it.
fn play_chimes(mut toasts: ResMut<Toasts>, audio: Option<ResMut<crate::audio::Audio>>) {
    if toasts.sounds.is_empty() {
        return;
    }
    let mut queued = std::mem::take(&mut toasts.sounds);
    queued.dedup();
    if let Some(mut audio) = audio {
        for path in queued {
            audio.play_path(path, 0.9, "quest chime");
        }
    }
}

const TOAST_SECS: f32 = 5.0;
const TOAST_MAX: usize = 6;

impl Toasts {
    pub fn push(&mut self, now: f32, text: impl Into<String>) {
        self.lines.push((text.into(), now + TOAST_SECS));
        let extra = self.lines.len().saturating_sub(TOAST_MAX);
        self.lines.drain(..extra);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cmd {
    Talk,
    Log,
    Up,
    Down,
    /// LB / the left arrow / `[`: the reward before the one in focus.
    Prev,
    /// RB / the right arrow / `]`: the next reward.
    Next,
    Confirm,
    Back,
    Abandon,
}

/// Unattended runs ignore live keys: `WOWSOULS_QUESTS_SCRIPT=<s>:<cmd>[:<arg>],...` presses them at real second
/// `s`. Commands: `talk`, `log`, `up`, `down`, `prev`, `next`, `confirm`, `back`, `abandon`, and two cheats for checks that
/// do not fight: `kill:<entry>x<n>` (credit kills), `give:<item>x<n>` (put items in the inventory) and
/// `xp:<n>` (add experience as a reward would).
#[derive(Resource, Default)]
struct Script {
    steps: Vec<(f32, String)>,
    next: usize,
}

fn parse_script(s: &str) -> Vec<(f32, String)> {
    let mut steps: Vec<(f32, String)> = s
        .split(',')
        .filter_map(|p| {
            let (t, rest) = p.trim().split_once(':')?;
            Some((t.trim().parse::<f32>().ok()?, rest.trim().to_string()))
        })
        .collect();
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    steps
}

fn parse_times(arg: &str) -> Option<(u32, u32)> {
    let (a, n) = arg.split_once('x')?;
    Some((a.trim().parse().ok()?, n.trim().parse().ok()?))
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Talk>()
        .init_resource::<prompt::Interact>()
        .init_resource::<prompt::PadUsed>()
        .init_resource::<LogScreen>()
        .init_resource::<Toasts>()
        .insert_resource(Script {
            steps: std::env::var("WOWSOULS_QUESTS_SCRIPT")
                .map(|s| parse_script(&s))
                .unwrap_or_default(),
            next: 0,
        })
        .add_systems(Startup, load)
        .add_systems(
            Update,
            (
                credit_kills,
                sync_items,
                drive
                    .before(crate::combat::act)
                    .before(crate::soulbind::Apply),
                pay_out,
                prompt::track_device,
                prompt::find.after(crate::soulbind::Apply),
                play_chimes,
            )
                .chain()
                .run_if(in_state(Phase::Playing)),
        )
        .add_plugins((ui::plugin, objects::plugin, voice::plugin));
}

fn save_path() -> PathBuf {
    std::env::var("WOWSOULS_QUESTS_SAVE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if crate::run_mode::unattended() {
                PathBuf::from(format!(
                    "cache/loot-check/quests-{}.txt",
                    std::process::id()
                ))
            } else {
                crate::saves::file("quests-v1.txt")
            }
        })
}

fn load(mut commands: Commands) {
    let world_path = crate::world::manifest_path().display().to_string();
    let mut world = match std::fs::read_to_string(&world_path)
        .map_err(|e| e.to_string())
        .and_then(|t| wow_world::manifest::parse(&t).map_err(|e| format!("{e:#}")))
    {
        Ok(w) => w,
        Err(e) => {
            warn!("quests: no quests ({world_path}: {e})");
            return;
        }
    };
    soul_tutorial::add(&mut world);
    let book = QuestBook::new(&world);
    let table = XpTable::from_world(&world);
    let path = save_path();
    let (save, blocked) = match std::fs::read_to_string(&path) {
        Ok(t) => match SaveGame::from_text(&t) {
            Ok(s) => (s, None),
            Err(e) => (
                SaveGame::default(),
                Some(format!("{}: {e:#}", path.display())),
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (SaveGame::default(), None),
        Err(e) => (
            SaveGame::default(),
            Some(format!("{}: {e}", path.display())),
        ),
    };
    if let Some(why) = &blocked {
        warn!("quests: the save cannot be read ({why}); playing without saving over it");
    }
    // The name and the class: the environment, else the created character's, else the old save's.
    let character = crate::character::load(&crate::character::save_path())
        .ok()
        .flatten();
    let class = character
        .as_ref()
        .and_then(|c| wow_world::quest::Class::from_id(c.class.id()))
        .unwrap_or(wow_world::quest::Class::Warrior);
    let name = std::env::var("WOWSOULS_NAME")
        .ok()
        .filter(|n| !n.trim().is_empty())
        .or(character.map(|c| c.name))
        .or_else(|| (!save.name.is_empty()).then(|| save.name.clone()))
        .unwrap_or_else(|| "Adventurer".to_string());
    let level = table.level_for(save.xp);
    info!(
        "quests: {} quests, {} playable, save {} (log {}, xp {}, level {level}, {} copper), player {name}",
        world.quests.len(),
        book.ids().filter(|&q| book.playable_for(q, class)).count(),
        path.display(),
        save.log.entries().count(),
        save.xp,
        save.money
    );
    commands.insert_resource(QuestGame {
        book,
        table,
        mobs: world
            .creatures
            .iter()
            .map(|c| (c.entry, MobXp::of(c)))
            .collect(),
        log: save.log,
        player: Player::of_class(&name, class, level),
        xp: save.xp,
        money: save.money,
        payout: 0,
        path,
        blocked,
    });
}

/// The words of a toast for an engine event, `None` for the ones that need none.
pub fn toast_text(book: &QuestBook, e: &Event) -> Option<String> {
    let title = |q: u32| book.quest(q).map_or("?", |q| q.title.as_str()).to_string();
    match e {
        Event::Accepted { quest, .. } => Some(format!("Quest accepted: {}", title(*quest))),
        Event::Progress {
            now, need, what, ..
        } => {
            let name = match what {
                Need::Kill { entry } if *entry >= 0 => format!(
                    "{} slain",
                    book.creature_name(*entry as u32).unwrap_or("Creature")
                ),
                Need::Kill { .. } => "Object used".to_string(),
                Need::Item { item } => book.item_name(*item).unwrap_or("Item").to_string(),
            };
            Some(format!("{name}: {now}/{need}"))
        }
        Event::ReadyToTurnIn { quest } => Some(format!("{} (Complete)", title(*quest))),
        Event::TurnedIn { quest, .. } => Some(format!("Quest handed in: {}", title(*quest))),
        Event::Abandoned { quest } => Some(format!("Quest abandoned: {}", title(*quest))),
    }
}

fn report(toasts: &mut Toasts, now: f32, book: &QuestBook, events: &[Event]) {
    for e in events {
        if let Some(t) = toast_text(book, e) {
            toasts.push(now, t);
        }
        if let Some(s) = chime_of(e) {
            toasts.sounds.push(s);
        }
    }
}

/// The creature entry a dead creature stood for, with its spawn record's guid when it has one: its spawn
/// record's, a critter's own, or (without the manifest, for the old hand-placed groups) its species'
/// kobold-family entry.
fn entry_of(
    who: Entity,
    ids: &Query<(
        Option<&SpawnId>,
        Option<&crate::species::Species>,
        Option<&Critter>,
    )>,
    book: Option<&WorldBook>,
) -> Option<(u32, Option<u32>)> {
    let (spawn, species, critter) = ids.get(who).ok()?;
    if let Some(c) = critter {
        return Some((c.entry, None));
    }
    if let (Some(s), Some(b)) = (spawn, book) {
        return b.book.spots.get(s.0).map(|s| (s.entry, Some(s.guid)));
    }
    species
        .and_then(|s| crate::world::foe_entry(s.0.name))
        .map(|entry| (entry, None))
}

/// A creature died by the player's hand: the quests that ask for it count it.
fn credit_kills(
    mut slain: MessageReader<Slain>,
    game: Option<ResMut<QuestGame>>,
    real: Res<Time<Real>>,
    book: Option<Res<WorldBook>>,
    ids: Query<(
        Option<&SpawnId>,
        Option<&crate::species::Species>,
        Option<&Critter>,
    )>,
    mut toasts: ResMut<Toasts>,
) {
    let Some(mut game) = game else {
        slain.clear();
        return;
    };
    let now = real.elapsed_secs();
    for dead in slain.read() {
        let Some((entry, guid)) = entry_of(dead.who, &ids, book.as_deref()) else {
            continue;
        };
        let g = &mut *game;
        let events = g.log.kill(&g.book, entry);
        let reward = g.kill_reward(entry, guid);
        if !events.is_empty() {
            report(&mut toasts, now, &g.book, &events);
        }
        if let Some((xp, level, leveled)) = reward {
            toasts.push(now, format!("{xp} experience gained"));
            if let Some(new) = leveled {
                toasts.push(now, format!("You have reached level {new}"));
            }
            info!(
                "quests: kill xp +{xp} (entry {entry}, level {level}; total {}, you level {})",
                g.xp, g.player.level
            );
        }
        if !events.is_empty() || reward.is_some() {
            g.save();
        }
    }
}

/// The inventory changed: the log follows its quest items, and the loot module is told which quest
/// items a corpse may still drop.
fn sync_items(
    game: Option<ResMut<QuestGame>>,
    owned: Option<ResMut<Owned>>,
    loot_data: Option<Res<crate::loot::Data>>,
    real: Res<Time<Real>>,
    mut acquired: MessageWriter<Acquired>,
    mut eligibility: ResMut<QuestLootEligibility>,
    mut toasts: ResMut<Toasts>,
) {
    let (Some(mut game), Some(mut owned)) = (game, owned) else {
        return;
    };
    let g = &mut *game;
    let now = real.elapsed_secs();
    if owned.is_changed() {
        if let Some(data) = loot_data.as_deref() {
            let starters: Vec<(u32, u32)> = data
                .0
                .item_extras
                .iter()
                .filter(|ix| ix.start_quest != 0 && owned.progress.inventory.count(ix.entry) > 0)
                .map(|ix| (ix.entry, ix.start_quest))
                .collect();
            for (starter_item, quest) in starters {
                if wow_world::quest::can_take(&g.book, &g.log, &g.player, quest) {
                    let src_same = g
                        .book
                        .quest(quest)
                        .is_some_and(|q| q.src_item == starter_item);
                    if !src_same {
                        let _ = trade(&mut owned, &[], &[(starter_item, 1)]);
                    }
                    accept_from_item(
                        g,
                        &mut owned,
                        &mut acquired,
                        &mut toasts,
                        now,
                        quest,
                        src_same,
                    );
                }
            }
        }
        let inv = &owned.progress.inventory;
        let needed: BTreeSet<u32> = g
            .log
            .active()
            .into_iter()
            .filter_map(|q| g.book.quest(q))
            .flat_map(|q| q.req_items.iter().map(|&(i, _)| i))
            .collect();
        let mut events = Vec::new();
        for item in needed {
            events.extend(g.log.item_count(&g.book, item, inv.count(item)));
        }
        if !events.is_empty() {
            report(&mut toasts, now, &g.book, &events);
            g.save();
        }
    }
    let inv = &owned.progress.inventory;
    let wanted = g.log.droppable_items(&g.book, &g.player, &|i| inv.count(i));
    if eligibility.0 != wanted {
        info!("quests: corpses may drop the quest items {wanted:?}");
        eligibility.0 = wanted;
    }
}

/// Put `give` into the one shared inventory and take `take` out of it, saved at once. Nothing changes
/// when the save would fail.
fn trade(owned: &mut Owned, give: &[ItemCount], take: &[ItemCount]) -> Result<(), String> {
    let mut next = owned.progress.clone();
    for &(item, n) in take {
        let n = n.min(next.inventory.count(item));
        if n > 0 {
            next.inventory.remove(item, n).map_err(|e| e.to_string())?;
        }
    }
    next.inventory.add(give).map_err(|e| e.to_string())?;
    owned.publish(next)
}

/// Puts the souls a hand-in earned in the counter (as a kill's souls roll in), and turns the copper an
/// older save still holds into souls once: the game has one currency now.
fn pay_out(
    game: Option<ResMut<QuestGame>>,
    mut souls: ResMut<crate::hud::souls::Souls>,
    mut counter: ResMut<crate::hud::souls::Counter>,
) {
    let Some(mut game) = game else {
        return;
    };
    if game.money > 0 {
        let converted = u32::try_from(game.money)
            .unwrap_or(u32::MAX)
            .saturating_mul(wow_world::shop::SOULS_PER_COPPER);
        info!(
            "quests: {} copper of an older save became {converted} souls",
            game.money
        );
        game.money = 0;
        game.payout = game.payout.saturating_add(converted);
        game.save();
    }
    if game.payout > 0 {
        let n = std::mem::take(&mut game.payout);
        souls.0 = souls.0.saturating_add(n);
        counter.add(n);
        info!("souls: +{n} from a quest ({} carried)", souls.0);
    }
}

/// Everything the player does on the quest screens, from live keys, the pad and the script.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    real: Res<Time<Real>>,
    (keys, pads, unattended, controls): (
        Res<ButtonInput<KeyCode>>,
        Query<&Gamepad>,
        Option<Res<crate::run_mode::Unattended>>,
        Controls,
    ),
    (mut script, binding, mut bind_requests): (
        ResMut<Script>,
        Res<crate::soulbind::Binding>,
        MessageWriter<crate::soulbind::BindRequest>,
    ),
    game: Option<ResMut<QuestGame>>,
    (mut talk, mut log_screen, mut toasts, mut shop): (
        ResMut<Talk>,
        ResMut<LogScreen>,
        ResMut<Toasts>,
        ResMut<crate::shop::Shop>,
    ),
    (mut screen, active): (ResMut<Screen>, Res<crate::loot::interaction::Active>),
    owned: Option<ResMut<Owned>>,
    mut acquired: MessageWriter<Acquired>,
    avatar: Query<(&Transform, &Action, &Health), With<Avatar>>,
    npcs: Query<(&Transform, &Npc, Option<&Name>), Without<Avatar>>,
    (mut commands, mut objects, crates, loot_data, eligibility): (
        Commands,
        ResMut<objects::Objects>,
        Query<(Entity, &Transform, &objects::QuestObject), Without<Avatar>>,
        Option<Res<crate::loot::Data>>,
        Res<QuestLootEligibility>,
    ),
) {
    let (Some(mut game), Some(mut owned)) = (game, owned) else {
        return;
    };
    let Ok((me, action, health)) = avatar.single() else {
        return;
    };
    let now = real.elapsed_secs();
    let modal = talk.0.is_some() || log_screen.open || shop.open.is_some();

    // This frame's commands.
    let mut cmds: Vec<Cmd> = Vec::new();
    if unattended.is_none() {
        let any = |k: &[KeyCode]| k.iter().any(|&k| keys.just_pressed(k));
        let pad = |b: GamepadButton| pads.iter().any(|p| p.just_pressed(b));
        // The player runs and looks around while a screen is up, so WASD and the stick are not the
        // screen's: it takes the arrows, the D-pad, Enter / A, Backspace / B and Delete.
        if modal {
            if any(&[KeyCode::ArrowUp]) || pad(GamepadButton::DPadUp) {
                cmds.push(Cmd::Up);
            }
            if any(&[KeyCode::ArrowDown]) || pad(GamepadButton::DPadDown) {
                cmds.push(Cmd::Down);
            }
            // The rewards of a quest are looked at with the shoulder buttons or the side arrows (the D-pad
            // is the screen's up and down, its right is the log's own key).
            if any(&[KeyCode::ArrowLeft, KeyCode::BracketLeft]) || pad(GamepadButton::LeftTrigger) {
                cmds.push(Cmd::Prev);
            }
            if any(&[KeyCode::ArrowRight, KeyCode::BracketRight])
                || pad(GamepadButton::RightTrigger)
            {
                cmds.push(Cmd::Next);
            }
            if any(&[KeyCode::Enter, KeyCode::NumpadEnter]) || pad(GamepadButton::South) {
                cmds.push(Cmd::Confirm);
            }
            if any(&[KeyCode::Backspace]) || pad(GamepadButton::East) {
                cmds.push(Cmd::Back);
            }
            if any(&[KeyCode::Delete]) {
                cmds.push(Cmd::Abandon);
            }
            // A confirms and B goes back: neither may also jump or roll, this frame or just after
            // the screen closed on it ([`Screen::blocks_actions`]).
            if !cmds.is_empty() {
                screen.swallow_until = now + SWALLOW_SECS;
            }
        }
        // The Talk binding (T, the D-pad up) talks: its key always, its pad button only when no screen
        // is up (the D-pad is the screen's up and down). The Interact binding (X, pad Y) is a loot key
        // too: it talks only when nothing to loot has its own prompt.
        let interact = controls.just_pressed(BindAction::Interact);
        if controls.just_pressed_keys(BindAction::Talk)
            || (!modal && controls.just_pressed(BindAction::Talk))
            || (!modal && interact && screen.prompt.is_empty())
        {
            cmds.push(Cmd::Talk);
        }
        if controls.just_pressed(BindAction::QuestLog) {
            cmds.push(Cmd::Log);
        }
    }
    let mut cheats: Vec<String> = Vec::new();
    while let Some((t, step)) = script.steps.get(script.next).cloned() {
        if t > now {
            break;
        }
        script.next += 1;
        match step.as_str() {
            "talk" => cmds.push(Cmd::Talk),
            "log" => cmds.push(Cmd::Log),
            "up" => cmds.push(Cmd::Up),
            "down" => cmds.push(Cmd::Down),
            "prev" => cmds.push(Cmd::Prev),
            "next" => cmds.push(Cmd::Next),
            "confirm" => cmds.push(Cmd::Confirm),
            "back" => cmds.push(Cmd::Back),
            "abandon" => cmds.push(Cmd::Abandon),
            other => cheats.push(other.to_string()),
        }
    }

    let g = &mut *game;
    // The cheats of a scripted check.
    for c in cheats {
        match c.split_once(':') {
            Some(("kill", arg)) => {
                if let Some((entry, n)) = parse_times(arg) {
                    for _ in 0..n {
                        let events = g.log.kill(&g.book, entry);
                        report(&mut toasts, now, &g.book, &events);
                    }
                    g.save();
                    info!("quests: script credited {n} kills of entry {entry}");
                }
            }
            Some(("xp", arg)) => {
                if let Ok(n) = arg.trim().parse::<u32>() {
                    let total = g.xp.saturating_add(n);
                    let leveled = g.set_xp(total);
                    toasts.push(now, format!("{n} experience gained"));
                    if let Some(level) = leveled {
                        toasts.push(now, format!("You have reached level {level}"));
                    }
                    g.save();
                    info!(
                        "quests: script added {n} xp (total {}, level {})",
                        g.xp, g.player.level
                    );
                }
            }
            Some(("give", arg)) => {
                if let Some((item, n)) = parse_times(arg) {
                    match trade(&mut owned, &[(item, n)], &[]) {
                        Ok(()) => info!("quests: script gave {n} of item {item}"),
                        Err(e) => warn!("quests: script give failed: {e}"),
                    }
                }
            }
            _ => warn!("quests: unknown script step {c:?}"),
        }
    }

    // A screen closes when the player is hurt, dies, is busy with a body action, or walks away.
    let near = |npc: QuestNpc| match npc {
        QuestNpc::Creature(entry) => npcs.iter().any(|(xf, n, _)| {
            n.entry == entry && flat_distance(me.translation, xf.translation) <= CLOSE_RANGE
        }),
        QuestNpc::Object(entry) => crates.iter().any(|(_, xf, o)| {
            objects.spots.get(o.0).is_some_and(|s| s.entry == entry)
                && flat_distance(me.translation, xf.translation) <= CLOSE_RANGE
        }),
    };
    let busy = action.is_dead() || health.cur <= 0.0 || action.owns_body();
    if busy || talk.0.as_ref().is_some_and(|d| !near(d.npc)) {
        talk.0 = None;
    }
    if busy
        || shop
            .open
            .as_ref()
            .is_some_and(|o| !near(QuestNpc::Creature(o.npc)))
    {
        shop.open = None;
    }
    if busy {
        log_screen.open = false;
    }

    for cmd in cmds {
        match cmd {
            Cmd::Talk => {
                if talk.0.is_some() {
                    talk.0 = None;
                } else if !log_screen.open
                    && shop.open.is_none()
                    && !screen.open
                    && active.0.is_none()
                    && !busy
                    && !action.owns_body()
                {
                    if let Some(site) = crate::soulbind::nearest(&binding.sites, me.translation) {
                        if !screen.locked() && !screen.modal {
                            bind_requests.write(crate::soulbind::BindRequest(site.id));
                            screen.swallow_until = now + SWALLOW_SECS;
                        }
                        continue;
                    }
                    // A quest object within reach that a quest still wants comes before an NPC.
                    if let (Some(data), Some((e, index))) = (
                        loot_data.as_deref(),
                        objects::nearest_gatherable(
                            &crates,
                            &objects,
                            me.translation,
                            &eligibility.0,
                        ),
                    ) {
                        let table = objects.spots.get(index).map_or(0, |s| s.loot);
                        let seed = (now * 1000.0) as u64 ^ ((index as u64) << 32);
                        let items: Vec<ItemCount> = if table != 0 {
                            wow_world::loot::roll_with(
                                &data.0.loot,
                                wow_world::model::LootKind::Object,
                                table,
                                seed,
                                &eligibility.0,
                                crate::loot::chances(),
                            )
                            .unwrap_or_default()
                        } else {
                            objects
                                .spots
                                .get(index)
                                .map(|s| {
                                    s.quest_items
                                        .iter()
                                        .copied()
                                        .filter(|i| eligibility.0.contains(i))
                                        .map(|i| (i, 1))
                                        .collect()
                                })
                                .unwrap_or_default()
                        };
                        if !items.is_empty() {
                            match trade(&mut owned, &items, &[]) {
                                Ok(()) => {
                                    acquired.write(Acquired {
                                        items: items.clone(),
                                        discovery: false,
                                    });
                                    commands.entity(e).despawn();
                                    objects.gathered(index, now);
                                    info!("quests: gathered object {index}: {items:?}");
                                }
                                Err(e) => {
                                    toasts.push(now, format!("Could not take it: {e}"));
                                }
                            }
                            continue;
                        }
                    }
                    let nearest = npcs
                        .iter()
                        .map(|(xf, n, name)| {
                            (flat_distance(me.translation, xf.translation), n, name)
                        })
                        .filter(|(d, _, _)| *d <= TALK_RANGE)
                        .min_by(|a, b| a.0.total_cmp(&b.0));
                    if let Some((_, n, name)) = nearest {
                        let name = name.map_or("Stranger", |n| n.as_str());
                        let services = crate::shop::services(
                            loot_data.as_deref(),
                            n.flags,
                            n.entry,
                            g.player.class as u8,
                        );
                        let d = dialogue::open(
                            &g.book,
                            &g.log,
                            &g.player,
                            QuestNpc::Creature(n.entry),
                            name,
                            services,
                        );
                        info!("quests: talking to {name} ({}): page {:?}", n.entry, d.page);
                        if let dialogue::Page::Service(service) = d.page {
                            open_service(&mut shop, loot_data.as_deref(), service, n.entry, name);
                        } else {
                            talk.0 = Some(d);
                        }
                    } else if let Some((_, idx, _)) =
                        objects::nearest_talkable(&crates, &objects, g, me.translation)
                    {
                        if let Some(spot) = objects.spots.get(idx) {
                            let d = dialogue::open(
                                &g.book,
                                &g.log,
                                &g.player,
                                QuestNpc::Object(spot.entry),
                                &spot.name,
                                dialogue::Services::default(),
                            );
                            info!(
                                "quests: inspecting object {} ({}): page {:?}",
                                spot.name, spot.entry, d.page
                            );
                            talk.0 = Some(d);
                        }
                    }
                }
            }
            Cmd::Log => {
                if talk.0.is_none() && shop.open.is_none() && !screen.open && !busy {
                    log_screen.open = !log_screen.open;
                    log_screen.selected = 0;
                    log_screen.reward = 0;
                    log_screen.abandon_until = None;
                    toasts.sounds.push(if log_screen.open {
                        CHIME_LOG_OPEN
                    } else {
                        CHIME_LOG_CLOSE
                    });
                } else if log_screen.open {
                    log_screen.open = false;
                    toasts.sounds.push(CHIME_LOG_CLOSE);
                }
            }
            Cmd::Up | Cmd::Down | Cmd::Prev | Cmd::Next | Cmd::Confirm | Cmd::Back
                if talk.0.is_some() =>
            {
                let key = match cmd {
                    Cmd::Up => Key::Up,
                    Cmd::Down => Key::Down,
                    Cmd::Prev => Key::Prev,
                    Cmd::Next => Key::Next,
                    Cmd::Confirm => Key::Confirm,
                    _ => Key::Back,
                };
                let Some(d) = talk.0.as_mut() else { continue };
                match dialogue::press(d, key, &g.book, &g.log, &g.player) {
                    Outcome::None => {}
                    Outcome::Close => talk.0 = None,
                    Outcome::Service(service) => {
                        let entry = match d.npc {
                            QuestNpc::Creature(e) => e,
                            QuestNpc::Object(e) => e,
                        };
                        let name = d.npc_name.clone();
                        talk.0 = None;
                        open_service(&mut shop, loot_data.as_deref(), service, entry, &name);
                    }
                    Outcome::Accept(quest) => {
                        accept(g, &mut owned, &mut acquired, &mut toasts, now, quest);
                        talk.0 = None;
                    }
                    Outcome::Complete { quest, choice } => {
                        let npc = d.npc;
                        match complete(
                            g,
                            &mut owned,
                            &mut acquired,
                            &mut toasts,
                            now,
                            quest,
                            npc,
                            choice,
                        ) {
                            Some(next) => dialogue::offer(d, next),
                            None => talk.0 = None,
                        }
                    }
                }
            }
            Cmd::Up | Cmd::Down | Cmd::Prev | Cmd::Next | Cmd::Back | Cmd::Abandon
                if log_screen.open =>
            {
                let active_quests = g.log.active();
                match cmd {
                    Cmd::Up => {
                        log_screen.selected = log_screen.selected.saturating_sub(1);
                        log_screen.reward = 0;
                    }
                    Cmd::Down => {
                        log_screen.selected =
                            (log_screen.selected + 1).min(active_quests.len().saturating_sub(1));
                        log_screen.reward = 0;
                    }
                    Cmd::Prev | Cmd::Next => {
                        let total = active_quests
                            .get(log_screen.selected)
                            .and_then(|&q| g.book.quest(q))
                            .map_or(0, |q| q.reward_choice.len() + q.reward_items.len());
                        log_screen.reward = if cmd == Cmd::Next {
                            (log_screen.reward + 1).min(total.saturating_sub(1))
                        } else {
                            log_screen.reward.saturating_sub(1)
                        };
                    }
                    Cmd::Back => log_screen.open = false,
                    _ => {
                        if let Some(&quest) = active_quests.get(log_screen.selected) {
                            if log_screen.abandon_until.is_some_and(|t| now <= t) {
                                abandon(g, &mut owned, &mut toasts, now, quest);
                                log_screen.abandon_until = None;
                                log_screen.selected = log_screen.selected.saturating_sub(1);
                            } else {
                                log_screen.abandon_until = Some(now + 3.0);
                                toasts.push(now, "Press Delete again to abandon this quest");
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    screen.modal = talk.0.is_some() || log_screen.open || shop.open.is_some();
}

/// Opens a service of NPC `entry`: its shop, the strongbox or its lessons.
fn open_service(
    shop: &mut crate::shop::Shop,
    data: Option<&crate::loot::Data>,
    service: dialogue::Service,
    entry: u32,
    name: &str,
) {
    let subtitle = data
        .and_then(|d| d.0.creatures.iter().find(|c| c.entry == entry))
        .map_or("", |c| c.subname.as_str());
    shop.open = Some(crate::shop::Open::new(entry, name, subtitle, service));
    info!("shop: {name} ({entry}) opens {service:?}");
}

fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

fn accept(
    g: &mut QuestGame,
    owned: &mut Owned,
    acquired: &mut MessageWriter<Acquired>,
    toasts: &mut Toasts,
    now: f32,
    quest: u32,
) {
    accept_from_item(g, owned, acquired, toasts, now, quest, false);
}

fn accept_from_item(
    g: &mut QuestGame,
    owned: &mut Owned,
    acquired: &mut MessageWriter<Acquired>,
    toasts: &mut Toasts,
    now: f32,
    quest: u32,
    already_holds_source: bool,
) {
    let inv = owned.progress.inventory.clone();
    let mut log = g.log.clone();
    match log.accept(&g.book, &g.player, quest, &|i| inv.count(i)) {
        Ok(events) => {
            let source: Vec<ItemCount> = if already_holds_source {
                Vec::new()
            } else {
                events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Accepted {
                            source_item: Some(it),
                            ..
                        } => Some(*it),
                        _ => None,
                    })
                    .collect()
            };
            if let Err(e) = trade(owned, &source, &[]) {
                toasts.push(now, format!("Could not take the quest: {e}"));
                return;
            }
            if !source.is_empty() {
                acquired.write(Acquired {
                    items: source,
                    discovery: false,
                });
            }
            g.log = log;
            report(toasts, now, &g.book, &events);
            g.save();
            info!("quests: accepted {quest}");
        }
        Err(why) => {
            toasts.push(now, format!("You cannot take that quest ({why:?})"));
        }
    }
}

/// Hand a quest in; the quest the NPC offers next (`NextQuestInChain`), if any.
#[allow(clippy::too_many_arguments)]
fn complete(
    g: &mut QuestGame,
    owned: &mut Owned,
    acquired: &mut MessageWriter<Acquired>,
    toasts: &mut Toasts,
    now: f32,
    quest: u32,
    npc: QuestNpc,
    choice: Option<usize>,
) -> Option<u32> {
    let inv = owned.progress.inventory.clone();
    let mut log = g.log.clone();
    let events = match log.turn_in(&g.book, &g.player, quest, npc, choice, &|i| inv.count(i)) {
        Ok(e) => e,
        Err(e) => {
            toasts.push(now, format!("You cannot hand that in ({e:?})"));
            return None;
        }
    };
    let Some(Event::TurnedIn { rewards, .. }) = events.first() else {
        return None;
    };
    if let Err(e) = trade(owned, &rewards.items, &rewards.consume) {
        toasts.push(now, format!("Could not hand the quest in: {e}"));
        return None;
    }
    if !rewards.items.is_empty() {
        acquired.write(Acquired {
            items: rewards.items.clone(),
            discovery: false,
        });
    }
    g.log = log;
    g.payout = g.payout.saturating_add(wow_world::shop::souls_for_copper(
        rewards.money.max(0) as u32
    ));
    let leveled = g.set_xp(g.xp + rewards.xp);
    report(toasts, now, &g.book, &events);
    if rewards.xp > 0 {
        toasts.push(now, format!("{} experience gained", rewards.xp));
    }
    if rewards.money != 0 {
        toasts.push(
            now,
            format!(
                "{} souls",
                wow_world::shop::souls_for_copper(rewards.money.max(0) as u32)
            ),
        );
    }
    if let Some(level) = leveled {
        toasts.push(now, format!("You have reached level {level}"));
    }
    g.save();
    info!(
        "quests: handed in {quest}: xp {} (total {}, level {}), {} copper, items {:?}",
        rewards.xp, g.xp, g.player.level, rewards.money, rewards.items
    );
    rewards.next_in_chain
}

fn abandon(g: &mut QuestGame, owned: &mut Owned, toasts: &mut Toasts, now: f32, quest: u32) {
    let src = g
        .book
        .quest(quest)
        .map(|q| (q.src_item, q.src_item_count.max(1)));
    let events = g.log.abandon(quest);
    report(toasts, now, &g.book, &events);
    // The start item goes back with the quest (54's documents).
    if let Some((item, n)) = src.filter(|s| s.0 != 0) {
        let held = owned.progress.inventory.count(item).min(n);
        if held > 0 {
            let _ = trade(owned, &[], &[(item, held)]);
        }
    }
    g.save();
    info!("quests: abandoned {quest}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quests::testing::real_world;

    #[test]
    fn a_script_is_sorted_and_malformed_steps_are_dropped() {
        let s = parse_script("3.5:confirm, 1:talk,bogus, 2:kill:6x10, x:talk");
        assert_eq!(
            s,
            vec![
                (1.0, "talk".to_string()),
                (2.0, "kill:6x10".to_string()),
                (3.5, "confirm".to_string())
            ]
        );
        assert_eq!(parse_times("6x10"), Some((6, 10)));
        assert_eq!(parse_times("6"), None);
    }

    #[test]
    fn toasts_keep_the_newest_few() {
        let mut t = Toasts::default();
        for i in 0..10 {
            t.push(i as f32, format!("line {i}"));
        }
        assert_eq!(t.lines.len(), TOAST_MAX);
        assert_eq!(t.lines[0].0, "line 4");
        assert_eq!(t.lines.last().unwrap().1, 9.0 + TOAST_SECS);
    }

    #[test]
    fn toast_words_name_the_quest_the_creature_and_the_counter() {
        let Some(w) = real_world() else { return };
        let book = QuestBook::new(&w);
        let ev = Event::Progress {
            quest: 7,
            slot: 0,
            now: 3,
            need: 10,
            what: Need::Kill { entry: 6 },
        };
        assert_eq!(
            toast_text(&book, &ev).as_deref(),
            Some("Kobold Vermin slain: 3/10")
        );
        assert_eq!(
            toast_text(
                &book,
                &Event::Accepted {
                    quest: 7,
                    source_item: None
                }
            )
            .as_deref(),
            Some("Quest accepted: Kobold Camp Cleanup")
        );
        assert_eq!(
            toast_text(&book, &Event::ReadyToTurnIn { quest: 7 }).as_deref(),
            Some("Kobold Camp Cleanup (Complete)")
        );
        let item = Event::Progress {
            quest: 33,
            slot: 0,
            now: 5,
            need: 8,
            what: Need::Item { item: 750 },
        };
        assert_eq!(
            toast_text(&book, &item).as_deref(),
            Some("Tough Wolf Meat: 5/8")
        );
    }

    pub(super) fn game_of(w: &wow_world::model::World) -> QuestGame {
        QuestGame {
            book: QuestBook::new(w),
            table: XpTable::from_world(w),
            mobs: w
                .creatures
                .iter()
                .map(|c| (c.entry, MobXp::of(c)))
                .collect(),
            log: QuestLog::default(),
            player: Player::warrior("Jacob", 1),
            xp: 0,
            money: 0,
            payout: 0,
            path: PathBuf::from("unused"),
            blocked: Some("never saved in a test".into()),
        }
    }

    #[test]
    fn freshly_created_mage_publishes_class_before_class_systems_or_talents_run() {
        let mut game = game_of(&wow_world::model::World::default());
        let mut character = crate::character::Character::human_warrior("Magus");
        character.class = crate::character::Class::Mage;
        game.set_player_character(&character);
        assert_eq!(game.player.class, wow_world::quest::Class::Mage);
        assert_eq!(game.player.race, wow_world::quest::Race::Human);
        assert_eq!(game.player.name, "Magus");
        assert!(game.blocked.is_some(), "test never writes a live save");
    }

    #[test]
    fn a_script_can_add_experience() {
        assert_eq!(parse_script("1:xp:395"), vec![(1.0, "xp:395".to_string())]);
    }

    #[test]
    fn kills_pay_xp_into_the_one_total_and_level_the_player() {
        let Some(w) = real_world() else { return };
        let mut g = game_of(&w);
        // Young Wolf (299) is level 1: 50 XP each at level 1, so the eighth kill reaches 400 and level 2.
        for kill in 1..=7 {
            let (xp, level, leveled) = g.kill_reward(299, Some(80_000 + kill)).unwrap();
            assert_eq!((xp, level, leveled), (50, 1, None), "kill {kill}");
        }
        assert_eq!(g.kill_reward(299, Some(80_008)), Some((50, 1, Some(2))));
        assert_eq!((g.xp, g.player.level), (400, 2));
        // At level 2 the same wolf is below the player: 55 x (5 + 1 - 2) / 5 = 44.
        assert_eq!(g.kill_reward(299, None), Some((44, 1, None)));
        assert_eq!(g.xp, 444);
    }

    #[test]
    fn critters_and_unknown_creatures_pay_nothing() {
        let Some(w) = real_world() else { return };
        let mut g = game_of(&w);
        assert_eq!(g.kill_reward(721, Some(1)), None, "a rabbit");
        assert_eq!(g.kill_reward(883, None), None, "a deer");
        assert_eq!(g.kill_reward(999_999, None), None, "no such entry");
        assert_eq!(g.xp, 0);
    }

    #[test]
    fn the_level_gate_opens_by_kills_alone() {
        let Some(w) = real_world() else { return };
        let mut g = game_of(&w);
        // Quest 18 needs level 2 (MinLevel); a Defias Thug (3-4) at level 1 pays 55 or 58.
        let mut kills = 0;
        while g.player.level < 2 {
            g.kill_reward(38, Some(80_170 + kills)).unwrap();
            kills += 1;
            assert!(kills < 20, "level 2 within a few kills");
        }
        assert_eq!(
            kills, 8,
            "55, 58, 55, 58, 55, 58, 55 is 394: the eighth crosses 400"
        );
    }

    #[test]
    fn the_player_levels_from_the_xp_total() {
        let Some(w) = real_world() else { return };
        let mut g = game_of(&w);
        assert_eq!(g.set_xp(399), None);
        assert_eq!(g.set_xp(400), Some(2));
        assert_eq!(g.set_xp(401), None, "already level 2");
        assert_eq!(g.player.level, 2);
        g.save(); // blocked: writes nothing
    }
}
