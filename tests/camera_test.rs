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
    assert_eq!(
        std::mem::size_of::<osm_world::camera::CameraUniform>(),
        80
    );
}
