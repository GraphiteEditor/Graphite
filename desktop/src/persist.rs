use crate::wrapper::messages::{Document, DocumentId, PersistedDocumentInfo};

// Wraps PersistedState (shared with the web frontend) and adds desktop-specific behavior like file I/O
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistentData {
	documents: Vec<PersistedDocumentInfo>,
	current_document: Option<DocumentId>,
	#[serde(skip)]
	document_order: Option<Vec<DocumentId>>,
}

impl PersistentData {
	pub(crate) fn write_document(&mut self, id: DocumentId, document: Document) {
		// Update or add the document metadata
		let info = PersistedDocumentInfo {
			id,
			name: document.name.clone(),
			path: document.path.clone(),
			is_saved: document.is_saved,
		};
		if let Some(existing) = self.documents.iter_mut().find(|doc| doc.id == id) {
			*existing = info;
		} else {
			self.documents.push(info);
		}

		// Write the document content to a separate file
		if let Err(e) = std::fs::write(Self::document_content_path(&id), document.content) {
			tracing::error!("Failed to write document {id:?} to disk: {e}");
		}

		if let Some(order) = &self.document_order {
			self.force_order(order.clone());
		}
		self.flush();
	}

	pub(crate) fn delete_document(&mut self, id: &DocumentId) {
		if Some(*id) == self.current_document {
			self.current_document = None;
		}

		self.documents.retain(|doc| doc.id != *id);
		if let Err(e) = std::fs::remove_file(Self::document_content_path(id)) {
			tracing::error!("Failed to delete document {id:?} from disk: {e}");
		}

		self.flush();
	}

	pub(crate) fn current_document_id(&self) -> Option<DocumentId> {
		match self.current_document {
			Some(id) => Some(id),
			None => Some(self.documents.first()?.id),
		}
	}

	pub(crate) fn current_document(&self) -> Option<(DocumentId, Document)> {
		let current_id = self.current_document_id()?;
		Some((current_id, self.read_document(&current_id)?))
	}

	pub(crate) fn documents_before_current(&self) -> Vec<(DocumentId, Document)> {
		let Some(current_id) = self.current_document_id() else {
			return Vec::new();
		};
		self.documents
			.iter()
			.take_while(|doc| doc.id != current_id)
			.filter_map(|doc| Some((doc.id, self.read_document(&doc.id)?)))
			.collect()
	}

	pub(crate) fn documents_after_current(&self) -> Vec<(DocumentId, Document)> {
		let Some(current_id) = self.current_document_id() else {
			return Vec::new();
		};
		self.documents
			.iter()
			.skip_while(|doc| doc.id != current_id)
			.skip(1)
			.filter_map(|doc| Some((doc.id, self.read_document(&doc.id)?)))
			.collect()
	}

	pub(crate) fn set_current_document(&mut self, id: DocumentId) {
		self.current_document = Some(id);
		self.flush();
	}

	pub(crate) fn force_document_order(&mut self, order: Vec<DocumentId>) {
		self.force_order(order.clone());
		self.document_order = Some(order);
		self.flush();
	}

	// Reads serialized document content from disk and combines it with the stored metadata
	fn read_document(&self, id: &DocumentId) -> Option<Document> {
		let info = self.documents.iter().find(|doc| doc.id == *id)?;
		let content = std::fs::read_to_string(Self::document_content_path(id)).ok()?;
		Some(Document {
			content,
			name: info.name.clone(),
			path: info.path.clone(),
			is_saved: info.is_saved,
		})
	}

	// Reorders the documents array to match a desired ordering, keeping unmentioned documents at the end
	fn force_order(&mut self, desired_order: Vec<DocumentId>) {
		let mut ordered_prefix_length = 0;
		for id in &desired_order {
			if let Some(offset) = self.documents[ordered_prefix_length..].iter().position(|doc| doc.id == *id) {
				let found_index = ordered_prefix_length + offset;
				if found_index != ordered_prefix_length {
					self.documents[ordered_prefix_length..=found_index].rotate_right(1);
				}
				ordered_prefix_length += 1;
			}
		}
	}

	fn flush(&self) {
		let data = match ron::ser::to_string_pretty(self, Default::default()) {
			Ok(d) => d,
			Err(e) => {
				tracing::error!("Failed to serialize persistent data: {e}");
				return;
			}
		};
		if let Err(e) = std::fs::write(Self::state_file_path(), data) {
			tracing::error!("Failed to write persistent data to disk: {e}");
		}
	}

	pub(crate) fn load_from_disk(&mut self) {
		let path = Self::state_file_path();
		let data = match std::fs::read_to_string(&path) {
			Ok(d) => d,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
				tracing::info!("No persistent data file found at {path:?}, starting fresh");
				return;
			}
			Err(e) => {
				tracing::error!("Failed to read persistent data from disk: {e}");
				return;
			}
		};
		let loaded = match ron::from_str(&data) {
			Ok(d) => d,
			Err(e) => {
				tracing::error!("Failed to deserialize persistent data: {e}");
				return;
			}
		};
		*self = loaded;
	}

	fn state_file_path() -> std::path::PathBuf {
		let mut path = crate::dirs::app_data_dir();
		path.push(crate::consts::APP_STATE_FILE_NAME);
		path
	}

	fn document_content_path(id: &DocumentId) -> std::path::PathBuf {
		let mut path = crate::dirs::app_autosave_documents_dir();
		path.push(format!("{:x}.{}", id.0, graphite_desktop_wrapper::FILE_EXTENSION));
		path
	}
}
