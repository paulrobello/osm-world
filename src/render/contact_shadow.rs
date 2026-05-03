use wgpu::util::DeviceExt;
use wgpu::*;

use crate::camera::{CONTACT_SHADOW_MAX_DISTANCE, CONTACT_SHADOW_STRENGTH};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ContactShadowUniforms {
    max_distance: f32,
    strength: f32,
    _pad: [f32; 2],
}

pub struct ContactShadowPass {
    pub pipeline: RenderPipeline,
    pub layout: BindGroupLayout,
    pub bind_group: BindGroup,
    pub color_texture: Texture,
    pub color_view: TextureView,
    pub sampler: Sampler,
    pub uniform_buffer: Buffer,
    surface_format: TextureFormat,
}

impl ContactShadowPass {
    pub fn new(
        device: &Device,
        scene_layout: &BindGroupLayout,
        surface_format: TextureFormat,
        width: u32,
        height: u32,
        depth_view: &TextureView,
    ) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("contact shadow shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/contact_shadow.wgsl").into()),
        });

        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("contact shadow bind group layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Depth,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("contact shadow pipeline layout"),
            bind_group_layouts: &[Some(scene_layout), Some(&layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("contact shadow pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("contact shadow sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("contact shadow uniform buffer"),
            contents: bytemuck::cast_slice(&[ContactShadowUniforms {
                max_distance: CONTACT_SHADOW_MAX_DISTANCE,
                strength: CONTACT_SHADOW_STRENGTH,
                _pad: [0.0; 2],
            }]),
            usage: BufferUsages::UNIFORM,
        });

        let (color_texture, color_view) =
            create_color_target(device, surface_format, width, height);
        let bind_group = create_bind_group(
            device,
            &layout,
            &color_view,
            depth_view,
            &sampler,
            &uniform_buffer,
        );

        Self {
            pipeline,
            layout,
            bind_group,
            color_texture,
            color_view,
            sampler,
            uniform_buffer,
            surface_format,
        }
    }

    pub fn resize(&mut self, device: &Device, width: u32, height: u32, depth_view: &TextureView) {
        let (color_texture, color_view) =
            create_color_target(device, self.surface_format, width, height);
        let bind_group = create_bind_group(
            device,
            &self.layout,
            &color_view,
            depth_view,
            &self.sampler,
            &self.uniform_buffer,
        );

        self.color_texture = color_texture;
        self.color_view = color_view;
        self.bind_group = bind_group;
    }
}

fn create_color_target(
    device: &Device,
    surface_format: TextureFormat,
    width: u32,
    height: u32,
) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("contact shadow color target"),
        size: Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: surface_format,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}

fn create_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    color_view: &TextureView,
    depth_view: &TextureView,
    sampler: &Sampler,
    uniform_buffer: &Buffer,
) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some("contact shadow bind group"),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(color_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(depth_view),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::Sampler(sampler),
            },
            BindGroupEntry {
                binding: 3,
                resource: uniform_buffer.as_entire_binding(),
            },
        ],
    })
}
