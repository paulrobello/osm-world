//! Streaming-LOD module boundary.
//!
//! Re-exports the tile/LOD data types used by the renderer. Runtime
//! incremental GPU upload (streaming tiles in/out of a fixed buffer budget
//! per frame) is not yet wired here — the types are in place but the upload
//! path is stubbed. See `docs/superpowers/plans/2026-05-02-phase3-streaming-lod.md`
//! and `docs/ARCHITECTURE.md` for the planned design.
// TODO(streaming): implement the per-frame tile upload/eviction loop.

pub mod lod;
pub mod tile;

pub use lod::{LodConfig, TileLod};
pub use tile::{TileAabb, TileCoord, TileDebugEntry, TileDebugState, TileRect};
