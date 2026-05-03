use crate::camera::Flycam;

fn player_arrow_direction(camera: &Flycam, rotate_with_camera: bool) -> egui::Vec2 {
    let forward = horizontal_forward(camera);
    let map_up = minimap_up(camera, rotate_with_camera);
    let view = glam::Mat4::look_to_rh(glam::Vec3::ZERO, glam::Vec3::NEG_Y, map_up);
    let screen_direction = view.transform_vector3(forward);
    egui::vec2(screen_direction.x, screen_direction.y).normalized()
}

fn horizontal_forward(camera: &Flycam) -> glam::Vec3 {
    let forward = camera.forward();
    let horizontal = glam::vec3(forward.x, 0.0, forward.z).normalize_or_zero();
    if horizontal.length_squared() > 0.0 {
        horizontal
    } else {
        glam::Vec3::NEG_Z
    }
}

fn minimap_up(camera: &Flycam, rotate_with_camera: bool) -> glam::Vec3 {
    if rotate_with_camera {
        -horizontal_forward(camera)
    } else {
        glam::Vec3::Z
    }
}

pub struct MinimapState {
    pub visible: bool,
    pub zoom: f32,
    pub rotate_with_camera: bool,
    pub texture_id: Option<egui::TextureId>,
}

impl Default for MinimapState {
    fn default() -> Self {
        Self {
            visible: true,
            zoom: 500.0,
            rotate_with_camera: false,
            texture_id: None,
        }
    }
}

pub fn draw(ctx: &egui::Context, camera: &Flycam, state: &mut MinimapState) {
    if !state.visible {
        return;
    }

    let minimap_size = 256.0_f32;
    let padding = 8.0;

    egui::Area::new(egui::Id::new("minimap"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-padding, -padding])
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_black_alpha(180))
                .corner_radius(4.0)
                .inner_margin(2.0)
                .show(ui, |ui| {
                    let (rect, response) = ui
                        .allocate_exact_size(egui::Vec2::splat(minimap_size), egui::Sense::click());

                    if let Some(tex_id) = state.texture_id {
                        let sized = egui::load::SizedTexture {
                            id: tex_id,
                            size: egui::Vec2::splat(minimap_size),
                        };
                        let uv =
                            egui::Rect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0));
                        ui.put(
                            rect,
                            egui::Image::from_texture(sized)
                                .uv(uv)
                                .fit_to_exact_size(egui::Vec2::splat(minimap_size)),
                        );
                    }

                    // Player arrow
                    let center = rect.center();
                    let arrow_size = 8.0;
                    let arrow_direction = player_arrow_direction(camera, state.rotate_with_camera);
                    let wing_direction = egui::Vec2::new(-arrow_direction.y, arrow_direction.x);
                    let tip = center + arrow_direction * arrow_size;
                    let tail = center - arrow_direction * arrow_size * 0.55;
                    let left = tail + wing_direction * arrow_size * 0.45;
                    let right = tail - wing_direction * arrow_size * 0.45;

                    let painter = ui.painter_at(rect);
                    painter.add(egui::Shape::convex_polygon(
                        vec![tip, left, right],
                        egui::Color32::WHITE,
                        egui::Stroke::new(1.0, egui::Color32::BLACK),
                    ));

                    // Scroll to zoom
                    if response.hover_pos().is_some_and(|p| rect.contains(p)) {
                        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                        if scroll != 0.0 {
                            state.zoom = (state.zoom * (1.0 - scroll * 0.001)).clamp(200.0, 2000.0);
                        }
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_points_north_when_camera_faces_north_on_fixed_map() {
        let mut camera = Flycam::new(1.0);
        camera.yaw = -std::f32::consts::FRAC_PI_2;
        camera.pitch = 0.0;

        let direction = player_arrow_direction(&camera, false);

        assert!(direction.x.abs() < 0.001);
        assert!(direction.y < -0.99);
    }

    #[test]
    fn arrow_points_up_when_map_rotates_with_camera() {
        let mut camera = Flycam::new(1.0);
        camera.yaw = 1.0;
        camera.pitch = 0.0;

        let direction = player_arrow_direction(&camera, true);

        assert!(direction.x.abs() < 0.001);
        assert!(direction.y < -0.99);
    }
}
