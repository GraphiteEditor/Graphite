use graphite_editor::messages::clipboard::utility_types::ClipboardContentRaw;
use graphite_editor::messages::portfolio::ingest::utility_types::TypeHint;
use graphite_editor::messages::prelude::*;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{DesktopFrontendMessage, DesktopWrapperMessage, EditorMessage, SaveFileDialogContext};

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
		DesktopWrapperMessage::IngestFile { path, content, action } => {
			let hint = TypeHint::new("", &path);
			dispatcher.queue_editor_message(IngestMessage::Ingest {
				data: content.into(),
				action,
				hint,
				path: Some(path),
			});
		}
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
		},
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
