use graphene::color::Color;
use graphene::DocumentError;

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

	#[error("A rollback was initiated but no transaction was in progress")]
	NoTransactionInProgress,

	#[error("{0}")]
	Misc(String),
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
derive_from!(DocumentError, Document);
