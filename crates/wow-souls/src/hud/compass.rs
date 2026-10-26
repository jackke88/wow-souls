//! Native heading tape, quest returns, memorials and the player's saved death spot.
mod nav;

use super::theme::{self, GOLD, IVORY};
use crate::avatar::Avatar;
use crate::combat::Health;
use crate::controller::PlaySet;
use crate::friendly::Npc;
use crate::loot::ui::Fonts;
use crate::loot::Screen;
use crate::quests::QuestGame;
use crate::settings::MenuUp;
use crate::settle::Phase;
use crate::world::WorldBook;
use bevy::prelude::*;
use nav::{Edge, WIDTH};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

const SLOTS: usize = 12;
const RULE_Y: f32 = 24.0;

#[derive(Component)]
struct Root;
#[derive(Component, Clone, Copy)]
enum Part {
    Tick(f32),
    Cardinal(f32),
    CardinalInk(f32),
    Marker(usize),
    Glyph(usize),
    MemorialRune(usize),
    PinIcon(usize),
    Arrow(usize),
    ArrowBox(usize),
    CountBox(usize),
    Count(usize),
    Caption,
    CaptionBox,
    Remains,
    RemainsArrow,
    RemainsCaption,
}
#[derive(Resource, Default)]
struct Cache {
    ready: Vec<u32>,
    destinations: Vec<nav::Destination>,
    loaded: bool,
    heading: f32,
    trace_at: f32,
}

fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| theme::enabled() && std::env::var("WOWSOULS_COMPASS").as_deref() != Ok("0"))
}
fn tracing() -> bool {
    static TRACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TRACE.get_or_init(|| std::env::var_os("WOWSOULS_COMPASS_TRACE").is_some())
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Cache>()
        .add_systems(OnEnter(Phase::Playing), spawn.run_if(enabled))
        .add_systems(OnExit(Phase::Playing), cleanup)
        .add_systems(
            Update,
            update
                .after(PlaySet::Camera)
                .run_if(in_state(Phase::Playing))
                .run_if(enabled),
        );
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(x),
        top: px(y),
        width: px(w),
        height: px(h),
        ..default()
    }
}

fn words(
    commands: &mut Commands,
    parent: Entity,
    font: &Handle<Font>,
    text: &str,
    size: f32,
    part: Part,
    node: Node,
) {
    let holder = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..node
            },
            ChildOf(parent),
        ))
        .id();
    let ink = match part {
        Part::Cardinal(angle) => {
            commands.entity(holder).insert(part);
            Part::CardinalInk(angle)
        }
        Part::Arrow(i) => {
            commands.entity(holder).insert(Part::ArrowBox(i));
            part
        }
        Part::Caption => {
            commands.entity(holder).insert(Part::CaptionBox);
            part
        }
        _ => part,
    };
    let id = theme::words(commands, holder, text, font, size, Node::default());
    commands
        .entity(id)
        .insert((ink, TextLayout::new(Justify::Center, LineBreak::NoWrap)));
}

