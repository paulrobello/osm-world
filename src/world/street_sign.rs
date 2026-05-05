use std::collections::{HashMap, HashSet};

use crate::render::vertex::{Vertex, feature};

use super::loader::ResolvedFeature;

pub const MAX_SIGNS_PER_ROAD: usize = 6;
const PERIODIC_SIGN_SPACING_METERS: f32 = 260.0;
const MIN_PERIODIC_ROAD_LENGTH_METERS: f32 = 360.0;
const MIN_SAME_NAME_SIGN_SPACING_METERS: f32 = 45.0;
const INTERSECTION_KEY_SCALE: f32 = 10.0;
const SIGN_POST_COLOR: [f32; 3] = [0.62, 0.64, 0.60];
const SIGN_BOARD_COLOR: [f32; 3] = [0.05, 0.42, 0.22];

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedStreetSign {
    pub name: String,
    pub point: (f32, f32),
    pub elevation: f32,
    pub tangent: (f32, f32),
    pub rep_lat: f64,
    pub rep_lon: f64,
}

pub fn street_name_for_road(tags: &HashMap<String, String>) -> Option<String> {
    let name = tags.get("name").map(String::as_str)?.trim();
    if name.is_empty() || !is_drivable_highway(tags.get("highway").map(String::as_str)?) {
        return None;
    }
    Some(name.to_string())
}

fn is_drivable_highway(highway: &str) -> bool {
    matches!(
        highway,
        "motorway"
            | "trunk"
            | "primary"
            | "secondary"
            | "tertiary"
            | "unclassified"
            | "residential"
            | "living_street"
            | "motorway_link"
            | "trunk_link"
            | "primary_link"
            | "secondary_link"
            | "tertiary_link"
    )
}

pub fn street_signs_for_roads(roads: &[ResolvedFeature]) -> Vec<ResolvedStreetSign> {
    let eligible: Vec<(usize, String, &ResolvedFeature)> = roads
        .iter()
        .enumerate()
        .filter_map(|(index, road)| {
            street_name_for_road(&road.tags).map(|name| (index, name, road))
        })
        .collect();

    let mut signs = Vec::new();
    let mut seen = HashSet::new();
    add_intersection_signs(&eligible, &mut signs, &mut seen);
    add_periodic_signs(&eligible, &mut signs, &mut seen);
    signs
}

type PointKey = (i32, i32);

fn point_key(point: (f32, f32)) -> PointKey {
    (
        (point.0 * INTERSECTION_KEY_SCALE).round() as i32,
        (point.1 * INTERSECTION_KEY_SCALE).round() as i32,
    )
}

fn add_intersection_signs(
    roads: &[(usize, String, &ResolvedFeature)],
    signs: &mut Vec<ResolvedStreetSign>,
    seen: &mut HashSet<(usize, PointKey)>,
) {
    let mut point_roads: HashMap<PointKey, Vec<(usize, usize)>> = HashMap::new();
    for (road_index, _name, road) in roads {
        for point_index in 0..road.points.len() {
            point_roads
                .entry(point_key(road.points[point_index]))
                .or_default()
                .push((*road_index, point_index));
        }
    }

    let roads_by_index: HashMap<usize, (&str, &ResolvedFeature)> = roads
        .iter()
        .map(|(road_index, name, road)| (*road_index, (name.as_str(), *road)))
        .collect();
    let mut intersections: Vec<_> = point_roads
        .into_iter()
        .filter(|(_, refs)| refs.len() > 1)
        .collect();
    intersections.sort_by_key(|(key, _)| *key);

    for (_key, mut refs) in intersections {
        refs.sort_unstable();
        let road_count = refs
            .iter()
            .map(|(road_index, _)| *road_index)
            .collect::<HashSet<_>>()
            .len();
        if road_count < 2 {
            continue;
        }
        for (road_index, point_index) in refs {
            let Some(&(name, road)) = roads_by_index.get(&road_index) else {
                continue;
            };
            push_sign_for_point(signs, seen, road_index, name, road, point_index);
        }
    }
}

