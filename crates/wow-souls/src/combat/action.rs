//! The pure action machine: what a body is committed to (a swing, an evade, a stagger, death),
//! which presses it accepts when, and how a press becomes a decision. No Bevy systems here: the
//! caller ([`super`]) feeds it a frame of input and carries out the decision.

use bevy::prelude::*;

use super::{Stamina, ATTACK_COST};

/// Swing windows as fractions of its length: before [`ACTIVE_FROM`] the wind-up, until
/// [`ACTIVE_TO`] the blade can hit, after [`CANCEL_FROM`] a buffered follow-up may start.
pub const ACTIVE_FROM: f32 = 0.30;
pub const ACTIVE_TO: f32 = 0.55;
pub const CANCEL_FROM: f32 = 0.70;
/// How long a press waits for the action to accept it (s).
pub const BUFFER: f32 = 0.35;

/// A stick push over this aims a roll; under it the evade is a backstep.
pub const ROLL_PUSH: f32 = 0.2;

/// An evade's shape: times (s), distance (yd) and stamina.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvadeSpec {
    pub duration: f32,
    /// The body travels over `[0, travel]`, then recovers in place.
    pub travel: f32,
    pub distance: f32,
    /// Hits pass through over `[from, to)`.
    pub iframes: (f32, f32),
    /// A buffered action may start from here.
    pub cancel_from: f32,
    pub cost: f32,
}

/// The roll, in Dark Souls medium-roll proportions: about 4 yd in 0.6 s, a third of a second of
/// invulnerability from the second frame, then a short recovery.
pub const ROLL: EvadeSpec = EvadeSpec {
    duration: 0.8,
    travel: 0.6,
    distance: 4.0,
    iframes: (0.05, 0.42),
    cancel_from: 0.6,
    cost: 20.0,
};

/// The backstep: a short hop back with a brief invulnerability.
pub const BACKSTEP: EvadeSpec = EvadeSpec {
    duration: 0.55,
    travel: 0.35,
    distance: 1.8,
    iframes: (0.03, 0.25),
    cancel_from: 0.4,
    cost: 15.0,
};

/// Where a swing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwingPhase {
    Startup,
    Active,
    Recovery,
}

/// How many hits the one-handed light chain has, and the heavy one.
pub const R1_CHAIN: u8 = 3;
pub const R2_CHAIN: u8 = 2;

/// Which move a swing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveKind {
    /// The light chain, hit `n` (0, 1, 2).
    R1(u8),
    /// The heavy attack and its follow-up (0, 1).
    R2(u8),
    /// The attack out of a roll or a backstep.
    RollAttack,
}

impl MoveKind {
    /// Every move, in the order of the cache's table.
    pub const ALL: [MoveKind; 6] = [
        MoveKind::R1(0),
        MoveKind::R1(1),
        MoveKind::R1(2),
        MoveKind::R2(0),
        MoveKind::R2(1),
        MoveKind::RollAttack,
    ];

    /// The name the cache's move table uses.
    pub fn name(self) -> &'static str {
        match self {
            MoveKind::R1(0) => "r1_1",
            MoveKind::R1(1) => "r1_2",
            MoveKind::R1(_) => "r1_3",
            MoveKind::R2(0) => "r2_1",
            MoveKind::R2(_) => "r2_2",
            MoveKind::RollAttack => "roll_attack",
        }
    }

    pub fn from_name(name: &str) -> Option<MoveKind> {
        MoveKind::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// A swing's timing and price, in seconds from its start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwingSpec {
    pub duration: f32,
    /// The blade can hit over `[hit.0, hit.1)`.
    pub hit: (f32, f32),
    /// A buffered attack may start from here (Dark Souls' action flag 4).
    pub chain_from: f32,
    /// A buffered evade may start from here (flag 26).
    pub evade_from: f32,
    /// A pushed stick ends the swing from here (flag 11).
    pub move_from: f32,
    pub cost: f32,
    /// The damage of a hit, as a multiple of the base hit.
    pub mult: f32,
    /// The flinch level of the hit (`AtkParam_Pc.dmgLevel`) and its poise damage
    /// (`atkSuperArmor`).
    pub level: u8,
    pub poise: f32,
    /// The hit-stop the blow freezes the world for (s, `hitStopTime`) and how far it pushes its
    /// target (Dark Souls metres, `knockbackDist`).
    pub stop: f32,
    pub knock: f32,
    /// The thickness of the blade's hit volume (yd; `hit0_Radius`): Dark Souls' blades are fat, which
    /// is how a swing reaches a small enemy.
    pub radius: f32,
}

impl SwingSpec {
    /// Today's procedural swing of length `len`: its phases and its cancel are fractions of it,
    /// and only an evade or the next press interrupts it.
    pub fn procedural(len: f32) -> SwingSpec {
        SwingSpec {
            duration: len,
            hit: (ACTIVE_FROM * len, ACTIVE_TO * len),
            chain_from: CANCEL_FROM * len,
            evade_from: ACTIVE_TO * len,
            move_from: len,
            cost: ATTACK_COST,
            mult: 1.0,
            level: 1,
            poise: 0.0,
            stop: crate::hits::HIT_STOP,
            knock: 0.0,
            radius: crate::hits::BLADE_RADIUS,
        }
    }
}

/// The swings a body can make.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moveset {
    pub chain_len: u8,
    pub r1: [SwingSpec; R1_CHAIN as usize],
    pub r2: Option<[SwingSpec; R2_CHAIN as usize]>,
    pub roll_attack: Option<SwingSpec>,
    /// The specs come from Dark Souls clips, which pose the rig and move the body.
    pub reference: bool,
}

impl Moveset {
    /// Today's: one light swing of length `len`, no heavy attack, no roll attack.
    pub fn procedural(len: f32) -> Moveset {
        Moveset {
            chain_len: 3,
            r1: [SwingSpec::procedural(len); R1_CHAIN as usize],
            r2: None,
            roll_attack: None,
            reference: false,
        }
    }

    pub fn spec(&self, kind: MoveKind) -> Option<SwingSpec> {
        match kind {
            MoveKind::R1(n) => self.r1.get(usize::from(n)).copied(),
            MoveKind::R2(n) => self.r2.and_then(|r| r.get(usize::from(n)).copied()),
            MoveKind::RollAttack => self.roll_attack,
        }
    }

    /// There is a heavy attack to press.
    pub fn has_heavy(&self) -> bool {
        self.r2.is_some()
    }
}

/// A swing in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swing {
    pub t: f32,
    pub kind: MoveKind,
    pub spec: SwingSpec,
    /// From a Dark Souls clip: it poses the rig and moves the body.
    pub reference: bool,
    /// The number of this swing since the body was made: a new serial may strike everyone again.
    pub serial: u32,
}

impl Swing {
    /// The phase now; `None` once the swing is over.
    pub fn phase(&self) -> Option<SwingPhase> {
        let s = &self.spec;
        if self.t >= s.duration {
            None
        } else if self.t < s.hit.0 {
            Some(SwingPhase::Startup)
        } else if self.t < s.hit.1 {
            Some(SwingPhase::Active)
        } else {
            Some(SwingPhase::Recovery)
        }
    }

    /// A buffered attack may start now.
    pub fn cancellable(&self) -> bool {
        self.t >= self.spec.chain_from
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvadeKind {
    Roll,
    Backstep,
}

impl EvadeKind {
    pub fn spec(self) -> &'static EvadeSpec {
        match self {
            EvadeKind::Roll => &ROLL,
            EvadeKind::Backstep => &BACKSTEP,
        }
    }
}

/// An evade in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Evade {
    pub kind: EvadeKind,
    pub t: f32,
    /// The ground direction of travel, unit length.
    pub dir: Vec3,
    /// Its shape, fixed when it starts (the DS roll's comes from its clip).
    pub spec: EvadeSpec,
    /// Played from the Dark Souls clip ([`crate::motion_clip`]): no somersault, root-motion travel.
    pub reference: bool,
}

