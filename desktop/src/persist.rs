use std::collections::HashMap;

use graphite_desktop_wrapper::messages::{Document, DocumentId};

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistentData {
	documents: HashMap<DocumentId, Document>,
	current_document: Option<DocumentId>,
	document_order: Vec<DocumentId>,
}

impl PersistentData {
	pub(crate) fn write_document(&mut self, id: DocumentId, document: Document) {
		self.documents.insert(id, document);
	}

	pub(crate) fn delete_document(&mut self, id: &DocumentId) {
		self.documents.remove(id);
	}

	pub(crate) fn get_current_document(&self) -> Option<(DocumentId, Document)> {
		self.current_document.and_then(|id| Some((id, self.documents.get(&id)?.clone())))
	}

	pub(crate) fn get_documents_except_current(&self) -> Vec<(DocumentId, Document)> {
		self.documents
			.iter()
			.filter(|(id, _)| Some(**id) != self.current_document)
			.map(|(id, doc)| (*id, doc.clone()))
			.collect()
	}

	pub(crate) fn set_current_document(&mut self, id: Option<DocumentId>) {
		self.current_document = id;
	}

	pub(crate) fn set_document_order(&mut self, order: Vec<DocumentId>) {
		self.document_order = order;
	}

	pub(crate) fn get_document_order(&self) -> &Vec<DocumentId> {
		&self.document_order
	}

	pub(crate) fn write_to_disk(&self) {
		let data = match ron::to_string(self) {
			Ok(d) => d,
			Err(e) => {
				tracing::error!("Failed to serialize persistent data: {e}");
				return;
			}
		};
		if let Err(e) = std::fs::write(Self::persistence_file_path(), data) {
			tracing::error!("Failed to write persistent data to disk: {e}");
		}
	}

	pub(crate) fn load_from_disk(&mut self) {}

	fn persistence_file_path() -> std::path::PathBuf {
		let mut path = crate::dirs::graphite_data_dir();
		path.push("persistent_data.ron");
		path
	}
}
