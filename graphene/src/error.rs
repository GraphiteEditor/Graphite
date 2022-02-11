use super::LayerId;

/// A representation of different errors that can occur when using graphene.
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
