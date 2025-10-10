use graphite_desktop_wrapper::messages::{Document, DocumentId, Preferences};

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistentData {
	documents: DocumentStore,
	current_document: Option<DocumentId>,
	#[serde(skip)]
	document_order: Option<Vec<DocumentId>>,
}

impl PersistentData {
	pub(crate) fn write_document(&mut self, id: DocumentId, document: Document) {
		self.documents.write(id, document);
		if let Some(order) = &self.document_order {
			self.documents.force_order(order);
		}
		self.flush();
	}

	pub(crate) fn delete_document(&mut self, id: &DocumentId) {
		if Some(*id) == self.current_document {
			self.current_document = None;
		}
		self.documents.delete(id);
		self.flush();
	}

	pub(crate) fn current_document_id(&self) -> Option<DocumentId> {
		match self.current_document {
			Some(id) => Some(id),
			None => Some(*self.documents.document_ids().first()?),
		}
	}

	pub(crate) fn current_document(&self) -> Option<(DocumentId, Document)> {
		let current_id = self.current_document_id()?;
		Some((current_id, self.documents.read(&current_id)?))
	}

	pub(crate) fn documents_before_current(&self) -> Vec<(DocumentId, Document)> {
		let Some(current_id) = self.current_document_id() else {
			return Vec::new();
		};
		self.documents
			.document_ids()
			.into_iter()
			.take_while(|id| *id != current_id)
			.filter_map(|id| Some((id, self.documents.read(&id)?)))
			.collect()
	}

	pub(crate) fn documents_after_current(&self) -> Vec<(DocumentId, Document)> {
		let Some(current_id) = self.current_document_id() else {
			return Vec::new();
		};
		self.documents
			.document_ids()
			.into_iter()
			.skip_while(|id| *id != current_id)
			.skip(1)
			.filter_map(|id| Some((id, self.documents.read(&id)?)))
			.collect()
	}

	pub(crate) fn set_current_document(&mut self, id: DocumentId) {
		self.current_document = Some(id);
		self.flush();
	}

	pub(crate) fn set_document_order(&mut self, order: Vec<DocumentId>) {
		self.document_order = Some(order);
		self.flush();
	}

	pub(crate) fn write_preferences(&mut self, preferences: Preferences) {
		let Ok(preferences) = ron::ser::to_string_pretty(&preferences, Default::default()) else {
			tracing::error!("Failed to serialize preferences");
			return;
		};
		std::fs::write(Self::preferences_file_path(), &preferences).unwrap_or_else(|e| {
			tracing::error!("Failed to write preferences to disk: {e}");
		});
	}

	pub(crate) fn load_preferences(&self) -> Option<Preferences> {
		let data = std::fs::read_to_string(Self::preferences_file_path()).ok()?;
		let preferences = ron::from_str(&data).ok()?;
		Some(preferences)
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
		let mut path = crate::dirs::graphite_data_dir();
		path.push(crate::consts::APP_STATE_FILE_NAME);
		path
	}

	fn preferences_file_path() -> std::path::PathBuf {
		let mut path = crate::dirs::graphite_data_dir();
		path.push(crate::consts::APP_PREFERENCES_FILE_NAME);
		path
	}
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct DocumentStore(Vec<DocumentInfo>);
impl DocumentStore {
	fn write(&mut self, id: DocumentId, document: Document) {
		let meta = DocumentInfo::new(id, &document);
		if let Some(existing) = self.0.iter_mut().find(|meta| meta.id == id) {
			*existing = meta;
		} else {
			self.0.push(meta);
		}
		if let Err(e) = std::fs::write(Self::document_path(&id), document.content) {
			tracing::error!("Failed to write document {id:?} to disk: {e}");
		}
	}

	fn delete(&mut self, id: &DocumentId) {
		self.0.retain(|meta| meta.id != *id);
		if let Err(e) = std::fs::remove_file(Self::document_path(id)) {
			tracing::error!("Failed to delete document {id:?} from disk: {e}");
		}
	}

	fn read(&self, id: &DocumentId) -> Option<Document> {
		let meta = self.0.iter().find(|meta| meta.id == *id)?;
		let content = std::fs::read_to_string(Self::document_path(id)).ok()?;
		Some(Document {
			content,
			name: meta.name.clone(),
			path: meta.path.clone(),
			is_saved: meta.is_saved,
		})
	}

	fn force_order(&mut self, desired_order: &Vec<DocumentId>) {
		let mut ordered_prefix_len = 0;
		for id in desired_order {
			if let Some(offset) = self.0[ordered_prefix_len..].iter().position(|meta| meta.id == *id) {
				let found_index = ordered_prefix_len + offset;
				if found_index != ordered_prefix_len {
					self.0[ordered_prefix_len..=found_index].rotate_right(1);
				}
				ordered_prefix_len += 1;
			}
		}
		self.0.truncate(ordered_prefix_len);
	}

	fn document_ids(&self) -> Vec<DocumentId> {
		self.0.iter().map(|meta| meta.id).collect()
	}

	fn document_path(id: &DocumentId) -> std::path::PathBuf {
		let mut path = crate::dirs::graphite_autosave_documents_dir();
		path.push(format!("{:x}.graphite", id.0));
		path
	}
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DocumentInfo {
	id: DocumentId,
	name: String,
	path: Option<std::path::PathBuf>,
	is_saved: bool,
}
impl DocumentInfo {
	fn new(id: DocumentId, Document { name, path, is_saved, .. }: &Document) -> Self {
		Self {
			id,
			name: name.clone(),
			path: path.clone(),
			is_saved: *is_saved,
		}
	}
}
