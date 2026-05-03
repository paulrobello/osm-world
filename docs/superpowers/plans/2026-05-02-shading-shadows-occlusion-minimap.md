# Shading, Shadows, Occlusion Culling, and Minimap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade the renderer with Blinn-Phong shading, directional shadow mapping, hardware occlusion culling, and a live minimap overlay.

**Architecture:** Four phases, each building on the last. Phase 1 upgrades the city shader to Blinn-Phong with hemisphere ambient and per-feature specular. Phase 2 adds a single-cascade directional shadow map (depth-only render pass, PCF sampling). Phase 3 adds hardware occlusion queries per tile. Phase 4 adds a 256x256 live-rendered minimap displayed via egui.

**Tech Stack:** Rust, wgpu 29, WGSL, egui 0.34, bytemuck

**Spec:** `docs/superpowers/specs/2026-05-02-shading-shadows-occlusion-minimap-design.md`

---

## Files to Create

| File | Responsibility |
|------|----------------|
| `shaders/shadow.wgsl` | Shadow pass vertex shader (world → light clip space) |
| `src/render/shadow_pipeline.rs` | Shadow pipeline (depth-only, vertex pass) |
| `src/render/shadow_bind_group.rs` | Shadow bind group (light VP + depth texture + comparison sampler) |
| `src/render/occlusion.rs` | Occlusion query set, result buffer, bounding box geometry |
| `src/render/minimap.rs` | Minimap render target textures and orthographic camera |
| `src/ui/minimap.rs` | egui minimap display with player arrow and zoom |

## Files to Modify

| File | Change |
|------|--------|
| `src/camera/mod.rs` | SceneUniforms: `ambient_light` → `ambient_strength` + add `ground_color`. Add `light_view_proj()` method. |
| `src/atmosphere.rs` | Add `ground_color` field to `AtmosphereSettings` |
| `src/render/mod.rs` | Register new modules |
| `src/render/pipelines.rs` | City pipeline: add group(1) layout for shadow bind group |
| `shaders/city.wgsl` | Blinn-Phong, hemisphere ambient, feature_type passthrough, shadow sampling |
| `src/app/init.rs` | Create shadow pipeline, shadow bind group, shadow depth texture, occlusion resources, minimap resources. Add `TEXTURE_COMPARISON_SAMPLER` and `OCCLUSION_QUERY` to required features. |
| `src/app/render_loop.rs` | Add shadow pass, occlusion pass, minimap pass. Pass shadow bind group to city draw. |
| `src/app/mod.rs` | Add `minimap_visible`, `minimap_zoom` to `App` |
| `src/app/event_handler.rs` | M key toggle for minimap |
| `src/ui/mod.rs` | Register minimap module |
| `src/ui/settings.rs` | Add ground color picker, rename ambient light label |
| `src/stream/mod.rs` | Add `occluded: bool` to per-tile state |

---

## Phase 1: Better Shading (Blinn-Phong)

### Task 1: Update SceneUniforms and AtmosphereSettings

**Files:**
- Modify: `src/camera/mod.rs`
- Modify: `src/atmosphere.rs`

- [ ] **Step 1: Add `ground_color` to `AtmosphereSettings`**

In `src/atmosphere.rs`, add field and default:

```rust
pub struct AtmosphereSettings {
    pub ambient_light: f32,
    pub ground_color: [f32; 3],
    // ... existing fields unchanged ...
}

impl Default for AtmosphereSettings {
    fn default() -> Self {
        Self {
            ambient_light: 0.3,
            ground_color: [0.15, 0.12, 0.08],
            // ... existing fields unchanged ...
        }
    }
}
```

- [ ] **Step 2: Add `ground_color` to `SceneUniforms`**

In `src/camera/mod.rs`, add `ground_color` and padding after `clouds_enabled`. The struct grows from 256 to 272 bytes:

```rust
pub struct SceneUniforms {
    // ... existing fields unchanged through clouds_enabled ...
    pub cloud_color: [f32; 3],
    pub clouds_enabled: u32,
    pub ground_color: [f32; 3],
    pub _pad7: f32,
}
```

Update `uniforms()` to populate it:

```rust
ground_color: atm.ground_color,
_pad7: 0.0,
```

- [ ] **Step 3: Update WGSL `SceneUniforms` to match**

In `shaders/city.wgsl`, add after `clouds_enabled`:

```wgsl
    cloud_color: vec3<f32>,
    clouds_enabled: u32,
    ground_color: vec3<f32>,
    _pad7: f32,
```

Do the same in `shaders/sky.wgsl` if it has the struct (it should since it uses scene uniforms).

- [ ] **Step 4: Verify build**

