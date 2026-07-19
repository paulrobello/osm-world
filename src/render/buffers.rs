use wgpu::util::DeviceExt;
use wgpu::*;

use super::vertex::{Vertex, feature};
#[cfg(any(test, feature = "dev_scene"))]
use crate::mesh::{BoxSpec, append_box};

/// Render layer classification for triangle batching.
///
/// Maps a vertex `feature_type` f32 constant to a typed layer, enabling
/// exhaustive `match` dispatch instead of float equality chains.
enum FeatureLayer {
    Terrain,
    Landuse,
    LanduseOverlay,
    Water,
    RoadPath,
    Road,
    Railway,
    RoadMarking,
    Solids,
}

impl FeatureLayer {
    fn from_f32(feature_type: f32) -> Self {
        match feature_type {
            _ if feature_type == feature::TERRAIN => Self::Terrain,
            _ if feature_type == feature::LANDUSE => Self::Landuse,
            _ if feature_type == feature::LANDUSE_OVERLAY => Self::LanduseOverlay,
            _ if feature_type == feature::WATER => Self::Water,
            _ if feature_type == feature::ROAD_PATH => Self::RoadPath,
            _ if feature_type == feature::ROAD || feature_type == feature::ROAD_LAYERED => {
                Self::Road
            }
            _ if feature_type == feature::RAILWAY => Self::Railway,
            _ if feature_type == feature::ROAD_MARKING
                || feature_type == feature::ROAD_MARKING_LAYERED =>
            {
                Self::RoadMarking
            }
            _ => Self::Solids,
        }
    }
}

pub struct RenderIndexBuffer {
    pub buffer: Buffer,
    pub count: u32,
}

pub struct SceneBuffers {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
    pub terrain: Option<RenderIndexBuffer>,
    pub landuse: Option<RenderIndexBuffer>,
    pub landuse_overlay: Option<RenderIndexBuffer>,
    pub water: Option<RenderIndexBuffer>,
    pub road_path: Option<RenderIndexBuffer>,
    pub road: Option<RenderIndexBuffer>,
    pub railway: Option<RenderIndexBuffer>,
    pub road_marking: Option<RenderIndexBuffer>,
    pub solids: Option<RenderIndexBuffer>,
    pub shadow_index_buffer: Buffer,
    pub shadow_index_count: u32,
}

impl SceneBuffers {
    pub fn new(device: &Device) -> Self {
        // The dev/test fallback scene (one box on a ground plane) is only
        // compiled under `cfg(test)` or the `dev_scene` feature. In a plain
        // release build `--input` is expected, so an empty buffer here is the
        // correct fallback rather than shipping test geometry in the binary.
        #[cfg(any(test, feature = "dev_scene"))]
        let (vertices, indices) = generate_test_scene();
        #[cfg(not(any(test, feature = "dev_scene")))]
        let (vertices, indices): (Vec<Vertex>, Vec<u32>) = (Vec::new(), Vec::new());
        Self::from_data(device, vertices, indices)
    }

    pub fn from_mesh(device: &Device, vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self::from_data(device, vertices, indices)
    }

    fn from_data(device: &Device, vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        let index_count = indices.len() as u32;
        let shadow_indices = shadow_index_data(&vertices, &indices);
        let render_layers = render_layer_index_data(&vertices, &indices);

        let vertex_contents: &[u8] = if vertices.is_empty() {
            &[0]
        } else {
            bytemuck::cast_slice(&vertices)
        };
        let index_contents: &[u8] = if indices.is_empty() {
            &[0]
        } else {
            bytemuck::cast_slice(&indices)
        };
        let shadow_contents: &[u8] = if shadow_indices.buffer_indices.is_empty() {
            &[0]
        } else {
            bytemuck::cast_slice(&shadow_indices.buffer_indices)
        };

        let vertex_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("scene vertex buffer"),
            contents: vertex_contents,
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("scene index buffer"),
            contents: index_contents,
            usage: BufferUsages::INDEX,
        });

        let shadow_index_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("shadow caster index buffer"),
            contents: shadow_contents,
            usage: BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count,
            terrain: create_render_index_buffer(
                device,
                "terrain index buffer",
                &render_layers.terrain,
            ),
            landuse: create_render_index_buffer(
                device,
                "landuse index buffer",
                &render_layers.landuse,
            ),
            landuse_overlay: create_render_index_buffer(
                device,
                "landuse overlay index buffer",
                &render_layers.landuse_overlay,
            ),
            water: create_render_index_buffer(device, "water index buffer", &render_layers.water),
            road_path: create_render_index_buffer(
                device,
                "road path index buffer",
                &render_layers.road_path,
            ),
            road: create_render_index_buffer(device, "road index buffer", &render_layers.road),
            railway: create_render_index_buffer(
                device,
                "railway index buffer",
                &render_layers.railway,
            ),
            road_marking: create_render_index_buffer(
                device,
                "road marking index buffer",
                &render_layers.road_marking,
            ),
            solids: create_render_index_buffer(device, "solid index buffer", &render_layers.solids),
            shadow_index_buffer,
            shadow_index_count: shadow_indices.draw_count,
        }
    }
}

