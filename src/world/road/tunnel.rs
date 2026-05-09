//! Tunnel structure mesh generation.
//!
//! Generates portal walls and lining segments for tunnel road segments.
//! All tunnel geometry uses `BUILDING` feature type so it casts shadows
//! and renders as solid geometry.

use crate::mesh::Vertex;

use super::{
    ROAD_Y_OFFSET, SegmentStripBox,
    segment_frame, append_segment_strip_box, append_box, bounds2d, same_point,
};

pub const TUNNEL_PORTAL_DEPTH: f32 = 1.0;
pub const TUNNEL_PORTAL_THICKNESS: f32 = 0.5;
pub const TUNNEL_CLEARANCE: f32 = 3.0;
pub const TUNNEL_LINING_HEIGHT_FRACTION: f32 = 0.35;
pub const TUNNEL_STRUCTURE_COLOR: [f32; 3] = [0.34, 0.32, 0.30];

pub fn append_tunnel_structure(
    points: &[(f32, f32)],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() != road_elevations.len() || points.len() < 2 {
        return;
    }

    let closed = points.len() >= 4 && same_point(points[0], points[points.len() - 1]);
    if closed {
        return;
    }

    append_tunnel_portal(points[0], points[1], road_elevations[0], width, verts, idxs);
    append_tunnel_portal(
        points[points.len() - 1],
        points[points.len() - 2],
        road_elevations[road_elevations.len() - 1],
        width,
        verts,
        idxs,
    );

    let lining_half_width = (width * 0.5 + 0.25).max(0.5);
    for i in 0..points.len() - 1 {
        let Some(frame) = segment_frame(points[i], points[i + 1]) else {
            continue;
        };
        let dx = points[i + 1].0 - points[i].0;
        let dz = points[i + 1].1 - points[i].1;
        let segment_length = (dx * dx + dz * dz).sqrt();
        let half_length = (segment_length * 0.25)
            .clamp(0.75, 4.0)
            .min((segment_length * 0.45).max(0.5));
        let mid = (
            (points[i].0 + points[i + 1].0) * 0.5,
            (points[i].1 + points[i + 1].1) * 0.5,
        );
        let (dir_x, dir_z) = frame.direction;
        let start = (mid.0 - dir_x * half_length, mid.1 - dir_z * half_length);
        let end = (mid.0 + dir_x * half_length, mid.1 + dir_z * half_length);
        let road_y = road_elevations[i].max(road_elevations[i + 1]) + ROAD_Y_OFFSET;
        let lining_min_y = road_y + TUNNEL_CLEARANCE * TUNNEL_LINING_HEIGHT_FRACTION;
        let lining_max_y = road_y + TUNNEL_CLEARANCE - 0.2;

        append_segment_strip_box(
            SegmentStripBox {
                a: start,
                b: end,
                lateral_offset: 0.0,
                half_width: lining_half_width,
                min_y: lining_min_y,
                max_y: lining_max_y,
                color: TUNNEL_STRUCTURE_COLOR,
            },
            verts,
            idxs,
        );
    }
}

fn append_tunnel_portal(
    point: (f32, f32),
    next: (f32, f32),
    elevation: f32,
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(frame) = segment_frame(point, next) else {
        return;
    };
    let (dx, dz) = frame.direction;
    let (px, pz) = frame.perpendicular;

    let road_y = elevation + ROAD_Y_OFFSET;
    let top_y = road_y + TUNNEL_CLEARANCE;
    let half_width = width * 0.5;
    let half_post = TUNNEL_PORTAL_THICKNESS * 0.5;
    let depth = TUNNEL_PORTAL_DEPTH.max(TUNNEL_PORTAL_THICKNESS);
    let front_dx = dx * depth;
    let front_dz = dz * depth;

    for sign in [1.0, -1.0] {
        let offset_x = px * (half_width + half_post) * sign;
        let offset_z = pz * (half_width + half_post) * sign;
        let start_x = point.0 + offset_x;
        let start_z = point.1 + offset_z;
        let end_x = start_x + front_dx;
        let end_z = start_z + front_dz;
        append_box(
            [
                start_x.min(end_x) - half_post,
                road_y,
                start_z.min(end_z) - half_post,
            ],
            [
                start_x.max(end_x) + half_post,
                top_y,
                end_z.max(end_z) + half_post,
            ],
            TUNNEL_STRUCTURE_COLOR,
            verts,
            idxs,
        );
    }

    let beam_corners = [
        (
            point.0 + px * (half_width + half_post),
            point.1 + pz * (half_width + half_post),
        ),
        (
            point.0 - px * (half_width + half_post),
            point.1 - pz * (half_width + half_post),
        ),
        (
            point.0 + px * (half_width + half_post) + front_dx,
            point.1 + pz * (half_width + half_post) + front_dz,
        ),
        (
            point.0 - px * (half_width + half_post) + front_dx,
            point.1 - pz * (half_width + half_post) + front_dz,
        ),
    ];
    let (beam_min_x, beam_max_x, beam_min_z, beam_max_z) = bounds2d(&beam_corners);
    append_box(
        [beam_min_x, top_y - TUNNEL_PORTAL_THICKNESS, beam_min_z],
        [beam_max_x, top_y, beam_max_z],
        TUNNEL_STRUCTURE_COLOR,
        verts,
        idxs,
    );
}
