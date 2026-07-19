use super::*;
use prepared_cache::*;
use routes::prepare_area;
use shell::{path_string, shell_quote};
use std::{ffi::OsString, path::Path, sync::Mutex};
use types::{PrepareAreaError, PrepareAreaRequest};
use validate::{
    prepared_cache_key, validate_bbox, validate_extra_args, validate_spawn,
    validate_srtm_tile_limit,
};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

struct EnvRestore {
    vars: Vec<(&'static str, Option<OsString>)>,
}

impl EnvRestore {
    fn capture(names: &[&'static str]) -> Self {
        Self {
            vars: names
                .iter()
                .map(|name| (*name, std::env::var_os(name)))
                .collect(),
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (name, value) in &self.vars {
            // SAFETY: EnvRestore is only used within a test guarded by
            // ENV_MUTEX, which serializes all tests that mutate env vars.
            // This ensures no concurrent access to the process environment.
            unsafe {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

fn set_test_cache_env(tmp: &tempfile::TempDir) {
    let home = tmp.path().join("home");
    let overpass_dir = tmp.path().join("overpass");
    let srtm_dir = tmp.path().join("srtm");
    let overture_dir = tmp.path().join("overture");
    // SAFETY: All tests calling this function hold ENV_MUTEX, serializing
    // access to the process environment. No production code reads these
    // vars concurrently during test execution.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("PAR_OSM_OVERPASS_CACHE_DIR", &overpass_dir);
        std::env::remove_var("OVERPASS_CACHE_DIR");
        std::env::set_var("PAR_OSM_SRTM_CACHE_DIR", &srtm_dir);
        std::env::remove_var("SRTM_CACHE_DIR");
        std::env::set_var("PAR_OSM_OVERTURE_CACHE_DIR", &overture_dir);
        std::env::remove_var("OVERTURE_CACHE_DIR");
    }
}

fn cache_xml_for_bbox(bbox: [f64; 4], filter: &par_osm_rust::filter::FeatureFilter) -> String {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
</osm>"#;
    cache_xml_for_bbox_with_xml(bbox, filter, xml)
}

fn cache_xml_for_bbox_with_xml(
    bbox: [f64; 4],
    filter: &par_osm_rust::filter::FeatureFilter,
    xml: &str,
) -> String {
    let overpass_url = par_osm_rust::overpass::default_overpass_url();
    cache_xml_for_bbox_with_xml_and_overpass_url(bbox, filter, xml, overpass_url)
}

fn cache_xml_for_bbox_with_overpass_url(
    bbox: [f64; 4],
    filter: &par_osm_rust::filter::FeatureFilter,
    overpass_url: &str,
) -> String {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0" lon="-121.0"/>
</osm>"#;
    cache_xml_for_bbox_with_xml_and_overpass_url(bbox, filter, xml, overpass_url)
}

fn cache_xml_for_bbox_with_xml_and_overpass_url(
    bbox: [f64; 4],
    filter: &par_osm_rust::filter::FeatureFilter,
    xml: &str,
    overpass_url: &str,
) -> String {
    let bbox_tuple = (bbox[0], bbox[1], bbox[2], bbox[3]);
    let cache_key = par_osm_rust::osm_cache::cache_key_for_url(bbox_tuple, filter, overpass_url);
    par_osm_rust::osm_cache::write_for_url(&cache_key, bbox_tuple, filter, xml, overpass_url)
        .unwrap();
    cache_key
}

fn cached_prepare_request(
    bbox: [f64; 4],
    filter: par_osm_rust::filter::FeatureFilter,
) -> PrepareAreaRequest {
    PrepareAreaRequest {
        bbox,
        filter,
        use_elevation: false,
        force_refresh: false,
        overpass_url: None,
        spawn_lat: None,
        spawn_lon: None,
        overture: false,
        overture_themes: Vec::new(),
        poi_source_mode: None,
        overture_failure_mode: None,
        overture_timeout: None,
    }
}

#[test]
fn prepare_area_uses_exact_cached_xml_with_spawn_point() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);

    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);
    let cache_key = prepared_cache_key(
        (bbox[0], bbox[1], bbox[2], bbox[3]),
        &filter,
        false,
        &[],
        par_osm_rust::sources::PoiSourceMode::OsmOnly,
        par_osm_rust::sources::OvertureFailureMode::FallbackToOsm,
        par_osm_rust::overpass::default_overpass_url(),
    );
    let mut req = cached_prepare_request(bbox, filter);
    req.spawn_lat = Some(38.0005);
    req.spawn_lon = Some(-120.9995);

    let response = prepare_area(req, tmp.path()).unwrap();

    assert_eq!(response.cache_key, cache_key);
    assert_eq!(response.cache_status, "prepared");
    assert_eq!(response.source_status, "osm_only");
    assert!(response.warnings.is_empty());
    assert_eq!(response.spawn_lat, Some(38.0005));
    assert_eq!(response.spawn_lon, Some(-120.9995));
    assert!(response.command_args.iter().any(|arg| arg == "--spawn-lat"));
    assert!(response.command_args.iter().any(|arg| arg == "38.0005"));
    assert!(response.command_args.iter().any(|arg| arg == "--spawn-lon"));
    assert!(response.command_args.iter().any(|arg| arg == "-120.9995"));
    let input_index = response
        .command_args
        .iter()
        .position(|arg| arg == "--input")
        .unwrap();
    assert_eq!(response.command_args[input_index + 2], "--spawn-lat");
    assert_eq!(response.command_args[input_index + 3], "38.0005");
    assert_eq!(response.command_args[input_index + 4], "--spawn-lon");
    assert_eq!(response.command_args[input_index + 5], "-120.9995");
    assert!(response.command.contains("--spawn-lat 38.0005"));
    assert!(response.command.contains("--spawn-lon -120.9995"));
}

#[test]
fn prepare_area_writes_prepared_history_metadata_for_reopen() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);

    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter {
        roads: true,
        buildings: false,
        water: true,
        landuse: false,
        railways: true,
    };
    cache_xml_for_bbox(bbox, &filter);
    let mut req = cached_prepare_request(bbox, filter.clone());
    req.spawn_lat = Some(38.0005);
    req.spawn_lon = Some(-120.9995);

