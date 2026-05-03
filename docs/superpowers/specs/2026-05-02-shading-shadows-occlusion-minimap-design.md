# Shading, Shadows, Occlusion Culling, and Minimap

**Date:** 2026-05-02
**Status:** Approved
**Decomposition:** Four independent subsystems, implemented in dependency order.

---

## 1. Better Shading (Blinn-Phong)

Replace the current Lambertian-only lighting in `city.wgsl` with Blinn-Phong (diffuse + specular + hemisphere ambient).

### Current state

```wgsl
let lighting = ambient + diffuse * (1.0 - ambient);
```

Flat ambient term, no specular, no directional ambient variation. Dark sides of buildings look uniformly dark.

### Hemisphere ambient

Blend between sky zenith color (from above) and ground color (from below) based on surface normal Y component. Reuses the existing `sky_color_zenith` from `SceneUniforms`:

```wgsl
let hemisphere = mix(scene.ground_color, scene.sky_zenith, normal.y * 0.5 + 0.5);
let ambient = hemisphere * scene.ambient_strength;
```

This replaces the flat `ambient_light` scalar. Upward-facing surfaces catch sky light; downward-facing surfaces get ground bounce. The `ambient_light` slider in the settings panel becomes `ambient_strength` (0.0-1.0).

### Diffuse

Same Lambertian `max(dot(N, L), 0.0)` as current. No change.

### Specular

Blinn-Phong using the half-vector `H = normalize(L + V)` where `V = normalize(camera_pos - world_position)`:

```wgsl
let view_dir = normalize(scene.camera_pos - in.world_position);
let half_vec = normalize(light_dir + view_dir);
let spec = pow(max(dot(normal, half_vec), 0.0), shininess);
let specular = specular_strength * spec * light_color;
```

### Per-feature material lookup

The `feature_type` attribute is already uploaded to the GPU but not interpolated to the fragment stage. Wire it through and use a small lookup:

| Feature | specular_strength | shininess |
|---------|------------------|-----------|
| TERRAIN | 0.0 | 1 |
| BUILDING | 0.15 | 32 |
| ROAD | 0.08 | 16 |
| WATER | 0.4 | 64 |
| LANDUSE | 0.0 | 1 |

Buildings get subtle specular, roads a slight sheen, water gets strong specular highlights, terrain and landuse get none.

### Changes

- `shaders/city.wgsl`: Wire `feature_type` through vertex output, add hemisphere ambient, add Blinn-Phong specular, add material lookup function.
- `src/camera/mod.rs` (`SceneUniforms`): Replace `ambient_light: f32` with `ambient_strength: f32` and `ground_color: [f32; 3]`. Add padding to maintain 16-byte alignment. The hemisphere ambient reuses `sky_zenith` already in the uniform buffer — no duplication.
- `src/atmosphere.rs`: Add `ground_color` field to `AtmosphereSettings`, default `[0.15, 0.12, 0.08]` (dark brown).
- `src/ui/settings.rs`: Add ground color picker to Sky Colors section.

### No Rust mesh generator changes

All existing mesh generation code (`building.rs`, `road.rs`, etc.) remains unchanged. The `feature_type` field is already set per vertex.

---

## 2. Shadows (Single Cascade Directional Shadow Map)

One extra render pass per frame rendering geometry from the sun's perspective into a depth-only texture. The city fragment shader samples this shadow map to determine if a fragment is in shadow.

### Shadow pass

Runs before the main render pass. Vertex-only pass (no fragment shader output), writes depth to a 2048x2048 `Depth32Float` texture with no color attachment.

**Shadow camera:** Orthographic projection fitted to the main camera's visible frustum as seen from the sun direction. The ortho box encloses a ~2000m region centered on the camera. Recomputed each frame from `sun_direction`.

**Depth bias:** `DepthBiasState { constant: 2, slope_scale: 1.0, clamp: 0.0 }` to prevent shadow acne (surface self-shadowing artifacts).

### Shadow bind group

New bind group at `@group(1)`:

