//! Map-only quest seals. Nearby NPCs share a compact group; both ! and ? remain visible.
//! The original overhead M2 signs and their animations are owned by quests/ui.rs.
use super::*;
use wow_world::quest::rules::{marks, Mark};

#[derive(Debug, Clone, PartialEq)]
struct Group {
    uv: Vec2,
    people: Vec<(String, Mark)>,
}
#[derive(Resource, Default)]
struct Cache {
    parent: Option<Entity>,
    groups: Vec<Group>,
}
#[derive(Component)]
struct Pin(usize);
#[derive(Component)]
struct Hint;
#[derive(Component)]
struct HintPanel;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Cache>().add_systems(
        Update,
        refresh
            .after(Input)
            .run_if(in_state(crate::settle::Phase::Playing)),
    );
}

pub(super) fn chrome(commands: &mut Commands, viewport: Entity, font: &Handle<Font>) {
    let legend = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                bottom: px(12),
                padding: UiRect::axes(px(10), px(5)),
                column_gap: px(12),
                align_items: AlignItems::Center,
                ..default()
            },
            ZIndex(10),
            BackgroundColor(theme::INK),
            ChildOf(viewport),
        ))
        .id();
    for (caption, color) in [
        ("! New quest", Color::srgb(0.85, 0.74, 0.46)),
        ("? Return", IVORY),
        ("? In progress", MUTED),
    ] {
        let word = theme::words(commands, legend, caption, font, 11.0, Node::default());
        commands.entity(word).insert(TextColor(color));
    }
    let panel = theme::panel(
        commands,
        Node {
            position_type: PositionType::Absolute,
            left: px(12),
            bottom: px(12),
            max_width: px(350),
            padding: UiRect::axes(px(12), px(8)),
            border: UiRect::bottom(px(0.7)),
            display: Display::None,
            ..default()
        },
        Some(viewport),
    );
    commands.entity(panel).insert((HintPanel, ZIndex(11)));
    let hint = theme::words(commands, panel, "", font, 12.0, Node::default());
    commands.entity(hint).insert(Hint);
}

fn glyphs(group: &Group) -> Vec<(char, Color)> {
    let mut result = Vec::new();
    if group
        .people
        .iter()
        .any(|(_, m)| matches!(m, Mark::Available | Mark::AvailableLow))
    {
        let active = group.people.iter().any(|(_, m)| *m == Mark::Available);
        result.push((
            '!',
            if active {
                Color::srgb(0.85, 0.74, 0.46)
            } else {
                MUTED
            },
        ));
    }
    if group
        .people
        .iter()
        .any(|(_, m)| matches!(m, Mark::Ready | Mark::InProgress))
    {
        let ready = group.people.iter().any(|(_, m)| *m == Mark::Ready);
        result.push((
            '?',
            if ready {
                Color::srgb(0.94, 0.88, 0.69)
            } else {
                MUTED
            },
        ));
    }
    result
}

fn group(points: Vec<(Vec2, String, Mark)>, size: Vec2) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for (uv, name, mark) in points {
        if let Some(near) = groups
            .iter_mut()
            .find(|g| ((g.uv - uv) * size).length() < 25.0)
        {
            let count = near.people.len() as f32;
            near.uv = (near.uv * count + uv) / (count + 1.0);
            near.people.push((name, mark));
        } else {
            groups.push(Group {
                uv,
                people: vec![(name, mark)],
            });
        }
    }
    groups
}

fn spawn(
    commands: &mut Commands,
    parent: Entity,
    font: &Handle<Font>,
    index: usize,
    group: &Group,
    point: Vec2,
) {
    let glyphs = glyphs(group);
    let width = glyphs.len() as f32 * 31.0 - 3.0;
    let root = commands
        .spawn((
            Pin(index),
            Name::new("Map quest seals"),
            ZIndex(4),
            BorderColor::all(GOLD),
            Node {
                column_gap: px(3),
                ..absolute(point.x - width * 0.5, point.y - 37.0, width, 32.0)
            },
            ChildOf(parent),
        ))
        .id();
    for (glyph, color) in glyphs {
        let seal = commands
            .spawn((
                Node {
                    width: px(28),
                    height: px(32),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.045, 0.047, 0.035, 0.94)),
                ChildOf(root),
            ))
            .id();
        commands.spawn((
            theme::Surface::Seal,
            absolute(-1.0, -1.0, 30.0, 34.0),
            ChildOf(seal),
        ));
        let text = theme::words(
            commands,
            seal,
            &glyph.to_string(),
            font,
            24.0,
            Node {
                margin: UiRect::bottom(px(2)),
                ..default()
            },
        );
        commands.entity(text).insert((
            TextColor(color),
            TextShadow {
                offset: Vec2::new(1.0, 1.0),
                color: Color::BLACK,
            },
        ));
    }
    if group.people.len() > 1 {
        let label = group.people.len().to_string();
        let badge = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: px(-4),
                    top: px(-5),
                    width: px((label.len() as f32 * 6.0 + 4.0).max(14.0)),
                    height: px(14),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::MAX,
                    border: UiRect::all(px(0.5)),
                    ..default()
                },
                ZIndex(5),
                BackgroundColor(theme::INK),
                BorderColor::all(theme::EDGE),
                ChildOf(root),
            ))
            .id();
        let count = theme::words(commands, badge, &label, font, 10.0, Node::default());
        commands
            .entity(count)
            .insert(TextLayout::new(Justify::Center, LineBreak::NoWrap));
    }
    commands.spawn((
        absolute(width * 0.5 - 0.5, 32.0, 1.0, 5.0),
        BackgroundColor(GOLD),
        ChildOf(root),
    ));
}

