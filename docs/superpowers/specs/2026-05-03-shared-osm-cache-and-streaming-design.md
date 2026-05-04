# Shared OSM Cache and Streaming Design

## Summary

Create a new shared Rust crate/repo at `~/Repos/par-osm-rust` that owns reusable OpenStreetMap and terrain data plumbing for both `osm-to-bedrock` and `osm-world`. The crate will be used through local path dependencies at first and can be published later.

The first user-visible osm-world feature will be a web area picker/launcher adapted from osm-to-bedrock. It will fetch/cache selected real-world OSM and SRTM data, launch osm-world when allowed, and always provide a copyable command fallback. The longer-term renderer goal is runtime streaming: while playing, osm-world should fetch missing real-world data into the same cache and stream generated tiles into the running scene.

## Goals

- Share one cache system between `osm-to-bedrock` and `osm-world`.
- Move existing osm-to-bedrock cache data into a neutral shared location:
  - Overpass XML cache: `~/.cache/par-osm-rust/overpass`
  - SRTM HGT cache: `~/.cache/par-osm-rust/srtm`
- Preserve existing environment overrides and add neutral aliases:
  - `PAR_OSM_OVERPASS_CACHE_DIR` then `OVERPASS_CACHE_DIR`
  - `PAR_OSM_SRTM_CACHE_DIR` then `SRTM_CACHE_DIR`
  - `OVERPASS_URL`
- Create `~/Repos/par-osm-rust` as the shared source of truth for OSM/SRTM fetch, parse, clipping, and cache metadata.
- Use local path dependencies until the crate is stable enough to publish.
- Adapt only the useful web UI concepts from osm-to-bedrock: search, bbox drawing, feature filters, cache overlays, data fetch, launch/copy command.
- Omit Minecraft/Bedrock-specific UI and backend features from osm-world.
- Design the shared crate APIs so osm-world can later stream map tiles during gameplay.

## Non-goals

- Do not publish `par-osm-rust` in the first implementation.
- Do not force osm-to-bedrock and osm-world into a Cargo workspace.
- Do not port Minecraft conversion, block preview, spawn point, sea level, world scale, terrain-only conversion, `.mcworld` download, or Overture-to-Minecraft priority settings to osm-world.
- Do not make runtime in-game streaming the first user-visible milestone if it blocks the shared crate and launcher foundation.
- Do not change cache file formats during migration; existing cached data should be moved or reused without refetching.

## Source architecture observed

### osm-to-bedrock

- Rust backend entry point: `/Users/probello/Repos/osm-to-bedrock/src/server.rs`.
- Frontend: `/Users/probello/Repos/osm-to-bedrock/web`, built with Next.js and OpenLayers.
- Useful frontend pieces:
  - `web/src/app/page.tsx`
  - `web/src/components/MapView.tsx`
  - `web/src/components/DataSourcePanel.tsx`
  - `web/src/components/ExportPanel.tsx`
  - `web/src/components/LayerPanel.tsx`
  - `web/src/components/SearchBar.tsx`
  - `web/src/lib/overpass.ts`
  - `web/src/lib/api-config.ts`
- Shared-candidate backend modules:
  - `src/filter.rs`
  - `src/osm.rs`
  - `src/osm_cache.rs`
  - `src/overpass.rs`
  - `src/srtm.rs`
  - `src/elevation.rs`
- Existing backend endpoints to learn from:
  - `POST /fetch-preview`
  - `POST /fetch-convert`
  - `GET /cache/areas`
  - `GET /status/{id}`
  - `GET /download/{id}`

### osm-world

- Desktop renderer entry point: `/Users/probello/Repos/osm-world/src/main.rs`.
- Runtime options flow: CLI args -> `AppOptions` -> `init_wgpu()` -> world load.
- Current render data seam:
  - `--input <path-to-osm-pbf>`
  - `--srtm-dir <path-to-hgt-cache>`
- Current world loader:
  - `src/world/loader.rs::load_world_source()` parses local PBF input and loads elevation from a supplied directory.
- Current SRTM cache/downloader already mirrors osm-to-bedrock default folder naming in `src/geo/srtm.rs`, but the downloader is not wired into the runtime load path.
- Current streaming flags exist in CLI/app options, but the runtime tile streaming system is not yet fully wired to data fetching.

## Proposed architecture

### 1. `par-osm-rust` shared crate

Create a new Rust library at:

```text
/Users/probello/Repos/par-osm-rust
```

Initial crate name:

```toml
[package]
name = "par-osm-rust"
```

Rust import crate name will be `par_osm_rust`.

Initial modules:

```text
src/lib.rs
src/filter.rs
src/osm.rs
src/osm_cache.rs
src/overpass.rs
src/srtm.rs
src/elevation.rs
```

Responsibilities:

