//! Held items: a weapon model on a body's hand attachment, and the hand closed around it.
//!
//! The item is an `ItemDisplayInfo` row's M2 under `Item\ObjectComponents\Weapon\`, skinned with
//! the row's texture on its object batches. It hangs from the wearer's attachment point
//! (`ModelAttachment` id 1, the right hand): an anchor entity that follows the bone's composed
//! pose ([`RigPose::anchor_for`]) and, under it, the item root at the attachment's offset, as
//! benilla's held-item lane does (`entities/equipment/spawn.rs`, benilla @ 2e82d34). The fingers
//! close by the model's `HandsClosed` overlay at weight 8 over the gait
//! (`creature_anim/driver/grip.rs`, `HAND_GRIP_WEIGHT`).
//!
//! The item's `$WTT`/`$WTB` markers (the swing-trail ends, top and bottom of the blade) are kept
//! on [`HeldBuilt`] for the blade hitbox.

use bevy::mesh::MeshTag;
use bevy::prelude::*;

use benilla_assets::{m2_url, skin_url, M2Model, ModelAnimations};
use benilla_formats::CharSkinSlot;
use benilla_world::model_render::{EntityUvLane, M2BatchMaterials, ModelKind, ModelPart};
use benilla_world::rig_anim::RigPose;

use super::{ActorBuilt, ActorModel, Catalogs};

/// The right-hand attachment point id.
pub const ATTACH_HAND_RIGHT: u16 = 1;
/// The `HandsClosed` overlay's weight over the gait (benilla `HAND_GRIP_WEIGHT`).
const HAND_GRIP_WEIGHT: f32 = 8.0;
/// Where weapon models and textures live.
const WEAPON_DIR: &str = "Item\\ObjectComponents\\Weapon";
/// Where shield models and textures live.
const SHIELD_DIR: &str = "Item\\ObjectComponents\\Shield";
/// The forearm attachment a drawn shield hangs from, and the left palm a drawn off-hand weapon
/// does (benilla `equipment::attach_id::{SHIELD, HAND_LEFT}`).
pub const ATTACH_SHIELD: u16 = 0;
pub const ATTACH_HAND_LEFT: u16 = 2;

/// A wearer's wish to hold an item display in a hand.
#[derive(Component, Debug, Clone)]
pub struct HeldItem {
    pub display: u32,
    pub attach: u16,
    /// Resolved from the catalog on first sight.
    m2: Option<Handle<M2Model>>,
    texture: Option<String>,
}

impl HeldItem {
    pub fn right_hand(display: u32) -> Self {
        Self {
            display,
            attach: ATTACH_HAND_RIGHT,
            m2: None,
            texture: None,
        }
    }

    /// An item in the left hand (a left-hander's weapon).
    pub fn left_hand(display: u32) -> Self {
        Self {
            attach: ATTACH_HAND_LEFT,
            ..Self::right_hand(display)
        }
    }
}

/// The held item is on: its root (under the hand anchor) and the blade's trail markers in the
/// item root's frame, `[top, bottom]`, when the model authors them.
#[derive(Component, Debug, Clone, Copy)]
pub struct HeldBuilt {
    pub root: Entity,
    pub blade: Option<[Vec3; 2]>,
    /// Which side of the shaft a lopsided head (an axe's) hangs on, in the item root's frame:
    /// where its edge is ([`crate::motion_clip::head_side`]); none for an even model.
    pub edge: Option<Vec3>,
    /// Measured palms for the Militia Warhammer's two-hand fit. Other models keep their mount.
    pub maul_palms: Option<[(usize, Vec3); 2]>,
}

/// The left arm's item: a shield on the forearm, or a weapon in the left hand.
#[derive(Component, Debug, Clone)]
pub struct OffHandItem {
    pub display: u32,
    pub shield: bool,
    m2: Option<Handle<M2Model>>,
    texture: Option<String>,
}

impl OffHandItem {
    pub fn new(display: u32, shield: bool) -> Self {
        Self {
            display,
            shield,
            m2: None,
            texture: None,
        }
    }

    fn attach(&self) -> u16 {
        if self.shield {
            ATTACH_SHIELD
        } else {
            ATTACH_HAND_LEFT
        }
    }

