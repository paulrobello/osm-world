//! Per-point road elevation sampling and bridge approach-ramp smoothing.
//!
//! Produces the elevation stream consumed by the ribbon/marking/cap builders.
//! Bridge profiles get extra sample points inserted at the ramp transitions so
//! the deck lifts smoothly from the surface instead of snapping vertically.

use std::collections::HashMap;

use super::profile::{road_profile, surface_road_y_offset};
use crate::world::road::profile::{RoadProfile, RoadProfileKind};

pub const BRIDGE_APPROACH_RAMP_LENGTH: f32 = 25.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeEndpointRamps {
    pub start: bool,
    pub end: bool,
}

impl Default for BridgeEndpointRamps {
    fn default() -> Self {
        Self {
            start: true,
            end: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RoadRenderPath {
    pub points: Vec<(f32, f32)>,
    pub terrain_elevations: Vec<f32>,
    pub road_elevations: Vec<f32>,
}

pub fn road_render_elevations(
    tags: &HashMap<String, String>,
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
) -> Vec<f32> {
    if points.len() != terrain_elevations.len() {
        return Vec::new();
    }

    let profile = road_profile(tags);
    let ramp_factors = if profile.kind == RoadProfileKind::Bridge {
        bridge_ramp_factors(points, BRIDGE_APPROACH_RAMP_LENGTH)
    } else {
        vec![1.0; points.len()]
    };
    render_elevations_for_factors(tags, terrain_elevations, &ramp_factors)
}

pub fn road_render_path(
    tags: &HashMap<String, String>,
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
) -> RoadRenderPath {
    road_render_path_with_bridge_endpoint_ramps(
        tags,
        points,
        terrain_elevations,
        BridgeEndpointRamps::default(),
    )
}

pub fn road_render_path_with_bridge_endpoint_ramps(
    tags: &HashMap<String, String>,
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    endpoint_ramps: BridgeEndpointRamps,
) -> RoadRenderPath {
    if points.len() != terrain_elevations.len() {
        return RoadRenderPath {
            points: Vec::new(),
            terrain_elevations: Vec::new(),
            road_elevations: Vec::new(),
        };
    }

    if road_profile(tags).kind != RoadProfileKind::Bridge || points.len() < 2 {
        return RoadRenderPath {
            points: points.to_vec(),
            terrain_elevations: terrain_elevations.to_vec(),
            road_elevations: road_render_elevations(tags, points, terrain_elevations),
        };
    }

    let distances = cumulative_distances(points);
    let total_length = *distances.last().unwrap_or(&0.0);
    if total_length <= 1e-6 {
        return RoadRenderPath {
            points: points.to_vec(),
            terrain_elevations: terrain_elevations.to_vec(),
            road_elevations: road_render_elevations(tags, points, terrain_elevations),
        };
    }

    let effective_ramp = BRIDGE_APPROACH_RAMP_LENGTH.min(total_length * 0.5);
    let mut sample_distances = distances.clone();
    if endpoint_ramps.start {
        push_sample_distance(&mut sample_distances, effective_ramp, total_length);
    }
    if endpoint_ramps.end {
        push_sample_distance(
            &mut sample_distances,
            total_length - effective_ramp,
            total_length,
        );
    }
    sample_distances.sort_by(f32::total_cmp);
    sample_distances.dedup_by(|a, b| (*a - *b).abs() < 1e-4);

    let mut render_points = Vec::with_capacity(sample_distances.len());
    let mut render_terrain_elevations = Vec::with_capacity(sample_distances.len());
    let mut ramp_factors = Vec::with_capacity(sample_distances.len());
    for distance in sample_distances {
        let (point, terrain_elevation) =
            interpolate_path_sample(points, terrain_elevations, &distances, distance);
        render_points.push(point);
        render_terrain_elevations.push(terrain_elevation);
        ramp_factors.push(bridge_ramp_factor_with_endpoints(
            distance,
            total_length,
            effective_ramp,
            endpoint_ramps,
        ));
    }

    let road_elevations =
        render_elevations_for_factors(tags, &render_terrain_elevations, &ramp_factors);
    RoadRenderPath {
        points: render_points,
        terrain_elevations: render_terrain_elevations,
        road_elevations,
    }
}

fn render_elevations_for_factors(
    tags: &HashMap<String, String>,
    terrain_elevations: &[f32],
    ramp_factors: &[f32],
) -> Vec<f32> {
    let profile: RoadProfile = road_profile(tags);
    if profile.kind != RoadProfileKind::Bridge {
        return terrain_elevations
            .iter()
            .map(|elevation| elevation + profile.layer_offset)
            .collect();
    }

    let surface_offset = surface_road_y_offset(tags);
    let bridge_lift = profile.layer_offset - surface_offset;
    terrain_elevations
        .iter()
        .zip(ramp_factors)
        .map(|(&elevation, &ramp_factor)| elevation + surface_offset + bridge_lift * ramp_factor)
        .collect()
}

fn bridge_ramp_factors(points: &[(f32, f32)], ramp_length: f32) -> Vec<f32> {
    if points.len() < 2 || ramp_length <= 0.0 {
        return vec![1.0; points.len()];
    }

    let distances = cumulative_distances(points);
    let total_length = *distances.last().unwrap_or(&0.0);
    if total_length <= 1e-6 {
        return vec![1.0; points.len()];
    }
    let effective_ramp = ramp_length.min(total_length * 0.5);
    distances
        .into_iter()
        .map(|distance| bridge_ramp_factor(distance, total_length, effective_ramp))
        .collect()
}

fn bridge_ramp_factor(distance: f32, total_length: f32, effective_ramp: f32) -> f32 {
    bridge_ramp_factor_with_endpoints(
        distance,
        total_length,
        effective_ramp,
        BridgeEndpointRamps::default(),
    )
}

fn bridge_ramp_factor_with_endpoints(
    distance: f32,
    total_length: f32,
    effective_ramp: f32,
    endpoint_ramps: BridgeEndpointRamps,
) -> f32 {
    if effective_ramp <= 1e-6 {
        return 1.0;
    }
    let from_start = if endpoint_ramps.start {
        (distance / effective_ramp).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let from_end = if endpoint_ramps.end {
        ((total_length - distance) / effective_ramp).clamp(0.0, 1.0)
    } else {
        1.0
    };
    from_start.min(from_end)
}

fn cumulative_distances(points: &[(f32, f32)]) -> Vec<f32> {
    let mut distances = Vec::with_capacity(points.len());
    if points.is_empty() {
        return distances;
    }
    distances.push(0.0f32);
    for i in 1..points.len() {
        let dx = points[i].0 - points[i - 1].0;
        let dz = points[i].1 - points[i - 1].1;
        let segment_length = (dx * dx + dz * dz).sqrt();
        distances.push(distances[i - 1] + segment_length);
    }
    distances
}

fn push_sample_distance(sample_distances: &mut Vec<f32>, distance: f32, total_length: f32) {
    if distance > 1e-4 && distance < total_length - 1e-4 {
        sample_distances.push(distance);
    }
}

fn interpolate_path_sample(
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    distances: &[f32],
    sample_distance: f32,
) -> ((f32, f32), f32) {
    debug_assert!(
        !points.is_empty()
            && points.len() == terrain_elevations.len()
            && points.len() == distances.len(),
        "interpolate_path_sample requires non-empty parallel slices of equal length"
    );
    if sample_distance <= 0.0 {
        return (points[0], terrain_elevations[0]);
    }
    for i in 1..distances.len() {
        if sample_distance <= distances[i] {
            let segment_length = distances[i] - distances[i - 1];
            if segment_length <= 1e-6 {
                return (points[i], terrain_elevations[i]);
            }
            let t = ((sample_distance - distances[i - 1]) / segment_length).clamp(0.0, 1.0);
            let x = points[i - 1].0 + (points[i].0 - points[i - 1].0) * t;
            let z = points[i - 1].1 + (points[i].1 - points[i - 1].1) * t;
            let elevation =
                terrain_elevations[i - 1] + (terrain_elevations[i] - terrain_elevations[i - 1]) * t;
            return ((x, z), elevation);
        }
    }
    (*points.last().unwrap(), *terrain_elevations.last().unwrap())
}
