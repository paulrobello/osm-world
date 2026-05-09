//! Shared source orchestration for OSM/Overpass plus optional Overture Maps data.
//!
//! This module is the preferred entry point for applications that want a single
//! fetch path with consistent source policy, POI dedupe, fallback warnings, and
//! progress reporting. It always fetches OSM/Overpass data first. Overture is
//! fetched only when [`SourceOptions::overture`] has `enabled = true`; source
//! mode alone never forces an Overture network/CLI request.
//!
//! The pure merge function [`merge_source_data`] is separated from the
//! side-effecting [`fetch_map_data`] entry point so tests and consumers can reuse
//! the policy logic with already-loaded data.

use std::collections::HashMap;

use anyhow::Result;

use crate::filter::FeatureFilter;
use crate::osm::{FeatureSource, OsmData, OsmPoiNode};
use crate::overture::OvertureParams;

/// Policy for which POI source should appear in the normalized output.
///
/// Non-POI Overture geometry may still be merged according to Overture theme
/// priority when Overture data is fetched. This enum only controls the final
/// `OsmData::poi_nodes` collection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoiSourceMode {
    /// Use OSM POIs only.
    OsmOnly,
    /// Use Overture POIs only; OSM POIs are cleared when Overture is unavailable.
    OvertureOnly,
    /// Merge OSM and Overture POIs, deduping near duplicates and preferring Overture
    /// representatives for duplicate groups.
    Both,
    /// Prefer Overture POIs, with OSM POIs as fallback when Overture is missing or
    /// returns no POIs.
    #[default]
    OverturePreferred,
}

/// How [`fetch_map_data`] handles Overture fetch failures when Overture is enabled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OvertureFailureMode {
    /// Return OSM data with a warning when Overture fails.
    #[default]
    FallbackToOsm,
    /// Return an error when Overture fails.
    Fail,
}

/// Configuration for [`fetch_map_data`].
#[derive(Debug, Clone)]
pub struct SourceOptions {
    /// Feature categories to request from OSM/Overpass.
    pub filter: FeatureFilter,
    /// Explicit Overpass endpoint. `None` uses [`crate::overpass::default_overpass_url`].
    pub overpass_url: Option<String>,
    /// Whether to read existing raw Overpass cache entries before fetching.
    /// Freshly fetched Overpass XML is still written to cache on success.
    pub use_overpass_cache: bool,
    /// Overture Maps fetch configuration. Overture is skipped unless `enabled` is `true`.
    pub overture: OvertureParams,
    /// Policy for final POI source selection and dedupe.
    pub poi_source_mode: PoiSourceMode,
    /// Failure policy for Overture fetch errors.
    pub overture_failure_mode: OvertureFailureMode,
}

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

/// Effective source outcome after fetching and merging.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    /// Output contains OSM POIs only.
    OsmOnly,
    /// Output contains Overture POIs only.
    OvertureOnly,
    /// Output merged both sources with dedupe.
    Both,
    /// Output preferred Overture POIs successfully.
    OverturePreferred,
    /// Overture was requested but unavailable, failed, or returned no POIs; OSM POIs were used.
    OvertureFallbackToOsm,
}

/// Data and metadata returned by [`fetch_map_data`] and [`merge_source_data`].
pub struct SourceFetchResult {
    /// Normalized map data after source merge policy has been applied.
    pub data: OsmData,
    /// Effective source outcome.
    pub status: SourceStatus,
    /// Human-readable non-fatal warnings, usually Overture fallback reasons.
    pub warnings: Vec<String>,
}

fn normalized_name(tags: &HashMap<String, String>) -> Option<String> {
    tags.get("name")
        .map(|name| name.trim().to_lowercase())
        .filter(|name| !name.is_empty())
}

