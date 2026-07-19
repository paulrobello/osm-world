//! Axum route handlers, router construction, and the `prepare_area` business logic.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::Context;
use axum::{
    Json, Router,
    extract::{Path as AxumPath, State, rejection::JsonRejection},
    http::{HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use tokio::task;
use tower_http::cors::{AllowOrigin, CorsLayer};

use super::prepared_cache::{
    delete_prepared_area, list_prepared_areas, prepared_metadata_path,
    read_prepared_cache_metadata, read_prepared_cache_metadata_struct,
    update_prepared_area_details, write_prepared_cache_metadata,
};
use super::shell::{launch_renderer, path_string, renderer_launch_command};
use super::types::{
    ApiError, AppState, DeletePreparedAreaResponse, ErrorResponse, HealthResponse,
    LaunchRendererRequest, LaunchRendererResponse, PrepareAreaError, PrepareAreaRequest,
    PrepareAreaResponse, PrepareResult, PreparedAreaEntry, PreparedAreaUpdate,
    PreparedCacheMetadata,
};
use super::validate::{
    auth_middleware, classify_srtm_error, effective_overpass_url_for_prepare,
    is_degraded_overture_prepared_cache, parse_overture_themes_for_prepare, prepared_cache_key,
    rate_limit_middleware, source_status_string, validate_bbox, validate_filter, validate_spawn,
    validate_srtm_tile_limit,
};

/// Default CORS origins when `OSM_WORLD_CORS_ORIGINS` is unset: the two
/// localhost origins used by the Web Explorer dev server.
const DEFAULT_CORS_ORIGINS: &[&str] = &["http://localhost:8032", "http://127.0.0.1:8032"];

/// Builds the Axum router with all API routes.
///
/// Routes are split into a tiny public router (`/health`) and an authenticated
/// router that carries every other endpoint — including the previously
/// unauthenticated read-only ones (`GET /cache/areas`, `GET /areas/prepared`),
/// which serialize filesystem paths (SEC-003). `auth_middleware` is a no-op when
/// `OSM_WORLD_API_TOKEN` is unset (local-default), so local UX is unchanged; only
/// remote operators who deliberately set a token see 401s.
///
/// `rate_limit_middleware` is applied to the *merged* router so read-only routes
/// are covered too (SEC-010) — `GET /cache/areas` does a `spawn_blocking` dir
/// scan per call and would otherwise be a cheap DoS vector.
///
/// CORS origins default to the two localhost origins used by the Web Explorer
/// dev server and can be overridden via `OSM_WORLD_CORS_ORIGINS` (ARC-010).
pub fn build_router(project_root: PathBuf) -> Router {
    let public = Router::new().route("/health", get(health));

    let authed = Router::new()
        .route("/cache/areas", get(cache_areas))
        .route("/areas/prepared", get(prepared_areas_handler))
        .route(
            "/areas/prepared/{cache_key}",
            post(update_prepared_area_handler).delete(delete_prepared_area_handler),
        )
        .route("/areas/prepare", post(prepare_area_handler))
        .route("/renderer/launch", post(launch_renderer_handler))
        .layer(axum::middleware::from_fn(auth_middleware));

    Router::new()
        .merge(public)
        .merge(authed)
        .fallback(not_found)
        .with_state(AppState { project_root })
        .layer(axum::middleware::from_fn(rate_limit_middleware))
        .layer(cors_layer())
}

/// Builds the CORS layer, reading allowed origins from the
/// `OSM_WORLD_CORS_ORIGINS` env var (comma-separated) when set, otherwise
/// falling back to [`DEFAULT_CORS_ORIGINS`]. Empty strings in the list are
/// dropped. Invalid header values are logged and skipped.
fn cors_layer() -> CorsLayer {
    let origins = allowed_cors_origins();
    let mut list: Vec<HeaderValue> = Vec::with_capacity(origins.len());
    for origin in &origins {
        match HeaderValue::from_str(origin) {
            Ok(value) => list.push(value),
            Err(err) => log::warn!("ignoring invalid CORS origin {origin:?}: {err}"),
        }
    }
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(list))
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ])
}

/// Resolves the effective CORS origin allowlist: the comma-separated values from
/// `OSM_WORLD_CORS_ORIGINS` when that env var is set and non-empty (after
/// trimming), otherwise the default localhost origins.
fn allowed_cors_origins() -> Vec<String> {
    match std::env::var("OSM_WORLD_CORS_ORIGINS") {
        Ok(raw) => {
            let parsed: Vec<String> = raw
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if parsed.is_empty() {
                DEFAULT_CORS_ORIGINS
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect()
            } else {
                parsed
            }
        }
        Err(_) => DEFAULT_CORS_ORIGINS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    }
}

/// Starts the Axum HTTP server on the given host and port.
///
/// Binds a TCP listener and serves the API router. The router is wrapped in
/// `into_make_service_with_connect_info` so each request carries the TCP peer
/// address; the rate-limit middleware uses that instead of trusting spoofable
/// client headers. Blocks until the server encounters an error or is shut down.
pub async fn run(host: &str, port: u16, project_root: PathBuf) -> anyhow::Result<()> {
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("binding HTTP server to {addr}"))?;
    log::info!("area prepare API listening on http://{addr}");
    axum::serve(
        listener,
        build_router(project_root).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("running HTTP server")
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
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

/// Core area preparation logic: validates the request, resolves source controls,
/// reuses or creates a prepared `.osm` file, optionally downloads SRTM tiles,
/// writes metadata, and builds the renderer launch command.
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
            super::shell::write_atomic(&osm_path, &xml)
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
