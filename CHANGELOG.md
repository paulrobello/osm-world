# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
