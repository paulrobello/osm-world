//! Input validation, authentication middleware, rate limiting, and constants.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{Json, http::StatusCode, response::IntoResponse};
use subtle::ConstantTimeEq;

use super::types::{ErrorResponse, PrepareAreaError, PrepareResult};

/// Maximum allowed span in degrees for either latitude or longitude in a bbox.
/// 0.5° is roughly 55 km at the equator; smaller toward the poles.
pub const MAX_BBOX_SPAN_DEGREES: f64 = 0.5;
/// Maximum allowed area in square degrees for a bbox. 0.10 deg² is a few
/// thousand km² (≈30 km × 30 km near the equator), large enough for a metro
/// area but small enough to bound SRTM download and parse cost.
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

/// Type of a value-flag entry: flag name plus the validator for its value.
type ValueFlag = (&'static str, fn(&str) -> Result<(), String>);

/// Value-taking flags paired with a validator that mirrors the parser the
/// renderer's own `clap` definition uses in `src/main.rs`. Validators return
/// `Err(message)` so callers see a single descriptive rejection string.
const VALUE_FLAGS: &[ValueFlag] = &[
    ("--screenshot", validate_screenshot_path),
    ("--screenshot-delay", validate_nonnegative_f32),
    ("--auto-exit", validate_nonnegative_f32),
    ("--width", validate_positive_f64),
    ("--height", validate_positive_f64),
    ("--cam-x", validate_finite_f32),
    ("--cam-y", validate_finite_f32),
    ("--cam-z", validate_finite_f32),
    ("--cam-yaw", validate_finite_f32),
    ("--cam-pitch", validate_finite_f32),
    ("--time-of-day", validate_hour_of_day),
    ("--visual-preset", validate_visual_preset),
    ("--landmark-detail", validate_landmark_detail),
    ("--facade-variation", validate_normalized_f32),
    ("--roof-variation", validate_normalized_f32),
    ("--vegetation-density", validate_density_multiplier),
    ("--synthetic-tree-cap", validate_positive_usize),
    ("--vegetation-distance", validate_nonnegative_f32),
    ("--tile-size", validate_positive_f32),
    ("--stream-radius", validate_positive_f32),
    ("--upload-budget-mb", validate_positive_f32),
    ("--max-uploaded-tiles", validate_positive_usize),
    ("--max-uploaded-mb", validate_positive_f32),
];

/// Validate that every flag in `extra_args` is in the allowlist and that every
/// value passed to a value-taking flag parses and is in range. Both space-
/// separated (`--flag value`) and `=`-joined (`--flag=value`) forms are
/// accepted. Returns an error listing every rejection.
pub fn validate_extra_args(extra_args: &[String]) -> Result<(), anyhow::Error> {
    let mut rejected: Vec<&str> = Vec::new();
    let mut value_errors: Vec<String> = Vec::new();
    let mut i = 0;
    while i < extra_args.len() {
        let arg = extra_args[i].as_str();
        // First handle the `--flag=value` form by splitting on the first `=`.
        if let Some(eq_pos) = arg.find('=') {
            let name = &arg[..eq_pos];
            if let Some((_, validator)) = VALUE_FLAGS.iter().find(|(n, _)| *n == name) {
                let value = &arg[eq_pos + 1..];
                if let Err(msg) = validator(value) {
                    value_errors.push(format!("{name}: {msg}"));
                }
                i += 1;
                continue;
            }
            if !ALLOWED_RENDERER_FLAGS.contains(&name) && name.starts_with("--") {
                rejected.push(name);
            } else if VALUE_FLAGS.iter().any(|(n, _)| *n == name) {
                // already handled above
            } else {
                // Boolean switch with an unexpected `=value` suffix. clap would
                // reject this at parse time; report it as a value error.
                value_errors.push(format!("{name} does not take a value"));
            }
            i += 1;
            continue;
        }
        if let Some((_, validator)) = VALUE_FLAGS.iter().find(|(n, _)| *n == arg) {
            match extra_args.get(i + 1) {
                Some(value) => {
                    if let Err(msg) = validator(value) {
                        value_errors.push(format!("{arg}: {msg}"));
                    }
                    i += 2;
                    continue;
                }
                None => {
                    value_errors.push(format!("{arg} requires a value"));
                    break;
                }
            }
        }
        if arg.starts_with("--") && !ALLOWED_RENDERER_FLAGS.contains(&arg) {
            rejected.push(arg);
        }
        i += 1;
    }
    let mut errors: Vec<String> = Vec::new();
    if !rejected.is_empty() {
        errors.push(format!(
            "unsupported renderer flag(s): {}. Allowed flags: {}",
            rejected.join(", "),
            ALLOWED_RENDERER_FLAGS.join(", ")
        ));
    }
    errors.extend(value_errors);
    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(errors.join("; "))
    }
}

