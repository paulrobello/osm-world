// Feature-type discriminant constants. Source of truth is
// `src/mesh.rs::feature`; this file MUST mirror it. `build.rs` parses both and
// fails the build on mismatch; `tests/shader_source_test.rs` re-checks at test
// time. See `src/mesh.rs` and `docs/ARCHITECTURE.md` for the slot-reservation
// convention.
//
// Shader ranges compare `feature_type` against these constants with a slop
// band (typically ±0.25, sometimes ±0.5 — see each call site). When adding or
// renumbering, keep at least 0.1 between adjacent slots so the bands do not
// collide.

const FEATURE_TERRAIN: f32 = 0.0;
const FEATURE_BUILDING: f32 = 1.0;
const FEATURE_ROAD: f32 = 2.0;
const FEATURE_ROAD_LAYERED: f32 = 2.10;
const FEATURE_ROAD_PATH: f32 = 2.25;
const FEATURE_WATER: f32 = 3.0;
const FEATURE_LANDUSE: f32 = 4.0;
const FEATURE_LANDUSE_OVERLAY: f32 = 4.25;
const FEATURE_ROAD_MARKING: f32 = 5.0;
const FEATURE_ROAD_MARKING_LAYERED: f32 = 5.10;
const FEATURE_RAILWAY: f32 = 6.0;
const FEATURE_POINT_FEATURE: f32 = 7.0;
const FEATURE_STREET_SIGN: f32 = 8.0;
