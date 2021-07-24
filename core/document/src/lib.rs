//! Graphite Document Core Library: `/core/document/`
//!
//! A stateless library for updating Graphite design document (GDD) files.
//! The official Graphite CLI and Editor Core Library are the primary users, but this library is intended to be useful
//! to any application that wants to link the library for the purpose of updating GDD files by sending edit operations.
//! Optionally depends on the Renderer Core Library if rendering is required.

pub mod bounding_box;
pub mod color;
pub mod document;
pub mod intersection;
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
	NonReorderableSelection,
}
