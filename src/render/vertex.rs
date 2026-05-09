//! Re-exports shared vertex types from [`crate::mesh`].
//!
//! The canonical definitions live in [`crate::mesh`] so that the `world`
//! module does not need an upward dependency on `render`. This module
//! re-exports them for backwards compatibility with existing call sites.

pub use crate::mesh::{Vertex, feature};
