//! The combat core: health, stamina and the action a body is committed to.
//!
//! An [`Action`] owns the body while it runs: the attack plays its own clip, moves the body by a
//! small lunge instead of the stick, and cannot be interrupted except by its successor in the
//! cancel window. Input is buffered for [`BUFFER`] seconds, so a press slightly early still
//! lands, as Souls games buffer it. Stamina pays for actions and regenerates only after a pause.
//!
//! The evade: pushed, a roll along the stick; still, a backstep. The 1.12 human has no roll clip,
//! so the roll is built: the body travels by a speed curve through the motor (fast off the mark,
//! easing to a stop) while the model, crouched in StealthStand, turns one forward somersault
//! under the body ([`crate::actor::ModelFrame`]). Hits pass through it in its invulnerability
//! window ([`Action::invulnerable`]). With the Dark Souls cache loaded ([`crate::motion_clip`]) the
//! roll is Dark Souls' own: its clip poses the rig, its root motion moves the body, and its
//! events set the invulnerability and the cancel. A pushed stick ends any evade from its cancel
//! window, as movement cancels a Dark Souls roll's stand-up.
//!
//! Timings are first-pass values in Souls proportions (startup, active, recovery as fractions of
//! the clip). They are to be tuned against Dark Souls Remastered once the slice is playtested.

use std::time::Duration;

use bevy::animation::transition::AnimationTransitions;
use bevy::animation::RepeatAnimation;
use bevy::prelude::*;

use benilla_assets::ModelAnimations;

mod action;
pub use action::*;
pub mod duel;
// Bevy's prelude has a `Press` too (picking): name ours, or the two globs clash.
use action::Press;

use crate::actor::ModelFrame;
use crate::avatar::Avatar;
use crate::camera::CamMode;
use crate::controller::PlaySet;
use crate::motor::{Motor, MoveIntent};
use crate::settings::binds::{Action as BindAction, Binds};
use crate::settle::Phase;

/// Attack1H, the one-handed swing (AnimationData.dbc).
pub const ATTACK_1H: u16 = 17;
/// Playback rate of the swing: WoW's auto-attack is slow for a Souls light attack.
pub const ATTACK_RATE: f32 = 1.25;
/// The forward lunge speed while winding up and swinging (yd/s).
pub const LUNGE_SPEED: f32 = 1.6;
pub const HEALTH_MAX: f32 = 100.0;
pub const STAMINA_MAX: f32 = 100.0;
/// Stamina a light attack costs.
pub const ATTACK_COST: f32 = 18.0;
/// StealthStand, the crouch the roll tumbles through (AnimationData.dbc).
pub const ROLL_CROUCH: u16 = 120;
/// Dodge, the sway the backstep plays.
pub const BACKSTEP_SWAY: u16 = 30;
/// The height over the feet (yd) the somersault turns the model about: the crouch's middle.
pub const ROLL_PIVOT: f32 = 0.7;
/// Stamina regained per second once regeneration runs.
pub const STAMINA_REGEN: f32 = 45.0;
/// The pause after spending before regeneration starts (s).
pub const STAMINA_DELAY: f32 = 0.6;

#[derive(Component, Debug, Clone, Copy)]
pub struct Health {
    pub cur: f32,
    pub max: f32,
}

impl Health {
    pub fn full(max: f32) -> Self {
        Self { cur: max, max }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct Stamina {
    pub cur: f32,
    pub max: f32,
    /// Seconds since stamina was last spent.
    pub rested: f32,
}

impl Stamina {
    pub fn full(max: f32) -> Self {
        Self {
            cur: max,
            max,
            rested: STAMINA_DELAY,
        }
    }

    /// An action may start with any stamina left; it then spends down to zero at most.
    pub fn can_spend(&self) -> bool {
        self.cur > 0.0
    }

    pub fn spend(&mut self, cost: f32) {
        self.cur = (self.cur - cost).max(0.0);
        self.rested = 0.0;
    }

    /// One frame of regeneration; `busy` (an action running) holds it.
    pub fn tick(&mut self, dt: f32, busy: bool) {
        if busy {
            self.rested = 0.0;
            return;
        }
        self.rested += dt;
        if self.rested >= STAMINA_DELAY {
            self.cur = (self.cur + STAMINA_REGEN * dt).min(self.max);
        }
    }
}

/// A scripted press: at `at` seconds, then every `every`, and none from `until` on. An
/// unattended-run instrument.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Schedule {
    pub at: f32,
    pub every: Option<f32>,
    pub until: Option<f32>,
}

/// `<s>[,<every s>[,<until s>]]`.
pub fn parse_schedule(v: &str) -> Option<Schedule> {
    let n: Vec<f32> = v
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect::<Option<_>>()?;
    match n[..] {
        [at] => Some(Schedule {
            at,
            every: None,
            until: None,
        }),
        [at, every] if every > 0.0 => Some(Schedule {
            at,
            every: Some(every),
            until: None,
        }),
        [at, every, until] if every > 0.0 && until > at => Some(Schedule {
            at,
            every: Some(every),
            until: Some(until),
        }),
        _ => None,
    }
}

impl Schedule {
    /// Whether the press is due at `now`; `next` keeps the schedule's place.
    fn due(&self, next: &mut Option<f32>, now: f32) -> bool {
        let due = next.get_or_insert(self.at);
        if now < *due || self.until.is_some_and(|u| now >= u) {
            return false;
        }
        *due = self.every.map_or(f32::INFINITY, |e| *due + e);
        true
    }
}

/// `WOWSOULS_ATTACK=<s>[,<every s>]`: press attack on a [`Schedule`].
#[derive(Resource, Debug, Clone, Copy)]
pub struct ScriptedAttack(pub Schedule);

/// `WOWSOULS_HEAVY=<s>[,<every s>]`: press heavy on a [`Schedule`] (nothing without the Dark
/// Souls moveset, which has the only heavy attack).
#[derive(Resource, Debug, Clone, Copy)]
pub struct ScriptedHeavy(pub Schedule);

/// `WOWSOULS_ROLL=<s>[,<every s>]`: press evade on a [`Schedule`]; `WOWSOULS_DRIVE` aims it.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ScriptedRoll(pub Schedule);

#[derive(Resource)]
pub struct ScriptedParry(pub Schedule);
#[derive(Resource)]
pub struct ScriptedGuard(pub crate::controller::DriveWindow);
#[derive(Resource)]
pub struct ParryAfter(pub f32);

/// `WOWSOULS_DODGE=<delay>[,<x>,<y>]`: press evade `delay` seconds after a creature's wind-up
/// starts ([`crate::npc::Telegraph`]), the stick pushed `(x right, y forward)` in the body's own
/// frame; without a push, a backstep. An unattended-run instrument.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct ScriptedDodge {
    pub delay: f32,
    pub push: Vec2,
}

