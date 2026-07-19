//! Road module tests. Split from `mod.rs` (ARC-012) without modification —
//! every test is preserved verbatim. The tests reach into sibling submodules
//! through `mod.rs`'s re-exports (`use super::*`).

use super::*;

fn triangle_normal_y(a: Vertex, b: Vertex, c: Vertex) -> f32 {
    let ux = b.position[0] - a.position[0];
    let uz = b.position[2] - a.position[2];
    let vx = c.position[0] - a.position[0];
    let vz = c.position[2] - a.position[2];
    uz * vx - ux * vz
}

#[test]
fn road_layer_y_offset_lifts_bridges_above_surface_roads() {
    let surface = std::collections::HashMap::from([("highway".to_string(), "primary".to_string())]);
    let bridge = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("bridge".to_string(), "yes".to_string()),
        ("layer".to_string(), "1".to_string()),
    ]);

    assert!(road_layer_y_offset(&surface) >= 0.02);
    assert!(road_layer_y_offset(&bridge) >= road_layer_y_offset(&surface) + 4.0);
}

#[test]
fn road_profile_classifies_bridge_tunnel_and_surface_roads() {
    let surface = std::collections::HashMap::from([("highway".to_string(), "primary".to_string())]);
    let bridge = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("bridge".to_string(), "yes".to_string()),
    ]);
    let tunnel = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("tunnel".to_string(), "yes".to_string()),
    ]);

    assert_eq!(road_profile(&surface).kind, RoadProfileKind::Surface);
    assert_eq!(road_profile(&bridge).kind, RoadProfileKind::Bridge);
    assert_eq!(road_profile(&tunnel).kind, RoadProfileKind::Tunnel);
}

#[test]
fn road_layer_y_offset_lowers_tunnels_below_surface_roads() {
    let surface = std::collections::HashMap::from([("highway".to_string(), "primary".to_string())]);
    let tunnel = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("tunnel".to_string(), "yes".to_string()),
        ("layer".to_string(), "-1".to_string()),
    ]);

    assert!(road_layer_y_offset(&surface) > 0.0);
    assert!(road_layer_y_offset(&tunnel) <= road_layer_y_offset(&surface) - 4.0);
}

#[test]
fn bridge_tag_wins_over_negative_layer_without_tunnel() {
    let tags = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("bridge".to_string(), "yes".to_string()),
        ("layer".to_string(), "-1".to_string()),
    ]);

    assert_eq!(road_profile(&tags).kind, RoadProfileKind::Bridge);
    assert!(road_layer_y_offset(&tags) > 0.0);
}

#[test]
fn explicit_tunnel_wins_over_bridge_tags() {
    let tags = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("bridge".to_string(), "yes".to_string()),
        ("tunnel".to_string(), "yes".to_string()),
        ("layer".to_string(), "1".to_string()),
    ]);

    assert_eq!(road_profile(&tags).kind, RoadProfileKind::Tunnel);
    assert!(road_layer_y_offset(&tags) < 0.0);
}

#[test]
fn centerline_dashes_can_use_layered_marking_feature_type() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let road_elevations = [3.0, 3.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    append_road_centerline_dashes_with_feature_type(
        &points,
        &road_elevations,
        6.0,
        feature::ROAD_MARKING_LAYERED,
        &mut vertices,
        &mut indices,
    );

    assert!(!indices.is_empty());
    assert!(
        vertices
            .iter()
            .all(|v| v.feature_type == feature::ROAD_MARKING_LAYERED)
    );
}

