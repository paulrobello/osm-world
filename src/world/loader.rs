//! World loading orchestrator.

use std::collections::HashMap;
use std::path::Path;

use crate::geo::{CoordConverter, ElevationData};
use crate::osm::parse::parse_pbf;
use crate::render::vertex::Vertex;

/// Combined mesh for the entire world.
pub struct WorldMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub center: (f32, f32, f32),
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
    // 1. Parse PBF
    let osm_data = parse_pbf(pbf_path)?;

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
        let is_water = tags.contains_key("waterway")
            || tags.get("natural").map(|s| s.as_str()) == Some("water")
            || tags.get("natural").map(|s| s.as_str()) == Some("wetland")
            || tags.get("landuse").map(|s| s.as_str()) == Some("basin")
            || tags.get("landuse").map(|s| s.as_str()) == Some("reservoir");
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
        let y = source.elevation_at(w.rep_lat, w.rep_lon) + 0.3;
        super::water::generate_water(&w.points, y, verts, idxs);
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

    let road_layer_offset = |width: f32| -> f32 {
        if width >= 5.0 {
            0.30
        } else if width >= 3.5 {
            0.20
        } else {
            0.10
        }
    };

    let mut road_caps: HashMap<RoadPointKey, RoadCap> = HashMap::new();
    for r in &source.roads {
        let width = super::color::road_width(&r.tags);
        let color = super::color::road_color(&r.tags);
        let layer_offset = road_layer_offset(width);
        let road_elevations: Vec<f32> = r.elevations.iter().map(|e| e + layer_offset).collect();
        super::road::generate_road_with_elevations(
            &r.points,
            &road_elevations,
            width,
            color,
            verts,
            idxs,
        );

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

/// Load and process OSM data, generating all meshes.
pub fn load_world(pbf_path: &Path, srtm_dir: Option<&Path>) -> anyhow::Result<WorldMesh> {
    let source = load_world_source(pbf_path, srtm_dir)?;
    Ok(generate_world_mesh(&source))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
