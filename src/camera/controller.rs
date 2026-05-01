use super::Flycam;
use std::collections::HashSet;
use winit::event::{ElementState, MouseButton};

pub struct CameraController {
    pub keys_pressed: HashSet<winit::keyboard::KeyCode>,
    pub mouse_dx: f32,
    pub mouse_dy: f32,
    pub right_mouse_held: bool,
    pub mouse_sensitivity: f32,
}

impl CameraController {
    pub fn new() -> Self {
        Self {
            keys_pressed: HashSet::new(),
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            right_mouse_held: false,
            mouse_sensitivity: 0.003,
        }
    }

    pub fn process_keyboard(&mut self, key: winit::keyboard::KeyCode, state: ElementState) {
        match state {
            ElementState::Pressed => {
                self.keys_pressed.insert(key);
            }
            ElementState::Released => {
                self.keys_pressed.remove(&key);
            }
        }
    }

    pub fn process_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        if button == MouseButton::Right {
            self.right_mouse_held = state == ElementState::Pressed;
        }
    }

    pub fn process_mouse_motion(&mut self, dx: f64, dy: f64) {
        if self.right_mouse_held {
            self.mouse_dx += dx as f32;
            self.mouse_dy += dy as f32;
        }
    }

    pub fn update_camera(&mut self, camera: &mut Flycam, dt: f32) {
        camera.yaw += self.mouse_dx * self.mouse_sensitivity;
        camera.pitch -= self.mouse_dy * self.mouse_sensitivity;
        camera.pitch = camera.pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;

        let speed = camera.speed * dt;
        let forward = camera.forward();
        let right = camera.right();
        use winit::keyboard::KeyCode;
        if self.keys_pressed.contains(&KeyCode::KeyW) {
            camera.position += forward * speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyS) {
            camera.position -= forward * speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyA) {
            camera.position -= right * speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyD) {
            camera.position += right * speed;
        }
        if self.keys_pressed.contains(&KeyCode::Space) {
            camera.position.y += speed;
        }
        if self.keys_pressed.contains(&KeyCode::ShiftLeft) {
            camera.position.y -= speed;
        }
    }
}
