//! World loading orchestrator.

use std::collections::HashMap;
use std::path::Path;

use crate::geo::{CoordConverter, ElevationData};
use crate::osm::parse::parse_osm_file;
use crate::render::vertex::Vertex;

/// Combined mesh for the entire world.
pub struct WorldMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub center: (f32, f32, f32),
}

#[derive(Clone, Debug)]
pub struct CpuMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct TileMeshSet {
    pub coord: crate::stream::TileCoord,
    pub aabb: crate::stream::TileAabb,
    pub lods: [CpuMesh; 3],
}

#[derive(Clone, Debug)]
pub struct ResolvedFeature {
    pub tags: HashMap<String, String>,
    pub points: Vec<(f32, f32)>,
    pub elevations: Vec<f32>,
    pub rep_lat: f64,
    pub rep_lon: f64,
}

pub struct WorldSource {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
    pub conv: CoordConverter,
    pub elevation: Option<ElevationData>,
    pub buildings: Vec<ResolvedFeature>,
    pub roads: Vec<ResolvedFeature>,
    pub waters: Vec<ResolvedFeature>,
    pub landuses: Vec<ResolvedFeature>,
}

impl WorldSource {
    pub fn elevation_at(&self, lat: f64, lon: f64) -> f32 {
        self.elevation
            .as_ref()
            .and_then(|e| e.elevation_at(lat, lon))
            .unwrap_or(0.0) as f32
    }

    pub fn feature_index_for_tile_size(
        &self,
        tile_size: f32,
    ) -> HashMap<crate::stream::TileCoord, crate::stream::tile::TileFeatureRefs> {
        let mut index = HashMap::new();

        if let Some((min_x, min_z, max_x, max_z)) = self.world_bbox() {
            for coord in tiles_for_half_open_bbox(min_x, min_z, max_x, max_z, tile_size) {
                index
                    .entry(coord)
                    .or_insert_with(crate::stream::tile::TileFeatureRefs::default);
            }
        }

        for (feature_idx, feature) in self.buildings.iter().enumerate() {
            if let Some(coord) = feature_owner_tile(feature, tile_size) {
                index
                    .entry(coord)
                    .or_insert_with(crate::stream::tile::TileFeatureRefs::default)
                    .buildings
                    .push(feature_idx);
            }
        }
        for (feature_idx, feature) in self.roads.iter().enumerate() {
            if let Some(coord) = feature_owner_tile(feature, tile_size) {
                index
                    .entry(coord)
                    .or_insert_with(crate::stream::tile::TileFeatureRefs::default)
                    .roads
                    .push(feature_idx);
            }
        }
        for (feature_idx, feature) in self.waters.iter().enumerate() {
            if let Some(coord) = feature_owner_tile(feature, tile_size) {
                index
                    .entry(coord)
                    .or_insert_with(crate::stream::tile::TileFeatureRefs::default)
                    .waters
                    .push(feature_idx);
            }
        }
        for (feature_idx, feature) in self.landuses.iter().enumerate() {
            if let Some(coord) = feature_owner_tile(feature, tile_size) {
                index
                    .entry(coord)
                    .or_insert_with(crate::stream::tile::TileFeatureRefs::default)
                    .landuses
                    .push(feature_idx);
            }
        }

        index
    }

    fn world_bbox(&self) -> Option<(f32, f32, f32, f32)> {
        let (x0, z0) = self.conv.to_world_xz(self.max_lat, self.min_lon);
        let (x1, z1) = self.conv.to_world_xz(self.min_lat, self.max_lon);
        let min_x = x0.min(x1);
        let max_x = x0.max(x1);
        let min_z = z0.min(z1);
        let max_z = z0.max(z1);
        (min_x.is_finite() && min_z.is_finite() && max_x.is_finite() && max_z.is_finite())
            .then_some((min_x, min_z, max_x, max_z))
    }
}

fn next_down_f32(value: f32) -> f32 {
    if value.is_nan() || value == f32::NEG_INFINITY {
        value
    } else if value == f32::INFINITY {
        f32::MAX
    } else if value == 0.0 {
        -f32::MIN_POSITIVE
    } else if value > 0.0 {
        f32::from_bits(value.to_bits() - 1)
    } else {
        f32::from_bits(value.to_bits() + 1)
    }
}

