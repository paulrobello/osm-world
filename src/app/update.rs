use super::App;

pub fn update(app: &mut App) {
    let now = std::time::Instant::now();
    let dt = (now - app.last_frame_time).as_secs_f32();
    app.last_frame_time = now;

    if let Some(state) = &mut app.state {
        app.controller.update_camera(&mut state.camera, dt);
        let uniforms = state.camera.uniforms(&app.day_cycle, &app.atmosphere);
        state.camera_bg.update(&state.queue, &uniforms);
    }
}
