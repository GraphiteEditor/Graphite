pub mod boolean_ops;
pub mod color;
pub mod consts;
pub mod document;
pub mod intersection;
pub mod layers;
pub mod operation;
pub mod response;

use boolean_ops::BooleanOperationError;
pub use intersection::Quad;
pub use operation::Operation;
pub use response::DocumentResponse;

pub type LayerId = u64;

#[derive(Debug, Clone, PartialEq)]
pub enum DocumentError {
	LayerNotFound(Vec<LayerId>),
	InvalidPath,
	IndexOutOfBounds,
	NotAFolder,
	NonReorderableSelection,
	NotAShape,
	InvalidFile(String),
}

//TODO: change how BooleanOperationErrors are handled
impl From<BooleanOperationError> for DocumentError {
	fn from(err: BooleanOperationError) -> Self {
		DocumentError::InvalidFile(format!("{:?}", err))
	}
}
