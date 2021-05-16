use document_core::document::Document as InteralDocument;

use crate::communication::document_action_handler::DocumentActionHandler;

#[derive(Clone, Debug, Default)]
pub struct Document {
	pub document: InteralDocument,
	pub handler: DocumentActionHandler,
	pub name: String,
}
