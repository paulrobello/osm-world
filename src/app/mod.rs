pub mod event_handler;
pub mod init;
pub mod prefs;
pub mod render_loop;
pub mod update;

use crate::camera::CameraController;

const PREFS_SAVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

pub use init::AppState;

#[derive(Clone, Debug)]
pub struct StreamingOptions {
    pub enabled: bool,
    pub tile_size: f32,
    pub stream_radius: f32,
    pub upload_budget_mb: f32,
    pub max_uploaded_tiles: usize,
    pub max_uploaded_mb: f32,
}

impl Default for StreamingOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            tile_size: 1000.0,
            stream_radius: 15_000.0,
            upload_budget_mb: 4.0,
            max_uploaded_tiles: 256,
            max_uploaded_mb: 512.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AreaSwitchState {
    pub input_path: String,
    pub srtm_dir: String,
    pub status: String,
    pub request_load: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AreaSwitchRequest {
    pub input_path: String,
    pub srtm_dir: Option<String>,
}

impl AreaSwitchState {
    pub fn take_request(&mut self) -> Option<AreaSwitchRequest> {
        if !self.request_load {
            return None;
        }
        self.request_load = false;
        let input_path = self.input_path.trim();
        if input_path.is_empty() {
            self.status = "Enter a prepared .osm path before loading.".to_string();
            return None;
        }
        let srtm_dir = match self.srtm_dir.trim() {
            "" => None,
            value => Some(value.to_string()),
        };
        Some(AreaSwitchRequest {
            input_path: input_path.to_string(),
            srtm_dir,
        })
    }
}

pub struct AppOptions {
    pub window_width: f64,
    pub window_height: f64,
    pub screenshot_path: Option<String>,
    pub screenshot_delay: f32,
    pub auto_exit_delay: Option<f32>,
    pub input_path: Option<String>,
    pub srtm_dir: Option<String>,
    pub cam_override: Option<crate::camera::CameraOverride>,
    pub show_settings: bool,
    pub initial_time_of_day: Option<f32>,
    pub real_time_of_day: bool,
    pub hide_poi_labels: bool,
    pub hide_address_labels: bool,
    pub hide_street_sign_labels: bool,
    pub hide_minimap: bool,
    pub rotate_minimap: bool,
    pub debug_shadow_cascades: bool,
    pub streaming: StreamingOptions,
    pub visual_detail: crate::visual_detail::VisualDetailSettings,
    pub visual_detail_overridden: bool,
}

#[derive(Clone, Debug)]
pub struct PerformanceState {
    pub show_fps: bool,
    pub fps: f32,
    smoothed_frame_time: f32,
}

impl Default for PerformanceState {
    fn default() -> Self {
        Self {
            show_fps: true,
            fps: 0.0,
            smoothed_frame_time: 0.0,
        }
    }
}

impl PerformanceState {
    pub fn update(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }

        self.smoothed_frame_time = if self.smoothed_frame_time == 0.0 {
            dt
        } else {
            self.smoothed_frame_time * 0.9 + dt * 0.1
        };
        self.fps = 1.0 / self.smoothed_frame_time;
    }
}

/// UI panel state: labels, search, inspect, minimap, area switch, settings toggle.
pub struct AppUiState {
    pub show_settings: bool,
    pub minimap: crate::ui::minimap::MinimapState,
    pub settings_sections: crate::app::prefs::SettingsSectionsPrefs,
    pub poi_labels: crate::ui::poi_labels::PoiLabelSettings,
    pub address_labels: crate::ui::poi_labels::AddressLabelSettings,
    pub street_sign_labels: crate::ui::poi_labels::StreetSignLabelSettings,
    pub search: crate::ui::search::SearchState,
    pub inspect: crate::ui::inspect::InspectState,
    pub last_cursor_pos: Option<egui::Pos2>,
    pub area_switch: AreaSwitchState,
}