// -- extra_args value validators --------------------------------------------
// Each validator mirrors the equivalent `clap` `value_parser` in `src/main.rs`
// so anything accepted here is also accepted by the renderer at parse time.

fn validate_screenshot_path(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err("screenshot path must not be empty".to_string());
    }
    if value.bytes().any(|b| b == 0) {
        return Err("screenshot path must not contain NUL bytes".to_string());
    }
    // Reject `..` components. The renderer runs as the same user as the server,
    // so this is hygiene rather than a hard boundary, but it keeps callers from
    // steering output at ancestor paths via traversal segments.
    if std::path::Path::new(value)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("screenshot path must not contain '..'".to_string());
    }
    Ok(())
}

fn validate_finite_f32(value: &str) -> Result<(), String> {
    let n = value
        .parse::<f32>()
        .map_err(|err| format!("invalid float: {err}"))?;
    if !n.is_finite() {
        return Err("must be a finite number".to_string());
    }
    Ok(())
}

fn validate_nonnegative_f32(value: &str) -> Result<(), String> {
    let n = value
        .parse::<f32>()
        .map_err(|err| format!("invalid float: {err}"))?;
    if !n.is_finite() || n < 0.0 {
        return Err("must be a finite nonnegative number".to_string());
    }
    Ok(())
}

fn validate_positive_f32(value: &str) -> Result<(), String> {
    let n = value
        .parse::<f32>()
        .map_err(|err| format!("invalid float: {err}"))?;
    if !n.is_finite() || n <= 0.0 {
        return Err("must be a finite positive number".to_string());
    }
    Ok(())
}

fn validate_positive_f64(value: &str) -> Result<(), String> {
    let n = value
        .parse::<f64>()
        .map_err(|err| format!("invalid float: {err}"))?;
    if !n.is_finite() || n <= 0.0 {
        return Err("must be a finite positive number".to_string());
    }
    Ok(())
}

fn validate_normalized_f32(value: &str) -> Result<(), String> {
    let n = value
        .parse::<f32>()
        .map_err(|err| format!("invalid float: {err}"))?;
    if !n.is_finite() || !(0.0..=1.0).contains(&n) {
        return Err("must be a finite number in the range 0..=1".to_string());
    }
    Ok(())
}

fn validate_density_multiplier(value: &str) -> Result<(), String> {
    let n = value
        .parse::<f32>()
        .map_err(|err| format!("invalid float: {err}"))?;
    if !n.is_finite() || !(0.0..=3.0).contains(&n) {
        return Err("must be a finite number in the range 0..=3".to_string());
    }
    Ok(())
}

fn validate_positive_usize(value: &str) -> Result<(), String> {
    let n = value
        .parse::<usize>()
        .map_err(|err| format!("invalid integer: {err}"))?;
    if n < 1 {
        return Err("must be at least 1".to_string());
    }
    Ok(())
}

fn validate_hour_of_day(value: &str) -> Result<(), String> {
    let n = value
        .parse::<f32>()
        .map_err(|err| format!("invalid hour: {err}"))?;
    if !n.is_finite() || !(0.0..=24.0).contains(&n) {
        return Err("must be a finite hour in the range 0..=24".to_string());
    }
    Ok(())
}

fn validate_visual_preset(value: &str) -> Result<(), String> {
    match value {
        "performance" | "balanced" | "showcase" => Ok(()),
        _ => Err("must be one of: performance, balanced, showcase".to_string()),
    }
}