fn spawn(
    mut commands: Commands,
    fonts: Res<Fonts>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
) {
    let rune = images.add(crate::soulbind::rune_texture());
    let pin = images.add(crate::cartography::marker_image());
    let root = commands
        .spawn((
            Name::new("Quest, memorial and remains compass"),
            Root,
            Visibility::Hidden,
            GlobalZIndex(12),
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: px(62),
                margin: UiRect::left(px(-WIDTH * 0.5)),
                width: px(WIDTH),
                height: px(130),
                ..default()
            },
        ))
        .id();
    let fade = |color: Color| {
        let c = color.to_srgba();
        BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(
            LinearGradient::TO_RIGHT,
            vec![
                ColorStop::percent(Color::srgba(c.red, c.green, c.blue, 0.0), 0.0),
                ColorStop::percent(color, 14.0),
                ColorStop::percent(color, 86.0),
                ColorStop::percent(Color::srgba(c.red, c.green, c.blue, 0.0), 100.0),
            ],
        ))])
    };
    commands.spawn((
        rect(0.0, RULE_Y + 0.7, WIDTH, 1.5),
        fade(Color::srgba(0.025, 0.025, 0.02, 0.8)),
        ChildOf(root),
    ));
    commands.spawn((
        rect(0.0, RULE_Y, WIDTH, 0.8),
        fade(GOLD.with_alpha(0.8)),
        ChildOf(root),
    ));
    for i in 0..24 {
        let bearing = (i as f32 * 15.0).to_radians();
        let height = if i % 6 == 0 {
            8.0
        } else if i % 3 == 0 {
            6.0
        } else {
            3.5
        };
        commands.spawn((
            Part::Tick(bearing),
            rect(0.0, RULE_Y - height * 0.5, 0.8, height),
            BackgroundColor(GOLD),
            ChildOf(root),
        ));
    }
    for (i, name) in ["N", "E", "S", "W"].into_iter().enumerate() {
        words(
            &mut commands,
            root,
            &fonts.body,
            name,
            12.5,
            Part::Cardinal(i as f32 * FRAC_PI_2),
            rect(0.0, 1.0, 28.0, 18.0),
        );
    }
    commands.spawn((
        rect(WIDTH * 0.5 - 2.5, RULE_Y - 2.5, 5.0, 5.0),
        UiTransform::from_rotation(Rot2::radians(FRAC_PI_4)),
        BackgroundColor(IVORY),
        ChildOf(root),
    ));
    for i in 0..SLOTS {
        let marker = commands
            .spawn((
                Name::new("Compass destination"),
                Part::Marker(i),
                Node {
                    display: Display::None,
                    ..rect(0.0, RULE_Y + 1.0, 40.0, 40.0)
                },
                ChildOf(root),
            ))
            .id();
        commands.spawn((
            rect(19.6, 0.0, 0.8, 6.0),
            BackgroundColor(GOLD),
            ChildOf(marker),
        ));
        words(
            &mut commands,
            marker,
            &fonts.body,
            "?",
            18.0,
            Part::Glyph(i),
            rect(10.0, 6.0, 20.0, 25.0),
        );
        commands.spawn((
            Part::MemorialRune(i),
            ImageNode::new(rune.clone()),
            Node {
                display: Display::None,
                ..rect(10.0, 6.0, 20.0, 30.0)
            },
            ChildOf(marker),
        ));
        commands.spawn((
            Part::PinIcon(i),
            ImageNode::new(pin.clone()),
            Node {
                display: Display::None,
                ..rect(9.0, 5.0, 22.0, 27.5)
            },
            ChildOf(marker),
        ));
        words(
            &mut commands,
            marker,
            &fonts.body,
            "",
            13.0,
            Part::Arrow(i),
            rect(0.0, 9.0, 14.0, 20.0),
        );
        let badge = commands
            .spawn((
                Part::CountBox(i),
                Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::MAX,
                    border: UiRect::all(px(0.5)),
                    ..rect(27.0, 0.0, 13.0, 13.0)
                },
                BackgroundColor(theme::INK),
                BorderColor::all(theme::EDGE),
                ChildOf(marker),
            ))
            .id();
        words(
            &mut commands,
            badge,
            &fonts.body,
            "",
            9.0,
            Part::Count(i),
            Node::default(),
        );
    }
    words(
        &mut commands,
        root,
        &fonts.body,
        "",
        11.0,
        Part::Caption,
        rect(-50.0, 66.0, WIDTH + 100.0, 18.0),
    );
    // An independent row preserves the death bearing when quest icons overlap it.
    let remains = commands
        .spawn((
            Name::new("Your remains bearing"),
            Part::Remains,
            Node {
                display: Display::None,
                ..rect(0.0, RULE_Y + 40.0, 20.0, 18.0)
            },
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        ImageNode::new(assets.load(benilla_assets::texture_url(
            "Interface\\TargetingFrame\\UI-TargetingFrame-Skull.blp",
            (false, false),
        ))),
        rect(2.0, 0.0, 16.0, 16.0),
        ChildOf(remains),
    ));
    words(
        &mut commands,
        remains,
        &fonts.body,
        "",
        13.0,
        Part::RemainsArrow,
        rect(-14.0, -2.0, 48.0, 20.0),
    );
    words(
        &mut commands,
        root,
        &fonts.body,
        "",
        11.0,
        Part::RemainsCaption,
        rect(-50.0, 85.0, WIDTH + 100.0, 18.0),
    );
}

