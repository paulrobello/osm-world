# Superpowers Specs and Plans Index

Design specifications and implementation plans for osm-world features. Specs describe the intended design; plans describe the implementation steps. Both reflect the state of the project at the time they were written. For current behavior, see the source code and `docs/ARCHITECTURE.md`.

## Specs

| Spec | Date | Status | Description |
| --- | --- | --- | --- |
| [3D Engine Design](specs/2026-05-01-osm-world-3d-engine-design.md) | 2026-05-01 | Implemented | Original renderer module layout, coordinate system, and mesh generation |
| [Streaming and LOD](specs/2026-05-02-phase3-streaming-lod-design.md) | 2026-05-02 | Partial | Tile streaming, LOD tiers, and runtime loading direction. Startup tile selection is implemented; runtime incremental upload is not yet complete |
| [Shading, Shadows, Occlusion, Minimap](specs/2026-05-02-shading-shadows-occlusion-minimap-design.md) | 2026-05-02 | Implemented | Cascaded shadow maps, contact shadows, occlusion queries, and minimap render target |
| [Cascaded Shadow LOD](specs/2026-05-03-cascaded-shadow-lod-design.md) | 2026-05-03 | Implemented | Shadow cascade blending and LOD-aware shadow selection |
| [Shared OSM Cache and Streaming](specs/2026-05-03-shared-osm-cache-and-streaming-design.md) | 2026-05-03 | Implemented | Shared `par-osm-rust` cache contract and prepare workflow |
| [Overpasses and Tunnels](specs/2026-05-04-overpasses-tunnels-design.md) | 2026-05-04 | Implemented | Bridge and tunnel geometry, elevated roads, and tunnel portals |
| [Point Features](specs/2026-05-04-point-features-design.md) | 2026-05-04 | Implemented | POI point features, addresses, and street signs |
| [Shared Overture POI Source](specs/2026-05-05-overture-poi-shared-source-design.md) | 2026-05-05 | Implemented | Overture Maps integration, theme selection, and failure modes |
| [Street Signs](specs/2026-05-05-street-signs-design.md) | 2026-05-05 | Implemented | Street sign rendering with name labels |
| [Visual Detail Controls](specs/2026-05-06-osm-world-visual-detail-controls-design.md) | 2026-05-06 | Implemented | Visual presets, landmark detail, facade/roof variation, vegetation density |
| [Sun Depth](specs/2026-05-06-sun-depth-design.md) | 2026-05-06 | Implemented | Layered sun depth shader for improved shadow quality |

## Plans

| Plan | Date | Description |
| --- | --- | --- |
| [Phase 1: Window, Flycam, Test Building](plans/2026-05-01-phase1-window-flycam-testbuilding.md) | 2026-05-01 | Initial WGPU window, camera, and test scene |
| [Shading, Shadows, Occlusion, Minimap](plans/2026-05-02-shading-shadows-occlusion-minimap.md) | 2026-05-02 | Shadow maps, contact shadows, minimap implementation |
| [Streaming and LOD](plans/2026-05-02-phase3-streaming-lod.md) | 2026-05-02 | Tile streaming and LOD implementation steps |
| [Cascaded Shadow LOD](plans/2026-05-03-cascaded-shadow-lod.md) | 2026-05-03 | Cascade blending implementation |
| [par-osm-rust Shared Cache Foundation](plans/2026-05-03-par-osm-rust-shared-cache-foundation.md) | 2026-05-03 | Shared cache crate setup |
| [par-osm-rust Consumer Migration](plans/2026-05-03-par-osm-rust-consumer-migration.md) | 2026-05-03 | Migrating osm-world to use the shared cache |
| [Web Picker](plans/2026-05-03-osm-world-web-picker.md) | 2026-05-03 | Next.js Web Explorer implementation |
| [Area Prepare Backend](plans/2026-05-03-osm-world-area-prepare-backend.md) | 2026-05-03 | Axum API server for area preparation |
| [Overpasses and Tunnels](plans/2026-05-04-overpasses-tunnels.md) | 2026-05-04 | Bridge and tunnel implementation |
| [POI Point Features](plans/2026-05-04-point-features.md) | 2026-05-04 | Point feature implementation |
| [Point Features (alt)](plans/2026-05-04-poi-point-features.md) | 2026-05-04 | Alternate point feature plan |
| [Overture POI Shared Source](plans/2026-05-05-overture-poi-shared-source.md) | 2026-05-05 | Overture integration implementation |
| [Street Signs](plans/2026-05-05-street-signs.md) | 2026-05-05 | Street sign implementation |
| [Sun Depth](plans/2026-05-06-sun-depth.md) | 2026-05-06 | Sun depth shader implementation |
| [Visual Detail Controls](plans/2026-05-06-visual-detail-controls.md) | 2026-05-06 | Visual detail settings implementation |

## Related Documentation

- [Architecture](../ARCHITECTURE.md) -- Current module map, data flow, and API surface
- [README](../../README.md) -- User-facing overview and quick start
