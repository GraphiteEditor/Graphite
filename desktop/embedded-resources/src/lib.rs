//! This crate provides `EMBEDDED_RESOURCES` that can be included in the desktop application binary.
//! It is intended to be used by the `embedded_resources` feature of the `graphite-desktop` crate.
//! The build script checks if the specified resources directory exists and sets the `embedded_resources` cfg flag accordingly.
//! If the resources directory does not exist, resources will not be embedded and a warning will be reported during compilation.

#[cfg(embedded_resources)]
pub static EMBEDDED_RESOURCES: Option<include_dir::Dir> = Some(include_dir::include_dir!("$EMBEDDED_RESOURCES"));

#[cfg(not(embedded_resources))]
pub static EMBEDDED_RESOURCES: Option<include_dir::Dir> = None;