fn poi_category(tags: &HashMap<String, String>) -> String {
    for key in [
        "amenity", "shop", "tourism", "leisure", "historic", "man_made",
    ] {
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

/// Merge already-loaded OSM and optional Overture data according to `poi_source_mode`.
///
/// Duplicate POIs are detected by category, normalized name, and distance. When
/// both sources describe the same POI, the Overture representative is retained.
/// This function performs no network or cache I/O.
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
            SourceFetchResult {
                data: osm_data,
                status: SourceStatus::OsmOnly,
                warnings,
            }
        }
        (PoiSourceMode::OsmOnly, None) => SourceFetchResult {
            data: osm_data,
            status: SourceStatus::OsmOnly,
            warnings,
        },
        (PoiSourceMode::OvertureOnly, Some(mut overture)) => {
            let overture_pois = overture.poi_nodes.clone();
            osm_data.poi_nodes = overture_pois;
            overture.poi_nodes.clear();
            osm_data.merge(overture);
            SourceFetchResult {
                data: osm_data,
                status: SourceStatus::OvertureOnly,
                warnings,
            }
        }
        (PoiSourceMode::OvertureOnly, None) => {
            osm_data.poi_nodes.clear();
            warnings.push("Overture POIs unavailable for overture-only mode".to_string());
            SourceFetchResult {
                data: osm_data,
                status: SourceStatus::OvertureOnly,
                warnings,
            }
        }
        (PoiSourceMode::Both, Some(mut overture)) => {
            let mut all_pois = original_osm_pois;
            all_pois.extend(overture.poi_nodes.clone());
            overture.poi_nodes.clear();
            osm_data.merge(overture);
            osm_data.poi_nodes = dedupe_pois_with_overture_preference(all_pois);
            SourceFetchResult {
                data: osm_data,
                status: SourceStatus::Both,
                warnings,
            }
        }
        (PoiSourceMode::Both, None) => {
            warnings.push("Overture POIs unavailable; using OSM POIs only".to_string());
            SourceFetchResult {
                data: osm_data,
                status: SourceStatus::OvertureFallbackToOsm,
                warnings,
            }
        }
        (PoiSourceMode::OverturePreferred, Some(mut overture))
            if !overture.poi_nodes.is_empty() =>
        {
            let mut all_pois = original_osm_pois;
            all_pois.extend(overture.poi_nodes.clone());
            overture.poi_nodes.clear();
            osm_data.merge(overture);
            osm_data.poi_nodes = dedupe_pois_with_overture_preference(all_pois);
            SourceFetchResult {
                data: osm_data,
                status: SourceStatus::OverturePreferred,
                warnings,
            }
        }
        (PoiSourceMode::OverturePreferred, Some(mut overture)) => {
            warnings.push("Overture returned no POIs; using OSM POIs only".to_string());
            overture.poi_nodes.clear();
            osm_data.merge(overture);
            osm_data.poi_nodes = original_osm_pois;
            SourceFetchResult {
                data: osm_data,
                status: SourceStatus::OvertureFallbackToOsm,
                warnings,
            }
        }
        (PoiSourceMode::OverturePreferred, None) => {
            warnings.push("Overture POIs unavailable; using OSM POIs only".to_string());
            SourceFetchResult {
                data: osm_data,
                status: SourceStatus::OvertureFallbackToOsm,
                warnings,
            }
        }
    }
}

fn emit_progress(
    progress_cb: &mut dyn FnMut(f32, &str),
    last_progress: &mut f32,
    pct: f32,
    message: &str,
) {
    let pct = if pct.is_finite() {
        pct.clamp(0.0, 1.0)
    } else {
        *last_progress
    };
    if pct >= *last_progress {
        *last_progress = pct;
        progress_cb(pct, message);
    }
}

