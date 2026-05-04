//! Railway track mesh generator.

use std::collections::HashMap;

use crate::render::vertex::{Vertex, feature};

const TRACK_GAUGE: f32 = 1.435;
const RAIL_HEAD_WIDTH: f32 = 0.16;
const TIE_LENGTH: f32 = 2.8;
const TIE_WIDTH: f32 = 0.45;
const TIE_SPACING: f32 = 3.0;
const TIE_Y_OFFSET: f32 = 0.06;
const RAIL_Y_OFFSET: f32 = 0.14;
const RAIL_COLOR: [f32; 3] = [0.58, 0.58, 0.55];
const TIE_COLOR: [f32; 3] = [0.24, 0.16, 0.10];

pub fn is_renderable_railway(tags: &HashMap<String, String>) -> bool {
    matches!(
        tags.get("railway").map(String::as_str),
        Some("rail" | "light_rail" | "tram" | "subway" | "narrow_gauge")
    )
}

pub fn generate_railway_track(
    tags: &HashMap<String, String>,
    points: &[(f32, f32)],
    elevations: &[f32],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    if points.len() != elevations.len() || points.len() < 2 {
        return;
    }

    let base_y_offset = super::road::road_layer_y_offset(tags) + super::road::ROAD_Y_OFFSET;
    for i in 0..points.len() - 1 {
        let a = points[i];
        let b = points[i + 1];
        let dx = b.0 - a.0;
        let dz = b.1 - a.1;
        let segment_length = (dx * dx + dz * dz).sqrt();
        if segment_length <= 1e-4 {
            continue;
        }

        let dir = (dx / segment_length, dz / segment_length);
        let perp = (-dir.1, dir.0);
        let start_y = elevations[i] + base_y_offset;
        let end_y = elevations[i + 1] + base_y_offset;

        for rail_offset in [-TRACK_GAUGE * 0.5, TRACK_GAUGE * 0.5] {
            append_segment_quad(
                SegmentQuad {
                    a,
                    b,
                    perp,
                    start_y: start_y + RAIL_Y_OFFSET,
                    end_y: end_y + RAIL_Y_OFFSET,
                    lateral_offset: rail_offset,
                    half_width: RAIL_HEAD_WIDTH * 0.5,
                    color: RAIL_COLOR,
                },
                verts,
                idxs,
            );
        }

        let mut tie_center = 0.0;
        while tie_center <= segment_length {
            let t = tie_center / segment_length;
            let cx = a.0 + dx * t;
            let cz = a.1 + dz * t;
            let y = start_y + (end_y - start_y) * t + TIE_Y_OFFSET;
            append_tie_quad((cx, cz), dir, perp, y, verts, idxs);
            tie_center += TIE_SPACING;
        }
    }
}

struct SegmentQuad {
    a: (f32, f32),
    b: (f32, f32),
    perp: (f32, f32),
    start_y: f32,
    end_y: f32,
    lateral_offset: f32,
    half_width: f32,
    color: [f32; 3],
}

fn append_segment_quad(quad: SegmentQuad, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    let left = quad.lateral_offset - quad.half_width;
    let right = quad.lateral_offset + quad.half_width;
    push_quad(
        [
            [
                quad.a.0 + quad.perp.0 * left,
                quad.start_y,
                quad.a.1 + quad.perp.1 * left,
            ],
            [
                quad.b.0 + quad.perp.0 * left,
                quad.end_y,
                quad.b.1 + quad.perp.1 * left,
            ],
            [
                quad.a.0 + quad.perp.0 * right,
                quad.start_y,
                quad.a.1 + quad.perp.1 * right,
            ],
            [
                quad.b.0 + quad.perp.0 * right,
                quad.end_y,
                quad.b.1 + quad.perp.1 * right,
            ],
        ],
        quad.color,
        verts,
        idxs,
    );
}

fn append_tie_quad(
    center: (f32, f32),
    dir: (f32, f32),
    perp: (f32, f32),
    y: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let half_tie_width = TIE_WIDTH * 0.5;
    let half_tie_length = TIE_LENGTH * 0.5;
    let start = (
        center.0 - dir.0 * half_tie_width,
        center.1 - dir.1 * half_tie_width,
    );
    let end = (
        center.0 + dir.0 * half_tie_width,
        center.1 + dir.1 * half_tie_width,
    );
    push_quad(
        [
            [
                start.0 + perp.0 * half_tie_length,
                y,
                start.1 + perp.1 * half_tie_length,
            ],
            [
                end.0 + perp.0 * half_tie_length,
                y,
                end.1 + perp.1 * half_tie_length,
            ],
            [
                start.0 - perp.0 * half_tie_length,
                y,
                start.1 - perp.1 * half_tie_length,
            ],
            [
                end.0 - perp.0 * half_tie_length,
                y,
                end.1 - perp.1 * half_tie_length,
            ],
        ],
        TIE_COLOR,
        verts,
        idxs,
    );
}

fn push_quad(
    positions: [[f32; 3]; 4],
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let base = verts.len() as u32;
    for position in positions {
        verts.push(Vertex {
            position,
            normal: [0.0, 1.0, 0.0],
            color,
            feature_type: feature::RAILWAY,
            uv: [0.0, 0.0],
        });
    }
    idxs.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn railway_track_emits_two_rails_and_repeated_ties() {
        let tags = HashMap::from([("railway".to_string(), "rail".to_string())]);
        let points = [(0.0, 0.0), (12.0, 0.0)];
        let elevations = [1.0, 1.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        generate_railway_track(&tags, &points, &elevations, &mut vertices, &mut indices);

        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
        assert!(vertices.iter().all(|v| v.feature_type == feature::RAILWAY));
        assert!(vertices.iter().any(|v| v.color == RAIL_COLOR));
        assert!(vertices.iter().any(|v| v.color == TIE_COLOR));
    }

    #[test]
    fn railway_track_skips_unknown_railway_values() {
        let tags = HashMap::from([("railway".to_string(), "platform".to_string())]);

        assert!(!is_renderable_railway(&tags));
    }

    #[test]
    fn railway_track_uses_layer_offsets_for_bridges() {
        let tags = HashMap::from([
            ("railway".to_string(), "rail".to_string()),
            ("bridge".to_string(), "yes".to_string()),
        ]);
        let points = [(0.0, 0.0), (6.0, 0.0)];
        let elevations = [0.0, 0.0];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        generate_railway_track(&tags, &points, &elevations, &mut vertices, &mut indices);

        let min_y = vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::INFINITY, f32::min);
        assert!(min_y > super::super::road::ROAD_Y_OFFSET + 4.0);
    }
}
