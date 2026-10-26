//! `WOWSOULS_FX_CENSUS=1`: every three seconds, log lines on the world's particle emitters within
//! 40 yd of the player (fires, braziers, candles, smoke): how many there are, how many have live
//! particles, are drawn, frozen, and the nearest ones with their place (WoW yd) and texture. An
//! instrument for "the fire is not there".

use benilla_world::particles::ParticleEmitter;
use bevy::prelude::*;

use crate::avatar::Avatar;

const RADIUS: f32 = 40.0;
const EVERY: f32 = 3.0;
const LISTED: usize = 24;

fn census(
    time: Res<Time>,
    assets: Res<AssetServer>,
    mut since: Local<f32>,
    avatar: Query<&Transform, With<Avatar>>,
    emitters: Query<(Entity, &ParticleEmitter)>,
) {
    *since += time.delta_secs();
    if *since < EVERY {
        return;
    }
    *since = 0.0;
    let Ok(me) = avatar.single() else {
        return;
    };
    let mut near: Vec<_> = emitters
        .iter()
        .map(|(e, p)| (e, p, p.anchor_world().distance(me.translation)))
        .filter(|(_, _, d)| *d <= RADIUS)
        .collect();
    near.sort_by(|a, b| a.2.total_cmp(&b.2));
    let live = near.iter().filter(|(_, e, _)| e.live() > 0).count();
    let drawn = near.iter().filter(|(_, e, _)| e.drawn()).count();
    let frozen = near.iter().filter(|(_, e, _)| e.is_frozen()).count();
    info!(
        "fx census: {} emitters within {RADIUS} yd ({} in the world), {live} with particles, {drawn} drawn, {frozen} frozen",
        near.len(),
        emitters.iter().count()
    );
    // Every texture among them, with how many emitters and how many of those are drawn.
    let mut kinds: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for (_, e, _) in &near {
        let name = assets
            .get_path(e.texture().id())
            .map_or_else(|| "-".to_string(), |p| p.to_string());
        let k = kinds.entry(name).or_default();
        k.0 += 1;
        k.1 += usize::from(e.drawn());
    }
    for (name, (n, drawn)) in &kinds {
        info!("fx census: texture {name}: {n} emitters, {drawn} drawn");
    }
    for (entity, e, d) in near.iter().take(LISTED) {
        let at = e.anchor_world();
        let texture = assets
            .get_path(e.texture().id())
            .map_or_else(|| "-".to_string(), |p| p.to_string());
        info!(
            "fx census: {entity} at {d:.1} yd wow ({:.1},{:.1},{:.1}), live {}, drawn {}, alpha {:.2}, frozen {}, {texture}",
            -at.z,
            -at.x,
            at.y,
            e.live(),
            e.drawn(),
            e.render_alpha(),
            e.is_frozen()
        );
    }
}

pub fn plugin(app: &mut App) {
    if std::env::var_os("WOWSOULS_FX_CENSUS").is_some() {
        app.add_systems(Update, census);
    }
}
