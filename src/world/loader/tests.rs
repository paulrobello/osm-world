use super::*;
use std::collections::HashMap;

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
fn load_world_source_classifies_renderable_railways() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("railway.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.001" lon="-121.0"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <tag k="railway" v="rail"/>
  </way>
  <way id="11">
    <nd ref="1"/>
    <nd ref="2"/>
    <tag k="railway" v="platform"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.railways.len(), 1);
    assert_eq!(
        source.railways[0].tags.get("railway").map(String::as_str),
        Some("rail")
    );
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
    <tag k="name" v="Example River"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.waters.len(), 1);
    assert_eq!(source.waterways.len(), 1);
    assert_eq!(
        source.waters[0].tags.get("natural").map(String::as_str),
        Some("water")
    );
    assert_eq!(
        source.waterways[0].tags.get("waterway").map(String::as_str),
        Some("river")
    );
}

#[test]
fn load_world_source_classifies_tagged_point_nodes() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("points.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0">
    <tag k="natural" v="tree"/>
  </node>
  <node id="2" lat="38.001" lon="-121.0"/>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.point_features.len(), 1);
    let point_feature = &source.point_features[0];
    assert_eq!(
        point_feature.tags.get("natural").map(String::as_str),
        Some("tree")
    );
    assert_eq!(point_feature.rep_lat, 38.0);
    assert_eq!(point_feature.rep_lon, -121.0);
}

#[test]
fn load_world_source_classifies_poi_nodes() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("poi.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0">
    <tag k="amenity" v="restaurant"/>
  </node>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.point_features.len(), 1);
    let point_feature = &source.point_features[0];
    assert_eq!(
        point_feature.tags.get("amenity").map(String::as_str),
        Some("restaurant")
    );
    assert_eq!(point_feature.rep_lat, 38.0);
    assert_eq!(point_feature.rep_lon, -121.0);
}

#[test]
fn load_world_source_moves_building_poi_way_markers_outside_footprint() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("poi-way.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.999"/>
  <node id="3" lat="38.001" lon="-120.999"/>
  <node id="4" lat="38.001" lon="-121.0"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <nd ref="4"/>
    <nd ref="1"/>
    <tag k="building" v="yes"/>
    <tag k="shop" v="convenience"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.buildings.len(), 1);
    assert_eq!(source.point_features.len(), 1);
    let point_feature = &source.point_features[0];
    assert_eq!(
        point_feature.tags.get("shop").map(String::as_str),
        Some("convenience")
    );
    assert_eq!(point_feature.elevation, 0.0);
    assert!(!point_in_polygon(
        point_feature.point,
        &source.buildings[0].points
    ));
}

#[test]
fn load_world_source_moves_poi_nodes_outside_containing_building() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("poi-node-inside-building.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.999"/>
  <node id="3" lat="38.001" lon="-120.999"/>
  <node id="4" lat="38.001" lon="-121.0"/>
  <node id="5" lat="38.0005" lon="-120.9995">
    <tag k="amenity" v="restaurant"/>
    <tag k="name" v="Center Cafe"/>
  </node>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <nd ref="4"/>
    <nd ref="1"/>
    <tag k="building" v="yes"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.buildings.len(), 1);
    assert_eq!(source.point_features.len(), 1);
    let point_feature = &source.point_features[0];
    assert_eq!(
        point_feature.tags.get("name").map(String::as_str),
        Some("Center Cafe")
    );
    assert!(!point_in_polygon(
        point_feature.point,
        &source.buildings[0].points
    ));
    let original_point = source.conv.to_world_xz(38.0005, -120.9995);
    assert_ne!(point_feature.point, original_point);
}

