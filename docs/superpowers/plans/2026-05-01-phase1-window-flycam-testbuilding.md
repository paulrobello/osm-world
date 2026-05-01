# Phase 1: Window + Flycam + Test Building — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Open a wgpu 29 window, render a colored box (building) on a flat ground plane, and navigate with a WASD flycam.

**Architecture:** winit 0.30 ApplicationHandler trait drives the event loop. wgpu 29 handles GPU initialization and rendering. A single render pipeline with a city.wgsl shader draws indexed geometry. Camera uniform buffer updated per-frame. Hardcoded test geometry proves the full pipeline from vertex data to pixels.

**Tech Stack:** Rust (edition 2024), wgpu 29.0.1, winit 0.30.13, glam 0.32.1, bytemuck 1.25.0

**Spec:** `docs/superpowers/specs/2026-05-01-osm-world-3d-engine-design.md` — Phase 1

---

## File Structure

| File | Purpose |
|------|---------|
| `Cargo.toml` | Dependencies (wgpu 29, winit 0.30, glam, bytemuck, pollster, clap, anyhow, log, env_logger) |
| `Makefile` | Standard targets: build, run, test, lint, fmt, typecheck, checkall |
| `.gitignore` | Rust + IDE ignores |
| `src/main.rs` | CLI entry point, launches App |
| `src/lib.rs` | Re-exports all public modules |
| `src/app/mod.rs` | App struct: owns window, device, renderer, camera, state |
| `src/app/init.rs` | wgpu init: instance, surface, device, queue, surface config |
| `src/app/render_loop.rs` | Render pass: clear, draw geometry, present |
| `src/app/update.rs` | Per-frame: update camera from input, request redraw |
| `src/app/event_handler.rs` | winit ApplicationHandler impl |
| `src/camera/mod.rs` | Flycam struct: position, yaw, pitch, matrices |
| `src/camera/controller.rs` | Input state tracking, camera movement/rotation |
| `src/render/mod.rs` | Renderer struct: pipeline, bind groups, geometry buffers |
| `src/render/vertex.rs` | Vertex struct (32 bytes, bytemuck) |
| `src/render/pipelines.rs` | WGSL shader + wgpu render pipeline creation |
| `src/render/bind_groups.rs` | Camera uniform buffer + bind group |
| `src/render/buffers.rs` | Vertex/index buffer creation for test geometry |
| `src/shaders/city.wgsl` | Vertex transform + directional light + vertex color |
| `tests/camera_test.rs` | Unit tests for Flycam matrices |

---

### Task 1: Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `Makefile`
- Create: `.gitignore`
- Create: `src/main.rs`
- Create: `src/lib.rs`

- [ ] **Step 1: Initialize git repo and create Cargo.toml**

```bash
cd /Users/probello/Repos/osm-world
git init
```

Create `Cargo.toml`:

```toml
[package]
name = "osm-world"
version = "0.1.0"
edition = "2024"
rust-version = "1.87"
description = "3D city renderer using OpenStreetMap data and WGPU"
authors = ["Paul Robello <probello@gmail.com>"]
license = "MIT"

[dependencies]
wgpu = "29.0.1"
winit = "0.30.13"
glam = "0.32.1"
bytemuck = { version = "1.25.0", features = ["derive"] }
pollster = "0.4"
anyhow = "1.0"
log = "0.4"
env_logger = "0.11"

[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

- [ ] **Step 2: Create .gitignore**

```
/target
*.swp
*.swo
*~
.DS_Store
.idea/
.vscode/
```

- [ ] **Step 3: Create src/main.rs (minimal entry point)**

```rust
fn main() -> anyhow::Result<()> {
    env_logger::init();
    log::info!("osm-world starting");
    println!("osm-world: 3D OSM city renderer");
    println!("Run with --help for usage");
    Ok(())
}
```

- [ ] **Step 4: Create src/lib.rs (empty)**

```rust
// osm-world: 3D city renderer using OpenStreetMap data and WGPU
```

- [ ] **Step 5: Create Makefile**

```makefile
.PHONY: build run test lint fmt typecheck checkall clean

build:
	cargo build

run:
	cargo run

test:
	cargo test

lint:
	cargo clippy -- -D warnings

fmt:
	cargo fmt -- --check

typecheck:
	cargo check

checkall: fmt typecheck lint test

clean:
	cargo clean