#[test]
fn centerline_dashes_emit_yellow_markings_above_wide_roads() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let road_elevations = [3.0, 3.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    append_road_centerline_dashes(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

    assert!(!vertices.is_empty());
    assert!(!indices.is_empty());
    assert!(
        vertices
            .iter()
            .all(|v| v.feature_type == feature::ROAD_MARKING)
    );
    assert!(vertices.iter().all(|v| v.color == CENTERLINE_COLOR));
    assert!(
        vertices
            .iter()
            .all(|v| v.position[1] > road_elevations[0] + ROAD_Y_OFFSET)
    );
}

#[test]
fn centerline_dashes_skip_narrow_roads() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let road_elevations = [3.0, 3.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    append_road_centerline_dashes(&points, &road_elevations, 2.0, &mut vertices, &mut indices);

    assert!(vertices.is_empty());
    assert!(indices.is_empty());
}

#[test]
fn centerline_dashes_follow_sloped_road_elevations() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let road_elevations = [0.0, 4.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    append_road_centerline_dashes(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

    let min_y = vertices
        .iter()
        .map(|v| v.position[1])
        .fold(f32::INFINITY, f32::min);
    let max_y = vertices
        .iter()
        .map(|v| v.position[1])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(max_y > min_y + 0.5);
}

#[test]
fn segment_strip_box_stays_close_to_diagonal_segment() {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    append_segment_strip_box(
        SegmentStripBox {
            a: (0.0, 0.0),
            b: (10.0, 10.0),
            lateral_offset: 0.0,
            half_width: 0.5,
            min_y: 1.0,
            max_y: 2.0,
            color: BRIDGE_STRUCTURE_COLOR,
        },
        &mut vertices,
        &mut indices,
    );

    assert!(!indices.is_empty());
    let max_distance_from_segment = vertices
        .iter()
        .map(|vertex| {
            let x = vertex.position[0];
            let z = vertex.position[2];
            ((z - x).abs()) / 2.0_f32.sqrt()
        })
        .fold(0.0_f32, f32::max);

    assert!(
        max_distance_from_segment <= 0.51,
        "diagonal strip expanded into a broad axis-aligned slab: max distance {max_distance_from_segment}"
    );
}

#[test]
fn bridge_structure_adds_side_beams_rails_and_support_geometry() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let terrain_elevations = [0.0, 0.0];
    let road_elevations = [5.7, 5.7];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    bridge::append_bridge_structure(
        &points,
        &terrain_elevations,
        &road_elevations,
        6.0,
        &mut vertices,
        &mut indices,
    );

    assert!(!vertices.is_empty());
    assert!(!indices.is_empty());
    assert!(vertices.iter().any(|v| v.feature_type == feature::BUILDING));
    assert!(
        vertices
            .iter()
            .any(|v| v.position[1] < road_elevations[0] + ROAD_Y_OFFSET)
    );
    assert!(
        vertices
            .iter()
            .any(|v| v.position[1] <= terrain_elevations[0] + 0.1)
    );
}

#[test]
fn bridge_structure_adds_rails_but_skips_flat_deck_on_sloped_ramp_segments() {
    let points = [(0.0, 0.0), (25.0, 0.0)];
    let terrain_elevations = [0.0, 0.0];
    let road_elevations = [0.7, 5.7];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    bridge::append_bridge_structure(
        &points,
        &terrain_elevations,
        &road_elevations,
        6.0,
        &mut vertices,
        &mut indices,
    );

    assert!(!vertices.is_empty());
    assert!(!indices.is_empty());
    assert!(vertices.iter().any(|v| v.position[1] < 4.0));
    let high_road_y = road_elevations[1] + ROAD_Y_OFFSET;
    assert!(vertices.iter().any(|v| v.position[1] > high_road_y + 0.5));
    assert!(
        vertices
            .iter()
            .all(|v| v.position[1] > terrain_elevations[0])
    );
}

#[test]
fn bridge_structure_adds_abutment_walls_at_approach_transitions() {
    let points = [(0.0, 0.0), (25.0, 0.0), (75.0, 0.0), (100.0, 0.0)];
    let terrain_elevations = [0.0, 0.0, 0.0, 0.0];
    let road_elevations = [0.7, 5.7, 5.7, 0.7];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    bridge::append_bridge_structure(
        &points,
        &terrain_elevations,
        &road_elevations,
        6.0,
        &mut vertices,
        &mut indices,
    );

    assert!(vertices.iter().any(|v| {
        (v.position[0] - 25.0).abs() <= 0.4
            && v.position[1] <= terrain_elevations[1] + 0.1
            && v.position[2].abs() >= 3.0
    }));
    assert!(vertices.iter().any(|v| {
        (v.position[0] - 75.0).abs() <= 0.4
            && v.position[1] <= terrain_elevations[2] + 0.1
            && v.position[2].abs() >= 3.0
    }));
}

#[test]
fn bridge_structure_keeps_beams_well_below_road_surface() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let terrain_elevations = [0.0, 0.0];
    let road_elevations = [5.7, 5.7];
    let road_y = road_elevations[0] + ROAD_Y_OFFSET;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    bridge::append_bridge_structure(
        &points,
        &terrain_elevations,
        &road_elevations,
        6.0,
        &mut vertices,
        &mut indices,
    );

    assert!(vertices.iter().any(|v| v.position[1] < road_y));
    assert!(
        vertices
            .iter()
            .filter(|v| v.position[1] < road_y)
            .all(|v| v.position[1] <= road_y - 0.75)
    );
}