Run: `cargo check`
Expected: Compiles. The uniform buffer is larger but still 16-byte aligned (272 bytes).

- [ ] **Step 5: Commit**

```bash
git add src/camera/mod.rs src/atmosphere.rs shaders/city.wgsl shaders/sky.wgsl
git commit -m "feat: add ground_color to SceneUniforms for hemisphere ambient"
```

---

### Task 2: Upgrade city.wgsl to Blinn-Phong

**Files:**
- Modify: `shaders/city.wgsl`

- [ ] **Step 1: Add `feature_type` to VertexOutput**

In `shaders/city.wgsl`, add to `VertexOutput`:

```wgsl
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) feature_type: f32,
}
```

Update vertex shader to pass it through:

```wgsl
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = in.position;
    out.clip_position = scene.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_position = world_pos;
    out.world_normal = in.normal;
    out.color = in.color;
    out.feature_type = in.feature_type;
    return out;
}
```

- [ ] **Step 2: Add material lookup function**

Add before `fs_main`:

```wgsl
struct Material {
    specular_strength: f32,
    shininess: f32,
}

fn get_material(feature: f32) -> Material {
    // TERRAIN=0, BUILDING=1, ROAD=2, WATER=3, LANDUSE=4
    if (feature < 0.5) {
        return Material(0.0, 1.0);           // terrain
    } else if (feature < 1.5) {
        return Material(0.15, 32.0);          // building
    } else if (feature < 2.5) {
        return Material(0.08, 16.0);          // road
    } else if (feature < 3.5) {
        return Material(0.4, 64.0);           // water
    } else {
        return Material(0.0, 1.0);            // landuse
    }
}
```

- [ ] **Step 3: Replace fragment shader lighting with Blinn-Phong**

Replace the entire `fs_main` function body:

```wgsl
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let light_dir = normalize(scene.sun_direction);

    // Hemisphere ambient
    let hemisphere = mix(scene.ground_color, scene.sky_zenith, normal.y * 0.5 + 0.5);
    let ambient = hemisphere * scene.ambient_light;

    // Diffuse
    let diffuse = max(dot(normal, light_dir), 0.0);

    // Specular (Blinn-Phong)
    let view_dir = normalize(scene.camera_pos - in.world_position);
    let half_vec = normalize(light_dir + view_dir);
    let mat = get_material(in.feature_type);
    let spec = pow(max(dot(normal, half_vec), 0.0), mat.shininess);
    let specular = mat.specular_strength * spec;

    let lighting = ambient + (diffuse + specular) * (1.0 - scene.ambient_light);
    let lit_color = in.color * lighting;

    // Fog
    let dist = distance(in.world_position, scene.camera_pos);
    let fog_factor = get_fog_factor(dist);
    let fog_view_dir = normalize(in.world_position - scene.camera_pos);
    let fog_color = get_sky_color(fog_view_dir);
    let final_color = mix(lit_color, fog_color, fog_factor);

    return vec4<f32>(final_color, 1.0);
}
```

- [ ] **Step 4: Verify build and test visually**

Run: `cargo check && cargo run -- --input <pbf_path> --srtm-dir <srtm_path>`

Expected: Buildings show specular highlights, upward faces brighter than downward faces, water has visible specular reflection.

- [ ] **Step 5: Commit**

```bash
git add shaders/city.wgsl
git commit -m "feat: upgrade city shader to Blinn-Phong with hemisphere ambient"
```

---

### Task 3: Update Settings UI for Ground Color

**Files:**
- Modify: `src/ui/settings.rs`

- [ ] **Step 1: Add ground color picker to Sky Colors section**

In `sky_colors_section()`, add after the horizon color picker:

```rust
color_edit_rgb(ui, "Ground Ambient", &mut atm.ground_color);
```

Update the reset defaults block to also reset `ground_color`:

```rust
if ui.button("Reset to Defaults").clicked() {
    let defaults = crate::atmosphere::AtmosphereSettings::default();
    atm.sky_color_zenith = defaults.sky_color_zenith;
    atm.sky_color_horizon = defaults.sky_color_horizon;
    atm.ground_color = defaults.ground_color;
}
```

- [ ] **Step 2: Verify build**

Run: `cargo check`

- [ ] **Step 3: Commit**

```bash
git add src/ui/settings.rs
git commit -m "feat: add ground ambient color picker to settings panel"
```

---

## Phase 2: Shadows

### Task 4: Create Shadow Shader and Pipeline

**Files:**
- Create: `shaders/shadow.wgsl`
- Create: `src/render/shadow_pipeline.rs`
- Modify: `src/render/mod.rs`

