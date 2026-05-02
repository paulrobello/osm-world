//! Landuse mesh generator.

use crate::render::vertex::{Vertex, feature};

/// Generate a flat triangulated mesh for a landuse polygon.
///
/// `points` is the footprint polygon as (x, z) world-space coordinates.
/// `y` is the base height (typically terrain elevation + small offset).
/// `color` is the RGB colour for all vertices.
/// Generated vertices and indices are appended to `verts` and `idxs`.
pub fn generate_landuse(
    points: &[(f32, f32)],
    y: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let y = y + 0.5; // offset above terrain to avoid z-fighting
    let normal = [0.0, 1.0, 0.0];

    let base = verts.len() as u32;

    for &(x, z) in points {
        verts.push(Vertex {
            position: [x, y, z],
            normal,
            color,
            feature_type: feature::LANDUSE,
        });
    }

    // Triangulate using earcutr (flat f64 array: x0, y0, x1, y1, ...)
    let earcut_pts: Vec<f64> = points
        .iter()
        .flat_map(|&(x, z)| [x as f64, z as f64])
        .collect();
    if let Ok(triangles) = earcutr::earcut(&earcut_pts, &[], 2) {
        for idx in triangles {
            idxs.push(base + idx as u32);
        }
    }
}
