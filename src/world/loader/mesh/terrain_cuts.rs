//! Tunnel-portal terrain cuts. Computes the rectangular depressions punched
//! into the terrain mesh at tunnel entrances so the road deck is visible
//! instead of buried under the surface.

use crate::world::loader::source::{ResolvedFeature, WorldSource};

pub(super) fn terrain_cuts_for_roads(
    roads: &[ResolvedFeature],
) -> Vec<crate::world::terrain::TerrainCut> {
    roads.iter().flat_map(terrain_cuts_for_road).collect()
}

pub(super) fn terrain_cuts_for_road_refs(
    source: &WorldSource,
    road_refs: &[usize],
) -> Vec<crate::world::terrain::TerrainCut> {
    road_refs
        .iter()
        .filter_map(|&feature_idx| source.roads.get(feature_idx))
        .flat_map(terrain_cuts_for_road)
        .collect()
}

fn terrain_cuts_for_road(road: &ResolvedFeature) -> Vec<crate::world::terrain::TerrainCut> {
    if crate::world::road::road_profile(&road.tags).kind
        != crate::world::road::RoadProfileKind::Tunnel
        || road.points.len() < 2
        || road.points.len() != road.elevations.len()
    {
        return Vec::new();
    }

    let road_elevations =
        crate::world::road::road_render_elevations(&road.tags, &road.points, &road.elevations);
    if road_elevations.len() != road.points.len() {
        return Vec::new();
    }

    let width = crate::world::color::road_width(&road.tags);
    let mut cuts = Vec::with_capacity(2);
    if let Some(cut) =
        tunnel_portal_terrain_cut(road.points[0], road.points[1], road_elevations[0], width)
    {
        cuts.push(cut);
    }
    let last = road.points.len() - 1;
    if let Some(cut) = tunnel_portal_terrain_cut(
        road.points[last],
        road.points[last - 1],
        road_elevations[last],
        width,
    ) {
        cuts.push(cut);
    }
    cuts
}

fn tunnel_portal_terrain_cut(
    point: (f32, f32),
    next: (f32, f32),
    road_elevation: f32,
    width: f32,
) -> Option<crate::world::terrain::TerrainCut> {
    let dx = next.0 - point.0;
    let dz = next.1 - point.1;
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1e-6 {
        return None;
    }

    let dir_x = dx / len;
    let dir_z = dz / len;
    let cut_length = 18.0_f32.min(len.max(8.0));
    Some(crate::world::terrain::TerrainCut {
        start: point,
        end: (point.0 + dir_x * cut_length, point.1 + dir_z * cut_length),
        half_width: width * 0.5 + 4.0,
        floor_y: road_elevation + crate::world::road::ROAD_Y_OFFSET + 0.25,
        blend_width: 12.0,
    })
}
