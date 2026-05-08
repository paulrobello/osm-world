//! Road ribbon strip mesh generator.

use std::collections::HashMap;

use crate::render::vertex::{Vertex, feature};

// Keep road/path overlays at curb-height scale; the city shader adds a tiny
// feature-specific depth bias so these close layers do not z-fight.
pub const ROAD_Y_OFFSET: f32 = 0.04;
const ROAD_CAP_EXTRA_Y_OFFSET: f32 = 0.008;
const ROAD_BRIDGE_LAYER_Y_OFFSET: f32 = 5.0;
const ROAD_TUNNEL_LAYER_Y_OFFSET: f32 = -5.0;
const ROAD_CAP_SEGMENTS: usize = 12;
pub const ROAD_CAP_RADIUS_SCALE: f32 = 1.05;
const BRIDGE_BEAM_THICKNESS: f32 = 0.6;
const BRIDGE_BEAM_TOP_CLEARANCE: f32 = 0.85;
const BRIDGE_BEAM_WIDTH: f32 = 0.45;
const BRIDGE_RAIL_BASE_CLEARANCE: f32 = 0.25;
const BRIDGE_RAIL_HEIGHT: f32 = 0.9;
const BRIDGE_RAIL_WIDTH: f32 = 0.25;
const BRIDGE_SUPPORT_WIDTH: f32 = 0.8;
const BRIDGE_ABUTMENT_THICKNESS: f32 = 0.7;
const BRIDGE_ABUTMENT_SIDE_OVERHANG: f32 = 0.7;
pub const BRIDGE_APPROACH_RAMP_LENGTH: f32 = 25.0;
const TUNNEL_PORTAL_DEPTH: f32 = 1.0;
const TUNNEL_PORTAL_THICKNESS: f32 = 0.5;
const TUNNEL_CLEARANCE: f32 = 3.0;
const TUNNEL_LINING_HEIGHT_FRACTION: f32 = 0.35;
const CENTERLINE_MIN_ROAD_WIDTH: f32 = 4.0;
const CENTERLINE_WIDTH: f32 = 0.22;
const CENTERLINE_DASH_LENGTH: f32 = 4.0;
const CENTERLINE_GAP_LENGTH: f32 = 6.0;
const CENTERLINE_Y_OFFSET: f32 = 0.008;
const CENTERLINE_COLOR: [f32; 3] = [1.0, 0.82, 0.05];
const BRIDGE_STRUCTURE_COLOR: [f32; 3] = [0.50, 0.52, 0.54];
const TUNNEL_STRUCTURE_COLOR: [f32; 3] = [0.34, 0.32, 0.30];

/// Additional per-feature Y offset applied before road ribbon generation.
///
/// The ribbon generator already adds [`ROAD_Y_OFFSET`] above sampled terrain.
/// This offset keeps road/path overlays just above landuse and water overlays
/// without visibly floating at eye level; layered crossings still separate by
/// several metres.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoadProfileKind {
    Surface,
    Bridge,
    Tunnel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadProfile {
    pub kind: RoadProfileKind,
    pub layer_offset: f32,
}

pub fn road_profile(tags: &HashMap<String, String>) -> RoadProfile {
    let surface_offset = surface_road_y_offset(tags);

    let osm_layer = tags
        .get("layer")
        .and_then(|layer| layer.parse::<f32>().ok())
        .unwrap_or(0.0);
    let is_bridge = matches!(
        tags.get("bridge").map(String::as_str),
        Some("yes" | "viaduct")
    );
    let is_tunnel = tags.get("tunnel").is_some_and(|value| value != "no");

    if is_tunnel {
        let layer_depth = if osm_layer < 0.0 {
            osm_layer.abs()
        } else {
            1.0
        };
        return RoadProfile {
            kind: RoadProfileKind::Tunnel,
            layer_offset: ROAD_TUNNEL_LAYER_Y_OFFSET * layer_depth,
        };
    }

    // Explicit bridge tags win over layer-only lowering; explicit tunnel tags
    // are handled above and still take precedence when present.
    if is_bridge || osm_layer > 0.0 {
        return RoadProfile {
            kind: RoadProfileKind::Bridge,
            layer_offset: surface_offset + (osm_layer.max(1.0) * ROAD_BRIDGE_LAYER_Y_OFFSET),
        };
    }

    if osm_layer < 0.0 {
        return RoadProfile {
            kind: RoadProfileKind::Tunnel,
            layer_offset: ROAD_TUNNEL_LAYER_Y_OFFSET * osm_layer.abs(),
        };
    }

    RoadProfile {
        kind: RoadProfileKind::Surface,
        layer_offset: surface_offset,
    }
}

