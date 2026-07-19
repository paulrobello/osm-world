//! OSM PBF file parser.
//!
//! Reads nodes, ways, and their tags from a `.osm.pbf` file.

use anyhow::{Context, Result};
use osmpbf::{Element, ElementReader};
use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::Event;
use quick_xml::events::attributes::Attribute;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A geographic point from the OSM dataset.
#[derive(Debug, Clone, Copy)]
pub struct OsmNode {
    pub lat: f64,
    pub lon: f64,
}

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

/// An OSM way: an ordered sequence of node references with tags.
#[derive(Debug, Clone)]
pub struct OsmWay {
    pub tags: HashMap<String, String>,
    pub node_refs: Vec<i64>,
}

/// A member of an OSM relation with its role.
#[derive(Debug, Clone)]
pub struct RelationMember {
    /// Way ID referenced by this member.
    pub way_id: i64,
    /// Role string (e.g. "outer", "inner").
    pub role: String,
}

/// An OSM relation: a collection of ways with roles and tags.
#[derive(Debug, Clone)]
pub struct OsmRelation {
    pub tags: HashMap<String, String>,
    pub members: Vec<RelationMember>,
}

/// Parsed OSM dataset.
pub struct OsmData {
    pub nodes: HashMap<i64, OsmNode>,
    pub ways: Vec<OsmWay>,
    /// Way lookup by ID for relation member resolution.
    ///
    /// Maps each OSM way ID to its position in the `ways` vector. Storing an
    /// index avoids duplicating `OsmWay` values while still allowing relation
    /// members to find their referenced ways efficiently.
    pub ways_by_id: HashMap<i64, usize>,
    /// Multipolygon relations.
    pub relations: Vec<OsmRelation>,
    /// Bounding box: (min_lat, min_lon, max_lat, max_lon)
    pub bounds: Option<(f64, f64, f64, f64)>,
    /// Standalone nodes with POI tags (amenity, shop, tourism, leisure, historic).
    pub poi_nodes: Vec<OsmPoiNode>,
    /// Standalone nodes with address tags (addr:housenumber).
    /// These are typically entrance/door nodes placed on building outlines in OSM.
    pub addr_nodes: Vec<OsmPoiNode>,
    /// Individual tree positions (from OSM `natural=tree` or Overture `land/tree`).
    pub tree_nodes: Vec<OsmNode>,
}

impl OsmData {
    /// Merge another `OsmData` into this one, combining nodes, ways, and bounds.
    pub fn merge(&mut self, other: OsmData) {
        self.nodes.extend(other.nodes);
        let offset = self.ways.len();
        self.ways.extend(other.ways);
        // Adjust indices from `other` to account for the ways already in `self`.
        self.ways_by_id.extend(
            other
                .ways_by_id
                .into_iter()
                .map(|(id, idx)| (id, idx + offset)),
        );
        self.relations.extend(other.relations);
        self.poi_nodes.extend(other.poi_nodes);
        self.addr_nodes.extend(other.addr_nodes);
        self.tree_nodes.extend(other.tree_nodes);
        match (self.bounds, other.bounds) {
            (Some((a0, a1, a2, a3)), Some((b0, b1, b2, b3))) => {
                self.bounds = Some((a0.min(b0), a1.min(b1), a2.max(b2), a3.max(b3)));
            }
            (None, b) => self.bounds = b,
            _ => {}
        }
    }

