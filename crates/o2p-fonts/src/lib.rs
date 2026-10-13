//! Generic explicit font registration, deterministic resolution and shaping.
//! No system discovery, Office semantics, PDF serialization, or network access.
mod metrics;
mod names;
mod registry;
mod shaping;
pub use metrics::{FontLineMetrics, FontVerticalMetrics, VerticalMetricSource};
pub use names::FontName;
pub use registry::*;
pub use shaping::{ShapeReport, shape};
