//! Building mesh generator.

use std::collections::HashMap;

use crate::render::vertex::{Vertex, feature};

/// Parse building height from OSM tags.
///
/// Tries, in order:
/// 1. `height` tag (strips trailing "m")
/// 2. `building:levels` tag multiplied by 3.0 metres
/// 3. Default of 10.0 metres
pub fn parse_building_height(tags: &HashMap<String, String>) -> f32 {
    if let Some(h) = tags.get("height") {
        let h = h.trim().trim_end_matches('m').trim();
        if let Ok(v) = h.parse::<f32>() {
            return v;
        }
    }
    if let Some(levels) = tags.get("building:levels")
        && let Ok(v) = levels.trim().parse::<f32>()
    {
        return v * 3.0;
    }
    10.0
}

/// Generate a building mesh from a closed footprint polygon.
///
/// `footprint` is the polygon as (x, z) world-space coordinates in CCW order
/// (first and last point should NOT be duplicated).
/// `base_y` is the terrain height at the building base.
/// `height` is the building height in metres.
/// `color` is the RGB colour for all vertices.
/// Generated vertices and indices are appended to `verts` and `idxs`.
pub fn generate_building(
    footprint: &[(f32, f32)],
    base_y: f32,
    height: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    generate_building_with_style(
        footprint,
        base_y,
        height,
        super::color::BuildingStyle {
            wall_color: color,
            roof_color: color,
            band_color: color,
            facade_intensity: 0.0,
            roof_intensity: 0.0,
        },
        verts,
        idxs,
    );
}

pub fn generate_building_with_style(
    footprint: &[(f32, f32)],
    base_y: f32,
    height: f32,
    style: super::color::BuildingStyle,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if footprint.len() < 3 {
        return;
    }

    let top_y = base_y + height;

    // -- Walls --
    let n = footprint.len();
    for i in 0..n {
        let (x0, z0) = footprint[i];
        let (x1, z1) = footprint[(i + 1) % n];

        // Edge direction
        let ex = x1 - x0;
        let ez = z1 - z0;
        let len = (ex * ex + ez * ez).sqrt();
        if len < 1e-6 {
            continue;
        }

        // Outward normal for a CCW footprint in X/Z. The polygon interior is
        // to the left of each edge in X/Z, so outward is the right-hand side.
        let nx = ez / len;
        let nz = -ex / len;
        let normal = [nx, 0.0, nz];

        let base_idx = verts.len() as u32;

        // Four corners of the wall quad: bottom-left, bottom-right, top-left, top-right.
        verts.push(Vertex {
            position: [x0, base_y, z0],
            normal,
            color: style.wall_color,
            uv: [0.0, 0.0],
            feature_type: feature::BUILDING,
        });
        verts.push(Vertex {
            position: [x1, base_y, z1],
            normal,
            color: style.wall_color,
            uv: [1.0, 0.0],
            feature_type: feature::BUILDING,
        });
        verts.push(Vertex {
            position: [x0, top_y, z0],
            normal,
            color: style.band_color,
            uv: [0.0, 1.0],
            feature_type: feature::BUILDING,
        });
        verts.push(Vertex {
            position: [x1, top_y, z1],
            normal,
            color: style.band_color,
            uv: [1.0, 1.0],
            feature_type: feature::BUILDING,
        });

        // Two triangles: 0-2-1, 1-2-3
        idxs.push(base_idx);
        idxs.push(base_idx + 2);
        idxs.push(base_idx + 1);
        idxs.push(base_idx + 1);
        idxs.push(base_idx + 2);
        idxs.push(base_idx + 3);
    }

    // -- Roof (flat top) --
    let roof_normal = [0.0, 1.0, 0.0];
    let roof_base = verts.len() as u32;

    for &(x, z) in footprint {
        verts.push(Vertex {
            position: [x, top_y, z],
            normal: roof_normal,
            color: style.roof_color,
            uv: [0.0, 0.0],
            feature_type: feature::BUILDING,
        });
    }

    // Triangulate the roof polygon using earcutr (flat f64 array: x0, y0, x1, y1, ...)
    let earcut_pts: Vec<f64> = footprint
        .iter()
        .flat_map(|&(x, z)| [x as f64, z as f64])
        .collect();
    if let Ok(triangles) = earcutr::earcut(&earcut_pts, &[], 2) {
        for tri in triangles.chunks_exact(3) {
            // earcut returns triangles with positive 2D winding for CCW X/Z input.
            // In the X/Y/Z coordinate system, a positive X/Z winding faces -Y,
            // so reverse each roof triangle to make flat roofs front-facing upward.
            idxs.push(roof_base + tri[0] as u32);
            idxs.push(roof_base + tri[2] as u32);
            idxs.push(roof_base + tri[1] as u32);
        }
    }
}

