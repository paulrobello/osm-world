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
        let uniforms = state.camera.uniforms_with_visual_detail(
            &app.day_cycle,
            &app.atmosphere,
            &app.visual_detail,
        );
        state.camera_bg.update(&state.queue, &uniforms);
        crate::stream::tile::update_loaded_tile_debug_states(
            &mut state.tile_debug_entries,
            app.opts.streaming.tile_size,
            state.camera.position,
            app.opts.streaming.stream_radius,
        );
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
        &app.visual_detail,
        &app.opts.streaming,
        state.camera.position,
    ) {
        Ok(loaded) => {
            state.scene = loaded.scene;
            state.coord_converter = loaded.coord_converter;
            state.poi_labels = loaded.poi_labels;
            state.address_labels = loaded.address_labels;
            state.street_sign_labels = loaded.street_sign_labels;
            state.search_entries = loaded.search_entries;
            state.identifiables = loaded.identifiables;
            state.tile_debug_entries = loaded.tile_debug_entries;
            state.tile_debug_tile_size = app.opts.streaming.tile_size;
            mark_area_load_success(app, request);
        }
        Err(err) => {
            app.area_switch.status = format!("Failed to load prepared area: {err:#}");
        }
    }
}

fn mark_area_load_success(app: &mut App, request: crate::app::AreaSwitchRequest) {
    app.opts.input_path = Some(request.input_path.clone());
    app.opts.srtm_dir = request.srtm_dir.clone();
    app.area_switch.input_path = request.input_path;
    app.area_switch.srtm_dir = request.srtm_dir.unwrap_or_default();
    app.area_switch.status = "Prepared area loaded.".to_string();
    app.visual_detail.reload_required = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_options_with_visual_reload_required() -> crate::app::AppOptions {
        crate::app::AppOptions {
            window_width: 800.0,
            window_height: 600.0,
            screenshot_path: None,
            screenshot_delay: 0.0,
            auto_exit_delay: None,
            input_path: None,
            srtm_dir: None,
            cam_override: None,
            show_settings: false,
            initial_time_of_day: None,
            real_time_of_day: false,
            hide_poi_labels: false,
            hide_address_labels: false,
            hide_street_sign_labels: false,
            hide_minimap: false,
            rotate_minimap: false,
            debug_shadow_cascades: false,
            streaming: crate::app::StreamingOptions::default(),
            visual_detail: crate::visual_detail::VisualDetailSettings::default()
                .with_reload_required(),
            visual_detail_overridden: true,
        }
    }

    #[test]
    fn area_load_success_clears_visual_reload_flag() {
        let mut app = App::new(app_options_with_visual_reload_required());
        let request = crate::app::AreaSwitchRequest {
            input_path: "/tmp/new-area.osm".to_string(),
            srtm_dir: Some("/tmp/srtm".to_string()),
        };

        mark_area_load_success(&mut app, request);

        assert_eq!(app.opts.input_path.as_deref(), Some("/tmp/new-area.osm"));
        assert_eq!(app.opts.srtm_dir.as_deref(), Some("/tmp/srtm"));
        assert_eq!(app.area_switch.input_path, "/tmp/new-area.osm");
        assert_eq!(app.area_switch.srtm_dir, "/tmp/srtm");
        assert_eq!(app.area_switch.status, "Prepared area loaded.");
        assert!(!app.visual_detail.reload_required);
    }
}
