//! Road ribbon strip mesh generator.

use crate::render::vertex::{Vertex, feature};

// Sacramento screenshots are taken from kilometre-scale distances; keep roads
// separated by multiple metres so depth quantization does not fight landuse.
pub const ROAD_Y_OFFSET: f32 = 2.0;
const ROAD_CAP_EXTRA_Y_OFFSET: f32 = 0.05;
const ROAD_CAP_SEGMENTS: usize = 12;
pub const ROAD_CAP_RADIUS_SCALE: f32 = 1.05;

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
