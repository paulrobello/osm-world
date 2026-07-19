> ⚠️ Historical implementation plan (2026-05) — retained for reference; current behavior may differ. See `docs/ARCHITECTURE.md` and the source code.

# par-osm-rust Consumer Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire `osm-to-bedrock` and `osm-world` to the local `par-osm-rust` shared crate so both projects use the neutral shared cache foundation.

**Architecture:** `par-osm-rust` remains the source of truth for shared OSM/SRTM data modules. `osm-to-bedrock` keeps its existing module paths by turning copied modules into thin re-export wrappers, while `osm-world` adopts the shared SRTM cache resolver immediately without changing renderer data flow yet.

**Tech Stack:** Rust 2024, Cargo local path dependencies, existing project Makefiles, `par-osm-rust = { path = "../par-osm-rust" }`.

---

## Scope boundary

This plan intentionally does **not** implement the osm-world web picker/launcher or runtime streaming. It only connects both consumer repos to the shared crate and shared neutral SRTM/Overpass cache locations.

## File structure

### `/Users/probello/Repos/osm-to-bedrock`

- Modify `Cargo.toml` — add local `par-osm-rust` dependency. Keep old dependencies for this pass unless Cargo reports them unused through project-specific linting.
- Modify `src/filter.rs` — replace body with `pub use par_osm_rust::filter::*;`.
- Modify `src/osm.rs` — replace body with `pub use par_osm_rust::osm::*;`.
- Modify `src/osm_cache.rs` — replace body with `pub use par_osm_rust::osm_cache::*;`.
- Modify `src/overpass.rs` — replace body with `pub use par_osm_rust::overpass::*;`.
- Modify `src/srtm.rs` — replace body with `pub use par_osm_rust::srtm::*;`.
- Modify `src/elevation.rs` — replace body with `pub use par_osm_rust::elevation::*;`.

### `/Users/probello/Repos/osm-world`

- Modify `Cargo.toml` — add local `par-osm-rust` dependency.
- Modify `src/geo/srtm.rs` — route `cache_dir()` to `par_osm_rust::cache::srtm_cache_dir()` so default SRTM reads/writes use `~/.cache/par-osm-rust/srtm` and legacy migration.
- Modify `Makefile` — update `run-sacramento` example to use `~/.cache/par-osm-rust/srtm`.

---

### Task 1: Migrate osm-to-bedrock shared modules to re-export par-osm-rust

**Files:**
- Modify: `/Users/probello/Repos/osm-to-bedrock/Cargo.toml`
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/filter.rs`
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/osm.rs`
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/osm_cache.rs`
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/overpass.rs`
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/srtm.rs`
- Modify: `/Users/probello/Repos/osm-to-bedrock/src/elevation.rs`

- [ ] **Step 1: Add the local dependency**

In `/Users/probello/Repos/osm-to-bedrock/Cargo.toml`, add this dependency under `[dependencies]`:

```toml
par-osm-rust = { path = "../par-osm-rust" }
```

- [ ] **Step 2: Replace each migrated module with a re-export wrapper**

Write these exact file contents:

`/Users/probello/Repos/osm-to-bedrock/src/filter.rs`:

```rust
pub use par_osm_rust::filter::*;
```

`/Users/probello/Repos/osm-to-bedrock/src/osm.rs`:

```rust
pub use par_osm_rust::osm::*;
```

`/Users/probello/Repos/osm-to-bedrock/src/osm_cache.rs`:

```rust
pub use par_osm_rust::osm_cache::*;
```

`/Users/probello/Repos/osm-to-bedrock/src/overpass.rs`:

```rust
pub use par_osm_rust::overpass::*;
```

`/Users/probello/Repos/osm-to-bedrock/src/srtm.rs`:

```rust
pub use par_osm_rust::srtm::*;
```

`/Users/probello/Repos/osm-to-bedrock/src/elevation.rs`:

```rust
pub use par_osm_rust::elevation::*;
```

- [ ] **Step 3: Run Rust verification**

Run:

```bash
cd /Users/probello/Repos/osm-to-bedrock
cargo fmt -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all commands pass. If clippy reports unused direct dependencies, remove only the dependencies proven unused by this migration and rerun the commands.

