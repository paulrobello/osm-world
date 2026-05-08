#[derive(
    Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum VisualPreset {
    Performance,
    Balanced,
    Showcase,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum LandmarkDetail {
    Off,
    Simple,
    Showcase,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisualDetailSettings {
    pub preset: VisualPreset,
    pub landmark_detail: LandmarkDetail,
    pub facade_variation: f32,
    pub roof_variation: f32,
    pub vegetation_visible: bool,
    pub vegetation_density: f32,
    pub synthetic_tree_cap: usize,
    pub vegetation_max_distance: f32,
    pub bike_ped_overlay: bool,
    pub reload_required: bool,
}

impl Default for VisualDetailSettings {
    fn default() -> Self {
        Self::from_preset(VisualPreset::Balanced)
    }
}

impl VisualDetailSettings {
    pub fn from_preset(preset: VisualPreset) -> Self {
        match preset {
            VisualPreset::Performance => Self {
                preset,
                landmark_detail: LandmarkDetail::Simple,
                facade_variation: 0.25,
                roof_variation: 0.25,
                vegetation_visible: true,
                vegetation_density: 0.35,
                synthetic_tree_cap: 60,
                vegetation_max_distance: 1200.0,
                bike_ped_overlay: false,
                reload_required: false,
            },
            VisualPreset::Balanced => Self {
                preset,
                landmark_detail: LandmarkDetail::Showcase,
                facade_variation: 0.65,
                roof_variation: 0.65,
                vegetation_visible: true,
                vegetation_density: 1.0,
                synthetic_tree_cap: 120,
                vegetation_max_distance: 2600.0,
                bike_ped_overlay: false,
                reload_required: false,
            },
            VisualPreset::Showcase => Self {
                preset,
                landmark_detail: LandmarkDetail::Showcase,
                facade_variation: 1.0,
                roof_variation: 1.0,
                vegetation_visible: true,
                vegetation_density: 1.8,
                synthetic_tree_cap: 240,
                vegetation_max_distance: 4200.0,
                bike_ped_overlay: false,
                reload_required: false,
            },
        }
    }

    pub fn clamp(&mut self) {
        self.facade_variation = clamp_finite(self.facade_variation, 0.0, 1.0);
        self.roof_variation = clamp_finite(self.roof_variation, 0.0, 1.0);
        self.vegetation_density = clamp_finite(self.vegetation_density, 0.0, 3.0);
        self.synthetic_tree_cap = self.synthetic_tree_cap.max(1);
        if !self.vegetation_max_distance.is_finite() || self.vegetation_max_distance < 0.0 {
            self.vegetation_max_distance = 0.0;
        }
    }

    pub fn with_reload_required(mut self) -> Self {
        self.reload_required = true;
        self
    }
}

fn clamp_finite(value: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        min
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showcase_preset_enables_rich_visual_detail() {
        let settings = VisualDetailSettings::from_preset(VisualPreset::Showcase);

        assert_eq!(settings.preset, VisualPreset::Showcase);
        assert_eq!(settings.landmark_detail, LandmarkDetail::Showcase);
        assert_eq!(settings.facade_variation, 1.0);
        assert_eq!(settings.roof_variation, 1.0);
        assert!(settings.vegetation_visible);
        assert_eq!(settings.vegetation_density, 1.8);
        assert_eq!(settings.synthetic_tree_cap, 240);
        assert_eq!(settings.vegetation_max_distance, 4200.0);
        assert!(!settings.reload_required);
    }

    #[test]
    fn performance_preset_reduces_clutter() {
        let settings = VisualDetailSettings::from_preset(VisualPreset::Performance);

        assert_eq!(settings.preset, VisualPreset::Performance);
        assert_eq!(settings.landmark_detail, LandmarkDetail::Simple);
        assert_eq!(settings.facade_variation, 0.25);
        assert_eq!(settings.roof_variation, 0.25);
        assert!(settings.vegetation_visible);
        assert_eq!(settings.vegetation_density, 0.35);
        assert_eq!(settings.synthetic_tree_cap, 60);
        assert_eq!(settings.vegetation_max_distance, 1200.0);
        assert!(!settings.reload_required);
    }

    #[test]
    fn default_uses_balanced_preset_and_reload_flag_can_be_set() {
        let settings = VisualDetailSettings::default();

        assert_eq!(settings.preset, VisualPreset::Balanced);
        assert_eq!(settings.landmark_detail, LandmarkDetail::Showcase);
        assert_eq!(settings.facade_variation, 0.65);
        assert_eq!(settings.roof_variation, 0.65);
        assert!(!settings.reload_required);

        assert!(settings.with_reload_required().reload_required);
    }

    #[test]
    fn clamp_prevents_invalid_values() {
        let mut settings = VisualDetailSettings {
            preset: VisualPreset::Balanced,
            landmark_detail: LandmarkDetail::Showcase,
            facade_variation: 1.5,
            roof_variation: -0.5,
            vegetation_visible: true,
            vegetation_density: 4.0,
            synthetic_tree_cap: 0,
            vegetation_max_distance: -10.0,
            bike_ped_overlay: true,
            reload_required: false,
        };

        settings.clamp();

        assert_eq!(settings.facade_variation, 1.0);
        assert_eq!(settings.roof_variation, 0.0);
        assert_eq!(settings.vegetation_density, 3.0);
        assert_eq!(settings.synthetic_tree_cap, 1);
        assert_eq!(settings.vegetation_max_distance, 0.0);
    }
}
