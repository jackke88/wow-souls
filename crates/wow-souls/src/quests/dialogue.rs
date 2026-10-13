//! The quest dialogue as a pure state machine: which page an NPC opens, what a key does on it, and the words
//! of each page. The UI draws a [`View`]; the plugin applies an [`Outcome`] to the log and the inventory.
//! The flow is the 1.12 client's: an NPC with one quest shows it at once, with several shows a list;
//! accepting closes the window; handing in closes it or goes on to the quest the same NPC offers next.

use wow_world::quest::rules::{can_take, Mark};
use wow_world::quest::text::expand;
use wow_world::quest::{Npc, Player, QuestBook, QuestLog, Status};

use super::lines;

/// What an NPC offers besides quests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Services {
    pub vendor: bool,
    pub banker: bool,
    pub trainer: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Shop,
    Bank,
    Train,
}

impl Service {
    /// The line of the list that opens it, in WoW's gossip wording.
    pub fn title(self) -> &'static str {
        match self {
            Service::Shop => "I want to browse your goods",
            Service::Bank => "I want to use the Abbey's strongbox",
            Service::Train => "I wish to train",
        }
    }
}

impl Services {
    fn list(self) -> Vec<Service> {
        let mut out = Vec::new();
        if self.vendor {
            out.push(Service::Shop);
        }
        if self.banker {
            out.push(Service::Bank);
        }
        if self.trainer {
            out.push(Service::Train);
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// Several quests to pick from.
    List,
    /// A quest on offer: its story and objectives, Accept / Decline.
    Detail(u32),
    /// A quest in the log that is not done: what the NPC still wants.
    Progress(u32),
    /// A finished quest: the thanks and the rewards, Complete Quest.
    Reward(u32),
    /// The NPC has no quest business with the player.
    Nothing,
    /// The NPC only offers one service: the game opens it at once (this page is never drawn).
    Service(Service),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialogue {
    pub npc: Npc,
    pub npc_name: String,
    pub page: Page,
    /// The selected row of the list.
    pub cursor: usize,
    /// The selected reward choice of a reward page.
    pub choice: usize,
    /// The reward being looked at, counted over the choices and then the sure rewards.
    pub focus: usize,
    /// What the NPC offers besides quests: its list ends with the way in.
    pub services: Services,
    /// With several choices the first press of Enter / A only picks the one in focus; the second hands in.
    pub armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    /// LB / the left arrow: the reward before the one in focus, on the pages that have rewards.
    Prev,
    /// RB / the right arrow: the next reward.
    Next,
    Confirm,
    Back,
}

/// What a key asks of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    None,
    Close,
    Accept(u32),
    Complete {
        quest: u32,
        choice: Option<usize>,
    },
    /// Open one of the NPC's services.
    Service(Service),
}

/// One entry of the list: a quest this NPC offers or takes in right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The quest, or 0 for the way into a service.
    pub quest: u32,
    pub service: Option<Service>,
    pub mark: Mark,
    pub title: String,
    pub level: i32,
}

/// What a page shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct View {
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub objectives: Vec<String>,
    /// The words under the reward icons: experience, money.
    pub rewards: Vec<String>,
    /// The items to choose one of, and the ones that come with the quest, `(item, count)`.
    pub choices: Vec<(u32, u32)>,
    pub fixed: Vec<(u32, u32)>,
    /// The reward in focus, over `choices` then `fixed`; `None` on a page without rewards.
    pub focus: Option<usize>,
    /// The choice in focus is picked and waits for a second press.
    pub armed: bool,
    pub list: Vec<String>,
    pub footer: String,
}

fn row_mark(book: &QuestBook, log: &QuestLog, p: &Player, quest: u32, npc: Npc) -> Option<Mark> {
    let q = book.quest(quest)?;
    let ends = book.enders(quest).contains(&npc);
    let gives = book.givers(quest).contains(&npc);
    match log.status(quest) {
        Some(Status::Complete) if ends => Some(Mark::Ready),
        Some(Status::Incomplete) if ends => Some(Mark::InProgress),
        None if gives && can_take(book, log, p, quest) => {
            Some(if p.level >= q.level.max(0) as u32 + 4 {
                Mark::AvailableLow
            } else {
                Mark::Available
            })
        }
        _ => None,
    }
}

