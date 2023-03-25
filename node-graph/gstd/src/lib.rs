// `macro_use` puts the log macros (`error!`, `warn!`, `debug!`, `info!` and `trace!`) in scope for the crate
#[macro_use]
extern crate log;

//pub mod value;
//#![feature(const_type_name)]

#[cfg(feature = "memoization")]
pub mod memo;

pub mod raster;

pub mod any;

#[cfg(feature = "gpu")]
pub mod executor;

#[cfg(feature = "quantization")]
pub mod quantization;

pub use graphene_core::*;

pub mod brush;
