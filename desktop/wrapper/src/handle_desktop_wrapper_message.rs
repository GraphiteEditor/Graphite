use graphite_editor::messages::clipboard::utility_types::ClipboardContentRaw;
use graphite_editor::messages::prelude::*;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{DesktopFrontendMessage, DesktopWrapperMessage, EditorMessage, OpenFileDialogContext, SaveFileDialogContext};

pub(super) fn handle_desktop_wrapper_message(dispatcher: &mut DesktopWrapperMessageDispatcher, message: DesktopWrapperMessage) {
	match message {
		DesktopWrapperMessage::FromWeb(message) => {
			dispatcher.queue_editor_message(*message);
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
		DesktopWrapperMessage::LoadDocument {
			id,
			document,
			to_front,
			select_after_open,
		} => {
			let message = PortfolioMessage::OpenDocumentFileWithId {
				document_id: id,
				document_name: Some(document.name),
				document_path: document.path,
				document_serialized_content: document.content,
				document_is_auto_saved: true,
				document_is_saved: document.is_saved,
				to_front,
				select_after_open,
			};
			dispatcher.queue_editor_message(message);
		}
		DesktopWrapperMessage::SelectDocument { id } => {
			let message = PortfolioMessage::SelectDocument { document_id: id };
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
	}
}
