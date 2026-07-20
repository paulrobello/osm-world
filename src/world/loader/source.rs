//! Data types and OSM loading orchestrator.

use std::collections::HashMap;
use std::path::Path;

use crate::geo::CoordConverter;
use crate::world::street_sign::ResolvedStreetSign;
use par_osm_rust::elevation::ElevationData;
use par_osm_rust::osm::parse_osm_file;

pub const POINT_FEATURE_BUILDING_CLEARANCE_METRES: f32 = 2.0;

/// Combined mesh for the entire world.
pub struct WorldMesh {
    pub vertices: Vec<crate::render::vertex::Vertex>,
    pub indices: Vec<u32>,
    pub center: (f32, f32, f32),
}

#[derive(Clone, Debug)]
pub struct CpuMesh {
    pub vertices: Vec<crate::render::vertex::Vertex>,
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

#[derive(Clone, Debug)]
pub struct ResolvedPointFeature {
    pub tags: HashMap<String, String>,
    pub point: (f32, f32),
    pub elevation: f32,
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
    pub railways: Vec<ResolvedFeature>,
    pub transit_routes: Vec<ResolvedFeature>,
    pub waters: Vec<ResolvedFeature>,
    pub waterways: Vec<ResolvedFeature>,
    pub landuses: Vec<ResolvedFeature>,
    pub point_features: Vec<ResolvedPointFeature>,
    pub address_points: Vec<ResolvedPointFeature>,
    pub street_signs: Vec<ResolvedStreetSign>,
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
            for coord in
                super::geometry::tiles_for_half_open_bbox(min_x, min_z, max_x, max_z, tile_size)
            {
                index
                    .entry(coord)
                    .or_insert_with(crate::stream::tile::TileFeatureRefs::default);
            }
        }

        macro_rules! index_features {
            ($features:expr, $field:ident) => {
                for (feature_idx, feature) in $features.iter().enumerate() {
                    if let Some(coord) = super::geometry::feature_owner_tile(feature, tile_size) {
                        index
                            .entry(coord)
                            .or_insert_with(crate::stream::tile::TileFeatureRefs::default)
                            .$field
                            .push(feature_idx);
                    }
                }
            };
        }

        index_features!(self.buildings, buildings);
        index_features!(self.roads, roads);
        index_features!(self.railways, railways);
        index_features!(self.waters, waters);
        index_features!(self.waterways, waterways);
        index_features!(self.landuses, landuses);

        for (feature_idx, feature) in self.point_features.iter().enumerate() {
            let coord =
                crate::stream::TileCoord::from_world(feature.point.0, feature.point.1, tile_size);
            index
                .entry(coord)
                .or_insert_with(crate::stream::tile::TileFeatureRefs::default)
                .point_features
                .push(feature_idx);
        }
        for (feature_idx, sign) in self.street_signs.iter().enumerate() {
            let coord = crate::stream::TileCoord::from_world(sign.point.0, sign.point.1, tile_size);
            index
                .entry(coord)
                .or_insert_with(crate::stream::tile::TileFeatureRefs::default)
                .street_signs
                .push(feature_idx);
        }