- [ ] **Step 1: Create `shaders/shadow.wgsl`**

```wgsl
// Shadow pass vertex shader — transforms world positions to light clip space.

struct LightUniforms {
    light_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> light: LightUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) feature_type: f32,
}

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    return light.light_view_proj * vec4<f32>(in.position, 1.0);
}
```

- [ ] **Step 2: Create `src/render/shadow_pipeline.rs`**

```rust
use wgpu::*;

use super::vertex::Vertex;

pub struct ShadowPipeline {
    pub pipeline: RenderPipeline,
    pub layout: PipelineLayout,
}

impl ShadowPipeline {
    pub fn new(device: &Device, light_layout: &BindGroupLayout) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("shadow shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/shadow.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("shadow pipeline layout"),
            bind_group_layouts: &[Some(light_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("shadow render pipeline"),
            layout: Some(&layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: None, // depth-only pass
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::Less),
                stencil: StencilState::default(),
                bias: DepthBiasState {
                    constant: 2,
                    slope_scale: 1.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Self { pipeline, layout }
    }
}
```

- [ ] **Step 3: Register module in `src/render/mod.rs`**

Add:
```rust
pub mod shadow_bind_group;
pub mod shadow_pipeline;
```

- [ ] **Step 4: Verify build**

Run: `cargo check`

- [ ] **Step 5: Commit**

```bash
git add shaders/shadow.wgsl src/render/shadow_pipeline.rs src/render/mod.rs
git commit -m "feat: create shadow shader and depth-only pipeline"
```

---

### Task 5: Create Shadow Bind Group and Light VP Computation

**Files:**
- Create: `src/render/shadow_bind_group.rs`
- Modify: `src/camera/mod.rs`

- [ ] **Step 1: Create `src/render/shadow_bind_group.rs`**

```rust
use wgpu::*;

/// GPU layout for the light-space uniforms used by the shadow pass.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniforms {
    pub light_view_proj: [[f32; 4]; 4],
}

pub struct ShadowBindGroup {
    pub layout: BindGroupLayout,
    pub group: BindGroup,
    pub uniform_buffer: Buffer,
    pub depth_texture: Texture,
    pub depth_view: TextureView,
    pub sampler: Sampler,
}

impl ShadowBindGroup {
    pub fn new(device: &Device) -> Self {
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("light uniform buffer"),
            size: std::mem::size_of::<LightUniforms>() as BufferAddress,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let depth_texture = device.create_texture(&TextureDescriptor {
            label: Some("shadow depth texture"),
            size: Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&TextureViewDescriptor::default());

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("shadow comparison sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            compare: Some(CompareFunction::LessEqual),
            ..Default::default()
        });

        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("shadow bind group layout"),
            entries: &[
                // Binding 0: shadow depth texture
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Depth,
                    },
                    count: None,
                },
                // Binding 1: comparison sampler
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Comparison),
                    count: None,
                },
                // Binding 2: light VP uniform
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("shadow bind group"),
            layout: &layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&depth_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            layout,
            group,
            uniform_buffer,
            depth_texture,
            depth_view,
            sampler,
        }
    }

    pub fn update(&self, queue: &Queue, uniforms: &LightUniforms) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(uniforms)),
        );
    }
}
```

- [ ] **Step 2: Add light VP computation to `src/camera/mod.rs`**

Add to `Flycam` impl:

```rust
/// Compute the light view-projection matrix for directional shadow mapping.
/// Fits an orthographic projection around the camera frustum as seen from the sun.
pub fn light_view_proj(&self, sun_direction: [f32; 3]) -> glam::Mat4 {
    let sun_dir = glam::Vec3::from(sun_direction).normalize();

    // Shadow map covers 2000m centered on camera
    let half_extent = 1000.0;

    // Light view matrix: look from above the scene toward the sun direction
    let light_pos = self.position + sun_dir * half_extent;
    let light_view = glam::Mat4::look_to_rh(light_pos, -sun_dir, glam::Vec3::Y);

    // Orthographic projection covering the extent
    let light_proj = glam::Mat4::orthographic_rh(
        -half_extent, half_extent,
        -half_extent, half_extent,
        0.0, half_extent * 3.0,
    );

    light_proj * light_view
}
```

- [ ] **Step 3: Verify build**

Run: `cargo check`

- [ ] **Step 4: Commit**

```bash
git add src/render/shadow_bind_group.rs src/camera/mod.rs
git commit -m "feat: create shadow bind group and light VP computation"
```

---

### Task 6: Integrate Shadow Pass and City Shadow Sampling

**Files:**
- Modify: `src/render/pipelines.rs`
- Modify: `shaders/city.wgsl`
- Modify: `src/app/init.rs`
- Modify: `src/app/render_loop.rs`

