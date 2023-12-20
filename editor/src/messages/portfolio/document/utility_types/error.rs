use graphene_core::raster::color::Color;

use thiserror::Error;

/// The error type used by the Graphite editor.
#[derive(Clone, Debug, Error)]
pub enum EditorError {
	#[error("Failed to execute operation:\n{0}")]
	InvalidOperation(String),

	#[error("Tried to construct an invalid color:\n{0:?}")]
	Color(String),

	#[error("The requested tool does not exist")]
	UnknownTool,

	#[error("The operation caused a document error:\n{0:?}")]
	Document(String),

	#[error("This document was created in an older version of the editor.\n\nBackwards compatibility is, regrettably, not present in the current alpha release.\n\nTechnical details:\n{0:?}")]
	DocumentDeserialization(String),

	#[error("{0}")]
	Misc(String),
}

macro_rules! derive_from {
	($type:ty, $kind:ident) => {
		impl From<$type> for EditorError {
			fn from(error: $type) -> Self {
				EditorError::$kind(format!("{error:?}"))
			}
		}
	};
}

derive_from!(&str, Misc);
derive_from!(String, Misc);
derive_from!(Color, Color);