fn add_periodic_signs(
    roads: &[(usize, String, &ResolvedFeature)],
    signs: &mut Vec<ResolvedStreetSign>,
    seen: &mut HashSet<(usize, PointKey)>,
) {
    for (road_index, name, road) in roads {
        if road.points.len() < 2 || road_length(road) < MIN_PERIODIC_ROAD_LENGTH_METERS {
            continue;
        }
        let mut next_distance = PERIODIC_SIGN_SPACING_METERS;
        let mut placed_for_road = signs_seen_for_road(seen, *road_index);
        for segment_index in 0..road.points.len() - 1 {
            let p0 = road.points[segment_index];
            let p1 = road.points[segment_index + 1];
            let segment_len = distance(p0, p1);
            if segment_len <= 1e-6 {
                continue;
            }
            while next_distance <= segment_len && placed_for_road < MAX_SIGNS_PER_ROAD {
                let t = next_distance / segment_len;
                if push_interpolated_sign(signs, seen, *road_index, name, road, segment_index, t) {
                    placed_for_road += 1;
                }
                next_distance += PERIODIC_SIGN_SPACING_METERS;
            }
            next_distance -= segment_len;
        }
    }
}

fn signs_seen_for_road(seen: &HashSet<(usize, PointKey)>, road_index: usize) -> usize {
    seen.iter()
        .filter(|(seen_road_index, _)| *seen_road_index == road_index)
        .count()
}

fn road_length(road: &ResolvedFeature) -> f32 {
    road.points
        .windows(2)
        .map(|pair| distance(pair[0], pair[1]))
        .sum()
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dz = b.1 - a.1;
    (dx * dx + dz * dz).sqrt()
}

fn push_sign_for_point(
    signs: &mut Vec<ResolvedStreetSign>,
    seen: &mut HashSet<(usize, PointKey)>,
    road_index: usize,
    name: &str,
    road: &ResolvedFeature,
    point_index: usize,
) {
    if signs_seen_for_road(seen, road_index) >= MAX_SIGNS_PER_ROAD {
        return;
    }
    let centerline_point = road.points[point_index];
    if !seen.insert((road_index, point_key(centerline_point))) {
        return;
    }
    let tangent = tangent_at_point(road, point_index);
    let terrain_elevation = road.elevations.get(point_index).copied().unwrap_or(0.0);
    let (point, elevation) = sign_pose(road, centerline_point, terrain_elevation, tangent);
    if has_nearby_sign_with_same_name(signs, name, point) {
        return;
    }
    signs.push(ResolvedStreetSign {
        name: name.to_string(),
        point,
        elevation,
        tangent,
        rep_lat: road.rep_lat,
        rep_lon: road.rep_lon,
    });
}

fn push_interpolated_sign(
    signs: &mut Vec<ResolvedStreetSign>,
    seen: &mut HashSet<(usize, PointKey)>,
    road_index: usize,
    name: &str,
    road: &ResolvedFeature,
    segment_index: usize,
    t: f32,
) -> bool {
    let p0 = road.points[segment_index];
    let p1 = road.points[segment_index + 1];
    let centerline_point = (p0.0 + (p1.0 - p0.0) * t, p0.1 + (p1.1 - p0.1) * t);
    if !seen.insert((road_index, point_key(centerline_point))) {
        return false;
    }
    let e0 = road.elevations.get(segment_index).copied().unwrap_or(0.0);
    let e1 = road
        .elevations
        .get(segment_index + 1)
        .copied()
        .unwrap_or(e0);
    let terrain_elevation = e0 + (e1 - e0) * t;
    let tangent = normalize_2d((p1.0 - p0.0, p1.1 - p0.1));
    let (point, elevation) = sign_pose(road, centerline_point, terrain_elevation, tangent);
    if has_nearby_sign_with_same_name(signs, name, point) {
        return false;
    }
    signs.push(ResolvedStreetSign {
        name: name.to_string(),
        point,
        elevation,
        tangent,
        rep_lat: road.rep_lat,
        rep_lon: road.rep_lon,
    });
    true
}

fn has_nearby_sign_with_same_name(
    signs: &[ResolvedStreetSign],
    name: &str,
    point: (f32, f32),
) -> bool {
    signs.iter().any(|sign| {
        sign.name == name && distance(sign.point, point) < MIN_SAME_NAME_SIGN_SPACING_METERS
    })
}

fn sign_pose(
    road: &ResolvedFeature,
    centerline_point: (f32, f32),
    terrain_elevation: f32,
    tangent: (f32, f32),
) -> ((f32, f32), f32) {
    const SIGN_ROADSIDE_CLEARANCE: f32 = 1.25;
    const SIGN_BASE_SURFACE_CLEARANCE: f32 = 0.12;

    let perpendicular = (-tangent.1, tangent.0);
    let lateral_offset = super::color::road_width(&road.tags) * 0.5 + SIGN_ROADSIDE_CLEARANCE;
    let point = (
        centerline_point.0 + perpendicular.0 * lateral_offset,
        centerline_point.1 + perpendicular.1 * lateral_offset,
    );
    let elevation = terrain_elevation
        + super::road::road_layer_y_offset(&road.tags)
        + super::road::ROAD_Y_OFFSET
        + SIGN_BASE_SURFACE_CLEARANCE;
    (point, elevation)
}

