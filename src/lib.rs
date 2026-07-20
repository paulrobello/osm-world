//! `osm-world` -- 3D city renderer from OpenStreetMap data.
//!
//! This crate provides a WGPU desktop renderer and an Axum API server for
//! preparing map areas and launching renderer commands. See the
//! [architecture documentation](../docs/ARCHITECTURE.md) for the full module map,
//! data flow, and design decisions.

pub mod app;
pub mod atmosphere;
pub mod camera;
pub mod geo;
pub mod mesh;
pub mod render;
pub mod server;
pub mod stream;
pub mod ui;
pub mod visual_detail;
pub mod world;
