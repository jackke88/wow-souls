//! A reusable installed-chart view: one shared paper treatment, accurate coordinates, one saved pin.
mod art;
pub(crate) mod view;
pub(crate) use art::marker_image;
use bevy::prelude::*;
use std::path::PathBuf;

#[derive(Resource, Clone)]
pub(crate) struct Atlas {
    image: Option<Handle<Image>>,
    area: Option<benilla_formats::WorldMapArea>,
    name: String,
    error: String,
}

#[derive(Resource)]
pub(crate) struct Charts {
    areas: benilla_formats::AreaTableCatalog,
    westfall: Atlas,
    elwynn: Atlas,
    forced: bool,
}

fn follow_area(
    point: benilla_world::world_point::WorldPoint,
    charts: Option<Res<Charts>>,
    mut atlas: ResMut<Atlas>,
) {
    let Some(charts) = charts.filter(|c| !c.forced) else {
        return;
    };
    let Some(area) = point.area() else { return };
    let westfall =
        wow_world::valley::has_ancestor(area, 40, &|id| charts.areas.get(id).map(|a| a.zone_id));
    let chart = if westfall {
        &charts.westfall
    } else {
        &charts.elwynn
    };
    if atlas.area.as_ref().map(|a| a.area_id) != chart.area.as_ref().map(|a| a.area_id) {
        *atlas = chart.clone();
        info!("map: switched to {}", atlas.name);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Marker {
    area: u32,
    uv: Vec2,
}

impl Marker {
    fn parse(text: &str) -> Result<Option<Self>, String> {
        let mut lines = text.lines();
        if lines.next() != Some("wow-souls-map 1") {
            return Err("Unknown map save".into());
        }
        let Some(line) = lines.next() else {
            return Ok(None);
        };
        let w: Vec<_> = line.split_whitespace().collect();
        if w.len() != 4 || w[0] != "marker" || lines.next().is_some() {
            return Err("Invalid map marker".into());
        }
        let area = w[1].parse().map_err(|_| "Invalid marker area")?;
        let u = w[2].parse::<f32>().map_err(|_| "Invalid marker position")?;
        let v = w[3].parse::<f32>().map_err(|_| "Invalid marker position")?;
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            return Err("Marker outside map".into());
        }
        Ok(Some(Self {
            area,
            uv: Vec2::new(u, v),
        }))
    }
}

#[derive(Resource)]
pub(crate) struct Journal {
    marker: Option<Marker>,
    path: PathBuf,
    blocked: bool,
    note: String,
}
impl Journal {
    fn set(&mut self, marker: Option<Marker>) {
        self.marker = marker;
        if self.blocked {
            self.note = "Map save unavailable; marker is for this session.".into();
            return;
        }
        let text = marker.map_or_else(
            || "wow-souls-map 1\n".into(),
            |m| format!("wow-souls-map 1\nmarker {} {} {}\n", m.area, m.uv.x, m.uv.y),
        );
        let save = || -> std::io::Result<()> {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let tmp = self.path.with_extension("tmp");
            std::fs::write(&tmp, text)?;
            std::fs::rename(tmp, &self.path)
        };
        self.note = match save() {
            Ok(()) => if marker.is_some() {
                "Marker placed"
            } else {
                "Marker removed"
            }
            .into(),
            Err(e) => format!("Could not save marker: {e}"),
        };
    }
}

fn load(
    mut commands: Commands,
    assets: Option<Res<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
) {
    use benilla_assets::LockRecover;
    let area = std::env::var("WOWSOULS_MAP_AREA")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let result = assets
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("WoW map files unavailable"))
        .and_then(|assets| art::read(&mut assets.chain.lock_recover(), area));
    let atlas = match result {
        Ok(chart) => {
            info!(
                "map: {} loaded from installed tiles (shared parchment grade)",
                chart.name
            );
            Atlas {
                image: Some(images.add(art::image(chart.pixels))),
                area: Some(chart.area),
                name: chart.name,
                error: String::new(),
            }
        }
        Err(e) => {
            warn!("map: {e:#}");
            Atlas {
                image: None,
                area: None,
                name: "Map unavailable".into(),
                error: "The installed area map could not be read.".into(),
            }
        }
    };
    if let Some(assets) = assets.as_ref() {
        let mut chain = assets.chain.lock_recover();
        let other_area = if area == 40 { 12 } else { 40 };
        if let (Ok(areas), Ok(chart)) = (
            benilla_formats::load_area_table_catalog(&mut chain),
            art::read(&mut chain, other_area),
        ) {
            let other = Atlas {
                image: Some(images.add(art::image(chart.pixels))),
                area: Some(chart.area),
                name: chart.name,
                error: String::new(),
            };
            let (elwynn, westfall) = if area == 40 {
                (other, atlas.clone())
            } else {
                (atlas.clone(), other)
            };
            commands.insert_resource(Charts {
                areas,
                elwynn,
                westfall,
                forced: std::env::var_os("WOWSOULS_MAP_AREA").is_some(),
            });
        }
    }
    commands.insert_resource(atlas);
    let path = std::env::var_os("WOWSOULS_MAP_SAVE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if crate::run_mode::unattended() {
                PathBuf::from(format!("cache/map-check/marker-{}.txt", std::process::id()))
            } else {
                crate::saves::file("map-v1.txt")
            }
        });
    let (marker, blocked) = match std::fs::read_to_string(&path) {
        Ok(text) => match Marker::parse(&text) {
            Ok(m) => (m, false),
            Err(_) => (None, true),
        },
        Err(e) => (None, e.kind() != std::io::ErrorKind::NotFound),
    };
    commands.insert_resource(Journal {
        marker,
        path,
        blocked,
        note: String::new(),
    });
}