    let response = prepare_area(req, tmp.path()).unwrap();
    let entries = list_prepared_areas(tmp.path()).unwrap();

    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.cache_key, response.cache_key);
    assert_eq!(entry.bbox, bbox);
    assert_eq!(entry.filter, filter);
    assert_eq!(entry.spawn_lat, Some(38.0005));
    assert_eq!(entry.spawn_lon, Some(-120.9995));
    assert_eq!(entry.source_status, "osm_only");
    assert_eq!(entry.osm_path, response.osm_path);
    assert_eq!(entry.command, response.command);
    assert_eq!(entry.display_name, None);
    assert!(!entry.favorite);
}

#[test]
fn update_prepared_area_details_persists_name_and_favorite() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);

    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);
    let response = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();

    let updated = update_prepared_area_details(
        &response.cache_key,
        PreparedAreaUpdate {
            display_name: Some("Downtown smoke test".to_string()),
            favorite: Some(true),
        },
        tmp.path(),
    )
    .unwrap();
    let listed = list_prepared_areas(tmp.path()).unwrap();

    assert_eq!(updated.display_name.as_deref(), Some("Downtown smoke test"));
    assert!(updated.favorite);
    assert_eq!(
        listed[0].display_name.as_deref(),
        Some("Downtown smoke test")
    );
    assert!(listed[0].favorite);
}

#[test]
fn delete_prepared_area_removes_osm_and_metadata() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);

    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);
    let response = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();
    let osm_path = Path::new(&response.osm_path).to_path_buf();
    let metadata_path = osm_path.with_extension("meta.json");

    let deleted = delete_prepared_area(&response.cache_key).unwrap();
    let listed = list_prepared_areas(tmp.path()).unwrap();

    assert_eq!(deleted.cache_key, response.cache_key);
    assert_eq!(deleted.status, "deleted");
    assert!(!osm_path.exists());
    assert!(!metadata_path.exists());
    assert!(listed.is_empty());
}

