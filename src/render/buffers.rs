use wgpu::util::DeviceExt;
use wgpu::*;

use super::vertex::{Vertex, feature};

pub struct SceneBuffers {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
    pub shadow_index_buffer: Buffer,
    pub shadow_index_count: u32,
}

impl SceneBuffers {
    pub fn new(device: &Device) -> Self {
        let (vertices, indices) = generate_test_scene();
        Self::from_data(device, vertices, indices)
    }

    pub fn from_mesh(device: &Device, vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self::from_data(device, vertices, indices)
    }

    fn from_data(device: &Device, vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        let index_count = indices.len() as u32;
        let shadow_indices = shadow_index_data(&vertices, &indices);

        let vertex_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("scene vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("scene index buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });

        let shadow_index_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("shadow caster index buffer"),
            contents: bytemuck::cast_slice(&shadow_indices.buffer_indices),
            usage: BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count,
            shadow_index_buffer,
            shadow_index_count: shadow_indices.draw_count,
        }
    }
}

struct ShadowIndexData {
    buffer_indices: Vec<u32>,
    draw_count: u32,
}

fn shadow_index_data(vertices: &[Vertex], indices: &[u32]) -> ShadowIndexData {
    let buffer_indices = shadow_casting_indices(vertices, indices);
    let draw_count = buffer_indices.len() as u32;

    ShadowIndexData {
        buffer_indices: if buffer_indices.is_empty() {
            vec![0]
        } else {
            buffer_indices
        },
        draw_count,
    }
}

fn shadow_casting_indices(vertices: &[Vertex], indices: &[u32]) -> Vec<u32> {
    debug_assert_eq!(indices.len() % 3, 0, "scene indices must be triangle lists");

    // Receiver surfaces (terrain/roads/water/landuse) are intentionally omitted:
    // near-coplanar receiver geometry in the depth map causes map-wide self-shadowing.
    indices
        .chunks_exact(3)
        .filter(|tri| {
            tri.iter().all(|&index| {
                vertices
                    .get(index as usize)
                    .is_some_and(|vertex| vertex.feature_type == feature::BUILDING)
            })
        })
        .flatten()
        .copied()
        .collect()
}

fn generate_test_scene() -> (Vec<Vertex>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();

    append_ground_plane(&mut verts, &mut idxs, 2000.0);
    append_box(
        &mut verts,
        &mut idxs,
        -10.0,
        10.0,
        0.0,
        15.0,
        -15.0,
        15.0,
        [0.85, 0.78, 0.65],
        feature::BUILDING,
    );

    (verts, idxs)
}

