use std::collections::HashMap;

use crate::render::vertex::{Vertex, feature};

const TREE_TRUNK_COLOR: [f32; 3] = [0.45, 0.24, 0.10];
const TREE_CANOPY_COLOR: [f32; 3] = [0.16, 0.48, 0.18];
const LANDMARK_COLOR: [f32; 3] = [0.72, 0.64, 0.45];
const NATURE_MARKER_COLOR: [f32; 3] = [0.24, 0.42, 0.58];
const POI_FOOD_COLOR: [f32; 3] = [1.00, 0.30, 0.16];
const POI_SERVICE_COLOR: [f32; 3] = [0.12, 0.50, 1.00];
const POI_SHOP_COLOR: [f32; 3] = [1.00, 0.30, 0.92];
const POI_TOURISM_COLOR: [f32; 3] = [1.00, 0.78, 0.12];
const POI_LEISURE_COLOR: [f32; 3] = [0.20, 0.90, 0.24];
const POI_POST_COLOR: [f32; 3] = [0.18, 0.18, 0.18];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointFeatureKind {
    Tree,
    Landmark,
    Nature,
    Poi,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoiCategory {
    Food,
    Service,
    Shop,
    Tourism,
    Leisure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointFeatureStyle {
    pub kind: PointFeatureKind,
    pub poi_category: Option<PoiCategory>,
}

pub fn point_feature_style(tags: &HashMap<String, String>) -> Option<PointFeatureStyle> {
    if tags.get("natural").map(String::as_str) == Some("tree") {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Tree,
            poi_category: None,
        });
    }
    if matches!(
        tags.get("natural").map(String::as_str),
        Some("peak" | "rock" | "spring")
    ) {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Nature,
            poi_category: None,
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
            poi_category: None,
        });
    }
    if let Some(category) = poi_category(tags) {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Poi,
            poi_category: Some(category),
        });
    }
    None
}

pub fn point_feature_label(tags: &HashMap<String, String>) -> Option<String> {
    if let Some(name) = tags
        .get("name")
        .map(String::as_str)
        .filter(|name| !name.trim().is_empty())
    {
        return Some(name.trim().to_string());
    }
    let style = point_feature_style(tags)?;
    match style.kind {
        PointFeatureKind::Landmark => Some("Landmark".to_string()),
        PointFeatureKind::Poi => Some(match style.poi_category? {
            PoiCategory::Food => "Food".to_string(),
            PoiCategory::Service => "Service".to_string(),
            PoiCategory::Shop => "Shop".to_string(),
            PoiCategory::Tourism => "Tourism".to_string(),
            PoiCategory::Leisure => "Park".to_string(),
        }),
        PointFeatureKind::Tree | PointFeatureKind::Nature => None,
    }
}

fn poi_category(tags: &HashMap<String, String>) -> Option<PoiCategory> {
    if matches!(
        tags.get("amenity").map(String::as_str),
        Some("restaurant" | "cafe" | "bar" | "pub" | "fast_food")
    ) {
        return Some(PoiCategory::Food);
    }
    if matches!(
        tags.get("amenity").map(String::as_str),
        Some("school" | "hospital" | "clinic" | "pharmacy" | "bank" | "fuel" | "parking")
    ) {
        return Some(PoiCategory::Service);
    }
    if tags.contains_key("shop") {
        return Some(PoiCategory::Shop);
    }
    if matches!(
        tags.get("tourism").map(String::as_str),
        Some("hotel" | "museum" | "guest_house")
    ) {
        return Some(PoiCategory::Tourism);
    }
    if matches!(
        tags.get("leisure").map(String::as_str),
        Some("park" | "playground" | "sports_centre" | "pitch")
    ) {
        return Some(PoiCategory::Leisure);
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
        PointFeatureKind::Poi => append_poi_marker(
            point,
            elevation,
            style.poi_category.expect("POI styles carry a category"),
            verts,
            idxs,
        ),
    }
}

