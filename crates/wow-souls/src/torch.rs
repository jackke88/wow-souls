//! The torch (2026-10-18, the user: "facklor ... en extra gear slot ... eftersom det kan bli så mörkt nu med
//! den nya grafiken ... brinna slut efter x mycket tid"; 2026-10-28: "ta bort det visuella ... det lyser
//! bara runt sig som om man hade en torch ute ... en action att kunna släcka den ... aktiv i x många
//! minuter"). The item and its slot are [`wow_world::torch`]. A worn torch has no model: the body just
//! carries the engine's flickering torch light, as if it held a lit one. [`Action::Torch`] douses and
//! lights it again; only a lit torch burns, for [`BURN_SECS`] of play, and then it is gone from the slot.
//! What is left of the burn (and whether the flame is up) is saved per character, so a torch taken off,
//! doused or carried over a restart goes on from where it was.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use wow_world::equipment::Slot;
use wow_world::torch::BURN_SECS;

use crate::avatar::Avatar;
use crate::combat::Action as Combat;
use crate::loot::Owned;
use crate::settings::binds::Action;
use crate::settings::Controls;
use crate::settle::Phase;

pub const FILE_NAME: &str = "torch-v1.txt";
/// Seconds between saves of the burn.
const SAVE_EVERY: f32 = 5.0;
/// The flame's warm light (linear RGB) and its strength against the engine's placed fires.
const COLOR: [f32; 3] = [1.0, 0.62, 0.28];
const STRENGTH: f32 = 0.9;
/// Where the light hangs in the body's frame (yd; -Z is ahead): left of the body, about head height,
/// a little forward: where a raised torch would be held. Nothing is drawn there.
const AT_HAND: Vec3 = Vec3::new(-0.3, 1.5, -0.2);

/// What happened to the torch that the player should be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Lit,
    Doused,
    BurntOut,
}

impl Event {
    /// The toast: what happened and, for a flame lit or put out, how much burn the torch has left.
    pub fn toast(self, left: f32) -> String {
        match self {
            Event::Lit => format!("You light your torch. {} left.", clock(remaining(left))),
            Event::Doused => format!("You douse your torch. {} left.", clock(remaining(left))),
            Event::BurntOut => "Your torch burns out.".to_string(),
        }
    }
}

/// The burn of the torch in the slot, and whether its flame is up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burning {
    /// Seconds of burn left; 0 when no torch has been started (the next one is fresh).
    pub left: f32,
    /// A full torch's burn (seconds): [`BURN_SECS`], or what `WOWSOULS_TORCH_SECS` shortens it to.
    pub full: f32,
    /// The flame is up. A torch put on is lit; this is how a doused one stays out across a restart.
    pub lit: bool,
    /// Whether a torch is in the slot; `None` until the slot has been looked at once, so what the save
    /// says about a torch that was already worn is not mistaken for putting one on.
    worn: Option<bool>,
}

impl Default for Burning {
    fn default() -> Self {
        Burning {
            left: 0.0,
            full: BURN_SECS,
            lit: true,
            worn: None,
        }
    }
}

impl Burning {
    pub fn new(left: f32, lit: bool, full: f32) -> Self {
        Burning {
            left: left.min(full),
            full,
            lit,
            worn: None,
        }
    }

    /// A torch is in the slot and its flame is up: the light is on and the burn runs.
    pub fn glows(&self) -> bool {
        self.worn == Some(true) && self.lit
    }

    /// The slot as it is now. A torch put on is lit (the player's own dousing is not remembered over a
    /// change of torch); the first look at the slot only learns what is there.
    pub fn sync(&mut self, worn: bool) -> Option<Event> {
        let was = self.worn.replace(worn);
        if worn && was == Some(false) {
            self.lit = true;
            return Some(Event::Lit);
        }
        None
    }

    /// The light/douse action: nothing to do with an empty slot.
    pub fn toggle(&mut self) -> Option<Event> {
        if self.worn != Some(true) {
            return None;
        }
        self.lit = !self.lit;
        Some(if self.lit { Event::Lit } else { Event::Doused })
    }

    /// `dt` seconds pass. A lit torch burns; one that runs dry goes out for good (`left` back to 0, so
    /// the next torch is a fresh one).
    pub fn tick(&mut self, dt: f32) -> Option<Event> {
        if !self.glows() {
            return None;
        }
        let (left, out) = burn(self.left, dt, self.full);
        self.left = left;
        if out {
            self.left = 0.0;
            self.lit = false;
            return Some(Event::BurntOut);
        }
        None
    }
}

