//! WGPU initialization, scene loading, and camera placement logic.
//!
//! Contains [`init_wgpu`] which creates the window, GPU device, render pipelines,
//! and loads the initial scene. Also provides [`load_scene_resources`] for
//! hot-swapping the scene at runtime through the area-switch feature.

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

/// Scene data produced after parsing and mesh generation.
///
/// Holds the GPU buffers alongside derived data for labels, search, inspection,
/// and tile debug overlays.
pub struct LoadedScene {
    pub scene: SceneBuffers,
    pub coord_converter: Option<crate::geo::CoordConverter>,
    pub poi_labels: Vec<crate::ui::poi_labels::PoiLabel>,
    pub address_labels: Vec<crate::ui::poi_labels::PoiLabel>,
    pub street_sign_labels: Vec<crate::ui::poi_labels::StreetSignLabel>,
    pub search_entries: Vec<crate::ui::search::SearchEntry>,
    pub identifiables: Vec<crate::ui::inspect::IdentifiableFeature>,
    pub tile_debug_entries: Vec<crate::stream::TileDebugEntry>,
}

/// Initialized WGPU state including device, surface, pipelines, scene buffers,
/// camera, and UI-derived data.
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
    pub poi_labels: Vec<crate::ui::poi_labels::PoiLabel>,
    pub address_labels: Vec<crate::ui::poi_labels::PoiLabel>,
    pub street_sign_labels: Vec<crate::ui::poi_labels::StreetSignLabel>,
    pub search_entries: Vec<crate::ui::search::SearchEntry>,
    pub identifiables: Vec<crate::ui::inspect::IdentifiableFeature>,
    pub camera_bg: SceneBindGroup,
    pub pipeline: CityPipeline,
    pub sky_pipeline: SkyPipeline,
    pub shadow_bg: ShadowBindGroup,
    pub shadow_pipeline: ShadowPipeline,
    pub contact_shadow: ContactShadowPass,
    pub scene: SceneBuffers,
    pub occlusion: OcclusionQueries,
    pub minimap_target: MinimapTarget,
    pub tile_debug_entries: Vec<crate::stream::TileDebugEntry>,
    pub tile_debug_tile_size: f32,
}

/// Options for WGPU initialization, passed from [`AppOptions`](super::AppOptions).
pub struct InitWgpuOptions<'a> {
    pub window_width: f64,
    pub window_height: f64,
    pub input_path: Option<&'a str>,
    pub srtm_dir: Option<&'a str>,
    pub cam_override: Option<&'a crate::camera::CameraOverride>,
    pub persisted_camera: Option<&'a crate::app::prefs::CameraPrefs>,
    pub visual_detail: &'a crate::visual_detail::VisualDetailSettings,
    pub streaming: &'a crate::app::StreamingOptions,
}

