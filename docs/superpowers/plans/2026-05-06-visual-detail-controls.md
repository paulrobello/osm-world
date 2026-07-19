> ⚠️ Historical implementation plan (2026-05) — retained for reference; current behavior may differ. See `docs/ARCHITECTURE.md` and the source code.

# Visual Detail Controls Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement ideas 4, 5, and 6 from `ideas.md`: landmark-specific models, procedural building façade/roof variation, and adjustable vegetation/detail controls with CLI and screenshot validation support.

**Architecture:** Add a `visual_detail` settings module, pass settings from CLI/App/UI into world generation and render uniforms, and encode procedural visual metadata into existing vertex colors/UVs. Keep the existing mesh renderer as the baseline, but add renderer-side shader controls for façade intensity and point-feature visibility/distance so important settings apply live; mesh-changing density/cap settings mark reload as required.

**Tech Stack:** Rust 2024, WGPU 29, WGSL, egui, clap, existing `cargo test` suite.

---

## File Structure

- Create `src/visual_detail.rs`: presets, detail enums, clamp helpers, CLI value enums, tests.
- Modify `src/lib.rs`: export `visual_detail`.
- Modify `src/main.rs`: parse visual-detail CLI flags and pass settings into `AppOptions`.
- Modify `src/app/mod.rs`: store `VisualDetailSettings` and generation settings on app options/state.
- Modify `src/app/init.rs`: load scenes with visual settings and create scene buffers from visual settings.
- Modify `src/app/update.rs`: pass visual settings when switching areas if needed.
- Modify `src/app/render_loop.rs`: include visual settings in UI render state and update camera uniforms with visual controls.
- Modify `src/ui/settings.rs`: add Visual Detail controls and reload-required message.
- Modify `src/camera/mod.rs` and `shaders/city.wgsl`: extend scene uniforms with visual parameters and shader-side façade/point-feature controls.
- Modify `src/world/point_feature.rs`: add landmark kinds and showcase-specific geometry.
- Modify `src/world/building.rs` and `src/world/color.rs`: add deterministic building style, façade bands, and roof variation.
- Modify `src/world/loader.rs`: apply vegetation density/caps and building style generation; pass settings through world/tile mesh generation.
- Modify `ideas.md`: remove completed items 4, 5, and 6 after implementation is verified.

---

### Task 1: Visual detail settings model and CLI parsing

**Files:**
- Create: `src/visual_detail.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write failing settings tests**

Add `src/visual_detail.rs` with only tests first. The tests should describe the desired API:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showcase_preset_enables_rich_visual_detail() {
        let settings = VisualDetailSettings::from_preset(VisualPreset::Showcase);
        assert_eq!(settings.landmark_detail, LandmarkDetail::Showcase);
        assert!(settings.facade_variation > 0.8);
        assert!(settings.roof_variation > 0.8);
        assert!(settings.vegetation_density > 1.0);
        assert!(settings.synthetic_tree_cap >= 180);
    }

    #[test]
    fn performance_preset_reduces_clutter() {
        let settings = VisualDetailSettings::from_preset(VisualPreset::Performance);
        assert_eq!(settings.landmark_detail, LandmarkDetail::Simple);
        assert!(settings.facade_variation <= 0.35);
        assert!(settings.vegetation_density < 1.0);
        assert!(settings.synthetic_tree_cap <= 80);
    }

    #[test]
    fn clamp_prevents_invalid_values() {
        let mut settings = VisualDetailSettings::from_preset(VisualPreset::Balanced);
        settings.facade_variation = 4.0;
        settings.roof_variation = -1.0;
        settings.vegetation_density = 99.0;
        settings.synthetic_tree_cap = 0;
        settings.vegetation_max_distance = -100.0;
        settings.clamp();
        assert_eq!(settings.facade_variation, 1.0);
        assert_eq!(settings.roof_variation, 0.0);
        assert_eq!(settings.vegetation_density, 3.0);
        assert_eq!(settings.synthetic_tree_cap, 1);
        assert_eq!(settings.vegetation_max_distance, 0.0);
    }
}
```

