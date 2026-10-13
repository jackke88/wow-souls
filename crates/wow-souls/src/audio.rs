//! The fight's sound, with WoW's own sounds from the user's install: the sword's swish, the blade
//! on flesh, the kobold's voice (aggro bark, attack grunt, wound, death) and its blow on the
//! player. Everything goes through a `SoundEntries` kit id and a position ([`Audio::play_kit`]),
//! so another bank (Dark Souls') can answer the same call later. Nothing here changes what the
//! game does: the triggers read the messages and state the fight already produces.
//!
//! `WOWSOULS_AUDIO` = `0` off, `null` the whole path with the output discarded and every play
//! logged, `1` the device even in an unattended run; unset: the device in an attended run, off
//! in an unattended one. `WOWSOULS_VOLUME=<0..1>` is the master (default 0.6; the options menu moves it). A sound is never a
//! reason to stop: a missing table, device or file is one warning and silence.
//!
//! The rules (impact slot by the victim's material, metal weapon rows, the natural weapon's
//! `CustomAttack` column, the weight of the swish, the voice chances) are benilla-app's, from its
//! `sound/combat.rs` and `kit.rs` (docs/PROVENANCE.md).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use benilla_assets::LockRecover;
use benilla_formats::{
    impact_slot, load_area_sound_catalog, load_creature_voice_catalog, load_footstep_catalog,
    load_item_sub_classes, load_npc_greeting_catalog, load_sound_kit_catalog,
    load_weapon_impact_catalog, load_weapon_swing_catalog, AreaSoundCatalog, Chain, CreatureVoice,
    CreatureVoiceCatalog, FootstepCatalog, ItemSubClassCatalog, NpcGreetingCatalog,
    SoundKitCatalog, WeaponImpactCatalog, WeaponSwingCatalog,
};
use benilla_world::view::WorldCamera;
use bevy::prelude::*;

use crate::actor::Catalogs;
use crate::avatar::Avatar;
use crate::combat::{Action, ActionState};
use crate::hits::Hit;
use crate::mixer::{Level, Output, StreamVoice, Voice};
use crate::npc::{Brain, Hostile, Mind, PlayerHurt, Telegraph};
use crate::pattern::Rng;
use crate::settle::Phase;
use crate::sfx::{self, Sound};

/// `ItemClass` 2 (weapon), and the starting sword's subclass (Sword1H, metal).
const WEAPON_CLASS: u32 = 2;
const SWORD_1H: u32 = 7;
/// The fist row of `WeaponImpactSounds`, a natural weapon's fallback.
const UNARMED: u32 = 13;
/// The swish sounds this long before a move's hit window opens (s).
const SWISH_LEAD: f32 = 0.08;
/// The reference's class chances (percent): a creature's exertion and injury vocals.
const EXERTION_CHANCE: u32 = 70;
const INJURY_CHANCE: u32 = 60;
/// Per-kind scales (tuned by ear); the master volume is the player's setting (default 0.6).
const SCALE_IMPACT: f32 = 1.0;
const SCALE_VOICE: f32 = 0.9;
const SCALE_SWISH: f32 = 0.7;
/// A sound that could not be loaded is tried again after this many seconds.
const RETRY_FAILED: f32 = 5.0;

/// The creature display whose `CreatureSoundData` row is this creature's voice.
#[derive(Component, Debug, Clone, Copy)]
pub struct VoiceOf(pub u32);

/// How the output is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Device,
    Null,
}

/// `WOWSOULS_AUDIO` and whether the run is unattended, to a mode (`None`: silent).
pub fn mode_from(value: Option<&str>, unattended: bool) -> Option<Mode> {
    match value.map(str::trim) {
        Some("0" | "off") => None,
        Some("null") => Some(Mode::Null),
        Some("1" | "on") => Some(Mode::Device),
        _ if unattended => None,
        _ => Some(Mode::Device),
    }
}

/// The client tables the fight's sounds read.
pub struct Tables {
    kits: SoundKitCatalog,
    swings: WeaponSwingCatalog,
    impacts: WeaponImpactCatalog,
    voices: CreatureVoiceCatalog,
    sub_classes: ItemSubClassCatalog,
    footsteps: FootstepCatalog,
    areas: AreaSoundCatalog,
    greetings: NpcGreetingCatalog,
}

