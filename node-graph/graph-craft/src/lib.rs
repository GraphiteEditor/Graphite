#[macro_use]
extern crate log;

pub mod document;
pub mod proto;

pub mod executor;

#[cfg(feature = "gpu")]
pub mod gpu;