- [ ] **Step 2: Run test to verify RED**

Run:

```bash
cargo test visual_detail --lib
```

Expected: compile failure because `visual_detail` types do not exist.

- [ ] **Step 3: Implement settings module**

Replace the test-only file with the implementation plus tests:

```rust
use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum VisualPreset {
    Performance,
    Balanced,
    Showcase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum LandmarkDetail {
    Off,
    Simple,
    Showcase,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisualDetailSettings {
    pub preset: VisualPreset,
    pub landmark_detail: LandmarkDetail,
    pub facade_variation: f32,
    pub roof_variation: f32,
    pub vegetation_visible: bool,
    pub vegetation_density: f32,
    pub synthetic_tree_cap: usize,
    pub vegetation_max_distance: f32,
    pub reload_required: bool,
}

impl Default for VisualDetailSettings {
    fn default() -> Self {
        Self::from_preset(VisualPreset::Balanced)
    }
}

impl VisualDetailSettings {
    pub fn from_preset(preset: VisualPreset) -> Self {
        match preset {
            VisualPreset::Performance => Self {
                preset,
                landmark_detail: LandmarkDetail::Simple,
                facade_variation: 0.25,
                roof_variation: 0.25,
                vegetation_visible: true,
                vegetation_density: 0.35,
                synthetic_tree_cap: 60,
                vegetation_max_distance: 1200.0,
                reload_required: false,
            },
            VisualPreset::Balanced => Self {
                preset,
                landmark_detail: LandmarkDetail::Showcase,
                facade_variation: 0.65,
                roof_variation: 0.65,
                vegetation_visible: true,
                vegetation_density: 1.0,
                synthetic_tree_cap: 120,
                vegetation_max_distance: 2600.0,
                reload_required: false,
            },
            VisualPreset::Showcase => Self {
                preset,
                landmark_detail: LandmarkDetail::Showcase,
                facade_variation: 1.0,
                roof_variation: 1.0,
                vegetation_visible: true,
                vegetation_density: 1.8,
                synthetic_tree_cap: 240,
                vegetation_max_distance: 4200.0,
                reload_required: false,
            },
        }
    }

    pub fn clamp(&mut self) {
        self.facade_variation = self.facade_variation.clamp(0.0, 1.0);
        self.roof_variation = self.roof_variation.clamp(0.0, 1.0);
        self.vegetation_density = self.vegetation_density.clamp(0.0, 3.0);
        self.synthetic_tree_cap = self.synthetic_tree_cap.max(1);
        self.vegetation_max_distance = self.vegetation_max_distance.max(0.0);
    }

    pub fn with_reload_required(mut self) -> Self {
        self.reload_required = true;
        self
    }
}
```

Add `pub mod visual_detail;` to `src/lib.rs`.

- [ ] **Step 4: Add CLI parser tests**

In `src/main.rs` tests, add:

```rust
#[test]
fn parses_visual_detail_flags() {
    let args = Args::try_parse_from([
        "osm-world",
        "--visual-preset",
        "showcase",
        "--landmark-detail",
        "simple",
        "--facade-variation",
        "0.75",
        "--roof-variation",
        "0.5",
        "--vegetation-density",
        "1.4",
        "--synthetic-tree-cap",
        "180",
        "--vegetation-distance",
        "3200",
    ])
    .unwrap();

    assert_eq!(args.visual_preset, osm_world::visual_detail::VisualPreset::Showcase);
    assert_eq!(args.landmark_detail, Some(osm_world::visual_detail::LandmarkDetail::Simple));
    assert_eq!(args.facade_variation, Some(0.75));
    assert_eq!(args.roof_variation, Some(0.5));
    assert_eq!(args.vegetation_density, Some(1.4));
    assert_eq!(args.synthetic_tree_cap, Some(180));
    assert_eq!(args.vegetation_distance, Some(3200.0));
}
```

- [ ] **Step 5: Run CLI test to verify RED**

Run:

```bash
cargo test parses_visual_detail_flags --bin osm-world
```

Expected: compile failure because CLI fields do not exist.

- [ ] **Step 6: Implement CLI flags**

