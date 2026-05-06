use egui::{CollapsingHeader, ComboBox, RichText, ScrollArea, Slider};

pub struct LabelSettingsMut<'a> {
    pub poi: &'a mut crate::ui::poi_labels::PoiLabelSettings,
    pub street_signs: &'a mut crate::ui::poi_labels::StreetSignLabelSettings,
}

pub struct SettingsDrawState<'a> {
    pub atmosphere: &'a mut crate::atmosphere::AtmosphereSettings,
    pub day_cycle: &'a mut crate::atmosphere::DayCycleState,
    pub performance: &'a mut crate::app::PerformanceState,
    pub minimap: &'a mut crate::ui::minimap::MinimapState,
    pub label_settings: LabelSettingsMut<'a>,
    pub area_switch: &'a mut crate::app::AreaSwitchState,
    pub visual_detail: &'a mut crate::visual_detail::VisualDetailSettings,
    pub show: &'a mut bool,
}

pub fn draw(ctx: &egui::Context, state: SettingsDrawState<'_>) {
    egui::Window::new("Settings")
        .open(state.show)
        .default_width(320.0)
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                day_cycle_section(ui, state.day_cycle, state.atmosphere);
                performance_section(ui, state.performance);
                visual_detail_section(ui, state.visual_detail);
                minimap_section(ui, state.minimap);
                area_switch_section(ui, state.area_switch);
                poi_labels_section(ui, state.label_settings.poi);
                street_sign_labels_section(ui, state.label_settings.street_signs);
                clouds_section(ui, state.atmosphere);
                fog_section(ui, state.atmosphere);
                sky_colors_section(ui, state.atmosphere);
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

fn visual_detail_section(
    ui: &mut egui::Ui,
    settings: &mut crate::visual_detail::VisualDetailSettings,
) {
    CollapsingHeader::new(RichText::new("Visual Detail").strong())
        .default_open(true)
        .show(ui, |ui| {
            let mut selected_preset = settings.preset;
            ComboBox::from_label("Preset")
                .selected_text(format!("{:?}", selected_preset))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut selected_preset,
                        crate::visual_detail::VisualPreset::Performance,
                        "Performance",
                    );
                    ui.selectable_value(
                        &mut selected_preset,
                        crate::visual_detail::VisualPreset::Balanced,
                        "Balanced",
                    );
                    ui.selectable_value(
                        &mut selected_preset,
                        crate::visual_detail::VisualPreset::Showcase,
                        "Showcase",
                    );
                });
            if selected_preset != settings.preset {
                apply_visual_preset(settings, selected_preset);
            }
            let mut landmark_changed = false;
            ComboBox::from_label("Landmark detail")
                .selected_text(format!("{:?}", settings.landmark_detail))
                .show_ui(ui, |ui| {
                    landmark_changed |= ui
                        .selectable_value(
                            &mut settings.landmark_detail,
                            crate::visual_detail::LandmarkDetail::Off,
                            "Off",
                        )
                        .changed();
                    landmark_changed |= ui
                        .selectable_value(
                            &mut settings.landmark_detail,
                            crate::visual_detail::LandmarkDetail::Simple,
                            "Simple",
                        )
                        .changed();
                    landmark_changed |= ui
                        .selectable_value(
                            &mut settings.landmark_detail,
                            crate::visual_detail::LandmarkDetail::Showcase,
                            "Showcase",
                        )
                        .changed();
                });
            mark_reload_required_if_changed(settings, landmark_changed, true);
            ui.checkbox(&mut settings.vegetation_visible, "Vegetation visible");
            ui.add(
                Slider::new(&mut settings.facade_variation, 0.0..=1.0)
                    .step_by(0.01)
                    .text("Facade variation"),
            );
            let roof_changed = ui
                .add(
                    Slider::new(&mut settings.roof_variation, 0.0..=1.0)
                        .step_by(0.01)
                        .text("Roof variation"),
                )
                .changed();
            mark_reload_required_if_changed(settings, roof_changed, true);
            if ui
                .add(
                    Slider::new(&mut settings.vegetation_density, 0.0..=3.0)
                        .step_by(0.05)
                        .text("Vegetation density"),
                )
                .changed()
            {
                settings.reload_required = true;
            }
            if ui
                .add(
                    Slider::new(&mut settings.synthetic_tree_cap, 1..=1000)
                        .text("Synthetic tree cap"),
                )
                .changed()
            {
                settings.reload_required = true;
            }
            ui.add(
                Slider::new(&mut settings.vegetation_max_distance, 0.0..=8000.0)
                    .step_by(25.0)
                    .text("Vegetation max distance"),
            );
            settings.clamp();
            if settings.reload_required {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Reload area to apply placement or baked-detail changes.",
                );
            }
        });
}