fn append_tree(point: (f32, f32), elevation: f32, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    append_hex_trunk(point, elevation, 0.65, 3.0, verts, idxs);
    append_octahedron_canopy(point, elevation + 4.2, 2.0, verts, idxs);
}

fn append_hex_trunk(
    point: (f32, f32),
    base_y: f32,
    radius: f32,
    height: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let bottom = hex_ring(point, base_y, radius);
    let top = hex_ring(point, base_y + height, radius * 0.82);
    for i in 0..bottom.len() {
        let next = (i + 1) % bottom.len();
        let p0 = bottom[i];
        let p1 = bottom[next];
        let p2 = top[next];
        let p3 = top[i];
        let face_center = (glam::Vec3::from_array(p0)
            + glam::Vec3::from_array(p1)
            + glam::Vec3::from_array(p2)
            + glam::Vec3::from_array(p3))
            / 4.0;
        let normal =
            glam::vec3(face_center.x - point.0, 0.0, face_center.z - point.1).normalize_or_zero();
        append_quad(
            QuadFace {
                positions: [p0, p1, p2, p3],
                normal: normal.to_array(),
            },
            TREE_TRUNK_COLOR,
            verts,
            idxs,
        );
    }
}

fn hex_ring(point: (f32, f32), y: f32, radius: f32) -> [[f32; 3]; 6] {
    std::array::from_fn(|i| {
        let angle = std::f32::consts::FRAC_PI_6 + i as f32 * std::f32::consts::TAU / 6.0;
        [
            point.0 + angle.cos() * radius,
            y,
            point.1 + angle.sin() * radius,
        ]
    })
}

fn append_octahedron_canopy(
    point: (f32, f32),
    center_y: f32,
    radius: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let center = glam::vec3(point.0, center_y, point.1);
    let raw = [
        glam::vec3(0.0, 1.0, 0.0),
        glam::vec3(1.0, 0.0, 0.0),
        glam::vec3(0.0, 0.0, 1.0),
        glam::vec3(-1.0, 0.0, 0.0),
        glam::vec3(0.0, 0.0, -1.0),
        glam::vec3(0.0, -1.0, 0.0),
    ];
    let points = raw.map(|p| (center + p * radius).to_array());
    for [a, b, c] in OCTAHEDRON_FACES {
        append_outward_tri(
            center,
            points[a],
            points[b],
            points[c],
            TREE_CANOPY_COLOR,
            verts,
            idxs,
        );
    }
}

const OCTAHEDRON_FACES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [0, 2, 3],
    [0, 3, 4],
    [0, 4, 1],
    [5, 2, 1],
    [5, 3, 2],
    [5, 4, 3],
    [5, 1, 4],
];