pub fn parse_dodge(v: &str) -> Option<ScriptedDodge> {
    let n: Vec<f32> = v
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect::<Option<_>>()?;
    let (delay, push) = match n[..] {
        [d] => (d, Vec2::ZERO),
        [d, x, y] => (d, Vec2::new(x, y).clamp_length_max(1.0)),
        _ => return None,
    };
    (delay >= 0.0).then_some(ScriptedDodge { delay, push })
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (act, tumble)
            .chain()
            .in_set(PlaySet::Intent)
            .after(crate::controller::read_intent),
    );
    let schedule = |key: &str| std::env::var(key).ok().and_then(|v| parse_schedule(&v));
    if let Some(s) = schedule("WOWSOULS_ATTACK") {
        app.insert_resource(ScriptedAttack(s));
    }
    if let Some(s) = schedule("WOWSOULS_HEAVY") {
        app.insert_resource(ScriptedHeavy(s));
    }
    if let Some(s) = schedule("WOWSOULS_ROLL") {
        app.insert_resource(ScriptedRoll(s));
    }
    if let Some(s) = schedule("WOWSOULS_PARRY") {
        app.insert_resource(ScriptedParry(s));
    }
    if let Some(s) = std::env::var("WOWSOULS_GUARD")
        .ok()
        .and_then(|v| crate::controller::parse_drive_window(&v))
    {
        app.insert_resource(ScriptedGuard(s));
    }
    if let Some(s) = std::env::var("WOWSOULS_PARRY_AFTER")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
    {
        app.insert_resource(ParryAfter(s));
    }
    if let Some(d) = std::env::var("WOWSOULS_DODGE")
        .ok()
        .and_then(|v| parse_dodge(&v))
    {
        app.insert_resource(d);
    }
    if let Some(seed) = std::env::var("WOWSOULS_DUEL")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
    {
        app.insert_resource(duel::DuelCall::default())
            .insert_resource(duel::Duel::new(seed))
            .add_systems(
                Update,
                duel::bot
                    .in_set(PlaySet::Intent)
                    .after(crate::controller::read_intent)
                    .before(act),
            );
    }
    if std::env::var_os("WOWSOULS_TRACE").is_some() {
        app.add_systems(
            Update,
            trace.after(PlaySet::Motor).run_if(in_state(Phase::Playing)),
        );
    }
}

/// The attack press this frame: the Light binding (F, the left mouse with the cursor captured, the
/// right bumper by default; the capturing click is no swing).
fn attack_pressed(
    binds: &Binds,
    keys: &ButtonInput<KeyCode>,
    buttons: &ButtonInput<MouseButton>,
    gamepads: &Query<&Gamepad>,
    captured: bool,
) -> bool {
    binds.just_pressed_in(BindAction::Light, keys, buttons, gamepads, captured)
}

/// The heavy press this frame: the Heavy binding (R, the right mouse with the cursor captured, the
/// right trigger by default).
fn heavy_pressed(
    binds: &Binds,
    keys: &ButtonInput<KeyCode>,
    buttons: &ButtonInput<MouseButton>,
    gamepads: &Query<&Gamepad>,
    captured: bool,
) -> bool {
    binds.just_pressed_in(BindAction::Heavy, keys, buttons, gamepads, captured)
}

/// The evade press this frame: the Evade binding (Space, or gamepad East; Shift+Space is the jump, the
/// more specific chord).
fn evade_pressed(
    binds: &Binds,
    keys: &ButtonInput<KeyCode>,
    buttons: &ButtonInput<MouseButton>,
    gamepads: &Query<&Gamepad>,
    captured: bool,
) -> bool {
    binds.just_pressed_in(BindAction::Evade, keys, buttons, gamepads, captured)
}

/// The parry press this frame: the Parry binding (C, the left trigger by default).
fn parry_pressed(
    binds: &Binds,
    keys: &ButtonInput<KeyCode>,
    buttons: &ButtonInput<MouseButton>,
    gamepads: &Query<&Gamepad>,
    captured: bool,
) -> bool {
    binds.just_pressed_in(BindAction::Parry, keys, buttons, gamepads, captured)
}

