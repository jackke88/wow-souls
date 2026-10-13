//! WoW's character-creation screen, drawn in Dark Souls parchment: every race, both sexes, every class of
//! each race and the five customization dials, with a live 3D preview; only the Human Warrior (male) can
//! be made yet ([`rules::Selection::playable`]). The layout and the behaviour are 1.12's own
//! (`CharacterCreate.xml` / `.lua`, read as the fork's `benilla-app/src/char_create` does; see
//! `docs/PROVENANCE.md`), the text is the client's `GlueStrings.lua`, the art its `Glues\CharacterCreate`.
//!
//! The flow state (title, focus ring, outcomes, the unattended script) is [`crate::menu`]; this module is
//! the rules ([`rules`], pure), the client data ([`CreateData`]) and the screen ([`ui`], [`preview`]).

pub mod art;
pub mod preview;
pub mod rules;
pub mod strings;
pub mod ui;

use std::sync::Arc;

use bevy::prelude::*;

use benilla_assets::{AssetSet, LockRecover, WorldAssets};
use benilla_formats::CharCreateCatalog;

pub use rules::Rules;
pub use strings::Strings;

/// What the creation screen reads from the client: the catalog (bodies, classes per race, dial ranges,
/// starting outfits), the glue strings and the plain rules built from the catalog.
#[derive(Resource, Clone)]
pub struct CreateData {
    pub catalog: Arc<CharCreateCatalog>,
    pub rules: Arc<Rules>,
    pub strings: Arc<Strings>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load.after(AssetSet::Open))
        .add_plugins((ui::plugin, preview::plugin));
}

fn load(mut commands: Commands, world: Option<Res<WorldAssets>>) {
    let Some(world) = world else {
        warn!("create: no client data, the creation screen stays the plain list");
        return;
    };
    let mut chain = world.chain.lock_recover();
    let catalog = match CharCreateCatalog::load(&mut chain) {
        Ok(c) => c,
        Err(e) => {
            warn!("create: the character catalog is unavailable ({e:#}); the plain list stays");
            return;
        }
    };
    let strings = match chain.read_file(strings::GLUE_STRINGS) {
        Ok(bytes) => Strings::from_source(&String::from_utf8_lossy(&bytes)),
        Err(e) => {
            warn!("create: GlueStrings.lua unavailable ({e:#}); built-in captions only");
            Strings::default()
        }
    };
    // The game has the Northshire Valley and the Human start only: the other races are shown grey and
    // cannot be chosen until each gets its own start zone (and its own backdrop).
    // `WOWSOULS_CREATE_ALL=1` lifts it (a look at every body; the instruments of the screen use it).
    let all = std::env::var("WOWSOULS_CREATE_ALL").is_ok_and(|v| v.trim() == "1");
    let mut rules = Rules::from_catalog(&catalog);
    if !all {
        rules = rules.only_races(&[1]);
    }
    info!(
        "create: catalog loaded ({} glue strings), {}",
        strings.len(),
        if all {
            "every race can be chosen"
        } else {
            "the Human only can be chosen"
        }
    );
    commands.insert_resource(CreateData {
        catalog: Arc::new(catalog),
        rules: Arc::new(rules),
        strings: Arc::new(strings),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real client tables: every race and sex has a body and dials to turn, the class lists are 1.12's.
    #[test]
    fn the_install_catalog_has_every_body_and_the_1_12_class_lists() {
        let Some(data) = benilla_formats::wow_data() else {
            eprintln!("skipped: no WoW install");
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).expect("open chain");
        let catalog = CharCreateCatalog::load(&mut chain).expect("catalog");
        let rules = Rules::from_catalog(&catalog);
        for race in 1..=8u8 {
            for sex in 0..=1u8 {
                assert!(
                    catalog.body_display(race, sex).is_some(),
                    "race {race} sex {sex} has a body"
                );
                let r = rules.ranges(race, sex);
                assert!(r.iter().all(|n| *n >= 1), "{race}/{sex}: {r:?}");
                assert!(r[0] > 1 && r[1] > 1, "skin and face have steps: {r:?}");
            }
        }
        assert_eq!(rules.classes(1), [1, 2, 4, 5, 8, 9], "human");
        assert_eq!(rules.classes(2), [1, 3, 4, 7, 9], "orc");
        assert_eq!(rules.classes(6), [1, 3, 7, 11], "tauren");
        assert!(
            !catalog.start_outfit(1, 1, 0).is_empty(),
            "a human warrior has a starting outfit"
        );
    }
}