fn append_ground_plane(verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>, size: f32) {
    let base = verts.len() as u32;
    let h = size / 2.0;
    let n = [0.0, 1.0, 0.0];
    let c = [0.35, 0.55, 0.25];
    verts.extend_from_slice(&[
        Vertex {
            position: [-h, 0.0, -h],
            normal: n,
            color: c,
            uv: [0.0, 0.0],
            feature_type: feature::TERRAIN,
        },
        Vertex {
            position: [h, 0.0, -h],
            normal: n,
            color: c,
            uv: [0.0, 0.0],
            feature_type: feature::TERRAIN,
        },
        Vertex {
            position: [h, 0.0, h],
            normal: n,
            color: c,
            uv: [0.0, 0.0],
            feature_type: feature::TERRAIN,
        },
        Vertex {
            position: [-h, 0.0, h],
            normal: n,
            color: c,
            uv: [0.0, 0.0],
            feature_type: feature::TERRAIN,
        },
    ]);
    idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

#[allow(clippy::too_many_arguments)]
fn append_box(
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    z0: f32,
    z1: f32,
    color: [f32; 3],
    feature_type: f32,
) {
    let base = verts.len() as u32;
    let v = |px: f32, py: f32, pz: f32, nx: f32, ny: f32, nz: f32| Vertex {
        position: [px, py, pz],
        normal: [nx, ny, nz],
        color,
        uv: [0.0, 0.0],
        feature_type,
    };

    // Front (z+)
    verts.extend_from_slice(&[
        v(x0, y0, z1, 0.0, 0.0, 1.0),
        v(x1, y0, z1, 0.0, 0.0, 1.0),
        v(x1, y1, z1, 0.0, 0.0, 1.0),
        v(x0, y1, z1, 0.0, 0.0, 1.0),
    ]);
    // Back (z-)
    verts.extend_from_slice(&[
        v(x1, y0, z0, 0.0, 0.0, -1.0),
        v(x0, y0, z0, 0.0, 0.0, -1.0),
        v(x0, y1, z0, 0.0, 0.0, -1.0),
        v(x1, y1, z0, 0.0, 0.0, -1.0),
    ]);
    // Right (x+)
    verts.extend_from_slice(&[
        v(x1, y0, z1, 1.0, 0.0, 0.0),
        v(x1, y0, z0, 1.0, 0.0, 0.0),
        v(x1, y1, z0, 1.0, 0.0, 0.0),
        v(x1, y1, z1, 1.0, 0.0, 0.0),
    ]);
    // Left (x-)
    verts.extend_from_slice(&[
        v(x0, y0, z0, -1.0, 0.0, 0.0),
        v(x0, y0, z1, -1.0, 0.0, 0.0),
        v(x0, y1, z1, -1.0, 0.0, 0.0),
        v(x0, y1, z0, -1.0, 0.0, 0.0),
    ]);
    // Top (y+)
    verts.extend_from_slice(&[
        v(x0, y1, z1, 0.0, 1.0, 0.0),
        v(x1, y1, z1, 0.0, 1.0, 0.0),
        v(x1, y1, z0, 0.0, 1.0, 0.0),
        v(x0, y1, z0, 0.0, 1.0, 0.0),
    ]);
    // Bottom (y-)
    verts.extend_from_slice(&[
        v(x0, y0, z0, 0.0, -1.0, 0.0),
        v(x1, y0, z0, 0.0, -1.0, 0.0),
        v(x1, y0, z1, 0.0, -1.0, 0.0),
        v(x0, y0, z1, 0.0, -1.0, 0.0),
    ]);

    for face in 0..6u32 {
        let b = base + face * 4;
        idxs.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(feature_type: f32) -> Vertex {
        Vertex {
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            color: [1.0; 3],
            uv: [0.0, 0.0],
            feature_type,
        }
    }

    #[test]
    fn shadow_indices_keep_only_building_triangles() {
        let vertices = vec![
            vertex(feature::TERRAIN),
            vertex(feature::TERRAIN),
            vertex(feature::TERRAIN),
            vertex(feature::BUILDING),
            vertex(feature::BUILDING),
            vertex(feature::BUILDING),
            vertex(feature::ROAD),
            vertex(feature::ROAD),
            vertex(feature::ROAD),
        ];
        let indices = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];

        assert_eq!(shadow_casting_indices(&vertices, &indices), vec![3, 4, 5]);
    }

    #[test]
    fn shadow_indices_drop_mixed_receiver_and_caster_triangles() {
        let vertices = vec![
            vertex(feature::TERRAIN),
            vertex(feature::BUILDING),
            vertex(feature::BUILDING),
        ];
        let indices = vec![0, 1, 2];

        assert!(shadow_casting_indices(&vertices, &indices).is_empty());
    }

    #[test]
    fn shadow_index_data_keeps_zero_draw_count_for_receiver_only_meshes() {
        let vertices = vec![
            vertex(feature::TERRAIN),
            vertex(feature::TERRAIN),
            vertex(feature::TERRAIN),
        ];
        let indices = vec![0, 1, 2];

        let data = shadow_index_data(&vertices, &indices);

        assert_eq!(data.draw_count, 0);
        assert_eq!(data.buffer_indices, vec![0]);
    }
}
