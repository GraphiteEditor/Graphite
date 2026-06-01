use graphite_editor::messages::clipboard::utility_types::ClipboardContentRaw;
use graphite_editor::messages::prelude::*;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{DesktopFrontendMessage, DesktopWrapperMessage, EditorMessage, OpenFileDialogContext, SaveFileDialogContext};

pub(super) fn handle_desktop_wrapper_message(dispatcher: &mut DesktopWrapperMessageDispatcher, message: DesktopWrapperMessage) {
	match message {
		DesktopWrapperMessage::FromWeb(message) => {
			dispatcher.queue_editor_message(*message);
		}
		DesktopWrapperMessage::Wake => {
			dispatcher.queue_editor_message(EditorMessage::Future(FutureMessage::Wake));
		}
		DesktopWrapperMessage::Input(message) => {
			dispatcher.queue_editor_message(EditorMessage::InputPreprocessor(message));
		}
		DesktopWrapperMessage::FileDialogResult { path, content, context } => match context {
			OpenFileDialogContext::Open => {
				dispatcher.queue_desktop_wrapper_message(DesktopWrapperMessage::OpenFile { path, content });
			}
			OpenFileDialogContext::Import => {
				dispatcher.queue_desktop_wrapper_message(DesktopWrapperMessage::ImportFile { path, content });
			}
		},
		DesktopWrapperMessage::SaveFileDialogResult { path, context } => match context {
			SaveFileDialogContext::Document { document_id, content } => {
				dispatcher.respond(DesktopFrontendMessage::WriteFile { path: path.clone(), content });
				dispatcher.queue_editor_message(EditorMessage::Portfolio(PortfolioMessage::DocumentPassMessage {
					document_id,
					message: DocumentMessage::SavedDocument { path: Some(path) },
				}));
			}
			SaveFileDialogContext::File { content } => {
				dispatcher.respond(DesktopFrontendMessage::WriteFile { path, content });
			}
			SaveFileDialogContext::MultipleFiles { files, expected_extension } => {
				// Treat the chosen path as the folder name. Strip only the export's expected extension (e.g. ".png" for a
				// PNG animation export) so that arbitrary dotted folder names like `v1.0` are preserved as-is, while a user
				// who typed `MyAnim.png` still gets a `MyAnim/` folder rather than a `MyAnim.png/` folder.
				// The `WriteFile` handler creates parent directories if they don't exist, so the folder is materialized on first write.
				let folder = match path.extension().and_then(|e| e.to_str()) {
					Some(ext) if ext.eq_ignore_ascii_case(&expected_extension) => path.with_extension(""),
					_ => path,
				};
				for (filename, content) in files {
					let file_path = folder.join(&filename);
					dispatcher.respond(DesktopFrontendMessage::WriteFile { path: file_path, content });
				}
			}
		},
		DesktopWrapperMessage::OpenFile { path, content } => {
			let message = PortfolioMessage::OpenFile { path, content };
			dispatcher.queue_editor_message(message);
		}
		DesktopWrapperMessage::ImportFile { path, content } => {
			let message = PortfolioMessage::ImportFile { path, content };
			dispatcher.queue_editor_message(message);
		}
		DesktopWrapperMessage::PollNodeGraphEvaluation => dispatcher.poll_node_graph_evaluation(),
		DesktopWrapperMessage::UpdateMaximized { maximized } => {
			let message = FrontendMessage::UpdateMaximized { maximized };
			dispatcher.queue_editor_message(message);
		}
		DesktopWrapperMessage::UpdateFullscreen { fullscreen } => {
			let message = FrontendMessage::UpdateFullscreen { fullscreen };
			dispatcher.queue_editor_message(message);
		}
		DesktopWrapperMessage::LoadDocumentContent { id, document } => {
			let message = PersistentStateMessage::LoadDocument { document_id: id, document };
			dispatcher.queue_editor_message(message);
		}
		DesktopWrapperMessage::LoadPersistedState { state } => {
			let message = PersistentStateMessage::LoadState { state };
			dispatcher.queue_editor_message(message);
		}
		DesktopWrapperMessage::LoadPreferences { preferences } => {
			let message = PreferencesMessage::Load { preferences };
			dispatcher.queue_editor_message(message);
		}
		#[cfg(target_os = "macos")]
		DesktopWrapperMessage::MenuEvent { id } => {
			if let Some(message) = crate::utils::menu::parse_item_path(id) {
				dispatcher.queue_editor_message(message);
			} else {
				tracing::error!("Received a malformed MenuEvent id");
			}
		}
		#[cfg(not(target_os = "macos"))]
		DesktopWrapperMessage::MenuEvent { id: _ } => {}
		DesktopWrapperMessage::ClipboardReadResult { content } => {
			if let Some(content) = content {
				let message = ClipboardMessage::ReadClipboard {
					content: ClipboardContentRaw::Text(content),
				};
				dispatcher.queue_editor_message(message);
			}
		}
		DesktopWrapperMessage::PointerLockMove { x, y } => {
			let message = AppWindowMessage::PointerLockMove { x, y };
			dispatcher.queue_editor_message(message);
		}
		DesktopWrapperMessage::LoadThirdPartyLicenses { text } => {
			let message = DialogMessage::RequestLicensesThirdPartyDialogWithLicenseText { license_text: text };
			dispatcher.queue_editor_message(message);
		}
	}
}
