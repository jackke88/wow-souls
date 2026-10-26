//! Chart controls share the system menu's pause, cursor and gameplay-input ownership.
mod pins;
use super::{art, project, Atlas, Journal, Marker};
use crate::hud::theme::{self, GOLD, IVORY, MUTED};
use crate::settings::{binds::Action as Bind, MenuUp, Settings};
use crate::sysmenu::{Page, SysMenu};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Input;
#[derive(Debug, Clone, Copy)]
enum Act {
    Open,
    Close,
    Zoom(f32),
    Pan(Vec2),
    Place,
    Point(Vec2),
    Clear,
    Center,
}
#[derive(Component, Clone, Copy)]
struct Click(Act);
#[derive(Component)]
struct Root;
#[derive(Component)]
struct Viewport;
#[derive(Component)]
enum Piece {
    Chart,
    Player,
    Marker,
    Crosshair,
}
#[derive(Component)]
enum Words {
    Zoom,
    Note,
    Title,
}
#[derive(Resource, Default)]
struct Script {
    steps: Vec<(f32, Act)>,
    next: usize,
}
#[derive(Resource, Default)]
struct State {
    view: View,
    was_open: bool,
    drag: Option<Drag>,
}
struct Drag {
    start: Vec2,
    last: Vec2,
    moved: bool,
}

#[derive(Debug, Clone, Copy)]
struct View {
    center: Vec2,
    zoom: f32,
}
impl Default for View {
    fn default() -> Self {
        Self {
            center: Vec2::splat(0.5),
            zoom: 1.0,
        }
    }
}
fn canvas(window: &Window, ui_scale: f32) -> Rect {
    let size = Vec2::new(window.width(), window.height()) / ui_scale;
    Rect::from_corners(
        Vec2::new(24.0, 78.0),
        (size - Vec2::new(24.0, 80.0)).max(Vec2::new(25.0, 79.0)),
    )
}
impl View {
    fn size(self, canvas: Rect) -> Vec2 {
        let image = Vec2::new(art::WIDTH as f32, art::HEIGHT as f32);
        image * (canvas.size() / image).min_element() * self.zoom
    }
    fn origin(self, canvas: Rect) -> Vec2 {
        canvas.center() - self.center * self.size(canvas)
    }
    fn point(self, canvas: Rect, uv: Vec2) -> Vec2 {
        self.origin(canvas) + uv * self.size(canvas)
    }
    fn uv(self, canvas: Rect, point: Vec2) -> Option<Vec2> {
        let uv = (point - self.origin(canvas)) / self.size(canvas);
        (canvas.contains(point) && inside(uv)).then_some(uv)
    }
    fn clamp(&mut self, canvas: Rect) {
        let half = (canvas.size() / self.size(canvas) * 0.5).min(Vec2::splat(0.5));
        self.center = self.center.clamp(half, Vec2::ONE - half);
    }
    fn zoom(&mut self, canvas: Rect, steps: f32, anchor: Vec2) {
        let uv = self.uv(canvas, anchor).unwrap_or(self.center);
        self.zoom = (self.zoom * 1.25_f32.powf(steps.clamp(-5.0, 5.0))).clamp(1.0, 5.0);
        self.center = uv - (anchor - canvas.center()) / self.size(canvas);
        self.clamp(canvas);
    }
}
fn inside(uv: Vec2) -> bool {
    uv.is_finite() && uv.cmpge(Vec2::ZERO).all() && uv.cmple(Vec2::ONE).all()
}

fn parse_script(text: &str) -> Vec<(f32, Act)> {
    let number = |s: &str| s.parse::<f32>().ok().filter(|v| v.is_finite());
    let mut steps: Vec<_> = text
        .split(',')
        .filter_map(|s| {
            let (time, cmd) = s.trim().split_once(':')?;
            let fields: Vec<_> = cmd.split(':').collect();
            let act = match fields.as_slice() {
                ["open"] => Act::Open,
                ["close"] => Act::Close,
                ["mark"] => Act::Place,
                ["clear"] => Act::Clear,
                ["center"] => Act::Center,
                ["zoom", n] => Act::Zoom(number(n)?),
                ["pan", u, v] => Act::Pan(Vec2::new(number(u)?, number(v)?)),
                ["point", u, v] => Act::Point(Vec2::new(number(u)?, number(v)?)),
                _ => return None,
            };
            Some((number(time)?, act))
        })
        .collect();
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    steps
}

