//! Authored treasures layered over baseline WoW loot. Inventory is owned once in wow-world.
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use benilla_assets::{m2_url, ModelAnimations};
use benilla_world::collision::{ColliderEpoch, WorldCollision};
use bevy::animation::RepeatAnimation;
use bevy::prelude::*;
use wow_world::inventory::{ChestState, Progress};
use wow_world::model::{ItemCount, LootKind, World};

use crate::actor::{ActorModel, Look};
use crate::avatar::Avatar;
use crate::combat::{Action, Health};
use crate::controller::PlaySet;
use crate::motor::{Motor, MoveIntent};
use crate::settings::binds::Action as BindAction;
use crate::settings::Controls;
use crate::settle::Phase;
pub mod interaction;
mod treasures;
pub(crate) mod ui;
pub(crate) mod vfx;
use interaction::{Active, Kind, Playing};

/// The screen's input system: the equipment page orders itself around it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Drive;

pub const SALVE: u32 = 9_000_001;
pub const ABBEY_CHEST: u32 = 9_000_001;
const CHEST_MODEL: &str = "World/SkillActivated/Containers/TreasureChest01.m2";
const SCALE: f32 = 0.85;
const WALK_LAYER_BIT: u32 = 1 << 1;

#[derive(Resource)]
pub struct Owned {
    pub progress: Progress,
    pub path: PathBuf,
    pub blocked: Option<String>,
}
impl Owned {
    pub(crate) fn publish(&mut self, next: Progress) -> Result<(), String> {
        if let Some(e) = &self.blocked {
            return Err(e.clone());
        }
        self.progress
            .publish(next, &self.path)
            .map_err(|e| format!("Could not save items: {e}"))
    }
}
#[derive(Resource)]
pub struct Data(pub World);
/// The quest log supplies only items still needed by accepted quests. Empty until connected.
#[derive(Resource, Default)]
pub struct QuestLootEligibility(pub BTreeSet<u32>);
#[derive(Message, Debug, Clone)]
pub struct Acquired {
    pub items: Vec<ItemCount>,
    pub discovery: bool,
}
#[derive(Resource, Default)]
pub struct Screen {
    pub open: bool,
    pub selected: usize,
    pub prompt: String,
    pub notice: String,
    pub notice_detail: String,
    pub notice_item: Option<u32>,
    pub notice_items: Vec<ItemCount>,
    pub notice_until: f32,
    pub discovery: bool,
    pub error: Option<String>,
    pub use_requested: bool,
    pub close_requested: bool,
    /// A modal screen of the quest phase (dialogue, quest log) is up: it holds the player's attacks,
    /// evades and jumps ([`Screen::blocks_actions`]) but, unlike the inventory, not their running or
    /// their camera.
    pub modal: bool,
    /// Which page is up: the pack's item list or the equipment (paper doll).
    pub page: crate::equip::Page,
    /// The equip button was pressed on the selected item of the list; [`crate::equip`] answers it.
    pub equip_requested: bool,
    /// The equipment page took this frame's back press (it closed its choice list), so the screen
    /// stays open.
    pub back_taken: bool,
    /// Real second until which a press the quest screen took (A confirms, B goes back) is still its
    /// own, so the same press does not also jump or roll in the frame the screen closes.
    pub swallow_until: f32,
}
impl Screen {
    /// The player's movement, camera and lock-on are held: the inventory is open.
    pub fn locked(&self) -> bool {
        self.open
    }

    /// The body's own actions (attacks, evade, guard, jump) are held at real second `now`: the
    /// inventory or a quest screen is up, or a quest screen just took the press.
    pub fn blocks_actions(&self, now: f32) -> bool {
        self.open || self.modal || now < self.swallow_until
    }
}
#[derive(Component)]
struct Chest {
    id: u32,
    reward: Option<&'static [ItemCount]>,
    restock: u32,
    table: u32,
}
#[derive(Component)]
struct ChestClip {
    id: u16,
}
/// The loot sparkles over a chest that is open and still holds something.
#[derive(Component)]
struct ChestGlow(Entity);
/// How far above a chest's origin the sparkles start (yd, in its own frame): the open lid.
const GLOW_LIFT: f32 = 0.35;
#[derive(Component)]
struct Corpse {
    items: Vec<ItemCount>,
    glow: Entity,
}
/// How far (yd, along the ground) around the player another corpse still joins a loot: one press takes
/// every dead creature's loot of the same area.
pub const AREA_LOOT_RADIUS: f32 = 10.0;

/// A corpse taken in an area loot: the creature, its glow, what it carried.
struct Taken {
    corpse: Entity,
    glow: Entity,
    items: Vec<ItemCount>,
}

/// The corpses one loot takes: `target` and every other dead creature that still carries loot within
/// [`AREA_LOOT_RADIUS`] of the player.
#[allow(clippy::type_complexity)]
fn area_corpses(
    player: Vec3,
    target: Entity,
    targets: &Query<
        (
            Entity,
            &Transform,
            Option<&Chest>,
            Option<&Corpse>,
            Option<&crate::npc::Reaction>,
        ),
        Without<Avatar>,
    >,
) -> Vec<Taken> {
    targets
        .iter()
        .filter_map(|(entity, xf, _, corpse, reaction)| {
            let corpse = corpse?;
            let dead = reaction == Some(&crate::npc::Reaction::Dead);
            let near = Vec2::new(xf.translation.x - player.x, xf.translation.z - player.z).length()
                <= AREA_LOOT_RADIUS;
            ((entity == target) || (dead && near)).then(|| Taken {
                corpse: entity,
                glow: corpse.glow,
                items: corpse.items.clone(),
            })
        })
        .collect()
}

/// The items of several corpses as one list, equal items added together, in the order first met.
fn merged<'a>(lists: impl IntoIterator<Item = &'a [ItemCount]>) -> Vec<ItemCount> {
    let mut all: Vec<ItemCount> = Vec::new();
    for &(item, n) in lists.into_iter().flatten() {
        match all.iter_mut().find(|(i, _)| *i == item) {
            Some(slot) => slot.1 += n,
            None => all.push((item, n)),
        }
    }
    all
}

#[derive(Resource, Default)]
struct Candidate(Option<Entity>);
#[derive(Resource, Default)]
struct Instruments {
    interact: Option<crate::combat::Schedule>,
    next: Option<f32>,
    inventory: Option<f32>,
    /// `WOWSOULS_GEAR_AT=<s>`: opens the equipment page at real second `s`.
    gear: Option<f32>,
    use_at: Option<f32>,
}