pub(crate) fn fetch_map_data_with_fetchers<FetchOsm, FetchOverture>(
    bbox: (f64, f64, f64, f64),
    options: &SourceOptions,
    progress_cb: &mut dyn FnMut(f32, &str),
    mut fetch_osm: FetchOsm,
    mut fetch_overture: FetchOverture,
) -> Result<SourceFetchResult>
where
    FetchOsm: FnMut((f64, f64, f64, f64), &FeatureFilter, bool, &str) -> Result<OsmData>,
    FetchOverture:
        FnMut((f64, f64, f64, f64), &OvertureParams, &mut dyn FnMut(f32, &str)) -> Result<OsmData>,
{
    const OSM_DONE_PROGRESS: f32 = 0.45;
    const OVERTURE_DONE_PROGRESS: f32 = 0.90;
    const MERGE_PROGRESS: f32 = 0.95;

    let mut last_progress = 0.0;
    emit_progress(progress_cb, &mut last_progress, 0.0, "Fetching OSM data…");
    let overpass_url = match options.overpass_url.as_deref() {
        Some(url) => url,
        None => crate::overpass::default_overpass_url(),
    };
    let osm_data = fetch_osm(
        bbox,
        &options.filter,
        options.use_overpass_cache,
        overpass_url,
    )?;

    let overture_data = if options.overture.enabled {
        emit_progress(
            progress_cb,
            &mut last_progress,
            OSM_DONE_PROGRESS,
            "OSM data ready; fetching Overture data…",
        );
        let overture_params = options.overture.clone();
        let mut overture_progress = |pct: f32, message: &str| {
            let pct = if pct.is_finite() {
                pct.clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mapped = OSM_DONE_PROGRESS + pct * (OVERTURE_DONE_PROGRESS - OSM_DONE_PROGRESS);
            emit_progress(progress_cb, &mut last_progress, mapped, message);
        };
        match fetch_overture(bbox, &overture_params, &mut overture_progress) {
            Ok(data) => Some(data),
            Err(err) if options.overture_failure_mode == OvertureFailureMode::FallbackToOsm => {
                let warning = format!("Overture fetch failed: {err:#}");
                log::warn!(
                    "{warning}; continuing with configured POI source mode {:?}",
                    options.poi_source_mode
                );
                let mut result = merge_source_data(osm_data, None, options.poi_source_mode);
                result.warnings.push(warning);
                emit_progress(
                    progress_cb,
                    &mut last_progress,
                    MERGE_PROGRESS,
                    "Merging map data…",
                );
                result.data.clip_to_bbox(bbox);
                emit_progress(progress_cb, &mut last_progress, 1.0, "Map data ready");
                return Ok(result);
            }
            Err(err) => return Err(err),
        }
    } else {
        emit_progress(
            progress_cb,
            &mut last_progress,
            OVERTURE_DONE_PROGRESS,
            "OSM data ready",
        );
        None
    };

    emit_progress(
        progress_cb,
        &mut last_progress,
        MERGE_PROGRESS,
        "Merging map data…",
    );
    let mut result = merge_source_data(osm_data, overture_data, options.poi_source_mode);
    result.data.clip_to_bbox(bbox);
    emit_progress(progress_cb, &mut last_progress, 1.0, "Map data ready");
    Ok(result)
}

/// Fetch OSM/Overpass data, optionally fetch Overture data, and apply source policy.
///
/// `bbox` is `(south, west, north, east)` in decimal degrees. `progress` receives
/// monotonically increasing values in the range `0.0..=1.0` for the source fetch
/// phase. The function uses blocking I/O and should be called from an appropriate
/// worker thread in async/UI applications.
///
/// Overture fetches are gated by `options.overture.enabled`. If Overture is
/// disabled, no Overture CLI check, cache read, or network request is performed
/// even when `options.poi_source_mode` is [`PoiSourceMode::OverturePreferred`].
pub fn fetch_map_data(
    bbox: (f64, f64, f64, f64),
    options: &SourceOptions,
    progress_cb: &mut dyn FnMut(f32, &str),
) -> Result<SourceFetchResult> {
    fetch_map_data_with_fetchers(
        bbox,
        options,
        progress_cb,
        crate::overpass::fetch_osm_data,
        crate::overture::fetch_overture_data,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_options_default_uses_overture_preferred_with_fallback() {
        let options = SourceOptions::default();

        assert_eq!(options.poi_source_mode, PoiSourceMode::OverturePreferred);
        assert_eq!(
            options.overture_failure_mode,
            OvertureFailureMode::FallbackToOsm
        );
        assert!(options.use_overpass_cache);
    }

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

    fn poi(
        lat: f64,
        lon: f64,
        key: &str,
        value: &str,
        name: &str,
        source: FeatureSource,
    ) -> OsmPoiNode {
        let mut tags = HashMap::from([(key.to_string(), value.to_string())]);
        if !name.is_empty() {
            tags.insert("name".to_string(), name.to_string());
        }
        OsmPoiNode {
            lat,
            lon,
            tags,
            source,
        }
    }

    fn test_bbox() -> (f64, f64, f64, f64) {
        (0.0, 0.0, 1.0, 1.0)
    }

    #[test]
    fn fetch_map_data_default_options_do_not_invoke_overture_fetcher() {
        let options = SourceOptions::default();
        let mut overture_called = false;
        let mut progress = Vec::new();

        let result = fetch_map_data_with_fetchers(
            test_bbox(),
            &options,
            &mut |pct, message| progress.push((pct, message.to_string())),
            |_, _, _, _| {
                let mut osm = empty_data();
                osm.poi_nodes.push(poi(
                    0.5,
                    0.5,
                    "shop",
                    "bakery",
                    "Bakery",
                    FeatureSource::Osm,
                ));
                Ok(osm)
            },
            |_, _, _| {
                overture_called = true;
                panic!("Overture fetcher should not be called when disabled");
            },
        )
        .expect("fetch succeeds");

        assert!(!overture_called);
        assert_eq!(result.status, SourceStatus::OvertureFallbackToOsm);
        assert_eq!(result.data.poi_nodes.len(), 1);
        assert_eq!(result.data.poi_nodes[0].source, FeatureSource::Osm);
        assert_eq!(progress.last().map(|(pct, _)| *pct), Some(1.0));
    }

    #[test]
    fn fetch_map_data_enabled_overture_invokes_fetcher_and_dedupes_preferred_pois() {
        let mut options = SourceOptions::default();
        options.overture.enabled = true;
        options.poi_source_mode = PoiSourceMode::OverturePreferred;
        let mut overture_called = false;

        let result = fetch_map_data_with_fetchers(
            test_bbox(),
            &options,
            &mut |_, _| {},
            |_, _, _, _| {
                let mut osm = empty_data();
                osm.poi_nodes.push(poi(
                    0.50000,
                    0.50000,
                    "amenity",
                    "restaurant",
                    "Diner",
                    FeatureSource::Osm,
                ));
                Ok(osm)
            },
            |_, params, progress| {
                overture_called = true;
                assert!(params.enabled);
                progress(0.0, "Overture starting");
                progress(1.0, "Overture done");
                let mut overture = empty_data();
                overture.poi_nodes.push(poi(
                    0.50005,
                    0.50005,
                    "amenity",
                    "restaurant",
                    "Diner",
                    FeatureSource::Overture,
                ));
                Ok(overture)
            },
        )
        .expect("fetch succeeds");

        assert!(overture_called);
        assert_eq!(result.status, SourceStatus::OverturePreferred);
        assert_eq!(result.data.poi_nodes.len(), 1);
        assert_eq!(result.data.poi_nodes[0].source, FeatureSource::Overture);
    }

    #[test]
    fn fetch_map_data_fallback_captures_overture_error_warning_and_keeps_osm_result() {
        let mut options = SourceOptions::default();
        options.overture.enabled = true;
        options.poi_source_mode = PoiSourceMode::OverturePreferred;
        options.overture_failure_mode = OvertureFailureMode::FallbackToOsm;

        let result = fetch_map_data_with_fetchers(
            test_bbox(),
            &options,
            &mut |_, _| {},
            |_, _, _, _| {
                let mut osm = empty_data();
                osm.poi_nodes.push(poi(
                    0.5,
                    0.5,
                    "shop",
                    "bakery",
                    "Bakery",
                    FeatureSource::Osm,
                ));
                Ok(osm)
            },
            |_, _, _| anyhow::bail!("synthetic overture failure"),
        )
        .expect("fallback succeeds");

        assert_eq!(result.status, SourceStatus::OvertureFallbackToOsm);
        assert_eq!(result.data.poi_nodes.len(), 1);
        assert_eq!(result.data.poi_nodes[0].source, FeatureSource::Osm);
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("synthetic overture failure"))
        );
    }

    #[test]
    fn fetch_map_data_strict_overture_failure_returns_error() {
        let mut options = SourceOptions::default();
        options.overture.enabled = true;
        options.overture_failure_mode = OvertureFailureMode::Fail;

        let err = match fetch_map_data_with_fetchers(
            test_bbox(),
            &options,
            &mut |_, _| {},
            |_, _, _, _| Ok(empty_data()),
            |_, _, _| anyhow::bail!("strict overture failure"),
        ) {
            Ok(_) => panic!("strict mode should return Overture error"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("strict overture failure"));
    }

    #[test]
    fn fetch_map_data_progress_is_monotonic_and_finishes_at_one() {
        let mut options = SourceOptions::default();
        options.overture.enabled = true;
        let mut progress_values = Vec::new();

        fetch_map_data_with_fetchers(
            test_bbox(),
            &options,
            &mut |pct, _| progress_values.push(pct),
            |_, _, _, _| Ok(empty_data()),
            |_, _, progress| {
                progress(0.0, "Overture reset to zero");
                progress(0.5, "Overture halfway");
                progress(1.0, "Overture complete");
                Ok(empty_data())
            },
        )
        .expect("fetch succeeds");

        assert!(!progress_values.is_empty());
        for window in progress_values.windows(2) {
            assert!(
                window[0] <= window[1],
                "progress moved backwards: {progress_values:?}"
            );
        }
        assert!(
            progress_values[..progress_values.len() - 1]
                .iter()
                .all(|pct| *pct < 1.0)
        );
        assert_eq!(progress_values.last().copied(), Some(1.0));
    }

    #[test]
    fn osm_only_keeps_osm_pois_and_reports_osm_only_status() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(
            0.0,
            0.0,
            "amenity",
            "restaurant",
            "Diner",
            FeatureSource::Osm,
        ));
        let mut overture = empty_data();
        overture.poi_nodes.push(poi(
            0.0,
            0.0,
            "amenity",
            "restaurant",
            "Diner",
            FeatureSource::Overture,
        ));

        let merged = merge_source_data(osm, Some(overture), PoiSourceMode::OsmOnly);

        assert_eq!(merged.status, SourceStatus::OsmOnly);
        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.poi_nodes[0].source, FeatureSource::Osm);
    }

    #[test]
    fn overture_only_keeps_overture_pois() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(
            0.0,
            0.0,
            "amenity",
            "restaurant",
            "Diner",
            FeatureSource::Osm,
        ));
        let mut overture = empty_data();
        overture.poi_nodes.push(poi(
            0.0,
            0.0,
            "amenity",
            "restaurant",
            "Diner",
            FeatureSource::Overture,
        ));

        let merged = merge_source_data(osm, Some(overture), PoiSourceMode::OvertureOnly);

        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.poi_nodes[0].source, FeatureSource::Overture);
    }

    #[test]
    fn overture_only_without_overture_clears_osm_pois_and_warns() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(
            0.0,
            0.0,
            "shop",
            "bakery",
            "Bakery",
            FeatureSource::Osm,
        ));

        let merged = merge_source_data(osm, None, PoiSourceMode::OvertureOnly);

        assert_eq!(merged.status, SourceStatus::OvertureOnly);
        assert!(merged.data.poi_nodes.is_empty());
        assert_eq!(
            merged.warnings,
            vec!["Overture POIs unavailable for overture-only mode".to_string()]
        );
    }

    #[test]
    fn both_dedupes_duplicate_pois_with_overture_winning_and_reports_both_status() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(
            51.50000,
            -0.10000,
            "amenity",
            "restaurant",
            "Diner",
            FeatureSource::Osm,
        ));
        let mut overture = empty_data();
        overture.poi_nodes.push(poi(
            51.50005,
            -0.10005,
            "amenity",
            "restaurant",
            "Diner",
            FeatureSource::Overture,
        ));

        let merged = merge_source_data(osm, Some(overture), PoiSourceMode::Both);

        assert_eq!(merged.status, SourceStatus::Both);
        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.poi_nodes[0].source, FeatureSource::Overture);
    }

    #[test]
    fn same_name_with_category_mismatch_keeps_both_pois() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(
            51.50000,
            -0.10000,
            "amenity",
            "restaurant",
            "Corner",
            FeatureSource::Osm,
        ));
        let mut overture = empty_data();
        overture.poi_nodes.push(poi(
            51.50005,
            -0.10005,
            "shop",
            "bakery",
            "Corner",
            FeatureSource::Overture,
        ));

        let merged = merge_source_data(osm, Some(overture), PoiSourceMode::Both);

        assert_eq!(merged.data.poi_nodes.len(), 2);
        assert!(
            merged
                .data
                .poi_nodes
                .iter()
                .any(|poi| poi.source == FeatureSource::Osm)
        );
        assert!(
            merged
                .data
                .poi_nodes
                .iter()
                .any(|poi| poi.source == FeatureSource::Overture)
        );
    }

    #[test]
    fn overture_preferred_dedupes_named_pois_with_overture_winning_and_reports_success() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(
            51.50000,
            -0.10000,
            "amenity",
            "restaurant",
            "Diner",
            FeatureSource::Osm,
        ));
        let mut overture = empty_data();
        overture.poi_nodes.push(poi(
            51.50005,
            -0.10005,
            "amenity",
            "restaurant",
            "Diner",
            FeatureSource::Overture,
        ));

        let merged = merge_source_data(osm, Some(overture), PoiSourceMode::OverturePreferred);

        assert_eq!(merged.status, SourceStatus::OverturePreferred);
        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.poi_nodes[0].source, FeatureSource::Overture);
    }

    #[test]
    fn overture_preferred_falls_back_when_overture_missing() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(
            0.0,
            0.0,
            "shop",
            "bakery",
            "Bakery",
            FeatureSource::Osm,
        ));

        let merged = merge_source_data(osm, None, PoiSourceMode::OverturePreferred);

        assert_eq!(merged.status, SourceStatus::OvertureFallbackToOsm);
        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.poi_nodes[0].source, FeatureSource::Osm);
        assert!(
            merged
                .warnings
                .iter()
                .any(|warning| warning.contains("Overture POIs unavailable"))
        );
    }

    #[test]
    fn overture_preferred_falls_back_precisely_when_overture_returns_zero_pois() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(
            0.0,
            0.0,
            "shop",
            "bakery",
            "Bakery",
            FeatureSource::Osm,
        ));
        let overture = empty_data();

        let merged = merge_source_data(osm, Some(overture), PoiSourceMode::OverturePreferred);

        assert_eq!(merged.status, SourceStatus::OvertureFallbackToOsm);
        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.poi_nodes[0].source, FeatureSource::Osm);
        assert_eq!(
            merged.warnings,
            vec!["Overture returned no POIs; using OSM POIs only".to_string()]
        );
    }

    #[test]
    fn non_poi_overture_tree_nodes_are_preserved_when_pois_are_filtered() {
        let mut osm = empty_data();
        osm.poi_nodes.push(poi(
            0.0,
            0.0,
            "shop",
            "bakery",
            "Bakery",
            FeatureSource::Osm,
        ));
        let mut overture = empty_data();
        overture.tree_nodes.push(crate::osm::OsmNode {
            lat: 51.5,
            lon: -0.1,
        });

        let merged = merge_source_data(osm, Some(overture), PoiSourceMode::OverturePreferred);

        assert_eq!(merged.status, SourceStatus::OvertureFallbackToOsm);
        assert_eq!(merged.data.poi_nodes.len(), 1);
        assert_eq!(merged.data.tree_nodes.len(), 1);
        assert_eq!(merged.data.tree_nodes[0].lat, 51.5);
        assert_eq!(merged.data.tree_nodes[0].lon, -0.1);
    }
}
