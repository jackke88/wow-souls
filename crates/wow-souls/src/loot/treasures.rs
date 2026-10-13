//! Optional supply caches in native Northshire scenery, away from the chapel and main paths.
use wow_world::model::ItemCount;

use super::{ABBEY_CHEST, SALVE};

pub(super) struct Treasure {
    pub id: u32,
    pub name: &'static str,
    pub wow: [f32; 3],
    pub yaw: f32,
    pub items: &'static [ItemCount],
}

pub(super) const TREASURES: &[Treasure] = &[
    Treasure {
        // This ID previously stood in the Abbey doorway. Moving it preserves existing claims.
        id: ABBEY_CHEST,
        name: "Abbey supply cache",
        wow: [-8896.0, -235.0, 80.3],
        yaw: 0.0,
        items: &[(SALVE, 3)],
    },
    Treasure {
        // The back of the canvas shelter, on the woods side of its boxes and barrels.
        id: 9_000_002,
        name: "Woodland supply cache",
        wow: [-8768.0, -170.0, 83.0],
        yaw: std::f32::consts::FRAC_PI_2,
        items: &[(SALVE, 2)],
    },
    Treasure {
        // Beyond the supplies at the clearing's edge, reached by circling the camp.
        id: 9_000_003,
        name: "Camp reserve cache",
        wow: [-8786.0, -254.0, 83.0],
        yaw: -std::f32::consts::FRAC_PI_2,
        items: &[(SALVE, 2)],
    },
];