#[test]
fn renderer_launch_command_uses_prepared_file_and_optional_runtime_flags() {
    let project_root = Path::new("/tmp/osm world");
    let req = LaunchRendererRequest {
        osm_path: "/tmp/prepared/city.osm".to_string(),
        srtm_dir: Some("/tmp/srtm cache".to_string()),
        spawn_lat: Some(38.0005),
        spawn_lon: Some(-120.9995),
        extra_args: vec!["--visual-preset".to_string(), "showcase".to_string()],
    };

    let command = shell::renderer_launch_command(project_root, &req).unwrap();

    assert_eq!(command.program, "cargo");
    assert_eq!(command.args[0], "run");
    assert_eq!(command.args[1], "--manifest-path");
    assert_eq!(command.args[2], "/tmp/osm world/Cargo.toml");
    assert!(
        command
            .args
            .windows(2)
            .any(|window| window == ["--input", "/tmp/prepared/city.osm"])
    );
    assert!(
        command
            .args
            .windows(2)
            .any(|window| window == ["--srtm-dir", "/tmp/srtm cache"])
    );
    assert!(
        command
            .args
            .windows(2)
            .any(|window| window == ["--spawn-lat", "38.0005"])
    );
    assert!(
        command
            .args
            .windows(2)
            .any(|window| window == ["--spawn-lon", "-120.9995"])
    );
    assert!(
        command
            .args
            .windows(2)
            .any(|window| window == ["--visual-preset", "showcase"])
    );
    assert!(command.command.contains("'/tmp/osm world/Cargo.toml'"));
}

#[test]
fn prepared_cache_key_ignores_overture_options_when_overture_disabled() {
    let bbox = (38.0, -121.0, 38.001, -120.999);
    let filter = par_osm_rust::filter::FeatureFilter::default();

    let default_key = prepared_cache_key(
        bbox,
        &filter,
        false,
        &[],
        par_osm_rust::sources::PoiSourceMode::OsmOnly,
        par_osm_rust::sources::OvertureFailureMode::FallbackToOsm,
        par_osm_rust::overpass::default_overpass_url(),
    );
    let noisy_key = prepared_cache_key(
        bbox,
        &filter,
        false,
        &["not-a-theme".to_string(), "places".to_string()],
        par_osm_rust::sources::PoiSourceMode::OverturePreferred,
        par_osm_rust::sources::OvertureFailureMode::Fail,
        par_osm_rust::overpass::default_overpass_url(),
    );

    assert_eq!(noisy_key, default_key);
}

#[test]
fn prepared_cache_key_canonicalizes_overture_theme_aliases_order_and_all_default() {
    let bbox = (38.0, -121.0, 38.001, -120.999);
    let filter = par_osm_rust::filter::FeatureFilter::default();
    let mode = par_osm_rust::sources::PoiSourceMode::OverturePreferred;
    let failure = par_osm_rust::sources::OvertureFailureMode::FallbackToOsm;

    let alias_key = prepared_cache_key(
        bbox,
        &filter,
        true,
        &[
            "places".to_string(),
            "BUILDING".to_string(),
            "place".to_string(),
        ],
        mode,
        failure,
        par_osm_rust::overpass::default_overpass_url(),
    );
    let canonical_key = prepared_cache_key(
        bbox,
        &filter,
        true,
        &["building".to_string(), "place".to_string()],
        mode,
        failure,
        par_osm_rust::overpass::default_overpass_url(),
    );
    assert_eq!(alias_key, canonical_key);

    let default_all_key = prepared_cache_key(
        bbox,
        &filter,
        true,
        &[],
        mode,
        failure,
        par_osm_rust::overpass::default_overpass_url(),
    );
    let explicit_all_key = prepared_cache_key(
        bbox,
        &filter,
        true,
        &[
            "address".to_string(),
            "base".to_string(),
            "building".to_string(),
            "place".to_string(),
            "transportation".to_string(),
        ],
        mode,
        failure,
        par_osm_rust::overpass::default_overpass_url(),
    );
    assert_eq!(default_all_key, explicit_all_key);
}

