> ⚠️ Historical implementation plan (2026-05) — retained for reference; current behavior may differ. See `docs/ARCHITECTURE.md` and the source code.

# Shared Overture POI Source Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Overture Maps POI ingestion into `par-osm-rust`, then update `osm-to-bedrock` and `osm-world` to use shared Overture-preferred POI source policy.

**Architecture:** `par-osm-rust` becomes the owner of Overture fetch/cache/parse, source metadata, POI source policy, and normalized OSM XML writing. `osm-to-bedrock` keeps its CLI/server surface but delegates source composition to `par_osm_rust::sources`; `osm-world` uses the same shared source composition during area preparation and still renders normalized OSM-style tags.

**Tech Stack:** Rust 2024, `par-osm-rust`, `osm-to-bedrock`, `osm-world`, `serde`, `serde_json`, `sha2`, `tempfile`, `overturemaps` Python CLI, Cargo tests, `make checkall`, `graphify update .`.

---

## Repositories and Working Directories

Run tasks from these roots as specified:

- Shared crate: `/Users/probello/Repos/par-osm-rust`
- Bedrock converter: `/Users/probello/Repos/osm-to-bedrock`
- World renderer/API: `/Users/probello/Repos/osm-world`

Commit after each task in the repository it changes. When a task changes more than one repo, commit each repo separately before moving on.

## File Structure

### `par-osm-rust`

- Modify `Cargo.toml` — add runtime `tempfile` dependency used by Overture CLI downloads.
- Modify `src/lib.rs` — export new `overture` and `sources` modules.
- Modify `src/osm.rs` — add `FeatureSource`, mark POI/address nodes with source, and add normalized XML writing.
- Create `src/overture.rs` — moved Overture CLI/cache/GeoJSON parser from `osm-to-bedrock` with imports adjusted to shared crate types.
- Create `src/sources.rs` — source options, POI policy, dedupe, fallback status, and bbox fetch orchestration.

### `osm-to-bedrock`

- Modify `src/lib.rs` — stop exporting a local Overture implementation as owner; keep compatibility through re-exports.
- Modify `src/overture.rs` — replace implementation with re-export shim from `par_osm_rust::overture`.
- Modify `src/params.rs` — re-export shared Overture/source enums and params for compatibility.
- Modify `src/main.rs` — map CLI options into shared `SourceOptions` and use shared fetch orchestration.
- Modify `src/server.rs` — map API options into shared `SourceOptions` and use shared fetch orchestration.
- Modify tests in touched files to use shared type paths and source metadata.

### `osm-world`

- Modify `src/server.rs` — extend prepare request/response with Overture source options, use shared source orchestration, write normalized prepared OSM XML, and report warnings.
- Modify server tests in `src/server.rs` — cover source options, fallback warnings, and prepared XML containing Overture-style POIs.
- Run `graphify update .` after code changes.

---

## Task 1: Add POI source metadata to `par-osm-rust`

**Files:**
- Modify: `/Users/probello/Repos/par-osm-rust/src/osm.rs`

- [ ] **Step 1: Write failing source metadata tests**

Add these tests inside `#[cfg(test)] mod tests` in `/Users/probello/Repos/par-osm-rust/src/osm.rs`:

```rust
#[test]
fn parse_xml_poi_nodes_are_marked_osm_source() {
    let xml = r#"<?xml version="1.0"?>
<osm version="0.6">
  <node id="1" lat="51.5" lon="-0.1">
    <tag k="amenity" v="restaurant"/>
    <tag k="name" v="The Pub"/>
  </node>
</osm>"#;

    let data = parse_osm_xml_str(xml).unwrap();

    assert_eq!(data.poi_nodes.len(), 1);
    assert_eq!(data.poi_nodes[0].source, FeatureSource::Osm);
}

#[test]
fn parse_xml_address_nodes_are_marked_osm_source() {
    let xml = r#"<?xml version="1.0"?>
<osm version="0.6">
  <node id="1" lat="51.5" lon="-0.1">
    <tag k="addr:housenumber" v="42"/>
    <tag k="addr:street" v="Baker Street"/>
  </node>
</osm>"#;

    let data = parse_osm_xml_str(xml).unwrap();

    assert_eq!(data.addr_nodes.len(), 1);
    assert_eq!(data.addr_nodes[0].source, FeatureSource::Osm);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `/Users/probello/Repos/par-osm-rust`:

```bash
cargo test parse_xml_poi_nodes_are_marked_osm_source -- --nocapture
cargo test parse_xml_address_nodes_are_marked_osm_source -- --nocapture
```

Expected: compile failure because `FeatureSource` and `OsmPoiNode::source` do not exist.

- [ ] **Step 3: Add `FeatureSource` and source field**

In `/Users/probello/Repos/par-osm-rust/src/osm.rs`, replace the current `OsmPoiNode` definition with:

```rust
/// Data source for normalized map features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureSource {
    #[default]
    Osm,
    Overture,
    Synthetic,
}

/// An OSM node that carries feature tags (amenity, shop, tourism, etc.).
/// Used for POI marker placement.
#[derive(Debug, Clone)]
pub struct OsmPoiNode {
    pub lat: f64,
    pub lon: f64,
    pub tags: HashMap<String, String>,
    pub source: FeatureSource,
}
```

- [ ] **Step 4: Mark parsed OSM POI/address nodes as `FeatureSource::Osm`**

In `/Users/probello/Repos/par-osm-rust/src/osm.rs`, update every `OsmPoiNode { lat, lon, tags: tags.clone() }` and XML equivalent to include `source: FeatureSource::Osm`:

```rust
poi_nodes.push(OsmPoiNode {
    lat,
    lon,
    tags: tags.clone(),
    source: FeatureSource::Osm,
});

addr_nodes.push(OsmPoiNode {
    lat,
    lon,
    tags: tags.clone(),
    source: FeatureSource::Osm,
});
```

For XML nodes using `cur_lat`, `cur_lon`, and `cur_node_tags`, use:

```rust
poi_nodes.push(OsmPoiNode {
    lat: cur_lat,
    lon: cur_lon,
    tags: cur_node_tags.clone(),
    source: FeatureSource::Osm,
});

