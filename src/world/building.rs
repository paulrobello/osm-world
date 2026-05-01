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
    if let Some(levels) = tags.get("building:levels") {
        if let Ok(v) = levels.trim().parse::<f32>() {
            return v * 3.0;
        }
    }
    10.0
}

/// Generate a building mesh from a closed footprint polygon.
///
/// `footprint` is the polygon as (x, z) world-space coordinates (first and last
/// point should NOT be duplicated).
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

        // Outward normal (perpendicular pointing away from the polygon interior)
        let nx = -ez / len;
        let nz = ex / len;
        let normal = [nx, 0.0, nz];

        let base_idx = verts.len() as u32;

        // Four corners of the wall quad: bottom-left, bottom-right, top-left, top-right
        verts.push(Vertex {
            position: [x0, base_y, z0],
            normal,
            color,
            feature_type: feature::BUILDING,
        });
        verts.push(Vertex {
            position: [x1, base_y, z1],
            normal,
            color,
            feature_type: feature::BUILDING,
        });
        verts.push(Vertex {
            position: [x0, top_y, z0],
            normal,
            color,
            feature_type: feature::BUILDING,
        });
        verts.push(Vertex {
            position: [x1, top_y, z1],
            normal,
            color,
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
            color,
            feature_type: feature::BUILDING,
        });
    }

    // Triangulate the roof polygon using earcutr (flat f64 array: x0, y0, x1, y1, ...)
    let earcut_pts: Vec<f64> = footprint
        .iter()
        .flat_map(|&(x, z)| [x as f64, z as f64])
        .collect();
    if let Ok(triangles) = earcutr::earcut(&earcut_pts, &[], 2) {
        for idx in triangles {
            idxs.push(roof_base + idx as u32);
        }
    }
}
