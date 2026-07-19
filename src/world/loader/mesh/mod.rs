//! Mesh generation from world source data.
//!
//! Sub-modules (split, ARC-012, from a single 960-line file):
//! - `terrain_cuts` -- tunnel-portal terrain depressions
//! - `water`        -- clipped water-area + uncovered-waterway emission
//! - `roads`        -- per-road ramps/structures + dead-end cap placement,
//!   including the shared tile/world cap collector
//!
//! `mod.rs` keeps the entry points and the two top-level orchestrators
//! (`append_world_mesh`, `append_tile_features_mesh`) that sequence every
//! per-feature mesh builder. The previous road-cap duplication between those
//! two orchestrators now routes through [`roads::emit_world_road_caps`] and
//! [`roads::append_tile_roads_mesh`].

mod roads;
mod terrain_cuts;
mod water;

use crate::render::vertex::Vertex;
use crate::world::loader::source::{CpuMesh, TileMeshSet, WorldMesh, WorldSource};

// Test-facing re-export: loader tests reach into the tile-road builder through
// `mesh::append_tile_roads_mesh`. Kept `pub(crate)` to match the original
// visibility of the function before the split.
#[cfg(test)]
pub(crate) use roads::append_tile_roads_mesh;

pub fn same_road_point(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() <= 0.05 && (a.1 - b.1).abs() <= 0.05
}

pub fn generate_world_mesh(source: &WorldSource) -> WorldMesh {
    generate_world_mesh_with_visual_detail(
        source,
        &crate::visual_detail::VisualDetailSettings::default(),
    )
}

pub fn generate_world_mesh_with_visual_detail(
    source: &WorldSource,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
) -> WorldMesh {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();
    append_world_mesh(source, visual_detail, &mut verts, &mut idxs);

    let (cx, cz) = source.conv.bbox_centre(
        source.min_lat,
        source.min_lon,
        source.max_lat,
        source.max_lon,
    );
    let cy = source.elevation_at(
        (source.min_lat + source.max_lat) / 2.0,
        (source.min_lon + source.max_lon) / 2.0,
    ) + 50.0;

    WorldMesh {
        vertices: verts,
        indices: idxs,
        center: (cx, cy, cz),
    }
}

