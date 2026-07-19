//! Shared point-feature geometry primitives used by every per-kind builder
//! (tree, landmark, nature, poi, transit). All emission tags vertices with
//! `feature::POINT_FEATURE` so the render pipeline routes them through the
//! point-feature layer.
//!
//! Box emission delegates to [`crate::mesh::append_box`] (ARC-016) via the
//! local `BoxSpec` parameter bundle — there is no duplicate box-building
//! algorithm here.

use crate::mesh::{Vertex, feature};

/// Parameter bundle for the centered-box call sites in the per-kind
/// builders. Delegates to the shared [`crate::mesh::append_box`] — no local
/// geometry algorithm.
pub(super) struct BoxSpec {
    pub point: (f32, f32),
    pub base_y: f32,
    pub half_extents: (f32, f32),
    pub height: f32,
    pub color: [f32; 3],
}

pub(super) fn append_box(spec: BoxSpec, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    crate::mesh::append_box(
        crate::mesh::BoxSpec::centered(
            spec.point,
            spec.base_y,
            spec.half_extents,
            spec.height,
            spec.color,
            feature::POINT_FEATURE,
        ),
        verts,
        idxs,
    );
}

pub(super) fn append_pyramid(
    point: (f32, f32),
    base_y: f32,
    apex_y: f32,
    half_size: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let (x, z) = point;
    let p0 = [x - half_size, base_y, z - half_size];
    let p1 = [x + half_size, base_y, z - half_size];
    let p2 = [x + half_size, base_y, z + half_size];
    let p3 = [x - half_size, base_y, z + half_size];
    let apex = [x, apex_y, z];

    append_tri(p1, p0, apex, color, verts, idxs);
    append_tri(p2, p1, apex, color, verts, idxs);
    append_tri(p3, p2, apex, color, verts, idxs);
    append_tri(p0, p3, apex, color, verts, idxs);
}

pub(super) struct QuadFace {
    pub positions: [[f32; 3]; 4],
    pub normal: [f32; 3],
}

pub(super) fn append_quad(
    face: QuadFace,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let base = verts.len() as u32;
    for position in face.positions {
        verts.push(vertex(position, face.normal, color));
    }

    let geometric_normal = triangle_normal(face.positions[0], face.positions[1], face.positions[2]);
    if glam::Vec3::from_array(geometric_normal).dot(glam::Vec3::from_array(face.normal)) >= 0.0 {
        idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        idxs.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}

pub(super) fn append_outward_tri(
    center: glam::Vec3,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let face_center =
        (glam::Vec3::from_array(p0) + glam::Vec3::from_array(p1) + glam::Vec3::from_array(p2))
            / 3.0;
    let normal = glam::Vec3::from_array(triangle_normal(p0, p1, p2));
    if normal.dot(face_center - center) >= 0.0 {
        append_tri(p0, p1, p2, color, verts, idxs);
    } else {
        append_tri(p0, p2, p1, color, verts, idxs);
    }
}

pub(super) fn append_tri(
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let normal = triangle_normal(p0, p1, p2);
    let base = verts.len() as u32;
    verts.push(vertex(p0, normal, color));
    verts.push(vertex(p1, normal, color));
    verts.push(vertex(p2, normal, color));
    idxs.extend_from_slice(&[base, base + 1, base + 2]);
}

/// Geometric normal of a triangle (counter-clockwise wound outward).
/// Exposed for tests that verify generated triangles face outward from the
/// marker centre.
pub(super) fn triangle_normal(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3]) -> [f32; 3] {
    let a = glam::Vec3::from_array(p1) - glam::Vec3::from_array(p0);
    let b = glam::Vec3::from_array(p2) - glam::Vec3::from_array(p0);
    a.cross(b).normalize_or_zero().to_array()
}

fn vertex(position: [f32; 3], normal: [f32; 3], color: [f32; 3]) -> Vertex {
    Vertex {
        position,
        normal,
        color,
        feature_type: feature::POINT_FEATURE,
        uv: [0.0, 0.0],
    }
}
