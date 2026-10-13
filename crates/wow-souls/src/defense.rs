//! Sword guard and empty-left-hand parry. Clip windows and item/attack numbers are cache data;
//! the front half-plane and parryable melee timing are explicit rules of this prototype.

use std::collections::HashMap;
use std::path::Path;

use benilla_world::rig_anim::RigPose;
use bevy::prelude::*;

use crate::avatar::Avatar;
use crate::combat::{Action, ActionState, Stamina};
use crate::dsclip::{read_clip, Grip};
use crate::npc::{Brain, Hostile, Mind, Reaction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Raise,
    Hold,
    Lower,
    Parry,
    Break(u8),
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Raise => "raise",
            Self::Hold => "hold",
            Self::Lower => "lower",
            Self::Parry => "parry",
            Self::Break(0) => "break0",
            Self::Break(1) => "break1",
            Self::Break(2) => "break2",
            Self::Break(_) => "break3",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        [
            Self::Raise,
            Self::Hold,
            Self::Lower,
            Self::Parry,
            Self::Break(0),
            Self::Break(1),
            Self::Break(2),
            Self::Break(3),
        ]
        .into_iter()
        .find(|k| k.name() == s)
    }
    pub fn guarding(self) -> bool {
        matches!(self, Self::Raise | Self::Hold)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    pub duration: f32,
    pub chain: f32,
    pub evade: f32,
    pub movement: f32,
    pub active: Option<(f32, f32)>,
    pub cost: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Specs {
    pub raise: Spec,
    pub hold: Spec,
    pub lower: Spec,
    pub parry: Spec,
}

impl Specs {
    pub fn get(self, kind: Kind) -> Spec {
        match kind {
            Kind::Raise => self.raise,
            Kind::Hold => self.hold,
            Kind::Lower => self.lower,
            _ => self.parry,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playing {
    pub kind: Kind,
    pub t: f32,
    pub spec: Spec,
}

impl Playing {
    pub fn new(kind: Kind, spec: Spec) -> Self {
        Self { kind, t: 0.0, spec }
    }
    pub fn time(self) -> f32 {
        if self.kind == Kind::Hold {
            self.t % self.spec.duration
        } else {
            self.t
        }
    }
    pub fn active(self) -> bool {
        self.spec
            .active
            .is_some_and(|(from, to)| self.time() >= from && self.time() < to)
    }
}

#[derive(Clone)]
pub struct Clip {
    pub kind: Kind,
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    pub spec: Spec,
    pub push: Vec2,
}

#[derive(Debug, Clone, Copy)]
pub struct Attack {
    pub rate: f32,
    pub unblockable: bool,
}

#[derive(Message, Clone, Copy)]
pub struct Parried {
    pub point: Vec3,
}

#[derive(Resource)]
pub struct Kit {
    pub clips: Vec<Clip>,
    pub specs: Specs,
    pub cut: f32,
    pub stability: f32,
    pub hand: u16,
    pub radius: f32,
    pub grip: Grip,
    attacks: HashMap<(String, String), Attack>,
}

impl Kit {
    pub fn clip(&self, kind: Kind) -> Option<&Clip> {
        self.clips.iter().find(|c| c.kind == kind)
    }
    pub fn attack(&self, kit: &str, name: &str) -> Option<Attack> {
        self.attacks
            .get(&(kit.to_owned(), name.to_owned()))
            .copied()
    }
    pub fn broken(&self, push: Vec2) -> &Clip {
        let index = crate::monster::best_by_push(
            self.clips
                .iter()
                .enumerate()
                .filter(|(_, c)| matches!(c.kind, Kind::Break(_)))
                .map(|(i, c)| (i, c.push)),
            push,
        )
        .expect("a defense kit has four break clips");
        &self.clips[usize::from(index)]
    }
}

pub(crate) fn spec(clip: &wscl::Clip, kind: Kind, cost: f32) -> Result<Spec, String> {
    let duration = clip.duration();
    let open = |flag| {
        clip.windows(flag)
            .map(|w| w.0)
            .min_by(f32::total_cmp)
            .unwrap_or(duration)
    };
    let active = match kind {
        Kind::Raise | Kind::Hold => {
            // DSAnimStudio's DS1 event schema: 3 is Guarding; 85 is Light Lantern Weapon.
            Some(clip.windows(3).next().ok_or("guard clip lacks flag 3")?)
        }
        Kind::Parry => Some(
            clip.hits
                .iter()
                .find(|h| h.judge == 450)
                .map(|h| (h.start, h.end))
                .ok_or("parry lacks judge 450")?,
        ),
        _ => None,
    };
    if let Some((from, to)) = active {
        if !from.is_finite() || !to.is_finite() || from < 0.0 || to <= from || from >= duration {
            return Err("invalid defense window".into());
        }
    }
    if clip.windows(wscl::FLAG_INVINCIBLE).next().is_some() {
        return Err("defense clip has invulnerability".into());
    }
    Ok(Spec {
        duration,
        chain: open(4),
        evade: open(26),
        movement: open(11),
        active,
        cost,
    })
}

pub fn load(text: &str, dir: &Path) -> Result<Option<Kit>, String> {
    if !text.lines().any(|l| l.starts_with("defense-version ")) {
        return Ok(None);
    }
    let (mut cut, mut stability, mut hand, mut radius, mut stamina, mut grip) =
        (None, None, None, None, None, None);
    let mut lines = Vec::new();
    let mut attacks = HashMap::new();
    let number = |s: &str| {
        s.parse::<f32>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 0.0)
            .ok_or_else(|| format!("invalid defense number {s:?}"))
    };
    for line in text.lines() {
        let w: Vec<_> = line.split_whitespace().collect();
        let bad = || format!("bad defense line {line:?}");
        match w.first().copied() {
            Some("defense-version") => {
                if w != ["defense-version", "1"] {
                    return Err(bad());
                }
            }
            Some("defense") => {
                let [_, "cut", c, "stability", s, "hand", h] = w[..] else {
                    return Err(bad());
                };
                cut = Some(number(c)?);
                stability = Some(number(s)?);
                hand = Some(h.parse::<u16>().map_err(|_| bad())?);
            }
            Some("defparry") => {
                let [_, "stamina", s, "radius", r] = w[..] else {
                    return Err(bad());
                };
                stamina = Some(number(s)?);
                radius = Some(number(r)? * crate::shove::METRES_TO_YARDS);
            }
            Some("defgrip") => {
                let [_, x, y, z, a, b, c] = w[..] else {
                    return Err(bad());
                };
                let n = |s: &str| {
                    s.parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .ok_or_else(bad)
                };
                let hand = Vec3::new(n(x)?, n(y)?, n(z)?);
                let weapon = Vec3::new(n(a)?, n(b)?, n(c)?);
                if hand.length_squared() < 0.5 || weapon.length_squared() < 0.5 {
                    return Err(bad());
                }
                grip = Some(Grip { hand, weapon });
            }
            Some("defclip") => {
                let [_, k, clip, "push", f, l] = w[..] else {
                    return Err(bad());
                };
                let kind = Kind::parse(k).ok_or_else(bad)?;
                let push = Vec2::new(
                    f.parse::<f32>().map_err(|_| bad())?,
                    l.parse::<f32>().map_err(|_| bad())?,
                );
                if !push.is_finite() || lines.iter().any(|(x, _, _)| *x == kind) {
                    return Err(bad());
                }
                lines.push((kind, clip.to_string(), push));
            }
            Some("defattack") => {
                let [_, kit, name, "rate", rate, "unblockable", u] = w[..] else {
                    return Err(bad());
                };
                if !["0", "1"].contains(&u) {
                    return Err(bad());
                }
                if attacks
                    .insert(
                        (kit.to_string(), name.to_string()),
                        Attack {
                            rate: number(rate)?,
                            unblockable: u == "1",
                        },
                    )
                    .is_some()
                {
                    return Err(bad());
                }
            }
            _ => {}
        }
    }
    let (cut, stability, hand, radius, stamina, grip) = (
        cut.ok_or("missing defense cut")?,
        stability.ok_or("missing stability")?,
        hand.ok_or("missing parry hand")?,
        radius.ok_or("missing parry radius")?,
        stamina.ok_or("missing parry stamina")?,
        grip.ok_or("missing defense grip")?,
    );
    if cut > 1.0 || stability > 100.0 || radius <= 0.0 || stamina <= 0.0 {
        return Err("defense values out of range".into());
    }
    let roll = text
        .lines()
        .find_map(|line| line.strip_prefix("roll "))
        .ok_or("defense cache names no roll")?;
    let bones = read_clip(dir, "human_male", roll)?.bones();
    let ds_bones = read_clip(dir, "c0000", roll)?.bones();
    let mut clips = Vec::new();
    for (kind, name, push) in lines {
        let wow = read_clip(dir, "human_male", &name)?;
        let ds = read_clip(dir, "c0000", &name)?;
        if usize::from(hand) >= wow.bones() || bones != wow.bones() || ds_bones != ds.bones() {
            return Err("defense bone mismatch".into());
        }
        let spec = spec(&wow, kind, if kind == Kind::Parry { stamina } else { 0.0 })?;
        clips.push(Clip {
            kind,
            wow,
            ds,
            spec,
            push,
        });
    }
    let get = |kind| {
        clips
            .iter()
            .find(|c| c.kind == kind)
            .map(|c| c.spec)
            .ok_or_else(|| format!("missing defense {}", kind.name()))
    };
    let specs = Specs {
        raise: get(Kind::Raise)?,
        hold: get(Kind::Hold)?,
        lower: get(Kind::Lower)?,
        parry: get(Kind::Parry)?,
    };
    for i in 0..4 {
        get(Kind::Break(i))?;
    }
    Ok(Some(Kit {
        clips,
        specs,
        cut,
        stability,
        hand,
        radius,
        grip,
        attacks,
    }))
}

fn startup(mut commands: Commands) {
    let dir = crate::dsclip::cache_dir();
    match crate::dsclip::checked_manifest(&dir).and_then(|s| load(&s, &dir)) {
        Ok(Some(kit)) => {
            info!("defense: sword block {:.0}% cut, stability {:.0}, active from {:.3}s; bare-hand parry {:?}, {:.2} yd, stamina {:.0}",
                kit.cut * 100.0, kit.stability, kit.specs.raise.active.map_or(0.0, |w| w.0), kit.specs.parry.active, kit.radius, kit.specs.parry.cost);
            commands.insert_resource(kit);
        }
        Ok(None) => info!("defense: no defense records; run wow-souls-dsr defense"),
        Err(why) => warn!("defense: unavailable ({why})"),
    }
}

/// The block's arithmetic; zero stamina breaks guard. Unblockable and back attacks pass through.
pub fn block(cut: f32, stability: f32, damage: f32, rate: f32, stamina: f32) -> (f32, f32, bool) {
    let cost = rate * (1.0 - stability / 100.0);
    (damage * (1.0 - cut), cost, stamina <= cost)
}

pub fn in_front(body: &Transform, from: Vec3) -> bool {
    let d = Vec3::new(
        from.x - body.translation.x,
        0.0,
        from.z - body.translation.z,
    )
    .normalize_or_zero();
    d != Vec3::ZERO && d.dot(*body.forward()) > 0.0
}

/// Wind-up or an active melee hit can be caught until the final hit closes, including combos.
fn parry_time(t: f32, windows: &[(f32, f32)]) -> bool {
    windows.iter().any(|&(_, end)| t < end)
}

/// Resolve defense at the actual collision, before the damage message reaches effects/readers.
pub fn intercept(
    kit: Option<&Kit>,
    stats: Option<Attack>,
    body: &Transform,
    from: Vec3,
    action: &mut Action,
    stamina: &mut Stamina,
    damage: f32,
) -> (f32, bool) {
    let (Some(kit), Some(stats), ActionState::Defend(d)) = (kit, stats, action.state) else {
        return (damage, false);
    };
    if !d.kind.guarding() || !d.active() || stats.unblockable || !in_front(body, from) {
        return (damage, false);
    }
    let (taken, cost, broken) = block(kit.cut, kit.stability, damage, stats.rate, stamina.cur);
    stamina.spend(cost);
    if broken {
        let away = body.rotation.inverse() * (body.translation - from).normalize_or_zero();
        let clip = kit.broken(Vec2::new(-away.z, -away.x));
        action.state = ActionState::Defend(Playing::new(clip.kind, clip.spec));
        action.buffer = None;
        info!("defense: guard broken ({})", clip.wow.name);
    }
    info!(
        "defense: blocked {damage:.1} -> {taken:.1}, stamina -{cost:.1} = {:.1}",
        stamina.cur
    );
    (taken, true)
}

/// During the parry volume the swung limb must be in front and overlap the creature's hurtbox.
fn parry(
    kit: Option<Res<Kit>>,
    roster: Option<Res<crate::monster::Roster>>,
    avatar: Query<(&Action, &Transform, &GlobalTransform, &RigPose), With<Avatar>>,
    mut foes: Query<
        (
            &crate::species::Species,
            &GlobalTransform,
            &crate::hits::Hurtbox,
            &mut Brain,
            &mut Reaction,
            &mut crate::motor::MoveIntent,
            &mut crate::monster::MonsterPlay,
        ),
        With<Hostile>,
    >,
    mut previous: Local<Option<Vec3>>,
    mut parried_events: MessageWriter<Parried>,
) {
    let (Some(kit), Some(roster), Ok((action, body, at, pose))) = (kit, roster, avatar.single())
    else {
        *previous = None;
        return;
    };
    let ActionState::Defend(d) = action.state else {
        *previous = None;
        return;
    };
    if d.kind != Kind::Parry || !d.active() {
        *previous = None;
        return;
    }
    let Some(center) = pose.posed_point(at, kit.hand, Vec3::ZERO) else {
        return;
    };
    let old = previous.replace(center);
    for (species, xf, hurt, mut brain, mut reaction, mut intent, mut play) in &mut foes {
        let Mind::Attack { t, mv } = brain.mind else {
            continue;
        };
        let Some(monster) = roster.get(species.0.kit) else {
            continue;
        };
        let (Some(parried), Some(attack)) =
            (&monster.parried, monster.attacks.get(usize::from(mv)))
        else {
            continue;
        };
        let eligible = kit
            .attack(species.0.kit, &attack.spec.name)
            .is_some_and(|s| !s.unblockable);
        if !eligible || !parry_time(t, &attack.spec.hits) || !in_front(body, xf.translation()) {
            continue;
        }
        let volume = crate::foehit::Volume {
            ends: [center, center],
            radius: kit.radius * body.scale.y,
        };
        if !crate::foehit::touches(
            volume,
            old.map(|p| [p, p]),
            hurt.axis(xf.translation()),
            hurt.radius,
        ) {
            continue;
        }
        let lock = parried
            .wow
            .windows(41)
            .map(|w| w.1)
            .max_by(f32::total_cmp)
            .unwrap_or(parried.wow.duration())
            .min(parried.wow.duration());
        brain.mind = Mind::Cooldown { left: 0.3 };
        brain.struck = u32::MAX;
        brain.wish = None;
        brain.last = None;
        *reaction = Reaction::Flinch(lock);
        *intent = crate::motor::MoveIntent::default();
        play.clip = Some((crate::monster::MonsterClip::Parried, 0.0));
        parried_events.write(Parried { point: center });
        info!(
            "defense: parried {} {} at {:.3}s (hand {:.3}s, locked {:.3}s)",
            species.0.name, attack.spec.name, t, d.t, lock
        );
    }
}

pub fn plugin(app: &mut App) {
    app.add_message::<Parried>()
        .add_systems(Startup, startup)
        .add_systems(
            PostUpdate,
            parry
                .after(benilla_world::billboard::BillboardPlace)
                .before(crate::npc::strike)
                .run_if(in_state(crate::settle::Phase::Playing)),
        )
        .add_systems(
            Update,
            flashes.run_if(in_state(crate::settle::Phase::Playing)),
        );
}

fn flashes(
    real: Res<Time<bevy::time::Real>>,
    mut hits: MessageReader<crate::npc::PlayerHurt>,
    mut parries: MessageReader<Parried>,
    avatar: Query<&Transform, With<Avatar>>,
    mut points: Local<Vec<(Vec3, f32)>>,
    mut gizmos: Gizmos,
) {
    let now = real.elapsed_secs();
    if let Ok(body) = avatar.single() {
        for _ in hits.read().filter(|h| h.blocked) {
            points.push((
                body.translation + Vec3::Y * 1.15 + *body.forward() * 0.5,
                now,
            ));
        }
    }
    points.extend(parries.read().map(|p| (p.point, now)));
    points.retain(|(_, at)| now - at < 0.15);
    for &(p, at) in points.iter() {
        let age = (now - at) / 0.15;
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.0;
            let dir = Vec3::new(a.cos(), a.sin(), 0.35).normalize();
            gizmos.line(
                p + dir * age * 0.25,
                p + dir * (0.07 + age * 0.4),
                Color::srgba(1.0, 0.8, 0.35, 1.0 - age),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parry_can_catch_later_combo_hits_but_not_the_final_recovery() {
        let windows = [(0.5, 0.6), (1.1, 1.2)];
        assert!(parry_time(0.0, &windows));
        assert!(
            parry_time(0.7, &windows),
            "the second hit is still winding up"
        );
        assert!(parry_time(1.15, &windows), "the second hit is active");
        assert!(!parry_time(1.2, &windows));
        assert!(!parry_time(1.5, &windows));
        assert!(!parry_time(0.0, &[]));
    }

    #[test]
    fn install_defense_matches_windows_and_resolves_front_back_and_grab_hits() {
        if std::env::var_os("DSR_DIR").is_none() {
            return;
        }
        let dir = crate::dsclip::cache_dir();
        if !dir.join("manifest.txt").exists() {
            return;
        }
        let text = crate::dsclip::checked_manifest(&dir).unwrap();
        let Some(kit) = load(&text, &dir).unwrap() else {
            return;
        };
        assert!(kit.specs.raise.active.unwrap().0.abs() < 0.001);
        assert!((kit.specs.parry.active.unwrap().1 - 7.0 / 30.0).abs() < 0.001);
        assert_eq!(kit.specs.parry.cost, 14.0);
        let stats = kit.attack("kobold", "small_1");
        let mut a = Action {
            state: ActionState::Defend(Playing::new(Kind::Raise, kit.specs.raise)),
            ..default()
        };
        let mut stamina = Stamina::full(100.0);
        assert_eq!(
            intercept(
                Some(&kit),
                stats,
                &Transform::IDENTITY,
                Vec3::NEG_Z,
                &mut a,
                &mut stamina,
                25.0
            ),
            (25.0, false)
        );
        a.state = ActionState::Defend(Playing::new(Kind::Hold, kit.specs.hold));
        assert_eq!(
            intercept(
                Some(&kit),
                stats,
                &Transform::IDENTITY,
                Vec3::Z,
                &mut a,
                &mut stamina,
                25.0
            ),
            (25.0, false)
        );
        assert_eq!(stamina.cur, 100.0);
        assert_eq!(
            intercept(
                Some(&kit),
                kit.attack("kobold", "pounce"),
                &Transform::IDENTITY,
                Vec3::NEG_Z,
                &mut a,
                &mut stamina,
                22.0
            ),
            (22.0, false)
        );
        assert_eq!(
            intercept(
                Some(&kit),
                stats,
                &Transform::IDENTITY,
                Vec3::NEG_Z,
                &mut a,
                &mut stamina,
                25.0
            ),
            (12.5, true)
        );
        assert!((stamina.cur - 86.4).abs() < 0.001);
        stamina.cur = 13.6;
        assert!(
            intercept(
                Some(&kit),
                stats,
                &Transform::IDENTITY,
                Vec3::NEG_Z,
                &mut a,
                &mut stamina,
                25.0
            )
            .1
        );
        assert!(
            matches!(a.state, ActionState::Defend(d) if matches!(d.kind, Kind::Break(_)) && !d.active())
        );
        assert!(!a.invulnerable());
        let roster = crate::monster::load(&dir).unwrap();
        assert!(roster.get("wolf").unwrap().parried.is_none());
        assert!(roster.get("kobold").unwrap().parried.is_some());
        assert!(roster.get("thug").unwrap().parried.is_some());
        assert!(roster.get("garrick").unwrap().parried.is_some());
    }
    #[test]
    fn blocks_preserve_chip_damage_and_exact_stamina_break_threshold() {
        assert_eq!(block(0.5, 32.0, 25.0, 20.0, 100.0), (12.5, 13.6, false));
        assert!(block(0.5, 32.0, 25.0, 20.0, 13.6).2);
        assert!(!block(0.5, 32.0, 25.0, 20.0, 13.61).2);
    }
    #[test]
    fn guard_covers_the_front_half_only() {
        let body = Transform::IDENTITY;
        assert!(in_front(&body, Vec3::NEG_Z));
        assert!(!in_front(&body, Vec3::Z));
        assert!(!in_front(&body, Vec3::X));
        assert!(!in_front(&body, Vec3::ZERO));
    }
    #[test]
    fn defense_cache_absence_and_malformed_numbers_fail_safely() {
        assert!(load("wscl-cache 6", Path::new("absent")).unwrap().is_none());
        for n in ["NaN", "inf", "-1"] {
            assert!(load(
                &format!("defense-version 1\ndefense cut {n} stability 32 hand 95"),
                Path::new("absent")
            )
            .is_err());
        }
    }
}