#[test]
fn poi_node_inside_named_building_inherits_building_name_when_unnamed() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("unnamed-poi-inside-named-building.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.999"/>
  <node id="3" lat="38.001" lon="-120.999"/>
  <node id="4" lat="38.001" lon="-121.0"/>
  <node id="5" lat="38.0005" lon="-120.9995">
    <tag k="amenity" v="library"/>
  </node>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <nd ref="4"/>
    <nd ref="1"/>
    <tag k="building" v="yes"/>
    <tag k="name" v="Woodland Public Library"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.point_features.len(), 1);
    assert_eq!(
        source.point_features[0]
            .tags
            .get("name")
            .map(String::as_str),
        Some("Woodland Public Library")
    );
    assert_eq!(
        crate::world::point_feature::point_feature_label(&source.point_features[0].tags)
            .as_deref(),
        Some("Woodland Public Library")
    );
}

#[test]
fn load_world_source_synthesizes_trees_for_orchard_areas() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("orchard.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.998"/>
  <node id="3" lat="38.002" lon="-120.998"/>
  <node id="4" lat="38.002" lon="-121.0"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <nd ref="4"/>
    <nd ref="1"/>
    <tag k="landuse" v="orchard"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.landuses.len(), 1);
    assert!(source.point_features.len() > 4);
    assert!(
        source
            .point_features
            .iter()
            .all(|feature| { feature.tags.get("natural").map(String::as_str) == Some("tree") })
    );
}

#[test]
fn load_world_source_synthesizes_sparse_trees_for_grass_areas() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("grass.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.998"/>
  <node id="3" lat="38.002" lon="-120.998"/>
  <node id="4" lat="38.002" lon="-121.0"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <nd ref="4"/>
    <nd ref="1"/>
    <tag k="landuse" v="grass"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.landuses.len(), 1);
    assert!(!source.point_features.is_empty());
    assert!(source.point_features.len() <= 12);
    assert!(
        source
            .point_features
            .iter()
            .all(|feature| { feature.tags.get("natural").map(String::as_str) == Some("tree") })
    );
}

#[test]
fn visual_settings_scale_synthetic_tree_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("orchard-visual.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.998"/>
  <node id="3" lat="38.002" lon="-120.998"/>
  <node id="4" lat="38.002" lon="-121.0"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <nd ref="4"/>
    <nd ref="1"/>
    <tag k="landuse" v="orchard"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let low_density = crate::visual_detail::VisualDetailSettings {
        vegetation_density: 0.25,
        synthetic_tree_cap: 120,
        ..Default::default()
    };
    let low_count = load_world_source_with_visual_detail(&path, None, &low_density)
        .unwrap()
        .point_features
        .len();

    let high_density = crate::visual_detail::VisualDetailSettings {
        vegetation_density: 1.0,
        ..low_density.clone()
    };
    let high_count = load_world_source_with_visual_detail(&path, None, &high_density)
        .unwrap()
        .point_features
        .len();

    let capped = crate::visual_detail::VisualDetailSettings {
        synthetic_tree_cap: 3,
        ..high_density.clone()
    };
    let capped_count = load_world_source_with_visual_detail(&path, None, &capped)
        .unwrap()
        .point_features
        .len();

    assert!(low_count > 0, "low density should still place some trees");
    assert!(
        high_count > low_count,
        "high density count {high_count} should exceed low density count {low_count}"
    );
    assert_eq!(capped_count, 3);
}

#[test]
fn hidden_vegetation_still_generates_synthetic_tree_points() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("orchard-hidden.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.998"/>
  <node id="3" lat="38.002" lon="-120.998"/>
  <node id="4" lat="38.002" lon="-121.0"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <nd ref="4"/>
    <nd ref="1"/>
    <tag k="landuse" v="orchard"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let hidden = crate::visual_detail::VisualDetailSettings {
        vegetation_visible: false,
        ..Default::default()
    };
    let hidden_count = load_world_source_with_visual_detail(&path, None, &hidden)
        .unwrap()
        .point_features
        .len();
    assert!(
        hidden_count > 0,
        "hidden vegetation should still generate tree source points for live visibility toggles"
    );

    let zero_density = crate::visual_detail::VisualDetailSettings {
        vegetation_density: 0.0,
        ..Default::default()
    };
    assert_eq!(
        load_world_source_with_visual_detail(&path, None, &zero_density)
            .unwrap()
            .point_features
            .len(),
        0
    );
}