```

- [ ] **Step 6: Build and verify**

Run: `cargo build`
Expected: compiles successfully with no errors

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: project scaffold with Cargo.toml, Makefile, minimal main"
```

---

### Task 2: Vertex and Camera Data Types

**Files:**
- Create: `src/render/mod.rs`
- Create: `src/render/vertex.rs`
- Create: `src/camera/mod.rs`
- Create: `tests/camera_test.rs`

- [ ] **Step 1: Create src/render/mod.rs**

```rust
pub mod vertex;
pub mod pipelines;
pub mod bind_groups;
pub mod buffers;
```

- [ ] **Step 2: Create src/render/vertex.rs**

```rust
use bytemuck::{Pod, Zeroable};

/// GPU vertex format. 32 bytes per vertex.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub feature_type: f32,
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x3,
        3 => Float32,
    ];
}

/// Feature type constants for shader branching.
pub mod feature {
    pub const TERRAIN: f32 = 0.0;
    pub const BUILDING: f32 = 1.0;
    pub const ROAD: f32 = 2.0;
    pub const WATER: f32 = 3.0;
    pub const LANDUSE: f32 = 4.0;
}
```

- [ ] **Step 3: Create src/camera/mod.rs**

```rust
use bytemuck::{Pod, Zeroable};

/// Camera uniform buffer layout (GPU). 68 bytes, padded to 80.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 3],
    pub _pad: f32,
}

/// Flycam: free-flight camera controlled by WASD + mouse.
pub struct Flycam {
    pub position: glam::Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Flycam {
    pub fn new(aspect: f32) -> Self {
        Self {
            position: glam::Vec3::new(0.0, 50.0, 100.0),
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: -0.3,
            speed: 100.0,
            fov: std::f32::consts::FRAC_PI_4,
            aspect,
            near: 0.5,
            far: 50000.0,
        }
    }

    pub fn forward(&self) -> glam::Vec3 {
        glam::Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    pub fn right(&self) -> glam::Vec3 {
        glam::Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos()).normalize()
    }

    pub fn view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::look_to_rh(self.position, self.forward(), glam::Vec3::Y)
    }

    pub fn projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    pub fn uniform(&self) -> CameraUniform {
        CameraUniform {
            view_proj: (self.projection_matrix() * self.view_matrix()).to_cols_array_2d(),
            position: self.position.to_array(),
            _pad: 0.0,
        }
    }
}
```

- [ ] **Step 4: Write camera tests in tests/camera_test.rs**

```rust
use glam::Vec3;

fn build_camera() -> osm_world::camera::Flycam {
    osm_world::camera::Flycam::new(1.0)
}

#[test]
fn forward_vector_is_normalized() {
    let cam = build_camera();
    let len = cam.forward().length();
    assert!((len - 1.0).abs() < 0.001, "forward length = {len}");
}

#[test]
fn forward_is_horizontal_when_pitch_zero() {
    let mut cam = build_camera();
    cam.pitch = 0.0;
    assert!(cam.forward().y.abs() < 0.001);
}

#[test]
fn forward_points_up_at_pitch_90() {
    let mut cam = build_camera();
    cam.pitch = std::f32::consts::FRAC_PI_2;
    assert!(cam.forward().y > 0.99);
}

#[test]
fn right_is_perpendicular_to_forward() {
    let cam = build_camera();
    let dot = cam.right().dot(cam.forward());
    assert!(dot.abs() < 0.001, "right·forward = {dot}");
}

#[test]
fn view_matrix_looks_forward() {
    let cam = build_camera();
    let view = cam.view_matrix();
    let fwd = cam.forward();
    let point_ahead = cam.position + fwd * 10.0;
    let clip = view * glam::Vec4::new(point_ahead.x, point_ahead.y, point_ahead.z, 1.0);
    assert!(clip.z < 0.0, "point ahead should have negative z in view space");
}

#[test]
fn uniform_has_correct_padding() {
    let cam = build_camera();
    let uniform = cam.uniform();
    assert_eq!(uniform._pad, 0.0);
    assert_eq!(std::mem::size_of::<osm_world::camera::CameraUniform>(), 80);
}
```

- [ ] **Step 5: Update src/lib.rs to export modules**

