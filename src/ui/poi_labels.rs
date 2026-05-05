use crate::camera::Flycam;
use crate::world::loader::ResolvedPointFeature;
use crate::world::point_feature::{PointFeatureKind, point_feature_label, point_feature_style};

const MAX_VISIBLE_LABELS: usize = 24;

#[derive(Clone, Debug)]
pub struct PoiLabelSettings {
    pub visible: bool,
    pub max_distance: f32,
}

impl Default for PoiLabelSettings {
    fn default() -> Self {
        Self {
            visible: true,
            max_distance: 300.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PoiLabel {
    pub text: String,
    pub position: glam::Vec3,
}

pub fn labels_from_point_features(point_features: &[ResolvedPointFeature]) -> Vec<PoiLabel> {
    point_features
        .iter()
        .filter_map(|feature| {
            let style = point_feature_style(&feature.tags)?;
            if !matches!(
                style.kind,
                PointFeatureKind::Poi | PointFeatureKind::Landmark
            ) {
                return None;
            }
            let text = point_feature_label(&feature.tags)?;
            let y_offset = match style.kind {
                PointFeatureKind::Landmark => 5.8,
                PointFeatureKind::Poi => 5.4,
                PointFeatureKind::Tree | PointFeatureKind::Nature => return None,
            };
            Some(PoiLabel {
                text,
                position: glam::vec3(
                    feature.point.0,
                    feature.elevation + y_offset,
                    feature.point.1,
                ),
            })
        })
        .collect()
}

pub fn draw(
    ctx: &egui::Context,
    camera: &Flycam,
    labels: &[PoiLabel],
    settings: &PoiLabelSettings,
    viewport_size: egui::Vec2,
) {
    if !settings.visible || labels.is_empty() {
        return;
    }

    let mut visible: Vec<_> = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| {
            let distance = label.position.distance(camera.position);
            if distance > settings.max_distance {
                return None;
            }
            let screen_pos = project_world_to_screen(camera, label.position, viewport_size)?;
            Some((distance, index, label, screen_pos))
        })
        .collect();
    visible.sort_by(|a, b| a.0.total_cmp(&b.0));

    for (_distance, index, label, screen_pos) in visible.into_iter().take(MAX_VISIBLE_LABELS) {
        egui::Area::new(egui::Id::new(("poi_label", index)))
            .order(egui::Order::Foreground)
            .interactable(false)
            .fixed_pos(screen_pos + egui::vec2(8.0, -30.0))
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(egui::Color32::from_black_alpha(185))
                    .corner_radius(3.0)
                    .inner_margin(egui::Margin::symmetric(5, 2))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&label.text)
                                .color(egui::Color32::WHITE)
                                .small(),
                        );
                    });
            });
    }
}

fn project_world_to_screen(
    camera: &Flycam,
    world_position: glam::Vec3,
    viewport_size: egui::Vec2,
) -> Option<egui::Pos2> {
    if viewport_size.x <= 0.0 || viewport_size.y <= 0.0 {
        return None;
    }
    let clip = (camera.projection_matrix() * camera.view_matrix()) * world_position.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 {
        return None;
    }
    Some(egui::pos2(
        (ndc.x + 1.0) * 0.5 * viewport_size.x,
        (1.0 - ndc.y) * 0.5 * viewport_size.y,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn labels_include_named_pois_and_skip_trees() {
        let features = vec![
            ResolvedPointFeature {
                tags: tags(&[("amenity", "restaurant"), ("name", "Taco Bell")]),
                point: (1.0, 2.0),
                elevation: 3.0,
                rep_lat: 0.0,
                rep_lon: 0.0,
            },
            ResolvedPointFeature {
                tags: tags(&[("natural", "tree")]),
                point: (4.0, 5.0),
                elevation: 6.0,
                rep_lat: 0.0,
                rep_lon: 0.0,
            },
        ];

        let labels = labels_from_point_features(&features);

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].text, "Taco Bell");
        assert_eq!(labels[0].position, glam::vec3(1.0, 8.4, 2.0));
    }

    #[test]
    fn label_settings_default_to_nearby_labels_only() {
        let settings = PoiLabelSettings::default();

        assert!(settings.visible);
        assert_eq!(settings.max_distance, 300.0);
    }

    #[test]
    fn projects_visible_world_points_to_screen() {
        let mut camera = Flycam::new(1.0);
        camera.position = glam::Vec3::ZERO;
        camera.yaw = -std::f32::consts::FRAC_PI_2;
        camera.pitch = 0.0;

        let screen = project_world_to_screen(
            &camera,
            glam::vec3(0.0, 0.0, -10.0),
            egui::vec2(100.0, 100.0),
        )
        .unwrap();

        assert!((screen.x - 50.0).abs() < 0.001);
        assert!((screen.y - 50.0).abs() < 0.001);
        assert!(
            project_world_to_screen(
                &camera,
                glam::vec3(0.0, 0.0, 10.0),
                egui::vec2(100.0, 100.0)
            )
            .is_none()
        );
    }
}
