# osm-world: WGPU 3D OSM City Renderer — Design Spec

## Context

Build a Rust application that renders real-world cities in 3D using OpenStreetMap data and SRTM elevation, powered by wgpu 29. The user wants to explore cities in real-time with a flycam, with the architecture supporting a future game engine phase.

The project leverages an existing, battle-tested OSM pipeline from `osm-to-bedrock` (adjacent repo), adapting its data acquisition and parsing modules while replacing the Minecraft Bedrock output with a WGPU mesh renderer.

**Why this project:** The `osm-to-bedrock` pipeline proves the OSM data acquisition works end-to-end. This project reuses that investment to build a proper 3D visualization instead of a block-world output.

## Architecture Overview

```
CLI (bbox or .osm.pbf)
  → osm/   Data acquisition (Overpass, PBF, cache)
  → geo/   Coordinate conversion, elevation, spatial index
  → world/ Mesh generation (OSM features → GPU meshes)
  → stream/ Tile streaming (background loading, GPU upload, eviction)
  → render/ WGPU renderer (pipelines, culling, LOD)
```

Five layers with clean boundaries. Layers 1-2 are adapted from `osm-to-bedrock`. Layers 3-5 are new.

## Module Layout

```
src/
  main.rs                  # CLI entry point (clap), app bootstrap
  lib.rs                   # Re-exports all public modules

  app/
    mod.rs                 # App struct, re-exports
    init.rs                # WGPU init (instance, surface, device, queue)
    render_loop.rs         # Main render loop: encode, submit, present
    update.rs              # Per-frame: camera, tile streaming tick, LOD transitions
    event_handler.rs       # winit 0.30 events: keyboard, mouse, resize

  camera/
    mod.rs                 # Flycam: position, yaw/pitch, speed, view/proj matrices
    controller.rs          # WASD + mouse look input → camera transform

  osm/
    mod.rs                 # Re-exports
    parse.rs               # Adapted from osm-to-bedrock osm.rs
    overpass.rs            # Adapted from overpass.rs
    filter.rs              # Adapted from filter.rs
    params.rs              # Adapted from params.rs (trimmed for 3D engine)

  geo/
    mod.rs                 # Re-exports
    coords.rs              # Adapted from convert.rs: lat/lon → world XZ metres (f32)
    elevation.rs           # Adapted from elevation.rs: SRTM bilinear sampling
    srtm.rs                # Adapted from srtm.rs: auto-download/cache HGT tiles
    spatial.rs             # Adapted from spatial.rs: SpatialIndex grid, HeightMap

  world/
    mod.rs                 # World struct, tile grid management
    tile.rs                # Tile struct, TileState state machine
    mesh.rs                # Coordinator: OsmData + tile bounds → TileMesh
    terrain.rs             # Heightmap → grid mesh
    building.rs            # Building footprint → extruded walls + roof + floor
    road.rs                # Road centerline → ribbon strip mesh
    water.rs               # Water polygon → flat triangulated mesh at y=water_level
    landuse.rs             # Landuse polygon → flat triangulated mesh at terrain height
    feature_color.rs       # OSM tag → RGB color mapping

  render/
    mod.rs                 # Renderer struct, re-exports
    pipelines.rs           # WGSL shaders + wgpu pipeline creation
    vertex.rs              # Vertex struct (bytemuck-compatible)
    buffers.rs             # GPU buffer management per tile
    bind_groups.rs         # Camera uniform, material bind group layouts
    lod.rs                 # Distance-based LOD selection
    frustum.rs             # Six-plane frustum culling (AABB test)

  stream/
    mod.rs                 # Tile streaming coordinator
    loader.rs              # Background thread pool: OSM parse → mesh generation
    upload.rs              # Frame-budgeted GPU upload queue (priority by distance)
    tile_cache.rs          # LRU tile cache, eviction of distant tiles

  shaders/
    city.wgsl              # Main vertex/fragment shader (terrain, buildings, roads, water)
    sky.wgsl               # Sky gradient background
```