/// The block is held: the Block binding (E, the left bumper by default).
fn guard_held(
    binds: &Binds,
    keys: &ButtonInput<KeyCode>,
    buttons: &ButtonInput<MouseButton>,
    gamepads: &Query<&Gamepad>,
    captured: bool,
) -> bool {
    binds.pressed_in(BindAction::Block, keys, buttons, gamepads, captured)
}

/// A body's intent for one frame's share `step` (yd) of a clip's root motion along `dir`. A
/// negative step (a sword's recoil) backs the body along `dir` through a negative speed: the body
/// turns toward its `dir`, so `dir` stays the way the clip faces. The motor integrates at most
/// [`crate::motor::MAX_STEP`] of a frame, so the step is spread over that much.
pub(crate) fn root_step(dir: Vec3, step: f32, dt: f32) -> MoveIntent {
    let secs = dt.min(crate::motor::MAX_STEP);
    MoveIntent {
        dir: if step == 0.0 { Vec3::ZERO } else { dir },
        speed: if secs > 0.0 { step / secs } else { 0.0 },
        jump: false,
    }
}

/// Where the last evade or move started and when (real s), for the trace's distance line.
#[derive(Default)]
pub(crate) struct ActionTrace(Option<(String, Vec3, f32)>);

impl ActionTrace {
    fn close(&mut self, at: Vec3, now: f32) {
        if let Some((what, from, t0)) = self.0.take() {
            let d = at - from;
            info!(
                "combat: {what} moved {:.2} yd in {:.2} s",
                Vec3::new(d.x, 0.0, d.z).length(),
                now - t0
            );
        }
    }
}

