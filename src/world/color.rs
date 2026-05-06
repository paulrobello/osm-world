//! Feature color scheme for OSM map elements.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildingStyle {
    pub wall_color: [f32; 3],
    pub roof_color: [f32; 3],
    pub band_color: [f32; 3],
    pub facade_intensity: f32,
    pub roof_intensity: f32,
}

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

pub fn building_style(
    tags: &HashMap<String, String>,
    seed: u64,
    facade_intensity: f32,
    roof_intensity: f32,
) -> BuildingStyle {
    let facade_intensity = clamp_unit(facade_intensity);
    let roof_intensity = clamp_unit(roof_intensity);
    let base_wall = explicit_color(tags, &["building:colour", "building:color"])
        .unwrap_or_else(|| building_color(tags));
    let wall_color = if has_strong_wall_material(tags) {
        base_wall
    } else {
        vary_color(
            base_wall,
            seed ^ 0x57a1_1fac_ade0_0001,
            facade_intensity,
            0.16,
        )
    };
    let roof_color = explicit_color(tags, &["roof:colour", "roof:color"])
        .or_else(|| roof_material_color(tags))
        .unwrap_or_else(|| default_roof_color(tags, seed, roof_intensity));
    let band_color = vary_color(
        wall_color,
        seed ^ 0xbaad_faca_de00_0002,
        facade_intensity,
        0.22,
    );

    BuildingStyle {
        wall_color,
        roof_color,
        band_color,
        facade_intensity,
        roof_intensity,
    }
}

fn has_strong_wall_material(tags: &HashMap<String, String>) -> bool {
    tags.contains_key("building:material")
        || tags.contains_key("building:colour")
        || tags.contains_key("building:color")
        || tags.contains_key("material")
}

fn roof_material_color(tags: &HashMap<String, String>) -> Option<[f32; 3]> {
    let material = tags
        .get("roof:material")
        .or_else(|| tags.get("roof:material:colour"))?;
    match material.as_str() {
        "brick" | "tile" | "tiles" | "roof_tiles" => Some([0.55, 0.22, 0.16]),
        "slate" => Some([0.25, 0.28, 0.32]),
        "metal" | "steel" | "tin" | "zinc" => Some([0.42, 0.43, 0.46]),
        "concrete" => Some([0.58, 0.58, 0.55]),
        "wood" | "timber" => Some([0.44, 0.32, 0.22]),
        "glass" => Some([0.50, 0.64, 0.70]),
        "grass" | "green_roof" => Some([0.30, 0.46, 0.24]),
        _ => None,
    }
}

fn default_roof_color(tags: &HashMap<String, String>, seed: u64, roof_intensity: f32) -> [f32; 3] {
    let base = match tags.get("roof:shape").map(String::as_str) {
        Some("flat") => [0.45, 0.43, 0.40],
        Some("gabled" | "hipped" | "pyramidal") => [0.50, 0.24, 0.18],
        Some("skillion" | "shed") => [0.40, 0.40, 0.42],
        Some("dome") => [0.36, 0.38, 0.34],
        _ => match tags.get("building").map(String::as_str) {
            Some("church" | "cathedral" | "chapel") => [0.36, 0.33, 0.30],
            Some("industrial" | "warehouse" | "factory") => [0.42, 0.42, 0.43],
            Some("commercial" | "office") => [0.38, 0.40, 0.43],
            _ => [0.48, 0.27, 0.20],
        },
    };
    vary_color(base, seed ^ 0x90ff_0000_0000_0003, roof_intensity, 0.20)
}

fn explicit_color(tags: &HashMap<String, String>, keys: &[&str]) -> Option<[f32; 3]> {
    keys.iter()
        .filter_map(|key| tags.get(*key))
        .find_map(|value| parse_osm_color(value))
}

fn parse_osm_color(value: &str) -> Option<[f32; 3]> {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "white" => Some([0.92, 0.90, 0.86]),
        "black" => Some([0.08, 0.08, 0.08]),
        "gray" | "grey" => Some([0.50, 0.50, 0.50]),
        "red" => Some([0.65, 0.18, 0.14]),
        "brown" => Some([0.45, 0.27, 0.16]),
        "orange" => Some([0.80, 0.42, 0.14]),
        "yellow" => Some([0.78, 0.68, 0.32]),
        "green" => Some([0.30, 0.50, 0.24]),
        "blue" => Some([0.25, 0.38, 0.62]),
        "beige" => Some([0.76, 0.68, 0.54]),
        _ => parse_hex_color(value.as_str()),
    }
}

