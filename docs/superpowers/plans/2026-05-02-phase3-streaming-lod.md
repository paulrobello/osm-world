# Phase 3 Streaming + LOD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single-mesh OSM renderer with tile streaming, LOD selection, frustum culling, upload budgets, and cache eviction while preserving a `--no-streaming` fallback.

**Architecture:** Parse Sacramento OSM/SRTM data once into a CPU `WorldSource`, index features by 1km tile, generate `TileMeshSet` LOD meshes in worker threads, upload ready tiles on the render thread under a per-frame budget, and draw visible uploaded tiles after frustum culling. Keep existing Phase 4 visual fixes by reusing current world mesh generators inside the tile path.

**Tech Stack:** Rust 2024, wgpu 29, winit 0.30, crossbeam-channel, egui, glam, existing OSM/SRTM/world mesh modules.

**Spec:** `docs/superpowers/specs/2026-05-02-phase3-streaming-lod-design.md`

---

## File Structure

Create:
- `src/stream/mod.rs` — streaming module exports and public config/stats types.
- `src/stream/tile.rs` — tile coordinates, bounds, AABB, tile math, feature-to-tile assignment helpers.
- `src/stream/lod.rs` — LOD enum, thresholds, hysteresis, LOD config.
- `src/stream/worker.rs` — worker request/result messages and background generation loop.
- `src/stream/streamer.rs` — `TileStreamer` state machine, desired-tile queueing, epoch handling, stats, CPU-ready result handling.
- `src/render/frustum.rs` — frustum plane extraction and AABB tests.
- `src/render/tile_buffers.rs` — GPU tile buffer structs, upload byte estimates, upload-budget selection, cache eviction helpers.

Modify:
- `src/lib.rs` — export `stream`.
- `src/render/mod.rs` — export `frustum` and `tile_buffers`.
- `src/world/loader.rs` — split parse/classify into `WorldSource`; preserve `load_world()` fallback; add tile mesh generation.
- `src/world/terrain.rs` — add terrain generation by world tile bounds and grid spacing.
- `src/app/mod.rs` — add streaming options and optional streamer state.
- `src/app/init.rs` — initialize single-mesh fallback or streaming source/streamer.
- `src/app/update.rs` — tick streamer, process worker results, upload ready tiles, update camera uniforms.
- `src/app/render_loop.rs` — draw either fallback scene or visible uploaded tiles.
- `src/ui/hud.rs` and `src/ui/settings.rs` — show streaming diagnostics.
- `src/main.rs` — add Phase 3 CLI flags.

---

### Task 1: Add Streaming CLI and Config Plumbing

**Files:**
- Modify: `src/main.rs`
- Modify: `src/app/mod.rs`

- [ ] **Step 1: Add failing CLI parse tests**

Add these tests to `src/main.rs` inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn parses_streaming_flags() {
    let args = Args::try_parse_from([
        "osm-world",
        "--no-streaming",
        "--tile-size",
        "500",
        "--stream-radius",
        "2500",
        "--upload-budget-mb",
        "2.5",
        "--max-uploaded-tiles",
        "64",
        "--max-uploaded-mb",
        "128",
    ])
    .unwrap();

    assert!(args.no_streaming);
    assert_eq!(args.tile_size, 500.0);
    assert_eq!(args.stream_radius, 2500.0);
    assert_eq!(args.upload_budget_mb, 2.5);
    assert_eq!(args.max_uploaded_tiles, 64);
    assert_eq!(args.max_uploaded_mb, 128.0);
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test parses_streaming_flags -- --nocapture
```

Expected: fails because `Args` has no streaming fields.

- [ ] **Step 3: Add config types**

In `src/app/mod.rs`, add:

```rust
#[derive(Clone, Debug)]
pub struct StreamingOptions {
    pub enabled: bool,
    pub tile_size: f32,
    pub stream_radius: f32,
    pub upload_budget_mb: f32,
    pub max_uploaded_tiles: usize,
    pub max_uploaded_mb: f32,
}

impl Default for StreamingOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            tile_size: 1000.0,
            stream_radius: 15_000.0,
            upload_budget_mb: 4.0,
            max_uploaded_tiles: 256,
            max_uploaded_mb: 512.0,
        }
    }
}
```

Add to `AppOptions`:

```rust
pub streaming: StreamingOptions,
```

- [ ] **Step 4: Add CLI flags and pass options**

In `src/main.rs`, add to `Args`:

```rust
/// Disable tile streaming and use the legacy single-mesh renderer
#[arg(long)]
no_streaming: bool,

/// Streaming tile size in metres
#[arg(long, default_value = "1000.0")]
tile_size: f32,

/// Streaming radius in metres
#[arg(long, default_value = "15000.0")]
stream_radius: f32,

/// Per-frame GPU upload budget in MiB
#[arg(long, default_value = "4.0")]
upload_budget_mb: f32,

/// Maximum number of uploaded streaming tiles
#[arg(long, default_value = "256")]
max_uploaded_tiles: usize,

/// Maximum estimated uploaded tile memory in MiB
#[arg(long, default_value = "512.0")]
max_uploaded_mb: f32,
```

When constructing `AppOptions`, pass:

```rust
streaming: osm_world::app::StreamingOptions {
    enabled: !args.no_streaming,
    tile_size: args.tile_size,
    stream_radius: args.stream_radius,
    upload_budget_mb: args.upload_budget_mb,
    max_uploaded_tiles: args.max_uploaded_tiles,
    max_uploaded_mb: args.max_uploaded_mb,
},
```

- [ ] **Step 5: Verify**

Run:

```bash
cargo test parses_streaming_flags -- --nocapture
cargo fmt -- --check
cargo check --all-targets
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/app/mod.rs
git commit -m "feat: add streaming configuration flags"
```

---

### Task 2: Add Tile Math and LOD Primitives

**Files:**
- Create: `src/stream/mod.rs`
- Create: `src/stream/tile.rs`
- Create: `src/stream/lod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Export the stream module**

Add to `src/lib.rs`:

```rust
pub mod stream;
```

Create `src/stream/mod.rs`:

```rust
pub mod lod;
pub mod tile;

pub use lod::{LodConfig, TileLod};
pub use tile::{TileAabb, TileCoord, TileRect};
```

- [ ] **Step 2: Write tile math tests**

