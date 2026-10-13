//! Actors: animated M2 bodies in the world, the player's and the creatures'. An [`ActorModel`] on
//! an entity names a display; [`build`] turns it into a skinned, posed, animated model under that
//! entity once the M2 and its render forms are ready.
//!
//! The pipeline is benilla's unit attach (benilla @ 2e82d34, `crates/benilla-app/src/entities/`:
//! `display::build_parts`, `attach::{setup_skinned_instance, arm_sequence_clock}`,
//! `attach::char_skin`, `attach::dress::spawn_part`), cut to what a single-player body needs: no
//! mount, corpse, appear fade, emitter or pick lane yet (a billboard batch is a card that follows the
//! actor, [`benilla_world::billboard::BillboardCard`]). A body's gear is a [`Dress`]: the geosets, the
//! composited armor regions, the cloak and the helm and shoulder models come from it, and a changed
//! `Dress` dresses the built body again in place. Everything it
//! calls is the engine's public API: the batch materials, the render forms, the skin palette, the
//! rig pose and the interior classifier.

use std::sync::{Arc, Mutex};

use bevy::animation::transition::AnimationTransitions;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::MeshTag;
use bevy::prelude::*;

use benilla_assets::coords::wow_to_bevy;
use benilla_assets::{m2_url, skin_url, AssetSet, LockRecover, M2Model, WorldAssets};
use benilla_formats::{
    Chain, CharSections, CharSkinSlot, CharacterGeosets, CreatureCatalog, ModelBlend,
};
use benilla_world::model_render::{EntityUvLane, M2BatchMaterials, ModelKind, ModelPart};

pub mod anim;
pub mod held;
mod skin;

use crate::motor::CAPSULE_HEIGHT;

/// The client tables the actors read, loaded once from the patch chain.
#[derive(Resource)]
pub struct Catalogs {
    pub creatures: CreatureCatalog,
    pub geosets: CharacterGeosets,
    pub sections: CharSections,
    pub items: benilla_formats::ItemDisplayCatalog,
    pub chain: Arc<Mutex<Chain>>,
}

/// A character body's appearance (the CharSections and geoset keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharLook {
    pub race: u8,
    pub sex: u8,
    pub skin: u8,
    pub face: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub facial_hair: u8,
}

/// How an actor's model is skinned.
#[derive(Debug, Clone)]
pub enum Look {
    /// A `Character\` model: geosets picked and the body composited from the look.
    Character(CharLook),
    /// A creature model: its display's `Monster1/2/3` skin variations from the model's folder.
    Creature { skins: [Option<String>; 3] },
}

/// What a character-model NPC wears, from its `CreatureDisplayInfoExtra` row: the baked body
/// atlas (its gear is painted on it) and the worn `ItemDisplayInfo` ids by slot: head, shoulder,
/// shirt, chest, belt, pants, boots, wrist, gloves, tabard.
#[derive(Debug, Clone)]
pub struct NpcWorn {
    pub bake_name: Option<String>,
    pub equipment: [u32; 10],
}

/// What a character body wears, as `ItemDisplayInfo` ids (0 = nothing): the player's gear, read the
/// way a character-model NPC's [`NpcWorn`] is. Changing it dresses the built body again.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Dress {
    /// Head, shoulder, shirt, chest, belt, pants, boots, wrist, gloves, tabard (the order of
    /// [`NpcWorn::equipment`]).
    pub worn: [u32; 10],
    /// The back slot's cloak.
    pub cloak: u32,
}

/// The part entities [`build`] spawned for an actor's current [`Dress`], taken down when it changes.
#[derive(Component, Debug, Default)]
pub struct DressParts(pub Vec<Entity>);

/// The model an actor wants; [`build`] dresses the entity and marks it [`ActorBuilt`].
#[derive(Component, Debug, Clone)]
pub struct ActorModel {
    pub m2: Handle<M2Model>,
    /// The model's folder, where creature skins live.
    pub dir: String,
    pub look: Look,
    /// The display's model scale.
    pub scale: f32,
    /// The creature display this body was built from (its blood, its voice).
    pub display: u32,
    /// A character-model NPC's gear (a display with a `CreatureDisplayInfoExtra` row and no look
    /// of its own given).
    pub worn: Option<NpcWorn>,
}