/// The torch in the slot and where its burn is saved.
#[derive(Resource, Debug)]
pub struct Fuel {
    pub torch: Burning,
    path: PathBuf,
    since_save: f32,
}

/// The light a lit torch gives, a child of the body.
#[derive(Component)]
struct Flame;

pub fn to_text(left: f32, lit: bool) -> String {
    format!("wstorch 2\nleft {left:.1}\nlit {}\n", u8::from(lit))
}

/// `(left, lit)` of a save. The first version had no `lit` line: its torch always burnt.
pub fn from_text(text: &str) -> Option<(f32, bool)> {
    let mut lines = text.lines();
    let version = match lines.next()? {
        "wstorch 1" => 1,
        "wstorch 2" => 2,
        _ => return None,
    };
    let left: f32 = lines.next()?.strip_prefix("left ")?.trim().parse().ok()?;
    if !(left.is_finite() && (0.0..=BURN_SECS).contains(&left)) {
        return None;
    }
    let lit = if version == 1 {
        true
    } else {
        match lines.next()?.strip_prefix("lit ")?.trim() {
            "1" => true,
            "0" => false,
            _ => return None,
        }
    };
    Some((left, lit))
}

/// `dt` seconds of burning: `left` the torch's fuel. A torch lit with nothing left is a fresh one (`full`
/// seconds). Returns the fuel after `dt` and whether it burnt out in it.
pub fn burn(left: f32, dt: f32, full: f32) -> (f32, bool) {
    let left = if left <= 0.0 { full } else { left };
    let after = (left - dt).max(0.0);
    (after, after <= 0.0)
}

/// The burn a torch shows: a torch not yet started is full.
pub fn remaining(left: f32) -> f32 {
    if left <= 0.0 {
        BURN_SECS
    } else {
        left
    }
}

/// `m:ss`, rounded up to the second so a torch never reads `0:00` while it still burns.
pub fn clock(secs: f32) -> String {
    let whole = secs.max(0.0).ceil() as u32;
    format!("{}:{:02}", whole / 60, whole % 60)
}

/// `WOWSOULS_TORCH_SECS=<s>`: a debug switch that shortens a full torch's burn, to see one burn out.
fn full_burn(var: Option<&str>) -> f32 {
    var.and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|s| s.is_finite() && *s > 0.0)
        .map_or(BURN_SECS, |s| s.min(BURN_SECS))
}

/// `WOWSOULS_TORCH_AT=<s>[,<s>...]`: the real seconds at which the light/douse action is pressed for a
/// run nobody plays (unattended runs ignore live keys).
fn script_times(var: Option<&str>) -> Vec<f32> {
    let mut times: Vec<f32> = var
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .filter(|s| s.is_finite() && *s >= 0.0)
        .collect();
    times.sort_by(f32::total_cmp);
    times
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load).add_systems(
        Update,
        (toggle, burn_down, light)
            .chain()
            .run_if(in_state(Phase::Playing)),
    );
}

fn load(mut commands: Commands) {
    let path = if crate::run_mode::unattended() {
        PathBuf::from(format!("cache/loot-check/torch-{}.txt", std::process::id()))
    } else {
        crate::saves::file(FILE_NAME)
    };
    let (left, lit) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| from_text(&t))
        .unwrap_or((0.0, true));
    let full = full_burn(std::env::var("WOWSOULS_TORCH_SECS").ok().as_deref());
    commands.insert_resource(Fuel {
        torch: Burning::new(left, lit, full),
        path,
        since_save: 0.0,
    });
}

fn save(path: &Path, torch: &Burning) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(path, to_text(torch.left, torch.lit)) {
        warn!("torch: cannot save {}: {e}", path.display());
    }
}

fn announce(toasts: &mut crate::quests::Toasts, now: f32, event: Event, left: f32) {
    toasts.push(now, event.toast(left));
    info!("torch: {event:?}, {left:.1} s left");
}

/// The light/douse action (and `WOWSOULS_TORCH_AT`'s presses) on the worn torch.
fn toggle(
    controls: Controls,
    real: Res<Time<Real>>,
    fuel: Option<ResMut<Fuel>>,
    screen: Res<crate::loot::Screen>,
    avatar: Query<&Combat, With<Avatar>>,
    mut script: Local<Option<Vec<f32>>>,
    mut toasts: ResMut<crate::quests::Toasts>,
) {
    let Some(mut fuel) = fuel else {
        return;
    };
    let now = real.elapsed_secs();
    let pending = script
        .get_or_insert_with(|| script_times(std::env::var("WOWSOULS_TORCH_AT").ok().as_deref()));
    let scripted = pending.first().is_some_and(|t| now >= *t);
    if scripted {
        pending.remove(0);
    }
    let pressed = !screen.open
        && !screen.modal
        && !avatar.iter().any(Combat::is_dead)
        && controls.just_pressed(Action::Torch);
    if !(scripted || pressed) {
        return;
    }
    if let Some(event) = fuel.torch.toggle() {
        announce(&mut toasts, now, event, fuel.torch.left);
        fuel.since_save = SAVE_EVERY;
    }
}

