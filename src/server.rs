use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Context;
use axum::{
    Json, Router,
    extract::{Path as AxumPath, State, rejection::JsonRejection},
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
const OVERTURE_FALLBACK_TO_OSM_STATUS: &str = "overture_fallback_to_osm";

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

#[derive(Debug, Clone, Serialize)]
pub struct PreparedAreaEntry {
    pub cache_key: String,
    pub display_name: Option<String>,
    pub favorite: bool,
    pub bbox: [f64; 4],
    pub filter: par_osm_rust::filter::FeatureFilter,
    pub use_elevation: bool,
    pub overture: bool,
    pub overture_themes: Vec<String>,
    pub poi_source_mode: Option<par_osm_rust::sources::PoiSourceMode>,
    pub overture_failure_mode: Option<par_osm_rust::sources::OvertureFailureMode>,
    pub overture_timeout: Option<u64>,
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

#[derive(Debug, Deserialize)]
pub struct PreparedAreaUpdate {
    pub display_name: Option<String>,
    pub favorite: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct DeletePreparedAreaResponse {
    pub status: &'static str,
    pub cache_key: String,
}

#[derive(Debug, Deserialize)]
pub struct LaunchRendererRequest {
    pub osm_path: String,
    pub srtm_dir: Option<String>,
    pub spawn_lat: Option<f64>,
    pub spawn_lon: Option<f64>,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RendererLaunchCommand {
    pub program: String,
    pub args: Vec<String>,
    pub command: String,
    pub command_cwd: String,
}

#[derive(Debug, Serialize)]
struct LaunchRendererResponse {
    status: &'static str,
    pid: u32,
    #[serde(flatten)]
    command: RendererLaunchCommand,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PreparedCacheMetadata {
    source_status: String,
    warnings: Vec<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    favorite: bool,
    #[serde(default)]
    bbox: Option<[f64; 4]>,
    #[serde(default)]
    filter: Option<par_osm_rust::filter::FeatureFilter>,
    #[serde(default)]
    use_elevation: bool,
    #[serde(default)]
    overture: bool,
    #[serde(default)]
    overture_themes: Vec<String>,
    #[serde(default)]
    poi_source_mode: Option<par_osm_rust::sources::PoiSourceMode>,
    #[serde(default)]
    overture_failure_mode: Option<par_osm_rust::sources::OvertureFailureMode>,
    #[serde(default)]
    overture_timeout: Option<u64>,
    #[serde(default)]
    srtm_dir: Option<String>,
    #[serde(default)]
    spawn_lat: Option<f64>,
    #[serde(default)]
    spawn_lon: Option<f64>,
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
        .route("/areas/prepared", get(prepared_areas_handler))
        .route(
            "/areas/prepared/{cache_key}",
            post(update_prepared_area_handler).delete(delete_prepared_area_handler),
        )
        .route("/areas/prepare", post(prepare_area_handler))
        .route("/renderer/launch", post(launch_renderer_handler))
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

async fn prepared_areas_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<PreparedAreaEntry>>, ApiError> {
    let project_root = state.project_root;
    task::spawn_blocking(move || list_prepared_areas(&project_root))
        .await
        .map_err(|err| ApiError::internal(format!("prepared areas task failed: {err}")))?
        .map(Json)
        .map_err(ApiError::from_prepare_error)
}

async fn update_prepared_area_handler(
    State(state): State<AppState>,
    AxumPath(cache_key): AxumPath<String>,
    payload: Result<Json<PreparedAreaUpdate>, JsonRejection>,
) -> Result<Json<PreparedAreaEntry>, ApiError> {
    let Json(update) = payload.map_err(ApiError::invalid_request)?;
    let project_root = state.project_root;
    task::spawn_blocking(move || update_prepared_area_details(&cache_key, update, &project_root))
        .await
        .map_err(|err| ApiError::internal(format!("prepared area update task failed: {err}")))?
        .map(Json)
        .map_err(ApiError::from_prepare_error)
}

async fn delete_prepared_area_handler(
    AxumPath(cache_key): AxumPath<String>,
) -> Result<Json<DeletePreparedAreaResponse>, ApiError> {
    task::spawn_blocking(move || delete_prepared_area(&cache_key))
        .await
        .map_err(|err| ApiError::internal(format!("prepared area delete task failed: {err}")))?
        .map(Json)
        .map_err(ApiError::from_prepare_error)
}

async fn launch_renderer_handler(
    State(state): State<AppState>,
    payload: Result<Json<LaunchRendererRequest>, JsonRejection>,
) -> Result<Json<LaunchRendererResponse>, ApiError> {
    let Json(req) = payload.map_err(ApiError::invalid_request)?;
    let project_root = state.project_root;
    task::spawn_blocking(move || launch_renderer(&project_root, &req))
        .await
        .map_err(|err| ApiError::internal(format!("renderer launch task failed: {err}")))?
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
    let effective_overpass_url = effective_overpass_url_for_prepare(req.overpass_url.as_deref())?;

    let requested_poi_source_mode = req
        .poi_source_mode
        .unwrap_or(par_osm_rust::sources::PoiSourceMode::OverturePreferred);
    let poi_source_mode = if req.overture {
        requested_poi_source_mode
    } else {
        par_osm_rust::sources::PoiSourceMode::OsmOnly
    };
    let failure_mode = if req.overture {
        req.overture_failure_mode
            .unwrap_or(par_osm_rust::sources::OvertureFailureMode::FallbackToOsm)
    } else {
        par_osm_rust::sources::OvertureFailureMode::FallbackToOsm
    };
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
        &effective_overpass_url,
    );
    let prepared_dir = par_osm_rust::cache::shared_cache_root().join("prepared");
    let osm_path = prepared_dir.join(format!("{cache_key}.osm"));
    let metadata_path = prepared_metadata_path(&osm_path);

    let prepared_cache_hit = if !req.force_refresh && osm_path.exists() {
        let (source_status, warnings) = read_prepared_cache_metadata(&metadata_path);
        if is_degraded_overture_prepared_cache(req.overture, &source_status) {
            None
        } else {
            Some((source_status, warnings))
        }
    } else {
        None
    };

    let (cache_status, source_status, warnings) =
        if let Some((source_status, warnings)) = prepared_cache_hit {
            ("prepared_cache_hit".to_string(), source_status, warnings)
        } else {
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
            } = par_osm_rust::sources::fetch_map_data(bbox, &source_options, &mut progress_cb)
                .map_err(|err| {
                    PrepareAreaError::upstream(
                        "failed to fetch map data",
                        err.context("fetching map data from configured sources"),
                    )
                })?;
            let source_status = source_status_string(status);
            let xml = par_osm_rust::osm::write_osm_xml_string(&data);

            std::fs::create_dir_all(&prepared_dir).map_err(|err| {
                PrepareAreaError::internal(
                    "failed to prepare area",
                    anyhow::Error::new(err).context(format!(
                        "creating prepared cache dir {}",
                        prepared_dir.display()
                    )),
                )
            })?;
            write_atomic(&osm_path, &xml)
                .map_err(|err| PrepareAreaError::internal("failed to prepare area", err))?;

            let cache_status = if req.force_refresh {
                "force_refreshed".to_string()
            } else {
                "prepared".to_string()
            };
            (cache_status, source_status, warnings)
        };

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
    let spawn_lat = spawn.map(|(lat, _)| lat);
    let spawn_lon = spawn.map(|(_, lon)| lon);
    let existing_metadata = read_prepared_cache_metadata_struct(&metadata_path).ok();
    let metadata = PreparedCacheMetadata {
        source_status: source_status.clone(),
        warnings: warnings.clone(),
        display_name: existing_metadata
            .as_ref()
            .and_then(|metadata| metadata.display_name.clone()),
        favorite: existing_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.favorite),
        bbox: Some(req.bbox),
        filter: Some(req.filter.clone()),
        use_elevation: req.use_elevation,
        overture: req.overture,
        overture_themes: req.overture_themes.clone(),
        poi_source_mode: req.poi_source_mode,
        overture_failure_mode: req.overture_failure_mode,
        overture_timeout: req.overture_timeout,
        srtm_dir: srtm_dir.clone(),
        spawn_lat,
        spawn_lon,
    };
    write_prepared_cache_metadata(&metadata_path, &metadata)
        .map_err(|err| PrepareAreaError::internal("failed to prepare area", err))?;

    let launch_req = LaunchRendererRequest {
        osm_path: osm_path.clone(),
        srtm_dir: srtm_dir.clone(),
        spawn_lat,
        spawn_lon,
        extra_args: Vec::new(),
    };
    let launch_command = renderer_launch_command(project_root, &launch_req)?;

    Ok(PrepareAreaResponse {
        bbox: req.bbox,
        cache_key,
        cache_status,
        source_status,
        warnings,
        osm_path,
        srtm_dir,
        spawn_lat,
        spawn_lon,
        command: launch_command.command,
        command_cwd: launch_command.command_cwd,
        command_program: launch_command.program,
        command_args: launch_command.args,
    })
}

pub(crate) fn list_prepared_areas(project_root: &Path) -> PrepareResult<Vec<PreparedAreaEntry>> {
    let prepared_dir = prepared_area_dir();
    let read_dir = match std::fs::read_dir(&prepared_dir) {
        Ok(read_dir) => read_dir,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(PrepareAreaError::internal(
                "failed to prepare area",
                anyhow::Error::new(err).context(format!(
                    "reading prepared cache dir {}",
                    prepared_dir.display()
                )),
            ));
        }
    };
    let entries = read_dir
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "osm"))
        .filter_map(|entry| prepared_entry_from_osm_path(project_root, &entry.path()).ok())
        .collect::<Vec<_>>();

    let mut entries = entries;
    entries.sort_by(|left, right| {
        right
            .favorite
            .cmp(&left.favorite)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.cache_key.cmp(&right.cache_key))
    });
    Ok(entries)
}

