//! Input validation, authentication middleware, and constants.

use axum::{
    Json,
    http::StatusCode,
    response::IntoResponse,
};

use super::types::{ErrorResponse, PrepareAreaError, PrepareResult};

pub const MAX_BBOX_SPAN_DEGREES: f64 = 0.5;
pub const MAX_BBOX_AREA_DEGREES: f64 = 0.10;
pub const MAX_SRTM_TILE_COUNT: usize = 16;
pub const OVERTURE_FALLBACK_TO_OSM_STATUS: &str = "overture_fallback_to_osm";

/// Authentication token for mutating API endpoints.
/// Read from the `OSM_WORLD_API_TOKEN` environment variable at server startup.
/// If not set, mutating endpoints return 401 to all callers.
pub fn api_auth_token() -> Option<&'static str> {
    static TOKEN: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    TOKEN
        .get_or_init(|| std::env::var("OSM_WORLD_API_TOKEN").ok())
        .as_deref()
}

/// Allowlist of renderer flags that may appear in `extra_args`.
/// Each entry is a flag name (with leading `--`). Flags that take values
/// must have the value validated separately if they accept untrusted input.
const ALLOWED_RENDERER_FLAGS: &[&str] = &[
    "--screenshot",
    "--screenshot-delay",
    "--auto-exit",
    "--width",
    "--height",
    "--cam-x",
    "--cam-y",
    "--cam-z",
    "--cam-yaw",
    "--cam-pitch",
    "--show-settings",
    "--time-of-day",
    "--real-time-of-day",
    "--hide-poi-labels",
    "--hide-address-labels",
    "--hide-street-sign-labels",
    "--hide-minimap",
    "--rotate-minimap",
    "--debug-shadow-cascades",
    "--visual-preset",
    "--landmark-detail",
    "--facade-variation",
    "--roof-variation",
    "--vegetation-density",
    "--synthetic-tree-cap",
    "--vegetation-distance",
    "--no-streaming",
    "--tile-size",
    "--stream-radius",
    "--upload-budget-mb",
    "--max-uploaded-tiles",
    "--max-uploaded-mb",
];

/// Validate that every flag in `extra_args` is in the allowlist.
/// Returns an error listing any rejected flags.
pub fn validate_extra_args(extra_args: &[String]) -> Result<(), anyhow::Error> {
    let rejected: Vec<&str> = extra_args
        .iter()
        .filter(|arg| arg.starts_with('-') && !ALLOWED_RENDERER_FLAGS.contains(&arg.as_str()))
        .map(|s| s.as_str())
        .collect();
    if rejected.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(
            "unsupported renderer flag(s): {}. Allowed flags: {}",
            rejected.join(", "),
            ALLOWED_RENDERER_FLAGS.join(", ")
        )
    }
}

/// Axum middleware that rejects unauthenticated requests to mutating endpoints.
/// Expects a `Bearer <token>` Authorization header matching `OSM_WORLD_API_TOKEN`.
pub async fn auth_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    match api_auth_token() {
        // No token configured -- allow all requests (local-only development).
        None => next.run(request).await,
        // Token configured -- require matching Bearer header.
        Some(expected) => {
            let provided = request
                .headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "));
            match provided {
                Some(token) if token == expected => next.run(request).await,
                _ => (
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "unauthorized".to_string(),
                    }),
                )
                    .into_response(),
            }
        }
    }
}

pub(crate) fn validate_cache_key(cache_key: &str) -> PrepareResult<()> {
    if cache_key.len() == 64 && cache_key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid prepared area cache key"
        )))
    }
}

