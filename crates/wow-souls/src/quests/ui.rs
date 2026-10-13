//! The quest screens, in the layout's 1280 x 720 pixels (`UiScale` scales them): the dialogue, the quest
//! log, the tracker, the toasts and the "!" / "?" over the NPCs. The look is the loot module's: the
//! Dark Souls parchment sprite, Georgia, ivory text. Text is plain ASCII.

use bevy::animation::transition::AnimationTransitions;
use bevy::animation::RepeatAnimation;
use bevy::prelude::*;
use bevy::sprite::{BorderRect, TextureSlicer};
use bevy::ui::widget::NodeImageMode;
use wow_world::quest::rules::{marks, Mark};
use wow_world::quest::Status;

use super::prompt::{self, Interact, PadUsed};
use super::rewards::{self, Where};
use super::{dialogue, lines, LogScreen, QuestGame, Talk, Toasts};
use crate::actor::{ActorModel, Look};
use crate::friendly::Npc;
use crate::settle::Phase;

const IVORY: Color = Color::srgb(0.88, 0.85, 0.77);
const GOLD: Color = Color::srgb(0.92, 0.76, 0.28);
const MUTED: Color = Color::srgb(0.67, 0.65, 0.58);

const TRACKER_MAX: usize = 5;

#[derive(Resource)]
struct Fonts {
    body: Handle<Font>,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Label {
    Title,
    Subtitle,
    Body,
    Objectives,
    Rewards,
    List,
    Footer,
    LogList,
    LogDetail,
    LogFooter,
    Tracker,
    Toast,
    InteractKey,
    InteractVerb,
    InteractName,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Dialogue,
    Log,
    Tracker,
    Toasts,
    Interact,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, fonts)
        .add_systems(OnEnter(Phase::Playing), spawn)
        .add_systems(
            Update,
            (refresh, rewards::refresh, mark_npcs, spin_marks)
                .after(super::drive)
                .run_if(in_state(Phase::Playing)),
        );
}

fn fonts(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let body = std::fs::read("C:/Windows/Fonts/georgia.ttf")
        .ok()
        .and_then(|b| Font::try_from_bytes(b).ok())
        .map(|f| fonts.add(f))
        .unwrap_or_default();
    commands.insert_resource(Fonts { body });
}

fn text(
    commands: &mut Commands,
    parent: Entity,
    font: &Handle<Font>,
    size: f32,
    color: Color,
    label: Label,
) -> Entity {
    commands
        .spawn((
            Text::new(""),
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            TextColor(color),
            Node {
                margin: UiRect::bottom(px(8)),
                ..default()
            },
            label,
            ChildOf(parent),
        ))
        .id()
}

fn frame(
    commands: &mut Commands,
    node: Node,
    image: Option<Handle<Image>>,
    panel: Panel,
) -> Entity {
    let mut e = commands.spawn((node, panel, Visibility::Hidden));
    match image {
        Some(image) => {
            let mut img = ImageNode::new(image);
            img.image_mode = NodeImageMode::Sliced(TextureSlicer {
                border: BorderRect::all(18.0),
                ..default()
            });
            e.insert(img);
        }
        None => {
            e.insert(BackgroundColor(Color::srgba(0.05, 0.04, 0.03, 0.92)));
        }
    }
    e.id()
}

