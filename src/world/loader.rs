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

/// Load and process OSM data, generating all meshes.
pub fn load_world(pbf_path: &Path, srtm_dir: Option<&Path>) -> anyhow::Result<WorldMesh> {
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

    let mut verts: Vec<Vertex> = Vec::new();
    let mut idxs: Vec<u32> = Vec::new();

    // 5. Resolve ways to world coordinates and classify
    #[derive(Clone)]
    struct ResolvedWay {
        tags: HashMap<String, String>,
        points: Vec<(f32, f32)>,
        // Lat/lon of a representative point for elevation lookup
        rep_lat: f64,
        rep_lon: f64,
    }

    let mut buildings: Vec<ResolvedWay> = Vec::new();
    let mut roads: Vec<ResolvedWay> = Vec::new();
    let mut waters: Vec<ResolvedWay> = Vec::new();
    let mut landuses: Vec<ResolvedWay> = Vec::new();

    for way in &osm_data.ways {
        // Resolve node references to world coordinates
        let mut points = Vec::with_capacity(way.node_refs.len());
        let mut sum_lat = 0.0f64;
        let mut sum_lon = 0.0f64;
        let mut count = 0usize;

        for &node_id in &way.node_refs {
            if let Some(node) = osm_data.nodes.get(&node_id) {
                let (x, z) = conv.to_world_xz(node.lat, node.lon);
                points.push((x, z));
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

        let resolved = ResolvedWay {
            tags: way.tags.clone(),
            points,
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

        let resolved = ResolvedWay {
            tags: rel.tags.clone(),
            points: all_points,
            rep_lat,
            rep_lon,
        };

        if is_water {
            waters.push(resolved);
        } else {
            landuses.push(resolved);
        }
    }

    // 7. Generate meshes in order: terrain, landuse, water, roads, buildings

    // Terrain
    super::terrain::generate_terrain(
        min_lat,
        min_lon,
        max_lat,
        max_lon,
        &conv,
        elevation.as_ref(),
        &mut verts,
        &mut idxs,
    );

    // Landuse
    for lu in &landuses {
        let color = super::color::landuse_color(&lu.tags);
        let y = elev(lu.rep_lat, lu.rep_lon);
        super::landuse::generate_landuse(&lu.points, y, color, &mut verts, &mut idxs);
    }

    // Water
    for w in &waters {
        let y = 0.0; // sea level
        super::water::generate_water(&w.points, y, &mut verts, &mut idxs);
    }

    // Roads
    for r in &roads {
        let y = elev(r.rep_lat, r.rep_lon);
        let width = super::color::road_width(&r.tags);
        let color = super::color::road_color(&r.tags);
        super::road::generate_road(&r.points, y, width, color, &mut verts, &mut idxs);
    }

    // Buildings
    for b in &buildings {
        let color = super::color::building_color(&b.tags);
        let base_y = elev(b.rep_lat, b.rep_lon);
        let height = super::building::parse_building_height(&b.tags);
        // Remove trailing duplicate point if present for the footprint
        let mut footprint = b.points.clone();
        if footprint.len() > 3 && footprint.first() == footprint.last() {
            footprint.pop();
        }
        super::building::generate_building(
            &footprint, base_y, height, color, &mut verts, &mut idxs,
        );
    }

    // 8. Compute center for camera placement
    let (cx, cz) = conv.bbox_centre(min_lat, min_lon, max_lat, max_lon);
    let cy = elev((min_lat + max_lat) / 2.0, (min_lon + max_lon) / 2.0) + 50.0;

    log::info!(
        "Generated world mesh: {} vertices, {} indices, {} buildings, {} roads, {} water areas, {} landuse areas",
        verts.len(),
        idxs.len(),
        buildings.len(),
        roads.len(),
        waters.len(),
        landuses.len(),
    );

    Ok(WorldMesh {
        vertices: verts,
        indices: idxs,
        center: (cx, cy, cz),
    })
}
