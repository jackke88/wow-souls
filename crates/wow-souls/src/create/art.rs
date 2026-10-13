//! The creation screen's art: the client's own icon sheets from `Interface\Glues\CharacterCreate`, loaded
//! as sprites through the engine's `mpq://` source, and the texcoord tables of `CharacterCreate.lua`.
//!
//! A sheet is one texture holding a grid of 64 x 64 icons; the buttons show one cell (`ImageNode::rect`),
//! so the sheets' pixel sizes are constants (checked against the files' headers in a test).

use bevy::prelude::*;

const DIR: &str = "Interface\\Glues\\CharacterCreate\\";

pub const RACES: &str = "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Races";
pub const CLASSES: &str = "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Classes";
pub const GENDER: &str = "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Gender";
pub const FACTIONS: &str = "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Factions";
pub const BANNERS: &str = "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Banners";
pub const ROTATE_UP: &str = "Interface\\Glues\\CharacterCreate\\UI-RotationRight-Big-Up";
pub const ROTATE_DOWN: &str = "Interface\\Glues\\CharacterCreate\\UI-RotationRight-Big-Down";

/// The icon sheets' pixel sizes.
pub const RACES_SIZE: Vec2 = Vec2::new(256.0, 256.0);
pub const CLASSES_SIZE: Vec2 = Vec2::new(256.0, 256.0);
pub const GENDER_SIZE: Vec2 = Vec2::new(128.0, 64.0);
pub const FACTIONS_SIZE: Vec2 = Vec2::new(128.0, 64.0);

/// A texcoord rectangle `[left, right, top, bottom]` as the Lua tables write it.
pub type Tc = [f32; 4];

/// `RACE_ICON_TCOORDS`: the four columns are Human, Dwarf, Gnome, Night Elf over Tauren, Undead, Troll,
/// Orc; the female rows are the same two rows lower.
pub fn race_tc(race: u8, sex: u8) -> Option<Tc> {
    let (col, row) = match race {
        1 => (0.0, 0.0), // Human
        3 => (1.0, 0.0), // Dwarf
        7 => (2.0, 0.0), // Gnome
        4 => (3.0, 0.0), // Night Elf
        6 => (0.0, 1.0), // Tauren
        5 => (1.0, 1.0), // Undead ("SCOURGE")
        8 => (2.0, 1.0), // Troll
        2 => (3.0, 1.0), // Orc
        _ => return None,
    };
    let row = row + if sex == 0 { 0.0 } else { 2.0 };
    Some([
        col * 0.25,
        (col + 1.0) * 0.25,
        row * 0.25,
        (row + 1.0) * 0.25,
    ])
}

/// `CLASS_ICON_TCOORDS`, verbatim (the columns are 63.5 pixels wide, not 64).
pub fn class_tc(class: u8) -> Option<Tc> {
    const C0: (f32, f32) = (0.0, 0.25);
    const C1: (f32, f32) = (0.25, 0.49609375);
    const C2: (f32, f32) = (0.49609375, 0.7421875);
    const C3: (f32, f32) = (0.7421875, 0.98828125);
    const R0: (f32, f32) = (0.0, 0.25);
    const R1: (f32, f32) = (0.25, 0.5);
    const R2: (f32, f32) = (0.5, 0.75);
    let ((l, r), (t, b)) = match class {
        1 => (C0, R0),  // Warrior
        8 => (C1, R0),  // Mage
        4 => (C2, R0),  // Rogue
        11 => (C3, R0), // Druid
        3 => (C0, R1),  // Hunter
        7 => (C1, R1),  // Shaman
        5 => (C2, R1),  // Priest
        9 => (C3, R1),  // Warlock
        2 => (C0, R2),  // Paladin
        _ => return None,
    };
    Some([l, r, t, b])
}

/// The gender sheet's two halves: male, female.
pub fn gender_tc(sex: u8) -> Tc {
    if sex == 0 {
        [0.0, 0.5, 0.0, 1.0]
    } else {
        [0.5, 1.0, 0.0, 1.0]
    }
}

/// The faction sheet's two halves: Alliance, Horde.
pub fn faction_tc(alliance: bool) -> Tc {
    if alliance {
        [0.0, 0.5, 0.0, 1.0]
    } else {
        [0.5, 1.0, 0.0, 1.0]
    }
}

/// A texcoord rectangle on a sheet of `size` pixels, as the pixel rectangle `ImageNode::rect` takes.
pub fn rect(size: Vec2, tc: Tc) -> Rect {
    Rect::new(
        tc[0] * size.x,
        tc[2] * size.y,
        tc[1] * size.x,
        tc[3] * size.y,
    )
}

/// The sheets, loaded once; a handle that fails to load simply draws nothing and the buttons keep
/// their caption.
#[derive(Resource, Clone)]
pub struct Art {
    pub races: Handle<Image>,
    pub classes: Handle<Image>,
    pub gender: Handle<Image>,
    pub factions: Handle<Image>,
    pub banners: Handle<Image>,
    pub rotate_up: Handle<Image>,
    pub rotate_down: Handle<Image>,
}

