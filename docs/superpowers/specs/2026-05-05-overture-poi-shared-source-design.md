# Shared Overture POI Source Design

## Goal

Move Overture Maps ingestion out of `osm-to-bedrock` and into the shared `par-osm-rust` crate, then update both `osm-to-bedrock` and `osm-world` to use shared Overture-aware data-source logic. Overture should become the preferred POI source by default, while still allowing explicit OSM-only, Overture-only, and merged modes.

## Current State

`osm-to-bedrock` already has Overture support in its own crate:

- `src/overture.rs` shells out to the optional `overturemaps` Python CLI, caches GeoJSON, converts Overture themes into `OsmData`, and maps Overture place categories to OSM-style tags.
- CLI and server paths fetch Overpass data, optionally fetch Overture data, then merge the two `OsmData` values.

`par-osm-rust` currently owns shared OSM/Overpass/cache/SRTM concerns, and its `OsmData` model already has POI/address/tree collections. It does not currently expose Overture modules or a shared source-composition policy.

`osm-world` uses `par-osm-rust` for server-side Overpass/cache/SRTM preparation, but its renderer consumes normalized OSM-style data through `load_world_source()`. It has no Overture fetch/merge path today.

## Scope

Included:

- Move the reusable Overture fetch/cache/parse/tag-mapping logic into `par-osm-rust`.
- Add shared source orchestration for OSM/Overpass plus Overture.
- Add a POI source policy with `OsmOnly`, `OvertureOnly`, `Both`, and `OverturePreferred` modes.
- Default to Overture-preferred POIs when Overture is enabled/configured.
- Make Overture failure behavior configurable, defaulting to graceful fallback to OSM POIs.
- Update `osm-to-bedrock` to call shared Overture/source helpers instead of owning the implementation.
- Update `osm-world` preparation to support Overture-aware POI preparation while keeping renderer-side OSM-style tags.

Deferred:

- Replacing the `overturemaps` CLI with a direct HTTP/Parquet reader.
- Full provider-trait plugin architecture for arbitrary future data sources.
- True renderer-side source visualization or per-source UI styling.
- Broad non-POI source replacement policy beyond the existing Overture theme options.

## Shared Crate Architecture

### `par_osm_rust::overture`

Add an Overture module to `par-osm-rust`, based on the proven `osm-to-bedrock` implementation. It will own:

- `OvertureTheme`
- `ThemePriority` if still needed for non-POI theme merge behavior
- `OvertureParams`
- `is_cli_available()`
- `fetch_geojson_for_type()`
- Overture GeoJSON cache helpers and listing/clear APIs
- `parse_overture_geojson()`
- `fetch_overture_data()` and best-effort fetch variants

The module should keep Overture data normalized into the existing `OsmData` model. Overture place points become POI nodes with OSM-style `amenity`, `shop`, `tourism`, or `leisure` tags. Overture land/tree points remain tree nodes.

### `par_osm_rust::sources`

Add a small source-orchestration module rather than a full provider framework. It should expose one high-level function for bbox-based fetching, for example:

```rust
pub fn fetch_map_data(
    bbox: (f64, f64, f64, f64),
    options: &SourceOptions,
    progress_cb: &mut dyn FnMut(f32, &str),
) -> Result<SourceFetchResult>;
```

`SourceOptions` should include:

- `filter: FeatureFilter`
- `overpass_url: Option<String>`
- `use_overpass_cache: bool`
- `overture: OvertureParams`
- `poi_source_mode: PoiSourceMode`
- `overture_failure_mode: OvertureFailureMode`

`SourceFetchResult` should include:

- merged `OsmData`
- source/fallback status suitable for logs or API responses
- cache-status details when available
- warnings for graceful fallback cases

## POI Source Policy

Add a shared enum:

```rust
pub enum PoiSourceMode {
    OsmOnly,
    OvertureOnly,
    Both,
    OverturePreferred,
}
```

Behavior:

- `OsmOnly`: use OSM/Overpass POIs only.
- `OvertureOnly`: use Overture POIs only; in strict failure mode, fail if unavailable.
- `Both`: merge OSM and Overture POIs and dedupe near duplicates.
- `OverturePreferred`: prefer Overture POIs. If Overture POIs are available, they win duplicate conflicts. If Overture is unavailable, fails, disabled, or returns no POIs, fall back to OSM POIs by default.

Add failure mode:

```rust
pub enum OvertureFailureMode {
    FallbackToOsm,
    Fail,
}
```

Default behavior is `FallbackToOsm`.

## Source Metadata and Deduplication

`OsmData` needs enough metadata to distinguish OSM-derived and Overture-derived POIs after conversion. Add a lightweight source marker, likely:

```rust
pub enum FeatureSource {
    Osm,
    Overture,
    Synthetic,
}
```