fn tangent_at_point(road: &ResolvedFeature, point_index: usize) -> (f32, f32) {
    if point_index + 1 < road.points.len() {
        let p = road.points[point_index];
        let next = road.points[point_index + 1];
        return normalize_2d((next.0 - p.0, next.1 - p.1));
    }
    if point_index > 0 {
        let prev = road.points[point_index - 1];
        let p = road.points[point_index];
        return normalize_2d((p.0 - prev.0, p.1 - prev.1));
    }
    (1.0, 0.0)
}

fn normalize_2d(v: (f32, f32)) -> (f32, f32) {
    let len = (v.0 * v.0 + v.1 * v.1).sqrt();
    if len <= 1e-6 {
        (1.0, 0.0)
    } else {
        (v.0 / len, v.1 / len)
    }
}

pub fn append_street_sign(sign: &ResolvedStreetSign, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    append_oriented_box(
        OrientedBoxSpec {
            point: sign.point,
            base_y: sign.elevation,
            half_extents: (0.08, 0.08),
            height: 2.4,
            tangent: (1.0, 0.0),
            color: SIGN_POST_COLOR,
        },
        verts,
        idxs,
    );
    append_oriented_box(
        OrientedBoxSpec {
            point: sign.point,
            base_y: sign.elevation + 2.32,
            half_extents: (1.32, 0.09),
            height: 0.48,
            tangent: sign.tangent,
            color: SIGN_BOARD_COLOR,
        },
        verts,
        idxs,
    );
}

struct OrientedBoxSpec {
    point: (f32, f32),
    base_y: f32,
    half_extents: (f32, f32),
    height: f32,
    tangent: (f32, f32),
    color: [f32; 3],
}

fn append_oriented_box(spec: OrientedBoxSpec, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    let t = normalize_2d(spec.tangent);
    let n = (-t.1, t.0);
    let center = glam::vec3(spec.point.0, spec.base_y, spec.point.1);
    let hx = spec.half_extents.0;
    let hz = spec.half_extents.1;
    let corners = [
        center + glam::vec3(-t.0 * hx - n.0 * hz, 0.0, -t.1 * hx - n.1 * hz),
        center + glam::vec3(t.0 * hx - n.0 * hz, 0.0, t.1 * hx - n.1 * hz),
        center + glam::vec3(t.0 * hx + n.0 * hz, 0.0, t.1 * hx + n.1 * hz),
        center + glam::vec3(-t.0 * hx + n.0 * hz, 0.0, -t.1 * hx + n.1 * hz),
    ];
    let top = corners.map(|p| p + glam::vec3(0.0, spec.height, 0.0));
    append_quad(
        [corners[0], top[0], top[1], corners[1]],
        spec.color,
        verts,
        idxs,
    );
    append_quad(
        [corners[1], top[1], top[2], corners[2]],
        spec.color,
        verts,
        idxs,
    );
    append_quad(
        [corners[2], top[2], top[3], corners[3]],
        spec.color,
        verts,
        idxs,
    );
    append_quad(
        [corners[3], top[3], top[0], corners[0]],
        spec.color,
        verts,
        idxs,
    );
    append_quad([top[0], top[3], top[2], top[1]], spec.color, verts, idxs);
    append_quad(
        [corners[0], corners[1], corners[2], corners[3]],
        spec.color,
        verts,
        idxs,
    );
}

