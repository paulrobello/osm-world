use std::path::{Path, PathBuf};

use anyhow::Context;
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::task;
use tower_http::cors::CorsLayer;

const MAX_BBOX_SPAN_DEGREES: f64 = 0.5;
const MAX_BBOX_AREA_DEGREES: f64 = 0.10;
const MAX_SRTM_TILE_COUNT: usize = 16;

#[derive(Clone)]
struct AppState {
    project_root: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct PrepareAreaRequest {
    pub bbox: [f64; 4],
    #[serde(default)]
    pub filter: par_osm_rust::filter::FeatureFilter,
    #[serde(default)]
    pub use_elevation: bool,
    #[serde(default)]
    pub force_refresh: bool,
    pub overpass_url: Option<String>,
    pub spawn_lat: Option<f64>,
    pub spawn_lon: Option<f64>,
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
}

#[derive(Debug, Serialize)]
pub struct PrepareAreaResponse {
    pub bbox: [f64; 4],
    pub cache_key: String,
    pub cache_status: String,
    pub source_status: String,
    pub warnings: Vec<String>,
    pub osm_path: String,
    pub srtm_dir: Option<String>,
    pub spawn_lat: Option<f64>,
    pub spawn_lon: Option<f64>,
    pub command: String,
    pub command_cwd: String,
    pub command_program: String,
    pub command_args: Vec<String>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    overpass_cache_dir: String,
    srtm_cache_dir: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    client_message: &'static str,
}

impl ApiError {
    fn invalid_request(detail: impl std::fmt::Display) -> Self {
        log::warn!("invalid API request: {detail}");
        Self {
            status: StatusCode::BAD_REQUEST,
            client_message: "invalid request",
        }
    }

    fn internal(detail: impl std::fmt::Display) -> Self {
        log::error!("internal API error: {detail}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            client_message: "failed to prepare area",
        }
    }

    fn from_prepare_error(err: PrepareAreaError) -> Self {
        match err {
            PrepareAreaError::BadRequest { source } => {
                log::warn!("invalid prepare area request: {source:#}");
                Self {
                    status: StatusCode::BAD_REQUEST,
                    client_message: "invalid request",
                }
            }
            PrepareAreaError::Upstream {
                client_message,
                source,
            } => {
                log::warn!("{client_message}: {source:#}");
                Self {
                    status: StatusCode::BAD_GATEWAY,
                    client_message,
                }
            }
            PrepareAreaError::Internal {
                client_message,
                source,
            } => {
                log::error!("{client_message}: {source:#}");
                Self {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    client_message,
                }
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.client_message.to_string(),
            }),
        )
            .into_response()
    }
}

#[derive(Debug)]
pub(crate) enum PrepareAreaError {
    BadRequest {
        source: anyhow::Error,
    },
    Upstream {
        client_message: &'static str,
        source: anyhow::Error,
    },
    Internal {
        client_message: &'static str,
        source: anyhow::Error,
    },
}

impl PrepareAreaError {
    fn bad_request(source: anyhow::Error) -> Self {
        Self::BadRequest { source }
    }

    fn upstream(client_message: &'static str, source: anyhow::Error) -> Self {
        Self::Upstream {
            client_message,
            source,
        }
    }

    fn internal(client_message: &'static str, source: anyhow::Error) -> Self {
        Self::Internal {
            client_message,
            source,
        }
    }
}

type PrepareResult<T> = Result<T, PrepareAreaError>;

pub fn build_router(project_root: PathBuf) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/cache/areas", get(cache_areas))
        .route("/areas/prepare", post(prepare_area_handler))
        .fallback(not_found)
        .with_state(AppState { project_root })
        .layer(CorsLayer::permissive())
}

pub async fn run(host: &str, port: u16, project_root: PathBuf) -> anyhow::Result<()> {
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("binding HTTP server to {addr}"))?;
    log::info!("area prepare API listening on http://{addr}");
    axum::serve(listener, build_router(project_root))
        .await
        .context("running HTTP server")
}

