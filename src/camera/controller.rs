//! Mouse-and-keyboard controller that translates winit input events into
//! flycam motion. Right-drag looks; WASD translates; Q/E move vertically;
//! Shift doubles speed.

use super::Flycam;
use std::collections::HashSet;
use winit::event::{ElementState, MouseButton};

impl Default for CameraController {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated input state for one frame: held keys, mouse delta from
/// right-drag, and look sensitivity.
pub struct CameraController {
    pub keys_pressed: HashSet<winit::keyboard::KeyCode>,
    pub mouse_dx: f32,
    pub mouse_dy: f32,
    pub right_mouse_held: bool,
    pub mouse_sensitivity: f32,
}

impl CameraController {
    /// Constructs a controller with no keys held, zero mouse delta, and the
    /// default mouse sensitivity.
    pub fn new() -> Self {
        Self {
            keys_pressed: HashSet::new(),
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            right_mouse_held: false,
            mouse_sensitivity: 0.003,
        }
    }

    /// Records a key press or release into the held-keys set.
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

    /// Tracks right-mouse-button state so `process_mouse_motion` knows when to
    /// apply look deltas.
    pub fn process_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        if button == MouseButton::Right {
            self.right_mouse_held = state == ElementState::Pressed;
        }
    }

    /// Accumulates raw mouse motion into look deltas only while the right mouse
    /// button is held.
    pub fn process_mouse_motion(&mut self, dx: f64, dy: f64) {
        if self.right_mouse_held {
            self.mouse_dx += dx as f32;
            self.mouse_dy += dy as f32;
        }
    }

    /// Applies one frame of accumulated input to `camera`, scaled by `dt`.
    /// Resets the mouse delta after applying it so each frame's look is independent.
    pub fn update_camera(&mut self, camera: &mut Flycam, dt: f32) {
        camera.yaw += self.mouse_dx * self.mouse_sensitivity;
        camera.pitch -= self.mouse_dy * self.mouse_sensitivity;
        camera.pitch = camera.pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;

        use winit::keyboard::KeyCode;
        let boost = self.keys_pressed.contains(&KeyCode::ShiftLeft)
            || self.keys_pressed.contains(&KeyCode::ShiftRight);
        let speed = camera.speed * if boost { 2.0 } else { 1.0 } * dt;
        let forward = camera.forward();
        let right = camera.right();
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
        if self.keys_pressed.contains(&KeyCode::KeyE) {
            camera.position.y += speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyQ) {
            camera.position.y -= speed;
        }
    }
}
