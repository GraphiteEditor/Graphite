pub mod color;
pub mod document;
pub mod intersection;
pub mod layers;
pub mod operation;
pub mod response;

pub use intersection::Quad;
pub use operation::Operation;
pub use response::DocumentResponse;

pub type LayerId = u64;

pub const GRAPHITE_DOCUMENT_VERSION:&'static str = "0.0.1";

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