pub(crate) fn update_prepared_area_details(
    cache_key: &str,
    update: PreparedAreaUpdate,
    project_root: &Path,
) -> PrepareResult<PreparedAreaEntry> {
    validate_cache_key(cache_key)?;
    let osm_path = prepared_area_dir().join(format!("{cache_key}.osm"));
    if !osm_path.exists() {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "unknown prepared area cache key"
        )));
    }
    let metadata_path = prepared_metadata_path(&osm_path);
    let mut metadata = read_prepared_cache_metadata_struct(&metadata_path)
        .map_err(|message| PrepareAreaError::bad_request(anyhow::anyhow!(message)))?;
    if let Some(display_name) = update.display_name {
        let trimmed = display_name.trim();
        metadata.display_name = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    if let Some(favorite) = update.favorite {
        metadata.favorite = favorite;
    }
    write_prepared_cache_metadata(&metadata_path, &metadata)
        .map_err(|err| PrepareAreaError::internal("failed to prepare area", err))?;
    prepared_entry_from_osm_path(project_root, &osm_path)
}

pub(crate) fn delete_prepared_area(cache_key: &str) -> PrepareResult<DeletePreparedAreaResponse> {
    validate_cache_key(cache_key)?;
    let osm_path = prepared_area_dir().join(format!("{cache_key}.osm"));
    if !osm_path.exists() {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "unknown prepared area cache key"
        )));
    }
    let metadata_path = prepared_metadata_path(&osm_path);

    std::fs::remove_file(&osm_path).map_err(|err| {
        PrepareAreaError::internal(
            "failed to prepare area",
            anyhow::Error::new(err).context(format!("removing {}", osm_path.display())),
        )
    })?;
    match std::fs::remove_file(&metadata_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(PrepareAreaError::internal(
                "failed to prepare area",
                anyhow::Error::new(err).context(format!("removing {}", metadata_path.display())),
            ));
        }
    }

    Ok(DeletePreparedAreaResponse {
        status: "deleted",
        cache_key: cache_key.to_string(),
    })
}

