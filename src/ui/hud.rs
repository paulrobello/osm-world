pub fn draw(
    ctx: &egui::Context,
    camera: &crate::camera::Flycam,
    day_cycle: &crate::atmosphere::DayCycleState,
) {
    egui::Area::new(egui::Id::new("camera_hud"))
        .anchor(egui::Align2::LEFT_TOP, [8.0, 8.0])
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(egui::Color32::from_black_alpha(160))
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                    ui.set_min_width(280.0);

                    let p = camera.position;
                    ui.label(format!("Pos:  ({:.1}, {:.1}, {:.1})", p.x, p.y, p.z));
                    ui.label(format!(
                        "Yaw:  {:.1}°  Pitch: {:.1}°",
                        camera.yaw.to_degrees(),
                        camera.pitch.to_degrees()
                    ));

                    let fwd = camera.forward();
                    ui.label(format!("Fwd:  ({:.2}, {:.2}, {:.2})", fwd.x, fwd.y, fwd.z));

                    let hours = (day_cycle.time_of_day * 24.0) as u32;
                    let mins = ((day_cycle.time_of_day * 24.0 - hours as f32) * 60.0) as u32;
                    ui.label(format!(
                        "Time: {:02}:{:02} {}",
                        hours,
                        mins,
                        if day_cycle.paused { "[PAUSED]" } else { "" }
                    ));

                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("F1: Settings").small().weak());
                });
        });
}
