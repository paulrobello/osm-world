//! Geometry utilities for polygon operations.

/// Ensure CCW winding for polygon features while keeping per-vertex data aligned.
pub fn ensure_ccw(poly: &mut [(f32, f32)], elevations: &mut [f32]) {
    if poly.len() < 3 {
        return;
    }
    let area: f32 = poly
        .iter()
        .enumerate()
        .map(|(i, (x0, y0))| {
            let (x1, y1) = poly[(i + 1) % poly.len()];
            x0 * y1 - x1 * y0
        })
        .sum();
    if area < 0.0 {
        poly.reverse();
        elevations.reverse();
    }
}

pub fn feature_bbox(feature: &super::ResolvedFeature) -> Option<(f32, f32, f32, f32)> {
    let mut iter = feature.points.iter();
    let &(first_x, first_z) = iter.next()?;
    let (mut min_x, mut max_x) = (first_x, first_x);
    let (mut min_z, mut max_z) = (first_z, first_z);
    for &(x, z) in iter {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    Some((min_x, min_z, max_x, max_z))
}

pub fn feature_owner_tile(
    feature: &super::ResolvedFeature,
    tile_size: f32,
) -> Option<crate::stream::TileCoord> {
    let (min_x, min_z, max_x, max_z) = feature_bbox(feature)?;
    let center_x = (min_x + max_x) * 0.5;
    let center_z = (min_z + max_z) * 0.5;
    Some(crate::stream::TileCoord::from_world(
        center_x, center_z, tile_size,
    ))
}

pub fn point_in_polygon(point: (f32, f32), polygon: &[(f32, f32)]) -> bool {
    let (x, z) = point;
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (xi, zi) = polygon[i];
        let (xj, zj) = polygon[j];
        if (zi > z) != (zj > z) {
            let x_intersection = (xj - xi) * (z - zi) / (zj - zi) + xi;
            if x < x_intersection {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

pub fn clip_polygon_to_rect(
    points: &[(f32, f32)],
    rect: crate::stream::TileRect,
) -> Vec<(f32, f32)> {
    let mut polygon = points.to_vec();
    if polygon.len() >= 2 && polygon.first() == polygon.last() {
        polygon.pop();
    }
    polygon = clip_polygon_edge(
        polygon,
        |point| point.0 >= rect.min_x,
        |a, b| intersect_at_x(a, b, rect.min_x),
    );
    polygon = clip_polygon_edge(
        polygon,
        |point| point.0 <= rect.max_x,
        |a, b| intersect_at_x(a, b, rect.max_x),
    );
    polygon = clip_polygon_edge(
        polygon,
        |point| point.1 >= rect.min_z,
        |a, b| intersect_at_z(a, b, rect.min_z),
    );
    clip_polygon_edge(
        polygon,
        |point| point.1 <= rect.max_z,
        |a, b| intersect_at_z(a, b, rect.max_z),
    )
}

fn clip_polygon_edge(
    polygon: Vec<(f32, f32)>,
    inside: impl Fn((f32, f32)) -> bool,
    intersect: impl Fn((f32, f32), (f32, f32)) -> (f32, f32),
) -> Vec<(f32, f32)> {
    if polygon.is_empty() {
        return polygon;
    }
    let mut clipped = Vec::new();
    let mut previous = *polygon.last().unwrap_or(&(0.0, 0.0));
    let mut previous_inside = inside(previous);
    for current in polygon {
        let current_inside = inside(current);
        if current_inside {
            if !previous_inside {
                clipped.push(intersect(previous, current));
            }
            clipped.push(current);
        } else if previous_inside {
            clipped.push(intersect(previous, current));
        }
        previous = current;
        previous_inside = current_inside;
    }
    clipped.dedup_by(|a, b| super::mesh::same_road_point(*a, *b));
    clipped
}

fn intersect_at_x(a: (f32, f32), b: (f32, f32), x: f32) -> (f32, f32) {
    let dx = b.0 - a.0;
    if dx.abs() <= f32::EPSILON {
        return (x, a.1);
    }
    let t = ((x - a.0) / dx).clamp(0.0, 1.0);
    (x, a.1 + (b.1 - a.1) * t)
}

fn intersect_at_z(a: (f32, f32), b: (f32, f32), z: f32) -> (f32, f32) {
    let dz = b.1 - a.1;
    if dz.abs() <= f32::EPSILON {
        return (a.0, z);
    }
    let t = ((z - a.1) / dz).clamp(0.0, 1.0);
    (a.0 + (b.0 - a.0) * t, z)
}

pub fn feature_indices_intersecting_tiles(
    features: &[super::ResolvedFeature],
    rects: &[crate::stream::TileRect],
) -> Vec<usize> {
    features
        .iter()
        .enumerate()
        .filter_map(|(idx, feature)| {
            let bbox = feature_bbox(feature)?;
            rects
                .iter()
                .any(|rect| bbox_intersects_rect(bbox, *rect))
                .then_some(idx)
        })
        .collect()
}

pub fn bbox_intersects_rect(
    (min_x, min_z, max_x, max_z): (f32, f32, f32, f32),
    rect: crate::stream::TileRect,
) -> bool {
    min_x < rect.max_x && max_x > rect.min_x && min_z < rect.max_z && max_z > rect.min_z
}

pub fn containing_building_name<'a>(
    point: (f32, f32),
    buildings: &'a [super::ResolvedFeature],
) -> Option<&'a str> {
    buildings
        .iter()
        .find(|building| point_in_polygon(point, &building.points))?
        .tags
        .get("name")
        .map(String::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

pub fn move_point_outside_containing_building(
    point: (f32, f32),
    buildings: &[super::ResolvedFeature],
) -> (f32, f32) {
    buildings
        .iter()
        .find(|building| point_in_polygon(point, &building.points))
        .and_then(|building| move_point_outside_polygon(point, &building.points))
        .unwrap_or(point)
}

pub fn move_point_outside_polygon(
    point: (f32, f32),
    polygon: &[(f32, f32)],
) -> Option<(f32, f32)> {
    let nearest = nearest_point_on_polygon_edges(point, polygon)?;
    let to_edge = glam::vec2(nearest.0 - point.0, nearest.1 - point.1);
    let fallback = {
        let center = polygon_center(polygon)?;
        glam::vec2(nearest.0 - center.0, nearest.1 - center.1)
    };
    let direction = if to_edge.length_squared() > 1e-8 {
        to_edge.normalize()
    } else if fallback.length_squared() > 1e-8 {
        fallback.normalize()
    } else {
        glam::Vec2::X
    };
    Some((
        nearest.0 + direction.x * super::source::POINT_FEATURE_BUILDING_CLEARANCE_METRES,
        nearest.1 + direction.y * super::source::POINT_FEATURE_BUILDING_CLEARANCE_METRES,
    ))
}

fn nearest_point_on_polygon_edges(point: (f32, f32), polygon: &[(f32, f32)]) -> Option<(f32, f32)> {
    if polygon.len() < 2 {
        return None;
    }

    let p = glam::vec2(point.0, point.1);
    let mut best: Option<(f32, (f32, f32))> = None;
    for i in 0..polygon.len() {
        let a = glam::vec2(polygon[i].0, polygon[i].1);
        let b = glam::vec2(
            polygon[(i + 1) % polygon.len()].0,
            polygon[(i + 1) % polygon.len()].1,
        );
        let ab = b - a;
        if ab.length_squared() <= 1e-8 {
            continue;
        }
        let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        let candidate = a + ab * t;
        let dist_sq = p.distance_squared(candidate);
        if best.is_none_or(|(best_dist_sq, _)| dist_sq < best_dist_sq) {
            best = Some((dist_sq, (candidate.x, candidate.y)));
        }
    }
    best.map(|(_, point)| point)
}

fn polygon_center(polygon: &[(f32, f32)]) -> Option<(f32, f32)> {
    let mut sum_x = 0.0;
    let mut sum_z = 0.0;
    let mut count = 0usize;
    for (index, point) in polygon.iter().enumerate() {
        if index + 1 == polygon.len() && polygon.first() == Some(point) {
            continue;
        }
        sum_x += point.0;
        sum_z += point.1;
        count += 1;
    }
    (count > 0).then_some((sum_x / count as f32, sum_z / count as f32))
}

fn next_down_f32(value: f32) -> f32 {
    if value.is_nan() || value == f32::NEG_INFINITY {
        value
    } else if value == f32::INFINITY {
        f32::MAX
    } else if value == 0.0 {
        -f32::MIN_POSITIVE
    } else if value > 0.0 {
        f32::from_bits(value.to_bits() - 1)
    } else {
        f32::from_bits(value.to_bits() + 1)
    }
}

pub fn tiles_for_half_open_bbox(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    tile_size: f32,
) -> Vec<crate::stream::TileCoord> {
    if tile_size <= 0.0
        || !min_x.is_finite()
        || !min_z.is_finite()
        || !max_x.is_finite()
        || !max_z.is_finite()
        || min_x >= max_x
        || min_z >= max_z
    {
        return Vec::new();
    }

    let start = crate::stream::TileCoord::from_world(min_x, min_z, tile_size);
    let end =
        crate::stream::TileCoord::from_world(next_down_f32(max_x), next_down_f32(max_z), tile_size);
    let mut out = Vec::new();
    for z in start.z..=end.z {
        for x in start.x..=end.x {
            out.push(crate::stream::TileCoord { x, z });
        }
    }
    out
}
