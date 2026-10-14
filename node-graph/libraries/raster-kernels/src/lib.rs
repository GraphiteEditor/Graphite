//! The kernels carrying out raster operations, such as color adjustments and procedural patterns, each worked out once from its settings.

pub mod adjustments;
pub mod blending;
pub mod color_lookup_table;
pub mod generators;
mod kernel;

pub use kernel::*;
