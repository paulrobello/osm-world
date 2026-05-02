# Phase 3: Streaming + LOD — Design Spec

## Context

Phase 1 built the WGPU app shell, camera, shaders, and render loop. Phase 2 loaded OSM/SRTM data into one synchronous world mesh. Phase 4 added egui diagnostics/settings and fixed the Sacramento visual artifacts found during screenshot-driven QA.

Phase 3 now turns the current single-buffer renderer into a full-city renderer. The target dataset is the Sacramento export used during visual QA:

```bash
../osm-to-bedrock/map_exports/planet_-121.7526,38.63863_-121.72179,38.65671.osm.pbf
```

with SRTM data from:

```bash
~/.cache/osm-to-bedrock/srtm
```

Sacramento remains the primary verification scene because it includes rivers, bridges, overpasses, large buildings, parks, landuse overlays, and road/path edge cases already exercised by Phase 4.

## Goals

1. Split the rendered world into spatial tiles instead of one monolithic `SceneBuffers`.
2. Load/generate tile meshes in background workers.
3. Upload tile buffers to the GPU under a per-frame upload budget.
4. Maintain a bounded uploaded-tile cache with distance-priority loading and LRU eviction.
5. Generate and render Near/Mid/Far LODs per tile.
6. Frustum-cull uploaded tiles before draw submission.
7. Expose streaming diagnostics in the egui HUD/settings overlay.
8. Preserve all Phase 4 visual fixes: building winding, road ribbons/caps, landuse offsets, default daytime, settings visibility, and screenshot CLI support.

## Non-Goals

- No network streaming from Overpass in this phase. Phase 3 operates on an already available `.osm.pbf` input.
- No procedural simplification algorithms beyond deterministic LOD knobs defined here.
- No occlusion culling.
- No bridge/overpass semantic rendering beyond preserving existing feature geometry and visual layering.
- No multi-city cache format on disk. All tile cache state is in memory for this phase.

## High-Level Architecture

Current flow:

```text
input .osm.pbf + SRTM → world::loader::load_world() → WorldMesh → SceneBuffers → one draw_indexed
```

Phase 3 flow:

```text
input .osm.pbf + SRTM
  → world::loader parses and classifies source features once
  → stream::TileStreamer owns tile state, worker queues, uploaded cache
  → workers generate TileMeshSet { near, mid, far } for requested tile coordinates
  → render::tile_buffers uploads TileMeshSet under a per-frame byte budget
  → render loop selects visible uploaded tiles + LOD and draws many buffers
```

The parser/classifier remains deterministic and CPU-only. GPU resources stay on the render thread. Worker threads produce CPU mesh data only.

## Tile Model

- Tile size: `1000.0m × 1000.0m` in world X/Z coordinates.
- Tile coordinate: `(i32 x, i32 z)` where:
  - `tile_x = floor(world_x / 1000.0)`
  - `tile_z = floor(world_z / 1000.0)`
- Tile bounds are half-open in X/Z: `[min_x, max_x) × [min_z, max_z)`.
- A feature is assigned to every tile touched by its X/Z bounding box.
- For polygons and long roads, each assigned tile initially receives the full feature geometry. This avoids clipping artifacts in Phase 3 and keeps correctness ahead of optimization. Later phases can add geometric clipping.
- Terrain is generated per tile from tile bounds plus the shared coordinate converter and SRTM elevation source.

## LOD Levels

Each generated tile contains three mesh variants:

| LOD | Name | Distance from camera to tile center | Terrain spacing | Roads | Buildings | Landuse/Water |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | Near | `< 2km` | `10m` | full width/detail | full mesh | full polygons |
| 1 | Mid | `2km..5km` | `50m` | same centerline width, duplicate points filtered | full building mesh | full polygons |
| 2 | Far | `>= 5km` | `100m` | omit footways/paths, keep major/residential roads | full building mesh | omit green overlays, keep water/base landuse |

The first implementation deliberately keeps buildings as full meshes in all LODs. This preserves visual correctness while moving the renderer to a tile/LOD architecture. Building simplification can be a later focused phase once streaming is stable.