    /// Clip data to a bounding box, keeping only features that touch the bbox.
    ///
    /// `bbox` is `(min_lat, min_lon, max_lat, max_lon)`.
    /// Ways are kept if at least one node falls inside the bbox.
    /// POI and address nodes are kept only if inside the bbox.
    /// Unreferenced nodes are pruned.
    pub fn clip_to_bbox(&mut self, bbox: (f64, f64, f64, f64)) {
        let (min_lat, min_lon, max_lat, max_lon) = bbox;

        let in_bbox = |lat: f64, lon: f64| -> bool {
            lat >= min_lat && lat <= max_lat && lon >= min_lon && lon <= max_lon
        };

        // Filter ways: keep if any node is inside the bbox
        let mut keep_node_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
        let mut new_ways = Vec::new();
        let mut new_ways_by_id = HashMap::new();
        for (way_id, &old_idx) in &self.ways_by_id {
            let way = &self.ways[old_idx];
            let touches_bbox = way
                .node_refs
                .iter()
                .any(|id| self.nodes.get(id).is_some_and(|n| in_bbox(n.lat, n.lon)));
            if touches_bbox {
                for id in &way.node_refs {
                    keep_node_ids.insert(*id);
                }
                let new_idx = new_ways.len();
                new_ways.push(way.clone());
                new_ways_by_id.insert(*way_id, new_idx);
            }
        }
        // Also keep ways not in ways_by_id (shouldn't happen, but be safe)
        // by scanning ways without a ways_by_id entry
        let indexed: std::collections::HashSet<usize> = self.ways_by_id.values().copied().collect();
        for (i, way) in self.ways.iter().enumerate() {
            if indexed.contains(&i) {
                continue;
            }
            let touches_bbox = way
                .node_refs
                .iter()
                .any(|id| self.nodes.get(id).is_some_and(|n| in_bbox(n.lat, n.lon)));
            if touches_bbox {
                for id in &way.node_refs {
                    keep_node_ids.insert(*id);
                }
                new_ways.push(way.clone());
            }
        }

        self.ways = new_ways;
        self.ways_by_id = new_ways_by_id;

        // Prune nodes to only those referenced by kept ways
        self.nodes.retain(|id, _| keep_node_ids.contains(id));

        // Filter POI and address nodes
        self.poi_nodes.retain(|p| in_bbox(p.lat, p.lon));
        self.addr_nodes.retain(|p| in_bbox(p.lat, p.lon));
        self.tree_nodes.retain(|n| in_bbox(n.lat, n.lon));

        // Filter relations: keep if any member way was kept
        self.relations.retain(|rel| {
            rel.members
                .iter()
                .any(|m| self.ways_by_id.contains_key(&m.way_id))
        });

        // Update bounds to the requested bbox
        self.bounds = Some(bbox);
    }
}

fn xml_attr_value(attr: &Attribute<'_>) -> String {
    attr.normalized_value(XmlVersion::Implicit1_0)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| {
            std::str::from_utf8(attr.value.as_ref())
                .unwrap_or("")
                .to_string()
        })
}

fn xml_attr_parse<T: std::str::FromStr>(attr: &Attribute<'_>) -> Option<T> {
    xml_attr_value(attr).parse().ok()
}

