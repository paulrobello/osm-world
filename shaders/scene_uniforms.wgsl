// Scene uniform block shared by city.wgsl and sky.wgsl.
//
// This file is the SINGLE source of truth for the `SceneUniforms` struct and
// its binding. `src/render/pipelines.rs` (city) and `src/render/sky_pipeline.rs`
// both prepend it to the main shader source at compile time. The Rust-side
// layout lives in `src/render/scene_uniforms.rs` and must agree byte-for-byte
// (std140 layout, manual padding); `tests/shader_source_test.rs` parses this
// file and asserts both shaders see the same struct.
//
// The `_pad0`..`_pad7` fields are explicit std140 padding. Do not reorder or
// remove fields without updating the Rust mirror and the offsets used by the
// shadow/sky/cloud getters below.

struct SceneUniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad0: f32,
    time_of_day: f32,
    animation_time: f32,
    ambient_light: f32,
    _pad1: f32,
    sun_direction: vec3<f32>,
    _pad2: f32,
    light_direction: vec3<f32>,
    light_intensity: f32,
    fog_density: f32,
    fog_start: f32,
    _pad3: vec2<f32>,
    sky_zenith: vec3<f32>,
    _pad4: f32,
    sky_horizon: vec3<f32>,
    _pad5: f32,
    cloud_speed: f32,
    cloud_coverage: f32,
    _pad6: vec2<f32>,
    cloud_color: vec3<f32>,
    clouds_enabled: u32,
    ground_color: vec3<f32>,
    _pad7: f32,
    visual_params: vec4<f32>,
    visual_params2: vec4<f32>,
};

@group(0) @binding(0) var<uniform> scene: SceneUniforms;
