# Troubleshooting

Common issues, likely causes, and fixes for osm-world.

## Table of Contents

- [Build Failures](#build-failures)
- [Renderer Issues](#renderer-issues)
- [API Server Issues](#api-server-issues)
- [Web Explorer Issues](#web-explorer-issues)
- [Data and Cache Issues](#data-and-cache-issues)

## Build Failures

### Error: `no matching package named 'par-osm-rust' found`

**Symptom:** `cargo build` fails with a path dependency resolution error referencing `par-osm-rust`.

**Likely cause:** The vendored `par-osm-rust` workspace member is missing or the workspace is not being built from the repository root.

**Fix:** Build from the repository root so Cargo resolves `par-osm-rust` from its vendored path at `crates/par-osm-rust`:

```bash
make build
```

If you previously checked out an older revision that pointed at a sibling `../par-osm-rust`, run `git pull` and remove any stale `par-osm-rust` directory left next to `osm-world`. No sibling checkout is required.

**Verify:** `make build` completes without errors.

### Error: GPU adapter not found at runtime

**Symptom:** The renderer exits with "no suitable GPU adapter found" when launched.

**Likely cause:** The system GPU drivers do not support Vulkan, Metal, or DX12, or the drivers are outdated.

**Fix:** Update GPU drivers to the latest stable version. On Linux, verify Vulkan support with `vulkaninfo`.

**Verify:** The renderer window opens and displays the scene.

### Error: WGPU surface creation fails on Linux (X11 or Wayland)

**Symptom:** The renderer panics or logs a `wgpu` surface-creation error shortly after opening the window, typically with messages about `Unsupported` surface, `X11`, `Wayland`, or `Failed to create surface`.

**Likely cause:** WGPU could not negotiate a window-system surface with the running compositor. On Linux this is usually missing `vulkan-loader`, missing X11/Wayland development libraries, or running through a remote/SSH session without a display.

**Fix:** Install the X11/Wayland/Vulkan runtime libraries, then retry. On Debian/Ubuntu:

```bash
sudo apt-get install -y libxcb1-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libvulkan1 mesa-vulkan-drivers
```

Verify the loader can see the install:

```bash
vulkaninfo | head -n 20
```

If you are running over SSH, forward the display (`ssh -X` or set `DISPLAY`/`WAYLAND_DISPLAY`) or run the renderer on the local seat.

**Verify:** `make run` opens a visible window and the renderer logs the chosen adapter and surface format.

### Error: `bun install` fails with permission or peer-dependency errors

**Symptom:** `make web-install` (or `bun install` in `web/`) exits with `EPERM`, `EACCES`, or unresolved peer-dependency errors.

**Likely cause:** Leftover files from a previous install are owned by a different user (often caused by running inside a container or with `sudo` once), or the global Bun cache is corrupted. Peer-dependency mismatches usually trace back to a stale lockfile.

**Fix:** Reset the local web install state, then reinstall:

```bash
cd web
rm -rf node_modules ~/.bun/install/cache
bun install
```

If a peer-dependency error mentions a specific package (for example `postcss`), confirm `web/package.json` `overrides` still pins the version the rest of the tree expects.

**Verify:** `bun install` completes and `bun run build` succeeds.

## Renderer Issues

### Error: Scene vertex buffer exceeds GPU limit

**Symptom:** Renderer exits with a message about the vertex or index buffer exceeding the GPU buffer size limit.

**Likely cause:** The prepared area is too large or the visual detail is too high for the available GPU memory.

**Fix:** Use one or more of these approaches:

- Enable streaming: `--tile-size 1000 --stream-radius 15000`
- Reduce visual detail: `--visual-preset performance`
- Prepare a smaller bounding box through the Web Explorer
- Increase the upload budget: `--upload-budget-mb 8`

**Verify:** The renderer opens and displays the city scene without the buffer error.

### Scene appears empty or features are missing

**Symptom:** The renderer window opens but shows only terrain or sky.

**Likely cause:** Feature filters excluded all features, or the input file has no data for the visible area.

**Fix:** Check that at least one feature type is enabled in the prepare request. Verify the `.osm` file contains data by inspecting it with a text editor (it should contain `<node>`, `<way>`, and `<tag>` elements).

**Verify:** Buildings, roads, or other features appear in the scene.

### Renderer shows visual artifacts at tile boundaries

**Symptom:** Gaps or seams appear between tiles in the rendered scene.

**Likely cause:** Streaming tile selection with GPU buffer budget limits may skip distant tiles.

**Fix:** Reduce the area size, increase the upload budget, or disable streaming with `--no-streaming` to compare against the full-scene mesh.

**Verify:** Run with `--no-streaming` and confirm the artifacts disappear.

## API Server Issues

### Error: `Address already in use` on port 3030

**Symptom:** `make serve` or `make dev` fails because port 3030 is already bound.

**Likely cause:** Another process (or a previous server instance) is using the port.

**Fix:** Either stop the other process or bind to a different port:

```bash
cargo run -- --serve --port 3031
```

Update `NEXT_PUBLIC_OSM_WORLD_API_URL` in the web frontend to match.

**Verify:** The server starts and logs "area prepare API listening on http://...".

### Error: `unauthorized` on mutating endpoints

**Symptom:** POST or DELETE requests to the API return 401 Unauthorized.

**Likely cause:** The `OSM_WORLD_API_TOKEN` environment variable is set on the server, and the request does not include a matching `Authorization: Bearer <token>` header.

**Fix:** Either include the correct Bearer token in requests, or unset `OSM_WORLD_API_TOKEN` for local development.

**Verify:** The API accepts the request without a 401 response.

### Error: `rate limit exceeded` on prepare or launch

**Symptom:** API returns 429 Too Many Requests.

**Likely cause:** More than 20 requests were sent within 60 seconds from the same client.

**Fix:** Wait a minute and retry. Reduce automated request frequency.

**Verify:** Subsequent requests succeed.

## Web Explorer Issues

### Web Explorer cannot connect to the API

**Symptom:** The Web Explorer at `http://localhost:8032` shows connection errors or fails to load data.

**Likely cause:** The Rust API server is not running, or the API URL is misconfigured.

**Fix:** Start the API server first:

```bash
make serve
```

If the server runs on a non-default port, set the environment variable before starting the web dev server:

```bash
NEXT_PUBLIC_OSM_WORLD_API_URL=http://127.0.0.1:3031 make web-dev
```

**Verify:** The Web Explorer loads health status and cache areas.

### Overpass rate limiting

**Symptom:** Prepare requests fail with Overpass-related errors or warnings.

**Likely cause:** The Overpass API throttles requests from the same IP address.

**Fix:** Wait a few minutes before retrying. Use smaller bounding boxes. Disable "Force refresh" to reuse cached data. Set a custom Overpass URL pointing to a mirror.

**Verify:** The prepare request completes without Overpass errors.

## Data and Cache Issues

### Prepared area is stale after source settings change

**Symptom:** Changes to Overture themes or POI source mode do not take effect.

**Likely cause:** The prepared area is served from cache and was built with the previous settings.

**Fix:** Enable "Force refresh" in the Web Explorer, or delete the prepared area and re-prepare.

**Verify:** The new prepared area reflects the updated source settings.

### SRTM elevation tiles fail to download

**Symptom:** Elevation data is missing or the prepare request fails with SRTM download errors.

**Likely cause:** Network connectivity issues or the SRTM tile server is temporarily unavailable.

**Fix:** Retry after a short wait. Use a smaller bounding box to reduce the number of required tiles (the API limits to 16 tiles). Disable elevation if not needed.

**Verify:** The prepare request completes with an SRTM directory in the response.

### Cache directory growing too large

**Symptom:** The shared cache at `~/.cache/par-osm-rust/` occupies significant disk space.

**Likely cause:** Accumulated Overpass responses, prepared areas, and SRTM tiles.

**Fix:** Delete individual prepared areas through the Web Explorer. Remove raw Overpass cache files that are no longer needed. SRTM tiles can be safely deleted and will be re-downloaded on the next request that needs them.

**Verify:** Disk usage of the cache directory decreases.

## Related Documentation

- [Architecture](ARCHITECTURE.md) -- Module map, data flow, and API surface
- [CONTRIBUTING.md](../CONTRIBUTING.md) -- Development setup and check commands
