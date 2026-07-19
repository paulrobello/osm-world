> ⚠️ Historical implementation plan (2026-05) — retained for reference; current behavior may differ. See `docs/ARCHITECTURE.md` and the source code.

# OSM Point Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render OSM node-based trees, landmarks, and nature points as simple 3D mesh features.

**Architecture:** Preserve node tags in the OSM parser, classify renderable tagged nodes into a dedicated `WorldSource.point_features` collection, index them for tiled rendering, and render deterministic low-poly markers via a new `src/world/point_feature.rs` module. Keep point features separate from roads, railways, landuse, and buildings.

**Tech Stack:** Rust, existing OSM XML/PBF parser, existing CPU mesh generation, wgpu vertex feature types, cargo tests, graphify.

---

## File Structure

- Modify `src/osm/parse.rs` — add tags to `OsmNode`; parse XML/PBF node tags.
- Modify `src/stream/tile.rs` — add `point_features: Vec<usize>` to tile refs.
- Modify `src/render/vertex.rs` — add `feature::POINT_FEATURE`.
- Modify `shaders/city.wgsl` — add point feature material branch.
- Create `src/world/point_feature.rs` — classify point tags and generate tree/landmark/nature meshes.
- Modify `src/world/mod.rs` — export `point_feature`.
- Modify `src/world/loader.rs` — resolve, index, and render point features in full-world and tile paths.

## Task 1: Preserve OSM node tags

**Files:**
- Modify: `src/osm/parse.rs`

- [ ] **Step 1: Add node tags to `OsmNode`**

Change `OsmNode` from copy-only lat/lon to a cloneable struct with tags:

```rust
#[derive(Debug, Clone)]
pub struct OsmNode {
    pub lat: f64,
    pub lon: f64,
    pub tags: HashMap<String, String>,
}
```

Update every construction site to include `tags: HashMap::new()` for untagged nodes.

- [ ] **Step 2: Parse PBF node tags**

In `parse_pbf()`, update `Element::Node(n)` to collect tags:

```rust
let tags: HashMap<String, String> = n
    .tags()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
nodes.insert(n.id(), OsmNode { lat, lon, tags });
```

- [ ] **Step 3: Parse XML tagged nodes**

In `parse_osm_xml_str()`, support both self-closing untagged nodes and start/end node elements with child `<tag>` entries. Add parser state for `in_node`, `current_node_id`, `current_node_lat`, `current_node_lon`, and `current_node_tags`. On `Event::End` for `node`, insert an `OsmNode` with collected tags.

- [ ] **Step 4: Add XML parser test**

Add a test in `src/osm/parse.rs`:

```rust
#[test]
fn parse_xml_tagged_nodes() {
    let xml = r#"<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0">
    <tag k="natural" v="tree"/>
  </node>
</osm>"#;

    let data = parse_osm_xml_str(xml).unwrap();
    let node = data.nodes.get(&1).unwrap();

    assert_eq!(node.tags.get("natural").map(String::as_str), Some("tree"));
}
```

- [ ] **Step 5: Verify parser task**

Run:

```bash
cargo test osm::parse::tests::parse_xml_tagged_nodes -- --exact
cargo check --all-targets
```

Expected: both pass.

- [ ] **Step 6: Commit**

```bash
git add src/osm/parse.rs
git commit -m "feat: preserve osm node tags"
```

## Task 2: Add point feature model, indexing, and classification

**Files:**
- Create: `src/world/point_feature.rs`
- Modify: `src/world/mod.rs`
- Modify: `src/world/loader.rs`
- Modify: `src/stream/tile.rs`

- [ ] **Step 1: Add point feature module skeleton**

Create `src/world/point_feature.rs` with classification types:

```rust
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointFeatureKind {
    Tree,
    Landmark,
    Nature,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointFeatureStyle {
    pub kind: PointFeatureKind,
}

pub fn point_feature_style(tags: &HashMap<String, String>) -> Option<PointFeatureStyle> {
    if tags.get("natural").map(String::as_str) == Some("tree") {
        return Some(PointFeatureStyle { kind: PointFeatureKind::Tree });
    }
    if matches!(tags.get("natural").map(String::as_str), Some("peak" | "rock" | "spring")) {
        return Some(PointFeatureStyle { kind: PointFeatureKind::Nature });
    }
    if matches!(tags.get("tourism").map(String::as_str), Some("attraction" | "viewpoint" | "artwork"))
        || tags.contains_key("historic")
        || matches!(tags.get("man_made").map(String::as_str), Some("tower" | "water_tower" | "chimney"))
    {
        return Some(PointFeatureStyle { kind: PointFeatureKind::Landmark });
    }
    None
}
```

Export it from `src/world/mod.rs` with `pub mod point_feature;`.

- [ ] **Step 2: Add classification tests**

Add tests in `src/world/point_feature.rs` for `natural=tree`, `natural=peak`, `historic=monument`, and an ignored tag.

- [ ] **Step 3: Add tile refs**

