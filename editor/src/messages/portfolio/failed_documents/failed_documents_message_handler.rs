use crate::consts::FILE_EXTENSION;
use crate::messages::dialog::simple_dialogs;
use crate::messages::frontend::utility_types::{DocumentInfo, FileFilter};
use crate::messages::layout::utility_types::layout_widget::DialogLayoutHolder;
use crate::messages::portfolio::document_storage_io::{DocumentStoreHandle, document_key, legacy_path, remove_stored_document};
use crate::messages::prelude::*;
use document_container::AsyncContainer;

#[derive(ExtractField)]
pub struct FailedDocumentsMessageContext<'a> {
	pub document_store: &'a DocumentStoreHandle,
}

#[derive(Debug, Default, ExtractField)]
pub struct FailedDocumentsMessageHandler {
	/// Stored documents that failed to load. Their store entries stay until explicitly discarded.
	// TODO: Eventually remove this document upgrade code
	failed_to_load_documents: HashMap<DocumentId, DocumentInfo>,
}

#[message_handler_data]
impl MessageHandler<FailedDocumentsMessage, FailedDocumentsMessageContext<'_>> for FailedDocumentsMessageHandler {
	fn process_message(&mut self, message: FailedDocumentsMessage, responses: &mut VecDeque<Message>, context: FailedDocumentsMessageContext) {
		let FailedDocumentsMessageContext { document_store } = context;

		match message {
			// TODO: Eventually remove this document upgrade code
			FailedDocumentsMessage::ShowFailedToLoadDocumentsDialog => {
				if self.failed_to_load_documents.is_empty() {
					return;
				}
				let failed_document_names = self.failed_to_load_documents.values().map(display_name_with_fallback).collect();
				let dialog = simple_dialogs::FailedToLoadDocumentsDialog { failed_document_names };
				dialog.send_dialog_to_frontend(responses);
			}
			// TODO: Eventually remove this document upgrade code
			FailedDocumentsMessage::DiscardFailedToLoadDocuments => {
				for document_id in std::mem::take(&mut self.failed_to_load_documents).into_keys() {
					responses.add(remove_stored_document(document_store.clone(), document_id));
				}
			}
			// TODO: Eventually remove this document upgrade code
			FailedDocumentsMessage::DownloadFailedToLoadDocuments => {
				if self.failed_to_load_documents.is_empty() {
					return;
				}

				let files = recovery_file_names(self.failed_to_load_documents.values());
				let store = document_store.clone();
				responses.add(async move {
					let mut entries = Vec::new();
					for (document_id, filename) in files {
						let content = match store.open(document_key(document_id)).await {
							Ok(container) => container.read(legacy_path()).await.map(|bytes| bytes.as_slice().to_vec()),
							Err(error) => Err(error),
						};
						match content {
							Ok(bytes) => entries.push((filename, bytes)),
							Err(error) => log::error!("Reading the stored document {document_id:?} for recovery failed: {error}"),
						}
					}
					recovery_download(entries)
				});
			}
		}
	}

	advertise_actions!(FailedDocumentsMessageDiscriminant;);
}

impl FailedDocumentsMessageHandler {
	// TODO: Eventually remove this document upgrade code
	pub(crate) fn record_failure(&mut self, document_id: DocumentId, info: DocumentInfo) {
		self.failed_to_load_documents.insert(document_id, info);
	}
}

// TODO: Eventually remove this document upgrade code
fn display_name_with_fallback(info: &DocumentInfo) -> String {
	if info.name.trim().is_empty() {
		format!("Untitled Document ({:x})", info.id.0)
	} else {
		info.name.clone()
	}
}

// TODO: Eventually remove this document upgrade code
fn recovery_file_names<'a>(infos: impl Iterator<Item = &'a DocumentInfo>) -> Vec<(DocumentId, String)> {
	let mut used_names = HashSet::new();
	infos
		.map(|info| {
			let stem = sanitize_filename_stem(&info.name).unwrap_or_else(|| format!("document-{:x}", info.id.0));

			// A name that was already given out, even as another document's numbered copy, takes the next free number
			let mut unique = format!("{stem}.{FILE_EXTENSION}");
			let mut copy_number = 1;
			while !used_names.insert(unique.clone()) {
				unique = format!("{stem} ({copy_number}).{FILE_EXTENSION}");
				copy_number += 1;
			}

			(info.id, unique)
		})
		.collect()
}