fn prepared_entry_from_osm_path(
    project_root: &Path,
    osm_path: &Path,
) -> PrepareResult<PreparedAreaEntry> {
    let cache_key = osm_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            PrepareAreaError::bad_request(anyhow::anyhow!("invalid prepared cache key"))
        })?
        .to_string();
    let metadata = read_prepared_cache_metadata_struct(&prepared_metadata_path(osm_path))
        .map_err(|message| PrepareAreaError::bad_request(anyhow::anyhow!(message)))?;
    let bbox = metadata.bbox.ok_or_else(|| {
        PrepareAreaError::bad_request(anyhow::anyhow!("prepared area metadata missing bbox"))
    })?;
    let filter = metadata.filter.clone().ok_or_else(|| {
        PrepareAreaError::bad_request(anyhow::anyhow!(
            "prepared area metadata missing feature filter"
        ))
    })?;
    let launch_req = LaunchRendererRequest {
        osm_path: path_string(osm_path),
        srtm_dir: metadata.srtm_dir.clone(),
        spawn_lat: metadata.spawn_lat,
        spawn_lon: metadata.spawn_lon,
        extra_args: Vec::new(),
    };
    let launch_command = renderer_launch_command(project_root, &launch_req)?;

    Ok(PreparedAreaEntry {
        cache_key,
        display_name: metadata.display_name,
        favorite: metadata.favorite,
        bbox,
        filter,
        use_elevation: metadata.use_elevation,
        overture: metadata.overture,
        overture_themes: metadata.overture_themes,
        poi_source_mode: metadata.poi_source_mode,
        overture_failure_mode: metadata.overture_failure_mode,
        overture_timeout: metadata.overture_timeout,
        source_status: metadata.source_status,
        warnings: metadata.warnings,
        osm_path: launch_req.osm_path,
        srtm_dir: launch_req.srtm_dir,
        spawn_lat: launch_req.spawn_lat,
        spawn_lon: launch_req.spawn_lon,
        command: launch_command.command,
        command_cwd: launch_command.command_cwd,
        command_program: launch_command.program,
        command_args: launch_command.args,
    })
}

