//! WoW's own effect models on a unit: the corpse's loot sparkles, the open chest's glow, the level-up
//! flash and the abilities' spell models ([`crate::abilities::fx`]). Native model, bones, loop and
//! emitters, without retuning.
//!
//! An instance is built the way the reference client attaches a spell-visual effect model (benilla's
//! `entities/spell_fx`: `attach_effect_visuals`, `arm_effect_rig`, `fx_part_material`, `tick_fx_tint`):
//!
//! - the model's rig is its own joint hierarchy under the instance root, played by an
//!   `AnimationPlayer` from the first sequence (once for a flash, looping for the sparkles);
//! - every batch is a child part (skinned by the instance's palette slot) whose material-alpha loops
//!   run on the instance's own clock ([`MatAnim`]), and whose animated colour and texture transform
//!   run on a per-instance clone of the material, so one cast is one phase;
//! - a flat ground quad (Battle Shout's crescents) is drawn as a decal that drapes the terrain
//!   ([`benilla_world::ground_fx`]), as the engine does, not as a depth-tested batch that a slope buries;
//! - billboard batches follow their joint as cards, and the emitters and ribbons ride the joints.
//!
//! A one-shot effect's geometry goes when its sequence ends (the client's span), its particles a
//! second later.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::animation::{AnimatedBy, RepeatAnimation};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::mesh::MeshTag;
use bevy::prelude::*;

use benilla_assets::materials::WowModelMaterial;
use benilla_assets::{bone_target_id, m2_url, LockRecover, M2Model};
use benilla_formats::{AlphaAnim, GroundQuad, ModelBlend, RgbAnim};
use benilla_world::billboard::{BillboardCard, BillboardJointRig};
use benilla_world::doodad_anim::{register_fx_uv, MatAnim, UvAnimMaterials, UvLoops};
use benilla_world::mat_anim_table::MatAnimTable;
use benilla_world::model_forms::ModelForms;
use benilla_world::model_render::{EntityUvLane, M2BatchMaterials, ModelKind, ModelPart};
use benilla_world::particles::{
    spawn_emitter, EmitClock, EmitterFrames, OwnerLoss, ParticleEmitter,
};
use benilla_world::ribbons::{spawn_ribbon, RibbonSeq};
use benilla_world::rig_anim::{GlobalSeqDrive, RigPose};
use benilla_world::rig_palette::{spawn_joints, RigPalettes, RigPart, RigSkin};

use crate::actor::{ActorModel, Catalogs};
use crate::settle::Phase;

#[derive(Resource)]
pub(crate) struct Art {
    model: Handle<M2Model>,
    /// The level-up flash, `"HARDCODED Unit Level Up"` (a one-shot on the unit's own clip).
    level_up: Option<Handle<M2Model>>,
}

/// A native effect model on an owner: its first sequence plays (looping for the loot sparkles, once
/// for a flash that then removes itself) and everything it draws is built as the client builds it.
#[derive(Component)]
pub(super) struct Sparkle {
    owner: Entity,
    model: Handle<M2Model>,
    attached: bool,
    built: bool,
    /// The owner's attachment point the effect rides (19 for a corpse); `None` stays at the owner's origin.
    attach: Option<u16>,
    /// Plays once and removes itself a second after the clip ended.
    once: bool,
    /// A one-shot's geometry (parts, decals, cards) goes when its sequence ends.
    visuals_until: Option<f32>,
    die_at: Option<f32>,
}

/// A drawn piece of an effect instance (a part, a card or a ground decal) and its root.
#[derive(Component)]
struct FxVisual(Entity);

#[derive(Component)]
struct LootEmitter;

/// Per-instance colour loops: material id -> (the M2Color loop, the time the instance attached). One
/// cast is one phase, so an animated colour cannot share the doodad lane's global clock; an entry
/// drops with its material.
#[derive(Resource, Default)]
pub(crate) struct FxTints(HashMap<bevy::asset::AssetId<WowModelMaterial>, (Arc<RgbAnim>, f32)>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FxTints>()
        .add_systems(OnEnter(Phase::Playing), load)
        .add_systems(
            Update,
            (build, tick_tints, settle, reap, trace)
                .chain()
                .run_if(in_state(Phase::Playing)),
        );
}