Create `src/stream/tile.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coord_from_world_handles_positive_and_negative_positions() {
        assert_eq!(TileCoord::from_world(0.0, 0.0, 1000.0), TileCoord { x: 0, z: 0 });
        assert_eq!(TileCoord::from_world(999.9, -0.1, 1000.0), TileCoord { x: 0, z: -1 });
        assert_eq!(TileCoord::from_world(-0.1, -1000.0, 1000.0), TileCoord { x: -1, z: -1 });
    }

    #[test]
    fn bounds_are_one_tile_wide() {
        let rect = TileCoord { x: 2, z: -3 }.rect(1000.0);
        assert_eq!(rect.min_x, 2000.0);
        assert_eq!(rect.max_x, 3000.0);
        assert_eq!(rect.min_z, -3000.0);
        assert_eq!(rect.max_z, -2000.0);
    }

    #[test]
    fn bbox_to_tiles_includes_all_touched_tiles() {
        let tiles = tiles_for_bbox(900.0, -1100.0, 2100.0, 100.0, 1000.0);
        assert!(tiles.contains(&TileCoord { x: 0, z: -2 }));
        assert!(tiles.contains(&TileCoord { x: 2, z: 0 }));
        assert_eq!(tiles.len(), 9);
    }
}
```

- [ ] **Step 3: Implement tile math**

