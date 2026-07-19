//! Shell command building and process launching utilities.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

use super::prepared_cache::prepared_area_dir;
use super::types::{
    LaunchRendererRequest, LaunchRendererResponse, PrepareAreaError, PrepareResult,
    RendererLaunchCommand,
};
use super::validate::{validate_extra_args, validate_spawn};

/// Spawns the local renderer process for a prepared area.
///
/// Validates spawn coordinates, extra args, and that `osm_path`/`srtm_dir`
/// resolve inside the prepared-area and SRTM cache roots (SEC-005), then builds
/// the command and spawns the process in the project root directory.
pub(crate) fn launch_renderer(
    project_root: &Path,
    req: &LaunchRendererRequest,
) -> PrepareResult<LaunchRendererResponse> {
    // SEC-005: confine `osm_path` to the prepared-area root and `srtm_dir` to
    // the SRTM cache root. Rejects traversal (`..`), absolute paths outside the
    // root, and escaping symlinks before they reach the renderer. Validation
    // lives here (not in `renderer_launch_command`) because the command builder
    // is also called from trusted internal paths (`prepare_area`,
    // `prepared_entry_from_osm_path`) that build their inputs from disk listings
    // and do not need re-validation.
    validate_path_inside(req.osm_path.as_str(), &prepared_area_dir(), "osm_path")?;
    if let Some(srtm_dir) = &req.srtm_dir
        && !srtm_dir.trim().is_empty()
    {
        validate_path_inside(srtm_dir, &par_osm_rust::cache::srtm_cache_dir(), "srtm_dir")?;
    }
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
///
/// Path confinement (SEC-005) is enforced by [`launch_renderer`], the public
/// entry point for HTTP-routed launches; this builder is also called from
/// trusted internal paths that do not require re-validation.
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

/// SEC-005: verifies that `candidate` resolves to a path inside `root` after
/// canonicalization (symlinks resolved). Returns the canonicalized path on
/// success.
///
/// Rejects:
/// - paths that do not exist (canonicalization fails)
/// - traversal (`..`) segments that escape the root
/// - absolute paths outside the root
/// - symlinks whose target leaves the root
///
/// `field_name` is included in the error so callers see which input was
/// rejected.
fn validate_path_inside(
    candidate: &str,
    root: &Path,
    field_name: &'static str,
) -> PrepareResult<PathBuf> {
    let candidate_path = Path::new(candidate);
    let canonical_root = root.canonicalize().map_err(|err| {
        PrepareAreaError::bad_request(anyhow::Error::new(err).context(format!(
            "{field_name} root {} is not accessible",
            root.display()
        )))
    })?;
    let canonical_candidate = candidate_path.canonicalize().map_err(|err| {
        PrepareAreaError::bad_request(anyhow::Error::new(err).context(format!(
            "{field_name} does not resolve inside {}",
            canonical_root.display()
        )))
    })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(PrepareAreaError::bad_request(anyhow::anyhow!(
            "{field_name} must stay inside {}",
            canonical_root.display()
        )));
    }
    Ok(canonical_candidate)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a temp directory that stands in for a cache root, plus a child
    /// file that is "inside" the root. Returns `(root, inside_file,
    /// outside_file)` where `outside_file` is a sibling of `root` (definitely
    /// outside the root prefix).
    fn fixture(file_name: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().expect("temp dir");
        let root = tmp.path().canonicalize().expect("canonicalize root");
        let inside_file = root.join(file_name);
        std::fs::write(&inside_file, b"fixture").expect("write fixture");
        // A second temp directory whose path is *not* under `root`.
        let outside_root = tempfile::tempdir().expect("outside temp dir");
        let outside_file = outside_root
            .path()
            .canonicalize()
            .expect("canonicalize outside")
            .join("outside.txt");
        std::fs::write(&outside_file, b"outside").expect("write outside fixture");
        // Leak the outside TempDir so the file survives the test body. Cleaned
        // up by the OS in /tmp; tests are short-lived.
        std::mem::forget(outside_root);
        (tmp, inside_file, outside_file)
    }

    #[test]
    fn sec005_accepts_path_inside_root() {
        let (_tmp, inside, _) = fixture("cache_key.osm");
        let result = validate_path_inside(
            &inside.display().to_string(),
            inside.parent().unwrap(),
            "osm_path",
        );
        assert!(result.is_ok(), "inside path should be accepted");
    }

    #[test]
    fn sec005_rejects_absolute_path_outside_root() {
        let (_tmp, inside, outside) = fixture("cache_key.osm");
        let root = inside.parent().unwrap();
        let err =
            validate_path_inside(&outside.display().to_string(), root, "osm_path").unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("must stay inside") || msg.contains("does not resolve"),
            "expected confinement error, got: {msg}"
        );
    }

    #[test]
    fn sec005_rejects_traversal_segment() {
        let (_tmp, inside, _) = fixture("cache_key.osm");
        let root = inside.parent().unwrap();
        // Build `<root>/../../../etc/hosts` — even if it happens to exist, the
        // canonicalized form will not start with `root`.
        let escaped = root
            .join("..")
            .join("..")
            .join("..")
            .join("etc")
            .join("hosts");
        let result = validate_path_inside(&escaped.display().to_string(), root, "osm_path");
        // Either the file does not exist (canonicalize fails) or the canonical
        // form is outside the root — both are rejections.
        assert!(result.is_err(), "traversal should be rejected");
    }

    #[test]
    fn sec005_rejects_nonexistent_path() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let root = tmp.path().canonicalize().expect("canonicalize root");
        let ghost = root.join("does-not-exist.osm");
        let err =
            validate_path_inside(&ghost.display().to_string(), &root, "osm_path").unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("does not resolve"),
            "expected resolve error, got: {msg}"
        );
    }

    #[test]
    fn sec005_rejects_symlink_escape() {
        // Symlink creation requires write access to a real FS; on some CI
        // filesystems (certain Linux overlay configs) symlink creation can be
        // blocked, so skip rather than fail when we cannot create one.
        let (_tmp, inside, outside) = fixture("cache_key.osm");
        let root = inside.parent().unwrap().to_path_buf();
        let link = root.join("escape.osm");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            match symlink(&outside, &link) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("skipping sec005_rejects_symlink_escape: symlink failed: {err}");
                    return;
                }
            }
        }
        #[cfg(not(unix))]
        {
            eprintln!(
                "skipping sec005_rejects_symlink_escape: symlink not supported on this platform"
            );
            return;
        }
        let err = validate_path_inside(&link.display().to_string(), &root, "osm_path").unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("must stay inside") || msg.contains("does not resolve"),
            "symlink escape should be rejected, got: {msg}"
        );
    }

    #[test]
    fn sec005_rejects_when_root_does_not_exist() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let ghost_root = tmp.path().join("ghost-root");
        let err = validate_path_inside(
            &ghost_root.join("x.osm").display().to_string(),
            &ghost_root,
            "osm_path",
        )
        .unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("not accessible"),
            "expected root error, got: {msg}"
        );
    }
}
