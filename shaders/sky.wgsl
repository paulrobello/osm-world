// Analytic sky shader — ported from voxel-world sky.glsl
// Renders as fullscreen triangle (no vertex buffer, 3 vertices via @builtin(vertex_index))

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

// --- Vertex/Fragment IO ---

struct SkyVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) ray_dir: vec3<f32>,
};

// --- Noise functions ---

fn hash(p: vec2<f32>) -> f32 {
    let p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    let d = dot(p3, p3 + 33.33);
    return fract((p3.x + p3.y) * p3.z + d);
}

fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

fn fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    for (var i = 0u; i < 4u; i = i + 1u) {
        value += amplitude * noise2d(pos);
        pos *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

// --- Sky helpers (loaded from sky_helpers.wgsl at compile time) ---

fn get_sun(ray: vec3<f32>) -> vec3<f32> {
    let sun = scene.sun_direction;
    let daylight = get_daylight(sun.y);
    let cos_angle = dot(normalize(ray), sun);

    let sun_radius = 0.0027;
    let sun_radius_norm = max((1.0 - cos_angle) / sun_radius, 0.0);
    let sun_disk = 1.0 - smoothstep(0.82, 1.08, sun_radius_norm);
    let sun_limb = 1.0 - smoothstep(0.0, 0.95, sun_radius_norm);
    let sun_core = 1.0 - smoothstep(0.0, 0.34, sun_radius_norm);

    var sun_right = cross(vec3<f32>(0.0, 1.0, 0.0), sun);
    if (length(sun_right) < 0.001) {
        sun_right = vec3<f32>(1.0, 0.0, 0.0);
    }
    sun_right = normalize(sun_right);
    let sun_up = normalize(cross(sun, sun_right));
    let sun_uv = vec2<f32>(dot(ray, sun_right), dot(ray, sun_up)) / sun_radius;
    let sun_surface = mix(
        0.90,
        1.08,
        fbm(
            sun_uv * 2.4 + vec2<f32>(scene.animation_time * 0.006, -scene.animation_time * 0.004),
        ),
    );

    let limb_color = vec3<f32>(0.92, 0.48, 0.16);
    let core_color = vec3<f32>(1.0, 0.86, 0.42);
    let disk_color =
        mix(limb_color, core_color, sun_limb) * sun_disk * (0.46 + sun_core * 0.24) * sun_surface;

    let tight_glow = pow(max(cos_angle, 0.0), 220.0) * 0.13;
    let wide_glow = pow(max(cos_angle, 0.0), 36.0) * 0.045;
    let sun_corona = (tight_glow + wide_glow) * (0.35 + daylight * 0.65);
    let corona_color = vec3<f32>(1.0, 0.62, 0.25) * sun_corona;

    return (disk_color + corona_color) * daylight;
}

fn get_moon(ray: vec3<f32>) -> vec3<f32> {
    let moon = -scene.sun_direction;
    let daylight = get_daylight(scene.sun_direction.y);
    let cos_angle = dot(normalize(ray), moon);
    let disk = smoothstep(0.998, 0.9995, cos_angle) * (1.0 - daylight);
    let glow = pow(max(cos_angle, 0.0), 16.0) * 0.15 * (1.0 - daylight);
    return vec3<f32>(0.7, 0.75, 0.9) * (disk + glow);
}

fn star_layer(ray: vec3<f32>, scale: f32, threshold: f32, radius: f32, speed: f32, offset: vec2<f32>) -> f32 {
    let star_coord = ray.xz / (abs(ray.y) + 0.001) * scale + offset;
    let cell = floor(star_coord);
    let local = fract(star_coord);
    let seed = hash(cell);
    if (seed < threshold) { return 0.0; }

    let star_pos = vec2<f32>(
        hash(cell + vec2<f32>(17.1, 29.4)),
        hash(cell + vec2<f32>(43.7, 11.9)),
    );
    let dist = distance(local, star_pos);
    let disk = smoothstep(radius + 0.02, radius, dist);

    let phase = hash(cell + vec2<f32>(101.3, 77.7)) * 6.2831;
    let twinkle = 0.65 + 0.35 * sin(scene.animation_time * speed + phase);
    let brightness = mix(0.45, 1.0, hash(cell + vec2<f32>(9.2, 63.4)));

    return disk * twinkle * brightness;
}

fn get_stars(ray: vec3<f32>) -> vec3<f32> {
    let daylight = get_daylight(scene.sun_direction.y);
    if (daylight > 0.5) { return vec3<f32>(0.0); }

    let night_fade = clamp(1.0 - daylight * 2.0, 0.0, 1.0);
    let large_stars = star_layer(ray, 180.0, 0.992, 0.035, 1.7, vec2<f32>(0.0, 0.0));
    let small_stars = star_layer(ray, 360.0, 0.996, 0.025, 2.6, vec2<f32>(53.2, 19.7));

    return vec3<f32>((large_stars + small_stars * 0.7) * night_fade);
}

// --- Clouds ---

const CLOUD_HEIGHT: f32 = 500.0;
const CLOUD_SCALE: f32 = 0.0005;
const CLOUD_WIND: f32 = 0.02;

fn get_clouds(ray: vec3<f32>) -> vec3<f32> {
    if (scene.clouds_enabled == 0u || ray.y <= 0.001) { return vec3<f32>(0.0); }

    let daylight = get_daylight(scene.sun_direction.y);

    let t = (CLOUD_HEIGHT - scene.camera_pos.y) / ray.y;
    if (t < 0.0) { return vec3<f32>(0.0); }

    let hit = scene.camera_pos + ray * t;
    let uv = hit.xz * CLOUD_SCALE;

    let wind_offset = vec2<f32>(
        scene.animation_time * CLOUD_WIND * scene.cloud_speed * CLOUD_SCALE,
        scene.animation_time * CLOUD_WIND * scene.cloud_speed * CLOUD_SCALE * 0.7,
    );

    let cloud1 = fbm(uv + wind_offset);
    let cloud2 = fbm(uv * 1.5 + wind_offset * 0.7 + vec2<f32>(100.0, 200.0));
    let cloud_val = (cloud1 + cloud2 * 0.5) / 1.5;

    let threshold = 1.0 - scene.cloud_coverage;
    let density = smoothstep(threshold, threshold + 0.2, cloud_val);

    let dist = t;
    let fade = smoothstep(20000.0, 5000.0, dist);
    let horizon_fade = smoothstep(0.0, 0.15, ray.y);

    let cloud_color = mix(scene.cloud_color, vec3<f32>(0.6, 0.6, 0.7), (1.0 - daylight) * 0.5);
    return cloud_color * density * fade * horizon_fade;
}

// --- Fog helpers (loaded from sky_helpers.wgsl at compile time) ---

// --- Vertex shader: fullscreen triangle ---

@vertex
fn vs_sky(@builtin(vertex_index) vid: u32) -> SkyVarying {
    // Fullscreen triangle: 3 vertices cover the entire screen
    // v0=(-1,-1) v1=(3,-1) v2=(-1,3)
    let x = select(-1.0, 3.0, vid == 1u);
    let y = select(-1.0, 3.0, vid == 2u);
    let clip = vec4<f32>(x, y, 1.0, 1.0);
    let world_h = scene.inv_view_proj * clip;
    let world_pos = world_h.xyz / world_h.w;
    var out: SkyVarying;
    out.position = vec4<f32>(x, y, 0.9999, 1.0);
    out.ray_dir = normalize(world_pos - scene.camera_pos);
    return out;
}

// --- Fragment shader ---

@fragment
fn fs_sky(in: SkyVarying) -> @location(0) vec4<f32> {
    let ray = normalize(in.ray_dir);
    var color = get_sky_color(ray);
    color += get_sun(ray);
    color += get_moon(ray);
    color += get_stars(ray);
    let cloud = get_clouds(ray);
    color = mix(color, cloud + color * 0.3, length(cloud));
    return vec4<f32>(color, 1.0);
}
