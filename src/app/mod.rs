pub mod event_handler;
pub mod init;
pub mod render_loop;
pub mod update;

use crate::camera::CameraController;

pub use init::AppState;

#[derive(Clone, Debug)]
pub struct StreamingOptions {
    pub enabled: bool,
    pub tile_size: f32,
    pub stream_radius: f32,
    pub upload_budget_mb: f32,
    pub max_uploaded_tiles: usize,
    pub max_uploaded_mb: f32,
}

impl Default for StreamingOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            tile_size: 1000.0,
            stream_radius: 15_000.0,
            upload_budget_mb: 4.0,
            max_uploaded_tiles: 256,
            max_uploaded_mb: 512.0,
        }
    }
}

pub struct AppOptions {
    pub window_width: f64,
    pub window_height: f64,
    pub screenshot_path: Option<String>,
    pub screenshot_delay: f32,
    pub auto_exit_delay: Option<f32>,
    pub input_path: Option<String>,
    pub srtm_dir: Option<String>,
    pub cam_override: Option<crate::camera::CameraOverride>,
    pub show_settings: bool,
    pub initial_time_of_day: Option<f32>,
    pub debug_shadow_cascades: bool,
    pub streaming: StreamingOptions,
}

pub struct App {
    pub state: Option<AppState>,
    pub egui: Option<crate::ui::EguiState>,
    pub controller: CameraController,
    pub last_frame_time: std::time::Instant,
    pub opts: AppOptions,
    pub render_start: Option<std::time::Instant>,
    pub screenshot_taken: bool,
    pub atmosphere: crate::atmosphere::AtmosphereSettings,
    pub day_cycle: crate::atmosphere::DayCycleState,
    pub show_settings: bool,
    pub minimap: crate::ui::minimap::MinimapState,
}

impl App {
    pub fn new(opts: AppOptions) -> Self {
        let atmosphere = crate::atmosphere::AtmosphereSettings {
            shadow_cascade_debug: opts.debug_shadow_cascades,
            ..Default::default()
        };

        let mut day_cycle = crate::atmosphere::DayCycleState::default();
        if let Some(time_of_day) = opts.initial_time_of_day {
            day_cycle.time_of_day = time_of_day;
        }

        Self {
            state: None,
            egui: None,
            controller: CameraController::new(),
            last_frame_time: std::time::Instant::now(),
            show_settings: opts.show_settings,
            opts,
            render_start: None,
            screenshot_taken: false,
            atmosphere,
            day_cycle,
            minimap: crate::ui::minimap::MinimapState::default(),
        }
    }
}
