//! Water mesh generator.

use crate::render::vertex::{Vertex, feature};

/// Generate a flat triangulated mesh for a water polygon.
///
/// `points` is the footprint polygon as (x, z) world-space coordinates.
/// `y` is the water surface height (typically 0.0 for sea level).
/// Generated vertices and indices are appended to `verts` and `idxs`.
pub fn generate_water(points: &[(f32, f32)], y: f32, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    let color = super::color::water_color();
    let normal = [0.0, 1.0, 0.0];

    let base = verts.len() as u32;

    for &(x, z) in points {
        verts.push(Vertex {
            position: [x, y, z],
            normal,
            color,
            feature_type: feature::WATER,
        });
    }

    // Triangulate using earcutr (flat f64 array: x0, y0, x1, y1, ...)
    let earcut_pts: Vec<f64> = points
        .iter()
        .flat_map(|&(x, z)| [x as f64, z as f64])
        .collect();
    if let Ok(triangles) = earcutr::earcut(&earcut_pts, &[], 2) {
        for tri in triangles.chunks_exact(3) {
            // earcut positive X/Z winding faces -Y in this coordinate system.
            idxs.push(base + tri[0] as u32);
            idxs.push(base + tri[2] as u32);
            idxs.push(base + tri[1] as u32);
        }
    }
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
    fn water_triangles_face_up_for_back_face_culling() {
        let points = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        generate_water(&points, 5.0, &mut vertices, &mut indices);
        assert!(!indices.is_empty(), "expected water triangles");

        for tri in indices.chunks_exact(3) {
            let normal_y = triangle_normal_y(
                vertices[tri[0] as usize],
                vertices[tri[1] as usize],
                vertices[tri[2] as usize],
            );
            assert!(normal_y > 0.0, "water triangle {tri:?} normal_y={normal_y}");
        }
    }
}
