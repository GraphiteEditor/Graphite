// `macro_use` puts the log macros (`error!`, `warn!`, `debug!`, `info!` and `trace!`) in scope for the crate
// #[macro_use]
extern crate log;

pub mod consts;
pub mod document;
pub mod document_metadata;
pub mod error;
pub mod intersection;
pub mod layers;
pub mod operation;
pub mod response;

pub use document::LayerId;
pub use error::DocumentError;
pub use operation::Operation;
pub use response::DocumentResponse;
