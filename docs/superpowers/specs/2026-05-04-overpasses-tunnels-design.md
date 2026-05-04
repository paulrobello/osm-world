# Overpasses and Tunnels Design

## Goal
Add visible support for OSM overpasses/bridges and tunnels in `osm-world` without coupling the first implementation to terrain excavation or relation-level bridge/tunnel modeling.

## Current Context
`osm-world` already parses OSM way tags into `ResolvedFeature.tags`. Roads are generated from per-point terrain elevations in `src/world/loader.rs`, then `src/world/road.rs::generate_road_with_elevations()` turns those elevations into a ribbon mesh. Recent work added `road_layer_y_offset()` so `bridge=yes`, `bridge=viaduct`, and positive `layer=*` roads render above surface roads to avoid z-fighting.

## Selected Approach: Hybrid Visible Structures
Implement visible structures now while deferring full terrain carving:

- Bridges/overpasses: raise the road using the existing elevation-offset path, add visible deck underside, side rails, and support pillars.
- Tunnels: lower the road for `tunnel=*` and negative `layer=*`, add portal frames at tunnel endpoints and a dark inner lining/ceiling cue.
- Terrain excavation/cutting is explicitly out of scope for this phase.
- Bridge/tunnel OSM relations are out of scope; support comes from way tags only.

## Architecture

### Road profile classification
Add a small road profile API in `src/world/road.rs` that classifies a road as surface, bridge, or tunnel from tags:

- `bridge=yes` and `bridge=viaduct` classify as bridge.
- Any non-`no` `tunnel=*` classifies as tunnel.
- Positive numeric `layer=*` contributes bridge-like elevation.
- Negative numeric `layer=*` contributes tunnel-like lowering.
- If both bridge and tunnel-like tags are present, explicit `tunnel=*` wins for lowering because the visual renderer cannot safely display both on one way.

### Bridge structures
Bridge roads use the existing road ribbon plus an added structure pass:

- A deck underside follows each road segment below the road surface.
- Low guard rails run along both sides of each segment.
- Support pillars are emitted at selected points along longer bridge spans down toward sampled terrain.
- The implementation should be conservative: simple box geometry is enough for deck, rails, and pillars.

### Tunnel structures
Tunnel roads use the existing road ribbon plus an added structure pass:

- Tunnel road elevation is lower than the surface road baseline.
- Portal frames are emitted at open endpoints.
- A simple dark lining/ceiling cue is emitted along the tunnel path so it reads as enclosed even without terrain carving.
- Full terrain mesh modification is deferred.

### Mesh integration
Both full-world and streaming tile road generation must use the same helper path so they stay visually consistent:

- `append_world_mesh()` should render roads and their structures.
- `append_tile_roads_mesh()` should render the same roads and structures for tile LODs.
- Far LOD may skip minor-road structures via the existing minor-highway filter; major bridge/tunnel structures should remain if the road itself remains.

## Data Flow
1. OSM parser preserves way tags.
2. `load_world_source()` stores tags, world points, and base terrain elevations in `ResolvedFeature`.
3. Road rendering classifies each feature into a profile from tags.
4. The profile transforms terrain elevations into render elevations.
5. The road ribbon is generated at those elevations.
6. Optional bridge/tunnel structure geometry is appended beside the road ribbon.

## Error Handling and Edge Cases
- Invalid `layer=*` values are treated as `0`.
- `bridge=no` is not a bridge.
- `tunnel=no` is not a tunnel.
- Degenerate road segments should produce no structure geometry, matching current road-ribbon behavior.
- Closed road loops should not emit portal frames.
- Structure helpers should avoid panics when point/elevation lengths differ.

## Testing
Use TDD for the implementation:

- Unit-test road profile classification and offsets in `src/world/road.rs`.
- Unit-test that tunnel roads lower below surface roads.
- Unit-test that bridge structures add non-road structure geometry above/below expected Y ranges.
- Unit-test that tunnel portals are emitted for open tunnel endpoints but not closed loops.
- Test full-world and tile-road paths where practical so the two render paths do not drift.

Canonical verification is `make checkall`. After editing Rust files, also run `graphify update .` to refresh the project graph.

## Deferred Work
- Terrain carving around tunnel corridors.
- Portal clipping into terrain faces.
- Bridge/tunnel relation parsing.
- More realistic bridge types, arches, ramps, abutments, and retaining walls.
- Collision or navigation changes.
