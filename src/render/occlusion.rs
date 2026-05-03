use wgpu::util::DeviceExt;
use wgpu::*;

/// Manages hardware occlusion queries for tile culling.
pub struct OcclusionQueries {
    pub query_set: QuerySet,
    pub result_buffer: Buffer,
    pub cube_vertices: Buffer,
    pub cube_indices: Buffer,
    pub query_count: u32,
}

impl OcclusionQueries {
    pub fn new(device: &Device, max_queries: u32) -> Self {
        let query_set = device.create_query_set(&QuerySetDescriptor {
            label: Some("occlusion query set"),
            ty: QueryType::Occlusion,
            count: max_queries,
        });

        let result_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("occlusion result buffer"),
            size: (max_queries as u64) * std::mem::size_of::<u64>() as u64,
            usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Unit cube: 8 vertices, 12 triangles (36 indices)
        let cube_verts: [[f32; 3]; 8] = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let cube_indices: [u32; 36] = [
            0, 1, 2, 0, 2, 3, // front
            4, 6, 5, 4, 7, 6, // back
            0, 4, 5, 0, 5, 1, // bottom
            2, 7, 3, 2, 6, 7, // top
            0, 3, 7, 0, 7, 4, // left
            1, 5, 6, 1, 6, 2, // right
        ];

        let cube_vertices = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("occlusion cube vertices"),
            contents: bytemuck::cast_slice(&cube_verts),
            usage: BufferUsages::VERTEX,
        });

        let cube_indices = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("occlusion cube indices"),
            contents: bytemuck::cast_slice(&cube_indices),
            usage: BufferUsages::INDEX,
        });

        Self {
            query_set,
            result_buffer,
            cube_vertices,
            cube_indices,
            query_count: max_queries,
        }
    }
}