#[test]
fn bridge_structure_does_not_emit_broad_deck_top_faces() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let terrain_elevations = [0.0, 0.0];
    let road_elevations = [5.7, 5.7];
    let road_y = road_elevations[0] + ROAD_Y_OFFSET;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    bridge::append_bridge_structure(
        &points,
        &terrain_elevations,
        &road_elevations,
        6.0,
        &mut vertices,
        &mut indices,
    );

    let widest_under_road_up_face = indices
        .chunks_exact(3)
        .filter_map(|tri| {
            let tri = [
                vertices[tri[0] as usize],
                vertices[tri[1] as usize],
                vertices[tri[2] as usize],
            ];
            if tri
                .iter()
                .all(|v| v.normal == [0.0, 1.0, 0.0] && v.position[1] < road_y)
            {
                let min_z = tri
                    .iter()
                    .map(|v| v.position[2])
                    .fold(f32::INFINITY, f32::min);
                let max_z = tri
                    .iter()
                    .map(|v| v.position[2])
                    .fold(f32::NEG_INFINITY, f32::max);
                Some(max_z - min_z)
            } else {
                None
            }
        })
        .fold(0.0, f32::max);

    assert!(widest_under_road_up_face <= 1.0);
}

#[test]
fn bridge_structure_box_geometry_has_per_face_normals() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let terrain_elevations = [0.0, 0.0];
    let road_elevations = [5.7, 5.7];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    bridge::append_bridge_structure(
        &points,
        &terrain_elevations,
        &road_elevations,
        6.0,
        &mut vertices,
        &mut indices,
    );

    for expected_normal in [
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ] {
        assert!(
            vertices.iter().any(|v| v.normal == expected_normal),
            "missing normal {expected_normal:?}"
        );
    }

    assert!(vertices.iter().all(|v| v.feature_type == feature::BUILDING));
}