fn append_quad(
    positions: [glam::Vec3; 4],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let normal = (positions[1] - positions[0])
        .cross(positions[2] - positions[0])
        .normalize_or_zero()
        .to_array();
    let base = verts.len() as u32;
    for position in positions {
        verts.push(Vertex {
            position: position.to_array(),
            normal,
            color,
            feature_type: feature::STREET_SIGN,
            uv: [0.0, 0.0],
        });
    }
    idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn road(name: &str, highway: &str, points: Vec<(f32, f32)>) -> ResolvedFeature {
        let mut tags = tags(&[("name", name), ("highway", highway)]);
        if name.is_empty() {
            tags.remove("name");
        }
        ResolvedFeature {
            tags,
            elevations: vec![0.0; points.len()],
            points,
            rep_lat: 38.0,
            rep_lon: -121.0,
        }
    }

    #[test]
    fn named_drivable_roads_are_eligible() {
        let drivable_highways = [
            "motorway",
            "trunk",
            "primary",
            "secondary",
            "tertiary",
            "unclassified",
            "residential",
            "living_street",
            "motorway_link",
            "trunk_link",
            "primary_link",
            "secondary_link",
            "tertiary_link",
        ];
        for highway in drivable_highways {
            assert_eq!(
                street_name_for_road(&tags(&[("name", "Main Street"), ("highway", highway)]))
                    .as_deref(),
                Some("Main Street"),
                "expected {highway} to be eligible"
            );
        }
        assert_eq!(
            street_name_for_road(&tags(&[("name", " Broadway "), ("highway", "primary")]))
                .as_deref(),
            Some("Broadway")
        );
    }

    #[test]
    fn unnamed_and_non_drivable_roads_are_skipped() {
        assert!(street_name_for_road(&tags(&[("highway", "residential")])).is_none());
        assert!(
            street_name_for_road(&tags(&[("name", "Oak Trail"), ("highway", "footway")])).is_none()
        );
        assert!(
            street_name_for_road(&tags(&[("name", "Service Road"), ("highway", "service")]))
                .is_none()
        );
        assert!(
            street_name_for_road(&tags(&[("name", "Future Road"), ("highway", "proposed")]))
                .is_none()
        );
        assert!(
            street_name_for_road(&tags(&[("name", "Work Zone"), ("highway", "construction")]))
                .is_none()
        );
    }

    #[test]
    fn long_named_roads_produce_capped_periodic_signs() {
        let roads = vec![road(
            "Main Street",
            "residential",
            vec![(0.0, 0.0), (600.0, 0.0), (1200.0, 0.0)],
        )];
        let signs = street_signs_for_roads(&roads);
        let main_count = signs
            .iter()
            .filter(|sign| sign.name == "Main Street")
            .count();
        assert!(main_count > 1, "expected periodic signs, got {main_count}");
        assert!(
            main_count <= MAX_SIGNS_PER_ROAD,
            "expected per-road cap, got {main_count}"
        );
    }

    #[test]
    fn shared_points_produce_intersection_signs() {
        let roads = vec![
            road("Main Street", "residential", vec![(0.0, 0.0), (100.0, 0.0)]),
            road(
                "Broadway",
                "primary",
                vec![(100.0, -100.0), (100.0, 0.0), (100.0, 100.0)],
            ),
        ];
        let signs = street_signs_for_roads(&roads);
        assert!(signs.iter().any(|sign| sign.name == "Main Street"));
        assert!(signs.iter().any(|sign| sign.name == "Broadway"));
        assert!(
            signs.iter().any(|sign| sign.name == "Broadway"
                && (sign.point.1 - 0.0).abs() < 0.01
                && sign.point.0 < 100.0),
            "Broadway sign should be offset beside the vertical road: {signs:?}"
        );
    }

    #[test]
    fn intersection_signs_are_emitted_in_stable_point_and_road_order() {
        let roads = vec![
            road("Main Street", "residential", vec![(0.0, 0.0), (10.0, 0.0)]),
            road("First Avenue", "primary", vec![(0.0, 0.0), (0.0, 10.0)]),
            road(
                "Oak Street",
                "residential",
                vec![(5.0, 0.0), (5.0, 5.0), (5.0, 10.0)],
            ),
            road(
                "Pine Street",
                "residential",
                vec![(0.0, 5.0), (5.0, 5.0), (10.0, 5.0)],
            ),
            road(
                "Maple Avenue",
                "primary",
                vec![(10.0, 0.0), (10.0, 5.0), (10.0, 10.0)],
            ),
            road(
                "Cedar Road",
                "secondary",
                vec![(0.0, 10.0), (5.0, 10.0), (10.0, 10.0)],
            ),
            road(
                "Elm Street",
                "tertiary",
                vec![(0.0, 0.0), (5.0, 5.0), (10.0, 10.0)],
            ),
            road(
                "Ash Avenue",
                "residential",
                vec![(10.0, 0.0), (5.0, 5.0), (0.0, 10.0)],
            ),
        ];

        let expected_names = vec![
            "Main Street",
            "First Avenue",
            "Elm Street",
            "Cedar Road",
            "Ash Avenue",
            "Oak Street",
            "Pine Street",
            "Maple Avenue",
        ];

        for _ in 0..8 {
            let signs = street_signs_for_roads(&roads);
            let actual_names: Vec<_> = signs.iter().map(|sign| sign.name.as_str()).collect();
            assert_eq!(actual_names, expected_names);
        }
    }

    #[test]
    fn nearby_duplicate_street_names_are_thinned_at_intersections() {
        let roads = vec![
            road("Main Street", "residential", vec![(0.0, 0.0), (20.0, 0.0)]),
            road("Main Street", "residential", vec![(0.0, 0.0), (0.0, 20.0)]),
            road("6th Street", "residential", vec![(-10.0, 0.0), (0.0, 0.0)]),
        ];

        let signs = street_signs_for_roads(&roads);
        let main_count = signs
            .iter()
            .filter(|sign| sign.name == "Main Street")
            .count();
        let sixth_count = signs
            .iter()
            .filter(|sign| sign.name == "6th Street")
            .count();

        assert_eq!(
            main_count, 1,
            "nearby duplicate Main Street signs: {signs:?}"
        );
        assert_eq!(
            sixth_count, 1,
            "different street name should remain visible"
        );
    }

    #[test]
    fn street_signs_include_later_roads_in_large_areas() {
        let road_count = 620;
        let roads: Vec<_> = (0..road_count)
            .map(|index| {
                let x = index as f32 * 20.0;
                road(
                    &format!("Road {index}"),
                    "residential",
                    vec![(x, 0.0), (x, 400.0)],
                )
            })
            .collect();

        let signs = street_signs_for_roads(&roads);

        assert!(
            signs.iter().any(|sign| sign.name == "Road 619"),
            "later roads should not be starved by a global truncation cap"
        );
    }

    #[test]
    fn street_sign_box_face_normals_point_outward() {
        let sign = ResolvedStreetSign {
            name: "Main Street".to_string(),
            point: (10.0, -20.0),
            elevation: 2.0,
            tangent: (1.0, 0.0),
            rep_lat: 38.0,
            rep_lon: -121.0,
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_street_sign(&sign, &mut vertices, &mut indices);

        let component_centers = [
            glam::vec3(sign.point.0, sign.elevation + 1.2, sign.point.1),
            glam::vec3(sign.point.0, sign.elevation + 2.56, sign.point.1),
        ];

        for (component_index, center) in component_centers.into_iter().enumerate() {
            let vertex_start = component_index * 24;
            for face_index in 0..6 {
                let face_start = vertex_start + face_index * 4;
                let face_vertices = &vertices[face_start..face_start + 4];
                let face_center = face_vertices
                    .iter()
                    .map(|vertex| glam::Vec3::from_array(vertex.position))
                    .sum::<glam::Vec3>()
                    / 4.0;
                let normal = glam::Vec3::from_array(face_vertices[0].normal);

                assert!(
                    normal.dot(face_center - center) > 0.0,
                    "component {component_index} face {face_index} normal {normal:?} points inward from center {center:?} to face {face_center:?}"
                );
            }
        }
    }

    #[test]
    fn street_signs_are_offset_from_road_centerline_and_above_road_surface() {
        let terrain_y = 10.0;
        let roads = vec![ResolvedFeature {
            tags: tags(&[("name", "Main Street"), ("highway", "residential")]),
            points: vec![(0.0, 0.0), (400.0, 0.0)],
            elevations: vec![terrain_y, terrain_y],
            rep_lat: 38.0,
            rep_lon: -121.0,
        }];

        let signs = street_signs_for_roads(&roads);
        let sign = signs
            .iter()
            .find(|sign| sign.name == "Main Street")
            .expect("expected a periodic street sign");

        assert!(
            sign.point.1.abs() > super::super::color::road_width(&roads[0].tags) * 0.5,
            "sign should be placed beside the road, got {:?}",
            sign.point
        );
        assert!(
            sign.elevation > terrain_y + super::super::road::ROAD_Y_OFFSET,
            "sign base should sit above the visible road surface, got {}",
            sign.elevation
        );
    }

    #[test]
    fn street_sign_mesh_stays_lightweight_for_large_areas() {
        let sign = ResolvedStreetSign {
            name: "Main Street".to_string(),
            point: (10.0, -20.0),
            elevation: 2.0,
            tangent: (1.0, 0.0),
            rep_lat: 38.0,
            rep_lon: -121.0,
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_street_sign(&sign, &mut vertices, &mut indices);

        assert!(
            vertices.len() <= 48,
            "street sign mesh should stay cheap enough for full-world rendering, got {} vertices",
            vertices.len()
        );
    }

    #[test]
    fn street_sign_mesh_emits_street_sign_feature_vertices() {
        let sign = ResolvedStreetSign {
            name: "Main Street".to_string(),
            point: (10.0, -20.0),
            elevation: 2.0,
            tangent: (1.0, 0.0),
            rep_lat: 38.0,
            rep_lon: -121.0,
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_street_sign(&sign, &mut vertices, &mut indices);
        assert!(!indices.is_empty());
        assert!(
            vertices
                .iter()
                .any(|v| v.feature_type == feature::STREET_SIGN)
        );
    }
}