addr_nodes.push(OsmPoiNode {
    lat: cur_lat,
    lon: cur_lon,
    tags: cur_node_tags.clone(),
    source: FeatureSource::Osm,
});
```

- [ ] **Step 5: Update existing test constructors**

Search for `OsmPoiNode {` in `/Users/probello/Repos/par-osm-rust/src` and add `source: FeatureSource::Osm` to OSM test data and `source: FeatureSource::Overture` only in tests that explicitly model Overture data.

Run:

```bash
rg "OsmPoiNode \{" src
```

Expected after edits: every constructor includes a `source` field.

- [ ] **Step 6: Run focused tests**

Run from `/Users/probello/Repos/par-osm-rust`:

```bash
cargo test parse_xml_poi_nodes_are_marked_osm_source -- --nocapture
cargo test parse_xml_address_nodes_are_marked_osm_source -- --nocapture
```

Expected: both tests pass.

- [ ] **Step 7: Run shared crate tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/osm.rs
git commit -m "feat: track osm poi feature source"
```

---

## Task 2: Move Overture module into `par-osm-rust`

**Files:**
- Modify: `/Users/probello/Repos/par-osm-rust/Cargo.toml`
- Modify: `/Users/probello/Repos/par-osm-rust/src/lib.rs`
- Create: `/Users/probello/Repos/par-osm-rust/src/overture.rs`

- [ ] **Step 1: Copy the proven Overture module**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cp /Users/probello/Repos/osm-to-bedrock/src/overture.rs src/overture.rs
```

- [ ] **Step 2: Add runtime dependency**

In `/Users/probello/Repos/par-osm-rust/Cargo.toml`, add `tempfile = "3.27.0"` under `[dependencies]`:

```toml
tempfile = "3.27.0"
```

Do not remove the existing dev-dependency until `cargo test` confirms duplicate dependency declarations are acceptable. If Cargo warns about duplicate dependency sections for the same crate, keep the `[dependencies]` entry and remove `tempfile` from `[dev-dependencies]`.

- [ ] **Step 3: Export the module**

In `/Users/probello/Repos/par-osm-rust/src/lib.rs`, add:

```rust
pub mod overture;
```

- [ ] **Step 4: Move Overture parameter types into the shared module**

At the top of `/Users/probello/Repos/par-osm-rust/src/overture.rs`, replace:

```rust
use crate::params::{OvertureParams, OvertureTheme};
```

with the shared type definitions and imports:

```rust
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Overture Maps theme selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OvertureTheme {
    Building,
    Transportation,
    Place,
    Base,
    Address,
}

impl OvertureTheme {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Building,
            Self::Transportation,
            Self::Place,
            Self::Base,
            Self::Address,
        ]
    }

    pub fn cli_types(&self) -> Vec<&'static str> {
        match self {
            Self::Building => vec!["building"],
            Self::Transportation => vec!["segment"],
            Self::Place => vec!["place"],
            Self::Base => vec!["land", "land_use", "water"],
            Self::Address => vec!["address"],
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().trim_end_matches('s') {
            "building" => Some(Self::Building),
            "transportation" | "transport" | "road" | "segment" => Some(Self::Transportation),
            "place" => Some(Self::Place),
            "base" | "land" | "land_use" | "landuse" | "water" => Some(Self::Base),
            "address" | "addr" => Some(Self::Address),
            _ => None,
        }
    }
}

impl std::fmt::Display for OvertureTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Building => write!(f, "building"),
            Self::Transportation => write!(f, "transportation"),
            Self::Place => write!(f, "place"),
            Self::Base => write!(f, "base"),
            Self::Address => write!(f, "address"),
        }
    }
}

/// Which data source wins when Overture and OSM both cover the same non-POI theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePriority {
    Overture,
    Osm,
    #[default]
    Both,
}

/// Parameters controlling Overture Maps data integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvertureParams {
    pub enabled: bool,
    pub themes: Vec<OvertureTheme>,
    pub priority: HashMap<OvertureTheme, ThemePriority>,
    pub timeout_secs: u64,
}

impl Default for OvertureParams {
    fn default() -> Self {
        Self {
            enabled: false,
            themes: OvertureTheme::all(),
            priority: HashMap::new(),
            timeout_secs: 120,
        }
    }
}

impl OvertureParams {
    pub fn priority_for(&self, theme: OvertureTheme) -> ThemePriority {
        self.priority.get(&theme).copied().unwrap_or(ThemePriority::Both)
    }
}
```

If `HashMap`, `Serialize`, or `Deserialize` are already imported elsewhere in the copied file, keep one import for each name.

- [ ] **Step 5: Mark Overture POIs with source metadata**

In `/Users/probello/Repos/par-osm-rust/src/overture.rs`, change the import:

```rust
use crate::osm::{OsmData, OsmNode, OsmPoiNode, OsmWay};
```

to:

```rust
use crate::osm::{FeatureSource, OsmData, OsmNode, OsmPoiNode, OsmWay};
```

In `parse_overture_geojson()`, update `let poi = OsmPoiNode { ... }` to:

```rust
let poi = OsmPoiNode {
    lat: node.lat,
    lon: node.lon,
    tags: tags.clone(),
    source: FeatureSource::Overture,
};
```

For address points, also use `source: FeatureSource::Overture`; address source metadata is useful for diagnostics even though address signs are not the primary POI target.

- [ ] **Step 6: Update empty/test `OsmData` constructors**

In `/Users/probello/Repos/par-osm-rust/src/overture.rs`, update every `OsmPoiNode` literal in tests to include source. For Overture parser tests, use:

```rust
source: FeatureSource::Overture,
```

Run:

```bash
rg "OsmPoiNode \{" src/overture.rs
```

Expected after edits: every constructor has `source:`.

- [ ] **Step 7: Run Overture tests**

Run from `/Users/probello/Repos/par-osm-rust`:

```bash
cargo test overture -- --nocapture
```

Expected: Overture parser/cache tests pass from the shared crate.

- [ ] **Step 8: Run all shared crate tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/overture.rs
git commit -m "feat: move overture support into shared crate"
```

---

## Task 3: Add normalized OSM XML writer to `par-osm-rust`

**Files:**
- Modify: `/Users/probello/Repos/par-osm-rust/src/osm.rs`

- [ ] **Step 1: Write failing XML writer tests**

Add these tests inside `#[cfg(test)] mod tests` in `/Users/probello/Repos/par-osm-rust/src/osm.rs`:

```rust
#[test]
fn write_osm_xml_string_serializes_poi_nodes_with_tags() {
    let data = OsmData {
        nodes: HashMap::new(),
        ways: Vec::new(),
        ways_by_id: HashMap::new(),
        relations: Vec::new(),
        bounds: Some((51.5, -0.1, 51.6, -0.0)),
        poi_nodes: vec![OsmPoiNode {
            lat: 51.55,
            lon: -0.05,
            tags: HashMap::from([
                ("amenity".to_string(), "restaurant".to_string()),
                ("name".to_string(), "A&B Cafe".to_string()),
            ]),
            source: FeatureSource::Overture,
        }],
        addr_nodes: Vec::new(),
        tree_nodes: Vec::new(),
    };

    let xml = write_osm_xml_string(&data);

    assert!(xml.contains("<bounds minlat=\"51.5\" minlon=\"-0.1\" maxlat=\"51.6\" maxlon=\"-0\"/>"));
    assert!(xml.contains("<tag k=\"amenity\" v=\"restaurant\"/>"));
    assert!(xml.contains("<tag k=\"name\" v=\"A&amp;B Cafe\"/>"));
}

#[test]
fn write_osm_xml_string_round_trips_poi_nodes_through_parser() {
    let data = OsmData {
        nodes: HashMap::new(),
        ways: Vec::new(),
        ways_by_id: HashMap::new(),
        relations: Vec::new(),
        bounds: Some((51.5, -0.1, 51.6, -0.0)),
        poi_nodes: vec![OsmPoiNode {
            lat: 51.55,
            lon: -0.05,
            tags: HashMap::from([("shop".to_string(), "bakery".to_string())]),
            source: FeatureSource::Overture,
        }],
        addr_nodes: Vec::new(),
        tree_nodes: Vec::new(),
    };

    let xml = write_osm_xml_string(&data);
    let parsed = parse_osm_xml_str(&xml).unwrap();

    assert_eq!(parsed.poi_nodes.len(), 1);
    assert_eq!(parsed.poi_nodes[0].tags.get("shop").map(String::as_str), Some("bakery"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `/Users/probello/Repos/par-osm-rust`:

```bash
cargo test write_osm_xml_string_serializes_poi_nodes_with_tags -- --nocapture
cargo test write_osm_xml_string_round_trips_poi_nodes_through_parser -- --nocapture
```

Expected: compile failure because `write_osm_xml_string` does not exist.

- [ ] **Step 3: Add XML escaping helper**

Add this helper near the bottom of `/Users/probello/Repos/par-osm-rust/src/osm.rs`, before the test module:

```rust
fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
```

- [ ] **Step 4: Add tag-writing helper**

Add below `escape_xml_attr()`:

```rust
fn write_tags(xml: &mut String, tags: &HashMap<String, String>) {
    let mut entries: Vec<_> = tags.iter().collect();
    entries.sort_by(|(ak, _), (bk, _)| ak.cmp(bk));
    for (key, value) in entries {
        xml.push_str("    <tag k=\"");
        xml.push_str(&escape_xml_attr(key));
        xml.push_str("\" v=\"");
        xml.push_str(&escape_xml_attr(value));
        xml.push_str("\"/>\n");
    }
}
```

- [ ] **Step 5: Add `write_osm_xml_string()`**

Add below `write_tags()`:

```rust
/// Serialize normalized [`OsmData`] into simple OSM XML that this crate and
/// `osm-world` can parse again.
pub fn write_osm_xml_string(data: &OsmData) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<osm version=\"0.6\">\n");

    if let Some((min_lat, min_lon, max_lat, max_lon)) = data.bounds {
        xml.push_str(&format!(
            "  <bounds minlat=\"{}\" minlon=\"{}\" maxlat=\"{}\" maxlon=\"{}\"/>\n",
            min_lat, min_lon, max_lat, max_lon
        ));
    }

    let mut nodes: Vec<_> = data.nodes.iter().collect();
    nodes.sort_by_key(|(id, _)| **id);
    for (id, node) in nodes {
        xml.push_str(&format!(
            "  <node id=\"{}\" lat=\"{}\" lon=\"{}\"/>\n",
            id, node.lat, node.lon
        ));
    }

    let mut synthetic_id = -9_000_000_000_i64;
    for poi in &data.poi_nodes {
        xml.push_str(&format!(
            "  <node id=\"{}\" lat=\"{}\" lon=\"{}\">\n",
            synthetic_id, poi.lat, poi.lon
        ));
        write_tags(&mut xml, &poi.tags);
        xml.push_str("  </node>\n");
        synthetic_id -= 1;
    }

    for addr in &data.addr_nodes {
        xml.push_str(&format!(
            "  <node id=\"{}\" lat=\"{}\" lon=\"{}\">\n",
            synthetic_id, addr.lat, addr.lon
        ));
        write_tags(&mut xml, &addr.tags);
        xml.push_str("  </node>\n");
        synthetic_id -= 1;
    }

    for tree in &data.tree_nodes {
        xml.push_str(&format!(
            "  <node id=\"{}\" lat=\"{}\" lon=\"{}\">\n    <tag k=\"natural\" v=\"tree\"/>\n  </node>\n",
            synthetic_id, tree.lat, tree.lon
        ));
        synthetic_id -= 1;
    }

    for (idx, way) in data.ways.iter().enumerate() {
        let way_id = data
            .ways_by_id
            .iter()
            .find_map(|(id, way_idx)| (*way_idx == idx).then_some(*id))
            .unwrap_or_else(|| -8_000_000_000_i64 - idx as i64);
        xml.push_str(&format!("  <way id=\"{}\">\n", way_id));
        for node_ref in &way.node_refs {
            xml.push_str(&format!("    <nd ref=\"{}\"/>\n", node_ref));
        }
        write_tags(&mut xml, &way.tags);
        xml.push_str("  </way>\n");
    }

    xml.push_str("</osm>\n");
    xml
}
```

- [ ] **Step 6: Run focused writer tests**

```bash
cargo test write_osm_xml_string_serializes_poi_nodes_with_tags -- --nocapture
cargo test write_osm_xml_string_round_trips_poi_nodes_through_parser -- --nocapture
```

Expected: both tests pass.

- [ ] **Step 7: Run all shared crate tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/osm.rs
git commit -m "feat: write normalized osm xml"
```

---

## Task 4: Add shared POI source policy and dedupe

**Files:**
- Modify: `/Users/probello/Repos/par-osm-rust/src/lib.rs`
- Create: `/Users/probello/Repos/par-osm-rust/src/sources.rs`

- [ ] **Step 1: Export `sources` module**

In `/Users/probello/Repos/par-osm-rust/src/lib.rs`, add:

```rust
pub mod sources;
```

- [ ] **Step 2: Create failing policy tests**

Create `/Users/probello/Repos/par-osm-rust/src/sources.rs` with this initial test module and imports:

```rust
use std::collections::HashMap;

use crate::osm::{FeatureSource, OsmData, OsmNode, OsmPoiNode};
use crate::overture::OvertureParams;
use crate::filter::FeatureFilter;

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_data() -> OsmData {
        OsmData {
            nodes: HashMap::new(),
            ways: Vec::new(),
            ways_by_id: HashMap::new(),
            relations: Vec::new(),
            bounds: Some((0.0, 0.0, 1.0, 1.0)),
            poi_nodes: Vec::new(),
            addr_nodes: Vec::new(),
            tree_nodes: Vec::new(),
        }
    }

    fn poi(lat: f64, lon: f64, key: &str, value: &str, name: &str, source: FeatureSource) -> OsmPoiNode {
        let mut tags = HashMap::from([(key.to_string(), value.to_string())]);
        if !name.is_empty() {
            tags.insert("name".to_string(), name.to_string());
        }
        OsmPoiNode { lat, lon, tags, source }
    }

    #[test]
    fn osm_only_keeps_osm_pois() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(0.0, 0.0, "amenity", "restaurant", "Diner", FeatureSource::Osm));
        let mut overture = empty_data();
        overture.poi_nodes.push(poi(0.0, 0.0, "amenity", "restaurant", "Diner", FeatureSource::Overture));

        let merged = merge_source_data(osm, Some(overture), PoiSourceMode::OsmOnly);

        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.poi_nodes[0].source, FeatureSource::Osm);
    }

    #[test]
    fn overture_only_keeps_overture_pois() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(0.0, 0.0, "amenity", "restaurant", "Diner", FeatureSource::Osm));
        let mut overture = empty_data();
        overture.poi_nodes.push(poi(0.0, 0.0, "amenity", "restaurant", "Diner", FeatureSource::Overture));

        let merged = merge_source_data(osm, Some(overture), PoiSourceMode::OvertureOnly);

        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.poi_nodes[0].source, FeatureSource::Overture);
    }

    #[test]
    fn overture_preferred_dedupes_named_pois_with_overture_winning() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(51.50000, -0.10000, "amenity", "restaurant", "Diner", FeatureSource::Osm));
        let mut overture = empty_data();
        overture.poi_nodes.push(poi(51.50005, -0.10005, "amenity", "restaurant", "Diner", FeatureSource::Overture));

        let merged = merge_source_data(osm, Some(overture), PoiSourceMode::OverturePreferred);

        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.poi_nodes[0].source, FeatureSource::Overture);
    }

    #[test]
    fn overture_preferred_falls_back_when_overture_missing() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(0.0, 0.0, "shop", "bakery", "Bakery", FeatureSource::Osm));

        let merged = merge_source_data(osm, None, PoiSourceMode::OverturePreferred);

        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.poi_nodes[0].source, FeatureSource::Osm);
        assert!(merged.warnings.iter().any(|warning| warning.contains("Overture POIs unavailable")));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run from `/Users/probello/Repos/par-osm-rust`:

```bash
cargo test sources -- --nocapture
```

Expected: compile failure because `PoiSourceMode`, `merge_source_data`, and result types do not exist.

- [ ] **Step 4: Add source option and status types**

Above the test module in `/Users/probello/Repos/par-osm-rust/src/sources.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoiSourceMode {
    OsmOnly,
    OvertureOnly,
    Both,
    OverturePreferred,
}

