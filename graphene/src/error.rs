use super::LayerId;
use crate::boolean_ops::BooleanOperationError;

/// A representation of different errors that can occur when using Graphene.
#[derive(Debug, Clone, PartialEq)]
pub enum DocumentError {
	LayerNotFound(Vec<LayerId>),
	InvalidPath,
	IndexOutOfBounds,
	NotAFolder,
	NonReorderableSelection,
	NotAShape,
	NotText,
	InvalidFile(String),
}

// TODO: change how BooleanOperationErrors are handled
impl From<BooleanOperationError> for DocumentError {
	fn from(err: BooleanOperationError) -> Self {
		DocumentError::InvalidFile(format!("{:?}", err))
	}
}
