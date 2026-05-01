use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::{App, init};
use crate::app::{render_loop, update};

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            match init::init_wgpu(event_loop) {
                Ok(state) => {
                    log::info!("WGPU initialized: {:?}", state.device.adapter_info());
                    self.state = Some(state);
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
        match event {
            WindowEvent::CloseRequested => {
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
                        state.camera.aspect =
                            physical_size.width as f32 / physical_size.height as f32;
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let winit::keyboard::PhysicalKey::Code(key) = event.physical_key {
                    self.controller.process_keyboard(key, event.state);
                }
            }
            WindowEvent::MouseInput { button, state, .. } => {
                self.controller.process_mouse_button(button, state);
            }
            WindowEvent::RedrawRequested => {
                update::update(self);
                if let Some(state) = &self.state {
                    render_loop::render(state);
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