#[test]
fn capped_synthetic_trees_are_spread_across_large_green_area() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("large-grass.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.990"/>
  <node id="3" lat="38.010" lon="-120.990"/>
  <node id="4" lat="38.010" lon="-121.0"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <nd ref="4"/>
    <nd ref="1"/>
    <tag k="landuse" v="grass"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert_eq!(source.point_features.len(), 12);
    let min_x = source
        .point_features
        .iter()
        .map(|feature| feature.point.0)
        .fold(f32::INFINITY, f32::min);
    let max_x = source
        .point_features
        .iter()
        .map(|feature| feature.point.0)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_z = source
        .point_features
        .iter()
        .map(|feature| feature.point.1)
        .fold(f32::INFINITY, f32::min);
    let max_z = source
        .point_features
        .iter()
        .map(|feature| feature.point.1)
        .fold(f32::NEG_INFINITY, f32::max);

    assert!(max_x - min_x > 500.0, "x span was {}", max_x - min_x);
    assert!(max_z - min_z > 800.0, "z span was {}", max_z - min_z);
}

#[test]
fn point_feature_index_maps_points_to_owner_tiles() {
    let mut source = empty_source();
    source.point_features.push(ResolvedPointFeature {
        tags: HashMap::from([("natural".to_string(), "tree".to_string())]),
        point: (125.0, -75.0),
        elevation: 3.0,
        rep_lat: 1.0,
        rep_lon: 2.0,
    });

    let index = source.feature_index_for_tile_size(100.0);

    assert_eq!(
        index
            .get(&crate::stream::TileCoord { x: 1, z: -1 })
            .unwrap()
            .point_features,
        vec![0]
    );
}

#[test]
fn load_world_source_generates_street_signs_for_named_drivable_roads() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("street_signs.osm");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <bounds minlat="38.0" minlon="-121.0" maxlat="38.01" maxlon="-120.99"/>
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.0" lon="-120.995"/>
  <node id="3" lat="38.0" lon="-120.99"/>
  <way id="10">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <tag k="highway" v="residential"/>
    <tag k="name" v="Main Street"/>
  </way>
</osm>"#,
    )
    .unwrap();

    let source = load_world_source(&path, None).unwrap();

    assert!(!source.street_signs.is_empty());
    assert!(
        source
            .street_signs
            .iter()
            .any(|sign| sign.name == "Main Street")
    );
}

#[test]
fn street_sign_index_maps_signs_to_owner_tiles() {
    let mut source = empty_source();
    source
        .street_signs
        .push(crate::world::street_sign::ResolvedStreetSign {
            name: "Main Street".to_string(),
            point: (125.0, -75.0),
            elevation: 3.0,
            tangent: (1.0, 0.0),
            rep_lat: 1.0,
            rep_lon: 2.0,
        });

    let index = source.feature_index_for_tile_size(100.0);

    assert_eq!(
        index
            .get(&crate::stream::TileCoord { x: 1, z: -1 })
            .unwrap()
            .street_signs,
        vec![0]
    );
}

#[test]
fn world_source_bbox_center_matches_converter() {
    let source = WorldSource {
        min_lat: 1.0,
        min_lon: 2.0,
        max_lat: 1.1,
        max_lon: 2.2,
        conv: crate::geo::CoordConverter::new(1.0, 2.0),
        elevation: None,
        buildings: Vec::new(),
        roads: Vec::new(),
        railways: Vec::new(),
        transit_routes: Vec::new(),
        waters: Vec::new(),
        waterways: Vec::new(),
        landuses: Vec::new(),
        point_features: Vec::new(),
        address_points: Vec::new(),
        street_signs: Vec::new(),
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
        conv: crate::geo::CoordConverter::new(1.0, 2.0),
        elevation: None,
        buildings: Vec::new(),
        roads: Vec::new(),
        railways: Vec::new(),
        transit_routes: Vec::new(),
        waters: Vec::new(),
        waterways: Vec::new(),
        landuses: Vec::new(),
        point_features: Vec::new(),
        address_points: Vec::new(),
        street_signs: Vec::new(),
    }
}

