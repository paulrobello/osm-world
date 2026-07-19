//! Landmark point-feature geometry. Dispatches per [`LandmarkKind`] to one
//! of seven specialised builders (tower / water-tower / chimney / monument /
//! peak / viewpoint / generic plinth).

use crate::mesh::Vertex;

use super::geometry::{BoxSpec, append_box, append_pyramid};
use super::style::LandmarkKind;

const LANDMARK_COLOR: [f32; 3] = [0.72, 0.64, 0.45];

pub(super) fn append_landmark(
    point: (f32, f32),
    elevation: f32,
    kind: LandmarkKind,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    match kind {
        LandmarkKind::Generic => append_generic_landmark(point, elevation, verts, idxs),
        LandmarkKind::Tower => append_tower_landmark(point, elevation, verts, idxs),
        LandmarkKind::WaterTower => append_water_tower_landmark(point, elevation, verts, idxs),
        LandmarkKind::Chimney => append_chimney_landmark(point, elevation, verts, idxs),
        LandmarkKind::Monument => append_monument_landmark(point, elevation, verts, idxs),
        LandmarkKind::Peak => append_peak_landmark(point, elevation, verts, idxs),
        LandmarkKind::Viewpoint => append_viewpoint_landmark(point, elevation, verts, idxs),
    }
}

fn append_generic_landmark(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_box(
        BoxSpec {
            point,
            base_y: elevation,
            half_extents: (0.72, 0.72),
            height: 4.2,
            color: LANDMARK_COLOR,
        },
        verts,
        idxs,
    );
    append_pyramid(
        point,
        elevation + 4.2,
        elevation + 5.1,
        0.62,
        LANDMARK_COLOR,
        verts,
        idxs,
    );
}

fn append_tower_landmark(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_box(
        BoxSpec {
            point,
            base_y: elevation,
            half_extents: (0.48, 0.48),
            height: 4.2,
            color: LANDMARK_COLOR,
        },
        verts,
        idxs,
    );
    append_box(
        BoxSpec {
            point,
            base_y: elevation + 4.2,
            half_extents: (0.32, 0.32),
            height: 2.0,
            color: LANDMARK_COLOR,
        },
        verts,
        idxs,
    );
    append_pyramid(
        point,
        elevation + 6.2,
        elevation + 8.0,
        0.38,
        LANDMARK_COLOR,
        verts,
        idxs,
    );
}

fn append_water_tower_landmark(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_box(
        BoxSpec {
            point,
            base_y: elevation,
            half_extents: (0.18, 0.18),
            height: 3.2,
            color: LANDMARK_COLOR,
        },
        verts,
        idxs,
    );
    append_box(
        BoxSpec {
            point,
            base_y: elevation + 3.15,
            half_extents: (0.95, 0.95),
            height: 1.15,
            color: LANDMARK_COLOR,
        },
        verts,
        idxs,
    );
    append_pyramid(
        point,
        elevation + 4.3,
        elevation + 5.05,
        0.82,
        LANDMARK_COLOR,
        verts,
        idxs,
    );
}

fn append_chimney_landmark(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_box(
        BoxSpec {
            point,
            base_y: elevation,
            half_extents: (0.28, 0.28),
            height: 6.0,
            color: LANDMARK_COLOR,
        },
        verts,
        idxs,
    );
    append_box(
        BoxSpec {
            point,
            base_y: elevation + 6.0,
            half_extents: (0.36, 0.36),
            height: 0.35,
            color: LANDMARK_COLOR,
        },
        verts,
        idxs,
    );
}

fn append_monument_landmark(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_box(
        BoxSpec {
            point,
            base_y: elevation,
            half_extents: (0.62, 0.62),
            height: 0.55,
            color: LANDMARK_COLOR,
        },
        verts,
        idxs,
    );
    append_pyramid(
        point,
        elevation + 0.55,
        elevation + 4.45,
        0.76,
        LANDMARK_COLOR,
        verts,
        idxs,
    );
}

fn append_peak_landmark(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_pyramid(
        point,
        elevation + 0.05,
        elevation + 2.05,
        0.98,
        LANDMARK_COLOR,
        verts,
        idxs,
    );
}

fn append_viewpoint_landmark(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_box(
        BoxSpec {
            point,
            base_y: elevation,
            half_extents: (0.22, 0.22),
            height: 2.5,
            color: LANDMARK_COLOR,
        },
        verts,
        idxs,
    );
    append_box(
        BoxSpec {
            point,
            base_y: elevation + 2.5,
            half_extents: (1.0, 1.0),
            height: 0.28,
            color: LANDMARK_COLOR,
        },
        verts,
        idxs,
    );
    append_pyramid(
        point,
        elevation + 2.9,
        elevation + 4.15,
        0.86,
        LANDMARK_COLOR,
        verts,
        idxs,
    );
}