/// How corpses, chests and quest objects roll their tables: quest items always drop, the rest at WoW's
/// chance (the user, 2026-10-18; everything dropped on 2026-10-17 was too much).
/// `WOWSOULS_LOOT_CHANCES=wow` (quest items at their chance too) or `=all` (everything drops).
pub fn chances() -> wow_world::loot::Chances {
    match std::env::var("WOWSOULS_LOOT_CHANCES").as_deref() {
        Ok("wow") => wow_world::loot::Chances::Wow,
        Ok("all") => wow_world::loot::Chances::Always,
        _ => wow_world::loot::Chances::QuestItems,
    }
}

fn unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn plugin(app: &mut App) {
    app.init_resource::<Active>()
        .init_resource::<Screen>()
        .init_resource::<Candidate>()
        .init_resource::<QuestLootEligibility>()
        .add_message::<Acquired>()
        .add_systems(Startup, load)
        .add_systems(Update, place_chests.run_if(in_state(Phase::Playing)))
        .add_systems(Update, corpse_loot.run_if(in_state(Phase::Playing)))
        .add_systems(Update, release_held.run_if(in_state(Phase::Playing)))
        .add_systems(Update, chest_glow.run_if(in_state(Phase::Playing)))
        .add_systems(
            Update,
            drive
                .in_set(PlaySet::Intent)
                .in_set(Drive)
                .after(crate::combat::act),
        )
        .add_systems(
            PostUpdate,
            animate_chests
                .before(bevy::app::AnimationSystems)
                .run_if(in_state(Phase::Playing)),
        )
        .add_systems(PostUpdate, free_hands.run_if(in_state(Phase::Playing)))
        .add_plugins((ui::plugin, vfx::plugin));
}
pub(crate) fn load(mut commands: Commands) {
    let path = std::env::var("WOWSOULS_ITEMS_SAVE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if crate::run_mode::unattended() {
                PathBuf::from(format!("cache/loot-check/items-{}.txt", std::process::id()))
            } else {
                crate::saves::file("items-v1.txt")
            }
        });
    let (progress, blocked) = match Progress::load(&path) {
        Ok(p) => (p, None),
        Err(e) => (
            Progress::default(),
            Some(format!("Inventory save could not be read: {e}")),
        ),
    };
    info!(
        "loot: save {} owned {} chest states {}",
        path.display(),
        progress.inventory.items.len(),
        progress.chests.len()
    );
    commands.insert_resource(Owned {
        progress,
        path,
        blocked,
    });
    let world_path = crate::world::manifest_path();
    let mut world = std::fs::read_to_string(&world_path)
        .ok()
        .and_then(|s| wow_world::manifest::parse(&s).ok())
        .unwrap_or_default();
    // The belt torch, ours: its item row and the general goods traders who sell it.
    wow_world::torch::add_to(&mut world);
    commands.insert_resource(Data(world));
    match interaction::Kit::load(&crate::motion_clip::cache_dir()) {
        Ok(k) => {
            commands.insert_resource(k);
        }
        Err(e) => {
            warn!("loot: interactions unavailable ({e}); restore the project anim/source interaction clips");
        }
    }
    commands.insert_resource(Instruments {
        interact: std::env::var("WOWSOULS_INTERACT")
            .ok()
            .and_then(|s| crate::combat::parse_schedule(&s)),
        inventory: std::env::var("WOWSOULS_INVENTORY_AT")
            .ok()
            .and_then(|s| s.parse().ok()),
        gear: std::env::var("WOWSOULS_GEAR_AT")
            .ok()
            .and_then(|s| s.parse().ok()),
        use_at: std::env::var("WOWSOULS_USE_AT")
            .ok()
            .and_then(|s| s.parse().ok()),
        ..default()
    });
}
fn place_chests(
    mut commands: Commands,
    data: Res<Data>,
    owned: Res<Owned>,
    assets: Res<AssetServer>,
    collide: WorldCollision,
    mut epoch: ResMut<ColliderEpoch>,
    mut placed: Local<BTreeSet<u32>>,
) {
    if crate::run_mode::unattended() && std::env::var("WOWSOULS_LOOT").as_deref() != Ok("1") {
        return;
    }
    let mut spots: Vec<_> = treasures::TREASURES
        .iter()
        .filter(|t| t.available(&data.0))
        .map(|t| (t.id, t.wow, t.yaw, t.name, Some(t.items), 0, 0))
        .collect();
    if crate::world::world_on() {
        for s in data
            .0
            .object_spawns
            .iter()
            .filter(|s| s.entry == 2843 && !s.event)
        {
            if let Some(t) = data.0.objects.iter().find(|t| t.entry == s.entry) {
                spots.push((
                    s.guid,
                    s.pos,
                    s.orientation,
                    "Battered Chest",
                    None,
                    s.respawn_max.max(300),
                    t.data[1] as u32,
                ));
            }
        }
    }
    for (id, wow, yaw, name, reward, restock, table) in spots {
        if placed.contains(&id) {
            continue;
        }
        let mut at = benilla_assets::coords::wow_to_bevy(wow);
        // Authored cave caches must wait for their own floor. The general spawn snap can fall
        // back from 60 yd above, which could permanently put a chest on a mine's roof instead.
        let ground = if reward.is_some() {
            collide
                .ray_body(at + Vec3::Y * 1.5, Dir3::NEG_Y, 3.0)
                .map(|hit| at.y + 1.5 - hit.distance)
        } else {
            crate::settle::snap_to_ground(&collide, at)
        };
        let Some(y) = ground else {
            continue;
        };
        at.y = y;
        let xf = Transform::from_translation(at)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(Vec3::splat(SCALE));
        commands.spawn((
            Name::new(name),
            xf,
            Visibility::default(),
            Chest {
                id,
                reward,
                restock,
                table,
            },
            ActorModel {
                m2: assets.load(m2_url(CHEST_MODEL)),
                dir: "World/SkillActivated/Containers".into(),
                look: Look::Creature {
                    skins: [None, None, None],
                },
                scale: SCALE,
                display: 259,
                worn: None,
            },
        ));
        // A solid base only; the opening lid is animated visual geometry.
        use avian3d::prelude::*;
        commands.spawn((
            Name::new("Chest base"),
            Transform::from_translation(at + Vec3::Y * 0.27).with_rotation(xf.rotation),
            RigidBody::Static,
            Collider::cuboid(1.55 * SCALE, 0.54, 1.18 * SCALE),
            CollisionLayers::new(LayerMask(WALK_LAYER_BIT), LayerMask::ALL),
        ));
        epoch.bump();
        placed.insert(id);
        info!(
            "loot: chest {id} at {:?} state {:?}",
            benilla_assets::coords::bevy_to_wow(at),
            owned.progress.chests.get(&id)
        );
    }
}
fn corpse_loot(
    mut commands: Commands,
    mut slain: MessageReader<crate::npc::Slain>,
    data: Res<Data>,
    quests: Res<QuestLootEligibility>,
    book: Option<Res<crate::world::WorldBook>>,
    bodies: Query<(
        &Transform,
        &crate::species::Species,
        Option<&crate::world::SpawnId>,
    )>,
    old: Query<(Entity, &Corpse, &crate::npc::Reaction)>,
    art: Option<Res<vfx::Art>>,
    mut state: ResMut<crate::world_state::WorldStateFile>,
) {
    for (e, loot, reaction) in &old {
        if *reaction != crate::npc::Reaction::Dead {
            commands.entity(loot.glow).despawn();
            commands.entity(e).remove::<Corpse>();
        }
    }
    for dead in slain.read() {
        let Ok((_, species, spot)) = bodies.get(dead.who) else {
            continue;
        };
        let entry = spot
            .zip(book.as_deref())
            .and_then(|(s, b)| b.book.spots.get(s.0))
            .map(|s| s.entry)
            .or_else(|| {
                data.0
                    .creatures
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(species.0.title))
                    .map(|c| c.entry)
            });
        let entry = entry.or_else(|| (species.0.name == "kobold").then_some(80));
        let table = entry
            .and_then(|e| data.0.creatures.iter().find(|c| c.entry == e))
            .map(|c| if c.loot_id == 0 { c.entry } else { c.loot_id });
        let Some(table) = table else {
            continue;
        };
        let mut items = match wow_world::loot::roll_with(
            &data.0.loot,
            LootKind::Creature,
            table,
            std::env::var("WOWSOULS_LOOT_SEED")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| unix() ^ dead.who.to_bits()),
            &quests.0,
            crate::loot::chances(),
        ) {
            Ok(i) => i,
            Err(e) => {
                warn!("loot: corpse table {table}: {e}");
                continue;
            }
        };
        // A boss or a rare gives the best of its table the first time it dies (once per creature entry).
        let rank = crate::species::rank_of(species.0);
        if rank.first_kill_best() {
            if let Some(entry) = entry.filter(|e| !state.state.first.contains(e)) {
                if let Some(best) = wow_world::loot::best_drop(
                    &data.0.loot,
                    &data.0.items,
                    LootKind::Creature,
                    table,
                ) {
                    match items.iter_mut().find(|(i, _)| *i == best) {
                        Some(slot) => slot.1 = slot.1.max(1),
                        None => items.push((best, 1)),
                    }
                    info!(
                        "loot: first kill of entry {entry}: the best of its table is item {best}"
                    );
                }
                state.state.first.insert(entry);
                state.mark();
            }
        }
        if items.is_empty() {
            continue;
        }
        let glow = if let Some(art) = art.as_deref() {
            vfx::spawn(&mut commands, dead.who, art)
        } else {
            commands
                .spawn((Transform::default(), ChildOf(dead.who)))
                .id()
        };
        info!("loot: rolled corpse {table} {:?}", items);
        // What a never-respawning creature carries is remembered until it is taken.
        if rank.never_respawns() {
            if let Some(guid) = spot
                .zip(book.as_deref())
                .and_then(|(s, b)| b.book.spots.get(s.0))
                .map(|s| s.guid)
            {
                state.state.held.insert(guid, items.clone());
                state.mark();
            }
        }
        commands.entity(dead.who).insert(Corpse { items, glow });
    }
}