pub(crate) fn validate_bbox(bbox: [f64; 4]) -> PrepareResult<(f64, f64, f64, f64)> {
    let [south, west, north, east] = bbox;
    if !south.is_finite() || !west.is_finite() || !north.is_finite() || !east.is_finite() {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid bbox: all coordinates must be finite"
        )));
    }
    if south < -90.0 || north > 90.0 {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid bbox: latitude must be in -90..=90"
        )));
    }
    if west < -180.0 || east > 180.0 {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid bbox: longitude must be in -180..=180"
        )));
    }
    if south >= north {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid bbox: south ({south}) must be less than north ({north})"
        )));
    }
    if west >= east {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid bbox: west ({west}) must be less than east ({east})"
        )));
    }

    let lat_span = north - south;
    let lon_span = east - west;
    if lat_span > MAX_BBOX_SPAN_DEGREES {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid bbox: latitude span ({lat_span}) exceeds maximum ({MAX_BBOX_SPAN_DEGREES})"
        )));
    }
    if lon_span > MAX_BBOX_SPAN_DEGREES {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid bbox: longitude span ({lon_span}) exceeds maximum ({MAX_BBOX_SPAN_DEGREES})"
        )));
    }
    let area = lat_span * lon_span;
    if area > MAX_BBOX_AREA_DEGREES {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid bbox: area ({area}) exceeds maximum ({MAX_BBOX_AREA_DEGREES})"
        )));
    }

    Ok((south, west, north, east))
}

pub(crate) fn validate_spawn(
    spawn_lat: Option<f64>,
    spawn_lon: Option<f64>,
    bbox: (f64, f64, f64, f64),
) -> PrepareResult<Option<(f64, f64)>> {
    let (lat, lon) = match (spawn_lat, spawn_lon) {
        (None, None) => return Ok(None),
        (Some(lat), Some(lon)) => (lat, lon),
        _ => {
            return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
                "spawn_lat and spawn_lon must be provided together"
            )));
        }
    };

    if !lat.is_finite() {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid spawn_lat: latitude must be finite"
        )));
    }
    if !lon.is_finite() {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid spawn_lon: longitude must be finite"
        )));
    }
    if !(-90.0..=90.0).contains(&lat) {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid spawn_lat: latitude must be in -90..=90"
        )));
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid spawn_lon: longitude must be in -180..=180"
        )));
    }

    let (south, west, north, east) = bbox;
    if lat < south || lat > north || lon < west || lon > east {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "spawn point must be inside requested bbox"
        )));
    }

    Ok(Some((lat, lon)))
}

pub(crate) fn validate_srtm_tile_limit(bbox: (f64, f64, f64, f64)) -> PrepareResult<()> {
    let tiles = par_osm_rust::srtm::tiles_for_bbox(bbox.0, bbox.1, bbox.2, bbox.3);
    if tiles.len() > MAX_SRTM_TILE_COUNT {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "requested bbox requires {} SRTM tiles; maximum is {MAX_SRTM_TILE_COUNT}",
            tiles.len()
        )));
    }
    Ok(())
}

pub(crate) fn validate_filter(filter: &par_osm_rust::filter::FeatureFilter) -> PrepareResult<()> {
    if !(filter.roads || filter.buildings || filter.water || filter.landuse || filter.railways) {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "all feature types are disabled"
        )));
    }
    Ok(())
}

pub(crate) fn classify_srtm_error(err: anyhow::Error) -> PrepareAreaError {
    let message = format!("{err:#}");
    if message.contains("Failed to write tmp file") || message.contains("Failed to rename") {
        PrepareAreaError::internal("failed to prepare area", err)
    } else {
        PrepareAreaError::upstream("failed to fetch elevation data", err)
    }
}

pub fn source_status_string(status: par_osm_rust::sources::SourceStatus) -> String {
    match status {
        par_osm_rust::sources::SourceStatus::OsmOnly => "osm_only",
        par_osm_rust::sources::SourceStatus::OvertureOnly => "overture_only",
        par_osm_rust::sources::SourceStatus::Both => "both",
        par_osm_rust::sources::SourceStatus::OverturePreferred => "overture_preferred",
        par_osm_rust::sources::SourceStatus::OvertureFallbackToOsm => {
            OVERTURE_FALLBACK_TO_OSM_STATUS
        }
    }
    .to_string()
}