/// Runs the avatar's action: reads the presses, steps the machine, plays the swing or the evade,
/// and while an action owns the body replaces the stick's intent with its own motion.
#[allow(clippy::too_many_arguments)]
pub(crate) fn act(
    time: Res<Time>,
    real: Res<Time<bevy::time::Real>>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    window: Query<&bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
    (mode, loot_screen, settings, menu): (
        Res<CamMode>,
        Res<crate::loot::Screen>,
        Res<crate::settings::Settings>,
        Res<crate::settings::MenuUp>,
    ),
    unattended: Option<Res<crate::run_mode::Unattended>>,
    scripted: (
        Option<Res<ScriptedAttack>>,
        Option<Res<ScriptedHeavy>>,
        Option<Res<ScriptedRoll>>,
        Option<Res<duel::DuelCall>>,
    ),
    mut next_scripted: Local<(Option<f32>, Option<f32>, Option<f32>)>,
    mut action_trace: Local<ActionTrace>,
    defense: (
        Option<Res<crate::defense::Kit>>,
        Option<Res<ScriptedGuard>>,
        Option<Res<ScriptedParry>>,
        Option<Res<ParryAfter>>,
        Local<Option<f32>>,
        Local<Option<f32>>,
    ),
    dodge: (
        MessageReader<crate::npc::Telegraph>,
        Option<Res<ScriptedDodge>>,
        Local<Option<f32>>,
    ),
    targets: Query<(&GlobalTransform, &crate::hits::Hurtbox), Without<Avatar>>,
    reference: (
        Option<Res<crate::motion_clip::EvadeClips>>,
        Option<Res<crate::motion_clip::WeaponClips>>,
        Option<Res<crate::reaction::PlayerReacts>>,
        Option<Res<crate::weapons::Ready>>,
    ),
    mut avatar: Query<
        (
            &mut Transform,
            &Motor,
            &mut MoveIntent,
            &mut Action,
            &mut Stamina,
            Option<&ModelAnimations>,
            Option<&mut AnimationPlayer>,
            Option<&mut AnimationTransitions>,
            Option<&crate::motor::swim::Swimming>,
        ),
        With<Avatar>,
    >,
) {
    let Ok((mut xf, m, mut intent, mut action, mut stamina, anims, player, transitions, swimming)) =
        avatar.single_mut()
    else {
        return;
    };
    // Afloat there is no swinging, rolling or guarding: the stroke is all the body does.
    if swimming.is_some_and(|s| s.on) && !action.owns_body() {
        stamina.tick(time.delta_secs(), false);
        return;
    }
    // An open inventory holds the body still; a quest screen holds its actions only, the player may
    // walk about while talking.
    if loot_screen.blocks_actions(real.elapsed_secs()) && !action.owns_body() {
        if loot_screen.locked() {
            *intent = MoveIntent::default();
        }
        stamina.tick(time.delta_secs(), false);
        return;
    }
    let (evade_clips, weapon_clips, reels, weapon_ready) = reference;
    let (defense, scripted_guard, scripted_parry, parry_after, mut next_parry, mut reactive_parry) =
        defense;
    let dt = time.delta_secs();
    let now = real.elapsed_secs();
    // Live presses only drive the body under the orbit camera, never in an unattended run.
    let live = unattended.is_none() && *mode == CamMode::Orbit && !menu.0;
    let captured = window
        .single()
        .is_ok_and(|c| c.grab_mode == bevy::window::CursorGrabMode::Locked);
    let duel = scripted.3.as_deref().copied().unwrap_or_default();
    let attack = (live && attack_pressed(&settings.binds, &keys, &buttons, &gamepads, captured))
        || duel.attack
        || scripted
            .0
            .is_some_and(|s| s.0.due(&mut next_scripted.0, now));
    let (mut telegraphs, scripted_dodge, mut dodge_due) = dodge;
    let mut dodge_stick = None;
    for t in telegraphs.read() {
        if let Some(d) = scripted_dodge.as_deref() {
            *dodge_due = Some(now + d.delay);
            info!("combat: {} winds up; evade due in {:.2} s", t.from, d.delay);
        }
        if let Some(p) = parry_after.as_deref() {
            *reactive_parry = Some(now + p.0);
        }
    }
    if let Some(d) = scripted_dodge.as_deref() {
        if dodge_due.is_some_and(|t| now >= t) {
            *dodge_due = None;
            dodge_stick = Some(xf.rotation * Vec3::new(d.push.x, 0.0, -d.push.y));
        }
    }
    let evade = dodge_stick.is_some()
        || duel.evade
        || (live && evade_pressed(&settings.binds, &keys, &buttons, &gamepads, captured))
        || scripted
            .2
            .is_some_and(|s| s.0.due(&mut next_scripted.2, now));
    let clip = anims.and_then(|a| a.find(ATTACK_1H));
    let swing_len = clip.map_or(0.8, |c| c.duration / ATTACK_RATE);
    // The swings this body can make: Dark Souls' from the cache, else the procedural one.
    let moves = weapon_clips
        .as_deref()
        .map_or_else(|| Moveset::procedural(swing_len), |d| d.moveset);
    // A heavy press needs a heavy attack, which only the Dark Souls moveset has.
    let heavy = moves.has_heavy()
        && (duel.heavy
            || (live && heavy_pressed(&settings.binds, &keys, &buttons, &gamepads, captured))
            || scripted
                .1
                .is_some_and(|s| s.0.due(&mut next_scripted.1, now)));
    let parry_due = reactive_parry.is_some_and(|at| now >= at);
    if parry_due {
        *reactive_parry = None;
    }
    let ranged = weapon_clips
        .as_ref()
        .is_some_and(|m| m.projectile.is_some());
    let parry = defense.is_some()
        && !ranged
        && !weapon_clips.as_ref().is_some_and(|m| m.two_handed)
        && ((live && parry_pressed(&settings.binds, &keys, &buttons, &gamepads, captured))
            || scripted_parry.is_some_and(|s| s.0.due(&mut next_parry, now))
            || duel.parry
            || parry_due);
    let guard = defense.is_some()
        && !ranged
        && weapon_clips
            .as_ref()
            .is_none_or(|m| m.guard.is_none_or(|g| g.2))
        && ((live && guard_held(&settings.binds, &keys, &buttons, &gamepads, captured))
            || duel.guard
            || scripted_guard.is_some_and(|s| s.0.contains(now)));
    let ready = weapon_ready.is_none_or(|r| r.0);
    let press = if parry && ready {
        Some(Press::Parry)
    } else if attack && ready {
        Some(Press::Attack)
    } else if heavy && ready {
        Some(Press::Heavy)
    } else if evade {
        Some(Press::Evade)
    } else {
        None
    };
    let input = StepInput {
        press,
        grounded: m.grounded,
        moves,
        // The controller has written the stick's camera-relative push.
        stick: dodge_stick.unwrap_or(intent.dir),
        facing: *xf.forward(),
        roll: evade_clips.as_deref().map_or(ROLL, |r| r.spec),
        evade_clips: evade_clips.is_some(),
        backstep: evade_clips
            .as_deref()
            .and_then(|r| r.back.as_ref())
            .map_or(BACKSTEP, |b| b.spec),
        clip_backstep: evade_clips.as_deref().is_some_and(|r| r.back.is_some()),
        guard,
        defense: defense.as_deref().map(|d| d.specs),
    };
    let trace = crate::actor::anim::trace_on();
    match step_action(&mut action, &stamina, input, dt) {
        Decision::Swing { kind, chained } => {
            let spec = moves
                .spec(kind)
                .expect("a swing that started is in the moveset");
            stamina.spend(spec.cost);
            // The procedural swing plays the WoW clip; a Dark Souls move poses the rig itself
            // ([`crate::motion_clip`]).
            if !moves.reference {
                if let (Some(c), Some(mut p), Some(mut tr)) = (clip, player, transitions) {
                    play_once(&mut p, &mut tr, c.node, ATTACK_RATE, chained);
                }
            }
            if trace {
                action_trace.close(xf.translation, now);
                action_trace.0 = Some((kind.name().to_string(), xf.translation, now));
                info!(
                    "combat: move {} ({:.2} s, hit {:.3}-{:.3}), stamina {:.0}",
                    kind.name(),
                    spec.duration,
                    spec.hit.0,
                    spec.hit.1,
                    stamina.cur
                );
            }
        }
        Decision::Evade(kind) => {
            stamina.spend(action.evading().map_or(kind.spec().cost, |e| e.spec.cost));
            let e = *action.evading().expect("an evade just started");
            if kind == EvadeKind::Roll {
                // The roll turns the body to where it goes; a backstep keeps its facing.
                xf.rotation = Quat::from_rotation_y(crate::controller::yaw_of(e.dir));
            }
            // The DS roll poses the rig from its own clip ([`crate::motion_clip`]).
            if !e.reference {
                if let (Some(a), Some(mut p), Some(mut tr)) = (anims, player, transitions) {
                    play_evade(a, &mut p, &mut tr, kind);
                }
            }
            if trace {
                action_trace.close(xf.translation, now);
                action_trace.0 = Some((format!("{kind:?}"), xf.translation, now));
                info!(
                    "combat: {kind:?} toward {:.2?}, stamina {:.0}",
                    e.dir, stamina.cur
                );
            }
        }
        Decision::Defense(kind) => {
            if trace {
                action_trace.close(xf.translation, now);
            }
            if kind == crate::defense::Kind::Parry {
                if let ActionState::Defend(d) = action.state {
                    stamina.spend(d.spec.cost);
                }
            }
            info!("combat: defense {} stamina {:.1}", kind.name(), stamina.cur);
        }
        Decision::Ended => {
            if trace {
                action_trace.close(xf.translation, now);
                info!("combat: action ended");
            }
        }
        Decision::None => {}
    }
    stamina.tick(dt, action.owns_body());
    match action.state {
        ActionState::Attack(s) => {
            let project_clip = weapon_clips
                .as_deref()
                .and_then(|d| d.clip(s.kind))
                .filter(|_| s.reference);
            *intent = match project_clip {
                // A Dark Souls move: this frame's share of its clip's root motion, along the
                // facing, recoil included. The motor stops it at a body.
                Some(c) => {
                    let prev = (s.t - dt).max(0.0);
                    let step = (crate::motion_clip::progress(&c.wow, s.t)
                        - crate::motion_clip::progress(&c.wow, prev))
                        * xf.scale.y;
                    root_step(*xf.forward(), step, dt)
                }
                // The procedural swing carries the body a little forward through its wind-up and
                // strike, but not into an enemy already in reach: pressed point-blank, the arc
                // passes over a short one.
                None => {
                    let in_reach = targets.iter().any(|(at, hurt)| {
                        enemy_in_reach(xf.translation, *xf.forward(), at.translation(), hurt.radius)
                    });
                    let lunge =
                        !in_reach && !matches!(s.phase(), Some(SwingPhase::Recovery) | None);
                    MoveIntent {
                        dir: if lunge { *xf.forward() } else { Vec3::ZERO },
                        speed: LUNGE_SPEED,
                        jump: false,
                    }
                }
            };
        }
        ActionState::Evade(e) => {
            // This frame's share of the evade's distance, as a speed along its direction: the DS
            // roll's own root motion, or the procedural curve.
            let prev = (e.t - dt).max(0.0);
            let step = match (e.reference, e.kind, evade_clips.as_deref()) {
                // Root motion is in model units; the body's scale (1 for HumanMale) makes it world.
                (true, EvadeKind::Roll, Some(r)) => {
                    (crate::motion_clip::progress(&r.wow, e.t)
                        - crate::motion_clip::progress(&r.wow, prev))
                        * xf.scale.y
                }
                // The DS backstep's root motion runs backwards, and so does `e.dir`: the distance
                // covered along it is the clip's travel the other way.
                (
                    true,
                    EvadeKind::Backstep,
                    Some(crate::motion_clip::EvadeClips { back: Some(b), .. }),
                ) => {
                    (crate::motion_clip::progress(&b.wow, prev)
                        - crate::motion_clip::progress(&b.wow, e.t))
                        * xf.scale.y
                }
                _ => {
                    let spec = e.spec();
                    (Evade::covered(spec, e.t) - Evade::covered(spec, prev)) * spec.distance
                }
            };
            *intent = MoveIntent {
                dir: if step > 0.0 { e.dir } else { Vec3::ZERO },
                speed: if dt > 0.0 { step / dt } else { 0.0 },
                jump: false,
            };
        }
        // Reeling or fallen, or committed to a cast, the body stands where it is.
        ActionState::Stagger { .. } | ActionState::Dead | ActionState::Cast(_) => {
            *intent = MoveIntent::default();
        }
        // A reaction that travels (a knockdown) carries the body through its clip; see [`reel_step`].
        ActionState::Reel(r) => *intent = reel_step(&r, reels.as_deref(), &xf, dt),
        ActionState::Defend(d) => {
            if d.kind.guarding() || d.kind == crate::defense::Kind::Lower {
                intent.speed = intent.speed.min(crate::motor::WALK_SPEED);
                intent.jump = false;
            } else if matches!(d.kind, crate::defense::Kind::Break(_)) {
                *intent = defense.as_deref().and_then(|k| k.clip(d.kind)).map_or_else(
                    MoveIntent::default,
                    |c| {
                        let motion = crate::motion_clip::root_offset(&c.wow, d.t)
                            - crate::motion_clip::root_offset(&c.wow, (d.t - dt).max(0.0));
                        let world = xf.rotation * Vec3::new(motion.x, 0.0, motion.y) * xf.scale.y;
                        root_step(world.normalize_or_zero(), world.length(), dt)
                    },
                );
            } else {
                *intent = MoveIntent::default();
            }
        }
        ActionState::Idle => {}
    }
}

