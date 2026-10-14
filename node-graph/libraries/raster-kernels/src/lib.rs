//! The kernels raster operations are carried out with, each a plain-data struct worked out once from its settings that then maps every color.

pub mod adjustments;
pub mod blending;
pub mod color_lookup_table;
mod kernel;

pub use kernel::*;
