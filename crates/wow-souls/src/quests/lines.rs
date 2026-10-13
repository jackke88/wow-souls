//! The words of the quest screens, pure: objective lines with their counters, reward lines, copper as
//! gold / silver / copper, XP progress.

use wow_world::model::Quest;
use wow_world::quest::{Entry, QuestBook, Status};

/// `Kobold Vermin slain: 3/10`, `Tough Wolf Meat: 5/8`: one line per objective of `quest` in the log
/// entry `e` (an entry that is not in the log counts nothing). The quest's own objective text for a slot
/// (`ObjectiveText1-4`) wins when it has one.
pub fn objective_lines(book: &QuestBook, quest: &Quest, e: Option<&Entry>) -> Vec<String> {
    let mut out = Vec::new();
    for (i, &(id, need)) in quest.req_kills.iter().enumerate().take(4) {
        let now = e.map_or(0, |e| e.kills[i]).min(need);
        let name = match quest.objective_text.get(i).filter(|t| !t.is_empty()) {
            Some(t) => t.clone(),
            None if id >= 0 => format!(
                "{} slain",
                book.creature_name(id as u32).unwrap_or("Creature")
            ),
            None => "Object used".to_string(),
        };
        out.push(format!("{name}: {now}/{need}"));
    }
    for (i, &(item, need)) in quest.req_items.iter().enumerate().take(4) {
        let now = e.map_or(0, |e| e.items[i]).min(need);
        let name = book.item_name(item).unwrap_or("Item");
        out.push(format!("{name}: {now}/{need}"));
    }
    out
}

/// `Reward: 170 experience, 100 souls` (the souls only when there are some: the quest's copper, paid in
/// souls at [`wow_world::shop::SOULS_PER_COPPER`]).
pub fn reward_line(quest: &Quest) -> String {
    let mut parts = Vec::new();
    if quest.reward_xp > 0 {
        parts.push(format!("{} experience", quest.reward_xp));
    }
    if quest.reward_money != 0 {
        parts.push(format!(
            "{} souls",
            wow_world::shop::souls_for_copper(quest.reward_money.max(0) as u32)
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("Reward: {}", parts.join(", "))
    }
}

/// How a quest stands, for the log: `Complete`, `Handed in`, or empty while it is open.
pub fn status_word(e: &Entry) -> &'static str {
    match e.status {
        Status::Incomplete => "",
        Status::Complete => "Complete",
        Status::Rewarded => "Handed in",
    }
}

/// `Level 2: 105/900 experience` from the XP table's `(level, into, need)`; the cap shows no fraction.
pub fn level_line(level: u32, into: u32, need: u32) -> String {
    if need == 0 {
        format!("Level {level}")
    } else {
        format!("Level {level}: {into}/{need} experience")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::quest::{Entry, Status};

    #[test]
    fn level_and_reward_lines() {
        assert_eq!(level_line(2, 105, 900), "Level 2: 105/900 experience");
        assert_eq!(level_line(60, 0, 0), "Level 60");
        let status = |s| Entry::new(s);
        assert_eq!(status_word(&status(Status::Incomplete)), "");
        assert_eq!(status_word(&status(Status::Complete)), "Complete");
        assert_eq!(status_word(&status(Status::Rewarded)), "Handed in");
    }

    #[test]
    fn objective_lines_show_counters_names_and_cap() {
        let Some(w) = crate::quests::testing::real_world() else {
            return;
        };
        let book = QuestBook::new(&w);
        let q = book.quest(7).unwrap();
        let mut e = Entry::new(Status::Incomplete);
        e.kills[0] = 3;
        assert_eq!(
            objective_lines(&book, q, Some(&e)),
            vec!["Kobold Vermin slain: 3/10"]
        );
        assert_eq!(
            objective_lines(&book, q, None),
            vec!["Kobold Vermin slain: 0/10"]
        );
        let q = book.quest(33).unwrap();
        let mut e = Entry::new(Status::Incomplete);
        e.items[0] = 5;
        assert_eq!(
            objective_lines(&book, q, Some(&e)),
            vec!["Tough Wolf Meat: 5/8"]
        );
        assert_eq!(
            reward_line(book.quest(7).unwrap()),
            "Reward: 170 experience, 100 souls"
        );
        assert_eq!(
            reward_line(book.quest(783).unwrap()),
            "Reward: 40 experience"
        );
    }
}
