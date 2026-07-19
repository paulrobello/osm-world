> ⚠️ Historical implementation plan (2026-05) — retained for reference; current behavior may differ. See `docs/ARCHITECTURE.md` and the source code.

# Cascaded Shadow LOD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single city-scale shadow map with a four-cascade dynamic shadow LOD system plus a lightweight screen-space contact-shadow pass.

**Architecture:** Use four shadow-map array layers. Compute all light view-projection matrices each frame from the dominant celestial light direction (sun by day, moon by night) and texel-snap each cascade independently. Render the caster-only index buffer once per cascade, batch those cascade passes into one shadow submit, and choose/fade cascades in the city shader by camera distance.

**Tech Stack:** Rust 2024, wgpu 29, glam, WGSL, cargo tests.

---

## File Responsibilities

- `src/camera/mod.rs`: cascade constants, cascade light matrix generation, testable cascade selection/fade helpers.
- `src/render/shadow_bind_group.rs`: shadow uniform layout, 4-layer depth texture/bind group resources, and per-cascade pass uniform buffers.
- `src/render/contact_shadow.rs`: fullscreen contact-shadow composite resources and pipeline.
- `src/app/render_loop.rs`: per-frame cascade uniform upload, four batched shadow render passes, offscreen scene render, and contact-shadow composite pass.
- `src/atmosphere.rs`: sun/moon dominant light helpers for day/night shadows.
- `shaders/shadow.wgsl`: use cascade index to render the correct light matrix.
- `shaders/city.wgsl`: sample a depth texture array, blend/fade cascade shadow factors, and optionally tint cascade bands for debugging.
- `shaders/sky.wgsl`: render stars as independent sub-cell points rather than whole twinkling cells.
- `src/render/buffers.rs`: retain dedicated building-caster index buffer.
- `tests/camera_test.rs`: cascade stability/selection coverage.

## Task 1: Cascade math and tests

**Files:**
- Modify: `src/camera/mod.rs`
- Modify: `tests/camera_test.rs`

- [ ] Add `ShadowCascade` and `ShadowCascadeSet` data types with two light matrices and two radii.
- [ ] Add `Flycam::shadow_cascades(sun_direction)` that returns near and mid cascade matrices.
- [ ] Add `shadow_cascade_blend(distance, near_radius, mid_radius)` for shader-equivalent selection/fade testing.
- [ ] Test that sub-texel camera movement keeps both cascade x/y projections stable.
- [ ] Test cascade blend behavior for near, mid, and far distances.
- [ ] Run `cargo test light_view_projection_is_stable_for_sub_texel_camera_motion shadow_cascade`.

## Task 2: Shadow uniform/resources

Status: implemented. The shadow uniform now carries four matrices, four radii, shader parameters including shadow-map size, and pass/debug parameters. The shadow map uses four array layers.

**Files:**
- Modify: `src/render/shadow_bind_group.rs`

- [ ] Replace single-matrix `LightUniforms` with a cascade uniform containing two `mat4x4` values, cascade radii, fade distances, and active cascade index for the shadow pass.
- [ ] Change shadow depth texture to `depth_or_array_layers: 2`.
- [ ] Bind it as a `D2Array` depth texture for city sampling.
- [ ] Create two per-layer render target views for shadow passes.
- [ ] Keep the comparison sampler.

## Task 3: Render loop cascade passes

Status: implemented. All four cascade passes are encoded into one shadow command encoder and submitted once before the main scene render.

**Files:**
- Modify: `src/app/render_loop.rs`

- [ ] Compute `state.camera.shadow_cascades(dominant_light_direction)` each frame.
- [ ] Render shadow cascade layer 0 with active cascade 0.
- [ ] Render shadow cascade layer 1 with active cascade 1.
- [ ] Use `state.scene.shadow_index_buffer` and `shadow_index_count` for both passes.
- [ ] Upload the final cascade uniform before city/minimap rendering.

## Task 4: WGSL shader updates

Status: implemented. WGSL now samples four cascades, reads shadow-map size from the uniform instead of hardcoding `2048`, and supports cascade debug tinting.

**Files:**
- Modify: `shaders/shadow.wgsl`
- Modify: `shaders/city.wgsl`

- [ ] Update shadow pass shader to use `light_view_proj[active_cascade]`.
- [ ] Update city shader shadow bindings to `texture_depth_2d_array`.
- [ ] Convert light NDC to shadow-map UVs with a flipped Y coordinate so sampled texture coordinates match the rendered depth layer.
- [ ] Add cascade selection based on `distance(world_position, camera_pos)`.
- [ ] Blend near/mid cascades over the transition band.
- [ ] Fade mid/far shadows to fully lit past the mid radius.
- [ ] Add settings/CLI cascade debug tint to make the smooth LOD bands visible.
- [ ] Use `scene.light_direction` for direct city lighting and shadow bias while preserving `scene.sun_direction` for sky/fog.

## Task 5: Contact shadows and verification

Status: implemented. The scene renders to an offscreen color target, then a fullscreen post pass samples scene depth to add short-range screen-space contact shadows before compositing to the surface.

**Files:**
- No source file changes expected.

- [ ] Run targeted cargo tests.
- [ ] Run `make checkall`.
- [ ] Run `graphify update .`.
- [ ] Capture screenshots at the known Sacramento camera pose at 14:00.
- [ ] Capture a cascade debug screenshot.
- [ ] Capture a night/moon-shadow screenshot.
- [ ] Capture a night-sky screenshot to verify independent star twinkle points.

## Self-Review

- Spec coverage: dynamic sun, two cascades, texel snapping, caster buffer, cascade fade, tests, and screenshots are covered.
- Placeholder scan: no TBD/TODO placeholders remain.
- Type consistency: `ShadowCascadeSet`, `LightUniforms`, and active cascade terminology are consistent across tasks.
