pub mod color;
pub mod document;
pub mod layers;
pub mod operation;
mod shape_points;

pub use operation::Operation;

type LayerId = u64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DocumentError {
	LayerNotFound,
	InvalidPath,
	IndexOutOfBounds,
}
