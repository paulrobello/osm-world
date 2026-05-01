//! Road ribbon strip mesh generator.

use crate::render::vertex::{Vertex, feature};

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
    let half_width = width / 2.0;
    let normal = [0.0, 1.0, 0.0];
    let y = y + 0.1; // small offset above terrain

    for i in 0..points.len().saturating_sub(1) {
        let (x0, z0) = points[i];
        let (x1, z1) = points[i + 1];

        // Direction vector
        let dx = x1 - x0;
        let dz = z1 - z0;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1e-6 {
            continue;
        }
        let dx = dx / len;
        let dz = dz / len;

        // Perpendicular vector (-dz, dx)
        let px = -dz * half_width;
        let pz = dx * half_width;

        let base = verts.len() as u32;

        // Left-start, right-start, left-end, right-end
        verts.push(Vertex {
            position: [x0 + px, y, z0 + pz],
            normal,
            color,
            feature_type: feature::ROAD,
        });
        verts.push(Vertex {
            position: [x0 - px, y, z0 - pz],
            normal,
            color,
            feature_type: feature::ROAD,
        });
        verts.push(Vertex {
            position: [x1 + px, y, z1 + pz],
            normal,
            color,
            feature_type: feature::ROAD,
        });
        verts.push(Vertex {
            position: [x1 - px, y, z1 - pz],
            normal,
            color,
            feature_type: feature::ROAD,
        });

        // Two triangles forming a quad: 0-1-2, 1-3-2
        idxs.push(base);
        idxs.push(base + 1);
        idxs.push(base + 2);
        idxs.push(base + 1);
        idxs.push(base + 3);
        idxs.push(base + 2);
    }
}