Above the tests in `src/stream/tile.rs`, add:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileCoord {
    pub x: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRect {
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileAabb {
    pub min: glam::Vec3,
    pub max: glam::Vec3,
}

impl TileCoord {
    pub fn from_world(x: f32, z: f32, tile_size: f32) -> Self {
        Self {
            x: (x / tile_size).floor() as i32,
            z: (z / tile_size).floor() as i32,
        }
    }

    pub fn rect(self, tile_size: f32) -> TileRect {
        let min_x = self.x as f32 * tile_size;
        let min_z = self.z as f32 * tile_size;
        TileRect {
            min_x,
            min_z,
            max_x: min_x + tile_size,
            max_z: min_z + tile_size,
        }
    }

    pub fn center(self, tile_size: f32) -> glam::Vec3 {
        let r = self.rect(tile_size);
        glam::Vec3::new((r.min_x + r.max_x) * 0.5, 0.0, (r.min_z + r.max_z) * 0.5)
    }
}

impl TileRect {
    pub fn intersects_bbox(&self, min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> bool {
        self.min_x <= max_x && self.max_x >= min_x && self.min_z <= max_z && self.max_z >= min_z
    }
}

pub fn tiles_for_bbox(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    tile_size: f32,
) -> Vec<TileCoord> {
    let start = TileCoord::from_world(min_x, min_z, tile_size);
    let end = TileCoord::from_world(max_x, max_z, tile_size);
    let mut out = Vec::new();
    for z in start.z..=end.z {
        for x in start.x..=end.x {
            out.push(TileCoord { x, z });
        }
    }
    out
}
```

- [ ] **Step 4: Write LOD tests**

Create `src/stream/lod.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_selects_by_distance() {
        let cfg = LodConfig::default();
        assert_eq!(cfg.select(1000.0, TileLod::Near), TileLod::Near);
        assert_eq!(cfg.select(3000.0, TileLod::Near), TileLod::Mid);
        assert_eq!(cfg.select(6000.0, TileLod::Mid), TileLod::Far);
    }

    #[test]
    fn lod_hysteresis_prevents_threshold_flicker() {
        let cfg = LodConfig::default();
        assert_eq!(cfg.select(2100.0, TileLod::Near), TileLod::Near);
        assert_eq!(cfg.select(1900.0, TileLod::Mid), TileLod::Mid);
        assert_eq!(cfg.select(5200.0, TileLod::Mid), TileLod::Mid);
        assert_eq!(cfg.select(4700.0, TileLod::Far), TileLod::Far);
    }
}
```

- [ ] **Step 5: Implement LOD primitives**

Above the tests in `src/stream/lod.rs`, add:

```rust
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileLod {
    Near = 0,
    Mid = 1,
    Far = 2,
}

#[derive(Clone, Copy, Debug)]
pub struct LodConfig {
    pub near_to_mid: f32,
    pub mid_to_near: f32,
    pub mid_to_far: f32,
    pub far_to_mid: f32,
}

impl Default for LodConfig {
    fn default() -> Self {
        Self {
            near_to_mid: 2200.0,
            mid_to_near: 1800.0,
            mid_to_far: 5500.0,
            far_to_mid: 4500.0,
        }
    }
}

impl LodConfig {
    pub fn select(&self, distance: f32, previous: TileLod) -> TileLod {
        match previous {
            TileLod::Near if distance > self.near_to_mid => TileLod::Mid,
            TileLod::Mid if distance < self.mid_to_near => TileLod::Near,
            TileLod::Mid if distance > self.mid_to_far => TileLod::Far,
            TileLod::Far if distance < self.far_to_mid => TileLod::Mid,
            other => other,
        }
    }

    pub fn terrain_spacing(lod: TileLod) -> f32 {
        match lod {
            TileLod::Near => 10.0,
            TileLod::Mid => 50.0,
            TileLod::Far => 100.0,
        }
    }
}
```

- [ ] **Step 6: Verify**

Run:

```bash
cargo test stream:: -- --nocapture
cargo fmt -- --check
cargo check --all-targets
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs src/stream
git commit -m "feat: add streaming tile and lod primitives"
```

---

### Task 3: Add Frustum Culling

**Files:**
- Create: `src/render/frustum.rs`
- Modify: `src/render/mod.rs`

- [ ] **Step 1: Export frustum module**

Add to `src/render/mod.rs`:

```rust
pub mod frustum;
```

- [ ] **Step 2: Write frustum tests**

Create `src/render/frustum.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_frustum() -> Frustum {
        let view = glam::Mat4::look_at_rh(glam::Vec3::ZERO, glam::Vec3::new(0.0, 0.0, -1.0), glam::Vec3::Y);
        let proj = glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        Frustum::from_view_proj(proj * view)
    }

    #[test]
    fn aabb_inside_frustum_intersects() {
        let f = test_frustum();
        assert!(f.intersects_aabb(glam::Vec3::new(-1.0, -1.0, -5.0), glam::Vec3::new(1.0, 1.0, -3.0)));
    }

    #[test]
    fn aabb_behind_camera_does_not_intersect() {
        let f = test_frustum();
        assert!(!f.intersects_aabb(glam::Vec3::new(-1.0, -1.0, 3.0), glam::Vec3::new(1.0, 1.0, 5.0)));
    }

    #[test]
    fn aabb_crossing_near_plane_intersects() {
        let f = test_frustum();
        assert!(f.intersects_aabb(glam::Vec3::new(-0.1, -0.1, -0.2), glam::Vec3::new(0.1, 0.1, 0.1)));
    }
}
```

- [ ] **Step 3: Implement frustum extraction**

Above tests in `src/render/frustum.rs`, add:

```rust
#[derive(Clone, Copy, Debug)]
struct Plane {
    normal: glam::Vec3,
    d: f32,
}

impl Plane {
    fn normalize(self) -> Self {
        let len = self.normal.length();
        if len <= f32::EPSILON {
            return self;
        }
        Self {
            normal: self.normal / len,
            d: self.d / len,
        }
    }

    fn distance(self, p: glam::Vec3) -> f32 {
        self.normal.dot(p) + self.d
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    pub fn from_view_proj(view_proj: glam::Mat4) -> Self {
        let m = view_proj.to_cols_array_2d();
        let row = |i: usize| glam::Vec4::new(m[0][i], m[1][i], m[2][i], m[3][i]);
        let r0 = row(0);
        let r1 = row(1);
        let r2 = row(2);
        let r3 = row(3);
        let make = |v: glam::Vec4| Plane {
            normal: glam::Vec3::new(v.x, v.y, v.z),
            d: v.w,
        }
        .normalize();

        Self {
            planes: [
                make(r3 + r0),
                make(r3 - r0),
                make(r3 + r1),
                make(r3 - r1),
                make(r3 + r2),
                make(r3 - r2),
            ],
        }
    }

    pub fn intersects_aabb(&self, min: glam::Vec3, max: glam::Vec3) -> bool {
        for plane in self.planes {
            let positive = glam::Vec3::new(
                if plane.normal.x >= 0.0 { max.x } else { min.x },
                if plane.normal.y >= 0.0 { max.y } else { min.y },
                if plane.normal.z >= 0.0 { max.z } else { min.z },
            );
            if plane.distance(positive) < 0.0 {
                return false;
            }
        }
        true
    }
}
```

- [ ] **Step 4: Verify**

Run:

```bash
cargo test render::frustum -- --nocapture
cargo fmt -- --check
cargo check --all-targets
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/render/mod.rs src/render/frustum.rs
git commit -m "feat: add tile frustum culling"
```

---

### Task 4: Refactor Loader into WorldSource and Full-World Compatibility

**Files:**
- Modify: `src/world/loader.rs`

- [ ] **Step 1: Add source structs and compatibility test**

In `src/world/loader.rs`, move the local `ResolvedWay` struct to module scope and make it public within the crate:

```rust
#[derive(Clone, Debug)]
pub struct ResolvedFeature {
    pub tags: HashMap<String, String>,
    pub points: Vec<(f32, f32)>,
    pub elevations: Vec<f32>,
    pub rep_lat: f64,
    pub rep_lon: f64,
}

pub struct WorldSource {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
    pub conv: CoordConverter,
    pub elevation: Option<ElevationData>,
    pub buildings: Vec<ResolvedFeature>,
    pub roads: Vec<ResolvedFeature>,
    pub waters: Vec<ResolvedFeature>,
    pub landuses: Vec<ResolvedFeature>,
}
```

Add this test near existing loader tests:

```rust
#[test]
fn world_source_bbox_center_matches_converter() {
    let source = WorldSource {
        min_lat: 1.0,
        min_lon: 2.0,
        max_lat: 1.1,
        max_lon: 2.2,
        conv: CoordConverter::new(1.0, 2.0),
        elevation: None,
        buildings: Vec::new(),
        roads: Vec::new(),
        waters: Vec::new(),
        landuses: Vec::new(),
    };

    let (cx, cz) = source.conv.bbox_centre(source.min_lat, source.min_lon, source.max_lat, source.max_lon);
    assert!(cx > 0.0);
    assert!(cz < 0.0);
}
```

- [ ] **Step 2: Extract `load_world_source()`**

Refactor the parse/classify portion of `load_world()` into:

```rust
pub fn load_world_source(pbf_path: &Path, srtm_dir: Option<&Path>) -> anyhow::Result<WorldSource> {
    let osm_data = parse_pbf(pbf_path)?;
    let (min_lat, min_lon, max_lat, max_lon) = osm_data
        .bounds
        .ok_or_else(|| anyhow::anyhow!("OSM data has no bounding box"))?;
    let conv = CoordConverter::new(min_lat, min_lon);
    let elevation = match srtm_dir {
        Some(dir) => Some(ElevationData::from_path(dir)?),
        None => None,
    };

    // Move the current way/relation resolution and classification code here.
    // The resulting feature counts and tags should match the pre-refactor path.

    Ok(WorldSource { min_lat, min_lon, max_lat, max_lon, conv, elevation, buildings, roads, waters, landuses })
}
```

Keep the current helper closure logic, but store `ResolvedFeature` instead of the local struct. Keep `ensure_ccw()` applied before returning from `load_world_source()` so all downstream mesh generation uses normalized polygons.

- [ ] **Step 3: Extract full-world mesh generation**

Add:

```rust
pub fn generate_world_mesh(source: &WorldSource) -> WorldMesh {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();
    append_world_mesh(source, &mut verts, &mut idxs);

    let (cx, cz) = source.conv.bbox_centre(source.min_lat, source.min_lon, source.max_lat, source.max_lon);
    let cy = source.elevation_at((source.min_lat + source.max_lat) / 2.0, (source.min_lon + source.max_lon) / 2.0) + 50.0;

    WorldMesh { vertices: verts, indices: idxs, center: (cx, cy, cz) }
}
```

Add an elevation helper:

```rust
impl WorldSource {
    pub fn elevation_at(&self, lat: f64, lon: f64) -> f32 {
        self.elevation
            .as_ref()
            .and_then(|e| e.elevation_at(lat, lon))
            .unwrap_or(0.0) as f32
    }
}
```

Move current terrain/landuse/water/road/building emission into a private `append_world_mesh(source, verts, idxs)` function.

Update `load_world()` to:

```rust
pub fn load_world(pbf_path: &Path, srtm_dir: Option<&Path>) -> anyhow::Result<WorldMesh> {
    let source = load_world_source(pbf_path, srtm_dir)?;
    Ok(generate_world_mesh(&source))
}
```

- [ ] **Step 4: Verify fallback path is unchanged**

Run:

```bash
cargo test world::loader -- --nocapture
cargo run -- --input ../osm-to-bedrock/map_exports/planet_-121.7526,38.63863_-121.72179,38.65671.osm.pbf --srtm-dir ~/.cache/osm-to-bedrock/srtm --no-streaming --screenshot test_images/sacramento_no_streaming_refactor.png --screenshot-delay 3 --auto-exit 5
cargo fmt -- --check
cargo check --all-targets
```

Expected: tests pass and the screenshot shows the same Sacramento scene through the fallback renderer.

- [ ] **Step 5: Commit**

```bash
git add src/world/loader.rs test_images/sacramento_no_streaming_refactor.png
git commit -m "refactor: split world source loading from mesh generation"
```

---

### Task 5: Add Tile Mesh Generation and Feature Indexing

**Files:**
- Modify: `src/world/terrain.rs`
- Modify: `src/world/loader.rs`
- Modify: `src/stream/tile.rs`

- [ ] **Step 1: Add terrain-by-bounds API test**

In `src/world/terrain.rs`, add a small test module if one does not exist:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_terrain_generates_grid_for_bounds() {
        let conv = CoordConverter::new(38.0, -122.0);
        let mut verts = Vec::new();
        let mut idxs = Vec::new();
        generate_terrain_for_world_rect(0.0, -100.0, 100.0, 0.0, 50.0, &conv, None, &mut verts, &mut idxs);
        assert_eq!(verts.len(), 9);
        assert_eq!(idxs.len(), 24);
    }
}
```

- [ ] **Step 2: Implement terrain-by-bounds API**

In `src/world/terrain.rs`, add:

```rust
#[allow(clippy::too_many_arguments)]
pub fn generate_terrain_for_world_rect(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    grid_spacing: f32,
    conv: &CoordConverter,
    elevation: Option<&ElevationData>,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    // Same algorithm as generate_terrain(), but rows/cols and x/z come from
    // the provided world rectangle instead of a lat/lon bbox.
}
```

Implement it by extracting the existing height sampling, normal computation, and index generation from `generate_terrain()` into this bounds-based function. Compute `cols = ((max_x - min_x) / grid_spacing).ceil() as usize + 1` and `rows = ((max_z - min_z) / grid_spacing).abs().ceil() as usize + 1`. For each row use `z = min_z + r as f32 * grid_spacing`.

- [ ] **Step 3: Add tile mesh data structures**

In `src/world/loader.rs`, add:

```rust
#[derive(Clone, Debug)]
pub struct CpuMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct TileMeshSet {
    pub coord: crate::stream::TileCoord,
    pub aabb: crate::stream::TileAabb,
    pub lods: [CpuMesh; 3],
}
```

- [ ] **Step 4: Add feature bbox helpers**

In `src/world/loader.rs`, add:

```rust
fn feature_bbox(feature: &ResolvedFeature) -> Option<(f32, f32, f32, f32)> {
    let mut iter = feature.points.iter();
    let &(first_x, first_z) = iter.next()?;
    let (mut min_x, mut max_x) = (first_x, first_x);
    let (mut min_z, mut max_z) = (first_z, first_z);
    for &(x, z) in iter {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    Some((min_x, min_z, max_x, max_z))
}
```

- [ ] **Step 5: Add tile feature index**

In `src/stream/tile.rs`, add:

```rust
#[derive(Clone, Debug, Default)]
pub struct TileFeatureRefs {
    pub buildings: Vec<usize>,
    pub roads: Vec<usize>,
    pub waters: Vec<usize>,
    pub landuses: Vec<usize>,
}
```

In `src/world/loader.rs`, add a method:

```rust
impl WorldSource {
    pub fn feature_index_for_tile_size(
        &self,
        tile_size: f32,
    ) -> HashMap<crate::stream::TileCoord, crate::stream::tile::TileFeatureRefs> {
        let mut index = HashMap::new();
        // Seed terrain-only tiles for the whole world bbox using half-open max bounds.
        // For each non-terrain feature, compute feature_bbox(), choose one deterministic
        // owner tile from the bbox center, and push the feature index into only that
        // owner's TileFeatureRefs. Do not duplicate full feature geometry into every
        // touched tile because adjacent rendered tiles would z-fight.
        index
    }
}
```

Required regression coverage for this step:

- a source with no features still seeds terrain tile entries for the world bbox,
- a bbox ending exactly on a tile boundary uses half-open max semantics and does not seed the outside max tile,
- a feature spanning two tiles is owned/emitted by one tile only.

- [ ] **Step 6: Add tile mesh generation**

In `src/world/loader.rs`, add:

```rust
pub fn generate_tile_mesh_set(
    source: &WorldSource,
    coord: crate::stream::TileCoord,
    refs: &crate::stream::tile::TileFeatureRefs,
    tile_size: f32,
) -> TileMeshSet {
    let lods = [
        generate_tile_lod_mesh(source, coord, refs, tile_size, crate::stream::TileLod::Near),
        generate_tile_lod_mesh(source, coord, refs, tile_size, crate::stream::TileLod::Mid),
        generate_tile_lod_mesh(source, coord, refs, tile_size, crate::stream::TileLod::Far),
    ];
    let aabb = aabb_from_lod_vertices(&lods).unwrap_or_else(|| {
        let rect = coord.rect(tile_size);
        crate::stream::TileAabb {
            min: glam::Vec3::new(rect.min_x, 0.0, rect.min_z),
            max: glam::Vec3::new(rect.max_x, 1.0, rect.max_z),
        }
    });
    TileMeshSet { coord, aabb, lods }
}
```

Compute the tile AABB from actual generated vertices across all LODs, including negative Y and any owner-tile geometry that extends outside the nominal tile rect.

Implement `generate_tile_lod_mesh()` by reusing the same emission order as `append_world_mesh()`:

1. terrain using `generate_terrain_for_world_rect()` and `LodConfig::terrain_spacing(lod)`,
2. landuse from `refs.landuses`, skipping green overlays only for Far LOD,
3. water from `refs.waters` using `generate_water_with_elevations`,
4. roads from `refs.roads`, skipping footway/path/cycleway/steps only for Far LOD,
5. buildings from `refs.buildings`.

Road geometry is emitted only for owner-tile roads, but dead-end cap endpoint counts must be computed from the global `source.roads` set so cross-tile connected roads are not misclassified as dead ends. Add a regression test for connected cross-tile roads not producing a false dead-end cap at their shared endpoint.

- [ ] **Step 7: Verify tile mesh generation**

Run:

```bash
cargo test world::terrain -- --nocapture
cargo test stream::tile -- --nocapture
cargo check --all-targets
cargo fmt -- --check
```

Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/world/terrain.rs src/world/loader.rs src/stream/tile.rs
git commit -m "feat: generate lod meshes for world tiles"
```

---

### Task 6: Add GPU Tile Buffers, Upload Budget, and Cache Eviction

**Files:**
- Create: `src/render/tile_buffers.rs`
- Modify: `src/render/mod.rs`

- [ ] **Step 1: Export tile buffers module**

Add to `src/render/mod.rs`:

```rust
pub mod tile_buffers;
```

- [ ] **Step 2: Write budget and eviction tests**

Create `src/render/tile_buffers.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::TileCoord;

    #[test]
    fn upload_budget_allows_one_oversized_tile() {
        let sizes = [10_u64 * 1024 * 1024];
        assert_eq!(select_upload_count(&sizes, 4 * 1024 * 1024), 1);
    }

    #[test]
    fn upload_budget_stops_before_exceeding_budget_after_first() {
        let sizes = [1_u64, 2, 10, 1];
        assert_eq!(select_upload_count(&sizes, 4), 2);
    }

    #[test]
    fn eviction_skips_visible_tiles() {
        let mut candidates = vec![
            EvictionCandidate { coord: TileCoord { x: 0, z: 0 }, visible: true, last_used_frame: 1, distance_sq: 10.0, outside_radius: true },
            EvictionCandidate { coord: TileCoord { x: 1, z: 0 }, visible: false, last_used_frame: 2, distance_sq: 20.0, outside_radius: true },
        ];
        let evicted = choose_eviction_order(&mut candidates);
        assert_eq!(evicted, vec![TileCoord { x: 1, z: 0 }]);
    }
}
```

- [ ] **Step 3: Implement upload helpers and structs**

In `src/render/tile_buffers.rs`, add:

```rust
use std::collections::HashMap;
use wgpu::util::DeviceExt;

use crate::stream::{TileAabb, TileCoord, TileLod};
use crate::world::loader::{CpuMesh, TileMeshSet};

pub struct GpuTileLod {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

pub struct GpuTile {
    pub coord: TileCoord,
    pub aabb: TileAabb,
    pub lods: [GpuTileLod; 3],
    pub current_lod: TileLod,
    pub last_used_frame: u64,
    pub estimated_bytes: u64,
}

#[derive(Default)]
pub struct UploadedTileCache {
    pub tiles: HashMap<TileCoord, GpuTile>,
    pub estimated_bytes: u64,
}

pub fn cpu_mesh_size_bytes(mesh: &CpuMesh) -> u64 {
    (mesh.vertices.len() * std::mem::size_of::<crate::render::vertex::Vertex>()
        + mesh.indices.len() * std::mem::size_of::<u32>()) as u64
}

pub fn mesh_upload_size_bytes(mesh: &TileMeshSet) -> u64 {
    mesh.lods.iter().map(cpu_mesh_size_bytes).sum()
}

pub fn select_upload_count(sizes: &[u64], budget_bytes: u64) -> usize {
    let mut used = 0;
    let mut count = 0;
    for &size in sizes {
        if count > 0 && used + size > budget_bytes {
            break;
        }
        used += size;
        count += 1;
    }
    count
}
```

- [ ] **Step 4: Implement GPU upload**

Add:

```rust
fn upload_lod(device: &wgpu::Device, coord: TileCoord, lod: TileLod, mesh: &CpuMesh) -> GpuTileLod {
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("tile {:?} {:?} vertex buffer", coord, lod)),
        contents: bytemuck::cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("tile {:?} {:?} index buffer", coord, lod)),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    GpuTileLod { vertex_buffer, index_buffer, index_count: mesh.indices.len() as u32 }
}