/// Initializes the WGPU renderer: creates the window, GPU device, surface,
/// depth buffer, all render pipelines, bind groups, and optionally loads a
/// scene from an input file.
///
/// Returns the initialized [`AppState`] and egui integration state.
pub fn init_wgpu(
    event_loop: &winit::event_loop::ActiveEventLoop,
    options: &InitWgpuOptions<'_>,
) -> anyhow::Result<(AppState, crate::ui::EguiState)> {
    let window = Arc::new(
        event_loop.create_window(
            winit::window::WindowAttributes::default()
                .with_title("osm-world")
                .with_inner_size(winit::dpi::LogicalSize::new(
                    options.window_width,
                    options.window_height,
                )),
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
    let spawn_lat_lon = camera_spawn_lat_lon(options.cam_override)?;
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

    let loaded_scene = match options.input_path {
        Some(path) => {
            let srtm = options.srtm_dir.map(std::path::Path::new);
            let source = crate::world::loader::load_world_source_with_visual_detail(
                std::path::Path::new(path),
                srtm,
                options.visual_detail,
            )?;
            apply_default_input_camera(&mut camera);
            apply_persisted_camera_if_allowed(
                &mut camera,
                options.persisted_camera,
                options.cam_override,
                source.world_bbox(),
            );
            if let Some((spawn_lat, spawn_lon)) = spawn_lat_lon {
                apply_spawn_camera_location(&mut camera, &source.conv, spawn_lat, spawn_lon);
            }
            apply_explicit_camera_overrides(&mut camera, options.cam_override);
            loaded_scene_from_source(
                &device,
                source,
                options.visual_detail,
                options.streaming,
                camera.position,
            )?
        }
        None => LoadedScene {
            scene: SceneBuffers::new(&device),
            coord_converter: None,
            poi_labels: Vec::new(),
            address_labels: Vec::new(),
            street_sign_labels: Vec::new(),
            search_entries: Vec::new(),
            identifiables: Vec::new(),
            tile_debug_entries: Vec::new(),
        },
    };

    apply_explicit_camera_overrides(&mut camera, options.cam_override);

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
            coord_converter: loaded_scene.coord_converter,
            poi_labels: loaded_scene.poi_labels,
            address_labels: loaded_scene.address_labels,
            street_sign_labels: loaded_scene.street_sign_labels,
            search_entries: loaded_scene.search_entries,
            identifiables: loaded_scene.identifiables,
            camera_bg,
            pipeline,
            sky_pipeline,
            shadow_bg,
            shadow_pipeline,
            contact_shadow,
            scene: loaded_scene.scene,
            occlusion,
            minimap_target,
            tile_debug_entries: loaded_scene.tile_debug_entries,
            tile_debug_tile_size: options.streaming.tile_size,
        },
        egui,
    ))
}

/// Loads scene resources from an input file at runtime.
///
/// Used by the area-switch feature to hot-swap the scene without restarting.
pub fn load_scene_resources(
    device: &Device,
    input_path: &std::path::Path,
    srtm_dir: Option<&std::path::Path>,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
    streaming: &crate::app::StreamingOptions,
    camera_position: glam::Vec3,
) -> anyhow::Result<LoadedScene> {
    let source = crate::world::loader::load_world_source_with_visual_detail(
        input_path,
        srtm_dir,
        visual_detail,
    )?;
    loaded_scene_from_source(device, source, visual_detail, streaming, camera_position)
}

fn loaded_scene_from_source(
    device: &Device,
    source: crate::world::loader::WorldSource,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
    streaming: &crate::app::StreamingOptions,
    camera_position: glam::Vec3,
) -> anyhow::Result<LoadedScene> {
    let coord_converter = source.conv;
    let poi_labels = crate::ui::poi_labels::labels_from_point_features(&source.point_features);
    let address_labels = crate::ui::poi_labels::labels_from_address_features(
        &source.buildings,
        &source.address_points,
    );
    let street_sign_labels = crate::ui::poi_labels::labels_from_street_signs(&source.street_signs);
    let search_entries = crate::ui::search::build_search_index(&source);
    let identifiables = crate::ui::inspect::build_identifiables(&source);
    let (scene, tile_debug_entries) =
        scene_buffers_from_source(device, &source, visual_detail, streaming, camera_position)?;
    Ok(LoadedScene {
        scene,
        coord_converter: Some(coord_converter),
        poi_labels,
        address_labels,
        street_sign_labels,
        search_entries,
        identifiables,
        tile_debug_entries,
    })
}

fn scene_buffers_from_source(
    device: &Device,
    source: &crate::world::loader::WorldSource,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
    streaming: &crate::app::StreamingOptions,
    camera_position: glam::Vec3,
) -> anyhow::Result<(SceneBuffers, Vec<crate::stream::TileDebugEntry>)> {
    let (vertices, indices, debug_entries) = if streaming.enabled {
        let result =
            streaming_mesh_for_camera(source, visual_detail, streaming, camera_position, device);
        if result.0.is_empty() {
            log::warn!("Streaming selected 0 tiles near camera — falling back to full mesh");
            let world =
                crate::world::loader::generate_world_mesh_with_visual_detail(source, visual_detail);
            let debug_entries = tile_debug_entries_from_source(source, streaming.tile_size);
            (world.vertices, world.indices, debug_entries)
        } else {
            result
        }
    } else {
        let world =
            crate::world::loader::generate_world_mesh_with_visual_detail(source, visual_detail);
        let debug_entries = tile_debug_entries_from_source(source, streaming.tile_size);
        (world.vertices, world.indices, debug_entries)
    };

    validate_scene_buffer_sizes(device, vertices.len(), indices.len())?;
    Ok((
        SceneBuffers::from_mesh(device, vertices, indices),
        debug_entries,
    ))
}