In `src/stream/tile.rs`, add `pub point_features: Vec<usize>,` to `TileFeatureRefs` and assert it is empty in `feature_refs_default_to_empty_vectors()`.

- [ ] **Step 4: Add resolved point feature type**

In `src/world/loader.rs`, add:

```rust
#[derive(Clone, Debug)]
pub struct ResolvedPointFeature {
    pub tags: HashMap<String, String>,
    pub point: (f32, f32),
    pub elevation: f32,
    pub rep_lat: f64,
    pub rep_lon: f64,
}
```

Add `pub point_features: Vec<ResolvedPointFeature>,` to `WorldSource`.

- [ ] **Step 5: Index point features by owner tile**

In `WorldSource::feature_index_for_tile_size()`, add a loop over `self.point_features` using `TileCoord::from_world(point.0, point.1, tile_size)` and push indexes into `refs.point_features`.

- [ ] **Step 6: Classify tagged nodes in loader**

After way/relation classification in `load_world_source()`, iterate `osm_data.nodes.values()`. For each node where `point_feature_style(&node.tags).is_some()`, convert lat/lon with `conv.to_world_xz`, compute elevation, and push `ResolvedPointFeature`.

- [ ] **Step 7: Add loader/index tests**

Add tests in `src/world/loader.rs` that XML tagged nodes become `source.point_features`, and that point feature refs appear in the expected tile.

- [ ] **Step 8: Verify model task**

Run:

```bash
cargo test world::point_feature -- --nocapture
cargo test world::loader::tests::load_world_source_classifies_tagged_point_nodes -- --exact
cargo test stream::tile::tests::feature_refs_default_to_empty_vectors -- --exact
```

Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add src/world/point_feature.rs src/world/mod.rs src/world/loader.rs src/stream/tile.rs
git commit -m "feat: classify osm point features"
```

## Task 3: Render point feature meshes

**Files:**
- Modify: `src/world/point_feature.rs`
- Modify: `src/world/loader.rs`
- Modify: `src/render/vertex.rs`
- Modify: `shaders/city.wgsl`

- [ ] **Step 1: Add feature type and shader material**

In `src/render/vertex.rs`, add:

```rust
pub const POINT_FEATURE: f32 = 7.0;
```

In `shaders/city.wgsl`, add a material branch after railway for point features, for example `Material(0.12, 24.0)`.

- [ ] **Step 2: Implement point feature mesh generation**

In `src/world/point_feature.rs`, add:

```rust
pub fn generate_point_feature(
    tags: &HashMap<String, String>,
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(style) = point_feature_style(tags) else { return; };
    match style.kind {
        PointFeatureKind::Tree => append_tree(point, elevation, verts, idxs),
        PointFeatureKind::Landmark => append_landmark(point, elevation, verts, idxs),
        PointFeatureKind::Nature => append_nature_marker(point, elevation, verts, idxs),
    }
}
```

Use simple helper geometry: a small box trunk and pyramid/low-poly canopy for trees, a taller narrow box for landmarks, and a small pyramid/cone-like marker for nature points. All vertices use `feature::POINT_FEATURE`.

- [ ] **Step 3: Render full-world point features**

In `append_world_mesh()`, after roads and railways and before buildings, loop over `source.point_features` and call `generate_point_feature()`.

- [ ] **Step 4: Render tile point features**

In `append_tile_mesh()`, after railways and before buildings, loop over `refs.point_features`, fetch from `source.point_features`, and call `generate_point_feature()`.

- [ ] **Step 5: Add mesh tests**

Add point feature tests asserting tree geometry contains brown/green vertices, landmark geometry is taller than tree trunk, nature marker emits point feature vertices, and tile mesh emits point feature vertices.

- [ ] **Step 6: Verify rendering task**

Run:

```bash
cargo test world::point_feature -- --nocapture
cargo test world::loader::tests::tile_mesh_emits_point_feature_geometry -- --exact
cargo check --all-targets
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/world/point_feature.rs src/world/loader.rs src/render/vertex.rs shaders/city.wgsl
git commit -m "feat: render landmark and nature point features"
```

## Task 4: Final verification and docs memory

**Files:**
- No required source edits unless verification finds issues.

- [ ] **Step 1: Run full verification**

```bash
cargo fmt -- --check
make checkall
graphify update .
```

Expected: all commands pass.

- [ ] **Step 2: Check git status**

```bash
git status --short
```

Expected: clean or only intentional graphify hook changes already committed by hooks.

- [ ] **Step 3: Save vault pattern**

Use the research agent to save a Parsidion note under `Patterns/osm/` covering OSM point-feature parsing/rendering, source files, and final commit SHAs.

- [ ] **Step 4: Final response**

Report implemented files, verification commands, commits, and any deferred work.

## Self-Review

Spec coverage: parser node tags, point feature classification, full-world/tile indexing, tree/landmark/nature rendering, tests, and deferred work are covered. Placeholder scan: no TBD/TODO placeholders. Type consistency: `ResolvedPointFeature`, `point_features`, `PointFeatureKind`, `PointFeatureStyle`, and `generate_point_feature` names are consistent across tasks.
