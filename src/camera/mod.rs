pub mod controller;

use bytemuck::{Pod, Zeroable};

pub use controller::CameraController;

pub const SHADOW_CASCADE_COUNT: usize = 4;
pub const SHADOW_MAP_SIZE: u32 = 2048;
pub const SHADOW_CASCADE_RADII: [f32; SHADOW_CASCADE_COUNT] = [350.0, 900.0, 2200.0, 5200.0];
pub const SHADOW_CASCADE_BLEND_DISTANCE: f32 = 150.0;
pub const SHADOW_FAR_FADE_DISTANCE: f32 = 650.0;
pub const CONTACT_SHADOW_MAX_DISTANCE: f32 = 260.0;
pub const CONTACT_SHADOW_STRENGTH: f32 = 0.35;
pub const CONTACT_SHADOW_MIN_OCCLUDER_HEIGHT: f32 = 8.0;
const SHADOW_MAP_SIZE_F32: f32 = SHADOW_MAP_SIZE as f32;

#[derive(Copy, Clone, Debug)]
pub struct ShadowCascade {
    pub light_view_proj: glam::Mat4,
    pub radius: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct ShadowCascadeSet {
    pub cascades: [ShadowCascade; SHADOW_CASCADE_COUNT],
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ShadowCascadeBlend {
    pub weights: [f32; SHADOW_CASCADE_COUNT],
    pub shadow_strength: f32,
}

pub fn shadow_cascade_blend(
    distance: f32,
    radii: [f32; SHADOW_CASCADE_COUNT],
) -> ShadowCascadeBlend {
    let mut weights = [0.0; SHADOW_CASCADE_COUNT];

    for cascade_index in 0..(SHADOW_CASCADE_COUNT - 1) {
        let radius = radii[cascade_index];
        let blend_distance = cascade_blend_distance(cascade_index, radii);
        let blend_start = (radius - blend_distance).max(0.0);

        if distance <= blend_start {
            weights[cascade_index] = 1.0;
            return ShadowCascadeBlend {
                weights,
                shadow_strength: 1.0,
            };
        }

        if distance < radius {
            let next_weight = smoothstep(blend_start, radius, distance);
            weights[cascade_index] = 1.0 - next_weight;
            weights[cascade_index + 1] = next_weight;
            return ShadowCascadeBlend {
                weights,
                shadow_strength: 1.0,
            };
        }
    }

    let last_index = SHADOW_CASCADE_COUNT - 1;
    let last_radius = radii[last_index];
    let fade_start = (last_radius - SHADOW_FAR_FADE_DISTANCE).max(0.0);

    if distance <= fade_start {
        weights[last_index] = 1.0;
        return ShadowCascadeBlend {
            weights,
            shadow_strength: 1.0,
        };
    }

    if distance < last_radius {
        weights[last_index] = 1.0;
        return ShadowCascadeBlend {
            weights,
            shadow_strength: 1.0 - smoothstep(fade_start, last_radius, distance),
        };
    }

    ShadowCascadeBlend {
        weights,
        shadow_strength: 0.0,
    }
}

fn cascade_blend_distance(_index: usize, _radii: [f32; SHADOW_CASCADE_COUNT]) -> f32 {
    SHADOW_CASCADE_BLEND_DISTANCE
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

    pub fn shadow_cascades(&self, light_direction: [f32; 3]) -> ShadowCascadeSet {
        let light_dir = glam::Vec3::from(light_direction).normalize();

        ShadowCascadeSet {
            cascades: SHADOW_CASCADE_RADII.map(|radius| ShadowCascade {
                light_view_proj: self.shadow_light_view_proj(light_dir, radius),
                radius,
            }),
        }
    }

    /// Compatibility helper for callers/tests that still expect a single matrix.
    pub fn light_view_proj(&self, light_direction: [f32; 3]) -> glam::Mat4 {
        self.shadow_cascades(light_direction).cascades[1].light_view_proj
    }

    fn shadow_light_view_proj(&self, light_dir: glam::Vec3, half_extent: f32) -> glam::Mat4 {
        let light_view_rotation =
            glam::Mat4::look_to_rh(glam::Vec3::ZERO, -light_dir, glam::Vec3::Y);
        let camera_light_space = light_view_rotation.transform_point3(self.position);
        let texel_size = (half_extent * 2.0) / SHADOW_MAP_SIZE_F32;
        let snapped_camera_light_space = glam::Vec3::new(
            (camera_light_space.x / texel_size).round() * texel_size,
            (camera_light_space.y / texel_size).round() * texel_size,
            camera_light_space.z,
        );
        let snapped_camera_world = light_view_rotation
            .inverse()
            .transform_point3(snapped_camera_light_space);

        let light_pos = snapped_camera_world + light_dir * half_extent;
        let light_view = glam::Mat4::look_to_rh(light_pos, -light_dir, glam::Vec3::Y);
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