async fn health() -> Result<Json<HealthResponse>, ApiError> {
    task::spawn_blocking(|| HealthResponse {
        status: "ok",
        overpass_cache_dir: path_string(par_osm_rust::cache::overpass_cache_dir()),
        srtm_cache_dir: path_string(par_osm_rust::cache::srtm_cache_dir()),
    })
    .await
    .map(Json)
    .map_err(|err| ApiError::internal(format!("health task failed: {err}")))
}

async fn cache_areas() -> Result<Json<Vec<par_osm_rust::osm_cache::CacheEntry>>, ApiError> {
    task::spawn_blocking(par_osm_rust::osm_cache::list_areas)
        .await
        .map(Json)
        .map_err(|err| ApiError::internal(format!("cache areas task failed: {err}")))
}

async fn prepare_area_handler(
    State(state): State<AppState>,
    payload: Result<Json<PrepareAreaRequest>, JsonRejection>,
) -> Result<Json<PrepareAreaResponse>, ApiError> {
    let Json(req) = payload.map_err(ApiError::invalid_request)?;
    let project_root = state.project_root;
    task::spawn_blocking(move || prepare_area(req, &project_root))
        .await
        .map_err(|err| ApiError::internal(format!("prepare task failed: {err}")))?
        .map(Json)
        .map_err(ApiError::from_prepare_error)
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "not found".to_string(),
        }),
    )
}

pub(crate) fn prepare_area(
    req: PrepareAreaRequest,
    project_root: &Path,
) -> PrepareResult<PrepareAreaResponse> {
    let bbox = validate_bbox(req.bbox)?;
    let spawn = validate_spawn(req.spawn_lat, req.spawn_lon, bbox)?;
    validate_filter(&req.filter)?;
    if let Some(url) = req.overpass_url.as_deref() {
        par_osm_rust::overpass::validate_overpass_url(url)
            .map_err(|err| PrepareAreaError::bad_request(err.context("validating Overpass URL")))?;
    }

    let requested_poi_source_mode = req
        .poi_source_mode
        .unwrap_or(par_osm_rust::sources::PoiSourceMode::OverturePreferred);
    let poi_source_mode = if req.overture {
        requested_poi_source_mode
    } else {
        par_osm_rust::sources::PoiSourceMode::OsmOnly
    };
    let failure_mode = req
        .overture_failure_mode
        .unwrap_or(par_osm_rust::sources::OvertureFailureMode::FallbackToOsm);
    let themes = if req.overture {
        parse_overture_themes_for_prepare(&req.overture_themes)?
    } else {
        Vec::new()
    };

    let cache_key = prepared_cache_key(
        bbox,
        &req.filter,
        req.overture,
        &req.overture_themes,
        poi_source_mode,
        failure_mode,
    );
    let cache_status = if req.force_refresh {
        "force_refreshed".to_string()
    } else {
        "prepared".to_string()
    };

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
    let mut progress_cb = |pct: f32, message: &str| {
        log::info!("preparing source data {:.0}%: {}", pct * 100.0, message);
    };
    let par_osm_rust::sources::SourceFetchResult {
        data,
        status,
        warnings,
    } = par_osm_rust::sources::fetch_map_data(bbox, &source_options, &mut progress_cb).map_err(
        |err| {
            PrepareAreaError::upstream(
                "failed to fetch map data",
                err.context("fetching map data from configured sources"),
            )
        },
    )?;
    let source_status = source_status_string(status);
    let xml = par_osm_rust::osm::write_osm_xml_string(&data);

    let prepared_dir = par_osm_rust::cache::shared_cache_root().join("prepared");
    std::fs::create_dir_all(&prepared_dir).map_err(|err| {
        PrepareAreaError::internal(
            "failed to prepare area",
            anyhow::Error::new(err).context(format!(
                "creating prepared cache dir {}",
                prepared_dir.display()
            )),
        )
    })?;
    let osm_path = prepared_dir.join(format!("{cache_key}.osm"));
    write_atomic(&osm_path, &xml)
        .map_err(|err| PrepareAreaError::internal("failed to prepare area", err))?;

    let srtm_dir = if req.use_elevation {
        validate_srtm_tile_limit(bbox)?;
        let srtm_dir = par_osm_rust::cache::srtm_cache_dir();
        par_osm_rust::srtm::download_tiles_for_bbox(
            bbox.0,
            bbox.1,
            bbox.2,
            bbox.3,
            &srtm_dir,
            &|index, total, tile| {
                log::info!("preparing SRTM tile {}/{}: {}", index + 1, total, tile);
            },
        )
        .map_err(|err| classify_srtm_error(err.context("downloading SRTM tiles")))?;
        Some(path_string(srtm_dir))
    } else {
        None
    };

    let osm_path = path_string(osm_path);
    let mut command_args = vec![
        "run".to_string(),
        "--manifest-path".to_string(),
        path_string(project_root.join("Cargo.toml")),
        "--".to_string(),
        "--input".to_string(),
        osm_path.clone(),
    ];
    if let Some((lat, lon)) = spawn {
        command_args.push("--spawn-lat".to_string());
        command_args.push(lat.to_string());
        command_args.push("--spawn-lon".to_string());
        command_args.push(lon.to_string());
    }
    if let Some(srtm_dir) = &srtm_dir {
        command_args.push("--srtm-dir".to_string());
        command_args.push(srtm_dir.clone());
    }
    let command_program = "cargo".to_string();
    let command = shell_command(&command_program, &command_args);

    Ok(PrepareAreaResponse {
        bbox: req.bbox,
        cache_key,
        cache_status,
        source_status,
        warnings,
        osm_path,
        srtm_dir,
        spawn_lat: spawn.map(|(lat, _)| lat),
        spawn_lon: spawn.map(|(_, lon)| lon),
        command,
        command_cwd: path_string(project_root),
        command_program,
        command_args,
    })
}