LOD selection is computed every frame from camera position to tile AABB/center. Hysteresis should be added to avoid flicker:

- Near → Mid only after `> 2200m`.
- Mid → Near only after `< 1800m`.
- Mid → Far only after `> 5500m`.
- Far → Mid only after `< 4500m`.

## Frustum Culling

Each uploaded tile stores an AABB:

```rust
pub struct TileAabb {
    pub min: glam::Vec3,
    pub max: glam::Vec3,
}
```

The render loop builds a frustum from the camera view-projection matrix each frame and culls uploaded tiles before drawing. Culling happens before LOD draw selection.

The frustum implementation lives in `src/render/frustum.rs` and exposes:

```rust
pub struct Frustum { /* six normalized planes */ }

impl Frustum {
    pub fn from_view_proj(view_proj: glam::Mat4) -> Self;
    pub fn intersects_aabb(&self, min: glam::Vec3, max: glam::Vec3) -> bool;
}
```

Unit tests cover inside, outside, and intersecting AABBs.

## Streaming State Machine

Each tile has one state:

```rust
pub enum TileState {
    Unrequested,
    Queued { epoch: u64 },
    Generating { epoch: u64 },
    CpuReady { epoch: u64 },
    Uploaded,
    Failed,
}
```

The `TileStreamer` owns:

- source feature data parsed once at startup,
- tile index from tile coord to feature IDs,
- desired radius and max render distance,
- worker request channel,
- worker result channel,
- uploaded tile cache,
- current epoch,
- stats for egui.

When camera movement changes the desired tile set, `TileStreamer` increments an epoch. Workers include the epoch in results. The main thread discards stale results whose epoch does not match the current tile epoch.

## Loading Priority

Every update computes desired tiles within `15km` of the camera, sorted by:

1. tile containing camera,
2. lower distance to camera,
3. currently visible/frustum-intersecting before non-visible,
4. lower LOD distance band before farther bands.

The queue should avoid duplicate work. A tile already queued, generating, CPU-ready, or uploaded is not requeued unless its source epoch changes.

## GPU Upload Budget

GPU resources are created only on the render thread.

- Default per-frame upload budget: `4 MiB` of vertex + index data.
- Upload at most as many CPU-ready tiles as fit in the budget.
- Always allow one tile larger than the budget to upload so the system cannot deadlock on a large downtown tile.
- Uploaded tile buffers are immutable for this phase.

The upload module exposes byte-size estimates before buffer creation:

```rust
pub fn mesh_upload_size_bytes(mesh: &TileMeshSet) -> u64;
```

## Tile Cache and Eviction

The uploaded cache is bounded by both count and estimated GPU bytes:

- Default max uploaded tiles: `256`.
- Default max uploaded bytes: `512 MiB`.

Eviction policy:

1. Never evict tiles visible this frame.
2. Prefer evicting tiles outside the desired radius.
3. Then evict least-recently-used tiles with greatest distance from camera.
4. Dropping a `GpuTile` releases its WGPU buffers.

## Render Data Structures

New render-side structures:

```rust
pub struct GpuTile {
    pub coord: TileCoord,
    pub aabb: TileAabb,
    pub lods: [GpuTileLod; 3],
    pub last_used_frame: u64,
    pub estimated_bytes: u64,
}

pub struct GpuTileLod {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}
```

The render loop draws uploaded visible tiles like this:

```text
for visible tile:
  choose lod from camera distance + hysteresis
  set vertex/index buffers
  draw_indexed
```

The current single `SceneBuffers` path remains available for `--no-streaming` debug mode during the transition.

## Source Feature Refactor

The current `world::loader::load_world()` both parses/classifies OSM data and emits one `WorldMesh`. Phase 3 splits this into two layers:

1. `load_world_source(path, srtm_dir) -> WorldSource`
   - parses OSM data,
   - loads elevation data,
   - converts nodes to world coordinates,
   - classifies buildings, roads, landuse, and water,
   - stores feature geometry and elevations.

