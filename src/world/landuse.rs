//! Landuse mesh generator.

use std::collections::HashMap;

use crate::render::vertex::{Vertex, feature};

pub const LANDUSE_Y_OFFSET: f32 = 1.0;
const LANDUSE_OVERLAY_Y_OFFSET: f32 = 1.5;

pub fn landuse_y_offset(tags: &HashMap<String, String>) -> f32 {
    let is_green_overlay = tags.contains_key("leisure")
        || matches!(
            tags.get("natural").map(String::as_str),
            Some(
                "wood" | "tree_row" | "tree" | "scrub" | "heath" | "moor" | "grassland" | "meadow"
            )
        )
        || matches!(
            tags.get("landuse").map(String::as_str),
            Some(
                "forest"
                    | "wood"
                    | "meadow"
                    | "grass"
                    | "grassland"
                    | "recreation_ground"
                    | "village_green"
            )
        );

    if is_green_overlay {
        LANDUSE_OVERLAY_Y_OFFSET
    } else {
        LANDUSE_Y_OFFSET
    }
}

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
    let elevations = vec![y; points.len()];
    generate_landuse_with_elevations(points, &elevations, color, verts, idxs);
}

/// Generate a triangulated landuse mesh with per-point terrain elevations.
pub fn generate_landuse_with_elevations(
    points: &[(f32, f32)],
    elevations: &[f32],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    generate_landuse_with_elevations_and_offset(
        points,
        elevations,
        LANDUSE_Y_OFFSET,
        color,
        verts,
        idxs,
    );
}

pub fn generate_landuse_with_elevations_and_offset(
    points: &[(f32, f32)],
    elevations: &[f32],
    y_offset: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() != elevations.len() {
        return;
    }

    let normal = [0.0, 1.0, 0.0];

    let base = verts.len() as u32;

    for (i, &(x, z)) in points.iter().enumerate() {
        verts.push(Vertex {
            position: [x, elevations[i] + y_offset, z],
            normal,
            color,
            uv: [0.0, 0.0],
            feature_type: feature::LANDUSE,
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
    fn green_landuse_overlay_offset_is_above_base_landuse() {
        let mut base_tags = std::collections::HashMap::new();
        base_tags.insert("landuse".to_string(), "residential".to_string());
        let mut overlay_tags = std::collections::HashMap::new();
        overlay_tags.insert("leisure".to_string(), "park".to_string());

        assert!(landuse_y_offset(&overlay_tags) > landuse_y_offset(&base_tags));
    }

    #[test]
    fn landuse_offsets_are_large_enough_for_depth_precision() {
        let mut base_tags = std::collections::HashMap::new();
        base_tags.insert("landuse".to_string(), "residential".to_string());
        let mut overlay_tags = std::collections::HashMap::new();
        overlay_tags.insert("leisure".to_string(), "park".to_string());

        assert!(landuse_y_offset(&base_tags) >= 1.0);
        assert!(landuse_y_offset(&overlay_tags) >= landuse_y_offset(&base_tags) + 0.5);
    }

    #[test]
    fn landuse_uses_per_point_elevation_offsets() {
        let points = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let elevations = [5.0, 6.0, 7.0, 8.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        generate_landuse_with_elevations(
            &points,
            &elevations,
            [1.0, 1.0, 1.0],
            &mut vertices,
            &mut indices,
        );

        for (vertex, elevation) in vertices.iter().zip(elevations) {
            assert_eq!(vertex.position[1], elevation + LANDUSE_Y_OFFSET);
        }
    }

    #[test]
    fn landuse_triangles_face_up_for_back_face_culling() {
        let points = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        generate_landuse(&points, 5.0, [1.0, 1.0, 1.0], &mut vertices, &mut indices);
        assert!(!indices.is_empty(), "expected landuse triangles");

        for tri in indices.chunks_exact(3) {
            let normal_y = triangle_normal_y(
                vertices[tri[0] as usize],
                vertices[tri[1] as usize],
                vertices[tri[2] as usize],
            );
            assert!(
                normal_y > 0.0,
                "landuse triangle {tri:?} normal_y={normal_y}"
            );
        }
    }
}
