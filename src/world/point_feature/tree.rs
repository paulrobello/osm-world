//! Tree point-feature geometry: hexagonal prism trunk + octahedron canopy.

use crate::mesh::Vertex;
use crate::visual_detail::{VisualDetailSettings, VisualPreset};

use super::geometry::{QuadFace, append_outward_tri, append_quad};

const TREE_TRUNK_COLOR: [f32; 3] = [0.45, 0.24, 0.10];
const TREE_CANOPY_COLOR: [f32; 3] = [0.16, 0.48, 0.18];
const SHOWCASE_TREE_CANOPY_COLOR: [f32; 3] = [0.24, 0.68, 0.28];

pub(super) fn append_tree_with_visual_detail(
    point: (f32, f32),
    elevation: f32,
    visual_detail: &VisualDetailSettings,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let (scale, canopy_color) = if visual_detail.preset == VisualPreset::Showcase {
        (1.35, SHOWCASE_TREE_CANOPY_COLOR)
    } else {
        (1.0, TREE_CANOPY_COLOR)
    };
    append_tree_with_style(point, elevation, scale, canopy_color, verts, idxs);
}

fn append_tree_with_style(
    point: (f32, f32),
    elevation: f32,
    scale: f32,
    canopy_color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    append_hex_trunk(point, elevation, 0.65 * scale, 3.0 * scale, verts, idxs);
    append_octahedron_canopy(
        point,
        elevation + 4.2 * scale,
        2.0 * scale,
        canopy_color,
        verts,
        idxs,
    );
}

fn append_hex_trunk(
    point: (f32, f32),
    base_y: f32,
    radius: f32,
    height: f32,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let bottom = hex_ring(point, base_y, radius);
    let top = hex_ring(point, base_y + height, radius * 0.82);
    for i in 0..bottom.len() {
        let next = (i + 1) % bottom.len();
        let p0 = bottom[i];
        let p1 = bottom[next];
        let p2 = top[next];
        let p3 = top[i];
        let face_center = (glam::Vec3::from_array(p0)
            + glam::Vec3::from_array(p1)
            + glam::Vec3::from_array(p2)
            + glam::Vec3::from_array(p3))
            / 4.0;
        let normal =
            glam::vec3(face_center.x - point.0, 0.0, face_center.z - point.1).normalize_or_zero();
        append_quad(
            QuadFace {
                positions: [p0, p1, p2, p3],
                normal: normal.to_array(),
            },
            TREE_TRUNK_COLOR,
            verts,
            idxs,
        );
    }
}

fn hex_ring(point: (f32, f32), y: f32, radius: f32) -> [[f32; 3]; 6] {
    std::array::from_fn(|i| {
        let angle = std::f32::consts::FRAC_PI_6 + i as f32 * std::f32::consts::TAU / 6.0;
        [
            point.0 + angle.cos() * radius,
            y,
            point.1 + angle.sin() * radius,
        ]
    })
}

fn append_octahedron_canopy(
    point: (f32, f32),
    center_y: f32,
    radius: f32,
    color: [f32; 3],
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u32>,
) {
    let center = glam::vec3(point.0, center_y, point.1);
    let raw = [
        glam::vec3(0.0, 1.0, 0.0),
        glam::vec3(1.0, 0.0, 0.0),
        glam::vec3(0.0, 0.0, 1.0),
        glam::vec3(-1.0, 0.0, 0.0),
        glam::vec3(0.0, 0.0, -1.0),
        glam::vec3(0.0, -1.0, 0.0),
    ];
    let points = raw.map(|p| (center + p * radius).to_array());
    for [a, b, c] in OCTAHEDRON_FACES {
        append_outward_tri(center, points[a], points[b], points[c], color, verts, idxs);
    }
}

const OCTAHEDRON_FACES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [0, 2, 3],
    [0, 3, 4],
    [0, 4, 1],
    [5, 2, 1],
    [5, 3, 2],
    [5, 4, 3],
    [5, 1, 4],
];