fn validate_scene_buffer_sizes(
    device: &Device,
    vertex_count: usize,
    index_count: usize,
) -> anyhow::Result<()> {
    let vertex_bytes =
        vertex_count.saturating_mul(std::mem::size_of::<crate::render::vertex::Vertex>());
    let index_bytes = index_count.saturating_mul(std::mem::size_of::<u32>());
    let max_buffer_size = device.limits().max_buffer_size as usize;

    if vertex_bytes > max_buffer_size {
        anyhow::bail!(
            "scene vertex buffer would be {:.1} MiB, exceeding this GPU's {:.1} MiB buffer limit; enable streaming or reduce the rendered area/detail",
            bytes_to_mib(vertex_bytes),
            bytes_to_mib(max_buffer_size),
        );
    }
    if index_bytes > max_buffer_size {
        anyhow::bail!(
            "scene index buffer would be {:.1} MiB, exceeding this GPU's {:.1} MiB buffer limit; enable streaming or reduce the rendered area/detail",
            bytes_to_mib(index_bytes),
            bytes_to_mib(max_buffer_size),
        );
    }
    Ok(())
}

fn streaming_mesh_for_camera(
    source: &crate::world::loader::WorldSource,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
    streaming: &crate::app::StreamingOptions,
    camera_position: glam::Vec3,
    device: &Device,
) -> (
    Vec<crate::render::vertex::Vertex>,
    Vec<u32>,
    Vec<crate::stream::TileDebugEntry>,
) {
    let index = source.feature_index_for_tile_size(streaming.tile_size);
    let selected = select_streaming_tiles(
        &index,
        streaming.tile_size,
        camera_position,
        streaming.stream_radius,
        streaming.max_uploaded_tiles,
    );
    let vertex_budget = device.limits().max_buffer_size as usize;
    let index_budget = device.limits().max_buffer_size as usize;
    let mut selected_coords: Vec<_> = selected.iter().map(|(coord, _)| *coord).collect();
    let requested_tile_count = selected_coords.len();

    let mut mesh = crate::world::loader::generate_streamed_startup_mesh(
        source,
        &selected_coords,
        streaming.tile_size,
        visual_detail,
    );
    while selected_coords.len() > 1
        && (mesh
            .vertices
            .len()
            .saturating_mul(std::mem::size_of::<crate::render::vertex::Vertex>())
            > vertex_budget
            || mesh
                .indices
                .len()
                .saturating_mul(std::mem::size_of::<u32>())
                > index_budget)
    {
        selected_coords.truncate((selected_coords.len() * 3 / 4).max(1));
        mesh = crate::world::loader::generate_streamed_startup_mesh(
            source,
            &selected_coords,
            streaming.tile_size,
            visual_detail,
        );
    }

    let debug_entries = selected_coords
        .iter()
        .copied()
        .map(|coord| crate::stream::TileDebugEntry {
            coord,
            state: crate::stream::TileDebugState::Uploaded,
        })
        .collect::<Vec<_>>();
    let skipped_for_budget = requested_tile_count.saturating_sub(debug_entries.len());

    log::info!(
        "Generated streamed startup mesh: {} vertices, {} indices, {} tiles loaded{}",
        mesh.vertices.len(),
        mesh.indices.len(),
        debug_entries.len(),
        if skipped_for_budget == 0 {
            String::new()
        } else {
            format!(", {skipped_for_budget} skipped for GPU buffer budget")
        }
    );

    (mesh.vertices, mesh.indices, debug_entries)
}