```rust
pub mod camera;
pub mod render;
```

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: All 6 camera tests PASS

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: Vertex struct, Flycam, CameraUniform with tests"
```

---

### Task 3: Camera Controller (Input Handling)

**Files:**
- Create: `src/camera/controller.rs`
- Update: `src/camera/mod.rs`

- [ ] **Step 1: Create src/camera/controller.rs**

```rust
use super::Flycam;
use std::collections::HashSet;
use winit::event::{ElementState, MouseButton};

/// Translates keyboard/mouse input into camera movement.
pub struct CameraController {
    pub keys_pressed: HashSet<winit::keyboard::KeyCode>,
    pub mouse_dx: f32,
    pub mouse_dy: f32,
    pub right_mouse_held: bool,
    pub mouse_sensitivity: f32,
}

impl CameraController {
    pub fn new() -> Self {
        Self {
            keys_pressed: HashSet::new(),
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            right_mouse_held: false,
            mouse_sensitivity: 0.003,
        }
    }

    pub fn process_keyboard(&mut self, key: winit::keyboard::KeyCode, state: ElementState) {
        match state {
            ElementState::Pressed => {
                self.keys_pressed.insert(key);
            }
            ElementState::Released => {
                self.keys_pressed.remove(&key);
            }
        }
    }

    pub fn process_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        if button == MouseButton::Right {
            self.right_mouse_held = state == ElementState::Pressed;
        }
    }

    pub fn process_mouse_motion(&mut self, dx: f64, dy: f64) {
        if self.right_mouse_held {
            self.mouse_dx += dx as f32;
            self.mouse_dy += dy as f32;
        }
    }

    pub fn update_camera(&mut self, camera: &mut Flycam, dt: f32) {
        // Rotation
        camera.yaw += self.mouse_dx * self.mouse_sensitivity;
        camera.pitch -= self.mouse_dy * self.mouse_sensitivity;
        camera.pitch = camera.pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;

        // Translation
        let speed = camera.speed * dt;
        let forward = camera.forward();
        let right = camera.right();
        use winit::keyboard::KeyCode;
        if self.keys_pressed.contains(&KeyCode::KeyW) {
            camera.position += forward * speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyS) {
            camera.position -= forward * speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyA) {
            camera.position -= right * speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyD) {
            camera.position += right * speed;
        }
        if self.keys_pressed.contains(&KeyCode::Space) {
            camera.position.y += speed;
        }
        if self.keys_pressed.contains(&KeyCode::ShiftLeft) {
            camera.position.y -= speed;
        }
    }
}
```

- [ ] **Step 2: Update src/camera/mod.rs to add controller module**

```rust
mod controller;

use bytemuck::{Pod, Zeroable};

pub use controller::CameraController;

// ... existing CameraUniform and Flycam code unchanged ...

// Add at end of file:
```

Add `pub mod controller;` at the top of `src/camera/mod.rs` and `pub use controller::CameraController;` in the public exports section.

- [ ] **Step 3: Build and verify**

Run: `cargo build`
Expected: compiles successfully

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: CameraController with WASD + mouse look input"
```

---

### Task 4: WGSL Shader

**Files:**
- Create: `src/shaders/city.wgsl`

- [ ] **Step 1: Create src/shaders/city.wgsl**

```wgsl
struct Camera {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    _pad: f32,
}

@group(0) @binding(0) var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) feature_type: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) color: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.world_normal = in.normal;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.4, 1.0, 0.3));
    let ambient = 0.3;
    let diffuse = max(dot(normalize(in.world_normal), light_dir), 0.0);
    let lighting = ambient + diffuse * 0.7;
    return vec4<f32>(in.color * lighting, 1.0);
}
```

- [ ] **Step 2: Verify shader syntax**

The shader uses standard WGSL with `struct`, `@group(0)`, `@vertex`, `@fragment`. No exotic features. Will be validated at pipeline creation time in Task 5.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: city.wgsl shader with directional lighting"
```

---

### Task 5: Render Pipeline + Bind Groups

**Files:**
- Create: `src/render/pipelines.rs`
- Create: `src/render/bind_groups.rs`

- [ ] **Step 1: Create src/render/bind_groups.rs**

```rust
use wgpu::*;

use crate::camera::CameraUniform;

pub struct CameraBindGroup {
    pub layout: BindGroupLayout,
    pub group: BindGroup,
    pub buffer: Buffer,
}

impl CameraBindGroup {
    pub fn new(device: &Device) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("camera uniform buffer"),
            size: std::mem::size_of::<CameraUniform>() as BufferAddress,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("camera bind group layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self { layout, group, buffer }
    }