impl Default for PoiSourceMode {
    fn default() -> Self {
        Self::OverturePreferred
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OvertureFailureMode {
    FallbackToOsm,
    Fail,
}

impl Default for OvertureFailureMode {
    fn default() -> Self {
        Self::FallbackToOsm
    }
}

#[derive(Debug, Clone)]
pub struct SourceOptions {
    pub filter: FeatureFilter,
    pub overpass_url: Option<String>,
    pub use_overpass_cache: bool,
    pub overture: OvertureParams,
    pub poi_source_mode: PoiSourceMode,
    pub overture_failure_mode: OvertureFailureMode,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    OsmOnly,
    OvertureOnly,
    Both,
    OverturePreferred,
    OvertureFallbackToOsm,
}

pub struct SourceFetchResult {
    pub data: OsmData,
    pub status: SourceStatus,
    pub warnings: Vec<String>,
}
```

- [ ] **Step 5: Add POI category helpers**

Below the types, add:

```rust
fn normalized_name(tags: &HashMap<String, String>) -> Option<String> {
    tags.get("name")
        .map(|name| name.trim().to_lowercase())
        .filter(|name| !name.is_empty())
}

fn poi_category(tags: &HashMap<String, String>) -> String {
    for key in ["amenity", "shop", "tourism", "leisure", "historic", "man_made"] {
        if let Some(value) = tags.get(key) {
            return format!("{key}:{value}");
        }
    }
    "unknown".to_string()
}

fn metres_between(a: &OsmPoiNode, b: &OsmPoiNode) -> f64 {
    let mean_lat = ((a.lat + b.lat) * 0.5).to_radians();
    let metres_per_degree_lat = 111_320.0;
    let metres_per_degree_lon = 111_320.0 * mean_lat.cos().abs().max(0.01);
    let dx = (a.lon - b.lon) * metres_per_degree_lon;
    let dz = (a.lat - b.lat) * metres_per_degree_lat;
    (dx * dx + dz * dz).sqrt()
}

fn poi_duplicates(a: &OsmPoiNode, b: &OsmPoiNode) -> bool {
    let same_category = poi_category(&a.tags) == poi_category(&b.tags);
    if !same_category {
        return false;
    }
    match (normalized_name(&a.tags), normalized_name(&b.tags)) {
        (Some(a_name), Some(b_name)) if a_name == b_name => metres_between(a, b) <= 25.0,
        (None, None) => metres_between(a, b) <= 10.0,
        _ => false,
    }
}
```

- [ ] **Step 6: Add dedupe and merge policy implementation**

Below the helper functions, add:

```rust
fn dedupe_pois_with_overture_preference(mut pois: Vec<OsmPoiNode>) -> Vec<OsmPoiNode> {
    pois.sort_by_key(|poi| match poi.source {
        FeatureSource::Overture => 0,
        FeatureSource::Osm => 1,
        FeatureSource::Synthetic => 2,
    });

    let mut kept: Vec<OsmPoiNode> = Vec::new();
    'next_poi: for poi in pois {
        for existing in &kept {
            if poi_duplicates(existing, &poi) {
                continue 'next_poi;
            }
        }
        kept.push(poi);
    }
    kept
}

pub fn merge_source_data(
    mut osm_data: OsmData,
    overture_data: Option<OsmData>,
    poi_source_mode: PoiSourceMode,
) -> SourceFetchResult {
    let original_osm_pois = osm_data.poi_nodes.clone();
    let mut warnings = Vec::new();

    match (poi_source_mode, overture_data) {
        (PoiSourceMode::OsmOnly, Some(mut overture)) => {
            overture.poi_nodes.clear();
            osm_data.merge(overture);
            osm_data.poi_nodes = original_osm_pois;
            SourceFetchResult { data: osm_data, status: SourceStatus::OsmOnly, warnings }
        }
        (PoiSourceMode::OsmOnly, None) => {
            SourceFetchResult { data: osm_data, status: SourceStatus::OsmOnly, warnings }
        }
        (PoiSourceMode::OvertureOnly, Some(mut overture)) => {
            let overture_pois = overture.poi_nodes.clone();
            osm_data.poi_nodes = overture_pois;
            overture.poi_nodes.clear();
            osm_data.merge(overture);
            SourceFetchResult { data: osm_data, status: SourceStatus::OvertureOnly, warnings }
        }
        (PoiSourceMode::OvertureOnly, None) => {
            osm_data.poi_nodes.clear();
            warnings.push("Overture POIs unavailable for overture-only mode".to_string());
            SourceFetchResult { data: osm_data, status: SourceStatus::OvertureOnly, warnings }
        }
        (PoiSourceMode::Both, Some(mut overture)) => {
            let mut all_pois = original_osm_pois;
            all_pois.extend(overture.poi_nodes.clone());
            overture.poi_nodes.clear();
            osm_data.merge(overture);
            osm_data.poi_nodes = dedupe_pois_with_overture_preference(all_pois);
            SourceFetchResult { data: osm_data, status: SourceStatus::Both, warnings }
        }
        (PoiSourceMode::Both, None) => {
            warnings.push("Overture POIs unavailable; using OSM POIs only".to_string());
            SourceFetchResult { data: osm_data, status: SourceStatus::OvertureFallbackToOsm, warnings }
        }
        (PoiSourceMode::OverturePreferred, Some(mut overture)) if !overture.poi_nodes.is_empty() => {
            let mut all_pois = original_osm_pois;
            all_pois.extend(overture.poi_nodes.clone());
            overture.poi_nodes.clear();
            osm_data.merge(overture);
            osm_data.poi_nodes = dedupe_pois_with_overture_preference(all_pois);
            SourceFetchResult { data: osm_data, status: SourceStatus::OverturePreferred, warnings }
        }
        (PoiSourceMode::OverturePreferred, Some(mut overture)) => {
            warnings.push("Overture POIs unavailable; using OSM POIs only".to_string());
            overture.poi_nodes.clear();
            osm_data.merge(overture);
            osm_data.poi_nodes = original_osm_pois;
            SourceFetchResult { data: osm_data, status: SourceStatus::OvertureFallbackToOsm, warnings }
        }
        (PoiSourceMode::OverturePreferred, None) => {
            warnings.push("Overture POIs unavailable; using OSM POIs only".to_string());
            SourceFetchResult { data: osm_data, status: SourceStatus::OvertureFallbackToOsm, warnings }
        }
    }
}
```

- [ ] **Step 7: Remove unused import**

If `OsmNode` is unused in `/Users/probello/Repos/par-osm-rust/src/sources.rs`, change:

```rust
use crate::osm::{FeatureSource, OsmData, OsmNode, OsmPoiNode};
```

to:

```rust
use crate::osm::{FeatureSource, OsmData, OsmPoiNode};
```

- [ ] **Step 8: Run focused tests**

```bash
cargo test sources -- --nocapture
```

Expected: all source policy tests pass.

- [ ] **Step 9: Run all shared crate tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 10: Commit**

```bash
git add src/lib.rs src/sources.rs
git commit -m "feat: add shared poi source policy"
```

---

## Task 5: Add shared bbox fetch orchestrator

**Files:**
- Modify: `/Users/probello/Repos/par-osm-rust/src/sources.rs`

- [ ] **Step 1: Write failing option default test**

Add this test to `/Users/probello/Repos/par-osm-rust/src/sources.rs`:

```rust
#[test]
fn source_options_default_uses_overture_preferred_with_fallback() {
    let options = SourceOptions::default();

    assert_eq!(options.poi_source_mode, PoiSourceMode::OverturePreferred);
    assert_eq!(options.overture_failure_mode, OvertureFailureMode::FallbackToOsm);
    assert!(options.use_overpass_cache);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test sources::tests::source_options_default_uses_overture_preferred_with_fallback -- --nocapture
```

Expected: compile failure because `SourceOptions::default()` is not implemented.

- [ ] **Step 3: Implement `Default` for `SourceOptions`**

Add below `SourceOptions`:

```rust
impl Default for SourceOptions {
    fn default() -> Self {
        Self {
            filter: FeatureFilter::default(),
            overpass_url: None,
            use_overpass_cache: true,
            overture: OvertureParams::default(),
            poi_source_mode: PoiSourceMode::OverturePreferred,
            overture_failure_mode: OvertureFailureMode::FallbackToOsm,
        }
    }
}
```

- [ ] **Step 4: Add fetch orchestrator implementation**

Add these imports at the top of `/Users/probello/Repos/par-osm-rust/src/sources.rs`:

```rust
use anyhow::Result;
```

Then add below `merge_source_data()`:

```rust
pub fn fetch_map_data(
    bbox: (f64, f64, f64, f64),
    options: &SourceOptions,
    progress_cb: &mut dyn FnMut(f32, &str),
) -> Result<SourceFetchResult> {
    progress_cb(0.0, "Fetching OSM data…");
    let overpass_url = options
        .overpass_url
        .as_deref()
        .unwrap_or_else(crate::overpass::default_overpass_url);
    let osm_data = crate::overpass::fetch_osm_data(
        bbox,
        &options.filter,
        options.use_overpass_cache,
        overpass_url,
    )?;

    let overture_requested = options.overture.enabled
        || matches!(
            options.poi_source_mode,
            PoiSourceMode::OvertureOnly | PoiSourceMode::Both | PoiSourceMode::OverturePreferred
        );

    let overture_data = if overture_requested {
        let mut overture_params = options.overture.clone();
        overture_params.enabled = true;
        match crate::overture::fetch_overture_data(bbox, &overture_params, progress_cb) {
            Ok(data) => Some(data),
            Err(err) if options.overture_failure_mode == OvertureFailureMode::FallbackToOsm => {
                log::warn!("Overture fetch failed; falling back to OSM POIs: {err:#}");
                None
            }
            Err(err) => return Err(err),
        }
    } else {
        None
    };

    let mut result = merge_source_data(osm_data, overture_data, options.poi_source_mode);
    result.data.clip_to_bbox(bbox);
    progress_cb(1.0, "Map data ready");
    Ok(result)
}
```

- [ ] **Step 5: Run focused tests**

```bash
cargo test sources -- --nocapture
```

Expected: all source tests pass.

- [ ] **Step 6: Run all shared crate tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/sources.rs
git commit -m "feat: orchestrate shared map source fetches"
```

---

## Task 6: Convert `osm-to-bedrock` to shared Overture types

**Files:**
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/overture.rs`
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/params.rs`
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/lib.rs`

- [ ] **Step 1: Replace local Overture module with compatibility re-exports**

Replace the full contents of `/Users/probello/Repos/osm-to-bedrock/src/overture.rs` with:

```rust
//! Compatibility re-exports for Overture Maps support.
//!
//! The implementation lives in `par-osm-rust` so `osm-to-bedrock` and
//! `osm-world` share one source of truth for Overture fetching, parsing,
//! caching, and source policy.

pub use par_osm_rust::overture::*;
```

- [ ] **Step 2: Replace local Overture params with shared re-exports**

In `/Users/probello/Repos/osm-to-bedrock/src/params.rs`, remove the local definitions of `OvertureTheme`, `ThemePriority`, and `OvertureParams`, including their impl blocks. Insert this near the top after imports:

```rust
pub use par_osm_rust::overture::{OvertureParams, OvertureTheme, ThemePriority};
pub use par_osm_rust::sources::{OvertureFailureMode, PoiSourceMode, SourceOptions, SourceStatus};
```

Keep `ConvertParams` and `TerrainParams` unchanged.

- [ ] **Step 3: Remove now-unused imports**

In `/Users/probello/Repos/osm-to-bedrock/src/params.rs`, remove:

```rust
use std::collections::HashMap;
```

if it is no longer used after deleting the local Overture types.

- [ ] **Step 4: Run focused check**

Run from `/Users/probello/Repos/osm-to-bedrock`:

```bash
cargo check --all-targets
```

Expected: compile errors only in call sites that still expect local Overture internals or missing `source` fields in test `OsmPoiNode` constructors.

- [ ] **Step 5: Fix `OsmPoiNode` constructors in `osm-to-bedrock` tests**

For every `OsmPoiNode { ... }` constructor in `/Users/probello/Repos/osm-to-bedrock/src`, add:

```rust
source: par_osm_rust::osm::FeatureSource::Osm,
```

Use `FeatureSource::Overture` only for tests explicitly representing Overture input.

Run:

```bash
rg "OsmPoiNode \{" src
```

Expected after edits: every constructor includes a `source` field.

- [ ] **Step 6: Run tests**

```bash
cargo test overture -- --nocapture
cargo test params -- --nocapture
cargo check --all-targets
```

Expected: tests/check pass.

- [ ] **Step 7: Commit**

```bash
git add src/overture.rs src/params.rs src/lib.rs src/**/*.rs
git commit -m "refactor: use shared overture types"
```

---

## Task 7: Update `osm-to-bedrock` fetch paths to shared source orchestration

**Files:**
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/main.rs`
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/server.rs`

- [ ] **Step 1: Add CLI field for POI source mode**

In `FetchConvertArgs` in `/Users/probello/Repos/osm-to-bedrock/src/main.rs`, add after `overture_priority`:

```rust
/// POI source mode: osm-only, overture-only, both, or overture-preferred
#[arg(long, default_value = "overture-preferred")]
poi_source: String,

/// Overture failure behavior: fallback-to-osm or fail
#[arg(long, default_value = "fallback-to-osm")]
overture_failure: String,
```

- [ ] **Step 2: Add parsers for source mode and failure mode**

In `/Users/probello/Repos/osm-to-bedrock/src/main.rs`, add below `parse_overture_themes()`:

```rust
fn parse_poi_source_mode(s: &str) -> Result<params::PoiSourceMode> {
    match s.to_lowercase().replace('_', "-").as_str() {
        "osm" | "osm-only" => Ok(params::PoiSourceMode::OsmOnly),
        "overture" | "overture-only" => Ok(params::PoiSourceMode::OvertureOnly),
        "both" => Ok(params::PoiSourceMode::Both),
        "overture-preferred" | "preferred" => Ok(params::PoiSourceMode::OverturePreferred),
        _ => bail!("unknown POI source mode '{s}' — expected osm-only, overture-only, both, or overture-preferred"),
    }
}

fn parse_overture_failure_mode(s: &str) -> Result<params::OvertureFailureMode> {
    match s.to_lowercase().replace('_', "-").as_str() {
        "fallback" | "fallback-to-osm" => Ok(params::OvertureFailureMode::FallbackToOsm),
        "fail" | "strict" => Ok(params::OvertureFailureMode::Fail),
        _ => bail!("unknown Overture failure mode '{s}' — expected fallback-to-osm or fail"),
    }
}
```

- [ ] **Step 3: Replace `FetchConvert` manual fetch/merge**

In the `Commands::FetchConvert(args)` arm, replace the block that builds `url`, calls `overpass::fetch_osm_data`, optionally calls `overture::fetch_overture_data`, merges, and clips with:

```rust
let url = args
    .overpass_url
    .as_deref()
    .or(config.overpass_url.as_deref())
    .filter(|s| !s.is_empty())
    .map(ToOwned::to_owned);
let overture_enabled = args.overture || config.overture.unwrap_or(false);
let themes = parse_overture_themes(&args.overture_themes)?;
let priority = parse_overture_priority(&args.overture_priority)?;
let source_options = params::SourceOptions {
    filter: filter.clone(),
    overpass_url: url,
    use_overpass_cache: true,
    overture: params::OvertureParams {
        enabled: overture_enabled,
        themes,
        priority,
        timeout_secs: args.overture_timeout,
    },
    poi_source_mode: parse_poi_source_mode(&args.poi_source)?,
    overture_failure_mode: parse_overture_failure_mode(&args.overture_failure)?,
};
let source_result = par_osm_rust::sources::fetch_map_data(
    bbox,
    &source_options,
    &mut |progress, msg| println!("[{:3.0}%] {msg}", progress * 100.0),
)?;
for warning in &source_result.warnings {
    log::warn!("{warning}");
}
let data = source_result.data;
```

- [ ] **Step 4: Keep `OvertureConvert` Overture-only**

In `Commands::OvertureConvert(args)`, keep the direct shared `overture::fetch_overture_data()` call. It now resolves through the re-export shim and remains a pure Overture command.

- [ ] **Step 5: Update server request types**

In `/Users/probello/Repos/osm-to-bedrock/src/server.rs`, add these fields to `FetchConvertOptions` near existing Overture fields:

```rust
/// POI source mode: osm-only, overture-only, both, or overture-preferred.
#[serde(default)]
poi_source: Option<String>,
/// Overture failure behavior: fallback-to-osm or fail.
#[serde(default)]
overture_failure: Option<String>,
```

- [ ] **Step 6: Add server parser helpers**

In `/Users/probello/Repos/osm-to-bedrock/src/server.rs`, add:

```rust
fn parse_poi_source_mode_for_server(value: Option<&str>) -> Result<crate::params::PoiSourceMode> {
    match value.unwrap_or("overture-preferred").to_lowercase().replace('_', "-").as_str() {
        "osm" | "osm-only" => Ok(crate::params::PoiSourceMode::OsmOnly),
        "overture" | "overture-only" => Ok(crate::params::PoiSourceMode::OvertureOnly),
        "both" => Ok(crate::params::PoiSourceMode::Both),
        "overture-preferred" | "preferred" => Ok(crate::params::PoiSourceMode::OverturePreferred),
        other => anyhow::bail!("unknown POI source mode '{other}'"),
    }
}

fn parse_overture_failure_mode_for_server(value: Option<&str>) -> Result<crate::params::OvertureFailureMode> {
    match value.unwrap_or("fallback-to-osm").to_lowercase().replace('_', "-").as_str() {
        "fallback" | "fallback-to-osm" => Ok(crate::params::OvertureFailureMode::FallbackToOsm),
        "fail" | "strict" => Ok(crate::params::OvertureFailureMode::Fail),
        other => anyhow::bail!("unknown Overture failure mode '{other}'"),
    }
}
```

- [ ] **Step 7: Replace async conversion manual fetch/merge**

In `/Users/probello/Repos/osm-to-bedrock/src/server.rs`, in the blocking conversion job that currently calls `crate::overpass::fetch_osm_data()` and optionally `crate::overture::fetch_overture_data()`, replace that fetch block with:

```rust
let themes: Vec<crate::params::OvertureTheme> = if req_overture_themes.is_empty() {
    crate::params::OvertureTheme::all()
} else {
    req_overture_themes
        .iter()
        .filter_map(|s| crate::params::OvertureTheme::from_str_loose(s))
        .collect()
};
let priority: std::collections::HashMap<crate::params::OvertureTheme, crate::params::ThemePriority> =
    req_overture_priority
        .iter()
        .filter_map(|(k, v)| {
            let theme = crate::params::OvertureTheme::from_str_loose(k)?;
            let prio = match v.as_str() {
                "overture" => crate::params::ThemePriority::Overture,
                "osm" => crate::params::ThemePriority::Osm,
                _ => crate::params::ThemePriority::Both,
            };
            Some((theme, prio))
        })
        .collect();
let poi_source_mode = match parse_poi_source_mode_for_server(options.poi_source.as_deref()) {
    Ok(mode) => mode,
    Err(e) => {
        set_job_error(&jobs, &jid, format!("Invalid POI source mode: {e}"));
        return;
    }
};
let overture_failure_mode = match parse_overture_failure_mode_for_server(options.overture_failure.as_deref()) {
    Ok(mode) => mode,
    Err(e) => {
        set_job_error(&jobs, &jid, format!("Invalid Overture failure mode: {e}"));
        return;
    }
};
let source_options = crate::params::SourceOptions {
    filter: filter.clone(),
    overpass_url: Some(overpass_url.clone()),
    use_overpass_cache: !force_refresh,
    overture: crate::params::OvertureParams {
        enabled: req_overture,
        themes,
        priority,
        timeout_secs: req_overture_timeout,
    },
    poi_source_mode,
    overture_failure_mode,
};
let jobs_for_progress = jobs.clone();
let jid_for_progress = jid.clone();
let data = match par_osm_rust::sources::fetch_map_data(
    bbox,
    &source_options,
    &mut |progress, msg| {
        let mut map = jobs_for_progress.lock().expect("jobs lock poisoned");
        map.insert(
            jid_for_progress.clone(),
            JobState::Running {
                progress: progress * 0.3,
                message: msg.to_string(),
            },
        );
    },
) {
    Ok(result) => result.data,
    Err(e) => {
        set_job_error(&jobs, &jid, format!("Map data fetch failed: {e}"));
        return;
    }
};
```

Keep later conversion code unchanged.

- [ ] **Step 8: Run checks**

```bash
cargo test -- --nocapture
cargo check --all-targets
```

Expected: tests/check pass.

- [ ] **Step 9: Commit**

```bash
git add src/main.rs src/server.rs
git commit -m "refactor: use shared map source orchestration"
```

---

## Task 8: Update `osm-world` prepare API to use shared Overture sources

**Files:**
- Modify: `/Users/probello/Repos/osm-world/src/server.rs`

- [ ] **Step 1: Add prepare API fields and response warnings**

In `/Users/probello/Repos/osm-world/src/server.rs`, extend `PrepareAreaRequest` with:

```rust
#[serde(default)]
pub overture: bool,
#[serde(default)]
pub overture_themes: Vec<String>,
#[serde(default)]
pub poi_source_mode: Option<par_osm_rust::sources::PoiSourceMode>,
#[serde(default)]
pub overture_failure_mode: Option<par_osm_rust::sources::OvertureFailureMode>,
#[serde(default)]
pub overture_timeout: Option<u64>,
```

Extend `PrepareAreaResponse` with:

```rust
pub source_status: String,
pub warnings: Vec<String>,
```

- [ ] **Step 2: Update test request helper**

In `cached_prepare_request()` in `/Users/probello/Repos/osm-world/src/server.rs`, add fields:

```rust
overture: false,
overture_themes: Vec::new(),
poi_source_mode: None,
overture_failure_mode: None,
overture_timeout: None,
```

- [ ] **Step 3: Write failing server fallback test**

Add this test to the server test module:

```rust
#[test]
fn prepare_area_reports_overture_fallback_warning() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
    ]);
    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);
    let mut req = cached_prepare_request(bbox, filter);
    req.overture = true;
    req.poi_source_mode = Some(par_osm_rust::sources::PoiSourceMode::OverturePreferred);
    req.overture_failure_mode = Some(par_osm_rust::sources::OvertureFailureMode::FallbackToOsm);

    let response = prepare_area(req, tmp.path()).unwrap();

    assert_eq!(response.source_status, "overture_fallback_to_osm");
    assert!(response.warnings.iter().any(|warning| warning.contains("Overture")));
    assert!(Path::new(&response.osm_path).exists());
}
```

- [ ] **Step 4: Run test to verify it fails**

Run from `/Users/probello/Repos/osm-world`:

```bash
cargo test server::tests::prepare_area_reports_overture_fallback_warning -- --nocapture
```

Expected: compile failure because response fields and shared source usage are not implemented.

- [ ] **Step 5: Add Overture theme parser helper**

In `/Users/probello/Repos/osm-world/src/server.rs`, add near validation helpers:

```rust
fn parse_overture_themes_for_prepare(values: &[String]) -> PrepareResult<Vec<par_osm_rust::overture::OvertureTheme>> {
    if values.is_empty() {
        return Ok(par_osm_rust::overture::OvertureTheme::all());
    }
    values
        .iter()
        .map(|value| {
            par_osm_rust::overture::OvertureTheme::from_str_loose(value)
                .ok_or_else(|| PrepareAreaError::bad_request(anyhow::anyhow!("unknown Overture theme '{value}'")))
        })
        .collect()
}
```

- [ ] **Step 6: Add prepared cache key helper**

Add near `write_atomic()`:

```rust
fn prepared_cache_key(
    bbox: (f64, f64, f64, f64),
    filter: &par_osm_rust::filter::FeatureFilter,
    overture: bool,
    themes: &[String],
    poi_source_mode: par_osm_rust::sources::PoiSourceMode,
    failure_mode: par_osm_rust::sources::OvertureFailureMode,
) -> String {
    use sha2::{Digest, Sha256};
    let payload = serde_json::json!({
        "schema": 2,
        "bbox": [bbox.0, bbox.1, bbox.2, bbox.3],
        "filter": filter,
        "overture": overture,
        "themes": themes,
        "poi_source_mode": poi_source_mode,
        "failure_mode": failure_mode,
    });
    let hash = Sha256::digest(payload.to_string().as_bytes());
    format!("{hash:x}")
}
```

- [ ] **Step 7: Replace Overpass-only fetch in `prepare_area()`**

In `prepare_area()`, replace the Overpass XML read/fetch block from `let cache_key = par_osm_rust::osm_cache::cache_key(...)` through `let xml = match xml { ... };` with:

```rust
let poi_source_mode = req
    .poi_source_mode
    .unwrap_or(par_osm_rust::sources::PoiSourceMode::OverturePreferred);