/// A never-respawning creature's corpse was looted (its `Corpse` is gone but the body stays): what it
/// held is the player's, so the world state forgets it. A body the ring removed has no spawn id any
/// more: its loot stays held for the next start.
fn release_held(
    mut gone: RemovedComponents<Corpse>,
    ids: Query<&crate::world::SpawnId>,
    book: Option<Res<crate::world::WorldBook>>,
    mut state: ResMut<crate::world_state::WorldStateFile>,
) {
    let Some(book) = book else { return };
    for e in gone.read() {
        let Ok(&crate::world::SpawnId(i)) = ids.get(e) else {
            continue;
        };
        if let Some(spot) = book.book.spots.get(i) {
            if state.state.held.remove(&spot.guid).is_some() {
                state.mark();
            }
        }
    }
}
/// Whether chest `id` is open with something in it: opened by the player and not yet emptied. The
/// state is the save's, so a chest opened in an earlier run glows again when the world loads.
fn holds_loot(progress: &Progress, id: u32) -> bool {
    matches!(progress.chests.get(&id), Some(ChestState::Open(items)) if !items.is_empty())
}
/// An opened chest that still holds something gives off WoW's loot sparkles, as a creature's corpse does
/// while it carries loot, so the player sees at a glance that there is something to take. They go with
/// the items: taking them, or the chest restocking shut, ends them.
fn chest_glow(
    mut commands: Commands,
    owned: Res<Owned>,
    art: Option<Res<vfx::Art>>,
    chests: Query<(Entity, &Chest, Option<&ChestGlow>)>,
) {
    let Some(art) = art else {
        return;
    };
    for (entity, chest, glow) in &chests {
        match (holds_loot(&owned.progress, chest.id), glow) {
            (true, None) => {
                let fx = vfx::spawn_chest(&mut commands, entity, &art, GLOW_LIFT);
                commands.entity(entity).insert(ChestGlow(fx));
                info!("loot: chest {} glows, it holds something", chest.id);
            }
            (false, Some(glow)) => {
                commands.entity(glow.0).despawn();
                commands.entity(entity).remove::<ChestGlow>();
            }
            _ => {}
        }
    }
}
fn chest_anchor(xf: &Transform) -> Vec3 {
    xf.translation + xf.rotation * Vec3::new(0.0, 0.0, -1.08)
}
pub fn item_name(data: &Data, id: u32) -> String {
    if id == SALVE {
        "Abbey Salve".into()
    } else {
        data.0
            .items
            .iter()
            .find(|i| i.entry == id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| format!("Item {id}"))
    }
}
pub fn item_detail(data: &Data, id: u32) -> String {
    if id == SALVE {
        "Restores 30 health. Prepared by the healers of Northshire Abbey.".into()
    } else if data
        .0
        .quests
        .iter()
        .any(|q| q.req_items.iter().any(|&(i, _)| i == id))
    {
        "Quest item".into()
    } else {
        data.0
            .items
            .iter()
            .find(|i| i.entry == id)
            .map_or("Item".into(), |i| match i.class {
                2 => "Weapon".into(),
                4 => format!("Armour · {} armour", i.armor),
                0 => "Consumable".into(),
                _ => "Material".into(),
            })
    }
}
/// After combat input: an attack, evade, guard or incoming reaction cancels the interaction.
fn drive(
    mut commands: Commands,
    (time, real): (Res<Time>, Res<Time<bevy::time::Real>>),
    (keys, pads, unattended, mode, controls): (
        Res<ButtonInput<KeyCode>>,
        Query<&Gamepad>,
        Option<Res<crate::run_mode::Unattended>>,
        Res<crate::camera::CamMode>,
        Controls,
    ),
    kit: Option<Res<interaction::Kit>>,
    mut active: ResMut<Active>,
    mut owned: ResMut<Owned>,
    mut screen: ResMut<Screen>,
    mut candidate: ResMut<Candidate>,
    data: Res<Data>,
    quests: Res<QuestLootEligibility>,
    mut instruments: ResMut<Instruments>,
    mut acquired: MessageWriter<Acquired>,
    mut last_health: Local<Option<f32>>,
    mut avatar: Query<
        (
            &mut Transform,
            &Motor,
            &Action,
            &mut MoveIntent,
            &mut Health,
        ),
        With<Avatar>,
    >,
    targets: Query<
        (
            Entity,
            &Transform,
            Option<&Chest>,
            Option<&Corpse>,
            Option<&crate::npc::Reaction>,
        ),
        Without<Avatar>,
    >,
) {
    let Ok((mut player, motor, action, mut intent, mut health)) = avatar.single_mut() else {
        return;
    };
    let now = real.elapsed_secs();
    let dt = time.delta_secs();
    let hurt = last_health.is_some_and(|hp| health.cur < hp);
    *last_health = Some(health.cur);
    let live = unattended.is_none();
    let inventory_due = instruments.inventory.is_some_and(|s| now >= s);
    if inventory_due {
        instruments.inventory = None;
    }
    let gear_due = instruments.gear.is_some_and(|s| now >= s);
    if gear_due {
        instruments.gear = None;
    }
    // The Pack binding (Tab by default; the menu's Pack entry opens it too) and the Equipment binding
    // (G, the pad's Select): the pack and the equipment page (the paper doll).
    let toggle = inventory_due || (live && controls.just_pressed(BindAction::Pack));
    let gear_toggle = gear_due || (live && controls.just_pressed(BindAction::Equipment));
    if (action.is_dead() || action.owns_body() || hurt) && screen.open {
        screen.open = false;
    }
    if (toggle || gear_toggle)
        && !hurt
        && health.cur > 0.0
        && !action.owns_body()
        && active.0.is_none()
    {
        let target = if gear_toggle {
            crate::equip::Page::Gear
        } else {
            crate::equip::Page::Items
        };
        if !screen.open {
            screen.open = true;
            screen.page = target;
        } else if screen.page == target {
            screen.open = false;
        } else {
            screen.page = target;
        }
        screen.selected = 0;
    }
    if screen.open {
        *intent = MoveIntent::default();
        candidate.0 = None;
        // The equipment page answers a back press itself while its choice list is open.
        if (screen.close_requested
            || keys.just_pressed(KeyCode::Escape)
            || pads.iter().any(|p| p.just_pressed(GamepadButton::East)))
            && !screen.back_taken
        {
            screen.open = false;
            screen.close_requested = false;
        }
        let length = owned.progress.inventory.items.len();
        let on_items = screen.page == crate::equip::Page::Items;
        let down = on_items
            && (keys.just_pressed(KeyCode::ArrowDown)
                || keys.just_pressed(KeyCode::KeyS)
                || pads.iter().any(|p| p.just_pressed(GamepadButton::DPadDown)));
        let up = on_items
            && (keys.just_pressed(KeyCode::ArrowUp)
                || keys.just_pressed(KeyCode::KeyW)
                || pads.iter().any(|p| p.just_pressed(GamepadButton::DPadUp)));
        if length > 0 && (down || up) {
            screen.selected = if crate::hud::theme::enabled() {
                if down {
                    (screen.selected + 5).min(length - 1)
                } else {
                    screen.selected.saturating_sub(5)
                }
            } else if down {
                (screen.selected + 1) % length
            } else {
                (screen.selected + length - 1) % length
            };
        }
        if length > 0 && on_items && crate::hud::theme::enabled() {
            let right = keys.just_pressed(KeyCode::ArrowRight)
                || keys.just_pressed(KeyCode::KeyD)
                || pads
                    .iter()
                    .any(|p| p.just_pressed(GamepadButton::DPadRight));
            let left = keys.just_pressed(KeyCode::ArrowLeft)
                || keys.just_pressed(KeyCode::KeyA)
                || pads.iter().any(|p| p.just_pressed(GamepadButton::DPadLeft));
            if right {
                screen.selected = (screen.selected + 1).min(length - 1);
            }
            if left {
                screen.selected = screen.selected.saturating_sub(1);
            }
        }
    }
    screen.back_taken = false;
    // Enter / A on a wearable item of the list puts it on (the Use button says Equip there).
    let on_items = screen.page == crate::equip::Page::Items;
    let selected_item = ui::ordered(&owned.progress.inventory)
        .get(screen.selected)
        .copied();
    if selected_item
        .and_then(|id| data.0.items.iter().find(|i| i.entry == id))
        .is_some_and(|i| !wow_world::equipment::slots_for(i.inventory_type).is_empty())
        && ((screen.open
            && on_items
            && (keys.just_pressed(KeyCode::Enter)
                || pads.iter().any(|p| p.just_pressed(GamepadButton::South))))
            || screen.use_requested)
    {
        screen.equip_requested = true;
        screen.use_requested = false;
    }
    let use_due = instruments.use_at.is_some_and(|s| now >= s);
    if use_due {
        instruments.use_at = None;
    }
    let use_item = use_due
        || (live && controls.just_pressed(BindAction::QuickUse))
        || (screen.use_requested
            && ui::ordered(&owned.progress.inventory).get(screen.selected) == Some(&SALVE))
        || (screen.open
            && on_items
            && (keys.just_pressed(KeyCode::Enter)
                || pads.iter().any(|p| p.just_pressed(GamepadButton::South)))
            && ui::ordered(&owned.progress.inventory).get(screen.selected) == Some(&SALVE));
    if screen.use_requested {
        screen.use_requested = false;
    }
    if use_item
        && !action.owns_body()
        && active.0.is_none()
        && health.cur > 0.0
        && health.cur < health.max
        && owned.progress.inventory.count(SALVE) > 0
    {
        let mut next = owned.progress.clone();
        if next.inventory.remove(SALVE, 1).is_ok() {
            match owned.publish(next) {
                Ok(()) => {
                    screen.error = None;
                    health.cur = (health.cur + 30.0).min(health.max);
                    screen.notice = format!(
                        "Abbey Salve · health restored\n{} remaining",
                        owned.progress.inventory.count(SALVE)
                    );
                    screen.discovery = false;
                    screen.notice_item = Some(SALVE);
                    screen.notice_items.clear();
                    screen.notice_detail.clear();
                    screen.notice_until = now + 3.0;
                    screen.selected = 0;
                    info!(
                        "loot: salve used health {:.1} remaining {}",
                        health.cur,
                        owned.progress.inventory.count(SALVE)
                    );
                }
                Err(e) => screen.error = Some(e),
            }
        }
    }
    // Baseline chest restock is durable and independent of authored one-time treasure claims.
    let expired: Vec<_> = owned
        .progress
        .chests
        .iter()
        .filter_map(|(&id, s)| match s {
            ChestState::Claimed(at) if *at != u64::MAX && unix() >= *at => Some(id),
            _ => None,
        })
        .collect();
    if !expired.is_empty() {
        let mut next = owned.progress.clone();
        for id in expired {
            next.chests.remove(&id);
        }
        if let Err(e) = owned.publish(next) {
            screen.error = Some(e);
        }
    }
    let ready = !screen.open
        && !hurt
        && *mode == crate::camera::CamMode::Orbit
        && !action.owns_body()
        && motor.grounded
        && health.cur > 0.0;
    if !ready && active.0.is_some() {
        info!("loot: interaction interrupted");
        active.0 = None;
    }
    let mut nearest = None;
    if ready && active.0.is_none() {
        let mut distance = 2.4;
        for (entity, xf, chest, corpse, reaction) in &targets {
            if chest.is_none() && corpse.is_none() {
                continue;
            }
            if corpse.is_some() && reaction != Some(&crate::npc::Reaction::Dead) {
                continue;
            }
            if chest.is_some_and(|c| {
                matches!(
                    owned.progress.chests.get(&c.id),
                    Some(ChestState::Claimed(_))
                )
            }) {
                continue;
            }
            let anchor = if chest.is_some() {
                chest_anchor(xf)
            } else {
                xf.translation + (player.translation - xf.translation).normalize_or_zero() * 0.45
            };
            let d = (anchor - player.translation).length();
            if d < distance {
                distance = d;
                nearest = Some(entity);
            }
        }
    }
    if candidate.0 != nearest {
        candidate.0 = nearest;
    }
    let prompt = nearest.and_then(|e| targets.get(e).ok()).map_or_else(
        String::new,
        |(entity, _, chest, _, _)| {
            let verb = chest.map_or_else(
                || {
                    // Several corpses around: one press takes them all.
                    match area_corpses(player.translation, entity, &targets).len() {
                        0 | 1 => "Collect".to_string(),
                        n => format!("Collect all ({n})"),
                    }
                },
                |c| {
                    if owned.progress.chests.contains_key(&c.id) {
                        "Take items".to_string()
                    } else {
                        "Open chest".to_string()
                    }
                },
            );
            format!("{}   {verb}", controls.binds().hint(BindAction::Interact))
        },
    );
    if screen.prompt != prompt {
        screen.prompt = prompt;
    }
    let scripted = if let Some(s) = instruments.interact {
        let due = instruments.next.unwrap_or(s.at);
        if now >= due && s.until.is_none_or(|u| now < u) {
            instruments.next = Some(s.every.map_or(f32::INFINITY, |e| due + e));
            true
        } else {
            false
        }
    } else {
        false
    };
    let pressed = scripted || (live && controls.just_pressed(BindAction::Interact));
    if ready && active.0.is_none() && pressed {
        if let (Some(target), Some(_)) = (nearest, kit.as_ref()) {
            let (_, _, chest, _, _) = targets.get(target).unwrap();
            let kind = chest.map_or(Kind::Ground, |c| {
                if owned.progress.chests.contains_key(&c.id) {
                    Kind::Take
                } else {
                    Kind::Open
                }
            });
            active.0 = Some(Playing {
                target,
                kind,
                t: -1.0,
                align_age: 0.0,
                committed: false,
            });
        } else if kit.is_none() {
            screen.error = Some("Loot animation cache unavailable".into());
        }
    }
    let (Some(mut play), Some(kit)) = (active.0, kit) else {
        return;
    };
    let Ok((_, xf, chest, corpse, reaction)) = targets.get(play.target) else {
        active.0 = None;
        return;
    };
    if play.kind == Kind::Ground
        && (reaction != Some(&crate::npc::Reaction::Dead) || (!play.committed && corpse.is_none()))
    {
        active.0 = None;
        return;
    }
    let anchor = if chest.is_some() {
        chest_anchor(xf)
    } else {
        let away = (player.translation - xf.translation)
            .with_y(0.0)
            .normalize_or_zero();
        xf.translation + away * 0.45
    };
    let delta = (anchor - player.translation).with_y(0.0);
    let face = (xf.translation - anchor).with_y(0.0);
    if play.t < 0.0 {
        play.align_age += dt;
        *intent = MoveIntent {
            dir: delta.normalize_or_zero(),
            speed: (delta.length() / dt.max(0.001)).min(2.5),
            jump: false,
        };
        if play.align_age > 2.0 {
            active.0 = None;
            return;
        }
        if delta.length() < 0.045 {
            play.t = 0.0;
            *intent = MoveIntent::default();
            info!(
                "loot: start {} at {:?}",
                play.kind.name(),
                benilla_assets::coords::bevy_to_wow(player.translation)
            );
        }
    } else {
        *intent = MoveIntent::default();
        play.t += dt;
        let clip = kit.clip(play.kind);
        if !play.committed && play.t >= clip.commit {
            let mut next = owned.progress.clone();
            // A corpse loot takes every corpse of the area, not only the one the player stands at.
            let area = if play.kind == Kind::Ground {
                area_corpses(player.translation, play.target, &targets)
            } else {
                Vec::new()
            };
            let result: Result<Vec<ItemCount>, String> = (|| match (play.kind, chest, corpse) {
                (Kind::Open, Some(c), _) => {
                    let items = if let Some(reward) = c.reward {
                        reward.to_vec()
                    } else {
                        wow_world::loot::roll_with(
                            &data.0.loot,
                            LootKind::Object,
                            c.table,
                            unix() ^ u64::from(c.id),
                            &quests.0,
                            crate::loot::chances(),
                        )
                        .map_err(|e| e.to_string())?
                    };
                    next.open(c.id, items).map_err(|e| e.to_string())?;
                    Ok(Vec::new())
                }
                (Kind::Take, Some(c), _) => next
                    .claim(
                        c.id,
                        if c.reward.is_some() {
                            u64::MAX
                        } else {
                            unix() + u64::from(c.restock)
                        },
                    )
                    .map_err(|e| e.to_string()),
                (Kind::Ground, _, Some(_)) => {
                    let items = merged(area.iter().map(|t| t.items.as_slice()));
                    next.inventory.add(&items).map_err(|e| e.to_string())?;
                    Ok(items)
                }
                _ => Err("loot target changed".into()),
            })();
            match result.and_then(|items| owned.publish(next).map(|()| items)) {
                Ok(items) => {
                    screen.error = None;
                    play.committed = true;
                    info!("loot: committed {} {:?}", play.kind.name(), items);
                    if play.kind == Kind::Ground {
                        if area.len() > 1 {
                            info!("loot: area loot took {} corpses", area.len());
                        }
                        for taken in &area {
                            commands.entity(taken.glow).despawn();
                            commands.entity(taken.corpse).remove::<Corpse>();
                        }
                    }
                    if play.kind != Kind::Open {
                        acquired.write(Acquired {
                            items,
                            discovery: chest.is_some_and(|c| c.reward.is_some()),
                        });
                    }
                }
                Err(e) => {
                    screen.error = Some(e);
                    active.0 = None;
                    return;
                }
            }
        }
        if play.t >= clip.wow.duration() {
            active.0 = None;
            return;
        }
    }
    if face.length_squared() > 0.001 {
        player.rotation = Quat::from_rotation_y(crate::controller::yaw_of(face));
    }
    active.0 = Some(play);
}