/// A reaction's intent: a knockdown is thrown across the ground by this frame's share of its clip's
/// root motion (the clip's own, in the body's frame, turned into the world's), a flinch stands where
/// it is.
fn reel_step(
    r: &Reel,
    kit: Option<&crate::reaction::PlayerReacts>,
    xf: &Transform,
    dt: f32,
) -> MoveIntent {
    let Some(c) = kit
        .and_then(|k| k.reacts.get(r.clip))
        .filter(|c| c.travels())
    else {
        return MoveIntent::default();
    };
    let prev = (r.t - dt).max(0.0);
    let d = crate::motion_clip::root_offset(&c.wow, r.t)
        - crate::motion_clip::root_offset(&c.wow, prev);
    let world = xf.rotation * Vec3::new(d.x, 0.0, d.y) * xf.scale.y;
    root_step(world.normalize_or_zero(), world.length(), dt)
}

/// Plays an evade's clip: the roll tumbles in the crouch, the backstep sways once across its
/// length. A model without the clip stands through it.
fn play_evade(
    anims: &ModelAnimations,
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    kind: EvadeKind,
) {
    let (id, repeat) = match kind {
        EvadeKind::Roll => (ROLL_CROUCH, RepeatAnimation::Forever),
        EvadeKind::Backstep => (BACKSTEP_SWAY, RepeatAnimation::Never),
    };
    let Some(clip) = anims
        .find(id)
        .or_else(|| anims.find(crate::actor::anim::ids::STAND))
    else {
        return;
    };
    let rate = match kind {
        EvadeKind::Roll => 1.0,
        EvadeKind::Backstep => (clip.duration / BACKSTEP.duration).clamp(0.5, 3.0),
    };
    transitions
        .play(player, clip.node, Duration::from_secs_f32(0.06))
        .set_repeat(repeat)
        .set_speed(rate);
}