pub fn upload_tile(device: &wgpu::Device, mesh: TileMeshSet, frame: u64) -> GpuTile {
    let estimated_bytes = mesh_upload_size_bytes(&mesh);
    let lods = [
        upload_lod(device, mesh.coord, TileLod::Near, &mesh.lods[0]),
        upload_lod(device, mesh.coord, TileLod::Mid, &mesh.lods[1]),
        upload_lod(device, mesh.coord, TileLod::Far, &mesh.lods[2]),
    ];
    GpuTile { coord: mesh.coord, aabb: mesh.aabb, lods, current_lod: TileLod::Near, last_used_frame: frame, estimated_bytes }
}
```

- [ ] **Step 5: Implement eviction ordering**

Add:

```rust
#[derive(Clone, Copy, Debug)]
pub struct EvictionCandidate {
    pub coord: TileCoord,
    pub visible: bool,
    pub last_used_frame: u64,
    pub distance_sq: f32,
    pub outside_radius: bool,
}

pub fn choose_eviction_order(candidates: &mut [EvictionCandidate]) -> Vec<TileCoord> {
    candidates.sort_by(|a, b| {
        b.outside_radius.cmp(&a.outside_radius)
            .then_with(|| a.visible.cmp(&b.visible))
            .then_with(|| a.last_used_frame.cmp(&b.last_used_frame))
            .then_with(|| b.distance_sq.total_cmp(&a.distance_sq))
    });
    candidates.iter().filter(|c| !c.visible).map(|c| c.coord).collect()
}
```

- [ ] **Step 6: Verify**

Run:

```bash
cargo test render::tile_buffers -- --nocapture
cargo fmt -- --check
cargo check --all-targets
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/render/mod.rs src/render/tile_buffers.rs
git commit -m "feat: add gpu tile buffer cache helpers"
```

---

### Task 7: Implement TileStreamer Workers and State Machine

**Files:**
- Create: `src/stream/worker.rs`
- Create: `src/stream/streamer.rs`
- Modify: `src/stream/mod.rs`

- [ ] **Step 1: Export worker and streamer modules**

In `src/stream/mod.rs`, add:

```rust
pub mod streamer;
pub mod worker;

