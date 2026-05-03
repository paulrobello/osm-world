use wgpu::*;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniforms {
    pub light_view_proj: [[f32; 4]; 4],
}

pub struct ShadowBindGroup {
    /// Layout used by the city pipeline's group(1) for shadow sampling (texture + sampler + uniform).
    pub layout: BindGroupLayout,
    /// Bind group for the city pipeline's group(1).
    pub group: BindGroup,
    /// Layout used by the shadow pipeline's group(0) for light VP only (uniform at binding 0).
    pub pass_layout: BindGroupLayout,
    /// Bind group for the shadow pipeline's group(0).
    pub pass_group: BindGroup,
    pub uniform_buffer: Buffer,
    pub depth_texture: Texture,
    pub depth_view: TextureView,
    pub sampler: Sampler,
}

impl ShadowBindGroup {
    pub fn new(device: &Device) -> Self {
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("light uniform buffer"),
            size: std::mem::size_of::<LightUniforms>() as BufferAddress,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let depth_texture = device.create_texture(&TextureDescriptor {
            label: Some("shadow depth texture"),
            size: Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&TextureViewDescriptor::default());

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
                        view_dimension: TextureViewDimension::D2,
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
                    resource: BindingResource::TextureView(&depth_view),
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

        let pass_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("shadow pass bind group"),
            layout: &pass_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            layout,
            group,
            pass_layout,
            pass_group,
            uniform_buffer,
            depth_texture,
            depth_view,
            sampler,
        }
    }

    pub fn update(&self, queue: &Queue, uniforms: &LightUniforms) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(uniforms)),
        );
    }
}
