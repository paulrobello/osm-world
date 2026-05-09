//! Request, response, and error types for the server API.

use std::path::PathBuf;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

/// Shared application state passed to all Axum route handlers.
///
/// Holds the project root path used to build renderer launch commands
/// and resolve the `Cargo.toml` manifest location.
#[derive(Clone)]
pub(crate) struct AppState {
    /// Absolute path to the project root directory containing `Cargo.toml`.
    pub project_root: PathBuf,
}

/// Shared source configuration extracted from common fields across request,
/// response, and metadata types. Provides a single place to read source settings
/// without duplicating field names across 5 structs.
#[derive(Debug, Clone, Serialize)]
pub struct SourceConfig {
    pub filter: par_osm_rust::filter::FeatureFilter,
    pub use_elevation: bool,
    pub overture: bool,
    pub overture_themes: Vec<String>,
    pub poi_source_mode: Option<par_osm_rust::sources::PoiSourceMode>,
    pub overture_failure_mode: Option<par_osm_rust::sources::OvertureFailureMode>,
    pub overture_timeout: Option<u64>,
}

/// Request body for `POST /areas/prepare`.
///
/// Describes the bounding box, feature filter, source controls, and optional
/// spawn point for preparing an OSM data extract.
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

impl PrepareAreaRequest {
    /// Extract the shared source configuration from this request.
    pub fn source_config(&self) -> SourceConfig {
        SourceConfig {
            filter: self.filter.clone(),
            use_elevation: self.use_elevation,
            overture: self.overture,
            overture_themes: self.overture_themes.clone(),
            poi_source_mode: self.poi_source_mode,
            overture_failure_mode: self.overture_failure_mode,
            overture_timeout: self.overture_timeout,
        }
    }
}

/// Response returned after a successful area preparation.
///
/// Includes the prepared file path, cache status, source status, warnings,
/// and a complete renderer launch command.
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

/// A prepared area entry as returned by `GET /areas/prepared`.
///
/// Combines the original preparation response with persisted metadata
/// (display name, favorite flag, filter, source settings).
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

impl PreparedAreaEntry {
    /// Extract the shared source configuration from this entry.
    pub fn source_config(&self) -> SourceConfig {
        SourceConfig {
            filter: self.filter.clone(),
            use_elevation: self.use_elevation,
            overture: self.overture,
            overture_themes: self.overture_themes.clone(),
            poi_source_mode: self.poi_source_mode,
            overture_failure_mode: self.overture_failure_mode,
            overture_timeout: self.overture_timeout,
        }
    }
}

/// Request body for `POST /areas/prepared/{cache_key}`.
///
/// Used to rename or toggle the favorite flag on a prepared area.
/// Both fields are optional; only provided fields are updated.
#[derive(Debug, Deserialize)]
pub struct PreparedAreaUpdate {
    pub display_name: Option<String>,
    pub favorite: Option<bool>,
}

/// Response returned after deleting a prepared area.
#[derive(Debug, Serialize)]
pub struct DeletePreparedAreaResponse {
    pub status: &'static str,
    pub cache_key: String,
}

/// Request body for `POST /renderer/launch`.
///
/// Specifies the prepared `.osm` file, optional SRTM directory, optional
/// spawn coordinates, and additional renderer flags to forward.
#[derive(Debug, Deserialize)]
pub struct LaunchRendererRequest {
    pub osm_path: String,
    pub srtm_dir: Option<String>,
    pub spawn_lat: Option<f64>,
    pub spawn_lon: Option<f64>,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

/// A fully resolved renderer launch command with program, arguments, and shell representation.
#[derive(Debug, Serialize)]
pub struct RendererLaunchCommand {
    pub program: String,
    pub args: Vec<String>,
    pub command: String,
    pub command_cwd: String,
}

/// Response returned after successfully launching the renderer process.
#[derive(Debug, Serialize)]
pub(crate) struct LaunchRendererResponse {
    pub status: &'static str,
}

/// Persisted metadata for a prepared area, stored alongside the `.osm` file
/// as a `.meta.json` sidecar.
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

/// Health check response returned by `GET /health`.
#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    pub status: &'static str,
}

/// Error response body returned for all API error conditions.
#[derive(Debug, Serialize)]
pub(crate) struct ErrorResponse {
    pub error: String,
}

/// Typed API error that converts into an Axum error response.
///
/// Carries the HTTP status code and a client-safe message.
/// Detailed error information is logged server-side but not exposed to callers.
#[derive(Debug)]
pub(crate) struct ApiError {
    pub status: StatusCode,
    pub client_message: &'static str,
}

impl ApiError {
    /// Creates a 400 Bad Request error, logging the detail.
    pub fn invalid_request(detail: impl std::fmt::Display) -> Self {
        log::warn!("invalid API request: {detail}");
        Self {
            status: StatusCode::BAD_REQUEST,
            client_message: "invalid request",
        }
    }

    /// Creates a 500 Internal Server Error, logging the detail.
    pub fn internal(detail: impl std::fmt::Display) -> Self {
        log::error!("internal API error: {detail}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            client_message: "failed to prepare area",
        }
    }

    /// Converts a `PrepareAreaError` into the appropriate `ApiError` variant.
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

/// Categorized error for area preparation operations.
///
/// Distinguishes between client errors (bad request), upstream errors
/// (Overpass/Overture failures), and internal errors (filesystem, serialization).
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
    /// Creates a `BadRequest` variant from a validation or parsing error.
    pub fn bad_request(source: anyhow::Error) -> Self {
        Self::BadRequest { source }
    }

    /// Creates an `Upstream` variant for external service failures.
    pub fn upstream(client_message: &'static str, source: anyhow::Error) -> Self {
        Self::Upstream {
            client_message,
            source,
        }
    }

    /// Creates an `Internal` variant for unexpected server-side failures.
    pub fn internal(client_message: &'static str, source: anyhow::Error) -> Self {
        Self::Internal {
            client_message,
            source,
        }
    }
}

pub(crate) type PrepareResult<T> = Result<T, PrepareAreaError>;
