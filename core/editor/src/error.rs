use crate::events::Event;
use crate::Color;
use document_core::DocumentError;
use thiserror::Error;

/// The error type used by the Graphite editor.
#[derive(Clone, Debug, Error)]
pub enum EditorError {
	#[error("Failed to execute operation: {0}")]
	InvalidOperation(String),
	#[error("Failed to dispatch event: {0}")]
	InvalidEvent(String),
	#[error("{0}")]
	Misc(String),
	#[error("Tried to construct an invalid color {0:?}")]
	Color(String),
	#[error("The requested tool does not exist")]
	UnknownTool,
	#[error("The operation caused a document error {0:?}")]
	Document(String),
}

macro_rules! derive_from {
	($type:ty, $kind:ident) => {
		impl From<$type> for EditorError {
			fn from(error: $type) -> Self {
				EditorError::$kind(format!("{:?}", error))
			}
		}
	};
}

derive_from!(&str, Misc);
derive_from!(String, Misc);
derive_from!(Color, Color);
derive_from!(Event, InvalidEvent);
derive_from!(DocumentError, Document);
