//! Overture Maps integration via the `overturemaps` Python CLI.
//!
//! This module provides helpers for checking whether the Overture CLI is
//! installed on the system PATH, invoking it to download GeoJSON data for a
//! given theme and bounding box, and converting the resulting GeoJSON into
//! the `OsmData` structure used by the rest of the pipeline.
//!
//! The `overturemaps` CLI (PyPI: `overturemaps`) is an optional runtime
//! dependency — callers should check [`is_cli_available`] before attempting
//! any download.  If the CLI is absent, the integration is silently skipped.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, Instant};

use crate::osm::{FeatureSource, OsmData, OsmNode, OsmPoiNode, OsmWay};

/// Overture Maps theme selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OvertureTheme {
    /// Building footprints and building metadata.
    Building,
    /// Transportation segments, normalized mostly as OSM-style roads.
    Transportation,
    /// Places and POIs, normalized into tagged POI nodes.
    Place,
    /// Base land, land-use, water, and tree features.
    Base,
    /// Address points, normalized into address nodes.
    Address,
}

impl OvertureTheme {
    /// Return all supported themes in a stable default order.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Building,
            Self::Transportation,
            Self::Place,
            Self::Base,
            Self::Address,
        ]
    }

    /// Return the `overturemaps download --type` values used for this theme.
    pub fn cli_types(&self) -> Vec<&'static str> {
        match self {
            Self::Building => vec!["building"],
            Self::Transportation => vec!["segment"],
            Self::Place => vec!["place"],
            Self::Base => vec!["land", "land_use", "water"],
            Self::Address => vec!["address"],
        }
    }

    /// Parse a user-facing theme string, accepting singular/plural aliases.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        let theme = s.to_lowercase();
        match theme.as_str() {
            "address" | "addresses" | "addr" => Some(Self::Address),
            _ => match theme.strip_suffix('s').unwrap_or(&theme) {
                "building" => Some(Self::Building),
                "transportation" | "transport" | "road" | "segment" => Some(Self::Transportation),
                "place" => Some(Self::Place),
                "base" | "land" | "land_use" | "landuse" | "water" => Some(Self::Base),
                _ => None,
            },
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
    /// Prefer Overture features for this theme.
    Overture,
    /// Prefer OSM/Overpass features for this theme.
    Osm,
    /// Keep features from both sources.
    #[default]
    Both,
}

/// Parameters controlling Overture Maps data integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvertureParams {
    /// Whether Overture should be fetched. Defaults to `false`.
    pub enabled: bool,
    /// Overture themes to fetch when enabled. Defaults to all supported themes.
    pub themes: Vec<OvertureTheme>,
    /// Per-theme source priority for non-POI features. Missing entries default to [`ThemePriority::Both`].
    pub priority: HashMap<OvertureTheme, ThemePriority>,
    /// Timeout for each Overture CLI download command.
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
    /// Return the configured priority for `theme`, defaulting to [`ThemePriority::Both`].
    pub fn priority_for(&self, theme: OvertureTheme) -> ThemePriority {
        self.priority
            .get(&theme)
            .copied()
            .unwrap_or(ThemePriority::Both)
    }
}

// ── Synthetic node-ID counter ─────────────────────────────────────────────

/// Atomic counter for synthetic negative node IDs.
///
/// Overture geometry nodes do not carry OSM IDs.  We assign synthetic
/// negative IDs starting at −1 000 000 000 to avoid any collision with
/// real OSM IDs (which are always positive).
static SYNTHETIC_ID_COUNTER: AtomicI64 = AtomicI64::new(-1_000_000_000);

/// Return the next unique synthetic (negative) node ID.
fn next_synthetic_id() -> i64 {
    SYNTHETIC_ID_COUNTER.fetch_sub(1, Ordering::Relaxed)
}

// ── CLI availability check ────────────────────────────────────────────────

const CLI_CHECK_TIMEOUT: Duration = Duration::from_secs(2);
const CLI_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Check whether the `overturemaps` CLI is available on the system PATH.
///
/// Runs `overturemaps --version` with a short timeout.  Returns `true` if
/// the command succeeds (exit code 0), `false` otherwise.
pub fn is_cli_available() -> bool {
    let Ok(mut child) = std::process::Command::new("overturemaps")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if start.elapsed() >= CLI_CHECK_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(CLI_POLL_INTERVAL);
            }
            Err(_) => return false,
        }
    }
}

const STDERR_SNIPPET_LIMIT: usize = 4096;

fn stderr_suffix(stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        String::new()
    } else if stderr.len() <= STDERR_SNIPPET_LIMIT {
        format!(": {stderr}")
    } else {
        let head_len = STDERR_SNIPPET_LIMIT / 2;
        let tail_len = STDERR_SNIPPET_LIMIT - head_len;
        let head = str_prefix_at_boundary(stderr, head_len);
        let tail = str_suffix_at_boundary(stderr, tail_len);
        let omitted = stderr.len().saturating_sub(head.len() + tail.len());
        format!(": {head}\n...[stderr truncated, {omitted} bytes omitted]...\n{tail}")
    }
}