2. `generate_tile_mesh_set(source, tile_coord, lod_config) -> TileMeshSet`
   - generates terrain for tile bounds,
   - emits features assigned to the tile,
   - applies existing Phase 4 geometry fixes,
   - returns Near/Mid/Far CPU meshes.

`load_world()` can then become a compatibility wrapper that calls `load_world_source()` and generates a single full-world mesh.

## Egui Diagnostics

Add a compact "Streaming" section to the existing settings/HUD UI:

- streaming enabled/disabled,
- current camera tile,
- desired tile count,
- queued/generating/CPU-ready/uploaded/failed counts,
- visible tile count,
- LOD0/LOD1/LOD2 draw counts,
- uploaded GPU MB estimate,
- tiles uploaded this frame,
- bytes uploaded this frame,
- evictions this frame.

The HUD should stay concise; detailed stats can live in the settings panel.

## CLI Flags

Add conservative debug flags:

```text
--no-streaming                Use existing single-mesh path
--tile-size <metres>          Defaults to 1000.0
--stream-radius <metres>      Defaults to 15000.0
--upload-budget-mb <mb>       Defaults to 4.0
--max-uploaded-tiles <count>  Defaults to 256
--max-uploaded-mb <mb>        Defaults to 512.0
```

Defaults should enable streaming for OSM input once Phase 3 is complete.

## Error Handling

- Parse/load failures remain fatal during startup.
- Worker tile-generation failures mark the tile `Failed` and record the error string in stats/logs.
- Stale worker results are discarded without logging as errors.
- GPU upload failures are not expected with valid meshes; validation issues should surface through WGPU and tests.

## Testing Strategy

Unit tests:

- tile coordinate conversion for positive/negative world coordinates,
- feature bbox to tile assignment,
- LOD distance thresholds and hysteresis,
- frustum vs AABB intersection,
- upload budget selection including the "one oversized tile may upload" rule,
- LRU eviction skips visible tiles,
- stale epoch worker results are discarded.

Integration/smoke tests:

- existing `make checkall`,
- Sacramento screenshot with streaming enabled,
- Sacramento screenshot with `--no-streaming` for comparison during transition,
- short auto-exit run with stats logging enabled.

Primary visual verification command:

```bash
cargo run -- \
  --input ../osm-to-bedrock/map_exports/planet_-121.7526,38.63863_-121.72179,38.65671.osm.pbf \
  --srtm-dir ~/.cache/osm-to-bedrock/srtm \
  --cam-x=5770.1 --cam-y=72.3 --cam-z=-11003.9 \
  --cam-yaw=-115.9 --cam-pitch=-45.3 \
  --screenshot test_images/sacramento_phase3_streaming.png \
  --screenshot-delay 3 \
  --auto-exit 5
```

## Acceptance Criteria

Phase 3 is complete when:

1. Streaming is the default path for OSM input.
2. The old single-mesh path still works behind `--no-streaming`.
3. Sacramento renders with visible buildings, roads, landuse, water, and terrain.
4. Egui shows non-zero streaming stats for desired/uploaded/visible tiles.
5. Frustum culling reduces draw submissions when looking away from parts of the map.
6. Upload budgets and cache limits are enforced by tests.
7. `graphify update . && cargo fmt && make checkall` passes.
8. A Sacramento streaming screenshot is captured and visually acceptable.

## Risks and Mitigations

- **Large feature duplication across tiles:** assigning full features to each touched tile may duplicate geometry. Mitigation: bounded cache and upload budget keep this acceptable for Phase 3; clipping can be a later optimization.
- **Visual seams between terrain tiles:** generate terrain on shared tile boundaries using the same world coordinates and SRTM sampler. Shared boundaries should sample identical heights.
- **Worker staleness during fast camera movement:** epoch cancellation discards stale results.
- **GPU memory growth:** uploaded byte limit and LRU eviction are mandatory in the first streaming implementation, not optional polish.
- **LOD popping:** use hysteresis; defer smooth cross-fade to a future phase.
