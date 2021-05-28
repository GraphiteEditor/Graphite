use crate::input::mouse::DocumentTransform;
use document_core::document::Document as InteralDocument;

#[derive(Clone, Debug, Default)]
pub struct Document {
	pub document: InteralDocument,
	pub name: String,
	pub document_transform: DocumentTransform,
}
