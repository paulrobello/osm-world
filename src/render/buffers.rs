use wgpu::util::DeviceExt;
use wgpu::*;

use super::vertex::{Vertex, feature};

pub struct SceneBuffers {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
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

        Self {
            vertex_buffer,
            index_buffer,
            index_count,
        }
    }
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
            feature_type: feature::TERRAIN,
        },
        Vertex {
            position: [h, 0.0, -h],
            normal: n,
            color: c,
            feature_type: feature::TERRAIN,
        },
        Vertex {
            position: [h, 0.0, h],
            normal: n,
            color: c,
            feature_type: feature::TERRAIN,
        },
        Vertex {
            position: [-h, 0.0, h],
            normal: n,
            color: c,
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
