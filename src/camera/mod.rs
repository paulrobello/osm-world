//! Camera types and shadow-cascade helpers: free-flight flycam, scene-uniform
//! layout for the GPU, and cascade selection/blending for the sun shadow map.

pub mod controller;

use bytemuck::{Pod, Zeroable};

pub use controller::CameraController;

/// Number of cascades used by the sun shadow map.
pub const SHADOW_CASCADE_COUNT: usize = 4;
/// Square edge length, in texels, of each sun shadow cascade map.
pub const SHADOW_MAP_SIZE: u32 = 2048;
/// World-space radius per cascade; cascades cover progressively larger areas.
pub const SHADOW_CASCADE_RADII: [f32; SHADOW_CASCADE_COUNT] = [350.0, 900.0, 2200.0, 5200.0];
/// Distance over which two adjacent cascades cross-fade.
pub const SHADOW_CASCADE_BLEND_DISTANCE: f32 = 150.0;
/// Distance over which the final cascade fades into "no shadow" at far range.
pub const SHADOW_FAR_FADE_DISTANCE: f32 = 650.0;
/// Maximum camera-to-occluder distance for which contact shadows are applied.
pub const CONTACT_SHADOW_MAX_DISTANCE: f32 = 260.0;
/// Strength multiplier for the screen-space contact-shadow term.
pub const CONTACT_SHADOW_STRENGTH: f32 = 0.35;
/// Minimum occluder height (in world units) eligible to cast a contact shadow.
pub const CONTACT_SHADOW_MIN_OCCLUDER_HEIGHT: f32 = 8.0;
const SHADOW_MAP_SIZE_F32: f32 = SHADOW_MAP_SIZE as f32;

/// One sun shadow cascade: the light view-projection matrix and its world-space radius.
#[derive(Copy, Clone, Debug)]
pub struct ShadowCascade {
    pub light_view_proj: glam::Mat4,
    pub radius: f32,
}

/// A full set of shadow cascades for a single frame, one per `SHADOW_CASCADE_COUNT` slot.
#[derive(Copy, Clone, Debug)]
pub struct ShadowCascadeSet {
    pub cascades: [ShadowCascade; SHADOW_CASCADE_COUNT],
}

/// Per-cascade blend weights for the current camera distance plus an overall
/// shadow-strength multiplier that fades shadows out at the far edge.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ShadowCascadeBlend {
    pub weights: [f32; SHADOW_CASCADE_COUNT],
    pub shadow_strength: f32,
}

/// Computes the cascade blend weights and overall shadow strength for a camera
/// at `distance` from the shadow origin, given a cascade `radii` table. The
/// result sums to a single dominant cascade except inside the blend band
/// between two cascades, where it cross-fades.
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

fn landmark_detail_value(detail: crate::visual_detail::LandmarkDetail) -> f32 {
    match detail {
        crate::visual_detail::LandmarkDetail::Off => 0.0,
        crate::visual_detail::LandmarkDetail::Simple => 1.0,
        crate::visual_detail::LandmarkDetail::Showcase => 2.0,
    }
}

/// Optional overrides for the initial camera position and orientation.
pub struct CameraOverride {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub z: Option<f32>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub spawn_lat: Option<f64>,
    pub spawn_lon: Option<f64>,
}

