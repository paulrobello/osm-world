// Shared sky color and fog helpers used by both city.wgsl and sky.wgsl.
// This file is concatenated at compile time by the Rust shader loader.

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