fn load(mut commands: Commands, catalogs: Option<Res<Catalogs>>, assets: Res<AssetServer>) {
    let Some(catalogs) = catalogs else {
        return;
    };
    match benilla_formats::load_spell_visual_catalog(&mut catalogs.chain.lock_recover()) {
        Ok(table) => {
            if let Some((id, path)) = table.loot_art_effect() {
                info!("loot vfx: HARDCODED Loot Art {id} {path}, attach 19, native scale/loop");
                let level_up =
                    table
                        .hardcoded_effect("HARDCODED Unit Level Up")
                        .map(|(id, path)| {
                            info!("vfx: HARDCODED Unit Level Up {id} {path}, one-shot on the unit");
                            assets.load(m2_url(path))
                        });
                commands.insert_resource(Art {
                    model: assets.load(m2_url(path)),
                    level_up,
                });
            }
        }
        Err(e) => warn!("loot vfx unavailable: {e}"),
    };
}

pub(crate) fn effect(
    commands: &mut Commands,
    name: &'static str,
    owner: Entity,
    model: Handle<M2Model>,
    attach: Option<u16>,
    once: bool,
) -> Entity {
    commands
        .spawn((
            Name::new(name),
            Transform::default(),
            Visibility::default(),
            ChildOf(owner),
            Sparkle {
                owner,
                model,
                attached: attach.is_none(),
                built: false,
                attach,
                once,
                visuals_until: None,
                die_at: None,
            },
        ))
        .id()
}

pub(crate) fn spawn(commands: &mut Commands, owner: Entity, art: &Art) -> Entity {
    effect(
        commands,
        "WoW loot sparkles",
        owner,
        art.model.clone(),
        Some(19),
        false,
    )
}

/// The same sparkles over an opened chest that still holds something: the model has no attachment
/// point to ride (the corpse's is attachment 19), so the effect sits `lift` above the chest's origin,
/// in the chest's own frame, and rises through the open lid.
pub(crate) fn spawn_chest(commands: &mut Commands, owner: Entity, art: &Art, lift: f32) -> Entity {
    let fx = effect(
        commands,
        "WoW chest loot sparkles",
        owner,
        art.model.clone(),
        None,
        false,
    );
    commands
        .entity(fx)
        .insert(Transform::from_translation(Vec3::Y * lift));
    fx
}

/// The level-up flash on `owner`'s feet, `None` when the client has no such effect.
pub(crate) fn spawn_level_up(commands: &mut Commands, owner: Entity, art: &Art) -> Option<Entity> {
    let model = art.level_up.clone()?;
    Some(effect(
        commands,
        "WoW level-up flash",
        owner,
        model,
        None,
        true,
    ))
}

/// A one-shot flash goes a second after its clip ended.
fn reap(
    mut commands: Commands,
    real: Res<Time<bevy::time::Real>>,
    effects: Query<(Entity, &Sparkle)>,
) {
    for (e, fx) in &effects {
        if fx.die_at.is_some_and(|t| real.elapsed_secs() >= t) {
            commands.entity(e).despawn();
        }
    }
}

/// A one-shot's geometry goes with its sequence: the client's instance ends after one pass, and a
/// material-alpha loop would otherwise start its flash again.
fn settle(
    mut commands: Commands,
    real: Res<Time<bevy::time::Real>>,
    effects: Query<&Sparkle>,
    visuals: Query<(Entity, &FxVisual)>,
) {
    for (e, FxVisual(root)) in &visuals {
        let over = effects
            .get(*root)
            .is_ok_and(|fx| fx.visuals_until.is_some_and(|t| real.elapsed_secs() >= t));
        if over {
            commands.entity(e).despawn();
        }
    }
}