- `filter`: `FeatureFilter` used by Overpass queries and cache keys.
- `osm`: OSM data model, XML/PBF parsing, bbox clipping, bounds/stat helpers.
- `osm_cache`: raw Overpass XML disk cache, metadata, area listing, containment lookup, clearing.
- `overpass`: Overpass query building, URL validation, HTTP fetch, cache-aware `fetch_osm_data()`.
- `srtm`: SRTM cache directory, tile naming, tile coverage, download/retry.
- `elevation`: memory-mapped HGT loading and bilinear elevation queries.

The first migration can copy these modules from osm-to-bedrock with minimal changes. Later cleanup can improve names and split APIs further.

### 2. Cache contract

The shared crate owns a neutral cache root and migrates existing osm-to-bedrock cache data into it.

Default shared cache layout:

```text
$HOME/.cache/par-osm-rust/overpass
$HOME/.cache/par-osm-rust/srtm
```

Windows default:

```text
%LOCALAPPDATA%\par-osm-rust\overpass
%LOCALAPPDATA%\par-osm-rust\srtm
```

Temp fallback:

```text
<temp>/par-osm-rust-overpass
<temp>/par-osm-rust-srtm
```

Overpass cache directory priority:

```text
PAR_OSM_OVERPASS_CACHE_DIR if set
else OVERPASS_CACHE_DIR if set
else shared default overpass directory
```

SRTM cache directory priority:

```text
PAR_OSM_SRTM_CACHE_DIR if set
else SRTM_CACHE_DIR if set
else shared default srtm directory
```

Migration behavior:

- On first use, if the shared directory is empty and the legacy osm-to-bedrock directory exists, move cache files from the legacy directory into the shared directory.
- Legacy Overpass source: `$HOME/.cache/osm-to-bedrock/overpass` or `%LOCALAPPDATA%\osm-to-bedrock\overpass`.
- Legacy SRTM source: `$HOME/.cache/osm-to-bedrock/srtm` or `%LOCALAPPDATA%\osm-to-bedrock\srtm`.
- If a cross-filesystem rename fails, fall back to copy-then-delete.
- If destination files already exist, keep the destination copy and leave/delete legacy duplicates only when byte-identical.
- Expose an explicit migration API and CLI-safe helper so both projects can run/report migration deliberately:
  - `cache::migrate_legacy_caches() -> MigrationReport`
  - `cache::shared_cache_root() -> PathBuf`

The old env vars remain supported for compatibility, but the old folder names are no longer the default once `par-osm-rust` is adopted.

### 3. Local dependency integration

Both projects will use local path references during development.

In `/Users/probello/Repos/osm-world/Cargo.toml`:

```toml
par-osm-rust = { path = "../par-osm-rust" }
```

In `/Users/probello/Repos/osm-to-bedrock/Cargo.toml`:

```toml
par-osm-rust = { path = "../par-osm-rust" }
```

As each project migrates, remove duplicated dependencies only when they become unused in that project. Keep migrations small and verifiable.

### 4. osm-world web picker/launcher

Add an osm-world-specific web UI adapted from osm-to-bedrock, but simplified.

Keep:

- search/geocode box
- map view
- draw/select bbox
- feature type filters: roads, buildings, water, landuse, railways
- cache overlay/listing
- data fetch/progress
- launch osm-world
- copy command fallback

Drop:

- Minecraft world settings
- block preview
- spawn point selection
- sea level, vertical scale, surface thickness, world scale
- terrain-only conversion
- `.mcworld` download/status semantics
- Bedrock conversion history
- Overture priority controls unless later needed for renderer source selection

The osm-world backend should expose renderer-oriented endpoints rather than conversion endpoints:

- `GET /health`
- `GET /cache/areas`
- `POST /areas/prepare`
- `POST /launch`
- `GET /jobs/{id}` if background jobs are needed

`POST /areas/prepare` should:

1. Accept bbox, `FeatureFilter`, `use_elevation`, `force_refresh`, optional Overpass URL.
2. Fetch OSM through `par_osm_rust::overpass` using cache first.
3. Ensure SRTM tiles exist when elevation is enabled.
4. Persist or reference a renderer-readable local OSM file.
5. Return:
   - bbox
   - cache status
   - OSM file path
   - SRTM dir path
   - copyable command

A first command shape can be:

```bash
cargo run -- --input <prepared.osm> --srtm-dir ~/.cache/par-osm-rust/srtm
```

The exact command may use the built binary instead of `cargo run` when available.

`POST /launch` should attempt to start osm-world with the prepared paths. If launching is disabled or fails, the frontend should still display the command fallback.

### 5. Runtime streaming direction

The shared crate should make runtime streaming possible without baking renderer concepts into the data crate.

Runtime streaming in osm-world should be a later phase with these boundaries:

- osm-world decides camera-centered geographic/tile requests.
- par-osm-rust fetches/cache-misses OSM/SRTM data.
- osm-world converts returned OSM data into `WorldSource`/tile meshes.
- rendering code uploads completed tiles asynchronously.

Suggested renderer-side abstractions for the later phase:

- `WorldDataProvider`: cache-first async API for bbox/tile requests.
- `WorldTileRequest`: geographic bbox, feature filter, elevation flag.
- `WorldTileResult`: clipped OSM data, elevation path, cache metadata.
- tile state machine: `Missing -> Fetching -> Meshing -> Uploaded -> Evictable`.

The shared crate should not know about WGPU, scene buffers, minimap, camera, or Minecraft.

## Data formats

### OSM cache

Keep raw Overpass XML cache format from osm-to-bedrock:

```text
{sha256}.xml
{sha256}.meta.json
```

The cache key remains based on rounded bbox plus `FeatureFilter`, so both projects hit the same entries for the same request.

### Prepared renderer input

osm-world currently accepts local PBF files and its parser can support XML via `parse_osm_file()` in adjacent parsing code. To reduce first-pass complexity, the design should prefer one of these implementation paths:

1. Teach `world::loader::load_world_source()` to use generic `parse_osm_file()` so prepared `.osm` XML files from the cache can be rendered directly.
2. Or write prepared `.osm` XML files into a renderer prep directory and pass them to the same generic parser.

Generating PBF from Overpass XML is not required for the first implementation.

## Error handling

- Overpass URL overrides must remain HTTPS-only and host-allowlisted.
- Overpass `429` should return a user-readable busy/retry message.
- Cache read failures should log and fall back to network fetch where possible.
- Cache write failures should not prevent rendering if data was fetched successfully.
- SRTM download failures should be surfaced clearly when elevation is requested.
- Launch failures should leave the prepared data and copyable command visible.
- Runtime streaming failures should mark individual tiles failed without crashing the renderer.

## Testing strategy

### par-osm-rust

- Unit tests for `FeatureFilter` serde defaults.
- Unit tests for Overpass query construction and bbox validation.
- Unit tests for Overpass URL validation.
- Unit tests for OSM cache write/read/list/clear/containment lookup using temp dirs.
- Unit tests for SRTM tile naming and bbox tile coverage.
- Unit tests for HGT elevation loading/query behavior.
- Unit tests for OSM parse and bbox clipping.

### osm-to-bedrock migration

- Run its existing `make checkall` or equivalent.
- Verify the server still builds and cache endpoints still return existing cache metadata.
- Verify local path dependency migrates user-facing cache locations to `~/.cache/par-osm-rust/*` and preserves access to existing data.

### osm-world integration

- Run `make checkall`.
- Verify generic `.osm` XML input renders if the prepared file path uses XML.
- Verify `--srtm-dir ~/.cache/par-osm-rust/srtm` loads terrain elevation after migration.
- Verify area prepare returns a command that can launch/render the selected bbox.
- For UI work, verify with screenshots of the picker and launched renderer.

## Implementation phases

### Phase 1: Shared crate foundation

- Create `~/Repos/par-osm-rust`.
- Copy shared-candidate modules from osm-to-bedrock.
- Keep tests passing in the new crate.
- Document shared cache defaults and legacy cache migration.

### Phase 2: osm-to-bedrock local dependency migration

- Replace duplicated module usage with `par_osm_rust` imports where practical.
- Keep behavior unchanged except for the deliberate migration to neutral shared cache folders.
- Run existing verification.

### Phase 3: osm-world local dependency and generic input

- Add local dependency.
- Migrate or bridge OSM/SRTM parsing/cache use.
- Make world loading accept prepared XML or PBF input via a generic parser path.
- Keep existing CLI behavior working.

### Phase 4: osm-world web picker/launcher

- Adapt the frontend from osm-to-bedrock, removing Minecraft-specific controls.
- Add a small Rust backend for prepare/launch/cache status.
- Return copyable launch commands as a fallback.
- Verify selected area renders.

### Phase 5: runtime streaming

- Add cache-first background fetch around camera/world tiles.
- Generate tile meshes as data arrives.
- Upload/evict tiles under memory budgets.
- Keep failures tile-local and visible in debug UI.

## Open decisions for implementation planning

The following are intentionally deferred to the implementation plan because they depend on detailed code constraints:

- Whether the osm-world web UI lives under `web/` in this repo or uses a smaller static frontend served by Rust.
- Whether `POST /areas/prepare` writes a new clipped `.osm` file or returns an existing cache XML path with requested bbox metadata.
- Whether launch uses `cargo run`, `target/debug/osm-world`, or a configured binary path by default.
- How much of osm-to-bedrock should be migrated to the shared crate in the first pass versus wrapped for compatibility.

These decisions should not change the core architecture: shared crate first, local path dependencies, neutral shared cache folders with legacy migration, launcher now, runtime streaming later.
