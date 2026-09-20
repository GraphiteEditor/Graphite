use crate::consts::FILE_EXTENSION;
use crate::messages::dialog::simple_dialogs;
use crate::messages::frontend::utility_types::{DocumentInfo, FileFilter};
use crate::messages::layout::utility_types::layout_widget::DialogLayoutHolder;
use crate::messages::portfolio::persistent_state::PersistentStateMessage;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct FailedDocumentsMessageContext<'a> {
	pub document_ids: &'a mut VecDeque<DocumentId>,
}

#[derive(Debug, Default, ExtractField)]
pub struct FailedDocumentsMessageHandler {
	/// Pairs of `(info, raw serialized content)` for autosaved documents that failed to deserialize.
	/// The info entries are folded back into `persisted_state_snapshot` so their on-disk autosave files survive garbage collection.
	// TODO: Eventually remove this document upgrade code
	failed_to_load_documents: HashMap<DocumentId, (DocumentInfo, String)>,
	/// In-flight count of autosaved-document loads from the initial startup batch; the batched failure dialog fires when this hits 0.
	// TODO: Eventually remove this document upgrade code
	pending_initial_autosave_loads: usize,
}

#[message_handler_data]
impl MessageHandler<FailedDocumentsMessage, FailedDocumentsMessageContext<'_>> for FailedDocumentsMessageHandler {
	fn process_message(&mut self, message: FailedDocumentsMessage, responses: &mut VecDeque<Message>, context: FailedDocumentsMessageContext) {
		let FailedDocumentsMessageContext { document_ids } = context;

		match message {
			// TODO: Eventually remove this document upgrade code
			FailedDocumentsMessage::ShowFailedToLoadDocumentsDialog => {
				if self.failed_to_load_documents.is_empty() {
					return;
				}
				let failed_document_names = self.failed_to_load_documents.values().map(|(info, _)| display_name_with_fallback(info)).collect();
				let dialog = simple_dialogs::FailedToLoadDocumentsDialog { failed_document_names };
				dialog.send_dialog_to_frontend(responses);
			}
			// TODO: Eventually remove this document upgrade code
			FailedDocumentsMessage::DiscardFailedToLoadDocuments => {
				let failed = std::mem::take(&mut self.failed_to_load_documents);
				for document_id in failed.keys() {
					document_ids.retain(|id| id != document_id);
					responses.add(PersistentStateMessage::DeleteDocument { document_id: *document_id });
				}
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				responses.add(PersistentStateMessage::WriteState);
			}
			// TODO: Eventually remove this document upgrade code
			FailedDocumentsMessage::DownloadFailedToLoadDocuments => {
				if self.failed_to_load_documents.is_empty() {
					return;
				}

				let mut used_names = HashSet::new();
				let files: Vec<(String, Vec<u8>)> = self
					.failed_to_load_documents
					.values()
					.map(|(info, content)| {
						let stem = sanitize_filename_stem(&info.name).unwrap_or_else(|| format!("document-{:x}", info.id.0));

						// A name that was already given out, even as another document's numbered copy, takes the next free number
						let mut unique = format!("{stem}.{FILE_EXTENSION}");
						let mut copy_number = 1;
						while !used_names.insert(unique.clone()) {
							unique = format!("{stem} ({copy_number}).{FILE_EXTENSION}");
							copy_number += 1;
						}

						(unique, content.as_bytes().to_vec())
					})
					.collect();

				const FOLDER_NAME: &str = "Graphite Recovered Documents";

				if files.len() == 1 {
					let (filename, content) = files.into_iter().next().expect("just checked there's one entry");
					responses.add(FrontendMessage::TriggerSaveFile {
						name: filename,
						folder: None,
						filters: vec![FileFilter {
							name: "Graphite Document".into(),
							extensions: vec![FILE_EXTENSION.into()],
							mime_types: Vec::new(),
						}],
						content: serde_bytes::ByteBuf::from(content),
					});
				} else {
					match build_recovery_zip(&files) {
						Ok(zip_bytes) => responses.add(FrontendMessage::TriggerSaveFile {
							name: format!("{FOLDER_NAME}.zip"),
							folder: None,
							filters: vec![FileFilter {
								name: "Zip Archive".into(),
								extensions: vec!["zip".into()],
								mime_types: Vec::new(),
							}],
							content: serde_bytes::ByteBuf::from(zip_bytes),
						}),
						Err(e) => {
							log::error!("Failed to build recovery zip: {e}");
							responses.add(DialogMessage::DisplayDialogError {
								title: "Failed to download".to_string(),
								description: format!("Could not bundle the failed documents for download.\n\n{e}"),
							});
						}
					}
				}
			}
		}
	}

	advertise_actions!(FailedDocumentsMessageDiscriminant;);
}

impl FailedDocumentsMessageHandler {
	// TODO: Eventually remove this document upgrade code
	/// Keeps a document that failed to load, with its raw serialized content, so it can be offered for download.
	pub(crate) fn record_failure(&mut self, document_id: DocumentId, info: DocumentInfo, serialized_content: String) {
		self.failed_to_load_documents.insert(document_id, (info, serialized_content));
	}

	// TODO: Eventually remove this document upgrade code
	/// Counts more autosaved documents of the startup batch whose loads the batched failure dialog waits for.
	pub(crate) fn expect_autosave_loads(&mut self, count: usize) {
		self.pending_initial_autosave_loads = self.pending_initial_autosave_loads.saturating_add(count);
	}

	// TODO: Eventually remove this document upgrade code
	/// Whether documents failed to load and none of the startup batch is still loading, so the failure dialog is due.
	pub(crate) fn has_failures_to_report(&self) -> bool {
		self.pending_initial_autosave_loads == 0 && !self.failed_to_load_documents.is_empty()
	}

	// TODO: Eventually remove this document upgrade code
	/// The info of each document that failed to load.
	pub(crate) fn failed_document_infos(&self) -> impl Iterator<Item = &DocumentInfo> {
		self.failed_to_load_documents.values().map(|(info, _)| info)
	}

	// TODO: Eventually remove this document upgrade code
	pub(crate) fn tick_autosave_load_progress(&mut self, responses: &mut VecDeque<Message>, failed: bool) {
		if self.pending_initial_autosave_loads > 0 {
			self.pending_initial_autosave_loads -= 1;
			if self.has_failures_to_report() {
				responses.add(FailedDocumentsMessage::ShowFailedToLoadDocumentsDialog);
			}
		} else if failed {
			responses.add(FailedDocumentsMessage::ShowFailedToLoadDocumentsDialog);
		}
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
	fn recovered_documents_sharing_a_name_with_a_numbered_copy_still_download() {
		let mut handler = FailedDocumentsMessageHandler::default();
		for (id, name) in [(1, "Art"), (2, "Art"), (3, "Art (1)")] {
			let info = DocumentInfo {
				id: DocumentId(id),
				name: name.to_string(),
				resources: None,
				path: None,
				is_saved: true,
			};
			handler.record_failure(DocumentId(id), info, "content".to_string());
		}

		let mut document_ids = VecDeque::new();
		let mut responses = VecDeque::new();
		let context = FailedDocumentsMessageContext { document_ids: &mut document_ids };
		handler.process_message(FailedDocumentsMessage::DownloadFailedToLoadDocuments, &mut responses, context);

		// A file name given out twice makes the archive fail to build, which shows an error dialog in place of the download
		assert!(matches!(responses.front(), Some(Message::Frontend(FrontendMessage::TriggerSaveFile { .. }))));
	}
}
