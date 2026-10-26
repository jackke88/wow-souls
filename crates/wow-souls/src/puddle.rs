//! Rain puddles you can splash through (2026-10-18, the user: "När det regnar blir det rainpuddles i marken
//! men när man springer på dem blir det ingen splash"). The puddles are drawn by the engine's terrain
//! shader alone (`wet_hook.wgsl` `wet_puddles`: a value-noise field over the ground, on flat ground, once
//! it is soaked, not under cover). [`amount`] is that function on the CPU, so a foot that lands where a
//! puddle is drawn knows it: [`crate::foot`] then plays WoW's water footstep (the splash kit of
//! `FootstepTerrainLookup`) and [`spray`] throws up WoW's own footstep spray
//! (`Particles\FootStepSprayWater.m2`, the model the client kicks up in shallow water).

use bevy::prelude::*;

use benilla_assets::m2_url;
use benilla_assets::M2Model;

/// Above this share a footfall counts as in a puddle (the shader's edge band is 0.06 wide).
pub const SPLASH_FROM: f32 = 0.35;
/// The spray models: running and walking.
const SPRAY_RUN: &str = "Particles\\FootStepSprayWater.m2";
const SPRAY_WALK: &str = "Particles\\FootStepSprayWaterWalk.m2";
/// Planar speed (yd/s) under which the walking spray is used.
const WALK_BELOW: f32 = 4.0;
/// Seconds a spray's anchor lives (the model is a short one-shot).
const SPRAY_LIFE: f32 = 1.0;
/// How many times the spray's droplets are thrown, and how much bigger than WoW's (taste).
const COPIES: usize = 1;
const DROPLET_SIZE: f32 = 1.6;
/// Seconds the spray throws droplets before its nozzle goes (the model's own track would go on).
const BURST: f32 = 0.1;

fn fract(x: f32) -> f32 {
    x - x.floor()
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `wet_hash`.
fn hash(p: Vec2) -> f32 {
    let q = Vec2::new(fract(p.x * 0.1031), fract(p.y * 0.1030));
    let d = q.dot(Vec2::new(q.y, q.x) + 33.33);
    let r = q + d;
    fract((r.x + r.y) * r.x)
}

/// `wet_noise`.
fn noise(p: Vec2) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let a = hash(i);
    let b = hash(i + Vec2::X);
    let c = hash(i + Vec2::Y);
    let d = hash(i + Vec2::ONE);
    let lo = a + (b - a) * u.x;
    let hi = c + (d - c) * u.x;
    lo + (hi - lo) * u.y
}

/// How much puddle the terrain shader draws at Bevy `xz` on ground whose normal has `normal_y`, under
/// `wetness` (0 dry .. 1 soaked) and `exposure` (1 open sky .. 0 under cover): `wet_puddles`, 0..1.
pub fn amount(xz: Vec2, normal_y: f32, wetness: f32, exposure: f32) -> f32 {
    if wetness <= 0.0 || exposure <= 0.0 {
        return 0.0;
    }
    let field = 0.65 * noise(xz * 0.17) + 0.35 * noise(xz * 0.53 + Vec2::splat(17.0));
    let level = smoothstep(0.94, 0.99, normal_y);
    let edge = 0.8 + (0.6 - 0.8) * smoothstep(0.4, 1.0, wetness);
    smoothstep(edge, edge + 0.06, field)
        * level
        * smoothstep(0.35, 0.8, wetness)
        * exposure.clamp(0.0, 1.0)
}

/// The puddle under a footfall at `feet`: the ground's normal from a short ray, the engine's wetness
/// and its rain shelter.
pub fn under(
    collide: &benilla_world::collision::WorldCollision,
    wetness: Option<&benilla_world::weather::Wetness>,
    shelter: Option<&benilla_world::weather::shelter::ShelterGrid>,
    feet: Vec3,
) -> f32 {
    // The shader reads the published row, which the engine's capture knob `WOW_WETNESS` pins.
    static PINNED: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    let pinned = *PINNED.get_or_init(|| {
        std::env::var("WOW_WETNESS")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .map(|v| v.clamp(0.0, 1.0))
    });
    let wet = pinned.unwrap_or_else(|| wetness.map_or(0.0, |w| w.wetness));
    if wet <= 0.0 {
        return 0.0;
    }
    let Some(hit) = collide.ray_body(feet + Vec3::Y * 0.5, Dir3::NEG_Y, 1.2) else {
        return 0.0;
    };
    let exposure = 1.0 - shelter.map_or(0.0, |s| s.shelter_at(feet));
    trace!(
        "puddle: normal {:.3} exposure {exposure:.2} wet {wet:.2} field at {:?}",
        hit.normal.y,
        Vec2::new(feet.x, feet.z)
    );
    amount(Vec2::new(feet.x, feet.z), hit.normal.y, wet, exposure)
}

/// The spray models, loaded once.
#[derive(Resource)]
pub struct Spray {
    run: Handle<M2Model>,
    walk: Handle<M2Model>,
}

/// One splash at a footfall: its model until the emitters are built, then how long it has lived.
#[derive(Component)]
struct Splash {
    model: Handle<M2Model>,
    built: bool,
    age: f32,
    /// What the emitters ride: gone after [`BURST`], which stops them and lets the droplets fall.
    nozzle: Option<Entity>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load).add_systems(
        Update,
        (build, reap, trace).run_if(in_state(crate::settle::Phase::Playing)),
    );
}

