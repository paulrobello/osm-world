//! Shared cache directory resolution and legacy cache migration.

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const SHARED_CACHE_NAME: &str = "par-osm-rust";
const LEGACY_CACHE_NAME: &str = "osm-to-bedrock";

/// Summary for migrating all known legacy cache directories.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MigrationReport {
    pub overpass: CacheMigrationReport,
    pub srtm: CacheMigrationReport,
    pub overture: CacheMigrationReport,
}

/// Summary for migrating one legacy cache directory into its shared location.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CacheMigrationReport {
    pub legacy_dir: PathBuf,
    pub shared_dir: PathBuf,
    pub moved_files: usize,
    pub copied_files: usize,
    pub skipped_files: usize,
    pub removed_duplicate_files: usize,
}

/// Return the platform default root for shared par-osm-rust caches.
pub fn shared_cache_root() -> PathBuf {
    platform_cache_root(SHARED_CACHE_NAME)
}

/// Return the platform default root for legacy osm-to-bedrock caches.
pub fn legacy_cache_root() -> PathBuf {
    platform_cache_root(LEGACY_CACHE_NAME)
}

/// Return the Overpass XML cache directory, creating it if possible.
///
/// Priority:
/// 1. `PAR_OSM_OVERPASS_CACHE_DIR`
/// 2. `OVERPASS_CACHE_DIR`
/// 3. shared default `overpass` directory
pub fn overpass_cache_dir() -> PathBuf {
    let dir = env_dir("PAR_OSM_OVERPASS_CACHE_DIR")
        .or_else(|| env_dir("OVERPASS_CACHE_DIR"))
        .unwrap_or_else(|| shared_cache_root().join("overpass"));
    ensure_dir(&dir, "Overpass");
    migrate_legacy_cache_dir_if_default(&dir, "overpass");
    dir
}

/// Return the SRTM tile cache directory, creating it if possible.
///
/// Priority:
/// 1. `PAR_OSM_SRTM_CACHE_DIR`
/// 2. `SRTM_CACHE_DIR`
/// 3. shared default `srtm` directory
pub fn srtm_cache_dir() -> PathBuf {
    let dir = env_dir("PAR_OSM_SRTM_CACHE_DIR")
        .or_else(|| env_dir("SRTM_CACHE_DIR"))
        .unwrap_or_else(|| shared_cache_root().join("srtm"));
    ensure_dir(&dir, "SRTM");
    migrate_legacy_cache_dir_if_default(&dir, "srtm");
    dir
}

/// Return the Overture GeoJSON cache directory, creating it if possible.
///
/// Priority:
/// 1. `PAR_OSM_OVERTURE_CACHE_DIR`
/// 2. `OVERTURE_CACHE_DIR`
/// 3. shared default `overture` directory
pub fn overture_cache_dir() -> PathBuf {
    let override_dir =
        env_dir("PAR_OSM_OVERTURE_CACHE_DIR").or_else(|| env_dir("OVERTURE_CACHE_DIR"));
    let is_default = override_dir.is_none();
    let dir = override_dir.unwrap_or_else(|| shared_cache_root().join("overture"));
    ensure_dir(&dir, "Overture");
    if is_default {
        migrate_legacy_cache_dir_if_default(&dir, "overture");
    }
    dir
}

