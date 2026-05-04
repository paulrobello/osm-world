//! Terrain heightfield mesh generator.

use crate::geo::{CoordConverter, ElevationData};
use crate::render::vertex::{Vertex, feature};

const GRID_SPACING: f32 = 10.0; // metres between grid vertices

/// Generate a terrain heightfield mesh for the given bounding box.
///
/// Creates a regular grid, samples elevation at each vertex, and computes
/// per-vertex normals via finite differences.
#[allow(clippy::too_many_arguments)]
pub fn generate_terrain(
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
    conv: &CoordConverter,
    elevation: Option<&ElevationData>,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let (world_w, world_d) = conv.bbox_world_size(min_lat, min_lon, max_lat, max_lon);

    let cols = (world_w / GRID_SPACING).ceil() as usize + 1;
    let rows = (world_d / GRID_SPACING).ceil() as usize + 1;

    if cols < 2 || rows < 2 {
        return;
    }

    // Grid origin in world space: (x, z) where z = -(lat - origin_lat) * scale
    // max_lat -> most negative z, min_lat -> least negative z
    let (min_x, min_z) = conv.to_world_xz(max_lat, min_lon);
    let (_max_x, _max_z) = conv.to_world_xz(min_lat, max_lon);

    append_terrain_grid(
        min_x,
        min_z,
        cols,
        rows,
        GRID_SPACING,
        conv,
        elevation,
        verts,
        idxs,
    );
}

/// Generate a terrain heightfield mesh for the given world-space rectangle.
///
/// Creates a regular grid whose X/Z bounds come from the supplied rectangle,
/// samples elevation at each vertex, and computes per-vertex normals via finite
/// differences.
#[allow(clippy::too_many_arguments)]
pub fn generate_terrain_for_world_rect(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    grid_spacing: f32,
    conv: &CoordConverter,
    elevation: Option<&ElevationData>,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let cols = ((max_x - min_x) / grid_spacing).ceil() as usize + 1;
    let rows = ((max_z - min_z) / grid_spacing).abs().ceil() as usize + 1;

    if cols < 2 || rows < 2 {
        return;
    }

    append_terrain_grid(
        min_x,
        min_z,
        cols,
        rows,
        grid_spacing,
        conv,
        elevation,
        verts,
        idxs,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_terrain_grid(
    min_x: f32,
    min_z: f32,
    cols: usize,
    rows: usize,
    grid_spacing: f32,
    conv: &CoordConverter,
    elevation: Option<&ElevationData>,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let color = super::color::terrain_color();

    // Sample heights into a 2D array
    let mut heights = vec![0.0f32; rows * cols];

    for r in 0..rows {
        for c in 0..cols {
            let x = min_x + (c as f32) * grid_spacing;
            let z = min_z + (r as f32) * grid_spacing;

            // Reverse world coords to lat/lon for elevation lookup
            let lat = conv.origin_lat - (z as f64) / 111_320.0;
            let metres_per_deg_lon = 111_320.0 * conv.origin_lat.to_radians().cos();
            let lon = conv.origin_lon + (x as f64) / metres_per_deg_lon;

            let h = elevation
                .and_then(|e| e.elevation_at(lat, lon))
                .unwrap_or(0.0) as f32;

            heights[r * cols + c] = h;
        }
    }

    // Compute normals via finite differences and emit vertices
    let base = verts.len() as u32;

    for r in 0..rows {
        for c in 0..cols {
            let x = min_x + (c as f32) * grid_spacing;
            let z = min_z + (r as f32) * grid_spacing;
            let h = heights[r * cols + c];

            // Finite difference normals
            let h_left = if c > 0 {
                heights[r * cols + (c - 1)]
            } else {
                h
            };
            let h_right = if c + 1 < cols {
                heights[r * cols + (c + 1)]
            } else {
                h
            };
            let h_up = if r > 0 {
                heights[(r - 1) * cols + c]
            } else {
                h
            };
            let h_down = if r + 1 < rows {
                heights[(r + 1) * cols + c]
            } else {
                h
            };

            // dx = (2 * grid_spacing, h_right - h_left, 0)
            // dz = (0, h_down - h_up, 2 * grid_spacing)
            // normal = cross(dx, dz) but we use cross(dz, dx) for upward facing
            let dhdx = (h_right - h_left) / (2.0 * grid_spacing);
            let dhdz = (h_down - h_up) / (2.0 * grid_spacing);
            let nx = -dhdx;
            let ny = 1.0;
            let nz = -dhdz;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            let normal = if len > 1e-6 {
                [nx / len, ny / len, nz / len]
            } else {
                [0.0, 1.0, 0.0]
            };

            verts.push(Vertex {
                position: [x, h, z],
                normal,
                color,
                uv: [0.0, 0.0],
                feature_type: feature::TERRAIN,
            });
        }
    }

    // Generate indices: two triangles per grid cell
    for r in 0..rows - 1 {
        for c in 0..cols - 1 {
            let i00 = base + (r * cols + c) as u32;
            let i10 = base + (r * cols + c + 1) as u32;
            let i01 = base + ((r + 1) * cols + c) as u32;
            let i11 = base + ((r + 1) * cols + c + 1) as u32;

            idxs.push(i00);
            idxs.push(i01);
            idxs.push(i10);

            idxs.push(i10);
            idxs.push(i01);
            idxs.push(i11);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_terrain_generates_grid_for_bounds() {
        let conv = CoordConverter::new(38.0, -122.0);
        let mut verts = Vec::new();
        let mut idxs = Vec::new();
        generate_terrain_for_world_rect(
            0.0, -100.0, 100.0, 0.0, 50.0, &conv, None, &mut verts, &mut idxs,
        );
        assert_eq!(verts.len(), 9);
        assert_eq!(idxs.len(), 24);
    }
}
