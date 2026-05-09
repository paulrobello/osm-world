//! Request, response, and error types for the server API.

use std::path::PathBuf;

use axum::{
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub(crate) struct AppState {
    pub project_root: PathBuf,
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
pub(crate) struct LaunchRendererResponse {
    pub status: &'static str,
    pub pid: u32,
    #[serde(flatten)]
    pub command: RendererLaunchCommand,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct PreparedCacheMetadata {
    pub source_status: String,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub bbox: Option<[f64; 4]>,
    #[serde(default)]
    pub filter: Option<par_osm_rust::filter::FeatureFilter>,
    #[serde(default)]
    pub use_elevation: bool,
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
    #[serde(default)]
    pub srtm_dir: Option<String>,
    #[serde(default)]
    pub spawn_lat: Option<f64>,
    #[serde(default)]
    pub spawn_lon: Option<f64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    pub status: &'static str,
    pub overpass_cache_dir: String,
    pub srtm_cache_dir: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ErrorResponse {
    pub error: String,
}

#[derive(Debug)]
pub(crate) struct ApiError {
    pub status: StatusCode,
    pub client_message: &'static str,
}

impl ApiError {
    pub fn invalid_request(detail: impl std::fmt::Display) -> Self {
        log::warn!("invalid API request: {detail}");
        Self {
            status: StatusCode::BAD_REQUEST,
            client_message: "invalid request",
        }
    }

    pub fn internal(detail: impl std::fmt::Display) -> Self {
        log::error!("internal API error: {detail}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            client_message: "failed to prepare area",
        }
    }

    pub fn from_prepare_error(err: PrepareAreaError) -> Self {
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
    pub fn bad_request(source: anyhow::Error) -> Self {
        Self::BadRequest { source }
    }

    pub fn upstream(client_message: &'static str, source: anyhow::Error) -> Self {
        Self::Upstream {
            client_message,
            source,
        }
    }

    pub fn internal(client_message: &'static str, source: anyhow::Error) -> Self {
        Self::Internal {
            client_message,
            source,
        }
    }
}

pub(crate) type PrepareResult<T> = Result<T, PrepareAreaError>;
