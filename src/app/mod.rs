pub mod event_handler;
pub mod init;
pub mod render_loop;
pub mod update;

use crate::camera::CameraController;

pub use init::AppState;

pub struct AppOptions {
    pub window_width: f64,
    pub window_height: f64,
    pub screenshot_path: Option<String>,
    pub screenshot_delay: f32,
    pub auto_exit_delay: Option<f32>,
    pub input_path: Option<String>,
    pub srtm_dir: Option<String>,
}

pub struct App {
    pub state: Option<AppState>,
    pub controller: CameraController,
    pub last_frame_time: std::time::Instant,
    pub opts: AppOptions,
    pub render_start: Option<std::time::Instant>,
    pub screenshot_taken: bool,
}

impl App {
    pub fn new(opts: AppOptions) -> Self {
        Self {
            state: None,
            controller: CameraController::new(),
            last_frame_time: std::time::Instant::now(),
            opts,
            render_start: None,
            screenshot_taken: false,
        }
    }
}