struct RenderLayerIndexData {
    terrain: Vec<u32>,
    landuse: Vec<u32>,
    landuse_overlay: Vec<u32>,
    water: Vec<u32>,
    road_path: Vec<u32>,
    road: Vec<u32>,
    railway: Vec<u32>,
    road_marking: Vec<u32>,
    solids: Vec<u32>,
}

fn create_render_index_buffer(
    device: &Device,
    label: &str,
    indices: &[u32],
) -> Option<RenderIndexBuffer> {
    if indices.is_empty() {
        return None;
    }
    let count = indices.len() as u32;
    let buffer = device.create_buffer_init(&util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(indices),
        usage: BufferUsages::INDEX,
    });
    Some(RenderIndexBuffer { buffer, count })
}

fn render_layer_index_data(vertices: &[Vertex], indices: &[u32]) -> RenderLayerIndexData {
    debug_assert_eq!(indices.len() % 3, 0, "scene indices must be triangle lists");

    let mut layers = RenderLayerIndexData {
        terrain: Vec::new(),
        landuse: Vec::new(),
        landuse_overlay: Vec::new(),
        water: Vec::new(),
        road_path: Vec::new(),
        road: Vec::new(),
        railway: Vec::new(),
        road_marking: Vec::new(),
        solids: Vec::new(),
    };

    for tri in indices.chunks_exact(3) {
        let feature_type = tri
            .iter()
            .filter_map(|&index| vertices.get(index as usize))
            .map(|vertex| vertex.feature_type)
            .next()
            .unwrap_or(feature::BUILDING);
        let layer = match FeatureLayer::from_f32(feature_type) {
            FeatureLayer::Terrain => &mut layers.terrain,
            FeatureLayer::Landuse => &mut layers.landuse,
            FeatureLayer::LanduseOverlay => &mut layers.landuse_overlay,
            FeatureLayer::Water => &mut layers.water,
            FeatureLayer::RoadPath => &mut layers.road_path,
            FeatureLayer::Road => &mut layers.road,
            FeatureLayer::Railway => &mut layers.railway,
            FeatureLayer::RoadMarking => &mut layers.road_marking,
            FeatureLayer::Solids => &mut layers.solids,
        };
        layer.extend_from_slice(tri);
    }

    layers
}

struct ShadowIndexData {
    buffer_indices: Vec<u32>,
    draw_count: u32,
}

fn shadow_index_data(vertices: &[Vertex], indices: &[u32]) -> ShadowIndexData {
    let buffer_indices = shadow_casting_indices(vertices, indices);
    let draw_count = buffer_indices.len() as u32;

    ShadowIndexData {
        buffer_indices: if buffer_indices.is_empty() {
            vec![0]
        } else {
            buffer_indices
        },
        draw_count,
    }
}

fn shadow_casting_indices(vertices: &[Vertex], indices: &[u32]) -> Vec<u32> {
    debug_assert_eq!(indices.len() % 3, 0, "scene indices must be triangle lists");

    // Receiver surfaces (terrain/roads/water/landuse) are intentionally omitted:
    // near-coplanar receiver geometry in the depth map causes map-wide self-shadowing.
    indices
        .chunks_exact(3)
        .filter(|tri| {
            tri.iter().all(|&index| {
                vertices
                    .get(index as usize)
                    .is_some_and(|vertex| vertex.feature_type == feature::BUILDING)
            })
        })
        .flatten()
        .copied()
        .collect()
}

#[cfg(any(test, feature = "dev_scene"))]
fn generate_test_scene() -> (Vec<Vertex>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idxs = Vec::new();

    append_ground_plane(&mut verts, &mut idxs, 2000.0);
    append_box(
        BoxSpec {
            min: [-10.0, 0.0, -15.0],
            max: [10.0, 15.0, 15.0],
            color: [0.85, 0.78, 0.65],
            feature_type: feature::BUILDING,
        },
        &mut verts,
        &mut idxs,
    );

    (verts, idxs)
}

