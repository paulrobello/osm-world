//! Mesh generation from world source data.

use std::collections::{HashMap, HashSet};

use crate::render::vertex::Vertex;

use super::source::{
    CpuMesh, ResolvedFeature, TileMeshSet, WorldMesh, WorldSource,
};

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

fn append_road_feature_mesh_with_ramps(
    road: &ResolvedFeature,
    endpoint_ramps: crate::world::road::BridgeEndpointRamps,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) -> (Vec<f32>, f32, [f32; 3]) {
    let width = crate::world::color::road_width(&road.tags);
    let color = crate::world::color::road_color(&road.tags);
    let profile = crate::world::road::road_profile(&road.tags);
    let render_path = crate::world::road::road_render_path_with_bridge_endpoint_ramps(
        &road.tags,
        &road.points,
        &road.elevations,
        endpoint_ramps,
    );

    let is_surface = profile.kind == crate::world::road::RoadProfileKind::Surface;
    let feature_type = if !is_surface {
        crate::render::vertex::feature::ROAD_LAYERED
    } else if crate::world::color::is_sidewalk_like_road(&road.tags) {
        crate::render::vertex::feature::ROAD_PATH
    } else {
        crate::render::vertex::feature::ROAD
    };
    let marking_feature_type = if is_surface {
        crate::render::vertex::feature::ROAD_MARKING
    } else {
        crate::render::vertex::feature::ROAD_MARKING_LAYERED
    };
    crate::world::road::generate_road_with_elevations_and_feature_type(
        &render_path.points,
        &render_path.road_elevations,
        width,
        color,
        feature_type,
        verts,
        idxs,
    );
    crate::world::road::append_road_centerline_dashes_with_feature_type(
        &render_path.points,
        &render_path.road_elevations,
        width,
        marking_feature_type,
        verts,
        idxs,
    );
    crate::world::road::append_road_structures(
        &road.tags,
        &render_path.points,
        &render_path.terrain_elevations,
        &render_path.road_elevations,
        width,
        verts,
        idxs,
    );

    let cap_elevations =
        crate::world::road::road_render_elevations(&road.tags, &road.points, &road.elevations);
    (cap_elevations, width, color)
}

fn bridge_endpoint_ramps_for_road(
    source: &WorldSource,
    road_index: usize,
) -> crate::world::road::BridgeEndpointRamps {
    let Some(road) = source.roads.get(road_index) else {
        return crate::world::road::BridgeEndpointRamps::default();
    };
    if crate::world::road::road_profile(&road.tags).kind != crate::world::road::RoadProfileKind::Bridge {
        return crate::world::road::BridgeEndpointRamps::default();
    }

    crate::world::road::BridgeEndpointRamps {
        start: road
            .points
            .first()
            .is_none_or(|&point| !has_connected_bridge_road_at(source, road_index, point)),
        end: road
            .points
            .last()
            .is_none_or(|&point| !has_connected_bridge_road_at(source, road_index, point)),
    }
}

fn has_connected_bridge_road_at(
    source: &WorldSource,
    road_index: usize,
    point: (f32, f32),
) -> bool {
    source.roads.iter().enumerate().any(|(other_index, other)| {
        other_index != road_index
            && crate::world::road::road_profile(&other.tags).kind == crate::world::road::RoadProfileKind::Bridge
            && other
                .points
                .iter()
                .any(|&other_point| same_road_point(other_point, point))
    })
}

fn terrain_cuts_for_roads(roads: &[ResolvedFeature]) -> Vec<crate::world::terrain::TerrainCut> {
    roads.iter().flat_map(terrain_cuts_for_road).collect()
}

fn terrain_cuts_for_road_refs(
    source: &WorldSource,
    road_refs: &[usize],
) -> Vec<crate::world::terrain::TerrainCut> {
    road_refs
        .iter()
        .filter_map(|&feature_idx| source.roads.get(feature_idx))
        .flat_map(terrain_cuts_for_road)
        .collect()
}