let failure_mode = req
    .overture_failure_mode
    .unwrap_or(par_osm_rust::sources::OvertureFailureMode::FallbackToOsm);
let themes = parse_overture_themes_for_prepare(&req.overture_themes)?;
let cache_key = prepared_cache_key(
    bbox,
    &req.filter,
    req.overture,
    &req.overture_themes,
    poi_source_mode,
    failure_mode,
);
let source_options = par_osm_rust::sources::SourceOptions {
    filter: req.filter.clone(),
    overpass_url: req.overpass_url.clone(),
    use_overpass_cache: !req.force_refresh,
    overture: par_osm_rust::overture::OvertureParams {
        enabled: req.overture,
        themes,
        priority: std::collections::HashMap::new(),
        timeout_secs: req.overture_timeout.unwrap_or(120),
    },
    poi_source_mode,
    overture_failure_mode: failure_mode,
};
let source_result = par_osm_rust::sources::fetch_map_data(
    bbox,
    &source_options,
    &mut |progress, msg| log::info!("prepare area source fetch {:3.0}%: {msg}", progress * 100.0),
)
.map_err(|err| PrepareAreaError::upstream("failed to fetch map data", err))?;
let xml = par_osm_rust::osm::write_osm_xml_string(&source_result.data);
let cache_status = if req.force_refresh { "force_refreshed" } else { "prepared" }.to_string();
```

Keep `validate_filter()` and Overpass URL validation before this block.

- [ ] **Step 8: Write response fields**

In the `Ok(PrepareAreaResponse { ... })` initializer, replace the old `cache_status: cache_status.unwrap_or_else(...)` with:

```rust
cache_status,
```

and add:

```rust
source_status: format!("{:?}", source_result.status)
    .chars()
    .flat_map(|ch| {
        if ch.is_uppercase() {
            vec!['_', ch.to_ascii_lowercase()]
        } else {
            vec![ch]
        }
    })
    .collect::<String>()
    .trim_start_matches('_')
    .to_string(),
