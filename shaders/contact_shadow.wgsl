// Fullscreen composite pass with short-range screen-space contact shadows.

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
struct ContactShadowUniforms {
    max_distance: f32,
    strength: f32,
    _pad: vec2<f32>,
};

@group(1) @binding(0) var scene_color: texture_2d<f32>;
@group(1) @binding(1) var scene_depth: texture_depth_2d;
@group(1) @binding(2) var scene_sampler: sampler;
@group(1) @binding(3) var<uniform> contact: ContactShadowUniforms;

struct PostVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> PostVarying {
    let x = select(-1.0, 3.0, vid == 1u);
    let y = select(-1.0, 3.0, vid == 2u);

    var out: PostVarying;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return out;
}

fn clip_from_uv_depth(uv: vec2<f32>, depth: f32) -> vec4<f32> {
    return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
}

fn world_from_uv_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let world_h = scene.inv_view_proj * clip_from_uv_depth(uv, depth);
    return world_h.xyz / world_h.w;
}

fn uv_depth_from_world(world_pos: vec3<f32>) -> vec3<f32> {
    let clip = scene.view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    return vec3<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5, ndc.z);
}

fn load_scene_depth(uv: vec2<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(scene_depth));
    let coords = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(scene_depth, coords, 0);
}

fn contact_shadow(uv: vec2<f32>, world_pos: vec3<f32>) -> f32 {
    let view_distance = distance(world_pos, scene.camera_pos);
    let distance_fade = 1.0 - smoothstep(contact.max_distance * 0.45, contact.max_distance, view_distance);
    if (distance_fade <= 0.0 || scene.light_intensity <= 0.0) {
        return 0.0;
    }

    let light_dir = normalize(scene.light_direction);
    var occlusion = 0.0;

    for (var step_index = 1u; step_index <= 6u; step_index += 1u) {
        let step_distance = f32(step_index) * 2.5;
        let ray_pos = world_pos + light_dir * step_distance;
        let ray_screen = uv_depth_from_world(ray_pos);

        if (ray_screen.x <= 0.0 || ray_screen.x >= 1.0 || ray_screen.y <= 0.0 || ray_screen.y >= 1.0 || ray_screen.z <= 0.0 || ray_screen.z >= 1.0) {
            continue;
        }

        let sampled_depth = load_scene_depth(ray_screen.xy);
        let depth_delta = ray_screen.z - sampled_depth;
        if (depth_delta > 0.00002 && depth_delta < 0.01) {
            let step_weight = 1.0 - f32(step_index - 1u) / 6.0;
            occlusion = max(occlusion, step_weight);
        }
    }

    return occlusion * distance_fade * contact.strength;
}

@fragment
fn fs_main(in: PostVarying) -> @location(0) vec4<f32> {
    let color = textureSample(scene_color, scene_sampler, in.uv);
    let depth = load_scene_depth(in.uv);
    if (depth >= 0.99999) {
        return color;
    }

    let world_pos = world_from_uv_depth(in.uv, depth);
    let contact = contact_shadow(in.uv, world_pos);
    return vec4<f32>(color.rgb * (1.0 - contact), color.a);
}
