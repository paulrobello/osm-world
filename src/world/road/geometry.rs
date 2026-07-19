//! Shared road geometry primitives used by the ribbon, marking, cap, bridge,
//! and tunnel builders. Lives at the `road` module level so every sibling
//! builder can `use super::geometry::Foo` (re-exported through `mod.rs`).
//!
//! Box emission delegates to [`crate::mesh::append_box`] (ARC-016) — there is
//! no duplicate box-building algorithm here.

use crate::mesh::{Vertex, feature};

/// Thin delegate over [`crate::mesh::append_box`] (ARC-016). Road structure
/// geometry (bridge piers, tunnel portals, abutments) is tagged
/// `feature::BUILDING` so it routes through the shadow-casting solids layer.
pub(super) fn append_box(
    min: [f32; 3],
    max: [f32; 3],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    crate::mesh::append_box(
        crate::mesh::BoxSpec {
            min,
            max,
            color,
            feature_type: feature::BUILDING,
        },
        verts,
        idxs,
    );
}

pub(super) fn bounds2d(points: &[(f32, f32)]) -> (f32, f32, f32, f32) {
    let mut min_x = points[0].0;
    let mut max_x = points[0].0;
    let mut min_z = points[0].1;
    let mut max_z = points[0].1;
    for &(x, z) in &points[1..] {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    (min_x, max_x, min_z, max_z)
}

pub(super) type Point2 = (f32, f32);

pub(super) struct SegmentFrame {
    pub direction: Point2,
    pub perpendicular: Point2,
}

pub(super) struct SegmentStripBox {
    pub a: Point2,
    pub b: Point2,
    pub lateral_offset: f32,
    pub half_width: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub color: [f32; 3],
}

pub(super) fn segment_frame(a: Point2, b: Point2) -> Option<SegmentFrame> {
    let dx = b.0 - a.0;
    let dz = b.1 - a.1;
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1e-6 {
        None
    } else {
        Some(SegmentFrame {
            direction: (dx / len, dz / len),
            perpendicular: (-dz / len, dx / len),
        })
    }
}

pub(super) fn append_segment_strip_box(
    strip: SegmentStripBox,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let min_y = strip.min_y;
    let max_y = strip.max_y;
    append_sloped_segment_strip_box(strip, min_y, max_y, min_y, max_y, verts, idxs);
}

pub(super) fn push_prism_face(
    face: [[f32; 3]; 4],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let ux = face[1][0] - face[0][0];
    let uy = face[1][1] - face[0][1];
    let uz = face[1][2] - face[0][2];
    let vx = face[2][0] - face[0][0];
    let vy = face[2][1] - face[0][1];
    let vz = face[2][2] - face[0][2];
    let nx = uy * vz - uz * vy;
    let ny = uz * vx - ux * vz;
    let nz = ux * vy - uy * vx;
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    let normal = if len < 1e-6 {
        [0.0, 1.0, 0.0]
    } else {
        [nx / len, ny / len, nz / len]
    };

    let base = verts.len() as u32;
    for position in face {
        verts.push(Vertex {
            position,
            normal,
            color,
            uv: [0.0, 0.0],
            feature_type: feature::BUILDING,
        });
    }
    idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

pub(super) fn append_sloped_segment_strip_box(
    strip: SegmentStripBox,
    start_min_y: f32,
    start_max_y: f32,
    end_min_y: f32,
    end_max_y: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(frame) = segment_frame(strip.a, strip.b) else {
        return;
    };
    let (px, pz) = frame.perpendicular;
    let a_left = (
        strip.a.0 + px * (strip.lateral_offset - strip.half_width),
        strip.a.1 + pz * (strip.lateral_offset - strip.half_width),
    );
    let a_right = (
        strip.a.0 + px * (strip.lateral_offset + strip.half_width),
        strip.a.1 + pz * (strip.lateral_offset + strip.half_width),
    );
    let b_left = (
        strip.b.0 + px * (strip.lateral_offset - strip.half_width),
        strip.b.1 + pz * (strip.lateral_offset - strip.half_width),
    );
    let b_right = (
        strip.b.0 + px * (strip.lateral_offset + strip.half_width),
        strip.b.1 + pz * (strip.lateral_offset + strip.half_width),
    );

    let abl = [a_left.0, start_min_y, a_left.1];
    let atl = [a_left.0, start_max_y, a_left.1];
    let abr = [a_right.0, start_min_y, a_right.1];
    let atr = [a_right.0, start_max_y, a_right.1];
    let bbl = [b_left.0, end_min_y, b_left.1];
    let btl = [b_left.0, end_max_y, b_left.1];
    let bbr = [b_right.0, end_min_y, b_right.1];
    let btr = [b_right.0, end_max_y, b_right.1];

    push_prism_face([abl, bbl, btl, atl], strip.color, verts, idxs);
    push_prism_face([abr, atr, btr, bbr], strip.color, verts, idxs);
    push_prism_face([atl, btl, btr, atr], strip.color, verts, idxs);
    push_prism_face([abl, abr, bbr, bbl], strip.color, verts, idxs);
    push_prism_face([abl, atl, atr, abr], strip.color, verts, idxs);
    push_prism_face([bbl, bbr, btr, btl], strip.color, verts, idxs);
}

pub(super) fn same_point(a: (f32, f32), b: (f32, f32)) -> bool {
    let dx = a.0 - b.0;
    let dz = a.1 - b.1;
    dx * dx + dz * dz < 1e-8
}