/// Reads the sound tables off the patch chain.
pub fn load_tables(chain: &mut Chain) -> anyhow::Result<Tables> {
    Ok(Tables {
        kits: load_sound_kit_catalog(chain)?,
        swings: load_weapon_swing_catalog(chain)?,
        impacts: load_weapon_impact_catalog(chain)?,
        voices: load_creature_voice_catalog(chain)?,
        sub_classes: load_item_sub_classes(chain)?,
        footsteps: load_footstep_catalog(chain)?,
        areas: load_area_sound_catalog(chain)?,
        greetings: load_npc_greeting_catalog(chain)?,
    })
}

/// A music track or an ambience bed whose loudness its owner moves (fades) and which it can stop.
#[derive(Clone)]
pub struct Playing {
    level: Level,
    stop: Arc<std::sync::atomic::AtomicBool>,
    done: Arc<std::sync::atomic::AtomicBool>,
}

impl Playing {
    pub fn set_level(&self, v: f32) {
        self.level.set(v);
    }

    pub fn stop(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// A streamed track that has played to its end (or was stopped); a loop is never done on its own.
    pub fn is_done(&self) -> bool {
        self.done.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// What an area plays, owned (so the owner can keep it while it plays): the zone's music row, its
/// track kits and silences `[day, night]`, and the ambience row and its bed kits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneChoice {
    pub name: String,
    pub music_row: u32,
    pub music_kits: [u32; 2],
    pub silence_min_ms: [u32; 2],
    pub silence_max_ms: [u32; 2],
    pub ambience_row: u32,
    pub ambience_kits: [u32; 2],
}

/// Where the sounds are heard from: the game camera.
#[derive(Debug, Clone, Copy)]
pub struct Listener {
    pub pos: Vec3,
    pub right: Vec3,
}

#[derive(Resource)]
pub struct Audio {
    mode: Mode,
    out: Option<Output>,
    rate: u32,
    chain: Arc<Mutex<Chain>>,
    tables: Tables,
    cache: HashMap<String, Result<Sound, std::time::Instant>>,
    dice: Rng,
    /// The factor every voice's gain carries besides the kit's own and the caller's `scale`: 1. The
    /// master volume is the mixer's (live, [`Audio::set_levels`]).
    volume: f32,
    /// The master volume the mixer was last told (for the log).
    master: f32,
    /// The player's sound-effects level: on every kit and path sound.
    effects: f32,
    /// The player's quest-voices level: on the Spoken Player files.
    voices: f32,
    trace: bool,
}

impl Audio {
    fn new(mode: Mode, out: Option<Output>, chain: Arc<Mutex<Chain>>, tables: Tables) -> Audio {
        // The player's levels (the settings file, `WOWSOULS_VOLUME` over it): the master goes to the
        // mixer, where it stays live.
        let levels = &crate::settings::initial().sound;
        if let Some(out) = &out {
            out.set_master(levels.master);
        }
        Audio {
            mode,
            rate: out.as_ref().map_or(48_000, |o| o.rate),
            out,
            chain,
            tables,
            cache: HashMap::new(),
            dice: Rng(0x2545_F491_4F6C_DD1D),
            volume: 1.0,
            master: levels.master,
            effects: levels.effects,
            voices: levels.voices,
            trace: crate::actor::anim::trace_on(),
        }
    }

    /// The player's levels, 0 to 1 each: the master (the mixer's, at once), the effects and the quest
    /// voices (on the sounds started from now on).
    pub fn set_levels(&mut self, master: f32, effects: f32, voices: f32) {
        self.master = master;
        self.effects = effects;
        self.voices = voices;
        if let Some(out) = &self.out {
            out.set_master(master);
        }
    }

    /// The decoded sound at `path`, from the cache or off the chain. A failure is warned once and the
    /// sound is tried again after [`RETRY_FAILED`] seconds: one bad read must not leave a kit (the
    /// sword's impact, say) silent for the rest of the run.
    fn sound(&mut self, path: &str) -> Option<Sound> {
        let mut warned = false;
        match self.cache.get(path) {
            Some(Ok(s)) => return Some(s.clone()),
            Some(Err(at)) if at.elapsed().as_secs_f32() < RETRY_FAILED => return None,
            Some(Err(_)) => warned = true,
            None => {}
        }
        let loaded = self
            .chain
            .lock_recover()
            .read_file(path)
            .map_err(|e| format!("{e:#}"))
            .and_then(|bytes| sfx::decode(bytes, self.rate).map_err(|e| format!("{e:#}")));
        match loaded {
            Ok(s) => {
                self.cache.insert(path.to_string(), Ok(s.clone()));
                Some(s)
            }
            Err(e) => {
                if !warned {
                    warn!("audio: {path}: {e}");
                }
                self.cache
                    .insert(path.to_string(), Err(std::time::Instant::now()));
                None
            }
        }
    }

    /// Plays sound kit `kit` (0 plays nothing) at `at` as heard from `listener`, `scale` on top of
    /// the kit's own volume and the master; `what` names the trigger in the log.
    pub fn play_kit(&mut self, kit: u32, at: Option<(Vec3, Listener)>, scale: f32, what: &str) {
        if kit == 0 {
            return;
        }
        let Some(k) = self.tables.kits.get(kit) else {
            return;
        };
        let (volume, min, cutoff) = (k.volume, k.min_distance, k.distance_cutoff);
        let roll = self.dice.next_u32();
        let Some(i) = sfx::pick_variation(&k.files, roll) else {
            return;
        };
        let path = k.files[i].0.clone();
        let (dist, pan) = match at {
            Some((p, l)) => ((p - l.pos).length(), sfx::pan_of(l.right, p - l.pos)),
            None => (0.0, 0.0),
        };
        let Some(g) = sfx::gains(
            volume * scale * self.volume * self.effects,
            dist,
            min,
            cutoff,
            pan,
        ) else {
            if self.trace {
                info!("audio: {what}: kit {kit} is inaudible (listener {dist:.1} yd away, min {min} cutoff {cutoff})");
            }
            return;
        };
        let Some(sound) = self.sound(&path) else {
            return;
        };
        if self.trace || self.mode == Mode::Null {
            info!(
                "audio: {what}: kit {kit} {path} (gain {:.2}/{:.2}, {dist:.1} yd, {} frames)",
                g.0,
                g.1,
                sound.len()
            );
        }
        if let Some(out) = &self.out {
            out.play(Voice::new(sound.frames, g));
        }
    }

    /// Plays any sound of the install by its chain path (`Sound\interface\LevelUp.wav`, a quest chime, a
    /// loot clink), not positioned, `scale` on top of the master. Streamed from the client's own archives
    /// on first use and kept decoded; a missing file is one warning and silence.
    pub fn play_path(&mut self, path: &str, scale: f32, what: &str) {
        let Some(sound) = self.sound(path) else {
            return;
        };
        let Some(g) = sfx::gains(scale * self.volume * self.effects, 0.0, 1.0, 1.0e6, 0.0) else {
            return;
        };
        if self.trace || self.mode == Mode::Null {
            info!(
                "audio: {what}: {path} ({:.1} s, gain {:.2})",
                sound.len() as f32 / self.rate as f32,
                g.0
            );
        }
        if let Some(out) = &self.out {
            out.play(Voice::new(sound.frames, g));
        }
    }

    /// Plays the sound file at `path` (a quest voice: `.ogg`, `.mp3` or `.wav` on disk) not positioned, `scale`
    /// on top of the master. The flag it returns cuts the voice short when set; `None` when nothing plays
    /// (no such file, a file the decoders cannot read).
    pub fn play_file(
        &mut self,
        path: &std::path::Path,
        scale: f32,
        what: &str,
    ) -> Option<Arc<std::sync::atomic::AtomicBool>> {
        let bytes = std::fs::read(path).ok()?;
        let sound = match sfx::decode(bytes, self.rate) {
            Ok(s) => s,
            Err(e) => {
                warn!("audio: {}: {e:#}", path.display());
                return None;
            }
        };
        let gain = sfx::gains(scale * self.volume * self.voices, 0.0, 1.0, 1.0e6, 0.0)?;
        if self.trace || self.mode == Mode::Null {
            info!(
                "audio: {what}: {} ({:.1} s, gain {:.2})",
                path.display(),
                sound.len() as f32 / self.rate as f32,
                gain.0
            );
        }
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        if let Some(out) = &self.out {
            let mut voice = Voice::new(sound.frames, gain);
            voice.stop = Some(stop.clone());
            out.play(voice);
        }
        Some(stop)
    }

    fn voice(&self, display: u32) -> Option<&CreatureVoice> {
        self.tables.voices.for_display(display)
    }

    /// The terrain chain of the footsteps (ground-effect layer -> terrain -> sound class -> kit).
    pub fn footsteps(&self) -> &FootstepCatalog {
        &self.tables.footsteps
    }

    /// A creature display's footstep class (CreatureSoundData.FootstepID); 0 is silent.
    pub fn footstep_class(&self, display: u32) -> u32 {
        self.voice(display).map_or(0, |v| v.footstep_class)
    }

    /// A creature display's voice row (idle fidgets, alert and so on), for the sounds that read it.
    pub fn creature_voice(&self, display: u32) -> Option<&CreatureVoice> {
        self.voice(display)
    }

    /// What an area (an MCNK area id) plays, parents inherited; `None` for an unknown area.
    pub fn zone_choice(&self, area: u32) -> Option<ZoneChoice> {
        let a = self.tables.areas.resolve(area)?;
        Some(ZoneChoice {
            name: a.area_name.to_string(),
            music_row: a.music.map_or(0, |m| m.id),
            music_kits: a.music.map_or([0; 2], |m| m.sounds),
            silence_min_ms: a.music.map_or([0; 2], |m| m.silence_min),
            silence_max_ms: a.music.map_or([0; 2], |m| m.silence_max),
            ambience_row: a.ambience.map_or(0, |m| m.id),
            ambience_kits: a.ambience.map_or([0; 2], |m| m.kits),
        })
    }

    /// A file of sound kit `kit`, picked by the kits' weights: `(chain path, the kit's own volume)`.
    pub fn pick_file(&mut self, kit: u32) -> Option<(String, f32)> {
        let k = self.tables.kits.get(kit)?;
        let volume = k.volume;
        let roll = self.dice.next_u32();
        let i = sfx::pick_variation(&k.files, roll)?;
        Some((k.files[i].0.clone(), volume))
    }

    /// Streams the install's `path` (a music track) at `scale` x the master, starting at `level`: it is
    /// read and decoded on a thread of its own and plays as it arrives. `None` when it cannot play.
    pub fn stream_path(
        &mut self,
        path: &str,
        scale: f32,
        level: f32,
        what: &str,
    ) -> Option<Playing> {
        use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
        let gain = sfx::gains(scale * self.volume, 0.0, 1.0, 1.0e6, 0.0)?.0;
        let (level, stop, done) = (
            Level::new(level),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        let chain = self.chain.clone();
        let file = path.to_string();
        let rx = crate::stream::spawn(path.to_string(), self.rate, move || {
            chain
                .lock_recover()
                .read_file(&file)
                .map_err(|e| anyhow::anyhow!("{e:#}"))
        });
        if self.trace || self.mode == Mode::Null {
            info!("audio: {what}: streaming {path} (gain {gain:.2})");
        }
        match &self.out {
            Some(out) => out.play_stream(StreamVoice::new(
                rx,
                gain,
                level.clone(),
                stop.clone(),
                done.clone(),
            )),
            // No device: a listener that takes the track in real time, so the schedule behaves.
            None => {
                let (rate, s, d) = (self.rate, stop.clone(), done.clone());
                let _ = std::thread::Builder::new()
                    .name("null-stream".into())
                    .spawn(move || {
                        use std::sync::mpsc::RecvTimeoutError::{Disconnected, Timeout};
                        while !s.load(Relaxed) {
                            match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                                Ok(c) => std::thread::sleep(std::time::Duration::from_secs_f64(
                                    c.len() as f64 / f64::from(rate),
                                )),
                                Err(Timeout) => {}
                                Err(Disconnected) => break,
                            }
                        }
                        d.store(true, Relaxed);
                    });
            }
        }
        Some(Playing { level, stop, done })
    }

    /// Loops the install's `path` (an ambience bed) at `scale` x the master from `level`; decoded whole,
    /// ambience beds being short loops.
    pub fn loop_path(&mut self, path: &str, scale: f32, level: f32, what: &str) -> Option<Playing> {
        let sound = self.sound(path)?;
        let gain = sfx::gains(scale * self.volume, 0.0, 1.0, 1.0e6, 0.0)?;
        let (level, stop) = (
            Level::new(level),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        if self.trace || self.mode == Mode::Null {
            info!(
                "audio: {what}: looping {path} ({:.1} s, gain {:.2})",
                sound.len() as f32 / self.rate as f32,
                gain.0
            );
        }
        if let Some(out) = &self.out {
            let mut voice = Voice::new(sound.frames, gain);
            voice.looped = true;
            voice.level = Some(level.clone());
            voice.stop = Some(stop.clone());
            out.play(voice);
        }
        Some(Playing {
            level,
            stop,
            done: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// An NPC display's greeting kits `(hello, goodbye)`; `None` for a display that does not greet.
    pub fn greeting(&self, display: u32) -> Option<(u32, u32)> {
        self.tables
            .greetings
            .for_display(display)
            .map(|g| (g.hello, g.goodbye))
    }

    /// Plays the sound kit the client names `name` (`LOOTWINDOWCOINSOUND`), not positioned.
    pub fn play_named(&mut self, name: &str, scale: f32, what: &str) {
        let Some(kit) = self.tables.kits.by_name(name).map(|k| k.id) else {
            return;
        };
        self.play_kit(kit, None, scale, what);
    }

    /// A draw of the audio's own dice (kit variations, chances).
    pub fn roll(&mut self) -> u32 {
        self.dice.next_u32()
    }

    /// The weapon impact kit of a sword (metal) against a creature of `impact_type`.
    fn sword_impact(&self, impact_type: u32) -> u32 {
        self.tables
            .impacts
            .get(SWORD_1H, true)
            .map_or(0, |r| r.impact[sfx::creature_impact_slot(impact_type)])
    }

    /// The swish kit of a weapon swing of `weight` (0 light, 1 medium, 2 heavy).
    fn swish(&self, weight: u32) -> u32 {
        self.tables.swings.kit(weight, false).unwrap_or(0)
    }

    /// The sword's own swing weight, bumped one for a heavy attack.
    fn sword_weight(&self, heavy: bool) -> u32 {
        let base = self
            .tables
            .sub_classes
            .weapon_swing_size(WEAPON_CLASS, SWORD_1H)
            .unwrap_or(0);
        (base + u32::from(heavy)).min(2)
    }
}

/// `WOWSOULS_AUDIO` mode, the device, the tables; the resource only when something is to be heard.
fn load_audio(
    mut commands: Commands,
    catalogs: Option<Res<Catalogs>>,
    existing: Option<Res<Audio>>,
) {
    // Opened with the menu (its music), not again when play starts.
    if existing.is_some() {
        return;
    }
    let Some(mode) = mode_from(
        std::env::var("WOWSOULS_AUDIO").ok().as_deref(),
        crate::run_mode::unattended(),
    ) else {
        return;
    };
    let Some(catalogs) = catalogs else {
        warn!("audio: no catalogs; silent");
        return;
    };
    let tables = match load_tables(&mut catalogs.chain.lock_recover()) {
        Ok(t) => t,
        Err(e) => {
            warn!("audio: the sound tables did not load ({e:#}); silent");
            return;
        }
    };
    let out = match mode {
        Mode::Device => match Output::open() {
            Ok(o) => Some(o),
            Err(e) => {
                warn!("audio: no output ({e:#}); silent");
                return;
            }
        },
        Mode::Null => None,
    };
    let audio = Audio::new(mode, out, catalogs.chain.clone(), tables);
    info!(
        "audio: {:?}, {} sound kits, {} Hz, master {:.2}",
        audio.mode,
        audio.tables.kits.len(),
        audio.rate,
        audio.master
    );
    commands.insert_resource(audio);
}

pub(crate) fn listener(cams: &Query<&GlobalTransform, With<WorldCamera>>) -> Option<Listener> {
    cams.iter().next().map(|g| Listener {
        pos: g.translation(),
        right: *g.right(),
    })
}

/// The player's blows landing: the sword's impact on the victim's material at the hit point, and
/// the victim's wound vocal (or its death cry).
fn on_hits(
    audio: Option<ResMut<Audio>>,
    cams: Query<&GlobalTransform, With<WorldCamera>>,
    mut hits: MessageReader<Hit>,
    targets: Query<(&GlobalTransform, Option<&VoiceOf>)>,
) {
    let Some(mut audio) = audio else {
        hits.clear();
        return;
    };
    let l = listener(&cams);
    let heard = |p: Vec3| l.map(|l| (p, l));
    for hit in hits.read() {
        let (body, display) = targets.get(hit.target).map_or((hit.point, None), |(g, v)| {
            (g.translation() + Vec3::Y * 0.6, v.map(|v| v.0))
        });
        let voice = display.and_then(|d| audio.voice(d)).map(|v| {
            (
                v.impact_type,
                if hit.lethal { v.death } else { v.injury[0] },
            )
        });
        let impact = audio.sword_impact(voice.map_or(0, |v| v.0));
        audio.play_kit(impact, heard(hit.point), SCALE_IMPACT, "sword impact");
        if let Some((_, vocal)) = voice {
            let roll = audio.dice.next_u32();
            if hit.lethal {
                audio.play_kit(vocal, heard(body), SCALE_VOICE, "death cry");
            } else if sfx::chance(INJURY_CHANCE, roll) {
                audio.play_kit(vocal, heard(body), SCALE_VOICE, "wound vocal");
            }
        }
    }
}

/// The player's swing: the swish of its weapon's weight just before the hit window opens.
fn on_player_swing(
    audio: Option<ResMut<Audio>>,
    cams: Query<&GlobalTransform, With<WorldCamera>>,
    avatar: Query<(&Action, &GlobalTransform), With<Avatar>>,
    mut last: Local<Option<u32>>,
) {
    let (Some(mut audio), Ok((action, at))) = (audio, avatar.single()) else {
        return;
    };
    let ActionState::Attack(swing) = action.state else {
        return;
    };
    if swing.t < swing.spec.hit.0 - SWISH_LEAD || *last == Some(swing.serial) {
        return;
    }
    *last = Some(swing.serial);
    let kit = audio.swish(audio.sword_weight(swing.spec.mult >= 1.2));
    let l = listener(&cams);
    let p = at.translation() + Vec3::Y;
    audio.play_kit(kit, l.map(|l| (p, l)), SCALE_SWISH, "player swish");
}

/// What the audio remembers of one creature between frames.
#[derive(Default)]
struct FoeState {
    alert: bool,
    /// How many of the swing's hit windows have had their swish.
    swished: usize,
}

/// The creatures: each one's aggro bark when it notices the player, its swish before its hit
/// window and its attack grunt at the telegraph.
#[allow(clippy::type_complexity)]
fn on_foes(
    audio: Option<ResMut<Audio>>,
    roster: Option<Res<crate::monster::Roster>>,
    cams: Query<&GlobalTransform, With<WorldCamera>>,
    mut telegraphs: MessageReader<Telegraph>,
    foes: Query<(
        Entity,
        &Brain,
        &Hostile,
        &GlobalTransform,
        &VoiceOf,
        &crate::species::Species,
    )>,
    mut state: Local<HashMap<Entity, FoeState>>,
) {
    let Some(mut audio) = audio else {
        telegraphs.clear();
        return;
    };
    let l = listener(&cams);
    let heard = |p: Vec3| l.map(|l| (p, l));
    for t in telegraphs.read() {
        let Ok((_, _, _, at, display, _)) = foes.get(t.from) else {
            continue;
        };
        let exertion = audio.voice(display.0).map_or(0, |v| v.exertion[0]);
        let roll = audio.dice.next_u32();
        if sfx::chance(EXERTION_CHANCE, roll) {
            audio.play_kit(
                exertion,
                heard(at.translation() + Vec3::Y * 0.6),
                SCALE_VOICE,
                "attack grunt",
            );
        }
    }
    for (e, brain, hostile, at, display, species) in &foes {
        let s = state.entry(e).or_default();
        let p = at.translation() + Vec3::Y * 0.6;
        if hostile.alert && !s.alert {
            let aggro = audio.voice(display.0).map_or(0, |v| v.aggro);
            audio.play_kit(aggro, heard(p), SCALE_VOICE, "aggro bark");
        }
        s.alert = hostile.alert;
        let kit = roster.as_deref().and_then(|r| r.get(species.0.kit));
        match (brain.mind, kit) {
            (Mind::Attack { t, mv }, Some(k)) => {
                if let Some(a) = k.attacks.get(usize::from(mv)) {
                    // Only a swung blow (a fist or a blade) whooshes; a bite does not.
                    if species.0.blow != crate::species::Blow::Bone
                        && a.spec
                            .hits
                            .get(s.swished)
                            .is_some_and(|w| t >= w.0 - SWISH_LEAD)
                    {
                        s.swished += 1;
                        let weight = u32::from(a.spec.name == "big");
                        let kit = audio.swish(weight);
                        audio.play_kit(
                            kit,
                            heard(p),
                            SCALE_SWISH,
                            &format!("{} swish", species.0.name),
                        );
                    }
                }
            }
            (Mind::Attack { .. }, None) => {}
            _ => s.swished = 0,
        }
    }
}

/// A creature's blow landing on the player: a held blade's metal-on-flesh impact, else its natural
/// weapon's `CustomAttack` kit at the player (else the unarmed flesh impact).
fn on_player_hurt(
    audio: Option<ResMut<Audio>>,
    cams: Query<&GlobalTransform, With<WorldCamera>>,
    mut hurts: MessageReader<PlayerHurt>,
    foes: Query<(&VoiceOf, &crate::species::Species)>,
    avatar: Query<&GlobalTransform, With<Avatar>>,
) {
    let Some(mut audio) = audio else {
        hurts.clear();
        return;
    };
    let Ok(player) = avatar.single() else {
        return;
    };
    let l = listener(&cams);
    let p = player.translation() + Vec3::Y;
    for h in hurts.read() {
        if h.blocked {
            let kit = audio
                .tables
                .impacts
                .get(SWORD_1H, true)
                .map_or(0, |r| r.impact[impact_slot::PARRY_METAL]);
            audio.play_kit(kit, l.map(|l| (p, l)), SCALE_IMPACT, "sword block");
            continue;
        }
        let foe = foes.get(h.from).ok();
        let armed = foe.is_some_and(|(_, s)| s.0.blow == crate::species::Blow::Blade);
        let custom: Vec<u32> = foe
            .and_then(|(d, _)| audio.voice(d.0))
            .map(|v| {
                v.custom_attack
                    .iter()
                    .copied()
                    .filter(|k| *k != 0)
                    .collect()
            })
            .unwrap_or_default();
        let kit = if armed {
            // The body struck is a human male's, like the player's own.
            let flesh = audio
                .voice(crate::avatar::HUMAN_MALE_DISPLAY)
                .map_or(0, |v| v.impact_type);
            audio.sword_impact(flesh)
        } else if custom.is_empty() {
            audio
                .tables
                .impacts
                .get(UNARMED, false)
                .map_or(0, |r| r.impact[impact_slot::FLESH])
        } else {
            custom[(audio.dice.next_u32() as usize) % custom.len()]
        };
        audio.play_kit(kit, l.map(|l| (p, l)), SCALE_IMPACT, "blow on the player");
    }
}

fn on_parried(
    audio: Option<ResMut<Audio>>,
    cams: Query<&GlobalTransform, With<WorldCamera>>,
    mut parries: MessageReader<crate::defense::Parried>,
) {
    let Some(mut audio) = audio else {
        parries.clear();
        return;
    };
    let l = listener(&cams);
    for p in parries.read() {
        let kit = audio
            .tables
            .impacts
            .get(SWORD_1H, true)
            .map_or(0, |r| r.impact[impact_slot::PARRY_METAL]);
        audio.play_kit(kit, l.map(|l| (p.point, l)), SCALE_IMPACT, "parry");
    }
}

/// The player's sound levels (the settings, changed in the menu) reach the mixer and the kits.
fn sync_levels(
    settings: Res<crate::settings::Settings>,
    audio: Option<ResMut<Audio>>,
    mut told: Local<Option<(f32, f32, f32)>>,
) {
    let Some(mut audio) = audio else {
        return;
    };
    let s = &settings.sound;
    let want = (s.master, s.effects, s.voices);
    if *told != Some(want) {
        audio.set_levels(want.0, want.1, want.2);
        *told = Some(want);
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Phase::Menu), load_audio)
        .add_systems(OnEnter(Phase::Playing), load_audio)
        .add_systems(Update, sync_levels)
        .add_systems(
            Update,
            (
                on_hits,
                on_player_swing,
                on_foes,
                on_player_hurt,
                on_parried,
            )
                .after(crate::controller::PlaySet::Motor)
                .run_if(in_state(Phase::Playing)),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug behind "the sword is silent": the listener was the first `Camera3d` of the query, and
    /// another one (a helper camera at the world's origin) came first, 8900 yd from the fight, so every
    /// positional sound fell off to nothing. The listener is the world camera, whatever else renders.
    #[test]
    fn the_listener_is_the_world_camera_not_any_other_camera() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        // A helper camera at the origin first, then the real one where the player is.
        world.spawn((Camera3d::default(), GlobalTransform::default()));
        world.spawn((
            Camera3d::default(),
            WorldCamera,
            GlobalTransform::from(Transform::from_xyz(-8900.0, 90.0, 140.0)),
        ));
        let heard = world
            .run_system_once(|cams: Query<&GlobalTransform, With<WorldCamera>>| listener(&cams))
            .unwrap()
            .expect("a listener");
        assert_eq!(heard.pos, Vec3::new(-8900.0, 90.0, 140.0));
        // A blow next to the player is heard in full from there.
        let dist = (Vec3::new(-8898.0, 90.0, 137.0) - heard.pos).length();
        let (l, r) = sfx::gains(1.0, dist, 8.0, 45.0, 0.0).expect("audible");
        assert!(l > 0.5 && r > 0.5, "{l} {r}");
    }

    #[test]
    fn the_mode_follows_the_variable_and_the_run() {
        assert_eq!(mode_from(None, false), Some(Mode::Device));
        assert_eq!(mode_from(None, true), None, "unattended runs are silent");
        assert_eq!(mode_from(Some("1"), true), Some(Mode::Device));
        assert_eq!(mode_from(Some("null"), true), Some(Mode::Null));
        assert_eq!(mode_from(Some("0"), false), None);
        assert_eq!(mode_from(Some("off"), false), None);
    }

    /// The tables the fight reads resolve to kits, and every file of every kit it uses decodes.
    #[test]
    fn install_the_fights_kits_resolve_and_decode() {
        let Some(data) = benilla_formats::wow_data() else {
            eprintln!("skipped: no WoW install");
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let t = load_tables(&mut chain).unwrap();
        // The sword (Sword1H, metal) strikes flesh; the swish has a kit at each weight.
        let flesh = t.impacts.get(SWORD_1H, true).unwrap().impact[impact_slot::FLESH];
        assert_ne!(flesh, 0);
        let weight = t
            .sub_classes
            .weapon_swing_size(WEAPON_CLASS, SWORD_1H)
            .unwrap();
        assert!(weight <= 2);
        let swishes: Vec<u32> = (0..3).map(|w| t.swings.kit(w, false).unwrap()).collect();
        // The kobold (display 365) has a voice.
        let v = t.voices.for_display(365).expect("the kobold's voice row");
        assert_ne!(v.aggro, 0);
        assert_ne!(v.death, 0);
        // It has no `CustomAttack` column: its grunt is the exertion kit, its blow the unarmed
        // flesh impact.
        assert_ne!(v.exertion[0], 0);
        assert_ne!(v.injury[0], 0);
        assert!(v.custom_attack.iter().all(|k| *k == 0));
        let unarmed = t.impacts.get(UNARMED, false).unwrap().impact[impact_slot::FLESH];
        let mut kits = vec![flesh, unarmed, v.aggro, v.death, v.injury[0], v.exertion[0]];
        kits.extend(swishes);
        kits.extend(v.custom_attack);
        let mut decoded = 0;
        for kit in kits.into_iter().filter(|k| *k != 0) {
            let k = t.kits.get(kit).unwrap_or_else(|| panic!("kit {kit}"));
            assert!(!k.files.is_empty(), "kit {kit} has files");
            for (path, _) in &k.files {
                let bytes = chain
                    .read_file(path)
                    .unwrap_or_else(|e| panic!("{path}: {e:#}"));
                let s = sfx::decode(bytes, 48_000).unwrap_or_else(|e| panic!("{path}: {e:#}"));
                assert!(s.len() > 100, "{path}: {} frames", s.len());
                decoded += 1;
            }
        }
        println!("{decoded} sound files decode");
        assert!(decoded >= 10);
    }
}
