pub mod event_handler;
pub mod init;
pub mod render_loop;
pub mod update;

use crate::camera::CameraController;

pub use init::AppState;

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

pub struct App {
    pub state: Option<AppState>,
    pub controller: CameraController,
    pub last_frame_time: std::time::Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: None,
            controller: CameraController::new(),
            last_frame_time: std::time::Instant::now(),
        }
    }
}