pub fn generate_simplified_building_with_style(
    footprint: &[(f32, f32)],
    base_y: f32,
    height: f32,
    style: super::color::BuildingStyle,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if footprint.len() < 3 {
        return;
    }

    let (mut min_x, mut min_z) = (f32::INFINITY, f32::INFINITY);
    let (mut max_x, mut max_z) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    for &(x, z) in footprint {
        min_x = min_x.min(x);
        min_z = min_z.min(z);
        max_x = max_x.max(x);
        max_z = max_z.max(z);
    }

    if max_x - min_x < 1e-3 || max_z - min_z < 1e-3 {
        generate_building_with_style(footprint, base_y, height, style, verts, idxs);
        return;
    }

    let simplified = [
        (min_x, min_z),
        (max_x, min_z),
        (max_x, max_z),
        (min_x, max_z),
    ];
    generate_building_with_style(&simplified, base_y, height, style, verts, idxs);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated_square() -> (Vec<Vertex>, Vec<u32>) {
        let footprint = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        generate_building(
            &footprint,
            10.0,
            5.0,
            [1.0, 1.0, 1.0],
            &mut vertices,
            &mut indices,
        );
        (vertices, indices)
    }

    fn triangle_normal_y(a: Vertex, b: Vertex, c: Vertex) -> f32 {
        let ux = b.position[0] - a.position[0];
        let uz = b.position[2] - a.position[2];
        let vx = c.position[0] - a.position[0];
        let vz = c.position[2] - a.position[2];
        uz * vx - ux * vz
    }

    #[test]
    fn styled_building_uses_roof_color_for_roof_vertices() {
        let footprint = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let style = super::super::color::BuildingStyle {
            wall_color: [0.1, 0.2, 0.3],
            roof_color: [0.4, 0.5, 0.6],
            band_color: [0.7, 0.8, 0.9],
            facade_intensity: 1.0,
            roof_intensity: 1.0,
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        generate_building_with_style(&footprint, 0.0, 5.0, style, &mut vertices, &mut indices);

        assert!(
            vertices[16..]
                .iter()
                .all(|vertex| vertex.color == style.roof_color)
        );
    }

    #[test]
    fn styled_building_wall_uvs_encode_edge_progress_and_height_ratio() {
        let footprint = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)];
        let style = super::super::color::BuildingStyle {
            wall_color: [0.1, 0.2, 0.3],
            roof_color: [0.4, 0.5, 0.6],
            band_color: [0.7, 0.8, 0.9],
            facade_intensity: 1.0,
            roof_intensity: 1.0,
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        generate_building_with_style(&footprint, 0.0, 5.0, style, &mut vertices, &mut indices);

        assert_eq!(vertices[0].uv, [0.0, 0.0]);
        assert_eq!(vertices[1].uv, [1.0, 0.0]);
        assert_eq!(vertices[2].uv, [0.0, 1.0]);
        assert_eq!(vertices[3].uv, [1.0, 1.0]);
        assert_eq!(vertices[2].color, style.band_color);
    }

    #[test]
    fn simplified_building_uses_bounding_box_and_reduces_complex_footprints() {
        let footprint = [
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 0.5),
            (1.0, 0.5),
            (1.0, 1.0),
            (2.0, 1.0),
            (2.0, 2.0),
            (0.0, 2.0),
        ];
        let style = super::super::color::BuildingStyle {
            wall_color: [0.1, 0.2, 0.3],
            roof_color: [0.4, 0.5, 0.6],
            band_color: [0.7, 0.8, 0.9],
            facade_intensity: 1.0,
            roof_intensity: 1.0,
        };
        let mut full_vertices = Vec::new();
        let mut full_indices = Vec::new();
        let mut simple_vertices = Vec::new();
        let mut simple_indices = Vec::new();

        generate_building_with_style(
            &footprint,
            3.0,
            9.0,
            style,
            &mut full_vertices,
            &mut full_indices,
        );
        generate_simplified_building_with_style(
            &footprint,
            3.0,
            9.0,
            style,
            &mut simple_vertices,
            &mut simple_indices,
        );

        assert!(simple_vertices.len() < full_vertices.len());
        assert!(simple_indices.len() < full_indices.len());
        assert!(
            simple_vertices
                .iter()
                .any(|v| v.position == [0.0, 3.0, 0.0])
        );
        assert!(
            simple_vertices
                .iter()
                .any(|v| v.position == [2.0, 12.0, 2.0])
        );
    }

    #[test]
    fn roof_triangles_face_up_for_ccw_footprint() {
        let (vertices, indices) = generated_square();
        assert!(indices.len() >= 6, "expected at least two roof triangles");
        let roof_indices = &indices[indices.len() - 6..];

        for tri in roof_indices.chunks_exact(3) {
            let normal_y = triangle_normal_y(
                vertices[tri[0] as usize],
                vertices[tri[1] as usize],
                vertices[tri[2] as usize],
            );
            assert!(normal_y > 0.0, "roof triangle {tri:?} normal_y={normal_y}");
        }
    }

    #[test]
    fn wall_normals_point_outward_for_ccw_footprint() {
        let (vertices, _) = generated_square();

        // First footprint edge is (0,0) -> (1,0). For a CCW polygon in X/Z,
        // the interior is +Z, so the outward normal is -Z.
        assert_eq!(vertices[0].normal, [0.0, 0.0, -1.0]);
        assert_eq!(vertices[1].normal, [0.0, 0.0, -1.0]);
        assert_eq!(vertices[2].normal, [0.0, 0.0, -1.0]);
        assert_eq!(vertices[3].normal, [0.0, 0.0, -1.0]);
    }
}
