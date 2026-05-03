pub mod controller;

use bytemuck::{Pod, Zeroable};

pub use controller::CameraController;

pub const SHADOW_NEAR_RADIUS: f32 = 900.0;
pub const SHADOW_MID_RADIUS: f32 = 2800.0;
pub const SHADOW_NEAR_BLEND_DISTANCE: f32 = 150.0;
pub const SHADOW_MID_FADE_DISTANCE: f32 = 300.0;
const SHADOW_MAP_SIZE: f32 = 2048.0;

#[derive(Copy, Clone, Debug)]
pub struct ShadowCascade {
    pub light_view_proj: glam::Mat4,
    pub radius: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct ShadowCascadeSet {
    pub near: ShadowCascade,
    pub mid: ShadowCascade,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ShadowCascadeBlend {
    pub near_weight: f32,
    pub mid_weight: f32,
    pub shadow_strength: f32,
}

pub fn shadow_cascade_blend(
    distance: f32,
    near_radius: f32,
    mid_radius: f32,
) -> ShadowCascadeBlend {
    let near_blend_start = (near_radius - SHADOW_NEAR_BLEND_DISTANCE).max(0.0);
    if distance <= near_blend_start {
        return ShadowCascadeBlend {
            near_weight: 1.0,
            mid_weight: 0.0,
            shadow_strength: 1.0,
        };
    }

    if distance < near_radius {
        let mid_weight = smoothstep(near_blend_start, near_radius, distance);
        return ShadowCascadeBlend {
            near_weight: 1.0 - mid_weight,
            mid_weight,
            shadow_strength: 1.0,
        };
    }

    let mid_fade_start = near_radius.max(mid_radius - SHADOW_MID_FADE_DISTANCE);
    if distance <= mid_fade_start {
        return ShadowCascadeBlend {
            near_weight: 0.0,
            mid_weight: 1.0,
            shadow_strength: 1.0,
        };
    }

    if distance < mid_radius {
        return ShadowCascadeBlend {
            near_weight: 0.0,
            mid_weight: 1.0,
            shadow_strength: 1.0 - smoothstep(mid_fade_start, mid_radius, distance),
        };
    }

    ShadowCascadeBlend {
        near_weight: 0.0,
        mid_weight: 0.0,
        shadow_strength: 0.0,
    }
}

fn smoothstep(start: f32, end: f32, value: f32) -> f32 {
    let t = ((value - start) / (end - start)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Optional overrides for the initial camera position and orientation.
pub struct CameraOverride {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub z: Option<f32>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
}

/// Scene uniform buffer layout (GPU). 288 bytes: camera + atmosphere.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SceneUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _pad0: f32,
    pub time_of_day: f32,
    pub animation_time: f32,
    pub ambient_light: f32,
    pub _pad1: f32,
    pub sun_direction: [f32; 3],
    pub _pad2: f32,
    pub light_direction: [f32; 3],
    pub light_intensity: f32,
    pub fog_density: f32,
    pub fog_start: f32,
    pub _pad3: [f32; 2],
    pub sky_zenith: [f32; 3],
    pub _pad4: f32,
    pub sky_horizon: [f32; 3],
    pub _pad5: f32,
    pub cloud_speed: f32,
    pub cloud_coverage: f32,
    pub _pad6: [f32; 2],
    pub cloud_color: [f32; 3],
    pub clouds_enabled: u32,
    pub ground_color: [f32; 3],
    pub _pad7: f32,
}

/// Flycam: free-flight camera controlled by WASD + mouse.
pub struct Flycam {
    pub position: glam::Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Flycam {
    pub fn new(aspect: f32) -> Self {
        Self {
            position: glam::Vec3::new(0.0, 50.0, 100.0),
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: -0.3,
            speed: 100.0,
            fov: std::f32::consts::FRAC_PI_4,
            aspect,
            near: 0.5,
            far: 50000.0,
        }
    }

    pub fn forward(&self) -> glam::Vec3 {
        glam::Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    pub fn right(&self) -> glam::Vec3 {
        glam::Vec3::new(-self.yaw.sin(), 0.0, self.yaw.cos()).normalize()
    }

    pub fn view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::look_to_rh(self.position, self.forward(), glam::Vec3::Y)
    }

    pub fn projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    pub fn uniforms(
        &self,
        day: &crate::atmosphere::DayCycleState,
        atm: &crate::atmosphere::AtmosphereSettings,
    ) -> SceneUniforms {
        let view = self.view_matrix();
        let proj = self.projection_matrix();
        let vp = proj * view;
        let sun_dir = crate::atmosphere::sun_direction(day.time_of_day);
        let light_dir = crate::atmosphere::dominant_light_direction(day.time_of_day);
        let light_intensity = crate::atmosphere::dominant_light_intensity(day.time_of_day);
        SceneUniforms {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            camera_pos: self.position.to_array(),
            _pad0: 0.0,
            time_of_day: day.time_of_day,
            animation_time: day.animation_time,
            ambient_light: atm.ambient_light,
            _pad1: 0.0,
            sun_direction: sun_dir,
            _pad2: 0.0,
            light_direction: light_dir,
            light_intensity,
            fog_density: atm.fog_density,
            fog_start: atm.fog_start,
            _pad3: [0.0; 2],
            sky_zenith: atm.sky_color_zenith,
            _pad4: 0.0,
            sky_horizon: atm.sky_color_horizon,
            _pad5: 0.0,
            cloud_speed: atm.cloud_speed,
            cloud_coverage: atm.cloud_coverage,
            _pad6: [0.0; 2],
            cloud_color: atm.cloud_color,
            clouds_enabled: atm.clouds_enabled as u32,
            ground_color: atm.ground_color,
            _pad7: 0.0,
        }
    }

    pub fn shadow_cascades(&self, sun_direction: [f32; 3]) -> ShadowCascadeSet {
        let sun_dir = glam::Vec3::from(sun_direction).normalize();

        ShadowCascadeSet {
            near: ShadowCascade {
                light_view_proj: self.shadow_light_view_proj(sun_dir, SHADOW_NEAR_RADIUS),
                radius: SHADOW_NEAR_RADIUS,
            },
            mid: ShadowCascade {
                light_view_proj: self.shadow_light_view_proj(sun_dir, SHADOW_MID_RADIUS),
                radius: SHADOW_MID_RADIUS,
            },
        }
    }

    /// Compatibility helper for callers/tests that still expect a single matrix.
    pub fn light_view_proj(&self, sun_direction: [f32; 3]) -> glam::Mat4 {
        self.shadow_cascades(sun_direction).mid.light_view_proj
    }

    fn shadow_light_view_proj(&self, sun_dir: glam::Vec3, half_extent: f32) -> glam::Mat4 {
        let light_view_rotation = glam::Mat4::look_to_rh(glam::Vec3::ZERO, -sun_dir, glam::Vec3::Y);
        let camera_light_space = light_view_rotation.transform_point3(self.position);
        let texel_size = (half_extent * 2.0) / SHADOW_MAP_SIZE;
        let snapped_camera_light_space = glam::Vec3::new(
            (camera_light_space.x / texel_size).round() * texel_size,
            (camera_light_space.y / texel_size).round() * texel_size,
            camera_light_space.z,
        );
        let snapped_camera_world = light_view_rotation
            .inverse()
            .transform_point3(snapped_camera_light_space);

        let light_pos = snapped_camera_world + sun_dir * half_extent;
        let light_view = glam::Mat4::look_to_rh(light_pos, -sun_dir, glam::Vec3::Y);
        let light_proj = glam::Mat4::orthographic_rh(
            -half_extent,
            half_extent,
            -half_extent,
            half_extent,
            0.0,
            half_extent * 3.0,
        );

        light_proj * light_view
    }
}