#[test]
fn prepare_area_reuses_existing_prepared_file_on_second_call() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);

    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    let source_cache_key = cache_xml_for_bbox(bbox, &filter);

    let first = prepare_area(cached_prepare_request(bbox, filter.clone()), tmp.path()).unwrap();
    std::fs::write(
        par_osm_rust::cache::overpass_cache_dir().join(format!("{source_cache_key}.xml")),
        "not valid osm xml",
    )
    .unwrap();
    let second = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();

    assert_eq!(first.cache_status, "prepared");
    assert_eq!(second.cache_status, "prepared_cache_hit");
    assert_eq!(second.cache_key, first.cache_key);
    assert_eq!(second.osm_path, first.osm_path);
    assert_eq!(second.source_status, "osm_only");
    assert!(second.warnings.is_empty());
}

#[test]
fn prepare_area_missing_prepared_metadata_returns_conservative_cache_hit() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);

    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);

    let first = prepare_area(cached_prepare_request(bbox, filter.clone()), tmp.path()).unwrap();
    std::fs::remove_file(Path::new(&first.osm_path).with_extension("meta.json")).unwrap();
    let second = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();

    assert_eq!(second.cache_status, "prepared_cache_hit");
    assert_eq!(second.source_status, "cached_unknown");
    assert!(
        second
            .warnings
            .iter()
            .any(|warning| warning.contains("metadata") && warning.contains("missing")),
        "expected missing metadata warning, got {:?}",
        second.warnings
    );
}

#[test]
fn prepare_area_includes_effective_overpass_url_in_prepared_cache_key() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);

    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);

    let default_response =
        prepare_area(cached_prepare_request(bbox, filter.clone()), tmp.path()).unwrap();
    let custom_overpass_url = "https://overpass.kumi.systems/api/interpreter";
    cache_xml_for_bbox_with_overpass_url(bbox, &filter, custom_overpass_url);
    let mut custom_req = cached_prepare_request(bbox, filter);
    custom_req.overpass_url = Some(custom_overpass_url.to_string());
    let custom_response = prepare_area(custom_req, tmp.path()).unwrap();

    assert_ne!(custom_response.cache_key, default_response.cache_key);
    assert_ne!(custom_response.osm_path, default_response.osm_path);
}

#[test]
fn prepare_area_rejects_invalid_overture_theme_only_when_overture_enabled() {
    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    let mut req = cached_prepare_request(bbox, filter);
    req.overture = true;
    req.overture_themes = vec!["definitely-not-a-theme".to_string()];

    let err = prepare_area(req, Path::new(".")).unwrap_err();

    assert!(matches!(err, PrepareAreaError::BadRequest { .. }));
}

#[test]
fn prepare_area_ignores_invalid_overture_theme_when_overture_disabled() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);

    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);
    let expected_key = prepared_cache_key(
        (bbox[0], bbox[1], bbox[2], bbox[3]),
        &filter,
        false,
        &[],
        par_osm_rust::sources::PoiSourceMode::OsmOnly,
        par_osm_rust::sources::OvertureFailureMode::FallbackToOsm,
        par_osm_rust::overpass::default_overpass_url(),
    );
    let mut req = cached_prepare_request(bbox, filter);
    req.overture_themes = vec!["definitely-not-a-theme".to_string()];
    req.poi_source_mode = Some(par_osm_rust::sources::PoiSourceMode::OvertureOnly);
    req.overture_failure_mode = Some(par_osm_rust::sources::OvertureFailureMode::Fail);

    let response = prepare_area(req, tmp.path()).unwrap();

    assert_eq!(response.cache_key, expected_key);
    assert_eq!(response.source_status, "osm_only");
}