fn validate_cache_key(cache_key: &str) -> PrepareResult<()> {
    if cache_key.len() == 64 && cache_key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "invalid prepared area cache key"
        )))
    }
}

fn launch_renderer(
    project_root: &Path,
    req: &LaunchRendererRequest,
) -> PrepareResult<LaunchRendererResponse> {
    let command = renderer_launch_command(project_root, req)?;
    let child = Command::new(&command.program)
        .args(&command.args)
        .current_dir(project_root)
        .spawn()
        .map_err(|err| {
            PrepareAreaError::internal(
                "failed to launch renderer",
                anyhow::Error::new(err).context("spawning renderer process"),
            )
        })?;

    Ok(LaunchRendererResponse {
        status: "launched",
        pid: child.id(),
        command,
    })
}

pub(crate) fn renderer_launch_command(
    project_root: &Path,
    req: &LaunchRendererRequest,
) -> PrepareResult<RendererLaunchCommand> {
    validate_spawn(req.spawn_lat, req.spawn_lon, (-90.0, -180.0, 90.0, 180.0))?;
    let osm_path = Path::new(&req.osm_path);
    if osm_path.extension().and_then(|ext| ext.to_str()) != Some("osm") {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "renderer launch requires a prepared .osm file"
        )));
    }

    let mut args = vec![
        "run".to_string(),
        "--manifest-path".to_string(),
        path_string(project_root.join("Cargo.toml")),
        "--".to_string(),
        "--input".to_string(),
        req.osm_path.clone(),
    ];
    if let Some((lat, lon)) = req.spawn_lat.zip(req.spawn_lon) {
        args.push("--spawn-lat".to_string());
        args.push(lat.to_string());
        args.push("--spawn-lon".to_string());
        args.push(lon.to_string());
    }
    if let Some(srtm_dir) = &req.srtm_dir {
        if !srtm_dir.trim().is_empty() {
            args.push("--srtm-dir".to_string());
            args.push(srtm_dir.clone());
        }
    }
    args.extend(req.extra_args.clone());
    let program = "cargo".to_string();
    let command = shell_command(&program, &args);
    Ok(RendererLaunchCommand {
        program,
        args,
        command,
        command_cwd: path_string(project_root),
    })
}

fn prepared_area_dir() -> PathBuf {
    par_osm_rust::cache::shared_cache_root().join("prepared")
}

