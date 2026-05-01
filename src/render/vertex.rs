use bytemuck::{Pod, Zeroable};

/// GPU vertex format. 32 bytes per vertex.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub feature_type: f32,
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x3,
        3 => Float32,
    ];
}

pub mod feature {
    pub const TERRAIN: f32 = 0.0;
    pub const BUILDING: f32 = 1.0;
    pub const ROAD: f32 = 2.0;
    pub const WATER: f32 = 3.0;
    pub const LANDUSE: f32 = 4.0;
}
