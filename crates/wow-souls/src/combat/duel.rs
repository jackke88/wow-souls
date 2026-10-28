//! `WOWSOULS_DUEL=<seed>`: a sparring bot for recordings and unattended checks. It reads the
//! nearest creature's swing the way a player reads a wind-up (the kit's hit windows against the
//! swing's clock), answers it before the blow lands with a guard, a parry, a roll or a backstep,
//! and steps in to strike when the creature is open. It presses only what a player can press:
//! [`DuelCall`] is read by [`super::act`] beside the live and the scripted presses, and the roll's
//! direction goes through the stick ([`MoveIntent`]). The seed shifts which answer a move gets
//! first, so two seeds give two different-looking fights.

use bevy::prelude::*;

use super::{Action, Stamina};
use crate::avatar::Avatar;
use crate::hits::Hurtbox;
use crate::monster::{AttackClip, Roster};
use crate::motor::MoveIntent;
use crate::npc::{Brain, Mind, Reaction};
use crate::species::Species;

/// How far a creature has to be (yd) for the bot to take notice.
const NOTICE: f32 = 16.0;
/// Seconds before a blow lands at which a roll starts: the roll's invulnerability opens just
/// after the press, so the body is inside it when the blow arrives.
const ROLL_LEAD: f32 = 0.30;
/// The same for the backstep, which has a shorter invulnerable window.
const BACK_LEAD: f32 = 0.22;
/// And for a parry press: the parry's catch window opens fast.
const PARRY_LEAD: f32 = 0.16;
/// The guard goes up this early and stays up this long after the last blow.
const GUARD_LEAD: f32 = 0.55;
const GUARD_TAIL: f32 = 0.14;
/// Seconds between two strikes of the bot, and the ones it takes a creature's open moment for.
const STRIKE_GAP: f32 = 0.62;
/// The bot does not start a swing when a blow is due within this long (it would be caught mid-swing).
const SAFE_SPAN: f32 = 0.85;

/// What the bot presses this frame.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct DuelCall {
    pub attack: bool,
    pub heavy: bool,
    pub evade: bool,
    pub guard: bool,
    pub parry: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Reply {
    Guard,
    Parry,
    /// A roll to one side of the line to the creature (+1 its right-hand side, -1 the other).
    Roll(f32),
    /// A roll away from the creature and to one side: out of reach of a combo's later blows.
    Away(f32),
    Back,
}

#[derive(Resource)]
pub struct Duel {
    seed: u32,
    /// Swings answered so far.
    seen: u32,
    /// The answer to the swing under way, and the hit windows already answered.
    reply: Option<Reply>,
    fired: u32,
    /// Strikes made since the creature last began a swing.
    cycle: u32,
    /// The clock the next strike may start at, and the strikes made.
    next_strike: f32,
    strikes: u32,
    /// The clock of the last strike (or of the start): a fight that goes long without one opens up.
    last_strike: f32,
}

impl Duel {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            seen: 0,
            reply: None,
            fired: 0,
            cycle: 0,
            next_strike: 0.0,
            strikes: 0,
            last_strike: 0.0,
        }
    }

    /// The answer to the `n`th swing, a `mv` called `name`: every move gets the answer that
    /// shows it best, the quick ones a parry first.
    fn choose(&self, name: &str, how: Option<(&AttackClip, bool)>) -> Reply {
        let n = self.seen.wrapping_add(self.seed);
        let side = if n % 2 == 0 { 1.0 } else { -1.0 };
        // Garrick's own moves, picked out by name.
        match name {
            "quick" if n % 3 != 1 => return Reply::Parry,
            "quick" => return Reply::Guard,
            "double" => return Reply::Parry,
            "triple" => return Reply::Away(side),
            "lunge" | "dash" => return Reply::Roll(side),
            _ => {}
        }
        let Some((a, can_parry)) = how else {
            return Reply::Guard;
        };
        // A bolt is sidestepped, a blast or a ground shock is left behind, a blow that takes a
        // third of the body is rolled; the rest is parried and blocked in turn.
        if a.bullet.is_some() {
            return Reply::Roll(side);
        }
        if a.shock.is_some() || a.blast.is_some() {
            return Reply::Away(side);
        }
        if a.reach.is_some() || a.spec.damage >= 30.0 {
            return Reply::Roll(side);
        }
        let parry = if can_parry {
            Reply::Parry
        } else {
            Reply::Guard
        };
        if a.spec.hits.len() >= 2 {
            return if n % 2 == 0 { parry } else { Reply::Away(side) };
        }
        match n % 4 {
            0 | 2 => parry,
            1 => Reply::Guard,
            _ if a.spec.hit.0 < 0.5 => Reply::Back,
            _ => Reply::Roll(side),
        }
    }
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