fn source_status_string(status: par_osm_rust::sources::SourceStatus) -> String {
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

fn is_degraded_overture_prepared_cache(overture_enabled: bool, source_status: &str) -> bool {
    overture_enabled && source_status == OVERTURE_FALLBACK_TO_OSM_STATUS
}

fn effective_overpass_url_for_prepare(overpass_url: Option<&str>) -> PrepareResult<String> {
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

fn prepared_metadata_path(osm_path: &Path) -> PathBuf {
    osm_path.with_extension("meta.json")
}

fn read_prepared_cache_metadata(metadata_path: &Path) -> (String, Vec<String>) {
    match read_prepared_cache_metadata_struct(metadata_path) {
        Ok(metadata) => (metadata.source_status, metadata.warnings),
        Err(message) => ("cached_unknown".to_string(), vec![message]),
    }
}

fn read_prepared_cache_metadata_struct(
    metadata_path: &Path,
) -> Result<PreparedCacheMetadata, String> {
    match std::fs::read_to_string(metadata_path) {
        Ok(contents) => serde_json::from_str::<PreparedCacheMetadata>(&contents).map_err(|err| {
            format!(
                "prepared cache metadata unreadable at {}; source status unknown: {err}",
                metadata_path.display()
            )
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "prepared cache metadata missing at {}; source status unknown",
            metadata_path.display()
        )),
        Err(err) => Err(format!(
            "prepared cache metadata unreadable at {}; source status unknown: {err}",
            metadata_path.display()
        )),
    }
}

fn write_prepared_cache_metadata(
    metadata_path: &Path,
    metadata: &PreparedCacheMetadata,
) -> anyhow::Result<()> {
    let contents =
        serde_json::to_string_pretty(metadata).context("serializing prepared cache metadata")?;
    write_atomic(metadata_path, &(contents + "\n")).with_context(|| {
        format!(
            "writing prepared cache metadata {}",
            metadata_path.display()
        )
    })
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

fn canonicalize_overture_themes(
    themes: Vec<par_osm_rust::overture::OvertureTheme>,
) -> Vec<par_osm_rust::overture::OvertureTheme> {
    let mut themes_by_name = std::collections::BTreeMap::new();
    for theme in themes {
        themes_by_name.entry(theme.to_string()).or_insert(theme);
    }
    themes_by_name.into_values().collect()
}

fn canonical_overture_theme_names_for_key(values: &[String]) -> Vec<String> {
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
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
</osm>"#;
        cache_xml_for_bbox_with_xml(bbox, filter, xml)
    }

    fn cache_xml_for_bbox_with_xml(
        bbox: [f64; 4],
        filter: &par_osm_rust::filter::FeatureFilter,
        xml: &str,
    ) -> String {
        let overpass_url = par_osm_rust::overpass::default_overpass_url();
        cache_xml_for_bbox_with_xml_and_overpass_url(bbox, filter, xml, overpass_url)
    }

    fn cache_xml_for_bbox_with_overpass_url(
        bbox: [f64; 4],
        filter: &par_osm_rust::filter::FeatureFilter,
        overpass_url: &str,
    ) -> String {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
</osm>"#;
        cache_xml_for_bbox_with_xml_and_overpass_url(bbox, filter, xml, overpass_url)
    }

    fn cache_xml_for_bbox_with_xml_and_overpass_url(
        bbox: [f64; 4],
        filter: &par_osm_rust::filter::FeatureFilter,
        xml: &str,
        overpass_url: &str,
    ) -> String {
        let bbox_tuple = (bbox[0], bbox[1], bbox[2], bbox[3]);
        let cache_key =
            par_osm_rust::osm_cache::cache_key_for_url(bbox_tuple, filter, overpass_url);
        par_osm_rust::osm_cache::write_for_url(&cache_key, bbox_tuple, filter, xml, overpass_url)
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
            par_osm_rust::overpass::default_overpass_url(),
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
    fn prepare_area_writes_prepared_history_metadata_for_reopen() {
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
        let filter = par_osm_rust::filter::FeatureFilter {
            roads: true,
            buildings: false,
            water: true,
            landuse: false,
            railways: true,
        };
        cache_xml_for_bbox(bbox, &filter);
        let mut req = cached_prepare_request(bbox, filter.clone());
        req.spawn_lat = Some(38.0005);
        req.spawn_lon = Some(-120.9995);

        let response = prepare_area(req, tmp.path()).unwrap();
        let entries = list_prepared_areas(tmp.path()).unwrap();

        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.cache_key, response.cache_key);
        assert_eq!(entry.bbox, bbox);
        assert_eq!(entry.filter, filter);
        assert_eq!(entry.spawn_lat, Some(38.0005));
        assert_eq!(entry.spawn_lon, Some(-120.9995));
        assert_eq!(entry.source_status, "osm_only");
        assert_eq!(entry.osm_path, response.osm_path);
        assert_eq!(entry.command, response.command);
        assert_eq!(entry.display_name, None);
        assert!(!entry.favorite);
    }

    #[test]
    fn update_prepared_area_details_persists_name_and_favorite() {
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
        let response = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();

        let updated = update_prepared_area_details(
            &response.cache_key,
            PreparedAreaUpdate {
                display_name: Some("Downtown smoke test".to_string()),
                favorite: Some(true),
            },
            tmp.path(),
        )
        .unwrap();
        let listed = list_prepared_areas(tmp.path()).unwrap();

        assert_eq!(updated.display_name.as_deref(), Some("Downtown smoke test"));
        assert!(updated.favorite);
        assert_eq!(
            listed[0].display_name.as_deref(),
            Some("Downtown smoke test")
        );
        assert!(listed[0].favorite);
    }

    #[test]
    fn delete_prepared_area_removes_osm_and_metadata() {
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
        let response = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();
        let osm_path = Path::new(&response.osm_path).to_path_buf();
        let metadata_path = osm_path.with_extension("meta.json");

        let deleted = delete_prepared_area(&response.cache_key).unwrap();
        let listed = list_prepared_areas(tmp.path()).unwrap();

        assert_eq!(deleted.cache_key, response.cache_key);
        assert_eq!(deleted.status, "deleted");
        assert!(!osm_path.exists());
        assert!(!metadata_path.exists());
        assert!(listed.is_empty());
    }

    #[test]
    fn renderer_launch_command_uses_prepared_file_and_optional_runtime_flags() {
        let project_root = Path::new("/tmp/osm world");
        let req = LaunchRendererRequest {
            osm_path: "/tmp/prepared/city.osm".to_string(),
            srtm_dir: Some("/tmp/srtm cache".to_string()),
            spawn_lat: Some(38.0005),
            spawn_lon: Some(-120.9995),
            extra_args: vec!["--visual-preset".to_string(), "showcase".to_string()],
        };

        let command = renderer_launch_command(project_root, &req).unwrap();

        assert_eq!(command.program, "cargo");
        assert_eq!(command.args[0], "run");
        assert_eq!(command.args[1], "--manifest-path");
        assert_eq!(command.args[2], "/tmp/osm world/Cargo.toml");
        assert!(
            command
                .args
                .windows(2)
                .any(|window| window == ["--input", "/tmp/prepared/city.osm"])
        );
        assert!(
            command
                .args
                .windows(2)
                .any(|window| window == ["--srtm-dir", "/tmp/srtm cache"])
        );
        assert!(
            command
                .args
                .windows(2)
                .any(|window| window == ["--spawn-lat", "38.0005"])
        );
        assert!(
            command
                .args
                .windows(2)
                .any(|window| window == ["--spawn-lon", "-120.9995"])
        );
        assert!(
            command
                .args
                .windows(2)
                .any(|window| window == ["--visual-preset", "showcase"])
        );
        assert!(command.command.contains("'/tmp/osm world/Cargo.toml'"));
    }

    #[test]
    fn prepared_cache_key_ignores_overture_options_when_overture_disabled() {
        let bbox = (38.0, -121.0, 38.001, -120.999);
        let filter = par_osm_rust::filter::FeatureFilter::default();

        let default_key = prepared_cache_key(
            bbox,
            &filter,
            false,
            &[],
            par_osm_rust::sources::PoiSourceMode::OsmOnly,
            par_osm_rust::sources::OvertureFailureMode::FallbackToOsm,
            par_osm_rust::overpass::default_overpass_url(),
        );
        let noisy_key = prepared_cache_key(
            bbox,
            &filter,
            false,
            &["not-a-theme".to_string(), "places".to_string()],
            par_osm_rust::sources::PoiSourceMode::OverturePreferred,
            par_osm_rust::sources::OvertureFailureMode::Fail,
            par_osm_rust::overpass::default_overpass_url(),
        );

        assert_eq!(noisy_key, default_key);
    }

    #[test]
    fn prepared_cache_key_canonicalizes_overture_theme_aliases_order_and_all_default() {
        let bbox = (38.0, -121.0, 38.001, -120.999);
        let filter = par_osm_rust::filter::FeatureFilter::default();
        let mode = par_osm_rust::sources::PoiSourceMode::OverturePreferred;
        let failure = par_osm_rust::sources::OvertureFailureMode::FallbackToOsm;

        let alias_key = prepared_cache_key(
            bbox,
            &filter,
            true,
            &[
                "places".to_string(),
                "BUILDING".to_string(),
                "place".to_string(),
            ],
            mode,
            failure,
            par_osm_rust::overpass::default_overpass_url(),
        );
        let canonical_key = prepared_cache_key(
            bbox,
            &filter,
            true,
            &["building".to_string(), "place".to_string()],
            mode,
            failure,
            par_osm_rust::overpass::default_overpass_url(),
        );
        assert_eq!(alias_key, canonical_key);

        let default_all_key = prepared_cache_key(
            bbox,
            &filter,
            true,
            &[],
            mode,
            failure,
            par_osm_rust::overpass::default_overpass_url(),
        );
        let explicit_all_key = prepared_cache_key(
            bbox,
            &filter,
            true,
            &[
                "address".to_string(),
                "base".to_string(),
                "building".to_string(),
                "place".to_string(),
                "transportation".to_string(),
            ],
            mode,
            failure,
            par_osm_rust::overpass::default_overpass_url(),
        );
        assert_eq!(default_all_key, explicit_all_key);
    }

    #[test]
    fn prepare_area_reuses_existing_prepared_file_on_second_call() {
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
        let source_cache_key = cache_xml_for_bbox(bbox, &filter);

        let first = prepare_area(cached_prepare_request(bbox, filter.clone()), tmp.path()).unwrap();
        std::fs::write(
            par_osm_rust::cache::overpass_cache_dir().join(format!("{source_cache_key}.xml")),
            "not valid osm xml",
        )
        .unwrap();
        let second = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();

        assert_eq!(first.cache_status, "prepared");
        assert_eq!(second.cache_status, "prepared_cache_hit");
        assert_eq!(second.cache_key, first.cache_key);
        assert_eq!(second.osm_path, first.osm_path);
        assert_eq!(second.source_status, "osm_only");
        assert!(second.warnings.is_empty());
    }

    #[test]
    fn prepare_area_missing_prepared_metadata_returns_conservative_cache_hit() {
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

        let first = prepare_area(cached_prepare_request(bbox, filter.clone()), tmp.path()).unwrap();
        std::fs::remove_file(Path::new(&first.osm_path).with_extension("meta.json")).unwrap();
        let second = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();

        assert_eq!(second.cache_status, "prepared_cache_hit");
        assert_eq!(second.source_status, "cached_unknown");
        assert!(
            second
                .warnings
                .iter()
                .any(|warning| warning.contains("metadata") && warning.contains("missing")),
            "expected missing metadata warning, got {:?}",
            second.warnings
        );
    }

    #[test]
    fn prepare_area_includes_effective_overpass_url_in_prepared_cache_key() {
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

        let default_response =
            prepare_area(cached_prepare_request(bbox, filter.clone()), tmp.path()).unwrap();
        let custom_overpass_url = "https://overpass.kumi.systems/api/interpreter";
        cache_xml_for_bbox_with_overpass_url(bbox, &filter, custom_overpass_url);
        let mut custom_req = cached_prepare_request(bbox, filter);
        custom_req.overpass_url = Some(custom_overpass_url.to_string());
        let custom_response = prepare_area(custom_req, tmp.path()).unwrap();

        assert_ne!(custom_response.cache_key, default_response.cache_key);
        assert_ne!(custom_response.osm_path, default_response.osm_path);
    }

    #[test]
    fn prepare_area_rejects_invalid_overture_theme_only_when_overture_enabled() {
        let bbox = [38.0, -121.0, 38.001, -120.999];
        let filter = par_osm_rust::filter::FeatureFilter::default();
        let mut req = cached_prepare_request(bbox, filter);
        req.overture = true;
        req.overture_themes = vec!["definitely-not-a-theme".to_string()];

        let err = prepare_area(req, Path::new(".")).unwrap_err();

        assert!(matches!(err, PrepareAreaError::BadRequest { .. }));
    }

    #[test]
    fn prepare_area_ignores_invalid_overture_theme_when_overture_disabled() {
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
        let expected_key = prepared_cache_key(
            (bbox[0], bbox[1], bbox[2], bbox[3]),
            &filter,
            false,
            &[],
            par_osm_rust::sources::PoiSourceMode::OsmOnly,
            par_osm_rust::sources::OvertureFailureMode::FallbackToOsm,
            par_osm_rust::overpass::default_overpass_url(),
        );
        let mut req = cached_prepare_request(bbox, filter);
        req.overture_themes = vec!["definitely-not-a-theme".to_string()];
        req.poi_source_mode = Some(par_osm_rust::sources::PoiSourceMode::OvertureOnly);
        req.overture_failure_mode = Some(par_osm_rust::sources::OvertureFailureMode::Fail);

        let response = prepare_area(req, tmp.path()).unwrap();

        assert_eq!(response.cache_key, expected_key);
        assert_eq!(response.source_status, "osm_only");
    }

    #[test]
    fn prepare_area_request_rejects_invalid_source_mode_enums() {
        let invalid_poi_mode = serde_json::json!({
            "bbox": [38.0, -121.0, 38.001, -120.999],
            "poi_source_mode": "not_a_mode"
        });
        let invalid_failure_mode = serde_json::json!({
            "bbox": [38.0, -121.0, 38.001, -120.999],
            "overture_failure_mode": "not_a_mode"
        });

        assert!(serde_json::from_value::<PrepareAreaRequest>(invalid_poi_mode).is_err());
        assert!(serde_json::from_value::<PrepareAreaRequest>(invalid_failure_mode).is_err());
    }

    #[test]
    fn prepare_area_writes_renderable_poi_xml() {
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
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <bounds minlat="38.0" minlon="-121.0" maxlat="38.001" maxlon="-120.999"/>
  <node id="1" lat="38.0005" lon="-120.9995">
    <tag k="amenity" v="restaurant"/>
    <tag k="name" v="Test Cafe"/>
  </node>
</osm>"#;
        cache_xml_for_bbox_with_xml(bbox, &filter, xml);

        let response = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();
        let source =
            crate::world::loader::load_world_source(Path::new(&response.osm_path), None).unwrap();

        assert_eq!(source.point_features.len(), 1);
        assert_eq!(
            source.point_features[0]
                .tags
                .get("name")
                .map(String::as_str),
            Some("Test Cafe")
        );
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
            par_osm_rust::overpass::default_overpass_url(),
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
        assert!(
            Path::new(&response.osm_path)
                .with_extension("meta.json")
                .exists()
        );
    }

    #[test]
    fn prepare_area_retries_degraded_overture_fallback_prepared_cache() {
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
        let mut req = cached_prepare_request(bbox, filter.clone());
        req.overture = true;
        req.poi_source_mode = Some(par_osm_rust::sources::PoiSourceMode::OverturePreferred);
        req.overture_failure_mode = Some(par_osm_rust::sources::OvertureFailureMode::FallbackToOsm);
        req.overture_timeout = Some(1);

        let first = prepare_area(req, tmp.path()).unwrap();
        let mut second_req = cached_prepare_request(bbox, filter);
        second_req.overture = true;
        second_req.poi_source_mode = Some(par_osm_rust::sources::PoiSourceMode::OverturePreferred);
        second_req.overture_failure_mode =
            Some(par_osm_rust::sources::OvertureFailureMode::FallbackToOsm);
        second_req.overture_timeout = Some(1);
        let second = prepare_area(second_req, tmp.path()).unwrap();

        assert_eq!(first.cache_status, "prepared");
        assert_eq!(second.cache_status, "prepared");
        assert_eq!(second.cache_key, first.cache_key);
        assert_eq!(second.osm_path, first.osm_path);
        assert_eq!(second.source_status, "overture_fallback_to_osm");
        assert!(
            second
                .warnings
                .iter()
                .any(|warning| warning.contains("Overture"))
        );
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