/// World X is north and Y is west; UI U is east and V is south. Preserve exact edge positions.
fn project(a: &benilla_formats::WorldMapArea, wow: Vec2) -> Vec2 {
    Vec2::new(
        (a.loc_left - wow.y) / (a.loc_left - a.loc_right),
        (a.loc_top - wow.x) / (a.loc_top - a.loc_bottom),
    )
}

fn unproject(a: &benilla_formats::WorldMapArea, uv: Vec2) -> Vec3 {
    let wow_x = a.loc_top - (a.loc_top - a.loc_bottom) * uv.y;
    let wow_y = a.loc_left - (a.loc_left - a.loc_right) * uv.x;
    benilla_assets::coords::wow_to_bevy([wow_x, wow_y, 0.0])
}

pub(crate) fn marker_world_point(
    journal: Option<&Journal>,
    atlas: Option<&Atlas>,
    charts: Option<&Charts>,
) -> Option<Vec3> {
    let m = journal?.marker?;
    let area = atlas
        .and_then(|a| a.area.as_ref())
        .filter(|a| a.area_id == m.area)
        .or_else(|| {
            let c = charts?;
            [c.elwynn.area.as_ref(), c.westfall.area.as_ref()]
                .into_iter()
                .flatten()
                .find(|a| a.area_id == m.area)
        })?;
    Some(unproject(area, m.uv))
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load.after(benilla_assets::AssetSet::Open));
    app.add_systems(
        Update,
        follow_area.run_if(in_state(crate::settle::Phase::Playing)),
    );
    view::plugin(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marker_save_rejects_corrupt_nonfinite_and_out_of_bounds_data() {
        for bad in [
            "foreign",
            "wow-souls-map 1\nmarker 12 NaN 0.5",
            "wow-souls-map 1\nmarker 12 1.1 0.5",
            "wow-souls-map 1\nmarker 12 0.5 0.5\nextra",
        ] {
            assert!(Marker::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(Marker::parse("wow-souls-map 1\n").unwrap(), None);
        assert_eq!(
            Marker::parse("wow-souls-map 1\nmarker 12 0.25 0.75\n").unwrap(),
            Some(Marker {
                area: 12,
                uv: Vec2::new(0.25, 0.75)
            })
        );
    }

    #[test]
    fn installed_projection_places_northshire_inside_elwynn_and_preserves_map_edges() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let maps = benilla_formats::load_world_map_area_catalog(&mut chain).unwrap();
        let a = maps.iter().find(|(_, a)| a.area_id == 12).unwrap().1;
        let uv = project(a, Vec2::new(-8921.0, -119.0));
        assert!((0.45..0.50).contains(&uv.x) && (0.40..0.45).contains(&uv.y));
        assert_eq!(project(a, Vec2::new(a.loc_top, a.loc_left)), Vec2::ZERO);
        assert_eq!(project(a, Vec2::new(a.loc_bottom, a.loc_right)), Vec2::ONE);
        for uv in [Vec2::new(0.1, 0.8), Vec2::new(0.95, 0.2)] {
            let bevy = unproject(a, uv);
            let wow = Vec2::new(-bevy.z, -bevy.x);
            assert!(project(a, wow).distance(uv) < 1e-5);
        }
    }

    #[test]
    fn saving_clearing_and_reloading_a_marker_preserves_unknown_save_files() {
        let path =
            std::env::temp_dir().join(format!("wow-souls-map-test-{}.txt", std::process::id()));
        let marker = Some(Marker {
            area: 12,
            uv: Vec2::new(0.55, 0.42),
        });
        let mut journal = Journal {
            marker: None,
            path: path.clone(),
            blocked: false,
            note: String::new(),
        };
        journal.set(marker);
        assert_eq!(
            Marker::parse(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            marker
        );
        journal.set(None);
        assert_eq!(
            Marker::parse(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            None
        );
        std::fs::write(&path, "future-format").unwrap();
        journal.blocked = true;
        journal.set(marker);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "future-format");
        std::fs::remove_file(path).unwrap();
    }
}
