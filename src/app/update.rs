use super::App;

pub fn update(app: &mut App) {
    let now = std::time::Instant::now();
    let dt = (now - app.last_frame_time).as_secs_f32();
    app.last_frame_time = now;
    app.performance.update(dt);

    if let Some(request) = app.area_switch.take_request() {
        load_requested_area(app, request);
    }

    if let Some(state) = &mut app.state {
        app.day_cycle.update(dt);
        app.controller.update_camera(&mut state.camera, dt);
        let uniforms = state.camera.uniforms(&app.day_cycle, &app.atmosphere);
        state.camera_bg.update(&state.queue, &uniforms);
    }
}

fn load_requested_area(app: &mut App, request: crate::app::AreaSwitchRequest) {
    let Some(state) = &mut app.state else {
        app.area_switch.status = "Renderer is not initialized yet.".to_string();
        return;
    };
    let srtm_dir = request.srtm_dir.as_deref().map(std::path::Path::new);
    match crate::app::init::load_scene_resources(
        &state.device,
        std::path::Path::new(&request.input_path),
        srtm_dir,
    ) {
        Ok(loaded) => {
            state.scene = loaded.scene;
            state.coord_converter = loaded.coord_converter;
            state.poi_labels = loaded.poi_labels;
            state.street_sign_labels = loaded.street_sign_labels;
            app.opts.input_path = Some(request.input_path.clone());
            app.opts.srtm_dir = request.srtm_dir.clone();
            app.area_switch.input_path = request.input_path;
            app.area_switch.srtm_dir = request.srtm_dir.unwrap_or_default();
            app.area_switch.status = "Prepared area loaded.".to_string();
        }
        Err(err) => {
            app.area_switch.status = format!("Failed to load prepared area: {err:#}");
        }
    }
}
