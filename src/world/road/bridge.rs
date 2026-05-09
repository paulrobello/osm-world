//! Bridge structure mesh generation.
//!
//! Generates beams, rails, support columns, and abutment walls for bridge
//! road segments. All bridge geometry uses `BUILDING` feature type so it
//! casts shadows and renders as solid geometry.

use crate::mesh::Vertex;

use super::{
    ROAD_Y_OFFSET, SegmentStripBox,
    segment_frame, append_segment_strip_box, append_box,
};

pub const BRIDGE_BEAM_THICKNESS: f32 = 0.6;
pub const BRIDGE_BEAM_TOP_CLEARANCE: f32 = 0.85;
pub const BRIDGE_BEAM_WIDTH: f32 = 0.45;
pub const BRIDGE_RAIL_BASE_CLEARANCE: f32 = 0.25;
pub const BRIDGE_RAIL_HEIGHT: f32 = 0.9;
pub const BRIDGE_RAIL_WIDTH: f32 = 0.25;
pub const BRIDGE_SUPPORT_WIDTH: f32 = 0.8;
pub const BRIDGE_ABUTMENT_THICKNESS: f32 = 0.7;
pub const BRIDGE_ABUTMENT_SIDE_OVERHANG: f32 = 0.7;
pub const BRIDGE_STRUCTURE_COLOR: [f32; 3] = [0.50, 0.52, 0.54];

struct SlopedBridgeRailSegment {
    a: (f32, f32),
    b: (f32, f32),
    rail_offset: f32,
    rail_half_width: f32,
    start_road_y: f32,
    end_road_y: f32,
}

pub fn append_bridge_structure(
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() != terrain_elevations.len()
        || points.len() != road_elevations.len()
        || points.len() < 2
    {
        return;
    }

    append_bridge_abutments(
        points,
        terrain_elevations,
        road_elevations,
        width,
        verts,
        idxs,
    );

    let half_width = (width * 0.5).max(0.0);
    let rail_half_width = (BRIDGE_RAIL_WIDTH * 0.5).max(0.05);
    let rail_offset = (half_width - rail_half_width).max(rail_half_width);
    for i in 0..points.len() - 1 {
        if segment_frame(points[i], points[i + 1]).is_none() {
            continue;
        }
        let start_road_y = road_elevations[i] + ROAD_Y_OFFSET;
        let end_road_y = road_elevations[i + 1] + ROAD_Y_OFFSET;
        if (start_road_y - end_road_y).abs() > 0.1 {
            append_sloped_bridge_rails(
                SlopedBridgeRailSegment {
                    a: points[i],
                    b: points[i + 1],
                    rail_offset,
                    rail_half_width,
                    start_road_y,
                    end_road_y,
                },
                verts,
                idxs,
            );
            continue;
        }
        let road_y = start_road_y.max(end_road_y);
        let terrain_y = terrain_elevations[i].min(terrain_elevations[i + 1]);

        for lateral_offset in [rail_offset, -rail_offset] {
            append_segment_strip_box(
                SegmentStripBox {
                    a: points[i],
                    b: points[i + 1],
                    lateral_offset,
                    half_width: BRIDGE_BEAM_WIDTH * 0.5,
                    min_y: road_y - BRIDGE_BEAM_TOP_CLEARANCE - BRIDGE_BEAM_THICKNESS,
                    max_y: road_y - BRIDGE_BEAM_TOP_CLEARANCE,
                    color: BRIDGE_STRUCTURE_COLOR,
                },
                verts,
                idxs,
            );
        }
        append_segment_strip_box(
            SegmentStripBox {
                a: points[i],
                b: points[i + 1],
                lateral_offset: rail_offset,
                half_width: rail_half_width,
                min_y: road_y + BRIDGE_RAIL_BASE_CLEARANCE,
                max_y: road_y + BRIDGE_RAIL_BASE_CLEARANCE + BRIDGE_RAIL_HEIGHT,
                color: BRIDGE_STRUCTURE_COLOR,
            },
            verts,
            idxs,
        );
        append_segment_strip_box(
            SegmentStripBox {
                a: points[i],
                b: points[i + 1],
                lateral_offset: -rail_offset,
                half_width: rail_half_width,
                min_y: road_y + BRIDGE_RAIL_BASE_CLEARANCE,
                max_y: road_y + BRIDGE_RAIL_BASE_CLEARANCE + BRIDGE_RAIL_HEIGHT,
                color: BRIDGE_STRUCTURE_COLOR,
            },
            verts,
            idxs,
        );

        if road_y - terrain_y > 2.0 {
            let half_support = BRIDGE_SUPPORT_WIDTH * 0.5;
            let cx = (points[i].0 + points[i + 1].0) * 0.5;
            let cz = (points[i].1 + points[i + 1].1) * 0.5;
            append_box(
                [cx - half_support, terrain_y, cz - half_support],
                [
                    cx + half_support,
                    road_y - BRIDGE_BEAM_TOP_CLEARANCE - BRIDGE_BEAM_THICKNESS,
                    cz + half_support,
                ],
                BRIDGE_STRUCTURE_COLOR,
                verts,
                idxs,
            );
        }
    }
}