fn load(assets: &AssetServer, path: &str) -> Handle<Image> {
    assets.load_with_settings(
        benilla_assets::texture_url(&format!("{path}.blp"), (false, false)),
        |s: &mut benilla_assets::BlpLoaderSettings| s.variant = benilla_assets::BlpVariant::Sprite,
    )
}

impl Art {
    pub fn load(assets: &AssetServer) -> Art {
        debug_assert!(RACES.starts_with(DIR));
        Art {
            races: load(assets, RACES),
            classes: load(assets, CLASSES),
            gender: load(assets, GENDER),
            factions: load(assets, FACTIONS),
            banners: load(assets, BANNERS),
            rotate_up: load(assets, ROTATE_UP),
            rotate_down: load(assets, ROTATE_DOWN),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_race_icons_follow_the_lua_table() {
        assert_eq!(race_tc(1, 0), Some([0.0, 0.25, 0.0, 0.25]), "HUMAN_MALE");
        assert_eq!(race_tc(3, 0), Some([0.25, 0.5, 0.0, 0.25]), "DWARF_MALE");
        assert_eq!(race_tc(7, 0), Some([0.5, 0.75, 0.0, 0.25]), "GNOME_MALE");
        assert_eq!(race_tc(4, 0), Some([0.75, 1.0, 0.0, 0.25]), "NIGHTELF_MALE");
        assert_eq!(race_tc(6, 0), Some([0.0, 0.25, 0.25, 0.5]), "TAUREN_MALE");
        assert_eq!(race_tc(5, 0), Some([0.25, 0.5, 0.25, 0.5]), "SCOURGE_MALE");
        assert_eq!(race_tc(8, 0), Some([0.5, 0.75, 0.25, 0.5]), "TROLL_MALE");
        assert_eq!(race_tc(2, 0), Some([0.75, 1.0, 0.25, 0.5]), "ORC_MALE");
        assert_eq!(race_tc(1, 1), Some([0.0, 0.25, 0.5, 0.75]), "HUMAN_FEMALE");
        assert_eq!(
            race_tc(5, 1),
            Some([0.25, 0.5, 0.75, 1.0]),
            "SCOURGE_FEMALE"
        );
        assert_eq!(race_tc(2, 1), Some([0.75, 1.0, 0.75, 1.0]), "ORC_FEMALE");
        assert_eq!(race_tc(9, 0), None);
    }

    #[test]
    fn the_class_icons_follow_the_lua_table() {
        assert_eq!(class_tc(1), Some([0.0, 0.25, 0.0, 0.25]), "WARRIOR");
        assert_eq!(class_tc(8), Some([0.25, 0.49609375, 0.0, 0.25]), "MAGE");
        assert_eq!(
            class_tc(11),
            Some([0.7421875, 0.98828125, 0.0, 0.25]),
            "DRUID"
        );
        assert_eq!(
            class_tc(9),
            Some([0.7421875, 0.98828125, 0.25, 0.5]),
            "WARLOCK"
        );
        assert_eq!(class_tc(2), Some([0.0, 0.25, 0.5, 0.75]), "PALADIN");
        assert_eq!(class_tc(6), None, "no class 6 in 1.12");
    }

    #[test]
    fn every_race_and_class_has_an_icon_inside_its_sheet() {
        for race in 1..=8u8 {
            for sex in 0..=1u8 {
                let r = rect(RACES_SIZE, race_tc(race, sex).unwrap());
                assert!(r.max.x <= 256.0 && r.max.y <= 256.0 && r.min.x >= 0.0);
                assert_eq!((r.width(), r.height()), (64.0, 64.0));
            }
        }
        for class in super::super::rules::ALL_CLASSES {
            let r = rect(CLASSES_SIZE, class_tc(class).unwrap());
            assert!(r.max.x <= 256.0 && r.max.y <= 256.0);
            assert!((r.width() - 63.0).abs() <= 1.0 && r.height() == 64.0);
        }
    }

    /// The constants are the files' real sizes (the BLP2 header: width at byte 12, height at byte 16).
    /// Skips without the install.
    #[test]
    fn the_sheet_sizes_are_the_blp_headers() {
        let Some(data) = benilla_formats::wow_data() else {
            eprintln!("skipped: no WoW install");
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).expect("open chain");
        for (path, size) in [
            (RACES, RACES_SIZE),
            (CLASSES, CLASSES_SIZE),
            (GENDER, GENDER_SIZE),
            (FACTIONS, FACTIONS_SIZE),
        ] {
            let bytes = chain
                .read_file(&format!("{path}.blp"))
                .unwrap_or_else(|e| panic!("{path}: {e:#}"));
            assert_eq!(&bytes[..4], b"BLP2", "{path}");
            let w = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
            let h = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
            assert_eq!(Vec2::new(w as f32, h as f32), size, "{path}");
        }
        for path in [BANNERS, ROTATE_UP, ROTATE_DOWN] {
            let bytes = chain.read_file(&format!("{path}.blp")).unwrap();
            assert_eq!(&bytes[..3], b"BLP", "{path}");
        }
    }
}
