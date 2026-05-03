fn build_camera() -> osm_world::camera::Flycam {
    osm_world::camera::Flycam::new(1.0)
}

fn default_atmosphere() -> (
    osm_world::atmosphere::DayCycleState,
    osm_world::atmosphere::AtmosphereSettings,
) {
    (
        osm_world::atmosphere::DayCycleState::default(),
        osm_world::atmosphere::AtmosphereSettings::default(),
    )
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
    assert!(
        clip.z < 0.0,
        "point ahead should have negative z in view space"
    );
}

#[test]
fn uniform_has_correct_size_and_padding() {
    let cam = build_camera();
    let (day, atm) = default_atmosphere();
    let uniforms = cam.uniforms(&day, &atm);
    assert_eq!(uniforms._pad0, 0.0);
    assert_eq!(
        uniforms.light_direction,
        osm_world::atmosphere::dominant_light_direction(day.time_of_day)
    );
    assert_eq!(std::mem::size_of::<osm_world::camera::SceneUniforms>(), 288);
}

#[test]
fn q_and_e_move_camera_down_and_up() {
    use winit::event::ElementState;
    use winit::keyboard::KeyCode;

    let mut cam = build_camera();
    let mut controller = osm_world::camera::CameraController::new();

    controller.process_keyboard(KeyCode::KeyQ, ElementState::Pressed);
    controller.update_camera(&mut cam, 1.0);
    assert_eq!(cam.position.y, -50.0);

    controller.process_keyboard(KeyCode::KeyQ, ElementState::Released);
    controller.process_keyboard(KeyCode::KeyE, ElementState::Pressed);
    controller.update_camera(&mut cam, 1.0);
    assert_eq!(cam.position.y, 50.0);
}

#[test]
fn shift_doubles_movement_speed_without_moving_vertically() {
    use winit::event::ElementState;
    use winit::keyboard::KeyCode;

    let mut cam = build_camera();
    cam.yaw = 0.0;
    cam.pitch = 0.0;
    let start_y = cam.position.y;
    let mut controller = osm_world::camera::CameraController::new();

    controller.process_keyboard(KeyCode::ShiftLeft, ElementState::Pressed);
    controller.process_keyboard(KeyCode::KeyW, ElementState::Pressed);
    controller.update_camera(&mut cam, 1.0);

    assert_eq!(cam.position.x, 200.0);
    assert_eq!(cam.position.y, start_y);
}

#[test]
fn light_view_projection_is_stable_for_sub_texel_camera_motion() {
    let mut cam = build_camera();
    cam.position = glam::Vec3::ZERO;
    let sun_direction = [0.5, -0.7, 0.3];
    let world_point = glam::Vec3::new(123.0, 12.0, -456.0);

    let before = cam.shadow_cascades(sun_direction);
    cam.position.x += 0.5;
    let after = cam.shadow_cascades(sun_direction);

    for (label, before_matrix, after_matrix) in [
        (
            "near",
            before.near.light_view_proj,
            after.near.light_view_proj,
        ),
        ("mid", before.mid.light_view_proj, after.mid.light_view_proj),
    ] {
        let before_clip = before_matrix * world_point.extend(1.0);
        let after_clip = after_matrix * world_point.extend(1.0);

        assert!(
            (before_clip.x - after_clip.x).abs() < 1e-7,
            "{label} cascade x moved: {before_clip:?} -> {after_clip:?}"
        );
        assert!(
            (before_clip.y - after_clip.y).abs() < 1e-7,
            "{label} cascade y moved: {before_clip:?} -> {after_clip:?}"
        );
    }
}

#[test]
fn shadow_cascade_blend_transitions_from_near_to_mid_to_far() {
    use osm_world::camera::{
        SHADOW_MID_RADIUS, SHADOW_NEAR_RADIUS, ShadowCascadeBlend, shadow_cascade_blend,
    };

    let exact_near = shadow_cascade_blend(100.0, SHADOW_NEAR_RADIUS, SHADOW_MID_RADIUS);
    assert_eq!(
        exact_near,
        ShadowCascadeBlend {
            near_weight: 1.0,
            mid_weight: 0.0,
            shadow_strength: 1.0,
        }
    );

    let near_transition = shadow_cascade_blend(
        SHADOW_NEAR_RADIUS - osm_world::camera::SHADOW_NEAR_BLEND_DISTANCE * 0.5,
        SHADOW_NEAR_RADIUS,
        SHADOW_MID_RADIUS,
    );
    assert!((near_transition.near_weight - 0.5).abs() < 1e-6);
    assert!((near_transition.mid_weight - 0.5).abs() < 1e-6);
    assert!((near_transition.shadow_strength - 1.0).abs() < 1e-6);

    let exact_mid = shadow_cascade_blend(
        SHADOW_MID_RADIUS - osm_world::camera::SHADOW_MID_FADE_DISTANCE,
        SHADOW_NEAR_RADIUS,
        SHADOW_MID_RADIUS,
    );
    assert_eq!(
        exact_mid,
        ShadowCascadeBlend {
            near_weight: 0.0,
            mid_weight: 1.0,
            shadow_strength: 1.0,
        }
    );

    let far_transition = shadow_cascade_blend(
        SHADOW_MID_RADIUS - osm_world::camera::SHADOW_MID_FADE_DISTANCE * 0.5,
        SHADOW_NEAR_RADIUS,
        SHADOW_MID_RADIUS,
    );
    assert!((far_transition.near_weight - 0.0).abs() < 1e-6);
    assert!((far_transition.mid_weight - 1.0).abs() < 1e-6);
    assert!((far_transition.shadow_strength - 0.5).abs() < 1e-6);

    let fully_lit = shadow_cascade_blend(
        SHADOW_MID_RADIUS + 10.0,
        SHADOW_NEAR_RADIUS,
        SHADOW_MID_RADIUS,
    );
    assert_eq!(
        fully_lit,
        ShadowCascadeBlend {
            near_weight: 0.0,
            mid_weight: 0.0,
            shadow_strength: 0.0,
        }
    );
}
