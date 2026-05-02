pub const DAY_CYCLE_DURATION: f32 = 120.0;
pub const DEFAULT_TIME_OF_DAY: f32 = 14.0 / 24.0;

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
}

impl Default for AtmosphereSettings {
    fn default() -> Self {
        Self {
            ambient_light: 0.3,
            fog_density: 0.002,
            fog_start: 500.0,
            cloud_speed: 1.0,
            cloud_coverage: 0.45,
            cloud_color: [1.0, 1.0, 1.0],
            clouds_enabled: true,
            sky_color_zenith: [0.25, 0.45, 0.85],
            sky_color_horizon: [0.6, 0.75, 0.95],
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
            paused: false,
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
