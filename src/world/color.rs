//! Feature color scheme for OSM map elements.

use std::collections::HashMap;

/// Color for a building based on its tags.
pub fn building_color(tags: &HashMap<String, String>) -> [f32; 3] {
    if let Some(material) = tags.get("building:material") {
        match material.as_str() {
            "brick" => return [0.65, 0.32, 0.28],
            "wood" | "timber" => return [0.68, 0.55, 0.38],
            "concrete" => return [0.75, 0.75, 0.72],
            "glass" => return [0.60, 0.72, 0.78],
            "stone" => return [0.62, 0.60, 0.56],
            "steel" | "metal" => return [0.58, 0.60, 0.65],
            _ => {}
        }
    }
    match tags.get("building").map(|s| s.as_str()) {
        Some("church" | "cathedral" | "chapel") => [0.82, 0.78, 0.65],
        Some("commercial" | "office") => [0.72, 0.72, 0.75],
        Some("residential" | "apartments" | "house" | "detached" | "terrace" | "semi") => {
            [0.80, 0.72, 0.58]
        }
        Some("industrial" | "warehouse" | "factory") => [0.60, 0.60, 0.58],
        Some("retail" | "shop") => [0.78, 0.65, 0.55],
        Some("school" | "university") => [0.75, 0.70, 0.60],
        Some("hospital") => [0.85, 0.82, 0.80],
        Some("garage" | "garages") => [0.55, 0.55, 0.55],
        Some("roof") => [0.50, 0.45, 0.40],
        _ => [0.78, 0.72, 0.62],
    }
}

/// Color for road ribbons.
pub fn road_color(_tags: &HashMap<String, String>) -> [f32; 3] {
    [0.02, 0.02, 0.02]
}

/// Width in metres for a road based on its highway tag or explicit width tag.
pub fn road_width(tags: &HashMap<String, String>) -> f32 {
    if let Some(w) = tags.get("width") {
        let w = w.trim().trim_end_matches('m');
        if let Ok(v) = w.parse::<f32>() {
            return v;
        }
    }
    match tags.get("highway").map(|s| s.as_str()) {
        Some("motorway") => 14.0,
        Some("motorway_link") => 7.0,
        Some("trunk") => 10.0,
        Some("trunk_link") => 6.0,
        Some("primary") => 8.0,
        Some("primary_link") => 5.0,
        Some("secondary") => 7.0,
        Some("secondary_link") => 5.0,
        Some("tertiary") => 6.0,
        Some("tertiary_link") => 4.5,
        Some("residential") => 5.0,
        Some("unclassified") => 4.5,
        Some("living_street") => 4.0,
        Some("service") => 3.5,
        Some("track") => 3.0,
        Some("path" | "footway" | "cycleway") => 2.0,
        Some("pedestrian") => 4.0,
        _ => 4.0,
    }
}

/// Constant water color.
pub fn water_color() -> [f32; 3] {
    [0.25, 0.50, 0.72]
}

/// Color for a landuse area based on its tags.
pub fn landuse_color(tags: &HashMap<String, String>) -> [f32; 3] {
    if let Some(v) = tags.get("landuse") {
        match v.as_str() {
            "residential" => [0.75, 0.70, 0.60],
            "commercial" => [0.70, 0.68, 0.65],
            "industrial" => [0.65, 0.63, 0.60],
            "forest" | "wood" => [0.25, 0.48, 0.22],
            "farmland" | "farmyard" => [0.68, 0.65, 0.35],
            "meadow" | "grass" | "grassland" => [0.48, 0.65, 0.30],
            "recreation_ground" | "village_green" => [0.45, 0.68, 0.35],
            "cemetery" | "grave_yard" => [0.50, 0.58, 0.38],
            "allotments" => [0.55, 0.62, 0.30],
            "brownfield" | "landfill" | "construction" => [0.65, 0.60, 0.50],
            "basin" | "reservoir" => water_color(),
            _ => [0.50, 0.58, 0.40],
        }
    } else if let Some(v) = tags.get("natural") {
        match v.as_str() {
            "wood" | "tree_row" | "tree" => [0.25, 0.48, 0.22],
            "scrub" | "heath" | "moor" => [0.42, 0.52, 0.28],
            "grassland" | "meadow" => [0.48, 0.65, 0.30],
            "wetland" => water_color(),
            "beach" | "sand" => [0.85, 0.82, 0.65],
            "bare_rock" | "scree" | "shingle" => [0.60, 0.58, 0.55],
            "glacier" | "snowfield" => [0.90, 0.92, 0.95],
            _ => [0.50, 0.58, 0.40],
        }
    } else if let Some(v) = tags.get("leisure") {
        match v.as_str() {
            "park" | "garden" => [0.45, 0.68, 0.35],
            "playground" => [0.55, 0.65, 0.40],
            "sports_centre" | "pitch" | "track" => [0.40, 0.62, 0.30],
            "golf_course" => [0.35, 0.60, 0.28],
            "nature_reserve" => [0.30, 0.52, 0.25],
            _ => [0.48, 0.62, 0.38],
        }
    } else {
        [0.50, 0.58, 0.40]
    }
}

/// Constant terrain color.
pub fn terrain_color() -> [f32; 3] {
    [0.42, 0.55, 0.30]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn road_color_is_black_for_all_highway_types() {
        let primary = HashMap::from([("highway".to_string(), "primary".to_string())]);
        let service = HashMap::from([("highway".to_string(), "service".to_string())]);

        assert_eq!(road_color(&primary), [0.02, 0.02, 0.02]);
        assert_eq!(road_color(&service), [0.02, 0.02, 0.02]);
    }
}