## Coordinate System

- **East = +X, North = -Z, Up = +Y** (matches osm-to-bedrock convention)
- **Origin:** SW corner of the requested bbox (lat/lon → world metres)
- **Equirectangular projection:** `dx = (lon - origin_lon) * metres_per_deg_lon`, `dz = -(lat - origin_lat) * 111320`
- **Y (height):** SRTM elevation sampled via bilinear interpolation. Building heights from OSM tags.
- All coordinates in world metres (`f32`), not integer blocks.

## Tile System

- **Tile size:** 1km x 1km (1000m x 1000m)
- **Tile assignment:** OSM ways assigned to tiles via SpatialIndex grid (same pattern as osm-to-bedrock's `TILE_CHUNKS = 64` grid)
- For a 500 km² city: ~500 tiles total, ~50-100 loaded at any time

### LOD Levels (pre-generated per tile)

| LOD | Distance | Terrain grid | Buildings | Roads | Water/Landuse |
|-----|----------|-------------|-----------|-------|---------------|
| 0 | 0-2 km | 10m grid (100×100) | Full detail | Full width | Full detail |
| 1 | 2-5 km | 50m grid (20×20) | Extruded boxes only | Half width | Simplified outline |
| 2 | 5-15 km | 100m grid (10×10) | Single box per cluster | Centerline only | Omitted |

LOD transition: all levels share one vertex buffer. LOD is selected by switching `index_offset` + `index_count` per draw call.

### Culling

- **Frustum culling:** AABB per tile vs 6 planes, CPU-side every frame
- **Distance culling:** Tiles beyond 15km not loaded
- No occlusion culling in initial version

## Key Data Structures

### Vertex (32 bytes)

```rust
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],      // world metres
    pub normal: [f32; 3],        // surface normal
    pub color: [f32; 3],         // flat color (RGB)
    pub feature_type: f32,       // 0=terrain, 1=building, 2=road, 3=water, 4=landuse
}
```

### Tile

```rust
pub struct Tile {
    pub coords: (i32, i32),
    pub bounds: (f32, f32, f32, f32),  // (min_x, min_z, max_x, max_z) in metres
    pub state: TileState,
    pub lod_meshes: [Option<TileMesh>; 3],
    pub aabb: Aabb,
}

pub enum TileState { Unloaded, Loading, Ready, Uploaded }

pub struct GpuTile {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub lod_draw: [(u32, u32); 3],  // (index_offset, index_count) per LOD
    pub aabb: Aabb,
}
```

### Camera Uniform (GPU)

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 3],
    pub _pad: f32,
}
```

## Mesh Generation Algorithms

### Buildings

Adapted from `osm-to-bedrock/src/geometry.rs::draw_building()`:

1. **Walls:** For each edge of the footprint polygon, emit a quad (2 triangles) from y=0 to y=height. Normal = perpendicular outward.
2. **Roof:** Triangulate footprint polygon at y=height via earcutr. Normal = (0, 1, 0).
3. **Floor:** Triangulate footprint at y=0. Only visible from below.
4. **Height:** From OSM tags (`height` → metres, `building:levels` × 3m, default 10m).
5. **Color:** From `building:material` tag (brick=red-brown, concrete=gray, wood=brown, glass=blue-tint, default=beige).

### Roads

Adapted from `osm-to-bedrock/src/geometry.rs::draw_road()`:

1. For each segment of the centerline, compute perpendicular direction.
2. Offset by ±half_width to get left/right edges.
3. Emit quad strip (2 triangles per segment). Normal = (0, 1, 0).
4. **Width:** From OSM tags (motorway=6m, primary=5m, residential=3.5m, footway=1m).
5. **Color:** highway type → gray shade (motorway=dark, residential=light, path=brown).

### Water

1. Triangulate polygon via earcutr at y=water_level (sea level = 0).
2. Normal = (0, 1, 0).
3. Color = blue with slight transparency in shader.

### Landuse

1. Triangulate polygon via earcutr at y=terrain_height.
2. Color from tag: forest=dark green, grass=green, farmland=tan, sand=yellow, industrial=gray.

### Terrain

1. Regular grid within tile bounds. Grid spacing varies by LOD (10m, 50m, 100m).
2. Sample SRTM elevation at each vertex.
3. Compute normals via finite differences of heightmap.
4. Color from dominant landuse tag in tile area, default green-tan.

## Dependencies

```toml
[package]
name = "osm-world"
version = "0.1.0"
edition = "2024"
rust-version = "1.87"