fn select_streaming_tiles(
    index: &std::collections::HashMap<
        crate::stream::TileCoord,
        crate::stream::tile::TileFeatureRefs,
    >,
    tile_size: f32,
    camera_position: glam::Vec3,
    stream_radius: f32,
    max_tiles: usize,
) -> Vec<(crate::stream::TileCoord, f32)> {
    let mut selected: Vec<_> = index
        .keys()
        .copied()
        .map(|coord| {
            let center = coord.center(tile_size);
            let delta = glam::vec2(center.x - camera_position.x, center.z - camera_position.z);
            (coord, delta.length())
        })
        .filter(|(_, distance)| *distance <= stream_radius)
        .collect();

    selected.sort_by(|(a_coord, a_distance), (b_coord, b_distance)| {
        a_distance
            .total_cmp(b_distance)
            .then_with(|| a_coord.cmp(b_coord))
    });
    selected.truncate(max_tiles.max(1));
    selected
}

fn bytes_to_mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn tile_debug_entries_from_source(
    source: &crate::world::loader::WorldSource,
    tile_size: f32,
) -> Vec<crate::stream::TileDebugEntry> {
    let mut coords: Vec<_> = source
        .feature_index_for_tile_size(tile_size)
        .keys()
        .copied()
        .collect();
    coords.sort_unstable();
    coords
        .into_iter()
        .map(|coord| crate::stream::TileDebugEntry {
            coord,
            state: crate::stream::TileDebugState::Uploaded,
        })
        .collect()
}

fn apply_default_input_camera(camera: &mut Flycam) {
    camera.position = glam::Vec3::new(5645.5, 122.8, -10505.8);
    camera.yaw = (-124.80_f32).to_radians();
    camera.pitch = (-16.30_f32).to_radians();
}

fn apply_persisted_camera_if_allowed(
    camera: &mut Flycam,
    persisted_camera: Option<&crate::app::prefs::CameraPrefs>,
    cam_override: Option<&crate::camera::CameraOverride>,
    world_bbox: Option<(f32, f32, f32, f32)>,
) {
    if has_camera_override(cam_override) {
        return;
    }
    let Some(persisted_camera) = persisted_camera else {
        return;
    };
    if let Some((min_x, min_z, max_x, max_z)) = world_bbox {
        // Persisted coords are dataset-relative. If the saved position is far
        // outside the current dataset's bounds, the prefs were saved for a
        // different dataset — keep the default camera instead.
        const BUFFER: f32 = 5_000.0;
        let in_range = persisted_camera.x >= min_x - BUFFER
            && persisted_camera.x <= max_x + BUFFER
            && persisted_camera.z >= min_z - BUFFER
            && persisted_camera.z <= max_z + BUFFER;
        if !in_range {
            log::info!(
                "Persisted camera ({:.0}, {:.0}, {:.0}) is outside current dataset bounds \
                 ({:.0}..{:.0}, {:.0}..{:.0}); using default camera",
                persisted_camera.x,
                persisted_camera.y,
                persisted_camera.z,
                min_x,
                max_x,
                min_z,
                max_z,
            );
            return;
        }
    }
    persisted_camera.apply_to_camera(camera);
}