impl Evade {
    pub fn spec(&self) -> &EvadeSpec {
        &self.spec
    }

    pub fn invulnerable(&self) -> bool {
        let (from, to) = self.spec().iframes;
        self.t >= from && self.t < to
    }

    /// The share of the distance covered `t` seconds in: `(3u - u³) / 2` over the travel, a
    /// speed of `1.5 (1 - u²)` times the average, fast off the mark and easing to a stop.
    pub fn covered(spec: &EvadeSpec, t: f32) -> f32 {
        let u = (t / spec.travel).clamp(0.0, 1.0);
        (3.0 * u - u * u * u) * 0.5
    }

    /// The somersault's forward turn (rad): one full turn over the travel, eased in and out; a
    /// backstep does not turn.
    pub fn tumble(&self) -> f32 {
        match self.kind {
            EvadeKind::Roll if !self.reference => {
                let u = (self.t / self.spec().travel).clamp(0.0, 1.0);
                std::f32::consts::TAU * u * u * (3.0 - 2.0 * u)
            }
            _ => 0.0,
        }
    }
}

/// A reaction's timing, in seconds from its start (from the clip's TAE flags): when the body may
/// attack, evade and move again, when it ends, and the window it cannot be hit in (a knockdown).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReelSpec {
    pub duration: f32,
    /// A buffered attack may start from here (Dark Souls' action flag 4).
    pub chain_from: f32,
    /// A buffered evade may start from here (flag 26).
    pub evade_from: f32,
    /// A pushed stick ends the reaction from here (flag 11).
    pub move_from: f32,
    /// Hits pass through the body over this window (flag 8, the knockdowns').
    pub invuln: Option<(f32, f32)>,
}

/// A reaction in progress: which clip of the kit ([`crate::reaction`]), how far in, its timing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reel {
    pub t: f32,
    pub clip: usize,
    pub spec: ReelSpec,
}

impl Reel {
    pub fn invulnerable(&self) -> bool {
        self.spec
            .invuln
            .is_some_and(|(from, to)| self.t >= from && self.t < to)
    }
}

/// A spell cast's timing, in seconds from its start (from the Dark Souls cast clip's own flags): when
/// the spell leaves the hand, when the body may act again, and when the clip ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastSpec {
    pub duration: f32,
    pub release: f32,
    pub recover: f32,
}

/// A cast in progress: the spell, how far in, its timing, its serial (a new cast may release again)
/// and whom it was aimed at when the button was pressed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cast {
    pub t: f32,
    pub spell: u32,
    pub spec: CastSpec,
    pub serial: u32,
    pub target: Option<Entity>,
}

impl Cast {
    /// The spell has left the hand.
    pub fn released(&self) -> bool {
        self.t >= self.spec.release
    }

    /// A buffered action may start now: the recovery has begun.
    pub fn cancellable(&self) -> bool {
        self.t >= self.spec.recover
    }
}

/// What the body is committed to.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ActionState {
    #[default]
    Idle,
    Attack(Swing),
    Evade(Evade),
    Defend(crate::defense::Playing),
    /// Struck: the body reels for `duration` and takes no action.
    Stagger {
        t: f32,
        duration: f32,
    },
    /// Struck, with the Dark Souls reaction clip: the body reels until the clip's control flags
    /// open, as a swing's cancel windows do.
    Reel(Reel),
    /// A spell is being cast: the Dark Souls cast clip poses the body, which is committed to it.
    Cast(Cast),
    /// Out of health: the body lies where it fell until the respawn resets it.
    Dead,
}

/// Which action a press asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// R1, the light attack.
    Attack,
    /// R2, the heavy attack.
    Heavy,
    Evade,
    Parry,
}

/// A pressed action waiting to be accepted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Buffered {
    pub press: Press,
    pub age: f32,
}

#[derive(Component, Debug, Default)]
pub struct Action {
    pub state: ActionState,
    pub buffer: Option<Buffered>,
    /// Swings started so far: the last one's serial.
    pub swings: u32,
    /// The death clip (an index of the kit's deaths) and the seconds since the body fell.
    pub death: Option<usize>,
    pub dead_t: f32,
    pub guard_timing: crate::defense::GuardTiming,
    pub training: crate::defense::Training,
    pub parry_caught: bool,
}

impl Action {
    pub fn guarding(&self) -> bool {
        matches!(self.state, ActionState::Defend(d) if d.kind.guarding() || d.kind == crate::defense::Kind::Lower)
    }
    /// The action owns the body's animation and movement.
    pub fn owns_body(&self) -> bool {
        !matches!(self.state, ActionState::Idle)
    }

    pub fn evading(&self) -> Option<&Evade> {
        match &self.state {
            ActionState::Evade(e) => Some(e),
            _ => None,
        }
    }

    /// Hits pass through the body now: in a roll's i-frames, or in a knockdown's.
    pub fn invulnerable(&self) -> bool {
        self.evading().is_some_and(Evade::invulnerable)
            || matches!(&self.state, ActionState::Reel(r) if r.invulnerable())
    }

    pub fn is_dead(&self) -> bool {
        self.state == ActionState::Dead
    }

    /// A cast may start now: the body is free, or an action is in its cancel window.
    pub fn can_cast(&self) -> bool {
        self.accepts(Press::Attack)
    }

    /// Starts a cast of `spell`; `false` (nothing changes) when the body cannot take one now.
    pub fn begin_cast(&mut self, spell: u32, spec: CastSpec, target: Option<Entity>) -> bool {
        if !self.can_cast() {
            return false;
        }
        self.swings += 1;
        self.state = ActionState::Cast(Cast {
            t: 0.0,
            spell,
            spec,
            serial: self.swings,
            target,
        });
        self.buffer = None;
        true
    }

    /// A blow lands: whatever the body was doing stops, and it reels.
    pub fn stagger(&mut self, duration: f32) {
        self.state = ActionState::Stagger { t: 0.0, duration };
        self.buffer = None;
    }

    /// A blow lands and the body plays Dark Souls reaction clip `clip` of the kit.
    pub fn reel(&mut self, clip: usize, spec: ReelSpec) {
        self.state = ActionState::Reel(Reel { t: 0.0, clip, spec });
        self.buffer = None;
    }

    /// Out of health with no death clip (the tests' shorthand; the game falls with `die_with`).
    #[cfg(test)]
    pub fn die(&mut self) {
        self.die_with(None);
    }

    /// Out of health, falling by the kit's death clip `clip` when there is one.
    pub fn die_with(&mut self, clip: Option<usize>) {
        self.state = ActionState::Dead;
        self.buffer = None;
        self.death = clip;
        self.dead_t = 0.0;
    }

    /// The current state takes a buffered `press` now: an idle body anything; a swing chains
    /// (light or heavy) from its chain time and evades from its evade time; an evade takes
    /// anything once it has travelled.
    fn accepts(&self, press: Press) -> bool {
        match (&self.state, press) {
            (ActionState::Idle, _) => true,
            (ActionState::Attack(s), Press::Attack | Press::Heavy | Press::Parry) => {
                s.cancellable()
            }
            (ActionState::Attack(s), Press::Evade) => s.t >= s.spec.evade_from,
            (ActionState::Evade(e), _) => e.t >= e.spec().cancel_from,
            (ActionState::Reel(r), Press::Attack | Press::Heavy | Press::Parry) => {
                r.t >= r.spec.chain_from
            }
            (ActionState::Reel(r), Press::Evade) => r.t >= r.spec.evade_from,
            (ActionState::Defend(d), Press::Evade) => {
                d.kind == crate::defense::Kind::Hold || d.t >= d.spec.evade
            }
            (ActionState::Defend(d), _) => {
                d.kind == crate::defense::Kind::Hold || d.t >= d.spec.chain
            }
            (ActionState::Cast(c), _) => c.cancellable(),
            (ActionState::Stagger { .. } | ActionState::Dead, _) => false,
        }
    }
}