#[test]
fn tunnel_structure_adds_lining_along_open_tunnel() {
    let points = [(0.0, 0.0), (40.0, 0.0)];
    let road_elevations = [-5.0, -5.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    tunnel::append_tunnel_structure(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

    assert!(!vertices.is_empty());
    assert!(!indices.is_empty());
    assert!(vertices.iter().any(|v| {
        v.feature_type == feature::BUILDING
            && v.position[0] > 15.0
            && v.position[0] < 25.0
            && v.position[1] > road_elevations[0] + ROAD_Y_OFFSET
    }));
}

#[test]
fn tunnel_structure_adds_portals_for_open_tunnel() {
    let points = [(0.0, 0.0), (20.0, 0.0)];
    let road_elevations = [-5.0, -5.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    tunnel::append_tunnel_structure(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

    assert!(!vertices.is_empty());
    assert!(!indices.is_empty());
    assert!(vertices.iter().any(|v| v.feature_type == feature::BUILDING));
    assert!(
        vertices
            .iter()
            .any(|v| v.position[1] > road_elevations[0] + ROAD_Y_OFFSET)
    );
}

#[test]
fn tunnel_structure_skips_portals_for_closed_loops() {
    let points = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 0.0)];
    let road_elevations = [-5.0, -5.0, -5.0, -5.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    tunnel::append_tunnel_structure(&points, &road_elevations, 6.0, &mut vertices, &mut indices);

    assert!(vertices.is_empty());
    assert!(indices.is_empty());
}

#[test]
fn bridge_render_path_inserts_ramp_breakpoints_for_two_point_bridge() {
    let tags = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("bridge".to_string(), "yes".to_string()),
    ]);
    let points = [(0.0, 0.0), (100.0, 0.0)];
    let terrain_elevations = [10.0, 10.0];

    let path = road_render_path(&tags, &points, &terrain_elevations);
    let surface_y = terrain_elevations[0] + surface_road_y_offset(&tags);
    let bridge_y = terrain_elevations[0] + road_layer_y_offset(&tags);

    assert_eq!(
        path.points,
        vec![(0.0, 0.0), (25.0, 0.0), (75.0, 0.0), (100.0, 0.0)]
    );
    assert!((path.road_elevations[0] - surface_y).abs() < 1e-5);
    assert!((path.road_elevations[1] - bridge_y).abs() < 1e-5);
    assert!((path.road_elevations[2] - bridge_y).abs() < 1e-5);
    assert!((path.road_elevations[3] - surface_y).abs() < 1e-5);
}

#[test]
fn bridge_render_elevations_ramp_from_surface_to_bridge_and_back() {
    let tags = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("bridge".to_string(), "yes".to_string()),
    ]);
    let points = [(0.0, 0.0), (12.5, 0.0), (25.0, 0.0), (50.0, 0.0)];
    let terrain_elevations = [10.0, 10.0, 10.0, 10.0];

    let elevations = road_render_elevations(&tags, &points, &terrain_elevations);
    let surface_y = terrain_elevations[0] + surface_road_y_offset(&tags);
    let bridge_y = terrain_elevations[0] + road_layer_y_offset(&tags);

    assert!((elevations[0] - surface_y).abs() < 1e-5);
    assert!(elevations[1] > surface_y);
    assert!(elevations[1] < bridge_y);
    assert!((elevations[2] - bridge_y).abs() < 1e-5);
    assert!((elevations[3] - surface_y).abs() < 1e-5);
}

#[test]
fn bridge_render_elevations_clamp_ramps_for_short_bridges() {
    let tags = std::collections::HashMap::from([
        ("highway".to_string(), "primary".to_string()),
        ("bridge".to_string(), "yes".to_string()),
    ]);
    let points = [(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)];
    let terrain_elevations = [0.0, 0.0, 0.0];

    let elevations = road_render_elevations(&tags, &points, &terrain_elevations);
    let surface_y = surface_road_y_offset(&tags);
    let bridge_y = road_layer_y_offset(&tags);

    assert_eq!(elevations[0], surface_y);
    assert_eq!(elevations[1], bridge_y);
    assert_eq!(elevations[2], surface_y);
}

#[test]
fn surface_render_elevations_keep_constant_surface_offset() {
    let tags = std::collections::HashMap::from([("highway".to_string(), "primary".to_string())]);
    let points = [(0.0, 0.0), (50.0, 0.0)];
    let terrain_elevations = [1.0, 2.0];

    let elevations = road_render_elevations(&tags, &points, &terrain_elevations);

    assert_eq!(
        elevations,
        vec![
            1.0 + road_layer_y_offset(&tags),
            2.0 + road_layer_y_offset(&tags)
        ]
    );
}

