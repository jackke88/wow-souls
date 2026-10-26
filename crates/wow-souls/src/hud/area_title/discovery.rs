//! Area identity, first-visit pacing and a versioned per-character journal.
use benilla_formats::AreaTableCatalog;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub const FILE: &str = "areas-v1.txt";
const HEADER: &str = "wow-souls-areas 1";
pub const DURATION: f32 = 5.2;
const SETTLE: f32 = 0.55;

/// Northshire's sibling subareas also hang directly under Elwynn in the client's table.
/// Keep them quiet; in particular, never discover Elwynn merely by loading the Abbey.
pub fn destination(catalog: &AreaTableCatalog, area: u32) -> Option<u32> {
    let mut id = area;
    for _ in 0..8 {
        if matches!(id, 9 | 24 | 34 | 59) {
            return None;
        }
        let row = catalog.get(id)?;
        if id == 87 || row.zone_id == 0 {
            return Some(id);
        }
        id = row.zone_id;
    }
    None
}

/// One shared, smooth opacity for the lettering, its shadow and the veil.
pub fn opacity(age: f32) -> f32 {
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    smooth(age / 0.9) * (1.0 - smooth((age - 3.7) / 1.5))
}

pub fn parse(text: &str) -> Result<BTreeSet<u32>, &'static str> {
    if text.len() > 65_536 {
        return Err("Area journal too large");
    }
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Err("Unknown area journal");
    }
    let mut seen = BTreeSet::new();
    for line in lines {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 2 || fields[0] != "area" {
            return Err("Invalid area entry");
        }
        let id = fields[1].parse::<u32>().map_err(|_| "Invalid area id")?;
        if id == 0 || !seen.insert(id) {
            return Err("Invalid or duplicate area id");
        }
    }
    Ok(seen)
}

pub struct Journal {
    pub seen: BTreeSet<u32>,
    path: PathBuf,
    blocked: bool,
}
impl Journal {
    pub fn load(path: PathBuf) -> Self {
        let (seen, blocked) = match std::fs::read_to_string(&path) {
            Ok(text) => match parse(&text) {
                Ok(seen) => (seen, false),
                Err(e) => {
                    bevy::log::warn!("area title: {e}; preserving {}", path.display());
                    (BTreeSet::new(), true)
                }
            },
            Err(e) => (BTreeSet::new(), e.kind() != std::io::ErrorKind::NotFound),
        };
        Self {
            seen,
            path,
            blocked,
        }
    }

    pub fn discover(&mut self, id: u32) {
        self.seen.insert(id);
        if self.blocked {
            return;
        }
        let mut text = format!("{HEADER}\n");
        for id in &self.seen {
            text.push_str(&format!("area {id}\n"));
        }
        let save = || -> std::io::Result<()> {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let tmp = self.path.with_extension("tmp");
            std::fs::write(&tmp, text)?;
            std::fs::rename(tmp, &self.path)
        };
        if let Err(e) = save() {
            bevy::log::warn!("area title: could not save discoveries ({e})");
        }
    }
}

#[derive(Default)]
pub struct Playback {
    candidate: Option<u32>,
    stable: f32,
    pub active: Option<(u32, f32)>,
}
impl Playback {
    /// Paused UI never consumes a discovery; pending destinations always follow current location.
    pub fn tick(
        &mut self,
        area: Option<u32>,
        dt: f32,
        available: bool,
        alive: bool,
        seen: &BTreeSet<u32>,
    ) -> Option<u32> {
        if !alive {
            *self = Self::default();
            return None;
        }
        if !available {
            return None;
        }
        let dt = dt.clamp(0.0, 0.1);
        if let Some((_, age)) = self.active.as_mut() {
            *age += dt;
            if *age >= DURATION {
                self.active = None;
            }
        }
        if self.candidate != area {
            self.candidate = area;
            self.stable = 0.0;
            return None;
        }
        self.stable += dt;
        let id = area?;
        if self.stable < SETTLE || self.active.is_some() || seen.contains(&id) {
            return None;
        }
        self.active = Some((id, 0.0));
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use benilla_formats::AreaTableRow;

    fn catalog() -> AreaTableCatalog {
        AreaTableCatalog::from_rows(
            [
                (12, 0),
                (9, 12),
                (24, 9),
                (34, 12),
                (59, 12),
                (87, 12),
                (10001, 87),
                (1519, 0),
                (1517, 1519),
                (18, 12),
            ]
            .into_iter()
            .map(|(id, parent)| {
                (
                    id,
                    AreaTableRow {
                        map_id: 0,
                        zone_id: parent,
                        explore_flag: 0,
                        flags: 0,
                        faction_group_mask: 0,
                        exploration_level: 1,
                        name: id.to_string(),
                    },
                )
            })
            .collect(),
        )
    }

    #[test]
    fn northshire_does_not_discover_elwynn_and_city_interiors_keep_identity() {
        let cat = catalog();
        for id in [9, 24, 34, 59, 0, 99999] {
            assert_eq!(destination(&cat, id), None);
        }
        assert_eq!(destination(&cat, 12), Some(12));
        assert_eq!(destination(&cat, 18), Some(12));
        assert_eq!(destination(&cat, 87), Some(87));
        assert_eq!(destination(&cat, 10001), Some(87));
        assert_eq!(destination(&cat, 1517), Some(1519));
    }

    #[test]
    fn installed_catalog_resolves_requested_names_and_stormwind_districts() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let cat = benilla_formats::load_area_table_catalog(&mut chain).unwrap();
        for (name, expected) in [
            ("Elwynn Forest", Some(12)),
            ("Goldshire", Some(87)),
            ("Stormwind City", Some(1519)),
            ("Northshire Valley", None),
            ("Northshire Abbey", None),
            ("Northshire Vineyards", None),
            ("Echo Ridge Mine", None),
        ] {
            let id = cat
                .id_for_name(name)
                .unwrap_or_else(|| panic!("missing installed area {name}"));
            assert_eq!(destination(&cat, id), expected, "{name} ({id})");
        }
        // Use the installed district names, which differ between client versions/locales.
        let mut districts = 0;
        for row in cat.rows().filter(|row| row.zone_id == 1519) {
            let id = cat.id_for_name(&row.name).unwrap();
            assert_eq!(destination(&cat, id), Some(1519), "{} ({id})", row.name);
            districts += 1;
        }
        assert!(districts > 0, "installed Stormwind has no districts");
    }

