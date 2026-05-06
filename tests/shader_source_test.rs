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
    let shader = include_str!("../shaders/sky.wgsl");

    naga::front::wgsl::parse_str(shader).expect("sky shader should parse as WGSL");
    assert!(shader.contains("sun_limb"));
    assert!(shader.contains("sun_surface"));
    assert!(shader.contains("sun_corona"));
}