/// The quests this NPC has business with, finished ones first, then new ones, then those in progress,
/// then the grey ones (a quest far below the player's level).
pub fn rows(
    book: &QuestBook,
    log: &QuestLog,
    p: &Player,
    npc: Npc,
    services: Services,
) -> Vec<Row> {
    let (entry, object) = match npc {
        Npc::Creature(e) => (e, false),
        Npc::Object(e) => (e, true),
    };
    let mut ids: Vec<u32> = book
        .by_giver(entry, object)
        .iter()
        .chain(book.by_ender(entry, object))
        .copied()
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let mut out: Vec<Row> = ids
        .into_iter()
        .filter_map(|quest| {
            let mark = row_mark(book, log, p, quest, npc)?;
            let q = book.quest(quest)?;
            Some(Row {
                quest,
                service: None,
                mark,
                title: q.title.clone(),
                level: q.level,
            })
        })
        .collect();
    let rank = |m: Mark| match m {
        Mark::Ready => 0,
        Mark::Available => 1,
        Mark::InProgress => 2,
        _ => 3,
    };
    out.sort_by_key(|r| (rank(r.mark), r.quest));
    for service in services.list() {
        out.push(Row {
            quest: 0,
            service: Some(service),
            mark: Mark::None,
            title: service.title().into(),
            level: 0,
        });
    }
    out
}

fn page_for(row: &Row) -> Page {
    if let Some(service) = row.service {
        return Page::Service(service);
    }
    match row.mark {
        Mark::Ready => Page::Reward(row.quest),
        Mark::InProgress => Page::Progress(row.quest),
        _ => Page::Detail(row.quest),
    }
}

/// The page an NPC opens: its one quest, its list, or nothing.
pub fn open(
    book: &QuestBook,
    log: &QuestLog,
    p: &Player,
    npc: Npc,
    npc_name: &str,
    services: Services,
) -> Dialogue {
    let rows = rows(book, log, p, npc, services);
    let page = match rows.as_slice() {
        [] => Page::Nothing,
        [one] => page_for(one),
        _ => Page::List,
    };
    Dialogue {
        npc,
        npc_name: npc_name.to_string(),
        page,
        cursor: 0,
        choice: 0,
        focus: 0,
        services,
        armed: false,
    }
}

/// Jump straight to the offer of `quest` (after a hand-in that chains on to the next quest).
pub fn offer(d: &mut Dialogue, quest: u32) {
    d.page = Page::Detail(quest);
    d.cursor = 0;
    d.choice = 0;
    d.focus = 0;
    d.armed = false;
}

fn choices(book: &QuestBook, quest: u32) -> usize {
    book.quest(quest).map_or(0, |q| q.reward_choice.len())
}

/// Every reward item of a quest: the choices, then the sure ones.
fn rewards(book: &QuestBook, quest: u32) -> usize {
    book.quest(quest)
        .map_or(0, |q| q.reward_choice.len() + q.reward_items.len())
}

/// Moves the focus over the rewards; a choice in focus is the choice, and a move unpicks it.
fn look(d: &mut Dialogue, book: &QuestBook, quest: u32, forward: bool) {
    let total = rewards(book, quest);
    d.focus = if forward {
        (d.focus + 1).min(total.saturating_sub(1))
    } else {
        d.focus.saturating_sub(1)
    };
    d.armed = false;
    if d.focus < choices(book, quest) {
        d.choice = d.focus;
    }
}

/// One key on the current page.
pub fn press(d: &mut Dialogue, key: Key, book: &QuestBook, log: &QuestLog, p: &Player) -> Outcome {
    let rows = rows(book, log, p, d.npc, d.services);
    match (d.page, key) {
        (Page::List, Key::Up) => d.cursor = d.cursor.saturating_sub(1),
        (Page::List, Key::Down) => d.cursor = (d.cursor + 1).min(rows.len().saturating_sub(1)),
        (Page::List, Key::Confirm) => {
            if let Some(row) = rows.get(d.cursor) {
                if let Some(service) = row.service {
                    return Outcome::Service(service);
                }
                d.page = page_for(row);
                d.choice = 0;
                d.focus = 0;
                d.armed = false;
            }
        }
        (Page::Detail(q) | Page::Reward(q), Key::Up | Key::Prev) => look(d, book, q, false),
        (Page::Detail(q) | Page::Reward(q), Key::Down | Key::Next) => look(d, book, q, true),
        (Page::Detail(q), Key::Confirm) => return Outcome::Accept(q),
        (Page::Reward(q), Key::Confirm) => {
            let n = choices(book, q);
            // Handing in is for good: with a choice to make the first press only picks it.
            if n >= 2 && !d.armed {
                d.armed = true;
                return Outcome::None;
            }
            return Outcome::Complete {
                quest: q,
                choice: (n > 0).then(|| d.choice.min(n - 1)),
            };
        }
        (Page::Service(service), Key::Confirm) => return Outcome::Service(service),
        (Page::Progress(_) | Page::Nothing, Key::Confirm) => return Outcome::Close,
        (Page::Detail(_) | Page::Progress(_) | Page::Reward(_), Key::Back) => {
            if rows.len() > 1 {
                d.page = Page::List;
            } else {
                return Outcome::Close;
            }
        }
        (Page::List | Page::Nothing | Page::Service(_), Key::Back) => return Outcome::Close,
        _ => {}
    }
    Outcome::None
}

