# par-osm-rust Shared Cache Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `~/Repos/par-osm-rust` as the shared Rust crate for OSM/SRTM parsing, fetching, caching, and legacy cache migration.

**Architecture:** This plan implements Phase 1 of `docs/superpowers/specs/2026-05-03-shared-osm-cache-and-streaming-design.md`. It copies the proven data modules from `osm-to-bedrock`, adds a neutral shared cache layer, and verifies the new crate independently before either consumer repo is migrated.

**Tech Stack:** Rust 2024, Cargo, `anyhow`, `serde`, `osmpbf`, `quick-xml`, `reqwest` blocking client, `chrono`, `sha2`, `flate2`, `memmap2`, `tempfile`.

---

## Scope boundary

The design spec spans multiple independent subsystems. This plan intentionally covers only the first executable milestone: **shared crate foundation plus neutral cache migration**. Separate follow-up plans should cover:

1. migrating `osm-to-bedrock` to the local path dependency,
2. integrating `osm-world` with the local path dependency and generic XML/PBF input,
3. adding the osm-world web picker/launcher,
4. adding in-game runtime streaming.

## File structure

### New repo: `/Users/probello/Repos/par-osm-rust`

- `Cargo.toml` — shared crate package metadata and dependencies copied from source modules.
- `README.md` — cache contract, local dependency usage, verification commands.
- `src/lib.rs` — public module exports and crate-level docs.
- `src/cache.rs` — neutral cache root resolution and legacy osm-to-bedrock cache migration.
- `src/filter.rs` — copied/adapted `FeatureFilter` from osm-to-bedrock.
- `src/osm.rs` — copied/adapted OSM data model, parser, and clipping logic.
- `src/osm_cache.rs` — copied/adapted raw Overpass XML cache; refactored to use `cache::overpass_cache_dir()`.
- `src/overpass.rs` — copied/adapted Overpass query/fetch/cache-aware API.
- `src/srtm.rs` — copied/adapted SRTM tile downloader; refactored to use `cache::srtm_cache_dir()`.
- `src/elevation.rs` — copied/adapted memory-mapped HGT loader/query code.

### Source repo used for copying

- `/Users/probello/Repos/osm-to-bedrock/src/filter.rs`
- `/Users/probello/Repos/osm-to-bedrock/src/osm.rs`
- `/Users/probello/Repos/osm-to-bedrock/src/osm_cache.rs`
- `/Users/probello/Repos/osm-to-bedrock/src/overpass.rs`
- `/Users/probello/Repos/osm-to-bedrock/src/srtm.rs`
- `/Users/probello/Repos/osm-to-bedrock/src/elevation.rs`

---

### Task 1: Scaffold the shared crate repository

**Files:**
- Create: `/Users/probello/Repos/par-osm-rust/Cargo.toml`
- Create: `/Users/probello/Repos/par-osm-rust/src/lib.rs`
- Create: `/Users/probello/Repos/par-osm-rust/README.md`
- Create: `/Users/probello/Repos/par-osm-rust/.gitignore`

- [ ] **Step 1: Create the crate directory and initialize git**

Run:

```bash
cd /Users/probello/Repos
cargo new par-osm-rust --lib --edition 2024
cd /Users/probello/Repos/par-osm-rust
git init
```

Expected:

```text
Created library `par-osm-rust` package
Initialized empty Git repository in /Users/probello/Repos/par-osm-rust/.git/
```

If `cargo new` reports the directory exists, stop and inspect it with:

```bash
ls -la /Users/probello/Repos/par-osm-rust
```

Only continue if it is empty or contains only the files created for this plan.

- [ ] **Step 2: Replace `Cargo.toml` with the shared crate manifest**

Write `/Users/probello/Repos/par-osm-rust/Cargo.toml`:

```toml
[package]
name = "par-osm-rust"
version = "0.1.0"
edition = "2024"
rust-version = "1.87"
description = "Shared OpenStreetMap and SRTM fetch, parse, and cache utilities"
authors = ["Paul Robello <probello@gmail.com>"]
license = "MIT"
repository = "https://github.com/paulrobello/par-osm-rust"

[dependencies]
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
flate2 = "1.1"
log = "0.4"
memmap2 = "0.9"
osmpbf = "0.3.8"
quick-xml = "0.37"
reqwest = { version = "0.12", features = ["blocking"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "0.10"
urlencoding = "2.1"

[dev-dependencies]
tempfile = "3.27.0"
```

- [ ] **Step 3: Replace `src/lib.rs` with public module exports**

Write `/Users/probello/Repos/par-osm-rust/src/lib.rs`:

```rust
//! Shared OpenStreetMap and SRTM fetch, parse, and cache utilities.
//!
//! This crate is shared by `osm-to-bedrock` and `osm-world` through local path
//! dependencies while the API stabilizes. It owns data-source concerns only:
//! OSM parsing, Overpass fetching, raw cache management, SRTM tile downloads,
//! and HGT elevation lookup. It intentionally does not depend on Minecraft,
//! WGPU, UI frameworks, or renderer types.

pub mod cache;
pub mod elevation;
pub mod filter;
pub mod osm;
pub mod osm_cache;
pub mod overpass;
pub mod srtm;
```

- [ ] **Step 4: Write `.gitignore`**

Write `/Users/probello/Repos/par-osm-rust/.gitignore`:

```gitignore
/target/
Cargo.lock
.DS_Store
```

Because this is a library crate, do not commit `Cargo.lock`.

- [ ] **Step 5: Write initial README**

Write `/Users/probello/Repos/par-osm-rust/README.md`:

```markdown
# par-osm-rust

Shared Rust utilities for OpenStreetMap and SRTM data access.

This crate is used by `osm-to-bedrock` and `osm-world` through local path dependencies while the API stabilizes:

```toml
par-osm-rust = { path = "../par-osm-rust" }
```

## Cache locations

Default shared cache directories:

- Overpass XML: `~/.cache/par-osm-rust/overpass`
- SRTM HGT: `~/.cache/par-osm-rust/srtm`

Environment override priority:

- Overpass: `PAR_OSM_OVERPASS_CACHE_DIR`, then `OVERPASS_CACHE_DIR`, then the shared default.
- SRTM: `PAR_OSM_SRTM_CACHE_DIR`, then `SRTM_CACHE_DIR`, then the shared default.
- Overpass endpoint: `OVERPASS_URL`, then `https://overpass-api.de/api/interpreter`.

On first use, the crate can migrate legacy caches from:

- `~/.cache/osm-to-bedrock/overpass`
- `~/.cache/osm-to-bedrock/srtm`

## Verification

```bash
cargo fmt -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test
```
```

- [ ] **Step 6: Verify formatting and the empty module references fail for the expected reason**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cargo check --all-targets
```

Expected: FAIL with missing module files for `cache`, `elevation`, `filter`, `osm`, `osm_cache`, `overpass`, and `srtm`. This proves the scaffold is active before copying modules.

- [ ] **Step 7: Commit scaffold**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
git add Cargo.toml README.md .gitignore src/lib.rs
git commit -m "Create shared OSM data crate"
```

Expected: commit succeeds.

---

### Task 2: Copy shared source modules from osm-to-bedrock

**Files:**
- Create: `/Users/probello/Repos/par-osm-rust/src/filter.rs`
- Create: `/Users/probello/Repos/par-osm-rust/src/osm.rs`
- Create: `/Users/probello/Repos/par-osm-rust/src/osm_cache.rs`
- Create: `/Users/probello/Repos/par-osm-rust/src/overpass.rs`
- Create: `/Users/probello/Repos/par-osm-rust/src/srtm.rs`
- Create: `/Users/probello/Repos/par-osm-rust/src/elevation.rs`

- [ ] **Step 1: Copy modules exactly from osm-to-bedrock**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cp /Users/probello/Repos/osm-to-bedrock/src/filter.rs src/filter.rs
cp /Users/probello/Repos/osm-to-bedrock/src/osm.rs src/osm.rs
cp /Users/probello/Repos/osm-to-bedrock/src/osm_cache.rs src/osm_cache.rs
cp /Users/probello/Repos/osm-to-bedrock/src/overpass.rs src/overpass.rs
cp /Users/probello/Repos/osm-to-bedrock/src/srtm.rs src/srtm.rs
cp /Users/probello/Repos/osm-to-bedrock/src/elevation.rs src/elevation.rs
```

- [ ] **Step 2: Add a temporary empty cache module so copied modules can compile before refactor**

Write `/Users/probello/Repos/par-osm-rust/src/cache.rs`:

```rust
//! Shared cache directory helpers.
//!
//! The full neutral cache migration implementation is added in the next task.
```

- [ ] **Step 3: Run check and capture compile issues**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cargo check --all-targets
```

Expected: PASS. If it fails because a copied module imports a module not in this crate, inspect the import and copy only the missing pure data helper from osm-to-bedrock if it belongs in `par-osm-rust`. Do not copy Bedrock, pipeline, config, server, or frontend-specific code.

- [ ] **Step 4: Run copied unit tests**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cargo test
```

Expected: all copied unit tests pass.

- [ ] **Step 5: Format**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cargo fmt
```

- [ ] **Step 6: Commit copied modules**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
git add src Cargo.toml
git commit -m "Copy OSM and SRTM data modules"
```

Expected: commit succeeds.

---

### Task 3: Add neutral shared cache directories and legacy migration

