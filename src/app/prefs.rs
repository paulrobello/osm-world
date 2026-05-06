use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct UserPrefs {
    pub minimap: MinimapPrefs,
    pub camera: Option<CameraPrefs>,
    pub settings_sections: SettingsSectionsPrefs,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct SettingsSectionsPrefs {
    pub day_cycle: bool,
    pub performance: bool,
    pub visual_detail: bool,
    pub minimap: bool,
    pub area_switch: bool,
    pub poi_labels: bool,
    pub address_labels: bool,
    pub street_sign_labels: bool,
    pub clouds: bool,
    pub fog: bool,
    pub sky_colors: bool,
}

impl SettingsSectionsPrefs {
    pub fn all_collapsed(&self) -> bool {
        !self.day_cycle
            && !self.performance
            && !self.visual_detail
            && !self.minimap
            && !self.area_switch
            && !self.poi_labels
            && !self.address_labels
            && !self.street_sign_labels
            && !self.clouds
            && !self.fog
            && !self.sky_colors
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct MinimapPrefs {
    pub visible: bool,
    pub zoom: f32,
    pub rotate_with_camera: bool,
    pub show_tile_debug: bool,
}

impl Default for MinimapPrefs {
    fn default() -> Self {
        let state = crate::ui::minimap::MinimapState::default();
        Self::from_minimap_state(&state)
    }
}

impl MinimapPrefs {
    pub fn from_minimap_state(state: &crate::ui::minimap::MinimapState) -> Self {
        Self {
            visible: state.visible,
            zoom: state.zoom,
            rotate_with_camera: state.rotate_with_camera,
            show_tile_debug: state.show_tile_debug,
        }
    }

    pub fn apply_to_minimap_state(&self, state: &mut crate::ui::minimap::MinimapState) {
        state.visible = self.visible;
        state.zoom = self.zoom;
        state.rotate_with_camera = self.rotate_with_camera;
        state.show_tile_debug = self.show_tile_debug;
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CameraPrefs {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl CameraPrefs {
    pub fn from_camera(camera: &crate::camera::Flycam) -> Self {
        Self {
            x: camera.position.x,
            y: camera.position.y,
            z: camera.position.z,
            yaw: camera.yaw,
            pitch: camera.pitch,
        }
    }

    pub fn apply_to_camera(&self, camera: &mut crate::camera::Flycam) {
        camera.position = glam::vec3(self.x, self.y, self.z);
        camera.yaw = self.yaw;
        camera.pitch = self.pitch;
    }
}

pub fn load_user_prefs() -> UserPrefs {
    load_user_prefs_from_path(&prefs_path()).unwrap_or_default()
}

pub fn save_user_prefs(prefs: &UserPrefs) -> anyhow::Result<()> {
    save_user_prefs_to_path(prefs, &prefs_path())
}

fn prefs_path() -> PathBuf {
    static PREFS_PATH: OnceLock<PathBuf> = OnceLock::new();
    PREFS_PATH.get_or_init(resolve_prefs_path).clone()
}

fn resolve_prefs_path() -> PathBuf {
    if let Some(path) = std::env::var_os("OSM_WORLD_PREFS_PATH") {
        return PathBuf::from(path);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("osm-world").join("prefs.json")
}

fn load_user_prefs_from_path(path: &Path) -> anyhow::Result<UserPrefs> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn save_user_prefs_to_path(prefs: &UserPrefs, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(prefs)?;
    std::fs::write(&tmp_path, bytes)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_prefs_file_loads_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let prefs = load_user_prefs_from_path(&tmp.path().join("missing.json")).unwrap_or_default();

        assert_eq!(prefs, UserPrefs::default());
    }

    #[test]
    fn default_settings_sections_start_collapsed() {
        let prefs = UserPrefs::default();

        assert!(prefs.settings_sections.all_collapsed());
    }

    #[test]
    fn settings_sections_round_trip_to_json_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("prefs.json");
        let prefs = UserPrefs {
            settings_sections: SettingsSectionsPrefs {
                day_cycle: true,
                minimap: true,
                sky_colors: true,
                ..Default::default()
            },
            ..Default::default()
        };

        save_user_prefs_to_path(&prefs, &path).unwrap();
        let loaded = load_user_prefs_from_path(&path).unwrap();

        assert_eq!(loaded.settings_sections, prefs.settings_sections);
    }

    #[test]
    fn minimap_prefs_round_trip_to_json_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("prefs.json");
        let prefs = UserPrefs {
            minimap: MinimapPrefs {
                visible: false,
                zoom: 875.0,
                rotate_with_camera: true,
                show_tile_debug: false,
            },
            camera: Some(CameraPrefs {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                yaw: 4.0,
                pitch: 5.0,
            }),
            settings_sections: SettingsSectionsPrefs::default(),
        };

        save_user_prefs_to_path(&prefs, &path).unwrap();
        let loaded = load_user_prefs_from_path(&path).unwrap();

        assert_eq!(loaded, prefs);
    }

    #[test]
    fn camera_prefs_round_trip_through_camera() {
        let mut camera = crate::camera::Flycam::new(1.6);
        camera.position = glam::vec3(12.0, 34.0, -56.0);
        camera.yaw = 1.25;
        camera.pitch = -0.35;
        let prefs = CameraPrefs::from_camera(&camera);

        let mut restored = crate::camera::Flycam::new(1.6);
        prefs.apply_to_camera(&mut restored);

        assert_eq!(restored.position, camera.position);
        assert_eq!(restored.yaw, camera.yaw);
        assert_eq!(restored.pitch, camera.pitch);
        assert_eq!(restored.aspect, 1.6);
    }

    #[test]
    fn minimap_prefs_apply_without_touching_texture_id() {
        let mut state = crate::ui::minimap::MinimapState {
            visible: true,
            zoom: 500.0,
            rotate_with_camera: false,
            show_tile_debug: true,
            texture_id: Some(egui::TextureId::User(42)),
        };
        let texture_id = state.texture_id;
        let prefs = MinimapPrefs {
            visible: false,
            zoom: 1000.0,
            rotate_with_camera: true,
            show_tile_debug: false,
        };

        prefs.apply_to_minimap_state(&mut state);

        assert!(!state.visible);
        assert_eq!(state.zoom, 1000.0);
        assert!(state.rotate_with_camera);
        assert!(!state.show_tile_debug);
        assert_eq!(state.texture_id, texture_id);
    }
}