fn animate_chests(
    mut commands: Commands,
    owned: Res<Owned>,
    active: Res<Active>,
    mut chests: Query<(
        Entity,
        &Chest,
        &ModelAnimations,
        &mut AnimationPlayer,
        Option<&mut ChestClip>,
    )>,
) {
    for (entity, chest, anims, mut player, previous) in &mut chests {
        let opening = active.0.filter(|p| {
            p.target == entity && p.kind == Kind::Open && p.t >= interaction::LID_START
        });
        let fraction = opening.map(|p| interaction::lid_fraction(p.t));
        let mut id = if fraction.is_some_and(|f| f < 1.0) {
            148
        } else if owned.progress.chests.contains_key(&chest.id) {
            149
        } else {
            147
        };
        let mut close_from = None;
        if id == 147 {
            if previous.as_ref().is_some_and(|p| p.id == 149) {
                close_from = Some(0.0);
                id = 146;
            } else if previous.as_ref().is_some_and(|p| p.id == 148) {
                if let Some(open) = anims.find(148) {
                    let f = player
                        .animation(open.node)
                        .map_or(0.0, |a| a.seek_time() / open.duration);
                    close_from = Some((1.0 - f).clamp(0.0, 1.0));
                    id = 146;
                }
            } else if previous.as_ref().is_some_and(|p| p.id == 146)
                && anims
                    .find(146)
                    .and_then(|c| player.animation(c.node))
                    .is_some_and(|a| !a.is_finished())
            {
                id = 146;
            }
        }
        let Some(clip) = anims.find(id) else {
            continue;
        };
        if previous.as_ref().is_none_or(|p| p.id != id) {
            player.stop_all();
            player.start(clip.node).set_repeat(if id == 147 {
                RepeatAnimation::Forever
            } else {
                RepeatAnimation::Never
            });
            if let Some(from) = close_from {
                player
                    .animation_mut(clip.node)
                    .unwrap()
                    .set_seek_time(from * clip.duration);
            }
            commands.entity(entity).insert(ChestClip { id });
        }
        if let Some(a) = player.animation_mut(clip.node) {
            if id == 148 {
                a.set_speed(0.0)
                    .set_seek_time(fraction.unwrap_or(0.0) * clip.duration);
            } else {
                a.set_speed(1.0);
            }
        }
    }
}

