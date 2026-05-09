//! World loading orchestrator.
//!
//! Splits into focused sub-modules:
//! - `source` -- data types and OSM loading
//! - `mesh` -- mesh generation from world source
//! - `geometry` -- polygon/geometry utilities
//! - `vegetation` -- tree scattering for landuse areas

pub mod geometry;
pub mod mesh;
pub mod source;
pub mod vegetation;

// Re-export public API from sub-modules for backwards compatibility.
pub use source::{
    CpuMesh, ResolvedFeature, ResolvedPointFeature, TileMeshSet, WorldMesh, WorldSource,
    load_world_source, load_world_source_with_visual_detail,
};

pub use mesh::{
    generate_streamed_startup_mesh, generate_tile_mesh_set,
    generate_tile_mesh_set_with_visual_detail, generate_world_mesh,
    generate_world_mesh_with_visual_detail, load_world, same_road_point,
};

pub use geometry::{
    bbox_intersects_rect, clip_polygon_to_rect, containing_building_name, ensure_ccw, feature_bbox,
    feature_indices_intersecting_tiles, feature_owner_tile, move_point_outside_containing_building,
    move_point_outside_polygon, point_in_polygon, tiles_for_half_open_bbox,
};

pub use vegetation::append_tree_area_point_features_with_settings;

#[cfg(test)]
mod tests;
