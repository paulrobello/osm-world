# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Security: Added auth middleware for mutating API endpoints (`OSM_WORLD_API_TOKEN`)
- Security: Replaced permissive CORS with explicit localhost allowlist
- Security: Added renderer flag allowlist validation for `extra_args`
- Security: Updated Next.js to 16.2.3 (DoS fix), PostCSS to >=8.5.10 (XSS fix)
- Security: Removed filesystem paths from health endpoint, PID from launch response
- Security: Fixed TOCTOU race in SRTM tile download, added rate limiting middleware
- Architecture: Split `loader.rs` (3,452 lines) into focused sub-modules
- Architecture: Split `server.rs` (2,017 lines) into focused sub-modules
- Architecture: Split `road.rs` (2,030 lines) into bridge/tunnel sub-modules
- Architecture: Vendored `par-osm-rust` into workspace for independent builds
- Architecture: Refactored `App` struct into sub-structs (`AppUiState`, `AppRenderState`, `AppViewState`)
- Architecture: Added `FeatureLayer` enum for type-safe layer dispatch
- Architecture: Moved `Vertex` to shared `src/mesh.rs` to fix upward dependency
- Architecture: Added `Option<RenderIndexBuffer>` to skip empty GPU layers
- Architecture: Derived `Default` on `AppOptions` for cleaner test construction
- Architecture: Added GitHub Actions CI workflow
- Quality: Extracted 2,500+ lines of inline tests into dedicated test modules
- Quality: Deduplicated WGSL shader functions into shared `sky_helpers.wgsl`
- Quality: Extracted React components from page.tsx God Component
- Quality: Replaced `window.prompt`/`window.confirm` with custom modal dialogs
- Quality: Extracted `SourceConfig` struct to deduplicate server request/response types
- Quality: Replaced 8 identical iteration blocks with `index_features!` macro
- Quality: Reduced excessive `clone()` calls with ownership transfer patterns
- Quality: Added SAFETY comments to all `unsafe` blocks in tests
- Documentation: Added CHANGELOG.md, CONTRIBUTING.md, troubleshooting guide
- Documentation: Added ~120 docstrings across server, app, atmosphere, visual_detail modules
- Documentation: Added API reference with request/response schemas to architecture docs
- Documentation: Added JSDoc to all web frontend library modules
- Documentation: Created superpowers spec index with implementation status

## [0.1.0] - 2026-05-09

### Added

- WGPU desktop renderer with Winit windowing and flycam camera controls
- Real-world city rendering from OpenStreetMap `.osm.pbf`, `.pbf`, and `.osm` inputs
- SRTM elevation tile support for terrain height data
- Equirectangular coordinate projection for city-scale scenes
- Terrain, land use, water, waterways, roads, railways, buildings, transit paths, street signs, addresses, POIs, and landmarks
- Sky rendering, day-cycle lighting, and cascaded shadow maps with contact shadows
- Minimap rendering with optional camera-relative rotation
- egui-based HUD with settings panels, POI labels, search, and feature inspection
- Screenshot automation with configurable delay and auto-exit
- Streaming tile selection with GPU buffer budget validation for startup mesh generation
- Level-of-detail model with Near, Mid, and Far tiers
- Visual detail presets (Performance, Balanced, Showcase) with configurable facade variation, roof variation, vegetation density, and landmark detail
- Prepared-area cache with metadata, rename, favorite, and delete lifecycle
- `par-osm-rust` vendored dependency for shared Overpass/Overture cache and source preparation
- Local Axum API server with health, cache listing, prepared-area CRUD, area preparation, and renderer launch endpoints
- Authentication middleware for mutating API endpoints via `OSM_WORLD_API_TOKEN`
- Renderer flag allowlist validation for `extra_args` on the launch endpoint
- Input validation for bounding boxes, spawn coordinates, feature filters, Overpass URLs, and SRTM tile limits
- Overture Maps source controls with theme selection, failure modes, and timeout configuration
- Next.js Web Explorer with OpenLayers map picker, bounding-box presets, source controls, renderer profile management, and copyable command variants
- 282 Rust unit tests covering camera behavior, CLI parsing, streaming helpers, shader validation, world generation, and UI state
- Web tests for settings profiles, command variants, bbox presets, and error hints
- Make targets for build, test, lint, format, typecheck, and combined checkall
- Architecture documentation with Mermaid diagrams, module map, and data flow description
- Documentation style guide