/// Re-samples every live instance's colour loop on its own clock.
fn tick_tints(
    time: Res<Time>,
    mut tints: ResMut<FxTints>,
    mut materials: ResMut<Assets<WowModelMaterial>>,
) {
    if tints.0.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    tints.0.retain(|id, (anim, origin)| {
        let Some(mat) = materials.get_mut(*id) else {
            return false; // the instance went, and its clone with it
        };
        let rgb = anim.sample(now - *origin);
        mat.extension.tint = Vec4::new(rgb[0], rgb[1], rgb[2], 1.0);
        true
    });
}

/// One batch of the model, ready to spawn.
struct Part {
    mesh: Handle<Mesh>,
    skinned: Option<Handle<Mesh>>,
    /// The instance's material: shared, or its own clone when its colour or texture moves.
    material: Handle<WowModelMaterial>,
    /// The shared material: where a ground decal reads its texture and fog.
    shared: Handle<WowModelMaterial>,
    blend: ModelBlend,
    additive: bool,
    geometry: Arc<benilla_formats::RenderSubmesh>,
    aabb: Option<bevy::camera::primitives::Aabb>,
    billboard: Option<benilla_assets::BillboardInfo>,
    alpha_anim: Option<Arc<AlphaAnim>>,
    rgb_anim: Option<Arc<RgbAnim>>,
    ground_quad: Option<GroundQuad>,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build(
    mut commands: Commands,
    real: Res<Time<bevy::time::Real>>,
    time: Res<Time>,
    models: Res<Assets<M2Model>>,
    ibps: Res<Assets<SkinnedMeshInverseBindposes>>,
    mut forms: ResMut<ModelForms>,
    mut mats: M2BatchMaterials,
    mut uv_reg: ResMut<UvAnimMaterials>,
    mut anim_table: ResMut<MatAnimTable>,
    mut palettes: ResMut<RigPalettes>,
    mut tints: ResMut<FxTints>,
    mut effects: Query<(Entity, &mut Sparkle, &mut Transform)>,
    mut owners: Query<(&ActorModel, Option<&mut RigPose>), Without<Sparkle>>,
) {
    for (root, mut fx, mut xf) in &mut effects {
        if !fx.attached {
            if let Ok((body, body_pose)) = owners.get_mut(fx.owner) {
                if let Some(model) = models.get(&body.m2) {
                    let wanted = fx.attach;
                    if let Some((point, mut pose)) = model
                        .attachments
                        .iter()
                        .find(|p| Some(p.id) == wanted)
                        .copied()
                        .zip(body_pose)
                    {
                        if let Some(joint) = pose.anchor_for(&mut commands, fx.owner, point.bone) {
                            xf.translation = point.offset;
                            commands.entity(joint).add_child(root);
                        }
                    }
                    fx.attached = true;
                }
            }
        }
        if fx.built {
            continue;
        }
        let Some(model) = models.get(&fx.model) else {
            continue; // loading
        };
        if !forms.require_rigged(&fx.model, 0) || !mats.ready() {
            continue; // render forms or the light buffer still coming
        }
        fx.built = true;
        let now = time.elapsed_secs();

        // The rig: the model's joints under the root, one player over the first sequence.
        let skeleton = &model.skeleton;
        let anims = model.animations.as_ref();
        let played = anims.and_then(|a| a.preferred_clip(Some(0)));
        let played_seq = played.map(|c| c.seq_index);
        let mut joints: Vec<Entity> = Vec::new();
        if !skeleton.joints.is_empty() {
            joints = spawn_joints(&mut commands, root, root, skeleton);
            if let Some(bb) = BillboardJointRig::new(skeleton, &joints, root) {
                commands.entity(root).insert(bb);
            }
            if let Some(anims) = anims {
                if let Some(clip) = played {
                    let mut player = AnimationPlayer::default();
                    player.start(clip.node).set_repeat(if fx.once {
                        RepeatAnimation::Never
                    } else {
                        RepeatAnimation::Forever
                    });
                    commands.entity(root).insert((
                        player,
                        AnimationGraphHandle(anims.graph.clone()),
                        anims.clone(),
                    ));
                    for (i, &j) in joints.iter().enumerate() {
                        commands
                            .entity(j)
                            .insert((bone_target_id(i as u16), AnimatedBy(root)));
                    }
                }
                if let Some(drive) = GlobalSeqDrive::new(&anims.global_bones, &joints) {
                    commands.entity(root).insert(drive);
                }
            }
        }
        let span = played.map_or_else(|| model.first_seq_span.unwrap_or(1.0), |clip| clip.duration);
        if fx.once {
            fx.visuals_until = Some(real.elapsed_secs() + span);
            fx.die_at = Some(real.elapsed_secs() + span + 1.0);
        }
        let rig_slot = if joints.is_empty() {
            0
        } else {
            RigSkin::allocate(
                &mut palettes,
                joints.clone(),
                model.inverse_bindposes.clone(),
            )
            .map_or(0, |rig| {
                let slot = rig.slot;
                commands.entity(root).insert(rig);
                slot
            })
        };
        let rigged = rig_slot != 0;

        // The parts: each batch's render forms and materials.
        let built = forms.slices(&fx.model);
        let (stat_forms, skin_forms) = (built.stat, built.skin.unwrap_or(&[]));
        let mut parts: Vec<Part> = Vec::with_capacity(model.submeshes.len());
        for (pi, sub) in model.submeshes.iter().enumerate() {
            let order = u16::try_from(pi + 1).unwrap_or(u16::MAX);
            let mut lane = EntityUvLane {
                reg: &mut uv_reg,
                table: &mut anim_table,
                instance: None,
            };
            let Some(v) = mats.entity_variants(sub, sub.texture.clone(), order, &mut lane) else {
                continue;
            };
            let shared = v.steady;
            // The colour or texture transform moves: the instance gets its own material, seeded
            // at its first key, so its phase starts at the cast.
            let loops = UvLoops::of(sub);
            let material = if sub.rgb_anim.is_none() && !loops.any() {
                shared.clone()
            } else {
                let store = mats.materials();
                benilla_world::model_render::lazy::realize(store, shared.id());
                match store.get(shared.id()).cloned() {
                    Some(mut mat) => {
                        if let Some(anim) = &sub.rgb_anim {
                            let t0 = anim.sample(0.0);
                            mat.extension.tint = Vec4::new(t0[0], t0[1], t0[2], 1.0);
                        }
                        // Off the shared mat-anim table, or its delta would add to the clone's own.
                        mat.extension.anim_slots = Vec4::ZERO;
                        if loops.any() {
                            let t0 = loops.open_offset();
                            mat.extension.sun_scale.z = t0[0];
                            mat.extension.sun_scale.w = t0[1];
                        }
                        let handle = store.add(mat);
                        if let Some(anim) = &sub.rgb_anim {
                            tints.0.insert(handle.id(), (anim.clone(), now));
                        }
                        if loops.any() {
                            register_fx_uv(
                                &mut uv_reg,
                                &mut anim_table,
                                store,
                                handle.id(),
                                loops,
                                root,
                                now,
                            );
                        }
                        handle
                    }
                    None => shared.clone(),
                }
            };
            parts.push(Part {
                mesh: stat_forms
                    .get(pi)
                    .map(|(h, _)| h.clone())
                    .unwrap_or_default(),
                skinned: skin_forms.get(pi).cloned(),
                material,
                shared,
                blend: sub.blend,
                additive: sub.additive,
                geometry: sub.geometry.clone(),
                aabb: stat_forms.get(pi).and_then(|(_, a)| *a),
                billboard: sub.billboard.clone(),
                alpha_anim: sub.alpha_anim.clone(),
                rgb_anim: sub.rgb_anim.clone(),
                ground_quad: sub.ground_quad,
            });
        }

        let mut drawn = 0;
        // Ordinary batches: children of the root, skinned by the instance's slot.
        commands.entity(root).with_children(|children| {
            for part in &parts {
                if part.billboard.is_some() || part.ground_quad.is_some() {
                    continue; // a card below, or a decal
                }
                let mesh = match (rigged, &part.skinned) {
                    (true, Some(sm)) => sm.clone(),
                    _ => part.mesh.clone(),
                };
                let mut child = children.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(part.material.clone()),
                    Transform::default(),
                    NoFrustumCulling,
                    ModelPart {
                        kind: ModelKind::Creature,
                        blend: part.blend,
                    },
                    benilla_world::interact::PickMesh(part.geometry.clone()),
                    FxVisual(root),
                ));
                let skinned = rigged && part.skinned.is_some();
                if skinned {
                    child.insert((
                        RigPart(root),
                        MeshTag(benilla_world::mesh_tag::spawn_tag(rig_slot, 1.0)),
                    ));
                }
                // The alpha loops on this instance's clock; the sampler is the tag's only writer.
                if let Some(anim) = &part.alpha_anim {
                    let mat_anim = MatAnim::driving_tag(anim.clone(), now, played_seq);
                    child.insert((
                        MeshTag(benilla_world::mesh_tag::spawn_tag(
                            if skinned { rig_slot } else { 0 },
                            mat_anim.current,
                        )),
                        mat_anim,
                    ));
                }
                drawn += 1;
            }
        });
        // Billboard batches: cards that follow their joint (or the root) and face the camera.
        for part in &parts {
            let Some(info) = &part.billboard else {
                continue;
            };
            let card = match joints.get(info.bone as usize) {
                Some(&j) => BillboardCard::following_joint(info, j),
                None => BillboardCard::following(info, root),
            };
            let mut spawned = commands.spawn((
                Mesh3d(part.mesh.clone()),
                MeshMaterial3d(part.material.clone()),
                Transform::default(),
                ModelPart {
                    kind: ModelKind::Creature,
                    blend: part.blend,
                },
                benilla_world::interact::PickMesh(part.geometry.clone()),
                card,
                FxVisual(root),
            ));
            if let Some(aabb) = part.aabb {
                spawned.insert(aabb);
            }
            if let Some(anim) = &part.alpha_anim {
                let mat_anim = MatAnim::driving_tag(anim.clone(), now, played_seq);
                spawned.insert((
                    MeshTag(benilla_world::mesh_tag::spawn_tag(0, mat_anim.current)),
                    mat_anim,
                ));
            }
            drawn += 1;
        }
        // Flat ground quads as projected decals on their joints: texture, blend, fog policy and
        // colour loop ride the record, the alpha loop the decal's own `MatAnim`.
        let binds = ibps.get(&model.inverse_bindposes);
        for part in &parts {
            let Some(quad) = part.ground_quad else {
                continue;
            };
            let (joint, ibp) = match joints.get(quad.bone as usize) {
                Some(&j) => (
                    j,
                    binds
                        .and_then(|b| b.get(quad.bone as usize))
                        .copied()
                        .unwrap_or(Mat4::IDENTITY),
                ),
                None => (root, Mat4::IDENTITY), // boneless model: the quad rides the root
            };
            let store = mats.materials();
            benilla_world::model_render::lazy::realize(store, part.shared.id());
            let (texture, fog_bits) = match store.get(part.shared.id()) {
                Some(mat) => (
                    mat.base.base_color_texture.clone(),
                    (mat.extension.clutter_fade.z as u32 >> 4) & 7,
                ),
                None => (None, u32::from(benilla_formats::FogPolicy::Scene as u8)),
            };
            let Some(texture) = texture else {
                continue; // no texture, nothing to draw
            };
            let decal = benilla_world::ground_fx::spawn_ground_fx_decal(
                &mut commands,
                texture,
                part.blend,
                part.additive,
                fog_bits,
                part.rgb_anim.as_ref().map(|a| (a.clone(), now)),
                &quad,
                joint,
                ibp,
            );
            commands.entity(decal).insert(FxVisual(root));
            if let Some(anim) = &part.alpha_anim {
                commands
                    .entity(decal)
                    .insert(MatAnim::driving_tag(anim.clone(), now, played_seq));
            }
            drawn += 1;
        }

