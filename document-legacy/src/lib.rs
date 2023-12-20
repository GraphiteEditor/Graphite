// `macro_use` puts the log macros (`error!`, `warn!`, `debug!`, `info!` and `trace!`) in scope for the crate
// #[macro_use]
extern crate log;

pub mod document;
pub mod document_metadata;
pub mod layers;
