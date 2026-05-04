use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
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
}

#[derive(Debug, Serialize)]
pub struct PrepareAreaResponse {
    pub bbox: [f64; 4],
    pub cache_key: String,
    pub cache_status: String,
    pub osm_path: String,
    pub srtm_dir: Option<String>,
    pub command: String,
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
    message: String,
}

impl ApiError {
    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    fn from_anyhow(err: anyhow::Error) -> Self {
        let message = format!("{err:#}");
        let status = if is_bad_request_error(&message) {
            StatusCode::BAD_REQUEST
        } else if is_upstream_error(&message) {
            StatusCode::BAD_GATEWAY
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        Self { status, message }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

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
    let Json(req) = payload.map_err(|err| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: err.to_string(),
    })?;
    let project_root = state.project_root;
    task::spawn_blocking(move || prepare_area(req, &project_root))
        .await
        .map_err(|err| ApiError::internal(format!("prepare task failed: {err}")))?
        .map(Json)
        .map_err(ApiError::from_anyhow)
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
    _project_root: &Path,
) -> anyhow::Result<PrepareAreaResponse> {
    let bbox = validate_bbox(req.bbox)?;
    if let Some(url) = req.overpass_url.as_deref() {
        par_osm_rust::overpass::validate_overpass_url(url)?;
    }

    let cache_key = par_osm_rust::osm_cache::cache_key(bbox, &req.filter);
    let mut cache_status = None;

    let xml = if req.force_refresh {
        None
    } else if let Some(xml) = par_osm_rust::osm_cache::read(&cache_key) {
        cache_status = Some("exact_cache_hit".to_string());
        Some(xml)
    } else if let Some(xml) = par_osm_rust::osm_cache::find_containing(bbox, &req.filter) {
        cache_status = Some("containing_cache_hit".to_string());
        Some(xml)
    } else {
        None
    };

    let xml = match xml {
        Some(xml) => xml,
        None => {
            let overpass_url = match req.overpass_url.as_deref() {
                Some(url) => url,
                None => par_osm_rust::overpass::default_overpass_url(),
            };
            let xml = par_osm_rust::overpass::fetch_osm_xml(bbox, &req.filter, overpass_url)
                .context("fetching OSM XML from Overpass")?;
            par_osm_rust::osm_cache::write(&cache_key, bbox, &req.filter, &xml)
                .context("writing exact Overpass cache entry")?;
            cache_status = Some(if req.force_refresh {
                "force_refreshed".to_string()
            } else {
                "fetched".to_string()
            });
            xml
        }
    };

    let prepared_dir = par_osm_rust::cache::shared_cache_root().join("prepared");
    std::fs::create_dir_all(&prepared_dir)
        .with_context(|| format!("creating prepared cache dir {}", prepared_dir.display()))?;
    let osm_path = prepared_dir.join(format!("{cache_key}.osm"));
    write_atomic(&osm_path, &xml).with_context(|| {
        format!(
            "writing prepared OSM XML to {}",
            osm_path.as_path().display()
        )
    })?;

    let srtm_dir = if req.use_elevation {
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
        .context("downloading SRTM tiles")?;
        Some(path_string(srtm_dir))
    } else {
        None
    };

    let mut command = format!(
        "cargo run -- --input {}",
        shell_quote(&path_string(&osm_path))
    );
    if let Some(srtm_dir) = &srtm_dir {
        command.push_str(" --srtm-dir ");
        command.push_str(&shell_quote(srtm_dir));
    }

    Ok(PrepareAreaResponse {
        bbox: req.bbox,
        cache_key,
        cache_status: cache_status.unwrap_or_else(|| "unknown".to_string()),
        osm_path: path_string(osm_path),
        srtm_dir,
        command,
    })
}

fn validate_bbox(bbox: [f64; 4]) -> anyhow::Result<(f64, f64, f64, f64)> {
    let [south, west, north, east] = bbox;
    if !south.is_finite() || !west.is_finite() || !north.is_finite() || !east.is_finite() {
        bail!("invalid bbox: all coordinates must be finite");
    }
    if south < -90.0 || north > 90.0 {
        bail!("invalid bbox: latitude must be in -90..=90");
    }
    if west < -180.0 || east > 180.0 {
        bail!("invalid bbox: longitude must be in -180..=180");
    }
    if south >= north {
        bail!("invalid bbox: south ({south}) must be less than north ({north})");
    }
    if west >= east {
        bail!("invalid bbox: west ({west}) must be less than east ({east})");
    }
    Ok((south, west, north, east))
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

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn is_bad_request_error(message: &str) -> bool {
    message.contains("invalid bbox")
        || message.contains("all feature types are disabled")
        || message.contains("Invalid Overpass URL")
        || message.contains("Overpass URL")
        || message.contains("Overpass host")
}

fn is_upstream_error(message: &str) -> bool {
    message.contains("Overpass") || message.contains("HTTP") || message.contains("Request failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, sync::Mutex};

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

    #[test]
    fn prepare_area_uses_exact_cached_xml_without_elevation() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let _restore = EnvRestore::capture(&[
            "HOME",
            "PAR_OSM_OVERPASS_CACHE_DIR",
            "OVERPASS_CACHE_DIR",
            "PAR_OSM_SRTM_CACHE_DIR",
            "SRTM_CACHE_DIR",
        ]);

        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let overpass_dir = tmp.path().join("overpass");
        let srtm_dir = tmp.path().join("srtm");
        unsafe {
            std::env::set_var("HOME", &home);
            std::env::set_var("PAR_OSM_OVERPASS_CACHE_DIR", &overpass_dir);
            std::env::remove_var("OVERPASS_CACHE_DIR");
            std::env::set_var("PAR_OSM_SRTM_CACHE_DIR", &srtm_dir);
            std::env::remove_var("SRTM_CACHE_DIR");
        }

        let bbox = [38.0, -121.0, 38.001, -120.999];
        let filter = par_osm_rust::filter::FeatureFilter::default();
        let cache_key =
            par_osm_rust::osm_cache::cache_key((bbox[0], bbox[1], bbox[2], bbox[3]), &filter);
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
</osm>"#;
        par_osm_rust::osm_cache::write(
            &cache_key,
            (bbox[0], bbox[1], bbox[2], bbox[3]),
            &filter,
            xml,
        )
        .unwrap();

        let req = PrepareAreaRequest {
            bbox,
            filter,
            use_elevation: false,
            force_refresh: false,
            overpass_url: None,
        };
        let response = prepare_area(req, tmp.path()).unwrap();

        assert_eq!(response.cache_key, cache_key);
        assert_eq!(response.cache_status, "exact_cache_hit");
        assert!(response.osm_path.ends_with(".osm"));
        assert!(std::path::Path::new(&response.osm_path).exists());
        assert_eq!(std::fs::read_to_string(&response.osm_path).unwrap(), xml);
        assert!(response.srtm_dir.is_none());
        assert!(response.command.contains("--input"));
        assert!(response.command.contains(&response.osm_path));
        assert!(!response.command.contains("--srtm-dir"));
    }
}
