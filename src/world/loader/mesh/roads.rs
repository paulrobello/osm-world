//! Road mesh helpers. Computes per-road ramps/structures, the dead-end
//! road-cap placement (shared by the whole-world and tiled paths), and the
//! tile-scoped road emission.
//!
//! The road-cap counting + emission logic was previously duplicated between
//! `append_world_mesh` and `append_tile_roads_mesh`; both now route through
//! [`emit_road_caps`] for a single source of truth.

use std::collections::HashMap;

use crate::render::vertex::Vertex;
use crate::world::loader::source::{ResolvedFeature, WorldSource};

type RoadPointKey = (i32, i32);

struct RoadCap {
    point: (f32, f32),
    elevation: f32,
    width: f32,
    radius_scale: f32,
    color: [f32; 3],
}

fn road_key(point: (f32, f32)) -> RoadPointKey {
    (
        (point.0 * 10.0).round() as i32,
        (point.1 * 10.0).round() as i32,
    )
}

/// Count how many roads touch each quantised road point. Used by the
/// dead-end cap filter (a point with count 1 at the end of a non-closed road
/// is a dead-end that needs a rounded cap).
fn count_road_points(source: &WorldSource) -> HashMap<RoadPointKey, usize> {
    let mut counts: HashMap<RoadPointKey, usize> = HashMap::new();
    for r in &source.roads {
        let is_closed = r.points.len() >= 4 && r.points.first() == r.points.last();
        let count_len = if is_closed {
            r.points.len() - 1
        } else {
            r.points.len()
        };
        for &point in &r.points[..count_len] {
            *counts.entry(road_key(point)).or_default() += 1;
        }
    }
    counts
}

/// Emit rounded end-caps for every dead-end road point across the whole
/// world. Used by `append_world_mesh` after all roads have been emitted.
pub(super) fn emit_world_road_caps(
    source: &WorldSource,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let road_point_counts = count_road_points(source);
    let mut road_caps: HashMap<RoadPointKey, RoadCap> = HashMap::new();
    for (road_index, r) in source.roads.iter().enumerate() {
        let ramps = bridge_endpoint_ramps_for_road(source, road_index);
        // The world path re-emits every road here; callers consume the cap
        // elevations separately through `append_road_feature_mesh_with_ramps`.
        // For cap collection we only need the elevations + width + color.
        let (road_elevations, width, color) =
            append_road_feature_mesh_with_ramps(r, ramps, verts, idxs);
        collect_dead_end_caps(
            r,
            &road_elevations,
            width,
            color,
            &road_point_counts,
            &mut road_caps,
        );
    }
    for (_key, cap) in road_caps {
        crate::world::road::append_road_cap_with_radius_scale(
            cap.point,
            cap.elevation,
            cap.width,
            cap.radius_scale,
            cap.color,
            verts,
            idxs,
        );
    }
}

/// Emit roads and rounded end-caps for a tile-scoped subset of roads. Used by
/// `append_tile_features_mesh`. Filters out minor highways at Far LOD.
pub(crate) fn append_tile_roads_mesh(
    source: &WorldSource,
    road_refs: &[usize],
    lod: crate::stream::TileLod,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let road_point_counts = count_road_points(source);
    let selected_roads: Vec<(usize, &ResolvedFeature)> = road_refs
        .iter()
        .filter_map(|&feature_idx| {
            source
                .roads
                .get(feature_idx)
                .map(|road| (feature_idx, road))
        })
        .filter(|(_, road)| lod != crate::stream::TileLod::Far || !is_minor_highway(&road.tags))
        .collect();

    let mut road_caps: HashMap<RoadPointKey, RoadCap> = HashMap::new();
    for (road_index, r) in selected_roads {
        let ramps = bridge_endpoint_ramps_for_road(source, road_index);
        let (road_elevations, width, color) =
            append_road_feature_mesh_with_ramps(r, ramps, verts, idxs);
        collect_dead_end_caps(
            r,
            &road_elevations,
            width,
            color,
            &road_point_counts,
            &mut road_caps,
        );
    }
    for (_key, cap) in road_caps {
        crate::world::road::append_road_cap_with_radius_scale(
            cap.point,
            cap.elevation,
            cap.width,
            cap.radius_scale,
            cap.color,
            verts,
            idxs,
        );
    }
}

