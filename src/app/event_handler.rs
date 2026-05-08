use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::{App, init};
use crate::app::{render_loop, update};

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let init_options = init::InitWgpuOptions {
                window_width: self.opts.window_width,
                window_height: self.opts.window_height,
                input_path: self.opts.input_path.as_deref(),
                srtm_dir: self.opts.srtm_dir.as_deref(),
                cam_override: self.opts.cam_override.as_ref(),
                persisted_camera: self.persisted_camera.as_ref(),
                visual_detail: &self.visual_detail,
                streaming: &self.opts.streaming,
            };
            match init::init_wgpu(event_loop, &init_options) {
                Ok((state, egui)) => {
                    log::info!("WGPU initialized: {:?}", state.device.adapter_info());
                    log::info!(
                        "Controls: [P]ause cycle  [BracketLeft/BracketRight] time  [C]louds  [Minus/Equal] fog  [9/0] coverage  [F1] settings"
                    );
                    self.state = Some(state);
                    self.egui = Some(egui);
                    self.render_start = Some(std::time::Instant::now());
                }
                Err(e) => {
                    log::error!("Failed to initialize WGPU: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Route events through egui first
        if let (Some(state), Some(egui)) = (&self.state, &mut self.egui) {
            let response = egui.winit_state.on_window_event(&state.window, &event);
            if response.consumed {
                return;
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                self.persist_preferences_if_changed(true);
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(state) = &mut self.state {
                    if physical_size.width > 0 && physical_size.height > 0 {
                        state.surface_config.width = physical_size.width;
                        state.surface_config.height = physical_size.height;
                        state
                            .surface
                            .configure(&state.device, &state.surface_config);
                        let (dt, dv) = init::create_depth_buffer(
                            &state.device,
                            physical_size.width,
                            physical_size.height,
                        );
                        state.depth_texture = dt;
                        state.depth_view = dv;
                        state.contact_shadow.resize(
                            &state.device,
                            physical_size.width,
                            physical_size.height,
                            &state.depth_view,
                        );
                        state.camera.aspect =
                            physical_size.width as f32 / physical_size.height as f32;
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let winit::keyboard::PhysicalKey::Code(key) = event.physical_key {
                    self.controller.process_keyboard(key, event.state);
                    if event.state == winit::event::ElementState::Pressed {
                        use winit::keyboard::KeyCode;
                        match key {
                            KeyCode::KeyP => {
                                self.day_cycle.paused = !self.day_cycle.paused;
                                log::info!(
                                    "Day cycle {}",
                                    if self.day_cycle.paused {
                                        "paused"
                                    } else {
                                        "running"
                                    }
                                );
                            }
                            KeyCode::BracketLeft => {
                                self.day_cycle.time_of_day =
                                    (self.day_cycle.time_of_day - 0.01).rem_euclid(1.0);
                            }
                            KeyCode::BracketRight => {
                                self.day_cycle.time_of_day =
                                    (self.day_cycle.time_of_day + 0.01).rem_euclid(1.0);
                            }
                            KeyCode::KeyC => {
                                self.atmosphere.clouds_enabled = !self.atmosphere.clouds_enabled;
                                log::info!(
                                    "Clouds {}",
                                    if self.atmosphere.clouds_enabled {
                                        "enabled"
                                    } else {
                                        "disabled"
                                    }
                                );
                            }
                            KeyCode::Minus => {
                                self.atmosphere.fog_density =
                                    (self.atmosphere.fog_density - 0.0005).max(0.0);
                            }
                            KeyCode::Equal => {
                                self.atmosphere.fog_density =
                                    (self.atmosphere.fog_density + 0.0005).min(0.05);
                            }
                            KeyCode::Digit9 => {
                                self.atmosphere.cloud_coverage =
                                    (self.atmosphere.cloud_coverage - 0.05).max(0.0);
                            }
                            KeyCode::Digit0 => {
                                self.atmosphere.cloud_coverage =
                                    (self.atmosphere.cloud_coverage + 0.05).min(1.0);
                            }
                            KeyCode::F1 => {
                                self.show_settings = !self.show_settings;
                            }
                            KeyCode::KeyM => {
                                self.minimap.visible = !self.minimap.visible;
                            }
                            _ => {}
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor_pos = Some(egui::pos2(position.x as f32, position.y as f32));
            }
            WindowEvent::MouseInput {
                button,
                state: button_state,
                ..
            } => {
                self.controller.process_mouse_button(button, button_state);
                if button == winit::event::MouseButton::Left
                    && button_state == winit::event::ElementState::Pressed
                {
                    if let (Some(state), Some(cursor_pos)) = (&self.state, self.last_cursor_pos) {
                        let scale = state.window.scale_factor() as f32;
                        let viewport_size = egui::vec2(
                            state.surface_config.width as f32 / scale,
                            state.surface_config.height as f32 / scale,
                        );
                        let click = egui::pos2(cursor_pos.x / scale, cursor_pos.y / scale);
                        self.inspect.selected = crate::ui::inspect::pick_identifiable(
                            &state.identifiables,
                            &state.camera,
                            viewport_size,
                            click,
                            18.0,
                        );
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                update::update(self);

                let screenshot_path = self.check_screenshot_cloned();

                if let (Some(state), Some(egui)) = (&mut self.state, &mut self.egui) {
                    render_loop::render(
                        state,
                        egui,
                        screenshot_path.as_deref(),
                        render_loop::RenderUiState {
                            atmosphere: &mut self.atmosphere,
                            day_cycle: &mut self.day_cycle,
                            show_settings: &mut self.show_settings,
                            minimap: &mut self.minimap,
                            settings_sections: &mut self.settings_sections,
                            poi_labels: &mut self.poi_labels,
                            address_labels: &mut self.address_labels,
                            street_sign_labels: &mut self.street_sign_labels,
                            search: &mut self.search,
                            inspect: &mut self.inspect,
                            performance: &mut self.performance,
                            area_switch: &mut self.area_switch,
                            visual_detail: &mut self.visual_detail,
                        },
                    );
                    self.persist_preferences_if_changed(false);
                }

                if screenshot_path.is_some() {
                    self.screenshot_taken = true;
                }

                if self.check_auto_exit() {
                    self.persist_preferences_if_changed(true);
                    event_loop.exit();
                    return;
                }

                if let Some(state) = &self.state {
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.controller.process_mouse_motion(dx, dy);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

impl App {
    fn check_screenshot_cloned(&self) -> Option<String> {
        if self.screenshot_taken {
            return None;
        }
        let path = self.opts.screenshot_path.as_ref()?;
        let start = self.render_start?;
        let elapsed = start.elapsed().as_secs_f32();
        if elapsed >= self.opts.screenshot_delay {
            Some(path.clone())
        } else {
            None
        }
    }

    fn persist_preferences_if_changed(&mut self, force: bool) {
        let minimap = crate::app::prefs::MinimapPrefs::from_minimap_state(&self.minimap);
        let camera = self
            .state
            .as_ref()
            .map(|state| crate::app::prefs::CameraPrefs::from_camera(&state.camera))
            .or_else(|| self.persisted_camera.clone());
        let settings_sections = self.settings_sections.clone();
        let visual_detail =
            crate::app::prefs::VisualDetailPrefs::from_visual_detail_settings(&self.visual_detail);
        let minimap_changed = minimap != self.persisted_minimap;
        let camera_changed = camera != self.persisted_camera;
        let settings_sections_changed = settings_sections != self.persisted_settings_sections;
        let visual_detail_changed = visual_detail != self.persisted_visual_detail;
        if !minimap_changed
            && !camera_changed
            && !settings_sections_changed
            && !visual_detail_changed
        {
            return;
        }
        if camera_changed
            && !minimap_changed
            && !settings_sections_changed
            && !visual_detail_changed
            && !force
            && self.last_prefs_save.elapsed() < super::PREFS_SAVE_INTERVAL
        {
            return;
        }

        let prefs = crate::app::prefs::UserPrefs {
            minimap: minimap.clone(),
            camera: camera.clone(),
            settings_sections: settings_sections.clone(),
            visual_detail: visual_detail.clone(),
        };
        match crate::app::prefs::save_user_prefs(&prefs) {
            Ok(()) => {
                self.persisted_minimap = minimap;
                self.persisted_camera = camera;
                self.persisted_settings_sections = settings_sections;
                self.persisted_visual_detail = visual_detail;
                self.last_prefs_save = std::time::Instant::now();
            }
            Err(err) => log::warn!("Failed to save preferences: {err}"),
        }
    }

    fn check_auto_exit(&self) -> bool {
        if let (Some(delay), Some(start)) = (self.opts.auto_exit_delay, self.render_start) {
            let elapsed = start.elapsed().as_secs_f32();
            if elapsed >= delay {
                log::info!("[AUTO-EXIT] Exiting after {:.2}s", elapsed);
                return true;
            }
        }
        false
    }
}
