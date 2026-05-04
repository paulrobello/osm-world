# Overpasses and Tunnels Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add visible OSM bridge/overpass and tunnel support with decks, rails, supports, portals, and tunnel lining while deferring full terrain carving.

**Architecture:** Keep OSM tag parsing unchanged; way tags already flow into `ResolvedFeature`. Add road-profile classification and structure mesh helpers in `src/world/road.rs`, then route both full-world and tile road rendering through one shared `append_road_feature_mesh()` helper in `src/world/loader.rs` so streaming and non-streaming output match.

**Tech Stack:** Rust 2024, existing `Vertex` mesh format, `cargo test`, `make checkall`, `graphify update .`.

---

## File Structure

- Modify `src/world/road.rs`
  - Add `RoadProfileKind` and `RoadProfile` for surface/bridge/tunnel classification.
  - Update `road_layer_y_offset()` to lower tunnel/negative-layer roads.
  - Add simple box-based structure helpers for bridge decks/rails/supports and tunnel portals/lining.
  - Keep all helpers panic-safe for short/degenerate point lists and mismatched elevation lengths.
- Modify `src/world/loader.rs`
  - Add one private `append_road_feature_mesh()` helper used by `append_world_mesh()` and `append_tile_roads_mesh()`.
  - Reuse existing road-cap behavior.
  - Add loader tests proving full-world and tile paths emit bridge/tunnel structure geometry.
- Do not modify OSM parsing, terrain generation, shaders, or web UI in this phase.

---

### Task 1: Road Profile Classification and Tunnel Offsets

**Files:**
- Modify: `src/world/road.rs`
- Test: inline `#[cfg(test)]` module in `src/world/road.rs`

- [ ] **Step 1: Write failing tests for road profile classification and tunnel lowering**

Add these tests near the existing `road_layer_y_offset_lifts_bridges_above_surface_roads` test:

```rust
#[test]
fn road_profile_classifies_bridge_tunnel_and_surface_roads() {
    let surface = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
    ]);
    let bridge = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("bridge".to_string(), "yes".to_string()),
    ]);
    let tunnel = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("tunnel".to_string(), "yes".to_string()),
    ]);

    assert_eq!(road_profile(&surface).kind, RoadProfileKind::Surface);
    assert_eq!(road_profile(&bridge).kind, RoadProfileKind::Bridge);
    assert_eq!(road_profile(&tunnel).kind, RoadProfileKind::Tunnel);
}

#[test]
fn road_layer_y_offset_lowers_tunnels_below_surface_roads() {
    let surface = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
    ]);
    let tunnel = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("tunnel".to_string(), "yes".to_string()),
        ("layer".to_string(), "-1".to_string()),
    ]);

    assert!(road_layer_y_offset(&surface) > 0.0);
    assert!(road_layer_y_offset(&tunnel) <= road_layer_y_offset(&surface) - 4.0);
}

#[test]
fn explicit_tunnel_wins_over_bridge_tags() {
    let tags = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("bridge".to_string(), "yes".to_string()),
        ("tunnel".to_string(), "yes".to_string()),
        ("layer".to_string(), "1".to_string()),
    ]);

    assert_eq!(road_profile(&tags).kind, RoadProfileKind::Tunnel);
    assert!(road_layer_y_offset(&tags) < 0.0);
}
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run:

```bash
cargo test world::road::tests::road_profile_classifies_bridge_tunnel_and_surface_roads world::road::tests::road_layer_y_offset_lowers_tunnels_below_surface_roads world::road::tests::explicit_tunnel_wins_over_bridge_tags
```

Expected: FAIL because `RoadProfileKind`, `road_profile()`, and tunnel lowering do not exist yet.

- [ ] **Step 3: Implement minimal road profile API**

In `src/world/road.rs`, replace the layer constants and `road_layer_y_offset()` implementation with this shape while preserving existing public constants:

```rust
const ROAD_BRIDGE_LAYER_Y_OFFSET: f32 = 5.0;
const ROAD_TUNNEL_LAYER_Y_OFFSET: f32 = -5.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoadProfileKind {
    Surface,
    Bridge,
    Tunnel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadProfile {
    pub kind: RoadProfileKind,
    pub layer_offset: f32,
}

pub fn road_profile(tags: &HashMap<String, String>) -> RoadProfile {
    let width = super::color::road_width(tags);
    let surface_offset = if width >= 5.0 {
        0.7
    } else if width >= 3.5 {
        0.6
    } else {
        0.5
    };

    let osm_layer = tags
        .get("layer")
        .and_then(|layer| layer.parse::<f32>().ok())
        .unwrap_or(0.0);
    let is_bridge = matches!(
        tags.get("bridge").map(String::as_str),
        Some("yes" | "viaduct")
    );
    let is_tunnel = tags
        .get("tunnel")
        .is_some_and(|value| value != "no");

    if is_tunnel {
        let layer_depth = if osm_layer < 0.0 { osm_layer.abs() } else { 1.0 };
        return RoadProfile {
            kind: RoadProfileKind::Tunnel,
            layer_offset: ROAD_TUNNEL_LAYER_Y_OFFSET * layer_depth,
        };
    }

    if is_bridge || osm_layer > 0.0 {
        return RoadProfile {
            kind: RoadProfileKind::Bridge,
            layer_offset: surface_offset + (osm_layer.max(1.0) * ROAD_BRIDGE_LAYER_Y_OFFSET),
        };
    }

    if osm_layer < 0.0 {
        return RoadProfile {
            kind: RoadProfileKind::Tunnel,
            layer_offset: ROAD_TUNNEL_LAYER_Y_OFFSET * osm_layer.abs(),
        };
    }

    RoadProfile {
        kind: RoadProfileKind::Surface,
        layer_offset: surface_offset,
    }
}

pub fn road_layer_y_offset(tags: &HashMap<String, String>) -> f32 {
    road_profile(tags).layer_offset
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run:

```bash
cargo test world::road::tests::road_profile_classifies_bridge_tunnel_and_surface_roads world::road::tests::road_layer_y_offset_lowers_tunnels_below_surface_roads world::road::tests::explicit_tunnel_wins_over_bridge_tags
```

Expected: PASS.

- [ ] **Step 5: Run existing road tests**

Run:

```bash
cargo test world::road
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/world/road.rs
git commit -m "feat: classify bridge and tunnel road profiles"
```

---

### Task 2: Bridge and Tunnel Structure Mesh Helpers

**Files:**
- Modify: `src/world/road.rs`
- Test: inline `#[cfg(test)]` module in `src/world/road.rs`

- [ ] **Step 1: Write failing bridge/tunnel structure tests**

Add these tests in `src/world/road.rs`:

```rust
#[test]
fn bridge_structure_adds_deck_rails_and_support_geometry() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let terrain_elevations = [0.0, 0.0];
    let road_elevations = [5.7, 5.7];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    append_bridge_structure(
        &points,
        &terrain_elevations,
        &road_elevations,
        6.0,
        &mut vertices,
        &mut indices,
    );

    assert!(!vertices.is_empty());
    assert!(!indices.is_empty());
    assert!(vertices.iter().any(|v| v.feature_type == feature::BUILDING));
    assert!(vertices.iter().any(|v| v.position[1] < road_elevations[0] + ROAD_Y_OFFSET));
    assert!(vertices.iter().any(|v| v.position[1] <= terrain_elevations[0] + 0.1));
}

#[test]
fn tunnel_structure_adds_portals_for_open_tunnel() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let road_elevations = [-5.0, -5.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    append_tunnel_structure(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

    assert!(!vertices.is_empty());
    assert!(!indices.is_empty());
    assert!(vertices.iter().any(|v| v.feature_type == feature::BUILDING));
    assert!(vertices.iter().any(|v| v.position[1] > road_elevations[0] + ROAD_Y_OFFSET));
}

#[test]
fn tunnel_structure_skips_portals_for_closed_loops() {
    let points = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 0.0)];
    let road_elevations = [-5.0, -5.0, -5.0, -5.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    append_tunnel_structure(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

    assert!(vertices.is_empty());
    assert!(indices.is_empty());
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test world::road::tests::bridge_structure_adds_deck_rails_and_support_geometry world::road::tests::tunnel_structure_adds_portals_for_open_tunnel world::road::tests::tunnel_structure_skips_portals_for_closed_loops
```

Expected: FAIL because `append_bridge_structure()` and `append_tunnel_structure()` do not exist.

- [ ] **Step 3: Add structure helper constants and public append function**

Add constants near the existing road constants:

```rust
const BRIDGE_DECK_THICKNESS: f32 = 0.6;
const BRIDGE_RAIL_HEIGHT: f32 = 0.9;
const BRIDGE_RAIL_WIDTH: f32 = 0.25;
const BRIDGE_SUPPORT_WIDTH: f32 = 0.8;
const TUNNEL_PORTAL_DEPTH: f32 = 1.0;
const TUNNEL_PORTAL_THICKNESS: f32 = 0.5;
const TUNNEL_CLEARANCE: f32 = 3.0;
const BRIDGE_STRUCTURE_COLOR: [f32; 3] = [0.50, 0.52, 0.54];
const TUNNEL_STRUCTURE_COLOR: [f32; 3] = [0.34, 0.32, 0.30];
```

Add this public dispatcher after `generate_road_with_elevations()`:

```rust
pub fn append_road_structures(
    tags: &HashMap<String, String>,
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    match road_profile(tags).kind {
        RoadProfileKind::Bridge => append_bridge_structure(
            points,
            terrain_elevations,
            road_elevations,
            width,
            verts,
            idxs,
        ),
        RoadProfileKind::Tunnel => append_tunnel_structure(points, road_elevations, width, verts, idxs),
        RoadProfileKind::Surface => {}
    }
}
```

- [ ] **Step 4: Add simple box geometry helpers**

Add these private helpers before `append_road_cap()`:

```rust
fn append_box(
    min: [f32; 3],
    max: [f32; 3],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let base = verts.len() as u32;
    let corners = [
        [min[0], min[1], min[2]],
        [max[0], min[1], min[2]],
        [max[0], max[1], min[2]],
        [min[0], max[1], min[2]],
        [min[0], min[1], max[2]],
        [max[0], min[1], max[2]],
        [max[0], max[1], max[2]],
        [min[0], max[1], max[2]],
    ];
    let normals = [
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];
    for (position, normal) in corners.into_iter().zip(normals) {
        verts.push(Vertex {
            position,
            normal,
            color,
            feature_type: feature::BUILDING,
        });
    }
    for tri in [
        [0, 2, 1], [0, 3, 2],
        [4, 5, 6], [4, 6, 7],
        [0, 1, 5], [0, 5, 4],
        [3, 7, 6], [3, 6, 2],
        [1, 2, 6], [1, 6, 5],
        [0, 4, 7], [0, 7, 3],
    ] {
        idxs.push(base + tri[0]);
        idxs.push(base + tri[1]);
        idxs.push(base + tri[2]);
    }
}

fn segment_frame(a: (f32, f32), b: (f32, f32)) -> Option<((f32, f32), (f32, f32), f32)> {
    let dx = b.0 - a.0;
    let dz = b.1 - a.1;
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1e-6 {
        None
    } else {
        Some(((dx / len, dz / len), (-dz / len, dx / len), len))
    }
}
```

- [ ] **Step 5: Add bridge and tunnel helper implementations**

Add these functions before `append_road_cap()`:

```rust
fn append_bridge_structure(
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() != terrain_elevations.len() || points.len() != road_elevations.len() || points.len() < 2 {
        return;
    }
    let half = width * 0.5;
    for i in 0..points.len() - 1 {
        let Some(((_dx, _dz), (px, pz), _len)) = segment_frame(points[i], points[i + 1]) else {
            continue;
        };
        let road_y = road_elevations[i].max(road_elevations[i + 1]) + ROAD_Y_OFFSET;
        let terrain_y = terrain_elevations[i].min(terrain_elevations[i + 1]);
        let x0 = points[i].0.min(points[i + 1].0) - px.abs() * half;
        let x1 = points[i].0.max(points[i + 1].0) + px.abs() * half;
        let z0 = points[i].1.min(points[i + 1].1) - pz.abs() * half;
        let z1 = points[i].1.max(points[i + 1].1) + pz.abs() * half;
        append_box(
            [x0, road_y - BRIDGE_DECK_THICKNESS, z0],
            [x1, road_y - 0.08, z1],
            BRIDGE_STRUCTURE_COLOR,
            verts,
            idxs,
        );
        append_box(
            [x0, road_y + 0.05, z0 - BRIDGE_RAIL_WIDTH],
            [x1, road_y + BRIDGE_RAIL_HEIGHT, z0],
            BRIDGE_STRUCTURE_COLOR,
            verts,
            idxs,
        );
        append_box(
            [x0, road_y + 0.05, z1],
            [x1, road_y + BRIDGE_RAIL_HEIGHT, z1 + BRIDGE_RAIL_WIDTH],
            BRIDGE_STRUCTURE_COLOR,
            verts,
            idxs,
        );
        if road_y - terrain_y > 2.0 {
            let cx = (points[i].0 + points[i + 1].0) * 0.5;
            let cz = (points[i].1 + points[i + 1].1) * 0.5;
            append_box(
                [cx - BRIDGE_SUPPORT_WIDTH * 0.5, terrain_y, cz - BRIDGE_SUPPORT_WIDTH * 0.5],
                [cx + BRIDGE_SUPPORT_WIDTH * 0.5, road_y - BRIDGE_DECK_THICKNESS, cz + BRIDGE_SUPPORT_WIDTH * 0.5],
                BRIDGE_STRUCTURE_COLOR,
                verts,
                idxs,
            );
        }
    }
}

fn append_tunnel_structure(
    points: &[(f32, f32)],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() != road_elevations.len() || points.len() < 2 {
        return;
    }
    let closed = points.len() >= 4 && same_point(points[0], points[points.len() - 1]);
    if closed {
        return;
    }
    append_tunnel_portal(points[0], points[1], road_elevations[0], width, verts, idxs);
    append_tunnel_portal(
        points[points.len() - 1],
        points[points.len() - 2],
        road_elevations[road_elevations.len() - 1],
        width,
        verts,
        idxs,
    );
}

fn append_tunnel_portal(
    point: (f32, f32),
    next: (f32, f32),
    elevation: f32,
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(((dx, dz), (px, pz), _len)) = segment_frame(point, next) else {
        return;
    };
    let road_y = elevation + ROAD_Y_OFFSET;
    let half = width * 0.5 + TUNNEL_PORTAL_THICKNESS;
    let depth_x = dx * TUNNEL_PORTAL_DEPTH;
    let depth_z = dz * TUNNEL_PORTAL_DEPTH;
    let left_x = point.0 + px * half;
    let left_z = point.1 + pz * half;
    let right_x = point.0 - px * half;
    let right_z = point.1 - pz * half;
    let top_y = road_y + TUNNEL_CLEARANCE;

    append_box(
        [left_x.min(left_x + depth_x) - TUNNEL_PORTAL_THICKNESS * 0.5, road_y, left_z.min(left_z + depth_z) - TUNNEL_PORTAL_THICKNESS * 0.5],
        [left_x.max(left_x + depth_x) + TUNNEL_PORTAL_THICKNESS * 0.5, top_y, left_z.max(left_z + depth_z) + TUNNEL_PORTAL_THICKNESS * 0.5],
        TUNNEL_STRUCTURE_COLOR,
        verts,
        idxs,
    );
    append_box(
        [right_x.min(right_x + depth_x) - TUNNEL_PORTAL_THICKNESS * 0.5, road_y, right_z.min(right_z + depth_z) - TUNNEL_PORTAL_THICKNESS * 0.5],
        [right_x.max(right_x + depth_x) + TUNNEL_PORTAL_THICKNESS * 0.5, top_y, right_z.max(right_z + depth_z) + TUNNEL_PORTAL_THICKNESS * 0.5],
        TUNNEL_STRUCTURE_COLOR,
        verts,
        idxs,
    );
    append_box(
        [left_x.min(right_x), top_y - TUNNEL_PORTAL_THICKNESS, left_z.min(right_z)],
        [left_x.max(right_x), top_y, left_z.max(right_z)],
        TUNNEL_STRUCTURE_COLOR,
        verts,
        idxs,
    );
}
```

If `cargo fmt` or tests expose narrow-axis boxes with zero thickness for north/south/east/west roads, adjust the affected min/max coordinate by `TUNNEL_PORTAL_THICKNESS` or `BRIDGE_RAIL_WIDTH` rather than changing public behavior.

- [ ] **Step 6: Run focused tests and then all road tests**

Run:

```bash
cargo test world::road::tests::bridge_structure_adds_deck_rails_and_support_geometry world::road::tests::tunnel_structure_adds_portals_for_open_tunnel world::road::tests::tunnel_structure_skips_portals_for_closed_loops
cargo test world::road
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/world/road.rs
git commit -m "feat: add bridge and tunnel structure meshes"
```

---

### Task 3: Integrate Structures into Full-World and Tile Road Rendering

**Files:**
- Modify: `src/world/loader.rs`
- Test: inline `#[cfg(test)]` module in `src/world/loader.rs`

- [ ] **Step 1: Write failing integration tests**

Add these tests near existing tile road tests in `src/world/loader.rs`:

```rust
#[test]
fn tile_road_mesh_emits_bridge_structure_geometry() {
    let mut source = empty_source();
    let mut bridge = feature(
        "highway",
        "primary",
        vec![(0.0, -50.0), (30.0, -50.0)],
    );
    bridge.tags.insert("bridge".to_string(), "yes".to_string());
    bridge.elevations = vec![0.0, 0.0];
    source.roads.push(bridge);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    append_tile_roads_mesh(
        &source,
        &[0],
        crate::stream::TileLod::Near,
        &mut vertices,
        &mut indices,
    );

    assert!(!indices.is_empty());
    assert!(vertices.iter().any(|v| v.feature_type == crate::render::vertex::feature::ROAD));
    assert!(vertices.iter().any(|v| v.feature_type == crate::render::vertex::feature::BUILDING));
}

#[test]
fn tile_road_mesh_emits_tunnel_portal_geometry() {
    let mut source = empty_source();
    let mut tunnel = feature(
        "highway",
        "primary",
        vec![(0.0, -50.0), (30.0, -50.0)],
    );
    tunnel.tags.insert("tunnel".to_string(), "yes".to_string());
    tunnel.elevations = vec![0.0, 0.0];
    source.roads.push(tunnel);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    append_tile_roads_mesh(
        &source,
        &[0],
        crate::stream::TileLod::Near,
        &mut vertices,
        &mut indices,
    );

    let road_min_y = vertices
        .iter()
        .filter(|v| v.feature_type == crate::render::vertex::feature::ROAD)
        .map(|v| v.position[1])
        .fold(f32::INFINITY, f32::min);

    assert!(road_min_y < super::super::road::ROAD_Y_OFFSET);
    assert!(vertices.iter().any(|v| v.feature_type == crate::render::vertex::feature::BUILDING));
}
```

- [ ] **Step 2: Run integration tests and verify they fail**

Run:

```bash
cargo test world::loader::tests::tile_road_mesh_emits_bridge_structure_geometry world::loader::tests::tile_road_mesh_emits_tunnel_portal_geometry
```

Expected: FAIL because the tile road path does not append structure meshes yet.

- [ ] **Step 3: Add shared road-feature append helper**

In `src/world/loader.rs`, add this private helper before `append_world_mesh()`:

```rust
fn append_road_feature_mesh(
    road: &ResolvedFeature,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) -> (Vec<f32>, f32, [f32; 3]) {
    let width = super::color::road_width(&road.tags);
    let color = super::color::road_color(&road.tags);
    let layer_offset = super::road::road_layer_y_offset(&road.tags);
    let road_elevations: Vec<f32> = road.elevations.iter().map(|e| e + layer_offset).collect();

    super::road::generate_road_with_elevations(
        &road.points,
        &road_elevations,
        width,
        color,
        verts,
        idxs,
    );
    super::road::append_road_structures(
        &road.tags,
        &road.points,
        &road.elevations,
        &road_elevations,
        width,
        verts,
        idxs,
    );

    (road_elevations, width, color)
}
```

- [ ] **Step 4: Use the helper in `append_world_mesh()`**

In the road loop inside `append_world_mesh()`, replace this block:

```rust
let width = super::color::road_width(&r.tags);
let color = super::color::road_color(&r.tags);
let layer_offset = super::road::road_layer_y_offset(&r.tags);
let road_elevations: Vec<f32> = r.elevations.iter().map(|e| e + layer_offset).collect();
super::road::generate_road_with_elevations(
    &r.points,
    &road_elevations,
    width,
    color,
    verts,
    idxs,
);
```

with:

```rust
let (road_elevations, width, color) = append_road_feature_mesh(r, verts, idxs);
```

Keep the cap loop immediately below unchanged except that it uses the returned `road_elevations`, `width`, and `color`.

- [ ] **Step 5: Use the helper in `append_tile_roads_mesh()`**

In the road loop inside `append_tile_roads_mesh()`, replace the same width/color/layer/generate block with:

```rust
let (road_elevations, width, color) = append_road_feature_mesh(r, verts, idxs);
```

Keep the cap loop immediately below unchanged except that it uses the returned `road_elevations`, `width`, and `color`.

- [ ] **Step 6: Run integration tests and loader tests**

Run:

```bash
cargo test world::loader::tests::tile_road_mesh_emits_bridge_structure_geometry world::loader::tests::tile_road_mesh_emits_tunnel_portal_geometry
cargo test world::loader
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/world/loader.rs
git commit -m "feat: render bridge and tunnel structures in world meshes"
```

---

### Task 4: Final Verification, Graph Update, and Browser Smoke Check

**Files:**
- Modify if needed: `src/world/road.rs`, `src/world/loader.rs`
- Generated update: `graphify-out/*` if `graphify update .` changes graph files

- [ ] **Step 1: Run format check**

Run:

```bash
cargo fmt -- --check
```

Expected: PASS. If it fails, run `cargo fmt`, inspect the diff, and include formatting in the final commit.

- [ ] **Step 2: Run canonical verification**

Run:

```bash
make checkall
```

Expected: PASS for fmt, typecheck, clippy, and tests.

- [ ] **Step 3: Update graphify graph after Rust code changes**

Run:

```bash
graphify update .
```

Expected: command exits successfully. If it changes `graphify-out/*`, review and commit those updates.

- [ ] **Step 4: Start the server for a smoke check**

Run in a background-safe shell:

```bash
make serve
```

Expected: server listens on `http://127.0.0.1:3030` and `/health` returns success.

- [ ] **Step 5: Open the server page in browser**

Use agentchrome:

```bash
agentchrome connect --launch
agentchrome navigate http://127.0.0.1:3030
agentchrome diagnose --current
agentchrome page screenshot --out /tmp/osm-world-overpasses-tunnels.png
```

Expected: page loads without browser console-blocking errors. Save screenshot path in the task result.

- [ ] **Step 6: Final commit if needed**

If formatting, graphify, or small verifier fixes changed files:

```bash
git add src/world/road.rs src/world/loader.rs graphify-out
git commit -m "chore: verify overpasses and tunnels"
```

- [ ] **Step 7: Report final verification evidence**

Include:

- `make checkall` result.
- `graphify update .` result.
- Browser smoke URL and screenshot path if available.
- Any deferred caveats from the design: no full terrain carving, no relation parsing.