fn append_world_mesh(
    source: &WorldSource,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    // Generate meshes in order: terrain, landuse, water, roads, railways, point features, street signs, buildings

    // Terrain
    let terrain_cuts = terrain_cuts::terrain_cuts_for_roads(&source.roads);
    let terrain_ctx = crate::world::terrain::TerrainContext {
        conv: &source.conv,
        elevation: source.elevation.as_ref(),
        cuts: &terrain_cuts,
    };
    let mut terrain_output = crate::world::terrain::MeshOutput {
        vertices: verts,
        indices: idxs,
    };
    crate::world::terrain::generate_terrain_with_cuts(
        source.min_lat,
        source.min_lon,
        source.max_lat,
        source.max_lon,
        &terrain_ctx,
        &mut terrain_output,
    );

    // Landuse
    for lu in &source.landuses {
        let color = crate::world::color::landuse_color(&lu.tags);
        let y_offset = crate::world::landuse::landuse_y_offset(&lu.tags);
        crate::world::landuse::generate_landuse_with_elevations_and_offset(
            &lu.points,
            &lu.elevations,
            y_offset,
            color,
            verts,
            idxs,
        );
    }

    // Water
    for w in &source.waters {
        crate::world::water::generate_water_with_elevations(&w.points, &w.elevations, verts, idxs);
    }
    let all_waterway_refs: Vec<_> = (0..source.waterways.len()).collect();
    water::append_uncovered_waterway_meshes(source, &all_waterway_refs, verts, idxs);

    // Roads (ribbons + structures + dead-end caps)
    roads::emit_world_road_caps(source, verts, idxs);

    for route in &source.transit_routes {
        crate::world::road::generate_road_with_elevations_and_feature_type(
            &route.points,
            &route.elevations,
            2.8,
            crate::world::transit::transit_route_color(&route.tags),
            crate::render::vertex::feature::ROAD_PATH,
            verts,
            idxs,
        );
    }

    for railway in &source.railways {
        crate::world::railway::generate_railway_track(
            &railway.tags,
            &railway.points,
            &railway.elevations,
            verts,
            idxs,
        );
    }

    for point_feature in &source.point_features {
        crate::world::point_feature::generate_point_feature_with_visual_detail(
            &point_feature.tags,
            point_feature.point,
            point_feature.elevation,
            visual_detail,
            verts,
            idxs,
        );
    }

    for sign in &source.street_signs {
        crate::world::street_sign::append_street_sign(sign, verts, idxs);
    }

    // Buildings
    for (feature_idx, b) in source.buildings.iter().enumerate() {
        let style = crate::world::color::building_style(
            &b.tags,
            feature_idx as u64,
            visual_detail.facade_variation,
            visual_detail.roof_variation,
        );
        let base_y = source.elevation_at(b.rep_lat, b.rep_lon);
        let height = crate::world::building::parse_building_height(&b.tags);
        // Remove trailing duplicate point if present for the footprint
        let mut footprint = b.points.clone();
        if footprint.len() > 3 && footprint.first() == footprint.last() {
            footprint.pop();
        }
        crate::world::building::generate_building_with_style(
            &footprint, base_y, height, style, verts, idxs,
        );
    }

    log::info!(
        "Generated world mesh: {} vertices, {} indices, {} buildings, {} roads, {} railways, {} point features, {} street signs, {} water areas, {} waterways, {} landuse areas",
        verts.len(),
        idxs.len(),
        source.buildings.len(),
        source.roads.len(),
        source.railways.len(),
        source.point_features.len(),
        source.street_signs.len(),
        source.waters.len(),
        source.waterways.len(),
        source.landuses.len(),
    );
}

pub fn generate_tile_mesh_set(
    source: &WorldSource,
    coord: crate::stream::TileCoord,
    refs: &crate::stream::tile::TileFeatureRefs,
    tile_size: f32,
) -> TileMeshSet {
    generate_tile_mesh_set_with_visual_detail(
        source,
        coord,
        refs,
        tile_size,
        &crate::visual_detail::VisualDetailSettings::default(),
    )
}

pub fn generate_tile_mesh_set_with_visual_detail(
    source: &WorldSource,
    coord: crate::stream::TileCoord,
    refs: &crate::stream::tile::TileFeatureRefs,
    tile_size: f32,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
) -> TileMeshSet {
    let lods = [
        generate_tile_lod_mesh(
            source,
            coord,
            refs,
            tile_size,
            crate::stream::TileLod::Near,
            visual_detail,
        ),
        generate_tile_lod_mesh(
            source,
            coord,
            refs,
            tile_size,
            crate::stream::TileLod::Mid,
            visual_detail,
        ),
        generate_tile_lod_mesh(
            source,
            coord,
            refs,
            tile_size,
            crate::stream::TileLod::Far,
            visual_detail,
        ),
    ];
    let aabb = aabb_for_lods(coord, tile_size, &lods);
    TileMeshSet { coord, aabb, lods }
}

