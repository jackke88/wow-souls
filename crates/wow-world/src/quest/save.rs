//! The quest part of a save: the log, the XP, the copper and the player's name, as plain text. The items are
//! NOT here: `wow_world::inventory` owns them and has its own save (`items-v1.txt`); the game saves both
//! together (G3c merges the two saves).
//!
//! ```text
//! wsquests 1
//! name<TAB>Adventurer
//! xp<TAB>1205
//! money<TAB>25
//! quest<TAB>7<TAB>complete<TAB>10 0 0 0<TAB>0 0 0 0
//! ```

use anyhow::{anyhow, bail, Context, Result};
use std::collections::BTreeSet;

use super::log::{Entry, QuestLog, Status};

/// What a quest save holds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SaveGame {
    pub log: QuestLog,
    /// All the XP ever earned (the level follows from it: `XpTable::level_for`).
    pub xp: u32,
    /// Copper.
    pub money: i64,
    pub name: String,
}

const HEADER: &str = "wsquests 1";

fn status_word(s: Status) -> &'static str {
    match s {
        Status::Incomplete => "incomplete",
        Status::Complete => "complete",
        Status::Rewarded => "rewarded",
    }
}

fn counts(c: &[u32; 4]) -> String {
    format!("{} {} {} {}", c[0], c[1], c[2], c[3])
}

fn parse_counts(s: &str) -> Result<[u32; 4]> {
    let v: Vec<u32> = s
        .split(' ')
        .map(|n| n.parse::<u32>().map_err(|e| anyhow!("count `{n}`: {e}")))
        .collect::<Result<_>>()?;
    <[u32; 4]>::try_from(v).map_err(|v| anyhow!("{} counts, expected 4", v.len()))
}

impl SaveGame {
    pub fn to_text(&self) -> String {
        // A name with a tab or a line break would break the line format: those characters become spaces.
        let name: String = self
            .name
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let mut out = format!(
            "{HEADER}\nname\t{name}\nxp\t{}\nmoney\t{}\n",
            self.xp, self.money
        );
        for (quest, e) in self.log.entries() {
            out += &format!(
                "quest\t{quest}\t{}\t{}\t{}\n",
                status_word(e.status),
                counts(&e.kills),
                counts(&e.items)
            );
        }
        out
    }

    /// Strict: a wrong header, an unknown line, a bad number or a duplicate quest is an error naming the line.
    pub fn from_text(text: &str) -> Result<SaveGame> {
        let mut lines = text.lines();
        match lines.next() {
            Some(HEADER) => {}
            other => bail!("not a quest save: first line {other:?}, expected {HEADER:?}"),
        }
        let mut save = SaveGame::default();
        let mut seen = BTreeSet::new();
        let mut have = (false, false, false);
        for (n, line) in lines.enumerate().map(|(i, l)| (i + 2, l)) {
            let f: Vec<&str> = line.split('\t').collect();
            let ctx = || format!("line {n}: {line:?}");
            match f.as_slice() {
                ["name", name] => {
                    save.name = (*name).to_string();
                    have.0 = true;
                }
                ["xp", v] => {
                    save.xp = v.parse().with_context(ctx)?;
                    have.1 = true;
                }
                ["money", v] => {
                    save.money = v.parse().with_context(ctx)?;
                    have.2 = true;
                }
                ["quest", id, status, kills, items] => {
                    let id: u32 = id.parse().with_context(ctx)?;
                    if !seen.insert(id) {
                        bail!("{}: quest {id} twice", ctx());
                    }
                    let status = match *status {
                        "incomplete" => Status::Incomplete,
                        "complete" => Status::Complete,
                        "rewarded" => Status::Rewarded,
                        other => bail!("{}: status `{other}`", ctx()),
                    };
                    save.log.insert(
                        id,
                        Entry {
                            status,
                            kills: parse_counts(kills).with_context(ctx)?,
                            items: parse_counts(items).with_context(ctx)?,
                        },
                    );
                }
                _ => bail!("{}: unknown line", ctx()),
            }
        }
        if !(have.0 && have.1 && have.2) {
            bail!("the save lacks its name, xp or money line");
        }
        Ok(save)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SaveGame {
        let mut log = QuestLog::default();
        log.insert(
            7,
            Entry {
                status: Status::Complete,
                kills: [10, 0, 0, 0],
                items: [0; 4],
            },
        );
        log.insert(
            33,
            Entry {
                status: Status::Incomplete,
                kills: [0; 4],
                items: [5, 0, 0, 0],
            },
        );
        log.insert(783, Entry::new(Status::Rewarded));
        SaveGame {
            log,
            xp: 1205,
            money: 65,
            name: "Adventurer".into(),
        }
    }

    #[test]
    fn a_save_round_trips() {
        let s = sample();
        let text = s.to_text();
        assert!(text.starts_with("wsquests 1\nname\tAdventurer\nxp\t1205\nmoney\t65\n"));
        assert!(text.contains("quest\t7\tcomplete\t10 0 0 0\t0 0 0 0\n"));
        assert_eq!(SaveGame::from_text(&text).unwrap(), s);
        assert_eq!(s.to_text(), SaveGame::from_text(&text).unwrap().to_text());
    }

    #[test]
    fn a_name_cannot_break_the_format() {
        let mut s = sample();
        s.name = "A\tB\nC".into();
        let back = SaveGame::from_text(&s.to_text()).unwrap();
        assert_eq!(back.name, "A B C");
        assert_eq!(back.log, s.log);
    }

    #[test]
    fn a_corrupt_save_is_refused_with_its_line() {
        let good = sample().to_text();
        let cases = [
            ("wsquests 2\n".to_string(), "not a quest save"),
            (good.replace("complete", "done"), "status `done`"),
            (good.replace("10 0 0 0", "10 0 0"), "3 counts"),
            (good.replace("xp\t1205", "xp\tmany"), "line 3"),
            (
                format!("{good}quest\t7\tcomplete\t0 0 0 0\t0 0 0 0\n"),
                "quest 7 twice",
            ),
            (format!("{good}surprise\t1\n"), "unknown line"),
            (good.replace("money\t65\n", ""), "lacks"),
            (String::new(), "not a quest save"),
        ];
        for (text, want) in cases {
            let e = format!("{:#}", SaveGame::from_text(&text).unwrap_err());
            assert!(e.contains(want), "{want:?} not in {e:?}");
        }
    }
}