    fn dir(&self) -> &'static str {
        if self.shield {
            SHIELD_DIR
        } else {
            WEAPON_DIR
        }
    }
}

/// The off-hand item is on: its root under the arm's anchor.
#[derive(Component, Debug, Clone, Copy)]
pub struct OffHandBuilt {
    pub root: Entity,
}

/// Nothing in the main hand: the fist strikes in a weapon's place (see [`build_fist`]).
#[derive(Component, Debug, Clone, Copy)]
pub struct Unarmed;

/// How far (yd) the bare fist reaches past the palm, as a blade marker.
pub const FIST_REACH: f32 = 0.3;
/// The direction of that reach in the hand attachment's frame: the axis a weapon's blade takes (the
/// short sword's `$WTT` / `$WTB` markers measure (0, 0, -0.89) and (0, 0, -0.19) in the item root).
pub const FIST_AXIS: Vec3 = Vec3::NEG_Z;

/// The left hand is closed around an off-hand weapon (a shield on the forearm leaves it open).
#[derive(Component)]
pub struct LeftGrip;

/// On the root of a worn item hung from an attachment point (a helm, a shoulder pad): the wearer,
/// so a change of dress can take them off again.
#[derive(Component, Debug, Clone, Copy)]
pub struct WornRoot(pub Entity);

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            build,
            build_off_hand,
            build_fist,
            build_worn,
            grip,
            grip_left,
            release_grips,
        ),
    )
    .add_systems(
        PostUpdate,
        draw_blade.after(benilla_world::billboard::BillboardPlace),
    );
}

#[allow(clippy::too_many_arguments)]
fn build(
    mut commands: Commands,
    mut wearers: Query<
        (Entity, &mut HeldItem, &ActorModel, &mut RigPose),
        (With<ActorBuilt>, Without<HeldBuilt>),
    >,
    catalogs: Option<Res<Catalogs>>,
    asset_server: Res<AssetServer>,
    m2s: Res<Assets<M2Model>>,
    mut forms: ResMut<benilla_world::model_forms::ModelForms>,
    mut mats: M2BatchMaterials,
    mut uv_reg: ResMut<benilla_world::doodad_anim::UvAnimMaterials>,
    mut anim_table: ResMut<benilla_world::mat_anim_table::MatAnimTable>,
) {
    for (wearer, mut held, body, mut pose) in &mut wearers {
        if held.m2.is_none() {
            let Some(row) = catalogs.as_deref().and_then(|c| c.items.get(held.display)) else {
                warn!("held: item display {} not in the catalog", held.display);
                commands.entity(wearer).remove::<HeldItem>();
                continue;
            };
            let Some(model) = row.model[0].as_deref() else {
                warn!("held: item display {} names no model", held.display);
                commands.entity(wearer).remove::<HeldItem>();
                continue;
            };
            held.m2 = Some(asset_server.load(m2_url(&format!("{WEAPON_DIR}\\{model}"))));
            held.texture = row.model_texture[0].clone();
        }
        let handle = held.m2.clone().expect("set above");
        let (Some(item), Some(body_model)) = (m2s.get(&handle), m2s.get(&body.m2)) else {
            continue; // loading
        };
        if !forms.require_rigged(&handle, 0) || !mats.ready() {
            continue;
        }
        let Some(point) = body_model
            .attachments
            .iter()
            .find(|a| a.id == held.attach)
            .copied()
        else {
            warn!("held: the body has no attachment {}", held.attach);
            commands.entity(wearer).remove::<HeldItem>();
            continue;
        };
        let Some(joint) = pose.anchor_for(&mut commands, wearer, point.bone) else {
            continue;
        };
        let root = commands
            .spawn((
                Name::new("Held item"),
                Transform::from_translation(point.offset),
                Visibility::default(),
            ))
            .id();
        commands.entity(joint).add_child(root);

        let texture = held
            .texture
            .as_deref()
            .map(|t| asset_server.load::<Image>(skin_url(WEAPON_DIR, t)));
        let parts = spawn_item_parts(
            &mut commands,
            root,
            &handle,
            item,
            texture,
            wearer,
            &mut forms,
            &mut mats,
            &mut uv_reg,
            &mut anim_table,
        );
        let blade = item.string_anchors.map(|[top, bottom]| [top.1, bottom.1]);
        let head = blade.and_then(|blade| {
            crate::motion_clip::head_side(
                item.submeshes
                    .iter()
                    .filter(|s| s.billboard.is_none())
                    .flat_map(|s| s.geometry.positions.iter())
                    .map(|p| benilla_assets::coords::wow_to_bevy(*p)),
                blade,
            )
        });
        let maul_palms = (held.display == 19544 && point.bone == 104)
            .then(|| {
                let left = body_model
                    .attachments
                    .iter()
                    .find(|a| a.id == ATTACH_HAND_LEFT && a.bone == 105)?;
                Some([
                    (usize::from(point.bone), point.offset),
                    (usize::from(left.bone), left.offset),
                ])
            })
            .flatten();
        commands.entity(wearer).insert((
            HeldBuilt {
                root,
                blade,
                edge: head.map(|(side, _)| side),
                maul_palms,
            },
            HandGrip,
        ));
        info!(
            "held: item display {} in hand {} ({parts} parts, blade markers {})",
            held.display,
            held.attach,
            if blade.is_some() { "yes" } else { "no" }
        );
        if let Some([top, bottom]) = blade {
            debug!("held: blade top {top:?} bottom {bottom:?}, head side {head:?}");
        }
    }
}