        index
    }

    pub fn world_bbox(&self) -> Option<(f32, f32, f32, f32)> {
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

pub fn load_world_source(pbf_path: &Path, srtm_dir: Option<&Path>) -> anyhow::Result<WorldSource> {
    load_world_source_with_visual_detail(
        pbf_path,
        srtm_dir,
        &crate::visual_detail::VisualDetailSettings::default(),
    )
}

pub fn load_world_source_with_visual_detail(
    pbf_path: &Path,
    srtm_dir: Option<&Path>,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
) -> anyhow::Result<WorldSource> {
    // 1. Parse OSM input (PBF or XML)
    let osm_data = parse_osm_file(pbf_path)?;

    // 2. Get bounding box
    let (min_lat, min_lon, max_lat, max_lon) = osm_data
        .bounds()
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
    let mut railways: Vec<ResolvedFeature> = Vec::new();
    let mut transit_routes: Vec<ResolvedFeature> = Vec::new();
    let mut waters: Vec<ResolvedFeature> = Vec::new();
    let mut waterways: Vec<ResolvedFeature> = Vec::new();
    let mut landuses: Vec<ResolvedFeature> = Vec::new();
    let mut point_features: Vec<ResolvedPointFeature> = Vec::new();
    let mut address_points: Vec<ResolvedPointFeature> = Vec::new();

    // 5. Resolve ways to world coordinates and classify
    for way in osm_data.ways() {
        // Resolve node references to world coordinates
        let mut points = Vec::with_capacity(way.node_refs.len());
        let mut elevations = Vec::with_capacity(way.node_refs.len());
        let mut sum_lat = 0.0f64;
        let mut sum_lon = 0.0f64;
        let mut count = 0usize;

        for &node_id in &way.node_refs {
            if let Some(node) = osm_data.nodes().get(&node_id) {
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
        let is_railway = crate::world::railway::is_renderable_railway(tags);
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

        if crate::world::point_feature::point_feature_style(tags).is_some() {
            let base_elevation = elev(rep_lat, rep_lon);
            let mut point = conv.to_world_xz(rep_lat, rep_lon);
            if is_building {
                point = super::geometry::move_point_outside_polygon(point, &resolved.points)
                    .unwrap_or(point);
            }
            point_features.push(ResolvedPointFeature {
                tags: tags.clone(),
                point,
                elevation: base_elevation,
                rep_lat,
                rep_lon,
            });
        }
        if is_closed {
            super::vegetation::append_tree_area_point_features_with_settings(
                &resolved,
                &conv,
                &elev,
                visual_detail,
                &mut point_features,
            );
        }

        // Classify into a mutually-exclusive polygon category first (building,
        // water, landuse, waterway), then into independent linear categories
        // (road, railway). At most one clone is needed: when both a polygon
        // category and an independent category match. When only the independent
        // categories match, resolved is moved into the last one.
        let matched_polygon = is_building
            || is_water
            || is_landuse
            || (crate::world::water::is_renderable_waterway(tags) && resolved.points.len() >= 2);

        if is_building {
            buildings.push(resolved.clone());
        } else if is_water {
            waters.push(resolved.clone());
        } else if is_landuse {
            landuses.push(resolved.clone());
        } else if crate::world::water::is_renderable_waterway(tags) && resolved.points.len() >= 2 {
            waterways.push(resolved.clone());
        }

        if is_road && is_railway {
            roads.push(resolved.clone());
            railways.push(resolved);
        } else if is_road {
            roads.push(if matched_polygon {
                resolved.clone()
            } else {
                resolved
            });
        } else if is_railway {
            railways.push(if matched_polygon {
                resolved.clone()
            } else {
                resolved
            });
        }
    }

    // Also process relation geometry for transit routes and multipolygon landuse/water features.
    for rel in osm_data.relations() {
        if crate::world::transit::is_transit_route(&rel.tags) {
            for member in &rel.members {
                let Some(&way_idx) = osm_data.ways_by_id().get(&member.way_id) else {
                    continue;
                };
                let way = &osm_data.ways()[way_idx];
                let mut points = Vec::new();
                let mut elevations = Vec::new();
                let mut sum_lat = 0.0f64;
                let mut sum_lon = 0.0f64;
                let mut count = 0usize;
                for &node_id in &way.node_refs {
                    if let Some(node) = osm_data.nodes().get(&node_id) {
                        let (x, z) = conv.to_world_xz(node.lat, node.lon);
                        points.push((x, z));
                        elevations.push(elev(node.lat, node.lon));
                        sum_lat += node.lat;
                        sum_lon += node.lon;
                        count += 1;
                    }
                }
                if points.len() >= 2 && count > 0 {
                    transit_routes.push(ResolvedFeature {
                        tags: rel.tags.clone(),
                        points,
                        elevations,
                        rep_lat: sum_lat / count as f64,
                        rep_lon: sum_lon / count as f64,
                    });
                }
            }
            continue;
        }

        let mut all_points: Vec<(f32, f32)> = Vec::new();
        let mut elevations: Vec<f32> = Vec::new();
        let mut sum_lat = 0.0f64;
        let mut sum_lon = 0.0f64;
        let mut count = 0usize;

        for member in &rel.members {
            if let Some(&way_idx) = osm_data.ways_by_id().get(&member.way_id)
                && member.role == "outer"
            {
                let way = &osm_data.ways()[way_idx];
                for &node_id in &way.node_refs {
                    if let Some(node) = osm_data.nodes().get(&node_id) {
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

        let tags = &rel.tags;
        if all_points.len() < 3 {
            continue;
        }

        let rep_lat = sum_lat / count as f64;
        let rep_lon = sum_lon / count as f64;

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

        if crate::world::point_feature::point_feature_style(tags).is_some() {
            point_features.push(ResolvedPointFeature {
                tags: tags.clone(),
                point: conv.to_world_xz(rep_lat, rep_lon),
                elevation: elev(rep_lat, rep_lon),
                rep_lat,
                rep_lon,
            });
        }
        if !is_water {
            super::vegetation::append_tree_area_point_features_with_settings(
                &resolved,
                &conv,
                &elev,
                visual_detail,
                &mut point_features,
            );
        }

        if is_water {
            waters.push(resolved);
        } else {
            landuses.push(resolved);
        }
    }

    for node in osm_data.tagged_nodes() {
        let raw_point = conv.to_world_xz(node.lat, node.lon);
        let point = super::geometry::move_point_outside_containing_building(raw_point, &buildings);
        if crate::world::point_feature::point_feature_style(&node.tags).is_some() {
            let mut tags = node.tags.clone();
            if !tags.contains_key("name")
                && let Some(name) = super::geometry::containing_building_name(raw_point, &buildings)
            {
                tags.insert("name".to_string(), name.to_string());
            }
            point_features.push(ResolvedPointFeature {
                tags,
                point,
                elevation: elev(node.lat, node.lon),
                rep_lat: node.lat,
                rep_lon: node.lon,
            });
        }
        if crate::world::address::address_label_text(&node.tags).is_some() {
            address_points.push(ResolvedPointFeature {
                tags: node.tags.clone(),
                point,
                elevation: elev(node.lat, node.lon),
                rep_lat: node.lat,
                rep_lon: node.lon,
            });
        }
    }

    // Ensure CCW winding for all polygon-based features (OSM data can be either winding)
    for b in &mut buildings {
        super::geometry::ensure_ccw(&mut b.points, &mut b.elevations);
    }
    for w in &mut waters {
        super::geometry::ensure_ccw(&mut w.points, &mut w.elevations);
    }
    for lu in &mut landuses {
        super::geometry::ensure_ccw(&mut lu.points, &mut lu.elevations);
    }

    let street_signs = crate::world::street_sign::street_signs_for_roads(&roads);

    Ok(WorldSource {
        min_lat,
        min_lon,
        max_lat,
        max_lon,
        conv,
        elevation,
        buildings,
        roads,
        railways,
        transit_routes,
        waters,
        waterways,
        landuses,
        point_features,
        address_points,
        street_signs,
    })
}
