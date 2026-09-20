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
	pub(crate) failed_to_load_documents: HashMap<DocumentId, (DocumentInfo, String)>,
	/// In-flight count of autosaved-document loads from the initial startup batch; the batched failure dialog fires when this hits 0.
	// TODO: Eventually remove this document upgrade code
	pub(crate) pending_initial_autosave_loads: usize,
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

				let mut used_names: HashMap<String, u32> = HashMap::new();
				let files: Vec<(String, Vec<u8>)> = self
					.failed_to_load_documents
					.values()
					.map(|(info, content)| {
						let stem = sanitize_filename_stem(&info.name).unwrap_or_else(|| format!("document-{:x}", info.id.0));
						let base = format!("{stem}.{FILE_EXTENSION}");
						let unique = match used_names.get(&base).copied() {
							None => {
								used_names.insert(base.clone(), 1);
								base
							}
							Some(n) => {
								used_names.insert(base.clone(), n + 1);
								format!("{stem} ({n}).{FILE_EXTENSION}")
							}
						};
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
	pub(crate) fn tick_autosave_load_progress(&mut self, responses: &mut VecDeque<Message>, failed: bool) {
		if self.pending_initial_autosave_loads > 0 {
			self.pending_initial_autosave_loads -= 1;
			if self.pending_initial_autosave_loads == 0 && !self.failed_to_load_documents.is_empty() {
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
