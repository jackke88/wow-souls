//! The quest texts with their placeholders filled, as the 1.12 client does it (report section 3): `$B` / `$b` a
//! line break, `$N` / `$n` the player's name, `$C` the class word capitalised and `$c` in lower case, `$R` /
//! `$r` the race the same way, and the gender switch `$gmale:female;`. An unknown `$x` form is left as it is
//! and reported by [`expand_checked`], so a test over all the texts can say whether one was missed.

use super::Player;

/// The text with its placeholders filled in for `p`.
pub fn expand(text: &str, p: &Player) -> String {
    expand_checked(text, p).0
}

/// The expanded text and the placeholders it did not understand (left in the text unchanged).
pub fn expand_checked(text: &str, p: &Player) -> (String, Vec<String>) {
    let mut out = String::with_capacity(text.len() + 16);
    let mut unknown = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let mut chars = tail.chars();
        let Some(tag) = chars.next() else {
            out.push('$');
            unknown.push("$".to_string());
            rest = "";
            break;
        };
        let after = &tail[tag.len_utf8()..];
        match tag {
            'N' | 'n' => {
                out.push_str(&p.name);
                rest = after;
            }
            'C' => {
                out.push_str(p.class.name());
                rest = after;
            }
            'c' => {
                out.push_str(&p.class.name().to_lowercase());
                rest = after;
            }
            'R' => {
                out.push_str(p.race.name());
                rest = after;
            }
            'r' => {
                out.push_str(&p.race.name().to_lowercase());
                rest = after;
            }
            'B' | 'b' => {
                out.push('\n');
                rest = after;
            }
            'g' | 'G' => match after.find(';').map(|end| (&after[..end], end)) {
                Some((pair, end)) if pair.contains(':') => {
                    let (male, female) = pair.split_once(':').unwrap_or((pair, ""));
                    out.push_str(if p.male { male } else { female });
                    rest = &after[end + 1..];
                }
                _ => {
                    out.push('$');
                    out.push(tag);
                    unknown.push(format!("${tag}"));
                    rest = after;
                }
            },
            other => {
                out.push('$');
                out.push(other);
                unknown.push(format!("${other}"));
                rest = after;
            }
        }
    }
    out.push_str(rest);
    (out, unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest::testing::real_world;
    use crate::quest::{Class, Race};

    fn hero() -> Player {
        Player::warrior("Jacob", 1)
    }

    #[test]
    fn name_class_race_and_breaks_are_filled() {
        let p = hero();
        assert_eq!(
            expand("Hail, $N.$B$BI am a $c. $C!", &p),
            "Hail, Jacob.\n\nI am a warrior. Warrior!"
        );
        assert_eq!(expand("$n the $r $R", &p), "Jacob the human Human");
        assert_eq!(expand("a$bb", &p), "a\nb");
    }

    #[test]
    fn the_gender_switch_picks_by_the_players_gender() {
        let mut p = hero();
        assert_eq!(
            expand("Greetings, $gbrother:sister;.", &p),
            "Greetings, brother."
        );
        p.male = false;
        assert_eq!(
            expand("Greetings, $gbrother:sister;.", &p),
            "Greetings, sister."
        );
        assert_eq!(
            expand("a $gman:woman; and a $gpriest:priestess;", &p),
            "a woman and a priestess"
        );
    }

    #[test]
    fn a_name_with_a_dollar_is_not_expanded_again() {
        let p = Player {
            name: "Mr$N".into(),
            male: true,
            class: Class::Mage,
            race: Race::Gnome,
            level: 1,
        };
        assert_eq!(expand("$N and $c", &p), "Mr$N and mage");
    }

    #[test]
    fn unknown_forms_are_left_alone_and_reported() {
        let p = hero();
        let (text, unknown) = expand_checked("a $x b $gbrother c $", &p);
        assert_eq!(text, "a $x b $gbrother c $");
        assert_eq!(unknown, vec!["$x", "$g", "$"]);
        let (text, unknown) = expand_checked("plain text, 5$ and a line\nfeed", &p);
        assert_eq!(text, "plain text, 5$ and a line\nfeed");
        assert_eq!(unknown, vec!["$ "], "a dollar and the character after it");
    }

    #[test]
    fn install_no_text_of_the_valleys_quests_has_a_placeholder_the_expander_misses() {
        let Some(w) = real_world() else { return };
        let p = hero();
        let mut seen = 0;
        for q in &w.quests {
            let fields = [
                &q.title,
                &q.details,
                &q.objectives,
                &q.offer_reward,
                &q.request_items,
                &q.end_text,
            ];
            for f in fields.into_iter().chain(q.objective_text.iter()) {
                let (text, unknown) = expand_checked(f, &p);
                assert!(unknown.is_empty(), "quest {}: {unknown:?} in {f:?}", q.id);
                assert!(
                    !text.contains('$'),
                    "quest {} still has a $ after expanding",
                    q.id
                );
                seen += usize::from(f.contains('$'));
            }
        }
        assert!(
            seen > 50,
            "the valley's texts use many placeholders ({seen})"
        );
    }
}