#[test]
fn prepare_area_request_rejects_invalid_source_mode_enums() {
    let invalid_poi_mode = serde_json::json!({
        "bbox": [38.0, -121.0, 38.001, -120.999],
        "poi_source_mode": "not_a_mode"
    });
    let invalid_failure_mode = serde_json::json!({
        "bbox": [38.0, -121.0, 38.001, -120.999],
        "overture_failure_mode": "not_a_mode"
    });

    assert!(serde_json::from_value::<PrepareAreaRequest>(invalid_poi_mode).is_err());
    assert!(serde_json::from_value::<PrepareAreaRequest>(invalid_failure_mode).is_err());
}

#[test]
fn prepare_area_writes_renderable_poi_xml() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);
    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <bounds minlat="38.0" minlon="-121.0" maxlat="38.001" maxlon="-120.999"/>
  <node id="1" lat="38.0005" lon="-120.9995">
    <tag k="amenity" v="restaurant"/>
    <tag k="name" v="Test Cafe"/>
  </node>
</osm>"#;
    cache_xml_for_bbox_with_xml(bbox, &filter, xml);

    let response = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();
    let source =
        crate::world::loader::load_world_source(Path::new(&response.osm_path), None).unwrap();

    assert_eq!(source.point_features.len(), 1);
    assert_eq!(
        source.point_features[0]
            .tags
            .get("name")
            .map(String::as_str),
        Some("Test Cafe")
    );
}

#[test]
fn prepare_area_uses_exact_cached_xml_without_elevation() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);

    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);
    let cache_key = prepared_cache_key(
        (bbox[0], bbox[1], bbox[2], bbox[3]),
        &filter,
        false,
        &[],
        par_osm_rust::sources::PoiSourceMode::OsmOnly,
        par_osm_rust::sources::OvertureFailureMode::FallbackToOsm,
        par_osm_rust::overpass::default_overpass_url(),
    );

    let response = prepare_area(cached_prepare_request(bbox, filter), tmp.path()).unwrap();

    assert_eq!(response.cache_key, cache_key);
    assert_eq!(response.cache_status, "prepared");
    assert_eq!(response.source_status, "osm_only");
    assert!(response.warnings.is_empty());
    assert!(response.osm_path.ends_with(".osm"));
    assert!(Path::new(&response.osm_path).exists());
    assert_eq!(
        std::fs::read_to_string(&response.osm_path).unwrap(),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<osm version=\"0.6\">\n  <bounds minlat=\"38\" minlon=\"-121\" maxlat=\"38.001\" maxlon=\"-120.999\"/>\n</osm>\n"
    );
    assert!(response.srtm_dir.is_none());
    assert!(response.command.contains("--manifest-path"));
    assert!(response.command.contains("--input"));
    assert!(response.command.contains(&response.osm_path));
    assert!(!response.command.contains("--srtm-dir"));
    assert_eq!(response.command_cwd, path_string(tmp.path()));
    assert_eq!(response.command_program, "cargo");
    assert!(response.command_args.iter().any(|arg| arg == "--input"));
    assert!(
        response
            .command_args
            .iter()
            .any(|arg| arg == &response.osm_path)
    );
    assert!(!response.command_args.iter().any(|arg| arg == "--srtm-dir"));
}

#[test]
fn command_quotes_project_root_with_spaces_and_quotes_and_preserves_structured_args() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);

    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);

    let project_root = tmp.path().join("project root with 'quote'");
    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);

    let response = prepare_area(cached_prepare_request(bbox, filter), &project_root).unwrap();
    let manifest_path = path_string(project_root.join("Cargo.toml"));

    assert_eq!(response.command_cwd, path_string(&project_root));
    assert_eq!(response.command_program, "cargo");
    assert_eq!(response.command_args[0], "run");
    assert_eq!(response.command_args[1], "--manifest-path");
    assert_eq!(response.command_args[2], manifest_path);
    assert!(response.command.contains(&shell_quote(&manifest_path)));
    assert!(response.command.contains(" -- --input "));
}

