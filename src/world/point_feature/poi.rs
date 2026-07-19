//! Point-of-interest marker geometry. Emits a thin vertical post (shared with
//! the transit marker) topped with a category-coloured cap and pyramid roof.

use crate::mesh::Vertex;

use super::geometry::{BoxSpec, append_box, append_pyramid};
use super::style::PoiCategory;

const POI_FOOD_COLOR: [f32; 3] = [1.00, 0.30, 0.16];
const POI_SERVICE_COLOR: [f32; 3] = [0.12, 0.50, 1.00];
const POI_SHOP_COLOR: [f32; 3] = [1.00, 0.30, 0.92];
const POI_TOURISM_COLOR: [f32; 3] = [1.00, 0.78, 0.12];
const POI_LEISURE_COLOR: [f32; 3] = [0.20, 0.90, 0.24];
/// Shared by POI and transit markers — the thin post the cap sits on.
pub(super) const POI_POST_COLOR: [f32; 3] = [0.18, 0.18, 0.18];

pub(super) fn append_poi_marker(
    point: (f32, f32),
    elevation: f32,
    category: PoiCategory,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_box(
        BoxSpec {
            point,
            base_y: elevation,
            half_extents: (0.22, 0.22),
            height: 2.4,
            color: POI_POST_COLOR,
        },
        verts,
        idxs,
    );
    let color = poi_color(category);
    append_box(
        BoxSpec {
            point,
            base_y: elevation + 2.45,
            half_extents: (0.85, 0.85),
            height: 1.25,
            color,
        },
        verts,
        idxs,
    );
    append_pyramid(
        point,
        elevation + 3.75,
        elevation + 4.9,
        1.0,
        color,
        verts,
        idxs,
    );
}

fn poi_color(category: PoiCategory) -> [f32; 3] {
    match category {
        PoiCategory::Food => POI_FOOD_COLOR,
        PoiCategory::Service => POI_SERVICE_COLOR,
        PoiCategory::Shop => POI_SHOP_COLOR,
        PoiCategory::Tourism => POI_TOURISM_COLOR,
        PoiCategory::Leisure => POI_LEISURE_COLOR,
    }
}
