# POI Point Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the existing OSM point-feature overlay to render common POIs from tagged OSM nodes.

**Architecture:** Reuse `src/world/point_feature.rs`, `WorldSource.point_features`, tile refs, and `feature::POINT_FEATURE`. Add a POI style category to central classification and render POIs as simple deterministic markers with category colors.

**Tech Stack:** Rust, existing OSM node tag parser, existing CPU mesh generation, cargo tests, graphify.

---

## File Structure

- Modify `docs/superpowers/specs/2026-05-04-point-features-design.md` — document POI tags and marker behavior.
- Modify `src/world/point_feature.rs` — add POI classification, categories, colors, marker mesh, and tests.
- Modify `src/world/loader.rs` — add/adjust tests proving POI node ingestion and tile rendering use existing point feature path.

## Task 1: Add POI classification

**Files:**
- Modify: `src/world/point_feature.rs`

- [ ] **Step 1: Extend enums and style**

Add POI category support:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointFeatureKind {
    Tree,
    Landmark,
    Nature,
    Poi,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoiCategory {
    Food,
    Service,
    Shop,
    Tourism,
    Leisure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointFeatureStyle {
    pub kind: PointFeatureKind,
    pub poi_category: Option<PoiCategory>,
}
```

Update existing style constructors for tree/landmark/nature to set `poi_category: None`.

- [ ] **Step 2: Add POI classifier helper**

Add a helper:

```rust
fn poi_category(tags: &HashMap<String, String>) -> Option<PoiCategory> {
    if matches!(tags.get("amenity").map(String::as_str), Some("restaurant" | "cafe" | "bar" | "pub" | "fast_food")) {
        return Some(PoiCategory::Food);
    }
    if matches!(tags.get("amenity").map(String::as_str), Some("school" | "hospital" | "clinic" | "pharmacy" | "bank" | "fuel" | "parking")) {
        return Some(PoiCategory::Service);
    }
    if tags.contains_key("shop") {
        return Some(PoiCategory::Shop);
    }
    if matches!(tags.get("tourism").map(String::as_str), Some("hotel" | "museum" | "guest_house")) {
        return Some(PoiCategory::Tourism);
    }
    if matches!(tags.get("leisure").map(String::as_str), Some("park" | "playground" | "sports_centre" | "pitch")) {
        return Some(PoiCategory::Leisure);
    }
    None
}
```

In `point_feature_style()`, after existing landmark/nature checks, return `PointFeatureKind::Poi` when this helper returns a category.

- [ ] **Step 3: Add classifier tests**

Add tests for:

```rust
classifies_amenity_restaurant_as_food_poi
classifies_shop_as_shop_poi
classifies_tourism_hotel_as_tourism_poi
classifies_leisure_playground_as_leisure_poi
```

Each test should assert `style.kind == PointFeatureKind::Poi` and the expected `poi_category`.

- [ ] **Step 4: Verify classification**

Run:

```bash
cargo test world::point_feature::tests::classifies_amenity_restaurant_as_food_poi -- --exact
cargo test world::point_feature -- --nocapture
```

Expected: all point feature tests pass.

## Task 2: Render POI markers

**Files:**
- Modify: `src/world/point_feature.rs`

- [ ] **Step 1: Add POI colors and marker renderer**

Add category colors:

```rust
const POI_FOOD_COLOR: [f32; 3] = [0.86, 0.28, 0.18];
const POI_SERVICE_COLOR: [f32; 3] = [0.20, 0.42, 0.86];
const POI_SHOP_COLOR: [f32; 3] = [0.82, 0.36, 0.78];
const POI_TOURISM_COLOR: [f32; 3] = [0.92, 0.66, 0.18];
const POI_LEISURE_COLOR: [f32; 3] = [0.24, 0.68, 0.28];
const POI_POST_COLOR: [f32; 3] = [0.18, 0.18, 0.18];
```

Add `poi_color(category: PoiCategory) -> [f32; 3]` and `append_poi_marker(point, elevation, category, verts, idxs)` using a narrow post box and a colored pyramid or cube cap.

- [ ] **Step 2: Route generation through POI renderer**

Update `generate_point_feature()` match:

```rust
PointFeatureKind::Poi => append_poi_marker(
    point,
    elevation,
    style.poi_category.expect("POI styles carry a category"),
    verts,
    idxs,
),
```

- [ ] **Step 3: Add marker tests**

Add `poi_marker_emits_post_and_category_cap()` that generates `amenity=restaurant` and asserts vertices include `POI_POST_COLOR`, `POI_FOOD_COLOR`, and `feature::POINT_FEATURE`.

- [ ] **Step 4: Verify rendering**

Run:

```bash
cargo test world::point_feature::tests::poi_marker_emits_post_and_category_cap -- --exact
cargo test world::point_feature -- --nocapture
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: all pass.

## Task 3: Add loader coverage for POIs

**Files:**
- Modify: `src/world/loader.rs`

- [ ] **Step 1: Add POI ingestion XML test**

Add a loader test that writes a tiny XML file with a tagged POI node:

```xml
<node id="1" lat="38.0" lon="-121.0">
  <tag k="amenity" v="restaurant"/>
</node>
```

Assert `source.point_features.len() == 1` and the tag is preserved.

- [ ] **Step 2: Add tile POI render test**

Add or extend tile mesh test with a `ResolvedPointFeature` tagged `shop=convenience`; assert generated tile vertices include `feature::POINT_FEATURE`.

- [ ] **Step 3: Verify loader coverage**

Run:

```bash
cargo test world::loader::tests::load_world_source_classifies_poi_nodes -- --exact
cargo test world::loader::tests::tile_mesh_emits_poi_point_feature_geometry -- --exact
cargo check --all-targets
```

Expected: all pass.

## Task 4: Final verification

**Files:**
- No source edits unless checks fail.

- [ ] **Step 1: Run full verification**

```bash
cargo fmt -- --check
make checkall
graphify update .
```

Expected: all pass.

- [ ] **Step 2: Commit implementation**

```bash
git add docs/superpowers/specs/2026-05-04-point-features-design.md docs/superpowers/plans/2026-05-04-poi-point-features.md src/world/point_feature.rs src/world/loader.rs
git commit -m "feat: add poi point feature support"
```

- [ ] **Step 3: Save/update vault note**

Update the existing point-feature Parsidion note to include POI support and rebuild the vault index.

## Self-Review

Spec coverage: POI classification, rendering, loader ingestion, tile rendering, tests, verification, and vault update are covered. Placeholder scan: no TBD/TODO placeholders. Type consistency: `PointFeatureKind::Poi`, `PoiCategory`, `poi_category`, and `append_poi_marker` are used consistently.
