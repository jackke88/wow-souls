//! First-visit location cards: locally embedded OFL serif, soft veil and quiet fading.
mod discovery;

use crate::avatar::Avatar;
use crate::combat::Health;
use crate::loot::Screen;
use crate::settings::MenuUp;
use crate::settle::Phase;
use benilla_assets::LockRecover;
use benilla_formats::AreaTableCatalog;
use benilla_world::world_point::WorldPoint;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use discovery::{Journal, Playback};

#[derive(Resource)]
struct Discovery {
    catalog: Option<AreaTableCatalog>,
    font: Handle<Font>,
    veil: Handle<Image>,
    journal: Journal,
    playback: Playback,
    last_area: Option<u32>,
}
#[derive(Component)]
struct Root;
#[derive(Component)]
struct Title;
#[derive(Component)]
struct Veil;
#[derive(Component)]
struct Rule;

fn enabled() -> bool {
    super::theme::enabled() && std::env::var("WOWSOULS_AREA_TITLE").as_deref() != Ok("0")
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, load.after(benilla_assets::AssetSet::Open))
        .add_systems(OnEnter(Phase::Playing), spawn.run_if(enabled))
        .add_systems(OnExit(Phase::Playing), cleanup)
        .add_systems(
            Update,
            update
                .after(benilla_world::terrain_stream::AreaAuthoritySet)
                .run_if(in_state(Phase::Playing))
                .run_if(enabled),
        );
}

fn load(
    mut commands: Commands,
    assets: Option<Res<benilla_assets::WorldAssets>>,
    mut fonts: ResMut<Assets<Font>>,
    mut images: ResMut<Assets<Image>>,
) {
    let catalog = assets.and_then(|a| {
        match benilla_formats::load_area_table_catalog(&mut a.chain.lock_recover()) {
            Ok(catalog) => Some(catalog),
            Err(e) => {
                warn!("area title: {e:#}");
                None
            }
        }
    });
    let font = fonts.add(
        Font::try_from_bytes(
            include_bytes!("../../assets/fonts/eb-garamond/EBGaramond.ttf").to_vec(),
        )
        .expect("bundled OFL EB Garamond is a valid font"),
    );
    // A separable soft veil, generated in memory: no extracted or generated-art dependency.
    let (w, h) = (512, 64);
    let mut pixels = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let v = (std::f32::consts::PI * y as f32 / (h - 1) as f32)
            .sin()
            .powf(0.8);
        for x in 0..w {
            let edge = ((x.min(w - 1 - x) as f32) / 110.0).clamp(0.0, 1.0);
            let edge = edge * edge * (3.0 - 2.0 * edge);
            pixels.extend_from_slice(&[12, 14, 12, (edge * v * 85.0) as u8]);
        }
    }
    let veil = images.add(Image::new(
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ));
    let path = std::env::var_os("WOWSOULS_AREAS_SAVE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            if crate::run_mode::unattended() {
                format!("cache/area-title/check-{}.txt", std::process::id()).into()
            } else {
                crate::saves::file(discovery::FILE)
            }
        });
    commands.insert_resource(Discovery {
        catalog,
        font,
        veil,
        journal: Journal::load(path),
        playback: Playback::default(),
        last_area: None,
    });
}