fn load(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Spray {
        run: assets.load(m2_url(SPRAY_RUN)),
        walk: assets.load(m2_url(SPRAY_WALK)),
    });
}

/// Throws WoW's footstep spray up at `at` (a foot on the water or in the puddle), the walking one for
/// a body slower than [`WALK_BELOW`].
pub fn spray(commands: &mut Commands, spray: &Spray, at: Vec3, speed: f32, facing: Quat) {
    let model = if speed < WALK_BELOW {
        spray.walk.clone()
    } else {
        spray.run.clone()
    };
    commands.spawn((
        Name::new("Footstep spray"),
        Transform::from_translation(at).with_rotation(facing),
        Visibility::default(),
        Splash {
            model,
            built: false,
            age: 0.0,
            nozzle: None,
        },
    ));
}

/// The spray's emitters, as [`crate::blood`] builds a spurt: WoW's own is sixteen specks of 0.03 yd for
/// a third of a second, so the droplets come [`COPIES`] times, each [`DROPLET_SIZE`] times bigger.
fn build(
    mut commands: Commands,
    m2s: Res<Assets<M2Model>>,
    mut splashes: Query<(Entity, &mut Splash)>,
) {
    use benilla_world::particles::{
        spawn_emitter, EmitClock, EmitterFrames, OwnerLoss, ParticleEmitter,
    };
    for (root, mut s) in &mut splashes {
        if s.built {
            continue;
        }
        let Some(model) = m2s.get(&s.model) else {
            continue;
        };
        s.built = true;
        s.age = 0.0;
        let nozzle = commands
            .spawn((Transform::default(), Visibility::default(), ChildOf(root)))
            .id();
        s.nozzle = Some(nozzle);
        for em in &model.emitters {
            for copy in 0..COPIES {
                let frames = EmitterFrames {
                    owner: Some((nozzle, [0.0; 3])),
                    anchor: Some(root),
                    alpha: None,
                    light_node: None,
                    on_owner_loss: OwnerLoss::Drain,
                };
                // The placement seeds the emitter's random numbers: a copy needs its own.
                let seed = Transform::from_translation(Vec3::splat(copy as f32 * 0.0137));
                let Some(e) =
                    spawn_emitter(&mut commands, em, seed, frames, EmitClock::Effect(None))
                else {
                    continue;
                };
                commands.entity(e).queue(|mut e: EntityWorldMut| {
                    if let Some(mut p) = e.get_mut::<ParticleEmitter>() {
                        p.set_size_scale(DROPLET_SIZE);
                    }
                });
            }
        }
    }
}

/// `debug`: the live droplets of the splashes, every 0.1 s while there are any.
fn trace(
    real: Res<Time<Real>>,
    splashes: Query<&Splash>,
    emitters: Query<&benilla_world::particles::ParticleEmitter>,
    mut next_at: Local<f32>,
) {
    let now = real.elapsed_secs();
    if now < *next_at || splashes.is_empty() {
        return;
    }
    *next_at = now + 0.1;
    let live: usize = emitters
        .iter()
        .filter(|e| e.anchor().is_some_and(|a| splashes.contains(a)))
        .map(benilla_world::particles::ParticleEmitter::live)
        .sum();
    debug!(
        "puddle: {} splash(es), {} built, {live} live droplets",
        splashes.iter().count(),
        splashes.iter().filter(|s| s.built).count()
    );
}

/// A splash goes [`SPRAY_LIFE`] after it was built (its emitters with it), or after a model that never
/// loads.
fn reap(mut commands: Commands, time: Res<Time>, mut splashes: Query<(Entity, &mut Splash)>) {
    for (e, mut s) in &mut splashes {
        s.age += time.delta_secs();
        if s.built && s.age > BURST {
            if let Some(n) = s.nozzle.take() {
                commands.entity(n).despawn();
            }
        }
        if s.age > if s.built { SPRAY_LIFE } else { 10.0 } {
            commands.entity(e).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_ground_slopes_and_cover_have_no_puddles() {
        let xz = Vec2::new(12.0, -40.0);
        assert_eq!(amount(xz, 1.0, 0.0, 1.0), 0.0, "dry");
        assert_eq!(amount(xz, 0.9, 1.0, 1.0), 0.0, "a slope");
        assert_eq!(amount(xz, 1.0, 1.0, 0.0), 0.0, "under a roof");
    }

    #[test]
    fn soaked_flat_ground_has_puddles_in_patches() {
        // Over a stretch of soaked flat ground some places hold water and most do not.
        let mut wet = 0;
        let n = 4000;
        for i in 0..n {
            let xz = Vec2::new((i % 80) as f32 * 0.7, (i / 80) as f32 * 0.7);
            if amount(xz, 1.0, 1.0, 1.0) > SPLASH_FROM {
                wet += 1;
            }
        }
        assert!(wet > n / 50 && wet < n / 2, "{wet} of {n}");
    }

    #[test]
    fn the_noise_matches_the_shader_hash_at_a_known_point() {
        // wet_hash(vec2(1, 2)): q = (0.1031, 0.2060); q + dot(q, q.yx + 33.33) = (6.9925, 7.0954),
        // fract((r.x + r.y) * r.x) by hand.
        let q = Vec2::new(0.1031, 0.2060);
        let d = q.dot(Vec2::new(q.y, q.x) + 33.33);
        let r = q + d;
        let want = ((r.x + r.y) * r.x).fract();
        assert!((hash(Vec2::new(1.0, 2.0)) - want).abs() < 1.0e-5);
    }
}
