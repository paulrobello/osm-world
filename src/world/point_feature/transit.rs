//! Transit stop marker geometry. Emits a thin post (shared with POI markers)
//! topped with a transit-kind-coloured cap and pyramid roof. Colour and kind
//! are resolved through [`crate::world::transit`].

use std::collections::HashMap;

use crate::mesh::Vertex;

use super::geometry::{BoxSpec, append_box, append_pyramid};
use super::poi::POI_POST_COLOR;

pub(super) fn append_transit_marker(
    tags: &HashMap<String, String>,
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let kind = crate::world::transit::transit_kind(tags)
        .expect("Transit point styles carry a transit kind");
    let color = crate::world::transit::transit_color(kind);
    append_box(
        BoxSpec {
            point,
            base_y: elevation,
            half_extents: (0.18, 0.18),
            height: 3.2,
            color: POI_POST_COLOR,
        },
        verts,
        idxs,
    );
    append_box(
        BoxSpec {
            point,
            base_y: elevation + 3.25,
            half_extents: (0.95, 0.35),
            height: 0.95,
            color,
        },
        verts,
        idxs,
    );
    append_pyramid(
        point,
        elevation + 4.25,
        elevation + 5.1,
        0.8,
        color,
        verts,
        idxs,
    );
}