Add these fields to `Args` in `src/main.rs`:

```rust
#[arg(long, value_enum, default_value_t = osm_world::visual_detail::VisualPreset::Balanced)]
visual_preset: osm_world::visual_detail::VisualPreset,

#[arg(long, value_enum)]
landmark_detail: Option<osm_world::visual_detail::LandmarkDetail>,

#[arg(long, value_parser = normalized_f32)]
facade_variation: Option<f32>,

#[arg(long, value_parser = normalized_f32)]
roof_variation: Option<f32>,

#[arg(long, value_parser = density_multiplier)]
vegetation_density: Option<f32>,

#[arg(long, value_parser = positive_usize)]
synthetic_tree_cap: Option<usize>,

#[arg(long, value_parser = nonnegative_f32)]
vegetation_distance: Option<f32>,
```

Add helper parsers near existing numeric parsers:

```rust
fn nonnegative_f32(s: &str) -> Result<f32, String> { /* parse finite >= 0 */ }
fn normalized_f32(s: &str) -> Result<f32, String> { /* parse finite 0..=1 */ }
fn density_multiplier(s: &str) -> Result<f32, String> { /* parse finite 0..=3 */ }
```

Build settings in `main()` before `App::new`:

```rust
let mut visual_detail = osm_world::visual_detail::VisualDetailSettings::from_preset(args.visual_preset);
if let Some(value) = args.landmark_detail { visual_detail.landmark_detail = value; }
if let Some(value) = args.facade_variation { visual_detail.facade_variation = value; }
if let Some(value) = args.roof_variation { visual_detail.roof_variation = value; }
if let Some(value) = args.vegetation_density { visual_detail.vegetation_density = value; }
if let Some(value) = args.synthetic_tree_cap { visual_detail.synthetic_tree_cap = value; }
if let Some(value) = args.vegetation_distance { visual_detail.vegetation_max_distance = value; }
visual_detail.clamp();
```

Pass `visual_detail` into `AppOptions` in Task 2.

- [ ] **Step 7: Run tests and commit**

Run:

```bash
cargo test visual_detail --lib
cargo test parses_visual_detail_flags --bin osm-world
```

Commit:

```bash
git add src/visual_detail.rs src/lib.rs src/main.rs
git commit -m "feat: add visual detail settings and cli flags"
```

---

### Task 2: Wire visual settings through App, uniforms, and Settings UI

**Files:**
- Modify: `src/app/mod.rs`
- Modify: `src/app/init.rs`
- Modify: `src/app/update.rs`
- Modify: `src/app/render_loop.rs`
- Modify: `src/ui/settings.rs`
- Modify: `src/camera/mod.rs`
- Modify: `src/render/minimap.rs`
- Modify: `shaders/city.wgsl`

- [ ] **Step 1: Write RED tests for uniform values**

In `src/camera/mod.rs` tests, add a test that calls a new `uniforms_with_visual_detail` API:

```rust
#[test]
fn scene_uniforms_include_visual_detail_params() {
    let camera = Flycam::new(1.0);
    let day = crate::atmosphere::DayCycleState::default();
    let atm = crate::atmosphere::AtmosphereSettings::default();
    let visual = crate::visual_detail::VisualDetailSettings::from_preset(
        crate::visual_detail::VisualPreset::Showcase,
    );

    let uniforms = camera.uniforms_with_visual_detail(&day, &atm, &visual);

    assert_eq!(uniforms.visual_params[0], visual.facade_variation);
    assert_eq!(uniforms.visual_params[1], visual.roof_variation);
    assert_eq!(uniforms.visual_params[2], visual.vegetation_max_distance);
    assert_eq!(uniforms.visual_params[3], 1.0);
}
```

Run:

```bash
cargo test scene_uniforms_include_visual_detail_params --lib
```

Expected: compile failure because the API/field does not exist.

- [ ] **Step 2: Extend uniforms and shader layout**

Add to `SceneUniforms`:

```rust
pub visual_params: [f32; 4], // facade, roof, vegetation max distance, vegetation visible
pub visual_params2: [f32; 4], // landmark detail numeric, reserved, reserved, reserved
```