fn spawn(mut commands: Commands, fonts: Res<Fonts>, hud: Option<Res<crate::hud::art::HudArt>>) {
    let parchment = hud
        .as_deref()
        .map(|h| h.sprite_image("soul_param/bg_parchment_0"))
        .filter(|h| *h != Handle::default());
    let f = &fonts.body;
    let pad = UiRect::all(px(26));

    // The dialogue: docked to the right edge, clear of the health bars and the pouch on the left and
    // of the player in the middle (the player walks and looks about while it is up).
    let dialogue = frame(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            right: px(36),
            top: px(64),
            width: px(520),
            max_height: px(640),
            flex_direction: FlexDirection::Column,
            padding: pad,
            overflow: Overflow::clip_y(),
            ..default()
        },
        parchment.clone(),
        Panel::Dialogue,
    );
    text(&mut commands, dialogue, f, 25.0, GOLD, Label::Title);
    text(&mut commands, dialogue, f, 14.0, MUTED, Label::Subtitle);
    text(&mut commands, dialogue, f, 16.0, IVORY, Label::Body);
    text(&mut commands, dialogue, f, 15.0, IVORY, Label::Objectives);
    rewards::spawn_strips(&mut commands, dialogue, Where::Dialogue, f);
    text(&mut commands, dialogue, f, 15.0, GOLD, Label::Rewards);
    text(&mut commands, dialogue, f, 18.0, IVORY, Label::List);
    text(&mut commands, dialogue, f, 13.0, MUTED, Label::Footer);

    // The quest log: a list and the selected quest.
    let log = frame(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            // Right of the reward card's place on the left.
            left: px(396),
            top: px(110),
            width: px(800),
            max_height: px(580),
            flex_direction: FlexDirection::Column,
            padding: pad,
            overflow: Overflow::clip_y(),
            ..default()
        },
        parchment.clone(),
        Panel::Log,
    );
    rewards::spawn_card(&mut commands, f, parchment);
    let columns = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(24),
                ..default()
            },
            ChildOf(log),
        ))
        .id();
    let list_col = commands
        .spawn((
            Node {
                width: px(270),
                min_width: px(270),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(columns),
        ))
        .id();
    text(&mut commands, list_col, f, 15.0, IVORY, Label::LogList);
    let detail_col = commands
        .spawn((
            Node {
                width: px(440),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(columns),
        ))
        .id();
    text(&mut commands, detail_col, f, 15.0, IVORY, Label::LogDetail);
    rewards::spawn_strips(&mut commands, detail_col, Where::Log, f);
    text(&mut commands, log, f, 13.0, MUTED, Label::LogFooter);

    // The tracker: top right under the souls of the HUD's corner.
    let tracker = frame(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            right: px(24),
            top: px(150),
            width: px(300),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        None,
        Panel::Tracker,
    );
    commands.entity(tracker).remove::<BackgroundColor>();
    text(&mut commands, tracker, f, 13.0, IVORY, Label::Tracker);

    // The interact prompt: an icon with the key, then the verb and the name, bottom centre.
    let prompt = frame(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            margin: UiRect::left(px(-200)),
            bottom: px(150),
            width: px(400),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(14),
            padding: UiRect::axes(px(16), px(10)),
            ..default()
        },
        None,
        Panel::Interact,
    );
    commands
        .entity(prompt)
        .insert(BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.78)));
    let cap = commands
        .spawn((
            Node {
                width: px(36),
                height: px(36),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(7)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(IVORY),
            BackgroundColor(Color::srgba(0.16, 0.14, 0.11, 0.95)),
            ChildOf(prompt),
        ))
        .id();
    commands.spawn((
        Text::new(""),
        TextFont {
            font: f.clone(),
            font_size: 22.0,
            ..default()
        },
        TextColor(GOLD),
        Label::InteractKey,
        ChildOf(cap),
    ));
    let words = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(prompt),
        ))
        .id();
    commands.spawn((
        Text::new(""),
        TextFont {
            font: f.clone(),
            font_size: 19.0,
            ..default()
        },
        TextColor(IVORY),
        Label::InteractVerb,
        ChildOf(words),
    ));
    commands.spawn((
        Text::new(""),
        TextFont {
            font: f.clone(),
            font_size: 13.0,
            ..default()
        },
        TextColor(MUTED),
        Label::InteractName,
        ChildOf(words),
    ));

    // The toasts: bottom centre.
    let toasts = frame(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            margin: UiRect::left(px(-300)),
            bottom: px(120),
            width: px(600),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        },
        None,
        Panel::Toasts,
    );
    commands.entity(toasts).remove::<BackgroundColor>();
    commands.spawn((
        Text::new(""),
        TextFont {
            font: f.clone(),
            font_size: 17.0,
            ..default()
        },
        TextColor(IVORY),
        TextLayout::new_with_justify(Justify::Center),
        Label::Toast,
        ChildOf(toasts),
    ));
}

fn set(text: &mut Text, node: &mut Node, value: String) {
    if text.0 != value {
        text.0 = value;
    }
    let want = if text.0.is_empty() {
        Display::None
    } else {
        Display::Flex
    };
    if node.display != want {
        node.display = want;
    }
}