[dependencies]
# GPU rendering
wgpu = "29.0.1"
winit = "0.30.13"

# Math
glam = "0.32.1"
bytemuck = { version = "1.25.0", features = ["derive"] }

# Triangulation
earcutr = "0.4"

# OSM data parsing
osmpbf = "0.3.8"
quick-xml = "0.37"

# HTTP (Overpass API, SRTM download)
reqwest = { version = "0.12", features = ["blocking"] }
urlencoding = "2.1"

# Elevation data
memmap2 = "0.9"
flate2 = "1.1"

# Parallelism
rayon = "1.10"
crossbeam-channel = "0.5"
pollster = "0.4"

# CLI
clap = { version = "4.6", features = ["derive"] }

# Logging
log = "0.4"
env_logger = "0.11"

# Serialization / hashing (for cache)
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "0.10"

# Error handling
anyhow = "1.0"
```

Version matrix proven compatible from vault research (wgpu 29.0.1 + winit 0.30.13 + glam 0.32.1 + bytemuck 1.25.0).

## CLI Interface

```
osm-world --bbox "47.5,8.4,47.6,8.5"          # Fetch from Overpass and render
osm-world --input zurich.osm.pbf                # Load local file and render
osm-world --input zurich.osm.pbf --no-elevation # Skip SRTM download
osm-world --bbox "..." --tile-size 500          # Smaller tiles for dense areas
```

## OSM Modules Adapted from osm-to-bedrock

| Source file | Adapted to | Changes |
|-------------|-----------|---------|
| `src/osm.rs` | `src/osm/parse.rs` | Remove bedrock-specific fields; keep OsmData, OsmNode, OsmWay, OsmRelation, OsmPoiNode, parse_pbf(), parse_osm_xml_str() |
| `src/overpass.rs` | `src/osm/overpass.rs` | As-is: Overpass QL builder, HTTP fetch, SSRF guard |
| `src/osm_cache.rs` | Inline in `src/osm/overpass.rs` | Merge caching into overpass module |
| `src/filter.rs` | `src/osm/filter.rs` | As-is: FeatureFilter |
| `src/params.rs` | `src/osm/params.rs` | Remove bedrock-specific params (sea_level, wall_straighten, snow_line); keep scale, elevation params, spawn coords |
| `src/convert.rs` | `src/geo/coords.rs` | Return f32 metres instead of i32 block coords; keep CoordConverter, equirectangular projection |
| `src/elevation.rs` | `src/geo/elevation.rs` | As-is: SRTM HGT loading, memmap, bilinear interpolation |
| `src/srtm.rs` | `src/geo/srtm.rs` | As-is: auto-download, retry, cache |
| `src/spatial.rs` | `src/geo/spatial.rs` | Adapt SpatialIndex for metre-based grid instead of block-based; keep HeightMap |
| `src/geometry.rs` | `src/world/building.rs`, `road.rs`, `water.rs`, `landuse.rs` | Translate set_block() calls to vertex/append patterns. Keep geometric logic (road widths, building heights, perpendicular computation) |

## Implementation Phases

### Phase 1: Window + Flycam + Test Building

**Goal:** Open a window, see a colored box on a ground plane with flycam controls.

Files to create:
- `main.rs`, `lib.rs`
- `app/mod.rs`, `app/init.rs`, `app/render_loop.rs`, `app/update.rs`, `app/event_handler.rs`
- `camera/mod.rs` (Flycam struct + controller)
- `render/mod.rs`, `render/vertex.rs`, `render/pipelines.rs`, `render/buffers.rs`, `render/bind_groups.rs`
- `shaders/city.wgsl` (basic vertex transform + directional light + vertex color output)

**Verification:** `cargo run` opens a window, renders a colored box on a flat plane, WASD/mouse moves camera.

### Phase 2: OSM Data → Real City (Small Area)

**Goal:** Load a real .osm.pbf or Overpass bbox, render buildings, roads, water, landuse, terrain as colored meshes.

Files to create:
- `osm/parse.rs`, `osm/overpass.rs`, `osm/filter.rs`, `osm/params.rs` (copied + adapted)
- `geo/coords.rs`, `geo/elevation.rs`, `geo/srtm.rs`, `geo/spatial.rs` (copied + adapted)
- `world/mod.rs`, `world/tile.rs`, `world/mesh.rs`, `world/terrain.rs`
- `world/building.rs`, `world/road.rs`, `world/water.rs`, `world/landuse.rs`, `world/feature_color.rs`

All tiles loaded synchronously (no streaming yet). Test with a small neighborhood (~1-4 tiles).

**Verification:** `cargo run -- --bbox "47.37,8.53,8.55,47.38"` renders a recognizable Zurich district.

### Phase 3: Streaming + LOD (Full City)

**Goal:** Load a full city with background tile streaming, LOD, frustum culling.

Files to create:
- `stream/mod.rs`, `stream/loader.rs`, `stream/upload.rs`, `stream/tile_cache.rs`
- `render/lod.rs`, `render/frustum.rs`
- `shaders/sky.wgsl`

Adapt voxel-world's chunk_loader pattern (crossbeam channels, epoch-based cancellation, background thread pool).

**Verification:** `cargo run -- --bbox "47.3,8.4,8.6,47.45"` renders all of Zurich smoothly at 60 FPS.

### Phase 4: Visual Polish

**Goal:** Make it look like a recognizable city.

- Directional lighting (sun) with diffuse shading
- Sky gradient background
- Building color variation from OSM tags
- Water with flat shading
- Terrain colored by landuse
- egui debug overlay (FPS, tile count, camera position)

**Verification:** Screenshots clearly show streets, buildings, parks, and water.

## Verification Plan

1. **Phase 1:** `cargo run` → window opens, test geometry visible, flycam responsive
2. **Phase 2:** `cargo run -- --input test.osm.pbf` → buildings and roads visible at correct positions
3. **Phase 3:** `cargo run -- --bbox "city_bbox"` → full city renders, no frame drops when flying
4. **Phase 4:** Visual inspection — city is recognizable, lighting looks natural
5. **Continuous:** `cargo clippy`, `cargo fmt`, `cargo test` pass at each phase

## Files Copied from osm-to-bedrock

| File | Destination | LOC (approx) |
|------|-------------|------|
| `src/osm.rs` | `src/osm/parse.rs` | ~400 |
| `src/overpass.rs` + `src/osm_cache.rs` | `src/osm/overpass.rs` | ~350 |
| `src/filter.rs` | `src/osm/filter.rs` | ~30 |
| `src/params.rs` | `src/osm/params.rs` | ~80 |
| `src/convert.rs` | `src/geo/coords.rs` | ~200 |
| `src/elevation.rs` | `src/geo/elevation.rs` | ~150 |
| `src/srtm.rs` | `src/geo/srtm.rs` | ~150 |
| `src/spatial.rs` | `src/geo/spatial.rs` | ~200 |

**Total adapted:** ~1560 LOC. **New code:** render/, world/mesh generation, stream/, app/ — estimated ~3000-4000 LOC across all phases.
