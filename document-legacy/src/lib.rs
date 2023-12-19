// `macro_use` puts the log macros (`error!`, `warn!`, `debug!`, `info!` and `trace!`) in scope for the crate
// #[macro_use]
extern crate log;

pub mod document;
pub mod document_metadata;
pub mod layers;

/// A set of different errors that can occur when using this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
	LayerNotFound(Vec<document::LayerId>),
	InvalidPath,
	NotFolder,
	InvalidFile(String),
}