**Files:**
- Modify: `/Users/probello/Repos/par-osm-rust/src/cache.rs`
- Modify: `/Users/probello/Repos/par-osm-rust/src/osm_cache.rs`
- Modify: `/Users/probello/Repos/par-osm-rust/src/srtm.rs`
- Modify: `/Users/probello/Repos/par-osm-rust/src/lib.rs`

- [ ] **Step 1: Replace `src/cache.rs` with the neutral cache implementation**

Write `/Users/probello/Repos/par-osm-rust/src/cache.rs`:

```rust
//! Shared cache directory resolution and legacy cache migration.

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const SHARED_CACHE_NAME: &str = "par-osm-rust";
const LEGACY_CACHE_NAME: &str = "osm-to-bedrock";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MigrationReport {
    pub overpass: CacheMigrationReport,
    pub srtm: CacheMigrationReport,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CacheMigrationReport {
    pub legacy_dir: PathBuf,
    pub shared_dir: PathBuf,
    pub moved_files: usize,
    pub copied_files: usize,
    pub skipped_files: usize,
    pub removed_duplicate_files: usize,
}

pub fn shared_cache_root() -> PathBuf {
    platform_cache_root(SHARED_CACHE_NAME)
}

pub fn legacy_cache_root() -> PathBuf {
    platform_cache_root(LEGACY_CACHE_NAME)
}

pub fn overpass_cache_dir() -> PathBuf {
    let dir = env_dir("PAR_OSM_OVERPASS_CACHE_DIR")
        .or_else(|| env_dir("OVERPASS_CACHE_DIR"))
        .unwrap_or_else(|| shared_cache_root().join("overpass"));
    ensure_dir(&dir, "Overpass");
    migrate_legacy_cache_dir_if_default(&dir, "overpass");
    dir
}

pub fn srtm_cache_dir() -> PathBuf {
    let dir = env_dir("PAR_OSM_SRTM_CACHE_DIR")
        .or_else(|| env_dir("SRTM_CACHE_DIR"))
        .unwrap_or_else(|| shared_cache_root().join("srtm"));
    ensure_dir(&dir, "SRTM");
    migrate_legacy_cache_dir_if_default(&dir, "srtm");
    dir
}

pub fn migrate_legacy_caches() -> Result<MigrationReport> {
    Ok(MigrationReport {
        overpass: migrate_legacy_cache_dir("overpass")?,
        srtm: migrate_legacy_cache_dir("srtm")?,
    })
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn platform_cache_root(app_name: &str) -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache").join(app_name)
    } else if let Ok(local) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local).join(app_name)
    } else {
        std::env::temp_dir().join(app_name)
    }
}

fn ensure_dir(dir: &Path, label: &str) {
    if let Err(err) = fs::create_dir_all(dir) {
        log::warn!("Could not create {label} cache dir {}: {err}", dir.display());
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

    let shared_empty = fs::read_dir(&shared_dir)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true);

    if !shared_empty {
        report.skipped_files = fs::read_dir(&legacy_dir)?.flatten().count();
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
    let a_meta = fs::metadata(a)?;
    let b_meta = fs::metadata(b)?;
    if a_meta.len() != b_meta.len() {
        return Ok(false);
    }
    Ok(fs::read(a)? == fs::read(b)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn set_env(key: &str, value: &Path) {
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env(key: &str) {
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn overpass_cache_prefers_neutral_env_var() {
        let _guard = env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let neutral = tmp.path().join("neutral-overpass");
        let legacy_override = tmp.path().join("legacy-overpass");
        set_env("PAR_OSM_OVERPASS_CACHE_DIR", &neutral);
        set_env("OVERPASS_CACHE_DIR", &legacy_override);

        let dir = overpass_cache_dir();

        assert_eq!(dir, neutral);
        assert!(dir.exists());
        remove_env("PAR_OSM_OVERPASS_CACHE_DIR");
        remove_env("OVERPASS_CACHE_DIR");
    }

    #[test]
    fn srtm_cache_prefers_neutral_env_var() {
        let _guard = env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let neutral = tmp.path().join("neutral-srtm");
        let legacy_override = tmp.path().join("legacy-srtm");
        set_env("PAR_OSM_SRTM_CACHE_DIR", &neutral);
        set_env("SRTM_CACHE_DIR", &legacy_override);

        let dir = srtm_cache_dir();

        assert_eq!(dir, neutral);
        assert!(dir.exists());
        remove_env("PAR_OSM_SRTM_CACHE_DIR");
        remove_env("SRTM_CACHE_DIR");
    }

    #[test]
    fn migration_moves_legacy_files_into_empty_shared_dir() {
        let _guard = env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        set_env("HOME", tmp.path());
        let legacy = tmp.path().join(".cache/osm-to-bedrock/overpass");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("abc.xml"), "<osm />").unwrap();

        let report = migrate_legacy_cache_dir("overpass").unwrap();

        let shared_file = tmp.path().join(".cache/par-osm-rust/overpass/abc.xml");
        assert!(shared_file.exists());
        assert!(!legacy.join("abc.xml").exists());
        assert_eq!(report.moved_files + report.copied_files, 1);
        remove_env("HOME");
    }

    #[test]
    fn migration_skips_when_shared_dir_already_has_files() {
        let _guard = env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        set_env("HOME", tmp.path());
        let legacy = tmp.path().join(".cache/osm-to-bedrock/srtm");
        let shared = tmp.path().join(".cache/par-osm-rust/srtm");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&shared).unwrap();
        fs::write(legacy.join("N38W122.hgt"), "legacy").unwrap();
        fs::write(shared.join("existing.hgt"), "shared").unwrap();

        let report = migrate_legacy_cache_dir("srtm").unwrap();

        assert_eq!(report.skipped_files, 1);
        assert!(legacy.join("N38W122.hgt").exists());
        remove_env("HOME");
    }
}
```

