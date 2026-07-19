//! Shared mesh types used by both world generation and render pipeline.
//!
//! This module breaks the upward dependency where `world` previously imported
//! `Vertex` from `render`. Both modules now import from this shared location.

use bytemuck::{Pod, Zeroable};

/// GPU vertex format. 48 bytes per vertex.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub feature_type: f32,
    pub uv: [f32; 2],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x3,
        3 => Float32,
        4 => Float32x2,
    ];
}

/// Axis-aligned box primitive emitted as 24 vertices (4 per face × 6 faces)
/// and 36 triangle-list indices (2 triangles × 6 faces). UVs are zeroed.
///
/// Consolidated (ARC-016) from three previously duplicate local impls in
/// `render::buffers`, `world::point_feature`, and `world::road`. The actual
/// box-building algorithm now lives in one place; callers that need a
/// domain-specific default `feature_type` keep a thin local wrapper that
/// delegates here.
#[derive(Copy, Clone, Debug)]
pub struct BoxSpec {
    /// Minimum corner (inclusive).
    pub min: [f32; 3],
    /// Maximum corner (inclusive).
    pub max: [f32; 3],
    /// Flat color applied to every vertex.
    pub color: [f32; 3],
    /// Feature-type discriminant written to `Vertex::feature_type`.
    pub feature_type: f32,
}

impl BoxSpec {
    /// Build a `BoxSpec` from a center point on the XZ plane with the base
    /// sitting at `base_y` and the top at `base_y + height`. Matches the
    /// point-feature placement style; the X/Z extents are `±half_extents`
    /// around `center`.
    pub fn centered(
        center: (f32, f32),
        base_y: f32,
        half_extents: (f32, f32),
        height: f32,
        color: [f32; 3],
        feature_type: f32,
    ) -> Self {
        let (half_x, half_z) = half_extents;
        Self {
            min: [center.0 - half_x, base_y, center.1 - half_z],
            max: [center.0 + half_x, base_y + height, center.1 + half_z],
            color,
            feature_type,
        }
    }
}

