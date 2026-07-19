//! Point-feature mesh generator for OSM trees, landmarks, nature markers,
//! POIs, and transit stops.
//!
//! Sub-modules (split, ARC-012, from a single 1333-line file):
//! - `style` -- tag classification (PointFeatureStyle / PointFeatureKind / ...)
//! - `tree` -- hex-prism trunk + octahedron canopy
//! - `landmark` -- tower / water-tower / chimney / monument / peak / viewpoint
//! - `nature` -- rock / spring pyramid marker
//! - `poi` -- post + category-coloured cap + roof
//! - `transit` -- post + transit-kind cap + roof
//! - `geometry` -- shared primitives (BoxSpec delegate, append_pyramid,
//!   append_quad, append_tri, triangle_normal)
//!
//! `mod.rs` remains the re-export hub: `crate::world::point_feature::Foo`
//! continues to resolve identically for every external caller (loader,
//! ui/poi_labels, ui/inspect, ui/search).

mod geometry;
mod landmark;
mod nature;
mod poi;
mod style;
mod transit;
mod tree;

use std::collections::HashMap;

use crate::mesh::Vertex;
use crate::visual_detail::{LandmarkDetail, VisualDetailSettings};

#[cfg(test)]
use geometry::triangle_normal;

// --- Public API re-exports (preserved verbatim from the pre-split module) ---

pub use style::{
    LandmarkKind, PoiCategory, PointFeatureKind, PointFeatureStyle, point_feature_label,
    point_feature_style,
};

pub fn generate_point_feature(
    tags: &HashMap<String, String>,
    point: (f32, f32),
    elevation: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    generate_point_feature_with_visual_detail(
        tags,
        point,
        elevation,
        &VisualDetailSettings::default(),
        verts,
        idxs,
    );
}

pub fn generate_point_feature_with_visual_detail(
    tags: &HashMap<String, String>,
    point: (f32, f32),
    elevation: f32,
    visual_detail: &VisualDetailSettings,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let Some(style) = point_feature_style(tags) else {
        return;
    };
    let first_vertex = verts.len();
    match style.kind {
        PointFeatureKind::Tree => {
            tree::append_tree_with_visual_detail(point, elevation, visual_detail, verts, idxs)
        }
        PointFeatureKind::Landmark => match visual_detail.landmark_detail {
            LandmarkDetail::Off => return,
            LandmarkDetail::Simple => {
                landmark::append_landmark(point, elevation, LandmarkKind::Generic, verts, idxs)
            }
            LandmarkDetail::Showcase => landmark::append_landmark(
                point,
                elevation,
                style.landmark_kind.expect("Landmark styles carry a kind"),
                verts,
                idxs,
            ),
        },
        PointFeatureKind::Nature => nature::append_nature_marker(point, elevation, verts, idxs),
        PointFeatureKind::Poi => poi::append_poi_marker(
            point,
            elevation,
            style.poi_category.expect("POI styles carry a category"),
            verts,
            idxs,
        ),
        PointFeatureKind::Transit => {
            transit::append_transit_marker(tags, point, elevation, verts, idxs)
        }
    }
    let marker_uv_kind = match style.kind {
        PointFeatureKind::Tree => 1.0,
        PointFeatureKind::Landmark => 2.0,
        PointFeatureKind::Nature | PointFeatureKind::Poi => 0.0,
        PointFeatureKind::Transit => 3.0,
    };
    for vertex in &mut verts[first_vertex..] {
        vertex.uv[0] = marker_uv_kind;
    }
}

#[cfg(test)]
mod tests;
