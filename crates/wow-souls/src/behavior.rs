//! Family tactics. Decisions use our current kits and yards; no source-game runtime is required.
//! See docs/plans/2026-10-16-enemy-family-ai-design.md for the approved reference mapping.
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    KoboldSkirmisher,
    WolfPack,
    DefiasRogue,
    DefiasBrawler,
    HeavyRaider,
    GnollPressure,
    MurlocSwarm,
    BoarCharge,
    BearPressure,
    ProtectedLeader,
    SpiderAmbush,
    CasterKite,
    LeaderEscort,
    EliteDuelist,
    BossCommitted,
}

impl Role {
    pub fn id(self) -> &'static str {
        match self {
            Self::KoboldSkirmisher => "kobold_skirmisher",
            Self::WolfPack => "wolf_pack",
            Self::DefiasRogue => "defias_rogue",
            Self::DefiasBrawler => "defias_brawler",
            Self::HeavyRaider => "heavy_raider",
            Self::GnollPressure => "gnoll_pressure",
            Self::MurlocSwarm => "murloc_swarm",
            Self::BoarCharge => "boar_charge",
            Self::BearPressure => "bear_pressure",
            Self::ProtectedLeader => "protected_leader",
            Self::SpiderAmbush => "spider_ambush",
            Self::CasterKite => "caster_kite",
            Self::LeaderEscort => "leader_escort",
            Self::EliteDuelist => "elite_duelist",
            Self::BossCommitted => "boss_committed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Assess,
    Approach,
    Aim,
    Engage,
    Withdraw,
    Flank,
    Gather,
    Protect,
    Recover,
}

#[derive(Debug, Default)]
pub struct State {
    pub phase: Phase,
    pub wait: f32,
    left: f32,
    age: f32,
    side: f32,
    point: Option<Vec3>,
    sorties: u32,
    open_for: f32,
    escaped: bool,
    seed: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct Context {
    pub me: Vec3,
    pub target: Vec3,
    pub forward: Vec3,
    pub facing: f32,
    pub reach: f32,
    pub near: f32,
    pub speed: f32,
    pub admitted: bool,
    pub open: bool,
    pub guarding: bool,
    pub sight: bool,
    pub can_attack: bool,
    pub allies: usize,
    pub center: Vec3,
    pub leader: Option<Vec3>,
}

#[derive(Debug, Clone, Copy)]
pub struct Decision {
    pub goal: Vec3,
    pub pace: f32,
    pub attack: bool,
}

impl State {
    pub fn reset(&mut self) {
        *self = Self {
            seed: self.seed,
            ..Self::default()
        };
    }

    pub fn seed(&mut self, seed: u64) {
        if self.seed == 0 {
            self.seed = seed.wrapping_add(1);
        }
        if self.side == 0.0 {
            self.side = if self.seed & 1 == 0 { 1.0 } else { -1.0 };
        }
    }

    fn enter(&mut self, phase: Phase, seconds: f32) {
        self.phase = phase;
        self.left = seconds;
        self.age = 0.0;
        self.point = None;
    }

    pub fn ready(&self) -> bool {
        matches!(self.phase, Phase::Engage | Phase::Protect)
    }

    pub fn started(&mut self) {
        self.wait = 0.0;
    }