/// Scene uniform buffer layout (GPU). 320 bytes: camera + atmosphere + visual detail.
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
    /// facade variation, roof variation, vegetation max distance, vegetation visible 1/0
    pub visual_params: [f32; 4],
    /// landmark detail numeric, reserved
    pub visual_params2: [f32; 4],
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
    /// Constructs a flycam at the default spawn position with sensible speed,
    /// field-of-view, and near/far plane defaults for the given `aspect` ratio.
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

    /// Returns the unit forward vector derived from yaw and pitch.
    pub fn forward(&self) -> glam::Vec3 {
        glam::Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    /// Returns the horizontal unit right vector derived from yaw (always Y-up).
    pub fn right(&self) -> glam::Vec3 {
        glam::Vec3::new(-self.yaw.sin(), 0.0, self.yaw.cos()).normalize()
    }

    /// Builds the right-handed view matrix from position and forward direction.
    pub fn view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::look_to_rh(self.position, self.forward(), glam::Vec3::Y)
    }

    /// Builds the right-handed perspective projection matrix from FOV, aspect,
    /// and near/far planes.
    pub fn projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    /// Builds scene uniforms with default visual-detail settings.
    pub fn uniforms(
        &self,
        day: &crate::atmosphere::DayCycleState,
        atm: &crate::atmosphere::AtmosphereSettings,
    ) -> SceneUniforms {
        self.uniforms_with_visual_detail(
            day,
            atm,
            &crate::visual_detail::VisualDetailSettings::default(),
        )
    }

    /// Builds scene uniforms with the supplied `visual` detail settings, baked
    /// into the visual-detail uniform slots alongside camera and atmosphere state.
    pub fn uniforms_with_visual_detail(
        &self,
        day: &crate::atmosphere::DayCycleState,
        atm: &crate::atmosphere::AtmosphereSettings,
        visual: &crate::visual_detail::VisualDetailSettings,
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
            visual_params: [
                visual.facade_variation,
                visual.roof_variation,
                visual.vegetation_max_distance,
                if visual.vegetation_visible { 1.0 } else { 0.0 },
            ],
            visual_params2: [
                landmark_detail_value(visual.landmark_detail),
                if visual.bike_ped_overlay { 1.0 } else { 0.0 },
                0.0,
                0.0,
            ],
        }
    }

    /// Builds the shadow cascade set for `light_direction`, snapping each
    /// cascade to its shadow-map texel grid to reduce shimmer on camera motion.
    pub fn shadow_cascades(&self, light_direction: [f32; 3]) -> ShadowCascadeSet {
        self.shadow_cascades_with_snap(light_direction, true)
    }

    /// Builds the shadow cascade set for `light_direction` without texel
    /// snapping. Use this when the light or camera moves every frame so that
    /// snapping would not produce a stable result anyway.
    pub fn shadow_cascades_for_dynamic_light(&self, light_direction: [f32; 3]) -> ShadowCascadeSet {
        self.shadow_cascades_with_snap(light_direction, false)
    }

    fn shadow_cascades_with_snap(
        &self,
        light_direction: [f32; 3],
        snap_to_texels: bool,
    ) -> ShadowCascadeSet {
        let light_dir = glam::Vec3::from(light_direction).normalize();

        ShadowCascadeSet {
            cascades: SHADOW_CASCADE_RADII.map(|radius| ShadowCascade {
                light_view_proj: self.shadow_light_view_proj(light_dir, radius, snap_to_texels),
                radius,
            }),
        }
    }

    /// Compatibility helper for callers/tests that still expect a single matrix.
    pub fn light_view_proj(&self, light_direction: [f32; 3]) -> glam::Mat4 {
        self.shadow_cascades(light_direction).cascades[1].light_view_proj
    }

    fn shadow_light_view_proj(
        &self,
        light_dir: glam::Vec3,
        half_extent: f32,
        snap_to_texels: bool,
    ) -> glam::Mat4 {
        let shadow_center = if snap_to_texels {
            let light_view_rotation =
                glam::Mat4::look_to_rh(glam::Vec3::ZERO, -light_dir, glam::Vec3::Y);
            let camera_light_space = light_view_rotation.transform_point3(self.position);
            let texel_size = (half_extent * 2.0) / SHADOW_MAP_SIZE_F32;
            let snapped_camera_light_space = glam::Vec3::new(
                (camera_light_space.x / texel_size).round() * texel_size,
                (camera_light_space.y / texel_size).round() * texel_size,
                camera_light_space.z,
            );
            light_view_rotation
                .inverse()
                .transform_point3(snapped_camera_light_space)
        } else {
            self.position
        };

        let light_pos = shadow_center + light_dir * half_extent;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_uniforms_include_visual_detail_params() {
        let camera = Flycam::new(1.0);
        let day = crate::atmosphere::DayCycleState::default();
        let atmosphere = crate::atmosphere::AtmosphereSettings::default();
        let visual = crate::visual_detail::VisualDetailSettings::from_preset(
            crate::visual_detail::VisualPreset::Showcase,
        );

        let uniforms = camera.uniforms_with_visual_detail(&day, &atmosphere, &visual);

        assert_eq!(uniforms.visual_params[0], visual.facade_variation);
        assert_eq!(uniforms.visual_params[1], visual.roof_variation);
        assert_eq!(uniforms.visual_params[2], visual.vegetation_max_distance);
        assert_eq!(uniforms.visual_params[3], 1.0);

        let hidden_visual = crate::visual_detail::VisualDetailSettings {
            vegetation_visible: false,
            ..visual
        };
        let hidden_uniforms = camera.uniforms_with_visual_detail(&day, &atmosphere, &hidden_visual);
        assert_eq!(
            hidden_uniforms.visual_params[2],
            hidden_visual.vegetation_max_distance
        );
        assert_eq!(hidden_uniforms.visual_params[3], 0.0);
    }
}