#[test]
fn road_ribbon_can_use_path_feature_type_for_ordered_overlay_draws() {
    let points = [(0.0, 0.0), (10.0, 0.0)];
    let elevations = [5.0, 7.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    generate_road_with_elevations_and_feature_type(
        &points,
        &elevations,
        2.0,
        [1.0, 1.0, 1.0],
        feature::ROAD_PATH,
        &mut vertices,
        &mut indices,
    );

    assert!(!indices.is_empty());
    assert!(
        vertices
            .iter()
            .all(|v| v.feature_type == feature::ROAD_PATH)
    );
}

#[test]
fn road_ribbon_uses_per_point_elevation_offsets() {
    let points = [(0.0, 0.0), (10.0, 0.0)];
    let elevations = [5.0, 7.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    generate_road_with_elevations(
        &points,
        &elevations,
        4.0,
        [1.0, 1.0, 1.0],
        &mut vertices,
        &mut indices,
    );

    assert_eq!(vertices[0].position[1], elevations[0] + ROAD_Y_OFFSET);
    assert_eq!(vertices[1].position[1], elevations[0] + ROAD_Y_OFFSET);
    assert_eq!(vertices[2].position[1], elevations[1] + ROAD_Y_OFFSET);
    assert_eq!(vertices[3].position[1], elevations[1] + ROAD_Y_OFFSET);
}

#[test]
fn road_cap_sits_above_road_ribbon() {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    append_road_cap(
        (0.0, 0.0),
        5.0,
        4.0,
        [1.0, 1.0, 1.0],
        &mut vertices,
        &mut indices,
    );

    assert_eq!(vertices.len(), ROAD_CAP_SEGMENTS + 1);
    assert_eq!(indices.len(), ROAD_CAP_SEGMENTS * 3);
    assert_eq!(
        vertices[0].position[1],
        5.0 + ROAD_Y_OFFSET + ROAD_CAP_EXTRA_Y_OFFSET
    );
    for tri in indices.chunks_exact(3) {
        let normal_y = triangle_normal_y(
            vertices[tri[0] as usize],
            vertices[tri[1] as usize],
            vertices[tri[2] as usize],
        );
        assert!(
            normal_y > 0.0,
            "road cap triangle {tri:?} normal_y={normal_y}"
        );
    }
}

#[test]
fn closed_road_loop_drops_duplicate_endpoint_and_joins_seam() {
    let points = [
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (0.0, 10.0),
        (0.0, 0.0),
    ];
    let elevations = [5.0, 5.0, 5.0, 5.0, 5.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    generate_road_with_elevations(
        &points,
        &elevations,
        4.0,
        [1.0, 1.0, 1.0],
        &mut vertices,
        &mut indices,
    );

    assert_eq!(vertices.len(), 8);
    assert_eq!(indices.len(), 24);
    for tri in indices.chunks_exact(3) {
        let normal_y = triangle_normal_y(
            vertices[tri[0] as usize],
            vertices[tri[1] as usize],
            vertices[tri[2] as usize],
        );
        assert!(
            normal_y > 0.0,
            "closed road triangle {tri:?} normal_y={normal_y}"
        );
    }
}

#[test]
fn road_ribbon_ignores_consecutive_duplicate_points() {
    let points = [(0.0, 0.0), (10.0, 0.0), (10.0, 0.0), (20.0, 0.0)];
    let elevations = [5.0, 5.0, 5.0, 5.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    generate_road_with_elevations(
        &points,
        &elevations,
        4.0,
        [1.0, 1.0, 1.0],
        &mut vertices,
        &mut indices,
    );

    assert_eq!(vertices.len(), 6);
    assert_eq!(indices.len(), 12);
}

#[test]
fn road_ribbon_uses_shared_join_vertices_at_curve() {
    let points = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
    let elevations = [5.0, 5.0, 5.0];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    generate_road_with_elevations(
        &points,
        &elevations,
        4.0,
        [1.0, 1.0, 1.0],
        &mut vertices,
        &mut indices,
    );

    assert_eq!(vertices.len(), points.len() * 2);
    assert_eq!(indices.len(), (points.len() - 1) * 6);
}

#[test]
fn road_ribbon_triangles_face_up_for_back_face_culling() {
    let points = [(0.0, 0.0), (10.0, 0.0)];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    generate_road(
        &points,
        5.0,
        4.0,
        [1.0, 1.0, 1.0],
        &mut vertices,
        &mut indices,
    );
    assert!(!indices.is_empty(), "expected road triangles");

    for tri in indices.chunks_exact(3) {
        let normal_y = triangle_normal_y(
            vertices[tri[0] as usize],
            vertices[tri[1] as usize],
            vertices[tri[2] as usize],
        );
        assert!(normal_y > 0.0, "road triangle {tri:?} normal_y={normal_y}");
    }
}