/// Render pipeline state: atmosphere, day cycle, visual detail, performance metrics.
pub struct AppRenderState {
    pub atmosphere: crate::atmosphere::AtmosphereSettings,
    pub day_cycle: crate::atmosphere::DayCycleState,
    pub visual_detail: crate::visual_detail::VisualDetailSettings,
    pub performance: PerformanceState,
    pub render_start: Option<std::time::Instant>,
    pub screenshot_taken: bool,
}

/// Persisted preferences and save tracking.
pub struct AppViewState {
    pub persisted_minimap: crate::app::prefs::MinimapPrefs,
    pub persisted_camera: Option<crate::app::prefs::CameraPrefs>,
    pub persisted_settings_sections: crate::app::prefs::SettingsSectionsPrefs,
    pub persisted_visual_detail: crate::app::prefs::VisualDetailPrefs,
    pub last_prefs_save: std::time::Instant,
}

pub struct App {
    pub state: Option<AppState>,
    pub egui: Option<crate::ui::EguiState>,
    pub controller: CameraController,
    pub last_frame_time: std::time::Instant,
    pub opts: AppOptions,
    pub ui: AppUiState,
    pub render: AppRenderState,
    pub view: AppViewState,
}

impl App {
    pub fn new(opts: AppOptions) -> Self {
        let atmosphere = crate::atmosphere::AtmosphereSettings {
            shadow_cascade_debug: opts.debug_shadow_cascades,
            ..Default::default()
        };

        let mut day_cycle = crate::atmosphere::DayCycleState::default();
        if let Some(time_of_day) = opts.initial_time_of_day {
            day_cycle.time_of_day = time_of_day;
        }
        day_cycle.real_clock = opts.real_time_of_day;

        let prefs = crate::app::prefs::load_user_prefs();
        let visual_detail = resolve_initial_visual_detail(
            &opts.visual_detail,
            &prefs.visual_detail,
            opts.visual_detail_overridden,
        );
        let mut minimap = crate::ui::minimap::MinimapState::default();
        prefs.minimap.apply_to_minimap_state(&mut minimap);
        if opts.hide_minimap {
            minimap.visible = false;
        }
        if opts.rotate_minimap {
            minimap.rotate_with_camera = true;
        }
        let poi_labels = crate::ui::poi_labels::PoiLabelSettings {
            visible: !opts.hide_poi_labels,
            ..Default::default()
        };
        let address_labels = crate::ui::poi_labels::AddressLabelSettings {
            visible: !opts.hide_address_labels,
            ..Default::default()
        };
        let street_sign_labels = crate::ui::poi_labels::StreetSignLabelSettings {
            visible: !opts.hide_street_sign_labels,
            ..Default::default()
        };
        let area_switch = AreaSwitchState {
            input_path: opts.input_path.clone().unwrap_or_default(),
            srtm_dir: opts.srtm_dir.clone().unwrap_or_default(),
            status: String::new(),
            request_load: false,
        };

        Self {
            state: None,
            egui: None,
            controller: CameraController::new(),
            last_frame_time: std::time::Instant::now(),
            opts,
            ui: AppUiState {
                show_settings: true,
                minimap,
                settings_sections: prefs.settings_sections.clone(),
                poi_labels,
                address_labels,
                street_sign_labels,
                search: crate::ui::search::SearchState::default(),
                inspect: crate::ui::inspect::InspectState::default(),
                last_cursor_pos: None,
                area_switch,
            },
            render: AppRenderState {
                atmosphere,
                day_cycle,
                visual_detail,
                performance: PerformanceState::default(),
                render_start: None,
                screenshot_taken: false,
            },
            view: AppViewState {
                persisted_minimap: prefs.minimap,
                persisted_camera: prefs.camera,
                persisted_settings_sections: prefs.settings_sections,
                persisted_visual_detail: prefs.visual_detail,
                last_prefs_save: std::time::Instant::now() - PREFS_SAVE_INTERVAL,
            },
        }
    }
}

