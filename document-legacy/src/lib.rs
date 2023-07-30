// `macro_use` puts the log macros (`error!`, `warn!`, `debug!`, `info!` and `trace!`) in scope for the crate
#[macro_use]
extern crate log;

pub mod boolean_ops;
/// Contains constant values used by this crate.
pub mod consts;
pub mod document;
pub mod document_metadata;
/// Defines errors that can occur when using this crate.
pub mod error;
/// Utilities for computing intersections.
pub mod intersection;
pub mod layers;
pub mod operation;
pub mod response;

pub use document::LayerId;
pub use error::DocumentError;
pub use operation::Operation;
pub use response::DocumentResponse;