fn spawn(mut commands: Commands, data: Res<Discovery>) {
    let root = commands
        .spawn((
            Root,
            Node {
                position_type: PositionType::Absolute,
                left: percent(3),
                top: percent(49),
                width: percent(94),
                height: px(96),
                margin: UiRect::top(px(-48)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(45),
            Visibility::Hidden,
        ))
        .id();
    commands.spawn((
        Veil,
        Node {
            position_type: PositionType::Absolute,
            width: px(860),
            max_width: percent(100),
            height: px(96),
            ..default()
        },
        ImageNode::new(data.veil.clone()).with_color(Color::NONE),
        ChildOf(root),
    ));
    for y in [13.0, 82.0] {
        commands.spawn((
            Rule,
            Node {
                position_type: PositionType::Absolute,
                top: px(y),
                width: px(650),
                max_width: percent(83),
                height: px(0.7),
                ..default()
            },
            BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(
                LinearGradient::TO_RIGHT,
                vec![
                    ColorStop::percent(Color::NONE, 0.0),
                    ColorStop::percent(Color::srgba(0.8, 0.79, 0.71, 0.0), 30.0),
                    ColorStop::percent(Color::srgba(0.8, 0.79, 0.71, 0.0), 70.0),
                    ColorStop::percent(Color::NONE, 100.0),
                ],
            ))]),
            ChildOf(root),
        ));
    }
    commands.spawn((
        Title,
        Text::new(""),
        Node::default(),
        TextFont {
            font: data.font.clone(),
            font_size: 62.0,
            ..default()
        },
        TextLayout::new(Justify::Center, LineBreak::NoWrap),
        TextColor(Color::NONE),
        TextShadow {
            offset: Vec2::new(1.0, 2.0),
            color: Color::NONE,
        },
        ChildOf(root),
    ));
}

#[allow(clippy::too_many_arguments)]
fn update(
    world: WorldPoint,
    time: Res<Time<bevy::time::Real>>,
    menu: Res<MenuUp>,
    screen: Res<Screen>,
    avatar: Query<(&Transform, &Health), With<Avatar>>,
    mut data: ResMut<Discovery>,
    mut roots: Query<&mut Visibility, With<Root>>,
    mut titles: Query<(&mut Text, &mut TextColor, &mut TextShadow), With<Title>>,
    mut veils: Query<&mut ImageNode, With<Veil>>,
    mut rules: Query<&mut BackgroundGradient, With<Rule>>,
) {
    let raw = world.area();
    let area = raw.and_then(|id| {
        data.catalog
            .as_ref()
            .and_then(|c| discovery::destination(c, id))
    });
    if raw != data.last_area {
        data.last_area = raw;
        if std::env::var_os("WOWSOULS_AREA_TRACE").is_some() {
            let pos = avatar
                .iter()
                .next()
                .map(|(t, _)| (-t.translation.z, -t.translation.x));
            info!("area title: area {raw:?}, destination {area:?}, wow {pos:?}");
        }
    }
    let alive = avatar.single().is_ok_and(|(_, hp)| hp.cur > 0.0);
    let available = !menu.0 && !screen.open && !screen.modal;
    let Discovery {
        playback, journal, ..
    } = &mut *data;
    if let Some(id) = playback.tick(area, time.delta_secs(), available, alive, &journal.seen) {
        journal.discover(id);
        if let Some(name) = data.catalog.as_ref().and_then(|c| c.name(id)) {
            for (mut text, ..) in &mut titles {
                text.0 = name.to_string();
            }
            info!("area title: discovered {id} ({name})");
        }
    }
    let alpha = if available && alive {
        data.playback
            .active
            .map_or(0.0, |(_, age)| discovery::opacity(age))
    } else {
        0.0
    };
    for mut visibility in &mut roots {
        *visibility = if alpha > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if alpha == 0.0 {
        return;
    }
    for (_, mut color, mut shadow) in &mut titles {
        color.0 = Color::srgba(0.94, 0.935, 0.90, alpha);
        shadow.color = Color::srgba(0.0, 0.0, 0.0, alpha * 0.85);
    }
    for mut veil in &mut veils {
        veil.color = Color::WHITE.with_alpha(alpha);
    }
    for mut gradient in &mut rules {
        if let Gradient::Linear(g) = &mut gradient.0[0] {
            for stop in &mut g.stops[1..3] {
                stop.color.set_alpha(alpha * 0.17);
            }
        }
    }
}

fn cleanup(mut commands: Commands, roots: Query<Entity, With<Root>>, mut data: ResMut<Discovery>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
    data.playback = Playback::default();
}