/// What the log shows for the selected quest.
fn log_detail(game: &QuestGame, quest: u32) -> String {
    let Some(q) = game.book.quest(quest) else {
        return String::new();
    };
    let e = game.log.entry(quest);
    let mut out = format!("{}  (level {})\n", q.title, q.level);
    if let Some(e) = e {
        let w = lines::status_word(e);
        if !w.is_empty() {
            out += &format!("{w}\n");
        }
    }
    out += "\n";
    out += &wow_world::quest::text::expand(&q.objectives, &game.player);
    out += "\n\n";
    for l in lines::objective_lines(&game.book, q, e) {
        out += &format!("- {l}\n");
    }
    if !game.book.ender_spawned(quest) {
        out += "\nIts quest giver stands outside the valley: it cannot be handed in here.\n";
    }
    let r = lines::reward_line(q);
    if !r.is_empty() {
        out += &format!("\n{r}\n");
    }
    out
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh(
    game: Option<Res<QuestGame>>,
    talk: Res<Talk>,
    log: Res<LogScreen>,
    toasts: Res<Toasts>,
    interact: Res<Interact>,
    shop: Res<crate::shop::Shop>,
    pad: Res<PadUsed>,
    real: Res<Time<Real>>,
    mut panels: Query<(&Panel, &mut Visibility)>,
    mut labels: Query<(&Label, &mut Text, &mut Node), Without<Panel>>,
) {
    let Some(game) = game else { return };
    let now = real.elapsed_secs();
    let live_toasts: Vec<&str> = toasts
        .lines
        .iter()
        .filter(|(_, until)| *until > now)
        .map(|(t, _)| t.as_str())
        .collect();
    let active = game.log.active();
    let modal = talk.0.is_some() || log.open || shop.open.is_some();

    for (panel, mut vis) in &mut panels {
        let show = match panel {
            Panel::Dialogue => talk.0.is_some(),
            Panel::Log => log.open,
            Panel::Tracker => !modal && !active.is_empty(),
            Panel::Toasts => !live_toasts.is_empty(),
            Panel::Interact => interact.0.is_some() && !modal,
        };
        let want = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }

    // The dialogue's words.
    let view = talk.0.as_ref().map(|d| {
        dialogue::view(d, &game.book, &game.log, &game.player, &|id| {
            game.book.item_name(id).unwrap_or("Item").to_string()
        })
    });
    // The log's words.
    let selected = active.get(log.selected).copied();
    let log_list: String = active
        .iter()
        .enumerate()
        .filter_map(|(i, &q)| {
            let title = &game.book.quest(q)?.title;
            let done = game.log.entry(q).map_or("", lines::status_word);
            Some(format!(
                "{} {}{}\n",
                if i == log.selected { ">" } else { " " },
                title,
                if done.is_empty() {
                    String::new()
                } else {
                    format!(" ({done})")
                }
            ))
        })
        .collect();
    let (lvl, into, need) = game.table.xp_into_level(game.xp);
    let status_line = lines::level_line(lvl, into, need);
    let tracker: String = active
        .iter()
        .take(TRACKER_MAX)
        .filter_map(|&q| {
            let quest = game.book.quest(q)?;
            let e = game.log.entry(q);
            let mut s = format!("{}\n", quest.title);
            for l in lines::objective_lines(&game.book, quest, e) {
                s += &format!("   {l}\n");
            }
            if e.is_some_and(|e| e.status == Status::Complete) {
                s += "   (Complete)\n";
            }
            Some(s)
        })
        .collect::<Vec<_>>()
        .join("\n");

    for (label, mut t, mut node) in &mut labels {
        let value = match label {
            Label::Title => view.as_ref().map(|v| v.title.clone()),
            Label::Subtitle => view.as_ref().map(|v| v.subtitle.clone()),
            Label::Body => view.as_ref().map(|v| v.body.clone()),
            Label::Objectives => view.as_ref().map(|v| {
                if v.objectives.is_empty() {
                    String::new()
                } else {
                    format!("Objectives\n{}", v.objectives.join("\n"))
                }
            }),
            Label::Rewards => view.as_ref().map(|v| v.rewards.join("\n")),
            Label::List => view.as_ref().map(|v| v.list.join("\n")),
            Label::Footer => view.as_ref().map(|v| v.footer.clone()),
            Label::LogList => Some(if log_list.is_empty() {
                "No quests.".to_string()
            } else {
                log_list.clone()
            }),
            Label::LogDetail => Some(selected.map_or_else(
                || format!("\n{status_line}"),
                |q| format!("{}\n{status_line}", log_detail(&game, q)),
            )),
            Label::LogFooter => Some(
                "Up/Down  Choose      Left/Right or LB/RB  Look at rewards      Delete  Abandon (twice)      L / Backspace / B  Close"
                    .to_string(),
            ),
            Label::Tracker => Some(tracker.clone()),
            Label::Toast => Some(live_toasts.join("\n")),
            Label::InteractKey => interact
                .0
                .as_ref()
                .map(|_| prompt::glyph(pad.0).to_string()),
            Label::InteractVerb => interact.0.as_ref().map(|t| t.verb.to_string()),
            Label::InteractName => interact.0.as_ref().map(|t| {
                if t.hint.is_empty() {
                    t.name.clone()
                } else {
                    format!("{}  -  {}", t.name, t.hint)
                }
            }),
        };
        if let Some(value) = value {
            set(&mut t, &mut node, value);
        }
    }
}

/// WoW's own overhead markers, the 3D models of the 1.12 client (`Interface\Buttons`): the yellow "!"
/// for a quest to take, the grey "!" for a low one, the grey "?" for one in progress and the yellow "?"
/// for one to hand in.
fn marker_model(mark: Mark) -> Option<&'static str> {
    match mark {
        Mark::Available => Some("Interface/Buttons/TalkToMe.m2"),
        Mark::AvailableLow => Some("Interface/Buttons/TalkToMeGrey.m2"),
        Mark::InProgress => Some("Interface/Buttons/TalkToMeQuestion_Grey.m2"),
        Mark::Ready => Some("Interface/Buttons/TalkToMeQuestionMark.m2"),
        Mark::None => None,
    }
}