fn terrain_cuts_for_road(road: &ResolvedFeature) -> Vec<crate::world::terrain::TerrainCut> {
    if crate::world::road::road_profile(&road.tags).kind != crate::world::road::RoadProfileKind::Tunnel
        || road.points.len() < 2
        || road.points.len() != road.elevations.len()
    {
        return Vec::new();
    }

    let road_elevations =
        crate::world::road::road_render_elevations(&road.tags, &road.points, &road.elevations);
    if road_elevations.len() != road.points.len() {
        return Vec::new();
    }

    let width = crate::world::color::road_width(&road.tags);
    let mut cuts = Vec::with_capacity(2);
    if let Some(cut) =
        tunnel_portal_terrain_cut(road.points[0], road.points[1], road_elevations[0], width)
    {
        cuts.push(cut);
    }
    let last = road.points.len() - 1;
    if let Some(cut) = tunnel_portal_terrain_cut(
        road.points[last],
        road.points[last - 1],
        road_elevations[last],
        width,
    ) {
        cuts.push(cut);
    }
    cuts
}

fn tunnel_portal_terrain_cut(
    point: (f32, f32),
    next: (f32, f32),
    road_elevation: f32,
    width: f32,
) -> Option<crate::world::terrain::TerrainCut> {
    let dx = next.0 - point.0;
    let dz = next.1 - point.1;
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1e-6 {
        return None;
    }

    let dir_x = dx / len;
    let dir_z = dz / len;
    let cut_length = 18.0_f32.min(len.max(8.0));
    Some(crate::world::terrain::TerrainCut {
        start: point,
        end: (point.0 + dir_x * cut_length, point.1 + dir_z * cut_length),
        half_width: width * 0.5 + 4.0,
        floor_y: road_elevation + crate::world::road::ROAD_Y_OFFSET + 0.25,
        blend_width: 12.0,
    })
}

