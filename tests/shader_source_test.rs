#[test]
fn city_shader_animates_water_normals_and_sun_glints() {
    let shader = include_str!("../shaders/city.wgsl");

    assert!(shader.contains("fn water_normal"));
    assert!(shader.contains("scene.animation_time"));
    assert!(shader.contains("water_sun_glint"));
    assert!(shader.contains("feature_type < 3.5"));
}

#[test]
fn sky_shader_sun_uses_layered_depth_terms() {
    let helpers = include_str!("../shaders/sky_helpers.wgsl");
    let shader_raw = include_str!("../shaders/sky.wgsl");
    // Simulate the compile-time concatenation done by sky_pipeline.rs
    let shader = if let Some(pos) =
        shader_raw.find("// --- Sky helpers (loaded from sky_helpers.wgsl at compile time) ---")
    {
        let mut combined = String::with_capacity(shader_raw.len() + helpers.len());
        combined.push_str(&shader_raw[..pos]);
        combined.push_str(helpers);
        combined.push_str(
            &shader_raw[pos
                + "// --- Sky helpers (loaded from sky_helpers.wgsl at compile time) ---".len()..],
        );
        // Remove the fog helpers placeholder (already in sky_helpers.wgsl)
        if let Some(fog_pos) =
            combined.find("// --- Fog helpers (loaded from sky_helpers.wgsl at compile time) ---")
        {
            combined.replace_range(
                fog_pos
                    ..fog_pos
                        + "// --- Fog helpers (loaded from sky_helpers.wgsl at compile time) ---"
                            .len(),
                "",
            );
        }
        combined
    } else {
        shader_raw.to_string()
    };

    naga::front::wgsl::parse_str(&shader).expect("sky shader should parse as WGSL");
    assert!(shader.contains("sun_limb"));
    assert!(shader.contains("sun_surface"));
    assert!(shader.contains("sun_corona"));
}
