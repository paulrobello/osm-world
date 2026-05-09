//! Terrain heightfield mesh generator.

use crate::geo::{CoordConverter, ElevationData};
use crate::render::vertex::{Vertex, feature};

const GRID_SPACING: f32 = 10.0; // metres between grid vertices

/// Mutable output buffers for terrain mesh generation.
pub struct MeshOutput<'a> {
    pub vertices: &'a mut Vec<Vertex>,
    pub indices: &'a mut Vec<u32>,
}

/// Shared terrain context: coordinate converter, elevation data, and road cuts.
pub struct TerrainContext<'a> {
    pub conv: &'a CoordConverter,
    pub elevation: Option<&'a ElevationData>,
    pub cuts: &'a [TerrainCut],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainCut {
    pub start: (f32, f32),
    pub end: (f32, f32),
    pub half_width: f32,
    pub floor_y: f32,
    pub blend_width: f32,
}

impl TerrainCut {
    fn apply(self, x: f32, z: f32, height: f32) -> f32 {
        if self.half_width <= 0.0 || self.blend_width < 0.0 {
            return height;
        }

        let dx = self.end.0 - self.start.0;
        let dz = self.end.1 - self.start.1;
        let len_sq = dx * dx + dz * dz;
        if len_sq < 1e-6 {
            return height;
        }

        let t = (((x - self.start.0) * dx + (z - self.start.1) * dz) / len_sq).clamp(0.0, 1.0);
        let closest_x = self.start.0 + dx * t;
        let closest_z = self.start.1 + dz * t;
        let distance = ((x - closest_x).powi(2) + (z - closest_z).powi(2)).sqrt();
        let blend_limit = self.half_width + self.blend_width;
        if distance > blend_limit || height <= self.floor_y {
            return height;
        }

        if distance <= self.half_width || self.blend_width <= 1e-6 {
            return self.floor_y;
        }

        let blend_t = ((distance - self.half_width) / self.blend_width).clamp(0.0, 1.0);
        self.floor_y + (height - self.floor_y) * smoothstep(blend_t)
    }
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Generate a terrain heightfield mesh for the given bounding box.
///
/// Creates a regular grid, samples elevation at each vertex, and computes
/// per-vertex normals via finite differences.
pub fn generate_terrain(
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
    ctx: &TerrainContext<'_>,
    output: &mut MeshOutput<'_>,
) {
    generate_terrain_with_cuts(min_lat, min_lon, max_lat, max_lon, ctx, output);
}

pub fn generate_terrain_with_cuts(
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
    ctx: &TerrainContext<'_>,
    output: &mut MeshOutput<'_>,
) {
    let (world_w, world_d) = ctx.conv.bbox_world_size(min_lat, min_lon, max_lat, max_lon);

    let cols = (world_w / GRID_SPACING).ceil() as usize + 1;
    let rows = (world_d / GRID_SPACING).ceil() as usize + 1;

    if cols < 2 || rows < 2 {
        return;
    }

    // Grid origin in world space: (x, z) where z = -(lat - origin_lat) * scale
    // max_lat -> most negative z, min_lat -> least negative z
    let (min_x, min_z) = ctx.conv.to_world_xz(max_lat, min_lon);
    let (_max_x, _max_z) = ctx.conv.to_world_xz(min_lat, max_lon);

    append_terrain_grid(min_x, min_z, cols, rows, GRID_SPACING, ctx, output);
}

/// Generate a terrain heightfield mesh for the given world-space rectangle.
///
/// Creates a regular grid whose X/Z bounds come from the supplied rectangle,
/// samples elevation at each vertex, and computes per-vertex normals via finite
/// differences.
pub fn generate_terrain_for_world_rect(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    grid_spacing: f32,
    ctx: &TerrainContext<'_>,
    output: &mut MeshOutput<'_>,
) {
    generate_terrain_for_world_rect_with_cuts(
        min_x,
        min_z,
        max_x,
        max_z,
        grid_spacing,
        ctx,
        output,
    );
}

pub fn generate_terrain_for_world_rect_with_cuts(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    grid_spacing: f32,
    ctx: &TerrainContext<'_>,
    output: &mut MeshOutput<'_>,
) {
    let cols = ((max_x - min_x) / grid_spacing).ceil() as usize + 1;
    let rows = ((max_z - min_z) / grid_spacing).abs().ceil() as usize + 1;

    if cols < 2 || rows < 2 {
        return;
    }

    append_terrain_grid(min_x, min_z, cols, rows, grid_spacing, ctx, output);
}

fn append_terrain_grid(
    min_x: f32,
    min_z: f32,
    cols: usize,
    rows: usize,
    grid_spacing: f32,
    ctx: &TerrainContext<'_>,
    output: &mut MeshOutput<'_>,
) {
    let color = super::color::terrain_color();

    // Sample heights into a 2D array
    let mut heights = vec![0.0f32; rows * cols];

    for r in 0..rows {
        for c in 0..cols {
            let x = min_x + (c as f32) * grid_spacing;
            let z = min_z + (r as f32) * grid_spacing;

            // Reverse world coords to lat/lon for elevation lookup
            let lat = ctx.conv.origin_lat - (z as f64) / 111_320.0;
            let metres_per_deg_lon = 111_320.0 * ctx.conv.origin_lat.to_radians().cos();
            let lon = ctx.conv.origin_lon + (x as f64) / metres_per_deg_lon;

            let mut h = ctx
                .elevation
                .and_then(|e| e.elevation_at(lat, lon))
                .unwrap_or(0.0) as f32;
            for cut in ctx.cuts {
                h = cut.apply(x, z, h);
            }

            heights[r * cols + c] = h;
        }
    }

    // Compute normals via finite differences and emit vertices
    let base = output.vertices.len() as u32;

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

            output.vertices.push(Vertex {
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

            output.indices.push(i00);
            output.indices.push(i01);
            output.indices.push(i10);

            output.indices.push(i10);
            output.indices.push(i01);
            output.indices.push(i11);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_terrain_generates_grid_for_bounds() {
        let conv = CoordConverter::new(38.0, -122.0);
        let ctx = TerrainContext {
            conv: &conv,
            elevation: None,
            cuts: &[],
        };
        let mut verts = Vec::new();
        let mut idxs = Vec::new();
        let mut output = MeshOutput {
            vertices: &mut verts,
            indices: &mut idxs,
        };
        generate_terrain_for_world_rect(0.0, -100.0, 100.0, 0.0, 50.0, &ctx, &mut output);
        assert_eq!(verts.len(), 9);
        assert_eq!(idxs.len(), 24);
    }

    #[test]
    fn tunnel_cut_lowers_only_vertices_inside_portal_cut() {
        let conv = CoordConverter::new(38.0, -122.0);
        let cuts = [TerrainCut {
            start: (0.0, 0.0),
            end: (30.0, 0.0),
            half_width: 8.0,
            floor_y: -4.8,
            blend_width: 8.0,
        }];
        let ctx = TerrainContext {
            conv: &conv,
            elevation: None,
            cuts: &cuts,
        };
        let mut verts = Vec::new();
        let mut idxs = Vec::new();
        let mut output = MeshOutput {
            vertices: &mut verts,
            indices: &mut idxs,
        };

        generate_terrain_for_world_rect_with_cuts(0.0, -40.0, 80.0, 40.0, 10.0, &ctx, &mut output);

        let centre = verts
            .iter()
            .find(|v| (v.position[0] - 10.0).abs() < 1e-4 && v.position[2].abs() < 1e-4)
            .expect("grid contains centre cut sample");
        assert!(centre.position[1] <= -4.7);

        let outside = verts
            .iter()
            .find(|v| (v.position[0] - 10.0).abs() < 1e-4 && (v.position[2] - 30.0).abs() < 1e-4)
            .expect("grid contains outside sample");
        assert_eq!(outside.position[1], 0.0);
    }
}