fn has_camera_override(cam_override: Option<&crate::camera::CameraOverride>) -> bool {
    let Some(ov) = cam_override else {
        return false;
    };
    ov.x.is_some()
        || ov.y.is_some()
        || ov.z.is_some()
        || ov.yaw.is_some()
        || ov.pitch.is_some()
        || ov.spawn_lat.is_some()
        || ov.spawn_lon.is_some()
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

fn apply_explicit_camera_overrides(
    camera: &mut Flycam,
    cam_override: Option<&crate::camera::CameraOverride>,
) {
    let Some(ov) = cam_override else {
        return;
    };
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

/// Creates a depth texture and view with `Depth32Float` format.
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
    fn persisted_camera_applies_when_no_camera_override_is_present() {
        let mut camera = Flycam::new(1.0);
        apply_default_input_camera(&mut camera);
        let prefs = crate::app::prefs::CameraPrefs {
            x: 10.0,
            y: 20.0,
            z: 30.0,
            yaw: 1.0,
            pitch: -0.5,
        };

        apply_persisted_camera_if_allowed(&mut camera, Some(&prefs), None, None);

        assert_eq!(camera.position, glam::vec3(10.0, 20.0, 30.0));
        assert_eq!(camera.yaw, 1.0);
        assert_eq!(camera.pitch, -0.5);
    }

    #[test]
    fn persisted_camera_outside_dataset_bounds_is_skipped() {
        let mut camera = Flycam::new(1.0);
        apply_default_input_camera(&mut camera);
        let default_position = camera.position;
        let prefs = crate::app::prefs::CameraPrefs {
            x: 50_000.0,
            y: 122.8,
            z: -50_000.0,
            yaw: 0.0,
            pitch: 0.0,
        };

        apply_persisted_camera_if_allowed(
            &mut camera,
            Some(&prefs),
            None,
            Some((0.0, -10_000.0, 5_000.0, 0.0)),
        );

        assert_eq!(camera.position, default_position);
    }

    #[test]
    fn persisted_camera_inside_dataset_bounds_applies() {
        let mut camera = Flycam::new(1.0);
        apply_default_input_camera(&mut camera);
        let prefs = crate::app::prefs::CameraPrefs {
            x: 2_000.0,
            y: 122.8,
            z: -3_000.0,
            yaw: 1.0,
            pitch: -0.5,
        };

        apply_persisted_camera_if_allowed(
            &mut camera,
            Some(&prefs),
            None,
            Some((0.0, -10_000.0, 5_000.0, 0.0)),
        );

        assert_eq!(camera.position, glam::vec3(2_000.0, 122.8, -3_000.0));
    }

    #[test]
    fn explicit_spawn_override_skips_persisted_camera() {
        let mut camera = Flycam::new(1.0);
        apply_default_input_camera(&mut camera);
        let before = camera.position;
        let prefs = crate::app::prefs::CameraPrefs {
            x: 10.0,
            y: 20.0,
            z: 30.0,
            yaw: 1.0,
            pitch: -0.5,
        };
        let override_with_spawn = crate::camera::CameraOverride {
            x: None,
            y: None,
            z: None,
            yaw: None,
            pitch: None,
            spawn_lat: Some(38.65671),
            spawn_lon: Some(-121.72179),
        };

        apply_persisted_camera_if_allowed(
            &mut camera,
            Some(&prefs),
            Some(&override_with_spawn),
            None,
        );

        assert_eq!(camera.position, before);
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

    #[test]
    fn explicit_camera_overrides_apply_before_streaming_selection() {
        let mut camera = Flycam::new(1.0);
        let overrides = crate::camera::CameraOverride {
            x: Some(10.0),
            y: Some(20.0),
            z: Some(-30.0),
            yaw: Some(45.0),
            pitch: Some(-10.0),
            spawn_lat: None,
            spawn_lon: None,
        };

        apply_explicit_camera_overrides(&mut camera, Some(&overrides));

        assert_eq!(camera.position, glam::vec3(10.0, 20.0, -30.0));
        assert_eq!(camera.yaw, 45.0_f32.to_radians());
        assert_eq!(camera.pitch, -10.0_f32.to_radians());
    }

    #[test]
    fn streaming_tile_selection_sorts_by_distance_and_caps_count() {
        let mut index = std::collections::HashMap::new();
        for coord in [
            crate::stream::TileCoord { x: 0, z: 0 },
            crate::stream::TileCoord { x: 1, z: 0 },
            crate::stream::TileCoord { x: 3, z: 0 },
        ] {
            index.insert(coord, crate::stream::tile::TileFeatureRefs::default());
        }

        let selected = select_streaming_tiles(&index, 100.0, glam::Vec3::ZERO, 250.0, 2);

        assert_eq!(
            selected.iter().map(|(coord, _)| *coord).collect::<Vec<_>>(),
            vec![
                crate::stream::TileCoord { x: 0, z: 0 },
                crate::stream::TileCoord { x: 1, z: 0 },
            ]
        );
    }
}
