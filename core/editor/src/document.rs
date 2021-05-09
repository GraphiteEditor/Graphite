use document_core::document::Document as InteralDocument;

use crate::dispatcher::document_event_handler::DocumentActionHandler;

#[derive(Clone, Debug, Default)]
pub struct Document {
	pub document: InteralDocument,
	pub handler: DocumentActionHandler,
	pub name: String,
}