fn free_hands(
    active: Res<Active>,
    avatar: Query<&crate::actor::held::HeldBuilt, With<Avatar>>,
    mut items: Query<&mut Visibility>,
    mut hidden: Local<Option<(Entity, Visibility)>>,
) {
    if active.playing().is_some() {
        if hidden.is_none() {
            if let Ok(held) = avatar.single() {
                if let Ok(mut v) = items.get_mut(held.root) {
                    *hidden = Some((held.root, *v));
                    *v = Visibility::Hidden;
                }
            }
        }
    } else if let Some((entity, visibility)) = hidden.take() {
        if let Ok(mut v) = items.get_mut(entity) {
            *v = visibility;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemId;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn a_chest_glows_only_while_it_is_open_with_something_in_it() {
        let mut p = Progress::default();
        assert!(!holds_loot(&p, 7), "a closed chest shows nothing");
        p.open(7, vec![(750, 1)]).unwrap();
        assert!(holds_loot(&p, 7), "opened, the items wait in it");
        p.open(8, Vec::new()).unwrap();
        assert!(!holds_loot(&p, 8), "opened empty: nothing to see");
        p.claim(7, u64::MAX).unwrap();
        assert!(!holds_loot(&p, 7), "taken: the sparkles end");
    }

    #[test]
    fn an_area_loot_adds_equal_items_together_in_the_order_first_met() {
        let a: &[ItemCount] = &[(750, 1), (752, 2)];
        let b: &[ItemCount] = &[(752, 1), (118, 1)];
        let c: &[ItemCount] = &[];
        assert_eq!(
            merged([a, b, c, a]),
            vec![(750, 2), (752, 5), (118, 1)],
            "752: 2 + 1 + 2"
        );
        assert_eq!(merged(std::iter::empty()), Vec::new());
    }

    struct Fixture {
        w: bevy::ecs::world::World,
        drive: SystemId,
        player: Entity,
        path: PathBuf,
        pad: Entity,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "wow-loot-flow-{}-{}.txt",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut w = bevy::ecs::world::World::new();
            w.insert_resource(Time::<()>::default());
            w.insert_resource(Time::<bevy::time::Real>::default());
            w.init_resource::<ButtonInput<KeyCode>>();
            w.init_resource::<Active>();
            w.init_resource::<Screen>();
            w.init_resource::<Candidate>();
            w.init_resource::<QuestLootEligibility>();
            w.init_resource::<Instruments>();
            w.init_resource::<Messages<Acquired>>();
            w.insert_resource(crate::camera::CamMode::Orbit);
            // The bindings the systems read; the tests open the pack with the pad's Start (the game's
            // default for that button is the menu, and the pack has no pad default).
            let mut settings = crate::settings::Settings::default();
            settings
                .binds
                .assign_pad(BindAction::Pack, GamepadButton::Start);
            w.insert_resource(settings);
            w.init_resource::<crate::settings::MenuUp>();
            w.init_resource::<ButtonInput<MouseButton>>();
            w.insert_resource(Data(World::default()));
            w.insert_resource(Owned {
                progress: Progress::default(),
                path: path.clone(),
                blocked: None,
            });
            w.insert_resource(interaction::Kit {
                clips: Kind::ALL
                    .into_iter()
                    .map(|kind| {
                        let wow = wscl::Clip {
                            name: kind.source().into(),
                            fps: 1.0,
                            frames: vec![Vec::new(); 3],
                            ..default()
                        };
                        interaction::Clip {
                            kind,
                            reference: wow.clone(),
                            wow,
                            commit: if kind == Kind::Open { 1.2 } else { 1.0 },
                        }
                    })
                    .collect(),
            });
            let mut motor = Motor::default();
            motor.grounded = true;
            let player = w
                .spawn((
                    Avatar,
                    Transform::from_xyz(0.0, 0.0, -1.08),
                    motor,
                    Action::default(),
                    MoveIntent::default(),
                    Health {
                        cur: 50.0,
                        max: 100.0,
                    },
                ))
                .id();
            w.spawn((
                Transform::default(),
                Chest {
                    id: ABBEY_CHEST,
                    reward: Some(&[(SALVE, 3)]),
                    restock: 0,
                    table: 0,
                },
            ));
            let pad = w.spawn(Gamepad::default()).id();
            let drive = w.register_system(super::drive);
            Self {
                w,
                drive,
                player,
                path,
                pad,
            }
        }
        fn frame(&mut self, dt: f32) {
            self.w
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(dt));
            self.w
                .resource_mut::<Time<bevy::time::Real>>()
                .advance_by(std::time::Duration::from_secs_f32(dt));
            self.w.run_system(self.drive).unwrap();
            self.w.resource_mut::<ButtonInput<KeyCode>>().reset_all();
            self.w
                .get_mut::<Gamepad>(self.pad)
                .unwrap()
                .digital_mut()
                .reset_all();
        }
        fn button(&mut self, b: GamepadButton) {
            self.w
                .get_mut::<Gamepad>(self.pad)
                .unwrap()
                .digital_mut()
                .press(b);
            self.frame(0.016);
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(self.path.with_extension("tmp"));
        }
    }
    #[test]
    fn gamepad_opens_then_acquires_once_and_the_saved_reward_survives_reload() {
        let mut f = Fixture::new();
        f.button(GamepadButton::North);
        assert_eq!(f.w.resource::<Active>().0.unwrap().kind, Kind::Open);
        f.frame(1.3);
        assert_eq!(f.w.resource::<Owned>().progress.inventory.count(SALVE), 0);
        assert!(matches!(
            Progress::load(&f.path).unwrap().chests.get(&ABBEY_CHEST),
            Some(ChestState::Open(_))
        ));
        f.frame(1.0);
        f.button(GamepadButton::North);
        assert_eq!(f.w.resource::<Active>().0.unwrap().kind, Kind::Take);
        f.frame(1.1);
        f.frame(1.0);
        f.button(GamepadButton::North);
        f.frame(3.0);
        assert_eq!(Progress::load(&f.path).unwrap().inventory.count(SALVE), 3);
        assert_eq!(f.w.resource::<Messages<Acquired>>().len(), 1);
    }
    #[test]
    fn interrupted_pickup_leaves_open_chest_and_no_item_grant() {
        let mut f = Fixture::new();
        f.button(GamepadButton::North);
        f.frame(1.3);
        f.frame(1.0);
        f.button(GamepadButton::North);
        f.frame(0.4);
        f.w.get_mut::<Health>(f.player).unwrap().cur = 20.0;
        f.frame(0.7);
        assert!(f.w.resource::<Active>().0.is_none());
        let saved = Progress::load(&f.path).unwrap();
        assert!(matches!(
            saved.chests.get(&ABBEY_CHEST),
            Some(ChestState::Open(_))
        ));
        assert_eq!(saved.inventory.count(SALVE), 0);
        assert_eq!(f.w.resource::<Messages<Acquired>>().len(), 0);
    }
    #[test]
    fn every_elwynn_bundle_opens_takes_and_reloads_once_through_the_real_flow() {
        for t in &treasures::TREASURES[3..] {
            let mut f = Fixture::new();
            {
                let mut query = f.w.query::<&mut Chest>();
                let mut chest = query.single_mut(&mut f.w).unwrap();
                chest.id = t.id;
                chest.reward = Some(t.items);
            }
            f.button(GamepadButton::North);
            f.frame(1.3);
            assert_eq!(
                Progress::load(&f.path).unwrap().chests[&t.id],
                ChestState::Open(t.items.to_vec()),
                "{} pending",
                t.name
            );
            f.frame(1.0);
            f.button(GamepadButton::North);
            f.frame(1.1);
            f.frame(1.0);
            let saved = Progress::load(&f.path).unwrap();
            assert_eq!(saved.chests[&t.id], ChestState::Claimed(u64::MAX));
            for &(id, count) in t.items {
                assert_eq!(saved.inventory.count(id), count, "{} item {id}", t.name);
            }
            f.button(GamepadButton::North);
            f.frame(3.0);
            assert_eq!(Progress::load(&f.path).unwrap().text(), saved.text());
            assert_eq!(f.w.resource::<Messages<Acquired>>().len(), 1);
        }
    }
    #[test]
    fn gamepad_inventory_navigation_use_and_back_work_and_hold_movement() {
        let mut f = Fixture::new();
        f.w.resource_mut::<Owned>()
            .progress
            .inventory
            .add(&[(12, 1), (SALVE, 3)])
            .unwrap();
        f.w.get_mut::<MoveIntent>(f.player).unwrap().speed = 7.0;
        f.button(GamepadButton::Start);
        assert!(f.w.resource::<Screen>().open);
        assert_eq!(f.w.get::<MoveIntent>(f.player).unwrap().speed, 0.0);
        f.button(GamepadButton::West);
        assert_eq!(f.w.get::<Health>(f.player).unwrap().cur, 80.0);
        assert_eq!(Progress::load(&f.path).unwrap().inventory.count(SALVE), 2);
        f.button(GamepadButton::DPadDown);
        assert_eq!(f.w.resource::<Screen>().selected, 1);
        f.button(GamepadButton::East);
        assert!(!f.w.resource::<Screen>().open);
    }
    #[test]
    fn failed_save_cannot_consume_an_item_or_heal_the_player() {
        let mut f = Fixture::new();
        f.w.resource_mut::<Owned>()
            .progress
            .inventory
            .add(&[(SALVE, 1)])
            .unwrap();
        std::fs::write(&f.path, "keep").unwrap();
        f.w.resource_mut::<Owned>().path = f.path.join("impossible.txt");
        f.button(GamepadButton::West);
        assert_eq!(f.w.get::<Health>(f.player).unwrap().cur, 50.0);
        assert_eq!(f.w.resource::<Owned>().progress.inventory.count(SALVE), 1);
        assert!(f.w.resource::<Screen>().error.is_some());
        assert_eq!(std::fs::read_to_string(&f.path).unwrap(), "keep");
    }
    #[test]
    fn taking_damage_with_poise_closes_inventory_without_waiting_for_stagger() {
        let mut f = Fixture::new();
        f.button(GamepadButton::Start);
        f.w.get_mut::<Health>(f.player).unwrap().cur = 20.0;
        f.frame(0.1);
        assert!(!f.w.resource::<Screen>().open);
    }
    #[test]
    fn a_quest_screen_holds_actions_not_running_and_swallows_its_own_press() {
        let mut s = Screen::default();
        assert!(!s.locked() && !s.blocks_actions(1.0));
        s.modal = true;
        assert!(!s.locked(), "the player runs and looks about while talking");
        assert!(s.blocks_actions(1.0), "but does not attack, roll or jump");
        // A confirmed the dialogue and closed it: the same press must not also jump in that frame.
        s.modal = false;
        s.swallow_until = 1.25;
        assert!(s.blocks_actions(1.2));
        assert!(!s.blocks_actions(1.3));
        // The inventory holds everything.
        s.open = true;
        assert!(s.locked() && s.blocks_actions(9.0));
    }
    #[test]
    fn one_loot_takes_every_dead_creature_of_the_area_and_no_other() {
        let mut f = Fixture::new();
        let chest =
            f.w.query_filtered::<Entity, With<Chest>>()
                .single(&f.w)
                .unwrap();
        f.w.despawn(chest);
        let corpse = |f: &mut Fixture, at: Vec3, items: Vec<ItemCount>, state| {
            let glow = f.w.spawn_empty().id();
            f.w.spawn((
                Transform::from_translation(at),
                Corpse { items, glow },
                state,
            ))
            .id()
        };
        let dead = crate::npc::Reaction::Dead;
        let near = corpse(&mut f, Vec3::new(0.0, 0.0, -1.53), vec![(755, 1)], dead);
        let aside = corpse(
            &mut f,
            Vec3::new(6.0, 0.0, -1.08),
            vec![(755, 2), (756, 1)],
            dead,
        );
        let far = corpse(&mut f, Vec3::new(14.0, 0.0, -1.08), vec![(757, 1)], dead);
        let alive = corpse(
            &mut f,
            Vec3::new(2.0, 0.0, -1.08),
            vec![(758, 1)],
            crate::npc::Reaction::None,
        );
        f.button(GamepadButton::North);
        for _ in 0..30 {
            f.frame(0.1);
        }
        let held = |f: &Fixture, item| f.w.resource::<Owned>().progress.inventory.count(item);
        assert_eq!(
            [755, 756, 757, 758].map(|i| held(&f, i)),
            [3, 1, 0, 0],
            "the two dead corpses within {AREA_LOOT_RADIUS} yd, summed; not the far one, not the living"
        );
        assert!(f.w.get::<Corpse>(near).is_none() && f.w.get::<Corpse>(aside).is_none());
        assert!(f.w.get::<Corpse>(far).is_some() && f.w.get::<Corpse>(alive).is_some());
        let _ = std::fs::remove_file(&f.path);
    }
    #[test]
    fn revived_corpse_cannot_be_collected_during_an_in_progress_pickup() {
        let mut f = Fixture::new();
        let chest =
            f.w.query_filtered::<Entity, With<Chest>>()
                .single(&f.w)
                .unwrap();
        f.w.despawn(chest);
        let glow = f.w.spawn_empty().id();
        let corpse =
            f.w.spawn((
                Transform::from_xyz(0.0, 0.0, -1.53),
                Corpse {
                    items: vec![(755, 1)],
                    glow,
                },
                crate::npc::Reaction::Dead,
            ))
            .id();
        f.button(GamepadButton::North);
        assert_eq!(f.w.resource::<Active>().0.unwrap().kind, Kind::Ground);
        f.frame(0.4);
        *f.w.get_mut::<crate::npc::Reaction>(corpse).unwrap() = crate::npc::Reaction::None;
        f.frame(0.7);
        assert!(f.w.resource::<Active>().0.is_none());
        assert_eq!(f.w.resource::<Owned>().progress.inventory.count(755), 0);
        assert!(!f.path.exists());
    }
}