fn resolve_initial_visual_detail(
    cli_settings: &crate::visual_detail::VisualDetailSettings,
    persisted: &crate::app::prefs::VisualDetailPrefs,
    cli_overridden: bool,
) -> crate::visual_detail::VisualDetailSettings {
    if cli_overridden {
        cli_settings.clone()
    } else {
        persisted.to_visual_detail_settings()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_visual_detail_is_used_when_cli_does_not_override() {
        let cli = crate::visual_detail::VisualDetailSettings::from_preset(
            crate::visual_detail::VisualPreset::Balanced,
        );
        let prefs = crate::app::prefs::VisualDetailPrefs::from_visual_detail_settings(
            &crate::visual_detail::VisualDetailSettings {
                vegetation_density: 2.5,
                synthetic_tree_cap: 333,
                ..crate::visual_detail::VisualDetailSettings::from_preset(
                    crate::visual_detail::VisualPreset::Showcase,
                )
            },
        );

        let resolved = resolve_initial_visual_detail(&cli, &prefs, false);

        assert_eq!(
            resolved.preset,
            crate::visual_detail::VisualPreset::Showcase
        );
        assert_eq!(resolved.vegetation_density, 2.5);
        assert_eq!(resolved.synthetic_tree_cap, 333);
    }

    #[test]
    fn explicit_cli_visual_detail_overrides_persisted_prefs() {
        let cli = crate::visual_detail::VisualDetailSettings::from_preset(
            crate::visual_detail::VisualPreset::Performance,
        );
        let prefs = crate::app::prefs::VisualDetailPrefs::from_visual_detail_settings(
            &crate::visual_detail::VisualDetailSettings::from_preset(
                crate::visual_detail::VisualPreset::Showcase,
            ),
        );

        let resolved = resolve_initial_visual_detail(&cli, &prefs, true);

        assert_eq!(
            resolved.preset,
            crate::visual_detail::VisualPreset::Performance
        );
    }

    #[test]
    fn app_starts_with_settings_panel_visible_by_default() {
        let app = App::new(AppOptions {
            window_width: 800.0,
            window_height: 600.0,
            screenshot_path: None,
            screenshot_delay: 0.0,
            auto_exit_delay: None,
            input_path: None,
            srtm_dir: None,
            cam_override: None,
            show_settings: false,
            initial_time_of_day: None,
            real_time_of_day: false,
            hide_poi_labels: false,
            hide_address_labels: false,
            hide_street_sign_labels: false,
            hide_minimap: false,
            rotate_minimap: false,
            debug_shadow_cascades: false,
            streaming: StreamingOptions::default(),
            visual_detail: crate::visual_detail::VisualDetailSettings::default(),
            visual_detail_overridden: false,
        });

        assert!(app.ui.show_settings);
    }

    #[test]
    fn area_switch_state_trims_paths_and_ignores_empty_input() {
        let mut empty = AreaSwitchState {
            input_path: "   ".to_string(),
            srtm_dir: " /tmp/srtm ".to_string(),
            status: String::new(),
            request_load: true,
        };
        assert!(empty.take_request().is_none());
        assert!(!empty.request_load);

        let mut with_srtm = AreaSwitchState {
            input_path: " /tmp/city.osm ".to_string(),
            srtm_dir: " /tmp/srtm ".to_string(),
            status: String::new(),
            request_load: true,
        };
        let request = with_srtm.take_request().unwrap();
        assert_eq!(request.input_path, "/tmp/city.osm");
        assert_eq!(request.srtm_dir.as_deref(), Some("/tmp/srtm"));

        let mut without_srtm = AreaSwitchState {
            input_path: "/tmp/city.osm".to_string(),
            srtm_dir: "  ".to_string(),
            status: String::new(),
            request_load: true,
        };
        let request = without_srtm.take_request().unwrap();
        assert_eq!(request.srtm_dir, None);
    }
}