fn parse_hex_color(value: &str) -> Option<[f32; 3]> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
}

fn vary_color(color: [f32; 3], seed: u64, intensity: f32, amount: f32) -> [f32; 3] {
    let warm = seed_unit(seed) * 2.0 - 1.0;
    let value = seed_unit(seed ^ 0x9e37_79b9_7f4a_7c15) * 2.0 - 1.0;
    let channel = |component: f32, bias: f32| {
        (component * (1.0 + value * amount * intensity) + bias * warm * amount * intensity)
            .clamp(0.0, 1.0)
    };
    [
        channel(color[0], 0.18),
        channel(color[1], 0.04),
        channel(color[2], -0.14),
    ]
}

fn seed_unit(seed: u64) -> f32 {
    let mut x = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    let x = x ^ (x >> 31);
    ((x >> 40) as f32) / ((1u64 << 24) as f32)
}

fn clamp_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

const VEHICLE_ROAD_COLOR: [f32; 3] = [0.02, 0.02, 0.02];
const SIDEWALK_ROAD_COLOR: [f32; 3] = [0.55, 0.55, 0.55];

pub fn is_sidewalk_like_road(tags: &HashMap<String, String>) -> bool {
    matches!(
        tags.get("highway").map(String::as_str),
        Some("footway" | "path" | "pedestrian" | "cycleway")
    )
}

/// Color for road ribbons.
pub fn road_color(tags: &HashMap<String, String>) -> [f32; 3] {
    if is_sidewalk_like_road(tags) {
        SIDEWALK_ROAD_COLOR
    } else {
        VEHICLE_ROAD_COLOR
    }
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
    [0.26, 0.27, 0.27]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_style_is_deterministic_for_same_tags_and_seed() {
        let tags = HashMap::from([("building".to_string(), "residential".to_string())]);

        let first = building_style(&tags, 42, 0.8, 0.6);
        let second = building_style(&tags, 42, 0.8, 0.6);

        assert_eq!(first, second);
        assert_eq!(first.facade_intensity, 0.8);
        assert_eq!(first.roof_intensity, 0.6);
    }

    #[test]
    fn building_style_varies_by_seed_when_no_material_tags_exist() {
        let tags = HashMap::from([("building".to_string(), "residential".to_string())]);

        let first = building_style(&tags, 7, 1.0, 1.0);
        let second = building_style(&tags, 99, 1.0, 1.0);

        assert_ne!(first.wall_color, second.wall_color);
        assert_ne!(first.roof_color, second.roof_color);
    }

    #[test]
    fn building_style_respects_roof_material_tags() {
        let tags = HashMap::from([
            ("building:material".to_string(), "brick".to_string()),
            ("roof:material".to_string(), "metal".to_string()),
        ]);

        let style = building_style(&tags, 1, 1.4, -0.5);

        assert_eq!(style.wall_color, building_color(&tags));
        assert_eq!(style.roof_color, [0.42, 0.43, 0.46]);
        assert_eq!(style.facade_intensity, 1.0);
        assert_eq!(style.roof_intensity, 0.0);
    }

    #[test]
    fn terrain_color_is_dark_grey() {
        assert_eq!(terrain_color(), [0.26, 0.27, 0.27]);
    }

    #[test]
    fn road_color_keeps_vehicle_roads_black() {
        let primary = HashMap::from([("highway".to_string(), "primary".to_string())]);
        let service = HashMap::from([("highway".to_string(), "service".to_string())]);

        assert_eq!(road_color(&primary), VEHICLE_ROAD_COLOR);
        assert_eq!(road_color(&service), VEHICLE_ROAD_COLOR);
    }

    #[test]
    fn road_color_makes_sidewalk_like_ways_grey() {
        for highway in ["footway", "path", "pedestrian", "cycleway"] {
            let tags = HashMap::from([("highway".to_string(), highway.to_string())]);
            assert_eq!(road_color(&tags), SIDEWALK_ROAD_COLOR);
        }
    }
}
