use bytemuck::{Pod, Zeroable};

/// GPU vertex format. 48 bytes per vertex.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub feature_type: f32,
    pub uv: [f32; 2],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x3,
        3 => Float32,
        4 => Float32x2,
    ];
}

pub mod feature {
    pub const TERRAIN: f32 = 0.0;
    pub const BUILDING: f32 = 1.0;
    pub const ROAD: f32 = 2.0;
    pub const WATER: f32 = 3.0;
    pub const LANDUSE: f32 = 4.0;
    pub const ROAD_MARKING: f32 = 5.0;
    pub const RAILWAY: f32 = 6.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_layout_includes_uvs() {
        assert_eq!(std::mem::size_of::<Vertex>(), 48);
        assert_eq!(Vertex::ATTRIBUTES.len(), 5);
    }
}