At minimum, `OsmPoiNode` should carry `source: FeatureSource` with serde-friendly defaults if serialization is used. Existing OSM parsers should mark parsed POIs as `Osm`; Overture conversion should mark place POIs as `Overture`; generated trees or other derived points can use `Synthetic` where needed.

Deduplication should be shared and deterministic:

- For named POIs, match normalized name plus broad category within about 25 metres.
- For unnamed POIs, match broad category within about 10 metres.
- In `OverturePreferred`, Overture entries win duplicate conflicts.
- In `Both`, keep a single representative per duplicate group, preferring Overture if present to avoid flicker across apps.

POI-tagged ways are trickier because OSM POIs can appear as tagged areas/buildings. First implementation should prioritize `OsmPoiNode` / Overture place point dedupe and preserve current way behavior. If OSM POI ways become duplicate-heavy in practice, add centroid-based source-aware filtering in a follow-up.

## `osm-to-bedrock` Updates

`osm-to-bedrock` should become a thin consumer of shared Overture/source logic:

- Remove the local Overture implementation or replace it with re-exports from `par_osm_rust::overture` during migration.
- Move local Overture parameter definitions to shared types, or re-export shared types through `osm_to_bedrock::params` for CLI compatibility.
- Replace manual `fetch_osm_data()` + optional `fetch_overture_data()` + `merge()` call sites with the shared source orchestrator.
- Preserve existing CLI/server flags where possible:
  - `--overture`
  - `--overture-themes`
  - `--overture-priority`
  - `--overture-timeout`
- Add or map a POI source mode control so users can select OSM-only, Overture-only, both, or Overture-preferred.
- Keep `overture-convert` as an Overture-only convenience command backed by shared code.

## `osm-world` Updates

`osm-world` should use the same shared source policy during area preparation:

- Extend `PrepareAreaRequest` with optional Overture settings:
  - `overture: bool`
  - `overture_themes: Vec<String>` or shared enum strings
  - `poi_source_mode: Option<PoiSourceMode>`
  - `overture_failure_mode: Option<OvertureFailureMode>`
  - `overture_timeout: Option<u64>`
- The server should call the shared source orchestrator instead of fetching only Overpass XML.
- Prepared renderer input should remain normalized OSM-style data so `load_world_source()` and the current renderer can keep working.
- The API response should expose warnings/status when Overture falls back to OSM.
- Renderer-side point feature classification remains unchanged: it consumes OSM-style tags generated from either source.

A practical first implementation may need a shared writer for prepared `.osm` XML or a prepared normalized file format. If writing merged `OsmData` back to XML is too broad, use a minimal prepared sidecar/JSON format only for the server-to-renderer path, but keep that choice explicit in the implementation plan.

## Error Handling

- Missing `overturemaps` CLI:
  - `FallbackToOsm`: warn and continue with OSM POIs.
  - `Fail`: return an error.
- Overture fetch timeout or parse failure:
  - `FallbackToOsm`: warn and continue with OSM POIs.
  - `Fail`: return an error.
- `OvertureOnly` plus fallback mode:
  - If Overture fails, return empty POIs only if explicitly documented in options; otherwise prefer an actionable warning and no OSM substitution because the user explicitly selected Overture-only.
- Overpass failure:
  - If the selected mode needs OSM data, fail as today.
  - If `OvertureOnly` does not need OSM data, Overpass should not be fetched.

## Testing

Shared crate tests:

- Overture GeoJSON parser tests moved from `osm-to-bedrock` to `par-osm-rust`.
- Overture cache key/read/write/list/clear tests.
- `PoiSourceMode` tests for `OsmOnly`, `OvertureOnly`, `Both`, and `OverturePreferred`.
- Dedup tests showing Overture wins named duplicate conflicts.
- Fallback-mode tests for missing/unavailable Overture.
- Overpass query tests remain unchanged except where POI source mode avoids unnecessary OSM POI queries.

`osm-to-bedrock` tests:

- CLI/server option parsing maps to shared source options.
- `fetch-convert` and async server conversion use shared orchestrator.
- Existing conversion output tests still pass.

`osm-world` tests:

- Prepare endpoint accepts source options.
- Prepared response reports fallback warnings/status.
- Cached/mock Overture place data appears as renderer point features.
- OSM-only mode preserves existing behavior.

Project verification:

- `par-osm-rust`: `cargo test`
- `osm-to-bedrock`: relevant unit tests plus full check command if available
- `osm-world`: `make checkall`
- After `osm-world` code changes, run `graphify update .`

## Migration Plan Notes

This should be implemented in phases:

1. Move Overture types and module into `par-osm-rust` with tests passing there.
2. Add shared POI source policy/dedupe and source metadata.
3. Update `osm-to-bedrock` to use shared APIs while preserving its public CLI/server behavior.
4. Update `osm-world` preparation to call shared source APIs and expose Overture settings.
5. Verify both projects and update documentation/vault notes if new reusable patterns emerge.