    pub fn update(&self, queue: &Queue, uniform: &CameraUniform) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(std::slice::from_ref(uniform)));
    }
}
```

- [ ] **Step 2: Create src/render/pipelines.rs**

```rust
use wgpu::*;

use super::vertex::Vertex;

pub struct CityPipeline {
    pub pipeline: RenderPipeline,
    pub layout: PipelineLayout,
}

impl CityPipeline {
    pub fn new(device: &Device, camera_layout: &BindGroupLayout, surface_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("city shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/city.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("city pipeline layout"),
            bind_group_layouts: &[Some(camera_layout)],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("city render pipeline"),
            layout: Some(&layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
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
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self { pipeline, layout }
    }
}
```

Note: wgpu 29 requires `depth_write_enabled: Some(true)` and `depth_compare: Some(...)` (Option-wrapped). `bind_group_layouts` entries must be wrapped in `Some()`. `multiview_mask` is absent (v28 removed it).

- [ ] **Step 3: Build and verify**

Run: `cargo build`
Expected: compiles successfully

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: CityPipeline and CameraBindGroup with wgpu 29 APIs"
```

---

### Task 6: Test Geometry Generation

**Files:**
- Create: `src/render/buffers.rs`

- [ ] **Step 1: Create src/render/buffers.rs**

This generates a hardcoded building box (beige, 20x30x15m) centered at the origin, and a large green ground plane.

```rust
use wgpu::*;

use super::vertex::{Vertex, feature};

pub struct SceneBuffers {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
}

impl SceneBuffers {
    pub fn new(device: &Device) -> Self {
        let (vertices, indices) = generate_test_scene();
        let index_count = indices.len() as u32;

        let vertex_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("scene vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("scene index buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });

        Self { vertex_buffer, index_buffer, index_count }
    }
}

fn generate_test_scene() -> (Vec<Vertex>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();

    // Ground plane: 2000x2000m at y=0
    append_ground_plane(&mut verts, &mut idxs, 2000.0);

    // Test building: 20m wide, 30m deep, 15m tall at position (0, 0, 0)
    append_box(&mut verts, &mut idxs,
        -10.0, 10.0,   // x range
        0.0, 15.0,     // y range
        -15.0, 15.0,   // z range
        [0.85, 0.78, 0.65], // beige
        feature::BUILDING,
    );

    (verts, idxs)
}

fn append_ground_plane(verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>, size: f32) {
    let base = verts.len() as u32;
    let h = size / 2.0;
    let n = [0.0, 1.0, 0.0];
    let c = [0.35, 0.55, 0.25]; // green
    verts.extend_from_slice(&[
        Vertex { position: [-h, 0.0, -h], normal: n, color: c, feature_type: feature::TERRAIN },
        Vertex { position: [ h, 0.0, -h], normal: n, color: c, feature_type: feature::TERRAIN },
        Vertex { position: [ h, 0.0,  h], normal: n, color: c, feature_type: feature::TERRAIN },
        Vertex { position: [-h, 0.0,  h], normal: n, color: c, feature_type: feature::TERRAIN },
    ]);
    idxs.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
}

/// Append an axis-aligned box with per-face normals.
fn append_box(
    verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>,
    x0: f32, x1: f32, y0: f32, y1: f32, z0: f32, z1: f32,
    color: [f32; 3], feature_type: f32,
) {
    let base = verts.len() as u32;
    let v = |px, py, pz, nx, ny, nz| Vertex {
        position: [px, py, pz], normal: [nx, ny, nz], color, feature_type,
    };

    // Front face (z+)
    verts.extend_from_slice(&[v(x0,y0,z1, 0,0,1), v(x1,y0,z1, 0,0,1), v(x1,y1,z1, 0,0,1), v(x0,y1,z1, 0,0,1)]);
    // Back face (z-)
    verts.extend_from_slice(&[v(x1,y0,z0, 0,0,-1), v(x0,y0,z0, 0,0,-1), v(x0,y1,z0, 0,0,-1), v(x1,y1,z0, 0,0,-1)]);
    // Right face (x+)
    verts.extend_from_slice(&[v(x1,y0,z1, 1,0,0), v(x1,y0,z0, 1,0,0), v(x1,y1,z0, 1,0,0), v(x1,y1,z1, 1,0,0)]);
    // Left face (x-)
    verts.extend_from_slice(&[v(x0,y0,z0, -1,0,0), v(x0,y0,z1, -1,0,0), v(x0,y1,z1, -1,0,0), v(x0,y1,z0, -1,0,0)]);
    // Top face (y+)
    verts.extend_from_slice(&[v(x0,y1,z1, 0,1,0), v(x1,y1,z1, 0,1,0), v(x1,y1,z0, 0,1,0), v(x0,y1,z0, 0,1,0)]);
    // Bottom face (y-)
    verts.extend_from_slice(&[v(x0,y0,z0, 0,-1,0), v(x1,y0,z0, 0,-1,0), v(x1,y0,z1, 0,-1,0), v(x0,y0,z1, 0,-1,0)]);

    for face in 0..6u32 {
        let b = base + face * 4;
        idxs.extend_from_slice(&[b, b+1, b+2, b, b+2, b+3]);
    }
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build`
Expected: compiles successfully. Note: `device.create_buffer_init` requires `wgpu::util::DeviceExt` trait import which the caller must bring into scope.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: test geometry (building box + ground plane) generation"
```

---

### Task 7: App Struct + WGPU Initialization

**Files:**
- Create: `src/app/mod.rs`
- Create: `src/app/init.rs`
- Update: `src/lib.rs`

- [ ] **Step 1: Create src/app/init.rs**

```rust
use wgpu::*;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

use crate::camera::Flycam;
use crate::render::bind_groups::CameraBindGroup;
use crate::render::buffers::SceneBuffers;
use crate::render::pipelines::CityPipeline;

pub struct AppState {
    pub window: Window,
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    pub depth_texture: Texture,
    pub depth_view: TextureView,
    pub camera: Flycam,
    pub camera_bg: CameraBindGroup,
    pub pipeline: CityPipeline,
    pub scene: SceneBuffers,
}

pub fn init_wgpu(event_loop: &ActiveEventLoop) -> anyhow::Result<AppState> {
    let window = event_loop.create_window(WindowAttributes::default().with_title("osm-world"))?;

    let instance = Instance::new(InstanceDescriptor::new_with_display_handle(
        Box::new(event_loop.owned_display_handle()),
    ));

    let surface = instance.create_surface(window.clone())?;
    let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    })).ok_or_else(|| anyhow::anyhow!("no suitable GPU adapter found"))?;

    let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
        label: Some("osm-world device"),
        required_features: Features::empty(),
        required_limits: Limits::default(),
        memory_hints: MemoryHints::Performance,
        trace: None,
    }, None))?;

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|f| f.is_srgb())
        .copied()
        .unwrap_or(surface_caps.formats[0]);

    let size = window.inner_size();
    let surface_config = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: PresentMode::AutoVsync,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_config);

    let (depth_texture, depth_view) = create_depth_buffer(&device, surface_config.width, surface_config.height);

    let camera = Flycam::new(surface_config.width as f32 / surface_config.height as f32);
    let camera_bg = CameraBindGroup::new(&device);
    let pipeline = CityPipeline::new(&device, &camera_bg.layout, surface_format);
    let scene = SceneBuffers::new(&device);

    Ok(AppState {
        window, device, queue, surface, surface_config,
        depth_texture, depth_view,
        camera, camera_bg, pipeline, scene,
    })
}

