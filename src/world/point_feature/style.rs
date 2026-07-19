//! Point-feature tag classification. Maps OSM tag combinations to a typed
//! [`PointFeatureStyle`] (tree / landmark / nature / POI / transit) and to a
//! human-readable label. Drives both the geometry dispatcher in `mod.rs`
//! and the UI label/inspection layers.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointFeatureKind {
    Tree,
    Landmark,
    Nature,
    Poi,
    Transit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoiCategory {
    Food,
    Service,
    Shop,
    Tourism,
    Leisure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LandmarkKind {
    Generic,
    Tower,
    WaterTower,
    Chimney,
    Monument,
    Peak,
    Viewpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointFeatureStyle {
    pub kind: PointFeatureKind,
    pub poi_category: Option<PoiCategory>,
    pub landmark_kind: Option<LandmarkKind>,
}

pub fn point_feature_style(tags: &HashMap<String, String>) -> Option<PointFeatureStyle> {
    if tags.get("natural").map(String::as_str) == Some("tree") {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Tree,
            poi_category: None,
            landmark_kind: None,
        });
    }

    if let Some(landmark_kind) = landmark_kind(tags) {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Landmark,
            poi_category: None,
            landmark_kind: Some(landmark_kind),
        });
    }

    if matches!(
        tags.get("natural").map(String::as_str),
        Some("rock" | "spring")
    ) {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Nature,
            poi_category: None,
            landmark_kind: None,
        });
    }
    if crate::world::transit::transit_kind(tags).is_some() {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Transit,
            poi_category: None,
            landmark_kind: None,
        });
    }
    if let Some(category) = poi_category(tags) {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Poi,
            poi_category: Some(category),
            landmark_kind: None,
        });
    }
    None
}

fn landmark_kind(tags: &HashMap<String, String>) -> Option<LandmarkKind> {
    match tags.get("man_made").map(String::as_str) {
        Some("tower") => return Some(LandmarkKind::Tower),
        Some("water_tower") => return Some(LandmarkKind::WaterTower),
        Some("chimney") => return Some(LandmarkKind::Chimney),
        _ => {}
    }

    if matches!(
        tags.get("historic").map(String::as_str),
        Some("monument" | "memorial")
    ) || tags.contains_key("memorial")
    {
        return Some(LandmarkKind::Monument);
    }

    if tags.get("natural").map(String::as_str) == Some("peak") {
        return Some(LandmarkKind::Peak);
    }

    if tags.get("tourism").map(String::as_str) == Some("viewpoint") {
        return Some(LandmarkKind::Viewpoint);
    }

    if matches!(
        tags.get("tourism").map(String::as_str),
        Some("attraction" | "artwork")
    ) || tags.contains_key("historic")
    {
        return Some(LandmarkKind::Generic);
    }

    None
}

pub fn point_feature_label(tags: &HashMap<String, String>) -> Option<String> {
    if let Some(name) = tags
        .get("name")
        .map(String::as_str)
        .filter(|name| !name.trim().is_empty())
    {
        return Some(name.trim().to_string());
    }
    let style = point_feature_style(tags)?;
    match style.kind {
        PointFeatureKind::Landmark => Some("Landmark".to_string()),
        PointFeatureKind::Poi => Some(match style.poi_category? {
            PoiCategory::Food => "Food".to_string(),
            PoiCategory::Service => "Service".to_string(),
            PoiCategory::Shop => "Shop".to_string(),
            PoiCategory::Tourism => "Tourism".to_string(),
            PoiCategory::Leisure => "Park".to_string(),
        }),
        PointFeatureKind::Transit => crate::world::transit::transit_label(tags),
        PointFeatureKind::Tree | PointFeatureKind::Nature => None,
    }
}

fn poi_category(tags: &HashMap<String, String>) -> Option<PoiCategory> {
    if matches!(
        tags.get("amenity").map(String::as_str),
        Some("restaurant" | "cafe" | "bar" | "pub" | "fast_food")
    ) {
        return Some(PoiCategory::Food);
    }
    if matches!(
        tags.get("amenity").map(String::as_str),
        Some(
            "school" | "library" | "hospital" | "clinic" | "pharmacy" | "bank" | "fuel" | "parking"
        )
    ) {
        return Some(PoiCategory::Service);
    }
    if tags.contains_key("shop") {
        return Some(PoiCategory::Shop);
    }
    if matches!(
        tags.get("tourism").map(String::as_str),
        Some("hotel" | "museum" | "guest_house")
    ) {
        return Some(PoiCategory::Tourism);
    }
    if matches!(
        tags.get("leisure").map(String::as_str),
        Some("park" | "playground" | "sports_centre" | "pitch")
    ) {
        return Some(PoiCategory::Leisure);
    }
    None
}