/// Reads the slot, burns a lit torch down, takes a spent one out of the slot, and saves the burn.
fn burn_down(
    time: Res<Time>,
    real: Res<Time<Real>>,
    fuel: Option<ResMut<Fuel>>,
    owned: Option<ResMut<Owned>>,
    mut toasts: ResMut<crate::quests::Toasts>,
) {
    let (Some(mut fuel), Some(mut owned)) = (fuel, owned) else {
        return;
    };
    let now = real.elapsed_secs();
    let worn = owned.progress.gear.get(Slot::Torch).is_some();
    if let Some(event) = fuel.torch.sync(worn) {
        announce(&mut toasts, now, event, fuel.torch.left);
        fuel.since_save = SAVE_EVERY;
    }
    let dt = time.delta_secs();
    if fuel.torch.glows() {
        fuel.since_save += dt;
    }
    if let Some(event) = fuel.torch.tick(dt) {
        let mut next = owned.progress.clone();
        next.gear.set(Slot::Torch, None);
        match owned.publish(next) {
            Ok(()) => announce(&mut toasts, now, event, 0.0),
            Err(e) => warn!("torch: {e}"),
        }
        fuel.since_save = SAVE_EVERY;
    }
    if fuel.since_save >= SAVE_EVERY {
        fuel.since_save = 0.0;
        save(&fuel.path, &fuel.torch);
    }
}