- [ ] **Step 2: Refactor `osm_cache::cache_dir()` to use the shared cache module**

In `/Users/probello/Repos/par-osm-rust/src/osm_cache.rs`, replace the body of `pub fn cache_dir() -> PathBuf` with:

```rust
pub fn cache_dir() -> PathBuf {
    crate::cache::overpass_cache_dir()
}
```

Keep the function name so existing osm-to-bedrock call sites can migrate with minimal code changes.

- [ ] **Step 3: Refactor `srtm::cache_dir()` to use the shared cache module**

In `/Users/probello/Repos/par-osm-rust/src/srtm.rs`, replace the body of `pub fn cache_dir() -> PathBuf` with:

```rust
pub fn cache_dir() -> PathBuf {
    crate::cache::srtm_cache_dir()
}
```

Keep the function name so existing osm-to-bedrock and osm-world call sites can migrate with minimal code changes.

- [ ] **Step 4: Run focused cache tests**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cargo test cache -- --nocapture
```

Expected: all `cache` module tests pass.

- [ ] **Step 5: Run existing cache tests to verify compatibility**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cargo test osm_cache srtm -- --nocapture
```

Expected: copied `osm_cache` and `srtm` tests pass with the new cache-dir routing.

- [ ] **Step 6: Format and lint**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cargo fmt
cargo clippy --all-targets -- -D warnings
```

Expected: clippy passes with zero warnings.

- [ ] **Step 7: Commit neutral cache migration**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
git add src/cache.rs src/osm_cache.rs src/srtm.rs src/lib.rs
git commit -m "Add shared cache migration"
```

Expected: commit succeeds.

---

### Task 4: Final crate verification and documentation pass

**Files:**
- Modify: `/Users/probello/Repos/par-osm-rust/README.md`

- [ ] **Step 1: Update README with migration API example**

Append this section to `/Users/probello/Repos/par-osm-rust/README.md`:

```markdown
## Cache migration API

Consumers can explicitly migrate legacy osm-to-bedrock caches before starting their own work:

```rust
let report = par_osm_rust::cache::migrate_legacy_caches()?;
println!("migrated overpass files: {}", report.overpass.moved_files + report.overpass.copied_files);
println!("migrated srtm files: {}", report.srtm.moved_files + report.srtm.copied_files);
```

The regular `par_osm_rust::osm_cache::cache_dir()` and `par_osm_rust::srtm::cache_dir()` helpers also attempt default-location legacy migration on first use.
```

- [ ] **Step 2: Run full verification**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cargo fmt -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all commands pass.

- [ ] **Step 3: Inspect public API docs build**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
cargo doc --no-deps
```

Expected: docs build succeeds.

- [ ] **Step 4: Commit final foundation**

Run:

```bash
cd /Users/probello/Repos/par-osm-rust
git add README.md
git commit -m "Document shared cache usage"
```

Expected: commit succeeds if README changed. If README already contains the exact section from Step 1, run `git status --short` and leave this step with no commit.

- [ ] **Step 5: Record crate readiness in osm-world plan notes**

Run:

```bash
cd /Users/probello/Repos/osm-world
git status --short
```

Expected: no uncommitted osm-world code changes from this plan except the plan document itself. The new crate lives in `/Users/probello/Repos/par-osm-rust` with its own git history.

---

## Self-review checklist

- Spec coverage: this plan covers Phase 1 shared crate creation, source module copy, neutral shared cache defaults, legacy osm-to-bedrock cache migration, and independent crate verification.
- Deferred requirements: consumer repo migrations, web picker/launcher, and runtime streaming are intentionally excluded and require follow-up plans.
- Type consistency: `MigrationReport`, `CacheMigrationReport`, `shared_cache_root()`, `overpass_cache_dir()`, `srtm_cache_dir()`, and `migrate_legacy_caches()` are defined before use.
- Verification: each task includes exact Cargo commands and expected results.
