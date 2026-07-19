//! Point-feature tests. Split from `mod.rs` (ARC-012) without modification —
//! every test is preserved verbatim. The tests reach into sibling submodules
//! through `mod.rs`'s re-exports (`use super::*`).

use super::*;
use std::collections::HashMap;

fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn classifies_natural_tree() {
    let style = point_feature_style(&tags(&[("natural", "tree")])).unwrap();
    assert_eq!(style.kind, PointFeatureKind::Tree);
}

#[test]
fn classifies_natural_peak_as_landmark_peak() {
    let style = point_feature_style(&tags(&[("natural", "peak")])).unwrap();
    assert_eq!(style.kind, PointFeatureKind::Landmark);
    assert_eq!(style.landmark_kind, Some(LandmarkKind::Peak));
}

#[test]
fn classifies_specific_landmark_kinds() {
    for (pairs, expected) in [
        (&[("man_made", "tower")][..], LandmarkKind::Tower),
        (&[("man_made", "water_tower")][..], LandmarkKind::WaterTower),
        (&[("man_made", "chimney")][..], LandmarkKind::Chimney),
        (&[("historic", "monument")][..], LandmarkKind::Monument),
        (&[("historic", "memorial")][..], LandmarkKind::Monument),
        (&[("memorial", "statue")][..], LandmarkKind::Monument),
        (&[("natural", "peak")][..], LandmarkKind::Peak),
        (&[("tourism", "viewpoint")][..], LandmarkKind::Viewpoint),
    ] {
        let style = point_feature_style(&tags(pairs)).unwrap();
        assert_eq!(style.kind, PointFeatureKind::Landmark);
        assert_eq!(style.landmark_kind, Some(expected));
    }
}

#[test]
fn classifies_historic_monument_as_landmark() {
    let style = point_feature_style(&tags(&[("historic", "monument")])).unwrap();
    assert_eq!(style.kind, PointFeatureKind::Landmark);
    assert_eq!(style.landmark_kind, Some(LandmarkKind::Monument));
}

#[test]
fn classifies_amenity_restaurant_as_food_poi() {
    let style = point_feature_style(&tags(&[("amenity", "restaurant")])).unwrap();
    assert_eq!(style.kind, PointFeatureKind::Poi);
    assert_eq!(style.poi_category, Some(PoiCategory::Food));
}

#[test]
fn classifies_shop_as_shop_poi() {
    let style = point_feature_style(&tags(&[("shop", "convenience")])).unwrap();
    assert_eq!(style.kind, PointFeatureKind::Poi);
    assert_eq!(style.poi_category, Some(PoiCategory::Shop));
}

#[test]
fn classifies_tourism_hotel_as_tourism_poi() {
    let style = point_feature_style(&tags(&[("tourism", "hotel")])).unwrap();
    assert_eq!(style.kind, PointFeatureKind::Poi);
    assert_eq!(style.poi_category, Some(PoiCategory::Tourism));
}

#[test]
fn classifies_leisure_playground_as_leisure_poi() {
    let style = point_feature_style(&tags(&[("leisure", "playground")])).unwrap();
    assert_eq!(style.kind, PointFeatureKind::Poi);
    assert_eq!(style.poi_category, Some(PoiCategory::Leisure));
}

#[test]
fn ignores_unrendered_tags() {
    assert!(point_feature_style(&tags(&[("amenity", "bench")])).is_none());
}