fn append_world_mesh(
    source: &WorldSource,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    // Generate meshes in order: terrain, landuse, water, roads, railways, point features, street signs, buildings

    // Terrain
    let terrain_cuts = terrain_cuts_for_roads(&source.roads);
    crate::world::terrain::generate_terrain_with_cuts(
        source.min_lat,
        source.min_lon,
        source.max_lat,
        source.max_lon,
        &source.conv,
        source.elevation.as_ref(),
        &terrain_cuts,
        verts,
        idxs,
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
    append_uncovered_waterway_meshes(source, &all_waterway_refs, verts, idxs);

    // Roads
    type RoadPointKey = (i32, i32);
    struct RoadCap {
        point: (f32, f32),
        elevation: f32,
        width: f32,
        radius_scale: f32,
        color: [f32; 3],
    }

    let road_key = |point: (f32, f32)| -> RoadPointKey {
        (
            (point.0 * 10.0).round() as i32,
            (point.1 * 10.0).round() as i32,
        )
    };
    let mut road_point_counts: HashMap<RoadPointKey, usize> = HashMap::new();
    for r in &source.roads {
        let is_closed = r.points.len() >= 4 && r.points.first() == r.points.last();
        let count_len = if is_closed {
            r.points.len() - 1
        } else {
            r.points.len()
        };
        for &point in &r.points[..count_len] {
            *road_point_counts.entry(road_key(point)).or_default() += 1;
        }
    }

    let mut road_caps: HashMap<RoadPointKey, RoadCap> = HashMap::new();
    for (road_index, r) in source.roads.iter().enumerate() {
        let ramps = bridge_endpoint_ramps_for_road(source, road_index);
        let (road_elevations, width, color) =
            append_road_feature_mesh_with_ramps(r, ramps, verts, idxs);

        let is_closed = r.points.len() >= 4 && r.points.first() == r.points.last();
        for (i, (&point, &elevation)) in r.points.iter().zip(&road_elevations).enumerate() {
            let key = road_key(point);
            let count = road_point_counts.get(&key).copied().unwrap_or(0);
            let is_dead_end = !is_closed && (i == 0 || i + 1 == r.points.len()) && count == 1;
            if !is_dead_end {
                continue;
            }

            let radius_scale = crate::world::road::ROAD_CAP_RADIUS_SCALE;

            road_caps
                .entry(key)
                .and_modify(|cap| {
                    cap.elevation = cap.elevation.max(elevation);
                    if width < cap.width {
                        cap.width = width;
                        cap.color = color;
                    }
                })
                .or_insert(RoadCap {
                    point,
                    elevation,
                    width,
                    radius_scale,
                    color,
                });
        }
    }
    for (_key, cap) in road_caps {
        crate::world::road::append_road_cap_with_radius_scale(
            cap.point,
            cap.elevation,
            cap.width,
            cap.radius_scale,
            cap.color,
            verts,
            idxs,
        );
    }

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
    let terrain_cuts = terrain_cuts_for_road_refs(source, &road_refs);

    for coord in &coords {
        let rect = coord.rect(tile_size);
        crate::world::terrain::generate_terrain_for_world_rect_with_cuts(
            rect.min_x,
            rect.min_z,
            rect.max_x,
            rect.max_z,
            crate::stream::LodConfig::terrain_spacing(crate::stream::TileLod::Near),
            &source.conv,
            source.elevation.as_ref(),
            &terrain_cuts,
            &mut vertices,
            &mut indices,
        );
    }

    let water_refs = super::geometry::feature_indices_intersecting_tiles(&source.waters, &rects);
    append_clipped_water_meshes(source, &water_refs, &rects, &mut vertices, &mut indices);

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

fn append_clipped_water_meshes(
    source: &WorldSource,
    water_refs: &[usize],
    rects: &[crate::stream::TileRect],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    for &feature_idx in water_refs {
        let Some(water) = source.waters.get(feature_idx) else {
            continue;
        };
        let Some(bbox) = super::geometry::feature_bbox(water) else {
            continue;
        };
        for &rect in rects {
            if !super::geometry::bbox_intersects_rect(bbox, rect) {
                continue;
            }
            let clipped = super::geometry::clip_polygon_to_rect(&water.points, rect);
            if clipped.len() < 3 {
                continue;
            }
            let elevations: Vec<_> = clipped
                .iter()
                .map(|&(x, z)| {
                    let (lat, lon) = source.conv.world_xz_to_lat_lon(x, z);
                    source.elevation_at(lat, lon)
                })
                .collect();
            crate::world::water::generate_water_with_elevations(&clipped, &elevations, verts, idxs);
        }
    }
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

    let terrain_cuts = terrain_cuts_for_road_refs(source, &refs.roads);
    crate::world::terrain::generate_terrain_for_world_rect_with_cuts(
        rect.min_x,
        rect.min_z,
        rect.max_x,
        rect.max_z,
        crate::stream::LodConfig::terrain_spacing(lod),
        &source.conv,
        source.elevation.as_ref(),
        &terrain_cuts,
        &mut vertices,
        &mut indices,
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

fn append_uncovered_waterway_meshes(
    source: &WorldSource,
    waterway_refs: &[usize],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let mut emitted_segments = HashSet::new();
    for &feature_idx in waterway_refs {
        let Some(waterway) = source.waterways.get(feature_idx) else {
            continue;
        };
        if waterway.points.len() != waterway.elevations.len() {
            continue;
        }
        for segment_idx in 0..waterway.points.len().saturating_sub(1) {
            let a = waterway.points[segment_idx];
            let b = waterway.points[segment_idx + 1];
            if same_road_point(a, b) {
                continue;
            }
            let key = normalized_segment_key(a, b);
            if !emitted_segments.insert(key) {
                continue;
            }
            let midpoint = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
            if point_is_inside_any_water_area(source, midpoint) {
                continue;
            }
            crate::world::water::generate_waterway_with_elevations(
                &[a, b],
                &[
                    waterway.elevations[segment_idx],
                    waterway.elevations[segment_idx + 1],
                ],
                &waterway.tags,
                verts,
                idxs,
            );
        }
    }
}

fn normalized_segment_key(a: (f32, f32), b: (f32, f32)) -> ((i32, i32), (i32, i32)) {
    let qa = quantized_point_key(a);
    let qb = quantized_point_key(b);
    if qa <= qb { (qa, qb) } else { (qb, qa) }
}

fn quantized_point_key(point: (f32, f32)) -> (i32, i32) {
    (
        (point.0 * 10.0).round() as i32,
        (point.1 * 10.0).round() as i32,
    )
}

fn point_is_inside_any_water_area(source: &WorldSource, point: (f32, f32)) -> bool {
    source.waters.iter().any(|water| {
        super::geometry::feature_bbox(water).is_some_and(|bbox| {
            point.0 >= bbox.0
                && point.0 <= bbox.2
                && point.1 >= bbox.1
                && point.1 <= bbox.3
                && super::geometry::point_in_polygon(point, &water.points)
        })
    })
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
    append_uncovered_waterway_meshes(source, &refs.waterways, verts, idxs);

    append_tile_roads_mesh(source, &refs.roads, lod, verts, idxs);

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

pub(crate) fn append_tile_roads_mesh(
    source: &WorldSource,
    road_refs: &[usize],
    lod: crate::stream::TileLod,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    type RoadPointKey = (i32, i32);
    struct RoadCap {
        point: (f32, f32),
        elevation: f32,
        width: f32,
        radius_scale: f32,
        color: [f32; 3],
    }

    let road_key = |point: (f32, f32)| -> RoadPointKey {
        (
            (point.0 * 10.0).round() as i32,
            (point.1 * 10.0).round() as i32,
        )
    };

    let selected_roads: Vec<(usize, &ResolvedFeature)> = road_refs
        .iter()
        .filter_map(|&feature_idx| {
            source
                .roads
                .get(feature_idx)
                .map(|road| (feature_idx, road))
        })
        .filter(|(_, road)| lod != crate::stream::TileLod::Far || !is_minor_highway(&road.tags))
        .collect();

    let mut road_point_counts: HashMap<RoadPointKey, usize> = HashMap::new();
    for r in &source.roads {
        let is_closed = r.points.len() >= 4 && r.points.first() == r.points.last();
        let count_len = if is_closed {
            r.points.len() - 1
        } else {
            r.points.len()
        };
        for &point in &r.points[..count_len] {
            *road_point_counts.entry(road_key(point)).or_default() += 1;
        }
    }

    let mut road_caps: HashMap<RoadPointKey, RoadCap> = HashMap::new();
    for (road_index, r) in selected_roads {
        let ramps = bridge_endpoint_ramps_for_road(source, road_index);
        let (road_elevations, width, color) =
            append_road_feature_mesh_with_ramps(r, ramps, verts, idxs);

        let is_closed = r.points.len() >= 4 && r.points.first() == r.points.last();
        for (i, (&point, &elevation)) in r.points.iter().zip(&road_elevations).enumerate() {
            let key = road_key(point);
            let count = road_point_counts.get(&key).copied().unwrap_or(0);
            let is_dead_end = !is_closed && (i == 0 || i + 1 == r.points.len()) && count == 1;
            if !is_dead_end {
                continue;
            }

            let radius_scale = crate::world::road::ROAD_CAP_RADIUS_SCALE;

            road_caps
                .entry(key)
                .and_modify(|cap| {
                    cap.elevation = cap.elevation.max(elevation);
                    if width < cap.width {
                        cap.width = width;
                        cap.color = color;
                    }
                })
                .or_insert(RoadCap {
                    point,
                    elevation,
                    width,
                    radius_scale,
                    color,
                });
        }
    }
    for (_key, cap) in road_caps {
        crate::world::road::append_road_cap_with_radius_scale(
            cap.point,
            cap.elevation,
            cap.width,
            cap.radius_scale,
            cap.color,
            verts,
            idxs,
        );
    }
}

fn is_green_landuse_overlay(tags: &std::collections::HashMap<String, String>) -> bool {
    crate::world::landuse::landuse_y_offset(tags) > crate::world::landuse::LANDUSE_Y_OFFSET
}

fn is_minor_highway(tags: &std::collections::HashMap<String, String>) -> bool {
    matches!(
        tags.get("highway").map(String::as_str),
        Some("footway" | "path" | "cycleway" | "steps")
    )
}

/// Load and process OSM data, generating all meshes.
pub fn load_world(pbf_path: &std::path::Path, srtm_dir: Option<&std::path::Path>) -> anyhow::Result<WorldMesh> {
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