/// Append an axis-aligned box (24 verts + 36 indices) to `verts` and `idxs`.
///
/// Degenerate axes (where `max[i] - min[i]` is below `1e-4`) are inflated by
/// `±0.05` so slab-like boxes used by road bridge/tunnel geometry remain
/// visible from both sides. Callers that already provide a full 3D box are
/// unaffected.
pub fn append_box(spec: BoxSpec, verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>) {
    let mut min = spec.min;
    let mut max = spec.max;
    for axis in 0..3 {
        if (max[axis] - min[axis]).abs() < 1e-4 {
            min[axis] -= 0.05;
            max[axis] += 0.05;
        }
    }

    let mut push_face = |positions: [[f32; 3]; 4], normal: [f32; 3]| {
        let base = verts.len() as u32;
        for position in positions {
            verts.push(Vertex {
                position,
                normal,
                color: spec.color,
                uv: [0.0, 0.0],
                feature_type: spec.feature_type,
            });
        }
        idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };

    // Back (z-)
    push_face(
        [
            [min[0], min[1], min[2]],
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], min[1], min[2]],
        ],
        [0.0, 0.0, -1.0],
    );
    // Front (z+)
    push_face(
        [
            [min[0], min[1], max[2]],
            [max[0], min[1], max[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ],
        [0.0, 0.0, 1.0],
    );
    // Left (x-)
    push_face(
        [
            [min[0], min[1], min[2]],
            [min[0], min[1], max[2]],
            [min[0], max[1], max[2]],
            [min[0], max[1], min[2]],
        ],
        [-1.0, 0.0, 0.0],
    );
    // Right (x+)
    push_face(
        [
            [max[0], min[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
            [max[0], min[1], max[2]],
        ],
        [1.0, 0.0, 0.0],
    );
    // Bottom (y-)
    push_face(
        [
            [min[0], min[1], min[2]],
            [max[0], min[1], min[2]],
            [max[0], min[1], max[2]],
            [min[0], min[1], max[2]],
        ],
        [0.0, -1.0, 0.0],
    );
    // Top (y+)
    push_face(
        [
            [min[0], max[1], min[2]],
            [min[0], max[1], max[2]],
            [max[0], max[1], max[2]],
            [max[0], max[1], min[2]],
        ],
        [0.0, 1.0, 0.0],
    );
}

pub mod feature {
    //! Per-vertex feature-type discriminant, written into `Vertex::feature_type`
    //! and read by both the WGSL fragment shader (see `shaders/features.wgsl`)
    //! and the CPU-side render-layer classifier (`FeatureLayer::from_f32`).
    //!
    //! Each discriminant reserves an integer slot. Sub-variants of a feature
    //! occupy fractional offsets inside the parent slot's slop band (typically
    //! ±0.25, occasionally ±0.5 — see the call-site comments in
    //! `shaders/city.wgsl`). When adding a discriminant:
    //!
    //! 1. Reserve a new integer slot unless the variant belongs to an existing
    //!    feature family (e.g. layered road surfaces reuse the road slot).
    //! 2. Update `shaders/features.wgsl` to match — `build.rs` cross-checks
    //!    the two at compile time and `tests/shader_source_test.rs` re-checks
    //!    at test time, so a drift fails the build.
    //! 3. Update `FeatureLayer::from_f32` in `src/render/buffers.rs` if the
    //!    new value should route to a dedicated render layer.
    //!
    //! See `docs/ARCHITECTURE.md` (render-layer splitting) for the broader
    //! design rationale.

    /// Ground/receiver surface — does not cast shadows.
    pub const TERRAIN: f32 = 0.0;
    /// Solid geometry that casts shadows. Drives the shadow-index filter.
    pub const BUILDING: f32 = 1.0;
    /// Flat road surface (base of the road slot at integer 2).
    pub const ROAD: f32 = 2.0;
    /// Layered road surface (bridge/overpass tier) reusing the road slot.
    pub const ROAD_LAYERED: f32 = 2.10;
    /// Pedestrian/path overlay sharing the road slop band (used by the
    /// bike/pedestrian overlay tint in `apply_bike_ped_overlay`).
    pub const ROAD_PATH: f32 = 2.25;
    /// Animated receiver surface — never casts shadows; see `water_normal`.
    pub const WATER: f32 = 3.0;
    /// Landuse base polygon (grass, parks, etc.).
    pub const LANDUSE: f32 = 4.0;
    /// Landuse overlay variant (bike/pedestrian tint band).
    pub const LANDUSE_OVERLAY: f32 = 4.25;
    /// Road markings painted on top of the road surface.
    pub const ROAD_MARKING: f32 = 5.0;
    /// Layered road-marking variant (markings on bridge/overpass tiers).
    pub const ROAD_MARKING_LAYERED: f32 = 5.10;
    /// Railway ribbons, treated as a distinct receiver layer for ordering.
    pub const RAILWAY: f32 = 6.0;
    /// Standalone point-feature geometry (trees, POIs, transit, etc.).
    pub const POINT_FEATURE: f32 = 7.0;
    /// Street-sign quads (always camera-facing billboards).
    pub const STREET_SIGN: f32 = 8.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_layout_includes_uvs() {
        assert_eq!(std::mem::size_of::<Vertex>(), 48);
        assert_eq!(Vertex::ATTRIBUTES.len(), 5);
    }

    #[test]
    fn append_box_emits_six_quads_with_inflation_safety() {
        let mut verts = Vec::new();
        let mut idxs = Vec::new();
        // Slab: zero extent on Y — degenerate-axis inflation must kick in.
        append_box(
            BoxSpec {
                min: [-1.0, 0.0, -1.0],
                max: [1.0, 0.0, 1.0],
                color: [1.0; 3],
                feature_type: feature::BUILDING,
            },
            &mut verts,
            &mut idxs,
        );

        assert_eq!(verts.len(), 24);
        assert_eq!(idxs.len(), 36);
        // Every triangle index must be in range.
        assert!(idxs.iter().all(|&i| (i as usize) < verts.len()));
        // All vertices carry the spec's feature_type.
        assert!(verts.iter().all(|v| v.feature_type == feature::BUILDING));
    }

    #[test]
    fn box_spec_centered_places_base_at_base_y() {
        let spec = BoxSpec::centered(
            (5.0, -3.0),
            10.0,
            (1.0, 2.0),
            4.0,
            [0.5; 3],
            feature::BUILDING,
        );
        assert_eq!(spec.min, [4.0, 10.0, -5.0]);
        assert_eq!(spec.max, [6.0, 14.0, -1.0]);
    }
}
