//! Road vertical-profile classification (surface / bridge / tunnel) and the
//! per-layer Y offsets derived from OSM `layer`/`bridge`/`tunnel` tags.

use std::collections::HashMap;

const ROAD_BRIDGE_LAYER_Y_OFFSET: f32 = 5.0;
const ROAD_TUNNEL_LAYER_Y_OFFSET: f32 = -5.0;

/// Additional per-feature Y offset applied before road ribbon generation.
///
/// The ribbon generator already adds [`super::ROAD_Y_OFFSET`] above sampled
/// terrain. This offset keeps road/path overlays just above landuse and water
/// overlays without visibly floating at eye level; layered crossings still
/// separate by several metres.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoadProfileKind {
    Surface,
    Bridge,
    Tunnel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadProfile {
    pub kind: RoadProfileKind,
    pub layer_offset: f32,
}

pub fn road_profile(tags: &HashMap<String, String>) -> RoadProfile {
    let surface_offset = surface_road_y_offset(tags);

    let osm_layer = tags
        .get("layer")
        .and_then(|layer| layer.parse::<f32>().ok())
        .unwrap_or(0.0);
    let is_bridge = matches!(
        tags.get("bridge").map(String::as_str),
        Some("yes" | "viaduct")
    );
    let is_tunnel = tags.get("tunnel").is_some_and(|value| value != "no");

    if is_tunnel {
        let layer_depth = if osm_layer < 0.0 {
            osm_layer.abs()
        } else {
            1.0
        };
        return RoadProfile {
            kind: RoadProfileKind::Tunnel,
            layer_offset: ROAD_TUNNEL_LAYER_Y_OFFSET * layer_depth,
        };
    }

    // Explicit bridge tags win over layer-only lowering; explicit tunnel tags
    // are handled above and still take precedence when present.
    if is_bridge || osm_layer > 0.0 {
        return RoadProfile {
            kind: RoadProfileKind::Bridge,
            layer_offset: surface_offset + (osm_layer.max(1.0) * ROAD_BRIDGE_LAYER_Y_OFFSET),
        };
    }

    if osm_layer < 0.0 {
        return RoadProfile {
            kind: RoadProfileKind::Tunnel,
            layer_offset: ROAD_TUNNEL_LAYER_Y_OFFSET * osm_layer.abs(),
        };
    }

    RoadProfile {
        kind: RoadProfileKind::Surface,
        layer_offset: surface_offset,
    }
}

pub fn road_layer_y_offset(tags: &HashMap<String, String>) -> f32 {
    road_profile(tags).layer_offset
}

/// Surface-road Y offset bucketed by road width. Visible to siblings so the
/// render-path elevation sampler can compute the surface-to-bridge lift.
pub(super) fn surface_road_y_offset(tags: &HashMap<String, String>) -> f32 {
    let width = crate::world::color::road_width(tags);
    if width >= 5.0 {
        0.03
    } else if width >= 3.5 {
        0.025
    } else {
        0.02
    }
}
