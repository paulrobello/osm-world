// Shadow pass vertex shader — transforms world positions to light clip space.

const SHADOW_CASCADE_COUNT: u32 = 4u;

struct LightUniforms {
    light_view_proj: array<mat4x4<f32>, 4>,
    cascade_radii: vec4<f32>,
    shadow_params: vec4<f32>,
    shadow_pass_params: vec4<u32>,
};

@group(0) @binding(0) var<uniform> light: LightUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) feature_type: f32,
}

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    let cascade_index = min(light.shadow_pass_params.x, SHADOW_CASCADE_COUNT - 1u);
    return light.light_view_proj[cascade_index] * vec4<f32>(in.position, 1.0);
}
