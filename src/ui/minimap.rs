use crate::camera::Flycam;

pub struct MinimapState {
    pub visible: bool,
    pub zoom: f32,
    pub texture_id: Option<egui::TextureId>,
}

impl Default for MinimapState {
    fn default() -> Self {
        Self {
            visible: true,
            zoom: 500.0,
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
                    let yaw = camera.yaw;
                    let arrow_size = 8.0;
                    let tip =
                        center + egui::Vec2::new(yaw.cos() * arrow_size, yaw.sin() * arrow_size);
                    let left = center
                        + egui::Vec2::new(
                            (yaw + 2.5).cos() * arrow_size * 0.6,
                            (yaw + 2.5).sin() * arrow_size * 0.6,
                        );
                    let right = center
                        + egui::Vec2::new(
                            (yaw - 2.5).cos() * arrow_size * 0.6,
                            (yaw - 2.5).sin() * arrow_size * 0.6,
                        );

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