impl ActorModel {
    /// A creature display's model, looked up in `CreatureDisplayInfo`; a character body takes
    /// `char_look`, a creature its display's skins.
    pub fn for_display(
        catalog: &CreatureCatalog,
        display_id: u32,
        char_look: Option<CharLook>,
        asset_server: &AssetServer,
    ) -> Option<Self> {
        let m = catalog.model(display_id)?;
        let dir = m
            .model_path
            .rsplit_once(['\\', '/'])
            .map_or("", |(d, _)| d)
            .to_string();
        let (look, worn) = match (char_look, &m.npc_appearance) {
            (Some(l), _) => (Look::Character(l), None),
            (None, Some(npc)) => (
                Look::Character(CharLook {
                    race: npc.race,
                    sex: npc.sex,
                    skin: npc.skin,
                    face: npc.face,
                    hair_style: npc.hair_style,
                    hair_color: npc.hair_color,
                    facial_hair: npc.facial_hair,
                }),
                Some(NpcWorn {
                    bake_name: npc.bake_name.clone(),
                    equipment: npc.equipment,
                }),
            ),
            (None, None) => (
                Look::Creature {
                    skins: m.textures.clone(),
                },
                None,
            ),
        };
        Some(Self {
            m2: asset_server.load(m2_url(&m.model_path)),
            dir,
            look,
            scale: if m.scale > 0.0 { m.scale } else { 1.0 },
            display: display_id,
            worn,
        })
    }
}

/// The `mpq://` URL of a baked NPC atlas under `Textures\BakedNpcTextures\`.
fn baked_npc_url(bake_name: &str) -> String {
    format!(
        "mpq://textures/bakednpctextures/{}",
        bake_name.replace('\\', "/").to_ascii_lowercase()
    )
}

/// The geoset selectors a character NPC's gear sets (benilla `equip_geosets`, branches B1-B8 of
/// `0x477520`): the body slots shirt to tabard by their rows' geoset groups, whether the forearm is
/// dressed, and the helm's hide mask for hair and ears.
fn equip_geosets(
    items: &benilla_formats::ItemDisplayCatalog,
    equipment: &[u32; 10],
) -> benilla_formats::EquipGeosets {
    let mut eg = benilla_formats::EquipGeosets::default();
    let mut worn: [Option<&benilla_formats::ItemDisplay>; 8] = [None; 8];
    for (i, slot) in worn.iter_mut().enumerate() {
        let id = equipment[i + 2];
        if id != 0 {
            *slot = items.get(id);
            eg.bodyslots[i] = slot.map(|row| row.geoset_groups);
        }
    }
    eg.forearm_dressed = benilla_formats::forearm_dressed(&worn);
    if equipment[0] != 0 {
        eg.helm_vis = items
            .get(equipment[0])
            .and_then(benilla_formats::ItemDisplay::worn_helm_vis);
    }
    eg
}

/// The displays whose region textures the body atlas paints: shirt to tabard, the engine's order.
fn body_displays<'a>(
    items: &'a benilla_formats::ItemDisplayCatalog,
    equipment: &[u32; 10],
) -> [Option<&'a benilla_formats::ItemDisplay>; 8] {
    let mut out = [None; 8];
    for (i, slot) in out.iter_mut().enumerate() {
        let id = equipment[i + 2];
        if id != 0 {
            *slot = items.get(id);
        }
    }
    out
}

/// The model is on: parts spawned, rig and clock armed.
#[derive(Component)]
pub struct ActorBuilt;

/// The model's frame: a child of the body that carries the rig (its `joints_root`), the parts and
/// the hand anchors. Identity unless an action turns the whole model under the body, as the roll's
/// somersault does; the body's own `Transform` (the motor's feet and facing) never tumbles.
#[derive(Component, Debug, Clone, Copy)]
pub struct ModelFrame(pub Entity);