warnings: source_result.warnings,
```

- [ ] **Step 9: Run focused server test**

```bash
cargo test server::tests::prepare_area_reports_overture_fallback_warning -- --nocapture
```

Expected: test passes.

- [ ] **Step 10: Run existing server tests**

```bash
cargo test server::tests -- --nocapture
```

Expected: all server tests pass. Update assertions that expected `cache_status == "exact_cache_hit"` to accept `"prepared"` only if those tests now exercise the new normalized-prepared path.

- [ ] **Step 11: Commit**

```bash
git add src/server.rs
git commit -m "feat: prepare osm-world areas with shared source policy"
```

---

## Task 9: Wire `osm-world` Overture-prepared POIs into rendering tests

**Files:**
- Modify: `/Users/probello/Repos/osm-world/src/server.rs`
- Modify: `/Users/probello/Repos/osm-world/src/world/loader.rs` only if normalized XML reveals parser/loader gaps.

- [ ] **Step 1: Write prepared XML renderability test**

Add this test to `/Users/probello/Repos/osm-world/src/server.rs` test module:

```rust
#[test]
fn prepare_area_writes_renderable_poi_xml() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
    ]);
    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    let cache_key = par_osm_rust::osm_cache::cache_key((bbox[0], bbox[1], bbox[2], bbox[3]), &filter);
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <bounds minlat="38.0" minlon="-121.0" maxlat="38.001" maxlon="-120.999"/>
  <node id="1" lat="38.0005" lon="-120.9995">
    <tag k="amenity" v="restaurant"/>
    <tag k="name" v="Test Cafe"/>
  </node>
