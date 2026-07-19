//! Search / fly-to overlay: builds an index of named world features and lets
//! the user jump the camera to a result by substring match.

/// Category used to label and group search results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchCategory {
    Address,
    Landmark,
    Poi,
    Road,
    Transit,
}

/// A single searchable feature: its display label, category, and world position.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchEntry {
    pub label: String,
    pub category: SearchCategory,
    pub position: glam::Vec3,
}

impl SearchEntry {
    /// Constructs a search entry from a label, category, and world-space position.
    pub fn new(label: impl Into<String>, category: SearchCategory, position: glam::Vec3) -> Self {
        Self {
            label: label.into(),
            category,
            position,
        }
    }
}

/// Mutable UI state for the search window: just the current query string.
#[derive(Clone, Debug, Default)]
pub struct SearchState {
    pub query: String,
}

/// Returns the default search-window position, placed just to the right of the HUD.
pub fn search_window_default_pos() -> egui::Pos2 {
    egui::pos2(
        crate::ui::hud::HUD_LEFT + crate::ui::hud::HUD_MIN_WIDTH + 16.0,
        crate::ui::hud::HUD_TOP,
    )
}

/// Filters `entries` by case-insensitive substring match against `query`,
/// returning up to `limit` clones.
pub fn search_entries(entries: &[SearchEntry], query: &str, limit: usize) -> Vec<SearchEntry> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() || limit == 0 {
        return Vec::new();
    }
    entries
        .iter()
        .filter(|entry| entry.label.to_ascii_lowercase().contains(&query))
        .take(limit)
        .cloned()
        .collect()
}

/// Moves `camera` to an overhead vantage point above and behind `entry`, facing
/// the result horizontally and pitched down ~32 degrees.
pub fn fly_to_entry(camera: &mut crate::camera::Flycam, entry: &SearchEntry) {
    camera.position = glam::vec3(
        entry.position.x,
        entry.position.y.max(0.0) + 55.0,
        entry.position.z + 85.0,
    );
    camera.yaw = -std::f32::consts::FRAC_PI_2;
    camera.pitch = (-32.0_f32).to_radians();
}

/// Builds the searchable index from named roads, buildings (by address),
/// address points, transit routes, transit point features, landmarks, and POIs
/// in `source`.
pub fn build_search_index(source: &crate::world::loader::WorldSource) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for road in &source.roads {
        if let Some(name) = tag_value(&road.tags, "name") {
            entries.push(SearchEntry::new(
                name,
                SearchCategory::Road,
                feature_position(road, 1.5),
            ));
        }
    }
    for building in &source.buildings {
        if let Some(address) = crate::world::address::address_full_text(&building.tags) {
            entries.push(SearchEntry::new(
                address,
                SearchCategory::Address,
                feature_position(building, 2.5),
            ));
        }
    }
    for address in &source.address_points {
        if let Some(label) = crate::world::address::address_full_text(&address.tags) {
            entries.push(SearchEntry::new(
                label,
                SearchCategory::Address,
                glam::vec3(address.point.0, address.elevation + 2.0, address.point.1),
            ));
        }
    }
    for route in &source.transit_routes {
        entries.push(SearchEntry::new(
            crate::world::transit::transit_route_label(&route.tags),
            SearchCategory::Transit,
            feature_position(route, 1.8),
        ));
    }
    for point in &source.point_features {
        if let Some(label) = crate::world::transit::transit_label(&point.tags) {
            entries.push(SearchEntry::new(
                label,
                SearchCategory::Transit,
                glam::vec3(point.point.0, point.elevation + 2.0, point.point.1),
            ));
        } else if let Some(label) = crate::world::point_feature::point_feature_label(&point.tags) {
            let category = crate::world::point_feature::point_feature_style(&point.tags)
                .map(|style| match style.kind {
                    crate::world::point_feature::PointFeatureKind::Landmark => {
                        SearchCategory::Landmark
                    }
                    crate::world::point_feature::PointFeatureKind::Transit => {
                        SearchCategory::Transit
                    }
                    _ => SearchCategory::Poi,
                })
                .unwrap_or(SearchCategory::Poi);
            entries.push(SearchEntry::new(
                label,
                category,
                glam::vec3(point.point.0, point.elevation + 2.0, point.point.1),
            ));
        }
    }
    entries
}

/// Draws the "Search / Fly To" window: a query text field and a list of up to
/// eight matching entries. Clicking a result flies the camera to it and keeps
/// the entry's label in the query box.
pub fn draw(
    ctx: &egui::Context,
    state: &mut SearchState,
    entries: &[SearchEntry],
    camera: &mut crate::camera::Flycam,
) {
    egui::Window::new("Search / Fly To")
        .default_pos(search_window_default_pos())
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search");
                ui.text_edit_singleline(&mut state.query);
            });
            for entry in search_entries(entries, &state.query, 8) {
                let text = format!("{} · {:?}", entry.label, entry.category);
                if ui.button(text).clicked() {
                    fly_to_entry(camera, &entry);
                    state.query = entry.label;
                }
            }
        });
}

fn tag_value(tags: &std::collections::HashMap<String, String>, key: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_state_filters_case_insensitively_and_limits_results() {
        let entries = vec![
            SearchEntry::new(
                "Main Street",
                SearchCategory::Road,
                glam::vec3(1.0, 2.0, 3.0),
            ),
            SearchEntry::new(
                "Main Street Cafe",
                SearchCategory::Poi,
                glam::vec3(4.0, 5.0, 6.0),
            ),
            SearchEntry::new("Broadway", SearchCategory::Road, glam::vec3(7.0, 8.0, 9.0)),
        ];

        let results = search_entries(&entries, "main", 1);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "Main Street");
    }

    #[test]
    fn fly_to_moves_camera_above_and_behind_result() {
        let entry = SearchEntry::new("Library", SearchCategory::Poi, glam::vec3(10.0, 2.0, -20.0));
        let mut camera = crate::camera::Flycam::new(1.0);

        fly_to_entry(&mut camera, &entry);

        assert_eq!(camera.position.x, 10.0);
        assert!(camera.position.y > 40.0);
        assert!(camera.position.z > -20.0);
    }

    #[test]
    fn search_window_starts_to_the_right_of_debug_hud() {
        let pos = search_window_default_pos();

        assert_eq!(
            pos.x,
            crate::ui::hud::HUD_LEFT + crate::ui::hud::HUD_MIN_WIDTH + 16.0
        );
        assert_eq!(pos.y, crate::ui::hud::HUD_TOP);
    }
}