/// The off-hand item hung from its attachment point: the shield on the forearm, the weapon in the
/// left hand. The same build as the main hand's, without a blade to strike with.
#[allow(clippy::too_many_arguments)]
fn build_off_hand(
    mut commands: Commands,
    mut wearers: Query<
        (Entity, &mut OffHandItem, &ActorModel, &mut RigPose),
        (With<ActorBuilt>, Without<OffHandBuilt>),
    >,
    catalogs: Option<Res<Catalogs>>,
    asset_server: Res<AssetServer>,
    m2s: Res<Assets<M2Model>>,
    mut forms: ResMut<benilla_world::model_forms::ModelForms>,
    mut mats: M2BatchMaterials,
    mut uv_reg: ResMut<benilla_world::doodad_anim::UvAnimMaterials>,
    mut anim_table: ResMut<benilla_world::mat_anim_table::MatAnimTable>,
) {
    for (wearer, mut held, body, mut pose) in &mut wearers {
        if held.m2.is_none() {
            let Some(row) = catalogs.as_deref().and_then(|c| c.items.get(held.display)) else {
                warn!("off hand: item display {} not in the catalog", held.display);
                commands.entity(wearer).remove::<OffHandItem>();
                continue;
            };
            let Some(model) = row.model[0].as_deref() else {
                warn!("off hand: item display {} names no model", held.display);
                commands.entity(wearer).remove::<OffHandItem>();
                continue;
            };
            held.m2 = Some(asset_server.load(m2_url(&format!("{}\\{model}", held.dir()))));
            held.texture = row.model_texture[0].clone();
        }
        let handle = held.m2.clone().expect("set above");
        let (Some(item), Some(body_model)) = (m2s.get(&handle), m2s.get(&body.m2)) else {
            continue; // loading
        };
        if !forms.require_rigged(&handle, 0) || !mats.ready() {
            continue;
        }
        let attach = held.attach();
        let Some(point) = body_model
            .attachments
            .iter()
            .find(|a| a.id == attach)
            .copied()
        else {
            warn!("off hand: the body has no attachment {attach}");
            commands.entity(wearer).remove::<OffHandItem>();
            continue;
        };
        let Some(joint) = pose.anchor_for(&mut commands, wearer, point.bone) else {
            continue;
        };
        let root = commands
            .spawn((
                Name::new("Off-hand item"),
                Transform::from_translation(point.offset),
                Visibility::default(),
            ))
            .id();
        commands.entity(joint).add_child(root);
        let texture = held
            .texture
            .as_deref()
            .map(|t| asset_server.load::<Image>(skin_url(held.dir(), t)));
        let parts = spawn_item_parts(
            &mut commands,
            root,
            &handle,
            item,
            texture,
            wearer,
            &mut forms,
            &mut mats,
            &mut uv_reg,
            &mut anim_table,
        );
        commands.entity(wearer).insert(OffHandBuilt { root });
        if !held.shield {
            commands.entity(wearer).insert(LeftGrip);
        }
        info!(
            "held: off-hand display {} ({}) on attachment {attach} ({parts} parts)",
            held.display,
            if held.shield { "shield" } else { "weapon" }
        );
    }
}

