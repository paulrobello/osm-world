use wgpu::*;

use crate::camera::{SHADOW_CASCADE_COUNT, SHADOW_MAP_SIZE};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniforms {
    pub light_view_proj: [[[f32; 4]; 4]; SHADOW_CASCADE_COUNT],
    pub cascade_radii: [f32; 4],
    pub shadow_params: [f32; 4],
    pub shadow_pass_params: [u32; 4],
}

pub struct ShadowBindGroup {
    /// Layout used by the city pipeline's group(1) for shadow sampling (texture + sampler + uniform).
    pub layout: BindGroupLayout,
    /// Bind group for the city pipeline's group(1).
    pub group: BindGroup,
    /// Layout used by the shadow pipeline's group(0) for light VP only (uniform at binding 0).
    pub pass_layout: BindGroupLayout,
    /// Bind groups for each shadow pass cascade.
    pub pass_groups: [BindGroup; SHADOW_CASCADE_COUNT],
    pub uniform_buffer: Buffer,
    pass_uniform_buffers: [Buffer; SHADOW_CASCADE_COUNT],
    pub depth_texture: Texture,
    pub depth_array_view: TextureView,
    pub cascade_views: [TextureView; SHADOW_CASCADE_COUNT],
    pub sampler: Sampler,
}

impl ShadowBindGroup {
    pub fn new(device: &Device) -> Self {
        let uniform_buffer = create_light_uniform_buffer(device, "light uniform buffer");
        let pass_uniform_buffers = std::array::from_fn(|cascade| {
            create_light_uniform_buffer(device, &format!("shadow pass {cascade} uniform buffer"))
        });

        let depth_texture = device.create_texture(&TextureDescriptor {
            label: Some("shadow depth texture"),
            size: Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: SHADOW_CASCADE_COUNT as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_array_view = depth_texture.create_view(&TextureViewDescriptor {
            label: Some("shadow depth array view"),
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        });
        let cascade_views = std::array::from_fn(|layer| {
            depth_texture.create_view(&TextureViewDescriptor {
                label: Some("shadow cascade view"),
                dimension: Some(TextureViewDimension::D2),
                base_array_layer: layer as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("shadow comparison sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Nearest,
            compare: Some(CompareFunction::LessEqual),
            ..Default::default()
        });

        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("shadow bind group layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2Array,
                        sample_type: TextureSampleType::Depth,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Comparison),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("shadow bind group"),
            layout: &layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&depth_array_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // Shadow pass layout: uniform buffer at binding 0, visible to vertex stage only.
        let pass_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("shadow pass layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pass_groups = std::array::from_fn(|cascade| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("shadow pass bind group"),
                layout: &pass_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: pass_uniform_buffers[cascade].as_entire_binding(),
                }],
            })
        });

        Self {
            layout,
            group,
            pass_layout,
            pass_groups,
            uniform_buffer,
            pass_uniform_buffers,
            depth_texture,
            depth_array_view,
            cascade_views,
            sampler,
        }
    }

    pub fn update(&self, queue: &Queue, uniforms: &LightUniforms) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(uniforms)),
        );

        for (cascade_index, pass_buffer) in self.pass_uniform_buffers.iter().enumerate() {
            let mut pass_uniforms = *uniforms;
            pass_uniforms.shadow_pass_params[0] = cascade_index as u32;
            queue.write_buffer(
                pass_buffer,
                0,
                bytemuck::cast_slice(std::slice::from_ref(&pass_uniforms)),
            );
        }
    }
}

fn create_light_uniform_buffer(device: &Device, label: &str) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: std::mem::size_of::<LightUniforms>() as BufferAddress,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