#[allow(clippy::too_many_arguments)]
pub fn bot(
    time: Res<Time<bevy::time::Real>>,
    roster: Option<Res<Roster>>,
    mut duel: ResMut<Duel>,
    mut call: ResMut<DuelCall>,
    mut avatar: Query<(&Transform, &Action, &Stamina, &mut MoveIntent), With<Avatar>>,
    foes: Query<(&GlobalTransform, &Hurtbox, &Brain, &Species, &Reaction), Without<Avatar>>,
) {
    *call = DuelCall::default();
    let Ok((xf, action, stamina, mut intent)) = avatar.single_mut() else {
        return;
    };
    if action.is_dead() {
        return;
    }
    let now = time.elapsed_secs();
    let me = xf.translation;
    let Some((at, hurt, brain, species, react)) = foes
        .iter()
        .filter(|(.., r)| **r != Reaction::Dead)
        .map(|(g, h, b, s, r)| (g.translation(), h, b, s, *r))
        .filter(|(p, ..)| flat(*p - me).length() < NOTICE)
        .min_by(|a, b| flat(a.0 - me).length().total_cmp(&flat(b.0 - me).length()))
    else {
        duel.reply = None;
        return;
    };
    let to = flat(at - me);
    let dist = to.length();
    let toward = to.normalize_or_zero();
    let right = Vec3::new(-toward.z, 0.0, toward.x);

    // The swing under way: the blows still to come (window, seconds until it lands).
    let mut windows: Vec<(u32, f32)> = Vec::new();
    let mut last_end = f32::NEG_INFINITY;
    let mut name = "";
    let mut how: Option<(&AttackClip, bool)> = None;
    let swinging = matches!(brain.mind, Mind::Attack { .. });
    if let Mind::Attack { t, mv } = brain.mind {
        let kit = roster.as_deref().and_then(|r| r.get(species.kit()));
        if let Some((a, k)) = kit.and_then(|k| Some((k.attacks.get(usize::from(mv))?, k))) {
            name = a.spec.name.as_str();
            how = Some((a, k.parried.is_some()));
            let single = [a.spec.hit];
            let hits: &[(f32, f32)] = if a.spec.hits.is_empty() {
                &single
            } else {
                &a.spec.hits
            };
            for (i, w) in hits.iter().enumerate() {
                if w.1 > t {
                    last_end = last_end.max(w.1 - t);
                    if brain.struck & (1 << i) == 0 {
                        windows.push((i as u32, (w.0 - t).max(0.0)));
                    }
                }
            }
        }
        if duel.reply.is_none() {
            let mut r = duel.choose(name, how);
            // A guard that is paid for in stamina the body does not have is no answer; mid-swing
            // there is only the roll that cancels it.
            if matches!(r, Reply::Guard) && stamina.cur < 38.0 || action.owns_body() {
                r = Reply::Roll(if duel.seen % 2 == 0 { 1.0 } else { -1.0 });
            }
            duel.reply = Some(r);
            duel.fired = 0;
            duel.cycle = 0;
            duel.seen += 1;
            info!(
                "duel: {}/{name} comes, answer {r:?}, stamina {:.0}",
                species.0.name, stamina.cur
            );
        }
    } else {
        duel.reply = None;
    }
    let next = windows.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));

    // Answer the swing: each blow once, a roll or a parry per blow, a guard held through.
    let mut busy = false;
    if let (Some(reply), Some((idx, until))) = (duel.reply, next) {
        let fresh = duel.fired & (1 << idx) == 0;
        match reply {
            Reply::Guard => {
                if until < GUARD_LEAD || last_end > 0.0 && until < 1.2 {
                    call.guard = true;
                    busy = true;
                }
            }
            Reply::Parry => {
                if until <= PARRY_LEAD && fresh {
                    call.parry = true;
                    duel.fired |= 1 << idx;
                }
                busy = until < 0.6;
            }
            Reply::Roll(side) => {
                if until <= ROLL_LEAD && fresh && stamina.can_spend() {
                    call.evade = true;
                    intent.dir = (right * side + toward * 0.25).normalize_or_zero();
                    duel.fired |= 1 << idx;
                }
                busy = until < 0.7;
            }
            Reply::Away(side) => {
                if until <= ROLL_LEAD && fresh && stamina.can_spend() {
                    call.evade = true;
                    intent.dir = (right * side * 0.7 - toward).normalize_or_zero();
                    duel.fired |= 1 << idx;
                }
                busy = until < 0.7;
            }
            Reply::Back => {
                if until <= BACK_LEAD && fresh && stamina.can_spend() {
                    call.evade = true;
                    intent.dir = Vec3::ZERO;
                    duel.fired |= 1 << idx;
                }
                busy = until < 0.6;
            }
        }
    } else if matches!(duel.reply, Some(Reply::Guard)) && last_end > -GUARD_TAIL && swinging {
        // The last blow is past: the guard comes down a beat after it.
        call.guard = true;
        busy = true;
    }

    // Offence and feet: strike once in an open moment with stamina to spare, step in when out of
    // reach, hold still when in it.
    let reach = super::enemy_in_reach(me, *xf.forward(), at, hurt.radius);
    // A swing lasts 2.5 s and only a roll cancels it after 0.9 s: the bot strikes only when the
    // creature is staggered (a parry, a broken swing) or well into its pause, never while it is
    // about to swing.
    // A fight that has gone 6 s without a blow of ours takes a shorter pause too, so a quick
    // creature is struck as well.
    let hungry = now - duel.last_strike > 6.0;
    let open = match (brain.mind, react) {
        (_, Reaction::Flinch(left)) => left > 0.6,
        (Mind::Cooldown { left }, _) => left > if hungry { 0.6 } else { 1.1 },
        (Mind::Chase | Mind::Idle, _) => hungry,
        _ => false,
    };
    if !call.evade && !action.owns_body() && !busy {
        if reach {
            intent.dir = Vec3::ZERO;
        } else if dist > 1.2 && !(swinging && windows.iter().any(|w| w.1 < SAFE_SPAN)) {
            intent.dir = toward;
        }
    }
    if open
        && !busy
        && reach
        && !call.guard
        && stamina.cur >= 62.0
        && duel.cycle < 2
        && now >= duel.next_strike
        && !action.owns_body()
    {
        duel.strikes += 1;
        duel.last_strike = now;
        duel.cycle += 1;
        duel.next_strike = now + STRIKE_GAP;
        if duel.strikes % 4 == 0 {
            call.heavy = true;
        } else {
            call.attack = true;
        }
    }
}