fn str_prefix_at_boundary(s: &str, max_bytes: usize) -> &str {
    let mut end = max_bytes.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn str_suffix_at_boundary(s: &str, max_bytes: usize) -> &str {
    let mut start = s.len().saturating_sub(max_bytes);
    while !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

fn read_stderr_file(stderr_path: &Path, cli_type: &str) -> Result<Vec<u8>> {
    std::fs::read(stderr_path)
        .with_context(|| format!("reading overturemaps stderr for type '{cli_type}'"))
}

fn wait_with_stderr_file_timeout(
    mut child: std::process::Child,
    stderr_path: &Path,
    timeout: Duration,
    timeout_secs: u64,
    cli_type: &str,
) -> Result<(std::process::ExitStatus, Vec<u8>)> {
    let start = Instant::now();
    loop {
        match child.try_wait().context("polling overturemaps CLI")? {
            Some(status) => {
                let stderr = read_stderr_file(stderr_path, cli_type)?;
                return Ok((status, stderr));
            }
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    child
                        .wait()
                        .context("waiting for overturemaps CLI after timeout")?;
                    let stderr = read_stderr_file(stderr_path, cli_type)?;
                    let stderr_msg = stderr_suffix(&stderr);
                    bail!(
                        "overturemaps CLI timed out after {timeout_secs}s for type '{cli_type}'{stderr_msg}"
                    );
                }
                std::thread::sleep(CLI_POLL_INTERVAL);
            }
        }
    }
}

// ── GeoJSON download via CLI ──────────────────────────────────────────────

/// Download Overture GeoJSON for a single CLI type and bounding box.
///
/// Invokes:
/// ```text
/// overturemaps download --bbox W,S,E,N -t <cli_type> -o <tmpfile>
/// ```
///
/// # Arguments
///
/// * `cli_type` – The Overture type string (e.g. `"building"`, `"segment"`).
/// * `bbox` – `(min_lat, min_lon, max_lat, max_lon)` bounding box.
/// * `timeout_secs` – Maximum wall-clock seconds to wait for the CLI.
///
/// # Returns
///
/// The GeoJSON string written by the CLI, or an error if the CLI fails or
/// times out.
pub fn fetch_geojson_for_type(
    cli_type: &str,
    bbox: (f64, f64, f64, f64),
    timeout_secs: u64,
) -> Result<String> {
    let (min_lat, min_lon, max_lat, max_lon) = bbox;
    // Overture CLI expects W,S,E,N order (min_lon, min_lat, max_lon, max_lat).
    let bbox_str = format!("{min_lon},{min_lat},{max_lon},{max_lat}");

    // Write output to a named temp file so the CLI can stream to disk.
    let tmp = tempfile::Builder::new()
        .suffix(".geojson")
        .tempfile()
        .context("creating temp file for overturemaps output")?;
    let tmp_path = tmp.path().to_path_buf();

    let stderr_tmp = tempfile::Builder::new()
        .suffix(".stderr")
        .tempfile()
        .context("creating temp file for overturemaps stderr")?;
    let stderr_path = stderr_tmp.path().to_path_buf();
    let stderr_file = stderr_tmp
        .reopen()
        .context("opening temp file for overturemaps stderr")?;

    let child = std::process::Command::new("overturemaps")
        .arg("download")
        .arg("-f")
        .arg("geojson")
        .arg("--bbox")
        .arg(&bbox_str)
        .arg("-t")
        .arg(cli_type)
        .arg("-o")
        .arg(&tmp_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(stderr_file))
        .spawn()
        .context("spawning overturemaps CLI")?;

    let (status, stderr) = wait_with_stderr_file_timeout(
        child,
        &stderr_path,
        Duration::from_secs(timeout_secs),
        timeout_secs,
        cli_type,
    )?;

    if !status.success() {
        let stderr_msg = stderr_suffix(&stderr);
        bail!(
            "overturemaps CLI exited with status {} for type '{cli_type}'{stderr_msg}",
            status.code().unwrap_or(-1)
        );
    }

    let content = std::fs::read_to_string(&tmp_path)
        .with_context(|| format!("reading overturemaps output for type '{cli_type}'"))?;

    Ok(content)
}

// ── GeoJSON → OsmData conversion ─────────────────────────────────────────

/// Update a running bounding-box accumulator with a new coordinate.
fn update_bounds(
    min_lat: &mut f64,
    min_lon: &mut f64,
    max_lat: &mut f64,
    max_lon: &mut f64,
    lat: f64,
    lon: f64,
) {
    *min_lat = min_lat.min(lat);
    *min_lon = min_lon.min(lon);
    *max_lat = max_lat.max(lat);
    *max_lon = max_lon.max(lon);
}

/// Convert a GeoJSON coordinate array `[lon, lat]` or `[lon, lat, ele]` to an
/// `(OsmNode, i64)` pair and update the bounding-box accumulator.
///
/// Returns the synthetic node ID and the node, or `None` if the array is
/// malformed.
fn coord_to_node(
    coord: &Value,
    min_lat: &mut f64,
    min_lon: &mut f64,
    max_lat: &mut f64,
    max_lon: &mut f64,
) -> Option<(i64, OsmNode)> {
    let arr = coord.as_array()?;
    let lon = arr.first()?.as_f64()?;
    let lat = arr.get(1)?.as_f64()?;
    update_bounds(min_lat, min_lon, max_lat, max_lon, lat, lon);
    Some((next_synthetic_id(), OsmNode { lat, lon }))
}

/// Convert a GeoJSON coordinate array (ring or line) into a list of node IDs
/// and the corresponding node map entries.
///
/// Each element of `coords` is expected to be a `[lon, lat]` array.
fn coords_to_nodes(
    coords: &[Value],
    min_lat: &mut f64,
    min_lon: &mut f64,
    max_lat: &mut f64,
    max_lon: &mut f64,
) -> (Vec<i64>, HashMap<i64, OsmNode>) {
    let mut node_refs = Vec::with_capacity(coords.len());
    let mut nodes = HashMap::with_capacity(coords.len());
    for coord in coords {
        if let Some((id, node)) = coord_to_node(coord, min_lat, min_lon, max_lat, max_lon) {
            node_refs.push(id);
            nodes.insert(id, node);
        }
    }
    (node_refs, nodes)
}

