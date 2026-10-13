//! The player's reaction kit: which Dark Souls animation is a flinch, a knockdown or a death for the
//! player (`c0000`, the `a00` set). Hand-made, like the weapon classes. The ids come from the
//! install's TAE files: `a00_2000`-`2003` is a 0.83 s flinch in four directions, `2004`-`2007` a 1.9 s
//! one, `2010`-`2013` a 1.97 s one, `2016`/`2017` and `2026`/`2027` knockdowns (6.4 and 3.2-3.7 yd of
//! root travel straight back or forward, an invulnerable window while down, a get-up at the end).
//! Which clip is which direction is measured from the head's push ([`crate::monster::push`]) or, for a
//! knockdown, from its travel; which blow level answers which class is the game's table.
//!
//! Left out: the knockdowns `2014`/`2015` and `2024`/`2025`, which travel sideways and turn the body
//! 90 degrees (root yaw), and the flag-less `2200`-`2213`, which do not retarget (their bones drift
//! 90-180 degrees from the bind pose: no ordinary pose). The player's death is a knockdown clip held
//! on the frame where it lies.

/// One reaction clip and the class it belongs to.
pub struct React {
    pub class: &'static str,
    pub clip: &'static str,
}

const fn react(class: &'static str, clip: &'static str) -> React {
    React { class, clip }
}

pub const REACTS: &[React] = &[
    react("s", "a00_2000"),
    react("s", "a00_2001"),
    react("s", "a00_2002"),
    react("s", "a00_2003"),
    react("m", "a00_2004"),
    react("m", "a00_2005"),
    react("m", "a00_2006"),
    react("m", "a00_2007"),
    react("l", "a00_2010"),
    react("l", "a00_2011"),
    react("l", "a00_2012"),
    react("l", "a00_2013"),
    react("down", "a00_2016"),
    react("down", "a00_2017"),
    react("down2", "a00_2026"),
    react("down2", "a00_2027"),
];

/// The death candidates: knockdown clips (also in [`REACTS`]) held on the frame where they lie; the way the
/// body travels decides which one a blow gets (back for a blow from the front, forward from behind).
pub const DEATHS: &[&str] = &["a00_2026", "a00_2027"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kit_names_distinct_clips_in_five_classes() {
        let mut clips: Vec<_> = REACTS.iter().map(|r| r.clip).collect();
        assert_eq!(clips.len(), 16);
        clips.sort_unstable();
        clips.dedup();
        assert_eq!(clips.len(), 16, "every clip once");
        assert!(clips.iter().all(|c| c.starts_with("a00_2")));
        let mut classes: Vec<_> = REACTS.iter().map(|r| r.class).collect();
        classes.sort_unstable();
        classes.dedup();
        assert_eq!(classes, ["down", "down2", "l", "m", "s"]);
        for (class, n) in [("s", 4), ("m", 4), ("l", 4), ("down", 2), ("down2", 2)] {
            assert_eq!(REACTS.iter().filter(|r| r.class == class).count(), n);
        }
        // Each death is one of the reactions.
        assert!(DEATHS.iter().all(|d| REACTS.iter().any(|r| r.clip == *d)));
    }
}