#[test]
fn world_mesh_with_visual_detail_uses_building_style_roof_color() {
    let mut source = empty_source();
    let mut building = feature(
        "building",
        "yes",
        vec![(0.0, 0.0), (8.0, 0.0), (8.0, 8.0), (0.0, 8.0), (0.0, 0.0)],
    );
    building
        .tags
        .insert("roof:material".to_string(), "metal".to_string());
    source.buildings.push(building);
    let visual = crate::visual_detail::VisualDetailSettings {
        facade_variation: 1.0,
        roof_variation: 1.0,
        ..Default::default()
    };

    let mesh = generate_world_mesh_with_visual_detail(&source, &visual);

    assert!(mesh.vertices.iter().any(|vertex| {
        vertex.feature_type == crate::render::vertex::feature::BUILDING
            && vertex.normal == [0.0, 1.0, 0.0]
            && vertex.color == [0.42, 0.43, 0.46]
    }));
}

#[test]
fn world_mesh_with_landmark_detail_off_suppresses_landmark_geometry() {
    let mut source = empty_source();
    source.point_features.push(ResolvedPointFeature {
        tags: HashMap::from([("man_made".to_string(), "tower".to_string())]),
        point: (0.0, 0.0),
        elevation: 0.0,
        rep_lat: 1.0,
        rep_lon: 2.0,
    });
    let visual = crate::visual_detail::VisualDetailSettings {
        landmark_detail: crate::visual_detail::LandmarkDetail::Off,
        ..Default::default()
    };

    let mesh = generate_world_mesh_with_visual_detail(&source, &visual);

    assert!(mesh.vertices.iter().all(|vertex| {
        vertex.feature_type != crate::render::vertex::feature::POINT_FEATURE
    }));
}

#[test]
fn tile_mesh_with_landmark_detail_off_suppresses_landmark_geometry() {
    let mut source = empty_source();
    source.point_features.push(ResolvedPointFeature {
        tags: HashMap::from([("man_made".to_string(), "tower".to_string())]),
        point: (0.0, 0.0),
        elevation: 0.0,
        rep_lat: 1.0,
        rep_lon: 2.0,
    });
    let refs = crate::stream::tile::TileFeatureRefs {
        point_features: vec![0],
        ..Default::default()
    };
    let visual = crate::visual_detail::VisualDetailSettings {
        landmark_detail: crate::visual_detail::LandmarkDetail::Off,
        ..Default::default()
    };

    let mesh = generate_tile_mesh_set_with_visual_detail(
        &source,
        crate::stream::TileCoord { x: 0, z: 0 },
        &refs,
        100.0,
        &visual,
    );

    let near_vertices = &mesh.lods[crate::stream::TileLod::Near as usize].vertices;
    assert!(near_vertices.iter().all(|vertex| {
        vertex.feature_type != crate::render::vertex::feature::POINT_FEATURE
    }));
}