/// Shared dead-end cap collector. For each endpoint of a non-closed road,
/// if no other road touches that point, record a cap candidate. When
/// multiple roads hit the same point, the narrowest road's color wins and
/// the highest elevation wins (matches the pre-split behaviour).
fn collect_dead_end_caps(
    road: &ResolvedFeature,
    road_elevations: &[f32],
    width: f32,
    color: [f32; 3],
    road_point_counts: &HashMap<RoadPointKey, usize>,
    road_caps: &mut HashMap<RoadPointKey, RoadCap>,
) {
    let is_closed = road.points.len() >= 4 && road.points.first() == road.points.last();
    for (i, (&point, &elevation)) in road.points.iter().zip(road_elevations).enumerate() {
        let key = road_key(point);
        let count = road_point_counts.get(&key).copied().unwrap_or(0);
        let is_dead_end = !is_closed && (i == 0 || i + 1 == road.points.len()) && count == 1;
        if !is_dead_end {
            continue;
        }

        let radius_scale = crate::world::road::ROAD_CAP_RADIUS_SCALE;

        road_caps
            .entry(key)
            .and_modify(|cap| {
                cap.elevation = cap.elevation.max(elevation);
                if width < cap.width {
                    cap.width = width;
                    cap.color = color;
                }
            })
            .or_insert(RoadCap {
                point,
                elevation,
                width,
                radius_scale,
                color,
            });
    }
}

fn append_road_feature_mesh_with_ramps(
    road: &ResolvedFeature,
    endpoint_ramps: crate::world::road::BridgeEndpointRamps,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) -> (Vec<f32>, f32, [f32; 3]) {
    let width = crate::world::color::road_width(&road.tags);
    let color = crate::world::color::road_color(&road.tags);
    let profile = crate::world::road::road_profile(&road.tags);
    let render_path = crate::world::road::road_render_path_with_bridge_endpoint_ramps(
        &road.tags,
        &road.points,
        &road.elevations,
        endpoint_ramps,
    );

    let is_surface = profile.kind == crate::world::road::RoadProfileKind::Surface;
    let feature_type = if !is_surface {
        crate::render::vertex::feature::ROAD_LAYERED
    } else if crate::world::color::is_sidewalk_like_road(&road.tags) {
        crate::render::vertex::feature::ROAD_PATH
    } else {
        crate::render::vertex::feature::ROAD
    };
    let marking_feature_type = if is_surface {
        crate::render::vertex::feature::ROAD_MARKING
    } else {
        crate::render::vertex::feature::ROAD_MARKING_LAYERED
    };
    crate::world::road::generate_road_with_elevations_and_feature_type(
        &render_path.points,
        &render_path.road_elevations,
        width,
        color,
        feature_type,
        verts,
        idxs,
    );
    crate::world::road::append_road_centerline_dashes_with_feature_type(
        &render_path.points,
        &render_path.road_elevations,
        width,
        marking_feature_type,
        verts,
        idxs,
    );
    crate::world::road::append_road_structures(
        &road.tags,
        &render_path.points,
        &render_path.terrain_elevations,
        &render_path.road_elevations,
        width,
        verts,
        idxs,
    );

    let cap_elevations =
        crate::world::road::road_render_elevations(&road.tags, &road.points, &road.elevations);
    (cap_elevations, width, color)
}

fn bridge_endpoint_ramps_for_road(
    source: &WorldSource,
    road_index: usize,
) -> crate::world::road::BridgeEndpointRamps {
    let Some(road) = source.roads.get(road_index) else {
        return crate::world::road::BridgeEndpointRamps::default();
    };
    if crate::world::road::road_profile(&road.tags).kind
        != crate::world::road::RoadProfileKind::Bridge
    {
        return crate::world::road::BridgeEndpointRamps::default();
    }

    crate::world::road::BridgeEndpointRamps {
        start: road
            .points
            .first()
            .is_none_or(|&point| !has_connected_bridge_road_at(source, road_index, point)),
        end: road
            .points
            .last()
            .is_none_or(|&point| !has_connected_bridge_road_at(source, road_index, point)),
    }
}

fn has_connected_bridge_road_at(
    source: &WorldSource,
    road_index: usize,
    point: (f32, f32),
) -> bool {
    source.roads.iter().enumerate().any(|(other_index, other)| {
        other_index != road_index
            && crate::world::road::road_profile(&other.tags).kind
                == crate::world::road::RoadProfileKind::Bridge
            && other
                .points
                .iter()
                .any(|&other_point| super::same_road_point(other_point, point))
    })
}

fn is_minor_highway(tags: &std::collections::HashMap<String, String>) -> bool {
    matches!(
        tags.get("highway").map(String::as_str),
        Some("footway" | "path" | "cycleway" | "steps")
    )
}
