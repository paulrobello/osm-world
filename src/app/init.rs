use std::sync::Arc;
use wgpu::*;

use crate::camera::Flycam;
use crate::render::bind_groups::SceneBindGroup;
use crate::render::buffers::SceneBuffers;
use crate::render::pipelines::CityPipeline;
use crate::render::sky_pipeline::SkyPipeline;

pub struct AppState {
    pub window: Arc<winit::window::Window>,
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    pub depth_texture: Texture,
    pub depth_view: TextureView,
    pub camera: Flycam,
    pub camera_bg: SceneBindGroup,
    pub pipeline: CityPipeline,
    pub sky_pipeline: SkyPipeline,
    pub scene: SceneBuffers,
}

pub fn init_wgpu(
    event_loop: &winit::event_loop::ActiveEventLoop,
    window_width: f64,
    window_height: f64,
    input_path: Option<&str>,
    srtm_dir: Option<&str>,
) -> anyhow::Result<AppState> {
    let window = Arc::new(
        event_loop.create_window(
            winit::window::WindowAttributes::default()
                .with_title("osm-world")
                .with_inner_size(winit::dpi::LogicalSize::new(window_width, window_height)),
        )?,
    );

    let instance = Instance::new(InstanceDescriptor::new_with_display_handle(Box::new(
        event_loop.owned_display_handle(),
    )));

    let surface = instance.create_surface(Arc::clone(&window))?;
    let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .map_err(|e| anyhow::anyhow!("no suitable GPU adapter found: {e}"))?;

    let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
        label: Some("osm-world device"),
        required_features: Features::empty(),
        required_limits: Limits::default(),
        memory_hints: MemoryHints::Performance,
        trace: Trace::Off,
        experimental_features: ExperimentalFeatures::default(),
    }))?;

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|f| !f.is_srgb())
        .copied()
        .unwrap_or(surface_caps.formats[0]);

    let size = window.inner_size();
    let surface_config = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        format: surface_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: PresentMode::AutoVsync,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_config);

    let (depth_texture, depth_view) =
        create_depth_buffer(&device, surface_config.width, surface_config.height);

    let mut camera = Flycam::new(surface_config.width as f32 / surface_config.height as f32);
    let camera_bg = SceneBindGroup::new(&device);
    let pipeline = CityPipeline::new(&device, &camera_bg.layout, surface_format);
    let sky_pipeline = SkyPipeline::new(&device, &camera_bg.layout, surface_format);

    let scene = match input_path {
        Some(path) => {
            let srtm = srtm_dir.map(std::path::Path::new);
            let world = crate::world::loader::load_world(std::path::Path::new(path), srtm)?;
            camera.position = glam::Vec3::new(world.center.0, world.center.1, world.center.2);
            camera.yaw = -std::f32::consts::FRAC_PI_2;
            camera.pitch = -0.3;
            SceneBuffers::from_mesh(&device, world.vertices, world.indices)
        }
        None => SceneBuffers::new(&device),
    };

    Ok(AppState {
        window,
        device,
        queue,
        surface,
        surface_config,
        depth_texture,
        depth_view,
        camera,
        camera_bg,
        pipeline,
        sky_pipeline,
        scene,
    })
}

pub fn create_depth_buffer(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("depth texture"),
        size: Extent3d {
            width,
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Depth32Float,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}