#[test]
fn tile_mesh_with_visual_detail_uses_building_style_roof_color() {
    let mut source = empty_source();
    let mut building = feature(
        "building",
        "yes",
        vec![(0.0, 0.0), (8.0, 0.0), (8.0, 8.0), (0.0, 8.0), (0.0, 0.0)],
    );
    building
        .tags
        .insert("roof:material".to_string(), "metal".to_string());
    source.buildings.push(building);
    let refs = crate::stream::tile::TileFeatureRefs {
        buildings: vec![0],
        ..Default::default()
    };
    let visual = crate::visual_detail::VisualDetailSettings {
        facade_variation: 1.0,
        roof_variation: 1.0,
        ..Default::default()
    };

    let mesh = generate_tile_mesh_set_with_visual_detail(
        &source,
        crate::stream::TileCoord { x: 0, z: 0 },
        &refs,
        100.0,
        &visual,
    );

    let near_vertices = &mesh.lods[crate::stream::TileLod::Near as usize].vertices;
    assert!(near_vertices.iter().any(|vertex| {
        vertex.feature_type == crate::render::vertex::feature::BUILDING
            && vertex.normal == [0.0, 1.0, 0.0]
            && vertex.color == [0.42, 0.43, 0.46]
    }));
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
    source.railways.push(feature(
        "railway",
        "rail",
        vec![(210.0, -10.0), (220.0, -10.0)],
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
            .get(&crate::stream::TileCoord { x: 2, z: -1 })
            .unwrap()
            .railways,
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
fn tile_mesh_skips_open_waterway_ribbon_segments_already_covered_by_water_area() {
    let mut source = empty_source();
    source.waters.push(feature(
        "natural",
        "water",
        vec![(-20.0, -30.0), (120.0, -30.0), (120.0, 30.0), (-20.0, 30.0)],
    ));
    let mut waterway = feature("waterway", "river", vec![(0.0, 0.0), (100.0, 0.0)]);
    waterway
        .tags
        .insert("name".to_string(), "Example River".to_string());
    source.waterways.push(waterway);
    let refs = crate::stream::tile::TileFeatureRefs {
        waterways: vec![0],
        ..Default::default()
    };

    let tile = mesh::generate_tile_lod_mesh_reexport(
        &source,
        crate::stream::TileCoord { x: 0, z: -1 },
        &refs,
        100.0,
        crate::stream::TileLod::Near,
        &crate::visual_detail::VisualDetailSettings::default(),
    );

    assert!(
        tile.vertices
            .iter()
            .all(|vertex| vertex.feature_type != crate::render::vertex::feature::WATER),
        "waterway centerline should not overdraw an existing water polygon"
    );
}

#[test]
fn tile_mesh_emits_open_waterway_ribbon_geometry() {
    let mut source = empty_source();
    let mut waterway = feature("waterway", "river", vec![(10.0, -10.0), (90.0, -10.0)]);
    waterway.tags.insert("width".to_string(), "20".to_string());
    waterway
        .tags
        .insert("name".to_string(), "Example River".to_string());
    source.waterways.push(waterway);
    let refs = crate::stream::tile::TileFeatureRefs {
        waterways: vec![0],
        ..Default::default()
    };

    let tile = mesh::generate_tile_lod_mesh_reexport(
        &source,
        crate::stream::TileCoord { x: 0, z: -1 },
        &refs,
        100.0,
        crate::stream::TileLod::Near,
        &crate::visual_detail::VisualDetailSettings::default(),
    );

    assert!(tile.vertices.iter().any(|vertex| {
        vertex.feature_type == crate::render::vertex::feature::WATER
            && (vertex.position[2] + 20.0).abs() < 0.1
    }));
}

#[test]
fn streamed_startup_mesh_emits_water_intersecting_selected_tile_even_when_owner_is_unselected()
{
    let mut source = empty_source();
    source.waters.push(feature(
        "natural",
        "water",
        vec![(80.0, -80.0), (220.0, -80.0), (220.0, -20.0), (80.0, -20.0)],
    ));

    let mesh = generate_streamed_startup_mesh(
        &source,
        &[crate::stream::TileCoord { x: 0, z: -1 }],
        100.0,
        &crate::visual_detail::VisualDetailSettings::default(),
    );

    let water_vertices: Vec<_> = mesh
        .vertices
        .iter()
        .filter(|vertex| vertex.feature_type == crate::render::vertex::feature::WATER)
        .collect();
    assert!(!water_vertices.is_empty());
    assert!(water_vertices.iter().all(|vertex| {
        (0.0..=100.0).contains(&vertex.position[0])
            && (-100.0..=0.0).contains(&vertex.position[2])
    }));
}

#[test]
fn streamed_startup_water_clipping_does_not_fill_dry_gap_between_disconnected_intersections() {
    let mut source = empty_source();
    source.waters.push(feature(
        "natural",
        "water",
        vec![
            (0.0, 100.0),
            (20.0, 100.0),
            (20.0, 0.0),
            (80.0, 0.0),
            (80.0, 100.0),
            (100.0, 100.0),
            (100.0, -20.0),
            (0.0, -20.0),
        ],
    ));

    let mesh = generate_streamed_startup_mesh(
        &source,
        &[crate::stream::TileCoord { x: 0, z: 0 }],
        100.0,
        &crate::visual_detail::VisualDetailSettings::default(),
    );

    let water_triangle_centroids: Vec<_> = mesh
        .indices
        .chunks_exact(3)
        .filter_map(|tri| {
            let vertices = [
                mesh.vertices[tri[0] as usize],
                mesh.vertices[tri[1] as usize],
                mesh.vertices[tri[2] as usize],
            ];
            vertices
                .iter()
                .all(|vertex| vertex.feature_type == crate::render::vertex::feature::WATER)
                .then(|| {
                    let x = vertices
                        .iter()
                        .map(|vertex| vertex.position[0])
                        .sum::<f32>()
                        / 3.0;
                    let z = vertices
                        .iter()
                        .map(|vertex| vertex.position[2])
                        .sum::<f32>()
                        / 3.0;
                    (x, z)
                })
        })
        .collect();

    assert!(!water_triangle_centroids.is_empty());
    assert!(
        water_triangle_centroids
            .iter()
            .all(|&(x, z)| !(40.0..=60.0).contains(&x) || !(40.0..=80.0).contains(&z)),
        "water triangles should not cover the dry middle gap; centroids={water_triangle_centroids:?}"
    );
}

#[test]
fn streamed_startup_water_clipping_does_not_emit_water_for_tile_inside_dry_concavity() {
    let mut source = empty_source();
    source.waters.push(feature(
        "natural",
        "water",
        vec![
            (0.0, 100.0),
            (20.0, 100.0),
            (20.0, 0.0),
            (80.0, 0.0),
            (80.0, 100.0),
            (100.0, 100.0),
            (100.0, -20.0),
            (0.0, -20.0),
        ],
    ));

    let mesh = generate_streamed_startup_mesh(
        &source,
        &[crate::stream::TileCoord { x: 2, z: 2 }],
        20.0,
        &crate::visual_detail::VisualDetailSettings::default(),
    );

    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.feature_type != crate::render::vertex::feature::WATER),
        "a tile wholly inside the dry concavity must not receive clipped water"
    );
}

#[test]
fn connected_bridge_road_fragments_stay_elevated_at_shared_split() {
    let mut source = empty_source();
    let tags = HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("bridge".to_string(), "yes".to_string()),
    ]);
    source.roads.push(ResolvedFeature {
        tags: tags.clone(),
        points: vec![(0.0, 0.0), (50.0, 0.0)],
        elevations: vec![0.0, 0.0],
        rep_lat: 1.0,
        rep_lon: 2.0,
    });
    source.roads.push(ResolvedFeature {
        tags: tags.clone(),
        points: vec![(50.0, 0.0), (100.0, 0.0)],
        elevations: vec![0.0, 0.0],
        rep_lat: 1.0,
        rep_lon: 2.0,
    });

    let mesh = generate_world_mesh(&source);
    let bridge_y = crate::world::road::road_layer_y_offset(&tags);
    let min_split_y = mesh
        .vertices
        .iter()
        .filter(|vertex| {
            vertex.feature_type == crate::render::vertex::feature::ROAD_LAYERED
                && (vertex.position[0] - 50.0).abs() < 0.1
        })
        .map(|vertex| vertex.position[1])
        .fold(f32::INFINITY, f32::min);

    assert!(
        min_split_y > bridge_y - 0.1,
        "bridge split dipped to {min_split_y}, expected near {bridge_y}"
    );
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
        && refs.railways.is_empty()
        && refs.waters.is_empty()
        && refs.waterways.is_empty()
        && refs.landuses.is_empty()
        && refs.point_features.is_empty()
        && refs.street_signs.is_empty()));
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
    mesh::append_tile_roads_mesh(
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
            && v.position[1] > crate::world::road::ROAD_Y_OFFSET
    });
    assert!(!has_shared_endpoint_cap_center);
}

