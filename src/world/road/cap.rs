//! Round road end-caps. Closes the polyline ribbon's loose ends with a
//! triangle fan so the road surface doesn't show a hard vertical edge.

use crate::mesh::{Vertex, feature};

use super::ROAD_Y_OFFSET;

pub(super) const ROAD_CAP_EXTRA_Y_OFFSET: f32 = 0.008;
pub(super) const ROAD_CAP_SEGMENTS: usize = 12;
pub const ROAD_CAP_RADIUS_SCALE: f32 = 1.05;

pub fn append_road_cap(
    point: (f32, f32),
    elevation: f32,
    width: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_road_cap_with_radius_scale(
        point,
        elevation,
        width,
        ROAD_CAP_RADIUS_SCALE,
        color,
        verts,
        idxs,
    );
}

pub fn append_road_cap_with_radius_scale(
    point: (f32, f32),
    elevation: f32,
    width: f32,
    radius_scale: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_cap(
        point.0,
        elevation + ROAD_Y_OFFSET + ROAD_CAP_EXTRA_Y_OFFSET,
        point.1,
        width / 2.0 * radius_scale,
        color,
        verts,
        idxs,
    );
}

fn append_cap(
    x: f32,
    y: f32,
    z: f32,
    radius: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let normal = [0.0, 1.0, 0.0];
    let base = verts.len() as u32;
    verts.push(Vertex {
        position: [x, y, z],
        normal,
        color,
        uv: [0.0, 0.0],
        feature_type: feature::ROAD,
    });

    for i in 0..ROAD_CAP_SEGMENTS {
        let angle = i as f32 / ROAD_CAP_SEGMENTS as f32 * std::f32::consts::TAU;
        verts.push(Vertex {
            position: [x + angle.cos() * radius, y, z + angle.sin() * radius],
            normal,
            color,
            uv: [0.0, 0.0],
            feature_type: feature::ROAD,
        });
    }

    for i in 0..ROAD_CAP_SEGMENTS {
        let current = base + 1 + i as u32;
        let next = base + 1 + ((i + 1) % ROAD_CAP_SEGMENTS) as u32;
        idxs.push(base);
        idxs.push(next);
        idxs.push(current);
    }
}
