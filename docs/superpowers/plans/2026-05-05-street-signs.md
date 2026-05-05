# Street Signs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add hybrid street-name signs: physical 3D signposts plus readable projected street-name labels.

**Architecture:** Add a focused `world::street_sign` module that owns road eligibility, anchor placement, and sign mesh generation. Integrate resolved signs into `WorldSource`, tile feature refs, full-world/tile mesh generation, and a UI label path parallel to existing POI labels.

**Tech Stack:** Rust, existing CPU mesh generation, `egui` projected overlay labels, cargo tests, `make checkall`, graphify.

---

## File Structure

- Create: `src/world/street_sign.rs` — street-sign data type, road eligibility, placement, mesh generation, and unit tests.
- Modify: `src/world/mod.rs` — expose the new module.
- Modify: `src/render/vertex.rs` — add a `feature::STREET_SIGN` feature constant.
- Modify: `src/stream/tile.rs` — add `street_signs: Vec<usize>` to `TileFeatureRefs`.
- Modify: `src/world/loader.rs` — add `WorldSource.street_signs`, derive signs after roads load, index signs by owner tile, append sign geometry in full-world and tile meshes, and add integration tests.
- Modify: `src/ui/poi_labels.rs` — reuse the existing projection code for street-sign labels while keeping POI labels independent.
- Modify: `src/ui/settings.rs` — add street-sign label settings controls.
- Modify: `src/app/init.rs`, `src/app/mod.rs`, `src/app/render_loop.rs` — carry street-sign labels/settings through app state and draw them.

## Task 1: Add the street-sign module

**Files:**
- Create: `src/world/street_sign.rs`
- Modify: `src/world/mod.rs`
- Modify: `src/render/vertex.rs`

- [ ] **Step 1: Add failing module/feature tests**

Create `src/world/street_sign.rs` with only tests and any imports needed for compilation once types are added:

```rust
use std::collections::HashMap;

use crate::render::vertex::{Vertex, feature};
use super::loader::ResolvedFeature;

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect()
    }

    fn road(name: &str, highway: &str, points: Vec<(f32, f32)>) -> ResolvedFeature {
        let mut tags = tags(&[("name", name), ("highway", highway)]);
        if name.is_empty() {
            tags.remove("name");
        }
        ResolvedFeature {
            tags,
            elevations: vec![0.0; points.len()],
            points,
            rep_lat: 38.0,
            rep_lon: -121.0,
        }
    }

    #[test]
    fn named_drivable_roads_are_eligible() {
        assert_eq!(street_name_for_road(&tags(&[("name", "Main Street"), ("highway", "residential")])).as_deref(), Some("Main Street"));
        assert_eq!(street_name_for_road(&tags(&[("name", " Broadway "), ("highway", "primary")])).as_deref(), Some("Broadway"));
    }

    #[test]
    fn unnamed_and_non_drivable_roads_are_skipped() {
        assert!(street_name_for_road(&tags(&[("highway", "residential")])).is_none());
        assert!(street_name_for_road(&tags(&[("name", "Oak Trail"), ("highway", "footway")])).is_none());
        assert!(street_name_for_road(&tags(&[("name", "Service Road"), ("highway", "service")])).is_none());
    }

    #[test]
    fn long_named_roads_produce_capped_periodic_signs() {
        let roads = vec![road("Main Street", "residential", vec![(0.0, 0.0), (600.0, 0.0), (1200.0, 0.0)])];
        let signs = street_signs_for_roads(&roads);
        let main_count = signs.iter().filter(|sign| sign.name == "Main Street").count();
        assert!(main_count > 1, "expected periodic signs, got {main_count}");
        assert!(main_count <= MAX_SIGNS_PER_ROAD, "expected per-road cap, got {main_count}");
    }

    #[test]
    fn shared_points_produce_intersection_signs() {
        let roads = vec![
            road("Main Street", "residential", vec![(0.0, 0.0), (100.0, 0.0)]),
            road("Broadway", "primary", vec![(100.0, -100.0), (100.0, 0.0), (100.0, 100.0)]),
        ];
        let signs = street_signs_for_roads(&roads);
        assert!(signs.iter().any(|sign| sign.name == "Main Street" && (sign.point.0 - 100.0).abs() < 0.01));
        assert!(signs.iter().any(|sign| sign.name == "Broadway" && (sign.point.0 - 100.0).abs() < 0.01));
    }

    #[test]
    fn street_sign_mesh_emits_street_sign_feature_vertices() {
        let sign = ResolvedStreetSign {
            name: "Main Street".to_string(),
            point: (10.0, -20.0),
            elevation: 2.0,
            tangent: (1.0, 0.0),
            rep_lat: 38.0,
            rep_lon: -121.0,
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_street_sign(&sign, &mut vertices, &mut indices);
        assert!(!indices.is_empty());
        assert!(vertices.iter().any(|v| v.feature_type == feature::STREET_SIGN));
    }
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test world::street_sign -- --nocapture
```

