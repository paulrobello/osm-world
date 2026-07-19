> ⚠️ Historical implementation plan (2026-05) — retained for reference; current behavior may differ. See `docs/ARCHITECTURE.md` and the source code.

# osm-world Area Prepare Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the backend foundation for the web area picker: osm-world can prepare a selected bbox by fetching/caching OSM XML and SRTM data through `par-osm-rust`, then return a copyable render command.

**Architecture:** Keep rendering separate from data preparation. `world::loader` learns to accept `.osm` XML as well as `.pbf`; a new lightweight Axum server exposes cache/prepare endpoints; CLI `--serve` starts only the backend for the future web UI.

**Tech Stack:** Rust 2024, Axum, Tokio, Tower HTTP CORS, local `par-osm-rust`, existing osm-world renderer and tests.

---

## File structure

- Modify `Cargo.toml` — add `axum`, `tokio`, `tower-http` for the small HTTP API.
- Modify `src/lib.rs` — export `server`.
- Create `src/server.rs` — health, cache areas, and area prepare endpoints.
- Modify `src/main.rs` — add `--serve`, `--host`, and `--port` CLI flags and start the API server before creating a Winit event loop.
- Modify `src/world/loader.rs` — use generic `parse_osm_file()` instead of `parse_pbf()` and add XML loader coverage.

---

### Task 1: Accept generic `.osm` XML input in the world loader

**Files:**
- Modify: `src/world/loader.rs`

- [ ] **Step 1: Replace the parser import**

Change:

```rust
use crate::osm::parse::parse_pbf;
```

to:

```rust
use crate::osm::parse::parse_osm_file;
```

- [ ] **Step 2: Replace the parse call and comment**

In `load_world_source()`, change:

```rust
// 1. Parse PBF
let osm_data = parse_pbf(pbf_path)?;
```

to:

```rust
// 1. Parse OSM input (PBF or XML)
let osm_data = parse_osm_file(pbf_path)?;
```

- [ ] **Step 3: Add a loader test for XML input**

In `src/world/loader.rs` test module, add:

```rust
#[test]
fn load_world_source_accepts_osm_xml_input() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("area.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.999"/>
  <node id="3" lat="38.001" lon="-120.999"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <tag k="highway" v="residential"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.roads.len(), 1);
    assert!(source.min_lat <= 38.0);
    assert!(source.max_lat >= 38.001);
}
```

- [ ] **Step 4: Verify**

Run:

```bash
cargo test world::loader::tests::load_world_source_accepts_osm_xml_input -- --nocapture
cargo test osm::parse::tests::parse_osm_file_detects_format -- --nocapture
```

Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add src/world/loader.rs
git commit -m "Load OSM XML world inputs"
```

---

### Task 2: Add area prepare HTTP server

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Create: `src/server.rs`

- [ ] **Step 1: Add dependencies**

Add to `[dependencies]` in `Cargo.toml`:

```toml
axum = "0.8.8"
tokio = { version = "1.50.0", features = ["full"] }
tower-http = { version = "0.6.8", features = ["cors"] }
```

- [ ] **Step 2: Export server module**

Add to `src/lib.rs`:

```rust
pub mod server;
```

- [ ] **Step 3: Create `src/server.rs`**

Implement these public shapes and endpoints:

```rust
use std::path::PathBuf;

use anyhow::Context;
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::{get, post}};
use serde::{Deserialize, Serialize};
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
```

Required behavior:

- `GET /health` returns JSON with `status`, `overpass_cache_dir`, and `srtm_cache_dir`.
- `GET /cache/areas` returns `par_osm_rust::osm_cache::list_areas()`.
- `POST /areas/prepare`:
  1. validates/fetches raw OSM XML cache-first using `par_osm_rust::osm_cache` and `par_osm_rust::overpass::fetch_osm_xml`,
  2. writes prepared XML to `par_osm_rust::cache::shared_cache_root()/prepared/{cache_key}.osm`,
  3. downloads SRTM tiles to `par_osm_rust::cache::srtm_cache_dir()` when `use_elevation` is true,
  4. returns a command like `cargo run -- --input <osm_path> --srtm-dir <srtm_dir>`.

Add helper functions:

```rust
pub fn build_router(project_root: PathBuf) -> Router
pub async fn run(host: &str, port: u16, project_root: PathBuf) -> anyhow::Result<()>
fn prepare_area(req: PrepareAreaRequest, project_root: &std::path::Path) -> anyhow::Result<PrepareAreaResponse>
```

- [ ] **Step 4: Add server tests in `src/server.rs`**

Add unit tests for command construction using a synthetic request and cached XML written into a temp cache via env overrides. At minimum, verify that a prepared response command includes `--input` and the prepared `.osm` path when `use_elevation` is false.

- [ ] **Step 5: Verify**

Run:

```bash
cargo test server -- --nocapture
cargo check --all-targets
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/server.rs
git commit -m "Add area prepare API"
```

---

### Task 3: Add CLI server mode and verify prepare endpoint

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add CLI flags**

Add to `Args`:

```rust
/// Run the HTTP API server instead of opening the renderer window
#[arg(long)]
serve: bool,

/// API server host when --serve is used
#[arg(long, default_value = "127.0.0.1")]
host: String,

/// API server port when --serve is used
#[arg(long, default_value_t = 3030)]
port: u16,
```

- [ ] **Step 2: Start server before Winit setup**

In `main()`, after parsing args and initializing logging, add:

```rust
if args.serve {
    let rt = tokio::runtime::Runtime::new()?;
    return rt.block_on(osm_world::server::run(
        &args.host,
        args.port,
        std::env::current_dir()?,
    ));
}
```

This must run before `winit::event_loop::EventLoop::new()`.

- [ ] **Step 3: Add CLI parse test**

In `src/main.rs` tests, add:

```rust
#[test]
fn parses_serve_flags() {
    let args = Args::try_parse_from([
        "osm-world",
        "--serve",
        "--host",
        "0.0.0.0",
        "--port",
        "3031",
    ])
    .unwrap();

    assert!(args.serve);
    assert_eq!(args.host, "0.0.0.0");
    assert_eq!(args.port, 3031);
}
```

- [ ] **Step 4: Verify CLI and server health**

Run:

```bash
cargo test tests::parses_serve_flags -- --nocapture
cargo run -- --serve --host 127.0.0.1 --port 3030 &
SERVER_PID=$!
sleep 2
curl -fsS http://127.0.0.1:3030/health
kill $SERVER_PID
wait $SERVER_PID || true
```

Expected: test passes and health endpoint returns JSON.

- [ ] **Step 5: Run full verification**

```bash
make checkall
graphify update .
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs graphify-out
git commit -m "Add API server mode"
```

---

## Self-review checklist

- Spec coverage: adds backend foundation for future web picker/launcher, generic `.osm` input, cache-first prepare endpoint, and shared SRTM download path.
- Deferred requirements: no frontend UI and no launch endpoint in this phase.
- Verification: includes focused tests, server health smoke, `make checkall`, and graphify update.