pub fn generate_streamed_startup_mesh(
    source: &WorldSource,
    selected_tiles: &[crate::stream::TileCoord],
    tile_size: f32,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
) -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut coords = selected_tiles.to_vec();
    coords.sort_unstable();
    coords.dedup();

    let rects: Vec<_> = coords.iter().map(|coord| coord.rect(tile_size)).collect();
    let road_refs = super::geometry::feature_indices_intersecting_tiles(&source.roads, &rects);
    let terrain_cuts = terrain_cuts::terrain_cuts_for_road_refs(source, &road_refs);

    let terrain_ctx = crate::world::terrain::TerrainContext {
        conv: &source.conv,
        elevation: source.elevation.as_ref(),
        cuts: &terrain_cuts,
    };
    for coord in &coords {
        let rect = coord.rect(tile_size);
        let mut terrain_output = crate::world::terrain::MeshOutput {
            vertices: &mut vertices,
            indices: &mut indices,
        };
        crate::world::terrain::generate_terrain_for_world_rect_with_cuts(
            rect.min_x,
            rect.min_z,
            rect.max_x,
            rect.max_z,
            crate::stream::LodConfig::terrain_spacing(crate::stream::TileLod::Near),
            &terrain_ctx,
            &mut terrain_output,
        );
    }

    let water_refs = super::geometry::feature_indices_intersecting_tiles(&source.waters, &rects);
    water::append_clipped_water_meshes(source, &water_refs, &rects, &mut vertices, &mut indices);

    let refs = crate::stream::tile::TileFeatureRefs {
        buildings: super::geometry::feature_indices_intersecting_tiles(&source.buildings, &rects),
        roads: road_refs,
        railways: super::geometry::feature_indices_intersecting_tiles(&source.railways, &rects),
        waters: Vec::new(),
        waterways: super::geometry::feature_indices_intersecting_tiles(&source.waterways, &rects),
        landuses: super::geometry::feature_indices_intersecting_tiles(&source.landuses, &rects),
        point_features: source
            .point_features
            .iter()
            .enumerate()
            .filter_map(|(idx, point)| {
                coords
                    .binary_search(&crate::stream::TileCoord::from_world(
                        point.point.0,
                        point.point.1,
                        tile_size,
                    ))
                    .is_ok()
                    .then_some(idx)
            })
            .collect(),
        street_signs: source
            .street_signs
            .iter()
            .enumerate()
            .filter_map(|(idx, sign)| {
                coords
                    .binary_search(&crate::stream::TileCoord::from_world(
                        sign.point.0,
                        sign.point.1,
                        tile_size,
                    ))
                    .is_ok()
                    .then_some(idx)
            })
            .collect(),
    };

    append_tile_features_mesh(
        source,
        &refs,
        crate::stream::TileLod::Near,
        visual_detail,
        &mut vertices,
        &mut indices,
    );

    CpuMesh { vertices, indices }
}

fn aabb_for_lods(
    coord: crate::stream::TileCoord,
    tile_size: f32,
    lods: &[CpuMesh; 3],
) -> crate::stream::TileAabb {
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    let mut has_vertices = false;

    for vertex in lods.iter().flat_map(|mesh| &mesh.vertices) {
        let position = glam::Vec3::from_array(vertex.position);
        min = min.min(position);
        max = max.max(position);
        has_vertices = true;
    }

    if has_vertices {
        crate::stream::TileAabb { min, max }
    } else {
        let rect = coord.rect(tile_size);
        crate::stream::TileAabb {
            min: glam::Vec3::new(rect.min_x, 0.0, rect.min_z),
            max: glam::Vec3::new(rect.max_x, 1.0, rect.max_z),
        }
    }
}

fn generate_tile_lod_mesh(
    source: &WorldSource,
    coord: crate::stream::TileCoord,
    refs: &crate::stream::tile::TileFeatureRefs,
    tile_size: f32,
    lod: crate::stream::TileLod,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
) -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let rect = coord.rect(tile_size);

    let terrain_cuts = terrain_cuts::terrain_cuts_for_road_refs(source, &refs.roads);
    let terrain_ctx = crate::world::terrain::TerrainContext {
        conv: &source.conv,
        elevation: source.elevation.as_ref(),
        cuts: &terrain_cuts,
    };
    let mut terrain_output = crate::world::terrain::MeshOutput {
        vertices: &mut vertices,
        indices: &mut indices,
    };
    crate::world::terrain::generate_terrain_for_world_rect_with_cuts(
        rect.min_x,
        rect.min_z,
        rect.max_x,
        rect.max_z,
        crate::stream::LodConfig::terrain_spacing(lod),
        &terrain_ctx,
        &mut terrain_output,
    );

    append_tile_features_mesh(
        source,
        refs,
        lod,
        visual_detail,
        &mut vertices,
        &mut indices,
    );

    CpuMesh { vertices, indices }
}

