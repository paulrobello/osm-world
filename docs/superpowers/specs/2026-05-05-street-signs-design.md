# Street Signs with Street Names Design

## Goal

Add hybrid street-name signs to `osm-world`: small physical signposts in the 3D world with readable projected labels for the street names.

## Scope

First pass includes:

- Named drivable OSM roads only.
- Physical sign geometry for selected sign anchors.
- Screen-space street-name labels pinned above those sign anchors.
- Intersection-based signs and periodic signs along longer named roads.
- Distance/count caps to avoid visual clutter.

Deferred:

- True 3D/baked text on sign boards.
- Signs for footways, paths, cycleways, or unnamed roads.
- Complex intersection naming/layout rules.
- Collision, interaction, or navigation behavior.

## Architecture

### Street-sign world layer

Create `src/world/street_sign.rs` for street-sign eligibility, placement, and mesh generation. Add a resolved street-sign type to the world layer, for example:

```rust
pub struct ResolvedStreetSign {
    pub name: String,
    pub point: (f32, f32),
    pub elevation: f32,
    pub tangent: (f32, f32),
    pub rep_lat: f64,
    pub rep_lon: f64,
}
```

`WorldSource` gets `street_signs: Vec<ResolvedStreetSign>`. Loader code derives these from `WorldSource.roads` after roads have world coordinates and elevations.

### Road eligibility

A road is eligible when:

- `name` exists and is not empty after trimming.
- `highway=*` exists.
- the highway value is drivable enough for street-name signs.

Excluded highway values include `footway`, `path`, `cycleway`, `bridleway`, `steps`, `pedestrian`, `corridor`, `service`, `track`, and other non-street or non-drivable paths. Excluding `service` and `track` is intentional in the first pass to reduce clutter from alleys, driveways, parking aisles, and access roads.

### Placement

Generate two anchor types:

1. **Intersection anchors** near points shared by two or more named drivable roads. The first pass can detect exact shared OSM/world points rather than trying geometric segment intersection.
2. **Periodic anchors** along longer eligible roads at a fixed spacing, with a minimum road length threshold.

Placement uses caps:

- per-road max anchors,
- per-tile or global max anchors,
- label rendering max-visible cap.

Each anchor stores the local road tangent so the board can be oriented consistently with the road direction.

## Rendering

Street-sign mesh generation appends:

- a narrow metal post,
- a green rectangular board,
- a simple light trim using existing box/quad geometry.

The board is placed above terrain at the anchor elevation and rotated using the stored tangent. The initial board has no baked text.

Full-world mesh generation and tile mesh generation append street-sign geometry after roads and before/near point features so signs sit above road surfaces and remain visually distinct.

## Labels

Generalize or mirror the current projected POI label path to support street-sign labels separately from POI labels.

Street-name labels:

- use the sign anchor name,
- project from a point above the sign board,
- render only within a configurable max distance,
- sort by distance and cap visible labels,
- have settings independent from POI labels.

This keeps the first pass readable without adding a 3D text/font texture pipeline.

## Data Flow

1. OSM parser preserves road way tags, including `name` and `highway`.
2. Loader resolves road ways to world coordinates and elevations.
3. Street-sign placement scans resolved roads and emits `ResolvedStreetSign` anchors.
4. Feature indexing assigns street signs to owner tiles by anchor position.
5. Mesh generation appends physical sign geometry.
6. UI label generation converts street signs into projected street-name labels.

## Testing

Add tests for:

- named drivable roads are eligible for signs,
- unnamed roads are skipped,
- footways/paths/cycleways are skipped,
- long named roads produce periodic sign anchors with per-road caps,
- shared named road points produce intersection anchors,
- full-world and tile mesh generation emit street-sign vertices,
- street-name labels include road names and remain independent from POI label settings.

Final verification should run the repository verifier, preferably `make checkall`, and run `graphify update .` after code changes.