        // The emitters and ribbons ride the joints (the root where the model has no bone).
        let mut count = 0;
        for em in &model.emitters {
            let owner = joints
                .get(em.def.bone as usize)
                .map_or((root, [0.0; 3]), |&j| (j, em.bone_pivot));
            if let Some(emitter) = spawn_emitter(
                &mut commands,
                em,
                Transform::IDENTITY,
                EmitterFrames {
                    owner: Some(owner),
                    anchor: Some(root),
                    alpha: None,
                    light_node: None,
                    on_owner_loss: OwnerLoss::Free,
                },
                EmitClock::Host(root),
            ) {
                commands.entity(emitter).insert(LootEmitter);
                count += 1;
            }
        }
        let mut ribbons = 0;
        for rb in &model.ribbons {
            let (owner, use_pivot) = joints
                .get(rb.def.bone as usize)
                .map_or((root, false), |&j| (j, true));
            if spawn_ribbon(
                &mut commands,
                rb,
                owner,
                use_pivot,
                1.0,
                RibbonSeq::Host(root),
                Some(root),
                None,
            )
            .is_some()
            {
                ribbons += 1;
            }
        }
        info!(
            "loot vfx: built {drawn} drawn parts, {count} native emitters, {ribbons} ribbons, loop {span:.3} s"
        );
    }
}

