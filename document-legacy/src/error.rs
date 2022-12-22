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
	NotAShape,
	NotText,
	NotAnImage,
	NotAnImaginate,
	InvalidFile(String),
}

// TODO: change how BooleanOperationErrors are handled
impl From<BooleanOperationError> for DocumentError {
	fn from(err: BooleanOperationError) -> Self {
		DocumentError::InvalidFile(format!("{:?}", err))
	}
}