- Binding 0: `texture_2d<f32>` — the 2048x2048 shadow depth map
- Binding 1: `sampler` — comparison sampler (`compare: LessEqual`, `filtering: Linear`) for PCF
- Binding 2: `mat4x4<f32>` — light view-projection matrix (uniform buffer)

### Shadow pipeline

Dedicated `ShadowPipeline` — same vertex shader as city pipeline (transforms world positions to light clip space), no fragment shader. Uses the same `Vertex` layout. `ColorWrites::EMPTY`, depth write enabled, depth test `Less`.

### City shader changes

New WGSL struct at `@group(1)`:

```wgsl
struct ShadowUniforms {
    light_view_proj: mat4x4<f32>,
};

@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var<uniform> shadow: ShadowUniforms;
```

**Shadow sampling:** Transform world position to light clip space, perform 4-tap PCF (2x2 bilinear PCF):

```wgsl
fn sample_shadow(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    let light_space = shadow.light_view_proj * vec4f(world_pos, 1.0);
    let ndc = light_space.xyz / light_space.w;
    let uv = ndc.xy * 0.5 + 0.5;
    let bias = max(0.002 * (1.0 - dot(normal, scene.sun_direction)), 0.001);
    // 4-tap PCF
    let texel_size = 1.0 / 2048.0;
    var shadow = 0.0;
    for (var x = -1; x <= 1; x += 2) {
        for (var y = -1; y <= 1; y += 2) {
            let offset = vec2f(f32(x), f32(y)) * texel_size;
            shadow += textureSampleCompare(shadow_map, shadow_sampler, uv + offset, ndc.z - bias);
        }
    }
    return shadow / 4.0;
}
```

**Lighting integration:** `diffuse = max(dot(N, L), 0.0) * shadow_factor`. Ambient (hemisphere) is unaffected — shadowed areas still receive ambient light.

### Changes

- New files: `src/render/shadow_pipeline.rs`, `src/render/shadow_bind_group.rs`
- `shaders/city.wgsl`: Add group(1) shadow bindings, `sample_shadow()`, integrate into lighting
- `shaders/shadow.wgsl` (new): Vertex-only shader transforming to light clip space
- `src/render/mod.rs`: Register new modules
- `src/render/pipelines.rs`: Update city pipeline to include group(1) layout
- `src/app/init.rs`: Create shadow pipeline, shadow bind group, shadow depth texture
- `src/app/render_loop.rs`: Add shadow pass before main pass, update main pass to bind shadow group
- `src/camera/mod.rs`: Add `light_view_proj` computation method
- `Cargo.toml`: No new dependencies needed

### Feature check

`Features::TEXTURE_COMPARISON_SAMPLER` may be needed for the comparison sampler. Verify at adapter request time, fall back to non-PCF sampling if unavailable.

---

## 3. Occlusion Culling (Hardware Occlusion Queries)

Use wgpu occlusion queries per tile to cull tiles fully hidden behind buildings or terrain.

### Occlusion pass

Runs after frustum culling, before the main render pass (or as part of the shadow pass, since both iterate tiles):

1. For each tile that passed frustum culling, render its axis-aligned bounding box as a simple 12-triangle solid cube with `ColorWrites::EMPTY` and depth writes disabled.
2. Wrap each box draw in `begin_occlusion_query(idx)` / `end_occlusion_query(idx)`.
3. The GPU counts how many samples pass the depth test.

### Query result readback

- `QuerySet` with `QueryType::Occlusion`, sized to `max_uploaded_tiles` (256 default).
- `resolve_query_set` writes results to a buffer each frame.
- Read **last frame's** results to decide what to render **this frame** (one-frame latency, standard approach, imperceptible).
- If a tile's query returns 0 visible samples, mark it occluded and skip rendering.
- A tile that becomes visible again (query returns > 0) gets re-enabled from already-loaded CPU mesh data.

### Integration with tile system

- Add `occluded: bool` field to per-tile state in `src/stream/`.
- During the tile update tick, skip mesh upload and render for occluded tiles.
- During the render loop, skip drawing occluded tiles in both main and minimap passes.

### Bounding box geometry

8 vertices, 12 triangles — a unit cube scaled to each tile's AABB. Generated once at init, instanced per tile with a model matrix push constant or per-draw uniform offset.