pub use streamer::{StreamingStats, TileStreamer};
```

- [ ] **Step 2: Add worker message types**

Create `src/stream/worker.rs`:

```rust
use std::sync::Arc;

use crate::stream::tile::{TileCoord, TileFeatureRefs};
use crate::world::loader::{TileMeshSet, WorldSource, generate_tile_mesh_set};

#[derive(Clone)]
pub struct TileRequest {
    pub coord: TileCoord,
    pub refs: TileFeatureRefs,
    pub tile_size: f32,
    pub epoch: u64,
}

pub struct TileResult {
    pub coord: TileCoord,
    pub epoch: u64,
    pub result: anyhow::Result<TileMeshSet>,
}

pub fn spawn_workers(
    count: usize,
    source: Arc<WorldSource>,
    request_rx: crossbeam_channel::Receiver<TileRequest>,
    result_tx: crossbeam_channel::Sender<TileResult>,
) {
    for idx in 0..count {
        let source = Arc::clone(&source);
        let request_rx = request_rx.clone();
        let result_tx = result_tx.clone();
        std::thread::Builder::new()
            .name(format!("osm tile worker {idx}"))
            .spawn(move || {
                while let Ok(req) = request_rx.recv() {
                    let result = Ok(generate_tile_mesh_set(&source, req.coord, &req.refs, req.tile_size));
                    let _ = result_tx.send(TileResult { coord: req.coord, epoch: req.epoch, result });
                }
            })
            .expect("spawn tile worker");
    }
}
```

- [ ] **Step 3: Add streamer tests**

Create `src/stream/streamer.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_results_are_counted_and_ignored() {
        let mut states = std::collections::HashMap::new();
        states.insert(TileCoord { x: 0, z: 0 }, TileState::Queued { epoch: 2 });
        assert!(!result_matches_tile_epoch(&states, TileCoord { x: 0, z: 0 }, 1));
        assert!(result_matches_tile_epoch(&states, TileCoord { x: 0, z: 0 }, 2));
    }
}
```

- [ ] **Step 4: Implement streamer state structs**

Above tests in `src/stream/streamer.rs`, add:

```rust
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};

