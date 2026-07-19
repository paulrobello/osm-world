//! Water-area and waterway mesh helpers. Handles clipped water-area emission
//! for tiled rendering, and uncovered-waterway emission that suppresses
//! segments already covered by a polygonal water area.

use std::collections::HashSet;

use crate::render::vertex::Vertex;
use crate::world::loader::geometry::{
    bbox_intersects_rect, clip_polygon_to_rect, feature_bbox, point_in_polygon,
};
use crate::world::loader::source::WorldSource;

pub(super) fn append_clipped_water_meshes(
    source: &WorldSource,
    water_refs: &[usize],
    rects: &[crate::stream::TileRect],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    for &feature_idx in water_refs {
        let Some(water) = source.waters.get(feature_idx) else {
            continue;
        };
        let Some(bbox) = feature_bbox(water) else {
            continue;
        };
        for &rect in rects {
            if !bbox_intersects_rect(bbox, rect) {
                continue;
            }
            let clipped = clip_polygon_to_rect(&water.points, rect);
            if clipped.len() < 3 {
                continue;
            }
            let elevations: Vec<_> = clipped
                .iter()
                .map(|&(x, z)| {
                    let (lat, lon) = source.conv.world_xz_to_lat_lon(x, z);
                    source.elevation_at(lat, lon)
                })
                .collect();
            crate::world::water::generate_water_with_elevations(&clipped, &elevations, verts, idxs);
        }
    }
}

pub(super) fn append_uncovered_waterway_meshes(
    source: &WorldSource,
    waterway_refs: &[usize],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let mut emitted_segments = HashSet::new();
    for &feature_idx in waterway_refs {
        let Some(waterway) = source.waterways.get(feature_idx) else {
            continue;
        };
        if waterway.points.len() != waterway.elevations.len() {
            continue;
        }
        for segment_idx in 0..waterway.points.len().saturating_sub(1) {
            let a = waterway.points[segment_idx];
            let b = waterway.points[segment_idx + 1];
            if super::same_road_point(a, b) {
                continue;
            }
            let key = normalized_segment_key(a, b);
            if !emitted_segments.insert(key) {
                continue;
            }
            let midpoint = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
            if point_is_inside_any_water_area(source, midpoint) {
                continue;
            }
            crate::world::water::generate_waterway_with_elevations(
                &[a, b],
                &[
                    waterway.elevations[segment_idx],
                    waterway.elevations[segment_idx + 1],
                ],
                &waterway.tags,
                verts,
                idxs,
            );
        }
    }
}

fn normalized_segment_key(a: (f32, f32), b: (f32, f32)) -> ((i32, i32), (i32, i32)) {
    let qa = quantized_point_key(a);
    let qb = quantized_point_key(b);
    if qa <= qb { (qa, qb) } else { (qb, qa) }
}

fn quantized_point_key(point: (f32, f32)) -> (i32, i32) {
    (
        (point.0 * 10.0).round() as i32,
        (point.1 * 10.0).round() as i32,
    )
}

fn point_is_inside_any_water_area(source: &WorldSource, point: (f32, f32)) -> bool {
    source.waters.iter().any(|water| {
        feature_bbox(water).is_some_and(|bbox| {
            point.0 >= bbox.0
                && point.0 <= bbox.2
                && point.1 >= bbox.1
                && point.1 <= bbox.3
                && point_in_polygon(point, &water.points)
        })
    })
}