/// Map an Overture place category string to the appropriate OSM primary key.
fn map_place_category_to_osm_key(category: &str) -> &'static str {
    match category {
        "restaurant" | "cafe" | "bar" | "fast_food" | "food_and_drink" => "amenity",
        "supermarket" | "grocery" | "clothing" | "electronics" | "retail" => "shop",
        "hotel" | "motel" | "hostel" | "accommodation" => "tourism",
        "park" | "playground" | "sports_centre" | "stadium" | "recreation" => "leisure",
        _ => "amenity",
    }
}

/// Map Overture feature properties to OSM-style tags for the given theme.
fn map_tags_for_theme(props: &Value, theme: OvertureTheme) -> HashMap<String, String> {
    let mut tags: HashMap<String, String> = HashMap::new();

    match theme {
        OvertureTheme::Building => {
            // class → building (default "yes")
            let class = props.get("class").and_then(|v| v.as_str()).unwrap_or("yes");
            tags.insert("building".into(), class.to_string());

            // height → building:height
            if let Some(h) = props.get("height").and_then(|v| v.as_f64()) {
                tags.insert("building:height".into(), h.to_string());
            }
            // num_floors → building:levels
            if let Some(f) = props.get("num_floors").and_then(|v| v.as_u64()) {
                tags.insert("building:levels".into(), f.to_string());
            }
        }

        OvertureTheme::Transportation => {
            // class → highway (default "unclassified")
            let class = props
                .get("class")
                .and_then(|v| v.as_str())
                .unwrap_or("unclassified");
            tags.insert("highway".into(), class.to_string());

            // names.primary → name
            if let Some(name) = props
                .get("names")
                .and_then(|n| n.get("primary"))
                .and_then(|v| v.as_str())
            {
                tags.insert("name".into(), name.to_string());
            }
            // road_surface → surface
            if let Some(surface) = props.get("road_surface").and_then(|v| v.as_str()) {
                tags.insert("surface".into(), surface.to_string());
            }
            // is_bridge → bridge=yes
            if props
                .get("is_bridge")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                tags.insert("bridge".into(), "yes".into());
            }
            // is_tunnel → tunnel=yes
            if props
                .get("is_tunnel")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                tags.insert("tunnel".into(), "yes".into());
            }
        }

        OvertureTheme::Place => {
            // categories.primary → amenity / shop / tourism / leisure
            if let Some(category) = props
                .get("categories")
                .and_then(|c| c.get("primary"))
                .and_then(|v| v.as_str())
            {
                let osm_key = map_place_category_to_osm_key(category);
                tags.insert(osm_key.into(), category.to_string());
            }
            // names.primary → name
            if let Some(name) = props
                .get("names")
                .and_then(|n| n.get("primary"))
                .and_then(|v| v.as_str())
            {
                tags.insert("name".into(), name.to_string());
            }
        }

        OvertureTheme::Base => {
            // Overture Base uses "subtype" and "class" to distinguish features.
            // We map them to the appropriate OSM keys.
            let subtype = props.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
            let class = props.get("class").and_then(|v| v.as_str()).unwrap_or("");

            // Water bodies
            if matches!(
                subtype,
                "water" | "lake" | "pond" | "reservoir" | "ocean" | "sea"
            ) {
                tags.insert("natural".into(), "water".into());
                if !subtype.is_empty() && subtype != "water" {
                    tags.insert("water".into(), subtype.to_string());
                }
            }
            // Waterways
            else if matches!(subtype, "river" | "stream" | "canal" | "drain" | "ditch") {
                tags.insert("waterway".into(), subtype.to_string());
            }
            // Land use — from class when subtype indicates land_use
            else if matches!(
                subtype,
                "forest"
                    | "farmland"
                    | "residential"
                    | "commercial"
                    | "industrial"
                    | "cemetery"
                    | "grass"
                    | "scrub"
                    | "farmyard"
            ) {
                tags.insert("landuse".into(), subtype.to_string());
            }
            // Natural land cover from class
            else if matches!(subtype, "land" | "")
                && matches!(
                    class,
                    "grass" | "scrub" | "heath" | "bare_rock" | "sand" | "beach"
                )
            {
                tags.insert("natural".into(), class.to_string());
            }
            // Leisure areas
            else if matches!(subtype, "park" | "garden" | "pitch" | "playground") {
                tags.insert("leisure".into(), subtype.to_string());
            }
            // Individual tree points
            else if subtype == "tree" {
                tags.insert("natural".into(), "tree".into());
            }
            // Fallback: try the class field
            else if !class.is_empty() {
                tags.insert("landuse".into(), class.to_string());
            }
        }

        OvertureTheme::Address => {
            // number → addr:housenumber
            if let Some(number) = props.get("number").and_then(|v| v.as_str()) {
                tags.insert("addr:housenumber".into(), number.to_string());
            }
            // street → addr:street
            if let Some(street) = props.get("street").and_then(|v| v.as_str()) {
                tags.insert("addr:street".into(), street.to_string());
            }
        }
    }

    tags
}

