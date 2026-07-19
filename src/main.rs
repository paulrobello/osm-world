use clap::Parser;
use osm_world::visual_detail::{LandmarkDetail, VisualDetailSettings, VisualPreset};

fn positive_f32(s: &str) -> Result<f32, String> {
    let value = s
        .parse::<f32>()
        .map_err(|err| format!("invalid float: {err}"))?;

    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err("must be a finite positive number".to_string())
    }
}

fn positive_usize(s: &str) -> Result<usize, String> {
    let value = s
        .parse::<usize>()
        .map_err(|err| format!("invalid integer: {err}"))?;

    if value >= 1 {
        Ok(value)
    } else {
        Err("must be at least 1".to_string())
    }
}

fn nonnegative_f32(s: &str) -> Result<f32, String> {
    let value = s
        .parse::<f32>()
        .map_err(|err| format!("invalid float: {err}"))?;

    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err("must be a finite nonnegative number".to_string())
    }
}

fn normalized_f32(s: &str) -> Result<f32, String> {
    let value = s
        .parse::<f32>()
        .map_err(|err| format!("invalid float: {err}"))?;

    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err("must be a finite number in the range 0..=1".to_string())
    }
}

fn density_multiplier(s: &str) -> Result<f32, String> {
    let value = s
        .parse::<f32>()
        .map_err(|err| format!("invalid float: {err}"))?;

    if value.is_finite() && (0.0..=3.0).contains(&value) {
        Ok(value)
    } else {
        Err("must be a finite number in the range 0..=3".to_string())
    }
}

fn latitude(s: &str) -> Result<f64, String> {
    let value = s
        .parse::<f64>()
        .map_err(|err| format!("invalid latitude: {err}"))?;

    if value.is_finite() && (-90.0..=90.0).contains(&value) {
        Ok(value)
    } else {
        Err("must be a finite latitude in the range -90..=90".to_string())
    }
}

fn longitude(s: &str) -> Result<f64, String> {
    let value = s
        .parse::<f64>()
        .map_err(|err| format!("invalid longitude: {err}"))?;

    if value.is_finite() && (-180.0..=180.0).contains(&value) {
        Ok(value)
    } else {
        Err("must be a finite longitude in the range -180..=180".to_string())
    }
}

fn hour_of_day(s: &str) -> Result<f32, String> {
    let value = s
        .parse::<f32>()
        .map_err(|err| format!("invalid hour: {err}"))?;

    if value.is_finite() && (0.0..=24.0).contains(&value) {
        Ok(value)
    } else {
        Err("must be a finite hour in the range 0..=24".to_string())
    }
}

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

    /// Run the HTTP API server instead of opening the renderer window
    #[arg(long)]
    serve: bool,

    /// API server host when --serve is used
    ///
    /// Defaults to loopback. Non-loopback hosts (e.g. `0.0.0.0`) are refused
    /// unless `OSM_WORLD_API_TOKEN` is set or `--allow-remote-host` is passed,
    /// so the localhost threat model cannot be voided by accident (SEC-004).
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// API server port when --serve is used
    #[arg(long, default_value_t = 3030)]
    port: u16,

    /// Explicitly allow binding the API to a non-loopback host without setting
    /// `OSM_WORLD_API_TOKEN`. Required to override the SEC-004 guard when the
    /// operator has another auth layer (reverse proxy, network firewall, etc.).
    #[arg(long = "allow-remote-host", default_value_t = false)]
    allow_remote_host: bool,

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

    /// Spawn latitude for initial camera placement
    #[arg(long, allow_hyphen_values = true, value_parser = latitude)]
    spawn_lat: Option<f64>,

    /// Spawn longitude for initial camera placement
    #[arg(long, allow_hyphen_values = true, value_parser = longitude)]
    spawn_lon: Option<f64>,

    /// Start with the in-game settings panel open
    #[arg(long)]
    show_settings: bool,

    /// Initial time of day in hours, where 0/24 is midnight and 12 is noon
    #[arg(long, value_parser = hour_of_day)]
    time_of_day: Option<f32>,

    /// Sync the in-game time of day to the local wall clock
    #[arg(long)]
    real_time_of_day: bool,

    /// Start with POI labels hidden
    #[arg(long)]
    hide_poi_labels: bool,

    /// Start with address labels hidden
    #[arg(long)]
    hide_address_labels: bool,

    /// Start with street sign labels hidden
    #[arg(long)]
    hide_street_sign_labels: bool,

    /// Start with the minimap hidden
    #[arg(long)]
    hide_minimap: bool,

    /// Start with the minimap rotating with the camera heading
    #[arg(long)]
    rotate_minimap: bool,

    /// Tint geometry by shadow cascade: blue near, orange mid, purple far fade
    #[arg(long)]
    debug_shadow_cascades: bool,

    /// Visual detail preset to use at startup
    #[arg(long, value_enum)]
    visual_preset: Option<VisualPreset>,

    /// Landmark rendering detail override
    #[arg(long, value_enum)]
    landmark_detail: Option<LandmarkDetail>,

    /// Facade variation multiplier in the range 0.0..=1.0
    #[arg(long, value_parser = normalized_f32)]
    facade_variation: Option<f32>,

    /// Roof variation multiplier in the range 0.0..=1.0
    #[arg(long, value_parser = normalized_f32)]
    roof_variation: Option<f32>,

    /// Vegetation density multiplier in the range 0.0..=3.0
    #[arg(long, value_parser = density_multiplier)]
    vegetation_density: Option<f32>,

    /// Maximum number of synthetic trees per tile
    #[arg(long, value_parser = positive_usize)]
    synthetic_tree_cap: Option<usize>,

    /// Maximum vegetation draw distance in metres
    #[arg(long = "vegetation-distance", value_parser = nonnegative_f32)]
    vegetation_distance: Option<f32>,

    /// Disable tile streaming and use the legacy single-mesh renderer
    #[arg(long)]
    no_streaming: bool,

    /// Streaming tile size in metres
    #[arg(long, default_value = "1000.0", value_parser = positive_f32)]
    tile_size: f32,

    /// Streaming radius in metres
    #[arg(long, default_value = "15000.0", value_parser = positive_f32)]
    stream_radius: f32,

    /// Per-frame GPU upload budget in MiB
    #[arg(long, default_value = "4.0", value_parser = positive_f32)]
    upload_budget_mb: f32,

    /// Maximum number of uploaded streaming tiles
    #[arg(long, default_value = "256", value_parser = positive_usize)]
    max_uploaded_tiles: usize,

    /// Maximum estimated uploaded tile memory in MiB
    #[arg(long, default_value = "512.0", value_parser = positive_f32)]
    max_uploaded_mb: f32,
}