Expected: compile failure because `street_sign` is not in `world::mod`, and functions/types such as `street_name_for_road`, `street_signs_for_roads`, `ResolvedStreetSign`, and `append_street_sign` are not implemented.

- [ ] **Step 3: Expose the module and feature constant**

In `src/world/mod.rs`, add:

```rust
pub mod street_sign;
```

In `src/render/vertex.rs`, add the new constant after `POINT_FEATURE`:

```rust
pub const STREET_SIGN: f32 = 8.0;
```

- [ ] **Step 4: Implement street-sign types and eligibility**

Replace the top of `src/world/street_sign.rs` before the tests with:

```rust
use std::collections::{HashMap, HashSet};

use crate::render::vertex::{Vertex, feature};

use super::loader::ResolvedFeature;

pub const MAX_SIGNS_PER_ROAD: usize = 6;
const MAX_STREET_SIGNS: usize = 600;
const PERIODIC_SIGN_SPACING_METERS: f32 = 260.0;
const MIN_PERIODIC_ROAD_LENGTH_METERS: f32 = 360.0;
const INTERSECTION_KEY_SCALE: f32 = 10.0;
const SIGN_POST_COLOR: [f32; 3] = [0.62, 0.64, 0.60];
const SIGN_BOARD_COLOR: [f32; 3] = [0.05, 0.42, 0.22];
const SIGN_TRIM_COLOR: [f32; 3] = [0.92, 0.95, 0.88];

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedStreetSign {
    pub name: String,
    pub point: (f32, f32),
    pub elevation: f32,
    pub tangent: (f32, f32),
    pub rep_lat: f64,
    pub rep_lon: f64,
}

pub fn street_name_for_road(tags: &HashMap<String, String>) -> Option<String> {
    let name = tags.get("name").map(String::as_str)?.trim();
    if name.is_empty() || !is_drivable_highway(tags.get("highway").map(String::as_str)?) {
        return None;
    }
    Some(name.to_string())
}

fn is_drivable_highway(highway: &str) -> bool {
    !matches!(
        highway,
        "footway" | "path" | "cycleway" | "bridleway" | "steps" | "pedestrian" | "corridor" | "service" | "track"
    )
}
```

- [ ] **Step 5: Implement placement helpers**

Add this placement code before the tests in `src/world/street_sign.rs`:

```rust
pub fn street_signs_for_roads(roads: &[ResolvedFeature]) -> Vec<ResolvedStreetSign> {
    let eligible: Vec<(usize, String, &ResolvedFeature)> = roads
        .iter()
        .enumerate()
        .filter_map(|(index, road)| street_name_for_road(&road.tags).map(|name| (index, name, road)))
        .collect();

    let mut signs = Vec::new();
    let mut seen = HashSet::new();
    add_intersection_signs(&eligible, &mut signs, &mut seen);
    add_periodic_signs(&eligible, &mut signs, &mut seen);
    signs.truncate(MAX_STREET_SIGNS);
    signs
}

type PointKey = (i32, i32);

fn point_key(point: (f32, f32)) -> PointKey {
    ((point.0 * INTERSECTION_KEY_SCALE).round() as i32, (point.1 * INTERSECTION_KEY_SCALE).round() as i32)
}

fn add_intersection_signs(
    roads: &[(usize, String, &ResolvedFeature)],
    signs: &mut Vec<ResolvedStreetSign>,
    seen: &mut HashSet<(usize, PointKey)>,
) {
    let mut point_roads: HashMap<PointKey, Vec<(usize, usize)>> = HashMap::new();
    for (road_index, _name, road) in roads {
        for point_index in 0..road.points.len() {
            point_roads.entry(point_key(road.points[point_index])).or_default().push((*road_index, point_index));
        }
    }

    for (_key, refs) in point_roads.into_iter().filter(|(_, refs)| refs.len() > 1) {
        let road_count = refs.iter().map(|(road_index, _)| *road_index).collect::<HashSet<_>>().len();
        if road_count < 2 {
            continue;
        }
        for (road_index, point_index) in refs {
            let Some((_idx, name, road)) = roads.iter().find(|(idx, _, _)| *idx == road_index) else { continue; };
            push_sign_for_point(signs, seen, road_index, name, road, point_index);
        }
    }
}

fn add_periodic_signs(
    roads: &[(usize, String, &ResolvedFeature)],
    signs: &mut Vec<ResolvedStreetSign>,
    seen: &mut HashSet<(usize, PointKey)>,
) {
    for (road_index, name, road) in roads {
        if road.points.len() < 2 || road_length(road) < MIN_PERIODIC_ROAD_LENGTH_METERS {
            continue;
        }
        let mut next_distance = PERIODIC_SIGN_SPACING_METERS;
        let mut placed_for_road = signs.iter().filter(|sign| sign.name == *name).count();
        for segment_index in 0..road.points.len() - 1 {
            let p0 = road.points[segment_index];
            let p1 = road.points[segment_index + 1];
            let segment_len = distance(p0, p1);
            while next_distance <= segment_len && placed_for_road < MAX_SIGNS_PER_ROAD {
                let t = next_distance / segment_len;
                push_interpolated_sign(signs, seen, *road_index, name, road, segment_index, t);
                placed_for_road += 1;
                next_distance += PERIODIC_SIGN_SPACING_METERS;
            }
            next_distance -= segment_len;
        }
    }
}

fn road_length(road: &ResolvedFeature) -> f32 {
    road.points.windows(2).map(|pair| distance(pair[0], pair[1])).sum()
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dz = b.1 - a.1;
    (dx * dx + dz * dz).sqrt()
}
```

- [ ] **Step 6: Implement sign creation and mesh generation**

Add this code before the tests in `src/world/street_sign.rs`:

```rust
fn push_sign_for_point(
    signs: &mut Vec<ResolvedStreetSign>,
    seen: &mut HashSet<(usize, PointKey)>,
    road_index: usize,
    name: &str,
    road: &ResolvedFeature,
    point_index: usize,
) {
    if signs.iter().filter(|sign| sign.name == name).count() >= MAX_SIGNS_PER_ROAD {
        return;
    }
    let point = road.points[point_index];
    if !seen.insert((road_index, point_key(point))) {
        return;
    }
    let tangent = tangent_at_point(road, point_index);
    let elevation = road.elevations.get(point_index).copied().unwrap_or(0.0);
    signs.push(ResolvedStreetSign {
        name: name.to_string(),
        point,
        elevation,
        tangent,
        rep_lat: road.rep_lat,
        rep_lon: road.rep_lon,
    });
}

fn push_interpolated_sign(
    signs: &mut Vec<ResolvedStreetSign>,
    seen: &mut HashSet<(usize, PointKey)>,
    road_index: usize,
    name: &str,
    road: &ResolvedFeature,
    segment_index: usize,
    t: f32,
) {
    let p0 = road.points[segment_index];
    let p1 = road.points[segment_index + 1];
    let point = (p0.0 + (p1.0 - p0.0) * t, p0.1 + (p1.1 - p0.1) * t);
    if !seen.insert((road_index, point_key(point))) {
        return;
    }
    let e0 = road.elevations.get(segment_index).copied().unwrap_or(0.0);
    let e1 = road.elevations.get(segment_index + 1).copied().unwrap_or(e0);
    signs.push(ResolvedStreetSign {
        name: name.to_string(),
        point,
        elevation: e0 + (e1 - e0) * t,
        tangent: normalize_2d((p1.0 - p0.0, p1.1 - p0.1)),
        rep_lat: road.rep_lat,
        rep_lon: road.rep_lon,
    });
}

fn tangent_at_point(road: &ResolvedFeature, point_index: usize) -> (f32, f32) {
    if point_index + 1 < road.points.len() {
        let p = road.points[point_index];
        let next = road.points[point_index + 1];
        return normalize_2d((next.0 - p.0, next.1 - p.1));
    }
    if point_index > 0 {
        let prev = road.points[point_index - 1];
        let p = road.points[point_index];
        return normalize_2d((p.0 - prev.0, p.1 - prev.1));
    }
    (1.0, 0.0)
}

fn normalize_2d(v: (f32, f32)) -> (f32, f32) {
    let len = (v.0 * v.0 + v.1 * v.1).sqrt();
    if len <= 1e-6 { (1.0, 0.0) } else { (v.0 / len, v.1 / len) }
}

pub fn append_street_sign(sign: &ResolvedStreetSign, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    append_oriented_box(sign.point, sign.elevation, (0.08, 0.08), 2.4, (1.0, 0.0), SIGN_POST_COLOR, verts, idxs);
    append_oriented_box(sign.point, sign.elevation + 2.25, (1.45, 0.08), 0.62, sign.tangent, SIGN_TRIM_COLOR, verts, idxs);
    append_oriented_box(sign.point, sign.elevation + 2.32, (1.32, 0.09), 0.48, sign.tangent, SIGN_BOARD_COLOR, verts, idxs);
}

fn append_oriented_box(
    point: (f32, f32),
    base_y: f32,
    half_extents: (f32, f32),
    height: f32,
    tangent: (f32, f32),
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let t = normalize_2d(tangent);
    let n = (-t.1, t.0);
    let center = glam::vec3(point.0, base_y, point.1);
    let hx = half_extents.0;
    let hz = half_extents.1;
    let corners = [
        center + glam::vec3(-t.0 * hx - n.0 * hz, 0.0, -t.1 * hx - n.1 * hz),
        center + glam::vec3( t.0 * hx - n.0 * hz, 0.0,  t.1 * hx - n.1 * hz),
        center + glam::vec3( t.0 * hx + n.0 * hz, 0.0,  t.1 * hx + n.1 * hz),
        center + glam::vec3(-t.0 * hx + n.0 * hz, 0.0, -t.1 * hx + n.1 * hz),
    ];
    let top = corners.map(|p| p + glam::vec3(0.0, height, 0.0));
    append_quad([corners[0], corners[1], top[1], top[0]], color, verts, idxs);
    append_quad([corners[1], corners[2], top[2], top[1]], color, verts, idxs);
    append_quad([corners[2], corners[3], top[3], top[2]], color, verts, idxs);
    append_quad([corners[3], corners[0], top[0], top[3]], color, verts, idxs);
    append_quad([top[0], top[1], top[2], top[3]], color, verts, idxs);
    append_quad([corners[3], corners[2], corners[1], corners[0]], color, verts, idxs);
}

fn append_quad(positions: [glam::Vec3; 4], color: [f32; 3], verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    let normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]).normalize_or_zero().to_array();
    let base = verts.len() as u32;
    for position in positions {
        verts.push(Vertex { position: position.to_array(), normal, color, feature_type: feature::STREET_SIGN, uv: [0.0, 0.0] });
    }
    idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}
```

- [ ] **Step 7: Run street-sign module tests**

Run:

```bash
cargo test world::street_sign -- --nocapture
```

Expected: all `world::street_sign` tests pass.

- [ ] **Step 8: Commit Task 1**

Run:

```bash
git add src/world/street_sign.rs src/world/mod.rs src/render/vertex.rs
git commit -m "feat: add street sign generation"
```

## Task 2: Integrate street signs into world loading and tile meshes

**Files:**
- Modify: `src/stream/tile.rs`
- Modify: `src/world/loader.rs`