// TODO: Eventually remove this document upgrade code
fn recovery_download(files: Vec<(String, Vec<u8>)>) -> Message {
	const FOLDER_NAME: &str = "Graphite Recovered Documents";

	if files.is_empty() {
		return DialogMessage::DisplayDialogError {
			title: "Failed to download".to_string(),
			description: "None of the failed documents could be read from storage.".to_string(),
		}
		.into();
	}

	if files.len() == 1 {
		let (filename, content) = files.into_iter().next().expect("just checked there's one entry");
		return FrontendMessage::TriggerSaveFile {
			name: filename,
			folder: None,
			filters: vec![FileFilter {
				name: "Graphite Document".into(),
				extensions: vec![FILE_EXTENSION.into()],
				mime_types: Vec::new(),
			}],
			content: serde_bytes::ByteBuf::from(content),
		}
		.into();
	}

	match build_recovery_zip(&files) {
		Ok(zip_bytes) => FrontendMessage::TriggerSaveFile {
			name: format!("{FOLDER_NAME}.zip"),
			folder: None,
			filters: vec![FileFilter {
				name: "Zip Archive".into(),
				extensions: vec!["zip".into()],
				mime_types: Vec::new(),
			}],
			content: serde_bytes::ByteBuf::from(zip_bytes),
		}
		.into(),
		Err(e) => {
			log::error!("Failed to build recovery zip: {e}");
			DialogMessage::DisplayDialogError {
				title: "Failed to download".to_string(),
				description: format!("Could not bundle the failed documents for download.\n\n{e}"),
			}
			.into()
		}
	}
}

/// Returns `None` if the name has no safe filename characters left or matches a Windows reserved device name, so callers fall back to an ID-based stem.
// TODO: Eventually remove this document upgrade code
fn sanitize_filename_stem(name: &str) -> Option<String> {
	let replaced: String = name
		.chars()
		.map(|c| {
			if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
				'_'
			} else {
				c
			}
		})
		.collect();

	// Trim dots to avoid `.` / `..` resolving against the parent directory, and to dodge Windows' trailing-dot/space quirks
	let trimmed = replaced.trim().trim_matches('.').trim();
	if trimmed.is_empty() {
		return None;
	}

	// Windows rejects these regardless of extension; superscript digits are normalized equivalently by some path APIs
	let first_segment = trimmed.split('.').next().unwrap_or("").to_ascii_uppercase();
	if matches!(
		first_segment.as_str(),
		"CON"
			| "PRN" | "AUX"
			| "NUL" | "COM1"
			| "COM2" | "COM3"
			| "COM4" | "COM5"
			| "COM6" | "COM7"
			| "COM8" | "COM9"
			| "COM¹" | "COM²"
			| "COM³" | "LPT1"
			| "LPT2" | "LPT3"
			| "LPT4" | "LPT5"
			| "LPT6" | "LPT7"
			| "LPT8" | "LPT9"
			| "LPT¹" | "LPT²"
			| "LPT³"
	) {
		return None;
	}

	Some(trimmed.to_string())
}

// TODO: Eventually remove this document upgrade code
fn build_recovery_zip(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
	use std::io::{Cursor, Write};
	use zip::write::{SimpleFileOptions, ZipWriter};

	let mut buffer = Cursor::new(Vec::<u8>::new());
	let mut writer = ZipWriter::new(&mut buffer);
	let options: SimpleFileOptions = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).unix_permissions(0o644);

	for (filename, content) in entries {
		writer.start_file(filename, options).map_err(|e| format!("start_file: {e}"))?;
		writer.write_all(content).map_err(|e| format!("write_all: {e}"))?;
	}

	writer.finish().map_err(|e| format!("finish: {e}"))?;
	Ok(buffer.into_inner())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn recovered_documents_sharing_a_name_with_a_numbered_copy_get_distinct_file_names() {
		let infos = [(1, "Art"), (2, "Art"), (3, "Art (1)")].map(|(id, name)| DocumentInfo {
			id: DocumentId(id),
			name: name.to_string(),
			path: None,
			is_saved: true,
		});

		let names: HashSet<_> = recovery_file_names(infos.iter()).into_iter().map(|(_, name)| name).collect();
		assert_eq!(names.len(), 3);
	}
}
