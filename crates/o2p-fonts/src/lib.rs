//! Generic explicit font registration, deterministic resolution and shaping.
//! No system discovery, Office semantics, PDF serialization, or network access.
mod registry;
mod shaping;
pub use registry::*;
pub use shaping::{ShapeReport, shape};