#[allow(clippy::too_many_arguments)]
fn refresh(
    mut commands: Commands,
    menu: Res<SysMenu>,
    state: Res<State>,
    atlas: Res<Atlas>,
    scale: Res<UiScale>,
    world: Option<Res<crate::world::WorldBook>>,
    game: Option<Res<crate::quests::QuestGame>>,
    fonts: Res<crate::loot::ui::Fonts>,
    window: Query<&Window, With<PrimaryWindow>>,
    viewport: Query<Entity, With<Viewport>>,
    mut cache: ResMut<Cache>,
    mut pins: Query<(Entity, &Pin, &mut Node), Without<HintPanel>>,
    mut hint: Query<&mut Text, With<Hint>>,
    mut panels: Query<&mut Node, (With<HintPanel>, Without<Pin>)>,
) {
    if !menu.open || menu.page != Page::Map {
        return;
    }
    let (Ok(window), Ok(parent), Some(area)) = (window.single(), viewport.single(), &atlas.area)
    else {
        return;
    };
    let box_ = canvas(window, scale.0);
    let mut view = state.view;
    view.clamp(box_);
    let mut points = Vec::new();
    if let (Some(world), Some(game)) = (world, game) {
        for spot in &world.book.spots {
            let Some(template) = world.book.templates.get(&spot.entry) else {
                continue;
            };
            if template.flags & 2 == 0 {
                continue;
            }
            let mark = marks(
                &game.book,
                &game.log,
                &game.player,
                wow_world::quest::Npc::Creature(spot.entry),
            );
            if mark == Mark::None {
                continue;
            }
            let uv = project(area, Vec2::new(spot.wow[0], spot.wow[1]));
            if inside(uv) {
                points.push((uv, template.name.clone(), mark));
            }
        }
    }
    let groups = group(points, view.size(box_));
    let cursor = window
        .cursor_position()
        .map(|p| p / scale.0)
        .filter(|p| box_.contains(*p));
    let selected = groups
        .iter()
        .enumerate()
        .filter_map(|(i, g)| {
            let point = view.point(box_, g.uv);
            if !box_.contains(point) {
                return None;
            }
            let half_width = (glyphs(g).len() as f32 * 31.0 - 3.0) * 0.5;
            let hit = Rect::from_corners(
                point - Vec2::new(half_width + 4.0, 41.0),
                point + Vec2::new(half_width + 4.0, 5.0),
            );
            if let Some(cursor) = cursor.filter(|p| hit.contains(*p)) {
                return Some((i, point.distance(cursor)));
            }
            let distance = point.distance(box_.center());
            (distance < 18.0).then_some((i, distance + 100.0))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i);
    for mut text in &mut hint {
        let value = selected
            .map(|i| {
                groups[i]
                    .people
                    .iter()
                    .map(|(name, mark)| {
                        let status = match mark {
                            Mark::Available => "New quest",
                            Mark::AvailableLow => "Low-level quest",
                            Mark::Ready => "Ready to return",
                            Mark::InProgress => "In progress",
                            Mark::None => "",
                        };
                        format!("{name}  -  {status}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if text.0 != value {
            text.0 = value;
        }
    }
    for mut node in &mut panels {
        node.display = if selected.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if cache.parent != Some(parent) || cache.groups != groups {
        for (e, _, _) in &mut pins {
            commands.entity(e).despawn();
        }
        for (i, g) in groups.iter().enumerate() {
            spawn(
                &mut commands,
                parent,
                &fonts.body,
                i,
                g,
                view.point(box_, g.uv) - box_.min,
            );
        }
        cache.parent = Some(parent);
        cache.groups = groups;
        return;
    }
    for (_, pin, mut node) in &mut pins {
        let group = &cache.groups[pin.0];
        let width = glyphs(group).len() as f32 * 31.0 - 3.0;
        let point = view.point(box_, group.uv) - box_.min - Vec2::new(width * 0.5, 37.0);
        node.left = px(point.x);
        node.top = px(point.y);
        node.border = UiRect::all(px(if selected == Some(pin.0) { 0.7 } else { 0.0 }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearby_quests_keep_both_glyphs_and_separate_when_zoomed() {
        let points = vec![
            (Vec2::new(0.5, 0.5), "Offer".into(), Mark::Available),
            (Vec2::new(0.51, 0.5), "Hand-in".into(), Mark::Ready),
            (Vec2::new(0.8, 0.8), "Pending".into(), Mark::InProgress),
        ];
        let groups = group(points.clone(), Vec2::splat(1000.0));
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].people.len(), 2);
        assert_eq!(
            glyphs(&groups[0]).iter().map(|g| g.0).collect::<String>(),
            "!?"
        );
        assert_eq!(glyphs(&groups[1]), vec![('?', MUTED)]);
        assert_eq!(group(points, Vec2::splat(5000.0)).len(), 3);
    }
}