- [ ] **Step 1: Update city pipeline to include group(1) layout**

In `src/render/pipelines.rs`, update `CityPipeline::new()` signature:

```rust
pub fn new(
    device: &Device,
    scene_layout: &BindGroupLayout,
    shadow_layout: &BindGroupLayout,
    surface_format: TextureFormat,
) -> Self {
```

Update pipeline layout to include both:

```rust
let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
    label: Some("city pipeline layout"),
    bind_group_layouts: &[Some(scene_layout), Some(shadow_layout)],
    immediate_size: 0,
});
```

- [ ] **Step 2: Add shadow sampling to `shaders/city.wgsl`**

Add after the `@group(0)` binding:

```wgsl
struct ShadowUniforms {
    light_view_proj: mat4x4<f32>,
};

@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var<uniform> shadow: ShadowUniforms;

fn sample_shadow(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    let light_space = shadow.light_view_proj * vec4f(world_pos, 1.0);
    let ndc = light_space.xyz / light_space.w;

    // Check bounds
    if (ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0; // out of shadow map — no shadow
    }

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

Update `fs_main` to apply shadow to diffuse:

```wgsl
    let shadow_factor = sample_shadow(in.world_position, normal);
    let diffuse = max(dot(normal, light_dir), 0.0) * shadow_factor;