pub fn is_degraded_overture_prepared_cache(overture_enabled: bool, source_status: &str) -> bool {
    overture_enabled && source_status == OVERTURE_FALLBACK_TO_OSM_STATUS
}

pub(crate) fn effective_overpass_url_for_prepare(overpass_url: Option<&str>) -> PrepareResult<String> {
    let url = match overpass_url {
        Some(url) => url,
        None => par_osm_rust::overpass::default_overpass_url(),
    };
    par_osm_rust::overpass::validate_overpass_url(url)
        .map_err(|err| PrepareAreaError::bad_request(err.context("validating Overpass URL")))?;
    reqwest::Url::parse(url)
        .map(|parsed| parsed.to_string())
        .map_err(|err| {
            PrepareAreaError::bad_request(anyhow::Error::new(err).context("parsing Overpass URL"))
        })
}

pub(crate) fn parse_overture_themes_for_prepare(
    values: &[String],
) -> PrepareResult<Vec<par_osm_rust::overture::OvertureTheme>> {
    let themes = if values.is_empty() {
        par_osm_rust::overture::OvertureTheme::all()
    } else {
        values
            .iter()
            .map(|value| {
                par_osm_rust::overture::OvertureTheme::from_str_loose(value).ok_or_else(|| {
                    PrepareAreaError::bad_request(anyhow::anyhow!(
                        "unknown Overture theme '{value}'"
                    ))
                })
            })
            .collect::<PrepareResult<Vec<_>>>()?
    };
    Ok(canonicalize_overture_themes(themes))
}

pub fn canonicalize_overture_themes(
    themes: Vec<par_osm_rust::overture::OvertureTheme>,
) -> Vec<par_osm_rust::overture::OvertureTheme> {
    let mut themes_by_name = std::collections::BTreeMap::new();
    for theme in themes {
        themes_by_name.entry(theme.to_string()).or_insert(theme);
    }
    themes_by_name.into_values().collect()
}

pub fn canonical_overture_theme_names_for_key(values: &[String]) -> Vec<String> {
    let themes = if values.is_empty() {
        par_osm_rust::overture::OvertureTheme::all()
    } else {
        values
            .iter()
            .filter_map(|value| par_osm_rust::overture::OvertureTheme::from_str_loose(value))
            .collect()
    };
    canonicalize_overture_themes(themes)
        .into_iter()
        .map(|theme| theme.to_string())
        .collect()
}

pub fn prepared_cache_key(
    bbox: (f64, f64, f64, f64),
    filter: &par_osm_rust::filter::FeatureFilter,
    overture: bool,
    themes: &[String],
    poi_source_mode: par_osm_rust::sources::PoiSourceMode,
    failure_mode: par_osm_rust::sources::OvertureFailureMode,
    overpass_url: &str,
) -> String {
    use sha2::{Digest, Sha256};
    let (effective_themes, effective_poi_source_mode, effective_failure_mode) = if overture {
        (
            canonical_overture_theme_names_for_key(themes),
            poi_source_mode,
            failure_mode,
        )
    } else {
        (
            Vec::new(),
            par_osm_rust::sources::PoiSourceMode::OsmOnly,
            par_osm_rust::sources::OvertureFailureMode::FallbackToOsm,
        )
    };
    let payload = serde_json::json!({
        "schema": 3,
        "bbox": [bbox.0, bbox.1, bbox.2, bbox.3],
        "filter": filter,
        "overture": overture,
        "themes": effective_themes,
        "poi_source_mode": effective_poi_source_mode,
        "failure_mode": effective_failure_mode,
        "overpass_url": overpass_url,
    });
    let hash = Sha256::digest(payload.to_string().as_bytes());
    format!("{hash:x}")
}