/// Migrate legacy osm-to-bedrock Overpass, SRTM, and Overture caches into shared defaults.
pub fn migrate_legacy_caches() -> Result<MigrationReport> {
    Ok(MigrationReport {
        overpass: migrate_legacy_cache_dir("overpass")?,
        srtm: migrate_legacy_cache_dir("srtm")?,
        overture: migrate_legacy_cache_dir("overture")?,
    })
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn platform_cache_root(app_name: &str) -> PathBuf {
    if let Some(home) = env_dir("HOME") {
        home.join(".cache").join(app_name)
    } else if let Some(local) = env_dir("LOCALAPPDATA") {
        local.join(app_name)
    } else {
        std::env::temp_dir().join(app_name)
    }
}

fn ensure_dir(dir: &Path, label: &str) {
    if let Err(err) = fs::create_dir_all(dir) {
        log::warn!(
            "Could not create {label} cache dir {}: {err}",
            dir.display()
        );
    }
}

fn migrate_legacy_cache_dir_if_default(shared_dir: &Path, subdir: &str) {
    let expected_default = shared_cache_root().join(subdir);
    if shared_dir != expected_default {
        return;
    }

    if let Err(err) = migrate_legacy_cache_dir(subdir) {
        log::warn!("Legacy {subdir} cache migration failed: {err:#}");
    }
}

fn migrate_legacy_cache_dir(subdir: &str) -> Result<CacheMigrationReport> {
    let legacy_dir = legacy_cache_root().join(subdir);
    let shared_dir = shared_cache_root().join(subdir);
    fs::create_dir_all(&shared_dir)
        .with_context(|| format!("creating shared cache dir {}", shared_dir.display()))?;

    let mut report = CacheMigrationReport {
        legacy_dir: legacy_dir.clone(),
        shared_dir: shared_dir.clone(),
        ..CacheMigrationReport::default()
    };

    if !legacy_dir.exists() {
        return Ok(report);
    }

    if !is_dir_empty(&shared_dir)? && subdir != "overture" {
        report.skipped_files = legacy_file_count(&legacy_dir)?;
        return Ok(report);
    }

    for entry in fs::read_dir(&legacy_dir)
        .with_context(|| format!("reading legacy cache dir {}", legacy_dir.display()))?
    {
        let entry = entry?;
        let src = entry.path();
        if !src.is_file() {
            report.skipped_files += 1;
            continue;
        }
        let dst = shared_dir.join(entry.file_name());
        migrate_file(&src, &dst, &mut report)?;
    }

    Ok(report)
}

fn is_dir_empty(dir: &Path) -> Result<bool> {
    Ok(fs::read_dir(dir)
        .with_context(|| format!("reading shared cache dir {}", dir.display()))?
        .next()
        .is_none())
}

fn legacy_file_count(legacy_dir: &Path) -> Result<usize> {
    let mut count = 0usize;
    for entry in fs::read_dir(legacy_dir)
        .with_context(|| format!("reading legacy cache dir {}", legacy_dir.display()))?
    {
        if entry?.path().is_file() {
            count += 1;
        }
    }
    Ok(count)
}

fn migrate_file(src: &Path, dst: &Path, report: &mut CacheMigrationReport) -> Result<()> {
    if dst.exists() {
        if files_equal(src, dst)? {
            fs::remove_file(src)
                .with_context(|| format!("removing duplicate legacy file {}", src.display()))?;
            report.removed_duplicate_files += 1;
        } else {
            report.skipped_files += 1;
        }
        return Ok(());
    }

    match fs::rename(src, dst) {
        Ok(()) => {
            report.moved_files += 1;
            Ok(())
        }
        Err(rename_err) => {
            fs::copy(src, dst).with_context(|| {
                format!(
                    "copying legacy cache file {} to {} after rename failed: {rename_err}",
                    src.display(),
                    dst.display()
                )
            })?;
            fs::remove_file(src)
                .with_context(|| format!("removing copied legacy file {}", src.display()))?;
            report.copied_files += 1;
            Ok(())
        }
    }
}

fn files_equal(a: &Path, b: &Path) -> Result<bool> {
    let a_meta =
        fs::metadata(a).with_context(|| format!("reading metadata for {}", a.display()))?;
    let b_meta =
        fs::metadata(b).with_context(|| format!("reading metadata for {}", b.display()))?;
    if a_meta.len() != b_meta.len() {
        return Ok(false);
    }
    Ok(
        fs::read(a).with_context(|| format!("reading {}", a.display()))?
            == fs::read(b).with_context(|| format!("reading {}", b.display()))?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    const ENV_KEYS: &[&str] = &[
        "HOME",
        "LOCALAPPDATA",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ];

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvSnapshot {
        values: HashMap<&'static str, Option<OsString>>,
    }

    impl EnvSnapshot {
        fn capture() -> Self {
            let values = ENV_KEYS
                .iter()
                .map(|&key| (key, std::env::var_os(key)))
                .collect();
            Self { values }
        }

        fn set_path(&self, key: &str, value: &Path) {
            unsafe {
                std::env::set_var(key, value);
            }
        }

        fn remove(&self, key: &str) {
            unsafe {
                std::env::remove_var(key);
            }
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (&key, value) in &self.values {
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
    }

    fn isolate_cache_env(env: &EnvSnapshot) {
        for key in ENV_KEYS {
            env.remove(key);
        }
    }

    #[test]
    fn shared_and_legacy_roots_use_home_when_available() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());

        assert_eq!(
            shared_cache_root(),
            tmp.path().join(".cache").join("par-osm-rust")
        );
        assert_eq!(
            legacy_cache_root(),
            tmp.path().join(".cache").join("osm-to-bedrock")
        );
    }

    #[test]
    fn roots_use_localappdata_when_home_is_unset() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("LOCALAPPDATA", tmp.path());

        assert_eq!(shared_cache_root(), tmp.path().join("par-osm-rust"));
        assert_eq!(legacy_cache_root(), tmp.path().join("osm-to-bedrock"));
    }

    #[test]
    fn overpass_cache_prefers_neutral_env_var() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        let neutral = tmp.path().join("neutral-overpass");
        let legacy_override = tmp.path().join("legacy-overpass");
        env.set_path("PAR_OSM_OVERPASS_CACHE_DIR", &neutral);
        env.set_path("OVERPASS_CACHE_DIR", &legacy_override);

        let dir = overpass_cache_dir();

        assert_eq!(dir, neutral);
        assert!(dir.exists());
    }

    #[test]
    fn overpass_cache_uses_legacy_env_var_before_default() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let legacy_override = tmp.path().join("legacy-overpass");
        env.set_path("HOME", &home);
        env.set_path("OVERPASS_CACHE_DIR", &legacy_override);

        let dir = overpass_cache_dir();

        assert_eq!(dir, legacy_override);
        assert!(dir.exists());
    }

    #[test]
    fn srtm_cache_prefers_neutral_env_var() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        let neutral = tmp.path().join("neutral-srtm");
        let legacy_override = tmp.path().join("legacy-srtm");
        env.set_path("PAR_OSM_SRTM_CACHE_DIR", &neutral);
        env.set_path("SRTM_CACHE_DIR", &legacy_override);

        let dir = srtm_cache_dir();

        assert_eq!(dir, neutral);
        assert!(dir.exists());
    }

    #[test]
    fn srtm_cache_uses_legacy_env_var_before_default() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let legacy_override = tmp.path().join("legacy-srtm");
        env.set_path("HOME", &home);
        env.set_path("SRTM_CACHE_DIR", &legacy_override);

        let dir = srtm_cache_dir();

        assert_eq!(dir, legacy_override);
        assert!(dir.exists());
    }

    #[test]
    fn overture_cache_prefers_neutral_env_var() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        let neutral = tmp.path().join("neutral-overture");
        let legacy_override = tmp.path().join("legacy-overture");
        env.set_path("PAR_OSM_OVERTURE_CACHE_DIR", &neutral);
        env.set_path("OVERTURE_CACHE_DIR", &legacy_override);

        let dir = overture_cache_dir();

        assert_eq!(dir, neutral);
        assert!(dir.exists());
    }

    #[test]
    fn overture_cache_uses_legacy_env_var_before_default() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let legacy_override = tmp.path().join("legacy-overture");
        env.set_path("HOME", &home);
        env.set_path("OVERTURE_CACHE_DIR", &legacy_override);

        let dir = overture_cache_dir();

        assert_eq!(dir, legacy_override);
        assert!(dir.exists());
    }

    #[test]
    fn overpass_cache_default_migrates_legacy_files_on_first_use() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());
        let legacy = tmp.path().join(".cache/osm-to-bedrock/overpass");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("abc.xml"), "<osm />").unwrap();

        let dir = overpass_cache_dir();

        let shared_file = dir.join("abc.xml");
        assert_eq!(dir, tmp.path().join(".cache/par-osm-rust/overpass"));
        assert!(shared_file.exists());
        assert!(!legacy.join("abc.xml").exists());
    }

    #[test]
    fn overture_cache_default_migrates_legacy_files_on_first_use() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());
        let legacy = tmp.path().join(".cache/osm-to-bedrock/overture");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("places.geojson"), "{}").unwrap();
        fs::write(legacy.join("places.meta.json"), "{}").unwrap();

        let dir = crate::overture::overture_cache_dir();

        assert_eq!(dir, tmp.path().join(".cache/par-osm-rust/overture"));
        assert!(dir.join("places.geojson").exists());
        assert!(dir.join("places.meta.json").exists());
        assert!(!legacy.join("places.geojson").exists());
        assert!(!legacy.join("places.meta.json").exists());
    }

    #[test]
    fn overture_cache_default_merges_legacy_files_into_non_empty_shared_dir() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());
        let legacy = tmp.path().join(".cache/osm-to-bedrock/overture");
        let shared = tmp.path().join(".cache/par-osm-rust/overture");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&shared).unwrap();
        fs::write(legacy.join("area-a.geojson"), "legacy-a").unwrap();
        fs::write(shared.join("area-b.geojson"), "shared-b").unwrap();

        let dir = crate::overture::overture_cache_dir();

        assert_eq!(dir, shared);
        assert_eq!(
            fs::read_to_string(dir.join("area-a.geojson")).unwrap(),
            "legacy-a"
        );
        assert_eq!(
            fs::read_to_string(dir.join("area-b.geojson")).unwrap(),
            "shared-b"
        );
        assert!(!legacy.join("area-a.geojson").exists());
    }

    #[test]
    fn overture_cache_env_override_does_not_migrate_legacy_files() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());
        let override_dir = tmp.path().join("custom-overture-cache");
        env.set_path("PAR_OSM_OVERTURE_CACHE_DIR", &override_dir);
        let legacy = tmp.path().join(".cache/osm-to-bedrock/overture");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("places.geojson"), "{}").unwrap();

        let dir = crate::overture::overture_cache_dir();

        assert_eq!(dir, override_dir);
        assert!(dir.exists());
        assert!(legacy.join("places.geojson").exists());
        assert!(
            !tmp.path()
                .join(".cache/par-osm-rust/overture/places.geojson")
                .exists()
        );
    }

    #[test]
    fn overture_cache_override_matching_default_does_not_migrate_legacy_files() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());
        let default_dir = tmp.path().join(".cache/par-osm-rust/overture");
        env.set_path("OVERTURE_CACHE_DIR", &default_dir);
        let legacy = tmp.path().join(".cache/osm-to-bedrock/overture");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("places.geojson"), "{}").unwrap();

        let dir = crate::overture::overture_cache_dir();

        assert_eq!(dir, default_dir);
        assert!(dir.exists());
        assert!(legacy.join("places.geojson").exists());
        assert!(!dir.join("places.geojson").exists());
    }

    #[test]
    fn migration_moves_legacy_files_into_empty_shared_dir() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());
        let legacy = tmp.path().join(".cache/osm-to-bedrock/overpass");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("abc.xml"), "<osm />").unwrap();

        let report = migrate_legacy_cache_dir("overpass").unwrap();

        let shared_file = tmp.path().join(".cache/par-osm-rust/overpass/abc.xml");
        assert_eq!(report.legacy_dir, legacy);
        assert_eq!(
            report.shared_dir,
            tmp.path().join(".cache/par-osm-rust/overpass")
        );
        assert!(shared_file.exists());
        assert!(!report.legacy_dir.join("abc.xml").exists());
        assert_eq!(report.moved_files + report.copied_files, 1);
        assert_eq!(report.skipped_files, 0);
        assert_eq!(report.removed_duplicate_files, 0);
    }

    #[test]
    fn migration_skips_when_shared_dir_already_has_files() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());
        let legacy = tmp.path().join(".cache/osm-to-bedrock/srtm");
        let shared = tmp.path().join(".cache/par-osm-rust/srtm");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&shared).unwrap();
        fs::write(legacy.join("N38W122.hgt"), "legacy").unwrap();
        fs::write(shared.join("existing.hgt"), "shared").unwrap();

        let report = migrate_legacy_cache_dir("srtm").unwrap();

        assert_eq!(report.skipped_files, 1);
        assert_eq!(report.moved_files, 0);
        assert_eq!(report.copied_files, 0);
        assert!(legacy.join("N38W122.hgt").exists());
    }

    #[test]
    fn migration_removes_identical_legacy_duplicate() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());
        let legacy = tmp.path().join(".cache/osm-to-bedrock/overpass");
        let shared = tmp.path().join(".cache/par-osm-rust/overpass");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&shared).unwrap();
        let legacy_file = legacy.join("same.xml");
        let shared_file = shared.join("same.xml");
        fs::write(&legacy_file, "same").unwrap();
        fs::write(&shared_file, "same").unwrap();
        let mut report = CacheMigrationReport::default();

        migrate_file(&legacy_file, &shared_file, &mut report).unwrap();

        assert_eq!(report.removed_duplicate_files, 1);
        assert!(!legacy_file.exists());
        assert!(shared_file.exists());
    }

    #[test]
    fn migration_skips_different_existing_destination() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());
        let legacy = tmp.path().join(".cache/osm-to-bedrock/overpass");
        let shared = tmp.path().join(".cache/par-osm-rust/overpass");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&shared).unwrap();
        let legacy_file = legacy.join("different.xml");
        let shared_file = shared.join("different.xml");
        fs::write(&legacy_file, "legacy").unwrap();
        fs::write(&shared_file, "shared").unwrap();
        let mut report = CacheMigrationReport::default();

        migrate_file(&legacy_file, &shared_file, &mut report).unwrap();

        assert_eq!(report.skipped_files, 1);
        assert!(legacy_file.exists());
        assert_eq!(fs::read_to_string(shared_file).unwrap(), "shared");
    }

    #[test]
    fn migrate_legacy_caches_reports_all_cache_types() {
        let _guard = env_lock().lock().unwrap();
        let env = EnvSnapshot::capture();
        isolate_cache_env(&env);
        let tmp = TempDir::new().unwrap();
        env.set_path("HOME", tmp.path());
        let overpass_legacy = tmp.path().join(".cache/osm-to-bedrock/overpass");
        let srtm_legacy = tmp.path().join(".cache/osm-to-bedrock/srtm");
        let overture_legacy = tmp.path().join(".cache/osm-to-bedrock/overture");
        fs::create_dir_all(&overpass_legacy).unwrap();
        fs::create_dir_all(&srtm_legacy).unwrap();
        fs::create_dir_all(&overture_legacy).unwrap();
        fs::write(overpass_legacy.join("abc.xml"), "<osm />").unwrap();
        fs::write(srtm_legacy.join("N38W122.hgt"), "hgt").unwrap();
        fs::write(overture_legacy.join("places.geojson"), "{}").unwrap();

        let report = migrate_legacy_caches().unwrap();

        assert_eq!(
            report.overpass.moved_files + report.overpass.copied_files,
            1
        );
        assert_eq!(report.srtm.moved_files + report.srtm.copied_files, 1);
        assert_eq!(
            report.overture.moved_files + report.overture.copied_files,
            1
        );
    }
}