fn validate_spawn_pair(args: &Args) -> anyhow::Result<()> {
    if args.spawn_lat.is_some() != args.spawn_lon.is_some() {
        anyhow::bail!("--spawn-lat and --spawn-lon must be provided together");
    }
    Ok(())
}

/// Returns true when `host` is a loopback address.
///
/// Recognizes the IPv4 loopback range (`127.0.0.0/8`), the IPv6 loopback
/// (`::1`), and common loopback hostnames (`localhost`). Anything else —
/// including `0.0.0.0` and other wildcard binds — is treated as remote.
fn is_loopback_host(host: &str) -> bool {
    let normalized = host.trim().to_ascii_lowercase();
    if normalized == "localhost" {
        return true;
    }
    let Ok(parsed) = normalized.parse::<std::net::IpAddr>() else {
        return false;
    };
    parsed.is_loopback()
}

/// Enforces the SEC-004 host-binding guard for `--serve`.
///
/// Loopback hosts are always allowed. Non-loopback hosts require either
/// `OSM_WORLD_API_TOKEN` to be set (so authenticated requests are the default)
/// or `--allow-remote-host` to be passed (so the operator acknowledges they are
/// responsible for an external auth/firewall layer). Local-default operation —
/// `--host 127.0.0.1` with no token — is unchanged.
fn enforce_host_policy(args: &Args) -> anyhow::Result<()> {
    if !args.serve {
        return Ok(());
    }
    enforce_host_policy_decision(
        &args.host,
        args.allow_remote_host,
        std::env::var("OSM_WORLD_API_TOKEN").is_ok(),
    )
}

