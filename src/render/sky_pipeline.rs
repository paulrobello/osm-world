use wgpu::*;

/// Concatenate sky_helpers.wgsl into the sky shader at the placeholder.
const SKY_HELPERS: &str = include_str!("../../shaders/sky_helpers.wgsl");

fn sky_shader_source() -> String {
    let mut source = String::with_capacity(8192);
    source.push_str(include_str!("../../shaders/sky.wgsl"));
    // Insert sky helpers at the first placeholder comment
    if let Some(pos) =
        source.find("// --- Sky helpers (loaded from sky_helpers.wgsl at compile time) ---")
    {
        source.replace_range(
            pos..pos
                + "// --- Sky helpers (loaded from sky_helpers.wgsl at compile time) ---".len(),
            SKY_HELPERS,
        );
    }
    // Insert fog helper at its placeholder comment
    if let Some(pos) =
        source.find("// --- Fog helpers (loaded from sky_helpers.wgsl at compile time) ---")
    {
        // The fog helper is already in SKY_HELPERS, so we only need to remove the placeholder
        source.replace_range(
            pos..pos
                + "// --- Fog helpers (loaded from sky_helpers.wgsl at compile time) ---".len(),
            "",
        );
    }
    source
}

pub struct SkyPipeline {
    pub pipeline: RenderPipeline,
}

impl SkyPipeline {
    pub fn new(
        device: &Device,
        scene_layout: &BindGroupLayout,
        surface_format: TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("sky shader"),
            source: ShaderSource::Wgsl(sky_shader_source().into()),
        });

        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("sky pipeline layout"),
            bind_group_layouts: &[Some(scene_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("sky render pipeline"),
            layout: Some(&layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_sky"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(CompareFunction::LessEqual),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Self { pipeline }
    }
}
