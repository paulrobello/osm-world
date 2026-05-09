//! Server API: route handlers, validation, prepared cache management, and shell utilities.

pub mod prepared_cache;
pub mod routes;
pub mod shell;
pub mod types;
pub mod validate;

// Re-export the public API so callers can use `crate::server::build_router` etc.
pub use routes::{build_router, run};
pub use types::{
    DeletePreparedAreaResponse, LaunchRendererRequest, PrepareAreaRequest, PrepareAreaResponse,
    PreparedAreaEntry, PreparedAreaUpdate, RendererLaunchCommand, SourceConfig,
};

#[cfg(test)]
mod tests;