/// Pure decision function extracted from [`enforce_host_policy`] so tests can
/// cover every branch without mutating the process env.
fn enforce_host_policy_decision(
    host: &str,
    allow_remote: bool,
    has_token: bool,
) -> anyhow::Result<()> {
    if is_loopback_host(host) {
        return Ok(());
    }
    if allow_remote || has_token {
        return Ok(());
    }
    anyhow::bail!(
        "refusing to bind --host '{}' without authentication: set OSM_WORLD_API_TOKEN or pass --allow-remote-host",
        host
    );
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();
    log::info!("osm-world starting");
    validate_spawn_pair(&args)?;

    let visual_detail_overridden = args.visual_preset.is_some()
        || args.landmark_detail.is_some()
        || args.facade_variation.is_some()
        || args.roof_variation.is_some()
        || args.vegetation_density.is_some()
        || args.synthetic_tree_cap.is_some()
        || args.vegetation_distance.is_some();
    let mut visual_detail =
        VisualDetailSettings::from_preset(args.visual_preset.unwrap_or(VisualPreset::Balanced));
    if let Some(landmark_detail) = args.landmark_detail {
        visual_detail.landmark_detail = landmark_detail;
    }
    if let Some(facade_variation) = args.facade_variation {
        visual_detail.facade_variation = facade_variation;
    }
    if let Some(roof_variation) = args.roof_variation {
        visual_detail.roof_variation = roof_variation;
    }
    if let Some(vegetation_density) = args.vegetation_density {
        visual_detail.vegetation_density = vegetation_density;
    }
    if let Some(synthetic_tree_cap) = args.synthetic_tree_cap {
        visual_detail.synthetic_tree_cap = synthetic_tree_cap;
    }
    if let Some(vegetation_distance) = args.vegetation_distance {
        visual_detail.vegetation_max_distance = vegetation_distance;
    }
    visual_detail.clamp();

    if args.serve {
        enforce_host_policy(&args)?;
        let rt = tokio::runtime::Runtime::new()?;
        return rt.block_on(osm_world::server::run(
            &args.host,
            args.port,
            std::env::current_dir()?,
        ));
    }

    let event_loop = winit::event_loop::EventLoop::new()?;

    let cam_override = if args.cam_x.is_some()
        || args.cam_y.is_some()
        || args.cam_z.is_some()
        || args.cam_yaw.is_some()
        || args.cam_pitch.is_some()
        || args.spawn_lat.is_some()
        || args.spawn_lon.is_some()
    {
        Some(osm_world::camera::CameraOverride {
            x: args.cam_x,
            y: args.cam_y,
            z: args.cam_z,
            yaw: args.cam_yaw,
            pitch: args.cam_pitch,
            spawn_lat: args.spawn_lat,
            spawn_lon: args.spawn_lon,
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
        initial_time_of_day: args.time_of_day.map(|hours| hours / 24.0),
        real_time_of_day: args.real_time_of_day,
        hide_poi_labels: args.hide_poi_labels,
        hide_address_labels: args.hide_address_labels,
        hide_street_sign_labels: args.hide_street_sign_labels,
        hide_minimap: args.hide_minimap,
        rotate_minimap: args.rotate_minimap,
        debug_shadow_cascades: args.debug_shadow_cascades,
        streaming: osm_world::app::StreamingOptions {
            enabled: !args.no_streaming,
            tile_size: args.tile_size,
            stream_radius: args.stream_radius,
            upload_budget_mb: args.upload_budget_mb,
            max_uploaded_tiles: args.max_uploaded_tiles,
            max_uploaded_mb: args.max_uploaded_mb,
        },
        visual_detail,
        visual_detail_overridden,
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

    #[test]
    fn parses_spawn_lat_lon_flags() {
        let args = Args::try_parse_from([
            "osm-world",
            "--spawn-lat",
            "38.65671",
            "--spawn-lon",
            "-121.72179",
        ])
        .unwrap();

        assert_eq!(args.spawn_lat, Some(38.65671));
        assert_eq!(args.spawn_lon, Some(-121.72179));
    }

    #[test]
    fn rejects_one_sided_spawn_lat_flag() {
        let args = Args::try_parse_from(["osm-world", "--spawn-lat", "38.65671"]).unwrap();

        let err = validate_spawn_pair(&args).unwrap_err();

        assert!(
            err.to_string()
                .contains("--spawn-lat and --spawn-lon must be provided together")
        );
    }

    #[test]
    fn rejects_invalid_spawn_coordinates() {
        for (flag, value) in [
            ("--spawn-lat", "NaN"),
            ("--spawn-lat", "91"),
            ("--spawn-lon", "inf"),
            ("--spawn-lon", "-181"),
        ] {
            let result = Args::try_parse_from(["osm-world", flag, value]);
            assert!(result.is_err(), "expected {flag} {value} to be rejected");
        }
    }

    #[test]
    fn parses_shadow_debug_and_time_flags() {
        let args = Args::try_parse_from([
            "osm-world",
            "--time-of-day",
            "21.5",
            "--debug-shadow-cascades",
        ])
        .unwrap();

        assert_eq!(args.time_of_day, Some(21.5));
        assert!(args.debug_shadow_cascades);
    }

    #[test]
    fn parses_real_time_of_day_flag() {
        let args = Args::try_parse_from(["osm-world", "--real-time-of-day"]).unwrap();

        assert!(args.real_time_of_day);
    }

    #[test]
    fn parses_serve_flags() {
        // Loopback hosts are always allowed by the SEC-004 guard.
        let args = Args::try_parse_from([
            "osm-world",
            "--serve",
            "--host",
            "127.0.0.1",
            "--port",
            "3031",
        ])
        .unwrap();

        assert!(args.serve);
        assert_eq!(args.host, "127.0.0.1");
        assert_eq!(args.port, 3031);
    }

    #[test]
    fn parses_allow_remote_host_flag() {
        // Non-loopback hosts parse fine; the SEC-004 guard is enforced at run
        // time, not parse time, so callers like our own tests can build the
        // Args and inspect them.
        let args = Args::try_parse_from([
            "osm-world",
            "--serve",
            "--host",
            "0.0.0.0",
            "--allow-remote-host",
        ])
        .unwrap();

        assert_eq!(args.host, "0.0.0.0");
        assert!(args.allow_remote_host);
    }

    #[test]
    fn sec004_loopback_hosts_are_allowed_without_token_or_flag() {
        for host in ["127.0.0.1", "::1", "localhost", "127.1.2.3"] {
            assert!(
                enforce_host_policy_decision(host, false, false).is_ok(),
                "loopback host {host:?} should be allowed"
            );
            assert!(is_loopback_host(host), "is_loopback_host({host:?})");
        }
    }

    #[test]
    fn sec004_non_loopback_host_rejected_without_token_or_flag() {
        let err = enforce_host_policy_decision("0.0.0.0", false, false).unwrap_err();
        assert!(
            err.to_string().contains("refusing to bind"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sec004_non_loopback_host_allowed_with_allow_remote_host_flag() {
        assert!(enforce_host_policy_decision("0.0.0.0", true, false).is_ok());
    }

    #[test]
    fn sec004_non_loopback_host_allowed_when_token_set() {
        assert!(enforce_host_policy_decision("0.0.0.0", false, true).is_ok());
    }

    #[test]
    fn sec004_non_loopback_ipv6_wildcard_also_rejected() {
        // `::` binds all IPv6 interfaces; treated as remote by the guard.
        let err = enforce_host_policy_decision("::", false, false).unwrap_err();
        assert!(err.to_string().contains("refusing to bind"));
    }

    #[test]
    fn parses_label_and_minimap_startup_flags() {
        let args = Args::try_parse_from([
            "osm-world",
            "--hide-poi-labels",
            "--hide-address-labels",
            "--hide-street-sign-labels",
            "--hide-minimap",
            "--rotate-minimap",
        ])
        .unwrap();

        assert!(args.hide_poi_labels);
        assert!(args.hide_address_labels);
        assert!(args.hide_street_sign_labels);
        assert!(args.hide_minimap);
        assert!(args.rotate_minimap);
    }

    #[test]
    fn parses_streaming_flags() {
        let args = Args::try_parse_from([
            "osm-world",
            "--no-streaming",
            "--tile-size",
            "500",
            "--stream-radius",
            "2500",
            "--upload-budget-mb",
            "2.5",
            "--max-uploaded-tiles",
            "64",
            "--max-uploaded-mb",
            "128",
        ])
        .unwrap();

        assert!(args.no_streaming);
        assert_eq!(args.tile_size, 500.0);
        assert_eq!(args.stream_radius, 2500.0);
        assert_eq!(args.upload_budget_mb, 2.5);
        assert_eq!(args.max_uploaded_tiles, 64);
        assert_eq!(args.max_uploaded_mb, 128.0);
    }

    #[test]
    fn parses_visual_detail_flags() {
        let args = Args::try_parse_from([
            "osm-world",
            "--visual-preset",
            "showcase",
            "--landmark-detail",
            "off",
            "--facade-variation",
            "0.4",
            "--roof-variation",
            "0.7",
            "--vegetation-density",
            "2.25",
            "--synthetic-tree-cap",
            "42",
            "--vegetation-distance",
            "1800",
        ])
        .unwrap();

        assert_eq!(
            args.visual_preset,
            Some(osm_world::visual_detail::VisualPreset::Showcase)
        );
        assert_eq!(
            args.landmark_detail,
            Some(osm_world::visual_detail::LandmarkDetail::Off)
        );
        assert_eq!(args.facade_variation, Some(0.4));
        assert_eq!(args.roof_variation, Some(0.7));
        assert_eq!(args.vegetation_density, Some(2.25));
        assert_eq!(args.synthetic_tree_cap, Some(42));
        assert_eq!(args.vegetation_distance, Some(1800.0));
    }

    #[test]
    fn rejects_invalid_streaming_numeric_flags() {
        for (flag, value) in [
            ("--tile-size", "0"),
            ("--stream-radius", "-1"),
            ("--upload-budget-mb", "NaN"),
            ("--max-uploaded-tiles", "0"),
            ("--max-uploaded-mb", "inf"),
            ("--time-of-day", "25"),
            ("--facade-variation", "-0.1"),
            ("--facade-variation", "1.1"),
            ("--roof-variation", "NaN"),
            ("--roof-variation", "2"),
            ("--vegetation-density", "3.1"),
            ("--vegetation-density", "inf"),
            ("--vegetation-distance", "-1"),
            ("--vegetation-distance", "NaN"),
        ] {
            let result = Args::try_parse_from(["osm-world", flag, value]);
            assert!(result.is_err(), "expected {flag} {value} to be rejected");
        }
    }
}
