//! Shell command building and process launching utilities.

use std::path::Path;
use std::process::Command;

use anyhow::Context;

use super::types::{
    LaunchRendererRequest, LaunchRendererResponse, PrepareAreaError, PrepareResult,
    RendererLaunchCommand,
};
use super::validate::{validate_extra_args, validate_spawn};

/// Spawns the local renderer process for a prepared area.
///
/// Validates spawn coordinates and extra args, builds the command, and spawns
/// the process in the project root directory.
pub(crate) fn launch_renderer(
    project_root: &Path,
    req: &LaunchRendererRequest,
) -> PrepareResult<LaunchRendererResponse> {
    let command = renderer_launch_command(project_root, req)?;
    Command::new(&command.program)
        .args(&command.args)
        .current_dir(project_root)
        .spawn()
        .map_err(|err| {
            PrepareAreaError::internal(
                "failed to launch renderer",
                anyhow::Error::new(err).context("spawning renderer process"),
            )
        })?;

    Ok(LaunchRendererResponse { status: "launched" })
}

/// Builds a `cargo run` command that launches the renderer for a prepared `.osm` file.
///
/// Validates the file extension, spawn coordinates, and extra args.
/// Appends spawn, SRTM, and profile flags to the base command.
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
    if let Some(srtm_dir) = &req.srtm_dir
        && !srtm_dir.trim().is_empty()
    {
        args.push("--srtm-dir".to_string());
        args.push(srtm_dir.clone());
    }
    validate_extra_args(&req.extra_args).map_err(PrepareAreaError::bad_request)?;
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

/// Writes a file atomically by writing to a temporary file first, then renaming.
///
/// Creates parent directories if needed. The temporary file uses a process-specific
/// nonce to avoid collisions.
pub(crate) fn write_atomic(path: &Path, contents: &str) -> anyhow::Result<()> {
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

/// Converts a path to a lossy display string.
pub(crate) fn path_string(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string()
}

/// Joins a program and arguments into a shell-escaped command string.
pub(crate) fn shell_command(program: &str, args: &[String]) -> String {
    std::iter::once(shell_arg(program))
        .chain(args.iter().map(|arg| shell_arg(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Quotes a shell argument if it contains characters that require quoting.
pub(crate) fn shell_arg(value: &str) -> String {
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

/// Wraps a value in single quotes, escaping embedded single quotes using `'"'"'`.
pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