#[test]
fn prepare_area_reports_overture_fallback_warning() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PATH",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);
    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);
    // SAFETY: This test holds ENV_MUTEX, serializing env var access.
    unsafe {
        std::env::set_var("PATH", tmp.path().join("empty-path"));
    }

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);
    let mut req = cached_prepare_request(bbox, filter);
    req.overture = true;
    req.poi_source_mode = Some(par_osm_rust::sources::PoiSourceMode::OverturePreferred);
    req.overture_failure_mode = Some(par_osm_rust::sources::OvertureFailureMode::FallbackToOsm);
    req.overture_timeout = Some(1);

    let response = prepare_area(req, tmp.path()).unwrap();

    assert_eq!(response.source_status, "overture_fallback_to_osm");
    assert!(
        response
            .warnings
            .iter()
            .any(|warning| warning.contains("Overture"))
    );
    assert!(Path::new(&response.osm_path).exists());
    assert!(
        Path::new(&response.osm_path)
            .with_extension("meta.json")
            .exists()
    );
}

#[test]
fn prepare_area_retries_degraded_overture_fallback_prepared_cache() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let _restore = EnvRestore::capture(&[
        "HOME",
        "PATH",
        "PAR_OSM_OVERPASS_CACHE_DIR",
        "OVERPASS_CACHE_DIR",
        "PAR_OSM_SRTM_CACHE_DIR",
        "SRTM_CACHE_DIR",
        "PAR_OSM_OVERTURE_CACHE_DIR",
        "OVERTURE_CACHE_DIR",
    ]);
    let tmp = tempfile::tempdir().unwrap();
    set_test_cache_env(&tmp);
    // SAFETY: This test holds ENV_MUTEX, serializing env var access.
    unsafe {
        std::env::set_var("PATH", tmp.path().join("empty-path"));
    }

    let bbox = [38.0, -121.0, 38.001, -120.999];
    let filter = par_osm_rust::filter::FeatureFilter::default();
    cache_xml_for_bbox(bbox, &filter);
    let mut req = cached_prepare_request(bbox, filter.clone());
    req.overture = true;
    req.poi_source_mode = Some(par_osm_rust::sources::PoiSourceMode::OverturePreferred);
    req.overture_failure_mode = Some(par_osm_rust::sources::OvertureFailureMode::FallbackToOsm);
    req.overture_timeout = Some(1);

    let first = prepare_area(req, tmp.path()).unwrap();
    let mut second_req = cached_prepare_request(bbox, filter);
    second_req.overture = true;
    second_req.poi_source_mode = Some(par_osm_rust::sources::PoiSourceMode::OverturePreferred);
    second_req.overture_failure_mode =
        Some(par_osm_rust::sources::OvertureFailureMode::FallbackToOsm);
    second_req.overture_timeout = Some(1);
    let second = prepare_area(second_req, tmp.path()).unwrap();

    assert_eq!(first.cache_status, "prepared");
    assert_eq!(second.cache_status, "prepared");
    assert_eq!(second.cache_key, first.cache_key);
    assert_eq!(second.osm_path, first.osm_path);
    assert_eq!(second.source_status, "overture_fallback_to_osm");
    assert!(
        second
            .warnings
            .iter()
            .any(|warning| warning.contains("Overture"))
    );
}