    fn step(p: &mut Playback, id: Option<u32>, seen: &BTreeSet<u32>) -> Option<u32> {
        (0..8).find_map(|_| p.tick(id, 0.1, true, true, seen))
    }

    #[test]
    fn boundary_jitter_and_paused_ui_do_not_consume_discovery() {
        let mut p = Playback::default();
        let seen = BTreeSet::new();
        for id in [Some(12), None, Some(12), None] {
            assert_eq!(p.tick(id, 0.1, true, true, &seen), None);
        }
        for _ in 0..100 {
            assert_eq!(p.tick(Some(87), 0.1, false, true, &seen), None);
        }
        assert_eq!(step(&mut p, Some(87), &seen), Some(87));
        let before = p.active;
        p.tick(Some(87), 0.1, false, true, &seen);
        assert_eq!(p.active, before);
        p.tick(Some(87), 0.1, true, false, &seen);
        assert!(p.active.is_none());
    }

    #[test]
    fn saved_discoveries_do_not_repeat_and_pending_areas_cannot_go_stale() {
        let mut p = Playback::default();
        let seen = BTreeSet::from([12]);
        assert_eq!(step(&mut p, Some(12), &seen), None);
        assert_eq!(step(&mut p, Some(87), &seen), Some(87));
        for _ in 0..10 {
            p.tick(Some(1519), 0.1, true, true, &seen);
        }
        for _ in 0..60 {
            assert_eq!(p.tick(Some(12), 0.1, true, true, &seen), None);
        }
        assert!(p.active.is_none());
        assert_eq!(step(&mut p, Some(1519), &seen), Some(1519));
    }

    #[test]
    fn opacity_has_clean_endpoints_and_a_readable_hold() {
        assert_eq!(opacity(0.0), 0.0);
        assert_eq!(opacity(DURATION), 0.0);
        assert_eq!(opacity(1.0), 1.0);
        assert_eq!(opacity(3.7), 1.0);
        assert!(opacity(0.45) > 0.49 && opacity(0.45) < 0.51);
        assert!(opacity(4.45) > 0.49 && opacity(4.45) < 0.51);
    }

    #[test]
    fn journal_roundtrip_isolated_characters_and_unknown_formats_are_protected() {
        let dir = std::env::temp_dir().join(format!("wsareas-{}", std::process::id()));
        let first = dir.join("1").join(FILE);
        let second = dir.join("2").join(FILE);
        let mut a = Journal::load(first.clone());
        a.discover(12);
        a.discover(87);
        assert_eq!(Journal::load(first).seen, BTreeSet::from([12, 87]));
        assert!(Journal::load(second).seen.is_empty());
        let future = dir.join("future.txt");
        let text = "wow-souls-areas 2\nunknown data\n";
        std::fs::write(&future, text).unwrap();
        let mut j = Journal::load(future.clone());
        j.discover(1519);
        assert_eq!(std::fs::read_to_string(future).unwrap(), text);
        assert!(j.seen.contains(&1519));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_journals_are_rejected_without_losing_unknown_valid_ids() {
        for text in [
            "",
            "wow-souls-areas 1\narea 0\n",
            "wow-souls-areas 1\narea 12\narea 12\n",
            "wow-souls-areas 1\narea no\n",
            "wow-souls-areas 1\nextra\n",
        ] {
            assert!(parse(text).is_err());
        }
        assert_eq!(
            parse("wow-souls-areas 1\narea 99999\n").unwrap(),
            BTreeSet::from([99999])
        );
    }
}