- [ ] **Step 4: Verify cache stats still resolve through the migrated module path**

Run:

```bash
cd /Users/probello/Repos/osm-to-bedrock
cargo run -- cache stats
```

Expected: command succeeds and reports cache stats using the shared cache paths under `~/.cache/par-osm-rust` unless cache env vars override them.

- [ ] **Step 5: Update graphify for osm-to-bedrock**

Run:

```bash
cd /Users/probello/Repos/osm-to-bedrock
graphify update .
```

Expected: graphify completes.

- [ ] **Step 6: Commit osm-to-bedrock migration**

Run:

```bash
cd /Users/probello/Repos/osm-to-bedrock
git add Cargo.toml Cargo.lock src/filter.rs src/osm.rs src/osm_cache.rs src/overpass.rs src/srtm.rs src/elevation.rs graphify-out
git commit -m "Use shared OSM data crate"
```

Expected: commit succeeds. Do not add unrelated dirty files such as `docs/DOCUMENTATION_STYLE_GUIDE.md` or `.githooks/*` unless the user explicitly asks.

---

### Task 2: Migrate osm-world SRTM cache resolution to par-osm-rust

**Files:**
- Modify: `/Users/probello/Repos/osm-world/Cargo.toml`
- Modify: `/Users/probello/Repos/osm-world/Cargo.lock`
- Modify: `/Users/probello/Repos/osm-world/src/geo/srtm.rs`
- Modify: `/Users/probello/Repos/osm-world/Makefile`

- [ ] **Step 1: Add the local dependency**

In `/Users/probello/Repos/osm-world/Cargo.toml`, add this dependency under `[dependencies]`:

```toml
par-osm-rust = { path = "../par-osm-rust" }
```

- [ ] **Step 2: Route SRTM cache dir through par-osm-rust**

In `/Users/probello/Repos/osm-world/src/geo/srtm.rs`, replace the body of `pub fn cache_dir() -> PathBuf` with:

```rust
pub fn cache_dir() -> PathBuf {
    par_osm_rust::cache::srtm_cache_dir()
}
```

Keep the rest of `src/geo/srtm.rs` unchanged for this pass.

- [ ] **Step 3: Update local run example**

In `/Users/probello/Repos/osm-world/Makefile`, update `run-sacramento` to use:

```make
--srtm-dir ~/.cache/par-osm-rust/srtm
```

- [ ] **Step 4: Run osm-world verification**

Run:

```bash
cd /Users/probello/Repos/osm-world
cargo fmt -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all commands pass.

- [ ] **Step 5: Verify existing screenshot path can read migrated cache**

Run:

```bash
cd /Users/probello/Repos/osm-world
cargo run -- --input ../osm-to-bedrock/map_exports/planet_-121.7526,38.63863_-121.72179,38.65671.osm.pbf --srtm-dir ~/.cache/par-osm-rust/srtm --screenshot /tmp/osm_shared_cache_smoke.png --screenshot-delay 2 --auto-exit 3
```

Expected: command exits successfully and writes `/tmp/osm_shared_cache_smoke.png`.

- [ ] **Step 6: Update graphify for osm-world**

Run:

```bash
cd /Users/probello/Repos/osm-world
graphify update .
```

Expected: graphify completes.

- [ ] **Step 7: Commit osm-world migration**

Run:

```bash
cd /Users/probello/Repos/osm-world
git add Cargo.toml Cargo.lock Makefile src/geo/srtm.rs graphify-out docs/superpowers/plans/2026-05-03-par-osm-rust-consumer-migration.md
git commit -m "Use shared SRTM cache crate"
```

Expected: commit succeeds.

---

## Self-review checklist

- Spec coverage: this plan covers local path dependency adoption by both consumer repos and moves active SRTM/Overpass cache resolution toward `~/.cache/par-osm-rust`.
- Deferred requirements: osm-world web picker/launcher and runtime streaming remain deferred.
- Type consistency: re-export wrappers preserve existing `crate::osm`, `crate::osm_cache`, `crate::overpass`, `crate::srtm`, `crate::elevation`, and `crate::filter` paths in osm-to-bedrock.
- Verification: each task has exact commands and expected results.