#[test]
fn validate_spawn_rejects_invalid_spawn_points() {
    let bbox = (38.0, -121.0, 38.001, -120.999);

    assert!(validate_spawn(None, None, bbox).unwrap().is_none());
    assert!(matches!(
        validate_spawn(Some(38.0005), None, bbox),
        Err(PrepareAreaError::BadRequest { .. })
    ));
    assert!(matches!(
        validate_spawn(None, Some(-120.9995), bbox),
        Err(PrepareAreaError::BadRequest { .. })
    ));

    for (lat, lon) in [
        (f64::NAN, -120.9995),
        (f64::INFINITY, -120.9995),
        (90.1, -120.9995),
        (-90.1, -120.9995),
        (38.0005, f64::NAN),
        (38.0005, f64::INFINITY),
        (38.0005, 180.1),
        (38.0005, -180.1),
        (37.9999, -120.9995),
        (38.0011, -120.9995),
        (38.0005, -121.0001),
        (38.0005, -120.9989),
    ] {
        assert!(
            matches!(
                validate_spawn(Some(lat), Some(lon), bbox),
                Err(PrepareAreaError::BadRequest { .. })
            ),
            "expected spawn ({lat}, {lon}) to be rejected"
        );
    }
}

#[test]
fn validate_bbox_rejects_huge_bbox() {
    let err = validate_bbox([38.0, -121.0, 38.51, -120.99]).unwrap_err();

    assert!(matches!(err, PrepareAreaError::BadRequest { .. }));
}

#[test]
fn validate_srtm_tile_limit_rejects_too_many_tiles_before_download() {
    let err = validate_srtm_tile_limit((-4.1, -4.1, 0.1, 0.1)).unwrap_err();

    assert!(matches!(err, PrepareAreaError::BadRequest { .. }));
}

// -- SEC-007: extra_args value validation -----------------------------------

#[test]
fn validate_extra_args_accepts_known_flag_with_valid_value() {
    let args = vec![
        "--visual-preset".to_string(),
        "showcase".to_string(),
        "--max-uploaded-tiles".to_string(),
        "64".to_string(),
    ];
    validate_extra_args(&args).expect("known flags with valid values should pass");
}

#[test]
fn validate_extra_args_accepts_boolean_switches_without_values() {
    let args = vec![
        "--hide-minimap".to_string(),
        "--debug-shadow-cascades".to_string(),
        "--no-streaming".to_string(),
    ];
    validate_extra_args(&args).expect("boolean switches should pass");
}

#[test]
fn validate_extra_args_rejects_unknown_flag_name() {
    let args = vec!["--dangerous-flag".to_string()];
    let err = validate_extra_args(&args).unwrap_err();
    assert!(
        err.to_string().contains("unsupported renderer flag"),
        "expected unsupported-flag error, got: {err}"
    );
}

#[test]
fn validate_extra_args_rejects_out_of_range_numeric_value() {
    let args = vec!["--max-uploaded-tiles".to_string(), "0".to_string()];
    validate_extra_args(&args).unwrap_err();

    let args = vec!["--time-of-day".to_string(), "25.0".to_string()];
    validate_extra_args(&args).unwrap_err();

    let args = vec!["--vegetation-density".to_string(), "5.0".to_string()];
    validate_extra_args(&args).unwrap_err();
}

#[test]
fn validate_extra_args_rejects_non_numeric_value_for_numeric_flag() {
    let args = vec!["--max-uploaded-mb".to_string(), "not-a-number".to_string()];
    validate_extra_args(&args).unwrap_err();
}

#[test]
fn validate_extra_args_rejects_screenshot_path_with_traversal() {
    let args = vec!["--screenshot".to_string(), "../escape.png".to_string()];
    validate_extra_args(&args).unwrap_err();
}

#[test]
fn validate_extra_args_rejects_unknown_visual_preset_value() {
    let args = vec!["--visual-preset".to_string(), "ultra".to_string()];
    validate_extra_args(&args).unwrap_err();
}

#[test]
fn validate_extra_args_rejects_value_flag_without_value() {
    let args = vec!["--width".to_string()];
    validate_extra_args(&args).unwrap_err();
}

#[test]
fn validate_extra_args_accepts_equals_form() {
    let args = vec!["--width=1280".to_string(), "--height=720".to_string()];
    validate_extra_args(&args).expect("equals-form flags should pass");
}

#[test]
fn validate_extra_args_rejects_negative_width_via_equals_form() {
    let args = vec!["--width=-100".to_string()];
    validate_extra_args(&args).unwrap_err();
}