pub(super) fn plugin(app: &mut App) {
    pins::plugin(app);
    app.init_resource::<State>()
        .insert_resource(Script {
            steps: std::env::var("WOWSOULS_MAP_SCRIPT")
                .map(|s| parse_script(&s))
                .unwrap_or_default(),
            next: 0,
        })
        .add_systems(OnEnter(crate::settle::Phase::Playing), spawn)
        .add_systems(OnExit(crate::settle::Phase::Playing), cleanup)
        .add_systems(
            Update,
            input
                .in_set(Input)
                .after(crate::sysmenu::ui::MenuInput)
                .before(crate::controller::PlaySet::Intent)
                .run_if(in_state(crate::settle::Phase::Playing)),
        )
        .add_systems(
            Update,
            refresh
                .after(Input)
                .after(super::follow_area)
                .run_if(in_state(crate::settle::Phase::Playing)),
        );
}

fn absolute(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(left),
        top: px(top),
        width: px(width),
        height: px(height),
        ..default()
    }
}
fn toolbar(commands: &mut Commands, parent: Entity, font: &Handle<Font>, caption: &str, act: Act) {
    let id = commands
        .spawn((
            Button,
            Click(act),
            Node {
                height: px(34),
                min_width: px(36),
                padding: UiRect::horizontal(px(12)),
                border: UiRect::bottom(px(0.7)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor::all(theme::EDGE),
            BackgroundColor(Color::srgba(0.05, 0.05, 0.04, 0.7)),
            ChildOf(parent),
        ))
        .id();
    theme::words(commands, id, caption, font, 15.0, Node::default());
}
fn spawn(
    mut commands: Commands,
    atlas: Res<Atlas>,
    fonts: Res<crate::loot::ui::Fonts>,
    mut images: ResMut<Assets<Image>>,
) {
    let root = commands
        .spawn((
            Root,
            Name::new("Area map"),
            Visibility::Hidden,
            GlobalZIndex(1000),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(Color::srgb(0.055, 0.060, 0.050)),
        ))
        .id();
    let header = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(28),
                right: px(28),
                top: px(19),
                height: px(40),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let title = theme::words(
        &mut commands,
        header,
        &atlas.name,
        &fonts.body,
        28.0,
        Node::default(),
    );
    commands.entity(title).insert(Words::Title);
    let actions = commands
        .spawn((
            Node {
                column_gap: px(8),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(header),
        ))
        .id();
    for (text, act) in [
        ("−", Act::Zoom(-1.0)),
        ("+", Act::Zoom(1.0)),
        ("Find player", Act::Center),
        ("Remove marker", Act::Clear),
        ("Close  [Esc]", Act::Close),
    ] {
        toolbar(&mut commands, actions, &fonts.body, text, act);
    }
    let viewport = commands
        .spawn((
            Viewport,
            Node {
                position_type: PositionType::Absolute,
                left: px(24),
                right: px(24),
                top: px(78),
                bottom: px(80),
                overflow: Overflow::clip(),
                border: UiRect::all(px(0.8)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.105, 0.103, 0.084)),
            BorderColor::all(theme::EDGE),
            ChildOf(root),
        ))
        .id();
    if let Some(image) = &atlas.image {
        commands.spawn((
            Piece::Chart,
            ImageNode::new(image.clone()),
            absolute(0.0, 0.0, 1.0, 1.0),
            ChildOf(viewport),
        ));
    } else {
        theme::words(
            &mut commands,
            viewport,
            &atlas.error,
            &fonts.body,
            20.0,
            Node {
                margin: UiRect::all(px(32)),
                ..default()
            },
        );
    }
    let player = commands
        .spawn((
            Piece::Player,
            ZIndex(6),
            absolute(0.0, 0.0, 20.0, 20.0),
            BackgroundColor(Color::srgba(0.07, 0.06, 0.04, 0.95)),
            BorderColor::all(IVORY),
            ChildOf(viewport),
        ))
        .id();
    commands.entity(player).insert(Node {
        border: UiRect::all(px(2)),
        border_radius: BorderRadius::MAX,
        ..absolute(0.0, 0.0, 20.0, 20.0)
    });
    commands.spawn((
        Node {
            border_radius: BorderRadius::MAX,
            ..absolute(5.0, 5.0, 6.0, 6.0)
        },
        BackgroundColor(IVORY),
        ChildOf(player),
    ));
    theme::words(
        &mut commands,
        player,
        "You",
        &fonts.body,
        12.0,
        Node {
            position_type: PositionType::Absolute,
            left: px(24),
            top: px(-2),
            ..default()
        },
    );
    commands.spawn((
        Piece::Marker,
        ZIndex(7),
        ImageNode::new(images.add(art::marker_image())),
        absolute(0.0, 0.0, 28.0, 35.0),
        ChildOf(viewport),
    ));
    let cursor = commands
        .spawn((
            Piece::Crosshair,
            ZIndex(8),
            absolute(0.0, 0.0, 28.0, 28.0),
            ChildOf(viewport),
        ))
        .id();
    for (x, y, w, h) in [
        (0.0, 13.0, 8.0, 1.0),
        (20.0, 13.0, 8.0, 1.0),
        (13.0, 0.0, 1.0, 8.0),
        (13.0, 20.0, 1.0, 8.0),
    ] {
        commands.spawn((
            absolute(x, y, w, h),
            BackgroundColor(IVORY),
            ChildOf(cursor),
        ));
    }
    let compass = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(43),
                top: px(96),
                width: px(56),
                height: px(56),
                border: UiRect::all(px(0.7)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(GOLD),
            BackgroundColor(Color::srgba(0.05, 0.05, 0.04, 0.88)),
            ChildOf(root),
        ))
        .id();
    for (text, x, y) in [
        ("N", 23.0, 1.0),
        ("E", 43.0, 21.0),
        ("S", 23.0, 40.0),
        ("W", 2.0, 21.0),
    ] {
        theme::words(
            &mut commands,
            compass,
            text,
            &fonts.body,
            11.0,
            absolute(x, y, 12.0, 14.0),
        );
    }
    let footer = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(28),
                right: px(28),
                bottom: px(14),
                height: px(52),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    theme::words(&mut commands, footer, "Wheel / + − / L2 R2: Zoom     Drag / arrows / left stick: Pan     Click / Enter / A: Marker", &fonts.body, 12.0, Node::default());
    theme::words(
        &mut commands,
        footer,
        "R / Y: Find player     Delete / X: Remove marker     Esc / B: Close",
        &fonts.body,
        12.0,
        Node::default(),
    );
    let zoom = theme::words(
        &mut commands,
        root,
        "",
        &fonts.body,
        13.0,
        Node {
            position_type: PositionType::Absolute,
            right: px(30),
            bottom: px(34),
            ..default()
        },
    );
    commands.entity(zoom).insert(Words::Zoom);
    let note = theme::words(
        &mut commands,
        root,
        "",
        &fonts.body,
        12.0,
        Node {
            position_type: PositionType::Absolute,
            right: px(30),
            bottom: px(15),
            ..default()
        },
    );
    commands
        .entity(note)
        .insert((Words::Note, TextColor(MUTED)));
    pins::chrome(&mut commands, viewport, &fonts.body);
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn input(
    window: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    scale: Res<UiScale>,
    real: Res<Time<Real>>,
    (keys, mouse): (Res<ButtonInput<KeyCode>>, Res<ButtonInput<MouseButton>>),
    pads: Query<&Gamepad>,
    mut wheel: MessageReader<MouseWheel>,
    settings: Res<Settings>,
    mut menu: ResMut<SysMenu>,
    mut up: ResMut<MenuUp>,
    mut screen: ResMut<crate::loot::Screen>,
    mut state: ResMut<State>,
    (mut journal, atlas): (ResMut<Journal>, Res<Atlas>),
    avatar: Query<&Transform, With<crate::avatar::Avatar>>,
    mut script: ResMut<Script>,
    unattended: Option<Res<crate::run_mode::Unattended>>,
    buttons: Query<(&Click, &Interaction), Changed<Interaction>>,
) {
    let Ok((window, cursor)) = window.single() else {
        return;
    };
    let box_ = canvas(window, scale.0);
    let player = avatar
        .single()
        .ok()
        .and_then(|t| {
            atlas
                .area
                .as_ref()
                .map(|a| project(a, Vec2::new(-t.translation.z, -t.translation.x)))
        })
        .filter(|p| inside(*p));
    let was_open = menu.open && menu.page == Page::Map;
    let fresh = was_open && !state.was_open;
    let live = unattended.is_none();
    let pad = |b| pads.iter().any(|p| p.just_pressed(b));
    let mut actions = Vec::new();
    while let Some(&(t, act)) = script.steps.get(script.next) {
        if t > real.elapsed_secs() {
            break;
        }
        script.next += 1;
        actions.push(act);
    }
    if live
        && settings.binds.just_pressed_in(
            Bind::Map,
            &keys,
            &mouse,
            &pads,
            cursor.grab_mode == CursorGrabMode::Locked,
        )
    {
        actions.push(if was_open { Act::Close } else { Act::Open });
    }
    if was_open && !fresh {
        actions.extend(
            buttons
                .iter()
                .filter(|(_, i)| **i == Interaction::Pressed)
                .map(|(b, _)| b.0),
        );
        if live {
            if keys.just_pressed(KeyCode::Escape)
                || pad(GamepadButton::East)
                || pad(GamepadButton::Start)
            {
                actions.push(Act::Close);
            }
            if keys.just_pressed(KeyCode::Enter) || pad(GamepadButton::South) {
                actions.push(Act::Place);
            }
            if keys.just_pressed(KeyCode::Delete)
                || keys.just_pressed(KeyCode::Backspace)
                || pad(GamepadButton::West)
            {
                actions.push(Act::Clear);
            }
            if keys.just_pressed(KeyCode::KeyR) || pad(GamepadButton::North) {
                actions.push(Act::Center);
            }
            if keys.just_pressed(KeyCode::Equal)
                || keys.just_pressed(KeyCode::NumpadAdd)
                || pad(GamepadButton::RightTrigger2)
            {
                actions.push(Act::Zoom(1.0));
            }
            if keys.just_pressed(KeyCode::Minus)
                || keys.just_pressed(KeyCode::NumpadSubtract)
                || pad(GamepadButton::LeftTrigger2)
            {
                actions.push(Act::Zoom(-1.0));
            }
            let mut pan = Vec2::new(
                f32::from(keys.pressed(KeyCode::ArrowRight))
                    - f32::from(keys.pressed(KeyCode::ArrowLeft)),
                f32::from(keys.pressed(KeyCode::ArrowDown))
                    - f32::from(keys.pressed(KeyCode::ArrowUp)),
            );
            for p in &pads {
                let stick = Vec2::new(
                    p.get(GamepadAxis::LeftStickX).unwrap_or(0.0),
                    -p.get(GamepadAxis::LeftStickY).unwrap_or(0.0),
                );
                if stick.length() > 0.18 {
                    pan += stick;
                }
            }
            if pan != Vec2::ZERO {
                let delta = pan.clamp_length_max(1.0) * real.delta_secs().min(0.1) * 400.0
                    / state.view.size(box_);
                actions.push(Act::Pan(delta));
            }
            let pointer = window.cursor_position().map(|p| p / scale.0);
            if let Some(p) = pointer {
                if mouse.just_pressed(MouseButton::Left) && state.view.uv(box_, p).is_some() {
                    state.drag = Some(Drag {
                        start: p,
                        last: p,
                        moved: false,
                    });
                }
                if mouse.pressed(MouseButton::Left) {
                    if let Some(mut drag) = state.drag.take() {
                        drag.moved |= p.distance(drag.start) > 4.0;
                        if drag.moved {
                            actions.push(Act::Pan(-(p - drag.last) / state.view.size(box_)));
                        }
                        drag.last = p;
                        state.drag = Some(drag);
                    }
                }
                if mouse.just_released(MouseButton::Left) {
                    if let Some(drag) = state.drag.take().filter(|d| !d.moved) {
                        if let Some(uv) = state.view.uv(box_, drag.start) {
                            actions.push(Act::Point(uv));
                        }
                    }
                }
            }
            for ev in wheel.read() {
                let steps = if ev.unit == MouseScrollUnit::Pixel {
                    ev.y / 80.0
                } else {
                    ev.y
                };
                state.view.zoom(
                    box_,
                    steps,
                    pointer
                        .filter(|p| box_.contains(*p))
                        .unwrap_or(box_.center()),
                );
            }
        }
    } else {
        wheel.clear();
    }
    for act in actions {
        if matches!(act, Act::Open) {
            if !screen.open
                && !screen.modal
                && (!menu.open || menu.page == Page::Root)
                && menu.capture.is_none()
            {
                menu.show_map();
                up.0 = true;
            }
            continue;
        }
        if !menu.open || menu.page != Page::Map {
            continue;
        }
        match act {
            Act::Close => {
                menu.hide();
                up.0 = false;
                screen.swallow_until = real.elapsed_secs() + 0.25;
                state.drag = None;
                info!("map: closed");
            }
            Act::Zoom(n) => state.view.zoom(box_, n, box_.center()),
            Act::Pan(delta) => {
                state.view.center += delta;
                state.view.clamp(box_);
            }
            Act::Center => {
                if let Some(p) = player {
                    state.view.center = p;
                    state.view.clamp(box_);
                }
            }
            Act::Place => {
                if let Some(a) = &atlas.area {
                    journal.set(Some(Marker {
                        area: a.area_id,
                        uv: state.view.center,
                    }));
                }
            }
            Act::Point(uv) if inside(uv) => {
                if let Some(a) = &atlas.area {
                    journal.set(Some(Marker {
                        area: a.area_id,
                        uv,
                    }));
                }
            }
            Act::Clear => journal.set(None),
            _ => {}
        }
    }
    let on = menu.open && menu.page == Page::Map;
    if on && !state.was_open {
        state.view = View {
            center: player.unwrap_or(Vec2::splat(0.5)),
            zoom: 1.0,
        };
        state.view.clamp(box_);
        state.drag = None;
        info!("map: opened");
    }
    state.was_open = on;
}

#[allow(clippy::too_many_arguments)]
fn refresh(
    menu: Res<SysMenu>,
    atlas: Res<Atlas>,
    journal: Res<Journal>,
    state: Res<State>,
    scale: Res<UiScale>,
    window: Query<&Window, With<PrimaryWindow>>,
    avatar: Query<&Transform, With<crate::avatar::Avatar>>,
    mut roots: Query<&mut Visibility, With<Root>>,
    mut pieces: Query<(&Piece, &mut Node)>,
    mut text: Query<(&Words, &mut Text)>,
    mut chart_images: Query<(&Piece, &mut ImageNode)>,
    mut buttons: Query<(&Interaction, &mut BorderColor), With<Click>>,
) {
    let on = menu.open && menu.page == Page::Map;
    for mut vis in &mut roots {
        *vis = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if !on {
        return;
    }
    if let Some(image) = &atlas.image {
        for (piece, mut node) in &mut chart_images {
            if matches!(piece, Piece::Chart) && node.image != *image {
                node.image = image.clone();
            }
        }
    }
    let Ok(window) = window.single() else { return };
    let box_ = canvas(window, scale.0);
    let mut view = state.view;
    view.clamp(box_);
    let player = avatar
        .single()
        .ok()
        .and_then(|t| {
            atlas
                .area
                .as_ref()
                .map(|a| project(a, Vec2::new(-t.translation.z, -t.translation.x)))
        })
        .filter(|p| inside(*p));
    for (piece, mut node) in &mut pieces {
        let (point, size, showing) = match piece {
            Piece::Chart => (view.origin(box_), view.size(box_), true),
            Piece::Player => (
                view.point(box_, player.unwrap_or_default()) - Vec2::splat(10.0),
                Vec2::splat(20.0),
                player.is_some(),
            ),
            Piece::Marker => {
                let marker = journal
                    .marker
                    .filter(|m| atlas.area.as_ref().is_some_and(|a| m.area == a.area_id));
                (
                    view.point(box_, marker.map_or(Vec2::ZERO, |m| m.uv)) - Vec2::new(14.0, 32.5),
                    Vec2::new(28.0, 35.0),
                    marker.is_some(),
                )
            }
            Piece::Crosshair => (
                box_.center() - Vec2::splat(14.0),
                Vec2::splat(28.0),
                atlas.area.is_some(),
            ),
        };
        let point = point - box_.min;
        let want = (
            px(point.x),
            px(point.y),
            px(size.x),
            px(size.y),
            if showing {
                Display::Flex
            } else {
                Display::None
            },
        );
        if (node.left, node.top, node.width, node.height, node.display) != want {
            node.left = want.0;
            node.top = want.1;
            node.width = want.2;
            node.height = want.3;
            node.display = want.4;
        }
    }
    for (what, mut text) in &mut text {
        let value = match what {
            Words::Zoom => format!("{:.1}×", view.zoom),
            Words::Note => journal.note.clone(),
            Words::Title => atlas.name.clone(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    for (interaction, mut border) in &mut buttons {
        *border = BorderColor::all(if *interaction == Interaction::None {
            theme::EDGE
        } else {
            IVORY
        });
    }
}
fn cleanup(mut commands: Commands, roots: Query<Entity, With<Root>>, mut state: ResMut<State>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    *state = State::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_anchor_round_trip_and_pan_bounds_survive_all_window_aspects() {
        for size in [
            Vec2::new(1232.0, 562.0),
            Vec2::new(1660.0, 562.0),
            Vec2::new(800.0, 600.0),
        ] {
            let box_ = Rect::from_corners(Vec2::new(24.0, 78.0), Vec2::new(24.0, 78.0) + size);
            let mut view = View {
                zoom: 3.0,
                ..default()
            };
            let uv = Vec2::new(0.6, 0.55);
            let point = view.point(box_, uv);
            assert!(view.uv(box_, point).unwrap().distance(uv) < 1e-5);
            view.zoom(box_, 2.0, point);
            assert!(view.uv(box_, point).unwrap().distance(uv) < 1e-5);
            for center in [Vec2::splat(-500.0), Vec2::splat(500.0)] {
                view.center = center;
                view.clamp(box_);
                let origin = view.origin(box_);
                let end = origin + view.size(box_);
                for axis in 0..2 {
                    if view.size(box_)[axis] >= size[axis] {
                        assert!(
                            origin[axis] <= box_.min[axis] + 0.01
                                && end[axis] >= box_.max[axis] - 0.01
                        );
                    }
                }
            }
            view.zoom(box_, 100.0, box_.center());
            assert!(view.zoom <= 5.0);
            for _ in 0..20 {
                view.zoom(box_, -5.0, box_.center());
            }
            assert_eq!(view.zoom, 1.0);
            assert_eq!(view.center, Vec2::splat(0.5));
        }
    }
    #[test]
    fn scripted_map_inputs_are_sorted_and_nonfinite_values_are_dropped() {
        let steps = parse_script("8:zoom:2,7:open,9:point:NaN:0.5,10:pan:0.1:0,11:clear,12:close");
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0].0, 7.0);
    }
}