fn tiles_for_half_open_bbox(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    tile_size: f32,
) -> Vec<crate::stream::TileCoord> {
    if tile_size <= 0.0
        || !min_x.is_finite()
        || !min_z.is_finite()
        || !max_x.is_finite()
        || !max_z.is_finite()
        || min_x >= max_x
        || min_z >= max_z
    {
        return Vec::new();
    }

    let start = crate::stream::TileCoord::from_world(min_x, min_z, tile_size);
    let end =
        crate::stream::TileCoord::from_world(next_down_f32(max_x), next_down_f32(max_z), tile_size);
    let mut out = Vec::new();
    for z in start.z..=end.z {
        for x in start.x..=end.x {
            out.push(crate::stream::TileCoord { x, z });
        }
    }
    out
}

fn feature_bbox(feature: &ResolvedFeature) -> Option<(f32, f32, f32, f32)> {
    let mut iter = feature.points.iter();
    let &(first_x, first_z) = iter.next()?;
    let (mut min_x, mut max_x) = (first_x, first_x);
    let (mut min_z, mut max_z) = (first_z, first_z);
    for &(x, z) in iter {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    Some((min_x, min_z, max_x, max_z))
}

fn feature_owner_tile(
    feature: &ResolvedFeature,
    tile_size: f32,
) -> Option<crate::stream::TileCoord> {
    let (min_x, min_z, max_x, max_z) = feature_bbox(feature)?;
    let center_x = (min_x + max_x) * 0.5;
    let center_z = (min_z + max_z) * 0.5;
    Some(crate::stream::TileCoord::from_world(
        center_x, center_z, tile_size,
    ))
}

// Ensure CCW winding for polygon features while keeping per-vertex data aligned.
fn ensure_ccw(poly: &mut [(f32, f32)], elevations: &mut [f32]) {
    if poly.len() < 3 {
        return;
    }
    let area: f32 = poly
        .iter()
        .enumerate()
        .map(|(i, (x0, y0))| {
            let (x1, y1) = poly[(i + 1) % poly.len()];
            x0 * y1 - x1 * y0
        })
        .sum();
    if area < 0.0 {
        poly.reverse();
        elevations.reverse();
    }
}

pub fn load_world_source(pbf_path: &Path, srtm_dir: Option<&Path>) -> anyhow::Result<WorldSource> {
    // 1. Parse OSM input (PBF or XML)
    let osm_data = parse_osm_file(pbf_path)?;

    // 2. Get bounding box
    let (min_lat, min_lon, max_lat, max_lon) = osm_data
        .bounds
        .ok_or_else(|| anyhow::anyhow!("OSM data has no bounding box"))?;

    // 3. Coordinate converter with SW corner as origin
    let conv = CoordConverter::new(min_lat, min_lon);

    // 4. Load elevation data
    let elevation = match srtm_dir {
        Some(dir) => Some(ElevationData::from_path(dir)?),
        None => None,
    };

    // Helper to get elevation at a lat/lon
    let elev = |lat: f64, lon: f64| -> f32 {
        elevation
            .as_ref()
            .and_then(|e| e.elevation_at(lat, lon))
            .unwrap_or(0.0) as f32
    };

    let mut buildings: Vec<ResolvedFeature> = Vec::new();
    let mut roads: Vec<ResolvedFeature> = Vec::new();
    let mut waters: Vec<ResolvedFeature> = Vec::new();
    let mut landuses: Vec<ResolvedFeature> = Vec::new();

    // 5. Resolve ways to world coordinates and classify
    for way in &osm_data.ways {
        // Resolve node references to world coordinates
        let mut points = Vec::with_capacity(way.node_refs.len());
        let mut elevations = Vec::with_capacity(way.node_refs.len());
        let mut sum_lat = 0.0f64;
        let mut sum_lon = 0.0f64;
        let mut count = 0usize;

        for &node_id in &way.node_refs {
            if let Some(node) = osm_data.nodes.get(&node_id) {
                let (x, z) = conv.to_world_xz(node.lat, node.lon);
                points.push((x, z));
                elevations.push(elev(node.lat, node.lon));
                sum_lat += node.lat;
                sum_lon += node.lon;
                count += 1;
            }
        }

        if points.len() < 2 {
            continue;
        }

        let rep_lat = if count > 0 {
            sum_lat / count as f64
        } else {
            0.0
        };
        let rep_lon = if count > 0 {
            sum_lon / count as f64
        } else {
            0.0
        };

        // Determine if the way is closed
        let is_closed = way.node_refs.len() >= 4 && way.node_refs.first() == way.node_refs.last();

        let resolved = ResolvedFeature {
            tags: way.tags.clone(),
            points,
            elevations,
            rep_lat,
            rep_lon,
        };

        // 6. Classify
        let tags = &way.tags;

        let is_building = tags.contains_key("building") && is_closed;
        let is_road = tags.contains_key("highway");
        let is_water = is_closed
            && (tags.contains_key("waterway")
                || tags.get("natural").map(|s| s.as_str()) == Some("water")
                || tags.get("natural").map(|s| s.as_str()) == Some("wetland")
                || tags.get("landuse").map(|s| s.as_str()) == Some("basin")
                || tags.get("landuse").map(|s| s.as_str()) == Some("reservoir"));
        let is_landuse = !is_building
            && !is_water
            && is_closed
            && (tags.contains_key("landuse")
                || (tags.contains_key("natural")
                    && tags.get("natural").map(|s| s.as_str()) != Some("water")
                    && tags.get("natural").map(|s| s.as_str()) != Some("wetland"))
                || tags.contains_key("leisure"));

        if is_building {
            buildings.push(resolved.clone());
        } else if is_water {
            waters.push(resolved.clone());
        } else if is_landuse {
            landuses.push(resolved.clone());
        }
        if is_road {
            roads.push(resolved);
        }
    }

    // Also process multipolygon relations as potential landuse/water features
    for rel in &osm_data.relations {
        let mut all_points: Vec<(f32, f32)> = Vec::new();
        let mut elevations: Vec<f32> = Vec::new();
        let mut sum_lat = 0.0f64;
        let mut sum_lon = 0.0f64;
        let mut count = 0usize;

        for member in &rel.members {
            if let Some(&way_idx) = osm_data.ways_by_id.get(&member.way_id) {
                if member.role == "outer" {
                    let way = &osm_data.ways[way_idx];
                    for &node_id in &way.node_refs {
                        if let Some(node) = osm_data.nodes.get(&node_id) {
                            let (x, z) = conv.to_world_xz(node.lat, node.lon);
                            all_points.push((x, z));
                            elevations.push(elev(node.lat, node.lon));
                            sum_lat += node.lat;
                            sum_lon += node.lon;
                            count += 1;
                        }
                    }
                }
            }
        }

        if all_points.len() < 3 {
            continue;
        }

        let rep_lat = sum_lat / count as f64;
        let rep_lon = sum_lon / count as f64;
        let tags = &rel.tags;

        let is_water = tags.get("natural").map(|s| s.as_str()) == Some("water")
            || tags.get("natural").map(|s| s.as_str()) == Some("wetland")
            || tags.get("landuse").map(|s| s.as_str()) == Some("basin")
            || tags.get("landuse").map(|s| s.as_str()) == Some("reservoir");

        let resolved = ResolvedFeature {
            tags: rel.tags.clone(),
            points: all_points,
            elevations,
            rep_lat,
            rep_lon,
        };

        if is_water {
            waters.push(resolved);
        } else {
            landuses.push(resolved);
        }
    }

    // Ensure CCW winding for all polygon-based features (OSM data can be either winding)
    for b in &mut buildings {
        ensure_ccw(&mut b.points, &mut b.elevations);
    }
    for w in &mut waters {
        ensure_ccw(&mut w.points, &mut w.elevations);
    }
    for lu in &mut landuses {
        ensure_ccw(&mut lu.points, &mut lu.elevations);
    }

    Ok(WorldSource {
        min_lat,
        min_lon,
        max_lat,
        max_lon,
        conv,
        elevation,
        buildings,
        roads,
        waters,
        landuses,
    })
}

pub fn generate_world_mesh(source: &WorldSource) -> WorldMesh {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();
    append_world_mesh(source, &mut verts, &mut idxs);

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

fn append_road_feature_mesh(
    road: &ResolvedFeature,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) -> (Vec<f32>, f32, [f32; 3]) {
    let width = super::color::road_width(&road.tags);
    let color = super::color::road_color(&road.tags);
    let layer_offset = super::road::road_layer_y_offset(&road.tags);
    let road_elevations: Vec<f32> = road.elevations.iter().map(|e| e + layer_offset).collect();

    super::road::generate_road_with_elevations(
        &road.points,
        &road_elevations,
        width,
        color,
        verts,
        idxs,
    );
    super::road::append_road_structures(
        &road.tags,
        &road.points,
        &road.elevations,
        &road_elevations,
        width,
        verts,
        idxs,
    );

    (road_elevations, width, color)
}

fn append_world_mesh(source: &WorldSource, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    // Generate meshes in order: terrain, landuse, water, roads, buildings

    // Terrain
    super::terrain::generate_terrain(
        source.min_lat,
        source.min_lon,
        source.max_lat,
        source.max_lon,
        &source.conv,
        source.elevation.as_ref(),
        verts,
        idxs,
    );

    // Landuse
    for lu in &source.landuses {
        let color = super::color::landuse_color(&lu.tags);
        let y_offset = super::landuse::landuse_y_offset(&lu.tags);
        super::landuse::generate_landuse_with_elevations_and_offset(
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
        super::water::generate_water_with_elevations(&w.points, &w.elevations, verts, idxs);
    }

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
    for r in &source.roads {
        let (road_elevations, width, color) = append_road_feature_mesh(r, verts, idxs);

        let is_closed = r.points.len() >= 4 && r.points.first() == r.points.last();
        for (i, (&point, &elevation)) in r.points.iter().zip(&road_elevations).enumerate() {
            let key = road_key(point);
            let count = road_point_counts.get(&key).copied().unwrap_or(0);
            let is_dead_end = !is_closed && (i == 0 || i + 1 == r.points.len()) && count == 1;
            if !is_dead_end {
                continue;
            }

            let radius_scale = super::road::ROAD_CAP_RADIUS_SCALE;

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
        super::road::append_road_cap_with_radius_scale(
            cap.point,
            cap.elevation,
            cap.width,
            cap.radius_scale,
            cap.color,
            verts,
            idxs,
        );
    }

    // Buildings
    for b in &source.buildings {
        let color = super::color::building_color(&b.tags);
        let base_y = source.elevation_at(b.rep_lat, b.rep_lon);
        let height = super::building::parse_building_height(&b.tags);
        // Remove trailing duplicate point if present for the footprint
        let mut footprint = b.points.clone();
        if footprint.len() > 3 && footprint.first() == footprint.last() {
            footprint.pop();
        }
        super::building::generate_building(&footprint, base_y, height, color, verts, idxs);
    }

    log::info!(
        "Generated world mesh: {} vertices, {} indices, {} buildings, {} roads, {} water areas, {} landuse areas",
        verts.len(),
        idxs.len(),
        source.buildings.len(),
        source.roads.len(),
        source.waters.len(),
        source.landuses.len(),
    );
}

pub fn generate_tile_mesh_set(
    source: &WorldSource,
    coord: crate::stream::TileCoord,
    refs: &crate::stream::tile::TileFeatureRefs,
    tile_size: f32,
) -> TileMeshSet {
    let lods = [
        generate_tile_lod_mesh(source, coord, refs, tile_size, crate::stream::TileLod::Near),
        generate_tile_lod_mesh(source, coord, refs, tile_size, crate::stream::TileLod::Mid),
        generate_tile_lod_mesh(source, coord, refs, tile_size, crate::stream::TileLod::Far),
    ];
    let aabb = aabb_for_lods(coord, tile_size, &lods);
    TileMeshSet { coord, aabb, lods }
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
) -> CpuMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let rect = coord.rect(tile_size);

    super::terrain::generate_terrain_for_world_rect(
        rect.min_x,
        rect.min_z,
        rect.max_x,
        rect.max_z,
        crate::stream::LodConfig::terrain_spacing(lod),
        &source.conv,
        source.elevation.as_ref(),
        &mut vertices,
        &mut indices,
    );

    append_tile_features_mesh(source, refs, lod, &mut vertices, &mut indices);

    CpuMesh { vertices, indices }
}

fn append_tile_features_mesh(
    source: &WorldSource,
    refs: &crate::stream::tile::TileFeatureRefs,
    lod: crate::stream::TileLod,
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
        let color = super::color::landuse_color(&lu.tags);
        let y_offset = super::landuse::landuse_y_offset(&lu.tags);
        super::landuse::generate_landuse_with_elevations_and_offset(
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
        super::water::generate_water_with_elevations(&w.points, &w.elevations, verts, idxs);
    }

    append_tile_roads_mesh(source, &refs.roads, lod, verts, idxs);

    for &feature_idx in &refs.buildings {
        let Some(b) = source.buildings.get(feature_idx) else {
            continue;
        };
        let color = super::color::building_color(&b.tags);
        let base_y = source.elevation_at(b.rep_lat, b.rep_lon);
        let height = super::building::parse_building_height(&b.tags);
        let mut footprint = b.points.clone();
        if footprint.len() > 3 && footprint.first() == footprint.last() {
            footprint.pop();
        }
        super::building::generate_building(&footprint, base_y, height, color, verts, idxs);
    }
}

fn append_tile_roads_mesh(
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

    let selected_roads: Vec<&ResolvedFeature> = road_refs
        .iter()
        .filter_map(|&feature_idx| source.roads.get(feature_idx))
        .filter(|road| lod != crate::stream::TileLod::Far || !is_minor_highway(&road.tags))
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
    for r in selected_roads {
        let (road_elevations, width, color) = append_road_feature_mesh(r, verts, idxs);

        let is_closed = r.points.len() >= 4 && r.points.first() == r.points.last();
        for (i, (&point, &elevation)) in r.points.iter().zip(&road_elevations).enumerate() {
            let key = road_key(point);
            let count = road_point_counts.get(&key).copied().unwrap_or(0);
            let is_dead_end = !is_closed && (i == 0 || i + 1 == r.points.len()) && count == 1;
            if !is_dead_end {
                continue;
            }

            let radius_scale = super::road::ROAD_CAP_RADIUS_SCALE;

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
        super::road::append_road_cap_with_radius_scale(
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

fn is_green_landuse_overlay(tags: &HashMap<String, String>) -> bool {
    super::landuse::landuse_y_offset(tags) > super::landuse::LANDUSE_Y_OFFSET
}

fn is_minor_highway(tags: &HashMap<String, String>) -> bool {
    matches!(
        tags.get("highway").map(String::as_str),
        Some("footway" | "path" | "cycleway" | "steps")
    )
}

/// Load and process OSM data, generating all meshes.
pub fn load_world(pbf_path: &Path, srtm_dir: Option<&Path>) -> anyhow::Result<WorldMesh> {
    let source = load_world_source(pbf_path, srtm_dir)?;
    Ok(generate_world_mesh(&source))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_world_source_accepts_osm_xml_input() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("area.osm");
        std::fs::write(
            &path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.999"/>
  <node id="3" lat="38.001" lon="-120.999"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <tag k="highway" v="residential"/>
  </way>
</osm>"#,
        )
        .unwrap();

        let source = load_world_source(&path, None).unwrap();

        assert_eq!(source.roads.len(), 1);
        assert!(source.min_lat <= 38.0);
        assert!(source.max_lat >= 38.001);
    }

    #[test]
    fn load_world_source_does_not_treat_open_waterways_as_water_polygons() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("waterways.osm");
        std::fs::write(
            &path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.99"/>
  <node id="3" lat="38.01" lon="-120.99"/>
  <node id="4" lat="38.01" lon="-121.0"/>
  <node id="5" lat="38.02" lon="-121.0"/>
  <node id="6" lat="38.03" lon="-120.98"/>
  <node id="7" lat="38.04" lon="-121.0"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <nd ref="4"/>
    <nd ref="1"/>
    <tag k="natural" v="water"/>
  </way>
  <way id="11">
    <nd ref="5"/>
    <nd ref="6"/>
    <nd ref="7"/>
    <tag k="waterway" v="river"/>
  </way>
</osm>"#,
        )
        .unwrap();

        let source = load_world_source(&path, None).unwrap();

        assert_eq!(source.waters.len(), 1);
        assert_eq!(
            source.waters[0].tags.get("natural").map(String::as_str),
            Some("water")
        );
    }

    #[test]
    fn world_source_bbox_center_matches_converter() {
        let source = WorldSource {
            min_lat: 1.0,
            min_lon: 2.0,
            max_lat: 1.1,
            max_lon: 2.2,
            conv: CoordConverter::new(1.0, 2.0),
            elevation: None,
            buildings: Vec::new(),
            roads: Vec::new(),
            waters: Vec::new(),
            landuses: Vec::new(),
        };

        let (cx, cz) = source.conv.bbox_centre(
            source.min_lat,
            source.min_lon,
            source.max_lat,
            source.max_lon,
        );
        assert!(cx > 0.0);
        assert!(cz < 0.0);
    }

    #[test]
    fn ensure_ccw_keeps_elevations_aligned_when_reversing() {
        let mut points = vec![(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)];
        let mut elevations = vec![1.0, 2.0, 3.0, 4.0];

        ensure_ccw(&mut points, &mut elevations);

        assert_eq!(
            points,
            vec![(10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)]
        );
        assert_eq!(elevations, vec![4.0, 3.0, 2.0, 1.0]);
    }

    fn feature(tag_key: &str, tag_value: &str, points: Vec<(f32, f32)>) -> ResolvedFeature {
        let mut tags = HashMap::new();
        tags.insert(tag_key.to_string(), tag_value.to_string());
        let elevations = vec![0.0; points.len()];
        ResolvedFeature {
            tags,
            points,
            elevations,
            rep_lat: 1.0,
            rep_lon: 2.0,
        }
    }

    fn empty_source() -> WorldSource {
        WorldSource {
            min_lat: 1.0,
            min_lon: 2.0,
            max_lat: 1.1,
            max_lon: 2.2,
            conv: CoordConverter::new(1.0, 2.0),
            elevation: None,
            buildings: Vec::new(),
            roads: Vec::new(),
            waters: Vec::new(),
            landuses: Vec::new(),
        }
    }

    #[test]
    fn feature_index_maps_feature_bboxes_to_owner_tiles() {
        let mut source = empty_source();
        source.buildings.push(feature(
            "building",
            "yes",
            vec![(10.0, -10.0), (20.0, -10.0), (20.0, -20.0)],
        ));
        source.roads.push(feature(
            "highway",
            "residential",
            vec![(110.0, -10.0), (120.0, -10.0)],
        ));
        source.waters.push(feature(
            "natural",
            "water",
            vec![(-10.0, -10.0), (10.0, -10.0), (10.0, -20.0)],
        ));
        source.landuses.push(feature(
            "landuse",
            "residential",
            vec![(10.0, -210.0), (20.0, -210.0), (20.0, -220.0)],
        ));

        let index = source.feature_index_for_tile_size(100.0);

        assert_eq!(
            index
                .get(&crate::stream::TileCoord { x: 0, z: -1 })
                .unwrap()
                .buildings,
            vec![0]
        );
        assert_eq!(
            index
                .get(&crate::stream::TileCoord { x: 1, z: -1 })
                .unwrap()
                .roads,
            vec![0]
        );
        assert_eq!(
            index
                .get(&crate::stream::TileCoord { x: 0, z: -1 })
                .unwrap()
                .waters,
            vec![0]
        );
        assert_eq!(
            index
                .get(&crate::stream::TileCoord { x: 0, z: -3 })
                .unwrap()
                .landuses,
            vec![0]
        );
    }

    #[test]
    fn cross_tile_feature_is_referenced_and_emitted_by_one_owner_tile_only() {
        let mut source = empty_source();
        source.max_lat = 1.002;
        source.max_lon = 2.002;
        source.roads.push(feature(
            "highway",
            "residential",
            vec![(80.0, -50.0), (140.0, -50.0)],
        ));

        let index = source.feature_index_for_tile_size(100.0);
        let owner_tiles: Vec<_> = index
            .iter()
            .filter_map(|(coord, refs)| refs.roads.contains(&0).then_some(*coord))
            .collect();
        assert_eq!(owner_tiles, vec![crate::stream::TileCoord { x: 1, z: -1 }]);

        let tiles_emitting_road = index
            .iter()
            .filter(|(coord, refs)| {
                let mesh = generate_tile_mesh_set(&source, **coord, refs, 100.0);
                mesh.lods[crate::stream::TileLod::Near as usize]
                    .vertices
                    .iter()
                    .any(|v| v.feature_type == crate::render::vertex::feature::ROAD)
            })
            .count();
        assert_eq!(tiles_emitting_road, 1);
    }

    #[test]
    fn feature_index_includes_empty_terrain_tiles_for_world_bbox() {
        let mut source = empty_source();
        source.max_lat = 1.002;
        source.max_lon = 2.002;

        let index = source.feature_index_for_tile_size(100.0);

        assert!(!index.is_empty());
        assert!(index.contains_key(&crate::stream::TileCoord { x: 0, z: -3 }));
        assert!(index.contains_key(&crate::stream::TileCoord { x: 2, z: -1 }));
        assert!(!index.contains_key(&crate::stream::TileCoord { x: 2, z: 0 }));
        assert!(index.values().all(|refs| refs.buildings.is_empty()
            && refs.roads.is_empty()
            && refs.waters.is_empty()
            && refs.landuses.is_empty()));
    }

    #[test]
    fn terrain_seeding_treats_world_bbox_max_as_half_open_on_tile_boundary() {
        let mut source = empty_source();
        let metres_per_deg_lon = 111_320.0 * source.min_lat.to_radians().cos();
        source.max_lat = source.min_lat + 200.0 / 111_320.0;
        source.max_lon = source.min_lon + 50.0 / metres_per_deg_lon;

        let index = source.feature_index_for_tile_size(100.0);
        let mut z_tiles: Vec<_> = index.keys().map(|coord| coord.z).collect();
        z_tiles.sort_unstable();
        z_tiles.dedup();

        assert_eq!(z_tiles, vec![-2, -1]);
        assert!(!index.contains_key(&crate::stream::TileCoord { x: 0, z: 0 }));
    }

    #[test]
    fn tile_roads_do_not_cap_endpoint_connected_to_neighbor_owned_road() {
        let mut source = empty_source();
        source.roads.push(feature(
            "highway",
            "residential",
            vec![(0.0, -50.0), (100.0, -50.0)],
        ));
        source.roads.push(feature(
            "highway",
            "residential",
            vec![(100.0, -50.0), (200.0, -50.0)],
        ));
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_tile_roads_mesh(
            &source,
            &[0],
            crate::stream::TileLod::Near,
            &mut vertices,
            &mut indices,
        );

        let has_shared_endpoint_cap_center = vertices.iter().any(|v| {
            v.feature_type == crate::render::vertex::feature::ROAD
                && (v.position[0] - 100.0).abs() < 1e-4
                && (v.position[2] + 50.0).abs() < 1e-4
                && v.position[1] > super::super::road::ROAD_Y_OFFSET
        });
        assert!(!has_shared_endpoint_cap_center);
    }

    #[test]
    fn tile_road_mesh_emits_bridge_structure_geometry() {
        let mut source = empty_source();
        let mut bridge = feature("highway", "primary", vec![(0.0, -50.0), (30.0, -50.0)]);
        bridge.tags.insert("bridge".to_string(), "yes".to_string());
        bridge.elevations = vec![0.0, 0.0];
        source.roads.push(bridge);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_tile_roads_mesh(
            &source,
            &[0],
            crate::stream::TileLod::Near,
            &mut vertices,
            &mut indices,
        );

        assert!(!indices.is_empty());
        assert!(
            vertices
                .iter()
                .any(|v| v.feature_type == crate::render::vertex::feature::ROAD)
        );
        assert!(
            vertices
                .iter()
                .any(|v| v.feature_type == crate::render::vertex::feature::BUILDING)
        );
    }

    #[test]
    fn tile_road_mesh_emits_tunnel_portal_geometry() {
        let mut source = empty_source();
        let mut tunnel = feature("highway", "primary", vec![(0.0, -50.0), (30.0, -50.0)]);
        tunnel.tags.insert("tunnel".to_string(), "yes".to_string());
        tunnel.elevations = vec![0.0, 0.0];
        source.roads.push(tunnel);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_tile_roads_mesh(
            &source,
            &[0],
            crate::stream::TileLod::Near,
            &mut vertices,
            &mut indices,
        );

        let road_min_y = vertices
            .iter()
            .filter(|v| v.feature_type == crate::render::vertex::feature::ROAD)
            .map(|v| v.position[1])
            .fold(f32::INFINITY, f32::min);

        assert!(road_min_y < super::super::road::ROAD_Y_OFFSET);
        assert!(
            vertices
                .iter()
                .any(|v| v.feature_type == crate::render::vertex::feature::BUILDING)
        );
    }

    #[test]
    fn tile_mesh_aabb_bounds_cross_tile_geometry() {
        let mut source = empty_source();
        source.waters.push(feature(
            "natural",
            "water",
            vec![(80.0, -40.0), (140.0, -40.0), (140.0, -60.0), (80.0, -60.0)],
        ));
        let refs = crate::stream::tile::TileFeatureRefs {
            waters: vec![0],
            ..Default::default()
        };

        let mesh = generate_tile_mesh_set(
            &source,
            crate::stream::TileCoord { x: 1, z: -1 },
            &refs,
            100.0,
        );

        assert!(mesh.aabb.min.x <= 80.0, "aabb min x: {}", mesh.aabb.min.x);
        assert!(mesh.aabb.max.x >= 200.0, "aabb max x: {}", mesh.aabb.max.x);
        assert!(mesh.aabb.min.z <= -100.0, "aabb min z: {}", mesh.aabb.min.z);
        assert!(mesh.aabb.max.z >= 0.0, "aabb max z: {}", mesh.aabb.max.z);
    }

    #[test]
    fn tile_mesh_aabb_includes_actual_negative_elevations() {
        let mut source = empty_source();
        source.waters.push(ResolvedFeature {
            tags: HashMap::from([("natural".to_string(), "water".to_string())]),
            points: vec![(10.0, -10.0), (20.0, -10.0), (20.0, -20.0), (10.0, -20.0)],
            elevations: vec![-250.0, -240.0, -230.0, -220.0],
            rep_lat: 1.0,
            rep_lon: 2.0,
        });
        let refs = crate::stream::tile::TileFeatureRefs {
            waters: vec![0],
            ..Default::default()
        };

        let mesh = generate_tile_mesh_set(
            &source,
            crate::stream::TileCoord { x: 0, z: -1 },
            &refs,
            100.0,
        );
        let min_vertex_y = mesh
            .lods
            .iter()
            .flat_map(|lod| lod.vertices.iter().map(|v| v.position[1]))
            .fold(f32::INFINITY, f32::min);

        assert_eq!(mesh.aabb.min.y, min_vertex_y);
        assert!(mesh.aabb.min.y < -200.0, "aabb min y: {}", mesh.aabb.min.y);
    }

    #[test]
    fn tile_mesh_lods_filter_far_details_and_keep_water_elevations() {
        let mut source = empty_source();
        source.landuses.push(feature(
            "leisure",
            "park",
            vec![(10.0, -10.0), (20.0, -10.0), (20.0, -20.0), (10.0, -20.0)],
        ));
        source.waters.push(ResolvedFeature {
            tags: HashMap::from([("natural".to_string(), "water".to_string())]),
            points: vec![(30.0, -30.0), (40.0, -30.0), (40.0, -40.0), (30.0, -40.0)],
            elevations: vec![5.0, 6.0, 7.0, 8.0],
            rep_lat: 1.0,
            rep_lon: 2.0,
        });
        source.roads.push(feature(
            "highway",
            "footway",
            vec![(50.0, -50.0), (60.0, -50.0)],
        ));

        let refs = crate::stream::tile::TileFeatureRefs {
            landuses: vec![0],
            waters: vec![0],
            roads: vec![0],
            ..Default::default()
        };
        let meshes = generate_tile_mesh_set(
            &source,
            crate::stream::TileCoord { x: 0, z: -1 },
            &refs,
            100.0,
        );

        let near = &meshes.lods[crate::stream::TileLod::Near as usize];
        let far = &meshes.lods[crate::stream::TileLod::Far as usize];
        assert!(
            near.vertices
                .iter()
                .any(|v| v.feature_type == crate::render::vertex::feature::LANDUSE)
        );
        assert!(
            near.vertices
                .iter()
                .any(|v| v.feature_type == crate::render::vertex::feature::ROAD)
        );
        assert!(
            !far.vertices
                .iter()
                .any(|v| v.feature_type == crate::render::vertex::feature::LANDUSE)
        );
        assert!(
            !far.vertices
                .iter()
                .any(|v| v.feature_type == crate::render::vertex::feature::ROAD)
        );

        let water_ys: Vec<f32> = near
            .vertices
            .iter()
            .filter(|v| v.feature_type == crate::render::vertex::feature::WATER)
            .map(|v| v.position[1])
            .collect();
        assert_eq!(
            water_ys,
            vec![
                5.0 + super::super::water::WATER_Y_OFFSET,
                6.0 + super::super::water::WATER_Y_OFFSET,
                7.0 + super::super::water::WATER_Y_OFFSET,
                8.0 + super::super::water::WATER_Y_OFFSET,
            ]
        );
    }
}