- [ ] **Step 1: Add failing loader/tile tests**

In `src/world/loader.rs` tests, add these tests near existing point feature and tile mesh tests:

```rust
#[test]
fn load_world_source_generates_street_signs_for_named_drivable_roads() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("street_signs.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <bounds minlat="38.0" minlon="-121.0" maxlat="38.01" maxlon="-120.99"/>
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.995"/>
  <node id="3" lat="38.0" lon="-120.99"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <tag k="highway" v="residential"/>
    <tag k="name" v="Main Street"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert!(!source.street_signs.is_empty());
    assert!(source.street_signs.iter().any(|sign| sign.name == "Main Street"));
}

#[test]
fn street_sign_index_maps_signs_to_owner_tiles() {
    let mut source = empty_source();
    source.street_signs.push(crate::world::street_sign::ResolvedStreetSign {
        name: "Main Street".to_string(),
        point: (125.0, -75.0),
        elevation: 3.0,
        tangent: (1.0, 0.0),
        rep_lat: 1.0,
        rep_lon: 2.0,
    });

    let index = source.feature_index_for_tile_size(100.0);

    assert_eq!(
        index.get(&crate::stream::TileCoord { x: 1, z: -1 }).unwrap().street_signs,
        vec![0]
    );
}

#[test]
fn world_mesh_emits_street_sign_geometry() {
    let mut source = empty_source();
    source.street_signs.push(crate::world::street_sign::ResolvedStreetSign {
        name: "Main Street".to_string(),
        point: (10.0, -20.0),
        elevation: 2.0,
        tangent: (1.0, 0.0),
        rep_lat: 1.0,
        rep_lon: 2.0,
    });

    let mesh = generate_world_mesh(&source);

    assert!(mesh.vertices.iter().any(|v| v.feature_type == crate::render::vertex::feature::STREET_SIGN));
}

#[test]
fn tile_mesh_emits_street_sign_geometry() {
    let mut source = empty_source();
    source.street_signs.push(crate::world::street_sign::ResolvedStreetSign {
        name: "Main Street".to_string(),
        point: (10.0, -20.0),
        elevation: 2.0,
        tangent: (1.0, 0.0),
        rep_lat: 1.0,
        rep_lon: 2.0,
    });
    let refs = crate::stream::tile::TileFeatureRefs {
        street_signs: vec![0],
        ..Default::default()
    };

    let mesh = generate_tile_mesh_set(&source, crate::stream::TileCoord { x: 0, z: -1 }, &refs, 100.0);

    let vertices = &mesh.lods[crate::stream::TileLod::Near as usize].vertices;
    assert!(vertices.iter().any(|v| v.feature_type == crate::render::vertex::feature::STREET_SIGN));
}
```

- [ ] **Step 2: Run failing loader tests**

Run:

```bash
cargo test world::loader::tests::load_world_source_generates_street_signs_for_named_drivable_roads -- --exact
cargo test world::loader::tests::street_sign_index_maps_signs_to_owner_tiles -- --exact
cargo test world::loader::tests::world_mesh_emits_street_sign_geometry -- --exact
cargo test world::loader::tests::tile_mesh_emits_street_sign_geometry -- --exact
```

Expected: compile failure because `WorldSource.street_signs` and `TileFeatureRefs.street_signs` do not exist.

- [ ] **Step 3: Add `street_signs` to tile refs**

In `src/stream/tile.rs`, update `TileFeatureRefs`:

```rust
#[derive(Clone, Debug, Default)]
pub struct TileFeatureRefs {
    pub buildings: Vec<usize>,
    pub roads: Vec<usize>,
    pub railways: Vec<usize>,
    pub waters: Vec<usize>,
    pub landuses: Vec<usize>,
    pub point_features: Vec<usize>,
    pub street_signs: Vec<usize>,
}
```

Update any tests that assert all refs are empty to include `refs.street_signs.is_empty()`.

- [ ] **Step 4: Add `street_signs` to `WorldSource` and constructors**

In `src/world/loader.rs`, import the type near other imports:

```rust
use crate::world::street_sign::ResolvedStreetSign;
```