Add `Flycam::uniforms_with_visual_detail(...)` and make existing `uniforms(...)` call it with `VisualDetailSettings::default()`.

Update `shaders/city.wgsl` `SceneUniforms` with matching fields:

```wgsl
visual_params: vec4<f32>,
visual_params2: vec4<f32>,
```

- [ ] **Step 3: Wire AppOptions/App state**

Add to `AppOptions`:

```rust
pub visual_detail: crate::visual_detail::VisualDetailSettings,
```

Add to `App`:

```rust
pub visual_detail: crate::visual_detail::VisualDetailSettings,
```

Set it in `App::new` from `opts.visual_detail.clone()`.

- [ ] **Step 4: Update render uniforms and UI state**

In `RenderUiState`, add:

```rust
pub visual_detail: &'a mut crate::visual_detail::VisualDetailSettings,
```

Where camera uniforms are updated, call:

```rust
let uniforms = state.camera.uniforms_with_visual_detail(ui_state.day_cycle, ui_state.atmosphere, ui_state.visual_detail);
state.camera_bg.update(&state.queue, &uniforms);
```

Do the same for minimap using default or current visual settings, whichever keeps the minimap consistent.

- [ ] **Step 5: Add Settings UI controls**

Add `visual_detail` to `SettingsDrawState` and call a new `visual_detail_section(ui, state.visual_detail)`.

The UI should include:

```rust
ui.label("Mesh-changing density/cap settings apply after reloading the area.");
ui.checkbox(&mut settings.vegetation_visible, "Vegetation visible");
ui.add(Slider::new(&mut settings.facade_variation, 0.0..=1.0).text("Facade variation"));
ui.add(Slider::new(&mut settings.roof_variation, 0.0..=1.0).text("Roof variation"));
ui.add(Slider::new(&mut settings.vegetation_density, 0.0..=3.0).text("Vegetation density"));
ui.add(Slider::new(&mut settings.vegetation_max_distance, 0.0..=8000.0).text("Vegetation distance (m)"));
if settings.reload_required { ui.colored_label(egui::Color32::YELLOW, "Reload area to apply density/cap placement changes."); }
```

When density or cap changes, set `settings.reload_required = true`.

- [ ] **Step 6: Run tests and commit**

Run:

```bash
cargo test scene_uniforms_include_visual_detail_params --lib
cargo test parses_visual_detail_flags --bin osm-world
```

Commit:

```bash
git add src/app src/ui src/camera/mod.rs src/render/minimap.rs shaders/city.wgsl src/main.rs
git commit -m "feat: wire visual detail settings through renderer ui"
```

---

### Task 3: Landmark-specific classification and showcase meshes

**Files:**
- Modify: `src/world/point_feature.rs`

- [ ] **Step 1: Write RED classification tests**

Add tests:

```rust
#[test]
fn classifies_specific_landmark_kinds() {
    assert_eq!(point_feature_style(&tags(&[("man_made", "tower")])).unwrap().landmark_kind, Some(LandmarkKind::Tower));
    assert_eq!(point_feature_style(&tags(&[("man_made", "water_tower")])).unwrap().landmark_kind, Some(LandmarkKind::WaterTower));
    assert_eq!(point_feature_style(&tags(&[("man_made", "chimney")])).unwrap().landmark_kind, Some(LandmarkKind::Chimney));
    assert_eq!(point_feature_style(&tags(&[("historic", "monument")])).unwrap().landmark_kind, Some(LandmarkKind::Monument));
    assert_eq!(point_feature_style(&tags(&[("tourism", "viewpoint")])).unwrap().landmark_kind, Some(LandmarkKind::Viewpoint));
    assert_eq!(point_feature_style(&tags(&[("natural", "peak")])).unwrap().landmark_kind, Some(LandmarkKind::Peak));
}
```

Run:

```bash
cargo test classifies_specific_landmark_kinds --lib
```

Expected: compile failure.

- [ ] **Step 2: Implement `LandmarkKind` and style field**