fn append_tile_features_mesh(
    source: &WorldSource,
    refs: &crate::stream::tile::TileFeatureRefs,
    lod: crate::stream::TileLod,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    for &feature_idx in &refs.landuses {
        let Some(lu) = source.landuses.get(feature_idx) else {
            continue;
        };
        if lod == crate::stream::TileLod::Far && is_green_landuse_overlay(&lu.tags) {
            continue;
        }
        let color = crate::world::color::landuse_color(&lu.tags);
        let y_offset = crate::world::landuse::landuse_y_offset(&lu.tags);
        crate::world::landuse::generate_landuse_with_elevations_and_offset(
            &lu.points,
            &lu.elevations,
            y_offset,
            color,
            verts,
            idxs,
        );
    }

    for &feature_idx in &refs.waters {
        let Some(w) = source.waters.get(feature_idx) else {
            continue;
        };
        crate::world::water::generate_water_with_elevations(&w.points, &w.elevations, verts, idxs);
    }
    water::append_uncovered_waterway_meshes(source, &refs.waterways, verts, idxs);

    roads::append_tile_roads_mesh(source, &refs.roads, lod, verts, idxs);

    for &feature_idx in &refs.railways {
        let Some(railway) = source.railways.get(feature_idx) else {
            continue;
        };
        crate::world::railway::generate_railway_track(
            &railway.tags,
            &railway.points,
            &railway.elevations,
            verts,
            idxs,
        );
    }

    for &feature_idx in &refs.point_features {
        let Some(point_feature) = source.point_features.get(feature_idx) else {
            continue;
        };
        crate::world::point_feature::generate_point_feature_with_visual_detail(
            &point_feature.tags,
            point_feature.point,
            point_feature.elevation,
            visual_detail,
            verts,
            idxs,
        );
    }

    for &feature_idx in &refs.street_signs {
        let Some(sign) = source.street_signs.get(feature_idx) else {
            continue;
        };
        crate::world::street_sign::append_street_sign(sign, verts, idxs);
    }

    for &feature_idx in &refs.buildings {
        let Some(b) = source.buildings.get(feature_idx) else {
            continue;
        };
        let style = crate::world::color::building_style(
            &b.tags,
            feature_idx as u64,
            visual_detail.facade_variation,
            visual_detail.roof_variation,
        );
        let base_y = source.elevation_at(b.rep_lat, b.rep_lon);
        let height = crate::world::building::parse_building_height(&b.tags);
        let mut footprint = b.points.clone();
        if footprint.len() > 3 && footprint.first() == footprint.last() {
            footprint.pop();
        }
        if lod == crate::stream::TileLod::Far {
            crate::world::building::generate_simplified_building_with_style(
                &footprint, base_y, height, style, verts, idxs,
            );
        } else {
            crate::world::building::generate_building_with_style(
                &footprint, base_y, height, style, verts, idxs,
            );
        }
    }
}

fn is_green_landuse_overlay(tags: &std::collections::HashMap<String, String>) -> bool {
    crate::world::landuse::landuse_y_offset(tags) > crate::world::landuse::LANDUSE_Y_OFFSET
}

/// Load and process OSM data, generating all meshes.
pub fn load_world(
    pbf_path: &std::path::Path,
    srtm_dir: Option<&std::path::Path>,
) -> anyhow::Result<WorldMesh> {
    let source = super::source::load_world_source(pbf_path, srtm_dir)?;
    Ok(generate_world_mesh(&source))
}

/// Test-only re-export of the private `generate_tile_lod_mesh` function.
#[cfg(test)]
pub fn generate_tile_lod_mesh_reexport(
    source: &WorldSource,
    coord: crate::stream::TileCoord,
    refs: &crate::stream::tile::TileFeatureRefs,
    tile_size: f32,
    lod: crate::stream::TileLod,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
) -> CpuMesh {
    generate_tile_lod_mesh(source, coord, refs, tile_size, lod, visual_detail)
}
