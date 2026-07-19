//! Nature point-feature geometry (rocks, springs). Emits a small pyramid
//! marker tinted with the nature-marker colour.

use crate::mesh::Vertex;

use super::geometry::append_pyramid;

const NATURE_MARKER_COLOR: [f32; 3] = [0.24, 0.42, 0.58];

pub(super) fn append_nature_marker(
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_pyramid(
        point,
        elevation + 0.05,
        elevation + 1.35,
        0.85,
        NATURE_MARKER_COLOR,
        verts,
        idxs,
    );
}