fn trace(
    time: Res<Time<bevy::time::Real>>,
    images: Res<Assets<Image>>,
    effects: Query<(), With<Sparkle>>,
    emitters: Query<&ParticleEmitter, With<LootEmitter>>,
    mut memo: Local<(f32, bool)>,
) {
    if std::env::var("WOWSOULS_TRACE").as_deref() != Ok("1") {
        return;
    }
    let active = !effects.is_empty() || !emitters.is_empty();
    if time.elapsed_secs() < memo.0 || (!active && !memo.1) {
        return;
    }
    memo.0 = time.elapsed_secs() + 0.4;
    memo.1 = active;
    info!(
        "loot vfx trace: t={:.2} roots={} emitters={} live={} drawn={} resident={} clouds={:?}",
        time.elapsed_secs(),
        effects.iter().count(),
        emitters.iter().count(),
        emitters.iter().map(ParticleEmitter::live).sum::<usize>(),
        emitters.iter().filter(|e| e.drawn()).count(),
        emitters
            .iter()
            .filter(|e| images.contains(e.texture()))
            .count(),
        emitters
            .iter()
            .filter_map(ParticleEmitter::cloud_fingerprint)
            .map(|(c, _, _, r)| (c, r))
            .collect::<Vec<_>>()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sparkle(once: bool, visuals_until: Option<f32>) -> Sparkle {
        Sparkle {
            owner: Entity::PLACEHOLDER,
            model: Handle::default(),
            attached: true,
            built: true,
            attach: None,
            once,
            visuals_until,
            die_at: visuals_until.map(|t| t + 1.0),
        }
    }

    #[test]
    fn a_one_shots_geometry_goes_with_its_sequence_and_a_loops_stays() {
        let mut app = App::new();
        app.insert_resource(Time::<bevy::time::Real>::default())
            .add_systems(Update, settle);
        let flash = app.world_mut().spawn(sparkle(true, Some(1.0))).id();
        let looped = app.world_mut().spawn(sparkle(false, None)).id();
        let flash_part = app.world_mut().spawn(FxVisual(flash)).id();
        let loop_part = app.world_mut().spawn(FxVisual(looped)).id();
        app.update();
        assert!(app.world().get_entity(flash_part).is_ok(), "before the end");
        app.world_mut()
            .resource_mut::<Time<bevy::time::Real>>()
            .advance_by(Duration::from_secs_f32(1.5));
        app.update();
        assert!(
            app.world().get_entity(flash_part).is_err(),
            "the sequence ended"
        );
        assert!(
            app.world().get_entity(flash).is_ok(),
            "its root waits for the particles"
        );
        assert!(
            app.world().get_entity(loop_part).is_ok(),
            "a loop has no end"
        );
    }

    #[test]
    fn installed_loot_art_resolves_to_the_clients_actual_model() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let table = benilla_formats::load_spell_visual_catalog(&mut chain).unwrap();
        let (id, path) = table.loot_art_effect().unwrap();
        assert_eq!(id, 14);
        assert_eq!(benilla_assets::m2_url(path), "mpq://particles/lootfx.m2");
        assert!(chain.read_file("Particles\\LootFX.m2").is_ok());
    }
}