fn append_bridge_abutments(
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() < 3 {
        return;
    }

    let first_high = (1..points.len() - 1)
        .find(|&i| bridge_clearance_at(i, terrain_elevations, road_elevations) > 2.0);
    let last_high = (1..points.len() - 1)
        .rev()
        .find(|&i| bridge_clearance_at(i, terrain_elevations, road_elevations) > 2.0);

    if let Some(i) = first_high {
        append_bridge_abutment_at(
            points[i],
            points[i + 1],
            terrain_elevations[i],
            road_elevations[i],
            width,
            verts,
            idxs,
        );
    }
    if let Some(i) = last_high.filter(|&i| Some(i) != first_high) {
        append_bridge_abutment_at(
            points[i],
            points[i - 1],
            terrain_elevations[i],
            road_elevations[i],
            width,
            verts,
            idxs,
        );
    }
}

fn bridge_clearance_at(index: usize, terrain_elevations: &[f32], road_elevations: &[f32]) -> f32 {
    road_elevations[index] + ROAD_Y_OFFSET - terrain_elevations[index]
}

fn append_bridge_abutment_at(
    point: (f32, f32),
    along: (f32, f32),
    terrain_y: f32,
    road_elevation: f32,
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(frame) = segment_frame(point, along) else {
        return;
    };
    let road_y = road_elevation + ROAD_Y_OFFSET;
    let top_y = road_y - BRIDGE_BEAM_TOP_CLEARANCE - BRIDGE_BEAM_THICKNESS;
    if top_y - terrain_y <= 0.5 {
        return;
    }

    let half_span = width * 0.5 + BRIDGE_ABUTMENT_SIDE_OVERHANG;
    let (px, pz) = frame.perpendicular;
    let a = (point.0 - px * half_span, point.1 - pz * half_span);
    let b = (point.0 + px * half_span, point.1 + pz * half_span);
    append_segment_strip_box(
        SegmentStripBox {
            a,
            b,
            lateral_offset: 0.0,
            half_width: BRIDGE_ABUTMENT_THICKNESS * 0.5,
            min_y: terrain_y,
            max_y: top_y,
            color: BRIDGE_STRUCTURE_COLOR,
        },
        verts,
        idxs,
    );
}

fn append_sloped_bridge_rails(
    rail_segment: SlopedBridgeRailSegment,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    for lateral_offset in [rail_segment.rail_offset, -rail_segment.rail_offset] {
        append_sloped_segment_strip_box(
            SegmentStripBox {
                a: rail_segment.a,
                b: rail_segment.b,
                lateral_offset,
                half_width: rail_segment.rail_half_width,
                min_y: 0.0,
                max_y: 0.0,
                color: BRIDGE_STRUCTURE_COLOR,
            },
            rail_segment.start_road_y + BRIDGE_RAIL_BASE_CLEARANCE,
            rail_segment.start_road_y + BRIDGE_RAIL_BASE_CLEARANCE + BRIDGE_RAIL_HEIGHT,
            rail_segment.end_road_y + BRIDGE_RAIL_BASE_CLEARANCE,
            rail_segment.end_road_y + BRIDGE_RAIL_BASE_CLEARANCE + BRIDGE_RAIL_HEIGHT,
            verts,
            idxs,
        );
    }
}

fn append_sloped_segment_strip_box(
    strip: SegmentStripBox,
    start_min_y: f32,
    start_max_y: f32,
    end_min_y: f32,
    end_max_y: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    super::append_sloped_segment_strip_box(
        strip, start_min_y, start_max_y, end_min_y, end_max_y, verts, idxs,
    );
}
