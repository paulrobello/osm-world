use wgpu::*;

use crate::camera::SceneUniforms;

pub struct MinimapTarget {
    pub color_texture: Texture,
    pub color_view: TextureView,
    pub depth_texture: Texture,
    pub depth_view: TextureView,
    pub bind_group: crate::render::bind_groups::SceneBindGroup,
    pub camera: crate::camera::Flycam,
}

impl MinimapTarget {
    pub const SIZE: u32 = 256;

    pub fn new(device: &Device, surface_format: TextureFormat) -> Self {
        let color_texture = device.create_texture(&TextureDescriptor {
            label: Some("minimap color texture"),
            size: Extent3d {
                width: Self::SIZE,
                height: Self::SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: surface_format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&TextureViewDescriptor::default());

        let depth_texture = device.create_texture(&TextureDescriptor {
            label: Some("minimap depth texture"),
            size: Extent3d {
                width: Self::SIZE,
                height: Self::SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&TextureViewDescriptor::default());

        let bind_group = crate::render::bind_groups::SceneBindGroup::new(device);
        let camera = crate::camera::Flycam::new(1.0);

        Self {
            color_texture,
            color_view,
            depth_texture,
            depth_view,
            bind_group,
            camera,
        }
    }

    /// Compute orthographic view-projection SceneUniforms for the minimap.
    pub fn uniforms(
        &self,
        main_camera: &crate::camera::Flycam,
        day: &crate::atmosphere::DayCycleState,
        atm: &crate::atmosphere::AtmosphereSettings,
        zoom_radius: f32,
        rotate_with_camera: bool,
    ) -> SceneUniforms {
        self.uniforms_with_visual_detail(
            main_camera,
            day,
            atm,
            &crate::visual_detail::VisualDetailSettings::default(),
            zoom_radius,
            rotate_with_camera,
        )
    }

    /// Compute orthographic view-projection SceneUniforms for the minimap.
    pub fn uniforms_with_visual_detail(
        &self,
        main_camera: &crate::camera::Flycam,
        day: &crate::atmosphere::DayCycleState,
        atm: &crate::atmosphere::AtmosphereSettings,
        visual: &crate::visual_detail::VisualDetailSettings,
        zoom_radius: f32,
        rotate_with_camera: bool,
    ) -> SceneUniforms {
        let horizontal_forward = {
            let forward = main_camera.forward();
            let horizontal = glam::Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            if horizontal.length_squared() > 0.0 {
                horizontal
            } else {
                glam::Vec3::NEG_Z
            }
        };
        let map_up = if rotate_with_camera {
            -horizontal_forward
        } else {
            glam::Vec3::Z
        };
        let view = glam::Mat4::look_to_rh(main_camera.position, glam::Vec3::NEG_Y, map_up);
        let proj = glam::Mat4::orthographic_rh(
            -zoom_radius,
            zoom_radius,
            -zoom_radius,
            zoom_radius,
            0.0,
            zoom_radius * 3.0,
        );
        let vp = proj * view;
        let sun_dir = crate::atmosphere::sun_direction(day.time_of_day);
        let light_dir = crate::atmosphere::dominant_light_direction(day.time_of_day);
        let light_intensity = crate::atmosphere::dominant_light_intensity(day.time_of_day);

        SceneUniforms {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            camera_pos: main_camera.position.to_array(),
            _pad0: 0.0,
            time_of_day: day.time_of_day,
            animation_time: day.animation_time,
            ambient_light: atm.ambient_light,
            _pad1: 0.0,
            sun_direction: sun_dir,
            _pad2: 0.0,
            light_direction: light_dir,
            light_intensity,
            fog_density: 0.0,
            fog_start: 99999.0,
            _pad3: [0.0; 2],
            sky_zenith: atm.sky_color_zenith,
            _pad4: 0.0,
            sky_horizon: atm.sky_color_horizon,
            _pad5: 0.0,
            cloud_speed: atm.cloud_speed,
            cloud_coverage: atm.cloud_coverage,
            _pad6: [0.0; 2],
            cloud_color: atm.cloud_color,
            clouds_enabled: 0,
            ground_color: atm.ground_color,
            _pad7: 0.0,
            visual_params: [
                visual.facade_variation,
                visual.roof_variation,
                visual.vegetation_max_distance,
                if visual.vegetation_visible { 1.0 } else { 0.0 },
            ],
            visual_params2: [landmark_detail_value(visual.landmark_detail), 0.0, 0.0, 0.0],
        }
    }
}

fn landmark_detail_value(detail: crate::visual_detail::LandmarkDetail) -> f32 {
    match detail {
        crate::visual_detail::LandmarkDetail::Off => 0.0,
        crate::visual_detail::LandmarkDetail::Simple => 1.0,
        crate::visual_detail::LandmarkDetail::Showcase => 2.0,
    }
}