/// Parse an Overture GeoJSON `FeatureCollection` string into an [`OsmData`].
///
/// Each GeoJSON feature is converted according to `theme`:
///
/// - `Point` geometries become POI nodes (Place theme) or address nodes (Address theme).
/// - `LineString` geometries become ways.
/// - `Polygon` geometries become ways using the outer ring.
/// - `MultiPolygon` geometries produce one way per polygon outer ring.
///
/// Synthetic negative node IDs are assigned to avoid collision with OSM IDs.
pub fn parse_overture_geojson(geojson_str: &str, theme: OvertureTheme) -> Result<OsmData> {
    let root: Value = serde_json::from_str(geojson_str).context("parsing Overture GeoJSON")?;

    let features = root
        .get("features")
        .and_then(|f| f.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let mut nodes: HashMap<i64, OsmNode> = HashMap::new();
    let mut ways: Vec<OsmWay> = Vec::new();
    let mut ways_by_id: HashMap<i64, usize> = HashMap::new();
    let mut poi_nodes: Vec<OsmPoiNode> = Vec::new();
    let mut addr_nodes: Vec<OsmPoiNode> = Vec::new();
    let mut tree_nodes: Vec<OsmNode> = Vec::new();

    let mut min_lat = f64::MAX;
    let mut min_lon = f64::MAX;
    let mut max_lat = f64::MIN;
    let mut max_lon = f64::MIN;

    for feature in features {
        let props = feature.get("properties").unwrap_or(&Value::Null);
        let tags = map_tags_for_theme(props, theme);

        let geometry = match feature.get("geometry") {
            Some(g) => g,
            None => continue,
        };
        let geom_type = geometry.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let coordinates = geometry.get("coordinates");

        match geom_type {
            "Point" => {
                if let Some(coord) = coordinates
                    && let Some((id, node)) = coord_to_node(
                        coord,
                        &mut min_lat,
                        &mut min_lon,
                        &mut max_lat,
                        &mut max_lon,
                    )
                {
                    nodes.insert(id, node);
                    let poi = OsmPoiNode {
                        lat: node.lat,
                        lon: node.lon,
                        tags: tags.clone(),
                        source: FeatureSource::Overture,
                    };
                    match theme {
                        OvertureTheme::Address => addr_nodes.push(poi),
                        OvertureTheme::Place => poi_nodes.push(poi),
                        _ => {
                            // Decorative tree nodes from land theme
                            if tags.get("natural").map(|s| s.as_str()) == Some("tree") {
                                tree_nodes.push(OsmNode {
                                    lat: node.lat,
                                    lon: node.lon,
                                });
                            }
                        }
                    }
                }
            }

            "LineString" => {
                if let Some(coords) = coordinates.and_then(|c| c.as_array()) {
                    let (node_refs, new_nodes) = coords_to_nodes(
                        coords,
                        &mut min_lat,
                        &mut min_lon,
                        &mut max_lat,
                        &mut max_lon,
                    );
                    if !node_refs.is_empty() {
                        let way_id = next_synthetic_id();
                        let idx = ways.len();
                        ways.push(OsmWay { tags, node_refs });
                        ways_by_id.insert(way_id, idx);
                        nodes.extend(new_nodes);
                    }
                }
            }

            "Polygon" => {
                // Use the outer ring (first element).
                if let Some(outer_ring) = coordinates
                    .and_then(|c| c.as_array())
                    .and_then(|rings| rings.first())
                    .and_then(|r| r.as_array())
                {
                    let (node_refs, new_nodes) = coords_to_nodes(
                        outer_ring,
                        &mut min_lat,
                        &mut min_lon,
                        &mut max_lat,
                        &mut max_lon,
                    );
                    if !node_refs.is_empty() {
                        let way_id = next_synthetic_id();
                        let idx = ways.len();
                        ways.push(OsmWay { tags, node_refs });
                        ways_by_id.insert(way_id, idx);
                        nodes.extend(new_nodes);
                    }
                }
            }

            "MultiPolygon" => {
                // Each polygon produces one way from its outer ring.
                if let Some(polygons) = coordinates.and_then(|c| c.as_array()) {
                    for polygon in polygons {
                        if let Some(outer_ring) = polygon
                            .as_array()
                            .and_then(|rings| rings.first())
                            .and_then(|r| r.as_array())
                        {
                            let (node_refs, new_nodes) = coords_to_nodes(
                                outer_ring,
                                &mut min_lat,
                                &mut min_lon,
                                &mut max_lat,
                                &mut max_lon,
                            );
                            if !node_refs.is_empty() {
                                let way_id = next_synthetic_id();
                                let idx = ways.len();
                                ways.push(OsmWay {
                                    tags: tags.clone(),
                                    node_refs,
                                });
                                ways_by_id.insert(way_id, idx);
                                nodes.extend(new_nodes);
                            }
                        }
                    }
                }
            }

            _ => {
                // Unknown geometry type — skip.
            }
        }
    }

    let bounds = if min_lat < f64::MAX {
        Some((min_lat, min_lon, max_lat, max_lon))
    } else {
        None
    };

    Ok(OsmData {
        nodes,
        ways,
        ways_by_id,
        relations: Vec::new(),
        bounds,
        poi_nodes,
        addr_nodes,
        tree_nodes,
    })
}

// ── Overture cache ────────────────────────────────────────────────────────

/// Serialised metadata stored alongside the `.geojson` cache file.
#[derive(Debug, Serialize, Deserialize)]
/// Metadata stored beside cached Overture GeoJSON files.
pub struct OvertureCacheMeta {
    /// Bounding box `[south, west, north, east]` for the cached download.
    pub bbox: [f64; 4],
    /// Overture CLI type value, such as `place`, `building`, or `segment`.
    pub cli_type: String,
    /// UTC creation timestamp.
    pub created_at: DateTime<Utc>,
    /// GeoJSON payload size in bytes.
    pub size_bytes: u64,
}

/// Return the Overture GeoJSON cache directory, creating it if needed.
///
/// Priority:
/// 1. `PAR_OSM_OVERTURE_CACHE_DIR` environment variable
/// 2. `OVERTURE_CACHE_DIR` environment variable
/// 3. shared default `overture` directory under [`crate::cache::shared_cache_root`]
///
/// When using the shared default, legacy osm-to-bedrock Overture cache files are
/// migrated into the shared cache on first use. Environment overrides are never
/// migrated.
pub fn overture_cache_dir() -> PathBuf {
    crate::cache::overture_cache_dir()
}

/// Build a deterministic SHA-256 cache key from a bounding box and CLI type.
///
/// Coordinates are snapped to 4 decimal places (~11 m) so small UI drags
/// reuse the same entry.
pub fn overture_cache_key(bbox: (f64, f64, f64, f64), cli_type: &str) -> String {
    let (s, w, n, e) = bbox;
    let canonical = format!("overture|{s:.4},{w:.4},{n:.4},{e:.4}|{cli_type}");
    let hash = Sha256::digest(canonical.as_bytes());
    format!("{hash:x}")
}

/// Return cached GeoJSON for `key`, or `None` if absent or unreadable.
pub fn overture_cache_read(dir: &Path, key: &str) -> Option<String> {
    let path = dir.join(format!("{key}.geojson"));
    match std::fs::read_to_string(&path) {
        Ok(s) => Some(s),
        Err(e) => {
            log::debug!("Overture cache miss for {key}: {e}");
            None
        }
    }
}

/// Atomically write `geojson` + metadata for `key`.
pub fn overture_cache_write(
    dir: &Path,
    key: &str,
    bbox: (f64, f64, f64, f64),
    cli_type: &str,
    geojson: &str,
) -> Result<()> {
    let (s, w, n, e) = bbox;
    let geojson_path = dir.join(format!("{key}.geojson"));
    let meta_path = dir.join(format!("{key}.meta.json"));
    let geojson_tmp = dir.join(format!("{key}.geojson.tmp"));
    let meta_tmp = dir.join(format!("{key}.meta.json.tmp"));

    // Atomic write: write to .tmp then rename
    std::fs::write(&geojson_tmp, geojson)?;
    std::fs::rename(&geojson_tmp, &geojson_path)?;

    let size_bytes = geojson.len() as u64;
    let meta = OvertureCacheMeta {
        bbox: [s, w, n, e],
        cli_type: cli_type.to_string(),
        created_at: Utc::now(),
        size_bytes,
    };
    std::fs::write(&meta_tmp, serde_json::to_string(&meta)?)?;
    std::fs::rename(&meta_tmp, &meta_path)?;

    Ok(())
}

/// A single Overture cache entry for listing purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// A single Overture cache entry returned by [`list_overture_areas`].
pub struct OvertureCacheEntry {
    pub key: String,
    pub bbox: [f64; 4],
    pub cli_type: String,
    pub created_at: DateTime<Utc>,
    pub size_bytes: u64,
}

/// List all valid Overture cache entries.
pub fn list_overture_areas() -> Vec<OvertureCacheEntry> {
    let dir = overture_cache_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(key) = name.strip_suffix(".meta.json") else {
            continue;
        };
        let geojson_path = dir.join(format!("{key}.geojson"));
        if !geojson_path.exists() {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<OvertureCacheMeta>(&raw) else {
            continue;
        };
        result.push(OvertureCacheEntry {
            key: key.to_string(),
            bbox: meta.bbox,
            cli_type: meta.cli_type,
            created_at: meta.created_at,
            size_bytes: meta.size_bytes,
        });
    }
    result
}

/// Clear Overture cache entries, optionally only those older than `min_age`.
///
/// Returns the number of entries deleted.
pub fn clear_overture_cache(min_age: Option<chrono::Duration>) -> Result<usize> {
    clear_overture_cache_dir(&overture_cache_dir(), min_age)
}

fn clear_overture_cache_dir(dir: &Path, min_age: Option<chrono::Duration>) -> Result<usize> {
    if !dir.exists() {
        log::info!("Overture cache dir does not exist; nothing to clear");
        return Ok(0);
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(0);
    };
    let now = Utc::now();
    let mut deleted = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(key) = name.strip_suffix(".meta.json") else {
            // Remove orphaned .geojson files (no paired .meta.json)
            if let Some(stem) = name.strip_suffix(".geojson") {
                let meta_name = format!("{stem}.meta.json");
                if !dir.join(&meta_name).exists() {
                    let _ = std::fs::remove_file(&path);
                }
            }
            continue;
        };
        if let Some(min_age) = min_age {
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(meta) = serde_json::from_str::<OvertureCacheMeta>(&raw) else {
                continue;
            };
            let age = now.signed_duration_since(meta.created_at);
            if age <= min_age {
                continue; // fresh — keep it
            }
        }
        let geojson_path = dir.join(format!("{key}.geojson"));
        let meta_path = dir.join(format!("{key}.meta.json"));
        let _ = std::fs::remove_file(&geojson_path);
        let _ = std::fs::remove_file(&meta_path);
        deleted += 1;
    }
    Ok(deleted)
}