/// The torch's light, on the body while a lit torch is worn and gone when it is doused, off or spent: the
/// engine's torch light, flickering, carried (it casts no outdoor shadow). No model is drawn.
fn light(
    mut commands: Commands,
    fuel: Option<Res<Fuel>>,
    avatar: Query<(Entity, &GlobalTransform), With<Avatar>>,
    flames: Query<(Entity, &ChildOf), With<Flame>>,
) {
    let want = fuel.is_some_and(|f| f.torch.glows());
    let Ok((me, at)) = avatar.single() else {
        return;
    };
    let mut have = false;
    for (flame, parent) in &flames {
        // One light on this body while the torch glows; any other (a race of deferred commands, a body
        // that is gone) goes.
        if want && parent.parent() == me && !have {
            have = true;
        } else {
            commands.entity(flame).despawn();
        }
    }
    if want && !have {
        commands.spawn((
            Name::new("Torch light"),
            benilla_world::terrain_stream::point_light(COLOR, STRENGTH),
            benilla_world::lighting::FlameFlicker::new(
                benilla_world::lighting::FlameKind::Torch,
                benilla_world::lighting::flicker_seed(at.translation()),
            ),
            crate::gfx::carried::HeldLight,
            Flame,
            Transform::from_translation(AT_HAND),
            Visibility::default(),
            ChildOf(me),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A torch that has been put on and looked at: worn, lit, with `left` seconds of burn.
    fn worn(left: f32) -> Burning {
        let mut b = Burning::new(left, true, BURN_SECS);
        assert_eq!(b.sync(false), None);
        assert_eq!(b.sync(true), Some(Event::Lit));
        b
    }

    #[test]
    fn a_lit_torch_burns_down_and_a_fresh_one_starts_full() {
        let (left, out) = burn(0.0, 1.0, BURN_SECS);
        assert_eq!(left, BURN_SECS - 1.0);
        assert!(!out);
        let (left, out) = burn(0.5, 1.0, BURN_SECS);
        assert_eq!(left, 0.0);
        assert!(out);
        let (left, _) = burn(0.0, 1.0, 8.0);
        assert_eq!(left, 7.0, "a shortened torch starts at its own full burn");
    }

    #[test]
    fn a_torch_burns_for_ten_minutes_of_play_and_then_goes_out_for_good() {
        assert_eq!(BURN_SECS, 600.0, "ten minutes");
        let mut b = worn(0.0);
        let mut events = Vec::new();
        // One game minute a step: nine of them leave a minute.
        for _ in 0..9 {
            events.extend(b.tick(60.0));
        }
        assert!(events.is_empty());
        assert!(b.glows());
        assert_eq!(b.left, 60.0);
        assert_eq!(b.tick(59.0), None);
        assert_eq!(b.tick(1.0), Some(Event::BurntOut));
        assert_eq!((b.left, b.lit), (0.0, false));
        assert!(!b.glows(), "no light from a spent torch");
        assert_eq!(b.tick(10.0), None, "and it burns no more");
        assert_eq!(b.left, 0.0);
    }

    #[test]
    fn a_doused_torch_gives_no_light_and_burns_nothing() {
        let mut b = worn(0.0);
        assert_eq!(b.tick(100.0), None);
        let kept = b.left;
        assert_eq!(kept, BURN_SECS - 100.0);
        assert_eq!(b.toggle(), Some(Event::Doused));
        assert!(!b.glows());
        for _ in 0..1000 {
            assert_eq!(b.tick(10.0), None);
        }
        assert_eq!(b.left, kept, "a doused torch keeps what it had");
        assert_eq!(b.toggle(), Some(Event::Lit));
        assert!(b.glows());
        assert_eq!(b.tick(10.0), None);
        assert_eq!(b.left, kept - 10.0, "lit again, it goes on from there");
    }

    #[test]
    fn the_action_does_nothing_without_a_torch_in_the_slot() {
        let mut b = Burning::default();
        assert_eq!(b.toggle(), None, "the slot has not been looked at");
        b.sync(false);
        assert_eq!(b.toggle(), None);
        assert!(!b.glows());
        assert_eq!(b.tick(10.0), None);
        assert_eq!(b.left, 0.0);
    }

    #[test]
    fn a_torch_put_on_is_lit_and_one_already_worn_keeps_what_the_save_says() {
        // Put on: lit, even over a doused flame left from the torch before.
        let mut b = Burning::new(300.0, false, BURN_SECS);
        assert_eq!(b.sync(false), None);
        assert_eq!(b.sync(true), Some(Event::Lit));
        assert!(b.glows());
        assert_eq!(b.sync(true), None, "nothing happens while it stays on");
        // Taken off: no light, the burn waits.
        assert_eq!(b.sync(false), None);
        assert!(!b.glows());
        assert_eq!(b.left, 300.0);
        // Already worn when the game opens, doused when it was saved: still doused, and told nothing.
        let mut b = Burning::new(300.0, false, BURN_SECS);
        assert_eq!(b.sync(true), None);
        assert!(!b.glows());
        assert_eq!(b.toggle(), Some(Event::Lit));
        assert!(b.glows());
    }

    #[test]
    fn the_burn_save_round_trips_and_refuses_nonsense() {
        assert_eq!(from_text(&to_text(123.4, true)), Some((123.4, true)));
        assert_eq!(from_text(&to_text(0.0, false)), Some((0.0, false)));
        assert_eq!(from_text("wstorch 2\nleft 5.0\nlit 2\n"), None);
        assert_eq!(from_text("wstorch 2\nleft 5.0\n"), None);
        assert_eq!(from_text("wstorch 2\nleft 601\nlit 1\n"), None);
        assert_eq!(from_text("wstorch 1\nleft -3\n"), None);
        assert_eq!(from_text("wstorch 3\nleft 3\nlit 1\n"), None);
        assert_eq!(from_text("junk"), None);
        // The first version had no flame line and always burnt.
        assert_eq!(from_text("wstorch 1\nleft 42.5\n"), Some((42.5, true)));
    }

    #[test]
    fn the_toasts_say_what_happened_and_how_long_is_left() {
        assert_eq!(Event::Lit.toast(0.0), "You light your torch. 10:00 left.");
        assert_eq!(
            Event::Doused.toast(125.2),
            "You douse your torch. 2:06 left."
        );
        assert_eq!(Event::BurntOut.toast(0.0), "Your torch burns out.");
        assert_eq!(clock(0.4), "0:01");
        assert_eq!(clock(59.0), "0:59");
        assert_eq!(clock(600.0), "10:00");
    }

    #[test]
    fn the_debug_switches_read_back() {
        assert_eq!(full_burn(None), BURN_SECS);
        assert_eq!(full_burn(Some(" 8 ")), 8.0);
        assert_eq!(
            full_burn(Some("9999")),
            BURN_SECS,
            "never longer than a torch"
        );
        assert_eq!(full_burn(Some("0")), BURN_SECS);
        assert_eq!(full_burn(Some("soon")), BURN_SECS);
        assert_eq!(script_times(Some("9, 3.5,x,-1")), vec![3.5, 9.0]);
        assert!(script_times(None).is_empty());
    }
}
