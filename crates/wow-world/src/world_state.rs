//! The world's memory between runs, `wsstate 1`: which never-respawning creatures are dead (`slain`, by
//! spawn guid), which creature entries have given their first-kill drop (`first`), and the loot a dead
//! never-respawning creature still carries (`held`). Plain text beside the other saves; a line that
//! cannot be read is skipped (and reported), the rest still loads.

use std::collections::{BTreeMap, BTreeSet};

pub const MAGIC: &str = "wsstate";
pub const VERSION: &str = "1";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorldState {
    /// Spawn guids of never-respawning creatures that are dead for good.
    pub slain: BTreeSet<u32>,
    /// Creature entries whose first-kill best drop has been given.
    pub first: BTreeSet<u32>,
    /// A dead never-respawning creature's unlooted items (spawn guid -> (item, count) pairs).
    pub held: BTreeMap<u32, Vec<(u32, u32)>>,
}

impl WorldState {
    /// The text, in a stable order: header, slain, first, held.
    pub fn write(&self) -> String {
        let mut out = format!("{MAGIC} {VERSION}\n");
        for g in &self.slain {
            out.push_str(&format!("slain {g}\n"));
        }
        for e in &self.first {
            out.push_str(&format!("first {e}\n"));
        }
        for (g, items) in &self.held {
            out.push_str(&format!("held {g}"));
            for (item, count) in items {
                out.push_str(&format!(" {item} {count}"));
            }
            out.push('\n');
        }
        out
    }

    /// Reads a text. A file that does not start with the header is not ours (empty state, reported); a
    /// line that cannot be read is skipped and reported, the rest loads.
    pub fn parse(text: &str) -> (WorldState, Vec<String>) {
        let mut s = WorldState::default();
        let mut skipped = Vec::new();
        let mut lines = text.lines();
        let Some(head) = lines.next() else {
            return (s, skipped);
        };
        if head.trim() != format!("{MAGIC} {VERSION}") {
            skipped.push(format!("not a {MAGIC} {VERSION} file: {head:?}"));
            return (s, skipped);
        }
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let mut w = line.split_whitespace();
            let ok = match w.next() {
                Some("slain") => w
                    .next()
                    .and_then(|g| g.parse().ok())
                    .map(|g| s.slain.insert(g))
                    .is_some(),
                Some("first") => w
                    .next()
                    .and_then(|e| e.parse().ok())
                    .map(|e| s.first.insert(e))
                    .is_some(),
                Some("held") => {
                    let guid: Option<u32> = w.next().and_then(|g| g.parse().ok());
                    let nums: Option<Vec<u32>> = w.map(|n| n.parse().ok()).collect();
                    match (guid, nums) {
                        (Some(g), Some(n)) if !n.is_empty() && n.len() % 2 == 0 => {
                            s.held
                                .insert(g, n.chunks(2).map(|c| (c[0], c[1])).collect());
                            true
                        }
                        _ => false,
                    }
                }
                _ => false,
            };
            if !ok {
                skipped.push(line.to_string());
            }
        }
        (s, skipped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_round_trips_in_a_stable_order() {
        let mut s = WorldState::default();
        s.slain.insert(80247);
        s.slain.insert(12);
        s.first.insert(103);
        s.held.insert(80247, vec![(2057, 1), (182, 1)]);
        let text = s.write();
        assert_eq!(
            text,
            "wsstate 1\nslain 12\nslain 80247\nfirst 103\nheld 80247 2057 1 182 1\n"
        );
        let (back, skipped) = WorldState::parse(&text);
        assert_eq!(back, s);
        assert!(skipped.is_empty());
    }

    #[test]
    fn a_bad_line_is_skipped_and_reported_and_the_rest_loads() {
        let text =
            "wsstate 1\nslain 5\nslain notanumber\nheld 7 9\nfirst 3\nwhat is this\nheld 8 100 2\n";
        let (s, skipped) = WorldState::parse(text);
        assert_eq!(s.slain.iter().copied().collect::<Vec<_>>(), vec![5]);
        assert_eq!(s.first.iter().copied().collect::<Vec<_>>(), vec![3]);
        assert_eq!(s.held.get(&8), Some(&vec![(100, 2)]));
        assert!(
            !s.held.contains_key(&7),
            "an odd item / count list is not read"
        );
        assert_eq!(skipped.len(), 3, "{skipped:?}");
    }

    #[test]
    fn an_empty_or_foreign_text_is_an_empty_state() {
        assert_eq!(WorldState::parse("").0, WorldState::default());
        let (s, skipped) = WorldState::parse("not a state file\nslain 3\n");
        assert!(
            s.slain.is_empty(),
            "the header decides: another file is not ours"
        );
        assert!(!skipped.is_empty());
    }
}