fn source_status_string(status: par_osm_rust::sources::SourceStatus) -> String {
    match status {
        par_osm_rust::sources::SourceStatus::OsmOnly => "osm_only",
        par_osm_rust::sources::SourceStatus::OvertureOnly => "overture_only",
        par_osm_rust::sources::SourceStatus::Both => "both",
        par_osm_rust::sources::SourceStatus::OverturePreferred => "overture_preferred",
        par_osm_rust::sources::SourceStatus::OvertureFallbackToOsm => "overture_fallback_to_osm",
    }
    .to_string()
}

fn validate_filter(filter: &par_osm_rust::filter::FeatureFilter) -> PrepareResult<()> {
    if !(filter.roads || filter.buildings || filter.water || filter.landuse || filter.railways) {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "all feature types are disabled"
        )));
    }
    Ok(())
}

fn parse_overture_themes_for_prepare(
    values: &[String],
) -> PrepareResult<Vec<par_osm_rust::overture::OvertureTheme>> {
    if values.is_empty() {
        return Ok(par_osm_rust::overture::OvertureTheme::all());
    }
    values
        .iter()
        .map(|value| {
            par_osm_rust::overture::OvertureTheme::from_str_loose(value).ok_or_else(|| {
                PrepareAreaError::bad_request(anyhow::anyhow!("unknown Overture theme '{value}'"))
            })
        })
        .collect()
}

fn validate_bbox(bbox: [f64; 4]) -> PrepareResult<(f64, f64, f64, f64)> {
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

fn validate_spawn(
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

fn validate_srtm_tile_limit(bbox: (f64, f64, f64, f64)) -> PrepareResult<()> {
    let tiles = par_osm_rust::srtm::tiles_for_bbox(bbox.0, bbox.1, bbox.2, bbox.3);
    if tiles.len() > MAX_SRTM_TILE_COUNT {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "requested bbox requires {} SRTM tiles; maximum is {MAX_SRTM_TILE_COUNT}",
            tiles.len()
        )));
    }
    Ok(())
}

fn classify_srtm_error(err: anyhow::Error) -> PrepareAreaError {
    let message = format!("{err:#}");
    if message.contains("Failed to write tmp file") || message.contains("Failed to rename") {
        PrepareAreaError::internal("failed to prepare area", err)
    } else {
        PrepareAreaError::upstream("failed to fetch elevation data", err)
    }
}

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

