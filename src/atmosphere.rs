pub const DAY_CYCLE_DURATION: f32 = 120.0;
pub const DEFAULT_TIME_OF_DAY: f32 = 14.0 / 24.0;
pub const MOON_LIGHT_INTENSITY: f32 = 0.25;

#[derive(Clone, Debug)]
pub struct AtmosphereSettings {
    pub ambient_light: f32,
    pub fog_density: f32,
    pub fog_start: f32,
    pub cloud_speed: f32,
    pub cloud_coverage: f32,
    pub cloud_color: [f32; 3],
    pub clouds_enabled: bool,
    pub sky_color_zenith: [f32; 3],
    pub sky_color_horizon: [f32; 3],
    pub ground_color: [f32; 3],
    pub shadow_cascade_debug: bool,
}

impl Default for AtmosphereSettings {
    fn default() -> Self {
        Self {
            ambient_light: 0.3,
            fog_density: 0.0008,
            fog_start: 1000.0,
            cloud_speed: 1.0,
            cloud_coverage: 0.45,
            cloud_color: [1.0, 1.0, 1.0],
            clouds_enabled: true,
            sky_color_zenith: [0.25, 0.45, 0.85],
            sky_color_horizon: [0.6, 0.75, 0.95],
            ground_color: [0.15, 0.12, 0.08],
            shadow_cascade_debug: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DayCycleState {
    pub time_of_day: f32,
    pub animation_time: f32,
    pub paused: bool,
}

impl Default for DayCycleState {
    fn default() -> Self {
        Self {
            time_of_day: DEFAULT_TIME_OF_DAY,
            animation_time: 0.0,
            paused: true,
        }
    }
}

impl DayCycleState {
    pub fn update(&mut self, dt: f32) {
        if !self.paused {
            self.time_of_day = (self.time_of_day + dt / DAY_CYCLE_DURATION).rem_euclid(1.0);
        }
        self.animation_time += dt;
    }
}

pub fn sun_direction(time_of_day: f32) -> [f32; 3] {
    let angle = time_of_day * 2.0 * std::f32::consts::PI;
    let y = -angle.cos();
    let xz = angle.sin();
    let len = (xz * xz + y * y + 0.09_f32).sqrt();
    [xz / len, y / len, 0.3 / len]
}

pub fn moon_direction(time_of_day: f32) -> [f32; 3] {
    let sun = sun_direction(time_of_day);
    [-sun[0], -sun[1], -sun[2]]
}

pub fn daylight_factor(time_of_day: f32) -> f32 {
    let sun_y = sun_direction(time_of_day)[1];
    smoothstep(-0.2, 0.3, sun_y)
}

pub fn dominant_light_direction(time_of_day: f32) -> [f32; 3] {
    let sun = sun_direction(time_of_day);
    if sun[1] >= 0.0 {
        sun
    } else {
        [-sun[0], -sun[1], -sun[2]]
    }
}

pub fn dominant_light_intensity(time_of_day: f32) -> f32 {
    let sun = sun_direction(time_of_day);
    if sun[1] >= 0.0 {
        daylight_factor(time_of_day)
    } else {
        (1.0 - daylight_factor(time_of_day)) * MOON_LIGHT_INTENSITY
    }
}

fn smoothstep(start: f32, end: f32, value: f32) -> f32 {
    let t = ((value - start) / (end - start)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_day_cycle_starts_paused_at_1400() {
        let day_cycle = DayCycleState::default();

        assert!(day_cycle.paused);
        assert_eq!(day_cycle.time_of_day, 14.0 / 24.0);
    }

    #[test]
    fn dominant_light_uses_sun_above_horizon() {
        let noon = 12.0 / 24.0;

        assert_eq!(dominant_light_direction(noon), sun_direction(noon));
        assert!(dominant_light_intensity(noon) > 0.99);
    }

    #[test]
    fn dominant_light_uses_moon_after_sunset() {
        let midnight = 0.0;

        assert_eq!(dominant_light_direction(midnight), moon_direction(midnight));
        assert!((dominant_light_intensity(midnight) - MOON_LIGHT_INTENSITY).abs() < 1e-6);
    }
}