/// Parse a `.osm.pbf` file and return all nodes and ways.
pub fn parse_pbf(path: &Path) -> Result<OsmData> {
    let reader =
        ElementReader::from_path(path).with_context(|| format!("opening {}", path.display()))?;

    let mut nodes: HashMap<i64, OsmNode> = HashMap::new();
    let mut ways: Vec<OsmWay> = Vec::new();
    let mut ways_by_id: HashMap<i64, usize> = HashMap::new();
    let mut relations: Vec<OsmRelation> = Vec::new();
    let mut poi_nodes: Vec<OsmPoiNode> = Vec::new();
    let mut addr_nodes: Vec<OsmPoiNode> = Vec::new();
    let mut tree_nodes: Vec<OsmNode> = Vec::new();
    let mut min_lat = f64::MAX;
    let mut min_lon = f64::MAX;
    let mut max_lat = f64::MIN;
    let mut max_lon = f64::MIN;

    reader
        .for_each(|element| match element {
            Element::Node(n) => {
                let lat = n.lat();
                let lon = n.lon();
                min_lat = min_lat.min(lat);
                min_lon = min_lon.min(lon);
                max_lat = max_lat.max(lat);
                max_lon = max_lon.max(lon);
                nodes.insert(n.id(), OsmNode { lat, lon });
                let tags: HashMap<String, String> = n
                    .tags()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                if tags.keys().any(|k| {
                    matches!(
                        k.as_str(),
                        "amenity" | "shop" | "tourism" | "leisure" | "historic"
                    )
                }) {
                    poi_nodes.push(OsmPoiNode {
                        lat,
                        lon,
                        tags: tags.clone(),
                        source: FeatureSource::Osm,
                    });
                }
                if tags.contains_key("addr:housenumber") {
                    addr_nodes.push(OsmPoiNode {
                        lat,
                        lon,
                        tags: tags.clone(),
                        source: FeatureSource::Osm,
                    });
                }
                if tags.get("natural").map(|s| s.as_str()) == Some("tree") {
                    tree_nodes.push(OsmNode { lat, lon });
                }
            }
            Element::DenseNode(n) => {
                let lat = n.lat();
                let lon = n.lon();
                min_lat = min_lat.min(lat);
                min_lon = min_lon.min(lon);
                max_lat = max_lat.max(lat);
                max_lon = max_lon.max(lon);
                nodes.insert(n.id(), OsmNode { lat, lon });
                let tags: HashMap<String, String> = n
                    .tags()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                if tags.keys().any(|k| {
                    matches!(
                        k.as_str(),
                        "amenity" | "shop" | "tourism" | "leisure" | "historic"
                    )
                }) {
                    poi_nodes.push(OsmPoiNode {
                        lat,
                        lon,
                        tags: tags.clone(),
                        source: FeatureSource::Osm,
                    });
                }
                if tags.contains_key("addr:housenumber") {
                    addr_nodes.push(OsmPoiNode {
                        lat,
                        lon,
                        tags: tags.clone(),
                        source: FeatureSource::Osm,
                    });
                }
                if tags.get("natural").map(|s| s.as_str()) == Some("tree") {
                    tree_nodes.push(OsmNode { lat, lon });
                }
            }
            Element::Way(w) => {
                let tags: HashMap<String, String> = w
                    .tags()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                let node_refs: Vec<i64> = w.refs().collect();
                let way = OsmWay {
                    tags: tags.clone(),
                    node_refs: node_refs.clone(),
                };
                let idx = ways.len();
                ways.push(way);
                ways_by_id.insert(w.id(), idx);
            }
            Element::Relation(r) => {
                let rel_type = r.tags().find(|(k, _)| *k == "type").map(|(_, v)| v);
                if rel_type == Some("multipolygon") {
                    let tags: HashMap<String, String> = r
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    let members: Vec<RelationMember> = r
                        .members()
                        .filter_map(|m| {
                            if matches!(m.member_type, osmpbf::elements::RelMemberType::Way) {
                                Some(RelationMember {
                                    way_id: m.member_id,
                                    role: m.role().unwrap_or_default().to_string(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect();
                    if !members.is_empty() {
                        relations.push(OsmRelation { tags, members });
                    }
                }
            }
        })
        .context("reading PBF elements")?;

    let bounds = if min_lat < f64::MAX {
        Some((min_lat, min_lon, max_lat, max_lon))
    } else {
        None
    };

    log::info!(
        "Parsed {} nodes, {} ways, {} relations, {} POI nodes, {} address nodes, {} tree nodes",
        nodes.len(),
        ways.len(),
        relations.len(),
        poi_nodes.len(),
        addr_nodes.len(),
        tree_nodes.len()
    );

    Ok(OsmData {
        nodes,
        ways,
        ways_by_id,
        relations,
        bounds,
        poi_nodes,
        addr_nodes,
        tree_nodes,
    })
}

/// Parse an OSM XML string into `OsmData`.
///
/// Uses a two-pass approach: nodes are collected in the first pass so that
/// way node references resolve correctly regardless of element ordering
/// (Overpass does not guarantee nodes appear before ways).
pub fn parse_osm_xml_str(xml: &str) -> Result<OsmData> {
    // ── Pass 1: collect all nodes (and POI-tagged nodes) ─────────────────
    let mut nodes: HashMap<i64, OsmNode> = HashMap::new();
    let mut poi_nodes: Vec<OsmPoiNode> = Vec::new();
    let mut addr_nodes: Vec<OsmPoiNode> = Vec::new();
    let mut tree_nodes: Vec<OsmNode> = Vec::new();
    let mut min_lat = f64::MAX;
    let mut min_lon = f64::MAX;
    let mut max_lat = f64::MIN;
    let mut max_lon = f64::MIN;
    let mut explicit_bounds: Option<(f64, f64, f64, f64)> = None;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // State for nodes that have child <tag> elements
    let mut in_node = false;
    let mut cur_lat = 0.0f64;
    let mut cur_lon = 0.0f64;
    let mut cur_node_tags: HashMap<String, String> = HashMap::new();

    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"bounds" => {
                let mut minlat: Option<f64> = None;
                let mut minlon: Option<f64> = None;
                let mut maxlat: Option<f64> = None;
                let mut maxlon: Option<f64> = None;
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"minlat" => minlat = xml_attr_parse(&attr),
                        b"minlon" => minlon = xml_attr_parse(&attr),
                        b"maxlat" => maxlat = xml_attr_parse(&attr),
                        b"maxlon" => maxlon = xml_attr_parse(&attr),
                        _ => {}
                    }
                }
                if let (Some(minlat), Some(minlon), Some(maxlat), Some(maxlon)) =
                    (minlat, minlon, maxlat, maxlon)
                {
                    explicit_bounds = Some((minlat, minlon, maxlat, maxlon));
                }
            }
            // Self-closing node (no child tags)
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"node" => {
                let mut id: Option<i64> = None;
                let mut lat: Option<f64> = None;
                let mut lon: Option<f64> = None;
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"id" => id = xml_attr_parse(&attr),
                        b"lat" => lat = xml_attr_parse(&attr),
                        b"lon" => lon = xml_attr_parse(&attr),
                        _ => {}
                    }
                }
                if let (Some(id), Some(lat), Some(lon)) = (id, lat, lon) {
                    min_lat = min_lat.min(lat);
                    min_lon = min_lon.min(lon);
                    max_lat = max_lat.max(lat);
                    max_lon = max_lon.max(lon);
                    nodes.insert(id, OsmNode { lat, lon });
                }
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"bounds" => {
                let mut minlat: Option<f64> = None;
                let mut minlon: Option<f64> = None;
                let mut maxlat: Option<f64> = None;
                let mut maxlon: Option<f64> = None;
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"minlat" => minlat = xml_attr_parse(&attr),
                        b"minlon" => minlon = xml_attr_parse(&attr),
                        b"maxlat" => maxlat = xml_attr_parse(&attr),
                        b"maxlon" => maxlon = xml_attr_parse(&attr),
                        _ => {}
                    }
                }
                if let (Some(minlat), Some(minlon), Some(maxlat), Some(maxlon)) =
                    (minlat, minlon, maxlat, maxlon)
                {
                    explicit_bounds = Some((minlat, minlon, maxlat, maxlon));
                }
            }
            // Opening <node> tag with child <tag> elements
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"node" => {
                let mut id: Option<i64> = None;
                let mut lat: Option<f64> = None;
                let mut lon: Option<f64> = None;
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"id" => id = xml_attr_parse(&attr),
                        b"lat" => lat = xml_attr_parse(&attr),
                        b"lon" => lon = xml_attr_parse(&attr),
                        _ => {}
                    }
                }
                if let (Some(id), Some(lat), Some(lon)) = (id, lat, lon) {
                    min_lat = min_lat.min(lat);
                    min_lon = min_lon.min(lon);
                    max_lat = max_lat.max(lat);
                    max_lon = max_lon.max(lon);
                    nodes.insert(id, OsmNode { lat, lon });
                    in_node = true;
                    cur_lat = lat;
                    cur_lon = lon;
                    cur_node_tags.clear();
                }
            }
            // <tag> child inside a node
            Ok(Event::Empty(ref e)) if in_node && e.name().as_ref() == b"tag" => {
                let mut k = String::new();
                let mut v = String::new();
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"k" => k = xml_attr_value(&attr),
                        b"v" => v = xml_attr_value(&attr),
                        _ => {}
                    }
                }
                if !k.is_empty() {
                    cur_node_tags.insert(k, v);
                }
            }
            // Closing </node>
            Ok(Event::End(ref e)) if e.name().as_ref() == b"node" && in_node => {
                in_node = false;
                if cur_node_tags.keys().any(|k| {
                    matches!(
                        k.as_str(),
                        "amenity" | "shop" | "tourism" | "leisure" | "historic"
                    )
                }) {
                    poi_nodes.push(OsmPoiNode {
                        lat: cur_lat,
                        lon: cur_lon,
                        tags: cur_node_tags.clone(),
                        source: FeatureSource::Osm,
                    });
                }
                if cur_node_tags.contains_key("addr:housenumber") {
                    addr_nodes.push(OsmPoiNode {
                        lat: cur_lat,
                        lon: cur_lon,
                        tags: cur_node_tags.clone(),
                        source: FeatureSource::Osm,
                    });
                }
                if cur_node_tags.get("natural").map(|s| s.as_str()) == Some("tree") {
                    tree_nodes.push(OsmNode {
                        lat: cur_lat,
                        lon: cur_lon,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "XML parse error at position {}: {e}",
                    reader.buffer_position()
                ));
            }
            _ => {}
        }
    }

    // ── Pass 2: collect ways and relations ───────────────────────────────
    let mut ways: Vec<OsmWay> = Vec::new();
    let mut ways_by_id: HashMap<i64, usize> = HashMap::new();
    let mut relations: Vec<OsmRelation> = Vec::new();

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut in_way = false;
    let mut in_relation = false;
    let mut current_way_id: i64 = 0;
    let mut current_tags: HashMap<String, String> = HashMap::new();
    let mut current_node_refs: Vec<i64> = Vec::new();
    let mut current_members: Vec<RelationMember> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"way" => {
                    in_way = true;
                    current_way_id = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"id")
                        .and_then(|a| xml_attr_parse(&a))
                        .unwrap_or(0);
                    current_tags.clear();
                    current_node_refs.clear();
                }
                b"relation" => {
                    in_relation = true;
                    current_tags.clear();
                    current_members.clear();
                }
                _ => {}
            },
            Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                b"nd" if in_way => {
                    if let Some(r) = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"ref")
                        .and_then(|a| xml_attr_parse::<i64>(&a))
                    {
                        current_node_refs.push(r);
                    }
                }
                b"tag" if in_way || in_relation => {
                    let mut k = String::new();
                    let mut v = String::new();
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"k" => k = xml_attr_value(&attr),
                            b"v" => v = xml_attr_value(&attr),
                            _ => {}
                        }
                    }
                    if !k.is_empty() {
                        current_tags.insert(k, v);
                    }
                }
                b"member" if in_relation => {
                    let mut mtype = String::new();
                    let mut mref: i64 = 0;
                    let mut mrole = String::new();
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"type" => mtype = xml_attr_value(&attr),
                            b"ref" => mref = xml_attr_parse(&attr).unwrap_or(0),
                            b"role" => mrole = xml_attr_value(&attr),
                            _ => {}
                        }
                    }
                    if mtype == "way" && mref != 0 {
                        current_members.push(RelationMember {
                            way_id: mref,
                            role: mrole,
                        });
                    }
                }
                _ => {}
            },
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"way" if in_way => {
                    in_way = false;
                    let way = OsmWay {
                        tags: current_tags.clone(),
                        node_refs: current_node_refs.clone(),
                    };
                    let idx = ways.len();
                    ways.push(way);
                    ways_by_id.insert(current_way_id, idx);
                }
                b"relation" if in_relation => {
                    in_relation = false;
                    let rel_type = current_tags.get("type").map(|s| s.as_str());
                    if rel_type == Some("multipolygon") && !current_members.is_empty() {
                        relations.push(OsmRelation {
                            tags: current_tags.clone(),
                            members: current_members.clone(),
                        });
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {e}")),
            _ => {}
        }
    }

    let bounds = explicit_bounds
        .or_else(|| (min_lat < f64::MAX).then_some((min_lat, min_lon, max_lat, max_lon)));

    log::info!(
        "Parsed {} nodes, {} ways, {} relations, {} POI nodes, {} address nodes, {} tree nodes (XML)",
        nodes.len(),
        ways.len(),
        relations.len(),
        poi_nodes.len(),
        addr_nodes.len(),
        tree_nodes.len()
    );

    Ok(OsmData {
        nodes,
        ways,
        ways_by_id,
        relations,
        bounds,
        poi_nodes,
        addr_nodes,
        tree_nodes,
    })
}