#[test]
fn showcase_tree_marker_is_larger_and_brighter_than_balanced_tree_marker() {
    let tags = tags(&[("natural", "tree")]);
    let balanced = crate::visual_detail::VisualDetailSettings::from_preset(
        crate::visual_detail::VisualPreset::Balanced,
    );
    let showcase = crate::visual_detail::VisualDetailSettings::from_preset(
        crate::visual_detail::VisualPreset::Showcase,
    );
    let mut balanced_verts = Vec::new();
    let mut balanced_idxs = Vec::new();
    generate_point_feature_with_visual_detail(
        &tags,
        (0.0, 0.0),
        0.0,
        &balanced,
        &mut balanced_verts,
        &mut balanced_idxs,
    );
    let mut showcase_verts = Vec::new();
    let mut showcase_idxs = Vec::new();
    generate_point_feature_with_visual_detail(
        &tags,
        (0.0, 0.0),
        0.0,
        &showcase,
        &mut showcase_verts,
        &mut showcase_idxs,
    );

    let balanced_top = balanced_verts
        .iter()
        .map(|v| v.position[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let showcase_top = showcase_verts
        .iter()
        .map(|v| v.position[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let balanced_green = balanced_verts
        .iter()
        .map(|v| v.color[1])
        .fold(0.0, f32::max);
    let showcase_green = showcase_verts
        .iter()
        .map(|v| v.color[1])
        .fold(0.0, f32::max);

    assert!(showcase_top > balanced_top + 0.5);
    assert!(showcase_green > balanced_green + 0.1);
}

#[test]
fn tree_geometry_contains_brown_and_green_point_feature_vertices() {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();

    generate_point_feature(
        &tags(&[("natural", "tree")]),
        (10.0, -20.0),
        3.0,
        &mut verts,
        &mut idxs,
    );

    assert!(!idxs.is_empty());
    assert!(
        verts
            .iter()
            .all(|v| v.feature_type == crate::render::vertex::feature::POINT_FEATURE)
    );
    assert!(
        verts
            .iter()
            .any(|v| approx_color(v.color, [0.45, 0.24, 0.10]))
    );
    assert!(
        verts
            .iter()
            .any(|v| approx_color(v.color, [0.16, 0.48, 0.18]))
    );
}

#[test]
fn landmark_geometry_is_taller_than_tree_trunk() {
    let mut tree_verts = Vec::new();
    let mut tree_idxs = Vec::new();
    generate_point_feature(
        &tags(&[("natural", "tree")]),
        (0.0, 0.0),
        0.0,
        &mut tree_verts,
        &mut tree_idxs,
    );
    let trunk_top = tree_verts
        .iter()
        .filter(|v| approx_color(v.color, [0.45, 0.24, 0.10]))
        .map(|v| v.position[1])
        .fold(f32::NEG_INFINITY, f32::max);

    let mut landmark_verts = Vec::new();
    let mut landmark_idxs = Vec::new();
    generate_point_feature(
        &tags(&[("historic", "monument")]),
        (0.0, 0.0),
        0.0,
        &mut landmark_verts,
        &mut landmark_idxs,
    );
    let landmark_top = landmark_verts
        .iter()
        .map(|v| v.position[1])
        .fold(f32::NEG_INFINITY, f32::max);

    assert!(!landmark_idxs.is_empty());
    assert!(landmark_top > trunk_top);
}

#[test]
fn landmark_detail_off_suppresses_landmarks_but_not_trees_or_pois() {
    let off = crate::visual_detail::VisualDetailSettings {
        landmark_detail: crate::visual_detail::LandmarkDetail::Off,
        ..Default::default()
    };

    let landmark = feature_signature_with_visual(&[("man_made", "tower")], &off);
    let tree = feature_signature_with_visual(&[("natural", "tree")], &off);
    let poi = feature_signature_with_visual(&[("amenity", "restaurant")], &off);

    assert_eq!(landmark.vertex_count, 0);
    assert_eq!(landmark.index_count, 0);
    assert!(tree.vertex_count > 0);
    assert!(poi.vertex_count > 0);
}

#[test]
fn landmark_detail_simple_uses_generic_landmark_shape_for_specific_landmarks() {
    let simple = crate::visual_detail::VisualDetailSettings {
        landmark_detail: crate::visual_detail::LandmarkDetail::Simple,
        ..Default::default()
    };
    let showcase = crate::visual_detail::VisualDetailSettings {
        landmark_detail: crate::visual_detail::LandmarkDetail::Showcase,
        ..Default::default()
    };

    let simple_tower = feature_signature_with_visual(&[("man_made", "tower")], &simple);
    let showcase_tower = feature_signature_with_visual(&[("man_made", "tower")], &showcase);
    let simple_generic = feature_signature_with_visual(&[("tourism", "attraction")], &simple);

    assert!(simple_tower.vertex_count > 0);
    assert_ne!(simple_tower, showcase_tower);
    assert_eq!(simple_tower, simple_generic);
}

#[test]
fn landmark_kinds_emit_distinct_showcase_silhouettes() {
    let tower = feature_height(&[("man_made", "tower")]);
    let chimney = feature_height(&[("man_made", "chimney")]);
    let monument = feature_height(&[("historic", "monument")]);
    let peak = feature_height(&[("natural", "peak")]);
    let viewpoint = feature_height(&[("tourism", "viewpoint")]);

    assert!(tower.vertex_count > 0);
    assert!(chimney.vertex_count > 0);
    assert!(monument.vertex_count > 0);
    assert!(peak.vertex_count > 0);
    assert!(viewpoint.vertex_count > 0);
    assert!(tower.max_y > peak.max_y);
    assert!(chimney.max_y > monument.max_y);
}

#[test]
fn point_feature_marker_uv_kind_channels_identify_trees_and_landmarks() {
    let mut tree_verts = Vec::new();
    let mut tree_idxs = Vec::new();
    generate_point_feature(
        &tags(&[("natural", "tree")]),
        (0.0, 0.0),
        0.0,
        &mut tree_verts,
        &mut tree_idxs,
    );

    let mut landmark_verts = Vec::new();
    let mut landmark_idxs = Vec::new();
    generate_point_feature(
        &tags(&[("man_made", "tower")]),
        (0.0, 0.0),
        0.0,
        &mut landmark_verts,
        &mut landmark_idxs,
    );

    assert!(tree_verts.iter().all(|v| (v.uv[0] - 1.0).abs() < 1e-4));
    assert!(landmark_verts.iter().all(|v| (v.uv[0] - 2.0).abs() < 1e-4));
}

#[test]
fn point_feature_triangles_face_outward_from_marker_center() {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();
    let point = (2.0, -4.0);

    generate_point_feature(
        &tags(&[("natural", "tree")]),
        point,
        1.0,
        &mut verts,
        &mut idxs,
    );

    for tri in idxs.chunks_exact(3) {
        let a = verts[tri[0] as usize].position;
        let b = verts[tri[1] as usize].position;
        let c = verts[tri[2] as usize].position;
        let normal = glam::Vec3::from_array(triangle_normal(a, b, c));
        let center =
            (glam::Vec3::from_array(a) + glam::Vec3::from_array(b) + glam::Vec3::from_array(c))
                / 3.0;
        let horizontal_from_marker = glam::Vec3::new(center.x - point.0, 0.0, center.z - point.1);
        let normal_horizontal = glam::Vec3::new(normal.x, 0.0, normal.z);
        if horizontal_from_marker.length() > 1e-4 && normal_horizontal.length() > 1e-4 {
            assert!(
                normal.dot(horizontal_from_marker.normalize()) > 0.0,
                "triangle {tri:?} normal {normal:?} points inward from {center:?}"
            );
        }
    }
}

#[test]
fn nature_marker_emits_point_feature_vertices() {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();

    generate_point_feature(
        &tags(&[("natural", "spring")]),
        (2.0, -4.0),
        1.0,
        &mut verts,
        &mut idxs,
    );

    assert!(!idxs.is_empty());
    assert!(
        verts
            .iter()
            .all(|v| v.feature_type == crate::render::vertex::feature::POINT_FEATURE)
    );
}

#[test]
fn poi_marker_emits_post_and_category_cap() {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();

    generate_point_feature(
        &tags(&[("amenity", "restaurant")]),
        (2.0, -4.0),
        1.0,
        &mut verts,
        &mut idxs,
    );

    assert!(!idxs.is_empty());
    assert!(
        verts
            .iter()
            .all(|v| v.feature_type == crate::render::vertex::feature::POINT_FEATURE)
    );
    assert!(
        verts
            .iter()
            .any(|v| approx_color(v.color, [0.18, 0.18, 0.18]))
    );
    assert!(
        verts
            .iter()
            .any(|v| approx_color(v.color, [1.00, 0.30, 0.16]))
    );
    let colored_top = verts
        .iter()
        .filter(|v| approx_color(v.color, [1.00, 0.30, 0.16]))
        .map(|v| v.position[1])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(colored_top >= 5.8);
}

#[test]
fn tree_trunk_is_visible_hexagonal_prism() {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();

    generate_point_feature(
        &tags(&[("natural", "tree")]),
        (2.0, -4.0),
        1.0,
        &mut verts,
        &mut idxs,
    );

    let trunk_verts: Vec<_> = verts
        .iter()
        .filter(|v| approx_color(v.color, [0.45, 0.24, 0.10]))
        .collect();
    assert_eq!(trunk_verts.len(), 24);
    let min_y = trunk_verts
        .iter()
        .map(|v| v.position[1])
        .fold(f32::INFINITY, f32::min);
    let max_y = trunk_verts
        .iter()
        .map(|v| v.position[1])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!((min_y - 1.0).abs() < 0.001);
    assert!((max_y - 4.0).abs() < 0.001);
}

#[test]
fn tree_canopy_is_lightweight_polyhedron() {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();

    generate_point_feature(
        &tags(&[("natural", "tree")]),
        (2.0, -4.0),
        1.0,
        &mut verts,
        &mut idxs,
    );

    let canopy_triangles = idxs
        .chunks_exact(3)
        .filter(|tri| approx_color(verts[tri[0] as usize].color, [0.16, 0.48, 0.18]))
        .count();
    assert_eq!(canopy_triangles, 8);
}

#[test]
fn tree_canopy_avoids_flat_base_faces_that_z_fight() {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();

    generate_point_feature(
        &tags(&[("natural", "tree")]),
        (2.0, -4.0),
        1.0,
        &mut verts,
        &mut idxs,
    );

    let canopy_downward_vertices = verts
        .iter()
        .filter(|v| approx_color(v.color, [0.16, 0.48, 0.18]))
        .filter(|v| v.normal[1] < -0.99)
        .count();
    assert_eq!(canopy_downward_vertices, 0);
}

#[derive(Debug, PartialEq)]
struct FeatureHeight {
    vertex_count: usize,
    index_count: usize,
    max_y: f32,
}

fn feature_height(tag_pairs: &[(&str, &str)]) -> FeatureHeight {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();
    generate_point_feature(&tags(tag_pairs), (0.0, 0.0), 0.0, &mut verts, &mut idxs);

    FeatureHeight {
        vertex_count: verts.len(),
        index_count: idxs.len(),
        max_y: verts
            .iter()
            .map(|v| v.position[1])
            .fold(f32::NEG_INFINITY, f32::max),
    }
}

fn feature_signature_with_visual(
    tag_pairs: &[(&str, &str)],
    visual: &crate::visual_detail::VisualDetailSettings,
) -> FeatureHeight {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();
    generate_point_feature_with_visual_detail(
        &tags(tag_pairs),
        (0.0, 0.0),
        0.0,
        visual,
        &mut verts,
        &mut idxs,
    );

    FeatureHeight {
        vertex_count: verts.len(),
        index_count: idxs.len(),
        max_y: verts
            .iter()
            .map(|v| v.position[1])
            .fold(f32::NEG_INFINITY, f32::max),
    }
}

fn approx_color(actual: [f32; 3], expected: [f32; 3]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, e)| (*a - e).abs() < 1e-4)
}