/// What [`step_action`] decided this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    None,
    /// A swing started; `chained` when it came out of another swing. The caller plays its clip
    /// (the procedural swing) and spends its stamina.
    Swing {
        kind: MoveKind,
        chained: bool,
    },
    /// An evade started; the caller plays its clip and spends stamina.
    Evade(EvadeKind),
    Defense(crate::defense::Kind),
    /// The action ended; the body is free again.
    Ended,
}

/// What [`step_action`] reads each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepInput {
    /// A fresh press this frame.
    pub press: Option<Press>,
    /// Whether the body may start an action.
    pub grounded: bool,
    /// The swings this body can make.
    pub moves: Moveset,
    /// The stick's ground direction, its length the push: it aims an evade.
    pub stick: Vec3,
    /// The body's facing, which a backstep leaves against.
    pub facing: Vec3,
    /// The roll this body rolls: the DS roll's shape when loaded, else [`ROLL`].
    pub roll: EvadeSpec,
    /// The roll is the Dark Souls clip.
    pub evade_clips: bool,
    /// The backstep this body makes: the DS backstep's shape when loaded, else [`BACKSTEP`].
    pub backstep: EvadeSpec,
    /// The backstep is the Dark Souls clip.
    pub clip_backstep: bool,
    pub guard: bool,
    pub defense: Option<crate::defense::Specs>,
}

/// One frame of the action machine.
pub fn step_action(a: &mut Action, stamina: &Stamina, i: StepInput, dt: f32) -> Decision {
    a.guard_timing.tick(dt);
    if let Some(press) = i.press {
        a.buffer = Some(Buffered { press, age: 0.0 });
    } else if let Some(b) = a.buffer.as_mut() {
        b.age += dt;
        if b.age > BUFFER {
            a.buffer = None;
        }
    }
    if a.state == ActionState::Dead {
        a.dead_t += dt;
    }
    match &mut a.state {
        ActionState::Idle | ActionState::Dead => {}
        ActionState::Attack(s) => s.t += dt,
        ActionState::Evade(e) => e.t += dt,
        ActionState::Stagger { t, .. } => *t += dt,
        ActionState::Reel(r) => r.t += dt,
        ActionState::Defend(d) => d.t += dt,
        ActionState::Cast(c) => c.t += dt,
    }
    let ready = stamina.can_spend() && i.grounded;
    if let Some(b) = a.buffer.filter(|b| ready && a.accepts(b.press)) {
        a.buffer = None;
        return start(a, b.press, &i);
    }
    if let Some(specs) = i.defense {
        use crate::defense::{Kind, Playing};
        let next = match a.state {
            ActionState::Defend(d) if d.kind.guarding() && !i.guard => Some(Kind::Lower),
            ActionState::Defend(d) if d.kind == Kind::Raise && d.t >= d.spec.duration => {
                Some(Kind::Hold)
            }
            ActionState::Idle if i.guard && ready => Some(Kind::Raise),
            ActionState::Attack(s) if i.guard && ready && s.t >= s.spec.evade_from => {
                Some(Kind::Raise)
            }
            ActionState::Evade(e) if i.guard && ready && e.t >= e.spec.cancel_from => {
                Some(Kind::Raise)
            }
            _ => None,
        };
        if let Some(kind) = next {
            if kind == Kind::Raise {
                a.guard_timing.raise();
            }
            a.state = ActionState::Defend(Playing::new(kind, specs.get(kind)));
            return Decision::Defense(kind);
        }
    }
    // Movement cancels an evade from its cancel window (Dark Souls' flag 11) and a swing from its
    // own: a pushed stick ends it, unless a buffered press was taken above.
    let pushed = Vec3::new(i.stick.x, 0.0, i.stick.z).length() > ROLL_PUSH;
    let free = match a.state {
        ActionState::Evade(e) => e.t >= e.spec.cancel_from,
        ActionState::Attack(s) => s.t >= s.spec.move_from,
        ActionState::Reel(r) => r.t >= r.spec.move_from,
        ActionState::Defend(d) => !d.kind.guarding() && d.t >= d.spec.movement,
        ActionState::Cast(c) => c.cancellable(),
        _ => false,
    };
    if pushed && free {
        a.state = ActionState::Idle;
        return Decision::Ended;
    }
    let over = match a.state {
        ActionState::Idle | ActionState::Dead => false,
        ActionState::Attack(s) => s.phase().is_none(),
        ActionState::Evade(e) => e.t >= e.spec().duration,
        ActionState::Stagger { t, duration } => t >= duration,
        ActionState::Reel(r) => r.t >= r.spec.duration,
        ActionState::Defend(d) => d.kind != crate::defense::Kind::Hold && d.t >= d.spec.duration,
        ActionState::Cast(c) => c.t >= c.spec.duration,
    };
    if over {
        a.state = ActionState::Idle;
        return Decision::Ended;
    }
    Decision::None
}

/// The move a press starts out of `state`: the chain's next light hit (looping after the last),
/// the first light hit out of a heavy one, a heavy press starts the heavy attack or its
/// follow-up (and the heavy attack again after that), and a light press out of an evade is the
/// roll attack where the moveset has one.
fn next_move(state: &ActionState, press: Press, moves: &Moveset) -> MoveKind {
    match (state, press) {
        (ActionState::Attack(s), Press::Heavy) => match s.kind {
            MoveKind::R2(0) => MoveKind::R2(1),
            _ => MoveKind::R2(0),
        },
        (_, Press::Heavy) => MoveKind::R2(0),
        (ActionState::Attack(s), _) => match s.kind {
            MoveKind::R1(n) => MoveKind::R1((n + 1) % moves.chain_len.clamp(1, R1_CHAIN)),
            _ => MoveKind::R1(0),
        },
        (ActionState::Evade(_), _) if moves.roll_attack.is_some() => MoveKind::RollAttack,
        _ => MoveKind::R1(0),
    }
}

