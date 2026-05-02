use wgpu::*;

use crate::camera::SceneUniforms;

pub struct SceneBindGroup {
    pub layout: BindGroupLayout,
    pub group: BindGroup,
    pub buffer: Buffer,
}

impl SceneBindGroup {
    pub fn new(device: &Device) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("scene uniform buffer"),
            size: std::mem::size_of::<SceneUniforms>() as BufferAddress,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("scene bind group layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("scene bind group"),
            layout: &layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            layout,
            group,
            buffer,
        }
    }

    pub fn update(&self, queue: &Queue, uniforms: &SceneUniforms) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(uniforms)),
        );
    }
}