fn apply_visual_preset(
    settings: &mut crate::visual_detail::VisualDetailSettings,
    preset: crate::visual_detail::VisualPreset,
) {
    if settings.preset == preset {
        return;
    }
    let mut next = crate::visual_detail::VisualDetailSettings::from_preset(preset);
    next.reload_required = true;
    *settings = next;
}

fn mark_reload_required_if_changed(
    settings: &mut crate::visual_detail::VisualDetailSettings,
    changed: bool,
    requires_reload: bool,
) {
    if changed && requires_reload {
        settings.reload_required = true;
    }
}

fn minimap_section(ui: &mut egui::Ui, minimap: &mut crate::ui::minimap::MinimapState) {
    CollapsingHeader::new(RichText::new("Minimap").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut minimap.visible, "Visible");
            ui.checkbox(&mut minimap.rotate_with_camera, "Rotate map with camera");
        });
}

fn area_switch_section(ui: &mut egui::Ui, area_switch: &mut crate::app::AreaSwitchState) {
    CollapsingHeader::new(RichText::new("Area Switching").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.label("Load a prepared .osm file without restarting. Camera height, yaw, pitch, labels, lighting, and minimap settings are preserved.");
            ui.horizontal(|ui| {
                ui.label("OSM path");
                ui.text_edit_singleline(&mut area_switch.input_path);
            });
            ui.horizontal(|ui| {
                ui.label("SRTM dir");
                ui.text_edit_singleline(&mut area_switch.srtm_dir);
            });
            if ui.button("Load prepared area").clicked() {
                area_switch.request_load = true;
            }
            if !area_switch.status.is_empty() {
                ui.label(&area_switch.status);
            }
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

#[cfg(test)]
mod tests {
    use super::{apply_visual_preset, mark_reload_required_if_changed};

    #[test]
    fn applying_showcase_preset_updates_visible_vegetation_controls_and_marks_reload() {
        let mut settings = crate::visual_detail::VisualDetailSettings::from_preset(
            crate::visual_detail::VisualPreset::Balanced,
        );

        apply_visual_preset(&mut settings, crate::visual_detail::VisualPreset::Showcase);

        assert_eq!(
            settings.preset,
            crate::visual_detail::VisualPreset::Showcase
        );
        assert!(settings.vegetation_density > 1.0);
        assert!(settings.synthetic_tree_cap > 120);
        assert_eq!(
            settings.landmark_detail,
            crate::visual_detail::LandmarkDetail::Showcase
        );
        assert!(settings.reload_required);
    }

    #[test]
    fn reload_required_marker_sets_flag_only_for_changed_reload_required_controls() {
        let mut settings = crate::visual_detail::VisualDetailSettings::default();

        mark_reload_required_if_changed(&mut settings, false, true);
        assert!(!settings.reload_required);

        mark_reload_required_if_changed(&mut settings, true, false);
        assert!(!settings.reload_required);

        mark_reload_required_if_changed(&mut settings, true, true);
        assert!(settings.reload_required);
    }
}