// ── High-level fetch API ──────────────────────────────────────────────────

/// Create an empty [`OsmData`] to accumulate merged results into.
fn empty_osm_data() -> OsmData {
    OsmData {
        nodes: HashMap::new(),
        ways: vec![],
        ways_by_id: HashMap::new(),
        relations: vec![],
        bounds: None,
        poi_nodes: vec![],
        addr_nodes: vec![],
        tree_nodes: vec![],
    }
}

/// Fetch and parse Overture Maps data for all enabled themes, merging into a
/// single [`OsmData`].
///
/// For each CLI type belonging to each requested theme:
/// 1. Check the disk cache.
/// 2. On cache miss, invoke the `overturemaps` CLI to download GeoJSON.
/// 3. Write the result to cache.
/// 4. Parse the GeoJSON into `OsmData` and merge.
///
/// # Errors
///
/// Returns an error if `params.enabled` is false, the CLI is not installed,
/// or any theme fetch or parse fails.
/// Fetch Overture data for the enabled themes in `params` and normalize it into [`OsmData`].
///
/// This function shells out to the optional `overturemaps` CLI and may perform
/// network I/O. The returned data can be merged with OSM data via
/// [`crate::sources::merge_source_data`] or fetched through the higher-level
/// [`crate::sources::fetch_map_data`] orchestrator.
pub fn fetch_overture_data(
    bbox: (f64, f64, f64, f64),
    params: &OvertureParams,
    progress_cb: &mut dyn FnMut(f32, &str),
) -> Result<OsmData> {
    if !params.enabled {
        bail!("Overture Maps integration is not enabled");
    }
    if !is_cli_available() {
        bail!(
            "The `overturemaps` CLI is not installed.\n\
             Install it with: pip install overturemaps\n\
             Then retry."
        );
    }

    let theme_names: Vec<String> = params.themes.iter().map(|t| t.to_string()).collect();
    log::info!(
        "Starting Overture Maps fetch (bbox: {:.4},{:.4},{:.4},{:.4}, themes: {})",
        bbox.0,
        bbox.1,
        bbox.2,
        bbox.3,
        theme_names.join(", ")
    );

    let cache_dir = overture_cache_dir();

    // Flatten all (theme, cli_type) pairs so we can report progress as a
    // fraction of total work.
    let pairs: Vec<(OvertureTheme, &'static str)> = params
        .themes
        .iter()
        .flat_map(|&theme| theme.cli_types().into_iter().map(move |t| (theme, t)))
        .collect();

    let total = pairs.len() as f32;
    let mut accumulated = empty_osm_data();

    for (i, (theme, cli_type)) in pairs.iter().enumerate() {
        let pct = i as f32 / total;
        progress_cb(pct, &format!("Fetching Overture {cli_type}…"));

        let key = overture_cache_key(bbox, cli_type);
        let geojson = if let Some(cached) = overture_cache_read(&cache_dir, &key) {
            log::debug!("Overture cache hit for {cli_type} (key {key})");
            cached
        } else {
            log::debug!("Overture cache miss for {cli_type} — downloading");
            let fetched = fetch_geojson_for_type(cli_type, bbox, params.timeout_secs)
                .with_context(|| format!("fetching Overture data for type '{cli_type}'"))?;
            overture_cache_write(&cache_dir, &key, bbox, cli_type, &fetched)
                .with_context(|| format!("caching Overture data for type '{cli_type}'"))?;
            fetched
        };

        let data = parse_overture_geojson(&geojson, *theme)
            .with_context(|| format!("parsing Overture GeoJSON for type '{cli_type}'"))?;
        accumulated.merge(data);
    }

    log::info!(
        "Overture Maps fetch complete ({} ways, {} POI nodes, {} address nodes)",
        accumulated.ways.len(),
        accumulated.poi_nodes.len(),
        accumulated.addr_nodes.len(),
    );
    progress_cb(1.0, "Overture data ready");
    Ok(accumulated)
}

/// Like [`fetch_overture_data`] but never fails.
///
/// - If Overture is disabled, returns empty [`OsmData`].
/// - If the CLI is unavailable, returns empty [`OsmData`] after logging a warning.
/// - If a theme fetch fails, logs a warning and skips it.
/// - If parsing a GeoJSON result fails, logs a warning and skips it.
///
/// Use this lower-level helper when callers want partial Overture data without
/// bubbling errors. Applications that need explicit fallback status should prefer
/// [`crate::sources::fetch_map_data`].
pub fn fetch_overture_data_best_effort(
    bbox: (f64, f64, f64, f64),
    params: &OvertureParams,
    progress_cb: &mut dyn FnMut(f32, &str),
) -> OsmData {
    if !params.enabled {
        return empty_osm_data();
    }
    if !is_cli_available() {
        log::warn!(
            "Overture Maps CLI not available — skipping Overture data.\n\
             Install with: pip install overturemaps"
        );
        return empty_osm_data();
    }

    let cache_dir = overture_cache_dir();

    let pairs: Vec<(OvertureTheme, &'static str)> = params
        .themes
        .iter()
        .flat_map(|&theme| theme.cli_types().into_iter().map(move |t| (theme, t)))
        .collect();

    let total = pairs.len() as f32;
    let mut accumulated = empty_osm_data();

    for (i, (theme, cli_type)) in pairs.iter().enumerate() {
        let pct = i as f32 / total;
        progress_cb(pct, &format!("Fetching Overture {cli_type}…"));

        let key = overture_cache_key(bbox, cli_type);
        let geojson = if let Some(cached) = overture_cache_read(&cache_dir, &key) {
            cached
        } else {
            match fetch_geojson_for_type(cli_type, bbox, params.timeout_secs) {
                Ok(fetched) => {
                    if let Err(e) = overture_cache_write(&cache_dir, &key, bbox, cli_type, &fetched)
                    {
                        log::warn!("Failed to write Overture cache for {cli_type}: {e}");
                    }
                    fetched
                }
                Err(e) => {
                    log::warn!("Skipping Overture type '{cli_type}': {e}");
                    continue;
                }
            }
        };

        match parse_overture_geojson(&geojson, *theme) {
            Ok(data) => accumulated.merge(data),
            Err(e) => {
                log::warn!("Failed to parse Overture GeoJSON for '{cli_type}': {e}");
            }
        }
    }

    progress_cb(1.0, "Overture data ready");
    accumulated
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::Mutex;

    static PATH_LOCK: Mutex<()> = Mutex::new(());

    struct PathGuard {
        original_path: Option<OsString>,
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            match &self.original_path {
                Some(path) => unsafe { std::env::set_var("PATH", path) },
                None => unsafe { std::env::remove_var("PATH") },
            }
        }
    }

    fn prepend_to_path(path: &Path) -> PathGuard {
        let original_path = std::env::var_os("PATH");
        let mut paths = vec![path.to_path_buf()];
        if let Some(original) = &original_path {
            paths.extend(std::env::split_paths(original));
        }
        let joined = std::env::join_paths(paths).expect("join PATH entries");
        unsafe { std::env::set_var("PATH", joined) };

        PathGuard { original_path }
    }

    #[cfg(unix)]
    fn write_fake_overturemaps(dir: &Path, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join("overturemaps");
        std::fs::write(&path, script).expect("write fake overturemaps script");
        let mut permissions = std::fs::metadata(&path)
            .expect("fake overturemaps metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod fake overturemaps script");
        path
    }

    // ── helpers ──────────────────────────────────────────────────────────

    fn point_feature(lon: f64, lat: f64, props: serde_json::Value) -> String {
        serde_json::json!({
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "geometry": {
                    "type": "Point",
                    "coordinates": [lon, lat]
                },
                "properties": props
            }]
        })
        .to_string()
    }

    fn polygon_feature(props: serde_json::Value) -> String {
        // A simple 4-corner square polygon.
        serde_json::json!({
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [[
                        [0.0, 0.0],
                        [0.0, 1.0],
                        [1.0, 1.0],
                        [1.0, 0.0],
                        [0.0, 0.0]
                    ]]
                },
                "properties": props
            }]
        })
        .to_string()
    }

    fn line_feature(props: serde_json::Value) -> String {
        serde_json::json!({
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "geometry": {
                    "type": "LineString",
                    "coordinates": [
                        [0.0, 0.0],
                        [0.0, 1.0],
                        [1.0, 1.0]
                    ]
                },
                "properties": props
            }]
        })
        .to_string()
    }

    // ── Theme parsing tests ──────────────────────────────────────────────

    #[test]
    fn from_str_loose_parses_address_singular_and_plural() {
        assert_eq!(
            OvertureTheme::from_str_loose("address"),
            Some(OvertureTheme::Address)
        );
        assert_eq!(
            OvertureTheme::from_str_loose("addresses"),
            Some(OvertureTheme::Address)
        );
    }

    #[test]
    fn from_str_loose_preserves_existing_accepted_forms() {
        assert_eq!(
            OvertureTheme::from_str_loose("buildings"),
            Some(OvertureTheme::Building)
        );
        assert_eq!(
            OvertureTheme::from_str_loose("roads"),
            Some(OvertureTheme::Transportation)
        );
        assert_eq!(
            OvertureTheme::from_str_loose("landuse"),
            Some(OvertureTheme::Base)
        );
        assert_eq!(
            OvertureTheme::from_str_loose("addr"),
            Some(OvertureTheme::Address)
        );
    }

    // ── CLI tests ────────────────────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn fetch_geojson_drains_large_stderr_without_waiting_for_timeout() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        write_fake_overturemaps(
            tmp.path(),
            r#"#!/bin/sh