#[test]
fn tile_road_mesh_emits_centerline_markings_for_wide_roads() {
    let mut source = empty_source();
    source.roads.push(feature(
        "highway",
        "primary",
        vec![(0.0, -50.0), (40.0, -50.0)],
    ));

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    mesh::append_tile_roads_mesh(
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
            .any(|v| v.feature_type == crate::render::vertex::feature::ROAD_MARKING)
    );
}

#[test]
fn world_mesh_emits_point_feature_geometry() {
    let mut source = empty_source();
    source.point_features.push(ResolvedPointFeature {
        tags: HashMap::from([("historic".to_string(), "monument".to_string())]),
        point: (10.0, -20.0),
        elevation: 2.0,
        rep_lat: 1.0,
        rep_lon: 2.0,
    });

    let mesh = generate_world_mesh(&source);

    assert!(
        mesh.vertices
            .iter()
            .any(|v| v.feature_type == crate::render::vertex::feature::POINT_FEATURE)
    );
}

#[test]
fn tile_mesh_emits_point_feature_geometry() {
    let mut source = empty_source();
    source.point_features.push(ResolvedPointFeature {
        tags: HashMap::from([("natural".to_string(), "tree".to_string())]),
        point: (10.0, -20.0),
        elevation: 2.0,
        rep_lat: 1.0,
        rep_lon: 2.0,
    });
    let refs = crate::stream::tile::TileFeatureRefs {
        point_features: vec![0],
        ..Default::default()
    };

    let mesh = generate_tile_mesh_set(
        &source,
        crate::stream::TileCoord { x: 0, z: -1 },
        &refs,
        100.0,
    );

    let vertices = &mesh.lods[crate::stream::TileLod::Near as usize].vertices;
    assert!(
        vertices
            .iter()
            .any(|v| v.feature_type == crate::render::vertex::feature::POINT_FEATURE)
    );
}

