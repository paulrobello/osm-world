//! Road ribbon strip mesh generator.
//!
//! Sub-modules:
//! - `profile` -- vertical profile classification (surface / bridge / tunnel)
//! - `render_path` -- per-point elevation sampling + bridge approach ramps
//! - `ribbon` -- flat road surface mesh
//! - `centerline` -- dashed lane markings
//! - `cap` -- round end-caps for polyline endpoints
//! - `geometry` -- shared primitives used by every road builder + bridges/tunnels
//! - `bridge` -- bridge beams, rails, supports, abutments
//! - `tunnel` -- tunnel portals and lining
//!
//! Split (ARC-012) from a single 1626-line `mod.rs` along the natural seams
//! above. `mod.rs` remains the re-export hub: `crate::world::road::Foo` and
//! `super::Foo` references from sibling modules continue to resolve unchanged.

mod cap;
mod centerline;
mod geometry;
mod profile;
mod render_path;
mod ribbon;

pub mod bridge;
pub mod tunnel;

use std::collections::HashMap;

use crate::mesh::Vertex;
#[cfg(test)]
use crate::mesh::feature;

// Keep road/path overlays at curb-height scale; the city shader adds a tiny
// feature-specific depth bias so these close layers do not z-fight.
pub const ROAD_Y_OFFSET: f32 = 0.04;

// --- Public API re-exports (preserved verbatim from the pre-split module) ---

pub use bridge::BRIDGE_STRUCTURE_COLOR;
pub use cap::{ROAD_CAP_RADIUS_SCALE, append_road_cap, append_road_cap_with_radius_scale};
pub use centerline::{
    append_road_centerline_dashes, append_road_centerline_dashes_with_feature_type,
};
pub use profile::{RoadProfile, RoadProfileKind, road_layer_y_offset, road_profile};
pub use render_path::{
    BridgeEndpointRamps, RoadRenderPath, road_render_elevations, road_render_path,
    road_render_path_with_bridge_endpoint_ramps,
};
pub use ribbon::{
    generate_road, generate_road_with_elevations, generate_road_with_elevations_and_feature_type,
};
pub use tunnel::TUNNEL_STRUCTURE_COLOR;

// --- Private re-exports used by sibling submodules (bridge, tunnel) ---

use geometry::{
    SegmentStripBox, append_box, append_segment_strip_box, append_sloped_segment_strip_box,
    bounds2d, same_point, segment_frame,
};

// --- Private re-exports used only by the test module ---

#[cfg(test)]
use cap::{ROAD_CAP_EXTRA_Y_OFFSET, ROAD_CAP_SEGMENTS};
#[cfg(test)]
use centerline::CENTERLINE_COLOR;
#[cfg(test)]
use profile::surface_road_y_offset;

/// Append road structure geometry (bridge/tunnel) appropriate for `tags`.
///
/// Surface roads emit no structure geometry; bridges and tunnels dispatch to
/// the dedicated submodules. Kept in the hub since it only routes between
/// siblings — no domain logic of its own.
pub fn append_road_structures(
    tags: &HashMap<String, String>,
    points: &[(f32, f32)],
    terrain_elevations: &[f32],
    road_elevations: &[f32],
    width: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    match road_profile(tags).kind {
        RoadProfileKind::Bridge => bridge::append_bridge_structure(
            points,
            terrain_elevations,
            road_elevations,
            width,
            verts,
            idxs,
        ),
        RoadProfileKind::Tunnel => {
            tunnel::append_tunnel_structure(points, road_elevations, width, verts, idxs)
        }
        RoadProfileKind::Surface => {}
    }
}

#[cfg(test)]
mod tests;