fn show(node: &mut Mut<Node>, on: bool) {
    let display = if on { Display::Flex } else { Display::None };
    if node.display != display {
        node.display = display;
    }
}
fn left(node: &mut Mut<Node>, x: f32) {
    if node.left != px(x) {
        node.left = px(x);
    }
}
fn text(mut target: Option<Mut<Text>>, value: String) {
    if let Some(target) = target.as_mut() {
        if target.0 != value {
            target.0 = value;
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update(
    time: Res<Time<bevy::time::Real>>,
    menu: Res<MenuUp>,
    screen: Res<Screen>,
    game: Option<Res<QuestGame>>,
    binding: Res<crate::soulbind::Binding>,
    stain: Res<crate::bloodstain::Bloodstain>,
    (journal, atlas, charts): (
        Option<Res<crate::cartography::Journal>>,
        Option<Res<crate::cartography::Atlas>>,
        Option<Res<crate::cartography::Charts>>,
    ),
    world: Option<Res<WorldBook>>,
    camera: Query<&Transform, With<benilla_world::view::WorldCamera>>,
    avatar: Query<(&Transform, &Health), With<Avatar>>,
    npcs: Query<(&Npc, &Transform)>,
    mut cache: ResMut<Cache>,
    mut roots: Query<&mut Visibility, With<Root>>,
    mut pieces: Query<(
        &Part,
        &mut Node,
        Option<&mut Text>,
        Option<&mut TextColor>,
        Option<&mut BackgroundColor>,
        Option<&mut TextShadow>,
        Option<&mut ImageNode>,
    )>,
) {
    let me = avatar.single().ok();
    let camera = camera.single().ok();
    let visible = !menu.0
        && !screen.open
        && !screen.modal
        && me.is_some_and(|(_, hp)| hp.cur > 0.0)
        && camera.is_some();
    for mut root in &mut roots {
        let want = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *root != want {
            *root = want;
        }
    }
    if !visible {
        return;
    }
    let Some((me, _)) = me else { return };
    let Some(camera) = camera else { return };
    if let Some(heading) = nav::heading(camera.rotation * Vec3::NEG_Z) {
        cache.heading = heading;
    }
    let tutorial = crate::quests::soul_tutorial::target(game.as_deref(), &binding).map(|s| s.id);
    match (game, world) {
        (Some(game), Some(world)) => {
            let ready = nav::completed(&game.log);
            if !cache.loaded || cache.ready != ready {
                cache.destinations = nav::destinations(
                    &game.book,
                    &ready,
                    world
                        .book
                        .spots
                        .iter()
                        .map(|s| (s.entry, benilla_assets::coords::wow_to_bevy(s.wow))),
                );
                cache.ready = ready;
                cache.loaded = true;
                info!(
                    "compass: {} ready quest(s), {} return destination(s)",
                    cache.ready.len(),
                    cache.destinations.len()
                );
            }
        }
        _ => {
            cache.loaded = false;
            cache.ready.clear();
            cache.destinations.clear();
        }
    }
    let live: Vec<_> = npcs
        .iter()
        .map(|(npc, xf)| (npc.entry, xf.translation))
        .collect();
    let mut destinations = cache.destinations.clone();
    destinations.extend(nav::memorials(&binding, tutorial, me.translation));
    destinations.extend(nav::marker(crate::cartography::marker_world_point(
        journal.as_deref(),
        atlas.as_deref(),
        charts.as_deref(),
    )));
    let targets = nav::targets(&destinations, me.translation, cache.heading, &live);
    let remains = nav::remains(&stain).and_then(|destination| {
        nav::targets(&[destination], me.translation, cache.heading, &[])
            .into_iter()
            .next()
    });
    let groups = nav::groups(&targets);
    let focus = nav::focus(&targets);
    let caption = focus
        .map(|i| {
            let target = &targets[i];
            let dest = &destinations[target.destination];
            if matches!(dest.kind, nav::Kind::Marker) {
                format!("{} - {:.0} yd", dest.name, target.distance)
            } else if let nav::Kind::Memorial { bound, .. } = dest.kind {
                let action = if bound { "Bound" } else { "Bind Soul" };
                format!("{} - {action} - {:.0} yd", dest.name, target.distance)
            } else if target.distance <= crate::quests::TALK_RANGE {
                format!("{} - Ready to return", dest.name)
            } else if dest.quests > 1 {
                format!(
                    "{} ({} quests) - {:.0} yd",
                    dest.name, dest.quests, target.distance
                )
            } else {
                format!("{} - {:.0} yd", dest.name, target.distance)
            }
        })
        .unwrap_or_default();
    for (part, mut node, label, ink, background, shadow, image) in &mut pieces {
        match *part {
            Part::Remains => {
                show(&mut node, remains.is_some());
                if let Some(target) = &remains {
                    left(&mut node, nav::projection(target.angle).0 - 10.0);
                }
            }
            Part::RemainsArrow => text(
                label,
                match remains.as_ref().map(|t| nav::projection(t.angle).1) {
                    Some(Edge::Left) => "<     ",
                    Some(Edge::Right) => "     >",
                    _ => "",
                }
                .into(),
            ),
            Part::RemainsCaption => text(
                label,
                remains.as_ref().map_or_else(String::new, |t| {
                    format!("Your remains - {:.0} yd", t.distance)
                }),
            ),
            Part::Tick(bearing) | Part::Cardinal(bearing) => {
                let angle = nav::delta(bearing, cache.heading);
                let on = angle.abs() < FRAC_PI_2 - 1e-5;
                show(&mut node, on);
                if !on {
                    continue;
                }
                let x = WIDTH * (0.5 + angle / std::f32::consts::PI);
                let fade = (x.min(WIDTH - x) / 28.0).clamp(0.0, 1.0);
                if matches!(part, Part::Tick(_)) {
                    left(&mut node, x - 0.4);
                    if let Some(mut color) = background {
                        let want = GOLD.with_alpha(fade * 0.75);
                        if color.0 != want {
                            color.0 = want;
                        }
                    }
                } else {
                    left(&mut node, x - 14.0);
                }
            }
            Part::CardinalInk(bearing) => {
                let angle = nav::delta(bearing, cache.heading);
                let x = WIDTH * (0.5 + angle / std::f32::consts::PI);
                let fade = (x.min(WIDTH - x) / 28.0).clamp(0.0, 1.0);
                if let Some(mut color) = ink {
                    let want = IVORY.with_alpha(fade * 0.88);
                    if color.0 != want {
                        color.0 = want;
                    }
                }
                if let Some(mut shadow) = shadow {
                    let want = Color::srgba(0.0, 0.0, 0.0, 0.95 * fade);
                    if shadow.color != want {
                        shadow.color = want;
                    }
                }
            }
            Part::CaptionBox => {
                let top = px(if remains.is_some() { 105.0 } else { 66.0 });
                if node.top != top {
                    node.top = top;
                }
            }
            Part::Caption => text(label, caption.clone()),
            Part::Marker(i) => {
                show(&mut node, groups.get(i).is_some());
                if let Some(group) = groups.get(i) {
                    left(&mut node, group.x - 20.0);
                }
            }
            Part::Glyph(i) | Part::MemorialRune(i) | Part::PinIcon(i) => {
                // Keep the tutorial question mark legible even when its bearing overlaps
                // another destination. Otherwise the nearest member leads the group.
                let kind = groups.get(i).and_then(|group| {
                    group
                        .members
                        .iter()
                        .map(|&i| destinations[targets[i].destination].kind)
                        .find(|kind| matches!(kind, nav::Kind::Memorial { tutorial: true, .. }))
                        .or_else(|| {
                            group
                                .members
                                .first()
                                .map(|&i| destinations[targets[i].destination].kind)
                        })
                });
                let rune = matches!(
                    kind,
                    Some(nav::Kind::Memorial {
                        tutorial: false,
                        ..
                    })
                );
                let pin = matches!(kind, Some(nav::Kind::Marker));
                let visible = match *part {
                    Part::PinIcon(_) => pin,
                    Part::MemorialRune(_) => rune,
                    Part::Glyph(_) => kind.is_some() && !rune && !pin,
                    _ => false,
                };
                show(&mut node, visible);
                let focused = groups
                    .get(i)
                    .is_some_and(|g| focus.is_some_and(|f| g.members.contains(&f)));
                if let Some(mut image) = image {
                    let want = if matches!(part, Part::PinIcon(_)) {
                        if focused {
                            Color::WHITE
                        } else {
                            Color::srgb(0.92, 0.90, 0.84)
                        }
                    } else if matches!(kind, Some(nav::Kind::Memorial { bound: true, .. })) {
                        Color::srgb(0.82, 1.0, 0.91)
                    } else {
                        IVORY
                    };
                    if image.color != want {
                        image.color = want;
                    }
                }
                if let Some(mut color) = ink {
                    let want = if focused { IVORY } else { GOLD };
                    if color.0 != want {
                        color.0 = want;
                    }
                }
            }
            Part::Arrow(i) => {
                let edge = groups.get(i).map_or(Edge::None, |g| g.edge);
                text(
                    label,
                    match edge {
                        Edge::Left => "<",
                        Edge::Right => ">",
                        Edge::None => "",
                    }
                    .into(),
                );
            }
            Part::ArrowBox(i) => {
                let edge = groups.get(i).map_or(Edge::None, |g| g.edge);
                left(&mut node, if edge == Edge::Left { -9.0 } else { 36.0 });
            }
            Part::CountBox(i) => {
                let count = groups.get(i).map_or(0, |g| g.count);
                show(&mut node, count > 1);
                if count > 1 {
                    let width = px(((count.ilog10() + 1) as f32 * 5.5 + 3.0).max(13.0));
                    if node.width != width {
                        node.width = width;
                    }
                }
            }
            Part::Count(i) => text(
                label,
                groups
                    .get(i)
                    .map_or_else(String::new, |g| g.count.to_string()),
            ),
        }
    }
    if tracing() && time.elapsed_secs() >= cache.trace_at {
        info!(
            "compass: heading {:.1} deg | {} targets / {} groups | {}",
            cache.heading.to_degrees(),
            targets.len(),
            groups.len(),
            caption
        );
        for target in &targets {
            let dest = &destinations[target.destination];
            let (x, edge) = nav::projection(target.angle);
            info!(
                "compass: {:?} bearing {:+.1} deg x {x:.1} {edge:?} distance {:.1} yd",
                dest.kind,
                target.angle.to_degrees(),
                target.distance
            );
        }
        if let Some(target) = &remains {
            info!(
                "compass: Remains bearing {:+.1} deg distance {:.1} yd",
                target.angle.to_degrees(),
                target.distance
            );
        }
        cache.trace_at = time.elapsed_secs() + 1.0;
    }
}

fn cleanup(mut commands: Commands, roots: Query<Entity, With<Root>>, mut cache: ResMut<Cache>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    *cache = Cache::default();
}
