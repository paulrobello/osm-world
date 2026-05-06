pub const HUD_LEFT: f32 = 8.0;
pub const HUD_TOP: f32 = 8.0;
pub const HUD_MIN_WIDTH: f32 = 280.0;

pub fn draw(
    ctx: &egui::Context,
    camera: &crate::camera::Flycam,
    camera_lat_lon: Option<(f64, f64)>,
    day_cycle: &crate::atmosphere::DayCycleState,
    performance: &crate::app::PerformanceState,
) {
    egui::Area::new(egui::Id::new("camera_hud"))
        .anchor(egui::Align2::LEFT_TOP, [HUD_LEFT, HUD_TOP])
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(egui::Color32::from_black_alpha(160))
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                    ui.set_min_width(HUD_MIN_WIDTH);

                    if performance.show_fps {
                        ui.label(format!("FPS:  {:.0}", performance.fps));
                    }

                    let p = camera.position;
                    ui.label(format!("Pos:  ({:.1}, {:.1}, {:.1})", p.x, p.y, p.z));
                    if let Some((lat, lon)) = camera_lat_lon {
                        ui.label(format!("Lat/Lon: {:.6}, {:.6}", lat, lon));
                    }
                    ui.label(format!(
                        "Yaw:  {:.1}°  Pitch: {:.1}°",
                        camera.yaw.to_degrees(),
                        camera.pitch.to_degrees()
                    ));

                    let fwd = camera.forward();
                    ui.label(format!("Fwd:  ({:.2}, {:.2}, {:.2})", fwd.x, fwd.y, fwd.z));

                    let hours = (day_cycle.time_of_day * 24.0) as u32;
                    let mins = ((day_cycle.time_of_day * 24.0 - hours as f32) * 60.0) as u32;
                    let time_mode = if day_cycle.real_clock {
                        "[REAL]"
                    } else if day_cycle.paused {
                        "[PAUSED]"
                    } else {
                        ""
                    };
                    ui.label(format!("Time: {:02}:{:02} {}", hours, mins, time_mode));

                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("F1: Settings").small().weak());
                });
        });
}
