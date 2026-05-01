fn main() -> anyhow::Result<()> {
    env_logger::init();
    log::info!("osm-world starting");
    println!("osm-world: 3D OSM city renderer");
    println!("Run with --help for usage");
    Ok(())
}