fn marker(m: Mark) -> &'static str {
    match m {
        Mark::Ready => "?",
        Mark::Available => "!",
        Mark::AvailableLow => "(!)",
        Mark::InProgress => "(?)",
        Mark::None => " ",
    }
}

/// The words of the current page.
pub fn view(
    d: &Dialogue,
    book: &QuestBook,
    log: &QuestLog,
    p: &Player,
    item_name: &dyn Fn(u32) -> String,
) -> View {
    let mut v = View {
        subtitle: d.npc_name.clone(),
        ..View::default()
    };
    let quest = match d.page {
        Page::Detail(q) | Page::Progress(q) | Page::Reward(q) => book.quest(q),
        _ => None,
    };
    match (d.page, quest) {
        (Page::Service(_), _) => {
            v.title = d.npc_name.clone();
            v.subtitle.clear();
            v.footer = "Enter / A  Go on      Backspace / B  Goodbye".into();
        }
        (Page::Nothing, _) => {
            v.title = d.npc_name.clone();
            v.subtitle.clear();
            v.body = "Nothing to say to you just now.".into();
            v.footer = "Enter / A  Goodbye".into();
        }
        (Page::List, _) => {
            v.title = d.npc_name.clone();
            v.subtitle.clear();
            let rows = rows(book, log, p, d.npc, d.services);
            v.list = rows
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let at = if i == d.cursor { ">" } else { " " };
                    if r.service.is_some() {
                        format!("{at} {}", r.title)
                    } else {
                        format!("{at} {} {} ({})", marker(r.mark), r.title, r.level)
                    }
                })
                .collect();
            v.footer = "Up/Down  Choose      Enter / A  Open      Backspace / B  Goodbye".into();
        }
        (Page::Detail(id), Some(q)) => {
            v.title = q.title.clone();
            v.body = expand(&q.details, p);
            v.objectives = vec![expand(&q.objectives, p)];
            fill_rewards(&mut v, q, d);
            let look = if rewards(book, id) > 0 {
                "Up/Down  Look at rewards      "
            } else {
                ""
            };
            v.footer = if book.ender_spawned(id) {
                format!("{look}Enter / A  Accept      Backspace / B  Decline")
            } else {
                format!("{look}Enter / A  Accept (it cannot be handed in inside the valley)      Backspace / B  Decline")
            };
        }
        (Page::Progress(_), Some(q)) => {
            v.title = q.title.clone();
            let ask = if q.request_items.trim().is_empty() {
                &q.objectives
            } else {
                &q.request_items
            };
            v.body = expand(ask, p);
            v.objectives = lines::objective_lines(book, q, log.entry(q.id));
            v.footer = "Enter / A  Goodbye      Backspace / B  Back".into();
        }
        (Page::Reward(id), Some(q)) => {
            v.title = q.title.clone();
            v.body = expand(&q.offer_reward, p);
            fill_rewards(&mut v, q, d);
            let picked = q
                .reward_choice
                .get(d.choice)
                .map_or_else(String::new, |&(item, _)| item_name(item));
            v.footer = if q.reward_choice.len() >= 2 && d.armed {
                format!("Enter / A  Take {picked} (final)      Up/Down  Look again")
            } else if q.reward_choice.len() >= 2 {
                "Up/Down  Look at rewards      Enter / A  Choose      Backspace / B  Back".into()
            } else if rewards(book, id) > 0 {
                "Up/Down  Look at rewards      Enter / A  Complete Quest      Backspace / B  Back"
                    .into()
            } else {
                "Enter / A  Complete Quest      Backspace / B  Back".into()
            };
        }
        _ => {}
    }
    v
}

