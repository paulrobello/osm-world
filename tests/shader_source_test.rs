#[test]
fn city_shader_animates_water_normals_and_sun_glints() {
    let shader = include_str!("../shaders/city.wgsl");

    assert!(shader.contains("fn water_normal"));
    assert!(shader.contains("scene.animation_time"));
    assert!(shader.contains("water_sun_glint"));
    assert!(shader.contains("feature_type < 3.5"));
}
