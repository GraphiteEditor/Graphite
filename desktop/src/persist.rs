use crate::wrapper::messages::{DocumentId, PersistedState};

pub(crate) fn read_state() -> PersistedState {
	let path = state_file_path();
	let data = match std::fs::read_to_string(&path) {
		Ok(d) => d,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
			tracing::info!("No persistent data file found at {path:?}, starting fresh");
			return PersistedState::default();
		}
		Err(e) => {
			tracing::error!("Failed to read persistent data from disk: {e}");
			return PersistedState::default();
		}
	};
	let loaded = match ron::from_str(&data) {
		Ok(d) => d,
		Err(e) => {
			tracing::error!("Failed to deserialize persistent data: {e}");
			return PersistedState::default();
		}
	};

	garbage_collect_document_files(&loaded);
	loaded
}

pub(crate) fn write_state(state: PersistedState) {
	let state: &PersistedState = &state;
	let data = match ron::ser::to_string_pretty(state, Default::default()) {
		Ok(d) => d,
		Err(e) => {
			tracing::error!("Failed to serialize persistent data: {e}");
			return;
		}
	};
	if let Err(e) = std::fs::write(state_file_path(), data) {
		tracing::error!("Failed to write persistent data to disk: {e}");
	}
	garbage_collect_document_files(&state);
}

pub(crate) fn write_document_content(id: DocumentId, document_content: String) {
	if let Err(e) = std::fs::write(document_content_path(&id), document_content) {
		tracing::error!("Failed to write document {id:?} to disk: {e}");
	}
}

pub(crate) fn read_document_content(id: &DocumentId) -> Option<String> {
	std::fs::read_to_string(document_content_path(id)).ok()
}

pub(crate) fn delete_document(id: &DocumentId) {
	if let Err(e) = std::fs::remove_file(document_content_path(id)) {
		tracing::error!("Failed to delete document {id:?} from disk: {e}");
	}
}

fn garbage_collect_document_files(state: &PersistedState) {
	let valid_paths: std::collections::HashSet<_> = state.documents.iter().map(|doc| document_content_path(&doc.id)).collect();

	let directory = crate::dirs::app_autosave_documents_dir();
	let entries = match std::fs::read_dir(&directory) {
		Ok(entries) => entries,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
		Err(e) => {
			tracing::error!("Failed to read autosave documents directory: {e}");
			return;
		}
	};

	for entry in entries.flatten() {
		let path = entry.path();
		if path.is_file() && !valid_paths.contains(&path) {
			if let Err(e) = std::fs::remove_file(&path) {
				tracing::error!("Failed to remove orphaned document file {path:?}: {e}");
			}
		}
	}
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
