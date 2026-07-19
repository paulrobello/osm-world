//! Road centerline dash markings. Emits the yellow dashed lane divider
//! painted on top of the road ribbon surface.

use crate::mesh::{Vertex, feature};

use super::ROAD_Y_OFFSET;
use super::geometry::segment_frame;

const CENTERLINE_MIN_ROAD_WIDTH: f32 = 4.0;
const CENTERLINE_WIDTH: f32 = 0.22;
const CENTERLINE_DASH_LENGTH: f32 = 4.0;
const CENTERLINE_GAP_LENGTH: f32 = 6.0;
const CENTERLINE_Y_OFFSET: f32 = 0.008;
pub(super) const CENTERLINE_COLOR: [f32; 3] = [1.0, 0.82, 0.05];

pub fn append_road_centerline_dashes(
    points: &[(f32, f32)],
    road_elevations: &[f32],
    road_width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_road_centerline_dashes_with_feature_type(
        points,
        road_elevations,
        road_width,
        feature::ROAD_MARKING,
        verts,
        idxs,
    );
}

pub fn append_road_centerline_dashes_with_feature_type(
    points: &[(f32, f32)],
    road_elevations: &[f32],
    road_width: f32,
    feature_type: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if road_width < CENTERLINE_MIN_ROAD_WIDTH
        || points.len() != road_elevations.len()
        || points.len() < 2
    {
        return;
    }

    for i in 0..points.len() - 1 {
        append_centerline_dashes_for_segment(
            points[i],
            points[i + 1],
            road_elevations[i],
            road_elevations[i + 1],
            feature_type,
            verts,
            idxs,
        );
    }
}

fn append_centerline_dashes_for_segment(
    a: (f32, f32),
    b: (f32, f32),
    start_elevation: f32,
    end_elevation: f32,
    feature_type: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(frame) = segment_frame(a, b) else {
        return;
    };
    let dx = b.0 - a.0;
    let dz = b.1 - a.1;
    let segment_length = (dx * dx + dz * dz).sqrt();
    if segment_length <= 1e-6 {
        return;
    }

    let (px, pz) = frame.perpendicular;
    let half_width = CENTERLINE_WIDTH * 0.5;
    let pattern_length = CENTERLINE_DASH_LENGTH + CENTERLINE_GAP_LENGTH;
    let mut dash_start = 0.0;
    while dash_start < segment_length {
        let dash_end = (dash_start + CENTERLINE_DASH_LENGTH).min(segment_length);
        if dash_end > dash_start {
            let start_t = dash_start / segment_length;
            let end_t = dash_end / segment_length;
            let sx = a.0 + dx * start_t;
            let sz = a.1 + dz * start_t;
            let ex = a.0 + dx * end_t;
            let ez = a.1 + dz * end_t;
            let sy = start_elevation
                + (end_elevation - start_elevation) * start_t
                + ROAD_Y_OFFSET
                + CENTERLINE_Y_OFFSET;
            let ey = start_elevation
                + (end_elevation - start_elevation) * end_t
                + ROAD_Y_OFFSET
                + CENTERLINE_Y_OFFSET;
            let base = verts.len() as u32;
            for position in [
                [sx + px * half_width, sy, sz + pz * half_width],
                [ex + px * half_width, ey, ez + pz * half_width],
                [sx - px * half_width, sy, sz - pz * half_width],
                [ex - px * half_width, ey, ez - pz * half_width],
            ] {
                verts.push(Vertex {
                    position,
                    normal: [0.0, 1.0, 0.0],
                    color: CENTERLINE_COLOR,
                    uv: [0.0, 0.0],
                    feature_type,
                });
            }
            idxs.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
        }
        dash_start += pattern_length;
    }
}
