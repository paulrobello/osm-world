//! Shader-source contract tests.
//!
//! These tests use the same public loader functions the renderer compiles
//! (`osm_world::render::pipelines::city_shader_source`,
//! `osm_world::render::sky_pipeline::sky_shader_source`) so they see exactly
//! what WGPU sees, then:
//!
//! 1. Assert every Rust feature-type constant in `src/mesh.rs::feature`
//!    appears in `shaders/features.wgsl` with the same value (real
//!    cross-check; replaces the previous tautological `contains("feature_type
//!    < 3.5")` assertion).
//! 2. Assert the shared `SceneUniforms` struct is not re-declared inside
//!    `city.wgsl` or `sky.wgsl` (it is prepended from
//!    `shaders/scene_uniforms.wgsl`).
//! 3. Parse both shaders with `naga::front::wgsl::parse_str` so WGSL validity
//!    is enforced.
//! 4. Keep the existing spot-checks for `water_normal` / `sun_limb` / etc. so
//!    a future edit cannot silently drop those features.

use std::collections::BTreeMap;

const MESH_RS: &str = "src/mesh.rs";
const FEATURES_WGSL: &str = "shaders/features.wgsl";
const CITY_WGSL: &str = "shaders/city.wgsl";
const SKY_WGSL: &str = "shaders/sky.wgsl";

/// Mirror of the parser in `build.rs`. Extracts `pub const NAME: f32 = VALUE;`
/// declarations from the `pub mod feature { ... }` block of `src/mesh.rs`.
fn parse_feature_module(src: &str) -> BTreeMap<String, String> {
    let start = src
        .find("pub mod feature {")
        .expect("src/mesh.rs should contain `pub mod feature { ... }`");
    let end = src[start..]
        .find("}\n")
        .expect("`pub mod feature` block should terminate with `}\\n`");
    let block = &src[start..start + end];

    let mut out = BTreeMap::new();
    for line in block.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("pub const ") else {
            continue;
        };
        let Some(rest) = rest.strip_suffix(';') else {
            continue;
        };
        let Some((name, value)) = rest.split_once(": f32 = ") else {
            continue;
        };
        out.insert(name.trim().to_string(), value.trim().to_string());
    }
    out
}

/// Mirror of the parser in `build.rs`. Extracts `const FEATURE_NAME: f32 =
/// VALUE;` from `shaders/features.wgsl`, keyed by `NAME`.
fn parse_features_wgsl(src: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in src.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("const FEATURE_") else {
            continue;
        };
        let Some(rest) = rest.strip_suffix(';') else {
            continue;
        };
        let Some((name, value)) = rest.split_once(": f32 = ") else {
            continue;
        };
        out.insert(name.trim().to_string(), value.trim().to_string());
    }
    out
}

#[test]
fn features_wgsl_matches_mesh_rs_constants() {
    let mesh_src = std::fs::read_to_string(MESH_RS).unwrap();
    let features_src = std::fs::read_to_string(FEATURES_WGSL).unwrap();

    let rust = parse_feature_module(&mesh_src);
    let wgsl = parse_features_wgsl(&features_src);

    assert!(
        !rust.is_empty(),
        "parser found no feature constants in src/mesh.rs — parse_feature_module is stale"
    );
    assert!(
        !wgsl.is_empty(),
        "parser found no FEATURE_* constants in shaders/features.wgsl — parse_features_wgsl is stale"
    );

    for (name, rust_value) in &rust {
        let wgsl_value = wgsl.get(name).unwrap_or(&"<missing>".to_string()).clone();
        assert_eq!(
            rust_value, &wgsl_value,
            "feature constant {name} drifted: src/mesh.rs = {rust_value}, shaders/features.wgsl = {wgsl_value}"
        );
    }
    for name in wgsl.keys() {
        assert!(
            rust.contains_key(name),
            "FEATURE_{name} appears in shaders/features.wgsl but not in src/mesh.rs::feature"
        );
    }
}

#[test]
fn scene_uniforms_not_redclared_in_main_shaders() {
    let city = std::fs::read_to_string(CITY_WGSL).unwrap();
    let sky = std::fs::read_to_string(SKY_WGSL).unwrap();
    assert!(
        !city.contains("struct SceneUniforms"),
        "city.wgsl re-declares `struct SceneUniforms`; it is now prepended from shaders/scene_uniforms.wgsl"
    );
    assert!(
        !city.contains("var<uniform> scene: SceneUniforms"),
        "city.wgsl re-declares the `scene` uniform binding; it is now prepended from shaders/scene_uniforms.wgsl"
    );
    assert!(
        !sky.contains("struct SceneUniforms"),
        "sky.wgsl re-declares `struct SceneUniforms`; it is now prepended from shaders/scene_uniforms.wgsl"
    );
    assert!(
        !sky.contains("var<uniform> scene: SceneUniforms"),
        "sky.wgsl re-declares the `scene` uniform binding; it is now prepended from shaders/scene_uniforms.wgsl"
    );
}

#[test]
fn city_shader_compiles_and_keeps_water_and_sun_features() {
    let shader = osm_world::render::pipelines::city_shader_source();

    naga::front::wgsl::parse_str(&shader).expect("city shader should parse as WGSL");

    assert!(shader.contains("fn water_normal"));
    assert!(shader.contains("scene.animation_time"));
    assert!(shader.contains("water_sun_glint"));
    // A representative feature-constant reference: confirms the constants are
    // in scope (i.e. features.wgsl was prepended). Replaces the old
    // tautological `contains("feature_type < 3.5")` check, which only verified
    // a literal string survived.
    assert!(shader.contains("FEATURE_WATER"));
    assert!(shader.contains("FEATURE_POINT_FEATURE"));
    assert!(shader.contains("FEATURE_BUILDING"));
}

#[test]
fn sky_shader_compiles_and_keeps_sun_layers() {
    let shader = osm_world::render::sky_pipeline::sky_shader_source();

    naga::front::wgsl::parse_str(&shader).expect("sky shader should parse as WGSL");

    assert!(shader.contains("sun_limb"));
    assert!(shader.contains("sun_surface"));
    assert!(shader.contains("sun_corona"));
}
