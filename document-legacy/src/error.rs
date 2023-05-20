use super::LayerId;

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
}
