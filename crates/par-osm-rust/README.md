# par-osm-rust

Shared OpenStreetMap-compatible fetch, cache, parse, and normalization utilities.

`par-osm-rust` is the data-source crate used by `osm-to-bedrock` and `osm-world`.
It owns network and cache concerns only: OSM/Overpass fetching, optional
Overture Maps fetching, source merge policy, raw cache management, OSM XML/PBF
parsing, SRTM tile downloads, and HGT elevation lookup. It intentionally does
not depend on Minecraft, WGPU, UI frameworks, renderer types, or application UI
state.

## Modules

| Module      | Responsibility                                                              |
| ----------- | --------------------------------------------------------------------------- |
| `overpass`  | Build safe Overpass QL queries and fetch raw OSM XML.                       |
| `osm_cache` | Store URL-aware raw Overpass XML cache entries.                             |
| `overture`  | Invoke the optional `overturemaps` CLI and normalize GeoJSON.               |
| `sources`   | Merge OSM and Overture data with POI source policy and fallback.            |
| `osm`       | Parse PBF/XML and write normalized OSM XML.                                 |
| `srtm`      | Download and cache SRTM HGT tiles.                                          |
| `elevation` | Sample elevation from HGT tiles.                                            |
| `cache`     | Resolve shared cache directories and migrate legacy caches.                |
| `filter`    | Feature-type filter controlling which OSM categories are fetched/rendered.  |

## High-level source orchestration

Use `sources::fetch_map_data` when an application wants one shared path for
OSM/Overpass plus optional Overture Maps data:

```no_run
use par_osm_rust::filter::FeatureFilter;
use par_osm_rust::overture::{OvertureParams, OvertureTheme};
use par_osm_rust::sources::{
    fetch_map_data, OvertureFailureMode, PoiSourceMode, SourceOptions,
};

# fn main() -> anyhow::Result<()> {
let bbox = (38.0, -121.0, 38.01, -120.99); // south, west, north, east
let options = SourceOptions {
    filter: FeatureFilter::default(),
    overpass_url: None,
    use_overpass_cache: true,
    overture: OvertureParams {
        enabled: true,
        themes: vec![OvertureTheme::Place],
        ..OvertureParams::default()
    },
    poi_source_mode: PoiSourceMode::OverturePreferred,
    overture_failure_mode: OvertureFailureMode::FallbackToOsm,
};
let mut progress = |_: f32, _: &str| {};
let result = fetch_map_data(bbox, &options, &mut progress)?;
println!("source status: {:?}", result.status);
# Ok(())
# }
```

Important: `sources::PoiSourceMode::OverturePreferred` is the default POI
policy, but Overture is fetched only when `overture::OvertureParams::enabled`
is `true`. Default `sources::SourceOptions` performs an OSM/Overpass fetch
only.

## Lower-level parsing

For applications that already have raw OSM XML or PBF bytes on disk, the
`osm` module exposes standalone parsers:

```no_run
use par_osm_rust::osm;

let data = osm::parse_osm_xml_str(
    r#"<?xml version="1.0" encoding="UTF-8"?>
       <osm version="0.6">
         <node id="1" lat="38.0" lon="-121.0"/>
       </osm>"#,
)?;
# Ok::<(), anyhow::Error>(())
```

## License

MIT
