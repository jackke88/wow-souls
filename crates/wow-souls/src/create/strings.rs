//! The 1.12 glue string table: the creation screen's own text (the race and class paragraphs, the
//! customization labels, the button captions), read from the player's install at start, never shipped.
//!
//! `Interface\GlueXML\GlueStrings.lua` is plain `KEY = "value";` lines, parsed without a Lua VM
//! (the fork's `benilla-app/src/glue_strings.rs` does the same; see `docs/PROVENANCE.md`). The locale
//! patch (`GlueLocalization.lua`'s `Localize()`) only touches login texts, so it is not read.

use std::collections::HashMap;

use bevy::prelude::*;

/// The file in the patch chain.
pub const GLUE_STRINGS: &str = "Interface\\GlueXML\\GlueStrings.lua";

/// The glue strings by key; empty without client data, and callers fall back to built-in captions.
#[derive(Resource, Default, Debug, Clone)]
pub struct Strings(HashMap<String, String>);

impl Strings {
    pub fn from_source(src: &str) -> Strings {
        Strings(parse(src))
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// The string for a key, or the built-in caption.
    pub fn text<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.get(key).unwrap_or(fallback)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[cfg(test)]
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Strings {
        Strings(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        )
    }
}

/// Parses the `KEY = "value";` lines, unfolding `\n`, `\t`, `\"` and `\\`; anything else is skipped
/// (comments, code, a string whose quote never closes).
pub fn parse(src: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in src.lines() {
        let Some((key, rest)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let Some(rest) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        let mut value = String::new();
        let mut chars = rest.chars();
        let mut closed = false;
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    closed = true;
                    break;
                }
                '\\' => match chars.next() {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some('r') => {}
                    Some(other) => value.push(other),
                    None => break,
                },
                other => value.push(other),
            }
        }
        if closed {
            out.insert(key.to_string(), value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_assignments_and_skips_the_rest() {
        let src = r#"
MALE = "Male";
FACTION_INFO_HORDE = "Four races\ncomprise the Horde";
QUOTED = "a \"b\" c";
-- a comment = "not a string";
CODE = getglobal("nope");
BROKEN = "no close
"#;
        let t = Strings::from_source(src);
        assert_eq!(t.get("MALE"), Some("Male"));
        assert_eq!(
            t.get("FACTION_INFO_HORDE"),
            Some("Four races\ncomprise the Horde")
        );
        assert_eq!(t.get("QUOTED"), Some(r#"a "b" c"#));
        assert_eq!(t.get("CODE"), None);
        assert_eq!(t.get("BROKEN"), None);
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn a_missing_key_takes_the_built_in_caption() {
        let t = Strings::from_pairs(&[("MALE", "Male")]);
        assert_eq!(t.text("MALE", "x"), "Male");
        assert_eq!(t.text("FEMALE", "Female"), "Female");
    }

    /// The real file: every key the screen reads for a human warrior resolves. Skips without data.
    #[test]
    fn the_install_has_the_keys_the_screen_reads() {
        let Some(data) = benilla_formats::wow_data() else {
            eprintln!("skipped: no WoW install");
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).expect("open chain");
        let bytes = chain.read_file(GLUE_STRINGS).expect("GlueStrings.lua");
        let t = Strings::from_source(&String::from_utf8_lossy(&bytes));
        for key in [
            "RACE_INFO_HUMAN",
            "ABILITY_INFO_HUMAN1",
            "CLASS_WARRIOR",
            "FACTION_INFO_ALLIANCE",
            "FACTION_INFO_HORDE",
            "ALLIANCE",
            "HORDE",
            "MALE",
            "FEMALE",
            "RANDOMIZE",
            "NAME",
            "BACK",
            "HAIR_NORMAL_STYLE",
            "HAIR_NORMAL_COLOR",
            "FACIAL_HAIR_NORMAL",
            "CHAR_CUSTOMIZATION1_DESC",
            "CHAR_CUSTOMIZATION2_DESC",
        ] {
            assert!(t.get(key).is_some(), "{key} is missing");
        }
        assert!(
            t.get("RACE_INFO_HUMAN").unwrap().contains("Humans"),
            "the human paragraph"
        );
    }
}
