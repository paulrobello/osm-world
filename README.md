# osm-world

![CI](https://github.com/paulrobello/osm-world/actions/workflows/ci.yml/badge.svg)
![Runs on Linux | MacOS | Windows](https://img.shields.io/badge/runs%20on-Linux%20%7C%20MacOS%20%7C%20Windows-blue)
![Arch x86-64 | ARM | AppleSilicon](https://img.shields.io/badge/arch-x86--64%20%7C%20ARM%20%7C%20AppleSilicon-blue)
![License](https://img.shields.io/badge/license-MIT-green)

Render real-world cities in 3D from [OpenStreetMap](https://www.openstreetmap.org/) data. `osm-world` is a Rust and WGPU desktop renderer with optional SRTM elevation, OpenStreetMap/Overture-backed area preparation, and a browser-based Web Explorer for selecting areas and launching repeatable renderer commands.

## Table of Contents

- [Getting Started](#getting-started)
- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Command-Line Options](#command-line-options)
- [Web Explorer](#web-explorer)
- [Documentation](#documentation)
- [Getting OSM Data](#getting-osm-data)
- [Architecture](#architecture)
- [Known Limitations](#known-limitations)
- [Contributing](#contributing)
- [License](#license)
- [Author](#author)
- [Links](#links)

## Getting Started

New to osm-world? Here are the quickest paths to a rendered city:

- **[Installation](#installation)** — Build from source and install web dependencies
- **[Quick Start](#quick-start)** — Open the renderer or load a local `.osm.pbf` / `.osm` file
- **[Web Explorer](#web-explorer)** — Draw a bounding box on a live map, prepare data, and launch the renderer
- **[Architecture](docs/ARCHITECTURE.md)** — Learn how the renderer, API server, cache, and web UI fit together

## Features

### 3D Renderer

- WGPU renderer with Winit desktop windowing
- Flycam navigation with saved camera preferences and explicit spawn coordinates
- Terrain, land use, closed water polygons, open waterway ribbons, roads, railways, buildings, transit paths, street signs, addresses, points of interest, and landmarks
- Sky, day-cycle lighting, cascaded shadow maps, contact shadows, minimap rendering, and HUD overlays
- Screenshot automation for repeatable visual checks

### Real-World Data Pipeline

- Parses `.osm.pbf`, `.pbf`, and `.osm` XML inputs
- Prepares map areas through the shared `par-osm-rust` cache and data-source crate
- Supports Overpass and Overture Maps source controls through the prepare API and Web Explorer
- Downloads SRTM elevation tiles when elevation is enabled
- Persists prepared `.osm` files and metadata under the shared cache root

### Web Explorer

- Browser-based map picker built with Next.js and OpenLayers
- Bounding-box presets, manual bbox input, and spawn-point selection
- Prepared-area history with rename, favorite, reload, and delete actions
- Renderer settings profiles for visual preset, time of day, streaming radius, labels, and minimap behavior
- Copyable debug, release, screenshot, and no-streaming command variants
- Optional renderer launch through the local Rust API server

### Development and Validation

- Make targets for build, release, run, serve, web dev, formatting, linting, type checking, and tests
- Rust unit tests for camera behavior, CLI parsing, streaming helpers, shader source validation, world generation, and UI state
- Web tests for settings profiles, command variants, bbox presets, and error hints

## Installation

### From Source

Requires Rust 1.92 (the `rust-version` declared in `Cargo.toml`).

```bash
git clone https://github.com/paulrobello/osm-world
cd osm-world
make build
# binary at target/debug/osm-world
```

The `par-osm-rust` data-source crate is vendored in-tree at `crates/par-osm-rust` and builds as part of the workspace, so no sibling checkout is required.

### Web Dependencies

```bash
make web-install
```

### Prerequisites

| Tool | Notes |
| --- | --- |
| Rust | Required for the renderer and API server. Use Rust 1.92 (the `rust-version` in `Cargo.toml`). |
| Bun | Required only for the Web Explorer. Tested with Bun 1.3.x. |

> **Note:** On macOS (especially under tmux), prefer `make run-app` over `make run` to launch the renderer. `run-app` opens the binary directly, which lets the window receive keyboard focus; the `cargo run` path used by `make run` can launch without focus under some terminal/tmux configurations.

## Quick Start

```bash
# Open the renderer with the built-in test scene.
# On macOS/tmux, prefer `make run-app` so the window receives keyboard focus.
make run

# Render a local OpenStreetMap extract
cargo run -- --input city.osm.pbf

# Render a prepared .osm file with SRTM elevation data
cargo run -- \
  --input ~/.cache/par-osm-rust/prepared/<cache-key>.osm \
  --srtm-dir ~/.cache/par-osm-rust/srtm

# Start the Rust API server and Web Explorer together
make dev
```

Open the Web Explorer at `http://localhost:8032`. The Rust API server listens on `http://127.0.0.1:3030` by default.

## Configuration

Most runtime configuration is passed through CLI flags or the Web Explorer renderer profile controls.

Useful environment variables:

| Variable | Purpose |
| --- | --- |
| `NEXT_PUBLIC_OSM_WORLD_API_URL` | Web frontend API base URL. Defaults to `http://127.0.0.1:3030`. |
| `PAR_OSM_OVERPASS_CACHE_DIR` | Override the shared Overpass cache directory used by `par-osm-rust`. |
| `PAR_OSM_SRTM_CACHE_DIR` | Override the shared SRTM cache directory used by `par-osm-rust`. |
| `OVERPASS_URL` | Override the Overpass endpoint used by the vendored `par-osm-rust` crate for source preparation. Read at process start; changes require a restart. |

## Command-Line Options

Run `cargo run -- --help` for the complete option list.

### Renderer Mode

```bash
cargo run -- \
  --input city.osm.pbf \
  --spawn-lat 38.65671 \
  --spawn-lon -121.72179 \
  --visual-preset showcase \
  --time-of-day 16.5
```

Common renderer options:

| Option | Purpose |
| --- | --- |
| `--input <path>` | Load a `.osm.pbf`, `.pbf`, or `.osm` source file. |
| `--srtm-dir <path>` | Use SRTM elevation tiles from a cache directory. |
| `--spawn-lat <lat> --spawn-lon <lon>` | Place the initial camera near a geographic coordinate. |
| `--cam-x`, `--cam-y`, `--cam-z` | Override the initial camera position in world space. |
| `--cam-yaw`, `--cam-pitch` | Override the initial camera yaw and pitch in degrees. |
| `--show-settings` | Start with the in-app settings panel open. |
| `--time-of-day <hours>` | Set lighting time, where `12` is noon. |
| `--real-time-of-day` | Sync lighting to the local wall clock. |
| `--visual-preset performance\|balanced\|showcase` | Select renderer detail defaults. |
| `--landmark-detail <level>` | Override landmark rendering detail. |
| `--facade-variation <0.0..=1.0>` | Building facade variation multiplier. |
| `--roof-variation <0.0..=1.0>` | Building roof variation multiplier. |
| `--vegetation-density <0.0..=3.0>` | Vegetation density multiplier. |
| `--synthetic-tree-cap <n>` | Maximum number of synthetic trees per tile. |
| `--vegetation-distance <metres>` | Maximum vegetation draw distance. |
| `--debug-shadow-cascades` | Tint geometry by shadow cascade for debugging. |
| `--hide-poi-labels`, `--hide-address-labels`, `--hide-street-sign-labels` | Hide label layers at startup. |
| `--hide-minimap`, `--rotate-minimap` | Control minimap startup behavior. |
| `--max-uploaded-tiles <n>` | Maximum number of streaming tiles uploaded to the GPU. |
| `--max-uploaded-mb <MiB>` | Maximum estimated uploaded tile memory. |

For the full flag list including streaming and tile-debug options, see the [Visual Detail Controls design](docs/superpowers/specs/2026-05-06-osm-world-visual-detail-controls-design.md) or run `cargo run -- --help`.

### Screenshot Mode

```bash
cargo run --release -- \
  --input city.osm.pbf \
  --screenshot screenshots/city.png \
  --screenshot-delay 5 \
  --auto-exit 8
```

### API Server Mode

```bash
cargo run -- --serve --host 127.0.0.1 --port 3030
```

The server exposes health, cache, prepared-area, area-prepare, and renderer-launch endpoints used by the Web Explorer.

### Streaming and Tile Debug Options

```bash
cargo run -- \
  --input city.osm.pbf \
  --tile-size 1000 \
  --stream-radius 15000 \
  --upload-budget-mb 4
```

Use `--no-streaming` to compare against the legacy single-mesh launch path exposed by the Web Explorer command variants.

## Web Explorer

The Web Explorer is a browser UI for selecting an area, preparing renderer input files, and launching or copying renderer commands.

```bash
# First time: install web dependencies
make web-install

# Start both the Rust API server and the web UI
make dev
```

Visit `http://localhost:8032` in a browser.

**Features:**

- Interactive OpenLayers map with bbox drawing and spawn-point selection
- Feature toggles for roads, buildings, water, land use, and railways
- Overpass and Overture source controls
- SRTM elevation toggle and force-refresh option
- Cache area overlay and prepared-area history
- Renderer profiles with visual preset, time of day, streaming, label, and minimap settings
- Copyable debug, release, screenshot, and no-streaming commands
- Local renderer launch through `POST /renderer/launch`

## Documentation

- **[Architecture](docs/ARCHITECTURE.md)** — Runtime modes, module map, data flow, renderer, API server, web UI, and design tradeoffs
- **[Troubleshooting](docs/troubleshooting.md)** — Build, renderer, API server, web, data, and cache failure modes with symptoms and fixes
- **[Changelog](CHANGELOG.md)** — Notable changes per release
- **[Contributing](CONTRIBUTING.md)** — Development setup, code style, tests, and pull request process
- **[Superpowers Specs and Plans Index](docs/superpowers/README.md)** — Historical design specs and implementation plans retained for reference
- **[Initial 3D Engine Design](docs/superpowers/specs/2026-05-01-osm-world-3d-engine-design.md)** — Original renderer architecture and mesh-generation plan
- **[Streaming and LOD Design](docs/superpowers/specs/2026-05-02-phase3-streaming-lod-design.md)** — Tile streaming and level-of-detail direction
- **[Shared Cache and Web Picker Design](docs/superpowers/specs/2026-05-03-shared-osm-cache-and-streaming-design.md)** — `par-osm-rust` cache contract and prepare workflow
- **[Visual Detail Controls Design](docs/superpowers/specs/2026-05-06-osm-world-visual-detail-controls-design.md)** — Visual presets, landmarks, facade variation, vegetation, and screenshot validation
- **[Documentation Style Guide](docs/DOCUMENTATION_STYLE_GUIDE.md)** — Formatting, structure, tone, and maintenance standards for project documentation

## Getting OSM Data

You can use the Web Explorer to fetch an area directly, or download local extracts from:

- [Geofabrik](https://download.geofabrik.de/) — continent, country, and region extracts
- [BBBike](https://extract.bbbike.org/) — custom bounding-box extracts
- [Overpass Turbo](https://overpass-turbo.eu/) — query and export OpenStreetMap XML

Small city extracts are the best starting point while tuning visuals and performance.

## Architecture

Parse source data → project coordinates → classify world features → build a capped startup mesh or no-streaming full mesh → validate and upload WGPU buffers → render the desktop scene.

The CLI also includes an Axum HTTP server powering the Web Explorer, with Overpass and Overture data fetching, disk caching, renderer launch, and SRTM elevation support.

See **[Architecture](docs/ARCHITECTURE.md)** for the full module map, data flow, API surface, and current streaming boundaries.

## Known Limitations

- **Projection model:** The renderer uses an equirectangular projection suited to city-scale areas; distortion increases across large regions and high latitudes.
- **Prepared-area size:** The API validates bbox size and limits SRTM tile counts to keep local preparation practical.
- **Streaming state:** Startup uses tile selection and GPU buffer caps, but runtime rendering still uploads one `SceneBuffers` allocation until incremental tile uploads are implemented.
- **Large-region detail:** Distant startup tiles can be skipped when the selected mesh would exceed the GPU buffer budget. Use smaller areas or lower visual detail for dense regions.

## Contributing

Before submitting changes, run the relevant checks:

```bash
make build      # Debug build
make test       # Run Rust tests
make lint       # cargo clippy
make fmt        # rustfmt check
make typecheck  # cargo check
make web-build  # Build the Next.js frontend
make checkall   # fmt + typecheck + lint + test
make clean      # cargo clean
make dev        # Start both Rust API + Web Explorer
make serve      # Start Rust API server only
```

For web-only changes, also run:

```bash
cd web
bun test
bun run build
```

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for the full text. The license is also declared in `Cargo.toml`.

## Author

Paul Robello - probello@gmail.com

## Links

- **GitHub**: [https://github.com/paulrobello/osm-world](https://github.com/paulrobello/osm-world)
- **OpenStreetMap**: [https://www.openstreetmap.org/](https://www.openstreetmap.org/)
- **Overpass API**: [https://overpass-api.de/](https://overpass-api.de/)
- **Overture Maps**: [https://overturemaps.org/](https://overturemaps.org/)
