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
}