/// The marker above an NPC: a child of the NPC whose model follows the NPC's quest business.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct QuestMarker(Mark);

/// The marker has started its looping animation.
#[derive(Component)]
struct Spinning;

/// How high over the NPC's feet the marker floats (yd) and how big it is (yd per model unit).
const MARK_HEIGHT: f32 = 2.4;
const MARK_SCALE: f32 = 1.3;

/// Keeps one marker model above each NPC with quest business, replacing it when the business changes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn mark_npcs(
    mut commands: Commands,
    game: Option<Res<QuestGame>>,
    assets: Res<AssetServer>,
    npcs: Query<(Entity, &Transform, &Npc, Option<&Children>)>,
    markers: Query<&QuestMarker>,
) {
    let Some(game) = game else { return };
    for (e, xf, npc, children) in &npcs {
        let mark = marks(
            &game.book,
            &game.log,
            &game.player,
            wow_world::quest::Npc::Creature(npc.entry),
        );
        let current = children
            .into_iter()
            .flat_map(|c| c.iter())
            .find_map(|c| markers.get(c).ok().map(|m| (c, m.0)));
        if current.is_some_and(|(_, m)| m == mark) {
            continue;
        }
        if let Some((old, _)) = current {
            commands.entity(old).despawn();
        }
        let Some(path) = marker_model(mark) else {
            continue;
        };
        info!("quests: mark {mark:?} over NPC {}", npc.entry);
        // Children inherit the NPC scale: undo it so the marker is the same size over everyone.
        let s = xf.scale.y.max(0.1);
        commands.spawn((
            Name::new("Quest mark"),
            QuestMarker(mark),
            Transform::from_translation(Vec3::Y * (MARK_HEIGHT / s))
                .with_scale(Vec3::splat(MARK_SCALE / s)),
            Visibility::default(),
            ActorModel {
                m2: assets.load(benilla_assets::m2_url(path)),
                dir: "Interface/Buttons".into(),
                look: Look::Creature {
                    skins: [None, None, None],
                },
                scale: 1.0,
                display: 0,
                worn: None,
            },
            ChildOf(e),
        ));
    }
}

/// Starts a marker's own looping animation (the "!" and "?" turn and bob) once its model is built.
fn spin_marks(
    mut commands: Commands,
    mut markers: Query<
        (
            Entity,
            &benilla_assets::ModelAnimations,
            &mut AnimationPlayer,
            &mut AnimationTransitions,
        ),
        (With<QuestMarker>, Without<Spinning>),
    >,
) {
    for (e, anims, mut player, mut transitions) in &mut markers {
        if let Some(clip) = anims.find(0) {
            transitions
                .play(&mut player, clip.node, std::time::Duration::ZERO)
                .set_repeat(RepeatAnimation::Forever);
        }
        commands.entity(e).insert(Spinning);
    }
}