Add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LandmarkKind { Generic, Tower, WaterTower, Chimney, Monument, Peak, Viewpoint }
```

Add `pub landmark_kind: Option<LandmarkKind>` to `PointFeatureStyle` and fill it in all constructors.

Ensure `natural=peak` now classifies as `PointFeatureKind::Landmark` with `LandmarkKind::Peak` instead of generic nature if the spec wants it to render as a landmark silhouette.

- [ ] **Step 3: Write RED geometry tests**

Add tests that compare silhouette heights/colors:

```rust
#[test]
fn landmark_kinds_emit_distinct_showcase_silhouettes() {
    let samples = [
        (("man_made", "tower"), LandmarkKind::Tower),
        (("man_made", "chimney"), LandmarkKind::Chimney),
        (("historic", "monument"), LandmarkKind::Monument),
        (("natural", "peak"), LandmarkKind::Peak),
        (("tourism", "viewpoint"), LandmarkKind::Viewpoint),
    ];

    let mut tops = Vec::new();
    for ((key, value), _kind) in samples {
        let mut verts = Vec::new();
        let mut idxs = Vec::new();
        generate_point_feature(&tags(&[(key, value)]), (0.0, 0.0), 0.0, &mut verts, &mut idxs);
        assert!(!idxs.is_empty());
        tops.push(verts.iter().map(|v| v.position[1]).fold(f32::NEG_INFINITY, f32::max));
    }

    assert!(tops[0] > tops[3], "tower should be taller than peak marker");
    assert!(tops[1] > tops[2], "chimney should be taller than monument");
}
```

Run it and expect failure until geometry differs.

- [ ] **Step 4: Implement geometry dispatch**

Change `generate_point_feature` landmark branch to dispatch by kind:

```rust
PointFeatureKind::Landmark => append_landmark_kind(
    point,
    elevation,
    style.landmark_kind.unwrap_or(LandmarkKind::Generic),
    verts,
    idxs,
),
```

Implement lightweight helpers using existing `append_box`, `append_pyramid`, and trunk/canopy helpers:

- tower: tall tapered stacked boxes plus spire;
- water tower: narrow post plus tank box/cylinder approximation;
- chimney: tall narrow stack;
- monument: obelisk/pyramid;
- peak: rocky low pyramid cluster;
- viewpoint: post plus platform/arrow marker;
- generic: existing landmark geometry.

Set point feature UV marker channel in `vertex()` later if Task 5 needs vegetation/landmark distance filtering.

- [ ] **Step 5: Run tests and commit**

Run:

```bash
cargo test world::point_feature --lib
```

Commit:

```bash
git add src/world/point_feature.rs
git commit -m "feat: render distinct landmark silhouettes"
```

---

### Task 4: Deterministic façade and roof variation

**Files:**
- Modify: `src/world/building.rs`
- Modify: `src/world/color.rs`
- Modify: `src/world/loader.rs`
- Modify: `shaders/city.wgsl`

- [ ] **Step 1: Write RED style tests**

In `src/world/color.rs`, add tests for new `building_style` API:

```rust
#[test]
fn building_style_is_deterministic_for_same_tags_and_seed() {
    let tags = HashMap::from([("building".to_string(), "apartments".to_string())]);
    let a = building_style(&tags, 42, 1.0, 1.0);
    let b = building_style(&tags, 42, 1.0, 1.0);
    assert_eq!(a, b);
}

#[test]
fn building_style_varies_by_seed_when_no_material_tags_exist() {
    let tags = HashMap::from([("building".to_string(), "yes".to_string())]);
    let a = building_style(&tags, 1, 1.0, 1.0);
    let b = building_style(&tags, 2, 1.0, 1.0);
    assert_ne!(a.wall_color, b.wall_color);
}
```

Run:

```bash
cargo test building_style --lib
```

Expected: compile failure.

- [ ] **Step 2: Implement `BuildingStyle`**

Add to `src/world/color.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildingStyle {
    pub wall_color: [f32; 3],
    pub roof_color: [f32; 3],
    pub band_color: [f32; 3],
    pub facade_intensity: f32,
    pub roof_intensity: f32,
}

