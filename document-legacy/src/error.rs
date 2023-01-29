use super::LayerId;
use crate::boolean_ops::BooleanOperationError;

/// A set of different errors that can occur when using this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
	LayerNotFound(Vec<LayerId>),
	InvalidPath,
	IndexOutOfBounds,
	NotAFolder,
	NonReorderableSelection,
	NotShape,
	NotText,
	NotNodeGraph,
	InvalidFile(String),
	BooleanOperationError(BooleanOperationError),
}

impl From<BooleanOperationError> for DocumentError {
	fn from(err: BooleanOperationError) -> Self {
		DocumentError::BooleanOperationError(err)
	}
}