fn validate_landmark_detail(value: &str) -> Result<(), String> {
    match value {
        "off" | "simple" | "showcase" => Ok(()),
        _ => Err("must be one of: off, simple, showcase".to_string()),
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
                // Constant-time comparison avoids a timing oracle that would
                // reveal how many leading bytes of a guessed token match.
                Some(token) if bool::from(token.as_bytes().ct_eq(expected.as_bytes())) => {
                    next.run(request).await
                }
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
pub(crate) fn effective_overpass_url_for_prepare(
    overpass_url: Option<&str>,
) -> PrepareResult<String> {
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
    sha256_hex(payload.to_string().as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    const HEX: &[u8; 16] = b"0123456789abcdef";
    let hash = Sha256::digest(bytes);
    let mut encoded = Vec::with_capacity(hash.len() * 2);
    for byte in hash {
        encoded.push(HEX[(byte >> 4) as usize]);
        encoded.push(HEX[(byte & 0x0f) as usize]);
    }
    String::from_utf8(encoded).expect("SHA-256 hex output is valid UTF-8")
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

/// Shared rate limiter: tracks request counts per client key (normally the TCP
/// peer address, optionally the left-most `X-Forwarded-For` entry when the
/// operator opts in via `OSM_WORLD_TRUST_PROXY`).
#[derive(Clone)]
pub(crate) struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, ClientBucket>>>,
    last_sweep: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    pub(crate) fn new() -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            last_sweep: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Returns `Ok(())` if the client is within rate limits, or a 429 error response.
    fn check(&self, client_key: &str) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
        let now = Instant::now();
        // Recover from a poisoned mutex rather than crashing the API. A panic
        // in another request handler would otherwise take down every future
        // rate-limited call.
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let mut last_sweep = self.last_sweep.lock().unwrap_or_else(|e| e.into_inner());
        // Evict expired buckets periodically so the map cannot grow unbounded
        // under header rotation or a noisy-peer attack.
        if now.duration_since(*last_sweep).as_secs() >= RATE_LIMIT_WINDOW_SECS {
            buckets.retain(|_, bucket| {
                now.duration_since(bucket.window_start).as_secs() < RATE_LIMIT_WINDOW_SECS
            });
            *last_sweep = now;
        }
        let bucket = buckets
            .entry(client_key.to_string())
            .or_insert_with(|| ClientBucket {
                count: 0,
                window_start: now,
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

/// Reads the trusted-proxy opt-in flag. When set (any of `"1"`, `"true"`,
/// `"yes"`, `"on"` case-insensitively), the rate-limit key derivation trusts
/// the `X-Forwarded-For` header chain. Otherwise, only the TCP peer address is
/// used. Header values are spoofable, so we never trust them by default.
fn trust_proxy() -> bool {
    static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FLAG.get_or_init(|| {
        matches!(
            std::env::var("OSM_WORLD_TRUST_PROXY")
                .ok()
                .map(|v| v.trim().to_ascii_lowercase())
                .as_deref(),
            Some("1" | "true" | "yes" | "on")
        )
    })
}

/// Derive the rate-limit key from the TCP peer address (preferred), optionally
/// falling back to the left-most `X-Forwarded-For` entry when `trust_proxy()`
/// is set. Returns the literal `"unknown"` only when no peer info is available
/// (e.g. tests that bypass the real `ConnectInfo` plumbing).
fn client_rate_limit_key(request: &axum::extract::Request) -> String {
    use std::net::SocketAddr;
    if trust_proxy()
        && let Some(forwarded) = request
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
        && let Some(first) = forwarded.split(',').next()
    {
        let trimmed = first.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(axum::extract::ConnectInfo(addr)) = request
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
    {
        return addr.ip().to_string();
    }
    "unknown".to_string()
}

/// Axum middleware that rate-limits requests based on client IP.
/// Applied to mutating endpoints that trigger expensive operations.
pub async fn rate_limit_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let client_key = client_rate_limit_key(&request);

    static LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();
    let limiter = LIMITER.get_or_init(RateLimiter::new);

    if let Err(response) = limiter.check(&client_key) {
        return response.into_response();
    }

    next.run(request).await
}