fn append_landmark(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_box(
        BoxSpec {
            point,
            base_y: elevation,
            half_extents: (0.72, 0.72),
            height: 4.2,
            color: LANDMARK_COLOR,
        },
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

fn append_poi_marker(
    point: (f32, f32),
    elevation: f32,
    category: PoiCategory,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_box(
        BoxSpec {
            point,
            base_y: elevation,
            half_extents: (0.22, 0.22),
            height: 2.4,
            color: POI_POST_COLOR,
        },
        verts,
        idxs,
    );
    let color = poi_color(category);
    append_box(
        BoxSpec {
            point,
            base_y: elevation + 2.45,
            half_extents: (0.85, 0.85),
            height: 1.25,
            color,
        },
        verts,
        idxs,
    );
    append_pyramid(
        point,
        elevation + 3.75,
        elevation + 4.9,
        1.0,
        color,
        verts,
        idxs,
    );
}

fn poi_color(category: PoiCategory) -> [f32; 3] {
    match category {
        PoiCategory::Food => POI_FOOD_COLOR,
        PoiCategory::Service => POI_SERVICE_COLOR,
        PoiCategory::Shop => POI_SHOP_COLOR,
        PoiCategory::Tourism => POI_TOURISM_COLOR,
        PoiCategory::Leisure => POI_LEISURE_COLOR,
    }
}

struct BoxSpec {
    point: (f32, f32),
    base_y: f32,
    half_extents: (f32, f32),
    height: f32,
    color: [f32; 3],
}

fn append_box(spec: BoxSpec, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    let (x, z) = spec.point;
    let (half_x, half_z) = spec.half_extents;
    let min_x = x - half_x;
    let max_x = x + half_x;
    let min_z = z - half_z;
    let max_z = z + half_z;
    let top_y = spec.base_y + spec.height;

    for face in [
        QuadFace {
            positions: [
                [min_x, spec.base_y, min_z],
                [max_x, spec.base_y, min_z],
                [max_x, top_y, min_z],
                [min_x, top_y, min_z],
            ],
            normal: [0.0, 0.0, -1.0],
        },
        QuadFace {
            positions: [
                [max_x, spec.base_y, max_z],
                [min_x, spec.base_y, max_z],
                [min_x, top_y, max_z],
                [max_x, top_y, max_z],
            ],
            normal: [0.0, 0.0, 1.0],
        },
        QuadFace {
            positions: [
                [min_x, spec.base_y, max_z],
                [min_x, spec.base_y, min_z],
                [min_x, top_y, min_z],
                [min_x, top_y, max_z],
            ],
            normal: [-1.0, 0.0, 0.0],
        },
        QuadFace {
            positions: [
                [max_x, spec.base_y, min_z],
                [max_x, spec.base_y, max_z],
                [max_x, top_y, max_z],
                [max_x, top_y, min_z],
            ],
            normal: [1.0, 0.0, 0.0],
        },
        QuadFace {
            positions: [
                [min_x, top_y, min_z],
                [max_x, top_y, min_z],
                [max_x, top_y, max_z],
                [min_x, top_y, max_z],
            ],
            normal: [0.0, 1.0, 0.0],
        },
        QuadFace {
            positions: [
                [min_x, spec.base_y, max_z],
                [max_x, spec.base_y, max_z],
                [max_x, spec.base_y, min_z],
                [min_x, spec.base_y, min_z],
            ],
            normal: [0.0, -1.0, 0.0],
        },
    ] {
        append_quad(face, spec.color, verts, idxs);
    }
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

    append_tri(p1, p0, apex, color, verts, idxs);
    append_tri(p2, p1, apex, color, verts, idxs);
    append_tri(p3, p2, apex, color, verts, idxs);
    append_tri(p0, p3, apex, color, verts, idxs);
}

struct QuadFace {
    positions: [[f32; 3]; 4],
    normal: [f32; 3],
}

fn append_quad(face: QuadFace, color: [f32; 3], verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    let base = verts.len() as u32;
    for position in face.positions {
        verts.push(vertex(position, face.normal, color));
    }

    let geometric_normal = triangle_normal(face.positions[0], face.positions[1], face.positions[2]);
    if glam::Vec3::from_array(geometric_normal).dot(glam::Vec3::from_array(face.normal)) >= 0.0 {
        idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        idxs.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}

fn append_outward_tri(
    center: glam::Vec3,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let face_center =
        (glam::Vec3::from_array(p0) + glam::Vec3::from_array(p1) + glam::Vec3::from_array(p2))
            / 3.0;
    let normal = glam::Vec3::from_array(triangle_normal(p0, p1, p2));
    if normal.dot(face_center - center) >= 0.0 {
        append_tri(p0, p1, p2, color, verts, idxs);
    } else {
        append_tri(p0, p2, p1, color, verts, idxs);
    }
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
            let horizontal_from_marker =
                glam::Vec3::new(center.x - point.0, 0.0, center.z - point.1);
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

    fn approx_color(actual: [f32; 3], expected: [f32; 3]) -> bool {
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (*a - e).abs() < 1e-4)
    }
}
