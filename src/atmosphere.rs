//! Day cycle, sun/moon direction, lighting, and atmospheric settings.
//!
//! Time of day is expressed as a fraction of 24 hours: 0.0 is midnight,
//! 0.5 is noon, and 1.0 wraps back to midnight. The `DAY_CYCLE_DURATION`
//! constant controls how many real seconds one full day cycle takes when
//! the animation is not paused and not synced to the wall clock.

use chrono::Timelike;

/// Duration of one full day-night cycle in real seconds (when animated).
pub const DAY_CYCLE_DURATION: f32 = 120.0;
/// Default time of day as a 0.0-1.0 fraction (14:00 / 24 = 0.5833).
pub const DEFAULT_TIME_OF_DAY: f32 = 14.0 / 24.0;
/// Light intensity multiplier used when the moon is the dominant light source.
pub const MOON_LIGHT_INTENSITY: f32 = 0.25;

/// Atmospheric and visual settings controlling fog, clouds, sky colors, and shadow debug.
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

/// Tracks the current time of day, animation state, and clock mode.
#[derive(Clone, Debug)]
pub struct DayCycleState {
    pub time_of_day: f32,
    pub animation_time: f32,
    pub paused: bool,
    pub real_clock: bool,
}

impl Default for DayCycleState {
    fn default() -> Self {
        Self {
            time_of_day: DEFAULT_TIME_OF_DAY,
            animation_time: 0.0,
            paused: true,
            real_clock: false,
        }
    }
}

impl DayCycleState {
    /// Advances the time of day by `dt` seconds.
    ///
    /// When `real_clock` is true, overrides the time with the local wall clock.
    /// When paused and not in real-clock mode, the time does not advance.
    /// `animation_time` always advances (used for cloud and shader animations).
    pub fn update(&mut self, dt: f32) {
        self.update_with_clock(dt, local_clock_time_of_day);
    }

    /// Like `update`, but accepts a custom clock function (testable).
    pub fn update_with_clock(&mut self, dt: f32, clock_time_of_day: impl FnOnce() -> f32) {
        if self.real_clock {
            self.time_of_day = clock_time_of_day().rem_euclid(1.0);
        } else if !self.paused {
            self.time_of_day = (self.time_of_day + dt / DAY_CYCLE_DURATION).rem_euclid(1.0);
        }
        self.animation_time += dt;
    }
}

/// Returns the current local wall-clock time as a 0.0-1.0 fraction of 24 hours.
pub fn local_clock_time_of_day() -> f32 {
    let now = chrono::Local::now();
    time_of_day_from_hms_nanos(now.hour(), now.minute(), now.second(), now.nanosecond())
}

/// Converts hours, minutes, and seconds to a 0.0-1.0 time-of-day fraction.
pub fn time_of_day_from_hms(hour: u32, minute: u32, second: u32) -> f32 {
    time_of_day_from_hms_nanos(hour, minute, second, 0)
}

/// Converts hours, minutes, seconds, and nanoseconds to a 0.0-1.0 time-of-day fraction.
pub fn time_of_day_from_hms_nanos(hour: u32, minute: u32, second: u32, nanosecond: u32) -> f32 {
    let seconds = (hour % 24) * 3600 + (minute % 60) * 60 + (second % 60);
    let fractional_second = (nanosecond % 1_000_000_000) as f32 / 1_000_000_000.0;
    (seconds as f32 + fractional_second) / 86_400.0
}

/// Returns the sun direction as a unit-ish vector for the given time-of-day fraction.
///
/// `time_of_day` is 0.0-1.0 where 0.25 is sunrise and 0.75 is sunset.
pub fn sun_direction(time_of_day: f32) -> [f32; 3] {
    let angle = time_of_day * 2.0 * std::f32::consts::PI;
    let y = -angle.cos();
    let xz = angle.sin();
    let len = (xz * xz + y * y + 0.09_f32).sqrt();
    [xz / len, y / len, 0.3 / len]
}

/// Returns the moon direction (opposite of the sun) for the given time-of-day fraction.
pub fn moon_direction(time_of_day: f32) -> [f32; 3] {
    let sun = sun_direction(time_of_day);
    [-sun[0], -sun[1], -sun[2]]
}

/// Returns a 0.0-1.0 daylight factor based on sun elevation.
///
/// Uses a smoothstep transition between -0.2 and 0.3 sun elevation.
pub fn daylight_factor(time_of_day: f32) -> f32 {
    let sun_y = sun_direction(time_of_day)[1];
    smoothstep(-0.2, 0.3, sun_y)
}

/// Returns the direction of the dominant light source (sun above horizon, moon below).
pub fn dominant_light_direction(time_of_day: f32) -> [f32; 3] {
    let sun = sun_direction(time_of_day);
    if sun[1] >= 0.0 {
        sun
    } else {
        [-sun[0], -sun[1], -sun[2]]
    }
}

/// Returns the dominant light intensity: daylight factor when the sun is up,
/// or a reduced moonlight factor when the sun is below the horizon.
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
    fn real_clock_fraction_uses_hours_minutes_and_seconds() {
        let fraction = time_of_day_from_hms(6, 30, 0);

        assert!((fraction - 6.5 / 24.0).abs() < 1e-6);
    }

    #[test]
    fn real_clock_fraction_includes_subsecond_progress() {
        let whole_second = time_of_day_from_hms(6, 30, 0);
        let half_second = time_of_day_from_hms_nanos(6, 30, 0, 500_000_000);

        assert!(half_second > whole_second);
        assert!((half_second - whole_second - 0.5 / 86_400.0).abs() < 1e-8);
    }

    #[test]
    fn real_clock_update_overrides_paused_animation() {
        let mut day_cycle = DayCycleState {
            time_of_day: 14.0 / 24.0,
            animation_time: 0.0,
            paused: true,
            real_clock: true,
        };

        day_cycle.update_with_clock(0.5, || 21.25 / 24.0);

        assert_eq!(day_cycle.time_of_day, 21.25 / 24.0);
        assert_eq!(day_cycle.animation_time, 0.5);
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