</osm>"#;
    par_osm_rust::osm_cache::write(&cache_key, (bbox[0], bbox[1], bbox[2], bbox[3]), &filter, xml).unwrap();

    let response = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();
    let source = crate::world::loader::load_world_source(Path::new(&response.osm_path), None).unwrap();

    assert_eq!(source.point_features.len(), 1);
    assert_eq!(
        source.point_features[0].tags.get("name").map(String::as_str),
        Some("Test Cafe")
    );
}
```

- [ ] **Step 2: Run test**

```bash
cargo test server::tests::prepare_area_writes_renderable_poi_xml -- --nocapture
```

Expected: pass. If it fails because normalized XML has missing bounds, fix `write_osm_xml_string()` in `par-osm-rust` and rerun the shared tests before rerunning this test.

- [ ] **Step 3: Run app/world tests**

```bash
cargo test world::loader::tests::load_world_source_classifies_poi_nodes -- --nocapture
cargo test server::tests -- --nocapture
```

Expected: tests pass.

- [ ] **Step 4: Run graph update**

```bash
graphify update .
```

Expected: graph update completes successfully.

- [ ] **Step 5: Commit**

```bash
git add src/server.rs src/world/loader.rs graphify-out
git commit -m "test: cover renderable prepared poi data"
```

---

## Task 10: Final verification across all repositories

**Files:**
- No source edits expected.

- [ ] **Step 1: Verify shared crate**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cargo test
```

Expected: all tests pass.

- [ ] **Step 2: Verify Bedrock converter**

Run:

```bash
cd /Users/probello/Repos/osm-to-bedrock
cargo test
cargo check --all-targets
```

Expected: all tests and check pass.

- [ ] **Step 3: Verify osm-world**

Run:

```bash
cd /Users/probello/Repos/osm-world
make checkall
graphify update .
```

Expected: `make checkall` passes and graphify completes.

- [ ] **Step 4: Inspect repo status**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust && git status --short
cd /Users/probello/Repos/osm-to-bedrock && git status --short
cd /Users/probello/Repos/osm-world && git status --short
```

Expected: no uncommitted source changes except intentional generated graph files that were already committed in Task 9.

- [ ] **Step 5: Save vault note if implementation reveals a reusable pattern**

If the shared source orchestrator or XML writer needed non-obvious fixes, create a note under `/Users/probello/ParsidionVault/Patterns/` or `/Users/probello/ParsidionVault/Debugging/` and rebuild the index:

```bash
uv run --no-project ~/.claude/skills/parsidion/scripts/update_index.py
```

Expected: index rebuild succeeds.

---

## Self-Review

- Spec coverage: The plan moves Overture into `par-osm-rust`, adds POI source modes and fallback behavior, updates both consuming projects, keeps normalized renderer input, and includes verification for all repos.
- Placeholder scan: No red-flag placeholder markers, unfinished tasks, or unspecified tests are included.
- Type consistency: Shared types are defined in `par_osm_rust::overture` and `par_osm_rust::sources`; `osm-to-bedrock::params` re-exports them for compatibility; `osm-world` references shared types directly.
