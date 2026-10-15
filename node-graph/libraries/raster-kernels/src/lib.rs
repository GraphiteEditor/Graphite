//! The kernels carrying out raster operations, such as color adjustments and procedural patterns, each worked out once from its settings.

pub mod adjustment_tables;
pub mod adjustments;
pub mod blending;
pub mod color_lookup_table;
pub mod effect_kernels;
pub mod generators;
pub mod raster_effects;
pub mod value_effects;
mod kernel;

pub use kernel::*;
pub use raster_effects::evaluate_raster_effects;
pub use value_effects::{evaluate_color_effects, evaluate_gradient_effects};