/// Parse a `.osm` XML file into `OsmData`.
pub fn parse_osm_xml(path: &Path) -> Result<OsmData> {
    let xml =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_osm_xml_str(&xml)
}

/// Detect file format by extension and dispatch to the correct parser.
/// Supports `.osm.pbf` / `.pbf` (PBF format) and `.osm` (XML format).
pub fn parse_osm_file(path: &Path) -> Result<OsmData> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "pbf" => parse_pbf(path),
        "osm" => parse_osm_xml(path),
        other => Err(anyhow::anyhow!(
            "unsupported file format '.{other}'; expected .osm.pbf or .osm"
        )),
    }
}

fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn write_tags(xml: &mut String, tags: &HashMap<String, String>) {
    let mut entries: Vec<_> = tags.iter().collect();
    entries.sort_by_key(|(key, _)| *key);
    for (key, value) in entries {
        xml.push_str("    <tag k=\"");
        xml.push_str(&escape_xml_attr(key));
        xml.push_str("\" v=\"");
        xml.push_str(&escape_xml_attr(value));
        xml.push_str("\"/>\n");
    }
}

fn next_synthetic_node_id(next_id: &mut i64, occupied: &mut HashSet<i64>) -> i64 {
    while occupied.contains(next_id) {
        *next_id -= 1;
    }
    let id = *next_id;
    occupied.insert(id);
    *next_id -= 1;
    id
}

