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
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();
    log::info!("osm-world starting");

    let event_loop = winit::event_loop::EventLoop::new()?;
    let mut app = osm_world::app::App::new(osm_world::app::AppOptions {
        window_width: args.width,
        window_height: args.height,
        screenshot_path: args.screenshot,
        screenshot_delay: args.screenshot_delay,
        auto_exit_delay: args.auto_exit,
        input_path: args.input,
        srtm_dir: args.srtm_dir,
    });
    event_loop.run_app(&mut app)?;

    Ok(())
}
