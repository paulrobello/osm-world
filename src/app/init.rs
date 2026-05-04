use std::sync::Arc;
use wgpu::*;

use crate::camera::Flycam;
use crate::render::bind_groups::SceneBindGroup;
use crate::render::buffers::SceneBuffers;
use crate::render::contact_shadow::ContactShadowPass;
use crate::render::minimap::MinimapTarget;
use crate::render::occlusion::OcclusionQueries;
use crate::render::pipelines::CityPipeline;
use crate::render::shadow_bind_group::ShadowBindGroup;
use crate::render::shadow_pipeline::ShadowPipeline;
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
    pub coord_converter: Option<crate::geo::CoordConverter>,
    pub camera_bg: SceneBindGroup,
    pub pipeline: CityPipeline,
    pub sky_pipeline: SkyPipeline,
    pub shadow_bg: ShadowBindGroup,
    pub shadow_pipeline: ShadowPipeline,
    pub contact_shadow: ContactShadowPass,
    pub scene: SceneBuffers,
    pub occlusion: OcclusionQueries,
    pub minimap_target: MinimapTarget,
}

pub fn init_wgpu(
    event_loop: &winit::event_loop::ActiveEventLoop,
    window_width: f64,
    window_height: f64,
    input_path: Option<&str>,
    srtm_dir: Option<&str>,
    cam_override: Option<&crate::camera::CameraOverride>,
) -> anyhow::Result<(AppState, crate::ui::EguiState)> {
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

    let egui = crate::ui::EguiState::new(&device, &surface_config, &window);

    let mut camera = Flycam::new(surface_config.width as f32 / surface_config.height as f32);
    let spawn_lat_lon = camera_spawn_lat_lon(cam_override)?;
    let camera_bg = SceneBindGroup::new(&device);
    let shadow_bg = ShadowBindGroup::new(&device);
    let pipeline = CityPipeline::new(
        &device,
        &camera_bg.layout,
        &shadow_bg.layout,
        surface_format,
    );
    let sky_pipeline = SkyPipeline::new(&device, &camera_bg.layout, surface_format);
    let shadow_pipeline = ShadowPipeline::new(&device, &shadow_bg.pass_layout);
    let contact_shadow = ContactShadowPass::new(
        &device,
        &camera_bg.layout,
        surface_format,
        surface_config.width,
        surface_config.height,
        &depth_view,
    );
    let occlusion = OcclusionQueries::new(&device, 256);
    let minimap_target = MinimapTarget::new(&device, surface_format);

    let (scene, coord_converter) = match input_path {
        Some(path) => {
            let srtm = srtm_dir.map(std::path::Path::new);
            let source = crate::world::loader::load_world_source(std::path::Path::new(path), srtm)?;
            let coord_converter = source.conv;
            let world = crate::world::loader::generate_world_mesh(&source);
            camera.position = glam::Vec3::new(5645.5, 122.8, -10505.8);
            camera.yaw = (-124.80_f32).to_radians();
            camera.pitch = (-16.30_f32).to_radians();
            if let Some((spawn_lat, spawn_lon)) = spawn_lat_lon {
                apply_spawn_camera_location(&mut camera, &source.conv, spawn_lat, spawn_lon);
            }
            (
                SceneBuffers::from_mesh(&device, world.vertices, world.indices),
                Some(coord_converter),
            )
        }
        None => (SceneBuffers::new(&device), None),
    };

    if let Some(ov) = cam_override {
        if let Some(x) = ov.x {
            camera.position.x = x;
        }
        if let Some(y) = ov.y {
            camera.position.y = y;
        }
        if let Some(z) = ov.z {
            camera.position.z = z;
        }
        if let Some(yaw) = ov.yaw {
            camera.yaw = yaw.to_radians();
        }
        if let Some(pitch) = ov.pitch {
            camera.pitch = pitch.to_radians();
        }
    }

    Ok((
        AppState {
            window,
            device,
            queue,
            surface,
            surface_config,
            depth_texture,
            depth_view,
            camera,
            coord_converter,
            camera_bg,
            pipeline,
            sky_pipeline,
            shadow_bg,
            shadow_pipeline,
            contact_shadow,
            scene,
            occlusion,
            minimap_target,
        },
        egui,
    ))
}

fn camera_spawn_lat_lon(
    cam_override: Option<&crate::camera::CameraOverride>,
) -> anyhow::Result<Option<(f64, f64)>> {
    let Some(ov) = cam_override else {
        return Ok(None);
    };

    match (ov.spawn_lat, ov.spawn_lon) {
        (Some(lat), Some(lon)) => Ok(Some((lat, lon))),
        (None, None) => Ok(None),
        _ => anyhow::bail!("--spawn-lat and --spawn-lon must be provided together"),
    }
}

fn apply_spawn_camera_location(
    camera: &mut Flycam,
    conv: &crate::geo::CoordConverter,
    lat: f64,
    lon: f64,
) {
    let (x, z) = conv.to_world_xz(lat, lon);
    camera.position.x = x;
    camera.position.z = z;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_lat_lon_requires_complete_pair() {
        let lat_only = crate::camera::CameraOverride {
            x: None,
            y: None,
            z: None,
            yaw: None,
            pitch: None,
            spawn_lat: Some(38.65671),
            spawn_lon: None,
        };
        let lon_only = crate::camera::CameraOverride {
            x: None,
            y: None,
            z: None,
            yaw: None,
            pitch: None,
            spawn_lat: None,
            spawn_lon: Some(-121.72179),
        };

        assert!(camera_spawn_lat_lon(Some(&lat_only)).is_err());
        assert!(camera_spawn_lat_lon(Some(&lon_only)).is_err());
    }

    #[test]
    fn apply_spawn_camera_location_sets_xz_and_preserves_y() {
        let conv = crate::geo::CoordConverter::new(38.63863, -121.7526);
        let mut camera = Flycam::new(1.0);
        camera.position.y = 122.8;
        let (expected_x, expected_z) = conv.to_world_xz(38.65671, -121.72179);

        apply_spawn_camera_location(&mut camera, &conv, 38.65671, -121.72179);

        assert_eq!(camera.position.x, expected_x);
        assert_eq!(camera.position.y, 122.8);
        assert_eq!(camera.position.z, expected_z);
    }
}