/// A bare fist in the main hand's place: the blade the strike sweeps is a short stub out of the
/// palm, along the axis a weapon's blade takes, so an unarmed swing still lands.
fn build_fist(
    mut commands: Commands,
    mut wearers: Query<
        (Entity, &ActorModel, &mut RigPose),
        (
            With<ActorBuilt>,
            With<Unarmed>,
            Without<HeldBuilt>,
            Without<HeldItem>,
        ),
    >,
    m2s: Res<Assets<M2Model>>,
) {
    for (wearer, body, mut pose) in &mut wearers {
        let Some(body_model) = m2s.get(&body.m2) else {
            continue;
        };
        let Some(point) = body_model
            .attachments
            .iter()
            .find(|a| a.id == ATTACH_HAND_RIGHT)
            .copied()
        else {
            commands.entity(wearer).remove::<Unarmed>();
            continue;
        };
        let Some(joint) = pose.anchor_for(&mut commands, wearer, point.bone) else {
            continue;
        };
        let root = commands
            .spawn((
                Name::new("Fist"),
                Transform::from_translation(point.offset),
                Visibility::default(),
            ))
            .id();
        commands.entity(joint).add_child(root);
        commands.entity(wearer).insert(HeldBuilt {
            root,
            blade: Some([FIST_AXIS * FIST_REACH, Vec3::ZERO]),
            edge: None,
            maul_palms: None,
        });
        info!("held: bare fist on attachment {ATTACH_HAND_RIGHT}");
    }
}

/// Spawns an item model's batches under `root`: the object batches take `texture`, the rest their
/// own. Returns how many parts were drawn.
#[allow(clippy::too_many_arguments)]
fn spawn_item_parts(
    commands: &mut Commands,
    root: Entity,
    handle: &Handle<M2Model>,
    item: &M2Model,
    texture: Option<Handle<Image>>,
    wearer: Entity,
    forms: &mut benilla_world::model_forms::ModelForms,
    mats: &mut M2BatchMaterials,
    uv_reg: &mut benilla_world::doodad_anim::UvAnimMaterials,
    anim_table: &mut benilla_world::mat_anim_table::MatAnimTable,
) -> usize {
    let stat = forms.slices(handle).stat;
    let mut parts = 0;
    for (pi, sub) in item.submeshes.iter().enumerate() {
        if sub.billboard.is_some() {
            continue;
        }
        let tex = match sub.char_slot {
            Some(CharSkinSlot::Object) => texture.clone(),
            _ => sub.texture.clone(),
        };
        let order = u16::try_from(pi + 1).unwrap_or(u16::MAX);
        let mut lane = EntityUvLane {
            reg: &mut *uv_reg,
            table: &mut *anim_table,
            instance: None,
        };
        let Some(v) = mats.entity_variants(sub, tex, order, &mut lane) else {
            continue;
        };
        let Some((mesh, aabb)) = stat.get(pi).cloned() else {
            continue;
        };
        let mut part = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(v.steady.clone()),
            Transform::default(),
            ChildOf(root),
            ModelPart {
                kind: ModelKind::Creature,
                blend: sub.blend,
            },
            MeshTag(benilla_world::mesh_tag::spawn_tag(0, 1.0)),
            // Held and worn items cast their shadow with the body (the character shadow lane).
            benilla_world::interact::PickMesh(sub.geometry.clone()),
        ));
        if let Some(aabb) = aabb {
            part.insert(aabb);
        }
        // Indoors it takes the wearer's light verdict, as the reference aliases the wearer's
        // collector into each held item.
        if let Some(lit) = benilla_world::interior::part_interior_lit(
            &v.steady,
            Some(&v.interior),
            Some(&v.interior_bake),
            Vec3::ZERO,
            wearer,
        ) {
            part.insert(lit);
        }
        parts += 1;
    }
    parts
}

