pub mod color;
pub mod document;
pub mod layers;
pub mod operation;
pub mod response;

pub use operation::Operation;
pub use response::DocumentResponse;

pub type LayerId = u64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DocumentError {
	LayerNotFound,
	InvalidPath,
	IndexOutOfBounds,
	NotAFolder,
}