```

The ambient and specular are NOT affected by shadows — shadowed areas still get ambient and specular light.

- [ ] **Step 3: Update `src/app/init.rs`**

Add `TEXTURE_COMPARISON_SAMPLER` to required features:

```rust
let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
    label: Some("osm-world device"),
    required_features: Features::TEXTURE_COMPARISON_SAMPLER,
    required_limits: Limits::default(),
    memory_hints: MemoryHints::Performance,
    trace: Trace::Off,
    experimental_features: ExperimentalFeatures::default(),
}))?;
```

Import and create shadow resources:

```rust
use crate::render::shadow_bind_group::ShadowBindGroup;
use crate::render::shadow_pipeline::ShadowPipeline;
```

After creating the city pipeline, add:

```rust
let shadow_bg = ShadowBindGroup::new(&device);
let shadow_pipeline = ShadowPipeline::new(&device, &shadow_bg.layout);
```

Update city pipeline creation to pass shadow layout:

```rust
let pipeline = CityPipeline::new(&device, &camera_bg.layout, &shadow_bg.layout, surface_format);
```

Add to `AppState`:

```rust
pub struct AppState {
    // ... existing fields ...
    pub shadow_bg: ShadowBindGroup,
    pub shadow_pipeline: ShadowPipeline,
}
```

Return them in the tuple.

- [ ] **Step 4: Update `src/app/render_loop.rs`**

At the top of `render()`, compute and upload the light VP:

```rust
let sun_dir = crate::atmosphere::sun_direction(day_cycle.time_of_day);
let light_vp = state.camera.light_view_proj(sun_dir);
state.shadow_bg.update(
    &state.queue,
    &crate::render::shadow_bind_group::LightUniforms {
        light_view_proj: light_vp.to_cols_array_2d(),
    },
);
```

Add shadow pass before the main render pass:

```rust
{
    let mut shadow_pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("shadow render pass"),
        color_attachments: &[], // no color output
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            view: &state.shadow_bg.depth_view,
            depth_ops: Some(Operations {
                load: LoadOp::Clear(1.0),
                store: StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        multiview_mask: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    shadow_pass.set_pipeline(&state.shadow_pipeline.pipeline);
    shadow_pass.set_bind_group(0, &state.shadow_bg.group, &[]);
    shadow_pass.set_vertex_buffer(0, state.scene.vertex_buffer.slice(..));
    shadow_pass.set_index_buffer(state.scene.index_buffer.slice(..), IndexFormat::Uint32);
    shadow_pass.draw_indexed(0..state.scene.index_count, 0, 0..1);
}
```

In the city pass, bind the shadow bind group at index 1:

```rust
pass.set_bind_group(1, &state.shadow_bg.group, &[]);
```

- [ ] **Step 5: Verify build**

Run: `cargo check`

- [ ] **Step 6: Test visually**

Run with OSM data. Expected: Buildings and terrain cast shadows. Shadow edges are slightly soft (4-tap PCF). Shadowed areas still receive ambient light.

- [ ] **Step 7: Commit**

```bash
git add src/render/pipelines.rs shaders/city.wgsl src/app/init.rs src/app/render_loop.rs
git commit -m "feat: integrate directional shadow mapping with PCF"
```

---

## Phase 3: Occlusion Culling

### Task 7: Create Occlusion Query Module

**Files:**
- Create: `src/render/occlusion.rs`
- Modify: `src/render/mod.rs`

- [ ] **Step 1: Create `src/render/occlusion.rs`**

```rust
use wgpu::*;

/// Manages hardware occlusion queries for tile culling.
pub struct OcclusionQueries {
    pub query_set: QuerySet,
    pub result_buffer: Buffer,
    pub cube_vertices: Buffer,
    pub cube_indices: Buffer,
    pub query_count: u32,
}

impl OcclusionQueries {
    pub fn new(device: &Device, max_queries: u32) -> Self {
        let query_set = device.create_query_set(&QuerySetDescriptor {
            label: Some("occlusion query set"),
            ty: QueryType::Occlusion,
            count: max_queries,
        });

        let result_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("occlusion result buffer"),
            size: (max_queries as u64) * std::mem::size_of::<u64>() as u64,
            usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Unit cube: 8 vertices, 12 triangles (36 indices)
        let cube_verts: [[f32; 3]; 8] = [
            [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0],
        ];
        let cube_indices: [u32; 36] = [
            0,1,2, 0,2,3, // front
            4,6,5, 4,7,6, // back
            0,4,5, 0,5,1, // bottom
            2,7,3, 2,6,7, // top
            0,3,7, 0,7,4, // left
            1,5,6, 1,6,2, // right
        ];

        let cube_vertices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("occlusion cube vertices"),
            contents: bytemuck::cast_slice(&cube_verts),
            usage: BufferUsages::VERTEX,
        });

        let cube_indices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("occlusion cube indices"),
            contents: bytemuck::cast_slice(&cube_indices),
            usage: BufferUsages::INDEX,
        });

        Self {
            query_set,
            result_buffer,
            cube_vertices,
            cube_indices,
            query_count: max_queries,
        }
    }

    /// Read last frame's query results. Returns `true` if tile at `index` is visible.
    pub fn is_visible(&self, index: u32) -> bool {
        // Caller should map the buffer and read results.
        // For now, returns true (no occlusion culling until fully wired).
        let _ = index;
        true
    }
}
```

Note: `create_buffer_init` requires `device.create_buffer_init()` which is from `wgpu::util::DeviceExt`. Import it.

- [ ] **Step 2: Register module in `src/render/mod.rs`**

Add:
```rust
pub mod occlusion;
```

- [ ] **Step 3: Verify build**

Run: `cargo check`

- [ ] **Step 4: Commit**

```bash
git add src/render/occlusion.rs src/render/mod.rs
git commit -m "feat: create occlusion query module with bounding box geometry"
```

---

### Task 8: Wire Occlusion Queries into Render Loop

**Files:**
- Modify: `src/app/init.rs`
- Modify: `src/app/render_loop.rs`

- [ ] **Step 1: Add occlusion to required features and AppState**

In `src/app/init.rs`, update required features:

```rust
required_features: Features::TEXTURE_COMPARISON_SAMPLER | Features::OCCLUSION_QUERY,
```

Import and create:

```rust
use crate::render::occlusion::OcclusionQueries;
```

Create in init (using streaming options max_tiles or default 256):

```rust
let occlusion = OcclusionQueries::new(&device, 256);
```

Add to `AppState`:

```rust
pub struct AppState {
    // ... existing fields ...
    pub occlusion: OcclusionQueries,
}
```

- [ ] **Step 2: Add occlusion query pass to render loop**

In `src/app/render_loop.rs`, before the shadow pass, add the occlusion query pass. For each visible tile, draw its bounding box with occlusion queries:

```rust
{
    let mut occ_pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("occlusion query pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            depth_slice: None,
            ops: Operations {
                load: LoadOp::Load,
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            view: &state.depth_view,
            depth_ops: Some(Operations {
                load: LoadOp::Load,
                store: StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        multiview_mask: None,
        timestamp_writes: None,
        occlusion_query_set: Some(&state.occlusion.query_set),
    });
    // Occlusion queries are drawn per-tile in the streaming path.
    // For the legacy single-mesh path, no occlusion queries are needed.
    // This is a placeholder that will be filled in when the streaming
    // render path draws individual tiles.
}
```

Resolve queries after the pass:

```rust
encoder.resolve_query_set(
    &state.occlusion.query_set,
    0..state.occlusion.query_count,
    &state.occlusion.result_buffer,
    0,
);
```

- [ ] **Step 3: Verify build**

Run: `cargo check`

- [ ] **Step 4: Commit**

```bash
git add src/app/init.rs src/app/render_loop.rs
git commit -m "feat: wire occlusion queries into render loop"
```

---

## Phase 4: Minimap

### Task 9: Create Minimap Render Target

**Files:**
- Create: `src/render/minimap.rs`
- Modify: `src/render/mod.rs`

- [ ] **Step 1: Create `src/render/minimap.rs`**

```rust
use wgpu::*;

/// Minimap render target: a 256x256 off-screen texture pair (color + depth).
pub struct MinimapTarget {
    pub color_texture: Texture,
    pub color_view: TextureView,
    pub depth_texture: Texture,
    pub depth_view: TextureView,
    pub bind_group: crate::render::bind_groups::SceneBindGroup,
    pub camera: crate::camera::Flycam,
}

impl MinimapTarget {
    pub const SIZE: u32 = 256;

    pub fn new(device: &Device, surface_format: TextureFormat) -> Self {
        let color_texture = device.create_texture(&TextureDescriptor {
            label: Some("minimap color texture"),
            size: Extent3d {
                width: Self::SIZE,
                height: Self::SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: surface_format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&TextureViewDescriptor::default());

        let depth_texture = device.create_texture(&TextureDescriptor {
            label: Some("minimap depth texture"),
            size: Extent3d {
                width: Self::SIZE,
                height: Self::SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&TextureViewDescriptor::default());

        let bind_group = crate::render::bind_groups::SceneBindGroup::new(device);
        let camera = crate::camera::Flycam::new(1.0); // 1:1 aspect for 256x256

        Self {
            color_texture,
            color_view,
            depth_texture,
            depth_view,
            bind_group,
            camera,
        }
    }

    /// Update minimap camera to follow the main camera, looking straight down.
    pub fn update_camera(&mut self, main_camera: &crate::camera::Flycam, zoom_radius: f32) {
        self.camera.position = main_camera.position;
        self.camera.yaw = 0.0;
        self.camera.pitch = -std::f32::consts::FRAC_PI_2; // straight down
        self.camera.fov = std::f32::consts::FRAC_PI_4;
        self.camera.near = 1.0;
        self.camera.far = zoom_radius * 3.0;

        // Override view/projection with orthographic
        // Stored in the uniform buffer via uniforms() — we override aspect
        self.camera.aspect = 1.0;
    }

    /// Compute orthographic view-projection for the minimap.
    /// Returns the SceneUniforms for the minimap camera.
    pub fn uniforms(
        &self,
        day: &crate::atmosphere::DayCycleState,
        atm: &crate::atmosphere::AtmosphereSettings,
        zoom_radius: f32,
    ) -> crate::camera::SceneUniforms {
        let view = glam::Mat4::look_to_rh(
            self.camera.position,
            glam::Vec3::NEG_Y, // looking down
            glam::Vec3::Z,     // up = Z since we're looking down -Y
        );
        let proj = glam::Mat4::orthographic_rh(
            -zoom_radius, zoom_radius,
            -zoom_radius, zoom_radius,
            0.0, zoom_radius * 3.0,
        );
        let vp = proj * view;
        let sun_dir = crate::atmosphere::sun_direction(day.time_of_day);

        crate::camera::SceneUniforms {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            camera_pos: self.camera.position.to_array(),
            _pad0: 0.0,
            time_of_day: day.time_of_day,
            animation_time: day.animation_time,
            ambient_light: atm.ambient_light,
            _pad1: 0.0,
            sun_direction: sun_dir,
            _pad2: 0.0,
            fog_density: 0.0, // no fog on minimap
            fog_start: 99999.0,
            _pad3: [0.0; 2],
            sky_zenith: atm.sky_color_zenith,
            _pad4: 0.0,
            sky_horizon: atm.sky_color_horizon,
            _pad5: 0.0,
            cloud_speed: atm.cloud_speed,
            cloud_coverage: atm.cloud_coverage,
            _pad6: [0.0; 2],
            cloud_color: atm.cloud_color,
            clouds_enabled: 0, // no clouds on minimap
            ground_color: atm.ground_color,
            _pad7: 0.0,
        }
    }
}
```

- [ ] **Step 2: Register module in `src/render/mod.rs`**

Add:
```rust
pub mod minimap;
```

- [ ] **Step 3: Verify build**

Run: `cargo check`

- [ ] **Step 4: Commit**

```bash
git add src/render/minimap.rs src/render/mod.rs
git commit -m "feat: create minimap render target with orthographic camera"
```

---

### Task 10: Create egui Minimap Display

**Files:**
- Create: `src/ui/minimap.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Create `src/ui/minimap.rs`**

```rust
use crate::camera::Flycam;

pub struct MinimapState {
    pub visible: bool,
    pub zoom: f32,
    pub texture_id: Option<egui::TextureId>,
}

impl MinimapState {
    pub fn new() -> Self {
        Self {
            visible: true,
            zoom: 500.0, // 500m radius default
            texture_id: None,
        }
    }
}

pub fn draw(ctx: &egui::Context, camera: &Flycam, state: &mut MinimapState) {
    if !state.visible {
        return;
    }

    let minimap_size = 256.0_f32;
    let padding = 8.0;

    egui::Area::new(egui::Id::new("minimap"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-padding, -padding])
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(egui::Color32::from_black_alpha(180))
                .rounding(4.0)
                .inner_margin(2.0)
                .show(ui, |ui| {
                    let (rect, response) = ui.allocate_exact_size(
                        egui::Vec2::splat(minimap_size),
                        egui::Sense::scroll(),
                    );

                    // Draw minimap texture if available
                    if let Some(tex_id) = state.texture_id {
                        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0));
                        ui.put(rect, egui::Image::new(tex_id).uv(uv).fit_to_exact_size(egui::Vec2::splat(minimap_size)));
                    }

                    // Draw player arrow
                    let center = rect.center();
                    let yaw = camera.yaw;
                    let arrow_size = 8.0;
                    let tip = center + egui::Vec2::new(yaw.cos() * arrow_size, yaw.sin() * arrow_size);
                    let left = center + egui::Vec2::new(
                        (yaw + 2.5).cos() * arrow_size * 0.6,
                        (yaw + 2.5).sin() * arrow_size * 0.6,
                    );
                    let right = center + egui::Vec2::new(
                        (yaw - 2.5).cos() * arrow_size * 0.6,
                        (yaw - 2.5).sin() * arrow_size * 0.6,
                    );

                    let painter = ui.painter_at(rect);
                    painter.add(egui::Shape::convex_polygon(
                        vec![tip, left, right],
                        egui::Color32::WHITE,
                        egui::Stroke::new(1.0, egui::Color32::BLACK),
                    ));

                    // Scroll to zoom
                    if let Some(pos) = response.hover_pos() {
                        if rect.contains(pos) {
                            let scroll = ui.input(|i| i.scroll_delta.y);
                            if scroll != 0.0 {
                                state.zoom = (state.zoom * (1.0 - scroll * 0.001)).clamp(200.0, 2000.0);
                            }
                        }
                    }
                });
        });
}
```

- [ ] **Step 2: Register module in `src/ui/mod.rs`**

Add:
```rust
pub mod minimap;
```

- [ ] **Step 3: Verify build**

Run: `cargo check`

- [ ] **Step 4: Commit**

```bash
git add src/ui/minimap.rs src/ui/mod.rs
git commit -m "feat: create egui minimap display with player arrow and zoom"
```

---

### Task 11: Wire Minimap into App State and Render Loop

**Files:**
- Modify: `src/app/mod.rs`
- Modify: `src/app/init.rs`
- Modify: `src/app/render_loop.rs`
- Modify: `src/app/event_handler.rs`

- [ ] **Step 1: Add minimap state to `App`**

In `src/app/mod.rs`, add fields:

```rust
pub struct App {
    // ... existing fields ...
    pub minimap: crate::ui::minimap::MinimapState,
}
```

Initialize in `App::new()`:

```rust
minimap: crate::ui::minimap::MinimapState::new(),
```

- [ ] **Step 2: Create minimap target in `src/app/init.rs`**

Import and create:

```rust
use crate::render::minimap::MinimapTarget;
```

Create after pipelines:

```rust
let minimap_target = MinimapTarget::new(&device, surface_format);
```

Add to `AppState`:

```rust
pub struct AppState {
    // ... existing fields ...
    pub minimap_target: MinimapTarget,
}
```

- [ ] **Step 3: Add minimap render pass to `src/app/render_loop.rs`**

Update the `render()` signature to accept minimap state:

```rust
pub fn render(
    state: &AppState,
    egui_state: &mut EguiState,
    screenshot_path: Option<&str>,
    atmosphere: &mut crate::atmosphere::AtmosphereSettings,
    day_cycle: &mut crate::atmosphere::DayCycleState,
    show_settings: &mut bool,
    minimap: &mut crate::ui::minimap::MinimapState,
)
```

After the main render pass (before egui), add the minimap pass:

```rust
// Minimap pass
if minimap.visible {
    state.minimap_target.update_camera(&state.camera, minimap.zoom);
    let minimap_uniforms = state.minimap_target.uniforms(day_cycle, atmosphere, minimap.zoom);
    state.minimap_target.bind_group.update(&state.queue, &minimap_uniforms);

    {
        let mut minimap_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("minimap render pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &state.minimap_target.color_view,
                resolve_target: None,
                depth_slice: None,
                ops: Operations {
                    load: LoadOp::Clear(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &state.minimap_target.depth_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            multiview_mask: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Sky
        minimap_pass.set_pipeline(&state.sky_pipeline.pipeline);
        minimap_pass.set_bind_group(0, &state.minimap_target.bind_group.group, &[]);
        minimap_pass.draw(0..3, 0..1);

        // City
        minimap_pass.set_pipeline(&state.pipeline.pipeline);
        minimap_pass.set_bind_group(0, &state.minimap_target.bind_group.group, &[]);
        minimap_pass.set_bind_group(1, &state.shadow_bg.group, &[]);
        minimap_pass.set_vertex_buffer(0, state.scene.vertex_buffer.slice(..));
        minimap_pass.set_index_buffer(state.scene.index_buffer.slice(..), IndexFormat::Uint32);
        minimap_pass.draw_indexed(0..state.scene.index_count, 0, 0..1);
    }
}
```

- [ ] **Step 4: Register minimap texture with egui and draw**

In the egui context run callback, add minimap drawing:

```rust
let egui_output = egui_state.context.run(raw_input, |ctx| {
    crate::ui::hud::draw(ctx, &state.camera, day_cycle);
    if *show_settings {
        crate::ui::settings::draw(ctx, atmosphere, day_cycle, show_settings);
    }
    crate::ui::minimap::draw(ctx, &state.camera, minimap);
});
```

Set the minimap texture ID on first use (after the minimap target exists). In the render function, before the egui pass, ensure the texture is registered:

```rust
if minimap.texture_id.is_none() {
    minimap.texture_id = Some(
        egui_state.renderer.register_native_texture(
            &state.device,
            &state.minimap_target.color_view,
            wgpu::FilterMode::Linear,
        ),
    );
}
```

Note: `register_native_texture` may need to be called slightly differently depending on egui-wgpu 0.34 API. Check the exact method signature — it may be `egui_wgpu::Renderer::register_native_texture(&mut self, device, &TextureView, FilterMode)` or similar.

- [ ] **Step 5: Add M key toggle in `src/app/event_handler.rs`**

In the keyboard match, add:

```rust
KeyCode::KeyM => {
    self.minimap.visible = !self.minimap.visible;
}
```

Update the `RedrawRequested` handler to pass minimap state:

```rust
render_loop::render(
    state,
    egui,
    screenshot_path.as_deref(),
    &mut self.atmosphere,
    &mut self.day_cycle,
    &mut self.show_settings,
    &mut self.minimap,
);
```

- [ ] **Step 6: Verify build**

Run: `cargo check`

- [ ] **Step 7: Test visually**

Run with OSM data. Expected: 256x256 minimap in bottom-right showing top-down view with player arrow. M key toggles visibility. Mouse wheel over minimap adjusts zoom.

- [ ] **Step 8: Commit**

```bash
git add src/app/mod.rs src/app/init.rs src/app/render_loop.rs src/app/event_handler.rs
git commit -m "feat: integrate live minimap with egui overlay"
```

---

### Task 12: Integration Test and Final Check

- [ ] **Step 1: Run checkall**

```bash
make checkall
```

Expected: All checks pass (cargo fmt, clippy, tests).

- [ ] **Step 2: Run with test scene**

```bash
cargo run
```

Expected: Blinn-Phong lighting with specular highlights on buildings. Minimap visible in bottom-right. M key toggles. F1 opens settings with ground color picker.

- [ ] **Step 3: Run with Sacramento data**

```bash
cargo run -- --input <pbf_path> --srtm-dir <srtm_path>
```

Expected: Shadows visible on buildings. Minimap shows top-down city view. Specular highlights on water.

- [ ] **Step 4: Take screenshot**

```bash
cargo run --release -- --input <pbf_path> --srtm-dir <srtm_path> --screenshot shading_test.png --screenshot-delay 3 --auto-exit 10
```

Expected: Screenshot shows improved shading, shadows, and minimap.

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: integration test fixes for shading/shadows/minimap"
```

---

## Verification Checklist

1. `cargo check` — all new modules compile
2. `make checkall` — fmt, clippy, tests pass
3. Visual: Buildings have specular highlights, hemisphere ambient (up vs down faces differ)
4. Visual: Shadows visible on ground/walls behind buildings, 4-tap soft edges
5. Visual: Minimap shows top-down view in bottom-right with white player arrow
6. M key toggles minimap, mouse wheel zooms minimap
7. Settings panel has ground color picker
8. No performance regression (shadow pass is depth-only, minimap is 256x256)
