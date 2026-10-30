//! The kernels raster operations are carried out with: each a plain-data struct with the WGSL the GPU runs it by, and for those mapping each color alone, the CPU mapping that colors and gradients are adjusted by.
//! Running them on rasters is the job of `wgpu-executor`.

pub mod adjustments;
pub mod blending;
pub mod color_lookup_table;
pub mod generators;
mod kernel;

pub use kernel::*;
