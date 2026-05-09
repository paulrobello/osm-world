//! Prepared area cache CRUD: directory layout, metadata read/write, list/update/delete.

use std::path::{Path, PathBuf};

use anyhow::Context;

use super::shell::{path_string, renderer_launch_command};
use super::types::{
    LaunchRendererRequest, PreparedAreaEntry, PreparedAreaUpdate, PreparedCacheMetadata,
    PrepareAreaError, PrepareResult, DeletePreparedAreaResponse,
};
use super::validate::validate_cache_key;

/// Returns the prepared area directory under the shared cache root.
pub(crate) fn prepared_area_dir() -> PathBuf {
    par_osm_rust::cache::shared_cache_root().join("prepared")
}

/// Returns the metadata sidecar path for a prepared `.osm` file (`.meta.json`).
pub(crate) fn prepared_metadata_path(osm_path: &Path) -> PathBuf {
    osm_path.with_extension("meta.json")
}

/// Reads source status and warnings from prepared cache metadata.
///
/// Returns `("cached_unknown", [error message])` when the metadata is missing or unreadable.
pub(crate) fn read_prepared_cache_metadata(metadata_path: &Path) -> (String, Vec<String>) {
    match read_prepared_cache_metadata_struct(metadata_path) {
        Ok(metadata) => (metadata.source_status, metadata.warnings),
        Err(message) => ("cached_unknown".to_string(), vec![message]),
    }
}

/// Reads and deserializes the prepared cache metadata struct from a `.meta.json` file.
///
/// Returns `Err(message)` with a human-readable description on read or parse failure.
pub(crate) fn read_prepared_cache_metadata_struct(
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

/// Serializes and writes prepared cache metadata to disk atomically.
pub(crate) fn write_prepared_cache_metadata(
    metadata_path: &Path,
    metadata: &PreparedCacheMetadata,
) -> anyhow::Result<()> {
    let contents =
        serde_json::to_string_pretty(metadata).context("serializing prepared cache metadata")?;
    super::shell::write_atomic(metadata_path, &(contents + "\n")).with_context(|| {
        format!(
            "writing prepared cache metadata {}",
            metadata_path.display()
        )
    })
}

/// Builds a `PreparedAreaEntry` from an `.osm` file path by reading its
/// sidecar metadata and constructing the renderer launch command.
pub(crate) fn prepared_entry_from_osm_path(
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

/// Lists all prepared areas sorted by favorite status, display name, and cache key.
///
/// Returns an empty list when the prepared directory does not exist.
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

/// Updates the display name and/or favorite flag of a prepared area.
///
/// Only provided fields are changed; the other fields remain unchanged.
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

/// Deletes a prepared area's `.osm` file and its `.meta.json` sidecar.
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