use crate::stream::tile::{TileCoord, TileFeatureRefs};
use crate::stream::worker::{TileRequest, TileResult, spawn_workers};
use crate::world::loader::{TileMeshSet, WorldSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileState {
    Unrequested,
    Queued { epoch: u64 },
    Generating { epoch: u64 },
    CpuReady { epoch: u64 },
    Uploaded,
    Failed,
}

#[derive(Clone, Debug, Default)]
pub struct StreamingStats {
    pub desired_tiles: usize,
    pub queued_tiles: usize,
    pub generating_tiles: usize,
    pub cpu_ready_tiles: usize,
    pub uploaded_tiles: usize,
    pub failed_tiles: usize,
    pub visible_tiles: usize,
    pub lod_draw_counts: [usize; 3],
    pub uploaded_mb: f32,
    pub uploaded_this_frame: usize,
    pub uploaded_bytes_this_frame: u64,
    pub evictions_this_frame: usize,
    pub stale_results: usize,
}

pub struct TileStreamer {
    pub source: Arc<WorldSource>,
    pub feature_index: HashMap<TileCoord, TileFeatureRefs>,
    pub states: HashMap<TileCoord, TileState>,
    pub cpu_ready: VecDeque<TileMeshSet>,
    request_tx: Sender<TileRequest>,
    result_rx: Receiver<TileResult>,
    pub tile_size: f32,
    pub stream_radius: f32,
    pub epoch: u64,
    pub stats: StreamingStats,
}
```

- [ ] **Step 5: Implement constructor and result epoch helper**

Add:

```rust
pub fn result_matches_tile_epoch(
    states: &HashMap<TileCoord, TileState>,
    coord: TileCoord,
    epoch: u64,
) -> bool {
    matches!(
        states.get(&coord),
        Some(TileState::Queued { epoch: e } | TileState::Generating { epoch: e } | TileState::CpuReady { epoch: e }) if *e == epoch
    )
}

impl TileStreamer {
    pub fn new(source: WorldSource, tile_size: f32, stream_radius: f32) -> Self {
        let source = Arc::new(source);
        let feature_index = source.feature_index_for_tile_size(tile_size);
        let (request_tx, request_rx) = crossbeam_channel::unbounded();
        let (result_tx, result_rx) = crossbeam_channel::unbounded();
        let workers = std::thread::available_parallelism().map(|n| n.get().saturating_sub(1).max(1)).unwrap_or(1);
        spawn_workers(workers, Arc::clone(&source), request_rx, result_tx);
        Self { source, feature_index, states: HashMap::new(), cpu_ready: VecDeque::new(), request_tx, result_rx, tile_size, stream_radius, epoch: 1, stats: StreamingStats::default() }
    }
}
```

- [ ] **Step 6: Implement desired tile queueing and result processing**

Add methods:

```rust
impl TileStreamer {
    pub fn update_desired_tiles(&mut self, camera_pos: glam::Vec3) {
        let center = TileCoord::from_world(camera_pos.x, camera_pos.z, self.tile_size);
        let radius_tiles = (self.stream_radius / self.tile_size).ceil() as i32;
        let mut desired = Vec::new();
        for z in center.z - radius_tiles..=center.z + radius_tiles {
            for x in center.x - radius_tiles..=center.x + radius_tiles {
                let coord = TileCoord { x, z };
                if !self.feature_index.contains_key(&coord) {
                    continue;
                }
                let dist = coord.center(self.tile_size).distance_squared(camera_pos);
                if dist <= self.stream_radius * self.stream_radius {
                    desired.push((dist, coord));
                }
            }
        }
        desired.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.stats.desired_tiles = desired.len();
        for (_, coord) in desired {
            if matches!(self.states.get(&coord), Some(TileState::Queued { .. } | TileState::Generating { .. } | TileState::CpuReady { .. } | TileState::Uploaded)) {
                continue;
            }
            if let Some(refs) = self.feature_index.get(&coord).cloned() {
                let epoch = self.epoch;
                self.states.insert(coord, TileState::Queued { epoch });
                let _ = self.request_tx.send(TileRequest { coord, refs, tile_size: self.tile_size, epoch });
            }
        }
        self.recount_states();
    }

    pub fn process_results(&mut self) {
        while let Ok(result) = self.result_rx.try_recv() {
            if !result_matches_tile_epoch(&self.states, result.coord, result.epoch) {
                self.stats.stale_results += 1;
                continue;
            }
            match result.result {
                Ok(mesh) => {
                    self.states.insert(result.coord, TileState::CpuReady { epoch: result.epoch });
                    self.cpu_ready.push_back(mesh);
                }
                Err(e) => {
                    log::error!("tile {:?} failed: {e:#}", result.coord);
                    self.states.insert(result.coord, TileState::Failed);
                }
            }
        }
        self.recount_states();
    }

    fn recount_states(&mut self) {
        self.stats.queued_tiles = 0;
        self.stats.generating_tiles = 0;
        self.stats.cpu_ready_tiles = self.cpu_ready.len();
        self.stats.failed_tiles = 0;
        for state in self.states.values() {
            match state {
                TileState::Queued { .. } => self.stats.queued_tiles += 1,
                TileState::Generating { .. } => self.stats.generating_tiles += 1,
                TileState::Failed => self.stats.failed_tiles += 1,
                _ => {}
            }
        }
    }
}
```

- [ ] **Step 7: Verify**

Run:

```bash
cargo test stream::streamer -- --nocapture
cargo fmt -- --check
cargo check --all-targets
```

Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/stream/mod.rs src/stream/worker.rs src/stream/streamer.rs
git commit -m "feat: add background tile streamer"
```

---

### Task 8: Integrate Streaming into App Update and Render Loop

**Files:**
- Modify: `src/app/mod.rs`
- Modify: `src/app/init.rs`
- Modify: `src/app/update.rs`
- Modify: `src/app/render_loop.rs`

- [ ] **Step 1: Add AppState streaming fields**

In `src/app/init.rs`, add imports:

```rust
use crate::render::tile_buffers::UploadedTileCache;
use crate::stream::TileStreamer;
```

Add fields to `AppState`:

```rust
pub streamer: Option<TileStreamer>,
pub uploaded_tiles: UploadedTileCache,
pub frame_index: u64,
pub streaming_options: crate::app::StreamingOptions,
```

- [ ] **Step 2: Add an empty scene buffer constructor**

In `src/render/buffers.rs`, add this method to `impl SceneBuffers` so the streaming path has valid zero-draw buffers without rendering the test scene:

```rust
pub fn empty(device: &Device) -> Self {
    let vertex_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("empty scene vertex buffer"),
        size: 4,
        usage: BufferUsages::VERTEX,
        mapped_at_creation: false,
    });
    let index_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("empty scene index buffer"),
        size: 4,
        usage: BufferUsages::INDEX,
        mapped_at_creation: false,
    });
    Self { vertex_buffer, index_buffer, index_count: 0 }
}
```

- [ ] **Step 3: Initialize streaming or fallback**

In `init_wgpu()`, replace the current `scene = match input_path` block with:

```rust
let mut streamer = None;
let scene = match input_path {
    Some(path) if streaming_options.enabled => {
        let srtm = srtm_dir.map(std::path::Path::new);
        let source = crate::world::loader::load_world_source(std::path::Path::new(path), srtm)?;
        camera.position = glam::Vec3::new(5645.5, 122.8, -10505.8);
        camera.yaw = (-124.80_f32).to_radians();
        camera.pitch = (-16.30_f32).to_radians();
        streamer = Some(TileStreamer::new(source, streaming_options.tile_size, streaming_options.stream_radius));
        SceneBuffers::empty(&device)
    }
    Some(path) => {
        let srtm = srtm_dir.map(std::path::Path::new);
        let world = crate::world::loader::load_world(std::path::Path::new(path), srtm)?;
        camera.position = glam::Vec3::new(5645.5, 122.8, -10505.8);
        camera.yaw = (-124.80_f32).to_radians();
        camera.pitch = (-16.30_f32).to_radians();
        SceneBuffers::from_mesh(&device, world.vertices, world.indices)
    }
    None => SceneBuffers::new(&device),
};
```

Update `init_wgpu()` parameters to accept `streaming_options: crate::app::StreamingOptions` and pass it from `event_handler.rs`.

- [ ] **Step 4: Upload ready tiles in update**

In `src/app/update.rs`, after camera movement and before uniforms, add:

```rust
if let Some(streamer) = &mut state.streamer {
    streamer.stats.uploaded_this_frame = 0;
    streamer.stats.uploaded_bytes_this_frame = 0;
    streamer.stats.evictions_this_frame = 0;
    streamer.update_desired_tiles(state.camera.position);
    streamer.process_results();

    let budget = (state.streaming_options.upload_budget_mb * 1024.0 * 1024.0) as u64;
    let mut used = 0_u64;
    while let Some(mesh) = streamer.cpu_ready.front() {
        let size = crate::render::tile_buffers::mesh_upload_size_bytes(mesh);
        if streamer.stats.uploaded_this_frame > 0 && used + size > budget {
            break;
        }
        let mesh = streamer.cpu_ready.pop_front().expect("front exists");
        let coord = mesh.coord;
        let gpu = crate::render::tile_buffers::upload_tile(&state.device, mesh, state.frame_index);
        used += gpu.estimated_bytes;
        state.uploaded_tiles.estimated_bytes += gpu.estimated_bytes;
        state.uploaded_tiles.tiles.insert(coord, gpu);
        streamer.states.insert(coord, crate::stream::streamer::TileState::Uploaded);
        streamer.stats.uploaded_this_frame += 1;
        streamer.stats.uploaded_bytes_this_frame += size;
    }
    streamer.stats.uploaded_tiles = state.uploaded_tiles.tiles.len();
    streamer.stats.uploaded_mb = state.uploaded_tiles.estimated_bytes as f32 / (1024.0 * 1024.0);
}
state.frame_index += 1;
```

- [ ] **Step 5: Draw uploaded visible tiles**

In `src/app/render_loop.rs`, replace the unconditional city draw with:

```rust
pass.set_pipeline(&state.pipeline.pipeline);
pass.set_bind_group(0, &state.camera_bg.group, &[]);

if state.streamer.is_some() {
    let view_proj = state.camera.projection_matrix() * state.camera.view_matrix();
    let frustum = crate::render::frustum::Frustum::from_view_proj(view_proj);
    for tile in state.uploaded_tiles.tiles.values() {
        if !frustum.intersects_aabb(tile.aabb.min, tile.aabb.max) {
            continue;
        }
        let lod = &tile.lods[tile.current_lod as usize];
        pass.set_vertex_buffer(0, lod.vertex_buffer.slice(..));
        pass.set_index_buffer(lod.index_buffer.slice(..), IndexFormat::Uint32);
        pass.draw_indexed(0..lod.index_count, 0, 0..1);
    }
} else {
    pass.set_vertex_buffer(0, state.scene.vertex_buffer.slice(..));
    pass.set_index_buffer(state.scene.index_buffer.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..state.scene.index_count, 0, 0..1);
}
```

- [ ] **Step 6: Add LOD selection before drawing**

Inside the streaming draw loop, compute distance to tile center and pick LOD:

```rust
let distance = tile.coord.center(state.streaming_options.tile_size).distance(state.camera.position);
let selected = crate::stream::LodConfig::default().select(distance, tile.current_lod);
let lod = &tile.lods[selected as usize];
```

Use the computed `selected` LOD for drawing in this task. Task 9 updates per-tile `current_lod` and visible statistics after cache diagnostics are wired in.

- [ ] **Step 7: Verify smoke commands**

Run:

```bash
cargo check --all-targets
cargo run -- --input ../osm-to-bedrock/map_exports/planet_-121.7526,38.63863_-121.72179,38.65671.osm.pbf --srtm-dir ~/.cache/osm-to-bedrock/srtm --no-streaming --screenshot test_images/sacramento_phase3_no_streaming.png --screenshot-delay 3 --auto-exit 5
cargo run -- --input ../osm-to-bedrock/map_exports/planet_-121.7526,38.63863_-121.72179,38.65671.osm.pbf --srtm-dir ~/.cache/osm-to-bedrock/srtm --screenshot test_images/sacramento_phase3_streaming_smoke.png --screenshot-delay 5 --auto-exit 7
cargo fmt -- --check
```

Expected: both screenshots are produced; streaming screenshot shows Sacramento geometry after enough tiles upload.

- [ ] **Step 8: Commit**

```bash
git add src/app/mod.rs src/app/init.rs src/app/update.rs src/app/render_loop.rs src/render/buffers.rs test_images/sacramento_phase3_no_streaming.png test_images/sacramento_phase3_streaming_smoke.png
git commit -m "feat: render streamed gpu tiles"
```

---

### Task 9: Add Cache Limits, Eviction, and Streaming Stats UI

**Files:**
- Modify: `src/app/update.rs`
- Modify: `src/render/tile_buffers.rs`
- Modify: `src/ui/hud.rs`
- Modify: `src/ui/settings.rs`

- [ ] **Step 1: Add cache enforcement helper tests**

In `src/render/tile_buffers.rs`, add a pure helper test for limit checks:

```rust
#[test]
fn cache_over_limit_detects_count_or_bytes() {
    assert!(cache_over_limit(257, 1, 256, 512));
    assert!(cache_over_limit(1, 513, 256, 512));
    assert!(!cache_over_limit(1, 1, 256, 512));
}
```

Add this implementation:

```rust
pub fn cache_over_limit(count: usize, bytes: u64, max_count: usize, max_bytes: u64) -> bool {
    count > max_count || bytes > max_bytes
}
```

- [ ] **Step 2: Enforce eviction after uploads**

In `src/app/update.rs`, after upload loop, build `EvictionCandidate`s for uploaded tiles. Use visible set from the previous render pass if available; if not yet tracked, pass `visible: false` and rely on distance/radius until Task 10 review. Remove tiles from `state.uploaded_tiles.tiles` until count and bytes are within limits:

```rust
let max_bytes = (state.streaming_options.max_uploaded_mb * 1024.0 * 1024.0) as u64;
while crate::render::tile_buffers::cache_over_limit(
    state.uploaded_tiles.tiles.len(),
    state.uploaded_tiles.estimated_bytes,
    state.streaming_options.max_uploaded_tiles,
    max_bytes,
) {
    let mut candidates = state.uploaded_tiles.tiles.values().map(|tile| {
        let distance_sq = tile.coord.center(state.streaming_options.tile_size).distance_squared(state.camera.position);
        crate::render::tile_buffers::EvictionCandidate {
            coord: tile.coord,
            visible: false,
            last_used_frame: tile.last_used_frame,
            distance_sq,
            outside_radius: distance_sq > state.streaming_options.stream_radius * state.streaming_options.stream_radius,
        }
    }).collect::<Vec<_>>();
    let Some(coord) = crate::render::tile_buffers::choose_eviction_order(&mut candidates).into_iter().next() else { break; };
    if let Some(tile) = state.uploaded_tiles.tiles.remove(&coord) {
        state.uploaded_tiles.estimated_bytes = state.uploaded_tiles.estimated_bytes.saturating_sub(tile.estimated_bytes);
        if let Some(streamer) = &mut state.streamer {
            streamer.states.remove(&coord);
            streamer.stats.evictions_this_frame += 1;
        }
    }
}
```

- [ ] **Step 3: Add stats snapshot to AppState**

In `src/app/init.rs`, add:

```rust
pub streaming_stats: crate::stream::StreamingStats,
```

At the end of update, copy:

```rust
if let Some(streamer) = &state.streamer {
    state.streaming_stats = streamer.stats.clone();
}
```

- [ ] **Step 4: Draw concise HUD stats**

In `src/ui/hud.rs`, update the HUD draw function signature to accept `Option<&StreamingStats>` and show one line:

```rust
if let Some(stats) = streaming {
    ui.label(format!(
        "Tiles: desired {} uploaded {} visible {} | upload {:.1} MiB",
        stats.desired_tiles,
        stats.uploaded_tiles,
        stats.visible_tiles,
        stats.uploaded_mb,
    ));
}
```

Update the call site in `render_loop.rs` to pass `state.streamer.as_ref().map(|_| &state.streaming_stats)`.

- [ ] **Step 5: Add settings Streaming section**

In `src/ui/settings.rs`, update the draw signature to accept `Option<&StreamingStats>` and add a default-open collapsing section:

```rust
egui::CollapsingHeader::new("Streaming")
    .default_open(true)
    .show(ui, |ui| {
        if let Some(stats) = streaming {
            ui.label(format!("Desired: {}", stats.desired_tiles));
            ui.label(format!("Queued: {}", stats.queued_tiles));
            ui.label(format!("CPU ready: {}", stats.cpu_ready_tiles));
            ui.label(format!("Uploaded: {} ({:.1} MiB)", stats.uploaded_tiles, stats.uploaded_mb));
            ui.label(format!("Visible: {}", stats.visible_tiles));
            ui.label(format!("LOD draws: {:?}", stats.lod_draw_counts));
            ui.label(format!("Uploaded this frame: {} tiles / {} bytes", stats.uploaded_this_frame, stats.uploaded_bytes_this_frame));
            ui.label(format!("Evictions this frame: {}", stats.evictions_this_frame));
            ui.label(format!("Stale results: {}", stats.stale_results));
        } else {
            ui.label("Streaming disabled");
        }
    });
```

- [ ] **Step 6: Verify UI and cache tests**

Run:

```bash
cargo test render::tile_buffers -- --nocapture
cargo run -- --input ../osm-to-bedrock/map_exports/planet_-121.7526,38.63863_-121.72179,38.65671.osm.pbf --srtm-dir ~/.cache/osm-to-bedrock/srtm --show-settings --screenshot test_images/sacramento_phase3_streaming_stats.png --screenshot-delay 5 --auto-exit 7
cargo fmt -- --check
cargo check --all-targets
```

Expected: screenshot shows settings panel with Streaming section and non-zero uploaded tile stats.

- [ ] **Step 7: Commit**

```bash
git add src/app/update.rs src/render/tile_buffers.rs src/ui/hud.rs src/ui/settings.rs src/app/init.rs src/app/render_loop.rs test_images/sacramento_phase3_streaming_stats.png
git commit -m "feat: add streaming cache limits and diagnostics"
```

---

### Task 10: Final Phase 3 Verification and Cleanup

**Files:**
- Modify only files required by verification failures.
- Update: `docs/superpowers/specs/2026-05-02-phase3-streaming-lod-design.md` only if implementation intentionally diverged.

- [ ] **Step 1: Run graph and full checks**

Run:

```bash
graphify update . && cargo fmt && make checkall
```

Expected: graphify completes and all format/check/clippy/test targets pass.

- [ ] **Step 2: Capture final Sacramento streaming screenshot**

Run:

```bash
cargo run -- \
  --input ../osm-to-bedrock/map_exports/planet_-121.7526,38.63863_-121.72179,38.65671.osm.pbf \
  --srtm-dir ~/.cache/osm-to-bedrock/srtm \
  --cam-x=5770.1 --cam-y=72.3 --cam-z=-11003.9 \
  --cam-yaw=-115.9 --cam-pitch=-45.3 \
  --screenshot test_images/sacramento_phase3_streaming.png \
  --screenshot-delay 5 \
  --auto-exit 7
```

Expected: screenshot contains terrain, landuse, roads, buildings, and no obvious Phase 4 regression.

- [ ] **Step 3: Capture final fallback screenshot**

Run:

```bash
cargo run -- \
  --input ../osm-to-bedrock/map_exports/planet_-121.7526,38.63863_-121.72179,38.65671.osm.pbf \
  --srtm-dir ~/.cache/osm-to-bedrock/srtm \
  --no-streaming \
  --cam-x=5770.1 --cam-y=72.3 --cam-z=-11003.9 \
  --cam-yaw=-115.9 --cam-pitch=-45.3 \
  --screenshot test_images/sacramento_phase3_no_streaming_final.png \
  --screenshot-delay 3 \
  --auto-exit 5
```

Expected: fallback still works.

- [ ] **Step 4: Inspect status and commit verification artifacts if desired**

Run:

```bash
git status --short
git diff --check
```

Expected: no whitespace errors. If screenshots are intentionally tracked, add them. If `test_images/` is not intended for commits, leave screenshots untracked and mention paths in final summary.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: complete phase 3 streaming lod renderer"
```

If there are no code/doc changes after prior task commits, skip this commit and report the existing commit list.

- [ ] **Step 6: Request code review**

Use the requesting-code-review skill or a focused reviewer subagent to inspect:

- worker/thread safety,
- stale epoch handling,
- GPU upload/cache accounting,
- render-loop correctness,
- fallback path preservation,
- Sacramento visual artifacts.

Address blocking findings, rerun `graphify update . && cargo fmt && make checkall`, and commit fixes.
