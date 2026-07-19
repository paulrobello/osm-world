//! Feature inspector: identifies world features under the cursor and renders
//! the inspector window that shows their label, kind, and tags.

use std::collections::HashMap;

/// High-level kind of an inspectable feature, shown in the inspector header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureKind {
    Address,
    Building,
    Landmark,
    Poi,
    Road,
    StreetSign,
    Transit,
}

/// Geometric shape used for hit-testing a feature against a screen click.
#[derive(Clone, Debug, PartialEq)]
pub enum IdentifyShape {
    Point(glam::Vec3),
    Polyline(Vec<glam::Vec3>),
    Polygon(Vec<glam::Vec3>),
}

/// A world feature that can be selected in the inspector.
#[derive(Clone, Debug, PartialEq)]
pub struct IdentifiableFeature {
    pub kind: FeatureKind,
    pub label: String,
    pub position: glam::Vec3,
    pub shape: IdentifyShape,
    pub tags: HashMap<String, String>,
}

impl IdentifiableFeature {
    /// Constructs a point-shaped feature with the given kind, label, position,
    /// and tag map.
    pub fn new(
        kind: FeatureKind,
        label: String,
        position: glam::Vec3,
        tags: HashMap<String, String>,
    ) -> Self {
        Self {
            kind,
            label,
            position,
            shape: IdentifyShape::Point(position),
            tags,
        }
    }

    /// Builder that replaces the default point shape with a polyline or polygon
    /// so hit-testing follows the feature's real geometry.
    pub fn with_shape(mut self, shape: IdentifyShape) -> Self {
        self.shape = shape;
        self
    }
}

/// A feature already projected to a screen position, used when picking from
/// pre-projected candidates (for example, label overlays).
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenIdentifiable {
    pub feature: IdentifiableFeature,
    pub screen_pos: egui::Pos2,
}

/// Mutable UI state for the inspector: the currently selected feature, if any.
#[derive(Clone, Debug, Default)]
pub struct InspectState {
    pub selected: Option<IdentifiableFeature>,
}