#[test]
fn world_mesh_emits_street_sign_geometry() {
    let mut source = empty_source();
    source
        .street_signs
        .push(crate::world::street_sign::ResolvedStreetSign {
            name: "Main Street".to_string(),
            point: (10.0, -20.0),
            elevation: 2.0,
            tangent: (1.0, 0.0),
            rep_lat: 1.0,
            rep_lon: 2.0,
        });

    let mesh = generate_world_mesh(&source);

    assert!(
        mesh.vertices
            .iter()
            .any(|v| v.feature_type == crate::render::vertex::feature::STREET_SIGN)
    );
}

#[test]
fn tile_mesh_emits_street_sign_geometry() {
    let mut source = empty_source();
    source
        .street_signs
        .push(crate::world::street_sign::ResolvedStreetSign {
            name: "Main Street".to_string(),
            point: (10.0, -20.0),
            elevation: 2.0,
            tangent: (1.0, 0.0),
            rep_lat: 1.0,
            rep_lon: 2.0,
        });
    let refs = crate::stream::tile::TileFeatureRefs {
        street_signs: vec![0],
        ..Default::default()
    };

    let mesh = generate_tile_mesh_set(
        &source,
        crate::stream::TileCoord { x: 0, z: -1 },
        &refs,
        100.0,
    );

    let vertices = &mesh.lods[crate::stream::TileLod::Near as usize].vertices;
    assert!(
        vertices
            .iter()
            .any(|v| v.feature_type == crate::render::vertex::feature::STREET_SIGN)
    );
}