pub fn road_layer_y_offset(tags: &HashMap<String, String>) -> f32 {
    road_profile(tags).layer_offset
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeEndpointRamps {
    pub start: bool,
    pub end: bool,
}

impl Default for BridgeEndpointRamps {
    fn default() -> Self {
        Self {
            start: true,
            end: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RoadRenderPath {
    pub points: Vec<(f32, f32)>,
    pub terrain_elevations: Vec<f32>,
    pub road_elevations: Vec<f32>,
}

pub fn road_render_elevations(
    tags: &HashMap<String, String>,
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
) -> Vec<f32> {
    if points.len() != terrain_elevations.len() {
        return Vec::new();
    }

    let profile = road_profile(tags);
    let ramp_factors = if profile.kind == RoadProfileKind::Bridge {
        bridge_ramp_factors(points, BRIDGE_APPROACH_RAMP_LENGTH)
    } else {
        vec![1.0; points.len()]
    };
    render_elevations_for_factors(tags, terrain_elevations, &ramp_factors)
}

pub fn road_render_path(
    tags: &HashMap<String, String>,
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
) -> RoadRenderPath {
    road_render_path_with_bridge_endpoint_ramps(
        tags,
        points,
        terrain_elevations,
        BridgeEndpointRamps::default(),
    )
}

pub fn road_render_path_with_bridge_endpoint_ramps(
    tags: &HashMap<String, String>,
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    endpoint_ramps: BridgeEndpointRamps,
) -> RoadRenderPath {
    if points.len() != terrain_elevations.len() {
        return RoadRenderPath {
            points: Vec::new(),
            terrain_elevations: Vec::new(),
            road_elevations: Vec::new(),
        };
    }

    if road_profile(tags).kind != RoadProfileKind::Bridge || points.len() < 2 {
        return RoadRenderPath {
            points: points.to_vec(),
            terrain_elevations: terrain_elevations.to_vec(),
            road_elevations: road_render_elevations(tags, points, terrain_elevations),
        };
    }

    let distances = cumulative_distances(points);
    let total_length = *distances.last().unwrap_or(&0.0);
    if total_length <= 1e-6 {
        return RoadRenderPath {
            points: points.to_vec(),
            terrain_elevations: terrain_elevations.to_vec(),
            road_elevations: road_render_elevations(tags, points, terrain_elevations),
        };
    }

    let effective_ramp = BRIDGE_APPROACH_RAMP_LENGTH.min(total_length * 0.5);
    let mut sample_distances = distances.clone();
    if endpoint_ramps.start {
        push_sample_distance(&mut sample_distances, effective_ramp, total_length);
    }
    if endpoint_ramps.end {
        push_sample_distance(
            &mut sample_distances,
            total_length - effective_ramp,
            total_length,
        );
    }
    sample_distances.sort_by(f32::total_cmp);
    sample_distances.dedup_by(|a, b| (*a - *b).abs() < 1e-4);

    let mut render_points = Vec::with_capacity(sample_distances.len());
    let mut render_terrain_elevations = Vec::with_capacity(sample_distances.len());
    let mut ramp_factors = Vec::with_capacity(sample_distances.len());
    for distance in sample_distances {
        let (point, terrain_elevation) =
            interpolate_path_sample(points, terrain_elevations, &distances, distance);
        render_points.push(point);
        render_terrain_elevations.push(terrain_elevation);
        ramp_factors.push(bridge_ramp_factor_with_endpoints(
            distance,
            total_length,
            effective_ramp,
            endpoint_ramps,
        ));
    }

    let road_elevations =
        render_elevations_for_factors(tags, &render_terrain_elevations, &ramp_factors);
    RoadRenderPath {
        points: render_points,
        terrain_elevations: render_terrain_elevations,
        road_elevations,
    }
}

fn render_elevations_for_factors(
    tags: &HashMap<String, String>,
    terrain_elevations: &[f32],
    ramp_factors: &[f32],
) -> Vec<f32> {
    let profile = road_profile(tags);
    if profile.kind != RoadProfileKind::Bridge {
        return terrain_elevations
            .iter()
            .map(|elevation| elevation + profile.layer_offset)
            .collect();
    }

    let surface_offset = surface_road_y_offset(tags);
    let bridge_lift = profile.layer_offset - surface_offset;
    terrain_elevations
        .iter()
        .zip(ramp_factors)
        .map(|(&elevation, &ramp_factor)| elevation + surface_offset + bridge_lift * ramp_factor)
        .collect()
}

fn surface_road_y_offset(tags: &HashMap<String, String>) -> f32 {
    let width = super::color::road_width(tags);
    if width >= 5.0 {
        0.03
    } else if width >= 3.5 {
        0.025
    } else {
        0.02
    }
}

fn bridge_ramp_factors(points: &[(f32, f32)], ramp_length: f32) -> Vec<f32> {
    if points.len() < 2 || ramp_length <= 0.0 {
        return vec![1.0; points.len()];
    }

    let distances = cumulative_distances(points);
    let total_length = *distances.last().unwrap_or(&0.0);
    if total_length <= 1e-6 {
        return vec![1.0; points.len()];
    }
    let effective_ramp = ramp_length.min(total_length * 0.5);
    distances
        .into_iter()
        .map(|distance| bridge_ramp_factor(distance, total_length, effective_ramp))
        .collect()
}

fn bridge_ramp_factor(distance: f32, total_length: f32, effective_ramp: f32) -> f32 {
    bridge_ramp_factor_with_endpoints(
        distance,
        total_length,
        effective_ramp,
        BridgeEndpointRamps::default(),
    )
}

fn bridge_ramp_factor_with_endpoints(
    distance: f32,
    total_length: f32,
    effective_ramp: f32,
    endpoint_ramps: BridgeEndpointRamps,
) -> f32 {
    if effective_ramp <= 1e-6 {
        return 1.0;
    }
    let from_start = if endpoint_ramps.start {
        (distance / effective_ramp).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let from_end = if endpoint_ramps.end {
        ((total_length - distance) / effective_ramp).clamp(0.0, 1.0)
    } else {
        1.0
    };
    from_start.min(from_end)
}

fn cumulative_distances(points: &[(f32, f32)]) -> Vec<f32> {
    let mut distances = Vec::with_capacity(points.len());
    if points.is_empty() {
        return distances;
    }
    distances.push(0.0f32);
    for i in 1..points.len() {
        let dx = points[i].0 - points[i - 1].0;
        let dz = points[i].1 - points[i - 1].1;
        let segment_length = (dx * dx + dz * dz).sqrt();
        distances.push(distances[i - 1] + segment_length);
    }
    distances
}

fn push_sample_distance(sample_distances: &mut Vec<f32>, distance: f32, total_length: f32) {
    if distance > 1e-4 && distance < total_length - 1e-4 {
        sample_distances.push(distance);
    }
}

fn interpolate_path_sample(
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    distances: &[f32],
    sample_distance: f32,
) -> ((f32, f32), f32) {
    if sample_distance <= 0.0 {
        return (points[0], terrain_elevations[0]);
    }
    for i in 1..distances.len() {
        if sample_distance <= distances[i] {
            let segment_length = distances[i] - distances[i - 1];
            if segment_length <= 1e-6 {
                return (points[i], terrain_elevations[i]);
            }
            let t = ((sample_distance - distances[i - 1]) / segment_length).clamp(0.0, 1.0);
            let x = points[i - 1].0 + (points[i].0 - points[i - 1].0) * t;
            let z = points[i - 1].1 + (points[i].1 - points[i - 1].1) * t;
            let elevation =
                terrain_elevations[i - 1] + (terrain_elevations[i] - terrain_elevations[i - 1]) * t;
            return ((x, z), elevation);
        }
    }
    (*points.last().unwrap(), *terrain_elevations.last().unwrap())
}

/// Generate a flat ribbon mesh for a road polyline.
///
/// `points` is a sequence of (x, z) world-space positions.
/// `y` is the base height (typically terrain elevation + small offset).
/// `width` is the road width in world units (metres).
/// `color` is the RGB colour for all vertices.
/// Generated vertices and indices are appended to `verts` and `idxs`.
pub fn generate_road(
    points: &[(f32, f32)],
    y: f32,
    width: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let elevations = vec![y; points.len()];
    generate_road_with_elevations(points, &elevations, width, color, verts, idxs);
}

/// Generate a flat ribbon mesh for a road polyline with per-point terrain elevations.
pub fn generate_road_with_elevations(
    points: &[(f32, f32)],
    elevations: &[f32],
    width: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    generate_road_with_elevations_and_feature_type(
        points,
        elevations,
        width,
        color,
        feature::ROAD,
        verts,
        idxs,
    );
}

pub fn generate_road_with_elevations_and_feature_type(
    points: &[(f32, f32)],
    elevations: &[f32],
    width: f32,
    color: [f32; 3],
    feature_type: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() != elevations.len() || points.len() < 2 {
        return;
    }

    let mut points = points.to_vec();
    let mut elevations = elevations.to_vec();
    let mut i = 1;
    while i < points.len() {
        if same_point(points[i - 1], points[i]) {
            points.remove(i);
            elevations.remove(i);
        } else {
            i += 1;
        }
    }

    let closed = points.len() >= 4 && same_point(points[0], points[points.len() - 1]);
    if closed {
        points.pop();
        elevations.pop();
    }
    if points.len() < 2 || (closed && points.len() < 3) {
        return;
    }

    let half_width = width / 2.0;
    let normal = [0.0, 1.0, 0.0];
    let base = verts.len() as u32;

    let segment_perp = |i: usize| -> Option<(f32, f32)> {
        let j = if i + 1 < points.len() { i + 1 } else { 0 };
        let (x0, z0) = points[i];
        let (x1, z1) = points[j];
        let dx = x1 - x0;
        let dz = z1 - z0;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1e-6 {
            None
        } else {
            Some((-dz / len, dx / len))
        }
    };

    for i in 0..points.len() {
        let prev = if i > 0 {
            segment_perp(i - 1)
        } else if closed {
            segment_perp(points.len() - 1)
        } else {
            None
        };
        let next = if i + 1 < points.len() || closed {
            segment_perp(i)
        } else {
            None
        };

        let (px, pz) = match (prev, next) {
            (Some((ax, az)), Some((bx, bz))) => {
                let mx = ax + bx;
                let mz = az + bz;
                let m_len = (mx * mx + mz * mz).sqrt();
                if m_len < 1e-6 {
                    (bx * half_width, bz * half_width)
                } else {
                    let mx = mx / m_len;
                    let mz = mz / m_len;
                    let dot = (mx * bx + mz * bz).abs().max(0.25);
                    let scale = (half_width / dot).min(half_width * 4.0);
                    (mx * scale, mz * scale)
                }
            }
            (Some((px, pz)), None) | (None, Some((px, pz))) => (px * half_width, pz * half_width),
            (None, None) => (0.0, 0.0),
        };

        let (x, z) = points[i];
        let y = elevations[i] + ROAD_Y_OFFSET;
        verts.push(Vertex {
            position: [x + px, y, z + pz],
            normal,
            color,
            uv: [0.0, 0.0],
            feature_type,
        });
        verts.push(Vertex {
            position: [x - px, y, z - pz],
            normal,
            color,
            uv: [0.0, 0.0],
            feature_type,
        });
    }

    let segment_count = if closed {
        points.len()
    } else {
        points.len() - 1
    };
    for i in 0..segment_count {
        if segment_perp(i).is_none() {
            continue;
        }
        let j = if i + 1 < points.len() { i + 1 } else { 0 };
        let left0 = base + (i * 2) as u32;
        let right0 = left0 + 1;
        let left1 = base + (j * 2) as u32;
        let right1 = left1 + 1;

        // Two upward-facing triangles forming a joined ribbon segment.
        idxs.push(left0);
        idxs.push(left1);
        idxs.push(right0);
        idxs.push(right0);
        idxs.push(left1);
        idxs.push(right1);
    }
}

pub fn append_road_centerline_dashes(
    points: &[(f32, f32)],
    road_elevations: &[f32],
    road_width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_road_centerline_dashes_with_feature_type(
        points,
        road_elevations,
        road_width,
        feature::ROAD_MARKING,
        verts,
        idxs,
    );
}

pub fn append_road_centerline_dashes_with_feature_type(
    points: &[(f32, f32)],
    road_elevations: &[f32],
    road_width: f32,
    feature_type: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if road_width < CENTERLINE_MIN_ROAD_WIDTH
        || points.len() != road_elevations.len()
        || points.len() < 2
    {
        return;
    }

    for i in 0..points.len() - 1 {
        append_centerline_dashes_for_segment(
            points[i],
            points[i + 1],
            road_elevations[i],
            road_elevations[i + 1],
            feature_type,
            verts,
            idxs,
        );
    }
}

fn append_centerline_dashes_for_segment(
    a: (f32, f32),
    b: (f32, f32),
    start_elevation: f32,
    end_elevation: f32,
    feature_type: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(frame) = segment_frame(a, b) else {
        return;
    };
    let dx = b.0 - a.0;
    let dz = b.1 - a.1;
    let segment_length = (dx * dx + dz * dz).sqrt();
    if segment_length <= 1e-6 {
        return;
    }

    let (px, pz) = frame.perpendicular;
    let half_width = CENTERLINE_WIDTH * 0.5;
    let pattern_length = CENTERLINE_DASH_LENGTH + CENTERLINE_GAP_LENGTH;
    let mut dash_start = 0.0;
    while dash_start < segment_length {
        let dash_end = (dash_start + CENTERLINE_DASH_LENGTH).min(segment_length);
        if dash_end > dash_start {
            let start_t = dash_start / segment_length;
            let end_t = dash_end / segment_length;
            let sx = a.0 + dx * start_t;
            let sz = a.1 + dz * start_t;
            let ex = a.0 + dx * end_t;
            let ez = a.1 + dz * end_t;
            let sy = start_elevation
                + (end_elevation - start_elevation) * start_t
                + ROAD_Y_OFFSET
                + CENTERLINE_Y_OFFSET;
            let ey = start_elevation
                + (end_elevation - start_elevation) * end_t
                + ROAD_Y_OFFSET
                + CENTERLINE_Y_OFFSET;
            let base = verts.len() as u32;
            for position in [
                [sx + px * half_width, sy, sz + pz * half_width],
                [ex + px * half_width, ey, ez + pz * half_width],
                [sx - px * half_width, sy, sz - pz * half_width],
                [ex - px * half_width, ey, ez - pz * half_width],
            ] {
                verts.push(Vertex {
                    position,
                    normal: [0.0, 1.0, 0.0],
                    color: CENTERLINE_COLOR,
                    uv: [0.0, 0.0],
                    feature_type,
                });
            }
            idxs.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
        }
        dash_start += pattern_length;
    }
}

pub fn append_road_structures(
    tags: &HashMap<String, String>,
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    match road_profile(tags).kind {
        RoadProfileKind::Bridge => append_bridge_structure(
            points,
            terrain_elevations,
            road_elevations,
            width,
            verts,
            idxs,
        ),
        RoadProfileKind::Tunnel => {
            append_tunnel_structure(points, road_elevations, width, verts, idxs)
        }
        RoadProfileKind::Surface => {}
    }
}

fn append_box(
    mut min: [f32; 3],
    mut max: [f32; 3],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    for axis in 0..3 {
        if (max[axis] - min[axis]).abs() < 1e-4 {
            min[axis] -= 0.05;
            max[axis] += 0.05;
        }
    }

    let mut push_face = |positions: [[f32; 3]; 4], normal: [f32; 3]| {
        let base = verts.len() as u32;
        for position in positions {
            verts.push(Vertex {
                position,
                normal,
                color,
                uv: [0.0, 0.0],
                feature_type: feature::BUILDING,
            });
        }
        idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };

    push_face(
        [
            [min[0], min[1], min[2]],
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], min[1], min[2]],
        ],
        [0.0, 0.0, -1.0],
    );
    push_face(
        [
            [min[0], min[1], max[2]],
            [max[0], min[1], max[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ],
        [0.0, 0.0, 1.0],
    );
    push_face(
        [
            [min[0], min[1], min[2]],
            [min[0], min[1], max[2]],
            [min[0], max[1], max[2]],
            [min[0], max[1], min[2]],
        ],
        [-1.0, 0.0, 0.0],
    );
    push_face(
        [
            [max[0], min[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
            [max[0], min[1], max[2]],
        ],
        [1.0, 0.0, 0.0],
    );
    push_face(
        [
            [min[0], min[1], min[2]],
            [max[0], min[1], min[2]],
            [max[0], min[1], max[2]],
            [min[0], min[1], max[2]],
        ],
        [0.0, -1.0, 0.0],
    );
    push_face(
        [
            [min[0], max[1], min[2]],
            [min[0], max[1], max[2]],
            [max[0], max[1], max[2]],
            [max[0], max[1], min[2]],
        ],
        [0.0, 1.0, 0.0],
    );
}

fn bounds2d(points: &[(f32, f32)]) -> (f32, f32, f32, f32) {
    let mut min_x = points[0].0;
    let mut max_x = points[0].0;
    let mut min_z = points[0].1;
    let mut max_z = points[0].1;
    for &(x, z) in &points[1..] {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    (min_x, max_x, min_z, max_z)
}

type Point2 = (f32, f32);

struct SegmentFrame {
    direction: Point2,
    perpendicular: Point2,
}

struct SegmentStripBox {
    a: Point2,
    b: Point2,
    lateral_offset: f32,
    half_width: f32,
    min_y: f32,
    max_y: f32,
    color: [f32; 3],
}

struct SlopedBridgeRailSegment {
    a: Point2,
    b: Point2,
    rail_offset: f32,
    rail_half_width: f32,
    start_road_y: f32,
    end_road_y: f32,
}

fn segment_frame(a: Point2, b: Point2) -> Option<SegmentFrame> {
    let dx = b.0 - a.0;
    let dz = b.1 - a.1;
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1e-6 {
        None
    } else {
        Some(SegmentFrame {
            direction: (dx / len, dz / len),
            perpendicular: (-dz / len, dx / len),
        })
    }
}

fn append_segment_strip_box(strip: SegmentStripBox, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    let min_y = strip.min_y;
    let max_y = strip.max_y;
    append_sloped_segment_strip_box(strip, min_y, max_y, min_y, max_y, verts, idxs);
}

fn push_prism_face(
    face: [[f32; 3]; 4],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let ux = face[1][0] - face[0][0];
    let uy = face[1][1] - face[0][1];
    let uz = face[1][2] - face[0][2];
    let vx = face[2][0] - face[0][0];
    let vy = face[2][1] - face[0][1];
    let vz = face[2][2] - face[0][2];
    let nx = uy * vz - uz * vy;
    let ny = uz * vx - ux * vz;
    let nz = ux * vy - uy * vx;
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    let normal = if len < 1e-6 {
        [0.0, 1.0, 0.0]
    } else {
        [nx / len, ny / len, nz / len]
    };

    let base = verts.len() as u32;
    for position in face {
        verts.push(Vertex {
            position,
            normal,
            color,
            uv: [0.0, 0.0],
            feature_type: feature::BUILDING,
        });
    }
    idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn append_sloped_segment_strip_box(
    strip: SegmentStripBox,
    start_min_y: f32,
    start_max_y: f32,
    end_min_y: f32,
    end_max_y: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(frame) = segment_frame(strip.a, strip.b) else {
        return;
    };
    let (px, pz) = frame.perpendicular;
    let a_left = (
        strip.a.0 + px * (strip.lateral_offset - strip.half_width),
        strip.a.1 + pz * (strip.lateral_offset - strip.half_width),
    );
    let a_right = (
        strip.a.0 + px * (strip.lateral_offset + strip.half_width),
        strip.a.1 + pz * (strip.lateral_offset + strip.half_width),
    );
    let b_left = (
        strip.b.0 + px * (strip.lateral_offset - strip.half_width),
        strip.b.1 + pz * (strip.lateral_offset - strip.half_width),
    );
    let b_right = (
        strip.b.0 + px * (strip.lateral_offset + strip.half_width),
        strip.b.1 + pz * (strip.lateral_offset + strip.half_width),
    );

    let abl = [a_left.0, start_min_y, a_left.1];
    let atl = [a_left.0, start_max_y, a_left.1];
    let abr = [a_right.0, start_min_y, a_right.1];
    let atr = [a_right.0, start_max_y, a_right.1];
    let bbl = [b_left.0, end_min_y, b_left.1];
    let btl = [b_left.0, end_max_y, b_left.1];
    let bbr = [b_right.0, end_min_y, b_right.1];
    let btr = [b_right.0, end_max_y, b_right.1];

    push_prism_face([abl, bbl, btl, atl], strip.color, verts, idxs);
    push_prism_face([abr, atr, btr, bbr], strip.color, verts, idxs);
    push_prism_face([atl, btl, btr, atr], strip.color, verts, idxs);
    push_prism_face([abl, abr, bbr, bbl], strip.color, verts, idxs);
    push_prism_face([abl, atl, atr, abr], strip.color, verts, idxs);
    push_prism_face([bbl, bbr, btr, btl], strip.color, verts, idxs);
}

fn append_sloped_bridge_rails(
    rail_segment: SlopedBridgeRailSegment,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    for lateral_offset in [rail_segment.rail_offset, -rail_segment.rail_offset] {
        append_sloped_segment_strip_box(
            SegmentStripBox {
                a: rail_segment.a,
                b: rail_segment.b,
                lateral_offset,
                half_width: rail_segment.rail_half_width,
                min_y: 0.0,
                max_y: 0.0,
                color: BRIDGE_STRUCTURE_COLOR,
            },
            rail_segment.start_road_y + BRIDGE_RAIL_BASE_CLEARANCE,
            rail_segment.start_road_y + BRIDGE_RAIL_BASE_CLEARANCE + BRIDGE_RAIL_HEIGHT,
            rail_segment.end_road_y + BRIDGE_RAIL_BASE_CLEARANCE,
            rail_segment.end_road_y + BRIDGE_RAIL_BASE_CLEARANCE + BRIDGE_RAIL_HEIGHT,
            verts,
            idxs,
        );
    }
}

fn append_bridge_structure(
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() != terrain_elevations.len()
        || points.len() != road_elevations.len()
        || points.len() < 2
    {
        return;
    }

    append_bridge_abutments(
        points,
        terrain_elevations,
        road_elevations,
        width,
        verts,
        idxs,
    );

    let half_width = (width * 0.5).max(0.0);
    let rail_half_width = (BRIDGE_RAIL_WIDTH * 0.5).max(0.05);
    let rail_offset = (half_width - rail_half_width).max(rail_half_width);
    for i in 0..points.len() - 1 {
        if segment_frame(points[i], points[i + 1]).is_none() {
            continue;
        }
        let start_road_y = road_elevations[i] + ROAD_Y_OFFSET;
        let end_road_y = road_elevations[i + 1] + ROAD_Y_OFFSET;
        if (start_road_y - end_road_y).abs() > 0.1 {
            append_sloped_bridge_rails(
                SlopedBridgeRailSegment {
                    a: points[i],
                    b: points[i + 1],
                    rail_offset,
                    rail_half_width,
                    start_road_y,
                    end_road_y,
                },
                verts,
                idxs,
            );
            continue;
        }
        let road_y = start_road_y.max(end_road_y);
        let terrain_y = terrain_elevations[i].min(terrain_elevations[i + 1]);

        for lateral_offset in [rail_offset, -rail_offset] {
            append_segment_strip_box(
                SegmentStripBox {
                    a: points[i],
                    b: points[i + 1],
                    lateral_offset,
                    half_width: BRIDGE_BEAM_WIDTH * 0.5,
                    min_y: road_y - BRIDGE_BEAM_TOP_CLEARANCE - BRIDGE_BEAM_THICKNESS,
                    max_y: road_y - BRIDGE_BEAM_TOP_CLEARANCE,
                    color: BRIDGE_STRUCTURE_COLOR,
                },
                verts,
                idxs,
            );
        }
        append_segment_strip_box(
            SegmentStripBox {
                a: points[i],
                b: points[i + 1],
                lateral_offset: rail_offset,
                half_width: rail_half_width,
                min_y: road_y + BRIDGE_RAIL_BASE_CLEARANCE,
                max_y: road_y + BRIDGE_RAIL_BASE_CLEARANCE + BRIDGE_RAIL_HEIGHT,
                color: BRIDGE_STRUCTURE_COLOR,
            },
            verts,
            idxs,
        );
        append_segment_strip_box(
            SegmentStripBox {
                a: points[i],
                b: points[i + 1],
                lateral_offset: -rail_offset,
                half_width: rail_half_width,
                min_y: road_y + BRIDGE_RAIL_BASE_CLEARANCE,
                max_y: road_y + BRIDGE_RAIL_BASE_CLEARANCE + BRIDGE_RAIL_HEIGHT,
                color: BRIDGE_STRUCTURE_COLOR,
            },
            verts,
            idxs,
        );

        if road_y - terrain_y > 2.0 {
            let half_support = BRIDGE_SUPPORT_WIDTH * 0.5;
            let cx = (points[i].0 + points[i + 1].0) * 0.5;
            let cz = (points[i].1 + points[i + 1].1) * 0.5;
            append_box(
                [cx - half_support, terrain_y, cz - half_support],
                [
                    cx + half_support,
                    road_y - BRIDGE_BEAM_TOP_CLEARANCE - BRIDGE_BEAM_THICKNESS,
                    cz + half_support,
                ],
                BRIDGE_STRUCTURE_COLOR,
                verts,
                idxs,
            );
        }
    }
}

fn append_bridge_abutments(
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() < 3 {
        return;
    }

    let first_high = (1..points.len() - 1)
        .find(|&i| bridge_clearance_at(i, terrain_elevations, road_elevations) > 2.0);
    let last_high = (1..points.len() - 1)
        .rev()
        .find(|&i| bridge_clearance_at(i, terrain_elevations, road_elevations) > 2.0);

    if let Some(i) = first_high {
        append_bridge_abutment_at(
            points[i],
            points[i + 1],
            terrain_elevations[i],
            road_elevations[i],
            width,
            verts,
            idxs,
        );
    }
    if let Some(i) = last_high.filter(|&i| Some(i) != first_high) {
        append_bridge_abutment_at(
            points[i],
            points[i - 1],
            terrain_elevations[i],
            road_elevations[i],
            width,
            verts,
            idxs,
        );
    }
}

fn bridge_clearance_at(index: usize, terrain_elevations: &[f32], road_elevations: &[f32]) -> f32 {
    road_elevations[index] + ROAD_Y_OFFSET - terrain_elevations[index]
}

fn append_bridge_abutment_at(
    point: Point2,
    along: Point2,
    terrain_y: f32,
    road_elevation: f32,
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(frame) = segment_frame(point, along) else {
        return;
    };
    let road_y = road_elevation + ROAD_Y_OFFSET;
    let top_y = road_y - BRIDGE_BEAM_TOP_CLEARANCE - BRIDGE_BEAM_THICKNESS;
    if top_y - terrain_y <= 0.5 {
        return;
    }

    let half_span = width * 0.5 + BRIDGE_ABUTMENT_SIDE_OVERHANG;
    let (px, pz) = frame.perpendicular;
    let a = (point.0 - px * half_span, point.1 - pz * half_span);
    let b = (point.0 + px * half_span, point.1 + pz * half_span);
    append_segment_strip_box(
        SegmentStripBox {
            a,
            b,
            lateral_offset: 0.0,
            half_width: BRIDGE_ABUTMENT_THICKNESS * 0.5,
            min_y: terrain_y,
            max_y: top_y,
            color: BRIDGE_STRUCTURE_COLOR,
        },
        verts,
        idxs,
    );
}

fn append_tunnel_structure(
    points: &[(f32, f32)],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() != road_elevations.len() || points.len() < 2 {
        return;
    }

    let closed = points.len() >= 4 && same_point(points[0], points[points.len() - 1]);
    if closed {
        return;
    }

    append_tunnel_portal(points[0], points[1], road_elevations[0], width, verts, idxs);
    append_tunnel_portal(
        points[points.len() - 1],
        points[points.len() - 2],
        road_elevations[road_elevations.len() - 1],
        width,
        verts,
        idxs,
    );

    let lining_half_width = (width * 0.5 + 0.25).max(0.5);
    for i in 0..points.len() - 1 {
        let Some(frame) = segment_frame(points[i], points[i + 1]) else {
            continue;
        };
        let dx = points[i + 1].0 - points[i].0;
        let dz = points[i + 1].1 - points[i].1;
        let segment_length = (dx * dx + dz * dz).sqrt();
        let half_length = (segment_length * 0.25)
            .clamp(0.75, 4.0)
            .min((segment_length * 0.45).max(0.5));
        let mid = (
            (points[i].0 + points[i + 1].0) * 0.5,
            (points[i].1 + points[i + 1].1) * 0.5,
        );
        let (dir_x, dir_z) = frame.direction;
        let start = (mid.0 - dir_x * half_length, mid.1 - dir_z * half_length);
        let end = (mid.0 + dir_x * half_length, mid.1 + dir_z * half_length);
        let road_y = road_elevations[i].max(road_elevations[i + 1]) + ROAD_Y_OFFSET;
        let lining_min_y = road_y + TUNNEL_CLEARANCE * TUNNEL_LINING_HEIGHT_FRACTION;
        let lining_max_y = road_y + TUNNEL_CLEARANCE - 0.2;

        append_segment_strip_box(
            SegmentStripBox {
                a: start,
                b: end,
                lateral_offset: 0.0,
                half_width: lining_half_width,
                min_y: lining_min_y,
                max_y: lining_max_y,
                color: TUNNEL_STRUCTURE_COLOR,
            },
            verts,
            idxs,
        );
    }
}

fn append_tunnel_portal(
    point: (f32, f32),
    next: (f32, f32),
    elevation: f32,
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(frame) = segment_frame(point, next) else {
        return;
    };
    let (dx, dz) = frame.direction;
    let (px, pz) = frame.perpendicular;

    let road_y = elevation + ROAD_Y_OFFSET;
    let top_y = road_y + TUNNEL_CLEARANCE;
    let half_width = width * 0.5;
    let half_post = TUNNEL_PORTAL_THICKNESS * 0.5;
    let depth = TUNNEL_PORTAL_DEPTH.max(TUNNEL_PORTAL_THICKNESS);
    let front_dx = dx * depth;
    let front_dz = dz * depth;

    for sign in [1.0, -1.0] {
        let offset_x = px * (half_width + half_post) * sign;
        let offset_z = pz * (half_width + half_post) * sign;
        let start_x = point.0 + offset_x;
        let start_z = point.1 + offset_z;
        let end_x = start_x + front_dx;
        let end_z = start_z + front_dz;
        append_box(
            [
                start_x.min(end_x) - half_post,
                road_y,
                start_z.min(end_z) - half_post,
            ],
            [
                start_x.max(end_x) + half_post,
                top_y,
                start_z.max(end_z) + half_post,
            ],
            TUNNEL_STRUCTURE_COLOR,
            verts,
            idxs,
        );
    }

    let beam_corners = [
        (
            point.0 + px * (half_width + half_post),
            point.1 + pz * (half_width + half_post),
        ),
        (
            point.0 - px * (half_width + half_post),
            point.1 - pz * (half_width + half_post),
        ),
        (
            point.0 + px * (half_width + half_post) + front_dx,
            point.1 + pz * (half_width + half_post) + front_dz,
        ),
        (
            point.0 - px * (half_width + half_post) + front_dx,
            point.1 - pz * (half_width + half_post) + front_dz,
        ),
    ];
    let (beam_min_x, beam_max_x, beam_min_z, beam_max_z) = bounds2d(&beam_corners);
    append_box(
        [beam_min_x, top_y - TUNNEL_PORTAL_THICKNESS, beam_min_z],
        [beam_max_x, top_y, beam_max_z],
        TUNNEL_STRUCTURE_COLOR,
        verts,
        idxs,
    );
}

pub fn append_road_cap(
    point: (f32, f32),
    elevation: f32,
    width: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_road_cap_with_radius_scale(
        point,
        elevation,
        width,
        ROAD_CAP_RADIUS_SCALE,
        color,
        verts,
        idxs,
    );
}

pub fn append_road_cap_with_radius_scale(
    point: (f32, f32),
    elevation: f32,
    width: f32,
    radius_scale: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_cap(
        point.0,
        elevation + ROAD_Y_OFFSET + ROAD_CAP_EXTRA_Y_OFFSET,
        point.1,
        width / 2.0 * radius_scale,
        color,
        verts,
        idxs,
    );
}

fn append_cap(
    x: f32,
    y: f32,
    z: f32,
    radius: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let normal = [0.0, 1.0, 0.0];
    let base = verts.len() as u32;
    verts.push(Vertex {
        position: [x, y, z],
        normal,
        color,
        uv: [0.0, 0.0],
        feature_type: feature::ROAD,
    });

    for i in 0..ROAD_CAP_SEGMENTS {
        let angle = i as f32 / ROAD_CAP_SEGMENTS as f32 * std::f32::consts::TAU;
        verts.push(Vertex {
            position: [x + angle.cos() * radius, y, z + angle.sin() * radius],
            normal,
            color,
            uv: [0.0, 0.0],
            feature_type: feature::ROAD,
        });
    }

    for i in 0..ROAD_CAP_SEGMENTS {
        let current = base + 1 + i as u32;
        let next = base + 1 + ((i + 1) % ROAD_CAP_SEGMENTS) as u32;
        idxs.push(base);
        idxs.push(next);
        idxs.push(current);
    }
}

fn same_point(a: (f32, f32), b: (f32, f32)) -> bool {
    let dx = a.0 - b.0;
    let dz = a.1 - b.1;
    dx * dx + dz * dz < 1e-8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle_normal_y(a: Vertex, b: Vertex, c: Vertex) -> f32 {
        let ux = b.position[0] - a.position[0];
        let uz = b.position[2] - a.position[2];
        let vx = c.position[0] - a.position[0];
        let vz = c.position[2] - a.position[2];
        uz * vx - ux * vz
    }

    #[test]
    fn road_layer_y_offset_lifts_bridges_above_surface_roads() {
        let surface =
            std::collections::HashMap::from([("highway".to_string(), "primary".to_string())]);
        let bridge = std::collections::HashMap::from([
            ("highway".to_string(), "primary".to_string()),
            ("bridge".to_string(), "yes".to_string()),
            ("layer".to_string(), "1".to_string()),
        ]);

        assert!(road_layer_y_offset(&surface) >= 0.02);
        assert!(road_layer_y_offset(&bridge) >= road_layer_y_offset(&surface) + 4.0);
    }

    #[test]
    fn road_profile_classifies_bridge_tunnel_and_surface_roads() {
        let surface =
            std::collections::HashMap::from([("highway".to_string(), "primary".to_string())]);
        let bridge = std::collections::HashMap::from([
            ("highway".to_string(), "primary".to_string()),
            ("bridge".to_string(), "yes".to_string()),
        ]);
        let tunnel = std::collections::HashMap::from([
            ("highway".to_string(), "primary".to_string()),
            ("tunnel".to_string(), "yes".to_string()),
        ]);

        assert_eq!(road_profile(&surface).kind, RoadProfileKind::Surface);
        assert_eq!(road_profile(&bridge).kind, RoadProfileKind::Bridge);
        assert_eq!(road_profile(&tunnel).kind, RoadProfileKind::Tunnel);
    }

    #[test]
    fn road_layer_y_offset_lowers_tunnels_below_surface_roads() {
        let surface =
            std::collections::HashMap::from([("highway".to_string(), "primary".to_string())]);
        let tunnel = std::collections::HashMap::from([
            ("highway".to_string(), "primary".to_string()),
            ("tunnel".to_string(), "yes".to_string()),
            ("layer".to_string(), "-1".to_string()),
        ]);

        assert!(road_layer_y_offset(&surface) > 0.0);
        assert!(road_layer_y_offset(&tunnel) <= road_layer_y_offset(&surface) - 4.0);
    }

    #[test]
    fn bridge_tag_wins_over_negative_layer_without_tunnel() {
        let tags = std::collections::HashMap::from([
            ("highway".to_string(), "primary".to_string()),
            ("bridge".to_string(), "yes".to_string()),
            ("layer".to_string(), "-1".to_string()),
        ]);

        assert_eq!(road_profile(&tags).kind, RoadProfileKind::Bridge);
        assert!(road_layer_y_offset(&tags) > 0.0);
    }

    #[test]
    fn explicit_tunnel_wins_over_bridge_tags() {
        let tags = std::collections::HashMap::from([
            ("highway".to_string(), "primary".to_string()),
            ("bridge".to_string(), "yes".to_string()),
            ("tunnel".to_string(), "yes".to_string()),
            ("layer".to_string(), "1".to_string()),
        ]);

        assert_eq!(road_profile(&tags).kind, RoadProfileKind::Tunnel);
        assert!(road_layer_y_offset(&tags) < 0.0);
    }

    #[test]
    fn centerline_dashes_can_use_layered_marking_feature_type() {
        let points = [(0.0, 0.0), (20.0, 0.0)];
        let road_elevations = [3.0, 3.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_road_centerline_dashes_with_feature_type(
            &points,
            &road_elevations,
            6.0,
            feature::ROAD_MARKING_LAYERED,
            &mut vertices,
            &mut indices,
        );

        assert!(!indices.is_empty());
        assert!(
            vertices
                .iter()
                .all(|v| v.feature_type == feature::ROAD_MARKING_LAYERED)
        );
    }

    #[test]
    fn centerline_dashes_emit_yellow_markings_above_wide_roads() {
        let points = [(0.0, 0.0), (20.0, 0.0)];
        let road_elevations = [3.0, 3.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_road_centerline_dashes(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
        assert!(
            vertices
                .iter()
                .all(|v| v.feature_type == feature::ROAD_MARKING)
        );
        assert!(vertices.iter().all(|v| v.color == CENTERLINE_COLOR));
        assert!(
            vertices
                .iter()
                .all(|v| v.position[1] > road_elevations[0] + ROAD_Y_OFFSET)
        );
    }

    #[test]
    fn centerline_dashes_skip_narrow_roads() {
        let points = [(0.0, 0.0), (20.0, 0.0)];
        let road_elevations = [3.0, 3.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_road_centerline_dashes(&points, &road_elevations, 2.0, &mut vertices, &mut indices);

        assert!(vertices.is_empty());
        assert!(indices.is_empty());
    }

    #[test]
    fn centerline_dashes_follow_sloped_road_elevations() {
        let points = [(0.0, 0.0), (20.0, 0.0)];
        let road_elevations = [0.0, 4.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_road_centerline_dashes(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

        let min_y = vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::INFINITY, f32::min);
        let max_y = vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_y > min_y + 0.5);
    }

    #[test]
    fn segment_strip_box_stays_close_to_diagonal_segment() {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_segment_strip_box(
            SegmentStripBox {
                a: (0.0, 0.0),
                b: (10.0, 10.0),
                lateral_offset: 0.0,
                half_width: 0.5,
                min_y: 1.0,
                max_y: 2.0,
                color: BRIDGE_STRUCTURE_COLOR,
            },
            &mut vertices,
            &mut indices,
        );

        assert!(!indices.is_empty());
        let max_distance_from_segment = vertices
            .iter()
            .map(|vertex| {
                let x = vertex.position[0];
                let z = vertex.position[2];
                ((z - x).abs()) / 2.0_f32.sqrt()
            })
            .fold(0.0_f32, f32::max);

        assert!(
            max_distance_from_segment <= 0.51,
            "diagonal strip expanded into a broad axis-aligned slab: max distance {max_distance_from_segment}"
        );
    }

    #[test]
    fn bridge_structure_adds_side_beams_rails_and_support_geometry() {
        let points = [(0.0, 0.0), (20.0, 0.0)];
        let terrain_elevations = [0.0, 0.0];
        let road_elevations = [5.7, 5.7];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_bridge_structure(
            &points,
            &terrain_elevations,
            &road_elevations,
            6.0,
            &mut vertices,
            &mut indices,
        );

        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
        assert!(vertices.iter().any(|v| v.feature_type == feature::BUILDING));
        assert!(
            vertices
                .iter()
                .any(|v| v.position[1] < road_elevations[0] + ROAD_Y_OFFSET)
        );
        assert!(
            vertices
                .iter()
                .any(|v| v.position[1] <= terrain_elevations[0] + 0.1)
        );
    }

    #[test]
    fn bridge_structure_adds_rails_but_skips_flat_deck_on_sloped_ramp_segments() {
        let points = [(0.0, 0.0), (25.0, 0.0)];
        let terrain_elevations = [0.0, 0.0];
        let road_elevations = [0.7, 5.7];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_bridge_structure(
            &points,
            &terrain_elevations,
            &road_elevations,
            6.0,
            &mut vertices,
            &mut indices,
        );

        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
        assert!(vertices.iter().any(|v| v.position[1] < 4.0));
        let high_road_y = road_elevations[1] + ROAD_Y_OFFSET;
        assert!(vertices.iter().any(|v| v.position[1] > high_road_y + 0.5));
        assert!(
            vertices
                .iter()
                .all(|v| v.position[1] > terrain_elevations[0])
        );
    }

    #[test]
    fn bridge_structure_adds_abutment_walls_at_approach_transitions() {
        let points = [(0.0, 0.0), (25.0, 0.0), (75.0, 0.0), (100.0, 0.0)];
        let terrain_elevations = [0.0, 0.0, 0.0, 0.0];
        let road_elevations = [0.7, 5.7, 5.7, 0.7];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_bridge_structure(
            &points,
            &terrain_elevations,
            &road_elevations,
            6.0,
            &mut vertices,
            &mut indices,
        );

        assert!(vertices.iter().any(|v| {
            (v.position[0] - 25.0).abs() <= 0.4
                && v.position[1] <= terrain_elevations[1] + 0.1
                && v.position[2].abs() >= 3.0
        }));
        assert!(vertices.iter().any(|v| {
            (v.position[0] - 75.0).abs() <= 0.4
                && v.position[1] <= terrain_elevations[2] + 0.1
                && v.position[2].abs() >= 3.0
        }));
    }

    #[test]
    fn bridge_structure_keeps_beams_well_below_road_surface() {
        let points = [(0.0, 0.0), (20.0, 0.0)];
        let terrain_elevations = [0.0, 0.0];
        let road_elevations = [5.7, 5.7];
        let road_y = road_elevations[0] + ROAD_Y_OFFSET;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_bridge_structure(
            &points,
            &terrain_elevations,
            &road_elevations,
            6.0,
            &mut vertices,
            &mut indices,
        );

        assert!(vertices.iter().any(|v| v.position[1] < road_y));
        assert!(
            vertices
                .iter()
                .filter(|v| v.position[1] < road_y)
                .all(|v| v.position[1] <= road_y - 0.75)
        );
    }

    #[test]
    fn bridge_structure_does_not_emit_broad_deck_top_faces() {
        let points = [(0.0, 0.0), (20.0, 0.0)];
        let terrain_elevations = [0.0, 0.0];
        let road_elevations = [5.7, 5.7];
        let road_y = road_elevations[0] + ROAD_Y_OFFSET;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_bridge_structure(
            &points,
            &terrain_elevations,
            &road_elevations,
            6.0,
            &mut vertices,
            &mut indices,
        );

        let widest_under_road_up_face = indices
            .chunks_exact(3)
            .filter_map(|tri| {
                let tri = [
                    vertices[tri[0] as usize],
                    vertices[tri[1] as usize],
                    vertices[tri[2] as usize],
                ];
                if tri
                    .iter()
                    .all(|v| v.normal == [0.0, 1.0, 0.0] && v.position[1] < road_y)
                {
                    let min_z = tri
                        .iter()
                        .map(|v| v.position[2])
                        .fold(f32::INFINITY, f32::min);
                    let max_z = tri
                        .iter()
                        .map(|v| v.position[2])
                        .fold(f32::NEG_INFINITY, f32::max);
                    Some(max_z - min_z)
                } else {
                    None
                }
            })
            .fold(0.0, f32::max);

        assert!(widest_under_road_up_face <= 1.0);
    }

    #[test]
    fn bridge_structure_box_geometry_has_per_face_normals() {
        let points = [(0.0, 0.0), (20.0, 0.0)];
        let terrain_elevations = [0.0, 0.0];
        let road_elevations = [5.7, 5.7];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_bridge_structure(
            &points,
            &terrain_elevations,
            &road_elevations,
            6.0,
            &mut vertices,
            &mut indices,
        );

        for expected_normal in [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ] {
            assert!(
                vertices.iter().any(|v| v.normal == expected_normal),
                "missing normal {expected_normal:?}"
            );
        }

        assert!(vertices.iter().all(|v| v.feature_type == feature::BUILDING));
    }

    #[test]
    fn tunnel_structure_adds_lining_along_open_tunnel() {
        let points = [(0.0, 0.0), (40.0, 0.0)];
        let road_elevations = [-5.0, -5.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_tunnel_structure(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
        assert!(vertices.iter().any(|v| {
            v.feature_type == feature::BUILDING
                && v.position[0] > 15.0
                && v.position[0] < 25.0
                && v.position[1] > road_elevations[0] + ROAD_Y_OFFSET
        }));
    }

    #[test]
    fn tunnel_structure_adds_portals_for_open_tunnel() {
        let points = [(0.0, 0.0), (20.0, 0.0)];
        let road_elevations = [-5.0, -5.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_tunnel_structure(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
        assert!(vertices.iter().any(|v| v.feature_type == feature::BUILDING));
        assert!(
            vertices
                .iter()
                .any(|v| v.position[1] > road_elevations[0] + ROAD_Y_OFFSET)
        );
    }

    #[test]
    fn tunnel_structure_skips_portals_for_closed_loops() {
        let points = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 0.0)];
        let road_elevations = [-5.0, -5.0, -5.0, -5.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_tunnel_structure(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

        assert!(vertices.is_empty());
        assert!(indices.is_empty());
    }

    #[test]
    fn bridge_render_path_inserts_ramp_breakpoints_for_two_point_bridge() {
        let tags = std::collections::HashMap::from([
            ("highway".to_string(), "primary".to_string()),
            ("bridge".to_string(), "yes".to_string()),
        ]);
        let points = [(0.0, 0.0), (100.0, 0.0)];
        let terrain_elevations = [10.0, 10.0];

        let path = road_render_path(&tags, &points, &terrain_elevations);
        let surface_y = terrain_elevations[0] + surface_road_y_offset(&tags);
        let bridge_y = terrain_elevations[0] + road_layer_y_offset(&tags);

        assert_eq!(
            path.points,
            vec![(0.0, 0.0), (25.0, 0.0), (75.0, 0.0), (100.0, 0.0)]
        );
        assert!((path.road_elevations[0] - surface_y).abs() < 1e-5);
        assert!((path.road_elevations[1] - bridge_y).abs() < 1e-5);
        assert!((path.road_elevations[2] - bridge_y).abs() < 1e-5);
        assert!((path.road_elevations[3] - surface_y).abs() < 1e-5);
    }

    #[test]
    fn bridge_render_elevations_ramp_from_surface_to_bridge_and_back() {
        let tags = std::collections::HashMap::from([
            ("highway".to_string(), "primary".to_string()),
            ("bridge".to_string(), "yes".to_string()),
        ]);
        let points = [(0.0, 0.0), (12.5, 0.0), (25.0, 0.0), (50.0, 0.0)];
        let terrain_elevations = [10.0, 10.0, 10.0, 10.0];

        let elevations = road_render_elevations(&tags, &points, &terrain_elevations);
        let surface_y = terrain_elevations[0] + surface_road_y_offset(&tags);
        let bridge_y = terrain_elevations[0] + road_layer_y_offset(&tags);

        assert!((elevations[0] - surface_y).abs() < 1e-5);
        assert!(elevations[1] > surface_y);
        assert!(elevations[1] < bridge_y);
        assert!((elevations[2] - bridge_y).abs() < 1e-5);
        assert!((elevations[3] - surface_y).abs() < 1e-5);
    }

    #[test]
    fn bridge_render_elevations_clamp_ramps_for_short_bridges() {
        let tags = std::collections::HashMap::from([
            ("highway".to_string(), "primary".to_string()),
            ("bridge".to_string(), "yes".to_string()),
        ]);
        let points = [(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)];
        let terrain_elevations = [0.0, 0.0, 0.0];

        let elevations = road_render_elevations(&tags, &points, &terrain_elevations);
        let surface_y = surface_road_y_offset(&tags);
        let bridge_y = road_layer_y_offset(&tags);

        assert_eq!(elevations[0], surface_y);
        assert_eq!(elevations[1], bridge_y);
        assert_eq!(elevations[2], surface_y);
    }

    #[test]
    fn surface_render_elevations_keep_constant_surface_offset() {
        let tags =
            std::collections::HashMap::from([("highway".to_string(), "primary".to_string())]);
        let points = [(0.0, 0.0), (50.0, 0.0)];
        let terrain_elevations = [1.0, 2.0];

        let elevations = road_render_elevations(&tags, &points, &terrain_elevations);

        assert_eq!(
            elevations,
            vec![
                1.0 + road_layer_y_offset(&tags),
                2.0 + road_layer_y_offset(&tags)
            ]
        );
    }

    #[test]
    fn road_ribbon_can_use_path_feature_type_for_ordered_overlay_draws() {
        let points = [(0.0, 0.0), (10.0, 0.0)];
        let elevations = [5.0, 7.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        generate_road_with_elevations_and_feature_type(
            &points,
            &elevations,
            2.0,
            [1.0, 1.0, 1.0],
            feature::ROAD_PATH,
            &mut vertices,
            &mut indices,
        );

        assert!(!indices.is_empty());
        assert!(
            vertices
                .iter()
                .all(|v| v.feature_type == feature::ROAD_PATH)
        );
    }

    #[test]
    fn road_ribbon_uses_per_point_elevation_offsets() {
        let points = [(0.0, 0.0), (10.0, 0.0)];
        let elevations = [5.0, 7.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        generate_road_with_elevations(
            &points,
            &elevations,
            4.0,
            [1.0, 1.0, 1.0],
            &mut vertices,
            &mut indices,
        );

        assert_eq!(vertices[0].position[1], elevations[0] + ROAD_Y_OFFSET);
        assert_eq!(vertices[1].position[1], elevations[0] + ROAD_Y_OFFSET);
        assert_eq!(vertices[2].position[1], elevations[1] + ROAD_Y_OFFSET);
        assert_eq!(vertices[3].position[1], elevations[1] + ROAD_Y_OFFSET);
    }

    #[test]
    fn road_cap_sits_above_road_ribbon() {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_road_cap(
            (0.0, 0.0),
            5.0,
            4.0,
            [1.0, 1.0, 1.0],
            &mut vertices,
            &mut indices,
        );

        assert_eq!(vertices.len(), ROAD_CAP_SEGMENTS + 1);
        assert_eq!(indices.len(), ROAD_CAP_SEGMENTS * 3);
        assert_eq!(
            vertices[0].position[1],
            5.0 + ROAD_Y_OFFSET + ROAD_CAP_EXTRA_Y_OFFSET
        );
        for tri in indices.chunks_exact(3) {
            let normal_y = triangle_normal_y(
                vertices[tri[0] as usize],
                vertices[tri[1] as usize],
                vertices[tri[2] as usize],
            );
            assert!(
                normal_y > 0.0,
                "road cap triangle {tri:?} normal_y={normal_y}"
            );
        }
    }

    #[test]
    fn closed_road_loop_drops_duplicate_endpoint_and_joins_seam() {
        let points = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ];
        let elevations = [5.0, 5.0, 5.0, 5.0, 5.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        generate_road_with_elevations(
            &points,
            &elevations,
            4.0,
            [1.0, 1.0, 1.0],
            &mut vertices,
            &mut indices,
        );

        assert_eq!(vertices.len(), 8);
        assert_eq!(indices.len(), 24);
        for tri in indices.chunks_exact(3) {
            let normal_y = triangle_normal_y(
                vertices[tri[0] as usize],
                vertices[tri[1] as usize],
                vertices[tri[2] as usize],
            );
            assert!(
                normal_y > 0.0,
                "closed road triangle {tri:?} normal_y={normal_y}"
            );
        }
    }

    #[test]
    fn road_ribbon_ignores_consecutive_duplicate_points() {
        let points = [(0.0, 0.0), (10.0, 0.0), (10.0, 0.0), (20.0, 0.0)];
        let elevations = [5.0, 5.0, 5.0, 5.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        generate_road_with_elevations(
            &points,
            &elevations,
            4.0,
            [1.0, 1.0, 1.0],
            &mut vertices,
            &mut indices,
        );

        assert_eq!(vertices.len(), 6);
        assert_eq!(indices.len(), 12);
    }

    #[test]
    fn road_ribbon_uses_shared_join_vertices_at_curve() {
        let points = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
        let elevations = [5.0, 5.0, 5.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        generate_road_with_elevations(
            &points,
            &elevations,
            4.0,
            [1.0, 1.0, 1.0],
            &mut vertices,
            &mut indices,
        );

        assert_eq!(vertices.len(), points.len() * 2);
        assert_eq!(indices.len(), (points.len() - 1) * 6);
    }

    #[test]
    fn road_ribbon_triangles_face_up_for_back_face_culling() {
        let points = [(0.0, 0.0), (10.0, 0.0)];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        generate_road(
            &points,
            5.0,
            4.0,
            [1.0, 1.0, 1.0],
            &mut vertices,
            &mut indices,
        );
        assert!(!indices.is_empty(), "expected road triangles");

        for tri in indices.chunks_exact(3) {
            let normal_y = triangle_normal_y(
                vertices[tri[0] as usize],
                vertices[tri[1] as usize],
                vertices[tri[2] as usize],
            );
            assert!(normal_y > 0.0, "road triangle {tri:?} normal_y={normal_y}");
        }
    }
}