#[cfg(any(test, feature = "dev_scene"))]
fn append_ground_plane(verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>, size: f32) {
    let base = verts.len() as u32;
    let h = size / 2.0;
    let n = [0.0, 1.0, 0.0];
    let c = [0.35, 0.55, 0.25];
    verts.extend_from_slice(&[
        Vertex {
            position: [-h, 0.0, -h],
            normal: n,
            color: c,
            uv: [0.0, 0.0],
            feature_type: feature::TERRAIN,
        },
        Vertex {
            position: [h, 0.0, -h],
            normal: n,
            color: c,
            uv: [0.0, 0.0],
            feature_type: feature::TERRAIN,
        },
        Vertex {
            position: [h, 0.0, h],
            normal: n,
            color: c,
            uv: [0.0, 0.0],
            feature_type: feature::TERRAIN,
        },
        Vertex {
            position: [-h, 0.0, h],
            normal: n,
            color: c,
            uv: [0.0, 0.0],
            feature_type: feature::TERRAIN,
        },
    ]);
    idxs.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(feature_type: f32) -> Vertex {
        Vertex {
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            color: [1.0; 3],
            uv: [0.0, 0.0],
            feature_type,
        }
    }

    #[test]
    fn render_layers_partition_surface_overlays_for_ordered_draws() {
        let vertices = vec![
            vertex(feature::TERRAIN),
            vertex(feature::TERRAIN),
            vertex(feature::TERRAIN),
            vertex(feature::LANDUSE),
            vertex(feature::LANDUSE),
            vertex(feature::LANDUSE),
            vertex(feature::LANDUSE_OVERLAY),
            vertex(feature::LANDUSE_OVERLAY),
            vertex(feature::LANDUSE_OVERLAY),
            vertex(feature::WATER),
            vertex(feature::WATER),
            vertex(feature::WATER),
            vertex(feature::ROAD_PATH),
            vertex(feature::ROAD_PATH),
            vertex(feature::ROAD_PATH),
            vertex(feature::ROAD),
            vertex(feature::ROAD),
            vertex(feature::ROAD),
            vertex(feature::RAILWAY),
            vertex(feature::RAILWAY),
            vertex(feature::RAILWAY),
            vertex(feature::ROAD_MARKING),
            vertex(feature::ROAD_MARKING),
            vertex(feature::ROAD_MARKING),
            vertex(feature::ROAD_LAYERED),
            vertex(feature::ROAD_LAYERED),
            vertex(feature::ROAD_LAYERED),
            vertex(feature::ROAD_MARKING_LAYERED),
            vertex(feature::ROAD_MARKING_LAYERED),
            vertex(feature::ROAD_MARKING_LAYERED),
            vertex(feature::BUILDING),
            vertex(feature::BUILDING),
            vertex(feature::BUILDING),
        ];
        let indices: Vec<u32> = (0..vertices.len() as u32).collect();

        let layers = render_layer_index_data(&vertices, &indices);

        assert_eq!(layers.terrain, vec![0, 1, 2]);
        assert_eq!(layers.landuse, vec![3, 4, 5]);
        assert_eq!(layers.landuse_overlay, vec![6, 7, 8]);
        assert_eq!(layers.water, vec![9, 10, 11]);
        assert_eq!(layers.road_path, vec![12, 13, 14]);
        assert_eq!(layers.road, vec![15, 16, 17, 24, 25, 26]);
        assert_eq!(layers.railway, vec![18, 19, 20]);
        assert_eq!(layers.road_marking, vec![21, 22, 23, 27, 28, 29]);
        assert_eq!(layers.solids, vec![30, 31, 32]);
    }

    #[test]
    fn shadow_indices_keep_only_building_triangles() {
        let vertices = vec![
            vertex(feature::TERRAIN),
            vertex(feature::TERRAIN),
            vertex(feature::TERRAIN),
            vertex(feature::BUILDING),
            vertex(feature::BUILDING),
            vertex(feature::BUILDING),
            vertex(feature::ROAD),
            vertex(feature::ROAD),
            vertex(feature::ROAD),
        ];
        let indices = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];

        assert_eq!(shadow_casting_indices(&vertices, &indices), vec![3, 4, 5]);
    }

    #[test]
    fn shadow_indices_drop_mixed_receiver_and_caster_triangles() {
        let vertices = vec![
            vertex(feature::TERRAIN),
            vertex(feature::BUILDING),
            vertex(feature::BUILDING),
        ];
        let indices = vec![0, 1, 2];

        assert!(shadow_casting_indices(&vertices, &indices).is_empty());
    }

    #[test]
    fn shadow_index_data_keeps_zero_draw_count_for_receiver_only_meshes() {
        let vertices = vec![
            vertex(feature::TERRAIN),
            vertex(feature::TERRAIN),
            vertex(feature::TERRAIN),
        ];
        let indices = vec![0, 1, 2];

        let data = shadow_index_data(&vertices, &indices);

        assert_eq!(data.draw_count, 0);
        assert_eq!(data.buffer_indices, vec![0]);
    }
}