/// Picks the closest pre-projected feature to `click` within `max_radius`
/// screen pixels. Returns `None` if nothing is in range.
pub fn pick_screen_identifiable(
    features: &[ScreenIdentifiable],
    click: egui::Pos2,
    max_radius: f32,
) -> Option<IdentifiableFeature> {
    features
        .iter()
        .filter_map(|feature| {
            let distance = feature.screen_pos.distance(click);
            (distance <= max_radius).then_some((distance, feature.feature.clone()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, feature)| feature)
}

/// Picks the closest feature to `click` by projecting each feature's shape into
/// screen space with `camera` and `viewport_size`, then taking the one inside
/// `max_radius` pixels. Returns `None` if nothing is in range.
pub fn pick_identifiable(
    features: &[IdentifiableFeature],
    camera: &crate::camera::Flycam,
    viewport_size: egui::Vec2,
    click: egui::Pos2,
    max_radius: f32,
) -> Option<IdentifiableFeature> {
    features
        .iter()
        .filter_map(|feature| {
            let distance = screen_shape_distance(feature, camera, viewport_size, click)?;
            (distance <= max_radius).then_some((distance, feature.clone()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, feature)| feature)
}

fn screen_shape_distance(
    feature: &IdentifiableFeature,
    camera: &crate::camera::Flycam,
    viewport_size: egui::Vec2,
    click: egui::Pos2,
) -> Option<f32> {
    match &feature.shape {
        IdentifyShape::Point(point) => project_point(camera, *point, viewport_size)
            .map(|screen_pos| screen_pos.distance(click)),
        IdentifyShape::Polyline(points) => {
            let screen_points = project_points(camera, points, viewport_size);
            polyline_screen_distance(&screen_points, click)
        }
        IdentifyShape::Polygon(points) => {
            let screen_points = project_points(camera, points, viewport_size);
            if screen_points.len() >= 3 {
                polygon_screen_distance(&screen_points, click)
            } else {
                project_point(camera, feature.position, viewport_size)
                    .map(|screen_pos| screen_pos.distance(click))
            }
        }
    }
}

fn project_point(
    camera: &crate::camera::Flycam,
    point: glam::Vec3,
    viewport_size: egui::Vec2,
) -> Option<egui::Pos2> {
    crate::ui::poi_labels::project_world_to_screen(camera, point, viewport_size)
}

fn project_points(
    camera: &crate::camera::Flycam,
    points: &[glam::Vec3],
    viewport_size: egui::Vec2,
) -> Vec<egui::Pos2> {
    points
        .iter()
        .filter_map(|point| project_point(camera, *point, viewport_size))
        .collect()
}

fn polyline_screen_distance(points: &[egui::Pos2], click: egui::Pos2) -> Option<f32> {
    if points.is_empty() {
        return None;
    }
    if points.len() == 1 {
        return Some(points[0].distance(click));
    }
    points
        .windows(2)
        .map(|segment| point_segment_distance(click, segment[0], segment[1]))
        .min_by(f32::total_cmp)
}

fn polygon_screen_distance(points: &[egui::Pos2], click: egui::Pos2) -> Option<f32> {
    if points.len() < 3 {
        return polyline_screen_distance(points, click);
    }
    if point_in_polygon(click, points) {
        return Some(0.0);
    }
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(&a, &b)| point_segment_distance(click, a, b))
        .min_by(f32::total_cmp)
}

fn point_segment_distance(point: egui::Pos2, start: egui::Pos2, end: egui::Pos2) -> f32 {
    let ab = end - start;
    let ap = point - start;
    let len_sq = ab.dot(ab);
    if len_sq <= f32::EPSILON {
        return point.distance(start);
    }
    let t = (ap.dot(ab) / len_sq).clamp(0.0, 1.0);
    point.distance(start + ab * t)
}

fn point_in_polygon(point: egui::Pos2, polygon: &[egui::Pos2]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let pi = polygon[i];
        let pj = polygon[j];
        let crosses = (pi.y > point.y) != (pj.y > point.y)
            && point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x;
        if crosses {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Builds the full set of inspectable features from `source`: buildings (with
/// polygon outlines), roads and transit routes (as polylines), address points,
/// generated street signs, landmarks, transit stops, and POIs.
pub fn build_identifiables(source: &crate::world::loader::WorldSource) -> Vec<IdentifiableFeature> {
    let mut features = Vec::new();
    for building in &source.buildings {
        let position = feature_position(building, 3.0);
        let label = crate::world::address::address_full_text(&building.tags)
            .or_else(|| tag_value(&building.tags, "name"))
            .unwrap_or_else(|| "Building".to_string());
        let kind = if crate::world::address::address_label_text(&building.tags).is_some() {
            FeatureKind::Address
        } else {
            FeatureKind::Building
        };
        features.push(
            IdentifiableFeature::new(kind, label, position, building.tags.clone())
                .with_shape(IdentifyShape::Polygon(feature_points(building, 3.0))),
        );
    }
    for road in &source.roads {
        features.push(
            IdentifiableFeature::new(
                FeatureKind::Road,
                tag_value(&road.tags, "name").unwrap_or_else(|| "Road".to_string()),
                feature_position(road, 1.5),
                road.tags.clone(),
            )
            .with_shape(IdentifyShape::Polyline(feature_points(road, 1.5))),
        );
    }
    for address in &source.address_points {
        if let Some(label) = crate::world::address::address_full_text(&address.tags) {
            features.push(IdentifiableFeature::new(
                FeatureKind::Address,
                label,
                glam::vec3(address.point.0, address.elevation + 2.0, address.point.1),
                address.tags.clone(),
            ));
        }
    }
    for sign in &source.street_signs {
        let mut tags = HashMap::new();
        tags.insert("name".to_string(), sign.name.clone());
        tags.insert("source".to_string(), "generated street_sign".to_string());
        features.push(IdentifiableFeature::new(
            FeatureKind::StreetSign,
            sign.name.clone(),
            glam::vec3(sign.point.0, sign.elevation + 2.5, sign.point.1),
            tags,
        ));
    }
    for route in &source.transit_routes {
        features.push(
            IdentifiableFeature::new(
                FeatureKind::Transit,
                crate::world::transit::transit_route_label(&route.tags),
                feature_position(route, 1.8),
                route.tags.clone(),
            )
            .with_shape(IdentifyShape::Polyline(feature_points(route, 1.8))),
        );
    }
    for point in &source.point_features {
        let position = glam::vec3(point.point.0, point.elevation + 2.5, point.point.1);
        if let Some(label) = crate::world::transit::transit_label(&point.tags) {
            features.push(IdentifiableFeature::new(
                FeatureKind::Transit,
                label,
                position,
                point.tags.clone(),
            ));
        } else if let Some(label) = crate::world::point_feature::point_feature_label(&point.tags) {
            let kind = crate::world::point_feature::point_feature_style(&point.tags)
                .map(|style| match style.kind {
                    crate::world::point_feature::PointFeatureKind::Landmark => {
                        FeatureKind::Landmark
                    }
                    crate::world::point_feature::PointFeatureKind::Transit => FeatureKind::Transit,
                    _ => FeatureKind::Poi,
                })
                .unwrap_or(FeatureKind::Poi);
            features.push(IdentifiableFeature::new(
                kind,
                label,
                position,
                point.tags.clone(),
            ));
        }
    }
    features
}

/// Draws the "Feature Inspector" window for the currently selected feature,
/// or does nothing when nothing is selected.
pub fn draw(ctx: &egui::Context, state: &mut InspectState) {
    let Some(feature) = &state.selected else {
        return;
    };
    egui::Window::new("Feature Inspector")
        .default_width(300.0)
        .show(ctx, |ui| {
            ui.heading(&feature.label);
            ui.label(format!("Type: {:?}", feature.kind));
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    let mut tags: Vec<_> = feature.tags.iter().collect();
                    tags.sort_by(|a, b| a.0.cmp(b.0));
                    for (key, value) in tags {
                        ui.horizontal_wrapped(|ui| {
                            ui.monospace(key);
                            ui.label(value);
                        });
                    }
                });
        });
}

fn tag_value(tags: &HashMap<String, String>, key: &str) -> Option<String> {
    tags.get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn feature_position(feature: &crate::world::loader::ResolvedFeature, y_offset: f32) -> glam::Vec3 {
    let len = feature.points.len().max(1) as f32;
    let (x, z) = feature
        .points
        .iter()
        .fold((0.0, 0.0), |acc, point| (acc.0 + point.0, acc.1 + point.1));
    let elevation = if feature.elevations.is_empty() {
        0.0
    } else {
        feature.elevations.iter().sum::<f32>() / feature.elevations.len() as f32
    };
    glam::vec3(x / len, elevation + y_offset, z / len)
}

fn feature_points(
    feature: &crate::world::loader::ResolvedFeature,
    y_offset: f32,
) -> Vec<glam::Vec3> {
    feature
        .points
        .iter()
        .enumerate()
        .map(|(index, &(x, z))| {
            let elevation = feature.elevations.get(index).copied().unwrap_or(0.0);
            glam::vec3(x, elevation + y_offset, z)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_identifiable_selects_nearest_screen_feature_within_radius() {
        let features = vec![
            ScreenIdentifiable {
                feature: IdentifiableFeature::new(
                    FeatureKind::Road,
                    "Far".to_string(),
                    glam::Vec3::ZERO,
                    HashMap::new(),
                ),
                screen_pos: egui::pos2(80.0, 80.0),
            },
            ScreenIdentifiable {
                feature: IdentifiableFeature::new(
                    FeatureKind::Poi,
                    "Cafe".to_string(),
                    glam::Vec3::ZERO,
                    HashMap::new(),
                ),
                screen_pos: egui::pos2(52.0, 49.0),
            },
        ];

        let picked = pick_screen_identifiable(&features, egui::pos2(50.0, 50.0), 12.0).unwrap();

        assert_eq!(picked.label, "Cafe");
        assert_eq!(picked.kind, FeatureKind::Poi);
    }

    #[test]
    fn polygon_screen_distance_is_zero_inside_shape() {
        let polygon = vec![
            egui::pos2(10.0, 10.0),
            egui::pos2(90.0, 10.0),
            egui::pos2(90.0, 90.0),
            egui::pos2(10.0, 90.0),
        ];

        assert_eq!(
            polygon_screen_distance(&polygon, egui::pos2(50.0, 50.0)),
            Some(0.0)
        );
        assert!(polygon_screen_distance(&polygon, egui::pos2(95.0, 50.0)).unwrap() <= 5.0);
    }
}
