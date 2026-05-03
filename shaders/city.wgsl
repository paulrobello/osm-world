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
};

@group(0) @binding(0) var<uniform> scene: SceneUniforms;

struct ShadowUniforms {
    light_view_proj: array<mat4x4<f32>, 2>,
    cascade_radii: vec4<f32>,
    shadow_pass_params: vec4<u32>,
};

@group(1) @binding(0) var shadow_map: texture_depth_2d_array;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var<uniform> shadow: ShadowUniforms;

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
    @location(3) feature_type: f32,
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

struct Material {
    specular_strength: f32,
    shininess: f32,
}

fn get_material(feature: f32) -> Material {
    if (feature < 0.5) {
        return Material(0.0, 1.0);           // terrain
    } else if (feature < 1.5) {
        return Material(0.15, 32.0);          // building
    } else if (feature < 2.5) {
        return Material(0.08, 16.0);          // road
    } else if (feature < 3.5) {
        return Material(0.4, 64.0);           // water
    } else {
        return Material(0.0, 1.0);            // landuse
    }
}

fn shadow_cascade_blend(distance_to_camera: f32) -> vec3<f32> {
    let near_radius = shadow.cascade_radii.x;
    let mid_radius = shadow.cascade_radii.y;
    let near_blend_distance = shadow.cascade_radii.z;
    let mid_fade_distance = shadow.cascade_radii.w;

    let near_blend_start = max(near_radius - near_blend_distance, 0.0);
    if (distance_to_camera <= near_blend_start) {
        return vec3<f32>(1.0, 0.0, 1.0);
    }

    if (distance_to_camera < near_radius) {
        let mid_weight = smoothstep(near_blend_start, near_radius, distance_to_camera);
        return vec3<f32>(1.0 - mid_weight, mid_weight, 1.0);
    }

    let mid_fade_start = max(near_radius, mid_radius - mid_fade_distance);
    if (distance_to_camera <= mid_fade_start) {
        return vec3<f32>(0.0, 1.0, 1.0);
    }

    if (distance_to_camera < mid_radius) {
        return vec3<f32>(0.0, 1.0, 1.0 - smoothstep(mid_fade_start, mid_radius, distance_to_camera));
    }

    return vec3<f32>(0.0, 0.0, 0.0);
}

fn sample_shadow_cascade(world_pos: vec3<f32>, normal: vec3<f32>, cascade_index: u32) -> f32 {
    let light_space = shadow.light_view_proj[cascade_index] * vec4f(world_pos, 1.0);
    let ndc = light_space.xyz / light_space.w;

    if (ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }

    let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let bias = max(0.002 * (1.0 - dot(normal, scene.light_direction)), 0.001);

    let texel_size = 1.0 / 2048.0;
    var shadow_factor = 0.0;
    for (var x = -1; x <= 1; x += 2) {
        for (var y = -1; y <= 1; y += 2) {
            let offset = vec2f(f32(x), f32(y)) * texel_size;
            shadow_factor += textureSampleCompare(
                shadow_map,
                shadow_sampler,
                uv + offset,
                i32(cascade_index),
                ndc.z - bias,
            );
        }
    }
    return shadow_factor / 4.0;
}

fn sample_shadow(world_pos: vec3<f32>, normal: vec3<f32>, distance_to_camera: f32) -> f32 {
    let blend = shadow_cascade_blend(distance_to_camera);

    if (blend.z <= 0.0) {
        return 1.0;
    }

    var shadow_factor = 1.0;
    if (blend.x > 0.0 && blend.y > 0.0) {
        let near_shadow = sample_shadow_cascade(world_pos, normal, 0u);
        let mid_shadow = sample_shadow_cascade(world_pos, normal, 1u);
        shadow_factor = near_shadow * blend.x + mid_shadow * blend.y;
    } else if (blend.x > 0.0) {
        shadow_factor = sample_shadow_cascade(world_pos, normal, 0u);
    } else if (blend.y > 0.0) {
        shadow_factor = sample_shadow_cascade(world_pos, normal, 1u);
    }

    return mix(1.0, shadow_factor, blend.z);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = in.position;
    out.clip_position = scene.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_position = world_pos;
    out.world_normal = in.normal;
    out.color = in.color;
    out.feature_type = in.feature_type;
    return out;
}

fn shadow_cascade_debug_color(distance_to_camera: f32) -> vec3<f32> {
    let blend = shadow_cascade_blend(distance_to_camera);
    let near_color = vec3<f32>(0.1, 0.45, 1.0);
    let mid_color = vec3<f32>(1.0, 0.65, 0.1);
    let far_color = vec3<f32>(0.65, 0.2, 0.9);
    return near_color * blend.x + mid_color * blend.y + far_color * (1.0 - blend.z);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let light_dir = normalize(scene.light_direction);

    // Hemisphere ambient
    let hemisphere = mix(scene.ground_color, scene.sky_zenith, normal.y * 0.5 + 0.5);
    let ambient = hemisphere * scene.ambient_light;

    let dist = distance(in.world_position, scene.camera_pos);

    // Diffuse + shadow
    let shadow_factor = sample_shadow(in.world_position, normal, dist);
    let diffuse = max(dot(normal, light_dir), 0.0) * shadow_factor;

    // Specular (Blinn-Phong)
    let view_dir = normalize(scene.camera_pos - in.world_position);
    let half_vec = normalize(light_dir + view_dir);
    let mat = get_material(in.feature_type);
    let spec = pow(max(dot(normal, half_vec), 0.0), mat.shininess);
    let specular = mat.specular_strength * spec;

    let lighting = ambient + (diffuse + specular) * scene.light_intensity * (1.0 - scene.ambient_light);
    let lit_color = in.color * lighting;

    // Fog
    let fog_factor = get_fog_factor(dist);
    let fog_view_dir = normalize(in.world_position - scene.camera_pos);
    let fog_color = get_sky_color(fog_view_dir);
    var final_color = mix(lit_color, fog_color, fog_factor);

    if (shadow.shadow_pass_params.y != 0u) {
        final_color = mix(final_color, shadow_cascade_debug_color(dist), 0.55);
    }

    return vec4<f32>(final_color, 1.0);
}
