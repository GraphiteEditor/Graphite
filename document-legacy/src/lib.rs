// `macro_use` puts the log macros (`error!`, `warn!`, `debug!`, `info!` and `trace!`) in scope for the crate
#[macro_use]
extern crate log;

pub mod boolean_ops;
/// Contains the [Color](color::Color) type.
pub mod color;
/// Contains constant values used by Graphene.
pub mod consts;
pub mod document;
/// Defines errors that can occur when using Graphene.
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
