use clap::Parser;

#[derive(Parser)]
#[command(
    name = "osm-world",
    about = "3D city renderer using OpenStreetMap data"
)]
struct Args {
    /// Path to .osm.pbf file to render
    #[arg(long)]
    input: Option<String>,

    /// Path to SRTM elevation cache directory
    #[arg(long)]
    srtm_dir: Option<String>,

    /// Save a screenshot to PATH after rendering starts
    #[arg(long)]
    screenshot: Option<String>,

    /// Delay in seconds before screenshot capture
    #[arg(long, default_value = "5.0")]
    screenshot_delay: f32,

    /// Exit automatically after N seconds
    #[arg(long)]
    auto_exit: Option<f32>,

    /// Window width in logical pixels
    #[arg(long, default_value = "1600")]
    width: f64,

    /// Window height in logical pixels
    #[arg(long, default_value = "1000")]
    height: f64,

    /// Override initial camera X position
    #[arg(long)]
    cam_x: Option<f32>,

    /// Override initial camera Y position
    #[arg(long)]
    cam_y: Option<f32>,

    /// Override initial camera Z position
    #[arg(long)]
    cam_z: Option<f32>,

    /// Override initial camera yaw in degrees
    #[arg(long)]
    cam_yaw: Option<f32>,

    /// Override initial camera pitch in degrees
    #[arg(long)]
    cam_pitch: Option<f32>,

    /// Start with the in-game settings panel open
    #[arg(long)]
    show_settings: bool,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();
    log::info!("osm-world starting");

    let event_loop = winit::event_loop::EventLoop::new()?;

    let cam_override = if args.cam_x.is_some()
        || args.cam_y.is_some()
        || args.cam_z.is_some()
        || args.cam_yaw.is_some()
        || args.cam_pitch.is_some()
    {
        Some(osm_world::camera::CameraOverride {
            x: args.cam_x,
            y: args.cam_y,
            z: args.cam_z,
            yaw: args.cam_yaw,
            pitch: args.cam_pitch,
        })
    } else {
        None
    };

    let mut app = osm_world::app::App::new(osm_world::app::AppOptions {
        window_width: args.width,
        window_height: args.height,
        screenshot_path: args.screenshot,
        screenshot_delay: args.screenshot_delay,
        auto_exit_delay: args.auto_exit,
        input_path: args.input,
        srtm_dir: args.srtm_dir,
        cam_override,
        show_settings: args.show_settings,
    });
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_show_settings_flag() {
        let args = Args::try_parse_from(["osm-world", "--show-settings"]).unwrap();
        assert!(args.show_settings);
    }
}