pub fn building_style(tags: &HashMap<String, String>, seed: u64, facade_intensity: f32, roof_intensity: f32) -> BuildingStyle { ... }
```

Use `building_color(tags)` as the base. Use a tiny deterministic hash/jitter helper:

```rust
fn seed_unit(seed: u64, salt: u64) -> f32 {
    let mut x = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    ((x >> 40) as f32) / ((1u64 << 24) as f32)
}
```

- [ ] **Step 3: Change building generation to accept style**

In `src/world/building.rs`, keep the old public `generate_building(..., color, ...)` wrapper for tests, and add:

```rust
pub fn generate_building_with_style(
    footprint: &[(f32, f32)],
    base_y: f32,
    height: f32,
    style: crate::world::color::BuildingStyle,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) { ... }
```

Wall vertices should alternate or band colors using height ratio. Set wall `uv` to `[edge_progress, height_ratio]`. Roof vertices use `style.roof_color` and `uv` values that let shader identify roof-like building surfaces if needed.

- [ ] **Step 4: Wire loader seed and settings**

In `src/world/loader.rs`, use a deterministic per-building seed from tags and representative lat/lon or feature index. For first pass, feature index is acceptable if stable for the same input ordering:

```rust
let style = super::color::building_style(&b.tags, feature_idx as u64, visual.facade_variation, visual.roof_variation);
super::building::generate_building_with_style(&footprint, base_y, height, style, verts, idxs);
```

This requires Task 5 or a small precursor to pass `visual` into `generate_world_mesh` and tile mesh generation.

- [ ] **Step 5: Add shader-side façade intensity**

In `city.wgsl`, before lighting, adjust building color:

```wgsl
fn apply_visual_detail_color(color: vec3<f32>, feature_type: f32, uv: vec2<f32>, dist: f32) -> vec3<f32> {
    var out_color = color;
    if (feature_type > 0.5 && feature_type < 1.5) {
        let band = step(0.55, fract(uv.y * 8.0));
        let facade = scene.visual_params.x;
        out_color = mix(out_color, out_color * mix(0.82, 1.08, band), facade * 0.35);
    }
    return out_color;
}
```

Use the adjusted color in fragment lighting.

- [ ] **Step 6: Run tests and commit**

Run:

```bash
cargo test world::building world::color --lib
```

Commit:

```bash
git add src/world/building.rs src/world/color.rs src/world/loader.rs shaders/city.wgsl
git commit -m "feat: add deterministic building facade variation"
```

---

### Task 5: Vegetation density/cap controls and scene generation settings

**Files:**
- Modify: `src/world/loader.rs`
- Modify: `src/app/init.rs`
- Modify: `src/app/update.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/app/render_loop.rs`
- Modify: `src/render/minimap.rs`
- Modify: `shaders/city.wgsl`

- [ ] **Step 1: Write RED loader tests**

Add tests in `src/world/loader.rs`:

```rust
#[test]
fn visual_settings_scale_synthetic_tree_counts() {
    let area = ResolvedFeature { /* square green area matching existing tests */ };
    let conv = CoordConverter::new(0.0, 0.0);
    let elev = |_lat: f64, _lon: f64| 0.0;

    let low = crate::visual_detail::VisualDetailSettings {
        vegetation_density: 0.25,
        synthetic_tree_cap: 10,
        ..crate::visual_detail::VisualDetailSettings::default()
    };
    let high = crate::visual_detail::VisualDetailSettings {
        vegetation_density: 2.0,
        synthetic_tree_cap: 200,
        ..crate::visual_detail::VisualDetailSettings::default()
    };

    let mut low_points = Vec::new();
    append_tree_area_point_features_with_settings(&area, &conv, &elev, &low, &mut low_points);
    let mut high_points = Vec::new();
    append_tree_area_point_features_with_settings(&area, &conv, &elev, &high, &mut high_points);

    assert!(low_points.len() <= 10);
    assert!(high_points.len() > low_points.len());
}
```

Use existing helper data from nearby tree tests instead of duplicating if clearer.

Run:

```bash
cargo test visual_settings_scale_synthetic_tree_counts --lib
```

Expected: compile failure.

- [ ] **Step 2: Add settings-aware tree generation**

Keep `append_tree_area_point_features` as a wrapper that uses defaults. Add:

```rust
fn append_tree_area_point_features_with_settings(
    area: &ResolvedFeature,
    conv: &CoordConverter,
    elev: &impl Fn(f64, f64) -> f32,
    visual: &crate::visual_detail::VisualDetailSettings,
    point_features: &mut Vec<ResolvedPointFeature>,
) { ... }
```

Scale spacing by density:

```rust
let density = visual.vegetation_density.max(0.0);
if density == 0.0 || !visual.vegetation_visible { return; }
let spacing = config.spacing_metres / density.sqrt().max(0.25);
let max_points = config.max_points.min(visual.synthetic_tree_cap);
```

- [ ] **Step 3: Pass visual settings through scene loading**

Add `load_world_source_with_visual_detail(path, srtm_dir, visual)` wrapper and make `load_world_source` call it with default settings.

Add `generate_world_mesh_with_visual_detail(source, visual)` wrapper and make `generate_world_mesh(source)` call default settings.

Update `init_wgpu` / `load_scene_resources` / area switching code to call settings-aware versions from `AppOptions.visual_detail` or `App.visual_detail`.

- [ ] **Step 4: Add shader-side point-feature visibility/distance**

Set point-feature vertex UV marker channels:

- tree vertices: `uv.x = 1.0`;
- landmark vertices: `uv.x = 2.0`;
- POI/nature vertices: `uv.x = 0.0`.

In `city.wgsl` fragment, before returning, discard vegetation detail when hidden or too far:

```wgsl
if (in.feature_type > 6.5 && in.uv.x > 0.5 && in.uv.x < 1.5) {
    if (scene.visual_params.w < 0.5 || dist > scene.visual_params.z) { discard; }
}
```

- [ ] **Step 5: Run tests and commit**

Run:

```bash
cargo test world::loader --lib
cargo test --lib
```

Commit:

```bash
git add src/world/loader.rs src/world/point_feature.rs src/app src/render/minimap.rs shaders/city.wgsl
git commit -m "feat: add adjustable vegetation detail controls"
```

---

### Task 6: Screenshot validation workflow and ideas cleanup

**Files:**
- Modify: `ideas.md`
- Optional modify: `docs/superpowers/plans/2026-05-06-visual-detail-controls.md` if exact screenshot commands change during implementation.

- [ ] **Step 1: Verify screenshot CLI is sufficient**

Check that these flags now work together:

```bash
cargo test parses_visual_detail_flags --bin osm-world
cargo test parses_shadow_debug_and_time_flags --bin osm-world
```

The existing screenshot flags are sufficient if this command is supported:

```bash
cargo run --release -- \
  --input <prepared-area.osm> \
  --visual-preset showcase \
  --time-of-day 16.5 \
  --spawn-lat <lat> \
  --spawn-lon <lon> \
  --cam-yaw <degrees> \
  --cam-pitch <degrees> \
  --screenshot artifacts/visual-showcase.png \
  --screenshot-delay 5 \
  --auto-exit 7
```

Do not add more screenshot flags unless this command cannot validate the work repeatably.

- [ ] **Step 2: Remove completed ideas**

Edit `ideas.md` and remove items 4, 5, and 6 from the list. Do not renumber unrelated items unless you are already editing that block and the markdown becomes confusing.

- [ ] **Step 3: Run full verification**

Run:

```bash
cargo fmt -- --check
cargo test
```

If `cargo fmt -- --check` fails due to formatting, run `cargo fmt`, then rerun checks.

- [ ] **Step 4: Update graphify**

Run from the worktree:

```bash
graphify update .
```

Expected: graph files update successfully or report no meaningful changes. If `graphify` is unavailable, note that in the final result.

- [ ] **Step 5: Commit cleanup**

Commit:

```bash
git add ideas.md graphify-out src shaders docs/superpowers/plans/2026-05-06-visual-detail-controls.md
git commit -m "chore: verify visual detail controls"
```