fn create_depth_buffer(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("depth texture"),
        size: Extent3d { width, height: height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Depth32Float,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}
```

- [ ] **Step 2: Create src/app/mod.rs**

```rust
pub mod init;
pub mod render_loop;
pub mod update;
pub mod event_handler;

use std::collections::HashSet;

use crate::camera::{CameraController, Flycam};
use crate::render::bind_groups::CameraBindGroup;
use crate::render::buffers::SceneBuffers;
use crate::render::pipelines::CityPipeline;

pub use init::AppState;

pub struct App {
    pub state: Option<AppState>,
    pub controller: CameraController,
    pub last_frame_time: std::time::Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: None,
            controller: CameraController::new(),
            last_frame_time: std::time::Instant::now(),
        }
    }
}
```

- [ ] **Step 3: Update src/lib.rs**

```rust
pub mod app;
pub mod camera;
pub mod render;
```

- [ ] **Step 4: Build and verify**

Run: `cargo build`
Expected: compiles successfully (note: render_loop.rs, update.rs, event_handler.rs are declared but don't exist yet — create stubs)

Create stub files:
- `src/app/render_loop.rs` — `// Phase 1 Task 8`
- `src/app/update.rs` — `// Phase 1 Task 8`
- `src/app/event_handler.rs` — `// Phase 1 Task 8`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: AppState struct and WGPU initialization (wgpu 29 + winit 0.30)"
```

---

### Task 8: Render Loop + Update + Event Handler

**Files:**
- Create: `src/app/render_loop.rs`
- Create: `src/app/update.rs`
- Create: `src/app/event_handler.rs`
- Update: `src/main.rs`

- [ ] **Step 1: Create src/app/render_loop.rs**

```rust
use wgpu::*;

