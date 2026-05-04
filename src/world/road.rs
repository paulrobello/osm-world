//! Road ribbon strip mesh generator.

use std::collections::HashMap;

use crate::render::vertex::{Vertex, feature};

// Sacramento screenshots are taken from kilometre-scale distances; keep roads
// separated by multiple metres so depth quantization does not fight landuse.
pub const ROAD_Y_OFFSET: f32 = 2.0;
const ROAD_CAP_EXTRA_Y_OFFSET: f32 = 0.05;
const ROAD_BRIDGE_LAYER_Y_OFFSET: f32 = 5.0;
const ROAD_TUNNEL_LAYER_Y_OFFSET: f32 = -5.0;
const ROAD_CAP_SEGMENTS: usize = 12;
pub const ROAD_CAP_RADIUS_SCALE: f32 = 1.05;
const BRIDGE_DECK_THICKNESS: f32 = 0.6;
const BRIDGE_RAIL_HEIGHT: f32 = 0.9;
const BRIDGE_RAIL_WIDTH: f32 = 0.25;
const BRIDGE_SUPPORT_WIDTH: f32 = 0.8;
const TUNNEL_PORTAL_DEPTH: f32 = 1.0;
const TUNNEL_PORTAL_THICKNESS: f32 = 0.5;
const TUNNEL_CLEARANCE: f32 = 3.0;
const BRIDGE_STRUCTURE_COLOR: [f32; 3] = [0.50, 0.52, 0.54];
const TUNNEL_STRUCTURE_COLOR: [f32; 3] = [0.34, 0.32, 0.30];

/// Additional per-feature Y offset applied before road ribbon generation.
///
/// The ribbon generator already adds [`ROAD_Y_OFFSET`] above sampled terrain.
/// This offset keeps road/path overlays at least about one metre above green
/// landuse overlays and separates bridges/layered crossings by several metres
/// so large city-scale depth buffers do not z-fight at intersections.
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
    let width = super::color::road_width(tags);
    let surface_offset = if width >= 5.0 {
        0.7
    } else if width >= 3.5 {
        0.6
    } else {
        0.5
    };

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
            feature_type: feature::ROAD,
        });
        verts.push(Vertex {
            position: [x - px, y, z - pz],
            normal,
            color,
            feature_type: feature::ROAD,
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
    let Some(frame) = segment_frame(strip.a, strip.b) else {
        return;
    };
    let (px, pz) = frame.perpendicular;
    let corners = [
        (
            strip.a.0 + px * (strip.lateral_offset - strip.half_width),
            strip.a.1 + pz * (strip.lateral_offset - strip.half_width),
        ),
        (
            strip.a.0 + px * (strip.lateral_offset + strip.half_width),
            strip.a.1 + pz * (strip.lateral_offset + strip.half_width),
        ),
        (
            strip.b.0 + px * (strip.lateral_offset - strip.half_width),
            strip.b.1 + pz * (strip.lateral_offset - strip.half_width),
        ),
        (
            strip.b.0 + px * (strip.lateral_offset + strip.half_width),
            strip.b.1 + pz * (strip.lateral_offset + strip.half_width),
        ),
    ];
    let (min_x, max_x, min_z, max_z) = bounds2d(&corners);
    append_box(
        [min_x, strip.min_y, min_z],
        [max_x, strip.max_y, max_z],
        strip.color,
        verts,
        idxs,
    );
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

    let half_width = (width * 0.5).max(0.0);
    let rail_half_width = (BRIDGE_RAIL_WIDTH * 0.5).max(0.05);
    let rail_offset = (half_width - rail_half_width).max(rail_half_width);
    for i in 0..points.len() - 1 {
        if segment_frame(points[i], points[i + 1]).is_none() {
            continue;
        }
        let road_y = road_elevations[i].max(road_elevations[i + 1]) + ROAD_Y_OFFSET;
        let terrain_y = terrain_elevations[i].min(terrain_elevations[i + 1]);

        append_segment_strip_box(
            SegmentStripBox {
                a: points[i],
                b: points[i + 1],
                lateral_offset: 0.0,
                half_width: half_width + BRIDGE_RAIL_WIDTH * 0.25,
                min_y: road_y - BRIDGE_DECK_THICKNESS,
                max_y: road_y - 0.08,
                color: BRIDGE_STRUCTURE_COLOR,
            },
            verts,
            idxs,
        );
        append_segment_strip_box(
            SegmentStripBox {
                a: points[i],
                b: points[i + 1],
                lateral_offset: rail_offset,
                half_width: rail_half_width,
                min_y: road_y + 0.05,
                max_y: road_y + BRIDGE_RAIL_HEIGHT,
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
                min_y: road_y + 0.05,
                max_y: road_y + BRIDGE_RAIL_HEIGHT,
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
                    road_y - BRIDGE_DECK_THICKNESS,
                    cz + half_support,
                ],
                BRIDGE_STRUCTURE_COLOR,
                verts,
                idxs,
            );
        }
    }
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
        feature_type: feature::ROAD,
    });

    for i in 0..ROAD_CAP_SEGMENTS {
        let angle = i as f32 / ROAD_CAP_SEGMENTS as f32 * std::f32::consts::TAU;
        verts.push(Vertex {
            position: [x + angle.cos() * radius, y, z + angle.sin() * radius],
            normal,
            color,
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

        assert!(road_layer_y_offset(&surface) >= 0.5);
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
    fn bridge_structure_adds_deck_rails_and_support_geometry() {
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
