// Shadow pass vertex shader — transforms world positions to light clip space.

struct LightUniforms {
    light_view_proj: mat4x4<f32>,
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
    return light.light_view_proj * vec4<f32>(in.position, 1.0);
}
