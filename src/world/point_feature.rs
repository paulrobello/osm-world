use std::collections::HashMap;

use crate::render::vertex::{Vertex, feature};

const TREE_TRUNK_COLOR: [f32; 3] = [0.45, 0.24, 0.10];
const TREE_CANOPY_COLOR: [f32; 3] = [0.16, 0.48, 0.18];
const LANDMARK_COLOR: [f32; 3] = [0.72, 0.64, 0.45];
const NATURE_MARKER_COLOR: [f32; 3] = [0.24, 0.42, 0.58];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointFeatureKind {
    Tree,
    Landmark,
    Nature,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointFeatureStyle {
    pub kind: PointFeatureKind,
}

pub fn point_feature_style(tags: &HashMap<String, String>) -> Option<PointFeatureStyle> {
    if tags.get("natural").map(String::as_str) == Some("tree") {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Tree,
        });
    }
    if matches!(
        tags.get("natural").map(String::as_str),
        Some("peak" | "rock" | "spring")
    ) {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Nature,
        });
    }
    if matches!(
        tags.get("tourism").map(String::as_str),
        Some("attraction" | "viewpoint" | "artwork")
    ) || tags.contains_key("historic")
        || matches!(
            tags.get("man_made").map(String::as_str),
            Some("tower" | "water_tower" | "chimney")
        )
    {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Landmark,
        });
    }
    None
}

pub fn generate_point_feature(
    tags: &HashMap<String, String>,
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(style) = point_feature_style(tags) else {
        return;
    };
    match style.kind {
        PointFeatureKind::Tree => append_tree(point, elevation, verts, idxs),
        PointFeatureKind::Landmark => append_landmark(point, elevation, verts, idxs),
        PointFeatureKind::Nature => append_nature_marker(point, elevation, verts, idxs),
    }
}

fn append_tree(point: (f32, f32), elevation: f32, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    append_box(
        point,
        elevation,
        0.32,
        0.32,
        1.7,
        TREE_TRUNK_COLOR,
        verts,
        idxs,
    );
    append_pyramid(
        point,
        elevation + 1.15,
        elevation + 3.55,
        1.25,
        TREE_CANOPY_COLOR,
        verts,
        idxs,
    );
}

fn append_landmark(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_box(
        point,
        elevation,
        0.72,
        0.72,
        4.2,
        LANDMARK_COLOR,
        verts,
        idxs,
    );
    append_pyramid(
        point,
        elevation + 4.2,
        elevation + 5.1,
        0.62,
        LANDMARK_COLOR,
        verts,
        idxs,
    );
}

fn append_nature_marker(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_pyramid(
        point,
        elevation + 0.05,
        elevation + 1.35,
        0.85,
        NATURE_MARKER_COLOR,
        verts,
        idxs,
    );
}

fn append_box(
    point: (f32, f32),
    base_y: f32,
    half_x: f32,
    half_z: f32,
    height: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let (x, z) = point;
    let min_x = x - half_x;
    let max_x = x + half_x;
    let min_z = z - half_z;
    let max_z = z + half_z;
    let top_y = base_y + height;

    append_quad(
        [min_x, base_y, min_z],
        [max_x, base_y, min_z],
        [max_x, top_y, min_z],
        [min_x, top_y, min_z],
        [0.0, 0.0, -1.0],
        color,
        verts,
        idxs,
    );
    append_quad(
        [max_x, base_y, max_z],
        [min_x, base_y, max_z],
        [min_x, top_y, max_z],
        [max_x, top_y, max_z],
        [0.0, 0.0, 1.0],
        color,
        verts,
        idxs,
    );
    append_quad(
        [min_x, base_y, max_z],
        [min_x, base_y, min_z],
        [min_x, top_y, min_z],
        [min_x, top_y, max_z],
        [-1.0, 0.0, 0.0],
        color,
        verts,
        idxs,
    );
    append_quad(
        [max_x, base_y, min_z],
        [max_x, base_y, max_z],
        [max_x, top_y, max_z],
        [max_x, top_y, min_z],
        [1.0, 0.0, 0.0],
        color,
        verts,
        idxs,
    );
    append_quad(
        [min_x, top_y, min_z],
        [max_x, top_y, min_z],
        [max_x, top_y, max_z],
        [min_x, top_y, max_z],
        [0.0, 1.0, 0.0],
        color,
        verts,
        idxs,
    );
    append_quad(
        [min_x, base_y, max_z],
        [max_x, base_y, max_z],
        [max_x, base_y, min_z],
        [min_x, base_y, min_z],
        [0.0, -1.0, 0.0],
        color,
        verts,
        idxs,
    );
}

fn append_pyramid(
    point: (f32, f32),
    base_y: f32,
    apex_y: f32,
    half_size: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let (x, z) = point;
    let p0 = [x - half_size, base_y, z - half_size];
    let p1 = [x + half_size, base_y, z - half_size];
    let p2 = [x + half_size, base_y, z + half_size];
    let p3 = [x - half_size, base_y, z + half_size];
    let apex = [x, apex_y, z];

    append_quad(p3, p2, p1, p0, [0.0, -1.0, 0.0], color, verts, idxs);
    append_tri(p0, p1, apex, color, verts, idxs);
    append_tri(p1, p2, apex, color, verts, idxs);
    append_tri(p2, p3, apex, color, verts, idxs);
    append_tri(p3, p0, apex, color, verts, idxs);
}

fn append_quad(
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    p3: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let base = verts.len() as u32;
    verts.push(vertex(p0, normal, color));
    verts.push(vertex(p1, normal, color));
    verts.push(vertex(p2, normal, color));
    verts.push(vertex(p3, normal, color));
    idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn append_tri(
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let normal = triangle_normal(p0, p1, p2);
    let base = verts.len() as u32;
    verts.push(vertex(p0, normal, color));
    verts.push(vertex(p1, normal, color));
    verts.push(vertex(p2, normal, color));
    idxs.extend_from_slice(&[base, base + 1, base + 2]);
}

fn triangle_normal(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3]) -> [f32; 3] {
    let a = glam::Vec3::from_array(p1) - glam::Vec3::from_array(p0);
    let b = glam::Vec3::from_array(p2) - glam::Vec3::from_array(p0);
    a.cross(b).normalize_or_zero().to_array()
}

fn vertex(position: [f32; 3], normal: [f32; 3], color: [f32; 3]) -> Vertex {
    Vertex {
        position,
        normal,
        color,
        feature_type: feature::POINT_FEATURE,
        uv: [0.0, 0.0],
    }
}

#[cfg(test)]
mod tests {
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
    fn classifies_natural_peak_as_nature() {
        let style = point_feature_style(&tags(&[("natural", "peak")])).unwrap();
        assert_eq!(style.kind, PointFeatureKind::Nature);
    }

    #[test]
    fn classifies_historic_monument_as_landmark() {
        let style = point_feature_style(&tags(&[("historic", "monument")])).unwrap();
        assert_eq!(style.kind, PointFeatureKind::Landmark);
    }

    #[test]
    fn ignores_unrendered_tags() {
        assert!(point_feature_style(&tags(&[("amenity", "bench")])).is_none());
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

    fn approx_color(actual: [f32; 3], expected: [f32; 3]) -> bool {
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (*a - e).abs() < 1e-4)
    }
}
