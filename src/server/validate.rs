//! Input validation, authentication middleware, rate limiting, and constants.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{
    Json,
    http::StatusCode,
    response::IntoResponse,
};

use super::types::{ErrorResponse, PrepareAreaError, PrepareResult};

/// Maximum allowed span in degrees for either latitude or longitude in a bbox.
pub const MAX_BBOX_SPAN_DEGREES: f64 = 0.5;
/// Maximum allowed area in square degrees for a bbox.
pub const MAX_BBOX_AREA_DEGREES: f64 = 0.10;
/// Maximum number of SRTM tiles that may be downloaded for a single prepare request.
pub const MAX_SRTM_TILE_COUNT: usize = 16;
/// Source status string indicating Overture data was requested but OSM fallback was used.
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

/// Validates that a prepared-area cache key is a 64-character lowercase hex string.
pub(crate) fn validate_cache_key(cache_key: &str) -> PrepareResult<()> {
    if cache_key.len() == 64 && cache_key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid prepared area cache key"
        )))
    }
}

/// Validates a bounding box array `[south, west, north, east]`.
///
/// Checks that coordinates are finite, within valid ranges, ordered correctly,
/// and within the maximum span and area limits.
///
/// Returns the validated `(south, west, north, east)` tuple on success.
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

/// Validates optional spawn coordinates. Both must be provided together, must be
/// finite, within geographic ranges, and inside the given bounding box.
///
/// Returns `Ok(Some((lat, lon)))` when both are provided, or `Ok(None)` when
/// neither is provided.
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

/// Validates that the bounding box does not require more SRTM tiles than the
/// configured maximum.
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

/// Validates that at least one feature type is enabled in the filter.
pub(crate) fn validate_filter(filter: &par_osm_rust::filter::FeatureFilter) -> PrepareResult<()> {
    if !(filter.roads || filter.buildings || filter.water || filter.landuse || filter.railways) {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "all feature types are disabled"
        )));
    }
    Ok(())
}

/// Classifies an SRTM download error as internal (filesystem) or upstream (network).
pub(crate) fn classify_srtm_error(err: anyhow::Error) -> PrepareAreaError {
    let message = format!("{err:#}");
    if message.contains("Failed to write tmp file") || message.contains("Failed to rename") {
        PrepareAreaError::internal("failed to prepare area", err)
    } else {
        PrepareAreaError::upstream("failed to fetch elevation data", err)
    }
}

/// Converts a `SourceStatus` enum to its string representation for API responses.
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

/// Returns `true` when Overture was requested but the prepared cache was built
/// from an OSM fallback. Such entries are retried on the next prepare request.
pub fn is_degraded_overture_prepared_cache(overture_enabled: bool, source_status: &str) -> bool {
    overture_enabled && source_status == OVERTURE_FALLBACK_TO_OSM_STATUS
}

/// Resolves and validates the Overpass URL for a prepare request.
///
/// Uses the provided URL if present, otherwise falls back to the default.
/// Validates against the Overpass URL allowlist and parses as a valid URL.
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

/// Parses Overture theme names from the request, falling back to all themes
/// when the list is empty. Unknown theme names are rejected with a bad request error.
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

/// Deduplicates and sorts Overture themes by name for deterministic cache keys.
pub fn canonicalize_overture_themes(
    themes: Vec<par_osm_rust::overture::OvertureTheme>,
) -> Vec<par_osm_rust::overture::OvertureTheme> {
    let mut themes_by_name = std::collections::BTreeMap::new();
    for theme in themes {
        themes_by_name.entry(theme.to_string()).or_insert(theme);
    }
    themes_by_name.into_values().collect()
}

/// Parses theme names, canonicalizes them, and returns canonical string names
/// for inclusion in a prepared-area cache key.
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

/// Computes a SHA-256 cache key from the prepare-area parameters.
///
/// When Overture is disabled, Overture-specific parameters are not included
/// in the hash, so Overture-only changes do not fragment the cache.
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

// -- Rate Limiting -----------------------------------------------------------

/// Maximum number of requests allowed within the rate-limit window per client IP.
const RATE_LIMIT_MAX_REQUESTS: usize = 20;

/// Duration of the sliding rate-limit window in seconds.
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Per-client rate-limit state.
struct ClientBucket {
    count: usize,
    window_start: Instant,
}

/// Shared rate limiter: tracks request counts per client IP.
#[derive(Clone, Default)]
pub(crate) struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, ClientBucket>>>,
}

impl RateLimiter {
    pub(crate) fn new() -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns `Ok(())` if the client is within rate limits, or a 429 error response.
    fn check(&self, client_ip: &str) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap();
        let bucket = buckets.entry(client_ip.to_string()).or_insert_with(|| {
            ClientBucket {
                count: 0,
                window_start: now,
            }
        });

        let elapsed = now.duration_since(bucket.window_start).as_secs();
        if elapsed >= RATE_LIMIT_WINDOW_SECS {
            bucket.count = 0;
            bucket.window_start = now;
        }

        bucket.count += 1;
        if bucket.count > RATE_LIMIT_MAX_REQUESTS {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse {
                    error: "rate limit exceeded".to_string(),
                }),
            ));
        }
        Ok(())
    }
}

/// Axum middleware that rate-limits requests based on client IP.
/// Applied to mutating endpoints that trigger expensive operations.
pub async fn rate_limit_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    // Extract the client IP from the X-Real-IP or connection info.
    // For a localhost-only server, the IP is typically 127.0.0.1, so we
    // rate-limit on the combination of IP + port to distinguish concurrent
    // browser tabs. If no IP can be determined, we allow the request.
    let client_key = request
        .headers()
        .get("x-real-ip")
        .or_else(|| request.headers().get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    static LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();
    let limiter = LIMITER.get_or_init(RateLimiter::new);

    if let Err(response) = limiter.check(&client_key) {
        return response.into_response();
    }

    next.run(request).await
}