printf 'fake overturemaps useful error: stderr flood begins\n' >&2
i=0
while [ "$i" -lt 20000 ]; do
  printf 'stderr filler line %05d abcdefghijklmnopqrstuvwxyz\n' "$i" >&2
  i=$((i + 1))
done
printf 'fake overturemaps useful error: final diagnostic\n' >&2
exit 23
"#,
        );

        let _lock = PATH_LOCK.lock().expect("PATH lock poisoned");
        let _path_guard = prepend_to_path(tmp.path());
        let start = Instant::now();

        let err = fetch_geojson_for_type("place", (51.5, -0.13, 51.52, -0.10), 5)
            .expect_err("fake CLI should fail");

        assert!(
            start.elapsed() < Duration::from_secs(2),
            "fetch should return promptly instead of waiting for timeout; elapsed {:?}",
            start.elapsed()
        );
        let message = err.to_string();
        assert!(
            message.contains("fake overturemaps useful error"),
            "error should include useful stderr snippet, got: {message}"
        );
    }

    // ── Building tests ───────────────────────────────────────────────────

    #[test]
    fn building_with_class_height_floors() {
        let geojson = polygon_feature(serde_json::json!({
            "class": "residential",
            "height": 12.5,
            "num_floors": 4
        }));
        let data = parse_overture_geojson(&geojson, OvertureTheme::Building).unwrap();
        assert_eq!(data.ways.len(), 1);
        let tags = &data.ways[0].tags;
        assert_eq!(tags["building"], "residential");
        assert_eq!(tags["building:height"], "12.5");
        assert_eq!(tags["building:levels"], "4");
    }

    #[test]
    fn building_no_class_defaults_yes() {
        let geojson = polygon_feature(serde_json::json!({}));
        let data = parse_overture_geojson(&geojson, OvertureTheme::Building).unwrap();
        assert_eq!(data.ways.len(), 1);
        assert_eq!(data.ways[0].tags["building"], "yes");
    }

    // ── Transportation tests ─────────────────────────────────────────────

    #[test]
    fn transportation_all_fields() {
        let geojson = line_feature(serde_json::json!({
            "class": "primary",
            "names": { "primary": "Main Street" },
            "road_surface": "paved",
            "is_bridge": true,
            "is_tunnel": false
        }));
        let data = parse_overture_geojson(&geojson, OvertureTheme::Transportation).unwrap();
        assert_eq!(data.ways.len(), 1);
        let tags = &data.ways[0].tags;
        assert_eq!(tags["highway"], "primary");
        assert_eq!(tags["name"], "Main Street");
        assert_eq!(tags["surface"], "paved");
        assert_eq!(tags["bridge"], "yes");
        assert!(!tags.contains_key("tunnel"));
    }

    #[test]
    fn transportation_no_class_defaults_unclassified() {
        let geojson = line_feature(serde_json::json!({}));
        let data = parse_overture_geojson(&geojson, OvertureTheme::Transportation).unwrap();
        assert_eq!(data.ways[0].tags["highway"], "unclassified");
    }

    // ── Place tests ──────────────────────────────────────────────────────

    #[test]
    fn place_becomes_poi_node() {
        let geojson = point_feature(
            -0.1,
            51.5,
            serde_json::json!({
                "categories": { "primary": "restaurant" },
                "names": { "primary": "The Bistro" }
            }),
        );
        let data = parse_overture_geojson(&geojson, OvertureTheme::Place).unwrap();
        assert_eq!(data.poi_nodes.len(), 1);
        assert_eq!(data.poi_nodes[0].tags["amenity"], "restaurant");
        assert_eq!(data.poi_nodes[0].tags["name"], "The Bistro");
        assert_eq!(data.poi_nodes[0].source, FeatureSource::Overture);
        assert!((data.poi_nodes[0].lat - 51.5).abs() < 1e-9);
        assert!((data.poi_nodes[0].lon - -0.1).abs() < 1e-9);
    }

    // ── Base theme tests ─────────────────────────────────────────────────

    #[test]
    fn base_water_subtype_maps_to_natural_water() {
        let geojson = polygon_feature(serde_json::json!({
            "subtype": "lake",
            "class": "lake"
        }));
        let data = parse_overture_geojson(&geojson, OvertureTheme::Base).unwrap();
        assert_eq!(data.ways.len(), 1);
        assert_eq!(data.ways[0].tags["natural"], "water");
        assert_eq!(data.ways[0].tags["water"], "lake");
    }

    #[test]
    fn base_landuse_forest_subtype() {
        let geojson = polygon_feature(serde_json::json!({
            "subtype": "forest",
            "class": "forest"
        }));
        let data = parse_overture_geojson(&geojson, OvertureTheme::Base).unwrap();
        assert_eq!(data.ways.len(), 1);
        assert_eq!(data.ways[0].tags["landuse"], "forest");
    }

    // ── Address tests ────────────────────────────────────────────────────

    #[test]
    fn address_becomes_addr_node() {
        let geojson = point_feature(
            -0.2,
            51.6,
            serde_json::json!({
                "number": "42",
                "street": "Baker Street"
            }),
        );
        let data = parse_overture_geojson(&geojson, OvertureTheme::Address).unwrap();
        assert_eq!(data.addr_nodes.len(), 1);
        assert_eq!(data.addr_nodes[0].tags["addr:housenumber"], "42");
        assert_eq!(data.addr_nodes[0].tags["addr:street"], "Baker Street");
        assert_eq!(data.addr_nodes[0].source, FeatureSource::Overture);
        // Should NOT appear in poi_nodes.
        assert_eq!(data.poi_nodes.len(), 0);
    }

    // ── Edge cases ───────────────────────────────────────────────────────

    #[test]
    fn empty_feature_collection_returns_empty_osm_data() {
        let geojson = r#"{"type":"FeatureCollection","features":[]}"#;
        let data = parse_overture_geojson(geojson, OvertureTheme::Building).unwrap();
        assert!(data.nodes.is_empty());
        assert!(data.ways.is_empty());
        assert!(data.poi_nodes.is_empty());
        assert!(data.addr_nodes.is_empty());
        assert!(data.bounds.is_none());
    }

    #[test]
    fn multipolygon_produces_multiple_ways() {
        let geojson = serde_json::json!({
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "geometry": {
                    "type": "MultiPolygon",
                    "coordinates": [
                        [[[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [0.0, 0.0]]],
                        [[[2.0, 2.0], [2.0, 3.0], [3.0, 3.0], [2.0, 2.0]]]
                    ]
                },
                "properties": { "class": "office" }
            }]
        })
        .to_string();
        let data = parse_overture_geojson(&geojson, OvertureTheme::Building).unwrap();
        assert_eq!(data.ways.len(), 2);
    }

    #[test]
    fn bounds_computed_correctly() {
        let geojson = polygon_feature(serde_json::json!({}));
        let data = parse_overture_geojson(&geojson, OvertureTheme::Building).unwrap();
        let (min_lat, min_lon, max_lat, max_lon) = data.bounds.unwrap();
        assert!((min_lat - 0.0).abs() < 1e-9);
        assert!((min_lon - 0.0).abs() < 1e-9);
        assert!((max_lat - 1.0).abs() < 1e-9);
        assert!((max_lon - 1.0).abs() < 1e-9);
    }

    // ── Cache tests ──────────────────────────────────────────────────────

    #[test]
    fn overture_cache_key_is_deterministic() {
        let bbox = (51.5, -0.13, 51.52, -0.10);
        let k1 = overture_cache_key(bbox, "building");
        let k2 = overture_cache_key(bbox, "building");
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 64, "SHA-256 hex should be 64 chars");
    }

    #[test]
    fn overture_cache_key_varies_by_theme() {
        let bbox = (51.5, -0.13, 51.52, -0.10);
        let k1 = overture_cache_key(bbox, "building");
        let k2 = overture_cache_key(bbox, "segment");
        assert_ne!(k1, k2);
    }

    #[test]
    fn overture_cache_write_read_roundtrip() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let bbox = (51.5_f64, -0.13_f64, 51.52_f64, -0.10_f64);
        let key = overture_cache_key(bbox, "building");
        let geojson = r#"{"type":"FeatureCollection","features":[]}"#;

        overture_cache_write(tmp.path(), &key, bbox, "building", geojson).unwrap();
        let result = overture_cache_read(tmp.path(), &key);
        assert_eq!(result.as_deref(), Some(geojson));
    }
}
