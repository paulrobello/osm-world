//! Flat road ribbon mesh generation. Produces the upward-facing triangle
//! strip that forms the visible road surface.

use crate::mesh::{Vertex, feature};

use super::ROAD_Y_OFFSET;
use super::geometry::same_point;

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
