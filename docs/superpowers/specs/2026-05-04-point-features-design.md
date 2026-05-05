# OSM Point Features Design

## Goal

Add visible, lightweight support for OSM node-based landmarks, nature features, and trees in `osm-world` so sparse point tags become recognizable 3D scene elements.

## Scope

This first pass renders tagged OSM **nodes** as a dedicated point-feature overlay layer. It does not attempt billboard icons, labels, dense forest generation from polygons, or complex landmark models.

Included tags:

- Trees: `natural=tree`
- Nature markers: `natural=peak`, `natural=rock`, `natural=spring`
- Landmarks: `tourism=attraction`, `tourism=viewpoint`, `tourism=artwork`, any `historic=*`, and `man_made=tower`, `man_made=water_tower`, `man_made=chimney`

## Architecture

### Parser

`src/osm/parse.rs` will preserve node tags in `OsmNode`. PBF parsing should collect tags from `Element::Node`; XML parsing should support both self-closing nodes and nodes with child `<tag>` elements.

### World source

`src/world/loader.rs` will add a `point_features: Vec<ResolvedPointFeature>` collection to `WorldSource`. A point feature stores tags, world `(x, z)`, terrain elevation, and representative lat/lon. Feature indexing will add point-feature references to `TileFeatureRefs` using the owning tile for the point coordinate.

### Rendering

Create `src/world/point_feature.rs` with tag classification and mesh generation:

- Tree: low-poly brown trunk plus green canopy.
- Landmark: vertical stone/gold marker or simple tower.
- Nature marker: small colored cone/marker.

Point features render after roads/railways and before buildings in both full-world and tile mesh paths. Geometry uses existing `Vertex` with a new `feature::POINT_FEATURE` material. Meshes stay intentionally small and deterministic.

## Data Flow

1. Parser reads node lat/lon and tags.
2. Loader converts tagged nodes to world coordinates and elevations.
3. Loader indexes point features by owner tile.
4. Mesh generation appends point-feature geometry to full-world and tile meshes.

## Testing

- Parser tests for tagged XML nodes and PBF-compatible node tag storage where practical.
- Loader test that tagged OSM nodes become `WorldSource.point_features`.
- Tile mesh test that point features emit `feature::POINT_FEATURE` vertices.
- Mesh tests for tree/landmark/nature classification and generated geometry.
- Existing `make checkall` and `graphify update .` remain required.

## Deferred Work

- Labels and icon billboards.
- Polygon-derived forests or distributed tree placement.
- Detailed landmark-specific models.
- Collision/navigation integration.
- Relation-based landmark geometry.