fn write_atomic(path: &Path, contents: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("creating parent dir {}", parent.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("path has no valid file name: {}", path.display()))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_nanos();
    let tmp_path = parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));
    std::fs::write(&tmp_path, contents)
        .with_context(|| format!("writing temp file {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, path)
        .with_context(|| format!("renaming {} to {}", tmp_path.display(), path.display()))?;
    Ok(())
}

fn path_string(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string()
}

fn shell_command(program: &str, args: &[String]) -> String {
    std::iter::once(shell_arg(program))
        .chain(args.iter().map(|arg| shell_arg(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_arg(value: &str) -> String {
    if value.bytes().all(|b| {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'@' | b'%' | b'_' | b'+' | b'=' | b':' | b',' | b'.' | b'/' | b'-'
            )
    }) {
        value.to_string()
    } else {
        shell_quote(value)
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, path::Path, sync::Mutex};

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    struct EnvRestore {
        vars: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvRestore {
        fn capture(names: &[&'static str]) -> Self {
            Self {
                vars: names
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (name, value) in &self.vars {
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
    }

    fn set_test_cache_env(tmp: &tempfile::TempDir) {
        let home = tmp.path().join("home");
        let overpass_dir = tmp.path().join("overpass");
        let srtm_dir = tmp.path().join("srtm");
        let overture_dir = tmp.path().join("overture");
        unsafe {
            std::env::set_var("HOME", &home);
            std::env::set_var("PAR_OSM_OVERPASS_CACHE_DIR", &overpass_dir);
            std::env::remove_var("OVERPASS_CACHE_DIR");
            std::env::set_var("PAR_OSM_SRTM_CACHE_DIR", &srtm_dir);
            std::env::remove_var("SRTM_CACHE_DIR");
            std::env::set_var("PAR_OSM_OVERTURE_CACHE_DIR", &overture_dir);
            std::env::remove_var("OVERTURE_CACHE_DIR");
        }
    }

    fn cache_xml_for_bbox(bbox: [f64; 4], filter: &par_osm_rust::filter::FeatureFilter) -> String {
        let cache_key =
            par_osm_rust::osm_cache::cache_key((bbox[0], bbox[1], bbox[2], bbox[3]), filter);
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
</osm>"#;
        par_osm_rust::osm_cache::write(
            &cache_key,
            (bbox[0], bbox[1], bbox[2], bbox[3]),
            filter,
            xml,
        )
        .unwrap();
        cache_key
    }

    fn cached_prepare_request(
        bbox: [f64; 4],
        filter: par_osm_rust::filter::FeatureFilter,
    ) -> PrepareAreaRequest {
        PrepareAreaRequest {
            bbox,
            filter,
            use_elevation: false,
            force_refresh: false,
            overpass_url: None,
            spawn_lat: None,
            spawn_lon: None,
            overture: false,
            overture_themes: Vec::new(),
            poi_source_mode: None,
            overture_failure_mode: None,
            overture_timeout: None,
        }
    }

    #[test]
    fn prepare_area_uses_exact_cached_xml_with_spawn_point() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let _restore = EnvRestore::capture(&[
            "HOME",
            "PAR_OSM_OVERPASS_CACHE_DIR",
            "OVERPASS_CACHE_DIR",
            "PAR_OSM_SRTM_CACHE_DIR",
            "SRTM_CACHE_DIR",
            "PAR_OSM_OVERTURE_CACHE_DIR",
            "OVERTURE_CACHE_DIR",
        ]);

        let tmp = tempfile::tempdir().unwrap();
        set_test_cache_env(&tmp);

        let bbox = [38.0, -121.0, 38.001, -120.999];
        let filter = par_osm_rust::filter::FeatureFilter::default();
        cache_xml_for_bbox(bbox, &filter);
        let cache_key = prepared_cache_key(
            (bbox[0], bbox[1], bbox[2], bbox[3]),
            &filter,
            false,
            &[],
            par_osm_rust::sources::PoiSourceMode::OsmOnly,
            par_osm_rust::sources::OvertureFailureMode::FallbackToOsm,
        );
        let mut req = cached_prepare_request(bbox, filter);
        req.spawn_lat = Some(38.0005);
        req.spawn_lon = Some(-120.9995);

        let response = prepare_area(req, tmp.path()).unwrap();

        assert_eq!(response.cache_key, cache_key);
        assert_eq!(response.cache_status, "prepared");
        assert_eq!(response.source_status, "osm_only");
        assert!(response.warnings.is_empty());
        assert_eq!(response.spawn_lat, Some(38.0005));
        assert_eq!(response.spawn_lon, Some(-120.9995));
        assert!(response.command_args.iter().any(|arg| arg == "--spawn-lat"));
        assert!(response.command_args.iter().any(|arg| arg == "38.0005"));
        assert!(response.command_args.iter().any(|arg| arg == "--spawn-lon"));
        assert!(response.command_args.iter().any(|arg| arg == "-120.9995"));
        let input_index = response
            .command_args
            .iter()
            .position(|arg| arg == "--input")
            .unwrap();
        assert_eq!(response.command_args[input_index + 2], "--spawn-lat");
        assert_eq!(response.command_args[input_index + 3], "38.0005");
        assert_eq!(response.command_args[input_index + 4], "--spawn-lon");
        assert_eq!(response.command_args[input_index + 5], "-120.9995");
        assert!(response.command.contains("--spawn-lat 38.0005"));
        assert!(response.command.contains("--spawn-lon -120.9995"));
    }

    #[test]
    fn prepare_area_uses_exact_cached_xml_without_elevation() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let _restore = EnvRestore::capture(&[
            "HOME",
            "PAR_OSM_OVERPASS_CACHE_DIR",
            "OVERPASS_CACHE_DIR",
            "PAR_OSM_SRTM_CACHE_DIR",
            "SRTM_CACHE_DIR",
            "PAR_OSM_OVERTURE_CACHE_DIR",
            "OVERTURE_CACHE_DIR",
        ]);

        let tmp = tempfile::tempdir().unwrap();
        set_test_cache_env(&tmp);

        let bbox = [38.0, -121.0, 38.001, -120.999];
        let filter = par_osm_rust::filter::FeatureFilter::default();
        cache_xml_for_bbox(bbox, &filter);
        let cache_key = prepared_cache_key(
            (bbox[0], bbox[1], bbox[2], bbox[3]),
            &filter,
            false,
            &[],
            par_osm_rust::sources::PoiSourceMode::OsmOnly,
            par_osm_rust::sources::OvertureFailureMode::FallbackToOsm,
        );

        let response = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();

        assert_eq!(response.cache_key, cache_key);
        assert_eq!(response.cache_status, "prepared");
        assert_eq!(response.source_status, "osm_only");
        assert!(response.warnings.is_empty());
        assert!(response.osm_path.ends_with(".osm"));
        assert!(Path::new(&response.osm_path).exists());
        assert_eq!(
            std::fs::read_to_string(&response.osm_path).unwrap(),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<osm version=\"0.6\">\n  <bounds minlat=\"38\" minlon=\"-121\" maxlat=\"38.001\" maxlon=\"-120.999\"/>\n</osm>\n"
        );
        assert!(response.srtm_dir.is_none());
        assert!(response.command.contains("--manifest-path"));
        assert!(response.command.contains("--input"));
        assert!(response.command.contains(&response.osm_path));
        assert!(!response.command.contains("--srtm-dir"));
        assert_eq!(response.command_cwd, path_string(tmp.path()));
        assert_eq!(response.command_program, "cargo");
        assert!(response.command_args.iter().any(|arg| arg == "--input"));
        assert!(
            response
                .command_args
                .iter()
                .any(|arg| arg == &response.osm_path)
        );
        assert!(!response.command_args.iter().any(|arg| arg == "--srtm-dir"));
    }

    #[test]
    fn command_quotes_project_root_with_spaces_and_quotes_and_preserves_structured_args() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let _restore = EnvRestore::capture(&[
            "HOME",
            "PAR_OSM_OVERPASS_CACHE_DIR",
            "OVERPASS_CACHE_DIR",
            "PAR_OSM_SRTM_CACHE_DIR",
            "SRTM_CACHE_DIR",
            "PAR_OSM_OVERTURE_CACHE_DIR",
            "OVERTURE_CACHE_DIR",
        ]);

        let tmp = tempfile::tempdir().unwrap();
        set_test_cache_env(&tmp);

        let project_root = tmp.path().join("project root with 'quote'");
        let bbox = [38.0, -121.0, 38.001, -120.999];
        let filter = par_osm_rust::filter::FeatureFilter::default();
        cache_xml_for_bbox(bbox, &filter);

        let response = prepare_area(cached_prepare_request(bbox, filter), &project_root).unwrap();
        let manifest_path = path_string(project_root.join("Cargo.toml"));

        assert_eq!(response.command_cwd, path_string(&project_root));
        assert_eq!(response.command_program, "cargo");
        assert_eq!(response.command_args[0], "run");
        assert_eq!(response.command_args[1], "--manifest-path");
        assert_eq!(response.command_args[2], manifest_path);
        assert!(response.command.contains(&shell_quote(&manifest_path)));
        assert!(response.command.contains(" -- --input "));
    }

    #[test]
    fn prepare_area_reports_overture_fallback_warning() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let _restore = EnvRestore::capture(&[
            "HOME",
            "PATH",
            "PAR_OSM_OVERPASS_CACHE_DIR",
            "OVERPASS_CACHE_DIR",
            "PAR_OSM_SRTM_CACHE_DIR",
            "SRTM_CACHE_DIR",
            "PAR_OSM_OVERTURE_CACHE_DIR",
            "OVERTURE_CACHE_DIR",
        ]);
        let tmp = tempfile::tempdir().unwrap();
        set_test_cache_env(&tmp);
        unsafe {
            std::env::set_var("PATH", tmp.path().join("empty-path"));
        }

        let bbox = [38.0, -121.0, 38.001, -120.999];
        let filter = par_osm_rust::filter::FeatureFilter::default();
        cache_xml_for_bbox(bbox, &filter);
        let mut req = cached_prepare_request(bbox, filter);
        req.overture = true;
        req.poi_source_mode = Some(par_osm_rust::sources::PoiSourceMode::OverturePreferred);
        req.overture_failure_mode = Some(par_osm_rust::sources::OvertureFailureMode::FallbackToOsm);
        req.overture_timeout = Some(1);

        let response = prepare_area(req, tmp.path()).unwrap();

        assert_eq!(response.source_status, "overture_fallback_to_osm");
        assert!(
            response
                .warnings
                .iter()
                .any(|warning| warning.contains("Overture"))
        );
        assert!(Path::new(&response.osm_path).exists());
    }

    #[test]
    fn validate_spawn_rejects_invalid_spawn_points() {
        let bbox = (38.0, -121.0, 38.001, -120.999);

        assert!(validate_spawn(None, None, bbox).unwrap().is_none());
        assert!(matches!(
            validate_spawn(Some(38.0005), None, bbox),
            Err(PrepareAreaError::BadRequest { .. })
        ));
        assert!(matches!(
            validate_spawn(None, Some(-120.9995), bbox),
            Err(PrepareAreaError::BadRequest { .. })
        ));

        for (lat, lon) in [
            (f64::NAN, -120.9995),
            (f64::INFINITY, -120.9995),
            (90.1, -120.9995),
            (-90.1, -120.9995),
            (38.0005, f64::NAN),
            (38.0005, f64::INFINITY),
            (38.0005, 180.1),
            (38.0005, -180.1),
            (37.9999, -120.9995),
            (38.0011, -120.9995),
            (38.0005, -121.0001),
            (38.0005, -120.9989),
        ] {
            assert!(
                matches!(
                    validate_spawn(Some(lat), Some(lon), bbox),
                    Err(PrepareAreaError::BadRequest { .. })
                ),
                "expected spawn ({lat}, {lon}) to be rejected"
            );
        }
    }

    #[test]
    fn validate_bbox_rejects_huge_bbox() {
        let err = validate_bbox([38.0, -121.0, 38.51, -120.99]).unwrap_err();

        assert!(matches!(err, PrepareAreaError::BadRequest { .. }));
    }

    #[test]
    fn validate_srtm_tile_limit_rejects_too_many_tiles_before_download() {
        let err = validate_srtm_tile_limit((-4.1, -4.1, 0.1, 0.1)).unwrap_err();

        assert!(matches!(err, PrepareAreaError::BadRequest { .. }));
    }
}
