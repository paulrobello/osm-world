fn main() -> anyhow::Result<()> {
    env_logger::init();
    log::info!("osm-world starting");

    let event_loop = winit::event_loop::EventLoop::new()?;
    let mut app = osm_world::app::App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