use super::AppState;

pub fn render(state: &AppState) {
    let output = match state.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) => frame,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
        wgpu::CurrentSurfaceTexture::Outdated
        | wgpu::CurrentSurfaceTexture::Suboptimal(_)
        | wgpu::CurrentSurfaceTexture::Lost => {
            state.surface.configure(&state.device, &state.surface_config);
            return;
        }
        wgpu::CurrentSurfaceTexture::Validation => return,
    };

    let view = output.texture.create_view(&TextureViewDescriptor::default());

    let mut encoder = state.device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("render encoder"),
    });

    {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("main render pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color { r: 0.53, g: 0.81, b: 0.92, a: 1.0 }),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &state.depth_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&state.pipeline.pipeline);
        pass.set_bind_group(0, &state.camera_bg.group, &[]);
        pass.set_vertex_buffer(0, state.scene.vertex_buffer.slice(..));
        pass.set_index_buffer(state.scene.index_buffer.slice(..), IndexFormat::Uint32);
        pass.draw_indexed(0..state.scene.index_count, 0, 0..1);
    }

    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();
}
```

- [ ] **Step 2: Create src/app/update.rs**

```rust
use super::App;

pub fn update(app: &mut App) {
    let now = std::time::Instant::now();
    let dt = (now - app.last_frame_time).as_secs_f32();
    app.last_frame_time = now;

    if let Some(state) = &mut app.state {
        app.controller.update_camera(&mut state.camera, dt);
        state.camera_bg.update(&state.queue, &state.camera.uniform());
    }
}
```

- [ ] **Step 3: Create src/app/event_handler.rs**

```rust
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::{init, App};
use crate::app::render_loop;
use crate::app::update;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            match init::init_wgpu(event_loop) {
                Ok(state) => {
                    log::info!("WGPU initialized: {:?}", state.device.get_adapter_info());
                    self.state = Some(state);
                }
                Err(e) => {
                    log::error!("Failed to initialize WGPU: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(state) = &mut self.state {
                    if physical_size.width > 0 && physical_size.height > 0 {
                        state.surface_config.width = physical_size.width;
                        state.surface_config.height = physical_size.height;
                        state.surface.configure(&state.device, &state.surface_config);
                        let (dt, dv) = super::init::create_depth_buffer(&state.device, physical_size.width, physical_size.height);
                        state.depth_texture = dt;
                        state.depth_view = dv;
                        state.camera.aspect = physical_size.width as f32 / physical_size.height as f32;
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(key) = event.physical_key.to_key_code() {
                    self.controller.process_keyboard(key, event.state);
                }
            }
            WindowEvent::MouseInput { button, state, .. } => {
                self.controller.process_mouse_button(button, state);
            }
            WindowEvent::CursorMoved { position, .. } => {
                // We use DeviceEvent for raw motion instead for smoother camera
                let _ = position;
            }
            WindowEvent::RedrawRequested => {
                update::update(self);
                if let Some(state) = &self.state {
                    render_loop::render(state);
                }
                state.window().request_redraw();
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.controller.process_mouse_motion(dx, dy);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}
```

Note: `state.window()` won't work directly — we need to access the window through `self.state`. Fix: access `state.window` directly since `Surface<'static>` owns the window reference. Use `self.state.as_ref().map(|s| s.window.request_redraw())` in `about_to_wait`.

In `window_event::RedrawRequested`, the borrow checker requires careful handling. The pattern is to split the render from state access:

```rust
WindowEvent::RedrawRequested => {
    update::update(self);
    if let Some(state) = &self.state {
        render_loop::render(state);
        state.window.request_redraw();
    }
}
```

- [ ] **Step 4: Update src/main.rs**

```rust
fn main() -> anyhow::Result<()> {
    env_logger::init();
    log::info!("osm-world starting");

    let event_loop = winit::event_loop::EventLoop::new()?;
    let mut app = osm_world::app::App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
```

- [ ] **Step 5: Build and verify**

Run: `cargo build`
Expected: compiles successfully

- [ ] **Step 6: Run and verify visually**

Run: `cargo run`
Expected: A window opens showing a beige box on a green ground plane with a light blue sky background. The box is lit by a directional light.

- [ ] **Step 7: Test flycam controls**

- Right-click + drag: rotate camera
- WASD: move forward/back/strafe
- Space/Shift: move up/down
- All controls should respond smoothly

- [ ] **Step 8: Run checkall**

Run: `make checkall`
Expected: fmt, typecheck, lint, test all pass

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: full render loop, event handling, flycam controls — Phase 1 complete"
```

---

### Task 9: Final Integration and Cleanup

**Files:**
- Update: `src/app/mod.rs` (expose `window()` accessor if needed for borrow checker)
- Update: `Cargo.toml` (ensure all deps present)
- Update: `Makefile` (ensure all targets work)

- [ ] **Step 1: Fix any borrow checker issues in event_handler.rs**

The key issue is that `WindowEvent::RedrawRequested` needs to call `update()`, `render()`, and `request_redraw()`. The window is inside `AppState` which is inside `App`. The render function takes `&AppState`. The update function takes `&mut App`. This should work because update borrows `&mut App` first, then we reborrow `&AppState` for render.

If the borrow checker complains about `state.window.request_redraw()` after `render_loop::render(state)`, restructure as:

```rust
WindowEvent::RedrawRequested => {
    update::update(self);
    if let Some(state) = &self.state {
        render_loop::render(state);
    }
    if let Some(state) = &self.state {
        state.window.request_redraw();
    }
}
```

- [ ] **Step 2: Ensure create_depth_buffer is accessible from event_handler**

Move `create_depth_buffer` to be `pub` in `src/app/init.rs` so the resize handler in `event_handler.rs` can call it:

```rust
pub fn create_depth_buffer(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
    // ... existing code ...
}
```

- [ ] **Step 3: Add `use wgpu::util::DeviceExt;` in buffers.rs**

The `create_buffer_init` method comes from the `DeviceExt` trait:

```rust
use wgpu::util::DeviceExt;
```

- [ ] **Step 4: Run full checkall**

Run: `make checkall`
Expected: All targets pass — fmt, typecheck, lint, test

- [ ] **Step 5: Final visual test**

Run: `cargo run`
Expected:
1. Window opens (titled "osm-world")
2. Light blue sky background
3. Green ground plane
4. Beige building box visible, lit by directional light
5. Right-click + mouse drag rotates camera
6. WASD moves camera, Space/Shift for vertical
7. Smooth 60 FPS (vsync)

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "fix: borrow checker fixes, pub visibility, Phase 1 fully working"
```

---

## Self-Review Checklist

**Spec coverage:** Every Phase 1 requirement from the spec maps to a task:
- Window creation → Task 7, 8
- Flycam + WASD → Task 2, 3, 8
- Test building → Task 6
- Render pipeline → Task 4, 5
- Directional lighting → Task 4 (in shader)
- Depth buffer → Task 7

**Placeholder scan:** No TBDs, TODOs, or "implement later" patterns. Every step has concrete code or commands.

**Type consistency:** `Vertex` struct defined in Task 2, used identically in Task 5 (pipeline) and Task 6 (geometry). `CameraUniform` defined in Task 2, used in Task 5 (bind groups) and Task 8 (render). `Flycam` methods (`forward()`, `right()`, `view_matrix()`, `projection_matrix()`, `uniform()`) are consistent across Task 2, 3 (controller), and Task 8 (update).

**wgpu 29 API compliance:** All wgpu calls use v29 patterns:
- `InstanceDescriptor::new_with_display_handle()` (Task 7)
- `CurrentSurfaceTexture` enum (Task 8)
- `Some()` wrapping in `bind_group_layouts` (Task 5)
- `Some(true)` / `Some(CompareFunction::Less)` for depth state (Task 5)
- `winit 0.30 ApplicationHandler` trait (Task 8)
- `about_to_wait()` for redraw (Task 8)