    /// Called only once the entire existing combo (including a spider's forced hop) is over.
    pub fn finished(&mut self, role: Role, variant: &str) {
        self.sorties += 1;
        self.side = -self.side;
        self.wait = 0.0;
        match role {
            Role::KoboldSkirmisher if !matches!(variant, "worker" | "narg" | "ruklar") => {
                self.enter(Phase::Withdraw, 0.65)
            }
            Role::WolfPack => self.enter(
                Phase::Flank,
                if matches!(variant, "young_wolf" | "mangy_wolf") {
                    1.1
                } else if variant == "gray_forest_wolf" {
                    0.55
                } else {
                    0.85
                },
            ),
            Role::DefiasRogue => self.enter(Phase::Flank, 0.55),
            Role::EliteDuelist => self.enter(Phase::Withdraw, 0.55),
            Role::MurlocSwarm => self.enter(Phase::Gather, 0.75),
            Role::ProtectedLeader | Role::LeaderEscort => self.enter(Phase::Protect, 0.0),
            Role::CasterKite => self.enter(Phase::Assess, 0.0),
            Role::HeavyRaider => self.enter(Phase::Recover, 0.7),
            Role::GnollPressure => self.enter(Phase::Recover, 0.5),
            Role::BearPressure => self.enter(Phase::Recover, 0.45),
            Role::SpiderAmbush => self.enter(Phase::Recover, 0.8),
            // Boars barely stop between blows: a short snort, then the next aim.
            Role::BoarCharge => self.enter(Phase::Recover, 0.3),
            _ => self.enter(Phase::Recover, 0.25),
        }
    }
}

impl Context {
    pub fn distance(self) -> f32 {
        flat(self.target - self.me).length()
    }
    fn toward(self) -> Vec3 {
        flat(self.target - self.me).normalize_or_zero()
    }
    fn right(self) -> Vec3 {
        self.toward().cross(Vec3::Y)
    }
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

impl Decision {
    fn hold(c: Context) -> Self {
        Self {
            goal: c.me,
            pace: 0.0,
            attack: false,
        }
    }
    fn go(goal: Vec3, pace: f32) -> Self {
        Self {
            goal,
            pace,
            attack: false,
        }
    }
}

/// Pure decisions in world space. Waypoints stay fixed for a phase, so repositioning ends
/// instead of continuously moving its destination around the player.
pub fn step(role: Role, variant: &str, s: &mut State, c: Context, dt: f32) -> Decision {
    s.seed(0);
    let dt = if dt.is_finite() {
        dt.clamp(0.0, 0.2)
    } else {
        0.0
    };
    s.age += dt;
    s.wait += dt;
    s.left = (s.left - dt).max(0.0);
    s.open_for = if c.open { s.open_for + dt } else { 0.0 };
    let dist = c.distance();
    let close = c.near.min(c.reach).max(0.4);
    if role == Role::BossCommitted {
        return Decision::go(c.target, 1.0);
    }

    // Ranged combat has a finite escape, and a real line-of-sight gate.
    if role == Role::CasterKite {
        if dist >= 4.5 {
            s.escaped = false;
        }
        if !c.sight {
            s.enter(Phase::Approach, 0.0);
            return Decision::go(c.target, 0.6);
        }
        if s.phase != Phase::Withdraw && dist < 3.0 && !s.escaped {
            s.enter(Phase::Withdraw, 0.75);
            s.escaped = true;
        }
        if s.phase != Phase::Withdraw {
            // Separate start/stop distances keep a retreating target from toggling
            // Approach/Engage every frame and continually stealing the casting turn.
            s.phase = if !c.can_attack || dist > 7.5 || (s.phase == Phase::Approach && dist > 6.5) {
                Phase::Approach
            } else {
                Phase::Engage
            };
        }
    }

    if role == Role::ProtectedLeader && c.allies > 0 {
        s.phase = Phase::Protect;
        if dist <= close && c.admitted {
            return Decision {
                attack: true,
                ..Decision::hold(c)
            };
        }
        let behind = c.center - flat(c.target - c.center).normalize_or_zero() * 2.0;
        return Decision::go(behind, 0.65);
    }
    if role == Role::LeaderEscort {
        if let Some(leader) = c.leader {
            if flat(leader - c.target).length() > 4.0 && dist > close + 0.8 {
                s.phase = Phase::Protect;
                let intercept = leader
                    + flat(c.target - leader).normalize_or_zero() * 2.0
                    + c.right() * s.side * 0.6;
                return Decision::go(intercept, 0.8);
            }
            if flat(leader - c.target).length() > 4.0
                && matches!(s.phase, Phase::Assess | Phase::Protect)
            {
                s.enter(Phase::Flank, 0.35);
            }
        }
    }
    if s.phase == Phase::Protect {
        s.enter(Phase::Approach, 0.0);
    }

    for _ in 0..4 {
        match s.phase {
            Phase::Assess => {
                match role {
                    Role::WolfPack | Role::DefiasRogue | Role::EliteDuelist => s.enter(
                        Phase::Flank,
                        if role == Role::WolfPack { 0.75 } else { 0.45 },
                    ),
                    Role::MurlocSwarm if c.allies > 0 => s.enter(Phase::Gather, 0.65),
                    Role::BoarCharge | Role::SpiderAmbush | Role::HeavyRaider
                        if dist <= c.reach =>
                    {
                        s.enter(Phase::Aim, aim_time(role));
                    }
                    Role::KoboldSkirmisher if dist <= close + 1.0 => {
                        // A brief readable assessment; a player attack must be observed for 0.25 s.
                        let pause = if variant == "vermin" {
                            0.4
                        } else if variant == "goldtooth" && s.open_for < 0.25 {
                            0.55
                        } else {
                            0.25
                        };
                        if s.age < pause {
                            return Decision::hold(c);
                        }
                        s.enter(Phase::Approach, 0.0);
                    }
                    _ => s.enter(Phase::Approach, 0.0),
                }
            }
            Phase::Approach => {
                let reach = if !c.can_attack {
                    close
                } else {
                    match role {
                        Role::CasterKite => 6.5,
                        Role::BearPressure
                        | Role::GnollPressure
                        | Role::DefiasBrawler
                        | Role::LeaderEscort
                        | Role::ProtectedLeader => close,
                        _ => c.reach,
                    }
                };
                if dist > reach - 0.05 {
                    if role == Role::MurlocSwarm
                        && matches!(variant, "streamrunner" | "lurker")
                        && dist > close + 0.4
                    {
                        return Decision::go(
                            c.target - flat(c.forward).normalize_or_zero() * 0.9
                                + c.right() * s.side,
                            1.0,
                        );
                    }
                    if role == Role::GnollPressure && c.allies > 0 && dist > close + 1.0 {
                        return Decision::go(
                            c.target - c.toward() * close * 0.4 + c.right() * s.side * 0.7,
                            0.9,
                        );
                    }
                    // A boar charges into the fight: it sprints the whole way in.
                    if role == Role::BoarCharge {
                        return Decision::go(c.target, BOAR_RUSH);
                    }
                    return Decision::go(c.target, 1.0);
                }
                if matches!(
                    role,
                    Role::BoarCharge | Role::SpiderAmbush | Role::HeavyRaider
                ) {
                    s.enter(Phase::Aim, aim_time(role));
                } else {
                    s.enter(Phase::Engage, 0.0);
                }
            }
            Phase::Aim => {
                if dist > c.reach + 0.5 {
                    s.enter(Phase::Approach, 0.0);
                    continue;
                }
                if s.left > 0.0 || c.facing < 0.94 {
                    return Decision::hold(c);
                }
                s.enter(Phase::Engage, 0.0);
            }
            Phase::Engage => {
                if dist > c.reach + 0.1 {
                    s.enter(Phase::Approach, 0.0);
                    continue;
                }
                // A waiting pack member takes a fixed side position; other roles hold their ground.
                if !c.admitted {
                    if matches!(
                        role,
                        Role::WolfPack | Role::KoboldSkirmisher | Role::DefiasRogue
                    ) {
                        let offset =
                            (-c.toward() * 0.8 + c.right() * s.side * 0.6).normalize_or_zero();
                        let point = *s
                            .point
                            .get_or_insert(c.target + offset * (c.reach - 0.15).max(0.4));
                        return Decision::go(point, 0.55);
                    }
                    return Decision::hold(c);
                }
                if !c.can_attack && dist > close - 0.05 {
                    s.enter(Phase::Approach, 0.0);
                    continue;
                }
                return Decision {
                    attack: c.sight,
                    ..Decision::hold(c)
                };
            }
            Phase::Withdraw => {
                let length = (if role == Role::CasterKite {
                    2.2_f32
                } else if variant == "vermin" {
                    1.6
                } else {
                    1.1
                })
                .min(c.speed * 0.7);
                let point = *s
                    .point
                    .get_or_insert(if variant == "geomancer" && c.allies > 0 {
                        c.center - flat(c.target - c.center).normalize_or_zero() * 1.4
                    } else {
                        c.me - c.toward() * length + c.right() * s.side * 0.55
                    });
                if s.left == 0.0 || flat(point - c.me).length() < 0.2 {
                    s.enter(Phase::Recover, 0.2);
                    continue;
                }
                return Decision::go(point, 0.65);
            }
            Phase::Flank => {
                // Opportunists react to a sustained visible opening, not a fresh input frame.
                if s.left == 0.0 || (role != Role::WolfPack && s.open_for >= 0.25) {
                    s.enter(Phase::Approach, 0.0);
                    continue;
                }
                let margin = if matches!(variant, "young_wolf" | "mangy_wolf") {
                    1.8
                } else if variant == "gray_forest_wolf" {
                    0.65
                } else {
                    1.1
                };
                let radius = (close + margin).clamp(1.5, 3.8);
                let point = *s.point.get_or_insert(
                    c.target - flat(c.forward).normalize_or_zero() * 0.65
                        + c.right() * radius * s.side,
                );
                return Decision::go(point, if role == Role::WolfPack { 0.75 } else { 0.6 });
            }
            Phase::Gather => {
                if c.allies == 0 || s.left == 0.0 {
                    s.enter(Phase::Approach, 0.0);
                    continue;
                }
                let point =
                    *s.point
                        .get_or_insert(if flat(c.center - c.target).length() < close + 0.8 {
                            c.me - c.toward() * 1.5
                        } else {
                            c.center
                                + if variant == "forager" {
                                    c.toward()
                                } else {
                                    Vec3::ZERO
                                }
                        });
                return Decision::go(point, 0.55);
            }
            Phase::Recover => {
                if s.left > 0.0 {
                    if role == Role::BearPressure && dist > close + 0.4 {
                        return Decision::go(c.target, 0.45);
                    }
                    return Decision::hold(c);
                }
                s.enter(Phase::Approach, 0.0);
            }
            Phase::Protect => return Decision::hold(c),
        }
    }
    Decision::hold(c)
}

/// A boar's pace when it is out of reach: a sprint past its chase speed (4.8 -> 6.5 yd/s).
pub const BOAR_RUSH: f32 = 1.35;

fn aim_time(role: Role) -> f32 {
    match role {
        Role::BoarCharge => 0.25,
        Role::SpiderAmbush => 0.8,
        _ => 0.4,
    }
}

/// Only request named openers that exist in this table and whose real band fits.
/// The NPC loop also checks the actual kit. All other selections keep the weighted pattern.
pub fn preferred(
    role: Role,
    state: &State,
    c: Context,
    table: &[crate::pattern::Row],
) -> Option<usize> {
    let dist = c.distance();
    let names: &[&str] = match role {
        Role::BoarCharge => &["charge", "gore"],
        Role::SpiderAmbush => &["leap", "spit", "bite"],
        Role::WolfPack => &["lunge", "bite"],
        Role::MurlocSwarm => &["lunge", "flurry"],
        // A caster with a heavy bolt (Surena) lets its weighted table choose between the two.
        Role::CasterKite if dist >= 2.5 && !table.iter().any(|r| r.name == "heavy_bolt") => {
            &["bolt"]
        }
        Role::CasterKite => &["swat", "small_1"],
        Role::DefiasBrawler if c.guarding => &["kick"],
        Role::HeavyRaider if c.guarding => &["axe_2"],
        Role::BearPressure if state.sorties.is_multiple_of(2) => &["swipe", "bite"],
        Role::BearPressure => &["bite", "swipe"],
        _ => &[],
    };
    names.iter().find_map(|name| {
        table
            .iter()
            .position(|r| r.name == *name && r.weight > 0 && dist >= r.band.0 && dist <= r.band.1)
    })
}

#[derive(Debug, Clone, Copy)]
pub struct Actor {
    pub id: u64,
    pub role: Role,
    pub at: Vec3,
    pub attacking: bool,
    pub ready: bool,
    pub wait: f32,
}

/// Two attacks per local encounter; a wolf pack reserves one of those for its active hunter.
/// Waiting time rotates priority independently of ECS query order.
pub fn admissions(actors: &[Actor], target: Vec3) -> Vec<u64> {
    let near = |a: &&Actor| flat(a.at - target).length_squared() <= 18.0 * 18.0;
    let busy = actors.iter().filter(near).filter(|a| a.attacking).count();
    let mut wolves = actors
        .iter()
        .filter(near)
        .filter(|a| a.attacking && a.role == Role::WolfPack)
        .count();
    let mut candidates: Vec<_> = actors
        .iter()
        .filter(near)
        .filter(|a| a.ready && !a.attacking)
        .collect();
    candidates.sort_by(|a, b| b.wait.total_cmp(&a.wait).then(a.id.cmp(&b.id)));
    let mut admitted = Vec::new();
    for actor in candidates {
        if admitted.len() >= 2usize.saturating_sub(busy) {
            break;
        }
        if actor.role == Role::WolfPack && wolves >= 1 {
            continue;
        }
        admitted.push(actor.id);
        if actor.role == Role::WolfPack {
            wolves += 1;
        }
    }
    admitted
}

/// Related allies within one local fight. Summoned escorts select a nearby living leader.
pub fn group(role: Role, id: u64, me: Vec3, actors: &[Actor]) -> (usize, Vec3, Option<Vec3>) {
    let mut count = 0;
    let mut center = Vec3::ZERO;
    let mut leader = None;
    let mut leader_dist = f32::MAX;
    for a in actors {
        if a.id == id || flat(a.at - me).length_squared() > 12.0 * 12.0 {
            continue;
        }
        if a.role == role
            || (role == Role::ProtectedLeader && a.role == Role::LeaderEscort)
            || (role == Role::CasterKite && a.role == Role::KoboldSkirmisher)
        {
            count += 1;
            center += a.at;
        }
        if role == Role::LeaderEscort && a.role == Role::ProtectedLeader {
            let d = flat(a.at - me).length_squared();
            if d < leader_dist {
                leader_dist = d;
                leader = Some(a.at);
            }
        }
    }
    (
        count,
        if count > 0 { center / count as f32 } else { me },
        leader,
    )
}

/// Probe only the next short step: the motor still owns full-size collision and sliding.
/// Start outside the creature's capsule so line checks do not hit its own body.
pub fn safe_direction(
    world: &benilla_world::collision::WorldCollision,
    feet: Vec3,
    wish: Vec3,
    radius: f32,
) -> Vec3 {
    let wish = flat(wish).normalize_or_zero();
    if wish == Vec3::ZERO {
        return wish;
    }
    for angle in [0.0_f32, 0.65, -0.65] {
        let dir = Quat::from_rotation_y(angle) * wish;
        let origin = feet + Vec3::Y * 0.8 + dir * (radius + 0.4);
        if world
            .cast_camera(origin, dir * 0.65, false)
            .is_some_and(|d| d < 0.6)
        {
            continue;
        }
        let at = feet + dir * (radius + 0.4).max(0.7) + Vec3::Y;
        if world
            .ray_body(at, Dir3::NEG_Y, 2.5)
            .is_some_and(|h| h.distance >= 0.35 && h.distance <= 2.0)
        {
            return dir;
        }
    }
    Vec3::ZERO
}

pub fn sight(
    world: &benilla_world::collision::WorldCollision,
    c: Context,
    radius: f32,
    player_radius: f32,
) -> bool {
    let toward = c.toward();
    let from = c.me + Vec3::Y * 1.1 + toward * (radius + 0.4);
    let to = c.target + Vec3::Y * 1.1 - toward * (player_radius + 0.4);
    let delta = to - from;
    delta.dot(toward) <= 0.0
        || world
            .cast_camera(from, delta, false)
            .is_none_or(|hit| hit >= delta.length() - 0.1)
}

#[cfg(test)]
#[path = "behavior/tests.rs"]
mod tests;
