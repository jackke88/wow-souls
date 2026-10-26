//! Restrained Elden Ring reference language, drawn by Bevy: smoke, ivory and aged gold.
//! Small material sheets add grain, edge wear and etched icons; values and layout remain native.
use bevy::prelude::*;
mod skin;
pub(crate) use skin::Surface;

pub(super) fn plugin(app: &mut App) {
    skin::plugin(app);
}

pub(crate) const IVORY: Color = Color::srgb(0.82, 0.81, 0.74);
pub(crate) const GOLD: Color = Color::srgb(0.67, 0.62, 0.43);
pub(crate) const MUTED: Color = Color::srgb(0.60, 0.59, 0.53);
pub(crate) const EDGE: Color = Color::srgba(0.49, 0.46, 0.33, 0.65);
pub(crate) const INK: Color = Color::srgba(0.065, 0.070, 0.060, 0.92);
pub(crate) const SELECTED: Color = Color::srgba(0.62, 0.57, 0.37, 0.24);

pub(crate) fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("WOWSOULS_HUD").as_deref() != Ok("0"))
}

pub(crate) fn surface() -> BackgroundGradient {
    BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(
        LinearGradient::TO_BOTTOM,
        vec![
            ColorStop::percent(Color::srgba(0.045, 0.052, 0.044, 0.96), 0.0),
            ColorStop::percent(Color::srgba(0.085, 0.090, 0.075, 0.94), 60.0),
            ColorStop::percent(Color::srgba(0.18, 0.18, 0.14, 0.89), 100.0),
        ],
    ))])
}

/// Set the shared panel look on an existing node; callers retain all input/state behavior.
pub(crate) fn dress(entity: &mut EntityCommands) {
    entity.insert((Surface::Panel, BorderColor::all(EDGE)));
}

pub(crate) fn panel(commands: &mut Commands, mut node: Node, parent: Option<Entity>) -> Entity {
    node.border = UiRect::axes(px(0.0), px(0.7));
    let mut entity = commands.spawn((node, surface(), Surface::Panel, BorderColor::all(EDGE)));
    if let Some(parent) = parent {
        entity.insert(ChildOf(parent));
    }
    entity.id()
}

pub(crate) fn words(
    commands: &mut Commands,
    parent: Entity,
    value: &str,
    font: &Handle<Font>,
    size: f32,
    node: Node,
) -> Entity {
    commands
        .spawn((
            Text::new(value),
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            TextColor(IVORY),
            TextShadow {
                offset: Vec2::new(0.7, 1.0),
                color: Color::srgba(0.0, 0.0, 0.0, 0.95),
            },
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            node,
            ChildOf(parent),
        ))
        .id()
}

pub(crate) fn fill_gradient(color: Color) -> BackgroundGradient {
    let c = color.to_srgba();
    let shade = |gain: f32| Color::srgb(c.red * gain, c.green * gain, c.blue * gain);
    BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(
        LinearGradient::TO_BOTTOM,
        vec![
            ColorStop::percent(shade(0.68), 0.0),
            ColorStop::percent(shade(1.0), 30.0),
            ColorStop::percent(shade(0.83), 100.0),
        ],
    ))])
}

/// Return track, chip and current fill; all are native UI shapes with a one-pixel rim.
pub(crate) fn meter(
    commands: &mut Commands,
    parent: Entity,
    mut node: Node,
    color: Color,
) -> (Entity, Entity, Entity) {
    node.border = UiRect::all(px(0.7));
    let track = commands
        .spawn((
            node,
            BackgroundColor(Color::srgba(0.035, 0.035, 0.028, 0.82)),
            BorderColor::all(EDGE),
            Surface::Bar,
            ChildOf(parent),
        ))
        .id();
    let inside = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(4),
                right: px(4),
                top: px(0.5),
                bottom: px(0.5),
                ..default()
            },
            ChildOf(track),
        ))
        .id();
    let mut layer = |color: Color| {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                fill_gradient(color),
                Surface::Fill(color),
                ChildOf(inside),
            ))
            .id()
    };
    let chip = layer(Color::srgb(0.57, 0.47, 0.22));
    let fill = layer(color);
    (track, chip, fill)
}