Add the field to `WorldSource`:

```rust
pub street_signs: Vec<ResolvedStreetSign>,
```

Add `street_signs: Vec::new(),` to every explicit `WorldSource { ... }` test constructor and helper, including `empty_source()`.

- [ ] **Step 5: Derive street signs in `load_world_source`**

Before the final `Ok(WorldSource { ... })` in `load_world_source`, add:

```rust
let street_signs = super::street_sign::street_signs_for_roads(&roads);
```

Add the field to the returned `WorldSource`:

```rust
street_signs,
```

- [ ] **Step 6: Index street signs by tile**

In `WorldSource::feature_index_for_tile_size`, after indexing `point_features`, add:

```rust
for (feature_idx, sign) in self.street_signs.iter().enumerate() {
    let coord = crate::stream::TileCoord::from_world(sign.point.0, sign.point.1, tile_size);
    index
        .entry(coord)
        .or_insert_with(crate::stream::tile::TileFeatureRefs::default)
        .street_signs
        .push(feature_idx);
}
```

- [ ] **Step 7: Append street-sign geometry**

In `append_world_mesh`, after point features and before buildings, add:

```rust
for sign in &source.street_signs {
    super::street_sign::append_street_sign(sign, verts, idxs);
}
```

In `append_tile_features_mesh`, after point features and before buildings, add:

```rust
for &feature_idx in &refs.street_signs {
    let Some(sign) = source.street_signs.get(feature_idx) else {
        continue;
    };
    super::street_sign::append_street_sign(sign, verts, idxs);
}
```

Update the `log::info!` in `append_world_mesh` to include `{} street signs` and `source.street_signs.len()`.

- [ ] **Step 8: Run loader/tile tests**

Run:

```bash
cargo test world::loader::tests::load_world_source_generates_street_signs_for_named_drivable_roads -- --exact
cargo test world::loader::tests::street_sign_index_maps_signs_to_owner_tiles -- --exact
cargo test world::loader::tests::world_mesh_emits_street_sign_geometry -- --exact
cargo test world::loader::tests::tile_mesh_emits_street_sign_geometry -- --exact
cargo test stream::tile -- --nocapture
```

Expected: all pass.

- [ ] **Step 9: Commit Task 2**

Run:

```bash
git add src/stream/tile.rs src/world/loader.rs
git commit -m "feat: integrate street signs into world meshes"
```

## Task 3: Add projected street-name labels and settings

**Files:**
- Modify: `src/ui/poi_labels.rs`
- Modify: `src/ui/settings.rs`
- Modify: `src/app/init.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/app/render_loop.rs`

- [ ] **Step 1: Add failing label tests**

In `src/ui/poi_labels.rs` tests, add:

```rust
#[test]
fn labels_include_street_sign_names_independent_from_pois() {
    let signs = vec![crate::world::street_sign::ResolvedStreetSign {
        name: "Main Street".to_string(),
        point: (1.0, 2.0),
        elevation: 3.0,
        tangent: (1.0, 0.0),
        rep_lat: 0.0,
        rep_lon: 0.0,
    }];

    let labels = labels_from_street_signs(&signs);

    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].text, "Main Street");
    assert_eq!(labels[0].position, glam::vec3(1.0, 6.2, 2.0));
}

#[test]
fn street_sign_label_settings_are_independent_from_poi_settings() {
    let poi = PoiLabelSettings::default();
    let street = StreetSignLabelSettings::default();

    assert!(poi.visible);
    assert!(street.visible);
    assert_ne!(poi.max_distance, street.max_distance);
}
```

- [ ] **Step 2: Run failing label tests**

Run:

```bash
cargo test ui::poi_labels::tests::labels_include_street_sign_names_independent_from_pois -- --exact
cargo test ui::poi_labels::tests::street_sign_label_settings_are_independent_from_poi_settings -- --exact
```

Expected: compile failure because `labels_from_street_signs` and `StreetSignLabelSettings` do not exist.

- [ ] **Step 3: Add street label settings and label generation**

In `src/ui/poi_labels.rs`, add this settings type after `PoiLabelSettings`:

```rust
#[derive(Clone, Debug)]
pub struct StreetSignLabelSettings {
    pub visible: bool,
    pub max_distance: f32,
}

impl Default for StreetSignLabelSettings {
    fn default() -> Self {
        Self {
            visible: true,
            max_distance: 500.0,
        }
    }
}
```

Add this function after `labels_from_point_features`:

```rust
pub fn labels_from_street_signs(
    street_signs: &[crate::world::street_sign::ResolvedStreetSign],
) -> Vec<PoiLabel> {
    street_signs
        .iter()
        .filter(|sign| !sign.name.trim().is_empty())
        .map(|sign| PoiLabel {
            text: sign.name.trim().to_string(),
            position: glam::vec3(sign.point.0, sign.elevation + 3.2, sign.point.1),
        })
        .collect()
}
```

- [ ] **Step 4: Reuse projection drawing for street signs**

In `src/ui/poi_labels.rs`, keep existing `draw` behavior for POIs and add a street-sign draw wrapper:

```rust
pub fn draw_street_signs(
    ctx: &egui::Context,
    camera: &Flycam,
    labels: &[PoiLabel],
    settings: &StreetSignLabelSettings,
    viewport_size: egui::Vec2,
) {
    draw_projected_labels(
        ctx,
        camera,
        labels,
        settings.visible,
        settings.max_distance,
        viewport_size,
        "street_sign_label",
        egui::Color32::from_rgba_unmultiplied(8, 74, 38, 220),
    );
}
```

Refactor the existing `draw` body so both wrappers call this helper:

```rust
fn draw_projected_labels(
    ctx: &egui::Context,
    camera: &Flycam,
    labels: &[PoiLabel],
    visible_enabled: bool,
    max_distance: f32,
    viewport_size: egui::Vec2,
    id_prefix: &'static str,
    fill: egui::Color32,
) {
    if !visible_enabled || labels.is_empty() {
        return;
    }

    let mut visible: Vec<_> = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| {
            let distance = label.position.distance(camera.position);
            if distance > max_distance {
                return None;
            }
            let screen_pos = project_world_to_screen(camera, label.position, viewport_size)?;
            Some((distance, index, label, screen_pos))
        })
        .collect();
    visible.sort_by(|a, b| a.0.total_cmp(&b.0));

    for (_distance, index, label, screen_pos) in visible.into_iter().take(MAX_VISIBLE_LABELS) {
        egui::Area::new(egui::Id::new((id_prefix, index)))
            .order(egui::Order::Foreground)
            .interactable(false)
            .fixed_pos(screen_pos + egui::vec2(8.0, -30.0))
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(fill)
                    .corner_radius(3.0)
                    .inner_margin(egui::Margin::symmetric(5, 2))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&label.text).color(egui::Color32::WHITE).small());
                    });
            });
    }
}
```

Update existing `draw` to call `draw_projected_labels` with POI settings and black fill:

```rust
pub fn draw(
    ctx: &egui::Context,
    camera: &Flycam,
    labels: &[PoiLabel],
    settings: &PoiLabelSettings,
    viewport_size: egui::Vec2,
) {
    draw_projected_labels(
        ctx,
        camera,
        labels,
        settings.visible,
        settings.max_distance,
        viewport_size,
        "poi_label",
        egui::Color32::from_black_alpha(185),
    );
}
```

- [ ] **Step 5: Wire labels through app state**

In `src/app/init.rs`, add to `AppState`:

```rust
pub street_sign_labels: Vec<crate::ui::poi_labels::PoiLabel>,
```

Change the tuple in `init_wgpu` from `(scene, coord_converter, poi_labels)` to `(scene, coord_converter, poi_labels, street_sign_labels)`. In the `Some(path)` branch, derive labels before generating the mesh:

```rust
let poi_labels = crate::ui::poi_labels::labels_from_point_features(&source.point_features);
let street_sign_labels = crate::ui::poi_labels::labels_from_street_signs(&source.street_signs);
```

Return `street_sign_labels` in both branches, using `Vec::new()` in the `None` branch, and set `street_sign_labels,` in `AppState` construction.