/// Puts a quest's rewards on the view: the items for the icons, the experience and money line under them,
/// the reward in focus and whether the choice is picked.
fn fill_rewards(v: &mut View, q: &wow_world::model::Quest, d: &Dialogue) {
    v.choices = q.reward_choice.clone();
    v.fixed = q.reward_items.clone();
    let line = lines::reward_line(q);
    if !line.is_empty() {
        v.rewards = vec![line];
    }
    if !v.choices.is_empty() || !v.fixed.is_empty() {
        v.focus = Some(d.focus.min(v.choices.len() + v.fixed.len() - 1));
    }
    v.armed = d.armed;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quests::testing::real_world;

    fn setup() -> Option<(QuestBook, QuestLog, Player)> {
        let w = real_world()?;
        Some((
            QuestBook::new(&w),
            QuestLog::default(),
            Player::warrior("Jacob", 1),
        ))
    }

    fn items(id: u32) -> String {
        format!("item {id}")
    }

    const MCBRIDE: Npc = Npc::Creature(197);
    const WILLEM: Npc = Npc::Creature(823);

    #[test]
    fn an_npc_with_no_business_says_so_and_one_with_one_quest_opens_it() {
        let Some((book, log, p)) = setup() else {
            return;
        };
        // McBride has nothing for a fresh Warrior (783 is Willem's to give).
        let d = open(
            &book,
            &log,
            &p,
            MCBRIDE,
            "Marshal McBride",
            Services::default(),
        );
        assert_eq!(d.page, Page::Nothing);
        assert_eq!(
            press(&mut d.clone(), Key::Confirm, &book, &log, &p),
            Outcome::Close
        );
        // Willem has exactly one: A Threat Within.
        let d = open(
            &book,
            &log,
            &p,
            WILLEM,
            "Deputy Willem",
            Services::default(),
        );
        assert_eq!(d.page, Page::Detail(783));
        let v = view(&d, &book, &log, &p, &items);
        assert_eq!(v.title, "A Threat Within");
        assert_eq!(v.subtitle, "Deputy Willem");
        assert!(
            v.body.contains("Jacob") || v.body.contains("warrior"),
            "{:?}",
            v.body
        );
        assert!(!v.body.contains('$'), "placeholders are filled");
        assert!(v.footer.starts_with("Enter / A  Accept"));
    }

    #[test]
    fn accept_complete_and_the_list_follow_the_log() {
        let Some((book, mut log, p)) = setup() else {
            return;
        };
        let mut d = open(
            &book,
            &log,
            &p,
            WILLEM,
            "Deputy Willem",
            Services::default(),
        );
        assert_eq!(
            press(&mut d, Key::Confirm, &book, &log, &p),
            Outcome::Accept(783)
        );
        log.accept(&book, &p, 783, &|_| 0).unwrap();
        // 783 is complete at once; McBride takes it in: a single Reward page.
        let d = open(
            &book,
            &log,
            &p,
            MCBRIDE,
            "Marshal McBride",
            Services::default(),
        );
        assert_eq!(d.page, Page::Reward(783));
        let mut d2 = d.clone();
        assert_eq!(
            press(&mut d2, Key::Confirm, &book, &log, &p),
            Outcome::Complete {
                quest: 783,
                choice: None
            }
        );
        let v = view(&d, &book, &log, &p, &items);
        assert!(
            v.rewards.iter().any(|l| l.contains("40 experience")),
            "{:?}",
            v.rewards
        );
        // Willem has nothing now (the other quests wait for 783 to be handed in).
        assert_eq!(
            open(
                &book,
                &log,
                &p,
                WILLEM,
                "Deputy Willem",
                Services::default()
            )
            .page,
            Page::Nothing
        );
    }

    #[test]
    fn several_quests_make_a_list_with_a_cursor_and_back_returns_to_it() {
        let Some((book, mut log, p)) = setup() else {
            return;
        };
        // After 783 is handed in, McBride offers 7 (and the Warrior's letter needs 7 first): hand 783 in,
        // take 7 and finish it; now McBride offers 15 and 3100 and takes 7 in.
        log.accept(&book, &p, 783, &|_| 0).unwrap();
        log.turn_in(&book, &p, 783, MCBRIDE, None, &|_| 0).unwrap();
        log.accept(&book, &p, 7, &|_| 0).unwrap();
        for _ in 0..10 {
            log.kill(&book, 6);
        }
        let mut d = open(
            &book,
            &log,
            &p,
            MCBRIDE,
            "Marshal McBride",
            Services::default(),
        );
        assert_eq!(
            d.page,
            Page::Reward(7),
            "the finished quest is the only row: 15 and 3100 need 7 handed in"
        );
        log.turn_in(&book, &p, 7, MCBRIDE, None, &|_| 0).unwrap();
        d = open(
            &book,
            &log,
            &p,
            MCBRIDE,
            "Marshal McBride",
            Services::default(),
        );
        assert_eq!(d.page, Page::List);
        let v = view(&d, &book, &log, &p, &items);
        assert_eq!(v.list.len(), 2, "{:?}", v.list);
        assert!(v.list[0].starts_with("> ! "), "{:?}", v.list);
        assert_eq!(press(&mut d, Key::Down, &book, &log, &p), Outcome::None);
        assert_eq!(d.cursor, 1);
        assert_eq!(press(&mut d, Key::Down, &book, &log, &p), Outcome::None);
        assert_eq!(d.cursor, 1, "clamped");
        assert_eq!(press(&mut d, Key::Confirm, &book, &log, &p), Outcome::None);
        assert!(matches!(d.page, Page::Detail(_)));
        assert_eq!(press(&mut d, Key::Back, &book, &log, &p), Outcome::None);
        assert_eq!(d.page, Page::List);
        assert_eq!(press(&mut d, Key::Back, &book, &log, &p), Outcome::Close);
    }

    #[test]
    fn a_reward_page_chooses_among_the_choices_and_a_quest_in_progress_asks_again() {
        let Some((book, mut log, p)) = setup() else {
            return;
        };
        log.accept(&book, &p, 783, &|_| 0).unwrap();
        log.turn_in(&book, &p, 783, MCBRIDE, None, &|_| 0).unwrap();
        log.accept(&book, &p, 5261, &|_| 0).unwrap();
        log.turn_in(&book, &p, 5261, Npc::Creature(196), None, &|_| 0)
            .unwrap();
        log.accept(&book, &p, 33, &|_| 0).unwrap();
        let eagan = Npc::Creature(196);
        let d = open(
            &book,
            &log,
            &p,
            eagan,
            "Eagan Peltskinner",
            Services::default(),
        );
        assert_eq!(d.page, Page::Progress(33), "the meat is not here yet");
        let v = view(&d, &book, &log, &p, &items);
        assert_eq!(v.objectives, vec!["Tough Wolf Meat: 0/8"]);
        log.item_count(&book, 750, 8);
        let mut d = open(
            &book,
            &log,
            &p,
            eagan,
            "Eagan Peltskinner",
            Services::default(),
        );
        assert_eq!(d.page, Page::Reward(33));
        let v = view(&d, &book, &log, &p, &items);
        assert_eq!(v.choices.len(), 2, "{:?}", v.choices);
        assert_eq!(v.focus, Some(0), "the first reward is in focus");
        assert!(!v.armed);
        assert_eq!(press(&mut d, Key::Down, &book, &log, &p), Outcome::None);
        assert_eq!(d.choice, 1);
        assert_eq!(press(&mut d, Key::Down, &book, &log, &p), Outcome::None);
        assert_eq!(d.choice, 1, "two choices: clamped");
        // Handing in is for good: the first press only picks, looking again unpicks.
        assert_eq!(press(&mut d, Key::Confirm, &book, &log, &p), Outcome::None);
        assert!(d.armed);
        let v = view(&d, &book, &log, &p, &items);
        assert!(v.armed && v.footer.contains("(final)"), "{:?}", v.footer);
        assert_eq!(press(&mut d, Key::Prev, &book, &log, &p), Outcome::None);
        assert!(!d.armed);
        assert_eq!((d.focus, d.choice), (0, 0));
        assert_eq!(press(&mut d, Key::Next, &book, &log, &p), Outcome::None);
        assert_eq!(press(&mut d, Key::Confirm, &book, &log, &p), Outcome::None);
        assert_eq!(
            press(&mut d, Key::Confirm, &book, &log, &p),
            Outcome::Complete {
                quest: 33,
                choice: Some(1)
            }
        );
    }

    #[test]
    fn an_offer_lets_the_rewards_be_looked_at_before_accepting() {
        let Some((book, log, p)) = setup() else {
            return;
        };
        let mut d = open(
            &book,
            &log,
            &p,
            MCBRIDE,
            "Marshal McBride",
            Services::default(),
        );
        // Brotherhood of Thieves: five choices (and whatever comes with it).
        offer(&mut d, 18);
        let v = view(&d, &book, &log, &p, &items);
        assert_eq!(v.choices.len(), 5, "{:?}", v.choices);
        assert_eq!(v.focus, Some(0));
        assert!(
            v.footer.starts_with("Up/Down  Look at rewards"),
            "{:?}",
            v.footer
        );
        let total = v.choices.len() + v.fixed.len();
        for _ in 0..total + 3 {
            assert_eq!(press(&mut d, Key::Next, &book, &log, &p), Outcome::None);
        }
        assert_eq!(d.focus, total - 1, "clamped on the last reward");
        assert_eq!(press(&mut d, Key::Prev, &book, &log, &p), Outcome::None);
        assert_eq!(d.focus, total - 2);
        // Looking changes nothing about accepting: one press does it.
        assert_eq!(
            press(&mut d, Key::Confirm, &book, &log, &p),
            Outcome::Accept(18)
        );
        // A quest with nothing to look at has no focus and no hint.
        let mut d = open(
            &book,
            &log,
            &p,
            WILLEM,
            "Deputy Willem",
            Services::default(),
        );
        offer(&mut d, 783);
        let v = view(&d, &book, &log, &p, &items);
        assert_eq!(v.focus, None);
        assert!(!v.footer.contains("Look at rewards"));
    }

    #[test]
    fn a_parked_quest_warns_at_the_offer() {
        let Some((book, mut log, p)) = setup() else {
            return;
        };
        let mcb = MCBRIDE;
        // Walk to 54: 783, 7, 15, 21 handed in.
        for q in [783, 7, 15, 21] {
            log.accept(&book, &p, q, &|_| 0).unwrap();
            let quest = book.quest(q).unwrap();
            for &(id, need) in &quest.req_kills {
                for _ in 0..need {
                    log.kill(&book, id as u32);
                }
            }
            let c = (!quest.reward_choice.is_empty()).then_some(0);
            log.turn_in(&book, &p, q, mcb, c, &|_| 0).unwrap();
        }
        let d = open(&book, &log, &p, mcb, "Marshal McBride", Services::default());
        let rows = rows(&book, &log, &p, mcb, Services::default());
        assert!(rows.iter().any(|r| r.quest == 54), "{rows:?}");
        let mut d54 = d.clone();
        offer(&mut d54, 54);
        let v = view(&d54, &book, &log, &p, &items);
        assert!(v.footer.contains("cannot be handed in"), "{:?}", v.footer);
    }

    #[test]
    fn a_service_npc_opens_at_once_or_lists_the_way_in_after_its_quests() {
        let Some((book, log, p)) = setup() else {
            return;
        };
        let sells = Services {
            vendor: true,
            ..Services::default()
        };
        // McBride has no quest for a fresh warrior: as a vendor he opens the shop at once.
        let mut d = open(&book, &log, &p, MCBRIDE, "Marshal McBride", sells);
        assert_eq!(d.page, Page::Service(Service::Shop));
        assert_eq!(
            press(&mut d.clone(), Key::Confirm, &book, &log, &p),
            Outcome::Service(Service::Shop)
        );
        assert_eq!(press(&mut d, Key::Back, &book, &log, &p), Outcome::Close);
        // Willem has a quest and wares: the list is the quest, then the shop.
        let mut d = open(&book, &log, &p, WILLEM, "Deputy Willem", sells);
        assert_eq!(d.page, Page::List);
        let v = view(&d, &book, &log, &p, &items);
        assert_eq!(v.list.len(), 2, "{:?}", v.list);
        assert!(v.list[1].contains("browse your goods"), "{:?}", v.list);
        assert_eq!(press(&mut d, Key::Down, &book, &log, &p), Outcome::None);
        assert_eq!(
            press(&mut d, Key::Confirm, &book, &log, &p),
            Outcome::Service(Service::Shop)
        );
        // A banker who also sells and trains lists each way in, in the order shop, bank, train.
        let all = Services {
            vendor: true,
            banker: true,
            trainer: true,
        };
        let d = open(&book, &log, &p, MCBRIDE, "Marshal McBride", all);
        assert_eq!(d.page, Page::List);
        let v = view(&d, &book, &log, &p, &items);
        assert_eq!(v.list.len(), 3, "{:?}", v.list);
        assert!(v.list[1].contains("strongbox") && v.list[2].contains("train"));
        // Without services there is no extra row.
        let d = open(
            &book,
            &log,
            &p,
            WILLEM,
            "Deputy Willem",
            Services::default(),
        );
        assert_eq!(d.page, Page::Detail(783));
    }
}