#[test]
fn tile_mesh_emits_poi_point_feature_geometry() {
    let mut source = empty_source();
    source.point_features.push(ResolvedPointFeature {
        tags: HashMap::from([("shop".to_string(), "convenience".to_string())]),
        point: (10.0, -20.0),
        elevation: 2.0,
        rep_lat: 1.0,
        rep_lon: 2.0,
    });
    let refs = crate::stream::tile::TileFeatureRefs {
        point_features: vec![0],
        ..Default::default()
    };

    let mesh = generate_tile_mesh_set(
        &source,
        crate::stream::TileCoord { x: 0, z: -1 },
        &refs,
        100.0,
    );

    let vertices = &mesh.lods[crate::stream::TileLod::Near as usize].vertices;
    assert!(
        vertices
            .iter()
            .any(|v| v.feature_type == crate::render::vertex::feature::POINT_FEATURE)
    );
}

#[test]
fn tile_mesh_emits_train_track_geometry_for_railways() {
    let mut source = empty_source();
    source.railways.push(feature(
        "railway",
        "rail",
        vec![(0.0, -50.0), (40.0, -50.0)],
    ));
    let refs = crate::stream::tile::TileFeatureRefs {
        railways: vec![0],
        ..Default::default()
    };

    let mesh = generate_tile_mesh_set(
        &source,
        crate::stream::TileCoord { x: 0, z: -1 },
        &refs,
        100.0,
    );

    let vertices = &mesh.lods[crate::stream::TileLod::Near as usize].vertices;
    assert!(
        vertices
            .iter()
            .any(|v| v.feature_type == crate::render::vertex::feature::RAILWAY)
    );
}

#[test]
fn tile_road_mesh_emits_bridge_structure_geometry() {
    let mut source = empty_source();
    let mut bridge = feature("highway", "primary", vec![(0.0, -50.0), (100.0, -50.0)]);
    bridge.tags.insert("bridge".to_string(), "yes".to_string());
    bridge.elevations = vec![0.0, 0.0];
    source.roads.push(bridge);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    mesh::append_tile_roads_mesh(
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
    mesh::append_tile_roads_mesh(
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

    assert!(road_min_y < crate::world::road::ROAD_Y_OFFSET);
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
            .any(|v| v.feature_type == crate::render::vertex::feature::LANDUSE_OVERLAY)
    );
    assert!(
        near.vertices
            .iter()
            .any(|v| v.feature_type == crate::render::vertex::feature::ROAD_PATH)
    );
    assert!(
        !far.vertices
            .iter()
            .any(|v| v.feature_type == crate::render::vertex::feature::LANDUSE_OVERLAY)
    );
    assert!(
        !far.vertices
            .iter()
            .any(|v| v.feature_type == crate::render::vertex::feature::ROAD_PATH)
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
            5.0 + crate::world::water::WATER_Y_OFFSET,
            6.0 + crate::world::water::WATER_Y_OFFSET,
            7.0 + crate::world::water::WATER_Y_OFFSET,
            8.0 + crate::world::water::WATER_Y_OFFSET,
        ]
    );
}

#[test]
fn far_tile_lod_simplifies_complex_building_footprints() {
    let mut source = empty_source();
    source.buildings.push(feature(
        "building",
        "yes",
        vec![
            (0.0, 0.0),
            (30.0, 0.0),
            (30.0, 8.0),
            (12.0, 8.0),
            (12.0, 20.0),
            (30.0, 20.0),
            (30.0, 30.0),
            (0.0, 30.0),
        ],
    ));
    let refs = crate::stream::tile::TileFeatureRefs {
        buildings: vec![0],
        ..Default::default()
    };

    let meshes = generate_tile_mesh_set(
        &source,
        crate::stream::TileCoord { x: 0, z: 0 },
        &refs,
        100.0,
    );
    let near_building_vertices = meshes.lods[crate::stream::TileLod::Near as usize]
        .vertices
        .iter()
        .filter(|v| v.feature_type == crate::render::vertex::feature::BUILDING)
        .count();
    let far_building_vertices = meshes.lods[crate::stream::TileLod::Far as usize]
        .vertices
        .iter()
        .filter(|v| v.feature_type == crate::render::vertex::feature::BUILDING)
        .count();

    assert_eq!(near_building_vertices, 40);
    assert_eq!(far_building_vertices, 20);
}
