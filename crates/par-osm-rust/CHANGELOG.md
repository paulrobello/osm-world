# Changelog

All notable changes to `par-osm-rust` are documented in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `README.md` and `CHANGELOG.md` documenting the crate (ARC-008).
- Integration test `tests/osm_parse.rs` exercising the public `osm::parse_osm_xml_str`
  parser end-to-end without network access (ARC-008).

## [0.1.0] - 2026-07-19

### Added
- Initial release of the shared OSM/SRTM/cache utilities consumed by
  `osm-world` and `osm-to-bedrock`.
- Modules: `overpass`, `osm_cache`, `overture`, `sources`, `osm`, `srtm`,
  `elevation`, `cache`, `filter`.
