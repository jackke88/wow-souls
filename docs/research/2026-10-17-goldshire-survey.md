> Superseded 2026-10-18 at the user's request: remove extra houses and vegetation omissions; keep sparse road traffic only. Current behavior: docs/GOLDSHIRE.md. The house survey is historical research and now writes only its research output, never production road data.

# Goldshire village geometry survey — 2026-10-17

Read-only source survey; no archive, original placement, creature spawn, object spawn or patrol was modified. The generated additions and omissions are a reversible runtime overlay.

## Accepted output

- `data/world/goldshire-village.tsv`: 14 family houses, 42 separate interior rest points, a connected 174-node / 173-edge native-road graph, work/social/Stormwind-approach anchors, dense entrance paths and 15 omitted native vegetation IDs.
- The 15 omissions comprise **10 trees and 5 bushes**. There are 419 original tree placements within 420 yd of the survey centre (654 in the wider placement inventory); central trees within 65 yd are protected. The original five native bird02 flights east of Goldshire and nearby butterflies are retained.
- Main living floor is at least **0.3025 yd above the highest native terrain** under every full rotated footprint. Lowest foundation is at least **2.3243 yd below the lowest terrain**, so the original stone foundation has support and the room is above ground.
- Terrain is checked with a 21×21 grid and exact extrema of native ADT triangles clipped to the rotated footprint; ≥99.9% terrain coverage is required. Accepted plot relief is 0.749–1.453 yd.
- The entrance path combines native terrain with actual farmhouse collision surfaces at 0.1 yd intervals. Largest remaining source doorstep is 0.4687 yd. This is source-geometry validation; live motor traversal remains an integration check.
- All 1,950 nearby packaged creature/object/patrol/treasure points and 884 original patrol segments are protected by rotated footprint tests with 1.5–2.5 yd margins and vertical separation. The fixed treasure source is read directly, including Goldshire and Mirror Lake caches. No original gameplay point is relocated to make a plot fit.

## Original house and coordinates

`world\wmo\azeroth\buildings\human_farm\farm.wmo`, already used in the surrounding Elwynn forest. It is a small original cottage, not the Goldshire inn.

| Native local measurement | Value, WoW yards |
|---|---|
| Complete authored bounds | min `[-9.332298,-9.199208,-0.027779]`, max `[9.597073,8.874414,27.484703]` |
| Main walking floor | approximately `z=4.1093` |
| Loft | approximately `z=8.75` |
| Door portal bottom centre | `[-1.539898,-6.026075,4.109328]` |
| Door width / top | `2.857333` / `z=7.526981` |
| Rest slots 0 / 1 / 2 | `[2,2,4.1093]`, `[0,2,4.1093]`, `[-2,2,4.1093]` |

Original farms bury roughly 3.1–3.7 yd of the stone foundation. For example native placement 64340 has origin `z=76.284`, terrain at origin `79.396`, and doorway `80.394`. Added farms use `origin_z = maximum_terrain_z − 3.75`, reproducing this construction instead of exposing a four-yard front cliff.

Rest slots are two yards apart. Native collision floor and a radius-0.55, height-2.0 standing body box were checked every 0.2 yd along their connecting room path. Doodad set 0 has loft barrels and chimney smoke; its main room has no existing furniture. No bed model is fabricated by the survey.

## Collision and roads

Native doodad bounds use all eight corners transformed with the exact raw-WoW equivalent of `benilla_assets::coords::placement_rotation`; pitch, roll and scale are preserved. Static vegetation uses full rendered foliage triangles, a triangle/AABB separating-axis test, and the cottage's native render silhouette in vertical slabs, padded by 0.65 yd. Thus a canopy is checked, not just its trunk. Animated bird/butterfly flights retain their full authored bounding envelopes. Bind-pose foliage plus the stated margin is the geometric scope of the tree test; live wind animation is not claimed tested.

Roads follow native Elwynn cobblestone texture weights. The raster graph checks standing-body clearance and terrain. Fifteen short (≤10 yd) links bridge native alpha-feather gaps only after a new collision/terrain walk sampled every 0.35 yd. The runtime graph retains the union of required native paths and simplifies only chains within 0.9 yd of that source path, with fresh walk checks. Every house road point is an exact graph vertex.

The travel anchor is the unobstructed Stormwind approach at approximately `[-9080.209,426.042,92.475]`. The large original Stormwind WMO is included even though its placement origin is outside the survey circle; city-wall geometry limits the endpoint. This is not a route through the city interior.

## Reproduce and inspect

```powershell
$env:WOW_DATA = '<your vanilla WoW Data directory>'
cargo run -j 4 -p wow-world --example goldshire_survey --offline
```

The example only reads `WOW_DATA`; it writes the local research outputs and the village overlay TSV. Native source APIs are `MapTiles::load`, `existing_in_radius`, `load_tile_mesh`, `terrain_height_at`, `load_m2_bounds`, `load_m2_mesh`, `parse_wmo_root`, `load_wmo`, `accumulate_wmo_group_collision`, `WmoRoot::group_infos`, `WmoRoot::portals` and `triangle_z_at`.

- `.checks.tsv`: exact terrain/foundation/floor/entrance evidence for every accepted house.
- `.candidates.tsv`: accepted source model, native Euler transform, complete bounds and anchors.
- `.omit.tsv`: exact native placement IDs, models and positions cleared for plots/approaches.
- `.placements.tsv`, `.models.tsv`, `.roads.tsv`: local native source inventory.
- `.svg` / `.png`: overhead map; brown = native road paint, thin blue-grey = measured route, blue numbered boxes = new house bounds, blue lines = entrances, red crosses = omitted vegetation. North is up, west is left.

Three independent geometry tests pass: native MODF yaw convention, triangle-box rejection where loose AABBs overlap, and exact terrain peak/coverage detection. `clippy-driver -D warnings` and `rustfmt --check` pass for the example. Shared Cargo build lock was occupied by the integration build, so the scoped checks used the already built dependency libraries. The parent task owns full workspace gates and live checks.