pub fn plugin(app: &mut App) {
    app.add_plugins(held::plugin)
        .add_systems(Startup, load_catalogs.after(AssetSet::Open))
        .add_systems(Update, build)
        // Motors only step in play: under the loading hold a body reads airborne.
        .add_systems(
            PostUpdate,
            anim::drive
                .before(bevy::app::AnimationSystems)
                .run_if(in_state(crate::settle::Phase::Playing)),
        );
}

fn load_catalogs(mut commands: Commands, world: Option<Res<WorldAssets>>) {
    let Some(world) = world else {
        warn!("actors: no client data, bodies stay capsules");
        return;
    };
    let mut chain = world.chain.lock_recover();
    let loaded = (|| -> anyhow::Result<Catalogs> {
        Ok(Catalogs {
            creatures: benilla_formats::load_creature_catalog(&mut chain)?,
            geosets: CharacterGeosets::load(&mut chain)?,
            sections: CharSections::load(&mut chain)?,
            items: benilla_formats::load_item_display_catalog(&mut chain)?,
            chain: world.chain.clone(),
        })
    })();
    match loaded {
        Ok(c) => {
            info!("actors: {} creature displays", c.creatures.len());
            commands.insert_resource(c);
        }
        Err(e) => warn!("actors: catalogs unavailable, bodies stay capsules: {e:#}"),
    }
}