/// Starts the action `press` asks for: a swing (the next of a chain out of a swing), or an evade,
/// a roll along a pushed stick or else a backstep. A swing the moveset lacks is no action.
fn start(a: &mut Action, press: Press, i: &StepInput) -> Decision {
    match press {
        Press::Parry => {
            let Some(specs) = i.defense else {
                return Decision::None;
            };
            let mut parry = specs.parry;
            a.parry_caught = false;
            parry.cost = (parry.cost - a.training.parry_discount).max(1.0);
            a.state = ActionState::Defend(crate::defense::Playing::new(
                crate::defense::Kind::Parry,
                parry,
            ));
            Decision::Defense(crate::defense::Kind::Parry)
        }
        Press::Attack | Press::Heavy => {
            let kind = next_move(&a.state, press, &i.moves);
            let Some(spec) = i.moves.spec(kind) else {
                return Decision::None;
            };
            let chained = matches!(a.state, ActionState::Attack(_));
            a.swings += 1;
            a.state = ActionState::Attack(Swing {
                t: 0.0,
                kind,
                spec,
                reference: i.moves.reference,
                serial: a.swings,
            });
            Decision::Swing { kind, chained }
        }
        Press::Evade => {
            let stick = Vec3::new(i.stick.x, 0.0, i.stick.z);
            let (kind, dir) = if stick.length() > ROLL_PUSH {
                (EvadeKind::Roll, stick.normalize())
            } else {
                let back = -Vec3::new(i.facing.x, 0.0, i.facing.z).normalize_or_zero();
                (EvadeKind::Backstep, back)
            };
            let (spec, reference) = match kind {
                EvadeKind::Roll => (i.roll, i.evade_clips),
                EvadeKind::Backstep => (i.backstep, i.clip_backstep),
            };
            a.state = ActionState::Evade(Evade {
                kind,
                t: 0.0,
                dir,
                spec,
                reference,
            });
            Decision::Evade(kind)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::STAMINA_MAX;

    const DT: f32 = 1.0 / 60.0;

    fn defense_input(guard: bool, press: Option<Press>) -> StepInput {
        use crate::defense::{Spec, Specs};
        let base = Spec {
            duration: 0.6,
            chain: 0.1,
            evade: 0.1,
            movement: 0.6,
            active: Some((0.2, 0.6)),
            cost: 0.0,
        };
        StepInput {
            guard,
            defense: Some(Specs {
                raise: base,
                hold: Spec {
                    duration: 2.0,
                    active: Some((0.0, 2.0)),
                    ..base
                },
                lower: Spec {
                    active: None,
                    ..base
                },
                parry: Spec {
                    duration: 1.2,
                    active: Some((0.0, 7.0 / 30.0)),
                    cost: 14.0,
                    ..base
                },
            }),
            ..input(press, true)
        }
    }

    #[test]
    fn guard_raises_holds_and_releases_without_buffering_the_hold() {
        use crate::defense::Kind;
        let mut a = Action::default();
        let s = full();
        assert_eq!(
            step_action(&mut a, &s, defense_input(true, None), DT),
            Decision::Defense(Kind::Raise)
        );
        assert!(matches!(a.state, ActionState::Defend(d) if !d.active()));
        step_action(&mut a, &s, defense_input(true, None), 0.21);
        assert!(matches!(a.state, ActionState::Defend(d) if d.active()));
        assert_eq!(
            step_action(&mut a, &s, defense_input(true, None), 0.4),
            Decision::Defense(Kind::Hold)
        );
        step_action(&mut a, &s, defense_input(true, None), 3.0);
        assert!(matches!(a.state, ActionState::Defend(d) if d.kind == Kind::Hold && d.active()));
        assert_eq!(
            step_action(&mut a, &s, defense_input(false, None), DT),
            Decision::Defense(Kind::Lower)
        );
        assert!(a.buffer.is_none());
        assert_eq!(
            step_action(&mut a, &s, defense_input(false, None), 0.7),
            Decision::Ended
        );
    }

    #[test]
    fn guard_can_roll_or_parry_and_failed_parry_has_no_invulnerability() {
        use crate::defense::Kind;
        let mut a = Action::default();
        let s = full();
        step_action(&mut a, &s, defense_input(true, None), DT);
        step_action(&mut a, &s, defense_input(true, None), 0.7);
        assert_eq!(
            step_action(&mut a, &s, defense_input(true, Some(Press::Parry)), DT),
            Decision::Defense(Kind::Parry)
        );
        assert!(!a.invulnerable());
        step_action(&mut a, &s, defense_input(false, None), 0.25);
        assert!(matches!(a.state, ActionState::Defend(d) if !d.active()));
        assert_eq!(
            step_action(&mut a, &s, defense_input(false, Some(Press::Evade)), DT),
            Decision::Evade(EvadeKind::Backstep)
        );
    }

    #[test]
    fn guard_and_parry_need_a_loaded_kit_and_some_stamina() {
        let mut a = Action::default();
        let empty = Stamina::full(0.0);
        assert_eq!(
            step_action(&mut a, &empty, defense_input(true, Some(Press::Parry)), DT),
            Decision::None
        );
        assert_eq!(a.state, ActionState::Idle);
        let no_kit = StepInput {
            defense: None,
            ..defense_input(true, Some(Press::Parry))
        };
        assert_eq!(step_action(&mut a, &full(), no_kit, DT), Decision::None);
        assert_eq!(a.state, ActionState::Idle);
    }

    fn full() -> Stamina {
        Stamina::full(STAMINA_MAX)
    }

    const LEN: f32 = 0.8;

    /// One frame's input: grounded, the stick still, facing `-Z`, today's swing.
    fn input(press: Option<Press>, grounded: bool) -> StepInput {
        StepInput {
            press,
            grounded,
            moves: Moveset::procedural(LEN),
            stick: Vec3::ZERO,
            facing: Vec3::NEG_Z,
            roll: ROLL,
            evade_clips: false,
            backstep: BACKSTEP,
            clip_backstep: false,
            guard: false,
            defense: None,
        }
    }

    /// The Dark Souls straight sword's table (the spec's), as a moveset.
    fn weapon_clips() -> Moveset {
        let s = |duration, hit, chain_from, evade_from, move_from, cost, mult| SwingSpec {
            duration,
            hit,
            chain_from,
            evade_from,
            move_from,
            cost,
            mult,
            level: 1,
            poise: 0.0,
            stop: 0.08,
            knock: 0.72,
            radius: crate::hits::BLADE_RADIUS,
        };
        Moveset {
            chain_len: 3,
            r1: [
                s(2.500, (0.533, 0.667), 0.767, 0.900, 1.033, 20.0, 1.00),
                s(2.233, (0.467, 0.567), 0.733, 0.900, 1.000, 20.0, 1.00),
                s(2.500, (0.467, 0.600), 0.767, 0.900, 1.033, 20.0, 1.00),
            ],
            r2: Some([
                s(2.533, (0.733, 0.867), 1.300, 1.300, 1.367, 45.0, 1.35),
                s(1.900, (0.333, 0.467), 0.933, 0.933, 1.200, 35.0, 1.25),
            ]),
            roll_attack: Some(s(2.100, (0.233, 0.333), 0.700, 0.767, 0.867, 20.0, 0.90)),
            reference: true,
        }
    }

    #[test]
    fn shorter_weapon_chains_loop_after_their_authored_last_attack() {
        let mut m = weapon_clips();
        m.chain_len = 2;
        let mut a = Action::default();
        let decisions = run_in(
            m,
            &mut a,
            &[
                (0.0, Press::Attack),
                (0.9, Press::Attack),
                (1.8, Press::Attack),
            ],
            Vec3::ZERO,
            2.1,
        );
        let kinds: Vec<_> = decisions
            .into_iter()
            .filter_map(|(_, d)| {
                if let Decision::Swing { kind, .. } = d {
                    Some(kind)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            kinds,
            vec![MoveKind::R1(0), MoveKind::R1(1), MoveKind::R1(0)]
        );
    }

    fn swing(kind: MoveKind, chained: bool) -> Decision {
        Decision::Swing { kind, chained }
    }

    /// Runs the machine on `moves` for `secs` with presses at the given times and a held stick;
    /// returns the decisions with their times.
    fn run_in(
        moves: Moveset,
        a: &mut Action,
        presses: &[(f32, Press)],
        stick: Vec3,
        secs: f32,
    ) -> Vec<(f32, Decision)> {
        let s = full();
        let mut out = Vec::new();
        let mut t = 0.0;
        while t < secs {
            let press = presses
                .iter()
                .find(|&&(p, _)| p >= t && p < t + DT)
                .map(|&(_, k)| k);
            let d = step_action(
                a,
                &s,
                StepInput {
                    stick,
                    moves,
                    ..input(press, true)
                },
                DT,
            );
            if d != Decision::None {
                out.push((t, d));
            }
            t += DT;
        }
        out
    }

    /// The same on today's procedural swing.
    fn run_with(
        a: &mut Action,
        presses: &[(f32, Press)],
        stick: Vec3,
        secs: f32,
    ) -> Vec<(f32, Decision)> {
        run_in(Moveset::procedural(LEN), a, presses, stick, secs)
    }

    /// Attack presses only.
    fn run(a: &mut Action, presses: &[f32], secs: f32) -> Vec<(f32, Decision)> {
        let p: Vec<_> = presses.iter().map(|&t| (t, Press::Attack)).collect();
        run_with(a, &p, Vec3::ZERO, secs)
    }

    #[test]
    fn the_swing_runs_its_phases_in_order() {
        let at = |t: f32| Swing {
            t,
            kind: MoveKind::R1(0),
            spec: SwingSpec::procedural(LEN),
            reference: false,
            serial: 1,
        };
        assert_eq!(at(0.0).phase(), Some(SwingPhase::Startup));
        assert_eq!(at(0.3 * LEN).phase(), Some(SwingPhase::Active));
        assert_eq!(at(0.6 * LEN).phase(), Some(SwingPhase::Recovery));
        assert_eq!(at(LEN).phase(), None);
    }

    #[test]
    fn a_press_swings_once_and_ends() {
        let mut a = Action::default();
        let d = run(&mut a, &[0.0], 2.0);
        assert_eq!(d.len(), 2, "{d:?}");
        assert_eq!(d[0].1, swing(MoveKind::R1(0), false));
        assert_eq!(d[1].1, Decision::Ended);
        assert!(
            (d[1].0 - LEN).abs() < 2.0 * DT,
            "ends after the swing: {d:?}"
        );
        assert!(!a.owns_body());
    }

    #[test]
    fn a_press_in_the_wind_up_is_buffered_into_a_chain() {
        let mut a = Action::default();
        // Pressed again during the active window, before the cancel window opens: buffered.
        let d = run(&mut a, &[0.0, 0.45], 3.0);
        let swings: Vec<_> = d
            .iter()
            .filter(|(_, d)| matches!(d, Decision::Swing { .. }))
            .collect();
        assert_eq!(swings.len(), 2, "{d:?}");
        assert_eq!(swings[1].1, swing(MoveKind::R1(1), true));
        // The follow-up waits for the cancel window.
        assert!(swings[1].0 >= CANCEL_FROM * LEN - DT, "{d:?}");
    }

    #[test]
    fn a_press_too_early_expires() {
        let mut a = Action::default();
        // Pressed at 0.05: older than the buffer by the cancel window at 0.56.
        let d = run(&mut a, &[0.0, 0.05], 3.0);
        let swings = d
            .iter()
            .filter(|(_, d)| matches!(d, Decision::Swing { .. }))
            .count();
        assert_eq!(swings, 1, "{d:?}");
    }

    #[test]
    fn no_stamina_no_swing_and_airborne_no_swing() {
        let mut a = Action::default();
        let mut empty = full();
        empty.spend(1000.0);
        assert_eq!(
            step_action(&mut a, &empty, input(Some(Press::Attack), true), DT),
            Decision::None
        );
        let mut a = Action::default();
        assert_eq!(
            step_action(&mut a, &full(), input(Some(Press::Attack), false), DT),
            Decision::None
        );
    }

    #[test]
    fn a_pushed_evade_rolls_along_the_stick_and_a_still_one_backsteps() {
        let mut a = Action::default();
        let d = run_with(&mut a, &[(0.0, Press::Evade)], Vec3::new(0.6, 0.0, 0.0), DT);
        assert_eq!(d, vec![(0.0, Decision::Evade(EvadeKind::Roll))]);
        let e = a.evading().copied().unwrap();
        assert!((e.dir - Vec3::X).length() < 1.0e-6, "unit, along the stick");

        let mut a = Action::default();
        // A push inside the roll threshold is no push.
        let d = run_with(&mut a, &[(0.0, Press::Evade)], Vec3::new(0.1, 0.0, 0.0), DT);
        assert_eq!(d, vec![(0.0, Decision::Evade(EvadeKind::Backstep))]);
        let e = a.evading().copied().unwrap();
        assert!((e.dir - Vec3::Z).length() < 1.0e-6, "away from the facing");
    }

    #[test]
    fn a_roll_runs_its_length_and_ends() {
        let mut a = Action::default();
        // Pushed for the press, then let go: a held stick would cancel it from its cancel window.
        let d = run_with(&mut a, &[(0.0, Press::Evade)], Vec3::NEG_Z, DT);
        assert_eq!(d, vec![(0.0, Decision::Evade(EvadeKind::Roll))]);
        let d = run_with(&mut a, &[], Vec3::ZERO, 2.0);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].1, Decision::Ended);
        assert!((d[0].0 + DT - ROLL.duration).abs() < 2.0 * DT, "{d:?}");
        assert!(!a.owns_body());
    }

    #[test]
    fn a_ds_roll_takes_its_shape_from_the_input_and_does_not_tumble() {
        let clip_spec = EvadeSpec {
            duration: 1.0,
            travel: 1.0,
            distance: 3.0,
            iframes: (0.1, 0.4),
            cancel_from: 0.7,
            cost: 20.0,
        };
        let mut a = Action::default();
        let i = StepInput {
            roll: clip_spec,
            evade_clips: true,
            stick: Vec3::NEG_Z,
            ..input(Some(Press::Evade), true)
        };
        assert_eq!(
            step_action(&mut a, &full(), i, DT),
            Decision::Evade(EvadeKind::Roll)
        );
        let e = *a.evading().unwrap();
        assert!(e.reference && e.spec == clip_spec);
        assert_eq!(Evade { t: 0.5, ..e }.tumble(), 0.0);
    }

    #[test]
    fn a_ds_backstep_takes_its_shape_from_the_input_and_leaves_the_roll_alone() {
        let clip_backstep = EvadeSpec {
            duration: 1.8,
            travel: 1.8,
            distance: 2.27,
            iframes: (0.0, 0.133),
            cancel_from: 0.87,
            cost: 15.0,
        };
        let mut a = Action::default();
        // Still stick: the backstep, shaped by the DS clip, leaving against the facing.
        let i = StepInput {
            backstep: clip_backstep,
            clip_backstep: true,
            ..input(Some(Press::Evade), true)
        };
        assert_eq!(
            step_action(&mut a, &full(), i, DT),
            Decision::Evade(EvadeKind::Backstep)
        );
        let e = *a.evading().unwrap();
        assert!(e.reference && e.spec == clip_backstep);
        assert_eq!(e.dir, Vec3::Z, "against the facing (-Z)");
        assert_eq!(Evade { t: 0.5, ..e }.tumble(), 0.0);
        // A pushed stick is still the roll, with the roll's own shape and clip flag.
        let mut b = Action::default();
        let i = StepInput {
            backstep: clip_backstep,
            clip_backstep: true,
            stick: Vec3::X,
            ..input(Some(Press::Evade), true)
        };
        assert_eq!(
            step_action(&mut b, &full(), i, DT),
            Decision::Evade(EvadeKind::Roll)
        );
        let r = *b.evading().unwrap();
        assert!(
            !r.reference && r.spec == ROLL,
            "the backstep's shape is not the roll's"
        );
        // Without the DS backstep the procedural one stays.
        let mut c = Action::default();
        step_action(&mut c, &full(), input(Some(Press::Evade), true), DT);
        let p = *c.evading().unwrap();
        assert!(!p.reference && p.spec == BACKSTEP);
    }

    #[test]
    fn a_pushed_stick_cancels_an_evade_only_from_its_cancel_window() {
        // Dark Souls' roll shape: a long stand-up, which movement may cancel from 1.1 s.
        let clip_spec = EvadeSpec {
            duration: 2.0,
            travel: 2.0,
            distance: 4.5,
            iframes: (0.0, 0.367),
            cancel_from: 1.1,
            cost: 20.0,
        };
        let s = full();
        let step = |a: &mut Action, press: Option<Press>, stick: Vec3| {
            let i = StepInput {
                roll: clip_spec,
                evade_clips: true,
                stick,
                ..input(press, true)
            };
            step_action(a, &s, i, DT)
        };
        let mut a = Action::default();
        assert_eq!(
            step(&mut a, Some(Press::Evade), Vec3::NEG_Z),
            Decision::Evade(EvadeKind::Roll)
        );
        // Let go, pushed again around 0.5 s, then pushed and held from 1.2 s.
        let mut t = 0.0;
        let mut ended = None;
        while t < 2.5 && ended.is_none() {
            t += DT;
            let pushed = (0.5..0.6).contains(&t) || t >= 1.2;
            let stick = if pushed { Vec3::X } else { Vec3::ZERO };
            if step(&mut a, None, stick) == Decision::Ended {
                ended = Some(t);
            }
        }
        let ended = ended.expect("the push ends it");
        assert!((ended - 1.2).abs() < 2.0 * DT, "{ended}");
        assert!(!a.owns_body());

        // A buffered press keeps priority over the push.
        let mut a = Action::default();
        step(&mut a, Some(Press::Evade), Vec3::NEG_Z);
        while a.evading().is_some_and(|e| e.t < 1.2) {
            step(&mut a, None, Vec3::ZERO);
        }
        assert_eq!(
            step(&mut a, Some(Press::Attack), Vec3::X),
            swing(MoveKind::R1(0), false)
        );
    }

    #[test]
    fn the_roll_covers_its_distance_fast_first_then_stops() {
        let spec = &ROLL;
        assert_eq!(Evade::covered(spec, 0.0), 0.0);
        assert!((Evade::covered(spec, spec.travel) - 1.0).abs() < 1.0e-6);
        assert!((Evade::covered(spec, spec.duration) - 1.0).abs() < 1.0e-6);
        // Front-loaded: two thirds of the distance by half the travel.
        assert!(Evade::covered(spec, 0.5 * spec.travel) > 0.65);
        // Frame by frame the shares sum to the whole distance.
        let mut sum = 0.0;
        let mut t = 0.0;
        while t < spec.duration {
            let next = t + DT;
            sum += (Evade::covered(spec, next) - Evade::covered(spec, t)) * spec.distance;
            t = next;
        }
        assert!((sum - spec.distance).abs() < 1.0e-4, "{sum}");
    }

    #[test]
    fn the_roll_is_invulnerable_only_inside_its_window() {
        let at = |t: f32| Evade {
            kind: EvadeKind::Roll,
            t,
            dir: Vec3::NEG_Z,
            spec: ROLL,
            reference: false,
        };
        assert!(!at(0.0).invulnerable(), "the first frame is open");
        assert!(at(0.1).invulnerable());
        assert!(at(0.4).invulnerable());
        assert!(!at(0.5).invulnerable());
        let mut a = Action::default();
        run_with(&mut a, &[(0.0, Press::Evade)], Vec3::NEG_Z, 0.2);
        assert!(a.invulnerable());
        assert!(!Action::default().invulnerable());
    }

    #[test]
    fn the_somersault_turns_once_and_only_in_a_roll() {
        let at = |kind: EvadeKind, t: f32| Evade {
            kind,
            t,
            dir: Vec3::NEG_Z,
            spec: *kind.spec(),
            reference: false,
        };
        assert_eq!(at(EvadeKind::Roll, 0.0).tumble(), 0.0);
        let half = at(EvadeKind::Roll, ROLL.travel * 0.5).tumble();
        assert!((half - std::f32::consts::PI).abs() < 1.0e-5, "{half}");
        let done = at(EvadeKind::Roll, ROLL.travel).tumble();
        assert!((done - std::f32::consts::TAU).abs() < 1.0e-5);
        assert_eq!(at(EvadeKind::Backstep, 0.2).tumble(), 0.0);
    }

    #[test]
    fn a_roll_cancels_a_swing_only_from_its_recovery() {
        let mut a = Action::default();
        // Pressed in the wind-up: buffered until the recovery opens at ACTIVE_TO.
        let d = run_with(
            &mut a,
            &[(0.0, Press::Attack), (0.2, Press::Evade)],
            Vec3::NEG_Z,
            2.0,
        );
        let roll = d
            .iter()
            .find(|(_, d)| *d == Decision::Evade(EvadeKind::Roll))
            .expect("the roll lands");
        assert!(roll.0 >= ACTIVE_TO * LEN - DT, "{d:?}");
        assert!(
            roll.0 < CANCEL_FROM * LEN,
            "before the attack's own cancel: {d:?}"
        );
    }

    #[test]
    fn an_attack_buffered_in_a_roll_starts_when_it_has_travelled() {
        let mut a = Action::default();
        let d = run_with(
            &mut a,
            &[(0.0, Press::Evade), (0.4, Press::Attack)],
            Vec3::NEG_Z,
            2.0,
        );
        let swing = d
            .iter()
            .find(|(_, d)| *d == swing(MoveKind::R1(0), false))
            .expect("the swing lands");
        assert!(swing.0 >= ROLL.cancel_from - DT, "{d:?}");
        assert!(swing.0 < ROLL.duration, "it cancels the recovery: {d:?}");
    }

    #[test]
    fn a_stagger_stops_the_swing_holds_presses_and_wears_off() {
        let mut a = Action::default();
        let s = full();
        assert_eq!(
            step_action(&mut a, &s, input(Some(Press::Attack), true), DT),
            swing(MoveKind::R1(0), false)
        );
        a.stagger(0.5);
        assert!(a.owns_body() && !a.invulnerable());
        // A press late in the stagger waits in the buffer, and starts the moment the body is free.
        let mut t = 0.0;
        let mut out = Vec::new();
        while t < 1.0 {
            let press = (0.3..0.3 + DT).contains(&t).then_some(Press::Evade);
            let d = step_action(&mut a, &s, input(press, true), DT);
            t += DT;
            if d != Decision::None {
                out.push((t, d));
            }
        }
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(out[0].1, Decision::Ended);
        assert!((out[0].0 - 0.5).abs() < 2.0 * DT, "{out:?}");
        assert_eq!(out[1].1, Decision::Evade(EvadeKind::Backstep));
        assert!(out[1].0 - out[0].0 < 2.0 * DT, "{out:?}");
    }

    #[test]
    fn the_dead_take_no_action() {
        let mut a = Action::default();
        a.die();
        for _ in 0..120 {
            assert_eq!(
                step_action(&mut a, &full(), input(Some(Press::Attack), true), DT),
                Decision::None
            );
        }
        assert!(a.is_dead() && a.owns_body());
    }

    #[test]
    fn the_procedural_swing_keeps_todays_windows() {
        let s = SwingSpec::procedural(LEN);
        assert_eq!(s.hit, (ACTIVE_FROM * LEN, ACTIVE_TO * LEN));
        assert_eq!(s.chain_from, CANCEL_FROM * LEN);
        assert_eq!(s.evade_from, ACTIVE_TO * LEN);
        assert_eq!(s.move_from, LEN, "a pushed stick never cuts it short");
        assert_eq!((s.cost, s.mult), (ATTACK_COST, 1.0));
        let m = Moveset::procedural(LEN);
        assert!(!m.has_heavy() && m.roll_attack.is_none() && !m.reference);
    }

    #[test]
    fn the_light_chain_runs_three_hits_and_loops() {
        let mut a = Action::default();
        // A press every 0.9 s: each lands in the chain window of the swing before.
        let presses: Vec<_> = (0..5).map(|n| (n as f32 * 0.9, Press::Attack)).collect();
        let d = run_in(weapon_clips(), &mut a, &presses, Vec3::ZERO, 4.6);
        let kinds: Vec<_> = d
            .iter()
            .filter_map(|(_, d)| match d {
                Decision::Swing { kind, chained } => Some((*kind, *chained)),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                (MoveKind::R1(0), false),
                (MoveKind::R1(1), true),
                (MoveKind::R1(2), true),
                (MoveKind::R1(0), true),
                (MoveKind::R1(1), true),
            ],
            "{d:?}"
        );
    }

    #[test]
    fn the_next_hit_waits_for_the_chain_flag() {
        let mut a = Action::default();
        // Pressed at 0.5 s into R1-1, the chain flag opens at 0.767 s: the buffer (0.35 s) holds it.
        let d = run_in(
            weapon_clips(),
            &mut a,
            &[(0.0, Press::Attack), (0.5, Press::Attack)],
            Vec3::ZERO,
            2.0,
        );
        let second = d
            .iter()
            .find(|(_, d)| *d == swing(MoveKind::R1(1), true))
            .expect("the second hit lands");
        assert!((second.0 - 0.767).abs() < 2.0 * DT, "{d:?}");
    }

    #[test]
    fn a_swing_serial_counts_every_swing() {
        let mut a = Action::default();
        run_in(
            weapon_clips(),
            &mut a,
            &[(0.0, Press::Attack), (0.9, Press::Attack)],
            Vec3::ZERO,
            1.2,
        );
        assert_eq!(a.swings, 2);
        let ActionState::Attack(s) = a.state else {
            panic!("a swing is running")
        };
        assert_eq!(s.serial, 2);
        assert!(s.reference);
    }

    #[test]
    fn heavy_starts_r2_and_then_its_follow_up_and_then_r2_again() {
        let mut a = Action::default();
        let presses = [
            (0.0, Press::Heavy),
            (1.4, Press::Heavy),
            (2.4, Press::Heavy),
        ];
        let d = run_in(weapon_clips(), &mut a, &presses, Vec3::ZERO, 3.6);
        let kinds: Vec<_> = d
            .iter()
            .filter_map(|(_, d)| match d {
                Decision::Swing { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![MoveKind::R2(0), MoveKind::R2(1), MoveKind::R2(0)],
            "{d:?}"
        );
    }

    #[test]
    fn light_and_heavy_swap_by_restarting_the_other() {
        let mut a = Action::default();
        let presses = [
            (0.0, Press::Attack),
            (0.9, Press::Heavy),
            (2.4, Press::Attack),
        ];
        let d = run_in(weapon_clips(), &mut a, &presses, Vec3::ZERO, 3.5);
        let kinds: Vec<_> = d
            .iter()
            .filter_map(|(_, d)| match d {
                Decision::Swing { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![MoveKind::R1(0), MoveKind::R2(0), MoveKind::R1(0)],
            "{d:?}"
        );
    }

    #[test]
    fn an_evade_waits_for_the_evade_flag() {
        // Pressed at 0.7 s into R1-1, the roll lands when the flag opens, at 0.9 s.
        let mut a = Action::default();
        let d = run_in(
            weapon_clips(),
            &mut a,
            &[(0.0, Press::Attack), (0.7, Press::Evade)],
            Vec3::NEG_Z,
            1.5,
        );
        let roll = d
            .iter()
            .find(|(_, d)| *d == Decision::Evade(EvadeKind::Roll))
            .expect("the roll lands");
        assert!((roll.0 - 0.9).abs() < 2.0 * DT, "{d:?}");
        // Pressed at 0.5 s, the press is 0.4 s old by then, past the buffer: no roll.
        let mut a = Action::default();
        let d = run_in(
            weapon_clips(),
            &mut a,
            &[(0.0, Press::Attack), (0.5, Press::Evade)],
            Vec3::NEG_Z,
            1.5,
        );
        assert!(
            d.iter().all(|(_, d)| !matches!(d, Decision::Evade(_))),
            "{d:?}"
        );
    }

    #[test]
    fn a_pushed_stick_ends_a_swing_only_from_its_movement_flag() {
        let s = full();
        let step = |a: &mut Action, press: Option<Press>, stick: Vec3| {
            step_action(
                a,
                &s,
                StepInput {
                    stick,
                    moves: weapon_clips(),
                    ..input(press, true)
                },
                DT,
            )
        };
        let mut a = Action::default();
        assert_eq!(
            step(&mut a, Some(Press::Attack), Vec3::ZERO),
            swing(MoveKind::R1(0), false)
        );
        let mut t = 0.0;
        let mut ended = None;
        while t < 2.5 && ended.is_none() {
            t += DT;
            // Pushed from 0.5 s on: the swing holds until 1.033 s.
            let stick = if t >= 0.5 { Vec3::X } else { Vec3::ZERO };
            if step(&mut a, None, stick) == Decision::Ended {
                ended = Some(t);
            }
        }
        let ended = ended.expect("the push ends it");
        assert!((ended - 1.033).abs() < 2.0 * DT, "{ended}");
        // The procedural swing is never cut short by the stick.
        let mut a = Action::default();
        step_action(&mut a, &s, input(Some(Press::Attack), true), DT);
        for _ in 0..30 {
            let i = StepInput {
                stick: Vec3::X,
                ..input(None, true)
            };
            assert_eq!(step_action(&mut a, &s, i, DT), Decision::None);
        }
    }

    #[test]
    fn a_light_press_in_a_ds_roll_is_the_roll_attack() {
        let evade_clips = EvadeSpec {
            duration: 2.0,
            travel: 2.0,
            distance: 4.5,
            iframes: (0.0, 0.367),
            cancel_from: 1.1,
            cost: 20.0,
        };
        let s = full();
        let step = |a: &mut Action, press: Option<Press>| {
            step_action(
                a,
                &s,
                StepInput {
                    roll: evade_clips,
                    evade_clips: true,
                    stick: Vec3::NEG_Z,
                    moves: weapon_clips(),
                    ..input(press, true)
                },
                DT,
            )
        };
        let mut a = Action::default();
        assert_eq!(
            step(&mut a, Some(Press::Evade)),
            Decision::Evade(EvadeKind::Roll)
        );
        let mut t = 0.0;
        let mut out = None;
        while t < 2.0 && out.is_none() {
            t += DT;
            let press = (1.0..1.0 + DT).contains(&t).then_some(Press::Attack);
            // The stick is held: a push cancels a roll from 1.1 s, after the press is taken.
            if let Decision::Swing { kind, chained } = step(&mut a, press) {
                out = Some((t, kind, chained));
            }
        }
        let (t, kind, chained) = out.expect("the roll attack starts");
        assert_eq!(kind, MoveKind::RollAttack);
        assert!(!chained);
        assert!((t - 1.1).abs() < 2.0 * DT, "{t}");
        // With no roll attack in the moveset it is the first light hit.
        let mut a = Action::default();
        let plain = |press| StepInput {
            roll: evade_clips,
            evade_clips: true,
            ..input(press, true)
        };
        step_action(
            &mut a,
            &s,
            StepInput {
                stick: Vec3::NEG_Z,
                ..plain(Some(Press::Evade))
            },
            DT,
        );
        while a.evading().is_some_and(|e| e.t < 1.2) {
            step_action(&mut a, &s, plain(None), DT);
        }
        let d = step_action(&mut a, &s, plain(Some(Press::Attack)), DT);
        assert_eq!(d, swing(MoveKind::R1(0), false));
    }

    #[test]
    fn a_heavy_press_without_a_heavy_attack_does_nothing() {
        let mut a = Action::default();
        let s = full();
        assert_eq!(
            step_action(&mut a, &s, input(Some(Press::Heavy), true), DT),
            Decision::None
        );
        assert!(!a.owns_body() && a.buffer.is_none(), "the press is dropped");
    }

    #[test]
    fn move_names_round_trip() {
        for k in MoveKind::ALL {
            assert_eq!(MoveKind::from_name(k.name()), Some(k));
        }
        assert_eq!(MoveKind::from_name("r3_1"), None);
    }

    fn reel_spec() -> ReelSpec {
        ReelSpec {
            duration: 1.0,
            chain_from: 0.6,
            evade_from: 0.5,
            move_from: 0.7,
            invuln: Some((0.0, 0.4)),
        }
    }

    fn reeling() -> Action {
        let mut a = Action::default();
        a.reel(3, reel_spec());
        a
    }

    #[test]
    fn a_reel_takes_a_buffered_attack_from_its_attack_time() {
        let mut a = reeling();
        let d = run_in(
            weapon_clips(),
            &mut a,
            &[(0.4, Press::Attack)],
            Vec3::ZERO,
            1.2,
        );
        let (t, what) = d[0];
        assert_eq!(what, swing(MoveKind::R1(0), false), "{d:?}");
        assert!((0.6 - DT..0.6 + 2.0 * DT).contains(&t), "started at {t}");
    }

    #[test]
    fn a_reel_takes_a_buffered_evade_from_its_evade_time() {
        let mut a = reeling();
        let d = run_in(
            weapon_clips(),
            &mut a,
            &[(0.3, Press::Evade)],
            Vec3::ZERO,
            1.2,
        );
        let (t, what) = d[0];
        assert_eq!(what, Decision::Evade(EvadeKind::Backstep), "{d:?}");
        assert!((0.5 - DT..0.5 + 2.0 * DT).contains(&t), "started at {t}");
    }

    #[test]
    fn a_pushed_stick_ends_a_reel_at_its_movement_time_else_it_ends_at_its_duration() {
        let mut a = reeling();
        let d = run_in(weapon_clips(), &mut a, &[], Vec3::X, 1.5);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].1, Decision::Ended);
        assert!((0.7 - DT..0.7 + 2.0 * DT).contains(&d[0].0), "{d:?}");
        let mut a = reeling();
        let d = run_in(weapon_clips(), &mut a, &[], Vec3::ZERO, 1.5);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!((1.0 - DT..1.0 + 2.0 * DT).contains(&d[0].0), "{d:?}");
        assert!(!a.owns_body());
    }

    #[test]
    fn a_reel_cannot_be_hit_only_inside_its_window() {
        let mut a = reeling();
        assert!(a.invulnerable());
        let s = full();
        let mut t = 0.0;
        while t < 0.35 {
            step_action(&mut a, &s, input(None, true), DT);
            t += DT;
        }
        assert!(a.invulnerable(), "still inside at 0.35 s");
        while t < 0.45 {
            step_action(&mut a, &s, input(None, true), DT);
            t += DT;
        }
        assert!(!a.invulnerable(), "out at 0.45 s");
        // A reel without a window is never invulnerable.
        let mut b = Action::default();
        b.reel(
            0,
            ReelSpec {
                invuln: None,
                ..reel_spec()
            },
        );
        assert!(!b.invulnerable());
    }

    #[test]
    fn a_blow_in_a_reel_starts_the_reel_again_and_a_death_runs_its_clock() {
        let mut a = reeling();
        let s = full();
        for _ in 0..30 {
            step_action(&mut a, &s, input(None, true), DT);
        }
        a.reel(5, reel_spec());
        assert!(matches!(a.state, ActionState::Reel(r) if r.t == 0.0 && r.clip == 5));
        a.die_with(Some(1));
        assert!(a.is_dead() && a.death == Some(1) && a.dead_t == 0.0);
        for _ in 0..60 {
            step_action(&mut a, &s, input(Some(Press::Attack), true), DT);
        }
        assert!((a.dead_t - 1.0).abs() < 0.02, "{}", a.dead_t);
        assert!(a.is_dead(), "a dead body takes no press");
        a.die();
        assert_eq!((a.death, a.dead_t), (None, 0.0));
    }

    fn cast_spec() -> CastSpec {
        CastSpec {
            duration: 1.7,
            release: 0.77,
            recover: 1.23,
        }
    }

    #[test]
    fn a_cast_commits_the_body_until_its_recovery_then_takes_a_roll() {
        let mut a = Action::default();
        assert!(a.begin_cast(133, cast_spec(), None));
        assert!(matches!(a.state, ActionState::Cast(c) if c.spell == 133 && !c.released()));
        let s = full();
        // A roll pressed in the wind-up waits in the buffer; the body stays in the cast.
        assert_eq!(
            step_action(&mut a, &s, input(Some(Press::Evade), true), 0.3),
            Decision::None
        );
        assert!(matches!(a.state, ActionState::Cast(_)));
        // The release passes; still committed.
        step_action(&mut a, &s, input(None, true), 0.5);
        assert!(matches!(a.state, ActionState::Cast(c) if c.released() && !c.cancellable()));
        // From the recovery a press starts the roll.
        assert!(matches!(
            step_action(&mut a, &s, input(Some(Press::Evade), true), 0.5),
            Decision::Evade(_)
        ));
    }

    #[test]
    fn a_cast_ends_at_its_duration_and_a_blow_ends_it_at_once() {
        let mut a = Action::default();
        assert!(a.begin_cast(133, cast_spec(), None));
        assert_eq!(
            step_action(&mut a, &full(), input(None, true), 2.0),
            Decision::Ended
        );
        assert_eq!(a.state, ActionState::Idle);
        assert!(a.begin_cast(133, cast_spec(), None));
        a.stagger(0.4);
        assert!(
            matches!(a.state, ActionState::Stagger { .. }),
            "the cast is gone"
        );
    }

    #[test]
    fn a_cast_cannot_start_in_a_swing_that_is_not_yet_cancellable() {
        let mut a = Action::default();
        let swing = Swing {
            t: 0.1,
            kind: MoveKind::R1(0),
            spec: weapon_clips().r1[0],
            reference: true,
            serial: 1,
        };
        a.state = ActionState::Attack(swing);
        assert!(!a.can_cast());
        assert!(!a.begin_cast(133, cast_spec(), None));
        assert!(matches!(a.state, ActionState::Attack(_)), "nothing changed");
        a.state = ActionState::Dead;
        assert!(!a.can_cast());
    }

    #[test]
    fn every_cast_has_its_own_serial_so_a_release_happens_once_per_cast() {
        let mut a = Action::default();
        a.begin_cast(133, cast_spec(), None);
        let first = match a.state {
            ActionState::Cast(c) => c.serial,
            _ => unreachable!(),
        };
        a.state = ActionState::Idle;
        a.begin_cast(116, cast_spec(), None);
        let second = match a.state {
            ActionState::Cast(c) => c.serial,
            _ => unreachable!(),
        };
        assert_ne!(first, second);
    }

    #[test]
    fn a_cast_can_follow_a_swing_once_its_chain_window_is_open() {
        let mut a = Action::default();
        let spec = weapon_clips().r1[0];
        a.state = ActionState::Attack(Swing {
            t: spec.chain_from + 0.01,
            kind: MoveKind::R1(0),
            spec,
            reference: true,
            serial: 1,
        });
        assert!(a.can_cast(), "a swing's chain window takes a cast");
        assert!(a.begin_cast(133, cast_spec(), None));
    }
}