### Feature availability

`Features::OCCLUSION_QUERY` — check at adapter request time. If unavailable, fall back to frustum-only culling with no occlusion queries.

### Changes

- `src/render/occlusion.rs` (new): QuerySet management, bounding box geometry, occlusion pass rendering
- `src/stream/mod.rs`: Add `occluded: bool` to tile state
- `src/app/render_loop.rs`: Add occlusion query pass, skip occluded tiles during main draw
- `src/app/init.rs`: Create occlusion query set and result buffer

---

## 4. Live Rendered Minimap

Render a top-down view of the scene to a 256x256 off-screen texture each frame, display as an egui overlay in the bottom-right corner.

### Minimap camera

Separate orthographic camera centered on the player position, looking straight down (pitch = -90°). Adjustable radius (200m to 2000m) via mouse wheel over the minimap.

### Minimap render pass

Runs after the main pass, before egui:

- Renders to a dedicated 256x256 color texture + matching 256x256 `Depth32Float` depth texture.
- Uses the **same sky and city pipelines** with a different bind group (minimap camera uniforms).
- Renders all visible tiles (frustum + occlusion culled, same set as the main pass).
- Sky pass included for natural ground color background.

### Minimap bind group

A second `SceneBindGroup` with the minimap camera's view-projection and position. Shares the same atmosphere/day-cycle settings as the main camera.

### egui display

- Convert the minimap texture to `egui::TextureId` via `egui_wgpu::Renderer::native_texture()`.
- Draw in a fixed `egui::Area` anchored to `BOTTOM_RIGHT` with `[8.0, 8.0]` padding.
- Semi-transparent dark frame matching HUD style.
- **Player arrow:** White triangle drawn via `egui::Painter` centered on the minimap, rotated to match player yaw.
- **Zoom:** Mouse wheel over minimap adjusts ortho radius (200m-2000m). Stored as `minimap_zoom: f32` on app state.
- **Toggle:** M key shows/hides the minimap.

### Render order

```
Shadow pass -> Main pass (sky + city) -> Minimap pass (sky + city) -> Screenshot copy -> Egui pass (HUD + minimap + settings)
```

### Performance

256x256 (1/64th of 1080p) with the same geometry. Fragment shading is negligible. Vertex processing matches the main pass. If performance is a concern, the minimap can use Far LOD exclusively.

### Changes

- New files: `src/ui/minimap.rs`, `src/render/minimap.rs`
- `src/app/init.rs`: Create minimap render target textures, minimap bind group
- `src/app/render_loop.rs`: Add minimap render pass after main pass
- `src/app/event_handler.rs`: M key toggle, mouse wheel zoom forwarding
- `src/app/mod.rs`: Add `minimap_visible: bool`, `minimap_zoom: f32` to app state

---

## Implementation Order

1. **Better shading** — upgrades city.wgsl, foundational for shadows
2. **Shadows** — depends on updated lighting model
3. **Occlusion culling** — independent of shading/shadows but benefits from being after both
4. **Minimap** — independent, uses final rendering pipeline as-is

## Compatible Crate Versions

No new dependencies required. All features use existing wgpu 29, egui 0.34, winit 0.30.

## Key Reference: Vault Patterns

| Source | What to use |
|--------|-------------|
| `~/ClaudeVault/Research/fractals/fractal-flythroughs-05-lighting-shading.md` | Complete Blinn-Phong and Cook-Torrance GLSL implementations |
| `~/ClaudeVault/Patterns/depth-texture-storage-usage-in-re-flora.md` | Shadow map texture lifecycle (depth attachment + sampled) |
| `~/ClaudeVault/Projects/voxel-world/ui-features-hud-minimap.md` | Minimap implementation: egui-based, player markers, zoom |
| `~/ClaudeVault/Patterns/egui-wgpu-29-winit-030-integration.md` | egui native texture integration for minimap |
| `~/ClaudeVault/Projects/par-fractal/post-processing-pipeline.md` | Multi-pass wgpu render pass patterns |
| `~/ClaudeVault/Projects/par-fractal/uniform-buffer-sync.md` | WGSL 16-byte alignment rules |
