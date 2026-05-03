pub mod controller;

use bytemuck::{Pod, Zeroable};

pub use controller::CameraController;

/// Optional overrides for the initial camera position and orientation.
pub struct CameraOverride {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub z: Option<f32>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
}

/// Scene uniform buffer layout (GPU). 272 bytes: camera + atmosphere.
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

    /// Compute the light view-projection matrix for directional shadow mapping.
    /// Fits an orthographic projection around the camera frustum as seen from the sun.
    pub fn light_view_proj(&self, sun_direction: [f32; 3]) -> glam::Mat4 {
        let sun_dir = glam::Vec3::from(sun_direction).normalize();

        let half_extent = 1000.0;

        let light_pos = self.position + sun_dir * half_extent;
        let light_view = glam::Mat4::look_to_rh(light_pos, -sun_dir, glam::Vec3::Y);

        let light_proj = glam::Mat4::orthographic_rh(
            -half_extent, half_extent,
            -half_extent, half_extent,
            0.0, half_extent * 3.0,
        );

        light_proj * light_view
    }
}