/// Serialize normalized [`OsmData`] into simple OSM XML that this crate and
/// `osm-world` can parse again.
pub fn write_osm_xml_string(data: &OsmData) -> String {
    let mut xml =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<osm version=\"0.6\">\n");

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

    let mut occupied_node_ids: HashSet<i64> = data.nodes.keys().copied().collect();
    let mut synthetic_id = -9_000_000_000_i64;
    for poi in &data.poi_nodes {
        let node_id = next_synthetic_node_id(&mut synthetic_id, &mut occupied_node_ids);
        xml.push_str(&format!(
            "  <node id=\"{}\" lat=\"{}\" lon=\"{}\">\n",
            node_id, poi.lat, poi.lon
        ));
        write_tags(&mut xml, &poi.tags);
        xml.push_str("  </node>\n");
    }

    for addr in &data.addr_nodes {
        let node_id = next_synthetic_node_id(&mut synthetic_id, &mut occupied_node_ids);
        xml.push_str(&format!(
            "  <node id=\"{}\" lat=\"{}\" lon=\"{}\">\n",
            node_id, addr.lat, addr.lon
        ));
        write_tags(&mut xml, &addr.tags);
        xml.push_str("  </node>\n");
    }

    for tree in &data.tree_nodes {
        let node_id = next_synthetic_node_id(&mut synthetic_id, &mut occupied_node_ids);
        xml.push_str(&format!(
            "  <node id=\"{}\" lat=\"{}\" lon=\"{}\">\n    <tag k=\"natural\" v=\"tree\"/>\n  </node>\n",
            node_id, tree.lat, tree.lon
        ));
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

    for (idx, relation) in data.relations.iter().enumerate() {
        let relation_id = -7_000_000_000_i64 - idx as i64;
        xml.push_str(&format!("  <relation id=\"{}\">\n", relation_id));
        for member in &relation.members {
            xml.push_str(&format!(
                "    <member type=\"way\" ref=\"{}\" role=\"{}\"/>\n",
                member.way_id,
                escape_xml_attr(&member.role)
            ));
        }
        write_tags(&mut xml, &relation.tags);
        xml.push_str("  </relation>\n");
    }

    xml.push_str("</osm>\n");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_OSM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="51.5" lon="-0.10"/>
  <node id="2" lat="51.5" lon="-0.09"/>
  <node id="3" lat="51.51" lon="-0.09"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <tag k="highway" v="residential"/>
    <tag k="name" v="Test Street"/>
  </way>
</osm>"#;

    #[test]
    fn parse_xml_nodes() {
        let data = parse_osm_xml_str(MINIMAL_OSM).unwrap();
        assert_eq!(data.nodes.len(), 3);
        let n = data.nodes.get(&1).unwrap();
        assert!((n.lat - 51.5).abs() < 0.0001);
        assert!((n.lon - -0.10).abs() < 0.0001);
    }

    #[test]
    fn parse_xml_ways() {
        let data = parse_osm_xml_str(MINIMAL_OSM).unwrap();
        assert_eq!(data.ways.len(), 1);
        assert_eq!(data.ways[0].tags["highway"], "residential");
        assert_eq!(data.ways[0].tags["name"], "Test Street");
        assert_eq!(data.ways[0].node_refs, vec![1, 2, 3]);
    }

    #[test]
    fn parse_xml_bounds_computed_from_nodes() {
        let data = parse_osm_xml_str(MINIMAL_OSM).unwrap();
        let (min_lat, min_lon, max_lat, max_lon) = data.bounds.unwrap();
        assert!((min_lat - 51.5).abs() < 0.0001);
        assert!((max_lat - 51.51).abs() < 0.0001);
        assert!((min_lon - -0.10).abs() < 0.0001);
        assert!((max_lon - -0.09).abs() < 0.0001);
    }

    #[test]
    fn parse_xml_preserves_explicit_bounds() {
        let xml = r#"<?xml version="1.0"?>
<osm version="0.6">
  <bounds minlat="10.0" minlon="20.0" maxlat="30.0" maxlon="40.0"/>
  <node id="1" lat="11.0" lon="21.0"/>
</osm>"#;

        let data = parse_osm_xml_str(xml).unwrap();

        assert_eq!(data.bounds, Some((10.0, 20.0, 30.0, 40.0)));
    }

    #[test]
    fn parse_xml_nodes_after_ways() {
        // Overpass does not guarantee node-before-way ordering
        let xml = r#"<?xml version="1.0"?>
<osm version="0.6">
  <way id="1">
    <nd ref="10"/>
    <nd ref="11"/>
    <tag k="highway" v="primary"/>
  </way>
  <node id="10" lat="1.0" lon="1.0"/>
  <node id="11" lat="1.1" lon="1.1"/>
</osm>"#;
        let data = parse_osm_xml_str(xml).unwrap();
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.ways.len(), 1);
        assert_eq!(data.ways[0].node_refs, vec![10, 11]);
    }

    #[test]
    fn parse_xml_multipolygon_relation() {
        let xml = r#"<?xml version="1.0"?>
<osm version="0.6">
  <node id="1" lat="0.0" lon="0.0"/>
  <way id="100">
    <nd ref="1"/>
    <tag k="landuse" v="park"/>
  </way>
  <relation id="200">
    <member type="way" ref="100" role="outer"/>
    <tag k="type" v="multipolygon"/>
    <tag k="landuse" v="park"/>
  </relation>
</osm>"#;
        let data = parse_osm_xml_str(xml).unwrap();
        assert_eq!(data.relations.len(), 1);
        assert_eq!(data.relations[0].members[0].way_id, 100);
        assert_eq!(data.relations[0].members[0].role, "outer");
        assert_eq!(data.relations[0].tags["landuse"], "park");
    }

    #[test]
    fn parse_xml_unescapes_node_way_and_relation_tag_attributes() {
        let xml = r#"<?xml version="1.0"?>
<osm version="0.6">
  <node id="1" lat="0.0" lon="0.0">
    <tag k="amenity" v="cafe"/>
    <tag k="name" v="A&amp;B"/>
    <tag k="brand&amp;operator" v="C&amp;D"/>
  </node>
  <way id="100">
    <nd ref="1"/>
    <tag k="highway" v="residential"/>
    <tag k="name&amp;operator" v="A&amp;B Road"/>
  </way>
  <relation id="200">
    <member type="way" ref="100" role="outer&amp;ring"/>
    <tag k="type" v="multipolygon"/>
    <tag k="landuse&amp;name" v="A&amp;B Park"/>
  </relation>
</osm>"#;

        let data = parse_osm_xml_str(xml).unwrap();

        assert_eq!(data.poi_nodes[0].tags["name"], "A&B");
        assert_eq!(data.poi_nodes[0].tags["brand&operator"], "C&D");
        assert_eq!(data.ways[0].tags["name&operator"], "A&B Road");
        assert_eq!(data.relations[0].members[0].role, "outer&ring");
        assert_eq!(data.relations[0].tags["landuse&name"], "A&B Park");
    }

    #[test]
    fn parse_xml_poi_nodes_collected() {
        let xml = r#"<?xml version="1.0"?>
<osm version="0.6">
  <node id="1" lat="51.5" lon="-0.10"/>
  <node id="2" lat="51.51" lon="-0.11">
    <tag k="amenity" v="restaurant"/>
    <tag k="name" v="The Pub"/>
  </node>
  <node id="3" lat="51.52" lon="-0.12">
    <tag k="shop" v="supermarket"/>
  </node>
</osm>"#;
        let data = parse_osm_xml_str(xml).unwrap();
        assert_eq!(data.nodes.len(), 3);
        assert_eq!(data.poi_nodes.len(), 2);
        assert_eq!(data.poi_nodes[0].tags["amenity"], "restaurant");
        assert_eq!(data.poi_nodes[0].tags["name"], "The Pub");
        assert_eq!(data.poi_nodes[1].tags["shop"], "supermarket");
    }

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

    #[test]
    fn parse_xml_non_poi_nodes_not_collected() {
        let xml = r#"<?xml version="1.0"?>
<osm version="0.6">
  <node id="1" lat="51.5" lon="-0.10"/>
  <node id="2" lat="51.51" lon="-0.11">
    <tag k="natural" v="tree"/>
  </node>
</osm>"#;
        let data = parse_osm_xml_str(xml).unwrap();
        assert_eq!(data.poi_nodes.len(), 0);
    }

    #[test]
    fn parse_osm_file_detects_format() {
        use std::io::Write;
        let mut tmp = tempfile::Builder::new().suffix(".osm").tempfile().unwrap();
        tmp.write_all(MINIMAL_OSM.as_bytes()).unwrap();
        let (_, path) = tmp.into_parts();
        let data = parse_osm_file(&path).unwrap();
        assert_eq!(data.nodes.len(), 3);
    }

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

        assert!(
            xml.contains("<bounds minlat=\"51.5\" minlon=\"-0.1\" maxlat=\"51.6\" maxlon=\"-0\"/>")
        );
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
        assert_eq!(
            parsed.poi_nodes[0].tags.get("shop").map(String::as_str),
            Some("bakery")
        );
    }

    #[test]
    fn write_osm_xml_string_round_trips_relations_with_tags_and_members() {
        let data = OsmData {
            nodes: HashMap::from([
                (1, OsmNode { lat: 0.0, lon: 0.0 }),
                (2, OsmNode { lat: 1.0, lon: 1.0 }),
            ]),
            ways: vec![
                OsmWay {
                    tags: HashMap::from([("landuse".to_string(), "park".to_string())]),
                    node_refs: vec![1, 2],
                },
                OsmWay {
                    tags: HashMap::from([("natural".to_string(), "water".to_string())]),
                    node_refs: vec![2, 1],
                },
            ],
            ways_by_id: HashMap::from([(100, 0), (101, 1)]),
            relations: vec![OsmRelation {
                tags: HashMap::from([
                    ("type".to_string(), "multipolygon".to_string()),
                    ("name".to_string(), "A&B Park".to_string()),
                ]),
                members: vec![
                    RelationMember {
                        way_id: 100,
                        role: "outer".to_string(),
                    },
                    RelationMember {
                        way_id: 101,
                        role: "inner".to_string(),
                    },
                ],
            }],
            bounds: None,
            poi_nodes: Vec::new(),
            addr_nodes: Vec::new(),
            tree_nodes: Vec::new(),
        };

        let xml = write_osm_xml_string(&data);
        assert!(xml.contains("<relation id=\"-7000000000\">"));

        let parsed = parse_osm_xml_str(&xml).unwrap();

        assert_eq!(parsed.relations.len(), 1);
        assert_eq!(parsed.relations[0].tags["name"], "A&B Park");
        assert_eq!(parsed.relations[0].tags["type"], "multipolygon");
        assert_eq!(parsed.relations[0].members.len(), 2);
        assert_eq!(parsed.relations[0].members[0].way_id, 100);
        assert_eq!(parsed.relations[0].members[0].role, "outer");
        assert_eq!(parsed.relations[0].members[1].way_id, 101);
        assert_eq!(parsed.relations[0].members[1].role, "inner");
    }

    #[test]
    fn write_osm_xml_string_allocates_synthetic_node_ids_without_collisions() {
        let data = OsmData {
            nodes: HashMap::from([(-9_000_000_000, OsmNode { lat: 0.0, lon: 0.0 })]),
            ways: Vec::new(),
            ways_by_id: HashMap::new(),
            relations: Vec::new(),
            bounds: None,
            poi_nodes: vec![OsmPoiNode {
                lat: 1.0,
                lon: 1.0,
                tags: HashMap::from([("amenity".to_string(), "cafe".to_string())]),
                source: FeatureSource::Overture,
            }],
            addr_nodes: Vec::new(),
            tree_nodes: Vec::new(),
        };

        let first_xml = write_osm_xml_string(&data);
        assert!(first_xml.contains("<node id=\"-9000000001\""));

        let parsed = parse_osm_xml_str(&first_xml).unwrap();
        let second_xml = write_osm_xml_string(&parsed);

        let node_ids: Vec<i64> = second_xml
            .lines()
            .filter_map(|line| line.trim_start().strip_prefix("<node id=\""))
            .filter_map(|rest| rest.split_once('"'))
            .map(|(id, _)| id.parse::<i64>().unwrap())
            .collect();
        let unique_node_ids: std::collections::HashSet<_> = node_ids.iter().copied().collect();

        assert_eq!(node_ids.len(), unique_node_ids.len());
        assert!(unique_node_ids.contains(&-9_000_000_002));
    }
}