In `src/app/mod.rs`, add to `App`:

```rust
pub street_sign_labels: crate::ui::poi_labels::StreetSignLabelSettings,
```

Initialize it in `App::new`:

```rust
street_sign_labels: crate::ui::poi_labels::StreetSignLabelSettings::default(),
```

In `src/app/render_loop.rs`, add to `RenderUiState`:

```rust
pub street_sign_labels: &'a mut crate::ui::poi_labels::StreetSignLabelSettings,
```

At the call site in `src/app/event_handler.rs`, pass:

```rust
street_sign_labels: &mut self.street_sign_labels,
```

- [ ] **Step 6: Draw street labels and add settings UI**

In `src/app/render_loop.rs`, after the existing POI label draw call, add:

```rust
crate::ui::poi_labels::draw_street_signs(
    ctx,
    &state.camera,
    &state.street_sign_labels,
    ui_state.street_sign_labels,
    viewport_size,
);
```

Update `crate::ui::settings::draw` signature and call sites to accept `street_sign_labels: &mut crate::ui::poi_labels::StreetSignLabelSettings`.

In `src/ui/settings.rs`, add this function:

```rust
fn street_sign_labels_section(
    ui: &mut egui::Ui,
    street_sign_labels: &mut crate::ui::poi_labels::StreetSignLabelSettings,
) {
    CollapsingHeader::new(RichText::new("Street Sign Labels").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut street_sign_labels.visible, "Visible");
            ui.add(
                Slider::new(&mut street_sign_labels.max_distance, 50.0..=2000.0)
                    .step_by(25.0)
                    .text("Max distance (m)"),
            );
        });
}
```

Call `street_sign_labels_section(ui, street_sign_labels);` immediately after `poi_labels_section(ui, poi_labels);`.

- [ ] **Step 7: Run UI/app tests and check**

Run:

```bash
cargo test ui::poi_labels -- --nocapture
cargo check --all-targets
```

Expected: all pass.

- [ ] **Step 8: Commit Task 3**

Run:

```bash
git add src/ui/poi_labels.rs src/ui/settings.rs src/app/init.rs src/app/mod.rs src/app/render_loop.rs src/app/event_handler.rs
git commit -m "feat: add street sign labels"
```

## Task 4: Final verification and graph update

**Files:**
- No source edits unless verification fails.

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt -- --check
```

Expected: pass. If it fails, run `cargo fmt`, inspect the diff, and commit formatting with the affected task commit or a separate `style: format street signs` commit.

- [ ] **Step 2: Run full verifier**

Run:

```bash
make checkall
```

Expected: `cargo fmt -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` all pass.

- [ ] **Step 3: Update graphify**

Run:

```bash
graphify update .
```

Expected: graph files update successfully.

- [ ] **Step 4: Commit verification/graph updates if needed**

Run:

```bash
git status --short
```

If `graphify-out/` or formatting files changed, run:

```bash
git add graphify-out src
if ! git diff --cached --quiet; then git commit -m "chore: update graph after street signs"; fi
```

- [ ] **Step 5: Save/update vault note**

Update the existing OSM pattern note about point feature/overlay labels or create a new vault note under `~/ParsidionVault/Patterns/osm/` documenting the hybrid physical-geometry plus projected-label pattern. Rebuild the vault index with:

```bash
uv run --no-project ~/.claude/skills/parsidion/scripts/update_index.py
```

## Self-Review

Spec coverage: Task 1 covers named drivable road eligibility, periodic/intersection placement, caps, tangent storage, and mesh generation. Task 2 covers `WorldSource`, tile indexing, full-world/tile mesh emission, and loader integration. Task 3 covers projected labels, distance caps, and independent settings from POI labels. Task 4 covers `make checkall`, graphify update, and vault capture.

Placeholder scan: no placeholder markers remain; all implementation steps include concrete file paths, code snippets, commands, and expected results.

Type consistency: the plan consistently uses `ResolvedStreetSign`, `WorldSource.street_signs`, `TileFeatureRefs.street_signs`, `feature::STREET_SIGN`, `labels_from_street_signs`, and `StreetSignLabelSettings`.
