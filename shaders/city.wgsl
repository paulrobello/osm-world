// City shader with fog and dynamic lighting

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
};

@group(0) @binding(0) var<uniform> scene: SceneUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) feature_type: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec3<f32>,
}

// --- Sky color helpers (duplicated from sky.wgsl for fog) ---

fn get_daylight(sun_y: f32) -> f32 {
    return smoothstep(-0.2, 0.3, sun_y);
}

fn get_sunset(sun_y: f32) -> f32 {
    return smoothstep(-0.3, 0.0, sun_y) * smoothstep(0.3, 0.0, sun_y);
}

fn get_sky_color(ray: vec3<f32>) -> vec3<f32> {
    let y = ray.y;
    let daylight = get_daylight(scene.sun_direction.y);
    let sunset = get_sunset(scene.sun_direction.y);

    let day_zenith = scene.sky_zenith;
    let day_horizon = scene.sky_horizon;
    let day_color = mix(day_horizon, day_zenith, max(pow(max(y, 0.0), 0.5), 0.0));

    let sunset_zenith = vec3<f32>(0.15, 0.1, 0.3);
    let sunset_horizon = vec3<f32>(0.9, 0.4, 0.1);
    let sunset_color = mix(sunset_horizon, sunset_zenith, max(pow(max(y, 0.0), 0.5), 0.0));

    let night_zenith = vec3<f32>(0.02, 0.02, 0.06);
    let night_horizon = vec3<f32>(0.05, 0.05, 0.12);
    let night_color = mix(night_horizon, night_zenith, max(pow(max(y, 0.0), 0.5), 0.0));

    let result = mix(night_color, day_color, daylight);
    let sunset_tinted = mix(result, sunset_color, sunset * 0.7);

    let below = pow(max(-y, 0.0), 0.7);
    return mix(sunset_tinted, sunset_tinted * 0.3, below);
}

fn get_fog_factor(distance: f32) -> f32 {
    if (distance < scene.fog_start) { return 0.0; }
    return 1.0 - exp(-scene.fog_density * (distance - scene.fog_start));
}

// --- City shaders ---

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = in.position;
    out.clip_position = scene.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_position = world_pos;
    out.world_normal = in.normal;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(scene.sun_direction);
    let ambient = scene.ambient_light;
    let diffuse = max(dot(normalize(in.world_normal), light_dir), 0.0);
    let lighting = ambient + diffuse * (1.0 - ambient);
    let lit_color = in.color * lighting;

    // Fog
    let dist = distance(in.world_position, scene.camera_pos);
    let fog_factor = get_fog_factor(dist);
    let view_dir = normalize(in.world_position - scene.camera_pos);
    let fog_color = get_sky_color(view_dir);
    let final_color = mix(lit_color, fog_color, fog_factor);

    return vec4<f32>(final_color, 1.0);
}
