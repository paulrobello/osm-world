//! Vegetation (tree) scattering for landuse areas.

use std::collections::HashMap;

use crate::geo::CoordConverter;
use crate::world::loader::source::{ResolvedFeature, ResolvedPointFeature};

#[derive(Clone, Copy)]
struct TreeAreaConfig {
    spacing_metres: f32,
    max_points: usize,
}

fn tree_area_config(tags: &HashMap<String, String>) -> Option<TreeAreaConfig> {
    if matches!(
        tags.get("landuse").map(String::as_str),
        Some("forest" | "orchard")
    ) || matches!(tags.get("natural").map(String::as_str), Some("wood"))
    {
        return Some(TreeAreaConfig {
            spacing_metres: 28.0,
            max_points: 120,
        });
    }
    if matches!(
        tags.get("landuse").map(String::as_str),
        Some("grass" | "meadow" | "recreation_ground" | "cemetery")
    ) || matches!(
        tags.get("leisure").map(String::as_str),
        Some("park" | "garden")
    ) {
        return Some(TreeAreaConfig {
            spacing_metres: 45.0,
            max_points: 12,
        });
    }
    None
}

pub fn append_tree_area_point_features_with_settings(
    area: &ResolvedFeature,
    conv: &CoordConverter,
    elev: &impl Fn(f64, f64) -> f32,
    visual_detail: &crate::visual_detail::VisualDetailSettings,
    point_features: &mut Vec<ResolvedPointFeature>,
) {
    if area.points.len() < 3 {
        return;
    }
    let density = visual_detail.vegetation_density;
    if !density.is_finite() || density <= 0.0 {
        return;
    }

    let Some(config) = tree_area_config(&area.tags) else {
        return;
    };
    let Some((min_x, min_z, max_x, max_z)) = super::geometry::feature_bbox(area) else {
        return;
    };
    let spacing_metres = config.spacing_metres / density.sqrt().max(0.25);
    let max_points = config.max_points.min(visual_detail.synthetic_tree_cap);
    let mut candidates = Vec::new();
    let mut row = 0usize;
    let mut z = min_z + spacing_metres * 0.5;
    while z <= max_z {
        let row_offset = if row.is_multiple_of(2) {
            0.0
        } else {
            spacing_metres * 0.5
        };
        let mut x = min_x + spacing_metres * 0.5 + row_offset;
        while x <= max_x {
            if super::geometry::point_in_polygon((x, z), &area.points) {
                candidates.push((x, z));
            }
            x += spacing_metres;
        }
        row += 1;
        z += spacing_metres;
    }

    for (x, z) in evenly_capped_points(&candidates, max_points) {
        let (lat, lon) = conv.world_xz_to_lat_lon(x, z);
        point_features.push(ResolvedPointFeature {
            tags: HashMap::from([("natural".to_string(), "tree".to_string())]),
            point: (x, z),
            elevation: elev(lat, lon),
            rep_lat: lat,
            rep_lon: lon,
        });
    }
}

fn evenly_capped_points(candidates: &[(f32, f32)], max_points: usize) -> Vec<(f32, f32)> {
    if candidates.len() <= max_points {
        return candidates.to_vec();
    }
    if max_points == 0 {
        return Vec::new();
    }
    if max_points == 1 {
        return vec![candidates[candidates.len() / 2]];
    }

    let last = candidates.len() - 1;
    (0..max_points)
        .map(|i| {
            let index = (i * last + (max_points - 1) / 2) / (max_points - 1);
            candidates[index]
        })
        .collect()
}