/// The model frame for a somersault turned `angle` forward: a turn about the body's right axis
/// through [`ROLL_PIVOT`] over the feet. Forward is `-Z`, so the head leads toward `-Z`.
pub fn tumble_transform(angle: f32) -> Transform {
    let turn = Quat::from_rotation_x(-angle);
    let pivot = Vec3::Y * ROLL_PIVOT;
    Transform {
        translation: pivot - turn * pivot,
        rotation: turn,
        scale: Vec3::ONE,
    }
}

/// Turns the avatar's model through the roll's somersault; identity otherwise.
fn tumble(
    avatar: Query<(&Action, &ModelFrame, Option<&crate::motor::swim::Swimming>), With<Avatar>>,
    mut frames: Query<&mut Transform>,
) {
    for (action, frame, swimming) in &avatar {
        let angle = action.evading().map_or(0.0, Evade::tumble);
        // A swimmer diving or climbing leans the model along the stroke, about the chest.
        let lean = swimming.filter(|s| s.on).map_or(0.0, |s| s.pitch);
        let want = if angle != 0.0 {
            tumble_transform(angle)
        } else if lean != 0.0 {
            let turn = Quat::from_rotation_x(lean);
            let pivot = Vec3::Y * 1.0;
            Transform {
                translation: pivot - turn * pivot,
                rotation: turn,
                scale: Vec3::ONE,
            }
        } else {
            Transform::IDENTITY
        };
        if let Ok(mut xf) = frames.get_mut(frame.0) {
            if *xf != want {
                *xf = want;
            }
        }
    }
}

/// Within this gap (yd) between the body and an enemy's hurtbox ahead, the swing stops lunging.
/// Measured against the kobold's posed body capsules ([`crate::hitbody`]): the light attack
/// connects from 1.1 to 1.6 yd centre to centre, best near 1.35 (this gap); at 1.6 it grazes, and
/// a kobold flinching back from 1.6 is out of reach of the follow-up.
pub const LUNGE_STOP_GAP: f32 = 0.6;

/// An enemy whose hurtbox (centre `at`, radius `radius`) is ahead of a body at `from` facing
/// `fwd`, within [`LUNGE_STOP_GAP`].
pub fn enemy_in_reach(from: Vec3, fwd: Vec3, at: Vec3, radius: f32) -> bool {
    let d = at - from;
    let flat = Vec3::new(d.x, 0.0, d.z);
    let gap = flat.length() - radius - crate::motor::CAPSULE_RADIUS;
    gap <= LUNGE_STOP_GAP && flat.normalize_or_zero().dot(fwd) > 0.5
}

/// Plays a clip once from its start; a chained swing of the same clip restarts it in place.
fn play_once(
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    node: AnimationNodeIndex,
    rate: f32,
    restart: bool,
) {
    if restart {
        if let Some(active) = player.animation_mut(node) {
            active.replay();
            active.set_speed(rate);
            return;
        }
    }
    transitions
        .play(player, node, Duration::from_secs_f32(0.1))
        .set_repeat(RepeatAnimation::Never)
        .set_speed(rate);
}