/// Where a worn item's models live, which of the display's two models it shows and where it
/// hangs (benilla `entities/equipment`: `ensure_item_model`, `attach_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WornKind {
    /// A helm: a model per race and sex (`Helm_Leather_A_03Defias_HuMale.m2`), on the head.
    Helm {
        race: u8,
        sex: u8,
    },
    ShoulderLeft,
    ShoulderRight,
}

impl WornKind {
    fn attach(self) -> u16 {
        match self {
            WornKind::Helm { .. } => 11,
            WornKind::ShoulderLeft => 6,
            WornKind::ShoulderRight => 5,
        }
    }

    fn dir(self) -> &'static str {
        match self {
            WornKind::Helm { .. } => "Item\\ObjectComponents\\Head",
            WornKind::ShoulderLeft | WornKind::ShoulderRight => "Item\\ObjectComponents\\Shoulder",
        }
    }

    /// The display's left (0) or right (1) model.
    fn column(self) -> usize {
        usize::from(self == WornKind::ShoulderRight)
    }
}

/// One worn item, built once its model is in.
#[derive(Debug, Clone)]
pub struct Worn {
    pub display: u32,
    pub kind: WornKind,
    m2: Option<Handle<M2Model>>,
    texture: Option<String>,
}

impl Worn {
    pub fn new(display: u32, kind: WornKind) -> Self {
        Self {
            display,
            kind,
            m2: None,
            texture: None,
        }
    }
}

/// What a body wears that hangs from an attachment point: a character NPC's helm and shoulders
/// (`CreatureDisplayInfoExtra`). Each entry leaves the list when its model is on, or when the
/// display names no model or the body has no such attachment.
#[derive(Component, Debug, Default)]
pub struct WornItems(pub Vec<Worn>);

/// The helm model's file for a race and sex: `Hu Or Dw Ni Sc Ta Gn Tr`, male or female.
fn helm_file(model: &str, race: u8, sex: u8) -> String {
    const RACE_PREFIX: [&str; 8] = ["Hu", "Or", "Dw", "Ni", "Sc", "Ta", "Gn", "Tr"];
    let prefix = RACE_PREFIX[usize::from(race.clamp(1, 8) - 1)];
    let letter = if sex == 1 { 'F' } else { 'M' };
    let stem = model.strip_suffix(".m2").unwrap_or(model);
    format!("{stem}_{prefix}{letter}.m2")
}

