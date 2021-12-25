pub mod color;
pub mod consts;
pub mod document;
pub mod intersection;
pub mod layers;
pub mod operation;
pub mod response;

pub use intersection::Quad;
pub use operation::Operation;
pub use response::DocumentResponse;

pub type LayerId = u64;

#[derive(Debug, Clone, PartialEq)]
pub enum DocumentError {
	LayerNotFound,
	InvalidPath,
	IndexOutOfBounds,
	NotAFolder,
	NonReorderableSelection,
	NotAShape,
	InvalidFile(String),
}