/// `WOWSOULS_TRACE`: the action, its phase and stamina, four times a second.
fn trace(
    time: Res<Time<bevy::time::Real>>,
    avatar: Query<(&Action, &Stamina), With<Avatar>>,
    mut next_at: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now < *next_at {
        return;
    }
    *next_at = now + 0.25;
    if let Ok((a, s)) = avatar.single() {
        let phase = match a.state {
            ActionState::Attack(sw) => {
                format!("{} {:?} t={:.2}", sw.kind.name(), sw.phase(), sw.t)
            }
            ActionState::Evade(e) => format!(
                "{:?} t={:.2} turn={:.0}deg invuln={}",
                e.kind,
                e.t,
                e.tumble().to_degrees(),
                a.invulnerable()
            ),
            ActionState::Stagger { t, duration } => format!("stagger t={t:.2}/{duration:.2}"),
            ActionState::Reel(r) => {
                format!("reel clip {} t={:.2}/{:.2}", r.clip, r.t, r.spec.duration)
            }
            ActionState::Dead => "dead".into(),
            ActionState::Defend(d) => format!(
                "defense {} t={:.2} active={}",
                d.kind.name(),
                d.t,
                d.active()
            ),
            ActionState::Cast(c) => {
                format!("cast {} t={:.2}/{:.2}", c.spell, c.t, c.spec.duration)
            }
            ActionState::Idle => "idle".into(),
        };
        info!("action t={now:6.2} {phase} stamina={:.1}", s.cur);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    fn full() -> Stamina {
        Stamina::full(STAMINA_MAX)
    }

    #[test]
    fn the_somersault_leads_with_the_head_and_keeps_the_pivot() {
        let head = Vec3::Y * 1.3;
        // A quarter turn in, the head is ahead (-Z) of the body.
        let q = tumble_transform(std::f32::consts::FRAC_PI_2).transform_point(head);
        assert!(q.z < -0.5 && (q.y - ROLL_PIVOT).abs() < 1.0e-5, "{q}");
        // The pivot stays put, and a full turn is home.
        let p = Vec3::Y * ROLL_PIVOT;
        assert!((tumble_transform(1.0).transform_point(p) - p).length() < 1.0e-5);
        let home = tumble_transform(std::f32::consts::TAU).transform_point(head);
        assert!((home - head).length() < 1.0e-4, "{home}");
    }

    #[test]
    fn a_dodge_parses_a_delay_and_an_optional_push() {
        assert_eq!(
            parse_dodge("0.3"),
            Some(ScriptedDodge {
                delay: 0.3,
                push: Vec2::ZERO
            })
        );
        assert_eq!(parse_dodge("0.3,-1,0").unwrap().push, Vec2::NEG_X);
        assert_eq!(parse_dodge("0.3,1"), None);
        assert_eq!(parse_dodge("-1"), None);
    }

    #[test]
    fn schedules_parse_a_time_and_an_optional_period() {
        assert_eq!(
            parse_schedule("3"),
            Some(Schedule {
                at: 3.0,
                every: None,
                until: None
            })
        );
        assert_eq!(
            parse_schedule("3, 1.5"),
            Some(Schedule {
                at: 3.0,
                every: Some(1.5),
                until: None
            })
        );
        // A third number ends the presses.
        let s = parse_schedule("1,1,2.5").unwrap();
        let mut next = None;
        assert!(s.due(&mut next, 1.0));
        assert!(s.due(&mut next, 2.0));
        assert!(!s.due(&mut next, 3.0), "past the end");
        assert_eq!(parse_schedule("3,1,2"), None, "an end before the start");
        assert_eq!(parse_schedule("3,0"), None);
        assert_eq!(parse_schedule("x"), None);
        let s = parse_schedule("1,2").unwrap();
        let mut next = None;
        assert!(!s.due(&mut next, 0.5));
        assert!(s.due(&mut next, 1.0));
        assert!(!s.due(&mut next, 2.0));
        assert!(s.due(&mut next, 3.0));
    }

    #[test]
    fn the_lunge_stops_short_of_an_enemy_ahead() {
        let fwd = Vec3::NEG_Z;
        // 1.2 yd ahead, hurtbox 0.4: a 0.47 yd gap, in reach.
        assert!(enemy_in_reach(
            Vec3::ZERO,
            fwd,
            Vec3::new(0.0, 0.0, -1.2),
            0.4
        ));
        // 3 yd ahead: lunge on.
        assert!(!enemy_in_reach(
            Vec3::ZERO,
            fwd,
            Vec3::new(0.0, 0.0, -3.0),
            0.4
        ));
        // Close but behind: lunge on.
        assert!(!enemy_in_reach(
            Vec3::ZERO,
            fwd,
            Vec3::new(0.0, 0.0, 1.0),
            0.4
        ));
    }

    #[test]
    fn stamina_waits_then_regenerates_and_caps() {
        let mut s = full();
        s.spend(50.0);
        assert_eq!(s.cur, 50.0);
        s.tick(STAMINA_DELAY * 0.5, false);
        assert_eq!(s.cur, 50.0, "no regen inside the delay");
        for _ in 0..60 {
            s.tick(DT, false);
        }
        assert!(s.cur > 50.0 && s.cur < STAMINA_MAX, "{}", s.cur);
        for _ in 0..600 {
            s.tick(DT, false);
        }
        assert_eq!(s.cur, STAMINA_MAX);
    }

    #[test]
    fn a_running_action_holds_regeneration() {
        let mut s = full();
        s.spend(50.0);
        for _ in 0..120 {
            s.tick(DT, true);
        }
        assert_eq!(s.cur, 50.0);
    }

    #[test]
    fn spending_floors_at_zero_but_any_stamina_starts_an_action() {
        let mut s = full();
        s.spend(95.0);
        assert!(s.can_spend());
        s.spend(ATTACK_COST);
        assert_eq!(s.cur, 0.0);
        assert!(!s.can_spend());
    }

    fn keys_with(pressed: &[KeyCode]) -> ButtonInput<KeyCode> {
        let mut keys = ButtonInput::default();
        for &k in pressed {
            keys.press(k);
        }
        keys
    }

    fn mouse_with(pressed: &[MouseButton]) -> ButtonInput<MouseButton> {
        let mut mouse = ButtonInput::default();
        for &b in pressed {
            mouse.press(b);
        }
        mouse
    }

    /// The attack press for these inputs and a pad that `setup` presses (none: a world without).
    fn attack_for(
        keys: ButtonInput<KeyCode>,
        mouse: ButtonInput<MouseButton>,
        captured: bool,
        pad: Option<GamepadButton>,
    ) -> bool {
        use crate::pad::testkit::{read, world_with_pad};
        let mut world = match pad {
            Some(b) => world_with_pad(move |p| {
                p.digital_mut().press(b);
            }),
            None => World::new(),
        };
        read(&mut world, move |pads| {
            attack_pressed(&Binds::default(), &keys, &mouse, pads, captured)
        })
    }

    #[test]
    fn attack_is_f_the_left_mouse_or_the_right_bumper() {
        let none = || ButtonInput::<KeyCode>::default();
        let no_mouse = ButtonInput::<MouseButton>::default;
        // F works with the cursor free: it needs no click to capture.
        assert!(attack_for(
            keys_with(&[KeyCode::KeyF]),
            no_mouse(),
            false,
            None
        ));
        // The left mouse counts only with the cursor captured (the capturing click is no swing).
        let lmb = || mouse_with(&[MouseButton::Left]);
        assert!(!attack_for(none(), lmb(), false, None));
        assert!(attack_for(none(), lmb(), true, None));
        // The right bumper (RB) is R1.
        assert!(attack_for(
            none(),
            no_mouse(),
            false,
            Some(GamepadButton::RightTrigger)
        ));
        // Other buttons and no input are no attack.
        assert!(!attack_for(
            none(),
            no_mouse(),
            true,
            Some(GamepadButton::South)
        ));
        assert!(!attack_for(none(), no_mouse(), true, None));
    }

    fn evade_for(keys: ButtonInput<KeyCode>, pad: Option<GamepadButton>) -> bool {
        use crate::pad::testkit::{read, world_with_pad};
        let mut world = match pad {
            Some(b) => world_with_pad(move |p| {
                p.digital_mut().press(b);
            }),
            None => World::new(),
        };
        read(&mut world, move |pads| {
            evade_pressed(
                &Binds::default(),
                &keys,
                &ButtonInput::<MouseButton>::default(),
                pads,
                false,
            )
        })
    }

    #[test]
    fn evade_is_space_or_b_and_shift_space_is_the_jump() {
        assert!(evade_for(keys_with(&[KeyCode::Space]), None));
        assert!(!evade_for(
            keys_with(&[KeyCode::Space, KeyCode::ShiftLeft]),
            None
        ));
        assert!(evade_for(keys_with(&[]), Some(GamepadButton::East)));
        assert!(!evade_for(keys_with(&[]), Some(GamepadButton::South)));
    }

    fn heavy_for(
        keys: ButtonInput<KeyCode>,
        mouse: ButtonInput<MouseButton>,
        captured: bool,
        pad: Option<GamepadButton>,
    ) -> bool {
        use crate::pad::testkit::{read, world_with_pad};
        let mut world = match pad {
            Some(b) => world_with_pad(move |p| {
                p.digital_mut().press(b);
            }),
            None => World::new(),
        };
        read(&mut world, move |pads| {
            heavy_pressed(&Binds::default(), &keys, &mouse, pads, captured)
        })
    }

    #[test]
    fn heavy_is_r_the_right_mouse_or_the_right_trigger() {
        let none = || ButtonInput::<KeyCode>::default();
        let no_mouse = ButtonInput::<MouseButton>::default;
        assert!(heavy_for(
            keys_with(&[KeyCode::KeyR]),
            no_mouse(),
            false,
            None
        ));
        // The right mouse counts only with the cursor captured, as the left does.
        let rmb = || mouse_with(&[MouseButton::Right]);
        assert!(!heavy_for(none(), rmb(), false, None));
        assert!(heavy_for(none(), rmb(), true, None));
        assert!(heavy_for(
            none(),
            no_mouse(),
            false,
            Some(GamepadButton::RightTrigger2)
        ));
        // The right bumper is the light attack, not the heavy one.
        assert!(!heavy_for(
            none(),
            no_mouse(),
            true,
            Some(GamepadButton::RightTrigger)
        ));
        assert!(!heavy_for(none(), no_mouse(), true, None));
    }

    #[test]
    fn root_motion_steps_back_along_the_facing_without_turning_it() {
        let near = |a: f32, b: f32| (a - b).abs() < 1.0e-3;
        let fwd = Vec3::NEG_Z;
        let go = root_step(fwd, 0.1, 0.02);
        assert_eq!(go.dir, fwd);
        assert!(near(go.speed, 5.0), "{go:?}");
        // Recoil: the same direction, a negative speed.
        let back = root_step(fwd, -0.1, 0.02);
        assert_eq!(back.dir, fwd);
        assert!(near(back.speed, -5.0), "{back:?}");
        // No step: no push at all.
        assert_eq!(root_step(fwd, 0.0, 0.02).dir, Vec3::ZERO);
        // A hitch longer than the motor integrates still delivers its step.
        let hitch = root_step(fwd, 0.2, 0.5);
        assert!(near(hitch.speed * crate::motor::MAX_STEP, 0.2), "{hitch:?}");
        // A frame of no time asks for nothing.
        assert_eq!(root_step(fwd, 0.1, 0.0).speed, 0.0);
    }
}