/// Builds each pending actor once its model and render forms are in.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build(
    mut commands: Commands,
    pending: Query<
        (
            Entity,
            &ActorModel,
            Option<&crate::cloak::Cloak>,
            Option<&Dress>,
            Option<&ModelFrame>,
            Option<&benilla_world::rig_palette::RigSkin>,
            Option<&DressParts>,
        ),
        Or<(Without<ActorBuilt>, Changed<Dress>)>,
    >,
    worn_roots: Query<(Entity, &held::WornRoot)>,
    m2s: Res<Assets<M2Model>>,
    mut forms: ResMut<benilla_world::model_forms::ModelForms>,
    mut mats: M2BatchMaterials,
    mut uv_reg: ResMut<benilla_world::doodad_anim::UvAnimMaterials>,
    mut anim_table: ResMut<benilla_world::mat_anim_table::MatAnimTable>,
    mut palettes: ResMut<benilla_world::rig_palette::RigPalettes>,
    catalogs: Option<Res<Catalogs>>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut transforms: Query<&mut Transform>,
    time: Res<Time>,
) {
    for (root, actor, cloak, dress, old_frame, old_rig, old_parts) in &pending {
        // A changed `Dress` on a built body: the rig stays, the parts and worn models are redone.
        let rebuilding = old_frame.is_some();
        let Some(model) = m2s.get(&actor.m2) else {
            continue; // loading
        };
        if !forms.require_rigged(&actor.m2, 0) || !mats.ready() {
            continue; // render forms or the light buffer still coming
        }
        let (Some(anims), Some(ibp)) = (model.animations.as_ref(), Some(&model.inverse_bindposes))
        else {
            warn!("actor {root}: model has no animations; left as is");
            commands.entity(root).insert(ActorBuilt);
            continue;
        };

        // What the body wears: a player's `Dress`, else a character NPC's own equipment rows.
        let worn_ids: Option<[u32; 10]> = dress
            .map(|d| d.worn)
            .or_else(|| actor.worn.as_ref().map(|w| w.equipment));
        let cloak_id: Option<u32> = cloak
            .map(|c| c.0)
            .or_else(|| dress.map(|d| d.cloak).filter(|&id| id != 0));

        // The character look: the shown geosets, the body atlas and the hair texture.
        let (visible, body_tex, hair_tex) = match (&actor.look, catalogs.as_deref()) {
            (Look::Character(l), Some(c)) => {
                let mut equip = worn_ids
                    .map_or_else(benilla_formats::EquipGeosets::default, |ids| {
                        equip_geosets(&c.items, &ids)
                    });
                // A cloak picks the cape geoset (group 15, style 1502-1506).
                if let Some(id) = cloak_id {
                    equip.cloak = c.items.get(id).map(|row| row.geoset_groups[0]);
                }
                let visible =
                    c.geosets
                        .visible_geosets(l.race, l.sex, l.hair_style, l.facial_hair, &equip);
                // An NPC's shipped atlas already carries its gear and is loaded as is.
                let baked = actor
                    .worn
                    .as_ref()
                    .and_then(|w| w.bake_name.as_deref())
                    .map(|n| asset_server.load::<Image>(baked_npc_url(n)));
                // A player's gear is painted on the atlas; an NPC's shipped atlas has its own.
                let body = baked.or_else(|| {
                    let regions = dress.map_or([None; 8], |d| body_displays(&c.items, &d.worn));
                    skin::body_atlas(l, &c.sections, &c.chain, &mut images, regions)
                });
                let hair = c
                    .sections
                    .hair_mesh_texture(l.race, l.sex, l.hair_style, l.hair_color)
                    .map(|p| {
                        asset_server.load::<Image>(format!(
                            "mpq://{}",
                            p.replace('\\', "/").to_ascii_lowercase()
                        ))
                    });
                if body.is_none() {
                    warn!("actor {root}: no body skin for {l:?}");
                }
                (Some(visible), body, hair)
            }
            _ => (None, None, None),
        };

        // The rig: a palette slot for the joint matrices the shader skins with, the pose buffer,
        // and the free-running global sequences (an eyelid's blink).
        // A body dressed again keeps its rig, frame and clock: only the parts below are redone.
        let nbones = model.skeleton.joints.len();
        let (slot, frame, pose) = if let Some(old) = old_frame {
            (old_rig.map_or(0, |r| r.slot), old.0, None)
        } else {
            let slot = match benilla_world::rig_palette::RigSkin::allocate_bones(
                &mut palettes,
                nbones as u32,
                ibp.clone(),
            ) {
                Some(rig) => {
                    let slot = rig.slot;
                    commands.entity(root).insert(rig);
                    slot
                }
                None => 0, // table full: bind-pose parts
            };
            let frame = commands
                .spawn((
                    Name::new("Model frame"),
                    Transform::default(),
                    Visibility::default(),
                    ChildOf(root),
                ))
                .id();
            let pose = benilla_world::rig_anim::RigPose::new(frame, &model.skeleton);
            if let Some(drive) =
                benilla_world::rig_anim::GlobalSeqDrive::new_rig(&anims.global_bones, nbones)
            {
                commands.entity(root).insert(drive);
            }
            (slot, frame, Some(pose))
        };

        let bake_center = model.bounds.map_or(Vec3::ZERO, |b| {
            wow_to_bevy([
                (b.bbox_min[0] + b.bbox_max[0]) * 0.5,
                (b.bbox_min[1] + b.bbox_max[1]) * 0.5,
                (b.bbox_min[2] + b.bbox_max[2]) * 0.5,
            ])
        });
        let idle_aabb = anims
            .idle_clip()
            .filter(|c| c.bounds_max.cmpgt(c.bounds_min).all())
            .map(|c| Aabb::from_min_max(c.bounds_min, c.bounds_max));

        // The cloak's cape texture: `ModelTextureLeft` of its display, under `Item/ObjectComponents/Cape/`.
        let cape_tex = cloak_id
            .and_then(|id| catalogs.as_deref()?.items.get(id)?.model_texture[0].as_deref())
            .map(|name| {
                asset_server.load::<Image>(format!(
                    "mpq://item/objectcomponents/cape/{}.blp",
                    name.to_ascii_lowercase()
                ))
            });

        // The parts.
        let built = forms.slices(&actor.m2);
        let (stat_forms, skin_forms) = (built.stat, built.skin.unwrap_or(&[]));
        let (mut shown, mut cards) = (0, 0);
        let mut spawned: Vec<Entity> = Vec::new();
        for (pi, sub) in model.submeshes.iter().enumerate() {
            if visible
                .as_ref()
                .is_some_and(|v| !v.contains(&sub.geoset_id))
            {
                continue;
            }
            let order = u16::try_from(pi + 1).unwrap_or(u16::MAX);
            let variants = match sub.char_slot {
                Some(CharSkinSlot::Body) if body_tex.is_some() => mats.char_variants(
                    body_tex.clone().expect("checked"),
                    ModelBlend::Opaque,
                    sub.two_sided,
                ),
                Some(CharSkinSlot::Hair) if hair_tex.is_some() => {
                    mats.char_variants(hair_tex.clone().expect("checked"), sub.blend, sub.two_sided)
                }
                // The cape batches (texture type 2) wear the cloak's texture.
                Some(CharSkinSlot::Object) if cape_tex.is_some() => {
                    mats.char_variants(cape_tex.clone().expect("checked"), sub.blend, sub.two_sided)
                }
                _ => {
                    let texture = resolve_texture(sub, &actor.dir, &actor.look, &asset_server);
                    let mut lane = EntityUvLane {
                        reg: &mut uv_reg,
                        table: &mut anim_table,
                        instance: None,
                    };
                    mats.entity_variants(sub, texture, order, &mut lane)
                }
            };
            let Some(v) = variants else { continue };
            // A billboard batch (the `?` quest marker, glow cards) is a world-root card that follows
            // the actor and turns to the camera every frame (`BillboardPlace`), as benilla's quest
            // markers (`quest_markers`) and unit parts (`dress::spawn_billboard_part`) place it.
            if let Some(info) = &sub.billboard {
                let mesh = stat_forms
                    .get(pi)
                    .map(|(h, _)| h.clone())
                    .unwrap_or_default();
                // A marker's own bob (anim 0) runs on the card's clock, armed now.
                let bob = info
                    .seq_translations
                    .iter()
                    .find(|(id, _)| *id == 0)
                    .map(|(_, l)| l.clone());
                let mut card = commands.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(v.steady.clone()),
                    Transform::default(),
                    ModelPart {
                        kind: ModelKind::Creature,
                        blend: sub.blend,
                    },
                    MeshTag(benilla_world::mesh_tag::spawn_tag(0, 1.0)),
                    benilla_world::billboard::BillboardCard::following(info, root)
                        .with_seq_translation(bob, time.elapsed().as_millis() as u32),
                ));
                if let Some(aabb) = stat_forms.get(pi).and_then(|(_, a)| *a) {
                    card.insert(aabb);
                }
                spawned.push(card.id());
                cards += 1;
                continue;
            }
            // The cloak's main cape mesh is cloth: its own mesh, simulated every frame in world
            // space and drawn through this part's transform (no skin), see `crate::cloak`.
            if cloak_id.is_some()
                && sub.char_slot == Some(CharSkinSlot::Object)
                && sub.geometry.positions.len() >= crate::cloak::MIN_CLOTH_VERTS
                && crate::cloak::cloth_enabled()
                && slot != 0
            {
                let topo = std::sync::Arc::new(crate::cloak::Topology::build(
                    &sub.geometry.positions,
                    &sub.geometry.uvs,
                    &sub.geometry.indices,
                    crate::cloak::LEVELS,
                ));
                let mesh = meshes.add(topo.mesh(&sub.geometry.vertex_colors));
                let mut part = commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(v.steady.clone()),
                    Transform::default(),
                    ChildOf(frame),
                    ModelPart {
                        kind: ModelKind::Creature,
                        blend: sub.blend,
                    },
                    // Unskinned: the cloth is already in world space.
                    MeshTag(benilla_world::mesh_tag::spawn_tag(0, 1.0)),
                    crate::cloak::ClothPart::new(root, mesh, topo, sub.geometry.clone()),
                    NoFrustumCulling,
                ));
                if let Some(lit) = benilla_world::interior::part_interior_lit(
                    &v.steady,
                    Some(&v.interior),
                    Some(&v.interior_bake),
                    bake_center,
                    root,
                ) {
                    part.insert(lit);
                }
                spawned.push(part.id());
                shown += 1;
                continue;
            }
            let skinned = slot != 0 && skin_forms.get(pi).is_some();
            let mesh = if skinned {
                skin_forms[pi].clone()
            } else {
                stat_forms
                    .get(pi)
                    .map(|(h, _)| h.clone())
                    .unwrap_or_default()
            };
            let mut part = commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(v.steady.clone()),
                Transform::default(),
                ChildOf(frame),
                ModelPart {
                    kind: ModelKind::Creature,
                    blend: sub.blend,
                },
                // The instance's palette slot and full alpha.
                MeshTag(benilla_world::mesh_tag::spawn_tag(slot, 1.0)),
                // The decoded geometry, which the character shadow lane skins into the sun's map.
                benilla_world::interact::PickMesh(sub.geometry.clone()),
            ));
            if skinned {
                part.insert(benilla_world::rig_palette::RigPart(root));
                // A skinned part moves past its bind box: the root owns the cull.
                if let Some(aabb) = idle_aabb.or(stat_forms.get(pi).and_then(|(_, a)| *a)) {
                    part.insert((aabb, NoFrustumCulling));
                }
            } else if let Some(aabb) = stat_forms.get(pi).and_then(|(_, a)| *a) {
                part.insert(aabb);
            }
            // Indoors the body is lit by the room's baked light, one verdict for the whole body.
            if let Some(lit) = benilla_world::interior::part_interior_lit(
                &v.steady,
                Some(&v.interior),
                Some(&v.interior_bake),
                bake_center,
                root,
            ) {
                part.insert(lit);
            }
            spawned.push(part.id());
            shown += 1;
        }

        // The helm and the shoulder pads hang from the body's attachment points.
        if let (Some(ids), Look::Character(l)) = (worn_ids, &actor.look) {
            let mut items = Vec::new();
            if ids[0] != 0 {
                items.push(held::Worn::new(
                    ids[0],
                    held::WornKind::Helm {
                        race: l.race,
                        sex: l.sex,
                    },
                ));
            }
            if ids[1] != 0 {
                items.push(held::Worn::new(ids[1], held::WornKind::ShoulderLeft));
                items.push(held::Worn::new(ids[1], held::WornKind::ShoulderRight));
            }
            commands.entity(root).insert(held::WornItems(items));
        }

        // Dressed again: the old parts and worn models go, the new ones are already queued.
        if rebuilding {
            for old in old_parts.into_iter().flat_map(|p| p.0.iter()) {
                commands.entity(*old).try_despawn();
            }
            for (entity, owner) in &worn_roots {
                if owner.0 == root {
                    commands.entity(entity).try_despawn();
                }
            }
            commands.entity(root).insert(DressParts(spawned));
            info!("actor {root}: dressed again, {shown} parts ({cards} billboard cards)");
            continue;
        }

        // The clock: Bevy's player and transitions over the model's graph; the engine's pose
        // evaluator reads its weights.
        commands.entity(root).insert((
            AnimationPlayer::default(),
            AnimationGraphHandle(anims.graph.clone()),
            anims.clone(),
            AnimationTransitions::new(),
            anim::Locomotion::default(),
            benilla_world::world_unit::WorldUnit {
                wades: true,
                scale: actor.scale,
                height: CAPSULE_HEIGHT,
                bound: Some(idle_aabb.unwrap_or(Aabb::from_min_max(Vec3::ZERO, Vec3::ZERO))),
            },
            benilla_world::interior::BodyBakeCenter(bake_center),
            benilla_world::entity_shade::GroundShade::default(),
            ModelFrame(frame),
            DressParts(spawned),
            ActorBuilt,
        ));
        if let Some(pose) = pose {
            commands.entity(root).insert(pose);
        }
        if let Ok(mut xf) = transforms.get_mut(root) {
            xf.scale = Vec3::splat(actor.scale);
        }
        let ids: Vec<u16> = anims.clips.iter().map(|c| c.anim_id).collect();
        info!(
            "actor {root}: built {shown} parts ({cards} billboard cards), {nbones} bones, \
             palette slot {slot}, clips {ids:?}"
        );
    }
}

/// A batch's texture: its embedded one, else a creature skin variation (`<dir>\<name>.blp`).
fn resolve_texture(
    sub: &benilla_assets::ModelSubmesh,
    dir: &str,
    look: &Look,
    asset_server: &AssetServer,
) -> Option<Handle<Image>> {
    match (&sub.texture, sub.skin_slot, look) {
        (Some(t), _, _) => Some(t.clone()),
        (None, Some(slot), Look::Creature { skins }) => skins
            .get(slot as usize)
            .and_then(|o| o.as_ref())
            .map(|name| asset_server.load(skin_url(dir, name))),
        _ => None,
    }
}
