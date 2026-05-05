use egui::{CollapsingHeader, RichText, ScrollArea, Slider};

pub struct LabelSettingsMut<'a> {
    pub poi: &'a mut crate::ui::poi_labels::PoiLabelSettings,
    pub street_signs: &'a mut crate::ui::poi_labels::StreetSignLabelSettings,
}

pub fn draw(
    ctx: &egui::Context,
    atm: &mut crate::atmosphere::AtmosphereSettings,
    day: &mut crate::atmosphere::DayCycleState,
    performance: &mut crate::app::PerformanceState,
    minimap: &mut crate::ui::minimap::MinimapState,
    label_settings: LabelSettingsMut<'_>,
    show: &mut bool,
) {
    egui::Window::new("Settings")
        .open(show)
        .default_width(320.0)
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                day_cycle_section(ui, day, atm);
                performance_section(ui, performance);
                minimap_section(ui, minimap);
                poi_labels_section(ui, label_settings.poi);
                street_sign_labels_section(ui, label_settings.street_signs);
                clouds_section(ui, atm);
                fog_section(ui, atm);
                sky_colors_section(ui, atm);
            });
        });
}

fn day_cycle_section(
    ui: &mut egui::Ui,
    day: &mut crate::atmosphere::DayCycleState,
    atm: &mut crate::atmosphere::AtmosphereSettings,
) {
    CollapsingHeader::new(RichText::new("Day / Night Cycle").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut day.paused, "Paused");

            let mut hours = day.time_of_day * 24.0;
            if ui
                .add(
                    Slider::new(&mut hours, 0.0..=24.0)
                        .step_by(0.1)
                        .text("Time"),
                )
                .changed()
            {
                day.time_of_day = hours / 24.0;
            }

            ui.add(
                Slider::new(&mut atm.ambient_light, 0.0..=1.0)
                    .step_by(0.01)
                    .text("Ambient Light"),
            );

            ui.checkbox(&mut atm.shadow_cascade_debug, "Debug shadow cascades");
            ui.label("Debug colors: blue = near, orange = mid, purple = far fade");
        });
}

fn performance_section(ui: &mut egui::Ui, performance: &mut crate::app::PerformanceState) {
    CollapsingHeader::new(RichText::new("Performance").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut performance.show_fps, "Show FPS counter");
            ui.label(format!("Current FPS: {:.0}", performance.fps));
        });
}

fn minimap_section(ui: &mut egui::Ui, minimap: &mut crate::ui::minimap::MinimapState) {
    CollapsingHeader::new(RichText::new("Minimap").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut minimap.visible, "Visible");
            ui.checkbox(&mut minimap.rotate_with_camera, "Rotate map with camera");
        });
}

fn poi_labels_section(ui: &mut egui::Ui, poi_labels: &mut crate::ui::poi_labels::PoiLabelSettings) {
    CollapsingHeader::new(RichText::new("POI Labels").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut poi_labels.visible, "Visible");
            ui.add(
                Slider::new(&mut poi_labels.max_distance, 50.0..=2000.0)
                    .step_by(25.0)
                    .text("Max distance (m)"),
            );
        });
}

fn street_sign_labels_section(
    ui: &mut egui::Ui,
    street_sign_labels: &mut crate::ui::poi_labels::StreetSignLabelSettings,
) {
    CollapsingHeader::new(RichText::new("Street Sign Labels").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut street_sign_labels.visible, "Visible");
            ui.add(
                Slider::new(&mut street_sign_labels.max_distance, 50.0..=2000.0)
                    .step_by(25.0)
                    .text("Max distance (m)"),
            );
        });
}

fn clouds_section(ui: &mut egui::Ui, atm: &mut crate::atmosphere::AtmosphereSettings) {
    CollapsingHeader::new(RichText::new("Clouds").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut atm.clouds_enabled, "Enabled");

            ui.add(
                Slider::new(&mut atm.cloud_speed, 0.0..=3.0)
                    .step_by(0.01)
                    .text("Speed"),
            );

            ui.add(
                Slider::new(&mut atm.cloud_coverage, 0.0..=1.0)
                    .step_by(0.01)
                    .text("Coverage"),
            );

            color_edit_rgb(ui, "Cloud Color", &mut atm.cloud_color);
        });
}

fn fog_section(ui: &mut egui::Ui, atm: &mut crate::atmosphere::AtmosphereSettings) {
    CollapsingHeader::new(RichText::new("Fog").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.add(
                Slider::new(&mut atm.fog_density, 0.0..=0.01)
                    .step_by(0.0001)
                    .text("Density"),
            );

            ui.add(
                Slider::new(&mut atm.fog_start, 0.0..=5000.0)
                    .step_by(10.0)
                    .text("Start Distance"),
            );
        });
}

fn sky_colors_section(ui: &mut egui::Ui, atm: &mut crate::atmosphere::AtmosphereSettings) {
    CollapsingHeader::new(RichText::new("Sky Colors").strong())
        .default_open(true)
        .show(ui, |ui| {
            color_edit_rgb(ui, "Zenith", &mut atm.sky_color_zenith);
            color_edit_rgb(ui, "Horizon", &mut atm.sky_color_horizon);
            color_edit_rgb(ui, "Ground Ambient", &mut atm.ground_color);

            if ui.button("Reset to Defaults").clicked() {
                let defaults = crate::atmosphere::AtmosphereSettings::default();
                atm.sky_color_zenith = defaults.sky_color_zenith;
                atm.sky_color_horizon = defaults.sky_color_horizon;
                atm.ground_color = defaults.ground_color;
            }
        });
}

fn color_edit_rgb(ui: &mut egui::Ui, label: &str, arr: &mut [f32; 3]) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.color_edit_button_rgb(arr);
    });
}