#[allow(clippy::too_many_arguments)]
fn build_worn(
    mut commands: Commands,
    mut wearers: Query<(Entity, &mut WornItems, &ActorModel, &mut RigPose), With<ActorBuilt>>,
    catalogs: Option<Res<Catalogs>>,
    asset_server: Res<AssetServer>,
    m2s: Res<Assets<M2Model>>,
    mut forms: ResMut<benilla_world::model_forms::ModelForms>,
    mut mats: M2BatchMaterials,
    mut uv_reg: ResMut<benilla_world::doodad_anim::UvAnimMaterials>,
    mut anim_table: ResMut<benilla_world::mat_anim_table::MatAnimTable>,
) {
    let Some(catalogs) = catalogs else {
        return;
    };
    for (wearer, mut worn, body, mut pose) in &mut wearers {
        let mut i = 0;
        while i < worn.0.len() {
            let w = &mut worn.0[i];
            if w.m2.is_none() {
                let col = w.kind.column();
                let Some(model) = catalogs.items.get(w.display).and_then(|row| {
                    row.model[col]
                        .as_deref()
                        .map(|m| (m, &row.model_texture[col]))
                }) else {
                    // A helm or shoulder row without a model (a bandana drawn on the body's own
                    // texture, a shoulder pair with one side only).
                    worn.0.remove(i);
                    continue;
                };
                let file = match w.kind {
                    WornKind::Helm { race, sex } => helm_file(model.0, race, sex),
                    _ => model.0.to_string(),
                };
                w.m2 = Some(asset_server.load(m2_url(&format!("{}\\{file}", w.kind.dir()))));
                w.texture = model.1.clone();
            }
            let handle = w.m2.clone().expect("set above");
            let (kind, item_id, texture_name) = (w.kind, w.display, w.texture.clone());
            let (Some(item), Some(body_model)) = (m2s.get(&handle), m2s.get(&body.m2)) else {
                i += 1; // loading
                continue;
            };
            if !forms.require_rigged(&handle, 0) || !mats.ready() {
                i += 1;
                continue;
            }
            let Some(point) = body_model
                .attachments
                .iter()
                .find(|a| a.id == kind.attach())
                .copied()
            else {
                warn!(
                    "worn: the body has no attachment {} for item {item_id}",
                    kind.attach()
                );
                worn.0.remove(i);
                continue;
            };
            let Some(joint) = pose.anchor_for(&mut commands, wearer, point.bone) else {
                i += 1;
                continue;
            };
            let root = commands
                .spawn((
                    Name::new("Worn item"),
                    WornRoot(wearer),
                    Transform::from_translation(point.offset),
                    Visibility::default(),
                ))
                .id();
            commands.entity(joint).add_child(root);
            let texture = texture_name
                .as_deref()
                .map(|t| asset_server.load::<Image>(skin_url(kind.dir(), t)));
            let parts = spawn_item_parts(
                &mut commands,
                root,
                &handle,
                item,
                texture,
                wearer,
                &mut forms,
                &mut mats,
                &mut uv_reg,
                &mut anim_table,
            );
            info!(
                "worn: item display {item_id} ({kind:?}) on attachment {} ({parts} parts)",
                kind.attach()
            );
            worn.0.remove(i);
        }
    }
}

/// The right hand is closed around a held item.
#[derive(Component)]
pub struct HandGrip;

/// Keeps the right hand's `HandsClosed` overlay playing over whatever the body does.
fn grip(mut wearers: Query<(&ModelAnimations, &mut AnimationPlayer), With<HandGrip>>) {
    for (anims, mut player) in &mut wearers {
        let Some(node) = anims.hand_close[0] else {
            continue;
        };
        if player.animation(node).is_none() {
            let active = player.play(node);
            // A single-key pose: repeat so it never finishes.
            active.repeat();
            active.set_weight(HAND_GRIP_WEIGHT);
        }
    }
}

/// Keeps the left hand's `HandsClosed` overlay playing around an off-hand weapon.
fn grip_left(mut wearers: Query<(&ModelAnimations, &mut AnimationPlayer), With<LeftGrip>>) {
    for (anims, mut player) in &mut wearers {
        let Some(node) = anims.hand_close[1] else {
            continue;
        };
        if player.animation(node).is_none() {
            let active = player.play(node);
            active.repeat();
            active.set_weight(HAND_GRIP_WEIGHT);
        }
    }
}

/// A hand that lets go of its item opens again: the overlay is stopped when its marker goes.
fn release_grips(
    mut right: RemovedComponents<HandGrip>,
    mut left: RemovedComponents<LeftGrip>,
    mut wearers: Query<(&ModelAnimations, &mut AnimationPlayer)>,
) {
    for (hand, entity) in right
        .read()
        .map(|e| (0, e))
        .chain(left.read().map(|e| (1, e)))
    {
        if let Ok((anims, mut player)) = wearers.get_mut(entity) {
            if let Some(node) = anims.hand_close[hand] {
                player.stop(node);
            }
        }
    }
}

/// F2: the blade segment between the trail markers, in the world, over the drawn weapon.
pub fn draw_blade(
    show: Res<crate::avatar::ShowCapsule>,
    held: Query<&HeldBuilt>,
    roots: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    if !show.0 {
        return;
    }
    for h in &held {
        let (Some([top, bottom]), Ok(root)) = (h.blade, roots.get(h.root)) else {
            continue;
        };
        let (a, b) = (root.transform_point(top), root.transform_point(bottom));
        gizmos.line(a, b, Color::srgb(1.0, 0.3, 0.2));
        gizmos.sphere(
            Isometry3d::from_translation(a),
            0.04,
            Color::srgb(1.0, 0.3, 0.2),
        );
    }
}
